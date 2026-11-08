//! A session as a picture: its blocks (moving ones mid-way), the output marker (orange stained
//! glass, the real-game pictures' marker), outlines on the inputs, the design's entities, a TNT
//! duper's ground (real blocks); the pictures' grass ground (`web compare` only); and the default
//! camera framing it like `mctas clock shot`.

use crate::atlas::Atlas;
use crate::camera::{CAM_DIR, Camera, VFOV};
use crate::design::Pos;
use crate::mesh::{self, Builder, EntityDraw, Item, Mesh, Vertex};
use crate::session::{Session, bp};
use redsim::Block;
use std::collections::HashSet;

/// How far the viewer's grass reaches around the build: past the browser viewer's render
/// distance (24 chunks, 384 blocks), so its fogged end sits at the horizon and no edge shows.
pub const GROUND_REACH: f32 = 420.0;
/// The real-game pictures' grass reach around a clock (`web compare`).
pub const PICTURE_GROUND_REACH: f32 = 56.0;

/// What to draw besides the blocks.
#[derive(Clone, Debug, Default)]
pub struct Look {
    /// The orange glass marker's cell (next to the output: [`marker_cell`]).
    pub marker: Option<Pos>,
    /// The marker glows while the output is on (the live viewer; still pictures keep the
    /// real-game pictures' plain orange glass).
    pub lit_marker: bool,
    /// Lime outlines (clickable inputs).
    pub outlines: Vec<Pos>,
    /// A driven input's lime box (R7): its cell and the judge's devices that drive it (the
    /// repeater, its support and the switch), one box (low, high corner cells): the judge's
    /// equipment reads as part of the input it drives.
    pub input_spans: Vec<(Pos, Pos)>,
    /// A circuit's other ports, outlined in their tag's colour: OUT orange, a shulker design's
    /// ITEMS gold and BOXES lilac chests, a BUD switch's TARGET cyan.
    pub port_outlines: Vec<(Pos, [u8; 3])>,
    /// The ground's top (None: no ground).
    pub ground: Option<f32>,
    /// How far the ground reaches around the build (None: 110 around a door, 56 otherwise).
    pub floor_margin: Option<f32>,
    /// Cells drawn see-through (redsim's blocks there, ghosted): the judge's test rig of a driven
    /// input (R7: its repeater, the repeater's support and the switch), not the build.
    pub ghost: Vec<Pos>,
    /// A flying machine's track (the live viewer): its whole path's box (low, high corners),
    /// the travel axis and the axis normal to the track's plane ([`Builder::track`]).
    pub track: Option<(Pos, Pos, usize, usize)>,
}

/// Where the real-game pictures put the orange marker (`clock/src/shot.rs::marker_cell`): the
/// output cell when it is air; else the air cell the output component powers (an observer's back,
/// a repeater's or comparator's front, above a standing torch); else its air neighbor most visible
/// from the camera.
pub fn marker_cell(s: &Session, out: Pos) -> Option<Pos> {
    let air = |p: Pos| -> bool {
        if s.simulated() {
            matches!(s.world.get(bp(p)), Block::Air)
        } else {
            !s.design.blocks.contains_key(&p)
        }
    };
    if air(out) {
        return Some(out);
    }
    let state = if s.simulated() { s.world.get(bp(out)).to_string() } else { s.design.blocks.get(&out).cloned().unwrap_or_default() };
    let st = crate::models::St::parse(&state);
    let step = |f: &str| -> Option<Pos> {
        Some(match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            "down" => [0, -1, 0],
            _ => return None,
        })
    };
    let preferred: Option<Pos> = match st.name {
        "observer" | "repeater" | "comparator" => st.get("facing").and_then(step).map(|d| [-d[0], -d[1], -d[2]]),
        "redstone_torch" => Some([0, 1, 0]),
        _ => None,
    };
    let mut cands: Vec<Pos> = vec![[0, 0, 1], [0, 1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, -1, 0]];
    let dot = |d: &Pos| d[0] as f32 * CAM_DIR[0] + d[1] as f32 * CAM_DIR[1] + d[2] as f32 * CAM_DIR[2];
    cands.sort_by(|a, b| dot(b).total_cmp(&dot(a)));
    if let Some(d) = preferred {
        cands.retain(|c| *c != d);
        cands.insert(0, d);
    }
    cands.into_iter().map(|d| [out[0] + d[0], out[1] + d[1], out[2] + d[2]]).find(|p| air(*p))
}

/// The default look of a session: the marker next to its first output, outlines on its inputs
/// (a door's lever), grass under its lowest block.
pub fn default_look(s: &Session) -> Look {
    // a circuit output holding the judge's lamp shows itself (lit when on), and a shulker
    // design's output holds the judge's chest: no marker there
    let shulker = crate::circuit::is_shulker(&s.design);
    let marker = s.outputs.first().filter(|o| !s.consumers.contains(o) && !shulker).and_then(|o| marker_cell(s, *o));
    let mut outlines: Vec<Pos> = Vec::new();
    // a design shown as built has nothing to press: no outlines (a recording plays by itself)
    if s.simulated() {
        for p in &s.inputs {
            if !outlines.contains(p) {
                outlines.push(*p);
            }
        }
    }
    // a circuit's other ports (shown as built too: nothing to press, but where they are)
    let hex = |c: &str| -> [u8; 3] {
        let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0xffffff);
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    // (a display face's lamps carry no outline: the lit lamps are the digit, and 13 red boxes
    // hid it; the coordinator 2026-10-22)
    let face = crate::circuit::display_face(&s.design).is_some();
    let port_outlines: Vec<(Pos, [u8; 3])> = crate::circuit::port_tags(&s.design).into_iter().filter(|(l, p, _)| !outlines.contains(p) && !(face && l.starts_with("OUT"))).map(|(_, p, c)| (p, hex(c))).collect();
    // a driven input (R7): one lime box around its cell and the judge's devices driving it, the
    // devices ghosted in the viewer (critique: the switch is the test rig, not the build)
    let mut input_spans: Vec<(Pos, Pos)> = Vec::new();
    let mut ghost: Vec<Pos> = Vec::new();
    if s.simulated() {
        for ci in &s.circuit_inputs {
            if let crate::circuit::InputHow::Driven { dir, .. } = ci.how {
                // the input, its repeater and the switch in a row (the repeater's stone support
                // under it stays plain)
                let (r, sup, sw) = crate::circuit::driver_cells(ci.pos, dir);
                let cells = [ci.pos, r, sw];
                let lo = std::array::from_fn(|k| cells.iter().map(|c| c[k]).min().unwrap_or(0));
                let hi = std::array::from_fn(|k| cells.iter().map(|c| c[k]).max().unwrap_or(0));
                input_spans.push((lo, hi));
                outlines.retain(|p| *p != ci.pos);
                ghost.extend([r, sup, sw]);
            }
        }
    }
    // a bedrock breaker: the roof cells beside each bedrock target drawn see-through (a cutaway),
    // so the target shows from any side, and so does its disappearance
    let targets: Vec<Pos> = s.design.ports.iter().filter(|(n, _)| crate::circuit::is_named(n, "bedrock")).map(|(_, (p, _))| *p).collect();
    for t in &targets {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let c = [t[0] + d[0], t[1], t[2] + d[2]];
            if !targets.contains(&c) && !ghost.contains(&c) && s.design.blocks.get(&c).is_some_and(|b| b.starts_with("minecraft:bedrock")) {
                ghost.push(c);
            }
        }
    }
    let (lo, _) = s.path_box();
    // the pictures' floor: `clock shot` fills grass at the box's lowest y - 2, its top one block
    // under the lowest cell
    Look { marker, lit_marker: false, outlines, input_spans, port_outlines, ground: Some(lo[1] as f32 - 1.0), floor_margin: None, ghost, track: None }
}

/// An OUT box's outline colour (the OUT tag's), and while its output is powered.
pub const OUT_COLOUR: [u8; 3] = [255, 176, 74];
pub const OUT_POWERED: [u8; 3] = [255, 59, 48];
/// A flying machine's track ([`Look::track`]): a pale grey, readable on the sky on either side of
/// the horizon.
pub const TRACK_COLOUR: [u8; 3] = [226, 230, 238];

/// The side tiles of a TNT duper's ground layers (`Floor::LAYERS`), top first.
pub const FLOOR_SIDES: [&str; 4] = ["grass_side", "dirt", "dirt", "bedrock"];

/// The viewer's look (live viewers, thumbnails): [`default_look`], with a clock's output in the
/// circuits' OUT box (vjeux: red while powered) instead of the real-game pictures' orange glass
/// marker (kept for `web compare`).
pub fn viewer_look(s: &Session) -> Look {
    let mut look = default_look(s);
    // no orange glass anywhere in the viewer: an output has its OUT box (a circuit's from its
    // port tags, a clock's added here)
    look.marker = None;
    // no ground but real blocks (vjeux 2026-10-20 17:53Z: "Please don't put a fake ground if the
    // tnt goes through it"): the judges' worlds are void under a design, so the viewer draws no
    // surface there (the sky goes on below the horizon); a TNT duper stands on its judge's
    // ground, real blocks of its world (`Session::floor`, drawn by `build`)
    look.ground = None;
    if !s.design.meta.contains_key("circuit")
        && !s.mode.is_door()
        && let Some(o) = s.outputs.first()
    {
        if !look.port_outlines.iter().any(|(p, _)| p == o) {
            look.port_outlines.push((*o, OUT_COLOUR));
        }
    }
    // a flying machine's track: the camera follows it over the void, and it seemed parked (QA
    // at phone size: fly-horizontal's pistons pumping, the view never moving); a ladder of thin
    // bars under its whole path (a tie at every block) slides by as it flies. An annotation like
    // the outlines, not a ground: nothing stands or lands on it
    if s.corridor.is_some()
        && s.design.meta.get("circuit").is_none_or(|k| k.trim_start().starts_with("flying-machine"))
        && let Some(v) = s.design.meta.get("travel").and_then(|t| crate::design::parse_travel(t))
        && let Some(axis) = (0..3).find(|&i| v[i] != 0)
    {
        let (lo, hi) = s.path_box();
        // under the path; a machine flying up or down: behind it (the pictures' camera looks
        // from the south, toward -z)
        look.track = Some((lo, hi, axis, if axis == 1 { 2 } else { 1 }));
    }
    look
}

/// Which part of a frame [`build_part`] makes: everything, or (a mob farm's view: its blocks change
/// seldom, its mobs every tick) the blocks with the outlines and ground, or the entities alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    All,
    Blocks,
    Entities,
}

/// The frame of a session at `partial` (0..1) after its current tick.
pub fn build(atlas: &Atlas, s: &Session, partial: f32, look: &Look) -> Mesh {
    build_part(atlas, s, partial, look, Part::All)
}

/// [`build`]'s blocks, outlines and ground, or its entities, or both: the two parts together are
/// the whole frame.
pub fn build_part(atlas: &Atlas, s: &Session, partial: f32, look: &Look, part: Part) -> Mesh {
    if part == Part::Entities {
        let mut b = Builder::new(atlas);
        b.ground = look.ground.map(|y| y.floor() as i32);
        entities_into(&mut b, s, partial);
        return b.mesh;
    }
    let mut st = Staged::new(s, partial, look, part);
    loop {
        if let Some(m) = st.step(atlas, s, partial, look, &mut || false) {
            return m;
        }
    }
}

/// [`build_part`] over several calls ([`Staged::step`]), each within its caller's time: a big
/// design's still took 45-60 ms in one task in a software GL, its first frame over 100 ms with the
/// draw (the coordinator: no main-thread task over ~50 ms). The blocks are the session's when it
/// starts; the mesh is the very one [`build_part`] makes (the same faces in the same order: it is
/// [`build_part`]'s own path).
pub struct Staged {
    part: Part,
    blocks: Vec<crate::session::RBlock>,
    /// A TNT duper's ground cells drawn as blocks (not by its intact faces) and the cells around
    /// them that cull and shade like blocks.
    skip: HashSet<Pos>,
    extra_occ: HashSet<Pos>,
    /// The builder between calls, once its first pass ran.
    builder: Option<mesh::BuilderParts>,
    /// The next block to draw.
    next: usize,
}

