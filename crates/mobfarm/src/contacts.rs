//! The carts' block contacts: ONE shared definition for every judge (ironspec Nov 13 02:41 / 02:48 /
//! 02:52, amended 07:47 on G1's CHANGES 3; the siding bench's face filter lifted here). A contact is
//! an axis on which the game's collision CLIPPED the cart's ATTEMPTED move: `Entity.move(MoverType.SELF,
//! requested)` returning a different movement on that axis (`!Mth.equal`, the test `horizontalCollision`
//! uses), on the face that component pointed toward, the box located with the 0.011 / 0.01 test:
//! - the move is replayed in the game's step order: y first (the returned y), then the larger
//!   horizontal component of the REQUESTED move, then the other (`Entity.collideWithShapes`: z before
//!   x when |x| < |z|), each axis moved by its returned component;
//! - on a clipped horizontal axis, a collision box counts when the box at that step (after the
//!   returned movement on the axis) is within 0.011 of its face on the side the requested component
//!   points, and overlaps it by more than 0.01 on BOTH other axes (the vertical included), judged
//!   at that step, not after the whole move;
//! - an axis the collision did not clip is never a contact: a rail snap (`setPos`, no move), a wall
//!   beside a centred cart, the supports a resting slope cart's box passes through, a rail's support
//!   under a rolling cart;
//! - a resting cart pushed into a wall by its follower IS one (its pushed motion is the attempted
//!   move the wall clips).
//!
//! redsim records each `move(SELF, …)` per entity (`Entity::self_moves`: the box before, requested,
//! returned, the tick; `World::record_self_moves`, on by default, its no-change test in redsim);
//! [`cart_contacts`] judges a cart's moves of the world's current tick, and [`stale_declared`] the
//! declared stops no contact met. ONE contact rule (ironspec Nov 13 10:02): every bench's contact
//! counts go through these (ironfarm's siding bench, b8949c96's L1 rides, 58f770cc's C10, 7b90c3f1's
//! H14), each caller's control a planted undeclared contact failing through them.

use redsim::entity::collide::SelfMove;
use redsim::entity::vec3::Aabb;
use redsim::pos::BlockPos;
use redsim::world::World;

/// The face tolerance along the clipped axis (ironspec 02:48).
pub const FACE: f64 = 0.011;
/// The least overlap on the other two axes (ironspec 02:52).
pub const OVERLAP: f64 = 0.01;
/// The comparisons' rounding allowance: FACE and OVERLAP are decimal bounds a double can't hold
/// exactly, so a gap of exactly 0.011 counts and an overlap of exactly 0.01 doesn't (G1's edge
/// controls), whatever the last bit of the subtraction.
const EPS: f64 = 1e-9;

/// `Mth.equal(double, double)`: the game's "not clipped" test on an axis.
fn same(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5f32 as f64
}

/// The collision boxes `shapes` gives that clipped the move `requested` -> `returned` of a box at
/// `before` (one `Entity.move(SELF, …)`; (x, y, z) components).
pub fn face_contacts(shapes: &dyn Fn(Aabb) -> Vec<Aabb>, before: Aabb, requested: (f64, f64, f64), returned: (f64, f64, f64)) -> Vec<Aabb> {
    face_contacts_detail(shapes, before, requested, returned).into_iter().map(|(a, _, _)| a).collect()
}

/// A contacted block's face (the face the cart's clipped component pointed at: moving north, -z, the
/// cart meets the block's SOUTH face).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Face {
    North,
    South,
    East,
    West,
}

impl Face {
    pub fn parse(s: &str) -> Result<Face, String> {
        match s.trim() {
            "north" => Ok(Face::North),
            "south" => Ok(Face::South),
            "east" => Ok(Face::East),
            "west" => Ok(Face::West),
            o => Err(format!("face={o}: north, south, east or west")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Face::North => "north",
            Face::South => "south",
            Face::East => "east",
            Face::West => "west",
        }
    }
}

/// [`face_contacts`] with each clipping box's face (of the BLOCK) and the clipped depth on its axis
/// (|requested - returned|, ironspec Nov 13 10:42 (3): "the CLIPPED amount per tick, from SelfMove").
pub fn face_contacts_detail(shapes: &dyn Fn(Aabb) -> Vec<Aabb>, before: Aabb, requested: (f64, f64, f64), returned: (f64, f64, f64)) -> Vec<(Aabb, Face, f64)> {
    let mut b = before.move_by(0.0, returned.1, 0.0);
    let z_first = requested.0.abs() < requested.2.abs();
    let mut out: Vec<(Aabb, Face, f64)> = Vec::new();
    for axis in if z_first { ['z', 'x'] } else { ['x', 'z'] } {
        let (req, ret) = if axis == 'x' { (requested.0, returned.0) } else { (requested.2, returned.2) };
        b = if axis == 'x' { b.move_by(ret, 0.0, 0.0) } else { b.move_by(0.0, 0.0, ret) };
        if same(req, ret) {
            continue;
        }
        let face = match (axis, req > 0.0) {
            ('x', true) => Face::West,
            ('x', false) => Face::East,
            (_, true) => Face::North,
            (_, false) => Face::South,
        };
        for a in shapes(b.inflate(if axis == 'x' { FACE } else { 0.0 }, 0.0, if axis == 'z' { FACE } else { 0.0 })) {
            let oy = b.max_y.min(a.max_y) - b.min_y.max(a.min_y);
            let (other, gap) = if axis == 'x' {
                (b.max_z.min(a.max_z) - b.min_z.max(a.min_z), if req > 0.0 { a.min_x - b.max_x } else { b.min_x - a.max_x })
            } else {
                (b.max_x.min(a.max_x) - b.min_x.max(a.min_x), if req > 0.0 { a.min_z - b.max_z } else { b.min_z - a.max_z })
            };
            if oy > OVERLAP + EPS && other > OVERLAP + EPS && gap.abs() <= FACE + EPS && !out.iter().any(|(o, _, _)| *o == a) {
                out.push((a, face, (req - ret).abs()));
            }
        }
    }
    out
}

/// One recorded move judged on a world: each clipping box's cell (the box's centre, floored) and
/// block.
pub fn face_contacts_world(world: &World, mv: &SelfMove) -> Vec<(BlockPos, String)> {
    let shapes = |bb: Aabb| world.collision_shapes(bb);
    face_contacts(&shapes, mv.bb_before, (mv.requested.x, mv.requested.y, mv.requested.z), (mv.returned.x, mv.returned.y, mv.returned.z))
        .into_iter()
        .map(|a| {
            let p = BlockPos::new(((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32);
            (p, world.get(p).to_string())
        })
        .collect()
}

/// The contacts of the entity at index `i` over its moves of the world's current tick (call after the
/// tick).
pub fn cart_contacts(world: &World, i: usize) -> Vec<(BlockPos, String)> {
    let t = world.time;
    world.entities.list[i].self_moves.iter().filter(|m| m.tick == t).flat_map(|m| face_contacts_world(world, m)).collect()
}

/// The declared stops no contact met (G1's CHANGES 3 (2), ironspec Nov 13 10:02 (b)): a declaration
/// that never applies is STALE and fails its row; `hits` counts a cell's contacts over the run.
pub fn stale_declared(declared: &[BlockPos], hits: &dyn Fn(BlockPos) -> usize) -> Vec<BlockPos> {
    declared.iter().copied().filter(|p| hits(*p) == 0).collect()
}

/// A bounded declaration's class (ironspec Nov 13 10:42 (3), amended 10:58).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// An intended stop or guide (the drop backstop, the bed head's face): every bound from a cited
    /// reference run (`ref=`) plus a margin (`+eps=` or `why=`).
    Design,
    /// A touch the design neither wants nor minds: depth <= 0.01, ticks <= 1, no stall, speed stated.
    Incidental,
    /// The old form (`CELLS=REASON`, no fields): judged for STALE and stalls only, printed as unbounded.
    Legacy,
}

/// One bound: the value (`<=` a maximum or `>=` a minimum), its margin (`+eps=`) and its reason (`why=`).
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    pub value: f64,
    pub eps: f64,
    pub why: Option<String>,
    pub at_least: bool,
    /// The decimals the value was written with (the reference check's rounding: a declared 0.2953
    /// equals a measured 0.29531).
    pub decimals: u32,
}

impl Bound {
    /// The bound as judged: a maximum widened by eps, a minimum lowered by it.
    pub fn limit(&self) -> f64 {
        if self.at_least { self.value - self.eps } else { self.value + self.eps }
    }
    pub fn ok(&self, v: f64) -> bool {
        if self.at_least { v >= self.limit() - EPS } else { v <= self.limit() + EPS }
    }
}

/// A bounded contact declaration: `CELLS=REASON` then fields, each after a `|` or a `;`; a part that
/// starts with no known key belongs to the REASON (so a REASON may hold "; "). Keys: `face=`
/// north|south|east|west (the contacted BLOCK's face), `class=` design|incidental, `ticks` (per event:
/// a maximal run of consecutive contact ticks on one face), `events` (per cart over the row), `depth`
/// (the clipped amount per tick), `speed_in` (the cart's horizontal speed requested at an event's
/// first tick), `speed_out` (returned at its last tick), `speed=A..B→C..D` (both, ironspec's form),
/// `ref=` (the reference run: tip, command, seeds), `rate=H/N` (the reference's H hits over N samples,
/// for the rate STALE); a bound is `KEY<=V` / `KEY≤V` (a maximum) or `KEY>=V` / `KEY≥V` (a minimum) or
/// `KEY=A..B`, optionally `+eps=E` and ` why=TEXT` in the same part. A `why=` or `ref=` text can't
/// hold '|' or ';' (each starts the next field; ironspec 11:51 (1)(c)). A bound's VALUE is its
/// reference's extreme (`reference_check`); a design threshold is reached with `+eps=`, its reason in
/// `why=` (e.g. `speed_out>=0.0468+eps=0.0068 why=<the measured threshold>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Decl {
    pub cells: Vec<BlockPos>,
    pub reason: String,
    pub class: Class,
    pub face: Option<Face>,
    pub ticks: Option<Bound>,
    pub events: Option<Bound>,
    pub depth: Option<Bound>,
    pub speed_in: Vec<Bound>,
    pub speed_out: Vec<Bound>,
    pub reference: Option<String>,
    pub rate: Option<(u64, u64, String)>,
}

