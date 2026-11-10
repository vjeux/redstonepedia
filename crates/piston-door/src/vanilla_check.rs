//! `mctas door vanilla-check`: run a design in an UNMODIFIED vanilla dedicated server (the
//! official server jar, `java -jar`, RCON + a bot player, see [`mctas::vanilla`]) and compare it
//! with the simulator tick for tick, at the judge's positions, with the judge's rules.
//!
//! Per position (design translated by the offset, same absolute coordinates in both worlds, so
//! position-dependent update orders match):
//! * Simulator: exactly the judge's sequence (raw placement, wire shape pass, a `self` update of
//!   every block, settle), recording the watch box after every tick.
//! * Vanilla: `/setblock ... strict` of the simulator's post-shape-pass states (strict uses the
//!   very flags of the judge's raw placement: UPDATE_CLIENTS|KNOWN_SHAPE|SUPPRESS_DROPS|
//!   SKIP_BLOCK_ENTITY_SIDEEFFECTS|SKIP_ON_PLACE); vanilla has no command for the judge's
//!   `self` update, so that step is skipped (a design whose rest state depends on it shows up
//!   as a rest-state divergence). Time is frozen and stepped one tick at a time; the lever is
//!   right-clicked by the bot (`LeverBlock.useWithoutItem` -> `pull`, what the judge calls);
//!   after every tick the box is read back from the saved region files.
//! * Both runs go through the same rule checks ([`judge`], a copy of `verify_at`'s), and the
//!   frames are compared cell by cell.

use crate::{DoorSpec, closed_problems, legality_problems, open_problems, settled_from, watch_box};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, StateFlags, Worker};
use mctas::vanilla::VanillaServer;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub cycles: u32,
    pub offsets: Vec<Pos>,
    pub max_ticks: u32,
    /// Print every position's details (default: only divergences and failures).
    pub verbose: bool,
    /// Also run one cycle in REAL TIME at the first position: unfrozen at 20 tps, the bot clicks
    /// while the game runs, the world is read back once everything has settled.
    pub free_run: bool,
    /// Record what is pending after every tick (both worlds) and print it for diverging phases.
    pub trace: bool,
}

/// One open/close cycle as recorded.
#[derive(Clone, Debug, Default)]
pub struct CycleRun {
    pub open: Vec<Region>,
    pub open_idle: bool,
    pub open_entities: u32,
    pub close: Vec<Region>,
    pub close_idle: bool,
    pub close_entities: u32,
    /// Per tick, what is still pending (only with `trace`).
    pub open_pending: Vec<String>,
    pub close_pending: Vec<String>,
}

/// Everything one world did at one position.
#[derive(Clone, Debug)]
pub struct Run {
    pub settle: Vec<Region>,
    pub settle_idle: bool,
    pub rest: Region,
    pub cycles: Vec<CycleRun>,
    pub settle_pending: Vec<String>,
}

/// The judge's rules (`verify_at` in lib.rs) applied to a recorded run. Returns the
/// (open, close) ticks of every cycle.
pub fn judge(run: &Run, spec: &DoorSpec) -> Result<Vec<(u32, u32)>> {
    if !run.settle_idle {
        bail!("the build never settles after placement ({} ticks)", run.settle.len());
    }
    let rest = &run.rest;
    let problems = closed_problems(rest, spec);
    if !problems.is_empty() {
        bail!("not closed at rest: {}", problems.join("; "));
    }
    let lever = rest.get(spec.lever).unwrap_or("?");
    if !lever.starts_with("minecraft:lever") {
        bail!("port lever {:?} is {lever}, expected a lever", spec.lever);
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap_or("?").to_string()).collect();
    let mut timings = Vec::new();
    for (cycle, c) in run.cycles.iter().enumerate() {
        if !c.open_idle {
            bail!("cycle {cycle}: still moving {} ticks after opening", c.open.len());
        }
        let last = c.open.last().context("no frames")?;
        let problems = open_problems(last, spec);
        if !problems.is_empty() {
            bail!("cycle {cycle}: not open after the lever was switched on: {}", problems.join("; "));
        }
        let open_ticks = settled_from(&c.open, |f| open_problems(f, spec).is_empty()).unwrap_or(0);
        if c.open_entities > 0 {
            bail!("cycle {cycle}: entities appeared while opening (something broke)");
        }
        if !c.close_idle {
            bail!("cycle {cycle}: still moving {} ticks after closing", c.close.len());
        }
        let last = c.close.last().context("no frames")?;
        let door_now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap_or("?").to_string()).collect();
        if door_now != closed_door {
            let diffs: Vec<String> = spec
                .door_cells()
                .iter()
                .zip(door_now.iter().zip(closed_door.iter()))
                .filter(|(_, (a, b))| a != b)
                .map(|(p, (a, b))| format!("{p:?} is {a}, was {b}"))
                .collect();
            bail!("cycle {cycle}: door not restored: {}", diffs.join("; "));
        }
        let diffs: Vec<String> =
            rest.iter().filter(|(p, s)| last.get(*p) != Some(*s)).take(8).map(|(p, s)| format!("{p:?} {s} -> {}", last.get(p).unwrap_or("?"))).collect();
        if !diffs.is_empty() {
            bail!("cycle {cycle}: the build did not return to its rest state: {}", diffs.join("; "));
        }
        if c.close_entities > 0 {
            bail!("cycle {cycle}: entities appeared while closing (something broke)");
        }
        let close_ticks =
            settled_from(&c.close, |f| spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))).unwrap_or(0);
        timings.push((open_ticks, close_ticks));
    }
    Ok(timings)
}

