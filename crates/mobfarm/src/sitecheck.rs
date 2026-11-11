//! `mctas mobfarm site-check`: the carousel brief's "The site (the placed farm)" (ironfarm's
//! 7b8d9a006 and its amend; ironspec and G1, 2026-11-11). Five ways a rider, its cart or the farm's
//! zombies can be lost that only the site can prevent, each checked against the placed farm: one
//! verdict per condition with its findings, one line each (a line that fails fails its
//! condition), then the positive controls: a copy of the placed farm with one change, which must
//! fail on the line the control aims at.
//!
//! The farm is placed as the judge places it (a terrain sample's cheapest spot, `--terrain`, or the
//! void), settled (its water flowed), its entities spawned. Every condition reads every position a
//! villager can occupy: the design's riders (their carts' cells), its `positions:` line,
//! `--positions` files and, with `--rails`, every rail cell; a villager's box there is the union of
//! a rider's (on its cart) and a villager's on foot, and a cart's box with it. The natural mobs are
//! the worst case of the spawn-proofing check (`hostile::flood`): every body from every position
//! it can stand or fly at outside the farm's own space, whatever the light.

use redsim::block::Block;
use redsim::entity::mob::hostile::{self, Body};
use redsim::entity::mob::types::MobType;
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::BlockPos;
use redsim::World;
use std::collections::{HashSet, VecDeque};

/// A melee attack's reach past the body's box, sideways (`Mob.getAttackBoundingBox`: the box
/// inflated by sqrt(2.04) - 0.6).
const REACH: f64 = 0.828_427_124_746_190_1;

/// The placed farm the five conditions read.
#[derive(Clone)]
pub struct Placed {
    pub w: World,
    pub label: String,
    /// Every position a villager can occupy (world cells).
    pub positions: Vec<BlockPos>,
    /// A villager's box at each position (a rider on its cart and a villager on foot: the union).
    pub villagers: Vec<Aabb>,
    /// A cart's box at each position on a track.
    pub carts: Vec<Aabb>,
    pub zombies: Vec<Zombie>,
    /// The farm's rail cells (an empty cart passes along them).
    pub tracks: Vec<BlockPos>,
    pub farm_blocks: Vec<BlockPos>,
    /// The farm's own bed heads and its shared bed (the one exclusion of condition 4).
    pub beds: Vec<BlockPos>,
    pub shared_bed: Option<BlockPos>,
    /// Where a golem can spawn for a villager at any position (`golem_spawn_cells_from`).
    pub golem_cells: Vec<BlockPos>,
    pub portal: Vec<BlockPos>,
    pub afk: Vec3,
    /// How the AFK spot was chosen (`--afk`, or the nearest legal spot the tool found).
    pub afk_note: String,
    /// The per-cell speeds of an empty cart from a runner trace (world cells, blocks a tick: the
    /// fastest crossing); `None`: every track counts for pickup.
    pub cart_speeds: Option<std::collections::HashMap<BlockPos, f64>>,
    /// The farm's own space (no flood starts there): every cell its villagers, carts, zombies and
    /// boats occupy, the golem cells and the two above them, every open cell of the design.
    pub interior: HashSet<BlockPos>,
    /// The terrain sample's world seed (slime chunks); `None` in the void.
    pub seed: Option<i64>,
    /// A control's additions: a part of the farm far from every rod; POIs seeded around it; item
    /// sources.
    pub extra_parts: Vec<BlockPos>,
    pub seeded_pois: Vec<(String, BlockPos)>,
    pub extra_items: Vec<BlockPos>,
}

/// One of the farm's zombies, at its post.
#[derive(Clone)]
pub struct Zombie {
    pub bb: Aabb,
    pub boat: Option<Aabb>,
    pub riding: bool,
    /// `PersistenceRequired`, or a name tag (`CustomName`).
    pub persistent: bool,
    /// What its entity data gives it to hold (`equipment` or `HandItems`), if anything.
    pub hands: Option<String>,
}

impl Zombie {
    fn eye(&self) -> Vec3 {
        Vec3::new((self.bb.min_x + self.bb.max_x) / 2.0, self.bb.min_y + 1.74, (self.bb.min_z + self.bb.max_z) / 2.0)
    }
    /// Its box and its boat's.
    fn boxes(&self) -> Vec<Aabb> {
        std::iter::once(self.bb).chain(self.boat).collect()
    }
}

/// A condition's verdict: its lines, each passing or failing.
pub struct Verdict {
    pub lines: Vec<(bool, String)>,
}

impl Verdict {
    pub fn pass(&self) -> bool {
        self.lines.iter().all(|(ok, _)| *ok)
    }
    /// The failing line that starts with `prefix`, if any.
    fn failing(&self, prefix: &str) -> Option<&str> {
        self.lines.iter().find(|(ok, l)| !*ok && l.starts_with(prefix)).map(|(_, l)| l.as_str())
    }
}

fn name_of(b: &Block) -> String {
    match b {
        Block::Terrain(id) => redsim::terrain::blocks::props(*id).name.clone(),
        _ => b.to_string(),
    }
}

fn cells_of(b: &Aabb) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for y in b.min_y.floor() as i32..=(b.max_y - 1.0e-6).floor() as i32 {
        for z in b.min_z.floor() as i32..=(b.max_z - 1.0e-6).floor() as i32 {
            for x in b.min_x.floor() as i32..=(b.max_x - 1.0e-6).floor() as i32 {
                v.push(BlockPos::new(x, y, z));
            }
        }
    }
    v
}

