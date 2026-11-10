//! The carousel's station + portal pad (vjeux 2026-11-10 06:25-06:33Z: one shared bed, compact,
//! seven batches of three, each part optimized on its own before the assembly; brief
//! designs/briefs/farm-iron-carousel.md, "The parts", 3; owner farmopt). A batch parked at its
//! stand cells (the release + hold part's stacked tracks, [`crate::hold`]) panics at the hold
//! zombies beside it and summons at its slot (gameTime % 100 == 0). Every golem lands on the pad:
//! trench rows one cell wide on both sides of portal planes, each trench cell touching a portal
//! cell (the golem's box, 1.4 wide, reaches 0.2 into its neighbours, the diagonals included: the
//! end cells reach the first and last portal cells past the frame's pillars), so it is in the
//! Nether at its second tick. Every other cell of every summoner's search is no floor.
//!
//! Geometry (x along the lanes' row, z toward the hold; [`PadOpts`]):
//! - each plane (`planes`, z) a cornerless obsidian frame: its bottom row at y `by` (x0+1..x1-1),
//!   its pillars at x0 and x1 (y by+1..by+h), its top row at y by+h+1, portal blocks inside (axis
//!   x, h tall);
//! - a trench row on both sides of each plane, x0..x1, its floor (cobblestone) at y by+h-1: a
//!   golem's feet at the portal's top row;
//! - over each plane, a tripwire line at y by+h+2 lying on the top row: its strings cap the top
//!   row (a floor needs air above it) and its hooks stand over the pillar tops (a hook is a signal
//!   source: no golem spawns under it). The west hooks hang on glass (no floor), the east ones on a
//!   cobblestone block the hook powers when a golem's box crosses a string: a dust on it and along
//!   x1+1 joins the planes' lines into the counter's pulse.
//!
//! [`check`] is the static half of the bench: for each of the 21 stand riders, the game's golem
//! search ([`redsim::entity::mob::spawn::golem_spawn_cells_from`], the judge's own function) cell
//! by cell, each spawn spot classed by portal contact; [`bench`] runs the seven batches through
//! the slots with the real release + hold part and measures the summons, the golems' stay and the
//! pulse.

use crate::hold::{BATCHES, LaneOpts};
use anyhow::Result;
use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::ai::brain::{self, Act};
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::mob::types::MobType;
use redsim::entity::mob::MobEvent;
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::world::World;

/// The pad's shape.
#[derive(Clone, Debug)]
pub struct PadOpts {
    /// The trench rows' x range; the portals' interior is one in from each end (x0+1..x1-1).
    pub x0: i32,
    pub x1: i32,
    /// The portal planes (z), each with a trench row on both sides.
    pub planes: Vec<i32>,
    /// The frames' bottom row (y).
    pub by: i32,
    /// The portals' height (3..21).
    pub h: i32,
}

impl Default for PadOpts {
    fn default() -> PadOpts {
        PadOpts { x0: -2, x1: 8, planes: vec![-5, -2], by: 0, h: 3 }
    }
}

const PORTAL: &str = "minecraft:nether_portal[axis=x]";
const OBSIDIAN: &str = "minecraft:obsidian";
const FLOOR: &str = "minecraft:cobblestone";
const GLASS: &str = "minecraft:glass";
const STRING: &str = "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]";
const DUST: &str = "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]";

impl PadOpts {
    /// The trench floors' y (a golem's feet one above, at the portals' top row).
    pub fn floor_y(&self) -> i32 {
        self.by + self.h - 1
    }

    /// The tripwire lines' y.
    pub fn wire_y(&self) -> i32 {
        self.by + self.h + 2
    }

    /// The trench rows (z), each once, in order.
    pub fn rows(&self) -> Vec<i32> {
        let mut r: Vec<i32> = self.planes.iter().flat_map(|&p| [p - 1, p + 1]).collect();
        r.sort();
        r.dedup();
        r
    }

    /// The counter's pulse: the last dust of the output line.
    pub fn pulse(&self) -> [i32; 3] {
        let z = *self.planes.iter().max().unwrap_or(&0);
        [self.x1 + 1, self.wire_y() + 1, z]
    }

    /// The `station:` line of a bench design.
    pub fn meta(&self) -> String {
        let planes: Vec<String> = self.planes.iter().map(|p| p.to_string()).collect();
        format!("x0={} x1={} planes={} by={} h={}", self.x0, self.x1, planes.join(","), self.by, self.h)
    }

    /// The pad's shape from a bench design's `station:` line.
    pub fn of(meta: &std::collections::BTreeMap<String, String>) -> Result<PadOpts> {
        let line = meta.get("station").ok_or_else(|| anyhow::anyhow!("not a station design (no `station:` line)"))?;
        let mut o = PadOpts::default();
        for kv in line.split_whitespace() {
            let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("bad `station:` entry {kv:?}"))?;
            match k {
                "x0" => o.x0 = v.parse()?,
                "x1" => o.x1 = v.parse()?,
                "planes" => o.planes = v.split(',').map(|p| p.parse()).collect::<Result<_, _>>()?,
                "by" => o.by = v.parse()?,
                "h" => o.h = v.parse()?,
                _ => anyhow::bail!("unknown `station:` entry {k:?}"),
            }
        }
        Ok(o)
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.x1 - self.x0 - 1 >= 2 && self.x1 - self.x0 - 1 <= 21, "a portal interior of 2..21 wide (x0+1..x1-1)");
        anyhow::ensure!((3..=21).contains(&self.h), "a portal 3..21 tall");
        anyhow::ensure!(!self.planes.is_empty(), "at least one plane");
        let mut p = self.planes.clone();
        p.sort();
        for w in p.windows(2) {
            anyhow::ensure!(w[1] - w[0] >= 3, "planes 3 or more apart (a trench row between them on each side: z {} and {})", w[0], w[1]);
        }
        Ok(())
    }
}

