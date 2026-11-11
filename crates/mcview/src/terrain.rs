//! A farm in its terrain sample (vjeux 2026-11-08 04:22: "The day 1 farm needs to run in a real
//! looking terrain, not a void world"): the sample's world (`redsim::terrain`: the official
//! server's blocks, light, heightmaps and biomes), the farm built into it as its judge builds it
//! (`redsim::entity::mob::site::Site`: the spot `place::best_spot` picks, the box cleared for a
//! void design or only its listed cells written for a design made for terrain), and what the view
//! draws of the landscape: every natural block with a face the eye can reach, lit by the sample's
//! own light and tinted by its biomes.

use crate::design::Design;
use redsim::entity::mob::farm::FarmSpec;
use redsim::entity::mob::site::Site;
use redsim::terrain::samples::Sample;
use redsim::{Block, BlockPos, World};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Whether the browser runs a farm in its terrain: yes since redsim's chunk sections keep their
/// randomly ticking counts (3c0384869: the judge's 200 settle ticks of D1U in village-edge 62 ms
/// natively, a step 0.11 ms); false would show the farm as built in its terrain ([`AS_BUILT`]).
pub const RUNS_IN_BROWSER: bool = true;
pub const AS_BUILT: &str = "the farm as built in its terrain: its run in the terrain is too slow for the browser yet";

/// A farm built into its terrain sample, as the view keeps it (one per design: the stills and the
/// runs of a page's view share it): the judge's site, the farm's box in the sample's world, and
/// the landscape the view draws around it (the sample's world with the farm's box cleared or its
/// cells written, outside that box).
pub struct TerrainFarm {
    pub site: &'static Site,
    /// The farm's box in the sample's world (the moved spec's box: what the farm's view draws from
    /// the world itself, tick by tick).
    pub farm_box: (BlockPos, BlockPos),
    /// The natural blocks drawn around it: world cell and the game's state string.
    pub landscape: Vec<([i32; 3], String)>,
    /// The hidden natural cells next to them (world cells).
    pub hidden: HashSet<[i32; 3]>,
    /// Each column's biome colours (grass, foliage, water) at the farm's ground (world x, z).
    pub biomes: Rc<HashMap<[i32; 2], [[u8; 3]; 3]>>,
    /// The lowest y drawn (world): nothing under the farm's box.
    pub min_y: i32,
    /// The cutaway (world z): no natural block north of the farm's front (z under this), so the
    /// farm dug into the sample shows from that side; in each row (x, y) of the farm's own
    /// cells, none north of its first cell there ([`TerrainFarm::cut_away`]): the farm stands
    /// out of the cut face in relief, the rock around and behind it.
    pub cut_z: i32,
    pub cut_rows: HashMap<[i32; 2], i32, crate::camera::FxBuild>,
    /// The columns the landscape is drawn in (world x and z, inclusive): within its reach of the
    /// farm's middle ([`LANDSCAPE_REACH`]).
    pub reach_xz: ([i32; 2], [i32; 2]),
    /// [`TerrainFarm::landscape`] by cell (its index there), made the first time it is asked
    /// ([`TerrainFarm::landscape_at`]).
    pub landscape_index: std::cell::OnceCell<HashMap<[i32; 3], u32, crate::camera::FxBuild>>,
}

impl TerrainFarm {
    /// The natural block drawn at world cell `c` as the landscape had it (None: none drawn there).
    pub fn landscape_at(&self, c: [i32; 3]) -> Option<&str> {
        let ix = self.landscape_index.get_or_init(|| self.landscape.iter().enumerate().map(|(k, (p, _))| (*p, k as u32)).collect());
        ix.get(&c).map(|&k| self.landscape[k as usize].1.as_str())
    }

    /// Design coordinates to the sample's world: add it; the world's to the design's: subtract it.
    pub fn offset(&self) -> [i32; 3] {
        self.site.offset
    }

    /// Whether world cell `p` is in the part of the sample the view draws (its columns within the
    /// landscape's reach, from its lowest y up): a block the run changes anywhere else (a crop
    /// growing in the village 50 blocks off, an amethyst bud deep under the farm) is not drawn.
    pub fn draws(&self, p: [i32; 3]) -> bool {
        let ((x0, x1), (z0, z1)) = ((self.reach_xz.0[0], self.reach_xz.0[1]), (self.reach_xz.1[0], self.reach_xz.1[1]));
        p[0] >= x0 && p[0] <= x1 && p[2] >= z0 && p[2] <= z1 && p[1] >= self.min_y
    }

    /// Whether the cutaway removes the natural block at world cell `p` (north of the farm's front,
    /// or of the farm's first cell in its row).
    pub fn cut_away(&self, p: [i32; 3]) -> bool {
        cut_away(&self.cut_rows, self.cut_z, p)
    }

    /// The biome colours of a design cell's column for the mesh ([`crate::mesh::Builder::biome_tint`]):
    /// the game's grass, foliage and water colours of its biome where a model carries the plains'.
    pub fn tint_fn(&self) -> crate::mesh::BiomeTintFn {
        use crate::gamemodel::{FOLIAGE_TINT, GRASS_TINT, WATER_TINT};
        let map = self.biomes.clone();
        let off = self.offset();
        Rc::new(move |c: [i32; 3], t: [u8; 3]| {
            let cols = map.get(&[c[0] + off[0], c[2] + off[2]])?;
            if t == GRASS_TINT {
                Some(cols[0])
            } else if t == FOLIAGE_TINT {
                Some(cols[1])
            } else if t == WATER_TINT {
                Some(cols[2])
            } else {
                None
            }
        })
    }
}

/// [`TerrainFarm::cut_away`] from its parts.
fn cut_away(rows: &HashMap<[i32; 2], i32, crate::camera::FxBuild>, cut_z: i32, p: [i32; 3]) -> bool {
    p[2] < rows.get(&[p[0], p[1]]).copied().unwrap_or(cut_z)
}

/// Each column's biome colours (grass, foliage, water) of `w` at height `y`: the client's
/// (`Biome.getGrassColor` through its modifier; a modifier redsim does not model: the biome's
/// colour without it).
fn biome_colours(w: &World, y: i32) -> HashMap<[i32; 2], [[u8; 3]; 3]> {
    let rgb = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8];
    let mut out = HashMap::new();
    for z in w.min.z..w.min.z + w.size[2] {
        for x in w.min.x..w.min.x + w.size[0] {
            let Some(b) = w.biome_at(BlockPos::new(x, y, z)).and_then(redsim::terrain::biome::biome) else { continue };
            let grass = b.grass_color(x as f64, z as f64).unwrap_or(b.grass);
            out.insert([x, z], [rgb(grass), rgb(b.foliage), rgb(b.water())]);
        }
    }
    out
}

