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
    /// A control's additions: POIs seeded around the farm; item sources.
    pub seeded_pois: Vec<(String, BlockPos)>,
    pub extra_items: Vec<BlockPos>,
    /// A dug-in farm's walls the natural rock gives (world cells, the natural block there).
    pub kept_walls: Vec<(BlockPos, Block)>,
    /// A dug-in farm's dug box (world cells), and whether its AFK spot is dug in.
    pub dig: Option<(BlockPos, BlockPos)>,
    pub afk_dug: bool,
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
    pub(crate) fn boxes(&self) -> Vec<Aabb> {
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

/// The AFK player's box at feet `afk` (0.6 x 1.8).
fn player_box(afk: Vec3) -> Aabb {
    Aabb::new(afk.x - 0.3, afk.y, afk.z - 0.3, afk.x + 0.3, afk.y + 1.8, afk.z + 0.3)
}

/// Every golem spawn cell the summon search finds from the villager cells `positions` (the game's
/// search, `golem_spawn_cells_from`: 8 sideways, from 6 above down to 6 below, a floor and room).
fn golem_cells_of(w: &World, positions: &[BlockPos]) -> Vec<BlockPos> {
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut out = Vec::new();
    for s in positions {
        for (g, ok) in redsim::entity::mob::spawn::golem_spawn_cells_from(w, *s) {
            if ok.is_ok() && seen.insert(g) {
                out.push(g);
            }
        }
    }
    out
}

/// The cells of the villagers' summon bands (each position's 8 sideways and 6 up or down, and the
/// golem's 3 cells over the band's top: the union of every position's own band, ironspec
/// 2026-11-11), outside the farm's box `lo..hi` (design coordinates).
fn band_cells(positions: &[BlockPos], lo: BlockPos, hi: BlockPos) -> Vec<BlockPos> {
    let mut seen: HashSet<BlockPos> = HashSet::new();
    for v in positions {
        for y in v.y - 6..=v.y + 8 {
            for z in v.z - 8..=v.z + 8 {
                for x in v.x - 8..=v.x + 8 {
                    if x >= lo.x && x <= hi.x && y >= lo.y && y <= hi.y && z >= lo.z && z <= hi.z {
                        continue;
                    }
                    seen.insert(BlockPos::new(x, y, z));
                }
            }
        }
    }
    seen.into_iter().collect()
}

/// The natural air cells among the band cells ([`band_cells`], moved by `off` into the world):
/// a cave there is an off-pad golem spot, filled (ironspec 2026-11-11).
fn band_fills(w: &World, cells: &[BlockPos], off: [i32; 3]) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for c in cells {
        let q = c.offset(off[0], off[1], off[2]);
        if w.contains(q) && redsim::terrain::place::air_like(&w.get(q)) && redsim::fluid::fluid_state(w, q).is_empty() {
            out.push(q);
        }
    }
    out.sort_by_key(|q| (q.y, q.z, q.x));
    out
}

/// What a dug-in AFK spot costs: its feet's depth under the lasting surface over it, the blocks
/// its cell and its 1-wide ladder shaft dig and place, the cells sealed around it, and the same at
/// the old depth (6 under) for comparison (the parent's ask, ironspec 2026-11-11).
#[derive(Clone, Copy, Debug)]
pub struct AfkCost {
    pub depth: i32,
    pub dug: u32,
    pub placed: u32,
    pub seals: u32,
    pub minutes: f64,
    pub dug6: u32,
    pub placed6: u32,
    pub minutes6: f64,
}

/// A dug-in farm's AFK spot (ironspec 2026-11-11: under the parent's rule the AFK spot is dug in
/// too): the nearest column 2 sections from the shared bed's (as [`legal_afk`]), 2 or more blocks
/// outside the farm's box (`farm`, world cells: the player stays out of the farm's own space) and
/// out of every mob spawner's range; the feet deep enough that no skeleton trap fires (ruled Nov
/// 11: more than 10 from every cell a strike can land on, on the lasting tops of the columns
/// within 10; `SkeletonTrapGoal` fires on a player within 10 of the horse), which also puts the
/// player's box 1 block out of every strike's reach (rule (b)); its two cells dug, a floor under
/// them, its roof never a falling block and nothing open or fluid beside them (each sealed with
/// cobblestone: G1). Priced with its 1-wide ladder shaft up to the surface (too narrow for a
/// horse). Fails when no column on the four axes qualifies.
fn dug_in_afk(w: &mut World, afk: Vec3, bed: Option<BlockPos>, farm: (BlockPos, BlockPos), positions: &[BlockPos]) -> Result<(Vec3, AfkCost), String> {
    // (the AFK route's every dug air column out of every villager's summon band: a golem needs only
    // a 1 x 1 column of 3 empty cells over a floor, `IronGolem.checkSpawnObstruction`, so a dug AFK
    // cell or shaft inside a band is a golem spot; the search's band: 8 sideways, from 6 above to 6
    // below each villager; ironspec 2026-11-11: the shaft, the cell and the room alike)
    let in_band = |c: BlockPos, feet_y: i32, top_y: i32| positions.iter().any(|v| (c.x - v.x).abs() <= 8 && (c.z - v.z).abs() <= 8 && feet_y <= v.y + 6 && top_y >= v.y - 6);
    let ok = |q: BlockPos| bed.is_none_or(|b| sec_dist(q, b) >= 2) && (q.x < farm.0.x - 2 || q.x > farm.1.x + 2 || q.z < farm.0.z - 2 || q.z > farm.1.z + 2);
    // the feet under a column: more than 10 from every lasting strike cell within 10 sideways,
    // measured to the nearest point of the strike column (G1: a horse stands anywhere in its cell;
    // the sample's columns only: one outside its box has no ground to read)
    let feet_at = |w: &World, c: BlockPos| -> BlockPos {
        let mut feet = i32::MAX;
        for dz in -10i32..=10 {
            for dx in -10i32..=10 {
                let (nx, nz) = ((dx.abs() as f64 - 0.5).max(0.0), (dz.abs() as f64 - 0.5).max(0.0));
                let d2 = nx * nx + nz * nz;
                if d2 >= 100.0 || !w.contains(BlockPos::new(c.x + dx, w.min.y, c.z + dz)) {
                    continue;
                }
                let s = lasting_height(w, c.x + dx, c.z + dz) as f64;
                feet = feet.min((s - (100.0 - d2).sqrt()).ceil() as i32 - 1);
            }
        }
        BlockPos::new(c.x, feet, c.z)
    };
    let c0 = cell_of(afk);
    // every cell a skeleton horse reaches from a strike anywhere in the sample (ruled Nov 11, G1:
    // walking or swimming, through water and connected caves, unbounded), once: no AFK feet
    // within 10 of one
    let horse: HashSet<BlockPos> = trap_reach(w);
    let trap_free = |feet: BlockPos| -> bool {
        let at = Vec3::new(feet.x as f64 + 0.5, feet.y as f64, feet.z as f64 + 0.5);
        for y in feet.y - 11..=feet.y + 11 {
            for z in feet.z - 11..=feet.z + 11 {
                for x in feet.x - 11..=feet.x + 11 {
                    let q = BlockPos::new(x, y, z);
                    if horse.contains(&q) && near_dist(q, at) <= 10.0 {
                        return false;
                    }
                }
            }
        }
        true
    };
    let mut best: Option<(i32, BlockPos)> = None;
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for k in 0..=96 {
            let q = BlockPos::new(c0.x + dx * k, c0.y, c0.z + dz * k);
            if !w.contains(BlockPos::new(q.x, w.min.y, q.z)) {
                break;
            }
            // (and no mob spawner within its player range of the feet: ironspec and G1; and no
            // cell a trap horse reaches within 10 of them, a cave included; 10 or more in from the
            // sample's edge, so every column within 10 is read)
            let inner = q.x - 10 >= w.min.x && q.x + 10 < w.min.x + w.size[0] && q.z - 10 >= w.min.z && q.z + 10 < w.min.z + w.size[2];
            if ok(q) && inner {
                let feet = feet_at(w, q);
                if spawners_near(w, Vec3::new(q.x as f64 + 0.5, feet.y as f64, q.z as f64 + 0.5)).is_empty() && trap_free(feet) && !in_band(q, feet.y, lasting_height(w, q.x, q.z)) {
                    if best.is_none_or(|(bk, _)| k < bk) {
                        best = Some((k, q));
                    }
                    break;
                }
            }
        }
    }
    let Some((_, c)) = best else {
        return Err(format!("no AFK column for the dug-in farm: none on the four axes from {} lies 2 sections from the shared bed's, 2 outside the farm's box, out of every spawner's range, out of every villager's summon band and more than 10 from every cell a trap horse reaches", at(c0)));
    };
    let feet = feet_at(w, c);
    let top = lasting_height(w, c.x, c.z);
    // the shaft and the cell, priced: every natural block from the feet up to the actual surface
    // dug (the stripped blocks over the lasting top included: the player digs them at the first
    // entry), a ladder in every cell up to it (the old depth: from 6 under the lowest strike cell
    // within 3, as rule (b) alone put it)
    let surf = redsim::weather::motion_blocking_height(w, c.x, c.z).max(top);
    let price = |feet_y: i32| -> (u32, u32, f64) {
        let mut dug = 0u32;
        for y in feet_y..surf {
            dug += !redsim::terrain::place::air_like(&w.get(BlockPos::new(c.x, y, c.z))) as u32;
        }
        let placed = (surf - feet_y).max(0) as u32;
        (dug, placed, (redsim::entity::mob::site::Effort::DUG_S * dug as f64 + redsim::entity::mob::site::Effort::PLACED_S * placed as f64) / 60.0)
    };
    let mut s6 = i32::MAX;
    for dz in -3..=3 {
        for dx in -3..=3 {
            s6 = s6.min(lasting_height(w, c.x + dx, c.z + dz));
        }
    }
    let (dug, placed, minutes) = price(feet.y);
    let (dug6, placed6, minutes6) = price(s6 - 6);
    // the cell and its 1-wide ladder shaft, dug into the checked world as the guide builds them
    // (G1 2026-11-11): the shaft from over the head up to under the lasting top, the top block kept
    // as the shaft's closure (the guide closes it behind the player) or replaced when it isn't a
    // full, non-falling, non-burning block; so no walker falls in, no sky light reaches the cell
    let shaft: Vec<BlockPos> = (feet.y + 2..top - 1).map(|y| BlockPos::new(c.x, y, c.z)).collect();
    for q in [feet, feet.above()].into_iter().chain(shaft.iter().copied()) {
        if w.contains(q) && !w.get(q).is_air() {
            w.place_raw(&[(q, Block::Air)]);
        }
    }
    let cobble = Block::parse("minecraft:cobblestone").map_err(|e| format!("cobblestone: {e}"))?;
    let mut seals = 0u32;
    let cap = BlockPos::new(c.x, (top - 1).max(feet.y + 2), c.z);
    if w.contains(cap) {
        let b = w.get(cap);
        // (the cap is the shaft's closure, a farm block: kept only if an inert stone no enderman
        // takes, else cobblestone; ironspec 2026-11-11)
        if !natural_wall(&b) || !redsim::fluid::fluid_state(w, cap).is_empty() {
            w.place_raw(&[(cap, cobble)]);
            seals += 1;
        }
    }
    if w.contains(feet.below()) && !redsim::behavior::full_cube(&w.get(feet.below())) {
        w.place_raw(&[(feet.below(), cobble)]);
        seals += 1;
    }
    // (nothing open, fluid or non-full beside the cell or the shaft: no cave opens into them)
    for q in [feet, feet.above()].into_iter().chain(shaft.iter().copied()) {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = q.offset(dx, 0, dz);
            if w.contains(n) && (redsim::terrain::place::air_like(&w.get(n)) || !redsim::fluid::fluid_state(w, n).is_empty() || !redsim::behavior::full_cube(&w.get(n))) {
                w.place_raw(&[(n, cobble)]);
                seals += 1;
            }
        }
    }
    // (G1 2026-11-11: the cell spawn-proofed with a wall torch, block light 14, so no monster
    // spawns in it while the player is 24 to 128 blocks away and none waits there on the return;
    // `Monster.isDarkEnoughToSpawn` needs block light 0)
    let walls = [((0, 1), "north"), ((0, -1), "south"), ((1, 0), "west"), ((-1, 0), "east")];
    if let Some((_, facing)) = walls.iter().find(|((dx, dz), _)| redsim::behavior::full_cube(&w.get(feet.offset(*dx, 0, *dz)))) {
        let torch = Block::parse(&format!("minecraft:wall_torch[facing={facing}]")).map_err(|e| format!("wall torch: {e}"))?;
        w.place_raw(&[(feet, torch)]);
        seals += 1;
    }
    let cost = AfkCost { depth: top - feet.y, dug, placed: placed + seals, seals, minutes: minutes + redsim::entity::mob::site::Effort::PLACED_S * seals as f64 / 60.0, dug6, placed6, minutes6 };
    Ok((Vec3::new(c.x as f64 + 0.5, feet.y as f64, c.z as f64 + 0.5), cost))
}

