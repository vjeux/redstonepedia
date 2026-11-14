//! (tickexact 2026-11-14, ironspec 02:18 (2): THE PARK STRING IS CART-KEYED) `mctas mobai park-string`: on the official
//! 26.3 server, the park of 58f770cc's re-laid stop (explore/bedtier-stop 4807aba6d, bed-tier-relaid.txt md5
//! f3a818710a8f) in a fresh world, each row in its own copy 10 blocks apart (world = stop-local + (10 k, 100, 0)): the
//! park rail (1,0,3) an UNPOWERED powered rail ascending east on its glass (1,-1,3), leaning on the slime (2,0,3); the
//! plain rails west of it ((0,0,3), and (-1,0,3) for the arrivals) on glass; the park string (1,1,3) one cell above the
//! park rail, north-south between its hooks (1,1,2) facing south and (1,1,4) facing north, on glass (1,1,1) / (1,1,5)
//! (the stop's own cells; the stop's drive repeater stays out: the park is braked throughout). One EMPTY minecart per
//! row (none in the control); every tick: the string's and both hooks' powered states, the cart's position and its box
//! top against the string's band (an attached string's box: y + 1/16 .. + 2.5/16 of its cell). Rows:
//! - (i) AT REST on the park at x 1.06 .. 1.61 by 0.05 (the reachable rest range), summoned at the slope's own height
//!   (cell y + (x - 1) + 0.0625) with no motion, 60 gt;
//! - (ii) ARRIVING from the plain rail west (x -0.5) moving east (up into the park) at 0.05 .. 0.60 b/t by 0.05, 100 gt;
//! - the control: no cart.
//! Each row prints the string's presses and RELEASES (a tick unpowered after the first press: the 10-gt re-check
//! finding no box in the string's band). First, the park laid as the file reads it (after its west rails, nothing above
//! its east end) to read the shape 26.3's RailState gives it (flat); the rows' park is laid against a temporary rail at
//! (2,1,3), then removed. Asserted (MCTAS_PARK_STRING_PLANT=k inverts assert k): (1) laid as the file reads it the park
//! goes FLAT; (2) at rest at x 1.06 .. 1.26: pressed from t1, released for one tick on rows 11 / 22 / 33 / 44 / 55; (3)
//! at rest at x 1.31 .. 1.61: pressed all 60 ticks; (4) arriving at 0.05 b/t: never reaches the park; (5) arriving at
//! 0.10 / 0.15 / 0.20 / 0.25 / 0.40: rests below x 1.30, a one-tick release every 11 gt after the first press; (6)
//! arriving at 0.30 / 0.35 / 0.45 .. 0.60: rests at x >= 1.30, never released; (7) the control never pressed; (8) a
//! release happens exactly when the rest is below x 1.30.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

/// The stop-local frame's y 0 in the world.
const Y: i32 = 100;

fn w(k: i32, p: [i32; 3]) -> [i32; 3] {
    [p[0] + 10 * k, p[1] + Y, p[2]]
}

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

fn powered(srv: &mut VanillaServer, p: [i32; 3], block: &str) -> Result<bool> {
    srv.test_block(p, &format!("{block}[powered=true]"))
}

fn cart_pos(srv: &mut VanillaServer, tag: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None })
}

/// One row's cart: stop-local position (x, y relative to the frame's y 0, z) and motion.
struct Cart {
    pos: [f64; 3],
    motion: [f64; 3],
}

/// One row's record.
pub struct Seen {
    pub on: usize,
    pub first: Option<u32>,
    /// ticks unpowered after the first press
    pub releases: Vec<u32>,
    /// the cart's stop-local x on the last tick
    pub rest_x: Option<f64>,
}

