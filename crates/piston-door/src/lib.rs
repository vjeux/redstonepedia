//! The 3x3 piston door.
//!
//! A design is a [`Design`] with two ports:
//! * `door`: the lowest corner (min x, min y) of the 3x3 opening. The door is the vertical
//!   plane `z = door.z` covering `x in door.x..door.x+3`, `y in door.y..door.y+3`.
//! * `lever`: a lever, off or on at rest (either polarity counts): the first flip from rest
//!   opens the door, the second closes it.
//!
//! What "works" means (the full rules: docs/rules.md):
//! * at rest, the 9 door cells hold 9 identical plain full blocks and the
//!   walkway (the opening extended 3 blocks in front and behind) is walkable (no collision);
//! * after the lever is flipped and everything settles, the 9 door cells and the walkway
//!   are all walkable;
//! * after the lever is flipped back and everything settles, every block of the build is back
//!   exactly where it started (so the door is the same blocks, and it can cycle forever);
//! * no item entity appears (nothing was broken), repeated for several cycles, at several
//!   positions in the world (Minecraft has location-dependent update orders).

pub mod cli;
pub mod mechsynth;
pub mod families;
pub mod mechwire;
pub mod archive;
pub mod buildable;
pub mod catalog;
pub mod clips;
pub mod designs;
pub mod designs_history;
pub mod designs_fresh_b;
pub mod designs_top;
pub mod designs_center_mech;
pub mod observer_seq;
pub mod obschain;
pub mod leaderboard;
pub mod layouts;
pub mod designs_gravity;
pub mod lm;
pub mod properties_dyn;
pub mod drive;
pub mod harvest;
pub mod ideal;
pub mod ideal_rs;
pub mod feeds;
pub mod interleave;
pub mod selffeed;
pub mod shuttle;
pub mod route;
pub mod route2;
pub mod minimize;
pub mod patch;
pub mod place;
pub mod properties;
pub mod relever;
pub mod rowsfire;
pub mod satsweep;
pub mod satwire;
pub mod boxproof;
pub mod macrosat;
pub mod cosearch;
pub mod cosearch_search;
pub mod search;
pub mod shapes;
pub mod showcase;
pub mod redsim_screen;
pub mod material;
pub mod synth;
pub mod tickvideo;
pub mod tile;
pub mod topology;
pub mod connect;
pub mod video;
pub mod vanilla_check;
pub mod vanilla_build;
pub mod vanilla_gadget;
pub mod pistfuzz;
pub mod fullmech;
pub mod mechspace;
pub mod world;
pub mod sheet;
pub mod cycle;
pub mod audit;
pub mod provenance;
pub mod mechmut;
pub mod selftest;
pub mod testplan;
pub mod mutate;
pub mod fill;

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};

/// Where the door and its lever are.
#[derive(Clone, Copy, Debug)]
pub struct DoorSpec {
    pub door: Pos,
    pub lever: Pos,
    /// The opening spans z (plane x = door.x) instead of x (plane z = door.z): a design turned a
    /// quarter turn ([`rotate_design`]). Design files always have the door in a z plane.
    pub along_z: bool,
    /// The opening's side: 3 (the 3x3 door, the default) or 2 (`door-size: 2`, the 2x2 door,
    /// judged by the same rules on its 4 cells; designs/piston-door-2x2/).
    pub size: i32,
}

impl DoorSpec {
    pub fn from_design(d: &Design) -> Result<DoorSpec> {
        let door = *d.ports.get("door").context("design has no 'port door: x y z'")?;
        let lever = *d.ports.get("lever").context("design has no 'port lever: x y z'")?;
        // `door-axis: z`: the opening spans z (an east/west-facing door, e.g. a variant built for
        // that orientation); default x (a north/south-facing door, plane z = door.z).
        let along_z = d.meta.get("door-axis").is_some_and(|v| v.trim() == "z");
        let size = door_size(d)?;
        Ok(DoorSpec { door, lever, along_z, size })
    }

    /// The same spec moved by `d`.
    pub fn translated(&self, d: Pos) -> DoorSpec {
        let mv = |p: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        DoorSpec { door: mv(self.door), lever: mv(self.lever), along_z: self.along_z, size: self.size }
    }

    /// One step along the opening's width (x, or z for a turned door).
    fn width_step(&self, i: i32) -> Pos {
        if self.along_z { [0, 0, i] } else { [i, 0, 0] }
    }

    /// One step through the door (z, or x for a turned door).
    fn depth_step(&self, i: i32) -> Pos {
        if self.along_z { [i, 0, 0] } else { [0, 0, i] }
    }

    /// The cells of the opening (9, or 4 for a 2x2 door), row by row from the top (y desc), west
    /// to east (north to south for a turned door).
    pub fn door_cells(&self) -> Vec<Pos> {
        let [x, y, z] = self.door;
        let mut out = Vec::new();
        for dy in (0..self.size).rev() {
            for dw in 0..self.size {
                let s = self.width_step(dw);
                out.push([x + s[0], y + dy, z + s[2]]);
            }
        }
        out
    }

    /// The walkway: the opening extended 3 blocks in front of and behind the door, so you
    /// can actually walk up to and through it.
    pub fn corridor_cells(&self) -> Vec<Pos> {
        self.in_front_and_behind(3)
    }

    /// The opening's columns in front of and behind the door, `depth` blocks each way.
    pub fn in_front_and_behind(&self, depth: i32) -> Vec<Pos> {
        let [x, y, z] = self.door;
        let mut out = Vec::new();
        for dd in (-depth..=depth).filter(|d| *d != 0) {
            let d = self.depth_step(dd);
            for dy in 0..self.size {
                for dw in 0..self.size {
                    let s = self.width_step(dw);
                    out.push([x + s[0] + d[0], y + dy, z + s[2] + d[2]]);
                }
            }
        }
        out
    }
}

/// The opening's side from a design's `door-size` header: 3 when absent, 2 for the 2x2 door.
pub fn door_size(d: &Design) -> Result<i32> {
    match d.meta.get("door-size").map(|v| v.trim()) {
        None | Some("3") => Ok(3),
        Some("2") => Ok(2),
        Some(v) => bail!("door-size: {v}: only 2 (the 2x2 door) or 3 (the 3x3 door, the default)"),
    }
}

/// `p` turned `turns` quarter turns clockwise seen from above, around the vertical axis through
/// the origin: north (-z) becomes east (+x), as the game's `Rotation.CLOCKWISE_90` does.
pub fn rotate_pos(p: Pos, turns: u8) -> Pos {
    let mut q = p;
    for _ in 0..turns % 4 {
        q = [-q[2], q[1], q[0]];
    }
    q
}

/// `d` turned `turns` quarter turns clockwise (seen from above) around the origin, every block
/// state turned by the game itself ([`Worker::rotate_states`]), with the door spec that goes with
/// it.
pub fn rotate_design(w: &mut Worker, d: &Design, turns: u8) -> Result<(Design, DoorSpec)> {
    transform_design(w, d, turns, false)
}

