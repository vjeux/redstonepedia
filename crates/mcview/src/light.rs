//! The game's light around a build, for the brightness of its block faces: the sky's light (15
//! straight down every open column, then one less per block around what blocks it: dark under
//! overhangs) and the light sources' block light, propagated like the game's light engine
//! (`LightEngine`: opaque blocks stop it, a non-opaque block with a full shape, a fluid or leaves
//! take one more level, a slab's or a stair's full faces stop it in their direction); and the
//! game's lightmap (`lightmap.fsh`: a light level's brightness at noon in the overworld, the
//! default brightness setting).

use crate::design::Pos;
use std::collections::HashSet;
use std::sync::OnceLock;

/// The game's direction order (`Direction.ordinal()`): down, up, north, south, west, east.
pub const DIRS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
const DOWN: usize = 0;
const UP: usize = 1;

fn opposite(d: usize) -> usize {
    d ^ 1
}

/// What a cell's block does to light.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    /// An opaque full block (solid render): no light in it, none through it.
    pub opaque: bool,
    /// Levels it takes on top of the one per block (`getLightDampening`: 1 for a non-opaque
    /// block with a full shape, a fluid, leaves).
    pub dampening: u8,
    /// The faces its shape covers whole, which light does not cross (bit `d` for `DIRS[d]`): a
    /// slab's bottom, a stair's bottom and back (`useShapeForLightOcclusion`).
    pub occludes: u8,
    /// The light it gives off (`lightEmission`).
    pub emission: u8,
}

/// A block state's light properties; `opaque`: an opaque full cube (its model); `full_shape`:
/// its model fills the cell (glass, ice, a slime block).
pub fn cell_of(state: &str, opaque: bool, full_shape: bool) -> Cell {
    let name = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    let has = |kv: &str| state.contains(kv);
    let emission = emission(state);
    if opaque {
        return Cell { opaque: true, dampening: 15, occludes: 0, emission };
    }
    // the game's `propagatesSkylightDown` exceptions: glass (TransparentBlock), shulker boxes,
    // barriers and light blocks let the sky through though they fill their cell
    let lets_sky_through = name.ends_with("glass") || name.ends_with("shulker_box") || name == "barrier" || name == "light";
    let fluid = name == "water" || name == "bubble_column" || name == "lava" || has("waterlogged=true");
    let dampening = if name.ends_with("leaves") || fluid || (full_shape && !lets_sky_through) { 1 } else { 0 };
    let bottom_or_top = |top: bool| if top { 1u8 << UP } else { 1u8 << DOWN };
    let occludes = if name.ends_with("_slab") {
        if has("type=double") { 0b11_1111 } else { bottom_or_top(has("type=top")) }
    } else if name.ends_with("_stairs") {
        // the half's whole face and the back (the step's full-height side)
        let back = match () {
            _ if has("facing=north") => 1u8 << 2,
            _ if has("facing=south") => 1u8 << 3,
            _ if has("facing=west") => 1u8 << 4,
            _ => 1u8 << 5,
        };
        bottom_or_top(has("half=top")) | back
    } else if matches!(name, "farmland" | "dirt_path" | "daylight_detector" | "sculk_sensor" | "calibrated_sculk_sensor" | "sculk_shrieker" | "stonecutter" | "enchanting_table" | "end_portal_frame" | "lectern")
        || (name == "snow" && !has("layers=8"))
    {
        1u8 << DOWN
    } else {
        0
    };
    Cell { opaque: false, dampening, occludes, emission }
}

