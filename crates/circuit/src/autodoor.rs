//! Study 20's AUTOMATIC DOOR (clk-obs, coordinator 2026-10-21): `circuit: auto-door`, a 2x2 piston
//! door that opens when someone walks up to it from either side and closes once they have gone
//! through, never closing on someone standing in the doorway.
//!
//! Ports: `port door.a: x y z` and `port door.b: x y z`, two opposite corners of the 2x2 opening
//! (the bottom one and the top one, in one vertical plane). No input: people are the input.
//!
//! The door part is the 2x2 door's (designs/piston-door-2x2): at rest the 4 opening cells hold 4
//! identical full blocks (flush); open, the opening and the walkway (the opening's columns 3 blocks
//! in front and behind) hold nothing with collision. The judge lays its own floor under the walk
//! paths (the opening's two columns, 7 cells each way) where the design has no block, after the design
//! is placed.
//!
//! The walker is the game's armor stand (0.5 wide, 1.975 tall: narrower than a player's 0.6, so a
//! detector that sees it sees a player; it presses wooden and stone plates and tripwire), moved
//! every tick by a teleport (`tp`) along a lane through the opening, starting 6 blocks out on one
//! side and leaving 6 blocks out on the other: walking 0.21585 b/t (4.317 m/s), sprinting 0.2806
//! b/t (5.612 m/s). A walker whose next step would overlap a block with collision (the door not
//! open yet) waits in place, like a player pushing against it. FAIL: the door does not open within
//! [`OPEN_WAIT`] gt of the walker reaching it; a door cell holds a block, a moving block or a
//! piston head while a walker's box overlaps that cell (it closed on someone); a walker is moved by
//! anything but the judge (pushed); the door is not closed and flush within [`CLOSE_WAIT`] gt
//! after the last walker has left (beyond 6 blocks), or does not then stay closed for [`HOLD`] gt.
//!
//! The walks: both directions; lanes at the opening's middle (straddling both columns) and at each
//! column's middle; walking and sprinting; two walkers in a row (2 blocks apart); then the
//! two-walker sweeps ([`walks`]): one behind the other at every gap from touching to [`SPREAD`], and
//! from opposite sides crossing in the doorway at every start offset (the door must never stay open
//! with nobody inside); then the lateral sweeps (coordinator 2026-10-22): one walker at every offset
//! across the doorway in [`LATERAL_STEP`] steps, stand-width (0.5) and player-width (0.6: four
//! stands), walking and sprinting, both ways. Last, two REPORTED walks (designs/briefs/auto-door.md, 2026-10-22): a walker
//! that stops with its middle in the door plane for [`STOP`] gt and then goes on. They are not
//! required: a door that closes on someone standing in the doorway pushes them out of it (a player
//! who stops in a 1-deep doorway stands on no plate, and only a door with memory, about twice the
//! blocks, keeps it open); the metric `stop in doorway` says which this door does. Metrics: blocks,
//! open wait (the longest a walker waited at the door), close delay (the longest from the last
//! walker leaving the opening to the door closed), stop in doorway.

use crate::check::Outcome;
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::EntitySpec;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};

/// Walking and sprinting speed, blocks per tick.
pub const WALK: f64 = 0.21585;
pub const SPRINT: f64 = 0.2806;
/// The armor stand's half width and height.
const HALF: f64 = 0.25;
const TALL: f64 = 1.975;
/// A player's half width (0.6 wide): the lateral sweep's player-width walker is four armor stands 0.05
/// off its middle each way (their boxes' union is a 0.6 x 0.6 footprint), teleported together.
const PLAYER_HALF: f64 = 0.3;
/// The lateral sweep's step across the doorway (coordinator 2026-10-22).
pub const LATERAL_STEP: f64 = 1.0 / 32.0;
/// Where walkers start and leave: this far from the door plane, each way.
pub const REACH: f64 = 6.0;
/// The widest gap the two-walker sweeps put between walkers (they start up to this much farther out).
pub const SPREAD: f64 = 6.0;
/// The gap sweeps' step.
pub const SWEEP_STEP: f64 = 0.25;
/// The door must open within this many gt of a walker waiting at it.
pub const OPEN_WAIT: u32 = 100;
/// The door must be closed within this many gt of the last walker leaving.
pub const CLOSE_WAIT: u32 = 300;
/// Then it must stay closed this long.
pub const HOLD: u32 = 100;
/// A stopping walker stands in the door plane this long.
pub const STOP: u32 = 400;

/// The opening: its 4 cells, the plane's normal axis (0 = x, 2 = z) and coordinate, the width axis.
#[derive(Clone, Copy, Debug)]
pub struct Opening {
    pub lo: Pos,
    pub hi: Pos,
    /// The axis people walk along (the plane's normal).
    pub n: usize,
    /// The axis across the opening.
    pub u: usize,
}

impl Opening {
    pub fn cells(&self) -> Vec<Pos> {
        let mut v = Vec::new();
        for x in self.lo[0]..=self.hi[0] {
            for y in self.lo[1]..=self.hi[1] {
                for z in self.lo[2]..=self.hi[2] {
                    v.push([x, y, z]);
                }
            }
        }
        v
    }
    /// The walkway: the opening's columns `depth` cells in front and behind (the opening included).
    pub fn walkway(&self, depth: i32) -> Vec<Pos> {
        let mut v = Vec::new();
        for d in -depth..=depth {
            for p in self.cells() {
                let mut q = p;
                q[self.n] += d;
                v.push(q);
            }
        }
        v
    }
}

