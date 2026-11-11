//! Native tools behind `mctas web shot` / `mctas web check`: render a design to PNG offscreen
//! (the same meshes and shaders as the browser), and the clock-period comparison.

use crate::atlas::Atlas;
use crate::render::Frame;
use crate::render::headless::Offscreen;
use crate::scene;
use crate::session::{Options, STILL_HORIZON, Session};

/// The `data-still` attribute of a design's thumbnail ([`Session::still_attr`]): the still frame
/// the viewer would search for itself, found at build time (the site puts it on every
/// `data-mcview="thumb"` element; the browser then runs one pass to it). None when the design
/// does not load in the viewer (omit the attribute: the viewer shows its own error).
pub fn still_attr(text: &str) -> Option<String> {
    // (a card's: void, as the browser's cards)
    let mut s = Session::new(text, &Options { terrain: false, ..Options::default() }).ok()?;
    Some(s.still_attr(STILL_HORIZON))
}

/// A design's still frame in the viewer as plain blocks (`mctas web snapshot`): every block the
/// viewer draws there (a door at rest; a circuit settled, with its judge's lamps, rig and chests;
/// a clock at its most active moment, with its judge's lamp), as (cell, state), and the design's
/// own entities as placed (a resting circuit's item frames). The real game and the viewer then
/// picture the very same blocks (`clock shot --still`, `web compare --still`): what differs is
/// the rendering. Err when redsim does not run it, or a block is in motion there.
pub fn snapshot(text: &str) -> Result<(Vec<(crate::design::Pos, String)>, Vec<crate::design::Entity>, String), String> {
    let mut s = Session::new(text, &Options::default())?;
    if let Some(why) = &s.unsimulated {
        return Err(format!("not simulated: {why}"));
    }
    let what = if s.mode.is_door() { format!("rest, tick {}", s.tick) } else { format!("still tick {}", s.go_to_still(STILL_HORIZON)) };
    let mut blocks = Vec::new();
    for b in s.render_blocks(0.0) {
        if b.offset != [0.0; 3] {
            return Err(format!("{what}: a block in motion at {:?}", b.cell));
        }
        blocks.push((b.cell, b.state));
    }
    Ok((blocks, s.design.entities.clone(), what))
}

/// Knobs of [`shot`].
#[derive(Clone, Debug)]
pub struct ShotOptions {
    pub width: u32,
    pub height: u32,
    /// Run this many ticks after placement (clocks) / after rest (doors).
    pub tick: Option<u32>,
    /// Lever flips before the picture (doors: 1 = open), each run until idle.
    pub flips: u32,
    /// Camera yaw / pitch in degrees (default: the real-game pictures' direction).
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    /// As the game pictures it: without the viewer's own marks (the lime outlines on the inputs).
    pub picture: bool,
    /// The ground's reach around the build (default: the viewer's).
    pub floor_margin: Option<f32>,
    /// As a live viewer of this size frames it (its default camera, with its outlines), not a
    /// thumbnail.
    pub live: bool,
    /// The device pixel ratio of a live viewer (its CSS size: the picture's size / this; a
    /// phone: 2).
    pub dpr: f32,
    /// The design as written (a still frame from [`snapshot`]): `tick` ticks after placement and
    /// nothing found (no output searched: no marker).
    pub as_written: bool,
    /// A picture's whole window at the game's field of view ([`picture_frames`]: the pictures'
    /// encoder crops it), not the pictures' centre crop.
    pub full_frame: bool,
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions { width: 800, height: 450, tick: None, flips: 0, yaw: None, pitch: None, picture: false, floor_margin: None, live: false, dpr: 1.0, as_written: false, full_frame: false }
    }
}

/// Render a design: RGBA pixels and a one-line description of what was shown.
pub fn shot(off: &mut Offscreen, atlas: &Atlas, text: &str, o: &ShotOptions) -> Result<(Vec<u8>, String), String> {
    shot_frames(off, atlas, text, o, false).map(|(rgba, _, note)| (rgba, note))
}

/// [`shot`], and with `floor_only` the same frame with nothing on the floor too.
fn shot_frames(off: &mut Offscreen, atlas: &Atlas, text: &str, o: &ShotOptions, floor_only: bool) -> Result<(Vec<u8>, Option<Vec<u8>>, String), String> {
    let mut s = Session::new(text, &Options::default())?;
    let what = match o.tick {
        Some(t) => {
            if !o.as_written {
                s.find_auto_output(20_000);
            }
            s.run(t);
            format!("tick {}", s.tick)
        }
        None if s.mode.is_door() => "rest".to_string(),
        None => format!("still tick {}", s.go_to_still(20_000)),
    };
    for _ in 0..o.flips {
        if let crate::session::Mode::Door { lever, .. } = s.mode {
            s.toggle(lever);
            let mut n = 0;
            while !s.idle() && n < 400 {
                s.step();
                n += 1;
            }
        }
    }
    let aspect = o.width as f32 / o.height as f32;
    let mut look = if o.picture { scene::default_look(&s) } else { scene::viewer_look(&s) };
    // a picture (`web compare`) draws no outlines; the viewer's thumbnail (the browser's: nothing
    // to press) only a BUD switch's TARGET (`scene::thumbnail_look`); both frame what they draw; a
    // live view keeps its outlines
    if o.picture {
        look.outlines.clear();
        look.input_spans.clear();
        look.port_outlines.clear();
    } else if !o.live {
        scene::thumbnail_look(&s, &mut look);
    }
    if o.picture {
        look.ghost.clear();
    }
    // as the real-game pictures show it (`web compare`): their camera and their grass reach;
    // else the viewer's (a door: the side its lever is seen from)
    look.floor_margin = o.floor_margin.or(if o.picture { Some(scene::PICTURE_GROUND_REACH) } else { None });
    let mut cam = if o.picture {
        let mut cam = scene::picture_camera(&s, &look, aspect);
        if o.full_frame {
            cam.vfov = crate::camera::VFOV;
        }
        cam
    } else if o.live {
        // a live viewer's, as it frames the design on load (tick 0, before any run: the camera
        // stays put while it plays; its window clear of the bar, the badges and, on a phone, the
        // Record pill, at its CSS size)
        let dpr = o.dpr.max(0.5);
        let mut p = crate::player::Player::new(text, aspect)?;
        p.frame_live(atlas, o.width as f32 / dpr, o.height as f32 / dpr);
        p.camera
    } else {
        // the viewer's thumbnail camera (a door: the side its lever is seen from)
        scene::thumb_camera(atlas, &s, &look, aspect)
    };
    if o.yaw.is_some() || o.pitch.is_some() {
        if let Some(y) = o.yaw {
            cam.yaw = y.to_radians();
        }
        if let Some(p) = o.pitch {
            cam.pitch = p.to_radians();
        }
        let (lo, hi) = scene::frame_bounds(&s, &look);
        cam.fit(lo, hi, aspect, 0.85);
    }
    let mesh = scene::build(atlas, &s, 0.0, &look);
    let (solid, translucent) = scene::triangles(&mesh, &cam);
    // the viewer's sky under the horizon (no ground there), the pictures' own for `web compare`
    off.r.void_below = !o.picture;
    let rgba = off.render(&Frame { solid: &solid, translucent: &translucent, camera: cam, width: o.width, height: o.height })?;
    // the same frame with nothing on the floor (`web compare`: what differs from it is the build)
    let bg = if floor_only {
        let g = scene::ground_mesh(atlas, &s, &look);
        Some(off.render(&Frame { solid: &g.solid, translucent: &[], camera: cam, width: o.width, height: o.height })?)
    } else {
        None
    };
    let note = match &s.unsimulated {
        Some(why) => format!("{what} (not simulated: {why})"),
        None => format!("{what}, output {:?}, marker {:?}", s.outputs.first(), look.marker),
    };
    Ok((rgba, bg, note))
}

