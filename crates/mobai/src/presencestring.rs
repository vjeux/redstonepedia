//! (tickexact 2026-11-13, ironspec 10:52 / 10:54) The presence-string row on the official server: a
//! tripwire string two blocks above a resting rail cell R (the y-26 form over R at y 24: the string
//! at R + 2, its hooks either side on glass), (A) a minecart ridden by a seated villager resting on R
//! under it, (B) an EMPTY minecart resting on a second R under its own string; each tick the two
//! strings' POWERED state, the rider's and cart's y and the rider's box top; a world open (save,
//! stop, start) mid-rest; then the ridden cart and its rider removed (the departure) and the release
//! tick. Bars: (A) pressed every tick from its first press to the departure, across the open, and
//! released <= 10 gt after; (B) never pressed.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn build(srv: &mut VanillaServer, x: i32, rise: i32) -> Result<()> {
    for (dx, dy, s) in [
        (0, -1, "minecraft:glass"),
        (0, 0, "minecraft:rail[shape=east_west,waterlogged=false]"),
        (-1, -1, "minecraft:glass"),
        (1, -1, "minecraft:glass"),
        (-1, 0, "minecraft:glass"),
        (1, 0, "minecraft:glass"),
        (-2, rise, "minecraft:glass"),
        (2, rise, "minecraft:glass"),
        (-1, rise, "minecraft:tripwire_hook[attached=true,facing=east,powered=false]"),
        (1, rise, "minecraft:tripwire_hook[attached=true,facing=west,powered=false]"),
        (0, rise, "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]"),
    ] {
        srv.cmd_ok(&format!("setblock {} {} 0 {s}", x + dx, Y + dy))?;
    }
    Ok(())
}

fn pressed(srv: &mut VanillaServer, x: i32, rise: i32) -> Result<bool> {
    crate::strict::test(srv, &format!("execute if block {x} {} 0 minecraft:tripwire[powered=true]", Y + rise))
}

fn y_of(srv: &mut VanillaServer, sel: &str) -> Result<Option<f64>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos[1]"))?;
    Ok(out.split("data: ").nth(1).and_then(|r| r.trim().trim_end_matches('d').parse().ok()))
}

/// The row at the y-26 form (the string two blocks above R).
pub fn presence_string(version: &str, ticks: u32, open_at: u32, depart_at: u32) -> Result<()> {
    presence_string_at(version, ticks, open_at, depart_at, 2)
}

