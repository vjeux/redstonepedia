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
        let spot = place::best_spot(&world, size, ground - lo.y, sample.near, sample.radius).ok_or_else(|| format!("no spot for a {}x{} footprint {} deep in {}", size[0], size[1], ground - lo.y, sample.name))?;
        let offset = [spot.lo_xz[0] - lo.x, spot.ground - ground, spot.lo_xz[1] - lo.z];
        let at = |p: BlockPos| BlockPos::new(p.x + offset[0], p.y + offset[1], p.z + offset[2]);
        let dug = if clear_box {
            place::footprint_cells(&world, at(lo), at(hi)).into_iter().filter(|p| !place::air_like(&world.get(*p))).count() as u32
        } else {
            spec.blocks.iter().filter(|(p, _)| !place::air_like(&world.get(at(*p)))).count() as u32
        };
        let placed = spec.blocks.iter().filter(|(_, b)| !b.is_air()).count() as u32;
        let afk = Vec3::new(afk[0] + offset[0] as f64, afk[1] + offset[1] as f64, afk[2] + offset[2] as f64);
        Ok(Site { sample: sample.name, world: std::sync::Mutex::new(world), offset, clear_box, dug, placed, afk })
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
        w
    }
}
