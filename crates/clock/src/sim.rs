//! `mctas clock sim`: the exact redsim period detector. Place the clock like the judge, kick it,
//! run, and find the first EXACT repeat of the whole state ([`redsim::World::state_key`]: every
//! cell, pending ticks, block events, moving blocks, block entities, torch memory): the warmup W is
//! the tick of the first occurrence and the period P the distance to the repeat. Then one period is
//! replayed to measure the waveform and the family ([`crate::wave`]).

use crate::wave::{self, CellWave, PeriodAnalysis};
use crate::{Action, Clock, Kick, Placement, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::hash::Hasher;

/// Knobs of [`simulate`].
#[derive(Clone, Debug)]
pub struct SimOptions {
    /// Ticks to run at most while looking for a repeat.
    pub horizon: u32,
    /// Free cells around the design's box (the simulated world; a write beyond it = escapes).
    pub margin: i32,
}

impl Default for SimOptions {
    fn default() -> Self {
        SimOptions { horizon: 50_000, margin: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Periodic with at least one block changing in every period.
    Clock,
    /// The state stops changing (or only hidden state changes).
    Stops,
    /// No repeat within the horizon.
    NoRepeat,
    /// A block or a case redsim does not model exactly.
    Unmodeled,
    /// Not a clock by the rules: a drop (item entity), a write beyond the margin, a bad kick.
    Invalid,
    /// Independent sub-clocks side by side: the output's own period is shorter than the whole
    /// structure's (coordinator's ruling: rejected).
    Composite,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Clock => "CLOCK",
            Verdict::Stops => "STOPS",
            Verdict::NoRepeat => "NO-REPEAT",
            Verdict::Unmodeled => "UNMODELED",
            Verdict::Invalid => "INVALID",
            Verdict::Composite => "COMPOSITE",
        }
    }
}

/// The output cell's waveform.
#[derive(Clone, Debug)]
pub struct Output {
    pub pos: Pos,
    /// Not declared (`port output`): the changing cell with the most rising edges.
    pub auto: bool,
    pub wave: CellWave,
}

/// Result of [`simulate`].
#[derive(Clone, Debug)]
pub struct SimReport {
    pub name: String,
    pub verdict: Verdict,
    /// Why it is not a clock (empty for a clock).
    pub why: String,
    /// The first tick from which the VISIBLE state repeats (block states + comparator output
    /// strengths: what the real-game judge sees).
    pub warmup: u32,
    /// The first tick from which the WHOLE state repeats exactly (hidden state included: pending
    /// ticks, torch memory, container contents): >= `warmup`.
    pub state_warmup: u32,
    /// The clock's period: the visible state repeats every `period` gt (a divisor of
    /// `state_period`).
    pub period: u32,
    /// The exact period of the whole state (larger than `period` only when hidden state cycles
    /// slower than everything visible, e.g. items sloshing in hoppers no comparator reads).
    pub state_period: u32,
    pub analysis: PeriodAnalysis,
    pub output: Option<Output>,
    pub blocks: usize,
    /// Every cell non-air at any tick of the run (warmup and one period) and the design's box.
    pub swept: (Pos, Pos),
    pub start: String,
    /// The cells an entity occupied during the period (never usable as the output).
    pub occupied: Vec<Pos>,
    /// A clock holding a sculk sensor: whether ambient game events leave its output alone
    /// ([`crate::vibration::sim_property`]; a property, never a failure).
    pub vibration: Option<crate::vibration::Property>,
}

impl SimReport {
    fn fail(c: &Clock, verdict: Verdict, why: String) -> SimReport {
        SimReport {
            name: c.name().to_string(),
            verdict,
            why,
            warmup: 0,
            state_warmup: 0,
            period: 0,
            state_period: 0,
            analysis: PeriodAnalysis::default(),
            output: None,
            blocks: c.blocks(),
            swept: c.bounds(),
            start: c.start_label(),
            occupied: Vec::new(),
            vibration: None,
        }
    }

    /// Width x height x depth of the swept box.
    pub fn dims(&self) -> [i32; 3] {
        let (lo, hi) = self.swept;
        [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
    }

    pub fn volume(&self) -> i32 {
        let d = self.dims();
        d[0] * d[1] * d[2]
    }

    /// The one-line summary (the judge's format, docs/clocks.md).
    pub fn summary(&self) -> String {
        let [w, h, d] = self.dims();
        match self.verdict {
            Verdict::Clock => {
                let (on, edges) = self.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
                format!(
                    "CLOCK: {} period {}, on {on}/{}, edges {edges}, warmup {}, family {}, {} blocks, {w}x{h}x{d}, {}",
                    self.name,
                    crate::gt_time(self.period),
                    self.period,
                    self.warmup,
                    wave::signature(&self.analysis.family),
                    self.blocks,
                    self.start
                )
            }
            v => format!("{}: {} {}", v.word(), self.name, self.why),
        }
    }
}

/// A fast 128-bit digest of a state key (two independent multiplicative lanes); every repeat it
/// finds is confirmed on the exact key.
struct H128 {
    a: u64,
    b: u64,
}

/// Cell `k`'s share of a frame digest ([`simulate`]: the frame's digest is the sum of its cells'
/// shares): its state (block and block-entity extras) hashed with the cell's index, each half
/// mixed through splitmix64's finalizer, so that shares summed look random.
fn cell_share(k: usize, b: &Block, extra: &Option<String>) -> u128 {
    use std::hash::Hash;
    let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
    h.write_u64(k as u64);
    b.hash(&mut h);
    extra.hash(&mut h);
    let mix = |mut z: u64| {
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    };
    ((mix(h.a) as u128) << 64) | mix(h.b ^ 0x9e3779b97f4a7c15) as u128
}

impl Hasher for H128 {
    fn finish(&self) -> u64 {
        self.a
    }
    fn write(&mut self, bytes: &[u8]) {
        for &x in bytes {
            self.a = (self.a ^ x as u64).wrapping_mul(0x100000001b3);
            self.b = (self.b.rotate_left(7) ^ x as u64).wrapping_mul(0x9e3779b97f4a7c15);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.a = (self.a ^ x).wrapping_mul(0x100000001b3).rotate_left(29);
        self.b = (self.b ^ x.rotate_left(17)).wrapping_mul(0x9e3779b97f4a7c15).rotate_left(31);
    }
    fn write_u8(&mut self, x: u8) {
        self.write_u64(x as u64 | 0x100);
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64 | 0x2_0000_0000);
    }
    fn write_i32(&mut self, x: i32) {
        self.write_u32(x as u32)
    }
    fn write_i64(&mut self, x: i64) {
        self.write_u64(x as u64 ^ 0x5555_0000_0000_0000)
    }
    fn write_isize(&mut self, x: isize) {
        self.write_u64(x as u64 ^ 0x3333_0000_0000_0000)
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64 ^ 0x7777_0000_0000_0000)
    }
}

fn digest(w: &World) -> u128 {
    let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
    w.state_key_into(&mut h);
    ((h.a as u128) << 64) | h.b as u128
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A placed world ready to run, and the kick's inputs still to apply (tick, action), in order.
struct Started {
    world: World,
    actions: Vec<(u32, Action)>,
}

/// Every block of the clock as a redsim state, or the names redsim does not model.
/// The block the judge puts in a clock's output cell (vjeux 2026-10-20: the judge reads a lamp
/// there, as a builder would attach one).
pub const JUDGE_LAMP: &str = "minecraft:redstone_lamp[lit=false]";

/// The cell of the judge's lamp: the declared output when the design leaves it empty (an air
/// output cell). Not a block of the design: never counted, never in the family or the box. Clocks
/// only: a circuit (a `circuit:` kind, or an input port) keeps its own consumer rule
/// (`circuit::consumer_cells`).
pub fn judge_lamp(c: &Clock) -> Option<Pos> {
    if c.design.meta.contains_key("circuit") || c.design.ports.contains_key("input") || c.design.ports.keys().any(|k| k.starts_with("input")) {
        return None;
    }
    c.output.filter(|op| !c.design.blocks.contains_key(op))
}

pub fn parse_blocks(c: &Clock) -> Result<Vec<(BlockPos, Block)>, String> {
    let mut out = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for (p, s) in &c.design.blocks {
        match Block::parse(s) {
            Ok(b) => out.push((bp(*p), b)),
            Err(e) => {
                if !bad.contains(&e) {
                    bad.push(e)
                }
            }
        }
    }
    if let Some(op) = judge_lamp(c)
        && let Ok(b) = Block::parse(JUDGE_LAMP)
    {
        out.push((bp(op), b));
    }
    if let Some(Kick::Place { state, .. }) = &c.kick
        && let Err(e) = Block::parse(state)
    {
        bad.push(e);
    }
    if bad.is_empty() { Ok(out) } else { Err(bad.join("; ")) }
}

fn start(c: &Clock, blocks: &[(BlockPos, Block)], margin: i32) -> Result<Started, (Verdict, String)> {
    let (lo, hi) = c.bounds();
    start_box(c, blocks, [lo[0] - margin, lo[1] - margin, lo[2] - margin], [hi[0] + margin, hi[1] + margin, hi[2] + margin])
}

/// [`start`] in a given world box (`from`..=`to`).
fn start_box(c: &Clock, blocks: &[(BlockPos, Block)], from: Pos, to: Pos) -> Result<Started, (Verdict, String)> {
    let min = BlockPos::new(from[0], from[1], from[2]);
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut w = World::new(min, size);
    let last = c.withheld().map(bp);
    let be_lines = |w: &mut World, only: Option<BlockPos>| -> Result<(), (Verdict, String)> {
        let lines: Vec<(BlockPos, &str)> = c.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).filter(|(p, _)| if only.is_some() { Some(*p) == only } else { Some(*p) != last }).collect();
        redsim::redstone::container::apply_block_entities(w, lines).map_err(|e| (Verdict::Unmodeled, format!("nbt: {e}")))
    };
    match c.placement {
        Placement::Judge => {
            let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
            w.place_raw(&silent);
            be_lines(&mut w, None)?;
            w.place_updates(&silent);
        }
        Placement::Player => {
            for (p, b) in blocks.iter().filter(|(p, _)| Some(*p) != last) {
                w.set_block(*p, *b, flags::ALL);
            }
            be_lines(&mut w, None)?;
        }
    }
    // the design's entities, once the blocks stand (as the real-game judge summons them)
    for e in &c.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| (Verdict::Unmodeled, format!("not modeled by redsim: {why}")))?;
    }
    Ok(Started { world: w, actions: c.actions() })
}