/// Step the simulator until idle (max ticks), recording the box; with `trace`, one tick per
/// request (same semantics: the bridge checks idle after every tick) plus the pending counts.
pub(crate) fn sim_phase(w: &mut Worker, sb: u32, max: u32, lo: Pos, hi: Pos, trace: bool) -> Result<(Vec<Region>, bool, Vec<String>)> {
    if !trace {
        let r = w.step_record(sb, max, true, lo, hi)?;
        return Ok((r.frames, r.idle == Some(true), Vec::new()));
    }
    let (mut frames, mut pending) = (Vec::new(), Vec::new());
    while (frames.len() as u32) < max {
        let r = w.step_record(sb, 1, true, lo, hi)?;
        frames.extend(r.frames);
        let st = w.status(sb)?;
        pending.push(format!("block ticks {} events {} moving {}", st.block_ticks, st.block_events, st.moving_pistons));
        if r.idle == Some(true) {
            return Ok((frames, true, pending));
        }
    }
    Ok((frames, false, pending))
}

/// The simulator side: the judge's exact sequence, every tick recorded. Also returns the
/// placement vanilla must reproduce (the design after the judge's wire shape pass).
fn sim_run(w: &mut Worker, sb: u32, d: &Design, spec: &DoorSpec, opts: &CheckOptions) -> Result<(Vec<(Pos, String)>, Run)> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let (lo, hi) = d.bounds().context("empty design")?;
    let shaped = w.get_region(sb, lo, hi)?;
    let placed: Vec<(Pos, String)> = blocks.iter().map(|(p, _)| (*p, shaped.get(*p).unwrap_or("?").to_string())).collect();
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let (wlo, whi) = watch_box(d, spec);
    let (settle, settle_idle, settle_pending) = sim_phase(w, sb, opts.max_ticks, wlo, whi, opts.trace)?;
    let rest = w.get_region(sb, wlo, whi)?;
    let mut cycles = Vec::new();
    for _ in 0..opts.cycles {
        let mut c = CycleRun::default();
        w.use_block(sb, spec.lever)?;
        (c.open, c.open_idle, c.open_pending) = sim_phase(w, sb, opts.max_ticks, wlo, whi, opts.trace)?;
        c.open_entities = w.entities(sb)?.len() as u32;
        w.use_block(sb, spec.lever)?;
        (c.close, c.close_idle, c.close_pending) = sim_phase(w, sb, opts.max_ticks, wlo, whi, opts.trace)?;
        c.close_entities = w.entities(sb)?.len() as u32;
        cycles.push(c);
    }
    Ok((placed, Run { settle, settle_idle, rest, cycles, settle_pending }))
}

/// Step one tick at a time, reading the box back after each.
///
/// With `judge_ticks = Some(n)` (the simulator went idle after n ticks), vanilla steps exactly n
/// ticks, so the next lever flip happens at the very same tick as in the judge; it must then be
/// quiet on its own observable state (no saved block/fluid ticks, no moving pistons, no falling
/// blocks). If it is not, it keeps stepping until it is (the extra frames are a divergence).
/// Without a target (the simulator never settled), vanilla stops on its own: quiet, confirmed by
/// one more tick in which nothing changes (block events are not saved to disk), or `max_ticks`.
pub(crate) fn vanilla_record(v: &mut VanillaServer, lo: Pos, hi: Pos, max_ticks: u32, judge_ticks: Option<usize>) -> Result<(Vec<Region>, bool, Vec<String>)> {
    let mut frames: Vec<Region> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut quiet_since: Option<usize> = None;
    loop {
        v.step(1)?;
        let snap = v.snapshot(lo, hi)?;
        let busy = !snap.block_ticks.is_empty() || snap.fluid_ticks > 0 || snap.moving_pistons() > 0;
        let quiet = !busy && v.count_entities("@e[type=minecraft:falling_block]")? == 0;
        let same_as_before = frames.last().is_some_and(|f| same_states(f, &snap.region));
        let ticks: Vec<String> = snap.block_ticks.iter().map(|(p, id, t)| format!("{p:?} {} +{t}", id.trim_start_matches("minecraft:"))).collect();
        pending.push(format!("block ticks {} [{}] moving {}", snap.block_ticks.len(), ticks.join(", "), snap.moving_pistons()));
        frames.push(snap.region);
        match judge_ticks {
            Some(n) if frames.len() < n => {}
            Some(_) if quiet => return Ok((frames, true, pending)),
            _ if quiet => match quiet_since {
                Some(_) if same_as_before => return Ok((frames, true, pending)),
                _ => quiet_since = Some(frames.len() - 1),
            },
            _ => quiet_since = None,
        }
        if frames.len() as u32 >= max_ticks + 1 {
            return Ok((frames, false, pending));
        }
    }
}