/// The opening from the two corner ports.
pub fn opening(c: &Circuit) -> Result<Opening> {
    let d = &c.clock.design;
    let (Some(a), Some(b)) = (d.ports.get("door.a").copied(), d.ports.get("door.b").copied()) else {
        bail!("auto-door: needs `port door.a: x y z` and `port door.b: x y z` (two opposite corners of the 2x2 opening)");
    };
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let (n, u) = match size {
        [2, 2, 1] => (2, 0),
        [1, 2, 2] => (0, 2),
        _ => bail!("auto-door: the corners {a:?} and {b:?} do not span a vertical 2x2 opening ({size:?})"),
    };
    Ok(Opening { lo, hi, n, u })
}

/// Blocks a player walks through (no collision).
pub fn walkable(state: &str) -> bool {
    let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    matches!(id, "air" | "cave_air" | "void_air" | "redstone_wire" | "tripwire" | "tripwire_hook" | "rail" | "powered_rail" | "detector_rail" | "activator_rail" | "redstone_torch" | "redstone_wall_torch" | "lever" | "light")
        || id.ends_with("_pressure_plate")
        || id.ends_with("_button")
        || id.ends_with("_carpet")
}

/// The static checks.
pub fn check_ports(c: &Circuit) -> Result<()> {
    let o = opening(c)?;
    if !c.inputs.is_empty() {
        bail!("auto-door: people are its input (no input port)");
    }
    for p in o.cells() {
        if !c.clock.design.blocks.get(&p).is_some_and(|s| !walkable(s)) {
            bail!("auto-door: at rest the door is closed: the opening cell {p:?} holds no block");
        }
    }
    let first = c.clock.design.blocks.get(&o.lo).cloned().unwrap_or_default();
    if o.cells().iter().any(|p| c.clock.design.blocks.get(p) != Some(&first)) {
        bail!("auto-door: the closed door is 4 identical blocks");
    }
    for p in o.walkway(3) {
        if o.cells().contains(&p) {
            continue;
        }
        if let Some(s) = c.clock.design.blocks.get(&p)
            && !walkable(s)
        {
            bail!("auto-door: the walkway (3 blocks in front of and behind the opening) holds {} at {p:?}", clock::short(s));
        }
    }
    Ok(())
}

/// One walker of a walk: the side it comes from (-1: the low side of the normal axis), its lane
/// (across the opening, 0.5 = the first column's middle, 1.0 = the opening's middle), its speed, how
/// much farther out than [`REACH`] it starts, and how long it stands with its middle in the door
/// plane (0: it does not stop).
#[derive(Clone, Debug)]
pub struct Walker {
    pub from: i32,
    pub lane: f64,
    pub v: f64,
    pub start: f64,
    pub stop: u32,
    /// A player-width walker (0.6 wide: four armor stands) instead of the 0.5-wide armor stand.
    pub player: bool,
}

impl Walker {
    fn half(&self) -> f64 {
        if self.player { PLAYER_HALF } else { HALF }
    }
}

/// A walk: its walkers, all moving at once, and its label; walks of one sweep share a family (the
/// table shows one line per family). A `reported` walk is not required: its outcome becomes a
/// metric instead of a fail (designs/briefs/auto-door.md: stopping in the doorway).
#[derive(Clone, Debug)]
pub struct Walk {
    pub walkers: Vec<Walker>,
    pub label: String,
    pub family: Option<String>,
    pub reported: bool,
}

fn one(from: i32, lane: f64, v: f64, stop: u32) -> Walker {
    Walker { from, lane, v, start: 0.0, stop, player: false }
}