/// A real-game picture next to ours: (side-by-side RGBA, its width, height, scores). Scores: the
/// mean absolute RGB difference per pixel (0-255) and the same on a 32x18 grid of block means
/// (the composition: framing, ground, sky, where the build is).
pub struct Comparison {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_diff: f64,
    pub coarse_diff: f64,
    /// The pixel diff of the bottom eighth (usually grass only: the textures' noise floor) and of
    /// the top eighth (the horizon, the sky band).
    pub bottom_diff: f64,
    pub top_diff: f64,
    /// The difference over the build alone (the pixels where ours differs from our floor alone,
    /// and 2 px around; 3x3 means): how the blocks themselves are drawn, the grass's noise left
    /// out; and how many pixels that is.
    pub build_diff: f64,
    pub build_px: usize,
    /// Where ours sits against the real picture ([`edge_offset`]: pixels, x right, y down) and
    /// how much that shift would gain.
    pub edge_offset: (f32, f32),
    pub edge_gain: f64,
    pub note: String,
}

/// The real-game pictures' window (`clock shot`'s client: 1920 x 1080, cropped and scaled by
/// `clock::shot::encode_picture`).
pub const PICTURE_FRAME: (u32, u32) = (1920, 1080);

/// Our frame of a design as the real game frames its picture (`clock shot`): the whole window
/// at the game's field of view, its camera and grass; with the same frame with nothing on the
/// floor (the build's pixels are where they differ), and a note. `web compare` passes both
/// through the pictures' own encoder (`clock::shot::encode_picture`: the same crop, resampling
/// and compression as the real picture; resampled here, the build sat a third of a pixel off).
/// `still`: a still frame from [`snapshot`], as written (tick 0, 1 with entities: the game runs
/// that tick with the camera there, so its client gets them).
pub fn picture_frames(off: &mut Offscreen, atlas: &Atlas, text: &str, floor_margin: Option<f32>, still: bool) -> Result<(Vec<u8>, Vec<u8>, String), String> {
    let tick = still.then(|| if text.lines().any(|l| l.starts_with("entity:")) { 1 } else { 0 });
    let (w, h) = PICTURE_FRAME;
    let (frame, floor, note) = shot_frames(off, atlas, text, &ShotOptions { width: w, height: h, picture: true, full_frame: true, floor_margin, tick, as_written: still, ..Default::default() }, true)?;
    let floor = floor.unwrap_or_else(|| frame.clone());
    Ok((frame, floor, note))
}

/// A JPEG's pixels.
pub fn decode_jpeg(jpg: &[u8]) -> Result<(Vec<[u8; 3]>, u32, u32), String> {
    let mut dec = jpeg_decoder::Decoder::new(jpg);
    let px = dec.decode().map_err(|e| format!("jpeg: {e}"))?;
    let info = dec.info().ok_or("jpeg: no info")?;
    let (w, h) = (info.width as u32, info.height as u32);
    let rgb: Vec<[u8; 3]> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        jpeg_decoder::PixelFormat::L8 => px.iter().map(|v| [*v, *v, *v]).collect(),
        f => return Err(format!("jpeg: unsupported pixel format {f:?}")),
    };
    Ok((rgb, w, h))
}

