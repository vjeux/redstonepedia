//! A contraption running on redsim: placed exactly like its judge (the door judge's placement
//! for doors, the clock judge's placement and start kick for clocks), stepped one game tick at a
//! time, with the player's inputs (lever flips, button presses) applied between ticks.
//!
//! The clock logic mirrors `crates/clock/src/sim.rs` (`start`, `between_ticks`, the period
//! detector) and the door logic `crates/piston-door/src/cosearch.rs` (`place_and_settle`), so the
//! browser shows what the judges measured; `mctas web check` compares the two natively and on the
//! shipped wasm.

use crate::design::{Design, Pos, parse_travel};
use redsim::piston::MovingBe;
use redsim::world::flags;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::HashMap;
use std::hash::Hasher;

/// The clock judge's one-time start kick (`port start: x y z <kick> [at N]`).
#[derive(Clone, Debug, PartialEq)]
pub enum Kick {
    Last { pos: Pos },
    Use { pos: Pos, again: Option<u32> },
    Place { pos: Pos, state: String, ticks: u32 },
}

/// One input applied between ticks.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Place { pos: Pos, block: Block, nbt: Option<String> },
    Use { pos: Pos },
    Clear { pos: Pos },
    /// Change the block with shape updates only (flags 2: the BUD judge).
    ShapeSet { pos: Pos, block: Block },
    /// A pure block update around the cell (`Level.updateNeighborsAt`: the BUD judge).
    Update { pos: Pos },
}

/// What kind of contraption a design is (from its ports).
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// A piston door: `port door` (bottom corner of the opening) and `port lever`; a 3x3 opening,
    /// or 2x2 with `door-size: 2` (clk-item's 2x2 doors, designs/piston-door-2x2/).
    Door { door: Pos, lever: Pos, along_z: bool, size: i32 },
    /// Anything that runs by itself (clocks; plain designs): optional output and start kick.
    Clock { output: Option<Pos>, kick: Option<(Kick, u32)>, player_placement: bool },
}

/// Knobs of [`Session::new`].
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Free cells around the design in the simulated world (the clock judge uses 4).
    pub margin: i32,
    /// Ticks a door may take to settle after placement.
    pub settle: u32,
    /// A TNT duper's ground reach (None: the reach its judge would grow the ground to,
    /// [`Session::with_ground`]).
    pub ground_reach: Option<i32>,
    /// Run the design. False: a still (vjeux 2026-10-21, "Why is it simulating everything?"; the
    /// coordinator: cards never simulate, a page's view simulates only when played or pressed):
    /// the design as the judge places it (raw), its judge's chests and crop, its own entities
    /// and a TNT duper's ground; nothing ticks ([`Session::still`]).
    pub simulate: bool,
    /// The look-aheads at construction (the native tools'): a shulker, sorter, smelter, brewer
    /// or harvester run probed [`SHULKER_PROBE`] ticks for something redsim does not model, a
    /// flying machine run to its end ([`Session::fly_end`]), a TNT duper's ground grown
    /// ([`Session::with_ground`]). False in the browser: the view meets an unmodeled tick when it
    /// gets there and a flying machine's end as it flies ([`Session::fly_end_due`]), and a TNT
    /// duper stands on its `ground_reach` (or [`GROUND_REACH`]).
    pub probe: bool,
    /// Run a kind whose judge's run the view does not replay exactly yet
    /// ([`crate::circuit::not_replayed_yet`]: shown as built otherwise): its tests and the work
    /// that makes it exact again.
    pub unverified: bool,
    /// A harvester's random tick speed (`circuit::farm::game_rts`, the judge's for its runs: the
    /// page's `data-farm-rts`); None: [`crate::circuit::farm_rts_for`].
    pub farm_rts: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Options { margin: 4, settle: 400, ground_reach: None, simulate: true, probe: true, unverified: false, farm_rts: None }
    }
}

impl Options {
    /// The browser's view of a design before it plays: a still ([`Options::simulate`] false).
    pub fn still() -> Options {
        Options { simulate: false, probe: false, ..Options::default() }
    }

    /// The browser's live view once it plays: the judge's seeding, no look-ahead
    /// ([`Options::probe`] false).
    pub fn live() -> Options {
        Options { probe: false, ..Options::default() }
    }
}

/// A TNT duper's ground (`Circuit::ground_blocks`, the judge's: grass over two dirt over
/// bedrock), real blocks of the simulated world: `lo..=hi` (x, y, z), its top layer at `hi[1]`.
#[derive(Clone, Debug)]
pub struct Floor {
    pub lo: Pos,
    pub hi: Pos,
    /// Its layers, top first: the block of each and its name.
    pub layers: Vec<(Block, String)>,
    /// How far it reaches around the design's box, and that box (the design as written).
    pub reach: i32,
    pub design_box: (Pos, Pos),
    /// Its cells that no longer hold their layer's block (a crater, something fallen in) and what
    /// they hold now; kept up to date from the world's write log after every tick
    /// ([`Session::take_writes`]).
    pub changed: std::collections::BTreeMap<Pos, Block>,
}

