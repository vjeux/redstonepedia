//! Block models: every block state as cuboids ("elements" like the game's JSON models, in
//! pixels 0..16) with a texture tile per face, rotated as a whole by quarter turns (the game's
//! blockstate `x`/`y` rotations). [`model_for`] reads any state string (`minecraft:name[k=v]`):
//! redsim's blocks, the archives' other blocks, and a tinted cube for anything unknown.

use crate::atlas::Atlas;
use redsim::Dir;

/// How a face is blended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Opaque,
    /// Alpha-tested (glass panes' frames, leaves, rails, torches).
    Cutout,
    /// Alpha-blended, drawn after everything else back to front (stained glass, slime, water).
    Translucent,
}

/// One face of a part.
#[derive(Clone, Debug)]
pub struct Face {
    pub tile: &'static str,
    /// Pixel rect (u0, v0, u1, v1) in the tile; None = the game's automatic UVs from the part.
    pub uv: Option<[f32; 4]>,
    /// Texture rotation in quarter turns clockwise.
    pub rot: u8,
    pub tint: [u8; 3],
    /// Hidden when the neighbor on this side is an opaque full block (only for faces on the
    /// block's boundary).
    pub cull: bool,
    /// Full brightness (lit lamps, torches, powered dust).
    pub emissive: bool,
}

impl Face {
    pub fn new(tile: &'static str) -> Face {
        Face { tile, uv: None, rot: 0, tint: [255, 255, 255], cull: true, emissive: false }
    }
}

/// Face indices: the order of `redsim::Dir` (Down, Up, North, South, West, East).
pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const NORTH: usize = 2;
pub const SOUTH: usize = 3;
pub const WEST: usize = 4;
pub const EAST: usize = 5;

/// A rotated part: axis (0 x, 1 y, 2 z), angle in degrees, origin in pixels; `rescale` stretches
/// it back across the other axes by 1 / cos(angle) (the game's crossed plants).
#[derive(Clone, Copy, Debug)]
pub struct PartRot {
    pub axis: u8,
    pub angle: f32,
    pub origin: [f32; 3],
    pub rescale: bool,
}

/// One cuboid ("element").
#[derive(Clone, Debug)]
pub struct Part {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub faces: [Option<Face>; 6],
    pub rot: Option<PartRot>,
    pub layer: Layer,
    /// Shade faces by direction (false: flat, e.g. cross-shaped plants).
    pub shade: bool,
    /// This part's own quarter turns (a multipart model's parts each have theirs), instead of
    /// the model's.
    pub qrot: Option<(u8, u8)>,
    /// The game's uvlock: textures keep the world's orientation through the quarter turns.
    pub uvlock: bool,
    /// The viewer's own mark seen through the blocks (`mesh::XRAY`: drawn at the nearest depth).
    pub xray: bool,
}

impl Part {
    pub fn new(from: [f32; 3], to: [f32; 3]) -> Part {
        Part { from, to, faces: Default::default(), rot: None, layer: Layer::Opaque, shade: true, qrot: None, uvlock: false, xray: false }
    }
    /// Seen through the blocks in front of it (a translucent mark: a mob farm's spawn spots).
    pub fn xray(mut self) -> Part {
        self.xray = true;
        self
    }
    /// The same tile on every face.
    pub fn all(mut self, tile: &'static str) -> Part {
        for f in self.faces.iter_mut() {
            *f = Some(Face::new(tile));
        }
        self
    }
    pub fn face(mut self, i: usize, tile: &'static str) -> Part {
        self.faces[i] = Some(Face::new(tile));
        self
    }
    pub fn no(mut self, i: usize) -> Part {
        self.faces[i] = None;
        self
    }
    pub fn uv(mut self, i: usize, r: [f32; 4]) -> Part {
        if let Some(f) = self.faces[i].as_mut() {
            f.uv = Some(r);
        }
        self
    }
    pub fn rot_uv(mut self, i: usize, k: u8) -> Part {
        if let Some(f) = self.faces[i].as_mut() {
            f.rot = k;
        }
        self
    }
    pub fn tint(mut self, c: [u8; 3]) -> Part {
        for f in self.faces.iter_mut().flatten() {
            f.tint = c;
        }
        self
    }
    pub fn emissive(mut self, on: bool) -> Part {
        for f in self.faces.iter_mut().flatten() {
            f.emissive = on;
        }
        self
    }
    pub fn nocull(mut self) -> Part {
        for f in self.faces.iter_mut().flatten() {
            f.cull = false;
        }
        self
    }
    pub fn layer(mut self, l: Layer) -> Part {
        self.layer = l;
        self
    }
    pub fn rotated(mut self, axis: u8, angle: f32, origin: [f32; 3]) -> Part {
        self.rot = Some(PartRot { axis, angle, origin, rescale: false });
        self
    }
    pub fn flat(mut self) -> Part {
        self.shade = false;
        self
    }
}

/// A block's model: parts, then a whole-model rotation (x quarter turns, then y quarter turns,
/// like the game's blockstate variants: x maps up -> north -> down, y maps north -> east).
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub parts: Vec<Part>,
    pub xrot: u8,
    pub yrot: u8,
    /// An opaque full cube: hides its neighbors' faces against it and darkens corners (AO).
    pub full: bool,
    /// The light the block gives off (the game lights such a block flat, without ambient
    /// occlusion: [`crate::light::emission`]).
    pub emission: u8,
    /// The game's model says `"ambientocclusion": false` (a torch): lit flat.
    pub no_ao: bool,
    /// One of its parts fills the cell (glass, ice, a slime block): for the light it takes.
    pub fills: bool,
    /// Each part's faces' block side (axis, -1 or 1) as a still block draws them (no turn, no
    /// transform: `mesh::Builder::new_model` works them out once), so a face hidden by its
    /// neighbour is dropped before its corners are worked out; empty: worked out per face.
    pub sides: Vec<[Option<(usize, i32)>; 6]>,
}

impl Model {
    pub fn one(p: Part) -> Model {
        Model { parts: vec![p], ..Default::default() }
    }
    fn cube(p: Part) -> Model {
        Model { parts: vec![p], full: true, fills: true, ..Default::default() }
    }
    fn rotated(mut self, (x, y): (u8, u8)) -> Model {
        self.xrot = x;
        self.yrot = y;
        self
    }
}

/// The orange output marker (a block of orange stained glass, as in the real-game pictures); lit
/// (bright, glowing) while the output is on.
pub fn marker_model(lit: bool) -> Model {
    let p = Part::new([0.0, 0.0, 0.0], [16.0, 16.0, 16.0]).all(if lit { "marker_lit" } else { "orange_stained_glass" }).layer(Layer::Translucent);
    Model::one(if lit { p.emissive(true) } else { p })
}