/// Apply the kick's inputs due at tick `t` (between ticks); check the world is still trustworthy.
fn between_ticks(s: &mut Started, t: u32) -> Result<(), (Verdict, String)> {
    while s.actions.first().is_some_and(|(at, _)| *at == t) {
        let (_, a) = s.actions.remove(0);
        match a {
            Action::Place { pos, state, nbt } => {
                let b = Block::parse(&state).map_err(|e| (Verdict::Unmodeled, e))?;
                s.world.set_block(bp(pos), b, flags::ALL);
                if let Some(n) = nbt {
                    redsim::redstone::container::apply_block_entities(&mut s.world, [(bp(pos), n.as_str())]).map_err(|e| (Verdict::Unmodeled, format!("nbt: {e}")))?;
                }
            }
            Action::UseEntity { pos } => {
                if !redsim::entity::frame::use_frame(&mut s.world, bp(pos)) {
                    return Err((Verdict::Unmodeled, format!("no single item frame at {pos:?} to turn")));
                }
            }
            Action::Use { pos } => {
                if !s.world.use_block(bp(pos)) {
                    return Err((Verdict::Unmodeled, format!("a right-click on {} is not modeled", s.world.get(bp(pos)))));
                }
            }
            Action::Clear { pos } => {
                s.world.set_block(bp(pos), Block::Air, flags::ALL);
            }
            Action::ShapeSet { pos, state } => {
                let b = Block::parse(&state).map_err(|e| (Verdict::Unmodeled, e))?;
                s.world.set_block(bp(pos), b, flags::CLIENTS);
            }
            Action::Update { pos } => {
                let src = s.world.get(bp(pos)).kind();
                s.world.update_neighbors_at(bp(pos), src);
            }
            Action::GameEvent { pos, event } => {
                s.world.game_event(event, bp(pos));
            }
            Action::Drop { pos, item } => {
                let (kind, at, nbt) = Action::drop_entity(pos, &item);
                redsim::entity::spawn(&mut s.world, &kind, at, Some(&nbt)).map_err(|e| (Verdict::Unmodeled, format!("a dropped item: {e}")))?;
            }
        }
    }
    if let Some(e) = s.world.errors.first() {
        if e.contains("outside the box") {
            return Err((Verdict::Invalid, format!("escapes its box by more than the margin by tick {t}: {e}")));
        }
        return Err((Verdict::Unmodeled, format!("tick {t}: {e}")));
    }
    if let Some((p, b)) = s.world.drops.first() {
        return Err((Verdict::Invalid, format!("tick {t}: {b} at {p} was destroyed with a drop (an item entity)")));
    }
    Ok(())
}

/// Is this state a component whose own state says whether it is on (else the output reads the
/// power the cell receives)?
fn self_reporting(s: &str) -> bool {
    wave::family_name(s).is_some()
}

/// The first `ticks` ticks after placement, one line per tick listing every cell that changed
/// (`mctas clock sim --trace N`): how a clock actually runs.
pub fn trace(c: &Clock, o: &SimOptions, ticks: u32) -> Vec<String> {
    let blocks = match parse_blocks(c) {
        Ok(b) => b,
        Err(e) => return vec![format!("not modeled by redsim: {e}")],
    };
    let mut s = match start(c, &blocks, o.margin) {
        Ok(s) => s,
        Err((_, why)) => return vec![why],
    };
    let mut out = Vec::new();
    let mut prev: HashMap<BlockPos, Block> = HashMap::new();
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t) {
            out.push(format!("t={t}: {why}"));
            break;
        }
        let now: HashMap<BlockPos, Block> = s.world.blocks().into_iter().collect();
        let mut changes: Vec<String> = Vec::new();
        let mut keys: Vec<BlockPos> = now.keys().chain(prev.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        for p in keys {
            let a = prev.get(&p).copied().unwrap_or(Block::Air);
            let b = now.get(&p).copied().unwrap_or(Block::Air);
            if a != b {
                changes.push(format!("{},{},{} {}", p.x, p.y, p.z, crate::short(&b.to_string())));
            }
        }
        let pending = s.world.ticks.count();
        out.push(format!("t={t} ({} pending ticks, {} moving): {}", pending, s.world.piston.live_count(), if changes.is_empty() { "-".into() } else { changes.join("; ") }));
        prev = now;
        s.world.tick();
    }
    out
}

/// A redsim world over `from..=to` with the design placed as the real-game judge places it (its
/// block entities and entities too), before any tick: for callers that drive the run themselves
/// (the shulker judge's replay, `mctas circuit shulker compare`).
pub fn world_for(c: &Clock, from: Pos, to: Pos) -> Result<World, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    start_box(c, &blocks, from, to).map(|s| s.world).map_err(|(_, why)| why)
}

/// Whether a block's state is shaped by its neighbours the way the design must store it (vjeux
/// 2026-10-21, "why don't the fences connect?"; the coordinator: a design stores what a player's
/// placement gives): a fence's arms, a fence gate's `in_wall`, a tripwire's connections (walls,
/// panes and bars when redsim models them). Not dust (the judge shapes it as it places it), nor
/// the states the first update sets with nothing to see (a note block's instrument, a repeater's
/// lock, leaves' distance).
pub fn shaped_by_neighbours(b: &Block) -> bool {
    matches!(b, Block::Fence { .. } | Block::FenceGate { .. } | Block::Tripwire { .. })
}

/// The design's neighbour-shaped blocks ([`shaped_by_neighbours`]) whose stored state is not what
/// a player's placement gives: placed like the judge (raw), each one's `updateFromNeighbourShapes`
/// (redsim's `update_shape` from its six neighbours). (cell, stored, placed).
pub fn shape_mismatches(c: &Clock) -> Result<Vec<(Pos, String, String)>, String> {
    let (lo, hi) = c.bounds();
    let from = [lo[0] - 8, lo[1] - 8, lo[2] - 8];
    let to = [hi[0] + 8, hi[1] + 8, hi[2] + 8];
    let mut w = world_for(c, from, to)?;
    let mut out = Vec::new();
    let mut cells: Vec<&Pos> = c.design.blocks.keys().collect();
    cells.sort();
    for p in cells {
        let b = w.get(bp(*p));
        if !shaped_by_neighbours(&b) {
            continue;
        }
        let placed = redsim::behavior::update_from_neighbour_shapes(&mut w, b, bp(*p));
        if placed != b && placed != Block::Air {
            out.push((*p, c.design.blocks[p].clone(), format!("{placed}")));
        }
    }
    Ok(out)
}

/// The design text with its neighbour-shaped blocks stored as placement gives them
/// ([`shape_mismatches`]): the old text's comments (in order), a comment saying what changed, then
/// the design.
pub fn with_placed_shapes(text: &str, c: &Clock) -> Result<Option<String>, String> {
    let fix = shape_mismatches(c)?;
    if fix.is_empty() {
        return Ok(None);
    }
    let mut d = c.design.clone();
    for (p, _, placed) in &fix {
        d.blocks.insert(*p, placed.clone());
    }
    let mut out = String::new();
    for l in text.lines().filter(|l| l.starts_with('#')) {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&format!("# 2026-10-21: {} block{} stored as a player's placement shapes {} (`mctas clock shapes`: a fence's arms, a gate's in_wall, a tripwire's connections)\n", fix.len(), if fix.len() == 1 { "" } else { "s" }, if fix.len() == 1 { "it" } else { "them" }));
    out.push_str(&d.to_text());
    Ok(Some(out))
}

/// [`drive`]'s loop for a world set up another way: placed like the judge places it in the box
/// `from`..=`to`, then `config` (e.g. `redsim::explosion::explode_on`), then for t in 0..=ticks:
/// the inputs due at t, `frame(t, world)`, one tick. Runs every tick even after a problem between
/// ticks (a drop, an error: the first one is returned with its tick, like [`drive`]'s `stopped`);
/// `Err` when redsim does not model the design.
/// [`drive_custom`] with the world MUTABLE in the frame callback (called between ticks, after
/// the kick's inputs due at that tick), for judges that act in the world themselves: the bedrock
/// study clones it to try a player's press in every window. `frame` returns false to stop.
pub fn drive_mut(c: &Clock, from: Pos, to: Pos, ticks: u32, mut frame: impl FnMut(u32, &mut World) -> bool) -> Result<Option<(u32, String)>, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to).map_err(|(_, why)| why)?;
    let mut first: Option<(u32, String)> = None;
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t)
            && first.is_none()
        {
            first = Some((t, why));
        }
        if !frame(t, &mut s.world) {
            break;
        }
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(first)
}

pub fn drive_custom(c: &Clock, from: Pos, to: Pos, ticks: u32, config: impl FnOnce(&mut World), mut frame: impl FnMut(u32, &World)) -> Result<Option<(u32, String)>, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to).map_err(|(_, why)| why)?;
    config(&mut s.world);
    let mut first: Option<(u32, String)> = None;
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t)
            && first.is_none()
        {
            first = Some((t, why));
        }
        frame(t, &s.world);
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(first)
}

/// The design's blocks that cannot stand where they are once placed like the judge places them:
/// the ones the placement removed (a floating dust loses its shape update) and the ones redsim's
/// `canSurvive` rejects (a floating lever, torch or repeater, which the raw placement keeps but a
/// player could not build), in the box `from`..=`to`. `Err` when redsim does not model the design.
pub fn unsupported(c: &Clock, from: Pos, to: Pos) -> Result<Vec<(Pos, String)>, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let s = start_box(c, &blocks, from, to).map_err(|(_, why)| why)?;
    let withheld = c.withheld();
    let mut out = Vec::new();
    for (p, s0) in &c.design.blocks {
        if Some(*p) == withheld {
            continue;
        }
        let b = s.world.get(bp(*p));
        if matches!(b, Block::Air) {
            out.push((*p, s0.clone()));
        } else if !redsim::behavior::can_survive(&s.world, &b, bp(*p)) {
            out.push((*p, b.to_string()));
        }
    }
    Ok(out)
}

/// One driven run on redsim (the circuit judge's screen, `mctas circuit sim`): what [`drive`]
/// recorded after every tick 0..=ticks.
pub struct Driven {
    /// Every non-air cell's full state (block state + comparator strength + container contents),
    /// sorted by position.
    pub cells: Vec<Vec<(Pos, String)>>,
    /// The best signal the neighbors give into each probe cell (what a lamp there would see).
    pub signals: Vec<Vec<u8>>,
    /// The run stopped early: the tick and why (a drop, a write outside the box, a case redsim does
    /// not model); `cells` and `signals` hold the frames before it.
    pub stopped: Option<(u32, String)>,
    /// TNT primed (track tnt): (the frame it first shows in, its cell).
    pub primed: Vec<(u32, Pos)>,
    /// Per frame, when the world has entities (the elevator's rider): every entity's kind (an
    /// armor stand is `armor_stand`) and position (its feet).
    pub entities: Vec<Vec<(String, [f64; 3])>>,
}

/// The entities of a world: (kind, feet position), insertion order.
pub fn entity_snapshot(w: &redsim::world::World) -> Vec<(String, [f64; 3])> {
    w.entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| {
            let k = match &e.kind {
                redsim::entity::EntityKind::ArmorStand(_) => "armor_stand",
                redsim::entity::EntityKind::Boat(_) => "boat",
                redsim::entity::EntityKind::Minecart(_) => "minecart",
                redsim::entity::EntityKind::Item(_) => "item",
                redsim::entity::EntityKind::PrimedTnt(_) => "tnt",
                redsim::entity::EntityKind::Mob(m) => m.ty.def().id,
                redsim::entity::EntityKind::Arrow(_) => "arrow",
            };
            (k.to_string(), [e.pos.x, e.pos.y, e.pos.z])
        })
        .collect()
}

/// Run a design on redsim in the world box `from`..=`to`: placed like the judge places it, its own
/// kick inputs and `extra` inputs applied between ticks (inputs due at tick t are applied after t
/// ticks, before frame t is recorded, like the real-game judge), recording [`Driven`] after every
/// tick 0..=ticks. Fails on a block redsim does not model; a problem during the run (a drop, a write
/// outside the box) stops it ([`Driven::stopped`]).
pub fn drive(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos) -> Result<Driven, (Verdict, String)> {
    drive_with(c, extra, ticks, probes, from, to, false)
}

/// [`drive`]; `remove_primed`: the caller removes every primed TNT before it explodes (the TNT
/// judge), so an explosion is no model note.
pub fn drive_with(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos, remove_primed: bool) -> Result<Driven, (Verdict, String)> {
    drive_seeded(c, extra, ticks, probes, from, to, remove_primed, None)
}

