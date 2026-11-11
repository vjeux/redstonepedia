//! A scene's triangles: block models placed in their cells (moving blocks at their offsets),
//! faces against opaque neighbors culled, corners darkened like the game's smooth lighting
//! (ambient occlusion), faces shaded by direction, translucent faces kept apart for sorting.

use crate::atlas::{Atlas, GAME_LAYER};
use crate::camera::FxBuild;
use crate::design::Pos;
use crate::models::{self, Face, Layer, Model, Part};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// Texture coordinates within the tile (0..1; beyond 1 repeats).
    pub uv: [f32; 2],
    /// The tile: a layer of the texture array.
    pub layer: u32,
    /// rgb = tint x shade x ambient occlusion, a = alpha (cutout faces are tested in the shader).
    pub color: [u8; 4],
    /// The game's light here, for the lightmap the frame applies ([`LIT`]: the day's own,
    /// `crate::daylight`): sky and block levels x 16 (a smooth-lit corner's fractional mean), and
    /// 1 when `color` is to be lit by them; [`UNLIT`]: `color` as it is (overlays, the sky's
    /// sprites, a design shown without light).
    pub light: [u8; 4],
}

/// A vertex's colour drawn as it is (no lightmap).
pub const UNLIT: [u8; 4] = [0; 4];

/// The light of a vertex lit at (sky, block) light levels (0..15, fractional).
pub fn lit(sky: f32, block: f32) -> [u8; 4] {
    [(sky.clamp(0.0, 15.0) * 16.0).round() as u8, (block.clamp(0.0, 15.0) * 16.0).round() as u8, 1, 0]
}

/// One quad (two triangles), kept for back-to-front sorting.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub v: [Vertex; 4],
    pub center: [f32; 3],
}

/// Something to draw besides blocks: an entity (feet position in blocks, yaw in degrees), with
/// its pitch (a minecart on a slope), age in ticks (a dropped item's bob and spin) and fuse left
/// (a primed TNT's flash and swell).
#[derive(Clone, Debug, Default)]
pub struct EntityDraw {
    pub kind: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub age: f32,
    pub fuse: f32,
    /// A mob's render state (`LivingEntityRenderState`: the mob farms' villagers, zombies and
    /// golems, [`crate::mobfarm::MobRun::draw`]).
    pub mob: Option<MobDraw>,
}

/// What `LivingEntityRenderer` draws a mob from (its render state, from redsim's state of the tick
/// and the client's walk animation): angles in degrees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobDraw {
    /// `bodyRot` (the yaw the body turns by) and `yRot` (the head's yaw relative to it).
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub walk_pos: f32,
    pub walk_speed: f32,
    /// `deathTime + partialTicks` (0: alive).
    pub death_time: f32,
    /// `hasRedOverlay`: hurt or dying.
    pub red: bool,
    /// `displayFireAnimation`.
    pub fire: bool,
    /// Sleeping: the bed's orientation.
    pub sleeping: Option<redsim::Dir>,
    /// The standing eye height (a sleeper's head offset: `eyeHeight - 0.1`).
    pub eye_standing: f32,
    /// A passenger (`isPassenger`: a rider's legs forward).
    pub riding: bool,
    /// An iron golem's crackiness (0 none .. 3 high).
    pub crack: u8,
    /// `ageInTicks`.
    pub age: f32,
    /// The bounding box (the flames' size).
    pub width: f32,
    pub height: f32,
    /// The main hand's item id (`getMainHandItem`: a skeleton's bow), if any: its arm's pose.
    pub held: Option<String>,
    /// `isAggressive` (a skeleton drawing its bow at a target).
    pub aggressive: bool,
    /// A sheep's wool `DyeColor` id (0 white).
    pub color: u8,
    /// A bat hanging upside down (`isResting`).
    pub resting: bool,
    /// The tick its keyframe animation started (a bat's: `AnimationState.startTick`).
    pub anim_start: i32,
    /// Its entity id (`getId`: a witch's nose wobbles at `id % 10`'s pace).
    pub entity_id: u32,
    /// A chicken's wing beat at the frame (`ChickenRenderState.flap`, `flapSpeed`).
    pub flap: [f32; 2],
    /// A sheep eating grass (`EatBlockGoal`): its head's drop and angle at the frame
    /// (`getHeadEatPositionScale`, `getHeadEatAngleScale`).
    pub eat: Option<[f32; 2]>,
    /// An enderman's carried block (`getCarriedBlock`), as a block state.
    pub carried: Option<String>,
}

/// Everything a frame needs.
#[derive(Default)]
pub struct Mesh {
    /// Opaque and cutout triangles.
    pub solid: Vec<Vertex>,
    /// Translucent quads (sort them with [`Mesh::sorted_translucent`]).
    pub translucent: Vec<Quad>,
    /// Camera-facing squares (the game's particles), turned to the camera when the frame is drawn
    /// ([`Sprite::quad`]), cut out like the solid faces.
    pub sprites: Vec<Sprite>,
    /// Burning entities' flames, turned about the vertical toward the camera when the frame is
    /// drawn (`crate::entitymodel::flame_triangles`).
    pub flames: Vec<Flame>,
    /// Bounds of everything drawn (blocks), for framing.
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// A burning entity's flames (`FlameFeatureRenderer`): its feet, its box, the fire textures' UVs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flame {
    pub pos: [f32; 3],
    pub width: f32,
    pub height: f32,
    pub uv: [[f32; 4]; 2],
}

/// A camera-facing square: its centre, half its size (blocks), its texture (layer, UV rect) and
/// colour (full brightness: the game's explosion particles light themselves).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub pos: [f32; 3],
    pub half: f32,
    pub layer: u32,
    pub uv: [f32; 4],
    pub color: [u8; 4],
}

impl Sprite {
    /// Its two triangles facing the camera (`right`, `up`: the camera's axes), the game's corner
    /// order (`SingleQuadParticle`: the texture upright on the screen).
    pub fn quad(&self, right: [f32; 3], up: [f32; 3]) -> [Vertex; 6] {
        let c = self.pos;
        let h = self.half;
        let at = |sx: f32, sy: f32| [c[0] + (right[0] * sx + up[0] * sy) * h, c[1] + (right[1] * sx + up[1] * sy) * h, c[2] + (right[2] * sx + up[2] * sy) * h];
        let [u0, v0, u1, v1] = self.uv;
        let v = |pos: [f32; 3], uv: [f32; 2]| Vertex { pos, uv, layer: self.layer, color: self.color, light: UNLIT };
        let (bl, br, tr, tl) = (v(at(-1.0, -1.0), [u0, v1]), v(at(1.0, -1.0), [u1, v1]), v(at(1.0, 1.0), [u1, v0]), v(at(-1.0, 1.0), [u0, v0]));
        [bl, br, tr, bl, tr, tl]
    }
}

/// A block to draw: cell, offset (blocks), state string.
#[derive(Clone, Copy)]
pub struct Item<'a> {
    pub cell: Pos,
    pub offset: [f32; 3],
    pub state: &'a str,
}

/// What [`Builder::blocks_begin_from`] and [`Builder::blocks_from`] draw: an [`Item`], or a
/// session's own block (`crate::session::RBlock`: a staged build draws them without a copy).
pub trait AsItem {
    fn item(&self) -> Item<'_>;
}

impl AsItem for Item<'_> {
    fn item(&self) -> Item<'_> {
        *self
    }
}

fn rotate(p: [f32; 3], axis: u8, angle_deg: f32, o: [f32; 3]) -> [f32; 3] {
    let (s, c) = angle_deg.to_radians().sin_cos();
    let d = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let r = match axis {
        0 => [d[0], d[1] * c - d[2] * s, d[1] * s + d[2] * c],
        1 => [d[0] * c + d[2] * s, d[1], -d[0] * s + d[2] * c],
        _ => [d[0] * c - d[1] * s, d[0] * s + d[1] * c, d[2]],
    };
    [r[0] + o[0], r[1] + o[1], r[2] + o[2]]
}

/// Quarter turns about the block center: x maps up -> north -> down, y maps north -> east.
fn quarter(p: [f32; 3], xrot: u8, yrot: u8) -> [f32; 3] {
    let mut q = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
    for _ in 0..xrot % 4 {
        q = [q[0], q[2], -q[1]];
    }
    for _ in 0..yrot % 4 {
        q = [-q[2], q[1], q[0]];
    }
    [q[0] + 0.5, q[1] + 0.5, q[2] + 0.5]
}

/// The 4 corners (top-left, bottom-left, bottom-right, top-right seen from outside) of face `i`
/// of a part, in pixels, and the game's automatic UV rect.
fn face_corners(from: [f32; 3], to: [f32; 3], i: usize) -> ([[f32; 3]; 4], [f32; 4]) {
    let [x0, y0, z0] = from;
    let [x1, y1, z1] = to;
    match i {
        models::DOWN => ([[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]], [x0, 16.0 - z1, x1, 16.0 - z0]),
        models::UP => ([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], [x0, z0, x1, z1]),
        models::NORTH => ([[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]], [16.0 - x1, 16.0 - y1, 16.0 - x0, 16.0 - y0]),
        models::SOUTH => ([[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]], [x0, 16.0 - y1, x1, 16.0 - y0]),
        models::WEST => ([[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]], [z0, 16.0 - y1, z1, 16.0 - y0]),
        _ => ([[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]], [16.0 - z1, 16.0 - y1, 16.0 - z0, 16.0 - y0]),
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// A face's corners as drawn in its block (`corners`: [`face_corners`]'s, in the model's 0..16):
/// its part's rotation (and the game's rescale), the model's quarter turns, an entity's yaw, and
/// a transform (a block model drawn as part of an entity).
fn face_points(m: &Model, p: &Part, corners: [[f32; 3]; 4], yaw: Option<f32>, xf: Option<&crate::entitymodel::M>) -> [[f32; 3]; 4] {
    let (qx, qy) = p.qrot.unwrap_or((m.xrot, m.yrot));
    corners.map(|c| {
        let c = match p.rot {
            Some(r) => {
                let mut q = rotate(c, r.axis, r.angle, r.origin);
                if r.rescale {
                    // stretched back across the other two axes (the game's rescale)
                    let k = 1.0 / r.angle.to_radians().cos().abs().max(1e-3);
                    for a in 0..3 {
                        if a != r.axis as usize {
                            q[a] = r.origin[a] + (q[a] - r.origin[a]) * k;
                        }
                    }
                }
                q
            }
            None => c,
        };
        let c = [c[0] / 16.0, c[1] / 16.0, c[2] / 16.0];
        let c = quarter(c, qx, qy);
        let c = match yaw {
            // entities: their model is built around x = z = 0 (the entity's position); the
            // game's yaw turns clockwise
            Some(y) => rotate(c, 1, -y, [0.0, 0.0, 0.0]),
            None => c,
        };
        // a block model drawn as part of an entity (its unit cube through the transform)
        match xf {
            Some(m) => m.point(c),
            None => c,
        }
    })
}

/// The block side a face lies on (axis, -1 or 1: flat against the cell's face, facing out), if
/// any: for culling it against its neighbour and for its corners' light.
fn block_side(pts: &[[f32; 3]; 4], n: [f32; 3]) -> Option<(usize, i32)> {
    let axis = (0..3).find(|&k| n[k].abs() > 0.999)?;
    let v = pts[0][axis];
    if !pts.iter().all(|q| (q[axis] - v).abs() < 1e-4) {
        return None;
    }
    let pos_side = n[axis] > 0.0;
    if (pos_side && (v - 1.0).abs() < 1e-4) || (!pos_side && v.abs() < 1e-4) {
        Some((axis, if pos_side { 1 } else { -1 }))
    } else {
        None
    }
}

/// [`Model::sides`]: each part's faces' block side as a still block draws them.
fn model_sides(m: &Model) -> Vec<[Option<(usize, i32)>; 6]> {
    m.parts
        .iter()
        .map(|p| {
            std::array::from_fn(|i| {
                p.faces[i].as_ref()?;
                let pts = face_points(m, p, face_corners(p.from, p.to, i).0, None, None);
                block_side(&pts, norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0]))))
            })
        })
        .collect()
}