pub(crate) fn same_states(a: &Region, b: &Region) -> bool {
    a.size == b.size && a.from == b.from && a.iter().zip(b.iter()).all(|((_, x), (_, y))| x == y)
}

/// Where the bot stands to click the lever: within reach, preferably outside the watch box (a
/// player pushed around on the ground could make vibrations; flying in the void it makes none).
pub(crate) fn bot_spot(d: &Design, spec: &DoorSpec, wlo: Pos, whi: Pos) -> [f64; 3] {
    // Within reach of the lever; then, in order: touching no block of the build (nothing can
    // push it, it stands on nothing, so it never steps or lands: no vibrations), outside the
    // watch box, far from sculk sensors, close to the lever. (A bot standing on rb2s2-50b's
    // sensor made it fire with power 15 at some flips: a player there is a real trigger.)
    let l = spec.lever;
    let inside = |p: Pos| (0..3).all(|i| p[i] >= wlo[i] && p[i] <= whi[i]);
    let sensors: Vec<Pos> = d.blocks.iter().filter(|(_, s)| s.contains("sculk_sensor")).map(|(p, _)| *p).collect();
    let mut best: Option<((i32, i32, i64, i64), Pos)> = None;
    for dx in -4..=4 {
        for dy in -4..=3 {
            for dz in -4..=4 {
                let feet = [l[0] + dx, l[1] + dy, l[2] + dz];
                let head = [feet[0], feet[1] + 1, feet[2]];
                let eye = [feet[0] as f64 + 0.5, feet[1] as f64 + 1.62, feet[2] as f64 + 0.5];
                let dist = ((eye[0] - l[0] as f64 - 0.5).powi(2) + (eye[1] - l[1] as f64 - 0.5).powi(2) + (eye[2] - l[2] as f64 - 0.5).powi(2)).sqrt();
                if dist > 4.0 || d.blocks.contains_key(&feet) || d.blocks.contains_key(&head) {
                    continue;
                }
                let mut touching = 0;
                for ox in -1..=1 {
                    for oy in -1..=2 {
                        for oz in -1..=1 {
                            if d.blocks.contains_key(&[feet[0] + ox, feet[1] + oy, feet[2] + oz]) {
                                touching += 1;
                            }
                        }
                    }
                }
                let sensor_dist = sensors
                    .iter()
                    .map(|s| (((s[0] - feet[0]).pow(2) + (s[1] - feet[1]).pow(2) + (s[2] - feet[2]).pow(2)) as f64).sqrt())
                    .fold(f64::INFINITY, f64::min);
                let score = (touching, if !inside(feet) && !inside(head) { 0 } else { 1 }, -(sensor_dist.min(99.0) * 100.0) as i64, (dist * 100.0) as i64);
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((score, feet));
                }
            }
        }
    }
    let p = best.map(|b| b.1).unwrap_or([l[0], l[1] + 2, l[2]]);
    [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5]
}

/// Entities the judge would see: every non-player entity, plus anything the bot picked up (a
/// player near a broken build collects the dropped items; the judge's sandbox has no player).
pub(crate) fn vanilla_entities(v: &mut VanillaServer) -> Result<u32> {
    let mut n = v.count_entities("@e[type=!minecraft:player]")?;
    let name = v.bot.as_ref().map(|b| b.name.clone()).unwrap_or_default();
    if v.cmd(&format!("execute if data entity {name} Inventory[0]"))?.contains("Test passed") {
        n += 1;
        v.cmd(&format!("clear {name}"))?;
    }
    Ok(n)
}