/// [`drive_with`]; `level`: the level random mirrored from this seed over these forced chunks
/// (`World::mirror_level_random`: the game sandbox reseeded the same way draws the same numbers,
/// so a dropper's random slot is modeled; the randomizer judge, crates/circuit/src/random.rs).
#[allow(clippy::too_many_arguments)]
pub fn drive_seeded(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos, remove_primed: bool, level: Option<(i64, [i32; 4])>) -> Result<Driven, (Verdict, String)> {
    // Each cell's state now (None: air), its text made again only when its block or extras change
    // (the same block and extras make the same string): the judges' runs formatted every non-air
    // cell at every tick (a full adder's 64 s, 2026-11-08). The cells read at a tick are the ones
    // the world's write log names and the ones whose extras change with no write, every cell at the
    // first tick, the last and every `TRACK_CHECK` ticks (as `simulate`'s record: a change neither
    // named runs it again reading every cell at every tick); a frame lists the non-air cells in
    // the box's x, y, z order, which is its order (by position).
    let mut tracked = true;
    loop {
        let mut out = Driven { cells: Vec::with_capacity(ticks as usize + 1), signals: Vec::with_capacity(ticks as usize + 1), stopped: None, primed: Vec::new(), entities: Vec::new() };
        let mut primed_seen = 0usize;
        type Now = Option<(Block, Option<String>, String, Option<u8>)>;
        let mut now: Vec<Now> = Vec::new();
        let mut cell_pos: Vec<BlockPos> = Vec::new();
        let mut present: Vec<usize> = Vec::new();
        let mut extras: Vec<usize> = Vec::new();
        let mut is_extras: Vec<bool> = Vec::new();
        let mut named: Vec<bool> = Vec::new();
        let mut every: Vec<usize> = Vec::new();
        let mut todo: Vec<usize> = Vec::new();
        let mut missed = false;
        let stopped = drive_each(c, extra, ticks, from, to, remove_primed, level, tracked, |t, world, written| {
            for &(_, p) in &world.primed[primed_seen..] {
                out.primed.push((t, p.arr()));
            }
            primed_seen = world.primed.len();
            let size = world.size;
            let n = (size[0] * size[1] * size[2]).max(0) as usize;
            if now.len() != n {
                now = vec![None; n];
                cell_pos = (0..size[0]).flat_map(|x| (0..size[1]).flat_map(move |y| (0..size[2]).map(move |z| (x, y, z)))).map(|(x, y, z)| world.min.offset(x, y, z)).collect();
                is_extras = vec![false; n];
                named = vec![false; n];
                every = (0..n).collect();
            }
            let index_of = |p: BlockPos| -> Option<usize> {
                let (x, y, z) = (p.x - world.min.x, p.y - world.min.y, p.z - world.min.z);
                (x >= 0 && y >= 0 && z >= 0 && x < size[0] && y < size[1] && z < size[2]).then(|| ((x * size[1] + y) * size[2] + z) as usize)
            };
            let full = !tracked || t == 0 || t == ticks || t % TRACK_CHECK == 0;
            todo.clear();
            if tracked {
                for q in written {
                    if let Some(i) = index_of(*q)
                        && !named[i]
                    {
                        named[i] = true;
                        todo.push(i);
                    }
                }
                for &i in &extras {
                    if !named[i] {
                        named[i] = true;
                        todo.push(i);
                    }
                }
                todo.sort_unstable();
            }
            let mut extras_moved = false;
            for &i in if full { &every } else { &todo } {
                let p = cell_pos[i];
                let b = world.get(p);
                let reads = wave::reads_extras(&b);
                if reads != is_extras[i] {
                    is_extras[i] = reads;
                    extras_moved = true;
                }
                let was_present = now[i].is_some();
                let changed = if b.is_air() {
                    now[i] = None;
                    was_present
                } else {
                    // (the same block holding the same reads: its text kept,
                    // `wave::comparator_read`, `wave::reads_extras`)
                    let quick = wave::comparator_read(world, p, &b);
                    let held = match &now[i] {
                        Some((sb, _, _, sq)) if *sb == b => {
                            if quick.is_some() {
                                *sq == quick
                            } else {
                                !reads
                            }
                        }
                        _ => false,
                    };
                    if held {
                        false
                    } else {
                        let extra = wave::be_extra_redsim(world, p, &b);
                        if let Some(e) = now[i].as_mut().filter(|e| e.0 == b && e.1 == extra) {
                            e.3 = quick;
                            false
                        } else {
                            let s = wave::with_extra(&b.to_string(), extra.as_deref());
                            now[i] = Some((b, extra, s, quick));
                            true
                        }
                    }
                };
                if changed {
                    // a change neither the write log nor the extras named
                    if tracked && t > 0 && !named[i] {
                        missed = true;
                    }
                    let is_present = now[i].is_some();
                    if is_present != was_present {
                        match present.binary_search(&i) {
                            Ok(k) if !is_present => {
                                present.remove(k);
                            }
                            Err(k) if is_present => present.insert(k, i),
                            _ => {}
                        }
                    }
                }
            }
            for &i in &todo {
                named[i] = false;
            }
            if missed {
                return false;
            }
            if extras_moved {
                extras = (0..n).filter(|&i| is_extras[i]).collect();
            }
            out.cells.push(present.iter().map(|&i| (cell_pos[i].arr(), now[i].as_ref().map(|e| e.2.clone()).unwrap_or_default())).collect());
            out.signals.push(probes.iter().map(|q| world.best_neighbor_signal(bp(*q)).clamp(0, 15) as u8).collect());
            if !world.entities.list.is_empty() {
                out.entities.push(entity_snapshot(world));
            }
            true
        })?;
        if missed {
            tracked = false;
            continue;
        }
        out.stopped = stopped;
        return Ok(out);
    }
}

/// The run [`drive_seeded`] makes, each tick's world handed to `each` (after the tick's actions,
/// before the world ticks) instead of kept, with `log`: the cells a write changed since the last
/// call (`World::log_writes`; none at the first: the placement is not logged); `each` returning
/// false ends the run there. Returns where it stopped early (the tick and why; an empty why: `each`
/// ended it).
#[allow(clippy::too_many_arguments)]
fn drive_each(c: &Clock, extra: &[(u32, Action)], ticks: u32, from: Pos, to: Pos, remove_primed: bool, level: Option<(i64, [i32; 4])>, log: bool, mut each: impl FnMut(u32, &World, &[BlockPos]) -> bool) -> Result<Option<(u32, String)>, (Verdict, String)> {
    let mut blocks = parse_blocks(c).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
    for (_, a) in extra {
        if let Action::Place { state, .. } | Action::ShapeSet { state, .. } = a {
            Block::parse(state).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
        }
    }
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to)?;
    if let Some((seed, chunks)) = level {
        s.world.mirror_level_random(seed, chunks);
        // the sandbox's random ticks (randomTickSpeed 3) on the mirrored randValue: only the
        // sections holding a randomly ticking block take positions (mobinfra: an unwaxed copper
        // bulb's oxidation in a circuit's trace)
        s.world.random_ticks = Some(redsim::crops::RandomTicks::new(3, seed));
    }
    s.world.remove_primed = remove_primed;
    s.actions.extend(extra.iter().cloned());
    s.actions.sort_by_key(|(t, _)| *t);
    s.world.log_writes = log;
    s.world.writes.clear();
    let mut written: Vec<BlockPos> = Vec::new();
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t) {
            return Ok(Some((t, why)));
        }
        if log {
            std::mem::swap(&mut written, &mut s.world.writes);
        }
        if !each(t, &s.world, &written) {
            return Ok(Some((t, String::new())));
        }
        written.clear();
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(None)
}

/// A live run for long benches (the carousel counter's 100,000-pulse bench, day-1 lane
/// 2026-11-10): the build started as [`drive_seeded`] starts it (no level random), `extra`
/// applied between ticks in order (walked by index: a bench brings hundreds of thousands of
/// actions, which `between_ticks`' `remove(0)` walk would make quadratic), then `each(t, world)`
/// (it reads its probes and may reload chunks: `redsim::reload`; false stops the run), then the
/// tick. Nothing is recorded.
pub fn drive_live(c: &Clock, extra: &[(u32, Action)], ticks: u32, from: Pos, to: Pos, mut each: impl FnMut(u32, &mut World) -> bool) -> Result<Option<(u32, String)>, (Verdict, String)> {
    let mut blocks = parse_blocks(c).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
    for (_, a) in extra {
        if let Action::Place { state, .. } | Action::ShapeSet { state, .. } = a {
            Block::parse(state).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
        }
    }
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to)?;
    let mut all: Vec<(u32, Action)> = std::mem::take(&mut s.actions);
    all.extend(extra.iter().cloned());
    all.sort_by_key(|(t, _)| *t);
    let mut k = 0usize;
    for t in 0..=ticks {
        let first = k;
        while k < all.len() && all[k].0 <= t {
            k += 1;
        }
        s.actions = all[first..k].iter().map(|(_, a)| (t, a.clone())).collect();
        if let Err((_, why)) = between_ticks(&mut s, t) {
            return Ok(Some((t, why)));
        }
        if !each(t, &mut s.world) {
            return Ok(Some((t, String::new())));
        }
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(None)
}

/// Why a declared output cell is not where a builder can take the signal (vjeux / coordinator,
/// 2026-10-20), or None: it must be AIR (only the judge's lamp lives there; `first` is its block at
/// the start of the period: the entered check covers the rest of the cycle), on the swept box's
/// boundary or one cell outside it with a face to free space, and not above tripwire, rails or
/// water (`below`: the states under it during the period).
pub fn placement_problem<'a>(op: Pos, swept: (Pos, Pos), first: &str, mut below: impl Iterator<Item = &'a str>) -> Option<String> {
    let name = mctas::blockstate::block_name(first);
    if !(name.ends_with("air")) {
        return Some(format!("the output cell {op:?} holds {}: it must be an AIR cell where the signal arrives (a lamp, dust or repeater put there gets it)", crate::short(first)));
    }
    let (slo, shi) = swept;
    let inside = |q: Pos| (0..3).all(|k| q[k] >= slo[k] && q[k] <= shi[k]);
    let dirs = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    let free = dirs.iter().any(|d| !inside([op[0] + d[0], op[1] + d[1], op[2] + d[2]]));
    let near = inside(op) || dirs.iter().any(|d| inside([op[0] + d[0], op[1] + d[1], op[2] + d[2]]));
    if !free || !near {
        return Some(format!("the output cell {op:?} is not on the build's exterior: it must sit on the boundary of the swept box {slo:?}..{shi:?} or one cell outside it, with a face to free space"));
    }
    if let Some(s) = below.find(|s| s.contains("tripwire") || s.contains("rail") || s.contains("water")) {
        return Some(format!("the output cell {op:?} sits on {}: not a place to take a signal", crate::short(s)));
    }
    None
}

/// Where a builder can take a clock's signal (vjeux / coordinator, 2026-10-20): an AIR cell free all
/// cycle (no block moved into or through it, no entity), on the swept box's boundary or one cell
/// outside it with a face to free space, not above tripwire, rails or water, whose received power
/// rises at evenly spaced ticks. Ranked: its pulses every `period` gt first (the clock's own
/// period), then the longest even period, the longest on-time, the lowest, the fewest changing
/// neighbours, a side over a top over a bottom position, then the position. The run: `ticks` gt
/// (warmup + period at least), its period read from tick `w`.
pub fn exterior_outputs(c: &Clock, w: u32, period: u32, swept: (Pos, Pos), occupied: &[Pos]) -> Vec<(Pos, CellWave)> {
    exterior_scan(c, w, period, swept, occupied, false)
}

