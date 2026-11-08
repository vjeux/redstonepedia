//! `mctas door vanilla-build <script>`: replay a survival build script (`door buildable`'s
//! `*.build.txt`) in an UNMODIFIED vanilla 26.3 server, placing every block the way a player does,
//! then check the built world against the judge's rest state and run the judge's open/close cycles
//! with the bot (vanilla-check's comparison).
//!
//! Per position (the judge's offsets; the script and the design translated alike):
//! * Simulator: the same script through the bridge's `build` op, ONE action per call, so the
//!   watch box can be read after every action; the op reports the exact click it used (look
//!   direction, hit point, sneaking) and the ticks it let pass (after the temporary dirt, after the
//!   block, before each right-click, after the action). Its click description must equal the
//!   script's line. Then the post-build settle (20 ticks + until idle, as `door buildable`), the
//!   judge's rest state (raw paste, a separate sandbox) and the judge's cycles from the built state.
//! * Vanilla: a bot player does what the script says. Per placement: the item in its main hand
//!   (`/item replace`), a `/tp` that sets where it stands and where it looks (yaw, pitch: what
//!   `getStateForPlacement` reads), sneaking (the `PlayerInput` shift key) when the script says so,
//!   then `use_item_on` on the given face of the given block at the face's centre: the server runs
//!   `ServerPlayerGameMode.useItemOn` -> `BlockItem.place`, exactly a player's click. The bot stands
//!   ON the view ray (eye = hit point minus a few blocks along the look direction, body clear of
//!   every block the build ever occupies, the ray free of blocks, within survival reach 4.5) when
//!   such a spot exists. A temporary dirt block is placed by a click too (against a neighbouring
//!   block, or against an invisible anchor block pasted silently next to it and removed silently
//!   right after when the void offers nothing to click) and broken by the bot (creative
//!   `START_DESTROY_BLOCK` = `playerWillDestroy` + `removeBlock` + `destroy`, the simulator's break).
//!   Right-clicks (repeater delays, the lever while building) are bot clicks with an empty hand.
//!   Time is frozen and stepped exactly as the simulator's build did; after every action vanilla
//!   must be quiet on its own state and its watch box must equal the simulator's.
//! * Then: the built vanilla world vs the judge's rest state, and 3 open/close cycles from there,
//!   compared tick for tick with the simulator's cycles from its built state (vanilla-check).

use crate::buildable::{Action, Step};
use crate::vanilla_check::{CycleRun, Oracle, Run, compare_frames, frame_diff, judge, verdict_str};
use crate::{DoorSpec, watch_box};
use anyhow::{Context, Result, bail};
use mctas::blockstate::{block_name, is_air};
use mctas::design::Design;
use mctas::sim::{Pos, Region, SandboxOptions, Worker};
use mctas::vanilla::VanillaServer;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::Duration;

const FACES: [(&str, [i32; 3]); 6] =
    [("down", [0, -1, 0]), ("up", [0, 1, 0]), ("north", [0, 0, -1]), ("south", [0, 0, 1]), ("west", [-1, 0, 0]), ("east", [1, 0, 0])];

fn face_index(name: &str) -> Option<u8> {
    FACES.iter().position(|(n, _)| *n == name).map(|i| i as u8)
}

fn face_normal(i: u8) -> [i32; 3] {
    FACES[i as usize].1
}

fn add(p: Pos, d: [i32; 3]) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// One numbered line of a build script.
#[derive(Clone, Debug)]
pub struct ScriptStep {
    pub number: usize,
    pub text: String,
    pub step: Step,
    /// The click as written ("click the up face of 1 2 3, looking down", "put a temporary dirt...").
    pub click: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Script {
    pub path: String,
    pub title: String,
    pub source: Option<String>,
    pub steps: Vec<ScriptStep>,
}

fn parse_pos(tokens: &[&str]) -> Result<Pos> {
    anyhow::ensure!(tokens.len() >= 3, "expected x y z, got {tokens:?}");
    Ok([tokens[0].parse()?, tokens[1].parse()?, tokens[2].trim_end_matches([':', ',']).parse()?])
}

/// Read a `door buildable` script: `# source: <design>` and the numbered action lines.
pub fn parse_script(path: &str, text: &str) -> Result<Script> {
    let (mut title, mut source, mut steps) = (String::new(), None, Vec::new());
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(h) = line.strip_prefix('#') {
            let h = h.trim();
            if title.is_empty() {
                title = h.to_string();
            }
            if let Some(s) = h.strip_prefix("source: ") {
                source = Some(s.trim().to_string());
            }
            continue;
        }
        let Some((num, rest)) = line.split_once(". ") else { continue };
        let Ok(number) = num.parse::<usize>() else { continue };
        let mut body = rest.to_string();
        if let Some(i) = body.find("  !! ") {
            bail!("{path} step {number} failed in the simulator when the script was made: {}", &body[i + 5..]);
        }
        if body.ends_with(" ticks]") {
            if let Some(i) = body.rfind(" [things move for ") {
                body.truncate(i);
            }
        }
        let mut note = String::new();
        if body.ends_with(')') {
            if let Some(i) = body.rfind(" (") {
                let inner = &body[i + 2..body.len() - 1];
                if !inner.contains(" degrees toward ") {
                    note = inner.to_string();
                    body.truncate(i);
                }
            }
        }
        let (what, click) = match body.split_once(": ") {
            Some((a, b)) => (a.to_string(), Some(b.to_string())),
            None => (body.clone(), None),
        };
        let t: Vec<&str> = what.split_whitespace().collect();
        let step = match t.first().copied() {
            Some("place") => {
                anyhow::ensure!(t.len() == 6 && t[2] == "at", "{path} step {number}: cannot read `{what}`");
                let state = if t[1].contains(':') { t[1].to_string() } else { format!("minecraft:{}", t[1]) };
                Step { pos: parse_pos(&t[3..6])?, action: Action::Place(state), note }
            }
            Some("right-click") => Step { pos: parse_pos(&t[1..])?, action: Action::Use, note },
            Some("break") => Step { pos: parse_pos(&t[1..])?, action: Action::Break, note },
            _ => bail!("{path} step {number}: unknown action `{what}`"),
        };
        steps.push(ScriptStep { number, text: rest.to_string(), step, click });
    }
    anyhow::ensure!(!steps.is_empty(), "{path}: no build steps");
    Ok(Script { path: path.to_string(), title, source, steps })
}

/// A click: the face of the support block, the hit point on it, the look direction (degrees;
/// the script omits the yaw when looking straight up or down) and sneaking. `scaffold`: the
/// support is a temporary dirt block placed at that position first and broken afterwards.
#[derive(Clone, Debug, PartialEq)]
pub struct Click {
    pub support: Pos,
    pub face: u8,
    pub hit: [f64; 3],
    pub yaw: Option<f32>,
    pub pitch: f32,
    pub sneak: bool,
    pub scaffold: Option<Pos>,
}

