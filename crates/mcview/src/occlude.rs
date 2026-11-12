//! Which of the kept triangles (a mob farm's blocks on the GPU: [`crate::render::Renderer::set_fixed`])
//! a camera can see, so a frame draws only those ([`visible`]). Exact by construction: a triangle
//! is left out only when the GPU would keep none of its samples: it faces away (the pipeline culls
//! back faces), it lies outside the view, or opaque faces in front of it surely cover every sample
//! it could touch. "Surely": each test is conservative at the frame's own pixels, with margins
//! wider than the GPU's sub-pixel snapping and its depth precision: an occluder counts only at a
//! pixel it covers whole, at the farthest depth it has there; a triangle stays when it might touch
//! a pixel at a depth not surely behind. The occluders are the opaque faces facing the camera,
//! coplanar block faces merged into rectangles first (two neighbours each cover only part of the
//! pixels along their shared edge; together they cover them whole).
//!
//! Why: a software GL (WebKit's on a GPU-less box: the phone rig) sets up every triangle it is
//! given on one thread; a farm in its terrain had 19,000 facing the camera in view, a few thousand
//! of them seen, and that setup was most of its 35 ms frame.

use crate::atlas::Atlas;
use crate::camera::Mat4;
use crate::mesh::{Vertex, XRAY};
use std::collections::HashMap;

/// Which textures a face is drawn opaque with at every pixel it covers whole (the alpha test keeps
/// all its samples): the game texture's 16-texel sprites and the viewer's own tiles, each fully
/// opaque or not ([`Opaque::of`]).
#[derive(Clone, Debug, Default)]
pub struct Opaque {
    /// The game texture's sprites, row by row (its width / 16 a row), and its size in texels.
    game: Option<(u32, u32, Vec<bool>)>,
    /// The viewer's own tiles (its texture array's layers).
    layers: Vec<bool>,
}

impl Opaque {
    /// From the atlas the renderer samples: a sprite or tile is opaque when every texel of it is
    /// (its mip levels are box-filtered within it: [`crate::atlas`], so they are too).
    pub fn of(atlas: &Atlas) -> Opaque {
        let tile = crate::atlas::TILE * crate::atlas::TILE * 4;
        let layers = atlas.mips.first().map(|m| m.chunks(tile).map(|l| l.chunks(4).all(|p| p[3] == 255)).collect()).unwrap_or_default();
        let game = atlas.game.as_ref().and_then(|g| {
            let img = g.mips.first()?;
            let (cols, rows) = (g.width / 16, g.height / 16);
            let mut flags = vec![true; (cols * rows) as usize];
            for y in 0..rows * 16 {
                for x in 0..cols * 16 {
                    if img[((y * g.width + x) * 4 + 3) as usize] != 255 {
                        flags[((y / 16) * cols + x / 16) as usize] = false;
                    }
                }
            }
            Some((g.width, g.height, flags))
        });
        Opaque { game, layers }
    }

    /// Whether triangle `t` (its three vertices) hides what is behind it at every pixel it covers
    /// whole: an opaque texture under all of it, not an X-ray face (drawn in front of everything).
    pub fn triangle(&self, t: &[Vertex]) -> bool {
        if t[0].layer & XRAY != 0 {
            return false;
        }
        let layer = t[0].layer & !XRAY;
        if layer >= crate::atlas::GAME_LAYER_MIN {
            let Some((w, h, flags)) = &self.game else { return false };
            let cols = (*w / 16) as i64;
            let rows = (*h / 16) as i64;
            let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for p in t {
                u0 = u0.min(p.uv[0]);
                u1 = u1.max(p.uv[0]);
                v0 = v0.min(p.uv[1]);
                v1 = v1.max(p.uv[1]);
            }
            // (the sprites under the uv box; a box ending on a sprite's edge does not reach the
            // next: a pixel covered whole samples inside the face)
            let lo = |q: f32, size: u32| ((q as f64 * size as f64 + 1e-3) / 16.0).floor() as i64;
            let hi = |q: f32, size: u32| ((q as f64 * size as f64 - 1e-3) / 16.0).floor() as i64;
            let (cx0, cx1, cy0, cy1) = (lo(u0, *w), hi(u1, *w), lo(v0, *h), hi(v1, *h));
            if cx0 < 0 || cy0 < 0 || cx1 >= cols || cy1 >= rows || cx1 < cx0 || cy1 < cy0 {
                return false;
            }
            (cy0..=cy1).all(|y| (cx0..=cx1).all(|x| flags[(y * cols + x) as usize]))
        } else {
            self.layers.get(layer as usize).copied().unwrap_or(false)
        }
    }
}

/// The frame a visible set is for ([`visible`]): the camera's view-projection, its eye, the
/// render distance in chunks (the shader drops fragments of chunks beyond it: no such face hides
/// anything), the frame's size in pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub vp: Mat4,
    pub eye: [f32; 3],
    pub chunks: f32,
    pub width: u32,
    pub height: u32,
}

/// Sub-pixel margin for the GPU's snapping of vertex positions (Vulkan allows as coarse as 1/16 px)
/// and the edge rules: an occluder must cover a pixel this far inside its edges, a triangle counts
/// as touching a pixel this far outside them.
const EDGE_MARGIN: f64 = 1.0 / 16.0;
/// Depth margin (the frame's 24-bit depth buffer: 6e-8 a step; interpolation in 32-bit floats).
const DEPTH_MARGIN: f64 = 1e-5;


/// At most this many edges a polygon (a triangle, or a rectangle's quadrilateral).
const MAX_EDGES: usize = 4;

/// The occlusion buffer's tiles (pixels a side): each keeps its pixels' farthest occluder depth, so
/// a polygon behind a whole tile is decided there.
const TILE: usize = 8;