/// [`exterior_outputs`], with `all`: also the candidate cells whose pulses are uneven (a report of
/// why a clock has no output: `clock sim --exterior-debug`).
pub fn exterior_scan(c: &Clock, w: u32, period: u32, swept: (Pos, Pos), occupied: &[Pos], all: bool) -> Vec<(Pos, CellWave)> {
    let (slo, shi) = swept;
    let inside = |q: Pos| (0..3).all(|k| q[k] >= slo[k] && q[k] <= shi[k]);
    let mut cand: Vec<Pos> = Vec::new();
    for x in slo[0] - 1..=shi[0] + 1 {
        for y in slo[1] - 1..=shi[1] + 1 {
            for z in slo[2] - 1..=shi[2] + 1 {
                let q = [x, y, z];
                // a face to free space: a neighbour outside the swept box
                let free = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d: &[i32; 3]| !inside([q[0] + d[0], q[1] + d[1], q[2] + d[2]]));
                // adjacent to the build: one cell outside the box counts only next to a face
                let near = inside(q) || [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d: &[i32; 3]| inside([q[0] + d[0], q[1] + d[1], q[2] + d[2]]));
                if free && near && !occupied.contains(&q) {
                    cand.push(q);
                }
            }
        }
    }
    let (lo, hi) = c.bounds();
    let (from, to) = ([lo[0].min(slo[0]) - 4, lo[1].min(slo[1]) - 4, lo[2].min(slo[2]) - 4], [hi[0].max(shi[0]) + 4, hi[1].max(shi[1]) + 4, hi[2].max(shi[2]) + 4]);
    // the run read as it goes, no frame kept (a long family member's kept every non-air cell's
    // state as a string at every tick, then a map of each tick to compare: 312,887 gt of
    // clk-rt-counter n = 3, 2026-11-08): every cell a block ever stands in and the cells holding
    // tripwire / rails / water at any tick; the cells whose state differs from the period's first
    // tick at some tick of the period; each candidate's power over the period. Cells are indexed
    // in the run's box (every block of the run stands in it); a cell's state string is made again
    // only when its block or extras change (the same block and extras make the same string), and
    // a cell is read only when the world's write log names it or its extras can change with no
    // write, all of them at the first tick, at W, at the last and every `TRACK_CHECK` ticks (as
    // `simulate`'s record: a full read that finds a change neither named reads every cell at every
    // tick from the start again).
    let end = w + period;
    let dims = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let idx = |q: Pos| -> Option<usize> {
        let r = [q[0] - from[0], q[1] - from[1], q[2] - from[2]];
        (0..3).all(|k| r[k] >= 0 && r[k] < dims[k]).then(|| ((r[1] * dims[2] + r[2]) * dims[0] + r[0]) as usize)
    };
    let n = (dims[0] * dims[1] * dims[2]).max(0) as usize;
    // the box's cells in its own order (y, z, x: the world's)
    let box_cells: Vec<Pos> = (0..dims[1]).flat_map(|y| (0..dims[2]).flat_map(move |z| (0..dims[0]).map(move |x| [from[0] + x, from[1] + y, from[2] + z]))).collect();
    let every: Vec<usize> = (0..n).collect();
    let mut tracked = true;
    let (filled, bad_floor, changing, signals) = loop {
        let mut filled = vec![false; n];
        let mut bad_floor = vec![false; n];
        let mut changing = vec![false; n];
        let mut base: Vec<Option<String>> = vec![None; n];
        // a cell's state now (None: air): its block and extras, their text, whether it is
        // tripwire / rails / water, a comparator's strength
        type Now = Option<(Block, Option<String>, String, bool, Option<u8>)>;
        let mut now: Vec<Now> = vec![None; n];
        let mut extras: Vec<usize> = Vec::new();
        let mut is_extras = vec![false; n];
        let mut named = vec![false; n];
        let mut todo: Vec<usize> = Vec::new();
        let mut missed = false;
        let mut signals: Vec<Vec<bool>> = vec![Vec::with_capacity(period as usize); cand.len()];
        // each candidate's power now, and the candidates near each cell of the box: a change can
        // move the power a candidate receives only from within `SIGNAL_REACH` cells of it (a
        // conductor's power is its neighbours', a wire's shape reads its neighbours' up and down:
        // 3 at most); a candidate is read again when a cell near it changes, every candidate at
        // every full read, which finds any other change (then every candidate at every tick)
        let mut near: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (k, q) in cand.iter().enumerate() {
            for dx in -SIGNAL_REACH..=SIGNAL_REACH {
                for dy in -SIGNAL_REACH..=SIGNAL_REACH {
                    for dz in -SIGNAL_REACH..=SIGNAL_REACH {
                        if let Some(i) = idx([q[0] + dx, q[1] + dy, q[2] + dz]) {
                            near[i].push(k as u32);
                        }
                    }
                }
            }
        }
        let mut sig: Vec<bool> = vec![false; cand.len()];
        let mut dirty: Vec<bool> = vec![false; cand.len()];
        let mut dirty_list: Vec<u32> = Vec::new();
        let run = drive_each(c, &[], end, from, to, false, None, tracked, |t, world, written| {
            let in_period = t > w && t < end;
            let full = !tracked || t == 0 || t == w || t == end || t % TRACK_CHECK == 0;
            todo.clear();
            if tracked {
                for q in written {
                    if let Some(i) = idx(q.arr())
                        && !named[i]
                    {
                        named[i] = true;
                        todo.push(i);
                    }
                }
                for &i in &extras {
                    if !named[i] {
                        named[i] = true;
                        todo.push(i);
                    }
                }
            }
            let mut extras_moved = false;
            for &i in if full { &every } else { &todo } {
                let p = bp(box_cells[i]);
                let b = world.get(p);
                let reads = wave::reads_extras(&b);
                if reads != is_extras[i] {
                    is_extras[i] = reads;
                    extras_moved = true;
                }
                let changed = if b.is_air() {
                    let was = now[i].is_some();
                    now[i] = None;
                    was
                } else {
                    // (the same block holding the same reads: its text kept,
                    // `wave::comparator_read`, `wave::reads_extras`)
                    let quick = wave::comparator_read(world, p, &b);
                    let held = match &now[i] {
                        Some((sb, _, _, _, sq)) if *sb == b => {
                            if quick.is_some() {
                                *sq == quick
                            } else {
                                !reads
                            }
                        }
                        _ => false,
                    };
                    if held {
                        false
                    } else {
                        let extra = wave::be_extra_redsim(world, p, &b);
                        if let Some(e) = now[i].as_mut().filter(|e| e.0 == b && e.1 == extra) {
                            e.4 = quick;
                            false
                        } else {
                            let s = wave::with_extra(&b.to_string(), extra.as_deref());
                            let bad = s.contains("tripwire") || s.contains("rail") || s.contains("water");
                            now[i] = Some((b, extra, s, bad, quick));
                            true
                        }
                    }
                };
                if changed || (t == 0 && now[i].is_some()) {
                    for &k in &near[i] {
                        if !dirty[k as usize] {
                            dirty[k as usize] = true;
                            dirty_list.push(k);
                        }
                    }
                    // a change neither the write log nor the extras named
                    if tracked && t > 0 && !named[i] {
                        missed = true;
                    }
                    if let Some((_, _, s, bad, _)) = &now[i] {
                        filled[i] = true;
                        if *bad {
                            bad_floor[i] = true;
                        }
                        if in_period && !changing[i] && base[i].as_deref() != Some(s.as_str()) {
                            changing[i] = true;
                        }
                    } else if in_period && base[i].is_some() {
                        changing[i] = true;
                    }
                }
            }
            for &i in &todo {
                named[i] = false;
            }
            if missed {
                return false;
            }
            if extras_moved {
                extras = (0..n).filter(|&i| is_extras[i]).collect();
            }
            if t == w {
                for i in 0..n {
                    base[i] = now[i].as_ref().map(|e| e.2.clone());
                }
            }
            if t >= w && t < end {
                if full {
                    for (k, q) in cand.iter().enumerate() {
                        let s = world.best_neighbor_signal(bp(*q)).clamp(0, 15) > 0;
                        // a change in a candidate's power with no change near it
                        if tracked && t > w && s != sig[k] && !dirty[k] {
                            missed = true;
                        }
                        sig[k] = s;
                    }
                } else {
                    for &k in &dirty_list {
                        sig[k as usize] = world.best_neighbor_signal(bp(cand[k as usize])).clamp(0, 15) > 0;
                    }
                }
                for (k, s) in sig.iter().enumerate() {
                    signals[k].push(*s);
                }
            }
            for &k in &dirty_list {
                dirty[k as usize] = false;
            }
            dirty_list.clear();
            !missed
        });
        if missed {
            tracked = false;
            continue;
        }
        if !matches!(run, Ok(None)) {
            return Vec::new();
        }
        break (filled, bad_floor, changing, signals);
    };
    let mut out: Vec<((bool, u32, u32, i32, usize, u8, Pos), Pos, CellWave)> = Vec::new();
    for (i, q) in cand.iter().enumerate() {
        if idx(*q).is_some_and(|i| filled[i]) || idx([q[0], q[1] - 1, q[2]]).is_some_and(|i| bad_floor[i]) {
            continue;
        }
        let seq: &[bool] = &signals[i];
        let gaps = wave::rise_gaps(&seq);
        let Some(&g0) = gaps.first() else { continue };
        if !all && gaps.iter().any(|&g| g != g0) {
            continue;
        }
        let (on, edges, widths) = wave::bool_wave(&seq);
        let moving = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().filter(|dd: &&[i32; 3]| idx([q[0] + dd[0], q[1] + dd[1], q[2] + dd[2]]).is_some_and(|i| changing[i])).count();
        // side over top over bottom: where its free face looks
        let side = if [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|dd: &[i32; 3]| !inside([q[0] + dd[0], q[1] + dd[1], q[2] + dd[2]])) && q[1] >= slo[1] && q[1] <= shi[1] {
            0
        } else if q[1] > shi[1] || !inside([q[0], q[1] + 1, q[2]]) {
            1
        } else {
            2
        };
        let key = (g0 != period, u32::MAX - g0, u32::MAX - on, q[1], moving, side, *q);
        let cw = CellWave { pos: *q, states: Vec::new(), kinds: Vec::new(), on, edges, widths, gaps };
        out.push((key, *q, cw));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, q, w)| (q, w)).collect()
}

