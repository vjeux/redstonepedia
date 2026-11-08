//! `mctas clock verify`: the official clock judge, in the real game (the JVM bridge the door judge
//! uses). The clock is built at N positions x 4 rotations (the game turns every block state), each
//! in a cleared sandbox, placed exactly like `clock sim` places it, kicked, and run to a long horizon
//! while every tick of its box (plus a margin) is recorded. Each run must be exactly periodic for
//! at least 3 periods after its warmup (block states compared cell by cell), create no entity (a
//! drop, a popped block) and never reach the edge of the watched box. Runs that agree on period,
//! output waveform and family make a `robust` clock; differences are reported as
//! position-dependent or rotation-dependent (allowed, flagged).

use crate::sim::{Output, Verdict};
use crate::wave::{self, CellWave, Interner, PeriodAnalysis};
use crate::{Action, Clock, Kick, Placement, Pos};
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use mctas::sim::{PlaceMode, Pool, Region, SandboxOptions, Worker, WorkerConfig};

/// Knobs of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Test positions (the door judge's offsets: its default four, then its extra ones).
    pub positions: usize,
    /// All 4 rotations (else only as built).
    pub rotations: bool,
    /// Ticks to run each build.
    pub horizon: u32,
    /// Watched cells around the clock's box (anything reaching the outer layer fails).
    pub margin: i32,
    /// Full periods required after the warmup.
    pub min_periods: usize,
    /// JVM workers (capped by MCTAS_MAX_WORKERS).
    pub workers: usize,
    pub version: String,
    /// Save, unload and load again the sandbox's chunks mid-run, after the warmup and 2 periods
    /// (coordinator 2026-10-20: a clock must survive a chunk reload): it must run at the same
    /// state period afterwards (a short glitch right after the reload is allowed).
    pub reload: bool,
    /// The tick of that reload (set by `verify_in_pool` from redsim's warmup and period).
    pub reload_at: Option<u32>,
    /// The state period the clock must keep across the reload (redsim's).
    pub reload_period: Option<u32>,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions { positions: 4, rotations: true, horizon: 4000, margin: 3, min_periods: 3, workers: 8, version: mctas::DEFAULT_VERSION.to_string(), reload: false, reload_at: None, reload_period: None }
    }
}

/// One build (a rotation at a position) that ran periodically.
#[derive(Clone, Debug)]
pub struct RunOk {
    pub warmup: u32,
    pub period: u32,
    pub analysis: PeriodAnalysis,
    pub output: Option<Output>,
    /// Width x height x depth of every cell non-air during the run (and the design's box).
    pub dims: [i32; 3],
    /// A clock holding a sculk sensor: whether ambient game events leave its output alone in this
    /// build ([`crate::vibration::game_property`]; a property, never a failure).
    pub vibration: Option<crate::vibration::Property>,
    /// The ITEMS property ([`items_property`], the base build only): `ok`, or what a dropped item
    /// changes; `None` without an open hopper top (or not the base build).
    pub items: Option<String>,
}

impl RunOk {
    /// What must agree between builds: period, output waveform (pulse widths up to a cyclic
    /// shift: where a period "starts" is arbitrary), family.
    fn signature(&self) -> String {
        let (on, edges, widths) = self.output.as_ref().map(|o| (o.wave.on, o.wave.edges, o.wave.widths.clone())).unwrap_or_default();
        let n = widths.len();
        let widths = (0..n.max(1)).map(|k| widths.iter().cycle().skip(k).take(n).copied().collect::<Vec<u32>>()).min().unwrap_or_default();
        format!("P={} on {on} edges {edges} pulses {widths:?} family {}", self.period, wave::signature(&self.analysis.family))
    }
}

#[derive(Clone, Debug)]
pub struct Run {
    pub turns: u8,
    pub offset: Pos,
    pub result: std::result::Result<RunOk, String>,
    /// With `--reload`: redsim's verdict on the same build's chunk reload
    /// ([`crate::sim::reload_check`]), which the game's run confirms.
    pub sim_reload: Option<std::result::Result<(), String>>,
}

/// Result of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyReport {
    pub name: String,
    pub blocks: usize,
    pub start: String,
    pub runs: Vec<Run>,
    /// `robust`, `position-dependent` or `rotation-dependent` (a clock), else why it fails.
    pub robustness: String,
    pub verdict: Verdict,
    /// The tick of the chunk reload (`VerifyOptions::reload`), when there was one.
    pub reload_at: Option<u32>,
}