/// The real picture against ours (both `w` x `h`, ours encoded like it, and our floor-only frame
/// for the build's pixels): three panels (the real picture, ours, where they differ) and the
/// differences.
pub fn compare_pictures(real: &[[u8; 3]], ours: &[[u8; 3]], floor_rgb: &[[u8; 3]], w: u32, h: u32, note: String) -> Comparison {
    let edge = edge_offset(real, ours, w, h);
    let mine: Vec<u8> = ours.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    let floor: Vec<u8> = floor_rgb.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    // three panels: the real picture, ours, and where they differ (the difference of 3x3 means,
    // red over the real picture dimmed)
    let mut out = vec![0u8; (3 * w * h * 4) as usize];
    let mut diff = 0f64;
    let (mut bottom, mut top) = (0f64, 0f64);
    let band = (h / 8).max(1) as usize;
    let (wu, hu) = (w as usize, h as usize);
    for y in 0..hu {
        for x in 0..wu {
            let r = real[y * wu + x];
            let m = &mine[(y * wu + x) * 4..(y * wu + x) * 4 + 3];
            let o = (y * 3 * wu + x) * 4;
            out[o..o + 4].copy_from_slice(&[r[0], r[1], r[2], 255]);
            let o = (y * 3 * wu + wu + x) * 4;
            out[o..o + 4].copy_from_slice(&[m[0], m[1], m[2], 255]);
            let d = (0..3).map(|k| (r[k] as f64 - m[k] as f64).abs()).sum::<f64>() / 3.0;
            diff += d;
            if y >= hu - band {
                bottom += d;
            }
            if y < band {
                top += d;
            }
        }
    }
    // the build's pixels: where ours differs from our floor alone (and 2 px around)
    let mut build = vec![false; wu * hu];
    for y in 0..hu {
        for x in 0..wu {
            let i = (y * wu + x) * 4;
            if (0..3).any(|k| (mine[i + k] as i32 - floor[i + k] as i32).abs() > 12) {
                for yy in y.saturating_sub(2)..(y + 3).min(hu) {
                    for xx in x.saturating_sub(2)..(x + 3).min(wu) {
                        build[yy * wu + xx] = true;
                    }
                }
            }
        }
    }
    // the difference of 3x3 means (the textures' sub-pixel shifts averaged out), over the build
    let mean3 = |get: &dyn Fn(usize, usize) -> [f64; 3], x: usize, y: usize| -> [f64; 3] {
        let mut s = [0f64; 3];
        let mut n = 0f64;
        for yy in y.saturating_sub(1)..(y + 2).min(hu) {
            for xx in x.saturating_sub(1)..(x + 2).min(wu) {
                let c = get(xx, yy);
                for k in 0..3 {
                    s[k] += c[k];
                }
                n += 1.0;
            }
        }
        s.map(|v| v / n)
    };
    let get_r = |x: usize, y: usize| -> [f64; 3] { real[y * wu + x].map(|v| v as f64) };
    let get_m = |x: usize, y: usize| -> [f64; 3] {
        let i = (y * wu + x) * 4;
        [mine[i] as f64, mine[i + 1] as f64, mine[i + 2] as f64]
    };
    let (mut bsum, mut bn) = (0f64, 0usize);
    for y in 0..hu {
        for x in 0..wu {
            let (a, b) = (mean3(&get_r, x, y), mean3(&get_m, x, y));
            let d = (0..3).map(|k| (a[k] - b[k]).abs()).sum::<f64>() / 3.0;
            if build[y * wu + x] {
                bsum += d;
                bn += 1;
            }
            let r = real[y * wu + x];
            let dim = |v: u8| (v as f64 * 0.35) as u8;
            let heat = (d * 5.0).min(255.0) as u8;
            let o = (y * 3 * wu + 2 * wu + x) * 4;
            out[o..o + 4].copy_from_slice(&[dim(r[0]).saturating_add(heat), dim(r[1]), dim(r[2]), 255]);
        }
    }
    let (gw, gh) = (32usize, 18usize);
    let mut coarse = 0f64;
    for gy in 0..gh {
        for gx in 0..gw {
            let (x0, x1) = (gx * w as usize / gw, (gx + 1) * w as usize / gw);
            let (y0, y1) = (gy * h as usize / gh, (gy + 1) * h as usize / gh);
            let mut a = [0f64; 3];
            let mut b = [0f64; 3];
            let n = ((x1 - x0) * (y1 - y0)).max(1) as f64;
            for y in y0..y1 {
                for x in x0..x1 {
                    let r = real[y * w as usize + x];
                    for k in 0..3 {
                        a[k] += r[k] as f64 / n;
                        b[k] += mine[(y * w as usize + x) * 4 + k] as f64 / n;
                    }
                }
            }
            coarse += (0..3).map(|k| (a[k] - b[k]).abs()).sum::<f64>() / 3.0;
        }
    }
    Comparison {
        rgba: out,
        width: 3 * w,
        height: h,
        pixel_diff: diff / (w * h) as f64,
        coarse_diff: coarse / (gw * gh) as f64,
        bottom_diff: bottom / (band * w as usize) as f64,
        top_diff: top / (band * w as usize) as f64,
        build_diff: if bn > 0 { bsum / bn as f64 } else { 0.0 },
        build_px: bn,
        edge_offset: edge.0,
        edge_gain: edge.1,
        note,
    }
}

/// A camera variant tried by [`fit`]: zoom (tan of the half field of view divided by it), pitch
/// and yaw offsets (degrees), the target moved up (blocks), the distance scaled.
#[derive(Clone, Copy, Debug)]
pub struct CamTweak {
    pub zoom: f32,
    pub dpitch: f32,
    pub dyaw: f32,
    pub dy: f32,
    pub dist: f32,
}

impl CamTweak {
    pub fn apply(&self, mut c: crate::camera::Camera) -> crate::camera::Camera {
        let t = (c.vfov.to_radians() / 2.0).tan() / self.zoom;
        c.vfov = (2.0 * t.atan()).to_degrees();
        c.pitch += self.dpitch.to_radians();
        c.yaw += self.dyaw.to_radians();
        c.target[1] += self.dy;
        c.dist *= self.dist;
        c
    }
}