/// A convex polygon in pixels: its edges' functions (`a x + b y + c`, inside: all >= 0), its
/// depth's plane (0..1, the GPU's), its pixel box (x0, y0, x1, y1: exclusive ends, in the frame),
/// its bounds' slack over a pixel (each edge's (|a| + |b|) / 2, the depth's), and its nearest
/// depth over the box.
struct Poly {
    edges: [[f64; 3]; MAX_EDGES],
    n: usize,
    depth: [f64; 3],
    bbox: [usize; 4],
    half: [f64; MAX_EDGES],
    dz: f64,
    near: f64,
}

impl Poly {
    /// From corners in pixels with their depth, in order around it (either way): None when edge-on.
    fn new(s: &[[f64; 3]], w: usize, h: usize) -> Option<Poly> {
        let n = s.len().min(MAX_EDGES);
        let mut area = 0.0;
        for i in 0..n {
            let (p, q) = (s[i], s[(i + 1) % n]);
            area += p[0] * q[1] - q[0] * p[1];
        }
        let scale: f64 = s.iter().map(|p| p[0].abs() + p[1].abs()).sum::<f64>() / n as f64 + 1.0;
        if area.abs() <= 1e-9 * scale * scale {
            return None;
        }
        let sign = if area > 0.0 { 1.0 } else { -1.0 };
        let mut edges = [[0.0f64; 3]; MAX_EDGES];
        let mut half = [0.0f64; MAX_EDGES];
        for i in 0..n {
            let (p, q) = (s[i], s[(i + 1) % n]);
            // inside on the left of p -> q for a positive area
            let (a, b) = (-(q[1] - p[1]) * sign, (q[0] - p[0]) * sign);
            edges[i] = [a, b, -(a * p[0] + b * p[1])];
            half[i] = (a.abs() + b.abs()) * 0.5;
        }
        // depth's plane through three corners
        let (p0, p1, p2) = (s[0], s[1], s[2]);
        let (ux, uy, uz) = (p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]);
        let (vx, vy, vz) = (p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]);
        let det = ux * vy - uy * vx;
        if det.abs() <= 1e-12 * scale * scale {
            return None;
        }
        let a = (uz * vy - uy * vz) / det;
        let b = (ux * vz - uz * vx) / det;
        let depth = [a, b, p0[2] - a * p0[0] - b * p0[1]];
        let dz = (a.abs() + b.abs()) * 0.5;
        let (x0, x1) = (s.iter().map(|q| q[0]).fold(f64::MAX, f64::min), s.iter().map(|q| q[0]).fold(f64::MIN, f64::max));
        let (y0, y1) = (s.iter().map(|q| q[1]).fold(f64::MAX, f64::min), s.iter().map(|q| q[1]).fold(f64::MIN, f64::max));
        let bbox = if x1 < -1.0 || y1 < -1.0 || x0 > w as f64 + 1.0 || y0 > h as f64 + 1.0 {
            [0, 0, 0, 0]
        } else {
            [(x0 - 1.0).floor().max(0.0) as usize, (y0 - 1.0).floor().max(0.0) as usize, ((x1 + 1.0).ceil().max(0.0) as usize).min(w), ((y1 + 1.0).ceil().max(0.0) as usize).min(h)]
        };
        // (the plane's nearest over the box's pixels, every margin off)
        let mut near = f64::MAX;
        for (x, y) in [(bbox[0], bbox[1]), (bbox[2], bbox[1]), (bbox[0], bbox[3]), (bbox[2], bbox[3])] {
            near = near.min(a * x as f64 + b * y as f64 + depth[2]);
        }
        let near = near - dz * (1.0 + 2.0 * EDGE_MARGIN) - DEPTH_MARGIN;
        Some(Poly { edges, n, depth, bbox, half, dz, near })
    }

    fn empty(&self) -> bool {
        self.bbox[2] <= self.bbox[0] || self.bbox[3] <= self.bbox[1]
    }

    /// Row `y`'s pixels (x0..x1) whose centres pass every edge: `inside`, the whole pixel and the
    /// margin in; else touching it, the margin out.
    fn span(&self, y: usize, inside: bool) -> (usize, usize) {
        let py = y as f64 + 0.5;
        let (mut lo, mut hi) = (self.bbox[0] as f64, self.bbox[2] as f64 - 1.0);
        for k in 0..self.n {
            let [a, b, c] = self.edges[k];
            let m = self.half[k] * (1.0 + 2.0 * EDGE_MARGIN);
            let t = if inside { m } else { -m };
            // a (x + 0.5) + b py + c >= t
            let rest = t - b * py - c;
            if a.abs() < 1e-12 {
                if rest > 0.0 {
                    return (1, 0);
                }
            } else if a > 0.0 {
                let x = rest / a - 0.5;
                // (inside: rounded in, touching: rounded out)
                lo = lo.max(if inside { (x + 1e-9).ceil() } else { (x - 1e-9).ceil() });
            } else {
                let x = rest / a - 0.5;
                hi = hi.min(if inside { (x - 1e-9).floor() } else { (x + 1e-9).floor() });
            }
        }
        if hi < lo {
            return (1, 0);
        }
        (lo as usize, hi as usize + 1)
    }

    /// Write its depth where it covers a pixel whole (the farthest it is there, with the snapping's
    /// share: within a polygon its depth moves at most its slope times the vertices' move), the
    /// nearest kept; the tiles written are marked stale.
    fn occlude(&self, z: &mut ZBuf) {
        let slack = self.dz * (1.0 + 2.0 * EDGE_MARGIN);
        let w = z.w;
        for y in self.bbox[1]..self.bbox[3] {
            let (x0, x1) = self.span(y, true);
            if x1 <= x0 {
                continue;
            }
            let py = y as f64 + 0.5;
            let row = &mut z.px[y * w..y * w + w];
            let mut d = self.depth[0] * (x0 as f64 + 0.5) + self.depth[1] * py + self.depth[2] + slack;
            for cell in &mut row[x0..x1] {
                // (rounded up: never nearer than it is)
                let f = d as f32;
                let f = if (f as f64) < d { f.next_up_f32() } else { f };
                if f < *cell {
                    *cell = f;
                }
                d += self.depth[0];
            }
            let ty = y / TILE;
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                z.stale[ty * z.tw + tx] = true;
            }
        }
    }

    /// Whether it might show a sample: touching a pixel at a depth not surely behind its occluders'.
    fn seen(&self, z: &mut ZBuf) -> bool {
        // (behind every tile under its box: hidden, no pixel looked at)
        if z.behind(self.bbox, self.near) {
            return false;
        }
        let slack = self.dz * (1.0 + 2.0 * EDGE_MARGIN) + DEPTH_MARGIN;
        let w = z.w;
        for y in self.bbox[1]..self.bbox[3] {
            let (x0, x1) = self.span(y, false);
            if x1 <= x0 {
                continue;
            }
            let py = y as f64 + 0.5;
            let row = &z.px[y * w..y * w + w];
            let mut d = self.depth[0] * (x0 as f64 + 0.5) + self.depth[1] * py + self.depth[2] - slack;
            for &cell in &row[x0..x1] {
                if d <= cell as f64 {
                    return true;
                }
                d += self.depth[0];
            }
        }
        false
    }
}