/// Right-click the lever with the bot and wait until it shows the OTHER state: a lever toggles,
/// and a design whose lever rests powered opens by switching it off. (Waiting for a fixed
/// `powered=on` returned at once on such a door, c79 among them, and the click then landed
/// whenever the server got to the bot's packet: before the next `tick step` on an idle box, a
/// tick later under load, so vanilla ran a phase one tick behind the simulator.) Returns the
/// lever's new state.
pub(crate) fn flip(v: &mut VanillaServer, lever: Pos) -> Result<bool> {
    if !v.test_block(lever, "minecraft:lever")? {
        // The lever is gone (a broken build popped it). The judge then right-clicks whatever is
        // there with a mock player (nothing, for air); the bot does the same and moves on.
        v.bot_use(lever, 1)?;
        std::thread::sleep(std::time::Duration::from_millis(200));
        v.cmd("list")?;
        return Ok(false);
    }
    let on = !v.test_block(lever, "minecraft:lever[powered=true]")?;
    v.bot_use(lever, 1)?;
    let t0 = std::time::Instant::now();
    loop {
        if v.test_block(lever, &format!("minecraft:lever[powered={on}]"))? {
            return Ok(on);
        }
        if let Some(e) = v.bot.as_ref().and_then(|b| b.error()) {
            bail!("bot: {e}");
        }
        if t0.elapsed() > std::time::Duration::from_secs(15) {
            let now = v.snapshot(lever, lever).ok().and_then(|s| s.region.get(lever).map(|x| x.to_string())).unwrap_or_default();
            let at = v.bot.as_ref().map(|b| b.position()).unwrap_or_default();
            bail!("the bot's click on the lever {lever:?} did not flip it: the cell holds {now}; bot at {at:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
}

/// Force-load the chunks of a box and wait until they tick with entities loaded.
pub(crate) fn load_box(v: &mut VanillaServer, lo: Pos, hi: Pos) -> Result<()> {
    v.cmd_ok(&format!("forceload add {} {} {} {}", lo[0], lo[2], hi[0], hi[2])).or_else(|e| {
        // "No chunks were marked" when they are all force-loaded already.
        if format!("{e:#}").contains("No chunks") { Ok(String::new()) } else { Err(e) }
    })?;
    let t0 = std::time::Instant::now();
    for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
        for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
            while !v.cmd(&format!("execute if loaded {} 0 {}", cx * 16, cz * 16))?.contains("Test passed") {
                if t0.elapsed() > std::time::Duration::from_secs(60) {
                    bail!("chunk {cx},{cz} never finished loading");
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
    Ok(())
}

/// Empty a box: blocks (no updates), non-player entities, and let pending ticks and the redstone
/// torch burnout memory (60 gt) expire.
pub(crate) fn clean(v: &mut VanillaServer, lo: Pos, hi: Pos) -> Result<()> {
    // A broken design can push blocks out of its watch box: clear a margin around it too.
    let (flo, fhi) = ([lo[0] - 6, lo[1] - 6, lo[2] - 6], [hi[0] + 6, hi[1] + 6, hi[2] + 6]);
    load_box(v, flo, fhi)?;
    v.cmd(&format!("fill {} {} {} {} {} {} minecraft:air strict", flo[0], flo[1], flo[2], fhi[0], fhi[1], fhi[2]))?;
    v.cmd("kill @e[type=!minecraft:player]")?;
    v.step(80)?;
    let snap = v.snapshot(flo, fhi)?;
    if snap.region.palette.iter().any(|s| s != "minecraft:air") || !snap.block_ticks.is_empty() || !snap.block_entities.is_empty() {
        bail!(
            "the box {flo:?}..{fhi:?} is not clean after clearing: {:?} ticks {:?} block entities {:?}",
            snap.region.palette,
            snap.block_ticks,
            snap.block_entities
        );
    }
    Ok(())
}

/// The vanilla side of one position.
fn vanilla_run(v: &mut VanillaServer, placed: &[(Pos, String)], d: &Design, spec: &DoorSpec, opts: &CheckOptions, sim: &Run) -> Result<Run> {
    // Flip on the judge's schedule: each phase lasts as long as it did in the simulator (when it
    // settled there); vanilla must be quiet by then on its own observable state.
    let target = |frames: &[Region], idle: bool| idle.then_some(frames.len());
    let (wlo, whi) = watch_box(d, spec);
    load_box(v, wlo, whi)?;
    // The previous position (any earlier design too) is wiped first, then this box.
    if let Some((plo, phi)) = v.dirty.take() {
        clean(v, plo, phi)?;
    }
    clean(v, wlo, whi)?;
    let mut spot = bot_spot(d, spec, wlo, whi);
    // Experiments: MCTAS_BOT_OFFSET=dx,dy,dz puts the bot's feet at lever + offset.
    if let Some(o) = std::env::var("MCTAS_BOT_OFFSET").ok().and_then(|s| {
        let v: Vec<i32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    }) {
        let l = spec.lever;
        spot = [(l[0] + o[0]) as f64 + 0.5, (l[1] + o[1]) as f64, (l[2] + o[2]) as f64 + 0.5];
    }
    v.teleport_bot(spot[0], spot[1], spot[2])?;
    v.dirty = Some((wlo, whi));
    for (p, s) in placed {
        let out = v.cmd(&format!("setblock {} {} {} {s} strict", p[0], p[1], p[2]))?;
        if !out.contains("Changed the block") {
            bail!("setblock {p:?} {s}: {out}");
        }
    }
    let (settle, settle_idle, settle_pending) = vanilla_record(v, wlo, whi, opts.max_ticks, target(&sim.settle, sim.settle_idle))?;
    let rest = settle.last().cloned().context("no frames")?;
    let mut cycles = Vec::new();
    for i in 0..opts.cycles as usize {
        let s = sim.cycles.get(i);
        let mut c = CycleRun::default();
        if let Err(e) = flip(v, spec.lever) {
            // Say what vanilla's rest looked like next to the simulator's (bare names unexpanded).
            let d = frame_diff(&sim.rest, &rest, 6);
            bail!("{e:#} | vanilla rest vs simulator rest: {}", if d.is_empty() { "same".into() } else { d.join("; ") });
        }
        let t = s.and_then(|s| target(&s.open, s.open_idle));
        (c.open, c.open_idle, c.open_pending) = vanilla_record(v, wlo, whi, opts.max_ticks, t)?;
        c.open_entities = vanilla_entities(v)?;
        flip(v, spec.lever)?;
        let t = s.and_then(|s| target(&s.close, s.close_idle));
        (c.close, c.close_idle, c.close_pending) = vanilla_record(v, wlo, whi, opts.max_ticks, t)?;
        c.close_entities = vanilla_entities(v)?;
        cycles.push(c);
    }
    Ok(Run { settle, settle_idle, rest, cycles, settle_pending })
}

/// What the simulator knows about block states: the full default state of a bare block name
/// (26.x saves default states as bare names) and the collision flags of a state (a pure
/// function of the state). Unknown ones are looked up by placing them in a scratch sandbox.
#[derive(Default)]
pub struct Oracle {
    defaults: HashMap<String, String>,
    flags: HashMap<String, StateFlags>,
}

impl Oracle {
    pub(crate) fn learn(&mut self, r: &Region) {
        for (s, f) in r.palette.iter().zip(r.flags.iter()) {
            self.flags.entry(s.clone()).or_insert(*f);
        }
    }

    /// Expand bare names and fill the flags of vanilla regions.
    pub(crate) fn apply(&mut self, w: &mut Worker, regions: &mut [&mut Region]) -> Result<()> {
        let mut unknown: Vec<String> = Vec::new();
        for r in regions.iter() {
            for s in &r.palette {
                let need = if s.contains('[') { !self.flags.contains_key(s) } else { !self.defaults.contains_key(s) };
                if need && !unknown.contains(s) {
                    unknown.push(s.clone());
                }
            }
        }
        if !unknown.is_empty() {
            let sb = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
            let at = |i: usize| -> Pos { [(i % 16) as i32, 2 * (i / 16) as i32, 0] };
            let blocks: Vec<(Pos, String)> = unknown.iter().enumerate().map(|(i, s)| (at(i), s.clone())).collect();
            w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
            let r = w.get_region(sb, [0, 0, 0], [15, 2 * (unknown.len() / 16) as i32 + 1, 0])?;
            let _ = w.drop_sandbox(sb);
            for (i, s) in unknown.iter().enumerate() {
                let full = r.get(at(i)).context("oracle read")?.to_string();
                let f = r.flags_at(at(i)).unwrap_or_default();
                if !s.contains('[') {
                    self.defaults.insert(s.clone(), full.clone());
                }
                self.flags.entry(full).or_insert(f);
            }
        }
        for r in regions.iter_mut() {
            for s in r.palette.iter_mut() {
                if !s.contains('[') {
                    if let Some(full) = self.defaults.get(s) {
                        *s = full.clone();
                    }
                }
            }
            r.flags = r.palette.iter().map(|s| self.flags.get(s).copied().unwrap_or_default()).collect();
        }
        Ok(())
    }
}

/// Give vanilla regions full state strings and the simulator's collision flags.
pub(crate) fn fill_flags(w: &mut Worker, oracle: &mut Oracle, sim: &Run, van: &mut Run) -> Result<()> {
    oracle.learn(&sim.rest);
    for f in sim.settle.iter().chain(sim.cycles.iter().flat_map(|c| c.open.iter().chain(c.close.iter()))) {
        oracle.learn(f);
    }
    let mut regions: Vec<&mut Region> = vec![&mut van.rest];
    regions.extend(van.settle.iter_mut());
    for c in van.cycles.iter_mut() {
        regions.extend(c.open.iter_mut());
        regions.extend(c.close.iter_mut());
    }
    oracle.apply(w, &mut regions)
}

/// Tick by tick: what each world still had pending, and the cells that differ.
pub(crate) fn print_trace(phase: &str, sim: &[Region], sim_pending: &[String], van: &[Region], van_pending: &[String]) {
    println!("    trace {phase}:");
    for t in 0..sim.len().max(van.len()) {
        let diff = match (sim.get(t), van.get(t)) {
            (Some(a), Some(b)) => {
                let d = frame_diff(a, b, 4);
                if d.is_empty() { "same".to_string() } else { d.join("; ") }
            }
            _ => "-".to_string(),
        };
        println!(
            "      t={:<3} sim: {:<40} | vanilla: {} | {diff}",
            t + 1,
            sim_pending.get(t).map(|s| s.as_str()).unwrap_or("-"),
            van_pending.get(t).map(|s| s.as_str()).unwrap_or("-")
        );
    }
}

/// First cells that differ between two frames.
pub(crate) fn frame_diff(a: &Region, b: &Region, limit: usize) -> Vec<String> {
    a.iter().filter(|(p, s)| b.get(*p) != Some(*s)).take(limit).map(|(p, s)| format!("{p:?} sim {s} / vanilla {}", b.get(p).unwrap_or("?"))).collect()
}

/// Compare a phase's frames; vanilla may carry one trailing "confirmation" frame equal to its
/// last one. Returns the first divergence.
pub(crate) fn compare_frames(phase: &str, sim: &[Region], van: &[Region], sim_idle: bool, van_idle: bool) -> Option<String> {
    let n = sim.len().min(van.len());
    for t in 0..n {
        let d = frame_diff(&sim[t], &van[t], 6);
        if !d.is_empty() {
            return Some(format!("{phase} tick {}: {}", t + 1, d.join("; ")));
        }
    }
    if sim_idle != van_idle {
        return Some(format!("{phase}: simulator idle={sim_idle} after {} ticks, vanilla idle={van_idle} after {} ticks", sim.len(), van.len()));
    }
    // Vanilla steps exactly as many ticks as the judge did when the judge settled; more frames
    // mean vanilla still had something pending (scheduled ticks, moving blocks) at that tick.
    if sim_idle && van.len() != sim.len() {
        let last = sim.last()?;
        let moved = van.iter().enumerate().skip(sim.len()).find(|(_, f)| !same_states(last, f));
        return Some(match moved {
            Some((t, f)) => format!("{phase} tick {}: vanilla still changing after the simulator went idle at tick {}: {}", t + 1, sim.len(), frame_diff(last, f, 6).join("; ")),
            None => format!("{phase}: vanilla still had pending work when the simulator went idle at tick {} (quiet after {}, no block changed)", sim.len(), van.len()),
        });
    }
    None
}

/// Result of one position.
#[derive(Debug)]
pub struct PositionResult {
    pub offset: Pos,
    pub sim: std::result::Result<Vec<(u32, u32)>, String>,
    pub vanilla: std::result::Result<Vec<(u32, u32)>, String>,
    pub divergence: Option<String>,
    pub ticks_compared: usize,
    /// The real-time cycle (first position only): Ok, or what differed / failed.
    pub free_run: Option<std::result::Result<(), String>>,
}

/// One open/close cycle with the game RUNNING (unfrozen, vanilla's 20 tps): the bot clicks, the
/// server ticks on its own, and the world is frozen and read back only after the simulator's
/// settle time plus a margin. The result must equal the simulator's settled open state and the
/// rest state, with no entities and nothing pending.
fn vanilla_free_run(v: &mut VanillaServer, oracle: &mut Oracle, w: &mut Worker, d: &Design, spec: &DoorSpec, sim: &Run, rate: u32) -> Result<std::result::Result<(), String>> {
    let (wlo, whi) = watch_box(d, spec);
    let c0 = sim.cycles.first().context("no cycles")?;
    let (Some(sim_open), Some(sim_closed)) = (c0.open.last(), c0.close.last()) else { bail!("no simulator frames") };
    let run_until = |v: &mut VanillaServer, ticks: usize| -> Result<(Region, u32, bool)> {
        if rate != 20 {
            v.cmd_ok("tick rate 20")?;
        }
        v.cmd_ok("tick unfreeze")?;
        flip(v, spec.lever)?;
        let t0 = v.game_time()?;
        let started = std::time::Instant::now();
        while v.game_time()? < t0 + ticks as i64 {
            if started.elapsed() > std::time::Duration::from_secs(120) {
                bail!("the game time does not advance while unfrozen");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        v.cmd_ok("tick freeze")?;
        if rate != 20 {
            v.cmd_ok(&format!("tick rate {rate}"))?;
        }
        let snap = v.snapshot(wlo, whi)?;
        let busy = !snap.block_ticks.is_empty() || snap.fluid_ticks > 0 || snap.moving_pistons() > 0;
        let entities = vanilla_entities(v)?;
        Ok((snap.region, entities, !busy))
    };
    let (mut open, open_entities, open_idle) = run_until(v, c0.open.len() + 20)?;
    let (mut closed, close_entities, close_idle) = run_until(v, c0.close.len() + 20)?;
    oracle.apply(w, &mut [&mut open, &mut closed])?;
    let mut problems = Vec::new();
    let d_open = frame_diff(sim_open, &open, 6);
    if !d_open.is_empty() {
        problems.push(format!("open state differs from the simulator's: {}", d_open.join("; ")));
    }
    let open_bad = open_problems(&open, spec);
    if !open_bad.is_empty() {
        problems.push(format!("not open: {}", open_bad.join("; ")));
    }
    let d_closed = frame_diff(sim_closed, &closed, 6);
    if !d_closed.is_empty() {
        problems.push(format!("closed state differs from the simulator's: {}", d_closed.join("; ")));
    }
    let d_rest = frame_diff(&sim.rest, &closed, 6);
    if !d_rest.is_empty() {
        problems.push(format!("not back to rest: {}", d_rest.join("; ")));
    }
    if open_entities + close_entities > 0 {
        problems.push(format!("{} entities appeared", open_entities + close_entities));
    }
    if !open_idle || !close_idle {
        problems.push("still busy (ticks or moving pistons) after the simulator's settle time + 20".into());
    }
    Ok(if problems.is_empty() { Ok(()) } else { Err(problems.join(" | ")) })
}

/// Check one design at every offset. Errors are tool failures; door verdicts are in the results.
pub fn check(w: &mut Worker, v: &mut VanillaServer, oracle: &mut Oracle, design: &Design, opts: &CheckOptions) -> Result<Vec<PositionResult>> {
    let illegal = legality_problems(design);
    if !illegal.is_empty() {
        bail!("illegal design: {}", illegal.join("; "));
    }
    // A file that is not its own rest state relies on the judge's `self` update (wire powers
    // recomputed, pistons re-checked) which vanilla has no command for: check its rest state.
    let rested;
    let full = |w: &mut Worker, oracle: &mut Oracle, s: &str| -> Result<String> {
        let mut r = Region { from: [0, 0, 0], size: [1, 1, 1], palette: vec![s.to_string()], flags: Vec::new(), data: vec![0] };
        oracle.apply(w, &mut [&mut r])?;
        Ok(r.palette[0].clone())
    };
    let design = match rest_state(w, design)? {
        Some(r) if r.blocks != design.blocks => {
            // Bare names in the file (`minecraft:sculk_sensor`) are the full default states.
            let mut n = design.blocks.keys().filter(|p| !r.blocks.contains_key(*p)).count();
            for (p, s) in &r.blocks {
                match design.blocks.get(p) {
                    Some(f) if f == s => {}
                    Some(f) if !f.contains('[') && full(w, oracle, f)? == *s => {}
                    _ => n += 1,
                }
            }
            if n > 0 {
                println!("  note: {} is not at rest as written ({n} cells change when the judge settles it); checking its rest state", design.name);
            }
            rested = r;
            &rested
        }
        _ => design,
    };
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut out = Vec::new();
    for off in &opts.offsets {
        let d = design.translated(*off);
        let spec = DoorSpec::from_design(&d)?;
        let (placed, sim) = sim_run(w, sb, &d, &spec, opts).with_context(|| format!("simulator at offset {off:?}"))?;
        let mut van = vanilla_run(v, &placed, &d, &spec, opts, &sim).with_context(|| format!("vanilla at offset {off:?}"))?;
        fill_flags(w, oracle, &sim, &mut van)?;
        let sim_verdict = judge(&sim, &spec).map_err(|e| format!("{e:#}"));
        let van_verdict = judge(&van, &spec).map_err(|e| format!("{e:#}"));
        let mut divergence = compare_frames("settle", &sim.settle, &van.settle, sim.settle_idle, van.settle_idle);
        let mut ticks = sim.settle.len().min(van.settle.len());
        if divergence.is_none() {
            let d = frame_diff(&sim.rest, &van.rest, 6);
            if !d.is_empty() {
                divergence = Some(format!("rest state: {}", d.join("; ")));
            }
        }
        for (i, (s, vc)) in sim.cycles.iter().zip(van.cycles.iter()).enumerate() {
            if divergence.is_some() {
                break;
            }
            divergence = compare_frames(&format!("cycle {i} open"), &s.open, &vc.open, s.open_idle, vc.open_idle)
                .or_else(|| (s.open_entities != vc.open_entities).then(|| format!("cycle {i} open: entities sim {} / vanilla {}", s.open_entities, vc.open_entities)))
                .or_else(|| compare_frames(&format!("cycle {i} close"), &s.close, &vc.close, s.close_idle, vc.close_idle))
                .or_else(|| (s.close_entities != vc.close_entities).then(|| format!("cycle {i} close: entities sim {} / vanilla {}", s.close_entities, vc.close_entities)));
            ticks += s.open.len().min(vc.open.len()) + s.close.len().min(vc.close.len());
        }
        let free_run = if opts.free_run && out.is_empty() && sim_verdict.is_ok() {
            Some(vanilla_free_run(v, oracle, w, &d, &spec, &sim, v.tick_rate).with_context(|| format!("free run at offset {off:?}"))?)
        } else {
            None
        };
        let r = PositionResult { offset: *off, sim: sim_verdict, vanilla: van_verdict, divergence, ticks_compared: ticks, free_run };
        if opts.trace && r.divergence.is_some() {
            print_trace("settle", &sim.settle, &sim.settle_pending, &van.settle, &van.settle_pending);
            for (i, (s, vc)) in sim.cycles.iter().zip(van.cycles.iter()).enumerate() {
                print_trace(&format!("cycle {i} open"), &s.open, &s.open_pending, &vc.open, &vc.open_pending);
                print_trace(&format!("cycle {i} close"), &s.close, &s.close_pending, &vc.close, &vc.close_pending);
            }
        }
        if opts.verbose || r.divergence.is_some() || r.sim.is_err() != r.vanilla.is_err() || r.free_run.as_ref().is_some_and(|f| f.is_err()) {
            let free = match &r.free_run {
                Some(Ok(())) => " | real-time cycle OK".to_string(),
                Some(Err(e)) => format!(" | REAL-TIME CYCLE: {e}"),
                None => String::new(),
            };
            println!(
                "  offset {:?}: sim {} | vanilla {} | {} ticks compared{}{free}",
                r.offset,
                verdict_str(&r.sim),
                verdict_str(&r.vanilla),
                r.ticks_compared,
                r.divergence.as_ref().map(|d| format!(" | DIVERGES: {d}")).unwrap_or_default()
            );
        }
        out.push(r);
    }
    let _ = w.drop_sandbox(sb);
    Ok(out)
}

pub fn verdict_str(v: &std::result::Result<Vec<(u32, u32)>, String>) -> String {
    match v {
        Ok(t) => {
            let open = t.iter().map(|x| x.0).max().unwrap_or(0);
            let close = t.iter().map(|x| x.1).max().unwrap_or(0);
            format!("PASS (open {open}, close {close})")
        }
        Err(e) => {
            let short: String = e.chars().take(160).collect();
            format!("FAIL ({short}{})", if e.chars().count() > 160 { "..." } else { "" })
        }
    }
}

/// The settled state of a design in the simulator (judge's placement sequence), read back over
/// its bounds plus a margin so blocks that moved are kept; `None` unless it settles and placing
/// the result again changes nothing (a true rest state). Used to make fuzz cases start at rest
/// (vanilla has no equivalent of the judge's `self` update).
pub fn rest_state(w: &mut Worker, d: &Design) -> Result<Option<Design>> {
    let once = |w: &mut Worker, d: &Design, allow_drops: bool| -> Result<Option<Region>> {
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
        let (lo, hi) = d.bounds().context("empty design")?;
        let r = w.get_region(sb, [lo[0] - 4, lo[1] - 4, lo[2] - 4], [hi[0] + 4, hi[1] + 4, hi[2] + 4])?;
        // Drops and strays (the design's own entities are part of it).
        let entities = w.entities(sb)?.iter().filter(|e| !e.design).count();
        let _ = w.drop_sandbox(sb);
        Ok((settle.idle == Some(true) && (allow_drops || entities == 0)).then_some(r))
    };
    let Some(r) = once(w, d, true)? else { return Ok(None) };
    let mut out = Design { name: d.name.clone(), meta: d.meta.clone(), ports: d.ports.clone(), blocks: Default::default(), entities: d.entities.clone(), block_entities: d.block_entities.clone(), nether_blocks: d.nether_blocks.clone(), nether_ports: d.nether_ports.clone() };
    for (p, s) in r.blocks() {
        if s.starts_with("minecraft:moving_piston") {
            return Ok(None);
        }
        out.blocks.insert(p, s);
    }
    let Some(again) = once(w, &out, false)? else { return Ok(None) };
    let same = r.iter().all(|(p, s)| again.get(p) == Some(s)) && again.iter().all(|(p, s)| r.get(p) == Some(s));
    Ok(same.then_some(out))
}

/// A random small contraption for differential fuzzing (not a door): a 6x5x6 box of solid
/// blocks, slime/honey, pistons, observers, repeaters, dust, torches, redstone blocks, lamps,
/// targets, waxed copper bulbs, note blocks, glazed terracotta, obsidian and sand, plus a lever
/// on a stone block. The `door` port sits at the box corner only so the watch box covers it.
pub fn fuzz_design(seed: u64) -> Design {
    let mut s = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut next = move |n: u64| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s % n
    };
    let dirs = ["north", "south", "east", "west", "up", "down"];
    let hdirs = ["north", "south", "east", "west"];
    let mut d = Design { name: format!("fuzz-{seed}"), ..Default::default() };
    for x in 0..6 {
        for y in 0..5 {
            for z in 0..6 {
                if next(100) >= 28 {
                    continue;
                }
                let f = dirs[next(6) as usize];
                let h = hdirs[next(4) as usize];
                let state = match next(24) {
                    0..=3 | 20..=23 => "minecraft:stone".to_string(),
                    4 => "minecraft:slime_block".to_string(),
                    5 => "minecraft:honey_block".to_string(),
                    6 => format!("minecraft:sticky_piston[extended=false,facing={f}]"),
                    7 => format!("minecraft:piston[extended=false,facing={f}]"),
                    8 => format!("minecraft:observer[facing={f},powered=false]"),
                    9 => format!("minecraft:repeater[delay={},facing={h},locked=false,powered=false]", 1 + next(4)),
                    10 | 11 => "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".to_string(),
                    12 => "minecraft:redstone_torch[lit=true]".to_string(),
                    13 => format!("minecraft:redstone_wall_torch[facing={h},lit=true]"),
                    14 => "minecraft:redstone_block".to_string(),
                    15 => "minecraft:redstone_lamp[lit=false]".to_string(),
                    16 => "minecraft:target[power=0]".to_string(),
                    17 => "minecraft:waxed_copper_bulb[lit=false,powered=false]".to_string(),
                    18 => format!("minecraft:white_glazed_terracotta[facing={h}]"),
                    _ => ["minecraft:obsidian", "minecraft:sand", "minecraft:note_block[instrument=harp,note=0,powered=false]"][next(3) as usize].to_string(),
                };
                d.blocks.insert([x, y, z], state);
            }
        }
    }
    // The lever sits on a random stone block of the box that touches a redstone component (it
    // strongly powers that block, so it drives the component); on top of the box if none.
    let active = |s: &str| {
        ["redstone_wire", "repeater", "piston", "redstone_torch", "redstone_wall_torch", "redstone_lamp", "target", "copper_bulb", "note_block"]
            .iter()
            .any(|k| s.contains(k))
    };
    let supports: Vec<Pos> = d
        .blocks
        .iter()
        .filter(|(p, s)| {
            s.as_str() == "minecraft:stone"
                && !d.blocks.contains_key(&[p[0], p[1] + 1, p[2]])
                && [[1, 0, 0], [-1, 0, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .any(|o| d.blocks.get(&[p[0] + o[0], p[1] + o[1], p[2] + o[2]]).is_some_and(|n| active(n)))
        })
        .map(|(p, _)| *p)
        .collect();
    let lever = if supports.is_empty() {
        let l = [next(6) as i32, 5, next(6) as i32];
        d.blocks.insert([l[0], 4, l[2]], "minecraft:stone".to_string());
        l
    } else {
        let p = supports[next(supports.len() as u64) as usize];
        [p[0], p[1] + 1, p[2]]
    };
    d.blocks.insert(lever, format!("minecraft:lever[face=floor,facing={},powered=false]", hdirs[next(4) as usize]));
    d.ports.insert("lever".to_string(), lever);
    d.ports.insert("door".to_string(), [0, 0, 0]);
    d
}