thread_local! {
    /// The samples' files as given ([`provide_sample`]: the browser fetches them), by name.
    static SAMPLES: RefCell<HashMap<String, Rc<Vec<u8>>>> = RefCell::new(HashMap::new());
    /// Each design's terrain farm once made (by its text).
    static FARMS: RefCell<HashMap<String, Rc<TerrainFarm>>> = RefCell::new(HashMap::new());
    /// How far from the farm's middle the landscape is drawn (blocks, along x and z: [`REACH`]).
    static REACH: std::cell::Cell<i32> = const { std::cell::Cell::new(LANDSCAPE_REACH) };
}

/// How far from the farm's middle the landscape is drawn, along x and z (blocks): a diorama of the
/// sample around the farm. Its whole width (village-edge's 120: 657,000 vertices for D1U) took
/// WebKit's software GL 66-113 ms a frame, 32 blocks (210,000) 32-41 ms, this reach (142,000) less.
pub const LANDSCAPE_REACH: i32 = 24;

/// Draw the landscape only this far from the farm's middle from now on (`mctas web` measures).
pub fn set_reach(blocks: i32) {
    REACH.with(|r| r.set(blocks));
    FARMS.with(|m| m.borrow_mut().clear());
}

/// A sample's file, given by the page (the browser fetched it): its farms can be made.
pub fn provide_sample(name: &str, bytes: Vec<u8>) {
    SAMPLES.with(|s| s.borrow_mut().insert(name.to_string(), Rc::new(bytes)));
}

/// A sample's file: given ([`provide_sample`]) or, natively, read from the checkout (the
/// repository root or this crate's).
pub fn sample_bytes(s: &Sample) -> Option<Rc<Vec<u8>>> {
    if let Some(b) = SAMPLES.with(|m| m.borrow().get(s.name).cloned()) {
        return Some(b);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        for path in [s.file.to_string(), format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), s.file)] {
            if let Ok(b) = std::fs::read(&path) {
                let b = Rc::new(b);
                SAMPLES.with(|m| m.borrow_mut().insert(s.name.to_string(), b.clone()));
                return Some(b);
            }
        }
    }
    None
}

/// The terrain farm of a design (`text`: its text, the key) whose judge's farm is `spec`: None for
/// a void design or while its sample's file is not there yet; made once, then shared.
pub fn farm_for(text: &str, d: &Design, spec: &FarmSpec) -> Result<Option<Rc<TerrainFarm>>, String> {
    crate::staged::block_on(farm_for_staged(text, d, spec, None, &crate::staged::Budget::none()))
}

/// A part shown in its terrain (ironspec 2026-11-10 on the dug-only drop shaft: "keep it, its file
/// carrying the terrain sample and its dug cells as air, so the view shows the shaft dug into the
/// ground"): a design that is no mob farm, with `terrain:` and `site: x,y,z` (its reference farm's
/// site offset, the judge's spot for that farm): its cells (the dug ones as air) at that offset in
/// the sample's world. Nothing of a farm runs in it.
pub fn is_part_in_terrain(d: &Design) -> bool {
    !crate::mobfarm::is_mob_farm(d) && sample_of(d).is_some() && site_meta(d).is_some()
}

/// A design's `site: x,y,z` (or `x y z`): the offset of its coordinates in its terrain sample.
pub fn site_meta(d: &Design) -> Option<[i32; 3]> {
    let v: Vec<i32> = d.meta.get("site")?.split([',', ' ']).filter(|t| !t.is_empty()).map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
    let [x, y, z] = v[..] else { return None };
    Some([x, y, z])
}

/// A part's cells as a farm spec for its terrain ([`is_part_in_terrain`]): its blocks (the dug ones
/// as air), no entity, its box with the judge's margin; its output an unused cell (nothing runs).
pub fn part_spec(d: &Design) -> Result<FarmSpec, String> {
    let mut blocks = Vec::with_capacity(d.blocks.len());
    for (q, s) in &d.blocks {
        blocks.push((BlockPos::new(q[0], q[1], q[2]), crate::mobfarm::farm_block(s).map_err(|e| format!("not modeled by redsim: {e}"))?));
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let (lo, hi) = d.bounds().ok_or("an empty design")?;
    let m = crate::mobfarm::WATCH_MARGIN;
    let block_entities = d.block_entities.iter().map(|(p, s)| (BlockPos::new(p[0], p[1], p[2]), s.clone())).collect();
    Ok(FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(lo[0], lo[1], lo[2]), lo: BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m), hi: BlockPos::new(hi[0] + m, hi[1] + m, hi[2] + m), block_entities, rock: d.meta.get("terrain").map(|t| t.trim()) == Some("rock"), nether: None, land: false })
}

/// A part's terrain ([`is_part_in_terrain`]): the sample's site at its `site:` offset alone (priced
/// there as the judge prices a spot: the seals its dug cells need), made once, then shared.
pub fn part_for(text: &str, d: &Design) -> Result<Option<Rc<TerrainFarm>>, String> {
    let spec = part_spec(d)?;
    crate::staged::block_on(farm_for_staged(text, d, &spec, site_meta(d), &crate::staged::Budget::none()))
}

/// A part's world in its terrain: the site's world (its seals) with the part's cells written raw
/// (a dug cell: air), then its `ghost:` cells (its reference farm's blocks of other parts inside
/// its box: the chamber's fence gates in the drop's column), in the world's coordinates.
pub fn part_world(t: &TerrainFarm, spec: &FarmSpec, ghosts: &[(crate::design::Pos, String)]) -> Result<World, String> {
    let moved = t.site.spec(spec);
    let mut w = t.site.world_for(&moved);
    w.place_raw(&moved.blocks);
    let o = t.offset();
    let mut placed = Vec::with_capacity(ghosts.len());
    for (p, s) in ghosts {
        placed.push((BlockPos::new(p[0] + o[0], p[1] + o[1], p[2] + o[2]), crate::mobfarm::farm_block(s).map_err(|e| format!("its ghost at {p:?}: not modeled by redsim: {e}"))?));
    }
    w.place_raw(&placed);
    Ok(w)
}

