//! (tickexact 2026-11-14, ironfarm 08:10 (4) / G1 07:56b (3): air for redsim's `air_tick`) `mctas mobai rider-air`: on the
//! official 26.3 server, a RIDDEN minecart held still underwater in a 1x1 glass shaft, its villager's eye 1.7 above the
//! cart: (a) over MAGMA, the water a DOWNWARD bubble column (`bubble_column[drag=true]`) that pulls the cart onto the
//! floor, so the rider's eye cell is a bubble-column block; (b) over STONE, plain water source blocks, the cart sunk onto
//! the floor. Every tick for 540 gt: the villager's Air and Health, the block at its eye, the cart's position. The 26.3
//! rule (LivingEntity.baseTick 440-462): with the eye in water and the eye's block NOT a bubble column, Air -1 a tick
//! (decreaseAirSupply 590-600; max 300, Entity 2892-2894); at <= -20 (shouldTakeDrowningDamage 508-510) Air resets to 0
//! and 2 drowning damage; with the eye out of water or in a bubble column, Air +4 a tick to 300 (602-604).
//! Asserted (the first run's answers; MCTAS_RIDER_AIR_PLANT=k inverts assert k, read strictly 1..=3): (1) in the bubble
//! column the rider's eye is in a bubble-column block every tick, its Air stays 300 and its Health 20.0; (2) in plain
//! water its Air falls by 1 every tick from 300 and its first drowning hit (Health 18.0) lands on the 320th tick; (3) in
//! plain water every later hit lands 20 ticks after the one before (2 damage each) until it dies.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_RIDER_AIR_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_RIDER_AIR_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=3).contains(&k), "MCTAS_RIDER_AIR_PLANT={k}: rider-air has asserts 1..3");
            Ok(Some(k))
        }
    }
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

fn num(s: &str) -> Option<f64> {
    s.split("entity data: ").nth(1).and_then(|t| t.trim().trim_end_matches(|c: char| c.is_alphabetic()).parse().ok())
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect()
}