/// [`rotate_design`] of the mirror image (x -> -x, the game's `Mirror.FRONT_BACK`) when `mirror`.
pub fn transform_design(w: &mut Worker, d: &Design, turns: u8, mirror: bool) -> Result<(Design, DoorSpec)> {
    let spec = DoorSpec::from_design(d)?;
    let turns = turns % 4;
    let tf = |p: Pos| rotate_pos(if mirror { [-p[0], p[1], p[2]] } else { p }, turns);
    let states: Vec<String> = d.blocks.values().cloned().collect();
    let turned = w.transform_states(&states, turns, mirror)?;
    let blocks = d.blocks.keys().zip(turned).map(|(p, s)| (tf(*p), s)).collect();
    let ports = d.ports.iter().map(|(k, p)| (k.clone(), tf(*p))).collect();
    // The opening's min corner after the transform.
    let cells: Vec<Pos> = spec.door_cells().iter().map(|p| tf(*p)).collect();
    let min = |i: usize| cells.iter().map(|p| p[i]).min().unwrap();
    let door = [min(0), min(1), min(2)];
    let rspec = DoorSpec { door, lever: tf(spec.lever), along_z: spec.along_z != (turns % 2 == 1), size: spec.size };
    // Entities: positions by the same map in continuous coordinates (a block cell [x, x+1) mirrors
    // to [-x, -x+1): X -> 1 - X; a turn takes cell (x, z) to (-z, x): (X, Z) -> (1 - Z, X)), their
    // data (yaw, a hanging entity's facing) by the game ([`Worker::transform_entities`]).
    let entities = if d.entities.is_empty() {
        Vec::new()
    } else {
        let nbts = w.transform_entities(&d.entities, turns, mirror)?;
        d.entities
            .iter()
            .zip(nbts)
            .map(|(e, nbt)| {
                let mut p = e.pos;
                if mirror {
                    p[0] = 1.0 - p[0];
                }
                for _ in 0..turns {
                    p = [1.0 - p[2], p[1], p[0]];
                }
                mctas::design::EntitySpec { kind: e.kind.clone(), pos: p, nbt }
            })
            .collect()
    };
    // Block-entity data (container contents) does not turn: positions only.
    let block_entities = d.block_entities.iter().map(|(p, s)| (tf(*p), s.clone())).collect();
    let mut rd = Design { name: d.name.clone(), meta: d.meta.clone(), ports, blocks, entities, block_entities, ..Default::default() };
    rd.ports.insert("door".into(), door);
    if rspec.along_z {
        rd.meta.insert("door-axis".into(), "z".into());
    } else {
        rd.meta.remove("door-axis");
    }
    Ok((rd, rspec))
}

/// The judge's checks ([`verify_in_spec`] with `opts`) on all 8 orientations of the door: 4 turns
/// (0, 90, 180, 270 degrees clockwise seen from above), each as built and mirrored. Returns
/// (turns, mirrored, `Ok` or the judge's reason) for each.
pub fn orientation_report(w: &mut Worker, d: &Design, opts: &VerifyOptions) -> Result<Vec<(u8, bool, std::result::Result<(), String>)>> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut out = Vec::new();
    for mirror in [false, true] {
        for turns in 0..4u8 {
            let (rd, rspec) = transform_design(w, d, turns, mirror)?;
            let r = verify_in_spec(w, sb, &rd, rspec, opts).map(|_| ()).map_err(|e| format!("{e:#}"));
            out.push((turns, mirror, r));
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(out)
}

/// Does the door also work turned 90, 180 and 270 degrees? Runs the judge's checks ([`verify_in`]
/// with `opts`) on each turned copy. `Ok(None)` = works in all four orientations; `Ok(Some((turns,
/// why)))` = the first orientation that fails (a quarter-turn count) and the judge's reason.
/// Redstone updates neighbors in a fixed direction order (west, east, down, up, north, south),
/// so a door can work facing one way and not another. Not part of the judge: the boards rank
/// rotation-safe designs and list the others apart.
pub fn rotation_check(w: &mut Worker, d: &Design, opts: &VerifyOptions) -> Result<Option<(u8, String)>> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut result = Ok(None);
    for turns in 1..4u8 {
        let (rd, rspec) = match rotate_design(w, d, turns) {
            Ok(x) => x,
            Err(e) => {
                result = Err(e);
                break;
            }
        };
        if let Err(e) = verify_in_spec(w, sb, &rd, rspec, opts) {
            result = Ok(Some((turns, format!("{e:#}"))));
            break;
        }
    }
    let _ = w.drop_sandbox(sb);
    result
}

/// Knobs of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Open/close cycles per position.
    pub cycles: u32,
    /// Cap on ticks to wait for the contraption to settle after each lever flip.
    pub max_ticks: u32,
    /// Positions (offsets added to the design) to test at.
    pub offsets: Vec<Pos>,
    /// Keep the per-tick frames of the first cycle (for timelines and videos).
    pub keep_frames: bool,
    /// Nothing at all may stand in front of or behind the door: at rest the 3x3 columns in front
    /// of and behind the door, at any distance, are air (no machine parts, dust, torches, rails,
    /// levers...). vjeux's rule since 2026-10-18 16:40Z; designs judged before keep their cached
    /// verdicts for now.
    pub empty_walkway: bool,
    /// Judge the design as a tile module instead ([`tile`]): copies of it side by side or
    /// stacked, one lever, every check on every opening. `None` = the single door (the judge).
    pub tile: Option<tile::Tile>,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions {
            cycles: 3,
            max_ticks: 400,
            offsets: vec![[0, 0, 0], [-7, 3, 5], [5, -11, -6], [-3, 40, 2]],
            keep_frames: false,
            empty_walkway: true,
            tile: None,
        }
    }
}

/// Timing of one open/close cycle, in game ticks after the lever flip.
#[derive(Clone, Debug)]
pub struct CycleReport {
    /// First tick after which the opening stays walkable.
    pub open_ticks: u32,
    /// Ticks until everything was idle after opening.
    pub open_settle: u32,
    /// First tick after which the door is back (all 9 cells full and identical to the start).
    pub close_ticks: u32,
    pub close_settle: u32,
}

/// Result of [`verify`].
#[derive(Clone, Debug)]
pub struct Report {
    pub blocks: usize,
    pub size: [i32; 3],
    pub cycles: Vec<CycleReport>,
    /// Frames of the door area for the first open and first close (when kept).
    pub open_frames: Vec<Region>,
    pub close_frames: Vec<Region>,
    /// Warnings that do not fail the door (e.g. the file was not in its rest state).
    pub warnings: Vec<String>,
}

/// Verdict: Ok(report) when the door works everywhere, Err(reason) otherwise.
pub fn verify(w: &mut Worker, design: &Design, opts: &VerifyOptions) -> Result<Report> {
    if let Some(t) = &opts.tile {
        return tile::verify_tiled(w, design, t, opts).map(|(r, _, _)| r);
    }
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = verify_in(w, sb, design, opts);
    let _ = w.drop_sandbox(sb);
    r
}

/// Count of placed blocks: every non-air block except piston heads (part of their piston).
/// Entities are not blocks: they are counted on their own ([`entity_count`]; vjeux 2026-10-18:
/// fewer blocks with more entities is fine), and the Entities board ranks by blocks, then entities.
pub fn block_count(d: &Design) -> usize {
    d.blocks.values().filter(|s| !s.starts_with("minecraft:piston_head")).count()
}

