//! The contraption studies ([docs/circuits.md](../../../docs/circuits.md)): pulse circuits, T
//! flip-flops, delay lines, logic gates, memory, piston extenders and flying machines.
//!
//! A circuit is a design in the door/clock design text format (`crates/mctas/src/design.rs`) with:
//!
//! * `circuit: <kind> [args]`: what it claims to be ([`Kind`]); the kind's checker generates the
//!   stimulus ([`stimulus`]) and judges the recorded run ([`check`]);
//! * `port input[.<name>]: x y z [lever | button | <on-state> [<off-state>]]`: the cells the judge
//!   drives ([`InputHow`]);
//! * `port output[.<name>]: x y z`: the cells whose level is recorded every gt ([`trace::level`]);
//! * machines: `port payload[.<name>]: x y z` (tracked blocks), `travel: dx dy dz`, `distance: k`,
//!   and the clock's one-time `port start:` kick.
//!
//! [`run`] records a [`trace::Trace`] on redsim (the screen) or in the real game (the judge);
//! [`verify`] runs the kind's stimulus at 4 positions x 4 rotations and compares the builds.

pub use clock::anneal;
pub mod archive;
pub mod compact;
pub mod crafter;
pub mod material;
pub mod canefarm;
pub mod bud;
pub mod compute;
pub mod check;
pub mod elevator;
pub mod station;
pub mod rig;
pub mod cartio;
pub mod autodoor;
pub mod farm;
pub mod farmgen;
pub mod fly;
pub mod intrusion;
pub mod lock;
pub mod random;
pub mod pulsemult;
pub mod robust;
pub mod run;
pub mod seq;
pub mod shulker;
pub mod local;
pub mod seqlock;
pub mod sorter;
pub mod smelter;
pub mod brewer;
pub mod composter;
pub mod factory;
pub mod cartsys;
pub mod stimulus;
pub mod tnt;
pub mod trace;
pub mod verify;
pub mod vibration;
pub mod registry;
// the study modules (`src/studies/*.rs`: crate::registry), each `crate::<file stem>`, and their
// registrations `STUDY_DEFS` (build.rs)
include!(concat!(env!("OUT_DIR"), "/studies.rs"));

use anyhow::{Context, Result, anyhow, bail};
use clock::{Action, Clock};
use mctas::sim::Worker;
use std::collections::BTreeSet;
use std::path::Path;

pub use clock::Pos;

/// The default on-state of an input cell: a redstone block written with updates.
pub const INPUT_ON: &str = "minecraft:redstone_block";

/// The judge's stone in a driven input's cell, and under its repeater (a block no design uses as
/// a payload or a part: smooth stone).
pub const DRIVEN_STONE: &str = "minecraft:smooth_stone";

/// The judge's consumer, placed in every air output cell (vjeux/coordinator 2026-10-19: a circuit
/// must DRIVE something): a redstone lamp. The output's level is still the best signal into the
/// cell, and the lamp must follow it ([`check::update_less`]): an output that changes without a
/// block update (a dust redirected without a power change) leaves the lamp stale and fails.
pub const CONSUMER: &str = "minecraft:redstone_lamp[lit=false]";

/// How the judge drives an input cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputHow {
    /// The cell is written with updates: `on` while the input is on, `off` (default air) while off;
    /// the design holds `off` there. A plain `port input:` is this only with
    /// `MCTAS_INPUT_DEVICE=block` (the old placement, kept for comparisons): see [`InputHow::Driven`].
    Block { on: String, off: String },
    /// A plain `port input:` driven like a real build drives it (vjeux's critique R7, coordinator
    /// 2026-10-20): the judge's stone sits in the input cell for the whole run, powered by the
    /// judge's delay-1 repeater in the cell `dir` from it (outside the build's box, with a stone
    /// under it), switched by a redstone block placed `2 dir` from the input cell 2 gt before each
    /// input edge, so the stone's power changes exactly at the stimulus ticks. No block is ever
    /// placed or removed next to the design; the design must not move the stone.
    Driven { dir: Pos, through: bool },
    /// The design's lever in this cell (unpowered at rest), right-clicked at every input edge.
    Lever,
    /// The design's button in this cell, right-clicked at every rising input edge (the button's
    /// own pulse length; falling edges are the button's).
    Button,
    /// A value 0..15 as a signal strength (study 12's hex kinds): the design's chest in this cell
    /// (its state), whose contents the judge sets to the value's fill
    /// ([`compute::strength_items`]) between ticks; the design reads it with its own comparator.
    Strength { chest: String },
    /// An item frame the player turns (study 14's locks): the design's `minecraft:item_frame`
    /// entity in this cell, holding an item, at `rotation` (0..7) as built; one right-click per
    /// step of its value (the bridge's `use_entity`), read by the design's own comparator.
    Frame { rotation: u8 },
}

/// The horizontal directions a driven input's repeater may stand in, in the order they are tried.
pub const DRIVE_DIRS: [Pos; 4] = [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// `MCTAS_INPUT_DEVICE=block` (or [`set_input_device_block`]): plain inputs written with a
/// redstone block as before R7 (for the comparison run that lists the designs passing only by
/// the judge's block placement, `mctas circuit r7`).
pub fn input_device_block() -> bool {
    INPUT_BLOCK.load(std::sync::atomic::Ordering::Relaxed) || std::env::var("MCTAS_INPUT_DEVICE").is_ok_and(|v| v == "block")
}

static INPUT_BLOCK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Load circuits with the old block placement (`true`) or real input devices (`false`, the judge).
pub fn set_input_device_block(on: bool) {
    INPUT_BLOCK.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A driven input's judge cells: its repeater, the repeater's support and the switch.
pub fn driver_cells(pos: Pos, dir: Pos) -> (Pos, Pos, Pos) {
    let r = add(pos, dir);
    (r, add(r, [0, -1, 0]), add(r, dir))
}

/// An input cell.
#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub pos: Pos,
    pub how: InputHow,
}

/// An output or payload cell.
#[derive(Clone, Debug)]
pub struct Probe {
    pub name: String,
    pub pos: Pos,
}

/// The ground a TNT duper is judged on ([`Circuit::ground_blocks`]): a superflat floor (grass over
/// two dirt over bedrock: explosions leave craters, as in a real world), and an obsidian floor (no
/// craters: a TNT lands where it first touches down). A duper must survive on both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Grass,
    Obsidian,
}

impl Ground {
    pub const ALL: [Ground; 2] = [Ground::Grass, Ground::Obsidian];

    /// Its layers, top first.
    pub fn layers(self) -> &'static [&'static str] {
        match self {
            Ground::Grass => &["minecraft:grass_block", "minecraft:dirt", "minecraft:dirt", "minecraft:bedrock"],
            Ground::Obsidian => &["minecraft:obsidian"],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ground::Grass => "grass ground",
            Ground::Obsidian => "obsidian ground",
        }
    }
}

/// What a circuit claims to be: the axis of its study and the checker it is judged by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    RisingEdge,
    FallingEdge,
    DualEdge,
    PulseLimiter,
    PulseExtender,
    Tff,
    Delay,
    /// A steady-state gate; the claimed function (`gate xor`) is checked when given.
    Gate(Option<String>),
    RsLatch,
    DLatch,
    /// Output changes once every N input pulses.
    Counter(u32),
    /// A T flip-flop with a reset input: each `t` pulse toggles q, an `r` pulse clears it.
    TffReset,
    /// An edge-triggered D flip-flop (a 1-bit register): q takes `d` at a `clk` edge (rising or
    /// falling, reported), holds otherwise.
    Dff,
    /// [`Kind::Dff`] with an enable input: loads only when `e` is on at the `clk` edge.
    DffEnable,
    /// A shift register of N cells: q is `d` as sampled N `clk` edges ago.
    Shift(u32),
    /// Pushes its payload N cells along `travel` and brings it back.
    PistonExtender(u32),
    FlyingMachine,
    /// A vertical two-way flying machine between two floors `N` blocks apart with a call button at
    /// each floor (`port input.bottom`, `port input.top`) and a platform carrying a rider (`port
    /// rider`): [`elevator`] (track fly, coordinator 2026-10-21).
    Elevator(u32),
    /// The multi-floor elevator: `N` floors `S` blocks apart (`circuit: elevator-floors N S`), a
    /// destination button per floor at every floor (`port input.<floor>-<dest>`); the car rests at
    /// the floor of the last press ([`elevator`], track fly, coordinator 2026-10-21).
    ElevatorFloors(u32, u32),
    /// A MINECART STATION on a rail line (study 19, [`station`]; clk-obs, coordinator 2026-10-21):
    /// a cart arriving through `port line.in` stops at the platform and a press of the design's
    /// button (`port input.go`) sends it on, out through `port line.out`; 1: a one-way line, 2: a
    /// two-way line (carts coming the other way pass through).
    Station(u32),
    /// A HOPPER-MINECART UNLOADER on a rail line (study 19, [`cartio`]): a hopper minecart arriving
    /// with a load stops, empties every item into the judge's chest (`port output:`) and leaves on
    /// its own once empty (out through `port line.out`, or back through `port line.in` at a terminus).
    CartUnloader,
    /// A HOPPER-MINECART LOADER (study 19, [`cartio`]): an empty hopper minecart arriving stops, fills
    /// from the judge's supply chest (`port items:`) and leaves on its own once full or once the
    /// supply runs dry.
    CartLoader,
    /// An AUTOMATIC 2x2 DOOR (study 21, [`autodoor`]): it opens for someone walking up from either
    /// side and closes once they have gone through, never on someone in the doorway.
    AutoDoor,
    /// Fills empty shulker boxes with items and delivers them full (study 8, [`shulker`]); the
    /// number is the scenario item's max stack (64: cobblestone, 16: snowballs, 1: iron shovels).
    ShulkerLoader(u32),
    /// Empties full shulker boxes: the items to one place, the empty boxes to another ([`shulker`]).
    ShulkerUnloader(u32),
    /// A BUD switch (study 10, [`bud`]): an output pulse for the block updates the judge performs
    /// at its `port target:` cells (place, remove, state, shape, update).
    Bud,
    /// A self-running TNT duper (study 9): spawns primed TNT every period while every TNT block
    /// of the design survives (`tnt.rs`).
    TntDuper,
    /// A TNT duper fired by its input: each pulse spawns the same number of primed TNT, nothing
    /// without one, every TNT block surviving (`tnt.rs`).
    TntDuperTriggered,
    /// Study 11 ([`pulsemult`]): one input pulse gives exactly N output pulses (a pulse multiplier).
    Burst(u32),
    /// Study 11: a running clock divided by N (N >= 3; 2 is a T flip-flop).
    Divider(u32),
    /// Study 11: one output pulse of L per rising edge, its behaviour under a second edge measured
    /// (`retriggerable` / `non-retriggerable`: the claim, when given).
    Monostable(Option<String>),
    /// Study 11: the output follows the input and stays on L more.
    OffDelay,
    /// Study 11: no output for inputs of T or less, input - T for longer ones (a pulse filter).
    OnDelay,
    /// Study 11: pulses every P gt while the input is on (a gated clock).
    PulseTrain,
    /// Study 12 ([`compute`]): an adder, subtractor, incrementer, comparator, multiplexer,
    /// demultiplexer, decoder, encoder (exhaustive truth table) or binary counter, N bits.
    Compute(compute::Spec),
    /// Study 13 ([`sorter`]): an item sorter slice: every item of its kind (the full id) from a
    /// line into its chest, every other item on along the line.
    Sorter(String),
    /// Study 14 ([`lock`]): a combination lock of N item-frame dials and N hidden code frames.
    Lock(u32),
    /// The lock study ([`seqlock`]): a button sequence lock of N buttons and a code of L presses
    /// (hidden code frames), strict reset.
    SeqLock(u32, u32),
    /// Study 16 ([`random`]): one press lights exactly one of N outputs, uniformly at random (the
    /// game's level random: a dropper's random slot), judged over many seeded presses.
    Randomizer(u32),
    /// Study 17 ([`smelter`]): an auto-smelter of N furnaces: items from an input chest and fuel
    /// from a fuel chest spread across the furnaces, every result into the output chest.
    Smelter(u32),
    /// Study 18 ([`farm`]): an automatic crop harvester for one crop (its full id, e.g.
    /// `minecraft:sugar_cane`): the judge plants the crop on every `port crop:`, lets it grow by
    /// real random ticks in the game, and every harvested item must reach the judge's chests
    /// (`port output:`).
    Harvester(String),
    /// Study 21 ([`brewer`]): an auto-brewer of N brewing stands: water bottles, blaze powder and
    /// the ingredients of each stage from their chests, every finished potion into the output
    /// chest.
    Brewer(u32),
    /// Study 22 ([`composter`]): an auto-composter of N composters: a farm's mixed output from an
    /// input chest along a hopper line, every compostable item composted, the bone meal into its
    /// chests, everything else on to an overflow chest.
    Composter(u32),
    /// Study 23 ([`factory`]): a self-fueling dried kelp block factory of N kelp plants (`port
    /// crop:`), started from one block in the chest on `port fuel:`, its surplus into `port
    /// output:`.
    KelpFactory(u32),
    /// The auto-crafter study ([`crafter`]): its recipe's ingredients from the judge's supply
    /// chests into a crafter, every complete set crafted into the result (the full id) and sent
    /// to the output chest.
    Crafter(String),
    /// A kind of a registered study (crate::registry: `src/studies/<module>.rs`), e.g. the item
    /// elevator (crate::lift, `item-elevator N`).
    Study(registry::StudyKind),
}