impl Staged {
    /// The blocks to draw now (moving ones at `partial`) and a TNT duper's ground cells.
    pub fn new(s: &Session, partial: f32, _look: &Look, part: Part) -> Staged {
        let mut blocks = s.render_blocks(partial);
        // a TNT duper's ground (real blocks of its world, `Session::floor`): what changed in it (a
        // crater, something fallen in) and the ground cells next to that are drawn as blocks, the
        // rest by its intact faces (`mesh::floor`); the intact cells around the drawn ones occlude
        // and shade them like the blocks they are
        let floor = s.floor.as_ref().filter(|_| s.simulated() && s.replay.is_none());
        let mut skip: HashSet<Pos> = HashSet::new();
        let mut extra_occ: HashSet<Pos> = HashSet::new();
        const NEXT: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        let add = |c: Pos, d: [i32; 3]| [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
        if let Some(f) = floor {
            let mut as_blocks: HashSet<Pos> = HashSet::new();
            for (c, _) in &f.changed {
                skip.insert(*c);
                for n in std::iter::once(*c).chain(NEXT.iter().map(|d| add(*c, *d))) {
                    if f.contains(n) && f.shown(n).is_some() {
                        as_blocks.insert(n);
                    }
                }
            }
            for c in &as_blocks {
                skip.insert(*c);
                if let Some(state) = f.shown(*c) {
                    blocks.push(crate::session::RBlock { cell: *c, offset: [0.0; 3], state });
                }
            }
            for c in &as_blocks {
                for d in NEXT {
                    let n = add(*c, d);
                    if f.contains(n) && !skip.contains(&n) {
                        extra_occ.insert(n);
                    }
                }
            }
        }
        Staged { part, blocks, skip, extra_occ, builder: None, next: 0 }
    }

    /// The blocks it draws.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Build on until done or `out_of_time` (the first pass, the light, then the blocks in order,
    /// asked every 16): the mesh once done (the session and look must be the ones it started
    /// with: its entities, outlines and ground are added at the end).
    pub fn step(&mut self, atlas: &Atlas, s: &Session, partial: f32, look: &Look, out_of_time: &mut dyn FnMut() -> bool) -> Option<Mesh> {
        let floor = s.floor.as_ref().filter(|_| s.simulated() && s.replay.is_none());
        let items: Vec<Item> = self.blocks.iter().map(|b| Item { cell: b.cell, offset: b.offset, state: &b.state }).collect();
        let mut b = match self.builder.take() {
            Some(p) => Builder::from_parts(atlas, p),
            None => {
                let mut b = Builder::new(atlas);
                b.ground = look.ground.map(|y| y.floor() as i32);
                // a TNT duper stands on its own ground (its top layer's air above it is lit), its
                // craters open to the air
                b.light_floor = floor.map(|f| f.hi[1] + 1);
                b.light_holes = floor.map(|f| f.changed.keys().copied().collect()).unwrap_or_default();
                b.extra_occ = std::mem::take(&mut self.extra_occ);
                b.ghost = look.ghost.iter().copied().collect();
                b.blocks_begin(&items);
                if out_of_time() {
                    self.builder = Some(b.into_parts());
                    return None;
                }
                b
            }
        };
        self.next = b.blocks_from(&items, self.next, out_of_time);
        if self.next < items.len() {
            self.builder = Some(b.into_parts());
            return None;
        }
        if self.part == Part::All {
            entities_into(&mut b, s, partial);
        }
        if let Some(m) = look.marker {
            b.marker(m, true, [255, 255, 255], look.lit_marker && s.output_on().unwrap_or(false));
        }
        for o in &look.outlines {
            b.marker(*o, false, [110, 230, 40], false);
        }
        for (lo, hi) in &look.input_spans {
            b.marker_span(*lo, *hi, [110, 230, 40]);
        }
        for (o, c) in &look.port_outlines {
            // an output's box: red while it is powered (redsim's state this tick), filled red but
            // over a lamp (the clock judge's): the lamp shows its own state, the game's 4 gt
            // off-delay and all (critique: a 4-tick clock's lamp seemed to go dark under the box's
            // fill, while the game keeps it lit)
            if s.outputs.contains(o) && s.output_powered(*o) {
                b.marker(*o, false, OUT_POWERED, false);
                let lamp = s.simulated() && matches!(s.world.get(bp(*o)), Block::Lamp { .. });
                if !lamp {
                    b.out_fill(*o);
                }
            } else {
                b.marker(*o, false, *c, false);
            }
        }
        if let Some((lo, hi, axis, normal)) = look.track {
            b.track(lo, hi, axis, normal, TRACK_COLOUR);
        }
        let light = b.take_light();
        let light_at = |c: Pos| -> (u8, u8) { light.as_ref().map_or((15, 0), |l| l.at(c)) };
        let mut m = b.mesh;
        if let Some(f) = floor {
            m.solid.extend_from_slice(&mesh::floor(atlas, f.lo, f.hi, &FLOOR_SIDES, &self.skip, f.design_box, &light_at));
        }
        if let Some(y) = look.ground {
            // the floor: past the fog's end in the viewer (no edge ever shows: the far grass fades
            // into the sky like the game's render distance); `web compare` passes the real-game
            // pictures' own reach (`clock shot` fills grass around a batch of clocks: 56 is the
            // best mean over its pictures, their sky shows beyond the edge); a travelling design's
            // whole path
            let (lo, hi) = if s.corridor.is_some() { s.path_box() } else { s.frame_box() };
            let margin = look.floor_margin.unwrap_or(GROUND_REACH);
            let g = mesh::ground(atlas, [lo[0] as f32, 0.0, lo[2] as f32], [hi[0] as f32 + 1.0, 0.0, hi[2] as f32 + 1.0], y, margin, &light_at);
            m.solid.extend_from_slice(&g);
        }
        Some(m)
    }
}

/// The frame's entities (a recording's, a still's design entities, else redsim's) and an
/// explosion's cloud for 10 ticks after each of redsim's blasts.
fn entities_into(b: &mut Builder, s: &Session, partial: f32) {
    if let Some(r) = &s.replay {
        for e in r.entities(partial) {
            b.entity(&e);
        }
    } else if !s.simulated() {
        for e in &s.design.entities {
            b.entity(&EntityDraw { kind: e.kind.clone(), pos: [e.pos[0] as f32, e.pos[1] as f32, e.pos[2] as f32], yaw: entity_yaw(e.nbt.as_deref()), ..Default::default() });
        }
    } else {
        for e in s.entities(partial) {
            b.entity(&e);
        }
        // an explosion's cloud for 10 ticks after each of redsim's blasts (the game client draws
        // its particles; a blast in mid-air breaks nothing and showed nothing at all: critique on
        // the precise drop, "no TNT falls or explodes")
        let now = s.world.time as f32 + partial;
        for x in s.world.explosions.iter().rev() {
            let age = now - x.time as f32;
            if age >= crate::particles::SHOWS_FOR {
                break;
            }
            if age >= 0.0 {
                b.explosion([x.center.x as f32, x.center.y as f32, x.center.z as f32], x.time as u64, age);
            }
        }
    }
}

/// The floor [`build`] draws, with nothing on it (`web compare`: the pixels that differ from it
/// are the build's).
pub fn ground_mesh(atlas: &Atlas, s: &Session, look: &Look) -> Mesh {
    let mut m = Mesh::default();
    if let Some(y) = look.ground {
        let (lo, hi) = if s.corridor.is_some() { s.path_box() } else { s.frame_box() };
        let margin = look.floor_margin.unwrap_or(GROUND_REACH);
        m.solid = mesh::ground(atlas, [lo[0] as f32, 0.0, lo[2] as f32], [hi[0] as f32 + 1.0, 0.0, hi[2] as f32 + 1.0], y, margin, &mesh::daylight);
    }
    m
}

/// `Rotation:[yaw f, pitch f]` in an entity's SNBT.
pub fn entity_yaw(nbt: Option<&str>) -> f32 {
    let Some(s) = nbt else { return 0.0 };
    let Some(i) = s.find("Rotation:[") else { return 0.0 };
    let rest = &s[i + "Rotation:[".len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    num.parse().unwrap_or(0.0)
}

/// The box the default camera frames: the design (and marker), 0.25 margin like the pictures.
pub fn frame_bounds(s: &Session, look: &Look) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = s.frame_box();
    if let Some(m) = look.marker {
        for k in 0..3 {
            lo[k] = lo[k].min(m[k]);
            hi[k] = hi[k].max(m[k]);
        }
    }
    ([lo[0] as f32 - 0.25, lo[1] as f32 - 0.25, lo[2] as f32 - 0.25], [hi[0] as f32 + 1.25, hi[1] as f32 + 1.25, hi[2] as f32 + 1.25])
}

/// The real-game pictures' camera for a session at an aspect ratio: `mctas clock shot` frames the
/// box at a vertical FOV of 70 degrees, then keeps the centered 64% of the frame (its ffmpeg
/// `crop=iw*0.64:ih*0.64`): the same eye with a narrower view; a build may run off its edges.
/// Only `web compare` (and `web fit`) use it, to compare with those pictures; the viewer frames
/// with [`default_camera`].
pub fn picture_camera(s: &Session, look: &Look, aspect: f32) -> Camera {
    camera_from(s, look, aspect, CAM_DIR)
}

/// Where a live viewer's build goes: x within +-0.8 of the view (80% of its width), y between
/// the bar (bottom, 60 of 480 px) and the badges (top), with room above for the tags. A viewer
/// of known height uses [`live_window`].
pub const LIVE_WINDOW: (f32, f32, f32) = (0.8, -0.7, 0.62);
/// A thumbnail's: 80% of each side, centred.
pub const THUMB_WINDOW: (f32, f32, f32) = (0.8, -0.8, 0.8);

/// [`LIVE_WINDOW`] for a viewer `h` CSS px tall: clear of its bar (60 px and a margin) and its
/// badges and tags (44 px) whatever its height (a short phone viewer: the bar takes more of it).
pub fn live_window(h: f32) -> (f32, f32, f32) {
    let h = h.max(120.0);
    let y0 = -1.0 + 2.0 * 74.0 / h;
    let y1 = (1.0 - 2.0 * 46.0 / h).max(y0 + 0.4);
    (0.8, y0, y1)
}

/// The smallest build a view frames, in blocks across (x, z) and high: a tiny one (a 2-block
/// clock) stands in a little grass instead of filling the view edge to edge.
const MIN_VIEW_W: f32 = 3.0;
const MIN_VIEW_H: f32 = 2.0;

/// A long thin build (its plan this many times longer than wide) is seen at 45 degrees along its
/// length (a 3/4 view) instead of side-on: its length foreshortened, the camera closer.
const LONG_BUILD: f32 = 3.0;

/// The direction a view looks from: the pictures' ([`CAM_DIR`]), or for a long thin build the
/// same elevation at 45 degrees in plan (from the same quarter).
pub fn view_dir(pts: &[[f32; 3]]) -> [f32; 3] {
    if pts.is_empty() {
        return CAM_DIR;
    }
    let ext = |k: usize| pts.iter().map(|p| p[k]).fold(f32::MIN, f32::max) - pts.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
    let (wx, wz) = (ext(0), ext(2));
    if wx.max(wz) < LONG_BUILD * wx.min(wz).max(1.0) {
        return CAM_DIR;
    }
    let h = (CAM_DIR[0] * CAM_DIR[0] + CAM_DIR[2] * CAM_DIR[2]).sqrt() / std::f32::consts::SQRT_2;
    [-h, CAM_DIR[1], h]
}

/// The viewer's camera (live viewers and thumbnails alike): from the pictures' direction, the
/// build's whole moving extent ([`view_points`]) filling `window`, never cut.
pub fn default_camera(s: &Session, look: &Look, aspect: f32) -> Camera {
    let pts = view_points(s, look);
    view_camera_at(s, look, &pts, aspect, view_dir(&pts), LIVE_WINDOW)
}

/// The points a view keeps in frame: the corners of every cell the build occupies or sweeps (its
/// blocks now and as placed, a door's opening, a clock's swept box over a period, a piston
/// extender's travel, its ports, inputs and marker, its entities); not its bounding box (a
/// sparse build would come out small).
pub fn view_points(s: &Session, look: &Look) -> Vec<[f32; 3]> {
    points(s, look, true, TNT_VIEW_REACH)
}

/// [`view_points`] for a view of this aspect: a portrait one (a phone's viewer) keeps a TNT
/// duper's landing zone only as near as any other run's entities ([`VIEW_REACH`]): beside the
/// duper in a narrow view it left the duper a speck in a corner.
pub fn view_points_for(s: &Session, look: &Look, aspect: f32) -> Vec<[f32; 3]> {
    points(s, look, true, if aspect >= 1.0 { TNT_VIEW_REACH } else { VIEW_REACH })
}

/// [`view_points`] of a still (a thumbnail): what the build occupies now and as placed, without
/// the travel ahead (a long extender's path, where redsim's run takes its entities).
pub fn still_points(s: &Session, look: &Look) -> Vec<[f32; 3]> {
    points(s, look, false, VIEW_REACH)
}

fn points(s: &Session, look: &Look, moving: bool, tnt_reach: f32) -> Vec<[f32; 3]> {
    let mut cells: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for b in s.render_blocks(0.0) {
        cells.insert(b.cell);
    }
    cells.extend(s.design.blocks.keys().copied());
    // a still (a thumbnail) frames the build, not the judge's see-through test rig around its
    // inputs (critique: the gate tiles came out small, washed out by the rig)
    if !moving {
        for g in &look.ghost {
            cells.remove(g);
        }
    }
    // the inputs and ports where the look marks them (a thumbnail draws no outlines: its empty
    // input and port cells are not framed; critique: a BUD switch's lone observer sat beside its
    // empty TARGET cell in a big frame); levers and buttons are blocks (above)
    cells.extend(look.outlines.iter().copied());
    cells.extend(look.port_outlines.iter().map(|(p, _)| *p));
    cells.extend(look.marker);
    if let crate::session::Mode::Door { door, along_z, size, .. } = s.mode {
        cells.extend(crate::session::door_cells(door, along_z, size));
    }
    let mut boxes: Vec<(Pos, Pos)> = Vec::new();
    if let Some(b) = s.picture {
        boxes.push(b);
    }
    // a driven input's whole lime box (its cell, the judge's repeater and switch)
    boxes.extend(look.input_spans.iter().copied());
    // a design that travels (a piston extender's payload): its whole swept path (without the
    // corridor's free cells; a still: not); a flying machine is followed instead
    let flies = s.design.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine"));
    if moving
        && s.corridor.is_some()
        && !flies
        && let Some((b, _)) = crate::session::travel_box(&s.design, &s.mode)
    {
        boxes.push(b);
    }
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for c in &cells {
        for k in 0..8 {
            pts.push([c[0] as f32 + (k & 1) as f32, c[1] as f32 + ((k >> 1) & 1) as f32, c[2] as f32 + ((k >> 2) & 1) as f32]);
        }
    }
    for (lo, hi) in boxes {
        for k in 0..8 {
            pts.push([if k & 1 == 0 { lo[0] } else { hi[0] + 1 } as f32, if k & 2 == 0 { lo[1] } else { hi[1] + 1 } as f32, if k & 4 == 0 { lo[2] } else { hi[2] + 1 } as f32]);
        }
    }
    let ents = if s.simulated() { s.entities(0.0) } else { Vec::new() };
    let mut ent_pos: Vec<[f32; 3]> = ents.iter().map(|e| e.pos).chain(s.design.entities.iter().map(|e| [e.pos[0] as f32, e.pos[1] as f32, e.pos[2] as f32])).collect();
    // a design with entities or TNT: where they go in redsim's run (a launched TNT's flight, a
    // cart's loop), within VIEW_REACH blocks of the build (one flung far away leaves the view)
    if moving && (!s.design.entities.is_empty() || crate::session::is_tnt_design(&s.design)) {
        let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
        // a TNT duper's landing zone, where its TNT blow their crater, can lie further out
        // (critique: tnt-clocked-22b's pit cut off at the view's edge): its TNT near the ground
        // count within `tnt_reach`, those in the air within VIEW_REACH like any run's entities (an
        // air burst's flight left its duper small in a wide frame)
        let surface = s.floor.as_ref().map(|f| f.hi[1] as f32 + 1.0);
        let within = |p: &[f32; 3], r: f32| (0..3).all(|k| p[k] >= lo[k] - r && p[k] <= hi[k] + r);
        // a duper standing high over its ground (a precise drop's declared stand): its TNT's
        // fall under it down to their burst point, however far (coordinator: "Camera should
        // frame machine + burst point")
        let column = |p: &[f32; 3]| {
            s.floor.as_ref().is_some_and(|f| f.design_box.0[1] - 1 - f.hi[1] > VIEW_REACH as i32) && surface.is_some_and(|y| p[1] > y) && [0, 2].iter().all(|&k| p[k] >= lo[k] - VIEW_REACH && p[k] <= hi[k] + VIEW_REACH)
        };
        let near = |p: &[f32; 3]| within(p, VIEW_REACH) || column(p) || surface.is_some_and(|y| p[1] <= y + 2.0 && within(p, tnt_reach));
        // a TNT duper's TNT that sinks into the craters it blows in its judge's ground counts at
        // the ground's surface: the pit shows from above its rim, and framing its bottom looked
        // down on a field of grass (critique on tnt-clocked-22b: ~70% of a phone's view grass)
        ent_pos.extend(s.run_extent(VIEW_RUN).into_iter().filter(near).map(|p| match surface {
            Some(y) if p[1] < y => [p[0], y, p[2]],
            _ => p,
        }));
    }
    for e in ent_pos {
        for k in 0..8 {
            pts.push([e[0] + if k & 1 == 0 { -0.7 } else { 0.7 }, e[1] + if k & 2 == 0 { 0.0 } else { 1.0 }, e[2] + if k & 4 == 0 { -0.7 } else { 0.7 }]);
        }
    }
    // a TNT duper standing above its judge's ground (8 blocks or less): in a live view the ground
    // under it is in the picture, so its height shows (critique: from above it looked like it sat
    // on the grass); a thumbnail frames the build alone (critique: a speck on a green field)
    if let Some(f) = s.floor.as_ref().filter(|_| moving) {
        let (lo, hi) = f.design_box;
        let up = lo[1] - 1 - f.hi[1];
        if up > 0 && up as f32 <= VIEW_REACH {
            for (x, z) in [(lo[0], lo[2]), (hi[0] + 1, hi[2] + 1)] {
                pts.push([x as f32, f.hi[1] as f32 + 1.0, z as f32]);
            }
        }
    }
    pts
}

/// A tiny build's stand-in box (smaller than MIN_VIEW_W in every direction: framed as if
/// MIN_VIEW_W blocks across and MIN_VIEW_H high, around its middle; a thin tower is not tiny):
/// its 8 corners, or None.
fn tiny_box(pts: &[[f32; 3]]) -> Option<Vec<[f32; 3]>> {
    if pts.is_empty() {
        return None;
    }
    let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
    let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
    if !(0..3).all(|k| hi[k] - lo[k] < MIN_VIEW_W) {
        return None;
    }
    let half = |k: usize, min: f32| {
        let e = hi[k] - lo[k];
        // a block more than the build, up to the minimum (a lone block framed as 2, not 3:
        // critique, a 1-block BUD at a fifth of its tile)
        (e.max((e + 1.0).min(min))) / 2.0
    };
    let (hx, hy, hz) = (half(0, MIN_VIEW_W), half(1, MIN_VIEW_H), half(2, MIN_VIEW_W));
    // around the build's middle, up and down alike (it sits mid-frame, not low)
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    Some((0..8).map(|k| [c[0] + if k & 1 == 0 { -hx } else { hx }, c[1] + if k & 2 == 0 { -hy } else { hy }, c[2] + if k & 4 == 0 { -hz } else { hz }]).collect())
}

/// Fit `cam` (its direction kept) to `pts` in `window`: the build centred in the window, a tiny
/// one ([`tiny_box`]) as far away as its stand-in box needs (it stays centred: the old padding
/// points pushed a tiny build off-centre from some sides).
pub fn fit_view(cam: &mut Camera, pts: &[[f32; 3]], aspect: f32, window: (f32, f32, f32)) {
    // a thumbnail (THUMB_WINDOW) fits the build itself, however small: a tile is small, its
    // margins are enough (critique: a lone BUD observer at a quarter of its tile)
    let min = tiny_box(pts)
        .filter(|_| window != THUMB_WINDOW)
        .map(|b| {
            let mut c = *cam;
            c.fit_points(&b, aspect, window.0, window.1, window.2);
            c.dist
        })
        .unwrap_or(0.0);
    cam.fit_points_min(pts, aspect, window.0, window.1, window.2, min);
}

/// How many ticks of redsim's run frame a design's moving entities ([`view_points`]).
const VIEW_RUN: u32 = 200;
/// How far beyond the build those entities stay framed.
const VIEW_REACH: f32 = 8.0;
/// [`VIEW_REACH`] of a TNT duper standing on its judge's ground: its landing zone.
const TNT_VIEW_REACH: f32 = 16.0;

/// The viewer's framing from direction `dir`: [`view_points`] filling `window` (see
/// [`LIVE_WINDOW`]) at the pictures' cropped field of view.
pub fn view_camera(s: &Session, look: &Look, aspect: f32, dir: [f32; 3], window: (f32, f32, f32)) -> Camera {
    view_camera_at(s, look, &view_points(s, look), aspect, dir, window)
}

/// [`view_camera`] with the points already known.
fn view_camera_at(s: &Session, look: &Look, pts: &[[f32; 3]], aspect: f32, dir: [f32; 3], window: (f32, f32, f32)) -> Camera {
    let (lo, hi) = frame_bounds(s, look);
    let crop = 2.0 * (0.64 * (VFOV.to_radians() / 2.0).tan()).atan().to_degrees();
    let mut cam = Camera::frame(lo, hi, dir, crop, aspect, 0.85);
    fit_view(&mut cam, pts, aspect, window);
    cam
}

/// The pictures' framing from direction `dir` (eye side, from the target): `clock shot`'s camera
/// exactly (`video::frame_box`: the box at 85% of the game's full 70-degree frame), then its
/// centre crop (64%): a build may run off the picture's edges like in the real-game pictures.
pub fn camera_from(s: &Session, look: &Look, aspect: f32, dir: [f32; 3]) -> Camera {
    let (lo, hi) = frame_bounds(s, look);
    let crop = 2.0 * (0.64 * (VFOV.to_radians() / 2.0).tan()).atan().to_degrees();
    if s.design.meta.contains_key("circuit") {
        // a circuit: the whole build in view (no real-game picture to match; a tall BUD switch
        // would run off the cropped frame)
        let mut cam = Camera::frame(lo, hi, dir, crop, aspect, 0.85);
        cam.vfov = crop;
        return cam;
    }
    let mut cam = Camera::frame(lo, hi, dir, VFOV, aspect, 0.85);
    cam.vfov = crop;
    cam
}

/// Solid and translucent triangles of a mesh for a camera.
pub fn triangles(m: &Mesh, cam: &Camera) -> (Vec<Vertex>, Vec<Vertex>) {
    let mut solid = m.solid.clone();
    if !m.sprites.is_empty() {
        // the particles face the camera (the game's: in the camera's plane)
        let (_, r, u) = cam.basis();
        solid.reserve(m.sprites.len() * 6);
        for s in &m.sprites {
            solid.extend_from_slice(&s.quad(r, u));
        }
    }
    if !m.flames.is_empty() {
        // a burning entity's flames turn about the vertical toward the camera
        // (`Mth.rotationAroundAxis(Y, camera.orientation)`)
        let (f, r, _) = cam.basis();
        let flat = |v: [f32; 3]| {
            let l = (v[0] * v[0] + v[2] * v[2]).sqrt().max(1e-6);
            [v[0] / l, 0.0, v[2] / l]
        };
        let (right, back) = (flat(r), flat([-f[0], -f[1], -f[2]]));
        for fl in &m.flames {
            solid.extend(crate::entitymodel::flame_triangles(fl, right, back));
        }
    }
    (solid, m.sorted_translucent(cam.eye()))
}

/// Where on the view (x, y in -1..1, y up) a click picks `cell`: the first of a few points of the
/// cell that the pick ray reaches (None: hidden or out of view).
pub fn click_point(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
    click_point_at(atlas, s, cam, cell, aspect, &[0.5, 0.3, 0.7, 0.15, 0.85])
}

/// [`click_point`] at the cell's centre only: where a tap on it (and a test's click) lands.
pub fn centre_point(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
    click_point_at(atlas, s, cam, cell, aspect, &[0.5])
}

fn click_point_at(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32, steps: &[f32]) -> Option<(f32, f32)> {
    let picker = crate::pick::Picker::new(atlas, &s.render_blocks(0.0));
    click_point_with(&picker, cam, cell, aspect, steps)
}

/// [`click_point_at`] with the blocks' boxes already gathered.
fn click_point_with(picker: &crate::pick::Picker, cam: &Camera, cell: Pos, aspect: f32, steps: &[f32]) -> Option<(f32, f32)> {
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let max = cam.dist * 4.0 + 50.0;
    let empty = !picker.occupied(cell);
    for &dy in steps {
        for &dx in steps {
            for &dz in steps {
                let p = [cell[0] as f32 + dx, cell[1] as f32 + dy, cell[2] as f32 + dz];
                let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
                let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
                let z = dot(f);
                if z <= 0.01 {
                    continue;
                }
                let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
                if x.abs() >= 0.98 || y.abs() >= 0.98 {
                    continue;
                }
                let (o, d) = cam.ray(x, y, aspect);
                let picked = picker.hit(o, d, max).map(|(c, _)| c);
                if picked == Some(cell) {
                    return Some((x, y));
                }
                // an empty cell (an input's cell with nothing in it): reached before any block,
                // the rule of `Player::input_at`, a few pixels around too (not a sliver)
                if empty {
                    let reached = |x: f32, y: f32| -> bool {
                        let (o, d) = cam.ray(x, y, aspect);
                        let picked = picker.hit(o, d, max).map(|(c, _)| c);
                        cell_entry(o, d, cell).is_some_and(|t| t <= picked.and_then(|p| cell_entry(o, d, p)).unwrap_or(f32::MAX) + 1e-3)
                    };
                    let e = 0.008;
                    if [(0.0, 0.0), (e, 0.0), (-e, 0.0), (0.0, e), (0.0, -e)].iter().all(|(a, b)| reached(x + a, y + b)) {
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

/// The live view's default camera ([`best_camera`] of the build's whole moving extent in
/// `window`; every lever, button and input tappable when a side allows it).
pub fn camera_showing_inputs(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32)) -> Camera {
    best_camera(atlas, s, look, aspect, window, &view_points_for(s, look, aspect))
}

/// A live viewer narrower than this (CSS px) is a phone's: two-line badges at the top (more room
/// kept there), the Record pill over the bottom right.
pub const NARROW_PX: f32 = 520.0;
/// The Record pill's box from the view's bottom right (CSS px: right margin, width, bottom,
/// height, a margin around it).
pub const PILL: (f32, f32, f32, f32) = (8.0, 104.0, 68.0, 36.0);

/// A live viewer's window ([`live_window`]) for its CSS size: on a phone ([`NARROW_PX`]) 66 px
/// kept at the top (the time badge on two lines and the output strip under it; critique: the
/// strip sat over a clock's block and the badge touched it).
pub fn live_window_css(w: f32, h: f32) -> (f32, f32, f32) {
    if w < NARROW_PX {
        let h = h.max(120.0);
        let y0 = -1.0 + 2.0 * 74.0 / h;
        let y1 = (1.0 - 2.0 * 66.0 / h).max(y0 + 0.4);
        (0.8, y0, y1)
    } else {
        live_window(h)
    }
}

/// A live viewer's default camera at its CSS size: [`camera_showing_inputs`] in
/// [`live_window_css`]; on a phone, when the build would come out under the Record pill (bottom
/// right, [`PILL`]), the window starts above the pill instead (a build clear of it keeps the
/// lower window: critique, a BUD sat in the top half with 40% empty below).
pub fn live_camera(atlas: &Atlas, s: &Session, look: &Look, w: f32, h: f32) -> (Camera, (f32, f32, f32)) {
    let mut search = LiveCameraSearch::new(atlas, s, look, w, h);
    loop {
        if let Some(r) = search.step(atlas, s, look, &mut || false) {
            return r;
        }
    }
}

/// [`live_camera`] before it was staged (kept for the test that the staged search finds the
/// same camera).
#[cfg(test)]
pub fn live_camera_at_once(atlas: &Atlas, s: &Session, look: &Look, w: f32, h: f32) -> (Camera, (f32, f32, f32)) {
    let aspect = w / h.max(1.0);
    let window = live_window_css(w, h);
    let cam = camera_showing_inputs(atlas, s, look, aspect, window);
    if w >= NARROW_PX {
        return (cam, window);
    }
    // the pill's box in view coordinates (x right, y up, -1..1)
    let (right, pw, bottom, ph) = PILL;
    let px0 = 1.0 - 2.0 * (right + pw + 4.0) / w;
    let py1 = -1.0 + 2.0 * (bottom + ph + 4.0) / h;
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let covered = view_points_for(s, look, aspect).iter().any(|p| {
        let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let z = dot(f).max(0.01);
        let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
        x > px0 && y < py1
    });
    if !covered {
        return (cam, window);
    }
    let raised = (window.0, (-1.0 + 2.0 * (bottom + ph + 10.0) / h.max(120.0)).min(window.2 - 0.4), window.2);
    (camera_showing_inputs(atlas, s, look, aspect, raised), raised)
}

/// A thumbnail's look (the site's tiles: a picture, nothing to press): no input or port outlines,
/// but a BUD switch's TARGET box, the cell its block changes happen at (critique: the BUD tile
/// was a lone observer "with no TARGET").
pub fn thumbnail_look(s: &Session, look: &mut Look) {
    look.outlines.clear();
    look.input_spans.clear();
    look.track = None;
    let targets: Vec<Pos> = s.design.ports.iter().filter(|(n, _)| *n == "target" || n.starts_with("target.")).map(|(_, (p, _))| *p).collect();
    look.port_outlines.retain(|(p, _)| targets.contains(p));
}

/// A thumbnail's camera: [`best_camera`] of the still ([`still_points`]) in [`THUMB_WINDOW`].
pub fn thumb_camera(atlas: &Atlas, s: &Session, look: &Look, aspect: f32) -> Camera {
    // (a display: its face fills the card, so its digit reads at a glance; the coordinator
    // 2026-10-22: the card's digit came out a fifth of its picture)
    if let Some(pts) = face_points(s) {
        return best_camera(atlas, s, look, aspect, THUMB_WINDOW, &pts);
    }
    best_camera(atlas, s, look, aspect, THUMB_WINDOW, &still_points(s, look))
}

/// A display face's frame for a card (`circuit::display_face`): the box of its lamps, a block
/// more around them in the face's plane and three behind it (a glimpse of the decoder), as its
/// eight corners.
fn face_points(s: &Session) -> Option<Vec<[f32; 3]>> {
    let way = crate::circuit::display_face(&s.design)?;
    let outs: Vec<Pos> = s.design.ports.iter().filter(|(n, _)| n.starts_with("output")).map(|(_, (p, _))| *p).collect();
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in &outs {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let (mut lo, mut hi) = (lo.map(|v| v as f32), hi.map(|v| v as f32 + 1.0));
    for k in 0..3 {
        if way[k] == 0.0 {
            lo[k] -= 1.0;
            hi[k] += 1.0;
        } else if way[k] < 0.0 {
            hi[k] += 3.0;
        } else {
            lo[k] -= 3.0;
        }
    }
    Some((0..8).map(|i| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }]).collect())
}

/// The default camera's directions (target to eye): the pictures' ([`CAM_DIR`]: 29 degrees off
/// the south axis, 28 up) and its 7 mirror images around the build, at the pictures' elevation
/// and at 45 degrees (each 29 degrees off an axis: three faces seen, never straight along a flat
/// face, never from below), then the 4 face-on directions, low ([`FACE_ON_PITCH`]: a flat build,
/// one block thick, is read face-on, like a drawing: critique on pext-n3-flat-cascade16, "at ~25°
/// rather than face-on"; any other build gets them only at a cost, [`FACE_ON_COST`]). The
/// pictures' own comes first (it wins ties).
pub fn candidate_dirs() -> Vec<[f32; 3]> {
    let d = CAM_DIR;
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let yaw0 = (d[0] / len).atan2(d[2] / len).abs();
    let pitch0 = (d[1] / len).asin();
    let mut v = Vec::new();
    for pitch in [pitch0, 45f32.to_radians()] {
        for q in 0..4 {
            for s in [-1.0f32, 1.0] {
                let yaw = q as f32 * std::f32::consts::FRAC_PI_2 + s * yaw0;
                v.push([pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()]);
            }
        }
    }
    let low = FACE_ON_PITCH.to_radians();
    for q in 0..4 {
        let yaw = q as f32 * std::f32::consts::FRAC_PI_2;
        v.push([low.cos() * yaw.sin(), low.sin(), low.cos() * yaw.cos()]);
    }
    v
}

/// The face-on candidates' elevation (degrees): low enough that a wall reads like its drawing,
/// high enough to see what stands on top of it.
pub const FACE_ON_PITCH: f32 = 12.0;

/// What a face-on candidate costs a build thicker than one block along the view (it would see a
/// single face of a 3D build: flat), and what it gains a one-block-thick wall seen from its face.
pub const FACE_ON_COST: f32 = 1.5;
pub const FACE_ON_BONUS: f32 = 1.0;

/// What a face-on candidate costs on top of [`FACE_ON_COST`] when it looks at a long build's
/// narrow end (the build at least [`END_ON_RATIO`] times as deep along the view as it is wide
/// across it): from that low, the near end hides everything behind it (the reviewer on
/// cane-sweeper-10x24, 14×6×34, on a desktop window: its station's end wall filled the view, no
/// cane and no machine; the output chests seen at that end won it the pick).
pub const END_ON_COST: f32 = 3.0;
pub const END_ON_RATIO: f32 = 1.5;

/// How much a block tells of how a build works (the default camera wants these in view):
/// pistons, observers, comparators, repeaters, torches, levers and buttons most; dust, lamps,
/// containers, rails and moving blocks less; plain blocks not at all.
pub fn part_weight(state: &str) -> f32 {
    let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    match id {
        "piston" | "sticky_piston" | "piston_head" | "moving_piston" | "observer" | "comparator" | "repeater" | "redstone_torch" | "redstone_wall_torch" | "lever" | "sculk_sensor" | "calibrated_sculk_sensor" => 1.0,
        "redstone_lamp" | "note_block" | "target" | "daylight_detector" | "bell" => 0.8,
        "redstone_wire" => 0.6,
        "dispenser" | "dropper" | "crafter" | "hopper" | "tnt" | "chest" | "barrel" => 0.6,
        "slime_block" | "honey_block" | "redstone_block" => 0.4,
        _ if id.ends_with("_button") => 1.0,
        _ if id.ends_with("copper_bulb") => 0.8,
        _ if id.ends_with("rail") || id.ends_with("shulker_box") => 0.6,
        _ if id.ends_with("trapdoor") || id.ends_with("_door") || id.ends_with("fence_gate") => 0.5,
        _ => 0.0,
    }
}

/// The most components the default camera scores (the heaviest, then by position).
const MAX_PARTS: usize = 64;

/// What the default camera is scored on ([`best_camera`]).
struct Targets {
    /// The port cells (inputs, outputs, the other ports, a door's opening) and their weights.
    ports: Vec<(Pos, f32)>,
    /// The components ([`part_weight`]), at most [`MAX_PARTS`].
    parts: Vec<(Pos, f32)>,
    /// The inputs a person taps, and the build's middle (the input side faces the camera).
    inputs: Vec<Pos>,
    /// Its outputs (a build without inputs: a far output's end faces the camera).
    outputs: Vec<Pos>,
    middle: [f32; 3],
    /// The build's own cells' corners (the design's blocks: not the TNT's flight, not an
    /// extender's travel): how big the build itself comes out in the frame.
    build: Vec<[f32; 3]>,
    /// Where a live view's tags hang (`tag_anchors`; none in a thumbnail): two of them on the
    /// same spot stack one tag on the other.
    anchors: Vec<[f32; 3]>,
    /// A thumbnail's drawn port cells' centres (none in a live view): two of them on the same spot
    /// of the picture hide one another (a BUD switch's TARGET box over its observer).
    centers: Vec<[f32; 3]>,
    /// Item frames a person turns (a combination lock's): (frame position, the way it faces,
    /// weight; a lock's dials 1, its code frames 0.25: the dials are what one turns, the code
    /// set once): a frame is seen from the side it faces.
    frames: Vec<([f32; 3], [f32; 3], f32)>,
}

/// Where the live viewer's tags hang (`overlay.rs`: each input's, each port group's, a clock's
/// OUT: the controls a player uses and the outputs, never a tag that only names a block the view
/// shows, vjeux 2026-10-21: "please remove the water label, we can see it"): over the middle of
/// its cells, at the top one's height.
pub fn tag_anchors(s: &Session) -> Vec<[f32; 3]> {
    let at = |cells: &[Pos]| -> [f32; 3] {
        let n = cells.len().max(1) as f32;
        let mid = |k: usize| cells.iter().map(|c| c[k] as f32 + 0.5).sum::<f32>() / n;
        let top = cells.iter().map(|c| c[1]).max().unwrap_or(0) as f32 + 1.05;
        [mid(0), top, mid(2)]
    };
    let mut v: Vec<[f32; 3]> = s.circuit_inputs.iter().map(|ci| at(&[ci.pos])).collect();
    // (a display face's per-lamp output tags are hidden by default: `overlay.rs`, the Labels
    // button shows them)
    let face = crate::circuit::display_face(&s.design).is_some();
    v.extend(crate::circuit::port_groups(&s.design).iter().filter(|g| !(face && g.label.starts_with("OUT"))).map(|g| at(&g.cells)));
    if !s.design.meta.contains_key("circuit") && !s.mode.is_door()
        && let Some(o) = s.outputs.first()
    {
        v.push(at(&[*o]));
    }
    v
}

fn targets(s: &Session, look: &Look, pts: &[[f32; 3]]) -> Targets {
    let mut ports: Vec<(Pos, f32)> = Vec::new();
    let add = |c: Pos, w: f32, ports: &mut Vec<(Pos, f32)>| {
        if !ports.iter().any(|(p, _)| *p == c) {
            ports.push((c, w));
        }
    };
    // a combination lock's code frames are set once, from their tags: they neither count as
    // ports nor need a tappable side (critique on lock3-q85: the code frames, facing away from
    // the dials, kept the camera off the dials' faces: "the dials show as thin brown frames seen
    // edge-on")
    let code: Vec<Pos> = s.circuit_inputs.iter().filter(|ci| ci.how == crate::circuit::InputHow::Frame && ci.name.starts_with('k')).map(|ci| ci.pos).collect();
    for c in s.inputs.iter().filter(|c| !code.contains(c)) {
        add(*c, 3.0, &mut ports);
    }
    for c in &s.outputs {
        add(*c, 3.0, &mut ports);
    }
    // the other ports; a sink the judge counts what reaches (an item sorter's PASS chest) weighs
    // like an output (critique: the sorter's PASS chest mostly hidden behind its ITEMS chest)
    for (label, c, _) in crate::circuit::port_tags(&s.design) {
        add(c, if label.starts_with("PASS") { 3.0 } else { 1.5 }, &mut ports);
    }
    if let crate::session::Mode::Door { door, along_z, size, .. } = s.mode {
        for c in crate::session::door_cells(door, along_z, size) {
            add(c, 0.5, &mut ports);
        }
    }
    // a TNT duper's TNT is what it is about: its TNT blocks count among the ports, 2 in all (like
    // one output: a heavier weight looked down on it from above, critique on tnt-clocked-22b)
    if crate::session::is_tnt_design(&s.design) {
        let tnt: Vec<Pos> = s.render_blocks(0.0).into_iter().filter(|b| b.offset == [0.0; 3] && b.state.starts_with("minecraft:tnt")).map(|b| b.cell).collect();
        let w = 2.0 / tnt.len().max(1) as f32;
        for c in tnt {
            add(c, w, &mut ports);
        }
    }
    // a port cell counts when it is drawn: a block, or an outlined cell (a thumbnail outlines
    // nothing but a BUD switch's TARGET: an empty port cell there is not drawn)
    {
        let solid: std::collections::HashSet<Pos> = s.render_blocks(0.0).into_iter().map(|b| b.cell).collect();
        let outlined = |c: &Pos| look.outlines.contains(c) || look.port_outlines.iter().any(|(p, _)| p == c) || look.input_spans.iter().any(|(a, b)| (0..3).all(|k| c[k] >= a[k].min(b[k]) && c[k] <= a[k].max(b[k])));
        ports.retain(|(c, _)| solid.contains(c) || outlined(c));
    }
    // the containers whose labels tell the story (a design's own containers holding items, all
    // labelled but on a shulker design: a sorter's filter hopper, a dropper ring's droppers) count
    // like ports (critique: a sorter's filter hopper hid behind its PASS chest), six at most
    if !crate::circuit::is_shulker(&s.design) && s.simulated() {
        let mut held: Vec<Pos> = s.world.containers.map.iter().filter(|(_, c)| c.slots.iter().any(|x| x.is_some())).map(|(p, _)| p.arr()).filter(|p| s.design.blocks.contains_key(p)).collect();
        held.sort();
        for c in held.into_iter().take(6) {
            add(c, 1.5, &mut ports);
        }
    }
    let mut parts: Vec<(Pos, f32)> = s.render_blocks(0.0).into_iter().filter(|b| b.offset == [0.0; 3]).map(|b| (b.cell, part_weight(&b.state))).filter(|(c, w)| *w > 0.0 && !ports.iter().any(|(p, _)| p == c)).collect();
    parts.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    parts.truncate(MAX_PARTS);
    let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
    let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
    let middle = if pts.is_empty() { [0.0; 3] } else { [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0] };
    // the build itself: the design's cells (a design with no blocks: the framed points)
    let mut build: Vec<[f32; 3]> = Vec::new();
    for c in s.design.blocks.keys().filter(|c| !look.ghost.contains(c)) {
        for k in 0..8 {
            build.push([c[0] as f32 + (k & 1) as f32, c[1] as f32 + ((k >> 1) & 1) as f32, c[2] as f32 + ((k >> 2) & 1) as f32]);
        }
    }
    if build.is_empty() {
        build = pts.to_vec();
    }
    let live = !look.outlines.is_empty() || look.port_outlines.len() > look.port_outlines.iter().filter(|(p, _)| s.design.ports.iter().any(|(n, (q, _))| q == p && n.starts_with("target"))).count();
    let anchors = if live { tag_anchors(s) } else { Vec::new() };
    let centers = if live { Vec::new() } else { ports.iter().map(|(c, _)| [c[0] as f32 + 0.5, c[1] as f32 + 0.5, c[2] as f32 + 0.5]).collect() };
    // the dials' way: a code frame facing the other way is a hidden code (`group: lock-hid`: the
    // code on the back face, away from the player), which pulls no camera round to it
    let dial_way: [i32; 3] = s
        .circuit_inputs
        .iter()
        .filter(|ci| ci.how == crate::circuit::InputHow::Frame && !ci.name.starts_with('k'))
        .filter_map(|ci| s.world.frames.iter().find(|f| f.pos == bp(ci.pos)).map(|f| f.facing.step()))
        .fold([0, 0, 0], |a, st| [a[0] + st[0], a[1] + st[1], a[2] + st[2]]);
    let frames = s
        .circuit_inputs
        .iter()
        .filter(|ci| ci.how == crate::circuit::InputHow::Frame)
        .filter_map(|ci| {
            let f = s.world.frames.iter().find(|f| f.pos == bp(ci.pos))?;
            let st = f.facing.step();
            let away = st[0] * dial_way[0] + st[1] * dial_way[1] + st[2] * dial_way[2] < 0;
            let w = if !ci.name.starts_with('k') {
                1.0
            } else if away {
                0.0
            } else {
                0.25
            };
            Some((crate::session::frame_position(f), [st[0] as f32, st[1] as f32, st[2] as f32], w))
        })
        .collect();
    // (a bedrock breaker's swap pistons are tapped like inputs: the camera keeps them tappable)
    let inputs: Vec<Pos> = s.inputs.iter().copied().filter(|c| !code.contains(c)).chain(s.swaps.iter().map(|(c, _)| *c)).collect();
    Targets { ports, parts, inputs, outputs: s.outputs.clone(), middle, build, anchors, centers, frames }
}

/// How much of a cell the camera sees (0..1): of four points (its main model box's centre and
/// the centres of the three faces turned to the eye; an empty cell's own), the share that are in
/// the view and that a ray from the eye reaches first.
fn cell_seen(picker: &crate::pick::Picker, cam: &Camera, aspect: f32, cell: Pos) -> f32 {
    let eye = cam.eye();
    let (f, r, u) = cam.basis();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let (lo, hi, inset) = match picker.main_box(cell) {
        Some((lo, hi)) => (lo, hi, 0.02),
        None => ([cell[0] as f32, cell[1] as f32, cell[2] as f32], [cell[0] as f32 + 1.0, cell[1] as f32 + 1.0, cell[2] as f32 + 1.0], 0.05),
    };
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let mut samples = vec![c];
    for k in 0..3 {
        let mut p = c;
        p[k] = if eye[k] > c[k] { hi[k] - inset } else { lo[k] + inset };
        samples.push(p);
    }
    let block = picker.has(cell);
    let mut seen = 0;
    for p in &samples {
        let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let z = dot(f);
        if z <= 0.01 {
            continue;
        }
        let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
        if x.abs() >= 0.98 || y.abs() >= 0.98 {
            continue;
        }
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
        let d = [v[0] / len, v[1] / len, v[2] / len];
        let hit = picker.hit(eye, d, len + 1.0);
        let ok = if block {
            hit.is_some_and(|(hc, _)| hc == cell)
        } else {
            // an empty cell (an input or output cell the judge fills): nothing in front of it
            match hit {
                Some((_, h)) => cell_entry(eye, d, cell).is_some_and(|e| e <= h + 1e-3),
                None => true,
            }
        };
        if ok {
            seen += 1;
        }
    }
    seen as f32 / samples.len() as f32
}

/// The default camera (vjeux 2026-10-20 23:53Z: "Can we have some kind of heuristic to figure out
/// the default camera position. Many of them look really bad right now"): of the 16
/// [`candidate_dirs`], each framing `pts` in `window`, the one where (in order of weight) the
/// ports are seen (inputs, outputs, the other ports, a door's opening: [`cell_seen`]), the
/// components are seen ([`part_weight`]), the build fills the frame (its projected box over the
/// window's), and the inputs are on the camera's side. A side from which every input can be
/// tapped at its centre comes first, then one from which each can be tapped somewhere (`web
/// click`); ties go to the earlier direction (the pictures' first). Deterministic: scores are
/// compared at a thousandth.
pub fn best_camera(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32), pts: &[[f32; 3]]) -> Camera {
    let list = camera_candidates(atlas, s, look, aspect, window, pts);
    let mut best: Option<&CameraCandidate> = None;
    for c in &list {
        if best.is_none_or(|b| c.key > b.key) {
            best = Some(c);
        }
    }
    best.map(|c| c.cam).unwrap_or_else(|| view_camera_at(s, look, pts, aspect, CAM_DIR, window))
}

/// One of [`best_camera`]'s candidates: its direction, its camera, the key it is ranked by (the
/// dials seen, the inputs' tier, the score in thousandths) and its score's parts.
#[derive(Clone, Debug)]
pub struct CameraCandidate {
    pub dir: [f32; 3],
    pub cam: Camera,
    pub key: (bool, u8, i64),
    pub parts: ScoreParts,
    /// The face-on cost (a bonus when negative) taken off the score.
    pub face_on: f32,
}

/// Every candidate [`best_camera`] weighs, in [`candidate_dirs`] order (`mctas web camera`
/// prints them).
pub fn camera_candidates(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32), pts: &[[f32; 3]]) -> Vec<CameraCandidate> {
    let mut cs = CameraSearch::new(atlas, s, look, aspect, window, pts);
    while !cs.step(s, look, &mut || false) {}
    cs.out
}

/// [`camera_candidates`] over several calls ([`CameraSearch::step`], a few directions each): a
/// big farm's camera search took 80-90 ms in one task in a software GL (the coordinator: no
/// main-thread task over ~50 ms). The same candidates in the same order, so the same camera.
pub struct CameraSearch {
    aspect: f32,
    window: (f32, f32, f32),
    /// The framed points (for the fallback camera) and those of them that can be extreme.
    all: Vec<[f32; 3]>,
    shell: Vec<[f32; 3]>,
    picker: crate::pick::Picker,
    tg: Targets,
    tap: bool,
    face_way: Option<[f32; 3]>,
    dial_way: Option<[f32; 3]>,
    thick: (f32, f32, f32),
    dirs: Vec<[f32; 3]>,
    next: usize,
    out: Vec<CameraCandidate>,
}

impl CameraSearch {
    pub fn new(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32), pts: &[[f32; 3]]) -> CameraSearch {
        // every candidate fits and scores the framed points by their projected extremes: the
        // points that can be extreme, once for all of them (the same cameras and scores, exactly)
        let shell = crate::camera::hull_candidates(pts);
        let blocks = s.render_blocks(0.0);
        let picker = crate::pick::Picker::new(atlas, &blocks);
        let tg = targets(s, look, &shell);
        let tap = s.simulated() && (!s.inputs.is_empty() || !s.swaps.is_empty());
        // the way a lock's dials face (the frames one turns, not its code's), if it has any
        // the way a display's face faces (`circuit::display_face`), if it has one
        let face_way = crate::circuit::display_face(&s.design);
        let dial_way: Option<[f32; 3]> = {
            let sum = tg.frames.iter().filter(|f| f.2 >= 1.0).fold([0.0f32; 3], |a, f| [a[0] + f.1[0], a[1] + f.1[1], a[2] + f.1[2]]);
            let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            (len > 1e-3).then(|| sum.map(|v| v / len))
        };
        // the build's thickness along x and z (a face-on view suits a one-block-thick wall only):
        // of every block drawn as it stands, the judge's chests too (a sorter's PASS chest sticks
        // out of its one-block line), not the judge's see-through rig
        let drawn: Vec<Pos> = blocks.into_iter().filter(|b| b.offset == [0.0; 3] && !look.ghost.contains(&b.cell)).map(|b| b.cell).collect();
        let ext = |k: usize| {
            let lo = drawn.iter().map(|p| p[k]).min().unwrap_or(0);
            let hi = drawn.iter().map(|p| p[k]).max().unwrap_or(0);
            (hi - lo + 1) as f32
        };
        let thick = (ext(0), ext(1), ext(2));
        // a TNT duper on its judge's ground: the pictures' side-on direction alone, the duper in
        // front and its landing zone beyond it (critique on tnt-clocked-22b: "frame the duper plus
        // its landing zone from the side-ish angle used before"; from elsewhere the zone came in
        // front and left the duper small in a corner of a phone's view)
        // a display's face: one direction, from the front, a little above and to the side (the
        // coordinator 2026-10-22: "its camera looks at the face from the front, slightly above and
        // to the side, so the digit reads"; the 29-degree candidates showed its digit too
        // obliquely)
        let dirs: Vec<[f32; 3]> = if let Some(f) = face_way {
            vec![face_front_dir(s, f)]
        } else if s.floor.is_some() {
            candidate_dirs().into_iter().take(1).collect()
        } else {
            candidate_dirs()
        };
        CameraSearch { aspect, window, all: pts.to_vec(), shell, picker, tg, tap, face_way, dial_way, thick, dirs, next: 0, out: Vec::new() }
    }

    /// Weigh candidates until all are weighed (true) or `out_of_time` (asked after each).
    pub fn step(&mut self, s: &Session, look: &Look, out_of_time: &mut dyn FnMut() -> bool) -> bool {
        let (aspect, window) = (self.aspect, self.window);
        let (thick_x, thick_y, thick_z) = self.thick;
        // a wall: one block thick along x (or z), at least 3 by 3 and 12 cells across its face (a
        // small thin build, a NOT gate or a BUD switch, reads better from three sides)
        let wall = |along_x: bool| {
            let (thick, wide) = if along_x { (thick_x, thick_z) } else { (thick_z, thick_x) };
            thick <= 1.01 && wide >= 3.0 && thick_y >= 3.0 && wide * thick_y >= 12.0
        };
        while self.next < self.dirs.len() {
            let k = self.next;
            let dir = self.dirs[k];
            self.next += 1;
            let (picker, tg, pts) = (&self.picker, &self.tg, &self.shell[..]);
            let cam = view_camera_at(s, look, pts, aspect, dir, window);
            // tappable inputs first (a side every input's centre can be tapped from, then one each
            // can be tapped somewhere from)
            let tier = if !self.tap {
                2
            } else if tg.inputs.iter().all(|c| click_point_with(picker, &cam, *c, aspect, &[0.5]).is_some()) {
                2
            } else if tg.inputs.iter().all(|c| click_point_with(picker, &cam, *c, aspect, &[0.5, 0.3, 0.7, 0.15, 0.85]).is_some()) {
                1
            } else {
                0
            };
            // a face-on candidate (looking along x or z): preferred for a one-block-thick wall
            // seen from its face (its pattern reads like a drawing), at a cost otherwise
            let face_on = if k >= 16 {
                let along_x = dir[0].abs() > dir[2].abs();
                if wall(along_x) {
                    -FACE_ON_BONUS
                } else {
                    // (a long build seen end-on from low: its near end hides the rest, END_ON_COST)
                    let (depth, width) = if along_x { (thick_x, thick_z) } else { (thick_z, thick_x) };
                    FACE_ON_COST + if depth >= END_ON_RATIO * width { END_ON_COST } else { 0.0 }
                }
            } else {
                0.0
            };
            let parts = score_parts(picker, &cam, aspect, window, pts, tg);
            let score = parts.total() + if k == 0 { 0.01 } else { 0.0 } - face_on;
            // a lock's dials seen from the side they face, first (critique on lock3-q85: "the
            // dials show as thin brown frames seen edge-on"; lock2-hid's eye came 70 degrees off
            // them)
            let dials_seen = self.dial_way.is_none_or(|w| w[0] * dir[0] + w[1] * dir[1] + w[2] * dir[2] >= DIALS_SEEN);
            // a display's face read from the front, first, before its input can be tapped (the
            // coordinator 2026-10-22: the 0-9 display seen from its input chest's side, its digit
            // unreadable; its input chest sits behind the decoder)
            let face_seen = self.face_way.is_none_or(|w| w[0] * dir[0] + w[1] * dir[1] + w[2] * dir[2] >= FACE_SEEN);
            let dials_seen = dials_seen && face_seen;
            let key = (dials_seen, tier, (score * 1000.0).round() as i64);
            self.out.push(CameraCandidate { dir, cam, key, parts, face_on });
            if self.next < self.dirs.len() && out_of_time() {
                return false;
            }
        }
        true
    }

    /// [`best_camera`]'s pick once every candidate is weighed.
    pub fn best(&self, s: &Session, look: &Look) -> Camera {
        let mut best: Option<&CameraCandidate> = None;
        for c in &self.out {
            if best.is_none_or(|b| c.key > b.key) {
                best = Some(c);
            }
        }
        best.map(|c| c.cam).unwrap_or_else(|| view_camera_at(s, look, &self.all, self.aspect, CAM_DIR, self.window))
    }
}

/// [`live_camera`] over several calls ([`LiveCameraSearch::step`]): the same camera and window.
pub struct LiveCameraSearch {
    w: f32,
    h: f32,
    window: (f32, f32, f32),
    search: CameraSearch,
    /// On a phone, the search again in the window raised above the Record pill.
    raised: bool,
}

impl LiveCameraSearch {
    pub fn new(atlas: &Atlas, s: &Session, look: &Look, w: f32, h: f32) -> LiveCameraSearch {
        let aspect = w / h.max(1.0);
        let window = live_window_css(w, h);
        let search = CameraSearch::new(atlas, s, look, aspect, window, &view_points_for(s, look, aspect));
        LiveCameraSearch { w, h, window, search, raised: false }
    }

    /// Search on; the camera and its window once found.
    pub fn step(&mut self, atlas: &Atlas, s: &Session, look: &Look, out_of_time: &mut dyn FnMut() -> bool) -> Option<(Camera, (f32, f32, f32))> {
        if !self.search.step(s, look, out_of_time) {
            return None;
        }
        let cam = self.search.best(s, look);
        if self.raised || self.w >= NARROW_PX {
            return Some((cam, self.window));
        }
        let (w, h) = (self.w, self.h);
        let aspect = w / h.max(1.0);
        // the pill's box in view coordinates (x right, y up, -1..1)
        let (right, pw, bottom, ph) = PILL;
        let px0 = 1.0 - 2.0 * (right + pw + 4.0) / w;
        let py1 = -1.0 + 2.0 * (bottom + ph + 4.0) / h;
        let (f, r, u) = cam.basis();
        let eye = cam.eye();
        let t = (cam.vfov.to_radians() / 2.0).tan();
        let covered = view_points_for(s, look, aspect).iter().any(|p| {
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f).max(0.01);
            let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
            x > px0 && y < py1
        });
        if !covered {
            return Some((cam, self.window));
        }
        let window = self.window;
        let raised = (window.0, (-1.0 + 2.0 * (bottom + ph + 10.0) / h.max(120.0)).min(window.2 - 0.4), window.2);
        self.window = raised;
        self.raised = true;
        self.search = CameraSearch::new(atlas, s, look, aspect, raised, &view_points_for(s, look, aspect));
        if out_of_time() {
            return None;
        }
        self.step(atlas, s, look, out_of_time)
    }
}

/// How far a default camera may look off a lock's dials: the cosine of its direction (target to
/// eye) and the way the dials face, at least this (about 65 degrees off their faces).
pub const DIALS_SEEN: f32 = 0.42;

/// How far a default camera may look off a display's face (`circuit::display_face`): the cosine
/// of its direction and the way the face faces, at least this (about 45 degrees: from the front,
/// a little above and to the side, so the digit reads).
pub const FACE_SEEN: f32 = 0.7;

/// How far a display's camera turns off its face's normal, sideways and up (degrees).
pub const FACE_YAW: f32 = 20.0;
pub const FACE_PITCH: f32 = 18.0;

/// A display's camera direction (target to eye): its face's normal `f` (horizontal) turned
/// [`FACE_YAW`] toward the side the face sits on in the build (the decoder then shows behind the
/// face, not across it) and [`FACE_PITCH`] up.
fn face_front_dir(s: &Session, f: [f32; 3]) -> [f32; 3] {
    let right = [f[2], 0.0, -f[0]];
    // the face's middle against the build's middle, along `right`
    let outs: Vec<Pos> = s.design.ports.iter().filter(|(n, _)| n.starts_with("output")).map(|(_, (p, _))| *p).collect();
    let mid = |cells: &mut dyn Iterator<Item = [f32; 3]>| -> f32 {
        let (mut sum, mut n) = (0.0f32, 0u32);
        for c in cells {
            sum += c[0] * right[0] + c[2] * right[2];
            n += 1;
        }
        if n == 0 { 0.0 } else { sum / n as f32 }
    };
    let face = mid(&mut outs.iter().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]));
    let build = mid(&mut s.design.blocks.keys().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]));
    let side = if face >= build { 1.0 } else { -1.0 };
    let (y, p) = (FACE_YAW.to_radians(), FACE_PITCH.to_radians());
    [
        p.cos() * (y.cos() * f[0] + side * y.sin() * right[0]),
        p.sin(),
        p.cos() * (y.cos() * f[2] + side * y.sin() * right[2]),
    ]
}