/// Count of the design's entities (minecarts, armor stands...), scored apart from blocks.
pub fn entity_count(d: &Design) -> usize {
    d.entities.len()
}

/// `n` deterministic pseudo-random extra test positions, on top of the default four.
pub fn extra_offsets(n: u32) -> Vec<Pos> {
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    let mut next = |m: i64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % (2 * m as u64 + 1)) as i32 - m as i32
    };
    (0..n).map(|_| [next(13), next(40), next(13)]).collect()
}

/// Blocks a survival player cannot place (creative-only or technical); designs may not use them.
const FORBIDDEN: &[&str] = &[
    "command_block", "chain_command_block", "repeating_command_block", "structure_block", "structure_void",
    "jigsaw", "barrier", "light", "bedrock", "end_portal_frame", "end_portal", "end_gateway", "nether_portal",
    "spawner", "trial_spawner", "vault", "reinforced_deepslate", "budding_amethyst", "petrified_oak_slab",
    "moving_piston", "test_block", "test_instance_block", "frosted_ice", "bubble_column", "fire", "soul_fire",
];

/// Gravity-affected full blocks. A door made of these is a different category (it can fall).
pub fn is_gravity_block(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    let n = name.trim_start_matches("minecraft:");
    n.ends_with("concrete_powder") || matches!(n, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel")
}

/// True when the design uses no slime and no honey blocks anywhere (its own leaderboard).
pub fn slime_free(d: &Design) -> bool {
    !d.blocks.values().any(|s| s.contains("slime_block") || s.contains("honey_block"))
}

/// True when every block of the design lies in the door's plane (z = door.z): the whole machine
/// is one block thick ("flat"; its own leaderboard together with slime-free).
pub fn single_layer(d: &Design) -> bool {
    let Some(door) = d.ports.get("door") else { return false };
    // The door's plane: z = door.z, or x = door.x for an east/west-facing door (`door-axis: z`).
    let axis = if d.meta.get("door-axis").is_some_and(|v| v.trim() == "z") { 0 } else { 2 };
    // The lever may hang on the front or back face of a block in the plane and the door still
    // counts as one wide (vjeux, 2026-10-19).
    let lever = d.ports.get("lever").copied();
    d.blocks.keys().all(|p| p[axis] == door[axis] || Some(*p) == lever)
}

/// How far a design reaches beyond its 3x3 opening on every side (piston heads included), along
/// the door's own axes: depth through the door (z, or x for a `door-axis: z` design), height, and
/// width along the opening. Builders care: a door that fits a thin wall, needs no digging or
/// stays narrow is easier to place than a smaller block count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Shape {
    /// Layers in front of the door plane (z < door.z) and behind it (z > door.z).
    pub front: i32,
    pub behind: i32,
    /// Rows below the door's bottom row (y < door.y) and above its top row (y > door.y + 2).
    pub below: i32,
    pub above: i32,
    /// Columns left of the opening (x < door.x) and right of it (x > door.x + 2).
    pub left: i32,
    pub right: i32,
    /// Blocks below the bottom row and above the top row (counted like the score).
    pub blocks_below: usize,
    pub blocks_above: usize,
}

impl Shape {
    /// Layers along the door's depth axis (the door plane included): 1 = flat.
    pub fn thickness(&self) -> i32 {
        1 + self.front + self.behind
    }
    /// Columns across (the opening's 3 included).
    pub fn width(&self) -> i32 {
        3 + self.left + self.right
    }
    /// Rows (the opening's 3 included).
    pub fn height(&self) -> i32 {
        3 + self.below + self.above
    }
    /// Bounding-box volume, the door included.
    pub fn volume(&self) -> i32 {
        self.thickness() * self.width() * self.height()
    }
    /// The wider of the two sides of the opening (columns).
    pub fn side(&self) -> i32 {
        self.left.max(self.right)
    }
}

/// The [`Shape`] of a design relative to its door (`None` without a door port or blocks).
pub fn shape(d: &Design) -> Option<Shape> {
    let door = *d.ports.get("door")?;
    shape_of_cells(d, door, d.blocks.keys())
}

/// The [`Shape`] of a set of occupied cells around `door` (blocks below / above counted from the
/// design's own blocks at rest). A `door-axis: z` design (an east/west-facing door) has its width
/// along z and its depth along x.
fn shape_of_cells<'a>(d: &Design, door: Pos, cells: impl IntoIterator<Item = &'a Pos>) -> Option<Shape> {
    let along_z = d.meta.get("door-axis").is_some_and(|v| v.trim() == "z");
    let (wi, di) = if along_z { (2, 0) } else { (0, 2) };
    let mut it = cells.into_iter();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    // the opening's last row / column (2 for the 3x3 door, 1 for the 2x2)
    let top = door_size(d).unwrap_or(3) - 1;
    let counted = |p: &&Pos| !d.blocks[*p].starts_with("minecraft:piston_head");
    Some(Shape {
        front: (door[di] - lo[di]).max(0),
        behind: (hi[di] - door[di]).max(0),
        below: (door[1] - lo[1]).max(0),
        above: (hi[1] - (door[1] + top)).max(0),
        left: (door[wi] - lo[wi]).max(0),
        right: (hi[wi] - (door[wi] + top)).max(0),
        blocks_below: d.blocks.keys().filter(|p| p[1] < door[1]).filter(counted).count(),
        blocks_above: d.blocks.keys().filter(|p| p[1] > door[1] + top).filter(counted).count(),
    })
}

/// The space a door needs: the [`Shape`] of every cell it occupies at rest or at any tick of one
/// open/close cycle (moving blocks included), at the design's own position. A builder has to
/// keep all of it free: a structure that slides out of a wall, or a block pulled below the door,
/// needs room there even if the door is compact at rest.
pub fn swept_shape(w: &mut Worker, d: &Design) -> Result<Shape> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = swept_shape_in(w, sb, d);
    let _ = w.drop_sandbox(sb);
    r
}

/// [`swept_shape`] in an existing sandbox (cleared first).
pub fn swept_shape_in(w: &mut Worker, sb: u32, d: &Design) -> Result<Shape> {
    let spec = DoorSpec::from_design(d)?;
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    w.step_until_idle(sb, 400)?;
    let (wlo, whi) = watch_box(d, &spec);
    let mut cells: std::collections::BTreeSet<Pos> = d.blocks.keys().copied().collect();
    let mut add = |r: &Region| {
        for (p, s) in r.iter() {
            if !mctas::blockstate::is_air(s) {
                cells.insert(p);
            }
        }
    };
    add(&w.get_region(sb, wlo, whi)?);
    for _ in 0..2 {
        w.use_block(sb, spec.lever)?;
        let run = w.step_record(sb, 400, true, wlo, whi)?;
        for f in &run.frames {
            add(f);
        }
    }
    shape_of_cells(d, spec.door, cells.iter()).context("empty design")
}

/// Shape limits (a board's rule, or a search constraint): every set bound must hold. Written
/// as a comma list of `key<=n` (`front behind thick below above left right side volume`; `key=0`
/// is `key<=0`) or a named preset ([`ShapeLimits::PRESETS`]: `wall`, `wall2`, `ground`...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ShapeLimits {
    pub front: Option<i32>,
    pub behind: Option<i32>,
    pub thick: Option<i32>,
    pub below: Option<i32>,
    pub above: Option<i32>,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub side: Option<i32>,
    pub volume: Option<i32>,
}

