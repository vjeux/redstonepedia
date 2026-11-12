//! (tickexact 2026-11-12, ironspec 12:44: A6's restart re-board) `mctas mobai reboard`: on the
//! official 26.3 server, the A6 stop alone (`--file`, its local frame), the landing (2,2,-3) as the
//! file has it (the chest: the rider's feet at 2.875) or the old landing (`--glass`, the control:
//! the chest gone, the rider on the glass (2,1,-3) under it, feet 2.0); a cart with a villager (HOME the bed's
//! head (-3,0,4), night) summoned at (3.5,2.0625,-3.5) heading west at each `--speeds` onto the
//! activator (2,2,-4), which ejects the rider; `k` gt after the eject (every k in 0..=`--window`)
//! a save, stop and start (boardingCooldown is not saved, so a rider touching its still-moving cart
//! after the start can board again); then 40 gt watched for a re-board. Per row: the eject tick, the
//! rider's y and the cart's box top and speed at the restart, the re-board tick if any.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

fn riding(srv: &mut VanillaServer) -> Result<bool> {
    Ok(srv.cmd_long("execute as @e[tag=rbv,limit=1] on vehicle if entity @s")?.contains("passed"))
}

/// (rider y, cart y, cart horizontal speed, rider x, rider z)
fn state(srv: &mut VanillaServer) -> Result<(f64, f64, f64, f64, f64)> {
    let r = nums(&srv.cmd_long("data get entity @e[tag=rbv,limit=1] Pos")?);
    let c = nums(&srv.cmd_long("data get entity @e[tag=rbc,limit=1] Pos")?);
    let m = nums(&srv.cmd_long("data get entity @e[tag=rbc,limit=1] Motion")?);
    Ok((
        r.get(1).copied().unwrap_or(f64::NAN) - DY as f64,
        c.get(1).copied().unwrap_or(f64::NAN) - DY as f64,
        if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { f64::NAN },
        r.first().copied().unwrap_or(f64::NAN),
        r.get(2).copied().unwrap_or(f64::NAN),
    ))
}

pub fn reboard(version: &str, file: &std::path::Path, speeds: &[f64], window: u32, glass: bool) -> Result<()> {
    println!(
        "== reboard on the official {version} server: {} alone, the landing (2,2,-3) {}; a ridden cart from (3.5,2.0625,-3.5) west at {speeds:?} onto the activator; a restart at eject + 0..={window}, then 40 gt",
        file.display(),
        if glass { "the old landing (the chest gone: the rider on the glass under it, the control)" } else { "as the file has it" }
    );
    let mut reboards = 0u32;
    let mut rows = 0u32;
    for &v in speeds {
        for k in 0..=window {
            let mut srv = VanillaServer::start(&opts(version, None))?;
            srv.cmd_ok("gamerule advance_time false")?;
            srv.cmd_ok("gamerule spawn_mobs false")?;
            srv.cmd_ok("time set 13800")?;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            srv.step(40)?;
            crate::seatclear::place_part(&mut srv, file)?;
            if glass {
                // the old landing: no chest, the rider stands on the glass under it (feet 2.0)
                srv.cmd_ok(&format!("setblock 2 {} -3 minecraft:air", 2 + DY))?;
            }
            srv.step(20)?;
            srv.cmd_ok(&format!(
                "summon minecraft:minecart 3.5 {} -3.5 {{Tags:[\"rbc\"],Motion:[{}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"rbv\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;-3,{},4]}}}}}}}}}}]}}",
                2.0625 + DY as f64,
                -v,
                DY
            ))?;
            let mut eject = None;
            for t in 1..=200u32 {
                srv.step(1)?;
                if !riding(&mut srv)? {
                    eject = Some(t);
                    break;
                }
            }
            let Some(e) = eject else {
                println!("  {v} b/gt, k {k}: no eject in 200 gt");
                let _ = std::fs::remove_dir_all(&srv.dir);
                continue;
            };
            srv.step(k)?;
            let (ry, cy, cs, rx, rz) = state(&mut srv)?;
            let riding_before = riding(&mut srv)?;
            srv.cmd_ok("save-on")?;
            srv.cmd_ok("save-all flush")?;
            let old = srv.dir.clone();
            let world = old.join(&srv.level_name);
            drop(srv);
            let mut srv = VanillaServer::start(&opts(version, Some(world)))?;
            let _ = std::fs::remove_dir_all(&old);
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            let mut again = None;
            for t in 1..=40u32 {
                srv.step(1)?;
                if riding(&mut srv)? {
                    again = Some(t);
                    break;
                }
            }
            rows += 1;
            if again.is_some() || riding_before {
                reboards += 1;
            }
            let after = state(&mut srv)?;
            println!(
                "  {v} b/gt, restart at eject + {k} (eject t{e}): rider at ({rx:.3},{ry:.4},{rz:.3}), cart y {cy:.4} (box top {:.4}), cart speed {cs:.4}{}; after the start: {}",
                cy + 0.7,
                if riding_before { " (riding again BEFORE the restart)" } else { "" },
                again.map(|t| format!("RE-BOARDED at +{t} (rider y {:.4}, cart y {:.4})", after.0, after.1)).unwrap_or_else(|| format!("no re-board in 40 gt (rider y {:.4}, cart y {:.4}, cart speed {:.4})", after.0, after.1, after.2))
            );
            let _ = std::fs::remove_dir_all(&srv.dir);
        }
    }
    println!("  re-boards: {reboards} of {rows} rows");
    Ok(())
}
