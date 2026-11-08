//! Study 17, auto-smelters (designs/circuits/tracks/smelter/NOTES.md, docs/circuits.md
//! "smelter N"): kind `smelter N`, a furnace array: items from an input chest and fuel from a fuel
//! chest spread across N furnaces, every result into one output chest (coordinator 2026-10-21,
//! under vjeux's standing bar "REAL USE IS THE BAR").
//!
//! Judged in the game by its own runner, like the sorter ([`crate::sorter`], whose benches it
//! shares): the judge puts a chest on `items` (the input, above the first input hopper), on `fuel`
//! (the fuel chest) and on `output` (where the results go), fills the input chest with a mixed load
//! of smeltables (raw gold, cobblestone, sand, logs, kelp, raw copper, iron tools: 16 items a
//! furnace) and the fuel chest with coal, empties the output chest every 20 gt, then waits for rest
//! (every furnace out) and takes an inventory. The runs ([`Run`]):
//! * `base` (4 positions x 4 rotations, the metrics): the design as built.
//! * `tiled` (required for a `tile:` slice, 4 rotations): 3 copies at the step, one input chest,
//!   one fuel chest (copy 0's), one output chest (the last copy's): a player's longer array.
//! * `intrusion` (required, 4 rotations; the real-use audit's ITEM INTRUSION): the run (tiled for
//!   a slice) with stray items dropped at rest on every open hopper top at a quarter, half and
//!   three quarters of the run: 1 cobblestone, 64 iron ingots (they do not smelt: in a furnace's
//!   input they block it for good), 1 iron shovel.
//!
//! Every run must: smelt every input item into its result in the output chest (an input burnt as
//! fuel, stuck or lost fails), put no fuel in the output chest nor in a furnace's input, waste no
//! fuel (the coal burnt is what each furnace's own items needed: ceil(items / 8) each), use every
//! furnace (the items spread across all N), leave nothing in the machine but fuel, and rest.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack, stacks_of_be};
use crate::sorter::{Bench, Frame, GameBench, STRAYS, SimBench, bp, held, on_top, sim_held, top_box};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// Copies in the tiled runs.
pub const COPIES: usize = 3;
/// The fuel the judge supplies.
pub const FUEL: &str = "minecraft:coal";
/// Items a coal smelts in a plain furnace (1600 / 200 gt).
pub const PER_COAL: u64 = 8;
/// The gt with nothing delivered before a stray lying on a hopper top is taken as never taken in.
pub const QUIET: u32 = 400;

/// A smeltable's result (redsim's smelting recipes: the judge's stream is all in its registry).
pub fn result_of(id: &str) -> Option<String> {
    use redsim::redstone::container as rc;
    let i = rc::item_id(id)?;
    rc::smelting(i).map(|(r, _)| format!("minecraft:{}", rc::item_name(r)))
}

/// A block the judge's input or fuel chest may replace (a cover on a line hopper): a full cube
/// that holds nothing (no container, no redstone component).
pub fn is_cover(state: &str) -> bool {
    match redsim::Block::parse(state) {
        Ok(b) => redsim::behavior::full_cube(&b) && !redsim::behavior::has_block_entity(&b) && !matches!(b, redsim::Block::Piston { .. } | redsim::Block::Observer { .. } | redsim::Block::Note { .. } | redsim::Block::Target { .. }),
        Err(_) => false,
    }
}

/// The judge's runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    /// A row of slices side by side with this many furnaces in all (4, 8, 16: [`ROWS`]).
    Tiled(u32),
    Intrusion,
    /// The super smelter's use (designs/briefs/super-smelter.md): a full double chest of raw iron
    /// (54 stacks, 3456) dumped in the input chest, the fuel chest kept stocked with coal; the
    /// number is the minutes until the last ingot reaches the output chest.
    Chest,
}

/// The super smelter's load: a double chest of raw iron.
pub const CHEST_LOAD: u32 = 54 * 64;

/// The rows a tileable slice is judged as (furnaces in all; coordinator 2026-10-21: players build
/// 8 to 16 furnace arrays).
pub const ROWS: [u32; 3] = [4, 8, 16];

impl Run {
    pub fn name(self) -> String {
        match self {
            Run::Base => "base".into(),
            Run::Tiled(4) => "tiled".into(),
            Run::Tiled(n) => format!("tiled{n}"),
            Run::Intrusion => "intrusion".into(),
            Run::Chest => "chest".into(),
        }
    }

    pub fn parse(s: &str) -> Option<Run> {
        Some(match s {
            "base" => Run::Base,
            "tiled" | "tileable" | "tiled4" => Run::Tiled(4),
            "tiled8" => Run::Tiled(8),
            "tiled16" => Run::Tiled(16),
            "intrusion" | "strays" => Run::Intrusion,
            "chest" | "double-chest" => Run::Chest,
            _ => return None,
        })
    }

    pub fn title(self) -> String {
        match self {
            Run::Base => "the array as built".into(),
            Run::Tiled(n) => format!("slices side by side, {n} furnaces in all (one input, one fuel and one output chest)"),
            Run::Intrusion => "stray items dropped on every open hopper top".into(),
            Run::Chest => "a double chest of raw iron, the fuel chest kept stocked".into(),
        }
    }
}

/// Copies of a slice of `n` furnaces for a row of `row` furnaces (at least 2).
pub fn copies_for(n: u32, row: u32) -> usize {
    (row.div_ceil(n.max(1)) as usize).max(2)
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    /// Furnaces in the run.
    pub furnaces: u32,
    /// The input chest's load and the fuel chest's.
    pub stream: Vec<Stack>,
    pub fuel: Vec<Stack>,
    /// The `intrusion` run: (gt, stack) dropped on every open hopper top at the first visit from
    /// that gt on.
    pub strays: Vec<(u32, Stack)>,
    pub cap: u32,
    /// The `chest` run: the furnaces' own limit, the gt the load takes with every furnace busy
    /// from the first tick (200 gt an item in a furnace, 100 in a blast furnace).
    pub ceiling: u32,
}

/// `n` of an item in stacks of its max.
fn stacks(id: &str, n: u32) -> Vec<Stack> {
    let max = crate::sorter::max_stack(id);
    let mut v = Vec::new();
    let mut left = n;
    while left > 0 {
        let k = left.min(max);
        v.push(Stack::new(id, k));
        left -= k;
    }
    v
}