/// A state string's name and properties.
pub struct St<'a> {
    pub name: &'a str,
    props: Vec<(&'a str, &'a str)>,
}

impl<'a> St<'a> {
    pub fn parse(s: &'a str) -> St<'a> {
        let s = s.trim();
        let s = s.strip_prefix("minecraft:").unwrap_or(s);
        let (name, rest) = match s.find('[') {
            Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']')),
            None => (s, ""),
        };
        let props = rest.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
        St { name, props }
    }
    pub fn get(&self, k: &str) -> Option<&'a str> {
        self.props.iter().find(|(kk, _)| *kk == k).map(|(_, v)| *v)
    }
    pub fn b(&self, k: &str, d: bool) -> bool {
        self.get(k).map(|v| v == "true").unwrap_or(d)
    }
    pub fn i(&self, k: &str, d: i32) -> i32 {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    pub fn dir(&self, k: &str, d: Dir) -> Dir {
        self.get(k).and_then(Dir::from_name).unwrap_or(d)
    }
}

/// Rotation of a model built facing north (front on the north face) to face `d`.
pub fn rot_north(d: Dir) -> (u8, u8) {
    match d {
        Dir::North => (0, 0),
        Dir::East => (0, 1),
        Dir::South => (0, 2),
        Dir::West => (0, 3),
        Dir::Up => (3, 0),
        Dir::Down => (1, 0),
    }
}

/// Rotation of a model built facing up (front on the top face) to face `d`.
pub fn rot_up(d: Dir) -> (u8, u8) {
    match d {
        Dir::Up => (0, 0),
        Dir::Down => (2, 0),
        Dir::North => (1, 0),
        Dir::East => (1, 1),
        Dir::South => (1, 2),
        Dir::West => (1, 3),
    }
}

/// Quarter turns about y taking north to `d` (horizontal).
fn yturns(d: Dir) -> u8 {
    match d {
        Dir::East => 1,
        Dir::South => 2,
        Dir::West => 3,
        _ => 0,
    }
}

const FULL: ([f32; 3], [f32; 3]) = ([0.0, 0.0, 0.0], [16.0, 16.0, 16.0]);

fn cube_all(tile: &'static str) -> Model {
    Model::cube(Part::new(FULL.0, FULL.1).all(tile))
}

/// Side tile on the four sides, `end` on top and bottom.
fn cube_column(side: &'static str, end: &'static str) -> Model {
    Model::cube(Part::new(FULL.0, FULL.1).all(side).face(UP, end).face(DOWN, end))
}

