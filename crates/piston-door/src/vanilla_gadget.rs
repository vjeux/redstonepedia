//! `mctas door vanilla-gadget <script.drive>...`: replay gadget drive scripts (designs/gadgets/)
//! in the UNMODIFIED official server and compare every tick with the simulator.
//!
//! A gadget has no door, so `vanilla-check` cannot run it. This tool takes the gadget's own drive
//! script instead: the design is placed in both worlds in its simulator rest state (`/setblock
//! ... strict` in vanilla, the judge's raw placement in the simulator; see `vanilla_check`), then
//! the script's actions are replayed in order:
//! * `use x y z`: the simulator's `use`; in vanilla the bot player (teleported within reach,
//!   touching no block) right-clicks the block (a lever: `LeverBlock.useWithoutItem`, the call the
//!   simulator makes);
//! * `setblock x y z <state>`: the game's `/setblock` command in BOTH worlds (the simulator runs it
//!   through the server's command dispatcher in the sandbox), so ideal inputs are bit-identical;
//! * every stepping command (`step`, `quiet`, `trace`, `expect_trace`, `changes`, `settle`)
//!   steps both worlds one tick at a time; the watch box (the design's bounds + 3, plus every
//!   action cell) is read back after every tick and after every action and compared cell by cell.
//! `set`/`clear` (the simulator's own placement call, no vanilla twin) are rewritten as the game's
//! `/setblock` in both worlds, and the rewritten script's `expect`s are checked by `door drive`
//! (the two placements differ only in the order of shape and neighbor updates; the result line
//! says so). The script's `expect`s are checked by `door drive` itself
//! (run here once at the script's own position); this tool proves vanilla does the same thing at
//! every position.

use crate::vanilla_check::{Oracle, clean, flip, frame_diff, load_box, rest_state, vanilla_entities};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker};
use mctas::vanilla::VanillaServer;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
enum Action {
    Use(Pos),
    SetBlock(Pos, String),
    Step(u32),
    Settle(u32),
}

/// A drive script reduced to what changes the world: its design (and optional offset) and the
/// actions in order.
struct Script {
    design: String,
    offset: Pos,
    actions: Vec<Action>,
}

fn parse(script: &str) -> Result<Script> {
    let mut design = None;
    let mut offset = [0, 0, 0];
    let mut actions = Vec::new();
    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        let line = if line.starts_with('#') { "" } else { line.split(" # ").next().unwrap().trim() };
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> { parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into) };
        let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        match parts[0] {
            "design" => {
                design = Some(parts.get(1).context("design needs a path")?.to_string());
                if parts.len() >= 5 {
                    offset = [int(2)?, int(3)?, int(4)?];
                }
            }
            "use" => actions.push(Action::Use(pos(1)?)),
            "setblock" => actions.push(Action::SetBlock(pos(1)?, parts.get(4).context("setblock needs a state")?.to_string())),
            "step" | "quiet" | "trace" | "changes" => actions.push(Action::Step(int(1)? as u32)),
            "expect_trace" => actions.push(Action::Step(parts.get(2).context("expect_trace needs values")?.chars().count() as u32)),
            "settle" => actions.push(Action::Settle(if parts.len() > 1 { int(1)? as u32 } else { 400 })),
            "set" | "clear" => bail!("line {}: `{}` has no vanilla twin; write ideal inputs with `setblock x y z <state>`", lineno + 1, parts[0]),
            "plane" | "planex" | "probe" | "probe_signal" | "probe_items" | "expect" | "expect_state" | "expect_be" | "reject_be" | "expect_no_be" | "items" | "print" | "box" | "mark" => {}
            other => bail!("line {}: unknown script command {other}", lineno + 1),
        }
    }
    Ok(Script { design: design.context("the script has no `design` line")?, offset, actions })
}

/// Where the bot stands to click `target`: within reach, touching no block of `blocks`, outside
/// the watch box when possible, close to the target (vanilla_check::bot_spot without a door).
fn bot_spot(blocks: &BTreeMap<Pos, String>, target: Pos, wlo: Pos, whi: Pos) -> [f64; 3] {
    let inside = |p: Pos| (0..3).all(|i| p[i] >= wlo[i] && p[i] <= whi[i]);
    let mut best: Option<((i32, i32, i64), Pos)> = None;
    for dx in -4..=4 {
        for dy in -4..=3 {
            for dz in -4..=4 {
                let feet = [target[0] + dx, target[1] + dy, target[2] + dz];
                let head = [feet[0], feet[1] + 1, feet[2]];
                let eye = [feet[0] as f64 + 0.5, feet[1] as f64 + 1.62, feet[2] as f64 + 0.5];
                let dist = ((eye[0] - target[0] as f64 - 0.5).powi(2) + (eye[1] - target[1] as f64 - 0.5).powi(2) + (eye[2] - target[2] as f64 - 0.5).powi(2)).sqrt();
                if dist > 4.0 || blocks.contains_key(&feet) || blocks.contains_key(&head) {
                    continue;
                }
                let mut touching = 0;
                for ox in -1..=1 {
                    for oy in -1..=2 {
                        for oz in -1..=1 {
                            if blocks.contains_key(&[feet[0] + ox, feet[1] + oy, feet[2] + oz]) {
                                touching += 1;
                            }
                        }
                    }
                }
                let score = (touching, if !inside(feet) && !inside(head) { 0 } else { 1 }, (dist * 100.0) as i64);
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((score, feet));
                }
            }
        }
    }
    let p = best.map(|b| b.1).unwrap_or([target[0], target[1] + 2, target[2]]);
    [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5]
}