/// Whether a design's terrain farm is made already (or it needs none): a live view makes it over
/// frames first ([`farm_for_staged`]), then its still and its run take it from here at once.
pub fn farm_ready(text: &str, d: &Design) -> bool {
    sample_of(d).is_none() || FARMS.with(|m| m.borrow().contains_key(text))
}

/// The design's site offset in its terrain sample as the view finds it (the judge's spot: the
/// cheapest of its spots, [`priced_staged`]), worked out natively ahead by the site build for the
/// page's `data-site` (the view then prices that spot alone: [`farm_for_staged`]'s `chosen`);
/// None: a void design or no sample file.
pub fn site_offset(d: &Design, spec: &FarmSpec) -> Result<Option<[i32; 3]>, String> {
    crate::staged::block_on(async {
        let Some((world, sample, fp, ground, _, clear_box)) = sample_world_staged(d, spec, &crate::staged::Budget::none()).await? else { return Ok(None) };
        let priced = priced_staged(&world, sample, spec, fp, ground, clear_box, None, &crate::staged::Budget::none()).await?;
        Ok(priced.first().map(|p| p.price.offset))
    })
}

/// A design's terrain sample world built (decoded, then built a slice at a time: `land::Builder`)
/// with the site's parameters (its footprint, ground layer, AFK spot, whether its box is cleared);
/// None: a void design or no sample file.
#[allow(clippy::type_complexity)]
async fn sample_world_staged(d: &Design, spec: &FarmSpec, b: &crate::staged::Budget) -> Result<Option<(World, &'static Sample, redsim::entity::mob::site::Footprint, i32, [f64; 3], bool)>, String> {
    use redsim::entity::mob::site;
    let meta = |k: &str| d.meta.get(k).map(String::as_str);
    let Some(sample) = site::sample_of(meta("terrain"), meta("sample"))? else { return Ok(None) };
    let Some(bytes) = sample_bytes(sample) else { return Ok(None) };
    let (ground, afk, clear_box) = site::params(spec, meta("ground"), meta("afk"), meta("terrain"))?;
    let t = redsim::terrain::format::Terrain::decode(&bytes)?;
    b.pause().await;
    let mut builder = redsim::terrain::land::Builder::new(&t)?;
    while !builder.step(BUILD_STEP) {
        b.pause().await;
    }
    let world = builder.finish();
    drop(t);
    b.pause().await;
    let fp = site::footprint(spec, ground)?;
    Ok(Some((world, sample, fp, ground, afk, clear_box)))
}

/// The design's spots priced the judge's way (`Site::with_shared`'s calls in its order: the spots,
/// each priced, the cheapest first, ties in the spots' order), a spot a slice; `chosen`: the spot
/// at that site offset alone (the page's `data-site`, the site build's [`site_offset`]): each spot
/// is priced on its own (`site::price_spots`, `site::Priced::at`), so the cheapest's price is the
/// same and the site built on it the same (test `a_chosen_site_is_the_searched_one`).
#[allow(clippy::too_many_arguments)]
async fn priced_staged(world: &World, sample: &Sample, spec: &FarmSpec, fp: redsim::entity::mob::site::Footprint, ground: i32, clear_box: bool, chosen: Option<[i32; 3]>, b: &crate::staged::Budget) -> Result<Vec<redsim::entity::mob::site::Priced>, String> {
    use redsim::entity::mob::site;
    // (`site::price_spots`: a spot's offset is [x - fp.lo.x, y - ground, z - fp.lo.z])
    let spots = match chosen {
        Some(o) => vec![([o[0] + fp.lo.x, o[2] + fp.lo.z], o[1] + ground)],
        None => site::spots_for_spec(world, sample, spec, fp, clear_box),
    };
    if spots.is_empty() {
        return Err(format!("no spot for a {}x{} footprint {} deep in {}", fp.size[0], fp.size[1], fp.below, sample.name));
    }
    b.pause().await;
    let mut priced = Vec::new();
    for price in site::price_spots(world, spec, fp, ground, clear_box, &spots) {
        b.pause().await;
        priced.push(site::Priced::at(world, spec, price, clear_box));
    }
    // (`site::price_all`'s order: cheapest first, ties in the spots' order)
    priced.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
    Ok(priced)
}

/// [`farm_for`] as staged work (`crate::staged`): the sample decoded, its world built a slice at a
/// time (`land::Builder`), the judge's spot search and pricing a spot at a time (`site`: the same
/// calls `Site::with_shared` makes, in its order; `chosen`: the page's `data-site`, that spot alone
/// priced), the world the farm is built in, its landscape layer by layer and its biomes, pausing
/// between them when the budget's time is up (no browser task over ~50 ms: the coordinator
/// 2026-10-22).
pub async fn farm_for_staged(text: &str, d: &Design, spec: &FarmSpec, chosen: Option<[i32; 3]>, b: &crate::staged::Budget) -> Result<Option<Rc<TerrainFarm>>, String> {
    if sample_of(d).is_none() {
        return Ok(None);
    }
    if let Some(f) = FARMS.with(|m| m.borrow().get(text).cloned()) {
        return Ok(Some(f));
    }
    let Some(f) = build_farm_staged(d, spec, chosen, b).await? else { return Ok(None) };
    let f = Rc::new(f);
    FARMS.with(|m| m.borrow_mut().insert(text.to_string(), f.clone()));
    Ok(Some(f))
}