fn centre(b: &Aabb) -> Vec3 {
    Vec3::new((b.min_x + b.max_x) / 2.0, (b.min_y + b.max_y) / 2.0, (b.min_z + b.max_z) / 2.0)
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn at(p: BlockPos) -> String {
    format!("({},{},{})", p.x, p.y, p.z)
}

fn vat(p: Vec3) -> String {
    format!("({:.1},{:.1},{:.1})", p.x, p.y, p.z)
}

fn cell_of(v: Vec3) -> BlockPos {
    BlockPos::new(v.x.floor() as i32, v.y.floor() as i32, v.z.floor() as i32)
}

fn rider_box(p: BlockPos) -> Aabb {
    Aabb::new(p.x as f64 + 0.2, p.y as f64, p.z as f64 + 0.2, p.x as f64 + 0.8, p.y as f64 + 2.4, p.z as f64 + 0.8)
}

fn cart_box(p: BlockPos) -> Aabb {
    Aabb::new(p.x as f64 + 0.01, p.y as f64, p.z as f64 + 0.01, p.x as f64 + 0.99, p.y as f64 + 0.7, p.z as f64 + 0.99)
}

fn sec(q: BlockPos) -> (i32, i32, i32) {
    (q.x.div_euclid(16), q.y.div_euclid(16), q.z.div_euclid(16))
}

fn sec_dist(a: BlockPos, b: BlockPos) -> i32 {
    let (a, b) = (sec(a), sec(b));
    (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs())
}

/// The nearest spot to `afk` 2 or more sections (Chebyshev) from `bed`'s section, at its height,
/// moved along x or z (the site rule: no siege, `VillageSiege` needing the player within 1 section
/// of an occupied POI); in the world, it stands on the column's nearest floor to that height.
fn legal_afk(w: &World, afk: Vec3, bed: BlockPos) -> Vec3 {
    let (bx, _, bz) = sec(bed);
    let candidates = [
        Vec3::new(((bx + 2) * 16) as f64 + 0.5, afk.y, afk.z),
        Vec3::new(((bx - 2) * 16 + 15) as f64 + 0.5, afk.y, afk.z),
        Vec3::new(afk.x, afk.y, ((bz + 2) * 16) as f64 + 0.5),
        Vec3::new(afk.x, afk.y, ((bz - 2) * 16 + 15) as f64 + 0.5),
    ];
    let best = candidates.into_iter().min_by(|a, b| dist(*a, afk).total_cmp(&dist(*b, afk))).unwrap_or(afk);
    // (standing: the nearest floor in its column within 16 of that height, the player's 0.6 x 1.8
    // box fitting)
    let c = cell_of(best);
    for d in 0..=16 {
        for y in [c.y - d, c.y + d] {
            let q = BlockPos::new(c.x, y, c.z);
            if w.contains(q)
                && let Some(f) = hostile::floor_in(w, q)
                && hostile::fits(w, &hostile::VILLAGER, q, f)
            {
                return Vec3::new(best.x, f, best.z);
            }
        }
    }
    best
}

/// The farm placed: built into `terrain` at the judge's cheapest spot, or in the void; settled;
/// its entities spawned. `positions` are design cells (moved with the farm); `rods` (design X, Z
/// columns) each add a lightning rod on a 1-wide stone pillar from the column's top up to 2 above
/// the farm's top (the site guide's rod, for a design that carries none); `afk` (design
/// coordinates) puts the AFK player there instead of the site's (else the nearest legal spot);
/// `speeds` (design cells, an empty cart's fastest crossing in blocks a tick, from a runner trace)
/// limits pickup to the cells a cart passes at 0.1 or faster.
pub fn place(c: &circuit::Circuit, spec: &redsim::entity::mob::farm::FarmSpec, terrain: Option<&str>, positions: &[BlockPos], rails: bool, rods: &[(i32, i32)], afk: Option<[f64; 3]>, speeds: Option<&[(BlockPos, f64)]>) -> Result<Placed, String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let (mut w, moved, offset, site_afk, seed, label) = match terrain {
        Some(t) => {
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(c, spec, t)?));
            let moved = site.spec(spec);
            let w = circuit::mobfarm::settled_world(site, &moved);
            let seed = redsim::terrain::samples::sample(site.sample).map(|s| s.seed);
            let label = format!("in {} at offset ({},{},{})", site.sample, site.offset[0], site.offset[1], site.offset[2]);
            (w, moved, site.offset, site.afk, seed, label)
        }
        None => {
            let o = RunOpts::steady(1, 0, WalkMode::Random);
            let mut run = Run::start(spec, &o)?;
            while run.settle_tick() {}
            let mut w = run.world;
            w.settle_light();
            w.entities.list.clear();
            let afk = Vec3::new((spec.lo.x + spec.hi.x) as f64 / 2.0 + 0.5, spec.hi.y as f64 + 1.0, (spec.lo.z + spec.hi.z) as f64 / 2.0 + 0.5);
            (w, spec.clone(), [0, 0, 0], afk, None, "in the void".to_string())
        }
    };
    let mv = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let afk_given = afk.is_some();
    let afk = afk.map(|a| Vec3::new(a[0] + offset[0] as f64, a[1] + offset[1] as f64, a[2] + offset[2] as f64)).unwrap_or(site_afk);
    // the site guide's rods (`--rod X,Z`): a stone pillar from the column's top, the rod on it
    let top = moved.blocks.iter().map(|(p, _)| p.y).max().unwrap_or(0);
    for (x, z) in rods {
        let (x, z) = (x + offset[0], z + offset[2]);
        let from = redsim::weather::world_surface_height(&w, x, z);
        let stone = Block::parse("minecraft:stone").map_err(|e| format!("stone: {e}"))?;
        let rod = Block::parse("minecraft:lightning_rod[facing=up]").map_err(|e| format!("lightning rod: {e}"))?;
        for y in from..=top + 1 {
            w.place_raw(&[(BlockPos::new(x, y, z), stone)]);
        }
        w.place_raw(&[(BlockPos::new(x, top + 2, z), rod)]);
    }
    // the entities, spawned into a copy to read their boxes (the checks read the blocks alone)
    let mut with = w.clone();
    for (k, p, n) in &moved.entities {
        redsim::entity::spawn(&mut with, k, *p, n.as_deref())?;
    }
    let mut positions_w: Vec<BlockPos> = positions.iter().map(|p| mv(*p)).collect();
    let mut zombies = Vec::new();
    let mut villagers = Vec::new();
    let mut carts = Vec::new();
    for e in &with.entities.list {
        match &e.kind {
            EntityKind::Mob(m) if m.ty == MobType::Villager => {
                villagers.push(e.bb);
                let cell = match e.vehicle.and_then(|v| with.entities.list.iter().find(|x| x.id == v)) {
                    Some(cart) => cart.block_position(),
                    None => e.block_position(),
                };
                positions_w.push(cell);
            }
            EntityKind::Minecart(_) => carts.push(e.bb),
            EntityKind::Mob(m) if m.ty == MobType::Zombie => {
                let boat = e.vehicle.and_then(|v| with.entities.list.iter().find(|x| x.id == v)).map(|b| b.bb);
                zombies.push(Zombie { bb: e.bb, boat, riding: e.vehicle.is_some(), persistent: m.persistence_required, hands: None });
            }
            _ => {}
        }
    }
    // what each zombie holds, and a name tag, from the design's own entity data (in the order the
    // design lists its zombies, the order they were spawned in)
    let mut data: Vec<String> = Vec::new();
    for (k, _, n) in &moved.entities {
        let n = n.clone().unwrap_or_default();
        if k.trim_start_matches("minecraft:") == "zombie" {
            data.push(n);
        } else if let Some(i) = n.find("id:\"minecraft:zombie\"") {
            data.push(n[i..].to_string());
        }
    }
    for (z, d) in zombies.iter_mut().zip(data.iter()) {
        if d.contains("CustomName") {
            z.persistent = true;
        }
        for key in ["equipment:{", "HandItems:["] {
            if let Some(i) = d.find(key) {
                let body: String = d[i..].chars().take_while(|c| *c != '}' && *c != ']').collect();
                if body.contains("id:") {
                    z.hands = Some(body);
                }
            }
        }
    }
    if let Some(list) = c.clock.design.meta.get("positions") {
        for t in list.split_whitespace() {
            let v: Vec<i32> = t.split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("positions: {t}: {e}"))?;
            if v.len() == 3 {
                positions_w.push(mv(BlockPos::new(v[0], v[1], v[2])));
            }
        }
    }
    let mut tracks = Vec::new();
    let mut beds = Vec::new();
    let mut portal = Vec::new();
    for (p, b) in &moved.blocks {
        match b {
            Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } => tracks.push(*p),
            Block::Bed { head: true, .. } => beds.push(*p),
            _ => {
                if name_of(b).contains("nether_portal") {
                    portal.push(*p);
                }
            }
        }
    }
    if rails {
        positions_w.extend(tracks.iter().copied());
    }
    positions_w.sort_by_key(|p| (p.x, p.y, p.z));
    positions_w.dedup();
    villagers.extend(positions_w.iter().map(|p| rider_box(*p)));
    carts.extend(positions_w.iter().filter(|p| tracks.contains(p)).map(|p| cart_box(*p)));
    let shared_bed = match c.clock.design.meta.get("shared_bed") {
        Some(t) => {
            let v: Vec<i32> = t.split([',', ' ']).filter(|s| !s.is_empty()).map(|n| n.parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("shared_bed: {e}"))?;
            if v.len() != 3 {
                return Err(format!("shared_bed: {t}: x,y,z (the bed's head, design cells)"));
            }
            Some(mv(BlockPos::new(v[0], v[1], v[2])))
        }
        None if beds.len() == 1 => Some(beds[0]),
        None => None,
    };
    // the AFK spot: `--afk`, or the site's default moved to the nearest legal spot (2 sections
    // from the shared bed's, the site rule against sieges), printed either way
    let (afk, afk_note) = match (afk_given, shared_bed) {
        (true, _) => (afk, format!("the AFK spot {} as given (--afk)", vat(afk))),
        (false, Some(bed)) if sec_dist(cell_of(afk), bed) < 2 => {
            let moved = legal_afk(&w, afk, bed);
            (moved, format!("the AFK spot moved from the default {} to {}, the nearest spot 2 sections from the shared bed's (--afk overrides)", vat(afk), vat(moved)))
        }
        (false, _) => (afk, format!("the AFK spot {} (the default)", vat(afk))),
    };
    let mut golem_cells: Vec<BlockPos> = Vec::new();
    for s in &positions_w {
        for (g, ok) in redsim::entity::mob::spawn::golem_spawn_cells_from(&w, *s) {
            if ok.is_ok() && !golem_cells.contains(&g) {
                golem_cells.push(g);
            }
        }
    }
    let mut interior: HashSet<BlockPos> = HashSet::new();
    for b in villagers.iter().copied().chain(carts.iter().copied()).chain(zombies.iter().flat_map(|z| z.boxes())) {
        interior.extend(cells_of(&b));
    }
    for g in &golem_cells {
        for dy in 0..3 {
            interior.insert(g.offset(0, dy, 0));
        }
    }
    for (q, b) in &moved.blocks {
        if !redsim::behavior::full_cube(b) {
            interior.insert(*q);
        }
    }
    let farm_blocks = moved.blocks.iter().map(|(p, _)| *p).collect();
    // the per-cell speeds of an empty cart (`--speeds`: design cells), moved with the farm
    let cart_speeds = speeds.map(|s| s.iter().map(|(q, v)| (mv(*q), *v)).collect());
    Ok(Placed { w, label, positions: positions_w, villagers, carts, zombies, tracks, farm_blocks, beds, shared_bed, golem_cells, portal, afk, afk_note, cart_speeds, interior, seed, extra_parts: Vec::new(), seeded_pois: Vec::new(), extra_items: Vec::new() })
}

// ------------------------------------------------------------------------- 1. lightning

