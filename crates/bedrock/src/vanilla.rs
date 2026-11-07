//! The same scenarios on the UNMODIFIED official server, the chord done by a bot over the
//! network (real `START_DESTROY_BLOCK` and `USE_ITEM_ON` packets). The server runs frozen
//! (`/tick freeze`) and is stepped one tick at a time, so "one inter-tick window" is exact: the
//! bot's packets of a window are all handled before the next `/tick step 1`.

use crate::{Action, Outcome, Scenario};
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::time::Duration;

/// Where the bot stands for the chord (feet) and where it looks (yaw, pitch: what a placed
/// piston's facing is read from). A piston placed while looking up (pitch -90) faces down.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Stance {
    pub pos: [f64; 3],
    pub look: [f32; 2],
    /// `standing` (eye 1.62, the default), `sneaking` (1.27) or `crawling` (0.4: a one-block-tall
    /// gap; the server picks the pose from the space, so the build must give it one).
    #[serde(default)]
    pub pose: Option<String>,
    /// Refuse clicks a real player could not make from this stance: the view ray from the eye
    /// along (yaw, pitch) must reach the clicked block within 4.5 blocks and enter it through
    /// the face the bot sends, and a placed piston's facing must be what that look gives
    /// (`getNearestLookingDirection().getOpposite()`).
    #[serde(default)]
    pub legit: bool,
}

impl Stance {
    fn eye(&self) -> [f64; 3] {
        let h = match self.pose.as_deref() {
            Some("crawling") => 0.4,
            Some("sneaking") => 1.27,
            _ => 1.62,
        };
        [self.pos[0], self.pos[1] + h, self.pos[2]]
    }

    fn view(&self) -> [f64; 3] {
        let (yaw, pitch) = ((self.look[0] as f64).to_radians(), (self.look[1] as f64).to_radians());
        [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
    }

    /// The face of block `p` the view ray enters (and the distance), or None when it misses.
    fn ray_face(&self, p: [i32; 3]) -> Option<(u8, f64)> {
        let (o, d) = (self.eye(), self.view());
        let (mut tmin, mut tmax, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 0usize);
        for a in 0..3 {
            let (lo, hi) = (p[a] as f64, p[a] as f64 + 1.0);
            if d[a].abs() < 1e-12 {
                if o[a] < lo || o[a] > hi {
                    return None;
                }
                continue;
            }
            let (t1, t2) = ((lo - o[a]) / d[a], (hi - o[a]) / d[a]);
            let (near, far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
            if near > tmin {
                tmin = near;
                axis = a;
            }
            tmax = tmax.min(far);
        }
        if tmin > tmax || tmin < 0.0 {
            return None;
        }
        // Entering through the face whose normal points back at the eye.
        let face = match (axis, d[axis] > 0.0) {
            (0, true) => 4,
            (0, false) => 5,
            (1, true) => 0,
            (1, false) => 1,
            (2, true) => 2,
            _ => 3,
        };
        Some((face, tmin))
    }

    /// The facing a piston placed from this look gets: the opposite of the nearest look direction.
    fn placed_facing(&self) -> &'static str {
        let d = self.view();
        let a = (0..3).max_by(|&i, &j| d[i].abs().total_cmp(&d[j].abs())).unwrap();
        match (a, d[a] > 0.0) {
            (0, true) => "west",
            (0, false) => "east",
            (1, true) => "down",
            (1, false) => "up",
            (2, true) => "north",
            _ => "south",
        }
    }

    /// The face to click on `p`, checked when `legit`.
    fn click_face(&self, p: [i32; 3]) -> Result<u8> {
        if !self.legit {
            return Ok(face_toward(self.pos, p));
        }
        match self.ray_face(p) {
            Some((face, t)) if t <= 4.5 => Ok(face),
            Some((_, t)) => bail!("legit: {p:?} is {t:.2} blocks from the eye (reach 4.5)"),
            None => bail!("legit: the view ray from {:?} (yaw {}, pitch {}) misses {p:?}", self.eye(), self.look[0], self.look[1]),
        }
    }
}

fn face_toward(from: [f64; 3], p: [i32; 3]) -> u8 {
    // The face of `p` the bot sees: the axis with the largest offset from the block center.
    let d = [from[0] - (p[0] as f64 + 0.5), from[1] + 1.62 - (p[1] as f64 + 0.5), from[2] - (p[2] as f64 + 0.5)];
    let (ax, v) = (0..3).map(|i| (i, d[i])).max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).unwrap();
    // Direction ids: 0 down, 1 up, 2 north (-z), 3 south (+z), 4 west (-x), 5 east (+x).
    match (ax, v > 0.0) {
        (0, true) => 5,
        (0, false) => 4,
        (1, true) => 1,
        (1, false) => 0,
        (2, true) => 3,
        _ => 2,
    }
}

fn face_id(name: &str) -> Result<u8> {
    Ok(match name {
        "down" => 0,
        "up" => 1,
        "north" => 2,
        "south" => 3,
        "west" => 4,
        "east" => 5,
        other => bail!("unknown face {other}"),
    })
}