/// [`farm_for_staged`]'s work, not kept.
async fn build_farm_staged(d: &Design, spec: &FarmSpec, chosen: Option<[i32; 3]>, b: &crate::staged::Budget) -> Result<Option<TerrainFarm>, String> {
    let Some((world, sample, fp, ground, afk, clear_box)) = sample_world_staged(d, spec, b).await? else { return Ok(None) };
    let priced = priced_staged(&world, sample, spec, fp, ground, clear_box, chosen, b).await?;
    let site: &'static Site = Box::leak(Box::new(Site::at_priced(std::sync::Arc::new(std::sync::RwLock::new(world)), sample, priced, 0, afk, clear_box)));
    b.pause().await;
    let moved = site.spec(spec);
    let world = site.world_for(&moved);
    b.pause().await;
    let farm_box = (moved.lo, moved.hi);
    // (the caves under the farm are out of sight: nothing under its box's bottom is drawn; the
    // cutaway at the farm's front, and in each row of its cells at the first one)
    let min_y = moved.lo.y;
    let cut_z = moved.blocks.iter().map(|(p, _)| p.z).min().unwrap_or(moved.lo.z);
    let mut cut_rows: HashMap<[i32; 2], i32, crate::camera::FxBuild> = HashMap::default();
    for (p, _) in &moved.blocks {
        let e = cut_rows.entry([p.x, p.y]).or_insert(p.z);
        *e = (*e).min(p.z);
    }
    let (cx, cz) = ((moved.lo.x + moved.hi.x) / 2, (moved.lo.z + moved.hi.z) / 2);
    let reach = REACH.with(|r| r.get());
    let far = |p: BlockPos| (p.x - cx).abs() > reach || (p.z - cz).abs() > reach;
    let (shown, hidden) = landscape_staged(&world, farm_box, min_y, cut_z, &|p| cut_away(&cut_rows, cut_z, p.arr()) || far(p), b).await;
    let landscape = shown.into_iter().map(|(p, b)| (p.arr(), b.to_string())).collect();
    let hidden = hidden.into_iter().map(|p| p.arr()).collect();
    b.pause().await;
    // (the biomes at the farm's ground: the site's ground layer)
    let biomes = Rc::new(biome_colours(&world, site.offset[1] + ground));
    Ok(Some(TerrainFarm { site, farm_box, landscape, hidden, biomes, min_y, cut_z, cut_rows, reach_xz: ([cx - reach, cx + reach], [cz - reach, cz + reach]), landscape_index: Default::default() }))
}

/// The terrain sample a design is judged in: the judge's rule (`site::sample_of`: its `sample:`
/// meta, else village-edge for a design with `terrain:`); None: a void design (or a sample name
/// the judge refuses: [`farm_for`] says why).
pub fn sample_of(d: &Design) -> Option<&'static Sample> {
    let meta = |k: &str| d.meta.get(k).map(String::as_str);
    redsim::entity::mob::site::sample_of(meta("terrain"), meta("sample")).ok().flatten()
}

/// Cells of the sample's world built per slice ([`farm_for_staged`]: ~3 ms natively).
const BUILD_STEP: usize = 500_000;

/// Whether a block hides the faces of the blocks next to it (a natural state the game renders as
/// a solid cube: `isSolidRender`). Anything else (air, a fluid, a plant, the farm's own blocks)
/// leaves its neighbours' faces to the eye.
pub fn hides_neighbours(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => redsim::terrain::blocks::props(*id).solid_render,
        _ => false,
    }
}

/// The natural blocks the view draws: every non-air cell of `w` from `min_y` up and from `cut_z` on
/// that the cutaway keeps (`cut`: true where it removes one: the sample cut through the farm's
/// front, so a farm dug into the ground shows), outside the box `skip` (the farm's: drawn with the
/// farm), with a neighbour that leaves a face open, and the hidden cells next to them (they still
/// hide those faces and darken their corners). Below `min_y` (under the farm) nothing is drawn:
/// the caves there are out of sight.
pub fn landscape(w: &World, skip: (BlockPos, BlockPos), min_y: i32, cut_z: i32, cut: &dyn Fn(BlockPos) -> bool) -> (Vec<(BlockPos, Block)>, HashSet<BlockPos>) {
    crate::staged::block_on(landscape_staged(w, skip, min_y, cut_z, cut, &crate::staged::Budget::none()))
}

