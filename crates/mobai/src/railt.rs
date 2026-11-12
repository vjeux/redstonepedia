//! (tickexact 2026-11-12, ironspec: the rail-T rule on 7b90c3f1's in-chunk branch, a replica)
//! `mctas mobai rail-t`: on the official 26.3 server, site A (the design): a row of plain rails
//! along x at y 25, z 9 with its junction J at (0,25,9), placed after its two row arms; the
//! branch: a detector rail D at (0,24,8) ascending south up to J, the merge curve C at (0,24,7)
//! north_west (carts from the slope enter by its free south side and are carried north, the
//! siding's from the west, both leaving by its north end, a rail end over the drop at (0,24,6)),
//! and the siding rails x -6..-1 at (x,24,7), all on glass. C is kept north_west only by placing
//! it powered: D first; then a temporary helper rail at (0,24,6) on a temporary support and a
//! temporary redstone block at (1,24,7); then C (north, south and west rail neighbours, powered:
//! `RailState.place`'s powered branch ends on NORTH_WEST); the helper and its support are removed
//! (a plain rail, no signal source), then the redstone block (C is down to 2 potential
//! connections, so nothing re-settles it). Every unpowered order settles C SOUTH_WEST. Rows:
//! - (a) ridden carts along the row through J, down the slope over D (pressing and releasing it)
//!   and in from the siding, before and after a save, stop and start: J and C never change, and
//!   the slope and siding carts leave north into the drop;
//! - (c) a stone (no signal source) placed beside J at (0,25,10): no re-settle;
//! - (d) the naive order (D first, no helper; C with south and west neighbours) settles C
//!   SOUTH_WEST, and (d') with D, the helper and the siding all present (north, south and west,
//!   unpowered) it settles SOUTH_WEST too (the unpowered branch's last valid shape);
//! - (e) removing the helper re-shapes nothing (the removed block is a plain rail, no signal
//!   source);
//! - (b) the positive control, its own site: a detector D2 inline on an east-west track, then T2
//!   whose other two arms are dead ends (all three neighbours connectable): T2 re-settles to its
//!   powered shape when a cart presses D2 and back on the release (`RailBlock.updateState`: a
//!   signal source's change and exactly 3 potential connections).
//! Every watched rail's shape is printed at each tick where one changes.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::PathBuf;

const SHAPES: [&str; 10] = ["north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south", "south_east", "south_west", "north_west", "north_east"];

pub(crate) fn shape(srv: &mut VanillaServer, p: [i32; 3], block: &str) -> Result<String> {
    for s in SHAPES {
        if srv.test_block(p, &format!("{block}[shape={s}]"))? {
            let powered = if block.contains("detector") && srv.test_block(p, &format!("{block}[powered=true]"))? { "*" } else { "" };
            return Ok(format!("{s}{powered}"));
        }
    }
    Ok("none".into())
}

/// Watched rails: name, position, block.
type Rails = Vec<(&'static str, [i32; 3], &'static str)>;

fn site_a() -> Rails {
    vec![("J", [0, 25, 9], "minecraft:rail"), ("D", [0, 24, 8], "minecraft:detector_rail"), ("C", [0, 24, 7], "minecraft:rail"), ("W", [-1, 25, 9], "minecraft:rail"), ("E", [1, 25, 9], "minecraft:rail"), ("S1", [-1, 24, 7], "minecraft:rail")]
}

fn site_p(x0: i32) -> Rails {
    vec![("D2", [x0, 24, 8], "minecraft:detector_rail"), ("T2", [x0 + 1, 24, 8], "minecraft:rail"), ("T2n", [x0 + 1, 24, 7], "minecraft:rail"), ("T2s", [x0 + 1, 24, 9], "minecraft:rail")]
}

fn shapes(srv: &mut VanillaServer, rails: &Rails) -> Result<Vec<String>> {
    let mut v = Vec::new();
    for (_, p, b) in rails {
        v.push(shape(srv, *p, b)?);
    }
    Ok(v)
}

fn line(rails: &Rails, v: &[String]) -> String {
    rails.iter().zip(v).map(|((n, _, _), s)| format!("{n} {s}")).collect::<Vec<_>>().join(", ")
}

/// Steps `n` ticks, printing the rails' shapes at every tick where one changes and the tagged
/// cart's position at each such tick; returns how many times each rail changed.
fn watch(srv: &mut VanillaServer, rails: &Rails, n: u32, label: &str, prev: &mut Vec<String>) -> Result<Vec<u32>> {
    let mut changes = vec![0u32; rails.len()];
    for k in 1..=n {
        srv.step(1)?;
        let now = shapes(srv, rails)?;
        if now != *prev {
            for (i, c) in changes.iter_mut().enumerate() {
                if now[i] != prev[i] {
                    *c += 1;
                }
            }
            println!("  {label} +{k}: {}", line(rails, &now));
            *prev = now;
        }
    }
    Ok(changes)
}