/// The occluders' depths: a pixel's, the farthest an occluder covering it whole is there (the
/// nearest such), and each tile's farthest pixel (read again when stale).
struct ZBuf {
    w: usize,
    px: Vec<f32>,
    tw: usize,
    tile: Vec<f32>,
    stale: Vec<bool>,
}

impl ZBuf {
    fn new(w: usize, h: usize) -> ZBuf {
        let (tw, th) = (w.div_ceil(TILE), h.div_ceil(TILE));
        ZBuf { w, px: vec![f32::INFINITY; w * h], tw, tile: vec![f32::INFINITY; tw * th], stale: vec![false; tw * th] }
    }

    /// Whether depth `near` is behind every pixel of the tiles under `bbox`.
    fn behind(&mut self, bbox: [usize; 4], near: f64) -> bool {
        let h = self.px.len() / self.w;
        for ty in bbox[1] / TILE..=(bbox[3].max(1) - 1) / TILE {
            for tx in bbox[0] / TILE..=(bbox[2].max(1) - 1) / TILE {
                let t = ty * self.tw + tx;
                if self.stale[t] {
                    let mut m = 0.0f32;
                    for y in ty * TILE..((ty + 1) * TILE).min(h) {
                        for &c in &self.px[y * self.w + tx * TILE..y * self.w + ((tx + 1) * TILE).min(self.w)] {
                            m = m.max(c);
                        }
                    }
                    self.tile[t] = m;
                    self.stale[t] = false;
                }
                if near <= self.tile[t] as f64 {
                    return false;
                }
            }
        }
        true
    }
}

trait NextUp {
    fn next_up_f32(self) -> f32;
}

impl NextUp for f32 {
    fn next_up_f32(self) -> f32 {
        if self.is_nan() || self == f32::INFINITY {
            return self;
        }
        let b = self.to_bits();
        let n = if self == 0.0 { 1 } else if self > 0.0 { b + 1 } else { b - 1 };
        f32::from_bits(n)
    }
}

/// A triangle the GPU draws none of (facing away, outside the view).
const OUT: u32 = u32::MAX;
/// A triangle kept whatever is in front (it crosses the near plane, is edge-on, or is an X-ray face).
const KEEP: u32 = u32::MAX - 1;

/// The occlusion buffer's cells (pixels a side): 2 x 2 pixels a cell cost 3% more triangles kept
/// than whole pixels for D1U, and a third less time.
const OCCLUSION_CELL: usize = 2;

/// Triangles (or polygons) worked through between two asks of the caller's time.
const STRIDE: usize = 2048;

/// The occluder side of the kept triangles, worked out once for them (whatever the camera): each
/// triangle's plane when it lies in one of the axes' (which way it faces then follows from the
/// eye's side of it alone), which ones hide what is behind them, and the block faces among those
/// merged into rectangles, plane by plane (pairs of triangles making an axis-aligned unit square
/// at whole coordinates: neighbours in a plane cover the pixels on their shared edge together, as
/// the shader samples at the centroid of the samples a triangle covers, and an opaque face writes
/// every one).
pub struct Occluders {
    /// Each triangle's axis, plane and facing (+ along the axis), when it lies in such a plane.
    planes: Vec<Option<(u8, f32, bool)>>,
    opaque: Vec<bool>,
    /// The rectangles: axis, plane, facing, and their corners' other coordinates (axis + 1:
    /// u0..u1, axis + 2: v0..v1).
    rects: Vec<(u8, i32, bool, [i32; 4])>,
    /// Which triangles are in a rectangle (the others that hide things do it on their own).
    in_rect: Vec<bool>,
}

/// Where a [`Cull`] is: building the occluders (each triangle, then the unit squares, then the
/// rectangles), then the frame's (each triangle, the rectangles on screen, the fill, the test).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Triangles,
    Squares,
    Rectangles,
    Classify,
    Project,
    Fill,
    Test,
    TestExtra,
    Done,
}