/// A candidate camera's score, part by part ([`best_camera`]: 3 x the ports seen + 2 x the
/// components seen + 2 x how much of the window the build fills + 0.75 x how far the inputs are
/// on the camera's side, and its costs; `mctas web camera` prints them).
#[derive(Clone, Copy, Debug, Default)]
pub struct ScoreParts {
    pub ports: f32,
    pub parts: f32,
    pub fill: f32,
    pub side: f32,
    pub high: f32,
    pub lean: f32,
    pub stacked: f32,
    pub hidden: f32,
    pub unseen: f32,
    pub facing: f32,
}

impl ScoreParts {
    pub fn total(&self) -> f32 {
        3.0 * self.ports + 2.0 * self.parts + 2.0 * self.fill + 0.75 * self.side - self.high + 0.5 * self.lean - self.stacked.min(1.5) - self.hidden.min(1.0) - 2.0 * self.unseen + 6.0 * self.facing
    }
}

fn score_parts(picker: &crate::pick::Picker, cam: &Camera, aspect: f32, window: (f32, f32, f32), pts: &[[f32; 3]], tg: &Targets) -> ScoreParts {
    let share = |list: &[(Pos, f32)]| -> f32 {
        let total: f32 = list.iter().map(|(_, w)| w).sum();
        if total <= 0.0 {
            return 1.0;
        }
        list.iter().map(|(c, w)| w * cell_seen(picker, cam, aspect, *c)).sum::<f32>() / total
    };
    let ports = share(&tg.ports);
    let parts = share(&tg.parts);
    // a port (an input, an output, a tagged chest, a labelled container: weight 1.5 or more) seen
    // less than half: a cost on top of its share (critique: a sorter's PASS chest mostly hidden
    // behind its ITEMS chest, its filter hopper before that; a fuller frame is no excuse), 2 per
    // output-weight port wholly hidden, 4 at most
    let unseen: f32 = tg.ports.iter().filter(|(_, w)| *w >= 1.5).map(|(c, w)| (w / 3.0) * ((0.5 - cell_seen(picker, cam, aspect, *c)) / 0.5).max(0.0)).sum::<f32>().min(2.0);
    // how much of the window the framed points fill (the view's shape suits the window's: a
    // latch's two inputs side by side across a wide view, not one behind the other), and how big
    // the build itself comes out (its own cells, the camera framing everything else too: a TNT
    // duper's landing zone, an extender's travel; a duper from the side rather than from above)
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let area = (2.0 * window.0) * (window.2 - window.1);
    let fill_of = |list: &[[f32; 3]]| -> f32 {
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in list {
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f).max(0.01);
            let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        if list.is_empty() || area <= 0.0 { 0.0 } else { ((x1 - x0).max(0.0) * (y1 - y0).max(0.0) / area).min(1.0) }
    };
    let fill = (fill_of(pts) + fill_of(&tg.build)) / 2.0;
    // the inputs on the camera's side of the build (in plan)
    let d = cam.dir();
    let side = if tg.inputs.is_empty() {
        0.5
    } else {
        tg.inputs
            .iter()
            .map(|c| {
                let (dx, dz) = (c[0] as f32 + 0.5 - tg.middle[0], c[2] as f32 + 0.5 - tg.middle[2]);
                let (l, h) = ((dx * dx + dz * dz).sqrt(), (d[0] * d[0] + d[2] * d[2]).sqrt());
                if l < 1e-3 || h < 1e-3 { 0.5 } else { ((dx * d[0] + dz * d[2]) / (l * h) + 1.0) / 2.0 }
            })
            .sum::<f32>()
            / tg.inputs.len() as f32
    };
    // the pictures' elevation unless the higher one shows more (from 45 degrees a wide ground or a
    // flat build reads worse)
    let high = if cam.pitch > 0.6 { 0.2 } else { 0.0 };
    // a build without inputs whose output lies far from its middle: that end toward the camera
    // (critique: a long ring of droppers seen with its working end, OUT and all, far away)
    let lean = if tg.inputs.is_empty() && !tg.outputs.is_empty() {
        tg.outputs
            .iter()
            .map(|c| {
                let (dx, dz) = (c[0] as f32 + 0.5 - tg.middle[0], c[2] as f32 + 0.5 - tg.middle[2]);
                let (l, h) = ((dx * dx + dz * dz).sqrt(), (d[0] * d[0] + d[2] * d[2]).sqrt());
                if l < 1e-3 || h < 1e-3 { 0.0 } else { (dx * d[0] + dz * d[2]) / (l * h) * ((l - 2.0) / 4.0).clamp(0.0, 1.0) }
            })
            .sum::<f32>()
            / tg.outputs.len() as f32
    } else {
        0.0
    };
    // two tags hanging at the same spot of the view stack one on the other (critique: a gate's
    // OUT tag sat on its A tag, A's cell right behind OUT's): 0.5 a pair, 1.5 at most (in units
    // of half the view's height: a tag about 0.08 tall and 0.16 wide in a 480 px viewer)
    let mut stacked = 0.0f32;
    let proj: Vec<(f32, f32)> = tg
        .anchors
        .iter()
        .map(|p| {
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f).max(0.01);
            (dot(r) / (z * t), dot(u) / (z * t))
        })
        .collect();
    for i in 0..proj.len() {
        for j in i + 1..proj.len() {
            if (proj[i].0 - proj[j].0).abs() < 0.16 && (proj[i].1 - proj[j].1).abs() < 0.08 {
                stacked += 0.5;
            }
        }
    }
    // a thumbnail's port cells on the same spot of the picture (one right behind the other): 0.5
    // a pair, 1 at most (closer than 0.6 of a block's size there)
    let cells: Vec<(f32, f32, f32)> = tg
        .centers
        .iter()
        .map(|p| {
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f).max(0.01);
            (dot(r) / (z * t), dot(u) / (z * t), 1.0 / (z * t))
        })
        .collect();
    let mut hidden = 0.0f32;
    for i in 0..cells.len() {
        for j in i + 1..cells.len() {
            let (dx, dy) = (cells[i].0 - cells[j].0, cells[i].1 - cells[j].1);
            if (dx * dx + dy * dy).sqrt() < 0.6 * (cells[i].2 + cells[j].2) / 2.0 {
                hidden += 0.5;
            }
        }
    }
    // item frames a person turns, seen from the side they face (critique: a lock's dials hung
    // facing away; their arrows are the combination)
    let fw: f32 = tg.frames.iter().map(|f| f.2).sum();
    let facing = if fw > 0.0 {
        tg.frames
            .iter()
            .map(|(p, d, w)| {
                let v = [eye[0] - p[0], eye[1] - p[1], eye[2] - p[2]];
                let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
                w * ((v[0] * d[0] + v[1] * d[1] + v[2] * d[2]) / n).max(0.0)
            })
            .sum::<f32>()
            / fw
    } else {
        0.0
    };
    ScoreParts { ports, parts, fill, side, high, lean, stacked, hidden, unseen, facing }
}

