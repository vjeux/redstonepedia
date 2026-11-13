//! (tickexact 2026-11-12, ironspec 16:36: a golem pad of bottom slabs on glass) `mctas mobai
//! golem-pad --pad <state> --stands N --ticks T`: on the official 26.3 server, N stands 64 apart in
//! a void, each a glass floor (LEGACY_IRON_GOLEM never spawns on glass) with three villagers in carts
//! on rails (their last sleep recent, panicking at a husk in a boat 5 away) and beside them a pad of
//! 5 x 3 cells of `--pad` laid on the glass (cobblestone: today's pad; a bottom slab: the question);
//! every golem is killed on its 2nd tick (the farm's leaves through a portal then). Logged (the brain
//! agent): every villager's GOLEM_DETECTED_RECENTLY set and erase; per stand, its summon chances
//! (the PANIC ticks % 100 == 0 at which none of its three has GOLEM_DETECTED_RECENTLY: Villager.
//! spawnGolemIfNeeded's tries) and its golems (a set on the trio with a golem born at the stand).
//! Printed: golems per chance per stand and pooled, and where each golem stood (on the pad?).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const SPACING: i32 = 64;
const MEMS: &str = "minecraft:golem_detected_recently";

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

pub fn golem_pad(version: &str, pad: &str, under: &str, stands: u32, ticks: u32) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    for k in 0..stands as i32 {
        let x0 = SPACING * k;
        srv.cmd_ok(&format!("forceload add {} -16 {} 24", x0 - 16, x0 + 16))?;
    }
    srv.step(200)?;
    println!(
        "== golem-pad on the official {version} server: {stands} stands, a glass floor, three panicking villagers in carts, a 5 x 3 pad of {pad} beside them over {under}; {ticks} gt; every golem killed on its 2nd tick"
    );
    let mut stand_villagers: Vec<Vec<String>> = Vec::new();
    for k in 0..stands as i32 {
        let x0 = SPACING * k;
        srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:glass", x0 - 10, Y - 1, -10, x0 + 10, Y - 1, 14))?;
        // the pad: x0+2..x0+4, z 0..4, on the glass
        srv.cmd_ok(&format!("fill {} {Y} 0 {} {Y} 4 {pad}", x0 + 2, x0 + 4))?;
        // the layer under the pad (glass as laid, or `--under`: air, ...)
        if under != "minecraft:glass" {
            srv.cmd_ok(&format!("fill {} {} 0 {} {} 4 {under}", x0 + 2, Y - 1, x0 + 4, Y - 1))?;
        }
        let mut vs = Vec::new();
        for (i, dz) in [0, 2, 4].iter().enumerate() {
            srv.cmd(&format!("setblock {x0} {Y} {dz} minecraft:rail"))?;
            let tag = format!("gp{k}v{i}");
            srv.cmd_ok(&format!(
                "summon minecraft:minecart {x0}.5 {}.0625 {dz}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}\"],Brain:{{memories:{{\"minecraft:last_slept\":{{value:900L}}}}}}}}]}}",
                Y
            ))?;
            vs.push(uuid_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?);
        }
        srv.cmd_ok(&format!("summon minecraft:oak_boat {}.5 {Y} 2.5 {{Passengers:[{{id:\"minecraft:husk\",PersistenceRequired:1b}}]}}", x0 - 5))?;
        stand_villagers.push(vs);
    }
    let mut log_at = 0u64;
    let mut drain = |srv: &VanillaServer| -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(log_at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        log_at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    };
    drain(&srv)?;
    // per stand: which villagers hold GOLEM_DETECTED_RECENTLY now, its chances, its golems
    let n = stands as usize;
    // per villager, the game time its GOLEM_DETECTED_RECENTLY lapses (set with a 600-gt expiry; the
    // brain's expiry erases it without the agent's erase hook, so it is timed here)
    let mut holding: Vec<std::collections::BTreeMap<String, i64>> = vec![Default::default(); n];
    let (mut chances, mut golems) = (vec![0u32; n], vec![0u32; n]);
    let mut on_pad = vec![0u32; n];
    let mut where_: Vec<Vec<String>> = vec![Vec::new(); n];
    let mut panicking: Vec<std::collections::BTreeSet<String>> = vec![Default::default(); n];
    // each golem's feet y on its 2nd tick (before the kill): on a slab it stands at the slab's top
    let mut second_y: Vec<f64> = Vec::new();
    let t0 = srv.game_time()?;
    for _ in 0..ticks {
        srv.step(1)?;
        let t = srv.game_time()?;
        // (the state at the tick's start: a summon this tick sets GOLEM_DETECTED_RECENTLY on the
        // trio within the tick)
        let was_empty: Vec<bool> = holding.iter().map(|h| h.values().all(|&until| until <= t)).collect();
        let was_panicking: Vec<bool> = panicking.iter().map(|p| !p.is_empty()).collect();
        for l in drain(&srv)? {
            let Some(who) = l.rsplit(" @").next().map(|u| u.trim().to_string()) else { continue };
            if let Some(a) = l.strip_prefix("activity ") {
                let a = a.split(" by ").next().unwrap_or(a);
                for (k, vs) in stand_villagers.iter().enumerate() {
                    if vs.contains(&who) {
                        if a.contains("panic") {
                            panicking[k].insert(who.clone());
                        } else {
                            panicking[k].remove(&who);
                        }
                    }
                }
                continue;
            }
            let set = l.starts_with("set minecraft:golem_detected_recently");
            let erase = l.starts_with("erase minecraft:golem_detected_recently") && !l.contains("(absent)");
            for (k, vs) in stand_villagers.iter().enumerate() {
                if vs.contains(&who) {
                    if set {
                        holding[k].insert(who.clone(), t + 600);
                    } else if erase {
                        holding[k].remove(&who);
                    }
                }
            }
        }
        // the golems born this tick: count and place them, then killed next tick (`gp_old`), their
        // feet y read on that 2nd tick first
        let old = srv.cmd_long("execute as @e[type=minecraft:iron_golem,tag=gp_old] run data get entity @s Pos")?;
        for part in old.split("entity data: ").skip(1) {
            let p = nums(&format!("entity data: {part}"));
            if p.len() == 3 {
                second_y.push(p[1] - Y as f64);
            }
        }
        srv.cmd("kill @e[type=minecraft:iron_golem,tag=gp_old]")?;
        let list = srv.cmd_long("execute as @e[type=minecraft:iron_golem,tag=!gp_old] run data get entity @s Pos")?;
        for part in list.split("entity data: ").skip(1) {
            let p = nums(&format!("entity data: {part}"));
            if p.len() == 3 {
                let k = ((p[0] + SPACING as f64 / 2.0) / SPACING as f64).floor() as usize;
                if k < n {
                    golems[k] += 1;
                    let x0 = SPACING as f64 * k as f64;
                    let pad_hit = (x0 + 2.0..x0 + 5.0).contains(&p[0]) && (0.0..5.0).contains(&p[2]);
                    if pad_hit {
                        on_pad[k] += 1;
                    }
                    where_[k].push(format!("t{} ({:.2},{:.3},{:.2})", t - t0, p[0] - x0, p[1] - Y as f64, p[2]));
                }
            }
        }
        srv.cmd("tag @e[type=minecraft:iron_golem,tag=!gp_old] add gp_old")?;
        // the chances: PANIC's spawnGolemIfNeeded runs at game times % 100 == 0 (a panicking
        // villager of the stand calls it); the stand tries when none of its three holds
        // GOLEM_DETECTED_RECENTLY at the tick's start
        if t % 100 == 0 {
            for k in 0..n {
                if (was_panicking[k] || !panicking[k].is_empty()) && was_empty[k] {
                    chances[k] += 1;
                }
            }
        }
    }
    let (tc, tg, tp): (u32, u32, u32) = (chances.iter().sum(), golems.iter().sum(), on_pad.iter().sum());
    for k in 0..n {
        println!("  stand {k}: {} golems in {} chances; on the pad {}; {}", golems[k], chances[k], on_pad[k], where_[k].iter().take(6).cloned().collect::<Vec<_>>().join(", "));
    }
    println!("  pooled: {tg} golems in {tc} chances ({:.3} per chance), {tp} on the pad", if tc > 0 { tg as f64 / tc as f64 } else { 0.0 });
    let mut ys: Vec<String> = second_y.iter().map(|y| format!("{y:.4}")).collect();
    ys.sort();
    ys.dedup();
    println!("  the golems' feet y (relative to the pad cell's y) on their 2nd tick: {}", if ys.is_empty() { "-".to_string() } else { ys.join(", ") });
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