impl VerifyReport {
    /// The reference build: as built at the first position, else the first periodic build.
    pub fn base(&self) -> Option<&RunOk> {
        self.runs.iter().find(|r| r.turns == 0 && r.offset == [0, 0, 0]).and_then(|r| r.result.as_ref().ok()).or_else(|| self.runs.iter().find_map(|r| r.result.as_ref().ok()))
    }

    /// `CLOCK: <name> period P gt (T), on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust`.
    pub fn summary(&self) -> String {
        match (self.verdict, self.base()) {
            (Verdict::Clock, Some(b)) => {
                let (on, edges) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
                let [w, h, d] = b.dims;
                format!(
                    "CLOCK: {} period {}, on {on}/{}, edges {edges}, warmup {}, family {}, {} blocks, {w}x{h}x{d}, {}",
                    self.name,
                    crate::gt_time(b.period),
                    b.period,
                    b.warmup,
                    wave::signature(&b.analysis.family),
                    self.blocks,
                    self.robustness
                )
            }
            (v, _) => format!("{}: {} {}", if v == Verdict::Clock { "FAIL" } else { v.word() }, self.name, self.robustness),
        }
    }

    /// With `--reload`: redsim's chunk-reload verdict for every build against the game's run
    /// (tickexact: the result comes from redsim, the game confirms it). `None` without a reload.
    pub fn reload_agreement(&self) -> Option<String> {
        let checked: Vec<&Run> = self.runs.iter().filter(|r| r.sim_reload.is_some()).collect();
        if checked.is_empty() {
            return None;
        }
        let game_survives = |r: &Run| !matches!(&r.result, Err(e) if e.contains("chunk reload"));
        let sim_survives = |r: &Run| matches!(&r.sim_reload, Some(Ok(())));
        let survive = checked.iter().filter(|r| sim_survives(r)).count();
        let differ: Vec<String> = checked
            .iter()
            .filter(|r| game_survives(r) != sim_survives(r))
            .map(|r| {
                let sim = match &r.sim_reload {
                    Some(Ok(())) => "survives".to_string(),
                    Some(Err(e)) => e.clone(),
                    None => String::new(),
                };
                let game = match &r.result {
                    Err(e) => e.clone(),
                    Ok(_) => "survives".into(),
                };
                format!("rotation {} at {:?}: redsim {sim}; the game {game}", r.turns, r.offset)
            })
            .collect();
        Some(if differ.is_empty() {
            format!("reload: redsim AGREES (the chunk reload: {survive} of {} builds survive, the game's runs the same)", checked.len())
        } else {
            format!("reload: redsim DISAGREES with the game on {} of {} builds: {}", differ.len(), checked.len(), differ.join(" | "))
        })
    }
}

/// The clock turned `turns` quarter turns clockwise (seen from above) around the origin; the game
/// turns every block state (and a temporary start block).
pub fn rotated(w: &mut Worker, c: &Clock, turns: u8) -> Result<Clock> {
    let turns = turns % 4;
    if turns == 0 {
        return Ok(c.clone());
    }
    let mut states: Vec<String> = c.design.blocks.values().cloned().collect();
    let kick_state = match &c.kick {
        Some(Kick::Place { state, .. }) => Some(state.clone()),
        _ => None,
    };
    if let Some(s) = &kick_state {
        states.push(s.clone());
    }
    let turned = w.transform_states(&states, turns, false)?;
    let mut d = c.design.clone();
    d.blocks = c.design.blocks.keys().zip(turned.iter()).map(|(p, s)| (crate::rotate_pos(*p, turns), s.clone())).collect();
    d.ports = c.design.ports.iter().map(|(k, p)| (k.clone(), crate::rotate_pos(*p, turns))).collect();
    d.block_entities = c.design.block_entities.iter().map(|(p, s)| (crate::rotate_pos(*p, turns), s.clone())).collect();
    // design entities (minecarts...): positions by the same map in continuous coordinates (a turn
    // takes cell (x, z) to (-z, x): (X, Z) -> (1 - Z, X), as the door judge turns them), their
    // data (Rotation) by the game, their Motion turned here
    if !c.design.entities.is_empty() {
        let nbts = w.transform_entities(&c.design.entities, turns, false)?;
        d.entities = c
            .design
            .entities
            .iter()
            .zip(nbts)
            .map(|(e, nbt)| {
                let mut p = e.pos;
                for _ in 0..turns {
                    p = [1.0 - p[2], p[1], p[0]];
                }
                let nbt = nbt.or_else(|| e.nbt.clone()).map(|s| crate::turn_motion(&s, turns));
                mctas::design::EntitySpec { kind: e.kind.clone(), pos: p, nbt }
            })
            .collect();
    }
    if let Some(Kick::Place { ticks, .. }) = &c.kick {
        let at = if c.kick_at > 0 { format!(" at {}", c.kick_at) } else { String::new() };
        d.meta.insert("port start".into(), format!("{} {ticks}{at}", turned.last().unwrap()));
    }
    Clock::from_design(d)
}