fn copy(srv: &mut VanillaServer, k: i32, label: &str, cart: Option<Cart>, ticks: u32) -> Result<Seen> {
    for p in [[-1, -1, 3], [0, -1, 3], [1, -1, 3], [2, 0, 3], [1, 1, 1], [1, 1, 5]] {
        set(srv, w(k, p), if p == [2, 0, 3] { "minecraft:slime_block" } else { "minecraft:glass" })?;
    }
    set(srv, w(k, [-1, 0, 3]), "minecraft:rail[shape=east_west]")?;
    set(srv, w(k, [0, 0, 3]), "minecraft:rail[shape=east_west]")?;
    // 26.3's RailState.place (226-327) reshapes a rail laid with the flat rail west of it and no rail above its east
    // end to EAST_WEST, so the park is laid against a temporary rail at (2,1,3) on the slime (its ascending check
    // finds it), which is then removed (a neighbour's removal recomputes only a powered rail's POWERED)
    set(srv, w(k, [2, 1, 3]), "minecraft:rail")?;
    set(srv, w(k, [1, 0, 3]), "minecraft:powered_rail[shape=ascending_east,powered=false]")?;
    set(srv, w(k, [2, 1, 3]), "minecraft:air")?;
    set(srv, w(k, [1, 1, 2]), "minecraft:tripwire_hook[attached=false,facing=south,powered=false]")?;
    set(srv, w(k, [1, 1, 4]), "minecraft:tripwire_hook[attached=false,facing=north,powered=false]")?;
    set(srv, w(k, [1, 1, 3]), "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
    srv.step(5)?;
    let attached = srv.test_block(w(k, [1, 1, 3]), "minecraft:tripwire[attached=true]")?;
    let park_ok = srv.test_block(w(k, [1, 0, 3]), "minecraft:powered_rail[shape=ascending_east,powered=false]")?;
    let tag = format!("parkcart{k}");
    if let Some(c) = &cart {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Motion:[{:.4}d,{:.4}d,{:.4}d]}}",
            c.pos[0] + 10.0 * k as f64,
            c.pos[1] + Y as f64,
            c.pos[2],
            c.motion[0],
            c.motion[1],
            c.motion[2]
        ))?;
    }
    println!("  copy {k}: {label}; the string attached {attached}, the park rail in place and unpowered {park_ok}");
    anyhow::ensure!(attached && park_ok, "copy {k}: the park is not as laid (string attached {attached}, park rail {park_ok})");
    let (mut on, mut first, mut releases, mut last) = (0usize, None::<u32>, Vec::new(), None::<[f64; 3]>);
    for t in 1..=ticks {
        srv.step(1)?;
        let s = powered(srv, w(k, [1, 1, 3]), "minecraft:tripwire")?;
        let hn = powered(srv, w(k, [1, 1, 2]), "minecraft:tripwire_hook")?;
        let hs = powered(srv, w(k, [1, 1, 4]), "minecraft:tripwire_hook")?;
        if s {
            on += 1;
            first.get_or_insert(t);
        } else if first.is_some() {
            releases.push(t);
        }
        let p = if cart.is_some() { cart_pos(srv, &tag)? } else { None };
        if p.is_some() {
            last = p;
        }
        println!(
            "    t{t:>3}: string {} | hooks {} / {} | cart {}",
            s as u8,
            hn as u8,
            hs as u8,
            p.map(|p| format!("x {:.4} y {:.4} (box top {:.4}; the band 1.0625..1.1563)", p[0] - 10.0 * k as f64, p[1] - Y as f64, p[1] - Y as f64 + 0.7)).unwrap_or_else(|| "-".into())
        );
    }
    let rest_x = last.map(|p| p[0] - 10.0 * k as f64);
    println!(
        "    => the string powered on {on} of {ticks} ticks{}; {}; the cart ends at x {}",
        first.map(|f| format!(", first at t{f}")).unwrap_or_default(),
        if releases.is_empty() { "no release after the first press".to_string() } else { format!("RELEASED (unpowered after the first press) on rows {releases:?}") },
        rest_x.map(|x| format!("{x:.4}")).unwrap_or_else(|| "-".into())
    );
    Ok(Seen { on, first, releases, rest_x })
}

