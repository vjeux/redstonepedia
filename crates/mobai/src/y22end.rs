//! (tickexact 2026-11-14, ironspec 03:10 / 03:14, b8949c96 03:12: the y-22 line's end, the P1 booster's closing
//! evidence) `mctas mobai y22-end <design>`: on the official 26.3 server, the design pasted at its own coordinates (9fc3155f's
//! scratch V rev 1 + the booster, a/3427690490750214 md5 f39e08f258ef: the line along x 9 at y 22, plain north_south rails
//! (9,22,-3..4), the booster (9,22,-4) a powered rail on its redstone block (9,21,-4), the column (9,22,-5) open air, the
//! wall (9,22,-6)), P1 (9,22,5) laid as b8949c96's braked powered rail ascending south on its glass (9,21,5); one cart at a
//! time, RIDDEN then EMPTY, resting in P1 at z 5.1222 / 5.15 / 5.19 / 5.3 / 5.45 / 5.6 / 5.7322 (its 148-run rest range,
//! the north edge 5.12 .. 5.19), released at t0 by P1 POWERED (its support swapped for a redstone block, the bench's
//! token): it rolls north down the slope, along the line, through the booster and off the end. Every tick: position and
//! motion. Printed per row: the horizontal speed at the first tick the cart is past the rail end (its centre in
//! (9,22,-5): z < -4.0), its fall (below y 21) and its landing (first tick on ground: the cell, the rider's health), or
//! STRANDED; against the game's all-phase band (every phase lands from 0.27 empty / 0.25 ridden: rail-end-threshold).
//!
//! The modes (each with its known answers asserted, `MCTAS_Y22_END_PLANT=k` inverting assert k):
//! - the releases above (asserts 1..3; `--booster2` / `--torch-boosters` / `--pane` add booster 2 as a block, two torches,
//!   or two torches on glass panes: the same answers to the 4th decimal);
//! - `--follow-on`: an empty cart parked on the line (z -0.31 / -2.09) and a ridden napper released after it (asserts 4..5;
//!   R driven when the napper is thrown onto R; `--token-latch` re-brakes P1 the tick after the napper enters (9,22,4));
//! - `--roll-in`: the rest point in P1 per arrival speed and phase (assert 6);
//! - `--unheld`: P1 powered, carts rolled straight through (assert 7, with both boosters);
//! - `--front-at Z [--front-empty]`: a ridden (or empty) cart planted at rest on the landing slope, a napper onto it, then
//!   the next napper; the arrival detector (9,-9,-5) per tick (assert 8 at the fronts run);
//! - `--front-string [--long-rest]`: a hooked string at (9,-9,-4) over the front slope, carts braked at rest under it, the
//!   string every tick for 100 gt (assert 9), or every rest position empty and ridden for 1,200 gt (assert 10).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

fn nums(s: &str) -> Vec<f64> {
    s.split('[').nth(1).unwrap_or("").split(',').filter_map(|x| x.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect()
}

struct Row {
    ridden: bool,
    v: f64,
    z0: f64,
    /// the horizontal speed after the first tick with the cart past the rail end, and that tick
    past: Option<(u32, f64)>,
    /// fell below y 21 at this tick
    fell: Option<u32>,
    last: Option<[f64; 3]>,
    /// at rest after the fall: its position and the rider's health
    land: Option<([f64; 3], Option<f64>)>,
}

/// MCTAS_Y22_END_PLANT=k inverts assert k: that run must fail at assert k, by name
fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

/// MCTAS_Y22_END_PLANT read strictly: unset, or an assert number 1..=12; anything else fails by name
fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_Y22_END_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_Y22_END_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=12).contains(&k), "MCTAS_Y22_END_PLANT={k}: y22-end has asserts 1..12");
            Ok(Some(k))
        }
    }
}

fn cell_of(p: [f64; 3]) -> [i32; 3] {
    [p[0].floor() as i32, (p[1] - 1e-6).floor() as i32, p[2].floor() as i32]
}