/// Perform actions with the bot (no tick between them); `place` puts the action's block in the
/// offhand first (RCON, handled in order with the bot's packets) and clicks with the offhand.
fn bot_actions(srv: &mut VanillaServer, stance: &Stance, actions: &[&Action]) -> Result<()> {
    let name = srv.bot.as_ref().context("no bot")?.name.clone();
    let mut last = 0;
    for a in actions {
        match a.what.as_str() {
            "break" => {
                let face = stance.click_face(a.pos)?;
                last = srv.bot.as_mut().unwrap().start_destroy(a.pos, face)?;
            }
            "place" => {
                let state = a.state.clone().context("place needs a state")?;
                if stance.legit && (state.contains("piston[") || state.contains("observer[")) {
                    if let Some(f) = state.split("facing=").nth(1).map(|r| r.split([',', ']']).next().unwrap_or("")) {
                        if f != stance.placed_facing() {
                            bail!("legit: {state} needs facing={f}, but the look (yaw {}, pitch {}) places facing={}", stance.look[0], stance.look[1], stance.placed_facing());
                        }
                    }
                }
                let item = state.split('[').next().unwrap_or(&state).to_string();
                srv.cmd_ok(&format!("item replace entity {name} weapon.offhand with {item}"))?;
                let (pos, face) = match (&a.click, &a.face) {
                    (Some(c), Some(f)) => (*c, face_id(f)?),
                    _ => (a.pos, stance.click_face(a.pos)?),
                };
                last = srv.bot.as_mut().unwrap().use_item_on_hand(1, pos, face, [0.5, 0.5, 0.5])?;
                // The next RCON command (another offhand item) must not overtake this click.
                srv.bot.as_ref().unwrap().wait_ack(last, Duration::from_secs(10))?;
            }
            "use" => {
                let face = stance.click_face(a.pos)?;
                last = srv.bot.as_mut().unwrap().use_item_on_hand(0, a.pos, face, [0.5, 0.5, 0.5])?;
            }
            "cmd" => {
                srv.cmd_ok(a.state.as_deref().unwrap_or(""))?;
            }
            other => bail!("unknown action {other}"),
        }
    }
    if last > 0 {
        srv.bot.as_ref().unwrap().wait_ack(last, Duration::from_secs(10))?;
    }
    Ok(())
}

fn is_bedrock(srv: &mut VanillaServer, sc: &Scenario, p: [i32; 3]) -> Result<bool> {
    match &sc.dimension {
        None => srv.test_block(p, "minecraft:bedrock"),
        Some(d) => {
            let out = srv.cmd(&format!("execute in minecraft:{d} if block {} {} {} minecraft:bedrock", p[0], p[1], p[2]))?;
            if out.contains("Test passed") {
                Ok(true)
            } else if out.contains("Test failed") {
                Ok(false)
            } else {
                bail!("execute in {d} if block: {out}")
            }
        }
    }
}

/// A command run in the scenario's dimension.
fn in_dim(sc: &Scenario, cmd: &str) -> String {
    match &sc.dimension {
        None => cmd.to_string(),
        Some(d) => format!("execute in minecraft:{d} run {cmd}"),
    }
}