/// The walks, in order: the 8 single and paired walks, then the two-walker sweeps (coordinator
/// 2026-10-21, the real-use bar): two walkers one behind the other, every gap from touching (0.5,
/// boxes side by side) to [`SPREAD`] in [`SWEEP_STEP`] steps, walking and sprinting, both ways;
/// two walkers from opposite sides in the two columns, crossing in the doorway, the second one's
/// start offset swept from -[`SPREAD`] to +[`SPREAD`] (0: they reach the door together), walking and
/// sprinting; last, the two reported stops in the doorway.
pub fn walks() -> Vec<Walk> {
    let w = |walkers: Vec<Walker>, label: &str| Walk { walkers, label: label.to_string(), family: None, reported: false };
    let pair = |from: i32, lane: f64, v: f64, gap: f64| vec![one(from, lane, v, 0), Walker { start: gap, ..one(from, lane, v, 0) }];
    let mut v = vec![
        w(vec![one(-1, 1.0, WALK, 0)], "walking through the middle"),
        w(vec![one(1, 1.0, WALK, 0)], "walking through the middle, the other way"),
        w(vec![one(-1, 0.5, SPRINT, 0)], "sprinting through the left column"),
        w(vec![one(1, 1.5, SPRINT, 0)], "sprinting through the right column, the other way"),
        w(pair(-1, 1.5, WALK, 2.0), "two walking in a row"),
        w(pair(1, 0.5, SPRINT, 2.0), "two sprinting in a row, the other way"),
        w(vec![one(-1, 0.5, WALK, 0)], "walking through the left column"),
        w(vec![one(1, 1.5, WALK, 0)], "walking through the right column, the other way"),
    ];
    let steps = (SPREAD / SWEEP_STEP).round() as i32;
    for (speed, sname) in [(WALK, "walking"), (SPRINT, "sprinting")] {
        for (from, side) in [(-1, ""), (1, ", the other way")] {
            let family = format!("two {sname} one behind the other{side}, every gap from 0.50 to {SPREAD:.2}");
            for k in 2..=steps {
                let gap = k as f64 * SWEEP_STEP;
                v.push(Walk { walkers: pair(from, 1.0, speed, gap), label: format!("two {sname} {gap:.2} apart{side}"), family: Some(family.clone()), reported: false });
            }
        }
        let family = format!("two {sname} from opposite sides, crossing in the doorway, every start offset from {:.2} to {SPREAD:.2}", -SPREAD);
        for k in -(steps / 2)..=(steps / 2) {
            let off = k as f64 * 2.0 * SWEEP_STEP;
            let (a, b) = (off.max(0.0), (-off).max(0.0));
            v.push(Walk {
                walkers: vec![Walker { start: a, ..one(-1, 0.5, speed, 0) }, Walker { start: b, ..one(1, 1.5, speed, 0) }],
                label: format!("two {sname} from opposite sides, offset {off:.2}"),
                family: Some(family.clone()),
                reported: false,
            });
        }
    }
    // the lateral sweeps (coordinator 2026-10-22): one walker at every offset across the doorway,
    // from touching its left edge to touching its right edge in LATERAL_STEP steps, with the stand's
    // 0.5-wide box and a player's 0.6-wide one, walking and sprinting, both ways: a door that opens
    // by halves shuts a half in the path of someone a little off centre
    for (player, wname) in [(false, "stand-width (0.5)"), (true, "player-width (0.6)")] {
        let half = if player { PLAYER_HALF } else { HALF };
        let mut lanes = Vec::new();
        let mut k = 0;
        loop {
            let lane = half + k as f64 * LATERAL_STEP;
            if lane > 2.0 - half + 1e-9 {
                break;
            }
            lanes.push(lane);
            k += 1;
        }
        if lanes.last().is_some_and(|l| (2.0 - half - l).abs() > 1e-9) {
            lanes.push(2.0 - half);
        }
        for (speed, sname) in [(WALK, "walking"), (SPRINT, "sprinting")] {
            for (from, side) in [(-1, ""), (1, ", the other way")] {
                let family = format!("one {sname} at every offset across the doorway, {wname}{side}, {} offsets", lanes.len());
                for &lane in &lanes {
                    v.push(Walk {
                        walkers: vec![Walker { player, ..one(from, lane, speed, 0) }],
                        label: format!("one {sname} at lane {lane:.4}, {wname}{side}"),
                        family: Some(family.clone()),
                        reported: false,
                    });
                }
            }
        }
    }
    // the reported walks, last (their outcome cannot change the others')
    let r = |walkers: Vec<Walker>, label: &str| Walk { walkers, label: label.to_string(), family: None, reported: true };
    v.push(r(vec![one(-1, 1.0, WALK, STOP)], "stopping in the doorway"));
    v.push(r(vec![one(1, 0.5, SPRINT, STOP)], "stopping in the doorway after a sprint, the other way"));
    v
}

/// What one run saw.
#[derive(Debug, Default)]
pub struct RunLog {
    pub lines: Vec<String>,
    pub fail: Option<String>,
    pub open_wait: u32,
    pub close_delay: u32,
    pub dims: [i32; 3],
    /// The reported stops: Some(true) when the door stayed open for every walker standing in the
    /// doorway, Some(false) when it closed on one.
    pub stop_safe: Option<bool>,
}

/// Run a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let o = match opening(c) {
        Ok(o) => o,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let (from, to) = run_box(c, &o, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: crate::shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, &o, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

/// The 4 opening cells hold 4 identical full blocks (the closed state).
fn closed(region: &mctas::sim::Region, o: &Opening, want: &str) -> bool {
    o.cells().iter().all(|p| region.get(*p).is_some_and(|s| s == want))
}

/// A walker's position: `c` along the normal axis (0 = the door plane's middle), `l` across.
fn world(o: &Opening, c: f64, lane: f64) -> [f64; 3] {
    let mut p = [0.0; 3];
    p[o.n] = o.lo[o.n] as f64 + 0.5 + c;
    p[o.u] = o.lo[o.u] as f64 + lane;
    p[1] = o.lo[1] as f64;
    p
}

/// The cells a walker's box at `p` (half width `half`) overlaps. A box exactly touching a cell's face
/// does not overlap it (the game's AABB test is strict); the epsilon on both sides keeps rounding at
/// some build positions from counting a touch (a walker at the doorway's very edge) as an overlap.
fn box_cells(p: [f64; 3], half: f64) -> Vec<Pos> {
    let lo = [(p[0] - half + 1e-6).floor() as i32, p[1].floor() as i32, (p[2] - half + 1e-6).floor() as i32];
    let hi = [(p[0] + half - 1e-6).floor() as i32, (p[1] + TALL - 1e-6).floor() as i32, (p[2] + half - 1e-6).floor() as i32];
    let mut v = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                v.push([x, y, z]);
            }
        }
    }
    v
}