/// `minecraft:water[level=1]` -> `minecraft:water`.
fn base_name(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

/// The game's directional shade for a face normal (up 1.0, down 0.5, z 0.8, x 0.6).
pub fn shade(n: [f32; 3]) -> f32 {
    let y = if n[1] > 0.0 { n[1] * n[1] } else { n[1] * n[1] * 0.5 };
    n[0] * n[0] * 0.6 + n[2] * n[2] * 0.8 + y
}

/// The four cells around a face, in the game's order (`AdjacencyInfo.corners`), for a face
/// direction (axis, sign): sides 0 and 1 along one axis, 2 and 3 along the other.
fn adjacency((axis, s): (usize, i32)) -> [(usize, i32); 4] {
    const W: (usize, i32) = (0, -1);
    const E: (usize, i32) = (0, 1);
    const D: (usize, i32) = (1, -1);
    const U: (usize, i32) = (1, 1);
    const N: (usize, i32) = (2, -1);
    const S: (usize, i32) = (2, 1);
    match (axis, s > 0) {
        (1, false) => [W, E, N, S],
        (1, true) => [E, W, N, S],
        (2, false) => [U, D, E, W],
        (2, true) => [W, E, D, U],
        (0, false) => [U, D, N, S],
        _ => [D, U, N, S],
    }
}

/// How opaque a ghosted cell's faces are ([`Builder::ghost`]: the judge's test rig).
pub const GHOST_ALPHA: u8 = 80;

/// A vertex layer's top bit: an X-ray face ([`models::Part::xray`]), drawn at the nearest depth
/// so the blocks in front never hide it (the shader masks the bit off for its texture).
pub const XRAY: u32 = 0x8000_0000;

/// The golem spawn area's colour ([`Builder::spawn_sheet`]): a translucent magenta, no farm
/// block's (water is blue, lava orange, glass pale).
pub const SPAWN_SHEET: [u8; 4] = [236, 64, 214, 130];

/// A [`Builder`] without its atlas (carried between frames: [`Builder::into_parts`]).
pub struct BuilderParts {
    occ: HashSet<Pos>,
    names: HashMap<Pos, String>,
    wet: HashSet<Pos>,
    ground: Option<i32>,
    extra_occ: HashSet<Pos>,
    ghost: HashSet<Pos>,
    cache: HashMap<String, Option<Rc<Model>>, FxBuild>,
    weights: HashMap<String, Option<Vec<u32>>, FxBuild>,
    vcache: HashMap<String, Vec<Option<Option<Rc<Model>>>>, FxBuild>,
    light_cells: Vec<(Pos, crate::light::Cell, u8)>,
    states: Option<HashMap<Pos, u32, crate::camera::FxBuild>>,
    ranges: Option<Vec<[u32; 4]>>,
    low: i32,
    mesh: Mesh,
    xf: Option<crate::entitymodel::M>,
    white: bool,
    light: Option<crate::light::Light>,
    light_floor: Option<i32>,
    light_holes: HashSet<Pos>,
    biome_tint: Option<BiomeTintFn>,
}

impl BuilderParts {
    /// Each item's ranges in the mesh, when kept ([`Builder::ranges`]).
    pub(crate) fn ranges(&self) -> Option<&Vec<[u32; 4]>> {
        self.ranges.as_ref()
    }

    /// How many cells for the light (one per still item).
    pub(crate) fn light_cells_len(&self) -> usize {
        self.light_cells.len()
    }

    /// The mesh dropped (kept elsewhere: a mesh updated in place draws into an empty one).
    pub(crate) fn clear_mesh(&mut self) {
        self.mesh = Mesh::default();
    }

    /// The light (a test compares it).
    pub(crate) fn light(&self) -> Option<&crate::light::Light> {
        self.light.as_ref()
    }
}

/// A terrain's biome colour at a cell for a face the game tints by biome (`tint`: the plains
/// colour the model carries, [`crate::gamemodel::GRASS_TINT`], `FOLIAGE_TINT` or `WATER_TINT`);
/// None: keep it.
pub type BiomeTintFn = std::rc::Rc<dyn Fn(Pos, [u8; 3]) -> Option<[u8; 3]>>;

pub struct Builder<'a> {
    pub atlas: &'a Atlas,
    /// Cells holding an opaque full block (not moving).
    occ: HashSet<Pos>,
    /// Block names of the still cells (translucent faces between the same blocks are culled).
    names: HashMap<Pos, String>,
    /// The still cells of waterlogged blocks (`waterlogged=true`): they hold a water source, drawn
    /// in their cell like the game's fluid renderer (a farm's water in its copper grate).
    wet: HashSet<Pos>,
    /// The ground: every cell below this y counts as an opaque block (culling, AO).
    pub ground: Option<i32>,
    /// More cells holding an opaque full block, drawn some other way (a TNT duper's intact
    /// ground around the ground cells drawn as blocks: [`floor`]): they cull and shade like blocks.
    pub extra_occ: HashSet<Pos>,
    /// Cells drawn see-through ([`crate::scene::Look::ghost`]): their faces go to the translucent
    /// pass at [`GHOST_ALPHA`], and they hide nothing (their neighbours keep their faces).
    pub ghost: HashSet<Pos>,
    cache: HashMap<String, Option<Rc<Model>>, FxBuild>,
    /// Each state's variants' weights (None: one model wherever it stands), and the models of
    /// the variants drawn so far ([`Builder::model_at`]).
    weights: HashMap<String, Option<Vec<u32>>, FxBuild>,
    vcache: HashMap<String, Vec<Option<Option<Rc<Model>>>>, FxBuild>,
    /// [`Builder::blocks_begin_from`]'s cells for the light so far, and the lowest cell's y.
    light_cells: Vec<(Pos, crate::light::Cell, u8)>,
    /// [`Builder::blocks_from`]'s still cells by cell (their index in its items), kept from one
    /// call of a staged build to the next.
    states: Option<HashMap<Pos, u32, crate::camera::FxBuild>>,
    /// When kept (`Some`): each item's vertices drawn by [`Builder::blocks_from`], its ranges of
    /// solid vertices and translucent quads (a mesh updated in place: `crate::remesh`).
    pub ranges: Option<Vec<[u32; 4]>>,
    low: i32,
    pub mesh: Mesh,
    /// A transform for the block model being drawn as part of an entity (a minecart's hopper, a
    /// dropped block item): its unit cube goes through it.
    xf: Option<crate::entitymodel::M>,
    /// The block model being drawn flashes white (a primed TNT: `OverlayTexture`'s white at
    /// u = 15, 75% white): its solid faces carry the mix in their vertex alpha.
    white: bool,
    /// The game's light around the still blocks ([`crate::light::Light`]: dark under overhangs,
    /// bright around light sources), set by [`Builder::blocks`]; it also holds which cells darken
    /// the corners next to them like a full block (an opaque cube, but also leaves, ice, a slime
    /// block: the game's `getShadeBrightness` 0.2; glass and the other `TransparentBlock`s do not).
    light: Option<crate::light::Light>,
    /// Where the ground is for the light, when it is not [`Builder::ground`] nor under the
    /// lowest cell (a TNT duper's own ground: cells below this y).
    pub light_floor: Option<i32>,
    /// Holes in that ground (a TNT crater's blown cells): open air for the light.
    pub light_holes: HashSet<Pos>,
    /// A farm in its terrain ([`crate::terrain`]): its biomes' colours for grass, leaves and water.
    pub biome_tint: Option<BiomeTintFn>,
}

impl<'a> Builder<'a> {
    pub fn new(atlas: &'a Atlas) -> Builder<'a> {
        Builder {
            atlas,
            occ: HashSet::new(),
            names: HashMap::new(),
            wet: HashSet::new(),
            ground: None,
            extra_occ: HashSet::new(),
            ghost: HashSet::new(),
            cache: HashMap::default(),
            weights: HashMap::default(),
            vcache: HashMap::default(),
            light_cells: Vec::new(),
            states: None,
            ranges: None,
            low: i32::MAX,
            mesh: Mesh { lo: [f32::MAX; 3], hi: [f32::MIN; 3], ..Default::default() },
            xf: None,
            white: false,
            light: None,
            light_floor: None,
            light_holes: HashSet::new(),
            biome_tint: None,
        }
    }

    fn occupied(&self, c: Pos) -> bool {
        if self.ground.is_some_and(|g| c[1] < g) {
            return true;
        }
        match &self.light {
            // the still blocks' opaque cells, looked up in the light's box (a dozen per face)
            Some(l) => l.opaque(c),
            None => self.occ.contains(&c) || self.extra_occ.contains(&c),
        }
    }

    /// The (sky, block) light the game reads at a cell (full daylight where none was computed).
    pub fn light_at(&self, c: Pos) -> (u8, u8) {
        match &self.light {
            Some(l) => l.at(c),
            None => (15, 0),
        }
    }

    /// The light in a cell a renderer lights an entity or a block entity by (its block position):
    /// its own, or, a block the light model counts as filling it (a chest, a bed), the brightest
    /// of its six neighbours' less one (the game's light passes into it: such a block blocks at
    /// most one level).
    pub fn light_inside(&self, c: Pos) -> (u8, u8) {
        let (mut s, mut b) = self.light_at(c);
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let (ns, nb) = self.light_at([c[0] + d[0], c[1] + d[1], c[2] + d[2]]);
            s = s.max(ns.saturating_sub(1));
            b = b.max(nb.saturating_sub(1));
        }
        (s, b)
    }

    /// The light computed by [`Builder::blocks`], for the grass under the build.
    pub fn take_light(&mut self) -> Option<crate::light::Light> {
        self.light.take()
    }

    /// The light back after [`Builder::take_light`].
    pub(crate) fn put_light(&mut self, l: Option<crate::light::Light>) {
        self.light = l;
    }

    /// A cell's shade for the game's ambient occlusion (`getShadeBrightness`).
    fn shade_at(&self, c: Pos) -> f32 {
        if self.occupied(c) || self.light.as_ref().is_some_and(|l| l.shade_full(c)) { 0.2 } else { 1.0 }
    }

    /// The cells holding an opaque full cube so far (they occlude and block the sky's light).
    pub fn opaque_cells(&self) -> &HashSet<Pos> {
        &self.occ
    }

    fn model(&mut self, state: &str) -> Option<Rc<Model>> {
        self.model_at(state, None)
    }

    /// A block's model; at `cell` for a block the game draws as one of several variants picked
    /// from its position (cached per state, and per variant for those: a terrain's tens of
    /// thousands of grass and stone blocks share a few models).
    fn model_at(&mut self, state: &str, cell: Option<Pos>) -> Option<Rc<Model>> {
        let variant = match (cell, self.atlas.game.as_ref()) {
            (Some(c), Some(g)) => {
                if !self.weights.contains_key(state) {
                    let w = g.models.variant_weights(state);
                    self.weights.insert(state.to_string(), w);
                }
                self.weights[state].as_ref().map(|w| (crate::gamemodel::pick_index(w, c), w.len()))
            }
            _ => None,
        };
        if let Some((i, n)) = variant {
            if let Some(Some(m)) = self.vcache.get(state).map(|v| &v[i]) {
                return m.clone();
            }
            let m = self.new_model(state, cell).map(Rc::new);
            self.vcache.entry(state.to_string()).or_insert_with(|| vec![None; n])[i] = Some(m.clone());
            return m;
        }
        if let Some(m) = self.cache.get(state) {
            return m.clone();
        }
        let m = self.new_model(state, None).map(Rc::new);
        self.cache.insert(state.to_string(), m.clone());
        m
    }

    /// [`Builder::model_at`] uncached.
    fn new_model(&self, state: &str, cell: Option<Pos>) -> Option<Model> {
        let m = models::model_at(state, self.atlas, cell);
        // a light source (a lit lamp, a torch, a copper bulb...): the game lights it flat, at its
        // own light, without corner darkening, its faces still shaded by direction (critique:
        // "Lamps never look lit" was its unlit texture; drawing it unshaded made its sides too
        // bright against the real game)
        m.map(|mut m| {
            m.emission = crate::light::emission(state);
            // (each face's side as a still block draws it: a hidden face dropped at once)
            m.sides = model_sides(&m);
            m
        })
    }

    /// Add blocks (call once per frame with every block).
    pub fn blocks(&mut self, items: &[Item]) {
        self.blocks_begin(items);
        self.blocks_from(items, 0, &mut || false);
    }

    /// [`Builder::blocks`]' first pass: every still cell's name, occupancy and light (the game's
    /// light over them all); then [`Builder::blocks_from`] draws them.
    pub fn blocks_begin(&mut self, items: &[Item]) {
        let done = self.blocks_begin_from(items, 0, &mut || false);
        debug_assert_eq!(done, items.len());
        self.blocks_begin_finish();
    }