impl ShapeLimits {
    /// Named shape rules: (name, limits as `key<=n` terms, what it means for a builder).
    pub const PRESETS: &'static [(&'static str, &'static str, &'static str)] = &[
        ("wall", "front<=1,behind<=1", "the whole machine inside a 3-thick wall centered on the door (z = door.z-1..door.z+1)"),
        ("wall2", "thick<=2", "the whole machine inside a 2-thick wall: the door plane and one layer on one side"),
        ("flat", "thick<=1", "one block thick (slime allowed): every block in the door's plane"),
        ("ground", "below=0", "nothing below the door's bottom row: it stands on the ground, no digging"),
        ("no-above", "above=0", "nothing above the door's top row: no ceiling machinery"),
        ("narrow1", "side<=1", "at most 1 column beside the opening on each side (5 wide)"),
        ("narrow2", "side<=2", "at most 2 columns beside the opening on each side (7 wide)"),
        ("wall-ground", "front<=1,behind<=1,below=0", "inside a 3-thick wall and nothing below the door"),
    ];

    pub fn parse(spec: &str) -> anyhow::Result<ShapeLimits> {
        let spec = spec.trim();
        if let Some((_, terms, _)) = Self::PRESETS.iter().find(|(n, _, _)| *n == spec) {
            return Self::parse(terms);
        }
        let mut l = ShapeLimits::default();
        for term in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let (key, n) = term
                .split_once("<=")
                .or_else(|| term.split_once('='))
                .ok_or_else(|| anyhow::anyhow!("shape term {term:?}: expected key<=n or key=0, or a preset ({})", Self::preset_names()))?;
            let n: i32 = n.trim().parse().with_context(|| format!("shape term {term:?}"))?;
            let slot = match key.trim() {
                "front" => &mut l.front,
                "behind" => &mut l.behind,
                "thick" => &mut l.thick,
                "below" => &mut l.below,
                "above" => &mut l.above,
                "left" => &mut l.left,
                "right" => &mut l.right,
                "side" => &mut l.side,
                "volume" => &mut l.volume,
                k => bail!("unknown shape key {k:?} (front behind thick below above left right side volume, or a preset: {})", Self::preset_names()),
            };
            *slot = Some(n);
        }
        Ok(l)
    }

    fn preset_names() -> String {
        Self::PRESETS.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(" ")
    }

    pub fn allows(&self, s: &Shape) -> bool {
        let ok = |lim: Option<i32>, v: i32| lim.is_none_or(|m| v <= m);
        ok(self.front, s.front)
            && ok(self.behind, s.behind)
            && ok(self.thick, s.thickness())
            && ok(self.below, s.below)
            && ok(self.above, s.above)
            && ok(self.left, s.left)
            && ok(self.right, s.right)
            && ok(self.side, s.side())
            && ok(self.volume, s.volume())
    }

    /// Does the design (at rest, as written) keep within these limits?
    pub fn allows_design(&self, d: &Design) -> bool {
        shape(d).is_some_and(|s| self.allows(&s))
    }
}

/// True when the whole machine fits a 3-thick wall centered on the door: every block within one
/// layer of the door plane (z = door.z-1..door.z+1). The "wall" board.
pub fn in_wall(d: &Design) -> bool {
    shape(d).is_some_and(|s| s.front <= 1 && s.behind <= 1)
}

/// True when the whole machine fits a 2-thick wall (the door plane and one layer on one side).
pub fn in_wall2(d: &Design) -> bool {
    shape(d).is_some_and(|s| s.thickness() <= 2)
}

/// True when nothing lies below the door's bottom row: the door stands on the ground, no digging.
pub fn on_ground(d: &Design) -> bool {
    shape(d).is_some_and(|s| s.below == 0)
}

/// True when nothing lies above the door's top row.
pub fn nothing_above(d: &Design) -> bool {
    shape(d).is_some_and(|s| s.above == 0)
}

/// True when at most `k` columns of the machine stand beside the opening on each side.
pub fn narrow(d: &Design, k: i32) -> bool {
    shape(d).is_some_and(|s| s.side() <= k)
}

/// The block a [`ground_check`] floor is made of: real ground (dirt, grass, stone) is a full,
/// movable, redstone-conducting block that slime sticks to, and so is stone (which, unlike grass,
/// takes no random ticks).
pub const GROUND_BLOCK: &str = "minecraft:stone";

/// `d` standing on the ground: a floor of [`GROUND_BLOCK`] one below the door's bottom row,
/// under the whole watched box (cells the design already fills are kept).
pub fn on_floor(d: &Design) -> Result<Design> {
    let spec = DoorSpec::from_design(d)?;
    let (lo, hi) = watch_box(d, &spec);
    let y = spec.door[1] - 1;
    let mut out = d.clone();
    for x in lo[0]..=hi[0] {
        for z in lo[2]..=hi[2] {
            out.blocks.entry([x, y, z]).or_insert_with(|| GROUND_BLOCK.to_string());
        }
    }
    Ok(out)
}

/// Does a door with nothing below it ([`on_ground`]) still work standing on real ground? The
/// judge's checks ([`verify_in`] with `opts`) on [`on_floor`]: the floor must not be dragged
/// (slime, honey), pushed or powered into the machine, and must be back in place after every
/// cycle. `Ok(None)` = works on the ground; `Ok(Some(why))` = the judge's reason it does not.
/// Not part of the judge: it decides the "on the ground" board.
pub fn ground_check(w: &mut Worker, d: &Design, opts: &VerifyOptions) -> Result<Option<String>> {
    let floor = on_floor(d)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = verify_in(w, sb, &floor, opts);
    let _ = w.drop_sandbox(sb);
    Ok(r.err().map(|e| format!("{e:#}")))
}

/// Blocks standing in the walkway at rest. The rules allow non-colliding blocks there (dust,
/// torches, levers), but a clean doorway has none, so the leaderboards show the count.
pub fn walkway_clutter(d: &Design) -> usize {
    let Ok(spec) = DoorSpec::from_design(d) else { return 0 };
    spec.corridor_cells().iter().filter(|p| d.blocks.get(*p).is_some_and(|s| !mctas::blockstate::is_air(s))).count()
}

/// The door category of a design: "entity" (it declares entities: minecarts, armor stands,
/// boats...; ranked on its own board, every other board bans entities), "solid" (normal blocks),
/// "gravity" (sand-like door blocks), or "sculk" (uses sculk sensors: vibrations from players or
/// mobs nearby can trigger them, so those doors are ranked separately).
pub fn category(d: &Design) -> &'static str {
    let door = d.ports.get("door").and_then(|p| d.blocks.get(p));
    if !d.entities.is_empty() {
        "entity"
    } else if door.is_some_and(|s| is_gravity_block(s)) {
        "gravity"
    } else if d.blocks.values().any(|s| s.contains("sculk_sensor")) {
        "sculk"
    } else {
        "solid"
    }
}