impl Floor {
    /// The judge's ground layers (`Ground::Grass`), top first.
    pub const LAYERS: [&'static str; 4] = ["minecraft:grass_block", "minecraft:dirt", "minecraft:dirt", "minecraft:bedrock"];

    /// The ground `reach` blocks around the design box `lo..=hi`, its top `drop` air blocks under
    /// the design's lowest layer (`Circuit::ground_blocks`).
    pub fn grass(lo: Pos, hi: Pos, reach: i32, drop: i32) -> Result<Floor, String> {
        let layers = Floor::LAYERS.iter().map(|s| Block::parse(s).map(|b| (b, b.to_string())).map_err(|e| format!("ground {s}: {e}"))).collect::<Result<Vec<_>, _>>()?;
        let top = lo[1] - 1 - drop;
        let n = layers.len() as i32;
        Ok(Floor { lo: [lo[0] - reach, top - n + 1, lo[2] - reach], hi: [hi[0] + reach, top, hi[2] + reach], layers, reach, design_box: (lo, hi), changed: Default::default() })
    }

    pub fn contains(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }

    /// The layer's block of a cell at height `y` (in the ground).
    pub fn layer(&self, y: i32) -> &(Block, String) {
        &self.layers[((self.hi[1] - y) as usize).min(self.layers.len() - 1)]
    }

    /// Every cell of the ground as laid (before anything runs).
    pub fn blocks(&self) -> Vec<(BlockPos, Block)> {
        let mut v = Vec::new();
        for y in self.lo[1]..=self.hi[1] {
            let b = self.layer(y).0;
            for x in self.lo[0]..=self.hi[0] {
                for z in self.lo[2]..=self.hi[2] {
                    v.push((BlockPos::new(x, y, z), b));
                }
            }
        }
        v
    }

    /// What a cell of the ground holds now (its layer's block unless it changed).
    pub fn now(&self, p: Pos) -> Block {
        match self.changed.get(&p) {
            Some(b) => *b,
            None => self.layer(p[1]).0,
        }
    }

    /// Whether a cell of the ground changed (a crater, something fallen in).
    pub fn is_changed(&self, p: Pos) -> bool {
        self.changed.contains_key(&p)
    }

    /// The state drawn at a cell of the ground (None: air).
    pub fn shown(&self, p: Pos) -> Option<String> {
        let b = self.now(p);
        if b == Block::Air {
            None
        } else if b == self.layer(p[1]).0 {
            Some(self.layer(p[1]).1.clone())
        } else {
            Some(b.to_string())
        }
    }
}

/// The reach a TNT duper's judge lays its ground at first, the widest it grows it to (it doubles
/// the reach while a burning TNT leaves the ground: `circuit::run::{GROUND_REACH,
/// MAX_GROUND_REACH}`), and the ticks the viewer runs to choose it.
pub const GROUND_REACH: i32 = 24;
pub const MAX_GROUND_REACH: i32 = 96;
pub const GROUND_PROBE: u32 = 600;

/// How far a thumbnail's still frame search ([`Session::go_to_still`]) looks for a clock's
/// period (ticks): the viewer's and the site's build-time `data-still` alike.
pub const STILL_HORIZON: u32 = 20_000;

/// A TNT duper with its explosions (`Circuit::explodes`): it stands on its judge's ground.
pub fn ground_duper(d: &Design) -> bool {
    let kind = d.meta.get("circuit").and_then(|c| c.split_whitespace().next().map(str::to_string)).unwrap_or_default();
    (kind == "tnt-duper" || kind == "tnt-duper-triggered") && d.meta.get("explode").is_none_or(|v| v.trim() != "off")
}

/// The safe drop an archived TNT duper's judge found (`# CIRCUIT: ... safe drop N blocks`: the
/// air blocks between its lowest layer and the ground); 0 (on the ground) when not written.
pub fn safe_drop(text: &str) -> i32 {
    text.lines()
        .filter(|l| l.starts_with("# CIRCUIT:"))
        .find_map(|l| l.split("safe drop ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse::<i32>().ok()))
        .unwrap_or(0)
        .max(0)
}

/// A TNT duper's declared stand (`stand: N`, a precise drop's usage height: its TNT fall straight
/// down and go off in mid-air below it; `circuit::robust::declared_stand`); 0 when absent.
pub fn declared_stand(d: &Design) -> i32 {
    d.meta.get("stand").and_then(|v| v.trim().parse().ok()).unwrap_or(0).max(0)
}

/// How far a TNT duper stands above its judge's ground, as its judge judges it
/// (`circuit::verify`: `c.drop = safe.max(declared_stand)`): its declared stand when higher than
/// its safe drop (vjeux 04:41Z on the precise drop: "Please make the height of the gap at least
/// the size of the drop before explosion"; its TNT landed before going off on a ground at the
/// safe drop), else the safe drop (every other duper keeps its ground: vjeux 04:42Z, "For
/// horizontal this is fine").
pub fn ground_drop(d: &Design, text: &str) -> i32 {
    safe_drop(text).max(declared_stand(d))
}

/// The mid-air burst point an archived TNT duper's judge found, in blocks below the machine
/// (`# CIRCUIT: ... precise: ... in mid-air X blocks below the machine`), if it says one.
pub fn burst_below(text: &str) -> Option<f32> {
    text.lines().filter(|l| l.starts_with("# CIRCUIT:")).find_map(|l| {
        let r = l.split("in mid-air ").nth(1)?;
        let i = r.find(" blocks below the machine")?;
        r[..i].parse::<f32>().ok()
    })
}

/// Whether an archived TNT duper's judge found its TNT going off in mid-air (`precise: ... in
/// mid-air`): none of them may touch its ground first.
pub fn bursts_in_mid_air(text: &str) -> bool {
    text.lines().any(|l| l.starts_with("# CIRCUIT:") && l.contains(" in mid-air"))
}

/// One event of the farm judge's run (`circuit::farm`'s run loop, between its batches).
#[derive(Clone, Debug, PartialEq)]
pub enum FarmEvent {
    /// `gamerule random_tick_speed n` (the judge stops growth at the run's end).
    Speed(u32),
    /// The bridge's `summon` of an intruder (`minecraft:item` with this data, at rest): its own
    /// random seeded by its summon number (`redsim::entity::spawn`).
    Drop { at: [f64; 3], nbt: String },
    /// The judge empties its chests (`circuit::farm::EMPTY`): each that holds a crop item or an
    /// intruder is cleared.
    Empty { chests: Vec<Pos>, crop_item: String },
    /// After the run and rest, growth resumes for a batch while a crop item lies on a crop
    /// (`circuit::farm::sim_finish`): `resumed` of at most `cap` gt so far, at `rts`.
    Regrow { resumed: u32, cap: u32, rts: u32, crop: String },
}

impl FarmEvent {
    /// The order of a tick's events, the judge's: the chests emptied at the end of the batch
    /// before, then the speed and the drops at the start of the next, then the regrowth check.
    fn rank(&self) -> u8 {
        match self {
            FarmEvent::Empty { .. } => 0,
            FarmEvent::Speed(_) => 1,
            FarmEvent::Drop { .. } => 2,
            FarmEvent::Regrow { .. } => 3,
        }
    }
}

pub struct Session {
    pub design: Design,
    pub mode: Mode,
    pub world: World,
    /// Ticks run since placement (a door's settling ticks included).
    pub tick: u32,
    /// The tick the door came to rest after placement (0 for clocks).
    pub rest_tick: u32,
    /// The last input's tick (a lever flip), if any.
    pub last_input: Option<u32>,
    actions: Vec<(u32, Action)>,
    /// Why the design is shown but not simulated (a block or entity redsim does not model).
    pub unsimulated: Option<String>,
    /// Plain blocks simulated as stone (redsim models one stone for every quartz-like block):
    /// their own names, for the renderer, while the cell still holds that stone.
    pub overlay: HashMap<Pos, String>,
    /// Cells a click toggles (levers, buttons, the input ports).
    pub inputs: Vec<Pos>,
    /// Output ports (the orange marker).
    pub outputs: Vec<Pos>,
    /// Everything as placed, for [`Session::reset`].
    initial: Option<(World, Vec<(u32, Action)>, u32)>,
    /// A real-game recording played instead of simulating ([`Session::attach_trace`]).
    pub replay: Option<crate::replay::Replay>,
    /// The box a travelling design runs in (a flying machine: `circuit fly sim`'s corridor).
    pub corridor: Option<(Pos, Pos)>,
    /// A flying machine: the tick its judge stops at (it reaches the corridor's end, comes to
    /// rest or breaks); the viewer loops there.
    pub end_tick: Option<u32>,
    /// A circuit's inputs (`port input[.<name>]`), driven like its judge drives them.
    pub circuit_inputs: Vec<crate::circuit::CircuitInput>,
    /// Each circuit input's level now.
    pub input_on: Vec<bool>,
    /// The judge's stimulus being replayed (the auto-demo), from placement.
    pub demo: Option<crate::circuit::Schedule>,
    /// Pending input releases (tick, input): a tapped pulse input goes off there.
    releases: Vec<(u32, usize)>,
    /// A clock's picture box (`clock shot`'s frame: the judge's swept box), once known.
    pub picture: Option<(Pos, Pos)>,
    /// The judge's consumer lamps placed in a circuit's air output cells (they show the output).
    pub consumers: Vec<Pos>,
    /// The clock judge's lamp in a clock's empty output cell (`clock::sim::judge_lamp`): the OUT
    /// signal there is the power delivered into the cell, not the lamp's own lit state.
    pub judge_lamp: Option<Pos>,
    /// Where each falling block (by entity id) was before the tick just run: drawn between there
    /// and where it is (redsim keeps no previous position for them).
    prev_falling: Vec<(u32, [f64; 3])>,
    /// redsim met something it does not model during the run (`World::errors`): the view stops
    /// at the last exact state and says so (never animates past it). Cleared by a reset.
    pub halted: Option<String>,
    /// A shulker loader's or unloader's judge chests (redsim's driver: kept full, emptied), an
    /// item sorter's (its source fed from the judge's stream, its sinks emptied), and their state
    /// as placed (for [`Session::reset`]).
    pub shulker: Option<redsim::redstone::shulker::Chests>,
    shulker_initial: Option<redsim::redstone::shulker::Chests>,
    /// The judge's visit is due before the next tick (it follows each tick's frame).
    visit_due: bool,
    /// The judge visits every this many ticks from `judge_from` (a shulker judge every tick, a
    /// sorter's every `circuit::SORTER_SEG`).
    judge_every: u32,
    judge_from: u32,
    /// A harvester's run events (`circuit::farm`'s run, by session tick, applied before that
    /// tick's step like the judge's between batches): the growth stopping at the run's end and
    /// the intruders dropped ([`FarmEvent`]).
    farm: Vec<(u32, FarmEvent)>,
    /// What the judge's visits took out of each chest it empties (a sorter's PASS and OUT, a
    /// shulker design's outputs and rejects, an unloader's item chests), summed by kind since the
    /// start: the run's tally, as the judge counts it (`circuit::sorter`'s `RunLog::reached`).
    pub received: std::collections::BTreeMap<Pos, Vec<crate::labels::Entry>>,
    /// A TNT duper's ground (its judge's: real blocks of the world, drawn from the world); None:
    /// no ground (nothing solid is drawn where redsim has void).
    pub floor: Option<Floor>,
    /// With a ground: the part of the world the machine lives in (the design's box and margin),
    /// where the blocks are drawn, hashed and framed from (the ground is drawn on its own); None:
    /// the whole world box.
    pub machine_box: Option<(BlockPos, [i32; 3])>,
    /// With a ground: the blocks outside the machine's box and the ground (something thrown or
    /// pushed there), followed from the world's write log and drawn with the machine's.
    pub outside: std::collections::BTreeMap<Pos, Block>,
    /// [`Session::period`]'s answer for a horizon (a pure function of the placed state, asked
    /// twice and more by a thumbnail's `go_to_still`: its passes over a long clock's period were
    /// most of a slow thumbnail's time on a phone).
    period_memo: std::cell::RefCell<Option<(u32, Result<Period, String>)>>,
    /// A still ([`Options::simulate`] false): the design as placed, never stepped (the browser's
    /// cards; a page's view until it plays: `Player::wake`).
    pub still: bool,
}

/// The run's mutable state, for the timelapse ([`crate::player::Player::step_back`]): everything
/// [`Session::step`] and its inputs change. The rest (the design, the placed state, the
/// judge's stimulus except as [`Session`]'s `demo` below) is the run's and untouched by a
/// rewind.
#[derive(Clone)]
pub struct RunState {
    pub world: World,
    pub tick: u32,
    pub last_input: Option<u32>,
    pub actions: Vec<(u32, Action)>,
    pub input_on: Vec<bool>,
    pub releases: Vec<(u32, usize)>,
    pub prev_falling: Vec<(u32, [f64; 3])>,
    pub halted: Option<String>,
    pub shulker: Option<redsim::redstone::shulker::Chests>,
    pub received: std::collections::BTreeMap<Pos, Vec<crate::labels::Entry>>,
    pub visit_due: bool,
    pub farm: Vec<(u32, FarmEvent)>,
    pub outside: std::collections::BTreeMap<Pos, Block>,
    pub end_tick: Option<u32>,
    pub replay: Option<crate::replay::Replay>,
    /// The judge's stimulus being replayed (cleared when the person takes over: `stop_demo`).
    pub demo: Option<crate::circuit::Schedule>,
}

/// The corridor of a design with `travel:` (`circuit_fly::machine::Machine::corridor`): its
/// blocks, kick cell and `stop:` blocks swept `distance:` cells (32 by default) along the travel,
/// plus 3 free cells; a piston extender sweeps its N (`circuit: piston-extender N`) + 1.
pub fn corridor_of(d: &Design, mode: &Mode) -> Option<(Pos, Pos)> {
    let ((lo, hi), margin) = travel_box(d, mode)?;
    Some(([lo[0] - margin, lo[1] - margin, lo[2] - margin], [hi[0] + margin, hi[1] + margin, hi[2] + margin]))
}

/// A travelling design's swept box (where it can be, without the free cells) and the corridor's
/// free margin.
pub fn travel_box(d: &Design, mode: &Mode) -> Option<((Pos, Pos), i32)> {
    let v = parse_travel(d.meta.get("travel")?)?;
    let kind = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("flying-machine");
    let (mut lo, mut hi) = d.bounds()?;
    if let Mode::Clock { kick: Some((k, _)), .. } = mode {
        let p = match k {
            Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
        };
        grow(&mut lo, &mut hi, p);
    }
    for (p, _) in &d.stops {
        grow(&mut lo, &mut hi, *p);
    }
    let (k, margin) = if kind.starts_with("flying-machine") {
        (d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(32), 3)
    } else {
        let n = kind.split_whitespace().nth(1).and_then(|w| w.parse::<i32>().ok()).unwrap_or(1);
        (d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(n + 1), 4)
    };
    for i in 0..3 {
        if v[i] > 0 {
            hi[i] += k;
        } else if v[i] < 0 {
            lo[i] -= k;
        }
    }
    Some(((lo, hi), margin))
}

/// Ticks a shulker design is run ahead on load: a run that meets something redsim does not
/// model by then is shown as built.
pub const SHULKER_PROBE: u32 = 1000;

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A design block as redsim state; quartz-like plain blocks (`crates/piston-door/src/properties.rs`)
/// fall back to stone like the door tools do.
fn convert(state: &str) -> Result<(Block, bool), String> {
    match Block::parse(state) {
        Ok(b) => Ok((b, false)),
        Err(e) => {
            if quartz_like(state) {
                Ok((Block::Simple(redsim::block::Simple::Stone), true))
            } else {
                Err(e)
            }
        }
    }
}

/// `piston-door/src/properties.rs::quartz_like`: plain full blocks the door tools treat as stone.
pub fn quartz_like(state: &str) -> bool {
    let n = state.split('[').next().unwrap_or(state);
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    if n.contains("glazed") || n.ends_with("concrete_powder") || n.contains("slab") || n.contains("stairs") || n.contains("wall") {
        return false;
    }
    matches!(
        n,
        "stone" | "smooth_stone" | "cobblestone" | "mossy_cobblestone" | "stone_bricks" | "mossy_stone_bricks" | "cracked_stone_bricks"
            | "chiseled_stone_bricks" | "granite" | "polished_granite" | "diorite" | "polished_diorite" | "andesite" | "polished_andesite"
            | "deepslate" | "cobbled_deepslate" | "polished_deepslate" | "deepslate_bricks" | "deepslate_tiles" | "tuff" | "calcite"
            | "dirt" | "coarse_dirt" | "quartz_block" | "smooth_quartz" | "quartz_bricks" | "chiseled_quartz_block" | "quartz_pillar"
            | "sandstone" | "smooth_sandstone" | "cut_sandstone" | "red_sandstone" | "bricks" | "terracotta" | "netherrack" | "end_stone"
            | "end_stone_bricks" | "blackstone" | "polished_blackstone" | "polished_blackstone_bricks" | "prismarine" | "dark_prismarine"
            | "purpur_block" | "mud_bricks" | "packed_mud" | "bone_block" | "nether_bricks" | "red_nether_bricks" | "basalt" | "smooth_basalt"
            | "polished_basalt" | "amethyst_block" | "lapis_block" | "coal_block" | "iron_block" | "gold_block" | "diamond_block" | "emerald_block"
    ) || n.ends_with("_planks")
        || n.ends_with("_wool")
        || n.ends_with("_concrete")
        || n.ends_with("_terracotta")
        || n.ends_with("_log")
        || n.ends_with("_wood")
        || (n.starts_with("waxed_") && (n.ends_with("copper") || n.ends_with("cut_copper") || n.ends_with("copper_block")))
}

fn parse_kick(pos: Pos, args: &str) -> Result<(Kick, u32), String> {
    let mut words: Vec<&str> = args.split_whitespace().collect();
    let mut at = 0;
    if words.len() >= 2 && words[words.len() - 2] == "at" {
        let n = words[words.len() - 1];
        at = n.parse().map_err(|_| format!("bad kick tick {n:?}"))?;
        words.truncate(words.len() - 2);
    }
    let k = match words.as_slice() {
        ["last"] => Kick::Last { pos },
        ["use"] => Kick::Use { pos, again: None },
        ["use", t] => Kick::Use { pos, again: Some(t.parse().map_err(|_| format!("bad tick count {t:?}"))?) },
        [state, t] => {
            let ticks: u32 = t.parse().map_err(|_| format!("bad tick count {t:?}"))?;
            if ticks == 0 {
                return Err("a temporary start block stays at least 1 gt".into());
            }
            Kick::Place { pos, state: crate::design::normalize_state(state)?, ticks }
        }
        _ => return Err(format!("port start: x y z (last | use [T] | <block state> <T>) [at N], got {args:?}")),
    };
    Ok((k, at))
}

impl Mode {
    pub fn of(d: &Design) -> Result<Mode, String> {
        if let (Some(door), Some(lever)) = (d.port("door"), d.port("lever")) {
            let along_z = d.meta.get("door-axis").is_some_and(|v| v.trim() == "z");
            // the opening's side (`piston_door::door_size`): 3 by default, 2 for the 2x2 door
            let size = match d.meta.get("door-size").map(|v| v.trim()) {
                None | Some("3") => 3,
                Some("2") => 2,
                Some(v) => return Err(format!("door-size: {v}: only 2 (the 2x2 door) or 3 (the 3x3 door, the default)")),
            };
            return Ok(Mode::Door { door, lever, along_z, size });
        }
        let kick = match d.ports.get("start") {
            Some((p, args)) if !args.is_empty() => Some(parse_kick(*p, args)?),
            Some(_) => return Err("port start needs a kick after its position: last | use [T] | <block state> <T>".into()),
            None => None,
        };
        let player_placement = match d.meta.get("placement").map(|s| s.as_str()) {
            None | Some("judge") => false,
            Some("player") => true,
            Some(v) => return Err(format!("placement: judge | player, got {v:?}")),
        };
        Ok(Mode::Clock { output: d.port("output"), kick, player_placement })
    }

    pub fn is_door(&self) -> bool {
        matches!(self, Mode::Door { .. })
    }
}

/// The cells of a door's opening (9, or 4 for a 2x2 door), row by row from the top, as the
/// judge lists them (`piston_door::DoorSpec::door_cells`).
pub fn door_cells(door: Pos, along_z: bool, size: i32) -> Vec<Pos> {
    let mut out = Vec::new();
    for dy in (0..size).rev() {
        for dw in 0..size {
            out.push(if along_z { [door[0], door[1] + dy, door[2] + dw] } else { [door[0] + dw, door[1] + dy, door[2]] });
        }
    }
    out
}

fn grow(lo: &mut Pos, hi: &mut Pos, p: Pos) {
    for i in 0..3 {
        lo[i] = lo[i].min(p[i]);
        hi[i] = hi[i].max(p[i]);
    }
}

impl Session {
    /// Parse, place like the judge, and (doors) settle. A design redsim cannot simulate still
    /// loads: [`Session::unsimulated`] says why and the world holds what it could place.
    pub fn new(text: &str, o: &Options) -> Result<Session, String> {
        crate::staged::block_on(Session::build(text.to_string(), *o, crate::staged::Budget::none()))
    }

    /// [`Session::new`] as staged work (`crate::staged`): every loop of ticks pauses when the
    /// budget's time is up (the browser builds the session a page's view plays over its frames,
    /// `Player::wake`; the coordinator 2026-10-22: no task over ~50-100 ms); the same ticks in the
    /// same order either way.
    pub async fn build(text: String, o: Options, b: crate::staged::Budget) -> Result<Session, String> {
        let text: &str = &text;
        let mut o = o;
        let mut design = Design::parse(text)?;
        if design.blocks.is_empty() && design.entities.is_empty() {
            return Err("empty design".into());
        }
        // an elevator's rider (track fly): the judge's armor stand on the platform at `port rider`
        // (`circuit::Circuit::from_design` adds it the same way, after the design's own entities,
        // for both elevator kinds)
        if design.meta.get("circuit").is_some_and(|k| matches!(k.split_whitespace().next(), Some("elevator" | "elevator-floors")))
            && let Some((r, _)) = design.ports.get("rider").cloned()
            && !design.entities.iter().any(|e| e.kind.ends_with("armor_stand"))
        {
            design.entities.push(crate::design::Entity { kind: "minecraft:armor_stand".into(), pos: [r[0] as f64 + 0.5, r[1] as f64, r[2] as f64 + 0.5], nbt: None });
        }
        if o.ground_reach.is_none() && ground_duper(&design) {
            // the browser never grows the ground by look-ahead (`Options::probe`): the judge's
            // first ground
            if !o.simulate || !o.probe {
                o = Options { ground_reach: Some(GROUND_REACH), ..o };
            } else {
                return Session::with_ground(text, &o);
            }
        }
        let o = &o;
        // the design's box as written (a TNT duper's ground is laid around it)
        let written = design.blocks.keys().fold(None, |acc: Option<(Pos, Pos)>, p| match acc {
            None => Some((*p, *p)),
            Some((mut lo, mut hi)) => {
                grow(&mut lo, &mut hi, *p);
                Some((lo, hi))
            }
        });
        // a circuit's inputs as its judge reads them, on the design as written (before its
        // consumers: the drivers' directions depend on the build's own box)
        let circuit_inputs = crate::circuit::inputs(&design);
        // the box the judge seeds its sandbox's level random from: the design as written, before
        // the consumers and the drivers below (`circuit::run::watch_box` of the circuit as loaded:
        // a driver behind an input is outside it)
        let judge_box = crate::circuit::judge_box(&design);
        // a harvester's: the farm judge's box (raised for the crop) and its intruder drops, from the
        // design as written
        let farm_box = crate::circuit::is_harvester(&design).then(|| crate::circuit::farm_box(&design)).flatten();
        // its random tick speed (the judge's for its robustness runs, `circuit::farm::game_rts`,
        // from its vanilla twin run: the page's `data-farm-rts`; else `farm_rts_for`) and its run
        let farm_rts = o.farm_rts.unwrap_or_else(|| crate::circuit::farm_rts_for(&design));
        let farm_ticks = crate::circuit::farm_ticks(&design, farm_rts);
        let farm_drops = if farm_box.is_some() { crate::circuit::farm_drops(&design, farm_ticks) } else { Vec::new() };
        // a circuit runs as its judge builds it: a lamp (the consumer) in every air output cell
        let consumers = crate::circuit::consumer_cells(&design);
        for p in &consumers {
            design.blocks.insert(*p, crate::circuit::CONSUMER.to_string());
        }
        // and every driven input's stone, repeater and support (R7, `Circuit::with_drivers`; the
        // switch comes and goes with the input edges)
        for ci in &circuit_inputs {
            if let crate::circuit::InputHow::Driven { dir, .. } = ci.how {
                let (r, sup, _) = crate::circuit::driver_cells(ci.pos, dir);
                design.blocks.entry(ci.pos).or_insert_with(|| crate::circuit::DRIVEN_STONE.to_string());
                design.blocks.insert(r, crate::circuit::driver_repeater(dir));
                design.blocks.insert(sup, crate::circuit::DRIVEN_STONE.to_string());
            }
        }
        // a shulker loader or unloader: the judge's chests on its ports, run by redsim's driver
        // once the design stands (`redstone::shulker::Chests`, below); their cells are in the box
        let shulker = crate::circuit::is_shulker(&design);
        let chest_cells = crate::circuit::chest_cells(&design);
        let mode = Mode::of(&design)?;
        let (mut lo, mut hi) = design.bounds().ok_or("empty design")?;
        for c in &chest_cells {
            grow(&mut lo, &mut hi, *c);
        }
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        for (name, (p, _)) in &design.ports {
            if is_port_named(name, "out") {
                outputs.push(*p);
            }
            if name == "lever" || is_port_named(name, "in") {
                inputs.push(*p);
            }
        }
        for ci in &circuit_inputs {
            if !inputs.contains(&ci.pos) {
                inputs.push(ci.pos);
            }
        }
        match &mode {
            Mode::Door { door, along_z, size, .. } => {
                let cells = door_cells(*door, *along_z, *size);
                for c in &cells {
                    grow(&mut lo, &mut hi, *c);
                }
                // the walkway in front of and behind the opening (the judge's watch box)
                let (a, b) = (cells[0], cells[cells.len() - 1]);
                let depth = if *along_z { [3, 0, 0] } else { [0, 0, 3] };
                grow(&mut lo, &mut hi, [a[0] - depth[0], a[1], a[2] - depth[2]]);
                grow(&mut lo, &mut hi, [b[0] + depth[0], b[1], b[2] + depth[2]]);
                lo = [lo[0] - 2, lo[1] - 2, lo[2] - 2];
                hi = [hi[0] + 2, hi[1] + 2, hi[2] + 2];
            }
            Mode::Clock { kick: Some((k, _)), .. } => {
                let p = match k {
                    Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
                };
                grow(&mut lo, &mut hi, p);
            }
            Mode::Clock { .. } => {}
        }
        for (p, s) in &design.blocks {
            let id = s.split('[').next().unwrap_or("");
            if matches!(id, "minecraft:lever" | "minecraft:stone_button" | "minecraft:oak_button") && !inputs.contains(p) {
                inputs.push(*p);
            }
        }
        let m = o.margin;
        let mut min = BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m);
        let mut size = [hi[0] - lo[0] + 1 + 2 * m, hi[1] - lo[1] + 1 + 2 * m, hi[2] - lo[2] + 1 + 2 * m];
        // a design that travels (a flying machine, a piston extender's payload): the world is the
        // judge's corridor, never the design's box (blocks leaving it would be cut off)
        let corridor = corridor_of(&design, &mode);
        if let Some((clo, chi)) = corridor {
            min = BlockPos::new(clo[0], clo[1], clo[2]);
            size = [chi[0] - clo[0] + 1, chi[1] - clo[1] + 1, chi[2] - clo[2] + 1];
        }
        // a TNT duper with its explosions stands on its judge's ground (coordinator 2026-10-20,
        // after vjeux 17:53Z "Please don't put a fake ground if the tnt goes through it"): real
        // blocks of the simulated world laid like the judge's strict run (`circuit::run::sim_tnt`:
        // `Circuit::ground_blocks`, `reach` around the design as written, its top the archived
        // safe drop under it) in the judge's world box around it; the machine keeps its own box
        let mut floor = None;
        let mut machine_box = None;
        if let (Some(r), true, Some((wlo, whi))) = (o.ground_reach, ground_duper(&design), written) {
            let drop = ground_drop(&design, text);
            floor = Some(Floor::grass(wlo, whi, r, drop)?);
            let from = [wlo[0] - r - 2, wlo[1] - 6 - drop, wlo[2] - r - 2];
            let to = [whi[0] + r + 2, whi[1] + 16, whi[2] + r + 2];
            // the machine's box, within the world's
            let mlo = [min.x.max(from[0]), min.y.max(from[1]), min.z.max(from[2])];
            let mhi = [(min.x + size[0] - 1).min(to[0]), (min.y + size[1] - 1).min(to[1]), (min.z + size[2] - 1).min(to[2])];
            machine_box = Some((BlockPos::new(mlo[0], mlo[1], mlo[2]), [mhi[0] - mlo[0] + 1, mhi[1] - mlo[1] + 1, mhi[2] - mlo[2] + 1]));
            min = BlockPos::new(from[0], from[1], from[2]);
            size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
        }
        let mut world = World::new(min, size);
        // a harvester's world is its judge's sandbox from the start (`circuit::farm_sandbox`: the
        // level random, its chunks, the drops and the random ticks' generator, as the bridge
        // creates it before the design is placed)
        if let Some((from, to)) = farm_box {
            crate::circuit::farm_sandbox(&mut world, from, to);
        }
        b.pause().await;
        let mut overlay = HashMap::new();
        let mut unsimulated: Vec<String> = Vec::new();
        let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
        for (p, s) in &design.blocks {
            match convert(s) {
                Ok((b, stone)) => {
                    if stone {
                        overlay.insert(*p, s.clone());
                    }
                    blocks.push((bp(*p), b));
                }
                Err(e) => {
                    if !unsimulated.contains(&e) {
                        unsimulated.push(e);
                    }
                }
            }
        }
        // a kind whose judge's run the view does not replay exactly yet: shown as built, saying
        // so (`circuit::not_replayed_yet`)
        if !o.unverified
            && let Some(why) = crate::circuit::not_replayed_yet(&design)
        {
            unsimulated.push(why);
        }
        // a big crop farm (more than FARM_RTS_PLANTS plants): its judge slows it further when its
        // collection would back up (`circuit::farm::game_rts`, from a 720,000 gt twin run): its
        // speed comes with the page (`data-farm-rts`); without it, shown as built
        if !o.unverified && o.farm_rts.is_none() && crate::circuit::is_harvester(&design) && crate::circuit::farm_rts_for(&design) < crate::circuit::FARM_RTS {
            unsimulated.push(format!("{} (a big crop farm, its judge's speed not given)", crate::circuit::NOT_REPLAYED));
        }
        // the clock judge's lamp (`clock::sim::judge_lamp`, vjeux 2026-10-20): in a clock's
        // declared output cell when the design leaves it empty, placed after the design's blocks
        // (what `clock::sim::parse_blocks` does); not a block of the design. Clocks only: a
        // design with a `circuit:` kind or an input port keeps its judge's consumers.
        let judge_lamp = design.ports.get("output").map(|(p, _)| *p).filter(|p| !design.blocks.contains_key(p) && !design.meta.contains_key("circuit") && !design.ports.keys().any(|k| k.starts_with("input")));
        if let Some(op) = judge_lamp
            && let Ok(b) = Block::parse(JUDGE_LAMP)
        {
            blocks.push((bp(op), b));
        }
        // shown as built (below): a shulker design's chests (the judge places them) keep it
        // readable; nothing the judge does not place is added
        let as_built = |blocks: &[(BlockPos, Block)]| -> Vec<(BlockPos, Block)> {
            let mut v = blocks.to_vec();
            if shulker {
                let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
                v.extend(chest_cells.iter().map(|p| (bp(*p), chest)));
            }
            v
        };
        let mut s = Session {
            design,
            mode,
            world: World::new(min, [1, 1, 1]),
            tick: 0,
            rest_tick: 0,
            last_input: None,
            actions: Vec::new(),
            unsimulated: None,
            overlay,
            inputs,
            outputs,
            initial: None,
            replay: None,
            corridor,
            end_tick: None,
            input_on: vec![false; circuit_inputs.len()],
            circuit_inputs,
            demo: None,
            releases: Vec::new(),
            picture: None,
            consumers,
            judge_lamp,
            prev_falling: Vec::new(),
            halted: None,
            shulker: None,
            shulker_initial: None,
            visit_due: false,
            judge_every: 1,
            judge_from: 0,
            farm: Vec::new(),
            received: Default::default(),
            floor,
            machine_box,
            outside: Default::default(),
            period_memo: Default::default(),
            still: false,
        };
        if !o.simulate {
            // a still (vjeux 2026-10-21: "Why is it simulating everything?"; the coordinator:
            // cards never simulate, a page's view simulates only when played or pressed): the
            // design as the judge places it (raw, as stored), its judge's chests and crop, its own
            // entities, a TNT duper's ground; nothing ticks
            let mut v = as_built(&blocks);
            v.extend(crate::circuit::still_judge_blocks(&s.design));
            world.place_raw(&v);
            for e in &s.design.entities {
                let _ = redsim::entity::spawn(&mut world, &e.kind, e.pos, e.nbt.as_deref());
            }
            if let Some(f) = &s.floor {
                world.place_raw(&f.blocks());
            }
            world.errors.clear();
            s.world = world;
            s.still = true;
            if !unsimulated.is_empty() {
                s.unsimulated = Some(unsimulated.join("; "));
                s.circuit_inputs.clear();
                s.input_on.clear();
            }
            return Ok(s);
        }
        if !unsimulated.is_empty() {
            // Shown as built: every block the model knows placed raw, nothing ticks (and nothing
            // to drive: no inputs). No ground: nothing falls on it.
            world.place_raw(&as_built(&blocks));
            s.world = world;
            s.unsimulated = Some(unsimulated.join("; "));
            s.circuit_inputs.clear();
            s.input_on.clear();
            s.floor = None;
            return Ok(s);
        }
        // the look-ahead for something redsim does not model in a judged run (`Options::probe`:
        // none in the browser, where the view stops when it gets there)
        let probe_ticks = if o.probe { SHULKER_PROBE } else { 0 };
        if let Err(e) = s.place(&mut world, &blocks, o, &b).await {
            // e.g. container contents redsim does not model: shown as built, not simulated
            let mut w = World::new(min, size);
            w.place_raw(&as_built(&blocks));
            s.world = w;
            s.unsimulated = Some(e);
            s.circuit_inputs.clear();
            s.input_on.clear();
            s.floor = None;
            return Ok(s);
        }
        b.pause().await;
        // a shulker loader or unloader runs like its judge: the chests placed raw after the
        // design, the settle, the boxes and the items (the start lever left to the visitor; the
        // probe below runs with it on); then the judge's visit before every tick (`step`)
        if let Some(mut ch) = crate::circuit::shulker_chests(&s.design) {
            // the judge's sandbox (clk-misc): the level random mirrored and the drops seeded from
            // the seed the judge builds this design with (its watch box), so a dropper choosing
            // among stacks and a hopper choosing among minecarts draw what the game draws
            if let Some((from, to)) = judge_box {
                redsim::redstone::shulker::judge_sandbox(&mut world, from, to);
            }
            ch.place(&mut world);
            for _ in 0..redsim::redstone::shulker::SETTLE {
                world.tick();
                b.pause().await;
            }
            ch.start(&mut world);
            // (the start lever stays off: the visitor turns it on, as the judge does at its start)
            // a run redsim cannot follow (something it does not model; the random choices are
            // drawn from the mirrored level random above): shown as built, chests and all
            let (mut pw, mut pc) = (world.clone(), ch.clone());
            for ci in &s.circuit_inputs {
                if ci.how == crate::circuit::InputHow::Lever && !(ci.name == "mode" || ci.name.starts_with("mode.")) {
                    pw.use_block(bp(ci.pos));
                }
            }
            for t in 0..probe_ticks {
                if t > 0 {
                    pc.visit(&mut pw);
                }
                pw.tick();
                if let Some(e) = pw.errors.first() {
                    s.world = world;
                    s.unsimulated = Some(format!("not modeled by redsim (tick {t} of its run): {e}"));
                    s.circuit_inputs.clear();
                    s.input_on.clear();
                    return Ok(s);
                }
            }
            s.tick += redsim::redstone::shulker::SETTLE;
            s.rest_tick = s.tick;
            s.shulker_initial = Some(ch.clone());
            s.shulker = Some(ch);
        }
        // an item sorter runs like its judge's redsim screen (`circuit::sorter::sim_run`; critique:
        // sorter-classic stood still at 0 ticks): a chest on each `items`, `pass` and `output` port
        // (set like a player's), the settle, then the source fed from the judge's stream and the
        // sinks emptied every SORTER_SEG gt
        if let Some(mut ch) = crate::circuit::sorter_chests(&s.design) {
            let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
            for p in ch.sources.iter().chain(&ch.outputs) {
                world.set_block(*p, chest, flags::ALL);
            }
            for _ in 0..crate::circuit::SETTLE {
                world.tick();
                b.pause().await;
            }
            ch.start(&mut world);
            // a run redsim cannot follow: shown as built
            let (mut pw, mut pc) = (world.clone(), ch.clone());
            for t in 0..probe_ticks {
                if t > 0 && t % crate::circuit::SORTER_SEG == 0 {
                    pc.visit(&mut pw);
                }
                pw.tick();
                if let Some(e) = pw.errors.first() {
                    s.world = world;
                    s.unsimulated = Some(format!("not modeled by redsim (tick {t} of its run): {e}"));
                    return Ok(s);
                }
            }
            s.tick += crate::circuit::SETTLE;
            s.rest_tick = s.tick;
            s.judge_every = crate::circuit::SORTER_SEG;
            s.judge_from = s.tick;
            s.shulker_initial = Some(ch.clone());
            s.shulker = Some(ch);
        }
        // an auto-smelter runs like its judge's base run (`circuit::smelter::sim_run`; the
        // coordinator 2026-10-21: the 3D view did not run the smelter study): a chest on its
        // `items`, `fuel` and `output` ports (in that order, set like a player's), the settle, the
        // input chest filled from the judge's load and the fuel chest with its coal, the output
        // chest emptied every SORTER_SEG gt; an auto-brewer likewise (`circuit::brewer::sim_run`:
        // its bottles, fuel, wart, sugar and output chests, the four supply chests filled once)
        if let Some(mut ch) = crate::circuit::smelter_chests(&s.design).or_else(|| crate::circuit::brewer_chests(&s.design)).or_else(|| crate::circuit::composter_chests(&s.design)) {
            // an auto-composter draws its composting chances from the level random: the judge's
            // sandbox, mirrored from the seed of the box the judge builds it in
            if crate::circuit::is_composter(&s.design)
                && let Some((from, to)) = judge_box
            {
                redsim::redstone::shulker::judge_sandbox(&mut world, from, to);
            }
            let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
            for p in ch.sources.iter().chain(&ch.supplies).chain(&ch.outputs) {
                world.set_block(*p, chest, flags::ALL);
            }
            // a double chest the judge places joined (`port items.b:`, `port fuel.b:`: sorter's
            // super smelters, whose 16-furnace build runs one hopper line under each half): both
            // halves, joined, one container for the hoppers under either half
            for name in ["items", "fuel"] {
                if let Some((a, b)) = crate::circuit::double_chest(&s.design, name) {
                    let (ha, hb) = crate::circuit::double_chest_blocks(a, b);
                    world.set_block(bp(a), ha, flags::ALL);
                    world.set_block(bp(b), hb, flags::ALL);
                }
            }
            for _ in 0..crate::circuit::SETTLE {
                world.tick();
                b.pause().await;
            }
            ch.start(&mut world);
            // a run redsim cannot follow: shown as built
            let (mut pw, mut pc) = (world.clone(), ch.clone());
            for t in 0..probe_ticks {
                if t > 0 && t % crate::circuit::SORTER_SEG == 0 {
                    pc.visit(&mut pw);
                }
                pw.tick();
                if let Some(e) = pw.errors.first() {
                    s.world = world;
                    s.unsimulated = Some(format!("not modeled by redsim (tick {t} of its run): {e}"));
                    return Ok(s);
                }
            }
            s.tick += crate::circuit::SETTLE;
            s.rest_tick = s.tick;
            s.judge_every = crate::circuit::SORTER_SEG;
            s.judge_from = s.tick;
            s.shulker_initial = Some(ch.clone());
            s.shulker = Some(ch);
        }
        // the judge's chests of a kind whose item runs the viewer does not replay yet (minecart
        // stations, auto-crafters, item elevators: `circuit::static_judge_chests`; site
        // 2026-10-21: "a chest there ... would show the build as used"): static chests on their
        // `items` and `output` cells, no lamp
        let station = crate::circuit::static_judge_chest_blocks(&s.design);
        if !station.is_empty() {
            world.place_raw(&station);
        }
        // a crop harvester runs like its judge's run (`circuit::farm::game_run`, which judges it
        // in the game; the coordinator 2026-10-21: replay the judge's run exactly): in the judge's
        // sandbox (`circuit::farm_sandbox`: its seed, the level random, its chunks' order and the
        // random ticks' generator, as clk-misc's exact random ticks mirror the game), the judge's
        // chest on every `port output:` (raw), its crop planted on every `port crop:` (with
        // updates) with random ticks off, the settle, then growth at the judge's speed (FARM_RTS)
        // for its run (FARM_RUN), its intruders dropped at its quarters (`circuit::farm_drops`)
        if let (Some((chests, crops)), Some(_)) = (crate::circuit::harvester_ports(&s.design), farm_box) {
            let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
            world.place_raw(&chests.iter().map(|p| (bp(*p), chest)).collect::<Vec<_>>());
            if let Some(rt) = world.random_ticks.as_mut() {
                rt.speed = 0;
            }
            for (p, st) in &crops {
                world.set_block(bp(*p), Block::parse(st)?, flags::ALL);
                b.pause().await;
            }
            for _ in 0..crate::circuit::SETTLE {
                world.tick();
                b.pause().await;
            }
            if let Some(rt) = world.random_ticks.as_mut() {
                rt.speed = farm_rts;
            }
            // the judge's run at its speed (`circuit::farm::sim_run`): the intruders at its
            // quarters, its chests emptied every FARM_EMPTY gt of the run and the rest, growth
            // stopped at the run's end, then resumed while a crop item lies on a crop
            let run = s.tick + crate::circuit::SETTLE;
            s.farm = farm_drops.into_iter().map(|(t, at, nbt)| (run + t, FarmEvent::Drop { at, nbt })).collect();
            s.farm.push((run + farm_ticks, FarmEvent::Speed(0)));
            let rest = 2 * crate::circuit::FARM_REST;
            let crop = crate::circuit::farm_crop_id(&s.design).unwrap_or_default();
            let crop_item = crate::circuit::farm_crop_item(&crop).to_string();
            let judge_chests: Vec<Pos> = chests.clone();
            for t in (crate::circuit::FARM_EMPTY..=farm_ticks + rest).step_by(crate::circuit::FARM_EMPTY as usize) {
                s.farm.push((run + t, FarmEvent::Empty { chests: judge_chests.clone(), crop_item: crop_item.clone() }));
            }
            s.farm.push((run + farm_ticks + rest, FarmEvent::Regrow { resumed: 0, cap: crate::circuit::FARM_REGROW * crate::circuit::FARM_RTS / farm_rts.max(1), rts: farm_rts, crop }));
            // (a tick's events in the judge's order; the drops keep theirs: the summon numbers)
            s.farm.sort_by_key(|(t, e)| (*t, e.rank()));
            // a run redsim cannot follow: shown as built
            let mut pw = world.clone();
            for t in 0..probe_ticks {
                pw.tick();
                if let Some(e) = pw.errors.first() {
                    s.world = world;
                    s.unsimulated = Some(format!("not modeled by redsim (tick {t} of its run): {e}"));
                    s.farm.clear();
                    return Ok(s);
                }
            }
            s.tick += crate::circuit::SETTLE;
            s.rest_tick = s.tick;
        } else if crate::circuit::is_harvester(&s.design) {
            // a crop the viewer does not grow (one redsim does not model): the judge's chests
            // stand on its `port output:` cells (no lamp) and the farm is shown as built
            let crop = s.design.meta.get("circuit").and_then(|k| k.split_whitespace().nth(1)).unwrap_or("its crop").trim_start_matches("minecraft:").to_string();
            let chest = Block::Chest { facing: Dir::North, kind: 0, waterlogged: false };
            let chests: Vec<(BlockPos, Block)> = s.design.ports.iter().filter(|(n, _)| crate::circuit::is_named(n, "output")).map(|(_, (p, _))| (bp(*p), chest)).collect();
            world.place_raw(&chests);
            s.world = world;
            s.unsimulated = Some(format!("{} is not modeled by redsim yet (the farm is shown as built)", crop.replace('_', " ")));
            return Ok(s);
        }
        // a design with TNT (clk-misc): its TNT lives and explodes like in a bridge sandbox
        // (redsim's TNT entities, the level random mirrored and the drops seeded from seed 0 with
        // the world box's chunks, like the clock traces): `explosion::explode_on`
        // a TNT duper's ground, raw, once the design stands (`clock::sim::drive_tnt`'s order);
        // from then on the world logs what it writes (the ground and anything outside the
        // machine's box are followed from that log: `take_writes`)
        if let Some(f) = &s.floor {
            world.place_raw(&f.blocks());
            world.log_writes = true;
            world.writes.clear();
        }
        if is_tnt_design(&s.design) {
            let to = [min.x + size[0] - 1, min.y + size[1] - 1, min.z + size[2] - 1];
            redsim::explosion::explode_on(&mut world, 0, redsim::redstone::shulker::chunk_range([min.x, min.y, min.z], to));
        }
        // a randomizer (study 16): a dropper's random slot is the game's level random, mirrored
        // like the judge's base run (its watch box's seed, salt 0, and forced chunks:
        // `circuit::random`): the view plays the judge's test press for press and a visitor's own
        // presses draw from the same random (site 2026-10-21: rand2-token halted at tick 44)
        if is_randomizer(&s.design)
            && let Some((from, to)) = judge_box
        {
            world.mirror_level_random(redsim::redstone::shulker::build_seed(from, to, 0), redsim::redstone::shulker::chunk_range(from, to));
        }
        b.pause().await;
        s.world = world;
        s.initial = Some((s.world.clone(), s.actions.clone(), s.tick));
        // a session at tick t has the inputs due at t applied (the judge's between-ticks order)
        s.apply_due();
        s.take_writes();
        b.pause().await;
        // a circuit stands at rest after placement as its judge lets it before the first input
        // (`circuit::stimulus::SETTLE`, its output steady over the second half): redsim runs until
        // idle, at most SETTLE / 2 ticks, before the first frame shown, and the badge counts from
        // there (coordinator: lock3-q85's OUT read powered for its first 4 ticks, a placement
        // transient, on a paused page). The judge's first input comes after SETTLE.
        if settles_at_rest(&s) {
            while s.tick < crate::circuit::SETTLE / 2 && !s.world.idle() && s.halted.is_none() {
                s.step();
                b.pause().await;
            }
            s.rest_tick = s.tick;
            s.initial = Some((s.world.clone(), s.actions.clone(), s.tick));
        }
        if o.probe && s.fly_end_due() {
            s.end_tick = Some(s.fly_end());
        }
        Ok(s)
    }

    /// A flying machine started by its own input (a start lever, no kick): it rests until a player
    /// or the auto-demo flips it, so it has no end before that (site 2026-10-21: the start-lever
    /// picks stood at 0 ticks, the end found at once undoing every flip).
    pub fn starts_on_input(&self) -> bool {
        !self.circuit_inputs.is_empty() && !self.design.ports.contains_key("start")
    }

    /// A TNT duper on its judge's ground at the reach its judge grows the ground to: 24, doubled
    /// while a burning TNT leaves it within the first [`GROUND_PROBE`] ticks (the judge's stimulus
    /// replayed for a triggered or switchable duper), at most 96; cached per design text.
    fn with_ground(text: &str, o: &Options) -> Result<Session, String> {
        thread_local! {
            static REACH: std::cell::RefCell<HashMap<u64, i32>> = std::cell::RefCell::new(HashMap::new());
        }
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut h);
            h.finish()
        };
        if let Some(r) = REACH.with(|m| m.borrow().get(&key).copied()) {
            return Session::new(text, &Options { ground_reach: Some(r), ..*o });
        }
        let mut r = GROUND_REACH;
        loop {
            let mut s = Session::new(text, &Options { ground_reach: Some(r), ..*o })?;
            if r >= MAX_GROUND_REACH || s.floor.is_none() || !s.tnt_leaves_ground(GROUND_PROBE) {
                s.reset();
                REACH.with(|m| m.borrow_mut().insert(key, r));
                return Ok(s);
            }
            r *= 2;
        }
    }

