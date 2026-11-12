//! (tickexact 2026-11-12, ironspec: the golem lock only on success, the T-5 replacement's premise)
//! `mctas mobai golem-lock`: on the official 26.3 server, 3 villagers seated in minecarts with a
//! recent LAST_SLEPT (their saved brain memory), panicking at a zombie seated in a boat behind
//! glass, at night in the sky (y 150: nothing natural within any box):
//! - (a) every cell within each villager's summon box (`SpawnUtil.trySpawnMob(..., 10, 8, 6, ...)`,
//!   x and z +-8, y +-6: 17 x 17 x 13) is glass, rails or air, so no spot passes
//!   `SpawnUtil.Strategy.LEGACY_IRON_GOLEM` (glass excluded): at each summon tick (game time a
//!   multiple of 100) no golem appears and GOLEM_DETECTED_RECENTLY stays absent on all three;
//! - (b) the positive control: the same with one valid spot (a stone with 3 air above) in reach,
//!   and a 4th, calm villager (no LAST_SLEPT, more than 8 from the zombie) within 10 of them: a
//!   golem spawns and the memory is set on all four (`Villager.spawnGolemIfNeeded` marks every
//!   villager in the summoner's box inflated by 10 through `GolemSensor.golemDetected`, only when
//!   `trySpawnMob` returned a golem).
//! Each villager's brain memories are printed before and after.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 150;

/// A villager seated in a minecart on a rail at (x, Y, z) over glass at Y - 1.
fn seat(srv: &mut VanillaServer, x: i32, z: i32, tag: &str, last_slept: Option<i64>) -> Result<()> {
    let _ = srv.cmd(&format!("setblock {x} {} {z} minecraft:glass", Y - 1))?;
    srv.cmd_ok(&format!("setblock {x} {Y} {z} minecraft:rail"))?;
    let mem = last_slept.map(|t| format!("\"minecraft:last_slept\":{{value:{t}L}}")).unwrap_or_default();
    // (the cell's centre is x + 0.5 as a number: "{x}.5" would read -9.5 for x = -9, the centre of -10)
    let (cx, cz) = (x as f64 + 0.5, z as f64 + 0.5);
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {cx} {}.0625 {cz} {{Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}\"],PersistenceRequired:1b,Brain:{{memories:{{{mem}}}}}}}]}}",
        Y
    ))?;
    Ok(())
}

/// A zombie in a boat at (x + 0.5, Y, z + 0.5) on glass (seated, so it can't move; in plain sight
/// of the villagers, as in t600: a glass wall between them would block the hostile sensor's line
/// of sight).
fn zombie(srv: &mut VanillaServer, x: i32, z: i32) -> Result<()> {
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:glass", x - 1, Y - 1, z - 1, x + 1, Y - 1, z + 1))?;
    srv.cmd_ok(&format!("summon minecraft:oak_boat {} {Y} {} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b}}]}}", x as f64 + 0.5, z as f64 + 0.5))?;
    Ok(())
}

fn memories(srv: &mut VanillaServer, tag: &str) -> Result<String> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Brain.memories"))?;
    Ok(out.split("entity data: ").nth(1).unwrap_or(&out).trim().to_string())
}

fn locked(srv: &mut VanillaServer, tag: &str) -> Result<bool> {
    Ok(memories(srv, tag)?.contains("golem_detected_recently"))
}

fn golems(srv: &mut VanillaServer) -> Result<u32> {
    let out = srv.cmd_long("execute if entity @e[type=minecraft:iron_golem]")?;
    Ok(out.rsplit(' ').next().and_then(|w| w.trim().parse().ok()).unwrap_or(0))
}

fn to_next_summon_tick(srv: &mut VanillaServer) -> Result<i64> {
    let now = srv.game_time()?;
    let t = (now / 100 + 1) * 100;
    srv.step((t - now) as u32)?;
    Ok(t)
}

/// One case at x offset `ox`: the members at (ox, 0), (ox + 2, 0), (ox + 4, 0), the zombie at
/// (ox + 2, 5); `spot`: a stone at (ox + 2, Y - 1, -6) with air above; `calm`: a 4th villager at
/// (ox + 2, -9) (14 from the zombie, within 10 of the members). Returns (golems, members locked).
fn case(srv: &mut VanillaServer, ox: i32, spot: bool, calm: bool, limit: u32) -> Result<(u32, Vec<(String, bool)>)> {
    srv.cmd_ok(&format!("forceload add {} -32 {} 31", ox - 32, ox + 40))?;
    srv.step(20)?;
    zombie(srv, ox + 2, 5)?;
    if spot {
        srv.cmd_ok(&format!("setblock {} {} -6 minecraft:stone", ox + 2, Y - 1))?;
    }
    let now = srv.game_time()?;
    let mut tags = Vec::new();
    for (k, x) in [(1, ox), (2, ox + 2), (3, ox + 4)] {
        let tag = format!("g{ox}m{k}");
        seat(srv, x, 0, &tag, Some(now))?;
        tags.push(tag);
    }
    if calm {
        let tag = format!("g{ox}calm");
        seat(srv, ox + 2, -9, &tag, None)?;
        tags.push(tag);
    }
    srv.step(5)?;
    for t in &tags {
        println!("  {t} before: {}", memories(srv, t)?);
    }
    let mut g = 0;
    for _ in 0..limit {
        let t = to_next_summon_tick(srv)?;
        srv.step(1)?;
        g = golems(srv)?;
        let mut line = format!("  t {t}: {} golem(s);", g);
        for tag in &tags {
            line.push_str(&format!(" {tag} {}", if locked(srv, tag)? { "LOCKED" } else { "free" }));
        }
        println!("{line}");
        if g > 0 {
            break;
        }
    }
    for t in &tags {
        let pos = srv.cmd_long(&format!("data get entity @e[tag={t},limit=1] Pos"))?;
        println!("  {t} after: {} at {}", memories(srv, t)?, pos.split("entity data: ").nth(1).unwrap_or(&pos).trim());
    }
    if g > 0 {
        let gp = srv.cmd_long("data get entity @e[type=minecraft:iron_golem,limit=1] Pos")?;
        println!("  the golem at {}", gp.split("entity data: ").nth(1).unwrap_or(&gp).trim());
    }
    let mut out = Vec::new();
    for t in &tags {
        out.push((t.clone(), locked(srv, t)?));
    }
    Ok((g, out))
}

pub fn golem_lock(version: &str) -> Result<()> {
    println!("== golem-lock on the official {version} server: 3 panicking villagers with LAST_SLEPT, in the sky at night");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 18000")?;
    println!("(a) no valid golem spot within any member's 17 x 17 x 13 box (glass, rails, air)");
    let (ga, la) = case(&mut srv, 0, false, false, 12)?;
    println!("(b) the positive control: one stone spot with air above in reach, and a calm 4th villager within 10");
    let (gb, lb) = case(&mut srv, 300, true, true, 80)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    let a_ok = ga == 0 && la.iter().all(|(_, l)| !l);
    let b_ok = gb > 0 && lb.iter().all(|(_, l)| *l);
    println!(
        "verdict: (a) {} golems, {} of {} locked; (b) {} golems, {} of {} locked",
        ga,
        la.iter().filter(|(_, l)| *l).count(),
        la.len(),
        gb,
        lb.iter().filter(|(_, l)| *l).count(),
        lb.len()
    );
    if !(a_ok && b_ok) {
        bail!("golem-lock: (a) {} (b) {}", if a_ok { "as expected" } else { "NOT as expected" }, if b_ok { "as expected" } else { "NOT as expected" });
    }
    Ok(())
}