thread_local! {
    /// `AUTODOOR_TRACE=1`: the design's box, whose changed cells each tick of a walk are printed.
    static TRACE_BOX: std::cell::RefCell<Option<(Pos, Pos)>> = const { std::cell::RefCell::new(None) };
}

/// The world the walks run in: the game's sandbox (a [`Worker`]'s) or redsim's ([`SimDoor`],
/// tickexact 2026-11-08: testgate's gap b, the walkers in redsim). The walks drive both through
/// these five calls only, so both run the same script.
trait DoorWorld {
    fn get_region(&mut self, sb: u32, from: Pos, to: Pos) -> Result<mctas::sim::Region>;
    fn step(&mut self, sb: u32, n: u32) -> Result<()>;
    fn summon(&mut self, sb: u32, e: &[EntitySpec]) -> Result<Vec<String>>;
    /// `tp <id> x y z` or `kill <id>`.
    fn exec(&mut self, sb: u32, cmd: &str) -> Result<()>;
    /// Every entity's id and position.
    fn entities(&mut self, sb: u32) -> Result<Vec<Stand>>;
}

/// An entity of a walk's world: its id (the game's uuid, redsim's id) and position.
struct Stand {
    uuid: String,
    pos: [f64; 3],
}

impl DoorWorld for Worker {
    fn get_region(&mut self, sb: u32, from: Pos, to: Pos) -> Result<mctas::sim::Region> {
        Worker::get_region(self, sb, from, to)
    }
    fn step(&mut self, sb: u32, n: u32) -> Result<()> {
        Worker::step(self, sb, n).map(|_| ())
    }
    fn summon(&mut self, sb: u32, e: &[EntitySpec]) -> Result<Vec<String>> {
        Worker::summon(self, sb, e)
    }
    fn exec(&mut self, sb: u32, cmd: &str) -> Result<()> {
        Worker::exec(self, sb, cmd)
    }
    fn entities(&mut self, sb: u32) -> Result<Vec<Stand>> {
        Ok(Worker::entities(self, sb)?.into_iter().map(|e| Stand { uuid: e.uuid, pos: e.pos }).collect())
    }
}

/// The walks in redsim: the design built as the game's sandbox builds it ([`crate::rig::sim_world_of`]),
/// armor stands summoned, teleported (`tp`: the position, no motion) and killed as the game's
/// commands do.
pub struct SimDoor {
    pub w: redsim::World,
}

impl DoorWorld for SimDoor {
    fn get_region(&mut self, _sb: u32, from: Pos, to: Pos) -> Result<mctas::sim::Region> {
        let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
        let mut palette: Vec<String> = Vec::new();
        let mut index: std::collections::HashMap<String, u16> = std::collections::HashMap::new();
        let mut data = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let s = self.w.get(redsim::BlockPos::new(from[0] + x, from[1] + y, from[2] + z)).to_string();
                    let k = match index.get(&s) {
                        Some(k) => *k,
                        None => {
                            palette.push(s.clone());
                            index.insert(s, (palette.len() - 1) as u16);
                            (palette.len() - 1) as u16
                        }
                    };
                    data.push(k);
                }
            }
        }
        Ok(mctas::sim::Region { from, size, palette, flags: Vec::new(), data })
    }
    fn step(&mut self, _sb: u32, n: u32) -> Result<()> {
        for _ in 0..n {
            self.w.tick();
        }
        if let Some(e) = self.w.errors.first() {
            bail!("redsim: {e}");
        }
        Ok(())
    }
    fn summon(&mut self, _sb: u32, es: &[EntitySpec]) -> Result<Vec<String>> {
        es.iter().map(|e| redsim::entity::spawn(&mut self.w, &e.kind, e.pos, e.nbt.as_deref()).map(|id| id.to_string()).map_err(|x| anyhow!("redsim: {x}"))).collect()
    }
    fn exec(&mut self, _sb: u32, cmd: &str) -> Result<()> {
        let f: Vec<&str> = cmd.split_whitespace().collect();
        let find = |w: &redsim::World, id: &str| -> Result<usize> {
            let id: u32 = id.parse()?;
            w.entities.list.iter().position(|e| e.id == id && !e.removed).ok_or_else(|| anyhow!("redsim: no entity {id}"))
        };
        match f.as_slice() {
            // `TeleportCommand` on an entity: `teleportSetPosition` (the position, the old position,
            // the motion zero: an absolute teleport)
            ["tp", id, x, y, z] => {
                let i = find(&self.w, id)?;
                let p = redsim::entity::Vec3::new(x.parse()?, y.parse()?, z.parse()?);
                self.w.entities.set_pos(i, p);
                let e = &mut self.w.entities.list[i];
                e.xo = p.x;
                e.yo = p.y;
                e.zo = p.z;
                e.delta_movement = redsim::entity::Vec3::new(0.0, 0.0, 0.0);
                Ok(())
            }
            ["kill", id] => {
                let i = find(&self.w, id)?;
                self.w.entities.list[i].removed = true;
                Ok(())
            }
            _ => bail!("redsim: the walks send no `{cmd}`"),
        }
    }
    fn entities(&mut self, _sb: u32) -> Result<Vec<Stand>> {
        Ok(self.w.entities.list.iter().filter(|e| !e.removed).map(|e| Stand { uuid: e.id.to_string(), pos: [e.pos.x, e.pos.y, e.pos.z] }).collect())
    }
}

