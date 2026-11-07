//! Bedrock breaking study (designs/bedrock/NOTES.md).
//!
//! A scenario is a build next to a bedrock block, a timed script of player actions (each runs in
//! the inter-tick window it names, in order, with no tick between actions of one window) and a
//! CHORD: what the player does at the moment that matters (usually: break the old piston and place
//! one facing the bedrock, an instamine left click plus a right click in one client tick).
//! [`scan`] tries the chord in every window of a range, each in a fresh sandbox, and reports for
//! each window whether the bedrock is gone (`broken`), gone only after the `release` actions
//! (a headless piston was made: `headless`), or still there (`fail`). The fraction of windows that
//! work is the odds of a player who presses the chord at a random moment.

pub mod cli;
pub mod machine;
pub mod portal;
pub mod vanilla;
pub mod video;

use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker, WorkerConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

/// One player action. `do`: `break` (survival break: playerWillDestroy, removeBlock, destroy),
/// `place` (a survival placement of `state`, the bridge finds the click), `use` (right click a
/// lever/button/block).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    #[serde(rename = "do")]
    pub what: String,
    pub pos: Pos,
    #[serde(default)]
    pub state: Option<String>,
    /// For script actions: the window (0 = before the first tick after the build settled).
    #[serde(default)]
    pub at: u32,
    /// Vanilla bot only: the block and face the player clicks to place (default: the target
    /// cell itself, the face toward the bot). A torch placed by clicking the top of the block
    /// under it stands; clicking the air cell while looking up makes a wall torch on any side.
    #[serde(default)]
    pub click: Option<Pos>,
    #[serde(default)]
    pub face: Option<String>,
    /// Vanilla bot only: look this way (yaw, pitch) for this click, from the stance's position
    /// (the bot is turned in place between ticks; `legit` checks use this look).
    #[serde(default)]
    pub look: Option<[f32; 2]>,
    /// Repeat this script action `times` times, `every` windows apart (a batch of portal trips).
    #[serde(default)]
    pub every: u32,
    #[serde(default)]
    pub times: u32,
}