/// The order a live viewer's input buttons go in: the inputs left to right as `cam` shows their
/// cells (critique 41f1ec62a: the AND gate's A tag on the right, B on the left, the reverse of
/// its A, B buttons). Ties keep the design's order, and so do inputs spread more up and down the
/// view than across it (a lock's dials in a column: D1, D2, D3).
pub fn inputs_left_to_right(s: &Session, cam: &Camera) -> Vec<usize> {
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let screen = |c: Pos| {
        let v = [c[0] as f32 + 0.5 - eye[0], c[1] as f32 + 0.5 - eye[1], c[2] as f32 + 0.5 - eye[2]];
        let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        let z = dot(f).max(0.01);
        (dot(r) / z, dot(u) / z)
    };
    let pts: Vec<(f32, f32)> = s.circuit_inputs.iter().map(|ci| screen(ci.pos)).collect();
    let spread = |k: fn(&(f32, f32)) -> f32| pts.iter().map(k).fold(f32::MIN, f32::max) - pts.iter().map(k).fold(f32::MAX, f32::min);
    if pts.len() < 2 || spread(|p| p.0) < spread(|p| p.1) {
        return (0..pts.len()).collect();
    }
    let mut v: Vec<(usize, f32)> = pts.iter().enumerate().map(|(i, p)| (i, p.0)).collect();
    v.sort_by(|a, b| if (a.1 - b.1).abs() < 1e-3 { a.0.cmp(&b.0) } else { a.1.total_cmp(&b.1) });
    v.into_iter().map(|(i, _)| i).collect()
}