const KEYS: [&str; 11] = ["face", "class", "ticks", "events", "depth", "speed_in", "speed_out", "speed", "ref", "rate", "why"];

fn key_of(part: &str) -> Option<&'static str> {
    let p = part.trim_start();
    KEYS.iter().copied().filter(|k| p.starts_with(k) && p[k.len()..].starts_with(['=', '<', '>', '≤', '≥'])).max_by_key(|k| k.len())
}

/// `V`, `V+eps=E`, either followed by ` why=TEXT`.
fn bound_of(rest: &str, at_least: bool, what: &str) -> Result<Bound, String> {
    let (val, why) = match rest.split_once(" why=") {
        Some((v, w)) => (v.trim(), Some(w.trim().to_string())),
        None => (rest.trim(), None),
    };
    let (v, eps) = match val.split_once("+eps=") {
        Some((v, e)) => (v.trim(), e.trim().parse::<f64>().map_err(|e| format!("{what}: eps: {e}"))?),
        None => (val, 0.0),
    };
    let value = v.parse::<f64>().map_err(|e| format!("{what}: {v:?}: {e}"))?;
    let decimals = v.split_once('.').map_or(0, |(_, f)| f.len() as u32);
    Ok(Bound { value, eps, why, at_least, decimals })
}

/// `KEY<=V`, `KEY>=V` (or ≤ / ≥) or `KEY=A..B` into its bounds.
fn bounds_of(part: &str, key: &str) -> Result<Vec<Bound>, String> {
    let rest = &part.trim_start()[key.len()..];
    let what = part.trim();
    if let Some(r) = rest.strip_prefix("<=").or_else(|| rest.strip_prefix('≤')) {
        return Ok(vec![bound_of(r, false, what)?]);
    }
    if let Some(r) = rest.strip_prefix(">=").or_else(|| rest.strip_prefix('≥')) {
        return Ok(vec![bound_of(r, true, what)?]);
    }
    if let Some(r) = rest.strip_prefix('=') {
        let (range, why) = match r.split_once(" why=") {
            Some((v, w)) => (v.trim(), Some(w.trim().to_string())),
            None => (r.trim(), None),
        };
        let (a, b) = range.split_once("..").ok_or_else(|| format!("{what}: A..B"))?;
        let lo = bound_of(a, true, what)?;
        let mut hi = bound_of(b, false, what)?;
        hi.why = why;
        return Ok(vec![lo, hi]);
    }
    Err(format!("{what}: {key}<=V, {key}>=V or {key}=A..B"))
}

/// The bounded `# contacts:` / `# stops:` reader (ironspec Nov 13 10:42 (3), 10:58): strict, every
/// field parsed or an error; a design-class entry needs `ref=` and a margin (`+eps=` or `why=`) on every
/// bound it states, and states face, ticks, events, depth and a speed; an incidental one may not
/// loosen depth <= 0.01 or ticks <= 1 and states a speed.
pub fn declared_contacts_bounded(text: &str) -> Result<Vec<Decl>, String> {
    let mut out = Vec::new();
    for key in ["contacts:", "stops:"] {
        for entry in crate::pushscan::header_entries(text, key) {
            let (cells, rest) = entry.split_once('=').ok_or_else(|| format!("# {key} {entry}: x,y,z[+x,y,z…]=REASON"))?;
            let mut reason = String::new();
            let mut fields: Vec<&str> = Vec::new();
            for part in rest.split(['|', ';']) {
                if key_of(part).is_some() {
                    fields.push(part);
                } else if fields.is_empty() {
                    if !reason.is_empty() {
                        reason.push(';');
                    }
                    reason.push_str(part);
                } else {
                    return Err(format!("# {key} {entry}: {part:?} is no field (face= class= ticks events depth speed speed_in speed_out ref= rate=; a why= or ref= text can't hold '|' or ';', which start the next field)"));
                }
            }
            let reason = reason.trim().to_string();
            if reason.is_empty() {
                return Err(format!("# {key} {entry}: an empty REASON (x,y,z[+x,y,z…]=REASON)"));
            }
            let mut cs = Vec::new();
            for cell in cells.split('+') {
                let v: Vec<i32> = cell.trim().trim_matches(|c| c == '(' || c == ')').split(',').map(|t| t.trim().replace('\u{2212}', "-").parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("# {key} {entry}: cell {cell:?}: {e}"))?;
                if v.len() != 3 {
                    return Err(format!("# {key} {entry}: cell {cell:?} is not x,y,z"));
                }
                cs.push(BlockPos::new(v[0], v[1], v[2]));
            }
            let mut d = Decl { cells: cs, reason, class: Class::Legacy, face: None, ticks: None, events: None, depth: None, speed_in: Vec::new(), speed_out: Vec::new(), reference: None, rate: None };
            let mut class: Option<Class> = None;
            for f in &fields {
                let k = key_of(f).unwrap_or_default();
                let val = || f.trim_start()[k.len()..].trim_start_matches('=').trim().to_string();
                match k {
                    "face" => d.face = Some(Face::parse(&val())?),
                    "class" => {
                        class = Some(match val().as_str() {
                            "design" => Class::Design,
                            "incidental" => Class::Incidental,
                            o => return Err(format!("# {key} {entry}: class={o}: design or incidental")),
                        })
                    }
                    "ref" => d.reference = Some(val()),
                    "rate" => {
                        // (rate=H/N UNIT: the reference's H hits over its N samples, counted in UNIT
                        // (passes, visits, rows, ...): the row's samples are given in the same unit)
                        let v = val();
                        let (hn, unit) = v.split_once(' ').ok_or_else(|| format!("# {key} {entry}: rate=H/N UNIT (the sample unit: passes, visits, …)"))?;
                        let (h, n) = hn.split_once('/').ok_or_else(|| format!("# {key} {entry}: rate=H/N UNIT"))?;
                        let unit = unit.trim();
                        if unit.is_empty() || unit.contains(' ') {
                            return Err(format!("# {key} {entry}: rate=H/N UNIT: one word for the unit"));
                        }
                        let (h, n): (u64, u64) = (h.trim().parse().map_err(|e| format!("# {key} {entry}: rate: {e}"))?, n.trim().parse().map_err(|e| format!("# {key} {entry}: rate: {e}"))?);
                        if n == 0 || h > n {
                            return Err(format!("# {key} {entry}: rate={h}/{n}: 0 <= H <= N, N > 0"));
                        }
                        d.rate = Some((h, n, unit.to_string()));
                    }
                    "why" => return Err(format!("# {key} {entry}: a why= belongs to a bound (KEY<=V why=…)")),
                    "ticks" | "events" | "depth" => {
                        let b = bounds_of(f, k).map_err(|e| format!("# {key} {entry}: {e}"))?;
                        let mx = b.into_iter().find(|b| !b.at_least).ok_or_else(|| format!("# {key} {entry}: {k} takes a maximum"))?;
                        *match k {
                            "ticks" => &mut d.ticks,
                            "events" => &mut d.events,
                            _ => &mut d.depth,
                        } = Some(mx);
                    }
                    "speed_in" => d.speed_in = bounds_of(f, k).map_err(|e| format!("# {key} {entry}: {e}"))?,
                    "speed_out" => d.speed_out = bounds_of(f, k).map_err(|e| format!("# {key} {entry}: {e}"))?,
                    _ => {
                        // (speed=A..B→C..D[+eps=E][ why=TEXT]: the margin applies to all four bounds)
                        let v = val();
                        let (v, why) = match v.split_once(" why=") {
                            Some((a, w)) => (a.trim().to_string(), Some(w.trim().to_string())),
                            None => (v, None),
                        };
                        let (a, b) = v.split_once('→').or_else(|| v.split_once("->")).ok_or_else(|| format!("# {key} {entry}: speed=A..B→C..D"))?;
                        d.speed_in = bounds_of(&format!("speed_in={a}"), "speed_in").map_err(|e| format!("# {key} {entry}: {e}"))?;
                        d.speed_out = bounds_of(&format!("speed_out={b}"), "speed_out").map_err(|e| format!("# {key} {entry}: {e}"))?;
                        for bd in d.speed_in.iter_mut().chain(d.speed_out.iter_mut()) {
                            if bd.why.is_none() {
                                bd.why = why.clone();
                            }
                        }
                    }
                }
            }
            d.class = match (class, fields.is_empty()) {
                (Some(c), _) => c,
                (None, true) => Class::Legacy,
                (None, false) => return Err(format!("# {key} {entry}: a bounded entry needs class=design|incidental")),
            };
            let stated = |b: &Option<Bound>| b.is_some();
            match d.class {
                Class::Design => {
                    let mut missing: Vec<&str> = Vec::new();
                    for (n, ok) in [("face", d.face.is_some()), ("ticks", stated(&d.ticks)), ("events", stated(&d.events)), ("depth", stated(&d.depth)), ("a speed", !d.speed_in.is_empty() || !d.speed_out.is_empty()), ("ref", d.reference.is_some())] {
                        if !ok {
                            missing.push(n);
                        }
                    }
                    if !missing.is_empty() {
                        return Err(format!("# {key} {entry}: class=design states every bound from its reference run: missing {}", missing.join(", ")));
                    }
                    let all: Vec<&Bound> = [&d.ticks, &d.events, &d.depth].into_iter().flatten().chain(d.speed_in.iter()).chain(d.speed_out.iter()).collect();
                    if all.iter().any(|b| b.eps == 0.0 && b.why.is_none()) {
                        return Err(format!("# {key} {entry}: class=design: every bound carries its margin (+eps=E or why=…)"));
                    }
                }
                Class::Incidental => {
                    if d.depth.as_ref().is_some_and(|b| b.limit() > 0.01 + EPS) || d.ticks.as_ref().is_some_and(|b| b.limit() > 1.0 + EPS) {
                        return Err(format!("# {key} {entry}: class=incidental: depth <= 0.01 and ticks <= 1 at most"));
                    }
                    if d.speed_in.is_empty() && d.speed_out.is_empty() {
                        return Err(format!("# {key} {entry}: class=incidental states its speed"));
                    }
                    d.depth.get_or_insert(Bound { value: 0.01, eps: 0.0, why: Some("incidental".into()), at_least: false, decimals: 2 });
                    d.ticks.get_or_insert(Bound { value: 1.0, eps: 0.0, why: Some("incidental".into()), at_least: false, decimals: 0 });
                }
                Class::Legacy => {}
            }
            out.push(d);
        }
    }
    Ok(out)
}