    /// Whether a burning TNT goes past the ground's reach within `ticks` (`TntOptions::escape`:
    /// more than `reach - 1` blocks sideways from the design's box), the judge's stimulus replayed
    /// (`circuit::tnt::stimulus`: a triggered duper's 4 gt pulses 100 gt apart, a switchable one
    /// off then on 400 gt each). The session is left where the probe stopped.
    fn tnt_leaves_ground(&mut self, ticks: u32) -> bool {
        let Some(((lo, hi), reach)) = self.floor.as_ref().map(|f| (f.design_box, f.reach as f64 - 1.0)) else { return false };
        let triggered = self.design.meta.get("circuit").is_some_and(|c| c.split_whitespace().next() == Some("tnt-duper-triggered"));
        let t0 = self.free_from() + 20;
        let end = self.tick + ticks;
        while self.tick < end && self.halted.is_none() {
            if !self.circuit_inputs.is_empty() && self.tick >= t0 {
                let k = self.tick - t0;
                let on = if triggered { k % 104 < 4 } else { (k / 400) % 2 == 1 };
                if self.input_on.first() != Some(&on) {
                    self.set_input(0, on);
                }
            }
            self.step();
            for e in self.world.entities.list.iter().filter(|e| !e.removed) {
                if matches!(e.kind, redsim::entity::EntityKind::PrimedTnt(_)) {
                    let p = [e.pos.x, e.pos.y, e.pos.z];
                    let side = |i: usize| (lo[i] as f64 - p[i]).max(p[i] - (hi[i] as f64 + 1.0)).max(0.0);
                    if side(0).max(side(2)) > reach {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// The cells the last ticks wrote (the world's write log, on with a TNT duper's ground): a
    /// ground cell that no longer holds its layer's block joins [`Floor::changed`] (and leaves it
    /// once it holds it again); a cell outside the machine's box and the ground joins `outside`
    /// while it holds a block (drawn with the machine's). No scan of the big world per tick.
    fn take_writes(&mut self) {
        if !self.world.log_writes {
            return;
        }
        let writes = std::mem::take(&mut self.world.writes);
        let (min, size) = self.machine();
        let in_machine = |p: BlockPos| p.x >= min.x && p.y >= min.y && p.z >= min.z && p.x < min.x + size[0] && p.y < min.y + size[1] && p.z < min.z + size[2];
        for p in writes {
            let b = self.world.get(p);
            let c = p.arr();
            match self.floor.as_mut() {
                Some(f) if f.contains(c) => {
                    if b == f.layer(c[1]).0 {
                        f.changed.remove(&c);
                    } else {
                        f.changed.insert(c, b);
                    }
                }
                _ if !in_machine(p) => {
                    if b == Block::Air {
                        self.outside.remove(&c);
                    } else {
                        self.outside.insert(c, b);
                    }
                }
                _ => {}
            }
        }
    }

    /// Start over from the world as placed: nothing changed in the ground, nothing outside.
    fn clear_writes(&mut self) {
        self.world.writes.clear();
        self.outside.clear();
        if let Some(f) = self.floor.as_mut() {
            f.changed.clear();
        }
    }

    /// The part of the world the machine's blocks are drawn and hashed from: its box, the ground
    /// left out.
    pub fn machine(&self) -> (BlockPos, [i32; 3]) {
        self.machine_box.unwrap_or((self.world.min, self.world.size))
    }

    fn in_floor(&self, p: Pos) -> bool {
        self.floor.as_ref().is_some_and(|f| f.contains(p))
    }

    /// The machine's non-air cells (the ground left out).
    pub fn machine_blocks(&self) -> Vec<(BlockPos, Block)> {
        let (min, size) = self.machine();
        let mut v = Vec::new();
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let q = min.offset(x, y, z);
                    let b = self.world.get(q);
                    if b != Block::Air && !self.in_floor(q.arr()) {
                        v.push((q, b));
                    }
                }
            }
        }
        v
    }

    async fn place(&mut self, w: &mut World, blocks: &[(BlockPos, Block)], o: &Options, b: &crate::staged::Budget) -> Result<(), String> {
        let bes = self.design.block_entities.clone();
        let nbt = |w: &mut World, only: Option<Pos>, skip: Option<Pos>| -> Result<(), String> {
            let lines: Vec<(BlockPos, &str)> = bes.iter().filter(|(p, _)| only.is_none_or(|q| q == **p) && skip != Some(**p)).map(|(p, s)| (bp(*p), s.as_str())).collect();
            redsim::redstone::container::apply_block_entities(w, lines).map_err(|e| format!("nbt: {e}"))
        };
        match self.mode.clone() {
            Mode::Door { .. } => {
                w.place_raw(blocks);
                nbt(w, None, None)?;
                w.place_updates(blocks);
                // `cosearch::place_and_settle`: tick until idle (at least once)
                let mut n = 0;
                while n < o.settle {
                    w.tick();
                    n += 1;
                    if w.idle() {
                        break;
                    }
                    b.pause().await;
                }
                self.tick = n;
                self.rest_tick = n;
            }
            Mode::Clock { kick, player_placement, .. } => {
                let last = match &kick {
                    Some((Kick::Last { pos }, _)) => Some(*pos),
                    _ => None,
                };
                // `stop:` blocks first, placed and updated on their own (the environment stands
                // before the build: `circuit fly sim`'s order)
                let mut stops: Vec<(BlockPos, Block)> = Vec::new();
                for (p, s) in &self.design.stops {
                    stops.push((bp(*p), convert(s)?.0));
                }
                if !stops.is_empty() {
                    w.place_raw(&stops);
                    w.place_updates(&stops);
                }
                let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(p.arr()) != last).copied().collect();
                if player_placement {
                    for (p, b) in &silent {
                        w.set_block(*p, *b, flags::ALL);
                    }
                    nbt(w, None, last)?;
                } else {
                    w.place_raw(&silent);
                    nbt(w, None, last)?;
                    w.place_updates(&silent);
                }
                // the design's entities, once the blocks stand (as the judge summons them)
                for e in self.design.entities.clone() {
                    redsim::entity::spawn(w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
                }
                if let Some((k, at)) = kick {
                    self.actions = match k {
                        Kick::Last { pos } => {
                            let b = blocks.iter().find(|(p, _)| p.arr() == pos).map(|(_, b)| *b).ok_or("port start: no design block there to place last")?;
                            vec![(at, Action::Place { pos, block: b, nbt: self.design.block_entities.get(&pos).cloned() })]
                        }
                        Kick::Use { pos, again } => {
                            let mut v = vec![(at, Action::Use { pos })];
                            if let Some(t) = again {
                                v.push((at + t, Action::Use { pos }));
                            }
                            v
                        }
                        Kick::Place { pos, state, ticks } => {
                            let b = convert(&state)?.0;
                            vec![(at, Action::Place { pos, block: b, nbt: None }), (at + ticks, Action::Clear { pos })]
                        }
                    };
                }
            }
        }
        Ok(())
    }

    /// Back to the placed state.
    pub fn reset(&mut self) {
        if let Some(r) = self.replay.as_mut() {
            r.reset();
            self.tick = r.tick;
            return;
        }
        if let Some((w, a, t)) = &self.initial {
            self.world = w.clone();
            self.actions = a.clone();
            self.tick = *t;
            self.last_input = None;
            self.input_on = vec![false; self.circuit_inputs.len()];
            self.releases.clear();
            self.prev_falling.clear();
            self.halted = None;
            self.shulker = self.shulker_initial.clone();
            self.received.clear();
            self.visit_due = false;
            self.apply_due();
            self.inputs_due();
            self.clear_writes();
            self.take_writes();
        }
    }

    /// Snapshot the run's mutable state (the timelapse).
    pub fn snapshot(&self) -> RunState {
        RunState {
            world: self.world.clone(),
            tick: self.tick,
            last_input: self.last_input,
            actions: self.actions.clone(),
            input_on: self.input_on.clone(),
            releases: self.releases.clone(),
            prev_falling: self.prev_falling.clone(),
            halted: self.halted.clone(),
            shulker: self.shulker.clone(),
            received: self.received.clone(),
            visit_due: self.visit_due,
            farm: self.farm.clone(),
            outside: self.outside.clone(),
            end_tick: self.end_tick,
            replay: self.replay.clone(),
            demo: self.demo.clone(),
        }
    }

    /// Restore a snapshot (the timelapse): the run continues from there exactly, the
    /// period cache and the write log cleared like [`Session::reset`].
    pub fn restore(&mut self, s: RunState) {
        self.world = s.world;
        self.tick = s.tick;
        self.last_input = s.last_input;
        self.actions = s.actions;
        self.input_on = s.input_on;
        self.releases = s.releases;
        self.prev_falling = s.prev_falling;
        self.halted = s.halted;
        self.shulker = s.shulker;
        self.received = s.received;
        self.visit_due = s.visit_due;
        self.farm = s.farm;
        self.outside = s.outside;
        self.end_tick = s.end_tick;
        self.replay = s.replay;
        self.demo = s.demo;
        self.period_memo = std::cell::RefCell::new(None);
        self.clear_writes();
        self.take_writes();
    }

    pub fn simulated(&self) -> bool {
        self.unsimulated.is_none()
    }

    /// Something moves when played: a redsim simulation or a recording.
    pub fn playable(&self) -> bool {
        self.simulated() || self.replay.is_some()
    }

    /// Is the (first) output on now? (None: no output, or nothing simulated or recorded.)
    pub fn output_on(&self) -> Option<bool> {
        let o = *self.outputs.first()?;
        if let Some(r) = &self.replay {
            return Some(r.state_at(o).is_some_and(active_state));
        }
        if !self.simulated() {
            return None;
        }
        Some(active(&self.world.get(bp(o))))
    }

    /// Play a real-game recording (`designs/clocks/traces/<stem>.trace`) instead of simulating:
    /// for the designs redsim does not model (boats, minecarts, fluids).
    pub fn attach_trace(&mut self, text: &str) -> Result<(), String> {
        let r = crate::replay::Replay::parse(text)?;
        if self.outputs.is_empty()
            && let Some(o) = r.output
        {
            self.outputs.push(o);
        }
        self.tick = r.tick;
        self.rest_tick = 0;
        self.replay = Some(r);
        Ok(())
    }

    /// Apply the scheduled inputs due now (between ticks).
    fn apply_due(&mut self) {
        while self.actions.first().is_some_and(|(at, _)| *at == self.tick) {
            let (_, a) = self.actions.remove(0);
            match a {
                Action::Place { pos, block, nbt } => {
                    self.world.set_block(bp(pos), block, flags::ALL);
                    if let Some(n) = nbt {
                        let _ = redsim::redstone::container::apply_block_entities(&mut self.world, [(bp(pos), n.as_str())]);
                    }
                }
                Action::Use { pos } => {
                    self.world.use_block(bp(pos));
                }
                Action::Clear { pos } => {
                    self.world.set_block(bp(pos), Block::Air, flags::ALL);
                }
                Action::ShapeSet { pos, block } => {
                    self.world.set_block(bp(pos), block, flags::CLIENTS);
                }
                Action::Update { pos } => {
                    let src = self.world.get(bp(pos)).kind();
                    self.world.update_neighbors_at(bp(pos), src);
                }
            }
        }
    }

    /// One game tick, then the scheduled inputs due at the new tick (so the state shown at tick t
    /// is what the judge sees between ticks t and t+1).
    pub fn step(&mut self) {
        if let Some(r) = self.replay.as_mut() {
            r.step();
            self.tick = r.tick;
            return;
        }
        if !self.simulated() || self.halted.is_some() {
            return;
        }
        self.apply_due();
        // a shulker design: the judge's visit after the previous tick's frame (what it takes out
        // of the chests it empties goes on the run's tally)
        if self.visit_due
            && let Some(ch) = self.shulker.as_mut()
        {
            let emptied: Vec<BlockPos> = ch.outputs.iter().chain(if ch.loader { &[][..] } else { &ch.sources[..] }).chain(&ch.rejects).copied().collect();
            for p in emptied {
                let slots: Vec<Option<redsim::redstone::container::Stack>> = redsim::redstone::shulker::chest_stacks(&self.world, p).into_iter().map(Some).collect();
                if slots.is_empty() {
                    continue;
                }
                let got = crate::labels::summarize(&self.world, &slots);
                let tally = self.received.entry(p.arr()).or_default();
                *tally = crate::labels::merge([tally.as_slice(), got.as_slice()]);
            }
            ch.visit(&mut self.world);
        }
        // a harvester: the farm judge's run events due before this tick
        let due: Vec<FarmEvent> = self.farm.iter().filter(|(t, _)| *t == self.tick).map(|(_, e)| e.clone()).collect();
        for e in due {
            match e {
                FarmEvent::Speed(n) => {
                    if let Some(rt) = self.world.random_ticks.as_mut() {
                        rt.speed = n;
                    }
                }
                FarmEvent::Drop { at, nbt } => {
                    if let Err(why) = redsim::entity::spawn(&mut self.world, "minecraft:item", at, Some(&nbt)) {
                        self.world.errors.push(format!("an intruder: {why}"));
                    }
                }
                FarmEvent::Empty { chests, crop_item } => {
                    // (`circuit::farm::sim_chest_counts`: a crop item or an intruder makes the
                    // judge take everything; the tally for the label)
                    for p in chests {
                        let counted = self.world.containers.map.get(&bp(p)).is_some_and(|c| {
                            c.slots.iter().flatten().any(|st| {
                                let id = redsim::redstone::container::item_name(st.item);
                                let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
                                id == crop_item || crate::circuit::FARM_INTRUDERS.iter().any(|(i, _)| *i == id)
                            })
                        });
                        if counted {
                            let slots: Vec<Option<redsim::redstone::container::Stack>> = redsim::redstone::shulker::chest_stacks(&self.world, bp(p)).into_iter().map(Some).collect();
                            let got = crate::labels::summarize(&self.world, &slots);
                            let tally = self.received.entry(p).or_default();
                            *tally = crate::labels::merge([tally.as_slice(), got.as_slice()]);
                            redsim::redstone::container::set_contents(&mut self.world, bp(p), &[]);
                        }
                    }
                }
                FarmEvent::Regrow { resumed, cap, rts, crop } => {
                    let item = crate::circuit::farm_crop_item(&crop).to_string();
                    let on_crop = self.world.entities.list.iter().filter(|e| !e.removed && e.summon_index.is_none()).any(|e| {
                        let redsim::entity::EntityKind::Item(st) = &e.kind else { return false };
                        let id = if st.item.contains(':') { st.item.clone() } else { format!("minecraft:{}", st.item) };
                        if id != item {
                            return false;
                        }
                        let under = BlockPos::new(e.pos.x.floor() as i32, (e.pos.y - 0.01).floor() as i32, e.pos.z.floor() as i32);
                        crate::circuit::farm_is_crop_block(&self.world.get(under).to_string(), &crop)
                    });
                    if on_crop && resumed < cap {
                        if let Some(rt) = self.world.random_ticks.as_mut() {
                            rt.speed = rts;
                        }
                        let at = self.tick + crate::circuit::FARM_BATCH;
                        self.farm.push((at, FarmEvent::Regrow { resumed: resumed + crate::circuit::FARM_BATCH, cap, rts, crop }));
                    } else if resumed > 0
                        && let Some(rt) = self.world.random_ticks.as_mut()
                    {
                        rt.speed = 0;
                    }
                }
            }
        }
        self.prev_falling = self.world.gravity.entities.iter().filter(|f| !f.removed).map(|f| (f.id, [f.x, f.y, f.z])).collect();
        self.world.tick();
        self.tick += 1;
        // the judge visits after every tick (a shulker design's) or every SORTER_SEG (a sorter's)
        self.visit_due = self.shulker.is_some() && self.tick > self.judge_from && (self.tick - self.judge_from) % self.judge_every.max(1) == 0;
        self.apply_due();
        self.inputs_due();
        self.take_writes();
        // redsim met something it does not model: the view stops at the last exact state
        if let Some(e) = self.world.errors.first() {
            self.halted = Some(format!("tick {}: {e}", self.tick));
        }
    }

    /// Ticks run since the last input (or since rest / placement).
    pub fn since_input(&self) -> u32 {
        self.tick - self.last_input.unwrap_or(self.rest_tick)
    }

    /// Is this cell clickable (a lever, a button, an input port)?
    pub fn is_input(&self, p: Pos) -> bool {
        self.inputs.contains(&p)
    }

    /// Drive circuit input `i` like the judge (between ticks): its cell written with updates, or its
    /// lever used at every edge, or its button at a rising edge. False when nothing changed.
    pub fn set_input(&mut self, i: usize, on: bool) -> bool {
        if !self.simulated() || i >= self.circuit_inputs.len() || self.input_on[i] == on {
            return false;
        }
        let ci = self.circuit_inputs[i].clone();
        match &ci.how {
            // an item frame: a press turns it one step; it never stays "on" (every press turns)
            crate::circuit::InputHow::Frame => {
                self.turn_frame(i);
                return true;
            }
            crate::circuit::InputHow::Block { on: a, off: b } => {
                self.world.set_block(bp(ci.pos), if on { *a } else { *b }, flags::ALL);
            }
            crate::circuit::InputHow::Lever => {
                self.world.use_block(bp(ci.pos));
            }
            crate::circuit::InputHow::Button => {
                if on {
                    self.world.use_block(bp(ci.pos));
                }
            }
            crate::circuit::InputHow::Strength { .. } => {
                self.set_strength(i, if on { 15 } else { 0 });
            }
            // the judge's switch behind its repeater: placed or removed (the input cell's power
            // follows 2 gt later, through the repeater)
            crate::circuit::InputHow::Driven { dir, .. } => {
                let (_, _, sw) = crate::circuit::driver_cells(ci.pos, *dir);
                let b = if on { Block::parse(crate::circuit::INPUT_ON).unwrap_or(Block::Air) } else { Block::Air };
                self.world.set_block(bp(sw), b, flags::ALL);
            }
        }
        self.input_on[i] = on;
        self.last_input = Some(self.tick);
        true
    }

    /// Item frame input `i` turned one step, like the judge's right-click (`Action::UseEntity`:
    /// redsim's `entity::frame::use_frame`, which updates the comparators reading it).
    pub fn turn_frame(&mut self, i: usize) {
        let Some(ci) = self.circuit_inputs.get(i) else { return };
        let pos = ci.pos;
        if !redsim::entity::frame::use_frame(&mut self.world, bp(pos)) {
            self.world.errors.push(format!("no single item frame at {pos:?} to turn"));
        }
        self.last_input = Some(self.tick);
    }

    /// The turn (0..8) of the item in input `i`'s frame, if it is one.
    pub fn frame_turn(&self, i: usize) -> Option<u8> {
        let pos = bp(self.circuit_inputs.get(i).filter(|c| c.how == crate::circuit::InputHow::Frame)?.pos);
        self.world.frames.iter().find(|f| f.pos == pos).map(|f| f.rotation)
    }

    /// A strength input's value, like the judge (`circuit::Circuit::actions`): its chest placed
    /// again (its state unchanged), then its contents set to the value's fill
    /// ([`crate::circuit::strength_items`]). Public for the site's lit stills (a display settled
    /// at a digit: `site::simulate::view::lit_snapshot`).
    pub fn set_level(&mut self, i: usize, value: u8) {
        self.set_strength(i, value);
    }

    /// A strength input's value, like the judge (`circuit::Circuit::actions`): its chest placed
    /// again (its state unchanged), then its contents set to the value's fill
    /// ([`crate::circuit::strength_items`]).
    fn set_strength(&mut self, i: usize, value: u8) {
        let Some(crate::circuit::InputHow::Strength { chest }) = self.circuit_inputs.get(i).map(|c| c.how.clone()) else { return };
        let pos = bp(self.circuit_inputs[i].pos);
        self.world.set_block(pos, chest, flags::ALL);
        if let Err(e) = redsim::redstone::container::apply_block_entities(&mut self.world, [(pos, crate::circuit::strength_items(value).as_str())]) {
            self.world.errors.push(format!("strength input: {e}"));
        }
        self.input_on[i] = value > 0;
        self.last_input = Some(self.tick);
    }

    /// A short pulse on input `i`: on now, off `len` ticks later (a tap on a pulse input).
    pub fn pulse_input(&mut self, i: usize, len: u32) -> bool {
        if !self.set_input(i, true) {
            return false;
        }
        self.releases.push((self.tick + len.max(1), i));
        true
    }

    /// Input `i` goes off at `tick` (a short press released early).
    pub fn release_input_at(&mut self, i: usize, tick: u32) {
        self.releases.push((tick, i));
    }

    /// Replay the judge's stimulus from placement (the auto-demo); its edges due now are applied.
    /// The level a circuit input shows (its bar button): its switch's state, except a driven
    /// input (R7) during the auto-demo shows the stimulus's level at this tick: its switch goes 2
    /// gt before each edge (the input cell's power changes at the edge), and the button lit
    /// early disagreed with the demo badge (critique: A lit while the badge still said "A off").
    pub fn input_level(&self, i: usize) -> bool {
        let on = self.input_on.get(i).copied().unwrap_or(false);
        let driven = matches!(self.circuit_inputs.get(i).map(|c| &c.how), Some(crate::circuit::InputHow::Driven { .. }));
        match (&self.demo, driven) {
            (Some(d), true) => d.edges.get(i).and_then(|e| e.iter().filter(|(at, _)| *at <= self.tick).max_by_key(|(at, _)| *at)).map(|(_, v)| *v).unwrap_or(on),
            _ => on,
        }
    }

    pub fn attach_demo(&mut self, s: crate::circuit::Schedule) {
        self.demo = Some(s);
        self.inputs_due();
    }

    /// The demo's edges and events, and the pending releases due at this tick.
    fn inputs_due(&mut self) {
        let t = self.tick;
        if let Some(d) = self.demo.clone() {
            for (i, e) in d.edges.iter().enumerate() {
                // a driven input's switch goes 2 gt (its repeater's delay) before the edge, so the
                // input cell's power changes at the stimulus tick (`Circuit::actions`)
                let early = if matches!(self.circuit_inputs.get(i).map(|c| &c.how), Some(crate::circuit::InputHow::Driven { .. })) { 2 } else { 0 };
                for &(at, on) in e {
                    if at.saturating_sub(early) == t {
                        self.set_input(i, on);
                    }
                }
            }
            // the judge's frame turns, then its strength values (after the edges, like
            // `Circuit::actions`): a frame's level step is one right-click
            if self.simulated() {
                for &(at, i, _) in &d.levels {
                    if at == t && self.circuit_inputs.get(i).is_some_and(|c| c.how == crate::circuit::InputHow::Frame) {
                        self.turn_frame(i);
                    }
                }
                for &(at, i, value) in &d.levels {
                    if at == t {
                        self.set_strength(i, value);
                    }
                }
            }
            if self.simulated() {
                for (at, p, op) in &d.events {
                    if *at != t {
                        continue;
                    }
                    let q = bp(*p);
                    match op {
                        crate::circuit::EventOp::Place(b) => {
                            self.world.set_block(q, *b, flags::ALL);
                        }
                        crate::circuit::EventOp::Clear => {
                            self.world.set_block(q, Block::Air, flags::ALL);
                        }
                        crate::circuit::EventOp::Shape(b) => {
                            self.world.set_block(q, *b, flags::CLIENTS);
                        }
                        crate::circuit::EventOp::Update => {
                            let src = self.world.get(q).kind();
                            self.world.update_neighbors_at(q, src);
                        }
                    }
                }
            }
        }
        let due: Vec<usize> = self.releases.iter().filter(|(at, _)| *at <= t).map(|(_, i)| *i).collect();
        self.releases.retain(|(at, _)| *at > t);
        for i in due {
            self.set_input(i, false);
        }
    }

    /// The player's right click on a cell (between ticks): levers flip, buttons press. Returns
    /// false when nothing happened.
    pub fn toggle(&mut self, p: Pos) -> bool {
        if !self.simulated() {
            return false;
        }
        let b = self.world.get(bp(p));
        if !matches!(b, Block::Lever { .. } | Block::Button { .. } | Block::Note { .. } | Block::Trapdoor { iron: false, .. } | Block::Door { iron: false, .. }) {
            return false;
        }
        let ok = self.world.use_block(bp(p));
        if ok {
            self.last_input = Some(self.tick);
        }
        ok
    }

    /// Is a component "on" now (a lever's or button's `powered`, a lamp's `lit`...)?
    pub fn is_on(&self, p: Pos) -> bool {
        if let Some(r) = &self.replay {
            return r.state_at(p).is_some_and(active_state);
        }
        self.simulated() && active(&self.world.get(bp(p)))
    }

    /// Is the output at `o` powered this tick (redsim's state)? Its own power state when it has
    /// one (a lamp lit, dust with power, a powered repeater or observer, an extended piston), else
    /// a redstone signal into its cell (a big dripleaf, a solid block); air never is. Its OUT box
    /// is red while it is.
    pub fn output_powered(&self, o: Pos) -> bool {
        if !self.simulated() || self.replay.is_some() {
            return self.is_on(o);
        }
        let b = self.world.get(bp(o));
        if matches!(b, Block::Air) {
            return false;
        }
        // the clock judge's lamp: the power delivered into its cell (what dust or a repeater
        // there would get; the lamp's own 4 gt off-delay is the lamp's, not the clock's)
        if self.judge_lamp == Some(o) {
            return self.world.best_neighbor_signal(bp(o)) > 0;
        }
        powered_by_state(&b.to_string()).unwrap_or_else(|| self.world.has_neighbor_signal(bp(o)))
    }

    /// Whether a view of this session looks for a short cycle to play it slowed
    /// ([`Session::short_cycle`]: a machine whose blocks repeat in under 10 gt flickers at 1x):
    /// not a circuit driven by inputs, a door, a TNT design (its point is the flight and the blast),
    /// nor an item sorter, smelter or brewer (their judge's stream moves at a hopper's pace).
    pub fn wants_cycle_check(&self) -> bool {
        // (a crop farm grows by random ticks: no short cycle; its look took a big farm's wake
        // 300+ ms natively)
        self.circuit_inputs.is_empty() && !self.mode.is_door() && !is_tnt_design(&self.design) && !crate::circuit::is_sorter(&self.design) && !crate::circuit::is_smelter(&self.design) && !crate::circuit::is_brewer(&self.design) && !crate::circuit::is_composter(&self.design) && !crate::circuit::is_harvester(&self.design)
    }

    /// The shortest cycle, 2 to `max_p` ticks, the blocks repeat in over the second half of the
    /// first `ticks` ticks of the run as placed (redsim's own run, from a copy). None: they stand
    /// still, cycle slower, or redsim stops. A fast machine starts in slow motion.
    pub fn short_cycle(&self, max_p: u32, ticks: u32) -> Option<u32> {
        crate::staged::block_on(self.short_cycle_staged(max_p, ticks, &crate::staged::Budget::none()))
    }

    /// [`Session::short_cycle`] as staged work: it pauses after every tick when the budget's time
    /// is up (the browser's wake spreads it over frames).
    pub async fn short_cycle_staged(&self, max_p: u32, ticks: u32, b: &crate::staged::Budget) -> Option<u32> {
        if !self.simulated() || self.replay.is_some() || self.initial.is_none() {
            return None;
        }
        let mut f = self.fresh();
        b.pause().await;
        let mut h = Vec::new();
        for _ in 0..ticks {
            h.push(frame_hash(0xcbf29ce484222325, &f.machine_blocks()));
            f.step();
            if f.halted.is_some() {
                return None;
            }
            b.pause().await;
        }
        let n = h.len();
        let from = n - n / 2;
        if h[from..].iter().all(|x| *x == h[n - 1]) {
            return None;
        }
        (2..=max_p).find(|&p| (from..n).all(|t| t >= p as usize && h[t] == h[t - p as usize]))
    }

    /// Is everything this session draws redsim's own state for this tick? Every still block drawn
    /// is the world's block in that cell (a quartz-like block simulated as stone drawn under its
    /// own name), nothing is drawn in an air cell, and the drawn entities are exactly redsim's
    /// live entities and falling blocks, each at redsim's position (a minecart on a rail where the
    /// game's renderer draws it, `AbstractMinecartRenderer.oldRender`: on the rail's line, from
    /// redsim's position). Moving blocks are drawn from redsim's block entities (not compared
    /// cell by cell); markers and outlines are annotations, not blocks. Err: the first difference.
    pub fn drawn_matches_simulation(&self) -> Result<(), String> {
        use redsim::entity::EntityKind;
        if !self.simulated() || self.replay.is_some() {
            return Ok(());
        }
        let w = &self.world;
        let mut still: HashMap<Pos, String> = HashMap::new();
        for b in self.render_blocks(0.0) {
            if b.offset != [0.0; 3] || matches!(w.get(bp(b.cell)), Block::MovingPiston { .. }) {
                continue;
            }
            if let Some(prev) = still.insert(b.cell, b.state.clone()) {
                return Err(format!("{:?} drawn twice ({prev}, {})", b.cell, b.state));
            }
        }
        let floor = self.floor.as_ref();
        // every block drawn is redsim's block there (a ground cell is drawn by the ground alone)
        let want = |c: Pos, wb: &Block| match (wb, self.overlay.get(&c)) {
            (Block::Simple(redsim::block::Simple::Stone), Some(name)) => name.clone(),
            _ => wb.to_string(),
        };
        for (c, s) in &still {
            if floor.is_some_and(|f| f.contains(*c)) {
                return Err(format!("{c:?} drawn twice ({s}, ground)"));
            }
            let wb = w.get(bp(*c));
            if wb == Block::Air {
                return Err(format!("{c:?}: {s} drawn where redsim has air"));
            }
            if *s != want(*c, &wb) {
                return Err(format!("{c:?}: drawn {s}, redsim has {}", want(*c, &wb)));
            }
        }
        // and every block of redsim's world is drawn: the ground (a TNT duper's, drawn on its
        // own) cell by cell as it draws it (its layer's block or what changed there), any other
        // block among the blocks drawn
        for y in 0..w.size[1] {
            for z in 0..w.size[2] {
                for x in 0..w.size[0] {
                    let q = w.min.offset(x, y, z);
                    let wb = w.get(q);
                    if let Some(f) = floor
                        && f.contains(q.arr())
                    {
                        let drawn = f.now(q.arr());
                        if drawn != wb {
                            return Err(format!("{:?}: the ground drawn as {drawn}, redsim has {wb}", q.arr()));
                        }
                        continue;
                    }
                    if !matches!(wb, Block::Air | Block::MovingPiston { .. }) && !still.contains_key(&q.arr()) {
                        return Err(format!("{:?}: redsim has {}, nothing drawn", q.arr(), want(q.arr(), &wb)));
                    }
                }
            }
        }
        let drawn = self.entities(1.0);
        let live: Vec<_> = w.entities.list.iter().filter(|e| !e.removed).collect();
        let falling: Vec<_> = w.gravity.entities.iter().filter(|f| !f.removed).collect();
        if drawn.len() != live.len() + falling.len() + w.frames.len() {
            return Err(format!("{} entities drawn, redsim has {} entities, {} falling blocks and {} item frames", drawn.len(), live.len(), falling.len(), w.frames.len()));
        }
        let near = |a: [f32; 3], b: [f64; 3]| (0..3).all(|k| (a[k] as f64 - b[k]).abs() < 1e-3);
        for (d, e) in drawn.iter().zip(&live) {
            let p = [e.pos.x, e.pos.y, e.pos.z];
            let want = match &e.kind {
                EntityKind::Minecart(_) => crate::entitymodel::minecart_pose(w, p, e.y_rot, e.x_rot).0,
                _ => p,
            };
            if !near(d.pos, want) {
                return Err(format!("entity {} ({}) drawn at {:?}, redsim has it at {want:?}", e.id, d.kind, d.pos));
            }
            let kind_ok = match &e.kind {
                EntityKind::Item(_) => d.kind.starts_with("item:"),
                EntityKind::PrimedTnt(_) => d.kind.starts_with("primed:"),
                _ => !d.kind.starts_with("item:") && !d.kind.starts_with("primed:") && !d.kind.starts_with("falling:"),
            };
            if !kind_ok {
                return Err(format!("entity {} drawn as {}", e.id, d.kind));
            }
        }
        for (d, f) in drawn[live.len()..].iter().zip(&falling) {
            if !d.kind.starts_with("falling:") || !near(d.pos, [f.x, f.y, f.z]) {
                return Err(format!("falling block {} drawn as {} at {:?}, redsim has it at {:?}", f.id, d.kind, d.pos, [f.x, f.y, f.z]));
            }
        }
        // the item frames, each at its cell facing its way with its item's turn
        for (d, f) in drawn[live.len() + falling.len()..].iter().zip(&w.frames) {
            let want = format!("frame:{}:{}:", f.facing.name(), f.rotation);
            if !d.kind.starts_with(&want) || (d.kind.len() > want.len()) != f.item || d.pos != frame_position(f) {
                return Err(format!("item frame at {:?} drawn as {} at {:?}, redsim has {f:?}", f.pos, d.kind, d.pos));
            }
        }
        Ok(())
    }

    /// The simulated entities to draw, each at its position `partial` of the way through the
    /// current tick (from where it started the tick), like the game interpolates them.
    pub fn entities(&self, partial: f32) -> Vec<crate::mesh::EntityDraw> {
        use redsim::entity::EntityKind;
        let lerp = |a: f64, b: f64| (a + (b - a) * partial as f64) as f32;
        // the design's entities by id: item frames take none (redsim keeps them as comparator
        // inputs, `World::frames`)
        let specs: Vec<&crate::design::Entity> = self.design.entities.iter().filter(|d| !d.kind.ends_with("item_frame")).collect();
        self.world
            .entities
            .list
            .iter()
            .filter(|e| !e.removed)
            .map(|e| {
                // the design's own type id (its wood, ...) for the entities it summons (spawned
                // first, in file order), else the kind's
                let kind = match specs.get(e.id as usize) {
                    Some(d) => d.kind.clone(),
                    None => match &e.kind {
                        EntityKind::Boat(b) if b.chest => "minecraft:oak_chest_boat".to_string(),
                        EntityKind::Boat(_) => "minecraft:oak_boat".to_string(),
                        EntityKind::Minecart(m) if m.hopper => "minecraft:hopper_minecart".to_string(),
                        EntityKind::Minecart(_) => "minecraft:minecart".to_string(),
                        EntityKind::Item(_) => String::new(),
                        EntityKind::ArmorStand(_) => "minecraft:armor_stand".to_string(),
                        EntityKind::PrimedTnt(_) => "minecraft:tnt".to_string(),
                    },
                };
                let kind = match &e.kind {
                    EntityKind::Item(it) => format!("item:{}", it.item),
                    _ => kind,
                };
                let yaw = e.y_rot_o + (e.y_rot - e.y_rot_o) * partial;
                let pitch = e.x_rot_o + (e.x_rot - e.x_rot_o) * partial;
                let pos = [lerp(e.xo, e.pos.x), lerp(e.yo, e.pos.y), lerp(e.zo, e.pos.z)];
                let age = e.tick_count as f32 + partial;
                if let EntityKind::PrimedTnt(st) = &e.kind {
                    // TntRenderer: the fuse as drawn (getFuse() - partialTicks + 1) flashes and swells it
                    return crate::mesh::EntityDraw { kind: "primed:minecraft:tnt[unstable=false]".to_string(), pos, fuse: st.fuse as f32 - partial + 1.0, ..Default::default() };
                }
                if let EntityKind::Minecart(_) = &e.kind {
                    // on a rail: the rail's line and slope (AbstractMinecartRenderer.oldRender)
                    let p64 = [e.xo + (e.pos.x - e.xo) * partial as f64, e.yo + (e.pos.y - e.yo) * partial as f64, e.zo + (e.pos.z - e.zo) * partial as f64];
                    let (p, rot, xrot) = crate::entitymodel::minecart_pose(&self.world, p64, yaw, pitch);
                    return crate::mesh::EntityDraw { kind, pos: [p[0] as f32, p[1] as f32, p[2] as f32], yaw: rot, pitch: xrot, age, ..Default::default() };
                }
                crate::mesh::EntityDraw { kind, pos, yaw, pitch, age, ..Default::default() }
            })
            .chain(self.world.gravity.entities.iter().filter(|f| !f.removed).map(|f| {
                // a falling block between where it was before the tick and where it is (one that
                // appeared in the tick: where it is)
                let was = self.prev_falling.iter().find(|(id, _)| *id == f.id).map(|(_, p)| *p).unwrap_or([f.x, f.y, f.z]);
                crate::mesh::EntityDraw { kind: format!("falling:{}", f.block), pos: [lerp(was[0], f.x), lerp(was[1], f.y), lerp(was[2], f.z)], ..Default::default() }
            }))
            .chain(self.world.frames.iter().map(|f| crate::mesh::EntityDraw { kind: self.frame_kind(f), pos: frame_position(f), ..Default::default() }))
            .collect()
    }

    /// How an item frame is drawn (`mesh::Builder::item_frame`): `frame:<facing>:<turn>:<item>`,
    /// its item the design's (`Item:{id:..}` of its `entity:` line), none when it holds none.
    fn frame_kind(&self, f: &redsim::entity::frame::Frame) -> String {
        let item = if f.item {
            self.design
                .entities
                .iter()
                .filter(|d| d.kind.ends_with("item_frame"))
                .find(|d| (0..3).all(|k| d.pos[k].floor() as i32 == f.pos.arr()[k]))
                .and_then(|d| d.nbt.as_deref())
                .and_then(|n| n.split("Item:").nth(1))
                .and_then(|r| r.split("id:").nth(1))
                .map(|r| r.trim_start().trim_start_matches(['"', '\'']).chars().take_while(|c| *c != '"' && *c != '\'' && *c != ',' && *c != '}').collect::<String>())
                .unwrap_or_else(|| "minecraft:arrow".to_string())
        } else {
            String::new()
        };
        format!("frame:{}:{}:{item}", f.facing.name(), f.rotation)
    }

    /// Every non-air cell of the world box as the judges trace it: (cell, full state = block state
    /// with the comparator strength / container contents the clock judge adds, `wave::with_extra`).
    pub fn cell_states(&self) -> Vec<(Pos, String)> {
        let mut v: Vec<(Pos, String)> = self
            .world
            .blocks()
            .into_iter()
            .map(|(p, b)| {
                let s = b.to_string();
                let full = match be_extra(&self.world, p, &b) {
                    Some(e) => format!("{s}{e}"),
                    None => s,
                };
                (p.arr(), full)
            })
            .collect();
        v.sort();
        v
    }

    /// The containers redsim models (droppers, dispensers, hoppers) with their stacks: (cell,
    /// slot count, (item, count) per non-empty slot).
    pub fn containers(&self) -> Vec<(Pos, usize, Vec<(&'static str, u8)>)> {
        if !self.simulated() || self.replay.is_some() {
            return Vec::new();
        }
        let mut v: Vec<(Pos, usize, Vec<(&'static str, u8)>)> = self
            .world
            .containers
            .map
            .iter()
            .map(|(p, c)| (p.arr(), c.slots.len(), c.slots.iter().flatten().map(|s| (redsim::redstone::container::item_name(s.item), s.count)).collect()))
            .collect();
        v.sort_by_key(|(p, _, _)| *p);
        v
    }

    /// A door's opening: how many of its cells (9, or 4 for a 2x2 door) hold a block (0 = open,
    /// all = closed); None for other designs.
    pub fn door_blocks(&self) -> Option<usize> {
        let Mode::Door { door, along_z, size, .. } = self.mode else { return None };
        Some(door_cells(door, along_z, size).iter().filter(|c| !matches!(self.world.get(bp(**c)), Block::Air)).count())
    }

    /// Nothing pending (no scheduled ticks, events, moving blocks, pending inputs).
    pub fn idle(&self) -> bool {
        self.actions.is_empty() && self.world.idle()
    }

    /// Every visible block at a moment between this tick and the next (`partial` in 0..1):
    /// moving blocks at their interpolated offset, like the client draws them.
    pub fn render_blocks(&self, partial: f32) -> Vec<RBlock> {
        let mut out = Vec::new();
        if let Some(r) = &self.replay {
            return r.render_blocks();
        }
        if !self.simulated() {
            // what redsim could not place is drawn from the design text
            for (p, s) in &self.design.blocks {
                out.push(RBlock { cell: *p, offset: [0.0; 3], state: s.clone() });
            }
            return out;
        }
        let w = &self.world;
        // the machine's part of the world (a TNT duper's ground is drawn on its own: `mesh::floor`),
        // then the blocks redsim wrote outside it (`outside`)
        let (min, size) = self.machine();
        let cell = |q: BlockPos, out: &mut Vec<RBlock>| {
            let b = w.get(q);
            match b {
                Block::Air => {}
                Block::MovingPiston { .. } => {
                    if let Some(be) = w.piston.be(q) {
                        moving_parts(q.arr(), be, partial, out);
                    }
                }
                Block::Simple(redsim::block::Simple::Stone) if self.overlay.contains_key(&q.arr()) => {
                    out.push(RBlock { cell: q.arr(), offset: [0.0; 3], state: self.overlay[&q.arr()].clone() });
                }
                _ => out.push(RBlock { cell: q.arr(), offset: [0.0; 3], state: b.to_string() }),
            }
        };
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let q = min.offset(x, y, z);
                    if self.in_floor(q.arr()) {
                        continue;
                    }
                    cell(q, &mut out);
                }
            }
        }
        for c in self.outside.keys() {
            cell(bp(*c), &mut out);
        }
        out
    }

    /// The box the camera frames: the design and its ports (a door's opening included).
    pub fn frame_box(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = match self.picture {
            Some(b) => b,
            None => self.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0])),
        };
        if let Mode::Door { door, along_z, size, .. } = self.mode {
            for c in door_cells(door, along_z, size) {
                grow(&mut lo, &mut hi, c);
            }
        }
        // a circuit's input cells (often empty: the judge writes a block there) are in the picture
        for ci in &self.circuit_inputs {
            grow(&mut lo, &mut hi, ci.pos);
        }
        (lo, hi)
    }

    /// [`Session::frame_box`] plus a travelling design's whole path, up to where its judge stops
    /// it (the corridor's edge): the floor goes under it.
    pub fn path_box(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.frame_box();
        if self.corridor.is_some()
            && let Some(((a, b), margin)) = travel_box(&self.design, &self.mode)
        {
            grow(&mut lo, &mut hi, a);
            grow(&mut lo, &mut hi, b);
            if let Some(v) = self.design.meta.get("travel").and_then(|t| parse_travel(t)) {
                for i in 0..3 {
                    if v[i] > 0 {
                        hi[i] += margin;
                    } else if v[i] < 0 {
                        lo[i] -= margin;
                    }
                }
            }
        }
        (lo, hi)
    }

    /// A flying machine's end, the way `circuit fly sim` ends a run: the first tick a block stands
    /// on the corridor's edge, or something is destroyed, or nothing is pending (at rest), or the
    /// horizon (200 + 60 gt per corridor cell).
    pub fn fly_end(&self) -> u32 {
        if self.corridor.is_none() {
            return 0;
        }
        let mut s = self.fresh();
        loop {
            if s.at_fly_end() {
                return s.tick;
            }
            s.step();
        }
    }

    /// A flying machine whose end the view finds as it flies (`Options::probe` false: the
    /// browser; or not found yet): kicked by itself, in its judge's corridor.
    pub fn fly_end_due(&self) -> bool {
        self.end_tick.is_none() && self.corridor.is_some() && self.design.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine")) && !self.starts_on_input()
    }

    /// Is a flying machine's run over at this tick ([`Session::fly_end`]'s test, the judge's
    /// stops: something broke or dropped, redsim met what it does not model, a block reached the
    /// corridor's edge, everything came to rest, or the judge's horizon, 200 + 60 gt a block of
    /// its distance)?
    pub fn at_fly_end(&self) -> bool {
        let Some((lo, hi)) = self.corridor else { return true };
        let k = self.design.meta.get("distance").and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(32).max(1);
        let horizon = 200 + 60 * k;
        let edge = |p: BlockPos| p.x <= lo[0] || p.y <= lo[1] || p.z <= lo[2] || p.x >= hi[0] || p.y >= hi[1] || p.z >= hi[2];
        !self.world.drops.is_empty() || !self.world.errors.is_empty() || self.world.blocks().iter().any(|(p, _)| edge(*p)) || (self.actions.is_empty() && self.world.idle()) || self.tick >= horizon
    }

    /// A digest of every frame (the non-air cells, sorted) of ticks 0..=`ticks` from placement:
    /// `mctas web check` compares it with `circuit fly sim`'s frames and with the shipped wasm's.
    pub fn frames_digest(&self, ticks: u32) -> u64 {
        let mut s = self.fresh();
        let mut h: u64 = 0xcbf29ce484222325;
        for _ in 0..=ticks {
            // a run redsim stops following (an error: something it does not model) is shown up to
            // its last exact frame
            if s.halted.is_some() {
                break;
            }
            h = frame_hash(h, &s.world.blocks());
            s.step();
        }
        h
    }

    /// [`Session::frames_digest`] with every entity too ([`entity_hash`]: its kind and the bits of
    /// its position and motion): `web check`'s TNT designs.
    pub fn frames_entities_digest(&self, ticks: u32) -> u64 {
        let mut s = self.fresh();
        let mut h: u64 = 0xcbf29ce484222325;
        for _ in 0..=ticks {
            if s.halted.is_some() {
                break;
            }
            h = frame_hash(h, &s.world.blocks());
            h = entity_hash(h, &s.world);
            s.step();
        }
        h
    }

    /// Run `n` ticks (stops early at an error).
    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }
}