/// The light a block state gives off (the game's `lightEmission` of the blocks a redstone build
/// uses; 0 for the others).
pub fn emission(state: &str) -> u8 {
    let name = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    let lit = state.contains("lit=true");
    match name {
        "redstone_lamp" | "furnace" | "blast_furnace" | "smoker" | "redstone_ore" | "deepslate_redstone_ore" => {
            if !lit {
                0
            } else if name == "redstone_lamp" {
                15
            } else if name.ends_with("redstone_ore") {
                9
            } else {
                13
            }
        }
        "redstone_torch" | "redstone_wall_torch" => {
            if lit {
                7
            } else {
                0
            }
        }
        n if n.ends_with("copper_bulb") => {
            if !lit {
                0
            } else if n.contains("weathered") {
                8
            } else if n.contains("exposed") {
                12
            } else if n.contains("oxidized") {
                4
            } else {
                15
            }
        }
        "glowstone" | "sea_lantern" | "shroomlight" | "jack_o_lantern" | "lantern" | "beacon" | "conduit" | "end_gateway" | "lava" | "fire" | "pearlescent_froglight" | "verdant_froglight" | "ochre_froglight" => 15,
        "torch" | "wall_torch" | "end_rod" => 14,
        "soul_torch" | "soul_wall_torch" | "soul_lantern" | "soul_fire" | "crying_obsidian" => 10,
        "magma_block" => 3,
        "sculk_sensor" | "calibrated_sculk_sensor" | "brewing_stand" | "brown_mushroom" | "dragon_egg" | "end_portal_frame" => 1,
        _ => 0,
    }
}

/// A cell's block darkens the corners around it like a full block (`getShadeBrightness` 0.2).
pub const SHADE_FULL: u8 = 1;
/// A cell's block has a full collision box (`isCollisionShapeFullBlock`): the game lights every
/// flat face of it from the cell in front, even one inside the block (a slime block's core).
pub const COLLISION_FULL: u8 = 2;

/// The light levels over a box around a build: sky and block light per cell.
pub struct Light {
    lo: Pos,
    dims: [i32; 3],
    sky: Vec<u8>,
    block: Vec<u8>,
    /// Each cell's block as far as light goes, and its [`SHADE_FULL`] / [`COLLISION_FULL`] flags.
    cells: Vec<Cell>,
    flags: Vec<u8>,
    /// Cells below it are the ground (no light)...
    floor: i32,
    /// ...but these: holes in the ground (a TNT crater's blown cells, open to the air and lit
    /// like it; their blocks, when something fell in, are among the cells).
    holes: HashSet<Pos>,
}