/// [`exterior_outputs`] confirmed with the judge's lamp in redsim, best first (at most `want`):
/// each candidate becomes the clock's `port output` (the judge's lamp goes there) and must give a
/// CLOCK of the same period as `free` (the clock with no output port: the lamp changes nothing),
/// passing the output rules, with the lamp lighting at evenly spaced ticks.
pub fn lamp_outputs(free: &Clock, want: usize) -> Vec<(Pos, SimReport)> {
    let base = simulate(free, &SimOptions::default());
    if base.verdict != Verdict::Clock {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (q, _) in exterior_outputs(free, base.state_warmup, base.period, base.swept, &base.occupied).into_iter().take(40) {
        let mut d = free.design.clone();
        d.ports.insert("output".into(), q);
        let Ok(v) = Clock::from_design(d) else { continue };
        let r = simulate(&v, &SimOptions::default());
        let even = r.output.as_ref().is_some_and(|o| !o.wave.gaps.is_empty() && o.wave.irregular().is_none());
        if r.verdict == Verdict::Clock && r.period == base.period && even {
            out.push((q, r));
            if out.len() >= want {
                break;
            }
        }
    }
    out
}

/// The exterior output cell of `free` (no declared output) where the judge's lamp lights once per
/// `period` ticks (the builder period: one rising edge per cycle, else the even spacing), else the
/// first one where it lights evenly; with the run with that output. The same candidates as
/// [`lamp_outputs`], each run with `o` (a long family member needs a longer horizon).
pub fn lamp_output_for(free: &Clock, period: u32, o: &SimOptions) -> Option<(Pos, SimReport)> {
    let base = simulate(free, o);
    if base.verdict != Verdict::Clock {
        return None;
    }
    let candidates: Vec<Pos> = exterior_outputs(free, base.state_warmup, base.period, base.swept, &base.occupied).into_iter().take(40).map(|(q, _)| q).collect();
    // the candidates' runs a few at a time on threads of their own, read in their order: the same
    // pick as one after the other (a long family member's runs were a site build's long pole:
    // clk-rt-counter n = 3, its base and its first candidate at 940,448 gt, 2026-11-08)
    let mut first = None;
    for chunk in candidates.chunks(LAMP_CANDIDATES_AT_ONCE) {
        let runs: Vec<Option<SimReport>> = std::thread::scope(|s| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|q| {
                    s.spawn(move || {
                        let mut d = free.design.clone();
                        d.ports.insert("output".into(), *q);
                        Clock::from_design(d).ok().map(|v| simulate(&v, o))
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().ok().flatten()).collect()
        });
        for (q, r) in chunk.iter().zip(runs) {
            let Some(r) = r else { continue };
            let even = r.output.as_ref().is_some_and(|w| !w.wave.gaps.is_empty() && w.wave.irregular().is_none());
            if r.verdict == Verdict::Clock && r.period == base.period && even {
                if crate::extend::builder_period_of(&r) == Some(period) {
                    return Some((*q, r));
                }
                if first.is_none() {
                    first = Some((*q, r));
                }
            }
        }
    }
    first
}

/// The candidates [`lamp_output_for`] runs at once.
const LAMP_CANDIDATES_AT_ONCE: usize = 4;

/// Every this many ticks [`simulate`]'s period record reads every cell, a check of the world's
/// write log (and at its first tick, at W and at its last).
const TRACK_CHECK: u32 = 4096;

/// How far (in each axis) a cell's change can move the power next to a cell
/// (`World::best_neighbor_signal`: a conductor's power is its neighbours', a wire's shape reads its
/// neighbours' up and down: 3), with one to spare: [`exterior_scan`].
const SIGNAL_REACH: i32 = 4;

/// TNT dupers with explosions on (track tnt, all-randoms judge): how [`drive_tnt`] runs them.
#[derive(Clone, Debug)]
pub struct TntOptions {
    /// The kick of every primed TNT (`World::tnt_kick`: the angle's `nextDouble()`), unless
    /// `kicks` gives the k-th spawn's own.
    pub kick: f64,
    pub kicks: Vec<f64>,
    /// Every explosion ray at its largest roll (`explosion::Rolls::Max`): surviving it is surviving
    /// every roll.
    pub max_rolls: bool,
    /// A burning TNT this far from `design_box` is removed as out of range (the real-game judge's
    /// rule, `circuit::run::OUT_OF_RANGE`).
    pub out_of_range: f64,
    pub design_box: (Pos, Pos),
    /// When set, only a destroyed block inside this box blows the machine up (the others, a floor
    /// under it for instance, are counted in `TntDriven::blasts`).
    pub machine_box: Option<(Pos, Pos)>,
    /// Ground blocks placed (raw) before the run; frames leave out everything under the design
    /// box's lowest layer (its craters are not the machine).
    pub floor: Vec<(Pos, String)>,
    /// The cells the frames record (default: the whole world box).
    pub watch: Option<(Pos, Pos)>,
    /// A burning TNT whose horizontal distance to `design_box` exceeds this has left the ground
    /// (`TntDriven::escaped`; the caller grows the ground and runs again).
    pub escape: Option<f64>,
}

/// One primed TNT followed by [`drive_tnt`] (the fields the real-game runner records).
#[derive(Clone, Debug, Default)]
pub struct TntRecord {
    pub frame: u32,
    pub first_pos: [f64; 3],
    pub first_motion: [f64; 3],
    pub launch: Option<[f64; 3]>,
    pub end: Option<([f64; 3], [f64; 3])>,
    pub end_frame: u32,
    pub moved: u32,
    pub last_pos: [f64; 3],
    pub gone: Option<([f64; 3], [f64; 3])>,
    /// Where it was at each of its first frames and then every 8 frames (frame, position): a
    /// blown-up verdict prints the culprit's path (2026-10-21: the fast engine's dud TNT)
    pub path: Vec<(u32, [f64; 3])>,
}

/// The result of [`drive_tnt`].
pub struct TntDriven {
    pub driven: Driven,
    pub tnt: Vec<TntRecord>,
    /// The frame of every explosion.
    pub explosions: Vec<u32>,
    pub out_of_range: u32,
    /// Every explosion: its frame, its center, the blocks it destroyed inside `machine_box` (the
    /// design box when unset) and elsewhere.
    pub blasts: Vec<(u32, [f64; 3], usize, usize)>,
    /// Every explosion (as `blasts`): whether its center's cell holds a fluid (it destroys
    /// nothing) and how many blocks it would destroy on open ground (`redsim::explosion::open_ground`).
    pub blast_use: Vec<(bool, usize)>,
    /// The first burning TNT that went past `TntOptions::escape`: its frame and position.
    pub escaped: Option<(u32, [f64; 3])>,
}

/// [`drive_with`] for TNT dupers with their explosions (redsim's exact port): every primed TNT is
/// an entity with the given kick, explodes at fuse 0 (rays at their largest roll with
/// `max_rolls`), and is removed out of range like the real-game judge does. A block of the design
/// destroyed by an explosion stops the run with `blown up: ...`.
pub fn drive_tnt(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos, o: &TntOptions) -> Result<TntDriven, (Verdict, String)> {
    let mut blocks = parse_blocks(c).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
    for (_, a) in extra {
        if let Action::Place { state, .. } | Action::ShapeSet { state, .. } = a {
            Block::parse(state).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
        }
    }
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to)?;
    // the ground, raw (it stands under the design before anything runs)
    if !o.floor.is_empty() {
        let mut ground: Vec<(BlockPos, Block)> = Vec::with_capacity(o.floor.len());
        for (p, st) in &o.floor {
            ground.push((bp(*p), Block::parse(st).map_err(|e| (Verdict::Unmodeled, format!("ground {st}: {e}")))?));
        }
        s.world.place_raw(&ground);
    }
    s.world.remove_primed = false;
    s.world.tnt_kick = Some(o.kick);
    s.world.tnt_kicks = o.kicks.clone();
    if o.max_rolls {
        s.world.explosion_rolls = redsim::explosion::Rolls::Max;
    }
    s.actions.extend(extra.iter().cloned());
    s.actions.sort_by_key(|(t, _)| *t);
    let mut out = TntDriven { driven: Driven { cells: Vec::with_capacity(ticks as usize + 1), signals: Vec::with_capacity(ticks as usize + 1), stopped: None, primed: Vec::new(), entities: Vec::new() }, tnt: Vec::new(), explosions: Vec::new(), out_of_range: 0, blasts: Vec::new(), blast_use: Vec::new(), escaped: None };
    let mut index: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut blasts_seen = 0usize;
    let (mut primed_seen, mut chain_seen) = (0usize, 0usize);
    let (lo, hi) = o.design_box;
    for t in 0..=ticks {
        // explosions of the last tick: a design block they destroyed blows the machine up
        let (mlo, mhi) = o.machine_box.unwrap_or(o.design_box);
        let inside = |p: &BlockPos| p.x >= mlo[0] && p.x <= mhi[0] && p.y >= mlo[1] && p.y <= mhi[1] && p.z >= mlo[2] && p.z <= mhi[2];
        for b in &s.world.explosions[blasts_seen..] {
            out.explosions.push(t);
            let mine = b.destroyed.iter().filter(|(p, _)| o.machine_box.is_none() || inside(p)).count();
            out.blasts.push((t, [b.center.x, b.center.y, b.center.z], mine, b.destroyed.len() - mine));
            out.blast_use.push((b.in_fluid, b.open_ground));
            if let Some((p, blk)) = b.destroyed.iter().find(|(p, _)| o.machine_box.is_none() || inside(p)) {
                // the TNT behind it (coordinator 2026-10-21: "which block breaks first, from which
                // TNT kick"): the record last seen nearest the blast's center; its index in spawn
                // order is its kick's
                let c3 = [b.center.x, b.center.y, b.center.z];
                let d2 = |q: &[f64; 3]| (0..3).map(|i| (q[i] - c3[i]).powi(2)).sum::<f64>();
                let who = out.tnt.iter().enumerate().min_by(|a, b| d2(&a.1.last_pos).total_cmp(&d2(&b.1.last_pos))).map(|(k, r)| {
                    let kick = o.kicks.get(k).copied().unwrap_or(o.kick);
                    let v = |a: [f64; 3]| format!("({:.3}, {:.3}, {:.3})", a[0], a[1], a[2]);
                    let launch = r.launch.map(|l| format!("launched at {}", v(l))).unwrap_or_else(|| "never launched".into());
                    let path: Vec<String> = r.path.iter().map(|(f, p)| format!("{f}: ({:.2}, {:.2}, {:.2})", p[0], p[1], p[2])).collect();
                    format!(
                        " by TNT {k} (primed in frame {}, kick nextDouble {kick:.6}; spawned at {} moving {}, {launch}, at rest from frame {}; path {})",
                        r.frame,
                        v(r.first_pos),
                        v(r.first_motion),
                        r.moved,
                        path.join(" ")
                    )
                });
                out.driven.stopped = Some((t, format!("blown up: {blk} at {} destroyed by an explosion at frame {t} centered at ({:.2}, {:.2}, {:.2}){}", p, c3[0], c3[1], c3[2], who.unwrap_or_default())));
            }
        }
        blasts_seen = s.world.explosions.len();
        if out.driven.stopped.is_some() {
            break;
        }
        // a blast's drops (the ground's craters; a machine block destroyed has already stopped
        // the run) are expected here
        s.world.drops.clear();
        if let Err((_, why)) = between_ticks(&mut s, t) {
            out.driven.stopped = Some((t, why));
            break;
        }
        for e in s.world.entities.list.iter_mut() {
            let redsim::entity::EntityKind::PrimedTnt(st) = e.kind else { continue };
            let pos = [e.pos.x, e.pos.y, e.pos.z];
            let mot = [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z];
            let k = *index.entry(e.id).or_insert_with(|| {
                out.tnt.push(TntRecord { frame: t, first_pos: pos, first_motion: mot, moved: t, last_pos: pos, ..Default::default() });
                out.tnt.len() - 1
            });
            let r = &mut out.tnt[k];
            let h = |m: &[f64; 3]| (m[0] * m[0] + m[2] * m[2]).sqrt();
            if h(&mot) >= 0.4 && r.launch.is_none_or(|l| h(&mot) > h(&l)) {
                r.launch = Some(mot);
            }
            if pos.iter().zip(&r.last_pos).any(|(a, b)| a.to_bits() != b.to_bits()) {
                r.moved = t;
                r.last_pos = pos;
            }
            let age = t - r.frame;
            if age < 8 || age % 8 == 0 {
                r.path.push((t, pos));
            }
            if st.fuse <= 1 && r.end.is_none() {
                r.end = Some((pos, mot));
                r.end_frame = t;
            }
            let dist = (0..3).map(|i| (lo[i] as f64 - pos[i]).max(pos[i] - (hi[i] as f64 + 1.0)).max(0.0)).fold(0.0f64, f64::max);
            if let Some(reach) = o.escape {
                let hdist = [0usize, 2].iter().map(|&i| (lo[i] as f64 - pos[i]).max(pos[i] - (hi[i] as f64 + 1.0)).max(0.0)).fold(0.0f64, f64::max);
                if hdist > reach && out.escaped.is_none() {
                    out.escaped = Some((t, pos));
                }
            }
            if dist > o.out_of_range && !e.removed {
                e.removed = true;
                r.gone = Some((pos, mot));
                out.out_of_range += 1;
            }
        }
        s.world.entities.list.retain(|e| !e.removed);
        for &(_, p) in &s.world.primed[primed_seen..] {
            out.driven.primed.push((t, p.arr()));
        }
        primed_seen = s.world.primed.len();
        // an explosion-primed TNT is a new TNT too (the real-game runner sees every new entity)
        for &(_, p) in &s.world.chain_primed[chain_seen..] {
            out.driven.primed.push((t, p.arr()));
        }
        chain_seen = s.world.chain_primed.len();
        let mut v: Vec<(Pos, String)> = match o.watch {
            // the watched cells only, nothing under the design's lowest layer (the ground)
            Some((wf, wt)) => {
                let mut v = Vec::new();
                for x in wf[0]..=wt[0] {
                    for y in wf[1].max(lo[1])..=wt[1] {
                        for z in wf[2]..=wt[2] {
                            let p = BlockPos::new(x, y, z);
                            let b = s.world.get(p);
                            if !matches!(b, Block::Air) {
                                v.push(([x, y, z], wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref())));
                            }
                        }
                    }
                }
                v
            }
            None => s.world.blocks().into_iter().filter(|(p, _)| o.floor.is_empty() || p.y >= lo[1]).map(|(p, b)| (p.arr(), wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref()))).collect(),
        };
        v.sort_by_key(|(p, _)| *p);
        out.driven.cells.push(v);
        out.driven.signals.push(probes.iter().map(|q| s.world.best_neighbor_signal(bp(*q)).clamp(0, 15) as u8).collect());
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(out)
}