/// The kept triangles a camera can see, worked out over as many calls as it takes
/// ([`Cull::step`]: a farm in its terrain's 164,000 triangles took 46 ms natively in one).
pub struct Cull {
    view: View,
    phase: Phase,
    i: usize,
    // the occluders (given, or built here)
    occluders: Option<std::rc::Rc<Occluders>>,
    planes: Vec<Option<(u8, f32, bool)>>,
    opaque: Vec<bool>,
    in_rect: Vec<bool>,
    grids: Vec<((u8, i32, bool), Vec<(i32, i32)>)>,
    grid_of: HashMap<(u8, i32, bool), usize>,
    rects: Vec<(u8, i32, bool, [i32; 4])>,
    // the frame's: each triangle OUT, KEEP or its polygon's index; the triangles hiding things on
    // their own (their polygons'); the rectangles on screen; the depths; the kept triangles
    kind: Vec<u32>,
    polys: Vec<Poly>,
    alone: Vec<u32>,
    occl: Vec<Poly>,
    z: Option<ZBuf>,
    out: Vec<u32>,
    /// The extra triangles' kept ones ([`Cull::step_with`]: the still translucent faces).
    out_extra: Vec<u32>,
}

impl Cull {
    /// For `view`, with the occluders of the kept triangles if already worked out.
    pub fn new(view: View, occluders: Option<std::rc::Rc<Occluders>>) -> Cull {
        let phase = if occluders.is_some() { Phase::Classify } else { Phase::Triangles };
        Cull {
            view,
            phase,
            i: 0,
            occluders,
            planes: Vec::new(),
            opaque: Vec::new(),
            in_rect: Vec::new(),
            grids: Vec::new(),
            grid_of: HashMap::new(),
            rects: Vec::new(),
            kind: Vec::new(),
            polys: Vec::new(),
            alone: Vec::new(),
            occl: Vec::new(),
            z: None,
            out: Vec::new(),
            out_extra: Vec::new(),
        }
    }

    /// The occluders, once worked out (for the next camera on the same kept triangles).
    pub fn occluders(&self) -> Option<std::rc::Rc<Occluders>> {
        self.occluders.clone()
    }

    /// The buffer's size in cells and the frame's in cells (fractional).
    fn grid(&self) -> (usize, usize, f64, f64) {
        let (wp, hp) = (self.view.width.max(1) as f64 / OCCLUSION_CELL as f64, self.view.height.max(1) as f64 / OCCLUSION_CELL as f64);
        (wp.ceil() as usize, hp.ceil() as usize, wp, hp)
    }

    /// A point in the buffer's cells with its depth (None: at or behind the near plane).
    fn screen(&self, q: [f64; 3]) -> Option<[f64; 3]> {
        let (_, _, wp, hp) = self.grid();
        let m = &self.view.vp;
        let mut c = [0.0f64; 4];
        for (i, ci) in c.iter_mut().enumerate() {
            *ci = m[0][i] as f64 * q[0] + m[1][i] as f64 * q[1] + m[2][i] as f64 * q[2] + m[3][i] as f64;
        }
        if c[3] < 1e-6 || c[2] < 0.0 {
            return None;
        }
        Some([(c[0] / c[3] + 1.0) * 0.5 * wp, (1.0 - c[1] / c[3]) * 0.5 * hp, c[2] / c[3]])
    }

    fn in_range(&self, x: f64, z: f64) -> bool {
        let eye_chunk = [(self.view.eye[0] / 16.0).floor(), (self.view.eye[2] / 16.0).floor()];
        ((x as f32 / 16.0).floor() - eye_chunk[0]).abs() <= self.view.chunks && ((z as f32 / 16.0).floor() - eye_chunk[1]).abs() <= self.view.chunks
    }

    /// Work on until done or `out_of_time` (asked every [`STRIDE`] triangles or so): the triangles
    /// of `v` (three vertices each: the same every call) the GPU could draw a sample of at the
    /// view, as an index list in their order (three indices each): what
    /// [`crate::render::Renderer`] draws in place of all of them, the same picture.
    pub fn step(&mut self, v: &[Vertex], opaque: &Opaque, out_of_time: &mut dyn FnMut() -> bool) -> Option<Vec<u32>> {
        self.step_with(v, &[], opaque, out_of_time).map(|(a, _)| a)
    }