/// Condition 1: a rod within 120 of every part, each its column's top; every villager's, cart's,
/// zombie's and boat's box outside each rod's reach (the bolt's damage box: x and z +-3, y -3..+9
/// of the block above the rod). The depth of every box under (or over) the reach of a strike on
/// the nearest strikeable surface prints as its own line, not a substitute.
pub fn lightning(p: &Placed) -> Verdict {
    let rods = redsim::weather::lightning::rods(&p.w);
    let mut lines = Vec::new();
    let boxes: Vec<(String, Aabb)> = p.villagers.iter().map(|b| ("a villager".to_string(), *b)).chain(p.carts.iter().map(|b| ("a cart".to_string(), *b))).chain(p.zombies.iter().flat_map(|z| std::iter::once(("a zombie".to_string(), z.bb)).chain(z.boat.map(|b| ("a boat".to_string(), b))))).collect();
    let parts: Vec<BlockPos> = p.farm_blocks.iter().chain(p.extra_parts.iter()).copied().collect();
    let d = |a: BlockPos, b: BlockPos| (((a.x - b.x).pow(2) + (a.y - b.y).pow(2) + (a.z - b.z).pow(2)) as f64).sqrt();
    if rods.is_empty() {
        lines.push((false, "rods: none tops its column (the brief: a rod within 120 of every part, each its column's top)".to_string()));
    } else {
        lines.push((true, format!("rods: {} topping their columns ({})", rods.len(), rods.iter().map(|r| at(*r)).collect::<Vec<_>>().join(" "))));
        let farthest = parts.iter().map(|q| (*q, rods.iter().map(|r| d(*q, *r)).fold(f64::INFINITY, f64::min))).max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((q, far)) = farthest {
            lines.push((far <= 120.0, format!("rod distance: the farthest part {:.0} blocks from a rod, {} (at most 120)", far, at(q))));
        }
        let mut hit = Vec::new();
        for r in &rods {
            let hb = redsim::weather::lightning::hit_box_at(Vec3::new(r.x as f64 + 0.5, r.y as f64 + 1.0, r.z as f64 + 0.5));
            for (what, b) in &boxes {
                if b.intersects(hb) {
                    hit.push(format!("{what} at {} by the rod at {}", vat(centre(b)), at(*r)));
                }
            }
        }
        lines.push((hit.is_empty(), if hit.is_empty() { "rod reach: no villager, cart, zombie or boat box in any rod's reach".to_string() } else { format!("rod reach: {} boxes in a rod's reach: {}", hit.len(), hit.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    }
    // the second line: each box against a strike on every column within 3 of it (the strike on
    // the column's top, `findLightningTargetAround`'s heightmap position)
    let mut worst: Option<(f64, String, i32, i32)> = None;
    for (what, b) in &boxes {
        let (x0, x1) = ((b.min_x - 3.5).ceil() as i32, (b.max_x + 2.5).floor() as i32);
        let (z0, z1) = ((b.min_z - 3.5).ceil() as i32, (b.max_z + 2.5).floor() as i32);
        for x in x0..=x1 {
            for z in z0..=z1 {
                let s = redsim::weather::motion_blocking_height(&p.w, x, z) as f64;
                // the clearance outside the bolt's box: under its bottom, or over its top
                let margin = (s - 3.0 - b.max_y).max(b.min_y - s - 9.0);
                if worst.as_ref().is_none_or(|w| margin < w.0) {
                    worst = Some((margin, format!("{what} at {}", vat(centre(b))), x, z));
                }
            }
        }
    }
    if let Some((m, what, x, z)) = worst {
        lines.push((true, format!("depth (the brief's second line): the closest box to a strike's reach is {what}, {:.2} blocks {} it (a strike on column {x},{z})", m.abs(), if m >= 0.0 { "outside" } else { "inside" })));
    }
    Verdict { lines }
}

// ------------------------------------------------------------------------ 4. village POIs

fn poi_kind(b: &Block) -> Option<&'static str> {
    if let Block::Bed { head, .. } = b {
        return head.then_some("bed");
    }
    let n = name_of(b);
    let n = n.trim_start_matches("minecraft:");
    let base = n.split('[').next().unwrap_or("");
    if base.ends_with("_bed") {
        return n.contains("part=head").then_some("bed");
    }
    if base == "bell" {
        return Some("bell");
    }
    const JOB: [&str; 16] = ["composter", "barrel", "lectern", "smoker", "blast_furnace", "grindstone", "loom", "stonecutter", "cartography_table", "fletching_table", "brewing_stand", "smithing_table", "cauldron", "water_cauldron", "lava_cauldron", "powder_snow_cauldron"];
    JOB.contains(&base).then_some("job site")
}

/// Every bed head, bell and job-site block of the placed world.
fn world_pois(w: &World) -> Vec<(&'static str, BlockPos)> {
    let mut out = Vec::new();
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let q = BlockPos::new(w.min.x + x, w.min.y + y, w.min.z + z);
                if let Some(k) = poi_kind(&w.get(q)) {
                    out.push((k, q));
                }
            }
        }
    }
    out
}

/// Condition 4: no bed other than the shared one, no job-site block and no bell within 8 sections
/// (Chebyshev, in section coordinates, all three axes) of any position a villager can occupy.
pub fn pois(p: &Placed) -> Verdict {
    let secs: HashSet<(i32, i32, i32)> = p.positions.iter().map(|q| sec(*q)).collect();
    let near = |q: BlockPos| {
        let (x, y, z) = sec(q);
        secs.iter().map(|(a, b, c)| (a - x).abs().max((b - y).abs()).max((c - z).abs())).min().unwrap_or(i32::MAX)
    };
    let mut found: Vec<(String, BlockPos, i32)> = Vec::new();
    for (k, q) in world_pois(&p.w) {
        if Some(q) != p.shared_bed && near(q) <= 8 {
            found.push((k.to_string(), q, near(q)));
        }
    }
    for (k, q) in &p.seeded_pois {
        if near(*q) <= 8 {
            found.push((format!("{k} (seeded)"), *q, near(*q)));
        }
    }
    found.sort_by_key(|f| f.2);
    let mut lines = vec![(
        true,
        match p.shared_bed {
            Some(b) => format!("excluded: the shared bed at {} (the excluded set: that bed alone)", at(b)),
            None => "excluded: none (no single bed and no `shared_bed:` line: every bed counts)".to_string(),
        },
    )];
    lines.push((found.is_empty(), if found.is_empty() { "POIs: no other bed, job-site block or bell within 8 sections of any villager position".to_string() } else { format!("POIs: {} within 8 sections: {}", found.len(), found.iter().take(5).map(|(k, q, d)| format!("{k} at {} ({d} sections)", at(*q))).collect::<Vec<_>>().join("; ")) }));
    Verdict { lines }
}

// ------------------------------------------------------------------- 5. the zombies stay

/// The cells a body can walk to from `starts` (a step, a 1.25 jump up, any fall), within
/// `bounds`. The starts count as given (a golem spawns where `checkSpawnObstruction` lets it, its
/// box tested against its column alone, so it can stand clipped into a wall it could not walk to).
fn walk_from(w: &World, b: &Body, starts: &[BlockPos], bounds: (BlockPos, BlockPos)) -> Vec<(BlockPos, f64)> {
    let (lo, hi) = bounds;
    let inside = |q: BlockPos| q.x >= lo.x && q.y >= lo.y.max(w.min.y + 1) && q.z >= lo.z && q.x <= hi.x && q.y <= hi.y && q.z <= hi.z && w.contains(q);
    let stand = |q: BlockPos| -> Option<f64> {
        let f = hostile::floor_in(w, q)?;
        hostile::fits(w, b, q, f).then_some(f)
    };
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut out = Vec::new();
    let mut queue = VecDeque::new();
    for s in starts {
        if inside(*s) && seen.insert(*s) {
            queue.push_back((*s, hostile::floor_in(w, *s).unwrap_or(s.y as f64)));
        }
    }
    while let Some((q, f)) = queue.pop_front() {
        out.push((q, f));
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [1, 0] {
                let n = q.offset(dx, dy, dz);
                if !inside(n) || seen.contains(&n) {
                    continue;
                }
                if let Some(nf) = stand(n)
                    && nf - f <= 1.25
                {
                    seen.insert(n);
                    queue.push_back((n, nf));
                    break;
                }
            }
            // a fall from the edge: down the column beside it
            let mut n = q.offset(dx, 0, dz);
            for _ in 0..24 {
                if !inside(n) || !hostile::fits(w, b, n, n.y as f64) {
                    break;
                }
                if let Some(nf) = stand(n) {
                    if seen.insert(n) {
                        queue.push_back((n, nf));
                    }
                    break;
                }
                n = n.below();
            }
        }
    }
    out
}