/// Every block of the world (with comparator strengths and container contents) after each tick
/// 0..=ticks, sorted by position: for comparing two runs (`ident`). `None` when redsim does not
/// model the design or the run meets something it does not model.
pub fn run_states(c: &Clock, o: &SimOptions, ticks: u32) -> Option<Vec<Vec<(BlockPos, String)>>> {
    if !c.design.entities.is_empty() {
        return None;
    }
    let blocks = parse_blocks(c).ok()?;
    let mut s = start(c, &blocks, o.margin).ok()?;
    let mut out = Vec::with_capacity(ticks as usize + 1);
    for t in 0..=ticks {
        between_ticks(&mut s, t).ok()?;
        let mut v: Vec<(BlockPos, String)> = s.world.blocks().into_iter().map(|(p, b)| (p, wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref()))).collect();
        v.sort_by_key(|(p, _)| *p);
        out.push(v);
        s.world.tick();
    }
    Some(out)
}

/// The census's fast screen (no waveform, no strings): place `blocks` like the judge (raw writes,
/// wire shapes, self updates), the block at index `last` withheld and placed last with updates
/// when given, then run until the whole state repeats. `Some((state_warmup, state_period))` for a
/// state that keeps changing; `None` when it goes still, never repeats within `horizon`, or the
/// model meets something it does not model exactly (an error, a drop, a write beyond `margin`).
pub fn quick_period(blocks: &[(BlockPos, Block)], last: Option<usize>, margin: i32, horizon: u32) -> Option<(u32, u32)> {
    let mut lo = blocks[0].0;
    let mut hi = blocks[0].0;
    for (p, _) in blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let min = BlockPos::new(lo.x - margin, lo.y - margin, lo.z - margin);
    let size = [hi.x - lo.x + 1 + 2 * margin, hi.y - lo.y + 1 + 2 * margin, hi.z - lo.z + 1 + 2 * margin];
    let mut w = World::new(min, size);
    let silent: Vec<(BlockPos, Block)> = blocks.iter().enumerate().filter(|(i, _)| Some(*i) != last).map(|(_, b)| *b).collect();
    w.place_raw(&silent);
    w.place_updates(&silent);
    if let Some(i) = last {
        w.set_block(blocks[i].0, blocks[i].1, flags::ALL);
    }
    let hoppers = blocks.iter().any(|(_, b)| matches!(b, Block::Hopper { .. }));
    let mut seen: HashMap<u128, u32> = HashMap::new();
    for t in 0..=horizon {
        if !w.errors.is_empty() || !w.drops.is_empty() {
            return None;
        }
        if !hoppers && w.idle() && !w.sculk.busy() {
            return None;
        }
        let d = digest(&w);
        if let Some(&t0) = seen.get(&d) {
            return if t - t0 > 1 { Some((t0, t - t0)) } else { None };
        }
        seen.insert(d, t);
        w.tick();
    }
    None
}

/// One period replayed on redsim for a picture: (whole-state warmup W, the clock's period P, for
/// each tick W..W+P the number of changing cells that are ON and the number of moving blocks: a
/// moving block is invisible in a still picture). `None` unless redsim says CLOCK.
pub fn activity(c: &Clock, o: &SimOptions) -> Option<(u32, u32, Vec<(u32, u32)>)> {
    let r = simulate(c, o);
    if r.verdict != Verdict::Clock {
        return None;
    }
    let blocks = parse_blocks(c).ok()?;
    let mut s = start(c, &blocks, o.margin).ok()?;
    let w0 = r.state_warmup.max(r.warmup);
    let cells: Vec<BlockPos> = r.analysis.cells.iter().map(|w| bp(w.pos)).collect();
    let mut counts = Vec::new();
    for t in 0..(w0 + r.period) {
        between_ticks(&mut s, t).ok()?;
        if t >= w0 {
            let on = cells.iter().filter(|p| wave::active(&s.world.get(**p).to_string())).count() as u32;
            let moving = cells.iter().filter(|p| matches!(s.world.get(**p), Block::MovingPiston { .. })).count() as u32;
            counts.push((on, moving));
        }
        s.world.tick();
    }
    Some((w0, r.period, counts))
}

/// The real-game judge's horizon and minimum periods (`verify::VerifyOptions`), used for designs
/// with entities.
pub const ENTITY_HORIZON: u32 = 4000;
const ENTITY_MIN_PERIODS: usize = 3;
/// A design with item entities runs past their lifetime (`ItemEntity.tick`: age >= 6000 discards
/// it): a clock must run forever (vjeux 2026-10-20, kit-bouncer-item-p887-16b: "items despawn
/// after 5 minutes"), so its period must still hold over the second half of 12000 gt.
pub const ITEM_HORIZON: u32 = 12_000;

/// The horizon of a design's entity run: [`ITEM_HORIZON`] with item entities, else
/// [`ENTITY_HORIZON`].
pub fn entity_horizon(c: &Clock) -> u32 {
    if c.design.entities.iter().any(|e| e.kind.contains("item")) { ITEM_HORIZON } else { ENTITY_HORIZON }
}

/// (warmup, period) of a design with entities by the real-game judge's rule: the block frames
/// (every cell of the world box: block state + block-entity extras; entities left out) over
/// 0..=[`ENTITY_HORIZON`], `wave::detect_period` from the first free tick with 3 periods, and the
/// period must hold over at least the second half of the horizon (warmup <= horizon / 2): an
/// entity that never repeats exactly (a drifting item) can show a short block pattern near the
/// end of any window (kit-bouncer-item-p12-20b: "period 12 from 3965" here, 13 from 4074 in the
/// game; clk-misc, 2026-10-20). `None`: not periodic.
fn entity_clock_period(c: &Clock, blocks: &[(BlockPos, Block)], o: &SimOptions, end: u32) -> Result<Option<(u32, u32)>, (Verdict, String)> {
    let mut s = start(c, blocks, o.margin)?;
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<BlockPos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| BlockPos::new(wmin.x + x, wmin.y + y, wmin.z + z))))
        .collect();
    let mut ids: HashMap<u128, u64> = HashMap::new();
    let horizon = entity_horizon(c);
    let mut frames: Vec<u64> = Vec::with_capacity(horizon as usize + 1);
    for t in 0..=horizon {
        between_ticks(&mut s, t)?;
        use std::hash::Hash;
        let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
        for q in &cells {
            let b = s.world.get(*q);
            b.hash(&mut h);
            wave::be_extra_redsim(&s.world, *q, &b).hash(&mut h);
        }
        let d = ((h.a as u128) << 64) | h.b as u128;
        let n = ids.len() as u64;
        frames.push(*ids.entry(d).or_insert(n));
        if t < horizon {
            s.world.tick();
        }
    }
    Ok(wave::detect_period(&frames, end as usize, ENTITY_MIN_PERIODS).filter(|(w, _)| *w <= horizon as usize / 2).map(|(w, p)| (w as u32, p as u32)))
}

/// (tickexact) The clock judge's chunk-reload check (`clock verify --reload`) in redsim, for one
/// build (`c` already turned and moved, `from..=to` its watched box): run to `reload_at`, save,
/// unload and load the build's chunks again (the game's sandbox box for that watched box:
/// `redsim::reload`), run on to `horizon`; the frames of every cell (block state + block-entity
/// extras), the judge's `wave::detect_period` and its rule (`verify::run_in`): after the reload
/// the clock runs at `want` gt again within `2 x want + 40` gt. `Err`: why it does not survive.
pub fn reload_check(c: &Clock, from: Pos, to: Pos, reload_at: u32, want: u32, horizon: u32, min_periods: usize) -> Result<(), String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to).map_err(|(_, why)| why)?;
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<BlockPos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| BlockPos::new(wmin.x + x, wmin.y + y, wmin.z + z))))
        .collect();
    let mut ids: HashMap<u128, u64> = HashMap::new();
    let mut frames: Vec<u64> = Vec::with_capacity(horizon as usize + 1);
    for t in 0..=horizon {
        between_ticks(&mut s, t).map_err(|(_, why)| why)?;
        use std::hash::Hash;
        let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
        for q in &cells {
            let b = s.world.get(*q);
            b.hash(&mut h);
            wave::be_extra_redsim(&s.world, *q, &b).hash(&mut h);
        }
        let d = ((h.a as u128) << 64) | h.b as u128;
        let n = ids.len() as u64;
        frames.push(*ids.entry(d).or_insert(n));
        if t == reload_at {
            redsim::reload::reload_chunks(&mut s.world, &redsim::reload::box_chunks(chunks));
            if let Some(e) = s.world.errors.first() {
                return Err(format!("not modeled by redsim: {e}"));
            }
        }
        if t < horizon {
            s.world.tick();
        }
    }
    let end = c.free_from() as usize;
    let Some((w0, p)) = wave::detect_period(&frames, end, min_periods) else {
        return Err(format!("not periodic for {min_periods} periods within {horizon} gt (with a chunk reload)"));
    };
    if (w0 + p) as u32 > reload_at && (p as u32 != want || w0 as u32 > reload_at + 2 * want + 40) {
        return Err(format!("does not survive a chunk reload at tick {reload_at}: it runs at period {p} gt only from tick {w0} (period {want} gt before the reload)"));
    }
    Ok(())
}