/// Place a clock like the judge (the kick's inputs are applied by [`apply`] at their ticks).
pub fn place(w: &mut Worker, sb: u32, c: &Clock) -> Result<()> {
    let mut blocks = c.design.block_list();
    // the judge's lamp in an air output cell (vjeux 2026-10-20)
    if let Some(op) = crate::sim::judge_lamp(c) {
        blocks.push((op, crate::sim::JUDGE_LAMP.to_string()));
    }
    let last = c.withheld();
    let kept: Vec<(Pos, String)> = blocks.iter().filter(|(p, _)| Some(*p) != last).cloned().collect();
    let merge = |w: &mut Worker| -> Result<()> {
        for (p, nbt) in &c.design.block_entities {
            if Some(*p) != last {
                w.merge_block_entity(sb, *p, nbt).with_context(|| format!("nbt: {} {} {} {nbt}", p[0], p[1], p[2]))?;
            }
        }
        Ok(())
    };
    match c.placement {
        Placement::Judge => {
            w.set_blocks(sb, &kept, PlaceMode::Raw)?;
            merge(w)?;
            let wires: Vec<Pos> = kept.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
            w.update(sb, &wires, "shape")?;
            let positions: Vec<Pos> = kept.iter().map(|(p, _)| *p).collect();
            w.update(sb, &positions, "self")?;
        }
        Placement::Player => {
            for b in &kept {
                w.set_blocks(sb, std::slice::from_ref(b), PlaceMode::Update)?;
            }
            merge(w)?;
        }
    }
    // the design's entities (minecarts...), once the blocks stand (as the door judge summons them)
    if !c.design.entities.is_empty() {
        w.summon(sb, &c.design.entities)?;
    }
    Ok(())
}

/// The end-of-run entity check: the design's own entities may stay (the same kinds and counts,
/// inside the watched box); anything else (a drop, a popped block, a lost or extra cart) fails.
fn entity_problem(c: &Clock, ents: &[mctas::sim::EntityInfo], from: Pos, to: Pos) -> Option<String> {
    let norm = |k: &str| if k.contains(':') { k.to_string() } else { format!("minecraft:{k}") };
    let mut want: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for e in &c.design.entities {
        *want.entry(norm(&e.kind)).or_default() += 1;
    }
    let mut have: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for e in ents {
        *have.entry(norm(&e.kind)).or_default() += 1;
    }
    if want != have {
        let list = |m: &std::collections::BTreeMap<String, usize>| if m.is_empty() { "none".to_string() } else { m.iter().map(|(k, n)| format!("{n} {}", short(k))).collect::<Vec<_>>().join(", ") };
        let first = ents.iter().find(|e| want.get(&norm(&e.kind)).copied().unwrap_or(0) < have[&norm(&e.kind)]);
        let at = first.map(|e| format!(" ({} at {:.1?})", short(&e.kind), e.pos)).unwrap_or_default();
        return Some(format!("entities at the end: {} (the design has {}): a drop, a popped block or a lost/extra entity{at}", list(&have), list(&want)));
    }
    ents.iter()
        .find(|e| (0..3).any(|i| e.pos[i] < from[i] as f64 || e.pos[i] >= to[i] as f64 + 1.0))
        .map(|e| format!("{} at {:.1?} left the watched box (margin cells included)", short(&e.kind), e.pos))
}

fn short(k: &str) -> &str {
    crate::short(k)
}

/// Apply one input of the start kick between ticks.
pub fn apply(w: &mut Worker, sb: u32, a: &Action) -> Result<()> {
    match a {
        Action::Place { pos, state, nbt } => {
            w.set_blocks(sb, &[(*pos, state.clone())], PlaceMode::Update)?;
            if let Some(n) = nbt {
                w.merge_block_entity(sb, *pos, n)?;
            }
        }
        Action::Use { pos } => {
            w.use_block(sb, *pos)?;
        }
        Action::UseEntity { pos } => {
            w.use_entity(sb, *pos)?;
        }
        Action::Clear { pos } => {
            w.set_blocks(sb, &[(*pos, "minecraft:air".to_string())], PlaceMode::Update)?;
        }
        Action::ShapeSet { pos, state } => {
            w.set_blocks(sb, &[(*pos, state.clone())], PlaceMode::Shape)?;
        }
        Action::Update { pos } => {
            w.update(sb, &[*pos], "neighbors")?;
        }
        Action::GameEvent { pos, event } => {
            w.game_event_at(sb, event, &[*pos], true)?;
        }
        Action::Drop { pos, item } => {
            let (kind, at, nbt) = Action::drop_entity(*pos, item);
            w.summon(sb, &[mctas::design::EntitySpec { kind, pos: at, nbt: Some(nbt) }])?;
        }
    }
    Ok(())
}