fn bounds_of(p: &Placed, margin: i32) -> (BlockPos, BlockPos) {
    let mut lo = BlockPos::new(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = BlockPos::new(i32::MIN, i32::MIN, i32::MIN);
    for q in p.farm_blocks.iter().chain(p.positions.iter()) {
        lo = BlockPos::new(lo.x.min(q.x - margin), lo.y.min(q.y - margin), lo.z.min(q.z - margin));
        hi = BlockPos::new(hi.x.max(q.x + margin), hi.y.max(q.y + margin), hi.z.max(q.z + margin));
    }
    (lo, hi)
}

/// Condition 5: every zombie never sees the sky in daylight, never despawns, is never attacked by
/// a golem and is never reached by water.
pub fn zombies_stay(p: &Placed) -> Verdict {
    let mut lines = Vec::new();
    if p.zombies.is_empty() {
        return Verdict { lines: vec![(true, "zombies: none in the farm".to_string())] };
    }
    let sky: Vec<String> = p.zombies.iter().filter(|z| redsim::weather::can_see_sky(&p.w, cell_of(z.eye())).unwrap_or(true)).map(|z| vat(z.eye())).collect();
    lines.push((sky.is_empty(), if sky.is_empty() { format!("sky: none of the {} zombies sees the sky from its eye cell", p.zombies.len()) } else { format!("sky: {} zombies see the sky from their eye cells (they burn in daylight): {}", sky.len(), sky.iter().take(4).cloned().collect::<Vec<_>>().join(" ")) }));
    let loose: Vec<String> = p.zombies.iter().filter(|z| !z.riding && !z.persistent).map(|z| vat(centre(&z.bb))).collect();
    lines.push((loose.is_empty(), if loose.is_empty() { format!("persistence: every zombie rides ({}) or is name-tagged or persistent ({}); the world never on Peaceful (the build guide)", p.zombies.iter().filter(|z| z.riding).count(), p.zombies.iter().filter(|z| !z.riding && z.persistent).count()) } else { format!("persistence: {} zombies neither ride nor carry a name tag (they despawn): {}", loose.len(), loose.iter().take(4).cloned().collect::<Vec<_>>().join(" ")) }));
    // water: none in or beside a post (the farm settled: its water has flowed)
    let wet: Vec<String> = p.zombies.iter().filter(|z| z.boxes().iter().any(|b| cells_of(&b.inflate(1.0, 0.0, 1.0)).iter().any(|c| p.w.contains(*c) && !redsim::fluid::fluid_state(&p.w, *c).is_empty()))).map(|z| vat(centre(&z.bb))).collect();
    lines.push((wet.is_empty(), if wet.is_empty() { "water: none in or beside any post (the settled farm)".to_string() } else { format!("water: in or beside {} posts (a zombie whose eye stays in water 600 gt converts): {}", wet.len(), wet.iter().take(4).cloned().collect::<Vec<_>>().join(" ")) }));
    // golems: from every cell a golem can walk to from its spawn cells, its attack box and a clear
    // line to a zombie
    let golems = walk_from(&p.w, &hostile::GOLEM, &p.golem_cells, bounds_of(p, 24));
    let mut hits = Vec::new();
    for (g, f) in &golems {
        let ab = hostile::reach_box(&hostile::GOLEM, *g, *f).inflate(REACH, 0.0, REACH);
        let eye = Vec3::new(g.x as f64 + 0.5, f + 2.295, g.z as f64 + 0.5);
        for z in &p.zombies {
            if z.boxes().iter().any(|b| b.intersects(ab)) && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, centre(&z.bb)) {
                hits.push(format!("a golem at {} hits the zombie at {}", at(*g), vat(centre(&z.bb))));
            }
        }
    }
    lines.push((hits.is_empty(), if hits.is_empty() { format!("golems: no golem position ({} walked from its {} spawn cells) reaches a zombie", golems.len(), p.golem_cells.len()) } else { format!("golems: {} golem positions reach a zombie: {}", hits.len(), hits.iter().take(3).cloned().collect::<Vec<_>>().join("; ")) }));
    Verdict { lines }
}

// ------------------------------------------------------------------ 3. empty-handed zombies

