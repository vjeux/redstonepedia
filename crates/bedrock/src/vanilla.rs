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
                let face = face_toward(stance.pos, a.pos);
                last = srv.bot.as_mut().unwrap().start_destroy(a.pos, face)?;
            }
            "place" => {
                let state = a.state.clone().context("place needs a state")?;
                let item = state.split('[').next().unwrap_or(&state).to_string();
                srv.cmd_ok(&format!("item replace entity {name} weapon.offhand with {item}"))?;
                let (pos, face) = match (&a.click, &a.face) {
                    (Some(c), Some(f)) => (*c, face_id(f)?),
                    _ => (a.pos, face_toward(stance.pos, a.pos)),
                };
                last = srv.bot.as_mut().unwrap().use_item_on_hand(1, pos, face, [0.5, 0.5, 0.5])?;
                // The next RCON command (another offhand item) must not overtake this click.
                srv.bot.as_ref().unwrap().wait_ack(last, Duration::from_secs(10))?;
            }
            "use" => {
                let face = face_toward(stance.pos, a.pos);
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

fn is_bedrock(srv: &mut VanillaServer, p: [i32; 3]) -> Result<bool> {
    srv.test_block(p, "minecraft:bedrock")
}

/// One trial: build, settle, run the script; the chord in window `chord_at`, or split over
/// windows `chord_at` and `chord_at + 1` (its first action alone, then the rest: the same clicks
/// one tick apart). Returns the outcome.
pub fn trial(srv: &mut VanillaServer, sc: &Scenario, stance: &Stance, chord_at: u32, split: bool) -> Result<Outcome> {
    let lo = [-6, 0, -6];
    let hi = [6, 8, 6];
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]))?;
    srv.cmd_ok("kill @e[type=!player]")?;
    for (x, y, z, s) in &sc.blocks {
        srv.cmd_ok(&format!("setblock {x} {y} {z} {s}"))?;
    }
    srv.step(sc.settle.max(1))?;
    if !is_bedrock(srv, sc.target)? {
        bail!("{}: the target is not bedrock after the build settled", sc.name);
    }
    srv.teleport_bot_to(stance.pos[0], stance.pos[1], stance.pos[2], Some((stance.look[0], stance.look[1])))?;
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
        if window == chord_at + 1 && std::env::var_os("MCTAS_BEDROCK_DEBUG").is_some() {
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
            if is_bedrock(srv, p)? {
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