/// Search the camera tweak that best overlays the viewer's frames on the real-game pictures
/// (mean RGB difference at `w` x `h`): the designs' texts and their pictures (JPEG). Returns the
/// tweaks tried, best first, with their mean difference.
pub fn fit(off: &mut Offscreen, atlas: &Atlas, cases: &[(String, Vec<u8>)], w: u32, h: u32, tweaks: &[CamTweak]) -> Result<Vec<(f64, CamTweak)>, String> {
    struct Case {
        mesh: crate::mesh::Mesh,
        cam: crate::camera::Camera,
        real: Vec<u8>,
    }
    let aspect = w as f32 / h as f32;
    let mut prepared = Vec::new();
    for (text, jpg) in cases {
        let (px, iw, ih) = decode_image(jpg)?;
        // box-downscale the picture to w x h
        let mut real = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x * iw / w, ((x + 1) * iw / w).max(x * iw / w + 1));
                let (y0, y1) = (y * ih / h, ((y + 1) * ih / h).max(y * ih / h + 1));
                let mut s = [0u32; 3];
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        for k in 0..3 {
                            s[k] += px[((yy * iw + xx) * 4) as usize + k] as u32;
                        }
                    }
                }
                let n = (x1 - x0) * (y1 - y0);
                for k in 0..3 {
                    real[((y * w + x) * 3) as usize + k] = (s[k] / n) as u8;
                }
            }
        }
        let mut s = Session::new(text, &Options::default())?;
        if !s.mode.is_door() {
            s.go_to_still(20_000);
        }
        let mut look = scene::default_look(&s);
        look.floor_margin = Some(scene::PICTURE_GROUND_REACH);
        let cam = scene::picture_camera(&s, &look, aspect);
        prepared.push(Case { mesh: scene::build(atlas, &s, 0.0, &look), cam, real });
    }
    let mut out = Vec::new();
    for t in tweaks {
        let mut total = 0f64;
        for c in &prepared {
            let cam = t.apply(c.cam);
            let (solid, translucent) = scene::triangles(&c.mesh, &cam);
            let mine = off.render(&Frame { solid: &solid, translucent: &translucent, camera: cam, width: w, height: h })?;
            let mut d = 0f64;
            for i in 0..(w * h) as usize {
                for k in 0..3 {
                    d += (mine[i * 4 + k] as f64 - c.real[i * 3 + k] as f64).abs();
                }
            }
            total += d / (w * h * 3) as f64;
        }
        out.push((total / prepared.len().max(1) as f64, *t));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

/// Resize RGBA with a separable Lanczos-3 filter (ffmpeg's `flags=lanczos` downscale).
pub fn lanczos_resize(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    fn weights(src_n: u32, dst_n: u32) -> Vec<(usize, Vec<f32>)> {
        let scale = src_n as f32 / dst_n as f32;
        let support = 3.0 * scale.max(1.0);
        let lanczos = |x: f32| -> f32 {
            if x.abs() < 1e-6 {
                1.0
            } else if x.abs() >= 3.0 {
                0.0
            } else {
                let px = std::f32::consts::PI * x;
                3.0 * px.sin() * (px / 3.0).sin() / (px * px)
            }
        };
        (0..dst_n)
            .map(|i| {
                let center = (i as f32 + 0.5) * scale;
                let a = ((center - support).floor() as i64).max(0) as usize;
                let b = ((center + support).ceil() as i64).min(src_n as i64) as usize;
                let mut ws: Vec<f32> = (a..b).map(|j| lanczos((j as f32 + 0.5 - center) / scale.max(1.0))).collect();
                let s: f32 = ws.iter().sum();
                if s.abs() > 1e-6 {
                    for v in &mut ws {
                        *v /= s;
                    }
                }
                (a, ws)
            })
            .collect()
    }
    let wx = weights(sw, dw);
    let wy = weights(sh, dh);
    // horizontal pass
    let mut tmp = vec![0f32; (dw * sh * 4) as usize];
    for y in 0..sh as usize {
        for (x, (a, ws)) in wx.iter().enumerate() {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let s = (y * sw as usize + a + k) * 4;
                for c in 0..4 {
                    acc[c] += src[s + c] as f32 * w;
                }
            }
            let d = (y * dw as usize + x) * 4;
            tmp[d..d + 4].copy_from_slice(&acc);
        }
    }
    // vertical pass
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for (y, (a, ws)) in wy.iter().enumerate() {
        for x in 0..dw as usize {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let s = ((a + k) * dw as usize + x) * 4;
                for c in 0..4 {
                    acc[c] += tmp[s + c] * w;
                }
            }
            let d = (y * dw as usize + x) * 4;
            for c in 0..4 {
                out[d + c] = acc[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// A grid of frames over time (the animation the browser plays): a door from its lever flip, a
/// clock from its still moment; `step` game ticks between frames (fractions show moving blocks
/// mid-way). Returns the grid's RGBA and its size.
pub fn strip(off: &mut Offscreen, atlas: &Atlas, text: &str, cols: u32, rows: u32, step: f32, w: u32, h: u32) -> Result<(Vec<u8>, u32, u32), String> {
    // the browser's player, played at 20 gt/s: the same camera (inputs in view, a flying
    // machine followed), the same interpolation
    let aspect = w as f32 / h as f32;
    let mut p = crate::player::Player::new(text, aspect)?;
    p.show_inputs(atlas, aspect, scene::live_window(h as f32));
    if p.session.mode.is_door() {
        p.flip();
    } else {
        p.session.go_to_still(20_000);
        p.invalidate();
    }
    p.playing = true;
    let (gw, gh) = (cols * w, rows * h);
    let mut grid = vec![0u8; (gw * gh * 4) as usize];
    for i in 0..cols * rows {
        let (solid, translucent) = p.triangles(atlas);
        let rgba = off.render(&Frame { solid: &solid, translucent: &translucent, camera: p.camera, width: w, height: h })?;
        let (ox, oy) = ((i % cols) * w, (i / cols) * h);
        for y in 0..h {
            let src = &rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
            let o = (((oy + y) * gw + ox) * 4) as usize;
            grid[o..o + (w * 4) as usize].copy_from_slice(src);
        }
        let mut left = step / p.tps;
        while left > 1e-6 {
            let d = left.min(0.25);
            p.update(d);
            left -= d;
        }
    }
    Ok((grid, gw, gh))
}

/// Decode a PNG or a JPEG to RGBA8.
pub fn decode_image(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut dec = jpeg_decoder::Decoder::new(bytes);
        let px = dec.decode().map_err(|e| format!("jpeg: {e}"))?;
        let info = dec.info().ok_or("jpeg: no info")?;
        let rgba: Vec<u8> = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
            f => return Err(format!("jpeg: unsupported pixel format {f:?}")),
        };
        return Ok((rgba, info.width as u32, info.height as u32));
    }
    decode_png(bytes)
}

pub use crate::atlas::vanilla::decode_png;

/// The vendored game textures: `MCTAS_TEXTURES_DIR` if set, else the repo's
/// `web/textures/<version>` beside this crate's checkout. No download: the mcmeta
/// mirror's versioned branches are gone, and the tools must work offline.
fn textures_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("MCTAS_TEXTURES_DIR")
        && !d.is_empty()
    {
        return std::path::PathBuf::from(d);
    }
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../")).join(crate::atlas::vanilla::DIR)
}

fn read_texture(name: &str) -> Result<Vec<u8>, String> {
    let p = textures_dir().join(name);
    std::fs::read(&p).map_err(|e| format!("{}: {e} (the vendored textures live in {}, override with MCTAS_TEXTURES_DIR)", p.display(), crate::atlas::vanilla::DIR))
}

/// The game's block atlas (vendored in the repo) and its block definitions and models
/// (`atlas::vanilla`): the PNG bytes, the atlas index, the definitions and the models.
/// The browser fetches the same files from the site.
pub fn vanilla_atlas() -> Result<(Vec<u8>, String), String> {
    let (png, index, _, _) = vanilla_assets()?;
    Ok((png, index))
}

/// [`vanilla_atlas`] with the block definitions and models.
pub fn vanilla_assets() -> Result<(Vec<u8>, String, String, String), String> {
    use crate::atlas::vanilla::*;
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    let png = read_texture(ATLAS)?;
    let index = text(read_texture(INDEX)?)?;
    let defs = text(read_texture(DEFS)?)?;
    let models = text(read_texture(MODELS)?)?;
    Ok((png, index, defs, models))
}

/// The game's entity and item atlases (PNG bytes, index), vendored like [`vanilla_assets`].
pub fn vanilla_entity_assets() -> Result<((Vec<u8>, String), (Vec<u8>, String)), String> {
    use crate::atlas::vanilla::*;
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    Ok(((read_texture(ENTITY_ATLAS)?, text(read_texture(ENTITY_INDEX)?)?), (read_texture(ITEM_ATLAS)?, text(read_texture(ITEM_INDEX)?)?)))
}

/// The game's particle sprites ([`crate::atlas::vanilla::particle_names`]: name, RGBA, size),
/// vendored like [`vanilla_assets`].
pub fn vanilla_particle_assets() -> Result<Vec<(String, Vec<u8>, u32, u32)>, String> {
    let mut out = Vec::new();
    for name in crate::atlas::vanilla::particle_names() {
        let rel = format!("{}/{name}.png", crate::atlas::vanilla::PARTICLE_DIR);
        let (rgba, w, h) = decode_png(&read_texture(&rel)?)?;
        out.push((name, rgba, w, h));
    }
    Ok(out)
}

/// The colors of a texture: its mean and a k-means palette of `k` colors with their share of the
/// opaque pixels (most common first), each color optionally multiplied by `tint` (grass, leaves).
pub fn palette(rgba: &[u8], k: usize, tint: Option<[u8; 3]>) -> ([u8; 3], Vec<([u8; 3], f32)>) {
    let t = tint.map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]).unwrap_or([1.0; 3]);
    let px: Vec<[f32; 3]> = rgba.chunks(4).filter(|c| c[3] >= 128).map(|c| [c[0] as f32 * t[0], c[1] as f32 * t[1], c[2] as f32 * t[2]]).collect();
    if px.is_empty() {
        return ([0; 3], Vec::new());
    }
    let n = px.len() as f32;
    let mean = (0..3).map(|i| px.iter().map(|p| p[i]).sum::<f32>() / n).collect::<Vec<_>>();
    let lum = |p: &[f32; 3]| p[0] * 0.3 + p[1] * 0.59 + p[2] * 0.11;
    let mut sorted = px.clone();
    sorted.sort_by(|a, b| lum(a).total_cmp(&lum(b)));
    let k = k.max(1).min(px.len());
    // seeds at luminance quantiles, then Lloyd iterations
    let mut centers: Vec<[f32; 3]> = (0..k).map(|i| sorted[(i * 2 + 1) * sorted.len() / (2 * k)]).collect();
    let mut assign = vec![0usize; px.len()];
    for _ in 0..20 {
        for (j, p) in px.iter().enumerate() {
            assign[j] = (0..k).min_by(|a, b| {
                let d = |c: &[f32; 3]| (0..3).map(|i| (c[i] - p[i]).powi(2)).sum::<f32>();
                d(&centers[*a]).total_cmp(&d(&centers[*b]))
            }).unwrap();
        }
        for (c, center) in centers.iter_mut().enumerate() {
            let members: Vec<&[f32; 3]> = px.iter().zip(&assign).filter(|(_, a)| **a == c).map(|(p, _)| p).collect();
            if !members.is_empty() {
                for i in 0..3 {
                    center[i] = members.iter().map(|p| p[i]).sum::<f32>() / members.len() as f32;
                }
            }
        }
    }
    let mut out: Vec<([u8; 3], f32)> = (0..k).map(|c| (centers[c].map(|v| v.round().clamp(0.0, 255.0) as u8), assign.iter().filter(|a| **a == c).count() as f32 / n)).filter(|(_, f)| *f > 0.0).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    ([mean[0].round() as u8, mean[1].round() as u8, mean[2].round() as u8], out)
}