    /// [`Builder::blocks_begin`] from item `from`, in order, until the last or until
    /// `out_of_time` says so (asked every 256 items): the next item (`items.len()`: done, then
    /// [`Builder::blocks_begin_finish`]). A farm in its terrain's tens of thousands of blocks take
    /// several frames ([`crate::scene::Staged`]); the same cells in the same order either way.
    pub fn blocks_begin_from<T: AsItem>(&mut self, items: &[T], from: usize, out_of_time: &mut dyn FnMut() -> bool) -> usize {
        if from == 0 {
            self.occ.clear();
            self.names.clear();
            self.wet.clear();
            self.light = None;
            self.light_cells = Vec::with_capacity(items.len() + self.extra_occ.len());
            self.low = i32::MAX;
            self.states = None;
        }
        let mut i = from;
        while i < items.len() {
            let it = items[i].item();
            i += 1;
            self.low = self.low.min(it.cell[1]);
            if it.offset == [0.0; 3] {
                let lc = self.begin_item(&it);
                self.light_cells.push(lc);
            }
            if i % 256 == 0 && i < items.len() && out_of_time() {
                break;
            }
        }
        i
    }

    /// The first pass of one still block: its cell's name, water and occupancy noted, and its
    /// cell for the light (its light properties and flags).
    pub(crate) fn begin_item(&mut self, it: &Item) -> (Pos, crate::light::Cell, u8) {
        // water and bubble columns are one fluid: no faces between them
        let n = match base_name(it.state) {
            "minecraft:bubble_column" => "minecraft:water",
            n => n,
        };
        self.names.insert(it.cell, n.to_string());
        if it.state.contains("waterlogged=true") {
            self.wet.insert(it.cell);
        }
        let m = self.model_at(it.state, Some(it.cell));
        let opaque = m.as_ref().is_some_and(|m| m.full && m.parts.iter().all(|p| p.layer == Layer::Opaque)) && !self.ghost.contains(&it.cell);
        if opaque {
            self.occ.insert(it.cell);
        }
        let fills = m.as_ref().is_some_and(|m| m.fills);
        // what darkens corners like a full block (`getShadeBrightness`): a full collision
        // box, but not glass and the other `TransparentBlock`s, nor a honey block
        let name = n.trim_start_matches("minecraft:");
        let shade_full = opaque || (fills && !self.ghost.contains(&it.cell) && !name.ends_with("glass") && !matches!(name, "honey_block" | "barrier" | "light" | "structure_void"));
        // a full collision box (`isCollisionShapeFullBlock`): not a honey block's, soul
        // sand's or mud's (smaller), whatever they draw
        let collision_full = (opaque || fills) && !matches!(name, "honey_block" | "soul_sand" | "mud" | "powder_snow" | "light" | "structure_void");
        let flags = if shade_full { crate::light::SHADE_FULL } else { 0 } | if collision_full { crate::light::COLLISION_FULL } else { 0 };
        (it.cell, crate::light::cell_of(it.state, opaque, fills), flags)
    }

    /// A still cell's block gone from the first pass (its name, water and occupancy).
    pub(crate) fn forget_cell(&mut self, c: Pos) {
        self.names.remove(&c);
        self.wet.remove(&c);
        self.occ.remove(&c);
    }

    /// [`Builder::blocks_begin`]'s end: the game's light over the cells gathered.
    pub fn blocks_begin_finish(&mut self) {
        self.light = self.compute_light();
    }

    /// The game's light over the cells gathered (the still blocks' and [`Builder::extra_occ`]),
    /// the cells kept (a mesh updated in place lights them again: [`Builder::relight`]).
    fn compute_light(&mut self) -> Option<crate::light::Light> {
        let n = self.light_cells.len();
        for c in &self.extra_occ {
            self.light_cells.push((*c, crate::light::Cell { opaque: true, dampening: 15, ..Default::default() }, crate::light::SHADE_FULL | crate::light::COLLISION_FULL));
        }
        // the game's light around them: the build stands on the judge's grass (one block of air
        // under its lowest cell) or on its own ground
        let light = if self.light_cells.is_empty() { None } else { Some(crate::light::Light::compute(&self.light_cells, self.light_floor.or(self.ground).unwrap_or(self.low - 1), &self.light_holes)) };
        self.light_cells.truncate(n);
        light
    }

    /// Item `i`'s cell for the light (a mesh updated in place: [`Builder::begin_item`]'s, or an air
    /// cell for a block gone), or a new item's at the end (`i` = the cells so far).
    pub(crate) fn set_light_cell(&mut self, i: usize, cell: (Pos, crate::light::Cell, u8)) {
        if i == self.light_cells.len() {
            self.light_cells.push(cell);
        } else {
            self.light_cells[i] = cell;
        }
    }

    /// The light again after cells changed: the cells whose light (or block) differs, None when
    /// only a whole rebuild knows (its box, floor or holes moved: the light is left as it was).
    pub(crate) fn relight(&mut self, touched: &[Pos]) -> Option<Vec<Pos>> {
        if self.light_floor.or(self.ground).is_none() {
            // (the floor under the lowest cell: a cell lower than it moves it)
            let low = self.light_cells.iter().map(|(c, _, _)| c[1]).min().unwrap_or(i32::MAX);
            if low != self.low {
                return None;
            }
        }
        if touched.is_empty() {
            return Some(Vec::new());
        }
        // the columns within 16 of the cells changed (a change reaches 15 sideways at most, the
        // sky down whole columns) worked out alone, the light around them as it was
        // (`Light::recompute_columns`); a region of over half the box: all of it again
        let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
        for c in touched {
            lo = [lo[0].min(c[0]), lo[1].min(c[2])];
            hi = [hi[0].max(c[0]), hi[1].max(c[2])];
        }
        let (lo, hi) = ([lo[0] - 16, lo[1] - 16], [hi[0] + 16, hi[1] + 16]);
        let small = self.light.as_ref().is_some_and(|l| {
            let (w, d) = l.columns();
            (hi[0] - lo[0] + 1).min(w) as i64 * (hi[1] - lo[1] + 1).min(d) as i64 * 2 < w as i64 * d as i64
        });
        if small {
            let mut light = self.light.take()?;
            let n = self.light_cells.len();
            for c in &self.extra_occ {
                self.light_cells.push((*c, crate::light::Cell { opaque: true, dampening: 15, ..Default::default() }, crate::light::SHADE_FULL | crate::light::COLLISION_FULL));
            }
            let changed = light.recompute_columns(&self.light_cells, lo, hi);
            self.light_cells.truncate(n);
            // (light that spread out of the region: the light is no longer whole, a whole build)
            self.light = Some(light);
            return changed;
        }
        let old = self.light.take()?;
        let Some(new) = self.compute_light() else {
            self.light = Some(old);
            return None;
        };
        let Some(changed) = new.changed_from(&old) else {
            self.light = Some(old);
            return None;
        };
        self.light = Some(new);
        Some(changed)
    }

