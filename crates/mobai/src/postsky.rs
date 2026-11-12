//! (tickexact 2026-11-12, ironspec: the queue post's sky) `mctas mobai post-sky`: on the official
//! 26.3 server, the composition (the module and its parts, every fluid flowed) with the post's zombie
//! summoned in its boat (`--summon`); at noon (clear) the light at the zombie's eye cell (the
//! location predicate's `light`: the brightest of sky light less the sky's darkening and block
//! light) and whether that cell sees the sky (`can_see_sky`); then a clear day from day time 0 with
//! the zombie's Fire and Health every 20 gt (a sun-burning zombie catches fire).

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;
const ZOMBIE_EYE: f64 = 1.74;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn light_at(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<u8>> {
    for n in 0..16u8 {
        let pred = format!("{{type:\"minecraft:location_check\",predicate:{{light:{{light:{n}}}}}}}");
        if srv.cmd_long(&format!("execute positioned {} {} {} if predicate {pred}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

pub fn post_sky(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], summon: &str) -> Result<()> {
    println!("== post-sky on the official {version} server: {} with {}; the post's zombie: {summon}", module.display(), parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    let (mut srv, _agent, _bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    let w: Vec<&str> = summon.trim().splitn(5, ' ').collect();
    anyhow::ensure!(w.len() >= 4, "--summon \"kind x y z [nbt]\", got {summon}");
    let (x, y, z): (f64, f64, f64) = (w[1].parse()?, w[2].parse()?, w[3].parse()?);
    srv.cmd_ok(&format!("summon {} {x} {} {z} {}", w[0], y + DY as f64, w.get(4).copied().unwrap_or("")))?;
    srv.step(200)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ", f.display(), m.len());
    }
    let sel = format!("@e[type=minecraft:zombie,sort=nearest,limit=1,x={x},y={},z={z}]", y + DY as f64);
    let zp = nums(&srv.cmd_long(&format!("data get entity {sel} Pos"))?);
    anyhow::ensure!(zp.len() == 3, "no zombie near the post");
    let eye = [zp[0].floor() as i32, (zp[1] - DY as f64 + ZOMBIE_EYE).floor() as i32, zp[2].floor() as i32];
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    srv.step(20)?;
    let light = light_at(&mut srv, eye)?;
    // the raw answers, so a refused predicate cannot pass for a "no"
    let raw_light = srv.cmd_long(&format!("execute positioned {} {} {} if predicate {{type:\"minecraft:location_check\",predicate:{{light:{{light:{{min:0,max:15}}}}}}}}", eye[0], eye[1] + DY, eye[2]))?;
    let raw_sky = srv.cmd_long(&format!("execute positioned {} {} {} if predicate {{type:\"minecraft:location_check\",predicate:{{can_see_sky:true}}}}", eye[0], eye[1] + DY, eye[2]))?;
    let raw_sky_above = srv.cmd_long(&format!("execute positioned {} {} {} if predicate {{type:\"minecraft:location_check\",predicate:{{can_see_sky:true}}}}", eye[0], 200, eye[2]))?;
    println!("  (the raw answers: light 0..15 at the eye: {}; can_see_sky at the eye: {}; can_see_sky at y 200 over it (the control): {})", raw_light.trim(), raw_sky.trim(), raw_sky_above.trim());
    let sky = srv.cmd_long(&format!("execute positioned {} {} {} if predicate {{type:\"minecraft:location_check\",predicate:{{can_see_sky:true}}}}", eye[0], eye[1] + DY, eye[2]))?.contains("passed");
    println!(
        "  the zombie at ({:.4},{:.4},{:.4}), its eye ({:.4}) in cell ({},{},{}): at noon (clear) the light there {}; it sees the sky: {sky}",
        zp[0],
        zp[1] - DY as f64,
        zp[2],
        zp[1] - DY as f64 + ZOMBIE_EYE,
        eye[0],
        eye[1],
        eye[2],
        light.map(|l| l.to_string()).unwrap_or_else(|| "?".into())
    );
    // a clear day: its Fire and Health every 20 gt
    srv.cmd_ok("time set 0")?;
    srv.cmd_ok("gamerule advance_time true")?;
    let (mut burning, mut min_health, mut n) = (Vec::new(), f64::MAX, 0u32);
    let mut t = 0u32;
    while t < 24000 {
        srv.step(20)?;
        t += 20;
        n += 1;
        let fire = nums(&srv.cmd_long(&format!("data get entity {sel} Fire"))?).first().copied().unwrap_or(-1.0);
        let health = nums(&srv.cmd_long(&format!("data get entity {sel} Health"))?).first().copied().unwrap_or(-1.0);
        min_health = min_health.min(health);
        if fire > 0.0 {
            burning.push(format!("{t}:{fire}"));
        }
    }
    println!("  a clear day, {n} readings: the zombie burning at {} readings{}; its least health {min_health}", burning.len(), if burning.is_empty() { String::new() } else { format!(" (day time:Fire) {}", burning.iter().take(20).cloned().collect::<Vec<_>>().join(" ")) });
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