/// Build, kick and run one clock (already turned and moved) in a fresh sandbox.
pub fn run_one(w: &mut Worker, c: &Clock, o: &VerifyOptions) -> Result<std::result::Result<RunOk, String>> {
    run_one_with(w, c, o, &[])
}

/// [`run_one`] with extra actions between ticks (the ITEMS property's dropped items).
pub fn run_one_with(w: &mut Worker, c: &Clock, o: &VerifyOptions, extra: &[(u32, Action)]) -> Result<std::result::Result<RunOk, String>> {
    let (lo, hi) = c.bounds();
    let m = o.margin;
    let from = [lo[0] - m, lo[1] - m, lo[2] - m];
    let to = [hi[0] + m, hi[1] + m, hi[2] + m];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, c, o, from, to, extra);
    let _ = w.drop_sandbox(sb);
    r
}

/// The ITEMS property of a passing build (the real-use audit's item intrusion, 2026-10-21; a
/// property, never a failure, clockinfra after vjeux's policy: only failures in normal running
/// hide a clock): `None` without an open hopper top a stray item can reach
/// ([`crate::intrusion::intakes`]); else the build again with 1 cobblestone, 64 iron ingots and 1
/// iron shovel dropped at rest on every intake after the warmup, one period and a third apart:
/// `ok` when it runs at the same period with the same output, else what changed.
pub fn items_property(w: &mut Worker, c: &Clock, o: &VerifyOptions, base: &RunOk) -> Result<Option<String>> {
    let at = crate::intrusion::intakes(&c.design, &[]);
    if at.is_empty() {
        return Ok(None);
    }
    const DROPS: [&str; 3] = ["{id:\"minecraft:cobblestone\",count:1}", "{id:\"minecraft:iron_ingot\",count:64}", "{id:\"minecraft:iron_shovel\",count:1}"];
    let step = base.period + base.period / 3 + 1;
    let mut extra: Vec<(u32, Action)> = Vec::new();
    for (k, item) in DROPS.iter().enumerate() {
        let f = base.warmup + base.period + k as u32 * step;
        for (i, p) in at.iter().enumerate() {
            extra.push((f + 3 * i as u32, Action::Drop { pos: *p, item: item.to_string() }));
        }
    }
    let last = extra.iter().map(|(t, _)| *t).max().unwrap_or(0);
    let mut o2 = o.clone();
    o2.reload_at = None;
    o2.horizon = o.horizon.max(last + (o.min_periods as u32 + 2) * base.period + 100).min(120_000);
    let cells: Vec<String> = at.iter().map(|p| format!("{p:?}")).collect();
    Ok(Some(match run_one_with(w, c, &o2, &extra)? {
        Ok(r) if r.period == base.period && r.signature() == base.signature() => "ok".to_string(),
        Ok(r) if r.period != base.period => format!("a dropped item changes its period ({} -> {} gt; open hopper tops {})", base.period, r.period, cells.join(" ")),
        Ok(_) => format!("a dropped item changes its output (open hopper tops {})", cells.join(" ")),
        Err(why) => format!("a dropped item breaks it: {why} (open hopper tops {})", cells.join(" ")),
    }))
}

/// The recorded frames of one build, kept sparse: per frame the (cell index in the watched box,
/// state id) of its non-air cells (big flat clocks over long horizons ran the bridge and this
/// judge out of memory with whole-box frames: clk-sculk's 60x1x60 sensor rings).
struct Frames {
    int: Interner,
    ids: Vec<u64>,
    sparse: Vec<Vec<(u32, u32)>>,
    slo: Pos,
    shi: Pos,
    from: Pos,
    to: Pos,
    size: [i32; 3],
    /// The judge's lamp (not the design's: left out of the swept box).
    lamp: Option<Pos>,
}

impl Frames {
    fn index(&self, p: Pos) -> u32 {
        (((p[1] - self.from[1]) * self.size[2] + (p[2] - self.from[2])) * self.size[0] + (p[0] - self.from[0])) as u32
    }