/// One block to draw: its cell, an offset in blocks (a moving block), its state string.
#[derive(Clone, Debug, PartialEq)]
pub struct RBlock {
    pub cell: Pos,
    pub offset: [f32; 3],
    pub state: String,
}

/// A moving block entity as the client's `PistonHeadRenderer` draws it.
fn moving_parts(cell: Pos, be: &MovingBe, partial: f32, out: &mut Vec<RBlock>) {
    let p = (be.progress_o as f32 + (be.progress as f32 - be.progress_o as f32) * partial.clamp(0.0, 1.0)) / 2.0;
    let p = p.min(1.0);
    let ext = if be.extending { p - 1.0 } else { 1.0 - p };
    let st = be.direction.step();
    let off = [st[0] as f32 * ext, st[1] as f32 * ext, st[2] as f32 * ext];
    match be.moved {
        Block::PistonHead { sticky, facing, .. } => {
            out.push(RBlock { cell, offset: off, state: Block::PistonHead { sticky, facing, short: p <= 0.5 }.to_string() });
        }
        Block::Piston { sticky, facing, .. } if be.source && !be.extending => {
            out.push(RBlock { cell, offset: off, state: Block::PistonHead { sticky, facing, short: p >= 0.5 }.to_string() });
            out.push(RBlock { cell, offset: [0.0; 3], state: Block::Piston { sticky, facing, extended: true }.to_string() });
        }
        m => out.push(RBlock { cell, offset: off, state: m.to_string() }),
    }
}