impl Click {
    fn translated(&self, o: Pos) -> Click {
        Click {
            support: add(self.support, o),
            hit: [self.hit[0] + o[0] as f64, self.hit[1] + o[1] as f64, self.hit[2] + o[2] as f64],
            scaffold: self.scaffold.map(|q| add(q, o)),
            ..self.clone()
        }
    }
}

fn face_center(p: Pos, face: u8) -> [f64; 3] {
    let n = face_normal(face);
    [p[0] as f64 + 0.5 + 0.5 * n[0] as f64, p[1] as f64 + 0.5 + 0.5 * n[1] as f64, p[2] as f64 + 0.5 + 0.5 * n[2] as f64]
}

fn compass_yaw(word: &str) -> Option<f32> {
    match word {
        "south" => Some(0.0),
        "west" => Some(90.0),
        "north" => Some(180.0),
        "east" => Some(270.0),
        _ => None,
    }
}

/// "looking down" | "looking up" | "facing <dir>[ (<n> degrees toward <dir>)][ looking down|up <p>]"
/// (the bridge's `Click.describe`). Returns (yaw if written, pitch).
fn parse_look(s: &str) -> Result<(Option<f32>, f32)> {
    let s = s.trim();
    match s {
        "looking down" => return Ok((None, 90.0)),
        "looking up" => return Ok((None, -90.0)),
        _ => {}
    }
    let rest = s.strip_prefix("facing ").with_context(|| format!("cannot read the look `{s}`"))?;
    let w: Vec<&str> = rest.split_whitespace().collect();
    let base = w.first().and_then(|c| compass_yaw(c)).with_context(|| format!("cannot read the look `{s}`"))?;
    let mut yaw = base;
    let mut i = 1;
    if w.get(1).is_some_and(|x| x.starts_with('(')) {
        let n: f32 = w[1].trim_start_matches('(').parse()?;
        anyhow::ensure!(w.get(2) == Some(&"degrees") && w.get(3) == Some(&"toward"), "cannot read the look `{s}`");
        let toward = w.get(4).and_then(|x| compass_yaw(x.trim_end_matches(')'))).with_context(|| format!("cannot read the look `{s}`"))?;
        // describe(): toward = fromYRot(base + signum(yaw - base) * 90).
        let plus = ((base + 90.0) % 360.0 - toward).abs() < 0.5;
        yaw = (base + if plus { n } else { -n }).rem_euclid(360.0);
        i = 5;
    }
    let pitch = match &w[i..] {
        [] => 0.0,
        ["looking", "down", p] => p.parse()?,
        ["looking", "up", p] => -p.parse::<f32>()?,
        _ => bail!("cannot read the look `{s}`"),
    };
    Ok((Some(yaw), pitch))
}

/// Parse a click description (see `Click.describe` in Sandbox.java).
pub fn parse_click(s: &str) -> Result<Click> {
    if let Some(rest) = s.strip_prefix("click the ") {
        let (face_word, rest) = rest.split_once(" face of ").with_context(|| format!("cannot read `{s}`"))?;
        let face = face_index(face_word).with_context(|| format!("unknown face in `{s}`"))?;
        let (coords, rest) = rest.split_once(", ").with_context(|| format!("cannot read `{s}`"))?;
        let support = parse_pos(&coords.split_whitespace().collect::<Vec<_>>())?;
        let (look, sneak) = match rest.strip_suffix(", sneaking") {
            Some(l) => (l, true),
            None => (rest, false),
        };
        let (yaw, pitch) = parse_look(look)?;
        Ok(Click { support, face, hit: face_center(support, face), yaw, pitch, sneak, scaffold: None })
    } else if let Some(rest) = s.strip_prefix("put a temporary dirt block at ") {
        let (coords, rest) = rest.split_once(", click its ").with_context(|| format!("cannot read `{s}`"))?;
        let support = parse_pos(&coords.split_whitespace().collect::<Vec<_>>())?;
        let (face_word, rest) = rest.split_once(" face, ").with_context(|| format!("cannot read `{s}`"))?;
        let face = face_index(face_word).with_context(|| format!("unknown face in `{s}`"))?;
        let look = rest.strip_suffix(", then break the dirt").with_context(|| format!("cannot read `{s}`"))?;
        let (yaw, pitch) = parse_look(look)?;
        Ok(Click { support, face, hit: face_center(support, face), yaw, pitch, sneak: false, scaffold: Some(support) })
    } else {
        bail!("cannot read the click `{s}`")
    }
}

/// What the simulator did for one action.
#[derive(Clone, Debug)]
pub struct SimStep {
    pub ok: bool,
    pub error: Option<String>,
    pub describe: Option<String>,
    pub click: Option<Click>,
    /// Ticks after the temporary dirt went in, after the block went in.
    pub scaffold_ticks: Option<(u32, u32)>,
    /// Ticks before each right-click that sets a click-set property.
    pub click_ticks: Vec<u32>,
    /// Ticks after the action (at least 4, then until idle).
    pub ticks: u32,
    /// With `--slow N`: the extra idle ticks, plus however long the world then took to be idle
    /// again (more than N: the slower player saw the build wake up).
    pub slow_ticks: u32,
    pub after: Region,
}

fn sim_click(r: &Value) -> Option<Click> {
    let ints = |v: &Value| -> Option<Pos> {
        let a = v.as_array()?;
        Some([a.first()?.as_i64()? as i32, a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32])
    };
    let support = ints(&r["hit_pos"])?;
    let hl = r["hit_loc"].as_array()?;
    let scaffold = r["scaffold"].as_str().and_then(|s| parse_pos(&s.split_whitespace().collect::<Vec<_>>()).ok());
    Some(Click {
        support,
        face: face_index(r["hit_face"].as_str()?)?,
        hit: [hl.first()?.as_f64()?, hl.get(1)?.as_f64()?, hl.get(2)?.as_f64()?],
        yaw: Some(r["yaw"].as_f64()? as f32),
        pitch: r["pitch"].as_f64()? as f32,
        sneak: r["sneak"].as_bool().unwrap_or(false),
        scaffold,
    })
}

/// The simulator's side of one position.
pub struct SimBuild {
    pub steps: Vec<SimStep>,
    pub run: Run,
    pub entities: usize,
    pub judge_rest: Region,
}

fn step_json(s: &Step) -> Value {
    match &s.action {
        Action::Place(st) => json!({"do": "place", "pos": s.pos, "state": st}),
        Action::Use => json!({"do": "use", "pos": s.pos}),
        Action::Break => json!({"do": "break", "pos": s.pos}),
    }
}