/// On the "Entities allowed" board: a solid design, or a design with entities whose door is
/// otherwise solid (plain door blocks, not gravity blocks, and no sculk sensors: a gravity or
/// sculk door with entities is ranked nowhere).
pub fn entities_allowed_board(d: &Design) -> bool {
    match category(d) {
        "solid" => true,
        "entity" => {
            let door = d.ports.get("door").and_then(|p| d.blocks.get(p));
            !door.is_some_and(|s| is_gravity_block(s)) && !d.blocks.values().any(|s| s.contains("sculk_sensor"))
        }
        _ => false,
    }
}

/// Entity types a design may declare: what a survival player can place and what never acts on
/// its own (no mobs: their AI is random; no items, arrows or experience: they despawn; no
/// command block or spawner minecarts: creative only).
pub fn entity_type_allowed(kind: &str) -> bool {
    let n = kind.trim_start_matches("minecraft:");
    matches!(
        n,
        "minecart" | "chest_minecart" | "hopper_minecart" | "furnace_minecart" | "tnt_minecart" | "armor_stand" | "item_frame" | "glow_item_frame"
            | "painting" | "bamboo_raft" | "bamboo_chest_raft"
    ) || n.ends_with("_boat")
}

/// Entity data keys a design may set: what a survival player chooses when placing or filling an
/// entity (its facing, a container's items, an armor stand's equipment, an item frame's item and
/// rotation, a painting's variant). Not NoGravity, Invulnerable, Tags, Passengers, poses... (only
/// commands set those).
const ENTITY_KEYS: &[&str] = &["Rotation", "Items", "equipment", "Item", "ItemRotation", "Facing", "facing", "variant"];

/// The top-level keys of an SNBT compound (`{a:1,b:[2,3],"c":{d:4}}` -> a, b, c).
pub fn snbt_top_keys(s: &str) -> std::result::Result<Vec<String>, String> {
    let inner = s.trim().strip_prefix('{').and_then(|r| r.strip_suffix('}')).ok_or_else(|| format!("not an SNBT compound: {s}"))?;
    let mut keys = Vec::new();
    let (mut depth, mut quote, mut escaped, mut in_key) = (0i32, None::<char>, false, true);
    let mut cur = String::new();
    for c in inner.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
                continue;
            } else if c == q {
                quote = None;
                continue;
            }
            if in_key && depth == 0 {
                cur.push(c);
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ':' if depth == 0 && in_key => {
                keys.push(cur.trim().to_string());
                cur.clear();
                in_key = false;
            }
            ',' if depth == 0 => {
                in_key = true;
                cur.clear();
            }
            _ if in_key && depth == 0 => cur.push(c),
            _ => {}
        }
    }
    if depth != 0 || quote.is_some() {
        return Err(format!("unbalanced SNBT: {s}"));
    }
    Ok(keys)
}

/// Static legality: no creative-only blocks, no infested blocks, and every piston head belongs to
/// an extended piston facing the same way (no headless/orphan heads).
pub fn legality_problems(d: &Design) -> Vec<String> {
    let mut out = Vec::new();
    for (p, s) in &d.blocks {
        let name = mctas::blockstate::block_name(s);
        let n = name.trim_start_matches("minecraft:");
        if FORBIDDEN.contains(&n) || n.starts_with("infested_") {
            out.push(format!("{p:?} is {s}: not placeable in survival"));
        }
        let facing = |st: &str| BlockStateLite::get(st, "facing");
        if n == "piston_head" {
            let f = facing(s).unwrap_or_default();
            let back = match f.as_str() {
                "up" => [p[0], p[1] - 1, p[2]],
                "down" => [p[0], p[1] + 1, p[2]],
                "north" => [p[0], p[1], p[2] + 1],
                "south" => [p[0], p[1], p[2] - 1],
                "east" => [p[0] - 1, p[1], p[2]],
                _ => [p[0] + 1, p[1], p[2]],
            };
            let ok = d.blocks.get(&back).is_some_and(|b| {
                let bn = mctas::blockstate::block_name(b);
                (bn == "minecraft:piston" || bn == "minecraft:sticky_piston")
                    && BlockStateLite::get(b, "extended").as_deref() == Some("true")
                    && facing(b).as_deref() == Some(f.as_str())
            });
            if !ok {
                out.push(format!("{p:?} is a piston head without its extended piston behind it"));
            }
        }
    }
    for e in &d.entities {
        if !entity_type_allowed(&e.kind) {
            out.push(format!("entity {} at {:?}: not allowed (minecarts, boats, armor stands, item frames and paintings only; no mobs)", e.kind, e.pos));
        }
        if let Some(nbt) = &e.nbt {
            match snbt_top_keys(nbt) {
                Ok(keys) => {
                    for k in keys.iter().filter(|k| !ENTITY_KEYS.contains(&k.as_str())) {
                        out.push(format!("entity {} at {:?}: data key {k} is not something a survival player sets (allowed: {})", e.kind, e.pos, ENTITY_KEYS.join(", ")));
                    }
                }
                Err(why) => out.push(format!("entity {} at {:?}: {why}", e.kind, e.pos)),
            }
        }
    }
    out
}

/// Minimal property reader for "minecraft:x[a=b,c=d]" strings.
struct BlockStateLite;
impl BlockStateLite {
    fn get(state: &str, key: &str) -> Option<String> {
        let props = state.split_once('[')?.1.trim_end_matches(']');
        props.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string()))
    }
}

/// [`verify`] in an existing sandbox (it is cleared first; reuse saves creating levels).
pub fn verify_in(w: &mut Worker, sb: u32, design: &Design, opts: &VerifyOptions) -> Result<Report> {
    if let Some(t) = &opts.tile {
        return tile::verify_tiled_in(w, sb, design, t, opts).map(|(r, _)| r);
    }
    let spec = DoorSpec::from_design(design)?;
    verify_in_spec(w, sb, design, spec, opts)
}

/// [`verify_in`] with an explicit door spec (a turned design's door spans z: [`rotate_design`]).
pub fn verify_in_spec(w: &mut Worker, sb: u32, design: &Design, spec: DoorSpec, opts: &VerifyOptions) -> Result<Report> {
    let illegal = legality_problems(design);
    if !illegal.is_empty() {
        bail!("illegal design: {}", illegal.join("; "));
    }
    let (lo, hi) = design.bounds().context("empty design")?;
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut report = Report {
        blocks: block_count(design),
        size,
        cycles: Vec::new(),
        open_frames: Vec::new(),
        close_frames: Vec::new(),
        warnings: Vec::new(),
    };
    let result = (|| -> Result<()> {
        for (i, off) in opts.offsets.iter().enumerate() {
            let d = design.translated(*off);
            let s = spec.translated(*off);
            verify_at(w, sb, &d, &s, opts, &mut report, i == 0).with_context(|| format!("at offset {off:?}"))?;
        }
        Ok(())
    })();
    result?;
    Ok(report)
}

/// How far (blocks, per axis) an entity may come back from its rest position after a cycle: float
/// noise of piston pushes only (see the close check in `verify_at`).
pub const ENTITY_POS_TOLERANCE: f64 = 1e-6;