/// A villager riding a minecart summoned at (x, y, z) with motion (vx, 0, vz), tagged `tag`.
pub(crate) fn cart(srv: &mut VanillaServer, p: [f64; 3], v: [f64; 2], tag: &str) -> Result<()> {
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {} {} {} {{Motion:[{}d,0.0d,{}d],Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}}]}}",
        p[0], p[1], p[2], v[0], v[1]
    ))?;
    Ok(())
}

fn cart_pos(srv: &mut VanillaServer, tag: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity @e[type=minecraft:minecart,tag={tag},limit=1] Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1], v[2]]))
}

/// Runs one cart for up to `n` ticks, watching the rails; prints where it went (its lowest y and
/// last position: below 24 north of C means it left by the north end into the drop).
fn run_cart(srv: &mut VanillaServer, rails: &Rails, p: [f64; 3], v: [f64; 2], tag: &str, label: &str, n: u32) -> Result<(Vec<u32>, String)> {
    let mut prev = shapes(srv, rails)?;
    cart(srv, p, v, tag)?;
    let mut changes = vec![0u32; rails.len()];
    let mut path = Vec::new();
    for k in 1..=n {
        let c = watch(srv, rails, 1, &format!("{label} t{k}"), &mut prev)?;
        for (i, x) in c.iter().enumerate() {
            changes[i] += x;
        }
        if let Some(q) = cart_pos(srv, tag)? {
            if k % 5 == 0 || q[1] < 23.9 {
                path.push(format!("t{k} ({:.2}, {:.2}, {:.2})", q[0], q[1], q[2]));
            }
            if q[1] < 21.0 {
                break;
            }
        }
    }
    srv.cmd_ok(&format!("kill @e[type=minecraft:minecart,tag={tag}]"))?;
    srv.cmd_ok("kill @e[type=minecraft:villager]")?;
    srv.step(1)?;
    Ok((changes, path.join(" ")))
}

/// Site A at x offset `ox`, built in `order`: "design" (the helper method), "naive" (D first, no
/// helper) or "helper-with-d" (D, the helper and the siding present when C is placed). Returns
/// C's shape after the build (and after the helper's removal).
fn build_a(srv: &mut VanillaServer, ox: i32, order: &str) -> Result<String> {
    let s = |srv: &mut VanillaServer, x: i32, y: i32, z: i32, b: &str| srv.cmd_ok(&format!("setblock {} {y} {z} {b}", ox + x));
    srv.cmd_ok(&format!("fill {} 20 0 {} 30 16 minecraft:air", ox - 8, ox + 8))?;
    srv.cmd_ok(&format!("fill {} 24 9 {} 24 9 minecraft:glass", ox - 6, ox + 6))?;
    for x in (-6..=6).filter(|x| *x != 0) {
        s(srv, x, 25, 9, "minecraft:rail[shape=east_west]")?;
    }
    s(srv, 0, 25, 9, "minecraft:rail[shape=east_west]")?;
    srv.cmd_ok(&format!("fill {} 23 7 {} 23 8 minecraft:glass", ox - 6, ox))?;
    srv.cmd_ok(&format!("fill {} 24 7 {} 24 7 minecraft:rail[shape=east_west]", ox - 6, ox - 1))?;
    let d = "minecraft:detector_rail[shape=ascending_south]";
    match order {
        "design" => {
            // D first; then the helper (with its support) north of C and a temporary redstone
            // block east of C: C placed with north, south and west rail neighbours and powered
            // settles NORTH_WEST (`RailState.place`'s powered branch ends on it); the helper is
            // removed (a plain rail, no signal source: no re-settle), then the redstone block (a
            // signal source, but C is down to 2 potential connections: no re-settle)
            s(srv, 0, 24, 8, d)?;
            s(srv, 0, 23, 6, "minecraft:glass")?;
            s(srv, 0, 24, 6, "minecraft:rail[shape=north_south]")?;
            s(srv, 1, 24, 7, "minecraft:redstone_block")?;
            s(srv, 0, 24, 7, "minecraft:rail[shape=north_west]")?;
            let c0 = shape(srv, [ox, 24, 7], "minecraft:rail")?;
            s(srv, 0, 24, 6, "minecraft:air")?;
            s(srv, 0, 23, 6, "minecraft:air")?;
            srv.step(2)?;
            let c1 = shape(srv, [ox, 24, 7], "minecraft:rail")?;
            s(srv, 1, 24, 7, "minecraft:air")?;
            srv.step(2)?;
            let c2 = shape(srv, [ox, 24, 7], "minecraft:rail")?;
            println!("  (e) C placed powered with the helper {c0}; the helper removed {c1}; the redstone block removed {c2}");
        }
        "unpowered-helper" => {
            // the helper and the siding only (D absent): exactly north and west -> NORTH_WEST;
            // then D is placed while C has 3 rail neighbours: D's update (a signal source block)
            // re-settles C unpowered
            s(srv, 0, 23, 6, "minecraft:glass")?;
            s(srv, 0, 24, 6, "minecraft:rail[shape=north_south]")?;
            s(srv, 0, 24, 7, "minecraft:rail[shape=north_west]")?;
            s(srv, 0, 24, 8, d)?;
            s(srv, 0, 24, 6, "minecraft:air")?;
            s(srv, 0, 23, 6, "minecraft:air")?;
        }
        "late-d" => {
            // the helper removed before D is placed: C's north connection is left soft, and D's
            // placement pulls C to connect it
            s(srv, 0, 23, 6, "minecraft:glass")?;
            s(srv, 0, 24, 6, "minecraft:rail[shape=north_south]")?;
            s(srv, 0, 24, 7, "minecraft:rail[shape=north_west]")?;
            s(srv, 0, 24, 6, "minecraft:air")?;
            s(srv, 0, 23, 6, "minecraft:air")?;
            s(srv, 0, 24, 8, d)?;
        }
        "naive" => {
            s(srv, 0, 24, 8, d)?;
            s(srv, 0, 24, 7, "minecraft:rail[shape=north_west]")?;
        }
        _ => {
            s(srv, 0, 24, 8, d)?;
            s(srv, 0, 23, 6, "minecraft:glass")?;
            s(srv, 0, 24, 6, "minecraft:rail[shape=north_south]")?;
            s(srv, 0, 24, 7, "minecraft:rail[shape=north_west]")?;
            let c0 = shape(srv, [ox, 24, 7], "minecraft:rail")?;
            s(srv, 0, 24, 6, "minecraft:air")?;
            s(srv, 0, 23, 6, "minecraft:air")?;
            srv.step(2)?;
            println!("  (d') C placed with D, the helper and the siding present: {c0}; after the helper's removal {}", shape(srv, [ox, 24, 7], "minecraft:rail")?);
        }
    }
    srv.step(2)?;
    shape(srv, [ox, 24, 7], "minecraft:rail")
}