/// The redstone dust color for a power level (`RedStoneWireBlock.getColorForPower`).
pub fn dust_color(power: i32) -> [u8; 3] {
    let f = power as f32 / 15.0;
    let r = f * 0.6 + if f > 0.0 { 0.4 } else { 0.3 };
    let g = (f * f * 0.7 - 0.5).clamp(0.0, 1.0);
    let b = (f * f * 0.6 - 0.7).clamp(0.0, 1.0);
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

/// A stable pastel color for a name (unknown blocks).
pub fn name_color(name: &str) -> [u8; 3] {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    [100 + (h & 0x7f) as u8, 100 + ((h >> 8) & 0x7f) as u8, 100 + ((h >> 16) & 0x7f) as u8]
}

/// The model of a block state (`None` for air and invisible blocks): the game's own model when
/// its models are loaded (`atlas.game`), else the viewer's.
pub fn model_for(state: &str, atlas: &Atlas) -> Option<Model> {
    model_at(state, atlas, None)
}

/// [`model_for`] for a block at `pos` (the game draws some blocks as one of several variants
/// picked from their position: stone, grass, dirt...).
pub fn model_at(state: &str, atlas: &Atlas, pos: Option<[i32; 3]>) -> Option<Model> {
    let st = St::parse(state);
    if let Some(g) = &atlas.game
        && !matches!(st.name, "air" | "cave_air" | "void_air" | "moving_piston" | "barrier" | "light" | "structure_void" | "water" | "lava" | "bubble_column")
        && let Some(m) = g.models.model(state, pos)
    {
        return Some(m);
    }
    hand_model(state, atlas)
}

/// The viewer's own model of a block state.
pub fn hand_model(state: &str, atlas: &Atlas) -> Option<Model> {
    let st = St::parse(state);
    let name = st.name;
    let m = match name {
        "air" | "cave_air" | "void_air" | "moving_piston" | "barrier" | "light" | "structure_void" => return None,
        "redstone_wire" => dust(&st),
        "redstone_torch" => torch(st.b("lit", true), None),
        "redstone_wall_torch" => torch(st.b("lit", true), Some(st.dir("facing", Dir::North))),
        "lever" => lever(&st),
        "stone_button" | "oak_button" => button(&st, name == "oak_button"),
        "repeater" => repeater(&st),
        "comparator" => comparator(&st),
        "observer" => observer(&st),
        "redstone_lamp" => {
            let lit = st.b("lit", false);
            let mut m = cube_all(if lit { "lamp_on" } else { "lamp_off" });
            m.parts[0] = m.parts[0].clone().emissive(lit);
            m
        }
        n if n.contains("copper_bulb") => {
            let k = if n.contains("exposed") {
                1
            } else if n.contains("weathered") {
                2
            } else if n.contains("oxidized") {
                3
            } else {
                0
            };
            let lit = st.b("lit", false);
            const BULB: [[&str; 4]; 4] = [
                ["bulb_0", "bulb_0_p", "bulb_0_lit", "bulb_0_lit_p"],
                ["bulb_1", "bulb_1_p", "bulb_1_lit", "bulb_1_lit_p"],
                ["bulb_2", "bulb_2_p", "bulb_2_lit", "bulb_2_lit_p"],
                ["bulb_3", "bulb_3_p", "bulb_3_lit", "bulb_3_lit_p"],
            ];
            let tile = BULB[k][lit as usize * 2 + st.b("powered", false) as usize];
            let mut m = cube_all(tile);
            m.parts[0] = m.parts[0].clone().emissive(lit);
            m
        }
        "note_block" => cube_all("note_block"),
        "target" => cube_column("target_side", "target_top"),
        "iron_trapdoor" | "oak_trapdoor" | "waxed_copper_trapdoor" | "copper_trapdoor" => trapdoor(&st),
        "iron_door" | "oak_door" => door(&st),
        "oak_fence_gate" => fence_gate(&st),
        "powered_rail" | "activator_rail" | "rail" | "detector_rail" => rail(&st),
        "dispenser" | "dropper" => {
            let facing = st.dir("facing", Dir::North);
            if matches!(facing, Dir::Up | Dir::Down) {
                // the game's orientable_vertical: the hole on the facing side, furnace top all around
                let front = if name == "dropper" { "dropper_front_vertical" } else { "dispenser_front_vertical" };
                let (f, back) = if facing == Dir::Up { (UP, DOWN) } else { (DOWN, UP) };
                return Some(Model::cube(Part::new(FULL.0, FULL.1).all("furnace_top").face(f, front).face(back, "furnace_top")));
            }
            let front = if name == "dropper" { "dropper_front" } else { "dispenser_front" };
            Model::cube(Part::new(FULL.0, FULL.1).all("furnace_side").face(UP, "furnace_top").face(DOWN, "furnace_top").face(NORTH, front)).rotated(rot_north(facing))
        }
        "crafter" => {
            let on = st.b("triggered", false) || st.b("crafting", false);
            let o = st.get("orientation").unwrap_or("north_up");
            let front = Dir::from_name(o.split('_').next().unwrap_or("north")).unwrap_or(Dir::North);
            Model::cube(Part::new(FULL.0, FULL.1).all("crafter_side").face(UP, "crafter_top").face(DOWN, "furnace_top").face(NORTH, if on { "crafter_front_on" } else { "crafter_front" })).rotated(rot_north(front))
        }
        "hopper" => hopper(&st),
        "smooth_stone_slab" => match st.get("type").unwrap_or("bottom") {
            "double" => cube_column("smooth_stone_slab_side", "smooth_stone"),
            t => {
                let (y0, y1) = if t == "top" { (8.0, 16.0) } else { (0.0, 8.0) };
                Model::one(Part::new([0.0, y0, 0.0], [16.0, y1, 16.0]).all("smooth_stone_slab_side").face(UP, "smooth_stone").face(DOWN, "smooth_stone"))
            }
        },
        "oak_leaves" => {
            let mut m = cube_all("oak_leaves");
            m.parts[0] = m.parts[0].clone().layer(Layer::Cutout).nocull();
            m.full = false;
            m
        }
        "scaffolding" => Model::one(Part::new(FULL.0, FULL.1).all("scaffolding_side").face(UP, "scaffolding_top").face(DOWN, "scaffolding_top").layer(Layer::Cutout).nocull()),
        "oak_log" | "oak_wood" => {
            let axis = st.get("axis").unwrap_or("y");
            let m = cube_column("oak_log", if name == "oak_wood" { "oak_log" } else { "oak_log_top" });
            match axis {
                "x" => m.rotated((1, 1)),
                "z" => m.rotated((1, 0)),
                _ => m,
            }
        }
        "skeleton_skull" => {
            let r = st.i("rotation", 0) as f32 * 22.5;
            Model::one(Part::new([4.0, 0.0, 4.0], [12.0, 8.0, 12.0]).all("skull_plain").face(NORTH, "skull").nocull().rotated(1, -r, [8.0, 0.0, 8.0]))
        }
        "skeleton_wall_skull" => Model::one(Part::new([4.0, 4.0, 0.0], [12.0, 12.0, 8.0]).all("skull_plain").face(NORTH, "skull").nocull()).rotated((0, yturns(st.dir("facing", Dir::North)))),
        "cake" => {
            let bites = st.i("bites", 0) as f32;
            Model::one(Part::new([1.0 + 2.0 * bites, 0.0, 1.0], [15.0, 8.0, 15.0]).all("cake_side").face(UP, "cake_top").face(DOWN, "cake_top").nocull())
        }
        "end_portal_frame" => {
            let mut parts = vec![Part::new([0.0, 0.0, 0.0], [16.0, 13.0, 16.0]).all("end_portal_frame_side").face(UP, "end_portal_frame_top").face(DOWN, "end_stone")];
            if st.b("eye", false) {
                parts.push(Part::new([4.0, 13.0, 4.0], [12.0, 16.0, 12.0]).all("ender_eye").nocull());
            }
            Model { parts, ..Default::default() }
        }
        "composter" => {
            let level = st.i("level", 0).clamp(0, 8) as f32;
            let mut parts = vec![
                Part::new([0.0, 0.0, 0.0], [16.0, 2.0, 16.0]).all("composter_side"),
                Part::new([0.0, 0.0, 0.0], [2.0, 16.0, 16.0]).all("composter_side"),
                Part::new([14.0, 0.0, 0.0], [16.0, 16.0, 16.0]).all("composter_side"),
                Part::new([2.0, 0.0, 0.0], [14.0, 16.0, 2.0]).all("composter_side"),
                Part::new([2.0, 0.0, 14.0], [14.0, 16.0, 16.0]).all("composter_side"),
            ];
            if level > 0.0 {
                parts.push(Part::new([2.0, 2.0, 2.0], [14.0, 2.0 + level.min(7.0) * 1.75, 14.0]).face(UP, "compost").nocull());
            }
            Model { parts, ..Default::default() }
        }
        "respawn_anchor" => cube_column("respawn_anchor_side", "respawn_anchor_top"),
        "piston" | "sticky_piston" => piston(&st, name == "sticky_piston"),
        "piston_head" => piston_head(&st),
        "sculk_sensor" | "calibrated_sculk_sensor" => sculk(&st, name == "calibrated_sculk_sensor"),
        "sandstone" => Model::cube(Part::new(FULL.0, FULL.1).all("sandstone_side").face(UP, "sandstone_top").face(DOWN, "sandstone_top")),
        "dried_kelp_block" => cube_column("dried_kelp_side", "dried_kelp_top"),
        "bookshelf" => cube_column("bookshelf", "oak_planks"),
        "crafting_table" => Model::cube(Part::new(FULL.0, FULL.1).all("crafting_table_side").face(UP, "crafting_table_top").face(DOWN, "oak_planks")),
        "pumpkin" => cube_column("pumpkin_side", "pumpkin_top"),
        "jukebox" => cube_column("jukebox_side", "jukebox_top"),
        "podzol" => Model::cube(Part::new(FULL.0, FULL.1).all("dirt").face(UP, "podzol_top")),
        "snow_block" => cube_all("snow"),
        "glass" => {
            let mut m = cube_all("glass");
            m.parts[0] = m.parts[0].clone().layer(Layer::Cutout);
            m.full = false;
            m
        }
        "tinted_glass" | "white_stained_glass" | "black_stained_glass" | "orange_stained_glass" | "lime_stained_glass" => {
            let tile: &'static str = match name {
                "tinted_glass" => "tinted_glass",
                "white_stained_glass" => "white_stained_glass",
                "black_stained_glass" => "black_stained_glass",
                "lime_stained_glass" => "lime_stained_glass",
                _ => "orange_stained_glass",
            };
            let mut m = cube_all(tile);
            m.parts[0] = m.parts[0].clone().layer(Layer::Translucent);
            m.full = false;
            m
        }
        "slime_block" | "honey_block" => {
            let tile = if name == "slime_block" { "slime_block" } else { "honey_block" };
            let inner = Part::new([3.0, 3.0, 3.0], [13.0, 13.0, 13.0]).all(tile).nocull().layer(Layer::Translucent);
            let outer = Part::new(FULL.0, FULL.1).all(tile).layer(Layer::Translucent);
            Model { parts: vec![inner, outer], fills: true, ..Default::default() }
        }
        "waxed_copper_grate" | "copper_grate" => {
            let mut m = cube_all("waxed_copper_grate");
            m.parts[0] = m.parts[0].clone().layer(Layer::Cutout);
            m.full = false;
            m
        }
        "glowstone" | "sea_lantern" | "shroomlight" => {
            let tile: &'static str = match name {
                "glowstone" => "glowstone",
                "sea_lantern" => "sea_lantern",
                _ => "shroomlight",
            };
            let mut m = cube_all(tile);
            m.parts[0] = m.parts[0].clone().emissive(true);
            m
        }
        "water" | "lava" | "bubble_column" => {
            // the game's fluid heights: a source 8/9 of a block, flowing water (8 - level)/9, a
            // falling column (level 8+) and a bubble column full (the builder also fills a cell
            // under another water cell)
            let level = st.i("level", 0);
            let h = if name == "bubble_column" || level >= 8 { 16.0 } else if level == 0 { 16.0 * 8.0 / 9.0 } else { (16.0 * (8 - level.min(7)) as f32 / 9.0).max(1.0) };
            if name == "lava" {
                // lava: its still texture, in the solid layer (`ItemBlockRenderTypes.getRenderLayer`:
                // only water is translucent), lit by its own light 15 (`LiquidBlockRenderer`)
                return Some(Model { parts: vec![Part::new([0.0, 0.0, 0.0], [16.0, h, 16.0]).all("lava").emissive(true)], full: false, ..Default::default() });
            }
            Model::one(Part::new([0.0, 0.0, 0.0], [16.0, h, 16.0]).all("water").layer(Layer::Translucent))
        }
        "magma_block" => {
            let mut m = cube_all("magma");
            m.parts[0] = m.parts[0].clone().emissive(true);
            m
        }
        "big_dripleaf" => {
            let tilt = match st.get("tilt").unwrap_or("none") {
                "unstable" => 10.0,
                "partial" => 22.5,
                "full" => 45.0,
                _ => 0.0,
            };
            // the leaf tips down toward `facing` (built for north) around its back edge
            let leaf = Part::new([0.0, 15.0, 0.0], [16.0, 15.5, 16.0]).face(UP, "dripleaf_top").face(DOWN, "dripleaf_top").nocull().rotated(0, tilt, [8.0, 15.0, 16.0]);
            let stem = Part::new([7.0, 0.0, 9.0], [9.0, 15.0, 11.0]).all("dripleaf_top").nocull();
            Model { parts: vec![leaf, stem], ..Default::default() }.rotated((0, yturns(st.dir("facing", Dir::North))))
        }
        "tripwire" => {
            let (ns, ew) = (st.b("north", false) || st.b("south", false), st.b("east", false) || st.b("west", false));
            let y = if st.b("attached", false) { 1.5 } else { 1.0 };
            let line = |rot: u8| Part::new([0.0, y, 0.0], [16.0, y, 16.0]).face(UP, "tripwire").face(DOWN, "tripwire").rot_uv(UP, rot).rot_uv(DOWN, rot).nocull().layer(Layer::Cutout).flat();
            let mut parts = Vec::new();
            if ew || !ns {
                parts.push(line(0));
            }
            if ns {
                parts.push(line(1));
            }
            Model { parts, ..Default::default() }
        }
        "tripwire_hook" => {
            let f = st.dir("facing", Dir::North);
            let parts = vec![
                Part::new([6.2, 0.0, 14.0], [9.8, 9.0, 16.0]).all("oak_planks").nocull(),
                Part::new([7.2, 6.0, 10.0], [8.8, 7.5, 14.0]).all("iron_block").nocull(),
                Part::new([7.0, 5.0, 8.0], [9.0, 9.0, 10.0]).all("iron_block").nocull(),
            ];
            Model { parts, ..Default::default() }.rotated((0, yturns(f)))
        }
        n if n.ends_with("_wall_banner") => {
            let color = n.trim_end_matches("_wall_banner");
            let tint = match color {
                "white" => [235, 235, 235],
                "red" => [180, 40, 40],
                "black" => [30, 30, 30],
                "blue" => [50, 70, 170],
                _ => name_color(color),
            };
            Model { parts: vec![Part::new([1.0, -14.0, 14.0], [15.0, 14.0, 15.0]).all("white_wool").tint(tint).nocull(), Part::new([0.0, 13.0, 13.5], [16.0, 15.0, 15.5]).all("oak_planks").nocull()], ..Default::default() }.rotated((0, yturns(st.dir("facing", Dir::North))))
        }
        "stone_pressure_plate" | "oak_pressure_plate" | "light_weighted_pressure_plate" | "heavy_weighted_pressure_plate" => {
            let down = st.b("powered", false) || st.i("power", 0) > 0;
            let tile = match name {
                "oak_pressure_plate" => "oak_planks",
                "light_weighted_pressure_plate" => "gold_block",
                "heavy_weighted_pressure_plate" => "iron_block",
                _ => "stone",
            };
            Model::one(Part::new([1.0, 0.0, 1.0], [15.0, if down { 0.5 } else { 1.0 }, 15.0]).all(tile))
        }
        "daylight_detector" => Model::one(Part::new([0.0, 0.0, 0.0], [16.0, 6.0, 16.0]).all("oak_planks").face(UP, "daylight_detector_top")),
        "sand" | "red_sand" | "gravel" | "white_concrete_powder" => cube_all(match name {
            "sand" => "sand",
            "red_sand" => "red_sand",
            "gravel" => "gravel",
            _ => "white_concrete_powder",
        }),
        n => {
            // plain blocks named after their tile; anything else: a tinted cube
            match atlas.names.get_key_value(n) {
                Some((k, _)) => cube_all(k),
                None => {
                    let fallback = if n.ends_with("_planks") {
                        "oak_planks"
                    } else if n.ends_with("_wool") {
                        "white_wool"
                    } else if n.ends_with("_concrete") {
                        "white_concrete"
                    } else if n.contains("deepslate") {
                        "blackstone"
                    } else if n.contains("quartz") {
                        "quartz_block"
                    } else if n.contains("stone") || n.contains("basalt") {
                        "stone"
                    } else {
                        "generic"
                    };
                    let mut m = cube_all(fallback);
                    if fallback == "generic" || n.ends_with("_wool") || n.ends_with("_concrete") || n.ends_with("_terracotta") {
                        m.parts[0] = m.parts[0].clone().tint(name_color(n));
                    }
                    m
                }
            }
        }
    };
    Some(m)
}