    fn pos_of(&self, i: u32) -> Pos {
        let i = i as i32;
        let (sx, sz) = (self.size[0], self.size[2]);
        [self.from[0] + i % sx, self.from[1] + i / (sx * sz), self.from[2] + (i / sx) % sz]
    }

    /// Add frame `t` (or replace the last one: the state at t after inputs applied at t). An
    /// error: a block reached the edge of the watched box.
    fn ingest(&mut self, f: &Region, extras: &HashMap<Pos, String>, replace_last: bool, t: usize, margin: i32) -> Option<String> {
        let mut v: Vec<(u32, u32)> = Vec::new();
        for (p, s) in f.iter() {
            let extra = extras.get(&p).map(|e| e.as_str());
            let air = mctas::blockstate::is_air(s);
            if air && extra.is_none() {
                continue;
            }
            if !air {
                if (0..3).any(|i| p[i] == self.from[i] || p[i] == self.to[i]) {
                    return Some(format!("tick {t}: {} at {p:?} reached the edge of the watched box (margin {margin}): rerun with a larger --margin", crate::short(s)));
                }
                if Some(p) != self.lamp {
                    crate::grow(&mut self.slo, &mut self.shi, p);
                }
            }
            // the full cell state (block state + comparator strength + container contents)
            // decides the period, exactly like clock sim
            let id = self.int.state(&wave::with_extra(s, extra));
            v.push((self.index(p), id));
        }
        v.sort_unstable();
        let flat: Vec<u32> = v.iter().flat_map(|(a, b)| [*a, *b]).collect();
        let id = self.int.frame(&flat);
        if replace_last {
            *self.ids.last_mut().unwrap() = id;
            *self.sparse.last_mut().unwrap() = v;
        } else {
            self.ids.push(id);
            self.sparse.push(v);
        }
        None
    }
}