/// One contact as the bounded judge reads it: a cart's clipped move on a block's face at a tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub tick: i64,
    pub cart: u32,
    pub cell: BlockPos,
    pub face: Face,
    /// The clipped amount on the face's axis this tick.
    pub depth: f64,
    /// The horizontal speed requested, and returned, by that move.
    pub speed_in: f64,
    pub speed_out: f64,
    /// The row's sample this hit belongs to (a pass, a visit: the unit of a declaration's `rate=`),
    /// set by the caller (ironspec 11:51 (1)(b): a rate's numerator counts samples with a hit, not
    /// ticks); `cart_hits` leaves it None.
    pub sample: Option<u64>,
}

/// The entity at index `i`'s contacts over its moves of the world's current tick, as [`Hit`]s.
pub fn cart_hits(world: &World, i: usize) -> Vec<Hit> {
    let t = world.time;
    let e = &world.entities.list[i];
    let shapes = |bb: Aabb| world.collision_shapes(bb);
    let mut out = Vec::new();
    for m in e.self_moves.iter().filter(|m| m.tick == t) {
        for (a, face, depth) in face_contacts_detail(&shapes, m.bb_before, (m.requested.x, m.requested.y, m.requested.z), (m.returned.x, m.returned.y, m.returned.z)) {
            let cell = BlockPos::new(((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32);
            out.push(Hit { tick: t, cart: e.id, cell, face, depth, speed_in: m.requested.x.hypot(m.requested.z), speed_out: m.returned.x.hypot(m.returned.z), sample: None });
        }
    }
    out
}

/// The vertical axis's classes (G1 Nov 13 09:56, ironspec 10:54 (a)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VClass {
    /// The cart's box intersects a collision shape at the tick's end (deeper than 0.01 on every axis):
    /// a cart summoned inside a block, or pressed into one.
    Embedded,
    /// A collision shape's bottom face within 0.05 above the cart's box top, overlapping it by more
    /// than 0.01 on both horizontal axes (a ceiling 0.04 above flags, 0.06 doesn't): a rising cart or a
    /// rider-pushed one would meet it.
    LowCeiling,
    /// An OFF-RAIL cart's attempted move clipped downward: it came to rest on a block's top (the lip
    /// rest on C's support is this class). On a rail the rail sets the cart's y, and its support's clip
    /// is no contact.
    Floor,
    /// An off-rail cart's attempted move clipped upward: it rose into a block's bottom.
    Ceiling,
}

/// The low-ceiling bound: a shape's bottom face this far above the cart's box top or closer.
pub const CEILING: f64 = 0.05;

/// One vertical contact: the cart, the class, the shape's cell, and the depth (the overlap for
/// Embedded, the gap for LowCeiling, the clipped y for Floor and Ceiling).
#[derive(Clone, Debug, PartialEq)]
pub struct VHit {
    pub tick: i64,
    pub cart: u32,
    pub class: VClass,
    pub cell: BlockPos,
    pub depth: f64,
}

fn cell_of(a: &Aabb) -> BlockPos {
    BlockPos::new(((a.min_x + a.max_x) / 2.0).floor() as i32, ((a.min_y + a.max_y) / 2.0).floor() as i32, ((a.min_z + a.max_z) / 2.0).floor() as i32)
}

/// The vertical contacts of a box `end` (the cart's box at the tick's end) given its tick's moves
/// (requested, returned, the box before each) and whether it is off its rail: LowCeiling from the end
/// box; Embedded (from the end box) and Floor / Ceiling (from each move's clipped y) when OFF-RAIL only
/// (ironspec 11:51 (2), the blocking finding on 6346783f0: a cart riding an ascending rail overlaps the
/// next cell's support by more than 0.01 on every axis in normal operation (the napper resting on R at
/// z 7.895 overlaps F's glass by 0.385 in z and 0.0425 in y), which 5fa67f60a's own negative calls
/// normal).
pub fn vertical_contacts(shapes: &dyn Fn(Aabb) -> Vec<Aabb>, end: Aabb, moves: &[(Aabb, (f64, f64, f64), (f64, f64, f64))], off_rail: bool) -> Vec<(VClass, Aabb, f64)> {
    let mut out: Vec<(VClass, Aabb, f64)> = Vec::new();
    for a in if off_rail { shapes(end) } else { Vec::new() } {
        let ox = end.max_x.min(a.max_x) - end.min_x.max(a.min_x);
        let oy = end.max_y.min(a.max_y) - end.min_y.max(a.min_y);
        let oz = end.max_z.min(a.max_z) - end.min_z.max(a.min_z);
        if ox > OVERLAP + EPS && oy > OVERLAP + EPS && oz > OVERLAP + EPS {
            out.push((VClass::Embedded, a, ox.min(oy).min(oz)));
        }
    }
    let above = Aabb::new(end.min_x, end.max_y, end.min_z, end.max_x, end.max_y + CEILING, end.max_z);
    for a in shapes(above) {
        let gap = a.min_y - end.max_y;
        let ox = end.max_x.min(a.max_x) - end.min_x.max(a.min_x);
        let oz = end.max_z.min(a.max_z) - end.min_z.max(a.min_z);
        if gap >= -EPS && gap <= CEILING + EPS && ox > OVERLAP + EPS && oz > OVERLAP + EPS && !out.iter().any(|(c, o, _)| *c == VClass::Embedded && *o == a) {
            out.push((VClass::LowCeiling, a, gap));
        }
    }
    if off_rail {
        for (before, req, ret) in moves {
            if same(req.1, ret.1) {
                continue;
            }
            let b = before.move_by(0.0, ret.1, 0.0);
            let (class, probe) = if req.1 < 0.0 {
                (VClass::Floor, Aabb::new(b.min_x, b.min_y - FACE, b.min_z, b.max_x, b.min_y, b.max_z))
            } else {
                (VClass::Ceiling, Aabb::new(b.min_x, b.max_y, b.min_z, b.max_x, b.max_y + FACE, b.max_z))
            };
            for a in shapes(probe) {
                let ox = b.max_x.min(a.max_x) - b.min_x.max(a.min_x);
                let oz = b.max_z.min(a.max_z) - b.min_z.max(a.min_z);
                let face_gap = if class == VClass::Floor { b.min_y - a.max_y } else { a.min_y - b.max_y };
                if ox > OVERLAP + EPS && oz > OVERLAP + EPS && face_gap.abs() <= FACE + EPS && !out.iter().any(|(c, o, _)| *c == class && *o == a) {
                    out.push((class, a, (req.1 - ret.1).abs()));
                }
            }
        }
    }
    out
}

/// The entity at index `i`'s vertical contacts at the world's current tick (call after the tick): its
/// box now, its SELF moves of this tick, off-rail when neither its block nor the one below is a rail.
pub fn cart_vertical_hits(world: &World, i: usize) -> Vec<VHit> {
    let t = world.time;
    let e = &world.entities.list[i];
    let shapes = |bb: Aabb| world.collision_shapes(bb);
    let feet = BlockPos::new(e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32);
    let rail = |p: BlockPos| world.get(p).to_string().contains("rail");
    let off_rail = !rail(feet) && !rail(BlockPos::new(feet.x, feet.y - 1, feet.z));
    let moves: Vec<(Aabb, (f64, f64, f64), (f64, f64, f64))> = e.self_moves.iter().filter(|m| m.tick == t).map(|m| (m.bb_before, (m.requested.x, m.requested.y, m.requested.z), (m.returned.x, m.returned.y, m.returned.z))).collect();
    vertical_contacts(&shapes, e.bb, &moves, off_rail).into_iter().map(|(class, a, depth)| VHit { tick: t, cart: e.id, class, cell: cell_of(&a), depth }).collect()
}

/// The bounded judge's verdict over a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verdict {
    /// Contacts no declaration covers (cell and face).
    pub undeclared: Vec<Hit>,
    /// Each bound exceeded, named: the declaration's cells, the bound, the measured value, the event.
    pub exceeded: Vec<String>,
    /// Events after which the cart neither moved nor fell within the stall rule's window.
    pub stalls: Vec<String>,
    /// Declarations judged STALE: never met (no `rate=`), or with `rate=` and the row's samples, met 0
    /// times where 0 has a probability under 0.001, or met so few times that P(X <= hits) < 0.001.
    pub stale: Vec<String>,
    /// Declarations whose staleness the row can't judge (printed, not failing): a `rate=` declaration
    /// met 0 times where 0 is not improbable (P >= 0.001).
    pub stale_not_judged: Vec<String>,
    /// The caller's wiring errors (G1 Nov 13 11:20): a `rate=` declaration judged with no samples, or
    /// with samples in another unit, FAILS loudly, naming the declaration and the row's unit.
    pub errors: Vec<String>,
    /// Per declaration: its events over the row.
    pub events: Vec<usize>,
}

impl Verdict {
    pub fn failed(&self) -> bool {
        !self.undeclared.is_empty() || !self.exceeded.is_empty() || !self.stalls.is_empty() || !self.stale.is_empty() || !self.errors.is_empty()
    }
}

fn cells_name(d: &Decl) -> String {
    d.cells.iter().map(|c| format!("({},{},{})", c.x, c.y, c.z)).collect::<Vec<_>>().join("+")
}

/// NO STALL's rule: after an event's last tick, within `window` ticks the cart moves more than
/// `min_speed` b/t horizontally or falls (vy below -`min_speed`). `IRONSPEC_1042` is the ruled default
/// (ironspec Nov 13 10:42 (3): "within 5 ticks of the event's end the cart moves > 0.003 b/t or is
/// falling"); a caller ties it to the game's normal-pass envelope (G1 11:12 (a): tick-exact's normal
/// lip passes sit up to 10-12 ticks off-rail on C's support top: a window from the envelope's longest
/// still time plus a margin), stating its source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StallRule {
    pub window: i64,
    pub min_speed: f64,
}

