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

/// (G1 Nov 13, N24 / N25) Every flee-check world: the settle, then the design's daylight detectors
/// (`Run::start` holds them back; the game's build lays them, so sight, paths and the shell statics
/// see them), then no redsim model error (an unseated rider, an unreadable light).
fn settle_full(run: &mut Run) -> Result<(), String> {
    while run.settle_tick() {}
    run.place_detectors();
    model_errors(&run.world)
}

/// (G1 Nov 13, N25) The world's model errors fail the static (after each build and query batch).
fn model_errors(w: &redsim::World) -> Result<(), String> {
    if w.errors.is_empty() { Ok(()) } else { Err(format!("redsim model error(s): {}", w.errors.iter().take(6).cloned().collect::<Vec<_>>().join("; "))) }
}

/// (G1 Nov 13, N5 / K10, ONE STATE SOURCE) The design as the settled world holds it: every cell of
/// the spec's box whose block the settle (and the daylight detectors' placement) changed, in its
/// settled state (a rail's shape, a trapdoor, flowing water); with each change printed.
pub fn settled_design(spec: &FarmSpec) -> Result<(FarmSpec, Vec<String>), String> {
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let map: HashMap<BlockPos, redsim::block::Block> = spec.blocks.iter().map(|(p, b)| (*p, *b)).collect();
    let mut s = spec.clone();
    let mut diffs = Vec::new();
    let mut changed: HashMap<BlockPos, redsim::block::Block> = HashMap::new();
    for y in spec.lo.y..=spec.hi.y {
        for z in spec.lo.z..=spec.hi.z {
            for x in spec.lo.x..=spec.hi.x {
                let p = BlockPos::new(x, y, z);
                let now = w.get(p);
                let file = map.get(&p).copied().unwrap_or(redsim::block::Block::Air);
                if now != file {
                    diffs.push(format!("{x},{y},{z} {} -> {}", file.to_string().trim_start_matches("minecraft:"), now.to_string().trim_start_matches("minecraft:")));
                    changed.insert(p, now);
                }
            }
        }
    }
    s.blocks.retain(|(p, _)| !changed.contains_key(p));
    s.blocks.extend(changed.into_iter().filter(|(_, b)| !b.is_air()));
    Ok((s, diffs))
}

/// One source's result at a place: its cells, and those with no complete path with the chance one
/// event picks each (a share of 0: an edge cell).
#[derive(Default, Clone)]
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
#[derive(Clone)]
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
    /// (ironspec Nov 12 21:58: the 6-8 gate sight-gated) The least distance to a zombie within the
    /// flee bar that some point of the place sees, or is hidden from only by partial blocks (the
    /// robust hiding: a full-collider block on the eye ray hides it); None: none in sight.
    pub near_seen: Option<f64>,
    /// (G1 Nov 13, N22) The least distance to a declared pillager over the place's points (the flee
    /// counts pillagers within 6, so staleness does too); None: no pillager.
    pub near_pill: Option<f64>,
}

/// A cart's path through rail cell `c` (shape from `state`) as a segment, 0.0625 over the rail's
/// surface: straight and sloped rails from edge to edge (a slope rising toward its side), a curve
/// between its two exits (the cart runs the chord between them). None for an unknown shape. (G1 Nov
/// 13: ONE cart-path model: the segment is `along_rail`'s t 0 to t 1.)
pub fn rail_segment(c: BlockPos, state: &str) -> Option<(Vec3, Vec3)> {
    let s = state.split("shape=").nth(1)?.split([',', ']']).next()?;
    const SHAPES: [&str; 10] = ["north_south", "east_west", "ascending_east", "ascending_west", "ascending_south", "ascending_north", "south_east", "south_west", "north_west", "north_east"];
    SHAPES.contains(&s).then(|| (along_rail(c, state, 0.0), along_rail(c, state, 1.0)))
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
pub fn pickup_check(spec_in: &FarmSpec, control: bool) -> (Vec<String>, usize, usize) {
    type Bx = ([f64; 3], [f64; 3]);
    // (G1 Nov 13, N2: the positive control is a real entity line, a boated zombie 0.1 beside the
    // first rail cell's cart, read through the same entity filter as the design's posts)
    let mut spec_c = spec_in.clone();
    if control {
        let mut rails0: Vec<(BlockPos, String)> = spec_in.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
        rails0.sort_by_key(|(p, _)| (p.y, p.z, p.x));
        if let Some((c, st)) = rails0.first() {
            let m = along_rail(*c, st, 0.5);
            spec_c.entities.push(("minecraft:oak_boat".to_string(), [m.x + 0.49 + 0.1 + 0.6875, m.y, m.z], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string())));
        }
    }
    let spec = &spec_c;
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
    let samples = |c: BlockPos, st: &str| -> Vec<Vec3> { (0..=20).map(|i| along_rail(c, st, i as f64 / 20.0)).collect() };
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
    (lines, posts.len(), bad.len())
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
    settle_full(&mut run)?;
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
        // (ironspec Nov 13 06:00 real: a control that can't be laid is an error, never a silent
        // no-op)
        let Some((c, _)) = rails.iter().find(|(c, _)| !taken.contains(&BlockPos::new(c.x, c.y + 1, c.z))) else {
            return Err("--eye-control: no rail cell has a free cell above it for the control's stone".to_string());
        };
        s.blocks.push((BlockPos::new(c.x, c.y + 1, c.z), redsim::block::Block::parse("minecraft:stone")?));
    }
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(&s, &o)?;
    settle_full(&mut run)?;
    let w = &mut run.world;
    let mut hits = Vec::new();
    // (G1 Nov 13, N16: the samples are the eye positions tested, three a rail cell)
    let mut tested = 0usize;
    for (c, state) in &rails {
        let p0 = along_rail(*c, state, 0.5);
        let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
        // (a failed spawn is an error, never a skipped rail: ironspec 06:00 real, POPULATIONS)
        let ci = redsim::entity::index_of(w, cart).ok_or_else(|| format!("eye check: the cart at {},{},{} did not spawn", c.x, c.y, c.z))?;
        let vid = *w.entities.list[ci].passengers.first().ok_or_else(|| format!("eye check: the cart at {},{},{} has no rider", c.x, c.y, c.z))?;
        let vi = redsim::entity::index_of(w, vid).ok_or_else(|| format!("eye check: the rider at {},{},{} did not spawn", c.x, c.y, c.z))?;
        let (width, eye_h) = match &w.entities.list[vi].kind {
            EntityKind::Mob(m) => (m.dims().0 as f64, m.eye() as f64),
            _ => return Err(format!("eye check: the rider at {},{},{} is not a mob", c.x, c.y, c.z)),
        };
        for t in [0.1, 0.5, 0.9] {
            tested += 1;
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
    Ok((tested, hits))
}

/// The eye check's lines: the suffocation gate (a failing line) and the blind places (information);
/// with the count of places with a suffocating block at an eye (G1 N16: a count, not a flag).
pub fn eye_lines(positions: usize, hits: &[EyeHit], show: usize) -> (Vec<String>, usize) {
    let suff: Vec<&EyeHit> = hits.iter().filter(|h| h.suffocating).collect();
    let blind: Vec<&EyeHit> = hits.iter().filter(|h| !h.suffocating).collect();
    let n_suff = suff.iter().map(|h| h.cell).collect::<std::collections::HashSet<_>>().len();
    let mut lines = vec![format!(
        "rider eyes (every rail cell, the rider seated at three points; the census: no suffocating block at a rider's eye): {positions} eye positions tested; {n_suff} places with a suffocating block at the eye{}; {} blind places (the eye inside a collision shape that doesn't suffocate: the sight ray starts inside it)",
        if suff.is_empty() { String::new() } else { format!(" ({})", suff.iter().take(8).map(|h| format!("{},{},{} at {},{},{} {}", h.cell.x, h.cell.y, h.cell.z, h.block.x, h.block.y, h.block.z, h.state.trim_start_matches("minecraft:").split('[').next().unwrap_or(""))).collect::<Vec<_>>().join("; ")) },
        blind.iter().map(|h| h.cell).collect::<std::collections::HashSet<_>>().len()
    )];
    for h in blind.iter().take(show.max(1) * 10) {
        lines.push(format!("  BLIND {},{},{}: the eye at {:.2},{:.2},{:.2} in {} at {},{},{}", h.cell.x, h.cell.y, h.cell.z, h.eye.x, h.eye.y, h.eye.z, h.state, h.block.x, h.block.y, h.block.z));
    }
    (lines, n_suff)
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
    // (ironspec Nov 13 06:00 real, POPULATIONS: a curve's cart positions run along the chord between
    // its two exits, as AbstractMinecart.moveAlongTrack snaps the cart onto the line through the
    // exit points (x + 0.5 + 0.5 dx, z + 0.5 + 0.5 dz); t 0 at the first named exit, 1 at the second)
    let curve = |a: (f64, f64), b: (f64, f64)| Vec3::new(x + 0.5 + 0.5 * (a.0 + (b.0 - a.0) * t), y, z + 0.5 + 0.5 * (a.1 + (b.1 - a.1) * t));
    let (n, s, e, w) = ((0.0, -1.0), (0.0, 1.0), (1.0, 0.0), (-1.0, 0.0));
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
    } else if state.contains("shape=north_east") {
        curve(n, e)
    } else if state.contains("shape=north_west") {
        curve(n, w)
    } else if state.contains("shape=south_east") {
        curve(s, e)
    } else if state.contains("shape=south_west") {
        curve(s, w)
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

/// (G1 Nov 13, N26; ironspec 06:18 platform: the claims follow the run's DECLARED START STATE) The POI
/// tickets the design's villagers hold in that state: every villager entity line's loaded JOB_SITE
/// memory (`"minecraft:job_site"`'s `pos:[I;x,y,z]`, a ticket it holds in either state) and, in
/// state (ii) `taken`, its loaded HOME memory too; in state (i) `free` a memory-only HOME takes no
/// ticket (a free bed makes no village centre: PoiManager's village centre needs an OCCUPIED POI).
/// No bed is guessed (the "nearest bed head" stand-in is dropped). With the lines naming each.
pub fn design_claims(spec: &FarmSpec, taken: bool) -> (Vec<BlockPos>, Vec<String>) {
    let mut out: Vec<BlockPos> = Vec::new();
    let mut lines = Vec::new();
    let memory = |n: &str, key: &str| -> Option<BlockPos> {
        let i = n.find(key)?;
        let rest = &n[i..];
        let at = rest.find("pos:[I;")? + 7;
        let end = rest[at..].find(']')? + at;
        let v: Vec<i32> = rest[at..end].split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
        (v.len() == 3).then(|| BlockPos::new(v[0], v[1], v[2]))
    };
    for (ty, p, nbt) in &spec.entities {
        let n = nbt.as_deref().unwrap_or("");
        if !(ty.ends_with("villager") || n.contains("minecraft:villager\"")) {
            continue;
        }
        let mut held: Vec<(BlockPos, &str)> = Vec::new();
        if let Some(j) = memory(n, "\"minecraft:job_site\"") {
            held.push((j, "its loaded JOB_SITE memory"));
        }
        match memory(n, "\"minecraft:home\"") {
            Some(h) if taken => held.push((h, "its loaded HOME memory, start state (ii): the ticket taken")),
            Some(h) => lines.push(format!("  the villager at {:.2},{:.2},{:.2}: HOME {},{},{} memory-only (start state (i): the ticket free, no claim)", p[0], p[1], p[2], h.x, h.y, h.z)),
            None => {}
        }
        for (c, how) in held {
            if !out.contains(&c) {
                out.push(c);
            }
            lines.push(format!("  the villager at {:.2},{:.2},{:.2} holds {},{},{} ({how})", p[0], p[1], p[2], c.x, c.y, c.z));
        }
    }
    (out, lines)
}

/// (G1 Nov 13, N26) Each claim taken in the world, refusing a cell that holds no POI (`take` would
/// accept it), then the village sections printed.
pub fn village_state(spec: &FarmSpec, claims: &[BlockPos]) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    apply_claims(&mut run.world, claims)
}

fn apply_claims(w: &mut redsim::World, claims: &[BlockPos]) -> Result<Vec<String>, String> {
    for c in claims {
        if redsim::entity::mob::ai::poi::poi_of(&w.get(*c)).is_none() {
            return Err(format!("claim {},{},{}: no POI there ({})", c.x, c.y, c.z, w.get(*c).to_string().trim_start_matches("minecraft:")));
        }
        redsim::entity::mob::ai::poi::take(w, *c);
    }
    if !claims.is_empty() {
        redsim::entity::mob::ai::poi::note_village_centres(w);
    }
    let v = redsim::entity::mob::ai::poi::village_sections(w);
    let mut centres: Vec<(i32, i32, i32)> = v.centres.clone();
    centres.sort();
    centres.dedup();
    Ok(vec![format!("the village state: {} claimed POI(s); village centre sections {}", claims.len(), if centres.is_empty() { "none (no village: both strolls draw without the village bias)".to_string() } else { centres.iter().map(|s| format!("({},{},{})", s.0, s.1, s.2)).collect::<Vec<_>>().join(" ") })])
}

/// (G1 Nov 13, N5: the gate's zombies come from the WORLD) The design's zombie lines (standing, or
/// riding a boat or cart) summoned into the settled world (`Run::start` places blocks only), then
/// each zombie read where the world seats it (`zombies_seated`'s world branch: the vehicle's
/// passenger attachment by `rider_seat`); a design with zombie lines and none in the world fails.
pub fn world_zombies(w: &mut redsim::World, spec: &FarmSpec) -> Result<Vec<Vec3>, String> {
    let lines: Vec<&(String, [f64; 3], Option<String>)> = spec.entities.iter().filter(|(ty, _, nbt)| ty.ends_with("zombie") || nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\""))).collect();
    for (ty, p, nbt) in &lines {
        redsim::entity::spawn(w, ty, *p, nbt.as_deref()).map_err(|e| format!("summoning the design's {ty} at {:.2},{:.2},{:.2}: {e}", p[0], p[1], p[2]))?;
    }
    let world_n = w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Zombie)).count();
    if world_n < lines.len() {
        return Err(format!("the design has {} zombie line(s) but the world holds {world_n} zombie(s) after the summons", lines.len()));
    }
    model_errors(w)?;
    Ok(zombies_seated(w, spec))
}

/// (G1 Nov 13, N39: the controls' fixture) A floor room: stone (y 0) over x -4..6, z -3..3, a
/// north-south rail at x 0 (z -1..1), a standing zombie at (3.5, 1, 0.5); `cage`: a stone pillar
/// top (-2,1,0) whose cell above (-2,2,0) is boxed in glass (a stable PANIC candidate 2 from the
/// rail no path reaches).
pub fn control_room(cage: bool) -> FarmSpec {
    let b = BlockPos::new;
    let p = |s: &str| redsim::block::Block::parse(s).expect("a block");
    let mut blocks = Vec::new();
    for x in -4..=6 {
        for z in -3..=3 {
            blocks.push((b(x, 0, z), p("minecraft:stone")));
        }
    }
    for z in -1..=1 {
        blocks.push((b(0, 1, z), p("minecraft:rail[shape=north_south,waterlogged=false]")));
    }
    blocks.push((b(-2, 1, 0), p("minecraft:stone")));
    if cage {
        for c in [b(-3, 2, 0), b(-1, 2, 0), b(-2, 2, -1), b(-2, 2, 1), b(-2, 3, 0)] {
            blocks.push((c, p("minecraft:glass")));
        }
    }
    FarmSpec {
        blocks,
        entities: vec![("minecraft:zombie".to_string(), [3.5, 1.0, 0.5], Some("{PersistenceRequired:1b}".to_string()))],
        output: b(6, 1, 3),
        lo: b(-6, -1, -5),
        hi: b(8, 6, 5),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    }
}

/// (ironspec Nov 13 06:18 platform (2), G1 N27 / N39) flee-check's OWN CONTROLS, run first in the
/// same run on the same build: each bar's positive control through its own path must bite, else
/// the run fails (no citation without them). The gate (a caged PANIC candidate fails it), the 6-8
/// gate (a zombie in sight 3 away), the suffocation gate (a stone at a rider's eye), the pickup gate
/// (a boated zombie's entity line 0.1 beside a cart), the HOME walk (a boxed-in day cell), the
/// reach calibration (48 / 767) and the blindness rule.
pub fn self_controls() -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    let caged = control_room(true);
    let (_, v) = clear_all(&caged, None, &[], &[], &[], &[], 39.0, &[], false, true, &[], &[], 39.0, &[], &[], &[], false, false, &PlaceClasses::default())?;
    lines.push(format!("  gate: a caged PANIC candidate: {} failing positions of {} ({} candidates judged)", v.gate_fails, v.gate_positions, v.gate_samples));
    if v.gate_fails == 0 || v.gate_samples == 0 {
        bad.push("the gate");
    }
    let places = check(&control_room(false), None, &[], &[])?;
    let (_, fails) = report_classed(&places, &[], 0, false, &PlaceClasses::default());
    let six = fails.iter().any(|f| f.starts_with("the 6-8 gate"));
    lines.push(format!("  6-8 gate: a zombie in sight 3 away: {}", if six { "fails, as it must" } else { "passes  DIFFERS" }));
    if !six {
        bad.push("the 6-8 gate");
    }
    let (n, hits) = eye_check(&control_room(false), true)?;
    let (_, suff) = eye_lines(n, &hits, 0);
    lines.push(format!("  suffocation: a stone at a rider's eye: {suff} places ({n} eye positions)"));
    if suff == 0 {
        bad.push("the suffocation gate");
    }
    let (_, posts, overlaps) = pickup_check(&control_room(false), true);
    lines.push(format!("  pickup: a boated zombie's entity line beside a cart: {overlaps} of {posts} posts overlap"));
    if overlaps == 0 {
        bad.push("the pickup gate");
    }
    let mut bed = control_room(true);
    bed.entities.clear();
    bed.blocks.push((BlockPos::new(4, 1, -2), redsim::block::Block::parse("minecraft:red_bed[facing=south,occupied=false,part=foot]").map_err(|e| e.to_string())?));
    bed.blocks.push((BlockPos::new(4, 1, -1), redsim::block::Block::parse("minecraft:red_bed[facing=south,occupied=false,part=head]").map_err(|e| e.to_string())?));
    let (_, hn) = home_walk(&bed, BlockPos::new(4, 1, -1), &[Vec3::new(-1.5, 2.0, 0.5)])?;
    lines.push(format!("  HOME walk: a boxed-in day cell: {}", if hn == 1 { "unreached, as it must" } else { "reached  DIFFERS" }));
    if hn != 1 {
        bad.push("the HOME walk");
    }
    let rc = reach_control()?;
    let rbad = rc.iter().filter(|r| !r.3).count();
    lines.push(format!("  reach calibration: {} of {} as expected", rc.len() - rbad, rc.len()));
    if rbad > 0 {
        bad.push("the reach calibration");
    }
    let bc = blind_control()?;
    let bbad = bc.iter().filter(|b| !b.2).count();
    lines.push(format!("  blindness rule: {} of {} as expected", bc.len() - bbad, bc.len()));
    if bbad > 0 {
        bad.push("the blindness rule");
    }
    lines.insert(0, format!("flee-check's own controls (run first, this build): {}", if bad.is_empty() { "every one bites (PASS)".to_string() } else { format!("FAIL: {}", bad.join(", ")) }));
    if bad.is_empty() { Ok(lines) } else { Err(lines.join("\n")) }
}

/// The surroundings a P static models (ironspec Nov 13 07:31, G1): FLOATING, the design as stored
/// (air outside its box), or BURIED, every air cell outside the design's box within the flee box of
/// the place (16 + 2 sideways, 7 + 2 up and down) filled with stone, a cave `pocket` (the cell and
/// the one above it) left open as the positive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surround {
    Floating,
    Buried { pocket: Option<BlockPos> },
}

/// `spec` with its surroundings per `s` (ironspec Nov 13 07:31 / 07:47, G1 07:48): FLOATING is the
/// design as stored (void outside its box); BURIED is G1's EXTERIOR FLOOD from the module's shell
/// outward: every AIR cell inside the union of `boxes` (the boxes the statics read: idle strolls,
/// the stop's stroll, the flee, the golem summon's box, each with its stability cell below) that is
/// outside the design's box, or inside it but reached from outside by K11's flood through
/// collision-less cells, becomes stone, except the `pocket` (the cell and the one above it). An
/// open shell (the flood reaching a rider cell in `riders`) fills only outside the design's box,
/// said so. Prints the fill's cell count, its extent and the nearest filled cell per rider cell.
pub fn surround_spec(spec: &FarmSpec, s: Surround, boxes: &[(BlockPos, BlockPos)], riders: &[BlockPos]) -> Result<(FarmSpec, Vec<String>), String> {
    let Surround::Buried { pocket } = s else {
        return Ok((spec.clone(), vec!["the surroundings: FLOATING (floating site: void outside the box, envelope covered: redsim reads AIR outside the world box (World::get -> get_outside, cc767b78 Nov 13 07:50))".into()]));
    };
    let mut run = Run::start(spec, &RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) })?;
    settle_full(&mut run)?;
    let w = &run.world;
    let shell = Shell::new(spec, w, &[]);
    let open_at: Option<BlockPos> = riders.iter().copied().find(|r| shell.in_box(r) && shell.outside(r));
    let in_design = |p: &BlockPos| shell.in_box(p);
    let mut cells: std::collections::HashSet<BlockPos> = std::collections::HashSet::new();
    for (lo, hi) in boxes {
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    cells.insert(BlockPos::new(x, y, z));
                }
            }
        }
    }
    let stone = redsim::block::Block::parse("minecraft:stone").map_err(|e| e.to_string())?;
    let mut fill: Vec<BlockPos> = cells
        .into_iter()
        .filter(|p| !pocket.is_some_and(|q| *p == q || *p == q.above()))
        .filter(|p| w.get(*p).is_air() && (!in_design(p) || (open_at.is_none() && shell.outside(p))))
        .collect();
    fill.sort_by_key(|p| (p.y, p.z, p.x));
    let mut out = spec.clone();
    let (mut lo, mut hi) = (spec.lo, spec.hi);
    for p in &fill {
        out.blocks.retain(|(q, _)| q != p);
        out.blocks.push((*p, stone));
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    out.lo = lo;
    out.hi = hi;
    let ext = fill.iter().fold(None, |a: Option<(BlockPos, BlockPos)>, p| {
        Some(a.map_or((*p, *p), |(l, h)| (BlockPos::new(l.x.min(p.x), l.y.min(p.y), l.z.min(p.z)), BlockPos::new(h.x.max(p.x), h.y.max(p.y), h.z.max(p.z)))))
    });
    let mut lines = vec![format!(
        "the surroundings: BURIED (G1's exterior flood from the shell outward over the union of {} read box(es)): {} cell(s) filled with stone{}{}{}",
        boxes.len(),
        fill.len(),
        ext.map_or(String::new(), |(l, h)| format!(", extent {},{},{}..{},{},{}", l.x, l.y, l.z, h.x, h.y, h.z)),
        pocket.map_or(String::new(), |q| format!(", a cave pocket at {},{},{} and above (the positive)", q.x, q.y, q.z)),
        match open_at {
            Some(r) => format!("; THE SHELL IS OPEN (the outside reaches rider cell {},{},{}): cells inside the design's box not filled", r.x, r.y, r.z),
            None => "; the shell closed: the exterior air inside the design's box filled too".to_string(),
        }
    )];
    // (the nearest filled cell per rider cell; over many rider cells (a whole module's rails) the
    // least one and the count, the per-cell list only for a short list)
    let nearest = |r: &BlockPos| fill.iter().min_by_key(|p| (p.x - r.x).pow(2) + (p.y - r.y).pow(2) + (p.z - r.z).pow(2)).map(|p| (*p, (((p.x - r.x).pow(2) + (p.y - r.y).pow(2) + (p.z - r.z).pow(2)) as f64).sqrt()));
    if riders.len() > 12 {
        let best = riders.iter().filter_map(|r| nearest(r).map(|(p, d)| (*r, p, d))).min_by(|a, b| a.2.total_cmp(&b.2));
        lines.push(format!(
            "  the nearest filled cell over {} rider cell(s): {}",
            riders.len(),
            best.map_or("none".to_string(), |(r, p, d)| format!("{},{},{} at {d:.2} from rider cell {},{},{}", p.x, p.y, p.z, r.x, r.y, r.z))
        ));
    } else {
        for r in riders {
            lines.push(format!(
                "  the nearest filled cell to rider cell {},{},{}: {}",
                r.x,
                r.y,
                r.z,
                nearest(r).map_or("none".to_string(), |(p, d)| format!("{},{},{} at {d:.2}", p.x, p.y, p.z))
            ));
        }
    }
    Ok((out, lines))
}

/// The boxes a P static reads around `p` (ironspec 07:47): the flee's (16 + 1 sideways, P.y - 8 ..
/// P.y + 7) or the stop's stroll (2 + 1, P.y - 3 .. P.y + 2), the idle stroll's (10 + 1, y - 8 ..
/// y + 7) and the golem summon's (8 + 1, y - 7 .. y + 6).
pub fn read_boxes(p: BlockPos, flee: bool) -> Vec<(BlockPos, BlockPos)> {
    let bx = |h: i32, dn: i32, up: i32| (BlockPos::new(p.x - h, p.y - dn, p.z - h), BlockPos::new(p.x + h, p.y + up, p.z + h));
    let mut v = vec![bx(11, 8, 7), bx(9, 7, 6)];
    v.push(if flee { bx(17, 8, 7) } else { bx(3, 3, 2) });
    v
}

/// The world a P static reads (G1 on c433eb107, ironspec 07:34): the design (its surroundings per
/// `surround`) settled; each stand-in through the settled-world override rule (before -> after; a
/// no-op FAILS); the stand-ins laid, then the world RE-SETTLED (a changed hatch updates its
/// neighbours: the cells the re-settle changed printed); each stand-in's survival (one the settle
/// changes FAILS unless held, then re-placed raw); the claims applied. Returns the run, the lines
/// and the failures.
fn p_world(spec: &FarmSpec, sets: &[(BlockPos, redsim::block::Block)], holds: &[(BlockPos, String)], claims: &[BlockPos]) -> Result<(Run, Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let mut lines = Vec::new();
    let mut fails = override_report(&run.world, sets, holds, &mut lines);
    if !sets.is_empty() {
        let mut before: Vec<(BlockPos, redsim::block::Block)> = Vec::new();
        for x in spec.lo.x..=spec.hi.x {
            for y in spec.lo.y..=spec.hi.y {
                for z in spec.lo.z..=spec.hi.z {
                    let p = BlockPos::new(x, y, z);
                    let b = sets.iter().find(|(q, _)| *q == p).map_or_else(|| run.world.get(p), |(_, b)| *b);
                    before.push((p, b));
                }
            }
        }
        run.world.place_raw(sets);
        settle_full(&mut run)?;
        let changed: Vec<String> = before
            .iter()
            .filter(|(p, b)| run.world.get(*p) != *b && !sets.iter().any(|(q, _)| q == p))
            .map(|(p, b)| format!("{},{},{} {} -> {}", p.x, p.y, p.z, b.to_string().trim_start_matches("minecraft:"), run.world.get(*p).to_string().trim_start_matches("minecraft:")))
            .collect();
        lines.push(format!("the re-settle after the lay: {} other cell(s) changed{}", changed.len(), if changed.is_empty() { String::new() } else { format!(": {}", changed.join("; ")) }));
        fails += standin_survival(&mut run.world, sets, holds, &mut lines);
    }
    apply_claims(&mut run.world, claims)?;
    Ok((run, lines, fails))
}

/// A villager seated in a cart at `place` (`exact`, or the rail's middle), its index; the seat's
/// on_ground false (G1 N28).
fn seat_rider(w: &mut redsim::World, place: BlockPos, exact: Option<Vec3>) -> Result<usize, String> {
    let state = w.get(place).to_string();
    let p0 = exact.unwrap_or_else(|| along_rail(place, &state, 0.5));
    let cart = redsim::entity::spawn(w, "minecraft:minecart", [p0.x, p0.y, p0.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let ci = redsim::entity::index_of(w, cart).ok_or("the cart did not spawn")?;
    let vid = *w.entities.list[ci].passengers.first().ok_or("the cart has no rider")?;
    let i = redsim::entity::index_of(w, vid).ok_or("the rider did not spawn")?;
    w.entities.list[ci].pos = p0;
    redsim::entity::position_rider(w, ci, i);
    w.entities.list[i].on_ground = false;
    redsim::entity::fluid_push::update_fluid_interaction(w, i);
    Ok(i)
}

/// The candidate rows of a P static: each cell with its share, its reach from the rider, its
/// floor's solid-render status, whether it lies OUTSIDE the design's box, and EVERY draw that lands
/// on it (ironspec 07:36: stability is tested at the DRAWN cell, then moveUpOutOfSolid lifts it with
/// no second test: a draw AT the cell, or into a solid run under it whose own floor renders solid,
/// within the draw's `v` of the rider's y). Returns the rows and the number outside the box.
fn candidate_rows(w: &mut redsim::World, i: usize, shares: &[(BlockPos, f64)], v: i32, box_lo: BlockPos, box_hi: BlockPos, label: &str) -> (Vec<String>, usize, usize) {
    use redsim::entity::mob::ai::goal::stroll;
    let me_y = w.entities.list[i].pos.y;
    let mut rows = Vec::new();
    let mut outside = 0usize;
    let mut unreached = 0usize;
    for (c, sh) in shares {
        let reached = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_pos(w, nm, *c, 0, arena).is_some_and(|p| p.reached)
        });
        unreached += usize::from(!reached);
        let below = w.get(c.below());
        let out = !(c.x >= box_lo.x && c.x <= box_hi.x && c.y >= box_lo.y && c.y <= box_hi.y && c.z >= box_lo.z && c.z <= box_hi.z);
        outside += usize::from(out);
        let mut landings = Vec::new();
        for k in 0..=(2 * v + 1) {
            let q = BlockPos::new(c.x, c.y - k, c.z);
            if (q.y as f64) < (me_y - v as f64 - 1.0).floor() {
                break;
            }
            if stroll::is_stable_destination(w, q) && stroll::move_up_out_of_solid(w, i, q) == Some(*c) {
                landings.push(if k == 0 { "drawn at it".to_string() } else { format!("drawn at {},{},{} (its floor {} solid-render) and moved up {k}", q.x, q.y, q.z, w.get(q.below()).to_string().trim_start_matches("minecraft:")) });
            }
        }
        rows.push(format!(
            "  {label} {},{},{}: {:.1}% an event, {}; its floor {} ({}){}; landings: {}",
            c.x,
            c.y,
            c.z,
            sh * 100.0,
            if reached { "a complete path reaches it" } else { "UNREACHED" },
            below.to_string().trim_start_matches("minecraft:"),
            if stroll::is_solid_render(&below) { "solid-render" } else { "not solid-render" },
            if out { "; OUTSIDE the design's box" } else { "" },
            if landings.is_empty() { "none found in the column".to_string() } else { landings.join(", ") }
        ));
    }
    (rows, outside, unreached)
}

/// A column's cells under `top` (ironspec 07:36: E's column (-2,-1..-8,2)), each with isSolid and
/// isSolidRender: the move-up path a draw can take onto `top`; and whether `top` itself passes
/// LandRandomPos's last filters for rider `i` (not water, no path-type malus: G1 07:38, E types
/// RAIL).
pub fn column_print(w: &redsim::World, i: usize, top: BlockPos, depth: i32) -> String {
    let cells: Vec<String> = (1..=depth)
        .map(|k| {
            let q = BlockPos::new(top.x, top.y - k, top.z);
            let b = w.get(q);
            format!(
                "{},{},{} {} (isSolid {}, isSolidRender {})",
                q.x,
                q.y,
                q.z,
                b.to_string().trim_start_matches("minecraft:"),
                redsim::behavior::is_solid(&b),
                redsim::entity::mob::ai::goal::stroll::is_solid_render(&b)
            )
        })
        .collect();
    let passes = redsim::entity::mob::ai::goal::stroll::move_up_out_of_solid(w, i, top) == Some(top);
    format!(
        "the column under {},{},{} ({}; {}): {}",
        top.x,
        top.y,
        top.z,
        w.get(top).to_string().trim_start_matches("minecraft:"),
        if passes { "the cell passes the water and malus filters: a landing there is KEPT" } else { "the cell fails the water or malus filter (or is solid): no landing kept there" },
        cells.join("; ")
    )
}

/// (ironspec Nov 13 07:11 / 07:16 / 07:36, G1: P's stroll static) The PANIC stroll's candidates
/// (VillageBoundRandomStroll 2/2: in a village a land draw around, else DefaultRandomPos toward the
/// section closest to the village within 2, or a land draw when that section is its own) at a
/// rider place, on [`p_world`]'s world (stand-ins through the override rule, re-settled): each
/// candidate with its share, reach, floor, box and landings. The bar is NONE: any candidate FAILS
/// (an arming point at a stop). `exact`: the rider's own position. `complete_ok` (G1 08:20, the
/// per-place bar): a boarded-exit place's bar is NONE OR COMPLETE (a seated rider's complete path
/// clears the stamp: only an unreached candidate fails); P's is NONE.
#[allow(clippy::too_many_arguments)]
pub fn stroll_candidates(spec: &FarmSpec, claims: &[BlockPos], place: BlockPos, exact: Option<Vec3>, sets: &[(BlockPos, redsim::block::Block)], holds: &[(BlockPos, String)], columns: &[BlockPos], box_of: (BlockPos, BlockPos), complete_ok: bool) -> Result<(Vec<String>, usize, usize), String> {
    use redsim::entity::mob::ai::goal::stroll;
    let (mut run, mut lines, fails) = p_world(spec, sets, holds, claims)?;
    let w = &mut run.world;
    let i = seat_rider(w, place, exact)?;
    let me = w.entities.list[i].pos;
    let me_b = w.entities.list[i].block_position();
    let vil = redsim::entity::mob::ai::poi::is_village(w, me_b);
    let s = (me_b.x.div_euclid(16), me_b.y.div_euclid(16), me_b.z.div_euclid(16));
    let best = if vil { s } else { redsim::entity::mob::ai::poi::section_closest_to_village(w, s, 2) };
    let shares = if best != s {
        let c = ((best.0 * 16 + 8) as f64 + 0.5, (best.2 * 16 + 8) as f64 + 0.5);
        stroll::pick_shares(w, i, 2.0, 2, Some((c.0 - me.x, c.1 - me.z)), false, 10)
    } else {
        stroll::pick_shares(w, i, 2.0, 2, None, true, 10)
    };
    let (rows, outside, unreached) = candidate_rows(w, i, &shares, 2, box_of.0, box_of.1, "CANDIDATE");
    for c in columns {
        lines.push(column_print(w, i, *c, 8));
    }
    model_errors(w)?;
    let failing = if complete_ok { unreached } else { shares.len() };
    lines.push(format!(
        "the PANIC stroll's candidates at {},{},{} (the rider at {:.3},{:.4},{:.3}; VillageBoundRandomStroll 2/2, priority 3 in the panic package; {}; the bar: {}): {} candidate cell(s), {unreached} unreached, {outside} outside the design's box{}",
        place.x,
        place.y,
        place.z,
        me.x,
        me.y,
        me.z,
        if best != s { "toward the village section" } else { "a land draw around" },
        if complete_ok { "NONE OR COMPLETE (a boarded-exit place: an unreached candidate fails)" } else { "NONE (P: a reachable candidate walks the napper off P)" },
        shares.len(),
        if failing == 0 && fails == 0 { " (PASS)" } else { " (FAIL)" }
    ));
    lines.extend(rows);
    Ok((lines, failing + fails, outside))
}

/// (ironspec Nov 13 07:27 / 07:31 / 07:36, G1's three terms: P's FLEE candidates) The flee's
/// targets at a rider place from a hostile's feet (SetWalkTargetAwayFrom: up to 10 calls of
/// LandRandomPos.getPosAway 16 / 7, each 10 draws: the WHOLE half-box away from the hostile; each
/// draw stable at the DRAWN cell, then moved up out of solid, then not water, no malus), on
/// [`p_world`]'s world (stand-ins through the override rule, re-settled): each candidate with its
/// share, reach, floor, box and landings. The bar is NONE (a reachable candidate fails too: the
/// napper would walk off P); the hostile's distance and clip are printed (a flee outside 6 or out of
/// sight never runs: printed, the candidates still listed).
#[allow(clippy::too_many_arguments)]
pub fn flee_candidates_at(spec: &FarmSpec, place: BlockPos, exact: Option<Vec3>, hostile: Vec3, hostile_eye: f64, sets: &[(BlockPos, redsim::block::Block)], holds: &[(BlockPos, String)], columns: &[BlockPos], box_of: (BlockPos, BlockPos), complete_ok: bool) -> Result<(Vec<String>, usize, usize), String> {
    let (mut run, mut lines, fails) = p_world(spec, sets, holds, &[])?;
    let w = &mut run.world;
    let i = seat_rider(w, place, exact)?;
    let me = w.entities.list[i].pos;
    let d = ((me.x - hostile.x).powi(2) + (me.y - hostile.y).powi(2) + (me.z - hostile.z).powi(2)).sqrt();
    let eye = Vec3::new(me.x, me.y + veye(), me.z);
    let heye = Vec3::new(hostile.x, hostile.y + hostile_eye, hostile.z);
    let seen = redsim::entity::mob::los::has_line_of_sight(w, eye, heye);
    let mut shares = redsim::entity::mob::ai::goal::stroll::flee_pick_shares(w, i, 16.0, 7, hostile);
    for c in redsim::entity::mob::ai::goal::stroll::flee_candidates(w, i, 16.0, 7, hostile) {
        if !shares.iter().any(|(q, _)| *q == c) {
            shares.push((c, 0.0));
        }
    }
    let (rows, outside, unreached) = candidate_rows(w, i, &shares, 7, box_of.0, box_of.1, "FLEE CANDIDATE");
    for c in columns {
        lines.push(column_print(w, i, *c, 8));
    }
    model_errors(w)?;
    // (ironspec Nov 13 13:50 (a): SetWalkTargetAwayFrom runs only with the hostile inside its 6 and
    // seen (the sensor's NEAREST_HOSTILE): elsewhere the candidates are printed and count nothing)
    let runs = d < 6.0 && seen;
    let failing = if !runs {
        0
    } else if complete_ok {
        unreached
    } else {
        shares.len()
    };
    lines.push(format!(
        "the FLEE's candidates at {},{},{} (the rider at {:.3},{:.4},{:.3}; SetWalkTargetAwayFrom: getPosAway 16 / 7; the bar: {}) from the hostile's feet {:.2},{:.3},{:.2}: {d:.2} away ({}), the clip {}; {} candidate(s), {unreached} unreached, {outside} outside the design's box{}",
        place.x,
        place.y,
        place.z,
        me.x,
        me.y,
        me.z,
        if complete_ok { "NONE OR COMPLETE (a boarded-exit place)" } else { "NONE (P)" },
        hostile.x,
        hostile.y,
        hostile.z,
        if d < 6.0 { "inside the flee's 6" } else { "beyond the flee's 6: the flee never runs" },
        if seen { "CLEAR" } else { "BLOCKED: the flee never runs" },
        shares.len(),
        if failing == 0 && fails == 0 {
            if runs { " (PASS)" } else { " (PASS: the flee never runs here, its candidates printed for the record)" }
        } else {
            " (FAIL)"
        }
    ));
    lines.extend(rows);
    Ok((lines, failing + fails, outside))
}

/// [`check_claims`], only the rider place at `only` (the others skipped: the fix search's runs).
fn check_inner(spec: &FarmSpec, home: Option<BlockPos>, foot: &[BlockPos], panic_boxes: &[([i32; 3], [i32; 3])], continuous: bool, claims: &[BlockPos], pills: &[Vec3], only: Option<BlockPos>, at: &[Vec3]) -> Result<Vec<Place>, String> {
    let continuous_off = !continuous;
    use redsim::entity::mob::ai::goal::stroll;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &mut run.world;
    apply_claims(w, claims)?;
    let foes = world_zombies(w, spec)?;
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
            // (ironspec Nov 13 06:00 real, POPULATIONS: a failed spawn is an error, never a
            // skipped place: a skip shifts every later result)
            let ci = redsim::entity::index_of(w, cart).ok_or_else(|| format!("flee-check: the cart at {},{},{} did not spawn", cell.x, cell.y, cell.z))?;
            let vid = *w.entities.list[ci].passengers.first().ok_or_else(|| format!("flee-check: the cart at {},{},{} has no rider", cell.x, cell.y, cell.z))?;
            (vid, Some(cart))
        };
        let i = redsim::entity::index_of(w, vid).ok_or_else(|| format!("flee-check: the villager at {},{},{} did not spawn", cell.x, cell.y, cell.z))?;
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
                    // (tick-exact Nov 13 (2), G1 N28: a SEATED rider's onGround is false every tick
                    // in the game, so its path starts at the scan-down cell, never floor(y + 0.5))
                    w.entities.list[i].on_ground = false;
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
        let mut near_seen: Option<f64> = None;
        let mut near_pill: Option<f64> = None;
        for &p in &points {
            let me = put(w, p);
            let eye = Vec3::new(me.x, me.y + veye(), me.z);
            let dist = |f: &Vec3| ((me.x - f.x).powi(2) + (me.y - f.y).powi(2) + (me.z - f.z).powi(2)).sqrt();
            if let Some(d) = pills.iter().map(dist).min_by(f64::total_cmp) {
                near_pill = Some(near_pill.map_or(d, |s: f64| s.min(d)));
            }
            for f in foes.iter().filter(|f| dist(f) < FLEE_BAR) {
                let zeye = Vec3::new(f.x, f.y + zeye(), f.z);
                let hidden = !redsim::entity::mob::los::has_line_of_sight(w, eye, zeye)
                    && ray_cells(eye, zeye).into_iter().any(|q| {
                        let b = w.get(q);
                        !b.is_air() && redsim::behavior::collision_full_block_at(w, &b, q)
                    });
                if !hidden {
                    near_seen = Some(near_seen.map_or(dist(f), |s: f64| s.min(dist(f))));
                }
            }
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
            panic_ok &= foes.iter().any(|f| dist(f) <= PANIC_BAR && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + zeye(), f.z)))
                || pills.iter().any(|f| dist(f) <= PILLAGER_BAR - 0.05 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z)));
            // (tick-exact Nov 13: NEAREST_HOSTILE needs line of sight (VillagerHostilesSensor over
            // NEAREST_VISIBLE_LIVING_ENTITIES, TargetingConditions' checkLineOfSight: eye to eye,
            // COLLIDER, Fluid NONE): the flee counts only hostiles within 6 that the rider SEES (G1
            // N39: an unseen hostile inside 6 doesn't panic); SetWalkTargetAwayFrom's own range 6)
            let near6: Vec<Vec3> = foes
                .iter()
                .filter(|f| dist(f) < 6.0 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + zeye(), f.z)))
                .chain(pills.iter().filter(|f| dist(f) < 6.0 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + PILLAGER_EYE, f.z))))
                .copied()
                .collect();
            let near8: Vec<Vec3> = foes.iter().copied().filter(|f| dist(f) <= 8.0).collect();
            let near15: Vec<Vec3> = pills.iter().copied().filter(|f| dist(f) <= PILLAGER_BAR).collect();
            flee_from = flee_from.max(near6.len());
            panic_from = panic_from.max(near8.len() + near15.len());
            seen = seen.max(
                near8.iter().filter(|f| redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + zeye(), f.z))).count()
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
                && redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(me.x, me.y + veye(), me.z), Vec3::new(q.x, q.y + veye(), q.z))
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
                // (G1 Nov 13, N8: a place inside the flee bar only between the sampled points gets
                // its sight test at the segment's nearest point too, with the same robust hiding)
                if d < FLEE_BAR {
                    let eye = Vec3::new(q.x, q.y + veye(), q.z);
                    let ze = Vec3::new(f.x, f.y + zeye(), f.z);
                    let hidden = !redsim::entity::mob::los::has_line_of_sight(w, eye, ze)
                        && ray_cells(eye, ze).into_iter().any(|c| {
                            let b = w.get(c);
                            !b.is_air() && redsim::behavior::collision_full_block_at(w, &b, c)
                        });
                    if !hidden {
                        near_seen = Some(near_seen.map_or(d, |s: f64| s.min(d)));
                    }
                }
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
        out.push(Place { cell, on_foot, flee_from, panic_from, seen, village, flee, panic_stroll, stroll: stroll_src, others, home_reached, near, worst, panic_declared, panic_ok, near_seen, near_pill });
        let _ = redsim::entity::kill(w, vid);
        if let Some(c) = cart {
            let _ = redsim::entity::kill(w, c);
        }
        w.entities.list.retain(|e| !e.removed);
    }
    Ok(out)
}

/// (ironspec Nov 13 06:36 real (3), G1's terms 06:40: the HOME walk is a BAR) From every day cell a
/// stranded napper occupies (the cells come from the runs' traces, never a hand list), a villager
/// standing there paths to the bed head with the call AcquirePoi makes for HOME: createPath(Set, 1)
/// (`create_path_positions`: no surface search, the target the head's own cell) with reach = HOME's
/// valid range 1, under the villager's 48 / 767 caps, on the
/// SETTLED world (N5). Per cell: the end node, its distance to the head, `reached`. FAIL at every
/// cell whose path doesn't reach; 0 cells is NOT JUDGED (usize::MAX).
pub fn home_walk(spec: &FarmSpec, head: BlockPos, feet: &[Vec3]) -> Result<(Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &mut run.world;
    let mut rows = Vec::new();
    let mut fails = 0usize;
    for f in feet {
        let c = BlockPos::new(f.x.floor() as i32, (f.y + 1e-6).floor() as i32, f.z.floor() as i32);
        let vid = redsim::entity::spawn(w, "minecraft:villager", [f.x, f.y, f.z], Some("{PersistenceRequired:1b}"))?;
        let i = redsim::entity::index_of(w, vid).ok_or_else(|| format!("home walk: the villager at {:.3},{:.4},{:.3} did not spawn", f.x, f.y, f.z))?;
        // (standing on the floor: `canUpdatePath` needs it on the ground)
        w.entities.list[i].on_ground = true;
        redsim::entity::fluid_push::update_fluid_interaction(w, i);
        // (AcquirePoi's call: `findPath(Set<BlockPos>, validRange)` = createPath(Set, reach), with
        // NO surface search: the target stays the bed head's own cell; `create_path_pos` would run
        // GroundPathNavigation's surface search and lift it onto the bed)
        let path = redsim::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| {
            let mut copy = nav.clone();
            copy.create_path_positions(w, nm, &[head], 1, arena)
        });
        let (end, dist, reached) = match &path {
            Some(p) => {
                let e = p.end_node().map(|n| n.pos());
                (e, e.map(|e| (((e.x - head.x).pow(2) + (e.y - head.y).pow(2) + (e.z - head.z).pow(2)) as f64).sqrt()), p.reached)
            }
            None => (None, None, false),
        };
        if !reached {
            fails += 1;
        }
        rows.push(format!(
            "  {} {},{},{} (feet {:.3},{:.4},{:.3}): start node {}, end node {}, {} from the head, reached {}",
            if reached { "REACHED" } else { "UNREACHED" },
            c.x,
            c.y,
            c.z,
            f.x,
            f.y,
            f.z,
            path.as_ref().and_then(|p| p.nodes.first().map(|n| format!("{},{},{}", n.x, n.y, n.z))).unwrap_or_else(|| "-".to_string()),
            end.map_or("none (no path)".to_string(), |e| format!("{},{},{}", e.x, e.y, e.z)),
            dist.map_or("-".to_string(), |d| format!("{d:.2}")),
            reached
        ));
        let _ = redsim::entity::kill(w, vid);
        w.entities.list.retain(|e| !e.removed);
    }
    model_errors(w)?;
    let mut lines = vec![format!(
        "the HOME walk (a bar, ironspec Nov 13 06:36 real: from every stranded napper's day cell a complete path to the bed head {},{},{} within HOME's valid range 1, AcquirePoi's createPath(Set, 1) (no surface search) under the 48 / 767 caps, on the settled world): {} day cells, {} unreached{}",
        head.x,
        head.y,
        head.z,
        feet.len(),
        fails,
        if feet.is_empty() { " (NOT JUDGED: 0 samples)" } else if fails == 0 { " (PASS)" } else { " (FAIL)" }
    )];
    lines.extend(rows);
    Ok((lines, if feet.is_empty() { usize::MAX } else { fails }))
}

/// The `KEY: x,y,z+x,y,z...=REASON; ...` entries of a design text's lines (bare `KEY:` meta lines
/// reach here as `# KEY:`; `#`-header lines are read as they stand), each cell with its reason.
fn parse_cell_entries(text: &str, key: &str) -> Result<Vec<(BlockPos, String)>, String> {
    let mut out = Vec::new();
    for e in crate::pushscan::header_entries(text, &format!("{key}:")) {
        let (cs, why) = e.split_once('=').ok_or_else(|| format!("{key} {e}: x,y,z+...=REASON"))?;
        for c in cs.split('+') {
            let n: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|err| format!("{key} {e}: {err}"))?;
            if n.len() != 3 {
                return Err(format!("{key} {e}: x,y,z+...=REASON"));
            }
            out.push((BlockPos::new(n[0], n[1], n[2]), why.trim().to_string()));
        }
    }
    Ok(out)
}

/// (ironspec Nov 12 21:49 (4)) The design's place classes: `riderless:` cells (an empty cart's
/// path: no rider there, dropped from the rider places and printed as such) and `waker-places:`
/// (a declared design exception, the woken napper's places: printed apart with their reason and
/// its evidence commit, not failing). Each entry `x,y,z+x,y,z...=REASON`, entries `;`-separated.
#[derive(Default, Clone, Debug)]
pub struct PlaceClasses {
    pub riderless: Vec<(BlockPos, String)>,
    pub waker: Vec<(BlockPos, String)>,
    /// (ironspec Nov 13 08:20 (a) / 09:01 (c), G1 08:23) `late-exempt:` draw cells whose late
    /// pairs are a declared exemption (the descent seat: a napper-only position whose stamp
    /// SleepInBed.start erases at (A)'s lie-down), printed as information with G1's three
    /// conditions, which the composed runs judge.
    pub late_exempt: Vec<(BlockPos, String)>,
    /// (G1 Nov 13 09:38 (a)) `switches:` the record's declared switched Ts (each T with its lever
    /// or line): when given, the walk's switched set is exactly these; an inferred T that isn't
    /// declared is printed.
    pub switches: Vec<(BlockPos, String)>,
    /// (7b90c3f1 / day-1 Nov 13 10:37-10:41) `update-only:` sources whose updates only re-settle a
    /// switched T to its own power's shape (a detector, a feed dust): printed with the Ts they
    /// reach; information (the T itself must be declared in `switches:`).
    pub update_only: Vec<(BlockPos, String)>,
    /// (ironspec Nov 13 10:54) `integrity:` entries, `X,Y,Z=connections A+B; holds T=shape`: a
    /// rail whose two connections are design-critical because they keep a T's arm from joining
    /// (P holding (A)'s K north_east); t-exposure cross-checks them against the settled world.
    pub integrity: Vec<(BlockPos, String)>,
}

impl PlaceClasses {
    pub fn from_meta(meta: &std::collections::BTreeMap<String, String>) -> Result<PlaceClasses, String> {
        let parse = |key: &str| -> Result<Vec<(BlockPos, String)>, String> {
            let Some(v) = meta.get(key) else { return Ok(Vec::new()) };
            parse_cell_entries(&format!("# {key}: {v}"), key)
        };
        Ok(PlaceClasses { riderless: parse("riderless")?, waker: parse("waker-places")?, late_exempt: parse("late-exempt")?, switches: parse("switches")?, update_only: parse("update-only")?, integrity: parse("integrity")? })
    }

    /// (7b90c3f1 Nov 13 10:37) The classes from the meta lines AND the design text's `# switches:`
    /// / `# update-only:` header lines (the lift file carries them there): each entry
    /// `x,y,z=REASON`, x,y,z the T RAIL's cell for `switches:`, the SOURCE's cell for `update-only:`.
    pub fn from_design(meta: &std::collections::BTreeMap<String, String>, text: &str) -> Result<PlaceClasses, String> {
        let mut c = PlaceClasses::from_meta(meta)?;
        for (p, why) in parse_cell_entries(text, "switches")? {
            if !c.switches.iter().any(|(q, _)| *q == p) {
                c.switches.push((p, why));
            }
        }
        for (p, why) in parse_cell_entries(text, "update-only")? {
            if !c.update_only.iter().any(|(q, _)| *q == p) {
                c.update_only.push((p, why));
            }
        }
        for (p, why) in parse_cell_entries(text, "integrity")? {
            if !c.integrity.iter().any(|(q, _)| *q == p) {
                c.integrity.push((p, why));
            }
        }
        Ok(c)
    }

    fn riderless(&self, c: &BlockPos) -> bool {
        self.riderless.iter().any(|(p, _)| p == c)
    }

    fn waker(&self, c: &BlockPos) -> Option<&str> {
        self.waker.iter().find(|(p, _)| p == c).map(|(_, w)| w.as_str())
    }

    fn late_exempt(&self, c: &BlockPos) -> Option<&str> {
        self.late_exempt.iter().find(|(p, _)| p == c).map(|(_, w)| w.as_str())
    }
}

/// `mctas mobfarm flee-check <design>`: the report, `show` cells a place; the verdicts last.
pub fn report(places: &[Place], jobs: &[(BlockPos, String)], show: usize) -> Vec<String> {
    report_with(places, jobs, show, false)
}

/// [`report`], with `clearing`: every place's clearing line (what can clear a stamp there); the
/// failing bars appended as one line (the CLI's exit reads [`report_classed`]'s list).
pub fn report_with(places: &[Place], jobs: &[(BlockPos, String)], show: usize, clearing: bool) -> Vec<String> {
    let (mut lines, fails) = report_classed(places, jobs, show, clearing, &PlaceClasses::default());
    lines.push(format!("failing bars: {}", if fails.is_empty() { "none".to_string() } else { fails.join("; ") }));
    lines
}

/// [`report_with`], the 6-8 rule read with the design's place classes (`riderless:` dropped,
/// `waker-places:` apart).
pub fn report_classed(places: &[Place], jobs: &[(BlockPos, String)], show: usize, clearing: bool, classes: &PlaceClasses) -> (Vec<String>, Vec<String>) {
    let mut lines = clearing_lines(places, clearing);
    let (body, fails) = report_body(places, jobs, show, classes);
    lines.extend(body);
    (lines, fails)
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

/// The villager's and the zombie's eye heights, from the shared `eye_of` (ironspec Nov 13 06:00
/// real, RANGES: never hard-coded).
fn veye() -> f64 {
    eye_of(redsim::entity::mob::types::MobType::Villager)
}

fn zeye() -> f64 {
    eye_of(redsim::entity::mob::types::MobType::Zombie)
}

/// (ironspec Nov 13 06:00 real, POPULATIONS: the gate's hostiles from the design's own entity
/// lines) The feet of every pillager the design seats in a boat (redsim spawns no pillager, so the
/// design's boats are read here: the boat's y + PILLAGER_SEAT), and every hostile line of a type the
/// sensor lists but redsim doesn't model (a husk, a drowned, a zombie villager, a vindicator ...,
/// free or in a boat or raft, and a pillager on a raft), which no run may skip.
pub fn design_hostiles(spec: &FarmSpec) -> (Vec<Vec3>, Vec<String>) {
    let mut pills = Vec::new();
    let mut unjudged = Vec::new();
    for (k, p, nbt) in &spec.entities {
        let n = nbt.as_deref().unwrap_or("");
        let rider = (k.ends_with("_boat") || k.ends_with("_raft")).then(|| {
            let at = n.find("id:\"").map(|i| i + 4)?;
            let end = n[at..].find('"')? + at;
            Some(n[at..end].to_string())
        });
        match rider.flatten() {
            Some(id) if hostile_range(&id).is_some() => {
                let short = id.trim_start_matches("minecraft:");
                if short == "pillager" && k.ends_with("_boat") {
                    pills.push(Vec3::new(p[0], p[1] + PILLAGER_SEAT, p[2]));
                } else if short != "zombie" || !k.ends_with("_boat") {
                    unjudged.push(format!("the {short} in the {} at {:.2},{:.2},{:.2}", k.trim_start_matches("minecraft:"), p[0], p[1], p[2]));
                }
            }
            _ => {
                let short = k.trim_start_matches("minecraft:");
                if hostile_range(k).is_some() && short != "zombie" && short != "pillager" {
                    unjudged.push(format!("the {short} at {:.2},{:.2},{:.2}", p[0], p[1], p[2]));
                }
            }
        }
    }
    (pills, unjudged)
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
        // (G1 Nov 13, N4: ONE cart-path model for every shape, curves on their chord)
        for i in 0..=20 {
            v.push((lab.clone(), along_rail(*c, &s, i as f64 / 20.0), 0.05));
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
/// (9fc3155f, ironspec Nov 12 20:10 (2a): the stand-in guard's 7th property, SIGHT) Every sight ray
/// the villagers' sensors cast (`LivingEntity.hasLineOfSight`: a COLLIDER clip, eye to eye) between
/// a rider and a scarer within 16 (NEAREST_VISIBLE_LIVING_ENTITIES' range; the hostile sensor then
/// keeps a zombie within 8, a pillager within 15) and between two riders within 16 (each rail
/// cell's midpoint), cast in the module and in the module with the stand-ins laid (`sets`): a ray
/// clear in one and blocked in the other is a stand-in that changes sight, listed under each
/// stand-in cell it crosses. The rider points: every rail cell at 0.1, 0.5 and 0.9 along it, the
/// eye 1.62 over the feet; the scarers: the design's boats with a zombie or a pillager seated, and
/// `extra` boats (x, y, z, is a pillager).
/// (ironspec Nov 12 23:30: one shared eye height, per type, from 26.3's EntityType builders)
/// A mob type's eye height above its feet: redsim's entity data (`TypeDef.eye`: `eyeHeight(..)`
/// in `EntityTypes`, else 0.85 x the height), the one function the sight statics and the judges
/// call.
pub fn eye_of(t: redsim::entity::mob::types::MobType) -> f64 {
    t.def().eye as f64
}

#[cfg(test)]
mod eye_tests {
    use redsim::entity::mob::types::MobType;

    fn spec_of(cells: &[(redsim::pos::BlockPos, &str)]) -> redsim::entity::mob::farm::FarmSpec {
        let blocks = cells.iter().map(|(p, b)| (*p, redsim::block::Block::parse(b).unwrap())).collect();
        redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: redsim::pos::BlockPos::new(3, 3, 3), lo: redsim::pos::BlockPos::new(-3, -1, -3), hi: redsim::pos::BlockPos::new(3, 4, 3), block_entities: Vec::new(), rock: false, nether: None, land: false }
    }

    /// (ironspec Nov 13 04:34) The shell rule: a rail inside a closed glass box has no opening
    /// (PASS); a lever in a wall cell instead of glass is an opening (FAIL, the positive control),
    /// as is an air cell; a wall cell of slabs is listed as a partial collider.
    #[test]
    fn shell_openings_lever_and_air() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let mut cells: Vec<(redsim::pos::BlockPos, &str)> = vec![(b(0, 0, 0), "minecraft:stone"), (b(0, 1, 0), "minecraft:rail[shape=north_south,waterlogged=false]")];
        for x in -1i32..=1 {
            for z in -1i32..=1 {
                for y in 0i32..=3 {
                    if (x != 0 || z != 0) || y == 3 {
                        cells.push((b(x, y, z), "minecraft:glass"));
                    }
                }
            }
        }
        let (l, n) = super::shell_openings(&spec_of(&cells), &[]).unwrap();
        assert!(n == 0 && l[0].ends_with("(PASS)"), "{l:?}");
        let lever: Vec<(redsim::pos::BlockPos, &str)> = cells.iter().map(|(p, s)| if *p == b(1, 1, 0) { (*p, "minecraft:lever[face=floor,facing=north,powered=false]") } else { (*p, *s) }).collect();
        let (l2, n2) = super::shell_openings(&spec_of(&lever), &[]).unwrap();
        assert!(n2 == 1 && l2.iter().any(|x| x.starts_with("  OPENING 1,1,0 lever")), "{l2:?}");
        let slab: Vec<(redsim::pos::BlockPos, &str)> = cells.iter().map(|(p, s)| if *p == b(1, 2, 0) { (*p, "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]") } else { (*p, *s) }).collect();
        let (l3, n3) = super::shell_openings(&spec_of(&slab), &[]).unwrap();
        assert!(n3 == 0 && l3.iter().any(|x| x.starts_with("  PARTIAL 1,2,0 smooth_stone_slab")), "{l3:?}");
    }

    /// (G1 Nov 13 12:52, S3's control) The outside is FLOODED from the box's exterior, never taken
    /// as the bounding box: an L-shaped glass module whose notch holds a rider's rail far from every
    /// box face (5 cells sideways, 3 up: beyond a wind charge's reach) is EXPOSED through the notch's
    /// own air, the outside's reach into the box.
    #[test]
    fn burst_exposure_notch_is_outside() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let mut cells: Vec<(redsim::pos::BlockPos, &str)> = vec![(b(6, 0, 6), "minecraft:stone"), (b(6, 1, 6), "minecraft:rail[shape=north_south,waterlogged=false]")];
        for y in 0..=3 {
            for i in 0..=10 {
                for j in 0..=1 {
                    cells.push((b(i, y, j), "minecraft:glass"));
                    cells.push((b(j, y, i), "minecraft:glass"));
                }
            }
        }
        cells.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        cells.dedup_by_key(|(p, _)| *p);
        let blocks = cells.iter().map(|(p, s)| (*p, redsim::block::Block::parse(s).unwrap())).collect();
        // (the world's box wide enough for the L; the output chest inside the glass mass's corner
        // cell, so the shell box stays the L's bounds)
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(0, 0, 0), lo: b(-4, -2, -4), hi: b(14, 7, 14), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (l, n) = super::burst_exposure(&spec, &[]).unwrap();
        // (the nearest outside air is the notch's own, within a player's charge (2.4); a bounding-box
        // inside would put it beyond the box's faces, 3 or more away)
        assert!(n >= 1 && l.iter().any(|x| x.starts_with("  EXPOSED 6,1,6 (rail)") && x.contains("a player's wind charge (2.4)")), "the notch's air is outside: {l:?}");
    }

    /// (G1 Nov 13 04:52, burst-exposure's positive control through its own path) A rider's rail in
    /// the open, with outside air within reach, is EXPOSED (FAIL); the same rail sealed in a glass
    /// box (walls, roof and floor) has no clear COLLIDER line out (PASS).
    #[test]
    fn burst_exposure_open_and_sealed() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let open = spec_of(&[(b(0, 0, 0), "minecraft:stone"), (b(0, 1, 0), "minecraft:rail[shape=north_south,waterlogged=false]")]);
        let (l, n) = super::burst_exposure(&open, &[]).unwrap();
        assert!(n == 1 && l.iter().any(|x| x.starts_with("  EXPOSED 0,1,0 (rail)") && x.contains("a player's wind charge (2.4)")), "{l:?}");
        let mut cells: Vec<(redsim::pos::BlockPos, &str)> = vec![(b(0, 0, 0), "minecraft:stone"), (b(0, 1, 0), "minecraft:rail[shape=north_south,waterlogged=false]")];
        for x in -1i32..=1 {
            for z in -1i32..=1 {
                for y in -1i32..=4 {
                    let interior = x == 0 && z == 0 && (0..=3).contains(&y);
                    if !interior {
                        cells.push((b(x, y, z), "minecraft:glass"));
                    }
                }
            }
        }
        let (l2, n2) = super::burst_exposure(&spec_of(&cells), &[]).unwrap();
        assert!(n2 == 0 && l2[0].ends_with("(none exposed: no breeze siting rule)"), "{l2:?}");
        // (G1 hole 8, part 2, the control: an outside cell holding a block, even a collision-less
        // rail, or air under a block, is no standing place; outside air with air above is)
        let mut wo = redsim::World::new(b(-4, -2, -4), [9, 9, 9]);
        wo.put(b(3, 1, 0), redsim::block::Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap());
        wo.put(b(3, 3, 1), redsim::block::Block::parse("minecraft:stone").unwrap());
        // (the shell box's faces glass: its interior is closed, not reached by the exterior flood)
        for x in -1i32..=1 {
            for y in -1i32..=4 {
                for z in -1i32..=1 {
                    if x.abs() == 1 || z.abs() == 1 || y == -1 || y == 4 {
                        wo.put(b(x, y, z), redsim::block::Block::parse("minecraft:glass").unwrap());
                    }
                }
            }
        }
        let sh = super::Shell::new(&open, &wo, &[(b(-1, -1, -1), b(1, 4, 1))]);
        assert!(!super::burst_stand_cell(&wo, &sh, b(3, 1, 0)), "a rail cell outside is no standing place");
        assert!(!super::burst_stand_cell(&wo, &sh, b(3, 2, 1)), "air under a block is no standing place");
        assert!(super::burst_stand_cell(&wo, &sh, b(3, 1, 2)), "outside air with air above is");
        assert!(!super::burst_stand_cell(&wo, &sh, b(0, 2, 0)), "inside a closed shell is not outside");
        // (an opening in the shell lets the outside in: the flood reaches the interior)
        wo.put(b(1, 2, 0), redsim::block::Block::Air);
        let sh2 = super::Shell::new(&open, &wo, &[(b(-1, -1, -1), b(1, 4, 1))]);
        assert!(super::burst_stand_cell(&wo, &sh2, b(0, 2, 0)), "through an opening the interior is the outside's reach");
    }

    /// (ironspec Nov 13 03:32 (3)) The cat reach: a rider's rail on the ground in the open, a cat
    /// walking up beside it is in sight (FAIL, the positive control); the same rail inside a glass
    /// box three high on the ground, no cat cell in sight (the box can't be climbed: PASS).
    #[test]
    fn cat_reach_open_and_boxed() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let open = spec_of(&[(b(0, 0, 0), "minecraft:stone"), (b(0, 1, 0), "minecraft:rail[shape=north_south,waterlogged=false]")]);
        let (l, n) = super::cat_reach(&open, -1, 9, &[]).unwrap();
        assert!(n > 0 && l[0].ends_with("(FAIL)") && l.iter().any(|x| x.starts_with("  IN SIGHT ") && x.contains("of the rider at 0,1,0")), "{l:?}");
        // (G1 hole 9: a ground at or above the module's lowest block, or a margin out of range, is
        // refused, never judged)
        assert!(super::cat_reach(&open, 0, 9, &[]).is_err() && super::cat_reach(&open, -1, 4, &[]).is_err());
        let mut cells: Vec<(redsim::pos::BlockPos, &str)> = vec![(b(0, 0, 0), "minecraft:stone"), (b(0, 1, 0), "minecraft:rail[shape=north_south,waterlogged=false]")];
        for x in -1i32..=1 {
            for z in -1i32..=1 {
                for y in 1i32..=3 {
                    if x != 0 || z != 0 {
                        cells.push((b(x, y, z), "minecraft:glass"));
                    }
                }
                cells.push((b(x, 4, z), "minecraft:glass"));
                if x != 0 || z != 0 {
                    cells.push((b(x, 0, z), "minecraft:glass"));
                }
            }
        }
        let (l2, n2) = super::cat_reach(&spec_of(&cells), -1, 9, &[]).unwrap();
        assert!(n2 == 0 && l2[0].ends_with("(PASS)"), "{l2:?}");
    }

    /// (ironspec Nov 13 04:07) An override that leaves its cell as the design has it FAILS, named;
    /// a real one prints its cell before and after; a hold naming no stand-in fails; a hold on a
    /// stand-in the settle leaves as stated fails (it re-places nothing).
    #[test]
    fn noop_overrides_fail() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let glass = redsim::block::Block::parse("minecraft:glass").unwrap();
        let stone = redsim::block::Block::parse("minecraft:stone").unwrap();
        let spec = spec_of(&[(b(1, 1, 1), "minecraft:glass")]);
        let _ = &spec;
        // (G1 hole 5: the settled module world is what an override is compared with)
        let mut settled = redsim::World::new(b(0, 0, 0), [3, 3, 3]);
        settled.put(b(1, 1, 1), glass);
        let mut l = Vec::new();
        assert_eq!(super::override_report(&settled, &[(b(1, 1, 1), glass)], &[], &mut l), 1);
        assert!(l[0].starts_with("NO-OP OVERRIDE 1,1,1: the settled module already holds glass"), "{l:?}");
        let mut l2 = Vec::new();
        assert_eq!(super::override_report(&settled, &[(b(1, 1, 1), stone)], &[(b(2, 2, 2), "a driver (a phase)".to_string())], &mut l2), 1);
        assert!(l2[0] == "override 1,1,1: glass -> stone" && l2[1].starts_with("NO-OP HOLD 2,2,2: no stand-in there"), "{l2:?}");
        let mut w = redsim::World::new(b(0, 0, 0), [3, 3, 3]);
        w.put(b(1, 1, 1), stone);
        let mut l3 = Vec::new();
        assert_eq!(super::standin_survival(&mut w, &[(b(1, 1, 1), stone)], &[(b(1, 1, 1), "a driver (a phase)".to_string())], &mut l3), 1);
        assert!(l3[0].starts_with("NO-OP HOLD 1,1,1: the stand-in survives the settle unheld"), "{l3:?}");
    }

    /// (ironspec Nov 13 03:22 / 03:27 / 03:43, tick-exact's 26.3 flags) The wind-burst static: a
    /// lever, an oak trapdoor, a stone button and an oak fence gate counted; an iron trapdoor not
    /// (iron never opens by wind charge); an oak trapdoor held powered by a redstone block exempt; one
    /// powered with no constant source counted; a lever an `emulated:` line names listed apart as a
    /// declared stand-in.
    #[test]
    fn burst_reactive_counts_and_standins() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let spec = spec_of(&[
            (b(0, 0, 0), "minecraft:stone"),
            (b(0, 1, 0), "minecraft:lever[face=floor,facing=north,powered=false]"),
            (b(1, 1, 0), "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
            (b(2, 1, 0), "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
            (b(0, 0, 2), "minecraft:stone"),
            (b(0, 1, 2), "minecraft:lever[face=floor,facing=north,powered=false]"),
            (b(1, 1, 2), "minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]"),
            (b(1, 0, 2), "minecraft:redstone_block"),
            (b(2, 0, 2), "minecraft:stone"),
            (b(2, 1, 2), "minecraft:stone_button[face=floor,facing=north,powered=false]"),
            (b(-2, 1, 0), "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]"),
            (b(-2, 1, 2), "minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]"),
        ]);
        // (the lever (0,1,2) is a declared stand-in by its own entry; the switch entry names (0,1,0))
        let text = "emulated: 0,1,2=lever (a stand-in for day-1's line end)\n# switch: 0,1,0=the T's lever\n";
        let (l, n) = super::burst_reactive(&spec, text);
        assert_eq!(n, 5, "{l:?}");
        assert!(l[0].contains("12 cells scanned, 7 reactive, 5 counted (bar 0), 1 exempt (held powered by a constant source), 1 declared stand-ins apart (FAIL)"), "{l:?}");
        // (G1 Nov 13 04:52: no block scanned is NOT JUDGED, never a pass)
        let (le, ne) = super::burst_reactive(&spec_of(&[]), "");
        assert!(ne > 0 && le[0].ends_with("(NOT JUDGED: 0 cells scanned)"), "{le:?}");
        assert!(l.iter().any(|x| x.starts_with("  0,1,0 lever") && x.contains("switch: 0,1,0=the T's lever")), "{l:?}");
        assert!(l.iter().any(|x| x.starts_with("  STAND-IN 0,1,2 lever")), "{l:?}");
        assert!(l.iter().any(|x| x.starts_with("  EXEMPT 1,1,2 oak_trapdoor")), "{l:?}");
        assert!(l.iter().any(|x| x.starts_with("  -2,1,2 oak_trapdoor")), "powered with no constant source (the settle unpowers it): counted: {l:?}");
        assert!(!l.iter().any(|x| x.contains("iron_trapdoor")), "the iron trapdoor is not listed: {l:?}");
        // (G1 hole 7: a redstone block that is a declared stand-in (it may model an intermittent
        // driver) exempts nothing)
        let (l2, n2) = super::burst_reactive(&spec, "emulated: 1,0,2=redstone_block (a stand-in for the N line's repeater)\n");
        assert!(n2 == 7 && l2.iter().any(|x| x.starts_with("  1,1,2 oak_trapdoor")), "{l2:?}");
    }

    /// (G1 Nov 13 05:20, hole 6, the control) An entry names a cell only by the cells left of its
    /// own `=`: a lever at (1,1,2) beside `emulated: -1,1,2=glass` is COUNTED, as it is beside an
    /// entry for (21,1,2) and beside a reason that mentions "(1,1,2)".
    #[test]
    fn burst_reactive_strict_entry_match() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let spec = spec_of(&[(b(1, 0, 2), "minecraft:stone"), (b(1, 1, 2), "minecraft:lever[face=floor,facing=north,powered=false]"), (b(-1, 1, 2), "minecraft:glass")]);
        for text in ["emulated: -1,1,2=glass (a stand-in)\n", "# switches: 21,1,2=a far lever (a stand-in)\n", "emulated: -1,1,2=glass (the lever on (1,1,2) beside it is a stand-in)\n"] {
            let (l, n) = super::burst_reactive(&spec, text);
            assert!(n == 1 && l.iter().any(|x| x.starts_with("  1,1,2 lever") && x.ends_with("no header line names it")), "{text}: {l:?}");
        }
        let e = super::header_entries_strict("# switches: (7b90c3f1 r3) -1,26,9=K(-1); 1,26,9+3,26,9=J (a reason; with a semicolon)\n");
        assert!(e.len() == 2 && e[1].cells.len() == 2 && e[1].reason.contains("with a semicolon"), "{e:?}");
    }

    /// (ironspec Nov 13 03:24 (2)) The outside-sight static: a villager in a glass box sees no cell
    /// outside it (glass blocks a COLLIDER clip: EMPTY); one wall cell open, the cells beyond it in
    /// sight, the opening named (the positive control).
    #[test]
    fn outside_sight_glass_box_and_an_opening() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let mut cells: Vec<(redsim::pos::BlockPos, &str)> = Vec::new();
        for x in -3i32..=3 {
            for y in -1i32..=4 {
                for z in -3i32..=3 {
                    if x.abs() == 3 || z.abs() == 3 || y == -1 || y == 4 {
                        cells.push((b(x, y, z), "minecraft:glass"));
                    }
                }
            }
        }
        let feet = [redsim::entity::Vec3::new(0.5, 0.0, 0.5)];
        let (l, n) = super::outside_sight(&spec_of(&cells), &[], &feet, false, &[]).unwrap();
        assert!(n == 0 && l[0].ends_with("(EMPTY)"), "{l:?}");
        let open: Vec<(redsim::pos::BlockPos, &str)> = cells.iter().copied().filter(|(p, _)| *p != b(3, 1, 0)).collect();
        let (l2, n2) = super::outside_sight(&spec_of(&open), &[], &feet, false, &[]).unwrap();
        // (the opening and the open space behind it are the outside's reach (the exterior flood,
        // G1 hole 11): the cell beyond, (4,1,0), is in sight)
        assert!(n2 > 0 && l2.iter().any(|x| x.starts_with("  IN SIGHT 4,1,0 through ")), "{l2:?}");
        // (a concave pocket inside the box: an L-shaped glass wall leaves (3,3,3) air at the box's
        // corner, outside the shell, and it is tested)
        let mut lcells = cells.clone();
        lcells.retain(|(p, _)| !(p.x >= 2 && p.z >= 2 && p.y >= 2));
        let (l3, _) = super::outside_sight(&spec_of(&lcells), &[], &feet, false, &[]).unwrap();
        assert!(l3[0].contains("cells inside reached from outside") && !l3[0].contains("(0 cells inside reached from outside)"), "{l3:?}");
    }

    /// (farmopt / ironspec Nov 13 01:07) A stand-in the settled world doesn't hold (an iron trapdoor
    /// stated open, closed with no power) fails survival; declared held, it is re-placed as stated
    /// and printed with its driver and phase.
    #[test]
    fn standin_survival_fails_unless_held() {
        let p = redsim::pos::BlockPos::new(1, 1, 1);
        let shut = redsim::block::Block::parse("minecraft:iron_trapdoor[facing=east,half=bottom,open=false,powered=false,waterlogged=false]").unwrap();
        let open = redsim::block::Block::parse("minecraft:iron_trapdoor[facing=east,half=bottom,open=true,powered=true,waterlogged=false]").unwrap();
        let mut w = redsim::World::new(redsim::pos::BlockPos::new(0, 0, 0), [3, 3, 3]);
        w.put(p, shut);
        let mut lines = Vec::new();
        assert_eq!(super::standin_survival(&mut w, &[(p, open)], &[], &mut lines), 1);
        assert!(lines[0].starts_with("STAND-IN FAILS SURVIVAL 1,1,1: iron_trapdoor[facing=east,half=bottom,open=true"), "{lines:?}");
        assert_eq!(w.get(p), shut, "a failing stand-in is not held");
        let mut held = Vec::new();
        assert_eq!(super::standin_survival(&mut w, &[(p, open)], &[(p, "N's power (night)".to_string())], &mut held), 0);
        assert!(held[0].starts_with("held: 1,1,1 iron_trapdoor[facing=east,half=bottom,open=true") && held[0].ends_with(": N's power (night)"), "{held:?}");
        assert_eq!(w.get(p), open);
    }

    /// (ironspec Nov 12 23:30) The shared eye heights: the villager 1.62, the zombie 1.74 (their
    /// EntityType builders' `eyeHeight`), the pillager 0.85 x 1.95 (none set).
    #[test]
    fn eyes_from_entity_types() {
        assert!((super::eye_of(MobType::Villager) - 1.62).abs() < 1e-6);
        assert!((super::eye_of(MobType::Zombie) - 1.74).abs() < 1e-6);
        assert!((super::eye_of(MobType::Pillager) - 0.85 * 1.95).abs() < 1e-5);
    }
}

/// (ironspec Nov 13 04:07: a bench override that doesn't change its cell FAILS the run) Each
/// override's cell before (the SETTLED module world's state there, G1 Nov 13 05:20 hole 5, as
/// wet-spawn judges) and after it is laid; an override leaving its cell as the settled module has
/// it is a silent no-op and FAILS, as does a hold naming no stand-in. Pushes the lines; returns the
/// failures.
pub fn override_report(settled: &redsim::World, sets: &[(BlockPos, redsim::block::Block)], hold: &[(BlockPos, String)], lines: &mut Vec<String>) -> usize {
    let mut fails = 0usize;
    for (p, b) in sets {
        let before = settled.get(*p);
        let (bs, ns) = (before.to_string().trim_start_matches("minecraft:").to_string(), b.to_string().trim_start_matches("minecraft:").to_string());
        if before == *b {
            fails += 1;
            lines.push(format!("NO-OP OVERRIDE {},{},{}: the settled module already holds {bs} (FAILS: an override that doesn't change its cell, ironspec Nov 13 04:07)", p.x, p.y, p.z));
        } else {
            lines.push(format!("override {},{},{}: {bs} -> {ns}", p.x, p.y, p.z));
        }
    }
    for (p, why) in hold {
        if !sets.iter().any(|(q, _)| q == p) {
            fails += 1;
            lines.push(format!("NO-OP HOLD {},{},{}: no stand-in there to hold ({why}) (FAILS)", p.x, p.y, p.z));
        }
    }
    fails
}

/// (farmopt / ironspec Nov 13 01:03-01:07: the seven-property guard's survival property) Each
/// stand-in against the settled world: one the settle leaves as stated survives; one it changes
/// (an unpowered iron trapdoor stated open closes) FAILS with its before / after state, unless
/// the row declares it held (`hold`): then it is re-placed raw after the settle and printed
/// "held". A default hold would hide a stand-in that can't survive in the real build. Pushes the
/// lines; returns the failures.
pub fn standin_survival(w: &mut redsim::World, sets: &[(BlockPos, redsim::block::Block)], hold: &[(BlockPos, String)], lines: &mut Vec<String>) -> usize {
    let mut fails = 0usize;
    for (p, b) in sets {
        let now = w.get(*p);
        if now == *b {
            // (ironspec Nov 13 04:07: a hold on a stand-in the settle leaves as stated re-places
            // nothing: a silent no-op, FAILS)
            if let Some((_, why)) = hold.iter().find(|(q, _)| q == p) {
                fails += 1;
                lines.push(format!("NO-OP HOLD {},{},{}: the stand-in survives the settle unheld ({why}) (FAILS)", p.x, p.y, p.z));
            }
            continue;
        }
        let (stated, left) = (b.to_string().trim_start_matches("minecraft:").to_string(), now.to_string().trim_start_matches("minecraft:").to_string());
        if let Some((_, why)) = hold.iter().find(|(q, _)| q == p) {
            lines.push(format!("held: {},{},{} {stated} (re-placed after the settle, which left {left}): {why}", p.x, p.y, p.z));
            w.put(*p, *b);
        } else {
            fails += 1;
            lines.push(format!("STAND-IN FAILS SURVIVAL {},{},{}: {stated} -> {left} after the settle (declare `--hold {},{},{}=DRIVER (PHASE)` if the row holds it so)", p.x, p.y, p.z, p.x, p.y, p.z));
        }
    }
    fails
}

/// The sight diff, its samples (the rays cast with stand-ins to compare: 0 without a stand-in or
/// a ray) and the rays the stand-ins change plus the stand-ins failing survival.
pub fn sight_diff_counts(spec: &FarmSpec, sets: &[(BlockPos, redsim::block::Block)], extra: &[(Vec3, bool)], hold: &[(BlockPos, String)]) -> Result<(Vec<String>, usize, usize), String> {
    use redsim::entity::mob::types::MobType;
    let build = |s: &FarmSpec| -> Result<redsim::World, String> {
        let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
        let mut run = Run::start(s, &o)?;
        settle_full(&mut run)?;
        Ok(run.world)
    };
    let mut bench = spec.clone();
    for (p, b) in sets {
        bench.blocks.retain(|(q, _)| q != p);
        if !b.is_air() {
            bench.blocks.push((*p, b.clone()));
        }
    }
    let (wa, mut wb) = (build(spec)?, build(&bench)?);
    // (farmopt Nov 13 01:02: a stand-in the settle re-evaluates (an iron trapdoor with no power
    // closes) is held: re-placed raw after the settle, as the stand-in states it, and listed)
    let mut held: Vec<String> = Vec::new();
    let noops = override_report(&wa, sets, hold, &mut held);
    let survival_fails = standin_survival(&mut wb, sets, hold, &mut held) + noops;
    let wb = wb;
    // (the scarers' eyes: each boat's seat, as the flee-check's posts)
    let mut scarers: Vec<(Vec3, String)> = Vec::new();
    let mut boat = |x: f64, y: f64, z: f64, pill: bool| {
        let feet = y + if pill { PILLAGER_SEAT } else { zombie_seat() };
        scarers.push((Vec3::new(x, feet + if pill { eye_of(MobType::Pillager) } else { eye_of(MobType::Zombie) }, z), format!("the {} at {x:.1},{y:.1},{z:.1}", if pill { "pillager" } else { "zombie" })));
    };
    for (k, p, nbt) in &spec.entities {
        let n = nbt.as_deref().unwrap_or("");
        if k.ends_with("_boat") && (n.contains("minecraft:zombie\"") || n.contains("minecraft:pillager\"")) {
            boat(p[0], p[1], p[2], n.contains("minecraft:pillager\""));
        }
    }
    for (p, pill) in extra {
        boat(p.x, p.y, p.z, *pill);
    }
    // (ironspec Nov 12 23:30: the eye heights from redsim's entity data, 26.3's EntityType builders:
    // the villager's 1.62, the zombie's 1.74, the pillager's 0.85 x 1.95)
    let reye = |p: &Vec3| Vec3::new(p.x, p.y + eye_of(MobType::Villager), p.z);
    let riders: Vec<(String, Vec3)> = scarer_points(spec, &[]).into_iter().map(|(n, p)| (n, reye(&p))).collect();
    let mids: Vec<(String, Vec3)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(c, b)| (format!("{},{},{}", c.x, c.y, c.z), reye(&along_rail(*c, &b.to_string(), 0.5)))).collect();
    let d = |a: &Vec3, b: &Vec3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let set_cells: std::collections::HashSet<BlockPos> = sets.iter().map(|(p, _)| *p).collect();
    let mut by_cell: std::collections::BTreeMap<(i32, i32, i32), Vec<String>> = std::collections::BTreeMap::new();
    let (mut n_rs, mut n_rr, mut changed) = (0usize, 0usize, 0usize);
    let mut check = |from: Vec3, to: Vec3, what: String, by_cell: &mut std::collections::BTreeMap<(i32, i32, i32), Vec<String>>| {
        let (a, b) = (redsim::entity::mob::los::has_line_of_sight(&wa, from, to), redsim::entity::mob::los::has_line_of_sight(&wb, from, to));
        if a != b {
            changed += 1;
            let cells: Vec<BlockPos> = ray_cells(from, to).into_iter().filter(|c| set_cells.contains(c)).collect();
            let line = format!("{what}: {} in the module, {} with the stand-ins", if a { "clear" } else { "blocked" }, if b { "clear" } else { "blocked" });
            if cells.is_empty() {
                by_cell.entry((i32::MIN, 0, 0)).or_default().push(line.clone());
            }
            for c in cells {
                by_cell.entry((c.y, c.z, c.x)).or_default().push(line.clone());
            }
        }
    };
    for (rn, r) in &riders {
        for (s, sn) in &scarers {
            if d(r, s) <= 16.0 {
                n_rs += 1;
                check(*r, *s, format!("rider {rn} (eye {:.2},{:.2},{:.2}) to {sn}", r.x, r.y, r.z), &mut by_cell);
            }
        }
    }
    for (i, (an, a)) in mids.iter().enumerate() {
        for (bn, b) in mids.iter().skip(i + 1) {
            if d(a, b) <= 16.0 {
                n_rr += 1;
                check(*a, *b, format!("rider {an} to rider {bn}"), &mut by_cell);
            }
        }
    }
    let blocks_a: std::collections::HashMap<BlockPos, String> = spec.blocks.iter().map(|(p, b)| (*p, b.to_string())).collect();
    let short = |s: &str| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
    let mut lines = vec![format!(
        "sight (the stand-in guard's 7th property, ironspec Nov 12 20:10 (2a): each sight ray eye to eye, a COLLIDER clip, cast with and without the stand-ins): {} stand-in cells, {} scarers; {n_rs} rider-scarer rays and {n_rr} rider-rider rays within 16; {changed} changed by the stand-ins{}",
        sets.len(),
        scarers.len(),
        if changed == 0 && survival_fails == 0 { " (PASS)" } else { " (FAIL)" }
    )];
    for h in &held {
        lines.push(format!("  {h}"));
    }
    for (p, b) in sets {
        let k = (p.y, p.z, p.x);
        let v = by_cell.get(&k).cloned().unwrap_or_default();
        lines.push(format!(
            "  {} {},{},{} ({} in the module, {} as the stand-in): {} ray(s) changed",
            if v.is_empty() { "SAME" } else { "CHANGED" },
            p.x,
            p.y,
            p.z,
            blocks_a.get(p).map(|s| short(s)).unwrap_or_else(|| "air".to_string()),
            short(&b.to_string()),
            v.len()
        ));
        // (farmopt / ironspec Nov 13 00:22: every changed ray, so a row can tell the seat's rays
        // from the designed scare rays)
        for l in &v {
            lines.push(format!("    RAY {l}"));
        }
    }
    if let Some(v) = by_cell.get(&(i32::MIN, 0, 0)) {
        lines.push(format!("  (rays changed through no stand-in cell, a settle difference: {}){}", v.len(), v.iter().take(3).map(|l| format!("; {l}")).collect::<String>()));
    }
    // (ironspec Nov 12 23:56: the samples, the rays cast with a stand-in to compare; none NOT JUDGED;
    // G1 Nov 13 05:20, hole 13: the header's word comes from the same numbers as the verdict)
    let samples = if sets.is_empty() { 0 } else { n_rs + n_rr };
    let word = if samples == 0 { " (NOT JUDGED: 0 samples)" } else if changed == 0 && survival_fails == 0 { " (PASS)" } else { " (FAIL)" };
    if let Some(h) = lines.first_mut() {
        for w in [" (PASS)", " (FAIL)"] {
            if h.ends_with(w) {
                h.truncate(h.len() - w.len());
                h.push_str(word);
                break;
            }
        }
    }
    Ok((lines, samples, changed + survival_fails))
}

/// [`sight_diff_counts`]'s lines alone.
pub fn sight_diff(spec: &FarmSpec, sets: &[(BlockPos, redsim::block::Block)], extra: &[(Vec3, bool)]) -> Result<Vec<String>, String> {
    sight_diff_counts(spec, sets, extra, &[]).map(|(l, _, _)| l)
}

/// `VillagerHostilesSensor.ACCEPTABLE_DISTANCE_FROM_HOSTILES` (26.3), by entity id.
fn hostile_range(id: &str) -> Option<f64> {
    match id.trim_start_matches("minecraft:") {
        "drowned" | "husk" | "vex" | "zombie" | "zombie_villager" => Some(8.0),
        "vindicator" | "zoglin" => Some(10.0),
        "evoker" | "illusioner" | "ravager" => Some(12.0),
        "pillager" => Some(15.0),
        _ => None,
    }
}

/// (farmopt / tick-exact Nov 13 00:16) The hostile sensor at a villager's feet points (a napper
/// seated at the queue seat: its cart's position, the eye 1.62 over it): every hostile the design
/// holds (a boat's zombie or pillager at its seat, a free one where it stands), its type's
/// `ACCEPTABLE_DISTANCE_FROM_HOSTILES`, the feet-to-feet distance (`distanceToSqr`) and the sight
/// ray eye to eye (`hasLineOfSight`: a COLLIDER clip from the villager's eye to the hostile's):
/// in range and in sight, the hostile is in NEAREST_HOSTILE (`NearestVisibleLivingEntitySensor`
/// over the visible list). For each boat hostile in it at some point, the occluder search
/// ([`occlude`]): glass on every ray to the points, none on a ray it covers now. Returns the lines
/// and the samples (points x hostiles).
/// (G1 Nov 13 05:20, hole 10, the one-state-source rule) Every rail cell of the SETTLED world inside
/// the box, with its settled state: the rider and cart positions the shell statics cast from.
pub fn settled_rails(w: &redsim::World, lo: BlockPos, hi: BlockPos) -> Vec<(BlockPos, String)> {
    let mut out = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                let s = w.get(p).to_string();
                if s.contains("rail[") {
                    out.push((p, s));
                }
            }
        }
    }
    out
}

/// The module's shell: the union of its boxes (one per copy in a tiled file, `--shell-box`; by
/// default the blocks' bounds), and the EXTERIOR cells inside them (G1 Nov 13 05:20, hole 11): every
/// cell with no collision shape that the outside reaches by a flood through such cells (a concave
/// corner's air; through a shell opening, the open space behind it too: for an open shell the
/// honest answer is that the outside comes in). `outside(c)`: not in any box, or such a cell.
pub struct Shell {
    pub boxes: Vec<(BlockPos, BlockPos)>,
    exterior_inside: std::collections::HashSet<BlockPos>,
}

impl Shell {
    pub fn new(spec: &FarmSpec, w: &redsim::World, boxes: &[(BlockPos, BlockPos)]) -> Shell {
        let boxes: Vec<(BlockPos, BlockPos)> = if boxes.is_empty() { vec![shell_box(spec)] } else { boxes.to_vec() };
        let in_box = |c: &BlockPos| boxes.iter().any(|(lo, hi)| c.x >= lo.x && c.x <= hi.x && c.y >= lo.y && c.y <= hi.y && c.z >= lo.z && c.z <= hi.z);
        let open = |c: &BlockPos| redsim::behavior::collision_boxes(&w.get(*c)).is_empty();
        let six = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
        let mut exterior_inside = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        // (the seeds: each open cell on a box's faces with a neighbour outside every box)
        for (lo, hi) in &boxes {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        if !(x == lo.x || x == hi.x || y == lo.y || y == hi.y || z == lo.z || z == hi.z) {
                            continue;
                        }
                        let c = BlockPos::new(x, y, z);
                        if open(&c) && six.iter().any(|(dx, dy, dz)| !in_box(&BlockPos::new(x + dx, y + dy, z + dz))) && exterior_inside.insert(c) {
                            queue.push_back(c);
                        }
                    }
                }
            }
        }
        while let Some(c) = queue.pop_front() {
            for (dx, dy, dz) in six {
                let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if in_box(&n) && open(&n) && exterior_inside.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        Shell { boxes, exterior_inside }
    }

    pub fn in_box(&self, c: &BlockPos) -> bool {
        self.boxes.iter().any(|(lo, hi)| c.x >= lo.x && c.x <= hi.x && c.y >= lo.y && c.y <= hi.y && c.z >= lo.z && c.z <= hi.z)
    }

    pub fn outside(&self, c: &BlockPos) -> bool {
        !self.in_box(c) || self.exterior_inside.contains(c)
    }

    /// The exterior cells inside the boxes (the outside's reach into the module).
    pub fn exterior_count(&self) -> usize {
        self.exterior_inside.len()
    }

    pub fn describe(&self) -> String {
        format!("{} ({} cells inside reached from outside)", self.boxes.iter().map(|(lo, hi)| format!("({},{},{})..({},{},{})", lo.x, lo.y, lo.z, hi.x, hi.y, hi.z)).collect::<Vec<_>>().join(" + "), self.exterior_inside.len())
    }
}

/// `--shell-box x0,y0,z0,x1,y1,z1` values, parsed.
pub fn parse_boxes(v: &[String]) -> Result<Vec<(BlockPos, BlockPos)>, String> {
    let mut out = Vec::new();
    for s in v {
        let n: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("--shell-box {s}: {e}"))?;
        if n.len() != 6 {
            return Err(format!("--shell-box {s}: x0,y0,z0,x1,y1,z1"));
        }
        out.push((BlockPos::new(n[0].min(n[3]), n[1].min(n[4]), n[2].min(n[5])), BlockPos::new(n[0].max(n[3]), n[1].max(n[4]), n[2].max(n[5]))));
    }
    Ok(out)
}

/// The module's shell box: the bounding box of the design's blocks (the design's own box is
/// padded; the statics of what lies outside the module start past its blocks).
pub fn shell_box(spec: &FarmSpec) -> (BlockPos, BlockPos) {
    let mut lo = BlockPos::new(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = BlockPos::new(i32::MIN, i32::MIN, i32::MIN);
    for (p, b) in &spec.blocks {
        if b.is_air() {
            continue;
        }
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    if lo.x > hi.x { (spec.lo, spec.hi) } else { (lo, hi) }
}

/// One header entry, parsed strictly: its key (`switches`, `emulated`, ...), the cells left of its
/// own `=` (`+`-joined x,y,z) and its reason.
#[derive(Clone, Debug)]
pub struct HeaderEntry {
    pub key: String,
    pub cells: Vec<BlockPos>,
    pub reason: String,
}

impl HeaderEntry {
    /// A declared stand-in: an `emulated:` entry, or one whose reason calls the cell a stand-in.
    pub fn is_standin(&self) -> bool {
        self.key == "emulated" || self.reason.contains("stand-in")
    }
}

/// Every `key: x,y,z[+x,y,z...]=REASON; ...` entry of a design's text, comment (`# key:`) and meta
/// forms alike, a leading attribution in parentheses skipped (G1 Nov 13 05:20, hole 6: a cell is
/// named only by the cells left of an entry's own `=`; an entry starts where a `;`-separated part
/// begins with such a cell list, so a `;` inside a reason stays in the reason).
pub fn header_entries_strict(text: &str) -> Vec<HeaderEntry> {
    let cells_of = |c: &str| -> Option<Vec<BlockPos>> {
        let mut v = Vec::new();
        for t in c.split('+') {
            let n: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
            if n.len() != 3 {
                return None;
            }
            v.push(BlockPos::new(n[0], n[1], n[2]));
        }
        (!v.is_empty()).then_some(v)
    };
    let mut out = Vec::new();
    for l in text.lines() {
        let body = l.trim_start().strip_prefix('#').map(str::trim_start).unwrap_or(l.trim_start());
        let Some((key, rest)) = body.split_once(':') else { continue };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '-' || c == '_') {
            continue;
        }
        let mut rest = rest.trim_start();
        if rest.starts_with('(') {
            // (a leading attribution in parentheses)
            let mut depth = 0i32;
            let mut end = None;
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(e) = end {
                rest = rest[e..].trim_start();
            }
        }
        let mut cur: Option<(Vec<BlockPos>, String)> = None;
        for part in rest.split(';') {
            let head = part.trim_start();
            if let Some((c, r)) = head.split_once('=')
                && let Some(cs) = cells_of(c)
            {
                if let Some((cells, reason)) = cur.take() {
                    out.push(HeaderEntry { key: key.to_string(), cells, reason: reason.trim().to_string() });
                }
                cur = Some((cs, r.to_string()));
                continue;
            }
            if let Some((_, reason)) = cur.as_mut() {
                reason.push(';');
                reason.push_str(part);
            }
        }
        if let Some((cells, reason)) = cur {
            out.push(HeaderEntry { key: key.to_string(), cells, reason: reason.trim().to_string() });
        }
    }
    out
}

/// (ironspec Nov 13 03:22) The blocks a wind burst (a breeze's wind charge, `onExplosionHit` with
/// `canTriggerBlocks`) can change, by 26.3's block classes: levers (pulled), buttons (pressed),
/// doors, trapdoors and fence gates whose BlockSetType opens by wind charge (wood and copper, never
/// iron), bells (rung), candles (put out), beehives and bee nests, creaking hearts; each with the
/// design's header line that names its cell (its role and owner). Returns the lines and the count
/// (the bar is 0).
pub fn burst_reactive(spec: &FarmSpec, text: &str) -> (Vec<String>, usize) {
    let reactive = |id: &str| -> Option<&'static str> {
        let id = id.trim_start_matches("minecraft:");
        if id == "lever" {
            Some("a lever (pulled)")
        } else if id.ends_with("_button") {
            Some("a button (pressed)")
        } else if id.ends_with("_door") && id != "iron_door" {
            Some("a door (opened or shut)")
        } else if id.ends_with("_trapdoor") && id != "iron_trapdoor" {
            Some("a trapdoor (opened or shut)")
        } else if id.ends_with("_fence_gate") {
            Some("a fence gate (opened or shut)")
        } else if id == "bell" {
            Some("a bell (rung)")
        } else if id == "candle" || id.ends_with("_candle") {
            Some("a candle (put out)")
        } else if id == "beehive" || id == "bee_nest" {
            Some("a beehive")
        } else if id == "creaking_heart" {
            Some("a creaking heart")
        } else {
            None
        }
    };
    let mut cells: Vec<(BlockPos, String, &'static str)> = spec
        .blocks
        .iter()
        .filter_map(|(p, b)| {
            let s = b.to_string();
            reactive(s.split('[').next().unwrap_or("")).map(|k| (*p, s, k))
        })
        .collect();
    cells.sort_by_key(|(p, _, _)| (p.y, p.z, p.x));
    let entries = header_entries_strict(text);
    // (the settled world: the constant-power exemption reads what the settle leaves)
    let settled = {
        let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
        match Run::start(spec, &o) {
            Ok(mut run) => {
                if let Err(e) = settle_full(&mut run) {
                    return (vec![format!("wind-burst reactive blocks: {e} (NOT JUDGED)")], usize::MAX);
                }
                run.world
            }
            Err(e) => return (vec![format!("wind-burst reactive blocks: the design does not build ({e}) (NOT JUDGED)")], usize::MAX),
        }
    };
    let mut lines = Vec::new();
    let mut standins: Vec<String> = Vec::new();
    let mut conditional: Vec<String> = Vec::new();
    let mut counted = 0usize;
    for (p, s, k) in &cells {
        let key = format!("{},{},{}", p.x, p.y, p.z);
        // (G1 Nov 13 05:20, hole 6: the header entries naming the cell, parsed strictly: an entry
        // names a cell only when the cell is one of the `+`-joined x,y,z left of its own `=`; a
        // mention inside a reason, or a longer number containing the digits, names nothing)
        let naming: Vec<&HeaderEntry> = entries.iter().filter(|e| e.cells.contains(p)).collect();
        let roles: Vec<String> = naming.iter().map(|e| format!("{}: {}={}", e.key, key, e.reason.chars().take(120).collect::<String>())).take(3).collect();
        // (ironspec Nov 13 03:27: a declared stand-in (an `emulated:` entry, or an entry whose
        // reason calls the cell a stand-in) on its own line until its real line replaces it)
        let standin = naming.iter().any(|e| e.is_standin());
        // (tick-exact's 26.3 flags, farmopt 03:41 / ironspec 03:43: a door, trapdoor or fence gate
        // is skipped while POWERED, so it is exempt only when held powered by a CONSTANT source in
        // every phase: in the SETTLED world it is powered and a redstone block beside it is no
        // declared stand-in (a stand-in may model an intermittent driver; G1 hole 7); a lever and
        // a button always count)
        let gated = s.contains("_door[") || s.contains("_trapdoor[") || s.contains("_fence_gate[");
        let settled_powered = settled.get(*p).to_string().contains("powered=true");
        let constant = gated
            && settled_powered
            && [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().any(|(dx, dy, dz)| {
                let n = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                settled.get(n).to_string().starts_with("minecraft:redstone_block") && !entries.iter().any(|e| e.cells.contains(&n) && e.is_standin())
            });
        let powered_skip = constant;
        let row = format!("  {key} {} ({k}){}", s.trim_start_matches("minecraft:"), if roles.is_empty() { "; no header line names it".to_string() } else { format!("; {}", roles.join(" | ")) });
        if standin {
            standins.push(row);
        } else if powered_skip {
            conditional.push(row);
        } else {
            counted += 1;
            lines.push(row);
        }
    }
    let mut out = vec![format!(
        "wind-burst reactive blocks (ironspec Nov 13 03:22 / 03:27 / 03:43, tick-exact's 26.3 flags: levers and buttons always count; doors and trapdoors of any set but iron, and fence gates, unless held powered by a constant source in every phase (a redstone block beside it); bells, candles, beehives, creaking hearts): {} cells scanned, {} reactive, {counted} counted (bar 0), {} exempt (held powered by a constant source), {} declared stand-ins apart{}",
        spec.blocks.len(),
        cells.len(),
        conditional.len(),
        standins.len(),
        // (G1 Nov 13 04:52: no vacuous pass: a design with no block scanned is NOT JUDGED)
        if spec.blocks.is_empty() { " (NOT JUDGED: 0 cells scanned)" } else if counted == 0 { " (PASS)" } else { " (FAIL)" }
    )];
    out.extend(lines);
    if !conditional.is_empty() {
        out.push(format!("  exempt (held powered by a constant source in every phase: a redstone block beside it): {}", conditional.len()));
        out.extend(conditional.into_iter().map(|r| format!("  EXEMPT {}", r.trim_start())));
    }
    if !standins.is_empty() {
        out.push(format!("  declared stand-ins (until their real lines replace them): {}", standins.len()));
        out.extend(standins.into_iter().map(|r| format!("  STAND-IN {}", r.trim_start())));
    }
    (out, if spec.blocks.is_empty() { usize::MAX } else { counted })
}

/// (ironspec Nov 13 03:24 (2)) The sources outside the module: every cell outside the design's
/// box within 8 of a villager's eye (its feet + 1.62) and in sight of it (LivingEntity.hasLineOfSight,
/// a COLLIDER clip: glass blocks the ray), to the cell's centre or its top; each with the module's
/// last cell the ray leaves through (the wall an opaque swap would close). Returns the lines and the
/// cells in sight.
pub fn outside_sight(spec: &FarmSpec, at: &[BlockPos], feet_points: &[Vec3], rails_mode: bool, boxes: &[(BlockPos, BlockPos)]) -> Result<(Vec<String>, usize), String> {
    use redsim::entity::mob::types::MobType;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let veye = eye_of(MobType::Villager);
    // (the module's shell: its boxes (the blocks' bounds, or one per copy), the exterior pockets
    // inside them counted outside (G1 hole 11), so a ray leaving through a shell opening names it)
    let shell = Shell::new(spec, w, boxes);
    let inside = |c: &BlockPos| !shell.outside(c);
    // (the villagers' feet: each `--at` rail cell's cart along the SETTLED rail (t 0, 0.25, ... 1),
    // every settled rail's cart at its middle with `--rails` (G1 hole 10 / 11), and `--feet`)
    let mut points: Vec<Vec3> = feet_points.to_vec();
    for c in at {
        let s = w.get(*c).to_string();
        if !s.contains("rail[") {
            return Err(format!("--at {},{},{}: not a rail cell in the settled world ({s})", c.x, c.y, c.z));
        }
        for k in 0..=4 {
            points.push(along_rail(*c, &s, k as f64 * 0.25));
        }
    }
    if rails_mode {
        for (lo, hi) in &shell.boxes {
            for (c, s) in settled_rails(w, *lo, *hi) {
                points.push(along_rail(c, &s, 0.5));
            }
        }
    }
    let mut seen: std::collections::BTreeMap<(i32, i32, i32), (String, Vec<String>)> = std::collections::BTreeMap::new();
    let mut tested = 0usize;
    for p in &points {
        let e = Vec3::new(p.x, p.y + veye, p.z);
        for x in (e.x - 9.0).floor() as i32..=(e.x + 9.0).ceil() as i32 {
            for y in (e.y - 9.0).floor() as i32..=(e.y + 9.0).ceil() as i32 {
                for z in (e.z - 9.0).floor() as i32..=(e.z + 9.0).ceil() as i32 {
                    let c = BlockPos::new(x, y, z);
                    if inside(&c) {
                        continue;
                    }
                    let targets = [Vec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), Vec3::new(x as f64 + 0.5, y as f64 + 0.95, z as f64 + 0.5)];
                    let near: Vec<&Vec3> = targets.iter().filter(|t| ((t.x - e.x).powi(2) + (t.y - e.y).powi(2) + (t.z - e.z).powi(2)).sqrt() <= 8.0).collect();
                    if near.is_empty() {
                        continue;
                    }
                    tested += 1;
                    if let Some(t) = near.iter().find(|t| redsim::entity::mob::los::has_line_of_sight(w, e, ***t)) {
                        // (the last module cell on the ray: the wall it leaves through)
                        let wall = ray_cells(e, **t).into_iter().filter(|q| inside(q)).last().map(|q| format!("{},{},{} ({})", q.x, q.y, q.z, w.get(q).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or(""))).unwrap_or_else(|| "none".to_string());
                        let entry = seen.entry((x, y, z)).or_insert((wall, Vec::new()));
                        entry.1.push(format!("{:.2},{:.3},{:.2}", p.x, p.y, p.z));
                    }
                }
            }
        }
    }
    let mut lines = vec![format!(
        "sources outside the module (ironspec Nov 13 03:24 (2) / 03:36: every cell outside the module's shell {} (its exterior pockets included) within 8 of a villager's eye (feet + {veye:.2}) and in sight: the single eye-to-eye segment clipped against the composed collision shapes (COLLIDER, fluids ignored; glass shields; the openings are air, collision-less blocks and partial colliders' gaps)): {} feet points, {} outside cells within 8 tested, {} in sight{}",
        shell.describe(),
        points.len(),
        tested,
        seen.len(),
        if tested == 0 { " (NOT JUDGED: 0 samples)" } else if seen.is_empty() { " (EMPTY)" } else { "" }
    )];
    for ((x, y, z), (wall, from)) in &seen {
        lines.push(format!("  IN SIGHT {x},{y},{z} through {wall}, from {} point(s): {}", from.len(), from.iter().take(4).cloned().collect::<Vec<_>>().join(" ")));
    }
    Ok((lines, if tested == 0 { usize::MAX } else { seen.len() }))
}

/// (G1 Nov 13 05:20, hole 8) A cell a breeze or a player can stand in for the burst exposure: outside
/// the shell, it and the cell above it AIR in the settled world (a block there, even one with no
/// collision, is no standing place).
pub fn burst_stand_cell(w: &redsim::World, shell: &Shell, q: BlockPos) -> bool {
    shell.outside(&q) && w.get(q).is_air() && w.get(q.above()).is_air()
}

/// (ironspec Nov 13 03:22, tick-exact's knockback numbers) The wind-burst exposure: every rider and
/// cart position (each rail cell's cart at its middle, a waiting napper included) with a clear
/// COLLIDER line from a point of its box (feet, waist, head) to an outside air cell within 10 (a
/// breeze or a player can stand there; the burst's exposure is getSeenPercent over the box's points),
/// with the nearest such cell and its reach class (a player's wind charge 2.4, a breeze's 6.0, the
/// Wind Burst mace 7.0, the Wind Charged death burst 6-10). Returns the lines and the exposed
/// positions.
pub fn burst_exposure(spec: &FarmSpec, boxes: &[(BlockPos, BlockPos)]) -> Result<(Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let shell = Shell::new(spec, w, boxes);
    // (G1 hole 10: the cart positions from the SETTLED world's rails, in every shell box)
    let mut rails: Vec<(BlockPos, String)> = shell.boxes.iter().flat_map(|(lo, hi)| settled_rails(w, *lo, *hi)).collect();
    rails.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    rails.dedup_by_key(|(p, _)| *p);
    let class = |d: f64| if d <= 2.4 { "a player's wind charge (2.4)" } else if d <= 6.0 { "a breeze's charge (6.0)" } else if d <= 7.0 { "the Wind Burst mace (7.0)" } else { "the Wind Charged death burst (6-10)" };
    let mut lines = Vec::new();
    let mut exposed = 0usize;
    for (c, s) in &rails {
        let cart = along_rail(*c, s, 0.5);
        let pts = [Vec3::new(cart.x, cart.y + 0.3, cart.z), Vec3::new(cart.x, cart.y + 1.0, cart.z), Vec3::new(cart.x, cart.y + 1.9, cart.z)];
        let mid = pts[1];
        let mut best: Option<(f64, BlockPos)> = None;
        for x in (mid.x - 11.0).floor() as i32..=(mid.x + 11.0).ceil() as i32 {
            for y in (mid.y - 11.0).floor() as i32..=(mid.y + 11.0).ceil() as i32 {
                for z in (mid.z - 11.0).floor() as i32..=(mid.z + 11.0).ceil() as i32 {
                    let q = BlockPos::new(x, y, z);
                    // (G1 hole 8: an outside cell a breeze or a player can stand in: it and the
                    // cell above it air in the SETTLED world, outside the shell)
                    if !burst_stand_cell(w, &shell, q) {
                        continue;
                    }
                    let t = Vec3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
                    let d = ((t.x - mid.x).powi(2) + (t.y - mid.y).powi(2) + (t.z - mid.z).powi(2)).sqrt();
                    if d > 10.0 || best.is_some_and(|(bd, _)| bd <= d) {
                        continue;
                    }
                    if pts.iter().any(|p| redsim::entity::mob::los::has_line_of_sight(w, *p, t)) {
                        best = Some((d, q));
                    }
                }
            }
        }
        if let Some((d, q)) = best {
            exposed += 1;
            lines.push(format!("  EXPOSED {},{},{} ({}): the outside air cell {},{},{} at {d:.2}, {}", c.x, c.y, c.z, s.trim_start_matches("minecraft:").split('[').next().unwrap_or(""), q.x, q.y, q.z, class(d)));
        }
    }
    let mut out = vec![format!(
        "wind-burst exposure (ironspec Nov 13 03:22, tick-exact's knockback: reach 2 x radius, exposure getSeenPercent over COLLIDER clips): {} rider / cart positions (each rail cell's cart), {} with a clear line to an outside air cell within 10{}",
        rails.len(),
        exposed,
        if rails.is_empty() { " (NOT JUDGED: 0 samples)" } else if exposed == 0 { " (none exposed: no breeze siting rule)" } else { "" }
    )];
    out.extend(lines);
    Ok((out, if rails.is_empty() { usize::MAX } else { exposed }))
}

/// (ironspec Nov 13 04:34: the shell rule, no collision-less block in a shell cell) The module's
/// openings: the open space a rider can see into (every cell with no collision shape, flooded
/// 6-connected from each rail cell, the riders' cells) followed to the shell box's boundary; each
/// open cell on the boundary (one face out of the box) is an opening, listed with its block (air,
/// or a collision-less block: a lever, a button, a rail, dust, a torch...). Cells with a partial
/// collision shape bordering that space on the boundary are listed apart (their gaps can pass a
/// sight segment). Returns the lines and the openings (the bar is 0).
pub fn shell_openings(spec: &FarmSpec, shell_boxes: &[(BlockPos, BlockPos)]) -> Result<(Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let shell = Shell::new(spec, w, shell_boxes);
    let six = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
    // (the openings are judged against the boxes themselves: the rider space reaching a box's face;
    // the exterior flood would already count an open module's rider space as outside)
    let inside = |c: &BlockPos| shell.in_box(c);
    let on_face = |c: &BlockPos| six.iter().any(|(dx, dy, dz)| !shell.in_box(&BlockPos::new(c.x + dx, c.y + dy, c.z + dz)));
    let boxes = |c: &BlockPos| redsim::behavior::collision_boxes(&w.get(*c));
    let full = |c: &BlockPos| boxes(c).iter().any(|b| b == &[0, 16, 0, 16, 0, 16]);
    let mut open: std::collections::HashSet<BlockPos> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<BlockPos> = std::collections::VecDeque::new();
    // (G1 hole 10: the riders' cells are the SETTLED world's rails, in every shell box)
    for (lo, hi) in &shell.boxes {
        for (p, _) in settled_rails(w, *lo, *hi) {
            if boxes(&p).is_empty() && inside(&p) && open.insert(p) {
                queue.push_back(p);
            }
        }
    }
    let seeds = open.len();
    let mut openings: Vec<(BlockPos, String)> = Vec::new();
    let mut partial: Vec<(BlockPos, String)> = Vec::new();
    while let Some(c) = queue.pop_front() {
        for (dx, dy, dz) in six {
            let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
            if !inside(&n) {
                // (the open space reaches the outside (past a box, or an exterior pocket inside
                // one): the cell it leaves from is an opening)
                if !openings.iter().any(|(q, _)| *q == c) {
                    openings.push((c, w.get(c).to_string().trim_start_matches("minecraft:").to_string()));
                }
                continue;
            }
            if open.contains(&n) {
                continue;
            }
            if boxes(&n).is_empty() {
                open.insert(n);
                queue.push_back(n);
            } else if !full(&n) && on_face(&n) && !partial.iter().any(|(q, _)| *q == n) {
                partial.push((n, w.get(n).to_string().trim_start_matches("minecraft:").to_string()));
            }
        }
    }
    openings.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    partial.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    let mut lines = vec![format!(
        "shell openings (ironspec Nov 13 04:34: no collision-less block in a shell cell): the open space flooded from {seeds} rail cells (every cell with no collision shape, 6-connected) to the shell boxes {} (each box's faces): {} open cells, {} openings on its boundary (bar 0), {} partial colliders on the boundary beside it{}",
        shell.describe(),
        open.len(),
        openings.len(),
        partial.len(),
        if seeds == 0 { " (NOT JUDGED: 0 samples)" } else if openings.is_empty() { " (PASS)" } else { " (FAIL)" }
    )];
    for (p, s) in &openings {
        lines.push(format!("  OPENING {},{},{} {s}", p.x, p.y, p.z));
    }
    for (p, s) in &partial {
        lines.push(format!("  PARTIAL {},{},{} {s}", p.x, p.y, p.z));
    }
    Ok((lines, if seeds == 0 { usize::MAX } else { openings.len() }))
}

/// (ironspec Nov 13 03:32 (3)) The cat reach: every cell a cat (0.6 x 0.7, eye 0.35) can stand on
/// that connects by cat-walkable moves to ground where cats can spawn, within 8 of a rider's eye and
/// in sight of it (the eye-to-eye segment, COLLIDER, fluids ignored). The ground is a plane at
/// `ground_y` (its top face) around the design's box widened by `margin`; the moves are a step to a
/// side cell whose floor rises at most 1 (a jump), or a drop of at most 3 (Mob.getMaxFallDistance);
/// a cat stands where its box fits on a collision top (a full block below, or a low shape in its own
/// cell). Rider eyes: each rail cell's cart at its middle + 1.62. Returns the lines and the cells
/// within 8 and in sight (the bar is 0).
pub fn cat_reach(spec: &FarmSpec, ground_y: i32, margin: i32, shell_boxes: &[(BlockPos, BlockPos)]) -> Result<(Vec<String>, usize), String> {
    use redsim::entity::Aabb;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let shell = Shell::new(spec, w, shell_boxes);
    // (the union of the shell boxes: the ground plane reaches `margin` past it)
    let lo = BlockPos::new(shell.boxes.iter().map(|b| b.0.x).min().unwrap_or(0), shell.boxes.iter().map(|b| b.0.y).min().unwrap_or(0), shell.boxes.iter().map(|b| b.0.z).min().unwrap_or(0));
    let hi = BlockPos::new(shell.boxes.iter().map(|b| b.1.x).max().unwrap_or(0), shell.boxes.iter().map(|b| b.1.y).max().unwrap_or(0), shell.boxes.iter().map(|b| b.1.z).max().unwrap_or(0));
    // (G1 Nov 13 05:20, hole 9: the ground below the module, the margin ranged: a plane at or above
    // the module's lowest block would seed cells that can't move and pass)
    if ground_y >= lo.y {
        return Err(format!("cat-reach: --ground-y {ground_y} must be below the module's lowest block (y {})", lo.y));
    }
    // (G1 on f4e5482ee, K9's remainder: a lower bound too: a ground far below the module seeds
    // cells no cat could climb from, a vacuous PASS)
    if ground_y < lo.y - 64 {
        return Err(format!("cat-reach: --ground-y {ground_y} is more than 64 below the module's lowest block (y {})", lo.y));
    }
    if !(9..=64).contains(&margin) {
        return Err(format!("cat-reach: --margin {margin} must be 9..=64 (the ground must reach past every rider's 8)"));
    }
    let (x0, x1, z0, z1) = (lo.x - margin, hi.x + margin, lo.z - margin, hi.z + margin);
    let (y0, y1) = (ground_y + 1, hi.y + 2);
    let in_box = |c: &BlockPos| shell.in_box(c);
    // (the feet height a cat stands at in cell c, if its box fits there: the highest collision top
    // under its box between c.y - 0.5 and c.y + 0.5 (a full block below: c.y; a slab or carpet in
    // c: above it), or the ground plane's top)
    let feet = |c: BlockPos| -> Option<f64> {
        let ground = (c.y == ground_y + 1 && !in_box(&BlockPos::new(c.x, ground_y, c.z))).then_some(c.y as f64);
        let probe = Aabb::new(c.x as f64 + 0.2, c.y as f64 - 0.5, c.z as f64 + 0.2, c.x as f64 + 0.8, c.y as f64 + 0.5, c.z as f64 + 0.8);
        let shape = w.collision_shapes(probe).iter().map(|a| a.max_y).filter(|t| *t >= c.y as f64 - 0.5 && *t <= c.y as f64 + 0.5 + 1e-9).fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m: f64| m.max(t))));
        let f = match (ground, shape) {
            (Some(g), Some(s)) => g.max(s),
            (g, s) => g.or(s)?,
        };
        let bb = Aabb::new(c.x as f64 + 0.2, f + 1e-6, c.z as f64 + 0.2, c.x as f64 + 0.8, f + 0.7, c.z as f64 + 0.8);
        if w.collision_shapes(bb).is_empty() { Some(f) } else { None }
    };
    let mut reached: std::collections::HashMap<BlockPos, f64> = std::collections::HashMap::new();
    let mut queue: std::collections::VecDeque<BlockPos> = std::collections::VecDeque::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            let c = BlockPos::new(x, ground_y + 1, z);
            if in_box(&c) {
                continue;
            }
            if let Some(f) = feet(c) {
                reached.insert(c, f);
                queue.push_back(c);
            }
        }
    }
    let seeds = reached.len();
    while let Some(c) = queue.pop_front() {
        let fc = reached[&c];
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            // (a step up to 1, level, or a drop of up to 3: the first standable cell down the column)
            for dy in [1, 0, -1, -2, -3] {
                let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if n.x < x0 || n.x > x1 || n.z < z0 || n.z > z1 || n.y < y0 || n.y > y1 || reached.contains_key(&n) {
                    continue;
                }
                let Some(fnb) = feet(n) else { continue };
                if fnb - fc > 1.0 + 1e-9 || fc - fnb > 3.0 + 1e-9 {
                    continue;
                }
                // (a step up needs the head room over the cat where it stands; a drop needs the
                // column over the landing clear down from the cat's level)
                let clear = |q: BlockPos, f: f64, h: f64| w.collision_shapes(Aabb::new(q.x as f64 + 0.2, f + 1e-6, q.z as f64 + 0.2, q.x as f64 + 0.8, f + h, q.z as f64 + 0.8)).is_empty();
                if dy == 1 && !clear(c, fc, 1.7) {
                    continue;
                }
                if dy < 0 && !clear(BlockPos::new(n.x, c.y, n.z), fnb, (fc - fnb) + 0.7) {
                    continue;
                }
                reached.insert(n, fnb);
                queue.push_back(n);
                break;
            }
        }
    }
    // (G1 hole 10: the riders from the SETTLED world's rails; hole 9: the villager's eye from
    // `eye_of`, the shared function)
    let mut rails: Vec<(BlockPos, String)> = settled_rails(w, lo, hi);
    rails.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    let veye = eye_of(redsim::entity::mob::types::MobType::Villager);
    let eyes: Vec<(BlockPos, Vec3)> = rails.iter().map(|(c, s)| { let p = along_rail(*c, s, 0.5); (*c, Vec3::new(p.x, p.y + veye, p.z)) }).collect();
    let mut hits: Vec<(BlockPos, BlockPos, f64)> = Vec::new();
    let mut cells: Vec<(&BlockPos, &f64)> = reached.iter().collect();
    cells.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    for (c, f) in cells {
        let ce = Vec3::new(c.x as f64 + 0.5, f + 0.35, c.z as f64 + 0.5);
        if let Some((rc, d)) = eyes.iter().filter_map(|(rc, e)| { let d = ((e.x - ce.x).powi(2) + (e.y - ce.y).powi(2) + (e.z - ce.z).powi(2)).sqrt(); (d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, *e, ce)).then_some((*rc, d)) }).min_by(|a, b| a.1.total_cmp(&b.1)) {
            hits.push((*c, rc, d));
        }
    }
    let top = reached.keys().filter(|c| c.y > ground_y + 1).map(|c| c.y).max();
    let mut lines = vec![format!(
        "cat reach (ironspec Nov 13 03:32 (3): a cat 0.6 x 0.7, eye 0.35; steps up 1, drops up to 3; sight eye to eye, COLLIDER): the ground plane's top at y {} around the box widened by {margin}; {seeds} ground cells; {} cells reached; the highest above the ground: {}; {} rider positions (each rail cell's cart); {} reached cells within 8 and in sight of a rider (bar 0){}",
        ground_y + 1,
        reached.len(),
        top.map(|y| format!("y {y}")).unwrap_or_else(|| "none (the ground only)".to_string()),
        eyes.len(),
        hits.len(),
        if seeds == 0 || eyes.is_empty() { " (NOT JUDGED: 0 samples)" } else if hits.is_empty() { " (PASS)" } else { " (FAIL)" }
    )];
    for (c, rc, d) in &hits {
        lines.push(format!("  IN SIGHT {},{},{} (a cat standing there) of the rider at {},{},{} ({d:.2})", c.x, c.y, c.z, rc.x, rc.y, rc.z));
    }
    Ok((lines, if seeds == 0 || eyes.is_empty() { usize::MAX } else { hits.len() }))
}

pub fn sensor_at(spec: &FarmSpec, points: &[Vec3], exclude: &[([i32; 3], [i32; 3])], sets: &[(BlockPos, redsim::block::Block)], hold: &[(BlockPos, String)], through: &[BlockPos]) -> Result<(Vec<String>, usize, usize), String> {
    use redsim::entity::mob::types::MobType;
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut held: Vec<String> = Vec::new();
    // (G1 hole 5: each override against the SETTLED module world)
    let noops = if sets.is_empty() && hold.is_empty() {
        0
    } else {
        let mut mrun = Run::start(spec, &o)?;
        settle_full(&mut mrun)?;
        override_report(&mrun.world, sets, hold, &mut held)
    };
    let mut bench = spec.clone();
    for (p, b) in sets {
        bench.blocks.retain(|(q, _)| q != p);
        if !b.is_air() {
            bench.blocks.push((*p, *b));
        }
    }
    let spec = &bench;
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    // (farmopt Nov 13 01:02: a stand-in the settle re-evaluates, an unpowered hatch stated open,
    // is held: re-placed raw after the settle as stated, and listed)
    let survival_fails = standin_survival(&mut run.world, sets, hold, &mut held) + noops;
    let w = &run.world;
    // (the hostiles: (label, id, feet, eye, the boat's post when seated in one); G1 Nov 13 05:20,
    // hole 12: every hostile the design holds is judged or FAILS as NOT JUDGED: a boat's or a
    // raft's passenger of any hostile type is read from its NBT; a type or a seat redsim doesn't
    // model (a husk's eye, a raft's seat) is never assumed)
    let mut hostiles: Vec<(String, String, Vec3, Vec3, Option<BlockPos>)> = Vec::new();
    let mut unjudged: Vec<String> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();
    // (G1 Nov 13 12:52 (e): every passenger a vehicle's NBT holds, in order (an `id:"…"` naming
    // a mob; an item's id names none), so a second rider or a rider the sensor ignores is never
    // dropped silently)
    let passengers_of = |n: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = n;
        while let Some(at) = rest.find("id:\"") {
            let tail = &rest[at + 4..];
            let Some(end) = tail.find('"') else { break };
            let id = &tail[..end];
            if MobType::from_id(id).is_some() || hostile_range(id).is_some() {
                out.push(id.to_string());
            }
            rest = &tail[end..];
        }
        out
    };
    for (k, p, nbt) in &spec.entities {
        let n = nbt.as_deref().unwrap_or("");
        let vehicle = k.ends_with("_boat") || k.ends_with("_raft");
        let riders = if vehicle { passengers_of(n) } else { Vec::new() };
        for (seat, id) in riders.iter().enumerate().filter(|(_, id)| hostile_range(id).is_none()) {
            ignored.push(format!("the {} in the {} at {:.2},{:.2},{:.2} (seat {seat}): not a hostile the villager sensor senses (ignored)", id.trim_start_matches("minecraft:"), k.trim_start_matches("minecraft:"), p[0], p[1], p[2]));
        }
        for (seat, id) in riders.iter().enumerate().filter(|(s, id)| *s > 0 && hostile_range(id).is_some()) {
            unjudged.push(format!("NOT JUDGED the {} in the {} at {:.2},{:.2},{:.2}: seat {seat} (a second rider's seat) is not modelled (FAILS)", id.trim_start_matches("minecraft:"), k.trim_start_matches("minecraft:"), p[0], p[1], p[2]));
        }
        let rider = riders.first().cloned();
        if let Some(id) = rider.filter(|id| hostile_range(id).is_some()) {
            let short = id.trim_start_matches("minecraft:").to_string();
            let ty = match short.as_str() {
                "pillager" => Some(MobType::Pillager),
                "zombie" => Some(MobType::Zombie),
                _ => None,
            };
            match ty {
                Some(ty) if k.ends_with("_boat") => {
                    let pill = ty == MobType::Pillager;
                    let feet = Vec3::new(p[0], p[1] + if pill { PILLAGER_SEAT } else { zombie_seat() }, p[2]);
                    let eye = Vec3::new(feet.x, feet.y + eye_of(ty), feet.z);
                    hostiles.push((format!("the {short} in the boat at {:.2},{:.2},{:.2}", p[0], p[1], p[2]), id.clone(), feet, eye, Some(BlockPos::new(p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32))));
                }
                _ => unjudged.push(format!(
                    "NOT JUDGED the {short} in the {} at {:.2},{:.2},{:.2}: {} (FAILS)",
                    k.trim_start_matches("minecraft:"),
                    p[0],
                    p[1],
                    p[2],
                    if ty.is_none() { "its eye height is not in redsim's data" } else { "a raft's seat is not modelled" }
                )),
            }
        } else if hostile_range(k).is_some() {
            let ty = match k.trim_start_matches("minecraft:") {
                "pillager" => Some(MobType::Pillager),
                "zombie" => Some(MobType::Zombie),
                _ => None,
            };
            let Some(ty) = ty else {
                unjudged.push(format!("NOT JUDGED the {k} at {:.2},{:.2},{:.2}: its eye height is not in redsim's data (FAILS)", p[0], p[1], p[2]));
                continue;
            };
            let feet = Vec3::new(p[0], p[1], p[2]);
            hostiles.push((format!("the {} standing at {:.2},{:.2},{:.2}", k.trim_start_matches("minecraft:"), p[0], p[1], p[2]), k.clone(), feet, Vec3::new(feet.x, feet.y + eye_of(ty), feet.z), None));
        }
    }
    let d = |a: &Vec3, b: &Vec3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let veye = eye_of(MobType::Villager);
    let mut lines = vec![format!(
        "the hostile sensor (VillagerHostilesSensor over NEAREST_VISIBLE_LIVING_ENTITIES, 26.3; ranges: zombie, drowned, husk, zombie villager, vex 8, vindicator, zoglin 10, evoker, illusioner, ravager 12, pillager 15; sight eye to eye, a COLLIDER clip; the villager's eye {veye:.4} over its feet): {} feet points, {} hostiles in the design",
        points.len(),
        hostiles.len()
    )];
    let mut sensed: Vec<usize> = Vec::new();
    lines.extend(unjudged.iter().map(|u| format!("  {u}")));
    lines.extend(ignored.iter().map(|u| format!("  {u}")));
    for (i, (label, id, feet, eye, _)) in hostiles.iter().enumerate() {
        let range = hostile_range(id).unwrap_or(0.0);
        let mut rows: Vec<String> = Vec::new();
        let mut any = false;
        for p in points {
            let dist = d(p, feet);
            let e = Vec3::new(p.x, p.y + veye, p.z);
            let seen = redsim::entity::mob::los::has_line_of_sight(w, e, *eye);
            let inside = dist <= range;
            if inside && seen {
                any = true;
            }
            // (ironspec Nov 13 06:50 real: per entrant, NearestLivingEntitySensor's 16-block scan
            // (the villager's box inflated by 16, the hostile's box meeting it; TargetingConditions'
            // range 16), the per-type distance and the COLLIDER clip with its blocking cell)
            let in_scan = (feet.x - p.x).abs() <= 16.3 + 0.3 && feet.y >= p.y - 16.0 - 1.95 && feet.y <= p.y + 1.95 + 16.0 && (feet.z - p.z).abs() <= 16.3 + 0.3 && dist <= 16.0;
            let blocked = if seen {
                String::new()
            } else {
                let hit = redsim::entity::mob::los::clip_collider(w, e, *eye);
                if hit.miss { " (no collider hit: beyond 128)".to_string() } else { format!(" at {},{},{} ({})", hit.pos.x, hit.pos.y, hit.pos.z, w.get(hit.pos).to_string().trim_start_matches("minecraft:")) }
            };
            rows.push(format!(
                "{:.3},{:.4},{:.3}: {dist:.2}{}{} sight {}{blocked}",
                p.x,
                p.y,
                p.z,
                if in_scan { " in the 16 scan," } else { " OUTSIDE the 16 scan," },
                if inside { " IN RANGE," } else { " out of range," },
                if seen { "CLEAR" } else { "blocked" }
            ));
        }
        if any {
            sensed.push(i);
        }
        // (tick-exact Nov 13 07:50 / 08:26, G1 07:51 / 08:22: a mob burns when the EYE CELL's sky
        // light is 15 (canSeeSky: sky brightness >= 15, not geometric); glass and iron trapdoors
        // don't cut it; the bar < 15. The sun-burn set is EXACTLY the entity-type tag
        // #minecraft:burn_in_daylight (Mob.aiStep 456-460, the tag the only switch): skeleton,
        // stray, wither_skeleton (tagged; its fire immunity uncited), bogged, zombie, zombie_horse,
        // zombie_villager, drowned, zombie_nautilus, phantom; a known type outside it says so; an
        // unknown type is an error)
        let name = id.trim_start_matches("minecraft:");
        const BURN_IN_DAYLIGHT: [&str; 10] = ["skeleton", "stray", "wither_skeleton", "bogged", "zombie", "zombie_horse", "zombie_villager", "drowned", "zombie_nautilus", "phantom"];
        const KNOWN_SAFE: [&str; 22] = ["pillager", "vindicator", "evoker", "illusioner", "ravager", "vex", "husk", "zoglin", "witch", "creeper", "spider", "cave_spider", "zombified_piglin", "piglin", "piglin_brute", "hoglin", "blaze", "ghast", "enderman", "silverfish", "breeze", "warden"];
        let undead = BURN_IN_DAYLIGHT.contains(&name);
        if !undead && !KNOWN_SAFE.contains(&name) {
            return Err(format!("the sunburn census: {id} is neither in #minecraft:burn_in_daylight nor a known type outside it (an unknown type is an error)"));
        }
        let tag_note = if name == "wither_skeleton" { " (tagged burn_in_daylight; its fire immunity uncited)" } else { "" };
        let ec = BlockPos::new(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
        let sky_line = match redsim::daylight::sky_light(w, ec) {
            Ok(l) => {
                let above: Vec<String> = ((ec.y + 1)..=(spec.hi.y + 1)).filter_map(|y| {
                    let b = w.get(BlockPos::new(ec.x, y, ec.z));
                    (!b.is_air()).then(|| format!("{} at y {y}", b.to_string().trim_start_matches("minecraft:")))
                }).collect();
                format!(
                    "the eye cell {},{},{} sky light {l}{}{tag_note}; the column above: {}",
                    ec.x,
                    ec.y,
                    ec.z,
                    if !undead { " (not in #minecraft:burn_in_daylight: does not burn)" } else if l >= 15 { ": BURNS by day (FAILS the bar < 15)" } else { ": under cover (passes the bar < 15)" },
                    if above.is_empty() { "open sky".to_string() } else { above.join(", ") }
                )
            }
            Err(e) => format!("the eye cell {},{},{} sky light UNKNOWN ({e}){}", ec.x, ec.y, ec.z, if undead { ": FAILS the bar (unknown is never read as covered)" } else { "" }),
        };
        lines.push(format!("  {} {label} (range {range}; {sky_line}): {}", if any { "IN NEAREST_HOSTILE" } else { "not sensed" }, rows.join("; ")));
    }
    lines[0].push_str(&format!(": {} sensed at some point", sensed.len()));
    for h in &held {
        lines.push(format!("  {h}"));
    }
    // (ironspec Nov 13 01:03 (3): each ray's entry and exit through a named cell, with the cell's
    // state in the world judged and each eye)
    for c in through {
        let st = w.get(*c);
        lines.push(format!("  through {},{},{} ({}; collision boxes {:?} in 1/16):", c.x, c.y, c.z, st.to_string().trim_start_matches("minecraft:"), redsim::behavior::collision_boxes(&st)));
        for (label, _, feet, eye, _) in &hostiles {
            for p in points {
                if d(p, feet) > 16.0 {
                    continue;
                }
                let e = Vec3::new(p.x, p.y + veye, p.z);
                let (lo, hi) = ([c.x as f64, c.y as f64, c.z as f64], [(c.x + 1) as f64, (c.y + 1) as f64, (c.z + 1) as f64]);
                let (a, dv) = ([e.x, e.y, e.z], [eye.x - e.x, eye.y - e.y, eye.z - e.z]);
                let (mut t0, mut t1) = (0.0f64, 1.0f64);
                let mut hit = true;
                for k in 0..3 {
                    if dv[k].abs() < 1e-12 {
                        if a[k] < lo[k] || a[k] > hi[k] {
                            hit = false;
                        }
                        continue;
                    }
                    let (u, v) = ((lo[k] - a[k]) / dv[k], (hi[k] - a[k]) / dv[k]);
                    t0 = t0.max(u.min(v));
                    t1 = t1.min(u.max(v));
                }
                if !hit || t0 > t1 {
                    continue;
                }
                let at = |t: f64| format!("{:.3},{:.3},{:.3}", a[0] + dv[0] * t, a[1] + dv[1] * t, a[2] + dv[2] * t);
                lines.push(format!(
                    "    eye {:.4},{:.4},{:.4} -> {label}'s eye {:.4},{:.4},{:.4}: enters at {}, leaves at {}; sight {}",
                    e.x,
                    e.y,
                    e.z,
                    eye.x,
                    eye.y,
                    eye.z,
                    at(t0),
                    at(t1),
                    if redsim::entity::mob::los::has_line_of_sight(w, e, *eye) { "CLEAR" } else { "blocked" }
                ));
            }
        }
    }
    for i in sensed {
        let (label, id, _, _, post) = &hostiles[i];
        match post {
            Some(post) if sets.is_empty() => {
                for l in occlude(spec, *post, id.ends_with("pillager"), points, exclude, &[])? {
                    lines.push(format!("  {l}"));
                }
            }
            Some(_) => lines.push(format!("  occluder for {label}: not searched with stand-ins (the search builds the design as stored)")),
            None => lines.push(format!("  occluder for {label}: not searched (not seated in a boat: the occluder search keeps a boat mob's covered rays)")),
        }
    }
    Ok((lines, points.len() * hostiles.len(), survival_fails + unjudged.len()))
}

pub fn occlude(spec: &FarmSpec, post: BlockPos, pill: bool, hide: &[Vec3], exclude: &[([i32; 3], [i32; 3])], extra: &[Vec3]) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &mut run.world;
    let glass = redsim::block::Block::parse("minecraft:glass")?;
    let air_block = redsim::block::Block::parse("minecraft:air")?;
    let seat = zombie_seat();
    let feet = Vec3::new(post.x as f64 + 0.5, post.y as f64 + if pill { PILLAGER_SEAT } else { seat }, post.z as f64 + 0.5);
    // (ironspec Nov 12 23:30: the shared eye heights)
    let eye = Vec3::new(feet.x, feet.y + eye_of(if pill { redsim::entity::mob::types::MobType::Pillager } else { redsim::entity::mob::types::MobType::Zombie }), feet.z);
    // (G1 Nov 13, N9: the hide points are judged at the sensor's own range, zombie 8 / pillager 15,
    // so a point at 7.95 < d <= 8 is occluded or reported too)
    let far = if pill { 15.0 } else { 8.0 };
    let veye = eye_of(redsim::entity::mob::types::MobType::Villager);
    let reye = |p: &Vec3| Vec3::new(p.x, p.y + veye, p.z);
    let d = |p: &Vec3| ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
    let sees = |w: &redsim::World, p: &Vec3| d(p) <= far && redsim::entity::mob::los::has_line_of_sight(w, reye(p), eye);
    let in_range = |p: &Vec3| d(p) <= far;
    // (ironspec Nov 13 19:15, B1: a hide point enters the search when EITHER direction is clear: the
    // design depends on the hostile's ray for a hide point, so the rider's ray alone can't gate it)
    let hidden_now: Vec<Vec3> = hide.iter().copied().filter(|p| in_range(p) && { let (a, b) = ray_pair(w, reye(p), eye); a || b }).collect();
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
        lines.push(format!("  NONE: no free cell on the ray to {:.2},{:.2},{:.2} that spares every kept ray (FAIL)", p.x, p.y, p.z));
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
    // (G1 Nov 13, N9: the glass laid with its neighbour updates (a pane or a fence beside it changes
    // shape), the re-check reading the updated world)
    for c in &chosen {
        w.set_block(*c, glass.clone(), redsim::world::flags::ALL);
    }
    // (G1 Nov 13 18:37, B1: the hostile's sight of a hide point counts too: a point stays seen while
    // EITHER direction is clear (the rider's sight of the hostile, the villager sensor's test, or the
    // hostile's sight of the rider, its targeting / charge test); a kept point is lost when the rider's
    // sight of the hostile is blocked)
    let still: Vec<&Vec3> = hidden_now.iter().filter(|p| in_range(p) && { let (a, b) = ray_pair(w, reye(p), eye); a || b }).collect();
    let lost: Vec<&String> = keep.iter().filter(|(_, p)| !sees(w, p)).map(|(l, _)| l).collect();
    for c in &chosen {
        w.set_block(*c, air_block.clone(), redsim::world::flags::ALL);
    }
    lines.push(format!(
        "  glass {}: {} ({} hide positions still seen; {} kept points lost{}){}",
        chosen.len(),
        chosen.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect::<Vec<_>>().join(" "),
        still.len(),
        lost.len(),
        if lost.is_empty() { String::new() } else { format!(": {}", lost.iter().take(6).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) },
        if still.is_empty() && lost.is_empty() { " (PASS)" } else { " (FAIL)" }
    ));
    // (G1 Nov 13 17:26 / 18:37, B1 / B8 / B9: both rays BY THE COLLIDER, each direction its own
    // `has_line_of_sight`: per glass laid ALONE the hide and the kept rays it turns from clear to
    // blocked in either direction (counted, not asserted); then every point before and after the
    // whole set; every pair whose two directions disagree named (the game's block walk breaks exact
    // ties by step order); these lines carry no "glass " prefix, so `occlude_failed` reads only the
    // verdict line above)
    let fmt = |p: &Vec3| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z);
    let before_hide: Vec<(bool, bool)> = hidden_now.iter().map(|p| ray_pair(w, reye(p), eye)).collect();
    let before_keep: Vec<(bool, bool)> = keep.iter().map(|(_, p)| ray_pair(w, reye(p), eye)).collect();
    let turned = |b0: (bool, bool), b1: (bool, bool)| (b0.0 && !b1.0) || (b0.1 && !b1.1);
    for c in &chosen {
        w.set_block(*c, glass.clone(), redsim::world::flags::ALL);
        let cut: Vec<String> = hidden_now.iter().zip(&before_hide).filter(|(p, b0)| turned(**b0, ray_pair(w, reye(p), eye))).map(|(p, _)| fmt(p)).collect();
        let crossed: Vec<&String> = keep.iter().zip(&before_keep).filter(|((_, p), b0)| turned(**b0, ray_pair(w, reye(p), eye))).map(|((l, _), _)| l).collect();
        w.set_block(*c, air_block.clone(), redsim::world::flags::ALL);
        lines.push(format!(
            "    rays cut by {},{},{} laid alone (both directions, the collider): {} hide ray(s){}; {} kept ray(s) cut{}",
            c.x,
            c.y,
            c.z,
            cut.len(),
            if cut.is_empty() { String::new() } else { format!(" (to {})", cut.join(" ")) },
            crossed.len(),
            if crossed.is_empty() { String::new() } else { format!(" ({})", crossed.iter().take(8).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")) }
        ));
    }
    for c in &chosen {
        w.set_block(*c, glass.clone(), redsim::world::flags::ALL);
    }
    let mut disagree = Vec::new();
    for (p, b0) in hidden_now.iter().zip(&before_hide) {
        let b1 = ray_pair(w, reye(p), eye);
        for (when, b) in [("before", *b0), ("with the glass", b1)] {
            if b.0 != b.1 {
                disagree.push(format!("hide point {} {when}: {} ({})", fmt(p), ray_tag(b), if b.1 { "the hostile's ray, the one the design depends on here (targeting), is CLEAR: a charge is possible" } else { "the hostile's ray, the one the design depends on here (targeting), is blocked: no charge, though the rider's ray is clear" }));
            }
        }
        lines.push(format!("    hide point {} ({:.2} away; the design depends on the hostile's ray, targeting: it must be blocked): before {}; with the glass {}", fmt(p), d(p), ray_tag(*b0), ray_tag(b1)));
    }
    for ((l, p), b0) in keep.iter().zip(&before_keep) {
        let b1 = ray_pair(w, reye(p), eye);
        for (when, b) in [("before", *b0), ("with the glass", b1)] {
            if b.0 != b.1 {
                disagree.push(format!("kept point {l} {when}: {} ({})", ray_tag(b), if b.0 { "the scare holds: the rider's ray, the one the design depends on here, is clear; the hostile's ray is blocked" } else { "the scare FAILS: the rider's ray, the one the design depends on here, is blocked, though the hostile sees the rider" }));
            }
        }
        lines.push(format!("    kept point {l} ({:.2} away; the design depends on the rider's ray, the scare: it must stay clear): before {}; with the glass {}", d(p), ray_tag(*b0), ray_tag(b1)));
    }
    for c in &chosen {
        w.set_block(*c, air_block.clone(), redsim::world::flags::ALL);
    }
    lines.push(format!("    pairs whose two directions disagree: {}{}", disagree.len(), if disagree.is_empty() { String::new() } else { format!(": {}", disagree.join("; ")) }));
    Ok(lines)
}

/// (G1 Nov 13 18:37, B1) Both collider rays between a rider's eye and a hostile's eye: (the rider's
/// sight of the hostile, the villager sensor's test; the hostile's sight of the rider, its targeting
/// and charge test). They can differ: the game's block walk breaks exact ties by step order.
pub fn ray_pair(w: &redsim::World, rider_eye: Vec3, hostile_eye: Vec3) -> (bool, bool) {
    (redsim::entity::mob::los::has_line_of_sight(w, rider_eye, hostile_eye), redsim::entity::mob::los::has_line_of_sight(w, hostile_eye, rider_eye))
}

/// A [`ray_pair`] in words; a pair whose directions differ reads DISAGREE.
pub fn ray_tag(b: (bool, bool)) -> &'static str {
    match b {
        (true, true) => "both CLEAR",
        (false, false) => "both blocked",
        (true, false) => "rider sees the hostile, the hostile's ray blocked (DISAGREE)",
        (false, true) => "the hostile sees the rider, the rider's ray blocked (DISAGREE)",
    }
}

/// (ironspec Nov 13 06:00 real, EXIT) Whether an occluder search's lines report a failure: no
/// occluder found (NONE), hide positions still seen, or kept points lost.
pub fn occlude_failed(lines: &[String]) -> bool {
    lines.iter().any(|l| l.trim_start().starts_with("NONE:") || (l.trim_start().starts_with("glass ") && l.ends_with("(FAIL)")))
}

pub fn scarer_check(spec: &FarmSpec, extra: &[Vec3], posts: &[(BlockPos, bool)], transient: &[String], golem_pad: Option<([i32; 3], [i32; 3])>, columns: &[[f64; 4]]) -> Result<Vec<String>, String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &run.world;
    let pts = scarer_points(spec, extra);
    let seat = zombie_seat();
    let mut covered = vec![false; pts.len()];
    let mut lines = Vec::new();
    let mut refused = 0usize;
    for (c, pill) in posts {
        let (near, far) = if *pill { (8.5, 15.0) } else { (FLEE_BAR, PANIC_BAR) };
        let feet = Vec3::new(c.x as f64 + 0.5, c.y as f64 + if *pill { PILLAGER_SEAT } else { seat }, c.z as f64 + 0.5);
        let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { zeye() }, feet.z);
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
            // (G1 Nov 13, N29: the hidden times' seeds are every position the zombie's SENSOR sees,
            // within its 8, not the 7.95 design bar)
            if !*pill && d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + veye(), p.z), eye) {
                seen8.push(*p);
            }
            if d > far {
                continue;
            }
            let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + veye(), p.z), eye);
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
                    let blocker = ray_cells(Vec3::new(p.x, p.y + veye(), p.z), eye).into_iter().find(|q| {
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
            match t {
                Some(t) if *t < HIDDEN_BAR_GT => bad.push(format!("{s}, only {t:.1} gt after the last position it sees within 8 (bar {HIDDEN_BAR_GT} gt, inherited: G1's CHANGES 2 on fee712f72, no measured failure)")),
                Some(_) => {}
                // (G1 Nov 13, N30: a hidden position no seen one reaches within 400 gt is printed,
                // never passed silently: no NEAREST_HOSTILE of this zombie can be stale there)
                None => lines.push(format!("    hidden {s}: no position it sees reaches it along the track within 400 gt (no stale memory possible; printed)")),
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
                if redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + veye(), p.z), eye) {
                    dense_least = dense_least.min(d);
                    if d <= 8.0 {
                        match within.iter_mut().find(|(l, _)| *l == lab) {
                            Some((_, len)) => *len += step,
                            None => within.push((lab, step)),
                        }
                    }
                }
            }
            // (G1 Nov 13, N30: the crossbow rule APPLIED: a rider resting within 8 in its sight is
            // seen for 5 ticks and more (seeTime >= 5: it charges and shoots), so any position of
            // the riders' paths it sees within 8 refuses the post)
            if !within.is_empty() {
                refused += 1;
            }
            lines.push(format!(
                "    pillager {},{},{} over the riders' paths (dense): the nearest position it sees {}; seen within its crossbow's 8: {}{}",
                c.x,
                c.y,
                c.z,
                if dense_least == f64::MAX { "none".to_string() } else { format!("{dense_least:.2}") },
                if within.is_empty() { "none (no tick of any rider within 8 in its sight)".to_string() } else { within.iter().map(|(l, len)| format!("{l} for {len:.2} blocks of path")).collect::<Vec<_>>().join(", ") },
                if within.is_empty() { "" } else { "; REFUSED (the crossbow: a rider resting there is seen 5 ticks and more)" }
            ));
        }
        // (the pad golems that see this post within their 16 follow range: 0 required, G1)
        let mut seers: Vec<String> = Vec::new();
        if let Some((glo, ghi)) = golem_pad {
            let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { zeye() }, feet.z);
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
    settle_full(&mut run)?;
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
                let eye = Vec3::new(feet.x, feet.y + if pill { PILLAGER_EYE } else { zeye() }, feet.z);
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
                        let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + veye(), p.z), eye);
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
                                    let pt_eye = |k: usize| Vec3::new(pts[k].1.x, pts[k].1.y + veye(), pts[k].1.z);
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
                let eye = Vec3::new(feet.x, feet.y + if *pill { PILLAGER_EYE } else { zeye() }, feet.z);
                for (k, (_, p)) in pts.iter().enumerate() {
                    let d = ((p.x - feet.x).powi(2) + (p.y - feet.y).powi(2) + (p.z - feet.z).powi(2)).sqrt();
                    if !*pill && d < nb {
                        ok = false;
                        break 'post;
                    }
                    if d > fb {
                        continue;
                    }
                    let seen = redsim::entity::mob::los::has_line_of_sight(w, Vec3::new(p.x, p.y + veye(), p.z), eye);
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
    settle_full(&mut run)?;
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
    // (G1 Nov 13, N10: no standable pad cell is NOT JUDGED, a failure)
    if tried == 0 {
        return Err(format!("golem paths to {},{},{}: the pad x {}..{} y {}..{} z {}..{} has no standable cell (NOT JUDGED)", target.x, target.y, target.z, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]));
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
    settle_full(&mut run)?;
    let boat = redsim::entity::spawn(&mut run.world, "minecraft:oak_boat", [cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}"))?;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    let mut seat_dy = (f64::MAX, f64::MIN);
    for _ in 0..ticks {
        run.tick();
        let w = &run.world;
        let Some(bi) = redsim::entity::index_of(w, boat) else { return Err(format!("post drift at {},{},{}: the boat is gone (G1 N10: a failure, never an exit-0 line)", cell.x, cell.y, cell.z)) };
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
        "what can clear a stamp, per place (design numbers, not a bar; the oldest-count rows decide; G1 N21: the DEFAULT model's places, each rail cell's three points with no tail, not clear-all's position model): {} panicking places (a hostile seen within its range: a zombie 8, a pillager 15) with a reachable panic-stroll cell, {} with none; {} calm places whose calm stroll can pick a reachable cell, {} with none",
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

fn report_body(places: &[Place], jobs: &[(BlockPos, String)], show: usize, classes: &PlaceClasses) -> (Vec<String>, Vec<String>) {
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
    // (ironspec Nov 12 21:49 (4): the design's `riderless:` cells are no rider places, dropped and
    // printed as such; its `waker-places:` are a declared exception, printed apart, not failing)
    let dropped: Vec<&Place> = places.iter().filter(|p| classes.riderless(&p.cell)).collect();
    let wakers: Vec<&Place> = places.iter().filter(|p| !classes.riderless(&p.cell) && classes.waker(&p.cell).is_some()).collect();
    let kept: Vec<Place> = places.iter().filter(|p| !classes.riderless(&p.cell) && classes.waker(&p.cell).is_none()).cloned().collect();
    let places: &[Place] = &kept;
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
        let sight = match p.near_seen {
            Some(d) => format!("in sight at {d:.2}: FAIL"),
            None => "out of sight (robust hiding): information, not failing".to_string(),
        };
        lines.push(format!("  INSIDE {},{},{}{}: the nearest zombie {lo:.2}..{hi:.2} (at {:.2},{:.2},{:.2}), {sight}", p.cell.x, p.cell.y, p.cell.z, if p.on_foot { " on foot" } else { "" }, h.x, h.y, h.z));
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
    // (ironspec Nov 12 21:58: the gate is the sight-gated verdict, the flee needing sight; a place
    // inside the bar out of sight under the robust hiding is printed as information)
    let inside_seen = inside.iter().filter(|p| p.near_seen.is_some()).count();
    lines.push(format!(
        "the 6-8 gate (sight-gated, ironspec Nov 12 21:58: a place inside {FLEE_BAR} of a zombie it sees or is hidden from only by partial blocks, or a declared panic place beyond {PANIC_BAR} or out of sight; {} inside by distance only, out of sight: information): {}",
        inside.len() - inside_seen,
        verdict(inside_seen + declared_bad.len())
    ));
    if !dropped.is_empty() || !wakers.is_empty() {
        lines.push(format!(
            "the 6-8 rule's classes (ironspec Nov 12 21:49 (4)): {} riderless places dropped (no rider there: {}){}; {} declared waker places apart (not failing)",
            dropped.len(),
            classes.riderless.first().map(|(_, w)| w.as_str()).unwrap_or(""),
            if dropped.is_empty() { String::new() } else { format!(": {}", dropped.iter().map(|p| format!("{},{},{}", p.cell.x, p.cell.y, p.cell.z)).collect::<Vec<_>>().join(" ")) },
            wakers.len()
        ));
        for p in &wakers {
            lines.push(format!(
                "  WAKER {},{},{}{}: the nearest zombie {}{} (declared waker place: {})",
                p.cell.x,
                p.cell.y,
                p.cell.z,
                if p.on_foot { " on foot" } else { "" },
                p.near.map_or("none".to_string(), |(lo, hi, _)| format!("{lo:.2}..{hi:.2}")),
                p.near_seen.map_or(String::new(), |d| format!(", in sight at {d:.2}")),
                classes.waker(&p.cell).unwrap_or("")
            ));
        }
    }
    lines.push(format!(
        "verdicts (every cell a source can pick reachable wherever it runs): flee {}, panic stroll {}, idle stroll REPORT-ONLY (by ruling 06:46): {stroll_bad} places, walks to villagers REPORT-ONLY (by ruling 06:46): {others_bad} places (the places with an unreached candidate; the bar behind both is the runs' oldest stamp under 1,000 gt)",
        verdict(flee_bad),
        verdict(panic_bad)
    ));
    // (ironspec Nov 13 06:00 real, EXIT: every failing bar reaches the run's exit through one list)
    let mut fails: Vec<String> = Vec::new();
    if inside_seen + declared_bad.len() > 0 {
        fails.push(format!("the 6-8 gate ({} inside in sight, {} PANIC-UNCOVERED)", inside_seen, declared_bad.len()));
    }
    if flee_bad > 0 {
        fails.push(format!("flee ({flee_bad} places)"));
    }
    if panic_bad > 0 {
        fails.push(format!("panic stroll ({panic_bad} places)"));
    }
    (lines, fails)
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
/// (ironspec Nov 13 06:11 platform: EVERY position a rider can occupy at night is under a scarer's
/// sight; farmopt 06:31: the coverage map over the whole night circuit) For every rider position
/// (each rail cell every 0.1 along the cart's path, a slope's 0.97 too, and each column every 0.25),
/// the sensor's own test against every hostile the design seats: NEAREST_VISIBLE_LIVING_ENTITIES'
/// 16-block scan, the per-type distance (zombie 8, pillager 15) and the eye-to-eye COLLIDER clip
/// from the rider's REAL eye (its feet, a seated villager at its cart's position, + 1.62). Per rail
/// cell: covered / dark positions; for each dark position the nearest hostile in range, the eye
/// used and the blocking cell. Settled world, the design's zombies summoned (N5), its pillager
/// boats seated (redsim spawns none). Returns the lines and the count of dark positions.
/// (ironspec Nov 13 08:26, day-1's fail-safe answer) Every rider position within `radius` of a
/// cell's centre (the bed head: a REST villager within 16 of an occupied bed is the exposure bar's
/// subject): each rail cell every 0.1 along it (a slope's 0.97 too) and each `column` every 0.25,
/// the rider's feet, grouped by cell with the least and greatest distance.
pub fn near_positions(spec: &FarmSpec, columns: &[[f64; 4]], center: BlockPos, radius: f64) -> Vec<String> {
    let c = Vec3::new(center.x as f64 + 0.5, center.y as f64 + 0.5, center.z as f64 + 0.5);
    let d = |p: &Vec3| ((p.x - c.x).powi(2) + (p.y - c.y).powi(2) + (p.z - c.z).powi(2)).sqrt();
    let mut by: std::collections::BTreeMap<(i32, i32, i32), (String, usize, f64, f64)> = std::collections::BTreeMap::new();
    for (cell, _, p) in rider_positions(spec) {
        let dd = d(&p);
        if dd <= radius {
            let e = by.entry((cell.x, cell.y, cell.z)).or_insert(("rail".into(), 0, f64::INFINITY, 0.0));
            e.1 += 1;
            e.2 = e.2.min(dd);
            e.3 = e.3.max(dd);
        }
    }
    for col in columns {
        let n = ((col[3] - col[1]).abs() / 0.25).round() as usize;
        for i in 0..=n {
            let y = col[1] + if col[3] >= col[1] { 1.0 } else { -1.0 } * i as f64 * 0.25;
            let p = Vec3::new(col[0], y, col[2]);
            let dd = d(&p);
            if dd <= radius {
                let e = by.entry((p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)).or_insert(("column".into(), 0, f64::INFINITY, 0.0));
                e.1 += 1;
                e.2 = e.2.min(dd);
                e.3 = e.3.max(dd);
            }
        }
    }
    let total: usize = by.values().map(|v| v.1).sum();
    let mut lines = vec![format!(
        "rider positions within {radius} of {},{},{}'s centre (ironspec Nov 13 08:26: the exposure bar's subject; each rail cell every 0.1, each column every 0.25, the rider's feet): {total} positions in {} cells",
        center.x,
        center.y,
        center.z,
        by.len()
    )];
    for ((x, y, z), (kind, n, lo, hi)) in &by {
        lines.push(format!("  {kind} {x},{y},{z}: {n} positions, {lo:.2}..{hi:.2}"));
    }
    lines
}

/// (ironspec Nov 13 08:18 (c) / 08:33 (2), the floating site's exterior spawns) The design's
/// settled world in the void (redsim reads AIR outside the world box: a floating site), for the
/// hostile checks run on it rather than in a terrain sample.
pub fn settled_world_void(spec: &FarmSpec) -> Result<redsim::World, String> {
    let mut run = Run::start(spec, &RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) })?;
    settle_full(&mut run)?;
    Ok(run.world)
}

/// (ironspec Nov 13 08:33 (2); 9fc3155f's correction 10:35) The hostile checks (`hostile::check`) on
/// the FLOATING site: the design's own settled world in the void (redsim reads AIR outside the box),
/// the AFK player at the run's default (60 under the chest) as the natural spawner's player, and
/// the SITE BIOME set (plains: the terrain runs' level biome outside their sample) so the spawn
/// lists are the site's. Without a biome every spawn list is empty and C1 ("can spawn inside the
/// farm") could never fire: a vacuous pass (the 10:33 rows' fault). Returns the AFK spot, the farm
/// and the exposures.
pub fn void_exposures(spec: &FarmSpec, biome: &'static str) -> Result<(Vec3, redsim::entity::mob::hostile::Farm, Vec<redsim::entity::mob::hostile::Exposure>), String> {
    // (ironspec Nov 13 10:38: plains AND ocean / river: the biome named, refused when unknown)
    if redsim::terrain::biome::biome(biome).is_none() {
        return Err(format!("the site biome {biome}: no such biome in redsim's table"));
    }
    let mut w = settled_world_void(spec)?;
    let afk = Vec3::new(spec.output.x as f64 + 0.5, (spec.lo.y - 60) as f64, spec.output.z as f64 + 0.5);
    use redsim::entity::mob::natural::{Difficulty, Natural};
    w.mob.natural = Some(Natural { void_biome: Some(biome), ..Natural::new(vec![afk], Difficulty::Hard) });
    let summoners = redsim::entity::mob::site::villager_cells(&with_design_entities(&w, spec, "the void exposures")?);
    let farm = circuit::mobfarm::hostile_farm(&w, spec, afk, &summoners);
    // (a light read that failed refuses the exposures: cc767b78's light series, ironspec 16:22's list)
    let ex = redsim::entity::mob::hostile::check(&w, &farm, true)?;
    Ok((afk, farm, ex))
}

/// (G1 Nov 13 20:02, E7; ironspec 20:12: nothing silently dropped) The design's entities spawned into a
/// copy of `w`, every line required to spawn, as `world_zombies` has it: a line redsim refuses errs by
/// name, so no villager silently leaves a bar-0 check.
pub fn with_design_entities(w: &redsim::World, spec: &FarmSpec, what: &str) -> Result<redsim::World, String> {
    let mut with = w.clone();
    for (k, p, n) in &spec.entities {
        redsim::entity::spawn(&mut with, k, *p, n.as_deref()).map_err(|e| format!("{what}: NOT JUDGED: summoning the design's {k} at {:.2},{:.2},{:.2}: {e}", p[0], p[1], p[2]))?;
    }
    Ok(with)
}

/// (G1 Nov 13 21:38 H12) The design's villagers' cells (its summoners) in `w`, every entity line required
/// to spawn (`with_design_entities`): the exposures' two CLI sites (the terrain branch of `exposures` and
/// `exposures --against`) read their summoners through it.
pub fn design_summoners(w: &redsim::World, spec: &FarmSpec, what: &str) -> Result<Vec<BlockPos>, String> {
    Ok(redsim::entity::mob::site::villager_cells(&with_design_entities(w, spec, what)?))
}

/// (ironspec Nov 13 17:26, STANDING; G1 17:20 / 18:37) The golem floors any rider of the route could
/// summon onto: every cell a seated villager's `blockPosition()` can take on a rail of the SETTLED
/// design (a rider's feet are its cart's position, `along_rail` every 0.1: its rail's cell, or the
/// cell above near a slope's top; golem guards aside), each caller's own search
/// (`golem_spawn_cells_from`: the game's ±8 / ±6 columns onto a valid floor, `checkSpawnObstruction`),
/// each golem cell ON THE PAD when a golem standing there meets a nether portal block
/// ([`golem_box_meets_portal`]: it crosses at once) and OFF THE PAD otherwise, each off-pad floor with
/// the callers whose own search yields it. The bar: 0 off the pad. No caller, or no golem cell, is
/// NOT JUDGED (an error by name: zero samples pass nothing). Returns the lines, the off-pad count and
/// every golem cell.
pub fn route_golem_floors(spec: &FarmSpec, biome: &'static str) -> Result<(Vec<String>, usize, Vec<BlockPos>), String> {
    if redsim::terrain::biome::biome(biome).is_none() {
        return Err(format!("the site biome {biome}: no such biome in redsim's table"));
    }
    let mut w = settled_world_void(spec)?;
    let afk = Vec3::new(spec.output.x as f64 + 0.5, (spec.lo.y - 60) as f64, spec.output.z as f64 + 0.5);
    use redsim::entity::mob::natural::{Difficulty, Natural};
    w.mob.natural = Some(Natural { void_biome: Some(biome), ..Natural::new(vec![afk], Difficulty::Hard) });
    // (G1 18:37, B5: the rails and the guards as the settled design holds them)
    let blk: HashMap<BlockPos, redsim::block::Block> = spec.blocks.iter().map(|(p, _)| (*p, w.get(*p))).collect();
    let rails: Vec<(BlockPos, String)> = blk.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let guards = rails.iter().filter(|(p, _)| golem_guard(&blk, *p)).count();
    // (G1 18:37, B4: a seated rider's cell along its rail, the cell above near a slope's top)
    let mut callers: Vec<BlockPos> = Vec::new();
    for (p, s) in rails.iter().filter(|(p, _)| !golem_guard(&blk, *p)) {
        // (t in [0, 1): t = 1 is the next cell's edge, its own rail's t = 0)
        for t in (0..10).map(|k| k as f64 / 10.0).chain([0.99]) {
            let f = along_rail(*p, s, t);
            let c = BlockPos::new(f.x.floor() as i32, f.y.floor() as i32, f.z.floor() as i32);
            if !callers.contains(&c) {
                callers.push(c);
            }
        }
    }
    // (G1 18:37 B4 / 19:23: and every villager the design itself holds, at its own `blockPosition()`:
    // the design's entities spawned into a copy of the settled world)
    let villagers = redsim::entity::mob::site::villager_cells(&with_design_entities(&w, spec, "the route's golem floors")?);
    for v in &villagers {
        if !callers.contains(v) {
            callers.push(*v);
        }
    }
    let nv = villagers.len();
    callers.sort_by_key(|p| (p.y, p.z, p.x));
    let cell = |p: &BlockPos| format!("{},{},{}", p.x, p.y, p.z);
    if callers.is_empty() {
        return Err(format!("the route's golem floors: NOT JUDGED: no caller ({} rail cell(s), {guards} golem guard(s) excluded)", rails.len()));
    }
    // (G1 18:37, B6: each golem cell with the callers whose own search yields it)
    let mut by: std::collections::BTreeMap<(i32, i32, i32), Vec<BlockPos>> = std::collections::BTreeMap::new();
    for s in &callers {
        for (c, r) in redsim::entity::mob::spawn::golem_spawn_cells_from(&w, *s) {
            if r.is_ok() {
                by.entry((c.y, c.z, c.x)).or_default().push(*s);
            }
        }
    }
    let cells: Vec<BlockPos> = by.keys().map(|(y, z, x)| BlockPos::new(*x, *y, *z)).collect();
    if cells.is_empty() {
        return Err(format!("the route's golem floors: NOT JUDGED: the {} caller cell(s)' golem searches reach no floor", callers.len()));
    }
    let off: Vec<BlockPos> = cells.iter().copied().filter(|c| !golem_box_meets_portal(&w, *c)).collect();
    let mut lines = vec![format!(
        "GOLEM FLOORS from every route rider's cell as a caller ({} caller cell(s) on {} rail cell(s), {guards} golem guard(s) excluded, {nv} villager cell(s) of the design added; ironspec 17:26, standing): {} golem cell(s), {} on the pad (a golem there meets a nether portal), {} OFF THE PAD{} (bar 0)",
        callers.len(),
        rails.len() - guards,
        cells.len(),
        cells.len() - off.len(),
        off.len(),
        if off.is_empty() { String::new() } else { format!(": {}", off.iter().map(cell).collect::<Vec<_>>().join(" ")) }
    )];
    for c in &off {
        let from = &by[&(c.y, c.z, c.x)];
        let t = golem_terms(&w, *c);
        lines.push(format!(
            "  OFF THE PAD {}: yielded by {} caller cell(s)' own search: {}; the rule's terms: {}",
            cell(c),
            from.len(),
            from.iter().map(cell).collect::<Vec<_>>().join(" "),
            if t.is_empty() { "all pass".to_string() } else { t.iter().map(|x| x.label()).collect::<Vec<_>>().join(", ") }
        ));
    }
    // (tick-exact 18:13, ironspec 18:15 (5), G1 18:15: every cell the callers' spot searches stop
    // at and refuse, on the full rule with each failing term named; FRAGILE when only a removable
    // rail or signal source refuses it (remove it and a golem spawns), STATE when only a block's
    // changeable state does (open / powered / lit: that state must be permanent))
    let mut refused: std::collections::BTreeMap<(i32, i32, i32), Vec<GolemTerm>> = std::collections::BTreeMap::new();
    for s in &callers {
        // (the game's search: each candidate it stops at and refuses (`checkSpawnObstruction`))
        for (c, r) in redsim::entity::mob::spawn::golem_spawn_cells_from(&w, *s) {
            if r.as_ref().err().is_some_and(|m| !m.starts_with("no cell in the column")) && !w.get(c.below()).is_air() {
                refused.entry((c.y, c.z, c.x)).or_insert_with(|| golem_terms(&w, c));
            }
        }
        // (and each floor the search passes over only because its feet cell is neither air nor a
        // liquid: a rail or a torch standing there; remove it and the search stops there)
        for dz in -8..=8 {
            for dx in -8..=8 {
                let top = BlockPos::new(s.x + dx, s.y + 6, s.z + dz);
                let mut above = w.get(top);
                for k in 1..=13 {
                    let q = top.offset(0, -k, 0);
                    let here = w.get(q);
                    // (G1 20:02, E1: the game's own test, `legacy_golem_can_spawn_on`: the search stops
                    // where it accepts the floor; a floor it passes over only because the feet cell is
                    // neither air nor a liquid block (a rail, waterlogged or not, a torch, a string) is
                    // listed)
                    if redsim::entity::mob::blocks::legacy_golem_can_spawn_on(&here, &above) {
                        break;
                    }
                    if redsim::behavior::is_solid(&here) && !redsim::entity::mob::blocks::golem_excluded(&here) {
                        refused.entry((q.y + 1, q.z, q.x)).or_insert_with(|| golem_terms(&w, q.above()));
                    }
                    above = here;
                }
            }
        }
    }
    // (ironspec 18:35 / G1 18:37: FRAGILE and STATE by the blocks' kinds, never by a term's wording)
    let mut by_term: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let (mut fragile, mut state) = (Vec::new(), Vec::new());
    for ((y, z, x), t) in &refused {
        for term in t {
            *by_term.entry(term.key()).or_default() += 1;
        }
        let labels = || t.iter().map(|x| x.label()).collect::<Vec<_>>().join(", ");
        if !t.is_empty() && t.iter().all(|x| x.removable(&w)) {
            fragile.push(format!("{x},{y},{z} ({})", labels()));
        } else if !t.is_empty() && t.iter().all(|x| x.stateful(&w)) {
            state.push(format!("{x},{y},{z} ({})", labels()));
        }
    }
    lines.push(format!(
        "  refused stop cells: {} (by term: {}); FRAGILE (only a removable rail, signal source or collision-free guard such as a string refuses it): {}{}; STATE (only a changeable state refuses it: keep that state permanent): {}{}",
        refused.len(),
        by_term.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", "),
        fragile.len(),
        if fragile.is_empty() { String::new() } else { format!(": {}", fragile.join("; ")) },
        state.len(),
        if state.is_empty() { String::new() } else { format!(": {}", state.join("; ")) }
    ));
    // (ironspec 18:20 / 19:13: one fault-note line per FRAGILE cell)
    for ((y, z, x), t) in refused.iter().filter(|(_, t)| !t.is_empty() && t.iter().all(|x| x.removable(&w))) {
        let what: Vec<String> = t.iter().map(|g| format!("the {} at {},{},{}", g.block.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or(""), g.pos.x, g.pos.y, g.pos.z)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        lines.push(format!("  FAULT NOTE {x},{y},{z}: removing {} makes this cell a golem floor; replace what you remove", what.join(" and ")));
    }
    // (ironspec 19:15 (3): every other non-full-block kind found refusing a cell, its class stated)
    let mut kinds: std::collections::BTreeMap<String, (usize, &'static str)> = std::collections::BTreeMap::new();
    for t in refused.values() {
        let class = if !t.is_empty() && t.iter().all(|x| x.removable(&w)) {
            "FRAGILE"
        } else if !t.is_empty() && t.iter().all(|x| x.stateful(&w)) {
            "STATE"
        } else {
            "structural (another term refuses the cell too)"
        };
        for g in t.iter().filter(|g| !redsim::behavior::full_cube(&g.block) && !g.block.is_air()) {
            let id = g.block.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
            let e = kinds.entry(id).or_insert((0, class));
            e.0 += 1;
            if e.1 != class {
                e.1 = "mixed (FRAGILE in some cells, not in others)";
            }
        }
    }
    lines.push(format!("  non-full-block kinds refusing cells: {}", if kinds.is_empty() { "none".to_string() } else { kinds.iter().map(|(k, (n, c))| format!("{k} {n} ({c})")).collect::<Vec<_>>().join(", ") }));
    Ok((lines, off.len(), cells))
}

/// (ironspec Nov 13 19:11 / 19:13 / 19:45 (c), G1 20:02 E5 / E6) The components a cell's state change
/// can trigger: every press or release of a string (a tripwire), a hook, a detector or a pressure plate
/// updates the cells of its footprint, so an observer, a piston (sticky or not, or its head, which
/// passes the update to its base), a dropper or a dispenser there (quasi-connected ones included: an
/// update is what wakes them) would act. The footprint is the repo's own model for a signal source
/// (`pushscan::update_reach`: a hook's six neighbours and its support's, a plate's or a detector's and
/// the block under it's, an ascending detector's above too), a hookless string's its six faces. A cell
/// holding none of those is NOT JUDGED. The bar: 0. Returns the lines and the count.
pub fn update_reach_components(spec: &FarmSpec, cells: &[BlockPos]) -> Result<(Vec<String>, usize), String> {
    let w = settled_world_void(spec)?;
    let mut lines = Vec::new();
    let mut n = 0usize;
    let n6 = |p: BlockPos| [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].map(|(dx, dy, dz)| BlockPos::new(p.x + dx, p.y + dy, p.z + dz)).to_vec();
    for c in cells {
        let b = w.get(*c);
        let here = b.to_string();
        let id = here.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        let (what, footprint): (String, Vec<BlockPos>) = if id == "tripwire" {
            // (G1 Nov 13 21:38 H8: pressing an ATTACHED string flips its hooks, TripWireBlock.updateSource →
            // TripWireHookBlock.calculateState → notifyNeighbors at each hook and its support: its
            // footprint is its six faces and each hook's footprint, the line walked both ways to the
            // hooks facing back along it, 41 cells at most; a loose string's is its six faces)
            let mut fp = n6(*c);
            let mut hooks = Vec::new();
            if here.contains("attached=true") {
                for (dx, dz, back) in [(1, 0, "west"), (-1, 0, "east"), (0, 1, "north"), (0, -1, "south")] {
                    let mut q = *c;
                    for _ in 0..41 {
                        q = BlockPos::new(q.x + dx, q.y, q.z + dz);
                        let s = w.get(q);
                        let st = s.to_string();
                        if st.contains("tripwire_hook[") {
                            if st.contains(&format!("facing={back}"))
                                && let Some((_, hf)) = crate::pushscan::update_reach(q, &s)
                            {
                                fp.extend(hf);
                                hooks.push(q);
                            }
                            break;
                        }
                        if !st.contains("tripwire[") {
                            break;
                        }
                    }
                }
            }
            if hooks.is_empty() {
                ("a string: its six faces".to_string(), fp)
            } else {
                (format!("an attached string: its six faces and its hook(s) {}' footprints", hooks.iter().map(|h| format!("{},{},{}", h.x, h.y, h.z)).collect::<Vec<_>>().join(" ")), fp)
            }
        } else if (id == "tripwire_hook" || id.ends_with("pressure_plate") || id == "detector_rail")
            && let Some((name, fp)) = crate::pushscan::update_reach(*c, &b)
        {
            (format!("a {name}: its footprint, {} cells", fp.len()), fp)
        } else {
            return Err(format!("update reach: NOT JUDGED: {},{},{} holds {} (no string, hook, plate or detector)", c.x, c.y, c.z, here.trim_start_matches("minecraft:")));
        };
        let mut hits = Vec::new();
        for q in footprint.into_iter().filter(|q| q != c) {
            let s = w.get(q).to_string();
            let qid = s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
            let hit = format!("{} at {},{},{}", s.trim_start_matches("minecraft:"), q.x, q.y, q.z);
            if matches!(qid.as_str(), "observer" | "piston" | "sticky_piston" | "piston_head" | "moving_piston" | "dropper" | "dispenser") && !hits.contains(&hit) {
                hits.push(hit);
            }
        }
        n += hits.len();
        lines.push(format!(
            "update reach of {},{},{} ({}; {what}): {} observer / piston / dropper / dispenser{}",
            c.x,
            c.y,
            c.z,
            here.trim_start_matches("minecraft:"),
            hits.len(),
            if hits.is_empty() { " (PASS)".to_string() } else { format!(": {} (FAIL)", hits.join("; ")) }
        ));
    }
    Ok((lines, n))
}

/// Whether a block can ever be a redstone signal source, whatever its state (a wire counts: redsim's
/// `is_signal_source` reads a global wire toggle).
fn may_source(w: &redsim::World, s: &redsim::block::Block) -> bool {
    matches!(s, redsim::block::Block::Wire { .. }) || redsim::behavior::is_signal_source(w, s)
}

/// (ironspec Nov 13 19:20 / 19:29, G1 19:19 / 21:38) Every block that powers, or could ever power, a
/// cell: each signal source among its six neighbours (direct), and each source beside a redstone
/// conductor among them (power through the block), whatever their state, so "nothing that could BECOME
/// a source there" is checked too. A piston, sticky piston, dropper or dispenser takes the cell above
/// as well (its quasi-connectivity: a tool default, H6; `no_qc` opts out); a powered or activator rail
/// also takes every same-kind rail joined to it within 8 (power passes along them). A cell holding no
/// power consumer is NOT JUDGED (H2: a typo'd cell on air never passes). `allow` names source cells the
/// design means (the bar counts the rest); an allowed cell no cell's reach holds FAILS as stale (H3).
/// Returns the lines and the count of sources outside `allow` plus stale allows.
pub fn power_into(spec: &FarmSpec, cells: &[BlockPos], no_qc: bool, allow: &[BlockPos]) -> Result<(Vec<String>, usize), String> {
    let w = settled_world_void(spec)?;
    let name = |q: BlockPos| w.get(q).to_string().trim_start_matches("minecraft:").to_string();
    let n6 = |p: BlockPos| [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].map(|(dx, dy, dz)| BlockPos::new(p.x + dx, p.y + dy, p.z + dz));
    let mut lines = Vec::new();
    let mut bad = 0usize;
    let mut matched: Vec<BlockPos> = Vec::new();
    for c in cells {
        let cid = name(*c).split('[').next().unwrap_or("").to_string();
        if !power_consumer(&cid) {
            return Err(format!("power into: NOT JUDGED: {},{},{} holds {} (no power consumer)", c.x, c.y, c.z, name(*c)));
        }
        let qc = !no_qc && ["piston", "sticky_piston", "dropper", "dispenser"].contains(&cid.as_str());
        let mut into = vec![*c];
        if qc {
            into.push(c.above());
        }
        // a powered / activator rail: the same-kind rails joined to it within 8 (26-neighbours, as the
        // rail's line may climb)
        let kind = |q: BlockPos| match w.get(q) {
            redsim::block::Block::Rail { activator, .. } => Some(activator),
            _ => None,
        };
        let mut chain = Vec::new();
        if let Some(k) = kind(*c) {
            let mut seen = vec![*c];
            let mut front = vec![*c];
            for _ in 0..8 {
                let mut next = Vec::new();
                for p in &front {
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                if (dx != 0 && dz != 0) || (dx == 0 && dy == 0 && dz == 0) {
                                    continue;
                                }
                                let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                                if !seen.contains(&q) && kind(q) == Some(k) {
                                    seen.push(q);
                                    next.push(q);
                                    chain.push(q);
                                }
                            }
                        }
                    }
                }
                front = next;
            }
            into.extend(chain.iter().copied());
        }
        let mut hits: Vec<(BlockPos, String)> = Vec::new();
        for s in &into {
            for q in n6(*s) {
                let b = w.get(q);
                if may_source(&w, &b) {
                    hits.push((q, format!("{} at {},{},{} beside {},{},{}", name(q), q.x, q.y, q.z, s.x, s.y, s.z)));
                } else if redsim::behavior::is_conductor(&w, &b, q) {
                    for r in n6(q) {
                        if r != *s && may_source(&w, &w.get(r)) {
                            hits.push((r, format!("{} at {},{},{} through the conductor {} at {},{},{} beside {},{},{}", name(r), r.x, r.y, r.z, name(q), q.x, q.y, q.z, s.x, s.y, s.z)));
                        }
                    }
                }
            }
        }
        hits.sort_by_key(|(q, t)| (q.y, q.z, q.x, t.clone()));
        hits.dedup_by(|a, b| a.1 == b.1);
        matched.extend(hits.iter().map(|(q, _)| *q).filter(|q| allow.contains(q)));
        let outside: Vec<&String> = hits.iter().filter(|(q, _)| !allow.contains(q)).map(|(_, t)| t).collect();
        let allowed: Vec<&String> = hits.iter().filter(|(q, _)| allow.contains(q)).map(|(_, t)| t).collect();
        bad += outside.len();
        lines.push(format!(
            "power into {},{},{} ({}){}{}: {} source(s) in reach outside the allowed{}{}",
            c.x,
            c.y,
            c.z,
            name(*c),
            if qc { format!(", with the cell above {},{},{} ({}, quasi-connectivity)", c.x, c.y + 1, c.z, name(c.above())) } else { String::new() },
            if chain.is_empty() { String::new() } else { format!(", with {} joined same-kind rail(s) {}", chain.len(), chain.iter().map(|q| format!("{},{},{}", q.x, q.y, q.z)).collect::<Vec<_>>().join(" ")) },
            outside.len(),
            if outside.is_empty() { " (PASS)".to_string() } else { format!(": {} (FAIL)", outside.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) },
            if allowed.is_empty() { String::new() } else { format!("; allowed: {}", allowed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) }
        ));
    }
    // (G1 21:38 H3: an allow is read back: one that no cell's reach holds is stale and fails)
    for a in allow.iter().filter(|a| !matched.contains(a)) {
        bad += 1;
        lines.push(format!("allow {},{},{} ({}): no source there in any cell's reach: a stale allow (FAIL)", a.x, a.y, a.z, name(*a)));
    }
    Ok((lines, bad))
}

/// Whether a block responds to redstone power (a consumer): a rail of any kind (a plain T is power-
/// switched in 26.3), dust, a diode, a piston, a dropper, a dispenser, a hopper, a door, trapdoor or
/// gate, a lamp, a note block, TNT, a bell, a crafter, a copper bulb, a command block.
fn power_consumer(id: &str) -> bool {
    id.contains("rail") || ["redstone_wire", "repeater", "comparator", "piston", "dropper", "dispenser", "hopper", "_door", "_trapdoor", "fence_gate", "redstone_lamp", "note_block", "tnt", "bell", "crafter", "copper_bulb", "command_block"].iter().any(|k| id.contains(k))
}

/// The cell a redstone torch reads (its support): a standing torch's block below, a wall torch's
/// block behind it (opposite its facing). None for anything else.
fn torch_support(q: BlockPos, s: &str) -> Option<BlockPos> {
    if s.contains("redstone_wall_torch[") {
        let (dx, dz) = if s.contains("facing=north") {
            (0, 1)
        } else if s.contains("facing=south") {
            (0, -1)
        } else if s.contains("facing=east") {
            (-1, 0)
        } else {
            (1, 0)
        };
        Some(BlockPos::new(q.x + dx, q.y, q.z + dz))
    } else if s.contains("redstone_torch[") {
        Some(q.below())
    } else {
        None
    }
}

/// (ironspec Nov 13 19:46 (iii), ironfarm 19:50, G1 21:38) Where a source's power can go: each power
/// consumer among its six neighbours (its weak power, whatever its facing), each consumer beside a
/// redstone conductor among them (strong power through the block; a hook's or a lever's support, a
/// plate's or a detector rail's floor), and every redstone torch whose support is the source or such a
/// conductor (a torch on a powered block turns off; H7). A cell that is a CONDUCTOR (a block a source
/// powers: a latch's stone) takes its six neighbours' consumers and the torches it supports, dust
/// marked (dust takes power only from a STRONGLY powered block). A cell that is neither a source nor a
/// conductor is NOT JUDGED (H2). The design's meant consumers named by `allow`; an allow no cell
/// reaches FAILS as stale (H3). Returns the lines and the count outside `allow` plus stale allows.
pub fn power_out(spec: &FarmSpec, cells: &[BlockPos], allow: &[BlockPos]) -> Result<(Vec<String>, usize), String> {
    let w = settled_world_void(spec)?;
    let name = |q: BlockPos| w.get(q).to_string().trim_start_matches("minecraft:").to_string();
    let id = |q: BlockPos| name(q).split('[').next().unwrap_or("").to_string();
    let n6 = |p: BlockPos| [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].map(|(dx, dy, dz)| BlockPos::new(p.x + dx, p.y + dy, p.z + dz));
    let mut lines = Vec::new();
    let mut bad = 0usize;
    let mut matched: Vec<BlockPos> = Vec::new();
    for c in cells {
        let here = w.get(*c);
        let source = may_source(&w, &here);
        let conductor = !source && redsim::behavior::is_conductor(&w, &here, *c);
        if !source && !conductor {
            return Err(format!("power out: NOT JUDGED: {},{},{} holds {} (no signal source, no conductor)", c.x, c.y, c.z, name(*c)));
        }
        let mut hits: Vec<(BlockPos, String)> = Vec::new();
        let mut through = Vec::new();
        // (a torch never powers its own support)
        let own_support = torch_support(*c, &here.to_string());
        for q in n6(*c) {
            if Some(q) == own_support {
                continue;
            }
            let qs = w.get(q).to_string();
            if torch_support(q, &qs) == Some(*c) {
                hits.push((q, format!("{} at {},{},{} on it (its support: it turns off when powered)", name(q), q.x, q.y, q.z)));
            } else if power_consumer(&id(q)) {
                let dust = conductor && id(q) == "redstone_wire";
                hits.push((q, format!("{} at {},{},{} beside it{}", name(q), q.x, q.y, q.z, if dust { " (dust: only if the block is STRONGLY powered)" } else { "" })));
            } else if source && redsim::behavior::is_conductor(&w, &w.get(q), q) {
                through.push(format!("{} at {},{},{}", id(q), q.x, q.y, q.z));
                for r in n6(q) {
                    if r == *c {
                        continue;
                    }
                    let rs = w.get(r).to_string();
                    if torch_support(r, &rs) == Some(q) {
                        hits.push((r, format!("{} at {},{},{} on the conductor {} at {},{},{} (its support: it turns off when powered)", name(r), r.x, r.y, r.z, id(q), q.x, q.y, q.z)));
                    } else if power_consumer(&id(r)) {
                        hits.push((r, format!("{} at {},{},{} through the conductor {} at {},{},{}", name(r), r.x, r.y, r.z, id(q), q.x, q.y, q.z)));
                    }
                }
            }
        }
        hits.sort_by_key(|(q, t)| (q.y, q.z, q.x, t.clone()));
        hits.dedup_by(|a, b| a.1 == b.1);
        matched.extend(hits.iter().map(|(q, _)| *q).filter(|q| allow.contains(q)));
        let outside: Vec<&String> = hits.iter().filter(|(q, _)| !allow.contains(q)).map(|(_, t)| t).collect();
        let allowed: Vec<&String> = hits.iter().filter(|(q, _)| allow.contains(q)).map(|(_, t)| t).collect();
        bad += outside.len();
        lines.push(format!(
            "power out of {},{},{} ({}{}): {}; {} consumer(s) in reach outside the allowed{}{}",
            c.x,
            c.y,
            c.z,
            name(*c),
            if conductor { ", a conductor: what it powers when powered" } else { "" },
            if conductor {
                "its six neighbours".to_string()
            } else {
                format!("conductors beside it: {}", if through.is_empty() { "none (its strong power goes nowhere)".to_string() } else { through.join(", ") })
            },
            outside.len(),
            if outside.is_empty() { " (PASS)".to_string() } else { format!(": {} (FAIL)", outside.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) },
            if allowed.is_empty() { String::new() } else { format!("; allowed: {}", allowed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")) }
        ));
    }
    // (G1 21:38 H3: an allow is read back: one that no cell reaches is stale and fails)
    for a in allow.iter().filter(|a| !matched.contains(a)) {
        bad += 1;
        lines.push(format!("allow {},{},{} ({}): no cell's power reaches it: a stale allow (FAIL)", a.x, a.y, a.z, name(*a)));
    }
    Ok((lines, bad))
}

/// (ironspec Nov 13 19:11, with day-1; G1 21:38 / 21:41) The riderless-source statics: what could take a
/// rider out of its cart, or kill it, on the route: every route position (`dense_rider_points`: each
/// rail at 1/20 of its length, each lift or drop column of `auto_columns` every 0.1; H5), NOT JUDGED
/// with no route at all.
/// - (a) every activator rail with every block that powers or could power it (`power_into`; the
///   design's own emptying rail named by `allow`, an allow read back: stale with no activator, H3);
/// - (c) every seated zombie whose attack box (`Mob.getAttackBoundingBox`: its box and its vehicle's
///   unioned horizontally, inflated by sqrt(2.04) - 0.6 in x and z) meets a rider's box at a route
///   point, and whether it sees the rider (`Sensing.hasLineOfSight`: the collider, eye to eye): a
///   strike needs both; a boat's zombie taken at its SEAT (`rider_seat`: the boat's y + 0.1875 - 0.7;
///   H4), where the static world left it unseated; seated pillagers listed (ranged: the flee check's
///   sight bar judges them);
/// - (d) every nether portal cell the CART's box meets (0.98 x 0.7 at the cart's position, swept
///   between consecutive samples, deflated 1e-5, against the full portal cell): a seated rider is
///   refused by the portal and rides through with its cart (tick-exact 21:40, G1 21:41);
/// - (e) every free fall off a rail end with its drop and whether water breaks it (a passenger takes
///   its cart's fall, `propagateFallToPassengers`: ceil(d - 3) damage; 20 kills a villager); a fall
///   with no drop to price (out of the design's box, onto the golem guard) FAILS (H1).
/// (b) in-wall damage is the eye check (`eye_check`). Returns the lines and the count of failures (an
/// unallowed activator source or a stale allow, a strike, a portal met, a fatal or unpriced fall).
pub fn riderless_statics(spec: &FarmSpec, allow_activators: &[BlockPos]) -> Result<(Vec<String>, usize), String> {
    let w = settled_world_void(spec)?;
    let mut lines = Vec::new();
    let mut bad = 0usize;
    // (a)
    let acts: Vec<BlockPos> = spec.blocks.iter().map(|(p, _)| *p).filter(|p| matches!(w.get(*p), redsim::block::Block::Rail { activator: true, .. })).collect();
    if acts.is_empty() {
        lines.push("(a) activator rails: none".to_string());
        for a in allow_activators {
            bad += 1;
            lines.push(format!("(a) allow {},{},{}: the design has no activator rail: a stale allow (FAIL)", a.x, a.y, a.z));
        }
    } else {
        let (al, an) = power_into(spec, &acts, false, allow_activators)?;
        lines.push(format!("(a) activator rails: {} ({} source(s) outside the allowed)", acts.len(), an));
        lines.extend(al.into_iter().map(|l| format!("  {l}")));
        bad += an;
    }
    let (cols, why) = auto_columns(spec, &[]);
    let dense = dense_rider_points(spec, &cols);
    if dense.is_empty() {
        return Err("riderless: NOT JUDGED: the design has no route (no rail, no column): no position to check".to_string());
    }
    let pts: Vec<(String, Vec3)> = dense.iter().map(|(l, p, _)| (l.clone(), *p)).collect();
    let vd = redsim::entity::mob::types::MobType::Villager.def();
    let (hw, h) = (vd.width as f64 / 2.0, vd.height as f64);
    let rider_box = |f: Vec3| redsim::entity::Aabb { min_x: f.x - hw, min_y: f.y, min_z: f.z - hw, max_x: f.x + hw, max_y: f.y + h, max_z: f.z + hw };
    // (c)
    let with = with_design_entities(&w, spec, "riderless")?;
    let reach = ((2.04f32) as f64).sqrt() - (0.6f32 as f64);
    let zd = redsim::entity::mob::types::MobType::Zombie.def();
    let (zhw, zh) = (zd.width as f64 / 2.0, zd.height as f64);
    let mut zombies = 0usize;
    for e in with.entities.list.iter() {
        let redsim::entity::EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != redsim::entity::mob::types::MobType::Zombie {
            continue;
        }
        zombies += 1;
        let mut bx = e.bb;
        let mut feet = e.pos;
        let mut vname = "on foot".to_string();
        if let Some(v) = e.vehicle.and_then(|v| redsim::entity::index_of(&with, v)) {
            let ve = &with.entities.list[v];
            // (G1 21:38 H4: the seat as the game places it, `positionRider`: the vehicle's position plus
            // its passenger attachment plus the rider's riding offset; redsim's static world leaves a
            // boat's passenger where it spawned)
            if let Some((seat, ro)) = redsim::entity::mob::rider_seat(&ve.kind, m) {
                feet = Vec3::new(ve.pos.x, (ve.pos.y + seat) + ro, ve.pos.z);
                bx = redsim::entity::Aabb { min_x: feet.x - zhw, min_y: feet.y, min_z: feet.z - zhw, max_x: feet.x + zhw, max_y: feet.y + zh, max_z: feet.z + zhw };
            }
            let vb = ve.bb;
            bx = redsim::entity::Aabb { min_x: bx.min_x.min(vb.min_x), min_y: bx.min_y, min_z: bx.min_z.min(vb.min_z), max_x: bx.max_x.max(vb.max_x), max_y: bx.max_y, max_z: bx.max_z.max(vb.max_z) };
            vname = format!("seated: feet y {:.4}", feet.y);
        }
        let ab = redsim::entity::Aabb { min_x: bx.min_x - reach, min_y: bx.min_y, min_z: bx.min_z - reach, max_x: bx.max_x + reach, max_y: bx.max_y, max_z: bx.max_z + reach };
        let zeye_at = Vec3::new(feet.x, feet.y + zeye(), feet.z);
        let mut met = Vec::new();
        let mut strikes = 0usize;
        for (lab, f) in &pts {
            let r = rider_box(*f);
            if ab.min_x < r.max_x && ab.max_x > r.min_x && ab.min_y < r.max_y && ab.max_y > r.min_y && ab.min_z < r.max_z && ab.max_z > r.min_z {
                let seen = ray_pair(&w, Vec3::new(f.x, f.y + veye(), f.z), zeye_at).1;
                if seen {
                    strikes += 1;
                }
                met.push(format!("{lab} ({:.2},{:.2},{:.2}): sight {}", f.x, f.y, f.z, if seen { "CLEAR: it can strike" } else { "blocked" }));
            }
        }
        bad += strikes;
        lines.push(format!(
            "(c) the zombie at {:.2},{:.2},{:.2} ({vname}; attack box x {:.2}..{:.2} y {:.2}..{:.2} z {:.2}..{:.2}): {} route point(s) in reach, {} it can strike{}",
            e.pos.x,
            e.pos.y,
            e.pos.z,
            ab.min_x,
            ab.max_x,
            ab.min_y,
            ab.max_y,
            ab.min_z,
            ab.max_z,
            met.len(),
            strikes,
            if met.is_empty() { " (PASS)".to_string() } else { format!(": {}{}", met.join("; "), if strikes > 0 { " (FAIL)" } else { " (PASS: no sight)" }) }
        ));
    }
    if zombies == 0 {
        lines.push("(c) seated zombies: none".to_string());
    }
    let (pills, unjudged) = design_hostiles(spec);
    if !pills.is_empty() {
        lines.push(format!("(c) seated pillagers: {} (ranged attackers: the flee check's sight bar judges them, not (c)'s melee reach): {}", pills.len(), pills.iter().map(|p| format!("{:.2},{:.2},{:.2}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ")));
    }
    for u in &unjudged {
        lines.push(format!("(c) {u}: a hostile redsim doesn't model: NOT JUDGED here"));
    }
    // (d) (tick-exact 21:40, G1 21:41: the portal takes the CART, a seated rider refused by it: the
    // cart's box swept between consecutive samples of one rail or column)
    let mut portal_hits: Vec<String> = Vec::new();
    let cart_box = |f: Vec3| [f.x - 0.49, f.x + 0.49, f.y, f.y + 0.7, f.z - 0.49, f.z + 0.49];
    for k in 0..dense.len() {
        let (lab, f, _) = &dense[k];
        let mut bx = cart_box(*f);
        if let Some((lab2, g, _)) = dense.get(k + 1)
            && lab2 == lab
        {
            let gb = cart_box(*g);
            bx = [bx[0].min(gb[0]), bx[1].max(gb[1]), bx[2].min(gb[2]), bx[3].max(gb[3]), bx[4].min(gb[4]), bx[5].max(gb[5])];
        }
        let e = 1e-5;
        for x in (bx[0] + e).floor() as i32..=(bx[1] - e).floor() as i32 {
            for y in (bx[2] + e).floor() as i32..=(bx[3] - e).floor() as i32 {
                for z in (bx[4] + e).floor() as i32..=(bx[5] - e).floor() as i32 {
                    if w.get(BlockPos::new(x, y, z)).to_string().contains("nether_portal") {
                        portal_hits.push(format!("the cart on {lab} meets the portal cell {x},{y},{z}"));
                    }
                }
            }
        }
    }
    portal_hits.sort();
    portal_hits.dedup();
    bad += portal_hits.len();
    lines.push(format!("(d) portal cells the cart's box meets (0.98 x 0.7, swept; {} route sample(s)): {}{}", dense.len(), portal_hits.len(), if portal_hits.is_empty() { " (PASS)".to_string() } else { format!(": {} (FAIL)", portal_hits.join("; ")) }));
    // (e)
    let nums = |s: &str| -> Vec<i32> { s.split(|c: char| !(c.is_ascii_digit() || c == '-')).filter_map(|t| t.parse::<i32>().ok()).collect() };
    let mut falls = 0usize;
    for l in why.iter().filter(|l| l.contains("a free fall off the rail end")) {
        falls += 1;
        let (Some(i), Some(j)) = (l.find(" into "), l.find(" down ")) else {
            // (G1 21:38 H1: a fall the static can't price, out of the design's box or onto the golem
            // guard, is a failure, never a pass)
            bad += 1;
            lines.push(format!("(e) {}: no drop to price (out of the design's box, or onto the golem guard): (FAIL)", l.trim_start_matches("auto column: ")));
            continue;
        };
        let m = nums(&l[i + 6..]);
        let d = nums(&l[j + 6..]);
        let (Some(&mx), Some(&my), Some(&mz), Some(&drop)) = (m.first(), m.get(1), m.get(2), d.first()) else {
            bad += 1;
            lines.push(format!("(e) {}: its drop doesn't parse: NOT JUDGED (FAIL)", l.trim_start_matches("auto column: ")));
            continue;
        };
        let ly = my - drop - 1;
        // (G1 22:13 / 22:15: water anywhere along the SWEPT fall column zeroes the fall)
        let wet = (ly + 1..=my).any(|y| {
            let s = w.get(BlockPos::new(mx, y, mz)).to_string();
            s.contains("water") || s.contains("bubble_column") || s.contains("waterlogged=true")
        });
        // (ironspec 22:12 (2), G1 22:15, cc767b78 22:15: the landed-on block's `fallOn`, as redsim's
        // `fall_on_factors`: a bed or a shelf mushroom halves the distance, slime zeroes the damage,
        // hay and honey take 0.2 of it; the landed-on block of a cart landing on a rail is the rail's
        // support (its on-position, getOnPos(0.2), 0.2 under the cart in the rail cell))
        let rail_landing = l.contains(" to the rail ");
        let land = ["to the rail ", "to the floor "].iter().find_map(|k| l.find(k).map(|i| nums(&l[i + k.len()..]))).and_then(|v| (v.len() >= 3).then(|| BlockPos::new(v[0], v[1], v[2])));
        let on = land.map(|c| if rail_landing { c.below() } else { c });
        let on_s = on.map(|c| w.get(c).to_string()).unwrap_or_default();
        let id = on_s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
        let mult = if id.ends_with("_bed") || id == "shelf_mushroom" { 0.5 } else { 1.0 };
        let modifier: f64 = match id.as_str() {
            "slime_block" => 0.0,
            "hay_block" | "honey_block" => 0.2,
            _ => 1.0,
        };
        let dmg = if wet { 0 } else { (((drop as f64) * mult + 1e-6 - 3.0) * modifier).floor().max(0.0) as i32 };
        let fatal = dmg >= 20;
        if fatal {
            bad += 1;
        }
        let on_say = on.map(|c| format!("landing on the {id} at {},{},{}", c.x, c.y, c.z)).unwrap_or_else(|| "its landing block unread".to_string());
        lines.push(format!(
            "(e) {}: {}",
            l.trim_start_matches("auto column: "),
            if wet {
                "water in the fall's swept column breaks it: no damage".to_string()
            } else if modifier == 0.0 {
                format!("{on_say}: fallOn x 0: no damage")
            } else if dmg == 0 {
                format!("{on_say}: no damage")
            } else if rail_landing {
                format!(
                    "{on_say} (fallOn x {modifier}{}): FALL DAMAGE UNCONFIRMED: up to {dmg} if the cart hits the support (moveAlongTrack's reset erases it if a tick ends in the cell above the landing rail; pending tick-exact's pin){}",
                    if mult < 1.0 { ", the distance halved" } else { "" },
                    if fatal { ": FATAL to a villager (20) if taken (FAIL: a hold pending the pin)" } else { "" }
                )
            } else {
                format!("{on_say} (fallOn x {modifier}): a passenger takes {dmg} damage{}", if fatal { ": FATAL to a villager (20) (FAIL)" } else { "" })
            }
        ));
    }
    if falls == 0 {
        lines.push("(e) free falls off rail ends: none".to_string());
    }
    lines.push("(b) in-wall damage: the eye check (flee-check's eye positions against suffocating blocks)".to_string());
    Ok((lines, bad))
}
/// (G1 Nov 13 18:37, B2 / B7) Whether an iron golem standing at `cell` (its box `MobType::IronGolem`'s
/// width and height around the cell's centre, deflated by 1e-5 as `Entity.checkInsideBlocks` takes
/// it) meets a nether portal block: a pad golem, which crosses at once.
pub fn golem_box_meets_portal(w: &redsim::World, cell: BlockPos) -> bool {
    let d = redsim::entity::mob::types::MobType::IronGolem.def();
    let (e, hw, h) = (1e-5, d.width as f64 / 2.0, d.height as f64);
    let (x0, x1) = ((cell.x as f64 + 0.5 - hw + e).floor() as i32, (cell.x as f64 + 0.5 + hw - e).floor() as i32);
    let (z0, z1) = ((cell.z as f64 + 0.5 - hw + e).floor() as i32, (cell.z as f64 + 0.5 + hw - e).floor() as i32);
    let (y0, y1) = ((cell.y as f64 + e).floor() as i32, (cell.y as f64 + h - e).floor() as i32);
    (x0..=x1).any(|x| (y0..=y1).any(|y| (z0..=z1).any(|z| w.get(BlockPos::new(x, y, z)).to_string().contains("nether_portal"))))
}

/// A term of the golem spawn rule (tick-exact Nov 13 18:13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GolemTermKind {
    FloorListed,
    FloorNotSolid,
    FloorNotStandable,
    FeetNotAirOrLiquid,
    FullBlock,
    SignalSource,
    Rail,
    Fluid,
    Dangerous,
}

/// One failing term of the golem spawn rule at a cell: where (-1 the floor, 0 the feet cell, 1 / 2
/// the cells above), which term, the block it reads and that block's cell.
#[derive(Clone, Debug)]
pub struct GolemTerm {
    pub at: i32,
    pub kind: GolemTermKind,
    pub block: redsim::block::Block,
    pub pos: BlockPos,
}

/// A rail by its kind: the modeled rail blocks, a terrain state by `#prevent_mob_spawning_inside`.
fn rail_kind(b: &redsim::block::Block) -> bool {
    use redsim::block::Block;
    matches!(b, Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. }) || matches!(b, Block::Terrain(id) if redsim::terrain::blocks::props(*id).has_tag("minecraft:prevent_mob_spawning_inside"))
}

impl GolemTerm {
    fn place(&self) -> String {
        match self.at {
            -1 => "floor".to_string(),
            0 => "feet".to_string(),
            k => format!("{k} above"),
        }
    }

    /// The term's name, the count key.
    pub fn key(&self) -> String {
        match self.kind {
            GolemTermKind::FloorListed => "floor listed".to_string(),
            GolemTermKind::FloorNotSolid => "floor not solid".to_string(),
            GolemTermKind::FloorNotStandable => "floor not standable".to_string(),
            GolemTermKind::FeetNotAirOrLiquid => "feet not air or liquid".to_string(),
            GolemTermKind::FullBlock => format!("{}: full block", self.place()),
            GolemTermKind::SignalSource => format!("{}: signal source", self.place()),
            GolemTermKind::Rail => format!("{}: rail", self.place()),
            GolemTermKind::Fluid => format!("{}: fluid", self.place()),
            GolemTermKind::Dangerous => format!("{}: dangerous", self.place()),
        }
    }

    /// The term with its block, as printed.
    pub fn label(&self) -> String {
        format!("{} ({})", self.key(), self.block.to_string().trim_start_matches("minecraft:"))
    }

    /// Whether the term reads a removable component: a rail or a signal source by its block's kind
    /// (in the feet cell, the "not air or liquid" term too).
    /// (G1 Nov 13 19:23: judged per BLOCK, not per term kind: a removable component (a rail by its
    /// kind, a signal source (a full-block one, an observer or a redstone block, too) or a block with
    /// no collision shape: the hookless string, a torch) makes every term it raises in the feet cell
    /// or above removable, its full-block term included; a floor term never is (no floor, no cell).)
    pub fn removable(&self, w: &redsim::World) -> bool {
        self.at >= 0 && (rail_kind(&self.block) || redsim::behavior::is_signal_source(w, &self.block) || redsim::behavior::collision_boxes(&self.block).is_empty())
    }

    /// Whether flipping its block's changeable state (lit, open, powered) turns the term off: that
    /// state must stay as it is.
    pub fn stateful(&self, w: &redsim::World) -> bool {
        use redsim::entity::mob::blocks as mb;
        state_flips(&self.block).iter().any(|f| match self.kind {
            GolemTermKind::FullBlock => !redsim::behavior::full_cube(f),
            GolemTermKind::SignalSource => !redsim::behavior::is_signal_source(w, f),
            GolemTermKind::Dangerous => !mb::is_dangerous(f, false),
            GolemTermKind::Fluid => redsim::fluid::fluid_state_of(f).is_empty(),
            GolemTermKind::FeetNotAirOrLiquid => f.is_air() || matches!(f, redsim::block::Block::Water { .. } | redsim::block::Block::Lava { .. } | redsim::block::Block::BubbleColumn { .. }),
            GolemTermKind::FloorListed => !mb::golem_excluded(f),
            GolemTermKind::FloorNotSolid => redsim::behavior::is_solid(f),
            GolemTermKind::FloorNotStandable => {
                let mut c = w.clone();
                c.put(self.pos, *f);
                mb::entity_can_stand_on(&c, self.pos)
            }
            GolemTermKind::Rail => !rail_kind(f),
        })
    }
}

/// The block with each of its changeable boolean states (lit, open, powered) flipped, one at a time.
fn state_flips(b: &redsim::block::Block) -> Vec<redsim::block::Block> {
    let s = b.to_string();
    let mut out = Vec::new();
    for prop in ["lit", "open", "powered"] {
        for (from, to) in [("true", "false"), ("false", "true")] {
            let pat = format!("{prop}={from}");
            if s.contains(&pat) {
                let t = s.replace(&pat, &format!("{prop}={to}"));
                if let Ok(f) = redsim::block::Block::parse(&t) {
                    out.push(f);
                } else if let Some(id) = redsim::terrain::blocks::id_of(&t) {
                    out.push(redsim::block::Block::Terrain(id));
                }
            }
        }
    }
    out
}

/// (tick-exact Nov 13 18:13, ironspec 18:15 (5), G1 18:15) Every term of the golem spawn rule a cell
/// fails: the floor (LEGACY_IRON_GOLEM: listed, not solid; `checkSpawnObstruction`: not standable),
/// the feet cell (air or liquid for the floor search; no full block, signal source, rail or dangerous
/// block, its fluid ignored) and the two cells above it (no full block, signal source, rail, fluid or
/// dangerous block). Empty: a golem spawns there (entities aside).
pub fn golem_terms(w: &redsim::World, cell: BlockPos) -> Vec<GolemTerm> {
    use redsim::entity::mob::blocks as mb;
    let mut f = Vec::new();
    let floor = w.get(cell.below());
    let term = |at: i32, kind: GolemTermKind, block: redsim::block::Block, pos: BlockPos| GolemTerm { at, kind, block, pos };
    if mb::golem_excluded(&floor) {
        f.push(term(-1, GolemTermKind::FloorListed, floor, cell.below()));
    } else if !redsim::behavior::is_solid(&floor) {
        f.push(term(-1, GolemTermKind::FloorNotSolid, floor, cell.below()));
    }
    if !mb::entity_can_stand_on(w, cell.below()) {
        f.push(term(-1, GolemTermKind::FloorNotStandable, floor, cell.below()));
    }
    let feet = w.get(cell);
    // (G1 Nov 13 19:23: the floor search's own test, the BLOCK's `liquid()`: water, lava, a bubble
    // column, a terrain liquid; a waterlogged block holds a fluid but is no liquid block)
    let liquid_block = matches!(feet, redsim::block::Block::Water { .. } | redsim::block::Block::Lava { .. } | redsim::block::Block::BubbleColumn { .. }) || matches!(feet, redsim::block::Block::Terrain(id) if redsim::terrain::blocks::props(id).liquid);
    if !feet.is_air() && !liquid_block {
        f.push(term(0, GolemTermKind::FeetNotAirOrLiquid, feet, cell));
    }
    for k in 0..3 {
        let p = cell.offset(0, k, 0);
        let s = w.get(p);
        if redsim::behavior::full_cube(&s) {
            f.push(term(k, GolemTermKind::FullBlock, s, p));
        }
        if redsim::behavior::is_signal_source(w, &s) {
            f.push(term(k, GolemTermKind::SignalSource, s, p));
        }
        // (#prevent_mob_spawning_inside is #rails: every rail kind)
        if rail_kind(&s) {
            f.push(term(k, GolemTermKind::Rail, s, p));
        }
        if k > 0 && !redsim::fluid::fluid_state(w, p).is_empty() {
            f.push(term(k, GolemTermKind::Fluid, s, p));
        }
        if mb::is_dangerous(&s, false) {
            f.push(term(k, GolemTermKind::Dangerous, s, p));
        }
    }
    f
}

/// (ironspec Nov 13 08:18 (e): the exterior hostile-spawn static on the FLOATING site) Every cell
/// on the build's outside a natural monster could spawn in at night: each cell right above a block
/// of the design (a top surface, the lightning rod's included) that is no design block itself and
/// not the farm's interior (`hostile_farm`'s), tested as C1 tests the inside (`mobs_at` over the
/// site biome's Monster list, then `can_spawn_at`: the floor tags, the 2-high space, no collision,
/// the darkest night sky; a world without land models no block light, so every cell is taken as
/// dark: a torch credits nothing here). The bar: 0 spawnable cells. Returns the lines and the
/// spawnable count.
pub fn exterior_spawns(spec: &FarmSpec, biome: &'static str) -> Result<(Vec<String>, usize), String> {
    spawn_static(spec, biome, false).map(|(lines, _, n)| (lines, n))
}

/// (G1 Nov 13 11:23 (a): the light-aware decider) The floating site's spawn static, inside and
/// out: the farm's interior cells (C1's) and its top-surface cells outside the interior, each
/// tested for the site biome's Monster list (`mobs_at`, `can_spawn_at`: floor, space, no
/// collision, the darkest night sky); with `lit`, a cell counts only where the design's own light
/// leaves block light 0 (redsim's block-light engine over the design's sources, `sun::emission`:
/// a monster needs block light 0, and at night the sky always passes the brightness draw), else
/// every cell is taken as dark (the void models no light: the conservative bound). Returns the
/// lines and the interior and exterior spawnable counts (each bar 0).
///
/// NOTES (G1 Nov 13 12:40 (a)): the light is `max(sun::emission, the game table's luminance)`; an
/// emitter in neither table counts 0 (redsim's `sun::emission` lacks the plain torch, cc767b78's
/// list): an UNDER-credit, conservative here because light only stops a spawn. A static where
/// light would HELP a hostile (none today) must not reuse this light as it stands. (An earlier
/// "occluding emitter lights nothing" report was a fixture error, withdrawn: the output chest had
/// overwritten the emitter; redsim's engine carries a floor emitter's light, cc767b78 12:50.)
pub fn spawn_static(spec: &FarmSpec, biome: &'static str, lit: bool) -> Result<(Vec<String>, usize, usize), String> {
    if redsim::terrain::biome::biome(biome).is_none() {
        return Err(format!("the site biome {biome}: no such biome in redsim's table"));
    }
    let mut w = settled_world_void(spec)?;
    // (the dark bound: no block light credited anywhere, the spawn rules included, as the void read before
    // land-less block light was modeled: cc767b78's light series on train 11's spawn static)
    w.block_light_zero = !lit;
    let afk = Vec3::new(spec.output.x as f64 + 0.5, (spec.lo.y - 60) as f64, spec.output.z as f64 + 0.5);
    use redsim::entity::mob::natural::{Category, Difficulty, Natural, kind_of, mobs_at};
    w.mob.natural = Some(Natural { void_biome: Some(biome), ..Natural::new(vec![afk], Difficulty::Hard) });
    // (the farm's interior as C1 reads it: its villagers summon, so the golem cells and the two
    // above them are the farm's own space, not its outside)
    let summoners = redsim::entity::mob::site::villager_cells(&with_design_entities(&w, spec, "the exterior spawn static")?);
    let farm = circuit::mobfarm::hostile_farm(&w, spec, afk, &summoners);
    let interior: std::collections::HashSet<BlockPos> = farm.interior.iter().copied().collect();
    let design: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| !matches!(b, redsim::block::Block::Air)).map(|(p, _)| *p).collect();
    let mut tops: Vec<BlockPos> = design.iter().map(|q| q.above()).filter(|p| !design.contains(p) && !interior.contains(p)).collect();
    tops.sort_by_key(|p| (p.y, p.z, p.x));
    tops.dedup();
    let mut inner: Vec<BlockPos> = farm.interior.clone();
    inner.sort_by_key(|p| (p.y, p.z, p.x));
    inner.dedup();
    let mut lit_off = 0usize;
    let mut marginal: Vec<(BlockPos, i32)> = Vec::new();
    let mut portal_lit: Vec<BlockPos> = Vec::new();
    let emit_cache: std::cell::RefCell<HashMap<redsim::block::Block, i32>> = std::cell::RefCell::new(HashMap::new());
    let mut test = |cells: &[BlockPos]| -> Result<Vec<(BlockPos, Vec<&'static str>, i32)>, String> {
        let mut hits = Vec::new();
        for p in cells {
            let mut kinds: Vec<&'static str> = Vec::new();
            for s in mobs_at(&w, Category::Monster, *p) {
                let Some(k) = kind_of(&s.ty) else { continue };
                if redsim::entity::mob::hostile::can_spawn_at(&w, k, *p) && !kinds.contains(&k.id) {
                    kinds.push(k.id);
                }
            }
            if kinds.is_empty() {
                continue;
            }
            // (the game's isValidEmptySpawnBlock also refuses a block in
            // #prevent_mob_spawning_inside, the rails: redsim's empty test lacks it, so a rail cell
            // is skipped here)
            if redsim::rails::is_rail(&w.get(*p)) {
                continue;
            }
            // (the light: redsim's block-light engine with the mob layer's emitters and each block's
            // own luminance from the game's block table (the nether portal's 11, crying obsidian's
            // 10: the sun table lacks them), memoised per block state)
            let emission = |b: &redsim::block::Block| -> i32 {
                if let Some(e) = emit_cache.borrow().get(b) {
                    return *e;
                }
                let e = redsim::entity::mob::sun::emission(b);
                let tid = match b {
                    redsim::block::Block::Terrain(id) => Some(*id),
                    _ => redsim::terrain::blocks::id_of(&b.to_string()),
                };
                let e = e.max(tid.map_or(0, |id| redsim::terrain::blocks::props(id).emission as i32));
                emit_cache.borrow_mut().insert(*b, e);
                e
            };
            let light = if lit { redsim::daylight::block_light_from(&w, *p, 1, &emission)? } else { 0 };
            if light > 0 {
                lit_off += 1;
                // (ironspec 11:31: a cell cleared by block light 1 or 2 gets a game light read or
                // is spawn-proofed anyway: listed as MARGINAL)
                if light <= 2 {
                    marginal.push((*p, light));
                }
                // (G1 Nov 13 12:32 (b): a cell only the portals light is spawn-proof only while
                // they burn: the same light without any nether portal's emission)
                let no_portal = |b: &redsim::block::Block| -> i32 {
                    let is_portal = match b {
                        redsim::block::Block::Terrain(id) => redsim::terrain::blocks::props(*id).name.starts_with("minecraft:nether_portal"),
                        _ => false,
                    };
                    if is_portal { 0 } else { emission(b) }
                };
                if redsim::daylight::block_light_from(&w, *p, 1, &no_portal)? == 0 {
                    portal_lit.push(*p);
                }
                continue;
            }
            hits.push((*p, kinds, light));
        }
        Ok(hits)
    };
    let in_hits = test(&inner)?;
    let out_hits = test(&tops)?;
    let mut lines = vec![format!(
        "the spawn static (ironspec Nov 13 08:18 (e), G1 11:23 (a); the FLOATING site: void outside the box, the site biome {biome}, the darkest night sky; {}): {} interior cells, {} spawnable; {} top-surface cells outside the interior, {} spawnable{}{}",
        if lit { "LIGHT-AWARE: the design's own block light (redsim's engine over its sources), a cell counts only at block light 0" } else { "ALL DARK: no block light modeled in the void, so no torch credited (a conservative bound)" },
        inner.len(),
        in_hits.len(),
        tops.len(),
        out_hits.len(),
        if lit { format!("; {lit_off} cell(s) a dark run would list are lit (block light > 0)") } else { String::new() },
        if in_hits.is_empty() && out_hits.is_empty() { " (PASS: bars 0)" } else { " (FAIL: bars 0)" }
    )];
    for (what, hits) in [("INSIDE", &in_hits), ("OUTSIDE", &out_hits)] {
        for (p, kinds, _) in hits.iter() {
            let below = spec.blocks.iter().find(|(q, _)| *q == p.below()).map(|(_, b)| b.to_string()).unwrap_or_default();
            lines.push(format!("  SPAWNABLE {what} {},{},{} on {}: {}", p.x, p.y, p.z, below.trim_start_matches("minecraft:"), kinds.iter().map(|k| k.trim_start_matches("minecraft:")).collect::<Vec<_>>().join(", ")));
        }
    }
    for (p, light) in &marginal {
        lines.push(format!("  MARGINAL {},{},{}: cleared by block light {light} only (ironspec 11:31: a game light read, or spawn-proof it)", p.x, p.y, p.z));
    }
    if !portal_lit.is_empty() {
        lines.push(format!(
            "  PORTAL-LIT ({} cell(s)): spawn-proof by portal light 11 only; if a portal is out, spawnable (G1 12:32 (b): the build lights the portals first; a torch per cell is the permanent alternative): {}",
            portal_lit.len(),
            portal_lit.iter().map(|p| format!("{},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ")
        ));
    }
    Ok((lines, in_hits.len(), out_hits.len()))
}

/// (ironspec Nov 13 09:44 / 09:55, G1 09:51 / 09:58) A signal source's class, DERIVED: its update
/// footprint (settle-check's `update_reach`) and whether its output can change in operation. A
/// redstone block is CONSTANT unless a piston can reach it (any piston within 13: it could be
/// moved); a torch is CONSTANT when nothing can power its support (the support no conductor, or
/// no source's power out reaching it), else OPERATING; every other source is OPERATING (detector
/// rails, levers and buttons (stand-ins of operating lines), repeaters, comparators, observers,
/// dust, plates, ...). Each class carries its derivation.
pub struct SourceClass {
    pub pos: BlockPos,
    pub block: String,
    pub kind: &'static str,
    pub reach: Vec<BlockPos>,
    pub operating: bool,
    pub why: String,
}

pub fn source_classes(spec: &FarmSpec) -> Vec<SourceClass> {
    let w0 = redsim::World::new(BlockPos::new(0, 0, 0), [1, 1, 1]);
    let pistons: Vec<BlockPos> = spec.blocks.iter().filter(|(_, b)| matches!(b, redsim::block::Block::Piston { .. })).map(|(p, _)| *p).collect();
    let powered_cells: Vec<(BlockPos, BlockPos)> = spec
        .blocks
        .iter()
        .filter_map(|(p, b)| crate::pushscan::power_out(*p, b).map(|(weak, strong)| (*p, weak, strong)))
        .flat_map(|(p, weak, strong)| weak.into_iter().chain(strong).map(move |c| (p, c)))
        .collect();
    let block_at = |q: BlockPos| spec.blocks.iter().find(|(p, _)| *p == q).map(|(_, b)| *b).unwrap_or(redsim::block::Block::Air);
    // (ironspec Nov 13 11:27 (a): the torches by a FIXPOINT: a torch whose support only CONSTANT
    // sources feed (a redstone block no piston reaches, or a constant LIT torch) is DEAD
    // (permanently off, a design error); a dead torch feeds nothing; a torch with an OPERATING
    // feeder is operating; a cycle left undecided is taken as operating)
    #[derive(Clone, Copy, PartialEq)]
    enum T {
        Lit,
        Dead,
        Op,
        Pending,
    }
    let mut torch: HashMap<BlockPos, (T, String, BlockPos, Vec<BlockPos>)> = HashMap::new();
    for (p, b) in &spec.blocks {
        if !matches!(b, redsim::block::Block::Torch { .. } | redsim::block::Block::WallTorch { .. }) {
            continue;
        }
        let support = match b {
            redsim::block::Block::WallTorch { facing, .. } => p.rel(facing.opposite()),
            _ => p.rel(redsim::pos::Dir::Down),
        };
        let sb = block_at(support);
        let feeders: Vec<BlockPos> = powered_cells.iter().filter(|(s, c)| *c == support && s != p).map(|(s, _)| *s).collect();
        let (t, why) = if !redsim::behavior::is_conductor(&w0, &sb, support) {
            (T::Lit, format!("its support {},{},{} ({}) is no conductor: nothing powers it", support.x, support.y, support.z, sb.to_string().trim_start_matches("minecraft:")))
        } else if feeders.is_empty() {
            (T::Lit, format!("no source's power reaches its support {},{},{}", support.x, support.y, support.z))
        } else {
            (T::Pending, String::new())
        };
        torch.insert(*p, (t, why, support, feeders));
    }
    let piston_near = |q: &BlockPos| pistons.iter().any(|r| (r.x - q.x).abs().max((r.y - q.y).abs()).max((r.z - q.z).abs()) <= 13);
    loop {
        let mut changed = false;
        let snapshot: HashMap<BlockPos, T> = torch.iter().map(|(p, v)| (*p, v.0)).collect();
        for (p, (t, why, support, feeders)) in torch.iter_mut() {
            if *t != T::Pending {
                continue;
            }
            // (each feeder's state: a dead torch feeds nothing; a lit constant torch or a redstone
            // block no piston reaches is constant; any other source operates)
            let mut live: Vec<(BlockPos, bool)> = Vec::new();
            let mut pending = false;
            for q in feeders.iter() {
                match snapshot.get(q) {
                    Some(T::Dead) => {}
                    Some(T::Lit) => live.push((*q, true)),
                    Some(T::Op) => live.push((*q, false)),
                    Some(T::Pending) => pending = true,
                    None => live.push((*q, matches!(block_at(*q), redsim::block::Block::RedstoneBlock) && !piston_near(q))),
                }
            }
            if live.iter().any(|(_, constant)| !constant) {
                let q = live.iter().find(|(_, c)| !c).expect("one").0;
                *t = T::Op;
                *why = format!("its support {},{},{} is powered from {},{},{}", support.x, support.y, support.z, q.x, q.y, q.z);
                changed = true;
            } else if !pending && live.is_empty() {
                *t = T::Lit;
                *why = format!("its support {},{},{}'s only feeders are dead torches: nothing powers it", support.x, support.y, support.z);
                changed = true;
            } else if !pending {
                let q = live[0].0;
                *t = T::Dead;
                *why = format!(
                    "DEAD TORCH (a design error, G1 Nov 13 10:38 (b), ironspec 11:27 (a)): its support {},{},{} is powered only by CONSTANT sources (the {} at {},{},{}), so the torch is permanently off",
                    support.x,
                    support.y,
                    support.z,
                    if matches!(block_at(q), redsim::block::Block::RedstoneBlock) { "redstone block" } else { "constant lit torch" },
                    q.x,
                    q.y,
                    q.z
                );
                changed = true;
            }
            let _ = p;
        }
        if !changed {
            break;
        }
    }
    let mut out = Vec::new();
    for (p, b) in &spec.blocks {
        let Some((kind, reach)) = crate::pushscan::update_reach(*p, b) else { continue };
        // (tick-exact Nov 13 10:40, DetectorRailBlock.checkPressed 80-115: a detector's press
        // change notifies with a signal-source source its 6 neighbours and the 6 of the block under
        // it; tick-exact 14:04 in the game (row a/1072581002473618), cc767b78 14:01, ironspec 14:04
        // (2)(c): 26.3's LevelChunk.setBlockState 317-318 runs affectNeighborsAfterRemoval on every
        // rail state change, and BaseRailBlock.affectNeighborsAfterRemoval 117-127 updates
        // pos.above()'s neighbours for a SLOPE, naming the detector: an ascending detector also
        // reaches the 6 of the block above it. `update_reach` holds exactly that footprint, taken
        // as it stands (powered rails are no signal source: a plain T ignores their updates))
        let (operating, why) = match kind {
            "redstone block" => match pistons.iter().find(|q| (q.x - p.x).abs().max((q.y - p.y).abs()).max((q.z - p.z).abs()) <= 13) {
                Some(q) => (true, format!("a piston at {},{},{} within 13 could move it", q.x, q.y, q.z)),
                None => (false, "no piston within 13".to_string()),
            },
            "redstone torch" | "redstone wall torch" => match torch.get(p) {
                Some((T::Lit, why, _, _)) | Some((T::Dead, why, _, _)) => (false, why.clone()),
                Some((T::Op, why, _, _)) => (true, why.clone()),
                Some((T::Pending, _, support, _)) => (true, format!("its support {},{},{} sits in a torch cycle: taken as operating", support.x, support.y, support.z)),
                None => (true, "its output changes in operation".to_string()),
            },
            _ => (true, "its output changes in operation".to_string()),
        };
        out.push(SourceClass { pos: *p, block: b.to_string(), kind, reach, operating, why });
    }
    out
}

/// (ironspec Nov 13 09:44, G1 09:45: the T exposure static) Every plain rail with
/// countPotentialConnections() == 3 (rails on three sides, level, one up or one down: RailBlock's
/// re-shape condition), each signal source whose update footprint holds it, classed by
/// [`source_classes`] (DERIVED: CONSTANT or OPERATING with the reason), and whether the record
/// declares it in `switches:`. The bar: an UNDECLARED T inside an OPERATING source's footprint
/// FAILS (its shape can flip at runtime); a declared one is by design. Returns the lines and the
/// fails.
/// (G1 Nov 13 14:36 (ii), ironspec 14:37 (1)) A composed-row citation in a declaration's reason: a
/// `row:` / `rows:` part as its own token (at the reason's start or after "; ") naming a FILE (a word
/// with a `.` or a `/`), a TICK RANGE (`t A..B` or `tA..B`, or `ticks A..B`) and a CART (`cart N`), as
/// `row: record7f13t.txt t 120..180 cart 3`. Returns the cited part; a reason that only mentions
/// "row:" (an "arrow:", or a part lacking one of the three) cites nothing. G1 15:51: the citation
/// must be DURABLE: a repo path (relative), or any file with an artifact id (`a/NNNN`) or an md5 (12+
/// hex digits) beside it; an absolute path alone (`/tmp/…`, a box-local file no reviewer can open)
/// is "(not durable)" and cites nothing ([`row_citation_why`] says why).
pub fn row_citation(why: &str) -> Option<String> {
    row_citation_why(why).ok()
}

/// [`row_citation`] with the reason a reason cites nothing: no `row:` part with all three pieces, or
/// one whose file is not durable.
pub fn row_citation_why(why: &str) -> Result<String, String> {
    row_citation_why_in(why, &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// (G1 Nov 13 18:37, B10–B12) [`row_citation_why`] against the repository at `repo_root`: a relative
/// file is durable only when it is COMMITTED there (`git cat-file -e HEAD:<file>`: the commit's tree,
/// so a staged-but-uncommitted file is not, and no pathspec globbing); a git that can't answer (not
/// installed, not a repository: exit 128) is named as such, never read as "not in the repo".
pub fn row_citation_why_in(why: &str, repo_root: &std::path::Path) -> Result<String, String> {
    let range = |w: &str| -> bool {
        let w = w.trim_end_matches([',', ';', ')']);
        w.split_once("..").is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()))
    };
    let mut not_durable: Option<String> = None;
    for part in std::iter::once(why).chain(why.match_indices("; ").map(|(i, _)| &why[i + 2..])) {
        let part = part.trim_start();
        let Some(rest) = part.strip_prefix("rows:").or_else(|| part.strip_prefix("row:")) else { continue };
        let rest = rest.split("; ").next().unwrap_or("");
        let words: Vec<&str> = rest.split_whitespace().map(|w| w.trim_end_matches([',', ';', ')'])).collect();
        let files: Vec<&str> = words.iter().copied().filter(|w| (w.contains('.') && !range(w) && w.chars().any(|c| c.is_alphabetic())) || w.contains('/')).collect();
        let ticks = words.windows(2).any(|p| (p[0] == "t" || p[0] == "ticks") && range(p[1])) || words.iter().any(|w| w.strip_prefix('t').is_some_and(&range));
        let cart = words.windows(2).any(|p| p[0] == "cart" && !p[1].is_empty());
        if files.is_empty() || !ticks || !cart {
            continue;
        }
        // (G1 16:36 (notes): an artifact id anchored at the word's start or after "/" (`a/NNNN`,
        // `(a/NNNN)`, `…/a/NNNN`), not inside a word ("data/123"); an md5 either named (`md5 <hex>`)
        // or exactly 12 or 32 hex digits)
        let artifact = words.iter().any(|w| {
            let w = w.trim_start_matches('(');
            let id = w.strip_prefix("a/").or_else(|| w.rsplit_once("/a/").map(|(_, n)| n));
            id.is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        });
        let hex = |w: &str| w.chars().all(|c| c.is_ascii_hexdigit()) && w.chars().any(|c| c.is_ascii_digit());
        let md5 = words.windows(2).any(|p| p[0] == "md5" && p[1].len() >= 12 && hex(p[1])) || words.iter().any(|w| (w.len() == 12 || w.len() == 32) && hex(w));
        if artifact || md5 {
            return Ok(rest.trim().to_string());
        }
        // (G1 16:36, CHANGES 1, and 18:37 B10 / B11: a relative path cites only when committed in the
        // repository; git's own failure is named)
        let mut git_failed: Option<String> = None;
        let mut committed = false;
        for f in files.iter().filter(|f| !f.starts_with('/') && !f.starts_with('~') && !f.split('/').any(|c| c == "..")) {
            match std::process::Command::new("git").arg("-C").arg(repo_root).args(["cat-file", "-e", &format!("HEAD:{f}")]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).output() {
                Ok(o) if o.status.success() => committed = true,
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                    // (exit 128 with "not a git repository" or a missing HEAD itself: git can't answer;
                    // ironspec 19:15 (b): only the BARE "HEAD" form: "Not a valid object name HEAD:<path>"
                    // is a missing path, i.e. not committed)
                    let bare_head = err.lines().any(|l| {
                        let l = l.trim_end().trim_end_matches('.');
                        l.ends_with("Not a valid object name HEAD") || l.ends_with("invalid object name 'HEAD'")
                    });
                    if err.contains("not a git repository") || bare_head {
                        git_failed = Some(format!("git could not check {}: {err}", repo_root.display()));
                    }
                }
                Err(e) => git_failed = Some(format!("git could not run: {e}")),
            }
        }
        if committed {
            return Ok(rest.trim().to_string());
        }
        let local: Vec<&&str> = files.iter().filter(|f| f.starts_with('/') || f.starts_with('~')).collect();
        not_durable = Some(if let Some(g) = git_failed {
            format!("not durable: {} (unchecked: {g})", files.join(" "))
        } else if local.is_empty() {
            format!("not durable: {} (not in the repo: not committed at HEAD; cite a committed repo path, or add its artifact id or md5)", files.join(" "))
        } else {
            format!("not durable: {} (a box-local path: add its artifact id or md5, or cite a repo path)", files.join(" "))
        });
    }
    Err(not_durable.unwrap_or_else(|| "no `row:` part with a file, a tick range and a cart".to_string()))
}

/// (onechunk / farmopt Nov 13 13:54, ironspec 13:54) The `update-only:` sources checked against
/// their own update footprints (`source_classes`, the game's: a detector's is n6(D) ∪ n6(D.below()),
/// plus n6(D.above()) for an ascending one, tick-exact 14:04): each entry's source must exist, and the Ts its footprint reaches must match what
/// the entry says. An entry whose reason says `reaches no T` asserts an empty reach; any other
/// entry asserts the source updates a T (the key's meaning: its updates only re-settle a switched T
/// to its own power's shape), and each T it reaches must be declared in `switches:`. A mismatch
/// FAILS (a stale reading). An entry covers only the Ts it names; the others in that reach are
/// judged on their own by `t_exposure` (ironspec 13:54).
pub fn update_only_check(spec: &FarmSpec, entries: &[(BlockPos, String)], switches: &[(BlockPos, String)]) -> (Vec<String>, usize) {
    let rails: HashMap<BlockPos, String> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let ts: Vec<BlockPos> = rails.iter().filter(|(_, s)| s.contains("minecraft:rail[")).map(|(c, _)| *c).filter(|c| Track::arm_rails(&rails, *c).iter().filter(|v| !v.is_empty()).count() == 3).collect();
    let classes = source_classes(spec);
    let (mut lines, mut flags) = (Vec::new(), 0usize);
    let cell = |p: &BlockPos| format!("{},{},{}", p.x, p.y, p.z);
    for (src, why) in entries {
        let Some(k) = classes.iter().find(|k| k.pos == *src) else {
            flags += 1;
            lines.push(format!("  UPDATE-ONLY {}: FLAG: no signal source at this cell (the entry names nothing the game updates from)", cell(src)));
            continue;
        };
        let mut reached: Vec<BlockPos> = ts.iter().filter(|t| k.reach.contains(t)).copied().collect();
        reached.sort_by_key(|p| (p.y, p.z, p.x));
        let says_none = why.contains("reaches no T");
        let undeclared: Vec<String> = reached.iter().filter(|t| !switches.iter().any(|(q, _)| q == *t)).map(&cell).collect();
        let reach_txt = if reached.is_empty() { "no T".to_string() } else { format!("T(s) {}", reached.iter().map(&cell).collect::<Vec<_>>().join(" ")) };
        let verdict = match (says_none, reached.is_empty()) {
            (true, true) => "AGREES (the entry says it reaches no T)".to_string(),
            (false, false) if undeclared.is_empty() => "AGREES (each T it reaches is declared in `switches:`)".to_string(),
            (false, false) => {
                flags += 1;
                format!("FLAG: it reaches the undeclared T(s) {} (declare each in `switches:`)", undeclared.join(" "))
            }
            (true, false) => {
                flags += 1;
                "FLAG: the entry says it reaches no T, but its footprint holds one (a stale reading)".to_string()
            }
            (false, true) => {
                flags += 1;
                "FLAG: the entry says it updates a T, but its footprint reaches none (a stale reading)".to_string()
            }
        };
        lines.push(format!("  UPDATE-ONLY {} {} ({}): its update footprint reaches {reach_txt}: {verdict}; the entry: {why}", cell(src), k.kind, if k.operating { "OPERATING" } else { "CONSTANT" }));
    }
    (lines, flags)
}

pub fn t_exposure(spec: &FarmSpec, declared: &[(BlockPos, String)], integrity: &[(BlockPos, String)]) -> Result<(Vec<String>, usize), String> {
    let rails: HashMap<BlockPos, String> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    let classes = source_classes(spec);
    // (ironspec Nov 13 10:47 (2), the rule made exact: the game's own re-settle shape, from redsim's
    // RailState on the settled world: the CONNECTABLE arms (canConnectTo) and the cell's own power
    // (hasNeighborSignal) now; for every T within any source's reach the stored shape must equal it)
    let w = settled_world_void(spec)?;
    let mut lines = Vec::new();
    let mut fails = 0usize;
    let mut ts: Vec<(BlockPos, String)> = rails.iter().filter(|(_, s)| s.contains("minecraft:rail[")).map(|(c, s)| (*c, s.clone())).filter(|(c, _)| Track::arm_rails(&rails, *c).iter().filter(|v| !v.is_empty()).count() == 3).collect();
    ts.sort_by_key(|(c, _)| (c.y, c.z, c.x));
    for (c, s) in &ts {
        let near: Vec<&SourceClass> = classes.iter().filter(|k| k.reach.contains(c)).collect();
        let operating: Vec<String> = near.iter().filter(|k| k.operating).map(|k| format!("{},{},{} {} ({}): OPERATING, {}", k.pos.x, k.pos.y, k.pos.z, k.kind, k.block.trim_start_matches("minecraft:"), k.why)).collect();
        let constants: Vec<String> = near.iter().filter(|k| !k.operating).map(|k| format!("{},{},{} {}: CONSTANT, {}", k.pos.x, k.pos.y, k.pos.z, k.kind, k.why)).collect();
        // (the game's re-settle: RailState.place from the connectable arms, by the cell's power)
        let state = w.get(*c);
        let stored = redsim::rails::shape_of(&state);
        let r = redsim::rails::RailState::new(*c, state);
        let raw = r.count_potential_connections(&w);
        let signal = w.has_neighbor_signal(*c);
        let (now, unpowered, powered) = (r.shape_for(&w, signal, stored), r.shape_for(&w, false, stored), r.shape_for(&w, true, stored));
        let shape_bad = !near.is_empty() && raw == 3 && now != stored;
        let shape_note = format!(
            "; the game's re-settle (connectable arms, its power {}): {} now (unpowered {}, powered {}){}",
            if signal { "ON" } else { "off" },
            now.name(),
            unpowered.name(),
            powered.name(),
            if shape_bad { format!(": SHAPE MISMATCH, stored {} (ironspec 10:47 (2): a T within a source's reach must store the shape the game re-settles it to)", stored.name()) } else { String::new() }
        );
        let decl = declared.iter().find(|(p, _)| p == c).map(|(_, w)| w.as_str());
        // (58f770cc / G1 10:46: a T whose arms can't all join (a neighbour already full with 2
        // connections) re-resolves to the same shape on any update: HELD while that neighbour
        // stays full; the condition is printed by name)
        let (_, per) = Track::joinable_arms(&rails, *c);
        let blocked: Vec<String> = per.iter().filter(|(_, ok)| !ok).map(|(m, _)| format!("{},{},{}", m.x, m.y, m.z)).collect();
        let held = !blocked.is_empty();
        // (ironspec Nov 13 10:54, 9fc3155f's proposal 11:01: a HELD T inside any source's reach
        // rests on its blocked arms staying full: each blocked arm's cell needs an `integrity:`
        // entry, its stated connections the settled world's own and its `holds T=shape` this T
        // at its re-settle shape; missing or stale FAILS)
        let mut integrity_bad: Vec<String> = Vec::new();
        let mut integrity_ok: Vec<String> = Vec::new();
        if held && !near.is_empty() {
            for (m, _) in per.iter().filter(|(_, ok)| !ok) {
                let Some((_, why)) = integrity.iter().find(|(q, _)| q == m) else {
                    integrity_bad.push(format!("{},{},{} has no `integrity:` entry", m.x, m.y, m.z));
                    continue;
                };
                let real: Vec<(i32, i32)> = {
                    let mut v: Vec<(i32, i32)> = redsim::rails::RailState::new(*m, w.get(*m)).connections().iter().map(|q| (q.x, q.z)).collect();
                    v.sort();
                    v
                };
                let parse3 = |t: &str| -> Option<BlockPos> {
                    let n: Vec<i32> = t.split(',').map(|s| s.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
                    (n.len() == 3).then(|| BlockPos::new(n[0], n[1], n[2]))
                };
                let stated: Option<Vec<(i32, i32)>> = why.split(';').next().and_then(|s| s.trim().strip_prefix("connections")).map(|s| {
                    let mut v: Vec<(i32, i32)> = s.split('+').filter_map(|t| parse3(t)).map(|q| (q.x, q.z)).collect();
                    v.sort();
                    v
                });
                let holds = why.split(';').nth(1).and_then(|s| s.trim().strip_prefix("holds")).and_then(|s| s.trim().split_once('=')).and_then(|(t, sh)| {
                    // (the shape is the entry's first word: a note may follow it, "north_east ((A)'s,
                    // local + (0,-27,-1))")
                    let word: String = sh.trim().chars().take_while(|ch| ch.is_ascii_alphabetic() || *ch == '_').collect();
                    parse3(t).map(|t| (t, word))
                });
                match (stated, holds) {
                    (Some(st), Some((t, sh))) if st == real && t == *c && sh == now.name() => integrity_ok.push(format!("{},{},{}", m.x, m.y, m.z)),
                    (st, h) => integrity_bad.push(format!(
                        "{},{},{}'s `integrity:` entry stale (stated {:?}, the settled world's connections {:?}; holds {:?}, this T {},{},{} re-settling {})",
                        m.x,
                        m.y,
                        m.z,
                        st,
                        real,
                        h,
                        c.x,
                        c.y,
                        c.z,
                        now.name()
                    )),
                }
            }
        }
        // (ironspec 10:47 (2): an undeclared T inside an operating footprint fails when its
        // re-settle shape changes with its power; one whose shape can't change (unpowered ==
        // powered: its connectable arms leave one shape) is HELD, its condition printed)
        let can_flip = unpowered != powered;
        let fail = shape_bad || !integrity_bad.is_empty() || (decl.is_none() && !operating.is_empty() && can_flip);
        fails += usize::from(fail);
        lines.push(format!(
            "  {} T {},{},{} {}: {} operating source(s) whose update footprint holds it{}{}{}{}{}",
            if fail {
                "FAIL"
            } else if decl.is_some() {
                "DECLARED"
            } else if !operating.is_empty() {
                "HELD"
            } else {
                "quiet"
            },
            c.x,
            c.y,
            c.z,
            s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("?"),
            operating.len(),
            if operating.is_empty() { String::new() } else { format!(" ({})", operating.join("; ")) },
            if constants.is_empty() { String::new() } else { format!("; {} constant: {}", constants.len(), constants.join("; ")) },
            shape_note,
            if held {
                format!(
                    "; HELD by its neighbours: the arm(s) {} can't join (each already carries 2 connections): it keeps its shape on any update while they stay full (the integrity condition: if one loses a connection, the T can re-shape){}{}",
                    blocked.join(", "),
                    if integrity_ok.is_empty() { String::new() } else { format!("; `integrity:` declared and matching for {}", integrity_ok.join(", ")) },
                    if integrity_bad.is_empty() { String::new() } else { format!("; INTEGRITY FAIL: {}", integrity_bad.join("; ")) }
                )
            } else {
                String::new()
            },
            decl.map_or(String::new(), |w| format!("; declared in `switches:`: {w}"))
        ));
    }
    // (G1 Nov 13 11:19: MUTUAL HOLDS) A HELD T's blocked arm that is itself a raw-3 T held back
    // through this one makes a cycle: each keeps its shape only while the other keeps its. Every
    // cycle of the held-by graph among the Ts is printed; it counts as held only when a CONSTANT
    // source reaches one of its members (that member's power, so its shape, never changes) or one
    // member is held through a non-T arm too (a fixed connection); otherwise it FAILS (unbroken:
    // the build-order test must cover both placement orders).
    {
        let tset: std::collections::HashSet<BlockPos> = ts.iter().map(|(c, _)| *c).collect();
        let held_by = |c: BlockPos| -> Vec<BlockPos> { Track::joinable_arms(&rails, c).1.into_iter().filter(|(_, ok)| !ok).map(|(m, _)| m).collect() };
        let mut seen_cycles: std::collections::BTreeSet<Vec<(i32, i32, i32)>> = std::collections::BTreeSet::new();
        for (c, _) in &ts {
            // (a DFS over held-by edges among the Ts, cycles through `c` up to 6 long)
            let mut stack: Vec<Vec<BlockPos>> = vec![vec![*c]];
            while let Some(path) = stack.pop() {
                let last = *path.last().expect("non-empty");
                for m in held_by(last) {
                    if !tset.contains(&m) {
                        continue;
                    }
                    if m == *c && path.len() >= 2 {
                        let mut key: Vec<(i32, i32, i32)> = path.iter().map(|q| (q.x, q.y, q.z)).collect();
                        key.sort();
                        if seen_cycles.insert(key) {
                            let constant_breaks = path.iter().find(|q| classes.iter().any(|k| !k.operating && k.reach.contains(q)));
                            let fixed_breaks = path.iter().find(|q| held_by(**q).iter().any(|a| !tset.contains(a)));
                            let names = path.iter().map(|q| format!("{},{},{}", q.x, q.y, q.z)).collect::<Vec<_>>().join(" -> ");
                            match (constant_breaks, fixed_breaks) {
                                (Some(q), _) => lines.push(format!("  MUTUAL HOLD {names} -> {},{},{}: broken by a CONSTANT source reaching {},{},{} (held)", c.x, c.y, c.z, q.x, q.y, q.z)),
                                (None, Some(q)) => lines.push(format!("  MUTUAL HOLD {names} -> {},{},{}: broken by {},{},{}'s fixed connection (a blocked arm that is no T; held)", c.x, c.y, c.z, q.x, q.y, q.z)),
                                (None, None) => {
                                    fails += 1;
                                    lines.push(format!("  FAIL MUTUAL HOLD {names} -> {},{},{}: each T held only through the other, no constant source or fixed connection breaks the cycle (G1 11:19: the build-order test must cover both placement orders)", c.x, c.y, c.z));
                                }
                            }
                        }
                    } else if !path.contains(&m) && path.len() < 6 {
                        let mut next = path.clone();
                        next.push(m);
                        stack.push(next);
                    }
                }
            }
        }
    }
    // (G1 Nov 13 10:38 (b): every DEAD torch (its support held powered by a constant source: the
    // torch permanently off) is a design error, listed by name and failing)
    let dead: Vec<String> = classes.iter().filter(|k| k.why.starts_with("DEAD TORCH")).map(|k| format!("  DEAD TORCH {},{},{} ({}): {}", k.pos.x, k.pos.y, k.pos.z, k.block.trim_start_matches("minecraft:"), k.why)).collect();
    fails += dead.len();
    lines.extend(dead.iter().cloned());
    // (ironspec Nov 13 15:05 (c): the tripwire hook, the design's own source kind (the hold strings,
    // the awake guard's string, the slot-1 strings), named wherever its footprint holds a T so
    // redsim's T tests pin that kind; none in reach of any T is printed too, and the pin skipped)
    let hooks: Vec<String> = classes
        .iter()
        .filter(|k| k.kind == "tripwire hook")
        .filter_map(|k| {
            let reached: Vec<String> = ts.iter().filter(|(c, _)| k.reach.contains(c)).map(|(c, _)| format!("{},{},{}", c.x, c.y, c.z)).collect();
            (!reached.is_empty()).then(|| format!("{},{},{} ({}) reaches T(s) {}", k.pos.x, k.pos.y, k.pos.z, if k.operating { "OPERATING" } else { "CONSTANT" }, reached.join(" ")))
        })
        .collect();
    let n_hooks = classes.iter().filter(|k| k.kind == "tripwire hook").count();
    lines.push(if hooks.is_empty() {
        format!("  TRIPWIRE HOOKS in reach of a T: none ({n_hooks} hook(s) in the design; redsim's hook-kind T pin is skipped, ironspec 15:05 (c))")
    } else {
        format!("  TRIPWIRE HOOKS in reach of a T (ironspec 15:05 (c): redsim's T tests pin this kind): {}", hooks.join("; "))
    });
    lines.insert(
        0,
        format!(
            "the T exposure (ironspec Nov 13 09:44, G1 09:51 / 10:38: every plain rail with countPotentialConnections == 3, the signal sources whose UPDATE FOOTPRINT (settle-check's update_reach per type) holds it, CONSTANT or OPERATING as derived (each source's reason printed); an undeclared T inside an operating source's footprint FAILS, a `switches:` T is by design; a dead torch (support held powered by a constant source) FAILS as a design error): {} T(s), {} declared, {} dead torch(es), {fails} failing{}",
            ts.len(),
            ts.iter().filter(|(c, _)| declared.iter().any(|(p, _)| p == c)).count(),
            dead.len(),
            if fails == 0 { " (PASS)" } else { " (FAIL)" }
        ),
    );
    Ok((lines, fails))
}

pub fn coverage_map(spec: &FarmSpec, columns: &[[f64; 4]], cells_only: &[BlockPos], feet_extra: &[(String, Vec3)]) -> Result<(Vec<String>, usize), String> {
    let o = RunOpts { ai: true, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    settle_full(&mut run)?;
    let w = &mut run.world;
    let zombies = world_zombies(w, spec)?;
    let (pills, unjudged) = design_hostiles(spec);
    if !unjudged.is_empty() {
        return Err(format!("coverage: hostile line(s) of a type redsim doesn't model, NOT JUDGED: {}", unjudged.join("; ")));
    }
    let w = &run.world;
    // (label, feet, eye, range)
    let mut hostiles: Vec<(String, Vec3, Vec3, f64)> = Vec::new();
    for z in &zombies {
        hostiles.push((format!("zombie {:.2},{:.3},{:.2}", z.x, z.y, z.z), *z, Vec3::new(z.x, z.y + zeye(), z.z), 8.0));
    }
    for p in &pills {
        hostiles.push((format!("pillager {:.2},{:.3},{:.2}", p.x, p.y, p.z), *p, Vec3::new(p.x, p.y + PILLAGER_EYE, p.z), 15.0));
    }
    let mut pos: Vec<(String, BlockPos, Vec3)> = rider_positions(spec).into_iter().map(|(c, t, p)| (format!("t {t:.2}"), c, p)).collect();
    let track = Track::new(spec, columns, &[]);
    for (k, _) in columns.iter().enumerate() {
        for i in 0..track.col_len[k] {
            let p = track.col_pos(k, i);
            pos.push((format!("column {k} #{i}"), BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32), p));
        }
    }
    if !cells_only.is_empty() {
        pos.retain(|(_, c, _)| cells_only.contains(c));
    }
    // (farmopt Nov 13 07:05: positions from the composed run's trace, H's cart on its trapdoor and
    // the lift column's riders included: each given feet position, its cell the floored one)
    for (lab, f) in feet_extra {
        pos.push((lab.clone(), BlockPos::new(f.x.floor() as i32, (f.y + 1e-6).floor() as i32, f.z.floor() as i32), *f));
    }
    let veye = veye();
    let mut by_cell: std::collections::BTreeMap<(i32, i32, i32), (usize, Vec<String>)> = std::collections::BTreeMap::new();
    let mut dark = 0usize;
    for (lab, c, feet) in &pos {
        let eye = Vec3::new(feet.x, feet.y + veye, feet.z);
        let d = |h: &Vec3| ((h.x - feet.x).powi(2) + (h.y - feet.y).powi(2) + (h.z - feet.z).powi(2)).sqrt();
        let in_scan = |h: &Vec3| (h.x - feet.x).abs() <= 16.6 && h.y >= feet.y - 17.95 && h.y <= feet.y + 17.95 && (h.z - feet.z).abs() <= 16.6;
        let covered = hostiles.iter().any(|(_, f, e, r)| d(f) <= *r && d(f) <= 16.0 && in_scan(f) && redsim::entity::mob::los::has_line_of_sight(w, eye, *e));
        let entry = by_cell.entry((c.y, c.z, c.x)).or_insert((0, Vec::new()));
        if covered {
            entry.0 += 1;
        } else {
            dark += 1;
            let nearest = hostiles.iter().filter(|(_, f, _, r)| d(f) <= *r).min_by(|a, b| d(&a.1).total_cmp(&d(&b.1)));
            let why = match nearest {
                Some((l, f, e, _)) => {
                    let hit = redsim::entity::mob::los::clip_collider(w, eye, *e);
                    format!("the nearest in range {l} at {:.2}, the clip from the eye {:.3},{:.3},{:.3} {}", d(f), eye.x, eye.y, eye.z, if hit.miss { "clear but outside the 16 scan".to_string() } else { format!("BLOCKED at {},{},{} ({})", hit.pos.x, hit.pos.y, hit.pos.z, w.get(hit.pos).to_string().trim_start_matches("minecraft:")) })
                }
                None => {
                    let n = hostiles.iter().min_by(|a, b| d(&a.1).total_cmp(&d(&b.1)));
                    format!("no hostile in its range (the nearest {})", n.map_or("none".to_string(), |(l, f, _, r)| format!("{l} at {:.2}, its range {r}", d(f))))
                }
            };
            entry.1.push(format!("{lab} feet {:.3},{:.4},{:.3}: {why}", feet.x, feet.y, feet.z));
        }
    }
    let mut lines = vec![format!(
        "night coverage (ironspec Nov 13 06:11: every position a rider can occupy at night under a scarer's sight; the sensor's test: the 16-block scan, zombie 8 / pillager 15, the eye-to-eye COLLIDER clip from the rider's eye, feet + {veye:.2}; GEOMETRIC, by day, no brain ticked): {} hostiles, {} positions, {} dark{}",
        hostiles.len(),
        pos.len(),
        dark,
        if pos.is_empty() { " (NOT JUDGED: 0 samples)" } else if dark == 0 { " (PASS)" } else { " (FAIL)" }
    )];
    for ((y, z, x), (n_cov, rows)) in &by_cell {
        if rows.is_empty() {
            continue;
        }
        lines.push(format!("  DARK {x},{y},{z}: {} dark, {n_cov} covered", rows.len()));
        for r in rows {
            lines.push(format!("    {r}"));
        }
    }
    let covered_cells = by_cell.values().filter(|(_, rows)| rows.is_empty()).count();
    lines.push(format!("  covered cells (every position seen): {covered_cells} of {}", by_cell.len()));
    Ok((lines, if pos.is_empty() { usize::MAX } else { dark }))
}

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
pub(crate) fn rail_exits(state: &str) -> Vec<(i32, i32, i32)> {
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
/// (58f770cc / ironspec Nov 13 16:34, ironspec 03:22) A GOLEM GUARD: a lone plain rail standing on a
/// top-half trapdoor cap (no collision; it keeps a golem's spawn space out of the cap's full top face),
/// with no rail beside it it could join (level, one up or one down): part of the record, never a track
/// end.
pub fn golem_guard(blocks: &HashMap<BlockPos, redsim::block::Block>, p: BlockPos) -> bool {
    let name = |q: BlockPos| blocks.get(&q).map(|b| b.to_string()).unwrap_or_default();
    let below = name(p.below());
    // (G1 / ironspec Nov 13 18:10, on part 13: the cap CLOSED: an open trapdoor has no top face, so
    // the rail pops at its next update (an unsupported rail, no guard))
    if !name(p).starts_with("minecraft:rail[") || !below.contains("_trapdoor[") || !below.contains("half=top") || !below.contains("open=false") {
        return false;
    }
    let lone = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|(dx, dz)| (-1..=1).all(|dy| !name(BlockPos::new(p.x + dx, p.y + dy, p.z + dz)).contains("rail")));
    lone && !name(p.above()).contains("rail") && !name(p.below().below()).contains("rail")
}

/// (ironspec Nov 13 19:32) The gate on `auto_columns`' lines: a side entry NOT JUDGED (its settle
/// failed) fails the run by name, never read as a pass.
pub fn auto_columns_judged(why: &[String]) -> Result<(), String> {
    let unjudged: Vec<&str> = why.iter().filter(|l| l.contains("NOT JUDGED")).map(|s| s.as_str()).collect();
    if unjudged.is_empty() {
        Ok(())
    } else {
        Err(format!("auto columns: NOT JUDGED: {} side entr(ies) whose settle failed: {}", unjudged.len(), unjudged.join(" | ")))
    }
}

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
        ys.sort();
        let mut start = 0;
        for k in 1..=ys.len() {
            if k == ys.len() || ys[k].0 != ys[k - 1].0 + 1 {
                let (lo, hi, down) = (ys[start].0, ys[k - 1].0, ys[start].1);
                // (G1 Nov 13, N15: a declared column at this x,z covers only the bubble runs its y
                // range meets; another run in the same x,z is still a column)
                if declared.iter().any(|c| c[0].floor() as i32 == x && c[2].floor() as i32 == z && c[1].min(c[3]) <= hi as f64 + 1.0 && c[1].max(c[3]) >= lo as f64) {
                    lines.push(format!("auto column: x {x} z {z} y {lo}..{hi}: the declared column there covers it"));
                    start = k;
                    continue;
                }
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
    // (the settled world, read only when a rail end opens into a column's side: G1 18:41)
    let mut settled: Option<Result<redsim::World, String>> = None;
    let mut rails: Vec<(BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    rails.sort_by_key(|(p, _)| (p.y, p.z, p.x));
    for (c, _) in rails.iter().filter(|(c, _)| golem_guard(&blk, *c)) {
        lines.push(format!("auto column: {},{},{} is a golem guard (a lone rail on a top-half trapdoor cap, 58f770cc 16:34): no track end, no fall", c.x, c.y, c.z));
    }
    rails.retain(|(c, _)| !golem_guard(&blk, *c));
    for (c, s) in &rails {
        for (dx, dy, dz) in rail_exits(s) {
            if dx == 0 && dz == 0 {
                continue;
            }
            let m = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
            if rail(m) || rail(m.below()) || !open(m) || !open(m.below()) {
                continue;
            }
            // (ironspec Nov 13 18:31: a rail end opening into a column's side, inside its y range, is
            // the column's side entry, not a fall; G1 18:41 / 19:23: credited only where the SETTLED
            // world holds a LIFTING bubble column (soul sand) at that cell, its y range read there; a
            // magma column pulls the cart down to its run's bottom, a landing the fall logic judges; a
            // column with no bubble block there, or a failed settle (NOT JUDGED), credits nothing;
            // G1 20:02 E3: and then no end rule credits it either: the fall is checked; E4: the walk
            // takes the entry as a column of its own from the entry cell: up the lift to its top, or
            // down the magma run to its bottom, where no rail is a track end)
            let mut side_refused = false;
            if let Some(k) = declared.iter().chain(out.iter()).find(|k| k[0].floor() as i32 == m.x && k[2].floor() as i32 == m.z && k[1].min(k[3]) <= m.y as f64 && m.y as f64 <= k[1].max(k[3])).copied() {
                if settled.is_none() {
                    settled = Some(settled_world_void(spec));
                }
                let side = format!("the rail end {},{},{} into {},{},{} opens into the column x {:.1} z {:.1} y {:.1}..{:.1}'s side", c.x, c.y, c.z, m.x, m.y, m.z, k[0], k[2], k[1], k[3]);
                let mut walk_col: Option<[f64; 4]> = None;
                let how = match settled.as_ref() {
                    Some(Ok(sw)) => {
                        let bubble = |y: i32| {
                            let s = sw.get(BlockPos::new(m.x, y, m.z)).to_string();
                            s.contains("bubble_column").then(|| s.contains("drag=true"))
                        };
                        match bubble(m.y) {
                            Some(down) => {
                                let (mut lo, mut hi) = (m.y, m.y);
                                while bubble(lo - 1) == Some(down) {
                                    lo -= 1;
                                }
                                while bubble(hi + 1) == Some(down) {
                                    hi += 1;
                                }
                                if down {
                                    // (ironspec 19:26: a magma column's side entry is a DESCENT judged to
                                    // the column's bottom; the run stands on magma (or a bubble block),
                                    // never a rail: a track end)
                                    let bottom = BlockPos::new(m.x, lo, m.z);
                                    let under = sw.get(bottom.below());
                                    walk_col = Some([m.x as f64 + 0.5, m.y as f64, m.z as f64 + 0.5, lo as f64]);
                                    Some(format!(
                                        "the settled world's bubble column y {lo}..{hi} there pulls it DOWN (magma): a DESCENT to y {lo}, onto {} at {},{},{}: no rail: a TRACK END",
                                        under.to_string().trim_start_matches("minecraft:"),
                                        bottom.x,
                                        bottom.y - 1,
                                        bottom.z
                                    ))
                                } else {
                                    // (G1 21:38 N4: the walk column runs up the SETTLED run to its exit cell, the
                                    // range the line prints, never the declaration's top)
                                    let top = if open(BlockPos::new(m.x, hi + 1, m.z)) { hi + 1 } else { hi };
                                    walk_col = Some([k[0], m.y as f64, k[2], top as f64]);
                                    Some(format!("the settled world's bubble column y {lo}..{hi} there LIFTS it (soul sand), up to y {top}"))
                                }
                            }
                            None => {
                                let declared_k = declared.iter().any(|d| *d == k);
                                lines.push(format!("auto column: {side}, but {} at {},{},{}: not credited, the fall checked instead", if declared_k { "a declared column, no bubble block here" } else { "the settled world holds no bubble column" }, m.x, m.y, m.z));
                                side_refused = true;
                                None
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // (ironspec 19:15 (2) / G1 19:23: a failed settle credits nothing: NOT JUDGED,
                        // the end falls through to the fall / track-end logic, the error printed)
                        lines.push(format!("auto column: {side}: NOT JUDGED, the settle FAILED ({e}): no side entry credited, the fall checked instead"));
                        side_refused = true;
                        None
                    }
                    None => None,
                };
                if let Some(how) = how {
                    lines.push(format!("auto column: the rail end {},{},{} into {},{},{} enters the column x {:.1} z {:.1} y {:.1}..{:.1} from its side: {how}; no fall of its own", c.x, c.y, c.z, m.x, m.y, m.z, k[0], k[2], k[1], k[3]));
                    if let Some(col) = walk_col {
                        out.push(col);
                    }
                    continue;
                }
            }
            if let Some(k) = declared.iter().chain(out.iter()).find(|k| near(first(k), *c) || near(last(k), *c)).filter(|_| !side_refused) {
                // (G1 Nov 13, N15: listed, never skipped silently)
                lines.push(format!("auto column: the rail end {},{},{} into {},{},{} is within 1 of the column x {:.1} z {:.1} y {:.1}..{:.1}'s end: taken as that column's entry or exit, no fall of its own", c.x, c.y, c.z, m.x, m.y, m.z, k[0], k[2], k[1], k[3]));
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
            // (ironspec Nov 13 18:09, on part 13: a fall onto a golem guard lands off the track: the
            // guard is no rail of the track, so a cart that could end on it is flagged UNLANDED)
            if let Some((l, true)) = land {
                if golem_guard(&blk, l) {
                    lines.push(format!("auto column: a free fall off the rail end {},{},{} into {},{},{} lands on the golem guard {},{},{} (no rail of the track): a TRACK END (UNLANDED)", c.x, c.y, c.z, m.x, m.y, m.z, l.x, l.y, l.z));
                    continue;
                }
            }
            let Some((l, on_rail)) = land else {
                // (ironspec Nov 13 06:00 real, POPULATIONS: a fall with no landing in the design's
                // box is listed as a track end, never skipped)
                lines.push(format!("auto column: a free fall off the rail end {},{},{} into {},{},{} lands nowhere in the design's box (down past y {}): a TRACK END (UNLANDED)", c.x, c.y, c.z, m.x, m.y, m.z, spec.lo.y));
                continue;
            };
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
    settle_full(&mut run)?;
    let w = &mut run.world;
    apply_claims(w, claims)?;
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
        // (tick-exact Nov 13 (2), G1 N28: a seated rider is never on the ground: the scan-down start)
        w.entities.list[i].on_ground = false;
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
    settle_full(&mut run)?;
    let w = &mut run.world;
    let cart = redsim::entity::spawn(w, "minecraft:minecart", [cart_at.x, cart_at.y, cart_at.z], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}"))?;
    let ci = redsim::entity::index_of(w, cart).ok_or("the cart")?;
    let vid = *w.entities.list[ci].passengers.first().ok_or("the rider")?;
    let i = redsim::entity::index_of(w, vid).ok_or("the rider")?;
    w.entities.list[ci].pos = cart_at;
    redsim::entity::position_rider(w, ci, i);
    // (tick-exact Nov 13 (2), G1 N28: a seated rider is never on the ground: the scan-down start)
    w.entities.list[i].on_ground = false;
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
    settle_full(&mut run)?;
    let w = &mut run.world;
    // (G1 Nov 13, N6 / N19: one hostile-position rule, the gate's: a boat's zombie seated, a
    // standing one at its feet, from the world)
    let zs = world_zombies(w, spec)?;
    let w = &run.world;
    let dist = |a: Vec3, b: Vec3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let mut lines = Vec::new();
    let mut blind = 0usize;
    for (name, p) in callers {
        let eye = Vec3::new(p.x, p.y + veye(), p.z);
        let (mut seen, mut hidden): (Vec<(f64, String)>, Vec<(f64, String)>) = (Vec::new(), Vec::new());
        for z in &zs {
            let f = *z;
            let d = dist(f, *p);
            if d > 8.0 {
                continue;
            }
            let s = redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + zeye(), f.z));
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
    /// (G1's note 4 on a4cd9b8b0; G1 Nov 13 09:02 / 09:58) The switched Ts: plain rails with three
    /// neighbouring rails that can join them (level, one up or one down) inside the update footprint
    /// of an OPERATING signal source (`source_classes`: derived, a constant source holds a shape),
    /// or the record's `switches:` Ts when it declares them: a cart may take either shape, so their
    /// moves are the union of every arm, and their shapes orient nothing.
    switched_ts: std::collections::HashSet<BlockPos>,
    /// The cells with a collision shape, with its boxes (1/16): a rail exit into one whose boxes
    /// meet the cart's is a wall the cart stops at, not a track end (the open ends are falls and
    /// roll-offs; G1's note (a) on fa1267b75: a carpet, a snow layer, an open trapdoor on the far
    /// side let the cart pass).
    solid: HashMap<BlockPos, Vec<redsim::behavior::Box16>>,
    /// (G1 Nov 13 09:42) The links a `--travel` / `# travel:` declaration orients against the
    /// direction the shapes / chains read: each FAILS the run (`orient` fills it).
    declared_conflicts: Vec<String>,
    /// (ironspec Nov 13 13:50 (b)) The links `orient` INFERRED (a junction completion, a forward
    /// exit) that no declaration confirms: a cited run's late verdict is NOT JUDGED while any stands
    /// (a `# travel:` line naming the move, with the run or composed rows that show it, confirms one).
    inferred_links: Vec<String>,
}

impl<'a> Track<'a> {
    fn new(spec: &FarmSpec, columns: &'a [[f64; 4]], rests: &'a [BlockPos]) -> Track<'a> {
        // (58f770cc / ironspec 16:34: a golem guard is no rail of the track: never a track end)
        let blk: HashMap<BlockPos, redsim::block::Block> = spec.blocks.iter().map(|(p, b)| (*p, *b)).collect();
        let rails: HashMap<BlockPos, String> = spec.blocks.iter().filter(|(p, b)| b.to_string().contains("rail") && !golem_guard(&blk, *p)).map(|(p, b)| (*p, b.to_string())).collect();
        // (G1's note (c) on fa1267b75: the signal sources are the game's `isSignalSource` set;
        // `update_reach` (settle-check's) holds that set with each type's update footprint)
        // (ironspec Nov 13 09:57, G1 09:58: a T is switched only inside the update footprint of an
        // OPERATING source (`source_classes`, derived); a constant source holds its shape)
        let sources: Vec<BlockPos> = source_classes(spec).into_iter().filter(|k| k.operating).flat_map(|k| k.reach).collect();
        let solid = spec.blocks.iter().map(|(p, b)| (*p, redsim::behavior::collision_boxes(b))).filter(|(_, v)| !v.is_empty()).collect();
        Track::from_parts(rails, columns, rests, sources, solid)
    }

    /// A track from its rail cells (game strings), columns, rests and signal sources.
    fn from_parts(rails: HashMap<BlockPos, String>, columns: &'a [[f64; 4]], rests: &'a [BlockPos], sources: Vec<BlockPos>, solid: HashMap<BlockPos, Vec<redsim::behavior::Box16>>) -> Track<'a> {
        let col_len = columns.iter().map(|c| ((c[3] - c[1]).abs() / 0.25).round() as usize + 1).collect();
        let mut switched_ts = std::collections::HashSet::new();
        for (c, s) in &rails {
            if !s.contains("minecraft:rail[") {
                continue;
            }
            // (tick-exact's official junction row: RailBlock.updateState re-shapes a plain rail only
            // when countPotentialConnections() == 3 (RailBlock.java 22-25): a 4th rail in a side's
            // column, a detector beside it say, keeps its placed shape for good)
            let (arms, sides) = Track::joinable_arms(&rails, *c);
            let joinable = sides.iter().filter(|(_, ok)| *ok).count();
            // (`sources`: the cells inside an OPERATING source's update footprint: Track::new derives
            // them; a T there with three joinable arms can re-shape)
            if arms == 3 && joinable == 3 && sources.contains(c) {
                switched_ts.insert(*c);
            }
        }
        Track { rails, columns, col_len, rests, allowed: None, switched_ts, solid, declared_conflicts: Vec::new(), inferred_links: Vec::new() }
    }

    /// (G1 Nov 13 09:02; 58f770cc 10:46) A rail cell's potential connections (`arms`: the sides
    /// with a rail level, one up or one down: countPotentialConnections, raw presence) and, per
    /// side, the first rail there and whether it CAN join this one: RailState.place re-shapes from
    /// the neighbours that can connect (hasConnection: the neighbour, its soft connections removed,
    /// `canConnectTo` this rail: it already points here, or it holds fewer than 2 connections); a
    /// neighbour whose shape points to two other rails that point back (a full one) never joins.
    fn joinable_arms(rails: &HashMap<BlockPos, String>, c: BlockPos) -> (usize, Vec<(BlockPos, bool)>) {
        let targets = |c: BlockPos| -> Vec<BlockPos> {
            let mut out = Vec::new();
            for (dx, dy, dz) in rail_exits(rails.get(&c).map(|s| s.as_str()).unwrap_or("")) {
                for m in [BlockPos::new(c.x + dx, c.y + dy, c.z + dz), BlockPos::new(c.x + dx, c.y + dy - 1, c.z + dz)] {
                    if rails.contains_key(&m) {
                        out.push(m);
                    }
                }
            }
            out
        };
        let hard = |m: BlockPos| targets(m).into_iter().filter(|t| targets(*t).contains(&m)).count();
        let can_connect = |m: BlockPos, c: BlockPos| targets(m).contains(&c) || hard(m) < 2;
        let sides = Track::arm_rails(rails, c);
        let arms = sides.iter().filter(|v| !v.is_empty()).count();
        let per: Vec<(BlockPos, bool)> = sides.iter().filter(|v| !v.is_empty()).map(|v| (v[0], v.iter().any(|m| can_connect(*m, c)))).collect();
        (arms, per)
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
    /// (G1's note (a) on fa1267b75) Whether the cell `m` beyond the rail `c`'s exit `e` stops a
    /// cart: one of its collision boxes meets the cart's box (0.98 wide, from the rail floor +
    /// 0.0625 to + 0.7625) as the cart's centre crosses into the cell, at the exit's edge midpoint
    /// (a slope's high exit one up).
    fn blocks_cart(&self, c: BlockPos, e: (i32, i32, i32), m: BlockPos) -> bool {
        let Some(boxes) = self.solid.get(&m) else { return false };
        let (ex, ez) = (c.x as f64 + 0.5 + e.0 as f64 * 0.5, c.z as f64 + 0.5 + e.2 as f64 * 0.5);
        let floor = (c.y + e.1) as f64;
        let cb = [ex - 0.49, ex + 0.49, floor + 0.0625, floor + 0.7625, ez - 0.49, ez + 0.49];
        boxes.iter().any(|b| {
            let wb = [m.x as f64 + b[0] as f64 / 16.0, m.x as f64 + b[1] as f64 / 16.0, m.y as f64 + b[2] as f64 / 16.0, m.y as f64 + b[3] as f64 / 16.0, m.z as f64 + b[4] as f64 / 16.0, m.z as f64 + b[5] as f64 / 16.0];
            wb[0] < cb[1] && wb[1] > cb[0] && wb[2] < cb[3] && wb[3] > cb[2] && wb[4] < cb[5] && wb[5] > cb[4]
        })
    }

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
        for (path, why) in travel {
            // (G1 Nov 13 13:56: a `travel` line confirms its moves only when it cites a composed row
            // (file, tick range, cart) as `row:` / `rows:` in its reason; an uncited one still orients
            // the walk but its moves stay unconfirmed, listed with the inferred links)
            let cited = match row_citation_why(why) {
                Ok(_) => true,
                Err(e) => {
                    warn.push(format!("--travel {}: UNCITED ({e})", path.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect::<Vec<_>>().join(">")));
                    false
                }
            };
            for w in path.windows(2) {
                let (a, b) = (rail(&w[0]), rail(&w[1]));
                if e.contains(&(a, b)) {
                    if f.insert((a, b)) {
                        queue.push((a, b));
                    }
                    src.insert((a, b), if cited { "declared" } else { "declared (UNCITED)" });
                    by_decl += 1;
                } else {
                    warn.push(format!("--travel {},{},{}>{},{},{}: no move of the track joins them that way", w[0].x, w[0].y, w[0].z, w[1].x, w[1].y, w[1].z));
                }
            }
        }
        // (a cart leaves a 2-arm cell by its other arm; a junction with every known arm one way
        // and one unknown has that one the other way)
        fn propagate(e: &BTreeSet<(TailNode, TailNode)>, adj: &std::collections::BTreeMap<TailNode, BTreeSet<TailNode>>, f: &mut BTreeSet<(TailNode, TailNode)>, src: &mut std::collections::BTreeMap<(TailNode, TailNode), &'static str>, mut queue: Vec<(TailNode, TailNode)>, chain_label: &'static str, shape_dirs: &std::collections::BTreeMap<TailNode, Vec<(i32, i32)>>) {
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
                        } else if let (TailNode::Rail(jx, _, jz), Some(dirs)) = (j, shape_dirs.get(&j)) {
                            // (G1 Nov 13 09:02, the forward exit: a cart leaves a cell only by a
                            // shape exit, the one ahead of its motion: through a shape arm, the
                            // other shape arm; from a free side, the shape exit opposite it (a
                            // curve's free side faces one of its exits). With one unknown SHAPE arm
                            // that no inflow's forward exit is, the arm can only be an inflow: a
                            // merge curve's other arm, the siding's S-bend into C)
                            let dir_of = |n: &TailNode| match n {
                                TailNode::Rail(x, _, z) => Some(((*x - jx).signum(), (*z - jz).signum())),
                                TailNode::Col(..) => None,
                            };
                            let u = unknown[0];
                            if let Some(ud) = dir_of(&u)
                                && dirs.contains(&ud)
                            {
                                let fwd = |a: &TailNode| -> Option<(i32, i32)> {
                                    let d = dir_of(a)?;
                                    if dirs.contains(&d) {
                                        dirs.iter().find(|x| **x != d).copied()
                                    } else {
                                        let o = (-d.0, -d.1);
                                        dirs.contains(&o).then_some(o)
                                    }
                                };
                                let fwds: Vec<Option<(i32, i32)>> = arms.iter().filter(|x| f.contains(&(**x, j))).map(fwd).collect();
                                if !fwds.is_empty() && fwds.iter().all(|x| x.is_some()) && !fwds.contains(&Some(ud)) {
                                    next.push((u, j, "forward exit (INFERRED)"));
                                }
                            }
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
        // (the forward-exit rule's shapes: each fixed rail's two exits, horizontally)
        let shape_dirs: std::collections::BTreeMap<TailNode, Vec<(i32, i32)>> = self
            .rails
            .iter()
            .filter(|(c, _)| !self.switched_ts.contains(c))
            .map(|(c, s)| (rail(c), rail_exits(s).into_iter().map(|(dx, _, dz)| (dx, dz)).collect::<Vec<_>>()))
            .filter(|(_, v)| v.len() == 2)
            .collect();
        propagate(&e, &adj, &mut f, &mut src, queue, "chain", &shape_dirs);
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
        propagate(&e, &adj, &mut f, &mut src, rq, "reversal chain", &shape_dirs);
        let name = |n: &TailNode| match n {
            TailNode::Rail(x, y, z) => format!("{x},{y},{z}"),
            TailNode::Col(k, i) => {
                let p = self.col_pos(*k, *i);
                format!("column {:.2},{:.2},{:.2}", p.x, p.y, p.z)
            }
        };
        let mut allowed: HashSet<(TailNode, TailNode)> = HashSet::new();
        let mut decl_conf: Vec<String> = Vec::new();
        let (mut one_way, mut undecided, mut both, mut conflict): (usize, BTreeSet<String>, usize, BTreeSet<String>) = (0, BTreeSet::new(), 0, BTreeSet::new());
        for (a, b) in &e {
            let (fwd, back) = (f.contains(&(*a, *b)), f.contains(&(*b, *a)));
            if fwd || !back {
                allowed.insert((*a, *b));
            }
            let key = if a < b { format!("{}-{}", name(a), name(b)) } else { format!("{}-{}", name(b), name(a)) };
            if fwd && back {
                if f1.contains(&(*a, *b)) && f1.contains(&(*b, *a)) {
                    // (G1 Nov 13 09:42: a DECLARED link against the shape-read direction FAILS the
                    // run; orient's own chain-vs-chain conflicts are printed)
                    if src.get(&(*a, *b)).is_some_and(|s| s.starts_with("declared")) || src.get(&(*b, *a)).is_some_and(|s| s.starts_with("declared")) {
                        decl_conf.push(key.clone());
                    }
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
            "travel (the direction-aware walk, ironspec Nov 12 16:02; --two-way for the old walk): {one_way} moves one way ({by_col} column moves, {by_shape} links one way by their rails' shapes away from switched Ts ({} switched Ts: three joinable arms inside an OPERATING source's update footprint (source_classes, derived) or declared in `switches:`: every arm walked, their shapes orient nothing), {by_decl} declared --travel steps, the rest carried along chains and through junctions), {both} reversal moves both ways (--reversal), {} links both ways where nothing orients them, {} links oriented both ways (a conflict: kept both ways)",
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
        if !decl_conf.is_empty() {
            out.push(format!("  DECLARED AGAINST THE SHAPES (G1 Nov 13 09:42: FAILS the run; the geometry or the declaration is wrong): {}", decl_conf.join("; ")));
        }
        let decl_conf_keep = decl_conf;
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
            ["column", "shape", "declared", "declared (UNCITED)", "chain", "junction (INFERRED)", "forward exit (INFERRED)", "reversal", "reversal chain"].iter().map(|k| format!("{} {k}", src.values().filter(|s| *s == k).count())).collect::<Vec<_>>().join(", "),
            if list { "" } else { " (--orient-list prints every one)" }
        ));
        // (ironspec Nov 13 13:50 (b): every INFERRED link listed, always: a declared move would read
        // "declared", so each one here is unconfirmed, and a cited run's late verdict is NOT JUDGED
        // until a `# travel:` line confirms it with the run or the composed rows that show it; G1
        // 13:56: a `travel` line citing no composed row confirms nothing, its moves listed too)
        let mut inferred_links: Vec<String> = src
            .iter()
            .filter(|(_, s)| s.contains("(INFERRED)") || s.contains("(UNCITED)"))
            .filter(|((a, b), _)| !matches!(a, TailNode::Col(..)) || !matches!(b, TailNode::Col(..)))
            .map(|((a, b), s)| format!("{} > {} ({s})", name(a), name(b)))
            .collect();
        inferred_links.sort();
        for l in &inferred_links {
            if l.contains("(UNCITED)") {
                out.push(format!("  UNCONFIRMED LINK {l}: its `travel` line cites no composed row (G1 13:56 / 14:36: `row: FILE t A..B cart N` as its own part of the reason); until then the late verdict is NOT JUDGED"));
            } else {
                out.push(format!("  INFERRED LINK {l}: unconfirmed (confirm it with a `# travel:` line naming the move and citing the composed row that shows it, `row: FILE t A..B cart N`; until then the late verdict is NOT JUDGED)"));
            }
        }
        let inferred_keep = inferred_links;
        if list {
            let mut all: Vec<String> = src
                .iter()
                .filter(|((a, b), _)| !matches!(a, TailNode::Col(..)) || !matches!(b, TailNode::Col(..)))
                .map(|((a, b), s)| format!("  ORIENTED {} > {} ({s})", name(a), name(b)))
                .collect();
            all.sort();
            out.extend(all);
        }
        // (G1's note (b) on fa1267b75: a lift leaves onto every level neighbour holding a rail, and
        // each such link is oriented away from it: each column's entry and exit rails printed, to
        // check against the runs)
        for k in 0..self.columns.len() {
            let c = self.columns[k];
            let first = self.col_cell(k, 0);
            let entry_rails: Vec<BlockPos> = self.rails.keys().filter(|r| self.entry_cells(**r).contains(&first)).copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            let entries: Vec<String> = entry_rails.iter().map(|r| format!("{},{},{}", r.x, r.y, r.z)).collect();
            let exit_cells = self.exit_rails(k);
            let exits: Vec<String> = exit_cells.iter().map(|r| format!("{},{},{}", r.x, r.y, r.z)).collect();
            // (G1 on 7659ee398: the landing's slope exception holds while a fall is entered with no
            // speed up its landing's slope: a fall entered from a rail moving toward the landing
            // slope's high side is flagged)
            let uphill: Vec<(i32, i32)> = if c[3] < c[1] {
                exit_cells.iter().filter_map(|l| rail_exits(self.rails.get(l).map(|s| s.as_str()).unwrap_or("")).into_iter().find(|e| e.1 == 1).map(|e| (e.0, e.2))).collect()
            } else {
                Vec::new()
            };
            let against: Vec<String> = entry_rails
                .iter()
                .filter(|r| (first.x - r.x, first.z - r.z) != (0, 0) && uphill.iter().any(|(ux, uz)| (first.x - r.x) * ux + (first.z - r.z) * uz > 0))
                .map(|r| format!("{},{},{}", r.x, r.y, r.z))
                .collect();
            out.push(format!(
                "  column {} {:.1},{:.1}..{:.1},{:.1}: entered from [{}], left onto [{}]{}{}",
                if c[3] < c[1] { "(a fall)" } else { "(a lift)" },
                c[0],
                c[1],
                c[3],
                c[2],
                entries.join(" "),
                exits.join(" "),
                if c[3] >= c[1] && exits.len() > 1 { " (a lift with more than one exit rail: each oriented away from it; check them against the runs)" } else { "" },
                if against.is_empty() { String::new() } else { format!(" (FLAG: entered from {} moving toward its landing slope's high side: the slope exception for its landing does not hold)", against.join(" ")) }
            ));
        }
        self.allowed = Some(allowed);
        self.declared_conflicts = decl_conf_keep;
        self.inferred_links = inferred_keep;
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

    /// (G1 08:22: the late pairs' routes) The walk's route from `from` to `to` within `max`, each
    /// node with its arrival time, along the same edges and rests as [`Track::walk`].
    fn walk_path(&self, from: TailNode, to: TailNode, max: f64) -> Option<Vec<(TailNode, f64)>> {
        let mut best: HashMap<TailNode, f64> = HashMap::new();
        let mut parent: HashMap<TailNode, TailNode> = HashMap::new();
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, TailNode)>> = std::collections::BinaryHeap::new();
        best.insert(from, 0.0);
        heap.push(std::cmp::Reverse((0, from)));
        while let Some(std::cmp::Reverse((_, n))) = heap.pop() {
            if n == to {
                break;
            }
            let d = best[&n];
            let next = match n {
                TailNode::Rail(x, y, z) if self.rests.contains(&BlockPos::new(x, y, z)) && d > 0.0 => Vec::new(),
                _ => self.edges(n),
            };
            for (m, w) in next {
                let nd = d + w;
                if nd <= max && best.get(&m).is_none_or(|b| nd < *b) {
                    best.insert(m, nd);
                    parent.insert(m, n);
                    heap.push(std::cmp::Reverse(((nd * 1000.0) as u64, m)));
                }
            }
        }
        let t = *best.get(&to)?;
        let mut path = vec![(to, t)];
        let mut c = to;
        while c != from {
            let p = *parent.get(&c)?;
            path.push((p, best[&p]));
            c = p;
        }
        path.reverse();
        Some(path)
    }

    /// A node as a route prints it: a rail cell with its shape, or a column position.
    fn node_label(&self, n: TailNode) -> String {
        match n {
            TailNode::Rail(x, y, z) => {
                let s = self.rails.get(&BlockPos::new(x, y, z)).map(|s| s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("?").to_string()).unwrap_or_else(|| "?".into());
                format!("{x},{y},{z} {s}")
            }
            TailNode::Col(k, i) => {
                let p = self.col_pos(k, i);
                format!("column {:.2},{:.2},{:.2}", p.x, p.y, p.z)
            }
        }
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
                    if self.rails.contains_key(&m) || self.rails.contains_key(&m.below()) || into_column || self.blocks_cart(c, (*dx, *dy, *dz), m) {
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
pub fn clear_all(spec: &FarmSpec, home: Option<BlockPos>, panic_boxes: &[([i32; 3], [i32; 3])], claims: &[BlockPos], pills: &[Vec3], columns: &[[f64; 4]], tail_gt: f64, rests: &[BlockPos], list: bool, idle: bool, watch: &[([i32; 3], [i32; 3])], approach: &[(BlockPos, f64)], late_gt: f64, ends_declared: &[(BlockPos, String)], travel: &[(Vec<BlockPos>, String)], reversal: &[(BlockPos, String)], two_way: bool, orient_list: bool, place_classes: &PlaceClasses) -> Result<(Vec<String>, ClearVerdict), String> {
    let mut pos = rider_positions(spec);
    let mut col_of: Vec<Option<(usize, usize)>> = vec![None; pos.len()];
    let mut track = Track::new(spec, columns, rests);
    // (G1 Nov 13 09:38 (a): the record's declared switched Ts are the switched set; the inferred
    // ones (proximity to a signal source, joinable arms) not declared are printed)
    let mut switch_lines: Vec<String> = Vec::new();
    if !place_classes.switches.is_empty() {
        let declared: std::collections::HashSet<BlockPos> = place_classes.switches.iter().map(|(p, _)| *p).collect();
        let mut undeclared: Vec<String> = track.switched_ts.iter().filter(|c| !declared.contains(c)).map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect();
        undeclared.sort();
        switch_lines.push(format!(
            "switched Ts: {} declared (`switches:`), the walk's set; {} inferred and not declared{}",
            declared.len(),
            undeclared.len(),
            if undeclared.is_empty() { String::new() } else { format!(" (printed, G1 09:38 (a)): {}", undeclared.join("; ")) }
        ));
        track.switched_ts = declared;
    }
    // (ironspec Nov 12 16:02: the walks follow the way carts travel each link, by default)
    let travel_lines = if two_way { vec!["travel: --two-way: every rail link walked either way (the old walk)".to_string()] } else { track.orient(travel, reversal, orient_list) };
    let travel_lines: Vec<String> = switch_lines.into_iter().chain(travel_lines).collect();
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
    // (ironspec Nov 13 06:00 real: the track-end bar's samples, the walks run; none NOT JUDGED)
    let mut track_walks = usize::from(!seeds.is_empty());
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
    let mut stale_waker: Vec<String> = Vec::new();
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
    let mut tails = 0usize;
    let mut riderless_tails = 0usize;
    // (G1 Nov 13 05:20, hole 4: the gate's samples are the PANIC candidates judged at panicking
    // positions, not the positions; a position with none is counted apart)
    let mut gate_candidates = 0usize;
    let mut gate_zero = 0usize;
    // (ironspec Nov 13 06:00 real, POPULATIONS: positions and places zipped only at equal lengths)
    if pos.len() != ps.len() {
        return Err(format!("clear-all: {} positions but {} places judged (a failed spawn would shift every later result)", pos.len(), ps.len()));
    }
    for (idx, ((c, t, p), pl)) in pos.iter().zip(ps.iter()).enumerate() {
        let tail = pl.seen == 0 && best.contains_key(&node_of(idx));
        if tail {
            // (G1 Nov 13, N14: a `riderless:` cell's tail position holds no rider: counted apart,
            // out of the staleness bar's samples, and printed)
            if place_classes.riderless(c) {
                riderless_tails += 1;
            } else {
                tails += 1;
            }
        }
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
        // failure: the remembered hostile within the flee bar after it left sight; ironspec Nov 12
        // 21:49 (4): a `riderless:` cell holds no rider, a `waker-places:` cell is the declared
        // exception, listed apart; G1 Nov 13, N22: a pillager within the flee bar too)
        let near_any = [pl.near.map(|(lo, _, _)| lo), pl.near_pill].into_iter().flatten().min_by(f64::total_cmp);
        if tail && near_any.is_some_and(|lo| lo < FLEE_BAR) && !place_classes.riderless(c) {
            let row = format!(
                "{},{},{} {} ({:.1} gt from a seen position, a hostile at {:.2})",
                c.x,
                c.y,
                c.z,
                if *t < 0.0 { "column".to_string() } else { format!("t {t:.2}") },
                best[&node_of(idx)],
                near_any.unwrap_or(0.0)
            );
            match place_classes.waker(c) {
                Some(w) => stale_waker.push(format!("{row}: declared waker place: {w}")),
                None => stale.push(row),
            }
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
        if panics {
            let n: usize = srcs.iter().filter(|(k, _)| *k < 2).map(|(_, s)| s.reached.len() + s.unreached.len()).sum();
            gate_candidates += n;
            if n == 0 {
                gate_zero += 1;
            }
        }
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
    // `late_gt` of travel after it (along the way carts travel each link, `orient`; a column only
    // onward) from that node's rider positions (a rail cell's three, a column's position): a late
    // FAIL where any of them leaves it unreached (inside a water column the path is the water's
    // surface, one node: redsim's passenger start rule, the official row's shape))
    let mut late: std::collections::BTreeMap<(i32, i32, i32), Vec<(String, String, f64, TailNode, TailNode)>> = std::collections::BTreeMap::new();
    let mut late_exempt_hits: std::collections::BTreeMap<(i32, i32, i32), (String, usize, f64)> = std::collections::BTreeMap::new();
    let mut late_positions = 0usize;
    // (G1 Nov 13 05:20, hole 3: the late paths' samples are the paths computed inside the window:
    // each (draw node, candidate, later node within --late-gt) check; none (no node a panicking
    // node's ride reaches inside the window) is NOT JUDGED)
    let mut late_checks = 0usize;
    if late_gt > 0.0 {
        // (ironspec Nov 13 06:00 real, P5: each later node's rider positions, a rail cell's three
        // (t 0.1, 0.5, 0.9 along the cart's path), a column's own; the draw node itself included)
        let node_pos = |n: &TailNode| -> Vec<Vec3> {
            match n {
                TailNode::Rail(x, y, z) => {
                    let c = BlockPos::new(*x, *y, *z);
                    track.rails.get(&c).map(|s| [0.1, 0.5, 0.9].iter().map(|t| along_rail(c, s, *t)).collect()).unwrap_or_default()
                }
                TailNode::Col(k, i) => vec![track.col_pos(*k, *i)],
            }
        };
        // (G1 Nov 13, N18: the late window follows the way carts TRAVEL each link (`orient`), the
        // physical walk: a cart never rolls back up a link it came down (a two-way walk invented
        // backward rides from the siding to the lift top); an INFERRED link is flagged by `orient`
        // (N32) and declared or fixed there; the comment above said "either way" and is corrected)
        let track_two = &track;
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
        let mut pair_ix: HashMap<(TailNode, usize, (i32, i32, i32)), usize> = HashMap::new();
        let mut checks: Vec<(TailNode, (i32, i32, i32), TailNode, usize, f64)> = Vec::new();
        for (nd, xs) in &by_node {
            if xs.is_empty() {
                continue;
            }
            let after_o = track.walk(&[*nd], late_gt);
            record_ends(&track, &after_o, late_gt, 1, &mut ends);
            track_walks += 1;
            let after = track_two.walk(&[*nd], late_gt);
            for (q, dt) in &after {
                for (k, qp) in node_pos(q).into_iter().enumerate() {
                    for x in xs {
                        let key = (*q, k, *x);
                        if !pair_ix.contains_key(&key) {
                            pair_ix.insert(key, pairs.len());
                            pairs.push((qp, BlockPos::new(x.0, x.1, x.2)));
                        }
                        checks.push((*nd, *x, *q, k, *dt));
                    }
                }
            }
        }
        let ok = reach_pairs(spec, home, claims, &pairs)?;
        late_checks = checks.len();
        let mut failed_nodes: std::collections::BTreeSet<TailNode> = std::collections::BTreeSet::new();
        for (nd, x, q, k, dt) in checks {
            if ok[pair_ix[&(q, k, x)]] {
                continue;
            }
            // (ironspec 09:01 (c): a declared `late-exempt:` draw cell's pairs are information,
            // printed with their routes and the exemption's conditions; never a failing node)
            if let TailNode::Rail(a, b2, c2) = nd
                && let Some(why) = place_classes.late_exempt(&BlockPos::new(a, b2, c2))
            {
                late_exempt_hits.entry((a, b2, c2)).or_insert_with(|| (why.to_string(), 0, f64::INFINITY)).1 += 1;
                let e = late_exempt_hits.get_mut(&(a, b2, c2)).expect("just inserted");
                e.2 = e.2.min(dt);
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
            late.entry(x).or_default().push((ns, qs, dt, nd, q));
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
        if late_gt <= 0.0 {
            "not checked".to_string()
        } else if late.is_empty() && !late_exempt_hits.is_empty() {
            // (G1 Nov 13 10:38 (a): a late PASS leaning on an exemption is PENDING until the runs
            // that judge its conditions pass; ironspec 13:50 (c): a cited run's exemption cites the
            // composed rows showing its three conditions, a `rows:` part of its reason)
            let uncited = late_exempt_hits.values().filter(|(why, _, _)| row_citation(why).is_none()).count();
            format!(
                "0 failing, PENDING: it leans on {} declared late-exempt draw cell(s) whose conditions are NOT JUDGED here{}",
                late_exempt_hits.len(),
                if uncited == 0 { String::new() } else { format!("; {uncited} of them cite no composed rows (`rows:` in the reason): NOT CITABLE (ironspec 13:50 (c))") }
            )
        } else if late.is_empty() {
            "0".to_string()
        } else {
            format!("FAIL at {late_positions} panicking nodes, {} candidate cells", late.len())
        }
    ));
    for ((x, y, z), (why, n, dt)) in &late_exempt_hits {
        lines.push(format!(
            "  EXEMPT (declared late-exempt: {why}) drawn at {x},{y},{z}: {n} (draw, later node) pairs, the first {dt:.1} gt later; information, not a failing node; its conditions are NOT JUDGED here (G1 Nov 13 10:38 (a): the composed runs judge them, and the late result leaning on this exemption stays PENDING until they pass): a napper-only position (every entity seated there printed), the stamp erased at the napper's lie-down (SleepInBed.start: before / after printed), arming -> lie-down under 1,000 gt with no SetWalkTargetFromBlockMemory run at >= 1,000 between{}",
            if row_citation(why).is_some() { "; the composed rows it cites are in its reason (ironspec 13:50 (c))".to_string() } else { "; it cites NO composed rows (ironspec 13:50 (c): add `rows: FILE t A..B cart N` showing the three conditions; until then a run leaning on it is not citable)".to_string() }
        ));
    }
    let mut lt: Vec<_> = late.into_iter().collect();
    lt.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for ((x, y, z), v) in lt {
        // (by draw node: its pairs and the earliest later node that leaves the candidate unreached)
        let mut by_draw: std::collections::BTreeMap<String, (usize, f64, String, Option<(TailNode, TailNode)>)> = std::collections::BTreeMap::new();
        for (ns, qs, dt, nd, q) in &v {
            let e = by_draw.entry(ns.clone()).or_insert((0, f64::INFINITY, String::new(), None));
            e.0 += 1;
            if *dt < e.1 {
                e.1 = *dt;
                e.2 = qs.clone();
                e.3 = Some((*nd, *q));
            }
        }
        lines.push(format!(
            "  LATE {x},{y},{z}: {} (draw, later node) pairs unreached; by draw node: {}",
            v.len(),
            by_draw.iter().map(|(ns, (n, dt, qs, _))| format!("drawn at {ns}: {n} pairs, the first pathed {dt:.1} gt later from {qs}")).collect::<Vec<_>>().join("; ")
        ));
        // (G1 08:22: each draw node's first pair with orient's route link by link, the rail shapes
        // it reads, so a pair no cart rides shows the link that makes it)
        for (ns, (_, _, _, nq)) in &by_draw {
            if let Some((nd, q)) = nq {
                let route = track.walk_path(*nd, *q, late_gt).map(|p| p.iter().map(|(n, t2)| format!("{} @{t2:.1}", track.node_label(*n))).collect::<Vec<_>>().join(" > "));
                lines.push(format!("    ROUTE drawn at {ns}: {}", route.unwrap_or_else(|| "none found".into())));
            }
        }
    }
    lines.push(format!(
        "staleness (an inherited policy, G1's CHANGES 2: a tail position within {FLEE_BAR} of a hostile (a zombie, or a pillager, G1 N22), NEAREST_HOSTILE remembered within the flee bar; {tails} tail positions judged, {riderless_tails} at `riderless:` cells apart): {}",
        if stale.is_empty() { "0".to_string() } else { format!("FAIL at {}: {}", stale.len(), stale.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }
    ));
    if !stale_waker.is_empty() {
        lines.push(format!("  staleness at declared waker places (apart, not failing): {}: {}", stale_waker.len(), stale_waker.iter().take(8).cloned().collect::<Vec<_>>().join("; ")));
    }
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
        settle_full(&mut run)?;
        let w = &mut run.world;
        // (G1 N6 / N19: the gate's hostile positions, from the world)
        let zs = world_zombies(w, spec)?;
        let w = &run.world;
        for (cell, gt) in approach {
            // (the positions BEFORE the entrance: the walk over the track's moves reversed)
            let reach = track.walk_rev(&[TailNode::Rail(cell.x, cell.y, cell.z)], *gt);
            record_ends(&track, &reach, *gt, 2, &mut ends);
            track_walks += 1;
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
                let eye = Vec3::new(p.x, p.y + veye(), p.z);
                let mut by: Vec<String> = Vec::new();
                for z in &zs {
                    let f = *z;
                    let d = ((f.x - p.x).powi(2) + (f.y - p.y).powi(2) + (f.z - p.z).powi(2)).sqrt();
                    if d <= 8.0 && redsim::entity::mob::los::has_line_of_sight(w, eye, Vec3::new(f.x, f.y + zeye(), f.z)) {
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
        "track ends the walks reach with time left (a rail exit with no rail or column beyond it, or a column's end with no rail: the walk cannot go on there; 0 undeclared, an inherited policy, ironspec Nov 12 15:03; {track_walks} walks run, none NOT JUDGED): {}",
        if track_walks == 0 {
            "NOT JUDGED (0 walks)".to_string()
        } else if ends.is_empty() {
            "0".to_string()
        } else if undeclared_ends == 0 {
            format!("{}, every one declared", ends.len())
        } else {
            format!("FAIL: {undeclared_ends} undeclared of {} (each truncates a walk with time left: declare it with --track-end x,y,z=REASON or a --tail-rest, pass the fall or column it leads into, or lay the missing track)", ends.len())
        }
    ));
    lines.extend(end_lines);
    // (G1 Nov 13, N14 / ironspec 04:07: a declaration that changes nothing FAILS: a --track-end
    // naming no end a walk reached, a --tail-rest on no rail of the track)
    let mut unused_decls = 0usize;
    for (p, r) in ends_declared {
        if !ends.values().any(|(cells, _)| cells.contains(p)) {
            unused_decls += 1;
            lines.push(format!("  UNUSED --track-end {},{},{}={r}: no end a walk reaches holds it (FAILS: a declaration that changes nothing)", p.x, p.y, p.z));
        }
    }
    for p in rests {
        if !track.rails.contains_key(p) {
            unused_decls += 1;
            lines.push(format!("  UNUSED --tail-rest {},{},{}: no rail of the track there (FAILS: a declaration that changes nothing)", p.x, p.y, p.z));
        }
    }
    lines.extend(travel_lines);
    lines.extend(listed);
    Ok((lines, ClearVerdict { undeclared_ends, gate_fails: fails, late_fails: late_positions, stale: stale.len(), hazards: n_haz, gate_samples: gate_candidates, late_samples: if late_gt > 0.0 { late_checks } else { 0 }, stale_samples: tails, gate_positions: panicking, gate_zero, late_gt, track_walks, unused_decls, declared_conflicts: track.declared_conflicts.len(), inferred_links: track.inferred_links.len() }))
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
    /// (ironspec Nov 12 23:56: each bar's samples; G1 Nov 13 05:20, holes 3 / 4) the gate's PANIC
    /// candidates judged at panicking positions, the late paths' checks computed inside the window
    /// (0 when not checked), the staleness bar's tail positions.
    pub gate_samples: usize,
    pub late_samples: usize,
    pub stale_samples: usize,
    /// The panicking positions (the gate's places) and those of them with no PANIC candidate.
    pub gate_positions: usize,
    pub gate_zero: usize,
    /// The late window, in gt, as run.
    pub late_gt: f64,
    /// The walks the track-end bar judged (the tail walk, each late-path walk, each approach walk).
    pub track_walks: usize,
    /// The declarations that change nothing (a --track-end no end holds, a --tail-rest off the
    /// track): each FAILS (ironspec Nov 13 04:07, G1 N14).
    pub unused_decls: usize,
    /// (G1 Nov 13 09:42) The declared travel links that conflict with the shape-read direction:
    /// each FAILS (the geometry or the declaration is wrong; the owners settle it).
    pub declared_conflicts: usize,
    /// (ironspec Nov 13 13:50 (b)) The INFERRED links no declaration confirms: while any stands, a
    /// cited run's late verdict is NOT JUDGED.
    pub inferred_links: usize,
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

    /// (ironspec Nov 13 06:00 real) A curve's cart positions run along the chord between its exits
    /// (moveAlongTrack's snap), not its centre for every t: north_east from its north exit
    /// (x + 0.5, z) to its east exit (x + 1, z + 0.5); south_west from (x + 0.5, z + 1) to (x, z + 0.5).
    #[test]
    fn along_rail_curves_follow_the_chord() {
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let ne = "minecraft:rail[shape=north_east,waterlogged=false]";
        let (a, m, b) = (along_rail(BlockPos::new(2, 5, 7), ne, 0.0), along_rail(BlockPos::new(2, 5, 7), ne, 0.5), along_rail(BlockPos::new(2, 5, 7), ne, 1.0));
        assert!(close(a.x, 2.5) && close(a.z, 7.0) && close(b.x, 3.0) && close(b.z, 7.5) && close(m.x, 2.75) && close(m.z, 7.25) && close(m.y, 5.0625), "{a:?} {m:?} {b:?}");
        // (G1: points on the EXIT HALVES, t 0.1 and 0.9, near each exit, off the centre)
        let (e1, e2) = (along_rail(BlockPos::new(2, 5, 7), ne, 0.1), along_rail(BlockPos::new(2, 5, 7), ne, 0.9));
        assert!(close(e1.x, 2.55) && close(e1.z, 7.05) && close(e2.x, 2.95) && close(e2.z, 7.45), "{e1:?} {e2:?}");
        // (one model: rail_segment's chord is along_rail's ends)
        let (s0, s1) = super::rail_segment(BlockPos::new(2, 5, 7), ne).expect("a curve has a segment");
        assert!(close(s0.x, a.x) && close(s0.z, a.z) && close(s1.x, b.x) && close(s1.z, b.z), "{s0:?} {s1:?}");
        let sw = along_rail(BlockPos::new(0, 0, 0), "minecraft:rail[shape=south_west,waterlogged=false]", 1.0);
        assert!(close(sw.x, 0.0) && close(sw.z, 0.5), "{sw:?}");
        let positions: Vec<_> = (0..=10).map(|k| along_rail(BlockPos::new(0, 0, 0), ne, k as f64 * 0.1)).collect();
        assert!(positions.windows(2).all(|p| (p[0].x - p[1].x).abs() > 1e-9), "every t a different point on a curve");
    }
}

#[cfg(test)]
mod orient_tests {
    use super::{PlaceClasses, TailNode, Track};
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

    /// (G1 Nov 13 09:02: a fixed S-bend orients one way) A siding's S-bend (R a slope, two curves,
    /// C a curve into a fall) with a detector rail within Manhattan 2 of its middle: the middle has
    /// rails on 3 sides, but its third neighbour R is full (both its ends taken by rails pointing
    /// back), so RailState.place can't re-shape it: NOT a switched T, and with the fall oriented the
    /// walk from C reaches no cell of the bend; a T whose third neighbour points at it IS switched.
    #[test]
    fn s_bend_is_no_switched_t() {
        let r = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        let mut m = HashMap::new();
        for (x, y, z, s) in [(0, 0, 1, "ascending_south"), (0, 1, 2, "north_south"), (0, 0, 0, "south_east"), (1, 0, 0, "south_west"), (1, 0, 1, "north_east"), (2, 0, 1, "north_west")] {
            m.insert(BlockPos::new(x, y, z), r(s));
        }
        m.insert(BlockPos::new(2, 0, 2), "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]".to_string());
        let cols = [[2.5, 0.0, 0.5, -3.0]];
        let rests: [BlockPos; 0] = [];
        // (`sources`: the cells inside an OPERATING source's update footprint (Track::new derives
        // them); the bend's middle is inside one here, so only the joinable-arm rule decides)
        let sources = vec![BlockPos::new(1, 0, 1)];
        let mut t = Track::from_parts(m.clone(), &cols, &rests, sources.clone(), Default::default());
        assert!(!t.switched_ts.contains(&BlockPos::new(1, 0, 1)), "the bend's middle is no switched T: {:?}", t.switched_ts);
        let _ = t.orient(&[], &[], false);
        let after = t.walk(&[n(2, 0, 1)], 40.0);
        assert!(!after.contains_key(&n(1, 0, 1)) && !after.contains_key(&n(0, 0, 1)), "no backward ride from C into the bend: {after:?}");
        // (a T whose third neighbour points at it: switched)
        let mut m2 = m.clone();
        m2.insert(BlockPos::new(0, 0, 1), r("east_west"));
        m2.remove(&BlockPos::new(0, 1, 2));
        let t2 = Track::from_parts(m2, &cols, &rests, sources, Default::default());
        assert!(t2.switched_ts.contains(&BlockPos::new(1, 0, 1)), "a joinable third arm: {:?}", t2.switched_ts);
    }

    /// (G1 on f4e5482ee, N18 / N39's late-path control) The late window's walk on a ONE-WAY line: a
    /// travel declared south along x 0 (z 0 -> 4); from a draw at (0,0,2) the walk within 6 gt reaches
    /// the FORWARD node (0,0,3) and never the BACKWARD (0,0,1): the late check walks exactly this
    /// oriented track (a two-way walk invented backward pairs).
    #[test]
    fn late_window_walks_forward_only() {
        let r = |s: &str| format!("minecraft:rail[shape={s},waterlogged=false]");
        let mut m = HashMap::new();
        for z in 0..=4 {
            m.insert(BlockPos::new(0, 0, z), r("north_south"));
        }
        let cols: [[f64; 4]; 0] = [];
        let rests: [BlockPos; 0] = [];
        let mut t = Track::from_parts(m, &cols, &rests, Vec::new(), Default::default());
        let line: Vec<BlockPos> = (0..=4).map(|z| BlockPos::new(0, 0, z)).collect();
        let _ = t.orient(&[(line, "the declared run south".into())], &[], false);
        let after = t.walk(&[n(0, 0, 2)], 6.0);
        assert!(after.contains_key(&n(0, 0, 3)), "the forward node inside the window: {after:?}");
        assert!(!after.contains_key(&n(0, 0, 1)), "no backward node: {after:?}");
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
        // (ironspec Nov 13 13:50 (b): the inferred link is listed, unconfirmed, and counted; a
        // `travel` declaration of the same move confirms it: no inferred link stands)
        assert!(t.inferred_links.iter().any(|l| l.starts_with("0,0,2 > 1,0,2")) && lines.iter().any(|l| l.starts_with("  INFERRED LINK 0,0,2 > 1,0,2") && l.contains("unconfirmed")), "{lines:?}");
        let mut tc = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lc = tc.orient(&[(vec![BlockPos::new(0, 0, 2), BlockPos::new(1, 0, 2)], "the composed run shows it; row: designs/farms/iron/carousel-assembly/carousel-v0a.txt t 120..180 cart 3".into())], &[], false);
        assert!(!tc.inferred_links.iter().any(|l| l.starts_with("0,0,2 > 1,0,2")), "a declared move citing its row is confirmed: {lc:?}");
        // (G1 13:56: a `travel` line citing no composed row orients the walk but confirms nothing)
        let mut tu = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lu = tu.orient(&[(vec![BlockPos::new(0, 0, 2), BlockPos::new(1, 0, 2)], "a run shows it".into())], &[], false);
        assert!(tu.inferred_links.iter().any(|l| l.starts_with("0,0,2 > 1,0,2 (declared (UNCITED))")) && lu.iter().any(|l| l.starts_with("  UNCONFIRMED LINK 0,0,2 > 1,0,2")), "{lu:?}");
        // (G1 14:36 (ii), ironspec 14:37 (1): the citation is parsed, `row:` / `rows:` as their own
        // token with a file, a tick range and a cart; a mere mention, an "arrow:", or a part missing
        // one of the three cites nothing)
        // (G1 16:36, CHANGES 1: a relative path cites only when the file is in the repo)
        assert!(super::row_citation("row: designs/farms/iron/carousel-assembly/carousel-v0a.txt t 120..180 cart 3").is_some(), "a committed design file");
        assert!(super::row_citation_why("row: record7f13t.txt t 120..180 cart 3").unwrap_err().contains("(not in the repo"), "a file not in the repo");
        assert!(super::row_citation_why("row: onechunk/t14-n2.txt t 1..2 cart 3").unwrap_err().contains("(not in the repo"), "G1's control");
        // (G1 18:37, B10–B12: committed means in HEAD's tree, checked on a TEMPORARY repository: a
        // committed file cites; a staged-but-uncommitted file, an untracked one and a glob that only
        // matches a committed name don't; a directory that is no repository is named "unchecked: git
        // could not check")
        {
            let root = std::env::temp_dir().join(format!("mobfarm-citation-repo-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&root).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).output().unwrap();
            git(&["init", "-q"]);
            std::fs::write(root.join("a1.txt"), "a").unwrap();
            git(&["add", "a1.txt"]);
            git(&["commit", "-q", "-m", "a"]);
            std::fs::write(root.join("staged.txt"), "s").unwrap();
            git(&["add", "staged.txt"]);
            std::fs::write(root.join("loose.txt"), "l").unwrap();
            let at = |f: &str| super::row_citation_why_in(&format!("row: {f} t 1..2 cart 3"), &root);
            assert!(at("a1.txt").is_ok(), "committed: {:?}", at("a1.txt"));
            for f in ["staged.txt", "loose.txt", "a[1].txt"] {
                assert!(at(f).as_ref().is_err_and(|e| e.contains("(not in the repo: not committed at HEAD")), "{f}: {:?}", at(f));
            }
            let plain = std::env::temp_dir().join(format!("mobfarm-citation-plain-{}", std::process::id()));
            std::fs::create_dir_all(&plain).unwrap();
            let r = super::row_citation_why_in("row: a1.txt t 1..2 cart 3", &plain);
            assert!(r.as_ref().is_err_and(|e| e.contains("(unchecked: git could not check")), "no repository: {r:?}");
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::remove_dir_all(&plain);
        }
        // (7b90c3f1's emitter form, 14:41: a /tmp path alone is NOT DURABLE (G1 15:51), the md5 in the
        // reason's first part not beside it; with its md5 or an artifact id in the row part, cited)
        let tmp = "a batch cart on the split row enters J(-1) from the west (onechunk's composed t14 night, 11b888a04256); row: /tmp/onechunk/t14/n-p0_0_0-d0-s1.txt t 2854..2855 cart 46";
        assert!(super::row_citation(tmp).is_none() && super::row_citation_why(tmp).unwrap_err().starts_with("not durable: /tmp/onechunk"), "{:?}", super::row_citation_why(tmp));
        assert!(super::row_citation("a batch cart; row: /tmp/onechunk/t14/n-p0_0_0-d0-s1.txt md5 9af7084ecb19 t 2854..2855 cart 46").is_some());
        assert!(super::row_citation("a batch cart; row: night n-p0 (a/2344002509732418) t 2854..2855 cart 46").is_some(), "an artifact id names the file durably");
        assert!(super::row_citation("a batch cart; row: n-p0_0_0-d0-s1.txt https://artifacts.internalmeta.com/a/2344002509732418 t 2854..2855 cart 46").is_some());
        assert!(super::row_citation("the composed night; rows: onechunk/t14-n2.txt ticks 4400..4480, cart 7").is_none(), "a box-relative file, not in the repo");
        // (G1 16:36 notes: an md5 named or exactly 12 / 32 hex; an artifact id anchored, not inside a word)
        assert!(super::row_citation("row: n2.txt md5 9af7084ecb19aa t 1..2 cart 3").is_some(), "a named md5");
        assert!(super::row_citation("row: n2.txt 123456789012345 t 1..2 cart 3").is_none(), "a long number is no md5");
        assert!(super::row_citation("row: data/123 t 1..2 cart 3").is_none(), "\"data/123\" is no artifact id");
        assert!(super::row_citation("seen in row: record7f13t.txt t120..180 cart 3").is_none(), "not at a part's start");
        assert!(super::row_citation("the arrow: record7f13t.txt t 120..180 cart 3").is_none());
        assert!(super::row_citation("row: record7f13t.txt cart 3").is_none(), "no tick range");
        assert!(super::row_citation("row: t 120..180 cart 3").is_none(), "no file");
        assert!(super::row_citation("row: record7f13t.txt t 120..180").is_none(), "no cart");
        let mut tm = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lm = tm.orient(&[(vec![BlockPos::new(0, 0, 2), BlockPos::new(1, 0, 2)], "a run; row: somewhere".into())], &[], false);
        assert!(tm.inferred_links.iter().any(|l| l.contains("(UNCITED)")), "a half citation confirms nothing: {lm:?}");
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lines = t.orient(&[(vec![BlockPos::new(0, 0, 2), BlockPos::new(1, 0, 2)], "a run".into()), (vec![BlockPos::new(1, 0, 2), BlockPos::new(0, 0, 2)], "another".into())], &[], false);
        let a = t.allowed.clone().expect("oriented");
        assert!(a.contains(&(n(0, 0, 2), n(1, 0, 2))) && a.contains(&(n(1, 0, 2), n(0, 0, 2))), "a conflict stays both ways: {lines:?}");
        // (G1 Nov 13 09:42: a declared link against the other direction is a FAILING conflict)
        assert!(!t.declared_conflicts.is_empty(), "the declared conflict is counted: {lines:?}");
        assert!(lines.iter().any(|l| l.contains("DECLARED AGAINST THE SHAPES")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("oriented both ways (a conflict") && l.contains("0,0,2-1,0,2")), "{lines:?}");
        let mut t = Track::from_parts(rails(), &cols, &rests, Vec::new(), Default::default());
        let lines = t.orient(&[], &[(BlockPos::new(0, 0, 1), "the run shows it".into())], false);
        let a = t.allowed.clone().expect("oriented");
        assert!(a.contains(&(n(0, 0, 1), n(0, 0, 0))) && a.contains(&(n(0, 0, 2), n(0, 0, 1))), "a reversal's links both ways: {lines:?}");
        assert!(!lines.iter().any(|l| l.starts_with("  oriented both ways")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("declared --reversal 0,0,1: the run shows it")), "{lines:?}");
    }

    /// (G1's note 4 on a4cd9b8b0) A T inside an operating source's update footprint moves onto
    /// every arm (the union of its shapes); outside every one, its stored shape's two only.
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
        // (the T (0,0,2) inside an operating source's footprint: a lever's at (0,1,2))
        let lever = Track::from_parts(rails(), &cols, &rests, vec![BlockPos::new(0, 0, 2)], Default::default());
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
        let mut wall = Track::from_parts(rails(), &cols, &rests, Vec::new(), [(BlockPos::new(3, 0, 2), vec![[0, 16, 0, 16, 0, 16]])].into_iter().collect());
        wall.orient(&[], &[], false);
        assert!(!east_end(&wall), "a wall beyond the rail end stops the cart: no track end");
        // (G1's note (a): a carpet's 1/16 top misses the cart's box, 0.0625 over the rail: an end)
        let mut carpet = Track::from_parts(rails(), &cols, &rests, Vec::new(), [(BlockPos::new(3, 0, 2), vec![[0, 16, 0, 1, 0, 16]])].into_iter().collect());
        carpet.orient(&[], &[], false);
        assert!(east_end(&carpet), "a carpet beyond the rail end lets the cart roll off: a track end");
    }

    /// (ironspec Nov 12 21:49 (4)) The place classes from the design's meta lines: `riderless:` cells
    /// and `waker-places:` entries, each cell with its reason; a missing line is no class.
    #[test]
    fn place_classes_from_meta() {
        let mut meta = std::collections::BTreeMap::new();
        meta.insert("riderless".to_string(), "1,-26,-5+0,-27,-5=the stop's empty cart".to_string());
        meta.insert("waker-places".to_string(), "-3,-27,3=asleep in the bed (2.04); -2,-27,4+0,-27,4=riding out past the hatch (A6 b36302af8)".to_string());
        let c = PlaceClasses::from_meta(&meta).unwrap();
        assert!(c.riderless(&BlockPos::new(0, -27, -5)) && !c.riderless(&BlockPos::new(-3, -27, 3)));
        assert_eq!(c.waker(&BlockPos::new(0, -27, 4)), Some("riding out past the hatch (A6 b36302af8)"));
        assert_eq!(c.waker(&BlockPos::new(-3, -27, 3)), Some("asleep in the bed (2.04)"));
        assert!(PlaceClasses::from_meta(&std::collections::BTreeMap::new()).unwrap().waker.is_empty());
        // (7b90c3f1 10:37: `# switches:` / `# update-only:` header lines join the meta lines)
        let text = "# switches: -1,25,9=J(-1): the lever (-1,26,9), on north_west, off north_east\n# update-only: -3,23,9=lane -1's feed dust: re-settles K(-1)\nname: x\n";
        let c3 = PlaceClasses::from_design(&std::collections::BTreeMap::new(), text).unwrap();
        assert_eq!(c3.switches, vec![(BlockPos::new(-1, 25, 9), "J(-1): the lever (-1,26,9), on north_west, off north_east".to_string())]);
        assert_eq!(c3.update_only.len(), 1);
        // (ironspec Nov 13 09:01 (c): `late-exempt:` draw cells, each with its reason)
        meta.insert("late-exempt".to_string(), "9,-14,-4+9,-13,-5=the descent seat: napper-only, its stamp erased at (A)'s lie-down".to_string());
        let c2 = PlaceClasses::from_meta(&meta).unwrap();
        assert_eq!(c2.late_exempt(&BlockPos::new(9, -13, -5)), Some("the descent seat: napper-only, its stamp erased at (A)'s lie-down"));
        assert!(c2.late_exempt(&BlockPos::new(9, -15, -2)).is_none());
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

/// (G1 Nov 13, N39 / ironspec 06:00 real, CONTROLS: each bar's positive control through its own
/// path) A floor room with a three-cell rail and a standing zombie in sight of it.
#[cfg(test)]
mod gate_tests {
    use redsim::pos::BlockPos;

    fn b(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::new(x, y, z)
    }

    fn room(cage: bool) -> redsim::entity::mob::farm::FarmSpec {
        super::control_room(cage)
    }

    fn gate(cage: bool) -> super::ClearVerdict {
        let (_, v) = super::clear_all(&room(cage), None, &[], &[], &[], &[], 39.0, &[], false, true, &[], &[], 39.0, &[], &[], &[], false, false, &super::PlaceClasses::default()).unwrap();
        v
    }

    /// The gate's positive control: the caged pillar top is a PANIC candidate of the panicking rider
    /// places and no path reaches it, so the gate FAILS; samples are counted (candidates judged).
    #[test]
    fn gate_fails_on_a_caged_candidate() {
        let v = gate(true);
        assert!(v.gate_samples > 0 && v.gate_positions > 0, "{v:?}");
        assert!(v.gate_fails > 0, "the caged candidate must fail the gate: {v:?}");
    }

    /// The 6-8 gate and the zombie read from the world: the rider places see the standing zombie
    /// (3.5 - 0.5 = 3 away, inside the flee bar), so `report_classed` lists the 6-8 gate as failing.
    #[test]
    fn six_eight_gate_fails_with_a_zombie_in_sight() {
        let places = super::check(&room(false), None, &[], &[]).unwrap();
        assert!(!places.is_empty() && places.iter().all(|p| p.seen > 0), "every rider place sees the zombie");
        let (_, fails) = super::report_classed(&places, &[], 0, false, &super::PlaceClasses::default());
        assert!(fails.iter().any(|f| f.starts_with("the 6-8 gate")), "{fails:?}");
    }

    /// (tick-exact Nov 13 (1), G1 N39) An UNSEEN hostile inside 6 doesn't panic: a glass wall
    /// between the rail and the zombie (3 away): NEAREST_HOSTILE needs the eye-to-eye COLLIDER clip
    /// clear, so no place counts it for the flee or sees it.
    #[test]
    fn unseen_hostile_inside_six_does_not_panic() {
        let mut s = room(false);
        for z in -3..=3 {
            for y in 1..=4 {
                s.blocks.push((b(2, y, z), redsim::block::Block::parse("minecraft:glass").unwrap()));
            }
        }
        let places = super::check(&s, None, &[], &[]).unwrap();
        assert!(!places.is_empty() && places.iter().all(|p| p.seen == 0 && p.flee_from == 0), "{:?}", places.iter().map(|p| (p.cell, p.seen, p.flee_from)).collect::<Vec<_>>());
    }

    /// (ironspec Nov 13 07:11 / 07:16, G1) P's stroll static and its planted positive: on a glass
    /// floor (not solid-render: no stable destination) the rail's PANIC stroll has NO candidate;
    /// one stone floor cell planted 2 away gives it a candidate, and the bar fails.
    fn glass_room() -> redsim::entity::mob::farm::FarmSpec {
        let mut s = room(false);
        for (p, blk) in s.blocks.iter_mut() {
            if p.y == 0 {
                *blk = redsim::block::Block::parse("minecraft:glass").unwrap();
            }
        }
        // (the room's stone pillar (-2,1,0) is a stable floor itself: taken out)
        s.blocks.retain(|(p, _)| *p != b(-2, 1, 0));
        s
    }

    fn header(l: &[String], kind: &str) -> String {
        l.iter().find(|x| x.starts_with(kind)).cloned().unwrap_or_default()
    }

    /// (ironspec Nov 13 07:11 / 07:16 / 07:36, G1: P's stroll static and its positives) On a glass
    /// floor (not solid-render: no stable destination) the rail's PANIC stroll has NO candidate; one
    /// stone floor cell planted 2 away gives it a candidate (the DIRECT form); a stone planted UNDER
    /// a glass floor cell gives the open cell above the glass through the up-move (a draw INTO the
    /// glass is stable on the stone, then climbs out of the solid glass: the UP-MOVE form); both FAIL.
    #[test]
    fn stroll_none_and_the_planted_cell() {
        let s = glass_room();
        let bx = (s.lo, s.hi);
        let (l, n, _) = super::stroll_candidates(&s, &[], b(0, 1, 0), None, &[], &[], &[], bx, false).unwrap();
        assert!(n == 0 && header(&l, "the PANIC stroll").ends_with("(PASS)"), "{l:?}");
        let mut planted = s.clone();
        planted.blocks.retain(|(p, _)| *p != b(2, 0, 1));
        planted.blocks.push((b(2, 0, 1), redsim::block::Block::parse("minecraft:stone").unwrap()));
        let (l2, n2, _) = super::stroll_candidates(&planted, &[], b(0, 1, 0), None, &[], &[], &[], bx, false).unwrap();
        assert!(n2 > 0 && header(&l2, "the PANIC stroll").ends_with("(FAIL)") && l2.iter().any(|x| x.contains("CANDIDATE 2,1,1") && x.contains("drawn at it")), "{l2:?}");
        let mut under = s.clone();
        under.blocks.push((b(2, -1, 1), redsim::block::Block::parse("minecraft:stone").unwrap()));
        under.lo = b(under.lo.x, -2, under.lo.z);
        let (l3, n3, _) = super::stroll_candidates(&under, &[], b(0, 1, 0), None, &[], &[], &[b(2, 1, 1)], (under.lo, under.hi), false).unwrap();
        assert!(n3 > 0 && l3.iter().any(|x| x.contains("CANDIDATE 2,1,1") && x.contains("drawn at 2,0,1") && x.contains("moved up 1")), "the up-move form: {l3:?}");
        assert!(l3.iter().any(|x| x.starts_with("the column under 2,1,1") && x.contains("2,0,1 glass (isSolid true, isSolidRender false)") && x.contains("2,-1,1 stone (isSolid true, isSolidRender true)")), "{l3:?}");
    }

    /// (ironspec Nov 13 07:27 / 07:31 / 07:36, G1's three terms) The flee static and its positives:
    /// on a glass floor a rider 3 from a standing zombie has NO flee candidate in the getPosAway box;
    /// a stone floor planted away from the zombie yields one (DIRECT); a stone UNDER a glass cell
    /// yields the open cell above it (UP-MOVE); BURIED, the stone fill outside the box is no floor
    /// anyone can reach but a cave pocket in it is a candidate OUTSIDE the design's box; each FAILS.
    #[test]
    fn flee_none_and_the_planted_cell() {
        let mut s = glass_room();
        s.entities.clear();
        let bx = (s.lo, s.hi);
        let zombie = redsim::entity::Vec3::new(3.5, 1.0, 0.5);
        let (l, n, _) = super::flee_candidates_at(&s, b(0, 1, 0), None, zombie, 1.74, &[], &[], &[], bx, false).unwrap();
        let h = header(&l, "the FLEE's");
        assert!(n == 0 && h.ends_with("(PASS)") && h.contains("inside the flee's 6"), "{l:?}");
        let mut planted = s.clone();
        planted.blocks.retain(|(p, _)| *p != b(-3, 0, 0));
        planted.blocks.push((b(-3, 0, 0), redsim::block::Block::parse("minecraft:stone").unwrap()));
        let (l2, n2, _) = super::flee_candidates_at(&planted, b(0, 1, 0), None, zombie, 1.74, &[], &[], &[], bx, false).unwrap();
        assert!(n2 > 0 && header(&l2, "the FLEE's").ends_with("(FAIL)") && l2.iter().any(|x| x.contains("FLEE CANDIDATE -3,1,0")), "{l2:?}");
        // (ironspec Nov 13 13:50 (a): with the zombie beyond the flee's 6 (6.04 away), or hidden
        // behind a stone wall, the flee never runs: the same candidate is printed and counts nothing)
        let far = redsim::entity::Vec3::new(6.0, 1.0, 3.0);
        let (lf, nf, _) = super::flee_candidates_at(&planted, b(0, 1, 0), None, far, 1.74, &[], &[], &[], bx, false).unwrap();
        let hf = header(&lf, "the FLEE's");
        assert!(nf == 0 && hf.contains("beyond the flee's 6") && hf.ends_with("its candidates printed for the record)"), "{lf:?}");
        let mut hidden = planted.clone();
        for y in 1..=3 {
            for z in -2..=2 {
                hidden.blocks.retain(|(p, _)| *p != b(2, y, z));
                hidden.blocks.push((b(2, y, z), redsim::block::Block::parse("minecraft:stone").unwrap()));
            }
        }
        let (lh, nh, _) = super::flee_candidates_at(&hidden, b(0, 1, 0), None, zombie, 1.74, &[], &[], &[], bx, false).unwrap();
        let hh = header(&lh, "the FLEE's");
        assert!(nh == 0 && hh.contains("BLOCKED: the flee never runs") && hh.contains("(PASS: the flee never runs here"), "{lh:?}");
        let mut under = s.clone();
        under.blocks.push((b(-3, -1, 0), redsim::block::Block::parse("minecraft:stone").unwrap()));
        under.lo = b(under.lo.x, -2, under.lo.z);
        let (l3, n3, _) = super::flee_candidates_at(&under, b(0, 1, 0), None, zombie, 1.74, &[], &[], &[], (under.lo, under.hi), false).unwrap();
        assert!(n3 > 0 && l3.iter().any(|x| x.contains("FLEE CANDIDATE -3,1,0") && x.contains("drawn at -3,0,0") && x.contains("moved up 1")), "the up-move form: {l3:?}");
        // BURIED: the fill's own top surface lies outside the flee box's reach in this room only if
        // no air sits on it; a cave pocket west of the room (away from the zombie) is the positive
        let pocket = b(s.lo.x - 2, 1, 0);
        let (buried, _) = super::surround_spec(&s, super::Surround::Buried { pocket: Some(pocket) }, &super::read_boxes(b(0, 1, 0), true), &[b(0, 1, 0)]).unwrap();
        let (l4, _, out4) = super::flee_candidates_at(&buried, b(0, 1, 0), None, zombie, 1.74, &[], &[], &[], bx, false).unwrap();
        assert!(out4 > 0 && l4.iter().any(|x| x.contains(&format!("FLEE CANDIDATE {},{},{}", pocket.x, pocket.y, pocket.z)) && x.contains("OUTSIDE the design's box")), "the buried pocket: {l4:?}");
    }

    /// (G1 N39: caller sight's control) A caller on the rail sees the standing zombie 3 away (not
    /// blind); a glass wall between them makes it blind.
    #[test]
    fn caller_sight_seen_and_blind() {
        let caller = vec![("the rail's caller".to_string(), redsim::entity::Vec3::new(0.5, 1.0625, 0.5))];
        let (_, blind) = super::caller_sight(&room(false), &[], &caller).unwrap();
        assert_eq!(blind, 0);
        let mut walled = room(false);
        for z in -3..=3 {
            for y in 1..=4 {
                walled.blocks.push((b(2, y, z), redsim::block::Block::parse("minecraft:glass").unwrap()));
            }
        }
        let (l, blind2) = super::caller_sight(&walled, &[], &caller).unwrap();
        assert_eq!(blind2, 1, "{l:?}");
    }

    /// (G1 N39: staleness's control) Glass hides the zombie from the rail's south end only: those
    /// positions sit in a seen position's tail within 6.05 of the zombie, so staleness FAILS.
    #[test]
    fn staleness_fails_in_a_hidden_tail() {
        let mut s = room(false);
        for y in 1..=4 {
            s.blocks.push((b(2, y, 1), redsim::block::Block::parse("minecraft:glass").unwrap()));
        }
        let (_, v) = super::clear_all(&s, None, &[], &[], &[], &[], 39.0, &[], false, true, &[], &[], 39.0, &[], &[], &[], false, false, &super::PlaceClasses::default()).unwrap();
        assert!(v.stale > 0 && v.stale_samples > 0, "{v:?}");
    }

    /// (G1 N39: the STAMP classes' control) A calm rider (no hostile) on a glass floor whose only
    /// stable cell is the caged pillar top: its calm stroll can only draw that unreached cell, so
    /// the class is "arms, never clears".
    #[test]
    fn stamp_class_arms_never_clears() {
        let mut s = room(true);
        s.entities.clear();
        for (p, blk) in s.blocks.iter_mut() {
            if p.y == 0 {
                *blk = redsim::block::Block::parse("minecraft:glass").unwrap();
            }
        }
        let places = super::check(&s, None, &[], &[]).unwrap();
        let rows = super::stamp_classes(&places, true);
        assert!(rows.iter().any(|r| r.contains("arms, never clears")), "{rows:?}");
    }

    /// (G1 N39 / N15: auto-columns' control) A bubble column over soul sand is a column; a rail end
    /// that falls with nothing below in the box is listed as an UNLANDED track end.
    #[test]
    fn auto_columns_column_and_unlanded() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = vec![(b(5, 0, 0), p("minecraft:soul_sand"))];
        for y in 1..=4 {
            blocks.push((b(5, y, 0), p("minecraft:bubble_column[drag=false]")));
        }
        blocks.push((b(0, 5, 1), p("minecraft:stone")));
        blocks.push((b(0, 6, 1), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(8, 1, 3), lo: b(-3, -1, -3), hi: b(8, 8, 3), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (cols, lines) = super::auto_columns(&spec, &[]);
        assert!(cols.iter().any(|c| c[0] == 5.5 && c[2] == 0.5), "{cols:?} {lines:?}");
        assert!(lines.iter().any(|l| l.contains("UNLANDED")), "{lines:?}");
        // (58f770cc / ironspec 16:34: the same lone rail on a top-half trapdoor cap is a golem guard:
        // named, no track end, no fall; with a rail beside it, it is track again)
        let mut g = spec.blocks.clone();
        g.retain(|(q, _)| *q != b(0, 5, 1));
        g.push((b(0, 5, 1), p("minecraft:oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")));
        let gspec = redsim::entity::mob::farm::FarmSpec { blocks: g.clone(), ..spec.clone() };
        let (_, gl) = super::auto_columns(&gspec, &[]);
        assert!(gl.iter().any(|l| l.contains("0,6,1 is a golem guard")) && !gl.iter().any(|l| l.contains("UNLANDED")), "{gl:?}");
        let mut t = g;
        t.push((b(0, 6, 2), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        t.push((b(0, 5, 2), p("minecraft:stone")));
        let tspec = redsim::entity::mob::farm::FarmSpec { blocks: t, ..spec.clone() };
        let (_, tl) = super::auto_columns(&tspec, &[]);
        assert!(!tl.iter().any(|l| l.contains("golem guard")), "{tl:?}");
        // (G1 / ironspec 18:10 on part 13: an OPEN cap, a bottom-half cap, a powered or a detector
        // rail on a closed top-half cap: no guard; a fall landing on a guard cell: UNLANDED)
        for (rail_s, cap) in [
            ("minecraft:rail[shape=north_south,waterlogged=false]", "minecraft:oak_trapdoor[facing=north,half=top,open=true,powered=false,waterlogged=false]"),
            ("minecraft:rail[shape=north_south,waterlogged=false]", "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]"),
            ("minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]", "minecraft:oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]"),
            ("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]", "minecraft:oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]"),
        ] {
            let mut n = spec.blocks.clone();
            n.retain(|(q, _)| *q != b(0, 5, 1) && *q != b(0, 6, 1));
            n.push((b(0, 5, 1), p(cap)));
            n.push((b(0, 6, 1), p(rail_s)));
            let nspec = redsim::entity::mob::farm::FarmSpec { blocks: n, ..spec.clone() };
            let (_, nl) = super::auto_columns(&nspec, &[]);
            assert!(!nl.iter().any(|l| l.contains("golem guard")), "{rail_s} on {cap}: {nl:?}");
        }
        let mut f = gspec.blocks.clone();
        f.push((b(0, 8, 0), p("minecraft:stone")));
        f.push((b(0, 9, 0), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        let fspec = redsim::entity::mob::farm::FarmSpec { blocks: f, hi: b(8, 10, 3), ..spec.clone() };
        let (_, fl) = super::auto_columns(&fspec, &[]);
        assert!(fl.iter().any(|l| l.contains("lands on the golem guard 0,6,1") && l.contains("UNLANDED")), "{fl:?}");
        // (ironspec 18:31: a rail end opening into the bubble column's side, inside its y range, is
        // carried by the column: no fall, no track end)
        let mut s = spec.blocks.clone();
        s.push((b(6, 2, 0), p("minecraft:stone")));
        s.push((b(6, 3, 0), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        let sspec = redsim::entity::mob::farm::FarmSpec { blocks: s.clone(), ..spec.clone() };
        let (_, sl) = super::auto_columns(&sspec, &[]);
        assert!(sl.iter().any(|l| l.contains("the rail end 6,3,0 into 5,3,0 enters the column x 5.5 z 0.5") && l.contains("LIFTS it (soul sand)") && l.contains("no fall of its own")), "{sl:?}");
        assert!(!sl.iter().any(|l| l.contains("off the rail end 6,3,0 into 5,3,0")), "{sl:?}");
        // (G1 18:41: the same side entry over MAGMA: the settled world's column pulls the cart down)
        let mut dn = s.clone();
        dn.retain(|(q, _)| q.x != 5 || q.z != 0);
        dn.push((b(5, 0, 0), p("minecraft:magma_block")));
        for y in 1..=4 {
            dn.push((b(5, y, 0), p("minecraft:bubble_column[drag=true]")));
        }
        let dspec = redsim::entity::mob::farm::FarmSpec { blocks: dn, ..spec.clone() };
        let (_, dl) = super::auto_columns(&dspec, &[]);
        assert!(dl.iter().any(|l| l.contains("the rail end 6,3,0 into 5,3,0 enters the column") && l.contains("pulls it DOWN (magma): a DESCENT to y 1, onto magma_block at 5,0,0: no rail: a TRACK END")), "the descent judged to the bottom: {dl:?}");
        // (ironspec 19:26: a DECLARED column with no bubble block at the entry carries nothing: the
        // fall is judged, "a declared column, no bubble block here" printed)
        let mut nb = s.clone();
        nb.retain(|(q, _)| q.x != 5 || q.z != 0);
        let nspec = redsim::entity::mob::farm::FarmSpec { blocks: nb, ..spec.clone() };
        let (_, nl) = super::auto_columns(&nspec, &[[5.5, 1.0, 0.5, 4.0]]);
        assert!(nl.iter().any(|l| l.contains("opens into the column x 5.5 z 0.5") && l.contains("a declared column, no bubble block here")), "{nl:?}");
        assert!(!nl.iter().any(|l| l.contains("enters the column")), "nothing carried: {nl:?}");
        // (G1 20:02 E3: and no end rule credits it either: the fall, never "taken as that column's entry
        // or exit"; edge twins: the entry at the declared top (inside the range: the fall) and one above
        // it (outside: the column's top exit, the end rule))
        assert!(nl.iter().any(|l| l.contains("a free fall off the rail end 6,3,0 into 5,3,0")) && !nl.iter().any(|l| l.contains("6,3,0 into 5,3,0 is within 1 of the column")), "the fall, not the end rule: {nl:?}");
        let edge = |y: i32| {
            let mut e: Vec<(redsim::pos::BlockPos, redsim::block::Block)> = spec.blocks.iter().filter(|(q, _)| q.x != 5 || q.z != 0).copied().collect();
            e.push((b(6, y - 1, 0), p("minecraft:stone")));
            e.push((b(6, y, 0), p("minecraft:rail[shape=east_west,waterlogged=false]")));
            super::auto_columns(&redsim::entity::mob::farm::FarmSpec { blocks: e, ..spec.clone() }, &[[5.5, 1.0, 0.5, 4.0]]).1
        };
        let at_top = edge(4);
        assert!(at_top.iter().any(|l| l.contains("a free fall off the rail end 6,4,0 into 5,4,0")) && !at_top.iter().any(|l| l.contains("6,4,0 into 5,4,0 is within 1 of the column")), "at the declared top: {at_top:?}");
        let above = edge(5);
        assert!(above.iter().any(|l| l.contains("the rail end 6,5,0 into 5,5,0 is within 1 of the column x 5.5 z 0.5 y 1.0..4.0's end")), "one above the top: {above:?}");
        // (G1 20:02 E4: the walk takes a credited side entry as a column from the entry cell: on the
        // soul-sand twin (fed by a rail from the east, so a cart arrives moving into the side) the rail
        // end into the lift's side is no open end (without that column it is); on the magma twin the
        // descent is entered and its column ends at the bottom with no rail: an open end there)
        let fed = |mut v: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| {
            v.push((b(7, 2, 0), p("minecraft:stone")));
            v.push((b(7, 3, 0), p("minecraft:rail[shape=east_west,waterlogged=false]")));
            redsim::entity::mob::farm::FarmSpec { blocks: v, ..spec.clone() }
        };
        let sfed = fed(sspec.blocks.clone());
        let (scols, _) = super::auto_columns(&sfed, &[]);
        let into_side = |t: &super::Track| t.open_ends(super::TailNode::Rail(6, 3, 0)).iter().any(|(d, _)| d.contains("exit to 5,3,0"));
        let st = super::Track::new(&sfed, &scols, &[]);
        assert!(scols.iter().any(|c| c[1] == 3.0 && c[3] > 3.0) && !into_side(&st), "the lift's side entry is walked: {scols:?}");
        let plain: Vec<[f64; 4]> = scols.iter().filter(|c| c[1] != 3.0).copied().collect();
        assert!(into_side(&super::Track::new(&sfed, &plain, &[])), "without the entry's column it is an open end");
        let dfed = fed(dspec.blocks.clone());
        let (dcols, _) = super::auto_columns(&dfed, &[]);
        let dt = super::Track::new(&dfed, &dcols, &[]);
        let k = dcols.iter().position(|c| *c == [5.5, 3.0, 0.5, 1.0]).unwrap_or_else(|| panic!("the descent's column: {dcols:?}"));
        assert!(!into_side(&dt) && !dt.open_ends(super::TailNode::Col(k, dt.col_len[k] - 1)).is_empty(), "the descent entered, an end at its bottom: {:?}", dt.open_ends(super::TailNode::Col(k, dt.col_len[k] - 1)));
        // (ironspec 19:15 (2): a FAILED settle credits no side entry: water at the box's edge flows out
        // of it (a model error), so the side entry falls through to the fall, the error printed)
        let mut bad = s;
        bad.push((b(-3, 0, -3), p("minecraft:stone")));
        bad.push((b(-3, 1, -3), p("minecraft:water[level=0]")));
        let bspec = redsim::entity::mob::farm::FarmSpec { blocks: bad, ..spec };
        assert!(super::settled_world_void(&bspec).is_err(), "the forced settle failure");
        let (_, bl) = super::auto_columns(&bspec, &[]);
        assert!(bl.iter().any(|l| l.contains("the rail end 6,3,0 into 5,3,0 opens into the column") && l.contains("the settle FAILED")), "{bl:?}");
        assert!(bl.iter().any(|l| l.contains("a free fall off the rail end 6,3,0 into 5,3,0")) && !bl.iter().any(|l| l.contains("enters the column")), "the fall kept: {bl:?}");
        // (ironspec 19:32: and the run fails by name: the CLI's gate on these lines)
        let gate = super::auto_columns_judged(&bl);
        assert!(gate.as_ref().is_err_and(|e| e.starts_with("auto columns: NOT JUDGED: 1 side entr(ies)")), "{gate:?}");
        assert!(super::auto_columns_judged(&sl).is_ok(), "a judged run passes the gate");
    }

    /// (tick-exact 18:13, ironspec 18:15 (a) / (5), G1 18:15 / 18:19) The golem floor rule pinned
    /// both ways on a stone platform with one caller: ACCEPTED an air or a water feet cell over
    /// stone; REFUSED a glass floor, a carpet floor, a rail or a redstone torch in the feet cell
    /// (both FRAGILE: remove it and a golem spawns), a full block one above (not fragile), a rail at
    /// feet + 1 on a top slab in the feet cell (refused by the slab too: not fragile) and a rail at
    /// feet + 2 on a top slab at feet + 1 (FRAGILE: only the rail refuses it).
    /// (G1 Nov 13 20:02, E1 / E2: the FRAGILE pins on the real path) Through `route_golem_floors`: a
    /// waterlogged rail on stone (walled in) is listed "feet not air or liquid", FRAGILE, with its one
    /// FAULT NOTE, exactly as its waterlogged=false twin; an observer at feet + 1 over stone (a string on
    /// it, so it is no floor) and a lone string in a feet cell are both FRAGILE with their FAULT NOTEs
    /// naming them; the FAULT NOTE count equals the FRAGILE count.
    #[test]
    fn golem_fragile_real_path_waterlogged_rail_observer_and_string() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s}"))));
        let string = "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]";
        let run = |wl: bool| {
            let mut blocks = Vec::new();
            for x in 0..=8 {
                for z in 0..=8 {
                    blocks.push((b(x, 0, z), p("minecraft:stone")));
                }
            }
            blocks.push((b(4, 1, 4), p("minecraft:rail[shape=north_south,waterlogged=false]")));
            blocks.push((b(1, 1, 1), p(if wl { "minecraft:rail[shape=north_south,waterlogged=true]" } else { "minecraft:rail[shape=north_south,waterlogged=false]" })));
            for c in [b(0, 1, 1), b(2, 1, 1), b(1, 1, 0), b(1, 1, 2)] {
                blocks.push((c, p("minecraft:stone")));
            }
            blocks.push((b(7, 2, 1), p("minecraft:observer[facing=north,powered=false]")));
            blocks.push((b(7, 3, 1), p(string)));
            blocks.push((b(1, 1, 7), p(string)));
            let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(4, -1, 4), lo: b(0, -1, 0), hi: b(8, 6, 8), block_entities: Vec::new(), rock: false, nether: None, land: false };
            super::route_golem_floors(&spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}")).0
        };
        for wl in [true, false] {
            let gl = run(wl);
            let r = gl.iter().find(|l| l.trim_start().starts_with("refused stop cells:")).unwrap_or_else(|| panic!("{gl:?}"));
            let fragile = r.split("FRAGILE").nth(1).and_then(|s| s.split("; STATE").next()).unwrap_or("");
            let notes: Vec<&String> = gl.iter().filter(|l| l.trim_start().starts_with("FAULT NOTE ")).collect();
            let n_fragile: usize = fragile.split("refuses it): ").nth(1).and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next()).and_then(|d| d.parse().ok()).unwrap_or(0);
            assert!(fragile.contains("1,1,1 (feet not air or liquid (rail[shape=north_south,waterlogged=") && notes.iter().filter(|l| l.contains("FAULT NOTE 1,1,1:") && l.contains("the rail at 1,1,1")).count() == 1, "the rail (waterlogged {wl}): {r} / {notes:?}");
            assert!(fragile.contains("7,1,1 (") && fragile.contains("observer") && notes.iter().any(|l| l.contains("FAULT NOTE 7,1,1:") && l.contains("the observer at 7,2,1")), "the observer at feet + 1 (waterlogged {wl}): {r} / {notes:?}");
            assert!(fragile.contains("1,1,7 (feet not air or liquid (tripwire") && notes.iter().any(|l| l.contains("FAULT NOTE 1,1,7:") && l.contains("the tripwire at 1,1,7")), "the lone string (waterlogged {wl}): {r} / {notes:?}");
            assert!(n_fragile > 0 && notes.len() == n_fragile, "one FAULT NOTE per FRAGILE cell (waterlogged {wl}): {n_fragile} vs {}: {r} / {notes:?}", notes.len());
        }
    }

    #[test]
    fn golem_rule_pinned_both_ways() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in 0..=6 {
            for z in 0..=6 {
                blocks.push((b(x, 0, z), p(if (x, z) == (6, 6) { "minecraft:glass" } else { "minecraft:stone" })));
            }
        }
        let rail = "minecraft:rail[shape=north_south,waterlogged=false]";
        let top_slab = "minecraft:smooth_stone_slab[type=top,waterlogged=false]";
        blocks.push((b(3, 1, 3), p(rail)));
        blocks.push((b(1, 1, 1), p("minecraft:water[level=0]")));
        for c in [b(0, 1, 1), b(2, 1, 1), b(1, 1, 0), b(1, 1, 2)] {
            blocks.push((c, p("minecraft:stone")));
        }
        blocks.push((b(6, 1, 0), redsim::block::Block::Terrain(redsim::terrain::blocks::id_of("minecraft:moss_carpet").expect("a carpet in the terrain table"))));
        blocks.push((b(0, 1, 6), p(rail)));
        blocks.push((b(6, 1, 3), p("minecraft:redstone_torch[lit=true]")));
        blocks.push((b(4, 2, 6), p("minecraft:stone")));
        blocks.push((b(2, 1, 6), p(top_slab)));
        blocks.push((b(2, 2, 6), p(rail)));
        blocks.push((b(0, 2, 3), p(top_slab)));
        blocks.push((b(0, 3, 3), p(rail)));
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(3, 0, 3), lo: b(0, 0, 0), hi: b(6, 5, 6), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (gl, _, cells) = super::route_golem_floors(&spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
        let has = |x: i32, y: i32, z: i32| cells.contains(&b(x, y, z));
        // (accepted)
        assert!(has(5, 1, 1) && has(1, 1, 1), "an air and a water feet cell over stone: {cells:?}");
        // (refused)
        for (c, why) in [((6, 1, 6), "a glass floor"), ((6, 1, 0), "a carpet in the feet cell"), ((6, 2, 0), "a carpet floor"), ((0, 1, 6), "a rail in the feet cell"), ((6, 1, 3), "a torch in the feet cell"), ((4, 1, 6), "a full block one above"), ((2, 1, 6), "a rail at feet + 1 on a slab"), ((0, 1, 3), "a rail at feet + 2")] {
            assert!(!has(c.0, c.1, c.2), "{why} refuses {c:?}: {cells:?}");
        }
        let r = gl.iter().find(|l| l.trim_start().starts_with("refused stop cells:")).unwrap_or_else(|| panic!("{gl:?}"));
        let fragile = r.split("FRAGILE").nth(1).and_then(|s| s.split("; STATE").next()).unwrap_or("");
        // (a rail at feet + 1 needs a support in the feet cell: over the slab the spot is the rail's
        // own cell, refused by the rail alone (fragile), and under it refused by the slab too)
        assert!(fragile.contains("0,1,6 (") && fragile.contains("6,1,3 (") && fragile.contains("0,1,3 (2 above: rail") && fragile.contains("2,2,6 ("), "the removable refusals: {r}");
        assert!(!fragile.contains("2,1,6 (") && !fragile.contains("6,1,0 (") && r.contains("feet not air or liquid"), "not fragile: {r}");
        // (ironspec 18:35 / G1 18:37: FRAGILE and STATE by the block's KIND: a rail term is removable,
        // a lit campfire's danger is a state (unlit, it isn't dangerous), a stone's full block neither)
        let w = super::settled_world_void(&spec).unwrap_or_else(|e| panic!("{e}"));
        let t = |kind: super::GolemTermKind, s: &str| super::GolemTerm { at: 1, kind, block: p(s), pos: b(5, 2, 5) };
        let rail_t = t(super::GolemTermKind::Rail, rail);
        let fire_t = t(super::GolemTermKind::Dangerous, "minecraft:campfire[facing=north,lit=true,signal_fire=false,waterlogged=false]");
        let stone_t = t(super::GolemTermKind::FullBlock, "minecraft:stone");
        assert!(rail_t.removable(&w) && !rail_t.stateful(&w), "a rail: removable");
        assert!(fire_t.stateful(&w) && !fire_t.removable(&w), "a lit campfire: a state");
        assert!(!stone_t.removable(&w) && !stone_t.stateful(&w), "stone: neither");
        // (ironspec 19:11: a lone string in the feet cell is a guard: its "feet not air or liquid" term
        // is removable, so the cell is FRAGILE)
        let string = "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]";
        let string_b = redsim::block::Block::parse(string).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(string).expect("the string state")));
        let string_t = super::GolemTerm { at: 0, kind: super::GolemTermKind::FeetNotAirOrLiquid, block: string_b, pos: b(5, 1, 5) };
        assert!(string_t.removable(&w), "a lone string: removable");
        // (G1 19:23: removability per BLOCK: an observer (a full-block signal source) at feet + 1 raises
        // a full-block and a signal-source term, both removable, so its cell is FRAGILE; a floor term
        // never is)
        let obs = p("minecraft:observer[facing=north,powered=false]");
        let obs_full = super::GolemTerm { at: 1, kind: super::GolemTermKind::FullBlock, block: obs, pos: b(5, 2, 5) };
        let obs_src = super::GolemTerm { at: 1, kind: super::GolemTermKind::SignalSource, block: obs, pos: b(5, 2, 5) };
        assert!(obs_full.removable(&w) && obs_src.removable(&w), "an observer at feet + 1: FRAGILE");
        let floor_t = super::GolemTerm { at: -1, kind: super::GolemTermKind::FloorNotSolid, block: string_b, pos: b(5, 0, 5) };
        assert!(!floor_t.removable(&w), "a floor term is never removable");
        // (G1 19:23 / 20:02 E1: the feet test is the BLOCK's liquid(): a waterlogged rail's term stands;
        // pinned on the real path in `golem_fragile_real_path_waterlogged_rail_observer_and_string`)
    }

    /// (ironspec Nov 13 19:13 / 19:45 (c), G1 20:02 E5 / E6) The update-reach footprints: a lone string
    /// passes and an observer beside it fails; a hook takes its support's neighbours (an observer beside
    /// the support fails); a plate takes the block under it's neighbours (a piston beside the stone under
    /// it fails), as `pushscan::update_reach` has it; a piston head beside a string fails; a cell holding
    /// no string, hook, plate or detector is NOT JUDGED.
    #[test]
    fn update_reach_footprints_string_hook_plate_and_not_judged() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s}"))));
        let string_b = p("minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]");
        let mk = |extra: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| {
            let mut blocks = Vec::new();
            for x in 0..=4 {
                for z in 0..=4 {
                    blocks.push((b(x, 0, z), p("minecraft:stone")));
                }
            }
            blocks.extend(extra);
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(4, 5, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let (ul, un) = super::update_reach_components(&mk(vec![(b(2, 1, 2), string_b)]), &[b(2, 1, 2)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(un == 0 && ul[0].ends_with("(PASS)"), "a lone string: {ul:?}");
        let (ol, on) = super::update_reach_components(&mk(vec![(b(2, 1, 2), string_b), (b(3, 1, 2), p("minecraft:observer[facing=west,powered=false]"))]), &[b(2, 1, 2)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(on == 1 && ol[0].contains("observer") && ol[0].ends_with("(FAIL)"), "an observer beside: {ol:?}");
        // a hook facing west on a glass support (2,1,1): an observer beside the support (3,1,1)
        let hook = p("minecraft:tripwire_hook[attached=false,facing=west,powered=false]");
        let (hl, hn) = super::update_reach_components(&mk(vec![(b(1, 1, 1), hook), (b(2, 1, 1), p("minecraft:glass"))]), &[b(1, 1, 1)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(hn == 0 && hl[0].contains("a tripwire hook: its footprint, 12 cells") && hl[0].ends_with("(PASS)"), "a lone hook: {hl:?}");
        let (hl2, hn2) = super::update_reach_components(&mk(vec![(b(1, 1, 1), hook), (b(2, 1, 1), p("minecraft:glass")), (b(3, 1, 1), p("minecraft:observer[facing=west,powered=false]"))]), &[b(1, 1, 1)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(hn2 == 1 && hl2[0].contains("at 3,1,1") && hl2[0].ends_with("(FAIL)"), "beside the support: {hl2:?}");
        // a plate on stone: a piston beside the stone under it
        let plate = p("minecraft:stone_pressure_plate[powered=false]");
        let mut pb = mk(vec![(b(2, 1, 3), plate)]);
        pb.blocks.retain(|(q, _)| *q != b(3, 0, 3));
        pb.blocks.push((b(3, 0, 3), p("minecraft:piston[extended=false,facing=up]")));
        let (pl, pn) = super::update_reach_components(&pb, &[b(2, 1, 3)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(pn == 1 && pl[0].contains("piston[") && pl[0].contains("at 3,0,3") && pl[0].ends_with("(FAIL)"), "a piston beside the plate's floor: {pl:?}");
        // a piston head beside a string (its piston held extended by a redstone block)
        let (kl, kn) = super::update_reach_components(&mk(vec![(b(2, 1, 2), string_b), (b(2, 2, 2), p("minecraft:piston_head[facing=down,short=false,type=normal]")), (b(2, 3, 2), p("minecraft:piston[extended=true,facing=down]")), (b(2, 4, 2), p("minecraft:redstone_block"))]), &[b(2, 1, 2)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(kn == 1 && kl[0].contains("piston_head"), "a piston head beside: {kl:?}");
        // (G1 21:38 H8) an ATTACHED string between two hooks: an observer beside one hook's support, two
        // cells from the string, fails through that hook's footprint; without the observer it passes; and
        // a plate on stone with nothing beside its floor passes (the plate's passing twin)
        let att = p("minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]");
        let line = vec![
            (b(1, 1, 1), p("minecraft:glass")),
            (b(2, 1, 1), p("minecraft:tripwire_hook[attached=true,facing=east,powered=false]")),
            (b(3, 1, 1), att),
            (b(4, 1, 1), p("minecraft:tripwire_hook[attached=true,facing=west,powered=false]")),
            (b(5, 1, 1), p("minecraft:glass")),
        ];
        let mk5 = |extra: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| {
            let mut blocks = Vec::new();
            for x in 0..=6 {
                for z in 0..=3 {
                    blocks.push((b(x, 0, z), p("minecraft:stone")));
                }
            }
            blocks.extend(extra);
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(6, 4, 3), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let (al, an) = super::update_reach_components(&mk5(line.clone()), &[b(3, 1, 1)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(an == 0 && al[0].contains("an attached string: its six faces and its hook(s)") && al[0].contains("2,1,1") && al[0].contains("4,1,1") && al[0].ends_with("(PASS)"), "an attached string: {al:?}");
        let mut obs_line = line;
        obs_line.push((b(6, 1, 1), p("minecraft:observer[facing=west,powered=false]")));
        let (ol2, on2) = super::update_reach_components(&mk5(obs_line), &[b(3, 1, 1)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(on2 == 1 && ol2[0].contains("observer") && ol2[0].contains("at 6,1,1") && ol2[0].ends_with("(FAIL)"), "an observer beside a hook's support: {ol2:?}");
        let (ql, qn) = super::update_reach_components(&mk(vec![(b(2, 1, 3), plate)]), &[b(2, 1, 3)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(qn == 0 && ql[0].ends_with("(PASS)"), "a lone plate: {ql:?}");
        // a cell holding none of them
        let e = super::update_reach_components(&mk(Vec::new()), &[b(2, 1, 2)]).err().unwrap_or_default();
        assert!(e.contains("NOT JUDGED") && e.contains("2,1,2"), "air: {e}");
    }

    /// (ironspec Nov 13 19:11, with day-1) The riderless statics, each both ways: a zombie seated in a
    /// boat beside a rail line strikes a rider (reach and sight), one two cells further reaches none;
    /// a rider's box meeting a framed lit portal fails; a rail end over a 29-block drop kills a
    /// villager passenger, water at the bottom breaks the fall.
    #[test]
    fn riderless_zombie_reach_portal_and_fatal_fall_both_ways() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s}"))));
        let mut base = Vec::new();
        for x in 0..=7 {
            for z in 0..=6 {
                base.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        for z in 1..=5 {
            base.push((b(3, 1, z), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        }
        let mk = |blocks: Vec<(redsim::pos::BlockPos, redsim::block::Block)>, ents: Vec<(String, [f64; 3], Option<String>)>| redsim::entity::mob::farm::FarmSpec { blocks, entities: ents, output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(7, 32, 6), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let zb = |x: f64| ("minecraft:oak_boat".to_string(), [x, 1.0, 3.5], Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".to_string()));
        let (l1, n1) = super::riderless_statics(&mk(base.clone(), vec![zb(4.5)]), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n1 > 0 && l1.iter().any(|l| l.starts_with("(c) the zombie at") && l.contains("it can strike") && l.ends_with("(FAIL)")), "beside: {l1:?}");
        let (l2, n2) = super::riderless_statics(&mk(base.clone(), vec![zb(6.5)]), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n2 == 0 && l2.iter().any(|l| l.starts_with("(c) the zombie at") && l.contains(": 0 route point(s) in reach") && l.ends_with("(PASS)")), "further: {l2:?}");
        // a framed lit portal the rail's end runs into
        let mut pb = base.clone();
        pb.retain(|(q, _)| q.x != 3 || q.y != 1);
        for (x, y) in [(0, 1), (1, 1), (2, 1), (3, 1), (0, 5), (1, 5), (2, 5), (3, 5), (0, 2), (0, 3), (0, 4), (3, 2), (3, 3), (3, 4)] {
            pb.push((b(x, y, 6), p("minecraft:obsidian")));
        }
        for x in 1..=2 {
            for y in 2..=4 {
                pb.push((b(x, y, 6), p("minecraft:nether_portal[axis=x]")));
            }
        }
        pb.push((b(1, 1, 5), p("minecraft:stone")));
        pb.push((b(1, 2, 5), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        let (l3, n3) = super::riderless_statics(&mk(pb, Vec::new()), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n3 > 0 && l3.iter().any(|l| l.starts_with("(d) portal cells the cart's box meets") && l.contains("the cart on 1,2,5 meets the portal cell 1,2,6") && l.ends_with("(FAIL)")), "the portal: {l3:?}");
        // a rail end 29 blocks over the floor, then water at the bottom
        let mut fb = base.clone();
        fb.push((b(3, 29, 3), p("minecraft:stone")));
        fb.push((b(3, 30, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        fb.push((b(2, 30, 3), p("minecraft:stone")));
        let (l4, n4) = super::riderless_statics(&mk(fb.clone(), Vec::new()), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n4 > 0 && l4.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,30,3 into 4,30,3") && l.contains("down 29 blocks") && l.contains("26 damage: FATAL to a villager (20) (FAIL)")), "the fall: {l4:?}");
        let mut wb = fb;
        wb.push((b(4, 1, 3), p("minecraft:water[level=0]")));
        for (x, z) in [(5, 3), (4, 2), (4, 4)] {
            wb.push((b(x, 1, z), p("minecraft:stone")));
        }
        let (l5, _) = super::riderless_statics(&mk(wb, Vec::new()), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(l5.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,30,3 into 4,30,3") && l.contains("water in the fall's swept column breaks it")), "water: {l5:?}");
        // (ironspec 22:12 (2)'s controls) a dry 8-block drop onto stone: 5; the same onto slime: 0; onto a
        // rail on slime: 0 (the support is the landed-on block); a 29-block drop onto a rail on stone:
        // FALL DAMAGE UNCONFIRMED (moveAlongTrack's reset pending the pin), held as a FAIL
        let drop8 = |floor: &str, rail: bool| {
            let mut v = base.clone();
            v.retain(|(q, _)| *q != b(4, 0, 3));
            v.push((b(4, 0, 3), p(floor)));
            v.push((b(3, 8, 3), p("minecraft:stone")));
            v.push((b(3, 9, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
            v.push((b(2, 9, 3), p("minecraft:stone")));
            if rail {
                v.push((b(4, 1, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
            }
            super::riderless_statics(&mk(v, Vec::new()), &[]).unwrap_or_else(|e| panic!("{e}")).0
        };
        let stone8 = drop8("minecraft:stone", false);
        assert!(stone8.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,9,3 into 4,9,3") && l.contains("down 8 blocks") && l.contains("landing on the stone at 4,0,3 (fallOn x 1): a passenger takes 5 damage")), "onto stone: {stone8:?}");
        let slime8 = drop8("minecraft:slime_block", false);
        assert!(slime8.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,9,3 into 4,9,3") && l.contains("landing on the slime_block at 4,0,3: fallOn x 0: no damage")), "onto slime: {slime8:?}");
        let rslime = drop8("minecraft:slime_block", true);
        assert!(rslime.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,9,3 into 4,9,3") && l.contains("to the rail 4,1,3") && l.contains("landing on the slime_block at 4,0,3: fallOn x 0: no damage")), "onto a rail on slime: {rslime:?}");
        let mut rb = base.clone();
        rb.push((b(3, 29, 3), p("minecraft:stone")));
        rb.push((b(3, 30, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        rb.push((b(2, 30, 3), p("minecraft:stone")));
        rb.push((b(4, 1, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        let (lr, nr) = super::riderless_statics(&mk(rb, Vec::new()), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(nr >= 1 && lr.iter().any(|l| l.starts_with("(e) a free fall off the rail end 3,30,3 into 4,30,3") && l.contains("FALL DAMAGE UNCONFIRMED: up to 25") && l.ends_with("(FAIL: a hold pending the pin)")), "onto a rail on stone: {lr:?}");
        // (G1 21:38 H4) the boat's zombie at its SEAT: the boat at y 1.0, its feet 1.0 + 0.1875 - 0.7, its
        // attack box y 0.49..2.44 (unseated, where the static world leaves it: 1.00..2.95)
        assert!(l1.iter().any(|l| l.starts_with("(c) the zombie at") && l.contains("seated: feet y 0.4875") && l.contains(" y 0.49..2.44 ")), "the seat: {l1:?}");
        // (G1 21:38 H3) an allow with no activator rail in the design is stale
        let (l6, n6) = super::riderless_statics(&mk(base.clone(), Vec::new()), &[b(1, 1, 1)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n6 == 1 && l6.iter().any(|l| l.starts_with("(a) allow 1,1,1: the design has no activator rail: a stale allow (FAIL)")), "a stale allow: {l6:?}");
        // (G1 21:38 H1) a rail end over the void: the fall leaves the design's box: FAIL, never a pass
        let mut vb = base.clone();
        vb.push((b(7, 4, 3), p("minecraft:stone")));
        vb.push((b(7, 5, 3), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        vb.push((b(6, 5, 3), p("minecraft:stone")));
        let vspec = redsim::entity::mob::farm::FarmSpec { hi: b(9, 32, 6), ..mk(vb, Vec::new()) };
        let (l7, n7) = super::riderless_statics(&vspec, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n7 >= 1 && l7.iter().any(|l| l.starts_with("(e) a free fall off the rail end 7,5,3 into 8,5,3") && l.ends_with("no drop to price (out of the design's box, or onto the golem guard): (FAIL)")), "over the void: {l7:?}");
        // no route at all: NOT JUDGED
        let e8 = super::riderless_statics(&mk(base.iter().filter(|(q, _)| q.y == 0).cloned().collect(), Vec::new()), &[]).err().unwrap_or_default();
        assert!(e8.contains("NOT JUDGED") && e8.contains("no route"), "no route: {e8}");
    }

    /// (tick-exact Nov 13 21:40, G1 21:41: the portal takes the CART, a seated rider refused) Riderless
    /// (d) tests the cart's 0.98 x 0.7 box: a framed lit portal whose cell is diagonal to a south_east
    /// curve is met by the cart's box along the curve's chord (FAIL); the same frame one block up, out of
    /// the cart's 0.7 but inside a rider's 1.95, passes (the portal refuses the passenger).
    #[test]
    fn riderless_portal_by_the_cart_box_beside_a_curve() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s}"))));
        let mk = |dy: i32| {
            let mut blocks = Vec::new();
            for x in 0..=7 {
                for z in 0..=7 {
                    blocks.push((b(x, 0, z), p("minecraft:stone")));
                }
            }
            blocks.push((b(3, 1, 3), p("minecraft:rail[shape=south_east,waterlogged=false]")));
            // a z-axis portal at x 4, z 4..5, its bottom at y 1 + dy (a frame of obsidian round it)
            let y0 = 1 + dy;
            for z in 4..=5 {
                blocks.push((b(4, y0 - 1, z), p("minecraft:obsidian")));
                blocks.push((b(4, y0 + 3, z), p("minecraft:obsidian")));
                for y in y0..=y0 + 2 {
                    blocks.push((b(4, y, z), p("minecraft:nether_portal[axis=z]")));
                }
            }
            for y in y0..=y0 + 2 {
                blocks.push((b(4, y, 3), p("minecraft:obsidian")));
                blocks.push((b(4, y, 6), p("minecraft:obsidian")));
            }
            blocks.retain(|(q, s)| !(q.y == 0 && q.x == 4 && (4..=5).contains(&q.z) && s.to_string().contains("stone") && dy == 0) || true);
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(7, 8, 7), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let at = mk(0);
        let w = super::settled_world_void(&at).unwrap_or_else(|e| panic!("{e}"));
        assert!(w.get(b(4, 1, 4)).to_string().contains("nether_portal"), "the settle keeps the portal: {}", w.get(b(4, 1, 4)));
        let (l, n) = super::riderless_statics(&at, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n >= 1 && l.iter().any(|x| x.starts_with("(d) portal cells the cart's box meets") && x.contains("the cart on 3,1,3 meets the portal cell 4,1,4") && x.ends_with("(FAIL)")), "beside the curve: {l:?}");
        let up = mk(1);
        let wu = super::settled_world_void(&up).unwrap_or_else(|e| panic!("{e}"));
        assert!(wu.get(b(4, 2, 4)).to_string().contains("nether_portal"), "the settle keeps the raised portal: {}", wu.get(b(4, 2, 4)));
        let (lu, _) = super::riderless_statics(&up, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(lu.iter().any(|x| x.starts_with("(d) portal cells the cart's box meets") && x.contains(": 0 (PASS)")), "one block up: {lu:?}");
    }

    /// (G1 Nov 13 21:38 H5) Riderless (c) checks every route position, the lift columns' too: a zombie
    /// standing on the glass beside a soul-sand lift's top, with no rail in the design, strikes a rider
    /// rising in the column (reach and sight).
    #[test]
    fn riderless_zombie_beside_a_lift_column() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(s).unwrap_or_else(|| panic!("{s}"))));
        let mut blocks = Vec::new();
        for x in 0..=8 {
            for z in 0..=6 {
                blocks.push((b(x, 0, z), p(if (x, z) == (6, 3) { "minecraft:soul_sand" } else { "minecraft:stone" })));
            }
        }
        for y in 1..=2 {
            blocks.push((b(6, y, 3), p("minecraft:bubble_column[drag=false]")));
            for (x, z) in [(5, 3), (7, 3), (6, 2), (6, 4)] {
                blocks.push((b(x, y, z), p("minecraft:glass")));
            }
        }
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: vec![("minecraft:zombie".to_string(), [5.5, 3.0, 3.5], Some("{PersistenceRequired:1b}".to_string()))], output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(8, 8, 6), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (l, n) = super::riderless_statics(&spec, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n >= 1 && l.iter().any(|x| x.starts_with("(c) the zombie at 5.50,3.00,3.50 (on foot") && x.contains("column ") && x.contains("CLEAR: it can strike") && x.ends_with("(FAIL)")), "beside the lift: {l:?}");
    }

    /// (ironspec Nov 13 19:20 / 19:29, G1 19:19) The power-into static: a lone activator rail on stone
    /// has no source in reach; a lever on the stone under it powers it through the conductor (FAIL,
    /// PASS when allowed); a lever beside a joined activator rail reaches it along the line; a dropper's
    /// quasi-connected cell above is read only with `qc`. And power-out of a hook (ironspec 19:46 (iii)).
    #[test]
    fn power_into_lone_activator_conductor_lever_chain_and_qc() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut base = Vec::new();
        for x in 0..=4 {
            for z in 0..=4 {
                base.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        let mk = |extra: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| {
            let mut blocks = base.clone();
            blocks.extend(extra);
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(0, -1, 0), lo: b(0, -1, 0), hi: b(4, 3, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let act = p("minecraft:activator_rail[powered=false,shape=north_south,waterlogged=false]");
        let (l0, n0) = super::power_into(&mk(vec![(b(2, 1, 2), act)]), &[b(2, 1, 2)], false, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n0 == 0 && l0[0].ends_with("(PASS)"), "a lone activator: {l0:?}");
        // a lever on the stone's east face under the rail: through the conductor
        let lever = p("minecraft:lever[face=wall,facing=east,powered=false]");
        let sp = mk(vec![(b(2, 1, 2), act), (b(3, 0, 2), lever)]);
        let mut sp2 = sp.clone();
        sp2.blocks.retain(|(q, _)| *q != b(3, 0, 2));
        sp2.blocks.push((b(3, 0, 2), lever));
        let (l1, n1) = super::power_into(&sp2, &[b(2, 1, 2)], false, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n1 == 1 && l1[0].contains("lever") && l1[0].contains("through the conductor stone at 2,0,2") && l1[0].contains("(FAIL)"), "a lever under: {l1:?}");
        let (l2, n2) = super::power_into(&sp2, &[b(2, 1, 2)], false, &[b(3, 0, 2)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n2 == 0 && l2[0].contains("(PASS)") && l2[0].contains("allowed: lever"), "allowed: {l2:?}");
        // a lever beside a joined activator: along the line
        let chain = mk(vec![(b(2, 1, 1), act), (b(2, 1, 2), act), (b(1, 1, 1), p("minecraft:lever[face=floor,facing=north,powered=false]"))]);
        let (l3, n3) = super::power_into(&chain, &[b(2, 1, 2)], false, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n3 >= 1 && l3[0].contains("joined same-kind rail(s) 2,1,1") && l3[0].contains("lever[") && l3[0].contains("at 1,1,1 beside 2,1,1"), "the chain: {l3:?}");
        // a dropper: the redstone block beside the cell above reaches it by quasi-connectivity, read by
        // DEFAULT (G1 21:38 H6: a tool default with an opt-out): without the opt-out it fails
        let qc = mk(vec![(b(0, 1, 4), p("minecraft:dropper[facing=up,triggered=false]")), (b(1, 2, 4), p("minecraft:redstone_block"))]);
        let (l4, n4) = super::power_into(&qc, &[b(0, 1, 4)], false, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n4 == 1 && l4[0].contains("quasi-connectivity") && l4[0].contains("redstone_block at 1,2,4 beside 0,2,4") && l4[0].contains("(FAIL)"), "qc by default: {l4:?}");
        let (l5, n5) = super::power_into(&qc, &[b(0, 1, 4)], true, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n5 == 0 && !l5[0].contains("quasi-connectivity"), "opted out: {l5:?}");
        // (G1 21:38 H2) a cell on air is NOT JUDGED, for both statics
        let e1 = super::power_into(&mk(Vec::new()), &[b(2, 1, 2)], false, &[]).err().unwrap_or_default();
        assert!(e1.contains("NOT JUDGED") && e1.contains("no power consumer"), "power into on air: {e1}");
        let e2 = super::power_out(&mk(Vec::new()), &[b(2, 2, 2)], &[]).err().unwrap_or_default();
        assert!(e2.contains("NOT JUDGED") && e2.contains("no signal source, no conductor"), "power out on air: {e2}");
        // (G1 21:38 H3) an allow that matches nothing is stale and fails
        let (l6, n6) = super::power_into(&mk(vec![(b(2, 1, 2), act)]), &[b(2, 1, 2)], false, &[b(4, 1, 4)]).unwrap_or_else(|e| panic!("{e}"));
        assert!(n6 == 1 && l6.iter().any(|l| l.starts_with("allow 4,1,4") && l.ends_with("a stale allow (FAIL)")), "a stale allow: {l6:?}");
        // (ironspec 19:46 (iii)) power out of a hook: on glass alone nothing; a powered rail beside it
        // fails; on stone, a powered rail beside the stone fails through the conductor
        let hook = "minecraft:tripwire_hook[attached=false,facing=west,powered=false]";
        let hk = redsim::block::Block::parse(hook).unwrap_or_else(|_| redsim::block::Block::Terrain(redsim::terrain::blocks::id_of(hook).expect("the hook")));
        let pr = p("minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]");
        let (o1, m1) = super::power_out(&mk(vec![(b(1, 1, 2), hk), (b(2, 1, 2), p("minecraft:glass"))]), &[b(1, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m1 == 0 && o1[0].contains("conductors beside it: stone at 1,0,2") && o1[0].ends_with("(PASS)"), "a hook on glass: {o1:?}");
        let (o2, m2) = super::power_out(&mk(vec![(b(1, 1, 2), hk), (b(2, 1, 2), p("minecraft:glass")), (b(1, 1, 3), pr)]), &[b(1, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m2 >= 1 && o2[0].contains("powered_rail[") && o2[0].contains("at 1,1,3 beside it") && o2[0].contains("(FAIL)"), "a powered rail beside: {o2:?}");
        let (o3, m3) = super::power_out(&mk(vec![(b(1, 1, 2), hk), (b(2, 1, 2), p("minecraft:stone")), (b(3, 1, 2), pr)]), &[b(1, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m3 >= 1 && o3[0].contains("at 3,1,2 through the conductor stone at 2,1,2"), "through the support: {o3:?}");
        // (G1 21:38 H7) a hook on stone with a redstone torch on that stone: the torch turns off when
        // the hook powers its support: it fails; a torch on glass beside the hook doesn't count
        let torch = p("minecraft:redstone_wall_torch[facing=south,lit=true]");
        let (o4, m4) = super::power_out(&mk(vec![(b(1, 1, 2), hk), (b(2, 1, 2), p("minecraft:stone")), (b(2, 1, 3), torch)]), &[b(1, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m4 == 1 && o4[0].contains("redstone_wall_torch[") && o4[0].contains("on the conductor stone at 2,1,2") && o4[0].contains("(FAIL)"), "a torch on the hook's support: {o4:?}");
        let (o5, m5) = super::power_out(&mk(vec![(b(1, 1, 2), hk), (b(2, 1, 2), p("minecraft:glass")), (b(1, 1, 1), p("minecraft:glass")), (b(1, 1, 0), p("minecraft:redstone_wall_torch[facing=north,lit=true]"))]), &[b(1, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m5 == 0 && o5[0].ends_with("(PASS)"), "a torch on glass elsewhere: {o5:?}");
        // a conductor cell (a latch's stone): its neighbours' consumers, dust marked strong-only, the
        // torch it supports
        let (o6, m6) = super::power_out(&mk(vec![(b(2, 1, 2), p("minecraft:stone")), (b(2, 2, 2), p("minecraft:redstone_torch[lit=true]")), (b(3, 1, 2), p("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=side]"))]), &[b(2, 1, 2)], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(m6 == 2 && o6[0].contains("a conductor") && o6[0].contains("redstone_torch[") && o6[0].contains("(dust: only if the block is STRONGLY powered)"), "a conductor: {o6:?}");
    }

    /// (G1 N28 / N39: the seated start rule on a slope) A seated rider near a slope's high end
    /// starts its path at the rail cell (the scan down from getY() + 1), never floor(y + 0.5)'s
    /// cell above it.
    #[test]
    fn seated_start_on_a_slope() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in -3..=4 {
            for z in -2..=2 {
                blocks.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        blocks.push((b(0, 1, 0), p("minecraft:rail[shape=ascending_east,waterlogged=false]")));
        blocks.push((b(1, 1, 0), p("minecraft:stone")));
        blocks.push((b(1, 2, 0), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(4, 1, 2), lo: b(-5, -1, -4), hi: b(6, 6, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let cart = super::along_rail(b(0, 1, 0), "minecraft:rail[shape=ascending_east,waterlogged=false]", 0.9);
        let l = super::path_probe(&spec, cart, b(-2, 1, 0)).unwrap();
        assert!(l.contains("0,1,0"), "the start is the slope's own cell: {l}");
    }

    /// (ironspec Nov 13 09:44, G1 09:51: the T exposure's control) A plain-rail T (rails on three
    /// sides) with a redstone torch beside it (its update footprint, Manhattan 2, holds the T) FAILS
    /// undeclared and passes declared in `switches:`; the same T with only a redstone block beside it
    /// is quiet (a constant source).
    #[test]
    fn t_exposure_fails_undeclared_operating() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in -3..=4 {
            for z in -3..=3 {
                blocks.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        for (x, z, s) in [(0, 0, "north_east"), (0, -1, "north_south"), (1, 0, "east_west"), (-1, 0, "east_west")] {
            blocks.push((b(x, 1, z), p(&format!("minecraft:rail[shape={s},waterlogged=false]"))));
        }
        let mut with_det = blocks.clone();
        with_det.push((b(0, 1, 1), p("minecraft:redstone_torch[lit=true]")));
        // (G1 09:58: the derivation's positive control: a lever under the torch's support powers it,
        // so the torch can change: OPERATING)
        with_det.push((b(0, -1, 1), p("minecraft:lever[face=ceiling,facing=north,powered=false]")));
        let mk = |bl: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| redsim::entity::mob::farm::FarmSpec { blocks: bl, entities: Vec::new(), output: b(4, 1, 3), lo: b(-5, -1, -5), hi: b(6, 5, 5), block_entities: Vec::new(), rock: false, nether: None, land: false };
        // (ironspec 10:47 (2): a T a lit torch or a redstone block powers re-settles north_west
        // (N, W, E all joinable, powered: the last-match order ends on north_west): the fixtures
        // that power it store that shape, so only the rule under test decides)
        let nw = |mut v: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| {
            v.retain(|(q, _)| *q != b(0, 1, 0));
            v.push((b(0, 1, 0), p("minecraft:rail[shape=north_west,waterlogged=false]")));
            v
        };
        let (l, f) = super::t_exposure(&mk(nw(with_det.clone())), &[], &[]).unwrap();
        assert!(f == 1 && l.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("OPERATING, its support 0,0,1 is powered from 0,-1,1")), "{l:?}");
        let (l2, f2) = super::t_exposure(&mk(nw(with_det.clone())), &[(b(0, 1, 0), "the switch lever".into())], &[]).unwrap();
        assert!(f2 == 0 && l2.iter().any(|x| x.contains("DECLARED T 0,1,0")), "{l2:?}");
        // (the exact rule's positive: the stored shape north_east while the lit torch powers the T,
        // so the game re-settles it north_west: a SHAPE MISMATCH fails even when declared)
        let (l2b, f2b) = super::t_exposure(&mk(with_det), &[(b(0, 1, 0), "the switch lever".into())], &[]).unwrap();
        assert!(f2b == 1 && l2b.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("SHAPE MISMATCH, stored north_east")), "{l2b:?}");
        // (the same torch on the same conductor with nothing powering it: CONSTANT, quiet)
        let mut with_t = blocks.clone();
        with_t.push((b(0, 1, 1), p("minecraft:redstone_torch[lit=true]")));
        let (l5, f5) = super::t_exposure(&mk(nw(with_t)), &[], &[]).unwrap();
        assert!(f5 == 0 && l5.iter().any(|x| x.contains("quiet T 0,1,0") && x.contains("CONSTANT, no source's power reaches its support 0,0,1")), "{l5:?}");
        let mut with_rb = blocks.clone();
        with_rb.push((b(0, 1, 1), p("minecraft:redstone_block")));
        let (l3, f3) = super::t_exposure(&mk(nw(with_rb.clone())), &[], &[]).unwrap();
        assert!(f3 == 0 && l3.iter().any(|x| x.contains("quiet T 0,1,0") && x.contains("1 constant") && x.contains("no piston within 13")), "{l3:?}");
        // (a piston that could move the redstone block: OPERATING)
        with_rb.push((b(3, 1, 3), p("minecraft:piston[extended=false,facing=west]")));
        let (l6, f6) = super::t_exposure(&mk(nw(with_rb)), &[], &[]).unwrap();
        assert!(f6 == 1 && l6.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("a piston at 3,1,3 within 13 could move it")), "{l6:?}");
        // (G1 10:38 (b): a torch on a support a constant redstone block powers is permanently off:
        // a DEAD TORCH, a design error by name)
        let mut dead = blocks.clone();
        dead.push((b(3, 1, -2), p("minecraft:redstone_torch[lit=false]")));
        dead.retain(|(q, _)| *q != b(3, -1, -2));
        dead.push((b(3, -1, -2), p("minecraft:redstone_block")));
        let (l7, f7) = super::t_exposure(&mk(dead), &[], &[]).unwrap();
        assert!(f7 == 1 && l7.iter().any(|x| x.contains("DEAD TORCH 3,1,-2") && x.contains("the redstone block at 3,-1,-2")), "{l7:?}");
        // (ironspec 11:27 (a), the fixpoint: a torch whose support only a constant LIT torch feeds
        // (a torch on glass, two below, strongly powering the stone above it) is dead too)
        let mut dead2 = blocks.clone();
        dead2.retain(|(q, _)| *q != b(3, 0, -3));
        dead2.push((b(3, 0, -3), p("minecraft:glass")));
        dead2.push((b(3, 1, -3), p("minecraft:redstone_torch[lit=true]")));
        dead2.push((b(3, 2, -3), p("minecraft:stone")));
        dead2.push((b(3, 3, -3), p("minecraft:redstone_torch[lit=false]")));
        let (l7b, f7b) = super::t_exposure(&mk(dead2), &[], &[]).unwrap();
        assert!(f7b == 1 && l7b.iter().any(|x| x.contains("DEAD TORCH 3,3,-3") && x.contains("constant lit torch at 3,1,-3")), "{l7b:?}");
        // (tick-exact 14:04 in the game, ironspec 14:04 (2)(c): a sloped detector one down and one
        // over from a T, ascending into it (D under J's arm, as the lanes' D (x,24,8) under J
        // (x,25,9)), reaches it through D.above()'s neighbours: an undeclared T there FAILS, a
        // declared one passes (10:40's "quiet" reading withdrawn))
        let mut det = blocks.clone();
        det.retain(|(q, _)| *q != b(0, 0, -1) && *q != b(0, 1, -1));
        det.push((b(0, -1, -1), p("minecraft:stone")));
        det.push((b(0, 0, -1), p("minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]")));
        let (l8, f8) = super::t_exposure(&mk(det.clone()), &[], &[]).unwrap();
        // (ironspec 15:05 (c): a tripwire hook whose footprint holds a T is named; with none, "none")
        let mut hk = blocks.clone();
        hk.retain(|(q, _)| *q != b(0, 1, 2));
        hk.push((b(0, 1, 2), p("minecraft:stone")));
        hk.push((b(0, 1, 1), p("minecraft:tripwire_hook[attached=false,facing=north,powered=false]")));
        let (lh, _) = super::t_exposure(&mk(hk), &[(b(0, 1, 0), "the T's own line".to_string())], &[]).unwrap();
        assert!(lh.iter().any(|x| x.starts_with("  TRIPWIRE HOOKS in reach of a T (ironspec 15:05 (c)") && x.contains("0,1,1 (") && x.contains("reaches T(s) 0,1,0")), "{lh:?}");
        let (lnh, _) = super::t_exposure(&mk(blocks.clone()), &[], &[]).unwrap();
        assert!(lnh.iter().any(|x| x.starts_with("  TRIPWIRE HOOKS in reach of a T: none")), "{lnh:?}");
        assert!(f8 == 1 && l8.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("0,0,-1 detector rail")), "{l8:?}");
        let (l8d, f8d) = super::t_exposure(&mk(det), &[(b(0, 1, 0), "J: its lever".to_string())], &[]).unwrap();
        assert!(f8d == 0 && l8d.iter().any(|x| x.contains("DECLARED T 0,1,0")), "{l8d:?}");
        // (ironspec 09:53 (B): a standing torch on glass never changes: constant, quiet)
        let mut with_tg = blocks;
        with_tg.retain(|(q, _)| *q != b(0, 0, 1));
        with_tg.push((b(0, 0, 1), p("minecraft:glass")));
        with_tg.push((b(0, 1, 1), p("minecraft:redstone_torch[lit=true]")));
        let (l4, f4) = super::t_exposure(&mk(nw(with_tg)), &[], &[]).unwrap();
        assert!(f4 == 0 && l4.iter().any(|x| x.contains("quiet T 0,1,0") && x.contains("1 constant")), "{l4:?}");
    }

    /// (G1 Nov 13 10:46 / 11:19 / 12:09, ironspec 10:47 (2) / 10:54) The HELD T's integrity entries
    /// and the MUTUAL HOLD rule, as their own controls: a T (0,1,0) whose W arm (-1,1,0) is full (a
    /// north_south rail between two that point back) is HELD under an operating torch, and that arm
    /// is itself a T held by (0,1,0): a two-T cycle. With no entries: 3 fails (two missing entries,
    /// the unbroken cycle); one stale entry: 2 (stale, the cycle); both entries matching: 1 (the
    /// cycle still unbroken by an operating torch); the torch made CONSTANT (on glass): broken, 0;
    /// the full arm freed of a connection: SHAPE MISMATCH.
    #[test]
    fn held_integrity_and_mutual_hold_controls() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut held = Vec::new();
        for x in -3..=4 {
            for z in -3..=3 {
                held.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        for (x, z, s) in [(0, 0, "north_east"), (0, -1, "north_south"), (1, 0, "east_west"), (-1, -1, "north_south"), (-1, 0, "north_south"), (-1, 1, "north_south")] {
            held.push((b(x, 1, z), p(&format!("minecraft:rail[shape={s},waterlogged=false]"))));
        }
        // (the operating torch: on stone, the stone powered by a ceiling lever)
        held.push((b(0, 1, 1), p("minecraft:redstone_torch[lit=true]")));
        held.push((b(0, -1, 1), p("minecraft:lever[face=ceiling,facing=north,powered=false]")));
        let mk = |bl: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| redsim::entity::mob::farm::FarmSpec { blocks: bl, entities: Vec::new(), output: b(4, 1, 3), lo: b(-5, -1, -5), hi: b(6, 5, 5), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (l9, f9) = super::t_exposure(&mk(held.clone()), &[], &[]).unwrap();
        assert!(f9 == 3 && l9.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("the arm(s) -1,1,0 can't join") && x.contains("-1,1,0 has no `integrity:` entry")) && l9.iter().any(|x| x.contains("FAIL MUTUAL HOLD")), "{l9:?}");
        let entries = |a: &str| vec![(b(-1, 1, 0), a.to_string()), (b(0, 1, 0), "connections 0,1,-1+1,1,0; holds -1,1,0=north_south (a note after the shape is allowed)".to_string())];
        let (l9b, f9b) = super::t_exposure(&mk(held.clone()), &[], &entries("connections -1,1,-1+-1,1,1; holds 0,1,0=north_east")).unwrap();
        assert!(f9b == 1 && l9b.iter().any(|x| x.contains("HELD T 0,1,0") && x.contains("declared and matching for -1,1,0")) && l9b.iter().any(|x| x.contains("FAIL MUTUAL HOLD")), "{l9b:?}");
        let (l9c, f9c) = super::t_exposure(&mk(held.clone()), &[], &entries("connections -1,1,-1+-2,1,0; holds 0,1,0=north_east")).unwrap();
        assert!(f9c == 2 && l9c.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("entry stale")), "{l9c:?}");
        let mut held_c = held.clone();
        held_c.retain(|(q, _)| *q != b(0, -1, 1) && *q != b(0, 0, 1));
        held_c.push((b(0, 0, 1), p("minecraft:glass")));
        let (l9d, f9d) = super::t_exposure(&mk(held_c), &[], &entries("connections -1,1,-1+-1,1,1; holds 0,1,0=north_east")).unwrap();
        assert!(f9d == 0 && l9d.iter().any(|x| x.contains("MUTUAL HOLD") && x.contains("broken by a CONSTANT source")), "{l9d:?}");
        let mut freed = held;
        freed.retain(|(q, _)| *q != b(-1, 1, 1));
        let (l10, f10) = super::t_exposure(&mk(freed), &[], &[]).unwrap();
        assert!(f10 == 1 && l10.iter().any(|x| x.contains("FAIL T 0,1,0") && x.contains("SHAPE MISMATCH, stored north_east")), "{l10:?}");
    }

    /// (G1 Nov 13 12:52 (e)) sensor-at's riders: a boat holding a pillager and a zombie is judged
    /// at its first seat and its second rider FAILS as NOT JUDGED (the seat unmodelled); a boat
    /// holding a skeleton is printed as ignored (not a sensor hostile), never dropped silently;
    /// the samples count the judged hostiles only (points × 1).
    #[test]
    fn sensor_at_names_every_rider() {
        let b = |x, y, z| redsim::pos::BlockPos::new(x, y, z);
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in -4..=4 {
            for z in -4..=4 {
                blocks.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        let boat = |pass: &str| Some(format!("{{Passengers:[{pass}]}}"));
        let spec = redsim::entity::mob::farm::FarmSpec {
            blocks,
            entities: vec![
                ("minecraft:oak_boat".to_string(), [2.5, 1.0, 0.5], boat("{id:\"minecraft:pillager\",HandItems:[{id:\"minecraft:crossbow\",count:1}]},{id:\"minecraft:zombie\"}")),
                ("minecraft:oak_boat".to_string(), [-2.5, 1.0, 0.5], boat("{id:\"minecraft:skeleton\"}")),
            ],
            output: b(4, 1, 4),
            lo: b(-5, -1, -5),
            hi: b(5, 4, 5),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
            land: false,
        };
        let points = [redsim::entity::Vec3::new(0.5, 1.0, 0.5)];
        let (l, samples, fails) = super::sensor_at(&spec, &points, &[], &[], &[], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(samples == 1, "the judged pillager only: {l:?}");
        assert!(fails >= 1 && l.iter().any(|x| x.contains("NOT JUDGED the zombie in the oak_boat") && x.contains("seat 1")), "{l:?}");
        assert!(l.iter().any(|x| x.contains("the skeleton in the oak_boat") && x.contains("ignored")), "{l:?}");
        assert!(!l.iter().any(|x| x.contains("crossbow")), "an item's id names no rider: {l:?}");
    }

    /// (onechunk / ironspec Nov 13 13:54) `update-only:` entries against their sources' own update
    /// footprints: a detector rail far from the T "reaches no T" (AGREES) and, read the old way as
    /// updating the T, is a stale reading (FLAG); a dust beside the T reaches it (AGREES with the T
    /// in `switches:`, FLAG without); an entry on a cell holding no source FLAGS.
    #[test]
    fn update_only_entries_match_their_footprints() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in -4..=5 {
            for z in -4..=5 {
                blocks.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        for (x, z, s) in [(0, 0, "north_east"), (0, -1, "north_south"), (1, 0, "east_west"), (-1, 0, "east_west")] {
            blocks.push((b(x, 1, z), p(&format!("minecraft:rail[shape={s},waterlogged=false]"))));
        }
        blocks.push((b(3, 1, 3), p("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]")));
        blocks.push((b(0, 1, 1), p("minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=none]")));
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(5, 1, 5), lo: b(-5, -1, -5), hi: b(6, 4, 6), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let sw = vec![(b(0, 1, 0), "the T's own line".to_string())];
        let (l1, f1) = super::update_only_check(&spec, &[(b(3, 1, 3), "the detector: its change reaches no T".to_string())], &sw);
        assert!(f1 == 0 && l1[0].contains("reaches no T: AGREES"), "{l1:?}");
        let (l2, f2) = super::update_only_check(&spec, &[(b(3, 1, 3), "the detector: its changes update the T (0,1,0)".to_string())], &sw);
        assert!(f2 == 1 && l2[0].contains("FLAG: the entry says it updates a T, but its footprint reaches none"), "{l2:?}");
        let (l3, f3) = super::update_only_check(&spec, &[(b(0, 1, 1), "the dust re-settles the T to its power's shape".to_string())], &sw);
        assert!(f3 == 0 && l3[0].contains("reaches T(s) 0,1,0") && l3[0].contains("AGREES"), "{l3:?}");
        let (l4, f4) = super::update_only_check(&spec, &[(b(0, 1, 1), "the dust re-settles the T to its power's shape".to_string())], &[]);
        assert!(f4 == 1 && l4[0].contains("FLAG: it reaches the undeclared T(s) 0,1,0"), "{l4:?}");
        let (l5, f5) = super::update_only_check(&spec, &[(b(5, 0, 5), "a stone".to_string())], &sw);
        assert!(f5 == 1 && l5[0].contains("no signal source at this cell"), "{l5:?}");
        // (tick-exact Nov 13 14:04, the game row a/1072581002473618: an ASCENDING detector's change
        // also updates the 6 of the block above it: a T beside D.above() is reached; the same
        // detector laid flat reaches nothing there)
        let mut sl = Vec::new();
        for x in -3..=3 {
            for z in -3..=4 {
                sl.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        for c in [b(0, 1, 1), b(1, 1, 1), b(-1, 1, 1)] {
            sl.push((c, p("minecraft:stone")));
        }
        sl.push((b(1, 2, 1), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        sl.push((b(-1, 2, 1), p("minecraft:rail[shape=east_west,waterlogged=false]")));
        sl.push((b(0, 2, 1), p("minecraft:rail[shape=north_east,waterlogged=false]")));
        let mk = |blocks: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(3, 1, 4), lo: b(-4, -1, -4), hi: b(4, 5, 5), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let mut asc = sl.clone();
        asc.push((b(0, 1, 0), p("minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]")));
        let swt = vec![(b(0, 2, 1), "the T's own line".to_string())];
        let (l6, f6) = super::update_only_check(&mk(asc), &[(b(0, 1, 0), "the detector: its change reaches no T".to_string())], &swt);
        assert!(f6 == 1 && l6[0].contains("reaches T(s) 0,2,1") && l6[0].contains("says it reaches no T, but its footprint holds one"), "{l6:?}");
        let mut flat = sl;
        flat.push((b(0, 1, 0), p("minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]")));
        let (l7, f7) = super::update_only_check(&mk(flat), &[(b(0, 1, 0), "the detector: its change reaches no T".to_string())], &swt);
        assert!(f7 == 0 && l7[0].contains("reaches no T: AGREES"), "{l7:?}");
    }

    /// (G1 Nov 13 18:37 B1 / 18:55: the expected directions in the name) A pair of cell-centred eyes
    /// on a diagonal (|dx| = |dz|) crossing a cell corner beside one solid neighbour (stone at
    /// (1,1,0)): the two directions' block walks break the tie on opposite sides, so the rider's ray
    /// (0.5,1.5,0.5) → (2.5,1.5,2.5) is CLEAR and the hostile's ray back is BLOCKED, and the tag
    /// names the disagreement; a pair with nothing between them reads "both CLEAR".
    #[test]
    fn b1_corner_pair_rider_ray_clear_hostile_ray_blocked() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let spec = redsim::entity::mob::farm::FarmSpec { blocks: vec![(b(1, 1, 0), p("minecraft:stone")), (b(0, 0, 0), p("minecraft:stone"))], entities: Vec::new(), output: b(0, 0, 0), lo: b(-1, -1, -1), hi: b(4, 3, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let w = super::settled_world_void(&spec).unwrap_or_else(|e| panic!("{e}"));
        let v3 = redsim::entity::Vec3::new;
        let (a, c) = (v3(0.5, 1.5, 0.5), v3(2.5, 1.5, 2.5));
        let pair = super::ray_pair(&w, a, c);
        assert!(pair == (true, false) && super::ray_tag(pair).contains("DISAGREE"), "the corner pair: {pair:?}");
        let calm = super::ray_pair(&w, a, v3(0.5, 1.5, 3.5));
        assert!(calm == (true, true) && super::ray_tag(calm) == "both CLEAR", "the open pair: {calm:?}");
    }

    /// (ironspec Nov 13 19:15, B1: the expected directions in the name) The corner pair MIRRORED as a
    /// hide point: a seated pillager's eye at (0.5,2.245,0.5) (its post (0,1,0)) and a hide point whose
    /// eye is at (2.5,2.245,2.5), stone at (1,2,0): the rider's ray to the hostile is BLOCKED and the
    /// hostile's ray to the point CLEAR. The point enters the search (the hostile's ray is the one a
    /// hide point depends on), is printed seen before the glass with the disagreement named, and is
    /// cut by the glass found.
    #[test]
    fn b1_mirrored_corner_hide_point_hostile_ray_clear_rider_ray_blocked() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let spec = redsim::entity::mob::farm::FarmSpec { blocks: vec![(b(1, 2, 0), p("minecraft:stone"))], entities: Vec::new(), output: b(3, -1, 3), lo: b(-1, -1, -1), hi: b(4, 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let eye_y = 1.0 + super::PILLAGER_SEAT + super::eye_of(redsim::entity::mob::types::MobType::Pillager);
        let point = redsim::entity::Vec3::new(2.5, eye_y - super::eye_of(redsim::entity::mob::types::MobType::Villager), 2.5);
        let l = super::occlude(&spec, b(0, 1, 0), true, &[point], &[], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(l[0].contains(": 1 of 1 hide positions seen now"), "the mirrored point enters the search: {l:?}");
        assert!(l.iter().any(|x| x.trim_start().starts_with("hide point 2.50,") && x.contains("before the hostile sees the rider, the rider's ray blocked (DISAGREE)")), "seen before the glass: {l:?}");
        assert!(l.iter().any(|x| x.contains("pairs whose two directions disagree: ") && x.contains("a charge is possible")), "{l:?}");
        assert!(l.iter().any(|x| x.trim_start().starts_with("glass ") && x.ends_with("(PASS)")), "cut by the glass: {l:?}");
        // (G1 19:23: and through `occlude` the same point FAILS when no glass may go on its ray (every
        // cell excluded): it entered the search, so it is reported, not skipped)
        let lf = super::occlude(&spec, b(0, 1, 0), true, &[point], &[([-1, -1, -1], [4, 4, 4])], &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(lf[0].contains(": 1 of 1 hide positions seen now") && super::occlude_failed(&lf) && lf.iter().any(|x| x.trim_start().starts_with("NONE:")), "no glass possible: FAIL: {lf:?}");
    }

    /// (G1 Nov 13 19:23, B4: the expected count in the name) A rider on an ascending rail calls from
    /// two cells: its rail's cell and, near the slope's top, the cell above; a villager the design
    /// holds off the rails calls from its own cell.
    #[test]
    fn b4_slope_top_rider_calls_from_two_cells_and_the_design_villager_from_its_own() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in 0..=4 {
            for z in 0..=4 {
                blocks.push((b(x, 0, z), p("minecraft:stone")));
            }
        }
        blocks.push((b(3, 1, 2), p("minecraft:stone")));
        blocks.push((b(2, 1, 2), p("minecraft:rail[shape=ascending_east,waterlogged=false]")));
        let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(2, -1, 2), lo: b(0, -1, 0), hi: b(4, 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        let (gl, _, _) = super::route_golem_floors(&spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
        assert!(gl[0].contains("(2 caller cell(s) on 1 rail cell(s), 0 golem guard(s) excluded, 0 villager cell(s) of the design added"), "the slope's two cells: {gl:?}");
        let with_villager = redsim::entity::mob::farm::FarmSpec { entities: vec![("minecraft:villager".to_string(), [0.5, 1.0, 0.5], None)], ..spec.clone() };
        let (vl, _, _) = super::route_golem_floors(&with_villager, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
        assert!(vl[0].contains("(3 caller cell(s) on 1 rail cell(s), 0 golem guard(s) excluded, 1 villager cell(s) of the design added"), "the design's villager: {vl:?}");
        // (G1 20:02, E7; ironspec 20:12: a line redsim refuses errs by name at each callsite: the
        // route's golem floors, the void exposures and the exterior spawn static, never a silently
        // missing villager)
        let refused = redsim::entity::mob::farm::FarmSpec { entities: vec![("minecraft:villager".to_string(), [0.5, 1.0, 0.5], Some("{Bogus:1b}".to_string()))], ..spec };
        let prefix = "NOT JUDGED: summoning the design's minecraft:villager at 0.50,1.00,0.50: ";
        let e1 = super::route_golem_floors(&refused, "minecraft:plains").err().unwrap_or_default();
        assert!(e1.starts_with("the route's golem floors: ") && e1.contains(prefix), "the golem floors: {e1}");
        let e2 = super::void_exposures(&refused, "minecraft:plains").err().unwrap_or_default();
        assert!(e2.starts_with("the void exposures: ") && e2.contains(prefix), "the void exposures: {e2}");
        let e3 = super::spawn_static(&refused, "minecraft:plains", false).err().unwrap_or_default();
        assert!(e3.starts_with("the exterior spawn static: ") && e3.contains(prefix), "the exterior spawn static: {e3}");
        // (G1 21:38 H12: the exposures' two CLI sites read their summoners through `design_summoners`)
        let w4 = super::settled_world_void(&refused).unwrap_or_else(|e| panic!("{e}"));
        let e4 = super::design_summoners(&w4, &refused, "the exposures (--against)").err().unwrap_or_default();
        assert!(e4.starts_with("the exposures (--against): ") && e4.contains(prefix), "the exposures: {e4}");
    }

    /// (ironspec Nov 13 08:18 (e): the exterior static's control) A stone platform floating in the
    /// void spawns monsters on its top at night (cells listed, the bar fails); a glass platform
    /// spawns none.
    #[test]
    fn exterior_spawns_bite() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        // (ironspec 17:26, standing: the route's golem floors; G1 18:37 B3 / B5 / B6 / B13) a rail on a
        // stone platform: its rider's own search yields the platform's 24 other cells, all off any
        // pad, each attributed to that caller; the single-change twin (the rail's support a closed
        // top-half trapdoor: the rail a golem guard, no caller) and a rail on a lone pillar (no floor
        // in reach) are NOT JUDGED by name
        {
            let mut blocks = Vec::new();
            for x in 0..=4 {
                for z in 0..=4 {
                    blocks.push((b(x, 0, z), p("minecraft:stone")));
                }
            }
            blocks.push((b(2, 1, 2), p("minecraft:rail[shape=north_south,waterlogged=false]")));
            // (the output chest under the platform, out of every floor's way)
            let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(2, -1, 2), lo: b(0, -1, 0), hi: b(4, 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
            let (gl, off, cells) = super::route_golem_floors(&spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
            assert!(off == 24 && cells.len() == 24 && gl[0].contains("(1 caller cell(s) on 1 rail cell(s), 0 golem guard(s) excluded") && gl.iter().any(|l| l.contains("yielded by 1 caller cell(s)' own search: 2,1,2")), "{gl:?}");
            let mut twin = spec.blocks.clone();
            twin.retain(|(q, _)| *q != b(2, 0, 2));
            twin.push((b(2, 0, 2), p("minecraft:oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]")));
            let tspec = redsim::entity::mob::farm::FarmSpec { blocks: twin, ..spec.clone() };
            let tw = super::settled_world_void(&tspec).unwrap_or_else(|e| panic!("{e}"));
            let r = super::route_golem_floors(&tspec, "minecraft:plains");
            assert!(r.as_ref().is_err_and(|e| e.contains("NOT JUDGED: no caller (1 rail cell(s), 1 golem guard(s) excluded)")), "a lone guard ({} on {}): {r:?}", tw.get(b(2, 1, 2)), tw.get(b(2, 0, 2)));
            let pillar = redsim::entity::mob::farm::FarmSpec {
                blocks: vec![(b(2, 0, 2), p("minecraft:stone")), (b(2, 1, 2), p("minecraft:rail[shape=north_south,waterlogged=false]"))],
                ..spec
            };
            let r2 = super::route_golem_floors(&pillar, "minecraft:plains");
            assert!(r2.as_ref().is_err_and(|e| e.contains("NOT JUDGED: the 1 caller cell(s)' golem searches reach no floor")), "a pillar: {r2:?}");
        }
        // (G1 18:37, B2 / B7: the pad test controlled) a framed, lit portal the settle keeps (its portal
        // blocks asserted after the settle); a golem one cell beside the portal meets it, one further
        // doesn't (its box spans c-1..c+1); with the portal's lowest block 2 above the golem's cell it
        // meets it, 3 above it doesn't; and a normal-operation spec whose every floor is on the pad:
        // cells > 0 and none off it
        {
            let obs = "minecraft:obsidian";
            let portal = redsim::block::Block::Terrain(redsim::terrain::blocks::id_of("minecraft:nether_portal[axis=x]").expect("the portal state"));
            let mut blocks = Vec::new();
            for x in 0..=3 {
                for y in 1..=5 {
                    let inside = (1..=2).contains(&x) && (2..=4).contains(&y);
                    blocks.push((b(x, y, 5), if inside { portal } else { p(obs) }));
                }
                blocks.push((b(x, 6, 5), p("minecraft:glass")));
            }
            blocks.push((b(1, 1, 6), p("minecraft:stone")));
            blocks.push((b(2, 1, 6), p("minecraft:stone")));
            blocks.push((b(1, 1, 8), p("minecraft:stone")));
            blocks.push((b(1, 2, 8), p("minecraft:rail[shape=north_south,waterlogged=false]")));
            let spec = redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(1, 1, 8), lo: b(-1, 0, 3), hi: b(4, 7, 9), block_entities: Vec::new(), rock: false, nether: None, land: false };
            let w = super::settled_world_void(&spec).unwrap_or_else(|e| panic!("{e}"));
            for c in [b(1, 2, 5), b(2, 2, 5), b(1, 4, 5), b(2, 4, 5)] {
                assert!(w.get(c).to_string().contains("nether_portal"), "the settle keeps the portal at {c:?}: {}", w.get(c));
            }
            assert!(super::golem_box_meets_portal(&w, b(1, 2, 6)) && !super::golem_box_meets_portal(&w, b(1, 2, 7)), "the width edge");
            assert!(super::golem_box_meets_portal(&w, b(1, 0, 6)) && !super::golem_box_meets_portal(&w, b(1, -1, 6)), "the height edge");
            let (gl, off, cells) = super::route_golem_floors(&spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
            assert!(off == 0 && !cells.is_empty() && cells.contains(&b(1, 2, 6)) && gl[0].contains(", 0 OFF THE PAD (bar 0)"), "every floor on the pad: {gl:?}");
        }
        let plat = |s: &str| {
            let mut blocks = Vec::new();
            for x in 0..=4 {
                for z in 0..=4 {
                    blocks.push((b(x, 0, z), p(s)));
                }
            }
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(2, 0, 2), lo: b(0, 0, 0), hi: b(4, 3, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let (l, n) = super::exterior_spawns(&plat("minecraft:stone"), "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
        assert!(n > 0 && l.iter().any(|x| x.contains("SPAWNABLE") && x.contains("zombie")), "{l:?}");
        let (l2, n2) = super::exterior_spawns(&plat("minecraft:glass"), "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
        assert!(n2 == 0 && l2[0].contains("PASS"), "{l2:?}");
        // (G1 11:23 (a): the light-aware read: a glowstone on the stone platform lights every top
        // cell (block light > 0): none spawnable with --lit, all of them in the dark bound)
        let mut glow = plat("minecraft:stone");
        glow.blocks.push((b(2, 1, 2), p("minecraft:glowstone")));
        let (l3, _, out3) = super::spawn_static(&glow, "minecraft:plains", true).unwrap_or_else(|e| panic!("{e}"));
        assert!(out3 == 0 && l3[0].contains("LIGHT-AWARE") && l3[0].contains("are lit (block light > 0)"), "{l3:?}");
        let (l4, _, out4) = super::spawn_static(&glow, "minecraft:plains", false).unwrap_or_else(|e| panic!("{e}"));
        assert!(out4 > 0 && l4[0].contains("ALL DARK"), "{l4:?}");
        // (a terrain-modeled emitter's own luminance counts (crying obsidian's 10, as the nether
        // portal's 11): the sun table alone would leave the platform dark)
        let mut cry = plat("minecraft:stone");
        cry.blocks.push((b(2, 1, 2), p("minecraft:crying_obsidian")));
        let (l5, _, out5) = super::spawn_static(&cry, "minecraft:plains", true).unwrap_or_else(|e| panic!("{e}"));
        assert!(out5 == 0, "a terrain emitter lights the platform: {l5:?}");
        // (#prevent_mob_spawning_inside: a rail cell is never a spawn cell)
        let mut rail = plat("minecraft:stone");
        rail.blocks.push((b(2, 1, 2), p("minecraft:rail[shape=north_south,waterlogged=false]")));
        let (l6, _, _) = super::spawn_static(&rail, "minecraft:plains", false).unwrap_or_else(|e| panic!("{e}"));
        assert!(!l6.iter().any(|x| x.contains("SPAWNABLE OUTSIDE 2,1,2 ") || x.contains("SPAWNABLE INSIDE 2,1,2 ")), "{l6:?}");
    }

    /// (cc767b78, G1 18:08 (b): the light series' fold on train 11's caller) A light read that fails during
    /// `void_exposures` refuses it with the read's message, never an empty list: the dark stone room with a brewing
    /// stand on its floor (a block whose light behaviour redsim doesn't model, so the spawn cells' block light
    /// can't be read) is refused; the same room without it reads.
    #[test]
    fn void_exposures_refuses_through_a_failed_light_read() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let mut blocks = Vec::new();
        for x in 0..=4 {
            for z in 0..=4 {
                for y in 0..=4 {
                    let edge = x == 0 || x == 4 || z == 0 || z == 4 || y == 4;
                    let s = if y == 0 || edge { "minecraft:stone" } else { "minecraft:air" };
                    blocks.push((b(x, y, z), p(s)));
                }
            }
        }
        let spec = |blocks: Vec<(redsim::pos::BlockPos, redsim::block::Block)>| redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(2, 1, 2), lo: b(0, 0, 0), hi: b(4, 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false };
        assert!(super::void_exposures(&spec(blocks.clone()), "minecraft:plains").is_ok(), "the plain room reads");
        blocks.retain(|(q, _)| *q != b(1, 1, 1));
        blocks.push((b(1, 1, 1), p("minecraft:brewing_stand")));
        match super::void_exposures(&spec(blocks), "minecraft:plains") {
            Ok((_, _, ex)) => panic!("a failed light read must refuse void_exposures, not give {} exposure(s)", ex.len()),
            Err(e) => assert!(e.contains("light"), "the refusal names the light read: {e}"),
        }
    }

    /// (9fc3155f's correction 10:35, G1's no-vacuous-pass rule) The floating site's spawn check
    /// bites: a closed dark room in the void with a stone floor gives C1 ("can spawn inside the
    /// farm", a zombie among them, the site biome set); the same room floored with glass gives none.
    #[test]
    fn void_exposures_c1_bites() {
        let p = |s: &str| redsim::block::Block::parse(s).unwrap();
        let room = |floor: &str| {
            let mut blocks = Vec::new();
            for x in 0..=4 {
                for z in 0..=4 {
                    for y in 0..=4 {
                        let edge = x == 0 || x == 4 || z == 0 || z == 4 || y == 4;
                        let s = if y == 0 { floor } else if edge { "minecraft:stone" } else { "minecraft:air" };
                        blocks.push((b(x, y, z), p(s)));
                    }
                }
            }
            redsim::entity::mob::farm::FarmSpec { blocks, entities: Vec::new(), output: b(2, 1, 2), lo: b(0, 0, 0), hi: b(4, 4, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
        };
        let c1 = |spec: &redsim::entity::mob::farm::FarmSpec| -> Vec<String> {
            let (_, _, ex) = super::void_exposures(spec, "minecraft:plains").unwrap_or_else(|e| panic!("{e}"));
            ex.iter().map(|e| format!("{e:?}")).filter(|e| e.contains("C1 natural spawn inside")).collect()
        };
        let stone = c1(&room("minecraft:stone"));
        assert!(stone.iter().any(|e| e.contains("\"zombie\"")), "a dark stone floor in the void: C1 must fire: {stone:?}");
        let glass = c1(&room("minecraft:glass"));
        assert!(glass.is_empty(), "a glass floor: no C1: {glass:?}");
        // (an unknown biome is refused, never a silent empty list)
        assert!(super::void_exposures(&room("minecraft:stone"), "minecraft:no_such_biome").is_err());
        // (G1 12:32 (a): the LIT model's positive control: the dark stone room's floor spawns under
        // --lit too (INSIDE, failing); a torch on its floor clears it)
        let (ld, inside_d, _) = super::spawn_static(&room("minecraft:stone"), "minecraft:plains", true).unwrap_or_else(|e| panic!("{e}"));
        assert!(inside_d > 0 && ld.iter().any(|x| x.contains("SPAWNABLE INSIDE")), "{ld:?}");
        let mut lit_room = room("minecraft:stone");
        lit_room.blocks.retain(|(q, _)| *q != b(1, 1, 1));
        lit_room.blocks.push((b(1, 1, 1), redsim::block::Block::parse("minecraft:torch").unwrap()));
        let (lt, inside_t, _) = super::spawn_static(&lit_room, "minecraft:plains", true).unwrap_or_else(|e| panic!("{e}"));
        // (a plain torch: its 14 comes from the game's block table, the sun table has none)
        assert!(inside_t == 0, "a torch on the floor lights the room: {lt:?}");
    }

    /// (ironspec Nov 13 06:18 platform (2)) flee-check's own controls bite on this build.
    #[test]
    fn self_controls_bite() {
        let l = super::self_controls().unwrap_or_else(|e| panic!("{e}"));
        assert!(l[0].ends_with("every one bites (PASS)"), "{l:?}");
    }

    /// (ironspec Nov 13 06:36 real (3), the HOME walk's known positive) A villager boxed in on a
    /// pillar top can't reach the bed head: UNREACHED, the bar FAILS; on the open floor it reaches.
    #[test]
    fn home_walk_boxed_cell_fails() {
        let mut s = room(true);
        s.blocks.push((b(4, 1, -2), redsim::block::Block::parse("minecraft:red_bed[facing=south,occupied=false,part=foot]").unwrap()));
        s.blocks.push((b(4, 1, -1), redsim::block::Block::parse("minecraft:red_bed[facing=south,occupied=false,part=head]").unwrap()));
        s.entities.clear();
        let boxed = redsim::entity::Vec3::new(-1.5, 2.0, 0.5);
        let open = redsim::entity::Vec3::new(2.5, 1.0, 2.5);
        let (l, n) = super::home_walk(&s, b(4, 1, -1), &[boxed, open]).unwrap();
        assert!(n == 1 && l.iter().any(|x| x.contains("UNREACHED -2,2,0")) && l.iter().any(|x| x.contains("REACHED 2,1,2")), "{l:?}");
        let (l0, n0) = super::home_walk(&s, b(4, 1, -1), &[]).unwrap();
        assert!(n0 == usize::MAX && l0[0].ends_with("(NOT JUDGED: 0 samples)"), "{l0:?}");
    }
}