pub fn y22_end(version: &str, design: &std::path::Path, follow_on: bool, booster2: bool, torch: bool, roll_in: bool, unheld: bool, front_at: Option<f64>, token_latch: bool, pane: bool, front_empty: bool, front_string: bool, long_rest: bool, front_hold: bool) -> Result<()> {
    let torch = torch || pane;
    let booster2 = booster2 || torch;
    plant()?;
    let d = mctas::design::Design::load(design)?;
    let blocks = d.block_list();
    println!("== y22-end on the official {version} server: {} ({} cells) pasted at its own coordinates (no entities); the line's end at x 9, y 22", design.display(), blocks.len());
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    let [x0, z0, x1, z1] = crate::portal::chunk_box(&blocks, 8);
    srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
    srv.step(40)?;
    crate::portal::clear_box(&mut srv, "minecraft:overworld", &blocks, 2)?;
    crate::portal::paste(&mut srv, "minecraft:overworld", &blocks)?;
    srv.step(100)?;
    for (p, s) in [
        ([9, 22, -4], "minecraft:powered_rail[shape=north_south,powered=true]"),
        ([9, 21, -4], "minecraft:redstone_block"),
        ([9, 22, -5], "minecraft:air"),
        ([9, 22, -6], "minecraft:glass"),
        ([9, 22, -3], "minecraft:rail[shape=north_south]"),
    ] {
        let ok = srv.test_block(p, s)?;
        println!("  ({},{},{}) {s}: {ok}", p[0], p[1], p[2]);
        anyhow::ensure!(ok, "the line's end is not as the file lays it at {p:?} ({s})");
    }
    // (ironspec 03:15 / b8949c96 03:17) P1 (9,22,5) a BRAKED powered rail ascending south (the scratch predates it: a plain
    // ascending rail there); each cart rests in it, released by P1 POWERED (its support (9,21,5) swapped for a redstone
    // block, as the bench emulates day-1's token): it rolls north by gravity down the slope, along the flat line, through
    // the booster and off the end
    srv.cmd_ok("setblock 9 22 5 minecraft:powered_rail[shape=ascending_south,powered=false]")?;
    srv.step(2)?;
    let p1 = srv.test_block([9, 22, 5], "minecraft:powered_rail[shape=ascending_south,powered=false]")?;
    println!("  P1 (9,22,5) a braked powered rail ascending south: {p1}");
    anyhow::ensure!(p1, "P1 is not a braked ascending_south powered rail");
    // (b8949c96 04:15, ironspec: BOOSTER 2 priced) `--booster2`: a second always-powered booster at (9,22,2): a
    // powered rail north_south on a redstone block at (9,21,2) in place of its glass support; booster 1 (9,22,-4) as laid
    if booster2 {
        // (ironspec 04:32 (2) / 04:37 (3)) `--torch-boosters`: BOTH boosters in the TORCH form: booster 2's rail (9,22,2)
        // on its glass support (9,21,2), powered by a lit redstone torch at (8,22,2) on glass (8,21,2); booster 1's rail
        // (9,22,-4) on glass (9,21,-4) (its redstone block replaced), powered by a lit torch at (8,22,-4) on glass (8,21,-4)
        if torch {
            // (ironspec 05:03 (4)(ii)) `--pane`: each torch stands on a GLASS PANE at (8,21,z) (joined east to the booster's
            // glass support (9,21,z)) instead of a glass block; after placing, the pane is set again (its shape changes and
            // back) so the torch's own survival check (updateShape from below) runs before the rows
            let support = if pane { "minecraft:glass_pane[east=true,north=false,south=false,waterlogged=false,west=false]" } else { "minecraft:glass" };
            srv.cmd("setblock 9 21 2 minecraft:glass")?;
            srv.cmd(&format!("setblock 8 21 2 {support}"))?;
            srv.cmd_ok("setblock 8 22 2 minecraft:redstone_torch[lit=true]")?;
            srv.cmd_ok("setblock 9 22 2 minecraft:powered_rail[shape=north_south,powered=true]")?;
            srv.cmd(&format!("setblock 8 21 -4 {support}"))?;
            srv.cmd_ok("setblock 8 22 -4 minecraft:redstone_torch[lit=true]")?;
            srv.cmd_ok("setblock 9 21 -4 minecraft:glass")?;
            srv.step(4)?;
            if pane {
                for z in [2, -4] {
                    srv.cmd_ok(&format!("setblock 8 21 {z} minecraft:glass_pane[east=false,north=false,south=false,waterlogged=false,west=false]"))?;
                    srv.step(2)?;
                    srv.cmd_ok(&format!("setblock 8 21 {z} {support}"))?;
                    srv.step(2)?;
                }
                srv.step(20)?;
            }
            let mut ok = true;
            for (p, s) in [
                ([9, 22, 2], "minecraft:powered_rail[shape=north_south,powered=true]"),
                ([9, 21, 2], "minecraft:glass"),
                ([8, 22, 2], "minecraft:redstone_torch[lit=true]"),
                ([8, 21, 2], if pane { "minecraft:glass_pane" } else { "minecraft:glass" }),
                ([9, 22, -4], "minecraft:powered_rail[shape=north_south,powered=true]"),
                ([9, 21, -4], "minecraft:glass"),
                ([8, 22, -4], "minecraft:redstone_torch[lit=true]"),
                ([8, 21, -4], if pane { "minecraft:glass_pane" } else { "minecraft:glass" }),
            ] {
                let held = srv.test_block(p, s)?;
                println!("  TORCH FORM ({},{},{}) {s}: {held}", p[0], p[1], p[2]);
                ok &= held;
            }
            anyhow::ensure!(ok, "the boosters' torch form is not laid as priced (a rail unpowered or a torch unlit)");
        } else {
            srv.cmd_ok("setblock 9 21 2 minecraft:redstone_block")?;
            srv.cmd_ok("setblock 9 22 2 minecraft:powered_rail[shape=north_south,powered=true]")?;
            srv.step(2)?;
            let b2 = srv.test_block([9, 22, 2], "minecraft:powered_rail[shape=north_south,powered=true]")? && srv.test_block([9, 21, 2], "minecraft:redstone_block")?;
            println!("  BOOSTER 2 (9,22,2) a powered rail north_south, powered, on a redstone block (9,21,2): {b2}");
            anyhow::ensure!(b2, "booster 2 is not laid as priced");
        }
    }
    if follow_on {
        return follow_on_rows(&mut srv, &blocks, booster2, token_latch);
    }
    if roll_in {
        return roll_in_rows(&mut srv);
    }
    if unheld {
        return unheld_rows(&mut srv, booster2);
    }
    if let Some(zf) = front_at {
        return front_rows(&mut srv, zf, front_empty);
    }
    if front_hold {
        return front_hold_rows(&mut srv);
    }
    if front_string {
        return front_string_rows(&mut srv, &blocks, long_rest);
    }
    let mut rows: Vec<Row> = Vec::new();
    for ridden in [true, false] {
        // (b8949c96 03:17) P1's rest range over its 148-run hold sweep: z 5.1222 .. 5.7322 (the north edge 5.12 .. 5.19)
        for z0 in [5.1222, 5.15, 5.19, 5.3, 5.45, 5.6, 5.7322] {
            rows.push(Row { ridden, v: 0.0, z0, past: None, fell: None, last: None, land: None });
        }
    }
    for (k, r) in rows.iter_mut().enumerate() {
        let tag = format!("ye{k}");
        let rider = if r.ridden { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
        srv.cmd("setblock 9 21 5 minecraft:glass")?;
        srv.step(2)?;
        srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {:.4} {:.4} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,0.0d]{rider}}}", 22.0 + (r.z0 - 5.0) + 0.0625, r.z0))?;
        srv.step(10)?;
        let rest = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
        // (b8949c96 03:17) the release: P1 powered at t0 by swapping its support (9,21,5) for a redstone block, as the
        // bench emulates day-1's token
        srv.cmd_ok("setblock 9 21 5 minecraft:redstone_block")?;
        if rest.len() == 3 {
            r.v = rest[2];
        }
        let mut landed: Option<(u32, [f64; 3], Option<f64>)> = None;
        let mut still = 0u32;
        let mut prev: Option<[f64; 3]> = None;
        for t in 1..=600u32 {
            srv.step(1)?;
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
            let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
            if p.len() != 3 || m.len() != 3 {
                break;
            }
            r.last = Some([p[0], p[1], p[2]]);
            // MCTAS_Y22_TRACE=1: every tick of every row
            if std::env::var("MCTAS_Y22_TRACE").is_ok() {
                println!("    t{t:>3}: z {:.4} y {:.4} | motion ({:+.4}, {:+.4}, {:+.4})", p[2], p[1], m[0], m[1], m[2]);
            }
            if r.past.is_none() && p[2] < -4.0 {
                r.past = Some((t, (m[0] * m[0] + m[2] * m[2]).sqrt()));
            }
            if r.fell.is_none() && p[1] < 21.0 {
                r.fell = Some(t);
            }
            // at rest: the position unchanged (to 1e-4) for 20 ticks; after the fall that is the landing (on a rail a cart
            // is never on ground)
            let moved = prev.is_none_or(|q| (0..3).any(|i| (q[i] - p[i]).abs() > 1e-4));
            prev = Some([p[0], p[1], p[2]]);
            still = if moved { 0 } else { still + 1 };
            if still >= 20 {
                if r.fell.is_some() {
                    let hp = if r.ridden { srv.cmd_long(&format!("data get entity @e[tag={tag}v,limit=1] Health"))?.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('f').parse().ok()) } else { None };
                    landed = Some((t - 20, [p[0], p[1], p[2]], hp));
                    r.land = Some(([p[0], p[1], p[2]], hp));
                }
                break;
            }
        }
        srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
        srv.cmd(&format!("kill @e[tag={tag}]"))?;
        srv.cmd("kill @e[type=minecraft:villager]")?;
        srv.cmd("kill @e[type=minecraft:item]")?;
        srv.cmd("setblock 9 21 5 minecraft:glass")?;
        srv.step(2)?;
        println!(
            "  {} resting at z {:.4} (summoned at {:.4}): {}; {}",
            if r.ridden { "RIDDEN" } else { "empty " },
            r.v,
            r.z0,
            r.past.map(|(t, s)| format!("first tick past the rail end t{t} at {s:.4} b/t")).unwrap_or_else(|| "never past the rail end".into()),
            match (r.fell, landed, r.last) {
                (Some(t), Some((tl, p, hp)), _) => format!("falls (below y 21 at t{t}), LANDS t{tl} at ({:.3}, {:.3}, {:.3}) cell ({}, {}, {}){}", p[0], p[1], p[2], p[0].floor(), p[1].floor(), p[2].floor(), hp.map(|h: f64| format!(", rider {h:.1} HP")).unwrap_or_default()),
                (Some(t), None, Some(l)) => format!("falls (below y 21 at t{t}), not at rest by t600: last ({:.3}, {:.3}, {:.3})", l[0], l[1], l[2]),
                (None, _, Some(l)) => format!("STRANDED: at rest at ({:.3}, {:.3}, {:.3})", l[0], l[1], l[2]),
                _ => "gone".into(),
            }
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    for ridden in [false, true] {
        let mine: Vec<&Row> = rows.iter().filter(|r| r.ridden == ridden).collect();
        let fell: Vec<f64> = mine.iter().filter(|r| r.fell.is_some()).filter_map(|r| r.past.map(|p| p.1)).collect();
        let strand: Vec<(f64, f64, Option<f64>)> = mine.iter().filter(|r| r.fell.is_none()).map(|r| (r.v, r.z0, r.past.map(|p| p.1))).collect();
        println!(
            "  {}: {} rows fall into the column, the least speed past the rail end among them {:?}; stranded rows (entry, z0, speed past the end): {strand:?}",
            if ridden { "RIDDEN" } else { "EMPTY" },
            fell.len(),
            fell.iter().cloned().reduce(f64::min)
        );
    }
    // (G1's pattern) the first runs' answers, per form: the booster as laid (block), booster 2 added (block or torch, the
    // same answers to the 4th decimal)
    let landed_ok = |r: &Row, want_hp: bool| r.fell.is_some() && r.land.is_some_and(|(p, hp)| cell_of(p) == [9, -10, -4] && (!want_hp || hp == Some(20.0)));
    let ridden: Vec<&Row> = rows.iter().filter(|r| r.ridden).collect();
    let empty: Vec<&Row> = rows.iter().filter(|r| !r.ridden).collect();
    pin(1, ridden.len() == 7 && ridden.iter().all(|r| landed_ok(r, true)), "(1) every RIDDEN row passes the rail end, falls and comes to rest in cell (9,-10,-4), its rider at 20.0 HP".into())?;
    let speed = |z: f64| ridden.iter().find(|r| (r.z0 - z).abs() < 1e-9).and_then(|r| r.past.map(|p| p.1));
    let (lo, hi) = if booster2 { (0.5995, 0.7676) } else { (0.4801, 0.5911) };
    pin(
        2,
        speed(5.1222).is_some_and(|s| (s - lo).abs() < 5e-4) && speed(5.7322).is_some_and(|s| (s - hi).abs() < 5e-4) && ridden.iter().all(|r| r.past.is_some_and(|p| p.1 >= 0.25)),
        format!("(2) RIDDEN past the rail end at {lo} from z 5.1222 and {hi} from 5.7322 (+-0.0005), every row >= 0.25 (the ridden all-phase bound): {:?} / {:?}", speed(5.1222), speed(5.7322)),
    )?;
    if booster2 {
        let least = empty.iter().filter_map(|r| r.past.map(|p| p.1)).fold(f64::INFINITY, f64::min);
        pin(
            3,
            empty.len() == 7 && empty.iter().all(|r| landed_ok(r, false) && r.past.is_some_and(|p| p.1 >= 0.27)) && (least - 0.3877).abs() < 5e-4,
            format!("(3) with booster 2 every EMPTY row passes the rail end at >= 0.27 (the least 0.3877 +- 0.0005) and comes to rest in cell (9,-10,-4): the least {least:.4}"),
        )?;
    } else {
        let on_line = |r: &Row| r.past.is_none() && r.fell.is_none() && r.last.is_some_and(|p| (p[1] - 22.0625).abs() < 1e-3 && p[2] > -4.0 && p[2] < 5.0);
        pin(
            3,
            empty.len() == 7 && empty.iter().all(|r| if r.z0 <= 5.45 { on_line(r) } else { landed_ok(r, false) && r.past.is_some_and(|p| p.1 >= 0.27) }),
            "(3) EMPTY from z 5.1222 .. 5.45 stops on the plain line before the booster (never past the rail end); from 5.6 / 5.7322 it passes at >= 0.27 and comes to rest in cell (9,-10,-4)".into(),
        )?;
    }
    println!("  every known answer held (asserts 1..3)");
    Ok(())
}

/// (ironspec 03:34 (2) / (3), G1 03:34b) `--follow-on`: the empty-cart fault's consequence: an EMPTY cart parked on the
/// line where the release rows left it (z -0.31, or -2.09), then a RIDDEN napper released from P1 (its north edge z
/// 5.1222, or its deepest rest 5.7322): does the ridden cart push the empty one through the booster (both down the
/// column), or do they jam? Printed: both carts' paths (first tick past the rail end, the fall), where each comes to
/// rest, the rider's health, and whether the second cart came down onto the first.
/// One follow-on row's outcome: (park, release z, the empty cart fell, the napper fell, the napper's rest, the napper on
/// top of the empty cart, the napper's rider health, the R-driven release if the napper rested on R: R's first powered
/// tick, the cart's rest, the rider's health, still seated)
type FoRow = (f64, f64, bool, bool, Option<[f64; 3]>, bool, Option<f64>, Option<(Option<u32>, Option<[f64; 3]>, Option<f64>, bool)>);