/// Result of one script at one position.
pub struct GadgetResult {
    pub offset: Pos,
    pub ticks_compared: usize,
    pub divergence: Option<String>,
}

/// Replay one script at one offset in both worlds.
fn run_at(w: &mut Worker, v: &mut VanillaServer, oracle: &mut Oracle, s: &Script, d: &Design, off: Pos, verbose: bool) -> Result<GadgetResult> {
    let mv = |p: Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
    let d = d.translated(off);
    let rest = rest_state(w, &d)?.context("the design never settles to a re-placeable rest state")?;
    let (mut lo, mut hi) = rest.bounds().context("empty design")?;
    for i in 0..3 {
        lo[i] -= 3;
        hi[i] += 3;
    }
    let actions: Vec<Action> = s
        .actions
        .iter()
        .map(|a| match a {
            Action::Use(p) => Action::Use(mv(*p)),
            Action::SetBlock(p, st) => Action::SetBlock(mv(*p), st.clone()),
            other => other.clone(),
        })
        .collect();
    for a in &actions {
        if let Action::Use(p) | Action::SetBlock(p, _) = a {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i] - 2);
                hi[i] = hi[i].max(p[i] + 2);
            }
        }
    }
    // Simulator: the rest state placed raw (it is at rest, so the judge's `self` update would be a
    // no-op), then idle.
    let sb = w.create(&SandboxOptions { chunks: [-4, -4, 3, 3], seed: 0 })?;
    let result = (|| -> Result<GadgetResult> {
        let blocks = rest.block_list();
        w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
        let settle = w.step_until_idle(sb, 100)?;
        if settle.idle != Some(true) {
            bail!("the rest state is not idle in the simulator");
        }
        // Vanilla: wipe the previous position and this box, place the same states strictly.
        load_box(v, lo, hi)?;
        if let Some((plo, phi)) = v.dirty.take() {
            clean(v, plo, phi)?;
        }
        clean(v, lo, hi)?;
        v.dirty = Some((lo, hi));
        let first_use = actions.iter().find_map(|a| if let Action::Use(p) = a { Some(*p) } else { None }).unwrap_or(lo);
        let spot = bot_spot(&rest.blocks, first_use, lo, hi);
        v.teleport_bot(spot[0], spot[1], spot[2])?;
        for (p, st) in &blocks {
            let out = v.cmd(&format!("setblock {} {} {} {st} strict", p[0], p[1], p[2]))?;
            if !out.contains("Changed the block") {
                bail!("setblock {p:?} {st}: {out}");
            }
        }
        if settle.ticked > 0 {
            v.step(settle.ticked)?;
        }
        let mut compared = 0usize;
        let mut tick = 0u32;
        let compare = |w: &mut Worker, v: &mut VanillaServer, oracle: &mut Oracle, label: &str| -> Result<Option<String>> {
            let sim = w.get_region(sb, lo, hi)?;
            let mut van = v.snapshot(lo, hi)?.region;
            oracle.learn(&sim);
            oracle.apply(w, &mut [&mut van])?;
            let diff = frame_diff(&sim, &van, 6);
            Ok((!diff.is_empty()).then(|| format!("{label}: {}", diff.join("; "))))
        };
        if let Some(d) = compare(w, v, oracle, "rest state")? {
            return Ok(GadgetResult { offset: off, ticks_compared: 0, divergence: Some(d) });
        }
        for a in &actions {
            match a {
                Action::Use(p) => {
                    let before = w.get_region(sb, *p, *p)?.get(*p).unwrap_or("?").to_string();
                    w.use_block(sb, *p)?;
                    let spot = bot_spot(&rest.blocks, *p, lo, hi);
                    v.teleport_bot(spot[0], spot[1], spot[2])?;
                    if before.starts_with("minecraft:lever") {
                        flip(v, *p, !before.contains("powered=true"))?;
                    } else {
                        v.bot_use(*p, 1)?;
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        v.cmd("list")?;
                    }
                    if let Some(d) = compare(w, v, oracle, &format!("after use {p:?} (t={tick})"))? {
                        return Ok(GadgetResult { offset: off, ticks_compared: compared, divergence: Some(d) });
                    }
                }
                Action::SetBlock(p, st) => {
                    let cmd = format!("setblock {} {} {} {st}", p[0], p[1], p[2]);
                    w.exec(sb, &cmd)?;
                    let out = v.cmd(&cmd)?;
                    if verbose {
                        println!("    t={tick} {cmd}: {}", out.trim());
                    }
                    if let Some(d) = compare(w, v, oracle, &format!("after {cmd} (t={tick})"))? {
                        return Ok(GadgetResult { offset: off, ticks_compared: compared, divergence: Some(d) });
                    }
                }
                Action::Step(n) | Action::Settle(n) => {
                    // `settle` steps the simulator one tick at a time until it is idle; vanilla
                    // steps as many ticks.
                    let settle = matches!(a, Action::Settle(_));
                    for _ in 0..*n {
                        if settle && w.status(sb)?.idle {
                            break;
                        }
                        w.step(sb, 1)?;
                        v.step(1)?;
                        tick += 1;
                        compared += 1;
                        if let Some(d) = compare(w, v, oracle, &format!("tick {tick}"))? {
                            return Ok(GadgetResult { offset: off, ticks_compared: compared, divergence: Some(d) });
                        }
                    }
                }
            }
        }
        // Then let both worlds run until the simulator is idle (max 60 ticks), still comparing.
        for _ in 0..60 {
            if w.status(sb)?.idle {
                break;
            }
            w.step(sb, 1)?;
            v.step(1)?;
            tick += 1;
            compared += 1;
            if let Some(d) = compare(w, v, oracle, &format!("tick {tick} (after the script)"))? {
                return Ok(GadgetResult { offset: off, ticks_compared: compared, divergence: Some(d) });
            }
        }
        let (se, ve) = (w.entities(sb)?.len() as u32, vanilla_entities(v)?);
        let divergence = (se != ve).then(|| format!("entities at the end: simulator {se}, vanilla {ve}"));
        Ok(GadgetResult { offset: off, ticks_compared: compared, divergence })
    })();
    let _ = w.drop_sandbox(sb);
    result
}