fn build_p(srv: &mut VanillaServer, x0: i32) -> Result<()> {
    srv.cmd_ok(&format!("fill {} 20 0 {} 30 16 minecraft:air", x0 - 8, x0 + 8))?;
    srv.cmd_ok(&format!("fill {} 23 8 {} 23 8 minecraft:glass", x0 - 6, x0 + 1))?;
    srv.cmd_ok(&format!("fill {} 23 7 {} 23 9 minecraft:glass", x0 + 1, x0 + 1))?;
    srv.cmd_ok(&format!("fill {} 24 8 {} 24 8 minecraft:rail[shape=east_west]", x0 - 6, x0 - 1))?;
    srv.cmd_ok(&format!("setblock {x0} 24 8 minecraft:detector_rail[shape=east_west]"))?;
    srv.cmd_ok(&format!("setblock {} 24 7 minecraft:rail[shape=north_south]", x0 + 1))?;
    srv.cmd_ok(&format!("setblock {} 24 9 minecraft:rail[shape=north_south]", x0 + 1))?;
    srv.cmd_ok(&format!("setblock {} 24 8 minecraft:rail[shape=east_west]", x0 + 1))?;
    Ok(())
}

/// Site A's carts: along the row, down the slope, in from the siding.
fn carts_a(srv: &mut VanillaServer, rails: &Rails, tag: &str) -> Result<Vec<u32>> {
    let mut total = vec![0u32; rails.len()];
    let runs: [(&str, [f64; 3], [f64; 2]); 3] = [
        ("row (west to east through J)", [-5.5, 25.0625, 9.5], [0.4, 0.0]),
        ("slope (down over D, into C by its free south side)", [0.5, 24.6, 8.6], [0.0, -0.3]),
        ("siding (from the west into C)", [-5.5, 24.0625, 7.5], [0.4, 0.0]),
    ];
    for (i, (name, p, v)) in runs.iter().enumerate() {
        let (c, path) = run_cart(srv, rails, *p, *v, &format!("rt{}{i}", tag.len()), &format!("{tag} {name}"), 80)?;
        println!("  {tag} {name}: changes {:?}; path {path}", c);
        for (k, x) in c.iter().enumerate() {
            total[k] += x;
        }
    }
    Ok(total)
}