/// A quick screen for entity-kit candidates (`clock kit`): run `ticks` gt; false when the design
/// comes to rest (nothing pending and every entity still for 3 ticks in a row), when no cell of the
/// world box changes in the last two thirds of the run (no clock with a period that fits), or on an
/// invalid or unmodeled run.
pub fn entity_screen(c: &Clock, margin: i32, ticks: u32) -> bool {
    let Ok(blocks) = parse_blocks(c) else { return false };
    let Ok(mut s) = start(c, &blocks, margin) else { return false };
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<BlockPos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| BlockPos::new(wmin.x + x, wmin.y + y, wmin.z + z))))
        .collect();
    let digest = |w: &World| -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for q in &cells {
            w.get(*q).hash(&mut h);
        }
        h.finish()
    };
    let mut prev = digest(&s.world);
    let mut changed_late = false;
    let mut still_run = 0;
    for t in 0..ticks {
        if between_ticks(&mut s, t).is_err() {
            return false;
        }
        s.world.tick();
        let d = digest(&s.world);
        if d != prev {
            if t >= ticks / 3 {
                changed_late = true;
            }
            prev = d;
        }
        let still = s.world.idle()
            && s.world.entities.list.iter().all(|e| {
                let v = e.delta_movement;
                v.x == 0.0 && v.y == 0.0 && v.z == 0.0 && e.pos.x == e.xo && e.pos.y == e.yo && e.pos.z == e.zo
            });
        still_run = if still { still_run + 1 } else { 0 };
        if still_run >= 3 {
            return false;
        }
    }
    changed_late && s.world.errors.is_empty() && s.world.drops.is_empty()
}

/// One period of every cell's full state (block state + extra), kept small: each state interned
/// once, the first frame, and only for the cells whose state changes their state at every tick
/// (whole frames of Strings over long periods ran the site build to 82 GB: the coordinator
/// 2026-10-21). Reads as the frames it replaces: [`PeriodStates::get`].
#[derive(Default)]
struct PeriodStates {
    palette: Vec<String>,
    index: HashMap<String, u32>,
    frame0: Vec<u32>,
    prev: Vec<u32>,
    /// The cells whose state changed so far: their state at every tick recorded.
    seqs: HashMap<usize, Vec<u32>>,
    /// The ticks recorded.
    len: usize,
}

impl PeriodStates {
    fn intern(&mut self, s: String) -> u32 {
        if let Some(&i) = self.index.get(&s) {
            return i;
        }
        let i = self.palette.len() as u32;
        self.palette.push(s.clone());
        self.index.insert(s, i);
        i
    }

    /// Record the next tick's frame (one state id per cell); `changed`: the cells whose state may
    /// differ from the previous tick's (every other cell holds its id).
    fn push(&mut self, frame: &[u32], changed: &[usize]) {
        if self.len == 0 {
            self.frame0 = frame.to_vec();
            self.prev = frame.to_vec();
            self.len = 1;
            return;
        }
        for &i in changed {
            if self.prev[i] != frame[i] && !self.seqs.contains_key(&i) {
                // unchanged until now: its first state at every tick so far
                self.seqs.insert(i, vec![self.frame0[i]; self.len]);
            }
        }
        for (i, s) in self.seqs.iter_mut() {
            s.push(frame[*i]);
        }
        for &i in changed {
            self.prev[i] = frame[i];
        }
        self.len += 1;
    }

    /// Cell `i`'s state at tick `t` of the recorded period.
    fn get(&self, t: usize, i: usize) -> &str {
        let id = match self.seqs.get(&i) {
            Some(s) => s[t],
            None => self.frame0[i],
        };
        &self.palette[id as usize]
    }

    /// Whether cell `i`'s state changes in the recorded period.
    fn changes(&self, i: usize) -> bool {
        self.seqs.contains_key(&i)
    }
}