impl StallRule {
    pub const IRONSPEC_1042: StallRule = StallRule { window: 5, min_speed: 0.003 };
}

/// `P(X <= k)` for X ~ Binomial(n, p).
pub fn binomial_cdf(k: u64, n: u64, p: f64) -> f64 {
    if p <= 0.0 {
        return 1.0;
    }
    if p >= 1.0 {
        return if k >= n { 1.0 } else { 0.0 };
    }
    let (lp, lq) = (p.ln(), (1.0 - p).ln());
    // (log C(n, i) built up term by term)
    let mut lc = 0.0f64;
    let mut sum = 0.0f64;
    for i in 0..=k.min(n) {
        if i > 0 {
            lc += ((n - i + 1) as f64).ln() - (i as f64).ln();
        }
        sum += (lc + i as f64 * lp + (n - i) as f64 * lq).exp();
    }
    sum.min(1.0)
}

/// The ONE bounded judge (ironspec Nov 13 10:42 (3), 10:58): every hit against the declarations;
/// events (maximal runs of consecutive ticks per cart, cell and face) against ticks, depth and speed;
/// each cart's events per declaration against events; NO STALL after any event (`motion(cart, tick)`:
/// the cart's horizontal speed and vy at that tick; None = no record, read as not moving: fail-closed;
/// judged by `stall`); STALE (ironspec 10:47's ruled form, G1 11:12): without `rate=`, a declaration
/// with no hit; with `rate=H/N UNIT` and the row's `samples` in the same UNIT: 0 hits where
/// (1 - H/N)^samples < 0.001 is STALE, 0 hits otherwise "stale not judged"; and some hits but
/// P(X <= hits) < 0.001 under the reference rate (an under-rate) is STALE too.
pub fn judge(decls: &[Decl], hits: &[Hit], motion: &dyn Fn(u32, i64) -> Option<(f64, f64)>, stall: StallRule, samples: Option<(u64, &str)>) -> Verdict {
    let mut v = Verdict { events: vec![0; decls.len()], ..Default::default() };
    let mut by: std::collections::BTreeMap<(usize, u32, Face), Vec<&Hit>> = std::collections::BTreeMap::new();
    let mut per_decl_hits = vec![0usize; decls.len()];
    for h in hits {
        match decls.iter().position(|d| d.cells.contains(&h.cell) && d.face.is_none_or(|f| f == h.face)) {
            Some(di) => {
                per_decl_hits[di] += 1;
                // (ironspec 11:51 (1)(a): a declaration's cells are ONE stop: runs merge per
                // declaration, cart and face, a tick with a hit on any of its cells)
                by.entry((di, h.cart, h.face)).or_default().push(h);
            }
            None => v.undeclared.push(h.clone()),
        }
    }
    let moving = |cart: u32, t: i64| motion(cart, t).is_some_and(|(sp, vy)| sp > stall.min_speed || vy < -stall.min_speed);
    let mut cart_events: std::collections::BTreeMap<(usize, u32), usize> = std::collections::BTreeMap::new();
    for ((di, cart, face), mut hs) in by {
        hs.sort_by_key(|h| h.tick);
        let d = &decls[di];
        for r in events_of(&hs) {
            let (t0, t1) = (r[0].tick, r[r.len() - 1].tick);
            let ticks = (t1 - t0 + 1) as f64;
            let ev = format!("{} {} face, cart {cart}, t {t0}..{t1}", cells_name(d), face.name());
            if let Some(b) = &d.ticks
                && !b.ok(ticks)
            {
                v.exceeded.push(format!("{ev}: ticks {ticks} over ticks<={}", b.limit()));
            }
            let deepest = r.iter().map(|h| h.depth).fold(0.0f64, f64::max);
            if let Some(b) = &d.depth
                && !b.ok(deepest)
            {
                v.exceeded.push(format!("{ev}: depth {deepest:.4} over depth<={:.4}", b.limit()));
            }
            for b in &d.speed_in {
                if !b.ok(r[0].speed_in) {
                    v.exceeded.push(format!("{ev}: speed_in {:.4} outside {}{:.4}", r[0].speed_in, if b.at_least { ">=" } else { "<=" }, b.limit()));
                }
            }
            for b in &d.speed_out {
                let so = r[r.len() - 1].speed_out;
                if !b.ok(so) {
                    v.exceeded.push(format!("{ev}: speed_out {so:.4} outside {}{:.4}", if b.at_least { ">=" } else { "<=" }, b.limit()));
                }
            }
            if !(t1 + 1..=t1 + stall.window).any(|t| moving(cart, t)) {
                v.stalls.push(format!("{ev}: no move over {} b/t and no fall within {} ticks of the event's end", stall.min_speed, stall.window));
            }
            v.events[di] += 1;
            *cart_events.entry((di, cart)).or_default() += 1;
        }
    }
    for ((di, cart), n) in cart_events {
        if let Some(b) = &decls[di].events
            && !b.ok(n as f64)
        {
            v.exceeded.push(format!("{}: cart {cart}: {n} events over events<={}", cells_name(&decls[di]), b.limit()));
        }
    }
    for (di, d) in decls.iter().enumerate() {
        let n = per_decl_hits[di] as u64;
        match &d.rate {
            None => {
                if n == 0 {
                    v.stale.push(format!("{}: never met", cells_name(d)));
                }
            }
            Some((h, nr, unit)) => {
                let p = *h as f64 / *nr as f64;
                // (ironspec 11:51 (1)(b): the numerator counts SAMPLES with a hit, as rate=H/N does,
                // never hit ticks (a multi-tick contact would over-count and the under-rate fail open):
                // the caller tags each hit with its sample (`Hit.sample`); untagged hits are an error)
                let mine: Vec<&Hit> = hits.iter().filter(|x| d.cells.contains(&x.cell) && d.face.is_none_or(|f| f == x.face)).collect();
                if mine.iter().any(|x| x.sample.is_none()) {
                    v.errors.push(format!("{}: it carries rate={h}/{nr} {unit}, and its hits carry no sample: a wiring error (ironspec 11:51 (1)(b)): tag each Hit with its sample", cells_name(d)));
                    continue;
                }
                let n = mine.iter().filter_map(|x| x.sample).collect::<std::collections::BTreeSet<u64>>().len() as u64;
                match samples {
                    Some((k, u)) if u == unit => {
                        let tail = binomial_cdf(n, k, p);
                        if n == 0 && tail < 0.001 {
                            v.stale.push(format!("{}: never met over {k} {unit}, P(0 | the reference's {h}/{nr}) = {tail:.2e} < 0.001", cells_name(d)));
                        } else if n == 0 {
                            v.stale_not_judged.push(format!("{}: never met over {k} {unit}, but P(0 | {h}/{nr}) = {tail:.3} >= 0.001: stale not judged", cells_name(d)));
                        } else if tail < 0.001 {
                            v.stale.push(format!("{}: {n} hits over {k} {unit}, under the reference's {h}/{nr}: P(X <= {n}) = {tail:.2e} < 0.001", cells_name(d)));
                        }
                    }
                    Some((k, u)) => v.errors.push(format!("{}: its rate is counted in {unit}, the row's samples in {u} ({k}): a wiring error (G1 11:20): count the row's samples in {unit}", cells_name(d))),
                    None => v.errors.push(format!("{}: it carries rate={h}/{nr} {unit}, and the row gave no samples: a wiring error (G1 11:20): count the row's {unit}", cells_name(d))),
                }
            }
        }
    }
    v
}

/// The maximal runs of consecutive ticks among `hs` (sorted by tick).
fn events_of<'a>(hs: &[&'a Hit]) -> Vec<Vec<&'a Hit>> {
    let mut runs: Vec<Vec<&Hit>> = Vec::new();
    for h in hs {
        match runs.last_mut() {
            Some(r) if r.last().is_some_and(|l| h.tick <= l.tick + 1) => r.push(h),
            _ => runs.push(vec![h]),
        }
    }
    runs
}

/// The reference re-run check (ironspec Nov 13 10:58, G1 11:12 (b)): judged on its own `ref=` run, a
/// design declaration's every stated bound must EQUAL that run's measured extreme (a maximum's value =
/// the run's largest, a minimum's = its least), to the decimals the value was written with; each
/// mismatch named. Returns the mismatches (empty: the declaration reproduces its reference).
pub fn reference_check(d: &Decl, hits: &[Hit]) -> Vec<String> {
    let mine: Vec<&Hit> = hits.iter().filter(|h| d.cells.contains(&h.cell) && d.face.is_none_or(|f| f == h.face)).collect();
    let mut out = Vec::new();
    if mine.is_empty() {
        out.push(format!("{}: its reference run has no hit on it", cells_name(d)));
        return out;
    }
    let mut groups: std::collections::BTreeMap<(u32, Face), Vec<&Hit>> = std::collections::BTreeMap::new();
    for h in &mine {
        groups.entry((h.cart, h.face)).or_default().push(h);
    }
    let (mut max_ticks, mut max_depth) = (0.0f64, 0.0f64);
    let mut per_cart: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let (mut sin, mut sout): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for ((cart, _), mut hs) in groups {
        hs.sort_by_key(|h| h.tick);
        for r in events_of(&hs) {
            max_ticks = max_ticks.max((r[r.len() - 1].tick - r[0].tick + 1) as f64);
            max_depth = max_depth.max(r.iter().map(|h| h.depth).fold(0.0, f64::max));
            sin.push(r[0].speed_in);
            sout.push(r[r.len() - 1].speed_out);
            *per_cart.entry(cart).or_default() += 1;
        }
    }
    let max_events = per_cart.values().copied().max().unwrap_or(0) as f64;
    let ext = |v: &[f64], least: bool| if least { v.iter().copied().fold(f64::MAX, f64::min) } else { v.iter().copied().fold(f64::MIN, f64::max) };
    let mut check = |name: &str, b: &Bound, measured: f64| {
        let tol = 0.5 * 10f64.powi(-(b.decimals as i32)) + 1e-12;
        if (b.value - measured).abs() > tol {
            out.push(format!("{}: {name}{}{} but its reference's {} is {measured:.6}", cells_name(d), if b.at_least { ">=" } else { "<=" }, b.value, if b.at_least { "least" } else { "largest" }));
        }
    };
    if let Some(b) = &d.ticks {
        check("ticks", b, max_ticks);
    }
    if let Some(b) = &d.events {
        check("events", b, max_events);
    }
    if let Some(b) = &d.depth {
        check("depth", b, max_depth);
    }
    for b in &d.speed_in {
        check("speed_in", b, ext(&sin, b.at_least));
    }
    for b in &d.speed_out {
        check("speed_out", b, ext(&sout, b.at_least));
    }
    out
}