impl Scenario {
    /// The script with every repeated action unrolled.
    pub fn expanded_script(&self) -> Vec<Action> {
        let mut out = Vec::new();
        for a in &self.script {
            let n = a.times.max(1);
            for k in 0..n {
                let mut b = a.clone();
                b.at = a.at + k * a.every;
                b.times = 0;
                out.push(b);
            }
        }
        out
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub notes: String,
    /// The bedrock block to remove.
    pub target: Pos,
    /// The build, placed with block updates, then settled `settle` ticks.
    pub blocks: Vec<(i32, i32, i32, String)>,
    #[serde(default = "default_settle")]
    pub settle: u32,
    /// Timed actions (each with its window `at`), the same in every trial.
    #[serde(default)]
    pub script: Vec<Action>,
    /// What the player does in the scanned window, in order.
    pub chord: Vec<Action>,
    /// Done 20 ticks after the chord when the bedrock is still there (turning a headless piston
    /// off at leisure).
    #[serde(default)]
    pub release: Vec<Action>,
    /// Ticks run after the chord before looking.
    #[serde(default = "default_horizon")]
    pub horizon: u32,
    /// More cells that must be gone (not bedrock) too, checked with the target after the run
    /// (batch scenarios: several headless pistons released at once).
    #[serde(default)]
    pub also: Vec<Pos>,
    /// Where the bot stands and looks for the vanilla confirmation (`bedrock vanilla`).
    #[serde(default)]
    pub stance: Option<vanilla::Stance>,
    /// `the_nether` / `the_end`: the build, the target checks and the bot are in that dimension
    /// (the official server's real terrain there, e.g. the Nether roof). Vanilla runs only.
    #[serde(default)]
    pub dimension: Option<String>,
    /// Vanilla runs: commands run before the build (e.g. `forceload` the build's chunks in
    /// another dimension, so the build and the target check can reach them).
    #[serde(default)]
    pub setup: Vec<String>,
    /// Vanilla runs: place observers with `setblock ... strict` (no shape update: a command-placed
    /// observer otherwise fires once as it is placed, which a player's placement never does).
    #[serde(default)]
    pub strict_observers: bool,
    /// Vanilla runs: the bot plays in SURVIVAL (not creative): Haste II, an Efficiency V
    /// netherite pickaxe in the main hand (instamines a piston: speed 49 >= 1.5 x 30), on the
    /// ground; its breaks go through the survival break code.
    #[serde(default)]
    pub survival: bool,
}

fn default_settle() -> u32 {
    40
}
fn default_horizon() -> u32 {
    40
}

impl Scenario {
    pub fn load(path: &Path) -> Result<Scenario> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut sc: Scenario = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        sc.script = sc.expanded_script();
        Ok(sc)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Broken,
    Headless,
    Fail,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowResult {
    pub window: u32,
    pub outcome: Outcome,
    /// The chord's per-action results from the bridge (`ok`, the click it used...).
    pub chord: Vec<Value>,
    /// The block at the chord's first position after the run.
    pub left: String,
}

/// The bridge's `build` op with no ticks between or after the actions: they all land in one
/// inter-tick window, like packets the server handles between two ticks. A `cmd` action (its
/// `state` is a command, e.g. `summon tnt ...` for TNT a player lit earlier) runs through `exec`,
/// in order, also without a tick.
fn act(w: &mut Worker, sandbox: u32, actions: &[&Action]) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < actions.len() {
        if actions[i].what == "cmd" {
            let c = actions[i].state.clone().unwrap_or_default();
            w.call(json!({"op": "exec", "sandbox": sandbox, "command": c}))?;
            out.push(json!({"ok": true, "cmd": c}));
            i += 1;
            continue;
        }
        let mut j = i;
        while j < actions.len() && actions[j].what != "cmd" {
            j += 1;
        }
        out.extend(build_steps(w, sandbox, &actions[i..j])?);
        i = j;
    }
    Ok(out)
}

fn build_steps(w: &mut Worker, sandbox: u32, actions: &[&Action]) -> Result<Vec<Value>> {
    if actions.is_empty() {
        return Ok(Vec::new());
    }
    let steps: Vec<Value> = actions
        .iter()
        .map(|a| {
            let mut s = json!({"do": a.what, "pos": [a.pos[0], a.pos[1], a.pos[2]]});
            if let Some(st) = &a.state {
                s["state"] = json!(st);
            }
            s
        })
        .collect();
    let r = w.call(json!({"op": "build", "sandbox": sandbox, "steps": steps, "min_ticks": 0, "settle": 0, "stop_on_fail": false}))?;
    Ok(r["results"].as_array().cloned().unwrap_or_default())
}

fn block_at(w: &mut Worker, sandbox: u32, p: Pos) -> Result<String> {
    let r = w.get_region(sandbox, p, p)?;
    Ok(r.get(p).unwrap_or("?").to_string())
}

/// Build, settle, then run the script and the chord (in window `chord_at`, or never), then
/// `horizon` ticks. Returns the outcome and the chord's results.
pub fn trial(w: &mut Worker, sandbox: u32, sc: &Scenario, chord_at: Option<u32>) -> Result<WindowResult> {
    w.clear(sandbox)?;
    run_setup(w, sandbox, sc)?;
    let blocks: Vec<(Pos, String)> = sc.blocks.iter().map(|(x, y, z, s)| ([*x, *y, *z], s.clone())).collect();
    w.set_blocks(sandbox, &blocks, PlaceMode::Update)?;
    if sc.settle > 0 {
        w.step(sandbox, sc.settle)?;
    }
    if !block_at(w, sandbox, sc.target)?.starts_with("minecraft:bedrock") {
        bail!("{}: the target {:?} is not bedrock after the build settled", sc.name, sc.target);
    }
    let last_script = sc.script.iter().map(|a| a.at).max().unwrap_or(0);
    let end = last_script.max(chord_at.unwrap_or(0)) + sc.horizon;
    let mut chord_results = Vec::new();
    for window in 0..=end {
        // `probe` actions are the vanilla harness's diagnostics (`bedrock watch --cells` here).
        let mut now: Vec<&Action> = sc.script.iter().filter(|a| a.at == window && a.what != "probe").collect();
        let n_script = now.len();
        if chord_at == Some(window) {
            now.extend(sc.chord.iter());
        }
        let res = act(w, sandbox, &now)?;
        if chord_at == Some(window) {
            chord_results = res[n_script..].to_vec();
        }
        w.step(sandbox, 1)?;
    }
    let gone = |w: &mut Worker, p: Pos| -> Result<bool> { Ok(!block_at(w, sandbox, p)?.starts_with("minecraft:bedrock")) };
    let all_gone = |w: &mut Worker| -> Result<bool> {
        for p in std::iter::once(sc.target).chain(sc.also.iter().copied()) {
            if !gone(w, p)? {
                return Ok(false);
            }
        }
        Ok(true)
    };
    let mut outcome = if all_gone(w)? { Outcome::Broken } else { Outcome::Fail };
    if outcome == Outcome::Fail && !sc.release.is_empty() {
        w.step(sandbox, 20)?;
        let rel: Vec<&Action> = sc.release.iter().collect();
        act(w, sandbox, &rel)?;
        w.step(sandbox, sc.horizon)?;
        if all_gone(w)? {
            outcome = Outcome::Headless;
        }
    }
    let left = match sc.chord.first() {
        Some(a) => block_at(w, sandbox, a.pos)?,
        None => String::new(),
    };
    Ok(WindowResult { window: chord_at.unwrap_or(u32::MAX), outcome, chord: chord_results, left })
}

pub struct ScanReport {
    pub baseline: Outcome,
    pub windows: Vec<WindowResult>,
}

/// What a player's chord would meet at the swap cell in one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Cell {
    /// An extended piston with its retract event pending: break + place works.
    Armed,
    /// Empty with a retract event pending for it: a lone placement works (after TNT).
    OpenArmed,
    /// A moving piston: unbreakable, cannot be placed into (a chord does nothing).
    Moving,
    /// Empty, nothing pending: a placement here leaves a piston that never fires.
    Open,
    /// A piston without a pending retract: a chord breaks it for nothing.
    Harmful,
    /// Anything else.
    Other,
}

