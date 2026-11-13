//! (tickexact 2026-11-12, ironspec 18:59 (2b): the physical junction bar) `mctas mobai t-switch`: on
//! the official 26.3 server, copies 16 apart in a void, each a plain-rail T at (x0,Y,0): a straight
//! approach from the south (x0, Y, 1..12), branches east (x0+1..x0+5, Y, 0) and west (x0-5..x0-1,
//! Y, 0), all on stone, and a lever on the T's north side (on a stone block at (x0,Y,-2)) whose power
//! flips the T's curve (unpowered: south_east, as the game shapes it; powered: south_west). A cart
//! with a villager riding comes from the south at each `--speeds`. A first pass with no switch finds
//! A (the cart's first tick in the T's cell) and D (its last); then (i) a copy per offset s in
//! `--arrive` with the lever switched on at tick A + s (before that tick runs): which branch the
//! cart leaves by; (ii) a copy per offset s in `--depart` with the lever switched at D + s: whether
//! the cart's path differs from the unswitched one. Printed per copy: the switch tick, the branch,
//! the cart's block position each tick from A - 2 to D + 3.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;
const L: i32 = 12;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

/// One pass: a copy per trigger tick (None: no switch); every copy's cart position each tick, and
/// the tick the T first reads south_west (the switch) at a tick's end.
/// `cause`: `lever` (a lever on the T's north side switched by a command between ticks),
/// `repeater` (a repeater (x0,Y,-1) facing north into the T, its input a redstone block placed at
/// (x0,Y,-2): the T switches in the block-tick phase 2 gt later), `detector-after` / `detector-before`
/// (a detector rail (x0,Y+2,0) on a stone (x0,Y+1,0) over the T, pressed in the entity phase by a
/// cart summoned onto it at the trigger tick (after the test cart in the entity list) or teleported
/// onto it from a parking rail (summoned before the test cart)).
fn pass(version: &str, v: f64, cause: &str, switches: &[Option<u32>], ticks: u32) -> Result<(Vec<Vec<[f64; 3]>>, Vec<Option<u32>>)> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    let n = switches.len() as i32;
    srv.cmd_ok(&format!("forceload add -16 -16 {} 32", 16 * n + 16))?;
    srv.step(20)?;
    for k in 0..n {
        let x0 = 16 * k;
        srv.cmd_ok(&format!("fill {} {} -3 {} {} 0 minecraft:stone", x0 - 5, Y - 1, x0 + 5, Y - 1))?;
        srv.cmd_ok(&format!("fill {x0} {} 1 {x0} {} {L} minecraft:stone", Y - 1, Y - 1))?;
        for z in 1..=L {
            srv.cmd_ok(&format!("setblock {x0} {Y} {z} minecraft:rail[shape=north_south]"))?;
        }
        for dx in 1..=5 {
            srv.cmd_ok(&format!("setblock {} {Y} 0 minecraft:rail[shape=east_west]", x0 + dx))?;
            srv.cmd_ok(&format!("setblock {} {Y} 0 minecraft:rail[shape=east_west]", x0 - dx))?;
        }
        srv.cmd_ok(&format!("setblock {x0} {Y} 0 minecraft:rail[shape=south_east]"))?;
        match cause {
            "repeater" => {
                srv.cmd_ok(&format!("setblock {x0} {Y} -1 minecraft:repeater[facing=north,delay=1]"))?;
            }
            "detector-after" | "detector-before" => {
                // the detector on a stone right over the T: a rail in any of the T's neighbour
                // columns (north/south/east/west at Y-1..Y+1) would count as a 4th potential
                // connection, and RailBlock.updateState re-shapes only a 3-connection rail
                srv.cmd_ok(&format!("setblock {x0} {} 0 minecraft:stone", Y + 1))?;
                srv.cmd_ok(&format!("setblock {x0} {} 0 minecraft:detector_rail[shape=east_west]", Y + 2))?;
                if cause == "detector-before" {
                    // the trigger cart's parking rail, 3 west of the detector
                    srv.cmd_ok(&format!("setblock {} {} 0 minecraft:stone", x0 - 3, Y + 1))?;
                    srv.cmd_ok(&format!("setblock {} {} 0 minecraft:rail[shape=east_west]", x0 - 3, Y + 2))?;
                }
            }
            _ => {
                srv.cmd_ok(&format!("setblock {x0} {Y} -2 minecraft:stone"))?;
                srv.cmd_ok(&format!("setblock {x0} {Y} -1 minecraft:lever[face=wall,facing=south,powered=false]"))?;
            }
        }
    }
    srv.step(5)?;
    for k in 0..n {
        let x0 = 16 * k;
        if cause == "detector-before" {
            srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {}.0625 0.5 {{Tags:[\"trig{k}\"]}}", x0 - 3, Y + 2))?;
        }
    }
    for k in 0..n {
        let x0 = 16 * k;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {}.5 {}.0625 {}.5 {{Tags:[\"ts{k}\"],Motion:[0.0d,0.0d,{}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}}]}}",
            x0,
            Y,
            L - 1,
            -v
        ))?;
    }
    let mut out: Vec<Vec<[f64; 3]>> = vec![Vec::new(); n as usize];
    let mut switched: Vec<Option<u32>> = vec![None; n as usize];
    for t in 1..=ticks {
        for (k, s) in switches.iter().enumerate() {
            if *s == Some(t) {
                let x0 = 16 * k as i32;
                match cause {
                    "repeater" => srv.cmd_ok(&format!("setblock {x0} {Y} -2 minecraft:redstone_block"))?,
                    "detector-after" => srv.cmd_ok(&format!("summon minecraft:minecart {x0}.5 {}.0625 0.5 {{Tags:[\"trig{k}\"]}}", Y + 2))?,
                    "detector-before" => srv.cmd_ok(&format!("tp @e[tag=trig{k},limit=1] {x0}.5 {}.0625 0.5", Y + 2))?,
                    _ => srv.cmd_ok(&format!("setblock {x0} {Y} -1 minecraft:lever[face=wall,facing=south,powered=true]"))?,
                };
            }
        }
        srv.step(1)?;
        if std::env::var_os("MCTAS_TSWITCH_DEBUG").is_some() && cause.starts_with("detector") {
            for (k, s) in switches.iter().enumerate() {
                if let Some(s) = s
                    && t >= *s
                    && t <= s + 3
                {
                    let x0 = 16 * k as i32;
                    let det = srv.cmd_long(&format!("execute if block {x0} {} 0 minecraft:detector_rail[powered=true]", Y + 2))?;
                    let any = srv.cmd_long(&format!("execute if block {x0} {} 0 minecraft:detector_rail", Y + 2))?;
                    let pos = srv.cmd_long(&format!("data get entity @e[tag=trig{k},limit=1] Pos"))?;
                    let tshape = srv.cmd_long(&format!("execute if block {x0} {Y} 0 minecraft:rail[shape=south_east]"))?;
                    eprintln!("  dbg copy {k} t{t}: detector present {} powered {}; trigger cart {}; T south_east {}", any.contains("passed"), det.contains("passed"), pos.trim(), tshape.contains("passed"));
                }
            }
        }
        for k in 0..n as usize {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=ts{k},limit=1] Pos"))?);
            out[k].push(if p.len() == 3 { [p[0] - 16.0 * k as f64, p[1] - Y as f64, p[2]] } else { [f64::NAN; 3] });
            if switched[k].is_none() && srv.cmd_long(&format!("execute if block {} {Y} 0 minecraft:rail[shape=south_west]", 16 * k as i32))?.contains("passed") {
                switched[k] = Some(t);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok((out, switched))
}

fn cell(p: &[f64; 3]) -> [i32; 3] {
    [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]
}

pub fn t_switch(version: &str, speeds: &[f64], cause: &str, arrive: &[i32], depart: &[i32], ticks: u32) -> Result<()> {
    println!(
        "== t-switch on the official {version} server: a plain-rail T (south_east unpowered, south_west powered), the cause `{cause}`, a ridden cart from the south at {speeds:?}; triggers at A + {arrive:?} (A: the first tick the cart ends in the T's cell; its ticks that START in the cell are A+1..D+1) and at D + {depart:?} (D: the last tick it ends in the cell); per copy the tick the T first reads south_west at a tick's end (the switch)"
    );
    for &v in speeds {
        let (mut nominal, _) = pass(version, v, cause, &[None], ticks)?;
        let nominal = nominal.remove(0);
        let a = nominal.iter().position(|p| cell(p) == [0, 0, 0]).map(|i| i as u32 + 1);
        let d = nominal.iter().rposition(|p| cell(p) == [0, 0, 0]).map(|i| i as u32 + 1);
        let (Some(a), Some(d)) = (a, d) else {
            println!("  {v} b/gt: the cart never entered the T's cell");
            continue;
        };
        let branch = |path: &Vec<[f64; 3]>| -> &'static str {
            match path.last() {
                Some(p) if p[0] >= 1.0 => "east (the old shape)",
                Some(p) if p[0] < 0.0 => "WEST (the new shape)",
                _ => "neither",
            }
        };
        println!("  {v} b/gt, no switch: A t{a}, D t{d} (ticks starting in the cell: t{}..t{}); leaves {}", a + 1, d + 1, branch(&nominal));
        let show = |path: &Vec<[f64; 3]>| -> String {
            (a.saturating_sub(2).max(1)..=(d + 3).min(ticks))
                .map(|t| {
                    let p = path[t as usize - 1];
                    let c = cell(&p);
                    format!("t{t} ({},{},{}) z {:.3} x {:.3}", c[0], c[1], c[2], p[2], p[0])
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        println!("    {}", show(&nominal));
        let at = |base: u32, s: i32| -> Option<u32> { u32::try_from(base as i64 + s as i64).ok().filter(|&t| t >= 1) };
        // the switch tick against the cart's ticks that start in the cell (A+1..D+1)
        let rel = |sw: Option<u32>| -> String {
            match sw {
                Some(s) => format!("the T switched at t{s} (in the tick that starts {} the cart's {} tick in the cell)", if s <= d + 1 { "at or before" } else { "after" }, if s <= a + 1 { "first".to_string() } else if s <= d + 1 { format!("{}th", s - a) } else { "last".to_string() }),
                None => "the T never switched".into(),
            }
        };
        let sw: Vec<Option<u32>> = arrive.iter().map(|&s| at(a, s)).collect();
        let (paths, switched) = pass(version, v, cause, &sw, ticks)?;
        for (((s, t), path), swt) in arrive.iter().zip(&sw).zip(&paths).zip(&switched) {
            println!(
                "  {v} b/gt, (i) triggered at A {}{} (t{}): {}; leaves {}",
                if *s < 0 { "-" } else { "+" },
                s.abs(),
                t.unwrap_or(0),
                rel(*swt),
                branch(path)
            );
            println!("    {}", show(path));
        }
        let sw: Vec<Option<u32>> = depart.iter().map(|&s| at(d, s)).collect();
        let (paths, switched) = pass(version, v, cause, &sw, ticks)?;
        for (((s, t), path), swt) in depart.iter().zip(&sw).zip(&paths).zip(&switched) {
            let same = path.iter().zip(&nominal).all(|(p, q)| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9 && (p[2] - q[2]).abs() < 1e-9);
            println!(
                "  {v} b/gt, (ii) triggered at D {}{} (t{}): {}; {}; leaves {}",
                if *s < 0 { "-" } else { "+" },
                s.abs(),
                t.unwrap_or(0),
                rel(*swt),
                if same { "undisturbed (the path identical to the unswitched one)" } else { "DISTURBED" },
                branch(path)
            );
            if !same {
                println!("    {}", show(path));
            }
        }
    }
    Ok(())
}