/// The cells an item can come to from `sources` (falling through cells it does not collide with,
/// carried by water both ways and up), within `bounds`.
fn item_reach(w: &World, sources: &[BlockPos], bounds: (BlockPos, BlockPos)) -> HashSet<BlockPos> {
    let (lo, hi) = bounds;
    let inside = |q: BlockPos| q.x >= lo.x && q.y >= lo.y && q.z >= lo.z && q.x <= hi.x && q.y <= hi.y && q.z <= hi.z && w.contains(q);
    let wet = |q: BlockPos| !redsim::fluid::fluid_state(w, q).is_empty();
    let open = |q: BlockPos| {
        let b = w.get(q);
        b.is_air() || !redsim::behavior::collides(&b) || wet(q)
    };
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut queue: VecDeque<BlockPos> = sources.iter().copied().filter(|q| inside(*q)).collect();
    seen.extend(queue.iter().copied());
    while let Some(q) = queue.pop_front() {
        let mut next = vec![q.below()];
        if wet(q) {
            next.extend([q.offset(1, 0, 0), q.offset(-1, 0, 0), q.offset(0, 0, 1), q.offset(0, 0, -1), q.above()]);
        }
        for n in next {
            if inside(n) && open(n) && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// Condition 3: every zombie holds nothing in either hand, and no item path ends within a post's
/// pickup box (its box inflated by 1, 0, 1) or on a tripwire string or a wooden or weighted plate
/// the farm relies on (stone plates ignore items).
pub fn empty_handed(p: &Placed) -> Verdict {
    let mut lines = Vec::new();
    let held: Vec<String> = p.zombies.iter().filter_map(|z| z.hands.as_ref().map(|h| format!("{} holds {h}", vat(centre(&z.bb))))).collect();
    lines.push((held.is_empty(), if held.is_empty() { format!("hands: every one of the {} zombies is empty-handed in the design", p.zombies.len()) } else { format!("hands: {} zombies hold something (a spear's stab dismounts its target): {}", held.len(), held.join("; ")) }));
    // item sources: the farm's containers' outputs (a hopper's or a dropper's facing cell), the
    // golem cells (a golem dies or drops there) and a control's items
    let mut sources: Vec<BlockPos> = Vec::new();
    for q in &p.farm_blocks {
        match p.w.get(*q) {
            Block::Hopper { facing, .. } => sources.push(q.rel(facing)),
            Block::Dispenser { facing, .. } => sources.push(q.rel(facing)),
            _ => {}
        }
    }
    sources.extend(p.golem_cells.iter().copied());
    sources.extend(p.extra_items.iter().copied());
    let reach = item_reach(&p.w, &sources, bounds_of(p, 24));
    let mut ends = Vec::new();
    for z in &p.zombies {
        if cells_of(&z.bb.inflate(1.0, 0.0, 1.0)).iter().any(|c| reach.contains(c)) {
            ends.push(format!("the post at {}", vat(centre(&z.bb))));
        }
    }
    let mut sensors = 0;
    for q in &p.farm_blocks {
        let b = p.w.get(*q);
        let sensitive = match b {
            Block::Tripwire { .. } => true,
            Block::Plate { kind, .. } => kind != 1,
            _ => {
                let n = name_of(&b);
                n.contains("_pressure_plate") && !n.contains("stone_pressure_plate") && !n.contains("blackstone_pressure_plate")
            }
        };
        if sensitive {
            sensors += 1;
            if reach.contains(q) {
                ends.push(format!("the {} at {}", name_of(&b).trim_start_matches("minecraft:").split('[').next().unwrap_or(""), at(*q)));
            }
        }
    }
    lines.push((ends.is_empty(), if ends.is_empty() { format!("items: no item path (from {} sources: container outputs, golem cells) ends at a post or at one of its {sensors} item-sensitive blocks (tripwire strings, wooden and weighted plates)", sources.len()) } else { format!("items: an item path ends at {}", ends.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    Verdict { lines }
}

// ------------------------------------------------------------------------ 2. natural mobs

/// A mob's position a channel found.
struct Hit {
    what: String,
    cell: BlockPos,
}

fn hits_line(name: &str, hits: &[Hit]) -> (bool, String) {
    if hits.is_empty() {
        return (true, format!("{name}: none"));
    }
    let mut kinds: Vec<String> = hits.iter().map(|h| h.what.clone()).collect();
    kinds.sort();
    kinds.dedup();
    (false, format!("{name}: {} positions ({}), e.g. {}", hits.len(), kinds.join(", "), hits.iter().take(3).map(|h| format!("{} at {}", h.what, at(h.cell))).collect::<Vec<_>>().join("; ")))
}

/// The floods of every body from every standable (or flyable) position outside the farm's space,
/// plus the mob cells a control injects.
fn floods(p: &Placed, extra: &[(Body, BlockPos)]) -> Vec<(Body, Vec<(BlockPos, f64)>)> {
    let bounds = bounds_of(p, 24);
    let bodies = [hostile::ZOMBIE, hostile::BABY_ZOMBIE, hostile::SKELETON, hostile::PILLAGER, hostile::CREEPER, hostile::SPIDER, hostile::CAVE_SPIDER, hostile::SLIME, hostile::FOX, hostile::VILLAGER, hostile::ENDERMAN, hostile::BAT];
    let mut out: Vec<(Body, Vec<(BlockPos, f64)>)> = bodies.iter().map(|b| (*b, hostile::flood(&p.w, *b, &p.interior, bounds).cells)).collect();
    for (b, q) in extra {
        let f = hostile::floor_in(&p.w, *q).unwrap_or(q.y as f64);
        match out.iter_mut().find(|(x, _)| x.name == b.name) {
            Some((_, cells)) => cells.push((*q, f)),
            None => out.push((*b, vec![(*q, f)])),
        }
    }
    out
}

/// Condition 2: no mob that can spawn naturally, come with a patrol or come through the farm's
/// own portal can harm a rider, a cart, a post or a boat, board an empty cart, block a golem spawn
/// cell, or make a rider flee; endermen; the napper's calm; the other spawners. `extra`: mob cells
/// a control injects.
pub fn natural_mobs(p: &Placed, extra: &[(Body, BlockPos)]) -> Verdict {
    natural_mobs_hits(p, extra).0
}

/// [`natural_mobs`] with each channel's mob cells (a control asks whether its mob is caught).
pub fn natural_mobs_hits(p: &Placed, extra: &[(Body, BlockPos)]) -> (Verdict, Vec<(&'static str, Vec<BlockPos>)>) {
    let fl = floods(p, extra);
    let cells = |name: &str| fl.iter().filter(|(b, _)| b.name == name).flat_map(|(b, c)| c.iter().map(move |x| (*b, x.0, x.1))).collect::<Vec<_>>();
    let mut lines = Vec::new();
    let posts: Vec<Aabb> = p.zombies.iter().flat_map(|z| z.boxes()).collect();
    let targets: Vec<Aabb> = p.villagers.iter().chain(p.carts.iter()).chain(posts.iter()).copied().collect();
    let villager_feet: Vec<Vec3> = p.positions.iter().map(|q| Vec3::new(q.x as f64 + 0.5, q.y as f64, q.z as f64 + 0.5)).collect();
    // reach: the zombie family's melee box against a villager's (walking, climbing and swimming
    // are the floods' moves; spiders and the rest do not attack villagers)
    let mut reach = Vec::new();
    for name in ["zombie", "baby zombie"] {
        for (b, c, f) in cells(name) {
            let ab = hostile::reach_box(&b, c, f).inflate(REACH, 0.0, REACH);
            if p.villagers.iter().any(|v| v.intersects(ab)) {
                reach.push(Hit { what: b.name.to_string(), cell: c });
            }
        }
    }
    // flee: a hostile-sensor type within 6 of a villager position (`SetWalkTargetAwayFrom` at 6)
    let mut flee = Vec::new();
    for name in ["zombie", "baby zombie", "pillager"] {
        for (b, c, f) in cells(name) {
            let feet = Vec3::new(c.x as f64 + 0.5, f, c.z as f64 + 0.5);
            if villager_feet.iter().any(|v| dist(*v, feet) <= 6.0) {
                flee.push(Hit { what: if name == "pillager" { "an illager".to_string() } else { b.name.to_string() }, cell: c });
            }
        }
    }
    // pickup: a pushable mob (not a bat, not an enderman out of reach) in an empty cart's pickup
    // box along a track (`pushAndPickupEntities`: the cart's box inflated 0.2 sideways)
    let pick_tracks: Vec<BlockPos> = match &p.cart_speeds {
        Some(s) => p.tracks.iter().copied().filter(|t| s.get(t).is_some_and(|v| *v >= 0.1)).collect(),
        None => p.tracks.clone(),
    };
    let pick_mode = match &p.cart_speeds {
        Some(_) => format!("from the runner trace: the {} of {} track cells an empty cart passes at 0.1 or faster", pick_tracks.len(), p.tracks.len()),
        None => format!("every one of the {} track cells (no runner trace)", p.tracks.len()),
    };
    let pick: Vec<Aabb> = pick_tracks.iter().map(|t| cart_box(*t).inflate(0.2, 0.0, 0.2)).collect();
    let mut pickup = Vec::new();
    for (b, cs) in &fl {
        if b.name == "bat" {
            continue;
        }
        for (c, f) in cs {
            let bb = hostile::box_at(b, *c, *f);
            if pick.iter().any(|k| k.intersects(bb)) {
                pickup.push(Hit { what: b.name.to_string(), cell: *c });
            }
        }
    }
    // golem spawn cells: any mob (bats too) meeting the golem's 1.4 x 2.7 box at a spawn cell
    let golem_boxes: Vec<Aabb> = p.golem_cells.iter().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64)).collect();
    let mut golem = Vec::new();
    for (b, cs) in &fl {
        for (c, f) in cs {
            let bb = hostile::box_at(b, *c, *f);
            if golem_boxes.iter().any(|g| g.intersects(bb)) {
                golem.push(Hit { what: b.name.to_string(), cell: *c });
            }
        }
    }
    // blasts: a creeper within 3 of the AFK player (it swells at its target) whose blast (6 for a
    // creeper's power 3) meets a rider, a cart, a boat, a post, or a pad's portal block (portal
    // blocks resist no blast: `Blocks.NETHER_PORTAL`'s `strength(-1)` leaves explosion resistance
    // 0, and one gone takes its plane with it, `NetherPortalBlock.updateShape`; then the golems
    // walk out of the pad)
    let portal_at: Vec<Vec3> = p.portal.iter().map(|q| Vec3::new(q.x as f64 + 0.5, q.y as f64 + 0.5, q.z as f64 + 0.5)).collect();
    let mut blast = Vec::new();
    for (_, c, f) in cells("creeper") {
        let feet = Vec3::new(c.x as f64 + 0.5, f, c.z as f64 + 0.5);
        if dist(feet, p.afk) > 3.0 {
            continue;
        }
        if targets.iter().any(|t| dist(centre(t), feet) <= 6.0) {
            blast.push(Hit { what: "creeper".to_string(), cell: c });
        } else if portal_at.iter().any(|q| dist(*q, feet) <= 6.0) {
            blast.push(Hit { what: "creeper by a portal".to_string(), cell: c });
        }
    }
    // ranged: in sight within the shooter's range (eye to the target's centre)
    let mut ranged = Vec::new();
    for (name, shooter, range) in [("skeleton", "a skeleton, stray or bogged", 16.0), ("pillager", "a pillager", 8.0), ("villager", "a witch", 10.0)] {
        for (b, c, f) in cells(name) {
            let eye = Vec3::new(c.x as f64 + 0.5, f + b.height * 0.85, c.z as f64 + 0.5);
            if targets.iter().any(|t| dist(eye, centre(t)) <= range && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, centre(t))) {
                ranged.push(Hit { what: shooter.to_string(), cell: c });
            }
        }
    }
    for (_, c, f) in cells("zombie") {
        if redsim::fluid::fluid_state(&p.w, c).is_empty() {
            continue;
        }
        let eye = Vec3::new(c.x as f64 + 0.5, f + 1.74, c.z as f64 + 0.5);
        if targets.iter().any(|t| dist(eye, centre(t)) <= 10.0 && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, centre(t))) {
            ranged.push(Hit { what: "a drowned".to_string(), cell: c });
        }
    }
    let mut channels: Vec<(&'static str, Vec<BlockPos>)> = Vec::new();
    for (name, hits) in [("reach", &reach), ("flee", &flee), ("pickup", &pickup), ("golem cells", &golem), ("blasts", &blast), ("ranged", &ranged)] {
        let (ok, mut l) = hits_line(name, hits);
        if name == "pickup" {
            l = format!("{l} [{pick_mode}]");
        }
        lines.push((ok, l));
        channels.push((name, hits.iter().map(|h| h.cell).collect()));
    }
    // the raiders the hostile sensor also takes, out of these channels: none spawns naturally here
    lines.push((true, "raiders: the ravager, the vex and the zoglin are not in the floods (no natural spawn in the overworld; a raid needs Bad Omen, which the site section has no line on yet)".to_string()));
    // endermen: a teleport onto a floor of the farm's space where its 2.9-tall box fits (within
    // +-32 sideways and -32..+31 up of an enderman's position); a block of the farm in the
    // `enderman_holdable` tag; a block left into an air cell the farm needs (within 1 sideways
    // and 0..1 up of where it stands)
    let enders: Vec<BlockPos> = cells("enderman").into_iter().map(|(_, c, _)| c).collect();
    let in_reach = |q: BlockPos| enders.iter().find(|e| (e.x - q.x).abs() <= 32 && (e.z - q.z).abs() <= 32 && q.y - e.y >= -32 && q.y - e.y <= 31).copied();
    let mut tele: Vec<(BlockPos, BlockPos)> = Vec::new();
    for q in &p.interior {
        if !p.w.get(*q).is_air() {
            continue;
        }
        let Some(f) = hostile::floor_in(&p.w, *q) else { continue };
        if !hostile::fits(&p.w, &hostile::ENDERMAN, *q, f) {
            continue;
        }
        if let Some(e) = in_reach(*q) {
            tele.push((*q, e));
        }
    }
    tele.sort_by_key(|(q, _)| (q.x, q.y, q.z));
    lines.push((tele.is_empty(), if tele.is_empty() { format!("enderman teleports: no floor of the farm's space (the golem cells among them) fits its 2.9-tall box within teleport reach of its {} positions", enders.len()) } else { format!("enderman teleports: {} floors of the farm's space fit it within reach, e.g. {}", tele.len(), tele.iter().take(3).map(|(q, e)| format!("{} (from {})", at(*q), at(*e))).collect::<Vec<_>>().join("; ")) }));
    channels.push(("enderman teleports", tele.iter().map(|(q, _)| *q).collect()));
    let holdable: Vec<BlockPos> = p.farm_blocks.iter().copied().filter(|q| hostile::holdable(&p.w, *q)).collect();
    lines.push((holdable.is_empty(), if holdable.is_empty() { "enderman holdable: no block of the farm in the enderman_holdable tag".to_string() } else { format!("enderman holdable: {} farm blocks in the tag, e.g. {}", holdable.len(), holdable.iter().take(3).map(|q| at(*q)).collect::<Vec<_>>().join(" ")) }));
    channels.push(("enderman holdable", holdable.clone()));
    let needed: HashSet<BlockPos> = p.tracks.iter().copied().chain(p.golem_cells.iter().copied()).chain(p.villagers.iter().flat_map(cells_of)).chain(posts.iter().flat_map(cells_of)).collect();
    let mut leave: Vec<BlockPos> = Vec::new();
    for e in &enders {
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in 0..=1 {
                    let q = e.offset(dx, dy, dz);
                    if needed.contains(&q) && p.w.get(q).is_air() {
                        leave.push(q);
                    }
                }
            }
        }
    }
    leave.sort_by_key(|q| (q.x, q.y, q.z));
    leave.dedup();
    lines.push((leave.is_empty(), if leave.is_empty() { "enderman leaves a block: no air cell the farm needs within 1 sideways and 0..1 up of an enderman's position".to_string() } else { format!("enderman leaves a block: {} air cells the farm needs, e.g. {}", leave.len(), leave.iter().take(3).map(|q| at(*q)).collect::<Vec<_>>().join(" ")) }));
    channels.push(("enderman leaves a block", leave));
    // the napper's calm: no hostile-sensor type within its panic distance and in sight of the bed
    // or the cells around it (the walk, the stand-up cell: the villager positions within 4 of the
    // bed): `VillagerHostilesSensor`, 8 for the zombie family (zombie, husk, drowned, zombie
    // villager; vex 8), 15 for a pillager (the illagers' body: vindicator 10, evoker and
    // illusioner 12 inside it; ravager 12 and zoglin 10 have no natural spawn here)
    if let Some(bed) = p.shared_bed {
        let calm: Vec<Vec3> = std::iter::once(Vec3::new(bed.x as f64 + 0.5, bed.y as f64 + 0.6, bed.z as f64 + 0.5)).chain(p.positions.iter().filter(|q| (q.x - bed.x).abs() <= 4 && (q.z - bed.z).abs() <= 4 && (q.y - bed.y).abs() <= 2).map(|q| Vec3::new(q.x as f64 + 0.5, q.y as f64 + 1.62, q.z as f64 + 0.5))).collect();
        let mut seen_by = Vec::new();
        for (name, range) in [("zombie", 8.0), ("baby zombie", 8.0), ("pillager", 15.0)] {
            for (b, c, f) in cells(name) {
                let eye = Vec3::new(c.x as f64 + 0.5, f + b.height * 0.85, c.z as f64 + 0.5);
                if calm.iter().any(|t| dist(eye, *t) <= range && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, *t)) {
                    seen_by.push(Hit { what: if name == "pillager" { "an illager".to_string() } else { b.name.to_string() }, cell: c });
                }
            }
        }
        lines.push(hits_line("napper's calm", &seen_by));
        channels.push(("napper's calm", seen_by.iter().map(|h| h.cell).collect()));
    }
    // the other spawners
    let sea = 63;
    let afk_cell = cell_of(p.afk);
    let phantom = p.afk.y >= sea as f64 && redsim::weather::can_see_sky(&p.w, afk_cell).unwrap_or(true);
    lines.push((!phantom, if phantom { format!("phantoms: the AFK cell {} sees the sky at y >= {sea}", at(afk_cell)) } else { format!("phantoms: none (the AFK cell {} {})", at(afk_cell), if p.afk.y < sea as f64 { "is below sea level" } else { "is under a roof" }) }));
    if let Some(bed) = p.shared_bed {
        let d = sec_dist(afk_cell, bed);
        lines.push((d >= 2, format!("siege: the AFK spot is {d} sections from the shared bed's (a siege needs it within 1; the brief asks 2 or more)")));
    }
    let beds_near = world_pois(&p.w).iter().filter(|(k, q)| *k == "bed" && (q.x - afk_cell.x).abs() <= 79 && (q.z - afk_cell.z).abs() <= 79).count();
    lines.push((beds_near <= 4, format!("cats: {beds_near} beds within reach of a cat's spawn around the AFK player (a village's cats need more than 4; no swamp hut in the samples)")));
    if !p.portal.is_empty() {
        // the portal's zombified piglins: on the first block under a portal column, if it is a
        // valid floor; then walking (a zombie's body) to a track, a rider, a post or a golem cell
        let mut bases: Vec<BlockPos> = Vec::new();
        for q in &p.portal {
            let mut b = *q;
            while p.portal.contains(&b) {
                b = b.below();
            }
            if hostile::floor_in(&p.w, b.above()).is_some() && !bases.contains(&b.above()) {
                bases.push(b.above());
            }
        }
        let walked = walk_from(&p.w, &hostile::ZOMBIE, &bases, bounds_of(p, 24));
        let reached: Vec<Hit> = walked.iter().filter(|(c, f)| {
            let bb = hostile::box_at(&hostile::ZOMBIE, *c, *f);
            p.villagers.iter().chain(pick.iter()).chain(posts.iter()).chain(golem_boxes.iter()).any(|t| t.intersects(bb))
        }).map(|(c, _)| Hit { what: "a zombified piglin".to_string(), cell: *c }).collect();
        let (ok, l) = hits_line(&format!("portal piglins (from its {} base cells, {} positions walked)", bases.len(), walked.len()), &reached);
        lines.push((ok, l));
    }
    let village = hostile::sections_to_village(&p.w, afk_cell, &p.beds);
    if village > 2 {
        // a patrol's pillagers (`PatrolSpawner`): within a pillager's panic distance (15) and in
        // sight of any villager position
        let eyes: Vec<Vec3> = p.positions.iter().map(|q| Vec3::new(q.x as f64 + 0.5, q.y as f64 + 1.62, q.z as f64 + 0.5)).collect();
        let panic: Vec<Hit> = cells("pillager").into_iter().filter(|(b, c, f)| {
            let eye = Vec3::new(c.x as f64 + 0.5, f + b.height * 0.85, c.z as f64 + 0.5);
            eyes.iter().any(|t| dist(eye, *t) <= 15.0 && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, *t))
        }).map(|(_, c, _)| Hit { what: "a patrol's pillager".to_string(), cell: c }).collect();
        lines.push(hits_line(&format!("patrols (can spawn: the AFK spot is {} sections from a village; a pillager within 15 and in sight of a villager position)", if village >= 7 { "7 or more".to_string() } else { village.to_string() }), &panic));
    } else {
        lines.push((true, format!("patrols: none (the AFK spot is {village} sections from a village)")));
    }
    let slime = p.seed.map(|s| {
        let mut chunks: Vec<(i32, i32)> = p.farm_blocks.iter().filter(|q| q.y < 40).map(|q| (q.x.div_euclid(16), q.z.div_euclid(16))).collect();
        chunks.sort();
        chunks.dedup();
        chunks.iter().filter(|(x, z)| redsim::entity::mob::natural::slime_chunk(*x, *z, s)).count()
    });
    lines.push((true, format!("sources: slime chunks under the farm below y 40: {} (slimes spawn there at any light; the floods ignore light); wandering traders, llamas, bats and slimes are in the floods", slime.map(|n| n.to_string()).unwrap_or_else(|| "none (the void)".to_string()))));
    (Verdict { lines }, channels)
}

// ------------------------------------------------------------------------- the controls

/// A control: what it changes, and whether it bites (the line it aims at fails where the placed
/// farm's own did not, or fails with more).
pub struct Control {
    pub name: String,
    pub bites: bool,
    pub why: String,
}

fn control(name: &str, prefix: &str, base: &Verdict, v: Verdict) -> Control {
    let now = v.failing(prefix).map(str::to_string);
    let before = base.failing(prefix).map(str::to_string);
    let bites = now.is_some() && now != before;
    Control { name: name.to_string(), bites, why: now.unwrap_or_else(|| format!("{prefix}: passes")) }
}

/// An open cell near a target box (air, a floor under it, the body fitting), the closest first
/// within `within` of the box's centre: where a control puts a mob.
fn beside(p: &Placed, b: &Body, target: &Aabb, within: f64) -> Option<BlockPos> {
    let c = centre(target);
    let mut best: Option<(f64, BlockPos)> = None;
    let r = within.ceil() as i32 + 1;
    for dy in -r..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                let q = BlockPos::new(c.x.floor() as i32 + dx, target.min_y.floor() as i32 + dy, c.z.floor() as i32 + dz);
                if !p.w.contains(q) || !p.w.get(q).is_air() {
                    continue;
                }
                let f = if b.flies { q.y as f64 } else { match hostile::floor_in(&p.w, q) { Some(f) => f, None => continue } };
                if !hostile::fits(&p.w, b, q, f) {
                    continue;
                }
                let d = dist(Vec3::new(q.x as f64 + 0.5, f, q.z as f64 + 0.5), c);
                if d <= within && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
        }
    }
    best.map(|x| x.1)
}

