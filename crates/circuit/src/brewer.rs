//! Study 21, auto-brewers (designs/circuits/tracks/brewer/NOTES.md, docs/circuits.md
//! "brewer N"): kind `brewer N`, N brewing stands fed by hoppers: water bottles, blaze powder and
//! the ingredient of each stage come from their chests, every finished potion goes into one output
//! chest, with no manual step (coordinator 2026-10-21, under vjeux's standing bar "REAL USE IS THE
//! BAR").
//!
//! The recipe is a two-stage one: water bottle + nether wart -> awkward potion, awkward + sugar ->
//! potion of swiftness. Sugar on a water bottle makes a mundane potion (the wrong stage: the batch is
//! lost), so a design must put the wart in first, then the sugar, one of each per three bottles.
//!
//! Judged in the game by its own runner, like the smelter ([`crate::smelter`], whose benches it
//! shares): the judge puts a chest on `bottles` (water bottles), `fuel` (blaze powder), `wart`
//! (nether wart), `sugar` (sugar) and `output`, fills them for B batches a stand (3 bottles, 1 wart,
//! 1 sugar each; blaze powder: one a stand and one more per 20 brews), empties the output chest every
//! 20 gt, samples every stand's brew timer, then waits for rest and takes an inventory. The runs
//! ([`Run`]):
//! * `base` (4 positions x 4 rotations, the metrics): the design as built.
//! * `tiled`, `tiled8` (required for a `tile:` slice, 4 rotations): rows of 4 and 8 stands (copies
//!   at the step; `tile slice:` end pieces as the smelter's), one set of supply chests, one output.
//! * `intrusion` (required, 4 rotations): the run (the row of 4 for a slice) with stray items dropped
//!   at rest on every open hopper top at a quarter, half and three quarters of the run: 1
//!   cobblestone, 64 iron ingots, 1 iron shovel (none of them goes into a brewing stand: one that
//!   falls into a hopper of the machine stays there for good).
//!
//! Every run must: turn every water bottle into a potion of swiftness in the output chest (no
//! water, awkward or mundane potion there: unfinished or brewed in the wrong order), use up every
//! wart and every sugar (one each per three bottles: a brew with fewer bottles wastes them), burn no
//! more blaze powder than the stands' brews need (`stands + brews / 20`), use every stand (each at
//! least two thirds of an even share of the brews), leave nothing in the machine but blaze powder
//! (the design brings no brewing item of its own), and end with every stand idle.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack, stacks_of_be};
use crate::sorter::{Bench, Frame, GameBench, STRAYS, SimBench, bp, held, on_top, sim_held, top_box};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// The judge's supplies and the potion it wants.
pub const WATER: &str = "minecraft:potion[water]";
pub const WART: &str = "minecraft:nether_wart";
pub const SUGAR: &str = "minecraft:sugar";
pub const FUEL: &str = "minecraft:blaze_powder";
pub const GOAL: &str = "minecraft:potion[swiftness]";
/// Brews a blaze powder gives (`brewing/uses_default`).
pub const USES: u64 = 20;
/// Batches a stand in the base run, and in the rows.
pub const BATCHES: u32 = 4;
pub const ROW_BATCHES: u32 = 2;
/// The rows a tileable slice is judged as (stands in all; coordinator 2026-10-21: 2, 4 and 8 stands).
pub const ROWS: [u32; 3] = [2, 4, 8];
/// The gt with nothing delivered before a stray lying on a hopper top is taken as never taken in.
pub const QUIET: u32 = 1200;

/// `brewer-run --trace`: every stand's contents, the hoppers above and under it and its brew timer
/// every sample of the first 4000 gt, on stderr (what a design does between the judge's checks).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The judge's runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    /// A row of slices side by side with this many stands in all ([`ROWS`]).
    Tiled(u32),
    Intrusion,
}

impl Run {
    pub fn name(self) -> String {
        match self {
            Run::Base => "base".into(),
            Run::Tiled(4) => "tiled".into(),
            Run::Tiled(n) => format!("tiled{n}"),
            Run::Intrusion => "intrusion".into(),
        }
    }