/// The block light at `p`: the terrain world's light engine, else the mob layer's light sources
/// (a void bench).
fn block_light_at(w: &World, p: BlockPos) -> i32 {
    w.block_light(p).or_else(|_| redsim::daylight::block_light_cached(w, p, 15)).unwrap_or(0)
}

/// The block spawners with the AFK player at `at` within their player range (`BaseSpawner`:
/// `hasNearbyAlivePlayer` within `requiredPlayerRange`, 16, of the spawner block's centre; a trial
/// spawner's 14, `TrialSpawner.DEFAULT_PLAYER_SCAN_RANGE`), with the distance.
fn spawners_near(w: &World, at: Vec3) -> Vec<(BlockPos, f64)> {
    let c = cell_of(at);
    let mut out = Vec::new();
    for y in c.y - 17..=c.y + 17 {
        for z in c.z - 17..=c.z + 17 {
            for x in c.x - 17..=c.x + 17 {
                let q = BlockPos::new(x, y, z);
                if !w.contains(q) {
                    continue;
                }
                let b = w.get(q);
                if b.is_air() {
                    continue;
                }
                let name = name_of(&b);
                let range = match name.trim_start_matches("minecraft:").split('[').next().unwrap_or("") {
                    "spawner" => 16.0,
                    "trial_spawner" => 14.0,
                    _ => continue,
                };
                let d = dist(Vec3::new(q.x as f64 + 0.5, q.y as f64 + 0.5, q.z as f64 + 0.5), at);
                if d < range {
                    out.push((q, d));
                }
            }
        }
    }
    out
}

/// The skeleton horse (`EntityTypes.SKELETON_HORSE`: 1.3964844 x 1.6; it steps 1 block).
const HORSE: Body = Body { name: "skeleton horse", width: 1.3964844, height: 1.6, step: 1.0, climbs: false, flies: false, doors: false, reach_width: 1.3964844, reach_height: 1.6 };

/// The cells a skeleton horse reaches from `starts` within `bounds`, walking (a step, a jump, any
/// fall) or swimming (a water cell its box fits in needs no floor: it wanders through water and
/// connected caves, `AbstractHorse`'s `WaterAvoidingRandomStrollGoal` and `PanicGoal`, G1).
fn horse_reach(w: &World, starts: &[BlockPos], bounds: (BlockPos, BlockPos)) -> Vec<BlockPos> {
    let (lo, hi) = bounds;
    let inside = |q: BlockPos| q.x >= lo.x && q.y >= lo.y.max(w.min.y + 1) && q.z >= lo.z && q.x <= hi.x && q.y <= hi.y && q.z <= hi.z && w.contains(q);
    let water = |q: BlockPos| !redsim::fluid::fluid_state(w, q).is_empty() && !matches!(w.get(q), Block::Lava { .. });
    let stand = |q: BlockPos| -> Option<f64> {
        if water(q) && hostile::fits(w, &HORSE, q, q.y as f64) {
            return Some(q.y as f64);
        }
        let f = hostile::floor_in(w, q)?;
        hostile::fits(w, &HORSE, q, f).then_some(f)
    };
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut out = Vec::new();
    let mut queue = VecDeque::new();
    for s in starts {
        if inside(*s) && seen.insert(*s) {
            queue.push_back((*s, stand(*s).unwrap_or(s.y as f64)));
        }
    }
    while let Some((q, f)) = queue.pop_front() {
        out.push(q);
        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (1, 1, 0), (-1, 1, 0), (0, 1, 1), (0, 1, -1), (0, 1, 0), (0, -1, 0)] {
            let n = q.offset(dx, dy, dz);
            if !inside(n) || seen.contains(&n) {
                continue;
            }
            // (up and down only through water; a step or a jump up to 1.25 on land)
            if dx == 0 && dz == 0 && !(water(q) || water(n)) {
                continue;
            }
            if let Some(nf) = stand(n)
                && nf - f <= 1.25
            {
                seen.insert(n);
                queue.push_back((n, nf));
            }
        }
        // a fall from the edge: down the column beside it, any distance (G1: a horse falls into a
        // ravine however deep)
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut n = q.offset(dx, 0, dz);
            loop {
                if !inside(n) || !hostile::fits(w, &HORSE, n, n.y as f64) {
                    break;
                }
                if stand(n).is_some() {
                    if seen.insert(n) {
                        queue.push_back((n, stand(n).unwrap_or(n.y as f64)));
                    }
                    break;
                }
                n = n.below();
            }
        }
    }
    out
}

/// Every cell a skeleton horse reaches from any cell a strike can land on in the whole sample,
/// today's tops and the lasting ones (G1 2026-11-11: unbounded, since a horse wanders tens of
/// blocks in its 18,000 gt; the finder and the line read the same set).
fn trap_reach(w: &World) -> HashSet<BlockPos> {
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut starts = Vec::new();
    for z in w.min.z..w.min.z + w.size[2] {
        for x in w.min.x..w.min.x + w.size[0] {
            for y in [redsim::weather::motion_blocking_height(w, x, z), lasting_height(w, x, z)] {
                let q = BlockPos::new(x, y, z);
                if w.contains(q) && seen.insert(q) {
                    starts.push(q);
                }
            }
        }
    }
    let top = starts.iter().map(|q| q.y).max().unwrap_or(w.min.y) + 2;
    let lo = BlockPos::new(w.min.x, w.min.y + 1, w.min.z);
    let hi = BlockPos::new(w.min.x + w.size[0] - 1, top, w.min.z + w.size[2] - 1);
    horse_reach(w, &starts, (lo, hi)).into_iter().collect()
}

/// The distance from `at` to the nearest point a horse can stand at in cell `q` (anywhere in the
/// cell sideways, on its floor: G1, not the cell's centre).
fn near_dist(q: BlockPos, at: Vec3) -> f64 {
    let x = at.x.clamp(q.x as f64, q.x as f64 + 1.0);
    let z = at.z.clamp(q.z as f64, q.z as f64 + 1.0);
    dist(Vec3::new(x, q.y as f64, z), at)
}

/// The nearest cell a skeleton horse reaches to the AFK player at `afk`, from every cell a strike
/// can land on in the whole sample ([`trap_reach`]; ruled Nov 11: no trap ever fires,
/// `SkeletonTrapGoal` firing on a player within 10 of the horse), with its distance to the
/// nearest point of the cell.
fn nearest_trap(w: &World, afk: Vec3) -> Option<(BlockPos, f64)> {
    trap_reach(w).into_iter().map(|q| (q, near_dist(q, afk))).min_by(|x, y| x.1.total_cmp(&y.1).then((x.0.x, x.0.y, x.0.z).cmp(&(y.0.x, y.0.y, y.0.z))))
}