/// The gate functions by truth table (inputs a, b in name order; bit index 2a + b), 2 inputs.
pub const GATES2: [(&str, [bool; 4]); 16] = [
    ("false", [false, false, false, false]),
    ("and", [false, false, false, true]),
    ("nimply", [false, false, true, false]),
    ("a", [false, false, true, true]),
    ("nimply-ba", [false, true, false, false]),
    ("b", [false, true, false, true]),
    ("xor", [false, true, true, false]),
    ("or", [false, true, true, true]),
    ("nor", [true, false, false, false]),
    ("xnor", [true, false, false, true]),
    ("not-b", [true, false, true, false]),
    ("imply-ba", [true, false, true, true]),
    ("not-a", [true, true, false, false]),
    ("imply", [true, true, false, true]),
    ("nand", [true, true, true, false]),
    ("true", [true, true, true, true]),
];

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        let w: Vec<&str> = s.split_whitespace().collect();
        let num = |i: usize| -> Result<u32> {
            let v = w.get(i).ok_or_else(|| anyhow!("circuit: {s:?}: missing its number"))?;
            v.parse().with_context(|| format!("circuit: {s:?}: bad number {v:?}"))
        };
        // a registered study's kind (crate::registry) reads its own words
        if let Some(k) = registry::parse(&w) {
            return Ok(Kind::Study(k.with_context(|| format!("circuit: {s:?}"))?));
        }
        Ok(match w.first().copied().unwrap_or("") {
            "rising-edge" => Kind::RisingEdge,
            "falling-edge" => Kind::FallingEdge,
            "dual-edge" => Kind::DualEdge,
            "pulse-limiter" => Kind::PulseLimiter,
            "pulse-extender" => Kind::PulseExtender,
            "tff" => Kind::Tff,
            "delay" => Kind::Delay,
            "gate" => {
                let f = w.get(1).map(|f| f.to_ascii_lowercase());
                if let Some(f) = &f
                    && f != "not"
                    && f != "buffer"
                    && !GATES2.iter().any(|(n, _)| n == f)
                {
                    bail!("circuit: gate {f:?}: not a gate function (not, buffer, and, or, xor, xnor, nand, nor, imply, nimply, imply-ba, nimply-ba)");
                }
                Kind::Gate(f)
            }
            "rs-latch" => Kind::RsLatch,
            "d-latch" => Kind::DLatch,
            "counter" => Kind::Counter(num(1)?),
            "tff-reset" => Kind::TffReset,
            "dff" => Kind::Dff,
            "dff-enable" => Kind::DffEnable,
            "shift" => {
                let n = num(1)?;
                if n == 0 {
                    bail!("circuit: shift 0: a shift register has at least 1 cell");
                }
                Kind::Shift(n)
            }
            "piston-extender" => Kind::PistonExtender(num(1)?),
            "flying-machine" => Kind::FlyingMachine,
            "elevator" => Kind::Elevator(num(1)?),
            "elevator-floors" => {
                let (n, s) = (num(1)?, num(2)?);
                if n < 2 || s < 1 {
                    bail!("circuit: elevator-floors N S: at least 2 floors, at least 1 block apart");
                }
                Kind::ElevatorFloors(n, s)
            }
            "station" => {
                let n = if w.len() > 1 { num(1)? } else { 1 };
                if !matches!(n, 1 | 2) {
                    bail!("circuit: {s:?}: station 1 (a one-way line) or station 2 (a two-way line)");
                }
                Kind::Station(n)
            }
            "cart-unloader" => Kind::CartUnloader,
            "cart-loader" => Kind::CartLoader,
            "auto-door" => Kind::AutoDoor,
            "shulker-loader" | "shulker-unloader" => {
                let stack = if w.len() > 1 { num(1)? } else { 64 };
                if !matches!(stack, 64 | 16 | 1) {
                    bail!("circuit: {s:?}: the item's max stack is 64 (cobblestone), 16 (snowballs) or 1 (iron shovels)");
                }
                if w[0] == "shulker-loader" { Kind::ShulkerLoader(stack) } else { Kind::ShulkerUnloader(stack) }
            }
            "bud" => Kind::Bud,
            "tnt-duper" => Kind::TntDuper,
            "tnt-duper-triggered" => Kind::TntDuperTriggered,
            "burst" => {
                let n = num(1)?;
                if n < 2 {
                    bail!("circuit: burst {n}: a burst gives at least 2 pulses (1 is a rising-edge detector)");
                }
                Kind::Burst(n)
            }
            "divider" => {
                let n = num(1)?;
                if n < 3 {
                    bail!("circuit: divider {n}: N >= 3 (a /2 divider is a T flip-flop: circuit: tff)");
                }
                Kind::Divider(n)
            }
            "monostable" => match w.get(1).copied() {
                None => Kind::Monostable(None),
                Some(c @ ("retriggerable" | "non-retriggerable")) => Kind::Monostable(Some(c.to_string())),
                Some(c) => bail!("circuit: monostable {c:?}: the claim is retriggerable or non-retriggerable"),
            },
            "off-delay" => Kind::OffDelay,
            "on-delay" => Kind::OnDelay,
            "pulse-train" => Kind::PulseTrain,
            "sorter" => match w.get(1) {
                Some(item) => Kind::Sorter(sorter::full_id(item)),
                None => bail!("circuit: sorter: name the item kind it sorts (`circuit: sorter iron_ingot`)"),
            },
            "lock" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) => Kind::Lock(n),
                None => bail!("circuit: lock: give the number of dials (`circuit: lock 4`)"),
            },
            "sequence-lock" => match (w.get(1).and_then(|s| s.parse::<u32>().ok()), w.get(2).and_then(|s| s.parse::<u32>().ok())) {
                (Some(n), Some(l)) => Kind::SeqLock(n, l),
                _ => bail!("circuit: sequence-lock: give the number of buttons and the code length (`circuit: sequence-lock 3 4`)"),
            },
            "randomizer" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) if n >= 2 => Kind::Randomizer(n),
                _ => bail!("circuit: randomizer: give the number of outputs, at least 2 (`circuit: randomizer 4`)"),
            },
            "smelter" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) if n >= 1 => Kind::Smelter(n),
                _ => bail!("circuit: smelter: give the number of furnaces (`circuit: smelter 4`; a tileable slice: `smelter 1` with `tile:`)"),
            },
            "harvester" => match w.get(1) {
                Some(crop) => Kind::Harvester(farm::crop_id(crop)?),
                None => bail!("circuit: harvester: name the crop (`circuit: harvester sugar_cane`)"),
            },
            "brewer" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) if n >= 1 => Kind::Brewer(n),
                _ => bail!("circuit: brewer: give the number of brewing stands (`circuit: brewer 1`; a tileable slice: with `tile:`)"),
            },
            "kelp-factory" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) if n >= 1 => Kind::KelpFactory(n),
                _ => bail!("circuit: kelp-factory: give the number of kelp plants (`circuit: kelp-factory 4`)"),
            },
            "composter" => match w.get(1).and_then(|s| s.parse::<u32>().ok()) {
                Some(n) if n >= 1 => Kind::Composter(n),
                _ => bail!("circuit: composter: give the number of composters (`circuit: composter 4`; a tileable slice: with `tile:`)"),
            },
            "crafter" => match w.get(1) {
                Some(r) if crafter::recipe(r).is_some() => Kind::Crafter(crafter::full_id(r)),
                Some(r) => bail!("circuit: crafter {r}: not a recipe of the study ({})", crafter::results()),
                None => bail!("circuit: crafter: name the result it crafts (`circuit: crafter iron_block`; {})", crafter::results()),
            },
            k if compute::Spec::parse(&w).is_some() => {
                let _ = k;
                Kind::Compute(compute::Spec::parse(&w).unwrap()?)
            }
            k => bail!(
                "circuit: unknown kind {k:?} (rising-edge, falling-edge, dual-edge, pulse-limiter, pulse-extender, tff, delay, gate [fn], rs-latch, d-latch, counter N, tff-reset, dff, dff-enable, shift N, piston-extender N, flying-machine, shulker-loader [64|16|1], shulker-unloader [64|16|1], tnt-duper, tnt-duper-triggered, bud, burst N, divider N, monostable [retriggerable|non-retriggerable], off-delay, on-delay, pulse-train, adder N, subtractor N, incrementer N, comparator N, mux N, demux N, decoder N, encoder N [priority], binary-counter N, lock N, randomizer N, crafter RESULT, {})",
                registry::all().map(|(_, d)| d.word).collect::<Vec<_>>().join(", ")
            ),
        })
    }

    /// The kind as written on the `circuit:` line.
    pub fn name(&self) -> String {
        match self {
            Kind::RisingEdge => "rising-edge".into(),
            Kind::FallingEdge => "falling-edge".into(),
            Kind::DualEdge => "dual-edge".into(),
            Kind::PulseLimiter => "pulse-limiter".into(),
            Kind::PulseExtender => "pulse-extender".into(),
            Kind::Tff => "tff".into(),
            Kind::Delay => "delay".into(),
            Kind::Gate(None) => "gate".into(),
            Kind::Gate(Some(f)) => format!("gate {f}"),
            Kind::RsLatch => "rs-latch".into(),
            Kind::DLatch => "d-latch".into(),
            Kind::Counter(n) => format!("counter {n}"),
            Kind::TffReset => "tff-reset".into(),
            Kind::Dff => "dff".into(),
            Kind::DffEnable => "dff-enable".into(),
            Kind::Shift(n) => format!("shift {n}"),
            Kind::PistonExtender(n) => format!("piston-extender {n}"),
            Kind::FlyingMachine => "flying-machine".into(),
            Kind::Elevator(n) => format!("elevator {n}"),
            Kind::ElevatorFloors(n, s) => format!("elevator-floors {n} {s}"),
            Kind::Station(n) => format!("station {n}"),
            Kind::CartUnloader => "cart-unloader".into(),
            Kind::CartLoader => "cart-loader".into(),
            Kind::AutoDoor => "auto-door".into(),
            Kind::ShulkerLoader(64) => "shulker-loader".into(),
            Kind::ShulkerLoader(s) => format!("shulker-loader {s}"),
            Kind::ShulkerUnloader(64) => "shulker-unloader".into(),
            Kind::ShulkerUnloader(s) => format!("shulker-unloader {s}"),
            Kind::Bud => "bud".into(),
            Kind::TntDuper => "tnt-duper".into(),
            Kind::TntDuperTriggered => "tnt-duper-triggered".into(),
            Kind::Burst(n) => format!("burst {n}"),
            Kind::Divider(n) => format!("divider {n}"),
            Kind::Monostable(None) => "monostable".into(),
            Kind::Monostable(Some(c)) => format!("monostable {c}"),
            Kind::OffDelay => "off-delay".into(),
            Kind::OnDelay => "on-delay".into(),
            Kind::PulseTrain => "pulse-train".into(),
            Kind::Compute(spec) => spec.kind_name(),
            Kind::Sorter(item) => format!("sorter {}", item.strip_prefix("minecraft:").unwrap_or(item)),
            Kind::Lock(n) => format!("lock {n}"),
            Kind::SeqLock(n, l) => format!("sequence-lock {n} {l}"),
            Kind::Randomizer(n) => format!("randomizer {n}"),
            Kind::Smelter(n) => format!("smelter {n}"),
            Kind::Harvester(crop) => format!("harvester {}", crop.strip_prefix("minecraft:").unwrap_or(crop)),
            Kind::Brewer(n) => format!("brewer {n}"),
            Kind::Composter(n) => format!("composter {n}"),
            Kind::KelpFactory(n) => format!("kelp-factory {n}"),
            Kind::Crafter(r) => format!("crafter {}", r.strip_prefix("minecraft:").unwrap_or(r)),
            Kind::Study(s) => s.name(),
        }
    }

    /// The study the kind belongs to (its catalog: `designs/circuits/<study>/`).
    pub fn study(&self) -> &'static str {
        match self {
            Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => "pulse",
            Kind::Tff => "tff",
            Kind::Delay => "delay",
            Kind::Gate(_) => "gates",
            Kind::RsLatch | Kind::DLatch | Kind::Counter(_) | Kind::TffReset | Kind::Dff | Kind::DffEnable | Kind::Shift(_) => "memory",
            Kind::PistonExtender(_) => "pext",
            Kind::FlyingMachine | Kind::Elevator(_) | Kind::ElevatorFloors(..) => "fly",
            Kind::Station(_) | Kind::CartUnloader | Kind::CartLoader => "rail",
            Kind::AutoDoor => "autodoor",
            Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_) => "shulker",
            Kind::Bud => "bud",
            Kind::TntDuper | Kind::TntDuperTriggered => "tnt",
            Kind::Burst(_) | Kind::Divider(_) | Kind::Monostable(_) | Kind::OffDelay | Kind::OnDelay | Kind::PulseTrain => "pulsemult",
            Kind::Compute(spec) if matches!(spec.op, compute::Op::SevenSegment | compute::Op::StorageCounter(_)) => "display",
            Kind::Compute(spec) if spec.op == compute::Op::Adder => "adder",
            Kind::Compute(..) => "compute",
            Kind::Sorter(_) => "sorter",
            Kind::Lock(_) | Kind::SeqLock(..) => "lock",
            Kind::Randomizer(_) => "random",
            Kind::Smelter(_) => "smelter",
            Kind::Harvester(_) => "farm",
            Kind::Brewer(_) => "brew",
            Kind::Composter(_) => "compost",
            Kind::KelpFactory(_) => "factory",
            Kind::Crafter(_) => "crafter",
            Kind::Study(s) => s.study().id,
        }
    }

    /// A shulker loader or unloader: judged by [`shulker`]'s own runner (judge chests, items and
    /// boxes counted), not by the waveform trace.
    pub fn is_shulker(&self) -> bool {
        matches!(self, Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_))
    }

    /// The input names the kind needs (`None`: any one input).
    fn inputs_needed(&self) -> Option<&'static [&'static str]> {
        match self {
            Kind::RsLatch => Some(&["r", "s"]),
            Kind::DLatch => Some(&["d", "e"]),
            Kind::TffReset => Some(&["r", "t"]),
            Kind::Dff | Kind::Shift(_) => Some(&["clk", "d"]),
            Kind::DffEnable => Some(&["clk", "d", "e"]),
            Kind::Elevator(_) => Some(&["bottom", "top"]),
            Kind::Station(_) => Some(&["go"]),
            // a registered kind's inputs (none: it runs by itself, checked below)
            Kind::Study(s) if !s.def().inputs.is_empty() => Some(s.def().inputs),
            _ => None,
        }
    }
}

