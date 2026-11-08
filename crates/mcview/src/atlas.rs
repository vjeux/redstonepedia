//! Minecraft-like block textures, drawn procedurally in 16x16 pixel tiles (no game asset is
//! shipped: the site is public). Every tile has a name; [`Atlas::build`] paints them all into one
//! RGBA atlas with per-tile mip levels (each tile filtered on its own, so nothing bleeds).

use std::collections::HashMap;

pub const TILE: usize = 16;
/// WebGL2's guaranteed texture-array depth.
pub const MAX_LAYERS: usize = 256;
/// Mip levels: 16, 8, 4, 2, 1 pixels per tile.
pub const MIPS: usize = 5;

pub type Rgba = [u8; 4];

pub const fn rgb(c: u32) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}
pub const fn rgba(c: u32, a: u8) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, a]
}
pub const CLEAR: Rgba = [0, 0, 0, 0];

/// A hash noise in 0..1 for (x, y, seed).
pub fn noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0xffffff) as f32 / 16777216.0
}

fn scale(c: Rgba, f: f32) -> Rgba {
    let s = |v: u8| (v as f32 * f).round().clamp(0.0, 255.0) as u8;
    [s(c[0]), s(c[1]), s(c[2]), c[3]]
}

fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

/// One 16x16 tile being painted.
#[derive(Clone)]
pub struct Tile {
    pub px: [Rgba; TILE * TILE],
    seed: u32,
}

impl Tile {
    fn new(seed: u32) -> Tile {
        Tile { px: [CLEAR; TILE * TILE], seed }
    }
    fn n(&self, x: i32, y: i32, k: u32) -> f32 {
        noise(x, y, self.seed.wrapping_add(k.wrapping_mul(7919)))
    }
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        self.px[(y.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize]
    }
    fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if (0..16).contains(&x) && (0..16).contains(&y) {
            self.px[(y * 16 + x) as usize] = c;
        }
    }
    fn fill(&mut self, c: Rgba) -> &mut Self {
        self.px = [c; 256];
        self
    }
    /// Per-pixel brightness jitter of +-amount around each pixel's color.
    fn vary(&mut self, amount: f32) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                let f = 1.0 + (self.n(x, y, 1) * 2.0 - 1.0) * amount;
                let c = self.get(x, y);
                self.set(x, y, scale(c, f));
            }
        }
        self
    }
    /// Pixels of color `c` with probability `p`.
    fn speckle(&mut self, c: Rgba, p: f32, k: u32) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                if self.n(x, y, 100 + k) < p {
                    self.set(x, y, c);
                }
            }
        }
        self
    }
    /// Blobs of 2x1 / 1x2 pixels of color `c` (stone-like grain).
    fn grain(&mut self, c: Rgba, p: f32, k: u32) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                if self.n(x, y, 200 + k) < p {
                    self.set(x, y, c);
                    if self.n(x, y, 300 + k) < 0.5 {
                        self.set(x + 1, y, c);
                    } else {
                        self.set(x, y + 1, c);
                    }
                }
            }
        }
        self
    }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) -> &mut Self {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, c);
            }
        }
        self
    }
    fn outline(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) -> &mut Self {
        for x in x0..=x1 {
            self.set(x, y0, c);
            self.set(x, y1, c);
        }
        for y in y0..=y1 {
            self.set(x0, y, c);
            self.set(x1, y, c);
        }
        self
    }
    /// Light top/left edge, dark bottom/right edge (a beveled block).
    fn bevel(&mut self, light: f32, dark: f32) -> &mut Self {
        for i in 0..16 {
            let c = self.get(i, 0);
            self.set(i, 0, scale(c, light));
            let c = self.get(0, i);
            self.set(0, i, scale(c, light));
            let c = self.get(i, 15);
            self.set(i, 15, scale(c, dark));
            let c = self.get(15, i);
            self.set(15, i, scale(c, dark));
        }
        self
    }
    fn map(&mut self, f: impl Fn(i32, i32, Rgba) -> Rgba) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                let c = self.get(x, y);
                self.set(x, y, f(x, y, c));
            }
        }
        self
    }
    /// Irregular stones (Voronoi cells) with dark seams: cobblestone-like.
    fn stones(&mut self, light: Rgba, dark: Rgba, seam: Rgba, cells: usize, k: u32) -> &mut Self {
        let pts: Vec<(f32, f32, f32)> = (0..cells).map(|i| (self.n(i as i32, 7, k) * 16.0, self.n(i as i32, 9, k) * 16.0, self.n(i as i32, 11, k))).collect();
        for y in 0..16 {
            for x in 0..16 {
                let mut best = (f32::MAX, f32::MAX, 0usize);
                for (i, (px, py, _)) in pts.iter().enumerate() {
                    // wrap so the tile repeats
                    let mut d = f32::MAX;
                    for (ox, oy) in [(0.0, 0.0), (16.0, 0.0), (-16.0, 0.0), (0.0, 16.0), (0.0, -16.0)] {
                        let dx = x as f32 + 0.5 - (px + ox);
                        let dy = y as f32 + 0.5 - (py + oy);
                        d = d.min(dx * dx + dy * dy);
                    }
                    if d < best.0 {
                        best = (d, best.0, i);
                    } else if d < best.1 {
                        best.1 = d;
                    }
                }
                let edge = best.1.sqrt() - best.0.sqrt();
                let c = if edge < 0.9 { seam } else { mix(dark, light, pts[best.2].2) };
                self.set(x, y, c);
            }
        }
        self
    }
    /// Brick rows: `h` px tall bricks `w` px wide, alternate rows offset, `mortar` lines.
    fn bricks(&mut self, brick: Rgba, mortar: Rgba, h: i32, w: i32) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                let row = y / h;
                let off = if row % 2 == 1 { w / 2 } else { 0 };
                let c = if y % h == h - 1 || (x + off) % w == w - 1 { mortar } else { scale(brick, 0.9 + 0.2 * self.n((x + off) / w, row, 5)) };
                self.set(x, y, c);
            }
        }
        self
    }
    /// Horizontal planks 4 px tall with grain.
    fn planks(&mut self, base: Rgba) -> &mut Self {
        for y in 0..16 {
            for x in 0..16 {
                let board = y / 4;
                let seam = y % 4 == 3;
                let split = (x + board * 5) % 16 == 0 && board % 2 == 0 || (x + board * 7) % 16 == 8 && board % 2 == 1;
                let grain = 0.93 + 0.12 * self.n(x / 3, y, 3) + if self.n(x, y, 4) < 0.08 { -0.12 } else { 0.0 };
                let c = if seam { scale(base, 0.62) } else if split { scale(base, 0.72) } else { scale(base, grain) };
                self.set(x, y, c);
            }
        }
        self
    }
    /// Paint 16 rows of 16 characters, each mapped to a color by `pal`; other characters (`.`)
    /// leave the pixel as it is.
    fn grid(&mut self, rows: &[&str; 16], pal: &[(char, Rgba)]) -> &mut Self {
        for (y, r) in rows.iter().enumerate() {
            for (x, ch) in r.chars().enumerate() {
                if let Some((_, c)) = pal.iter().find(|(k, _)| *k == ch) {
                    self.set(x as i32, y as i32, *c);
                }
            }
        }
        self
    }
    /// Mottled pixels in the rectangle (x0, y0)..=(x1, y1) with exactly these colors in these
    /// shares, darkest where a tileable smooth noise (cells `sx` x `sy` px; 16 must divide by them)
    /// mixed with per-pixel noise (`clump` 0..1 = how blotchy) is lowest.
    #[allow(clippy::too_many_arguments)]
    fn mottle_in(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, cols: &[(u32, f32)], sx: i32, sy: i32, clump: f32, k: u32) -> &mut Self {
        let (cx, cy) = (16 / sx.max(1), 16 / sy.max(1));
        let lat = |t: &Tile, i: i32, j: i32| t.n(i.rem_euclid(cx), j.rem_euclid(cy), 500 + k);
        let mut vals: Vec<(f32, i32, i32)> = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (fx, fy) = (x as f32 / sx as f32, y as f32 / sy as f32);
                let (i0, j0) = (fx.floor() as i32, fy.floor() as i32);
                let (tx, ty) = (fx - i0 as f32, fy - j0 as f32);
                let top = lat(self, i0, j0) * (1.0 - tx) + lat(self, i0 + 1, j0) * tx;
                let bot = lat(self, i0, j0 + 1) * (1.0 - tx) + lat(self, i0 + 1, j0 + 1) * tx;
                let s = top * (1.0 - ty) + bot * ty;
                vals.push(((1.0 - clump) * self.n(x, y, 600 + k) + clump * s, x, y));
            }
        }
        vals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cols: Vec<(Rgba, f32)> = cols.iter().map(|(c, f)| (rgb(*c), *f)).collect();
        let lum = |c: &Rgba| c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11;
        cols.sort_by(|a, b| lum(&a.0).total_cmp(&lum(&b.0)));
        let total: f32 = cols.iter().map(|c| c.1).sum::<f32>().max(1e-6);
        let n = vals.len() as f32;
        let mut acc = 0.0;
        let mut ci = 0;
        for (i, (_, x, y)) in vals.iter().enumerate() {
            while ci + 1 < cols.len() && (i as f32 + 0.5) / n > (acc + cols[ci].1) / total {
                acc += cols[ci].1;
                ci += 1;
            }
            self.set(*x, *y, cols[ci].0);
        }
        self
    }
    /// [`Tile::mottle_in`] over the whole tile.
    fn mottle(&mut self, cols: &[(u32, f32)], sx: i32, sy: i32, clump: f32, k: u32) -> &mut Self {
        self.mottle_in(0, 0, 15, 15, cols, sx, sy, clump, k)
    }
}