/// The judge's floor under the walk paths (out to where the farthest walker starts): stone under
/// the opening's two columns, where the design has no block.
fn walk_floor(c: &Circuit, o: &Opening) -> Vec<(Pos, String)> {
    let mut floor = Vec::new();
    for d in -((REACH + SPREAD) as i32 + 1)..=((REACH + SPREAD) as i32 + 1) {
        // under the opening's two columns only (the walker's lanes stay over them): the cells beside
        // are the design's (its wiring)
        for k in o.lo[o.u]..=o.hi[o.u] {
            let mut q = [0; 3];
            q[o.n] = o.lo[o.n] + d;
            q[o.u] = k;
            q[1] = o.lo[1] - 1;
            if !c.clock.design.blocks.contains_key(&q) {
                floor.push((q, "minecraft:stone".to_string()));
            }
        }
    }
    floor
}

/// The box a run watches (the design's, out to where the farthest walker starts).
fn run_box(c: &Circuit, o: &Opening, margin: i32) -> (Pos, Pos) {
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    for i in 0..3 {
        from[i] = from[i].min(o.lo[i] - if i == o.n { (REACH + SPREAD) as i32 + 2 } else { 0 });
        to[i] = to[i].max(o.hi[i] + if i == o.n { (REACH + SPREAD) as i32 + 2 } else { 0 });
    }
    (from, to)
}