/// The pad's blocks.
pub fn pad(o: &PadOpts) -> Result<Vec<([i32; 3], String)>> {
    o.validate()?;
    let mut out: Vec<([i32; 3], String)> = Vec::new();
    let (fy, wy) = (o.floor_y(), o.wire_y());
    for z in o.rows() {
        for x in o.x0..=o.x1 {
            out.push(([x, fy, z], FLOOR.into()));
        }
    }
    for &z in &o.planes {
        for x in o.x0 + 1..o.x1 {
            out.push(([x, o.by, z], OBSIDIAN.into()));
            out.push(([x, o.by + o.h + 1, z], OBSIDIAN.into()));
            for y in o.by + 1..=o.by + o.h {
                out.push(([x, y, z], PORTAL.into()));
            }
            out.push(([x, wy, z], STRING.into()));
        }
        for y in o.by + 1..=o.by + o.h {
            out.push(([o.x0, y, z], OBSIDIAN.into()));
            out.push(([o.x1, y, z], OBSIDIAN.into()));
        }
        out.push(([o.x0, wy, z], "minecraft:tripwire_hook[attached=true,facing=east,powered=false]".into()));
        out.push(([o.x0 - 1, wy, z], GLASS.into()));
        out.push(([o.x1, wy, z], "minecraft:tripwire_hook[attached=true,facing=west,powered=false]".into()));
        out.push(([o.x1 + 1, wy, z], FLOOR.into()));
    }
    // the output line: a dust over each east support, along x1+1 between the planes on glass
    let (z0, z1) = (*o.planes.iter().min().unwrap_or(&0), *o.planes.iter().max().unwrap_or(&0));
    for z in z0..=z1 {
        if !o.planes.contains(&z) {
            out.push(([o.x1 + 1, wy, z], GLASS.into()));
        }
        out.push(([o.x1 + 1, wy + 1, z], DUST.into()));
    }
    Ok(out)
}

/// The pad's Nether part (relative to the Nether image of the design's anchor, its y as written):
/// a 2 x 3 Nether portal (axis z, plane x 0, interior z 0..1, y 64..66) linked to every pad plane
/// (it is the only portal within 16 of their targets). A golem arrives centred on the plane, the
/// collision-free search moving it 0.2 east off the player corridor's ceiling (west of the plane,
/// 2 high: a golem never fits in it). East of the plane, a pit 2 deep (x 1..`len`, z 0..1, hopper
/// floor at y 61, feet 62) under a lava sheet at y 64: sources at x 1 against the plane, held by
/// wall signs at y 63, flowing to the wall at x len+1. The sheet at its feet pushes the golem off
/// the frame's bottom row into the pit (it cannot step back up 2) and burns it with the sheet at
/// its head; the flow carries it east, clear of the next golem's drop (a short pit piles them at
/// the drop and their pushes lift the newcomer onto the walls: 4 long, the last one is past it);
/// it dies with its feet over the hoppers, which feed the chest at (1, 60, 0). Every wall rises to
/// y 66 (its top 3 over the frame's bottom row and 5 over the pit: a golem steps up 1).
pub fn nether_part() -> (Vec<([i32; 3], String)>, [i32; 3]) {
    let len = 4;
    let mut out: Vec<([i32; 3], String)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, s: &str| out.push(([x, y, z], if s.starts_with("minecraft:") { s.to_string() } else { format!("minecraft:{s}") }));
    for z in 0..=1 {
        put(0, 63, z, "obsidian");
        put(0, 67, z, "obsidian");
        for y in 64..=66 {
            put(0, y, z, "nether_portal[axis=z]");
        }
        // the player corridor's floor and ceiling (2 high: the player's way out, a golem's no)
        put(-1, 63, z, "cobblestone");
        put(-1, 66, z, "cobblestone");
        // under the frame: the pit's west end
        put(0, 62, z, "cobblestone");
        // the end wall
        for y in 62..=66 {
            put(len + 1, y, z, "cobblestone");
        }
        put(1, 64, z, "lava[level=0]");
    }
    for y in 64..=66 {
        put(0, y, -1, "obsidian");
        put(0, y, 2, "obsidian");
    }
    for x in 1..=len {
        for y in 62..=66 {
            put(x, y, -1, "cobblestone");
            put(x, y, 2, "cobblestone");
        }
        put(x, 63, 0, "oak_wall_sign[facing=south,waterlogged=false]");
        put(x, 63, 1, "oak_wall_sign[facing=north,waterlogged=false]");
        // the hopper floor: row z 1 north into row z 0, row z 0 west to x 1, down into the chest
        put(x, 61, 1, "hopper[enabled=true,facing=north]");
        put(x, 61, 0, if x == 1 { "hopper[enabled=true,facing=down]" } else { "hopper[enabled=true,facing=west]" });
    }
    put(1, 60, 0, "chest[facing=east,type=single,waterlogged=false]");
    (out, [1, 60, 0])
}