/// The painted tiles, by name, in a fixed order.
fn paint_all() -> Vec<(&'static str, Tile)> {
    let mut out: Vec<(&'static str, Tile)> = Vec::new();
    let mut seed = 1u32;
    let mut add = |name: &'static str, f: &dyn Fn(&mut Tile)| {
        seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        let mut t = Tile::new(seed);
        f(&mut t);
        out.push((name, t));
    };
    // ---- stone family
    add("stone", &|t| {
        t.fill(rgb(0x7f7f7f)).vary(0.06).grain(rgb(0x6c6c6c), 0.1, 1).grain(rgb(0x929292), 0.07, 2);
    });
    add("smooth_stone", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a));
    });
    add("smooth_stone_slab_side", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a)).rect(1, 7, 14, 8, rgb(0x8f8f8f));
    });
    add("cobblestone", &|t| {
        t.stones(rgb(0x9a9a9a), rgb(0x6a6a6a), rgb(0x4a4a4a), 9, 1).vary(0.05);
    });
    add("mossy_cobblestone", &|t| {
        t.stones(rgb(0x9a9a9a), rgb(0x6a6a6a), rgb(0x4a4a4a), 9, 1).vary(0.05).speckle(rgb(0x5c7a3a), 0.22, 3).speckle(rgb(0x48632c), 0.08, 4);
    });
    add("obsidian", &|t| {
        t.fill(rgb(0x100c1c)).vary(0.2).grain(rgb(0x2b1d40), 0.12, 1).speckle(rgb(0x3c2a5a), 0.04, 2);
    });
    add("crying_obsidian", &|t| {
        t.fill(rgb(0x100c1c)).vary(0.2).grain(rgb(0x2b1d40), 0.12, 1).grain(rgb(0x8a2be0), 0.07, 2).speckle(rgb(0xc060ff), 0.03, 3);
    });
    add("granite", &|t| {
        t.fill(rgb(0x9a6a58)).vary(0.06).speckle(rgb(0xb98a78), 0.15, 1).speckle(rgb(0x7a4f40), 0.1, 2).speckle(rgb(0xc8a090), 0.04, 3);
    });
    add("polished_granite", &|t| {
        t.fill(rgb(0x9c6b59)).vary(0.04).speckle(rgb(0xae7d6a), 0.12, 1).outline(0, 0, 15, 15, rgb(0x82574a)).bevel(1.1, 0.85);
    });
    add("diorite", &|t| {
        t.fill(rgb(0xbdbdbd)).vary(0.05).speckle(rgb(0x8e8e8e), 0.12, 1).speckle(rgb(0xe6e6e6), 0.12, 2);
    });
    add("andesite", &|t| {
        t.fill(rgb(0x888888)).vary(0.05).speckle(rgb(0x707070), 0.14, 1).speckle(rgb(0xa2a2a2), 0.1, 2);
    });
    add("stone_bricks", &|t| {
        t.bricks(rgb(0x7c7c7c), rgb(0x5a5a5a), 8, 16).vary(0.05).grain(rgb(0x6c6c6c), 0.06, 2);
    });
    add("tuff", &|t| {
        t.fill(rgb(0x6c6d66)).vary(0.06).speckle(rgb(0x5a5b53), 0.15, 1).speckle(rgb(0x7f8078), 0.1, 2);
    });
    add("calcite", &|t| {
        t.fill(rgb(0xdfe0dc)).vary(0.03).speckle(rgb(0xc8c9c4), 0.1, 1);
    });
    add("bricks", &|t| {
        t.bricks(rgb(0x96614f), rgb(0xa89e92), 4, 8).vary(0.05);
    });
    add("sandstone_side", &|t| {
        t.fill(rgb(0xd8cb9b)).vary(0.03).rect(0, 0, 15, 2, rgb(0xe0d4a6)).rect(0, 12, 15, 15, rgb(0xcbbd8b)).speckle(rgb(0xc7b986), 0.06, 1);
    });
    add("sandstone_top", &|t| {
        t.fill(rgb(0xdccf9f)).vary(0.03).speckle(rgb(0xcfc290), 0.08, 1);
    });
    add("nether_bricks", &|t| {
        t.bricks(rgb(0x2e161a), rgb(0x160a0c), 4, 8).vary(0.08).speckle(rgb(0x45222a), 0.06, 2);
    });
    add("end_stone", &|t| {
        t.fill(rgb(0xdcdfa1)).vary(0.04).speckle(rgb(0xc6c98a), 0.12, 1).speckle(rgb(0xeaedb6), 0.06, 2);
    });
    add("blackstone", &|t| {
        t.fill(rgb(0x2a2328)).vary(0.1).speckle(rgb(0x3b3238), 0.15, 1).speckle(rgb(0x18131a), 0.1, 2);
    });
    add("quartz_block", &|t| {
        t.fill(rgb(0xebe5de)).vary(0.015).outline(0, 0, 15, 15, rgb(0xd6cfc5));
    });
    add("prismarine", &|t| {
        t.fill(rgb(0x63a398)).vary(0.08).speckle(rgb(0x4c8b7f), 0.15, 1).speckle(rgb(0x82bfb2), 0.1, 2);
    });
    add("purpur_block", &|t| {
        t.fill(rgb(0xa97ca9)).vary(0.04).outline(0, 0, 15, 15, rgb(0x8b5f8b)).outline(1, 1, 14, 14, rgb(0xb88db8));
    });
    add("terracotta", &|t| {
        t.fill(rgb(0x985e43)).vary(0.03).speckle(rgb(0x8c553c), 0.08, 1);
    });
    add("coal_block", &|t| {
        t.fill(rgb(0x141414)).vary(0.2).speckle(rgb(0x2a2a2a), 0.15, 1).bevel(1.6, 0.7);
    });
    add("raw_iron_block", &|t| {
        t.stones(rgb(0xc2a083), rgb(0x8f6f55), rgb(0x6c5340), 7, 3).vary(0.05);
    });
    add("mud_bricks", &|t| {
        t.bricks(rgb(0x8b6b50), rgb(0x6b513d), 4, 8).vary(0.05);
    });
    add("mud", &|t| {
        t.fill(rgb(0x3c393d)).vary(0.06).speckle(rgb(0x47434a), 0.15, 1);
    });
    add("white_concrete", &|t| {
        t.fill(rgb(0xcfd5d6)).vary(0.012);
    });
    add("black_concrete", &|t| {
        t.fill(rgb(0x0a0c11)).vary(0.1);
    });
    add("white_concrete_powder", &|t| {
        t.fill(rgb(0xe1e3e4)).vary(0.05).speckle(rgb(0xcfd2d3), 0.15, 1);
    });
    add("soul_sand", &|t| {
        t.fill(rgb(0x513e32)).vary(0.08).speckle(rgb(0x3d2e25), 0.15, 1).rect(4, 5, 5, 6, rgb(0x2e221b)).rect(9, 5, 10, 6, rgb(0x2e221b)).rect(6, 9, 8, 10, rgb(0x2e221b));
    });
    add("soul_soil", &|t| {
        t.fill(rgb(0x4b3a2f)).vary(0.08).speckle(rgb(0x3a2c24), 0.15, 1).speckle(rgb(0x5c4839), 0.1, 2);
    });
    add("clay", &|t| {
        t.fill(rgb(0xa1a7b2)).vary(0.04).speckle(rgb(0x949aa5), 0.1, 1);
    });
    add("packed_ice", &|t| {
        t.fill(rgb(0x8db4f8)).vary(0.04).speckle(rgb(0xa4c4fa), 0.08, 1);
    });
    add("sand", &|t| {
        t.fill(rgb(0xdbd0a3)).vary(0.05).speckle(rgb(0xcbbf90), 0.15, 1).speckle(rgb(0xe8dfb8), 0.08, 2);
    });
    add("red_sand", &|t| {
        t.fill(rgb(0xbe6621)).vary(0.05).speckle(rgb(0xa8571a), 0.15, 1).speckle(rgb(0xd07a33), 0.08, 2);
    });
    add("gravel", &|t| {
        t.stones(rgb(0x9a9393), rgb(0x6d6767), rgb(0x5a5555), 14, 5).vary(0.06);
    });
    add("dirt", &|t| {
        t.fill(rgb(0x866043)).vary(0.08).speckle(rgb(0x6c4c34), 0.15, 1).speckle(rgb(0x9a7354), 0.1, 2);
    });
    add("grass_top", &|t| {
        t.fill(rgb(0x527a3a)).vary(0.07).speckle(rgb(0x456a30), 0.25, 1).speckle(rgb(0x5d8744), 0.18, 2).speckle(rgb(0x3e612b), 0.06, 3);
    });
    // ---- metal / mineral blocks
    add("gold_block", &|t| {
        t.fill(rgb(0xf6d23f)).vary(0.03).speckle(rgb(0xfdf08a), 0.06, 1).bevel(1.15, 0.8).outline(1, 1, 14, 14, rgb(0xe8be2e));
    });
    add("iron_block", &|t| {
        t.fill(rgb(0xdcdcdc)).vary(0.015).bevel(1.08, 0.82).rect(1, 7, 14, 8, rgb(0xc8c8c8));
    });
    add("emerald_block", &|t| {
        t.fill(rgb(0x2ab34e)).vary(0.05).bevel(1.3, 0.75).outline(2, 2, 13, 13, rgb(0x1f9a40));
    });
    add("diamond_block", &|t| {
        t.fill(rgb(0x62ded6)).vary(0.04).bevel(1.25, 0.75).outline(2, 2, 13, 13, rgb(0x4cc7bf));
    });
    add("lapis_block", &|t| {
        t.fill(rgb(0x1f43a1)).vary(0.08).speckle(rgb(0x2c58c4), 0.15, 1).bevel(1.3, 0.75);
    });
    add("netherite_block", &|t| {
        t.fill(rgb(0x433f42)).vary(0.05).bevel(1.25, 0.75).rect(2, 7, 13, 8, rgb(0x383537));
    });
    add("redstone_block", &|t| {
        t.fill(rgb(0xaf1906)).vary(0.06).speckle(rgb(0xe02a10), 0.18, 1).speckle(rgb(0x7f1104), 0.12, 2).bevel(1.25, 0.75);
    });
    add("amethyst_block", &|t| {
        t.fill(rgb(0x8562bf)).vary(0.08).speckle(rgb(0xa98ae0), 0.15, 1).speckle(rgb(0x5f3f96), 0.1, 2);
    });
    add("glowstone", &|t| {
        t.stones(rgb(0xf8d898), rgb(0xc8944c), rgb(0x8a5c2c), 8, 6).vary(0.05);
    });
    add("sea_lantern", &|t| {
        t.fill(rgb(0xacc8be)).vary(0.04).outline(0, 0, 15, 15, rgb(0x8bb0a4)).rect(4, 4, 11, 11, rgb(0xe8f4ee)).outline(4, 4, 11, 11, rgb(0xc8e0d8));
    });
    add("shroomlight", &|t| {
        t.fill(rgb(0xf19a4d)).vary(0.08).speckle(rgb(0xfccb7a), 0.15, 1);
    });
    add("nether_wart_block", &|t| {
        t.fill(rgb(0x730303)).vary(0.1).speckle(rgb(0x8f1010), 0.15, 1);
    });
    add("warped_wart_block", &|t| {
        t.fill(rgb(0x167b7b)).vary(0.1).speckle(rgb(0x1f9a9a), 0.15, 1);
    });
    add("dried_kelp_side", &|t| {
        t.fill(rgb(0x333e25)).vary(0.1).map(|x, _, c| if x % 4 == 0 { scale(c, 0.7) } else { c });
    });
    add("dried_kelp_top", &|t| {
        t.fill(rgb(0x3a4628)).vary(0.1).outline(2, 2, 13, 13, rgb(0x262f1a));
    });
    add("waxed_copper_grate", &|t| {
        t.fill(CLEAR);
        for y in 0..16 {
            for x in 0..16 {
                let bar = x % 4 == 0 || y % 4 == 0;
                if bar {
                    t.set(x, y, scale(rgb(0xc06c50), 0.9 + 0.2 * noise(x, y, 9)));
                }
            }
        }
        t.outline(0, 0, 15, 15, rgb(0xa45a42));
    });
    // ---- wood
    add("oak_planks", &|t| {
        t.planks(rgb(0xa2834f));
    });
    add("spruce_planks", &|t| {
        t.planks(rgb(0x735535));
    });
    add("oak_log", &|t| {
        t.fill(rgb(0x6d5534)).map(|x, y, c| {
            let s = 0.85 + 0.25 * noise(x, y / 5, 77) + if x % 5 == 2 { -0.15 } else { 0.0 };
            scale(c, s)
        });
    });
    add("oak_log_top", &|t| {
        t.fill(rgb(0xb39363)).map(|x, y, c| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d > 7.0 {
                rgb(0x6d5534)
            } else if (d as i32) % 2 == 0 {
                scale(c, 0.85)
            } else {
                c
            }
        });
    });
    add("oak_leaves", &|t| {
        let s = t.seed;
        t.map(|x, y, _| {
            let n = noise(x, y, s);
            if n < 0.2 { CLEAR } else { scale(rgb(0x4c8a2f), 0.7 + 0.5 * noise(x, y, s + 1)) }
        });
    });
    add("bookshelf", &|t| {
        t.planks(rgb(0xa2834f));
        let books = [0x8c2a2a, 0x2a4d8c, 0x3c7a3c, 0x8c6a2a, 0x6a2a8c, 0x2a7a7a, 0x9a4a1a, 0x4a4a4a];
        for shelf in 0..2 {
            let y0 = 1 + shelf * 8;
            for b in 0..7 {
                let c = rgb(books[(b + shelf as usize * 3) % books.len()]);
                let h = 5 + (noise(b as i32, shelf, 3) * 2.0) as i32;
                t.rect(1 + b as i32 * 2, y0 + 6 - h, 1 + b as i32 * 2, y0 + 5, c);
                t.rect(2 + b as i32 * 2, y0 + 6 - h, 2 + b as i32 * 2, y0 + 5, scale(c, 0.8));
            }
        }
    });
    add("crafting_table_top", &|t| {
        t.planks(rgb(0xa2834f));
        t.outline(0, 0, 15, 15, rgb(0x6b4f2a)).rect(0, 5, 15, 5, rgb(0x6b4f2a)).rect(0, 10, 15, 10, rgb(0x6b4f2a)).rect(5, 0, 5, 15, rgb(0x6b4f2a)).rect(10, 0, 10, 15, rgb(0x6b4f2a));
    });
    add("crafting_table_side", &|t| {
        t.planks(rgb(0xa2834f));
        t.rect(0, 0, 15, 2, rgb(0x6b4f2a)).rect(3, 5, 4, 12, rgb(0x8a8a8a)).rect(10, 5, 12, 6, rgb(0x8a8a8a)).rect(11, 7, 11, 12, rgb(0x6b4f2a));
    });
    add("pumpkin_side", &|t| {
        t.fill(rgb(0xc57719)).map(|x, y, c| scale(c, if x % 4 == 0 { 0.8 } else { 0.95 + 0.1 * noise(x, y, 5) }));
    });
    add("pumpkin_top", &|t| {
        t.fill(rgb(0xc57719)).vary(0.05).rect(7, 7, 8, 8, rgb(0x5e7a2a));
    });
    add("white_glazed_terracotta", &|t| {
        t.fill(rgb(0xe9f0ea)).map(|x, y, c| {
            let d = (x - y).abs();
            if d < 2 || (x + y - 15).abs() < 1 && x < 8 { rgb(0x4ea6c4) } else if (x - 3).abs() + (y - 12).abs() < 3 { rgb(0xe7c35a) } else { c }
        });
    });
    // ---- wool, glass
    add("white_wool", &|t| {
        t.fill(rgb(0xe9ecec)).vary(0.04).map(|x, y, c| if (x + y) % 3 == 0 { scale(c, 0.95) } else { c });
    });
    add("orange_wool", &|t| {
        t.fill(rgb(0xf07613)).vary(0.04).map(|x, y, c| if (x + y) % 3 == 0 { scale(c, 0.93) } else { c });
    });
    add("black_wool", &|t| {
        t.fill(rgb(0x15151a)).vary(0.1).map(|x, y, c| if (x + y) % 3 == 0 { scale(c, 0.85) } else { c });
    });
    add("glass", &|t| {
        t.fill(CLEAR).outline(0, 0, 15, 15, rgba(0xdbeef2, 255)).rect(2, 2, 3, 3, rgba(0xffffff, 230)).rect(4, 4, 4, 4, rgba(0xffffff, 200)).rect(3, 3, 3, 3, rgba(0xffffff, 230));
        t.rect(1, 1, 1, 1, rgba(0xa9c9cf, 255)).rect(14, 14, 14, 14, rgba(0xa9c9cf, 255));
    });
    add("tinted_glass", &|t| {
        t.fill(rgba(0x2a2230, 200)).outline(0, 0, 15, 15, rgba(0x4a3e56, 255));
    });
    add("white_stained_glass", &|t| {
        t.fill(rgba(0xffffff, 100)).outline(0, 0, 15, 15, rgba(0xf0f0f0, 180));
    });
    add("black_stained_glass", &|t| {
        t.fill(rgba(0x191919, 150)).outline(0, 0, 15, 15, rgba(0x101010, 200));
    });
    add("orange_stained_glass", &|t| {
        t.fill(rgba(0xd87f33, 110)).outline(0, 0, 15, 15, rgba(0xe89a50, 190)).rect(2, 2, 3, 3, rgba(0xf8c890, 140));
    });
    add("marker_lit", &|t| {
        t.fill(rgba(0xff9a1a, 205)).outline(0, 0, 15, 15, rgba(0xffe08a, 250)).outline(1, 1, 14, 14, rgba(0xffc04a, 230)).rect(2, 2, 3, 3, rgba(0xfff0c0, 235));
    });
    // a powered output's box (the viewer's OUT box, red while powered): a translucent red fill
    add("out_powered", &|t| {
        t.fill(rgba(0xff3b30, 72));
    });
    // a mob farm's golem spawn spots (`mobfarm::MobRun::spots`, tinted per cell): a cell where a
    // golem can spawn, one the search reaches but refuses (dim), and a golem's spawn flash
    add("spawn_spot", &|t| {
        t.fill(rgba(0xffffff, 70)).outline(0, 0, 15, 15, rgba(0xffffff, 210));
    });
    add("spawn_spot_dim", &|t| {
        t.fill(rgba(0xffffff, 34)).outline(0, 0, 15, 15, rgba(0xffffff, 80));
    });
    add("spawn_flash", &|t| {
        t.fill(rgba(0xffffff, 150));
    });
    add("grass_side", &|t| {
        t.fill(rgb(0x866043)).vary(0.1).map(|x, y, c| {
            let fringe = 2 + ((x * 7 + 3) % 5 == 0) as i32 + ((x * 3) % 4 == 1) as i32;
            if y < fringe { rgb(0x5d8a3a) } else { c }
        });
    });
    add("lime_stained_glass", &|t| {
        t.fill(rgba(0x7fcc19, 110)).outline(0, 0, 15, 15, rgba(0x99dd33, 190)).rect(2, 2, 3, 3, rgba(0xc8f090, 140));
    });
    add("slime_block", &|t| {
        t.fill(rgba(0x79c05f, 170)).outline(0, 0, 15, 15, rgba(0x5fa047, 220)).outline(3, 3, 12, 12, rgba(0x5fa047, 200)).rect(4, 4, 11, 11, rgba(0x8bd17a, 190));
    });
    add("honey_block", &|t| {
        t.fill(rgba(0xf9b233, 170)).outline(0, 0, 15, 15, rgba(0xe0961c, 220)).rect(3, 3, 12, 12, rgba(0xfbc350, 180));
    });
    // ---- redstone
    add("dust_dot", &|t| {
        t.fill(CLEAR);
        for y in 0..16 {
            for x in 0..16 {
                let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
                if d < 4.2 {
                    t.set(x, y, if d < 2.5 { rgb(0xffffff) } else { rgb(0xc8c8c8) });
                }
            }
        }
    });
    add("dust_line", &|t| {
        t.fill(CLEAR);
        for y in 0..16 {
            for x in 5..11 {
                let edge = x == 5 || x == 10;
                let n = noise(x, y, 41);
                if edge && n < 0.35 {
                    continue;
                }
                t.set(x, y, if edge { rgb(0xb4b4b4) } else { scale(rgb(0xffffff), 0.9 + 0.1 * n) });
            }
        }
    });
    add("torch_on", &|t| {
        t.fill(CLEAR).rect(7, 8, 8, 15, rgb(0x6b4f2a)).rect(7, 9, 7, 15, rgb(0x87663a)).rect(7, 6, 8, 7, rgb(0xff4b2b)).rect(7, 6, 7, 6, rgb(0xffd0a0));
    });
    add("torch_off", &|t| {
        t.fill(CLEAR).rect(7, 8, 8, 15, rgb(0x6b4f2a)).rect(7, 9, 7, 15, rgb(0x87663a)).rect(7, 6, 8, 7, rgb(0x5c1a10));
    });
    add("torch_top_on", &|t| {
        t.fill(CLEAR).rect(7, 7, 8, 8, rgb(0xff6040));
    });
    add("torch_top_off", &|t| {
        t.fill(CLEAR).rect(7, 7, 8, 8, rgb(0x5c1a10));
    });
    add("lever_handle", &|t| {
        t.fill(CLEAR).rect(7, 6, 8, 15, rgb(0x6b4f2a)).rect(7, 6, 7, 15, rgb(0x87663a));
    });
    add("repeater_top", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a)).rect(7, 1, 8, 14, rgb(0x6e1a12));
    });
    add("repeater_top_on", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a)).rect(7, 1, 8, 14, rgb(0xe8301a));
    });
    add("comparator_top", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a)).rect(4, 3, 4, 12, rgb(0x6e1a12)).rect(11, 3, 11, 12, rgb(0x6e1a12)).rect(4, 12, 11, 12, rgb(0x6e1a12));
    });
    add("comparator_top_on", &|t| {
        t.fill(rgb(0xa0a0a0)).vary(0.025).outline(0, 0, 15, 15, rgb(0x8a8a8a)).rect(4, 3, 4, 12, rgb(0xe8301a)).rect(11, 3, 11, 12, rgb(0xe8301a)).rect(4, 12, 11, 12, rgb(0xe8301a));
    });
    add("bedrock", &|t| {
        t.fill(rgb(0x565656)).vary(0.2).speckle(rgb(0x2a2a2a), 0.25, 1).speckle(rgb(0x8a8a8a), 0.1, 2);
    });
    add("observer_front", &|t| {
        // a dark face: two square eyes and a mouth slot in a stone frame
        t.fill(rgb(0x6f6f6f)).vary(0.05).grain(rgb(0x5f5f5f), 0.12, 1).outline(0, 0, 15, 15, rgb(0x4a4a4a));
        t.rect(2, 2, 13, 13, rgb(0x2f2f2f)).outline(2, 2, 13, 13, rgb(0x3c3c3c));
        t.rect(4, 4, 6, 7, rgb(0x101010)).rect(9, 4, 11, 7, rgb(0x101010)).rect(4, 10, 11, 11, rgb(0x181818));
        t.rect(5, 5, 5, 5, rgb(0x707070)).rect(10, 5, 10, 5, rgb(0x707070));
    });
    add("observer_back", &|t| {
        t.fill(rgb(0x6f6f6f)).vary(0.05).grain(rgb(0x5f5f5f), 0.12, 1).outline(0, 0, 15, 15, rgb(0x4a4a4a)).rect(3, 3, 12, 12, rgb(0x353535)).rect(6, 6, 9, 9, rgb(0x1c1010)).rect(7, 7, 8, 8, rgb(0x3a1010));
    });
    add("observer_back_on", &|t| {
        t.fill(rgb(0x6f6f6f)).vary(0.05).grain(rgb(0x5f5f5f), 0.12, 1).outline(0, 0, 15, 15, rgb(0x4a4a4a)).rect(3, 3, 12, 12, rgb(0x353535)).rect(6, 6, 9, 9, rgb(0xc01c10)).rect(7, 7, 8, 8, rgb(0xff6a40));
    });
    add("observer_side", &|t| {
        // light stone bands top and bottom, a dark body with a ridge (the game's look)
        t.fill(rgb(0x3a3a3a)).vary(0.08);
        t.rect(0, 0, 15, 3, rgb(0x8c8c8c)).rect(0, 13, 15, 15, rgb(0x8c8c8c));
        t.map(|x, y, c| if y <= 3 || y >= 13 { scale(c, 0.9 + 0.2 * noise(x, y, 31)) } else { c });
        t.rect(0, 3, 15, 3, rgb(0x5e5e5e)).rect(0, 13, 15, 13, rgb(0x5e5e5e)).rect(1, 8, 14, 8, rgb(0x4a4a4a)).rect(1, 7, 14, 7, rgb(0x2c2c2c));
    });
    add("observer_top", &|t| {
        t.fill(rgb(0x8a8a8a)).vary(0.06).grain(rgb(0x7a7a7a), 0.12, 2).outline(0, 0, 15, 15, rgb(0x5a5a5a));
        t.rect(2, 6, 13, 9, rgb(0x3a3a3a)).rect(3, 7, 12, 8, rgb(0x262626)).rect(6, 2, 9, 4, rgb(0x6a6a6a)).rect(7, 1, 8, 1, rgb(0x6a6a6a));
    });
    add("lamp_off", &|t| {
        t.fill(rgb(0x5b3a21)).vary(0.08).outline(0, 0, 15, 15, rgb(0x2e1c0e)).map(|x, y, c| {
            let d = (x as i32 - 7).abs().min((x as i32 - 8).abs()) + (y as i32 - 7).abs().min((y as i32 - 8).abs());
            if (1..15).contains(&x) && (1..15).contains(&y) && (d == 5 || d == 2) { rgb(0x8a5a32) } else { c }
        });
    });
    add("lamp_on", &|t| {
        t.fill(rgb(0xc98a3c)).vary(0.06).outline(0, 0, 15, 15, rgb(0x6a4218)).map(|x, y, c| {
            let d = (x as i32 - 7).abs().min((x as i32 - 8).abs()) + (y as i32 - 7).abs().min((y as i32 - 8).abs());
            if (1..15).contains(&x) && (1..15).contains(&y) && (d <= 2 || d == 5) { rgb(0xffe6a8) } else if d == 3 || d == 6 { rgb(0xf0b860) } else { c }
        });
    });
    for (name, base) in [("bulb_0", 0xb4684d_u32), ("bulb_1", 0xa87f68), ("bulb_2", 0x6a9a7c), ("bulb_3", 0x52a28a)] {
        let base = rgb(base);
        let off: &'static str = Box::leak(name.to_string().into_boxed_str());
        let on: &'static str = Box::leak(format!("{name}_lit").into_boxed_str());
        // four petals in the quadrants around the filament, a copper frame
        let petals = move |t: &mut Tile, petal: Rgba, core: Rgba| {
            t.fill(base).vary(0.06).outline(0, 0, 15, 15, scale(base, 0.7)).rect(2, 2, 13, 13, scale(base, 0.55));
            for (x0, y0) in [(3, 3), (9, 3), (3, 9), (9, 9)] {
                t.rect(x0, y0, x0 + 3, y0 + 3, petal).rect(x0 + 1, y0 + 1, x0 + 2, y0 + 2, scale(petal, 1.12));
            }
            t.rect(7, 2, 8, 13, scale(base, 0.8)).rect(2, 7, 13, 8, scale(base, 0.8)).rect(7, 7, 8, 8, core);
        };
        add(off, &|t| petals(t, rgb(0x6e4a3a), rgb(0x3a2620)));
        add(on, &|t| petals(t, rgb(0xffd77a), rgb(0xfff4d0)));
        let off_p: &'static str = Box::leak(format!("{name}_p").into_boxed_str());
        let on_p: &'static str = Box::leak(format!("{name}_lit_p").into_boxed_str());
        add(off_p, &|t| petals(t, rgb(0x6e4a3a), rgb(0xd02010)));
        add(on_p, &|t| petals(t, rgb(0xffd77a), rgb(0xe02810)));
    }
    add("note_block", &|t| {
        t.planks(rgb(0x6a432c));
        t.outline(0, 0, 15, 15, rgb(0x3a2418)).rect(3, 3, 12, 12, rgb(0x2a1a12)).map(|x, y, c| if (3..13).contains(&x) && (3..13).contains(&y) && (x + y) % 2 == 0 { rgb(0x4a3020) } else { c });
    });
    add("target_side", &|t| {
        t.fill(rgb(0xe6ddd0)).map(|x, y, c| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 1.6 || (3.0..4.6).contains(&d) || (6.0..7.6).contains(&d) { rgb(0xc0392b) } else { c }
        });
    });
    add("target_top", &|t| {
        t.fill(rgb(0xc8b06a)).vary(0.1).map(|x, y, c| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 1.6 || (3.0..4.6).contains(&d) { rgb(0xc0392b) } else { c }
        });
    });
    add("iron_trapdoor", &|t| {
        t.fill(rgb(0xcfcfcf)).vary(0.03).outline(0, 0, 15, 15, rgb(0x9a9a9a)).map(|x, y, c| if (2..14).contains(&x) && (2..14).contains(&y) && x % 4 != 1 && y % 4 != 1 && (x % 4 == 3 || y % 4 == 3) { CLEAR } else { c });
    });
    add("oak_trapdoor", &|t| {
        t.planks(rgb(0x9d7a47));
        t.outline(0, 0, 15, 15, rgb(0x6b5030)).map(|x, y, c| if (3..7).contains(&x) && (3..7).contains(&y) || (9..13).contains(&x) && (3..7).contains(&y) || (3..7).contains(&x) && (9..13).contains(&y) || (9..13).contains(&x) && (9..13).contains(&y) { CLEAR } else { c });
    });
    add("iron_door_top", &|t| {
        t.fill(rgb(0xcfcfcf)).vary(0.03).outline(0, 0, 15, 15, rgb(0x9a9a9a)).rect(3, 3, 6, 7, CLEAR).rect(9, 3, 12, 7, CLEAR).rect(3, 10, 12, 12, rgb(0xb0b0b0));
    });
    add("iron_door_bottom", &|t| {
        t.fill(rgb(0xcfcfcf)).vary(0.03).outline(0, 0, 15, 15, rgb(0x9a9a9a)).rect(3, 2, 12, 5, rgb(0xb8b8b8)).rect(3, 8, 12, 13, rgb(0xb8b8b8)).rect(12, 6, 13, 7, rgb(0x707070));
    });
    add("oak_door_top", &|t| {
        t.planks(rgb(0x9d7a47));
        t.outline(0, 0, 15, 15, rgb(0x6b5030)).rect(3, 3, 6, 7, CLEAR).rect(9, 3, 12, 7, CLEAR);
    });
    add("oak_door_bottom", &|t| {
        t.planks(rgb(0x9d7a47));
        t.outline(0, 0, 15, 15, rgb(0x6b5030)).rect(12, 6, 13, 7, rgb(0x404040));
    });
    add("rail_base", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
    });
    add("rail", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0x8a8a8a)).rect(12, 0, 13, 15, rgb(0x8a8a8a));
    });
    // a curve joining the south and east edges (the game's rail_corner, shape south_east)
    add("rail_corner", &|t| {
        t.fill(CLEAR);
        for y in 0..16 {
            for x in 0..16 {
                let (dx, dy) = (16.0 - (x as f32 + 0.5), 16.0 - (y as f32 + 0.5));
                let r = (dx * dx + dy * dy).sqrt();
                let a = dy.atan2(dx).to_degrees();
                let sleeper = (a % 22.5) < 6.0 && (1.5..14.5).contains(&r);
                if (3.0..4.2).contains(&r) || (12.8..14.0).contains(&r) {
                    t.px[y * 16 + x] = rgb(0x8a8a8a);
                } else if sleeper {
                    t.px[y * 16 + x] = rgb(0x6b5034);
                }
            }
        }
    });
    add("powered_rail", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0xe7c35a)).rect(12, 0, 13, 15, rgb(0xe7c35a)).rect(7, 0, 8, 15, rgb(0x5a1a12));
    });
    add("powered_rail_on", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0xe7c35a)).rect(12, 0, 13, 15, rgb(0xe7c35a)).rect(7, 0, 8, 15, rgb(0xff3a1a));
    });
    add("activator_rail", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0x8a8a8a)).rect(12, 0, 13, 15, rgb(0x8a8a8a)).rect(7, 0, 8, 15, rgb(0x5a1a12));
    });
    add("activator_rail_on", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0x8a8a8a)).rect(12, 0, 13, 15, rgb(0x8a8a8a)).rect(7, 0, 8, 15, rgb(0xff3a1a));
    });
    add("detector_rail", &|t| {
        t.fill(CLEAR);
        for y in [1, 5, 9, 13] {
            t.rect(1, y, 14, y + 1, rgb(0x6b5034));
        }
        t.rect(2, 0, 3, 15, rgb(0x8a8a8a)).rect(12, 0, 13, 15, rgb(0x8a8a8a)).rect(5, 6, 10, 9, rgb(0x5a5a5a)).rect(6, 7, 9, 8, rgb(0x7a1a12));
    });
    add("furnace_side", &|t| {
        t.fill(rgb(0x787878)).vary(0.05).grain(rgb(0x666666), 0.1, 1).outline(0, 0, 15, 15, rgb(0x5a5a5a));
    });
    add("furnace_top", &|t| {
        t.fill(rgb(0x6e6e6e)).vary(0.04).outline(0, 0, 15, 15, rgb(0x555555)).outline(2, 2, 13, 13, rgb(0x5f5f5f));
    });
    add("dispenser_front", &|t| {
        t.fill(rgb(0x787878)).vary(0.05).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(4, 4, 11, 11, rgb(0x2a2a2a)).rect(5, 6, 10, 9, rgb(0x141414)).rect(6, 5, 9, 10, rgb(0x141414));
    });
    add("dropper_front", &|t| {
        t.fill(rgb(0x787878)).vary(0.05).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(4, 4, 11, 11, rgb(0x2a2a2a)).rect(6, 6, 9, 9, rgb(0x141414));
    });
    // facing up or down: the hole in the furnace-top face (the game's *_front_vertical)
    add("dispenser_front_vertical", &|t| {
        t.fill(rgb(0x6e6e6e)).vary(0.05).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(4, 4, 11, 11, rgb(0x2a2a2a)).rect(5, 6, 10, 9, rgb(0x141414)).rect(6, 5, 9, 10, rgb(0x141414));
    });
    add("dropper_front_vertical", &|t| {
        t.fill(rgb(0x6e6e6e)).vary(0.05).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(4, 4, 11, 11, rgb(0x2a2a2a)).rect(6, 6, 9, 9, rgb(0x141414));
    });
    add("crafter_top", &|t| {
        t.fill(rgb(0x6e6a68)).vary(0.04).outline(0, 0, 15, 15, rgb(0x4a4644)).rect(2, 2, 13, 13, rgb(0x5a5654)).map(|x, y, c| if (2..14).contains(&x) && (2..14).contains(&y) && (x % 4 == 1 || y % 4 == 1) { rgb(0x403c3a) } else { c });
    });
    add("crafter_side", &|t| {
        t.fill(rgb(0x6e6a68)).vary(0.04).outline(0, 0, 15, 15, rgb(0x4a4644)).rect(3, 6, 12, 9, rgb(0xb06030)).rect(4, 7, 11, 8, rgb(0x803c18));
    });
    add("crafter_front", &|t| {
        t.fill(rgb(0x6e6a68)).vary(0.04).outline(0, 0, 15, 15, rgb(0x4a4644)).rect(4, 4, 11, 11, rgb(0x2a2626)).outline(4, 4, 11, 11, rgb(0xb06030));
    });
    add("crafter_front_on", &|t| {
        t.fill(rgb(0x6e6a68)).vary(0.04).outline(0, 0, 15, 15, rgb(0x4a4644)).rect(4, 4, 11, 11, rgb(0xffb050)).outline(4, 4, 11, 11, rgb(0xe07030));
    });
    add("hopper_outside", &|t| {
        t.fill(rgb(0x464646)).vary(0.06).outline(0, 0, 15, 15, rgb(0x2e2e2e));
    });
    add("hopper_inside", &|t| {
        t.fill(rgb(0x2a2a2a)).vary(0.06);
    });
    add("hopper_top", &|t| {
        t.fill(rgb(0x464646)).vary(0.06).rect(2, 2, 13, 13, rgb(0x2a2a2a)).map(|x, y, c| if (3..13).contains(&x) && (3..13).contains(&y) && (x % 3 == 0 || y % 3 == 0) { rgb(0x555555) } else { c });
    });
    add("piston_side", &|t| {
        t.fill(rgb(0x7f7f7f)).vary(0.06).grain(rgb(0x6c6c6c), 0.1, 1).rect(0, 4, 15, 4, rgb(0x5e5e5e)).outline(0, 4, 15, 15, rgb(0x5a5a5a));
        for y in 0..4 {
            for x in 0..16 {
                let c = if y == 3 { rgb(0x6e5634) } else { scale(rgb(0xb4955f), 0.92 + 0.12 * noise(x / 3, y, 8)) };
                t.set(x, y, c);
            }
        }
    });
    add("piston_top", &|t| {
        t.planks(rgb(0xb4955f));
        t.outline(0, 0, 15, 15, rgb(0x7a7a7a)).outline(1, 1, 14, 14, rgb(0x8f8f8f)).rect(6, 6, 9, 9, rgb(0x8a8a8a)).outline(6, 6, 9, 9, rgb(0x6a6a6a));
    });
    add("piston_top_sticky", &|t| {
        t.planks(rgb(0xb4955f));
        t.outline(0, 0, 15, 15, rgb(0x7a7a7a)).rect(2, 2, 13, 13, rgb(0x76b35a)).vary(0.06).outline(2, 2, 13, 13, rgb(0x5e9a46)).rect(5, 5, 10, 10, rgb(0x8bcf6c));
    });
    add("piston_bottom", &|t| {
        t.fill(rgb(0x7f7f7f)).vary(0.06).grain(rgb(0x6c6c6c), 0.1, 1).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(5, 5, 10, 10, rgb(0x5e5e5e)).outline(5, 5, 10, 10, rgb(0x4a4a4a));
    });
    add("piston_inner", &|t| {
        t.fill(rgb(0x7f7f7f)).vary(0.06).outline(0, 0, 15, 15, rgb(0x5a5a5a)).rect(4, 4, 11, 11, rgb(0x4a4a4a)).rect(6, 6, 9, 9, rgb(0x2a2a2a));
    });
    add("piston_arm", &|t| {
        t.map(|x, y, _| scale(rgb(0xb4955f), 0.9 + 0.15 * noise(x, y / 4, 12)));
    });
    add("sculk_top", &|t| {
        t.fill(rgb(0x0f3a40)).vary(0.15).speckle(rgb(0x1d6066), 0.15, 1).outline(0, 0, 15, 15, rgb(0x0a2a2e));
    });
    add("sculk_side", &|t| {
        t.fill(CLEAR).rect(0, 8, 15, 15, rgb(0x0f3a40)).map(|x, y, c| if y >= 8 { scale(c, 0.85 + 0.3 * noise(x, y, 3)) } else { c }).rect(0, 8, 15, 8, rgb(0x1d6066));
    });
    add("sculk_tendril", &|t| {
        t.fill(CLEAR).rect(7, 4, 8, 15, rgb(0x1d5a60)).rect(7, 3, 8, 3, rgb(0x2a7a80)).rect(6, 8, 6, 10, rgb(0x1d5a60)).rect(9, 6, 9, 8, rgb(0x1d5a60));
    });
    add("sculk_tendril_active", &|t| {
        t.fill(CLEAR).rect(7, 4, 8, 15, rgb(0x3ad8e8)).rect(7, 3, 8, 3, rgb(0xb0f8ff)).rect(6, 8, 6, 10, rgb(0x3ad8e8)).rect(9, 6, 9, 8, rgb(0x3ad8e8));
    });
    add("amethyst_cluster", &|t| {
        t.fill(CLEAR).rect(7, 2, 8, 15, rgb(0xc890f2)).rect(5, 7, 6, 15, rgb(0xa070d8)).rect(9, 5, 10, 15, rgb(0xb27ee6)).rect(7, 2, 7, 6, rgb(0xf0d8ff));
    });
    add("skull", &|t| {
        t.fill(rgb(0xc4c4b6)).vary(0.04).rect(4, 6, 6, 8, rgb(0x2a2a2a)).rect(9, 6, 11, 8, rgb(0x2a2a2a)).rect(7, 10, 8, 10, rgb(0x4a4a4a)).rect(5, 12, 10, 12, rgb(0x6a6a5e));
    });
    add("skull_plain", &|t| {
        t.fill(rgb(0xc4c4b6)).vary(0.04);
    });
    add("cake_top", &|t| {
        t.fill(rgb(0xf4f0ec)).vary(0.02).speckle(rgb(0xc8302a), 0.05, 1);
    });
    add("cake_side", &|t| {
        t.fill(rgb(0xc89a6a)).vary(0.04).rect(0, 0, 15, 4, rgb(0xf4f0ec)).rect(0, 4, 15, 4, rgb(0xd8302a)).rect(0, 9, 15, 9, rgb(0xd8302a));
    });
    add("end_portal_frame_top", &|t| {
        t.fill(rgb(0x3b6e5d)).vary(0.08).outline(0, 0, 15, 15, rgb(0xdbd9a6)).outline(1, 1, 14, 14, rgb(0xc6c48e)).rect(4, 4, 11, 11, rgb(0x2a4e44));
    });
    add("end_portal_frame_side", &|t| {
        t.fill(rgb(0xdbd9a6)).vary(0.05).rect(0, 0, 15, 2, rgb(0x3b6e5d)).speckle(rgb(0xc6c48e), 0.1, 1);
    });
    add("ender_eye", &|t| {
        t.fill(rgb(0x2a7a5a)).vary(0.1).rect(5, 5, 10, 10, rgb(0x1a3a2a)).rect(7, 7, 8, 8, rgb(0x0a1a12));
    });
    add("composter_side", &|t| {
        t.planks(rgb(0x8f6a3c));
        t.outline(0, 0, 15, 15, rgb(0x5e4426));
    });
    add("compost", &|t| {
        t.fill(rgb(0x5a4a2a)).vary(0.15).speckle(rgb(0x6e8a3a), 0.15, 1);
    });
    add("respawn_anchor_side", &|t| {
        t.fill(rgb(0x1e1426)).vary(0.2).grain(rgb(0x3a2a4a), 0.12, 1).outline(0, 0, 15, 15, rgb(0x120a18));
    });
    add("respawn_anchor_top", &|t| {
        t.fill(rgb(0x1e1426)).vary(0.2).rect(4, 4, 11, 11, rgb(0x5a1a8a)).rect(6, 6, 9, 9, rgb(0xc34cf2));
    });
    add("scaffolding_top", &|t| {
        t.fill(CLEAR).outline(0, 0, 15, 15, rgb(0xd8b45a)).outline(1, 1, 14, 14, rgb(0xb8943a)).rect(7, 1, 8, 14, rgb(0xc8a44a)).rect(1, 7, 14, 8, rgb(0xc8a44a));
    });
    add("scaffolding_side", &|t| {
        t.fill(CLEAR).outline(0, 0, 15, 15, rgb(0xd8b45a)).rect(1, 1, 1, 14, rgb(0xb8943a)).rect(14, 1, 14, 14, rgb(0xb8943a)).map(|x, y, c| if (x - y).abs() <= 0 { rgb(0xc8a44a) } else { c });
    });
    add("jukebox_side", &|t| {
        t.planks(rgb(0x7a4e33));
        t.outline(0, 0, 15, 15, rgb(0x4a2e1e)).rect(0, 4, 15, 4, rgb(0x4a2e1e));
    });
    add("jukebox_top", &|t| {
        t.planks(rgb(0x7a4e33));
        t.outline(0, 0, 15, 15, rgb(0x4a2e1e)).rect(4, 6, 11, 9, rgb(0x1a1a1a));
    });
    add("water", &|t| {
        t.fill(rgba(0x3f76e4, 170)).vary(0.05);
    });
    // lava's stand-in until the game's texture loads (`lava_still`: orange, opaque)
    add("lava", &|t| {
        t.fill(rgb(0xd5600f)).vary(0.12).map(|x, y, c| if (x * 5 + y * 3) % 7 == 0 { rgb(0xf6b23a) } else { c });
    });
    // the game's water sides use the flowing texture (its top-left quarter: `FluidRenderer`)
    add("water_flow", &|t| {
        t.fill(rgba(0x3f76e4, 170)).vary(0.05);
    });
    add("bubbles", &|t| {
        t.fill(rgba(0x5a8cf0, 150)).vary(0.06).map(|x, y, c| if (x * 7 + y * 13) % 11 == 0 { rgba(0xd8e8ff, 210) } else { c });
    });
    // an explosion particle's stand-in until the game's sprite loads (`particles`): a white puff
    add("explosion_puff", &|t| {
        t.fill(CLEAR).map(|x, y, _| {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            if dx * dx + dy * dy <= 49.0 { rgb(0xffffff) } else { CLEAR }
        });
    });
    add("magma", &|t| {
        t.stones(rgb(0x8a3a10), rgb(0x5a1e08), rgb(0xff8a20), 8, 9).vary(0.08);
    });
    add("dripleaf_top", &|t| {
        t.fill(rgb(0x5f9a2c)).vary(0.08).map(|x, y, c| if (x - y).abs() < 1 || (x + y - 15).abs() < 1 { rgb(0x4a7e22) } else { c }).outline(0, 0, 15, 15, rgb(0x4a7e22));
    });
    add("tripwire", &|t| {
        t.fill(CLEAR).rect(0, 7, 15, 8, rgba(0xdedede, 230));
    });
    add("snow", &|t| {
        t.fill(rgb(0xf4fcfc)).vary(0.02);
    });
    add("podzol_top", &|t| {
        t.fill(rgb(0x5b3f1d)).vary(0.1).speckle(rgb(0x7a5a2a), 0.2, 1);
    });
    add("daylight_detector_top", &|t| {
        t.fill(rgb(0xa08a6a)).vary(0.04).rect(2, 2, 13, 13, rgb(0x3a5a7a)).map(|x, y, c| if (2..14).contains(&x) && (2..14).contains(&y) && (x % 4 == 1 || y % 4 == 1) { rgb(0x8a7a5a) } else { c });
    });
    add("generic", &|t| {
        t.fill(rgb(0xffffff)).vary(0.05).outline(0, 0, 15, 15, rgb(0xd0d0d0));
    });
    // the hand-drawn vanilla-look tiles replace the procedural ones of the same name
    for (name, f) in art::tiles() {
        seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        let mut t = Tile::new(seed);
        f(&mut t);
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = t,
            None => out.push((name, t)),
        }
    }
    out
}