/// Textures in a grid, each scaled up `scale` times (nearest), on a checkerboard (transparency).
pub fn sheet(paths: &[std::path::PathBuf], scale: u32, cols: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let cell = 16 * scale + 4;
    let rows = (paths.len() as u32).div_ceil(cols.max(1));
    let (w, h) = (cols * cell, rows.max(1) * cell);
    let mut out = vec![40u8; (w * h * 4) as usize];
    for (i, p) in paths.iter().enumerate() {
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let (px, tw, th) = decode_png(&bytes).map_err(|e| format!("{}: {e}", p.display()))?;
        let (ox, oy) = ((i as u32 % cols) * cell + 2, (i as u32 / cols) * cell + 2);
        for y in 0..16 * scale {
            for x in 0..16 * scale {
                let (sx, sy) = (x * tw.min(16) / (16 * scale), y * th.min(16).max(1) / (16 * scale));
                let s = ((sy * tw + sx) * 4) as usize;
                let a = px[s + 3] as u32;
                let bg = if ((x / 8) + (y / 8)) % 2 == 0 { 90 } else { 120 };
                let o = (((oy + y) * w + ox + x) * 4) as usize;
                for k in 0..3 {
                    out[o + k] = ((px[s + k] as u32 * a + bg * (255 - a)) / 255) as u8;
                }
                out[o + 3] = 255;
            }
        }
    }
    Ok((out, w, h))
}

/// The shipped wasm (`mcview_bg.wasm`) run natively by an interpreter, to check that the browser's
/// simulation is the native one: its `mcview_clock_period` export on design texts. Every import
/// (the browser's API) traps if called; the period detector needs none.
pub struct WasmProbe {
    store: wasmi::Store<()>,
    memory: wasmi::Memory,
    alloc: wasmi::TypedFunc<u32, u32>,
    period: wasmi::TypedFunc<(u32, u32, u32), u64>,
    fly_end: wasmi::TypedFunc<(u32, u32), u32>,
    digest: wasmi::TypedFunc<(u32, u32, u32), u64>,
    /// `mcview_mob_digest` (a bundle from before the mob farms ran has none).
    mob: Option<wasmi::TypedFunc<(u32, u32, u32), u64>>,
}