/// Redstone dust: a dot or lines toward the connected sides, walls for `up` sides.
fn dust(st: &St) -> Model {
    let power = st.i("power", 0);
    let tint = dust_color(power);
    let lit = power > 0;
    let side = |k: &str| st.get(k).unwrap_or("none");
    let (n, e, s, w) = (side("north"), side("east"), side("south"), side("west"));
    let conn = |v: &str| v != "none";
    let y = 0.25;
    let mut parts = Vec::new();
    let flat = |from: [f32; 3], to: [f32; 3], tile: &'static str, rot: u8| {
        Part::new(from, to).face(UP, tile).uv(UP, [from[0], from[2], to[0], to[2]]).rot_uv(UP, rot).tint(tint).emissive(lit).nocull().layer(Layer::Cutout).flat()
    };
    let any = conn(n) || conn(e) || conn(s) || conn(w);
    let ns = conn(n) || conn(s);
    let ew = conn(e) || conn(w);
    if !any {
        parts.push(flat([0.0, y, 0.0], [16.0, y, 16.0], "dust_dot", 0));
    } else {
        if !(ns && !ew || ew && !ns) {
            parts.push(flat([0.0, y, 0.0], [16.0, y, 16.0], "dust_dot", 0));
        }
        // half lines toward each connected side (the line tile runs along v: north-south)
        if conn(n) {
            parts.push(flat([0.0, y, 0.0], [16.0, y, 8.0], "dust_line", 0));
        }
        if conn(s) {
            parts.push(flat([0.0, y, 8.0], [16.0, y, 16.0], "dust_line", 0));
        }
        if conn(w) {
            parts.push(Part::new([0.0, y, 0.0], [8.0, y, 16.0]).face(UP, "dust_line").uv(UP, [0.0, 0.0, 16.0, 8.0]).rot_uv(UP, 1).tint(tint).emissive(lit).nocull().layer(Layer::Cutout).flat());
        }
        if conn(e) {
            parts.push(Part::new([8.0, y, 0.0], [16.0, y, 16.0]).face(UP, "dust_line").uv(UP, [0.0, 8.0, 16.0, 16.0]).rot_uv(UP, 1).tint(tint).emissive(lit).nocull().layer(Layer::Cutout).flat());
        }
    }
    // up the side of the block next to it
    let o = 0.25;
    for (v, face, from, to) in [
        (n, SOUTH, [0.0, 0.0, o], [16.0, 16.0, o]),
        (s, NORTH, [0.0, 0.0, 16.0 - o], [16.0, 16.0, 16.0 - o]),
        (w, EAST, [o, 0.0, 0.0], [o, 16.0, 16.0]),
        (e, WEST, [16.0 - o, 0.0, 0.0], [16.0 - o, 16.0, 16.0]),
    ] {
        if v == "up" {
            parts.push(Part::new(from, to).face(face, "dust_line").uv(face, [0.0, 0.0, 16.0, 16.0]).tint(tint).emissive(lit).nocull().layer(Layer::Cutout).flat());
        }
    }
    Model { parts, ..Default::default() }
}