/// Condition 2's site lines that read no flood: the block spawners the AFK player wakes (ironspec
/// and G1 2026-11-11: a spawner's mobs spawn within 4 of it while a player is within its range),
/// a dug-in farm's walls the natural rock gives, each one no mob, physics or light removes; the
/// skeleton traps a strike could leave within 10 of the AFK player (ruled: none ever fires); and
/// the dig's physics (no falling block over the dug box, no fluid or budding amethyst on its
/// faces, the AFK cells roofed and closed).
fn site_lines(p: &Placed) -> Vec<(bool, String)> {
    let mut lines = Vec::new();
    let sp = spawners_near(&p.w, p.afk);
    lines.push((sp.is_empty(), if sp.is_empty() { "spawners: no mob spawner has the AFK spot within its player range (16 blocks; a trial spawner's 14)".to_string() } else { format!("spawners: {} have the AFK spot within their player range: {}", sp.len(), sp.iter().take(4).map(|(q, d)| format!("{} at {} ({d:.1} blocks)", name_of(&p.w.get(*q)), at(*q))).collect::<Vec<_>>().join("; ")) }));
    let bad: Vec<String> = p.kept_walls.iter().filter_map(|(q, n)| unsafe_wall(n).map(|why| format!("{} at {} ({why})", name_of(n), at(*q)))).collect();
    lines.push((bad.is_empty(), if bad.is_empty() { format!("natural walls: {} of the farm's walls are natural rock, none an enderman can take, falling, ice, snow or infested", p.kept_walls.len()) } else { format!("natural walls: {} of {} unsafe: {}", bad.len(), p.kept_walls.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    match nearest_trap(&p.w, p.afk) {
        Some((q, d)) if d <= 10.0 => lines.push((false, format!("skeleton traps: a trap horse can stand at {} ({d:.1} blocks from the AFK player, a trap fires within 10)", at(q)))),
        Some((q, d)) => lines.push((true, format!("skeleton traps: none fires (the nearest cell a trap horse reaches from a strike within the sample is {} at {d:.1} blocks from the AFK player; more than 10)", at(q)))),
        None => lines.push((true, "skeleton traps: no cell a strike can land on lies within reach".to_string())),
    }
    let mut open: Vec<String> = Vec::new();
    if let Some((lo, hi)) = p.dig {
        let sp = spawners_in(&p.w, lo.offset(-4, -1, -4), hi.offset(4, 1, 4));
        open.extend(shell_seals(&p.w, lo, hi, &sp).into_iter().map(|(q, k)| format!("{k} at {}", at(q))));
    }
    let f = cell_of(p.afk);
    if falls(&p.w.get(f.offset(0, 2, 0))) {
        open.push(format!("a falling block over the AFK cell at {}", at(f.offset(0, 2, 0))));
    }
    for q in [f, f.above()] {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = q.offset(dx, 0, dz);
            if !p.w.contains(n) {
                continue;
            }
            if !redsim::fluid::fluid_state(&p.w, n).is_empty() || matches!(p.w.get(n), Block::Lava { .. }) {
                open.push(format!("a fluid beside the AFK spot at {}", at(n)));
            } else if p.afk_dug && redsim::terrain::place::air_like(&p.w.get(n)) {
                open.push(format!("an open cave beside the AFK spot at {}", at(n)));
            }
        }
    }
    lines.push((open.is_empty(), if open.is_empty() { "dig physics: no falling block over the dug box, no fluid, budding amethyst or spawner reaching it, the AFK cells roofed and closed".to_string() } else { format!("dig physics: {} unsealed: {}", open.len(), open.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    // the dug AFK cell spawn-proofed (G1 2026-11-11): a dark cell on a solid floor holds a zombie,
    // skeleton, creeper or witch whenever the player is 24 to 128 blocks away, waiting there on
    // the return (walking off and back is normal operation)
    if p.afk_dug {
        let feet = cell_of(p.afk);
        let here = p.w.get(feet);
        let lamp = matches!(here, Block::Torch { .. } | Block::WallTorch { .. }) || name_of(&here).contains("torch") || name_of(&here).contains("lantern");
        let light = if lamp { 14 } else { block_light_at(&p.w, feet) };
        let dark = light == 0 && redsim::behavior::full_cube(&p.w.get(feet.below()));
        lines.push((!dark, if dark { format!("AFK cell: dark (block light 0) on a solid floor at {}: a zombie, skeleton, creeper or witch spawns in it while the player is 24 to 128 blocks away", at(feet)) } else { format!("AFK cell: no monster spawns in it (block light {light} at {})", at(feet)) }));
    }
    // the off-pad golem spots on natural ground (ironspec 2026-11-11: every eligible rider tries a
    // summon every 100 gt, searching 8 sideways and 6 up or down; the column is narrower than that
    // band): golem spawn cells whose floor is not a farm block, natural caves and surface included
    let farm: HashSet<BlockPos> = p.farm_blocks.iter().copied().collect();
    let natural: Vec<BlockPos> = p.golem_cells.iter().copied().filter(|g| !farm.contains(&g.below())).collect();
    lines.push((natural.is_empty(), if natural.is_empty() { format!("natural golem spots: none (every one of the {} golem spawn cells the villagers' summon search finds stands on a farm block)", p.golem_cells.len()) } else { format!("natural golem spots: {} golem spawn cells on natural ground in a villager's summon band (8 sideways, 6 up or down), e.g. {}", natural.len(), natural.iter().take(4).map(|g| at(*g)).collect::<Vec<_>>().join(" ")) }));
    lines
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

/// The cover a dug-in spot asks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cover {
    /// The parent's rule (b): at least 1 block of natural cover over every farm block, and every
    /// villager, cart, zombie, boat and golem cell's box at least 1 block outside the damage box
    /// of a strike on any column within 3 of it.
    B,
    /// At least 1 block of natural cover over every farm block, nothing more (the comparison the
    /// parent asked for).
    OneBlock,
}

/// Where site-check puts the farm in a terrain sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spot {
    /// The judge's cheapest spot (`circuit::mobfarm::site_of`).
    Judge,
    /// A fixed spot, a control (ironspec 2026-11-11): the footprint's min column at world X, Z,
    /// its ground layer on the highest surface over the footprint, no flatness rule; the run says
    /// "fixed spot, not the judge's".
    At(i32, i32),
    /// The dug-in spot (the parent's ruling 2026-11-11, ironspec): every farm block under the
    /// natural surface with the cover asked, among the sample's spots, the cheapest by the bill's
    /// minutes that passes every site rule.
    DugIn(Cover),
}

/// How many of a dug-in search's cheapest candidates get the full five-condition check, cheapest
/// first, until one passes every site rule.
const DUG_IN_TRIES: usize = 5;

/// The farm placed: built into a terrain sample (`terrain`: its name and the spot), or in the
/// void; settled; its entities spawned. `positions` are design cells (moved with the farm); `afk`
/// (design coordinates) puts the AFK player there (else the nearest legal spot to the site's);
/// `speeds` (design cells, an empty cart's fastest crossing in blocks a tick, from a runner trace)
/// limits pickup to the cells a cart passes at 0.1 or faster. A dug-in spot is the cheapest of
/// the search's candidates that passes all five conditions (the first [`DUG_IN_TRIES`] tried;
/// none passing, the cheapest, and the line says so).
pub fn place(c: &circuit::Circuit, spec: &redsim::entity::mob::farm::FarmSpec, terrain: Option<(&str, Spot)>, positions: &[BlockPos], rails: bool, afk: Option<[f64; 3]>, speeds: Option<&[(BlockPos, f64)]>) -> Result<Placed, String> {
    match terrain {
        None => place_in(c, spec, None, positions, rails, afk, speeds),
        Some((t, Spot::DugIn(cover))) => {
            let d = dug_in(c, spec, t, cover, positions, rails)?;
            let tries = d.cands.len().min(DUG_IN_TRIES);
            let mut first: Option<Placed> = None;
            let mut why_rejected: Vec<String> = Vec::new();
            for k in 0..tries {
                let (site, label, dig) = d.site(spec, k)?;
                let p = place_in(c, spec, Some((site, label, Some(dig))), positions, rails, afk, speeds)?;
                // (a spot whose AFK player wakes a mob spawner, or whose natural walls can be
                // removed, is rejected outright: ironspec 2026-11-11; the skeleton traps and the
                // dig's physics are site rules of the full check below)
                let rejected: Vec<String> = site_lines(&p).into_iter().filter(|(ok, l)| !ok && (l.starts_with("spawners") || l.starts_with("natural walls"))).map(|(_, l)| l).collect();
                if !rejected.is_empty() {
                    why_rejected.push(format!("number {}: {}", k + 1, rejected.join("; ")));
                    continue;
                }
                // (the cheap conditions first; the floods last)
                if lightning(&p).pass() && pois(&p).pass() && zombies_stay(&p).pass() && empty_handed(&p).pass() && natural_mobs(&p, &[]).pass() {
                    let mut p = p;
                    p.label.push_str(&format!("; the cheapest candidate that passes every site rule (number {} by price)", k + 1));
                    return Ok(p);
                }
                if first.is_none() {
                    first = Some(p);
                }
            }
            let mut p = first.ok_or_else(|| format!("no dug-in spot in {t}: each of the {tries} cheapest has a mob spawner in range of its AFK spot or a natural wall that can be removed ({})", why_rejected.join(" | ")))?;
            p.label.push_str(&format!("; none of the {tries} cheapest candidates passes every site rule: the cheapest with no spawner in range and safe natural walls shown"));
            Ok(p)
        }
        Some((t, spot)) => {
            let (site, label, dig) = site_at(c, spec, t, spot)?;
            place_in(c, spec, Some((site, label, dig)), positions, rails, afk, speeds)
        }
    }
}

/// [`place`] on a site (with its line and, dug in, the box to dig out), or in the void.
fn place_in(c: &circuit::Circuit, spec: &redsim::entity::mob::farm::FarmSpec, site: Option<(&'static redsim::entity::mob::site::Site, String, Option<Dig>)>, positions: &[BlockPos], rails: bool, afk: Option<[f64; 3]>, speeds: Option<&[(BlockPos, f64)]>) -> Result<Placed, String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let _ = c;
    let (mut w, moved, offset, site_afk, seed, label, dugin, kept_walls, dig_box) = match site {
        Some((site, label, dig)) => {
            let moved = site.spec(spec);
            let mut w = circuit::mobfarm::settled_world(site, &moved);
            // (a dug-in farm: its box dug out, the natural rock over it kept as its cover; the
            // design's own blocks stay)
            if let Some(Dig { lo, hi, seals, fills, .. }) = &dig {
                let (lo, hi) = (*lo, *hi);
                let own: HashSet<BlockPos> = moved.blocks.iter().filter(|(_, b)| !b.is_air()).map(|(p, _)| *p).collect();
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        for x in lo.x..=hi.x {
                            let q = BlockPos::new(x, y, z);
                            if !own.contains(&q) && !w.get(q).is_air() {
                                w.place_raw(&[(q, Block::Air)]);
                            }
                        }
                    }
                }
                // (the dig's physics: the falling blocks over the box, the fluids and budding
                // amethyst on its faces, sealed with cobblestone, as priced)
                let cobble = Block::parse("minecraft:cobblestone").map_err(|e| format!("cobblestone: {e}"))?;
                for q in seals {
                    w.place_raw(&[(*q, cobble)]);
                }
                // (the caves in the villagers' summon bands, filled: no off-pad golem spot)
                let stone = Block::parse("minecraft:stone").map_err(|e| format!("stone: {e}"))?;
                for q in fills {
                    w.place_raw(&[(*q, stone)]);
                }
                w.settle_light();
            }
            let seed = redsim::terrain::samples::sample(site.sample).map(|s| s.seed);
            let dugin = dig.as_ref().map(|d| d.minutes);
            let dig_box = dig.as_ref().map(|d| (d.lo, d.hi));
            (w, moved, site.offset, site.afk, seed, label, dugin, dig.map(|d| d.kept).unwrap_or_default(), dig_box)
        }
        None => {
            let o = RunOpts::steady(1, 0, WalkMode::Random);
            let mut run = Run::start(spec, &o)?;
            while run.settle_tick() {}
            let mut w = run.world;
            w.settle_light();
            w.entities.list.clear();
            let afk = Vec3::new((spec.lo.x + spec.hi.x) as f64 / 2.0 + 0.5, spec.hi.y as f64 + 1.0, (spec.lo.z + spec.hi.z) as f64 / 2.0 + 0.5);
            (w, spec.clone(), [0, 0, 0], afk, None, "in the void".to_string(), None, Vec::new(), None)
        }
    };
    let mv = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let afk_given = afk.is_some();
    let afk = afk.map(|a| Vec3::new(a[0] + offset[0] as f64, a[1] + offset[1] as f64, a[2] + offset[2] as f64)).unwrap_or(site_afk);
    // the entities, spawned into a copy to read their boxes (the checks read the blocks alone)
    let mut with = w.clone();
    // (the farm's own entities only: a terrain sample keeps its saved mobs, which are the site's)
    with.entities.list.clear();
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
        (false, bed) if dugin.is_some() => {
            let (moved_afk, cost) = dug_in_afk(&mut w, afk, bed, (moved.lo, moved.hi), &positions_w)?;
            // (the dug cell, its shaft and its torch lit again: the sky view and the AFK cell's
            // light read them)
            w.settle_light();
            let farm = dugin.unwrap_or(0.0);
            let extra = cost.minutes - cost.minutes6;
            let note = format!(
                "the AFK spot dug in at {}, its feet {} under the lasting surface over it (more than 10 from every cell a strike can land on: no skeleton trap fires), 2 sections from the shared bed's, {} cells around it sealed (--afk overrides); its cell and 1-wide ladder shaft: {} dug + {} placed, {:.1} minutes, against {} dug + {} placed, {:.1} minutes at the old 6 under: {:+.1} minutes, {:+.1}% of the farm's {:.1}",
                vat(moved_afk), cost.depth, cost.seals, cost.dug, cost.placed, cost.minutes, cost.dug6, cost.placed6, cost.minutes6, extra, if farm > 0.0 { 100.0 * extra / farm } else { 0.0 }, farm
            );
            (moved_afk, note)
        }
        (false, Some(bed)) if sec_dist(cell_of(afk), bed) < 2 => {
            let moved = legal_afk(&w, afk, bed);
            (moved, format!("the AFK spot moved from the default {} to {}, the nearest spot 2 sections from the shared bed's (--afk overrides)", vat(afk), vat(moved)))
        }
        (false, _) => (afk, format!("the AFK spot {} (the default)", vat(afk))),
    };
    let golem_cells: Vec<BlockPos> = golem_cells_of(&w, &positions_w);
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
    Ok(Placed { w, label, positions: positions_w, villagers, carts, zombies, tracks, farm_blocks, beds, shared_bed, golem_cells, portal, afk, afk_note, cart_speeds, interior, seed, kept_walls, dig: dig_box, afk_dug: dugin.is_some() && !afk_given, seeded_pois: Vec::new(), extra_items: Vec::new() })
}

/// The site of a design in terrain sample `t` at `spot`, leaked (`RunOpts.site` is `'static`), the
/// line saying where and what it costs, and for a dug-in farm the box to dig out (world cells).
pub fn site_at(c: &circuit::Circuit, spec: &redsim::entity::mob::farm::FarmSpec, t: &str, spot: Spot) -> Result<(&'static redsim::entity::mob::site::Site, String, Option<Dig>), String> {
    use redsim::entity::mob::site as ts;
    use redsim::terrain::place;
    if spot == Spot::Judge {
        let site: &'static ts::Site = Box::leak(Box::new(circuit::mobfarm::site_of(c, spec, t)?));
        let label = format!("in {} at offset ({},{},{}), the judge's cheapest spot ({} placed, {} dug)", site.sample, site.offset[0], site.offset[1], site.offset[2], site.placed, site.dug);
        return Ok((site, label, None));
    }
    let sample = redsim::terrain::samples::sample(t).ok_or_else(|| format!("no terrain sample {t}"))?;
    let d = &c.clock.design;
    let (ground, afk_d, clear_box) = ts::params(spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str))?;
    let shared = ts::sample_world_shared(sample)?;
    let fp = ts::footprint(spec, ground)?;
    let surfaces = |w: &World, lo: [i32; 2]| -> (i32, i32) {
        let (mut a, mut b) = (i32::MAX, i32::MIN);
        for z in lo[1]..lo[1] + fp.size[1] {
            for x in lo[0]..lo[0] + fp.size[0] {
                let s = place::surface_y(w, x, z);
                a = a.min(s);
                b = b.max(s);
            }
        }
        (a, b)
    };
    match spot {
        Spot::Judge => unreachable!(),
        Spot::At(x, z) => {
            let w0 = shared.read().unwrap_or_else(|e| e.into_inner());
            let gy = place::ground_y(&w0, [x, z], [x + fp.size[0] - 1, z + fp.size[1] - 1]);
            let (lo_s, hi_s) = surfaces(&w0, [x, z]);
            let priced = ts::price_all(&w0, spec, fp, ground, clear_box, &[([x, z], gy)]);
            drop(w0);
            let secs = priced[0].seconds();
            let site: &'static ts::Site = Box::leak(Box::new(ts::Site::at_priced(shared.clone(), sample, priced, 0, afk_d, clear_box)));
            let chunk = match chunk_rule(c)? {
                Some(r) => format!("; its chunk reference block at X & 15 = {}, Z & 15 = {}: {} the design's chunk: header ({}-{}, {}-{})", (r.0.x + x - fp.lo.x).rem_euclid(16), (r.0.z + z - fp.lo.z).rem_euclid(16), if chunk_ok(&r, x - fp.lo.x, z - fp.lo.z) { "meets" } else { "MISSES" }, r.1.0, r.1.1, r.2.0, r.2.1),
                None => "; no chunk: header (any offset)".to_string(),
            };
            let label = format!("in {t} at the fixed spot ({x},{z}), fixed spot, not the judge's: relief {} (surface {lo_s}..{hi_s}), the ground layer at y {gy}, {} placed, {} dug, {:.1} build minutes{chunk}", hi_s - lo_s, site.placed, site.dug, secs / 60.0);
            Ok((site, label, None))
        }
        Spot::DugIn(cover) => {
            let (site, label, dig) = dug_in(c, spec, t, cover, &[], true)?.site(spec, 0)?;
            Ok((site, label, Some(dig)))
        }
    }
}

/// The dug-in spot finder (the parent's ruling 2026-11-11, ironspec): every candidate spot of the
/// sample's world (the footprint's min column, 8 in from its edge), the farm as high as the cover
/// lets it (`cover`: 1 block of natural cover over its whole box, dug out and the rock over it
/// kept; with rule (b), also every villager, cart, zombie, boat and golem cell's box 1 block
/// outside a strike's damage box on any column within 3); filtered on the 8-section POI distance;
/// ordered by the bill's minutes (its blocks placed, the natural blocks its box digs; the 40
/// cheapest by dig priced in full, `price_all`, for the rest of the bill).
struct DugIn {
    sample: &'static redsim::terrain::samples::Sample,
    cover: Cover,
    ground: i32,
    fp: redsim::entity::mob::site::Footprint,
    shared: std::sync::Arc<std::sync::RwLock<World>>,
    /// The candidates priced in full, cheapest first ([`Cand`]).
    cands: Vec<Cand>,
    /// The sample's block spawners.
    spawners: Vec<BlockPos>,
    /// The villagers' summon-band cells outside the box, design coordinates ([`band_cells`]).
    bands: Vec<BlockPos>,
    /// How many spots passed the cover and POI filters.
    n: usize,
    /// Whether the design's `chunk:` header limited the spots to its chunk offsets.
    chunk: bool,
}

/// A dug-in candidate priced in full: the bill's seconds, the price, the natural blocks its box
/// digs, the farm's walls the natural rock gives.
#[derive(Clone)]
struct Cand {
    secs: f64,
    price: redsim::entity::mob::site::Priced,
    dug: u32,
    kept: u32,
}

/// A design block that is a plain solid wall (natural rock can stand in for it): a full cube of
/// stone, cobblestone, deepslate, dirt and the like, nothing that acts.
fn plain_wall(b: &Block) -> bool {
    const PLAIN: &[&str] = &["stone", "cobblestone", "deepslate", "cobbled_deepslate", "stone_bricks", "andesite", "diorite", "granite", "tuff", "dirt", "smooth_stone", "calcite", "netherrack", "polished_andesite", "polished_diorite", "polished_granite"];
    PLAIN.contains(&name_of(b).trim_start_matches("minecraft:").split('[').next().unwrap_or(""))
}

/// Whether a block is in the `enderman_holdable` tag (`hostile::holdable`, on a block).
fn holdable_block(b: &Block) -> bool {
    let id = match b {
        Block::Terrain(id) => Some(*id),
        _ => redsim::terrain::blocks::id_of(&b.to_string()),
    };
    id.is_some_and(|id| redsim::terrain::blocks::props(id).has_tag("minecraft:enderman_holdable"))
}

/// The inert natural stones a dug-in farm may keep as its walls and floors (G1 2026-11-11: an
/// allow-list, since a deny-list passes magma (it burns what stands on it, light 3), crying
/// obsidian (light 10), sculk catalysts (they spread sculk when a mob dies near them) and the
/// lit redstone ores).
fn inert_stone(base: &str) -> bool {
    const INERT: &[&str] = &[
        "stone", "granite", "diorite", "andesite", "deepslate", "tuff", "calcite", "cobblestone", "cobbled_deepslate", "mossy_cobblestone", "smooth_stone", "stone_bricks", "mossy_stone_bricks", "cracked_stone_bricks", "deepslate_bricks", "deepslate_tiles", "cracked_deepslate_bricks", "cracked_deepslate_tiles", "polished_deepslate", "polished_granite", "polished_diorite", "polished_andesite", "basalt", "smooth_basalt", "polished_basalt", "blackstone", "polished_blackstone", "sandstone", "red_sandstone", "smooth_sandstone", "cut_sandstone", "terracotta", "dripstone_block", "obsidian", "raw_iron_block", "raw_copper_block", "amethyst_block",
    ];
    INERT.contains(&base) || (base.ends_with("_terracotta") && !base.contains("glazed")) || (base.ends_with("_ore") && !base.contains("redstone"))
}

/// A natural block that can stand as a farm's wall: one no mob, physics or light removes in normal
/// operation (ironspec and G1 2026-11-11): an inert stone ([`inert_stone`]) and a full cube; not in
/// the `enderman_holdable` tag (the dirt family, sand, gravel, clay...: a kept wall stands in for a
/// design wall, a farm block, and the brief keeps no farm block holdable; only cover above the farm
/// counts as erosion, out of scope); not falling; not ice or snow (ice melts over block light 11,
/// `IceBlock.randomTick`); not infested (a silverfish hatches); not flammable, a log or leaves; not
/// a fluid.
fn natural_wall(n: &Block) -> bool {
    let name = name_of(n);
    let base = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
    inert_stone(base)
        && !redsim::terrain::place::air_like(n)
        && redsim::behavior::full_cube(n)
        && !matches!(n, Block::Falling { .. } | Block::Log { .. } | Block::Leaves { .. })
        && !holdable_block(n)
        && !base.contains("ice")
        && !base.contains("snow")
        && !base.starts_with("infested_")
        && redsim::weather::fire::burn_odds(n) == 0
}

/// Why a natural block kept as a farm wall can be removed, if it can ([`natural_wall`]'s test,
/// said).
fn unsafe_wall(n: &Block) -> Option<&'static str> {
    let name = name_of(n);
    let base = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
    if holdable_block(n) {
        Some("an enderman can take it (a kept wall is a farm block)")
    } else if matches!(n, Block::Falling { .. }) || falls(n) {
        Some("it falls")
    } else if base.contains("ice") || base.contains("snow") {
        Some("light melts it")
    } else if base.starts_with("infested_") {
        Some("a silverfish hatches from it")
    } else if !inert_stone(base) {
        Some("not an inert stone (magma, crying obsidian, sculk, redstone ore and the rest off the allow-list)")
    } else if !natural_wall(n) {
        Some("not a solid, non-flammable full cube")
    } else {
        None
    }
}

/// The box a dug-in farm digs out (world cells) and the walls the natural rock gives there (each
/// cell with its natural block).
#[derive(Clone, Debug)]
pub struct Dig {
    pub lo: BlockPos,
    pub hi: BlockPos,
    pub kept: Vec<(BlockPos, Block)>,
    /// The cells sealed with cobblestone around the box ([`shell_seals`]).
    pub seals: Vec<BlockPos>,
    /// The cave cells filled in the villagers' summon bands ([`band_fills`]).
    pub fills: Vec<BlockPos>,
    /// The farm's build and dig minutes at this spot (the AFK spot's apart).
    pub minutes: f64,
}

impl DugIn {
    /// The `k`-th cheapest candidate's site, its line, and the box to dig out (world cells) with
    /// the walls the natural rock gives there.
    fn site(&self, spec: &redsim::entity::mob::farm::FarmSpec, k: usize) -> Result<(&'static redsim::entity::mob::site::Site, String, Dig), String> {
        use redsim::entity::mob::site as ts;
        let Cand { secs, price: best, dug, kept } = self.cands.get(k).cloned().ok_or_else(|| format!("no dug-in candidate {k}"))?;
        let (fp, ground) = (self.fp, self.ground);
        let off = best.price.offset;
        let gy = off[1] + ground;
        // (the AFK player on the natural surface over the footprint's middle, design coordinates)
        let mid = [fp.lo.x + fp.size[0] / 2, fp.lo.z + fp.size[1] / 2];
        let (surf, kept_walls) = {
            let w = self.shared.read().unwrap_or_else(|e| e.into_inner());
            let kept_walls: Vec<(BlockPos, Block)> = spec.blocks.iter().filter(|(_, b)| plain_wall(b)).map(|(p, _)| p.offset(off[0], off[1], off[2])).map(|q| (q, w.get(q))).filter(|(_, n)| natural_wall(n)).collect();
            (redsim::terrain::place::surface_y(&w, mid[0] + off[0], mid[1] + off[2]), kept_walls)
        };
        let afk = [mid[0] as f64 + 0.5, (surf - off[1]) as f64, mid[1] as f64 + 0.5];
        let placed = best.price.placed - kept.min(best.price.placed);
        let site: &'static ts::Site = Box::leak(Box::new(ts::Site::at_priced(self.shared.clone(), self.sample, vec![best], 0, afk, false)));
        let (lo, hi) = (BlockPos::new(spec.lo.x + off[0], spec.lo.y + off[1], spec.lo.z + off[2]), BlockPos::new(spec.hi.x + off[0], spec.hi.y + off[1], spec.hi.z + off[2]));
        let seals = {
            let w = self.shared.read().unwrap_or_else(|e| e.into_inner());
            shell_seals(&w, lo, hi, &self.spawners)
        };
        let fills = {
            let w = self.shared.read().unwrap_or_else(|e| e.into_inner());
            band_fills(&w, &self.bands, off)
        };
        let count = |k: &str| seals.iter().filter(|(_, x)| *x == k).count();
        let sealed = format!("{} falling, {} fluid, {} budding-amethyst and {} spawner cells around it sealed", count("falling"), count("fluid"), count("amethyst"), count("spawner"));
        let dig = Dig { lo, hi, kept: kept_walls, seals: seals.iter().map(|(q, _)| *q).collect(), fills: fills.clone(), minutes: secs / 60.0 };
        let label = format!("in {} at the dug-in spot ({},{}) ({}): its box dug out under the rock, the ground layer at y {gy}, {placed} placed + {} cave cells filled in the villagers' summon bands, {dug} dug (seals included: {sealed}), {kept} of its walls left as natural rock, {:.1} build+dig minutes; number {} by price of {} candidates clear of village POIs{} ({} priced in full)", self.sample.name, fp.lo.x + off[0], fp.lo.z + off[2], if self.cover == Cover::B { "lightning rule (b) cover" } else { "1-block cover" }, fills.len(), secs / 60.0, k + 1, self.n, if self.chunk { " at the design's chunk offsets" } else { "" }, self.cands.len());
        Ok((site, label, dig))
    }
}

fn dug_in(c: &circuit::Circuit, spec: &redsim::entity::mob::farm::FarmSpec, t: &str, cover: Cover, positions: &[BlockPos], rails: bool) -> Result<DugIn, String> {
    use redsim::entity::mob::site as ts;
    use redsim::terrain::place;
    let sample = redsim::terrain::samples::sample(t).ok_or_else(|| format!("no terrain sample {t}"))?;
    let d = &c.clock.design;
    let (ground, _, _) = ts::params(spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str))?;
    let shared = ts::sample_world_shared(sample)?;
    let fp = ts::footprint(spec, ground)?;
    // the design's cover constraints, in design coordinates: its box dug out, the box's top must
    // lie 1 block under the natural surface in every column of its footprint; with rule (b), every
    // box the strikes must not reach (the farm placed in the void: its riders on every rail cell,
    // its carts, zombies and boats, its golem cells)
    let y_top = spec.hi.y;
    // the farm placed in the void, once: its boxes (rule (b)) and its villager positions
    // (the villager positions given, `--positions` and `--rails`, as the check takes them)
    let v = place(c, spec, None, positions, rails, None, None)?;
    let boxes: Vec<Aabb> = if cover == Cover::B {
        v.villagers.iter().chain(v.carts.iter()).copied().chain(v.zombies.iter().flat_map(|z| z.boxes())).chain(v.golem_cells.iter().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64))).collect()
    } else {
        Vec::new()
    };
    // no golem spot on the natural ground beside the box (farmopt's golem-spawn check: the summon
    // searches up to 6 above a villager and 8 sideways, down to a floor, `SpawnUtil.trySpawnMob`):
    // every column within 8 of a villager position and outside the box keeps its ground's top
    // above that reach (its lasting strike cell 8 or more over the villager's feet); each column
    // with the highest villager reaching it (design coordinates)
    let mut golem_cols: std::collections::HashMap<(i32, i32), i32> = std::collections::HashMap::new();
    for q in &v.positions {
        for z in q.z - 8..=q.z + 8 {
            for x in q.x - 8..=q.x + 8 {
                if x >= spec.lo.x && x <= spec.hi.x && z >= spec.lo.z && z <= spec.hi.z {
                    continue;
                }
                let e = golem_cols.entry((x, z)).or_insert(i32::MIN);
                *e = (*e).max(q.y);
            }
        }
    }
    let w = shared.read().unwrap_or_else(|e| e.into_inner());
    let pois = world_pois(&w);
    let (bx0, bx1, bz0, bz1) = (spec.lo.x, spec.hi.x, spec.lo.z, spec.hi.z);
    let (y0, y1) = (spec.lo.y, spec.hi.y);
    // the farm's plain solid walls (design cells): where the natural rock is solid, non-falling and
    // non-flammable, it stays as the wall, neither dug nor placed (ironspec 2026-11-11)
    let walls: HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| plain_wall(b)).map(|(p, _)| *p).collect();
    // the sample's block spawners, once (a spawner whose spawn range reaches the box is removed
    // and priced: ironspec 2026-11-11)
    let spawners = world_spawners(&w);
    // the villagers' summon bands outside the box, once (design cells; their caves are filled)
    let bands = band_cells(&v.positions, spec.lo, spec.hi);
    // each column's lasting strike cell, once (the cover and rule (b) read it: G1 2026-11-11)
    let (wx0, wz0, wsx, wsz) = (w.min.x, w.min.z, w.size[0], w.size[2]);
    let mut hmap = vec![0i32; (wsx * wsz) as usize];
    for z in 0..wsz {
        for x in 0..wsx {
            hmap[(z * wsx + x) as usize] = lasting_height(&w, wx0 + x, wz0 + z);
        }
    }
    let lh = |x: i32, z: i32| -> i32 { if x < wx0 || z < wz0 || x >= wx0 + wsx || z >= wz0 + wsz { redsim::weather::motion_blocking_height(&w, x, z) } else { hmap[((z - wz0) * wsx + (x - wx0)) as usize] } };
    // the design's chunk alignment (its `chunk:` header), the offsets the assembly allows
    let rule = chunk_rule(c)?;
    // a spot priced: the box at shift `dy`
    let eval = |lx: i32, lz: i32, sx: i32, sz: i32, dy: i32| -> Option<Est> {
        if dy == i32::MAX || y0 + dy <= w.min.y + 1 {
            return None;
        }
        // the 8-section POI distance from the farm's box
        let (bl, bh) = (BlockPos::new(bx0 + sx, y0 + dy, bz0 + sz), BlockPos::new(bx1 + sx, y1 + dy, bz1 + sz));
        let ((ax, ay, az), (cx, cy, cz)) = (sec(bl), sec(bh));
        if pois.iter().any(|(_, q)| {
            let (qx, qy, qz) = sec(*q);
            qx >= ax - 8 && qx <= cx + 8 && qy >= ay - 8 && qy <= cy + 8 && qz >= az - 8 && qz <= cz + 8
        }) {
            return None;
        }
        // the natural blocks its box digs, and its walls kept as natural rock
        let (mut dug, mut kept) = (0u32, 0u32);
        for y in bl.y..=bh.y {
            for z in bl.z..=bh.z {
                for x in bl.x..=bh.x {
                    let n = w.get(BlockPos::new(x, y, z));
                    if walls.contains(&BlockPos::new(x - sx, y - dy, z - sz)) && natural_wall(&n) {
                        kept += 1;
                    } else if !place::air_like(&n) {
                        dug += 1;
                    }
                }
            }
        }
        // the dig's physics, sealed and priced (G1 2026-11-11): a falling block over the box
        // dug and replaced, a fluid beside it replaced, a budding amethyst on its faces dug and
        // replaced
        let seals = shell_seals(&w, bl, bh, &spawners);
        let swap = seals.iter().filter(|(_, k)| *k != "fluid").count() as u32;
        let (dug, extra) = (dug + swap, seals.len() as u32);
        let e = ts::Effort::DUG_S * dug as f64 + ts::Effort::PLACED_S * extra as f64 - ts::Effort::PLACED_S * kept as f64;
        Some(Est { e, lo: [lx, lz], g: ground + dy, dug, kept, extra })
    };
    let mut est: Vec<Est> = Vec::new();
    for lz in (w.min.z + 8)..=(w.min.z + w.size[2] - 9 - fp.size[1]) {
        for lx in (w.min.x + 8)..=(w.min.x + w.size[0] - 9 - fp.size[0]) {
            let (sx, sz) = (lx - fp.lo.x, lz - fp.lo.z);
            if let Some(r) = &rule
                && !chunk_ok(r, sx, sz)
            {
                continue;
            }
            // the highest shift (design to world y) every constraint allows: the box's top 1
            // block under every column's lasting top; with rule (b), every box 1 block out of a
            // strike's reach on the lasting tops; the natural ground beside the box out of every
            // summon band
            let mut dy = i32::MAX;
            for z in bz0..=bz1 {
                for x in bx0..=bx1 {
                    dy = dy.min(lh(x + sx, z + sz) - 2 - y_top);
                }
            }
            for b in &boxes {
                let (x0, x1) = ((b.min_x - 3.5).ceil() as i32, (b.max_x + 2.5).floor() as i32);
                let (z0, z1) = ((b.min_z - 3.5).ceil() as i32, (b.max_z + 2.5).floor() as i32);
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        dy = dy.min((lh(x + sx, z + sz) as f64 - 4.0 - b.max_y).floor() as i32);
                    }
                }
            }
            for ((x, z), vy) in &golem_cols {
                dy = dy.min(lh(x + sx, z + sz) - 8 - vy);
            }
            if let Some(p) = eval(lx, lz, sx, sz, dy) {
                est.push(p);
            }
        }
    }
    if est.is_empty() {
        return Err(format!("no dug-in spot in {} ({} cover{}): every spot is within 8 sections of a village POI, off the design's chunk alignment, or too deep for the world", sample.name, if cover == Cover::B { "rule (b)" } else { "1-block" }, if rule.is_some() { ", its chunk: header" } else { "" }));
    }
    est.sort_by(|a, b| a.e.total_cmp(&b.e));
    let n = est.len();
    // the rest of the bill (its blocks placed, the seals of its listed cells, its lava, its
    // villagers) from `price_all` as a design written into the rock; its box's dig, the dig's
    // seals, and the walls the rock gives (not placed), our own
    let take: Vec<Est> = est.iter().take(40).cloned().collect();
    let mut priced: Vec<Cand> = take
        .iter()
        .map(|e| {
            let p = ts::price_all(&w, spec, fp, ground, false, &[(e.lo, e.g)]).remove(0);
            // (the caves in the villagers' summon bands, filled: off-pad golem spots)
            let off = p.price.offset;
            let fills = band_fills(&w, &bands, off).len() as u32;
            let secs = p.seconds() + ts::Effort::DUG_S * (e.dug as f64 - p.price.dug as f64) + ts::Effort::PLACED_S * (e.extra + fills) as f64 - ts::Effort::PLACED_S * e.kept as f64;
            Cand { secs, price: p, dug: e.dug, kept: e.kept }
        })
        .collect();
    drop(w);
    priced.sort_by(|a, b| a.secs.total_cmp(&b.secs));
    Ok(DugIn { sample, cover, ground, fp, shared, cands: priced, n, chunk: rule.is_some(), spawners, bands })
}