fn follow_on_rows(srv: &mut VanillaServer, blocks: &[(mctas::sim::Pos, String)], booster2: bool, token_latch: bool) -> Result<()> {
    let mut outcomes: Vec<FoRow> = Vec::new();
    // (P1 laid by the caller)
    anyhow::ensure!(srv.test_block([9, 22, 5], "minecraft:powered_rail[shape=ascending_south,powered=false]")?, "P1 is not a braked ascending_south powered rail");
    println!("  --follow-on: an empty cart parked on the line, then a ridden napper released from P1 behind it");
    let mut k = 0;
    for park in [-0.31, -2.09] {
        for rel in [5.1222, 5.7322] {
            k += 1;
            // MCTAS_Y22_ROW=k: only row k (1: park -0.31 / z 5.1222, 2: -0.31 / 5.7322, 3: -2.09 / 5.1222, 4: -2.09 / 5.7322)
            if std::env::var("MCTAS_Y22_ROW").ok().and_then(|s| s.trim().parse::<u32>().ok()).is_some_and(|only| only != k) {
                continue;
            }
            let (e, r) = (format!("fe{k}"), format!("fr{k}"));
            srv.cmd("setblock 9 21 5 minecraft:glass")?;
            srv.step(2)?;
            srv.cmd_ok(&format!("summon minecraft:minecart 9.5 22.0625 {park:.4} {{Tags:[\"{e}\"],Motion:[0.0d,0.0d,0.0d]}}"))?;
            srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {:.4} {rel:.4} {{Tags:[\"{r}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{r}v\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;-3,-20,3]}}}}}}}}}}]}}", 22.0 + (rel - 5.0) + 0.0625))?;
            srv.step(10)?;
            srv.cmd_ok("setblock 9 21 5 minecraft:redstone_block")?;
            // (b8949c96 05:03) `--token-latch`: the token is a latch cleared the tick the napper passes P1's exit cell
            // (9,22,4) (its centre first in that cell), and P1 re-braked the next tick (its support back to glass); without
            // it P1 stays powered for the whole row
            let mut entered_exit: Option<u32> = None;
            let mut rebraked: Option<u32> = None;
            let mut past: [Option<(u32, f64)>; 2] = [None, None];
            let mut fell: [Option<u32>; 2] = [None, None];
            let mut last: [Option<[f64; 3]>; 2] = [None, None];
            let mut still = 0u32;
            let mut prev: Option<[[f64; 3]; 2]> = None;
            let mut t_end = 0;
            for t in 1..=800u32 {
                srv.step(1)?;
                if token_latch && rebraked.is_none() && entered_exit.is_some_and(|te| t == te + 1) {
                    srv.cmd_ok("setblock 9 21 5 minecraft:glass")?;
                    rebraked = Some(t);
                    println!("    TOKEN LATCH: the napper entered (9,22,4) at t{}; P1 re-braked (its support back to glass) before t{t}", entered_exit.unwrap_or(0));
                }
                let mut now = [[f64::NAN; 3]; 2];
                for (i, tag) in [&e, &r].iter().enumerate() {
                    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
                    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
                    if p.len() != 3 || m.len() != 3 {
                        continue;
                    }
                    now[i] = [p[0], p[1], p[2]];
                    last[i] = Some(now[i]);
                    if i == 1 && entered_exit.is_none() && now[i][2] < 5.0 && now[i][2] >= 4.0 {
                        entered_exit = Some(t);
                    }
                    // (cc767b78 04:50 via farmopt) MCTAS_Y22_TRACE=1: every tick, both carts as the server's doubles (and the
                    // napper's rider), from P1's release until both rest
                    if std::env::var("MCTAS_Y22_TRACE").is_ok() {
                        let rider = if i == 1 { format!(" rider {}", srv.cmd_long(&format!("execute as @e[tag={r},limit=1] on passengers run data get entity @s Pos"))?.split("data: ").nth(1).unwrap_or("none").trim()) } else { String::new() };
                        println!("    t{t:>3} {}: pos [{:?}, {:?}, {:?}] motion [{:?}, {:?}, {:?}]{rider}", if i == 0 { "EMPTY " } else { "NAPPER" }, p[0], p[1], p[2], m[0], m[1], m[2]);
                    }
                    if past[i].is_none() && p[2] < -4.0 {
                        past[i] = Some((t, (m[0] * m[0] + m[2] * m[2]).sqrt()));
                    }
                    if fell[i].is_none() && p[1] < 21.0 {
                        fell[i] = Some(t);
                    }
                }
                let moved = prev.is_none_or(|q| (0..2).any(|i| (0..3).any(|j| (q[i][j] - now[i][j]).abs() > 1e-4)));
                prev = Some(now);
                still = if moved { 0 } else { still + 1 };
                t_end = t;
                if still >= 20 {
                    break;
                }
            }
            let hp: Option<f64> = srv.cmd_long(&format!("data get entity @e[tag={r}v,limit=1] Health"))?.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('f').parse().ok());
            let fmt = |i: usize| -> String {
                format!(
                    "{}; {}; rests at {}",
                    past[i].map(|(t, s)| format!("past the rail end t{t} at {s:.4} b/t")).unwrap_or_else(|| "never past the rail end".into()),
                    fell[i].map(|t| format!("fell (below y 21) t{t}")).unwrap_or_else(|| "did not fall".into()),
                    last[i].map(|p| format!("({:.3}, {:.3}, {:.3})", p[0], p[1], p[2])).unwrap_or_else(|| "gone".into())
                )
            };
            let on_top = match (last[0], last[1]) {
                (Some(a), Some(b)) => (a[2] - b[2]).abs() < 0.98 && b[1] > a[1] + 0.3,
                _ => false,
            };
            println!("  park z {park:.2} + napper from z {rel:.4} (all at rest by t{t_end}): EMPTY {}; RIDDEN {}; rider {}; the ridden cart came down ONTO the empty one: {on_top}", fmt(0), fmt(1), hp.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| "gone (dead or dismounted)".into()));
            let (rest_r, hp_r) = (last[1], hp);
            let mut r_driven: Option<(Option<u32>, Option<[f64; 3]>, Option<f64>, bool)> = None;
            // (G1 03:47) where each cart ends and what it touches: its box (centre +- 0.49 in x / z, y .. y + 0.7), every
            // cell it overlaps and the cell under its centre, with the design's state there and the live powered / open
            for (name, i) in [("EMPTY", 0usize), ("RIDDEN", 1usize)] {
                let Some(p) = last[i] else { continue };
                let (x0, x1, z0, z1, y0, y1) = (p[0] - 0.49, p[0] + 0.49, p[2] - 0.49, p[2] + 0.49, p[1], p[1] + 0.7);
                let mut cells: Vec<[i32; 3]> = Vec::new();
                for x in x0.floor() as i32..=(x1 - 1e-9).floor() as i32 {
                    for y in y0.floor() as i32..=(y1 - 1e-9).floor() as i32 {
                        for z in z0.floor() as i32..=(z1 - 1e-9).floor() as i32 {
                            cells.push([x, y, z]);
                        }
                    }
                }
                let below = [p[0].floor() as i32, (p[1] - 1e-6).floor() as i32 - if p[1].fract().abs() < 1e-6 { 0 } else { 0 }, p[2].floor() as i32];
                let state_of = |c: [i32; 3]| blocks.iter().find(|(q, _)| *q == c).map(|(_, s)| s.clone()).unwrap_or_else(|| "air (not in the design)".into());
                let mut line = format!("    {name} cart's box x {x0:.3}..{x1:.3} y {y0:.3}..{y1:.3} z {z0:.3}..{z1:.3}; overlaps");
                for c in cells.iter().chain(std::iter::once(&below)) {
                    let live = format!(
                        "{}{}",
                        if srv.test_block(*c, &format!("{}[powered=true]", state_of(*c).split('[').next().unwrap_or(""))).unwrap_or(false) { " (live: powered)" } else { "" },
                        if srv.test_block(*c, &format!("{}[open=true]", state_of(*c).split('[').next().unwrap_or(""))).unwrap_or(false) { " (live: open)" } else { "" }
                    );
                    line.push_str(&format!(" | ({},{},{}){} {}{live}", c[0], c[1], c[2], if *c == below { " [under the centre]" } else { "" }, state_of(*c)));
                }
                println!("{line}");
            }
            // (G1 03:58a (3)) the unslept rider's HOME at the end: the napper carries HOME = V's bed (-3,-20,3) (NBT, as
            // the farm's nappers do); the night is run on (REST) for 1,300 gt and its HOME memory read (the stamp itself
            // is not saved in NBT, so only HOME is readable: erased once a stamp has aged 1,200 gt)
            let home_before = srv.cmd_long(&format!("data get entity @e[tag={r}v,limit=1] Brain.memories.\"minecraft:home\""))?;
            srv.cmd_ok("time set 13000")?;
            srv.cmd_ok("gamerule advance_time true")?;
            srv.step(1300)?;
            srv.cmd_ok("gamerule advance_time false")?;
            let home_after = srv.cmd_long(&format!("data get entity @e[tag={r}v,limit=1] Brain.memories.\"minecraft:home\""))?;
            let riding = srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Passengers"))?.contains("villager");
            let has = |s: &str| s.contains("pos") && !s.contains("found no") && !s.contains("No entity");
            println!(
                "    the napper's HOME before the night {}; after 1,300 gt of night (REST) {}; still seated in its cart: {riding}",
                if has(&home_before) { "present" } else { "ABSENT" },
                if has(&home_after) { "present".to_string() } else { format!("ERASED ({})", home_after.trim().chars().take(80).collect::<String>()) }
            );
            srv.cmd_ok("time set 6000")?;
            let before = last;
            srv.step(2000)?;
            for (name, tag, i) in [("EMPTY", &e, 0usize), ("RIDDEN", &r, 1usize)] {
                let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
                let now = (p.len() == 3).then(|| [p[0], p[1], p[2]]);
                println!(
                    "    2000 gt later the {name} cart {}",
                    match (before[i], now) {
                        (Some(b), Some(n)) if (0..3).all(|j| (b[j] - n[j]).abs() < 1e-4) => "has not moved (nothing in the scene moved it)".to_string(),
                        (Some(_), Some(n)) => format!("MOVED to ({:.3}, {:.3}, {:.3})", n[0], n[1], n[2]),
                        (_, None) => "is gone".to_string(),
                        (None, Some(n)) => format!("is at ({:.3}, {:.3}, {:.3})", n[0], n[1], n[2]),
                    }
                );
            }
            // (ironfarm 04:12 via farmopt) R DRIVEN, in the row whose napper rests on lane 7's R (5,24,7): R's chain (7,23,9)
            // day-1's line end -> (6,23,9) repeater east -> (5,23,9) wire -> (5,23,8) repeater south -> (5,23,7) slime ->
            // R; one pulse as its lip bench drives it (10 gt): a redstone block at (7,23,9) for 10 gt, the design's state
            // restored after; every tick R's and the repeaters' powered states and the cart, until it rests 20 gt (<= 400)
            let on_r = |p: Option<[f64; 3]>| p.is_some_and(|p| p[0].floor() as i32 == 5 && (p[1] - 1e-6).floor() as i32 == 24 && p[2].floor() as i32 == 7);
            let p_now = nums(&srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Pos"))?);
            if p_now.len() == 3 && on_r(Some([p_now[0], p_now[1], p_now[2]])) {
                let input = [7, 23, 9];
                let restore = blocks.iter().find(|(q, _)| *q == input).map(|(_, s)| s.clone()).unwrap_or_else(|| "minecraft:air".into());
                println!("    R DRIVEN: the ridden cart rests on R (5,24,7); a redstone block at (7,23,9) (the design's {restore}) for 10 gt from t0, every tick:");
                srv.cmd_ok("setblock 7 23 9 minecraft:redstone_block")?;
                let (mut moved, mut still, mut prev): (bool, u32, Option<[f64; 3]>) = (false, 0, None);
                let mut trail: Vec<[i32; 3]> = Vec::new();
                let mut first_r: Option<u32> = None;
                let mut last_state = String::new();
                for t in 1..=400u32 {
                    srv.step(1)?;
                    if t == 10 {
                        srv.cmd(&format!("setblock 7 23 9 {restore}"))?;
                    }
                    let rp = srv.test_block([5, 24, 7], "minecraft:powered_rail[powered=true]")?;
                    let r1 = srv.test_block([6, 23, 9], "minecraft:repeater[powered=true]")?;
                    let r2 = srv.test_block([5, 23, 8], "minecraft:repeater[powered=true]")?;
                    if rp && first_r.is_none() {
                        first_r = Some(t);
                    }
                    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Pos"))?);
                    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Motion"))?);
                    let now = (p.len() == 3).then(|| [p[0], p[1], p[2]]);
                    if let Some(n) = now {
                        let c = [n[0].floor() as i32, (n[1] - 1e-6).floor() as i32, n[2].floor() as i32];
                        if trail.last() != Some(&c) {
                            trail.push(c);
                        }
                    }
                    let state = format!(
                        "repeater (6,23,9) {} | repeater (5,23,8) {} | R {} | cart {}",
                        if r1 { "ON" } else { "off" },
                        if r2 { "ON" } else { "off" },
                        if rp { "POWERED" } else { "braked" },
                        now.map(|n| format!("({:.3}, {:.3}, {:.3}) v ({:.3}, {:.3}, {:.3})", n[0], n[1], n[2], m.first().copied().unwrap_or(0.0), m.get(1).copied().unwrap_or(0.0), m.get(2).copied().unwrap_or(0.0))).unwrap_or_else(|| "gone".into())
                    );
                    let short = format!("{}{}{}", r1, r2, rp);
                    let movedn = match (prev, now) {
                        (Some(a), Some(b)) => (0..3).any(|j| (a[j] - b[j]).abs() > 1e-4),
                        _ => false,
                    };
                    if movedn || short != last_state || t <= 12 {
                        println!("      t{t}: {state}");
                    }
                    last_state = short;
                    if movedn {
                        moved = true;
                        still = 0;
                    } else {
                        still += 1;
                    }
                    prev = now;
                    if moved && still >= 20 {
                        break;
                    }
                }
                println!(
                    "    R DRIVEN: R first powered at t{} after the block; the cart {}; its cells in order: {}",
                    first_r.map_or("-".into(), |t| t.to_string()),
                    if moved { format!("came to rest at {}", prev.map(|n| format!("({:.3}, {:.3}, {:.3})", n[0], n[1], n[2])).unwrap_or_else(|| "gone".into())) } else { "never moved in 400 gt".to_string() },
                    trail.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" -> ")
                );
                let hp: Option<f64> = srv.cmd_long(&format!("data get entity @e[tag={r}v,limit=1] Health"))?.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('f').parse().ok());
                let seated = srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Passengers")).map(|s| s.contains("villager")).unwrap_or(false);
                println!("    R DRIVEN: the rider {}", hp.map(|h| format!("{h:.1} HP, still seated: {seated}")).unwrap_or_else(|| "gone".into()));
                r_driven = Some((first_r, prev, hp, seated));
            }
            outcomes.push((park, rel, fell[0].is_some(), fell[1].is_some(), rest_r, on_top, hp_r, r_driven));
            for tag in [&e, &r] {
                srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
                srv.cmd(&format!("kill @e[tag={tag}]"))?;
            }
            srv.cmd("kill @e[type=minecraft:villager]")?;
            srv.cmd("kill @e[type=minecraft:item]")?;
            srv.step(2)?;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    // (G1's pattern) the first runs' answers. A row filtered out by MCTAS_Y22_ROW is not judged.
    let row = |park: f64, rel: f64| outcomes.iter().find(|o| (o.0 - park).abs() < 1e-9 && (o.1 - rel).abs() < 1e-9);
    let self_clears = |o: &FoRow| o.2 && o.3 && o.5 && o.6 == Some(20.0);
    let napper_cell = |o: &FoRow| o.4.map(cell_of);
    if !booster2 && token_latch {
        // (b8949c96's token latch, P1 re-braked the tick after the napper enters (9,22,4)) the one row run with it:
        // parked at -0.31, the napper from z 5.7322 creeps back and stops in P1
        let mut ok5 = true;
        let mut judged = 0;
        if let Some(o) = row(-0.31, 5.7322) {
            judged += 1;
            ok5 &= !o.3 && napper_cell(o) == Some([9, 22, 5]);
        }
        pin(5, judged == 1 && ok5, format!("(5) with the token latch, parked at -0.31, the napper from P1's deepest rest (z 5.7322) jams, creeps back and stops in P1 (9,22,5) ({judged} rows judged; run it with MCTAS_Y22_ROW=2)"))?;
        println!("  every known answer held (assert 5)");
    } else if !booster2 {
        // the north-edge releases push the parked empty through: both fall, the napper perched on it, unhurt
        let north: Vec<&FoRow> = [row(-0.31, 5.1222), row(-2.09, 5.1222)].into_iter().flatten().collect();
        pin(4, north.iter().all(|o| self_clears(o)), format!("(4) from P1's north edge (z 5.1222) the napper pushes the parked empty cart through: both fall, the napper rests on the empty cart, its rider at 20.0 HP ({} rows judged)", north.len()))?;
        // the deep releases jam: (-0.31) thrown back onto R and released into lane 7's slot 1 by R's pulse (P1 held
        // powered), or stopped in P1 (the token latch); (-2.09) the empty pushed through, the napper stopped on the line
        let mut ok5 = true;
        let mut judged = 0;
        if let Some(o) = row(-0.31, 5.7322) {
            judged += 1;
            ok5 &= !o.3 && napper_cell(o) == Some([5, 24, 7]) && o.7.is_some_and(|(first, rest, hp, seated)| first == Some(4) && rest.map(cell_of) == Some([7, 17, 1]) && hp == Some(20.0) && seated);
        }
        if let Some(o) = row(-2.09, 5.7322) {
            judged += 1;
            ok5 &= o.2 && !o.3 && o.4.is_some_and(|p| cell_of(p)[1] == 22 && p[2] > -2.0 && p[2] < 0.0);
        }
        pin(
            5,
            ok5,
            format!(
                "(5) from P1's deepest rest (z 5.7322) the napper jams: parked at -0.31 it is thrown back onto R (5,24,7), and R's pulse (powered 4 gt after the line end) releases it into lane 7's slot 1 (7,17,1), its rider seated at 20.0 HP; parked at -2.09 the empty is pushed through and the napper stops on the line ({judged} rows judged)"
            ),
        )?;
        println!("  every known answer held (asserts 4..5)");
    } else {
        // with booster 2 every parked-empty row jams: -0.31 throws the napper onto R; -2.09 stops it on the line
        let all: Vec<&FoRow> = outcomes.iter().collect();
        pin(
            4,
            all.iter().all(|o| !o.3 && if (o.0 + 0.31).abs() < 1e-9 { napper_cell(o) == Some([5, 24, 7]) } else { o.2 && o.4.is_some_and(|p| cell_of(p)[1] == 22 && p[2] > -2.0 && p[2] < 0.0) }),
            format!("(4) with booster 2 a parked empty cart jams every napper: parked at -0.31 the napper is thrown onto R (5,24,7); parked at -2.09 the empty is pushed through and the napper stops on the line ({} rows judged)", all.len()),
        )?;
        println!("  every known answer held (assert 4)");
    }
    Ok(())
}