/// An open cell where a body's box meets `target` (air, a floor under it unless it flies, the
/// body fitting): where a control puts a mob that must touch a box (a cart's pickup box, a golem
/// spawn cell's box).
fn touching(p: &Placed, b: &Body, target: &Aabb) -> Option<BlockPos> {
    let c = centre(target);
    let mut best: Option<(f64, BlockPos)> = None;
    for dy in -3..=3 {
        for dz in -3..=3 {
            for dx in -3..=3 {
                let q = BlockPos::new(c.x.floor() as i32 + dx, target.min_y.floor() as i32 + dy, c.z.floor() as i32 + dz);
                // (an open cell: air, or a block a mob walks through, a rail among them)
                if !p.w.contains(q) || redsim::behavior::collides(&p.w.get(q)) {
                    continue;
                }
                let f = if b.flies { q.y as f64 } else { match hostile::floor_in(&p.w, q) { Some(f) => f, None => continue } };
                if !hostile::fits(&p.w, b, q, f) || !hostile::box_at(b, q, f).intersects(*target) {
                    continue;
                }
                let d = dist(Vec3::new(q.x as f64 + 0.5, f, q.z as f64 + 0.5), c);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
        }
    }
    best.map(|x| x.1)
}

/// Every condition's positive controls, each on a copy of the placed farm with one change.
pub fn controls(p: &Placed) -> Vec<(u8, Control)> {
    let mut out = Vec::new();
    let stone = Block::parse("minecraft:stone").expect("stone");
    let rod_block = Block::parse("minecraft:lightning_rod[facing=up]").expect("rod");
    let air = Block::Air;
    let top = p.w.min.y + p.w.size[1] - 1;
    // 1. lightning: a box in a rod's reach; a covered rod; a part more than 120 from every rod
    let base1 = lightning(p);
    if let Some(v) = p.villagers.first() {
        let mut q = p.clone();
        let c = centre(v);
        let rod = BlockPos::new(c.x.floor() as i32, v.max_y.ceil() as i32 + 1, c.z.floor() as i32);
        for y in rod.y + 1..=top {
            q.w.place_raw(&[(BlockPos::new(rod.x, y, rod.z), air)]);
        }
        q.w.place_raw(&[(rod, rod_block)]);
        out.push((1, control(&format!("a rod at {} over the villager at {}", at(rod), vat(c)), "rod reach", &base1, lightning(&q))));
    }
    if let Some(r) = redsim::weather::lightning::rods(&p.w).first() {
        let mut q = p.clone();
        q.w.place_raw(&[(r.above(), stone)]);
        out.push((1, control(&format!("the rod at {} covered by a block", at(*r)), "rods", &base1, lightning(&q))));
        let mut q = p.clone();
        q.extra_parts.push(r.offset(121, 0, 0));
        out.push((1, control("a part 121 blocks from the rod", "rod distance", &base1, lightning(&q))));
    }
    // 2. natural mobs: one mob put by a target, for each target; it bites when the channel it
    // aims at catches that mob (the placed farm's own line may already fail on other mobs)
    let caught = |q: &Placed, what: &str, prefix: &'static str, b: Body, cell: BlockPos| -> Control {
        let (v, ch) = natural_mobs_hits(q, &[(b, cell)]);
        let bites = ch.iter().any(|(n, cells)| *n == prefix && cells.contains(&cell));
        Control { name: format!("{what} at {}", at(cell)), bites, why: v.failing(prefix).map(str::to_string).unwrap_or_else(|| format!("{prefix}: passes")) }
    };
    let golem_box = p.golem_cells.first().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64));
    let fast = |t: &BlockPos| p.cart_speeds.as_ref().is_none_or(|s| s.get(t).is_some_and(|v| *v >= 0.1));
    let pick_box = p.tracks.iter().filter(|t| fast(t)).map(|t| cart_box(*t).inflate(0.2, 0.0, 0.2)).find(|k| touching(p, &hostile::SLIME, k).is_some());
    let tries: [(&str, &'static str, Body, Option<&Aabb>, f64); 3] = [
        ("a zombie beside a rider", "reach", hostile::ZOMBIE, p.villagers.first(), 1.6),
        ("a zombie 5 blocks from a rider", "flee", hostile::ZOMBIE, p.villagers.first(), 5.5),
        ("a skeleton in sight of a cart", "ranged", hostile::SKELETON, p.carts.first(), 8.0),
    ];
    for (what, prefix, b, target, within) in tries {
        if let Some(t) = target
            && let Some(cell) = beside(p, &b, t, within)
        {
            out.push((2, caught(p, what, prefix, b, cell)));
        }
    }
    let touches: [(&str, &'static str, Body, Option<&Aabb>); 3] = [
        ("a spider in an empty cart's pickup box", "pickup", hostile::SPIDER, pick_box.as_ref()),
        ("a slime in an empty cart's pickup box", "pickup", hostile::SLIME, pick_box.as_ref()),
        ("a bat on a golem spawn cell", "golem cells", hostile::BAT, golem_box.as_ref()),
    ];
    for (what, prefix, b, target) in touches {
        if let Some(t) = target
            && let Some(cell) = touching(p, &b, t)
        {
            out.push((2, caught(p, what, prefix, b, cell)));
        }
    }
    if let Some(cart) = p.carts.first() {
        let mut q = p.clone();
        q.afk = centre(cart);
        if let Some(cell) = beside(&q, &hostile::CREEPER, cart, 2.5) {
            out.push((2, caught(&q, "a creeper by the AFK player beside a cart", "blasts", hostile::CREEPER, cell)));
        }
    }
    if let Some(portal) = p.portal.first() {
        // a creeper within blast range of a pad's portal (the AFK player beside it: a creeper
        // swells at its target)
        let pb = Aabb::new(portal.x as f64, portal.y as f64, portal.z as f64, portal.x as f64 + 1.0, portal.y as f64 + 1.0, portal.z as f64 + 1.0);
        let mut q = p.clone();
        if let Some(cell) = beside(&q, &hostile::CREEPER, &pb, 5.5) {
            q.afk = Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5);
            // (no rider, cart, boat or post within its blast: the portal alone is the target)
            q.villagers.clear();
            q.carts.clear();
            q.zombies.clear();
            out.push((2, caught(&q, &format!("a creeper by the pad's portal block at {}", at(*portal)), "blasts", hostile::CREEPER, cell)));
        }
    }
    if let Some(bed) = p.shared_bed
        && let Some(cell) = beside(p, &hostile::ZOMBIE, &Aabb::new(bed.x as f64, bed.y as f64, bed.z as f64, bed.x as f64 + 1.0, bed.y as f64 + 0.6, bed.z as f64 + 1.0), 6.0)
    {
        out.push((2, caught(p, "a zombie in sight of the bed", "napper's calm", hostile::ZOMBIE, cell)));
    }
    // endermen: a 3-tall space opened in the farm's space within teleport reach of an enderman's
    // position (its floor must be caught); a holdable block (dirt) in the first post's wall
    let ender_floor = p.interior.iter().copied().filter(|q| p.w.get(*q).is_air() && hostile::floor_in(&p.w, *q).is_some()).min_by_key(|q| (q.x, q.y, q.z));
    if let Some(q0) = ender_floor {
        let mut q = p.clone();
        for dy in 1..=3 {
            q.w.place_raw(&[(q0.offset(0, dy, 0), air)]);
        }
        // (an enderman's position within its reach: the cell beside the opened space)
        let spot = q0.offset(0, 0, 0);
        let (v, ch) = natural_mobs_hits(&q, &[(hostile::ENDERMAN, spot)]);
        let bites = ch.iter().any(|(n, cells)| *n == "enderman teleports" && cells.contains(&q0));
        out.push((2, Control { name: format!("a 3-tall space opened at {} in reach of an enderman", at(q0)), bites, why: v.failing("enderman teleports").map(str::to_string).unwrap_or_else(|| "enderman teleports: passes".to_string()) }));
    }
    if let Some(z) = p.zombies.first() {
        let wall = cells_of(&z.bb.inflate(1.0, 0.0, 1.0)).into_iter().find(|c| p.farm_blocks.contains(c) && redsim::behavior::collides(&p.w.get(*c)));
        if let Some(wall) = wall {
            let mut q = p.clone();
            let dirt = Block::parse("minecraft:dirt").expect("dirt");
            q.w.place_raw(&[(wall, dirt)]);
            let (v, ch) = natural_mobs_hits(&q, &[]);
            let bites = ch.iter().any(|(n, cells)| *n == "enderman holdable" && cells.contains(&wall));
            out.push((2, Control { name: format!("dirt (holdable) in the first post's wall at {}", at(wall)), bites, why: v.failing("enderman holdable").map(str::to_string).unwrap_or_else(|| "enderman holdable: passes".to_string()) }));
        }
    }
    // 3. a held item; an item path ending at a post
    let base3 = empty_handed(p);
    if let Some(z) = p.zombies.first() {
        let mut q = p.clone();
        q.zombies[0].hands = Some("equipment:{mainhand:{id:\"minecraft:iron_spear\"".to_string());
        out.push((3, control("the first zombie holding a spear", "hands", &base3, empty_handed(&q))));
        let mut q = p.clone();
        let over = BlockPos::new(centre(&z.bb).x.floor() as i32 + 1, z.bb.max_y.ceil() as i32, centre(&z.bb).z.floor() as i32);
        q.extra_items.push(over);
        out.push((3, control(&format!("an item dropped at {} over the first post", at(over)), "items", &base3, empty_handed(&q))));
    }
    // an item path ending on an item-sensitive block of the farm (the bed stop's FC string, on a
    // design that carries it: the first tripwire string, else the first wooden or weighted plate)
    let sensor = p.farm_blocks.iter().copied().find(|q| matches!(p.w.get(*q), Block::Tripwire { .. })).or_else(|| p.farm_blocks.iter().copied().find(|q| matches!(p.w.get(*q), Block::Plate { kind, .. } if kind != 1)));
    if let Some(s) = sensor {
        let mut q = p.clone();
        q.extra_items.push(s.above());
        out.push((3, control(&format!("an item dropped at {} onto the {} at {}", at(s.above()), name_of(&p.w.get(s)).trim_start_matches("minecraft:").split('[').next().unwrap_or(""), at(s)), "items", &base3, empty_handed(&q))));
    }
    // 4. a second bed seeded 7 sections out
    let base4 = pois(p);
    if let Some(v) = p.positions.first() {
        let mut q = p.clone();
        q.seeded_pois.push(("bed".to_string(), v.offset(7 * 16, 0, 0)));
        out.push((4, control("a second bed 7 sections out", "POIs", &base4, pois(&q))));
    }
    // 5. a post opened to the sky; a zombie out of its boat with no name tag; a post opened to
    // the golem route; a water path to a post
    let base5 = zombies_stay(p);
    if let Some(z) = p.zombies.first() {
        let e = cell_of(z.eye());
        let mut q = p.clone();
        for y in e.y + 1..=top {
            q.w.place_raw(&[(BlockPos::new(e.x, y, e.z), air)]);
        }
        out.push((5, control("the first post opened to the sky", "sky", &base5, zombies_stay(&q))));
        let mut q = p.clone();
        q.zombies[0].riding = false;
        q.zombies[0].persistent = false;
        out.push((5, control("the first zombie out of its boat, no name tag", "persistence", &base5, zombies_stay(&q))));
        // a golem's route by the post: a golem standing in the air cell beside the zombie (as a
        // spawn cell counts, its box against its column alone)
        let zc = cell_of(centre(&z.bb));
        if let Some(cell) = [zc.offset(1, 0, 0), zc.offset(-1, 0, 0), zc.offset(0, 0, 1), zc.offset(0, 0, -1)].into_iter().find(|c| p.w.contains(*c) && p.w.get(*c).is_air() && redsim::entity::mob::los::has_line_of_sight(&p.w, Vec3::new(c.x as f64 + 0.5, c.y as f64 + 2.295, c.z as f64 + 0.5), centre(&z.bb))) {
            let mut q = p.clone();
            q.golem_cells.push(cell);
            out.push((5, control(&format!("a golem's route at {} beside the first post", at(cell)), "golems", &base5, zombies_stay(&q))));
        }
        let mut q = p.clone();
        let wet = BlockPos::new(centre(&z.bb).x.floor() as i32 + 1, z.bb.min_y.floor() as i32, centre(&z.bb).z.floor() as i32);
        let water = Block::parse("minecraft:water").expect("water");
        q.w.place_raw(&[(wet, water)]);
        out.push((5, control(&format!("water at {} beside the first post", at(wet)), "water", &base5, zombies_stay(&q))));
    }
    out
}