    /// The faces of one still block drawn into the mesh ([`Builder::blocks_from`]'s, a mesh updated
    /// in place: `states` a still cell's state): the solid vertices' and translucent quads' ranges.
    pub(crate) fn emit_item<'s>(&mut self, it: &Item, states: &dyn Fn(Pos) -> Option<&'s str>) -> [u32; 4] {
        let (s0, t0) = (self.mesh.solid.len() as u32, self.mesh.translucent.len() as u32);
        self.block_item(it, states);
        [s0, self.mesh.solid.len() as u32, t0, self.mesh.translucent.len() as u32]
    }

    /// The still cells' states by cell (their index in the items), taken out for a mesh updated
    /// in place (put back with [`Builder::put_states`]).
    pub(crate) fn take_states(&mut self) -> Option<HashMap<Pos, u32, crate::camera::FxBuild>> {
        self.states.take()
    }

    pub(crate) fn put_states(&mut self, s: HashMap<Pos, u32, crate::camera::FxBuild>) {
        self.states = Some(s);
    }

    /// [`Builder::blocks`]' second pass from item `from`, in order, until the last or until
    /// `out_of_time` says so (asked every 16 blocks): the next item to draw (`items.len()`: done).
    /// The same faces in the same order whether it runs at once or over several calls (a big
    /// design's still is built over several frames: [`crate::scene::Staged`]).
    pub fn blocks_from<T: AsItem>(&mut self, items: &[T], from: usize, out_of_time: &mut dyn FnMut() -> bool) -> usize {
        // the still cells' states by cell (a water cell's neighbours' levels), worked out once a
        // build: a staged build's calls share it (it took a whole frame's time on a farm in its
        // terrain, every call)
        let states = match self.states.take() {
            Some(s) if from > 0 => s,
            _ => {
                let mut s: HashMap<Pos, u32, crate::camera::FxBuild> = HashMap::with_capacity_and_hasher(items.len(), Default::default());
                for (k, it) in items.iter().enumerate() {
                    let it = it.item();
                    if it.offset == [0.0; 3] {
                        s.insert(it.cell, k as u32);
                    }
                }
                s
            }
        };
        let mut i = from;
        if from == 0
            && let Some(r) = self.ranges.as_mut()
        {
            r.clear();
        }
        while i < items.len() {
            let it = items[i].item();
            let r = self.emit_item(&it, &|p: Pos| states.get(&p).map(|&k| items[k as usize].item().state));
            if let Some(v) = self.ranges.as_mut() {
                v.push(r);
            }
            i += 1;
            if i % 16 == 0 && i < items.len() && out_of_time() {
                break;
            }
        }
        self.states = Some(states);
        i
    }

    /// One block of [`Builder::blocks_from`] (`states`: a still cell's state).
    fn block_item<'s>(&mut self, it: &Item, states: &dyn Fn(Pos) -> Option<&'s str>) {
        // water: the game's sloped surface (corner heights from the neighbouring cells), not
        // cuboids (their steps showed as lines across a pool)
        if it.offset == [0.0; 3] && matches!(base_name(it.state), "minecraft:water" | "minecraft:bubble_column") {
            self.water(it.cell, states);
            return;
        }
        // water under water fills its cell (the game's surface is only at the top)
        let state = if it.offset == [0.0; 3] && base_name(it.state) == "minecraft:water" && self.names.get(&[it.cell[0], it.cell[1] + 1, it.cell[2]]).is_some_and(|n| n == "minecraft:water") {
            "minecraft:water[level=8]"
        } else {
            it.state
        };
        // chests and shulker boxes: the game's block-entity models (ChestModel, ShulkerModel)
        // with its entity textures, once loaded
        if it.offset == [0.0; 3] && self.block_entity(state, it.cell) {
            return;
        }
        if let Some(m) = self.model_at(state, Some(it.cell)) {
            let base = [it.cell[0] as f32 + it.offset[0], it.cell[1] as f32 + it.offset[1], it.cell[2] as f32 + it.offset[2]];
            let still = it.offset == [0.0; 3];
            self.add_model(&m, base, if still { Some(it.cell) } else { None }, None);
        }
        // a waterlogged block's water, in its cell
        if it.offset == [0.0; 3] && self.wet.contains(&it.cell) {
            self.water(it.cell, states);
        }
    }

    /// The builder without its atlas, to carry it from one frame to the next
    /// ([`Builder::from_parts`]).
    pub fn into_parts(self) -> BuilderParts {
        BuilderParts {
            occ: self.occ,
            names: self.names,
            wet: self.wet,
            ground: self.ground,
            extra_occ: self.extra_occ,
            ghost: self.ghost,
            cache: self.cache,
            weights: self.weights,
            vcache: self.vcache,
            light_cells: self.light_cells,
            states: self.states,
            ranges: self.ranges,
            low: self.low,
            mesh: self.mesh,
            xf: self.xf,
            white: self.white,
            light: self.light,
            light_floor: self.light_floor,
            light_holes: self.light_holes,
            biome_tint: self.biome_tint,
        }
    }

    /// A builder again from [`Builder::into_parts`].
    pub fn from_parts(atlas: &'a Atlas, p: BuilderParts) -> Builder<'a> {
        Builder {
            atlas,
            occ: p.occ,
            names: p.names,
            wet: p.wet,
            ground: p.ground,
            extra_occ: p.extra_occ,
            ghost: p.ghost,
            cache: p.cache,
            weights: p.weights,
            vcache: p.vcache,
            light_cells: p.light_cells,
            states: p.states,
            ranges: p.ranges,
            low: p.low,
            mesh: p.mesh,
            xf: p.xf,
            white: p.white,
            light: p.light,
            light_floor: p.light_floor,
            light_holes: p.light_holes,
            biome_tint: p.biome_tint,
        }
    }

    /// A chest or shulker box drawn like the game's block-entity renderers (false: not one, or
    /// the entity textures are not loaded yet: the viewer's model is drawn).
    fn block_entity(&mut self, state: &str, cell: Pos) -> bool {
        let id = base_name(state);
        if id != "minecraft:chest" && !id.ends_with("shulker_box") {
            return false;
        }
        let Some(g) = self.atlas.game.clone() else { return false };
        if !g.entities {
            g.wants_entities.set(true);
            return false;
        }
        match crate::entitymodel::block_entity_triangles(&g, state, cell) {
            Some(mut tris) => {
                // its light (the block entity renderer's: its cell's), for the frame's lightmap
                let (s, b) = self.light_inside(cell);
                let l = lit(s as f32, b as f32);
                for v in tris.iter_mut() {
                    v.light = l;
                    for k in 0..3 {
                        self.mesh.lo[k] = self.mesh.lo[k].min(v.pos[k]);
                        self.mesh.hi[k] = self.mesh.hi[k].max(v.pos[k]);
                    }
                }
                self.mesh.solid.extend_from_slice(&tris);
                true
            }
            None => false,
        }
    }

    /// An entity model at a feet position, turned by yaw (degrees, the game's: 0 = south), lit by
    /// its cell's light (`EntityRenderer.getPackedLightCoords`: the light at its block position)
    /// for the frame's lightmap.
    pub fn entity(&mut self, e: &EntityDraw) {
        let (s0, t0) = (self.mesh.solid.len(), self.mesh.translucent.len());
        self.entity_parts(e);
        let cell = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
        let (s, b) = self.light_inside(cell);
        let l = lit(s as f32, b as f32);
        for v in &mut self.mesh.solid[s0..] {
            if v.light == UNLIT {
                v.light = l;
            }
        }
        for q in &mut self.mesh.translucent[t0..] {
            for v in q.v.iter_mut() {
                if v.light == UNLIT {
                    v.light = l;
                }
            }
        }
    }

    fn entity_parts(&mut self, e: &EntityDraw) {
        if let Some(rest) = e.kind.strip_prefix("frame:") {
            self.item_frame(e.pos, rest);
            return;
        }
        if let Some(state) = e.kind.strip_prefix("falling:") {
            // FallingBlockRenderer: the block model, half a block back (the game's or the viewer's)
            if let Some(m) = self.model(state) {
                self.add_model(&m, [e.pos[0] - 0.5, e.pos[1], e.pos[2] - 0.5], None, None);
            }
            return;
        }
        if let Some(state) = e.kind.strip_prefix("primed:") {
            // TntRenderer: the block model centred on the entity, swelling over the last 10
            // ticks, flashing white every 5 (the white overlay: 75% white)
            if let Some(m) = self.model(state) {
                let (scale, white) = crate::entitymodel::tnt_look(e.fuse);
                self.xf = Some(crate::entitymodel::M::id().translate([e.pos[0], e.pos[1] + 0.5, e.pos[2]]).scale([scale, scale, scale]).translate([-0.5, -0.5, -0.5]));
                self.white = white;
                self.add_model(&m, [0.0; 3], None, None);
                self.xf = None;
                self.white = false;
            }
            return;
        }
        let game = self.atlas.game.clone();
        if let Some(g) = &game {
            if !g.entities {
                // drawn with the viewer's stand-in until the page loads the game's entity textures
                g.wants_entities.set(true);
            } else if self.game_entity(g, e) {
                return;
            }
        }
        // an item entity: a small cube of its block's texture, else of its color
        if let Some(id) = e.kind.strip_prefix("item:") {
            let n = id.strip_prefix("minecraft:").unwrap_or(id);
            let part = match self.atlas.names.get_key_value(n) {
                Some((tile, _)) => Part::new([-2.0, 0.0, -2.0], [2.0, 4.0, 2.0]).all(tile),
                None => Part::new([-2.0, 0.0, -2.0], [2.0, 4.0, 2.0]).all("generic").tint(models::name_color(n)),
            };
            let m = Model::one(part.nocull());
            self.add_model(&m, [e.pos[0], e.pos[1] + 0.1, e.pos[2]], None, Some(e.yaw));
            return;
        }
        if let Some(m) = models::entity_model(&e.kind) {
            self.add_model(&m, e.pos, None, Some(e.yaw));
        }
    }

    /// An entity drawn like the game: its own model and texture (boats, minecarts with their
    /// display block, armor stands), a falling block's block model, a dropped item (a block
    /// item's block model, else its sprite). False when the viewer has nothing for it.
    /// An item frame (`ItemFrameRenderer`; the session's `frame:<facing>:<turn>:<item>`): the
    /// game's `item_frame` block model (birch planks around a leather back, built facing north)
    /// turned to face its way in its cell, then its item at half size against the back, turned
    /// its turn x 45 degrees about the frame's normal (clockwise seen from the front). Without the
    /// game's models: the template's shape in the viewer's planks.
    fn item_frame(&mut self, pos: [f32; 3], rest: &str) {
        use crate::entitymodel as em;
        use redsim::Dir;
        let mut it = rest.splitn(3, ':');
        let d = it.next().and_then(Dir::from_name).unwrap_or(Dir::South);
        let turn: u8 = it.next().and_then(|r| r.parse().ok()).unwrap_or(0);
        let item = it.next().unwrap_or("");
        // the entity sits 0.46875 back toward its wall; the renderer moves it to the cell's centre
        let st = d.step();
        let center = [0, 1, 2].map(|k| pos[k] + st[k] as f32 * 0.46875);
        let frame = em::item_frame_transform(center, d);
        let model = self.model("minecraft:item_frame[map=false]").filter(|m| !m.full).unwrap_or_else(|| {
            let wood = |from: [f32; 3], to: [f32; 3]| Part::new(from, to).all("oak_planks").nocull();
            Rc::new(Model {
                parts: vec![
                    Part::new([3.0, 3.0, 15.5], [13.0, 13.0, 16.0]).face(models::NORTH, "generic").face(models::SOUTH, "generic").tint([110, 70, 45]).nocull(),
                    wood([2.0, 2.0, 15.0], [14.0, 3.0, 16.0]),
                    wood([2.0, 13.0, 15.0], [14.0, 14.0, 16.0]),
                    wood([2.0, 3.0, 15.0], [3.0, 13.0, 16.0]),
                    wood([13.0, 3.0, 15.0], [14.0, 13.0, 16.0]),
                ],
                ..Default::default()
            })
        });
        self.xf = Some(frame.translate([-0.5, -0.5, -0.5]));
        self.add_model(&model, [0.0; 3], None, None);
        self.xf = None;
        if item.is_empty() {
            return;
        }
        let m = em::framed_item_transform(&frame, turn);
        let game = self.atlas.game.clone();
        let name = item.strip_prefix("minecraft:").unwrap_or(item);
        if let Some(g) = &game
            && g.tex.contains_key(&format!("item/{name}"))
        {
            if let Some(tris) = em::sprite_triangles(g, item, &m) {
                for v in &tris {
                    for k in 0..3 {
                        self.mesh.lo[k] = self.mesh.lo[k].min(v.pos[k]);
                        self.mesh.hi[k] = self.mesh.hi[k].max(v.pos[k]);
                    }
                }
                self.mesh.solid.extend_from_slice(&tris);
            }
            return;
        }
        if let Some(g) = &game
            && !g.entities
        {
            // the item textures come with the entity textures: the page loads them
            g.wants_entities.set(true);
        }
        // a block item (its block model at the `fixed` display's half size), else a flat square
        // of the item's colour until the item textures are in
        if let Some(bm) = self.model(&format!("minecraft:{name}")).filter(|_| game.as_ref().is_some_and(|g| g.models.model(&format!("minecraft:{name}"), None).is_some())) {
            self.xf = Some(m.translate([0.5, 0.5, 0.0]).scale([0.5, 0.5, 0.5]).translate([-0.5, -0.5, -0.5]));
            self.add_model(&bm, [0.0; 3], None, None);
            self.xf = None;
        } else {
            let p = Part::new([2.0, 2.0, 0.0], [14.0, 14.0, 0.5]).all("generic").tint(models::name_color(name)).nocull();
            self.xf = Some(m);
            self.add_model(&Model::one(p), [0.0; 3], None, None);
            self.xf = None;
        }
    }

    fn game_entity(&mut self, g: &crate::atlas::GameAssets, e: &EntityDraw) -> bool {
        use crate::entitymodel as em;
        let push = |b: &mut Self, tris: Vec<Vertex>| {
            for v in &tris {
                for k in 0..3 {
                    b.mesh.lo[k] = b.mesh.lo[k].min(v.pos[k]);
                    b.mesh.hi[k] = b.mesh.hi[k].max(v.pos[k]);
                }
            }
            b.mesh.solid.extend_from_slice(&tris);
        };
        if let Some(id) = e.kind.strip_prefix("item:") {
            let name = id.strip_prefix("minecraft:").unwrap_or(id);
            if g.tex.contains_key(&format!("item/{name}")) || (em::BLOCK_SPRITE_ITEMS.contains(&name) && g.tex.contains_key(&format!("block/{name}"))) {
                if let Some(t) = em::item_sprite_triangles(g, id, e.pos, e.age) {
                    push(self, t);
                    return true;
                }
            } else if let Some(m) = self.model(&format!("minecraft:{name}")) {
                self.xf = Some(em::item_block_transform(e.pos, e.age));
                self.add_model(&m, [0.0; 3], None, None);
                self.xf = None;
                return true;
            }
            return false;
        }
        // a mob farm's mob: its model, texture and renderer's transforms, and its flames
        if let Some(mob) = &e.mob {
            let Some(tris) = em::mob_triangles(g, &e.kind, e.pos, mob) else { return false };
            push(self, tris);
            // an enderman's carried block (`CarriedBlockLayer`): its block model in its arms
            if let Some(state) = &mob.carried
                && let Some(m) = self.model(state)
            {
                self.xf = Some(em::carried_block_transform(e.pos, mob));
                self.add_model(&m, [0.0; 3], None, None);
                self.xf = None;
            }
            if mob.fire
                && let Some(uv) = em::fire_uvs(g)
            {
                self.mesh.flames.push(Flame { pos: e.pos, width: mob.width, height: mob.height, uv });
            }
            return true;
        }
        let Some(tris) = em::entity_triangles(g, &e.kind, e.pos, e.yaw, e.pitch) else { return false };
        push(self, tris);
        // a hopper minecart's hopper (display offset 1), a chest minecart's chest...
        let k = e.kind.strip_prefix("minecraft:").unwrap_or(&e.kind);
        let display = match k {
            "hopper_minecart" => Some(("minecraft:hopper[enabled=true,facing=down]", 1)),
            "chest_minecart" => Some(("minecraft:chest[facing=north,type=single,waterlogged=false]", 8)),
            "tnt_minecart" => Some(("minecraft:tnt[unstable=false]", 6)),
            "furnace_minecart" => Some(("minecraft:furnace[facing=north,lit=false]", 6)),
            _ => None,
        };
        if let Some((state, offset)) = display
            && let Some(m) = self.model(state)
        {
            self.xf = Some(em::minecart_block_transform(e.pos, e.yaw, e.pitch, offset));
            self.add_model(&m, [0.0; 3], None, None);
            self.xf = None;
        }
        true
    }

    /// A water (or bubble column) cell like the game draws fluids: its surface at each corner the
    /// average height of the water cells sharing that corner (full when water stands above any
    /// of them), faces only toward cells that are neither water nor opaque cubes.
    fn water<'s>(&mut self, cell: Pos, states: &dyn Fn(Pos) -> Option<&'s str>) {
        // water cells, and waterlogged blocks' (their own water: a source)
        let is_water = |b: &Self, p: Pos| b.names.get(&p).is_some_and(|n| n == "minecraft:water") || b.wet.contains(&p);
        // the own height of a water cell (in blocks): full under water, else the game's amount / 9
        let level_of = |p: Pos| -> Option<u32> {
            states(p).map(|s| {
                if base_name(s) == "minecraft:bubble_column" {
                    0
                } else {
                    s.split("level=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).unwrap_or(0)
                }
            })
        };
        let me = &*self;
        let height = |p: Pos| -> Option<f32> {
            if !is_water(me, p) {
                return None;
            }
            if is_water(me, [p[0], p[1] + 1, p[2]]) {
                return Some(1.0);
            }
            let l = level_of(p).unwrap_or(0);
            Some(if l == 0 || l >= 8 { 8.0 / 9.0 } else { (8 - l) as f32 / 9.0 })
        };
        let above = is_water(me, [cell[0], cell[1] + 1, cell[2]]);
        // corner (dx, dz) in {0,1}: the cells sharing it
        let corner = |dx: i32, dz: i32| -> f32 {
            if above {
                return 1.0;
            }
            let (mut sum, mut n) = (0.0f32, 0.0f32);
            for (ox, oz) in [(dx - 1, dz - 1), (dx, dz - 1), (dx - 1, dz), (dx, dz)] {
                let q = [cell[0] + ox, cell[1], cell[2] + oz];
                if let Some(h) = height(q) {
                    if h >= 1.0 {
                        return 1.0;
                    }
                    sum += h;
                    n += 1.0;
                }
            }
            if n > 0.0 { sum / n } else { 8.0 / 9.0 }
        };
        let h = [corner(0, 0), corner(0, 1), corner(1, 1), corner(1, 0)];
        let tile = self.atlas.tile("water") as u32;
        let flow = self.atlas.tile("water_flow") as u32;
        let (x, y, z) = (cell[0] as f32, cell[1] as f32, cell[2] as f32);
        // its light (`LiquidBlockRenderer.getLightColor`: each of sky and block, the larger of the
        // cell's and the one above's), for the frame's lightmap
        let wl = {
            let (a, b) = (self.light_at(cell), self.light_at([cell[0], cell[1] + 1, cell[2]]));
            lit(a.0.max(b.0) as f32, a.1.max(b.1) as f32)
        };
        let quad = |b: &mut Self, pts: [[f32; 3]; 4], uv: [[f32; 2]; 4], n: [f32; 3], layer: u32| {
            let l = shade(n);
            let c = (255.0 * l).round() as u8;
            let v: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: pts[k], uv: uv[k], layer, color: [c, c, c, 255], light: wl });
            let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
            b.mesh.translucent.push(Quad { v, center });
            for q in pts {
                for k in 0..3 {
                    b.mesh.lo[k] = b.mesh.lo[k].min(q[k]);
                    b.mesh.hi[k] = b.mesh.hi[k].max(q[k]);
                }
            }
        };
        let open = |b: &Self, q: Pos| !is_water(b, q) && !b.occupied(q);
        // the surface (seen from above and below)
        if !above {
            quad(self, [[x, y + h[0], z], [x, y + h[1], z + 1.0], [x + 1.0, y + h[2], z + 1.0], [x + 1.0, y + h[3], z]], [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], [0.0, 1.0, 0.0], tile);
            quad(self, [[x, y + h[0], z], [x + 1.0, y + h[3], z], [x + 1.0, y + h[2], z + 1.0], [x, y + h[1], z + 1.0]], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [0.0, 1.0, 0.0], tile);
        }
        if open(self, [cell[0], cell[1] - 1, cell[2]]) {
            quad(self, [[x, y, z], [x + 1.0, y, z], [x + 1.0, y, z + 1.0], [x, y, z + 1.0]], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [0.0, -1.0, 0.0], tile);
        }
        // sides: north (-z) uses corners 0 (x0,z0) and 3 (x1,z0); south (+z) 1 and 2; west (-x)
        // 0 and 1; east (+x) 3 and 2
        let sides: [(Pos, [[f32; 3]; 4], [f32; 2], [f32; 3]); 4] = [
            ([0, 0, -1], [[x + 1.0, y, z], [x, y, z], [x, y + h[0], z], [x + 1.0, y + h[3], z]], [h[3], h[0]], [0.0, 0.0, -1.0]),
            ([0, 0, 1], [[x, y, z + 1.0], [x + 1.0, y, z + 1.0], [x + 1.0, y + h[2], z + 1.0], [x, y + h[1], z + 1.0]], [h[1], h[2]], [0.0, 0.0, 1.0]),
            ([-1, 0, 0], [[x, y, z], [x, y, z + 1.0], [x, y + h[1], z + 1.0], [x, y + h[0], z]], [h[0], h[1]], [-1.0, 0.0, 0.0]),
            ([1, 0, 0], [[x + 1.0, y, z + 1.0], [x + 1.0, y, z], [x + 1.0, y + h[3], z], [x + 1.0, y + h[2], z + 1.0]], [h[2], h[3]], [1.0, 0.0, 0.0]),
        ];
        for (d, pts, top, n) in sides {
            let q = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
            if open(self, q) {
                let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 1.0 - top[1]], [0.0, 1.0 - top[0]]];
                quad(self, pts, uv, n, flow);
                // seen from inside too, the same shade (the game's back face), but against glass,
                // ice, slime, honey or leaves (its overlay texture, one-sided)
                let overlay = self.names.get(&q).is_some_and(|nm| {
                    let nm = nm.trim_start_matches("minecraft:");
                    nm.ends_with("glass") || nm.ends_with("leaves") || matches!(nm, "ice" | "frosted_ice" | "slime_block" | "honey_block")
                });
                if !overlay {
                    quad(self, [pts[0], pts[3], pts[2], pts[1]], [uv[0], uv[3], uv[2], uv[1]], n, flow);
                }
            }
        }
    }

    /// The orange marker (stained glass) or an outline box (input) at a cell.
    pub fn marker(&mut self, cell: Pos, glass: bool, color: [u8; 3], lit: bool) {
        let base = [cell[0] as f32, cell[1] as f32, cell[2] as f32];
        if glass {
            // culled against the solid blocks it touches, like the game draws glass: never a
            // face coplanar with a block's (no z-fighting)
            let m = models::marker_model(lit);
            self.add_model(&m, base, Some(cell), None);
            return;
        }
        let t = 0.4;
        let (a, b) = (-0.3, 16.3);
        let mut parts = Vec::new();
        for (from, to) in [
            ([a, a, a], [b, a + t, a + t]),
            ([a, b - t, a], [b, b, a + t]),
            ([a, a, b - t], [b, a + t, b]),
            ([a, b - t, b - t], [b, b, b]),
            ([a, a, a], [a + t, b, a + t]),
            ([b - t, a, a], [b, b, a + t]),
            ([a, a, b - t], [a + t, b, b]),
            ([b - t, a, b - t], [b, b, b]),
            ([a, a, a], [a + t, a + t, b]),
            ([b - t, a, a], [b, a + t, b]),
            ([a, b - t, a], [a + t, b, b]),
            ([b - t, b - t, a], [b, b, b]),
        ] {
            parts.push(Part::new(from, to).all("white_concrete").tint(color).emissive(true).nocull());
        }
        let m = Model { parts, ..Default::default() };
        self.add_model(&m, base, None, None);
    }

    /// An outline box around the cells from `lo` to `hi` (a driven input and its judge devices).
    pub fn marker_span(&mut self, lo: Pos, hi: Pos, color: [u8; 3]) {
        let base = [lo[0] as f32, lo[1] as f32, lo[2] as f32];
        let t = 0.4;
        let a = [-0.3f32; 3];
        let b: [f32; 3] = std::array::from_fn(|k| 16.0 * (hi[k] - lo[k] + 1) as f32 + 0.3);
        let mut parts = Vec::new();
        // the 12 edges: 4 along each axis
        for axis in 0..3 {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for (eu, ev) in [(a[u], a[v]), (b[u] - t, a[v]), (a[u], b[v] - t), (b[u] - t, b[v] - t)] {
                let mut from = [0.0f32; 3];
                let mut to = [0.0f32; 3];
                from[axis] = a[axis];
                to[axis] = b[axis];
                from[u] = eu;
                to[u] = eu + t;
                from[v] = ev;
                to[v] = ev + t;
                parts.push(Part::new(from, to).all("white_concrete").tint(color).emissive(true).nocull());
            }
        }
        let m = Model { parts, ..Default::default() };
        self.add_model(&m, base, None, None);
    }

    /// A flying machine's track (an annotation, no block: [`crate::scene::Look::track`]): thin
    /// bars in the plane just under (or, `normal` = z, behind) the path box `lo`..`hi`: two rails
    /// along `axis` at its sides and a tie at every block, so the machine is seen to fly while
    /// the camera follows it (the judges' worlds are void: nothing else to see it move against).
    pub fn track(&mut self, lo: Pos, hi: Pos, axis: usize, normal: usize, color: [u8; 3]) {
        if axis == normal || axis > 2 || normal > 2 {
            return;
        }
        let w = 3 - axis - normal;
        let t = 0.5;
        let len = 16.0 * (hi[axis] - lo[axis] + 1) as f32;
        let wid = 16.0 * (hi[w] - lo[w] + 1) as f32;
        let bar = |a0: f32, a1: f32, w0: f32, w1: f32| {
            let (mut from, mut to) = ([0.0f32; 3], [0.0f32; 3]);
            from[axis] = a0;
            to[axis] = a1;
            from[w] = w0;
            to[w] = w1;
            from[normal] = -t;
            to[normal] = 0.0;
            Part::new(from, to).all("white_concrete").tint(color).emissive(true).nocull()
        };
        let mut parts = vec![bar(0.0, len, 0.0, t), bar(0.0, len, wid - t, wid)];
        let mut a = 0.0;
        while a <= len + 0.01 {
            parts.push(bar((a - t / 2.0).max(0.0), (a + t / 2.0).min(len), 0.0, wid));
            a += 16.0;
        }
        let m = Model { parts, ..Default::default() };
        self.add_model(&m, [lo[0] as f32, lo[1] as f32, lo[2] as f32], None, None);
    }

    /// A powered output's box filled with translucent red (its outline drawn by [`Builder::marker`]).
    pub fn out_fill(&mut self, cell: Pos) {
        let base = [cell[0] as f32, cell[1] as f32, cell[2] as f32];
        let p = Part::new([-0.2, -0.2, -0.2], [16.2, 16.2, 16.2]).all("out_powered").layer(models::Layer::Translucent).emissive(true).nocull();
        self.add_model(&Model::one(p), base, None, None);
    }

    /// A design's chunk borders (ironspec 2026-11-10: a design with a `chunk:` alignment, its
    /// reference block at the middle of its ranges, `design::ChunkAlign::borders`): the borders of
    /// every chunk its footprint (`lo`..`hi`, x and z) touches, as lines on its floor (`y`: the
    /// bottom of its lowest cells) seen through the blocks, as the game's F3+G shows them. `r`: the
    /// residues of the design x and z that start a chunk.
    pub fn chunk_grid(&mut self, lo: Pos, hi: Pos, y: f32, r: [i32; 2]) {
        const LIFT: f32 = 0.04;
        const W: f32 = 0.12;
        const LINE: [u8; 4] = [255, 214, 51, 235];
        let tile = self.atlas.tile("spawn_spot") as u32 | XRAY;
        // (the border at or below the footprint's low edge, the one at or above its high edge)
        let below = |v: i32, r: i32| v - (v - r).rem_euclid(16);
        let above = |v: i32, r: i32| v + (r - v).rem_euclid(16);
        let (x0, x1) = (below(lo[0], r[0]), above(hi[0] + 1, r[0]));
        let (z0, z1) = (below(lo[2], r[1]), above(hi[2] + 1, r[1]));
        let y = y + LIFT;
        let quad = |b: &mut Self, pts: [[f32; 3]; 4]| {
            let uv = [[0.3, 0.3], [0.3, 0.7], [0.7, 0.7], [0.7, 0.3]];
            let center = [(pts[0][0] + pts[2][0]) / 2.0, y, (pts[0][2] + pts[2][2]) / 2.0];
            // (seen from above and from under it)
            for p in [pts, [pts[0], pts[3], pts[2], pts[1]]] {
                let v: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: p[k], uv: uv[k], layer: tile, color: LINE, light: UNLIT });
                b.mesh.translucent.push(Quad { v, center });
            }
        };
        let (fx0, fx1, fz0, fz1) = (x0 as f32, x1 as f32, z0 as f32, z1 as f32);
        for x in (x0..=x1).step_by(16) {
            let x = x as f32;
            quad(self, [[x - W / 2.0, y, fz0], [x - W / 2.0, y, fz1], [x + W / 2.0, y, fz1], [x + W / 2.0, y, fz0]]);
        }
        for z in (z0..=z1).step_by(16) {
            let z = z as f32;
            quad(self, [[fx0, y, z - W / 2.0], [fx0, y, z + W / 2.0], [fx1, y, z + W / 2.0], [fx1, y, z - W / 2.0]]);
        }
    }

    /// A mob farm's golem spawn area (vjeux 2026-11-08: "Can you make areas continuous for spawn
    /// targets. Also can golems really spawn in the air?"; "What's up with the spawning slots
    /// changing color and the progress bar around the iron golem?"): one steady translucent sheet
    /// in one colour on the floor where a golem's feet would stand (each spawn cell's bottom: the
    /// block under it is what it stands on), only the area's outer edges outlined, seen through the
    /// blocks. No counts and no flash: the golem appearing is the signal. Its colour is no farm
    /// block's ([`SPAWN_SHEET`]: magenta; the blue read as water beside a farm's real water).
    pub fn spawn_sheet(&mut self, cells: &[Pos]) {
        const LIFT: f32 = 0.03;
        const SHEET: [u8; 4] = SPAWN_SHEET;
        const EDGE: [u8; 4] = [255, 255, 255, 235];
        const EDGE_W: f32 = 0.08;
        let tile = self.atlas.tile("spawn_spot") as u32 | XRAY;
        // (each cell once, whichever villagers' searches reach it: one uniform sheet)
        let at: HashSet<Pos> = cells.iter().copied().collect();
        let mut cells: Vec<Pos> = at.iter().copied().collect();
        cells.sort();
        let quad = |b: &mut Self, pts: [[f32; 3]; 4], col: [u8; 4]| {
            let uv = [[0.3, 0.3], [0.3, 0.7], [0.7, 0.7], [0.7, 0.3]];
            let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
            // (seen from above and from under it)
            for p in [pts, [pts[0], pts[3], pts[2], pts[1]]] {
                let v: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: p[k], uv: uv[k], layer: tile, color: col, light: UNLIT });
                b.mesh.translucent.push(Quad { v, center });
            }
        };
        for c in &cells {
            let (x, y, z) = (c[0] as f32, c[1] as f32 + LIFT, c[2] as f32);
            quad(self, [[x, y, z], [x, y, z + 1.0], [x + 1.0, y, z + 1.0], [x + 1.0, y, z]], SHEET);
            // the area's outer edges: a cell's side with no spawn cell beside it at its height
            let e = EDGE_W;
            let y2 = y + 0.005;
            for (d, pts) in [
                ([0, 0, -1], [[x, y2, z], [x, y2, z + e], [x + 1.0, y2, z + e], [x + 1.0, y2, z]]),
                ([0, 0, 1], [[x, y2, z + 1.0 - e], [x, y2, z + 1.0], [x + 1.0, y2, z + 1.0], [x + 1.0, y2, z + 1.0 - e]]),
                ([-1, 0, 0], [[x, y2, z], [x, y2, z + 1.0], [x + e, y2, z + 1.0], [x + e, y2, z]]),
                ([1, 0, 0], [[x + 1.0 - e, y2, z], [x + 1.0 - e, y2, z + 1.0], [x + 1.0, y2, z + 1.0], [x + 1.0, y2, z]]),
            ] {
                if !at.contains(&[c[0] + d[0], c[1], c[2] + d[2]]) {
                    quad(self, pts, EDGE);
                }
            }
        }
    }

    /// An explosion's particles `age` ticks after the blast at `c` (its tick `time`): the game's
    /// emitter and puffs ([`crate::particles`]), the game's sprite frames once loaded (a white
    /// puff until then; a page without them asks for them with the entity textures).
    pub fn explosion(&mut self, c: [f32; 3], time: u64, age: f32) {
        if let Some(g) = &self.atlas.game
            && !g.tex.contains_key("particle/explosion_0")
        {
            g.wants_entities.set(true);
        }
        for p in crate::particles::explosion(c, time, age) {
            let (layer, uv) = self.atlas.sprite(&format!("particle/explosion_{}", p.frame), "explosion_puff");
            let g = (p.grey * 255.0).round() as u8;
            self.mesh.sprites.push(Sprite { pos: p.pos, half: p.half, layer, uv, color: [g, g, g, 255] });
        }
    }

    fn add_model(&mut self, m: &Model, base: [f32; 3], cell: Option<Pos>, yaw: Option<f32>) {
        for (pi, p) in m.parts.iter().enumerate() {
            for (i, f) in p.faces.iter().enumerate() {
                if let Some(f) = f {
                    self.add_face(m, p, pi, i, f, base, cell, yaw);
                }
            }
        }
    }

    /// Whether the neighbour on a still block's side (`c`'s, along `axis` by `s`) hides that side's
    /// face of part `p`: an opaque cube; a translucent block's also the same translucent block
    /// (water, glass).
    fn hidden_against(&self, c: Pos, axis: usize, s: i32, p: &Part) -> bool {
        let mut q = c;
        q[axis] += s;
        self.occupied(q) || (p.layer == Layer::Translucent && self.names.get(&q).is_some_and(|n| self.names.get(&c) == Some(n)))
    }

    #[allow(clippy::too_many_arguments)]
    /// A cell's (shade for the ambient occlusion, light, opaque) at once: one look-up in the
    /// light's box.
    fn probe(&self, c: Pos) -> (f32, (u8, u8), bool) {
        if self.ground.is_some_and(|g| c[1] < g) {
            return (0.2, self.light_at(c), true);
        }
        match &self.light {
            Some(l) => {
                let (opaque, shade_full, light) = l.probe(c);
                (if opaque || shade_full { 0.2 } else { 1.0 }, light, opaque)
            }
            None => {
                let opaque = self.occupied(c);
                (if opaque { 0.2 } else { 1.0 }, (15, 0), opaque)
            }
        }
    }

    /// The game's light at a still block's face corners (`BlockModelLighter`), before the face's
    /// direction shade: smooth for a block that gives off no light and whose model allows it
    /// (each corner the mean of the four cells around it in front of the face: their shade, the
    /// ambient occlusion, and their light), flat otherwise (the light in front of the face). Each
    /// corner's (ambient occlusion, sky light, block light): the lightmap of that light is the
    /// frame's (`Vertex::light`). `pts`: the face's corners in the block (0..1).
    fn face_light(&self, m: &Model, c: Pos, n: [f32; 3], side: Option<(usize, i32)>, pts: &[[f32; 3]; 4]) -> [(f32, f32, f32); 4] {
        let step = |p: Pos, (axis, s): (usize, i32)| -> Pos {
            let mut q = p;
            q[axis] += s;
            q
        };
        let flat = |at: Pos| -> [(f32, f32, f32); 4] {
            let (s, b) = self.light_at(at);
            [(1.0, s as f32, b.max(m.emission) as f32); 4]
        };
        // the face's direction (a turned face: flat at the block's own light)
        let Some(d) = (0..3).find(|&k| n[k].abs() > 0.999).map(|axis| (axis, if n[axis] > 0.0 { 1 } else { -1 })) else {
            return flat(c);
        };
        // on the block's side, or any flat face of a block with a full collision box (a slime
        // block's core: `prepareQuadShape`'s faceCubic): the cells in front of it; inside another
        // block (a slab's top): the block's own cell and its neighbours
        let cubic = side.is_some() || self.light.as_ref().is_some_and(|l| l.collision_full(c));
        let base = if cubic { step(c, d) } else { c };
        if m.no_ao || m.emission > 0 {
            return flat(base);
        }
        let sides = adjacency(d);
        let cells: [Pos; 4] = sides.map(|sd| step(base, sd));
        let probes = cells.map(|q| self.probe(q));
        let shade: [f32; 4] = probes.map(|p| p.0);
        let light: [(u8, u8); 4] = probes.map(|p| p.1);
        // the cell beyond each side, one further out: whether light gets around that corner
        let permeable: [bool; 4] = cells.map(|q| !self.occupied(step(q, d)));
        // a corner's cell, or (light cannot get around it) side 0's values, as the game does
        let corner = |i: usize, j: usize| -> (f32, (u8, u8)) {
            if !permeable[j] && !permeable[i] {
                (shade[0], light[0])
            } else {
                let (s, l, _) = self.probe(step(cells[i], sides[j]));
                (s, l)
            }
        };
        let next = step(c, d);
        let (next_shade, next_light, next_opaque) = self.probe(next);
        let center_light = if cubic || !next_opaque { next_light } else { self.light_at(c) };
        let center_shade = if cubic { next_shade } else { self.shade_at(c) };
        // the four corners of the cell's face, (side i, side j): ambient occlusion and light
        let at_corner = |i: usize, j: usize| -> (f32, (f32, f32)) {
            let (cs, cl) = corner(i, j);
            ((shade[j] + shade[i] + cs + center_shade) * 0.25, crate::light::smooth_blend(light[j], light[i], cl, center_light))
        };
        let (l03, l02, l12, l13) = (at_corner(0, 3), at_corner(0, 2), at_corner(1, 2), at_corner(1, 3));
        // each of the face's corners between them by its place on the cell's face (a part
        // smaller than the block: the game's weights)
        let (a01, a23) = (sides[0].0, sides[2].0);
        pts.map(|q| {
            let toward = |sd: (usize, i32), axis: usize| -> f32 { if sd.1 > 0 { q[axis].clamp(0.0, 1.0) } else { 1.0 - q[axis].clamp(0.0, 1.0) } };
            let (w0, w2) = (toward(sides[0], a01), toward(sides[2], a23));
            let (w1, w3) = (1.0 - w0, 1.0 - w2);
            let ws = [(w0 * w3, l03), (w0 * w2, l02), (w1 * w2, l12), (w1 * w3, l13)];
            let ao: f32 = ws.iter().map(|(w, v)| w * v.0).sum();
            let sky: f32 = ws.iter().map(|(w, v)| w * v.1.0).sum();
            let block: f32 = ws.iter().map(|(w, v)| w * v.1.1).sum();
            (ao, sky, block)
        })
    }

    fn add_face(&mut self, m: &Model, p: &Part, pi: usize, i: usize, f: &Face, base: [f32; 3], cell: Option<Pos>, yaw: Option<f32>) {
        // (a still block's face its neighbour hides: dropped before its corners are worked out,
        // its side known from its model: `Model::sides`, the very side worked out below)
        if let Some(c) = cell
            && f.cull
            && yaw.is_none()
            && self.xf.is_none()
            && let Some(Some((axis, s))) = m.sides.get(pi).map(|sd| sd[i])
            && self.hidden_against(c, axis, s, p)
        {
            return;
        }
        let (corners, auto) = face_corners(p.from, p.to, i);
        let mut pts = face_points(m, p, corners, yaw, self.xf.as_ref());
        let n = norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])));
        // which block side the face lies on, if any (for culling and AO)
        let side = cell.and_then(|_| block_side(&pts, n));
        if let (Some(c), Some((axis, s))) = (cell, side)
            && f.cull
            && self.hidden_against(c, axis, s, p)
        {
            return;
        }
        let sh = if f.emissive || !p.shade { 1.0 } else { shade(n) };
        // the game's light on the face (`BlockModelLighter`): each corner's ambient occlusion
        // times its direction's shade in the colour, its (sky, block) light for the lightmap the
        // frame applies (`Vertex::light`: the day's own); and the corners' brightness at noon's
        // lightmap (the pictures') for the diagonal rule below
        let (lum, light, ao): ([[f32; 3]; 4], [[u8; 4]; 4], [f32; 4]) = if f.emissive {
            // the viewer's own marks: as bright as they are
            ([[1.0; 3]; 4], [UNLIT; 4], [3.0; 4])
        } else {
            match cell {
                Some(c) => {
                    let fl = self.face_light(m, c, n, side, &pts);
                    (fl.map(|(a, _, _)| [a * sh; 3]), fl.map(|(_, s, b)| lit(s, b)), fl.map(|(a, s, b)| crate::light::lightmap(s, b).iter().sum::<f32>() * a * sh))
                }
                None => ([[sh; 3]; 4], [UNLIT; 4], [3.0 * sh; 4]),
            }
        };
        let r = f.uv.unwrap_or(auto);
        // uvlock: the texture keeps the world's orientation: each corner's pixel from its final
        // place on the block, like the automatic UVs of an unturned face
        let (qx, qy) = p.qrot.unwrap_or((m.xrot, m.yrot));
        let locked: Option<[[f32; 2]; 4]> = if p.uvlock && p.rot.is_none() && (qx, qy) != (0, 0) {
            let axis = (0..3).find(|&k| n[k].abs() > 0.999);
            axis.map(|a| {
                pts.map(|q| {
                    let (x, y, z) = (q[0] * 16.0, q[1] * 16.0, q[2] * 16.0);
                    match (a, n[a] > 0.0) {
                        (1, true) => [x, z],
                        (1, false) => [x, 16.0 - z],
                        (2, false) => [16.0 - x, 16.0 - y],
                        (2, true) => [x, 16.0 - y],
                        (0, false) => [z, 16.0 - y],
                        _ => [16.0 - z, 16.0 - y],
                    }
                })
            })
        } else {
            None
        };
        // the game's own texture (a game model's face): its place in the game texture
        let game = self.atlas.game.as_ref().and_then(|g| g.tex.get(f.tile).map(|t| (t.rect, g.width as f32, g.height as f32)));
        let (layer, uv): (u32, [[f32; 2]; 4]) = match game {
            Some((rect, w, h)) => {
                let s = rect[2] as f32 / 16.0;
                let (x0, y0) = (rect[0] as f32, rect[1] as f32);
                let (x1, y1) = (x0 + rect[2] as f32, y0 + rect[3] as f32);
                // inside the sprite by a sliver: nearest sampling never reads the neighbour
                let map = |px: [f32; 2]| -> [f32; 2] { [((x0 + px[0] * s).clamp(x0 + 0.02, x1 - 0.02)) / w, ((y0 + px[1] * s).clamp(y0 + 0.02, y1 - 0.02)) / h] };
                let uv = match locked {
                    Some(l) => l.map(map),
                    None => {
                        let base_px = [[r[0], r[1]], [r[0], r[3]], [r[2], r[3]], [r[2], r[1]]];
                        std::array::from_fn(|k| map(base_px[(k + f.rot as usize) % 4]))
                    }
                };
                (GAME_LAYER, uv)
            }
            None => {
                let tile = self.atlas.tile(f.tile);
                let uv = match locked {
                    Some(l) => l.map(|px| [px[0] / 16.0, px[1] / 16.0]),
                    None => {
                        let [u0, v0, u1, v1] = self.atlas.uv(tile, r);
                        let base_uv = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
                        std::array::from_fn(|k| base_uv[(k + f.rot as usize) % 4])
                    }
                };
                (tile as u32, uv)
            }
        };
        // solid faces: 255, or the white flash's 25% (fs_solid mixes toward white by 1 - alpha);
        // a ghosted cell's faces: see-through, in the translucent pass
        let ghost = cell.is_some_and(|c| self.ghost.contains(&c));
        let alpha = if ghost {
            GHOST_ALPHA
        } else if self.white && p.layer != Layer::Translucent {
            64u8
        } else {
            255u8
        };
        for q in pts.iter_mut() {
            *q = [q[0] + base[0], q[1] + base[1], q[2] + base[2]];
            for k in 0..3 {
                self.mesh.lo[k] = self.mesh.lo[k].min(q[k]);
                self.mesh.hi[k] = self.mesh.hi[k].max(q[k]);
            }
        }
        // a farm in its terrain: grass, leaves and water take their biome's colour there
        let tint = match (&self.biome_tint, cell) {
            (Some(bt), Some(c)) if f.tint != [255, 255, 255] => bt(c, f.tint).unwrap_or(f.tint),
            _ => f.tint,
        };
        let v: [Vertex; 4] = std::array::from_fn(|k| {
            let l = lum[k];
            let c = |t: u8, ch: usize| (t as f32 * l[ch]).round().clamp(0.0, 255.0) as u8;
            let layer = if p.xray { layer | XRAY } else { layer };
            Vertex { pos: pts[k], uv: uv[k], layer, color: [c(tint[0], 0), c(tint[1], 1), c(tint[2], 2), alpha], light: light[k] }
        });
        match p.layer {
            _ if ghost => {
                let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
                self.mesh.translucent.push(Quad { v, center });
            }
            Layer::Translucent => {
                let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
                self.mesh.translucent.push(Quad { v, center });
            }
            _ => {
                // flip the diagonal where AO would make an ugly seam (the game's rule)
                if ao[0] + ao[2] < ao[1] + ao[3] {
                    self.mesh.solid.extend_from_slice(&[v[1], v[2], v[3], v[1], v[3], v[0]]);
                } else {
                    self.mesh.solid.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
                }
            }
        }
    }
}