/// A torch standing (`wall` = None) or on the wall of the block behind `facing`.
fn torch(lit: bool, wall: Option<Dir>) -> Model {
    let tile = if lit { "torch_on" } else { "torch_off" };
    let stick = |p: Part| p.face(NORTH, tile).face(SOUTH, tile).face(WEST, tile).face(EAST, tile).face(UP, tile).uv(UP, [7.0, 6.0, 9.0, 8.0]).nocull().layer(Layer::Cutout).emissive(lit);
    match wall {
        None => Model::one(stick(Part::new([7.0, 0.0, 7.0], [9.0, 10.0, 9.0])).uv(NORTH, [7.0, 6.0, 9.0, 16.0]).uv(SOUTH, [7.0, 6.0, 9.0, 16.0]).uv(WEST, [7.0, 6.0, 9.0, 16.0]).uv(EAST, [7.0, 6.0, 9.0, 16.0])),
        Some(f) => {
            // built for facing north: against the south wall, leaning north
            let p = stick(Part::new([7.0, 3.5, 15.0], [9.0, 13.5, 17.0]))
                .uv(NORTH, [7.0, 6.0, 9.0, 16.0])
                .uv(SOUTH, [7.0, 6.0, 9.0, 16.0])
                .uv(WEST, [7.0, 6.0, 9.0, 16.0])
                .uv(EAST, [7.0, 6.0, 9.0, 16.0])
                .rotated(0, -22.5, [8.0, 3.5, 16.0]);
            Model::one(p).rotated((0, yturns(f)))
        }
    }
}

/// face=floor|wall|ceiling and facing: the rotation of a floor-built attached block.
fn attached_rot(st: &St) -> (u8, u8) {
    let f = st.dir("facing", Dir::North);
    match st.get("face").unwrap_or("wall") {
        "floor" => (0, yturns(f)),
        "ceiling" => (2, (yturns(f) + 2) % 4),
        _ => (1, yturns(f)),
    }
}

fn lever(st: &St) -> Model {
    let on = st.b("powered", false);
    let base = Part::new([5.0, 0.0, 4.0], [11.0, 3.0, 12.0]).all("cobblestone").nocull();
    let handle = Part::new([7.0, 1.0, 7.0], [9.0, 11.0, 9.0])
        .face(NORTH, "lever_handle")
        .face(SOUTH, "lever_handle")
        .face(WEST, "lever_handle")
        .face(EAST, "lever_handle")
        .face(UP, "lever_handle")
        .uv(UP, [7.0, 6.0, 9.0, 8.0])
        .uv(NORTH, [7.0, 6.0, 9.0, 16.0])
        .uv(SOUTH, [7.0, 6.0, 9.0, 16.0])
        .uv(WEST, [7.0, 6.0, 9.0, 16.0])
        .uv(EAST, [7.0, 6.0, 9.0, 16.0])
        .nocull()
        .layer(Layer::Cutout)
        .rotated(0, if on { -45.0 } else { 45.0 }, [8.0, 1.0, 8.0]);
    Model { parts: vec![base, handle], ..Default::default() }.rotated(attached_rot(st))
}