/// The box the verifier watches: the design (its entities' cells included) plus a margin (blocks
/// can move out of it).
pub(crate) fn watch_box(d: &Design, spec: &DoorSpec) -> (Pos, Pos) {
    let (mut lo, mut hi) = d.bounds().unwrap();
    for p in spec.door_cells().iter().chain(spec.corridor_cells().iter()).chain(d.entities.iter().map(|e| e.cell()).collect::<Vec<_>>().iter()) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    ([lo[0] - 2, lo[1] - 2, lo[2] - 2], [hi[0] + 2, hi[1] + 2, hi[2] + 2])
}

/// Inclusive block box of a set of cells.
fn cell_box(cells: &[Pos]) -> (Pos, Pos) {
    let mut lo = cells[0];
    let mut hi = cells[0];
    for p in cells {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

/// Does an entity's bounding box overlap the inclusive block box (`lo`..=`hi`)? Touching faces do
/// not count.
fn overlaps(bbox: &[f64; 6], (lo, hi): (Pos, Pos)) -> bool {
    (0..3).all(|i| bbox[i + 3] > lo[i] as f64 && bbox[i] < (hi[i] + 1) as f64)
}

/// The 3x3 columns in front of and behind the door, `depth` blocks each way, as two block boxes.
fn front_back_boxes(spec: &DoorSpec, depth: i32) -> [(Pos, Pos); 2] {
    let axis = if spec.along_z { 0 } else { 2 };
    let cells = spec.in_front_and_behind(depth);
    let front: Vec<Pos> = cells.iter().filter(|p| p[axis] < spec.door[axis]).copied().collect();
    let back: Vec<Pos> = cells.iter().filter(|p| p[axis] > spec.door[axis]).copied().collect();
    [cell_box(&front), cell_box(&back)]
}

/// The opening and its walkway as one block box (3 wide, 3 high, 7 deep).
fn walk_box(spec: &DoorSpec) -> (Pos, Pos) {
    let mut cells = spec.door_cells();
    cells.extend(spec.corridor_cells());
    cell_box(&cells)
}

fn entity_line(e: &mctas::sim::EntityInfo) -> String {
    format!("{} at {:?} (motion {:?})", e.kind, e.pos, e.motion)
}

/// The design's entities at rest (after placement and settling): exactly the declared ones, all at
/// rest, inside the watch box, none in the 3x3 columns in front of or behind the door (at any
/// distance, like blocks) and none with collision in the walkway. Returns them (the state every
/// cycle must come back to).
fn entity_rest(
    w: &mut Worker,
    sb: u32,
    d: &Design,
    spec: &DoorSpec,
    opts: &VerifyOptions,
    watch: (Pos, Pos),
    uuids: &[String],
    report: &mut Report,
    first_position: bool,
) -> Result<Vec<mctas::sim::EntityInfo>> {
    let ents = w.entities(sb)?;
    if let Some(e) = ents.iter().find(|e| !e.design) {
        bail!("an entity the design does not declare is there at rest (something broke or dropped): {}", entity_line(e));
    }
    if ents.len() != d.entities.len() {
        bail!("{} of the design's {} entities are left after placement", ents.len(), d.entities.len());
    }
    for e in &ents {
        if !e.still {
            bail!("entity not at rest after placement: {}", entity_line(e));
        }
        if e.embedded {
            bail!("entity inside a block at rest (a survival player cannot place it there): {}", entity_line(e));
        }
        if !(0..3).all(|i| e.bbox[i] >= watch.0[i] as f64 && e.bbox[i + 3] <= (watch.1[i] + 1) as f64) {
            bail!("entity outside the watch box at rest: {}", entity_line(e));
        }
        if opts.empty_walkway {
            let reach = (watch.1[0] - watch.0[0]).max(watch.1[2] - watch.0[2]) + 1;
            if front_back_boxes(spec, reach).iter().any(|b| overlaps(&e.bbox, *b)) {
                bail!("entity in front of or behind the door at rest (nothing may stand in the 3x3 columns in front of and behind the door, at any distance): {}", entity_line(e));
            }
        }
        if e.collides && front_back_boxes(spec, 3).iter().any(|b| overlaps(&e.bbox, *b)) {
            bail!("not closed at rest: an entity with collision stands in the walkway: {}", entity_line(e));
        }
    }
    if first_position {
        // Report the entities that did not stay where the file put them (like blocks "not at
        // rest as written"); `uuids[i]` is the design's entity number i (summon order).
        for (spec_e, uuid) in d.entities.iter().zip(uuids) {
            if let Some(e) = ents.iter().find(|e| &e.uuid == uuid) {
                if e.pos != spec_e.pos {
                    report.warnings.push(format!("entity not at rest as written: {} {:?} settled at {:?}", spec_e.kind, spec_e.pos, e.pos));
                }
            }
        }
    }
    Ok(ents)
}

/// After a lever flip of a design with entities: no entity left the watch box, none appeared that
/// the design does not declare, and every declared one is still there.
fn entity_cycle_checks(w: &mut Worker, sb: u32, d: &Design, step: &mctas::sim::StepResult, what: &str) -> Result<Vec<mctas::sim::EntityInfo>> {
    if let Some(x) = &step.entity_escape {
        bail!("an entity left the watch box while {what}: {x}");
    }
    if let Some(x) = &step.entity_foreign {
        bail!("an entity the design does not declare appeared while {what} (something broke or dropped): {x}");
    }
    let ents = w.entities(sb)?;
    if let Some(e) = ents.iter().find(|e| !e.design) {
        bail!("an entity the design does not declare is there after {what}: {}", entity_line(e));
    }
    if ents.len() != d.entities.len() {
        bail!("{} of the design's {} entities are left after {what}", ents.len(), d.entities.len());
    }
    Ok(ents)
}

/// First 1-based index from which every remaining flag is true.
fn settled_from_flags(ok: &[bool]) -> Option<u32> {
    let mut first = None;
    for (i, &b) in ok.iter().enumerate() {
        if b {
            if first.is_none() {
                first = Some(i as u32 + 1);
            }
        } else {
            first = None;
        }
    }
    first
}

fn is_moving(state: &str) -> bool {
    state.starts_with("minecraft:moving_piston")
}

/// Blocks that may not be door blocks: the door must be made of plain building blocks.
fn is_mechanism_block(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    [
        "slime_block", "honey_block", "piston", "sticky_piston", "piston_head", "moving_piston", "observer",
        "redstone_block", "redstone_lamp", "target", "repeater", "comparator", "redstone_wire", "redstone_torch",
        "redstone_wall_torch", "lever", "dispenser", "dropper", "hopper", "note_block", "tnt",
    ]
    .iter()
    .any(|n| name == format!("minecraft:{n}"))
}

/// Door cells hold 9 identical full plain blocks; corridor is walkable.
pub(crate) fn closed_problems(r: &Region, spec: &DoorSpec) -> Vec<String> {
    let mut out = Vec::new();
    let first = r.get(spec.door_cells()[0]).unwrap_or("?").to_string();
    for p in spec.door_cells() {
        let s = r.get(p).unwrap_or("?");
        if s != first {
            out.push(format!("door cell {p:?} is {s}, but the door must be {} identical blocks ({first})", spec.door_cells().len()));
        }
        if is_mechanism_block(s) {
            out.push(format!("door cell {p:?} is {s}, not a plain door block"));
        }
    }
    for p in spec.door_cells() {
        let s = r.get(p).unwrap_or("?");
        let full = r.flags_at(p).is_some_and(|f| f.full);
        if !full || is_moving(s) {
            out.push(format!("door cell {p:?} is {s}"));
        }
    }
    for p in spec.corridor_cells() {
        if r.flags_at(p).is_some_and(|f| f.collides) {
            out.push(format!("corridor cell {p:?} is blocked by {}", r.get(p).unwrap_or("?")));
        }
    }
    out
}

/// Door and corridor cells are all walkable.
pub(crate) fn open_problems(r: &Region, spec: &DoorSpec) -> Vec<String> {
    let mut out = Vec::new();
    for p in spec.door_cells().into_iter().chain(spec.corridor_cells()) {
        let s = r.get(p).unwrap_or("?");
        if r.flags_at(p).is_some_and(|f| f.collides) || is_moving(s) {
            out.push(format!("{p:?} is {s}"));
        }
    }
    out
}

/// First index from which `ok(frame)` holds for every remaining frame (1-based tick count).
pub(crate) fn settled_from(frames: &[Region], ok: impl Fn(&Region) -> bool) -> Option<u32> {
    let mut first = None;
    for (i, f) in frames.iter().enumerate() {
        if ok(f) {
            if first.is_none() {
                first = Some(i as u32 + 1);
            }
        } else {
            first = None;
        }
    }
    first
}

fn verify_at(
    w: &mut Worker,
    sb: u32,
    d: &Design,
    spec: &DoorSpec,
    opts: &VerifyOptions,
    report: &mut Report,
    first_position: bool,
) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    // Compute redstone wire connections (raw placement skips shape updates), let every block
    // re-check its inputs once (no observer pulses), then settle.
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    // A design with entities (category entity): summon them once the blocks stand; settling then
    // also waits for every one of them to come to rest.
    let has_entities = !d.entities.is_empty();
    let uuids = if has_entities { w.summon(sb, &d.entities)? } else { Vec::new() };
    let settle = w.step_until_idle(sb, opts.max_ticks)?;
    if settle.idle != Some(true) {
        bail!("the build never settles after placement ({} ticks)", settle.ticked);
    }
    let (wlo, whi) = watch_box(d, spec);
    let rest = w.get_region(sb, wlo, whi)?;
    // Survival-buildability: nothing may float (levers, torches, dust, repeaters on air...), and
    // nothing may change by itself over time (random ticks: unwaxed copper oxidizes...).
    let (floating, ticking) = w.static_problems(sb, wlo, whi)?;
    if !floating.is_empty() {
        let list: Vec<String> = floating.iter().map(|p| format!("{p:?} {}", rest.get(*p).unwrap_or("?"))).collect();
        bail!("unsupported blocks (they could not stand there in survival): {}", list.join("; "));
    }
    if !ticking.is_empty() {
        let list: Vec<String> = ticking.iter().map(|p| format!("{p:?} {}", rest.get(*p).unwrap_or("?"))).collect();
        bail!("blocks that change by themselves over time (random ticks, e.g. unwaxed copper oxidizes; use waxed): {}", list.join("; "));
    }
    for (p, s) in &blocks {
        if rest.get(*p) != Some(s.as_str()) && first_position {
            report.warnings.push(format!("not at rest as written: {p:?} {s} became {}", rest.get(*p).unwrap_or("?")));
        }
    }
    let problems = closed_problems(&rest, spec);
    if !problems.is_empty() {
        bail!("not closed at rest: {}", problems.join("; "));
    }
    if opts.empty_walkway {
        // The 3x3 columns in front of and behind the door, out to the edge of the watch box.
        let reach = (whi[0] - wlo[0]).max(whi[2] - wlo[2]) + 1;
        let clutter: Vec<String> = spec
            .in_front_and_behind(reach)
            .iter()
            .filter_map(|p| rest.get(*p).filter(|s| !mctas::blockstate::is_air(s)).map(|s| format!("{p:?} {s}")))
            .collect();
        if !clutter.is_empty() {
            bail!(
                "blocks in front of or behind the door (nothing may stand in the 3x3 columns in front of and behind the door, at any distance, at rest: not even dust, torches, rails or levers): {}",
                clutter.join("; ")
            );
        }
    }
    let lever = rest.get(spec.lever).unwrap_or("?");
    // Either rest polarity counts (vjeux, 2026-10-19: "doesn't matter whether it's on or off for
    // the door"): the first flip from rest opens the door, the second closes it.
    if !lever.starts_with("minecraft:lever") {
        bail!("port lever {:?} is {lever}, expected a lever", spec.lever);
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap().to_string()).collect();
    // The entities' rest state, and the boxes the entity checks use.
    let rest_entities = if has_entities { entity_rest(w, sb, d, spec, opts, (wlo, whi), &uuids, report, first_position)? } else { Vec::new() };
    let watch = mctas::sim::EntityWatch { watch: (wlo, whi), walk: walk_box(spec) };

    // vjeux (2026-10-19): "internal state doesn't matter as long as the door works repeatedly".
    // So the build need not return to its exact rest state after each close: it must be PERIODIC.
    // Once the closed state after some cycle equals one seen before (the rest state or an earlier
    // closed state), the deterministic world repeats from there, so the door works forever. Every
    // cycle up to that point must open and close correctly; at most MAX_PERIOD_EXTRA extra cycles.
    let mut seen_closed: Vec<Region> = vec![rest.clone()];
    let mut periodic = false;
    for cycle in 0..opts.cycles + MAX_PERIOD_EXTRA {
        if cycle >= opts.cycles && periodic {
            break;
        }
        // Designs with entities: wait 0-20 extra ticks before each flip (a different count per
        // position, cycle and edge). A sensor held by an entity (a detector rail under a cart, a
        // pressure plate) re-checks every 10 or 20 ticks whatever the lever does, so a door must
        // not depend on where in that period the player flips the lever.
        let phase = |edge: i32| (spec.door[0] * 7 + spec.door[1] * 3 + spec.door[2] * 5 + cycle as i32 * 11 + edge * 13).rem_euclid(21) as u32;
        if has_entities && phase(0) > 0 {
            w.step(sb, phase(0))?;
        }
        // Open.
        w.use_block(sb, spec.lever)?;
        let open = if has_entities { w.step_record_watch(sb, opts.max_ticks, true, wlo, whi, &watch)? } else { w.step_record(sb, opts.max_ticks, true, wlo, whi)? };
        if open.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after opening", open.ticked);
        }
        let last = open.frames.last().context("no frames")?;
        let problems = open_problems(last, spec);
        if !problems.is_empty() {
            bail!("cycle {cycle}: not open after the lever was switched on: {}", problems.join("; "));
        }
        let open_ticks = if has_entities {
            entity_cycle_checks(w, sb, d, &open, "opening").with_context(|| format!("cycle {cycle}"))?;
            if open.entity_blocked.last() == Some(&true) {
                let ents: Vec<String> =
                    w.entities(sb)?.iter().filter(|e| e.collides && overlaps(&e.bbox, watch.walk)).map(entity_line).collect();
                bail!("cycle {cycle}: not open after the lever was switched on: an entity with collision stands in the opening or its walkway: {}", ents.join("; "));
            }
            // Open from the first tick where the blocks are walkable and no colliding entity is in
            // the way, for good.
            let ok: Vec<bool> =
                open.frames.iter().zip(open.entity_blocked.iter()).map(|(f, blocked)| !blocked && open_problems(f, spec).is_empty()).collect();
            settled_from_flags(&ok).unwrap()
        } else {
            settled_from(&open.frames, |f| open_problems(f, spec).is_empty()).unwrap()
        };
        if !has_entities && !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while opening (something broke): {:?}", w.entities(sb)?);
        }
        if has_entities && phase(1) > 0 {
            w.step(sb, phase(1))?;
        }
        // Close.
        w.use_block(sb, spec.lever)?;
        let close = if has_entities { w.step_record_watch(sb, opts.max_ticks, true, wlo, whi, &watch)? } else { w.step_record(sb, opts.max_ticks, true, wlo, whi)? };
        if close.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after closing", close.ticked);
        }
        let last = close.frames.last().context("no frames")?;
        let door_now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap().to_string()).collect();
        if door_now != closed_door {
            bail!(
                "cycle {cycle}: door not restored: {}",
                spec.door_cells()
                    .iter()
                    .zip(door_now.iter().zip(closed_door.iter()))
                    .filter(|(_, (a, b))| a != b)
                    .map(|(p, (a, b))| format!("{p:?} is {a}, was {b}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        // The closed state must still be closed (door cells, walkway), then periodic (see above).
        let problems = closed_problems(last, spec);
        if !problems.is_empty() {
            bail!("cycle {cycle}: not closed after the lever was switched back: {}", problems.join("; "));
        }
        if seen_closed.iter().any(|s| same_region(s, last)) {
            periodic = true;
        } else {
            seen_closed.push(last.clone());
        }
        if has_entities {
            // Every entity back where it rested (to 1e-6 block: a piston push leaves an entity at
            // the moving face + PUSH_OFFSET 0.01 + its half-width, a float (0.49F for a minecart
            // = 0.49000000953674316), computed in doubles (PistonMovingBlockEntity
            // .moveCollidedEntities), so a cart stated at x.5 comes back 1e-8 off it, then exactly
            // the same every cycle; any real displacement is at least 0.01), at rest, same state
            // (rail, contents, pose...).
            let now = entity_cycle_checks(w, sb, d, &close, "closing").with_context(|| format!("cycle {cycle}"))?;
            for r in &rest_entities {
                let Some(e) = now.iter().find(|e| e.uuid == r.uuid) else {
                    bail!("cycle {cycle}: the design's {} that rested at {:?} is gone after closing", r.kind, r.pos);
                };
                if (0..3).any(|i| (e.pos[i] - r.pos[i]).abs() > ENTITY_POS_TOLERANCE) {
                    bail!("cycle {cycle}: {} not back after closing: at {:?}, rested at {:?}", e.kind, e.pos, r.pos);
                }
                if !e.still {
                    bail!("cycle {cycle}: {} still moving after closing: {}", e.kind, entity_line(e));
                }
                if e.state != r.state {
                    bail!("cycle {cycle}: {} at {:?} changed after closing: {} (at rest: {})", e.kind, e.pos, e.state, r.state);
                }
            }
        } else if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while closing (something broke)");
        }
        let close_ticks = settled_from(&close.frames, |f| {
            spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))
        })
        .unwrap();
        if first_position && cycle == 0 {
            report.cycles.push(CycleReport { open_ticks, open_settle: open.ticked, close_ticks, close_settle: close.ticked });
            if opts.keep_frames {
                report.open_frames = open.frames;
                report.close_frames = close.frames;
            }
        } else if first_position {
            report.cycles.push(CycleReport { open_ticks, open_settle: open.ticked, close_ticks, close_settle: close.ticked });
        }
    }
    if !periodic {
        bail!(
            "the build never comes back to a closed state it had before within {} cycles (its internal state drifts, so it may stop working)",
            opts.cycles + MAX_PERIOD_EXTRA
        );
    }
    Ok(())
}