/// The run in redsim: [`game_run`]'s, the design built as the game's sandbox builds it. Outer Err:
/// what redsim does not model.
pub fn sim_run(c: &Circuit, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let o = match opening(c) {
        Ok(o) => o,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let (from, to) = run_box(c, &o, margin);
    let floor = walk_floor(c, &o);
    let w = match crate::rig::sim_world_of(&c.clock, from, to, &floor).map_err(|e| anyhow!("{e}"))? {
        Ok(w) => w,
        Err(why) => return Ok(Err(why)),
    };
    let mut sim = SimDoor { w };
    DoorWorld::step(&mut sim, 0, c.clock.free_from() + SETTLE)?;
    walks_in(&mut sim, 0, c, &o)
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, o: &Opening, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    if std::env::var("AUTODOOR_TRACE").is_ok() {
        let (lo, hi) = c.clock.design.bounds().unwrap_or(([0; 3], [0; 3]));
        TRACE_BOX.with(|t| *t.borrow_mut() = Some((lo, hi)));
    }
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    // the judge's floor under the walk paths (out to where the farthest walker starts)
    let floor = walk_floor(c, o);
    w.set_blocks(sb, &floor, PlaceMode::Raw)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    walks_in(w, sb, c, o)
}

/// The walks after the settle (the game's or redsim's world): the door closed at rest, then every
/// walk of [`walks`], the families' worst lines.
fn walks_in(w: &mut dyn DoorWorld, sb: u32, c: &Circuit, o: &Opening) -> Result<std::result::Result<RunLog, String>> {
    let door_state = c.clock.design.blocks.get(&o.lo).cloned().unwrap_or_default();
    // the corridor read every tick
    let mut cf = o.lo;
    let mut ct = o.hi;
    cf[o.n] -= (REACH + SPREAD) as i32 + 1;
    ct[o.n] += (REACH + SPREAD) as i32 + 1;
    cf[o.u] -= 1;
    ct[o.u] += 1;
    let mut log = RunLog { dims: crate::shulker::design_dims(c), ..Default::default() };
    let region = w.get_region(sb, cf, ct)?;
    if !closed(&region, o, &door_state) {
        return Ok(Ok(RunLog { fail: Some("not closed at rest after the settle".into()), ..log }));
    }
    // the families' running worst (walks, longest wait, longest close delay), shown as one line each
    let mut fams: Vec<(String, usize, u32, u32)> = Vec::new();
    for (k, wk) in walks().iter().enumerate() {
        let label = format!("walk {} ({})", k + 1, wk.label);
        match walk(w, sb, o, wk, &door_state, (cf, ct), &mut log)? {
            Ok((wait, delay)) if wk.reported => {
                log.stop_safe = Some(log.stop_safe.unwrap_or(true));
                log.lines.push(format!("{label}, reported: the door stayed open; closed {delay} gt after the walker left the opening (waited {wait} gt at the door)"));
            }
            Err(why) if wk.reported => {
                // not required: the door closed on the walker standing in the doorway. It must still
                // come to rest closed with nobody there before the next walk.
                log.stop_safe = Some(false);
                log.lines.push(format!("{label}, reported: {why}"));
                let mut rest = false;
                for _ in 0..CLOSE_WAIT {
                    if closed(&w.get_region(sb, cf, ct)?, o, &door_state) {
                        rest = true;
                        break;
                    }
                    w.step(sb, 1)?;
                }
                if !rest {
                    log.fail = Some(format!("{label}: after it closed on the walker, the door was not closed {CLOSE_WAIT} gt later"));
                    break;
                }
                w.step(sb, HOLD)?;
            }
            Ok((wait, delay)) => match &wk.family {
                None => log.lines.push(format!(
                    "{label}: waited {wait} gt at the door{}, closed {delay} gt after the last walker left the opening",
                    wk.walkers.iter().map(|x| x.stop).max().filter(|s| *s > 0).map(|s| format!(", stood {s} gt in the doorway")).unwrap_or_default()
                )),
                Some(f) => match fams.iter_mut().find(|e| &e.0 == f) {
                    Some(e) => {
                        e.1 += 1;
                        e.2 = e.2.max(wait);
                        e.3 = e.3.max(delay);
                    }
                    None => fams.push((f.clone(), 1, wait, delay)),
                },
            },
            Err(why) => {
                log.fail = Some(format!("{label}: {why}"));
                break;
            }
        }
    }
    for (f, n, wait, delay) in fams {
        log.lines.push(format!("{f} ({n} walks): waited at most {wait} gt at the door, closed at most {delay} gt after the last walker left the opening"));
    }
    Ok(Ok(log))
}

fn walk(w: &mut dyn DoorWorld, sb: u32, o: &Opening, wk: &Walk, door: &str, (cf, ct): (Pos, Pos), log: &mut RunLog) -> Result<std::result::Result<(u32, u32), String>> {
    let n = wk.walkers.len();
    let dirs: Vec<f64> = wk.walkers.iter().map(|x| -x.from as f64).collect();
    let lanes: Vec<f64> = wk.walkers.iter().map(|x| x.lane).collect();
    let halves: Vec<f64> = wk.walkers.iter().map(|x| x.half()).collect();
    // the walkers' positions along the normal axis (0 = the door plane's middle)
    let mut cs: Vec<f64> = wk.walkers.iter().map(|x| x.from as f64 * (REACH + x.start)).collect();
    // who follows whom: the nearest walker ahead going the same way with an overlapping lane; a
    // follower never closes in on it (it keeps at least the gap it started with)
    let follow: Vec<Option<(usize, f64)>> = (0..n)
        .map(|j| {
            (0..n)
                .filter(|&i| i != j && wk.walkers[i].from == wk.walkers[j].from && (lanes[i] - lanes[j]).abs() < halves[i] + halves[j] && (cs[i] - cs[j]) * dirs[j] > 0.0)
                .map(|i| (i, (cs[i] - cs[j]) * dirs[j]))
                .min_by(|a, b| a.1.total_cmp(&b.1))
        })
        .collect();
    let mut uuids = Vec::new();
    // each walker's armor stands, as (along, across) offsets from its middle: a stand-width walker is
    // one stand; a player-width one (0.6 x 0.6) is four stands 0.05 off its middle each way, whose
    // boxes' union is exactly a player's footprint, so a plate or a string sees what a player's box
    // would (the bridge allows no mob: a villager is refused)
    let parts: Vec<Vec<(f64, f64)>> = wk.walkers.iter().map(|x| if x.player { vec![(-0.05, -0.05), (-0.05, 0.05), (0.05, -0.05), (0.05, 0.05)] } else { vec![(0.0, 0.0)] }).collect();
    for i in 0..n {
        let mut ids = Vec::new();
        for (dc, dl) in &parts[i] {
            let p = world(o, cs[i] + dc, lanes[i] + dl);
            ids.push(w.summon(sb, &[EntitySpec { kind: "minecraft:armor_stand".into(), pos: p, nbt: None }])?.into_iter().next().ok_or_else(|| anyhow!("summon: no uuid"))?);
        }
        uuids.push(ids);
    }
    let mut done = vec![false; n];
    let mut waited = vec![0u32; n];
    let mut stopped = vec![0u32; n];
    let mut left_opening: Option<u32> = None;
    let mut t = 0u32;
    let fmt = |p: [f64; 3]| format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]);
    let opening_cells = o.cells();
    let mut trace_prev: Option<mctas::sim::Region> = None;
    // front walkers first, so a follower sees where its leader went this tick
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| (cs[b] * dirs[b]).total_cmp(&(cs[a] * dirs[a])));
    let result: std::result::Result<(), String> = 'walk: loop {
        let region = w.get_region(sb, cf, ct)?;
        for &i in &order {
            if done[i] {
                continue;
            }
            let dir = dirs[i];
            if cs[i] * dir >= 0.0 && stopped[i] < wk.walkers[i].stop {
                stopped[i] += 1;
                continue;
            }
            let next = cs[i] + dir * wk.walkers[i].v;
            let np = world(o, next, lanes[i]);
            let blocked = box_cells(np, halves[i]).iter().any(|q| region.get(*q).is_some_and(|s| !walkable(s)));
            let behind_front = follow[i].is_some_and(|(f, gap)| !done[f] && (cs[f] - next) * dir < gap - 1e-9);
            if blocked || behind_front {
                waited[i] += 1;
                if waited[i] > OPEN_WAIT {
                    break 'walk Err(format!("walker {} waited {OPEN_WAIT} gt at {} and the door did not open", i + 1, fmt(world(o, cs[i], lanes[i]))));
                }
                continue;
            }
            cs[i] = next;
            if cs[i] * dir > REACH {
                done[i] = true;
                for u in &uuids[i] {
                    w.exec(sb, &format!("kill {u}"))?;
                }
            } else {
                for (k, (dc, dl)) in parts[i].iter().enumerate() {
                    let p = world(o, cs[i] + dc, lanes[i] + dl);
                    w.exec(sb, &format!("tp {} {:.5} {:.5} {:.5}", uuids[i][k], p[0], p[1], p[2]))?;
                }
            }
        }
        if done.iter().all(|d| *d) {
            break Ok(());
        }
        w.step(sb, 1)?;
        t += 1;
        if let Some((lo, hi)) = TRACE_BOX.with(|b| *b.borrow()) {
            let now = w.get_region(sb, lo, hi)?;
            let mut changes = Vec::new();
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        let (a, b) = (trace_prev.as_ref().and_then(|r: &mctas::sim::Region| r.get([x, y, z]).map(|s| s.to_string())), now.get([x, y, z]).map(|s| s.to_string()));
                        if trace_prev.is_some() && a != b {
                            changes.push(format!("{:?} {}", [x, y, z], clock::short(b.as_deref().unwrap_or("air"))));
                        }
                    }
                }
            }
            if !changes.is_empty() {
                eprintln!("t={t} walkers {:?}: {}", cs.iter().map(|c| format!("{c:.2}")).collect::<Vec<_>>(), changes.join(" | "));
            }
            trace_prev = Some(now);
        }
        // the checks after the tick: nobody pushed, the door never closing on anyone
        let region = w.get_region(sb, cf, ct)?;
        let ents = w.entities(sb)?;
        for i in 0..n {
            if done[i] {
                continue;
            }
            let want = world(o, cs[i], lanes[i]);
            for (k, (dc, dl)) in parts[i].iter().enumerate() {
                let at = world(o, cs[i] + dc, lanes[i] + dl);
                let Some(e) = ents.iter().find(|e| e.uuid == uuids[i][k]) else { break 'walk Err(format!("walker {} is gone", i + 1)) };
                if (0..3).any(|j| (e.pos[j] - at[j]).abs() > 0.02) {
                    break 'walk Err(format!("walker {} was moved from {} to {} (pushed)", i + 1, fmt(at), fmt(e.pos)));
                }
            }
            for q in box_cells(want, halves[i]) {
                if opening_cells.contains(&q)
                    && let Some(s) = region.get(q).filter(|s| !walkable(s))
                {
                    break 'walk Err(format!("the door closed on walker {} at {}: {} in {q:?}", i + 1, fmt(want), clock::short(s)));
                }
            }
        }
        // the last walker out of the opening
        let in_opening = (0..n).any(|i| !done[i] && box_cells(world(o, cs[i], lanes[i]), halves[i]).iter().any(|q| opening_cells.contains(q)));
        let passed = (0..n).all(|i| done[i] || cs[i] * dirs[i] > 0.5 + halves[i]);
        if passed && !in_opening && left_opening.is_none() {
            left_opening = Some(t);
        }
        if t > 4000 {
            break Err("the walk took over 4000 gt".into());
        }
    };
    // a reported walk the door failed (it closed on a walker standing in the doorway): what the
    // closing does to them, the walker left alone (no more teleports) for 40 gt
    let mut aftermath = String::new();
    if wk.reported && result.is_err() {
        w.step(sb, 40)?;
        let ents = w.entities(sb)?;
        let region = w.get_region(sb, cf, ct)?;
        for i in 0..n {
            if done[i] {
                continue;
            }
            if let Some(e) = ents.iter().find(|e| e.uuid == uuids[i][0]) {
                let inside = box_cells(e.pos, halves[i]).iter().any(|q| region.get(*q).is_some_and(|s| !walkable(s)));
                let from_plane = e.pos[o.n] - (o.lo[o.n] as f64 + 0.5);
                aftermath.push_str(&format!(
                    "; left alone 40 gt, walker {} stands at {} ({from_plane:+.2} from the door plane), {}",
                    i + 1,
                    fmt(e.pos),
                    if inside { "inside the door's blocks" } else { "pushed clear of every block" }
                ));
            }
        }
    }
    for (i, ids) in uuids.iter().enumerate() {
        if !done[i] {
            for u in ids {
                w.exec(sb, &format!("kill {u}"))?;
            }
        }
    }
    if let Err(why) = result {
        return Ok(Err(format!("{why}{aftermath}")));
    }
    // the door closes, then stays closed
    let t_left = left_opening.unwrap_or(t);
    let mut closed_at = None;
    for k in 0..CLOSE_WAIT {
        let region = w.get_region(sb, cf, ct)?;
        if closed(&region, o, door) {
            closed_at = Some(t + k);
            break;
        }
        w.step(sb, 1)?;
    }
    let Some(tc) = closed_at else { return Ok(Err(format!("the door was not closed {CLOSE_WAIT} gt after the last walker left (it stays open with nobody inside)"))) };
    let delay = tc.saturating_sub(t_left);
    for k in 0..HOLD {
        w.step(sb, 1)?;
        let region = w.get_region(sb, cf, ct)?;
        if !closed(&region, o, door) {
            return Ok(Err(format!("closed, then opened again {} gt later with nobody there", k + 1)));
        }
    }
    let wmax = waited.iter().copied().max().unwrap_or(0);
    log.open_wait = log.open_wait.max(wmax);
    log.close_delay = log.close_delay.max(delay);
    Ok(Ok((wmax, delay)))
}