/// The canonical input name: long aliases of the latch inputs (`set`, `reset`, `data`, `enable`,
/// `clock`, `toggle`) map to `s`, `r`, `d`, `e`, `clk`, `t`.
fn input_alias(name: &str) -> String {
    match name {
        "set" => "s".into(),
        "reset" => "r".into(),
        "data" => "d".into(),
        "enable" => "e".into(),
        "clock" => "clk".into(),
        "toggle" => "t".into(),
        n => n.to_string(),
    }
}

/// A circuit design.
#[derive(Clone, Debug)]
pub struct Circuit {
    /// The design with its placement and its optional one-time kick (machines), as a clock.
    pub clock: Clock,
    /// `None`: no `circuit:` line (an explicit `stimulus:` only, for exploring).
    pub kind: Option<Kind>,
    /// In name order (a bare `port input:` is named `in`).
    pub inputs: Vec<Input>,
    /// In name order (a bare `port output:` is named `out`).
    pub outputs: Vec<Probe>,
    pub payloads: Vec<Probe>,
    /// Return-station cells of a two-way flying machine (`port station[.<name>]: x y z`, each on a
    /// design block): real blocks that count, left out of the machine's tracked shape.
    pub stations: Vec<Pos>,
    /// The push or travel direction (a unit vector).
    pub travel: Option<Pos>,
    /// How far a flying machine must travel.
    pub distance: Option<u32>,
    /// The explicit `stimulus:` header.
    pub stimulus: Option<String>,
    /// The judge's chests of a shulker loader or unloader (`port items[.k]`, `port boxes[.k]`,
    /// `port reject`), in name order; its `port output[.k]` cells are judge chests too.
    pub stores: Vec<Store>,
    /// `tile: dx dy dz [copies]` (shulker and study-11 kinds): the design repeats every (dx, dy,
    /// dz) blocks.
    pub tile: Option<Tile>,
    /// `slice: dx dy dz` (study 12 bit slices, [`compute::sliced`]): the design is one bit of a
    /// ripple circuit; copy k + 1 sits one step from copy k, its chain input on copy k's chain
    /// output.
    pub slice: Option<Pos>,
    /// A BUD switch's target cells (`port target[.<name>]: x y z [state-a state-b]`), where the
    /// judge performs its block updates.
    pub targets: Vec<bud::Target>,
    /// The update classes a BUD switch claims to detect (`bud: place remove ...`).
    pub bud_claims: Vec<String>,
    /// R7: why the judge cannot drive one of its plain inputs like a real build (no way in for
    /// its repeater); the judge refuses the design with it, the tools that only read it still load
    /// it (the input stays a written block).
    pub input_refused: Option<String>,
    /// A TNT duper's drop: how many air blocks the judge leaves between its lowest block and the
    /// ground under it (set by the judge from the safe drop height, `robust::safe_drop`; 0 = it
    /// stands on the ground).
    pub drop: i32,
    /// The seed salt of a randomizer's runs ([`random`]): every build's level random is seeded
    /// from its watch box and this salt (0: the judge's builds; the redsim screen sweeps more).
    pub salt: i64,
    /// A flying machine run over the ground (clk-misc's real-use audit, 2026-10-21: a slime face
    /// next to grass drags it along): grass over dirt under its whole corridor, `drop` air blocks
    /// below its lowest block ([`crate::robust::fly_clearance`]). Off for every other run.
    pub grounded: bool,
}