/// (b8949c96 04:28, G1 04:24c) `--roll-in`: the game's rest point in P1 per arrival: each cart rolled in from (x0,
/// 25.0625, 9.5) heading EAST at 0.10 .. 0.46 b/t by 0.02, four entry phases x0 = 8.2 / 8.4 / 8.6 / 8.8 (where in the
/// first cell it starts), RIDDEN and EMPTY, P1 braked; until at rest 20 gt (<= 400 gt). Printed per row: the cells it
/// passes (the first row in full), where it comes to rest (the server's doubles), whether that is in P1 (9,22,5).
fn roll_in_rows(srv: &mut VanillaServer) -> Result<()> {
    anyhow::ensure!(srv.test_block([9, 22, 5], "minecraft:powered_rail[shape=ascending_south,powered=false]")?, "P1 is not a braked ascending_south powered rail");
    println!("  --roll-in: carts rolled in from (x0, 25.0625, 9.5) heading east, P1 braked; the rest point per speed and phase");
    let mut k = 0u32;
    let mut rolls: Vec<(bool, i32, f64, Option<[f64; 3]>)> = Vec::new();
    for ridden in [true, false] {
        for c in (10..=46).step_by(2) {
            let v = c as f64 / 100.0;
            let mut cells_line = String::new();
            for x0 in [8.2, 8.4, 8.6, 8.8] {
                k += 1;
                let tag = format!("ri{k}");
                let rider = if ridden { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
                srv.cmd_ok(&format!("summon minecraft:minecart {x0:.4} 25.0625 9.5 {{Tags:[\"{tag}\"],Motion:[{v:.4}d,0.0d,0.0d]{rider}}}"))?;
                let (mut still, mut prev, mut last): (u32, Option<[f64; 3]>, Option<[f64; 3]>) = (0, None, None);
                let mut trail: Vec<[i32; 3]> = Vec::new();
                let mut t_end = 0;
                for t in 1..=400u32 {
                    srv.step(1)?;
                    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
                    if p.len() != 3 {
                        break;
                    }
                    let now = [p[0], p[1], p[2]];
                    let cell = [now[0].floor() as i32, (now[1] - 1e-6).floor() as i32, now[2].floor() as i32];
                    if trail.last() != Some(&cell) {
                        trail.push(cell);
                    }
                    let moved = prev.is_none_or(|q| (0..3).any(|i| (q[i] - now[i]).abs() > 1e-6));
                    still = if moved { 0 } else { still + 1 };
                    prev = Some(now);
                    last = Some(now);
                    t_end = t;
                    if still >= 20 {
                        break;
                    }
                }
                let in_p1 = last.is_some_and(|p| p[0].floor() as i32 == 9 && (p[1] - 1e-6).floor() as i32 == 22 && p[2].floor() as i32 == 5);
                if cells_line.is_empty() {
                    cells_line = trail.iter().map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" -> ");
                }
                println!(
                    "  {} v {v:.2} x0 {x0:.1}: {} (by t{t_end}){}",
                    if ridden { "RIDDEN" } else { "empty " },
                    last.map(|p| format!("rests at x {:?} y {:?} z {:?}", p[0], p[1], p[2])).unwrap_or_else(|| "gone".into()),
                    if in_p1 { " IN P1".to_string() } else { format!(" NOT in P1 (cells {})", trail.iter().rev().take(3).map(|c| format!("({},{},{})", c[0], c[1], c[2])).collect::<Vec<_>>().join(" <- ")) }
                );
                rolls.push((ridden, c, x0, last));
                srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
                srv.cmd(&format!("kill @e[tag={tag}]"))?;
                srv.cmd("kill @e[type=minecraft:villager]")?;
                srv.cmd("kill @e[type=minecraft:item]")?;
                srv.step(2)?;
            }
            if c == 10 {
                println!("    (its first row's cells: {cells_line})");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    // (G1's pattern) the first run's answers: every ridden arrival rests in P1 within z 5.1284 .. 5.7349; every empty one
    // too (z 5.1410 .. 5.7611) except the slowest from the cell's west edge (0.10 from x0 8.2), which stops on (9,25,8)
    let in_range = |p: Option<[f64; 3]>, lo: f64, hi: f64| p.is_some_and(|p| cell_of(p) == [9, 22, 5] && p[2] >= lo - 5e-4 && p[2] <= hi + 5e-4);
    let ridden_ok = rolls.iter().filter(|r| r.0).all(|r| in_range(r.3, 5.1284, 5.7349));
    let empty_ok = rolls.iter().filter(|r| !r.0).all(|r| if r.1 == 10 && (r.2 - 8.2).abs() < 1e-9 { r.3.is_some_and(|p| cell_of(p) == [9, 25, 8]) } else { in_range(r.3, 5.1410, 5.7611) });
    pin(6, rolls.len() == 152 && ridden_ok && empty_ok, "(6) every ridden roll-in rests in P1 (9,22,5) within z 5.1284 .. 5.7349, every empty one within 5.1410 .. 5.7611 except 0.10 b/t from x0 8.2, which stops on (9,25,8)".into())?;
    println!("  every known answer held (assert 6)");
    Ok(())
}

/// One cart tracked until it rests 20 gt (<= `max` gt): the first tick past the rail end (z < -4) and its horizontal
/// speed, the least z while still at the line's height (y >= 21.9: the stop wall (9,22,-6) is met at a centre z of -4.51),
/// the tick it falls below y 21, and where it rests.
struct Track {
    past: Option<(u32, f64)>,
    min_z_high: f64,
    fell: Option<u32>,
    last: Option<[f64; 3]>,
    t_end: u32,
    /// (G1 06:22 (1)) the ticks the arrival detector (9,-9,-5) reads powered
    det: Vec<u32>,
}

fn ticks_text(v: &[u32]) -> String {
    if v.is_empty() {
        return "never".into();
    }
    let mut runs: Vec<String> = Vec::new();
    let mut start = v[0];
    let mut prev = v[0];
    for &t in &v[1..] {
        if t != prev + 1 {
            runs.push(if start == prev { format!("t{start}") } else { format!("t{start}..t{prev}") });
            start = t;
        }
        prev = t;
    }
    runs.push(if start == prev { format!("t{start}") } else { format!("t{start}..t{prev}") });
    runs.join(", ")
}

fn track(srv: &mut VanillaServer, tag: &str, max: u32) -> Result<Track> {
    let mut tr = Track { past: None, min_z_high: f64::INFINITY, fell: None, last: None, t_end: 0, det: Vec::new() };
    let (mut still, mut prev): (u32, Option<[f64; 3]>) = (0, None);
    for t in 1..=max {
        srv.step(1)?;
        let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
        let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
        if p.len() != 3 || m.len() != 3 {
            break;
        }
        let now = [p[0], p[1], p[2]];
        if tr.past.is_none() && now[2] < -4.0 {
            tr.past = Some((t, (m[0] * m[0] + m[2] * m[2]).sqrt()));
        }
        if now[1] >= 21.9 {
            tr.min_z_high = tr.min_z_high.min(now[2]);
        }
        if srv.test_block([9, -9, -5], "minecraft:detector_rail[powered=true]")? {
            tr.det.push(t);
        }
        if tr.fell.is_none() && now[1] < 21.0 {
            tr.fell = Some(t);
        }
        let moved = prev.is_none_or(|q| (0..3).any(|i| (q[i] - now[i]).abs() > 1e-4));
        still = if moved { 0 } else { still + 1 };
        prev = Some(now);
        tr.last = Some(now);
        tr.t_end = t;
        if still >= 20 {
            break;
        }
    }
    Ok(tr)
}

fn health(srv: &mut VanillaServer, tag: &str) -> Result<Option<f64>> {
    Ok(srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Health"))?.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('f').parse().ok()))
}

fn clear_carts(srv: &mut VanillaServer, tags: &[&str]) -> Result<()> {
    for tag in tags {
        srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
        srv.cmd(&format!("kill @e[tag={tag}]"))?;
    }
    srv.cmd("kill @e[type=minecraft:villager]")?;
    srv.cmd("kill @e[type=minecraft:item]")?;
    srv.step(2)?;
    Ok(())
}

/// (b8949c96 04:44 (a)) `--unheld`: a napper that finds the token already set passes P1 UNHELD: P1 powered (its support a
/// redstone block), each cart rolled in from (x0, 25.0625, 9.5) heading east at 0.10 / 0.20 / 0.30 / 0.40 / 0.46 b/t, x0 =
/// 8.2 / 8.5 / 8.8, RIDDEN and EMPTY, straight through P1 and down the line; with the boosters as the caller laid them.
/// Printed per row: the speed past the rail end, whether its box met the stop wall (9,22,-6) (the least z at the line's
/// height <= -4.51), the fall, the landing and the rider's health.
fn unheld_rows(srv: &mut VanillaServer, booster2: bool) -> Result<()> {
    srv.cmd_ok("setblock 9 21 5 minecraft:redstone_block")?;
    srv.step(2)?;
    anyhow::ensure!(srv.test_block([9, 22, 5], "minecraft:powered_rail[shape=ascending_south,powered=true]")?, "P1 is not powered (unheld)");
    println!("  --unheld: P1 POWERED; carts rolled in from (x0, 25.0625, 9.5) heading east, straight through P1, the line and the boosters");
    let mut k = 0u32;
    let mut top: f64 = 0.0;
    #[allow(clippy::type_complexity)]
    let mut unheld_rows_seen: Vec<(bool, i32, f64, Option<f64>, f64, Option<[f64; 3]>, Option<f64>)> = Vec::new();
    for ridden in [true, false] {
        for c in [10, 20, 30, 40, 46] {
            let v = c as f64 / 100.0;
            for x0 in [8.2, 8.5, 8.8] {
                k += 1;
                let tag = format!("un{k}");
                let rider = if ridden { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
                srv.cmd_ok(&format!("summon minecraft:minecart {x0:.4} 25.0625 9.5 {{Tags:[\"{tag}\"],Motion:[{v:.4}d,0.0d,0.0d]{rider}}}"))?;
                let tr = track(srv, &tag, 600)?;
                let hp = if ridden { health(srv, &format!("{tag}v"))? } else { None };
                if let Some((_, s)) = tr.past {
                    top = top.max(s);
                }
                println!(
                    "  {} v {v:.2} x0 {x0:.1}: {}; {}; {}; rests at {} (by t{}){}",
                    if ridden { "RIDDEN" } else { "empty " },
                    tr.past.map(|(t, s)| format!("past the rail end t{t} at {s:.4} b/t")).unwrap_or_else(|| "NEVER past the rail end".into()),
                    if tr.min_z_high <= -4.51 + 1e-6 { format!("its box MET the stop wall's face z -5.000 (least centre z at the line's height {:.4})", tr.min_z_high) } else { format!("clear of the stop wall (least z at the line's height {:.4})", tr.min_z_high) },
                    tr.fell.map(|t| format!("fell t{t}")).unwrap_or_else(|| "did not fall".into()),
                    tr.last.map(|p| format!("({:.3}, {:.3}, {:.3}) cell ({}, {}, {})", p[0], p[1], p[2], cell_of(p)[0], cell_of(p)[1], cell_of(p)[2])).unwrap_or_else(|| "gone".into()),
                    tr.t_end,
                    if ridden { hp.map(|h| format!(", rider {h:.1} HP")).unwrap_or_else(|| ", rider GONE".into()) } else { String::new() }
                );
                if ridden {
                    unheld_rows_seen.push((ridden, c, x0, tr.past.map(|p| p.1), tr.min_z_high, tr.last, hp));
                } else {
                    unheld_rows_seen.push((ridden, c, x0, tr.past.map(|p| p.1), tr.min_z_high, tr.last, None));
                }
                clear_carts(srv, &[&tag])?;
            }
        }
    }
    println!("  --unheld: the fastest exit {top:.4} b/t");
    let _ = std::fs::remove_dir_all(&srv.dir);
    // (G1's pattern) the first run's answers, with both boosters laid (block, torch or pane: the same to the 4th decimal)
    if booster2 {
        let lands = |p: Option<[f64; 3]>| p.is_some_and(|p| cell_of(p) == [9, -10, -4]);
        let wall = |z: f64| (z + 4.51).abs() < 1e-6;
        let ok = unheld_rows_seen.len() == 30
            && unheld_rows_seen.iter().all(|r| {
                if r.0 {
                    r.3.is_some_and(|s| (0.80..=1.075).contains(&s)) && wall(r.4) && lands(r.5) && r.6 == Some(20.0)
                } else if r.1 == 10 && (r.2 - 8.2).abs() < 1e-9 {
                    r.3.is_none() && r.5.is_some_and(|p| cell_of(p) == [9, 25, 8])
                } else {
                    r.3.is_some_and(|s| s >= 0.27) && wall(r.4) && lands(r.5)
                }
            });
        pin(7, ok, "(7) unheld (P1 powered) with both boosters: every ridden cart leaves the rail end at 0.80 .. 1.075 b/t, meets the stop wall flush (least centre z at the line's height -4.51), and comes to rest in (9,-10,-4) at 20.0 HP; every empty one leaves at >= 0.27, meets the wall and lands there, except 0.10 b/t from x0 8.2, which stops on (9,25,8)".into())?;
        println!("  every known answer held (assert 7)");
    }
    Ok(())
}

/// (b8949c96 04:44 (b)) `--front-at Z`: a RIDDEN cart planted at rest on the landing slope (9,-10,-4) at z Z (y on the
/// ascending_north slope: -10 + (1 - frac(Z)) + 0.0625), read after 20 gt where it settles; then a ridden napper released
/// from P1 (z 5.1222, then 5.7322, a fresh front each) falls onto it. Printed: the front's settled place, the napper's rest
/// and both riders' health (the car-4 rule: a 31-block fall onto a ridden cart).
fn front_rows(srv: &mut VanillaServer, zf: f64, front_empty: bool) -> Result<()> {
    println!("  front {}", if front_empty { "EMPTY (no rider: ironspec 05:57)" } else { "RIDDEN" });
    println!("  --front-at {zf}: a ridden cart planted at rest on the landing slope (9,-10,-4), a ridden napper released from P1 onto it");
    let frac = zf - zf.floor();
    let yf = -10.0 + (1.0 - frac) + 0.0625;
    let mut k = 0u32;
    // per release: (the napper's rider died, its health, the next napper's rider died)
    let mut front_seen: Vec<(bool, Option<f64>, bool, Option<f64>, bool)> = Vec::new();
    for rel in [5.1222, 5.7322] {
        k += 1;
        let (f, r) = (format!("ff{k}"), format!("fn{k}"));
        srv.cmd("setblock 9 21 5 minecraft:glass")?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 9.5 {yf:.4} {zf:.4} {{Tags:[\"{f}\"],Motion:[0.0d,0.0d,0.0d]{}}}",
            if front_empty { String::new() } else { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{f}v\"]}}]") }
        ))?;
        srv.step(20)?;
        let fp = nums(&srv.cmd_long(&format!("data get entity @e[tag={f},limit=1] Pos"))?);
        srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {:.4} {rel:.4} {{Tags:[\"{r}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{r}v\"],CustomName:\"{r}v\"}}]}}", 22.0 + (rel - 5.0) + 0.0625))?;
        srv.step(10)?;
        srv.cmd_ok("setblock 9 21 5 minecraft:redstone_block")?;
        let tr = track(srv, &r, 600)?;
        let (hn, hf) = (health(srv, &format!("{r}v"))?, health(srv, &format!("{f}v"))?);
        let fp2 = nums(&srv.cmd_long(&format!("data get entity @e[tag={f},limit=1] Pos"))?);
        let log = std::fs::read_to_string(srv.dir.join("logs").join("latest.log")).unwrap_or_default();
        let death = log.lines().find(|l| l.contains("Named entity") && l.contains(&format!("['{r}v'/")) && l.contains(" died: ")).map(|l| l.split(" died: ").nth(1).unwrap_or("").to_string());
        println!(
            "  front planted at z {zf:.4} y {yf:.4}: settled at {}; napper from z {rel:.4}: {}; {}; rests at {} (by t{}); the NAPPER's rider {}; the FRONT's rider {}; the front then at {}",
            if fp.len() == 3 { format!("({:?}, {:?}, {:?})", fp[0], fp[1], fp[2]) } else { "gone".into() },
            tr.past.map(|(t, s)| format!("past the rail end t{t} at {s:.4} b/t")).unwrap_or_else(|| "never past the rail end".into()),
            tr.fell.map(|t| format!("fell t{t}")).unwrap_or_else(|| "did not fall".into()),
            tr.last.map(|p| format!("({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])).unwrap_or_else(|| "gone".into()),
            tr.t_end,
            hn.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| format!("GONE{}", death.as_ref().map(|d| format!(" (the server log: died: {d})")).unwrap_or_default())),
            hf.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| "GONE".into()),
            if fp2.len() == 3 { format!("({:.4}, {:.4}, {:.4})", fp2[0], fp2[1], fp2[2]) } else { "gone".into() }
        );
        println!("    the arrival detector (9,-9,-5) powered while the napper fell and landed: {}", ticks_text(&tr.det));
        // (ironspec 06:48 (2)(i)) the lander's own cart against the detector's cover (its box reaches the detector's cell
        // z -5 .. -4 when its centre is below z -3.51; ironspec's cover bound z < -3.71), and the detector at the end
        if let Some(p) = tr.last {
            println!(
                "    the napper's cart rests at z {:.4}: {} the cover bound -3.71 by {:.4}; the detector at its rest tick t{}: {}",
                p[2],
                if p[2] < -3.71 { "INSIDE" } else { "OUTSIDE" },
                (p[2] + 3.71).abs(),
                tr.t_end,
                if tr.det.last() == Some(&tr.t_end) { "POWERED" } else { "unpowered" }
            );
        }
        // (cc767b78 05:10) the NEXT napper after it: released from P1's north edge (z 5.1222) once the first rests; where
        // all three carts end (is the off-rail cart pushed on?), the next rider's health and its death line if any
        let nx = format!("fx{k}");
        srv.cmd("setblock 9 21 5 minecraft:glass")?;
        srv.step(2)?;
        srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {:.4} 5.1222 {{Tags:[\"{nx}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{nx}v\"],CustomName:\"{nx}v\"}}]}}", 22.0 + 0.1222 + 0.0625))?;
        srv.step(10)?;
        let rp_before = nums(&srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Pos"))?);
        srv.cmd_ok("setblock 9 21 5 minecraft:redstone_block")?;
        let tn = track(srv, &nx, 600)?;
        let hx = health(srv, &format!("{nx}v"))?;
        let log2 = std::fs::read_to_string(srv.dir.join("logs").join("latest.log")).unwrap_or_default();
        let death2 = log2.lines().find(|l| l.contains("Named entity") && l.contains(&format!("['{nx}v'/")) && l.contains(" died: ")).map(|l| l.split(" died: ").nth(1).unwrap_or("").to_string());
        let rp_after = nums(&srv.cmd_long(&format!("data get entity @e[tag={r},limit=1] Pos"))?);
        let fp3 = nums(&srv.cmd_long(&format!("data get entity @e[tag={f},limit=1] Pos"))?);
        let fmt3 = |v: &[f64]| if v.len() == 3 { format!("({:.4}, {:.4}, {:.4})", v[0], v[1], v[2]) } else { "gone".into() };
        println!(
            "    NEXT napper from z 5.1222: {}; {}; rests at {} (by t{}); its rider {}; the dead napper's cart {} -> {}{}; the front at {}",
            tn.past.map(|(t, s)| format!("past the rail end t{t} at {s:.4} b/t")).unwrap_or_else(|| "never past the rail end".into()),
            tn.fell.map(|t| format!("fell t{t}")).unwrap_or_else(|| "did not fall".into()),
            tn.last.map(|p| format!("({:.4}, {:.4}, {:.4})", p[0], p[1], p[2])).unwrap_or_else(|| "gone".into()),
            tn.t_end,
            hx.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| format!("GONE{}", death2.as_ref().map(|d| format!(" (the server log: died: {d})")).unwrap_or_default())),
            fmt3(&rp_before),
            fmt3(&rp_after),
            if rp_before.len() == 3 && rp_after.len() == 3 && (0..3).any(|i| (rp_before[i] - rp_after[i]).abs() > 1e-4) { " (MOVED)" } else { " (not moved)" },
            fmt3(&fp3)
        );
        println!("    the arrival detector (9,-9,-5) powered while the NEXT napper fell and landed: {}", ticks_text(&tn.det));
        front_seen.push((hn.is_none() && death.is_some(), hn, hx.is_none() && death2.is_some(), tr.last.map(|p| p[2]), tr.det.last() == Some(&tr.t_end)));
        clear_carts(srv, &[&f, &r, &nx])?;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    // (G1's pattern) the first runs' answers, for the fronts they were run at (another Z prints only)
    let at = |z: f64| (zf - z).abs() < 1e-6;
    let first_dies = front_seen.iter().all(|s| s.0);
    let first_unhurt = front_seen.iter().all(|s| s.1 == Some(20.0));
    let next_dies = front_seen.iter().all(|s| s.2);
    // the lander's cart rests at `want` (+- 5e-4), inside the cover (z < -3.71), the detector powered at its rest tick
    let covered = |want: f64| front_seen.iter().all(|s| s.3.is_some_and(|z| (z - want).abs() < 5e-4 && z < -3.71) && s.4);
    if !front_empty && at(-3.887) {
        pin(8, front_seen.len() == 2 && first_dies && next_dies, "(8) a RIDDEN front at its game rest z -3.887: the napper's rider dies (its death line), and so does the next napper's".into())?;
        println!("  every known answer held (assert 8)");
    } else if !front_empty && at(-3.5) {
        pin(8, front_seen.len() == 2 && first_unhurt, "(8) a RIDDEN front at z -3.5: the napper perches on it unhurt (20.0 HP)".into())?;
        println!("  every known answer held (assert 8)");
    } else if front_empty && (at(-3.887) || at(-3.880) || at(-3.5) || at(-4.2)) {
        pin(8, front_seen.len() == 2 && first_unhurt && next_dies, format!("(8) an EMPTY front at z {zf}: the napper lands unhurt (20.0 HP), and the NEXT napper's rider dies on the pile (its death line)"))?;
        println!("  every known answer held (assert 8)");
    } else if front_empty && at(-3.2197) {
        // (ironspec 06:37 (2), b8949c96's PE1) the shoved empty alone: the napper lands on the front rail unhurt; the
        // next one, onto the napper's own cart still at the front, dies
        pin(8, front_seen.len() == 2 && first_unhurt && next_dies && covered(-3.9155), "(8) an EMPTY front alone at z -3.2197 (the perch-shoved rest): the napper lands unhurt (20.0 HP), its cart at z -3.9155 inside the arrival detector's cover (z < -3.71) and pressing it; the NEXT napper, onto the first napper's cart still at the front, dies (its death line)".into())?;
        println!("  every known answer held (assert 8)");
    } else if front_empty && (at(-3.65) || at(-3.35) || at(-3.08) || at(-3.05)) {
        // (ironspec 06:48 (2)(i), b8949c96 06:52) the landers onto an empty alone: unhurt, their own cart inside the cover
        let want = if at(-3.65) { -3.9799 } else if at(-3.35) { -3.9748 } else { -3.8867 };
        pin(8, front_seen.len() == 2 && first_unhurt && next_dies && covered(want), format!("(8) an EMPTY front alone at z {zf}: the napper lands unhurt (20.0 HP), its cart at z {want} inside the arrival detector's cover (z < -3.71) and pressing it at its rest; the NEXT napper, onto the first napper's cart still at the front, dies (its death line)"))?;
        println!("  every known answer held (assert 8)");
    } else {
        println!("  (no known answer for this front: printed only)");
    }
    Ok(())
}

/// (ironspec 06:48 (2)(ii), b8949c96 06:52) `--front-hold`: a RIDDEN cart (its villager's AI on) planted at rest on the
/// front slope at z -3.887 (its natural rest) and -3.8048 (where a dying napper shoves it), 1,200 gt each: the arrival
/// detector (9,-9,-5) every tick (its longest unpowered run), the cart's moves and its rider's health
fn front_hold_rows(srv: &mut VanillaServer) -> Result<()> {
    anyhow::ensure!(srv.test_block([9, -10, -4], "minecraft:powered_rail[powered=false,shape=ascending_north]")?, "the front slope (9,-10,-4) is not a braked ascending_north powered rail");
    anyhow::ensure!(srv.test_block([9, -9, -5], "minecraft:detector_rail[powered=false]")?, "the arrival detector (9,-9,-5) is not an unpowered detector rail before the rows");
    let mut seen: Vec<(f64, u32, u32, u32, Option<f64>)> = Vec::new();
    // (G1 07:10 (2)(b)/(e)) both start phases (summoned on an even and an odd tick) at each rest, and a ridden front at
    // the cover's edge (z -3.69); the least overlap of the cart's box with the detector's cell (z -5 .. -4) printed
    let mut rows: Vec<(f64, u32)> = Vec::new();
    for z in [-3.887, -3.8048, -3.69] {
        for phase in [0u32, 1] {
            rows.push((z, phase));
        }
    }
    for (k, (z, phase)) in rows.into_iter().enumerate() {
        let tag = format!("fh{k}");
        let frac = z - f64::floor(z);
        let y = -10.0 + (1.0 - frac) + 0.0625;
        srv.step(phase)?;
        srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {y:.4} {z:.4} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]}}"))?;
        let (mut on, mut run, mut longest, mut moves) = (0u32, 0u32, 0u32, 0u32);
        let mut first: Option<u32> = None;
        let mut prev: Option<Vec<f64>> = None;
        let mut last = String::new();
        let mut least_overlap = f64::INFINITY;
        for t in 1..=1200u32 {
            srv.step(1)?;
            let p = srv.test_block([9, -9, -5], "minecraft:detector_rail[powered=true]")?;
            if p {
                on += 1;
                first.get_or_insert(t);
                run = 0;
            } else if first.is_some() {
                run += 1;
                longest = longest.max(run);
            }
            let pos = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
            if pos.len() == 3 {
                least_overlap = least_overlap.min((-4.0f64).min(pos[2] + 0.49) - (-5.0f64).max(pos[2] - 0.49));
            }
            if let Some(q) = &prev {
                if pos.len() == 3 && q.len() == 3 && (0..3).any(|i| (pos[i] - q[i]).abs() > 1e-9) {
                    moves += 1;
                }
            }
            last = if pos.len() == 3 { format!("({:.4}, {:.4}, {:.4})", pos[0], pos[1], pos[2]) } else { "-".into() };
            prev = Some(pos);
        }
        let hp = health(srv, &format!("{tag}v"))?;
        let seated = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Passengers")).map(|s| s.contains("villager")).unwrap_or(false);
        println!(
            "  => RIDDEN front at z {z}, start phase {phase} ({} the cover bound -3.71 by {:.4}): the detector powered on {on} of 1200 ticks{}; LONGEST UNPOWERED RUN {longest} gt; the box's least overlap with the detector's cell z -5 .. -4: {least_overlap:.4}; the cart moved on {moves} ticks, last at {last}; its rider {}, seated: {seated}",
            if z < -3.71 { "inside" } else { "OUTSIDE" },
            (z + 3.71f64).abs(),
            first.map(|f| format!(", first at t{f}")).unwrap_or_default(),
            hp.map(|h| format!("{h:.1} HP")).unwrap_or_else(|| "GONE".into())
        );
        seen.push((z, on, longest, moves, hp));
        srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
        srv.cmd(&format!("kill @e[tag={tag}]"))?;
        srv.cmd("kill @e[type=minecraft:villager]")?;
        srv.step(30)?;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let worst = seen.iter().filter(|s| s.0 < -3.71).map(|s| s.2).max().unwrap_or(0);
    println!("  --front-hold: the detector's longest unpowered run under a resting ridden front inside the cover: {worst} gt");
    // (ironspec 06:48 (2)(ii), G1 07:10 (2)(b)/(e)) the first runs' answers: inside the cover (z < -3.71: the detector's
    // search box is its cell inset 0.2, DetectorRailBlock.getSearchBB 165-167) the detector is powered all 1,200 ticks at
    // both start phases, the cart never moves, its rider at 20.0 HP; at the cover's edge (z -3.69, 0.02 outside) the
    // detector never powers on any tick, though the cart's box overlaps the detector's cell by 0.18
    let inside: Vec<_> = seen.iter().filter(|s| s.0 < -3.71).collect();
    let edge: Vec<_> = seen.iter().filter(|s| s.0 >= -3.71).collect();
    pin(11, inside.len() == 4 && inside.iter().all(|s| s.1 == 1200 && s.2 == 0 && s.3 == 0 && s.4 == Some(20.0)), "(11) a RIDDEN front at rest at z -3.887 and -3.8048 (inside the cover), each from an even and an odd start tick, holds the arrival detector (9,-9,-5) powered all 1,200 ticks (longest unpowered run 0), never moves, its rider at 20.0 HP".into())?;
    pin(12, edge.len() == 2 && edge.iter().all(|s| s.1 == 0 && s.3 == 0 && s.4 == Some(20.0)), "(12) a RIDDEN front at rest at z -3.69 (0.02 outside the cover bound -3.71), from an even and an odd start tick, NEVER powers the arrival detector in 1,200 ticks and never moves, its rider at 20.0 HP".into())?;
    println!("  every known answer held (asserts 11..12)");
    Ok(())
}

/// (9fc3155f 06:19) `--front-string`: a hooked tripwire string at (9,-9,-4) over the front slope (9,-10,-4) (its band y
/// -8.9375 .. -8.84375), hooks (8,-9,-4) facing east / (10,-9,-4) facing west on glass (7,-9,-4) / (11,-9,-4) (V's cells
/// there replaced, printed); then one cart at a time BRAKED AT REST on the slope: EMPTY at z -3.2197 (where a perching
/// napper shoves it), EMPTY at -3.35, RIDDEN at -3.2197, and no cart; every tick for 60 gt: the string's powered state
/// and the cart's position.
fn front_string_rows(srv: &mut VanillaServer, blocks: &[(mctas::sim::Pos, String)], long: bool) -> Result<()> {
    let state_of = |c: [i32; 3]| blocks.iter().find(|(q, _)| *q == c).map(|(_, s)| s.clone()).unwrap_or_else(|| "air".into());
    for c in [[7, -9, -4], [8, -9, -4], [9, -9, -4], [10, -9, -4], [11, -9, -4]] {
        println!("  V's cell ({},{},{}): {} -> replaced", c[0], c[1], c[2], state_of(c));
    }
    srv.cmd("setblock 7 -9 -4 minecraft:glass")?;
    srv.cmd("setblock 11 -9 -4 minecraft:glass")?;
    srv.cmd("setblock 8 -9 -4 minecraft:tripwire_hook[attached=false,facing=east,powered=false]")?;
    srv.cmd("setblock 10 -9 -4 minecraft:tripwire_hook[attached=false,facing=west,powered=false]")?;
    srv.cmd("setblock 9 -9 -4 minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
    srv.step(10)?;
    anyhow::ensure!(srv.test_block([9, -9, -4], "minecraft:tripwire[attached=true,powered=false]")?, "the front string is not attached and unpowered");
    anyhow::ensure!(srv.test_block([9, -10, -4], "minecraft:powered_rail[powered=false,shape=ascending_north]")?, "the front slope (9,-10,-4) is not a braked ascending_north powered rail");
    println!("  the string (9,-9,-4) attached and unpowered; the front slope (9,-10,-4) a braked ascending_north powered rail");
    let mut k = 0u32;
    let mut strings_seen: Vec<(String, u32, Vec<u32>, u32)> = Vec::new();
    // (G1 06:31 (2)) `--long-rest`: every rest position of the front (its game rests -3.887 ridden / b8949c96's shoved
    // -3.8048 / -3.5 / -3.35 / the perch-shoved -3.2197), EMPTY and RIDDEN, 1,200 gt each: the longest unpowered run
    let rows: Vec<(String, Option<f64>, bool)> = if long {
        let mut v: Vec<(String, Option<f64>, bool)> = Vec::new();
        for z in [-3.887, -3.8048, -3.5, -3.35, -3.2197] {
            for ridden in [false, true] {
                v.push((format!("{} at rest at z {z}", if ridden { "RIDDEN" } else { "EMPTY" }), Some(z), ridden));
            }
        }
        v.push(("no cart".into(), None, false));
        v
    } else {
        vec![
            ("EMPTY at rest at z -3.2197".into(), Some(-3.2197), false),
            ("EMPTY at rest at z -3.35".into(), Some(-3.35), false),
            ("EMPTY at rest at z -3.5".into(), Some(-3.5), false),
            ("RIDDEN at rest at z -3.2197".into(), Some(-3.2197), true),
            ("no cart".into(), None, false),
        ]
    };
    let ticks: u32 = if long { 1200 } else { 100 };
    let mut long_seen: Vec<(String, u32, u32)> = Vec::new();
    for (label, z, ridden) in rows {
        let label = label.as_str();
        k += 1;
        let tag = format!("fs{k}");
        let mut box_y: Option<f64> = None;
        if let Some(z) = z {
            let frac = z - f64::floor(z);
            let y = -10.0 + (1.0 - frac) + 0.0625;
            box_y = Some(y);
            let rider = if ridden { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
            srv.cmd_ok(&format!("summon minecraft:minecart 9.5 {y:.4} {z:.4} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,0.0d]{rider}}}"))?;
        }
        let mut on = 0u32;
        let mut first: Option<u32> = None;
        let mut moves = 0u32;
        let mut prev: Option<Vec<f64>> = None;
        let mut last = String::new();
        // (ironspec 06:23 (1)) the string's power every tick (its 10-gt rechecks included), as a P / . string
        let mut bits = String::new();
        let mut off_after_first: Vec<u32> = Vec::new();
        let mut run = 0u32;
        let mut longest = 0u32;
        for t in 1..=ticks {
            srv.step(1)?;
            let p = srv.test_block([9, -9, -4], "minecraft:tripwire[powered=true]")?;
            bits.push(if p { 'P' } else { '.' });
            if p {
                on += 1;
                first.get_or_insert(t);
                run = 0;
            } else if first.is_some() {
                off_after_first.push(t);
                run += 1;
                longest = longest.max(run);
            }
            let pos = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
            if let Some(q) = &prev {
                if pos.len() == 3 && q.len() == 3 && (0..3).any(|i| (pos[i] - q[i]).abs() > 1e-9) {
                    moves += 1;
                }
            }
            if t == 1 {
                println!("    {label} t{t:>2}: string {} | cart {}", if p { "POWERED" } else { "unpowered" }, if pos.len() == 3 { format!("({:?}, {:?}, {:?})", pos[0], pos[1], pos[2]) } else { "-".into() });
            }
            last = if pos.len() == 3 { format!("({:.4}, {:.4}, {:.4})", pos[0], pos[1], pos[2]) } else { "-".into() };
            prev = Some(pos);
        }
        if long {
            let mut gaps: Vec<u32> = off_after_first.windows(2).map(|w| w[1] - w[0]).collect();
            gaps.sort_unstable();
            gaps.dedup();
            // the cart's end-of-tick box (height 0.7) against the attached string's band y -8.9375 .. -8.84375
            let cause = match box_y {
                Some(y) if ridden => format!("the cart's end-of-tick box y {y:.4} .. {:.4}; the rider's box rises above it through the band", y + 0.7),
                Some(y) if y + 0.7 >= -8.9375 => format!("its end-of-tick box y {y:.4} .. {:.4} reaches the band (-8.9375 .. -8.84375)", y + 0.7),
                Some(y) => format!("its end-of-tick box y {y:.4} .. {:.4} misses the band (-8.9375 .. -8.84375): only the move check presses, and each 10-gt recheck (TripWireBlock.tick) finds no box in it", y + 0.7),
                None => "no cart".into(),
            };
            println!(
                "  => {label}: powered on {on} of {ticks} ticks{}; unpowered after the first press on {} ticks; LONGEST UNPOWERED RUN {longest} gt{}; the cart moved on {moves} ticks, last at {last}; {cause}",
                first.map(|f| format!(", first at t{f}")).unwrap_or_default(),
                off_after_first.len(),
                if gaps.is_empty() { String::new() } else { format!(" (the gaps between unpowered ticks: {:?} gt)", gaps) }
            );
            long_seen.push((label.to_string(), on, longest));
        } else {
            println!("    {label} t1..t{ticks}: {bits}");
            println!("    {label}: unpowered after the first press at {}", ticks_text(&off_after_first));
            println!("  => {label}: the string powered on {on} of {ticks} ticks{}; the cart moved on {moves} ticks, last at {last}", first.map(|f| format!(", first at t{f}")).unwrap_or_default());
        }
        strings_seen.push((label.to_string(), on, off_after_first.clone(), moves));
        srv.cmd(&format!("execute as @e[tag={tag}] run tp @s 9.5 -40 30"))?;
        srv.cmd(&format!("kill @e[tag={tag}]"))?;
        srv.cmd("kill @e[type=minecraft:villager]")?;
        srv.cmd("kill @e[type=minecraft:item]")?;
        srv.step(30)?;
        anyhow::ensure!(srv.test_block([9, -9, -4], "minecraft:tripwire[powered=false]")?, "the string did not release before the next row");
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    if long {
        // (G1 06:31 (2)) the first run's answers: every position, empty and ridden, holds the string all 1,200 ticks,
        // except the perch-shoved empty at -3.2197: 1091 of 1200, one-tick releases exactly every 11 gt (its end-of-tick
        // box misses the band, only the move check presses); the longest unpowered run anywhere is 1 gt; no cart never
        let worst = long_seen.iter().filter(|s| s.0 != "no cart").map(|s| s.2).max().unwrap_or(0);
        println!("  --long-rest: the longest unpowered run over every rest position, empty and ridden: {worst} gt");
        let get = |l: &str| long_seen.iter().find(|s| s.0 == l);
        let low = get("EMPTY at rest at z -3.2197").is_some_and(|s| s.1 == 1091 && s.2 == 1)
            && strings_seen.iter().find(|s| s.0 == "EMPTY at rest at z -3.2197").is_some_and(|s| s.2.windows(2).all(|w| w[1] - w[0] == 11) && s.3 == 0);
        let rest = long_seen.iter().filter(|s| s.0 != "EMPTY at rest at z -3.2197" && s.0 != "no cart").all(|s| s.1 == 1200 && s.2 == 0);
        let none = get("no cart").is_some_and(|s| s.1 == 0);
        pin(10, long_seen.len() == 11 && low && rest && none && worst == 1, "(10) over 1,200 gt at every front rest position (-3.887 / -3.8048 / -3.5 / -3.35 / -3.2197), empty and ridden, the string's longest unpowered run is 1 gt: only the empty at -3.2197 releases (1091 of 1200 powered, one tick every 11 gt); every other row holds all 1200; no cart never presses".into())?;
        println!("  every known answer held (assert 10)");
        return Ok(());
    }
    // (G1's pattern) the first run's answers: every cart at rest stays put; the shoved empty at -3.2197 keeps the string
    // pressed except one tick every 11 (t11, 22 .. 99: its 10-gt recheck finds no box in the band, the next raised check
    // presses again); -3.35, -3.5 and the ridden cart hold it all 100 ticks; no cart never presses it
    let get = |l: &str| strings_seen.iter().find(|s| s.0 == l);
    let elevens: Vec<u32> = (1..=9).map(|k| 11 * k).collect();
    let ok = get("EMPTY at rest at z -3.2197").is_some_and(|s| s.1 == 91 && s.2 == elevens && s.3 == 0)
        && ["EMPTY at rest at z -3.35", "EMPTY at rest at z -3.5", "RIDDEN at rest at z -3.2197"].iter().all(|l| get(l).is_some_and(|s| s.1 == 100 && s.3 == 0))
        && get("no cart").is_some_and(|s| s.1 == 0);
    pin(9, ok, "(9) carts braked at rest on the front slope press the string at (9,-9,-4): the empty at -3.2197 on 91 of 100 ticks (released one tick at t11, 22 .. 99), the empties at -3.35 / -3.5 and the ridden cart at -3.2197 on all 100, no cart never".into())?;
    println!("  every known answer held (assert 9)");
    Ok(())
}