fn sim_build(w: &mut Worker, d: &Design, spec: &DoorSpec, steps: &[Step], opts: &BuildOptions) -> Result<SimBuild> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<SimBuild> {
        let judge_rest = crate::buildable::judge_rest(w, sb, d)?;
        w.clear(sb)?;
        let (wlo, whi) = watch_box(d, spec);
        let mut out = Vec::new();
        for s in steps {
            let v = w.call(json!({"op": "build", "sandbox": sb, "steps": [step_json(s)], "settle": 200, "min_ticks": 4, "stop_on_fail": false}))?;
            let r = &v["results"][0];
            // A slower player: idle ticks after the action; if that wakes something up (a
            // vibration still travelling when the build op called the world idle), until idle.
            let mut slow_ticks = 0;
            if opts.slow > 0 {
                w.step(sb, opts.slow)?;
                slow_ticks = opts.slow;
                if !w.status(sb)?.idle {
                    slow_ticks += w.step_until_idle(sb, 400)?.ticked;
                }
            }
            let ticks_pair = r["scaffold_ticks"].as_array().map(|a| (a[0].as_u64().unwrap_or(0) as u32, a[1].as_u64().unwrap_or(0) as u32));
            out.push(SimStep {
                ok: r["ok"].as_bool().unwrap_or(false),
                error: r["error"].as_str().map(|x| x.to_string()),
                describe: r["click"].as_str().map(|x| x.to_string()),
                click: sim_click(r),
                scaffold_ticks: ticks_pair,
                click_ticks: r["click_ticks"].as_array().map(|a| a.iter().map(|t| t.as_u64().unwrap_or(0) as u32).collect()).unwrap_or_default(),
                ticks: r["ticks"].as_u64().unwrap_or(0) as u32,
                slow_ticks,
                after: w.get_region(sb, wlo, whi)?,
            });
        }
        // As `door buildable`: 20 ticks for vibrations still travelling, then until idle.
        let a = w.step_record(sb, 20, false, wlo, whi)?;
        let b = w.step_record(sb, 400, true, wlo, whi)?;
        let mut settle = a.frames;
        settle.extend(b.frames);
        let rest = w.get_region(sb, wlo, whi)?;
        let entities = w.entities(sb)?.len();
        let mut cycles = Vec::new();
        for _ in 0..opts.cycles {
            let mut c = CycleRun::default();
            w.use_block(sb, spec.lever)?;
            (c.open, c.open_idle, c.open_pending) = crate::vanilla_check::sim_phase(w, sb, opts.max_ticks, wlo, whi, false)?;
            c.open_entities = w.entities(sb)?.len() as u32;
            w.use_block(sb, spec.lever)?;
            (c.close, c.close_idle, c.close_pending) = crate::vanilla_check::sim_phase(w, sb, opts.max_ticks, wlo, whi, false)?;
            c.close_entities = w.entities(sb)?.len() as u32;
            cycles.push(c);
        }
        Ok(SimBuild { steps: out, run: Run { settle, settle_idle: b.idle == Some(true), rest, cycles, settle_pending: Vec::new() }, entities, judge_rest })
    })();
    let _ = w.drop_sandbox(sb);
    r
}

// ---- where the bot stands ----------------------------------------------------------------------

/// A spot for the bot: feet position and look direction.
#[derive(Clone, Debug)]
struct Spot {
    feet: [f64; 3],
    yaw: f32,
    pitch: f32,
    /// The eye is on the view ray and the ray reaches the hit point through air only.
    on_ray: bool,
}

const EYE: f64 = 1.62;
const REACH: f64 = 4.5;

fn solid(world: &Region, p: Pos) -> bool {
    world.get(p).is_some_and(|s| !is_air(s))
}

fn body_clear(feet: [f64; 3], blocked: &BTreeSet<Pos>, margin: f64) -> bool {
    let lo = [feet[0] - 0.3 - margin, feet[1] - margin, feet[2] - 0.3 - margin];
    let hi = [feet[0] + 0.3 + margin, feet[1] + 1.8 + margin, feet[2] + 0.3 + margin];
    for x in lo[0].floor() as i32..=(hi[0] - 1e-9).floor() as i32 {
        for y in lo[1].floor() as i32..=(hi[1] - 1e-9).floor() as i32 {
            for z in lo[2].floor() as i32..=(hi[2] - 1e-9).floor() as i32 {
                if blocked.contains(&[x, y, z]) {
                    return false;
                }
            }
        }
    }
    true
}

/// Distance from the eye to a block's box (the server's reach test, `isWithinBlockInteractionRange`).
fn reach(eye: [f64; 3], p: Pos) -> f64 {
    let mut d2 = 0.0;
    for i in 0..3 {
        let c = eye[i].clamp(p[i] as f64, p[i] as f64 + 1.0);
        d2 += (eye[i] - c).powi(2);
    }
    d2.sqrt()
}