/// A design's `tile:` header: the judge builds `copies` (default 3) side by side along `step`
/// ([`tiled`]) and judges each copy on its own: a shulker loader with its own judge chests (badge
/// `tileable`), a study-11 kind against its own input, the copies' stimuli shifted
/// ([`pulsemult::TILE_SHIFTS`]: 7 gt apart, then in phase; metric `tile`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub step: Pos,
    pub copies: u32,
}

/// A tiled design ([`Tile`]): `copies` copies along the step, copy k's ports renamed `<port>.t<k>`
/// (copy 0 keeps its names), as one circuit the runner judges copy by copy. Copies that overlap,
/// or a port landing on another copy, are refused.
pub fn tiled(c: &Circuit) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), t.copies.to_string());
    for k in 1..t.copies as i32 {
        let off = [t.step[0] * k, t.step[1] * k, t.step[2] * k];
        let sh = |p: &Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        for (p, s) in &base.blocks {
            let q = sh(p);
            if let Some(o) = d.blocks.get(&q) {
                bail!("tile: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
            }
            d.blocks.insert(q, s.clone());
        }
        for (p, s) in &base.block_entities {
            d.block_entities.insert(sh(p), s.clone());
        }
        for e in &base.entities {
            let mut e2 = e.clone();
            for i in 0..3 {
                e2.pos[i] += off[i] as f64;
            }
            d.entities.push(e2);
        }
        for (name, p) in &base.ports {
            let q = sh(p);
            // a port on the design's own block (a BUD's reset input) sits on its copy's block
            let own = base.blocks.contains_key(p);
            if (d.blocks.contains_key(&q) && !own) || d.ports.values().any(|x| *x == q) {
                bail!("tile: copy {k}'s port {name} at {q:?} lands on the other copies");
            }
            let renamed = format!("{name}.t{k}");
            if let Some(args) = base.meta.get(&format!("port {name}")) {
                d.meta.insert(format!("port {renamed}"), args.clone());
            }
            d.ports.insert(renamed, q);
        }
    }
    // copy 0's ports must not land on the other copies' blocks either
    for (name, p) in &base.ports {
        if !base.blocks.contains_key(p) && d.blocks.contains_key(p) {
            bail!("tile: port {name} at {p:?} is covered by another copy");
        }
    }
    Circuit::from_design(d)
}

/// A judge chest of a shulker loader or unloader ([`shulker`]): placed by the judge on an air cell
/// of the design (not counted), its contents filled or emptied between ticks.
#[derive(Clone, Debug)]
pub struct Store {
    /// `items`, `boxes` or `reject`.
    pub role: String,
    /// The port's full name (`items`, `items.a`).
    pub name: String,
    pub pos: Pos,
}

impl Circuit {
    pub fn load(path: &Path) -> Result<Circuit> {
        let d = mctas::design::Design::load(path)?;
        Circuit::from_design(d).with_context(|| format!("circuit design {}", path.display()))
    }

