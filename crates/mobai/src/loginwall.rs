//! (tickexact 2026-11-14, ironfarm 07:47 / ironspec 11:35 PT: the bench's world-open rows) `mctas mobai login-wall`: on the
//! official 26.3 server, carts at rest FLUSH against a glass wall (the cart's box face on the wall's face, its centre 0.49
//! from it): an EMPTY and a RIDDEN cart on a flat rail with the wall across its end; an EMPTY and a RIDDEN cart on a BRAKED
//! slope (an unpowered powered rail ascending north, the wall at its low south end: the slope pushes the cart into the
//! wall, the brake zeroes it); and an EMPTY cart on a plain-rail slope against the same kind of wall (no brake: the slope
//! pushes it into the wall every tick). Each settles 40 gt; then a real world open (save, stop, start:
//! `restartcheck::reload`, its game-time jump printed), and every cart's position and Motion at the save and every tick for
//! 20 gt after the open, with each cart's position and speed change against its saved state.
//! Asserted (the first run's answers; MCTAS_LOGIN_WALL_PLANT=k inverts assert k, read strictly 1..=3): (1) at the save every
//! cart rests flush against its wall (its centre 0.49 from the wall's face); (2) after the open no cart's position changes on
//! any of the 20 ticks; (3) the flat and braked carts keep Motion 0 on every tick (speed change +0.0000).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_LOGIN_WALL_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_LOGIN_WALL_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=3).contains(&k), "MCTAS_LOGIN_WALL_PLANT={k}: login-wall has asserts 1..3");
            Ok(Some(k))
        }
    }
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect()
}

/// One cart: its label, x of its copy, the rail under it, ridden, braked (its motion must stay 0)
struct Spec {
    label: &'static str,
    x: i32,
    slope: Option<&'static str>,
    ridden: bool,
    zero_motion: bool,
}