/// The run's verdict.
pub fn judge(_c: &Circuit, log: &RunLog) -> Outcome {
    let table = log.lines.clone();
    if let Some(f) = &log.fail {
        return Outcome { pass: false, why: f.clone(), metrics: Vec::new(), table };
    }
    let metrics = vec![
        ("door".to_string(), "2x2".to_string()),
        ("open wait".to_string(), format!("{} gt", log.open_wait)),
        ("close delay".to_string(), format!("{} gt", log.close_delay)),
        ("stop in doorway".to_string(), stop_metric(log.stop_safe).to_string()),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The reported stops' metric value.
fn stop_metric(safe: Option<bool>) -> &'static str {
    match safe {
        Some(true) => "the door stays open",
        Some(false) => "the door closes on them",
        None => "not run",
    }
}

/// The judge (`mctas circuit verify`): the run at N positions x 4 rotations; the worst metrics.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&rc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    Ok(report(c, kind, builds))
}

/// The judge's redsim twin (`circuit sim`, `circuit door-run --builds`): every build of
/// [`verify_in_pool`], the walks in redsim ([`sim_run`]). Err: what redsim does not model.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    let kind = c.kind.clone().ok_or_else(|| "no kind".to_string())?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut builds = Vec::new();
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            let rc = tc.translated(*off).map_err(|e| format!("{e:#}"))?;
            let r = sim_run(&rc, o.margin).map_err(|why| format!("rotation {t} at {off:?}: {why:#}"))?;
            builds.push(Build { turns: *t, offset: *off, result: r.map(|log| BuildOk { outcome: judge(&rc, &log), dims: log.dims, family: family.clone() }) });
        }
    }
    Ok(report(c, kind, builds))
}