impl Cell {
    pub fn letter(self) -> char {
        match self {
            Cell::Armed => 'A',
            Cell::OpenArmed => 'O',
            Cell::Moving => 'm',
            Cell::Open => '.',
            Cell::Harmful => 'x',
            Cell::Other => '?',
        }
    }
}

/// The pending block events: (pos, block id, type).
pub fn pending_events(w: &mut Worker, sandbox: u32) -> Result<Vec<(Pos, String, i64)>> {
    let r = w.call(json!({"op": "events", "sandbox": sandbox}))?;
    let mut out = Vec::new();
    for e in r["events"].as_array().cloned().unwrap_or_default() {
        let p = &e["pos"];
        let pos = [p[0].as_i64().unwrap_or(0) as i32, p[1].as_i64().unwrap_or(0) as i32, p[2].as_i64().unwrap_or(0) as i32];
        out.push((pos, e["block"].as_str().unwrap_or("").to_string(), e["a"].as_i64().unwrap_or(-1)));
    }
    Ok(out)
}

pub fn classify(state: &str, events: &[(Pos, String, i64)], cell: Pos) -> Cell {
    let block = state.split('[').next().unwrap_or("");
    let retract = |b: &str| events.iter().any(|(p, eb, a)| *p == cell && eb == b && (*a == 1 || *a == 2));
    if block == "minecraft:moving_piston" {
        return Cell::Moving;
    }
    if block == "minecraft:air" {
        let any = events.iter().any(|(p, _, a)| *p == cell && (*a == 1 || *a == 2));
        return if any { Cell::OpenArmed } else { Cell::Open };
    }
    if block == "minecraft:piston" || block == "minecraft:sticky_piston" {
        let extended = state.contains("extended=true");
        return if extended && retract(block) { Cell::Armed } else { Cell::Harmful };
    }
    Cell::Other
}