/// The segment eye -> hit crosses only air (cells in `allowed` excepted).
fn ray_clear(eye: [f64; 3], hit: [f64; 3], world: &Region, allowed: &[Pos]) -> bool {
    let d = [hit[0] - eye[0], hit[1] - eye[1], hit[2] - eye[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let n = (len / 0.02).ceil() as usize;
    for i in 0..n {
        let s = i as f64 / n as f64;
        if s * len > len - 0.01 {
            break;
        }
        let c = [(eye[0] + d[0] * s).floor() as i32, (eye[1] + d[1] * s).floor() as i32, (eye[2] + d[2] * s).floor() as i32];
        if !allowed.contains(&c) && solid(world, c) {
            return false;
        }
    }
    true
}

fn look_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = ((yaw as f64).to_radians(), (pitch as f64).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
    let pitch = (-d[1] / len).asin().to_degrees();
    let yaw = (-d[0]).atan2(d[2]).to_degrees();
    ((yaw as f32).rem_euclid(360.0), pitch as f32)
}

/// Off the view ray: any spot within reach of `blocks` whose body is clear (the server checks
/// reach, not sight), preferring a clear line of sight and a short distance.
fn spot_anywhere(hit: [f64; 3], blocks: &[Pos], blocked: &BTreeSet<Pos>, world: &Region, allowed: &[Pos], look: Option<(f32, f32)>) -> Option<Spot> {
    let mut best: Option<(f64, Spot)> = None;
    for dx in -8..=8 {
        for dy in -10..=6 {
            for dz in -8..=8 {
                let feet = [hit[0] + dx as f64 * 0.5, hit[1] + dy as f64 * 0.5, hit[2] + dz as f64 * 0.5];
                let eye = [feet[0], feet[1] + EYE, feet[2]];
                if blocks.iter().any(|b| reach(eye, *b) > REACH) || !body_clear(feet, blocked, 0.05) {
                    continue;
                }
                let sight = ray_clear(eye, hit, world, allowed);
                let dist = ((eye[0] - hit[0]).powi(2) + (eye[1] - hit[1]).powi(2) + (eye[2] - hit[2]).powi(2)).sqrt();
                let score = if sight { 0.0 } else { 100.0 } + dist;
                if best.as_ref().is_none_or(|(b, _)| score < *b) {
                    let (yaw, pitch) = look.unwrap_or_else(|| look_at(eye, hit));
                    best = Some((score, Spot { feet, yaw, pitch, on_ray: false }));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Where to stand for a click with a given look: on the view ray when possible. When not, says
/// why: "sight" (every point of the ray within reach sees a block before the face) or "body" (the
/// ray is open but a standing player's body does not fit around its eye there).
fn spot_for_look(hit: [f64; 3], yaw: f32, pitch: f32, blocks: &[Pos], blocked: &BTreeSet<Pos>, world: &Region, allowed: &[Pos]) -> Option<(Spot, Option<&'static str>)> {
    let v = look_vector(yaw, pitch);
    let mut open_ray = false;
    for margin in [0.2, 0.05] {
        for t in [2.5, 2.2, 2.8, 2.0, 3.0, 1.8, 3.2, 3.4, 1.6, 3.6, 3.8, 4.0, 1.4, 4.2, 1.2, 1.0, 0.8] {
            let eye = [hit[0] - v[0] * t, hit[1] - v[1] * t, hit[2] - v[2] * t];
            let feet = [eye[0], eye[1] - EYE, eye[2]];
            if blocks.iter().any(|b| reach(eye, *b) > REACH) || !ray_clear(eye, hit, world, allowed) || solid(world, [eye[0].floor() as i32, eye[1].floor() as i32, eye[2].floor() as i32]) {
                continue;
            }
            open_ray = true;
            if body_clear(feet, blocked, margin) {
                return Some((Spot { feet, yaw, pitch, on_ray: true }, None));
            }
        }
    }
    spot_anywhere(hit, blocks, blocked, world, allowed, Some((yaw, pitch))).map(|s| (s, Some(if open_ray { "body" } else { "sight" })))
}

/// Where to stand for a click with a free look (a temporary block, a right-click, a break): in
/// front of the face when there is one, looking at the hit point.
fn spot_free(hit: [f64; 3], face: Option<u8>, blocks: &[Pos], blocked: &BTreeSet<Pos>, world: &Region, allowed: &[Pos]) -> Option<Spot> {
    let mut dirs: Vec<[f64; 3]> = Vec::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if (x, y, z) == (0, 0, 0) {
                    continue;
                }
                if let Some(f) = face {
                    let n = face_normal(f);
                    if x * n[0] + y * n[1] + z * n[2] <= 0 {
                        continue;
                    }
                }
                let l = ((x * x + y * y + z * z) as f64).sqrt();
                dirs.push([x as f64 / l, y as f64 / l, z as f64 / l]);
            }
        }
    }
    // Straight out of the face first, then the diagonals.
    if let Some(f) = face {
        let n = face_normal(f);
        dirs.sort_by(|a, b| {
            let da = a[0] * n[0] as f64 + a[1] * n[1] as f64 + a[2] * n[2] as f64;
            let db = b[0] * n[0] as f64 + b[1] * n[1] as f64 + b[2] * n[2] as f64;
            db.partial_cmp(&da).unwrap()
        });
    }
    for margin in [0.2, 0.05] {
        for t in [2.5, 2.0, 3.0, 3.5, 1.6] {
            for dir in &dirs {
                let eye = [hit[0] + dir[0] * t, hit[1] + dir[1] * t, hit[2] + dir[2] * t];
                let feet = [eye[0], eye[1] - EYE, eye[2]];
                if blocks.iter().any(|b| reach(eye, *b) > REACH) || !body_clear(feet, blocked, margin) || !ray_clear(eye, hit, world, allowed) {
                    continue;
                }
                let (yaw, pitch) = look_at(eye, hit);
                return Some(Spot { feet, yaw, pitch, on_ray: true });
            }
        }
    }
    spot_anywhere(hit, blocks, blocked, world, allowed, None)
}

// ---- the vanilla side --------------------------------------------------------------------------

/// The item a player holds to place a block state.
fn item_for(state: &str) -> String {
    let name = block_name(state).trim_start_matches("minecraft:");
    let item = match name {
        "redstone_wire" => "redstone",
        "redstone_wall_torch" => "redstone_torch",
        "wall_torch" => "torch",
        "soul_wall_torch" => "soul_torch",
        "tripwire" => "string",
        n => n,
    };
    format!("minecraft:{item}")
}

/// Counters and notes of one vanilla build.
#[derive(Default, Debug)]
pub struct BuildStats {
    pub clicks_on_ray: usize,
    pub clicks_off_ray: usize,
    /// Script step numbers whose click had no open spot on the view ray, with why ("sight": the
    /// ray meets a block first; "body": a standing player does not fit there). The bot clicked
    /// from elsewhere within reach: the server checks reach, not sight.
    pub off_ray_steps: Vec<String>,
    pub dirt_on_block: usize,
    pub dirt_on_anchor: usize,
    /// Game ticks stepped during the build (on the simulator's schedule).
    pub build_ticks: usize,
    pub notes: Vec<String>,
}

struct Ctx<'a> {
    v: &'a mut VanillaServer,
    w: &'a mut Worker,
    oracle: &'a mut Oracle,
    wlo: Pos,
    whi: Pos,
    /// Cells the bot's body keeps clear of during the current action.
    blocked: BTreeSet<Pos>,
    stats: BuildStats,
    /// The script step being replayed (for notes).
    current_step: usize,
    /// The game runs unfrozen at 20 tps while the bot builds.
    real_time: bool,
    /// Extra idle ticks after every action (`--slow`).
    slow: u32,
    /// Move the bot far away while the world ticks after each action (default; `--stay` keeps
    /// it at its click spot like a real player standing by).
    park: bool,
}

/// Extra game ticks a real-time build waits after each action, on top of the simulator's.
const REAL_TIME_MARGIN: i64 = 10;

/// Wait (the game running) until `n` more game ticks have passed.
fn wait_game_ticks(v: &mut VanillaServer, n: i64) -> Result<()> {
    let t0 = v.game_time()?;
    let started = std::time::Instant::now();
    while v.game_time()? < t0 + n {
        if started.elapsed() > Duration::from_secs(120) {
            bail!("the game time does not advance (is the server frozen?)");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

impl Ctx<'_> {
    fn bot(&mut self) -> Result<&mut mctas::vanilla::bot::Bot> {
        self.v.bot.as_mut().context("no bot")
    }

    fn go(&mut self, s: &Spot) -> Result<()> {
        self.v.teleport_bot_to(s.feet[0], s.feet[1], s.feet[2], Some((s.yaw, s.pitch)))
    }

    /// The simulator has no player while it ticks: wait far above the build, out of every sculk
    /// sensor's range (8 blocks), touching nothing (unless `--stay`).
    fn park_bot(&mut self) -> Result<()> {
        if !self.park {
            return Ok(());
        }
        let (lo, hi) = (self.wlo, self.whi);
        let x = (lo[0] + hi[0]) as f64 / 2.0 + 0.5;
        let z = (lo[2] + hi[2]) as f64 / 2.0 + 0.5;
        self.v.teleport_bot_to(x, hi[1] as f64 + 12.0, z, Some((0.0, 0.0)))
    }

    fn park_bot_if(&mut self, ticks: u32) -> Result<()> {
        if ticks > 0 { self.park_bot() } else { Ok(()) }
    }

    fn ack(&mut self, seq: i32) -> Result<()> {
        self.bot()?.wait_ack(seq, Duration::from_secs(20))
    }

    /// Read the watch box (full state names, simulator flags) and whether vanilla is quiet.
    fn read(&mut self) -> Result<(Region, bool)> {
        let (r, quiet, _) = self.read_pending()?;
        Ok((r, quiet))
    }

    /// Like `read`, plus what is still pending (scheduled ticks with their delay, moving pistons).
    fn read_pending(&mut self) -> Result<(Region, bool, String)> {
        let snap = self.v.snapshot(self.wlo, self.whi)?;
        let busy = !snap.block_ticks.is_empty() || snap.fluid_ticks > 0 || snap.moving_pistons() > 0;
        let falling = self.v.count_entities("@e[type=minecraft:falling_block]")?;
        let quiet = !busy && falling == 0;
        let ticks: Vec<String> = snap.block_ticks.iter().map(|(p, id, t)| format!("{p:?} {} in {t}", id.trim_start_matches("minecraft:"))).collect();
        let pending = format!("block ticks [{}], moving pistons {}, falling blocks {falling}", ticks.join(", "), snap.moving_pistons());
        let mut r = snap.region;
        self.oracle.apply(self.w, &mut [&mut r])?;
        Ok((r, quiet, pending))
    }

    fn step(&mut self, n: u32) -> Result<()> {
        if self.real_time {
            // The game runs on its own (20 tps): wait the simulator's ticks plus a margin (none
            // when the simulator acted at once: the bot then acts as fast as it can).
            self.stats.build_ticks += n as usize;
            return if n > 0 { wait_game_ticks(self.v, n as i64 + REAL_TIME_MARGIN) } else { Ok(()) };
        }
        if n > 0 {
            self.v.step(n)?;
            self.stats.build_ticks += n as usize;
        }
        Ok(())
    }

    /// Right-click a block with an empty hand (a lever, a repeater's delay), like the simulator's
    /// `use` (face UP, hit at the block centre).
    fn use_block(&mut self, p: Pos, world: &Region) -> Result<()> {
        let centre = [p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5];
        let mut blocked = self.blocked.clone();
        blocked.insert(p);
        let spot = spot_free(centre, None, &[p], &blocked, world, &[p]).context("no spot within reach to right-click")?;
        self.v.set_bot_main_hand(None)?;
        self.go(&spot)?;
        let seq = self.bot()?.use_item_on_at(p, 1, [0.5, 0.5, 0.5])?;
        self.ack(seq)
    }

    /// Break a block like a creative player (no drops), the simulator's `breakLikePlayer`.
    fn break_block(&mut self, p: Pos, world: &Region) -> Result<()> {
        let centre = [p[0] as f64 + 0.5, p[1] as f64 + 0.5, p[2] as f64 + 0.5];
        let mut blocked = self.blocked.clone();
        blocked.insert(p);
        let spot = spot_free(centre, None, &[p], &blocked, world, &[p]).context("no spot within reach to break")?;
        self.v.set_bot_main_hand(None)?;
        self.go(&spot)?;
        let seq = self.bot()?.start_destroy(p, 1)?;
        self.ack(seq)
    }

    /// Place the temporary dirt block at `q` with a click: against a neighbouring block when there
    /// is one (sneaking, so it is never used instead), else against an anchor block pasted
    /// silently (strict setblock: no updates, no game event) next to `q` where nothing touches it,
    /// removed silently right after the click.
    fn place_dirt(&mut self, q: Pos, target: Pos, world: &Region) -> Result<()> {
        let mut blocked = self.blocked.clone();
        blocked.insert(q);
        blocked.insert(target);
        let mut anchor = None;
        let mut choice: Option<(Pos, u8, Spot, bool)> = None;
        for (_, d) in FACES.iter() {
            let n = add(q, *d);
            if n == target || !solid(world, n) {
                continue;
            }
            // Click n's face that looks at q.
            let face = FACES.iter().position(|(_, e)| *e == [-d[0], -d[1], -d[2]]).unwrap() as u8;
            let hit = face_center(n, face);
            if let Some(s) = spot_free(hit, Some(face), &[n, q], &blocked, world, &[n]) {
                choice = Some((n, face, s, true));
                break;
            }
        }
        if choice.is_none() {
            // Behind the dirt (away from the target) first, then below, then the rest.
            let away = [q[0] - target[0], q[1] - target[1], q[2] - target[2]];
            let mut dirs: Vec<[i32; 3]> = vec![away, [0, -1, 0]];
            dirs.extend(FACES.iter().map(|f| f.1));
            for d in dirs {
                let a = add(q, d);
                if a == target || solid(world, a) || FACES.iter().any(|(_, e)| add(a, *e) != q && solid(world, add(a, *e))) {
                    continue;
                }
                let face = FACES.iter().position(|(_, e)| *e == [-d[0], -d[1], -d[2]]).unwrap() as u8;
                let mut b2 = blocked.clone();
                b2.insert(a);
                if let Some(s) = spot_free(face_center(a, face), Some(face), &[a, q], &b2, world, &[a]) {
                    anchor = Some(a);
                    choice = Some((a, face, s, false));
                    break;
                }
            }
        }
        let (support, face, spot, sneak) = choice.with_context(|| format!("nothing to click to put the temporary dirt at {q:?}"))?;
        if let Some(a) = anchor {
            let out = self.v.cmd(&format!("setblock {} {} {} minecraft:glass strict", a[0], a[1], a[2]))?;
            anyhow::ensure!(out.contains("Changed the block"), "anchor setblock: {out}");
            self.stats.dirt_on_anchor += 1;
        } else {
            self.stats.dirt_on_block += 1;
        }
        self.v.set_bot_main_hand(Some("minecraft:dirt"))?;
        self.go(&spot)?;
        if sneak {
            self.bot()?.set_sneaking(true)?;
        }
        let hit = face_center(support, face);
        let cursor = [(hit[0] - support[0] as f64) as f32, (hit[1] - support[1] as f64) as f32, (hit[2] - support[2] as f64) as f32];
        let seq = self.bot()?.use_item_on_at(support, face, cursor)?;
        self.ack(seq)?;
        if sneak {
            self.bot()?.set_sneaking(false)?;
        }
        if let Some(a) = anchor {
            let out = self.v.cmd(&format!("setblock {} {} {} minecraft:air strict", a[0], a[1], a[2]))?;
            anyhow::ensure!(out.contains("Changed the block"), "anchor removal: {out}");
        }
        anyhow::ensure!(self.v.test_block(q, "minecraft:dirt")?, "the bot's click did not put the temporary dirt at {q:?}");
        Ok(())
    }

    /// The bot's click that places `state` at `target` (the item in hand, the look, sneaking).
    fn place(&mut self, state: &str, target: Pos, click: &Click, yaw: f32, world: &Region) -> Result<()> {
        let mut blocked = self.blocked.clone();
        blocked.insert(target);
        let (spot, why) = spot_for_look(click.hit, yaw, click.pitch, &[click.support, target], &blocked, world, &[click.support])
            .context("no spot within reach for the click")?;
        if spot.on_ray {
            self.stats.clicks_on_ray += 1;
        } else {
            self.stats.clicks_off_ray += 1;
            self.stats.off_ray_steps.push(format!("{} ({})", self.current_step, why.unwrap_or("?")));
        }
        self.v.set_bot_main_hand(Some(&item_for(state)))?;
        self.go(&spot)?;
        if click.sneak {
            self.bot()?.set_sneaking(true)?;
        }
        let s = click.support;
        let cursor = [(click.hit[0] - s[0] as f64) as f32, (click.hit[1] - s[1] as f64) as f32, (click.hit[2] - s[2] as f64) as f32];
        let seq = self.bot()?.use_item_on_at(s, click.face, cursor)?;
        self.ack(seq)?;
        if click.sneak {
            self.bot()?.set_sneaking(false)?;
        }
        self.v.set_bot_main_hand(None)?;
        Ok(())
    }
}

/// One action, on the simulator's schedule. Returns extra ticks vanilla needed to be quiet.
fn vanilla_action(ctx: &mut Ctx, step: &Step, click: Option<(&Click, f32)>, sim: &SimStep, world: &Region) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    match &step.action {
        Action::Place(state) => {
            let (c, yaw) = click.context("a placement without a click")?;
            if let Some(q) = c.scaffold {
                let (a, b) = sim.scaffold_ticks.context("the simulator used no temporary dirt here")?;
                ctx.place_dirt(q, step.pos, world)?;
                ctx.park_bot_if(a)?;
                ctx.step(a)?;
                let (now, quiet) = ctx.read()?;
                if !quiet {
                    notes.push(format!("vanilla still busy {a} ticks after the temporary dirt (the simulator was idle)"));
                }
                ctx.place(state, step.pos, c, yaw, &now)?;
                ctx.park_bot_if(b)?;
                ctx.step(b)?;
                let (now, quiet) = ctx.read()?;
                if !quiet {
                    notes.push(format!("vanilla still busy {b} ticks after the block (the simulator was idle)"));
                }
                ctx.break_block(q, &now)?;
            } else {
                ctx.place(state, step.pos, c, yaw, world)?;
            }
            for &t in &sim.click_ticks {
                ctx.park_bot_if(t)?;
                ctx.step(t)?;
                let (now, _) = ctx.read()?;
                ctx.use_block(step.pos, &now)?;
            }
        }
        Action::Use => ctx.use_block(step.pos, world)?,
        Action::Break => ctx.break_block(step.pos, world)?,
    }
    if ctx.park {
        ctx.park_bot()?;
    }
    ctx.step(sim.ticks)?;
    if sim.slow_ticks > 0 {
        ctx.step(sim.slow_ticks)?;
        if sim.slow_ticks > ctx.slow {
            // Not a divergence (vanilla follows the same schedule): a fact about the script.
            let n = ctx.current_step;
            ctx.stats.notes.push(format!(
                "step {n}: slower player: the simulator itself woke up again during the {} idle ticks after this action and needed {} ticks in all",
                ctx.slow, sim.slow_ticks
            ));
        }
    }
    Ok(notes)
}

/// Result of one position.
#[derive(Debug)]
pub struct PositionResult {
    pub offset: Pos,
    pub steps: usize,
    /// First action after which vanilla's watch box differs from the simulator's.
    pub step_divergence: Option<String>,
    pub steps_diverging: usize,
    /// The built vanilla world vs the judge's rest state.
    pub built_vs_rest: Option<String>,
    pub sim_built_vs_rest: Option<String>,
    pub sim: std::result::Result<Vec<(u32, u32)>, String>,
    pub vanilla: std::result::Result<Vec<(u32, u32)>, String>,
    pub divergence: Option<String>,
    pub ticks_compared: usize,
    pub stats: BuildStats,
}

impl PositionResult {
    pub fn pass(&self) -> bool {
        self.step_divergence.is_none() && self.built_vs_rest.is_none() && self.sim_built_vs_rest.is_none() && self.divergence.is_none() && self.vanilla.is_ok() && self.sim.is_ok()
    }
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub cycles: u32,
    pub offsets: Vec<Pos>,
    pub max_ticks: u32,
    /// Replay only the first N actions (a minimal repro), then compare and stop.
    pub until_step: Option<usize>,
    pub verbose: bool,
    /// Build with the game running at 20 tps (unfrozen), waiting the simulator's ticks plus a
    /// margin after each action; the cycles still run frozen, tick for tick.
    pub real_time: bool,
    /// A slower player: this many extra idle ticks after every action, in BOTH worlds (stepped).
    pub slow: u32,
    /// Keep the bot at its click spot while the world ticks (a player standing by: near a sculk
    /// sensor it is a vibration source the simulator does not have). Default: it waits far above
    /// the build, out of every sensor's range, as the simulator has no player at all.
    pub stay: bool,
}

fn regions_cells(r: &Region, into: &mut BTreeSet<Pos>) {
    for (p, s) in r.iter() {
        if !is_air(s) {
            into.insert(p);
        }
    }
}

/// Replay `script` at one offset in both worlds and compare.
fn check_at(w: &mut Worker, v: &mut VanillaServer, oracle: &mut Oracle, d: &Design, script: &Script, off: Pos, opts: &BuildOptions) -> Result<PositionResult> {
    let dt = d.translated(off);
    let spec = DoorSpec::from_design(&dt)?;
    let n = opts.until_step.unwrap_or(script.steps.len()).min(script.steps.len());
    let steps: Vec<Step> = script.steps[..n].iter().map(|s| Step { pos: add(s.step.pos, off), ..s.step.clone() }).collect();
    let sim = sim_build(w, &dt, &spec, &steps, opts)?;
    let (wlo, whi) = watch_box(&dt, &spec);
    // The clicks: the script's, checked against the simulator's (which also gives the yaw the
    // script leaves out when looking straight up or down, and the exact hit point).
    let mut clicks: Vec<Option<(Click, f32)>> = Vec::new();
    let mut notes = Vec::new();
    for (s, ss) in script.steps[..n].iter().zip(sim.steps.iter()) {
        if !ss.ok {
            notes.push(format!("step {}: the simulator's placement failed now: {}", s.number, ss.error.clone().unwrap_or_default()));
        }
        let Some(text) = &s.click else {
            clicks.push(None);
            continue;
        };
        let parsed = parse_click(text).with_context(|| format!("step {}", s.number))?.translated(off);
        // Same click as the simulator's (its text carries translated coordinates).
        let same_text = ss.describe.as_deref().and_then(|t| parse_click(t).ok()).is_some_and(|c| c == parsed);
        if !same_text {
            notes.push(format!("step {}: the script says `{text}`, the simulator now clicks `{}`", s.number, ss.describe.clone().unwrap_or_default()));
        }
        let yaw = match (parsed.yaw, &ss.click) {
            (Some(y), _) => y,
            (None, Some(c)) if same_text => c.yaw.unwrap_or(0.0),
            _ => bail!("step {}: the script does not say the yaw (`{text}`) and the simulator clicks differently", s.number),
        };
        let mut c = parsed.clone();
        if let (true, Some(sc)) = (same_text, &ss.click) {
            // Same click: take the simulator's exact hit point (half blocks are hit at 0.25/0.75).
            anyhow::ensure!(sc.support == c.support && sc.face == c.face && sc.sneak == c.sneak && sc.scaffold == c.scaffold, "step {}: script and simulator clicks disagree", s.number);
            c.hit = sc.hit;
        }
        clicks.push(Some((c, yaw)));
    }
    // Vanilla: clean, then the bot builds.
    crate::vanilla_check::load_box(v, wlo, whi)?;
    if let Some((plo, phi)) = v.dirty.take() {
        crate::vanilla_check::clean(v, plo, phi)?;
    }
    crate::vanilla_check::clean(v, wlo, whi)?;
    v.dirty = Some((wlo, whi));
    let mut machine = BTreeSet::new();
    for s in &sim.steps {
        regions_cells(&s.after, &mut machine);
    }
    for f in sim.run.settle.iter().chain(sim.run.cycles.iter().flat_map(|c| c.open.iter().chain(c.close.iter()))) {
        regions_cells(f, &mut machine);
    }
    regions_cells(&sim.judge_rest, &mut machine);
    v.set_bot_main_hand(None)?;
    let mut ctx = Ctx { v, w, oracle, wlo, whi, blocked: BTreeSet::new(), stats: BuildStats::default(), current_step: 0, real_time: opts.real_time, slow: opts.slow, park: !opts.stay };
    ctx.stats.notes = notes;
    if opts.real_time {
        ctx.v.cmd_ok("tick rate 20")?;
        ctx.v.cmd_ok("tick unfreeze")?;
    }
    let (mut world, _) = ctx.read()?;
    let mut step_divergence = None;
    let mut steps_diverging = 0;
    for (k, step) in steps.iter().enumerate() {
        let ss = &sim.steps[k];
        // The bot's body keeps clear of what is there before and after the action; when the
        // action sets something in motion, of every cell the machine ever sweeps too.
        let mut blocked = BTreeSet::new();
        regions_cells(&world, &mut blocked);
        regions_cells(&ss.after, &mut blocked);
        let others_changed = world.iter().any(|(p, s)| p != step.pos && ss.after.get(p) != Some(s));
        let busy = ss.ticks > 4 || ss.scaffold_ticks.is_some_and(|(a, b)| a > 4 || b > 4) || !ss.click_ticks.is_empty();
        if others_changed || busy {
            blocked.extend(machine.iter().copied());
        }
        ctx.blocked = blocked;
        ctx.current_step = script.steps[k].number;
        let click = clicks[k].as_ref().map(|(c, y)| (c, *y));
        let label = format!("step {} `{}`", script.steps[k].number, script.steps[k].text);
        let mut step_notes = vanilla_action(&mut ctx, step, click, ss, &world).with_context(|| format!("vanilla, {label}"))?;
        let (mut now, mut quiet, first_pending) = ctx.read_pending()?;
        if !quiet {
            // Experiments: MCTAS_VB_PROBE="x,y,z;..." prints those blocks' NBT (a sculk sensor's
            // listener: event, source, distance) when vanilla is busy where the simulator is not.
            if let Ok(spec) = std::env::var("MCTAS_VB_PROBE") {
                for p in spec.split(';').filter(|s| !s.is_empty()) {
                    if p == "bot" {
                        let name = ctx.v.bot.as_ref().map(|b| b.name.clone()).unwrap_or_default();
                        for key in ["Pos", "Motion", "OnGround", "fall_distance", "abilities"] {
                            println!("    probe after step {}: bot {key}: {}", script.steps[k].number, ctx.v.cmd(&format!("data get entity {name} {key}"))?);
                        }
                        continue;
                    }
                    let c: Vec<i32> = p.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                    if c.len() == 3 {
                        let (x, y, z) = (c[0] + off[0], c[1] + off[1], c[2] + off[2]);
                        println!("    probe after step {}: {}", script.steps[k].number, ctx.v.cmd(&format!("data get block {x} {y} {z}"))?);
                    }
                }
            }
        }
        let mut extra = 0;
        while !quiet && extra < 200 {
            ctx.step(1)?;
            extra += 1;
            (now, quiet) = ctx.read()?;
        }
        if extra > 0 {
            step_notes.push(format!("vanilla needed {extra} more ticks to be quiet after the simulator's {} (pending then: {first_pending})", ss.ticks));
        }
        let diff = frame_diff(&ss.after, &now, 6);
        if !diff.is_empty() || !step_notes.is_empty() {
            steps_diverging += 1;
            if step_divergence.is_none() {
                step_divergence = Some(format!("after {label}: {}", diff.into_iter().chain(step_notes).collect::<Vec<_>>().join("; ")));
            }
        }
        world = now;
    }
    let stats = std::mem::take(&mut ctx.stats);
    drop(ctx);
    let target = |frames: &[Region], idle: bool| idle.then_some(frames.len());
    let (mut settle, settle_idle) = if opts.real_time {
        // Let it run past the simulator's post-build settle, then freeze and read it once.
        wait_game_ticks(v, sim.run.settle.len() as i64 + REAL_TIME_MARGIN)?;
        v.cmd_ok("tick freeze")?;
        if v.tick_rate != 20 {
            v.cmd_ok(&format!("tick rate {}", v.tick_rate))?;
        }
        let snap = v.snapshot(wlo, whi)?;
        let quiet = snap.block_ticks.is_empty() && snap.fluid_ticks == 0 && snap.moving_pistons() == 0;
        (vec![snap.region], quiet)
    } else {
        let (f, idle, _) = crate::vanilla_check::vanilla_record(v, wlo, whi, opts.max_ticks + 20, target(&sim.run.settle, sim.run.settle_idle))?;
        (f, idle)
    };
    let rest = settle.last().cloned().context("no frames")?;
    let built_entities = crate::vanilla_check::vanilla_entities(v)?;
    let mut van = Run { settle: std::mem::take(&mut settle), settle_idle, rest, cycles: Vec::new(), settle_pending: Vec::new() };
    if n == script.steps.len() {
        let spot = crate::vanilla_check::bot_spot(&dt, &spec, wlo, whi);
        v.teleport_bot(spot[0], spot[1], spot[2])?;
        for i in 0..opts.cycles as usize {
            let s = sim.run.cycles.get(i);
            let mut c = CycleRun::default();
            crate::vanilla_check::flip(v, spec.lever)?;
            let t = s.and_then(|s| target(&s.open, s.open_idle));
            (c.open, c.open_idle, c.open_pending) = crate::vanilla_check::vanilla_record(v, wlo, whi, opts.max_ticks, t)?;
            c.open_entities = crate::vanilla_check::vanilla_entities(v)?;
            crate::vanilla_check::flip(v, spec.lever)?;
            let t = s.and_then(|s| target(&s.close, s.close_idle));
            (c.close, c.close_idle, c.close_pending) = crate::vanilla_check::vanilla_record(v, wlo, whi, opts.max_ticks, t)?;
            c.close_entities = crate::vanilla_check::vanilla_entities(v)?;
            van.cycles.push(c);
        }
    }
    crate::vanilla_check::fill_flags(w, oracle, &sim.run, &mut van)?;
    // A real-time build has no tick-exact settle frames: only its final state is compared.
    let mut divergence = if opts.real_time {
        let d = frame_diff(sim.run.settle.last().unwrap_or(&sim.run.rest), &van.rest, 6);
        (!d.is_empty() || !van.settle_idle).then(|| format!("after the real-time build: {}{}", d.join("; "), if van.settle_idle { "" } else { " (still busy)" }))
    } else {
        compare_frames("post-build settle", &sim.run.settle, &van.settle, sim.run.settle_idle, van.settle_idle)
    };
    if divergence.is_none() && built_entities as usize != sim.entities {
        divergence = Some(format!("after the build: entities sim {} / vanilla {built_entities}", sim.entities));
    }
    let mut ticks = sim.run.settle.len().min(van.settle.len());
    for (i, (s, vc)) in sim.run.cycles.iter().zip(van.cycles.iter()).enumerate() {
        if divergence.is_some() {
            break;
        }
        divergence = compare_frames(&format!("cycle {i} open"), &s.open, &vc.open, s.open_idle, vc.open_idle)
            .or_else(|| (s.open_entities != vc.open_entities).then(|| format!("cycle {i} open: entities sim {} / vanilla {}", s.open_entities, vc.open_entities)))
            .or_else(|| compare_frames(&format!("cycle {i} close"), &s.close, &vc.close, s.close_idle, vc.close_idle))
            .or_else(|| (s.close_entities != vc.close_entities).then(|| format!("cycle {i} close: entities sim {} / vanilla {}", s.close_entities, vc.close_entities)));
        ticks += s.open.len().min(vc.open.len()) + s.close.len().min(vc.close.len());
    }
    let rest_diff = |a: &Region, b: &Region| {
        let d = frame_diff(a, b, 6);
        (!d.is_empty()).then(|| d.join("; ").replace("sim ", "rest ").replace("/ vanilla ", "/ built "))
    };
    let full = n == script.steps.len();
    let (sim_verdict, van_verdict) = if full {
        (judge(&sim.run, &spec).map_err(|e| format!("{e:#}")), judge(&van, &spec).map_err(|e| format!("{e:#}")))
    } else {
        (Err(format!("stopped after step {n}")), Err(format!("stopped after step {n}")))
    };
    Ok(PositionResult {
        offset: off,
        steps: n,
        step_divergence,
        steps_diverging,
        built_vs_rest: if full { rest_diff(&sim.judge_rest, &van.rest) } else { None },
        sim_built_vs_rest: if full { rest_diff(&sim.judge_rest, &sim.run.rest) } else { None },
        sim: sim_verdict,
        vanilla: van_verdict,
        divergence,
        ticks_compared: ticks,
        stats,
    })
}

/// Check one script at every offset.
pub fn check(w: &mut Worker, v: &mut VanillaServer, oracle: &mut Oracle, d: &Design, script: &Script, opts: &BuildOptions) -> Result<Vec<PositionResult>> {
    let mut out = Vec::new();
    for off in &opts.offsets {
        let r = check_at(w, v, oracle, d, script, *off, opts);
        if opts.real_time {
            // Never leave the server running after a failed real-time build.
            let _ = v.cmd("tick freeze");
            let rate = v.tick_rate;
            let _ = v.cmd(&format!("tick rate {rate}"));
        }
        let r = r.with_context(|| format!("offset {off:?}"))?;
        if opts.verbose || !r.pass() {
            print_position(&r);
        }
        out.push(r);
    }
    Ok(out)
}

pub fn print_position(r: &PositionResult) {
    let s = &r.stats;
    println!(
        "  offset {:?}: {} actions ({} build ticks), {} clicks on the view ray, {} off it, temporary dirt {} against a block / {} against an anchor | {} | built {} | sim {} | vanilla {} | {} ticks compared{}",
        r.offset,
        r.steps,
        s.build_ticks,
        s.clicks_on_ray,
        s.clicks_off_ray,
        s.dirt_on_block,
        s.dirt_on_anchor,
        match &r.step_divergence {
            None => "every action identical to the simulator".to_string(),
            Some(d) => format!("ACTIONS DIVERGE ({} of {}), first {d}", r.steps_diverging, r.steps),
        },
        match (&r.built_vs_rest, &r.sim_built_vs_rest) {
            (None, None) => "= judge rest".to_string(),
            (b, sb) => format!("!= judge rest: vanilla [{}] sim [{}]", b.clone().unwrap_or_else(|| "same".into()), sb.clone().unwrap_or_else(|| "same".into())),
        },
        verdict_str(&r.sim),
        verdict_str(&r.vanilla),
        r.ticks_compared,
        r.divergence.as_ref().map(|d| format!(" | DIVERGES: {d}")).unwrap_or_default()
    );
    for n in &s.notes {
        println!("    note: {n}");
    }
    if !s.off_ray_steps.is_empty() {
        println!("    clicked off the view ray (no open standing spot on it within reach 4.5) at steps {}", s.off_ray_steps.join(", "));
    }
}