/// A dug-in spot's estimate (the finder's first pass): the estimated seconds, the footprint's min
/// column, the ground layer, the natural blocks dug, the walls kept, the seals placed.
#[derive(Clone, Copy)]
struct Est {
    e: f64,
    lo: [i32; 2],
    g: i32,
    dug: u32,
    kept: u32,
    extra: u32,
}

/// A block that falls when its support goes (`FallingBlock`: sand, red sand, gravel, the
/// suspicious ones, concrete powder).
fn falls(b: &Block) -> bool {
    if matches!(b, Block::Falling { .. }) {
        return true;
    }
    let name = name_of(b);
    let base = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
    matches!(base, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel") || base.ends_with("_concrete_powder")
}

/// A block the dig treats specially, by its name, cached per terrain state: "spawner" (a monster
/// spawner), "trial spawner", "amethyst" (a budding amethyst).
fn special(b: &Block) -> Option<&'static str> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<u16, Option<&'static str>>> = Default::default();
    }
    let of_name = |b: &Block| -> Option<&'static str> {
        let name = name_of(b);
        match name.trim_start_matches("minecraft:").split('[').next().unwrap_or("") {
            "spawner" => Some("spawner"),
            "trial_spawner" => Some("trial spawner"),
            "budding_amethyst" => Some("amethyst"),
            _ => None,
        }
    };
    match b {
        Block::Air => None,
        Block::Terrain(id) => CACHE.with(|c| *c.borrow_mut().entry(*id).or_insert_with(|| of_name(b))),
        _ => of_name(b),
    }
}