impl Light {
    /// The light around `cells` (the still blocks: their light properties and flags), over their
    /// box and a block around it, from above it down to `floor` (cells below are the ground: a
    /// build stands on the judge's grass, one block of air under its lowest cell), but `holes` in
    /// it (a crater: the game lights it like the open air; it was all dark, the bedrock at a
    /// crater's bottom black: vjeux 2026-10-21). Kept as dense boxes: a mesh looks a dozen cells
    /// up per face.
    pub fn compute(cells: &[(Pos, Cell, u8)], floor: i32, holes: &HashSet<Pos>) -> Light {
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for (c, _, _) in cells {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        if cells.is_empty() {
            return Light { lo: [0; 3], dims: [0; 3], sky: Vec::new(), block: Vec::new(), cells: Vec::new(), flags: Vec::new(), floor, holes: HashSet::new() };
        }
        let deepest = holes.iter().map(|h| h[1]).min().unwrap_or(floor);
        let lo = [lo[0] - 1, floor.min(lo[1]).min(deepest), lo[2] - 1];
        let hi = [hi[0] + 1, hi[1] + 1, hi[2] + 1];
        let dims = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let n = (dims[0] * dims[1] * dims[2]) as usize;
        let mut l = Light { lo, dims, sky: vec![0; n], block: vec![0; n], cells: vec![Cell::default(); n], flags: vec![0; n], floor, holes: holes.clone() };
        for (c, cell, flags) in cells {
            if let Some(i) = l.index(*c) {
                l.cells[i] = *cell;
                l.flags[i] = *flags;
            }
        }
        // the sky: 15 down every column until something stops or dims it, then spread
        let mut seeds = Vec::new();
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let mut above: Option<Cell> = None;
                for y in (lo[1]..=hi[1]).rev() {
                    let c = [x, y, z];
                    let here = l.cell(c);
                    if here.opaque || here.dampening > 0 || above.is_some_and(|a| a.occludes & (1 << DOWN) != 0) || here.occludes & (1 << UP) != 0 || l.ground(c) {
                        break;
                    }
                    let i = l.index(c).unwrap();
                    l.sky[i] = 15;
                    seeds.push((c, 15u8));
                    above = Some(here);
                }
            }
        }
        l.spread(seeds, true);
        // the light sources
        let sources: Vec<(Pos, u8)> = cells.iter().filter(|(_, c, _)| c.emission > 0).map(|(p, c, _)| (*p, c.emission)).collect();
        for (p, e) in &sources {
            if let Some(i) = l.index(*p) {
                l.block[i] = l.block[i].max(*e);
            }
        }
        l.spread(sources, false);
        l
    }

    /// A cell's block, as far as light goes (air outside the box).
    pub fn cell(&self, c: Pos) -> Cell {
        self.index(c).map(|i| self.cells[i]).unwrap_or_default()
    }

    /// An opaque full block there (one of the still blocks).
    pub fn opaque(&self, c: Pos) -> bool {
        self.index(c).is_some_and(|i| self.cells[i].opaque)
    }

    /// A block there that darkens the corners around it like a full block.
    pub fn shade_full(&self, c: Pos) -> bool {
        self.index(c).is_some_and(|i| self.flags[i] & SHADE_FULL != 0)
    }

    /// A block there with a full collision box ([`COLLISION_FULL`]).
    pub fn collision_full(&self, c: Pos) -> bool {
        self.index(c).is_some_and(|i| self.flags[i] & COLLISION_FULL != 0)
    }

    /// (opaque, darkens corners, [`Light::at`]) at once.
    pub fn probe(&self, c: Pos) -> (bool, bool, (u8, u8)) {
        if self.ground(c) {
            return (false, false, (0, 0));
        }
        match self.index(c) {
            Some(i) => {
                let cell = self.cells[i];
                let light = if cell.opaque { (0, cell.emission) } else { (self.sky[i], self.block[i].max(cell.emission)) };
                (cell.opaque, self.flags[i] & SHADE_FULL != 0, light)
            }
            None => (false, false, (15, 0)),
        }
    }

    fn index(&self, c: Pos) -> Option<usize> {
        let d = [c[0] - self.lo[0], c[1] - self.lo[1], c[2] - self.lo[2]];
        if (0..3).any(|k| d[k] < 0 || d[k] >= self.dims[k]) {
            return None;
        }
        Some(((d[1] * self.dims[2] + d[2]) * self.dims[0] + d[0]) as usize)
    }

    /// The ground there: below the floor and no hole.
    fn ground(&self, c: Pos) -> bool {
        c[1] < self.floor && !self.holes.contains(&c)
    }

    /// Spread from the seeds, brightest first (a bucket per level): each step one level less (and
    /// the block's dampening), the sky's 15 straight down through what does not dim it.
    fn spread(&mut self, seeds: Vec<(Pos, u8)>, sky: bool) {
        let mut buckets: Vec<Vec<Pos>> = vec![Vec::new(); 16];
        for (p, lv) in seeds {
            buckets[lv as usize].push(p);
        }
        for lv in (1..16usize).rev() {
            let mut k = 0;
            while k < buckets[lv].len() {
                let a = buckets[lv][k];
                k += 1;
                let Some(ia) = self.index(a) else { continue };
                let cur = if sky { self.sky[ia] } else { self.block[ia] };
                if cur as usize != lv {
                    continue;
                }
                let from = self.cells[ia];
                for (d, off) in DIRS.iter().enumerate() {
                    let b = [a[0] + off[0], a[1] + off[1], a[2] + off[2]];
                    if self.ground(b) {
                        continue;
                    }
                    let Some(ib) = self.index(b) else { continue };
                    let to = self.cells[ib];
                    // an opaque light source sends its light out; nothing enters an opaque block
                    let source = from.opaque;
                    if to.opaque || (!source && from.occludes & (1 << d) != 0) || to.occludes & (1 << opposite(d)) != 0 {
                        continue;
                    }
                    let nl = if sky && d == DOWN && lv == 15 && to.dampening == 0 { 15 } else { (lv as u8).saturating_sub(to.dampening.max(1)) };
                    let slot = if sky { &mut self.sky[ib] } else { &mut self.block[ib] };
                    if nl > *slot {
                        *slot = nl;
                        buckets[nl as usize].push(b);
                    }
                }
            }
        }
    }

    /// The (sky, block) light the game reads at a cell (`getLightCoords`): none in the ground or
    /// in an opaque block (but a light source's own), the open sky outside the box.
    pub fn at(&self, c: Pos) -> (u8, u8) {
        if self.ground(c) {
            return (0, 0);
        }
        match self.index(c) {
            Some(i) => {
                let cell = self.cells[i];
                if cell.opaque { (0, cell.emission) } else { (self.sky[i], self.block[i].max(cell.emission)) }
            }
            None => (15, 0),
        }
    }
}