/// One layer's mip levels (16, 8, 4, 2, 1 px), box-filtered with premultiplied alpha.
fn layer_mips(px: &[Rgba; TILE * TILE]) -> Vec<Vec<u8>> {
    let mut level: Vec<[f32; 4]> = px
        .iter()
        .map(|c| {
            let a = c[3] as f32 / 255.0;
            [c[0] as f32 * a, c[1] as f32 * a, c[2] as f32 * a, a]
        })
        .collect();
    let mut n = TILE;
    let mut out = Vec::new();
    for _ in 0..MIPS {
        let mut bytes = Vec::with_capacity(n * n * 4);
        for p in &level {
            let (r, g, b) = if p[3] > 0.0 { (p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { (0.0, 0.0, 0.0) };
            bytes.extend_from_slice(&[r.round().clamp(0.0, 255.0) as u8, g.round().clamp(0.0, 255.0) as u8, b.round().clamp(0.0, 255.0) as u8, (p[3] * 255.0).round().clamp(0.0, 255.0) as u8]);
        }
        out.push(bytes);
        if n > 1 {
            let m = n / 2;
            let mut next = vec![[0f32; 4]; m * m];
            for y in 0..m {
                for x in 0..m {
                    let mut s = [0f32; 4];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let q = level[(2 * y + dy) * n + 2 * x + dx];
                        for c in 0..4 {
                            s[c] += q[c] / 4.0;
                        }
                    }
                    next[y * m + x] = s;
                }
            }
            level = next;
            n = m;
        }
    }
    out
}

mod art;
pub mod vanilla;

/// The textures as an array (one 16x16 layer per tile, so filtering never bleeds between
/// tiles): RGBA pixels per mip level of every layer, and each name's layer.
#[derive(Clone)]
pub struct Atlas {
    pub names: HashMap<&'static str, u16>,
    /// Number of layers.
    pub layers: usize,
    /// `mips[k]`: every layer's (TILE >> k)^2 RGBA pixels, layer after layer.
    pub mips: Vec<Vec<u8>>,
    /// The game's block models and textures, once loaded (`gamemodel`): faces that name a game
    /// texture (`block/...`) sample [`GameAssets`]'s 2D texture (vertex layer [`GAME_LAYER`]).
    pub game: Option<std::rc::Rc<GameAssets>>,
}

/// A vertex `layer` that samples the game texture instead of the tile array.
pub const GAME_LAYER: u32 = 65535;

/// [`GAME_LAYER`] with the game's red overlay (`OverlayTexture`'s hurt row: a hurt or dying mob,
/// `color = mix(red, color, 0xB2 / 255)`): the solid shader mixes toward red, not white, by
/// 1 - the vertex alpha.
pub const GAME_LAYER_RED: u32 = 65534;

/// The game's models with its block atlas as one 2D texture and that texture's mip levels (and,
/// once loaded, the game's entity and item textures packed below the block atlas).
pub struct GameAssets {
    pub models: std::rc::Rc<crate::gamemodel::GameModels>,
    /// Every texture in the game texture: `block/...`, and `entity/...` / `item/...` once packed.
    pub tex: HashMap<String, crate::gamemodel::TexRect>,
    pub width: u32,
    pub height: u32,
    /// `mips[k]`: the (width >> k) x (height >> k) RGBA level (sprites sit on a 16-pixel grid:
    /// [`MIPS`] levels never mix two sprites).
    pub mips: Vec<Vec<u8>>,
    /// The entity and item textures are in (`add_game_entities`).
    pub entities: bool,
    /// An entity was drawn without its texture: the page should load them.
    pub wants_entities: std::cell::Cell<bool>,
}

/// Mip levels of an RGBA image (box filter, premultiplied alpha).
pub fn image_mips(rgba: &[u8], w: u32, h: u32, levels: usize) -> Vec<Vec<u8>> {
    let mut out = vec![rgba.to_vec()];
    let (mut cw, mut ch) = (w as usize, h as usize);
    for _ in 1..levels {
        let prev = out.last().unwrap();
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                let mut s = [0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(cw - 1), (2 * y + dy).min(ch - 1));
                    let o = (sy * cw + sx) * 4;
                    let a = prev[o + 3] as f32 / 255.0;
                    for c in 0..3 {
                        s[c] += prev[o + c] as f32 * a / 4.0;
                    }
                    s[3] += a / 4.0;
                }
                let o = (y * nw + x) * 4;
                for c in 0..3 {
                    next[o + c] = if s[3] > 0.0 { (s[c] / s[3]).round().clamp(0.0, 255.0) as u8 } else { 0 };
                }
                next[o + 3] = (s[3] * 255.0).round() as u8;
            }
        }
        out.push(next);
        cw = nw;
        ch = nh;
    }
    out
}