/// The clock judge's fast 128-bit digest (`crates/clock/src/sim.rs::H128`, copied so the browser
/// finds the same repeats).
pub struct H128 {
    pub a: u64,
    pub b: u64,
}

impl Default for H128 {
    fn default() -> Self {
        H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 }
    }
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

/// A world's whole-state digest (`clock::sim::digest`).
pub fn digest(w: &World) -> u128 {
    let mut h = H128::default();
    w.state_key_into(&mut h);
    ((h.a as u128) << 64) | h.b as u128
}

/// A clock's exact numbers on redsim, as `mctas clock sim` finds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period {
    /// First tick of the first exact repeat of the whole state.
    pub state_warmup: u32,
    /// Distance to the repeat.
    pub state_period: u32,
    /// The visible period: the smallest divisor of the state period over which every cell (block
    /// state + comparator strength + container contents) repeats.
    pub period: u32,
    /// The first tick from which the visible state repeats.
    pub warmup: u32,
}

/// Comparator strength and container contents: the clock judge's `wave::be_extra_redsim`.
/// A design whose placement transient settles before its first frame ([`Session::new`]): a
/// circuit redsim runs, but a flying machine (it moves from its kick), a design with a kick, a
/// shulker design or sorter (their judges' own settle), a TNT design (its run starts at once).
fn settles_at_rest(s: &Session) -> bool {
    let kind = s.design.meta.get("circuit").map(|k| k.split_whitespace().next().unwrap_or("").to_string());
    s.simulated() && s.replay.is_none() && kind.as_deref().is_some_and(|k| !k.starts_with("flying-machine")) && s.actions.is_empty() && s.shulker.is_none() && !is_tnt_design(&s.design)
}

/// Where an item frame entity is (`HangingEntity`: its cell's centre moved 0.46875 back toward
/// the block it hangs on; the renderer moves it back to the centre).
pub fn frame_position(f: &redsim::entity::frame::Frame) -> [f32; 3] {
    let st = f.facing.step();
    let c = f.pos.arr();
    [0, 1, 2].map(|k| c[k] as f32 + 0.5 - st[k] as f32 * 0.46875)
}

fn be_extra(w: &World, p: BlockPos, b: &Block) -> Option<String> {
    match b {
        Block::Comparator { .. } => Some(format!("{{out={}}}", w.comparator_out.get(&p).copied().unwrap_or(0))),
        Block::Hopper { .. } | Block::Dispenser { .. } | Block::Crafter { .. } => {
            let c = w.containers.map.get(&p);
            let mut list: Vec<String> = Vec::new();
            if let Some(c) = c {
                for (slot, st) in c.slots.iter().enumerate() {
                    if let Some(st) = st {
                        list.push(format!("{slot}:{}:{}", st.count, redsim::redstone::container::item_name(st.item).trim_start_matches("minecraft:")));
                    }
                }
            }
            let cd = if matches!(b, Block::Hopper { .. }) { format!(",cd={}", c.map(|c| c.cooldown).unwrap_or(-1)) } else { String::new() };
            Some(format!("{{items={}{cd}}}", list.join(",")))
        }
        _ => None,
    }
}

/// The machine's visible state this tick (every cell of its box with its comparator strength or
/// container contents; a TNT duper's ground left out, like its judge's frames).
fn frame_digest(s: &Session) -> u128 {
    use std::hash::Hash;
    let mut h = H128::default();
    let w = &s.world;
    let (min, size) = s.machine();
    for y in 0..size[1] {
        for z in 0..size[2] {
            for x in 0..size[0] {
                let q = min.offset(x, y, z);
                if s.in_floor(q.arr()) {
                    continue;
                }
                let b = w.get(q);
                b.hash(&mut h);
                be_extra(w, q, &b).hash(&mut h);
            }
        }
    }
    ((h.a as u128) << 64) | h.b as u128
}

impl Session {
    /// The tick from which the clock runs with no input (the end of its kick).
    pub fn free_from(&self) -> u32 {
        self.initial.as_ref().map(|(_, a, _)| a.iter().map(|(t, _)| *t).max().unwrap_or(0)).unwrap_or(0)
    }

    /// The clock judge's exact period detector on a fresh copy of the placed state (the session
    /// itself is not moved): first exact repeat of the whole state, then the visible period.
    pub fn period(&self, horizon: u32) -> Result<Period, String> {
        if let Some((h, r)) = self.period_memo.borrow().as_ref()
            && *h == horizon
        {
            return r.clone();
        }
        let r = self.period_of(horizon);
        *self.period_memo.borrow_mut() = Some((horizon, r.clone()));
        r
    }

    fn period_of(&self, horizon: u32) -> Result<Period, String> {
        if let Some(why) = &self.unsimulated {
            return Err(format!("not modeled by redsim: {why}"));
        }
        let end = self.free_from();
        let found = if !self.design.entities.is_empty() || self.world.tnt_kick.is_some() {
            // designs with entities never repeat their exact state (an entity's doubles drift;
            // a TNT design's TNT keep coming and exploding): the real-game judge's rule
            // (`clock::sim::entity_clock_period`): the block frames over 4000 gt, periodic for 3
            // periods from the first free tick
            let mut s = self.fresh();
            let mut ids: HashMap<u128, u64> = HashMap::new();
            let mut frames: Vec<u64> = Vec::with_capacity(ENTITY_HORIZON as usize + 1);
            for t in 0..=ENTITY_HORIZON {
                s.apply_due();
                s.check(t)?;
                let d = frame_digest(&s);
                let n = ids.len() as u64;
                frames.push(*ids.entry(d).or_insert(n));
                if t < ENTITY_HORIZON {
                    s.world.tick();
                    s.tick += 1;
                }
            }
            Some(detect_period(&frames, end as usize, 3).map(|(w, p)| (w as u32, p as u32)).ok_or_else(|| format!("not periodic for 3 periods within {ENTITY_HORIZON} gt (the judge's rule for designs with entities)"))?)
        } else {
            None
        };
        let mut s = self.fresh();
        let mut seen: HashMap<u128, u32> = HashMap::new();
        let mut found = found;
        for t in 0..=horizon {
            if found.is_some() {
                break;
            }
            s.apply_due();
            s.check(t)?;
            if t >= end {
                let d = digest(&s.world);
                if let Some(&t0) = seen.get(&d) {
                    found = Some((t0, t - t0));
                    break;
                }
                seen.insert(d, t);
            }
            s.world.tick();
            s.tick += 1;
        }
        let (w0, p) = found.ok_or_else(|| format!("no repeat of its state within {horizon} gt"))?;
        let mut s = self.fresh();
        let mut frames: Vec<u128> = Vec::with_capacity((w0 + p + 1) as usize);
        for t in 0..=(w0 + p) {
            s.apply_due();
            s.check(t)?;
            frames.push(frame_digest(&s));
            if t == w0 + p {
                break;
            }
            s.world.tick();
            s.tick += 1;
        }
        let (w0u, pu) = (w0 as usize, p as usize);
        let full_p = (1..=pu).filter(|d| pu % d == 0).find(|&d| (w0u..w0u + pu).all(|t| frames[t] == frames[w0u + (t - w0u + d) % pu])).unwrap_or(pu);
        let mut vis_w = w0u;
        while vis_w > end as usize && frames[vis_w - 1] == frames[vis_w - 1 + full_p] {
            vis_w -= 1;
        }
        Ok(Period { state_warmup: w0, state_period: p, period: full_p as u32, warmup: vis_w as u32 })
    }

    /// Where the entities and falling blocks go over the first `ticks` ticks of the run as placed
    /// (redsim's own run, from a copy): for framing the build's whole moving extent.
    pub fn run_extent(&self, ticks: u32) -> Vec<[f32; 3]> {
        if !self.simulated() || self.replay.is_some() || self.initial.is_none() {
            return Vec::new();
        }
        let mut f = self.fresh();
        let mut pts = Vec::new();
        for _ in 0..ticks {
            f.step();
            if f.halted.is_some() {
                break;
            }
            pts.extend(f.entities(1.0).iter().map(|e| e.pos));
        }
        pts
    }

    /// A copy back at placement (the inputs due at its tick applied).
    fn fresh(&self) -> Session {
        let (w, a, t) = self.initial.clone().expect("simulated session");
        let mut s = Session {
            design: Design::default(),
            mode: self.mode.clone(),
            world: w,
            tick: t,
            rest_tick: self.rest_tick,
            last_input: None,
            actions: a,
            unsimulated: None,
            overlay: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            initial: None,
            replay: None,
            corridor: self.corridor,
            end_tick: None,
            circuit_inputs: Vec::new(),
            input_on: Vec::new(),
            demo: None,
            releases: Vec::new(),
            picture: None,
            consumers: Vec::new(),
            judge_lamp: self.judge_lamp,
            prev_falling: Vec::new(),
            halted: None,
            shulker: self.shulker_initial.clone(),
            shulker_initial: None,
            visit_due: false,
            judge_every: self.judge_every,
            judge_from: self.judge_from,
            farm: self.farm.clone(),
            received: Default::default(),
            floor: self.floor.clone(),
            machine_box: self.machine_box,
            outside: Default::default(),
            period_memo: Default::default(),
            still: self.still,
        };
        // the world as placed: its ground intact
        if let Some(f) = s.floor.as_mut() {
            f.changed.clear();
        }
        s.apply_due();
        s.take_writes();
        s
    }

    /// The clock judge's validity checks between ticks.
    fn check(&self, t: u32) -> Result<(), String> {
        if let Some(e) = self.world.errors.first() {
            if e.contains("outside the box") {
                return Err(format!("INVALID: escapes its box by more than the margin by tick {t}: {e}"));
            }
            return Err(format!("UNMODELED: tick {t}: {e}"));
        }
        if let Some((p, b)) = self.world.drops.first() {
            return Err(format!("INVALID: tick {t}: {b} at {p} was destroyed with a drop (an item entity)"));
        }
        Ok(())
    }

    /// The clock's (warmup, period) for pictures: the exact detector like `clock::sim::activity`
    /// (whole-state warmup, visible period), else the archive's judge line.
    pub fn picture_period(&self, horizon: u32) -> Option<(u32, u32)> {
        if !self.simulated() || self.mode.is_door() {
            return None;
        }
        match self.period(horizon) {
            Ok(r) if r.period > 0 => Some((r.state_warmup.max(r.warmup), r.period)),
            _ => self.design.clock_hint().filter(|(w, p)| w + p <= horizon && *p > 0),
        }
    }