/// One texel of the game's lightmap (`lightmap.fsh` at noon in the overworld: ambient light
/// 0x0A0A0A, white sky light at full strength, block light 1.4x tinted 0xFFD88C, then halfway to
/// `notGamma` for the default brightness setting, 0.5).
fn texel(sky: u8, block: u8) -> [f32; 3] {
    let gb = |l: f32| l / (4.0 - 3.0 * l);
    let (bl, sl) = (block as f32 / 15.0, sky as f32 / 15.0);
    let (block_b, sky_b) = (gb(bl) * 1.4, gb(sl));
    let ambient = 10.0 / 255.0;
    let tint = [1.0, 216.0 / 255.0, 140.0 / 255.0];
    let pm = (2.0 * bl - 1.0) * (2.0 * bl - 1.0);
    let mut c = [0f32; 3];
    for k in 0..3 {
        let blc = tint[k] + (1.0 - tint[k]) * 0.9 * pm;
        c[k] = (ambient + sky_b + blc * block_b).clamp(0.0, 1.0);
    }
    let max = c[0].max(c[1]).max(c[2]);
    if max <= 0.0 {
        return c;
    }
    let inv = 1.0 - max;
    let scaled = 1.0 - inv * inv * inv * inv;
    c.map(|v| v + (v * scaled / max - v) * 0.5)
}

/// The lightmap at (sky, block) light levels, fractional ones (a smooth-lit corner's mean)
/// between texels like the game's linear sampler.
pub fn lightmap(sky: f32, block: f32) -> [f32; 3] {
    // the 16 x 16 texels, worked out once
    static TEXELS: OnceLock<Vec<[f32; 3]>> = OnceLock::new();
    let t = TEXELS.get_or_init(|| (0..256).map(|i| texel((i / 16) as u8, (i % 16) as u8)).collect());
    let texel = |s: u8, b: u8| t[s as usize * 16 + b as usize];
    let (s, b) = (sky.clamp(0.0, 15.0), block.clamp(0.0, 15.0));
    let (s0, b0) = (s.floor() as u8, b.floor() as u8);
    let (s1, b1) = ((s0 + 1).min(15), (b0 + 1).min(15));
    let (fs, fb) = (s - s0 as f32, b - b0 as f32);
    let (t00, t01, t10, t11) = (texel(s0, b0), texel(s0, b1), texel(s1, b0), texel(s1, b1));
    std::array::from_fn(|k| {
        let lo = t00[k] + (t01[k] - t00[k]) * fb;
        let hi = t10[k] + (t11[k] - t10[k]) * fb;
        lo + (hi - lo) * fs
    })
}