pub fn rail_t(version: &str) -> Result<()> {
    println!("== rail-t on the official {version} server: 7b90c3f1's in-chunk branch (a replica)");
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        world,
        // the rail agent names the call path of every re-settle at the probes' J and C
        java_agent: mctas::bridge::rail_agent(version).ok().map(|jar| PathBuf::from(format!("{}=150,25,9;170,25,9;170,24,7;0,25,9", jar.display()))),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 223 31")?;
    srv.step(20)?;
    let c_design = build_a(&mut srv, 0, "design")?;
    let c_naive = build_a(&mut srv, 30, "naive")?;
    let c_helper_d = build_a(&mut srv, 60, "helper-with-d")?;
    let c_late_d = build_a(&mut srv, 120, "late-d")?;
    let c_unpowered_helper = build_a(&mut srv, 200, "unpowered-helper")?;
    println!("  C after the build: design order (powered placement) {c_design}; (d) naive order {c_naive}; (d') helper with D present, unpowered {c_helper_d}; the helper removed before D {c_late_d}; the helper with D placed after C {c_unpowered_helper}");
    build_p(&mut srv, 90)?;
    srv.step(5)?;
    let a = site_a();
    let p = site_p(90);
    println!("  site A at the start: {}", line(&a, &shapes(&mut srv, &a)?));
    println!("  site P at the start: {}", line(&p, &shapes(&mut srv, &p)?));
    // the J probes (copies of site A, the design order): at x 150 D's state set powered by
    // `setblock` (only the block change's own updates), at x 170 a cart set down at rest on D
    // (checkPressed: the state change, updatePowerToConnected, updateNeighborsAt on D and below)
    for (px, how) in [(150, "setblock D powered=true"), (170, "a cart at rest on D")] {
        build_a(&mut srv, px, "design")?;
        srv.step(5)?;
        let ap: Rails = site_a().into_iter().map(|(n, q, b)| (n, [q[0] + px, q[1], q[2]], b)).collect();
        let mut prev = shapes(&mut srv, &ap)?;
        println!("  probe x {px} start: {}", line(&ap, &prev));
        if px == 150 {
            srv.cmd_ok(&format!("setblock {px} 24 8 minecraft:detector_rail[shape=ascending_south,powered=true]"))?;
        } else {
            cart(&mut srv, [px as f64 + 0.5, 24.5, 8.5], [0.0, 0.0], "probe")?;
        }
        let pc = watch(&mut srv, &ap, 3, &format!("probe x {px} ({how})"), &mut prev)?;
        srv.cmd_ok("kill @e[type=minecraft:minecart,tag=probe]")?;
        srv.cmd_ok("kill @e[type=minecraft:villager]")?;
        let pc2 = watch(&mut srv, &ap, 30, &format!("probe x {px} (after)"), &mut prev)?;
        println!("  probe x {px} ({how}): changes {:?}, after {:?}", pc, pc2);
    }
    // the rail agent's reports so far (each re-settle's call path at the probes' J and C)
    let log = std::fs::read_to_string(&srv.log).unwrap_or_default();
    for l in log.lines().filter(|l| l.contains("mctas rail agent")) {
        println!("  {}", l.trim());
    }
    let a1 = carts_a(&mut srv, &a, "(a) before")?;
    let (p1, path) = run_cart(&mut srv, &p, [83.5, 24.0625, 8.5], [0.4, 0.0], "pc", "(b) a cart over D2", 60)?;
    println!("  (b) the cart over D2: changes {:?}; path {path}", p1);
    // a save, stop and start
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut srv = VanillaServer::start(&opts(Some(world)))?;
    let _ = std::fs::remove_dir_all(&old);
    srv.step(1)?;
    println!("  after the restart, first tick: {}", line(&a, &shapes(&mut srv, &a)?));
    let a2 = carts_a(&mut srv, &a, "(a) after")?;
    let mut prev = shapes(&mut srv, &a)?;
    srv.cmd_ok("setblock 0 25 10 minecraft:stone")?;
    println!("  (c) a stone placed beside J at (0,25,10)");
    let a3 = watch(&mut srv, &a, 10, "(c)", &mut prev)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    let jc = a1[0] + a2[0] + a3[0];
    let cc = a1[2] + a2[2] + a3[2];
    let t2 = p1[1];
    println!(
        "verdict: J re-settled {jc} times, C {cc} times (design order C {c_design}); naive order C {c_naive}, helper with D present C {c_helper_d}; T2 (positive control) re-settled {t2} times"
    );
    // J's re-settle at a press is a finding (BaseRailBlock.affectNeighborsAfterRemoval: a sloped
    // rail's state change updates the neighbours of the cell above it with its own block), not a
    // harness failure; the harness fails if C isn't kept north_west or the control doesn't move
    if cc > 0 || t2 == 0 || c_design != "north_west" {
        bail!("rail-t: J {jc}, C {cc}, T2 {t2}, design C {c_design}");
    }
    Ok(())
}