    pub fn parse(s: &str) -> Option<Run> {
        Some(match s {
            "base" => Run::Base,
            "tiled" | "tileable" | "tiled4" => Run::Tiled(4),
            "tiled2" => Run::Tiled(2),
            "tiled8" => Run::Tiled(8),
            "intrusion" | "strays" => Run::Intrusion,
            _ => return None,
        })
    }

    pub fn title(self) -> String {
        match self {
            Run::Base => "the brewer as built".into(),
            Run::Tiled(n) => format!("slices side by side, {n} brewing stands in all (one set of supply chests, one output chest)"),
            Run::Intrusion => "stray items dropped on every open hopper top".into(),
        }
    }
}

/// Copies of a slice of `n` stands for a row of `row` stands (at least 2).
pub fn copies_for(n: u32, row: u32) -> usize {
    (row.div_ceil(n.max(1)) as usize).max(2)
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    pub stands: u32,
    pub batches: u32,
    pub bottles: Vec<Stack>,
    pub fuel: Vec<Stack>,
    pub wart: Vec<Stack>,
    pub sugar: Vec<Stack>,
    pub strays: Vec<(u32, Stack)>,
    pub cap: u32,
}

/// `n` of an item in stacks of its max.
fn stacks(id: &str, n: u32) -> Vec<Stack> {
    let max = redsim::redstone::container::max_stack_of(id) as u32;
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
    /// A run over `stands` stands, `batches` batches each.
    pub fn new(run: Run, stands: u32, batches: u32, intrusion: bool) -> Scenario {
        let n = stands.max(1);
        let b = n * batches;
        let brews = 2 * b as u64;
        // the fuel chest as the smelter's coal: two a stand and one per 20 brews, at least a stack
        // (a design may keep a few in its hoppers; what it burns is checked against the brews)
        let fuel = (2 * n as u64 + brews / USES).max(64);
        // two 400 gt brews a batch, the batches of one stand one after the other, + settling
        let cap = batches * 2000 + 6000;
        let strays = if intrusion {
            let span = batches * 900;
            STRAYS.iter().enumerate().map(|(k, (id, c))| (span * (k as u32 + 1) / 4, Stack::new(id, *c))).collect()
        } else {
            Vec::new()
        };
        Scenario { run, stands: n, batches, bottles: stacks(WATER, 3 * b), fuel: stacks(FUEL, fuel as u32), wart: stacks(WART, b), sugar: stacks(SUGAR, b), strays, cap }
    }

    /// Potions wanted.
    pub fn total(&self) -> u64 {
        3 * (self.stands * self.batches) as u64
    }

    pub fn fuel_supplied(&self) -> u64 {
        self.fuel.iter().map(|s| s.count as u64).sum()
    }
}

/// One run.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// What reached the output chest, per kind.
    pub reached: BTreeMap<String, u64>,
    /// (gt, potions of the goal delivered so far) at every change.
    pub flow: Vec<(u32, u64)>,
    pub end: u32,
    pub timed_out: bool,
    pub dropped: BTreeMap<String, u64>,
    pub lying: BTreeMap<String, u64>,
    /// Left in the judge's supply chests at the end, per kind.
    pub left: BTreeMap<String, u64>,
    /// Per stand (position order): the brews it started (its timer seen going up).
    pub brews: Vec<(Pos, u64)>,
    /// Per stand: its contents at the end (the table).
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    pub loose: Vec<String>,
    pub restless: String,
    pub rests: bool,
    pub awake: u32,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn goal(&self) -> u64 {
        self.reached.get(GOAL).copied().unwrap_or(0)
    }
}

/// Where the judge's things go.
struct Ports {
    bottles: Pos,
    fuel: Pos,
    wart: Pos,
    sugar: Pos,
    output: Pos,
    judge: Vec<Pos>,
    strays: Vec<Pos>,
}

fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let get = |role: &str| c.stores.iter().find(|s| s.role == role).map(|s| s.pos).ok_or_else(|| format!("brewer: no `port {role}:`"));
    let (bottles, fuel, wart, sugar) = (get("bottles")?, get("fuel")?, get("wart")?, get("sugar")?);
    let output = c.outputs.first().map(|o| o.pos).ok_or_else(|| "brewer: no `port output:`".to_string())?;
    let judge = vec![bottles, fuel, wart, sugar, output];
    let mut blocked = crate::intrusion::device_cells(c);
    blocked.extend(judge.iter().copied());
    let strays = clock::intrusion::intakes(&c.clock.design, &blocked).into_iter().filter(|p| !judge.contains(p)).collect();
    Ok(Ports { bottles, fuel, wart, sugar, output, judge, strays })
}

/// The design's brewing stands (position order).
fn stands(c: &Circuit) -> Vec<Pos> {
    c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:brewing_stand")).map(|(p, _)| *p).collect()
}

/// A brewing item (what the design may not hold of its own).
fn brewing_item(id: &str) -> bool {
    id.starts_with("minecraft:potion") || [WART, SUGAR, FUEL, "minecraft:glass_bottle"].contains(&id)
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
    let chests: Vec<(Pos, String)> = p.judge.iter().map(|q| (*q, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    let mut b = GameBench { w, sb, from, to, base: None, frames: record.then(Vec::new) };
    b.record()?;
    b.step(c.clock.free_from() + SETTLE)?;
    b.base = Some(b.w.get_region(sb, from, to)?);
    let mut log = RunLog { dims: shulker::design_dims(c), initial: held(b.w, sb, from, to, &p.judge)?, ..Default::default() };
    drive(&mut b, c, sc, &p, &mut log)?;
    let frames = b.frames.take().unwrap_or_default();
    finish(w, sb, c, from, to, &p, &mut log)?;
    Ok(Ok((log, frames)))
}

/// The run's loop: the chests filled, then every [`SEG`] gt the output counted and emptied, the
/// chests topped up, the stands' brew timers sampled (a brew starts when a timer goes up); strays
/// when due. Stops when every potion is delivered (and every stray accounted for), or at the cap.
fn drive<B: Bench>(b: &mut B, c: &Circuit, sc: &Scenario, p: &Ports, log: &mut RunLog) -> Result<()> {
    let at = stands(c);
    let mut cursors = [0usize; 4];
    let lists: [(&Pos, &Vec<Stack>); 4] = [(&p.bottles, &sc.bottles), (&p.fuel, &sc.fuel), (&p.wart, &sc.wart), (&p.sugar, &sc.sugar)];
    for (k, (q, l)) in lists.iter().enumerate() {
        topup(b, **q, l, &mut cursors[k])?;
    }
    log.brews = at.iter().map(|q| (*q, 0)).collect();
    let mut last = b.brew_times(&at)?;
    let (mut t, mut quiet, mut strays) = (0u32, 0u32, 0usize);
    loop {
        b.step(SEG)?;
        t += SEG;
        b.note(t, &mut log.notes)?;
        while strays < sc.strays.len() && t >= sc.strays[strays].0 {
            let s = &sc.strays[strays].1;
            for q in &p.strays {
                b.drop_item([q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], s)?;
                *log.dropped.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            log.notes.push(format!("t={t}: the judge drops {} x{} on each of {} open hopper top(s) {:?}", clock::short(&s.id), s.count, p.strays.len(), p.strays));
            strays += 1;
        }
        let now_t = b.brew_times(&at)?;
        if TRACE.load(std::sync::atomic::Ordering::Relaxed) && t <= 4000 {
            let show = |v: Vec<Stack>| v.iter().map(|s| format!("{}x{}", clock::short(&s.id), s.count)).collect::<Vec<_>>().join(" ");
            let mut line = format!("t={t} brew={now_t:?}");
            for q in at.iter() {
                line.push_str(&format!(" stand[{}]", show(b.read(*q)?)));
                for (dy, name) in [(1, "top"), (-1, "bottom")] {
                    if let Ok(v) = b.read([q[0], q[1] + dy, q[2]]) {
                        line.push_str(&format!(" {name}[{}]", show(v)));
                    }
                }
            }
            // every other hopper and dropper of the design holding a brewing item (not the clock's
            // stones)
            for (q, st) in &c.clock.design.blocks {
                let near_stand = at.iter().any(|s| s[0] == q[0] && s[2] == q[2] && (q[1] - s[1]).abs() <= 1);
                if near_stand || !(st.starts_with("minecraft:hopper") || st.starts_with("minecraft:dropper")) {
                    continue;
                }
                let v: Vec<Stack> = b.read(*q)?.into_iter().filter(|s| !s.id.ends_with(":stone")).collect();
                if !v.is_empty() {
                    line.push_str(&format!(" {q:?}[{}]", show(v)));
                }
            }
            eprintln!("{line}");
        }
        for (i, (x, y)) in last.iter().zip(&now_t).enumerate() {
            if y > x {
                log.brews[i].1 += 1;
            }
        }
        last = now_t;
        let before: u64 = log.reached.values().sum();
        let have = b.read(p.output)?;
        if !have.is_empty() {
            for s in &have {
                *log.reached.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(p.output, &[])?;
        }
        let now: u64 = log.reached.values().sum();
        if now != before {
            log.flow.push((t, log.goal()));
            quiet = 0;
        } else {
            quiet += SEG;
        }
        for (k, (q, l)) in lists.iter().enumerate() {
            topup(b, **q, l, &mut cursors[k])?;
        }
        let dropped: u64 = log.dropped.values().sum();
        let all = cursors.iter().zip(lists.iter()).all(|(c, (_, l))| *c >= l.len()) && strays >= sc.strays.len();
        let strays_done = if dropped > 0 {
            let reached_strays: u64 = log.dropped.keys().map(|k| log.reached.get(k).copied().unwrap_or(0)).sum();
            let mut lying = 0;
            for q in &p.strays {
                let (lo, hi) = top_box(*q);
                lying += b.items_in(lo, hi)?.iter().map(|x| x.1).sum::<u64>();
            }
            reached_strays + lying >= dropped && (lying == 0 || quiet >= QUIET)
        } else {
            true
        };
        let done = all && log.goal() >= sc.total() && strays_done;
        // a machine that stopped short: nothing delivered for a long while once all is supplied
        let stuck = all && quiet >= 4 * QUIET;
        if done || stuck || t >= sc.cap {
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
    for q in [p.bottles, p.fuel, p.wart, p.sugar] {
        for s in b.read(q)? {
            *log.left.entry(s.id.clone()).or_insert(0) += s.count as u64;
        }
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

/// After the run: rest (every stand idle), the design's containers, the entities left.
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
    log.rests = a == b && status.block_ticks + status.block_events + status.moving_pistons == 0;
    if log.rests {
        w.step(sb, 7)?;
        let c2 = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        log.rests = c2 == b;
        b = c2;
    }
    let st = stands(c);
    if let Some(s) = st.iter().find(|s| b.1.iter().any(|be| be.pos == **s && crate::sorter::int_field(&be.nbt, "BrewTime:") > 0)) {
        log.restless = format!("the brewing stand at {s:?} still brews {} gt after the last potion", 6 * REST);
    }
    log.awake = b.0.iter().filter(|(q, s)| c.clock.design.blocks.contains_key(q) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
    log.final_held = held(w, sb, from, to, &p.judge)?;
    for s in &st {
        if let Some(be) = b.1.iter().find(|be| be.pos == *s) {
            let items = stacks_of_be(&be.nbt);
            if !items.is_empty() {
                log.notes.push(format!("brewing stand at {s:?} holds {}", items.iter().map(|x| format!("{} x{}", clock::short(&x.id), x.count)).collect::<Vec<_>>().join(", ")));
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

pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<RunLog, String> {
    sim_run_rec(c, sc, margin, false).map(|(log, _)| log)
}

/// The redsim twin's frames of a run, every gt from placement (each non-air cell of the watch box
/// with its block entity's data), for the 3D view's check (`mctas web check`: the view replays the
/// base run).
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
    let chest = redsim::Block::parse(CHEST)?;
    for q in &p.judge {
        w.set_block(bp(*q), chest, redsim::world::flags::ALL);
    }
    let mut b = SimBench { w: &mut w, from, to, frames: record.then(Vec::new), coal: None, watch: None };
    b.record();
    b.step(c.clock.free_from() + SETTLE).map_err(|e| format!("{e:#}"))?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: sim_held(b.w, &p.judge), ..Default::default() };
    drive(&mut b, c, sc, &p, &mut log).map_err(|e| format!("{e:#}"))?;
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
    let st = stands(c);
    if let Some(s) = st.iter().find(|s| w.containers.map.get(&bp(**s)).is_some_and(|x| x.brew_time > 0)) {
        log.restless = format!("the brewing stand at {s:?} still brews {} gt after the last potion", 6 * REST);
    }
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    log.awake = c.clock.design.blocks.keys().filter(|q| matches!(w.get(bp(**q)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sim_held(&w, &p.judge);
    for s in &st {
        if let Some(x) = w.containers.map.get(&bp(*s)) {
            let items: Vec<String> = x.slots.iter().flatten().map(|i| format!("{} x{}", redsim::redstone::container::item_name(i.item), i.count)).collect();
            if !items.is_empty() {
                log.notes.push(format!("brewing stand at {s:?} holds {}", items.join(", ")));
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
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let mut table: Vec<String> = vec![format!("output: {}", l(&log.reached))];
    table.push(format!("supplied {} water bottles, {} wart, {} sugar, {} blaze powder; {} potions of swiftness delivered, end t={}", 3 * sc.stands * sc.batches, sc.stands * sc.batches, sc.stands * sc.batches, sc.fuel_supplied(), log.goal(), log.end));
    table.push(format!("brews per stand: {}", log.brews.iter().map(|(p, n)| format!("{p:?} {n}")).collect::<Vec<_>>().join(", ")));
    if !log.left.is_empty() {
        table.push(format!("left in the supply chests: {}", l(&log.left)));
    }
    if !log.dropped.is_empty() {
        table.push(format!("strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), if log.lying.is_empty() { "none".to_string() } else { l(&log.lying) }));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    // the output: potions of swiftness, and strays passed through; nothing else
    for (k, n) in &log.reached {
        if k == GOAL {
            continue;
        }
        let allowed = log.dropped.get(k).copied().unwrap_or(0);
        if *n > allowed {
            let what = match k.as_str() {
                WATER => "a water bottle never brewed",
                "minecraft:potion[awkward]" => "brewed with the wart only: taken out before the sugar",
                "minecraft:potion[mundane]" => "the sugar reached the water bottles before the wart: the wrong stage",
                WART | SUGAR | FUEL => "an ingredient passed through",
                _ => "not the potion wanted",
            };
            return fail(format!("{} x{} reached the output chest ({what})", clock::short(k), n - allowed), table);
        }
    }
    if let Some((k, n)) = log.initial.iter().find(|(k, _)| brewing_item(k)) {
        return fail(format!("the design holds {n} {} of its own (every brewing item comes from the judge's chests)", clock::short(k)), table);
    }
    if log.timed_out {
        return fail(format!("{} of {} potions of swiftness delivered after {} gt: it stops short", log.goal(), sc.total(), log.end), table);
    }
    if log.goal() != sc.total() {
        return fail(format!("{} potions of swiftness for {} water bottles", log.goal(), sc.total()), table);
    }
    for (k, n) in &log.left {
        if k != FUEL {
            return fail(format!("{} x{n} never left its chest", clock::short(k)), table);
        }
    }
    // strays: passed through, or lying on a top that cannot take them
    for (k, d) in &log.dropped {
        let got = log.reached.get(k).copied().unwrap_or(0) + log.lying.get(k).copied().unwrap_or(0);
        if got != *d {
            return fail(format!("{got} of {d} stray {} accounted for (stuck in the machine)", clock::short(k)), table);
        }
    }
    // nothing left in the machine but blaze powder (and the design's own items, unchanged)
    for (k, n) in &log.final_held {
        let was = log.initial.get(k).copied().unwrap_or(0);
        if k != FUEL && *n != was {
            return fail(format!("{} x{} left in the machine (only blaze powder may stay)", clock::short(k), n.saturating_sub(was)), table);
        }
    }
    // blaze powder: a stand takes one at its first brew and one more every 20 brews
    let kept = log.final_held.get(FUEL).copied().unwrap_or(0);
    let burnt = sc.fuel_supplied().saturating_sub(log.left.get(FUEL).copied().unwrap_or(0) + kept);
    let brews: u64 = log.brews.iter().map(|(_, n)| n).sum();
    let allowed = log.brews.len() as u64 + brews / USES;
    if burnt > allowed {
        return fail(format!("blaze powder wasted: {burnt} used for {brews} brews in {} stand(s) (at most {allowed})", log.brews.len()), table);
    }
    if brews != 2 * sc.total() / 3 {
        return fail(format!("{brews} brews for {} batches (two a batch: a brew stopped or started again)", sc.total() / 3), table);
    }
    if let Some((p, _)) = log.brews.iter().find(|(_, n)| *n == 0) {
        return fail(format!("the brewing stand at {p:?} never brewed (the batches are not spread across the {} stands)", log.brews.len()), table);
    }
    let even = brews / log.brews.len().max(1) as u64;
    if let Some((p, n)) = log.brews.iter().find(|(_, n)| 3 * *n < 2 * even) {
        return fail(format!("the brewing stand at {p:?} brewed {n} of {brews} times (an even share is {even}: the batches are not shared out across the {} stands)", log.brews.len()), table);
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
    let metrics = vec![
        ("potions/min per stand".to_string(), format!("{:.1}", rate / sc.stands.max(1) as f64)),
        ("first potion".to_string(), format!("by {first} gt")),
        ("blaze powder".to_string(), format!("{burnt} for {brews} brews")),
        ("hoppers awake".to_string(), log.awake.to_string()),
        ("at rest".to_string(), if log.rests { "yes".to_string() } else { "no, a clock runs on".to_string() }),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The tiled design: `copies` copies at the `tile:` step (with `tile slice:` end pieces, as the
/// smelter's); supply ports on copy 0 (or the start piece), the output past the last copy.
pub fn tiled(c: &Circuit, copies: usize) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let n = copies as i32;
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), n.to_string());
    d.ports.retain(|name, _| matches!(name.as_str(), "bottles" | "fuel" | "wart" | "sugar"));
    let axis = (0..3).find(|i| t.step[*i] != 0).unwrap_or(0);
    let span = crate::smelter::slice_span(c)?;
    let in_slice = |p: &Pos| span.is_none_or(|(lo, hi)| p[axis] >= lo && p[axis] <= hi);
    let after = |p: &Pos| span.is_some_and(|(_, hi)| p[axis] > hi);
    let last = [t.step[0] * (n - 1), t.step[1] * (n - 1), t.step[2] * (n - 1)];
    let add = |a: &Pos, b: &Pos| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    if span.is_some() {
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
        let before = span.is_some_and(|(lo, _)| p[axis] < lo);
        d.ports.insert("output".into(), if before { *p } else { add(p, &last) });
    }
    for (name, p) in &d.ports {
        if let Some(s) = d.blocks.get(p)
            && !(matches!(name.as_str(), "bottles" | "fuel" | "wart" | "sugar") && crate::smelter::is_cover(s))
        {
            bail!("tile: port {name} at {p:?} lands on a copy's block");
        }
    }
    Circuit::from_design(d).context("tiled brewer")
}

/// Blocks per stand (a row with end pieces: the repeated slice's, and the end pieces apart).
pub fn per_stand(c: &Circuit, n: u32) -> Vec<(String, String)> {
    let n = n.max(1) as f64;
    match (crate::smelter::slice_span(c).ok().flatten(), c.tile) {
        (Some((lo, hi)), Some(t)) => {
            let axis = (0..3).find(|i| t.step[*i] != 0).unwrap_or(0);
            let slice = c.clock.design.blocks.keys().filter(|p| p[axis] >= lo && p[axis] <= hi).count();
            let ends = c.blocks() as usize - slice;
            vec![("blocks per stand".to_string(), format!("{:.1}", slice as f64 / n)), ("end pieces".to_string(), format!("{ends} blocks"))]
        }
        _ => vec![("blocks per stand".to_string(), format!("{:.1}", c.blocks() as f64 / n))],
    }
}

/// The scenario and the circuit (the row for a tiled run) of one run.
pub fn scenario_for(c: &Circuit, run: Run) -> Result<(Scenario, Circuit)> {
    let Some(Kind::Brewer(n)) = &c.kind else { bail!("{}: not a brewer", c.name()) };
    match run {
        Run::Base => Ok((Scenario::new(run, *n, BATCHES, false), c.clone())),
        Run::Tiled(row) => {
            if c.tile.is_none() {
                bail!("{}: no `tile:` header (a fixed brewer is judged as built)", c.name());
            }
            let k = copies_for(*n, row);
            Ok((Scenario::new(run, *n * k as u32, ROW_BATCHES, false), tiled(c, k)?))
        }
        Run::Intrusion => {
            if c.tile.is_some() {
                let k = copies_for(*n, 4);
                Ok((Scenario::new(run, *n * k as u32, ROW_BATCHES, true), tiled(c, k)?))
            } else {
                Ok((Scenario::new(run, *n, BATCHES, true), c.clone()))
            }
        }
    }
}

/// The runs a design must pass besides the base run.
fn required(c: &Circuit) -> Vec<Run> {
    let mut v: Vec<Run> = if c.tile.is_some() { ROWS.iter().map(|r| Run::Tiled(*r)).collect() } else { Vec::new() };
    v.push(Run::Intrusion);
    v
}

/// The judge (`mctas circuit verify`): the base run at N positions x 4 rotations, then the
/// required runs at 4 rotations each.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Brewer(n) = kind else { bail!("{}: not a brewer", kind.name()) };
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
                    && let Some(m) = outs.first().and_then(|k| k.metrics.iter().find(|(k, _)| k == "potions/min per stand"))
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
        for (k2, v) in per_stand(c, n) {
            k.outcome.metrics.push((k2, v));
        }
        if let Some(t) = c.tile {
            k.outcome.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
            k.outcome.metrics.push(("rows potions/min per stand".to_string(), row_rate));
        }
        k.outcome.metrics.push(("bill".to_string(), bill));
        k.outcome.table.extend(lines);
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

/// A design aid (`mctas circuit brewer-run`): one run at rotation 0 in the game.
pub fn run_one(pool: &Pool, c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, &rc, &sc, margin));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(log) => Ok(judge(&sc, &log)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// [`run_one`] on redsim.
pub fn sim_one(c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, run)?;
    Ok(match sim_run(&rc, &sc, margin) {
        Ok(log) => judge(&sc, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The whole judge on redsim at rotation 0 (`brewer-run --sim --run all`): the base run's outcome,
/// failing with the first required run that fails; plus a score (runs passed x 1000 + potions of
/// the first failing run).
pub fn sim_score(c: &Circuit) -> (i64, Outcome) {
    let Some(Kind::Brewer(n)) = &c.kind else { return (0, Outcome::fail("not a brewer".to_string(), Vec::new())) };
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
                let k = log.goal().min(999) as i64;
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
                let m = out.metrics.iter().find(|(k, _)| k == "potions/min per stand").map(|x| x.1.clone()).unwrap_or_default();
                row_rate = format!("{row_rate}{}{row}: {m}", if row_rate.is_empty() { "" } else { ", " });
            }
            Run::Intrusion => {}
        }
    }
    let mut base = base.unwrap_or_else(|| Outcome::fail("no base run", Vec::new()));
    for (k, v) in per_stand(c, *n) {
        base.metrics.push((k, v));
    }
    if let Some(t) = c.tile {
        base.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
        base.metrics.push(("rows potions/min per stand".to_string(), row_rate));
    }
    (passed * 1000, base)
}

/// The exact gate: one run in the game and on redsim, every cell of the watched box (block state,
/// container contents, brew timers, comparator outputs) compared every gt.
pub fn compare(pool: &Pool, c: &Circuit, margin: i32, run: Run) -> Result<std::result::Result<usize, String>> {
    compare_with(Some(pool), c, margin, run, std::path::Path::new(""), &crate::recorded::Mode::Live)
}

/// [`compare`] live, recording the game's frames, or replaying a recording (no game, no pool):
/// `mctas test`'s recorded brewer runs ([`crate::recorded`]).
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
    crate::recorded::gate("brewer-run", design, &run.name(), margin, sim, game, mode)
}