fn run_in(w: &mut Worker, sb: u32, c: &Clock, o: &VerifyOptions, from: Pos, to: Pos, extra: &[(u32, Action)]) -> Result<std::result::Result<RunOk, String>> {
    place(w, sb, c)?;
    let probes: Vec<Pos> = c.output.into_iter().collect();
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let (slo, shi) = c.bounds();
    let mut fr = Frames { int: Interner::default(), ids: Vec::new(), sparse: Vec::new(), slo, shi, from, to, size, lamp: crate::sim::judge_lamp(c) };
    // Frame t = the state after t ticks, with the kick's inputs due at t applied (frame 0: right
    // after placement and the inputs due at 0): exactly what `clock sim` records.
    let mut signals: Vec<Vec<u8>> = Vec::new();
    let extras_of = |infos: Vec<mctas::sim::BlockEntityInfo>| -> HashMap<Pos, String> {
        infos.into_iter().filter_map(|b| wave::be_extra_from_nbt(&b.kind, &b.nbt).map(|e| (b.pos, e))).collect()
    };
    // steps in segments: the bridge returns a segment's frames in one message, so at most ~400k
    // cells per message (a long horizon or a big box at once ran it out of memory)
    let volume = (size[0] * size[1] * size[2]).max(1) as u32;
    let seg = (400_000 / volume).clamp(10, 2000);
    let mut actions = c.actions();
    actions.extend(extra.iter().cloned());
    actions.sort_by_key(|(t, _)| *t);
    let mut t = 0u32;
    loop {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            apply(w, sb, &a)?;
        }
        // the state at t (after its inputs): replace the recorded frame when inputs were applied
        let f = w.get_region(sb, from, to)?;
        let sg = probe_now(w, sb, &probes)?;
        let ex = extras_of(w.block_entities(sb, from, to)?);
        let replace = fr.ids.len() as u32 == t + 1;
        if let Some(e) = fr.ingest(&f, &ex, replace, t as usize, o.margin) {
            return Ok(Err(e));
        }
        if replace {
            *signals.last_mut().unwrap() = sg;
        } else {
            signals.push(sg);
        }
        let next = actions.first().map(|(at, _)| *at).unwrap_or(o.horizon).min(o.horizon).min(t + seg);
        // stop at the reload tick
        let next = match o.reload_at {
            Some(r) if r > t => next.min(r),
            _ => next,
        };
        if next <= t {
            break;
        }
        let st = w.step_probe_be(sb, next - t, false, &probes, Some((from, to)), true)?;
        if st.block_entities.len() != st.frames.len() {
            return Err(anyhow!("block-entity frames ({}) do not match the block frames ({}): the bridge is older than this judge (rebuild it: mctas setup)", st.block_entities.len(), st.frames.len()));
        }
        for (k, (f, dump)) in st.frames.iter().zip(&st.block_entities).enumerate() {
            let ex = extras_of(mctas::sim::parse_block_entities(dump)?);
            if let Some(e) = fr.ingest(f, &ex, false, t as usize + 1 + k, o.margin) {
                return Ok(Err(e));
            }
        }
        signals.extend(st.signals);
        t = next;
        if o.reload_at == Some(t) {
            w.reload(sb)?;
        }
        if t >= o.horizon && actions.is_empty() {
            break;
        }
    }
    let ents = w.entities(sb)?;
    if let Some(why) = entity_problem(c, &ents, from, to) {
        return Ok(Err(why));
    }
    let (slo, shi) = (fr.slo, fr.shi);
    let end = c.free_from() as usize;
    let Some((w0, p)) = wave::detect_period(&fr.ids, end, o.min_periods) else {
        return Ok(Err(format!("not periodic for {} periods within {} gt{}", o.min_periods, o.horizon, if o.reload_at.is_some() { " (with a chunk reload)" } else { "" })));
    };
    // a chunk reload: the clock must keep working, i.e. run at the same state period afterwards; a
    // short glitch right after it (lost block events, a phase shift) is fine, as for a builder
    if let Some(r) = o.reload_at
        && (w0 + p) as u32 > r
    {
        let want = o.reload_period.unwrap_or(p as u32);
        if p as u32 != want || w0 as u32 > r + 2 * want + 40 {
            return Ok(Err(format!("does not survive a chunk reload at tick {r}: it runs at period {p} gt only from tick {w0} (period {want} gt before the reload)")));
        }
    }
    // designs with entities: the period must hold over at least the second half of the horizon
    // (`sim::entity_clock_period`: a drifting entity can show a short pattern near the end)
    if !c.design.entities.is_empty() && w0 > fr.ids.len() / 2 {
        return Ok(Err(format!("not periodic over the second half of {} gt (period {p} only from {w0}: an entity that never repeats)", o.horizon)));
    }
    // the cells: every cell non-air at some tick of one period, and the output cell; their states
    // over that period (index t - w0)
    let mut idxs: std::collections::BTreeSet<u32> = (w0..w0 + p).flat_map(|t| fr.sparse[t].iter().map(|(i, _)| *i)).collect();
    if let Some(op) = c.output {
        idxs.insert(fr.index(op));
    }
    let idx_vec: Vec<u32> = idxs.into_iter().collect();
    let cells: Vec<Pos> = idx_vec.iter().map(|i| fr.pos_of(*i)).collect();
    let air = fr.int.state("minecraft:air");
    let per_frame: Vec<Vec<u32>> = (w0..w0 + p)
        .map(|t| {
            let mut v = vec![air; idx_vec.len()];
            for (i, id) in &fr.sparse[t] {
                if let Ok(k) = idx_vec.binary_search(i) {
                    v[k] = *id;
                }
            }
            v
        })
        .collect();
    let int = &fr.int;
    let states: Vec<Vec<&str>> = per_frame.iter().map(|v| v.iter().map(|id| int.name(*id)).collect()).collect();
    // the judge's lamp is no part of the clock: left out of its cells and family
    let mut analysis = match fr.lamp.and_then(|l| cells.iter().position(|q| *q == l)) {
        Some(li) => {
            let cells2: Vec<Pos> = cells.iter().enumerate().filter(|(i, _)| *i != li).map(|(_, q)| *q).collect();
            let states2: Vec<Vec<&str>> = states.iter().map(|v| v.iter().enumerate().filter(|(i, _)| *i != li).map(|(_, s)| *s).collect()).collect();
            wave::analyze(&cells2, &states2)
        }
        None => wave::analyze(&cells, &states),
    };
    // the design's entities (a minecart riding its rails) are part of the mechanism: their kinds
    // join the family (`detector_rail+minecart`)
    for e in &c.design.entities {
        let k = short(&e.kind).to_string();
        if !analysis.family.contains(&k) {
            analysis.family.push(k);
        }
    }
    analysis.family.sort();
    if analysis.cells.is_empty() {
        return Ok(Err(if p == 1 { format!("goes still at tick {w0}") } else { format!("no block changes in its period ({p} gt)") }));
    }
    let cell_period = |pos: Pos| -> usize {
        match cells.iter().position(|q| *q == pos) {
            Some(i) => wave::cyclic_period(&states.iter().map(|v| v[i]).collect::<Vec<_>>()),
            None => p,
        }
    };
    let (output, output_period) = match c.output {
        Some(op) => {
            let idx = cells.iter().position(|q| *q == op).ok_or_else(|| anyhow!("output outside the box"))?;
            // the output cell stays free all cycle (vjeux 2026-10-20): the same block every tick,
            // never a moving block or a head (entities in it: redsim checks)
            let first = mctas::blockstate::block_name(wave::block_part(int.name(per_frame[0][idx]))).to_string();
            for t in w0..w0 + p {
                let s = int.name(per_frame[t - w0][idx]);
                let name = mctas::blockstate::block_name(wave::block_part(s));
                if name != first || name.contains("moving_piston") || name.contains("piston_head") {
                    return Ok(Err(format!("the output cell {op:?} is entered during the cycle ({} at tick {t}): it must hold the same block all cycle, nothing moved into or through it", short(s))));
                }
            }
            // where a builder can take the signal: an exterior air cell, not above tripwire, rails
            // or water (vjeux 2026-10-20)
            let design_block = if fr.lamp.is_some() { "minecraft:air".to_string() } else { c.design.blocks.get(&op).cloned().unwrap_or_else(|| "minecraft:air".into()) };
            let below = [op[0], op[1] - 1, op[2]];
            let bi = fr.index(below);
            let below_states: Vec<&str> = (w0..w0 + p).filter_map(|t| fr.sparse[t].iter().find(|(i, _)| *i == bi).map(|(_, id)| int.name(*id))).collect();
            if let Some(why) = crate::sim::placement_problem(op, (slo, shi), &design_block, below_states.into_iter()) {
                return Ok(Err(why));
            }
            let seq: Vec<bool> = (w0..w0 + p)
                .map(|t| {
                    let s = wave::block_part(int.name(per_frame[t - w0][idx]));
                    if fr.lamp.is_none() && wave::family_name(s).is_some() { wave::active(s) } else { signals.get(t).and_then(|v| v.first()).copied().unwrap_or(0) > 0 }
                })
                .collect();
            let levels: Vec<u8> = (w0..w0 + p).map(|t| signals.get(t).and_then(|v| v.first()).copied().unwrap_or(0)).collect();
            let (on, edges, widths) = wave::bool_wave(&seq);
            let gaps = wave::rise_gaps(&seq);
            let states = analysis.cells.iter().find(|w| w.pos == op).map(|w| w.states.clone()).unwrap_or_default();
            let own = if states.is_empty() { wave::cyclic_period(&levels) } else { cell_period(op) };
            (Some(Output { pos: op, auto: false, wave: CellWave { pos: op, states, kinds: Vec::new(), on, edges, widths, gaps } }), own)
        }
        None => return Ok(Err("no output cell: a clock needs an exterior air cell where the judge's lamp gets evenly spaced pulses (declare `port output`; `clock sim --exterior` finds the best one)".into())),
    };
    let groups = wave::spatial_groups(&analysis.cells.iter().map(|w| w.pos).collect::<Vec<_>>(), &wave::sensors_of(&c.design.blocks));
    if output_period < p && output_period > 1 && groups > 1 {
        let pos = output.as_ref().map(|o| o.pos).unwrap_or([0, 0, 0]);
        return Ok(Err(format!("COMPOSITE: the output {pos:?} repeats every {output_period} gt but the whole structure every {p} gt, and its changing cells form {groups} independent groups (sub-clocks side by side)")));
    }
    let dims = [shi[0] - slo[0] + 1, shi[1] - slo[1] + 1, shi[2] - slo[2] + 1];
    // a sculk clock: the ambient property in this build (two more runs in fresh sandboxes; the
    // events skip the swept box seen here)
    let vibration = if crate::vibration::has_sculk(&c.design) { crate::vibration::game_property(w, c, o.margin, w0 as u32, p as u32, (slo, shi), output.as_ref().map(|o| o.pos))? } else { None };
    Ok(Ok(RunOk { warmup: w0 as u32, period: p as u32, analysis, output, dims, vibration, items: None }))
}