/// The whole report: the five verdicts, then the controls (each must bite).
pub fn report(p: &Placed, with_controls: bool) -> (String, bool) {
    let mut s = format!("site-check {}: {} villager positions, {} tracks, {} zombies, {} golem spawn cells, {} portal blocks (the world {} to {}); {}\n", p.label, p.positions.len(), p.tracks.len(), p.zombies.len(), p.golem_cells.len(), p.portal.len(), at(p.w.min), at(p.w.min.offset(p.w.size[0] - 1, p.w.size[1] - 1, p.w.size[2] - 1)), p.afk_note);
    let verdicts = [("1 lightning", lightning(p)), ("2 natural mobs", natural_mobs(p, &[])), ("3 empty-handed zombies", empty_handed(p)), ("4 no village POI within 8 sections", pois(p)), ("5 the farm's zombies stay", zombies_stay(p))];
    let mut all = true;
    for (name, v) in &verdicts {
        all &= v.pass();
        s.push_str(&format!("{name}: {}\n", if v.pass() { "PASS" } else { "FAIL" }));
        for (ok, l) in &v.lines {
            s.push_str(&format!("    {} {l}\n", if *ok { "ok  " } else { "FAIL" }));
        }
    }
    let mut controls_ok = true;
    if with_controls {
        s.push_str("positive controls (each must bite: its line fails where the placed farm's did not, or fails with more):\n");
        for (n, c) in controls(p) {
            controls_ok &= c.bites;
            s.push_str(&format!("    {n}: {}: {} ({})\n", c.name, if c.bites { "bites" } else { "DOES NOT BITE" }, c.why));
        }
    }
    (s, all && controls_ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every condition's positive controls bite on the carousel's assembly v0a in the void (with
    /// a rod over its middle, the site guide's, so the rod controls run): each of the five
    /// conditions has controls, and every one fails the line it aims at.
    #[test]
    fn every_site_check_control_bites() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/carousel-assembly/carousel-v0a.txt");
        let c = circuit::Circuit::load(&path).unwrap();
        let spec = circuit::mobfarm::spec_of(&c).unwrap();
        let p = place(&c, &spec, None, &[], true, &[(5, 10)], None, None).unwrap();
        // (the default AFK spot is the nearest legal one: 2 sections from the shared bed's)
        let bed = p.shared_bed.expect("v0a's single bed");
        assert!(sec_dist(cell_of(p.afk), bed) >= 2, "{}", p.afk_note);
        assert!(lightning(&p).pass(), "the rod over the middle keeps every box out of its reach");
        let cs = controls(&p);
        for n in 1..=5u8 {
            assert!(cs.iter().any(|(k, _)| *k == n), "condition {n} has controls");
        }
        let idle: Vec<String> = cs.iter().filter(|(_, c)| !c.bites).map(|(n, c)| format!("{n}: {} ({})", c.name, c.why)).collect();
        assert!(idle.is_empty(), "controls that do not bite: {idle:?}");
        // pickup from a runner trace: only the cells an empty cart passes at 0.1 or faster count;
        // every track slow, no pickup at all
        let slow: Vec<(BlockPos, f64)> = p.tracks.iter().map(|t| (*t, 0.05)).collect();
        let q = place(&c, &spec, None, &[], true, &[(5, 10)], None, Some(&slow)).unwrap();
        let (v, ch) = natural_mobs_hits(&q, &[]);
        assert!(ch.iter().any(|(n, cells)| *n == "pickup" && cells.is_empty()), "every track slow: no pickup");
        assert!(v.lines.iter().any(|(_, l)| l.starts_with("pickup") && l.contains("from the runner trace: the 0 of")), "the mode is printed");
    }

    #[test]
    fn village_pois_and_section_distance() {
        assert_eq!(poi_kind(&Block::parse("minecraft:red_bed[facing=north,occupied=false,part=head]").unwrap()), Some("bed"));
        assert_eq!(poi_kind(&Block::parse("minecraft:red_bed[facing=north,occupied=false,part=foot]").unwrap()), None);
        assert_eq!(poi_kind(&Block::parse("minecraft:stone").unwrap()), None);
        // (Chebyshev in sections, all three axes: 7 sections out on x alone is 7)
        assert_eq!(sec_dist(BlockPos::new(0, 0, 0), BlockPos::new(7 * 16, 0, 0)), 7);
        assert_eq!(sec_dist(BlockPos::new(-1, 0, 0), BlockPos::new(0, 0, 0)), 1);
        assert_eq!(sec_dist(BlockPos::new(15, 15, 15), BlockPos::new(0, 0, 0)), 0);
    }
}