impl WasmProbe {
    pub fn load(bytes: &[u8]) -> Result<WasmProbe, String> {
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, bytes).map_err(|e| format!("wasm: {e}"))?;
        let mut store = wasmi::Store::new(&engine, ());
        let mut linker = wasmi::Linker::<()>::new(&engine);
        for imp in module.imports() {
            if let wasmi::ExternType::Func(ft) = imp.ty() {
                let label = format!("{}.{}", imp.module(), imp.name());
                linker.func_new(imp.module(), imp.name(), ft.clone(), move |_, _, _| Err(wasmi::Error::new(format!("the check called the browser import {label}")))).map_err(|e| format!("{e}"))?;
            }
        }
        let instance = linker.instantiate_and_start(&mut store, &module).map_err(|e| format!("instantiate: {e}"))?;
        let memory = instance.get_memory(&store, "memory").ok_or("no exported memory")?;
        let alloc = instance.get_typed_func::<u32, u32>(&store, "mcview_alloc").map_err(|e| format!("mcview_alloc: {e}"))?;
        let period = instance.get_typed_func::<(u32, u32, u32), u64>(&store, "mcview_clock_period").map_err(|e| format!("mcview_clock_period: {e}"))?;
        let fly_end = instance.get_typed_func::<(u32, u32), u32>(&store, "mcview_fly_end").map_err(|e| format!("mcview_fly_end: {e}"))?;
        let digest = instance.get_typed_func::<(u32, u32, u32), u64>(&store, "mcview_frames_digest").map_err(|e| format!("mcview_frames_digest: {e}"))?;
        let mob = instance.get_typed_func::<(u32, u32, u32), u64>(&store, "mcview_mob_digest").ok();
        Ok(WasmProbe { store, memory, alloc, period, fly_end, digest, mob })
    }

    /// A mob farm's run in the wasm over `ticks` judged ticks (`mcview::mobfarm::chain`; 0: not
    /// simulated there). Err when the bundle has no such export.
    pub fn mob_digest(&mut self, text: &str, ticks: u32) -> Result<u64, String> {
        let (ptr, len) = self.put(text)?;
        let f = self.mob.ok_or("the bundle has no mcview_mob_digest (rebuild it: mctas web bundle)")?;
        f.call(&mut self.store, (ptr, len, ticks)).map_err(|e| format!("mob_digest: {e}"))
    }

    /// Copy a text into the wasm's memory: (pointer, length).
    fn put(&mut self, text: &str) -> Result<(u32, u32), String> {
        let len = text.len() as u32;
        let ptr = self.alloc.call(&mut self.store, len).map_err(|e| format!("alloc: {e}"))?;
        self.memory.write(&mut self.store, ptr as usize, text.as_bytes()).map_err(|e| format!("write: {e}"))?;
        Ok((ptr, len))
    }

    /// A flying machine's end tick in the wasm (None: not simulated).
    pub fn fly_end(&mut self, text: &str) -> Result<Option<u32>, String> {
        let (ptr, len) = self.put(text)?;
        let t = self.fly_end.call(&mut self.store, (ptr, len)).map_err(|e| format!("fly_end: {e}"))?;
        Ok((t != u32::MAX).then_some(t))
    }

    /// The wasm's frames digest over ticks 0..=`ticks`.
    pub fn frames_digest(&mut self, text: &str, ticks: u32) -> Result<u64, String> {
        let (ptr, len) = self.put(text)?;
        self.digest.call(&mut self.store, (ptr, len, ticks)).map_err(|e| format!("digest: {e}"))
    }

    /// (state warmup, state period, warmup, period), or None when the wasm finds no clock.
    pub fn period(&mut self, text: &str, horizon: u32) -> Result<Option<(u32, u32, u32, u32)>, String> {
        let len = text.len() as u32;
        let ptr = self.alloc.call(&mut self.store, len).map_err(|e| format!("alloc: {e}"))?;
        self.memory.write(&mut self.store, ptr as usize, text.as_bytes()).map_err(|e| format!("write: {e}"))?;
        let r = self.period.call(&mut self.store, (ptr, len, horizon)).map_err(|e| format!("period: {e}"))?;
        if r == u64::MAX {
            return Ok(None);
        }
        let f = |k: u32| ((r >> (16 * k)) & 0xffff) as u32;
        Ok(Some((f(0), f(1), f(2), f(3))))
    }
}

/// How far ours sits from the real picture, in pixels (x right, y down, 1/8 px steps within
/// +-1): the shift of ours (sampled bilinearly) that best matches the real picture on its edges
/// (strong gradients, below the sky band), and how much better than none that is (a fraction).
/// Two pictures made alike come out within 1/8 px (`web compare`; resampling ours apart from the
/// real one's encoder had put it a third of a pixel off).
pub fn edge_offset(real: &[[u8; 3]], ours: &[[u8; 3]], w: u32, h: u32) -> ((f32, f32), f64) {
    let (wu, hu) = (w as usize, h as usize);
    if wu < 16 || hu < 16 {
        return ((0.0, 0.0), 0.0);
    }
    let at = |img: &[[u8; 3]], x: usize, y: usize| -> [f32; 3] { img[y * wu + x].map(|v| v as f32) };
    let bilinear = |x: f32, y: f32| -> [f32; 3] {
        let (x0, y0) = (x.floor().clamp(0.0, (wu - 2) as f32), y.floor().clamp(0.0, (hu - 2) as f32));
        let (fx, fy) = (x - x0, y - y0);
        let (xi, yi) = (x0 as usize, y0 as usize);
        let (a, b, c, d) = (at(ours, xi, yi), at(ours, xi + 1, yi), at(ours, xi, yi + 1), at(ours, xi + 1, yi + 1));
        std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    };
    // the real picture's edges
    let mut edges: Vec<(usize, usize, [f32; 3])> = Vec::new();
    for y in hu / 6..hu - 4 {
        for x in 4..wu - 4 {
            let (r, g, gy) = (at(real, x, y), at(real, x + 1, y), at(real, x, y + 1));
            if (0..3).map(|k| (r[k] - g[k]).abs() + (r[k] - gy[k]).abs()).sum::<f32>() >= 60.0 {
                edges.push((x, y, r));
            }
        }
    }
    if edges.is_empty() {
        return ((0.0, 0.0), 0.0);
    }
    let cost = |dx: f32, dy: f32| -> f64 {
        edges.iter().map(|(x, y, r)| {
            let m = bilinear(*x as f32 + dx, *y as f32 + dy);
            (0..3).map(|k| (r[k] - m[k]).abs() as f64).sum::<f64>()
        }).sum()
    };
    let base = cost(0.0, 0.0);
    let mut best = (base, 0.0f32, 0.0f32);
    for sy in -8..=8 {
        for sx in -8..=8 {
            if sx == 0 && sy == 0 {
                continue;
            }
            let (dx, dy) = (sx as f32 * 0.125, sy as f32 * 0.125);
            let c = cost(dx, dy);
            // a tie (a direction with nothing to tell): the smaller shift
            if c < best.0 * (1.0 - 1e-9) || (c <= best.0 * (1.0 + 1e-9) && dx.abs() + dy.abs() < best.1.abs() + best.2.abs()) {
                best = (c, dx, dy);
            }
        }
    }
    // ours sampled at +d matches the real picture best: its content sits d from the real one's
    ((best.1, best.2), if base > 0.0 { 1.0 - best.0 / base } else { 0.0 })
}