    /// [`Cull::step`] with `extra` triangles tested too but hiding nothing (the still translucent
    /// faces: drawn without writing depth, a face behind the opaque ones draws no sample): the
    /// kept of `v` and the kept of `extra` (indices into each, in their order).
    pub fn step_with(&mut self, v: &[Vertex], extra: &[Vertex], opaque: &Opaque, out_of_time: &mut dyn FnMut() -> bool) -> Option<(Vec<u32>, Vec<u32>)> {
        let n = v.len() / 3;
        loop {
            match self.phase {
                Phase::Triangles => {
                    let end = (self.i + STRIDE).min(n);
                    for t in v[self.i * 3..end * 3].chunks_exact(3) {
                        self.opaque.push(opaque.triangle(t));
                        let axis = (0..3).find(|&a| t[1].pos[a] == t[0].pos[a] && t[2].pos[a] == t[0].pos[a]);
                        self.planes.push(axis.and_then(|a| {
                            let (p0, p1, p2) = (t[0].pos, t[1].pos, t[2].pos);
                            let (u, w) = ([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]], [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]]);
                            let nrm = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                            (nrm[a] != 0.0).then_some((a as u8, p0[a], nrm[a] > 0.0))
                        }));
                    }
                    self.i = end;
                    if self.i == n {
                        (self.phase, self.i) = (Phase::Squares, 0);
                        self.in_rect = vec![false; n];
                    }
                }
                Phase::Squares => {
                    let end = (self.i + STRIDE).min(n);
                    let mut ti = self.i;
                    while ti < end {
                        if ti + 1 < n
                            && self.opaque[ti]
                            && self.opaque[ti + 1]
                            && let Some(key) = unit_square(&v[ti * 3..ti * 3 + 6])
                        {
                            let g = *self.grid_of.entry((key.0, key.1, key.2)).or_insert_with(|| {
                                self.grids.push(((key.0, key.1, key.2), Vec::new()));
                                self.grids.len() - 1
                            });
                            self.grids[g].1.push((key.3, key.4));
                            self.in_rect[ti] = true;
                            self.in_rect[ti + 1] = true;
                            ti += 2;
                            continue;
                        }
                        ti += 1;
                    }
                    self.i = ti;
                    if self.i >= n {
                        (self.phase, self.i) = (Phase::Rectangles, 0);
                    }
                }
                Phase::Rectangles => {
                    let mut cells = 0;
                    while self.i < self.grids.len() && cells < STRIDE * 4 {
                        let ((axis, plane, facing), mut g) = std::mem::take(&mut self.grids[self.i]);
                        cells += g.len();
                        for (u0, u1, v0, v1) in rectangles(&mut g) {
                            self.rects.push((axis, plane, facing, [u0, u1, v0, v1]));
                        }
                        self.i += 1;
                    }
                    if self.i == self.grids.len() {
                        let occluders = Occluders { planes: std::mem::take(&mut self.planes), opaque: std::mem::take(&mut self.opaque), rects: std::mem::take(&mut self.rects), in_rect: std::mem::take(&mut self.in_rect) };
                        self.occluders = Some(std::rc::Rc::new(occluders));
                        self.grids = Vec::new();
                        self.grid_of = HashMap::new();
                        (self.phase, self.i) = (Phase::Classify, 0);
                    }
                }
                Phase::Classify => {
                    let oc = self.occluders.clone().unwrap();
                    let (w, h, _, _) = self.grid();
                    let eye = [self.view.eye[0] as f64, self.view.eye[1] as f64, self.view.eye[2] as f64];
                    let pos = |p: [f32; 3]| [p[0] as f64, p[1] as f64, p[2] as f64];
                    let end = (self.i + STRIDE).min(n);
                    for ti in self.i..end {
                        let t = &v[ti * 3..ti * 3 + 3];
                        // (an X-ray face is drawn in front of everything: `render.rs`'s vertex
                        // shader)
                        if t[0].layer & XRAY != 0 {
                            self.kind.push(KEEP);
                            continue;
                        }
                        // (in an axis's plane: facing away when the eye is on its back, clearly;
                        // the GPU's cull agrees, its winding on screen being the plane's side the
                        // eye is on)
                        if let Some((a, p, plus)) = oc.planes[ti] {
                            let d = eye[a as usize] - p as f64;
                            if (plus && d < -1e-3) || (!plus && d > 1e-3) {
                                self.kind.push(OUT);
                                continue;
                            }
                        }
                        let [Some(a), Some(b), Some(c)] = [self.screen(pos(t[0].pos)), self.screen(pos(t[1].pos)), self.screen(pos(t[2].pos))] else {
                            self.kind.push(KEEP);
                            continue;
                        };
                        let s = [a, b, c];
                        // (the screen's y runs down: a front face, counter-clockwise in the GPU's
                        // coordinates, turns clockwise here: a negative area)
                        let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
                        let scale = (s[1][0] - s[0][0]).abs() + (s[2][0] - s[0][0]).abs() + (s[1][1] - s[0][1]).abs() + (s[2][1] - s[0][1]).abs();
                        if area.abs() <= 1e-9 * (1.0 + scale * scale) {
                            self.kind.push(KEEP);
                            continue;
                        }
                        if area > 0.0 || s.iter().all(|q| q[2] > 1.0) {
                            self.kind.push(OUT);
                            continue;
                        }
                        match Poly::new(&s, w, h) {
                            Some(p) if !p.empty() => {
                                if oc.opaque[ti] && !oc.in_rect[ti] && s.iter().all(|q| q[2] <= 1.0) && t.iter().all(|q| self.in_range(q.pos[0] as f64, q.pos[2] as f64)) {
                                    self.alone.push(self.polys.len() as u32);
                                }
                                self.kind.push(self.polys.len() as u32);
                                self.polys.push(p);
                            }
                            Some(_) => self.kind.push(OUT),
                            None => self.kind.push(KEEP),
                        }
                    }
                    self.i = end;
                    if self.i == n {
                        (self.phase, self.i) = (Phase::Project, 0);
                    }
                }
                Phase::Project => {
                    // the rectangles facing the eye, wholly in range and in front of the far plane
                    let oc = self.occluders.clone().unwrap();
                    let (w, h, _, _) = self.grid();
                    let eye = [self.view.eye[0] as f64, self.view.eye[1] as f64, self.view.eye[2] as f64];
                    let end = (self.i + STRIDE).min(oc.rects.len());
                    for &(axis, plane, plus, [u0, u1, v0, v1]) in &oc.rects[self.i..end] {
                        let d = eye[axis as usize] - plane as f64;
                        if (plus && d <= 1e-3) || (!plus && d >= -1e-3) {
                            continue;
                        }
                        let corner = |u: i32, vv: i32| -> [f64; 3] {
                            let mut q = [0.0f64; 3];
                            q[axis as usize] = plane as f64;
                            q[(axis as usize + 1) % 3] = u as f64;
                            q[(axis as usize + 2) % 3] = vv as f64;
                            q
                        };
                        let cs = [corner(u0, v0), corner(u1, v0), corner(u1, v1), corner(u0, v1)];
                        let mut s = [[0.0f64; 3]; 4];
                        let mut ok = true;
                        for (k, q) in cs.iter().enumerate() {
                            match self.screen(*q) {
                                Some(p) if p[2] <= 1.0 && self.in_range(q[0], q[2]) => s[k] = p,
                                _ => ok = false,
                            }
                        }
                        if ok
                            && let Some(p) = Poly::new(&s, w, h)
                            && !p.empty()
                        {
                            self.occl.push(p);
                        }
                    }
                    self.i = end;
                    if self.i == oc.rects.len() {
                        // nearest first: an occluder behind those already in is skipped where they
                        // hide it whole; then those standing alone
                        self.occl.sort_by(|a, b| a.near.total_cmp(&b.near));
                        self.z = Some(ZBuf::new(w, h));
                        (self.phase, self.i) = (Phase::Fill, 0);
                    }
                }
                Phase::Fill => {
                    let total = self.occl.len() + self.alone.len();
                    let end = (self.i + STRIDE / 8).min(total);
                    let z = self.z.as_mut().unwrap();
                    for k in self.i..end {
                        let p = if k < self.occl.len() { &self.occl[k] } else { &self.polys[self.alone[k - self.occl.len()] as usize] };
                        if !z.behind(p.bbox, p.near) {
                            p.occlude(z);
                        }
                    }
                    self.i = end;
                    if self.i == total {
                        self.occl = Vec::new();
                        (self.phase, self.i) = (Phase::Test, 0);
                    }
                }
                Phase::Test => {
                    // each triangle: kept when it might touch a pixel at a depth not surely behind
                    // that pixel's
                    let end = (self.i + STRIDE).min(n);
                    let z = self.z.as_mut().unwrap();
                    for ti in self.i..end {
                        let keep = match self.kind[ti] {
                            OUT => false,
                            KEEP => true,
                            i => self.polys[i as usize].seen(z),
                        };
                        if keep {
                            let b = (ti * 3) as u32;
                            self.out.extend_from_slice(&[b, b + 1, b + 2]);
                        }
                    }
                    self.i = end;
                    if self.i == n {
                        (self.phase, self.i) = (Phase::TestExtra, 0);
                    }
                }
                Phase::TestExtra => {
                    // the extra triangles the same way, each worked out here (none hides anything)
                    let m = extra.len() / 3;
                    let end = (self.i + STRIDE).min(m);
                    let (w, h, _, _) = self.grid();
                    let pos = |p: [f32; 3]| [p[0] as f64, p[1] as f64, p[2] as f64];
                    for ti in self.i..end {
                        let t = &extra[ti * 3..ti * 3 + 3];
                        let keep = if t[0].layer & XRAY != 0 {
                            true
                        } else if let [Some(a), Some(b), Some(c)] = [self.screen(pos(t[0].pos)), self.screen(pos(t[1].pos)), self.screen(pos(t[2].pos))] {
                            let s = [a, b, c];
                            let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
                            let scale = (s[1][0] - s[0][0]).abs() + (s[2][0] - s[0][0]).abs() + (s[1][1] - s[0][1]).abs() + (s[2][1] - s[0][1]).abs();
                            if area.abs() <= 1e-9 * (1.0 + scale * scale) {
                                true
                            } else if area > 0.0 || s.iter().all(|q| q[2] > 1.0) {
                                false
                            } else {
                                match Poly::new(&s, w, h) {
                                    Some(p) if !p.empty() => p.seen(self.z.as_mut().unwrap()),
                                    Some(_) => false,
                                    None => true,
                                }
                            }
                        } else {
                            true
                        };
                        if keep {
                            let b = (ti * 3) as u32;
                            self.out_extra.extend_from_slice(&[b, b + 1, b + 2]);
                        }
                    }
                    self.i = end;
                    if self.i == m {
                        self.phase = Phase::Done;
                    }
                }
                Phase::Done => {
                    (self.kind, self.polys, self.alone, self.z) = (Vec::new(), Vec::new(), Vec::new(), None);
                    return Some((std::mem::take(&mut self.out), std::mem::take(&mut self.out_extra)));
                }
            }
            if self.phase != Phase::Done && out_of_time() {
                return None;
            }
        }
    }
}