    /// One period from `w0`: every changing cell and its state at each tick (two passes on fresh
    /// copies; the session is not moved).
    pub fn period_cells(&self, w0: u32, p: u32) -> Vec<(Pos, Vec<Block>)> {
        let mut s = self.fresh();
        while s.tick < w0 {
            s.step();
        }
        let first = cells_of(&s.world);
        let mut changing = vec![false; first.len()];
        for _ in 0..p {
            for (i, b) in cells_of(&s.world).iter().enumerate() {
                if *b != first[i] {
                    changing[i] = true;
                }
            }
            s.step();
        }
        let idx: Vec<usize> = (0..first.len()).filter(|&i| changing[i]).collect();
        let w = &s.world;
        let pos_of = |i: usize| -> Pos {
            let (sx, sz) = (w.size[0] as usize, w.size[2] as usize);
            let (x, z, y) = (i % sx, (i / sx) % sz, i / (sx * sz));
            [w.min.x + x as i32, w.min.y + y as i32, w.min.z + z as i32]
        };
        let mut out: Vec<(Pos, Vec<Block>)> = idx.iter().map(|&i| (pos_of(i), Vec::with_capacity(p as usize))).collect();
        let mut s = self.fresh();
        while s.tick < w0 {
            s.step();
        }
        for _ in 0..p {
            let cells = cells_of(&s.world);
            for (k, &i) in idx.iter().enumerate() {
                out[k].1.push(cells[i]);
            }
            s.step();
        }
        out
    }

    /// The clock judge's output when none is declared: the changing cell with the longest own
    /// period, then the most rising edges, then the most ticks on (`clock::sim::simulate`).
    pub fn auto_output(cells: &[(Pos, Vec<Block>)]) -> Option<Pos> {
        let stat = |seq: &[Block]| {
            let n = seq.len();
            let period = (1..=n.max(1)).filter(|d| n % d == 0).find(|&d| (0..n).all(|t| seq[t] == seq[(t + d) % n])).unwrap_or(n.max(1));
            let on: Vec<bool> = seq.iter().map(active).collect();
            let edges = (0..n).filter(|&t| on[t] && !on[(t + n - 1) % n]).count();
            let ons = on.iter().filter(|b| **b).count();
            (period, edges, ons)
        };
        cells.iter().max_by(|a, b| stat(&a.1).cmp(&stat(&b.1)).then(b.0.cmp(&a.0))).map(|(p, _)| *p)
    }

    /// The still frame a thumbnail shows: a door at rest (the settled closed state); a clock at
    /// the most active moment of one period (most changing cells on, nothing moving if
    /// possible), like `mctas clock shot` picks it. Leaves the session at that tick and records the
    /// clock judge's output when the design declares none.
    pub fn go_to_still(&mut self, horizon: u32) -> u32 {
        // a travelling design (flying machine, extender) is pictured as built: no period to find
        if self.corridor.is_some() {
            return self.tick;
        }
        let Some((w0, p)) = self.picture_period(horizon) else { return self.tick };
        let cells = self.period_cells(w0, p);
        if self.outputs.is_empty()
            && let Some(o) = Session::auto_output(&cells)
        {
            self.outputs.push(o);
        }
        self.picture = self.picture_box(horizon);
        let mut best = (0u32, (std::cmp::Reverse(usize::MAX), 0usize));
        for t in 0..p as usize {
            let on = cells.iter().filter(|(_, seq)| active(&seq[t])).count();
            let moving = cells.iter().filter(|(_, seq)| matches!(seq[t], Block::MovingPiston { .. })).count();
            let score = (std::cmp::Reverse(moving), on);
            if t == 0 || score > best.1 {
                best = (t as u32, score);
            }
        }
        let target = w0 + best.0;
        self.reset();
        while self.tick < target {
            self.step();
        }
        target
    }

    /// What [`Session::go_to_still`] finds, as a thumbnail's `data-still` attribute carries it
    /// (the site computes it at build time: the browser then runs one pass to the tick instead
    /// of finding the period again, most of a long clock's thumbnail time on a phone):
    /// `<tick>`, then ` out=x,y,z` (the clock judge's output it adds when the design declares
    /// none) and ` box=x0,y0,z0,x1,y1,z1` (the picture box). Leaves the session at the still.
    pub fn still_attr(&mut self, horizon: u32) -> String {
        let had = self.outputs.len();
        let t = self.go_to_still(horizon);
        let mut a = t.to_string();
        if let Some(o) = self.outputs.get(had) {
            a += &format!(" out={},{},{}", o[0], o[1], o[2]);
        }
        if let Some((lo, hi)) = self.picture {
            a += &format!(" box={},{},{},{},{},{}", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
        }
        a
    }

    /// [`Session::go_to_still`] from its [`Session::still_attr`] answer: the output and picture
    /// box set, one run to the tick. An attribute it cannot read changes nothing (Err).
    pub fn go_to_still_from(&mut self, attr: &str) -> Result<u32, String> {
        let tick = self.take_still_marks(attr)?;
        self.reset();
        while self.tick < tick && self.halted.is_none() {
            self.step();
        }
        Ok(tick)
    }

    /// The marks of a [`Session::still_attr`] answer, no tick run (the browser: a still, a view
    /// before it plays): the output cell (when the design names none) and the picture box. Its
    /// tick (Err: an attribute it cannot read, nothing changed).
    pub fn take_still_marks(&mut self, attr: &str) -> Result<u32, String> {
        let nums = |v: &str, n: usize| -> Result<Vec<i32>, String> {
            let v: Vec<i32> = v.split(',').map(|x| x.trim().parse::<i32>().map_err(|_| format!("data-still: {x}?"))).collect::<Result<_, _>>()?;
            if v.len() == n { Ok(v) } else { Err(format!("data-still: {n} numbers wanted, {v:?}")) }
        };
        let mut it = attr.split_whitespace();
        let tick: u32 = it.next().and_then(|t| t.parse().ok()).ok_or_else(|| format!("data-still: no tick in {attr:?}"))?;
        let (mut out, mut picture) = (None, None);
        for w in it {
            if let Some(v) = w.strip_prefix("out=") {
                let v = nums(v, 3)?;
                out = Some([v[0], v[1], v[2]]);
            } else if let Some(v) = w.strip_prefix("box=") {
                let v = nums(v, 6)?;
                picture = Some(([v[0], v[1], v[2]], [v[3], v[4], v[5]]));
            } else {
                return Err(format!("data-still: {w}?"));
            }
        }
        if let Some(o) = out
            && self.outputs.is_empty()
        {
            self.outputs.push(o);
        }
        if picture.is_some() {
            self.picture = picture;
        }
        Ok(tick)
    }

    /// A clock's picture box, like `clock shot`: the judge's swept box (`clock::sim`: the design's
    /// box with its kick cell, grown by every cell occupied over the state warmup and one state
    /// period), and for an elongated clock (its longest side over 16 blocks and over 5x the next) a
    /// window of it along that side around the output, 3x the next side long (8 to 16 blocks).
    pub fn picture_box(&self, horizon: u32) -> Option<(Pos, Pos)> {
        if !self.simulated() || self.mode.is_door() || self.corridor.is_some() || self.replay.is_some() {
            return None;
        }
        let r = self.period(horizon).ok()?;
        let (mut lo, mut hi) = self.design.bounds()?;
        if let Mode::Clock { kick: Some((k, _)), .. } = &self.mode {
            let p = match k {
                Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
            };
            grow(&mut lo, &mut hi, p);
        }
        let mut s = self.fresh();
        let end = r.state_warmup + r.state_period;
        loop {
            for (q, _) in s.machine_blocks() {
                grow(&mut lo, &mut hi, q.arr());
            }
            if s.tick >= end {
                break;
            }
            s.step();
        }
        let ext = |lo: &Pos, hi: &Pos, k: usize| hi[k] - lo[k] + 1;
        let mut axes = [0usize, 1, 2];
        axes.sort_by_key(|&k| std::cmp::Reverse(ext(&lo, &hi, k)));
        let (long, next) = (axes[0], axes[1]);
        if ext(&lo, &hi, long) > 16 && ext(&lo, &hi, long) > 5 * ext(&lo, &hi, next) {
            let win = (3 * ext(&lo, &hi, next)).clamp(8, 16);
            let c = self.outputs.first().map(|o| o[long]).unwrap_or((lo[long] + hi[long]) / 2);
            let a = (c - win / 2).clamp(lo[long], hi[long] - win + 1);
            lo[long] = a;
            hi[long] = a + win - 1;
        }
        Some((lo, hi))
    }

    /// Find the clock judge's output (no `port output`) without moving the session.
    pub fn find_auto_output(&mut self, horizon: u32) {
        // a circuit has its outputs as ports (a TNT duper has none: no marker)
        if !self.outputs.is_empty() || self.corridor.is_some() || self.design.meta.contains_key("circuit") {
            return;
        }
        if let Some((w0, p)) = self.picture_period(horizon)
            && let Some(o) = Session::auto_output(&self.period_cells(w0, p))
        {
            self.outputs.push(o);
        }
    }
}

/// Fold one frame (non-air cells, any order) into a digest: the cells sorted by position, each
/// position and block state hashed ([`H128`]: the same on the native and the wasm build), then
/// `|`. No strings: a TNT duper's ground puts 150,000 cells in every frame.
pub fn frame_hash(h: u64, cells: &[(BlockPos, Block)]) -> u64 {
    use std::hash::Hash;
    let mut v: Vec<&(BlockPos, Block)> = cells.iter().collect();
    v.sort_unstable_by_key(|(p, _)| (p.x, p.y, p.z));
    let mut st = H128 { a: h, b: h.rotate_left(32) ^ 0x84222325cbf29ce4 };
    for (p, b) in v {
        st.write_i32(p.x);
        st.write_i32(p.y);
        st.write_i32(p.z);
        b.hash(&mut st);
    }
    st.write_u8(b'|');
    st.a ^ st.b.rotate_left(13)
}

/// Every live entity (falling blocks included) folded into `h` in spawn order: its kind and the
/// bits of its position and motion (FNV-1a, like [`frame_hash`]).
pub fn entity_hash(mut h: u64, w: &World) -> u64 {
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        eat(format!("{:?}", std::mem::discriminant(&e.kind)).as_bytes());
        for v in [e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z] {
            eat(&v.to_bits().to_le_bytes());
        }
    }
    for f in w.gravity.entities.iter().filter(|f| !f.removed) {
        for v in [f.x, f.y, f.z, f.vy] {
            eat(&v.to_bits().to_le_bytes());
        }
    }
    eat(b"#");
    h
}

/// The block the clock judge puts in a clock's empty output cell (`clock::sim::JUDGE_LAMP`).
pub const JUDGE_LAMP: &str = "minecraft:redstone_lamp[lit=false]";

/// A design with a TNT block: it runs with redsim's TNT entities and explosions
/// (`explosion::explode_on`).
pub fn is_tnt_design(d: &Design) -> bool {
    d.blocks.values().any(|s| s.trim_start_matches("minecraft:").split('[').next() == Some("tnt"))
}

/// A randomizer (`circuit: randomizer N`, study 16): its outcomes come from the level random.
pub fn is_randomizer(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| k.trim_start().starts_with("randomizer"))
}
/// Is a block powered by its own state: a lamp lit, dust with power, a powered repeater or
/// observer, an extended piston, a hopper locked, a sculk sensor active? None: it has no such
/// state (a big dripleaf, a solid block: see [`Session::output_powered`]).
fn powered_by_state(state: &str) -> Option<bool> {
    let st = crate::models::St::parse(state);
    match st.name {
        "hopper" => return Some(st.get("enabled") == Some("false")),
        "sculk_sensor" | "calibrated_sculk_sensor" => return Some(st.get("sculk_sensor_phase") == Some("active")),
        _ => {}
    }
    for k in ["powered", "lit", "extended", "triggered", "open", "attached"] {
        if let Some(v) = st.get(k) {
            return Some(v == "true");
        }
    }
    st.get("power").map(|p| p.parse::<u32>().is_ok_and(|p| p > 0))
}

pub fn active_state(state: &str) -> bool {
    let st = crate::models::St::parse(state);
    match st.name {
        "air" | "cave_air" | "void_air" => return false,
        "piston_head" | "moving_piston" => return true,
        "hopper" => return st.get("enabled") == Some("false"),
        "sculk_sensor" | "calibrated_sculk_sensor" => return st.get("sculk_sensor_phase") == Some("active"),
        "leaves" => return st.get("distance").and_then(|d| d.parse::<u32>().ok()).is_some_and(|d| d < 7),
        _ => {}
    }
    for k in ["powered", "lit", "extended", "open", "triggered", "attached"] {
        if let Some(v) = st.get(k) {
            return v == "true";
        }
    }
    if let Some(p) = st.get("power") {
        return p.parse::<u32>().is_ok_and(|p| p > 0);
    }
    true
}

/// Is a component "on" (`crates/clock/src/wave.rs::active`)?
pub fn active(b: &Block) -> bool {
    match *b {
        Block::Air => false,
        Block::Piston { extended, .. } => extended,
        Block::PistonHead { .. } | Block::MovingPiston { .. } => true,
        Block::Sculk { phase, .. } => phase == 1,
        Block::Hopper { enabled, .. } => !enabled,
        Block::Dispenser { triggered, .. } => triggered,
        Block::Crafter { triggered, .. } => triggered,
        Block::Leaves { distance, .. } => distance < 7,
        Block::Scaffolding { distance, .. } => distance > 0,
        Block::Door { open, .. } | Block::Trapdoor { open, .. } | Block::FenceGate { open, .. } => open,
        Block::Torch { lit } | Block::WallTorch { lit, .. } | Block::Lamp { lit } | Block::Bulb { lit, .. } => lit,
        Block::Wire { power, .. } | Block::Target { power } => power > 0,
        Block::Lever { powered, .. }
        | Block::Button { powered, .. }
        | Block::Repeater { powered, .. }
        | Block::Comparator { powered, .. }
        | Block::Observer { powered, .. }
        | Block::Note { powered, .. }
        | Block::Rail { powered, .. }
        | Block::Skull { powered, .. }
        | Block::WallSkull { powered, .. } => powered,
        _ => true,
    }
}

/// Every cell of the world's box, in storage order.
pub fn cells_of(w: &World) -> Vec<Block> {
    let mut v = Vec::with_capacity((w.size[0] * w.size[1] * w.size[2]) as usize);
    let min = w.min;
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                v.push(w.get(min.offset(x, y, z)));
            }
        }
    }
    v
}

/// `out`, `out_a`, `out2`, `output`, `output_b`...: a port named after `prefix`.
/// The real-game judge's horizon for designs with entities (`clock::sim::ENTITY_HORIZON`).
pub const ENTITY_HORIZON: u32 = 4000;

/// `clock::wave::detect_period`: the smallest period of a sequence of frame ids periodic from some
/// warmup on, seen in `frames[start..]`: per candidate p the earliest W from which
/// `frames[t] == frames[t + p]` holds to the end, with at least `min_periods` periods after it;
/// the smallest W, then the smallest p.
pub fn detect_period(frames: &[u64], start: usize, min_periods: usize) -> Option<(usize, usize)> {
    let n = frames.len();
    if n <= start + 1 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let maxp = (n - start) / min_periods.max(1);
    for p in 1..=maxp {
        let mut w = n - p;
        while w > start && frames[w - 1] == frames[w - 1 + p] {
            w -= 1;
        }
        if n - w < min_periods * p {
            continue;
        }
        if best.is_none_or(|(bw, bp)| (w, p) < (bw, bp)) {
            best = Some((w, p));
        }
    }
    best
}

fn is_port_named(name: &str, prefix: &str) -> bool {
    let long = if prefix == "in" { "input" } else { "output" };
    let rest = name.strip_prefix(long).or_else(|| name.strip_prefix(prefix));
    rest.is_some_and(|r| r.is_empty() || r.starts_with('_') || r.starts_with('-') || r.starts_with('.') || r.chars().all(|c| c.is_ascii_alphanumeric()) && r.chars().next().is_some_and(|c| c.is_ascii_digit() || c.is_ascii_uppercase()))
}