/// One run of the scenario without the chord; the swap cell (the chord's first position) is
/// classified in every window `0..=to` (after that window's script actions, i.e. exactly what a
/// chord pressed in that window would meet). `extra` cells are reported per window too.
pub struct Watch {
    pub cells: Vec<Cell>,
    pub rows: Vec<WatchRow>,
}

pub struct WatchRow {
    pub window: u32,
    /// The swap cell's block and every pending block event.
    pub state: String,
    /// One `x,y,z=block` per extra cell.
    pub extra: Vec<String>,
}

fn short(state: &str) -> String {
    state.replace("minecraft:", "")
}

/// The scenario's setup commands, in the simulator (scenarios without a `dimension`: the sandbox
/// is an overworld-type level), before the build: e.g. a `fill` of the ground the build stands on.
fn run_setup(w: &mut Worker, sandbox: u32, sc: &Scenario) -> Result<()> {
    if sc.dimension.is_some() {
        return Ok(());
    }
    for c in &sc.setup {
        w.call(json!({"op": "exec", "sandbox": sandbox, "command": c}))?;
    }
    Ok(())
}

pub fn watch(version: &str, sc: &Scenario, to: u32, extra: &[Pos]) -> Result<Watch> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let sandbox = w.create(&SandboxOptions::default())?;
    let cell = sc.chord.first().map(|a| a.pos).context("the scenario has no chord")?;
    run_setup(&mut w, sandbox, sc)?;
    let blocks: Vec<(Pos, String)> = sc.blocks.iter().map(|(x, y, z, s)| ([*x, *y, *z], s.clone())).collect();
    w.set_blocks(sandbox, &blocks, PlaceMode::Update)?;
    if sc.settle > 0 {
        w.step(sandbox, sc.settle)?;
    }
    let mut out = Watch { cells: Vec::new(), rows: Vec::new() };
    for window in 0..=to {
        let now: Vec<&Action> = sc.script.iter().filter(|a| a.at == window && a.what != "probe").collect();
        act(&mut w, sandbox, &now)?;
        let state = block_at(&mut w, sandbox, cell)?;
        let events = pending_events(&mut w, sandbox)?;
        out.cells.push(classify(&state, &events, cell));
        let mut extras = Vec::new();
        for p in extra {
            extras.push(format!("{},{},{}={}", p[0], p[1], p[2], short(&block_at(&mut w, sandbox, *p)?)));
        }
        let ev: Vec<String> = events.iter().map(|(p, b, a)| format!("{}@{},{},{} t{a}", short(b), p[0], p[1], p[2])).collect();
        out.rows.push(WatchRow { window, state: format!("{:<36} events[{}]", short(&state), ev.join(" ")), extra: extras });
        w.step(sandbox, 1)?;
    }
    Ok(out)
}

/// The chord in each window of `from..=to`, plus a baseline run without it.
pub fn scan(version: &str, sc: &Scenario, from: u32, to: u32) -> Result<ScanReport> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let sandbox = w.create(&SandboxOptions::default())?;
    let baseline = trial(&mut w, sandbox, sc, None)?.outcome;
    let mut windows = Vec::new();
    for k in from..=to {
        windows.push(trial(&mut w, sandbox, sc, Some(k))?);
    }
    Ok(ScanReport { baseline, windows })
}