/// The station's bench design: the release + hold part ([`crate::hold::design`]) with the pad
/// and its Nether part.
pub fn design(o: &PadOpts, lanes: &LaneOpts) -> Result<String> {
    let text = crate::hold::design(lanes)?;
    let mut d = Design::parse(&text)?;
    for (p, s) in pad(o)? {
        if let Some(old) = d.blocks.get(&p)
            && *old != s
        {
            anyhow::bail!("the pad's {s} at {p:?} meets the release + hold part's {old}");
        }
        d.blocks.insert(p, s);
    }
    let (nether, out) = nether_part();
    for (p, s) in nether {
        d.nether_blocks.insert(p, s);
    }
    d.nether_ports.insert("nether-output".into(), out);
    d.ports.insert("nether-anchor".into(), [0, 0, 0]);
    d.ports.insert("nether-axis".into(), [1, 0, 0]);
    d.name = "carousel-station-7".into();
    d.meta.insert("station".into(), o.meta());
    d.ports.insert("output.pulse".into(), o.pulse());
    // every villager's HOME the shared bed (the R1 double claim, as the judge loads it)
    let bed = lanes.bed();
    d.ports.insert("home".into(), [bed.x, bed.y, bed.z]);
    let w = o.x1 - o.x0 - 1;
    let caption = [
        format!("The carousel's station + portal pad (bench): {} portal plane(s) {w} x {} at z {:?}, a trench row on both sides of each (x {}..{}, floors at y {}),", o.planes.len(), o.h, o.planes, o.x0, o.x1, o.floor_y()),
        "a tripwire line on each frame's top row (the counter's pulse); on the release + hold part's bench (7 lanes of 3 stacked tracks).".into(),
    ];
    let mut s = String::new();
    for l in caption {
        s.push_str("# ");
        s.push_str(&l);
        s.push('\n');
    }
    s.push_str(&d.to_text());
    Ok(s)
}

/// A stand rider's cell: lane `b`, level `k` (the part's parked carts: (b+0.5, 3k+0.57, k+0.49)).
pub fn rider_cell(lanes: &LaneOpts, b: i32, k: i32) -> BlockPos {
    BlockPos::new(lanes.x(b), lanes.level_gap * k, k)
}

/// A stand rider's parked feet.
pub fn rider_feet(lanes: &LaneOpts, b: i32, k: i32) -> Vec3 {
    Vec3::new(lanes.x(b) as f64 + 0.5, (lanes.level_gap * k) as f64 + 0.57, k as f64 + 0.49)
}

fn is_portal(w: &World, p: BlockPos) -> bool {
    match w.get(p) {
        Block::Terrain(id) => redsim::terrain::blocks::props(id).block_id() == "minecraft:nether_portal",
        _ => false,
    }
}