/// Every block spawner of a world (cells), once per world.
fn world_spawners(w: &World) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let q = BlockPos::new(w.min.x + x, w.min.y + y, w.min.z + z);
                let b = w.get(q);
                if matches!(b, Block::Terrain(_)) && matches!(special(&b), Some("spawner") | Some("trial spawner")) {
                    out.push(q);
                }
            }
        }
    }
    out
}

/// The block spawners in a region (cells).
fn spawners_in(w: &World, lo: BlockPos, hi: BlockPos) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let q = BlockPos::new(x, y, z);
                if w.contains(q) && matches!(special(&w.get(q)), Some("spawner") | Some("trial spawner")) {
                    out.push(q);
                }
            }
        }
    }
    out
}

/// The seals a box dug out of the rock needs (G1 2026-11-11), each with its kind: every falling
/// block right over the box's top ("falling": it would fall in once the box is dug; the blocks
/// over it then rest on the seal), every fluid on the box's faces ("fluid": it would flow in),
/// every budding amethyst on its faces ("amethyst": its buds grow into the air beside it), and
/// every block spawner whose spawn range reaches the box (`spawners`: "spawner", removed: it places
/// its mobs within 4 sideways and 1 up or down of itself, `BaseSpawner` spawnRange, while any
/// player is within 16; ironspec 2026-11-11). Each is replaced with cobblestone.
fn shell_seals(w: &World, lo: BlockPos, hi: BlockPos, spawners: &[BlockPos]) -> Vec<(BlockPos, &'static str)> {
    let mut out = Vec::new();
    for s in spawners {
        let inside = s.x >= lo.x && s.x <= hi.x && s.y >= lo.y && s.y <= hi.y && s.z >= lo.z && s.z <= hi.z;
        if !inside && s.x >= lo.x - 4 && s.x <= hi.x + 4 && s.y >= lo.y - 1 && s.y <= hi.y + 1 && s.z >= lo.z - 4 && s.z <= hi.z + 4 {
            out.push((*s, "spawner"));
        }
    }
    let mut look = |q: BlockPos, over: bool| {
        if !w.contains(q) || out.iter().any(|(c, _)| *c == q) {
            return;
        }
        let b = w.get(q);
        if over && falls(&b) {
            out.push((q, "falling"));
        } else if !redsim::fluid::fluid_state(w, q).is_empty() {
            out.push((q, "fluid"));
        } else if special(&b) == Some("amethyst") {
            out.push((q, "amethyst"));
        }
    };
    for z in lo.z..=hi.z {
        for x in lo.x..=hi.x {
            look(BlockPos::new(x, hi.y + 1, z), true);
            look(BlockPos::new(x, lo.y - 1, z), false);
        }
    }
    for y in lo.y..=hi.y {
        for x in lo.x..=hi.x {
            look(BlockPos::new(x, y, lo.z - 1), false);
            look(BlockPos::new(x, y, hi.z + 1), false);
        }
        for z in lo.z..=hi.z {
            look(BlockPos::new(lo.x - 1, y, z), false);
            look(BlockPos::new(hi.x + 1, y, z), false);
        }
    }
    out
}

/// A design's `chunk: <x> <y> <z> <X0>-<X1> <Z0>-<Z1>` header (the brief's chunk alignment): the
/// reference block's design cell and the allowed world X & 15 and Z & 15 ranges.
pub(crate) fn chunk_rule(c: &circuit::Circuit) -> Result<Option<(BlockPos, (i32, i32), (i32, i32))>, String> {
    let Some(v) = c.clock.design.meta.get("chunk") else { return Ok(None) };
    let t: Vec<&str> = v.split_whitespace().collect();
    let range = |s: &str| -> Result<(i32, i32), String> {
        let (a, b) = s.split_once('-').unwrap_or((s, s));
        Ok((a.trim().parse::<i32>().map_err(|e| format!("chunk: {v}: {e}"))?, b.trim().parse::<i32>().map_err(|e| format!("chunk: {v}: {e}"))?))
    };
    if t.len() != 5 {
        return Err(format!("chunk: {v}: takes <x> <y> <z> <X0>-<X1> <Z0>-<Z1>"));
    }
    let n = |s: &str| s.parse::<i32>().map_err(|e| format!("chunk: {v}: {e}"));
    Ok(Some((BlockPos::new(n(t[0])?, n(t[1])?, n(t[2])?), range(t[3])?, range(t[4])?)))
}

/// Whether the design moved by `dx`, `dz` (design to world) puts its chunk reference block at an
/// allowed X & 15 and Z & 15.
pub(crate) fn chunk_ok(r: &(BlockPos, (i32, i32), (i32, i32)), dx: i32, dz: i32) -> bool {
    let (x, z) = ((r.0.x + dx).rem_euclid(16), (r.0.z + dz).rem_euclid(16));
    x >= r.1.0 && x <= r.1.1 && z >= r.2.0 && z <= r.2.1
}

// ------------------------------------------------------------------------- 1. lightning

/// Whether a block burns, decays or melts away (leaves, logs, every block `FireBlock` burns, ice
/// and snow): what a column's lasting top strips (ironspec 2026-11-11).
fn burns(b: &Block) -> bool {
    if matches!(b, Block::Leaves { .. } | Block::Log { .. }) || redsim::weather::fire::burn_odds(b) > 0 {
        return true;
    }
    let name = name_of(b);
    let base = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("");
    if base.contains("ice") || base.contains("snow") {
        return true;
    }
    if let Block::Terrain(id) = b {
        let p = redsim::terrain::blocks::props(*id);
        return p.leaves_block || p.has_tag("minecraft:logs");
    }
    false
}

/// A column's strike cell on its lasting top (G1 2026-11-11): the first cell above its top
/// motion-blocking block once leaves, logs, every block that burns, and ice and snow are gone (a
/// bolt's own fire burns a canopy from inside, out of the rain's reach; leaves decay once the logs
/// go; players cut trees). A column with nothing lasting answers as the heightmap does for an
/// empty one. (Erosion is out of scope: vjeux's standing rule 2026-11-12, designs cover normal
/// operation only, so enderman-holdable blocks stay.)
fn lasting_height(w: &World, x: i32, z: i32) -> i32 {
    let top = redsim::weather::motion_blocking_height(w, x, z);
    let mut y = top - 1;
    while y >= w.min.y {
        let b = w.get(BlockPos::new(x, y, z));
        if redsim::weather::motion_blocking(&b) && !burns(&b) {
            return y + 1;
        }
        y -= 1;
    }
    top.min(w.min.y)
}