impl Atlas {
    /// Take the game's block models (the vendored block definitions and models) with its block atlas.
    pub fn set_game(&mut self, rgba: &[u8], w: u32, h: u32, index: &str, defs: &str, models: &str) -> Result<usize, String> {
        let m = crate::gamemodel::GameModels::parse(defs, models, index, rgba, w, h)?;
        Ok(self.set_game_models(m, rgba, w, h))
    }

    /// [`Atlas::set_game`] with its models already parsed (the browser parses them in a task and
    /// makes the texture's mip levels in the next: 45 ms in one task in a software GL's page).
    pub fn set_game_models(&mut self, m: crate::gamemodel::GameModels, rgba: &[u8], w: u32, h: u32) -> usize {
        let n = m.tex.len();
        let tex = m.tex.clone();
        self.game = Some(std::rc::Rc::new(GameAssets { models: std::rc::Rc::new(m), tex, width: w, height: h, mips: image_mips(rgba, w, h, MIPS), entities: false, wants_entities: Default::default() }));
        n
    }

    /// Pack the game's entity textures (boats, chest boats, the minecart, the armor stand: from
    /// the vendored entity atlas) and its whole item atlas below the block atlas: a new game
    /// texture. Returns how many textures were packed.
    pub fn add_game_entities(&mut self, ent: (&[u8], u32, u32, &str), items: (&[u8], u32, u32, &str)) -> Result<usize, String> {
        let Some(g) = self.game.clone() else { return Err("no game assets".into()) };
        let (bw, bh) = (g.width, g.height);
        let parse = |index: &str| -> Result<Vec<(String, [u32; 4])>, String> {
            let j = crate::json::Json::parse(index).map_err(|e| format!("index: {e}"))?;
            let mut v = Vec::new();
            if let crate::json::Json::Obj(kv) = j {
                for (k, r) in kv {
                    let r: Vec<u32> = r.as_arr().unwrap_or(&[]).iter().filter_map(|x| x.as_f64()).map(|x| x as u32).collect();
                    if r.len() == 4 {
                        v.push((k, [r[0], r[1], r[2], r[3]]));
                    }
                }
            }
            Ok(v)
        };
        let all_ent = parse(ent.3)?;
        let ent_rects: Vec<(String, [u32; 4])> = all_ent
            .iter()
            .filter(|(k, r)| (k.starts_with("entity/boat/") || k.starts_with("entity/chest_boat/") || k == "entity/minecart/minecart" || k == "entity/armorstand/armorstand" || k == "entity/chest/normal" || k.starts_with("entity/shulker/shulker") || k == "entity/zombie/zombie") && r[0] + r[2] <= ent.1 && r[1] + r[3] <= ent.2)
            .cloned()
            .collect();
        // the mob farms' mobs: each texture with its overlay layers composited in
        // (`entitymodel::MOB_COMPOSITES`), packed like the others
        let crop = |r: [u32; 4]| -> Vec<u8> {
            let mut v = Vec::with_capacity((r[2] * r[3] * 4) as usize);
            for row in 0..r[3] {
                let s = (((r[1] + row) * ent.1 + r[0]) * 4) as usize;
                v.extend_from_slice(&ent.0[s..s + (r[2] * 4) as usize]);
            }
            v
        };
        let fits = |r: &[u32; 4]| r[0] + r[2] <= ent.1 && r[1] + r[3] <= ent.2;
        let find = |k: &str| all_ent.iter().find(|(n, r)| n == k && fits(r)).map(|(_, r)| *r);
        let mut composites: Vec<(String, Vec<u8>, [u32; 4])> = Vec::new();
        for (name, base, over) in crate::entitymodel::MOB_COMPOSITES {
            let Some(b) = find(base) else { continue };
            let mut img = crop(b);
            if let Some(o) = over.and_then(find).filter(|o| o[2] == b[2] && o[3] == b[3]) {
                // the layer is drawn again over the model with a cutout (alpha under 0.1 dropped)
                for (d, s) in img.chunks_exact_mut(4).zip(crop(o).chunks_exact(4)) {
                    if s[3] as f32 / 255.0 >= 0.1 {
                        d.copy_from_slice(&[s[0], s[1], s[2], 255]);
                    }
                }
            }
            composites.push((name.to_string(), img, b));
        }
        // items: the whole atlas at (0, bh); entities shelf-packed at x >= its width
        let (iw, ih) = (items.1, items.2);
        let x_start = iw.div_ceil(64) * 64;
        // (name, rect, index of its composited image or None: a rect of the entity atlas)
        let mut order: Vec<(String, [u32; 4], Option<usize>)> = ent_rects.iter().map(|(k, r)| (k.clone(), *r, None)).collect();
        order.extend(composites.iter().enumerate().map(|(i, (k, _, r))| (k.clone(), *r, Some(i))));
        order.sort_by(|a, b| b.1[3].cmp(&a.1[3]).then(a.0.cmp(&b.0)));
        let (mut x, mut y, mut row_h) = (x_start, bh, 0u32);
        let mut placed: Vec<(String, [u32; 4], [u32; 2], Option<usize>)> = Vec::new();
        for (k, r, src) in order {
            if x + r[2] > bw {
                x = x_start;
                y += row_h;
                row_h = 0;
            }
            placed.push((k, r, [x, y], src));
            x += r[2];
            row_h = row_h.max(r[3]);
        }
        let height = (y + row_h).max(bh + ih).div_ceil(16) * 16;
        let mut img = vec![0u8; (bw * height * 4) as usize];
        img[..(bw * bh * 4) as usize].copy_from_slice(&g.mips[0]);
        let mut blit = |src: &[u8], sw: u32, sx: u32, sy: u32, w: u32, h: u32, dx: u32, dy: u32| {
            for row in 0..h {
                let s = (((sy + row) * sw + sx) * 4) as usize;
                let d = (((dy + row) * bw + dx) * 4) as usize;
                img[d..d + (w * 4) as usize].copy_from_slice(&src[s..s + (w * 4) as usize]);
            }
        };
        blit(items.0, iw, 0, 0, iw.min(bw), ih, 0, bh);
        let mut tex = g.tex.clone();
        for (k, r) in parse(items.3)? {
            if r[0] + r[2] <= iw && r[1] + r[3] <= ih {
                tex.insert(k, crate::gamemodel::TexRect { rect: [r[0], bh + r[1], r[2], r[2].min(r[3])], transparent: true });
            }
        }
        for (k, r, [dx, dy], src) in &placed {
            match src {
                Some(i) => blit(&composites[*i].1, r[2], 0, 0, r[2], r[3], *dx, *dy),
                None => blit(ent.0, ent.1, r[0], r[1], r[2], r[3], *dx, *dy),
            }
            tex.insert(k.clone(), crate::gamemodel::TexRect { rect: [*dx, *dy, r[2], r[3]], transparent: true });
        }
        let n = placed.len();
        self.game = Some(std::rc::Rc::new(GameAssets { models: g.models.clone(), tex, width: bw, height, mips: image_mips(&img, bw, height, MIPS), entities: true, wants_entities: Default::default() }));
        Ok(n)
    }