/// Two triangles (six vertices) making an axis-aligned unit square at whole coordinates: its
/// axis, plane, facing (+ along the axis), and its corner's two other coordinates (axis + 1,
/// axis + 2, mod 3).
fn unit_square(t: &[Vertex]) -> Option<(u8, i32, bool, i32, i32)> {
    let mut corners = [[0.0f32; 3]; 4];
    let mut nc = 0;
    for p in t {
        if !corners[..nc].contains(&p.pos) {
            if nc == 4 {
                return None;
            }
            corners[nc] = p.pos;
            nc += 1;
        }
    }
    if nc != 4 || corners.iter().flatten().any(|c| c.fract() != 0.0) {
        return None;
    }
    let axis = (0..3).find(|&a| corners.iter().all(|c| c[a] == corners[0][a]))?;
    let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
    let (u0, u1) = (corners.iter().map(|c| c[ua]).fold(f32::MAX, f32::min), corners.iter().map(|c| c[ua]).fold(f32::MIN, f32::max));
    let (v0, v1) = (corners.iter().map(|c| c[va]).fold(f32::MAX, f32::min), corners.iter().map(|c| c[va]).fold(f32::MIN, f32::max));
    if u1 - u0 != 1.0 || v1 - v0 != 1.0 {
        return None;
    }
    // the two triangles must share their diagonal (a square, not two triangles of other squares)
    let tri = |k: usize| [t[k].pos, t[k + 1].pos, t[k + 2].pos];
    let (a, b) = (tri(0), tri(3));
    if a.iter().filter(|p| b.contains(p)).count() != 2 {
        return None;
    }
    // facing: the first triangle's normal along the axis
    let (p0, p1, p2) = (a[0], a[1], a[2]);
    let (u, w) = ([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]], [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]]);
    let nrm = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    Some((axis as u8, corners[0][axis] as i32, nrm[axis] > 0.0, u0 as i32, v0 as i32))
}

