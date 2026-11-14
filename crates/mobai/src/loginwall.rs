//! (tickexact 2026-11-14, ironfarm 07:47 / ironspec 11:35 PT: the bench's world-open rows) `mctas mobai login-wall`: on the
//! official 26.3 server, carts at rest FLUSH against a glass wall (the cart's box face on the wall's face, its centre 0.49
//! from it): an EMPTY and a RIDDEN cart on a flat rail with the wall across its end; an EMPTY and a RIDDEN cart on a BRAKED
//! slope (an unpowered powered rail ascending north, the wall at its low south end: the slope pushes the cart into the
//! wall, the brake zeroes it); and an EMPTY cart on a plain-rail slope against the same kind of wall (no brake: the slope
//! pushes it into the wall every tick). Each settles 40 gt; (G1 car-13 CHANGES 6 (1)(2)) beside them a MOVING TWIN, an empty
//! cart rolling on a long flat rail away from any wall (summoned at 0.3 b/t 2 gt before the save), which is the reload's
//! probe. Then a world reopen (save, stop, start with the chunks forceloaded and no player: `restartcheck::reload`, its
//! verdict and game-time jump printed), and every cart's position and Motion at the save and every tick for 20 gt after
//! the open, each compared EXACTLY with its saved state. The glass walls and the riders (seated, NoAI) are read back.
//! Asserted (the first run's answers; MCTAS_LOGIN_WALL_PLANT=k inverts assert k, read strictly 1..=6; a planted run that
//! never reaches its assert fails, "control planted nothing"): (1) at the save every walled cart rests flush against its
//! wall (its centre 0.49 from the wall's face) and every wall reads back glass; (2) after the open no walled cart's
//! position changes on any of the 20 ticks (compared exactly); (3) the flat and braked carts keep Motion 0 on every tick;
//! (4) each ridden cart carries its villager, NoAI read back, at the save and at every read after the open; (5) the reload
//! is clean by its own verdict (`Reload::clean`) with the moving twin as its probe, which was moving at the save and held
//! across the load; (6) the moving twin moves on every one of the 20 ticks after the open, its first step equal to its
//! saved Motion (within 1e-9: the game's own per-tick step), so a cart that moves IS seen moving through the same reload.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_LOGIN_WALL_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_LOGIN_WALL_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=6).contains(&k), "MCTAS_LOGIN_WALL_PLANT={k}: login-wall has asserts 1..6");
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
    srv.cmd_ok("forceload add -8 -32 32 32")?;
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
    // the moving twin's line: a flat rail on stone at x 24 from z -30 to 30, nothing at either end within its roll
    const TX: i32 = 24;
    srv.cmd(&format!("fill {} {} -31 {} {} 31 minecraft:air", TX - 1, Y - 1, TX + 1, Y + 3))?;
    srv.cmd(&format!("fill {TX} {} -31 {TX} {} 31 minecraft:stone", Y - 1, Y - 1))?;
    srv.cmd(&format!("fill {TX} {Y} -30 {TX} {Y} 30 minecraft:rail[shape=north_south,waterlogged=false]"))?;
    srv.step(10)?;
    for s in &specs {
        if let Some(state) = s.slope {
            anyhow::ensure!(srv.test_block([s.x, Y, 0], state)?, "{}: the slope at ({},{Y},0) is not {state}", s.label, s.x);
        }
    }
    // (G1 CHANGES 6 (1)) the walls read back
    let walls_ok = |srv: &mut VanillaServer| -> Result<bool> {
        let mut ok = true;
        for s in &specs {
            ok &= srv.test_block([s.x, Y, 1], "minecraft:glass")? && srv.test_block([s.x, Y + 1, 1], "minecraft:glass")?;
        }
        Ok(ok)
    };
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
    srv.step(38)?;
    // the moving twin, 2 gt before the save: an empty cart at 0.3 b/t north-to-south on the long line
    srv.cmd_ok(&format!("summon minecraft:minecart {TX}.5 {}.0625 -20.5 {{Tags:[\"lwm\"],Motion:[0.0d,0.0d,0.3d]}}", Y))?;
    srv.step(2)?;
    let read = |srv: &mut VanillaServer, k: usize| -> Result<(Vec<f64>, Vec<f64>)> {
        Ok((nums(&srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Pos"))?), nums(&srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Motion"))?)))
    };
    let read_twin = |srv: &mut VanillaServer| -> Result<(Vec<f64>, Vec<f64>)> {
        Ok((nums(&srv.cmd_long("data get entity @e[tag=lwm,limit=1] Pos")?), nums(&srv.cmd_long("data get entity @e[tag=lwm,limit=1] Motion")?)))
    };
    let mut saved: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
    for (k, s) in specs.iter().enumerate() {
        let (p, m) = read(&mut srv, k)?;
        println!("  at the save: {} pos {:?} motion {:?}", s.label, p, m);
        saved.push((p, m));
    }
    let twin_saved = read_twin(&mut srv)?;
    println!("  at the save: the MOVING TWIN pos {:?} motion {:?}", twin_saved.0, twin_saved.1);
    anyhow::ensure!(twin_saved.0.len() == 3 && twin_saved.1.len() == 3, "the moving twin is not in the world at the save");
    // (ironspec 08:18a (1), G1 CHANGES 6 (1)) each ridden cart read back carrying its villager, NoAI, at the save and at
    // every read after the open
    let carries = |srv: &mut VanillaServer, k: usize| -> Result<bool> {
        let p = srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Passengers"))?;
        let n = srv.cmd_long(&format!("data get entity @e[tag=lw{k},limit=1] Passengers[0].NoAI"))?;
        Ok(p.contains("minecraft:villager") && n.contains("1b"))
    };
    let mut seated_at_save = Vec::new();
    for (k, s) in specs.iter().enumerate() {
        if s.ridden {
            seated_at_save.push((k, carries(&mut srv, k)?));
        }
    }
    let walls_at_save = walls_ok(&mut srv)?;
    let flush = walls_at_save && saved.iter().all(|(p, _)| p.len() == 3 && p[2] == 0.51);
    let (mut srv, rl) = crate::restartcheck::reload(srv, |w| opts(Some(w)), Some("@e[tag=lwm,limit=1]"))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("forceload add -8 -32 32 32")?;
    println!("  WORLD OPEN: {} (game time +{}; the moving twin the probe: held {:?}, at rest at the save {})", rl.describe(), rl.time_jump, rl.held, rl.probe_at_rest);
    let walls_after = walls_ok(&mut srv)?;
    println!("  the walls read back glass at the save: {walls_at_save}, after the open: {walls_after}");
    let mut moved = vec![0u32; specs.len()];
    let mut spun = vec![0u32; specs.len()];
    let mut seated = vec![0u32; specs.len()];
    let mut last: Vec<(Vec<f64>, Vec<f64>)> = saved.clone();
    let mut twin_prev = twin_saved.0.clone();
    let mut twin_moves = 0u32;
    let mut twin_first_step: Option<f64> = None;
    for t in 0..=20u32 {
        if t > 0 {
            srv.step(1)?;
        }
        let mut line = format!("    t+{t:>2}:");
        for (k, s) in specs.iter().enumerate() {
            let (p, m) = read(&mut srv, k)?;
            // (G1 CHANGES 6 (2)) compared exactly: any change, or a read that did not parse, counts
            let same = p.len() == 3 && p == saved[k].0;
            let dp = if p.len() == 3 && saved[k].0.len() == 3 { (0..3).map(|i| (p[i] - saved[k].0[i]).abs()).fold(0.0, f64::max) } else { f64::INFINITY };
            let sp = if m.len() == 3 { (m[0] * m[0] + m[2] * m[2]).sqrt() } else { f64::INFINITY };
            if !same {
                moved[k] += 1;
            }
            if m.len() != 3 || m.iter().any(|&v| v != 0.0) {
                spun[k] += 1;
            }
            if s.ridden && carries(&mut srv, k)? {
                seated[k] += 1;
            }
            line.push_str(&format!(" | {}: dpos {dp:.6} speed {sp:.6}", &s.label[..s.label.find(" on ").unwrap_or(s.label.len())]));
            last[k] = (p, m);
        }
        let (tp, tm) = read_twin(&mut srv)?;
        if t > 0 {
            if tp.len() == 3 && tp != twin_prev {
                twin_moves += 1;
            }
            if t == 1 && tp.len() == 3 && twin_prev.len() == 3 {
                twin_first_step = Some(tp[2] - twin_prev[2]);
            }
        }
        line.push_str(&format!(" | TWIN: z {} vz {}", tp.get(2).map(|z| format!("{z:.6}")).unwrap_or_else(|| "gone".into()), tm.get(2).map(|v| format!("{v:.6}")).unwrap_or_else(|| "-".into())));
        if tp.len() == 3 {
            twin_prev = tp;
        }
        println!("{line}");
    }
    for (k, s) in specs.iter().enumerate() {
        println!("  => {}: moved on {} of 21 reads, nonzero Motion on {} of 21; last pos {:?} motion {:?}", s.label, moved[k], spun[k], last[k].0, last[k].1);
    }
    println!("  => the MOVING TWIN: moved on {twin_moves} of 20 ticks; its first step after the open {:?} vs its saved Motion z {:.9}", twin_first_step, twin_saved.1[2]);
    let _ = std::fs::remove_dir_all(&srv.dir);
    pin(1, flush, format!("(1) at the save every walled cart rests flush against its wall (its centre z 0.51 exactly, 0.49 from the wall's face) and every wall reads back glass ({walls_at_save})"))?;
    pin(2, moved.iter().all(|&n| n == 0) && walls_after, "(2) after the world open no walled cart's position changes on any of the 20 ticks (nor at the open; compared exactly), the walls still glass".into())?;
    pin(3, specs.iter().zip(&spun).filter(|(s, _)| s.zero_motion).all(|(_, &n)| n == 0), "(3) the flat and braked carts keep Motion 0 exactly at the open and on every tick after it".into())?;
    let ridden_ok = seated_at_save.len() == 2 && seated_at_save.iter().all(|(k, s)| *s && seated[*k] == 21);
    pin(4, ridden_ok, format!("(4) each ridden cart is read back carrying its villager, NoAI 1b, at the save and at all 21 reads after the open: at the save {seated_at_save:?}, after it {:?}", specs.iter().enumerate().filter(|(_, s)| s.ridden).map(|(k, _)| seated[k]).collect::<Vec<_>>()))?;
    pin(5, rl.clean() && rl.held == Some(true) && !rl.probe_at_rest, format!("(5) the reload is clean by its own verdict with the moving twin as its probe (moving at the save, held across the load): {}", rl.describe()))?;
    let step_ok = twin_first_step.is_some_and(|s| (s - twin_saved.1[2]).abs() < 1e-9);
    pin(6, twin_moves == 20 && step_ok, format!("(6) the moving twin moves on every one of the 20 ticks after the open ({twin_moves}), its first step {twin_first_step:?} equal to its saved Motion z {:.9}", twin_saved.1[2]))?;
    if let Some(k) = plant()? {
        anyhow::bail!("control planted nothing: MCTAS_LOGIN_WALL_PLANT={k} never failed its assert");
    }
    println!("  every known answer held (asserts 1..6)");
    Ok(())
}