impl Mesh {
    /// The translucent quads as triangles, farthest from `eye` first.
    pub fn sorted_translucent(&self, eye: [f32; 3]) -> Vec<Vertex> {
        let mut order: Vec<(f32, usize)> = self
            .translucent
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let d = sub(q.center, eye);
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2], i)
            })
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out = Vec::with_capacity(order.len() * 6);
        for (_, i) in order {
            let v = self.translucent[i].v;
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
        out
    }
}

/// The ground: a grass plane under the build (top face at `y`), `r` blocks around its box.
/// Which of 4 equally weighted blockstate variants the game draws at a block (grass blocks' 4
/// turns): `random.setSeed(Mth.getSeed(pos))` then `random.nextInt(4)` with the game's
/// `LegacyRandomSource` (java.util.Random's generator).
pub fn variant_of_4(x: i32, y: i32, z: i32) -> u32 {
    // Mth.getSeed: `x * 3129871` is an int product, widened
    let mut seed: i64 = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    seed = seed.wrapping_mul(seed).wrapping_mul(42317861).wrapping_add(seed.wrapping_mul(11));
    let seed = seed >> 16;
    const MASK: i64 = (1 << 48) - 1;
    let mut s = (seed ^ 0x5DEECE66D) & MASK;
    s = (s.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & MASK;
    let next31 = (s >> (48 - 31)) as i32 as i64;
    // nextInt(bound) for a power of two: (bound * next(31)) >> 31
    ((4 * next31) >> 31) as u32
}

/// A grass top's corner (`cx`, `cz`) lit like the game's smooth lighting: the light of the four
/// air cells around it over the grass (row `ly`: darker under the build, its contact shadow), for
/// the frame's lightmap.
fn grass_corner(light: &dyn Fn(Pos) -> (u8, u8), cx: i32, ly: i32, cz: i32) -> [u8; 4] {
    let at = |x: i32, z: i32| light([x, ly, z]);
    let (sky, block) = crate::light::smooth_blend(at(cx - 1, cz - 1), at(cx, cz - 1), at(cx - 1, cz), at(cx, cz));
    lit(sky, block)
}

/// The open sky's light (sky 15, no block light): the ground's far strips and edges.
pub const OPEN_SKY: [u8; 4] = [240, 0, 1, 0];

/// No light computed: the open sky everywhere.
pub fn daylight(_: Pos) -> (u8, u8) {
    (15, 0)
}

pub fn ground(atlas: &Atlas, lo: [f32; 3], hi: [f32; 3], y: f32, r: f32, light: &dyn Fn(Pos) -> (u8, u8)) -> Vec<Vertex> {
    // a one-block-thick layer of grass blocks: the grass tile repeated once per block on top,
    // dirt under it and grass-block sides around it (seen from below or from the edge too)
    let top = atlas.tile("grass_top") as u32;
    let dirt = atlas.tile("dirt") as u32;
    let side = atlas.tile("grass_side") as u32;
    let x0 = (lo[0] - r).floor();
    let x1 = (hi[0] + r).ceil();
    let z0 = (lo[2] - r).floor();
    let z1 = (hi[2] + r).ceil();
    let yb = y - 1.0;
    let lum = |s: f32| {
        let c = (255.0 * s).round() as u8;
        [c, c, c, 255]
    };
    let mut out = Vec::new();
    let mut quad = |v: [Vertex; 4]| out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    let c = lum(1.0);
    // one quad per grass block near the build: the game turns each block's model by a random
    // quarter (its blockstate's 4 variants, picked from the block's position), so the top texture
    // lines up with the real game's pixel for pixel; farther out (small on screen) a few large
    // quads with the texture repeated
    let gy = (y - 1.0).floor() as i32;
    let (ix0, ix1, iz0, iz1) = (x0 as i32, x1 as i32, z0 as i32, z1 as i32);
    let near = 24;
    let (nx0, nx1, nz0, nz1) = ((lo[0].floor() as i32 - near).max(ix0), (hi[0].ceil() as i32 + near).min(ix1), (lo[2].floor() as i32 - near).max(iz0), (hi[2].ceil() as i32 + near).min(iz1));
    // the game's smooth lighting of a grass top: each corner at the mean sky light of the four air
    // cells around it, over the grass (darker under the build: its contact shadow)
    let corner = |cx: i32, cz: i32| -> [u8; 4] { grass_corner(light, cx, gy + 1, cz) };
    for bz in nz0..nz1 {
        for bx in nx0..nx1 {
            let k = variant_of_4(bx, gy, bz);
            // the texture coordinate of a corner (local dx, dz): the corner turned back by k
            // quarters (the game's y rotation turns clockwise seen from above)
            let uv = |dx: f32, dz: f32| -> [f32; 2] {
                let (mut px, mut pz) = (dx - 0.5, dz - 0.5);
                for _ in 0..k {
                    (px, pz) = (pz, -px);
                }
                [px + 0.5, pz + 0.5]
            };
            let (fx, fz) = (bx as f32, bz as f32);
            quad([
                Vertex { pos: [fx, y, fz], uv: uv(0.0, 0.0), layer: top, color: [255; 4], light: corner(bx, bz) },
                Vertex { pos: [fx, y, fz + 1.0], uv: uv(0.0, 1.0), layer: top, color: [255; 4], light: corner(bx, bz + 1) },
                Vertex { pos: [fx + 1.0, y, fz + 1.0], uv: uv(1.0, 1.0), layer: top, color: [255; 4], light: corner(bx + 1, bz + 1) },
                Vertex { pos: [fx + 1.0, y, fz], uv: uv(1.0, 0.0), layer: top, color: [255; 4], light: corner(bx + 1, bz) },
            ]);
        }
    }
    // the far grass around the near square: four strips, the texture repeated (world coordinates)
    let (nx0, nx1, nz0, nz1) = (nx0 as f32, nx1 as f32, nz0 as f32, nz1 as f32);
    for (a, b) in [([x0, z0], [x1, nz0]), ([x0, nz1], [x1, z1]), ([x0, nz0], [nx0, nz1]), ([nx1, nz0], [x1, nz1])] {
        if b[0] <= a[0] || b[1] <= a[1] {
            continue;
        }
        quad([
            Vertex { pos: [a[0], y, a[1]], uv: [a[0], a[1]], layer: top, color: c, light: OPEN_SKY },
            Vertex { pos: [a[0], y, b[1]], uv: [a[0], b[1]], layer: top, color: c, light: OPEN_SKY },
            Vertex { pos: [b[0], y, b[1]], uv: [b[0], b[1]], layer: top, color: c, light: OPEN_SKY },
            Vertex { pos: [b[0], y, a[1]], uv: [b[0], a[1]], layer: top, color: c, light: OPEN_SKY },
        ]);
    }
    let c = lum(0.5);
    quad([
        Vertex { pos: [x0, yb, z0], uv: [x0, z0], layer: dirt, color: c, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z0], uv: [x1, z0], layer: dirt, color: c, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z1], uv: [x1, z1], layer: dirt, color: c, light: OPEN_SKY },
        Vertex { pos: [x0, yb, z1], uv: [x0, z1], layer: dirt, color: c, light: OPEN_SKY },
    ]);
    // sides, wound to face outward (north -z, south +z, west -x, east +x)
    let (cz, cx) = (lum(0.8), lum(0.6));
    quad([
        Vertex { pos: [x0, y, z0], uv: [x0, 0.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x1, y, z0], uv: [x1, 0.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z0], uv: [x1, 1.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x0, yb, z0], uv: [x0, 1.0], layer: side, color: cz, light: OPEN_SKY },
    ]);
    quad([
        Vertex { pos: [x1, y, z1], uv: [x1, 0.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x0, y, z1], uv: [x0, 0.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x0, yb, z1], uv: [x0, 1.0], layer: side, color: cz, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z1], uv: [x1, 1.0], layer: side, color: cz, light: OPEN_SKY },
    ]);
    quad([
        Vertex { pos: [x0, y, z1], uv: [z1, 0.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x0, y, z0], uv: [z0, 0.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x0, yb, z0], uv: [z0, 1.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x0, yb, z1], uv: [z1, 1.0], layer: side, color: cx, light: OPEN_SKY },
    ]);
    quad([
        Vertex { pos: [x1, y, z0], uv: [z0, 0.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x1, y, z1], uv: [z1, 0.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z1], uv: [z1, 1.0], layer: side, color: cx, light: OPEN_SKY },
        Vertex { pos: [x1, yb, z0], uv: [z0, 1.0], layer: side, color: cx, light: OPEN_SKY },
    ]);
    out
}