/// Simulate a clock on redsim (exact): see the module doc.
pub fn simulate(c: &Clock, o: &SimOptions) -> SimReport {
    let blocks = match parse_blocks(c) {
        Ok(b) => b,
        Err(e) => return SimReport::fail(c, Verdict::Unmodeled, format!("not modeled by redsim: {e}")),
    };
    let end = c.free_from();
    // Designs with entities: their exact state never repeats (an entity's doubles drift in the
    // low bits), so the period comes from the BLOCKS, by the real-game judge's own rule
    // (`verify::run_in`): the frames of every cell (block state + block-entity extras, entities
    // left out) must repeat with period p for 3 periods within its 4000 gt horizon; the smallest
    // warmup, then the smallest p (`wave::detect_period`).
    let entity_period = if c.design.entities.is_empty() {
        None
    } else {
        match entity_clock_period(c, &blocks, o, end) {
            Ok(Some(wp)) => Some(wp),
            Ok(None) => return SimReport::fail(c, Verdict::NoRepeat, format!("not periodic for 3 periods over the second half of {} gt (the judge's rule for designs with entities; items despawn at 6000 gt)", entity_horizon(c))),
            Err((v, why)) => return SimReport::fail(c, v, why),
        }
    };
    // Phase 1: the first repeat of the full state.
    let mut s = match start(c, &blocks, o.margin) {
        Ok(s) => s,
        Err((v, why)) => return SimReport::fail(c, v, why),
    };
    let mut seen: HashMap<u128, u32> = HashMap::new();
    let mut found = entity_period;
    for t in 0..=o.horizon {
        if found.is_some() {
            break;
        }
        if let Err((v, why)) = between_ticks(&mut s, t) {
            return SimReport::fail(c, v, why);
        }
        if t >= end {
            let d = digest(&s.world);
            if let Some(&t0) = seen.get(&d) {
                found = Some((t0, t - t0));
                break;
            }
            seen.insert(d, t);
        }
        s.world.tick();
    }
    let Some((w0, p)) = found else {
        return SimReport::fail(c, Verdict::NoRepeat, format!("no repeat of its state within {} gt", o.horizon));
    };
    // Phase 2: replay to W + P, confirm the repeat on the exact key, record one period.
    let (lo, hi) = c.bounds();
    let out_pos = c.output;
    let lamp = judge_lamp(c);
    // The cells read at a tick: every cell at the first tick, at W, at the last and every
    // `TRACK_CHECK` ticks; at the others only the cells the world's write log names
    // (`World::log_writes`: every cell a write changed) and the cells whose extras change with no
    // write (comparators, containers: `wave::reads_extras`). A cell's state can change no other
    // way, and a full read that finds a cell changed that neither named runs the record again
    // reading every cell at every tick. (The long members read 4,400 cells a tick to see the few
    // dozen that change: clk-rt-counter n = 3, 312,816 gt a run, 2026-11-08.)
    let mut tracked = true;
    let (slo, shi, cells, ps, mut out_seq, mut out_level, out_entered, ent_cells, frames) = 'record: loop {
        let mut s = start(c, &blocks, o.margin).ok().unwrap();
        s.world.log_writes = tracked;
        s.world.writes.clear();
        let (mut slo, mut shi) = (lo, hi);
        let wmin = s.world.min;
        let size = s.world.size;
        let cells: Vec<Pos> = (0..size[1])
            .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| [wmin.x + x, wmin.y + y, wmin.z + z])))
            .collect();
        let n = cells.len();
        let index_of = |q: BlockPos| -> Option<usize> {
            let (x, y, z) = (q.x - wmin.x, q.y - wmin.y, q.z - wmin.z);
            (x >= 0 && y >= 0 && z >= 0 && x < size[0] && y < size[1] && z < size[2]).then(|| ((y * size[2] + z) * size[0] + x) as usize)
        };
        let every: Vec<usize> = (0..n).collect();
        // one period of every cell's state, kept small (the coordinator 2026-10-21: whole frames of
        // Strings ran the site build's family-member pass to 82 GB): each state interned once, the
        // frame at W, and the state at every tick of the cells that change
        let mut ps = PeriodStates::default();
        let mut out_seq: Vec<bool> = Vec::with_capacity(p as usize);
        let mut out_level: Vec<i32> = Vec::with_capacity(p as usize);
        let mut key_w = Vec::new();
        // the declared output cell's first block in the period, and how it was entered if it was
        let mut out_first: Option<String> = None;
        let mut out_entered: Option<String> = None;
        let mut ent_cells: std::collections::HashSet<Pos> = std::collections::HashSet::new();
        // a digest of the full cell states after every tick (block state + comparator strength +
        // container contents): the clock's warmup and period
        let mut frames: Vec<u128> = Vec::with_capacity((w0 + p + 1) as usize);
        // each cell's state as last read (its block and block-entity extras), its share of the
        // frame digest and, while recording, its interned state: an unchanged cell interns to the
        // same id, so reusing it changes nothing; the frame digest is the sum of the cells' keyed
        // shares ([`cell_share`]), which equal frames share and other frames do not but by a
        // 2^-128 chance: frames are only compared with each other
        let mut cur: Vec<Option<(Block, Option<String>, Option<u8>)>> = vec![None; n];
        let mut shares: Vec<u128> = vec![0; n];
        let mut ids: Vec<Option<u32>> = vec![None; n];
        let mut digest_sum: u128 = 0;
        // every cell's id while recording (the cells not read hold theirs)
        let mut frame: Vec<u32> = vec![0; n];
        // the cells whose extras are read at every tick, the cells named this tick, the cells read
        // whose state changed
        let mut extras: Vec<usize> = Vec::new();
        let mut is_extras = vec![false; n];
        let mut named = vec![false; n];
        let mut todo: Vec<usize> = Vec::new();
        let mut changed: Vec<usize> = Vec::new();
        for t in 0..=(w0 + p) {
            if let Err((v, why)) = between_ticks(&mut s, t) {
                return SimReport::fail(c, v, why);
            }
            let recording = t >= w0 && t < w0 + p;
            let full = !tracked || t == 0 || t == w0 || t == w0 + p || t % TRACK_CHECK == 0;
            todo.clear();
            if tracked {
                let mut log = std::mem::take(&mut s.world.writes);
                for q in log.drain(..) {
                    if let Some(k) = index_of(q)
                        && !named[k]
                    {
                        named[k] = true;
                        todo.push(k);
                    }
                }
                // (the log's buffer back, empty)
                s.world.writes = log;
                for &k in &extras {
                    if !named[k] {
                        named[k] = true;
                        todo.push(k);
                    }
                }
                todo.sort_unstable();
            }
            changed.clear();
            let mut missed = false;
            let mut extras_moved = false;
            // (in the box's own order: the swept box grows over its blocks here, the lamp aside)
            for &k in if full { &every } else { &todo } {
                let q = cells[k];
                let b = s.world.get(bp(q));
                if !b.is_air() && Some(q) != lamp {
                    crate::grow(&mut slo, &mut shi, q);
                }
                // a comparator's extras are its strength alone (`wave::comparator_read`), a block
                // without extras has none (`wave::reads_extras`): the same block holding the same
                // reads again changes nothing, and its text is not made again
                let quick = wave::comparator_read(&s.world, bp(q), &b);
                let held = match &cur[k] {
                    Some((cb, _, cq)) if *cb == b => {
                        if quick.is_some() {
                            *cq == quick
                        } else {
                            !wave::reads_extras(&b)
                        }
                    }
                    _ => false,
                };
                if !held {
                    let extra = wave::be_extra_redsim(&s.world, bp(q), &b);
                    if !matches!(&cur[k], Some((cb, ce, _)) if *cb == b && *ce == extra) {
                        // a change neither the write log nor the extras named
                        if tracked && t > 0 && !named[k] {
                            missed = true;
                        }
                        let share = cell_share(k, &b, &extra);
                        digest_sum = digest_sum.wrapping_sub(shares[k]).wrapping_add(share);
                        shares[k] = share;
                        ids[k] = None;
                        changed.push(k);
                    }
                    cur[k] = Some((b, extra, quick));
                }
                let reads = wave::reads_extras(&b);
                if reads != is_extras[k] {
                    is_extras[k] = reads;
                    extras_moved = true;
                }
                if recording {
                    let id = match ids[k] {
                        Some(id) => id,
                        None => {
                            let (cb, ce, _) = cur[k].as_ref().expect("read above");
                            let id = ps.intern(wave::with_extra(&cb.to_string(), ce.as_deref()));
                            ids[k] = Some(id);
                            id
                        }
                    };
                    frame[k] = id;
                }
            }
            if missed {
                tracked = false;
                continue 'record;
            }
            for &k in &todo {
                named[k] = false;
            }
            if extras_moved {
                extras = (0..n).filter(|&k| is_extras[k]).collect();
            }
            frames.push(digest_sum);
            if t == w0 {
                key_w = s.world.state_key();
            }
            if recording {
                ps.push(&frame, if full { &every } else { &changed });
                if let Some(op) = out_pos {
                    let st = s.world.get(bp(op));
                    let level = s.world.best_neighbor_signal(bp(op));
                    // at the judge's lamp: the power delivered into the cell (what dust or a repeater
                    // there would get; coordinator 2026-10-20: the lamp's 4 gt off-delay is the
                    // lamp's limit, not the clock's)
                    let on = if lamp.is_none() && self_reporting(&st.to_string()) { wave::active(&st.to_string()) } else { level > 0 };
                    out_seq.push(on);
                    out_level.push(level);
                    // the output cell stays free all cycle (vjeux 2026-10-20): the same block in it
                    // every tick, never a moving block or a head, no entity in it
                    if out_entered.is_none() {
                        let text = st.to_string();
                        let name = mctas::blockstate::block_name(&text).to_string();
                        let first = out_first.get_or_insert_with(|| name.clone()).clone();
                        if name.contains("moving_piston") || name.contains("piston_head") || name != first {
                            out_entered = Some(format!("{} at tick {t}", crate::short(&text)));
                        }
                        let cell = (op[0] as f64, op[1] as f64, op[2] as f64);
                        if let Some(e) = s.world.entities.list.iter().find(|e| {
                            !e.removed && e.bb.max_x > cell.0 && e.bb.min_x < cell.0 + 1.0 && e.bb.max_y > cell.1 && e.bb.min_y < cell.1 + 1.0 && e.bb.max_z > cell.2 && e.bb.min_z < cell.2 + 1.0
                        }) {
                            out_entered = Some(format!("{:?} entity at tick {t}", e.kind));
                        }
                    }
                }
                // the cells an entity occupies during the period (never the output: see above)
                for e in s.world.entities.list.iter().filter(|e| !e.removed) {
                    for x in e.bb.min_x.floor() as i32..=(e.bb.max_x - 1e-9).floor() as i32 {
                        for y in e.bb.min_y.floor() as i32..=(e.bb.max_y - 1e-9).floor() as i32 {
                            for z in e.bb.min_z.floor() as i32..=(e.bb.max_z - 1e-9).floor() as i32 {
                                ent_cells.insert([x, y, z]);
                            }
                        }
                    }
                }
            }
            if t == w0 + p {
                if entity_period.is_none() && s.world.state_key() != key_w {
                    return SimReport::fail(c, Verdict::Unmodeled, format!("digest collision: states at {w0} and {} differ (report this)", w0 + p));
                }
                break;
            }
            s.world.tick();
        }
        break (slo, shi, cells, ps, out_seq, out_level, out_entered, ent_cells, frames);
    };
    // The clock's period F: the smallest divisor of the whole-state period over which every cell's
    // full state (block state + comparator strength + container contents) repeats; its warmup: the
    // first tick from which it does (what the real-game judge measures). Pending ticks and the
    // torch memory may cycle slower (`state_period`: their order among ticks due together is not
    // physics).
    let (w0u, pu) = (w0 as usize, p as usize);
    let full_p = (1..=pu).filter(|d| pu % d == 0).find(|&d| (w0u..w0u + pu).all(|t| frames[t] == frames[w0u + (t - w0u + d) % pu])).unwrap_or(pu);
    let mut vis_w = w0u;
    while vis_w > c.free_from() as usize && frames[vis_w - 1] == frames[vis_w - 1 + full_p] {
        vis_w -= 1;
    }
    out_seq.truncate(full_p);
    out_level.truncate(full_p);
    // the judge's lamp is no part of the clock: left out of its cells and family
    let mut analysis = match lamp.and_then(|l| cells.iter().position(|q| *q == l)) {
        Some(li) => {
            let cells2: Vec<Pos> = cells.iter().enumerate().filter(|(i, _)| *i != li).map(|(_, q)| *q).collect();
            let orig = |j: usize| if j < li { j } else { j + 1 };
            wave::analyze_with(&cells2, full_p, |t, j| ps.get(t, orig(j)), |j| ps.changes(orig(j)))
        }
        None => wave::analyze_with(&cells, full_p, |t, i| ps.get(t, i), |i| ps.changes(i)),
    };
    // the design's entities (a minecart riding its rails) are part of the mechanism: their kinds
    // join the family, as in the real-game judge (`detector_rail+minecart`)
    for e in &c.design.entities {
        let k = crate::short(&e.kind).to_string();
        if !analysis.family.contains(&k) {
            analysis.family.push(k);
        }
    }
    analysis.family.sort();
    let mut rep = SimReport {
        name: c.name().to_string(),
        verdict: Verdict::Clock,
        why: String::new(),
        warmup: vis_w as u32,
        state_warmup: w0,
        period: full_p as u32,
        state_period: p,
        analysis,
        output: None,
        blocks: c.blocks(),
        swept: (slo, shi),
        start: c.start_label(),
        occupied: {
            let mut v: Vec<Pos> = ent_cells.iter().copied().collect();
            v.sort();
            v
        },
        vibration: None,
    };
    if rep.analysis.cells.is_empty() {
        rep.verdict = Verdict::Stops;
        rep.why = if p == 1 { format!("goes still at tick {w0}") } else { format!("no block changes in its period (the state repeats every {p} gt from tick {w0})") };
        return rep;
    }
    // The output: `port output`, else the changing cell with the longest own period (then the most
    // edges); its own period must be the clock's (coordinator's ruling 2026-10-19: else the design
    // is independent sub-clocks side by side: COMPOSITE).
    let cell_period = |pos: Pos| -> usize {
        match cells.iter().position(|q| *q == pos) {
            Some(i) => wave::cyclic_period(&(0..full_p).map(|t| ps.get(t, i)).collect::<Vec<_>>()),
            None => full_p,
        }
    };
    let (output, out_period) = match out_pos {
        Some(op) => {
            let (on, edges, widths) = wave::bool_wave(&out_seq);
            let gaps = wave::rise_gaps(&out_seq);
            let states = rep.analysis.cells.iter().find(|w| w.pos == op).map(|w| w.states.clone()).unwrap_or_default();
            // a component reports its own state; a plain block or air: the power it receives
            let own = if states.is_empty() { wave::cyclic_period(&out_level) } else { cell_period(op) };
            (Some(Output { pos: op, auto: false, wave: CellWave { pos: op, states, kinds: Vec::new(), on, edges, widths, gaps } }), own)
        }
        None => {
            // (no declared output: the changing cell with the longest own period, for the census
            // and reports; the judge itself picks an exterior air cell, `lamp_outputs`)
            let best = rep.analysis.cells.iter().max_by(|a, b| (cell_period(a.pos), a.edges, a.on).cmp(&(cell_period(b.pos), b.edges, b.on)).then(b.pos.cmp(&a.pos)));
            match best {
                Some(w) => (Some(Output { pos: w.pos, auto: true, wave: w.clone() }), cell_period(w.pos)),
                None => (None, full_p),
            }
        }
    };
    rep.output = output;
    // the output cell stays free all cycle (vjeux 2026-10-20, clk-misc-leaf-p10-4b: "not okay to
    // have the out slot have something pushed into it during the cycle")
    if let Some(how) = out_entered {
        rep.verdict = Verdict::Invalid;
        rep.why = format!("the output cell {:?} is entered during the cycle ({how}): it must hold the same block all cycle, nothing moved into or through it, no entity in it (`clock retap` moves the output to a free cell that pulses evenly)", out_pos.unwrap_or([0, 0, 0]));
        return rep;
    }
    // ... and where a builder can take the signal: an exterior air cell (vjeux 2026-10-20)
    if let Some(op) = out_pos {
        let design_block = if lamp.is_some() { "minecraft:air".to_string() } else { c.design.blocks.get(&op).cloned().unwrap_or_else(|| "minecraft:air".into()) };
        let first = &design_block;
        let below: Vec<&str> = match cells.iter().position(|q| *q == [op[0], op[1] - 1, op[2]]) {
            Some(i) => (0..ps.len).map(|t| ps.get(t, i)).collect(),
            None => Vec::new(),
        };
        if let Some(why) = placement_problem(op, rep.swept, first, below.into_iter()) {
            rep.verdict = Verdict::Invalid;
            rep.why = format!("{why} (`clock retap` moves the output to the best exterior cell)");
            return rep;
        }
    }
    // Composite (coordinator's ruling 2026-10-19): the output repeats faster than the whole
    // structure AND the changing cells split into independent groups (sub-clocks side by side).
    // One mechanism whose output fires twice per cycle (an observer watching both strokes of a
    // piston) is a normal clock of the whole cycle. (A declared output that never changes says
    // nothing about the structure: not judged here.)
    let groups = wave::spatial_groups(&rep.analysis.cells.iter().map(|w| w.pos).collect::<Vec<_>>(), &wave::sensors_of(&c.design.blocks));
    if out_period < full_p && out_period > 1 && groups > 1 {
        rep.verdict = Verdict::Composite;
        let pos = rep.output.as_ref().map(|o| o.pos).unwrap_or([0, 0, 0]);
        rep.why = format!(
            "composite: the output {pos:?} repeats every {out_period} gt but the whole structure every {full_p} gt, and its changing cells form {groups} independent groups (sub-clocks side by side: the period would be their lcm); keep one sub-clock, or declare the combining cell as `port output` (a beat clock, marked `# composite`)"
        );
    }
    // a sculk clock: does it run the same with footsteps and blocks around it? (a property, never
    // a failure: crate::vibration)
    if rep.verdict == Verdict::Clock {
        rep.vibration = crate::vibration::sim_property(c, o.margin, rep.warmup, rep.period, rep.swept, rep.output.as_ref().map(|o| o.pos));
    }
    rep
}

#[cfg(test)]
mod shape_tests {
    use super::*;

    /// Two fences side by side stored as posts (tnt-precise-drop's, vjeux 2026-10-21: "why don't
    /// the fences connect?"): a player's placement joins them; stored joined, nothing to refuse; a
    /// lone fence stays a post.
    #[test]
    fn fences_store_what_placement_gives() {
        let post = "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]";
        let text = format!("name: t\norigin: 0 0 0\npalette:\n  F {post}\n  S minecraft:stone\nlayer 0:\n  SSS.S\nlayer 1:\n  .FF.F\n");
        let c = Clock::from_design(mctas::design::Design::parse(&text).unwrap()).unwrap();
        let bad = shape_mismatches(&c).unwrap();
        assert_eq!(bad.iter().map(|(p, _, placed)| (*p, placed.contains("east=true"), placed.contains("west=true"))).collect::<Vec<_>>(), vec![([1, 1, 0], true, false), ([2, 1, 0], false, true)]);
        let fixed = with_placed_shapes(&text, &c).unwrap().unwrap();
        let c2 = Clock::from_design(mctas::design::Design::parse(&fixed).unwrap()).unwrap();
        assert!(shape_mismatches(&c2).unwrap().is_empty(), "{fixed}");
        assert_eq!(c2.design.blocks[&[4, 1, 0]], post, "the lone fence stays a post");
    }
}