/// What a live viewer's frames cost natively ([`frame_cost`]): per frame, the mesh (the
/// player's triangles, as the browser asks them), the draw (submitted and waited for: the
/// GPU's work, lavapipe on a GPU-less box, like a software WebGL), and the vertices drawn.
#[derive(Clone, Debug, Default)]
pub struct FrameCost {
    pub frames: u32,
    pub mesh_ms: f64,
    pub draw_ms: f64,
    /// The vertices kept on the GPU (a mob farm's blocks), drawn each frame without an upload.
    pub fixed: usize,
    /// The vertices uploaded each frame: solid and translucent (a frame's mean).
    pub solid: usize,
    pub translucent: usize,
    /// The blocks' mesh rebuilt this many times over the frames (the session's blocks changed).
    pub rebuilds: u64,
    /// With the occlusion cull ([`crate::render::Renderer::cull_fixed`]): how many times it was
    /// worked out, its mean time, and the last one's triangles kept of those tested.
    pub culls: u32,
    pub cull_ms: f64,
    pub kept: usize,
    pub tested: usize,
    /// With the check: the most pixels any frame drawn culled differs by from the same frame
    /// drawn whole (0: the same picture).
    pub differing: usize,
    /// With a budget ([`frame_cost`]'s `budget`, the browser's per-frame time for staged work): the
    /// blocks' meshes finished over the frames, and the frames that worked on one.
    pub built: u32,
    pub staged_frames: u32,
    /// With the cull: the updates in place whose last cull was carried over at once
    /// (`Renderer::carry_cull`, checked with `check` too), and the last one's triangles kept.
    pub carried: u32,
    pub carried_kept: usize,
    /// With `check`: the most pixels a frame drawn whole differs by from the same frame with the
    /// sky drawn first, under everything (`Renderer::sky_first`: the order of before).
    pub sky_differing: usize,
}