    pub fn from_design(design: mctas::design::Design) -> Result<Circuit> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut payloads = Vec::new();
        let mut stations = Vec::new();
        let mut stores = Vec::new();
        let mut targets = Vec::new();
        for (name, pos) in &design.ports {
            let args = design.meta.get(&format!("port {name}")).cloned().unwrap_or_default();
            let (base, sub) = match name.split_once('.') {
                Some((b, s)) => (b, Some(s)),
                None => (name.as_str(), None),
            };
            match base {
                "input" => {
                    let iname = input_alias(sub.unwrap_or("in"));
                    let how = parse_input(&design, *pos, &args).with_context(|| format!("port {name}"))?;
                    inputs.push(Input { name: iname, pos: *pos, how });
                }
                "output" => outputs.push(Probe { name: sub.unwrap_or("out").to_string(), pos: *pos }),
                "payload" => payloads.push(Probe { name: sub.unwrap_or("payload").to_string(), pos: *pos }),
                "station" => {
                    // an empty station cell is only a part a switch moves into place (checked
                    // below, once every port is read: the design needs a lever input)
                    stations.push(*pos);
                }
                "target" => targets.push(bud::Target::parse(&design, sub.unwrap_or("t"), *pos, &args).with_context(|| format!("port {name}"))?),
                "start" => {}
                // an elevator's rider cell (the judge's armor stand stands there: crate::elevator)
                "rider" => {}
                // a station's line ends (the judge's track starts there: crate::station)
                "line" => {}
                // an automatic door's opening corners (crate::autodoor)
                "door" => {}
                // a bedrock breaker's target, swap piston and player cell (crate::bedrock)
                "bedrock" | "swap" | "press" => {}
                "items" | "boxes" | "reject" | "pass" | "feed" | "fuel" | "crop" | "bottles" | "wart" | "sugar" => {
                    // a smelter's input and fuel chests may stand where the design has a cover (a
                    // full block on its first line hopper, there for the copies after it: the
                    // judge's chest replaces it, as a player puts the chest there instead)
                    let smelter = design.meta.get("circuit").is_some_and(|k| matches!(k.split_whitespace().next(), Some("smelter" | "brewer" | "composter")));
                    let cover = smelter && matches!(base, "items" | "fuel" | "bottles" | "wart" | "sugar") && design.blocks.get(pos).is_some_and(|s| smelter::is_cover(s));
                    // kelp is planted in water: its crop cell is the design's water source
                    let kelp = base == "crop" && design.meta.get("circuit").is_some_and(|k| k.split_whitespace().next() == Some("kelp-factory") || k.split_whitespace().nth(1).is_some_and(|c| c.ends_with("kelp")));
                    let water = kelp && design.blocks.get(pos).is_some_and(|s| farm::is_water_source(s));
                    if kelp && !water {
                        bail!("port {name}: kelp needs a water source at the crop cell (the player plants it in water), the design has {:?} at {pos:?}", design.blocks.get(pos).map(|s| s.as_str()).unwrap_or("air"));
                    }
                    if design.blocks.contains_key(pos) && !cover && !water {
                        bail!("port {name}: a judge chest goes on an air cell, the design has a block at {pos:?}");
                    }
                    stores.push(Store { role: base.to_string(), name: name.clone(), pos: *pos });
                }
                _ => bail!("port {name}: unknown port (input[.name], output[.name], payload[.name], station[.name], start, items[.name], boxes[.name], reject, pass, feed, fuel, crop[.name], target[.name])"),
            }
        }
        targets.sort_by(|a: &bud::Target, b: &bud::Target| a.name.cmp(&b.name));
        inputs.sort_by(|a, b| a.name.cmp(&b.name));
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        payloads.sort_by(|a, b| a.name.cmp(&b.name));
        stores.sort_by(|a: &Store, b: &Store| a.name.cmp(&b.name));
        for w in inputs.windows(2) {
            if w[0].name == w[1].name {
                bail!("two inputs named {:?}", w[0].name);
            }
        }
        let kind = design.meta.get("circuit").map(|s| Kind::parse(s)).transpose()?;
        let travel = match design.meta.get("travel") {
            Some(t) => {
                let v: Vec<i32> = t.split_whitespace().map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("travel: {t:?}"))?;
                if v.len() != 3 || v.iter().map(|x| x.abs()).sum::<i32>() != 1 {
                    bail!("travel: {t:?}: want a unit vector dx dy dz (one of them 1 or -1)");
                }
                Some([v[0], v[1], v[2]])
            }
            None => None,
        };
        let distance = design.meta.get("distance").map(|d| d.trim().parse::<u32>().with_context(|| format!("distance: {d:?}"))).transpose()?;
        let stimulus = design.meta.get("stimulus").cloned();
        let tile = match design.meta.get("tile") {
            Some(t) => {
                let v: Vec<i32> = t.split_whitespace().map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("tile: {t:?}"))?;
                if !(v.len() == 3 || v.len() == 4) || v[..3] == [0, 0, 0] || v.get(3).is_some_and(|n| *n < 2) {
                    bail!("tile: {t:?}: want dx dy dz [copies >= 2] (the step between copies)");
                }
                Some(Tile { step: [v[0], v[1], v[2]], copies: v.get(3).copied().unwrap_or(3) as u32 })
            }
            None => None,
        };
        let bud_claims = design.meta.get("bud").map(|s| s.split_whitespace().map(|w| w.to_string()).collect()).unwrap_or_default();
        let slice = match design.meta.get("slice") {
            Some(t) => {
                let v: Vec<i32> = t.split_whitespace().map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("slice: {t:?}"))?;
                if v.len() != 3 || v == [0, 0, 0] {
                    bail!("slice: {t:?}: want dx dy dz (the step from one bit to the next)");
                }
                Some([v[0], v[1], v[2]])
            }
            None => None,
        };
        if let Some(p) = stations.iter().find(|p| !design.blocks.contains_key(*p))
            && !inputs.iter().any(|i| i.how == InputHow::Lever)
            && !matches!(kind, Some(Kind::Elevator(_)) | Some(Kind::ElevatorFloors(..)))
        {
            bail!("port station: a return-station port marks a design block, none at {p:?} (an empty station cell is only for a part an on/off lever moves into place)");
        }
        // an elevator's rider: the judge's armor stand on the platform (crate::elevator); once
        // (a rotated or moved build is parsed again from a design that already carries it)
        let mut design = design;
        if let (Some(Kind::Elevator(_) | Kind::ElevatorFloors(..)), Some(r)) = (&kind, design.ports.get("rider").copied()) {
            let pos = [r[0] as f64 + 0.5, r[1] as f64, r[2] as f64 + 0.5];
            if !design.entities.iter().any(|e| e.kind.ends_with("armor_stand")) {
                design.entities.push(mctas::design::EntitySpec { kind: "minecraft:armor_stand".into(), pos, nbt: None });
            }
        }
        let clock = Clock::from_design(design)?;
        let mut c = Circuit { clock, kind, inputs, outputs, payloads, stations, travel, distance, stimulus, stores, tile, slice, targets, bud_claims, input_refused: None, drop: 0, salt: 0, grounded: false };
        c.check_ports()?;
        c.assign_drivers()?;
        Ok(c)
    }

    /// The box the design occupies during its run: its blocks' box, swept along its travel by a
    /// piston extender's reach or a flying machine's distance.
    pub fn swept_box(&self) -> Option<(Pos, Pos)> {
        let (mut lo, mut hi) = self.clock.design.bounds()?;
        let reach = match &self.kind {
            Some(Kind::PistonExtender(n)) => *n as i32 + 1,
            Some(Kind::FlyingMachine) => self.distance.unwrap_or(0) as i32 + 2,
            _ => 0,
        };
        if let (Some(t), true) = (self.travel, reach > 0) {
            for i in 0..3 {
                lo[i] = lo[i].min(lo[i] + t[i] * reach);
                hi[i] = hi[i].max(hi[i] + t[i] * reach);
            }
        }
        Some((lo, hi))
    }

    /// Turn each plain input into a driven one ([`InputHow::Driven`]): the first direction of
    /// [`DRIVE_DIRS`] whose repeater, support and switch cells are outside the box (a tiled run:
    /// free of every copy's blocks) and free of design blocks, ports and other drivers; else one
    /// through the build's air cells, its switch outside the box (static designs only: a moving
    /// one keeps all three outside its swept box). The repeater never touches a design block
    /// other than the input cell's.
    fn assign_drivers(&mut self) -> Result<()> {
        if input_device_block() {
            return Ok(());
        }
        let Some((lo, hi)) = self.swept_box() else { return Ok(()) };
        let tiled = self.clock.design.meta.contains_key("tiled");
        let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
        let mut taken: BTreeSet<Pos> = self.clock.design.ports.values().copied().collect();
        taken.extend(self.clock.design.blocks.keys().copied());
        let design_cells = taken.clone();
        let mut refused: Option<String> = None;
        for inp in &mut self.inputs {
            let InputHow::Block { on, .. } = &inp.how else { continue };
            // a redstone block written into the cell (over air, or over the design's own block
            // such as `redstone_block stone`): driven instead, the design's block (if any)
            // staying as the input's receiving block
            if on != INPUT_ON {
                continue;
            }
            let p = inp.pos;
            // first a side leading straight out of the box; else one through air cells of the
            // build (a real input line entering it), outside the swept box of a moving design
            let moving = matches!(self.kind, Some(Kind::PistonExtender(_)) | Some(Kind::FlyingMachine));
            let free = |q: Pos| !taken.contains(&q);
            // a diode in the input cell (the design's own receiving repeater or comparator) takes
            // its input only at its back (the cell in its facing direction): the judge's
            // repeater must stand there, pointing into it
            let here = self.clock.design.blocks.get(&p).map(|s| s.as_str()).unwrap_or("");
            let back = if here.starts_with("minecraft:repeater") || here.starts_with("minecraft:comparator") {
                match here.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
                    Some("north") => Some([0, 0, -1]),
                    Some("south") => Some([0, 0, 1]),
                    Some("west") => Some([-1, 0, 0]),
                    Some("east") => Some([1, 0, 0]),
                    _ => None,
                }
            } else {
                None
            };
            let fits = |d: &Pos| back.is_none_or(|b| b == *d);
            let straight = |d: &Pos| {
                let (r, sup, sw) = driver_cells(p, *d);
                [r, sup, sw].iter().all(|q| free(*q) && (tiled || outside(*q)))
            };
            // the judge's repeater must act on the input cell only. A repeater powers only the
            // block in front of it (DiodeBlock.getSignal answers only toward its facing) and its
            // state change sends neighbour updates only to that block and its neighbours
            // (DiodeBlock.updateNeighborsInFront), so the design blocks beside it that could still
            // see it are an observer (it watches the repeater's state change) and a diode pointing
            // into its side (it would lock the judge's repeater): no such block may touch it.
            // Dust beside it is inert (dust connects only to a repeater's front and back).
            let alone = |d: &Pos| {
                let (r, sup, sw) = driver_cells(p, *d);
                let quiet = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(r, *n)).filter(|q| *q != p && *q != sup && *q != sw).all(|q| {
                    self.clock.design.blocks.get(&q).is_none_or(|s| !(s.starts_with("minecraft:observer") || s.starts_with("minecraft:repeater") || s.starts_with("minecraft:comparator")))
                });
                // the switch (a redstone block the judge places and removes) powers and updates
                // all six of its neighbours: none but the repeater may be a design block or a port
                // (an output cell holds the judge's lamp; another input's cell its receiving
                // block). Another input's driver beside it is inert (a redstone block neither
                // powers a stone nor locks a repeater from the side).
                let isolated = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(sw, *n)).filter(|q| *q != r).all(|q| !design_cells.contains(&q));
                quiet && isolated && fits(d)
            };
            let dir = DRIVE_DIRS.iter().copied().find(|d| straight(d) && alone(d)).map(|d| (d, false)).or_else(|| {
                DRIVE_DIRS
                    .iter()
                    .copied()
                    .find(|d| {
                        let (r, sup, sw) = driver_cells(p, *d);
                        [r, sup, sw].iter().all(|q| free(*q) && (!moving || outside(*q))) && outside(sw) && alone(d)
                    })
                    .map(|d| (d, true))
            });
            match dir {
                Some((d, through)) => {
                    let (r, sup, sw) = driver_cells(p, d);
                    taken.extend([r, sup, sw]);
                    inp.how = InputHow::Driven { dir: d, through };
                }
                None => {
                    if refused.is_none() {
                        refused = Some(format!(
                            "input {} at {p:?}: no side of the input cell leads out of the build for the judge's input repeater (a free cell for the repeater, one under it and one beyond it outside the box; a moving design: all outside its swept box). R7: inputs are driven like a real build drives them; move the input to the edge of the build",
                            inp.name
                        ));
                    }
                }
            }
        }
        self.input_refused = refused;
        Ok(())
    }

    /// The circuit as the judge runs it: every driven input's stone, repeater and support placed
    /// (the switch comes and goes with the input edges, [`Circuit::actions`]).
    pub fn with_drivers(&self) -> Circuit {
        let mut c = self.clone();
        for inp in &self.inputs {
            if let InputHow::Driven { dir, .. } = inp.how {
                let (r, sup, _) = driver_cells(inp.pos, dir);
                let facing = match dir {
                    [0, 0, -1] => "north",
                    [0, 0, 1] => "south",
                    [-1, 0, 0] => "west",
                    _ => "east",
                };
                c.clock.design.blocks.entry(inp.pos).or_insert_with(|| DRIVEN_STONE.to_string());
                c.clock.design.blocks.insert(r, format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]"));
                c.clock.design.blocks.insert(sup, DRIVEN_STONE.to_string());
            }
        }
        c
    }

    /// The first frame where a driven input's cell lost its block (the judge's stone, or the
    /// design's own receiving block: the design moved it), with the cell.
    pub fn input_entered(&self, t: &trace::Trace) -> Option<String> {
        for inp in &self.inputs {
            if !matches!(inp.how, InputHow::Driven { .. }) {
                continue;
            }
            let id = |s: &str| s.split('[').next().unwrap_or(s).to_string();
            let want = id(self.clock.design.blocks.get(&inp.pos).map(|s| s.as_str()).unwrap_or(DRIVEN_STONE));
            for (f, frame) in t.cells.iter().enumerate() {
                let here = frame.binary_search_by_key(&inp.pos, |(q, _)| *q).ok().map(|i| t.names[frame[i].1 as usize].as_str());
                if !here.is_some_and(|s| id(s) == want) {
                    return Some(format!("the design moves the judge's input stone at {:?} (input {}, frame {f}: {}): an input cell is never entered", inp.pos, inp.name, here.map(clock::short).unwrap_or_else(|| "air".into())));
                }
            }
        }
        None
    }

    /// The output cells' blocks stay put for the whole run (the R7 counterpart of
    /// [`Circuit::input_entered`] for outputs): the block a design leaves in an output cell (a
    /// self-reporting part: dust, repeater, comparator, torch, observer) or the judge's lamp in an
    /// air one ([`CONSUMER`]) is never moved out or replaced by a block moved in. Otherwise the
    /// judge would read two different things in one cell: the part's own state while it is there,
    /// the signal into the emptied cell from a neighbour (a redstone block beside it) while it is
    /// away, and no builder's wire can follow an output that leaves its cell. Kinds whose machines
    /// move as a whole (flying machines, extenders, TNT) are not output-cell kinds and are skipped.
    pub fn output_entered(&self, t: &trace::Trace) -> Option<String> {
        let driven = self.inputs.iter().any(|i| matches!(i.how, InputHow::Driven { .. }));
        if !driven && self.kind != Some(Kind::Bud) {
            return None;
        }
        let id = |s: &str| s.split(['[', '{']).next().unwrap_or(s).to_string();
        let lamp = id(CONSUMER);
        for out in &self.outputs {
            let want = self.clock.design.blocks.get(&out.pos).map(|s| id(s)).unwrap_or_else(|| lamp.clone());
            for (f, frame) in t.cells.iter().enumerate() {
                let here = frame.binary_search_by_key(&out.pos, |(q, _)| *q).ok().map(|i| t.names[frame[i].1 as usize].as_str());
                let s = here.unwrap_or("minecraft:air");
                if id(s) != want {
                    return Some(format!("the design moves a block into or out of the output cell {:?} (output {}, frame {f}: {} instead of {}): an output's block stays put", out.pos, out.name, clock::short(s), clock::short(&want)));
                }
            }
        }
        None
    }

    /// The ports the kind needs.
    fn check_ports(&self) -> Result<()> {
        let Some(k) = &self.kind else {
            return Ok(());
        };
        if *k == Kind::Bud {
            return bud::check_ports(self);
        }
        if let Kind::Compute(spec) = k {
            return compute::check_ports(self, *spec);
        }
        if let Kind::SeqLock(n, l) = k {
            return seqlock::check_ports(self, *n, *l);
        }
        if let Kind::Lock(n) = k {
            return lock::check_ports(self, *n);
        }
        if let Kind::Randomizer(n) = k {
            return random::check_ports(self, *n);
        }
        if !self.targets.is_empty() {
            bail!("{}: target ports are for the bud kind", k.name());
        }
        let names: Vec<&str> = self.inputs.iter().map(|i| i.name.as_str()).collect();
        // a tiled run ([`tiled`]): copy k's ports are renamed `<name>.t<k>`, so the kind's port
        // check runs on copy 0's names (`<name>` and `<name>.t<k>`, `copies` of each)
        let copies = self.clock.design.meta.get("tiled").and_then(|n| n.trim().parse::<usize>().ok());
        let base: Vec<&str> = match copies {
            // copy k's port is `<name>.t<k>`, so the default-named input becomes `t<k>`
            Some(_) => names.iter().filter(|n| crate::shulker::group_of(n) == 0).copied().collect(),
            None => names.clone(),
        };
        match k.inputs_needed() {
            Some(need) => {
                let mut want: Vec<&str> = need.to_vec();
                want.sort();
                let ok = base == want && copies.is_none_or(|c| names.len() == want.len() * c);
                if !ok {
                    bail!("{}: needs inputs {} (port input.<name>), has {:?}", k.name(), want.join(", "), names);
                }
            }
            None => match k {
                Kind::FlyingMachine | Kind::TntDuper => {}
                Kind::Sorter(_) | Kind::Smelter(_) | Kind::Brewer(_) | Kind::Composter(_) | Kind::KelpFactory(_) => {
                    if !names.is_empty() {
                        bail!("{}: runs by itself (no input port), has {names:?}", k.name());
                    }
                }
                Kind::Harvester(_) => {
                    if !names.is_empty() {
                        bail!("harvester: runs by itself (no input port), has {names:?}");
                    }
                }
                Kind::Crafter(_) => {
                    if !names.is_empty() {
                        bail!("crafter: runs by itself (no input port), has {names:?}");
                    }
                }
                Kind::Study(s) => {
                    if !names.is_empty() {
                        bail!("{}: runs by itself (no input port), has {names:?}", s.word);
                    }
                }
                Kind::CartUnloader | Kind::CartLoader | Kind::AutoDoor => {
                    if !names.is_empty() {
                        bail!("{}: runs by itself (no input port), has {names:?}", k.name());
                    }
                }
                Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_) => {
                    // a tiled run ([`shulker::tiled`]) has one lever per copy; `input.mode` is a
                    // combo's mode lever (badge `combo`)
                    let most = self.clock.design.meta.get("tiled").and_then(|n| n.trim().parse::<usize>().ok()).unwrap_or(1);
                    let starts = names.iter().filter(|n| !(**n == "mode" || n.starts_with("mode."))).count();
                    let modes = names.len() - starts;
                    if starts > most || modes > most || self.inputs.iter().any(|i| i.how != InputHow::Lever) {
                        bail!("{}: at most one start lever (`port input: x y z lever`, none = always on) and one mode lever (`port input.mode: x y z lever`, a loader + unloader combo)", k.name());
                    }
                }
                Kind::Gate(_) if base.len() == 1 || base.len() == 2 => {
                    // a tiled run ([`tiled`]): `copies` copies of the gate's 1 or 2 inputs
                    if copies.is_some_and(|c| names.len() != base.len() * c) {
                        bail!("gate: a tiled run needs each input on every copy, has {names:?}");
                    }
                }
                Kind::Gate(_) => bail!("gate: needs 1 or 2 inputs (port input.a, port input.b), has {}", names.len()),
                // destination buttons: checked by crate::elevator::check_ports
                Kind::ElevatorFloors(..) => {}
                _ if names.len() == 1 => {}
                // a tiled run ([`tiled`]) of a tileable design: one input per copy
                _ if matches!(k.study(), "pulsemult" | "pulse" | "tff" | "memory" | "delay" | "gates") && self.clock.design.meta.get("tiled").and_then(|n| n.trim().parse::<usize>().ok()) == Some(names.len()) => {}
                _ => bail!("{}: needs exactly one input (port input:), has {}", k.name(), names.len()),
            },
        }
        match k {
            Kind::Elevator(_) | Kind::ElevatorFloors(..) => elevator::check_ports(self)?,
            Kind::Station(_) => station::check_ports(self)?,
            Kind::CartUnloader | Kind::CartLoader => cartio::check_ports(self)?,
            Kind::AutoDoor => autodoor::check_ports(self)?,
            Kind::PistonExtender(_) => {
                if self.payloads.is_empty() {
                    bail!("piston-extender: needs `port payload: x y z` (the pushed block)");
                }
                if self.travel.is_none() {
                    bail!("piston-extender: needs `travel: dx dy dz` (the push direction)");
                }
            }
            Kind::FlyingMachine => {
                if self.travel.is_none() {
                    bail!("flying-machine: needs `travel: dx dy dz`");
                }
            }
            Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_) => {
                let has = |role: &str| self.stores.iter().any(|s| s.role == role);
                if !has("items") || !has("boxes") || self.outputs.is_empty() {
                    bail!("{}: needs judge chests `port items:` (the items), `port boxes:` (the box supply) and `port output:` (where the boxes go)", k.name());
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
            }
            Kind::Sorter(_) => {
                let has = |role: &str| self.stores.iter().any(|s| s.role == role);
                if !has("items") || !has("pass") || self.outputs.is_empty() {
                    bail!("sorter: needs judge chests `port items:` (the line's source), `port pass:` (the end of the line) and `port output:` (the slice's chest)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
            }
            Kind::KelpFactory(n) => {
                let crops = self.stores.iter().filter(|s| s.role == "crop").count() as u32;
                if crops != *n || !self.stores.iter().any(|s| s.role == "fuel") || self.outputs.is_empty() {
                    bail!("kelp-factory {n}: needs {n} `port crop:` (the kelp cells, water sources), `port fuel:` (the chest the starting block goes in) and `port output:` (the surplus blocks); has {crops} crop port(s)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
            }
            Kind::Composter(n) => {
                let has = |role: &str| self.stores.iter().any(|s| s.role == role);
                if !has("items") || !has("pass") || self.outputs.is_empty() {
                    bail!("composter: needs judge chests `port items:` (the input chest), `port pass:` (the overflow chest) and `port output:` (the bone meal)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
                let have = self.clock.design.blocks.values().filter(|s| s.starts_with("minecraft:composter")).count() as u32;
                let tiled = self.clock.design.meta.get("tiled").and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(1);
                if have != n * tiled {
                    bail!("composter {n}: the design has {have} composter(s), not {}", n * tiled);
                }
            }
            Kind::Brewer(n) => {
                let has = |role: &str| self.stores.iter().any(|s| s.role == role);
                if !has("bottles") || !has("fuel") || !has("wart") || !has("sugar") || self.outputs.is_empty() {
                    bail!("brewer: needs judge chests `port bottles:` (water bottles), `port fuel:` (blaze powder), `port wart:` (nether wart), `port sugar:` (sugar) and `port output:` (the potions)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
                let stands = self.clock.design.blocks.values().filter(|s| s.starts_with("minecraft:brewing_stand")).count() as u32;
                let tiled = self.clock.design.meta.get("tiled").and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(1);
                if stands != n * tiled {
                    bail!("brewer {n}: the design has {stands} brewing stand(s), not {}", n * tiled);
                }
            }
            Kind::Smelter(n) => {
                let has = |role: &str| self.stores.iter().any(|s| s.role == role);
                if !has("items") || !has("fuel") || self.outputs.is_empty() {
                    bail!("smelter: needs judge chests `port items:` (the input chest), `port fuel:` (the fuel chest) and `port output:` (the output chest)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
                let furnaces = self.clock.design.blocks.values().filter(|s| s.starts_with("minecraft:furnace[") || s.as_str() == "minecraft:furnace" || s.starts_with("minecraft:blast_furnace")).count() as u32;
                let tiled = self.clock.design.meta.get("tiled").and_then(|t| t.trim().parse::<u32>().ok()).unwrap_or(1);
                if furnaces != n * tiled {
                    bail!("smelter {n}: the design has {furnaces} furnace(s), not {}", n * tiled);
                }
            }
            Kind::Crafter(r) => {
                let ports = crafter::supply_ports(self);
                if ports.is_empty() || self.outputs.len() != 1 {
                    bail!("crafter: needs judge chests `port items:` (the supply; or one `port items.<ingredient>:` per ingredient) and one `port output:` (the results)");
                }
                if let Some(rec) = crafter::recipe(r) {
                    crafter::Scenario::new(crafter::Run::Base, rec, &ports)?;
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
            }
            // read through the primed TNT they spawn (no output cell)
            Kind::TntDuper | Kind::TntDuperTriggered => {}
            Kind::Harvester(_) => {
                if !self.stores.iter().any(|s| s.role == "crop") || self.outputs.is_empty() {
                    bail!("harvester: needs `port crop: x y z` (where the judge plants the crop: an air cell on its soil) and `port output: x y z` (the judge's chest the harvest goes to)");
                }
                for o in &self.outputs {
                    if self.clock.design.blocks.contains_key(&o.pos) {
                        bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
                    }
                }
            }
            // a registered study's kind (crate::registry): its own port checks
            Kind::Study(s) => (s.def().check)(self, &s.args)?,
            _ => {
                if self.outputs.is_empty() {
                    bail!("{}: needs `port output: x y z`", k.name());
                }
            }
        }
        if self.tile.is_some() && !k.is_shulker() && !matches!(k.study(), "pulsemult" | "pulse" | "tff" | "memory" | "delay" | "gates" | "sorter" | "smelter" | "farm" | "brew" | "compost") {
            bail!("tile: only shulker loaders and unloaders, BUD switches, pulse circuits, the study-11 kinds, the sequential, delay and gate kinds, sorters, smelters, harvesters, brewers and composters are tiled");
        }
        if self.slice.is_some() {
            bail!("slice: only study 12 kinds (adders, subtractors, incrementers, binary counters) are bit slices");
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.clock.design.name
    }

    /// The output cells that get the judge's consumer ([`CONSUMER`]): air in the design and not
    /// an input cell, for every kind read through its outputs (piston extenders and flying
    /// machines are read through their payload and travel), and not where a piston may move a block
    /// in (the block there is what the output reads): a piston's head cell, or the cell after an
    /// unbroken row of blocks in front of a piston (up to 12); with slime or honey in the design,
    /// any cell within 13 in front of a piston.
    pub fn consumer_cells(&self) -> Vec<Pos> {
        if matches!(self.kind, Some(Kind::PistonExtender(_)) | Some(Kind::FlyingMachine) | Some(Kind::ShulkerLoader(_)) | Some(Kind::ShulkerUnloader(_))) {
            return Vec::new();
        }
        let ins: BTreeSet<Pos> = self.inputs.iter().map(|i| i.pos).collect();
        let blocks = &self.clock.design.blocks;
        let sticky_blocks = blocks.values().any(|s| s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block"));
        let mut lines: BTreeSet<Pos> = BTreeSet::new();
        for (p, s) in blocks {
            if !(s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[")) {
                continue;
            }
            let f = match s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
                Some("north") => [0, 0, -1],
                Some("south") => [0, 0, 1],
                Some("east") => [1, 0, 0],
                Some("west") => [-1, 0, 0],
                Some("up") => [0, 1, 0],
                Some("down") => [0, -1, 0],
                _ => continue,
            };
            let at = |k: i32| [p[0] + k * f[0], p[1] + k * f[1], p[2] + k * f[2]];
            if sticky_blocks {
                lines.extend((1..=13).map(at));
                continue;
            }
            // the head cell, then the cell after each block of the row the piston would push
            lines.insert(at(1));
            let mut k = 1;
            while k <= 12 && blocks.contains_key(&at(k)) {
                lines.insert(at(k + 1));
                k += 1;
            }
        }
        let mut v: Vec<Pos> = self.outputs.iter().map(|o| o.pos).filter(|p| !ins.contains(p) && !lines.contains(p) && !blocks.contains_key(p)).collect();
        v.sort();
        v.dedup();
        v
    }

    /// The circuit as the judge builds it: its consumer ([`CONSUMER`]) in every consumer cell
    /// (not one of the design's blocks: not counted, not part of its type).
    pub fn with_consumers(&self) -> Result<Circuit> {
        let cells = self.consumer_cells();
        if cells.is_empty() {
            return Ok(self.clone());
        }
        let mut d = self.clock.design.clone();
        for p in cells {
            d.blocks.insert(p, CONSUMER.to_string());
        }
        // the judge's lamps are not the design: its input drivers stay as assigned (webrender
        // 2026-10-20: a lamp just outside the build widened the box and flipped a driver)
        Ok(Circuit::from_design(d)?.keep_drivers(self, 0))
    }

    /// `self` (rebuilt from `from`'s design: moved, turned, or with the judge's lamps) with
    /// `from`'s input drivers (R7), turned by `turns` quarter turns: the judge's repeater stays on
    /// the same side of the design in every build.
    fn keep_drivers(mut self, from: &Circuit, turns: u8) -> Circuit {
        for (i, f) in self.inputs.iter_mut().zip(&from.inputs) {
            if i.name != f.name {
                continue;
            }
            match f.how {
                InputHow::Driven { dir, through } => i.how = InputHow::Driven { dir: clock::rotate_pos(dir, turns), through },
                _ if matches!(i.how, InputHow::Driven { .. }) => i.how = f.how.clone(),
                _ => {}
            }
        }
        self.input_refused = from.input_refused.clone();
        self
    }

    /// The lamps whose lit state must follow an output ([`check::update_less`]): the judge's in
    /// every consumer cell, and a lamp the design itself puts in an output cell.
    pub fn watched_lamps(&self) -> Vec<Pos> {
        let mut v = self.consumer_cells();
        for o in &self.outputs {
            if self.clock.design.blocks.get(&o.pos).is_some_and(|s| s.starts_with("minecraft:redstone_lamp")) {
                v.push(o.pos);
            }
        }
        v.sort();
        v.dedup();
        v
    }

    /// `explode: on` (TNT dupers, the SURVIVABLE variation): the judge's game run lets every primed
    /// TNT explode instead of removing it; redsim does not model explosions (UNMODELED).
    /// TNT dupers explode by default (vjeux 2026-10-20: "this will blow up the machine"): the
    /// game run lets every primed TNT explode and the design must survive; `explode: off` turns
    /// that off for analysis only (such a design is not archived as a duper that works).
    pub fn explodes(&self) -> bool {
        let tnt = matches!(self.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered));
        tnt && self.clock.design.meta.get("explode").is_none_or(|v| v.trim() != "off")
    }

    /// The ground a TNT duper with explosions stands on (coordinator 2026-10-20, after vjeux 13:13Z
    /// "There's no way this tnt duper doesn't explode after a few seconds": in the void sandbox a
    /// dropped TNT fell away forever): `reach` blocks around the design box, its top `drop` air
    /// blocks under the design's lowest layer (0: right under it); the blocks, top layer first.
    pub fn ground_blocks(&self, ground: Ground, reach: i32, drop: i32) -> Vec<(Pos, String)> {
        let Some((lo, hi)) = self.clock.design.bounds() else { return Vec::new() };
        let mut v = Vec::new();
        for (k, st) in ground.layers().iter().enumerate() {
            for x in lo[0] - reach..=hi[0] + reach {
                for z in lo[2] - reach..=hi[2] + reach {
                    v.push(([x, lo[1] - 1 - drop - k as i32, z], st.to_string()));
                }
            }
        }
        v
    }

    /// The lowest layer of the design: a TNT duper's frames leave out the ground below it (its
    /// craters are not the machine).
    pub fn lowest_y(&self) -> i32 {
        self.clock.design.bounds().map(|(lo, _)| lo[1]).unwrap_or(0)
    }

    /// An output cell holding a block that drives nothing by itself (a stone, glass, a redstone
    /// block...): its "level" would be read by query alone, so a change without a block update would
    /// go unseen. An output cell is air (the judge's lamp goes there) or holds a redstone component
    /// or consumer (dust, repeater, comparator, torch, observer, lamp, piston, copper bulb, note
    /// block, door, trapdoor, fence gate, dispenser, dropper, crafter, rail, TNT, bell, hopper).
    /// Piston extenders and flying machines are read through payloads.
    pub fn passive_output(&self) -> Option<(String, Pos, String)> {
        if matches!(self.kind, Some(Kind::PistonExtender(_)) | Some(Kind::FlyingMachine) | Some(Kind::ShulkerLoader(_)) | Some(Kind::ShulkerUnloader(_))) {
            return None;
        }
        const ACTIVE: [&str; 20] = [
            "redstone_wire",
            "repeater",
            "comparator",
            "redstone_torch",
            "redstone_wall_torch",
            "observer",
            "redstone_lamp",
            "piston",
            "sticky_piston",
            "copper_bulb",
            "note_block",
            "door",
            "trapdoor",
            "fence_gate",
            "dispenser",
            "dropper",
            "crafter",
            "rail",
            "tnt",
            "bell",
        ];
        for o in &self.outputs {
            let Some(s) = self.clock.design.blocks.get(&o.pos) else { continue };
            let id = s.split(['[', '{']).next().unwrap_or(s);
            let id = id.strip_prefix("minecraft:").unwrap_or(id);
            if !ACTIVE.iter().any(|a| id == *a || id.ends_with(&format!("_{a}"))) && id != "hopper" {
                return Some((o.name.clone(), o.pos, id.to_string()));
            }
        }
        None
    }

    /// The design's blocks, except piston heads (part of their piston) and the blocks in input
    /// cells (a lever or button there is the stimulus device), plus design entities.
    pub fn blocks(&self) -> usize {
        // a lever, button or judge-written input cell is the input device, not a design block; a
        // driven input's own receiving block (R7 model B: dust, a repeater) is one; bedrock is the
        // world's (a player cannot place it: a bedrock breaker's roof)
        let ins: BTreeSet<Pos> = self.inputs.iter().filter(|i| !matches!(i.how, InputHow::Driven { .. })).map(|i| i.pos).collect();
        self.clock.design.blocks.iter().filter(|(p, s)| !ins.contains(*p) && !s.starts_with("minecraft:piston_head") && !s.starts_with("minecraft:bedrock")).count() + self.clock.design.entities.len()
    }

    /// The kinds of blocks it is built with (as `Clock::kinds`), input devices left out.
    pub fn kinds(&self) -> BTreeSet<String> {
        let mut d = self.clock.design.clone();
        for i in self.inputs.iter().filter(|i| !matches!(i.how, InputHow::Driven { .. })) {
            d.blocks.remove(&i.pos);
        }
        match Clock::from_design(d) {
            Ok(c) => c.kinds(),
            Err(_) => self.clock.kinds(),
        }
    }

    /// The box of its blocks and every port (inputs, outputs, payloads, the kick cell).
    pub fn bounds(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.clock.bounds();
        for p in self.clock.design.ports.values() {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        (lo, hi)
    }

    /// The same circuit moved by `d`.
    pub fn translated(&self, d: Pos) -> Result<Circuit> {
        Ok(Circuit::from_design(self.clock.design.translated(d))?.keep_drivers(self, 0))
    }

    /// The circuit turned `turns` quarter turns clockwise (seen from above) around the origin: the
    /// game turns every block state, the ports and the travel vector turn with it (custom input
    /// states are written as given).
    pub fn rotated(&self, w: &mut Worker, turns: u8) -> Result<Circuit> {
        let rc = clock::verify::rotated(w, &self.clock, turns)?;
        self.with_turned_design(rc.design, turns)
    }

    /// [`Circuit::rotated`] without the game: block states turned by `clock::canon::rotate_state`
    /// (redsim's judge checking the turned builds), design entities by the same continuous map
    /// as the game judge's (`(X, Z) -> (1 - Z, X)` per turn) with their data turned like the
    /// bridge turns it (`clock::turn_entity_nbt`; hanging entities are refused: the game turns
    /// their facing).
    pub fn turned_pure(&self, turns: u8) -> Result<Circuit> {
        let turns = turns % 4;
        if turns == 0 {
            return Ok(self.clone());
        }
        let c = &self.clock;
        let mut d = c.design.clone();
        d.entities = c
            .design
            .entities
            .iter()
            .map(|e| {
                let mut p = e.pos;
                for _ in 0..turns {
                    p = [1.0 - p[2], p[1], p[0]];
                }
                let nbt = clock::turn_entity_nbt(e.nbt.as_deref(), turns).map_err(|why| anyhow::anyhow!("{}: {why}", e.kind))?;
                Ok(mctas::design::EntitySpec { kind: e.kind.clone(), pos: p, nbt })
            })
            .collect::<Result<Vec<_>>>()?;
        d.blocks = c.design.blocks.iter().map(|(p, s)| (clock::rotate_pos(*p, turns), clock::canon::rotate_state(s, turns))).collect();
        d.ports = c.design.ports.iter().map(|(k, p)| (k.clone(), clock::rotate_pos(*p, turns))).collect();
        d.block_entities = c.design.block_entities.iter().map(|(p, s)| (clock::rotate_pos(*p, turns), s.clone())).collect();
        if let Some(clock::Kick::Place { state, ticks, .. }) = &c.kick {
            let at = if c.kick_at > 0 { format!(" at {}", c.kick_at) } else { String::new() };
            d.meta.insert("port start".into(), format!("{} {ticks}{at}", clock::canon::rotate_state(state, turns)));
        }
        self.with_turned_design(d, turns)
    }

    fn with_turned_design(&self, mut d: mctas::design::Design, turns: u8) -> Result<Circuit> {
        if let Some(t) = self.travel {
            let r = clock::rotate_pos(t, turns);
            d.meta.insert("travel".into(), format!("{} {} {}", r[0], r[1], r[2]));
        }
        if let Some(t) = self.tile {
            let r = clock::rotate_pos(t.step, turns);
            d.meta.insert("tile".into(), format!("{} {} {} {}", r[0], r[1], r[2], t.copies));
        }
        if let Some(st) = self.slice {
            let r = clock::rotate_pos(st, turns);
            d.meta.insert("slice".into(), format!("{} {} {}", r[0], r[1], r[2]));
        }
        let mut out = Circuit::from_design(d)?.keep_drivers(self, turns);
        out.drop = self.drop;
        Ok(out)
    }

    /// The judge's inputs for a schedule: at each input edge, the action applied between ticks
    /// (input order within a tick).
    pub fn actions(&self, s: &stimulus::Schedule) -> Vec<(u32, Action)> {
        let mut v = Vec::new();
        for (i, inp) in self.inputs.iter().enumerate() {
            for &(t, on) in s.edges.get(i).map(|e| e.as_slice()).unwrap_or(&[]) {
                let a = match (&inp.how, on) {
                    (InputHow::Block { on: st, .. }, true) => Action::Place { pos: inp.pos, state: st.clone(), nbt: None },
                    (InputHow::Block { off, .. }, false) => {
                        if mctas::blockstate::is_air(off) {
                            Action::Clear { pos: inp.pos }
                        } else {
                            Action::Place { pos: inp.pos, state: off.clone(), nbt: None }
                        }
                    }
                    // the switch behind the judge's repeater, 2 gt (its delay) before the edge
                    (InputHow::Driven { dir, .. }, on) => {
                        let (_, _, sw) = driver_cells(inp.pos, *dir);
                        let a = if on { Action::Place { pos: sw, state: INPUT_ON.to_string(), nbt: None } } else { Action::Clear { pos: sw } };
                        v.push((t.saturating_sub(2), a));
                        continue;
                    }
                    (InputHow::Lever, _) => Action::Use { pos: inp.pos },
                    (InputHow::Button, true) => Action::Use { pos: inp.pos },
                    (InputHow::Button, false) => continue,
                    // a strength input changes by `Schedule::levels`, not by edges
                    (InputHow::Strength { .. }, _) => continue,
                    // a frame turns by `Schedule::levels` (one click per step), not by edges
                    (InputHow::Frame { .. }, _) => continue,
                };
                v.push((t, a));
            }
        }
        // frames: one right-click per level step (each step is +1 of 8)
        for &(t, i, _) in &s.levels {
            if let Some(Input { pos, how: InputHow::Frame { .. }, .. }) = self.inputs.get(i) {
                v.push((t, Action::UseEntity { pos: *pos }));
            }
        }
        // strength inputs: the chest's contents rewritten (its state unchanged: no block update,
        // the contents change updates the comparators reading it)
        for &(t, i, value) in &s.levels {
            if let Some(Input { pos, how: InputHow::Strength { chest }, .. }) = self.inputs.get(i) {
                v.push((t, Action::Place { pos: *pos, state: chest.clone(), nbt: Some(compute::strength_items(value)) }));
            }
        }
        let probes = if s.events.iter().any(|e| matches!(e.1, stimulus::EventAt::Probe(_))) { self.probe_cells() } else { Vec::new() };
        for (t, at, op) in &s.events {
            let pos = match at {
                stimulus::EventAt::Target(i) => match self.targets.get(*i) {
                    Some(tg) => tg.pos,
                    None => continue,
                },
                stimulus::EventAt::Probe(k) => match probes.get(*k) {
                    Some(p) => *p,
                    None => continue,
                },
                stimulus::EventAt::Input(i) => match self.inputs.get(*i) {
                    Some(inp) => inp.pos,
                    None => continue,
                },
                stimulus::EventAt::Cell(p) => *p,
            };
            let a = match op {
                stimulus::TargetOp::Place(st) => Action::Place { pos, state: st.clone(), nbt: None },
                stimulus::TargetOp::Clear => Action::Clear { pos },
                stimulus::TargetOp::Shape(st) => Action::ShapeSet { pos, state: st.clone() },
                stimulus::TargetOp::Update => Action::Update { pos },
                stimulus::TargetOp::GameEvent(ev) => Action::GameEvent { pos, event: ev },
                stimulus::TargetOp::Drop(item) => Action::Drop { pos, item: item.clone() },
            };
            v.push((*t, a));
        }
        v.sort_by_key(|(t, _)| *t);
        v
    }

    /// Whether the design holds a sculk sensor (plain or calibrated): it hears game events, so
    /// the judge adds the vibration test ([`vibration`]).
    pub fn has_sculk(&self) -> bool {
        self.clock.design.blocks.values().any(|s| s.contains("sculk_sensor"))
    }

    /// The design declares that its input arrives by a line from beyond any sensor's reach
    /// (`input-device: far`): the vibration test emits no input-device events.
    pub fn input_far(&self) -> bool {
        self.clock.design.meta.get("input-device").is_some_and(|v| v.trim() == "far")
    }

    /// The design claims it is vibration-proof (`vibration: proof`): the vibration test refuses it
    /// when ambient game events change it ([`vibration`]).
    pub fn claims_vibration_proof(&self) -> bool {
        self.clock.design.meta.get("vibration").is_some_and(|v| v.trim() == "proof")
    }

    /// The air cells next to the design (face neighbors of its blocks) that are no port of it, in
    /// position order: where the BUD judge probes the footprint.
    pub fn probe_cells(&self) -> Vec<Pos> {
        let d = &self.clock.design;
        let ports: BTreeSet<Pos> = d.ports.values().copied().collect();
        let mut out: BTreeSet<Pos> = BTreeSet::new();
        for p in d.blocks.keys() {
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let q = [p[0] + dx, p[1] + dy, p[2] + dz];
                if !d.blocks.contains_key(&q) && !ports.contains(&q) {
                    out.insert(q);
                }
            }
        }
        out.into_iter().collect()
    }
}

/// An input port's driver from its arguments, checked against the design's block in that cell.
/// A `frame` input's item frame: exactly one item frame entity of the design in that cell (its
/// position written as the cell, e.g. `x.5 y.5 z.5`), holding an item; its `ItemRotation` (0..7).
fn frame_rotation(d: &mctas::design::Design, pos: Pos) -> Result<u8> {
    let here: Vec<_> = d
        .entities
        .iter()
        .filter(|e| e.kind.trim_start_matches("minecraft:") == "item_frame" && (0..3).all(|k| e.pos[k].floor() as i32 == pos[k]))
        .collect();
    let [e] = here.as_slice() else {
        bail!("frame input at {pos:?}: the design needs exactly one item frame entity in that cell (`entity: minecraft:item_frame x.5 y.5 z.5 {{Facing:..b,Item:{{..}}}}`), it has {}", here.len());
    };
    let nbt = e.nbt.as_deref().unwrap_or("");
    if !nbt.contains("Item:") {
        bail!("frame input at {pos:?}: the item frame must hold an item (an empty frame reads 0 and does not turn)");
    }
    let rot = nbt.split("ItemRotation:").nth(1).map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).unwrap_or_default();
    Ok(rot.parse::<u8>().unwrap_or(0) % 8)
}

fn parse_input(d: &mctas::design::Design, pos: Pos, args: &str) -> Result<InputHow> {
    let here = d.blocks.get(&pos).map(|s| s.as_str());
    let words: Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        ["lever"] => {
            let s = here.ok_or_else(|| anyhow!("lever input at {pos:?}: no design block there"))?;
            if !s.starts_with("minecraft:lever") {
                bail!("lever input at {pos:?}: the design has {s} there");
            }
            if s.contains("powered=true") {
                bail!("lever input at {pos:?}: the lever must be off at rest (powered=false)");
            }
            Ok(InputHow::Lever)
        }
        ["button"] => {
            let s = here.ok_or_else(|| anyhow!("button input at {pos:?}: no design block there"))?;
            if !s.contains("_button") {
                bail!("button input at {pos:?}: the design has {s} there");
            }
            Ok(InputHow::Button)
        }
        ["strength"] => {
            let s = here.ok_or_else(|| anyhow!("strength input at {pos:?}: the design must hold a chest there (the judge fills it)"))?;
            if !s.starts_with("minecraft:chest") || s.contains("type=left") || s.contains("type=right") {
                bail!("strength input at {pos:?}: the design must hold a single chest there (minecraft:chest[type=single]), it has {s}");
            }
            Ok(InputHow::Strength { chest: s.to_string() })
        }
        ["frame"] => Ok(InputHow::Frame { rotation: frame_rotation(d, pos)? }),
        _ => {
            let (on, off) = match words.as_slice() {
                [] => (INPUT_ON.to_string(), "minecraft:air".to_string()),
                [on] => (mctas::blockstate::BlockState::parse(on)?.to_string(), "minecraft:air".to_string()),
                [on, off] => (mctas::blockstate::BlockState::parse(on)?.to_string(), mctas::blockstate::BlockState::parse(off)?.to_string()),
                _ => bail!("want `lever`, `button` or `<on-state> [<off-state>]`, got {args:?}"),
            };
            let rest = here.unwrap_or("minecraft:air");
            // R7: a plain input may hold the design's own receiving block (dust for a comparator's
            // side, a repeater, a conductor...), which the judge's repeater powers from outside
            if words.is_empty() && here.is_some() && !input_device_block() {
                return Ok(InputHow::Block { on, off });
            }
            if !trace::state_matches(rest, &off) {
                bail!("input at {pos:?}: the design must hold its off-state {off} there (it has {rest})");
            }
            Ok(InputHow::Block { on, off })
        }
    }
}