/// Unit cells merged into rectangles (u0, u1, v0, v1: exclusive ends), greedily: rows along u,
/// grown along v while the whole row is there.
fn rectangles(cells: &mut [(i32, i32)]) -> Vec<(i32, i32, i32, i32)> {
    if cells.is_empty() {
        return Vec::new();
    }
    // (a grid over the cells' box: 0 none, 1 a cell, 2 one in a rectangle already)
    let (mut ulo, mut uhi, mut vlo, mut vhi) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for &(u, v) in cells.iter() {
        (ulo, uhi, vlo, vhi) = (ulo.min(u), uhi.max(u), vlo.min(v), vhi.max(v));
    }
    let (gw, gh) = ((uhi - ulo + 1) as usize, (vhi - vlo + 1) as usize);
    let mut grid = vec![0u8; gw * gh];
    for &(u, v) in cells.iter() {
        grid[(v - vlo) as usize * gw + (u - ulo) as usize] = 1;
    }
    let mut out = Vec::new();
    for gy in 0..gh {
        for gx in 0..gw {
            if grid[gy * gw + gx] != 1 {
                continue;
            }
            let mut x1 = gx + 1;
            while x1 < gw && grid[gy * gw + x1] == 1 {
                x1 += 1;
            }
            let mut y1 = gy + 1;
            while y1 < gh && (gx..x1).all(|x| grid[y1 * gw + x] == 1) {
                y1 += 1;
            }
            for y in gy..y1 {
                for x in gx..x1 {
                    grid[y * gw + x] = 2;
                }
            }
            out.push((ulo + gx as i32, ulo + x1 as i32, vlo + gy as i32, vlo + y1 as i32));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(path: &str) -> String {
        std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// The culled frames are the very pictures the whole ones are (`Renderer::cull_fixed` draws
    /// only the kept triangles the camera can see; `mctas web framecost --cull --check` on more
    /// designs): a mob farm and a farm in its terrain, at a phone's and a desktop's size, pixel for
    /// pixel, with most of the terrain's triangles left out. Skipped without a GPU adapter
    /// (lavapipe on a GPU-less box).
    #[test]
    fn culled_frames_are_the_whole_frames() {
        let mut atlas = Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, w, h) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, w, h, &idx, &defs, &models).unwrap();
        let Ok(mut off) = crate::render::headless::Offscreen::new(&atlas) else {
            eprintln!("no GPU adapter: skipped");
            return;
        };
        for (design, terrain) in [("designs/circuits/mobfarm/archive/iron-day1.txt", false), ("designs/farms/iron/day-1/d1u-underground.txt", true)] {
            let text = read(design);
            for (w, h, dpr) in [(798, 960, 2.0), (1232, 480, 1.0)] {
                let c = crate::tools::frame_cost(&mut off, &atlas, &text, w, h, dpr, 0, 2, 1, true, true, None, None, (None, None), None, None, None, None, None).unwrap();
                assert_eq!(c.differing, 0, "{design} at {w}x{h}: {} pixels differ culled", c.differing);
                assert_eq!(c.culls, 1, "{design} at {w}x{h}: worked out once for a camera at rest");
                if terrain {
                    assert!(c.kept * 2 < c.tested, "{design} at {w}x{h}: {} of {} triangles kept", c.kept, c.tested);
                }
            }
        }
    }

    /// How close the culled set is to the triangles really seen (`--nocapture`): D1U's live view
    /// at phone size, the triangles winning a sample of a z-buffer at `MCTAS_SS` x the pixels
    /// (point samples at the sub-pixels' centres, every face taken as opaque: a lower bound of
    /// what any exact cull keeps), against [`Cull`]'s.
    #[test]
    #[ignore]
    fn print_visible_bound() {
        let mut atlas = Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, gw, gh) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, gw, gh, &idx, &defs, &models).unwrap();
        let text = read(&std::env::var("MCTAS_DESIGN").unwrap_or_else(|_| "designs/farms/iron/day-1/d1u-underground.txt".into()));
        let (width, height, dpr) = (798u32, 960u32, 2.0f32);
        let mut p = crate::player::Player::new_web(&text, width as f32 / height as f32).unwrap();
        p.frame_live(&atlas, width as f32 / dpr, height as f32 / dpr);
        let (fixed, _, _) = p.triangles_split_within(&atlas, &mut || false).unwrap();
        let v = fixed.map(|(_, v)| v).unwrap_or_default();
        let m = p.camera.view_proj(width as f32 / height as f32);
        let view = View { vp: m, eye: p.camera.eye(), chunks: 64.0, width, height };
        let culled = Cull::new(view, None).step(&v, &Opaque::of(&atlas), &mut || false).unwrap().len() / 3;
        for ss in [1usize, 2, 3] {
            let (w, h) = (width as usize * ss, height as usize * ss);
            let mut zbuf = vec![f64::MAX; w * h];
            let mut ids = vec![u32::MAX; w * h];
            for (ti, t) in v.chunks(3).enumerate() {
                let c: Vec<[f64; 4]> = t
                    .iter()
                    .map(|x| {
                        let q = x.pos;
                        let mut o = [0.0f64; 4];
                        for (i, oi) in o.iter_mut().enumerate() {
                            *oi = m[0][i] as f64 * q[0] as f64 + m[1][i] as f64 * q[1] as f64 + m[2][i] as f64 * q[2] as f64 + m[3][i] as f64;
                        }
                        o
                    })
                    .collect();
                if c.iter().any(|q| q[3] <= 1e-3) {
                    continue;
                }
                let s: Vec<[f64; 3]> = c.iter().map(|q| [(q[0] / q[3] + 1.0) * 0.5 * w as f64, (1.0 - q[1] / q[3]) * 0.5 * h as f64, q[2] / q[3]]).collect();
                let area = (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
                if area >= 0.0 {
                    continue;
                }
                let (x0, x1) = (s.iter().map(|q| q[0]).fold(f64::MAX, f64::min).floor().max(0.0) as usize, s.iter().map(|q| q[0]).fold(f64::MIN, f64::max).ceil().min(w as f64) as usize);
                let (y0, y1) = (s.iter().map(|q| q[1]).fold(f64::MAX, f64::min).floor().max(0.0) as usize, s.iter().map(|q| q[1]).fold(f64::MIN, f64::max).ceil().min(h as f64) as usize);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                        let e = |a: [f64; 3], b: [f64; 3]| (b[0] - a[0]) * (py - a[1]) - (b[1] - a[1]) * (px - a[0]);
                        let (e0, e1, e2) = (e(s[1], s[2]), e(s[2], s[0]), e(s[0], s[1]));
                        if e0 > 0.0 || e1 > 0.0 || e2 > 0.0 {
                            continue;
                        }
                        let z = (e0 * s[0][2] + e1 * s[1][2] + e2 * s[2][2]) / (e0 + e1 + e2);
                        let k = y * w + x;
                        if z < zbuf[k] {
                            zbuf[k] = z;
                            ids[k] = ti as u32;
                        }
                    }
                }
            }
            let seen: std::collections::HashSet<u32> = ids.iter().copied().filter(|&i| i != u32::MAX).collect();
            eprintln!("{ss}x{ss} samples a pixel: {} triangles seen; the cull keeps {culled} of {}", seen.len(), v.len() / 3);
            if ss == 3 {
                // the kept triangles never seen: by the plane they lie in (axis and facing), and
                // how far behind the nearest surface at their centre they are
                let kept: Vec<u32> = Cull::new(view, None).step(&v, &Opaque::of(&atlas), &mut || false).unwrap().chunks(3).map(|t| t[0] / 3).collect();
                let mut by: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                for &ti in &kept {
                    if seen.contains(&ti) {
                        continue;
                    }
                    let t = &v[ti as usize * 3..ti as usize * 3 + 3];
                    let axis = (0..3).find(|&a| t[1].pos[a] == t[0].pos[a] && t[2].pos[a] == t[0].pos[a]);
                    let op = Opaque::of(&atlas).triangle(t);
                    *by.entry(format!("axis {axis:?} opaque {op}")).or_default() += 1;
                }
                eprintln!("kept, never seen: {by:?}");
                // why the opaque ones are kept: at the first pixel cell that keeps each, no
                // occluder there (INF), or one not surely in front
                let mut job = Cull::new(view, None);
                while job.phase != Phase::Test {
                    let _ = job.step(&v, &Opaque::of(&atlas), &mut || true);
                }
                let (mut inf, mut near, mut slack) = (0usize, 0usize, Vec::new());
                let z = job.z.as_mut().unwrap();
                for &ti in &kept {
                    let t = &v[ti as usize * 3..ti as usize * 3 + 3];
                    if seen.contains(&ti) || !Opaque::of(&atlas).triangle(t) {
                        continue;
                    }
                    let k = job.kind[ti as usize];
                    if k == OUT || k == KEEP {
                        continue;
                    }
                    let p = &job.polys[k as usize];
                    let sl = p.dz * (1.0 + 2.0 * EDGE_MARGIN) + DEPTH_MARGIN;
                    'cells: for y in p.bbox[1]..p.bbox[3] {
                        let (x0, x1) = p.span(y, false);
                        for x in x0..x1 {
                            let d = p.depth[0] * (x as f64 + 0.5) + p.depth[1] * (y as f64 + 0.5) + p.depth[2] - sl;
                            let c = z.px[y * z.w + x];
                            if d <= c as f64 {
                                if c == f32::INFINITY {
                                    inf += 1;
                                } else {
                                    near += 1;
                                    slack.push((sl, c as f64 - (d + sl)));
                                }
                                break 'cells;
                            }
                        }
                    }
                }
                slack.sort_by(|a, b| a.1.total_cmp(&b.1));
                let q = |f: f64| slack.get(((slack.len() as f64 - 1.0) * f) as usize).copied().unwrap_or_default();
                eprintln!("opaque kept never seen: {inf} at a cell no occluder covers, {near} at a covered one (the occluder's farthest minus its centre depth, and its slack, at the 10/50/90%: {:?} {:?} {:?})", q(0.1), q(0.5), q(0.9));
            }
        }
    }

    /// How many of a farm's still translucent triangles (its water) the cull would keep, were they
    /// tested like the solid ones (`--nocapture`; `MCTAS_DESIGN`, default cross-4-underground).
    #[test]
    #[ignore]
    fn print_translucent_kept() {
        let mut atlas = Atlas::build();
        let (png, idx, defs, models) = crate::tools::vanilla_assets().unwrap();
        atlas.apply_vanilla_png(&png, &idx).unwrap();
        let (rgba, gw, gh) = crate::tools::decode_png(&png).unwrap();
        atlas.set_game(&rgba, gw, gh, &idx, &defs, &models).unwrap();
        let text = read(&std::env::var("MCTAS_DESIGN").unwrap_or_else(|_| "designs/farms/iron/middle/cross-4-underground.txt".into()));
        for (width, height, dpr) in [(798u32, 960u32, 2.0f32), (1232, 480, 1.0)] {
            let mut p = crate::player::Player::new_web(&text, width as f32 / height as f32).unwrap();
            p.frame_live(&atlas, width as f32 / dpr, height as f32 / dpr);
            p.triangles_split_within(&atlas, &mut || false).unwrap();
            let m = p.blocks_mesh().unwrap();
            let solid = m.solid.clone();
            let tr = m.sorted_translucent(p.camera.eye());
            let mut all = solid.clone();
            all.extend_from_slice(&tr);
            let view = View { vp: p.camera.view_proj(width as f32 / height as f32), eye: p.camera.eye(), chunks: 64.0, width, height };
            let kept = Cull::new(view, None).step(&all, &Opaque::of(&atlas), &mut || false).unwrap();
            let first_tr = (solid.len() / 3) as u32;
            let kept_tr = kept.chunks(3).filter(|t| t[0] / 3 >= first_tr).count();
            eprintln!("{width}x{height}: translucent triangles kept {kept_tr} of {}; solid kept {} of {}", tr.len() / 3, kept.len() / 3 - kept_tr, solid.len() / 3);
        }
    }
}