/// Unit step of a direction as f32.
pub fn dir_step(d: Dir) -> [f32; 3] {
    let s = d.step();
    [s[0] as f32, s[1] as f32, s[2] as f32]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The sorter agent's first auto-smelter (study 16), as archived 2026-10-21: a hopper feeding
    /// the furnace's top from the input chest, one its side from the fuel chest, one under it.
    const SMELTER_1: &str = "name: smelter-1\ncircuit: smelter 1\nport items: 0 3 0\nport fuel: 0 2 1\nport output: 0 -1 0\norigin: 0 0 0\npalette:\n  D minecraft:hopper[enabled=true,facing=down]\n  N minecraft:hopper[enabled=true,facing=north]\n  F minecraft:furnace[facing=north,lit=false]\nlayer 0:\n  D\n  .\nlayer 1:\n  F\n  N\nlayer 2:\n  D\n  .\n";

    /// A double input chest the judge places joined (`port items.b:`, sorter's super smelters):
    /// the view sets both halves joined (facing north along x, the first half LEFT when the second
    /// lies east), shows no `ITEMS B` tag or outline of its own (the coordinator 2026-10-22: it sat
    /// on an empty frame), and runs the judge's chest run through it (both halves full of raw iron).
    #[test]
    fn a_double_input_chest_is_one_joined_chest() {
        let text = SMELTER_1.replace("port fuel:", "port items.b: 1 3 0\nport fuel:");
        let d = crate::Design::parse(&text).unwrap();
        assert_eq!(crate::circuit::double_chest(&d, "items"), Some(([0, 3, 0], [1, 3, 0])));
        assert!(crate::circuit::port_tags(&d).iter().all(|(l, ..)| l != "ITEMS B"), "{:?}", crate::circuit::port_tags(&d));
        assert!(crate::circuit::port_tags(&d).iter().any(|(l, p, _)| l == "ITEMS" && *p == [0, 3, 0]));
        // (a super smelter runs its judge's chest run, `circuit::smelter::Scenario::chest`: both
        // halves topped up from one load of 54 stacks of raw iron)
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        assert!(matches!(s.world.get(bp([0, 3, 0])), Block::Chest { facing: Dir::North, kind: 1, .. }), "{}", s.world.get(bp([0, 3, 0])));
        assert!(matches!(s.world.get(bp([1, 3, 0])), Block::Chest { facing: Dir::North, kind: 2, .. }), "{}", s.world.get(bp([1, 3, 0])));
        for half in [[0, 3, 0], [1, 3, 0]] {
            let st = redsim::redstone::shulker::chest_stacks(&s.world, bp(half));
            assert!(st.len() == 27 && st.iter().all(|x| redsim::redstone::container::item_name(x.item) == "raw_iron" && x.count == 64), "half {half:?}: {} stacks", st.len());
        }
        for _ in 0..4400 {
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        let got: Vec<(&str, u32)> = s.received.get(&[0, -1, 0]).into_iter().flatten().map(|e| match e {
            crate::labels::Entry::Items { id, count, .. } => (*id, *count),
            crate::labels::Entry::Boxes { id, n, .. } => (*id, *n),
        }).collect();
        let ingots: u32 = got.iter().filter(|(id, _)| *id == "iron_ingot").map(|(_, n)| n).sum();
        assert!(ingots >= 15 && got.iter().all(|(id, _)| *id == "iron_ingot"), "the furnace smelts the raw iron through the joined chest, one per 200 gt: {got:?}");
    }

    /// A whole minecart line as the site shows it (`circuit: cart-line` over sorter's line-b,
    /// 2026-10-22): its supply and storage chests stand as joined double chests, one ITEMS and
    /// one OUT tag, no test lamp.
    #[test]
    fn a_minecart_line_shows_its_double_chests() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/rail/system/line-b.txt")).unwrap().replace("name: line-b", "name: line-b\ncircuit: cart-line");
        let d = crate::Design::parse(&text).unwrap();
        let tags: Vec<String> = crate::circuit::port_tags(&d).into_iter().map(|(l, ..)| l).collect();
        assert_eq!(tags, vec!["OUT".to_string(), "ITEMS".to_string()]);
        assert!(crate::circuit::consumer_cells(&d).is_empty(), "no test lamp");
        let s = Session::new(&text, &Options::still()).unwrap();
        for (name, _) in [("items", ()), ("output", ())] {
            let (a, b) = crate::circuit::double_chest(&d, name).unwrap();
            let kinds: Vec<u8> = [a, b].iter().map(|p| match s.world.get(bp(*p)) {
                Block::Chest { kind, .. } => kind,
                other => panic!("{name}: {other}"),
            }).collect();
            assert!(kinds.contains(&1) && kinds.contains(&2), "{name}: one double chest, {kinds:?}");
        }
    }

    /// An auto-smelter runs like its judge's base run (the coordinator 2026-10-21: the 3D view
    /// did not run the smelter study): the judge's chests on its ports, its load in the input
    /// chest and its coal in the fuel chest, the output emptied every 20 gt. The furnace lights
    /// (the game's lit model), its screen's fire and arrow move, and every item of the load comes
    /// out smelted, on the run's tally; `web check` compares the frames and the deliveries with
    /// the judge's redsim run.
    #[test]
    fn a_smelter_runs_like_its_judge() {
        use redsim::redstone::container::item_name;
        let mut s = Session::new(SMELTER_1, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let ch = s.shulker.clone().expect("the judge's chests");
        assert_eq!(ch.supplies, vec![bp([0, 2, 1])], "the fuel chest");
        let furnace = bp([0, 1, 0]);
        let (mut lit, mut arrows) = (false, 0);
        for _ in 0..4400 {
            s.step();
            lit |= matches!(s.world.get(furnace), Block::Furnace { lit: true, .. });
            let c = crate::labels::Cooking::of(&s.world.containers.map[&furnace]);
            arrows = arrows.max(c.arrow);
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        assert!(lit, "the furnace lit");
        assert_eq!(arrows, 24, "an item cooked through: the whole arrow");
        let out: Vec<(&str, u32)> = s.received.get(&[0, -1, 0]).into_iter().flatten().map(|e| match e {
            crate::labels::Entry::Items { id, count, .. } => (*id, *count),
            crate::labels::Entry::Boxes { id, n, .. } => (*id, *n),
        }).collect();
        let total: u32 = out.iter().map(|(_, n)| n).sum();
        assert_eq!(total, 18, "every item of the load smelted: {out:?}");
        for want in ["gold_ingot", "stone", "glass", "charcoal", "dried_kelp", "copper_ingot", "iron_nugget"] {
            assert!(out.iter().any(|(id, _)| *id == want), "{want} in {out:?}");
        }
        assert!(out.iter().all(|(id, _)| *id != item_name(redsim::redstone::container::item_id("coal").unwrap())), "no coal in the output");
    }

    /// An auto-brewer runs like its judge's base run (`circuit::brewer::sim_run`): the judge's
    /// bottles, fuel, wart and sugar chests filled once after the settle, the output emptied every
    /// 20 gt. brewer-1 (the sorter agent's first, archived 2026-10-21) brews its four batches: all
    /// twelve bottles come out as potions of swiftness, none as water.
    #[test]
    fn a_brewer_runs_like_its_judge() {
        let text = include_str!("../../../designs/circuits/tracks/brewer/brewer-1.txt");
        let mut s = Session::new(text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let ch = s.shulker.clone().expect("the judge's chests");
        assert_eq!(ch.supplies, vec![bp([0, 4, 1]), bp([1, 4, 2]), bp([2, 5, 2]), bp([1, 6, 0])], "the bottles, fuel, wart and sugar chests");
        for _ in 0..7200 {
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        let out: Vec<(&str, u32)> = s.received.get(&[1, 2, 2]).into_iter().flatten().map(|e| match e {
            crate::labels::Entry::Items { id, count, .. } => (*id, *count),
            crate::labels::Entry::Boxes { id, n, .. } => (*id, *n),
        }).collect();
        assert_eq!(out, vec![("potion[swiftness]", 12)], "every bottle brewed to swiftness: {out:?}");
    }

    /// An auto-composter slice runs like its judge (`circuit::composter::sim_run`; the view shows the
    /// slice alone, its one crop): the level random mirrored from the judge's box, the input chest
    /// topped up from the judge's load, the overflow and bone meal chests emptied every 20 gt. The
    /// slice composts its wheat seeds into bone meal and passes every other item on.
    #[test]
    fn a_composter_slice_runs_like_its_judge() {
        let text = include_str!("../../../designs/circuits/tracks/composter/compost-row.txt");
        let mut s = Session::new(text, &Options { unverified: true, ..Options::default() }).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        for _ in 0..3000 {
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        let got = |p: [i32; 3]| -> Vec<(&str, u32)> {
            s.received.get(&p).into_iter().flatten().map(|e| match e {
                crate::labels::Entry::Items { id, count, .. } => (*id, *count),
                crate::labels::Entry::Boxes { id, n, .. } => (*id, *n),
            }).collect()
        };
        let bone = got([0, -2, 0]);
        assert!(bone.iter().any(|(id, n)| *id == "bone_meal" && *n >= 1), "bone meal collected: {bone:?}");
        let pass = got([1, 3, 0]);
        assert!(pass.iter().all(|(id, _)| *id != "wheat_seeds"), "no wheat seeds in the overflow: {pass:?}");
        assert!(pass.iter().any(|(id, _)| *id == "cobblestone"), "the other items pass on: {pass:?}");
    }

    /// w86's first crop farm (study 16), as archived 2026-10-21: an observer watching the cane's
    /// bottom block, a piston breaking its second block, a hopper into the judge's chest.
    const CANE_8B: &str = "name: cane-8b\ncircuit: harvester sugar_cane\ntile: 5 0 0\nport crop: 0 1 0\nport output: 1 0 0\norigin: -2 0 0\npalette:\n  S minecraft:stone\n  D minecraft:dirt\n  G minecraft:waxed_copper_grate[waterlogged=true]\n  O minecraft:observer[facing=east,powered=false]\n  P minecraft:piston[extended=false,facing=east]\n  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]\n  H minecraft:hopper[enabled=true,facing=down]\nlayer 0:\n  .GD.\nlayer 1:\n  SO.H\nlayer 2:\n  dP..\nlayer 3:\n  ..S.\n";

    /// A crop harvester runs like its judge's run (the coordinator 2026-10-21: the view showed a
    /// lamp on the chest and bare soil): the judge's chest at `port output:`, the cane planted at
    /// `port crop:` (it stands: water in the grate beside its dirt), growth by random ticks at the
    /// judge's speed, every grown block broken by the piston and its item in the chest; nothing
    /// loose, the cane never left standing tall.
    #[test]
    fn a_harvester_grows_and_harvests_like_its_judge() {
        let mut s = Session::new(CANE_8B, &Options { unverified: true, ..Options::default() }).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        assert!(matches!(s.world.get(bp([1, 0, 0])), Block::Chest { .. }), "the judge's chest, not a lamp: {}", s.world.get(bp([1, 0, 0])));
        assert!(matches!(s.world.get(bp([0, 1, 0])), Block::SugarCane { .. }), "the cane planted: {}", s.world.get(bp([0, 1, 0])));
        // the judge's sandbox: the level random mirrored, its chunks' order, randValue as the bridge
        // starts it, the judge's speed after the settle
        assert!(s.world.level_random.is_some() && !s.world.chunk_order.is_empty());
        assert_eq!(s.world.random_ticks.as_ref().map(|r| r.speed), Some(crate::circuit::FARM_RTS));
        // a growth: the bottom block's age drops (it restarts at 0 as the block above grows), as the
        // judge counts them (a block broken within the tick it grew still counts)
        let age = |s: &Session| match s.world.get(bp([0, 1, 0])) {
            Block::SugarCane { age } => age,
            b => panic!("tick {}: the cane's bottom block stays, not {b}", s.tick),
        };
        let (mut grown, mut tall_for, mut tall_max) = (0, 0, 0);
        // past the judge's first intruder drop (a cobblestone on the hopper's open top and above the
        // cane, a quarter into its run)
        for _ in 0..crate::circuit::FARM_RUN / 4 + 400 {
            let before = age(&s);
            s.step();
            if age(&s) < before {
                grown += 1;
            }
            let now = matches!(s.world.get(bp([0, 2, 0])), Block::SugarCane { .. });
            tall_for = if now { tall_for + 1 } else { 0 };
            tall_max = tall_max.max(tall_for);
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        let held = |kind: &str| -> u32 { redsim::redstone::shulker::chest_stacks(&s.world, bp([1, 0, 0])).iter().filter(|st| redsim::redstone::container::item_name(st.item) == kind).map(|st| st.count as u32).sum() };
        // (the judge empties its chest every FARM_EMPTY gt, counting what it takes: the label's tally)
        let taken = |kind: &str| -> u32 { s.received.get(&[1, 0, 0]).map(|v| v.iter().map(|e| match e { crate::labels::Entry::Items { id, count, .. } if *id == kind => *count, _ => 0 }).sum()).unwrap_or(0) };
        let loose = s.world.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Item(st) if st.item.ends_with("sugar_cane"))).count() as u32;
        assert!(grown >= 20, "about one growth every 250 ticks at speed 300: {grown} in 6400");
        assert!(tall_max <= 4, "a grown block is harvested at once: stood {tall_max} ticks");
        assert!(taken("sugar_cane") > 0, "the judge emptied its chest");
        let got = held("sugar_cane") + taken("sugar_cane");
        assert!(got + loose == grown && got + 1 >= grown, "{grown} grown, {got} in the chest or taken, {loose} on their way");
        assert_eq!(held("cobblestone") + taken("cobblestone"), 1, "the judge's intruder on the hopper's open top reaches the chest (the one above the cane rests on its stone cap)");
    }

    /// An elevator carries its rider (the coordinator 2026-10-21: "rider + button presses play"):
    /// the judge's armor stand stands on the platform at `port rider` (as `circuit` adds it), and a
    /// press of the top floor's button calls the platform up with the rider on it; `web check`
    /// compares the rider's feet with circuit sim's every frame of the judge's run.
    #[test]
    fn an_elevator_carries_its_rider() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/fly/archive/fly-elevator-5.txt");
        let text = std::fs::read_to_string(path).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let rider = |s: &Session| s.world.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::ArmorStand(_))).map(|e| [e.pos.x, e.pos.y, e.pos.z]);
        let (r, _) = s.design.ports["rider"].clone();
        let start = rider(&s).expect("the judge's rider");
        assert_eq!(start, [r[0] as f64 + 0.5, r[1] as f64, r[2] as f64 + 0.5]);
        let top = s.circuit_inputs.iter().position(|i| i.name == "top").expect("the top floor's button");
        assert!(s.set_input(top, true));
        for _ in 0..30 {
            s.step();
        }
        s.set_input(top, false);
        for _ in 0..300 {
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        let end = rider(&s).expect("the rider stays");
        assert!((end[1] - start[1] - 5.0).abs() < 0.01, "carried up the 5 floors' distance: {start:?} -> {end:?}");
    }

    /// A melon farm (w86's melon-mc-7b, on clk-misc's redsim stems) replays its judge's run: the
    /// stem planted on farmland, its fruit growing beside it (the stem attached), the piston
    /// breaking the melon, its slices (3 to 7 from the loot table's own random) into the judge's
    /// chest; `web check` holds every tick to the judge's twin.
    #[test]
    fn a_melon_farm_grows_and_harvests_like_its_judge() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/farm/archive/melon-mc-7b.txt");
        let text = std::fs::read_to_string(path).unwrap();
        // (a small farm runs at the judge's full speed; a big one is shown as built unless the page
        // gives its judge's speed, `data-farm-rts`)
        assert!(Session::new(&text, &Options::default()).unwrap().simulated());
        let big = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/farm/archive/melon-lattice8.txt")).unwrap();
        let shown = Session::new(&big, &Options::default()).unwrap();
        assert!(!shown.simulated() && shown.unsimulated.as_deref().is_some_and(|u| u.contains(crate::circuit::NOT_REPLAYED)), "{:?}", shown.unsimulated);
        assert!(Session::new(&big, &Options { farm_rts: Some(8), ..Options::still() }).unwrap().unsimulated.is_none(), "with its speed it plays");
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let (crop, _) = s.design.ports["crop"].clone();
        let (out, _) = s.design.ports["output"].clone();
        assert!(matches!(s.world.get(bp(crop)), Block::Stem { melon: true, .. }), "the stem planted: {}", s.world.get(bp(crop)));
        assert!(matches!(s.world.get(bp(out)), Block::Chest { .. }), "the judge's chest: {}", s.world.get(bp(out)));
        let mut attached = 0;
        for _ in 0..8_000 {
            let before = matches!(s.world.get(bp(crop)), Block::AttachedStem { .. });
            s.step();
            if !before && matches!(s.world.get(bp(crop)), Block::AttachedStem { .. }) {
                attached += 1;
            }
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        // in the chest, or taken by the judge (it empties it every FARM_EMPTY gt: the label's tally)
        let held: u32 = redsim::redstone::shulker::chest_stacks(&s.world, bp(out)).iter().filter(|st| redsim::redstone::container::item_name(st.item) == "melon_slice").map(|st| st.count as u32).sum();
        let taken: u32 = s.received.get(&out).map(|v| v.iter().map(|e| match e { crate::labels::Entry::Items { id, count, .. } if *id == "melon_slice" => *count, _ => 0 }).sum()).unwrap_or(0);
        let slices = held + taken;
        assert!(attached >= 3, "fruits grow at the judge's speed: {attached} in 8000 ticks");
        assert!(slices >= 3 * (attached - 1), "every harvested melon's slices reach the chest: {slices} for {attached} fruits");
    }

    /// clk-obs's hopper-minecart unloader (rail study), as archived on explore/clk-obs 2026-10-22
    /// (cart-unloader-t2): its `port output` is the judge's chest under the unloading hopper.
    const CART_UNLOADER: &str = "name: cart-unloader-t2\ncircuit: cart-unloader\nport line.in: -4 2 0\nport output: 0 0 0\norigin: -4 0 0\npalette:\n  S minecraft:stone\n  B minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]\n  D minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]\n  L minecraft:rail[shape=east_west,waterlogged=false]\n  E minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]\n  H minecraft:hopper[enabled=true,facing=down]\n  C minecraft:comparator[facing=north,mode=compare,powered=false]\n  T minecraft:redstone_torch[lit=true]\n  R minecraft:repeater[delay=1,facing=south,locked=false,powered=false]\n  W minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]\nlayer 0:\n  ......\n  ..S.S.\n  ..SS..\nlayer 1:\n  .SSSH.\n  .SW.CS\n  ..WWSS\nlayer 2:\n  .BDLES\n  .T...R\n  ....TW\n";

    /// A minecart station shows its judge's chests (site 2026-10-21: the cart unloader's OUT cell
    /// drew a redstone lamp where the judge's chest goes): a chest at `port output`, no lamp.
    #[test]
    fn a_cart_station_shows_its_judge_chests() {
        let s = Session::new(CART_UNLOADER, &Options::default()).unwrap();
        assert!(matches!(s.world.get(bp([0, 0, 0])), Block::Chest { .. }), "the judge's chest under the hopper: {}", s.world.get(bp([0, 0, 0])));
        assert!(crate::circuit::consumer_cells(&s.design).is_empty(), "no lamp on a station's chest ports");
    }

    /// A card is a still (vjeux 2026-10-21, "Why is it simulating everything?"; the coordinator:
    /// cards never simulate): the auto-brewer that froze the Auto-brewer group's page (its still
    /// search ran 7,625 ticks of a 2-hopper clock in every card) is the design as placed, its
    /// judge's chests standing, at tick 0, nothing scheduled.
    #[test]
    fn a_card_is_a_still_and_never_ticks() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/brew/archive/brewer-small.txt")).unwrap();
        let t0 = std::time::Instant::now();
        let s = Session::new(&text, &Options::still()).unwrap();
        assert!(s.still && s.tick == 0 && s.world.ticks.count() == 0, "a still: tick {}, {} scheduled", s.tick, s.world.ticks.count());
        let (out, _) = s.design.ports["output"].clone();
        assert!(matches!(s.world.get(bp(out)), Block::Chest { .. }), "the judge's output chest: {}", s.world.get(bp(out)));
        assert!(t0.elapsed().as_millis() < 200, "built in {:?}", t0.elapsed());
        // the flying machines, TNT dupers and farms too: no look-ahead at all
        for f in ["circuits/farm/archive/cane4-mc-21b", "circuits/tnt/archive/tnt-clocked-22b"] {
            let text = std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let s = Session::new(&text, &Options::still()).unwrap();
            assert!(s.still && s.tick == 0, "{f}: tick {}", s.tick);
        }
    }

    /// The browser builds a page's session over frames (`Session::build` with a budget that runs
    /// out; the coordinator 2026-10-22: pressing play ran one 300-450 ms task): paused at every
    /// pause point, it is the very session `Session::new` builds, tick for tick (the judge's
    /// seeding kept): a door, a clock, a farm (its random ticks), a smelter, a brewer, a sorter, a
    /// shulker loader, a randomizer and a TNT duper.
    #[test]
    fn a_staged_build_is_the_same_session() {
        let designs = [
            "piston-door-3x3/archive/b38h-obs7-45b",
            "clocks/archive/clk-misc-dripleaf-stand",
            "circuits/farm/archive/melon-mc-7b",
            "circuits/farm/archive/cane4-mc-21b",
            "circuits/smelter/archive/super-smelter-8",
            "circuits/brew/archive/brewer-small",
            "circuits/sorter/archive/sorter-classic",
            "circuits/shulker/archive/shulker-auto-loader",
            "circuits/tnt/archive/tnt-clocked-22b",
        ];
        let mut texts: Vec<(String, String)> = designs.iter().map(|f| (f.to_string(), std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap())).collect();
        texts.push(("rand2-token".into(), RAND2_TOKEN.to_string()));
        for (f, text) in &texts {
            let mut a = Session::new(text, &Options::live()).unwrap();
            // a clock always past its deadline: a pause at every pause point
            let t = std::rc::Rc::new(std::cell::Cell::new(0.0));
            let clock = {
                let t = t.clone();
                std::rc::Rc::new(move || {
                    t.set(t.get() + 1.0);
                    t.get()
                })
            };
            let b = crate::staged::Budget::with_clock(clock);
            let mut task = crate::staged::Task::new(Session::build(text.clone(), Options::live(), b.clone()));
            let mut polls = 0;
            let mut s = loop {
                b.give(0.0);
                polls += 1;
                if let Some(r) = task.poll() {
                    break r.unwrap();
                }
            };
            assert!(polls > 1 || !a.simulated(), "{f}: built in one go");
            assert_eq!((a.tick, a.rest_tick, a.simulated()), (s.tick, s.rest_tick, s.simulated()), "{f}");
            for k in 0..300 {
                assert!(a.machine_blocks() == s.machine_blocks(), "{f}: the blocks differ at tick {} ({k} played)", a.tick);
                let pos = |x: &Session| x.world.entities.list.iter().filter(|e| !e.removed).map(|e| (e.pos.x.to_bits(), e.pos.y.to_bits(), e.pos.z.to_bits())).collect::<Vec<_>>();
                assert!(pos(&a) == pos(&s), "{f}: the entities differ at tick {}", a.tick);
                a.step();
                s.step();
            }
            // the short cycle too, staged
            let b = crate::staged::Budget::with_clock(std::rc::Rc::new(|| 1.0));
            let tx = text.clone();
            let mut cyc = crate::staged::Task::new(async move {
                let s = Session::new(&tx, &Options::live()).unwrap();
                s.short_cycle_staged(9, 120, &b).await
            });
            let staged = loop {
                if let Some(c) = cyc.poll() {
                    break c;
                }
            };
            assert_eq!(staged, Session::new(text, &Options::live()).unwrap().short_cycle(9, 120), "{f}: the short cycle");
        }
    }

    /// clk-item's first randomizer (study 16), as archived 2026-10-21: two droppers trading one
    /// item each way, the comparator of whichever holds the other's item lights its output.
    const RAND2_TOKEN: &str = "name: rand2-token\ncircuit: randomizer 2\nport input: 0 2 0\nport output.a: -4 0 0\nport output.b: 5 0 0\nnbt: 0 0 0 {Items:[{Slot:0b,id:\"minecraft:iron_shovel\",count:1}]}\nnbt: 1 0 0 {Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}\norigin: -4 0 0\npalette:\n  R minecraft:repeater[delay=1,facing=east]\n  r minecraft:repeater[delay=1,facing=west]\n  w minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]\n  x minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]\n  C minecraft:comparator[facing=east,mode=compare,powered=false]\n  c minecraft:comparator[facing=west,mode=compare,powered=false]\n  D minecraft:dropper[facing=east,triggered=false]\n  d minecraft:dropper[facing=west,triggered=false]\n  S minecraft:stone\nlayer -1:\n  SSSS..SSSS\nlayer 0:\n  RwwCDdcwwr\nlayer 1:\n  ....x\n";

    /// A randomizer plays (site 2026-10-21: rand2-token stopped at tick 44, "a dropper picks a
    /// random slot ... not modeled without the level random"): its droppers draw from the level
    /// random mirrored like the judge's base run, seeded from the box the judge builds the design
    /// in (the design as written: its input's driver behind it is not in that box). Every press
    /// lights exactly one output (hold style) and both outputs win; `web check` compares the
    /// frames with circuit sim's under the judge's stimulus.
    #[test]
    fn a_randomizer_draws_the_judges_random() {
        let mut s = Session::new(RAND2_TOKEN, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let (from, to) = crate::circuit::judge_box(&Design::parse(RAND2_TOKEN).unwrap()).unwrap();
        assert_eq!((from, to), ([-7, -4, -3], [8, 5, 3]), "the judge's watch box: the design and its ports, 3 around");
        let mut fresh = redsim::rng::LegacyRandom::new(redsim::redstone::shulker::build_seed(from, to, 0));
        for _ in 0..s.tick * 12 {
            fresh.next_int(48);
        }
        assert_eq!(s.world.level_random, Some(fresh), "seeded like the judge: 4 forced chunks draw 12 a tick");
        let mut wins = [0, 0];
        for k in 0..8 {
            assert!(s.set_input(0, true));
            for _ in 0..30 {
                s.step();
            }
            s.set_input(0, false);
            for _ in 0..30 {
                s.step();
            }
            assert!(s.halted.is_none(), "press {k}: {:?}", s.halted);
            let (a, b) = (s.output_powered([-4, 0, 0]), s.output_powered([5, 0, 0]));
            assert!(a != b, "press {k}: exactly one output on (a {a}, b {b})");
            wins[usize::from(b)] += 1;
        }
        assert!(wins[0] > 0 && wins[1] > 0, "both outputs win over 8 presses: {wins:?}");
    }
    /// Critique 41f1ec62a: on the AND gate's "A on, B on" the output came on 4 gt after the
    /// inputs (its latency: "reacts in 4 ticks"), and a 4-tick clock's lamp stays lit through the
    /// clock's off ticks (the lamp's own 4 gt off-delay, redsim's `lamp::tick`), while the clock's
    /// output itself pulses.
    /// The lock study's archived locks (name, text): replaced as the study finds smaller ones.
    fn archived_locks() -> Vec<(String, String)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/lock/archive");
        let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        v.sort();
        // the item-frame combination locks (`circuit: lock`; clk-item's sequence locks share the
        // folder: `circuit: sequence-lock`)
        v.iter().map(|p| (p.file_stem().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(p).unwrap())).filter(|(_, t)| is_frame_lock(t)).collect()
    }

    /// An item-frame combination lock's design (`circuit: lock N`), not a sequence lock.
    pub(crate) fn is_frame_lock(text: &str) -> bool {
        text.lines().any(|l| l.strip_prefix("circuit:").is_some_and(|k| k.split_whitespace().next() == Some("lock")))
    }

    /// Critique 41f1ec62a: the combination lock ran as built ("not simulated: not modeled by
    /// redsim"). Its item frames are redsim's now: tapping a dial turns its frame one step like
    /// the judge's right-click; the lamp lights once every dial shows its code frame's turn and
    /// goes off at the next turn. Every archived lock (1 to 4 dials).
    #[test]
    fn the_locks_open_at_their_code() {
        let locks = archived_locks();
        assert!(!locks.is_empty());
        for (name, text) in locks {
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{name}: {:?}", s.unsimulated);
            let n = s.circuit_inputs.len() / 2;
            let names: Vec<String> = s.circuit_inputs.iter().map(|c| c.name.clone()).collect();
            let want: Vec<String> = (0..n).map(|i| format!("d{i}")).chain((0..n).map(|i| format!("k{i}"))).collect();
            assert_eq!(names, want, "{name}");
            assert!(s.circuit_inputs.iter().all(|c| c.how == crate::circuit::InputHow::Frame), "{name}");
            let labels: Vec<String> = s.circuit_inputs.iter().map(|c| crate::circuit::input_label(&s.design, c)).collect();
            let want: Vec<String> = (1..=n).map(|i| format!("DIAL {i}")).chain((1..=n).map(|i| format!("CODE {i}"))).collect();
            assert_eq!(labels, want, "{name}");
            let code: Vec<u8> = (n..2 * n).map(|i| s.frame_turn(i).unwrap()).collect();
            assert!(code.iter().any(|k| *k > 0), "{name}: a code to turn to: {code:?}");
            let out = s.outputs[0];
            // its placement transient (lock3-q85: OUT powered for 4 ticks) settled before the
            // first frame shown; the badge counts from there, and a reset comes back to it
            assert!(s.tick < crate::circuit::SETTLE / 2 && s.rest_tick == s.tick && s.world.idle(), "{name}: settled at {}", s.tick);
            assert!(!s.output_powered(out), "{name}: no false OUT on a paused page");
            let settled = s.tick;
            s.run(7);
            s.reset();
            assert_eq!(s.tick, settled, "{name}");
            s.run(40);
            assert!(!s.output_powered(out), "{name}");
            // each dial turned to its code frame's turn, one tap at a time: open exactly when
            // every dial matches (read once the lock settled: it opens and closes within 10 gt)
            for (d, &k) in code.iter().enumerate() {
                for _ in 0..k {
                    // a tap on a pulse input: on now (the turn), off 2 gt later (nothing)
                    assert!(s.pulse_input(d, 2));
                    s.run(12);
                    let dials: Vec<u8> = (0..n).map(|i| s.frame_turn(i).unwrap()).collect();
                    assert_eq!(s.output_powered(out), dials == code, "{name}: dials {dials:?}, code {code:?}");
                }
            }
            assert!(s.output_powered(out), "{name}: open at the code");
            assert!(s.pulse_input(n - 1, 2));
            s.run(16);
            assert!(!s.output_powered(out), "{name}: shut one turn past it");
            assert!(s.halted.is_none() && s.world.errors.is_empty(), "{name}: {:?}", s.world.errors);
            // drawn: an item frame entity per frame, with its turn
            let frames: Vec<String> = s.entities(0.0).into_iter().map(|e| e.kind).filter(|k| k.starts_with("frame:")).collect();
            assert_eq!(frames.len(), 2 * n, "{name}: {frames:?}");
            s.drawn_matches_simulation().unwrap();
        }
    }

    /// webrender (2026-10-21): the item counters stopped at tick 44 in the viewer ("dropper picks a
    /// random slot among 2 stacks (not modeled)"): their droppers hold 2+ stacks of one item, and
    /// whichever slot goes the world is the same (the same item moves, the same fill level), so
    /// redsim takes the first without the level random (exact with it: trace-compare's
    /// counter100-items-judged scene). 30 pulses run clean.
    #[test]
    fn item_counters_run_past_their_first_two_stack_drop() {
        for name in ["counter100-items", "counter576-items"] {
            let text = std::fs::read_to_string(format!("{}/../../designs/circuits/memory/archive/{name}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{name}: {:?}", s.unsimulated);
            for _ in 0..30 {
                assert!(s.pulse_input(0, 4), "{name}");
                s.run(14);
            }
            assert!(s.halted.is_none() && s.world.errors.is_empty(), "{name}: {:?} {:?}", s.halted, s.world.errors);
        }
    }

    /// The site's build-time `data-still` (coordinator: long pages on a phone; a thumbnail's
    /// period search was most of its time): replaying it lands on the very still frame the
    /// viewer's own search finds — tick, output, picture box, blocks and entities — over a sample
    /// of the clock archive (clocks with no declared output, elongated ones), circuits and a door.
    #[test]
    fn data_still_replays_the_still_frame() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs");
        let list = |dir: &str| -> Vec<std::path::PathBuf> {
            let mut v: Vec<std::path::PathBuf> = std::fs::read_dir(format!("{root}/{dir}")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
            v.sort();
            v
        };
        let clocks = list("clocks/archive");
        let mut files: Vec<std::path::PathBuf> = clocks.iter().step_by(97).cloned().collect();
        // a clock with no declared output (the viewer picks one: `out=`), whatever the sample
        // holds (the usable-output rule of 2026-10-21 gave most of them a port)
        files.extend(clocks.iter().find(|p| std::fs::read_to_string(p).is_ok_and(|t| !t.lines().any(|l| l.starts_with("port output:")) && Session::new(&t, &Options::default()).is_ok_and(|mut s| s.still_attr(STILL_HORIZON).contains(" out=")))).cloned());
        files.extend(list("circuits/gates/archive").into_iter().take(3));
        files.extend(list("piston-door-3x3/archive").into_iter().take(2));
        let (mut outs, mut boxes) = (0, 0);
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            let Ok(mut a) = Session::new(&text, &Options::default()) else { continue };
            let attr = a.still_attr(STILL_HORIZON);
            outs += attr.contains(" out=") as usize;
            boxes += attr.contains(" box=") as usize;
            let mut b = Session::new(&text, &Options::default()).unwrap();
            assert_eq!(b.go_to_still_from(&attr), Ok(a.tick), "{}: {attr}", f.display());
            assert_eq!((b.tick, &b.outputs, b.picture), (a.tick, &a.outputs, a.picture), "{}: {attr}", f.display());
            assert!(b.render_blocks(0.0) == a.render_blocks(0.0), "{}: blocks differ at {attr}", f.display());
            assert_eq!(format!("{:?}", b.entities(0.0)), format!("{:?}", a.entities(0.0)), "{}", f.display());
        }
        // the sample covers both extras (a clock judged by its own output; an elongated clock's
        // window)
        assert!(outs > 0 && boxes > 0, "{outs} with out=, {boxes} with box= among {}", files.len());
        // an attribute it cannot read changes nothing
        let mut s = Session::new(&std::fs::read_to_string(&clocks[0]).unwrap(), &Options::default()).unwrap();
        let t = s.tick;
        assert!(s.go_to_still_from("12 out=1,2").is_err() && s.go_to_still_from("x").is_err() && s.go_to_still_from("5 tilt=3").is_err());
        assert_eq!(s.tick, t);
    }

    #[test]
    fn and_reacts_in_4_and_the_lamp_stays_lit() {
        let read = |f: &str| std::fs::read_to_string(format!("{}/../../designs/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let mut s = Session::new(&read("circuits/gates/archive/gate-and-5b-r7"), &Options::default()).unwrap();
        let names: Vec<String> = s.circuit_inputs.iter().map(|c| c.name.clone()).collect();
        let d = crate::circuit::parse_stimulus("a 80+ 100- 120+ 140- 160+ 180- 200+ 260-; b 40+ 60- 120+ 160- 180+ 220- 240+ 260-", &names, 40).unwrap();
        s.attach_demo(d);
        let o = s.outputs[0];
        assert_eq!(s.judge_lamp, None);
        let mut on = Vec::new();
        while s.tick < 160 {
            if s.tick >= 100 && s.output_powered(o) {
                on.push(s.tick);
            }
            s.step();
        }
        assert_eq!((on.first().copied(), on.last().copied(), on.len()), (Some(124), Some(143), 20), "on from 4 gt after both inputs to 4 gt after A went off");
        // clk-obs-p4-obs-4b as archived before the usable-output rule moved its output (a lamp
        // under the build that a 2-in-4 pulse keeps lit)
        let p4 = "name: clk-obs-p4-obs-4b\norigin: 0 0 0\nport output: 0 -1 2\npalette:\n  A minecraft:piston[extended=false,facing=south]\n  B minecraft:observer[facing=up,powered=false]\n  C minecraft:redstone_block\n  D minecraft:observer[facing=down,powered=false]\nlayer 0:\n  A\n  B\n  .\nlayer 1:\n  .\n  C\n  D\n";
        let mut s = Session::new(p4, &Options::default()).unwrap();
        let lamp = [0, -1, 2];
        assert_eq!(s.judge_lamp, Some(lamp));
        s.run(10);
        let (mut lit, mut pulses) = (0, 0);
        for _ in 0..40 {
            lit += matches!(s.world.get(bp(lamp)), Block::Lamp { lit: true }) as u32;
            pulses += s.output_powered(lamp) as u32;
            s.step();
        }
        assert_eq!((lit, pulses), (40, 20), "the lamp lit all along, the output on 2 of every 4 ticks");
    }

    const BULB_CLOCK: &str = "name: c\nport start: 0 0 0 last\norigin: 0 0 0\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";

    /// An item sorter runs live (critique: sorter-classic stood still at 0 ticks): its judge's
    /// chests on its ports, the source fed from the judge's stream (its kind mixed with others),
    /// the sinks emptied every 20 gt; items of its kind leave the source and the slice keeps its
    /// recipe.
    #[test]
    fn sorters_run_live() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/sorter/archive/sorter-classic.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let ch = s.shulker.clone().expect("the judge's chests");
        assert_eq!((ch.sources.len(), ch.outputs.len()), (1, 2), "items; pass and output");
        let src = ch.sources[0];
        let held = |s: &Session| -> u32 { redsim::redstone::shulker::chest_stacks(&s.world, src).iter().map(|x| x.count as u32).sum() };
        let start = held(&s);
        assert!(start > 300, "the source fed: {start}");
        let kinds: std::collections::BTreeSet<u8> = redsim::redstone::shulker::chest_stacks(&s.world, src).iter().map(|x| x.item).collect();
        assert!(kinds.len() >= 4, "a mixed stream: {kinds:?}");
        // the judge's stream leaves out what the design holds (its recipe: iron and paper)
        assert!(crate::circuit::design_kinds(&s.design).iter().any(|k| k == "paper"));
        s.run(400);
        assert!(held(&s) < start, "items left the source");
        // the judge's tally of what reached its sinks (the OUT chest's tag: "received")
        let out = ch.outputs.iter().map(|p| p.arr()).find(|p| *p == [0, 0, 0]).expect("the output chest");
        let got: u32 = s.received.get(&out).map(|v| v.iter().map(|e| if let crate::labels::Entry::Items { count, .. } = e { *count } else { 0 }).sum()).unwrap_or(0);
        assert!(got > 10, "iron reached OUT: {got}");
        // a reset replays the same run
        let a = s.world.state_key();
        s.reset();
        s.run(400);
        assert_eq!(s.world.state_key(), a);
    }

    /// A shulker loader and an unloader run live (redsim's driver of the judge's chests): their
    /// dispensers place box after box, the pistons break them, the drops are picked up, and a
    /// reset replays the same run.
    #[test]
    fn shulker_designs_run_live() {
        for (file, boxes) in [("shulker-ref-loader-1", 5), ("shulker-ref-unloader-1", 5), ("shulker-filter-loader", 1)] {
            // the reference loader left the archive (2026-10-21 rejudge): its copy lives beside its trace
            let dir = if file == "shulker-ref-loader-1" { "traces/exact" } else { "archive" };
            let text = std::fs::read_to_string(format!("{}/../../designs/circuits/shulker/{dir}/{file}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{file}: {:?}", s.unsimulated);
            // the visitor turns the start lever on (the judge does at its start)
            for i in 0..s.circuit_inputs.len() {
                s.set_input(i, true);
            }
            let cell = crate::circuit::box_cells(&s.design)[0];
            let ticks = if file.contains("filter") { 16000 } else { 1300 };
            let (mut placed, mut was) = (0, false);
            for _ in 0..ticks {
                s.step();
                let now = matches!(s.world.get(bp(cell)), Block::ShulkerBox { .. });
                placed += (now && !was) as u32;
                was = now;
            }
            assert!(s.world.errors.is_empty(), "{file}: {:?}", s.world.errors);
            assert!(placed >= boxes, "{file}: {placed} boxes in {ticks} gt");
            let d = s.frames_digest(400);
            s.reset();
            assert_eq!(d, s.frames_digest(400), "{file}: a reset replays the run");
        }
    }

    #[test]
    fn entity_clocks_run_their_entities() {
        // the boat ring's stripped core (the 2026-10-23 strip renamed clk-misc-boat-ring-3x3)
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-p412-oak_boat-oak_pressure_plate-stone-water-82b-e58845.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let start = s.entities(0.0);
        assert_eq!(start.len(), 1, "the boat");
        assert_eq!(start[0].kind, "minecraft:oak_boat");
        for _ in 0..60 {
            s.step();
        }
        let later = s.entities(0.0);
        let moved = (0..3).map(|k| (later[0].pos[k] - start[0].pos[k]).abs()).sum::<f32>();
        assert!(moved > 0.5, "the boat moves with the current: {:?} -> {:?}", start[0].pos, later[0].pos);
    }

    /// A falling block (redsim's gravity entities) is drawn: its block model where it is, from
    /// where it was before the tick (redsim's own position then, never a guess).
    #[test]
    fn falling_blocks_are_drawn() {
        let mut s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        let sand = redsim::Block::parse("minecraft:sand").unwrap();
        let top = (s.world.min.y + s.world.size[1] - 1) as f64;
        s.world.gravity.entities.push(redsim::gravity::FallingBlock::new(sand, 4.5, top, 0.5, 0.0, 3, 0));
        // just appeared (no tick run since): where it is
        let e = s.entities(0.5);
        let f = e.iter().find(|e| e.kind.starts_with("falling:")).expect("the falling block");
        assert!((f.pos[1] as f64 - top).abs() < 1e-4, "{:?}", f.pos);
        s.step();
        let now = s.world.gravity.entities.iter().find(|f| !f.removed).expect("still falling").y;
        assert!(now < top, "it fell: {now}");
        let e = s.entities(0.5);
        let f = e.iter().find(|e| e.kind.starts_with("falling:")).expect("the falling block");
        assert!(f.kind.contains("sand"), "{}", f.kind);
        assert!((f.pos[1] as f64 - (top + now) / 2.0).abs() < 1e-4, "half way from {top} to {now}: {:?}", f.pos);
        s.drawn_matches_simulation().unwrap();
        let atlas = crate::atlas::Atlas::build();
        let mut b = crate::mesh::Builder::new(&atlas);
        b.entity(f);
        assert!(!b.mesh.solid.is_empty(), "a falling block draws its block model");
        // a full block, half a block back from its position (FallingBlockRenderer)
        let lo = (0..3).map(|k| b.mesh.solid.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min)).collect::<Vec<_>>();
        assert!((lo[0] - 4.0).abs() < 1e-4 && (lo[1] - f.pos[1]).abs() < 1e-4 && lo[2].abs() < 1e-4, "{lo:?}");
    }

    /// vjeux: the view shows only what redsim simulates. Everything drawn is redsim's state on
    /// every tick: a TNT design's blasts (TNT entities, drops), an entity clock's boats, a
    /// shulker unloader's carts and boxes.
    #[test]
    fn drawn_is_redsims_state() {
        for (file, ticks) in [("circuits/tnt/archive/tnt-clocked-22b", 300), ("clocks/archive/clk-misc-p412-oak_boat-oak_pressure_plate-stone-water-82b-e58845", 200), ("circuits/shulker/archive/shulker-ref-unloader-1", 300)] {
            let text = std::fs::read_to_string(format!("{}/../../designs/{file}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{file}: {:?}", s.unsimulated);
            for i in 0..s.circuit_inputs.len() {
                s.set_input(i, true);
            }
            let mut entities = 0;
            for _ in 0..ticks {
                s.drawn_matches_simulation().unwrap_or_else(|e| panic!("{file} tick {}: {e}", s.tick));
                entities = entities.max(s.entities(1.0).len());
                s.step();
            }
            assert!(s.halted.is_none(), "{file}: {:?}", s.halted);
            assert!(entities > 0 || file.contains("shulker"), "{file}: no entity drawn");
        }
    }

    /// vjeux 04:41Z on the precise drop: "Please make the height of the gap at least the size of
    /// the drop before explosion". A duper that declares its stand (`stand: 23`) stands that high
    /// over its ground, as its judge judges it (`ground_drop`: the declared stand over the safe
    /// drop), and its TNT go off in mid-air under it, above the grass; one without (the clocked
    /// duper, the air burst) keeps its safe drop.
    #[test]
    fn a_precise_drop_stands_at_its_declared_height() {
        let read = |f: &str| std::fs::read_to_string(format!("{}/../../designs/circuits/tnt/archive/{f}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let text = read("tnt-precise-drop");
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        assert_eq!((safe_drop(&text), declared_stand(&s.design), ground_drop(&s.design, &text)), (4, 23, 23));
        let f = s.floor.clone().expect("its ground");
        let lowest = s.design.blocks.keys().map(|p| p[1]).min().unwrap();
        assert_eq!(f.hi[1], lowest - 1 - 23, "23 air blocks under the design");
        let surface = f.hi[1] as f64 + 1.0;
        let below = burst_below(&text).expect("its burst point");
        assert!(lowest as f64 - below as f64 > surface, "the burst point above the ground");
        let mut blasts = 0;
        for _ in 0..400 {
            s.step();
            for b in &s.world.explosions[blasts..] {
                assert!(b.center.y > surface + 0.5, "tick {}: a TNT went off at y {} on the ground (surface {surface})", s.tick, b.center.y);
            }
            blasts = s.world.explosions.len();
        }
        assert!(blasts > 3, "{blasts} explosions");
        assert!(s.floor.as_ref().unwrap().changed.is_empty(), "no crater in the ground");
        for f in ["tnt-clocked-22b", "tnt-airburst"] {
            let t = read(f);
            let s = Session::new(&t, &Options::default()).unwrap();
            let lowest = s.design.blocks.keys().map(|p| p[1]).min().unwrap();
            assert_eq!(s.floor.as_ref().map(|g| lowest - 1 - g.hi[1]), Some(safe_drop(&t)), "{f}: its safe drop");
        }
    }

    /// vjeux 17:53Z: "Please don't put a fake ground if the tnt goes through it". A TNT duper
    /// stands on its judge's ground, real blocks of its world: grass over two dirt over bedrock,
    /// its top the archived safe drop under the design (tnt-clocked-22b, the TNT study's one
    /// duper since 2026-10-20: 0 blocks, right under it), the judge's reach around it; its TNT
    /// land and explode on it (craters: ground cells that changed), and what is drawn of it is the
    /// world's every tick. A clock has no ground at all.
    #[test]
    fn tnt_dupers_stand_on_their_judges_ground() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/tnt/archive/tnt-clocked-22b.txt")).unwrap();
        let drop = safe_drop(&text);
        assert_eq!(drop, 0, "tnt-clocked-22b stands on the ground");
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let f = s.floor.clone().expect("a TNT duper's ground");
        let lowest = s.design.blocks.keys().map(|p| p[1]).min().unwrap();
        assert_eq!(f.hi[1], lowest - 1 - drop, "the top {drop} air blocks under the design");
        assert_eq!(f.hi[1] - f.lo[1], 3, "4 layers");
        assert!([GROUND_REACH, 2 * GROUND_REACH, MAX_GROUND_REACH].contains(&f.reach), "reach {}", f.reach);
        assert_eq!(s.world.get(BlockPos::new(f.lo[0], f.hi[1], f.lo[2])).to_string(), Block::parse("minecraft:grass_block").unwrap().to_string());
        assert_eq!(s.world.get(BlockPos::new(f.hi[0], f.lo[1], f.hi[2])), Block::parse("minecraft:bedrock").unwrap());
        let mut craters = 0;
        for _ in 0..300 {
            s.drawn_matches_simulation().unwrap_or_else(|e| panic!("tick {}: {e}", s.tick));
            craters = craters.max(s.floor.as_ref().unwrap().changed.len());
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        assert!(craters > 0, "no TNT went off on the ground");
        // the reset lays it back as it was
        s.reset();
        assert!(s.floor.as_ref().unwrap().changed.is_empty());
        let clock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-dripleaf-stand.txt")).unwrap();
        let c = Session::new(&clock, &Options::default()).unwrap();
        assert!(c.floor.is_none() && crate::scene::viewer_look(&c).ground.is_none());
    }

    /// vjeux: a clock's OUT box is red while its output is powered. The dripleaf stand's big
    /// dripleaf (its tilt looks the same either way) is powered from below 2 ticks of each 4 gt
    /// cycle; its blocks repeat in 4 (it starts in slow motion). (Its `port output: 0 0 0` names
    /// the air two blocks above the leaf: ports are world coordinates, not shifted by `origin`.)
    #[test]
    fn dripleaf_output_pulses() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-dripleaf-stand.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        s.find_auto_output(20_000);
        assert_eq!(s.short_cycle(9, 120), Some(4));
        let leaf = [0, -2, 0];
        assert!(s.design.blocks[&leaf].contains("big_dripleaf"));
        let mut seq = Vec::new();
        for _ in 0..40 {
            s.step();
            seq.push(s.output_powered(leaf));
        }
        assert_eq!(seq[20..40].iter().filter(|x| **x).count(), 10, "{seq:?}");
        assert_eq!(seq[20..24], seq[24..28], "{seq:?}");
        let o = s.outputs[0];
        let look = crate::scene::viewer_look(&s);
        assert!(look.marker.is_none() && look.port_outlines.iter().any(|(p, c)| *p == o && *c == crate::scene::OUT_COLOUR), "the OUT box, no glass");
    }

    #[test]
    fn auto_output_is_the_clock_judges() {
        let s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        let (w0, p) = s.picture_period(20_000).unwrap();
        assert_eq!((w0, p), (0, 12));
        let cells = s.period_cells(w0, p);
        assert_eq!(Session::auto_output(&cells), Some([2, 0, 0]));
    }

    #[test]
    fn the_kick_block_shows_at_tick_0() {
        // `port start: 0 0 0 last`: the observer placed last is there from the first frame
        let s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        assert_eq!(s.tick, 0);
        assert!(s.render_blocks(0.0).iter().any(|b| b.cell == [0, 0, 0] && b.state.contains("observer")));
    }

    #[test]
    fn recorded_output_states_light_the_marker() {
        assert!(active_state("minecraft:redstone_lamp[lit=true]"));
        assert!(!active_state("minecraft:redstone_wire[power=0,east=side]"));
        assert!(active_state("minecraft:redstone_wire[power=3]"));
        assert!(!active_state("minecraft:air"));
        assert!(active_state("minecraft:hopper[enabled=false,facing=down]"));
    }
}