fn button(st: &St, wood: bool) -> Model {
    let h = if st.b("powered", false) { 1.0 } else { 2.0 };
    let tile = if wood { "oak_planks" } else { "stone" };
    Model::one(Part::new([5.0, 0.0, 6.0], [11.0, h, 10.0]).all(tile).nocull()).rotated(attached_rot(st))
}

/// A small torch on a diode (center x, z in pixels, height above the slab).
fn diode_torch(x: f32, z: f32, h: f32, lit: bool) -> Part {
    let tile = if lit { "torch_on" } else { "torch_off" };
    Part::new([x - 1.0, 2.0, z - 1.0], [x + 1.0, 2.0 + h, z + 1.0])
        .face(NORTH, tile)
        .face(SOUTH, tile)
        .face(WEST, tile)
        .face(EAST, tile)
        .face(UP, tile)
        .uv(UP, [7.0, 6.0, 9.0, 8.0])
        .uv(NORTH, [7.0, 6.0, 9.0, 6.0 + h])
        .uv(SOUTH, [7.0, 6.0, 9.0, 6.0 + h])
        .uv(WEST, [7.0, 6.0, 9.0, 6.0 + h])
        .uv(EAST, [7.0, 6.0, 9.0, 6.0 + h])
        .nocull()
        .layer(Layer::Cutout)
        .emissive(lit)
}

/// Built for facing north: the input on the north side, the output on the south side.
fn diode_slab(top: &'static str) -> Part {
    Part::new([0.0, 0.0, 0.0], [16.0, 2.0, 16.0]).all("smooth_stone_slab_side").face(UP, top).face(DOWN, "smooth_stone").rot_uv(UP, 2)
}

fn repeater(st: &St) -> Model {
    let on = st.b("powered", false);
    let delay = st.i("delay", 1).clamp(1, 4) as f32;
    let z = 9.0 - 2.0 * (delay - 1.0);
    let mut parts = vec![diode_slab(if on { "repeater_top_on" } else { "repeater_top" }), diode_torch(8.0, 13.0, 5.0, on)];
    if st.b("locked", false) {
        parts.push(Part::new([2.0, 2.0, z - 1.0], [14.0, 4.0, z + 1.0]).all("bedrock").nocull());
    } else {
        parts.push(diode_torch(8.0, z, 5.0, on));
    }
    Model { parts, ..Default::default() }.rotated((0, yturns(st.dir("facing", Dir::North))))
}

fn comparator(st: &St) -> Model {
    let on = st.b("powered", false);
    let subtract = st.get("mode") == Some("subtract");
    let parts = vec![
        diode_slab(if on { "comparator_top_on" } else { "comparator_top" }),
        diode_torch(4.5, 4.0, 5.0, on),
        diode_torch(11.5, 4.0, 5.0, on),
        diode_torch(8.0, 13.0, if subtract { 3.0 } else { 2.0 }, subtract),
    ];
    Model { parts, ..Default::default() }.rotated((0, yturns(st.dir("facing", Dir::North))))
}

fn observer(st: &St) -> Model {
    let on = st.b("powered", false);
    let p = Part::new(FULL.0, FULL.1)
        .face(NORTH, "observer_front")
        .face(SOUTH, if on { "observer_back_on" } else { "observer_back" })
        .face(WEST, "observer_side")
        .face(EAST, "observer_side")
        .uv(EAST, [16.0, 0.0, 0.0, 16.0])
        .face(UP, "observer_top")
        .face(DOWN, "observer_top")
        .uv(DOWN, [0.0, 16.0, 16.0, 0.0]);
    let mut p = p;
    if on {
        if let Some(f) = p.faces[SOUTH].as_mut() {
            f.emissive = true;
        }
    }
    Model::cube(p).rotated(rot_north(st.dir("facing", Dir::South)))
}

fn trapdoor(st: &St) -> Model {
    let tile = if st.name.starts_with("iron") { "iron_trapdoor" } else { "oak_trapdoor" };
    let tint = if st.name.contains("copper") { [235, 150, 110] } else { [255, 255, 255] };
    let open = st.b("open", false);
    let top = st.get("half") == Some("top");
    let f = st.dir("facing", Dir::North);
    let p = if open {
        // against the side opposite `facing` (built for north: the south side)
        Part::new([0.0, 0.0, 13.0], [16.0, 16.0, 16.0])
    } else if top {
        Part::new([0.0, 13.0, 0.0], [16.0, 16.0, 16.0])
    } else {
        Part::new([0.0, 0.0, 0.0], [16.0, 3.0, 16.0])
    };
    Model::one(p.all(tile).tint(tint).layer(Layer::Cutout)).rotated((0, yturns(f)))
}

fn door(st: &St) -> Model {
    let iron = st.name.starts_with("iron");
    let upper = st.get("half") == Some("upper");
    let tile = match (iron, upper) {
        (true, true) => "iron_door_top",
        (true, false) => "iron_door_bottom",
        (false, true) => "oak_door_top",
        (false, false) => "oak_door_bottom",
    };
    let f = st.dir("facing", Dir::East);
    let open = st.b("open", false);
    let right = st.get("hinge") == Some("right");
    // `DoorBlock.getShape`: closed = against the side opposite facing; open = turned to the hinge
    let side = if !open {
        f.opposite()
    } else if right {
        f.counter_clockwise().opposite()
    } else {
        f.clockwise().opposite()
    };
    // a 3-thick slab against `side`, built against the south side
    let p = Part::new([0.0, 0.0, 13.0], [16.0, 16.0, 16.0]).all(tile).layer(Layer::Cutout);
    let turns = (yturns(side.opposite()) + 0) % 4;
    Model::one(p).rotated((0, turns))
}

fn fence_gate(st: &St) -> Model {
    let open = st.b("open", false);
    let f = st.dir("facing", Dir::North);
    let y0 = if st.b("in_wall", false) { -3.0 } else { 0.0 };
    let mut parts = vec![
        Part::new([0.0, 5.0 + y0, 7.0], [2.0, 16.0 + y0, 9.0]).all("oak_planks"),
        Part::new([14.0, 5.0 + y0, 7.0], [16.0, 16.0 + y0, 9.0]).all("oak_planks"),
    ];
    if open {
        parts.push(Part::new([0.0, 6.0 + y0, 9.0], [2.0, 15.0 + y0, 15.0]).all("oak_planks"));
        parts.push(Part::new([14.0, 6.0 + y0, 9.0], [16.0, 15.0 + y0, 15.0]).all("oak_planks"));
    } else {
        parts.push(Part::new([2.0, 6.0 + y0, 7.0], [14.0, 9.0 + y0, 9.0]).all("oak_planks"));
        parts.push(Part::new([2.0, 12.0 + y0, 7.0], [14.0, 15.0 + y0, 9.0]).all("oak_planks"));
        parts.push(Part::new([6.0, 9.0 + y0, 7.0], [10.0, 12.0 + y0, 9.0]).all("oak_planks"));
    }
    Model { parts: parts.into_iter().map(|p| p.nocull()).collect(), ..Default::default() }.rotated((0, yturns(f)))
}