/// A box's least clearance outside the damage box of a strike on any column within 3 of it (the
/// strike on the column's lasting top, [`lasting_height`]: `findLightningTargetAround`'s
/// MOTION_BLOCKING heightmap cell, the column's leaves, logs and burning blocks stripped; the
/// damage box x and z +-3, y -3..+9, `LightningBolt.tick`): under its bottom or over its top;
/// negative inside.
fn box_margin(w: &World, b: &Aabb) -> f64 {
    let (x0, x1) = ((b.min_x - 3.5).ceil() as i32, (b.max_x + 2.5).floor() as i32);
    let (z0, z1) = ((b.min_z - 3.5).ceil() as i32, (b.max_z + 2.5).floor() as i32);
    let mut least = f64::INFINITY;
    for x in x0..=x1 {
        for z in z0..=z1 {
            let s = lasting_height(w, x, z) as f64;
            least = least.min((s - 3.0 - b.max_y).max(b.min_y - s - 9.0));
        }
    }
    least
}

/// Condition 1, the parent's rule (b) (ironspec 2026-11-11: "nothing on the natural surface"
/// stays literal, no rod): (i) cover: no bolt's damage box (x and z +-3, y -3..+9 of a strike on
/// a column's top, `findLightningTargetAround`'s MOTION_BLOCKING heightmap position) reaches any
/// villager, cart, zombie or boat box or any golem cell's box, with a margin of at least 1 block;
/// (ii) sky: no farm living entity sees the sky from its block (`canSeeSky(blockPosition)`, the
/// entity fallback's test), so no strike is drawn onto one; (iii) fire: no flammable farm block
/// within fire reach of any surface strike (the strike's block and the 27 cells around it,
/// `LightningBolt.spawnFire`, then `FireBlock`'s spread in the storm's rain:
/// `lightning::fires_reaching`).
pub fn lightning(p: &Placed) -> Verdict {
    let mut lines = Vec::new();
    let golems: Vec<Aabb> = p.golem_cells.iter().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64)).collect();
    let boxes: Vec<(String, Aabb)> = p.villagers.iter().map(|b| ("a villager".to_string(), *b)).chain(p.carts.iter().map(|b| ("a cart".to_string(), *b))).chain(p.zombies.iter().flat_map(|z| std::iter::once(("a zombie".to_string(), z.bb)).chain(z.boat.map(|b| ("a boat".to_string(), b))))).chain(golems.iter().map(|b| ("a golem cell".to_string(), *b))).chain(std::iter::once(("the AFK player".to_string(), player_box(p.afk)))).collect();
    // (i) cover: each box against a strike on every column within 3 of it
    let mut worst: Option<(f64, String)> = None;
    for (what, b) in &boxes {
        let m = box_margin(&p.w, b);
        if worst.as_ref().is_none_or(|w| m < w.0) {
            worst = Some((m, format!("{what} at {}", vat(centre(b)))));
        }
    }
    match worst {
        Some((m, what)) => lines.push((m >= 1.0, format!("cover: the least depth margin {m:.2} blocks ({what}; at least 1)"))),
        None => lines.push((true, "cover: no box".to_string())),
    }
    // (ii) sky: every farm living entity's block (the riders at every position, the zombies), the
    // golem cells and the AFK player's feet (G1 2026-11-11: `findLightningTargetAround` takes any
    // living entity that sees the sky, players included, and the trap horse spawns at its feet)
    let living: Vec<(String, BlockPos)> = p
        .positions
        .iter()
        .map(|q| ("a villager".to_string(), *q))
        .chain(p.zombies.iter().map(|z| ("a zombie".to_string(), cell_of(Vec3::new(centre(&z.bb).x, z.bb.min_y, centre(&z.bb).z)))))
        .chain(p.golem_cells.iter().map(|g| ("a golem cell".to_string(), *g)))
        .chain(std::iter::once(("the AFK player".to_string(), cell_of(p.afk))))
        .collect();
    let sky: Vec<String> = living.iter().filter(|(_, q)| redsim::weather::can_see_sky(&p.w, *q).unwrap_or(true)).map(|(w, q)| format!("{w} at {}", at(*q))).collect();
    lines.push((sky.is_empty(), if sky.is_empty() { format!("sky view: no ({} farm living entities' blocks, golem cells and the AFK player's feet, none sees the sky)", living.len()) } else { format!("sky view: yes, {} ({})", sky.len(), sky.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) }));
    // (iii) fire: every surface strike over the farm (its columns and 8 around), its fires
    let flammable: Vec<BlockPos> = p.farm_blocks.iter().copied().filter(|q| redsim::weather::lightning::flammable(&p.w.get(*q)).is_some()).collect();
    if flammable.is_empty() {
        lines.push((true, "fire: no flammable farm block".to_string()));
    } else {
        let (lo, hi) = bounds_of(p, 8);
        let mut starts: Vec<BlockPos> = Vec::new();
        let mut nearest: Option<(f64, BlockPos, BlockPos)> = None;
        for x in lo.x..=hi.x {
            for z in lo.z..=hi.z {
                // (a strike lands on the column's top now, and on its lasting top once trees and
                // whatever burns are gone)
                let (now, lasting) = (redsim::weather::motion_blocking_height(&p.w, x, z), lasting_height(&p.w, x, z));
                for y in if now == lasting { vec![now] } else { vec![now, lasting] } {
                    let s = BlockPos::new(x, y, z);
                    if !p.w.contains(s) {
                        continue;
                    }
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                starts.push(s.offset(dx, dy, dz));
                            }
                        }
                    }
                    for f in &flammable {
                        let d = dist(Vec3::new(f.x as f64, f.y as f64, f.z as f64), Vec3::new(s.x as f64, s.y as f64, s.z as f64));
                        if nearest.is_none_or(|(nd, _, _)| d < nd) {
                            nearest = Some((d, *f, s));
                        }
                    }
                }
            }
        }
        starts.sort_by_key(|q| (q.x, q.y, q.z));
        starts.dedup();
        let reaching = redsim::weather::lightning::fires_reaching(&p.w, &flammable, &starts);
        let near = nearest.map(|(d, f, s)| format!("the nearest flammable farm block {} is {d:.1} blocks from the strike at {}", at(f), at(s))).unwrap_or_default();
        lines.push((reaching.is_empty(), if reaching.is_empty() { format!("fire: no surface strike's fire reaches any of the {} flammable farm blocks ({near})", flammable.len()) } else { format!("fire: {} strike fires reach a flammable farm block, e.g. from {} ({near})", reaching.len(), reaching.iter().take(3).map(|q| at(*q)).collect::<Vec<_>>().join(" ")) }));
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