impl Scenario {
    /// A run over `furnaces` furnaces: 16 items a furnace of six smeltables (two of them
    /// non-stackable iron shovels in all), coal for every item and two more a furnace (at least a
    /// stack).
    pub fn new(run: Run, furnaces: u32, intrusion: bool) -> Scenario {
        let n = furnaces.max(1);
        let mut stream = Vec::new();
        stream.extend(stacks("minecraft:raw_gold", 4 * n));
        stream.extend(stacks("minecraft:cobblestone", 3 * n));
        stream.extend(stacks("minecraft:iron_shovel", 1));
        stream.extend(stacks("minecraft:sand", 3 * n));
        stream.extend(stacks("minecraft:oak_log", 2 * n));
        stream.extend(stacks("minecraft:kelp", 2 * n));
        stream.extend(stacks("minecraft:iron_shovel", 1));
        stream.extend(stacks("minecraft:raw_copper", 2 * n));
        let total: u32 = stream.iter().map(|s| s.count).sum();
        let coal = (total.div_ceil(PER_COAL as u32) + 2 * n).max(64);
        let fuel = stacks(FUEL, coal);
        // every item through one furnace at 200 gt, + the burn-out and settling
        let cap = total * 200 + 4000;
        let strays = if intrusion {
            let span = total * 200 / n;
            STRAYS.iter().enumerate().map(|(k, (id, c))| (span * (k as u32 + 1) / 4, Stack::new(id, *c))).collect()
        } else {
            Vec::new()
        };
        Scenario { run, furnaces: n, stream, fuel, strays, cap, ceiling: 0 }
    }

    /// The `chest` run over furnaces cooking in `cook` gt each (200: a furnace, 100: a blast
    /// furnace): 54 stacks of raw iron, and the coal they need plus one stack (496), put in the
    /// fuel chest once (never refilled: the coal a player has).
    pub fn chest(cook: &[u32]) -> Scenario {
        let n = cook.len().max(1) as u32;
        let stream = stacks("minecraft:raw_iron", CHEST_LOAD);
        // the coal the load needs and one stack more, never refilled (coordinator 2026-10-21: the
        // supply a player has; a build that hoards it starves its last furnaces)
        let fuel = stacks(FUEL, CHEST_LOAD / PER_COAL as u32 + 64);
        let rate: f64 = cook.iter().map(|t| 1.0 / *t as f64).sum::<f64>().max(1.0 / 200.0);
        let ceiling = (CHEST_LOAD as f64 / rate).ceil() as u32;
        // four times the ceiling and an hour: an array whose furnaces sit idle still finishes
        let cap = ceiling * 4 + 72000;
        Scenario { run: Run::Chest, furnaces: n, stream, fuel, strays: Vec::new(), cap, ceiling }
    }

    pub fn total(&self) -> u64 {
        self.stream.iter().map(|s| s.count as u64).sum()
    }

    /// Stream items of a kind.
    pub fn supplied(&self, id: &str) -> u64 {
        self.stream.iter().filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }

    /// The source kinds (stream and strays).
    pub fn sources(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for s in self.stream.iter().chain(self.strays.iter().map(|s| &s.1)) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
        v
    }
}

/// One run.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// What reached the output chest, per kind.
    pub reached: BTreeMap<String, u64>,
    /// (gt, items delivered so far) at every change.
    pub flow: Vec<(u32, u64)>,
    pub end: u32,
    pub timed_out: bool,
    /// Strays dropped so far and still lying on an open hopper top at the end, per kind.
    pub dropped: BTreeMap<String, u64>,
    pub lying: BTreeMap<String, u64>,
    /// Fuel and input left in the judge's chests at the end.
    pub fuel_left: u64,
    pub input_left: BTreeMap<String, u64>,
    /// Per furnace (position order): the items it smelted, and its input slot at the end.
    pub smelted: Vec<(Pos, u64)>,
    /// Per furnace: the kinds in its input and fuel slots at the end (a jam's diagnosis).
    pub slots: Vec<(Pos, Option<String>, Option<String>)>,
    /// The design's containers: before the judge fills anything, and at the end.
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    pub loose: Vec<String>,
    pub restless: String,
    /// The whole build still at the end (no clock of its own running on).
    pub rests: bool,
    pub awake: u32,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn delivered(&self) -> u64 {
        self.reached.values().sum()
    }
}

/// Where the judge's things go.
struct Ports {
    items: Pos,
    /// The double input chest's other half (`port items.b:`).
    items_b: Option<Pos>,
    fuel: Pos,
    /// The double fuel chest's other half (`port fuel.b:`).
    fuel_b: Option<Pos>,
    output: Pos,
    /// Every judge container (left out of the design's own).
    judge: Vec<Pos>,
    /// The open hopper tops (strays), the judge's chests closing the ones they stand on.
    strays: Vec<Pos>,
    /// `smelter-run --trace`: the design's hoppers and furnaces, printed every SEG gt.
    trace: Vec<Pos>,
}

fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let get = |role: &str| c.stores.iter().find(|s| s.role == role).map(|s| s.pos).ok_or_else(|| format!("smelter: no `port {role}:`"));
    let items = c.stores.iter().find(|s| s.role == "items" && s.name == "items").or_else(|| c.stores.iter().find(|s| s.role == "items")).map(|s| s.pos).ok_or_else(|| "smelter: no `port items:`".to_string())?;
    // a double input chest (the super smelter: `port items.b:` beside `port items:`)
    let items_b = c.stores.iter().find(|s| s.name == "items.b").map(|s| s.pos);
    let fuel_b = c.stores.iter().find(|s| s.name == "fuel.b").map(|s| s.pos);
    let fuel = c.stores.iter().find(|s| s.name == "fuel").or_else(|| c.stores.iter().find(|s| s.role == "fuel")).map(|s| s.pos).ok_or_else(|| "smelter: no `port fuel:`".to_string())?;
    let output = c.outputs.first().map(|o| o.pos).ok_or_else(|| "smelter: no `port output:`".to_string())?;
    let mut judge = vec![items, fuel, output];
    judge.extend(items_b);
    judge.extend(fuel_b);
    // the judge's chests stand in every run: a stray can't pass through them either
    let mut blocked = crate::intrusion::device_cells(c);
    blocked.extend(judge.iter().copied());
    let strays = clock::intrusion::intakes(&c.clock.design, &blocked).into_iter().filter(|p| !judge.contains(p)).collect();
    let trace = if TRACE.load(std::sync::atomic::Ordering::Relaxed) { c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:hopper") || s.contains("furnace")).map(|(p, _)| *p).collect() } else { Vec::new() };
    Ok(Ports { items, items_b, fuel, fuel_b, output, judge, strays, trace })
}

/// The judge's chests: a single chest on every judge cell, a double chest across `items` and
/// `items.b` (left and right halves facing so they join: `ChestBlock.getConnectedDirection`, a
/// left half's other half is clockwise of its facing).
fn chest_states(p: &Ports) -> Vec<(Pos, String)> {
    let mut v: Vec<(Pos, String)> = p.judge.iter().map(|q| (*q, CHEST.to_string())).collect();
    for (a, b) in [(p.items, p.items_b), (p.fuel, p.fuel_b)] {
        let Some(b) = b else { continue };
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let (facing, a_left) = match d {
            [1, 0, 0] => ("north", true),
            [-1, 0, 0] => ("north", false),
            [0, 0, 1] => ("east", true),
            _ => ("east", false),
        };
        let st = |left: bool| format!("minecraft:chest[facing={facing},type={},waterlogged=false]", if left { "left" } else { "right" });
        for e in v.iter_mut() {
            if e.0 == a {
                e.1 = st(a_left);
            } else if e.0 == b {
                e.1 = st(!a_left);
            }
        }
    }
    v
}

