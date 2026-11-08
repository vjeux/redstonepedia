//! Study 22, auto-composters (designs/circuits/tracks/composter/NOTES.md, docs/circuits.md
//! "composter N"): kind `composter N`, N composters fed by a hopper line: a farm's mixed output
//! from an input chest, every compostable item composted, the bone meal pulled out into a chest,
//! everything else passed on to an overflow chest without jamming (coordinator 2026-10-21, under
//! vjeux's standing bar "REAL USE IS THE BAR").
//!
//! A composter takes items only from a hopper or dropper above it, and only compostable ones
//! (`ComposterBlock.InputContainer`), so a hopper over a composter keeps every other item for good:
//! a design must sort the compostables out of the line first (the item sorter's filter, one kind a
//! slice). Composting is random: a layer with the item's chance (wheat seeds and kelp 30%, sugar
//! cane 50%, wheat 65%; the first layer always), drawn from the level random, which redsim mirrors
//! draw for draw (every build's sandbox is seeded from its watch box).
//!
//! Judged in the game by its own runner on the sorter's benches: the judge puts a chest on `items`
//! (the mixed load: wheat seeds, wheat, kelp and sugar cane, with cobblestone, iron ingots and iron
//! shovels between them, topped up as it empties), one on `pass` (the overflow) and one on every
//! `output` (the bone meal), empties those every 20 gt, then waits for rest. The runs ([`Run`]):
//! * `base` (4 positions x 4 rotations, the metrics): the design as built; a tileable slice (one
//!   kind a slice, `tile:`) is built as a row of 4 slices, the judge's four kinds in turn (copy k's
//!   filter holds kind k instead of the design's kind, as the sorter's storage rows).
//! * `tiled8` (a slice: a row of 8 slices, each kind twice).
//! * `intrusion` (required, 4 rotations): the base run with the sorter's strays dropped on every
//!   open hopper top at a quarter, half and three quarters of the run.
//!
//! Every run must: compost every compostable item (none in the overflow chest, none left in the
//! input chest or the machine), pass every other item to the overflow chest (strays: passed or
//! lying on the top they fell on), put every bone meal in a bone meal chest (none in the overflow,
//! no composter left full), use every composter, leave no item on the floor and come to rest.
//! Metrics: bone meal/h (per composter), items composted/min, first bone meal, hoppers awake, at
//! rest, blocks per composter.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack};
use crate::sorter::{Bench, Frame, GameBench, STRAYS, SimBench, bp, held, on_top, sim_held, top_box};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// The judge's compostable kinds (a farm's crops), in the order a row's slices get them.
pub const CROPS: [&str; 4] = ["minecraft:wheat_seeds", "minecraft:wheat", "minecraft:kelp", "minecraft:sugar_cane"];
/// The judge's other items (passed on to the overflow chest).
pub const JUNK: [&str; 3] = ["minecraft:cobblestone", "minecraft:iron_ingot", "minecraft:iron_shovel"];
pub const BONE_MEAL: &str = "minecraft:bone_meal";
/// The gt with nothing moving before a stray lying on a hopper top is taken as never taken in.
pub const QUIET: u32 = 1200;
/// A Minecraft hour in gt (bone meal/h).
pub const HOUR: f64 = 72000.0;

/// The judge's runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    /// A slice's row of 8 slices.
    Tiled8,
    Intrusion,
}

impl Run {
    pub fn name(self) -> &'static str {
        match self {
            Run::Base => "base",
            Run::Tiled8 => "tiled8",
            Run::Intrusion => "intrusion",
        }
    }

    pub fn parse(s: &str) -> Option<Run> {
        Some(match s {
            "base" => Run::Base,
            "tiled8" | "tiled" => Run::Tiled8,
            "intrusion" | "strays" => Run::Intrusion,
            _ => return None,
        })
    }

    pub fn title(self) -> String {
        match self {
            Run::Base => "the composter as built (a slice: a row of 4, one kind each)".into(),
            Run::Tiled8 => "a row of 8 slices, each kind twice (one input chest, one overflow chest)".into(),
            Run::Intrusion => "stray items dropped on every open hopper top".into(),
        }
    }
}