/// Teleport the bot to the stance, in the scenario's dimension, and wait until it accepted.
fn place_bot(srv: &mut VanillaServer, sc: &Scenario, stance: &Stance) -> Result<()> {
    if sc.dimension.is_none() {
        return srv.teleport_bot_to(stance.pos[0], stance.pos[1], stance.pos[2], Some((stance.look[0], stance.look[1])));
    }
    let (name, before) = {
        let b = srv.bot.as_ref().context("no bot")?;
        (b.name.clone(), b.teleports_accepted())
    };
    let tp = format!("tp {name} {:.5} {:.5} {:.5} {} {}", stance.pos[0], stance.pos[1], stance.pos[2], stance.look[0], stance.look[1]);
    srv.cmd_ok(&in_dim(sc, &tp))?;
    let t0 = std::time::Instant::now();
    while srv.bot.as_ref().unwrap().teleports_accepted() <= before {
        if let Some(e) = srv.bot.as_ref().unwrap().error() {
            bail!("bot: {e}");
        }
        if t0.elapsed() > Duration::from_secs(30) {
            bail!("the bot never acknowledged its teleport into {:?}", sc.dimension);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Let the server tick the arrival (the pose follows the space: a 1-tall gap = crawling).
    srv.step(2)?;
    srv.cmd("list")?;
    Ok(())
}

/// One trial: build, settle, run the script; the chord in window `chord_at`, or split over
/// windows `chord_at` and `chord_at + 1` (its first action alone, then the rest: the same clicks
/// one tick apart). Returns the outcome.
pub fn trial(srv: &mut VanillaServer, sc: &Scenario, stance: &Stance, chord_at: u32, split: bool) -> Result<Outcome> {
    let lo = [-6, 0, -6];
    let hi = [6, 8, 6];
    if sc.dimension.is_none() {
        srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]))?;
    }
    srv.cmd_ok("kill @e[type=!player]")?;
    for c in &sc.setup {
        srv.cmd_ok(c)?;
    }
    for (x, y, z, s) in &sc.blocks {
        if sc.dimension.is_some() {
            // Re-runs find the build already there: "Could not set the block" is fine.
            let out = srv.cmd(&in_dim(sc, &format!("setblock {x} {y} {z} {s}")))?;
            let l = out.to_lowercase();
            if l.contains("not loaded") || l.contains("unknown") || l.contains("incorrect") || l.contains("expected") {
                bail!("setblock {x} {y} {z} {s} -> {out}");
            }
            continue;
        }
        srv.cmd_ok(&in_dim(sc, &format!("setblock {x} {y} {z} {s}")))?;
    }
    srv.step(sc.settle.max(1))?;
    if !is_bedrock(srv, sc, sc.target)? {
        bail!("{}: the target is not bedrock after the build settled", sc.name);
    }
    place_bot(srv, sc, stance)?;
    let last_script = sc.script.iter().map(|a| a.at).max().unwrap_or(0);
    let end = last_script.max(chord_at + 1) + sc.horizon;
    for window in 0..=end {
        let script: Vec<&Action> = sc.script.iter().filter(|a| a.at == window).collect();
        bot_actions(srv, stance, &script)?;
        if window == chord_at {
            let chord: Vec<&Action> = sc.chord.iter().collect();
            let n = if split { 1 } else { chord.len() };
            bot_actions(srv, stance, &chord[..n])?;
        }
        if split && window == chord_at + 1 {
            let chord: Vec<&Action> = sc.chord.iter().skip(1).collect();
            bot_actions(srv, stance, &chord)?;
        }
        srv.step(1)?;
        if window == chord_at + 1 && sc.dimension.is_some() && std::env::var_os("MCTAS_BEDROCK_DEBUG").is_some() {
            let mut probes: Vec<[i32; 3]> = sc.chord.iter().map(|a| a.pos).collect();
            probes.push(sc.target);
            let states = [
                "minecraft:bedrock", "minecraft:air", "minecraft:moving_piston",
                "minecraft:piston[facing=down,extended=false]", "minecraft:piston[facing=down,extended=true]",
                "minecraft:piston[facing=west,extended=false]", "minecraft:piston[facing=west,extended=true]",
                "minecraft:piston[facing=up]", "minecraft:piston[facing=north]", "minecraft:piston[facing=south]", "minecraft:piston[facing=east]",
            ];
            for p in probes {
                let mut hits = Vec::new();
                for s in states {
                    let out = srv.cmd(&in_dim(sc, &format!("execute if block {} {} {} {s}", p[0], p[1], p[2])).replace("run execute ", ""))?;
                    if out.contains("Test passed") {
                        hits.push(s);
                    }
                }
                eprintln!("  after the chord: {p:?} {hits:?}");
            }
        }
        if window == chord_at + 1 && sc.dimension.is_none() && std::env::var_os("MCTAS_BEDROCK_DEBUG").is_some() {
            let cells: Vec<[i32; 3]> = sc.chord.iter().map(|a| a.pos).collect();
            let lo = [cells.iter().map(|c| c[0]).min().unwrap() - 1, cells.iter().map(|c| c[1]).min().unwrap() - 1, cells.iter().map(|c| c[2]).min().unwrap() - 1];
            let hi = [cells.iter().map(|c| c[0]).max().unwrap() + 1, cells.iter().map(|c| c[1]).max().unwrap() + 1, cells.iter().map(|c| c[2]).max().unwrap() + 1];
            let snap = srv.snapshot(lo, hi)?;
            for (p, s) in snap.region.iter() {
                if s != "minecraft:air" {
                    eprintln!("  after the chord: {p:?} {s}");
                }
            }
        }
    }
    let all_gone = |srv: &mut VanillaServer| -> Result<bool> {
        for p in std::iter::once(sc.target).chain(sc.also.iter().copied()) {
            if is_bedrock(srv, sc, p)? {
                return Ok(false);
            }
        }
        Ok(true)
    };
    if all_gone(srv)? {
        return Ok(Outcome::Broken);
    }
    if !sc.release.is_empty() {
        srv.step(20)?;
        let rel: Vec<&Action> = sc.release.iter().collect();
        bot_actions(srv, stance, &rel)?;
        srv.step(sc.horizon)?;
        if all_gone(srv)? {
            return Ok(Outcome::Headless);
        }
    }
    Ok(Outcome::Fail)
}

/// Start the official server with the bot and run the trials.
pub fn confirm(version: &str, sc: &Scenario, stance: &Stance, windows: &[u32], split: bool) -> Result<Vec<(u32, Outcome)>> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), ..Default::default() })?;
    let mut out = Vec::new();
    for &w in windows {
        out.push((w, trial(&mut srv, sc, stance, w, split)?));
    }
    Ok(out)
}