pub fn rider_air(version: &str) -> Result<()> {
    plant()?;
    println!("== rider-air on the official {version} server: a ridden cart held underwater, its rider's eye in a bubble column (over magma) and in plain water (over stone)");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    srv.cmd_ok("forceload add -8 -8 16 8")?;
    srv.step(20)?;
    // (copy, x, floor)
    let copies = [("BUBBLE COLUMN (over magma)", 0, "minecraft:magma_block"), ("PLAIN WATER (over stone)", 6, "minecraft:stone")];
    for (_, x, floor) in copies {
        srv.cmd(&format!("fill {} {} -2 {} {} 2 minecraft:glass", x - 2, Y - 2, x + 2, Y + 6))?;
        srv.cmd(&format!("setblock {x} {} 0 {floor}", Y - 1))?;
        srv.cmd(&format!("fill {x} {Y} 0 {x} {} 0 minecraft:water", Y + 4))?;
    }
    srv.step(20)?;
    anyhow::ensure!(srv.test_block([0, Y + 1, 0], "minecraft:bubble_column[drag=true]")?, "the magma copy's column at (0,{},0) is not a downward bubble column", Y + 1);
    anyhow::ensure!(srv.test_block([6, Y + 1, 0], "minecraft:water")?, "the stone copy's column at (6,{},0) is not water", Y + 1);
    for (k, (_, x, _)) in copies.iter().enumerate() {
        srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 {Y}.0 0.5 {{Tags:[\"ra{k}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"ra{k}v\"]}}]}}"))?;
    }
    // per copy: per tick (air, health, eye block is a bubble column, cart y)
    let mut rows: Vec<Vec<(Option<f64>, Option<f64>, Option<bool>, Option<f64>)>> = vec![Vec::new(); copies.len()];
    let mut seated = vec![0u32; copies.len()];
    let mut seated_alive = vec![0u32; copies.len()];
    for t in 1..=540u32 {
        srv.step(1)?;
        let mut line = format!("    t{t:>3}:");
        for (k, (label, x, _)) in copies.iter().enumerate() {
            let air = num(&srv.cmd_long(&format!("data get entity @e[tag=ra{k}v,limit=1] Air"))?);
            let hp = num(&srv.cmd_long(&format!("data get entity @e[tag=ra{k}v,limit=1] Health"))?);
            let vp = nums(&srv.cmd_long(&format!("data get entity @e[tag=ra{k}v,limit=1] Pos"))?);
            let cp = nums(&srv.cmd_long(&format!("data get entity @e[tag=ra{k},limit=1] Pos"))?);
            // (ironspec 08:18a (1)) the rider read back SEATED in its own cart, every tick it is alive (a dead villager
            // stays seated through its 20-tick death before it is removed)
            if srv.cmd_long(&format!("data get entity @e[tag=ra{k},limit=1] Passengers"))?.contains(&format!("\"ra{k}v\"")) {
                seated[k] += 1;
                if hp.is_some() {
                    seated_alive[k] += 1;
                }
            }
            // a seated villager's eye: its feet + 1.62 (the villager's standing eye height; seated it keeps it)
            let eye = if vp.len() == 3 { Some([vp[0].floor() as i32, (vp[1] + 1.62).floor() as i32, vp[2].floor() as i32]) } else { None };
            let in_column = match eye {
                Some(e) => Some(srv.test_block(e, "minecraft:bubble_column")?),
                None => None,
            };
            if t <= 3 || t % 20 == 0 || (318..=324).contains(&t) {
                line.push_str(&format!(
                    " | {}: Air {} HP {} eye {} cart y {}",
                    &label[..label.find(' ').unwrap_or(label.len())],
                    air.map(|a| format!("{a:.0}")).unwrap_or_else(|| "-".into()),
                    hp.map(|h| format!("{h:.1}")).unwrap_or_else(|| "-".into()),
                    match (eye, in_column) {
                        (Some(e), Some(b)) => format!("({},{},{}) {}", e[0] - x, e[1], e[2], if b { "bubble column" } else { "not a bubble column" }),
                        _ => "-".into(),
                    },
                    cp.get(1).map(|y| format!("{y:.4}")).unwrap_or_else(|| "-".into())
                ));
            }
            rows[k].push((air, hp, in_column, cp.get(1).copied()));
        }
        if line.contains('|') {
            println!("{line}");
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    // the plain-water copy's hits: the ticks its Health drops
    let hits: Vec<u32> = rows[1].windows(2).enumerate().filter(|(_, w)| matches!((w[0].1, w[1].1), (Some(a), Some(b)) if b < a) || (w[0].1.is_some() && w[1].1.is_none())).map(|(i, _)| i as u32 + 2).collect();
    println!("  PLAIN WATER: Health drops at ticks {hits:?}; seated: BUBBLE {} of 540 ticks, PLAIN {} of 540 ({} of the {} ticks its villager read alive)", seated[0], seated[1], seated_alive[1], rows[1].iter().filter(|r| r.1.is_some()).count());
    let bubble = &rows[0];
    let ok1 = bubble.len() == 540 && bubble.iter().all(|r| r.0 == Some(300.0) && r.1 == Some(20.0) && r.2 == Some(true)) && seated[0] == 540;
    let plain = &rows[1];
    let falls = (0..319).all(|i| plain[i].0 == Some(299.0 - i as f64));
    let alive = rows[1].iter().filter(|r| r.1.is_some()).count() as u32;
    let ok2 = falls && hits.first() == Some(&320) && plain[319].1 == Some(18.0) && seated_alive[1] == alive;
    let ok3 = hits.len() >= 2 && hits.windows(2).all(|w| w[1] - w[0] == 20);
    pin(1, ok1, "(1) in the downward bubble column the rider, read back SEATED every tick, has its eye in a bubble-column block every tick, its Air stays 300 and its Health 20.0 (540 ticks)".into())?;
    pin(2, ok2, "(2) in plain water the rider, read back SEATED on every tick it lives, loses 1 Air every tick from 300 (299 at the first tick) and takes its first drowning hit (Health 18.0) on the 320th tick".into())?;
    pin(3, ok3, format!("(3) in plain water every later drowning hit lands 20 ticks after the one before: {hits:?}"))?;
    println!("  every known answer held (asserts 1..3)");
    Ok(())
}
