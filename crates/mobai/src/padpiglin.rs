//! (tickexact 2026-11-12, ironspec's queue after F3's seats) `mctas mobai pad-piglins`: on the
//! official 26.3 server, a zombified piglin in the sealed pad's portal rows (dfb89499's station
//! file at 4bce82b85, layers y -9..-4, shifted up by `DY` into open air), the two ways one gets
//! there: (1) as the pad portal's random tick spawns it (`NetherPortalBlock.randomTick`: at the
//! lowest portal cell above the frame, then `setPortalCooldown()`, 300), summoned there with that
//! cooldown; (2) crossing back from the Nether through the linked portal (a chicken first sent
//! through to make the Nether portal; then a piglin summoned in it). Per case and per player
//! distance (the bot under 32 blocks, 32..128, over 128, in the Nether): every 100 gt over `ticks`,
//! whether the piglin is there, where (inside a portal cell or not), its PortalCooldown; the tick it
//! despawns. 26.3's code: `Mob.checkDespawn` discards a non-persistent mob at once beyond its
//! category's 128, and beyond 32 after noActionTime 600 at 1 in 800 a tick; with no player in the
//! level nothing happens; `Entity.setAsInsidePortal` resets a cooling-down entity's cooldown to 300
//! every tick it is inside.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn at(x: f64, y: f64, z: f64) -> String {
    format!("{} {} {}", x, y + DY as f64, z)
}

fn build(srv: &mut VanillaServer, station: &std::path::Path) -> Result<()> {
    srv.cmd_ok(&format!("fill -12 {} -12 16 {} 8 minecraft:air", DY - 12, DY + 6))?;
    let d = mctas::design::Design::load(station)?;
    for pass in 0..2 {
        for (p, s) in d.blocks.iter() {
            if !(-9..=-4).contains(&p[1]) {
                continue;
            }
            let portal = s.contains("nether_portal");
            if (pass == 0) == portal {
                continue;
            }
            srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
        }
    }
    Ok(())
}

/// The piglin's position (y less `DY`) and PortalCooldown in `dim`.
fn piglin(srv: &mut VanillaServer, dim: &str) -> Result<Option<([f64; 3], i64)>> {
    let sel = "@e[type=minecraft:zombified_piglin,tag=pp,limit=1,distance=0..]";
    let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    if v.len() != 3 {
        return Ok(None);
    }
    let cd = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity {sel} PortalCooldown"))?;
    let cd = cd.split("entity data: ").nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(-1);
    let y = if dim == "overworld" { v[1] - DY as f64 } else { v[1] };
    Ok(Some(([v[0], y, v[2]], cd)))
}

pub fn pad_piglins(version: &str, station: &std::path::Path, ticks: u32) -> Result<()> {
    println!("== pad-piglins on the official {version} server: a zombified piglin in the sealed pad's portal rows ({}), by the player's distance", station.display());
    for case in ["portal-spawned", "crossed back from the Nether"] {
        for (label, dist) in [("the player under 32", 20.0f64), ("the player 32..128", 64.0), ("the player over 128", 200.0), ("the player in the Nether", -1.0)] {
            let opts = VanillaOptions {
                version: version.to_string(),
                difficulty: "normal".into(),
                with_bot: true,
                biome: "minecraft:plains".into(),
                tick_rate: 1000,
                ..Default::default()
            };
            let bot = opts.bot_name.clone();
            let mut srv = VanillaServer::start(&opts)?;
            srv.cmd_ok("gamerule spawn_mobs false")?;
            srv.cmd_ok("gamerule advance_time false")?;
            srv.cmd_ok("time set 6000")?;
            srv.cmd_ok("forceload add -16 -16 15 15")?;
            srv.step(20)?;
            build(&mut srv, station)?;
            srv.teleport_bot(2.5, (DY + 2) as f64, -40.0)?;
            srv.step(400)?;
            let dim_now;
            if case == "portal-spawned" {
                // the lowest portal cell of row z -5 above the frame, the cooldown a portal spawn sets
                srv.cmd_ok(&format!("summon minecraft:zombified_piglin {} {{Tags:[\"pp\"],PortalCooldown:300}}", at(2.5, -8.0, -4.5)))?;
                dim_now = "overworld";
            } else {
                // the Nether side: a 2 x 3 portal built where the pad links (the pad at x -2..7, z
                // -5 / -2 maps to about (0..1, -1) in the Nether), its chunk loaded 400 gt first; the
                // piglin summoned inside it crosses to the pad
                srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 15 15")?;
                srv.cmd_ok("execute in minecraft:the_nether run fill -3 69 -3 4 76 2 minecraft:air")?;
                srv.cmd_ok("execute in minecraft:the_nether run fill -3 70 -3 4 70 2 minecraft:obsidian")?;
                srv.cmd_ok("execute in minecraft:the_nether run fill -1 71 -1 2 75 -1 minecraft:obsidian")?;
                srv.step(400)?;
                srv.cmd_ok("execute in minecraft:the_nether run fill 0 72 -1 1 74 -1 minecraft:nether_portal[axis=x]")?;
                srv.step(20)?;
                srv.cmd_ok("execute in minecraft:the_nether run summon minecraft:zombified_piglin 0.5 72 -0.5 {Tags:[\"pp\"]}")?;
                let mut arrived = None;
                for t in 0..200 {
                    srv.step(1)?;
                    if piglin(&mut srv, "overworld")?.is_some() {
                        arrived = Some(t + 1);
                        break;
                    }
                }
                match arrived {
                    Some(t) => println!("  {case}, {label}: the piglin, summoned in the Nether portal (0..1, 72..74, -1), arrived in the overworld after {t} gt at {:?}", piglin(&mut srv, "overworld")?),
                    None => {
                        println!("  {case}, {label}: the piglin never crossed to the overworld in 200 gt (in the Nether: {:?}); case skipped", piglin(&mut srv, "the_nether")?);
                        let _ = std::fs::remove_dir_all(&srv.dir);
                        continue;
                    }
                }
                dim_now = "overworld";
            }
            if dist > 0.0 {
                srv.teleport_bot(2.5 + dist, (DY - 7) as f64, -4.5)?;
            } else {
                srv.cmd_ok(&format!("execute in minecraft:the_nether run tp {bot} 0 70 0"))?;
            }
            let mut line = Vec::new();
            let mut gone = None;
            for t in 0..=ticks {
                if t > 0 {
                    srv.step(1)?;
                }
                if t % 100 == 0 || gone.is_none() {
                    let p = piglin(&mut srv, dim_now)?;
                    match p {
                        Some((q, cd)) => {
                            if t % 100 == 0 {
                                let inside = srv.test_block([q[0].floor() as i32, q[1].floor() as i32 + DY, q[2].floor() as i32], "minecraft:nether_portal")?;
                                line.push(format!("t{t} ({:.2},{:.2},{:.2}) {} cd {cd}", q[0], q[1], q[2], if inside { "in a portal cell" } else { "OUT of the portal" }));
                            }
                        }
                        None => {
                            if gone.is_none() {
                                let n = piglin(&mut srv, "the_nether")?;
                                gone = Some(t);
                                line.push(format!("t{t} gone from the overworld{}", n.map(|(q, cd)| format!(" (in the Nether at ({:.1},{:.1},{:.1}) cd {cd})", q[0], q[1], q[2])).unwrap_or_else(|| " (despawned)".into())));
                                break;
                            }
                        }
                    }
                }
            }
            println!("  {case}, {label}: {}", line.join("; "));
            let _ = std::fs::remove_dir_all(&srv.dir);
        }
    }
    Ok(())
}