/// The verdict over the builds, the base build's card carrying the worst metrics (the longest open
/// wait and close delay; a door that closes on a stopped walker in any build closes on them).
fn report(c: &Circuit, kind: crate::Kind, builds: Vec<Build>) -> VerifyReport {
    let mut schedule = Schedule::new(0);
    schedule.ticks = 0;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return rep;
    }
    let num = |k: &BuildOk, name: &str| k.outcome.metrics.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
    let oks: Vec<&BuildOk> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|k| k.outcome.pass).collect();
    let wait = oks.iter().filter_map(|k| num(k, "open wait")).fold(0.0f64, f64::max);
    let close = oks.iter().filter_map(|k| num(k, "close delay")).fold(0.0f64, f64::max);
    // the stops: the worst build's (one build closing on a walker is the door's answer)
    let closes = stop_metric(Some(false));
    let stop_worst = if oks.iter().any(|k| k.outcome.metrics.iter().any(|(n, v)| n == "stop in doorway" && v == closes)) { closes } else { stop_metric(Some(true)) };
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        for (n, v) in k.outcome.metrics.iter_mut() {
            match n.as_str() {
                "open wait" => *v = format!("{} gt", wait as u32),
                "close delay" => *v = format!("{} gt", close as u32),
                "stop in doorway" => *v = stop_worst.to_string(),
                _ => {}
            }
        }
    }
    rep
}

/// A design aid (`mctas circuit door-run`): one run turned and moved like one of `circuit verify`'s builds.
pub fn run_one(pool: &Pool, c: &Circuit, turns: u8, offset: Pos, margin: i32) -> Result<Outcome> {
    let r = pool.map(vec![0u8], |w, _, _| {
        let rc = c.rotated(w, turns)?.translated(offset)?;
        Ok((game_run(w, &rc, margin)?, rc))
    });
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        (Ok(log), rc) => Ok(judge(&rc, &log)),
        (Err(why), _) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes() {
        // a walker centred in a cell overlaps that column only, 2 cells tall
        let cells = box_cells([0.5, 0.0, 0.5], HALF);
        assert_eq!(cells, vec![[0, 0, 0], [0, 1, 0]]);
        // straddling two columns
        assert_eq!(box_cells([1.0, 0.0, 0.5], HALF).len(), 4);
        // the lateral sweep: from touching the opening's left edge to touching its right edge
        let lanes = walks().iter().filter(|w| w.family.as_deref().is_some_and(|f| f.contains("offset across") && f.contains("player-width") && f.starts_with("one walking") && !f.contains("other way"))).map(|w| w.walkers[0].lane).collect::<Vec<_>>();
        assert_eq!(lanes.first().copied(), Some(0.3));
        assert!((lanes.last().unwrap() - 1.7).abs() < 1e-9);
        assert_eq!(lanes.len(), 46);
        assert!(walkable("minecraft:oak_pressure_plate[powered=false]"));
        assert!(walkable("minecraft:tripwire[attached=true]"));
        assert!(!walkable("minecraft:stone"));
        assert!(!walkable("minecraft:moving_piston[facing=east,type=normal]"));
    }
}