/// The same row with the string `rise` blocks above R (2 = the y-26 form, 1 = one lower).
pub fn presence_string_at(version: &str, ticks: u32, open_at: u32, depart_at: u32, rise: i32) -> Result<()> {
    anyhow::ensure!((1..=3).contains(&rise), "--rise 1..3");
    anyhow::ensure!(open_at < depart_at && depart_at + 20 <= ticks, "--open-at < --depart-at <= --ticks - 20");
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -16 -16 31 15")?;
    srv.cmd_ok("time set 13000")?;
    build(&mut srv, 0, rise)?;
    build(&mut srv, 10, rise)?;
    let out = srv.cmd(&format!("summon minecraft:minecart 0.5 {} 0.5 {{Tags:[\"pa\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"pav\"],PersistenceRequired:1b}}]}}", Y as f64 + 0.0625))?;
    anyhow::ensure!(out.contains("Summoned new"), "the ridden cart: {}", out.trim());
    let out = srv.cmd(&format!("summon minecraft:minecart 10.5 {} 0.5 {{Tags:[\"pb\"]}}", Y as f64 + 0.0625))?;
    anyhow::ensure!(out.contains("Summoned new"), "the empty cart: {}", out.trim());
    println!("== presence-string on the official {version} server: string at R + {rise} (y {}), its hooks either side on glass; (A) a cart ridden by a seated villager resting on R (y {Y}) under it at x 0; (B) an empty cart on R under its own string at x 10; world open before t{open_at}; the ridden cart and its rider removed before t{depart_at}", Y + rise);
    let (mut first_press, mut a_gaps, mut b_presses, mut release) = (None::<u32>, Vec::new(), 0u32, None::<u32>);
    let (mut top_min, mut top_max) = (f64::MAX, f64::MIN);
    for t in 0..ticks {
        if t == open_at {
            let (s, _) = crate::restartcheck::reload(srv, |w| opts(Some(w)), None)?;
            srv = s;
            println!("  t{t}: world open (save, stop, start)");
        }
        if t == depart_at {
            // the rider leaves at once (a teleported passenger dismounts) and the cart goes: no death
            // animation keeps a box under the string
            srv.cmd_ok(&format!("tp @e[tag=pav] 0.5 {} 0.5", Y - 40))?;
            srv.cmd_ok("kill @e[tag=pa]")?;
            println!("  t{t}: the rider teleported away (it dismounts) and the ridden cart removed (the departure; the rider leaves the string's box this tick)");
        }
        srv.cmd_ok("tick step 1")?;
        let a = pressed(&mut srv, 0, rise)?;
        let b = pressed(&mut srv, 10, rise)?;
        let ry = if t < depart_at { y_of(&mut srv, "@e[tag=pav,limit=1]")? } else { None };
        let cy = if t < depart_at { y_of(&mut srv, "@e[tag=pa,limit=1]")? } else { None };
        if let Some(r) = ry {
            top_min = top_min.min(r + 1.95);
            top_max = top_max.max(r + 1.95);
        }
        if t < 12 || t % 20 == 0 || (t + 2 >= open_at && t <= open_at + 2) || (t + 2 >= depart_at && t <= depart_at + 12) {
            println!(
                "  t{t}: string A {} | string B {} | rider y {} (box top {}) cart y {}",
                if a { "PRESSED" } else { "released" },
                if b { "PRESSED" } else { "released" },
                ry.map(|v| format!("{v:.6}")).unwrap_or("-".into()),
                ry.map(|v| format!("{:.6}", v + 1.95)).unwrap_or("-".into()),
                cy.map(|v| format!("{v:.6}")).unwrap_or("-".into())
            );
        }
        if a && first_press.is_none() {
            first_press = Some(t);
        }
        if t < depart_at && first_press.is_some() && !a {
            a_gaps.push(t);
        }
        if t >= depart_at && !a && release.is_none() {
            release = Some(t);
        }
        b_presses += u32::from(b);
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!(
        "  the rider's box top over the rest: {top_min:.6}..{top_max:.6} (the string's slab: {:.6}..{:.6})",
        (Y + rise) as f64 + 1.0 / 16.0,
        (Y + rise) as f64 + 2.5 / 16.0
    );
    println!(
        "  bars: (A) first press {}; unpressed ticks after it before the departure: {} {:?}; released at {} ({} gt after the departure); (B) the empty cart's string pressed on {b_presses} tick(s)",
        first_press.map(|t| format!("t{t}")).unwrap_or("NEVER".into()),
        a_gaps.len(),
        a_gaps.iter().take(8).collect::<Vec<_>>(),
        release.map(|t| format!("t{t}")).unwrap_or("never".into()),
        release.map(|t| (t - depart_at).to_string()).unwrap_or("-".into())
    );
    // (G1 on P #2) the bars reach the exit code: (A) pressed from its first press every tick to the
    // departure and released <= 10 gt after the rider left (the departure tick here); (B) never pressed
    let a_ok = first_press.is_some() && a_gaps.is_empty() && release.is_some_and(|r| r - depart_at <= 10);
    let b_ok = b_presses == 0;
    println!("  verdict: (A) {} (B) {}", if a_ok { "HELD" } else { "FAILED" }, if b_ok { "HELD" } else { "FAILED" });
    anyhow::ensure!(a_ok && b_ok, "a presence-string bar failed (A {a_ok}, B {b_ok}): the run fails");
    Ok(())
}

/// (farmopt / ironfarm 11:41) The presence-string row on the DESIGN's R: R an unpowered powered rail
/// ascending south (the brake) on slime at (x, Y+24, 7), F a north-south rail at (x, Y+25, 8) on
/// glass, flat rails north of R at (x, Y+24, 4..6) on glass (the departure's track), the string at R's
/// cell + 2 (x, Y+26, 7) with its hooks east-west on glass. Lanes: ridden carts rolled in from F
/// heading north at v (the design band at K), ridden carts placed at rest at the lowest and highest
/// measured rests (z 7.343 / 7.922), and empty carts the same; per tick each string and each rider's
/// box top; a world open; the departure (R swapped for a plain ascending rail: the brake released,
/// the cart rolls north off R).
pub fn presence_slope(version: &str, ticks: u32, open_at: u32, depart_at: u32) -> Result<()> {
    const B: i32 = 80;
    // (name, x, ridden, Some(v) = rolled in from F at vz -v, None = placed at rest at z)
    let lanes: Vec<(&str, i32, bool, Option<f64>, f64)> = vec![
        ("ridden, rolled in at v 0.10", 0, true, Some(0.10), 0.0),
        ("ridden, rolled in at v 0.30", 6, true, Some(0.30), 0.0),
        ("ridden, rolled in at v 0.44", 12, true, Some(0.44), 0.0),
        ("ridden, placed at rest z 7.343", 18, true, None, 7.343),
        ("ridden, placed at rest z 7.922", 24, true, None, 7.922),
        ("EMPTY, placed at rest z 7.343", 30, false, None, 7.343),
        ("EMPTY, placed at rest z 7.922", 36, false, None, 7.922),
        ("EMPTY, rolled in at v 0.30", 42, false, Some(0.30), 0.0),
    ];
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -16 -16 63 31")?;
    srv.cmd_ok("time set 13000")?;
    let y = |dy: i32| B + dy;
    for (_, x, _, _, _) in &lanes {
        let x = *x;
        for (dx, dy, dz, s) in [
            (0, 23, 7, "minecraft:slime_block"),
            (0, 24, 7, "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]"),
            (0, 24, 8, "minecraft:glass"),
            (0, 25, 8, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (0, 24, 9, "minecraft:glass"),
            (0, 25, 9, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (0, 23, 6, "minecraft:glass"),
            (0, 24, 6, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (0, 23, 5, "minecraft:glass"),
            (0, 24, 5, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (0, 23, 4, "minecraft:glass"),
            (0, 24, 4, "minecraft:rail[shape=north_south,waterlogged=false]"),
            (-2, 26, 7, "minecraft:glass"),
            (2, 26, 7, "minecraft:glass"),
            (-1, 26, 7, "minecraft:tripwire_hook[attached=true,facing=east,powered=false]"),
            (1, 26, 7, "minecraft:tripwire_hook[attached=true,facing=west,powered=false]"),
            (0, 26, 7, "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]"),
        ] {
            srv.cmd_ok(&format!("setblock {} {} {dz} {s}", x + dx, y(dy)))?;
        }
    }
    for (k, (_, x, ridden, v, z)) in lanes.iter().enumerate() {
        let pass = if *ridden { format!(",Passengers:[{{id:\"minecraft:villager\",Tags:[\"pv{k}\"],PersistenceRequired:1b}}]") } else { String::new() };
        let (pz, py, vz) = match v {
            Some(v) => (9.5, y(25) as f64 + 0.0625, -v),
            None => (*z, y(24) as f64 + (z - 7.0) + 0.0625, 0.0),
        };
        let out = srv.cmd(&format!("summon minecraft:minecart {} {py} {pz} {{Tags:[\"pc{k}\"],Motion:[0.0d,0.0d,{vz:?}d]{pass}}}", *x as f64 + 0.5))?;
        anyhow::ensure!(out.contains("Summoned new"), "lane {k}: {}", out.trim());
    }
    println!("== presence-string on the design's R (official {version}): R an unpowered powered rail ascending south on slime at y {} (frame 24), F north-south at y {} (frame 25), the string at R's cell + 2 (y {}, its slab {:.4}..{:.4}), hooks east-west on glass; world open before t{open_at}; the brake released (R -> a plain ascending rail) before t{depart_at}", y(24), y(25), y(26), y(26) as f64 + 1.0 / 16.0, y(26) as f64 + 2.5 / 16.0);
    let n = lanes.len();
    let mut first = vec![None::<u32>; n];
    let mut gaps = vec![0u32; n];
    let mut release = vec![None::<u32>; n];
    let mut presses = vec![0u32; n];
    let mut tops: Vec<(f64, f64)> = vec![(f64::MAX, f64::MIN); n];
    let mut rest_z: Vec<Option<f64>> = vec![None; n];
    let mut leave = vec![None::<u32>; n];
    for t in 0..ticks {
        if t == open_at {
            let (s, _) = crate::restartcheck::reload(srv, |w| opts(Some(w)), None)?;
            srv = s;
            println!("  t{t}: world open (save, stop, start)");
        }
        if t == depart_at {
            for (_, x, _, _, _) in &lanes {
                srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:rail[shape=ascending_south,waterlogged=false]", y(24)))?;
            }
            println!("  t{t}: the brake released on every lane (R -> a plain ascending rail)");
        }
        srv.cmd_ok("tick step 1")?;
        let mut line = format!("  t{t}:");
        for (k, (_, x, ridden, _, _)) in lanes.iter().enumerate() {
            let p = crate::strict::test(&mut srv, &format!("execute if block {x} {} 7 minecraft:tripwire[powered=true]", y(26)))?;
            presses[k] += u32::from(p);
            if p && first[k].is_none() {
                first[k] = Some(t);
            }
            if t < depart_at && first[k].is_some() && !p {
                gaps[k] += 1;
            }
            if t >= depart_at && !p && release[k].is_none() && first[k].is_some() {
                release[k] = Some(t);
            }
            let ry = if *ridden { y_of(&mut srv, &format!("@e[tag=pv{k},limit=1]"))? } else { None };
            // (G1 on P #2) the rider's own leave tick after the brake's release: its box no longer meets
            // the string's slab (top below the slab's bottom, or its z range [z - 0.3, z + 0.3] off the
            // string cell's [7, 8])
            if *ridden && t >= depart_at && leave[k].is_none() {
                let rz = {
                    let out = srv.cmd_long(&format!("data get entity @e[tag=pv{k},limit=1] Pos[2]"))?;
                    out.split("data: ").nth(1).and_then(|r| r.trim().trim_end_matches('d').parse::<f64>().ok())
                };
                let slab = y(26) as f64 + 1.0 / 16.0;
                let meets = matches!((ry, rz), (Some(r), Some(z)) if r + 1.95 >= slab && z + 0.3 > 7.0 && z - 0.3 < 8.0);
                if !meets {
                    leave[k] = Some(t);
                }
            }
            let cz = {
                let out = srv.cmd_long(&format!("data get entity @e[tag=pc{k},limit=1] Pos[2]"))?;
                out.split("data: ").nth(1).and_then(|r| r.trim().trim_end_matches('d').parse::<f64>().ok())
            };
            if let Some(r) = ry
                && t < depart_at
            {
                tops[k] = (tops[k].0.min(r + 1.95), tops[k].1.max(r + 1.95));
            }
            if t + 1 == open_at {
                let out = srv.cmd_long(&format!("data get entity @e[tag=pc{k},limit=1] Pos[2]"))?;
                rest_z[k] = out.split("data: ").nth(1).and_then(|r| r.trim().trim_end_matches('d').parse().ok());
            }
            line.push_str(&format!(" {k}:{}{}{}", if p { "P" } else { "-" }, ry.map(|r| format!("({:.3})", r + 1.95)).unwrap_or_default(), cz.map(|z| format!("z{z:.3}")).unwrap_or_default()));
            if k == 1 && t < 30 {
                let pr = crate::strict::test(&mut srv, &format!("execute if block {x} {} 7 minecraft:powered_rail[powered=true]", y(24)))?;
                let dm = srv.cmd_long(&format!("data get entity @e[tag=pc{k},limit=1] Motion"))?;
                line.push_str(&format!("[R powered {pr}; motion {}]", dm.split("data: ").nth(1).unwrap_or("?").trim()));
            }
        }
        if t < 40 || t % 20 == 0 || (t + 2 >= open_at && t <= open_at + 2) || (t + 1 >= depart_at && t <= depart_at + 15) {
            println!("{line}");
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let mut failed: Vec<String> = Vec::new();
    for (k, (name, _, ridden, _, _)) in lanes.iter().enumerate() {
        // (G1 on P #2) bar A for a ridden lane: pressed every tick from its first press to the brake's
        // release, and released <= 10 gt after the rider's own leave tick; bar B for an empty lane: never
        let verdict = if *ridden {
            let ok = first[k].is_some() && gaps[k] == 0 && matches!((release[k], leave[k]), (Some(r), Some(l)) if r >= l && r - l <= 10);
            if !ok {
                failed.push(format!("lane {k} ({name}): A"));
            }
            if ok { "A HELD" } else { "A FAILED" }
        } else {
            let ok = presses[k] == 0;
            if !ok {
                failed.push(format!("lane {k} ({name}): B"));
            }
            if ok { "B HELD" } else { "B FAILED" }
        };
        println!(
            "  lane {k} ({name}): rest z {}; rider box top {}; first press {}; unpressed ticks after it before the departure {}; the rider left the string's box {}; released {}; pressed ticks in all {}{}; {verdict}",
            rest_z[k].map(|z| format!("{z:.4}")).unwrap_or("-".into()),
            if *ridden { format!("{:.4}..{:.4}", tops[k].0, tops[k].1) } else { "- (empty)".into() },
            first[k].map(|t| format!("t{t}")).unwrap_or("NEVER".into()),
            gaps[k],
            leave[k].map(|t| format!("t{t} (+{} gt after the brake's release)", t - depart_at)).unwrap_or("-".into()),
            release[k].map(|t| format!("t{t} (+{} gt after the brake's release{})", t - depart_at, leave[k].map(|l| format!(", +{} after the leave", t as i64 - l as i64)).unwrap_or_default())).unwrap_or("-".into()),
            presses[k],
            if !*ridden && presses[k] > 0 { " (AN EMPTY CART PRESSED IT)" } else { "" }
        );
    }
    anyhow::ensure!(failed.is_empty(), "presence-string bars failed: {}", failed.join("; "));
    Ok(())
}