/// The design's furnaces and blast furnaces (position order).
fn furnaces(c: &Circuit) -> Vec<Pos> {
    c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:furnace") || s.starts_with("minecraft:blast_furnace")).map(|(p, _)| *p).collect()
}

/// Each furnace's cook time for raw iron (a blast furnace cooks twice as fast: `FAST_FURNACE`).
pub fn cook_times(c: &Circuit) -> Vec<u32> {
    c.clock.design.blocks.values().filter_map(|s| if s.starts_with("minecraft:blast_furnace") { Some(100) } else if s.starts_with("minecraft:furnace") { Some(200) } else { None }).collect()
}

/// Run a scenario on a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in_rec(w, sb, c, sc, from, to, false).map(|r| r.map(|(log, _)| log));
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in_rec(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos, record: bool) -> Result<std::result::Result<(RunLog, Vec<Frame>), String>> {
    let p = match ports(c) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(q, _)| placed.get(**q).is_none_or(mctas::blockstate::is_air)).map(|(q, s)| (*q, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|q| (*q, c.clock.design.blocks.get(q).cloned().unwrap_or_default()));
    if let Some((q, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let chests = chest_states(&p);
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    let mut b = GameBench { w, sb, from, to, base: None, frames: record.then(Vec::new) };
    b.record()?;
    b.step(c.clock.free_from() + SETTLE)?;
    b.base = Some(b.w.get_region(sb, from, to)?);
    let mut log = RunLog { dims: shulker::design_dims(c), initial: held(b.w, sb, from, to, &p.judge)?, ..Default::default() };
    drive(&mut b, sc, &p, &mut log)?;
    let frames = b.frames.take().unwrap_or_default();
    finish(w, sb, c, from, to, &p, &mut log)?;
    Ok(Ok((log, frames)))
}

/// The run's loop: the chests filled, then every [`SEG`] gt the output counted and emptied and the
/// chests topped up (a load over 27 stacks); strays when due. Stops when every item (and stray)
/// is accounted for, or at the cap.
fn drive<B: Bench>(b: &mut B, sc: &Scenario, p: &Ports, log: &mut RunLog) -> Result<()> {
    let (mut cursor, mut fuel_cursor) = (0usize, 0usize);
    topup(b, p.items, &sc.stream, &mut cursor)?;
    if let Some(q) = p.items_b {
        topup(b, q, &sc.stream, &mut cursor)?;
    }
    topup(b, p.fuel, &sc.fuel, &mut fuel_cursor)?;
    let (mut t, mut quiet, mut strays) = (0u32, 0u32, 0usize);
    loop {
        if !p.trace.is_empty() && (TRACE_FINE.0..TRACE_FINE.1).contains(&t) {
            // tick by tick through the fine window: the line, top and side hoppers (y >= 0)
            for k in 0..SEG {
                b.step(1)?;
                let mut line = format!("tick {}", t + k + 1);
                for q in &p.trace {
                    let have = b.read(*q)?;
                    if q[1] >= 0 && !have.is_empty() {
                        line.push_str(&format!(" {q:?}{}", have.iter().map(|s| s.count).sum::<u32>()));
                    }
                }
                eprintln!("{line}");
            }
        } else {
            b.step(SEG)?;
        }
        t += SEG;
        b.note(t, &mut log.notes)?;
        if !p.trace.is_empty() && t <= TRACE_UNTIL {
            let mut line = format!("t={t}");
            for q in &p.trace {
                let have = b.read(*q)?;
                if !have.is_empty() {
                    line.push_str(&format!(" {q:?}[{}]", have.iter().map(|s| format!("{}x{}", clock::short(&s.id), s.count)).collect::<Vec<_>>().join(" ")));
                }
            }
            eprintln!("{line}");
        }
        while strays < sc.strays.len() && t >= sc.strays[strays].0 {
            let s = &sc.strays[strays].1;
            for q in &p.strays {
                b.drop_item([q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], s)?;
                *log.dropped.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            log.notes.push(format!("t={t}: the judge drops {} x{} on each of {} open hopper top(s) {:?}", clock::short(&s.id), s.count, p.strays.len(), p.strays));
            strays += 1;
        }
        let before = log.delivered();
        let have = b.read(p.output)?;
        if !have.is_empty() {
            for s in &have {
                *log.reached.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(p.output, &[])?;
        }
        let now = log.delivered();
        if now != before {
            log.flow.push((t, now));
            quiet = 0;
        } else {
            quiet += SEG;
        }
        topup(b, p.items, &sc.stream, &mut cursor)?;
        if let Some(q) = p.items_b {
            topup(b, q, &sc.stream, &mut cursor)?;
        }
        topup(b, p.fuel, &sc.fuel, &mut fuel_cursor)?;
        let dropped: u64 = log.dropped.values().sum();
        let all = cursor >= sc.stream.len() && strays >= sc.strays.len();
        let lying: u64 = if dropped > 0 && now < sc.total() + dropped {
            let mut n = 0;
            for q in &p.strays {
                let (lo, hi) = top_box(*q);
                n += b.items_in(lo, hi)?.iter().map(|x| x.1).sum::<u64>();
            }
            n
        } else {
            0
        };
        let done = all && now + lying >= sc.total() + dropped && (lying == 0 || quiet >= QUIET);
        if done || t >= sc.cap {
            log.timed_out = !done;
            break;
        }
    }
    for q in &p.strays {
        let (lo, hi) = top_box(*q);
        for (id, n) in b.items_in(lo, hi)? {
            *log.lying.entry(id).or_insert(0) += n;
        }
    }
    log.fuel_left = b.read(p.fuel)?.iter().chain(match p.fuel_b { Some(q) => b.read(q)?, None => Vec::new() }.iter()).filter(|s| s.id == FUEL).map(|s| s.count as u64).sum();
    for s in b.read(p.items)?.into_iter().chain(match p.items_b { Some(q) => b.read(q)?, None => Vec::new() }) {
        *log.input_left.entry(s.id.clone()).or_insert(0) += s.count as u64;
    }
    log.end = t;
    Ok(())
}

/// Top a judge chest up from a list of stacks (27 slots).
fn topup<B: Bench>(b: &mut B, at: Pos, list: &[Stack], cursor: &mut usize) -> Result<()> {
    if *cursor >= list.len() {
        return Ok(());
    }
    let mut have = b.read(at)?;
    let before = have.len();
    while have.len() < 27 && *cursor < list.len() {
        have.push(list[*cursor].clone());
        *cursor += 1;
    }
    if have.len() != before {
        b.write(at, &have)?;
    }
    Ok(())
}

/// The items a furnace block entity's saved data says it smelted (`RecipesUsed`, summed).
fn smelted_of_nbt(nbt: &str) -> u64 {
    use redsim::redstone::container::{Snbt, parse_snbt};
    let Ok(Snbt::Compound(fields)) = parse_snbt(nbt) else { return 0 };
    let Some((_, Snbt::Compound(used))) = fields.iter().find(|(k, _)| k == "RecipesUsed") else { return 0 };
    used.iter()
        .map(|(_, v)| match v {
            Snbt::Int(n) => *n as u64,
            Snbt::Word(w) => w.trim_end_matches(['b', 's', 'L', 'l']).parse().unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// The item ids in a furnace block entity's input and fuel slots (slots 0 and 1).
fn slots_of_nbt(nbt: &str) -> (Option<String>, Option<String>) {
    use redsim::redstone::container::{Snbt, parse_snbt};
    let Ok(Snbt::Compound(fields)) = parse_snbt(nbt) else { return (None, None) };
    let Some((_, Snbt::List(items))) = fields.iter().find(|(k, _)| k == "Items") else { return (None, None) };
    let mut out = (None, None);
    for e in items {
        let Snbt::Compound(f) = e else { continue };
        let slot = f.iter().find(|(k, _)| k == "Slot").and_then(|(_, v)| match v {
            Snbt::Int(n) => Some(*n),
            Snbt::Word(w) => w.trim_end_matches(['b', 'B']).parse().ok(),
            _ => None,
        });
        let id = f.iter().find(|(k, _)| k == "id").and_then(|(_, v)| match v {
            Snbt::Str(s) | Snbt::Word(s) => Some(s.clone()),
            _ => None,
        });
        match slot {
            Some(0) => out.0 = id,
            Some(1) => out.1 = id,
            _ => {}
        }
    }
    out
}

/// After the run: rest (every furnace out), the design's containers, the furnaces' counts, the
/// entities left (strays lying on an open hopper top apart).
fn finish(w: &mut Worker, sb: u32, c: &Circuit, from: Pos, to: Pos, p: &Ports, log: &mut RunLog) -> Result<()> {
    let mut a = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
    let mut b = a.clone();
    let mut status = w.status(sb)?;
    for _ in 0..6 {
        w.step(sb, REST)?;
        b = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        status = w.status(sb)?;
        if a == b && status.block_ticks + status.block_events + status.moving_pistons == 0 {
            break;
        }
        a = b.clone();
    }
    // a furnace still lit (burning with nothing to smelt) fails; anything else still moving (a
    // design's own clock) is a metric: `at rest` no
    log.rests = a == b && status.block_ticks + status.block_events + status.moving_pistons == 0;
    if log.rests {
        // (a clock of the design's own can be in the same state at two samples REST apart: one
        // more look 7 gt later, off its beat)
        w.step(sb, 7)?;
        let c2 = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        log.rests = c2 == b;
        b = c2;
    }
    if let Some(f) = furnaces(c).into_iter().find(|f| b.0.get(*f).is_some_and(|s| s.contains("lit=true"))) {
        log.restless = format!("the furnace at {f:?} still burns {} gt after the last item", 6 * REST);
    }
    let _ = &a;
    log.awake = b.0.iter().filter(|(q, s)| c.clock.design.blocks.contains_key(q) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
    log.final_held = held(w, sb, from, to, &p.judge)?;
    let fs = furnaces(c);
    let bes = &b.1;
    log.smelted = fs.iter().map(|f| (*f, bes.iter().find(|be| be.pos == *f).map(|be| smelted_of_nbt(&be.nbt)).unwrap_or(0))).collect();
    log.slots = fs.iter().map(|f| {
        let (i, u) = bes.iter().find(|be| be.pos == *f).map(|be| slots_of_nbt(&be.nbt)).unwrap_or((None, None));
        (*f, i, u)
    }).collect();
    // a furnace's input slot at the end (the table: a jammed furnace shows what blocks it)
    for f in &fs {
        if let Some(be) = bes.iter().find(|be| be.pos == *f) {
            let st = stacks_of_be(&be.nbt);
            if !st.is_empty() {
                log.notes.push(format!("furnace at {f:?} holds {}", st.iter().map(|s| format!("{} x{}", clock::short(&s.id), s.count)).collect::<Vec<_>>().join(", ")));
            }
        }
    }
    for e in w.entities(sb)? {
        if (e.design && e.kind != "minecraft:item") || on_top(&p.strays, e.pos) {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    Ok(())
}

/// The same run on redsim (milliseconds). Err: the design cannot stand or redsim does not model it.
/// `smelter-run --coal-log`: redsim runs print, per furnace, the gt each coal reached its fuel slot.
pub static COAL_LOG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `smelter-run --trace`: every hopper's and furnace's contents every 20 gt up to [`TRACE_UNTIL`] (stderr).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub const TRACE_UNTIL: u32 = 80000;
/// `smelter-run --trace`: the gt span traced tick by tick (the line, top and side hoppers).
pub const TRACE_FINE: (u32, u32) = (340, 400);

pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<RunLog, String> {
    sim_run_rec(c, sc, margin, false).map(|(log, _)| log)
}

/// The redsim twin's frames of a run, every gt from placement (each non-air cell of the watch box
/// with its block entity's data), for the 3D view's check (`mctas web check`: the view replays a
/// smelter's base run, a super smelter's chest run).
pub fn sim_frames(c: &Circuit, run: Run, margin: i32) -> std::result::Result<Vec<Frame>, String> {
    let (sc, rc) = scenario_for(c, run).map_err(|e| format!("{e:#}"))?;
    sim_run_rec(&rc, &sc, margin, true).map(|(_, f)| f)
}

fn sim_run_rec(c: &Circuit, sc: &Scenario, margin: i32, record: bool) -> std::result::Result<(RunLog, Vec<Frame>), String> {
    let (from, to) = crate::run::watch_box(c, margin);
    if let Some((q, st)) = clock::sim::unsupported(&c.clock, from, to)?.first() {
        return Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let p = ports(c)?;
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    for (q, st) in chest_states(&p) {
        w.set_block(bp(q), redsim::Block::parse(&st)?, redsim::world::flags::ALL);
    }
    let watch = COAL_LOG.load(std::sync::atomic::Ordering::Relaxed).then(|| crate::sorter::CoalWatch::new(furnaces(c)));
    let mut b = SimBench { w: &mut w, from, to, frames: record.then(Vec::new), coal: watch, watch: None };
    b.record();
    b.step(c.clock.free_from() + SETTLE).map_err(|e| format!("{e:#}"))?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: sim_held(b.w, &p.judge), ..Default::default() };
    drive(&mut b, sc, &p, &mut log).map_err(|e| format!("{e:#}"))?;
    if let Some(cw) = b.coal.take() {
        for l in cw.intake_report() {
            eprintln!("{l}");
        }
        for (f, got) in cw.at.iter().zip(&cw.got) {
            eprintln!("coal log: furnace {f:?} got {} coal at gt {}", got.len(), got.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" "));
        }
        let mut held: Vec<(Pos, u32)> = Vec::new();
        for (q, x) in b.w.containers.map.iter() {
            let n: u32 = x.slots.iter().flatten().filter(|s| redsim::redstone::container::item_name(s.item) == "coal").map(|s| s.count as u32).sum();
            if n > 0 {
                held.push(([q.x, q.y, q.z], n));
            }
        }
        held.sort();
        eprintln!("coal log: coal held at the end: {}", held.iter().map(|(q, n)| format!("{q:?} {n}")).collect::<Vec<_>>().join(", "));
    }
    let frames = b.frames.take().unwrap_or_default();
    let mut key = w.state_key();
    let mut still = false;
    for _ in 0..6 {
        for _ in 0..REST {
            w.tick();
        }
        let k = w.state_key();
        if k == key && w.idle() {
            still = true;
            break;
        }
        key = k;
    }
    log.rests = still;
    if let Some(f) = furnaces(c).into_iter().find(|f| matches!(w.get(bp(*f)), redsim::Block::Furnace { lit: true, .. })) {
        log.restless = format!("the furnace at {f:?} still burns {} gt after the last item", 6 * REST);
    }
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    log.awake = c.clock.design.blocks.keys().filter(|q| matches!(w.get(bp(**q)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sim_held(&w, &p.judge);
    log.smelted = furnaces(c).iter().map(|f| (*f, w.containers.map.get(&bp(*f)).map(|x| x.smelted as u64).unwrap_or(0))).collect();
    {
        use redsim::redstone::container::item_name;
        let name = |s: Option<redsim::redstone::container::Stack>| s.map(|s| format!("minecraft:{}", item_name(s.item)));
        log.slots = furnaces(c).iter().map(|f| {
            let x = w.containers.map.get(&bp(*f));
            (*f, x.and_then(|x| name(x.slots[0])), x.and_then(|x| name(x.slots[1])))
        }).collect();
    }
    for f in furnaces(c) {
        if let Some(x) = w.containers.map.get(&bp(f)) {
            let st: Vec<String> = x.slots.iter().flatten().map(|s| format!("{} x{}", redsim::redstone::container::item_name(s.item), s.count)).collect();
            if !st.is_empty() {
                log.notes.push(format!("furnace at {f:?} holds {}", st.join(", ")));
            }
        }
    }
    log.loose = w.entities.list.iter().filter(|e| !e.removed && !on_top(&p.strays, [e.pos.x, e.pos.y, e.pos.z])).map(|e| format!("{:?}", e.kind)).collect();
    if let Some((q, bl)) = w.drops.first() {
        log.loose.push(format!("{bl} at {q} broke with a drop"));
    }
    Ok((log, frames))
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(sc: &Scenario, log: &RunLog) -> Outcome {
    if sc.run == Run::Chest {
        return judge_chest(sc, log);
    }
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let mut table: Vec<String> = vec![format!("output: {}", l(&log.reached))];
    table.push(format!("supplied {} items and {} coal, delivered {}, end t={}", sc.total(), sc.fuel.iter().map(|s| s.count as u64).sum::<u64>(), log.delivered(), log.end));
    table.push(format!("smelted per furnace: {}", log.smelted.iter().map(|(p, n)| format!("{p:?} {n}")).collect::<Vec<_>>().join(", ")));
    if !log.dropped.is_empty() {
        table.push(format!("strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), if log.lying.is_empty() { "none".to_string() } else { l(&log.lying) }));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    // the output: results, and strays (raw or smelted); never fuel, never a raw stream item
    if let Some(n) = log.reached.get(FUEL) {
        return fail(format!("{n} {} reached the output chest (fuel mixed into the results)", clock::short(FUEL)), table);
    }
    let results: BTreeMap<String, String> = sc.sources().into_iter().filter_map(|s| result_of(&s).map(|r| (r, s))).collect();
    for (k, n) in &log.reached {
        if results.contains_key(k) {
            continue;
        }
        let allowed = log.dropped.get(k).copied().unwrap_or(0);
        if *n > allowed {
            let what = if sc.supplied(k) > 0 { "never smelted" } else { "not a result of the load" };
            return fail(format!("{} x{} reached the output chest ({what})", clock::short(k), n - allowed), table);
        }
    }
    if log.timed_out {
        // why it jams: a furnace's input that does not smelt, or items without fuel
        let mut why = String::new();
        for (p, input, fuel) in &log.slots {
            match input {
                Some(i) if i == FUEL => why = format!(": {} in the input slot of the furnace at {p:?} (fuel mixed into the input does not smelt)", clock::short(i)),
                Some(i) if result_of(i).is_none() => why = format!(": {} in the input slot of the furnace at {p:?} does not smelt", clock::short(i)),
                Some(i) if fuel.is_none() => why = format!(": the furnace at {p:?} holds {} but no fuel (the fuel does not reach it)", clock::short(i)),
                _ => {}
            }
            if !why.is_empty() {
                break;
            }
        }
        return fail(format!("{} of {} items delivered after {} gt (the cap): it jams{why}", log.delivered(), sc.total() + log.dropped.values().sum::<u64>(), log.end), table);
    }
    if let Some((k, n)) = log.initial.iter().find(|(k, _)| burns(k)) {
        return fail(format!("the design holds {n} {} of its own (fuel comes from the fuel chest)", clock::short(k)), table);
    }
    // every source item accounted for: its result, itself (a stray), or lying on a hopper top
    for s in sc.sources() {
        let want = sc.supplied(&s) + log.dropped.get(&s).copied().unwrap_or(0);
        let got = result_of(&s).and_then(|r| log.reached.get(&r).copied()).unwrap_or(0) + if result_of(&s).is_none() || log.dropped.contains_key(&s) { log.reached.get(&s).copied().unwrap_or(0) } else { 0 } + log.lying.get(&s).copied().unwrap_or(0);
        if got != want {
            let why = if got < want && burns(&s) { " (burnt as fuel or stuck)" } else { "" };
            return fail(format!("{got} of {want} {} accounted for{why}", clock::short(&s)), table);
        }
    }
    if let Some((k, n)) = log.input_left.iter().next() {
        return fail(format!("{} x{n} never left the input chest", clock::short(k)), table);
    }
    // nothing left in the machine but fuel
    for (k, n) in &log.final_held {
        let was = log.initial.get(k).copied().unwrap_or(0);
        if k != FUEL && *n != was {
            return fail(format!("{} x{} left in the machine (only fuel may stay)", clock::short(k), n.saturating_sub(was)), table);
        }
    }
    // fuel: what each furnace's own items needed, no more
    let supplied: u64 = sc.fuel.iter().map(|s| s.count as u64).sum();
    let kept = log.final_held.get(FUEL).copied().unwrap_or(0).saturating_sub(log.initial.get(FUEL).copied().unwrap_or(0));
    let burnt = supplied.saturating_sub(log.fuel_left + kept);
    // the items' own coal (8 a coal) and one more a furnace: each furnace burns its last coal to
    // the end, and a short wait between two items costs a little; a furnace lit again and again
    // for a few items at a time burns far more
    let smelted: u64 = log.smelted.iter().map(|(_, n)| n).sum();
    let allowed = (smelted + PER_COAL * log.smelted.len() as u64) / PER_COAL;
    if burnt > allowed {
        return fail(format!("fuel wasted: {burnt} coal burnt for {smelted} items in {} furnace(s) (at most {allowed}: the items' own coal and one more a furnace)", log.smelted.len()), table);
    }
    if let Some((p, _)) = log.smelted.iter().find(|(_, n)| *n == 0) {
        return fail(format!("the furnace at {p:?} never smelted (the items are not spread across the {} furnaces)", log.smelted.len()), table);
    }
    // shared out: every furnace smelts at least two thirds of an even share (a row whose first or
    // last furnace does most of the work is no faster than a smaller array)
    let even = smelted / log.smelted.len().max(1) as u64;
    if let Some((p, n)) = log.smelted.iter().find(|(_, n)| 3 * *n < 2 * even) {
        return fail(format!("the furnace at {p:?} smelted {n} of {smelted} items (an even share is {even}: the items are not shared out across the {} furnaces)", log.smelted.len()), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    if !log.restless.is_empty() {
        return fail(log.restless.clone(), table);
    }
    let total = sc.total();
    let at = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).copied();
    let rate = match (at(total.div_ceil(10)), at(total * 9 / 10)) {
        (Some((ta, qa)), Some((tb, qb))) if tb > ta => (qb - qa) as f64 * 20.0 * 60.0 / (tb - ta) as f64,
        _ => 0.0,
    };
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let (lo, hi) = (log.smelted.iter().map(|x| x.1).min().unwrap_or(0), log.smelted.iter().map(|x| x.1).max().unwrap_or(0));
    let metrics = vec![
        ("items/min per furnace".to_string(), format!("{:.1}", rate / sc.furnaces.max(1) as f64)),
        ("first item".to_string(), format!("by {first} gt")),
        ("items per coal".to_string(), if burnt == 0 { "n/a".to_string() } else { format!("{:.1}", smelted as f64 / burnt as f64) }),
        ("spread".to_string(), if lo == hi { format!("{lo} each") } else { format!("{lo}-{hi}") }),
        ("hoppers awake".to_string(), log.awake.to_string()),
        ("at rest".to_string(), if log.rests { "yes".to_string() } else { "no, a clock runs on".to_string() }),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The `chest` run's must-haves (designs/briefs/super-smelter.md): every raw iron into an ingot in
/// the output chest, no fuel there, nothing jams, nothing left in the machine but fuel, nothing on
/// the floor. Not demanded: an even share between the furnaces, coal accounting, rest. The number:
/// minutes until the last ingot, against the furnaces' own ceiling.
fn judge_chest(sc: &Scenario, log: &RunLog) -> Outcome {
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let min = |gt: u32| gt as f64 / 1200.0;
    let mut table: Vec<String> = vec![format!("output: {}", l(&log.reached))];
    table.push(format!("smelted per furnace: {}", log.smelted.iter().map(|(p, n)| format!("{p:?} {n}")).collect::<Vec<_>>().join(", ")));
    let delivered_by = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).map(|x| x.0);
    table.push(format!(
        "ingots out by the quarter: {}",
        [1u64, 2, 3, 4].iter().map(|k| delivered_by(sc.total() * k / 4).map(|t| format!("{:.1} min", min(t))).unwrap_or_else(|| "-".into())).collect::<Vec<_>>().join(", ")
    ));
    table.push(format!("in the machine at the end: {}", l(&log.final_held)));
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    if let Some(n) = log.reached.get(FUEL) {
        return fail(format!("{n} {} reached the output chest (fuel mixed into the ingots)", clock::short(FUEL)), table);
    }
    if let Some((k, n)) = log.reached.iter().find(|(k, _)| k.as_str() != "minecraft:iron_ingot") {
        return fail(format!("{} x{n} reached the output chest (iron ingots only)", clock::short(k)), table);
    }
    if log.timed_out {
        let mut why = String::new();
        for (p, input, fuel) in &log.slots {
            if let Some(i) = input
                && fuel.is_none()
            {
                why = format!(": the furnace at {p:?} holds {} but no fuel (the fuel does not reach it)", clock::short(i));
                break;
            }
        }
        let coal_left = log.fuel_left + log.final_held.get(FUEL).copied().unwrap_or(0);
        return fail(format!("{:.0}% smelted ({} of {} ingots) after {:.0} min (the cap), coal left {coal_left}: it stops{why}", 100.0 * log.delivered() as f64 / sc.total().max(1) as f64, log.delivered(), sc.total(), min(log.end)), table);
    }
    if let Some((k, n)) = log.initial.iter().find(|(k, _)| burns(k)) {
        return fail(format!("the design holds {n} {} of its own (fuel comes from the fuel chest)", clock::short(k)), table);
    }
    let ingots = log.reached.get("minecraft:iron_ingot").copied().unwrap_or(0);
    if ingots != sc.total() {
        return fail(format!("{ingots} of {} ingots reached the output chest", sc.total()), table);
    }
    if let Some((k, n)) = log.input_left.iter().next() {
        return fail(format!("{} x{n} never left the input chest", clock::short(k)), table);
    }
    for (k, n) in &log.final_held {
        let was = log.initial.get(k).copied().unwrap_or(0);
        if k != FUEL && *n != was {
            return fail(format!("{} x{} left in the machine (only fuel may stay)", clock::short(k), n.saturating_sub(was)), table);
        }
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    let last = log.flow.last().map(|x| x.0).unwrap_or(log.end);
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let (lo, hi) = (log.smelted.iter().map(|x| x.1).min().unwrap_or(0), log.smelted.iter().map(|x| x.1).max().unwrap_or(0));
    let held = log.final_held.get(FUEL).copied().unwrap_or(0);
    let metrics = vec![
        ("minutes for a double chest".to_string(), format!("{:.1}", min(last))),
        ("of the ceiling".to_string(), format!("{:.0}% ({:.1} min: {} furnaces always busy)", 100.0 * sc.ceiling as f64 / last.max(1) as f64, min(sc.ceiling), sc.furnaces)),
        ("first ingot".to_string(), format!("by {first} gt")),
        ("spread".to_string(), if lo == hi { format!("{lo} each") } else { format!("{lo}-{hi} a furnace") }),
        ("coal left".to_string(), format!("{} ({held} in the machine)", held + log.fuel_left)),
        ("at rest".to_string(), if log.rests { "yes".to_string() } else { "no, a clock runs on".to_string() }),
        ("hoppers awake".to_string(), log.awake.to_string()),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// An item a furnace can burn (an input that would be lost as fuel: logs).
fn burns(id: &str) -> bool {
    use redsim::redstone::container as rc;
    rc::item_id(id).is_some_and(|i| rc::burn_time(i) > 0)
}

/// The tiled design: `copies` copies at the `tile:` step; ports copy 0's `items` and `fuel`, the
/// last copy's `output`.
pub fn tiled(c: &Circuit, copies: usize) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let n = copies as i32;
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), n.to_string());
    d.ports.retain(|name, _| name == "items" || name == "fuel");
    // `tile slice: lo hi`: only the cells from lo to hi along the step's axis repeat; the cells
    // before are the row's start piece, those after its end piece (moved past the last copy): an
    // array with fixed ends, as players build them
    let axis = (0..3).find(|i| t.step[*i] != 0).unwrap_or(0);
    let span = slice_span(c)?;
    let in_slice = |p: &Pos| span.is_none_or(|(lo, hi)| p[axis] >= lo && p[axis] <= hi);
    let after = |p: &Pos| span.is_some_and(|(_, hi)| p[axis] > hi);
    let last = [t.step[0] * (n - 1), t.step[1] * (n - 1), t.step[2] * (n - 1)];
    let add = |a: &Pos, b: &Pos| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    if span.is_some() {
        // the end piece moves past the last copy
        d.blocks.retain(|p, _| !after(p));
        d.block_entities.retain(|p, _| !after(p));
        for (p, s) in base.blocks.iter().filter(|(p, _)| after(p)) {
            d.blocks.insert(add(p, &last), s.clone());
        }
        for (p, s) in base.block_entities.iter().filter(|(p, _)| after(p)) {
            d.block_entities.insert(add(p, &last), s.clone());
        }
    }
    for k in 1..n {
        let off = [t.step[0] * k, t.step[1] * k, t.step[2] * k];
        for (p, s) in base.blocks.iter().filter(|(p, _)| in_slice(p)) {
            let q = add(p, &off);
            if let Some(o) = d.blocks.get(&q) {
                bail!("tile: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
            }
            d.blocks.insert(q, s.clone());
        }
        for (p, s) in base.block_entities.iter().filter(|(p, _)| in_slice(p)) {
            d.block_entities.insert(add(p, &off), s.clone());
        }
    }
    if let Some(p) = base.ports.get("output") {
        // the output chest: past the last copy (in the end piece or the last slice), or in the
        // start piece where the design puts it there
        let before = span.is_some_and(|(lo, _)| p[axis] < lo);
        d.ports.insert("output".into(), if before { *p } else { add(p, &last) });
    }
    for (name, p) in &d.ports {
        // (the input and fuel chests may replace copy 0's covers)
        if let Some(s) = d.blocks.get(p)
            && !(matches!(name.as_str(), "items" | "fuel") && is_cover(s))
        {
            bail!("tile: port {name} at {p:?} lands on a copy's block");
        }
    }
    Circuit::from_design(d).context("tiled smelter")
}

/// A design's `tile slice: lo hi` (the repeating cells along the tile's axis), checked against
/// the step (the slice is exactly one step wide).
/// Blocks per furnace: a row with end pieces counts its repeated slice (what each more furnace
/// costs) and names the end pieces' blocks apart; otherwise every block over the furnaces.
pub fn per_furnace(c: &Circuit, n: u32) -> Vec<(String, String)> {
    let n = n.max(1) as f64;
    match (slice_span(c).ok().flatten(), c.tile) {
        (Some((lo, hi)), Some(t)) => {
            let axis = (0..3).find(|i| t.step[*i] != 0).unwrap_or(0);
            let slice = c.clock.design.blocks.keys().filter(|p| p[axis] >= lo && p[axis] <= hi).count();
            let ends = c.blocks() as usize - slice;
            vec![("blocks per furnace".to_string(), format!("{:.1}", slice as f64 / n)), ("end pieces".to_string(), format!("{ends} blocks"))]
        }
        _ => vec![("blocks per furnace".to_string(), format!("{:.1}", c.blocks() as f64 / n))],
    }
}

/// A design's `tile slice: lo hi` (the repeating cells along the tile's axis), checked against
/// the step (the slice is exactly one step wide).
pub fn slice_span(c: &Circuit) -> Result<Option<(i32, i32)>> {
    let Some(v) = c.clock.design.meta.get("tile slice") else { return Ok(None) };
    let w: Vec<i32> = v.split_whitespace().filter_map(|s| s.parse().ok()).collect();
    let [lo, hi] = w[..] else { bail!("tile slice: give the first and last coordinate along the tile's axis (`tile slice: 0 0`), got {v:?}") };
    let t = c.tile.ok_or_else(|| anyhow!("tile slice: needs a `tile:` header"))?;
    let step = t.step.iter().map(|s| s.abs()).sum::<i32>();
    if hi < lo || hi - lo + 1 != step || t.step.iter().filter(|s| **s != 0).count() != 1 || t.step.iter().any(|s| *s < 0) {
        bail!("tile slice: {lo} {hi} must be exactly one step ({:?}) wide, along a positive axis", t.step);
    }
    Ok(Some((lo, hi)))
}

/// The scenario and the circuit (the row for a tiled run) of one run.
pub fn scenario_for(c: &Circuit, run: Run) -> Result<(Scenario, Circuit)> {
    let Some(Kind::Smelter(n)) = &c.kind else { bail!("{}: not a smelter", c.name()) };
    match run {
        Run::Base => Ok((Scenario::new(run, *n, false), c.clone())),
        Run::Chest => Ok((Scenario::chest(&cook_times(c)), c.clone())),
        Run::Tiled(row) => {
            if c.tile.is_none() {
                bail!("{}: no `tile:` header (a fixed array is judged as built)", c.name());
            }
            let k = copies_for(*n, row);
            Ok((Scenario::new(run, *n * k as u32, false), tiled(c, k)?))
        }
        Run::Intrusion => {
            if c.tile.is_some() {
                let k = copies_for(*n, ROWS[0]);
                Ok((Scenario::new(run, *n * k as u32, true), tiled(c, k)?))
            } else {
                Ok((Scenario::new(run, *n, true), c.clone()))
            }
        }
    }
}

/// The runs a design must pass besides the base run.
fn required(c: &Circuit) -> Vec<Run> {
    if c.tile.is_some() {
        let mut v: Vec<Run> = ROWS.iter().map(|r| Run::Tiled(*r)).collect();
        v.push(Run::Intrusion);
        v
    } else {
        vec![Run::Intrusion]
    }
}

/// The judge (`mctas circuit verify`): the base run at N positions x 4 rotations, then the
/// required runs at 4 rotations each.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Smelter(n) = kind else { bail!("{}: not a smelter", kind.name()) };
    if c.clock.design.meta.get("judge").is_some_and(|j| j.trim() == "chest") {
        return verify_chest(pool, c, o, kind.clone());
    }
    let (sc, _) = scenario_for(c, Run::Base)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&sc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = sc.cap;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    let mut lines: Vec<String> = Vec::new();
    let mut row_rate = String::new();
    for run in required(c) {
        match run_rotations(pool, c, run, o) {
            Ok(outs) => {
                lines.push(format!("{}: passes (4 rotations)", run.title()));
                if let Run::Tiled(row) = run
                    && let Some(m) = outs.first().and_then(|k| k.metrics.iter().find(|(k, _)| k == "items/min per furnace"))
                {
                    row_rate = format!("{row_rate}{}{row}: {}", if row_rate.is_empty() { "" } else { ", " }, m.1);
                }
            }
            Err(why) => {
                rep.pass = false;
                rep.verdict = format!("{}: {why}", run.title());
                return Ok(rep);
            }
        }
    }
    let bill = shulker::bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        for (k2, v) in per_furnace(c, n) {
            k.outcome.metrics.push((k2, v));
        }
        if let Some(t) = c.tile {
            k.outcome.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
            k.outcome.metrics.push(("rows items/min per furnace".to_string(), row_rate));
        }
        k.outcome.metrics.push(("bill".to_string(), bill));
        k.outcome.table.extend(lines);
    }
    Ok(rep)
}

/// The super smelter's judge (a design with `judge: chest`; designs/briefs/super-smelter.md): the
/// chest run at N positions x 4 rotations, the bill on the base build.
fn verify_chest(pool: &Pool, c: &Circuit, o: &Options, kind: Kind) -> Result<VerifyReport> {
    let (sc, _) = scenario_for(c, Run::Chest)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&sc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = sc.cap;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    let bill = shulker::bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("coal needed".to_string(), format!("{} (the load's {} and a stack)", sc.fuel.iter().map(|s| s.count).sum::<u32>(), CHEST_LOAD / PER_COAL as u32)));
        k.outcome.metrics.push(("bill".to_string(), bill));
    }
    Ok(rep)
}

/// One run at 4 rotations (the first position): the outcomes, or Err(why) at the first failure.
fn run_rotations(pool: &Pool, c: &Circuit, run: Run, o: &Options) -> std::result::Result<Vec<Outcome>, String> {
    let (sc, tc) = scenario_for(c, run).map_err(|e| format!("{e:#}"))?;
    let jobs: Vec<u8> = vec![0, 1, 2, 3];
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = tc.rotated(w, t)?;
        game_run(w, &rc, &sc, o.margin)
    });
    let mut outs = Vec::new();
    for (t, r) in jobs.into_iter().zip(results) {
        match r {
            Ok(Ok(log)) => {
                let out = judge(&sc, &log);
                if !out.pass {
                    return Err(format!("rotation {t}: {}", out.why));
                }
                outs.push(out);
            }
            Ok(Err(why)) => return Err(format!("rotation {t}: {why}")),
            Err(e) => return Err(format!("rotation {t}: {e:#}")),
        }
    }
    Ok(outs)
}

/// A design aid (`mctas circuit smelter-run`): one run at rotation 0 in the game.
pub fn run_one(pool: &Pool, c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, &rc, &sc, margin));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(log) => Ok(judge(&sc, &log)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// A design aid (`smelter-run --run chest --rotations`): the chest run in the game at 4 rotations
/// (and the given positions), every outcome.
pub fn chest_rotations(pool: &Pool, c: &Circuit, margin: i32, positions: usize) -> Result<Vec<(u8, Pos, Outcome)>> {
    let (sc, _) = scenario_for(c, Run::Chest)?;
    let offsets = clock::offsets(positions);
    let jobs: Vec<(u8, Pos)> = [0u8, 1, 2, 3].iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        game_run(w, &rc, &sc, margin)
    });
    let mut out = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let o = match r? {
            Ok(log) => judge(&sc, &log),
            Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
        };
        out.push((t, off, o));
    }
    Ok(out)
}

/// [`run_one`] on redsim.
pub fn sim_one(c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    Ok(match sim_run(&rc, &sc, margin) {
        Ok(log) => judge(&sc, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The whole judge on redsim at rotation 0 (`smelter-run --sim --run all`, the searches' screen):
/// the base run's outcome, failing with the first required run that fails; plus the score for
/// `circuit compact` (runs passed x 1000 + items delivered by the first failing run).
pub fn sim_score(c: &Circuit) -> (i64, Outcome) {
    let Some(Kind::Smelter(n)) = &c.kind else { return (0, Outcome::fail("not a smelter".to_string(), Vec::new())) };
    let mut passed = 0i64;
    let mut base: Option<Outcome> = None;
    let mut row_rate = String::new();
    let mut runs = vec![Run::Base];
    runs.extend(required(c));
    for r in runs {
        let (sc, rc) = match scenario_for(c, r) {
            Ok(x) => x,
            Err(e) => return (passed * 1000, Outcome::fail(format!("{e:#}"), Vec::new())),
        };
        let (out, k) = match sim_run(&rc, &sc, 3) {
            Ok(log) => {
                // a broken run's credit: the items its furnaces smelted, each furnace's counted up
                // to its fair share (all into one furnace earns no more than a 1/N share), and a
                // furnace's share again for each result delivered at all
                let share = sc.total().div_ceil(sc.furnaces.max(1) as u64);
                let spread: u64 = log.smelted.iter().map(|(_, n)| (*n).min(share)).sum();
                let k = (spread * 2 + log.delivered()).min(999) as i64;
                (judge(&sc, &log), k)
            }
            Err(why) => (Outcome::fail(why, Vec::new()), 0),
        };
        if !out.pass {
            let why = if r == Run::Base { out.why } else { format!("{}: {}", r.title(), out.why) };
            return (passed * 1000 + k, Outcome::fail(why, out.table));
        }
        passed += 1;
        match r {
            Run::Base => base = Some(out),
            Run::Tiled(row) => {
                let m = out.metrics.iter().find(|(k, _)| k == "items/min per furnace").map(|x| x.1.clone()).unwrap_or_default();
                row_rate = format!("{row_rate}{}{row}: {m}", if row_rate.is_empty() { "" } else { ", " });
            }
            Run::Intrusion | Run::Chest => {}
        }
    }
    let mut base = base.unwrap_or_else(|| Outcome::fail("no base run", Vec::new()));
    for (k, v) in per_furnace(c, *n) {
        base.metrics.push((k, v));
    }
    if let Some(t) = c.tile {
        base.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
        base.metrics.push(("rows items/min per furnace".to_string(), row_rate));
    }
    (passed * 1000, base)
}

/// The exact gate: one run in the game and on redsim, every cell of the watched box (block state,
/// container contents, furnace timers, comparator outputs) compared every gt.
pub fn compare(pool: &Pool, c: &Circuit, margin: i32, run: Run) -> Result<std::result::Result<usize, String>> {
    compare_with(Some(pool), c, margin, run, std::path::Path::new(""), &crate::recorded::Mode::Live)
}

/// [`compare`] live, recording the game's frames, or replaying a recording (no game, no pool):
/// `mctas test`'s recorded smelter runs ([`crate::recorded`]).
pub fn compare_with(pool: Option<&Pool>, c: &Circuit, margin: i32, run: Run, design: &std::path::Path, mode: &crate::recorded::Mode) -> Result<std::result::Result<usize, String>> {
    let (sc, rc) = scenario_for(c, run)?;
    let sim = sim_run_rec(&rc, &sc, margin, true).map(|(_, f)| f);
    let game = || -> Result<std::result::Result<Vec<Frame>, String>> {
        let pool = pool.ok_or_else(|| anyhow!("the game run needs a pool"))?;
        let r = pool.map(vec![0u8], |w, _, _| {
            let (from, to) = crate::run::watch_box(&rc, margin);
            let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
            let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
            let r = run_in_rec(w, sb, &rc, &sc, from, to, true);
            let _ = w.drop_sandbox(sb);
            r
        });
        Ok(r.into_iter().next().ok_or_else(|| anyhow!("no run"))??.map(|(_, f)| f))
    };
    crate::recorded::gate("smelter-run", design, &run.name(), margin, sim, game, mode)
}
