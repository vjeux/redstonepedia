//! (tickexact 2026-11-12, ironspec: H = 15 and DAY's thresholds) `mctas mobai detector-in-place`:
//! on the official 26.3 server, the composition (the module and its parts, rails as stated, every
//! fluid let flow), the daylight detector at its cell (`--at`, rev 3's (-5,15,3)); the column over
//! it as the game has it (every cell that is not air, identified), then its output every 20 gt over
//! a clear day, a rain day and a thunder day (each from day time 0, the weather held), with the
//! largest output and the day times of each change printed per day.

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;

fn power(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<u8>> {
    for n in 0..16u8 {
        if srv.cmd_long(&format!("execute if block {} {} {} minecraft:daylight_detector[power={n}]", p[0], p[1] + DY, p[2]))?.contains("passed") {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

pub fn detector_in_place(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], at: [i32; 3], every: u32) -> Result<()> {
    println!(
        "== detector-in-place on the official {version} server: {} with {}; the detector at ({},{},{}); its output every {every} gt over a clear, a rain and a thunder day",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        at[0],
        at[1],
        at[2]
    );
    let (mut srv, _agent, _bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    // every fluid flows (the lifts' water) before anything is read
    srv.step(200)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) });
    }
    if power(&mut srv, at)?.is_none() {
        println!("  no daylight detector at ({},{},{}) in the game: placed one there", at[0], at[1], at[2]);
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:daylight_detector", at[0], at[1] + DY, at[2]))?;
    }
    // the column over the detector up to the build height
    let mut over = Vec::new();
    for y in at[1] + 1..=at[1] + 60 {
        if !srv.cmd_long(&format!("execute if block {} {} {} minecraft:air", at[0], y + DY, at[2]))?.contains("passed") {
            let id = crate::bridgehash::identify(&mut srv, [at[0], y, at[2]])?;
            over.push(format!("({},{y},{}) {}", at[0], at[2], id.as_deref().unwrap_or("(other)")));
        }
    }
    println!("  the column over it (y {}..{}), cells that are not air: {}", at[1] + 1, at[1] + 60, if over.is_empty() { "none".to_string() } else { over.join("; ") });
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("gamerule advance_weather true")?;
    for weather in ["clear", "rain", "thunder"] {
        srv.cmd_ok(&format!("weather {weather} 1000000"))?;
        srv.cmd_ok("time set 0")?;
        // the rain and thunder levels ramp over 100 gt: let them settle before the day is read
        srv.cmd_ok("gamerule advance_time false")?;
        srv.step(200)?;
        srv.cmd_ok("time set 0")?;
        srv.cmd_ok("gamerule advance_time true")?;
        // the detector updates when the game time is a multiple of 20 (its block entity's tick):
        // the phase of the day times it updates on
        let g0 = srv.game_time()?;
        println!("  {weather} day: the game time at day time 0 is {g0} (mod 20: {}); the detector updates at day times = {} mod 20", g0.rem_euclid(20), (20 - g0.rem_euclid(20)) % 20);
        let (mut last, mut best, mut changes, mut n) = (None::<u8>, 0u8, Vec::new(), 0u32);
        let mut t = 0u32;
        while t < 24000 {
            srv.step(every)?;
            t += every;
            let p = power(&mut srv, at)?;
            n += 1;
            if let Some(v) = p {
                best = best.max(v);
            }
            if p != last {
                changes.push(format!("{t}:{}", p.map(|v| v.to_string()).unwrap_or_else(|| "?".into())));
                last = p;
            }
        }
        println!("  {weather} day: {n} readings, largest output {best}; (day time:output at each change) {}", changes.join(" "));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