/// The game's smooth blend of four (sky, block) lights in 16ths of a level (`smoothBlend`): a
/// neighbour with no light (an opaque block) takes the centre's, then the mean, truncated.
pub fn smooth_blend(n1: (u8, u8), n2: (u8, u8), n3: (u8, u8), center: (u8, u8)) -> (f32, f32) {
    let fix = |n: (u8, u8)| -> (u32, u32) {
        let c = (center.0 as u32 * 16, center.1 as u32 * 16);
        let v = (n.0 as u32 * 16, n.1 as u32 * 16);
        if center.0 > 2 || center.1 > 2 {
            if v == (0, 0) {
                return c;
            }
            if v.0 == 0 {
                return (c.0, v.1);
            }
        }
        v
    };
    let (a, b, c) = (fix(n1), fix(n2), fix(n3));
    let sky = (a.0 + b.0 + c.0 + center.0 as u32 * 16) >> 2;
    let block = (a.1 + b.1 + c.1 + center.1 as u32 * 16) >> 2;
    (sky as f32 / 16.0, block as f32 / 16.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_a_block_the_sky_is_one_less() {
        let l = Light::compute(&[([0, 2, 0], Cell { opaque: true, dampening: 15, ..Default::default() }, SHADE_FULL | COLLISION_FULL)], 1, &HashSet::new());
        assert_eq!(l.at([0, 3, 0]), (15, 0));
        assert_eq!(l.at([0, 1, 0]), (14, 0), "the cell under the block: its neighbours' 15 less one");
        assert_eq!(l.at([1, 1, 0]), (15, 0));
        assert_eq!(l.at([0, 0, 0]), (0, 0), "the ground");
        assert_eq!(l.at([0, 2, 0]), (0, 0), "no light inside an opaque block");
        assert!(l.opaque([0, 2, 0]) && l.shade_full([0, 2, 0]) && !l.opaque([0, 1, 0]));
    }

    /// A TNT crater in the judge's ground (vjeux 2026-10-21: "the bedrock texture no longer works
    /// correctly": the crater's walls and its bedrock bottom were black, every cell below the
    /// ground's top counted as ground): its blown cells are open air, the sky straight down into
    /// it, one less around under an overhang; the intact ground stays dark.
    #[test]
    fn a_crater_in_the_ground_is_lit() {
        let solid = Cell { opaque: true, dampening: 15, ..Default::default() };
        let f = SHADE_FULL | COLLISION_FULL;
        // the ground's top layer at y = -1 (the floor: the air from y = 0), a crater two deep at
        // x = 0..=1, z = 0 under an overhang at (1, 0, 0), bedrock at its bottom (y = -3)
        let holes: HashSet<Pos> = [[0, -1, 0], [0, -2, 0], [1, -1, 0], [1, -2, 0]].into_iter().collect();
        let mut cells = vec![([1, 0, 0], solid, f)];
        for x in -1..=2 {
            for z in -1..=1 {
                for y in -3..=-1 {
                    if !holes.contains(&[x, y, z]) {
                        cells.push(([x, y, z], solid, f));
                    }
                }
            }
        }
        let l = Light::compute(&cells, 0, &holes);
        assert_eq!(l.at([0, -2, 0]), (15, 0), "open to the sky straight down");
        assert_eq!(l.at([1, -2, 0]), (14, 0), "under the overhang: from its neighbour");
        assert_eq!(l.at([2, -2, 1]), (0, 0), "the intact ground");
        // what the viewer did before: the crater dark
        assert_eq!(Light::compute(&cells, 0, &HashSet::new()).at([0, -2, 0]), (0, 0));
    }

    #[test]
    fn a_lit_lamp_lights_around_it() {
        let l = Light::compute(&[([0, 5, 0], cell_of("minecraft:redstone_lamp[lit=true]", true, true), SHADE_FULL | COLLISION_FULL)], 0, &HashSet::new());
        assert_eq!(l.at([0, 5, 0]), (0, 15));
        assert_eq!(l.at([1, 5, 0]).1, 14);
    }

    #[test]
    fn a_bottom_slab_stops_the_sky_below_it() {
        let l = Light::compute(&[([0, 3, 0], cell_of("minecraft:stone_slab[type=bottom,waterlogged=false]", false, false), 0)], 0, &HashSet::new());
        assert_eq!(l.at([0, 3, 0]).0, 15, "light enters the slab's open top");
        assert_eq!(l.at([0, 2, 0]).0, 14, "below it only from the sides");
    }

    #[test]
    fn the_lightmap_is_the_games() {
        // full light is white; a level less is the game's 0.908 (the default brightness)
        assert!(lightmap(15.0, 0.0).iter().all(|v| (v - 1.0).abs() < 1e-4));
        let l14 = lightmap(14.0, 0.0)[0];
        assert!((l14 - 0.908).abs() < 0.002, "{l14}");
    }
}