/// [`landscape`] as staged work: a pause (when the budget's time is up) after each layer.
pub async fn landscape_staged(w: &World, skip: (BlockPos, BlockPos), min_y: i32, cut_z: i32, cut: &dyn Fn(BlockPos) -> bool, budget: &crate::staged::Budget) -> (Vec<(BlockPos, Block)>, HashSet<BlockPos>) {
    let (lo, hi) = skip;
    let inside = |p: BlockPos| p.x >= lo.x && p.y >= lo.y && p.z >= lo.z && p.x <= hi.x && p.y <= hi.y && p.z <= hi.z;
    let (min, size) = (w.min, w.size);
    const NEXT: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    let mut shown = Vec::new();
    let mut hidden: HashSet<BlockPos> = HashSet::new();
    let y0 = (min_y - min.y).clamp(0, size[1]);
    let z0 = (cut_z - min.z).clamp(0, size[2]);
    for y in y0..size[1] {
        budget.pause().await;
        for z in z0..size[2] {
            for x in 0..size[0] {
                let p = min.offset(x, y, z);
                // (the cheap checks first: most cells are air, the cutaway's rows a map's)
                let b = w.get(p);
                if b == Block::Air || inside(p) || cut(p) {
                    continue;
                }
                // (the box's edges: open to the sides and above, closed below: the sample's floor;
                // the cut under the farm: closed; the cutaway in front: open)
                let open = NEXT.iter().any(|d| {
                    let q = p.offset(d[0], d[1], d[2]);
                    let outside_box = q.x < min.x || q.z < min.z || q.x >= min.x + size[0] || q.z >= min.z + size[2] || q.y >= min.y + size[1];
                    if q.y < min.y || q.y < min_y {
                        return false;
                    }
                    outside_box || inside(q) || !hides_neighbours(&w.get(q)) || cut(q)
                });
                if open {
                    shown.push((p, b));
                }
            }
        }
    }
    budget.pause().await;
    let shown_set: HashSet<BlockPos, crate::camera::FxBuild> = shown.iter().map(|(p, _)| *p).collect();
    budget.pause().await;
    for (k, (p, _)) in shown.iter().enumerate() {
        if k % 4096 == 0 {
            budget.pause().await;
        }
        for d in NEXT {
            let q = p.offset(d[0], d[1], d[2]);
            if hides_neighbours(&w.get(q)) && !inside(q) && !cut(q) && !shown_set.contains(&q) {
                hidden.insert(q);
            }
        }
    }
    (shown, hidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page's `data-site` (the site build's [`site_offset`]: the judge's spot, found ahead) gives
    /// the view the very farm its own search finds: the same site (offset, blocks placed and dug,
    /// AFK spot, glass covers, seals, fire clearance), the same landscape and hidden cells, for
    /// every farm judged in terrain on the site (each spot is priced on its own).
    #[test]
    fn a_chosen_site_is_the_searched_one() {
        let root = format!("{}/../../designs/farms/iron", env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        for stage in ["day-1", "middle", "late"] {
            let Ok(dir) = std::fs::read_dir(format!("{root}/{stage}")) else { continue };
            let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
            files.sort();
            for path in files {
                let text = std::fs::read_to_string(&path).unwrap();
                let Ok(d) = Design::parse(&text) else { continue };
                if sample_of(&d).is_none() {
                    continue;
                }
                let Ok(spec) = crate::mobfarm::spec(&d) else { continue };
                let none = crate::staged::Budget::none();
                let Some(offset) = site_offset(&d, &spec).unwrap() else { continue };
                let searched = crate::staged::block_on(build_farm_staged(&d, &spec, None, &none)).unwrap().unwrap();
                let chosen = crate::staged::block_on(build_farm_staged(&d, &spec, Some(offset), &none)).unwrap().unwrap();
                let (a, b) = (searched.site, chosen.site);
                let name = path.display();
                assert_eq!(a.offset, offset, "{name}: the searched site's offset");
                assert_eq!((b.offset, b.dug, b.placed, b.clear_box), (a.offset, a.dug, a.placed, a.clear_box), "{name}");
                assert_eq!([b.afk.x, b.afk.y, b.afk.z], [a.afk.x, a.afk.y, a.afk.z], "{name}: the AFK spot");
                assert_eq!((&b.fills, &b.seals, &b.clears), (&a.fills, &a.seals, &a.clears), "{name}: covers, seals, clearance");
                assert_eq!((chosen.farm_box, chosen.min_y, chosen.cut_z, chosen.reach_xz), (searched.farm_box, searched.min_y, searched.cut_z, searched.reach_xz), "{name}");
                assert_eq!(chosen.landscape, searched.landscape, "{name}: the landscape");
                assert_eq!(chosen.hidden, searched.hidden, "{name}: the hidden cells");
                checked += 1;
            }
        }
        assert!(checked >= 10, "{checked} farms in terrain checked");
    }

    fn read(path: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// vjeux 2026-11-08 ("The day 1 farm needs to run in a real looking terrain, not a void
    /// world"): a farm judged in its terrain sample shows in it: its site is the judge's (the view
    /// prices the spots a slice at a time: the same spot, blocks dug, placed and covered as
    /// `Site::from_terrain`), its still is the judge's world before its run with the landscape
    /// around it in the design's coordinates, and its run plays in it.
    #[test]
    fn a_farm_judged_in_terrain_shows_in_it() {
        let text = String::from_utf8(read("designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        let spec = crate::mobfarm::spec(&d).unwrap();
        let sample = sample_of(&d).expect("D1U is judged in terrain");
        let t = farm_for(&text, &d, &spec).unwrap().expect("its sample's file is in the checkout");
        let meta = |k: &str| d.meta.get(k).map(String::as_str);
        let (ground, afk, clear_box) = redsim::entity::mob::site::params(&spec, meta("ground"), meta("afk"), meta("terrain")).unwrap();
        let tf = redsim::terrain::format::Terrain::decode(&sample_bytes(sample).unwrap()).unwrap();
        let judge = Site::from_terrain(&tf, sample, &spec, ground, afk, clear_box).unwrap();
        let key = |s: &Site| format!("{:?} {} {} {:?} {:?}", s.offset, s.dug, s.placed, s.fills, s.afk);
        assert_eq!(key(t.site), key(&judge), "the view's site is the judge's");
        let s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        assert!(s.terrain.is_some() && s.unsimulated.is_none(), "{:?}", s.unsimulated);
        let blocks = s.render_blocks(0.0);
        // the farm's chest where the design puts it, the landscape around (grass), all in the
        // design's coordinates
        assert!(blocks.iter().any(|b| b.cell == [1, -15, 3] && b.state.starts_with("minecraft:chest")), "the chest at its port");
        assert!(blocks.iter().filter(|b| b.state.starts_with("minecraft:grass_block")).count() > 500, "the landscape's grass");
        let e = s.entities(0.0);
        assert!(e.iter().any(|e| e.kind.ends_with("villager") && (e.pos[0] - 0.5).abs() < 0.01 && (e.pos[2] - 0.5).abs() < 0.01), "a villager where the design puts it");
        // a card stays void
        let card = crate::session::Session::new(&text, &crate::session::Options { terrain: false, ..crate::session::Options::still() }).unwrap();
        assert!(card.terrain.is_none());
    }

    /// tick-exact 2026-11-10: in a terrain sample the village's and the natural mobs take the
    /// first entity ids, so the design's entities are counted from its first
    /// (`Session::design_base`): cross-4-underground's boat 17 was drawn as the zombie the
    /// design's list held at index 17 ("drawn differs from redsim: entity 17 (a boat) drawn as
    /// minecraft:zombie").
    #[test]
    fn a_terrain_farms_entities_are_drawn_as_their_kinds() {
        let text = String::from_utf8(read("designs/farms/iron/middle/cross-4-underground.txt")).unwrap();
        for o in [crate::session::Options::still(), crate::session::Options::live()] {
            let s = crate::session::Session::new(&text, &o).unwrap();
            // (its run: the judge's settle spawns natural mobs before the design's arrive)
            assert!(s.terrain.is_some() && (o.simulate == (s.design_base > 0)), "the settle's mobs come first in a run: {}", s.design_base);
            s.drawn_matches_simulation().unwrap();
            assert!(s.entities(0.0).iter().any(|e| e.kind.ends_with("_boat")), "its zombies' boats drawn as boats");
        }
    }

    /// A terrain view draws the mobs in the landscape it draws only (`TerrainFarm::draws`, the
    /// blocks' rule): an item a plant dropped past the landscape's reach floated in the sky
    /// (a67650c84's plants popping off their supports, cross-4-underground's run).
    #[test]
    fn a_mob_past_the_landscape_drawn_is_not_drawn() {
        let text = String::from_utf8(read("designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt")).unwrap();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let t = s.terrain.clone().expect("D1U in its terrain");
        let off = t.offset();
        let ([x0, x1], [_, z1]) = t.reach_xz;
        // (above the farm's box and south of its cutaway: drawn by the landscape's rule alone)
        let (y, z) = ((t.farm_box.1.y + 2) as f64, (z1 - 1) as f64 + 0.5);
        let mid = ((x0 + x1) / 2) as f64 + 0.5;
        let n0 = s.entities(0.0).len();
        redsim::entity::spawn(&mut s.world, "minecraft:zombie", [mid, y, z], None).unwrap();
        redsim::entity::spawn(&mut s.world, "minecraft:zombie", [(x0 - 3) as f64 + 0.5, y, z], None).unwrap();
        let e = s.entities(0.0);
        assert_eq!(e.len(), n0 + 1, "only the mob in the landscape drawn");
        assert!(e.iter().any(|e| (e.pos[0] - (mid as f32 - off[0] as f32)).abs() < 0.01 && (e.pos[2] - (z as f32 - off[2] as f32)).abs() < 0.01));
    }

    /// ironspec 2026-11-10 on drop-shaft-11 (a dug-only part: "keep it, its file carrying the
    /// terrain sample and its dug cells as air, so the view shows the shaft dug into the ground"):
    /// a part in its terrain is its cells cut into the sample's world at its reference farm's site
    /// (`site:`, the farm's page's), the landscape around; its cells air in the world, no farm runs.
    #[test]
    fn a_dug_part_shows_in_its_ground() {
        let text = String::from_utf8(read("designs/farms/iron/parts/transport/drop-shaft-11.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        assert!(is_part_in_terrain(&d) && site_meta(&d) == Some([-345, 65, 246]));
        // (its reference farm's page site: the judge's spot for bed-plate-lava-out-of-sight)
        let farm_text = String::from_utf8(read("designs/farms/iron/day-1/bed-plate-lava-out-of-sight.txt")).unwrap();
        let farm = Design::parse(&farm_text).unwrap();
        assert_eq!(site_offset(&farm, &crate::mobfarm::spec(&farm).unwrap()).unwrap(), site_meta(&d), "the part's site is its farm's");
        for o in [crate::session::Options::still(), crate::session::Options::live()] {
            let s = crate::session::Session::new(&text, &o).unwrap();
            let t = s.terrain.as_ref().expect("shown in its terrain");
            assert!(s.unsimulated.is_none() && s.mob.is_none(), "{:?}", s.unsimulated);
            assert_eq!(t.offset(), [-345, 65, 246]);
            // every dug cell is air in the world, natural blocks around (the shaft's walls)
            for (p, _) in &d.blocks {
                let q = BlockPos::new(p[0] - 345, p[1] + 65, p[2] + 246);
                assert!(matches!(s.world.get(q), Block::Air), "{p:?}: {}", s.world.get(q));
            }
            assert!(matches!(s.world.get(BlockPos::new(-1 - 345, -10 + 65, 4 + 246)), Block::Terrain(_)), "the shaft's west wall is the sample's ground");
            let blocks = s.render_blocks(0.0);
            assert!(blocks.iter().filter(|b| b.state.starts_with("minecraft:grass_block")).count() > 100, "the landscape's grass");
            assert!(!blocks.iter().any(|b| d.blocks.contains_key(&b.cell)), "nothing drawn in the shaft");
            // its reference farm's blocks of other parts inside its box (`ghost:` lines: the
            // chamber's fence gates and walls), placed as the farm has them and drawn see-through
            // (ironspec 2026-11-10: "not ... natural ground, or the shaft reads as blocked")
            assert!(d.ghosts.iter().filter(|(_, st)| st.starts_with("minecraft:oak_fence_gate")).count() >= 8, "{:?}", d.ghosts.len());
            let look = crate::scene::viewer_look(&s);
            for (p, st) in &d.ghosts {
                let q = BlockPos::new(p[0] - 345, p[1] + 65, p[2] + 246);
                assert_eq!(s.world.get(q), crate::mobfarm::farm_block(st).unwrap(), "{p:?}");
                if st.as_str() != "minecraft:air" {
                    assert!(look.ghost.contains(p) && blocks.iter().any(|b| b.cell == *p && b.state == *st), "{p:?} {st} drawn see-through");
                }
            }
        }
        // a void design with `site:` but no `terrain:` stays void; a mob farm never takes this path
        assert!(!is_part_in_terrain(&Design::parse("name: x\nsite: 1,2,3\npalette:\n  S stone\nlayer 0:\n  S\n").unwrap()));
        assert!(!is_part_in_terrain(&farm));
    }

    /// What a farm in its terrain costs in the view (`--nocapture`): its still, its run's session
    /// (the judge's settle) and its steps, natively.
    #[test]
    #[ignore]
    fn print_terrain_session_costs() {
        let text = String::from_utf8(read("designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt")).unwrap();
        for reach in [24, 32, 48, 64, 128] {
            set_reach(reach);
            let still = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
            let mut atlas = crate::atlas::Atlas::build();
            if let Ok((png, idx, defs, models)) = crate::tools::vanilla_assets() {
                let _ = atlas.apply_vanilla_png(&png, &idx);
                let _ = crate::tools::decode_png(&png).and_then(|(rgba, w, h)| atlas.set_game(&rgba, w, h, &idx, &defs, &models));
            }
            let look = crate::scene::default_look(&still);
            let mesh = loop {
                if let Some(m) = crate::scene::Staged::new(&still, 0.0, &look, crate::scene::Part::All).step(&atlas, &still, 0.0, &look, &mut || false) {
                    break m;
                }
            };
            eprintln!("reach {reach}: {} blocks drawn, {} solid vertices, {} translucent quads", still.render_blocks(0.0).len(), mesh.solid.len(), mesh.translucent.len());
        }
        set_reach(LANDSCAPE_REACH);
        let t0 = std::time::Instant::now();
        let still = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let t_still = t0.elapsed().as_secs_f64() * 1000.0;
        let blocks = still.render_blocks(0.0).len();
        {
            // the still's mesh, phase by phase (the game's models, as the browser draws them)
            let mut atlas = crate::atlas::Atlas::build();
            if let Ok((png, idx, defs, models)) = crate::tools::vanilla_assets() {
                let _ = atlas.apply_vanilla_png(&png, &idx);
                let _ = crate::tools::decode_png(&png).and_then(|(rgba, w, h)| atlas.set_game(&rgba, w, h, &idx, &defs, &models));
            }
            let look = crate::scene::default_look(&still);
            let t0 = std::time::Instant::now();
            let mut st = crate::scene::Staged::new(&still, 0.0, &look, crate::scene::Part::All);
            let t_new = t0.elapsed().as_secs_f64() * 1000.0;
            let mut slices = Vec::new();
            loop {
                let t0 = std::time::Instant::now();
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(4);
                let done = st.step(&atlas, &still, 0.0, &look, &mut || std::time::Instant::now() > deadline).is_some();
                slices.push(t0.elapsed().as_secs_f64() * 1000.0);
                if done {
                    break;
                }
            }
            let total: f64 = slices.iter().sum();
            eprintln!("the still's mesh: Staged::new {t_new:.1} ms, {} slices ({total:.0} ms), the first {:.1} ms, the longest after it {:.1} ms", slices.len(), slices[0], slices.iter().skip(1).cloned().fold(0.0, f64::max));
            // what a frame sends: the mesh's triangles in the live camera's view
            let mesh = loop {
                if let Some(m) = crate::scene::Staged::new(&still, 0.0, &look, crate::scene::Part::All).step(&atlas, &still, 0.0, &look, &mut || false) {
                    break m;
                }
            };
            for (w, h) in [(1232.0, 480.0), (399.0, 480.0)] {
                let (cam, _) = crate::scene::live_camera(&atlas, &still, &look, w, h);
                let t0 = std::time::Instant::now();
                let (solid, translucent) = crate::scene::triangles(&mesh, &cam);
                eprintln!("a frame at {w}x{h}: {} solid vertices of {} ({} translucent), {:.1} ms", solid.len(), mesh.solid.len(), translucent.len(), t0.elapsed().as_secs_f64() * 1000.0);
            }
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for (_, s) in &still.terrain.as_ref().unwrap().landscape {
                *by.entry(s.split('[').next().unwrap_or(s).to_string()).or_default() += 1;
            }
            let mut v: Vec<_> = by.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            eprintln!("the landscape's blocks: {:?}", &v[..v.len().min(14)]);
        }
        {
            let look = crate::scene::default_look(&still);
            let pts = crate::scene::view_points_for(&still, &look, 2.5);
            let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
            let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
            eprintln!("framed: {} points, {lo:?}..{hi:?}; design bounds {:?}", pts.len(), still.design.bounds());
        }
        let t0 = std::time::Instant::now();
        let still2 = crate::session::Session::new(&text, &crate::session::Options::still()).unwrap();
        let t_still2 = t0.elapsed().as_secs_f64() * 1000.0;
        drop(still2);
        // the terrain farm over slices, as the browser makes it (4 ms a slice): its longest
        {
            set_reach(LANDSCAPE_REACH);
            let d = crate::design::Design::parse(&text).unwrap();
            let spec = crate::mobfarm::spec(&d).unwrap();
            let base = std::time::Instant::now();
            let b = crate::staged::Budget::with_clock(Rc::new(move || base.elapsed().as_secs_f64() * 1000.0));
            let (t2, b2) = (text.clone(), b.clone());
            let mut task = crate::staged::Task::new(async move { farm_for_staged(&t2, &d, &spec, None, &b2).await.map(|f| f.is_some()) });
            let mut slices = Vec::new();
            loop {
                b.give(4.0);
                let t0 = std::time::Instant::now();
                let done = task.poll();
                slices.push(t0.elapsed().as_secs_f64() * 1000.0);
                if let Some(r) = done {
                    assert_eq!(r, Ok(true));
                    break;
                }
            }
            let total: f64 = slices.iter().sum();
            let longest = slices.iter().cloned().fold(0.0, f64::max);
            let at = slices.iter().position(|v| *v == longest).unwrap();
            eprintln!("the terrain farm over {} slices ({total:.0} ms), the longest {longest:.1} ms (slice {at})", slices.len());
        }
        // the settle tick by tick
        {
            let d = crate::design::Design::parse(&text).unwrap();
            let spec = crate::mobfarm::spec(&d).unwrap();
            let t = farm_for(&text, &d, &spec).unwrap().unwrap();
            let mut m = crate::mobfarm::MobRun::start(&d, crate::mobfarm::JUDGE_SEED, Some(&t)).unwrap();
            let mut times = Vec::new();
            loop {
                let t0 = std::time::Instant::now();
                if !m.settle_tick() {
                    break;
                }
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            let total: f64 = times.iter().sum();
            let slowest = times.iter().cloned().fold(0.0, f64::max);
            eprintln!("settle: {} ticks, {total:.0} ms, first {:.1} ms, slowest {slowest:.1} ms, last {:.1} ms; ticks 1..10 {:?}", times.len(), times[0], times[times.len() - 1], times.iter().take(10).map(|v| (*v * 10.0).round() / 10.0).collect::<Vec<_>>());
        }
        let t0 = std::time::Instant::now();
        let mut live = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let t_live = t0.elapsed().as_secs_f64() * 1000.0;
        let t0 = std::time::Instant::now();
        for _ in 0..100 {
            live.step();
        }
        let t_steps = t0.elapsed().as_secs_f64() * 1000.0 / 100.0;
        eprintln!("D1U in village-edge: still {t_still:.0} ms (again {t_still2:.0} ms), {blocks} blocks drawn; live session {t_live:.0} ms (its settle); a step {t_steps:.2} ms; unsimulated {:?}", live.unsimulated);
        let t0 = std::time::Instant::now();
        for _ in 0..24000 {
            live.step();
        }
        let m = live.mob.as_ref().unwrap();
        eprintln!("a game day more in {:.1} s: {} golems, {} iron in the chest; tick {}, halted {:?}", t0.elapsed().as_secs_f64(), m.golems, m.iron_in_chests(&live.world), live.tick, live.halted);
    }

    /// When a landscape cell changes in the run (`MCVIEW_CELL=x,y,z` in design coordinates;
    /// `--nocapture`).
    #[test]
    #[ignore]
    fn print_cell_history() {
        let text = std::fs::read_to_string("/tmp/mv/d1u-fixed/d1u-underground.txt").unwrap();
        let c: Vec<i32> = std::env::var("MCVIEW_CELL").unwrap_or_else(|_| "-21,-14,9".into()).split(',').map(|v| v.trim().parse().unwrap()).collect();
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        let off = s.view_offset();
        let p = redsim::BlockPos::new(c[0] + off[0], c[1] + off[1], c[2] + off[2]);
        let t = s.terrain.clone().unwrap();
        let land = t.landscape.iter().find(|(q, _)| *q == p.arr()).map(|(_, b)| b.clone());
        eprintln!("cell {c:?} = world {p:?}: landscape {land:?}, the run's world at its start {} (time {})", s.world.get(p), s.world.time);
        let mut last = s.world.get(p).to_string();
        for _ in 0..400 {
            s.step();
            let now = s.world.get(p).to_string();
            if now != last {
                eprintln!("time {}: {last} -> {now}; in the writes kept outside the farm: {:?}", s.world.time, s.outside.get(&p.arr()).map(|b| b.to_string()));
                last = now;
            }
        }
    }

    /// Which villagers' golem searches reach the surface (`--nocapture`).
    #[test]
    #[ignore]
    fn print_spawn_spots_by_villager() {
        let text = std::fs::read_to_string("/tmp/mv/d1u-fixed/d1u-underground.txt").unwrap_or_else(|_| String::from_utf8(read("designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt")).unwrap());
        let mut s = crate::session::Session::new(&text, &crate::session::Options::live()).unwrap();
        for _ in 0..50 {
            s.step();
        }
        let w = &s.world;
        let d = Design::parse(&text).unwrap();
        let spec = s.terrain.as_ref().unwrap().site.spec(&crate::mobfarm::spec(&d).unwrap());
        let starts = redsim::entity::mob::site::panicking(w, &spec.blocks);
        let ground = s.terrain.as_ref().unwrap().offset()[1] + d.meta.get("ground").and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(0);
        for (i, e) in w.entities.list.iter().enumerate() {
            if !matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager) {
                continue;
            }
            let cells = redsim::entity::mob::spawn::golem_spawn_cells(w, i);
            let open: Vec<_> = cells.iter().filter(|(_, r)| crate::mobfarm::spot_open(r)).map(|(c, _)| *c).collect();
            let high = open.iter().filter(|c| c.y >= ground).count();
            eprintln!("villager {i} at {:?} (block {:?}): {} open spots, {high} at or above the ground layer y {ground}; panics: {}", e.pos, e.block_position(), open.len(), starts.contains(&e.block_position()));
        }
    }

    /// The live view's frame of a farm in its terrain, as the browser draws it (`--nocapture`):
    /// the blocks kept on the GPU apart (`Player::triangles_split_within`) against the whole
    /// frame (`Player::triangles`), still and woken, as PNGs in /tmp.
    #[test]
    #[ignore]
    fn print_terrain_split_frames() {
        let text = String::from_utf8(read("designs/circuits/mobfarm/tracks/ironfarm/d1u-underground.txt")).unwrap();
        let mut atlas = crate::atlas::Atlas::build();
        if let Ok((png, idx, defs, models)) = crate::tools::vanilla_assets() {
            let _ = atlas.apply_vanilla_png(&png, &idx);
            let _ = crate::tools::decode_png(&png).and_then(|(rgba, w, h)| atlas.set_game(&rgba, w, h, &idx, &defs, &models));
        }
        let (w, h) = (1232u32, 480u32);
        let mut p = crate::player::Player::new_lazy_with(&text, w as f32 / h as f32, None, None).unwrap();
        let (cam, _) = crate::scene::live_camera(&atlas, &p.session, &p.look, w as f32, h as f32);
        p.camera = cam;
        let mut off = crate::render::headless::Offscreen::new(&atlas).unwrap();
        for stage in ["still", "woken"] {
            if stage == "woken" {
                assert!(p.wake());
                p.playing = true;
                for _ in 0..40 {
                    p.update(0.05);
                }
            }
            let (fixed, solid, translucent) = loop {
                if let Some(t) = p.triangles_split_within(&atlas, &mut || false) {
                    break t;
                }
            };
            off.r.set_fixed(fixed.as_ref().map(|(k, v)| (*k, v.as_slice())));
            let a = off.render(&crate::render::Frame { solid: &solid, translucent: &translucent, camera: p.camera, width: w, height: h }).unwrap();
            crate::render::headless::write_png(std::path::Path::new(&format!("/tmp/mv/t/split-{stage}.png")), w, h, &a).unwrap();
            off.r.set_fixed(None);
            let (s2, t2) = p.triangles(&atlas);
            let b = off.render(&crate::render::Frame { solid: &s2, translucent: &t2, camera: p.camera, width: w, height: h }).unwrap();
            crate::render::headless::write_png(std::path::Path::new(&format!("/tmp/mv/t/whole-{stage}.png")), w, h, &b).unwrap();
            let differ = a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count();
            eprintln!("{stage}: fixed {} + {} solid, {} translucent; whole {} solid, {} translucent; {differ} pixels differ; mob {}", fixed.as_ref().map(|f| f.1.len()).unwrap_or(0), solid.len(), translucent.len(), s2.len(), t2.len(), p.session.mob.is_some());
        }
    }

    /// What a sample costs to load and how much of it the view draws (`--nocapture`).
    #[test]
    #[ignore]
    fn print_landscape_costs() {
        let bytes = read("designs/terrain/village-edge.mcterrain");
        let t0 = std::time::Instant::now();
        let t = redsim::terrain::format::Terrain::decode(&bytes).unwrap();
        let t_decode = t0.elapsed().as_secs_f64() * 1000.0;
        let t0 = std::time::Instant::now();
        let w = redsim::terrain::land::build_world(&t).unwrap();
        let t_world = t0.elapsed().as_secs_f64() * 1000.0;
        let t0 = std::time::Instant::now();
        let none = (BlockPos::new(1, 1, 1), BlockPos::new(0, 0, 0));
        let keep = |_: BlockPos| false;
        let (shown, hidden) = landscape(&w, none, w.min.y, w.min.z, &keep);
        let t_land = t0.elapsed().as_secs_f64() * 1000.0;
        let t0 = std::time::Instant::now();
        let (shown49, hidden49) = landscape(&w, none, 49, w.min.z, &keep);
        let t_land49 = t0.elapsed().as_secs_f64() * 1000.0;
        eprintln!("from y 49: {} shown, {} hidden, {t_land49:.1} ms", shown49.len(), hidden49.len());
        let t0 = std::time::Instant::now();
        let (shown_cut, hidden_cut) = landscape(&w, none, 49, 248, &|p: BlockPos| p.z < 248);
        eprintln!("from y 49 and z 248: {} shown, {} hidden, {:.1} ms", shown_cut.len(), hidden_cut.len(), t0.elapsed().as_secs_f64() * 1000.0);
        let mut non_air = 0;
        for y in 0..w.size[1] {
            for z in 0..w.size[2] {
                for x in 0..w.size[0] {
                    if w.get(w.min.offset(x, y, z)) != Block::Air {
                        non_air += 1;
                    }
                }
            }
        }
        eprintln!(
            "village-edge {} KB: {:?} at {:?}, {non_air} non-air; decode {t_decode:.1} ms, world {t_world:.1} ms, landscape {t_land:.1} ms: {} shown, {} hidden next to them",
            bytes.len() / 1024,
            w.size,
            w.min,
            shown.len(),
            hidden.len()
        );
    }
}