/// The portal cells a box meets (`Entity.checkInsideBlocks`: the cells its box spans, a hair in
/// from each face).
fn portal_cells_met(w: &World, bb: Aabb) -> Vec<BlockPos> {
    let e = 1.0e-7;
    let mut out = Vec::new();
    for x in (bb.min_x + e).floor() as i32..=(bb.max_x - e).floor() as i32 {
        for y in (bb.min_y + e).floor() as i32..=(bb.max_y - e).floor() as i32 {
            for z in (bb.min_z + e).floor() as i32..=(bb.max_z - e).floor() as i32 {
                let p = BlockPos::new(x, y, z);
                if is_portal(w, p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn golem_box(cell: BlockPos) -> Aabb {
    let d = MobType::IronGolem.def();
    redsim::entity::make_bounding_box(Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5), d.width, d.height)
}

/// The smallest inflation (every axis alike) that makes box `bb` meet a portal cell within 2 of
/// it: the largest of its per-axis gaps to the nearest such cell; None with none within 2.
fn portal_gap(w: &World, bb: Aabb) -> Option<f64> {
    let mut best: Option<f64> = None;
    for x in bb.min_x.floor() as i32 - 2..=bb.max_x.floor() as i32 + 2 {
        for y in bb.min_y.floor() as i32 - 2..=bb.max_y.floor() as i32 + 2 {
            for z in bb.min_z.floor() as i32 - 2..=bb.max_z.floor() as i32 + 2 {
                if !is_portal(w, BlockPos::new(x, y, z)) {
                    continue;
                }
                let gap = |lo: f64, hi: f64, c: i32| (c as f64 - hi).max(lo - (c as f64 + 1.0)).max(0.0);
                let g = gap(bb.min_x, bb.max_x, x).max(gap(bb.min_y, bb.max_y, y)).max(gap(bb.min_z, bb.max_z, z));
                best = Some(best.map_or(g, |b: f64| b.min(g)));
            }
        }
    }
    best
}

fn kind_name(k: &EntityKind) -> &'static str {
    match k {
        EntityKind::Boat(_) => "boat",
        EntityKind::Minecart(_) => "minecart",
        EntityKind::Item(_) => "item",
        EntityKind::ArmorStand(_) => "armor stand",
        EntityKind::PrimedTnt(_) => "tnt",
        EntityKind::Mob(m) => m.ty.def().id,
    }
}

fn is_piglin(k: &EntityKind) -> bool {
    matches!(k, EntityKind::Mob(m) if m.ty.def().id == "zombified_piglin")
}

/// One rider's search, classed.
#[derive(Clone, Debug, Default)]
pub struct RiderSearch {
    pub lane: i32,
    pub level: i32,
    pub cell: [i32; 3],
    /// Spawn spots whose golem touches a portal cell.
    pub valid: Vec<[i32; 3]>,
    /// Spawn spots without a portal contact (violations).
    pub stray: Vec<[i32; 3]>,
    /// Spawn spots the batch's own parked carts and riders obstruct.
    pub obstructed: Vec<[i32; 3]>,
}

/// The static check's report.
#[derive(Clone, Debug, Default)]
pub struct CheckReport {
    pub riders: Vec<RiderSearch>,
    /// Entities (and the stand carts) closer than a cell to a portal cell, with the gap (0: they
    /// touch one).
    pub near_portal: Vec<(String, f64)>,
    pub portal_cells: usize,
    pub obsidian: usize,
    pub pad_blocks: usize,
    /// The pad's box (x, y, z extents) and volume.
    pub pad_box: ([i32; 3], [i32; 3]),
}

impl CheckReport {
    /// A batch's success per slot with `c` callers (each its 10 tries; the first golem ends it).
    pub fn success(&self, lane: i32, c: usize) -> f64 {
        let ks: Vec<usize> = self.riders.iter().filter(|r| r.lane == lane).map(|r| r.valid.len() - r.obstructed.len().min(r.valid.len())).collect();
        if ks.is_empty() {
            return 0.0;
        }
        // (the c callers with the fewest spots: the worst case for any c of the three)
        let mut ks = ks;
        ks.sort();
        let fail: f64 = ks.iter().take(c).map(|&k| (1.0 - k as f64 / 289.0).powi(10)).product();
        1.0 - fail
    }
}

/// The world a station design runs in (the bench's: built, settled, its entities spawned).
fn world_of(spec: &FarmSpec, seed: i64) -> Result<Run, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(seed, 24000, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    Ok(run)
}

/// The stand batch's parked carts and riders (lane `b`): their boxes.
fn stand_boxes(lanes: &LaneOpts, b: i32) -> Vec<Aabb> {
    let mut out = Vec::new();
    for k in 0..lanes.levels {
        let f = rider_feet(lanes, b, k);
        out.push(redsim::entity::make_bounding_box(f, 0.98, 0.7));
        out.push(redsim::entity::make_bounding_box(f, 0.6, 1.95));
    }
    out
}

/// The static check: every stand rider's golem search, cell by cell (the judge's function), each
/// spawn spot classed by portal contact; the batch's own parked boxes against each spot; every
/// entity's box (and the stand carts') a cell clear of every portal cell; the pad's counts.
pub fn check(spec: &FarmSpec, lanes: &LaneOpts, o: &PadOpts) -> Result<CheckReport, String> {
    let run = world_of(spec, 1)?;
    let w = &run.world;
    let mut rep = CheckReport::default();
    for b in 0..BATCHES {
        let parked = stand_boxes(lanes, b);
        for k in 0..lanes.levels {
            let start = rider_cell(lanes, b, k);
            let mut r = RiderSearch { lane: b, level: k, cell: [start.x, start.y, start.z], ..Default::default() };
            for (cell, res) in redsim::entity::mob::spawn::golem_spawn_cells_from(w, start) {
                if res.is_err() {
                    continue;
                }
                let bb = golem_box(cell);
                let c = [cell.x, cell.y, cell.z];
                if portal_cells_met(w, bb).is_empty() {
                    r.stray.push(c);
                } else {
                    r.valid.push(c);
                    if parked.iter().any(|p| p.intersects(bb)) {
                        r.obstructed.push(c);
                    }
                }
            }
            rep.riders.push(r);
        }
    }
    // every staying box clear of the portals (a cell clear, the contract's margin): the design's
    // entities as they stand, the stand carts and riders where they park; the gap is the
    // inflation that would make a box meet a portal cell (0: it touches one)
    for e in &w.entities.list {
        if let Some(g) = portal_gap(w, e.bb)
            && g < 1.0
        {
            rep.near_portal.push((format!("{} at {:.2} {:.2} {:.2}", kind_name(&e.kind), e.pos.x, e.pos.y, e.pos.z), g));
        }
    }
    for b in 0..BATCHES {
        for bb in stand_boxes(lanes, b) {
            if let Some(g) = portal_gap(w, bb)
                && g < 1.0
            {
                rep.near_portal.push((format!("lane {b}'s parked cart or rider (box {:.2}..{:.2} in z)", bb.min_z, bb.max_z), g));
            }
        }
    }
    let blocks = pad(o).map_err(|e| e.to_string())?;
    rep.pad_blocks = blocks.len();
    rep.portal_cells = blocks.iter().filter(|(_, s)| s == PORTAL).count();
    rep.obsidian = blocks.iter().filter(|(_, s)| s == OBSIDIAN).count();
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for (p, _) in &blocks {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    rep.pad_box = (lo, hi);
    Ok(rep)
}

/// The pulse's path after a golem stands on a trench cell (`station-check --probe`): each tick,
/// where the golem is and the states along one plane's line (a string over the cell, the east
/// hook, its support, the output dust).
pub fn probe_pulse(spec: &FarmSpec, o: &PadOpts, cell: [i32; 3], ticks: u32) -> Result<Vec<String>, String> {
    let mut run = world_of(spec, 1)?;
    let w = &mut run.world;
    let pos = [cell[0] as f64 + 0.5, cell[1] as f64, cell[2] as f64 + 0.5];
    let id = redsim::entity::spawn(w, "minecraft:iron_golem", pos, None)?;
    let plane = *o.planes.iter().min_by_key(|p| (**p - cell[2]).abs()).unwrap_or(&0);
    let at = |x: i32, y: i32, z: i32| BlockPos::new(x, y, z);
    let cells = [at(cell[0].clamp(o.x0 + 1, o.x1 - 1), o.wire_y(), plane), at(o.x1, o.wire_y(), plane), at(o.x1 + 1, o.wire_y(), plane), at(o.x1 + 1, o.wire_y() + 1, plane), BlockPos::new(o.pulse()[0], o.pulse()[1], o.pulse()[2])];
    let mut out = Vec::new();
    for t in 0..=ticks {
        if t > 0 {
            w.tick();
        }
        let g = w.entities.list.iter().find(|e| e.id == id && !e.removed).map(|e| format!("golem at {:.2} {:.2} {:.2}", e.pos.x, e.pos.y, e.pos.z)).unwrap_or_else(|| "golem gone".into());
        let states: Vec<String> = cells.iter().map(|p| format!("{p}: {}", w.get(*p))).collect();
        out.push(format!("t+{t}: {g}; {}", states.join(" | ")));
    }
    Ok(out)
}

/// `station-check --official`: the pad and its Nether part on the official server, one golem
/// summoned on each given trench cell `stagger` ticks apart, printed each tick for `ticks` ticks
/// after each summon: where the golem is (overworld, Nether or gone) and the pulse's dust power.
pub fn probe_official(version: &str, o: &PadOpts, cells: &[[i32; 3]], ticks: u32, stagger: u32) -> Result<Vec<String>> {
    use mctas::vanilla::{VanillaOptions, VanillaServer};
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    let ow = pad(o)?;
    let (ne, _) = nether_part();
    let boxed = |b: &[([i32; 3], String)], m: i32| {
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for (p, _) in b {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i] - m);
                hi[i] = hi[i].max(p[i] + m);
            }
        }
        (lo, hi)
    };
    let mut out = Vec::new();
    for (dim, blocks) in [("minecraft:overworld", &ow), ("minecraft:the_nether", &ne)] {
        let run = format!("execute in {dim} run");
        let (lo, hi) = boxed(blocks, 4);
        srv.cmd_ok(&format!("{run} forceload add {} {} {} {}", lo[0], lo[2], hi[0], hi[2]))?;
        srv.step(40)?;
        for y in lo[1]..=hi[1] {
            srv.cmd(&format!("{run} fill {} {y} {} {} {y} {} minecraft:air", lo[0], lo[2], hi[0], hi[2]))?;
        }
        // (strict: no updates, so the portal cells stand before their frame is whole; the
        // attached blocks after their supports; fluids last, each its own placing update)
        let fluid = |s: &str| s.starts_with("minecraft:lava");
        let late = |s: &str| s.contains("tripwire") || s.contains("wall_sign") || s.contains("redstone_wire") || s.contains("nether_portal");
        for pass in 0..3 {
            for (p, s) in blocks.iter().filter(|(_, s)| match pass {
                0 => !fluid(s) && !late(s),
                1 => late(s),
                _ => fluid(s),
            }) {
                let r = srv.cmd(&format!("{run} setblock {} {} {} {s}{}", p[0], p[1], p[2], if pass == 2 { "" } else { " strict" }))?;
                if !r.contains("Changed the block") && !r.contains("Could not set the block") {
                    anyhow::bail!("setblock {p:?} {s} in {dim}: {r}");
                }
            }
        }
    }
    srv.step(200)?;
    let pulse = o.pulse();
    for (k, c) in cells.iter().enumerate() {
        if k > 0 {
            srv.step(stagger)?;
        }
        let tag = format!("sp{k}");
        srv.cmd_ok(&format!("summon minecraft:iron_golem {} {} {} {{PersistenceRequired:1b,Tags:[\"{tag}\"]}}", c[0] as f64 + 0.5, c[1], c[2] as f64 + 0.5))?;
        for t in 0..=ticks {
            if t > 0 {
                srv.step(1)?;
            }
            let here = |srv: &mut VanillaServer, dim: &str| -> Result<bool> { Ok(srv.cmd(&format!("execute in {dim} positioned 0 0 0 if entity @e[tag={tag},distance=0..]"))?.contains("passed")) };
            let at = if here(&mut srv, "minecraft:overworld")? {
                "overworld"
            } else if here(&mut srv, "minecraft:the_nether")? {
                "nether"
            } else {
                "gone"
            };
            let unpowered = srv.cmd(&format!("execute if block {} {} {} minecraft:redstone_wire[power=0]", pulse[0], pulse[1], pulse[2]))?.contains("passed");
            out.push(format!("golem {k} on {c:?} t+{t}: {at}, pulse {}", if unpowered { "off" } else { "ON" }));
        }
    }
    Ok(out)
}

/// The dynamic bench's options.
#[derive(Clone, Copy, Debug)]
pub struct BenchOpts {
    pub seed: i64,
    pub days: u32,
    /// Each line switch 1 + a draw in 0..=jitter gt after its slot (the station's pulse delay and
    /// the counter's).
    pub jitter: u32,
    pub verbose: bool,
}

/// What the dynamic bench measured.
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    pub slots: u32,
    /// Slots where a golem spawned.
    pub summoned: u32,
    /// Per slot: the batch's riders that wanted a golem at the slot (0..=3), and how many tried.
    pub willing: Vec<u8>,
    pub tries: Vec<u8>,
    /// Per slot: whether a golem came.
    pub ok: Vec<bool>,
    /// Each golem's overworld stay (gt from its spawn tick to its removal), and the ones still there
    /// at the end.
    pub dwell: Vec<i64>,
    pub lingering: u32,
    /// Golems that spawned without a portal contact.
    pub stray: Vec<String>,
    /// The pulse: per golem, its delay from the spawn tick (None: no pulse) and its length.
    pub pulse: Vec<(Option<i64>, i64)>,
    /// Pulses with no golem.
    pub spurious_pulses: u32,
    /// The batch's riders panicking at the slot, per slot (0..=3).
    pub panicking: Vec<u8>,
    /// Zombified piglins: seen (distinct ids) and those that left the overworld.
    pub piglins: u32,
    pub piglins_crossed: u32,
    /// Piglins ever out of the portal cells, and alive at the end (the rest despawned).
    pub piglins_out: u32,
    pub piglins_alive: u32,
    /// The Nether part: each golem's life there (gt from its arrival to its death), the ones still
    /// alive at the end, golems that came back to the overworld, items seen in the overworld
    /// (drops sent back), the iron in the Nether chest, its hoppers and lying loose.
    pub nether_life: Vec<i64>,
    pub nether_alive: u32,
    pub returned: u32,
    pub items_back: u32,
    pub iron_chest: u32,
    pub iron_hoppers: u32,
    pub iron_loose: u32,
    /// The iron that dropped in the Nether (each iron item counted when first seen), and when the
    /// output chest was first full (gt after the run's start).
    pub iron_dropped: u32,
    pub poppies_dropped: u32,
    pub iron_burnt: u32,
    pub chest_full_at: Option<i64>,
    pub violations: Vec<String>,
    pub violation_count: u32,
    pub errors: Vec<String>,
}