    /// Add the game's particle sprites (`particle/<name>`: the explosion's frames, [`crate::particles`])
    /// below what the game texture holds (after [`Atlas::add_game_entities`]: it repacks below the
    /// block atlas). Returns how many.
    pub fn add_game_particles(&mut self, sprites: &[(String, Vec<u8>, u32, u32)]) -> Result<usize, String> {
        let Some(g) = self.game.clone() else { return Err("no game assets".into()) };
        let (bw, bh) = (g.width, g.height);
        let row_h = sprites.iter().map(|s| s.3).max().unwrap_or(0);
        let mut placed: Vec<(usize, u32, u32)> = Vec::new();
        let (mut x, mut y) = (0u32, bh);
        for (i, s) in sprites.iter().enumerate() {
            if s.1.len() != (s.2 * s.3 * 4) as usize || s.2 > bw {
                return Err(format!("particle {}: bad image", s.0));
            }
            if x + s.2 > bw {
                x = 0;
                y += row_h;
            }
            placed.push((i, x, y));
            x += s.2;
        }
        let height = (y + row_h).div_ceil(16) * 16;
        let mut img = vec![0u8; (bw * height * 4) as usize];
        img[..(bw * bh * 4) as usize].copy_from_slice(&g.mips[0]);
        let mut tex = g.tex.clone();
        for &(i, dx, dy) in &placed {
            let (name, rgba, w, h) = &sprites[i];
            for row in 0..*h {
                let s = (row * w * 4) as usize;
                let d = (((dy + row) * bw + dx) * 4) as usize;
                img[d..d + (w * 4) as usize].copy_from_slice(&rgba[s..s + (w * 4) as usize]);
            }
            tex.insert(format!("particle/{name}"), crate::gamemodel::TexRect { rect: [dx, dy, *w, *h], transparent: true });
        }
        self.game = Some(std::rc::Rc::new(GameAssets { models: g.models.clone(), tex, width: bw, height, mips: image_mips(&img, bw, height, MIPS), entities: g.entities, wants_entities: Default::default() }));
        Ok(placed.len())
    }