fn rail(st: &St) -> Model {
    let on = st.b("powered", false);
    let tile: &'static str = match (st.name, on) {
        ("powered_rail", false) => "powered_rail",
        ("powered_rail", true) => "powered_rail_on",
        ("activator_rail", false) => "activator_rail",
        ("activator_rail", true) => "activator_rail_on",
        ("detector_rail", _) => "detector_rail",
        _ => "rail",
    };
    let shape = st.get("shape").unwrap_or("north_south");
    let flat = |rot: u8| Model::one(Part::new([0.0, 1.0, 0.0], [16.0, 1.0, 16.0]).face(UP, tile).face(DOWN, tile).rot_uv(UP, rot).rot_uv(DOWN, rot).nocull().layer(Layer::Cutout));
    match shape {
        "east_west" => flat(1),
        "ascending_north" | "ascending_south" | "ascending_east" | "ascending_west" => {
            // a ramp: the flat rail tilted 45 degrees up toward the named side
            let p = Part::new([0.0, 9.0, 0.0], [16.0, 9.0, 16.0]).face(UP, tile).face(DOWN, tile).nocull().layer(Layer::Cutout);
            let (axis, angle) = match shape {
                "ascending_north" => (0, 45.0),
                "ascending_south" => (0, -45.0),
                "ascending_east" => (2, 45.0),
                _ => (2, -45.0),
            };
            let mut p = p.rotated(axis, angle, [8.0, 9.0, 8.0]);
            if axis == 2 {
                p = p.rot_uv(UP, 1).rot_uv(DOWN, 1);
            }
            Model::one(p)
        }
        "south_east" | "south_west" | "north_west" | "north_east" if st.name == "rail" => {
            // the game's rail_curved model (rail_corner joins the south and east edges), turned
            let rot = match shape {
                "south_east" => 0,
                "south_west" => 3,
                "north_west" => 2,
                _ => 1,
            };
            Model::one(Part::new([0.0, 1.0, 0.0], [16.0, 1.0, 16.0]).face(UP, "rail_corner").face(DOWN, "rail_corner").rot_uv(UP, rot).rot_uv(DOWN, rot).nocull().layer(Layer::Cutout))
        }
        "south_east" | "south_west" | "north_west" | "north_east" => flat(0),
        _ => flat(0),
    }
}

fn hopper(st: &St) -> Model {
    let f = st.dir("facing", Dir::Down);
    let o = "hopper_outside";
    let mut parts = vec![
        Part::new([0.0, 10.0, 0.0], [16.0, 11.0, 16.0]).all(o).face(UP, "hopper_inside"),
        Part::new([0.0, 11.0, 0.0], [2.0, 16.0, 16.0]).all(o),
        Part::new([14.0, 11.0, 0.0], [16.0, 16.0, 16.0]).all(o),
        Part::new([2.0, 11.0, 0.0], [14.0, 16.0, 2.0]).all(o),
        Part::new([2.0, 11.0, 14.0], [14.0, 16.0, 16.0]).all(o),
        Part::new([4.0, 4.0, 4.0], [12.0, 10.0, 12.0]).all(o),
    ];
    let spout = match f {
        Dir::Down => Part::new([6.0, 0.0, 6.0], [10.0, 4.0, 10.0]),
        Dir::North => Part::new([6.0, 4.0, 0.0], [10.0, 8.0, 4.0]),
        Dir::South => Part::new([6.0, 4.0, 12.0], [10.0, 8.0, 16.0]),
        Dir::West => Part::new([0.0, 4.0, 6.0], [4.0, 8.0, 10.0]),
        _ => Part::new([12.0, 4.0, 6.0], [16.0, 8.0, 10.0]),
    };
    parts.push(spout.all(o));
    Model { parts, ..Default::default() }
}

fn piston(st: &St, sticky: bool) -> Model {
    let f = st.dir("facing", Dir::North);
    let front = if sticky { "piston_top_sticky" } else { "piston_top" };
    if st.b("extended", false) {
        // built facing up: the base is 12 px tall, its top shows the inner face
        Model::one(Part::new([0.0, 0.0, 0.0], [16.0, 12.0, 16.0]).all("piston_side").face(UP, "piston_inner").face(DOWN, "piston_bottom")).rotated(rot_up(f))
    } else {
        Model::cube(Part::new(FULL.0, FULL.1).all("piston_side").face(UP, front).face(DOWN, "piston_bottom")).rotated(rot_up(f))
    }
}

fn piston_head(st: &St) -> Model {
    let f = st.dir("facing", Dir::North);
    let front = if st.get("type") == Some("sticky") { "piston_top_sticky" } else { "piston_top" };
    let short = st.b("short", false);
    // built facing up: the plate on top, the arm down into the base below
    let plate = Part::new([0.0, 12.0, 0.0], [16.0, 16.0, 16.0]).all("piston_side").face(UP, front).face(DOWN, "piston_top").uv(NORTH, [0.0, 0.0, 16.0, 4.0]).uv(SOUTH, [0.0, 0.0, 16.0, 4.0]).uv(WEST, [0.0, 0.0, 16.0, 4.0]).uv(EAST, [0.0, 0.0, 16.0, 4.0]);
    let arm = Part::new([6.0, if short { 0.0 } else { -4.0 }, 6.0], [10.0, 12.0, 10.0]).face(NORTH, "piston_arm").face(SOUTH, "piston_arm").face(WEST, "piston_arm").face(EAST, "piston_arm").nocull();
    Model { parts: vec![plate, arm], ..Default::default() }.rotated(rot_up(f))
}

