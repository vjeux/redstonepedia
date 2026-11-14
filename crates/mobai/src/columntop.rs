//! (tickexact 2026-11-14, ironfarm 09:15 / 09:43, vcompose 09:20, ironspec 09:17a (1) / 09:22a (1), G1 09:43 (3): the lift
//! column's top cell and a stranded riser's air) `mctas mobai column-top <design>`: on the official 26.3 server, the design
//! pasted at its own coordinates (no entities), settled 100 gt with its column running, `--set x,y,z=STATE` cells changed
//! after the paste (a candidate fix, each read back after 100 gt); then for each probed cell (default: the lift column
//! (-5,16..26,7) and the exit curve (-3,25,9)) the design's own cell beside the game's live block, a fluid named exactly
//! (`water[level=…]` or `bubble_column[drag=…]`, tested by `execute if block`).
//! With `--held x,y,z --riser x,y,z [--risers N]`: ironfarm's idealized stop (09:43: a riderless cart that cannot move,
//! not a buildable stop): an empty cart at the held position, put back there with Motion 0 after EVERY tick (`tp` and
//! `data merge`; how far the tick had moved it printed); and N carts each ridden by a villager, summoned in the column at
//! the riser position and 2 blocks below one another, rising and queueing behind it. Every tick for `--ticks` gt, per
//! rider: its cart's position, its eye cell (feet + 1.62: 26.3 seats a villager's feet AT its cart's position, seat-offset;
//! the villager's eye height 1.62, EntityTypes 1068 `eyeHeight(1.62F)`), the live block there, its Air and Health. The 26.3
//! rules the verdicts rest on: Air -1 a tick only with the eye in water that is not a bubble column (LivingEntity.baseTick
//! 440-462; 2 damage at -20, 508-510), +4 a tick otherwise (602-604); an eye inside glass takes no in-wall damage and
//! breathes (Blocks.GLASS 732-742 `isSuffocating(Blocks::never)`; Entity.isInWall 2313-2329 counts only suffocating blocks).
//! Asserted (MCTAS_COLUMN_TOP_PLANT=k inverts assert k, read strictly 1..=3; a planted run that never reaches its assert
//! fails, "control planted nothing"): (1) every probed cell the design lays as a fluid holds a fluid in the game (the paste
//! and the settle emptied nothing) and `--expect-top STATE` (when given) is the column's top fluid cell; (2) with risers:
//! every rider is read back seated on every tick it lives; (3) with `--expect-air V1,V2,…` (one per rider, `drowns` or
//! `breathes`): `drowns` = its stranded eye (the last 100 ticks' eye cell) is in PLAIN water and its Air reaches -19 or
//! below with a Health drop; `breathes` = its Air never falls below 300 after its first 40 ticks and it takes no damage.

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_COLUMN_TOP_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_COLUMN_TOP_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=3).contains(&k), "MCTAS_COLUMN_TOP_PLANT={k}: column-top has asserts 1..3");
            Ok(Some(k))
        }
    }
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

fn cell(s: &str) -> Result<[i32; 3]> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>().map_err(|_| anyhow::anyhow!("not a cell x,y,z: {s}"))?;
    anyhow::ensure!(v.len() == 3, "not a cell x,y,z: {s}");
    Ok([v[0], v[1], v[2]])
}

fn point(s: &str) -> Result<[f64; 3]> {
    let v: Vec<f64> = s.split(',').map(|t| t.trim().parse()).collect::<Result<_, _>>().map_err(|_| anyhow::anyhow!("not a position x,y,z: {s}"))?;
    anyhow::ensure!(v.len() == 3, "not a position x,y,z: {s}");
    Ok([v[0], v[1], v[2]])
}