/// A TNT duper's ground as it stands (`Session::floor`: real blocks of the world, `lo..=hi`, its
/// top layer at `hi[1]`), every cell but those in `skip` (drawn as blocks: around a crater): the
/// grass tops (one quad per block near the build `near`, with the game's turned variants and
/// smooth sky light like [`ground`]; farther, runs of blocks with the texture repeated), each
/// layer's side at the ground's edge (`sides`: tile names, top layer first) and the bedrock's
/// underside. `light`: the light over the grass (the build's: [`Builder::take_light`]).
pub fn floor(atlas: &Atlas, lo: Pos, hi: Pos, sides: &[&str], skip: &HashSet<Pos>, near: (Pos, Pos), light: &dyn Fn(Pos) -> (u8, u8)) -> Vec<Vertex> {
    let top = atlas.tile("grass_top") as u32;
    let bottom = atlas.tile("bedrock") as u32;
    let lum = |s: f32| {
        let c = (255.0 * s).round() as u8;
        [c, c, c, 255]
    };
    let mut out = Vec::new();
    let mut quad = |v: [Vertex; 4]| out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    let gy = hi[1];
    let y = gy as f32 + 1.0;
    let r = 24;
    let (nx0, nx1, nz0, nz1) = ((near.0[0] - r).max(lo[0]), (near.1[0] + r).min(hi[0]), (near.0[2] - r).max(lo[2]), (near.1[2] + r).min(hi[2]));
    let is_near = |x: i32, z: i32| x >= nx0 && x <= nx1 && z >= nz0 && z <= nz1;
    let corner = |cx: i32, cz: i32| -> [u8; 4] { grass_corner(light, cx, gy + 1, cz) };
    let c = lum(1.0);
    for bz in lo[2]..=hi[2] {
        let mut run: Option<i32> = None;
        for bx in lo[0]..=hi[0] + 1 {
            let far_top = bx <= hi[0] && !is_near(bx, bz) && !skip.contains(&[bx, gy, bz]);
            if far_top {
                run.get_or_insert(bx);
            } else if let Some(a) = run.take() {
                // a run of far grass tops: the texture repeated (world coordinates)
                let (a, b, fz) = (a as f32, bx as f32, bz as f32);
                quad([
                    Vertex { pos: [a, y, fz], uv: [a, fz], layer: top, color: c, light: OPEN_SKY },
                    Vertex { pos: [a, y, fz + 1.0], uv: [a, fz + 1.0], layer: top, color: c, light: OPEN_SKY },
                    Vertex { pos: [b, y, fz + 1.0], uv: [b, fz + 1.0], layer: top, color: c, light: OPEN_SKY },
                    Vertex { pos: [b, y, fz], uv: [b, fz], layer: top, color: c, light: OPEN_SKY },
                ]);
            }
            if bx > hi[0] || !is_near(bx, bz) || skip.contains(&[bx, gy, bz]) {
                continue;
            }
            // a near grass top: the block's turned variant, its corners' sky light
            let k = variant_of_4(bx, gy, bz);
            let uv = |dx: f32, dz: f32| -> [f32; 2] {
                let (mut px, mut pz) = (dx - 0.5, dz - 0.5);
                for _ in 0..k {
                    (px, pz) = (pz, -px);
                }
                [px + 0.5, pz + 0.5]
            };
            let (fx, fz) = (bx as f32, bz as f32);
            quad([
                Vertex { pos: [fx, y, fz], uv: uv(0.0, 0.0), layer: top, color: [255; 4], light: corner(bx, bz) },
                Vertex { pos: [fx, y, fz + 1.0], uv: uv(0.0, 1.0), layer: top, color: [255; 4], light: corner(bx, bz + 1) },
                Vertex { pos: [fx + 1.0, y, fz + 1.0], uv: uv(1.0, 1.0), layer: top, color: [255; 4], light: corner(bx + 1, bz + 1) },
                Vertex { pos: [fx + 1.0, y, fz], uv: uv(1.0, 0.0), layer: top, color: [255; 4], light: corner(bx + 1, bz) },
            ]);
        }
    }
    // the sides at the edge, layer by layer, in runs of cells not drawn as blocks
    let (cz, cx) = (lum(0.8), lum(0.6));
    for gyl in lo[1]..=hi[1] {
        let tile = atlas.tile(sides.get((hi[1] - gyl) as usize).copied().unwrap_or("dirt")) as u32;
        let (yt, yb) = (gyl as f32 + 1.0, gyl as f32);
        // along x: the north (z = lo) and south (z = hi) faces
        for (zc, north) in [(lo[2], true), (hi[2], false)] {
            let mut run: Option<i32> = None;
            for x in lo[0]..=hi[0] + 1 {
                if x <= hi[0] && !skip.contains(&[x, gyl, zc]) {
                    run.get_or_insert(x);
                } else if let Some(a) = run.take() {
                    let (x0, x1) = (a as f32, x as f32);
                    if north {
                        let z = zc as f32;
                        quad([
                            Vertex { pos: [x0, yt, z], uv: [x0, 0.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x1, yt, z], uv: [x1, 0.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x1, yb, z], uv: [x1, 1.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x0, yb, z], uv: [x0, 1.0], layer: tile, color: cz, light: OPEN_SKY },
                        ]);
                    } else {
                        let z = zc as f32 + 1.0;
                        quad([
                            Vertex { pos: [x1, yt, z], uv: [x1, 0.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x0, yt, z], uv: [x0, 0.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x0, yb, z], uv: [x0, 1.0], layer: tile, color: cz, light: OPEN_SKY },
                            Vertex { pos: [x1, yb, z], uv: [x1, 1.0], layer: tile, color: cz, light: OPEN_SKY },
                        ]);
                    }
                }
            }
        }
        // along z: the west (x = lo) and east (x = hi) faces
        for (xc, west) in [(lo[0], true), (hi[0], false)] {
            let mut run: Option<i32> = None;
            for z in lo[2]..=hi[2] + 1 {
                if z <= hi[2] && !skip.contains(&[xc, gyl, z]) {
                    run.get_or_insert(z);
                } else if let Some(a) = run.take() {
                    let (z0, z1) = (a as f32, z as f32);
                    if west {
                        let x = xc as f32;
                        quad([
                            Vertex { pos: [x, yt, z1], uv: [z1, 0.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yt, z0], uv: [z0, 0.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yb, z0], uv: [z0, 1.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yb, z1], uv: [z1, 1.0], layer: tile, color: cx, light: OPEN_SKY },
                        ]);
                    } else {
                        let x = xc as f32 + 1.0;
                        quad([
                            Vertex { pos: [x, yt, z0], uv: [z0, 0.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yt, z1], uv: [z1, 0.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yb, z1], uv: [z1, 1.0], layer: tile, color: cx, light: OPEN_SKY },
                            Vertex { pos: [x, yb, z0], uv: [z0, 1.0], layer: tile, color: cx, light: OPEN_SKY },
                        ]);
                    }
                }
            }
        }
    }
    // the bedrock's underside, in runs
    let (yb, c) = (lo[1] as f32, lum(0.5));
    for bz in lo[2]..=hi[2] {
        let mut run: Option<i32> = None;
        for bx in lo[0]..=hi[0] + 1 {
            if bx <= hi[0] && !skip.contains(&[bx, lo[1], bz]) {
                run.get_or_insert(bx);
            } else if let Some(a) = run.take() {
                let (x0, x1, z0, z1) = (a as f32, bx as f32, bz as f32, bz as f32 + 1.0);
                quad([
                    Vertex { pos: [x0, yb, z0], uv: [x0, z0], layer: bottom, color: c, light: OPEN_SKY },
                    Vertex { pos: [x1, yb, z0], uv: [x1, z0], layer: bottom, color: c, light: OPEN_SKY },
                    Vertex { pos: [x1, yb, z1], uv: [x1, z1], layer: bottom, color: c, light: OPEN_SKY },
                    Vertex { pos: [x0, yb, z1], uv: [x0, z1], layer: bottom, color: c, light: OPEN_SKY },
                ]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brightness of the top face's corners (y = top) of the block at `cell` (a stone's
    /// grey: its red channel over the texture's tint, 0..1): the triangles lying in that face.
    fn top_corners(b: &Builder, cell: Pos) -> Vec<f32> {
        let top = cell[1] as f32 + 1.0;
        let inside = |v: &Vertex| (v.pos[1] - top).abs() < 1e-4 && (v.pos[0] - cell[0] as f32 - 0.5).abs() <= 0.5 && (v.pos[2] - cell[2] as f32 - 0.5).abs() <= 0.5;
        // (as drawn at noon: the colour times the lightmap of its light)
        let noon = crate::daylight::Env::noon();
        let drawn = |v: &Vertex| v.color[0] as f32 / 255.0 * if v.light[2] > 0 { crate::daylight::lightmap(&noon, v.light[0] as f32 / 16.0, v.light[1] as f32 / 16.0)[0] } else { 1.0 };
        b.mesh.solid.chunks(3).filter(|t| t.iter().all(inside)).flatten().map(drawn).collect()
    }

    /// The game's smooth lighting (`BlockModelLighter`): a corner next to an opaque block is 0.8
    /// of the open one (its ambient occlusion: (0.2 + 1 + 1 + 1) / 4), and a face under an
    /// overhang has the cell in front of it one level darker (14: the light of its open
    /// neighbours less one), each corner the mean with the three open cells around it (14.75).
    #[test]
    fn faces_are_lit_like_the_game() {
        let atlas = crate::atlas::Atlas::build();
        let stone = "minecraft:stone";
        let mut b = Builder::new(&atlas);
        b.blocks(&[Item { cell: [0, 0, 0], offset: [0.0; 3], state: stone }, Item { cell: [1, 1, 0], offset: [0.0; 3], state: stone }]);
        let c = top_corners(&b, [0, 0, 0]);
        let (lo, hi) = (c.iter().cloned().fold(f32::MAX, f32::min), c.iter().cloned().fold(f32::MIN, f32::max));
        assert!(!c.is_empty() && (lo / hi - 0.8).abs() < 0.02, "the corners against the block: {c:?}");
        let mut b = Builder::new(&atlas);
        b.blocks(&[Item { cell: [0, 0, 0], offset: [0.0; 3], state: stone }, Item { cell: [0, 2, 0], offset: [0.0; 3], state: stone }]);
        let mut open = Builder::new(&atlas);
        open.blocks(&[Item { cell: [0, 0, 0], offset: [0.0; 3], state: stone }]);
        let (under, lit) = (top_corners(&b, [0, 0, 0]), top_corners(&open, [0, 0, 0]));
        let ratio = under[0] / lit[0];
        let want = crate::light::lightmap(14.75, 0.0)[0];
        assert!(want < 0.99 && (ratio - want).abs() < 0.01, "under the overhang: {under:?} against {lit:?}, want {want}");
    }

    /// A slime block's core is lit from the cells around the block like its outer faces (the
    /// game's `faceCubic` for a block with a full collision box): it was drawn 20% darker, the
    /// slime block's own cell darkening its corners (critique: slime ~6% darker than the game's).
    #[test]
    fn a_slime_blocks_core_is_lit_from_outside() {
        let atlas = crate::atlas::Atlas::build();
        let mut b = Builder::new(&atlas);
        b.blocks(&[Item { cell: [0, 0, 0], offset: [0.0; 3], state: "minecraft:slime_block" }]);
        // the top faces: the core's at 13/16, the outer one at 1
        let top = |y: f32| -> Vec<[u8; 4]> { b.mesh.translucent.iter().filter(|q| q.v.iter().all(|v| (v.pos[1] - y).abs() < 1e-4)).flat_map(|q| q.v.iter().map(|v| v.color)).collect() };
        let (core, outer) = (top(13.0 / 16.0), top(1.0));
        assert!(!core.is_empty() && !outer.is_empty());
        assert!(core.iter().all(|c| *c == outer[0]), "the core's top {core:?} against the outer top {:?}", outer[0]);
    }

    /// Water's sides are seen from inside too (`FluidRenderer` draws their back faces): a lone
    /// source shows its top twice, its bottom once and each side twice.
    #[test]
    fn water_sides_are_seen_from_both_sides() {
        let atlas = crate::atlas::Atlas::build();
        let mut b = Builder::new(&atlas);
        b.blocks(&[Item { cell: [0, 5, 0], offset: [0.0; 3], state: "minecraft:water[level=0]" }]);
        assert_eq!(b.mesh.translucent.len(), 2 + 1 + 4 * 2);
        // against glass the side is one-sided (the game's overlay)
        let mut b = Builder::new(&atlas);
        b.blocks(&[Item { cell: [0, 5, 0], offset: [0.0; 3], state: "minecraft:water[level=0]" }, Item { cell: [1, 5, 0], offset: [0.0; 3], state: "minecraft:glass" }]);
        let flow = atlas.tile("water_flow") as u32;
        let sides = b.mesh.translucent.iter().filter(|q| q.v[0].layer == flow).count();
        assert_eq!(sides, 3 * 2 + 1);
    }
}