fn sculk(st: &St, calibrated: bool) -> Model {
    let active = st.get("sculk_sensor_phase") == Some("active");
    let base = Part::new([0.0, 0.0, 0.0], [16.0, 8.0, 16.0]).all("sculk_side").face(UP, "sculk_top").face(DOWN, "sculk_top").uv(NORTH, [0.0, 8.0, 16.0, 16.0]).uv(SOUTH, [0.0, 8.0, 16.0, 16.0]).uv(WEST, [0.0, 8.0, 16.0, 16.0]).uv(EAST, [0.0, 8.0, 16.0, 16.0]);
    let tendril = if active { "sculk_tendril_active" } else { "sculk_tendril" };
    let cross = |x: f32, z: f32, angle: f32| {
        Part::new([x - 4.0, 8.0, z], [x + 4.0, 16.0, z]).face(NORTH, tendril).face(SOUTH, tendril).uv(NORTH, [4.0, 0.0, 12.0, 8.0]).uv(SOUTH, [4.0, 0.0, 12.0, 8.0]).nocull().layer(Layer::Cutout).emissive(active).flat().rotated(1, angle, [x, 8.0, z])
    };
    let mut parts = vec![base, cross(4.0, 4.0, 45.0), cross(12.0, 4.0, -45.0), cross(4.0, 12.0, -45.0), cross(12.0, 12.0, 45.0)];
    if calibrated {
        parts.truncate(1);
        parts.push(cross(8.0, 8.0, 45.0));
        parts.push(cross(8.0, 8.0, -45.0));
        let am = |angle: f32| Part::new([4.0, 8.0, 8.0], [12.0, 16.0, 8.0]).face(NORTH, "amethyst_cluster").face(SOUTH, "amethyst_cluster").uv(NORTH, [4.0, 8.0, 12.0, 16.0]).uv(SOUTH, [4.0, 8.0, 12.0, 16.0]).nocull().layer(Layer::Cutout).flat().rotated(1, angle, [8.0, 8.0, 8.0]);
        parts[1] = am(45.0);
        parts[2] = am(-45.0);
        let f = st.dir("facing", Dir::North);
        return Model { parts, ..Default::default() }.rotated((0, yturns(f)));
    }
    Model { parts, ..Default::default() }
}

/// An entity's model (static): minecarts and boats, in blocks around its feet position.
pub fn entity_model(kind: &str) -> Option<Model> {
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind);
    if k.contains("minecart") {
        let body = "iron_block";
        let parts = vec![
            Part::new([-4.0, 1.0, -6.5], [4.0, 2.0, 6.5]).all(body).tint([150, 150, 150]),
            Part::new([-5.0, 1.0, -7.0], [5.0, 9.0, -6.0]).all(body).tint([170, 170, 170]),
            Part::new([-5.0, 1.0, 6.0], [5.0, 9.0, 7.0]).all(body).tint([170, 170, 170]),
            Part::new([-5.0, 1.0, -6.0], [-4.0, 9.0, 6.0]).all(body).tint([170, 170, 170]),
            Part::new([4.0, 1.0, -6.0], [5.0, 9.0, 6.0]).all(body).tint([170, 170, 170]),
        ];
        let mut parts: Vec<Part> = parts.into_iter().map(|p| p.nocull()).collect();
        if k.contains("hopper") {
            parts.push(Part::new([-4.0, 2.0, -6.0], [4.0, 10.0, 6.0]).all("hopper_outside").nocull());
        } else if k.contains("chest") {
            parts.push(Part::new([-4.0, 2.0, -6.0], [4.0, 10.0, 6.0]).all("oak_planks").nocull());
        } else if k.contains("tnt") {
            parts.push(Part::new([-4.0, 2.0, -6.0], [4.0, 10.0, 6.0]).all("redstone_block").nocull());
        } else if k.contains("furnace") {
            parts.push(Part::new([-4.0, 2.0, -6.0], [4.0, 10.0, 6.0]).all("furnace_side").nocull());
        }
        return Some(Model { parts, ..Default::default() });
    }
    if k.contains("boat") || k.contains("raft") {
        // the boat's wood when the tiles have it
        let wood = match k.split('_').next().unwrap_or("oak") {
            "spruce" => "spruce_planks",
            _ => "oak_planks",
        };
        let mut parts = vec![
            // the hull: its floor a little above the entity's position, its rim well above the
            // water line like the game's boat model
            Part::new([-7.0, 2.0, -11.0], [7.0, 3.0, 11.0]).all(wood),
            Part::new([-8.0, 2.0, -11.0], [-7.0, 9.0, 11.0]).all(wood),
            Part::new([7.0, 2.0, -11.0], [8.0, 9.0, 11.0]).all(wood),
            Part::new([-7.0, 2.0, -12.0], [7.0, 9.0, -11.0]).all(wood),
            Part::new([-7.0, 2.0, 11.0], [7.0, 9.0, 12.0]).all(wood),
        ];
        if k.contains("chest") {
            // the chest at the stern, its lid line and latch
            parts.push(Part::new([-6.0, 3.0, 1.0], [6.0, 14.0, 10.0]).all("oak_planks").tint([200, 150, 90]));
            parts.push(Part::new([-6.2, 10.0, 0.8], [6.2, 10.6, 10.2]).all("oak_log").tint([90, 60, 30]));
            parts.push(Part::new([-1.0, 8.0, 0.5], [1.0, 11.0, 1.0]).all("iron_block"));
        }
        return Some(Model { parts: parts.into_iter().map(|p| p.nocull()).collect(), ..Default::default() });
    }
    if k.contains("armor_stand") {
        return Some(Model { parts: vec![Part::new([-1.0, 0.0, -1.0], [1.0, 24.0, 1.0]).all("oak_planks").nocull(), Part::new([-6.0, 0.0, -6.0], [6.0, 1.0, 6.0]).all("smooth_stone").nocull()], ..Default::default() });
    }
    Some(Model::one(Part::new([-4.0, 0.0, -4.0], [4.0, 8.0, 4.0]).all("generic").tint(name_color(k)).nocull()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_archive_block_has_a_model() {
        let atlas = Atlas::build();
        for s in [
            "minecraft:observer[facing=east,powered=false]",
            "minecraft:sticky_piston[extended=true,facing=up]",
            "minecraft:piston_head[facing=up,short=false,type=sticky]",
            "minecraft:redstone_wire[east=side,north=up,power=15,south=none,west=side]",
            "minecraft:repeater[delay=4,facing=north,locked=true,powered=false]",
            "minecraft:comparator[facing=west,mode=subtract,powered=true]",
            "minecraft:lever[face=wall,facing=north,powered=true]",
            "minecraft:hopper[enabled=true,facing=east]",
            "minecraft:calibrated_sculk_sensor[facing=east,power=0,sculk_sensor_phase=active]",
            "minecraft:oak_door[facing=east,half=upper,hinge=left,open=true,powered=false]",
            "minecraft:rail[shape=ascending_east]",
            "minecraft:deepslate",
        ] {
            assert!(model_for(s, &atlas).is_some_and(|m| !m.parts.is_empty()), "{s}");
        }
        assert!(model_for("minecraft:air", &atlas).is_none());
    }
}