/// The floods of every body from every position a natural mob of it can spawn at (the game's spawn
/// rules: `hostile::Seeding::SpawnRules`, ironspec 2026-11-11, vjeux's normal-operation scope) or
/// walk in from beyond the bounds, plus the mob cells a control injects.
fn floods(p: &Placed, extra: &[(Body, BlockPos)]) -> Vec<(Body, Vec<(BlockPos, f64)>)> {
    let bounds = bounds_of(p, 24);
    let bodies = [hostile::ZOMBIE, hostile::BABY_ZOMBIE, hostile::SKELETON, hostile::PILLAGER, hostile::CREEPER, hostile::SPIDER, hostile::CAVE_SPIDER, hostile::SLIME, hostile::FOX, hostile::VILLAGER, hostile::ENDERMAN, hostile::BAT];
    let seeding = hostile::Seeding::SpawnRules { seed: p.seed };
    // (a trap's starts on the lasting tops too, the strike cells once trees are gone)
    let (lo, hi) = bounds;
    let lasting: Vec<BlockPos> = (lo.z..=hi.z)
        .flat_map(|z| (lo.x..=hi.x).map(move |x| (x, z)))
        .filter(|(x, z)| p.w.contains(BlockPos::new(*x, p.w.min.y, *z)))
        .map(|(x, z)| BlockPos::new(x, lasting_height(&p.w, x, z), z))
        .collect();
    let mut out: Vec<(Body, Vec<(BlockPos, f64)>)> = bodies.iter().map(|b| (*b, hostile::flood_from(&p.w, *b, &p.interior, bounds, seeding, if b.name == "skeleton" { &lasting } else { &[] }).cells)).collect();
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
    // (golems and the AFK player are targets too: a trap's skeletons, and every skeleton and
    // zombie, attack iron golems and players; ironspec 2026-11-11)
    let golem_targets: Vec<Aabb> = p.golem_cells.iter().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64)).collect();
    let afk_box = player_box(p.afk);
    let farm_targets: Vec<Aabb> = p.villagers.iter().chain(p.carts.iter()).chain(posts.iter()).copied().collect();
    let targets: Vec<Aabb> = farm_targets.iter().chain(golem_targets.iter()).copied().chain(std::iter::once(afk_box)).collect();
    let melee_targets: Vec<Aabb> = p.villagers.iter().chain(golem_targets.iter()).copied().chain(std::iter::once(afk_box)).collect();
    let villager_feet: Vec<Vec3> = p.positions.iter().map(|q| Vec3::new(q.x as f64 + 0.5, q.y as f64, q.z as f64 + 0.5)).collect();
    // the AFK player's own two cells hold no mob: nothing spawns within 24 blocks of a player
    // (`NaturalSpawner`: the nearest player farther than 24), and the dug-in AFK cell is sealed
    let afk_feet = BlockPos::new(p.afk.x.floor() as i32, p.afk.y.floor() as i32, p.afk.z.floor() as i32);
    let afk_cells = [afk_feet, afk_feet.above()];
    // reach: the zombie family's and the skeletons' melee box against a villager's, a golem
    // cell's or the AFK player's (walking, climbing and swimming are the floods' moves; spiders
    // and the rest do not attack them)
    let mut reach = Vec::new();
    for name in ["zombie", "baby zombie", "skeleton"] {
        for (b, c, f) in cells(name) {
            if afk_cells.contains(&c) {
                continue;
            }
            let ab = hostile::reach_box(&b, c, f).inflate(REACH, 0.0, REACH);
            if melee_targets.iter().any(|v| v.intersects(ab)) {
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
        if afk_cells.contains(&c) {
            continue;
        }
        let feet = Vec3::new(c.x as f64 + 0.5, f, c.z as f64 + 0.5);
        if dist(feet, p.afk) > 3.0 {
            continue;
        }
        if farm_targets.iter().any(|t| dist(centre(t), feet) <= 6.0) {
            blast.push(Hit { what: "creeper".to_string(), cell: c });
        } else if portal_at.iter().any(|q| dist(*q, feet) <= 6.0) {
            blast.push(Hit { what: "creeper by a portal".to_string(), cell: c });
        } else {
            // (the AFK player is a target too: a creeper swells at the player within 3 and its
            // blast loses the loader: G1 2026-11-11)
            blast.push(Hit { what: "creeper by the AFK player".to_string(), cell: c });
        }
    }
    // ranged: in sight within the shooter's range (eye to the target's centre)
    let mut ranged = Vec::new();
    for (name, shooter, range) in [("skeleton", "a skeleton, stray or bogged", 16.0), ("pillager", "a pillager", 8.0), ("villager", "a witch", 10.0)] {
        for (b, c, f) in cells(name) {
            if afk_cells.contains(&c) {
                continue;
            }
            let eye = Vec3::new(c.x as f64 + 0.5, f + b.height * 0.85, c.z as f64 + 0.5);
            if targets.iter().any(|t| dist(eye, centre(t)) <= range && redsim::entity::mob::los::has_line_of_sight(&p.w, eye, centre(t))) {
                ranged.push(Hit { what: shooter.to_string(), cell: c });
            }
        }
    }
    for (_, c, f) in cells("zombie") {
        if redsim::fluid::fluid_state(&p.w, c).is_empty() || afk_cells.contains(&c) {
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
    // (raids are out: vjeux's standing rule 2026-11-12, designs cover normal operation only)
    lines.extend(site_lines(p));
    // the dug cavity's spawn-proofing, priced (not a gate; ironspec 2026-11-11: the bills price
    // the site fixes): every cell a zombie stands on inside the dug box, a slab or carpet each
    if let Some((lo, hi)) = p.dig {
        let inside = |c: &BlockPos| c.x >= lo.x && c.x <= hi.x && c.y >= lo.y && c.y <= hi.y && c.z >= lo.z && c.z <= hi.z;
        let n = fl.iter().filter(|(b, _)| b.name == "zombie").flat_map(|(_, c)| c.iter()).filter(|(c, _)| inside(c)).count();
        lines.push((true, format!("spawn-proofing (priced, not a gate): {n} cells a zombie stands on inside the dug box; a slab or carpet on each: +{n} placed, {:.1} minutes", n as f64 * redsim::entity::mob::site::Effort::PLACED_S / 60.0)));
    }
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
    lines.push((true, format!("sources (the game's spawn rules, ironspec and G1 2026-11-11): monsters at block light 0 on their spawn floor (the spawn box free at the cell's bottom); endermen the same, then every spot their teleport reaches (32 sideways, -32..+31 up, onto a motion-blocking floor with the box free); a trap skeleton on every strike cell, today's tops and the lasting ones, on any floor at any light, unless a lightning rod topping its column within 128 (a sphere about the strike cell) takes the strike; a patrol's pillager on the surface at block light 8 or less; slimes in slime chunks below y 40 (under the farm: {}) and in swamps at y 51 to 69 at night light 7 or less (where the biome isn't known, there too); bats under the world surface at night light 6 or less (Halloween's limit, Oct 20 to Nov 3, counted whatever the date); foxes at light 9 or more; anything from beyond across the bounds' ring; wandering traders and llamas are in the floods", slime.map(|n| n.to_string()).unwrap_or_else(|| "none, the void".to_string()))));
    lines.push((true, "regimes: the player within 128 blocks, the channels above; the player away with a chunk loader, no natural spawn anywhere (`NaturalSpawner` needs a player within 128), every natural channel 0 (lightning, random ticks and skeleton traps still run in an entity-ticking chunk)".to_string()));
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
    let air = Block::Air;
    let top = p.w.min.y + p.w.size[1] - 1;
    // 1. lightning, rule (b): a villager 2 below the surface; a zombie with sky view; a wooden
    // trapdoor at the surface (each on an open column of the world's corner, its own line)
    let corner = |q: &mut Placed| -> BlockPos {
        let (x, z) = (q.w.min.x + 2, q.w.min.z + 2);
        let f = BlockPos::new(x, redsim::weather::motion_blocking_height(&q.w, x, z).max(q.w.min.y + 1), z);
        for y in f.y..=top {
            q.w.place_raw(&[(BlockPos::new(x, y, z), air)]);
        }
        q.w.place_raw(&[(f.below(), stone)]);
        f
    };
    {
        let mut q = p.clone();
        let f = corner(&mut q);
        let b = rider_box(f);
        q.w.place_raw(&[(f.offset(0, 4, 0), stone)]);
        q.villagers.push(b);
        let v = lightning(&q);
        let bites = box_margin(&q.w, &b) < 1.0 && v.failing("cover").is_some();
        out.push((1, Control { name: format!("a villager at {} 2 below the surface", at(f)), bites, why: v.failing("cover").map(str::to_string).unwrap_or_else(|| "cover: passes".to_string()) }));
    }
    {
        let mut q = p.clone();
        let f = corner(&mut q);
        q.zombies.push(Zombie { bb: hostile::box_at(&hostile::ZOMBIE, f, f.y as f64), boat: None, riding: true, persistent: true, hands: None });
        let v = lightning(&q);
        let bites = redsim::weather::can_see_sky(&q.w, f).unwrap_or(false) && v.failing("sky view").is_some();
        out.push((1, Control { name: format!("a zombie at {} with sky view", at(f)), bites, why: v.failing("sky view").map(str::to_string).unwrap_or_else(|| "sky view: passes".to_string()) }));
    }
    {
        // (ironspec asked a wooden trapdoor; trapdoors do not burn in 26.3: `FireBlock.bootStrap`
        // makes the fence gates flammable, 5/20, and no trapdoor or door, so an oak fence gate)
        let mut q = p.clone();
        let f = corner(&mut q);
        let gate = Block::parse("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]").expect("oak fence gate");
        q.w.place_raw(&[(f.below(), gate)]);
        q.farm_blocks.push(f.below());
        let v = lightning(&q);
        let starts: Vec<BlockPos> = (-1..=1).flat_map(|dy| (-1..=1).flat_map(move |dz| (-1..=1).map(move |dx| f.offset(dx, dy, dz)))).collect();
        let bites = !redsim::weather::lightning::fires_reaching(&q.w, &[f.below()], &starts).is_empty() && v.failing("fire").is_some();
        out.push((1, Control { name: format!("an oak fence gate at {} at the surface", at(f.below())), bites, why: v.failing("fire").map(str::to_string).unwrap_or_else(|| "fire: passes".to_string()) }));
    }
    {
        // (ironspec 2026-11-11: the AFK spot under 1 block of cover: a strike on the roof puts
        // its box from 3 below the bolt's cell, over the player)
        let mut q = p.clone();
        let f = corner(&mut q);
        q.w.place_raw(&[(f.offset(0, 2, 0), stone)]);
        q.afk = Vec3::new(f.x as f64 + 0.5, f.y as f64, f.z as f64 + 0.5);
        let v = lightning(&q);
        let bites = box_margin(&q.w, &player_box(q.afk)) < 1.0 && v.failing("cover").is_some();
        out.push((1, Control { name: format!("the AFK spot at {} under 1 block of cover", at(f)), bites, why: v.failing("cover").map(str::to_string).unwrap_or_else(|| "cover: passes".to_string()) }));
    }
    {
        // (G1 2026-11-11: a box covered only by a tree canopy: a 7 x 7 scene in the world's
        // corner, a stone floor, a villager in its middle, oak leaves 6 over the floor; the
        // canopy keeps it 1.6 out of reach on today's tops, but not on the lasting ones)
        let mut q = p.clone();
        let (x0, z0) = (q.w.min.x, q.w.min.z);
        let fy = q.w.min.y + 2;
        let leaves = Block::parse("minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]").or_else(|_| redsim::terrain::blocks::id_of("minecraft:oak_leaves[distance=1,persistent=false,waterlogged=false]").map(Block::Terrain).ok_or(())).expect("oak leaves");
        for z in z0..z0 + 7 {
            for x in x0..x0 + 7 {
                for y in fy..=top {
                    q.w.place_raw(&[(BlockPos::new(x, y, z), air)]);
                }
                q.w.place_raw(&[(BlockPos::new(x, fy - 1, z), stone)]);
                q.w.place_raw(&[(BlockPos::new(x, fy + 6, z), leaves)]);
            }
        }
        let f = BlockPos::new(x0 + 3, fy, z0 + 3);
        let b = rider_box(f);
        q.villagers.push(b);
        let v = lightning(&q);
        // (today's top over it: the canopy; the lasting top: the floor)
        let today = (redsim::weather::motion_blocking_height(&q.w, f.x, f.z) as f64 - 3.0 - b.max_y) >= 1.0;
        let bites = today && box_margin(&q.w, &b) < 1.0 && v.failing("cover").is_some();
        out.push((1, Control { name: format!("a villager at {} covered only by a tree canopy", at(f)), bites, why: v.failing("cover").map(str::to_string).unwrap_or_else(|| "cover: passes".to_string()) }));
    }
    // 2. natural mobs: one mob put by a target, for each target; it bites when the channel it
    // aims at catches that mob (the placed farm's own line may already fail on other mobs)
    {
        // (the site lines that read no flood: a natural wall of dirt, one of infested stone, each
        // named by the walls line; a mob spawner 10 blocks from the AFK spot, named by the
        // spawners line: ironspec 2026-11-11)
        let line = |q: &Placed, prefix: &str| site_lines(q).into_iter().find(|(ok, l)| !ok && l.starts_with(prefix)).map(|(_, l)| l);
        // (natural blocks as the samples hold them: the terrain table's)
        let natural = |name: &str| redsim::terrain::blocks::id_of(name).map(Block::Terrain).or_else(|| Block::parse(name).ok()).expect("a natural block");
        let wall_cell = p.kept_walls.first().map(|(q, _)| *q).or_else(|| p.farm_blocks.first().copied()).unwrap_or(p.w.min);
        for (what, block) in [("a natural dirt cell", "minecraft:dirt"), ("a natural magma block", "minecraft:magma_block"), ("a natural infested-stone cell", "minecraft:infested_stone")] {
            let mut q = p.clone();
            let n = natural(block);
            q.kept_walls.retain(|(c, _)| *c != wall_cell);
            q.kept_walls.push((wall_cell, n));
            let l = line(&q, "natural walls");
            out.push((2, Control { name: format!("{what} kept as the farm's wall at {}", at(wall_cell)), bites: l.as_ref().is_some_and(|l| l.contains(&at(wall_cell))), why: l.unwrap_or_else(|| "natural walls: passes".to_string()) }));
        }
        let mut q = p.clone();
        let a = cell_of(p.afk);
        let spot = [a.offset(10, 0, 0), a.offset(-10, 0, 0), a.offset(0, 0, 10), a.offset(0, 0, -10)].into_iter().find(|s| q.w.contains(*s)).unwrap_or(a.offset(10, 0, 0));
        if q.w.contains(spot) {
            q.w.place_raw(&[(spot, natural("minecraft:spawner"))]);
        }
        let l = line(&q, "spawners");
        out.push((2, Control { name: format!("a mob spawner at {}, 10 blocks from the AFK spot", at(spot)), bites: l.as_ref().is_some_and(|l| l.contains(&at(spot))), why: l.unwrap_or_else(|| "spawners: passes".to_string()) }));
        // (G1 2026-11-11: the dig's physics: gravel over a dug cell; lava beside the AFK spot;
        // each named by the dig-physics line)
        let mut q = p.clone();
        let (lo, hi) = q.dig.unwrap_or_else(|| bounds_of(p, 0));
        q.dig = Some((lo, hi));
        let over = BlockPos::new(lo.x, hi.y + 1, lo.z);
        if q.w.contains(over) {
            q.w.place_raw(&[(over, natural("minecraft:gravel"))]);
        }
        let l = line(&q, "dig physics");
        out.push((2, Control { name: format!("gravel at {} over a dug cell", at(over)), bites: l.as_ref().is_some_and(|l| l.contains(&at(over))), why: l.unwrap_or_else(|| "dig physics: passes".to_string()) }));
        let mut q = p.clone();
        let beside = cell_of(p.afk).offset(1, 0, 0);
        if q.w.contains(beside) {
            q.w.place_raw(&[(beside, Block::parse("minecraft:lava[level=0]").expect("lava"))]);
        }
        let l = line(&q, "dig physics");
        out.push((2, Control { name: format!("lava at {} beside the AFK spot", at(beside)), bites: l.as_ref().is_some_and(|l| l.contains(&at(beside))), why: l.unwrap_or_else(|| "dig physics: passes".to_string()) }));
        // (ironspec 2026-11-11: a spawner 3 blocks from the cavity: its spawn range reaches the
        // dug box)
        let mut q = p.clone();
        let (lo, hi) = q.dig.unwrap_or_else(|| bounds_of(p, 0));
        q.dig = Some((lo, hi));
        let sp = BlockPos::new(hi.x + 3, lo.y, lo.z);
        if q.w.contains(sp) {
            q.w.place_raw(&[(sp, natural("minecraft:spawner"))]);
        }
        let l = line(&q, "dig physics");
        out.push((2, Control { name: format!("a mob spawner at {}, 3 blocks from the cavity", at(sp)), bites: l.as_ref().is_some_and(|l| l.contains(&at(sp))), why: l.unwrap_or_else(|| "dig physics: passes".to_string()) }));
        // (ironspec 2026-11-11: a cave cell opened in a rider's summon band: a natural floor 8
        // sideways of the first villager position, outside the farm's box, with room over it)
        let mut q = p.clone();
        let (blo, bhi) = bounds_of(p, 0);
        let spot = p.positions.iter().flat_map(|v| [v.offset(8, 0, 0), v.offset(-8, 0, 0), v.offset(0, 0, 8), v.offset(0, 0, -8), v.offset(8, 0, 8), v.offset(-8, 0, -8), v.offset(8, 0, -8), v.offset(-8, 0, 8)]).find(|c| q.w.contains(c.offset(-1, -1, -1)) && q.w.contains(c.offset(1, 3, 1)) && (c.x + 1 < blo.x || c.x - 1 > bhi.x || c.z + 1 < blo.z || c.z - 1 > bhi.z));
        if let Some(c) = spot {
            // (a 3 x 3 natural floor under 3 clear layers: room for the golem's 1.4-wide box)
            for dz in -1..=1 {
                for dx in -1..=1 {
                    q.w.place_raw(&[(c.offset(dx, -1, dz), natural("minecraft:stone"))]);
                    for y in 0..3 {
                        q.w.place_raw(&[(c.offset(dx, y, dz), air)]);
                    }
                }
            }
            q.golem_cells = golem_cells_of(&q.w, &q.positions);
            let l = line(&q, "natural golem spots");
            out.push((2, Control { name: format!("a cave floor at {} in a rider's summon band", at(c.below())), bites: l.as_ref().is_some_and(|l| l.contains("natural golem spots:")) && q.golem_cells.contains(&c), why: l.unwrap_or_else(|| "natural golem spots: passes".to_string()) }));
        } else {
            out.push((2, Control { name: "a cave floor in a rider's summon band".to_string(), bites: false, why: "no cell 8 or 9 sideways of a villager position outside the farm's box".to_string() }));
        }
        // (ruled Nov 11: the AFK spot 9 below a surface cell a strike can land on: a trap horse
        // standing there is within 10)
        let mut q = p.clone();
        let (x0, z0) = (q.w.min.x + 3, q.w.min.z + 3);
        let fy = q.w.min.y + 2;
        for y in fy - 1..=top {
            q.w.place_raw(&[(BlockPos::new(x0, y, z0), stone)]);
        }
        for y in fy..fy + 2 {
            q.w.place_raw(&[(BlockPos::new(x0, y, z0), air)]);
        }
        for y in fy + 9..=top {
            q.w.place_raw(&[(BlockPos::new(x0, y, z0), air)]);
        }
        let cap = BlockPos::new(x0, fy + 9, z0);
        q.afk = Vec3::new(x0 as f64 + 0.5, fy as f64, z0 as f64 + 0.5);
        let l = line(&q, "skeleton traps");
        let near = horse_reach(&q.w, &[cap], (cap.offset(-12, -12, -12), cap.offset(12, 2, 12))).into_iter().any(|c| dist(Vec3::new(c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5), q.afk) <= 10.0);
        out.push((2, Control { name: format!("the AFK spot at {} 9 below the surface cell {}", at(cell_of(q.afk)), at(cap)), bites: near && l.is_some(), why: l.unwrap_or_else(|| "skeleton traps: passes".to_string()) }));
        // (N2, G1 and ironspec 2026-11-11: the AFK shaft topped with a trapdoor, which passes full
        // sky light while it counts as the column's motion-blocking top: the AFK player sees the
        // sky; the same shaft capped with stone does not)
        let shaft = |cap_block: Block| -> Placed {
            let mut q = p.clone();
            let (x0, z0) = (q.w.min.x + 3, q.w.min.z + 3);
            let fy = q.w.min.y + 2;
            for y in fy - 1..=fy + 5 {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        q.w.place_raw(&[(BlockPos::new(x0 + dx, y, z0 + dz), stone)]);
                    }
                }
            }
            for y in fy..fy + 5 {
                q.w.place_raw(&[(BlockPos::new(x0, y, z0), air)]);
            }
            q.w.place_raw(&[(BlockPos::new(x0, fy + 5, z0), cap_block)]);
            for y in fy + 6..=top {
                q.w.place_raw(&[(BlockPos::new(x0, y, z0), air)]);
            }
            q.w.settle_light();
            q.afk = Vec3::new(x0 as f64 + 0.5, fy as f64, z0 as f64 + 0.5);
            q
        };
        let trapdoor = Block::parse("minecraft:oak_trapdoor[facing=north,half=top,open=false,powered=false,waterlogged=false]").expect("an oak trapdoor");
        // (G1 2026-11-11: the dug AFK cell left dark, a monster's spawn cell on the return; the same
        // cell with its wall torch passes)
        {
            let cell = |lit: bool| -> Placed {
                let mut q = shaft(stone);
                q.afk_dug = true;
                if lit {
                    let feet = cell_of(q.afk);
                    q.w.place_raw(&[(feet, Block::parse("minecraft:wall_torch[facing=north]").expect("a wall torch"))]);
                    q.w.settle_light();
                }
                q
            };
            let (dark, lit) = (cell(false), cell(true));
            let (ld, ll) = (line(&dark, "AFK cell"), line(&lit, "AFK cell"));
            out.push((2, Control { name: format!("the dug AFK cell at {} left dark (the same cell with its wall torch passes)", at(cell_of(dark.afk))), bites: ld.is_some() && ll.is_none(), why: ld.unwrap_or_else(|| "AFK cell: passes".to_string()) }));
        }
        // (ironspec 2026-11-11: the AFK cell back inside a villager's summon band: a 1 x 1 shaft
        // over a natural floor 7 sideways of a villager position, outside the farm, is a golem spot)
        {
            let mut q = p.clone();
            let (blo, bhi) = bounds_of(p, 0);
            let spot = p.positions.iter().flat_map(|v| [v.offset(7, 0, 0), v.offset(-7, 0, 0), v.offset(0, 0, 7), v.offset(0, 0, -7)]).find(|c| q.w.contains(c.offset(-1, -1, -1)) && q.w.contains(c.offset(1, 9, 1)) && (c.x + 1 < blo.x || c.x - 1 > bhi.x || c.z + 1 < blo.z || c.z - 1 > bhi.z));
            match spot {
                Some(c) => {
                    // (the shaft's rock up to over the search's top, 6 above the villager, so the
                    // search finds no floor before the AFK cell's)
                    for y in -1..=8 {
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                q.w.place_raw(&[(c.offset(dx, y, dz), natural("minecraft:stone"))]);
                            }
                        }
                    }
                    for y in 0..8 {
                        q.w.place_raw(&[(c.offset(0, y, 0), air)]);
                    }
                    q.golem_cells = golem_cells_of(&q.w, &q.positions);
                    let l = line(&q, "natural golem spots");
                    out.push((2, Control { name: format!("the AFK cell at {} inside a villager's summon band (a 1 x 1 shaft over a natural floor)", at(c)), bites: l.as_ref().is_some_and(|l| l.contains("natural golem spots:")) && q.golem_cells.contains(&c), why: l.unwrap_or_else(|| "natural golem spots: passes".to_string()) }));
                }
                None => out.push((2, Control { name: "the AFK cell inside a villager's summon band".to_string(), bites: false, why: "no column 7 sideways of a villager position outside the farm's box".to_string() })),
            }
        }
        let (with_trap, with_stone) = (shaft(trapdoor), shaft(stone));
        let sees = |q: &Placed| redsim::weather::can_see_sky(&q.w, cell_of(q.afk)).unwrap_or(false);
        let lt = lightning(&with_trap).failing("sky view").map(str::to_string);
        out.push((1, Control { name: format!("the AFK shaft at {} topped with an oak trapdoor (the same shaft capped with stone: the AFK player out of the sky)", at(cell_of(with_trap.afk))), bites: sees(&with_trap) && !sees(&with_stone) && lt.is_some(), why: lt.unwrap_or_else(|| "sky view: passes".to_string()) }));
        // (C.3, G1 2026-11-11: a strike cell lit by a torch is still a skeleton source, since the
        // floods seed every standable cell at any light: a floor 4 to 8 sideways of a villager,
        // outside the farm, the column's top, a torch beside it, in a skeleton's sight and range)
        let mut q = p.clone();
        let mut lit = None;
        'find: for v in p.positions.iter() {
            let target = Vec3::new(v.x as f64 + 0.5, v.y as f64 + 0.9, v.z as f64 + 0.5);
            for (dx, dz) in [(6, 0), (-6, 0), (0, 6), (0, -6), (5, 5), (-5, -5), (5, -5), (-5, 5), (8, 0), (-8, 0), (0, 8), (0, -8), (4, 0), (-4, 0), (0, 4), (0, -4)] {
                let c = v.offset(dx, 0, dz);
                let t = c.offset(1, 0, 0);
                let clear = (0..3).all(|y| q.w.contains(c.offset(0, y, 0)) && q.w.get(c.offset(0, y, 0)).is_air()) && q.w.contains(c.below()) && q.w.get(c.below()).is_air() && q.w.contains(t) && q.w.get(t).is_air() && !q.interior.contains(&c);
                let eye = Vec3::new(c.x as f64 + 0.5, c.y as f64 + 1.69, c.z as f64 + 0.5);
                if clear && redsim::weather::motion_blocking_height(&q.w, c.x, c.z) <= c.y - 1 && redsim::entity::mob::los::has_line_of_sight(&q.w, eye, target) {
                    lit = Some((c, t));
                    break 'find;
                }
            }
        }
        match lit {
            Some((c, t)) => {
                q.w.place_raw(&[(c.below(), natural("minecraft:stone"))]);
                q.w.place_raw(&[(t, Block::parse("minecraft:torch").expect("a torch"))]);
                q.w.settle_light();
                let (v, ch) = natural_mobs_hits(&q, &[]);
                let bites = ch.iter().any(|(n, cells)| *n == "ranged" && cells.contains(&c));
                out.push((2, Control { name: format!("a lit strike cell at {} (a torch beside it) in a skeleton's range of a villager", at(c)), bites, why: v.failing("ranged").map(str::to_string).unwrap_or_else(|| "ranged: passes".to_string()) }));
            }
            None => out.push((2, Control { name: "a lit strike cell in a skeleton's range of a villager".to_string(), bites: false, why: "no open cell 4 to 8 sideways of a villager position in sight of it".to_string() })),
        }
        // (G1 and ironspec 2026-11-11: a trap's skeleton horse appears where the strike lands, on
        // any floor at any light, so a strike cell on leaves is a source; a lightning rod topping
        // its column within 128 takes the strike, `findLightningRod`, and spawns no trap)
        if let Some((c, _)) = lit {
            let mut q = p.clone();
            q.w.place_raw(&[(c.below(), Block::parse("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]").expect("leaves"))]);
            q.w.settle_light();
            let hit = |q: &Placed| natural_mobs_hits(q, &[]).1.iter().any(|(n, cells)| *n == "ranged" && cells.contains(&c));
            let open = hit(&q);
            let mut r = q.clone();
            let corner = BlockPos::new(r.w.min.x, r.w.height(redsim::terrain::Heightmap::WorldSurface, r.w.min.x, r.w.min.z), r.w.min.z);
            r.w.place_raw(&[(corner, Block::parse("minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]").expect("a lightning rod"))]);
            let rod = hit(&r);
            out.push((2, Control { name: format!("a strike cell on leaves at {} in a skeleton's range of a villager (with a lightning rod topping its column at {}: no trap)", at(c), at(corner)), bites: open && !rod, why: format!("ranged {} without the rod, {} with it", if open { "catches it" } else { "misses it" }, if rod { "still catches it" } else { "clear" }) }));
        }
    }
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
            // (the AFK player one block over: no mob stands in the player's own cells)
            q.afk = Vec3::new(cell.x as f64 + 1.5, cell.y as f64, cell.z as f64 + 0.5);
            // (no rider, cart, boat or post within its blast: the portal alone is the target)
            q.villagers.clear();
            q.carts.clear();
            q.zombies.clear();
            let (v, ch) = natural_mobs_hits(&q, &[(hostile::CREEPER, cell)]);
            let l = v.failing("blasts").map(str::to_string);
            let bites = ch.iter().any(|(n, cells)| *n == "blasts" && cells.contains(&cell)) && l.as_ref().is_some_and(|l| l.contains("creeper by a portal"));
            out.push((2, Control { name: format!("a creeper by the pad's portal block at {} at {}", at(*portal), at(cell)), bites, why: l.unwrap_or_else(|| "blasts: passes".to_string()) }));
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

    /// Every condition's positive controls bite on the carousel's assembly v0a in the void: each
    /// of the five conditions has controls, and every one fails the line it aims at.
    #[test]
    fn every_site_check_control_bites() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/carousel-assembly/carousel-v0a.txt");
        let c = circuit::Circuit::load(&path).unwrap();
        let spec = circuit::mobfarm::spec_of(&c).unwrap();
        let p = place(&c, &spec, None, &[], true, None, None).unwrap();
        // (the default AFK spot is the nearest legal one: 2 sections from the shared bed's)
        let bed = p.shared_bed.expect("v0a's single bed");
        assert!(sec_dist(cell_of(p.afk), bed) >= 2, "{}", p.afk_note);
        let cs = controls(&p);
        for n in 1..=5u8 {
            assert!(cs.iter().any(|(k, _)| *k == n), "condition {n} has controls");
        }
        let idle: Vec<String> = cs.iter().filter(|(_, c)| !c.bites).map(|(n, c)| format!("{n}: {} ({})", c.name, c.why)).collect();
        assert!(idle.is_empty(), "controls that do not bite: {idle:?}");
        // pickup from a runner trace: only the cells an empty cart passes at 0.1 or faster count;
        // every track slow, no pickup at all
        let slow: Vec<(BlockPos, f64)> = p.tracks.iter().map(|t| (*t, 0.05)).collect();
        let q = place(&c, &spec, None, &[], true, None, Some(&slow)).unwrap();
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

    /// The dug-in spot finder in plains (the parent's ruling 2026-11-11): the cheapest spot keeps
    /// lightning rule (b) (every box 1 block out of every surface strike's damage box, no farm
    /// entity seeing the sky, no strike's fire reaching a flammable farm block); rule (b) never
    /// puts the farm higher than 1-block cover does; and `--at` says it is not the judge's.
    #[test]
    fn dug_in_keeps_rule_b() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/farms/iron/carousel-assembly/carousel-v0a.txt");
        let c = circuit::Circuit::load(&path).unwrap();
        let spec = circuit::mobfarm::spec_of(&c).unwrap();
        let b = dug_in(&c, &spec, "plains", Cover::B, &[], true).unwrap();
        let one = dug_in(&c, &spec, "plains", Cover::OneBlock, &[], true).unwrap();
        let low = |d: &DugIn| d.cands.iter().map(|c| c.price.price.offset[1]).max().unwrap();
        assert!(low(&b) <= low(&one), "rule (b) is at least as deep as 1-block cover");
        let (site, label, dig) = b.site(&spec, 0).unwrap();
        let p = place_in(&c, &spec, Some((site, label, Some(dig))), &[], true, None, None).unwrap();
        let v = lightning(&p);
        assert!(v.pass(), "{:?}", v.lines);
        assert!(p.label.contains("dug-in spot") && p.label.contains("build+dig minutes"), "{}", p.label);
        let s = redsim::terrain::samples::sample("plains").unwrap();
        let q = place(&c, &spec, Some(("plains", Spot::At(s.near[0], s.near[1]))), &[], true, None, None).unwrap();
        assert!(q.label.contains("fixed spot, not the judge's") && q.label.contains("relief"), "{}", q.label);
    }
}