/// Where a ray (origin, direction) enters a cell's box, if it does (the slab test).
fn cell_entry(o: [f32; 3], d: [f32; 3], c: Pos) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        let (lo, hi) = (c[i] as f32, c[i] as f32 + 1.0);
        if d[i].abs() < 1e-9 {
            if o[i] < lo || o[i] > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o[i]) / d[i], (hi - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Options;

    /// The crater a TNT duper blows in its judge's ground is lit like the open air (vjeux
    /// 2026-10-21, "the bedrock texture no longer works correctly": since the game's light on
    /// every face, the light counted every cell under the ground's top as ground, and the crater's
    /// walls and bedrock bottom were drawn black): after tnt-clocked-22b's first blasts, the top
    /// faces at the bottom of its crater are bright.
    #[test]
    fn a_tnt_crater_is_lit() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/tnt/archive/tnt-clocked-22b.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        let blown = |s: &Session| -> Vec<Pos> { s.floor.as_ref().map(|f| f.changed.keys().copied().filter(|c| f.shown(*c).is_none()).collect()).unwrap_or_default() };
        while blown(&s).is_empty() && s.tick < 2000 {
            s.step();
        }
        let holes = blown(&s);
        assert!(!holes.is_empty(), "a crater by tick {}", s.tick);
        let f = s.floor.as_ref().unwrap();
        // the crater's floor: a blown cell over a block still there
        let bottoms: Vec<Pos> = holes.iter().copied().filter(|c| f.contains([c[0], c[1] - 1, c[2]]) && f.shown([c[0], c[1] - 1, c[2]]).is_some()).collect();
        assert!(!bottoms.is_empty(), "{holes:?}");
        let look = viewer_look(&s);
        let atlas = crate::atlas::Atlas::build();
        let m = build(&atlas, &s, 0.0, &look);
        let mut lum = Vec::new();
        for v in m.solid.iter() {
            if bottoms.iter().any(|c| v.pos[1] == c[1] as f32 && v.pos[0] >= c[0] as f32 && v.pos[0] <= c[0] as f32 + 1.0 && v.pos[2] >= c[2] as f32 && v.pos[2] <= c[2] as f32 + 1.0) {
                lum.push(v.color[0] as f32);
            }
        }
        assert!(!lum.is_empty(), "the crater's floor drawn");
        let mean = lum.iter().sum::<f32>() / lum.len() as f32;
        assert!(mean > 120.0, "the crater's floor lit: mean {mean} over {} corners", lum.len());
    }

    /// A blast's particles are the game's (vjeux 2026-10-21, "the particules effect for the tnt
    /// explosion looks broken": nine grey cubes): camera-facing squares of the explosion sprite
    /// (`particles`), 6 more each tick of the emitter, turned to whatever camera draws them.
    #[test]
    fn a_blast_draws_the_games_particles() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/tnt/archive/tnt-clocked-22b.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        while s.world.explosions.is_empty() && s.tick < 2000 {
            s.step();
        }
        assert!(!s.world.explosions.is_empty(), "a blast by tick {}", s.tick);
        // three ticks after it: the emitter's first three ticks of puffs
        for _ in 0..3 {
            s.step();
        }
        let look = viewer_look(&s);
        let atlas = crate::atlas::Atlas::build();
        let m = build(&atlas, &s, 0.0, &look);
        assert!(m.sprites.len() >= 12 && m.sprites.len() <= 18, "{} puffs", m.sprites.len());
        assert!(m.sprites.iter().all(|p| p.layer == atlas.tile("explosion_puff") as u32), "the hand-drawn puff without the game's sprites");
        for cam in [default_camera(&s, &look, 16.0 / 9.0), Camera { yaw: 1.1, pitch: -0.7, ..default_camera(&s, &look, 1.0) }] {
            let (f, _, _) = cam.basis();
            let (solid, _) = triangles(&m, &cam);
            let quads = &solid[m.solid.len()..];
            assert_eq!(quads.len(), m.sprites.len() * 6);
            for (q, sp) in quads.chunks(6).zip(&m.sprites) {
                for v in q {
                    let d = [v.pos[0] - sp.pos[0], v.pos[1] - sp.pos[1], v.pos[2] - sp.pos[2]];
                    let along = d[0] * f[0] + d[1] * f[1] + d[2] * f[2];
                    assert!(along.abs() < 1e-3, "a corner off the camera's plane by {along}");
                }
            }
        }
    }

    /// An input whose centre hides behind another block from the pictures' side (here A behind
    /// the judge's lamp in the output cell in front of it) makes the camera start on a side every
    /// input's centre can be tapped from.
    /// Critique on an R7 gate: the judge's switch (a red block) and repeater are the test rig,
    /// not the build: they are drawn see-through (redsim's blocks there all the same), and the
    /// camera frames each input's whole lime box (one ran off the edge).
    #[test]
    fn the_judges_rig_is_ghosted_and_framed() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/gates/archive/gate-and-5b-r7.txt")).unwrap();
        let s = Session::new(&text, &Options::default()).unwrap();
        let look = viewer_look(&s);
        let driven: Vec<_> = s.circuit_inputs.iter().filter_map(|ci| if let crate::circuit::InputHow::Driven { dir, .. } = ci.how { Some(crate::circuit::driver_cells(ci.pos, dir)) } else { None }).collect();
        assert_eq!(driven.len(), 2, "two driven inputs");
        for (r, sup, sw) in &driven {
            for c in [r, sup, sw] {
                assert!(look.ghost.contains(c), "{c:?} ghosted");
            }
        }
        assert!(s.circuit_inputs.iter().all(|ci| !look.ghost.contains(&ci.pos)), "the input cells stay solid");
        let pts = view_points(&s, &look);
        for (lo, hi) in &look.input_spans {
            for corner in [[lo[0] as f32, lo[1] as f32, lo[2] as f32], [hi[0] as f32 + 1.0, hi[1] as f32 + 1.0, hi[2] as f32 + 1.0]] {
                assert!(pts.contains(&corner), "{corner:?} framed");
            }
        }
        // ghosted faces go to the translucent pass, see-through
        let atlas = crate::atlas::Atlas::build();
        let m = build(&atlas, &s, 0.0, &look);
        assert!(m.translucent.iter().any(|q| q.v[0].color[3] == mesh::GHOST_ALPHA));
    }

    /// Critique of the first heuristic (94f0b1b81). A one-block-thick wall (an extender built
    /// flat, pext-n3-flat-cascade16: "flat 1-wide builds need the face-on view") is seen face-on;
    /// the TNT duper (tnt-clocked-22b) from the pictures' side, its pit framed at the ground's
    /// surface (not looking down at a field of grass), a phone's view keeping only the near part
    /// of its landing zone; a thumbnail frames the gate, not the judge's see-through rig.
    #[test]
    fn walls_face_on_dupers_side_on() {
        let read = |f: &str| std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let atlas = crate::atlas::Atlas::build();
        let aspect = 1232.0 / 480.0;
        // the wall: looking along z (its normal), from the pictures' elevation
        let s = Session::new(&read("circuits/pext/archive/pext-n3-flat-cascade16-9x5"), &Options::default()).unwrap();
        let look = viewer_look(&s);
        let cam = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW);
        let d = cam.dir();
        assert!(d[0].abs() < 0.02 && d[2].abs() > 0.8, "face-on: {d:?}");
        // a 3D build never gets a face-on view (gate-xor)
        let s = Session::new(&read("circuits/gates/archive/gate-xor"), &Options::default()).unwrap();
        let look = viewer_look(&s);
        let d = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW).dir();
        assert!(d[0].abs() > 0.3 && d[2].abs() > 0.3, "three faces: {d:?}");
        // the TNT duper: the pictures' own direction on a desktop and on a phone
        let text = read("circuits/tnt/archive/tnt-clocked-22b");
        let s = Session::new(&text, &Options::default()).unwrap();
        let look = viewer_look(&s);
        let top = s.floor.as_ref().unwrap().hi[1] as f32 + 1.0;
        assert!(view_points(&s, &look).iter().all(|p| p[1] >= top - 1.0), "nothing framed below the ground's surface but the build");
        let far = |pts: &[[f32; 3]]| pts.iter().map(|p| p[2].abs().max(p[0].abs())).fold(0.0f32, f32::max);
        assert!(far(&view_points_for(&s, &look, 0.7)) < far(&view_points_for(&s, &look, 2.5)), "a phone keeps less of the landing zone");
        let want = candidate_dirs()[0];
        for (w, h) in [(1232.0, 480.0), (327.0, 480.0)] {
            let (cam, _) = live_camera(&atlas, &s, &look, w, h);
            let d = cam.dir();
            assert!((0..3).all(|k| (d[k] - want[k]).abs() < 1e-3), "{w}x{h}: {d:?}");
        }
        // the gate's thumbnail: not framed on the rig
        let s = Session::new(&read("circuits/gates/archive/gate-and-5b-r7"), &Options::default()).unwrap();
        let look = viewer_look(&s);
        assert!(!look.ghost.is_empty());
        let still = still_points(&s, &look);
        for g in &look.ghost {
            let inside = still.iter().any(|p| (0..3).all(|k| p[k] > g[k] as f32 + 0.01 && p[k] < g[k] as f32 + 0.99));
            assert!(!inside, "{g:?}");
        }
    }

    /// Critique 205ae4e74. A gate's tags never hang on one spot (its OUT tag sat on its A tag): in
    /// the phone's live view the AND gate's tag anchors stand apart. A BUD switch's thumbnail
    /// outlines its TARGET (nothing else), frames it with the observer, and keeps the two apart
    /// in the picture (not one right behind the other).
    /// Coordinator (vjeux 04:41Z, the precise drop): "Camera should frame machine + burst
    /// point": a duper standing high over its ground frames its TNT's fall under it down to where
    /// they go off (10 blocks under the machine), on a phone too; not the ground 23 blocks down.
    #[test]
    fn a_precise_drop_frames_its_burst_point() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/tnt/archive/tnt-precise-drop.txt")).unwrap();
        let s = Session::new(&text, &Options::default()).unwrap();
        let look = viewer_look(&s);
        let f = s.floor.as_ref().unwrap();
        let burst = f.design_box.0[1] as f32 - crate::session::burst_below(&text).unwrap();
        for aspect in [0.7, 2.5] {
            let low = view_points_for(&s, &look, aspect).iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            assert!(low <= burst + 1.0 && low > f.hi[1] as f32 + 1.0, "aspect {aspect}: framed down to y {low}, burst at {burst}, ground at {}", f.hi[1] + 1);
        }
    }

    /// QA at phone size: a flying machine followed by the camera over the void seemed parked
    /// (its pistons pumping, nothing passing by). The live view lays a track under its whole path
    /// (behind it for one flying up or down), a tie at every block; thumbnails and anything that
    /// does not travel have none.
    #[test]
    fn a_flying_machine_runs_on_a_track() {
        let read = |f: &str| std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let atlas = crate::atlas::Atlas::build();
        for (f, axis, normal) in [("circuits/fly/archive/fly-horizontal-6b-10gt-01ce95", 2, 1), ("circuits/fly/archive/fly-up-6b-9gt-0e4df1", 1, 2)] {
            let s = Session::new(&read(f), &Options::default()).unwrap();
            let mut look = viewer_look(&s);
            let (lo, hi, a, n) = look.track.unwrap_or_else(|| panic!("{f}: a track"));
            assert_eq!((a, n), (axis, normal), "{f}");
            assert!(hi[axis] - lo[axis] > 20, "{f}: its whole path ({lo:?}..{hi:?})");
            let with = build(&atlas, &s, 0.0, &look).solid.len();
            thumbnail_look(&s, &mut look);
            assert!(look.track.is_none(), "{f}: no track on its thumbnail");
            let without = build(&atlas, &s, 0.0, &look).solid.len();
            assert!(with > without, "{f}: the track drawn ({with} vs {without} solid vertices)");
        }
        let gate = Session::new(&read("circuits/gates/archive/gate-and-5b-r7"), &Options::default()).unwrap();
        assert!(viewer_look(&gate).track.is_none());
        let pext = Session::new(&read("circuits/pext/archive/pext-n2-rider7-flat"), &Options::default()).unwrap();
        assert!(viewer_look(&pext).track.is_none(), "an extender comes back: no track");
    }

    #[test]
    fn tags_and_ports_stand_apart() {
        let read = |f: &str| std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let atlas = crate::atlas::Atlas::build();
        let project = |cam: &Camera, aspect: f32, p: [f32; 3]| -> (f32, f32, f32) {
            let (f, r, u) = cam.basis();
            let eye = cam.eye();
            let t = (cam.vfov.to_radians() / 2.0).tan();
            let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f);
            let _ = aspect;
            (dot(r) / (z * t), dot(u) / (z * t), 1.0 / (z * t))
        };
        let s = Session::new(&read("circuits/gates/archive/gate-and-5b-r7"), &Options::default()).unwrap();
        let look = viewer_look(&s);
        let (cam, _) = live_camera(&atlas, &s, &look, 399.0, 480.0);
        let anchors: Vec<(f32, f32, f32)> = tag_anchors(&s).into_iter().map(|p| project(&cam, 399.0 / 480.0, p)).collect();
        assert_eq!(anchors.len(), 3, "A, B, OUT");
        for i in 0..anchors.len() {
            for j in i + 1..anchors.len() {
                let (a, b) = (anchors[i], anchors[j]);
                assert!((a.0 - b.0).abs() >= 0.16 || (a.1 - b.1).abs() >= 0.08, "tags {i} and {j} on one spot: {a:?} {b:?}");
            }
        }
        // the bar's buttons go left to right as the view shows the inputs (critique 41f1ec62a: the
        // AND gate's A tag on the right, its A button on the left)
        let a = project(&cam, 1.0, [s.circuit_inputs[0].pos[0] as f32 + 0.5, s.circuit_inputs[0].pos[1] as f32 + 0.5, s.circuit_inputs[0].pos[2] as f32 + 0.5]);
        let b = project(&cam, 1.0, [s.circuit_inputs[1].pos[0] as f32 + 0.5, s.circuit_inputs[1].pos[1] as f32 + 0.5, s.circuit_inputs[1].pos[2] as f32 + 0.5]);
        let order = inputs_left_to_right(&s, &cam);
        assert_eq!(order, if a.0 < b.0 { vec![0, 1] } else { vec![1, 0] }, "A {a:?} B {b:?}");
        // the sorter's PASS chest seen (critique 41f1ec62a: mostly hidden behind ITEMS)
        let s = Session::new(&read("circuits/sorter/archive/sorter-classic"), &Options::default()).unwrap();
        let look = viewer_look(&s);
        let (cam, _) = live_camera(&atlas, &s, &look, 399.0, 480.0);
        let picker = crate::pick::Picker::new(&atlas, &s.render_blocks(0.0));
        let pass = s.design.ports["pass"].0;
        assert!(cell_seen(&picker, &cam, 399.0 / 480.0, pass) >= 0.5, "PASS seen");
        // a lock's dials seen from the side they face (critique on lock3-q85: "the dials show as
        // thin brown frames seen edge-on"), on a phone and a desktop (the study's archived locks:
        // the eye on the dials' side, at most ~65 degrees off their faces)
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/lock/archive");
        let mut locks: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        locks.sort();
        for path in &locks {
            let text = std::fs::read_to_string(path).unwrap();
            if !crate::session::tests::is_frame_lock(&text) {
                continue;
            }
            let s = Session::new(&text, &Options::default()).unwrap();
            let d0 = s.circuit_inputs.iter().find(|c| c.name == "d0").unwrap().pos;
            let facing = s.world.frames.iter().find(|f| f.pos == bp(d0)).unwrap().facing.step();
            let look = viewer_look(&s);
            for (w, h) in [(399.0, 480.0), (1232.0, 480.0)] {
                let (cam, _) = live_camera(&atlas, &s, &look, w, h);
                let d = cam.dir();
                let toward = d[0] * facing[0] as f32 + d[1] * facing[1] as f32 + d[2] * facing[2] as f32;
                assert!(toward > 0.35, "{}: {w}x{h}: the dials face {facing:?}, the eye looks from {d:?}", path.display());
            }
        }
        // a lit lamp gives off light: the game lights it flat, its faces still shaded by
        // direction (drawn unshaded, its sides were brighter than the real game's)
        assert!(crate::light::emission("minecraft:redstone_lamp[lit=true]") == 15 && crate::light::emission("minecraft:redstone_lamp[lit=false]") == 0);
        let s = Session::new(&read("circuits/bud/archive/bud-observer"), &Options::default()).unwrap();
        let mut look = viewer_look(&s);
        thumbnail_look(&s, &mut look);
        assert!(look.outlines.is_empty() && look.input_spans.is_empty());
        assert_eq!(look.port_outlines.iter().map(|(p, _)| *p).collect::<Vec<_>>(), vec![[0, 1, 0]], "the TARGET alone");
        let still = still_points(&s, &look);
        assert!(still.iter().any(|p| p[0] < 0.5), "the TARGET framed");
        let aspect = 4.0 / 3.0;
        let cam = thumb_camera(&atlas, &s, &look, aspect);
        let (a, b) = (project(&cam, aspect, [1.5, 1.5, 0.5]), project(&cam, aspect, [0.5, 1.5, 0.5]));
        let (dx, dy) = (a.0 - b.0, a.1 - b.1);
        assert!((dx * dx + dy * dy).sqrt() >= 0.6 * (a.2 + b.2) / 2.0, "observer and TARGET apart: {a:?} {b:?}");
    }

    /// A long build is not seen end-on from low (the reviewer on clk-rt's cane-sweeper-10x24, 14×6×34,
    /// on a desktop window: its station's end wall filled the view, no cane and no machine): the
    /// face-on candidate along its 34-block length costs END_ON_COST and loses to a three-quarter
    /// view from above (a one-block-thick wall keeps its face-on bonus: the flat cascades).
    #[test]
    fn a_long_build_is_not_seen_end_on() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/farm/archive/cane-sweeper-10x24.txt")).unwrap();
        let s = Session::new(&text, &Options::default()).unwrap();
        let look = viewer_look(&s);
        let atlas = crate::atlas::Atlas::build();
        let (cam, _) = live_camera(&atlas, &s, &look, 1217.0, 480.0);
        assert!(cam.pitch.to_degrees() > 20.0, "seen from above, not face-on: yaw {} pitch {}", cam.yaw.to_degrees(), cam.pitch.to_degrees());
        let end_on = camera_candidates(&atlas, &s, &look, 1217.0 / 480.0, live_window_css(1217.0, 480.0), &view_points_for(&s, &look, 1217.0 / 480.0)).into_iter().find(|c| c.dir[0].abs() < 1e-3 && c.dir[2] > 0.9).expect("the face-on candidate along z");
        assert_eq!(end_on.face_on, FACE_ON_COST + END_ON_COST, "end-on along its length");
    }

    /// The coordinator 2026-10-22 on the 0-9 display: its view looked at it from its input
    /// chest's side, an OUT tag on each of its 13 lamps. Its face faces -z (its lamps stand in
    /// front of the decoder); the page's camera and the card's look at it from the front, a little
    /// above and to the side, and no lamp carries a tag or an outline.
    #[test]
    fn a_display_is_read_from_the_front() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/display/archive/seven-segment-decimal.txt")).unwrap();
        let s = Session::new(&text, &Options::default()).unwrap();
        assert_eq!(crate::circuit::display_face(&s.design), Some([0.0, 0.0, -1.0]));
        let look = viewer_look(&s);
        assert!(look.port_outlines.iter().all(|(p, _)| p[2] != -21), "no outline on the face's lamps");
        let atlas = crate::atlas::Atlas::build();
        for (w, h) in [(1217.0, 480.0), (342.0, 480.0)] {
            let (cam, _) = live_camera(&atlas, &s, &look, w, h);
            let dir = cam.dir();
            assert!(-dir[2] > 0.85 && dir[1] > 0.2, "{w}x{h}: from the front, a little above: {dir:?}");
        }
        let mut tl = viewer_look(&s);
        thumbnail_look(&s, &mut tl);
        let cam = thumb_camera(&atlas, &s, &tl, 286.0 / 215.0);
        assert!(-cam.dir()[2] > 0.85, "the card: from the front");
        // (a row of outputs is no face: the 13 lamps spread up and sideways)
        let mut row = s.design.clone();
        for (i, (_, (p, _))) in row.ports.iter_mut().filter(|(n, _)| n.starts_with("output")).enumerate() {
            *p = [i as i32, 8, -21];
        }
        assert_eq!(crate::circuit::display_face(&row), None);
    }

    /// vjeux 23:53Z: "some kind of heuristic to figure out the default camera position" (on
    /// gate-not, whose torch the pictures' side hid behind the input block). The default camera
    /// sees the NOT gate's input and its torch, and the same camera comes out every time.
    #[test]
    fn the_default_camera_sees_the_ports() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/gates/archive/gate-not.txt")).unwrap();
        let s = Session::new(&text, &Options::default()).unwrap();
        let look = viewer_look(&s);
        let atlas = crate::atlas::Atlas::build();
        let aspect = 1232.0 / 480.0;
        let cam = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW);
        let again = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW);
        assert_eq!((cam.yaw, cam.pitch, cam.dist), (again.yaw, again.pitch, again.dist), "deterministic");
        let picker = crate::pick::Picker::new(&atlas, &s.render_blocks(0.0));
        let out = s.outputs[0];
        assert!(s.design.blocks[&out].contains("redstone_wall_torch"));
        assert!(cell_seen(&picker, &cam, aspect, out) >= 0.5, "the torch (the output) is seen");
        for ci in &s.circuit_inputs {
            assert!(centre_point(&atlas, &s, &cam, ci.pos, aspect).is_some(), "the input can be tapped");
        }
        // the pictures' own side hides the torch behind the input block
        let pictures = view_camera(&s, &look, aspect, candidate_dirs()[0], LIVE_WINDOW);
        assert!(cell_seen(&picker, &pictures, aspect, out) < cell_seen(&picker, &cam, aspect, out));
        assert_eq!(candidate_dirs().len(), 20);
    }

    #[test]
    fn empty_input_cells_are_tappable() {
        let text = "name: t\ncircuit: gate and\nport input.a: 0 1 0\nport input.b: 1 1 0\nport output: 0 1 1\npalette:\n  S minecraft:stone\nlayer 0:\n  SS\n  S.\n";
        let s = Session::new(text, &Options::default()).unwrap();
        let atlas = crate::atlas::Atlas::build();
        let look = viewer_look(&s);
        let aspect = 1217.0 / 480.0;
        let cam = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW);
        assert!(s.inputs.iter().all(|c| centre_point(&atlas, &s, &cam, *c, aspect).is_some()), "every input's centre tappable from {cam:?}");
    }

    /// Every circuit port is outlined like its tag: the inputs lime (when the design runs), the
    /// outputs orange, a shulker design's ITEMS gold and BOXES lilac chests, a BUD switch's TARGET
    /// cyan; for every archived shulker, TNT and BUD design.
    #[test]
    fn every_port_is_outlined_like_its_tag() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits");
        let mut n = 0;
        for kind in ["shulker", "tnt", "bud"] {
            let mut files: Vec<_> = std::fs::read_dir(format!("{root}/{kind}/archive")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
            files.sort();
            for f in files {
                let s = Session::new(&std::fs::read_to_string(&f).unwrap(), &Options::default()).unwrap();
                let look = default_look(&s);
                for (label, p, colour) in crate::circuit::port_tags(&s.design) {
                    let found = look.port_outlines.iter().find(|(q, _)| *q == p).map(|(_, c)| *c);
                    let v = u32::from_str_radix(colour.trim_start_matches('#'), 16).unwrap();
                    assert_eq!(found, Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]), "{}: {label} at {p:?}", f.display());
                }
                for ci in &s.circuit_inputs {
                    // an input outlined (a driven one: in its span with the judge's devices)
                    let spanned = look.input_spans.iter().any(|(lo, hi)| (0..3).all(|k| ci.pos[k] >= lo[k] && ci.pos[k] <= hi[k]));
                    assert_eq!(look.outlines.contains(&ci.pos) || spanned, s.simulated(), "{}: input {} at {:?}", f.display(), ci.name, ci.pos);
                }
                n += 1;
            }
        }
        assert!(n >= 20, "{n} designs");
    }

    const OBS2: &str = "name: c\norigin: 0 0 0\nport output: 0 0 0\nport start: 1 0 0 last\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=west,powered=false]\nlayer 0:\n  AB\n";

    #[test]
    fn the_marker_never_shares_a_face_with_a_block() {
        let atlas = Atlas::build();
        let s = Session::new(OBS2, &Options::default()).unwrap();
        let look = default_look(&s);
        assert_eq!(look.marker, Some([-1, 0, 0]));
        let m = build(&atlas, &s, 0.0, &look);
        // the marker is the only translucent block: its face against the observer is culled (it
        // would z-fight); the floor is one block lower (the pictures' floor), so 5 faces remain
        assert_eq!(m.translucent.len(), 5);
        assert!(m.translucent.iter().all(|q| q.center[0] < -0.01));
    }
}