pub fn park_string(version: &str) -> Result<()> {
    println!("== park-string on the official {version} server (ironspec 02:18 (2)): the re-laid stop's park (4807aba6d): an EMPTY cart on the braked ascending-east park rail under the park string, at rest over the reachable rest range and arriving at every speed; every tick printed (stop-local x / y)");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 271 15")?;
    srv.step(20)?;
    // the stop's own order with no temporary rail: what shape does the park take when laid after its west rails?
    let flat_shape = {
        let k = -1;
        for p in [[-1, -1, 3], [0, -1, 3], [1, -1, 3], [2, 0, 3]] {
            set(&mut srv, w(k, p), if p == [2, 0, 3] { "minecraft:slime_block" } else { "minecraft:glass" })?;
        }
        set(&mut srv, w(k, [-1, 0, 3]), "minecraft:rail[shape=east_west]")?;
        set(&mut srv, w(k, [0, 0, 3]), "minecraft:rail[shape=east_west]")?;
        set(&mut srv, w(k, [1, 0, 3]), "minecraft:powered_rail[shape=ascending_east,powered=false]")?;
        srv.step(2)?;
        let mut shape = "none of those".to_string();
        for s in ["ascending_east", "east_west", "ascending_west"] {
            if srv.test_block(w(k, [1, 0, 3]), &format!("minecraft:powered_rail[shape={s}]"))? {
                shape = s.to_string();
            }
        }
        println!("  the park laid as the file reads it, after its west rails, with no temporary rail: the park rail's shape {shape}");
        shape
    };
    let mut rows: Vec<(String, Option<Cart>, u32)> = Vec::new();
    for i in 0..12 {
        let x = 1.06 + 0.05 * i as f64;
        rows.push((format!("(i) at rest at x {x:.2}"), Some(Cart { pos: [x, (x - 1.0) + 0.0625, 3.5], motion: [0.0; 3] }), 60));
    }
    for i in 1..=12 {
        let v = 0.05 * i as f64;
        rows.push((format!("(ii) arriving from x -0.5 moving east at {v:.2} b/t"), Some(Cart { pos: [-0.5, 0.0625, 3.5], motion: [v, 0.0, 0.0] }), 100));
    }
    rows.push(("the control: no cart".into(), None, 60));
    let mut seen: Vec<(String, Seen)> = Vec::new();
    for (k, (label, cart, ticks)) in rows.into_iter().enumerate() {
        let s = copy(&mut srv, k as i32, &label, cart, ticks).with_context(|| format!("copy {k}"))?;
        seen.push((label, s));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  summary:");
    for (label, s) in &seen {
        println!(
            "    {label}: rest x {}; powered {} ticks{}; releases {:?}",
            s.rest_x.map(|x| format!("{x:.4}")).unwrap_or_else(|| "-".into()),
            s.on,
            s.first.map(|f| format!(" from t{f}")).unwrap_or_default(),
            s.releases
        );
    }
    // (G1's pattern) the first run's answers, asserted; MCTAS_PARK_STRING_PLANT=k inverts assert k
    let s = |i: usize| &seen[i].1;
    let every_11 = |x: &Seen, ticks: u32| x.first.is_some_and(|f| x.releases == (f + 10..=ticks).step_by(11).collect::<Vec<u32>>());
    pin(1, flat_shape == "east_west", format!("(1) the park laid as the file reads it, after its west rails and with nothing above its east end, is re-shaped FLAT (east_west): {flat_shape}"))?;
    pin(2, (0..5).all(|i| s(i).first == Some(1) && s(i).on == 55 && s(i).releases == vec![11, 22, 33, 44, 55]), format!("(2) at rest at x 1.06 .. 1.26: pressed from t1 and released for one tick on rows 11 / 22 / 33 / 44 / 55: {:?}", (0..5).map(|i| (s(i).on, s(i).releases.clone())).collect::<Vec<_>>()))?;
    pin(3, (5..12).all(|i| s(i).first == Some(1) && s(i).on == 60), format!("(3) at rest at x 1.31 .. 1.61: pressed on all 60 ticks: {:?}", (5..12).map(|i| s(i).on).collect::<Vec<_>>()))?;
    pin(4, s(12).on == 0 && s(12).rest_x.is_some_and(|x| x < 1.0), format!("(4) arriving at 0.05 b/t: never reaches the park (rests short of x 1.0), never pressed: rest {:?}, {} ticks", s(12).rest_x, s(12).on))?;
    let low = [13usize, 14, 15, 16, 19];
    pin(5, low.iter().all(|&i| s(i).rest_x.is_some_and(|x| x < 1.30) && every_11(s(i), 100)), format!("(5) arriving at 0.10 / 0.15 / 0.20 / 0.25 / 0.40 b/t: rests below x 1.30 and, after the first press, releases for one tick every 11 gt (first + 10, + 21, ..): {:?}", low.iter().map(|&i| (s(i).rest_x, s(i).first, s(i).releases.clone())).collect::<Vec<_>>()))?;
    let high = [17usize, 18, 20, 21, 22, 23];
    pin(6, high.iter().all(|&i| s(i).rest_x.is_some_and(|x| x >= 1.30) && s(i).first.is_some() && s(i).releases.is_empty()), format!("(6) arriving at 0.30 / 0.35 / 0.45 .. 0.60 b/t: rests at x >= 1.30, pressed and never released: {:?}", high.iter().map(|&i| (s(i).rest_x, s(i).first, s(i).releases.len())).collect::<Vec<_>>()))?;
    pin(7, s(24).on == 0, format!("(7) the control (no cart): never pressed: {}", s(24).on))?;
    pin(8, (0..24).filter(|&i| s(i).first.is_some()).all(|i| s(i).releases.is_empty() == s(i).rest_x.is_some_and(|x| x >= 1.30)), "(8) over every pressed row: a release happens exactly when the cart rests below x 1.30 (its box top under the band's 1.0625)".to_string())?;
    println!("  every known answer held (asserts 1..8)");
    Ok(())
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_PARK_STRING_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}