pub fn login_wall(version: &str) -> Result<()> {
    plant()?;
    let specs = [
        Spec { label: "EMPTY on a flat rail", x: 0, slope: None, ridden: false, zero_motion: true },
        Spec { label: "RIDDEN on a flat rail", x: 4, slope: None, ridden: true, zero_motion: true },
        Spec { label: "EMPTY on a braked slope", x: 8, slope: Some("minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]"), ridden: false, zero_motion: true },
        Spec { label: "RIDDEN on a braked slope", x: 12, slope: Some("minecraft:powered_rail[powered=false,shape=ascending_north,waterlogged=false]"), ridden: true, zero_motion: true },
        Spec { label: "EMPTY on a plain-rail slope (no brake)", x: 16, slope: Some("minecraft:rail[shape=ascending_north,waterlogged=false]"), ridden: false, zero_motion: false },
    ];
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        tick_rate: 1000,
        keep_dir: true,
        world,
        ..Default::default()
    };
    println!("== login-wall on the official {version} server: carts at rest flush against glass walls, then a world open (save, stop, start)");
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("forceload add -8 -8 24 8")?;
    srv.step(20)?;
    for s in &specs {
        let x = s.x;
        srv.cmd(&format!("fill {} {} -4 {} {} 4 minecraft:air", x - 1, Y - 1, x + 1, Y + 3))?;
        srv.cmd(&format!("fill {x} {} -4 {x} {} 2 minecraft:stone", Y - 1, Y - 1))?;
        srv.cmd(&format!("fill {x} {Y} -3 {x} {Y} -1 minecraft:rail[shape=north_south,waterlogged=false]"))?;
        match s.slope {
            // the slope's high end is north (z -1 .. 0 rises to y Y+1 at z 0's north edge); its low end meets the wall
            Some(state) => {
                srv.cmd(&format!("setblock {x} {Y} -1 minecraft:stone"))?;
                srv.cmd(&format!("setblock {x} {} -1 minecraft:rail[shape=north_south,waterlogged=false]", Y + 1))?;
                srv.cmd(&format!("setblock {x} {Y} 0 {state}"))?;
            }
            None => {
                srv.cmd(&format!("setblock {x} {Y} 0 minecraft:rail[shape=north_south,waterlogged=false]"))?;
            }
        }
        srv.cmd(&format!("setblock {x} {Y} 1 minecraft:glass"))?;
        srv.cmd(&format!("setblock {x} {} 1 minecraft:glass", Y + 1))?;
    }
    srv.step(10)?;
    for s in &specs {
        if let Some(state) = s.slope {
            anyhow::ensure!(srv.test_block([s.x, Y, 0], state)?, "{}: the slope at ({},{Y},0) is not {state}", s.label, s.x);
        }
    }
    // each cart flush against its wall: the wall's face at z 1, the cart's centre at z 0.51 (its box 0.98 wide)
    for (k, s) in specs.iter().enumerate() {
        let z = 0.51;
        let y = match s.slope {
            Some(_) => Y as f64 + (1.0 - z) + 0.0625,
            None => Y as f64 + 0.0625,
        };
        let rider = if s.ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}]".to_string() } else { String::new() };
        srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {y:.4} {z} {{Tags:[\"lw{k}\"],Motion:[0.0d,0.0d,0.0d]{rider}}}", s.x))?;
    }
    srv.step(40)?;
    let read = |srv: &mut VanillaServer, k: usize| -> Result<(Vec<f64>, Vec<f64>)> {
        Ok((nums(&srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Pos"))?), nums(&srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Motion"))?)))
    };
    let mut saved: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
    for (k, s) in specs.iter().enumerate() {
        let (p, m) = read(&mut srv, k)?;
        println!("  at the save: {} pos {:?} motion {:?}", s.label, p, m);
        saved.push((p, m));
    }
    let flush = saved.iter().all(|(p, _)| p.len() == 3 && (p[2] - 0.51).abs() < 1e-9);
    let (mut srv, rl) = crate::restartcheck::reload(srv, |w| opts(Some(w)), Some("@e[tag=lw0,limit=1]"))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("forceload add -8 -8 24 8")?;
    println!("  WORLD OPEN: {} (game time +{})", rl.describe(), rl.time_jump);
    let mut moved = vec![0u32; specs.len()];
    let mut spun = vec![0u32; specs.len()];
    let mut last: Vec<(Vec<f64>, Vec<f64>)> = saved.clone();
    for t in 0..=20u32 {
        if t > 0 {
            srv.step(1)?;
        }
        let mut line = format!("    t+{t:>2}:");
        for (k, s) in specs.iter().enumerate() {
            let (p, m) = read(&mut srv, k)?;
            let dp = if p.len() == 3 && saved[k].0.len() == 3 { (0..3).map(|i| (p[i] - saved[k].0[i]).abs()).fold(0.0, f64::max) } else { f64::INFINITY };
            let sp = if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { f64::INFINITY };
            if dp > 1e-12 {
                moved[k] += 1;
            }
            if m.iter().any(|v| v.abs() > 1e-12) || m.len() != 3 {
                spun[k] += 1;
            }
            line.push_str(&format!(" | {}: dpos {dp:.6} speed {sp:.6}", &s.label[..s.label.find(" on ").unwrap_or(s.label.len())]));
            last[k] = (p, m);
        }
        println!("{line}");
    }
    for (k, s) in specs.iter().enumerate() {
        println!("  => {}: moved on {} of 21 reads, nonzero Motion on {} of 21; last pos {:?} motion {:?}", s.label, moved[k], spun[k], last[k].0, last[k].1);
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    pin(1, flush, "(1) at the save every cart rests flush against its wall (its centre z 0.51, 0.49 from the wall's face)".into())?;
    pin(2, moved.iter().all(|&n| n == 0), "(2) after the world open no cart's position changes on any of the 20 ticks (nor at the open)".into())?;
    pin(3, specs.iter().zip(&spun).filter(|(s, _)| s.zero_motion).all(|(_, &n)| n == 0), "(3) the flat and braked carts keep Motion 0 at the open and on every tick after it (speed change +0.0000)".into())?;
    println!("  every known answer held (asserts 1..3)");
    Ok(())
}