/// A design's declared contacts, strictly (G1's CHANGES 3 (3)): every `# contacts:` and `# stops:`
/// line's entries (split by the shared `pushscan::header_entries`, so a REASON may hold "; "), each
/// `x,y,z[+x,y,z…]=REASON`; a malformed cell or an entry with no '=' is an error, never dropped.
pub fn declared_contacts_strict(text: &str) -> Result<Vec<(BlockPos, String)>, String> {
    let mut out = Vec::new();
    for key in ["contacts:", "stops:"] {
        for entry in crate::pushscan::header_entries(text, key) {
            let (cells, reason) = entry.split_once('=').ok_or_else(|| format!("# {key} {entry}: x,y,z[+x,y,z…]=REASON"))?;
            // (G1's note on 329b5acc3: a declaration with an empty REASON is as bad as none)
            if reason.trim().is_empty() {
                return Err(format!("# {key} {entry}: an empty REASON (x,y,z[+x,y,z…]=REASON)"));
            }
            for cell in cells.split('+') {
                let v: Vec<i32> = cell.trim().trim_matches(|c| c == '(' || c == ')').split(',').map(|t| t.trim().replace('\u{2212}', "-").parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("# {key} {entry}: cell {cell:?}: {e}"))?;
                if v.len() != 3 {
                    return Err(format!("# {key} {entry}: cell {cell:?} is not x,y,z"));
                }
                out.push((BlockPos::new(v[0], v[1], v[2]), reason.trim().to_string()));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boxes of full blocks at `cells`, as `World::collision_shapes` gives them (those meeting the query box).
    fn blocks(cells: &[(i32, i32, i32)]) -> impl Fn(Aabb) -> Vec<Aabb> + '_ {
        move |q: Aabb| {
            cells
                .iter()
                .map(|&(x, y, z)| Aabb::new(x as f64, y as f64, z as f64, x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0))
                .filter(|a| a.intersects(q) || a.min_x == q.max_x || a.max_x == q.min_x || a.min_z == q.max_z || a.max_z == q.min_z)
                .collect()
        }
    }

    /// A cart's box (0.98 x 0.7) bottom-centred at (x, y, z).
    fn cart(x: f64, y: f64, z: f64) -> Aabb {
        Aabb::new(x - 0.49, y, z - 0.49, x + 0.49, y + 0.7, z + 0.49)
    }

    /// Positive: a backstop in the cart's path counts (the drop entry's (x,24,5): the cart moving north
    /// at 0.3 is clipped to 0.21, its box front on the glass's south face).
    #[test]
    fn a_backstop_in_the_path_counts() {
        let s = blocks(&[(3, 24, 5)]);
        let c = face_contacts(&s, cart(3.5, 24.0, 6.7), (0.0, 0.0, -0.3), (0.0, 0.0, -0.21));
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!((c[0].min_x, c[0].min_z), (3.0, 5.0));
    }

    /// Positive (ironspec 02:48 (b)): R58-m8's fence post. The x step is clipped at 8.625 while the
    /// box's z still overlaps the post (box max z -3.61 against the post's -3.625..-3.375); the z step
    /// then slides the box off. Judged at the x step it counts; the box after the whole move misses it.
    #[test]
    fn the_fence_post_clipped_before_the_z_step_counts() {
        let post = Aabb::new(8.375, 0.0, -3.625, 8.625, 1.5, -3.375);
        let s = move |q: Aabb| if post.intersects(q) || q.min_x == post.max_x { vec![post] } else { Vec::new() };
        let before = Aabb::new(8.7, 0.0, -4.59, 9.68, 0.7, -3.61);
        let c = face_contacts(&s, before, (-0.3, 0.0, -0.05), (-0.075, 0.0, -0.05));
        assert_eq!(c, vec![post]);
        let after = before.move_by(-0.075, 0.0, -0.05);
        assert!(after.max_z < post.min_z);
    }

    /// The fence post's mirror (G1's edge control 3): a |z| > |x| move steps z FIRST, clipped by a
    /// post the box still overlaps in x; the x step then slides it off. The z-first order catches it;
    /// an x-first replay would slide off first and miss it.
    #[test]
    fn z_first_order_catches_the_mirrored_post() {
        let post = Aabb::new(-3.625, 0.0, 8.375, -3.375, 1.5, 8.625);
        let s = move |q: Aabb| if post.intersects(q) || q.min_z == post.max_z { vec![post] } else { Vec::new() };
        let before = Aabb::new(-4.59, 0.0, 8.7, -3.61, 0.7, 9.68);
        let c = face_contacts(&s, before, (-0.05, 0.0, -0.3), (-0.05, 0.0, -0.075));
        assert_eq!(c, vec![post]);
        // (an x-first replay: x slides the box off the post (max x -3.66 < -3.625) before the z step)
        assert!(before.move_by(-0.05, 0.0, 0.0).max_x < post.min_x);
    }

    /// G1's edge control 1: on a clipped axis, a face exactly 0.011 away counts; 0.012 doesn't.
    #[test]
    fn the_face_gap_bound_is_0_011() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        // (the box's front stops 0.011 / 0.012 short of the wall's south face z 5.0)
        let at = |gap: f64| Aabb::new(3.01, 24.0, 5.0 + gap, 3.99, 24.7, 5.98 + gap);
        assert_eq!(face_contacts(&s, at(0.011), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)), vec![wall]);
        assert!(face_contacts(&s, at(0.012), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)).is_empty());
    }

    /// G1's edge control 2: the other-axis overlap must exceed 0.01: exactly 0.01 doesn't count,
    /// 0.0101 does.
    #[test]
    fn the_other_axis_overlap_bound_is_0_01() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        let at = |ov: f64| Aabb::new(4.0 - ov, 24.0, 5.0, 4.98 - ov, 24.7, 5.98);
        assert!(face_contacts(&s, at(0.01), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)).is_empty());
        assert_eq!(face_contacts(&s, at(0.0101), (0.0, 0.0, -0.2), (0.0, 0.0, 0.0)), vec![wall]);
    }

    /// G1's edge control 4: a contact that exists only after the y step. A cart dropping 0.5 while
    /// rolling north (its box 24.0..24.7 before, 23.5..24.2 after the y step) is clipped in z by a block
    /// at y 23..24: after the y step their vertical overlap is 0.5; judged on the box before the y step
    /// they would only touch (no vertical overlap) and it would be missed.
    #[test]
    fn a_contact_after_the_y_step_counts() {
        let low = Aabb::new(3.0, 23.0, 4.0, 4.0, 24.0, 5.0);
        let s = move |_q: Aabb| vec![low];
        let before = Aabb::new(3.01, 24.0, 5.0, 3.99, 24.7, 5.98);
        let c = face_contacts(&s, before, (0.0, -0.5, -0.2), (0.0, -0.5, 0.0));
        assert_eq!(c, vec![low]);
        // (the box before the y step meets the block's top face only: no vertical overlap)
        assert!(before.min_y >= low.max_y);
    }

    /// Negative: a centred cart rolling between walls 0.01 away (a 1-wide path): no axis clipped.
    #[test]
    fn a_cart_between_walls_counts_0() {
        let walls: Vec<(i32, i32, i32)> = (-3..=3).flat_map(|z| [(2, 24, z), (4, 24, z)]).collect();
        let s = blocks(&walls);
        assert!(face_contacts(&s, cart(3.5, 24.0, 1.0), (0.0, 0.0, -0.3), (0.0, 0.0, -0.3)).is_empty());
    }

    /// Negative (G1's control): a rail SNAP beside a wall it never touches. The snap is a `setPos`, not
    /// a move: the cart's recorded move clips nothing, so its box 0.005 from a wall's face, moving
    /// toward it, counts 0 (the old motion-toward-the-face rule would have counted it).
    #[test]
    fn a_snap_beside_a_wall_counts_0() {
        let wall = Aabb::new(3.0, 24.0, 4.0, 4.0, 25.0, 5.0);
        let s = move |_q: Aabb| vec![wall];
        let b = Aabb::new(3.01, 24.0, 5.005, 3.99, 24.7, 5.985);
        assert!(face_contacts(&s, b, (0.0, 0.0, -0.004), (0.0, 0.0, -0.004)).is_empty());
    }

    /// Negative: a pressed queue resting on slopes counts 0 supports (no axis clipped; the box passes
    /// through the next slope's support: deep overlap, no face).
    #[test]
    fn a_resting_slope_queue_counts_0_supports() {
        let s = blocks(&[(3, 18, 3), (3, 19, 4), (3, 17, 2)]);
        let b = cart(3.5, 18.6, 2.9);
        assert!(face_contacts(&s, b, (0.0, 0.0, -0.001), (0.0, 0.0, -0.001)).is_empty());
        assert!(face_contacts(&s, b, (0.0, 0.0, 0.001), (0.0, 0.0, 0.001)).is_empty());
    }

    /// Negative (G1, ironspec 02:52): a cart rolling over flat rails on their supports counts 0.
    #[test]
    fn a_cart_rolling_over_flat_rails_counts_0() {
        let supports: Vec<(i32, i32, i32)> = (0..=6).map(|z| (3, 23, z)).collect();
        let s = blocks(&supports);
        for y in [24.0, 24.0625] {
            assert!(face_contacts(&s, cart(3.5, y, 5.2), (0.0, 0.0, -0.4), (0.0, 0.0, -0.4)).is_empty());
            assert!(face_contacts(&s, cart(3.5, y, 2.9), (0.0, 0.0, 0.4), (0.0, 0.0, 0.4)).is_empty());
        }
    }

    /// Positive through redsim (G1's control): a resting cart pushed into a wall by its follower. Cart
    /// A rests on a flat north_south track against a stone wall at its north end; cart B rolls north
    /// into it. A's pushed motion is an attempted move the wall clips: A's recorded moves count the
    /// wall.
    #[test]
    fn a_resting_cart_pushed_into_a_wall_counts() {
        use redsim::block::Block;
        let mut w = World::new(BlockPos::new(-2, 0, -4), [6, 4, 16]);
        let stone = Block::parse("minecraft:stone").unwrap();
        let rail = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
        for z in -3..12 {
            w.set_block(BlockPos::new(0, 0, z), stone.clone(), redsim::world::flags::RAW);
        }
        for z in 0..12 {
            w.set_block(BlockPos::new(0, 1, z), rail.clone(), redsim::world::flags::RAW);
        }
        w.set_block(BlockPos::new(0, 1, -1), stone.clone(), redsim::world::flags::RAW);
        let a = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 0.5], Some("{Motion:[0.0d,0.0d,0.0d]}")).unwrap();
        let _b = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 3.5], Some("{Motion:[0.0d,0.0d,-0.4d]}")).unwrap();
        let mut hit = Vec::new();
        for _ in 0..40 {
            w.tick();
            let Some(i) = w.entities.list.iter().position(|e| e.id == a && !e.removed) else { break };
            hit.extend(cart_contacts(&w, i));
        }
        assert!(hit.iter().any(|(p, _)| *p == BlockPos::new(0, 1, -1)), "{hit:?}");
        // (ironspec 10:02 (b), the stale class through the same record: the wall, declared, is hit;
        // a declared cell no cart ever met reads stale)
        let count = |p: BlockPos| hit.iter().filter(|(q, _)| *q == p).count();
        assert_eq!(stale_declared(&[BlockPos::new(0, 1, -1), BlockPos::new(0, 1, 11)], &count), vec![BlockPos::new(0, 1, 11)]);
    }

    /// The strict `# contacts:` reader (G1's CHANGES 3 (3)): every entry of every line, cells joined
    /// by '+'; a malformed cell, a 2-number cell or an entry with no '=' is an error.
    #[test]
    fn declared_contacts_read_strictly() {
        let ok = "# contacts: -1,24,5+3,24,5+7,24,5=the drop entry's backstop; a REASON with \"; \" in it; 2,0,4=another\n";
        let d = declared_contacts_strict(ok).unwrap();
        assert_eq!(d.len(), 4);
        assert_eq!(d[1].0, BlockPos::new(3, 24, 5));
        assert_eq!(d[3], (BlockPos::new(2, 0, 4), "another".to_string()));
        assert!(declared_contacts_strict("# contacts: -1,24,x=bad\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24=short\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5=\n").is_err());
        assert!(declared_contacts_strict("# contacts: -1,24,5=   \n").is_err());
    }

    /// The bounded form's faces and depths (ironspec Nov 13 10:42 (3)): the backstop clip of
    /// `a_backstop_in_the_path_counts` is the block's SOUTH face (the cart moving north), depth 0.09.
    #[test]
    fn a_clip_names_the_blocks_face_and_its_depth() {
        let s = blocks(&[(3, 24, 5)]);
        let c = face_contacts_detail(&s, cart(3.5, 24.0, 6.7), (0.0, 0.0, -0.3), (0.0, 0.0, -0.21));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].1, Face::South);
        assert!((c[0].2 - 0.09).abs() < 1e-12, "{}", c[0].2);
        let w = blocks(&[(5, 24, 6)]);
        let c = face_contacts_detail(&w, cart(4.4, 24.0, 6.5), (0.2, 0.0, 0.0), (0.11, 0.0, 0.0));
        assert_eq!(c[0].1, Face::West);
    }

    fn hit(tick: i64, cart: u32, cell: (i32, i32, i32), face: Face, depth: f64, sin: f64, sout: f64) -> Hit {
        Hit { tick, cart, cell: BlockPos::new(cell.0, cell.1, cell.2), face, depth, speed_in: sin, speed_out: sout, sample: Some(tick as u64 / 100) }
    }

    const DESIGN: &str = "# contacts: 3,24,5=the drop entry's backstop | face=south | class=design | ref=5fa67f60a escape-bench --siding seeds 1..8 | ticks<=2+eps=1 | events<=1 why=one stop a visit | depth<=0.30+eps=0.05 | speed=0.30..0.45→0..0.05 why=the backstop stops the cart\n";

    /// The bounded reader: the design form parses every field; an incidental entry gets its implicit
    /// bounds; the legacy form stays Legacy; a REASON may hold "; ".
    #[test]
    fn the_bounded_form_parses() {
        let d = declared_contacts_bounded(DESIGN).unwrap();
        assert_eq!(d.len(), 1);
        let d = &d[0];
        assert_eq!((d.class, d.face), (Class::Design, Some(Face::South)));
        assert_eq!(d.ticks.as_ref().unwrap().limit(), 3.0);
        assert_eq!(d.events.as_ref().unwrap().limit(), 1.0);
        assert!((d.depth.as_ref().unwrap().limit() - 0.35).abs() < 1e-12);
        assert_eq!(d.speed_in.len(), 2);
        assert_eq!(d.speed_out.len(), 2);
        assert!(d.reference.as_deref().unwrap().starts_with("5fa67f60a"));
        let inc = declared_contacts_bounded("# contacts: 0,1,2=a glancing touch; harmless | class=incidental | speed_out>=0.1\n").unwrap();
        assert_eq!(inc[0].reason, "a glancing touch; harmless");
        assert_eq!((inc[0].depth.as_ref().unwrap().limit(), inc[0].ticks.as_ref().unwrap().limit()), (0.01, 1.0));
        let leg = declared_contacts_bounded("# contacts: 1,2,3+4,5,6=an old stop; with a semicolon\n").unwrap();
        assert_eq!((leg[0].class, leg[0].cells.len()), (Class::Legacy, 2));
        assert_eq!(leg[0].reason, "an old stop; with a semicolon");
    }

    /// The bounded reader's refusals: no class, an unknown field after the fields begin, a design
    /// entry missing its reference or a bound, a bound with no margin, an incidental loosening its
    /// bounds or stating no speed, a malformed value, a why= on its own.
    #[test]
    fn the_bounded_form_refuses() {
        for bad in [
            "# contacts: 3,24,5=r | face=south\n",
            "# contacts: 3,24,5=r | class=design | face=south | ticks<=1 why=w | events<=1 why=w | depth<=0.1 why=w | speed=0..1→0..1 why=w | ref=x | nonsense\n",
            "# contacts: 3,24,5=r | class=design | face=south | ticks<=1 why=w | events<=1 why=w | depth<=0.1 why=w | speed=0..1→0..1 why=w\n",
            "# contacts: 3,24,5=r | class=design | face=south | ticks<=1 why=w | depth<=0.1 why=w | speed=0..1→0..1 why=w | ref=x\n",
            "# contacts: 3,24,5=r | class=design | face=south | ticks<=1 | events<=1 why=w | depth<=0.1 why=w | speed=0..1→0..1 why=w | ref=x\n",
            "# contacts: 3,24,5=r | class=incidental | depth<=0.02 | speed_out>=0\n",
            "# contacts: 3,24,5=r | class=incidental | ticks<=2 | speed_out>=0\n",
            "# contacts: 3,24,5=r | class=incidental\n",
            "# contacts: 3,24,5=r | class=incidental | depth<=x | speed_out>=0\n",
            "# contacts: 3,24,5=r | class=incidental | speed_out>=0 | why=floating\n",
            "# contacts: 3,24,5=r | class=sideways | speed_out>=0\n",
            "# contacts: 3,24,5=r | class=incidental | face=up | speed_out>=0\n",
        ] {
            assert!(declared_contacts_bounded(bad).is_err(), "{bad}");
        }
    }

    fn moving(_: u32, _: i64) -> Option<(f64, f64)> {
        Some((0.1, 0.0))
    }

    /// The judge's bounds, each exceeded once and named: a clean design stop passes; ticks, depth,
    /// speed_in, speed_out and events each fail on their own.
    #[test]
    fn each_bound_fails_on_its_own() {
        let d = declared_contacts_bounded(DESIGN).unwrap();
        let ok = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0), hit(11, 1, (3, 24, 5), Face::South, 0.01, 0.01, 0.0)];
        let v = judge(&d, &ok, &moving, StallRule::IRONSPEC_1042, None);
        assert!(!v.failed(), "{v:?}");
        assert_eq!(v.events, vec![1]);
        let long: Vec<Hit> = (10..14).map(|t| hit(t, 1, (3, 24, 5), Face::South, 0.01, if t == 10 { 0.38 } else { 0.01 }, 0.0)).collect();
        assert!(judge(&d, &long, &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("ticks 4")));
        let deep = vec![hit(10, 1, (3, 24, 5), Face::South, 0.36, 0.38, 0.0)];
        assert!(judge(&d, &deep, &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("depth 0.3600")));
        let slow = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.20, 0.0)];
        assert!(judge(&d, &slow, &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("speed_in 0.2000")));
        let bounced = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.20)];
        assert!(judge(&d, &bounced, &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("speed_out 0.2000")));
        let twice = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0), hit(20, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0)];
        let v = judge(&d, &twice, &moving, StallRule::IRONSPEC_1042, None);
        assert!(v.exceeded.iter().any(|e| e.contains("2 events over events<=1")), "{v:?}");
        // (another cart's own event is its own count)
        let two_carts = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0), hit(20, 2, (3, 24, 5), Face::South, 0.2, 0.38, 0.0)];
        assert!(!judge(&d, &two_carts, &moving, StallRule::IRONSPEC_1042, None).failed());
    }

    /// A hit on a declared cell's OTHER face, or on another cell, is undeclared.
    #[test]
    fn a_hit_on_an_undeclared_face_is_undeclared() {
        // (ironspec 11:51 (1)(a): a '+'-joined declaration's cells are ONE stop: one contact meeting both
        // cells in the same ticks is one event, so events<=1 holds; the reference check agrees)
        let two = declared_contacts_bounded("# contacts: 3,24,5+4,24,5=the backstop's two cells | face=south | class=design | ref=x | ticks<=2 why=w | events<=1 why=w | depth<=0.20 why=w | speed=0.38..0.38→0.0..0.0 why=w\n").unwrap();
        let both = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0), hit(10, 1, (4, 24, 5), Face::South, 0.2, 0.38, 0.0), hit(11, 1, (3, 24, 5), Face::South, 0.01, 0.38, 0.0), hit(11, 1, (4, 24, 5), Face::South, 0.01, 0.38, 0.0)];
        let v = judge(&two, &both, &moving, StallRule::IRONSPEC_1042, None);
        assert!(!v.failed() && v.events == vec![1], "{v:?}");
        assert_eq!(reference_check(&two[0], &both), Vec::<String>::new());
        let d = declared_contacts_bounded(DESIGN).unwrap();
        let v = judge(&d, &[hit(10, 1, (3, 24, 5), Face::East, 0.2, 0.38, 0.0), hit(11, 1, (3, 23, 7), Face::North, 0.01, 0.05, 0.0)], &moving, StallRule::IRONSPEC_1042, None);
        assert_eq!(v.undeclared.len(), 2);
        assert_eq!(v.stale.len(), 1, "the declaration itself was never met");
    }

    /// NO STALL (ironspec 10:42 (3)): an event after which the cart neither moves over 0.003 b/t nor
    /// falls within 5 ticks fails, whatever its bounds; a cart moving again at the 5th tick passes; a
    /// fall passes; no motion record reads as not moving; a caller's wider window (the game's normal-pass
    /// envelope, G1 11:12 (a)) passes the 6th tick.
    #[test]
    fn an_event_followed_by_a_stall_fails() {
        let d = declared_contacts_bounded(DESIGN).unwrap();
        let h = vec![hit(10, 1, (3, 24, 5), Face::South, 0.2, 0.38, 0.0)];
        let r = StallRule::IRONSPEC_1042;
        let still = |_: u32, _: i64| Some((0.0, 0.0));
        assert_eq!(judge(&d, &h, &still, r, None).stalls.len(), 1);
        let late = |_: u32, t: i64| Some((if t == 16 { 0.1 } else { 0.0 }, 0.0));
        assert_eq!(judge(&d, &h, &late, r, None).stalls.len(), 1, "the 6th tick is too late");
        let fifth = |_: u32, t: i64| Some((if t == 15 { 0.1 } else { 0.0 }, 0.0));
        assert!(judge(&d, &h, &fifth, r, None).stalls.is_empty());
        let falls = |_: u32, t: i64| Some((0.0, if t == 12 { -0.04 } else { 0.0 }));
        assert!(judge(&d, &h, &falls, r, None).stalls.is_empty());
        let creeps = |_: u32, _: i64| Some((0.003, 0.0));
        assert_eq!(judge(&d, &h, &creeps, r, None).stalls.len(), 1, "0.003 is not over 0.003");
        let unknown = |_: u32, _: i64| None;
        assert_eq!(judge(&d, &h, &unknown, r, None).stalls.len(), 1);
        assert!(judge(&d, &h, &late, StallRule { window: 12, min_speed: 0.003 }, None).stalls.is_empty());
    }

    /// STALE (ironspec 10:47's ruled form, G1 11:12): without `rate=`, never met fails; with
    /// `rate=H/N UNIT` and the row's samples in that unit, 0 hits fails only when P(0) < 0.001, else
    /// "stale not judged"; some hits fail only when P(X <= hits) < 0.001; a unit mismatch or no samples
    /// is not judged. G1's case: 296/408 over 4 passes with 1 hit is not stale (P ≈ 0.066).
    #[test]
    fn stale_by_absence_and_by_rate() {
        let plain = declared_contacts_bounded("# contacts: 3,24,5=a stop | class=incidental | speed_out>=0\n").unwrap();
        let r = StallRule::IRONSPEC_1042;
        assert!(judge(&plain, &[], &moving, r, None).stale.iter().any(|s| s.contains("never met")));
        let d = declared_contacts_bounded("# contacts: 3,24,5=a stop | class=incidental | speed_out>=0 | rate=300/400 passes\n").unwrap();
        let one = vec![hit(10, 1, (3, 24, 5), Face::South, 0.005, 0.1, 0.0)];
        // (0 hits over 40 passes at 0.75: P(0) = 0.25^40: STALE)
        assert!(judge(&d, &[], &moving, r, Some((40, "passes"))).stale.iter().any(|s| s.contains("never met over 40 passes")));
        // (1 hit over 40: P(X <= 1) ~ 1e-22: an under-rate, STALE)
        assert!(judge(&d, &one, &moving, r, Some((40, "passes"))).stale.iter().any(|s| s.contains("under the reference")));
        // (0 hits over 2 passes: P(0) = 0.0625: not judged, not failing)
        let v = judge(&d, &[], &moving, r, Some((2, "passes")));
        assert!(v.stale.is_empty() && v.stale_not_judged.len() == 1 && !v.failed(), "{v:?}");
        // (a unit mismatch, or no samples: a wiring ERROR that fails loudly (G1 11:20), never "not judged")
        let mm = judge(&d, &[], &moving, r, Some((40, "visits")));
        assert!(mm.errors.len() == 1 && mm.errors[0].contains("visits") && mm.failed() && mm.stale_not_judged.is_empty(), "{mm:?}");
        let ns = judge(&d, &one, &moving, r, None);
        assert!(ns.errors.len() == 1 && ns.errors[0].contains("no samples") && ns.failed(), "{ns:?}");
        // (G1's case)
        let g = declared_contacts_bounded("# contacts: 3,24,5=the bed head | class=incidental | speed_out>=0 | rate=296/408 passes\n").unwrap();
        let v = judge(&g, &one, &moving, r, Some((4, "passes")));
        assert!(v.stale.is_empty(), "{v:?}");
        assert!((binomial_cdf(1, 4, 296.0 / 408.0) - 0.0664).abs() < 0.001, "{}", binomial_cdf(1, 4, 296.0 / 408.0));
        // (ironspec 11:51 (1)(b): the numerator counts samples with a hit, not ticks: a 3-tick contact in
        // one sample is 1 of 40, an under-rate; counting its ticks (3) would read the same here, so the
        // case that tells them apart: 3 ticks in one sample against 300/400 over 4 samples is n = 1, P(X
        // <= 1) = 0.051: not stale; as ticks it would have been 3)
        let three: Vec<Hit> = (10..13).map(|t| hit(t, 1, (3, 24, 5), Face::South, 0.005, 0.1, 0.0)).collect();
        let v = judge(&d, &three, &moving, r, Some((4, "passes")));
        assert!(v.stale.is_empty() && v.errors.is_empty(), "{v:?}");
        let v1 = judge(&d, &three, &moving, r, Some((40, "passes")));
        assert!(v1.stale.iter().any(|s| s.contains("1 hits over 40")), "{v1:?}");
        // (an untagged hit on a rate declaration is a wiring error)
        let mut untagged = one.clone();
        untagged[0].sample = None;
        assert!(judge(&d, &untagged, &moving, r, Some((40, "passes"))).errors.iter().any(|e| e.contains("no sample")));
        // (the rate's unit is required)
        assert!(declared_contacts_bounded("# contacts: 3,24,5=s | class=incidental | speed_out>=0 | rate=3/4\n").is_err());
    }

    /// The reference re-run check (ironspec 10:58, G1 11:12 (b)): judged on its own reference run, a
    /// design declaration's bounds equal the run's extremes to their written decimals; a drifted bound
    /// is named.
    #[test]
    fn a_design_declaration_reproduces_its_reference() {
        let refrun = vec![
            hit(10, 1, (3, 24, 5), Face::South, 0.2953, 0.3812, 0.0),
            hit(11, 1, (3, 24, 5), Face::South, 0.01, 0.01, 0.0),
            hit(40, 2, (3, 24, 5), Face::South, 0.25, 0.3011, 0.0),
        ];
        let ok = declared_contacts_bounded("# contacts: 3,24,5=the backstop | face=south | class=design | ref=x | ticks<=2+eps=1 | events<=1 why=w | depth<=0.2953+eps=0.05 | speed=0.3011..0.3812→0.0..0.0 why=w\n").unwrap();
        assert_eq!(reference_check(&ok[0], &refrun), Vec::<String>::new());
        let drift = declared_contacts_bounded("# contacts: 3,24,5=the backstop | face=south | class=design | ref=x | ticks<=2+eps=1 | events<=1 why=w | depth<=0.31+eps=0.05 | speed=0.3011..0.3812→0.0..0.0 why=w\n").unwrap();
        let m = reference_check(&drift[0], &refrun);
        assert!(m.len() == 1 && m[0].contains("depth<=0.31") && m[0].contains("0.295300"), "{m:?}");
        // (written with 2 decimals, 0.30 IS the reference's 0.2953 rounded: no drift)
        let rounded = declared_contacts_bounded("# contacts: 3,24,5=the backstop | face=south | class=design | ref=x | ticks<=2+eps=1 | events<=1 why=w | depth<=0.30+eps=0.05 | speed=0.3011..0.3812→0.0..0.0 why=w\n").unwrap();
        assert_eq!(reference_check(&rounded[0], &refrun), Vec::<String>::new());
        assert!(!reference_check(&ok[0], &[]).is_empty(), "a reference with no hit fails");
    }

    /// The incidental class's implicit bounds: a second tick or a depth over 0.01 fails.
    #[test]
    fn the_incidental_bounds_hold_without_being_stated() {
        let d = declared_contacts_bounded("# contacts: 3,24,5=a touch | class=incidental | speed_out>=0\n").unwrap();
        assert!(!judge(&d, &[hit(10, 1, (3, 24, 5), Face::South, 0.01, 0.1, 0.0)], &moving, StallRule::IRONSPEC_1042, None).failed());
        assert!(judge(&d, &[hit(10, 1, (3, 24, 5), Face::South, 0.011, 0.1, 0.0)], &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("depth")));
        assert!(judge(&d, &[hit(10, 1, (3, 24, 5), Face::South, 0.005, 0.1, 0.0), hit(11, 1, (3, 24, 5), Face::South, 0.005, 0.1, 0.0)], &moving, StallRule::IRONSPEC_1042, None).exceeded.iter().any(|e| e.contains("ticks 2")));
    }

    /// Through redsim: the pushed resting cart's wall contacts as Hits carry the wall's SOUTH face and
    /// a positive depth, and a legacy declaration of the wall judges them stall-free (the cart is
    /// pushed on, it doesn't stop for good: B keeps moving).
    #[test]
    fn cart_hits_through_redsim() {
        use redsim::block::Block;
        let mut w = World::new(BlockPos::new(-2, 0, -4), [6, 4, 16]);
        let stone = Block::parse("minecraft:stone").unwrap();
        let rail = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
        for z in -3..12 {
            w.set_block(BlockPos::new(0, 0, z), stone.clone(), redsim::world::flags::RAW);
        }
        for z in 0..12 {
            w.set_block(BlockPos::new(0, 1, z), rail.clone(), redsim::world::flags::RAW);
        }
        w.set_block(BlockPos::new(0, 1, -1), stone.clone(), redsim::world::flags::RAW);
        let a = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 0.5], Some("{Motion:[0.0d,0.0d,0.0d]}")).unwrap();
        let _b = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 3.5], Some("{Motion:[0.0d,0.0d,-0.4d]}")).unwrap();
        let mut hs = Vec::new();
        for _ in 0..40 {
            w.tick();
            let Some(i) = w.entities.list.iter().position(|e| e.id == a && !e.removed) else { break };
            hs.extend(cart_hits(&w, i));
        }
        assert!(!hs.is_empty());
        assert!(hs.iter().all(|h| h.cell == BlockPos::new(0, 1, -1) && h.face == Face::South && h.depth > 0.0), "{hs:?}");
    }

    /// The vertical axis (G1 09:56, ironspec 10:54 (a)): EMBEDDED deeper than 0.01 on every axis; a
    /// LOW CEILING 0.04 above flags, 0.06 doesn't, nor one overlapping only 0.01 across.
    #[test]
    fn embedded_and_low_ceiling_bounds() {
        let at = |x: f64, y: f64, z: f64| Aabb::new(x, y, z, x + 1.0, y + 1.0, z + 1.0);
        let c = cart(3.5, 24.0, 6.5);
        let one = |a: Aabb| move |_q: Aabb| vec![a];
        let v = vertical_contacts(&one(at(3.0, 24.4, 6.0)), c, &[], true);
        assert_eq!(v.iter().map(|x| x.0).collect::<Vec<_>>(), vec![VClass::Embedded], "{v:?}");
        // (ironspec 11:51 (2): on a rail the same overlap is normal slope riding: no Embedded)
        assert!(vertical_contacts(&one(at(3.0, 24.4, 6.0)), c, &[], false).is_empty(), "on-rail: no embedding");
        assert!(vertical_contacts(&one(at(3.0, 23.01, 6.0)), c, &[], true).is_empty(), "an overlap of exactly 0.01 in y is no embedding");
        let low = vertical_contacts(&one(at(3.0, 24.74, 6.0)), c, &[], false);
        assert!(low.len() == 1 && low[0].0 == VClass::LowCeiling && (low[0].2 - 0.04).abs() < 1e-9, "{low:?}");
        assert!(vertical_contacts(&one(at(3.0, 24.76, 6.0)), c, &[], false).is_empty(), "0.06 above doesn't flag");
        assert!(vertical_contacts(&one(at(3.98, 24.74, 6.0)), c, &[], false).is_empty(), "an overlap of only 0.01 across doesn't flag");
    }

    /// FLOOR and CEILING clips count only off the rail: a falling off-rail cart clipped on a block's top
    /// is a Floor (the lip rest on C's support); the same move on a rail is nothing; a rising off-rail
    /// cart clipped by a block's bottom is a Ceiling.
    #[test]
    fn off_rail_y_clips_are_classed() {
        let glass = Aabb::new(3.0, 23.0, 7.0, 4.0, 24.0, 8.0);
        let s = move |_q: Aabb| vec![glass];
        // (a cart over the drop cell, its box over the support's top, falling 0.04: clipped to 0)
        let before = cart(3.5, 24.0, 6.9);
        let m = [(before, (0.0, -0.04, 0.0), (0.0, 0.0, 0.0))];
        let f = vertical_contacts(&s, before, &m, true);
        assert!(f.len() == 1 && f[0].0 == VClass::Floor && (f[0].2 - 0.04).abs() < 1e-12, "{f:?}");
        assert!(vertical_contacts(&s, before, &m, false).is_empty(), "on a rail the support's clip is no contact");
        let roof = Aabb::new(3.0, 24.75, 6.0, 4.0, 25.75, 7.0);
        let r = move |_q: Aabb| vec![roof];
        let rising = cart(3.5, 24.0, 6.5);
        let up = vertical_contacts(&r, rising.move_by(0.0, 0.05, 0.0), &[(rising, (0.0, 0.2, 0.0), (0.0, 0.05, 0.0))], true);
        assert!(up.iter().any(|v| v.0 == VClass::Ceiling), "{up:?}");
    }

    /// Through redsim: a cart summoned inside a glass block is Embedded at its first tick; an off-rail
    /// cart dropped onto stone lands as a Floor; a cart rolling on rails has no vertical contact.
    #[test]
    fn vertical_hits_through_redsim() {
        use redsim::block::Block;
        let mut w = World::new(BlockPos::new(-4, 0, -4), [10, 6, 16]);
        let stone = Block::parse("minecraft:stone").unwrap();
        let glass = Block::parse("minecraft:glass").unwrap();
        let rail = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
        for z in -3..12 {
            w.set_block(BlockPos::new(0, 0, z), stone.clone(), redsim::world::flags::RAW);
            w.set_block(BlockPos::new(3, 0, z), stone.clone(), redsim::world::flags::RAW);
        }
        for z in 0..12 {
            w.set_block(BlockPos::new(0, 1, z), rail.clone(), redsim::world::flags::RAW);
        }
        w.set_block(BlockPos::new(-3, 1, 5), glass.clone(), redsim::world::flags::RAW);
        let rolling = redsim::entity::spawn(&mut w, "minecraft:minecart", [0.5, 1.0, 2.5], Some("{Motion:[0.0d,0.0d,0.3d]}")).unwrap();
        let falling = redsim::entity::spawn(&mut w, "minecraft:minecart", [3.5, 3.0, 5.5], Some("{Motion:[0.0d,0.0d,0.0d]}")).unwrap();
        let inside = redsim::entity::spawn(&mut w, "minecraft:minecart", [-2.5, 1.2, 5.5], Some("{Motion:[0.0d,0.0d,0.0d]}")).unwrap();
        let mut got: std::collections::BTreeMap<u32, Vec<VClass>> = std::collections::BTreeMap::new();
        for _ in 0..40 {
            w.tick();
            for (i, e) in w.entities.list.iter().enumerate() {
                if !e.removed {
                    got.entry(e.id).or_default().extend(cart_vertical_hits(&w, i).into_iter().map(|v| v.class));
                }
            }
        }
        assert!(got.get(&rolling).is_none_or(|v| v.is_empty()), "{got:?}");
        assert!(got.get(&falling).is_some_and(|v| v.contains(&VClass::Floor)), "{got:?}");
        assert!(got.get(&inside).is_some_and(|v| v.contains(&VClass::Embedded)), "{got:?}");
    }

    /// ironspec 11:51 (2)'s negative: a cart braked to rest on an R-like slope (an unpowered powered
    /// rail ascending_south on slime, its high side's support glass carrying F's rail) overlaps that
    /// glass by more than 0.01 on every axis in normal operation: on its rail, no Embedded hit, at any
    /// rest along the slope.
    #[test]
    fn a_cart_resting_on_a_slope_is_not_embedded() {
        use redsim::block::Block;
        for v in [0.10, 0.30, 0.44] {
            let mut w = World::new(BlockPos::new(-2, 0, -2), [8, 6, 14]);
            let stone = Block::parse("minecraft:stone").unwrap();
            let slime = Block::parse("minecraft:slime_block").unwrap();
            let glass = Block::parse("minecraft:glass").unwrap();
            let r = Block::parse("minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]").unwrap();
            let f = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
            for z in 0..9 {
                w.set_block(BlockPos::new(2, 0, z), stone.clone(), redsim::world::flags::RAW);
            }
            w.set_block(BlockPos::new(2, 0, 4), slime.clone(), redsim::world::flags::RAW);
            w.set_block(BlockPos::new(2, 1, 4), r.clone(), redsim::world::flags::RAW);
            for z in 5..8 {
                w.set_block(BlockPos::new(2, 1, z), glass.clone(), redsim::world::flags::RAW);
                w.set_block(BlockPos::new(2, 2, z), f.clone(), redsim::world::flags::RAW);
            }
            w.set_block(BlockPos::new(2, 1, 3), stone.clone(), redsim::world::flags::RAW);
            let c = redsim::entity::spawn(&mut w, "minecraft:minecart", [2.5, 2.0625, 6.5], Some(&format!("{{Motion:[0.0d,0.0d,{:.4}d]}}", -v))).unwrap();
            let mut embedded = 0;
            let mut rested = false;
            for _ in 0..120 {
                w.tick();
                let Some(i) = w.entities.list.iter().position(|e| e.id == c && !e.removed) else { break };
                embedded += cart_vertical_hits(&w, i).iter().filter(|h| h.class == VClass::Embedded).count();
                let e = &w.entities.list[i];
                rested |= e.pos.z < 5.0 && e.delta_movement.x.hypot(e.delta_movement.z) < 0.001;
            }
            assert!(rested, "the cart came to rest on the slope (v {v})");
            assert_eq!(embedded, 0, "v {v}: on its rail, no Embedded");
        }
    }
}