/// The script with the simulator-only `set`/`clear` lines rewritten as the game's `/setblock`
/// (`set x y z s` -> `setblock x y z s`, `clear x y z` -> `setblock x y z minecraft:air`), so both
/// worlds run the same command; `None` when there was nothing to rewrite. The rewritten script's
/// own `expect`s are checked by `door drive` too (the two placements differ only in the order of
/// shape and neighbor updates).
fn rewrite_set(script: &str) -> Option<String> {
    let mut changed = false;
    let out: Vec<String> = script
        .lines()
        .map(|raw| {
            let t = raw.trim_start();
            if let Some(rest) = t.strip_prefix("set ") {
                changed = true;
                format!("setblock {rest}")
            } else if let Some(rest) = t.strip_prefix("clear ") {
                changed = true;
                let coords: Vec<&str> = rest.split_whitespace().take(3).collect();
                format!("setblock {} minecraft:air", coords.join(" "))
            } else {
                raw.to_string()
            }
        })
        .collect();
    changed.then(|| out.join("\n"))
}

/// `door vanilla-gadget`: every script at every offset; prints one line per script.
pub fn run_cli(w: &mut Worker, v: &mut VanillaServer, scripts: &[std::path::PathBuf], offsets: &[Pos], verbose: bool) -> Result<bool> {
    let mut oracle = Oracle::default();
    let mut all_ok = true;
    for path in scripts {
        let t0 = std::time::Instant::now();
        let original = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let rewritten = rewrite_set(&original);
        let text = rewritten.clone().unwrap_or(original);
        let line = (|| -> Result<String> {
            let s = parse(&text)?;
            let drive = crate::drive::run_opts(w, &text, false)?;
            let d = crate::cli::load_design(std::path::Path::new(&s.design))?.translated(s.offset);
            let mut ticks = 0;
            let mut bad = Vec::new();
            for off in offsets {
                let r = run_at(w, v, &mut oracle, &s, &d, *off, verbose).with_context(|| format!("offset {off:?}"))?;
                ticks += r.ticks_compared;
                if let Some(div) = r.divergence {
                    bad.push(format!("offset {:?}: DIVERGES {div}", r.offset));
                }
            }
            let gadget = if drive.failures.is_empty() { format!("drive PASS ({} checks)", drive.checks) } else { format!("drive FAIL: {}", drive.failures.join("; ")) };
            let gadget = if rewritten.is_some() { format!("{gadget}, set/clear replayed as /setblock") } else { gadget };
            if bad.is_empty() && drive.failures.is_empty() {
                Ok(format!("PASS: identical in vanilla at {} positions ({ticks} ticks compared); {gadget}", offsets.len()))
            } else {
                all_ok = false;
                Ok(format!("FAIL: {}; {gadget}", if bad.is_empty() { "vanilla identical".to_string() } else { bad.join(" | ") }))
            }
        })();
        match line {
            Ok(l) => println!("vanilla-gadget: {} {l} [{:.0}s]", path.display(), t0.elapsed().as_secs_f64()),
            Err(e) => {
                all_ok = false;
                println!("vanilla-gadget: {} ERROR: {e:#}", path.display());
            }
        }
    }
    Ok(all_ok)
}