fn probe_now(w: &mut Worker, sb: u32, probes: &[Pos]) -> Result<Vec<u8>> {
    if probes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(w.signals(sb, probes)?.into_iter().flat_map(|(a, b)| [a, b]).collect())
}

/// The judge: see the module doc.
pub fn verify(c: &Clock, o: &VerifyOptions) -> Result<VerifyReport> {
    let pool = spawn_pool(o)?;
    verify_in_pool(&pool, c, o)
}

/// The judge's JVM workers (`o.workers`, capped by MCTAS_MAX_WORKERS), to judge many clocks.
pub fn spawn_pool(o: &VerifyOptions) -> Result<Pool> {
    let builds = o.positions * if o.rotations { 4 } else { 1 };
    Pool::spawn(o.workers.min(builds).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })
}

/// [`verify`] with existing workers.
pub fn verify_in_pool(pool: &Pool, c: &Clock, o: &VerifyOptions) -> Result<VerifyReport> {
    // No `port output`: pick the output cell once (redsim's auto output on the unrotated clock)
    // and carry it through every rotation and position, so builds compare the same cell. The
    // horizon grows with redsim's period: warmup + 4 periods (capped at 20000 gt; coordinator,
    // 2026-10-19: a P2304 divider was "not periodic within 4000 gt").
    let mut fixed = c.clone();
    let r = crate::sim::simulate(c, &crate::sim::SimOptions::default());
    let mut o = o.clone();
    if r.verdict == Verdict::Clock {
        // the judge's periods of the whole state (the state can repeat slower than the output)
        let sp = r.period.max(r.state_period);
        let need = r.warmup.max(r.state_warmup) + (o.min_periods as u32 + 1) * sp + 100;
        // designs with entities must also repeat over the second half of the horizon; with item
        // entities, past their 6000 gt lifetime (a clock runs forever)
        let need = if c.design.entities.is_empty() { need } else { need.max(2 * r.warmup.max(r.state_warmup) + 100).max(crate::sim::entity_horizon(c)) };
        // the horizon scales with the period (vjeux 2026-10-20, root cause of 2 refusals: a
        // 20 000 gt cap left the P6926 jukebox carousel "still" during its third song and the
        // P10816 div-fade6-x5 short of 3 periods); 100 000 gt (83 min) at most
        o.horizon = o.horizon.max(need.min(100_000));
        if o.reload {
            // the chunk reload after the warmup and 2 periods, then room for a short glitch and
            // the judge's periods again
            let at = r.warmup.max(r.state_warmup) + 2 * sp + 1;
            o.reload_at = Some(at);
            o.reload_period = Some(r.state_period);
            o.horizon = o.horizon.max((at + (o.min_periods as u32 + 3) * sp + 140).min(100_000));
        }
    }
    let o = &o;
    if fixed.output.is_none() {
        // no declared output: the best exterior air cell where the judge's lamp gets evenly spaced
        // pulses (vjeux / coordinator, 2026-10-20), redsim's pick
        if let Some((q, _)) = crate::sim::lamp_outputs(&fixed, 1).into_iter().next() {
            fixed.design.ports.insert("output".into(), q);
            fixed.output = Some(q);
        }
    }
    let c = &fixed;
    let offsets = crate::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = rotated(w, c, t)?.translated(off);
        let mut r = run_one(w, &rc, o)?;
        // the ITEMS property on the base build (a property, never a failure)
        if t == 0
            && off == offsets[0]
            && let Ok(ok) = &mut r
        {
            let base = ok.clone();
            ok.items = items_property(w, &rc, o, &base)?;
        }
        // the chunk reload in redsim on the same build (tickexact: redsim::reload), which the
        // game's run confirms
        let sim = match (o.reload_at, o.reload_period) {
            (Some(at), Some(want)) => {
                let (lo, hi) = rc.bounds();
                let m = o.margin;
                Some(crate::sim::reload_check(&rc, [lo[0] - m, lo[1] - m, lo[2] - m], [hi[0] + m, hi[1] + m, hi[2] + m], at, want, o.horizon, o.min_periods))
            }
            _ => None,
        };
        Ok((r, sim))
    });
    let mut runs = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let (result, sim_reload) = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        runs.push(Run { turns: t, offset: off, result, sim_reload });
    }
    let mut rep = VerifyReport { name: c.name().to_string(), blocks: c.blocks(), start: c.start_label(), runs, robustness: String::new(), verdict: Verdict::Clock, reload_at: o.reload_at };
    let ok: Vec<&Run> = rep.runs.iter().filter(|r| r.result.is_ok()).collect();
    if ok.is_empty() {
        rep.verdict = Verdict::Invalid;
        rep.robustness = rep.runs.first().and_then(|r| r.result.as_ref().err().cloned()).unwrap_or_default();
        if rep.robustness.starts_with("COMPOSITE") {
            rep.verdict = Verdict::Composite;
        }
        return Ok(rep);
    }
    let base_sig = rep.base().unwrap().signature();
    let differs = |r: &Run| r.result.as_ref().map(|k| k.signature() != base_sig).unwrap_or(true);
    let pos_dep = rep.runs.iter().filter(|r| r.turns == 0).any(differs);
    let rot_dep = rep.runs.iter().filter(|r| r.turns != 0).any(differs);
    rep.robustness = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    Ok(rep)
}