/// Extra cycles the judge runs to find the period of a door whose internal state differs between
/// cycles (vjeux: internal state doesn't matter as long as the door works repeatedly).
pub const MAX_PERIOD_EXTRA: u32 = 8;

/// Same block state in every cell of two regions of the same box (the judge's periodic-rest test;
/// shared by every tool that replays cycles: ideal, props-dyn, the search screens, tile).
pub fn same_region(a: &Region, b: &Region) -> bool {
    a.from == b.from && a.size == b.size && a.iter().all(|(p, s)| b.get(p) == Some(s))
}

/// One character per door cell for timelines.
pub fn cell_char(state: &str, collides: bool) -> char {
    if is_moving(state) {
        '~'
    } else if state.starts_with("minecraft:piston_head") {
        'h'
    } else if mctas::blockstate::is_air(state) {
        '.'
    } else if collides {
        '#'
    } else {
        ','
    }
}

/// ASCII timeline of the 3x3 opening, one line per tick: three rows top to bottom.
pub fn timeline(frames: &[Region], spec: &DoorSpec) -> String {
    let mut out = String::new();
    for (t, f) in frames.iter().enumerate() {
        let cells = spec.door_cells();
        let mut line = format!("  t={:<3}", t + 1);
        for row in 0..3 {
            line.push(' ');
            for col in 0..3 {
                let p = cells[row * 3 + col];
                let s = f.get(p).unwrap_or("?");
                line.push(cell_char(s, f.flags_at(p).is_some_and(|fl| fl.collides)));
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The design with every block replaced by the exact state it has once placed and settled
/// (wire connections, repeater flags...), so the file is the true rest state.
pub fn canonicalize(w: &mut Worker, d: &Design) -> Result<Design> {
    // A design written with stale redstone states (dust at power 0, a repeater off) can let a
    // piston fire before its signal settles, so the "rest" read back is a half-moved machine.
    // Settle, copy back only the redstone parts' states, and retry until the machine stays put.
    let is_redstone = |s: &str| {
        let n = mctas::blockstate::block_name(s);
        ["redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "observer", "redstone_lamp"]
            .iter()
            .any(|k| n == format!("minecraft:{k}"))
    };
    let mut cur = d.clone();
    for _ in 0..6 {
        let settled = canonicalize_once(w, &cur)?;
        let moved = cur.blocks.iter().any(|(p, s)| !is_redstone(s) && settled.blocks.get(p) != Some(s));
        if !moved {
            return Ok(settled);
        }
        let mut next = d.clone();
        for (p, s) in next.blocks.iter_mut() {
            if is_redstone(s) {
                *s = settled.blocks[p].clone();
            }
        }
        if next == cur {
            return Ok(settled);
        }
        cur = next;
    }
    canonicalize_once(w, &cur)
}

fn canonicalize_once(w: &mut Worker, d: &Design) -> Result<Design> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    if !d.entities.is_empty() {
        w.summon(sb, &d.entities)?;
    }
    let settle = w.step_until_idle(sb, 400)?;
    if settle.idle != Some(true) {
        bail!("design never settles");
    }
    let (lo, hi) = d.bounds().context("empty design")?;
    let r = w.get_region(sb, lo, hi)?;
    let _ = w.drop_sandbox(sb);
    let mut out = d.clone();
    for (p, s) in out.blocks.iter_mut() {
        *s = r.get(*p).context("outside region")?.to_string();
    }
    Ok(out)
}