/// The live block at `p` when it is a fluid or air, named exactly; `None` for any other block
pub fn fluid_at(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<String>> {
    for drag in ["true", "false"] {
        let s = format!("minecraft:bubble_column[drag={drag}]");
        if srv.test_block(p, &s)? {
            return Ok(Some(s));
        }
    }
    if srv.test_block(p, "minecraft:water")? {
        for level in 0..16 {
            let s = format!("minecraft:water[level={level}]");
            if srv.test_block(p, &s)? {
                return Ok(Some(s));
            }
        }
    }
    if srv.test_block(p, "minecraft:air")? {
        return Ok(Some("minecraft:air".into()));
    }
    Ok(None)
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect()
}

fn num(s: &str) -> Option<f64> {
    s.split("entity data: ").nth(1).and_then(|t| t.trim().trim_end_matches(|c: char| c.is_alphabetic()).parse().ok())
}

/// One rider's record: per tick (Air, Health, the eye cell, its block), seated-and-alive ticks, alive ticks
#[derive(Default)]
struct Rider {
    airs: Vec<Option<f64>>,
    eyes: Vec<Option<([i32; 3], String)>>,
    seated_alive: u32,
    alive: u32,
    hp_drop: bool,
    last_hp: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn column_top(version: &str, design: &std::path::Path, cells: &[String], sets: &[String], riser: Option<&str>, risers: u32, held: Option<&str>, ticks: u32, expect_top: Option<&str>, expect_air: Option<&str>) -> Result<()> {
    plant()?;
    anyhow::ensure!(held.is_none() || riser.is_some(), "--held needs risers (--riser)");
    anyhow::ensure!((1..=4).contains(&risers), "--risers is 1 .. 4");
    let expect: Vec<String> = expect_air.map(|s| s.split(',').map(|t| t.trim().to_string()).collect()).unwrap_or_default();
    anyhow::ensure!(expect.is_empty() || riser.is_some(), "--expect-air needs risers");
    anyhow::ensure!(expect.is_empty() || expect.len() == risers as usize, "--expect-air takes one verdict per rider ({risers})");
    anyhow::ensure!(expect.iter().all(|e| e == "drowns" || e == "breathes"), "--expect-air verdicts are drowns or breathes");
    let sets: Vec<([i32; 3], String)> = sets
        .iter()
        .map(|s| -> Result<([i32; 3], String)> {
            let (c, st) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--set is x,y,z=STATE, got {s}"))?;
            Ok((cell(c)?, st.trim().to_string()))
        })
        .collect::<Result<_>>()?;
    let probes: Vec<[i32; 3]> = if cells.is_empty() { (16..=26).map(|y| [-5, y, 7]).chain([[-3, 25, 9]]).collect() } else { cells.iter().map(|s| cell(s)).collect::<Result<_>>()? };
    let d = mctas::design::Design::load(design)?;
    let blocks = d.block_list();
    let own: std::collections::BTreeMap<[i32; 3], String> = blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
    println!("== column-top on the official {version} server: {} ({} cells) pasted at its own coordinates (no entities)", design.display(), blocks.len());
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    let [x0, z0, x1, z1] = crate::portal::chunk_box(&blocks, 8);
    srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
    srv.step(40)?;
    crate::portal::clear_box(&mut srv, "minecraft:overworld", &blocks, 2)?;
    crate::portal::paste(&mut srv, "minecraft:overworld", &blocks)?;
    srv.step(100)?;
    for (p, st) in &sets {
        srv.cmd_ok(&format!("setblock {} {} {} {st}", p[0], p[1], p[2]))?;
    }
    if !sets.is_empty() {
        srv.step(100)?;
        for (p, st) in &sets {
            anyhow::ensure!(srv.test_block(*p, st)?, "--set ({},{},{})={st} did not hold through 100 gt", p[0], p[1], p[2]);
            println!("  set ({},{},{}) = {st} (the design lays {}), read back after 100 gt", p[0], p[1], p[2], own.get(p).map(String::as_str).unwrap_or("air (not written)"));
        }
    }
    let mut emptied = Vec::new();
    for p in &probes {
        let mine = own.get(p).map(String::as_str).unwrap_or("air (not written)");
        let live = fluid_at(&mut srv, *p)?;
        println!("  ({},{},{}): the design lays {mine}; the game holds {}", p[0], p[1], p[2], live.as_deref().unwrap_or("a non-fluid block (see the design's cell)"));
        let laid_fluid = mine.starts_with("minecraft:water") || mine.starts_with("minecraft:bubble_column");
        if laid_fluid && !live.as_deref().is_some_and(|l| l.starts_with("minecraft:water") || l.starts_with("minecraft:bubble_column")) {
            emptied.push(*p);
        }
    }
    let col = probes[0];
    let mut top: Option<([i32; 3], String)> = None;
    for p in probes.iter().filter(|p| p[0] == col[0] && p[2] == col[2]) {
        if let Some(l) = fluid_at(&mut srv, *p)? {
            if l != "minecraft:air" {
                top = Some((*p, l));
            }
        }
    }
    if let Some((p, l)) = &top {
        println!("  the column's top fluid cell: ({},{},{}) {l}", p[0], p[1], p[2]);
    }
    let top_ok = match expect_top {
        None => true,
        Some(want) => top.as_ref().is_some_and(|(_, l)| l == want),
    };
    pin(1, emptied.is_empty() && top_ok, format!("(1) every probed cell the design lays as a fluid holds a fluid in the game (emptied: {emptied:?}){}", expect_top.map(|w| format!("; the column's top fluid cell is {w}")).unwrap_or_default()))?;
    if let Some(r) = riser {
        let r = point(r)?;
        let h = held.map(point).transpose()?;
        if let Some(h) = h {
            srv.cmd_ok(&format!("summon minecraft:minecart {:.4} {:.4} {:.4} {{Tags:[\"cth\"],Motion:[0.0d,0.0d,0.0d]}}", h[0], h[1], h[2]))?;
            anyhow::ensure!(nums(&srv.cmd_long("data get entity @e[tag=cth,limit=1] Pos")?).len() == 3, "the held cart is not in the world after its summon");
        }
        for i in 0..risers {
            let y = r[1] - 2.0 * i as f64;
            srv.cmd_ok(&format!("summon minecraft:minecart {:.4} {y:.4} {:.4} {{Tags:[\"ctr{i}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"ctr{i}v\"]}}]}}", r[0], r[2]))?;
            anyhow::ensure!(srv.cmd_long(&format!("data get entity @e[tag=ctr{i},limit=1] Passengers"))?.contains(&format!("\"ctr{i}v\"")), "riser {i} was summoned ridden but the game reads back no rider");
            println!("  riser {i} summoned at ({:.4},{y:.4},{:.4}), its villager read back seated", r[0], r[2]);
        }
        match h {
            Some(h) => println!("  the held cart: an empty cart put back at ({:.4},{:.4},{:.4}) with Motion 0 after every tick (ironfarm's idealized stop)", h[0], h[1], h[2]),
            None => println!("  no held cart (an obstacle, if any, is a --set block)"),
        }
        let mut riders: Vec<Rider> = (0..risers).map(|_| Rider::default()).collect();
        let mut held_pushed_max = 0.0f64;
        let mut last_lines: Vec<String> = vec![String::new(); risers as usize];
        for t in 1..=ticks {
            srv.step(1)?;
            // how far this tick moved the held cart, then put it back
            if let Some(h) = h {
                let hp_now = nums(&srv.cmd_long("data get entity @e[tag=cth,limit=1] Pos")?);
                anyhow::ensure!(hp_now.len() == 3, "the held cart left the world at t{t}");
                held_pushed_max = held_pushed_max.max((0..3).map(|i| (hp_now[i] - h[i]).abs()).fold(0.0, f64::max));
                srv.cmd_ok(&format!("tp @e[tag=cth,limit=1] {:.4} {:.4} {:.4}", h[0], h[1], h[2]))?;
                srv.cmd_ok("data merge entity @e[tag=cth,limit=1] {Motion:[0.0d,0.0d,0.0d]}")?;
            }
            for (i, rd) in riders.iter_mut().enumerate() {
                let cp = nums(&srv.cmd_long(&format!("data get entity @e[tag=ctr{i},limit=1] Pos"))?);
                let vp = nums(&srv.cmd_long(&format!("data get entity @e[tag=ctr{i}v,limit=1] Pos"))?);
                let air = num(&srv.cmd_long(&format!("data get entity @e[tag=ctr{i}v,limit=1] Air"))?);
                let hp = num(&srv.cmd_long(&format!("data get entity @e[tag=ctr{i}v,limit=1] Health"))?);
                let seated = srv.cmd_long(&format!("data get entity @e[tag=ctr{i},limit=1] Passengers"))?.contains(&format!("\"ctr{i}v\""));
                if hp.is_some() {
                    rd.alive += 1;
                    if seated {
                        rd.seated_alive += 1;
                    }
                }
                if let (Some(a), Some(b)) = (rd.last_hp, hp) {
                    if b < a {
                        rd.hp_drop = true;
                    }
                }
                if hp.is_some() {
                    rd.last_hp = hp;
                }
                rd.airs.push(air);
                let eye = (vp.len() == 3).then(|| [vp[0].floor() as i32, (vp[1] + 1.62).floor() as i32, vp[2].floor() as i32]);
                let eye_block = match eye {
                    Some(e) => match fluid_at(&mut srv, e)? {
                        Some(f) => f,
                        None => format!("a non-fluid block (the design lays {})", own.get(&e).map(String::as_str).unwrap_or("nothing there")),
                    },
                    None => "-".into(),
                };
                rd.eyes.push(eye.map(|e| (e, eye_block.clone())));
                let line = format!(
                    "rider {i}: cart {} | eye {} {eye_block} | Air {} HP {}{}",
                    if cp.len() == 3 { format!("({:.4},{:.4},{:.4})", cp[0], cp[1], cp[2]) } else { "gone".into() },
                    eye.map(|e| format!("({},{},{})", e[0], e[1], e[2])).unwrap_or_else(|| "-".into()),
                    air.map(|a| format!("{a:.0}")).unwrap_or_else(|| "-".into()),
                    hp.map(|h| format!("{h:.1}")).unwrap_or_else(|| "-".into()),
                    if seated { "" } else { " | NOT SEATED" }
                );
                let changed = line.split(" | Air").next() != last_lines[i].split(" | Air").next();
                if t <= 3 || t % 50 == 0 || changed {
                    println!("    t{t:>4} {line}");
                }
                last_lines[i] = line;
            }
        }
        println!("  the held cart: the most any tick moved it before it was put back: {held_pushed_max:.4}");
        let mut seated_ok = true;
        for (i, rd) in riders.iter().enumerate() {
            let min_air = rd.airs.iter().flatten().fold(f64::INFINITY, |m, &a| m.min(a));
            let late_min = rd.airs.iter().skip(40).flatten().fold(f64::INFINITY, |m, &a| m.min(a));
            let rest: Vec<&([i32; 3], String)> = rd.eyes.iter().rev().take(100).flatten().collect();
            let rest_eye = rest.first().map(|(e, b)| format!("({},{},{}) {b}", e[0], e[1], e[2])).unwrap_or_else(|| "-".into());
            let rest_constant = rest.windows(2).all(|w| w[0] == w[1]);
            println!(
                "  => rider {i}: seated on {} of the {} ticks it lived; least Air {min_air:.0} (after its first 40 ticks {late_min:.0}); a Health drop: {}; its stranded eye (the last 100 ticks{}): {rest_eye}",
                rd.seated_alive,
                rd.alive,
                rd.hp_drop,
                if rest_constant { ", one cell" } else { ", NOT one cell" }
            );
            seated_ok &= rd.alive > 0 && rd.seated_alive == rd.alive;
        }
        pin(2, seated_ok, "(2) every rider is read back seated on every tick it lives".into())?;
        if !expect.is_empty() {
            let mut verdicts = Vec::new();
            let mut ok = true;
            for (i, rd) in riders.iter().enumerate() {
                let min_air = rd.airs.iter().flatten().fold(f64::INFINITY, |m, &a| m.min(a));
                let late_min = rd.airs.iter().skip(40).flatten().fold(f64::INFINITY, |m, &a| m.min(a));
                let rest: Vec<&([i32; 3], String)> = rd.eyes.iter().rev().take(100).flatten().collect();
                let plain_water = rest.first().is_some_and(|(_, b)| b.starts_with("minecraft:water"));
                let v = if expect[i] == "drowns" { plain_water && min_air <= -19.0 && rd.hp_drop } else { late_min >= 300.0 && !rd.hp_drop };
                verdicts.push(format!("rider {i} {}: {v}", expect[i]));
                ok &= v;
            }
            pin(3, ok, format!("(3) each rider's air as expected (drowns: its stranded eye in plain water, Air to -19 or below, a Health drop; breathes: Air 300 after its first 40 ticks, no damage): {verdicts:?}"))?;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    if let Some(k) = plant()? {
        bail!("control planted nothing: MCTAS_COLUMN_TOP_PLANT={k} never failed its assert (the run did not reach it)");
    }
    println!("  every assert this run reached held");
    Ok(())
}