    /// Where a sprite is in the textures: the game texture's layer and its rect there (UV 0..1)
    /// when the game's `name` is in, else the hand-drawn `fallback` tile, whole.
    pub fn sprite(&self, name: &str, fallback: &str) -> (u32, [f32; 4]) {
        match self.game.as_ref().and_then(|g| g.tex.get(name).map(|t| (t.rect, g.width as f32, g.height as f32))) {
            Some((r, w, h)) => {
                // inside the sprite by a sliver: nearest sampling never reads the neighbour
                let (x0, y0, x1, y1) = (r[0] as f32 + 0.02, r[1] as f32 + 0.02, (r[0] + r[2]) as f32 - 0.02, (r[1] + r[3]) as f32 - 0.02);
                (GAME_LAYER, [x0 / w, y0 / h, x1 / w, y1 / h])
            }
            None => (self.tile(fallback) as u32, [0.0, 0.0, 1.0, 1.0]),
        }
    }

    pub fn build() -> Atlas {
        let mut tiles = paint_all();
        // wgpu's WebGL2 backend guesses a texture's view kind from its layer count: a multiple of
        // 6 is taken for a cube map array, and our 2D array then samples black (every block and
        // the floor: f141e8417's 180 tiles). A blank layer keeps the count off multiples of 6.
        if tiles.len() % 6 == 0 {
            tiles.push(("pad", Tile::new(0)));
        }
        assert!(tiles.len() <= MAX_LAYERS, "too many tiles for a WebGL2 texture array");
        assert!(tiles.len() % 6 != 0);
        let mut names = HashMap::new();
        let mut mips: Vec<Vec<u8>> = (0..MIPS).map(|_| Vec::new()).collect();
        for (i, (name, t)) in tiles.iter().enumerate() {
            names.insert(*name, i as u16);
            for (k, level) in layer_mips(&t.px).into_iter().enumerate() {
                mips[k].extend_from_slice(&level);
            }
        }
        Atlas { names, layers: tiles.len(), mips, game: None }
    }

    /// Replace one layer's pixels (16x16 RGBA) and its mip levels.
    pub fn set_layer(&mut self, layer: u16, px: &[Rgba; TILE * TILE]) {
        for (k, level) in layer_mips(px).into_iter().enumerate() {
            let n = level.len();
            let o = layer as usize * n;
            self.mips[k][o..o + n].copy_from_slice(&level);
        }
    }

    /// The tile of a name (the generic white tile for unknown names).
    pub fn tile(&self, name: &str) -> u16 {
        self.names.get(name).copied().unwrap_or_else(|| self.names["generic"])
    }

    pub fn has(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }

    /// UV rectangle (0..1 within its layer) of pixel rect (u0, v0, u1, v1) (in 0..16).
    pub fn uv(&self, _tile: u16, r: [f32; 4]) -> [f32; 4] {
        let t = TILE as f32;
        [r[0] / t, r[1] / t, r[2] / t, r[3] / t]
    }
}