/// A live viewer of `text` at `width x height` device pixels (`dpr`: its CSS size is that / dpr),
/// framed as it loads ([`crate::player::Player::frame_live`]), played `warm` ticks, then
/// `frames` frames of `ticks` ticks each, each frame's triangles drawn with the browser's renderer
/// and waited for ([`FrameCost`]); `cull`: the kept triangles the camera can see only, as the
/// browser draws them; `check`: each frame also drawn whole and the two pictures compared;
/// `budget`: each frame's staged mesh work stops after that many ms, as in the browser (None: a
/// changed mesh is built whole in its frame); `png_dir`: each frame's picture also written there
/// (`frame_00001.png`, ...: a film of the live view); `view`: the camera's yaw and pitch in
/// degrees, framed as a page's `data-yaw` / `data-pitch` frame it (None: the live camera); `zoom`:
/// the camera that many times closer.
#[allow(clippy::too_many_arguments)]
pub fn frame_cost(off: &mut Offscreen, atlas: &Atlas, text: &str, width: u32, height: u32, dpr: f32, warm: u32, frames: u32, ticks: u32, cull: bool, check: bool, budget: Option<f64>, png_dir: Option<&std::path::Path>, view: (Option<f32>, Option<f32>), zoom: Option<f32>, look_at: Option<[f32; 3]>, follow: Option<redsim::entity::mob::drive::Who>, brightness: Option<f32>, fit: Option<([f32; 3], [f32; 3])>) -> Result<FrameCost, String> {
    let mut p = crate::player::Player::new_web(text, width as f32 / height as f32)?;
    p.frame_live(atlas, width as f32 / dpr.max(0.5), height as f32 / dpr.max(0.5));
    if view.0.is_some() || view.1.is_some() {
        // (as `live::finish_load` frames a page's `data-yaw` / `data-pitch`)
        if let Some(y) = view.0 {
            p.camera.yaw = y.to_radians();
        }
        if let Some(pt) = view.1 {
            p.camera.pitch = pt.to_radians();
        }
        let (lo, hi) = scene::frame_bounds(&p.session, &p.look);
        p.camera.fit(lo, hi, width as f32 / height as f32, 0.85);
        p.home = p.camera;
    }
    if let Some((lo, hi)) = fit {
        // (a film's subject box, its cells: the camera aimed at its centre and fitted to it at its
        // yaw and pitch, as the view fits a design's bounds; the loop of a long design, not its
        // far wires; `Camera::fit` keeps the target, so the centre first)
        let hi = [hi[0] + 1.0, hi[1] + 1.0, hi[2] + 1.0];
        p.camera.target = std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0);
        p.camera.fit(lo, hi, width as f32 / height as f32, 0.85);
        p.home = p.camera;
    }
    if let Some(z) = zoom.filter(|z| *z > 0.0) {
        // (closer by that factor: the distance divided)
        p.camera.dist /= z;
        p.home = p.camera;
    }
    if let Some(t) = look_at {
        // (a film's subject: the camera aimed there, its framing kept)
        p.camera.target = [t[0] + 0.5, t[1] + 0.5, t[2] + 0.5];
        p.home = p.camera;
    }
    // (`follow`'s entity, by its id: found as the drive names it once its lines are spawned, at the
    // session's start, before the warm ticks: a napper leaves its cart for the bed later)
    let followed: Option<u32> = follow.and_then(|who| {
        let dv = p.session.mob.as_ref().and_then(|m| m.drive.as_ref())?;
        let i = redsim::entity::mob::drive::index_of(&p.session.world, &dv.lines, who)?;
        Some(p.session.world.entities.list[i].id)
    });
    for _ in 0..warm {
        p.session.step();
    }
    let tex = off.r.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("framecost"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    off.r.void_below = true;
    let opaque = crate::occlude::Opaque::of(atlas);
    let mut c = FrameCost { frames, ..Default::default() };
    let version = p.session.blocks_version;
    // (one frame first: the meshes built and the fixed vertices uploaded, not counted)
    for k in 0..=frames {
        for _ in 0..ticks {
            p.session.step();
        }
        // (a film's subject that moves: the camera aimed at that entity of the part's bench drive
        // each frame, eN or eN.pK as the drive names them, by its id: a napper leaving its cart
        // for the bed is no longer the cart's passenger; its framing kept)
        if let Some(e) = followed.and_then(|id| p.session.world.entities.list.iter().find(|e| e.id == id && !e.removed)) {
            let o = p.session.view_offset();
            p.camera.target = [e.pos.x as f32 - o[0] as f32, e.pos.y as f32 + 0.9 - o[1] as f32, e.pos.z as f32 - o[2] as f32];
        }
        let t0 = std::time::Instant::now();
        let mut oot = || budget.is_some_and(|b| k > 0 && t0.elapsed().as_secs_f64() * 1000.0 > b);
        let before = p.fixed_key();
        // (the blocks' translucent faces kept on the GPU and culled too, as the browser does)
        p.split_blend = cull;
        let Some((fixed, solid, translucent)) = p.triangles_split_within(atlas, &mut oot) else { return Err("no triangles".into()) };
        let blend = p.fixed_blend();
        if k > 0 {
            c.staged_frames += p.staged_now() as u32;
            c.built += (p.fixed_key() != before) as u32;
        }
        let t1 = std::time::Instant::now();
        off.r.env = p.daylight();
        // (a film's Brightness option: the game's slider, 0.5 its default)
        if let Some(b) = brightness {
            let mut e = off.r.env.unwrap_or_else(crate::daylight::Env::noon);
            e.brightness = b;
            off.r.env = Some(e);
        }
        off.r.set_fixed(fixed.as_ref().map(|(k, v)| (*k, v.as_slice())));
        off.r.set_fixed_blend(blend.as_ref().map(|(k, v)| (*k, v.as_slice())));
        // an update in place: the last cull carried over at once (`Renderer::carry_cull`), its
        // frame checked against the whole one too before the exact cull replaces it
        if cull
            && let Some((key, v)) = &fixed
            && let Some((ok, nk, d)) = p.fixed_delta()
            && nk == *key
            && let Some((kept, _)) = off.r.carry_cull((ok, nk, &d), v, blend.as_ref().map(|(k, v)| (*k, v.as_slice())), &p.camera, width, height)
        {
            c.carried += 1;
            c.carried_kept = kept;
            if check {
                let frame = Frame { solid: &solid, translucent: &translucent, camera: p.camera, width, height };
                let carried = off.render(&frame)?;
                let saved = (off.r.fixed_visible.take(), off.r.fixed_blend_visible.take());
                let whole = off.render(&frame)?;
                (off.r.fixed_visible, off.r.fixed_blend_visible) = saved;
                let differing = carried.chunks(4).zip(whole.chunks(4)).filter(|(a, b)| a != b).count();
                c.differing = c.differing.max(differing);
            }
        }
        if cull && let Some((key, v)) = &fixed {
            let tc = std::time::Instant::now();
            if let Some((kept, tested)) = off.r.cull_fixed((*key, v), blend.as_ref().map(|(k, v)| (*k, v.as_slice())), &opaque, &p.camera, width, height, &mut || false) {
                c.culls += 1;
                c.cull_ms += tc.elapsed().as_secs_f64() * 1000.0;
                (c.kept, c.tested) = (kept, tested);
            }
        }
        let t1 = if cull { std::time::Instant::now() } else { t1 };
        let frame = Frame { solid: &solid, translucent: &translucent, camera: p.camera, width, height };
        off.r.draw(&view, &frame);
        let _ = off.r.device.poll(wgpu::PollType::wait_indefinitely());
        let t2 = std::time::Instant::now();
        if let Some(dir) = png_dir
            && k > 0
        {
            let rgba = off.render(&frame)?;
            crate::render::headless::write_png(&dir.join(format!("frame_{k:05}.png")), width, height, &rgba)?;
        }
        if check && k > 0 {
            let culled = off.render(&frame)?;
            let saved = (off.r.fixed_visible.take(), off.r.fixed_blend_visible.take());
            let whole = off.render(&frame)?;
            // (the sky drawn first, under everything, as before: the same pixels)
            off.r.sky_first = true;
            let sky_first = off.render(&frame)?;
            off.r.sky_first = false;
            (off.r.fixed_visible, off.r.fixed_blend_visible) = saved;
            let differing = culled.chunks(4).zip(whole.chunks(4)).filter(|(a, b)| a != b).count();
            c.differing = c.differing.max(differing);
            let sky = sky_first.chunks(4).zip(whole.chunks(4)).filter(|(a, b)| a != b).count();
            c.sky_differing = c.sky_differing.max(sky);
        }
        if k > 0 {
            c.mesh_ms += (t1 - t0).as_secs_f64() * 1000.0;
            c.draw_ms += (t2 - t1).as_secs_f64() * 1000.0;
            c.fixed = fixed.as_ref().map(|(_, v)| v.len()).unwrap_or(0);
            c.solid += solid.len();
            c.translucent += translucent.len() + blend.as_ref().map(|(_, v)| v.len()).unwrap_or(0);
        }
    }
    let n = frames.max(1) as f64;
    c.rebuilds = p.session.blocks_version - version;
    c.mesh_ms /= n;
    c.draw_ms /= n;
    c.cull_ms /= c.culls.max(1) as f64;
    c.solid = (c.solid as f64 / n) as usize;
    c.translucent = (c.translucent as f64 / n) as usize;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture of soft stripes both ways; ours the same moved half a pixel right: the edge
    /// offset finds +0.5, 0 (the third of a pixel `web compare` had: ours resampled apart).
    #[test]
    fn edge_offset_finds_half_a_pixel() {
        let (w, h) = (96u32, 64u32);
        let at = |x: f32, y: f32| -> [u8; 3] {
            let v = (128.0 + 60.0 * (x * 0.45).sin() + 60.0 * (y * 0.37).sin()) as u8;
            [v, v, v]
        };
        let real: Vec<[u8; 3]> = (0..h).flat_map(|y| (0..w).map(move |x| at(x as f32, y as f32))).collect();
        let ours: Vec<[u8; 3]> = (0..h).flat_map(|y| (0..w).map(move |x| at(x as f32 - 0.5, y as f32))).collect();
        let ((dx, dy), gain) = edge_offset(&real, &ours, w, h);
        assert!((dx - 0.5).abs() < 0.07 && dy.abs() < 0.07 && gain > 0.5, "{dx} {dy} {gain}");
        let ((dx, dy), _) = edge_offset(&real, &real, w, h);
        assert_eq!((dx, dy), (0.0, 0.0));
    }
}