/// One run's inputs: the input chest's stacks in order, the strays.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    pub composters: u32,
    pub stream: Vec<Stack>,
    pub strays: Vec<(u32, Stack)>,
    pub cap: u32,
}

impl Scenario {
    /// A mixed load for `composters` composters (`share`: per composter, per kind group).
    pub fn new(run: Run, composters: u32, intrusion: bool) -> Scenario {
        let n = composters.max(1);
        let st = |id: &str, k: u32| Stack::new(id, k);
        let mut stream = Vec::new();
        // two rounds per composter: every kind in short runs, the other items between them
        for _ in 0..2 * n {
            stream.extend([
                st(CROPS[0], 16),
                st(JUNK[0], 10),
                st(CROPS[1], 10),
                st(CROPS[2], 10),
                st(JUNK[1], 5),
                st(CROPS[3], 10),
                st(JUNK[2], 1),
            ]);
        }
        let total: u32 = stream.iter().map(|s| s.count).sum();
        // the line's pace (8 gt an item) and room for the last bone meal
        let cap = total * 8 + 8000;
        let strays = if intrusion {
            let span = total * 8;
            STRAYS.iter().enumerate().map(|(k, (id, c))| (span * (k as u32 + 1) / 4, Stack::new(id, *c))).collect()
        } else {
            Vec::new()
        };
        Scenario { run, composters: n, stream, strays, cap }
    }

    pub fn supplied(&self, id: &str) -> u64 {
        self.stream.iter().filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }

    pub fn compostables(&self) -> u64 {
        CROPS.iter().map(|k| self.supplied(k)).sum()
    }
}

/// One run.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// What reached the overflow chest, per kind.
    pub passed: BTreeMap<String, u64>,
    /// What reached the bone meal chests, per kind.
    pub reached: BTreeMap<String, u64>,
    /// (gt, bone meal so far) at every change.
    pub flow: Vec<(u32, u64)>,
    /// (gt, items gone from the input chest so far) at every sample.
    pub fed: Vec<(u32, u64)>,
    pub end: u32,
    pub timed_out: bool,
    pub dropped: BTreeMap<String, u64>,
    pub lying: BTreeMap<String, u64>,
    pub left: BTreeMap<String, u64>,
    /// Per composter (position order): its level at the end and the bone meal its chest got.
    pub levels: Vec<(Pos, i32)>,
    pub per_chest: Vec<(Pos, u64)>,
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    pub loose: Vec<String>,
    pub rests: bool,
    pub awake: u32,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn bone_meal(&self) -> u64 {
        self.reached.get(BONE_MEAL).copied().unwrap_or(0)
    }
}

/// Where the judge's things go.
struct Ports {
    items: Pos,
    pass: Pos,
    outputs: Vec<Pos>,
    judge: Vec<Pos>,
    strays: Vec<Pos>,
}

fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let get = |role: &str| c.stores.iter().find(|s| s.role == role).map(|s| s.pos).ok_or_else(|| format!("composter: no `port {role}:`"));
    let (items, pass) = (get("items")?, get("pass")?);
    let outputs: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    if outputs.is_empty() {
        return Err("composter: no `port output:` (the bone meal chest)".into());
    }
    let mut judge = vec![items, pass];
    judge.extend(outputs.iter().copied());
    let mut blocked = crate::intrusion::device_cells(c);
    blocked.extend(judge.iter().copied());
    let strays = clock::intrusion::intakes(&c.clock.design, &blocked).into_iter().filter(|p| !judge.contains(p)).collect();
    Ok(Ports { items, pass, outputs, judge, strays })
}

