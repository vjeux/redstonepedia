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
    /// lock so the judge's threads share one site (`World` is not `Sync`).
    pub world: std::sync::Mutex<World>,
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
    /// How many spots the build was priced at, and their build times' range (seconds).
    pub spots: usize,
    pub build_range: (f64, f64),
}

/// The spots priced (brief revision 8: the spot is a player's choice, the cheapest of the nearest).
pub const SPOTS: usize = 12;

/// A build priced at one spot.
struct Built {
    offset: [i32; 3],
    dug: u32,
    placed: u32,
    fills: Vec<BlockPos>,
}

impl Built {
    /// Its blocks' share of the build time (brief revision 8: 2 s a block placed, 1 s dug).
    fn seconds(&self) -> f64 {
        2.0 * (self.placed + self.fills.len() as u32) as f64 + self.dug as f64
    }
}

/// The valid min columns for a footprint near `near` (level ground within 2, no refused natural
/// block: `place::refusal`), the `k` nearest, each with its ground layer.
fn valid_spots(w: &World, size: [i32; 2], below: i32, near: [i32; 2], radius: i32, k: usize) -> Vec<([i32; 2], i32)> {
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

/// The build of `spec` at `offset`: its blocks written (the box cleared first for a void design),
/// its entities in, the natural cells the panicking villagers' golem search could spawn a golem
/// on covered with glass (excluded by LEGACY_IRON_GOLEM: one block a spot, nothing new above it)
/// until only the farm's own floors are left.
fn build_at(world: &World, spec: &FarmSpec, offset: [i32; 3], clear_box: bool) -> Built {
    let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
    let (lo, hi) = blocks_box(spec);
    let mut w = world.clone();
    let dug = if clear_box {
        let d = place::footprint_cells(&w, at(lo), at(hi)).into_iter().filter(|p| !place::air_like(&w.get(*p))).count() as u32;
        place::clear(&mut w, at(lo), at(hi));
        d
    } else {
        spec.blocks.iter().filter(|(p, _)| !place::air_like(&w.get(at(*p)))).count() as u32
    };
    let placed = spec.blocks.iter().filter(|(_, b)| !b.is_air()).count() as u32;
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
    Built { offset, dug, placed, fills }
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
    /// file (read relative to the working directory, the repository root), the spot for the
    /// footprint of its blocks reaching `ground - lo.y` layers under the ground layer (`ground`:
    /// the design's y that lies at the ground layer), and the blocks dug and placed there. `afk`:
    /// the player's feet in design coordinates.
    pub fn new(sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        let bytes = std::fs::read(sample.file).map_err(|e| format!("terrain sample {}: {e}", sample.file))?;
        let t = crate::terrain::format::Terrain::decode(&bytes)?;
        Site::from_terrain(&t, sample, spec, ground, afk, clear_box)
    }

    /// [`Site::new`] from the sample's decoded file (the 3D viewer, compiled to wasm, reads no file).
    pub fn from_terrain(t: &crate::terrain::format::Terrain, sample: &'static Sample, spec: &FarmSpec, ground: i32, afk: [f64; 3], clear_box: bool) -> Result<Site, String> {
        let world = crate::terrain::land::build_world(t)?;
        if spec.blocks.is_empty() {
            return Err("an empty design".into());
        }
        let (lo, hi) = blocks_box(spec);
        let size = [hi.x - lo.x + 1, hi.z - lo.z + 1];
        let below = ground - lo.y;
        // the spots a player would take near the sample's start: the SPOTS nearest valid ones
        // (level ground, no refused natural block), each priced as built
        let spots = valid_spots(&world, size, below, sample.near, sample.radius, SPOTS);
        if spots.is_empty() {
            return Err(format!("no spot for a {}x{} footprint {below} deep in {}", size[0], size[1], sample.name));
        }
        let mut priced: Vec<(f64, Built)> = Vec::new();
        for (lo_xz, g) in spots {
            let offset = [lo_xz[0] - lo.x, g - ground, lo_xz[1] - lo.z];
            let b = build_at(&world, spec, offset, clear_box);
            priced.push((b.seconds(), b));
        }
        let range = (priced.iter().map(|p| p.0).fold(f64::INFINITY, f64::min), priced.iter().map(|p| p.0).fold(0.0, f64::max));
        let k = priced.len();
        let (_, best) = priced.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).expect("a spot");
        let offset = best.offset;
        let afk = Vec3::new(afk[0] + offset[0] as f64, afk[1] + offset[1] as f64, afk[2] + offset[2] as f64);
        Ok(Site { sample: sample.name, world: std::sync::Mutex::new(world), offset, clear_box, dug: best.dug, placed: best.placed, afk, fills: best.fills, spots: k, build_range: range })
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
        let stone: Vec<(BlockPos, crate::block::Block)> = self.fills.iter().map(|p| (*p, crate::block::Block::Simple(crate::block::Simple::Glass))).collect();
        w.place_raw(&stone);
        w
    }
}