fn wire_power(w: &World, p: [i32; 3]) -> i32 {
    match w.get(BlockPos::new(p[0], p[1], p[2])) {
        Block::Wire { power, .. } => power as i32,
        _ => 0,
    }
}

/// The seven batches through the slots on the release + hold part's lines (each switch 1 +
/// 0..=jitter gt after a slot, as [`crate::hold::bench`]), measuring every slot's willing riders and
/// tries, every golem's stay and contact, the pulse and the zombified piglins.
pub fn bench(spec: &FarmSpec, lanes: &LaneOpts, o: &PadOpts, bo: &BenchOpts) -> Result<BenchReport, String> {
    let ro = RunOpts { ai: true, ..RunOpts::steady(bo.seed, bo.days * 24000, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let first = run.world.entities.next_id;
    for (ty, pos, nbt) in &spec.entities {
        redsim::entity::spawn(&mut run.world, ty, *pos, nbt.as_deref())?;
    }
    run.own = (first..run.world.entities.next_id).collect();
    let bed = lanes.bed();
    let t0 = run.world.time;
    redsim::entity::mob::ai::poi::take(&mut run.world, bed);
    let mut riders: Vec<(u32, i32)> = Vec::new();
    for i in 0..run.world.entities.list.len() {
        let e = &run.world.entities.list[i];
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.ty != MobType::Villager {
            continue;
        }
        let lane = (0..BATCHES).find(|&k| (e.pos.x - (lanes.x(k) as f64 + 0.5)).abs() < 0.4).ok_or_else(|| format!("a villager at x {:.2} is in no lane", e.pos.x))?;
        riders.push((e.id, lane));
        let b = brain::brain_mut(&mut run.world, i);
        b.home = Some(bed);
        b.last_slept = Some(t0 - 100);
    }
    // the slot grid and the line switches (as the release + hold bench draws them)
    let mut rng = redsim::rng::LegacyRandom::new(bo.seed ^ 0x686f_6c64);
    let s0 = ((t0 + 150) / 100 + 1) * 100;
    let end = t0 + bo.days as i64 * 24000;
    let mut slots: Vec<i64> = Vec::new();
    let mut s = s0;
    while s <= end {
        slots.push(s);
        s += 100;
    }
    let switches: Vec<i64> = (0..slots.len())
        .map(|n| {
            let base = if n == 0 { slots[0] - 99 } else { slots[n - 1] + 1 };
            base + if bo.jitter > 0 { rng.next_int(bo.jitter as i32 + 1) as i64 } else { 0 }
        })
        .collect();
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let off = Block::parse("minecraft:glass").map_err(|e| e.to_string())?;
    let set_line = |w: &mut World, k: i32, high: bool| {
        let fl = redsim::world::flags::ALL;
        for p in lanes.hold_ctrls(k) {
            w.set_block(p, if high { on.clone() } else { off.clone() }, fl);
        }
        for p in lanes.stand_ctrls(k) {
            w.set_block(p, if high { off.clone() } else { on.clone() }, fl);
        }
    };
    let mut rep = BenchReport::default();
    let violate = |rep: &mut BenchReport, t: i64, what: String| {
        rep.violation_count += 1;
        if rep.violations.len() < 12 {
            rep.violations.push(format!("gt {t}: {what}"));
        }
    };
    let pulse_at = o.pulse();
    let index_of = |w: &World, id: u32| w.entities.list.iter().position(|e| e.id == id && !e.removed);
    let mut next_switch = 0usize;
    let mut next_slot = 0usize;
    let mut selected: Option<i32> = None;
    // every golem: (id, spawn tick, pulse rise, pulse fall, removal); `alive` indexes the ones
    // still in the overworld
    let mut golems: Vec<(u32, i64, Option<i64>, Option<i64>, Option<i64>)> = Vec::new();
    let mut alive: Vec<usize> = Vec::new();
    let mut pulse_on: Option<(i64, Option<usize>)> = None;
    let mut piglins: std::collections::BTreeSet<u32> = Default::default();
    let mut piglins_out: std::collections::BTreeSet<u32> = Default::default();
    // the Nether: (copy id, arrival tick, death tick); crossings read so far; ids seen back
    let mut nether: Vec<(u32, i64, Option<i64>, (Vec3, f32, i64))> = Vec::new();
    let mut crossed_seen = run.world.crossings.len();
    let mut items_back: std::collections::BTreeSet<u32> = Default::default();
    let mut nether_ev_seen = run.world.nether.as_deref().map_or(0, |n| n.mob.events.len());
    let mut returned: std::collections::BTreeSet<u32> = Default::default();
    let mut ev_seen = run.world.mob.events.len();
    // the slot being summoned: its tick, the batch's riders, tries, success
    let mut slot_tries: Option<(i64, Vec<u32>, u8, bool)> = None;
    let mut slept_night: Option<i64> = None;
    while run.world.time < end {
        let t_next = run.world.time + 1;
        // the sleep routine's ideal stand-in (as the release + hold bench): every villager's
        // LAST_SLEPT set once each night, so the riders keep wanting golems past day one
        let night_of = run.world.day_time() / 24000;
        if run.world.day_time() % 24000 >= 13000 && slept_night != Some(night_of) {
            slept_night = Some(night_of);
            let now = run.world.time;
            for (id, _) in &riders {
                if let Some(i) = index_of(&run.world, *id) {
                    brain::brain_mut(&mut run.world, i).last_slept = Some(now);
                }
            }
        }
        while next_switch < switches.len() && switches[next_switch] <= t_next {
            let k = (next_switch % BATCHES as usize) as i32;
            if let Some(p) = selected {
                set_line(&mut run.world, p, false);
            }
            set_line(&mut run.world, k, true);
            selected = Some(k);
            next_switch += 1;
        }
        // the slot about to tick: who of the batch wants a golem, who panics
        if next_slot < slots.len() && slots[next_slot] == t_next {
            let k = (next_slot % BATCHES as usize) as i32;
            let w = &run.world;
            let (mut want, mut panic) = (0u8, 0u8);
            let mut ids = Vec::new();
            for (id, lane) in &riders {
                if *lane != k {
                    continue;
                }
                ids.push(*id);
                let Some(i) = index_of(w, *id) else { continue };
                if let EntityKind::Mob(m) = &w.entities.list[i].kind
                    && m.brain.as_deref().is_some_and(|b| b.wants_golem(t_next))
                {
                    want += 1;
                }
                if brain::has_exact(w, i) && brain::ex(w, i).active == Act::Panic {
                    panic += 1;
                }
            }
            rep.willing.push(want);
            rep.panicking.push(panic);
            slot_tries = Some((t_next, ids, 0, false));
            rep.slots += 1;
            next_slot += 1;
        }
        run.tick();
        let t = run.world.time;
        // the tick's events: the stand batch's tries and every golem
        let evs: Vec<MobEvent> = run.world.mob.events[ev_seen.min(run.world.mob.events.len())..].to_vec();
        ev_seen = run.world.mob.events.len();
        for ev in evs {
            if let MobEvent::GolemTry { t: et, villager, golem, at, .. } = ev {
                if let Some((st, ids, n, ok)) = slot_tries.as_mut()
                    && *st == et
                    && ids.contains(&villager)
                {
                    *n += 1;
                    *ok |= golem.is_some();
                }
                if let (Some(id), Some(c)) = (golem, at) {
                    let cell = BlockPos::new(c[0], c[1], c[2]);
                    if portal_cells_met(&run.world, golem_box(cell)).is_empty() {
                        rep.stray.push(format!("gt {et}: a golem at {c:?} touches no portal"));
                    }
                    if !slot_tries.as_ref().is_some_and(|(st, ids, _, _)| *st == et && ids.contains(&villager)) {
                        violate(&mut rep, et, format!("a golem summoned by villager {villager}, not the stand batch's"));
                    }
                    golems.push((id, et, None, None, None));
                    alive.push(golems.len() - 1);
                }
            }
        }
        if let Some((st, _, n, ok)) = &slot_tries
            && *st == t
        {
            rep.tries.push(*n);
            rep.ok.push(*ok);
            if *ok {
                rep.summoned += 1;
            }
            slot_tries = None;
        }
        // the pulse: a rise within 20 gt of the newest golem without one is its pulse
        let p = wire_power(&run.world, pulse_at);
        match (p > 0, pulse_on) {
            (true, None) => {
                let g = golems.iter().rposition(|g| g.2.is_none() && t - g.1 <= 20);
                match g {
                    Some(gi) => golems[gi].2 = Some(t),
                    None => rep.spurious_pulses += 1,
                }
                pulse_on = Some((t, g));
            }
            (false, Some((_, g))) => {
                if let Some(gi) = g {
                    golems[gi].3 = Some(t);
                }
                pulse_on = None;
            }
            _ => {}
        }
        // the golems' stay
        let w = &run.world;
        alive.retain(|&gi| {
            if index_of(w, golems[gi].0).is_some() {
                return true;
            }
            golems[gi].4 = Some(t);
            false
        });
        for &gi in &alive {
            let (id, born, ..) = golems[gi];
            if t - born == 20 {
                violate(&mut rep, t, format!("golem {id} still in the overworld 20 gt after its spawn"));
            }
        }
        // the zombified piglins: seen, ever out of the portal cells (not confined), in a golem's
        // spawn box at a summon (they would block it)
        for e in &w.entities.list {
            if is_piglin(&e.kind) && !e.removed {
                piglins.insert(e.id);
                if portal_cells_met(w, e.bb).is_empty() && piglins_out.insert(e.id) {
                    violate(&mut rep, t, format!("a zombified piglin out of the portal at {:.2} {:.2} {:.2}", e.pos.x, e.pos.y, e.pos.z));
                }
            }
        }
        // the Nether: each crossing golem's copy, its death; golems back; items back
        for &(ct, from, to) in &w.crossings[crossed_seen.min(w.crossings.len())..] {
            if golems.iter().any(|g| g.0 == from) {
                nether.push((to, ct, None, (Vec3::new(0.0, 0.0, 0.0), 100.0, ct)));
            }
        }
        crossed_seen = w.crossings.len();
        if let Some(n) = w.nether.as_deref() {
            for g in nether.iter_mut().filter(|g| g.2.is_none()) {
                match n.entities.list.iter().find(|e| e.id == g.0 && !e.removed) {
                    None => g.2 = Some(t),
                    Some(e) => {
                        let hp = match &e.kind {
                            EntityKind::Mob(m) => m.health,
                            _ => 0.0,
                        };
                        g.3 = (e.pos, hp, t);
                    }
                }
            }
            // the iron dropped there (the golems' Drop events) and burnt
            let evs = &n.mob.events[nether_ev_seen.min(n.mob.events.len())..];
            for ev in evs {
                match ev {
                    MobEvent::Drop { what: "minecraft:iron_ingot", count, .. } => rep.iron_dropped += *count,
                    MobEvent::Drop { what: "minecraft:poppy", count, .. } => rep.poppies_dropped += *count,
                    MobEvent::Burnt { what, count, .. } if what == "minecraft:iron_ingot" => rep.iron_burnt += *count,
                    _ => {}
                }
            }
            nether_ev_seen = n.mob.events.len();
            if rep.chest_full_at.is_none() {
                let full = spec.nether.as_ref().and_then(|ns| ns.output_at()).and_then(|p| n.containers.map.get(&p)).is_some_and(|c| c.slots.iter().all(|s| s.as_ref().is_some_and(|s| s.count >= 64)));
                if full {
                    rep.chest_full_at = Some(t - t0);
                }
            }
        }
        for e in &w.entities.list {
            if e.removed {
                continue;
            }
            match &e.kind {
                EntityKind::Item(_) if items_back.insert(e.id) => violate(&mut rep, t, format!("an item ({}) in the overworld at {:.2} {:.2} {:.2}", kind_name(&e.kind), e.pos.x, e.pos.y, e.pos.z)),
                EntityKind::Mob(m) if m.ty == MobType::IronGolem && !golems.iter().any(|g| g.0 == e.id) && returned.insert(e.id) => violate(&mut rep, t, format!("a golem back in the overworld at {:.2} {:.2} {:.2}", e.pos.x, e.pos.y, e.pos.z)),
                _ => {}
            }
        }
    }
    for &(_, born, rise, fall, removed) in &golems {
        if let Some(r) = removed {
            rep.dwell.push(r - born);
        }
        match (rise, fall) {
            (Some(a), Some(b)) => rep.pulse.push((Some(a - born), b - a)),
            (Some(a), None) => rep.pulse.push((Some(a - born), end - a)),
            (None, _) if end - born > 20 => rep.pulse.push((None, 0)),
            _ => {}
        }
    }
    rep.lingering = alive.iter().filter(|&&gi| end - golems[gi].1 > 2).count() as u32;
    for (id, arrived, died, last) in &nether {
        match died {
            Some(d) => rep.nether_life.push(d - arrived),
            None => rep.nether_alive += 1,
        }
        let life = died.unwrap_or(end) - arrived;
        if life > 300 {
            violate(&mut rep, *arrived, format!("golem {id} lived {life} gt in the Nether{}: last seen at gt {} at {:.2} {:.2} {:.2} with {} HP", if died.is_none() { " (still alive)" } else { "" }, last.2, last.0.x, last.0.y, last.0.z, last.1));
        }
    }
    rep.returned = returned.len() as u32;
    rep.items_back = items_back.len() as u32;
    if let Some(n) = run.world.nether.as_deref() {
        use redsim::redstone::container as rc;
        let (ingot, block) = (rc::item_id("iron_ingot"), rc::item_id("iron_block"));
        let out = spec.nether.as_ref().and_then(|ns| ns.output_at());
        for (p, c) in &n.containers.map {
            let iron: u32 = c.slots.iter().flatten().map(|s| if Some(s.item) == ingot { s.count as u32 } else if Some(s.item) == block { 9 * s.count as u32 } else { 0 }).sum();
            if Some(*p) == out {
                rep.iron_chest += iron;
            } else {
                rep.iron_hoppers += iron;
            }
        }
        rep.iron_loose = n.entities.list.iter().filter(|e| !e.removed).map(|e| match &e.kind {
            EntityKind::Item(it) if it.item == "minecraft:iron_ingot" => it.count as u32,
            _ => 0,
        }).sum();
    }
    rep.piglins = piglins.len() as u32;
    rep.piglins_crossed = run.world.crossings.iter().filter(|(_, from, _)| piglins.contains(from)).count() as u32;
    rep.piglins_out = piglins_out.len() as u32;
    rep.piglins_alive = run.world.entities.list.iter().filter(|e| is_piglin(&e.kind) && !e.removed).count() as u32;
    rep.errors = run.world.errors.clone();
    let _ = bo.verbose;
    Ok(rep)
}