/// The design's composters (position order).
pub fn composters(c: &Circuit) -> Vec<Pos> {
    c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:composter")).map(|(p, _)| *p).collect()
}

/// A compostable item of the judge's (what the design may hold of its own only in its filters).
fn crop(id: &str) -> bool {
    CROPS.contains(&id)
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

/// The run's loop: the input chest filled, then every [`SEG`] gt the overflow and bone meal chests
/// counted and emptied, the input chest topped up; strays when due. Stops once everything is
/// supplied and nothing has moved for [`QUIET`] gt (every stray accounted for), or at the cap.
fn drive<B: Bench>(b: &mut B, c: &Circuit, sc: &Scenario, p: &Ports, log: &mut RunLog) -> Result<()> {
    let at = composters(c);
    let mut cursor = 0usize;
    topup(b, p.items, &sc.stream, &mut cursor)?;
    let total: u64 = sc.stream.iter().map(|s| s.count as u64).sum();
    let mut per: BTreeMap<Pos, u64> = p.outputs.iter().map(|q| (*q, 0)).collect();
    let (mut t, mut quiet, mut strays) = (0u32, 0u32, 0usize);
    let mut last_in = u64::MAX;
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
        let mut moved = false;
        let have = b.read(p.pass)?;
        if !have.is_empty() {
            for s in &have {
                *log.passed.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(p.pass, &[])?;
            moved = true;
        }
        for q in &p.outputs {
            let have = b.read(*q)?;
            if !have.is_empty() {
                for s in &have {
                    *log.reached.entry(s.id.clone()).or_insert(0) += s.count as u64;
                    if s.id == BONE_MEAL {
                        *per.entry(*q).or_insert(0) += s.count as u64;
                    }
                }
                b.write(*q, &[])?;
                moved = true;
                log.flow.push((t, log.bone_meal()));
            }
        }
        // the input chest: what is still in it (supplied so far minus what it holds)
        let in_chest: u64 = b.read(p.items)?.iter().map(|s| s.count as u64).sum();
        let supplied: u64 = sc.stream[..cursor].iter().map(|s| s.count as u64).sum();
        let gone = supplied - in_chest.min(supplied);
        if in_chest != last_in {
            moved = true;
            last_in = in_chest;
        }
        log.fed.push((t, gone));
        if moved {
            quiet = 0;
        } else {
            quiet += SEG;
        }
        topup(b, p.items, &sc.stream, &mut cursor)?;
        let all = cursor >= sc.stream.len() && gone >= total && strays >= sc.strays.len();
        let mut lying = 0;
        for q in &p.strays {
            let (lo, hi) = top_box(*q);
            lying += b.items_in(lo, hi)?.iter().map(|x| x.1).sum::<u64>();
        }
        let done = all && quiet >= QUIET.max(if lying > 0 { QUIET } else { 0 });
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
    for s in b.read(p.items)? {
        *log.left.entry(s.id.clone()).or_insert(0) += s.count as u64;
    }
    log.levels = at.iter().copied().zip(b.levels(&at)?).collect();
    log.per_chest = per.into_iter().collect();
    log.end = t;
    Ok(())
}

/// Top the input chest up from the load (27 slots).
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

/// After the run: rest, the design's containers, the entities left.
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
    log.awake = b.0.iter().filter(|(q, s)| c.clock.design.blocks.contains_key(q) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
    log.final_held = held(w, sb, from, to, &p.judge)?;
    // the levels once at rest (a composter at 7 turns 8 a second later)
    let at = composters(c);
    log.levels = at.iter().map(|q| (*q, b.0.get(*q).map(|s| level_of(s)).unwrap_or(0))).collect();
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

/// A composter state's level.
pub fn level_of(state: &str) -> i32 {
    state.split("level=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).unwrap_or(0)
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
    // the sandbox's level random (composting draws from it), seeded as the game's
    let seed = shulker::build_seed(from, to, 0);
    w.mirror_level_random(seed, [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)]);
    w.drop_seed = Some(seed);
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
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    log.awake = c.clock.design.blocks.keys().filter(|q| matches!(w.get(bp(**q)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sim_held(&w, &p.judge);
    log.levels = composters(c).into_iter().map(|q| (q, if let redsim::Block::Composter { level } = w.get(bp(q)) { level as i32 } else { 0 })).collect();
    log.loose = w.entities.list.iter().filter(|e| !e.removed && !on_top(&p.strays, [e.pos.x, e.pos.y, e.pos.z])).map(|e| format!("{:?}", e.kind)).collect();
    if let Some((q, bl)) = w.drops.first() {
        log.loose.push(format!("{bl} at {q} broke with a drop"));
    }
    Ok((log, frames))
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(sc: &Scenario, log: &RunLog) -> Outcome {
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let mut table: Vec<String> = vec![format!("bone meal chests: {}", l(&log.reached)), format!("overflow chest: {}", l(&log.passed))];
    table.push(format!(
        "supplied {} (compostable {}, other {}); {} bone meal, end t={}",
        sc.stream.iter().map(|s| s.count as u64).sum::<u64>(),
        sc.compostables(),
        JUNK.iter().map(|k| sc.supplied(k)).sum::<u64>(),
        log.bone_meal(),
        log.end
    ));
    table.push(format!("composter levels at the end: {}", log.levels.iter().map(|(p, v)| format!("{p:?} {v}")).collect::<Vec<_>>().join(", ")));
    table.push(format!("bone meal per chest: {}", log.per_chest.iter().map(|(p, n)| format!("{p:?} {n}")).collect::<Vec<_>>().join(", ")));
    if !log.left.is_empty() {
        table.push(format!("left in the input chest: {}", l(&log.left)));
    }
    if !log.dropped.is_empty() {
        table.push(format!("strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), if log.lying.is_empty() { "none".to_string() } else { l(&log.lying) }));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    if let Some((k, n)) = log.initial.iter().find(|(k, _)| k.as_str() == BONE_MEAL) {
        return fail(format!("the design holds {n} {} of its own", clock::short(k)), table);
    }
    if log.timed_out {
        let fed = log.fed.last().map(|x| x.1).unwrap_or(0);
        return fail(format!("{fed} of {} items left the input chest after {} gt: it jams", sc.stream.iter().map(|s| s.count as u64).sum::<u64>(), log.end), table);
    }
    for (k, n) in &log.left {
        return fail(format!("{} x{n} never left the input chest", clock::short(k)), table);
    }
    // the overflow: everything but the crops and the bone meal
    for k in CROPS {
        if let Some(n) = log.passed.get(k) {
            return fail(format!("{} x{n} reached the overflow chest (a compostable item not composted)", clock::short(k)), table);
        }
    }
    if let Some(n) = log.passed.get(BONE_MEAL) {
        return fail(format!("bone meal x{n} reached the overflow chest (not collected)"), table);
    }
    // the bone meal chests: bone meal only (a stray passed through is allowed)
    for (k, n) in &log.reached {
        if k != BONE_MEAL && log.dropped.get(k).copied().unwrap_or(0) < *n {
            return fail(format!("{} x{n} reached a bone meal chest", clock::short(k)), table);
        }
    }
    // every other item passed on (strays: passed, reached, or lying where they fell)
    for k in JUNK {
        let supplied = sc.supplied(k) + log.dropped.get(k).copied().unwrap_or(0);
        let got = log.passed.get(k).copied().unwrap_or(0) + log.reached.get(k).copied().unwrap_or(0) + log.lying.get(k).copied().unwrap_or(0);
        if got != supplied {
            return fail(format!("{got} of {supplied} {} reached the overflow chest (stuck in the machine)", clock::short(k)), table);
        }
    }
    for (k, d) in &log.dropped {
        if JUNK.contains(&k.as_str()) {
            continue;
        }
        let got = log.passed.get(k).copied().unwrap_or(0) + log.reached.get(k).copied().unwrap_or(0) + log.lying.get(k).copied().unwrap_or(0);
        if got != *d {
            return fail(format!("{got} of {d} stray {} accounted for (stuck in the machine)", clock::short(k)), table);
        }
    }
    // nothing left in the machine but its own items, unchanged (its filters' stacks)
    for (k, n) in &log.final_held {
        let was = log.initial.get(k).copied().unwrap_or(0);
        if *n != was {
            let what = if crop(k) { " (a compostable item not composted)" } else if k == BONE_MEAL { " (bone meal not collected)" } else { "" };
            return fail(format!("{} x{} left in the machine{what}", clock::short(k), n.abs_diff(was)), table);
        }
    }
    if let Some((p, _)) = log.levels.iter().find(|(_, v)| *v >= 7) {
        return fail(format!("the composter at {p:?} ends full (its bone meal never collected)"), table);
    }
    if log.bone_meal() == 0 {
        return fail("no bone meal".to_string(), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    let bm = log.bone_meal();
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let last = log.flow.last().map(|x| x.0).unwrap_or(0);
    let per_h = if last > 0 { bm as f64 * HOUR / last as f64 / sc.composters.max(1) as f64 } else { 0.0 };
    // items composted a minute: the input chest's 10% to 90% (the line's pace)
    let total = sc.stream.iter().map(|s| s.count as u64).sum::<u64>();
    let at = |q: u64| log.fed.iter().find(|(_, m)| *m >= q).copied();
    let rate = match (at(total.div_ceil(10)), at(total * 9 / 10)) {
        (Some((ta, qa)), Some((tb, qb))) if tb > ta => (qb - qa) as f64 * 1200.0 / (tb - ta) as f64 * sc.compostables() as f64 / total as f64,
        _ => 0.0,
    };
    let metrics = vec![
        ("bone meal/h per composter".to_string(), format!("{per_h:.0}")),
        ("items composted/min".to_string(), format!("{rate:.1}")),
        ("bone meal".to_string(), format!("{bm} from {} items", sc.compostables())),
        ("first bone meal".to_string(), format!("by {first} gt")),
        ("hoppers awake".to_string(), log.awake.to_string()),
        ("at rest".to_string(), if log.rests { "yes".to_string() } else { "no, a clock runs on".to_string() }),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// A slice's filter kind: the first of the judge's crops its block entities hold.
fn slice_kind(c: &Circuit) -> Option<&'static str> {
    CROPS.iter().copied().find(|k| c.clock.design.block_entities.values().any(|s| s.contains(&format!("\"{k}\""))))
}

/// A slice's row of `copies` slices, the judge's crops in turn (the design's kind first).
pub fn row(c: &Circuit, copies: usize) -> Result<Circuit> {
    let own = slice_kind(c).ok_or_else(|| anyhow!("{}: a slice's filter must hold one of the judge's crops ({})", c.name(), CROPS.join(", ")))?;
    let start = CROPS.iter().position(|k| *k == own).unwrap_or(0);
    let kinds: Vec<String> = (0..copies).map(|k| CROPS[(start + k) % CROPS.len()].to_string()).collect();
    crate::sorter::tiled(c, &kinds)
}

/// Blocks per composter.
pub fn per_composter(c: &Circuit, n: u32) -> Vec<(String, String)> {
    vec![("blocks per composter".to_string(), format!("{:.1}", c.blocks() as f64 / n.max(1) as f64))]
}

/// The scenario and the circuit (a slice's row) of one run.
pub fn scenario_for(c: &Circuit, run: Run) -> Result<(Scenario, Circuit)> {
    let Some(Kind::Composter(n)) = &c.kind else { bail!("{}: not a composter", c.name()) };
    let copies = match (c.tile.is_some(), run) {
        (false, Run::Tiled8) => bail!("{}: no `tile:` header (a fixed composter is judged as built)", c.name()),
        (false, _) => 0,
        (true, Run::Tiled8) => 8usize.div_ceil(*n as usize),
        (true, _) => 4usize.div_ceil(*n as usize),
    };
    let rc = if copies > 0 { row(c, copies)? } else { c.clone() };
    let m = if copies > 0 { *n * copies as u32 } else { *n };
    Ok((Scenario::new(run, m, run == Run::Intrusion), rc))
}

/// The runs a design must pass besides the base run.
fn required(c: &Circuit) -> Vec<Run> {
    let mut v: Vec<Run> = if c.tile.is_some() { vec![Run::Tiled8] } else { Vec::new() };
    v.push(Run::Intrusion);
    v
}

/// The judge (`mctas circuit verify`): the base run at N positions x 4 rotations, then the
/// required runs at 4 rotations each.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Composter(n) = kind else { bail!("{}: not a composter", kind.name()) };
    let (sc, bc) = scenario_for(c, Run::Base)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = bc.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&sc, &log), dims: shulker::design_dims(c), family: family.clone() }))
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
    for run in required(c) {
        match run_rotations(pool, c, run, o) {
            Ok(_) => lines.push(format!("{}: passes (4 rotations)", run.title())),
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
        for (k2, v) in per_composter(c, n) {
            k.outcome.metrics.push((k2, v));
        }
        if let Some(t) = c.tile {
            k.outcome.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
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

/// A design aid (`mctas circuit composter-run`): one run at rotation 0 in the game.
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

/// The whole judge on redsim at rotation 0 (`composter-run --sim --run all`): the base run's
/// outcome, failing with the first required run that fails; plus a score (runs passed x 1000 +
/// the bone meal of the first failing run).
pub fn sim_score(c: &Circuit) -> (i64, Outcome) {
    let Some(Kind::Composter(n)) = &c.kind else { return (0, Outcome::fail("not a composter".to_string(), Vec::new())) };
    let mut passed = 0i64;
    let mut base: Option<Outcome> = None;
    let mut runs = vec![Run::Base];
    runs.extend(required(c));
    for r in runs {
        let (sc, rc) = match scenario_for(c, r) {
            Ok(x) => x,
            Err(e) => return (passed * 1000, Outcome::fail(format!("{e:#}"), Vec::new())),
        };
        let (out, k) = match sim_run(&rc, &sc, 3) {
            Ok(log) => {
                let k = log.bone_meal().min(999) as i64;
                (judge(&sc, &log), k)
            }
            Err(why) => (Outcome::fail(why, Vec::new()), 0),
        };
        if !out.pass {
            let why = if r == Run::Base { out.why } else { format!("{}: {}", r.title(), out.why) };
            return (passed * 1000 + k, Outcome::fail(why, out.table));
        }
        passed += 1;
        if r == Run::Base {
            base = Some(out);
        }
    }
    let mut base = base.unwrap_or_else(|| Outcome::fail("no base run", Vec::new()));
    for (k, v) in per_composter(c, *n) {
        base.metrics.push((k, v));
    }
    if let Some(t) = c.tile {
        base.metrics.push(("tileable".to_string(), format!("every {}", t.step.iter().map(|v| v.abs()).sum::<i32>())));
    }
    (passed * 1000, base)
}

/// The exact gate: one run in the game and on redsim, every cell of the watched box (block state,
/// container contents) compared every gt.
pub fn compare(pool: &Pool, c: &Circuit, margin: i32, run: Run) -> Result<std::result::Result<usize, String>> {
    compare_with(Some(pool), c, margin, run, std::path::Path::new(""), &crate::recorded::Mode::Live)
}

/// [`compare`] live, recording the game's frames, or replaying a recording (no game, no pool):
/// `mctas test`'s recorded composter runs ([`crate::recorded`]).
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
    crate::recorded::gate("composter-run", design, &run.name(), margin, sim, game, mode)
}
