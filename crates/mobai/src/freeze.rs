//! (tickexact 2026-11-12, ironspec's ruling for onechunk's loader) `mctas mobai freeze-rows`: on the
//! official 26.3 server with the load agent, onechunk's landed item-loop loader
//! (`wip/onechunk/loader/l1-item-loop-bench.txt`, main 59d96295d) with the farm module around it, as
//! onechunk lays them: the loader at its frame + (17, 0, 17), so the whole loader, its portal
//! included, sits in the gap chunk (1, 1), its Nether side at the eighth (frame + (2, 0, 2)); N = 1
//! module at frame + (6, 0, 6) (chunk (0, 0), its split row in chunk row 1); N = 4 adds (38, 0, 6)
//! and the two southern ones mirrored in z (world z = 41 - frame z). Everything shifted up by `DY`
//! (the y frames unchanged relative to each other). The module's overworld part with its ridden
//! carts; its scarers and its Nether side are left out (no rider panics, so no golem is summoned
//! and none crosses to the loader's Nether portal). Simulation and view distance 10 (the defaults).
//! Per player place (`far`: at simulation distance + 1, 11 chunks east of the easternmost farm or
//! loader chunk column, where the player's own simulation leaves that column block-ticking with its
//! entities frozen unless a ticket lifts it; `nether`: in the Nether, far from the loader's Nether
//! side): (a) the loop running 2,000 gt
//! after its build with the player there: per farm chunk its best ticket level over time (the
//! least over the live tickets of the ticket's level plus its Chebyshev distance), the ticks it was
//! entity-ticking, the ticks its blocks ticked with its entities frozen, the least entities ticked
//! a tick; (b) a save, stop and start with the player there: whether the PORTAL tickets came back
//! with the save, when the loop's first arrival renewed one, and per farm chunk how long it sat
//! block-ticking without its entities.

use crate::loadwindow::{episodes, parse};
use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DY: i32 = 80;
/// The loader's Nether side's y shift.
const NY: i32 = 60;
/// Ticks the agent prints every tick from a start (the build, the row's run, the restart's window).
const TICKS: u32 = 6000;

/// A block state mirrored in z (north and south swapped): facings, rail shapes, connections.
fn mirror_z(s: &str) -> String {
    let mut out = s.to_string();
    for (a, b) in [("facing=north", "facing=@S"), ("facing=south", "facing=north"), ("facing=@S", "facing=south")] {
        out = out.replace(a, b);
    }
    for (a, b) in [
        ("shape=ascending_north", "shape=@AS"),
        ("shape=ascending_south", "shape=ascending_north"),
        ("shape=@AS", "shape=ascending_south"),
        ("shape=north_east", "shape=@SE"),
        ("shape=south_east", "shape=north_east"),
        ("shape=@SE", "shape=south_east"),
        ("shape=north_west", "shape=@SW"),
        ("shape=south_west", "shape=north_west"),
        ("shape=@SW", "shape=south_west"),
    ] {
        out = out.replace(a, b);
    }
    // connection flags (wire, tripwire, panes, fences): north=... and south=... swapped
    let n = out.find("north=").map(|i| out[i + 6..].split([',', ']']).next().unwrap_or("").to_string());
    let so = out.find("south=").map(|i| out[i + 6..].split([',', ']']).next().unwrap_or("").to_string());
    if let (Some(n), Some(so)) = (n, so) {
        out = out.replacen(&format!("north={n}"), "north=@N", 1).replacen(&format!("south={so}"), &format!("south={n}"), 1).replacen("north=@N", &format!("north={so}"), 1);
    }
    // a door's hinge flips with the mirror
    if out.contains("_door[") {
        out = if out.contains("hinge=left") { out.replace("hinge=left", "hinge=right") } else { out.replace("hinge=right", "hinge=left") };
    }
    out
}

/// One module copy: world = frame + (dx, DY, dz), or mirrored: world z = mz - frame z.
#[derive(Clone, Copy)]
struct Copy {
    dx: i32,
    dz: i32,
    mirror: Option<i32>,
}

impl Copy {
    fn at(&self, p: [i32; 3]) -> [i32; 3] {
        match self.mirror {
            Some(mz) => [p[0] + self.dx, p[1] + DY, mz - p[2]],
            None => [p[0] + self.dx, p[1] + DY, p[2] + self.dz],
        }
    }
    fn atf(&self, p: [f64; 3]) -> [f64; 3] {
        match self.mirror {
            Some(mz) => [p[0] + self.dx as f64, p[1] + DY as f64, mz as f64 + 1.0 - p[2]],
            None => [p[0] + self.dx as f64, p[1] + DY as f64, p[2] + self.dz as f64],
        }
    }
}

fn copies(n: u32) -> Vec<Copy> {
    let mut v = vec![Copy { dx: 6, dz: 6, mirror: None }];
    if n == 4 {
        v.push(Copy { dx: 38, dz: 6, mirror: None });
        v.push(Copy { dx: 6, dz: 0, mirror: Some(41) });
        v.push(Copy { dx: 38, dz: 0, mirror: Some(41) });
    }
    v
}

/// The module's overworld part at `c`: blocks (rails as the file states them, portals last; with
/// `no_pads` its pad portal blocks left out), its ridden carts; the farm chunks it covers.
fn place_module(srv: &mut VanillaServer, d: &mctas::design::Design, c: Copy, no_pads: bool) -> Result<Vec<(i32, i32)>> {
    let mut chunks = std::collections::BTreeSet::new();
    for pass in 0..3 {
        for (p, s) in &d.blocks {
            let kind = if s.contains("nether_portal") { 2 } else if s.contains("rail") { 1 } else { 0 };
            if kind != pass {
                continue;
            }
            let w = c.at(*p);
            chunks.insert((w[0].div_euclid(16), w[2].div_euclid(16)));
            if kind == 2 && no_pads {
                continue;
            }
            let s = if c.mirror.is_some() { mirror_z(s) } else { s.clone() };
            let mode = if kind == 1 { " strict" } else { "" };
            srv.cmd_ok(&format!("setblock {} {} {} {s}{mode}", w[0], w[1], w[2]))?;
        }
    }
    for e in &d.entities {
        if e.kind != "minecraft:minecart" {
            continue;
        }
        let w = c.atf(e.pos);
        srv.cmd_ok(&format!("summon minecraft:minecart {} {} {} {{Tags:[\"farmcart\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"farmrider\"]}}]}}", w[0], w[1], w[2]))?;
    }
    Ok(chunks.into_iter().collect())
}

/// The tickets on the watched chunks at each printed line of `dim`: (game time, per chunk (cx, cz)
/// its tickets (type, level)).
fn ticket_lines(log: &str, dim: &str) -> Vec<(i64, BTreeMap<(i32, i32), Vec<(String, i32)>>)> {
    let mut out = Vec::new();
    for l in log.lines() {
        let Some(rest) = l.split("mctas-tickets ").nth(1) else { continue };
        if !rest.split(" gt ").next().unwrap_or("").contains(dim) {
            continue;
        }
        let mut parts = rest.split(" | ");
        let Some(gt) = parts.next().and_then(|h| h.rsplit(' ').next()).and_then(|v| v.parse::<i64>().ok()) else { continue };
        let mut m = BTreeMap::new();
        for p in parts {
            let Some((cxz, tickets)) = p.split_once(' ') else { continue };
            let Some((cx, cz)) = cxz.split_once(',') else { continue };
            let (Ok(cx), Ok(cz)) = (cx.parse::<i32>(), cz.parse::<i32>()) else { continue };
            let mut v = Vec::new();
            for t in tickets.split("Ticket[").skip(1) {
                let head = t.split(']').next().unwrap_or("");
                let mut w = head.split_whitespace();
                if let (Some(kind), Some(level)) = (w.next(), w.next().and_then(|x| x.parse::<i32>().ok())) {
                    v.push((kind.to_string(), level));
                }
            }
            m.insert((cx, cz), v);
        }
        out.push((gt, m));
    }
    out
}

/// The overworld's PORTAL ticket on chunk (1, 1) at each printed line: (game time, ticks left).
fn portal_ticks_left(log: &str) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for l in log.lines() {
        let Some(rest) = l.split("mctas-tickets ").nth(1) else { continue };
        if !rest.split(" gt ").next().unwrap_or("").contains("overworld") {
            continue;
        }
        let Some(gt) = rest.split(" | ").next().and_then(|h| h.rsplit(' ').next()).and_then(|v| v.parse::<i64>().ok()) else { continue };
        for p in rest.split(" | ").skip(1) {
            if !p.starts_with("1,1 ") {
                continue;
            }
            for t in p.split("Ticket[").skip(1) {
                if t.starts_with("minecraft:portal ") {
                    if let Some(left) = t.split("] with ").nth(1).and_then(|s| s.split(' ').next()).and_then(|v| v.parse::<i64>().ok()) {
                        out.push((gt, left));
                    }
                }
            }
        }
    }
    out
}

/// A chunk's best ticket level from the tickets on the watched chunks.
/// tickets, player_loading and unknown, left out: they load a chunk but never tick it).
fn best_level(tickets: &BTreeMap<(i32, i32), Vec<(String, i32)>>, c: (i32, i32)) -> Option<(i32, String)> {
    let mut best: Option<(i32, String)> = None;
    for ((x, z), v) in tickets {
        let d = (x - c.0).abs().max((z - c.1).abs());
        for (k, l) in v {
            if k.ends_with("player_loading") || k.ends_with("unknown") {
                continue;
            }
            let e = l + d;
            if best.as_ref().is_none_or(|b| e < b.0) {
                best = Some((e, format!("{k} {l} at {x},{z}")));
            }
        }
    }
    best
}

fn status(level: Option<i32>) -> &'static str {
    match level {
        Some(l) if l <= 31 => "entity-ticking",
        Some(32) => "block-ticking",
        Some(33) => "loaded",
        _ => "unloaded",
    }
}

/// Per farm chunk over the agent's lines from game time `from`: the worst best-level and its
/// status, the ticks it was entity-ticking, the ticks blocks ran with entities frozen, the least
/// entities ticked a tick.
fn report(log: &str, farm: &[(i32, i32)], from: i64, label: &str) {
    let lines: Vec<_> = parse(log).into_iter().filter(|l| l.1 >= from).collect();
    let tk = ticket_lines(log, "overworld");
    println!("  {label}: {} ticks", lines.len());
    for &c in farm {
        let mut worst: Option<(i32, i64, String)> = None;
        for (gt, m) in tk.iter().filter(|(gt, _)| *gt >= from) {
            let b = best_level(m, c);
            let lv = b.as_ref().map(|b| b.0).unwrap_or(99);
            if worst.as_ref().is_none_or(|w| lv > w.0) {
                worst = Some((lv, *gt, b.map(|b| b.1).unwrap_or_else(|| "no ticket in range".into())));
            }
        }
        let (mut et, mut frozen, mut least) = (0u32, 0u32, u32::MAX);
        let most = lines.iter().filter_map(|(_, _, ch)| ch.iter().find(|x| x.0 == c.0 && x.1 == c.1).map(|x| x.7)).max().unwrap_or(0);
        let mut short: Vec<u32> = Vec::new();
        for (tick, _, ch) in &lines {
            if let Some(&(_, _, st, bt, _, e, _, ents)) = ch.iter().find(|x| x.0 == c.0 && x.1 == c.1) {
                if e {
                    et += 1;
                    least = least.min(ents);
                    if ents < most {
                        short.push(*tick);
                    }
                } else if st || bt {
                    frozen += 1;
                }
            }
        }
        if most > 0 && !short.is_empty() {
            println!(
                "    chunk {},{}: {} ticks ticked fewer entities than its {most} (entity-ticking all the same), the first at ticks {:?}",
                c.0,
                c.1,
                short.len(),
                short.iter().take(8).collect::<Vec<_>>()
            );
        }
        println!(
            "    chunk {},{}: worst best-ticket-level {} ({}; {}, at gt {}); entity-ticking {et} of {} ticks; blocks ticking with entities frozen {frozen} ticks; least entities ticked a tick {}",
            c.0,
            c.1,
            worst.as_ref().map(|w| w.0.to_string()).unwrap_or_else(|| "-".into()),
            worst.as_ref().map(|w| status(Some(w.0))).unwrap_or("-"),
            worst.as_ref().map(|w| w.2.clone()).unwrap_or_default(),
            worst.as_ref().map(|w| w.1).unwrap_or(0),
            lines.len(),
            if least == u32::MAX { "-".to_string() } else { least.to_string() }
        );
    }
}

pub fn freeze_rows(version: &str, loader: &Path, module: &Path, n: u32, place: &str, ticks: u32) -> Result<()> {
    if n != 1 && n != 4 {
        bail!("--modules 1 or 4");
    }
    if place != "far" && place != "nether" {
        bail!("--place far or nether");
    }
    let ld = mctas::design::Design::load(loader)?;
    let md = mctas::design::Design::load(module)?;
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=-1,-1,3,3;ticks={TICKS};tickets=1;running=1", jar.display()));
    println!(
        "== freeze-rows on the official {version} server: the loader {} at its frame + (17,0,17) (chunk (1,1)), its Nether side at + (2,0,2); {n} module(s) of {}; the player {}; simulation and view distance 10",
        loader.display(),
        module.display(),
        if place == "far" { "at simulation distance + 1 (11 chunks) east of the easternmost farm or loader chunk column, in row 1" } else { "in the Nether, far from the loader's Nether side" }
    );
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: true,
        keep_dir: true,
        tick_rate: 1000,
        view_distance: 10,
        simulation_distance: 10,
        world,
        java_agent: Some(agent.clone()),
        ..Default::default()
    };
    let bot = opts(None).bot_name.clone();
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 18000")?;
    // the far place: simulation distance + 1 (11 chunks) east of the farm's and the loader's
    // easternmost chunk column, in chunk row 1 (the player's own simulation reaches that column
    // at level 32: block-ticking, its entities frozen, unless a ticket lifts it); a glass floor
    // under it, the world spawn there (a restart's login lands there too)
    let east = if n == 4 { 2 } else { 1 };
    let far = [(east + 11) * 16 + 8, DY + 1, 16 + 8];
    srv.cmd_ok(&format!("forceload add {} {} {} {}", far[0], far[2], far[0], far[2]))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:glass", far[0], far[1] - 1, far[2]))?;
    srv.cmd_ok(&format!("setworldspawn {} {} {}", far[0], far[1], far[2]))?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add 200 200 200 200")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 198 69 198 202 73 202 minecraft:air")?;
    srv.cmd_ok("execute in minecraft:the_nether run setblock 200 69 200 minecraft:glass")?;
    srv.teleport_bot(far[0] as f64 + 0.5, far[1] as f64, far[2] as f64 + 0.5)?;
    // the build, force-loaded
    srv.cmd_ok("forceload add 0 0 47 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add 0 0 15 15")?;
    srv.step(20)?;
    let t = [17, DY, 17];
    let n0 = [2, NY, 2];
    let nc = |c: String| format!("execute in minecraft:the_nether run {c}");
    srv.cmd_ok(&nc(format!("fill {} {} {} {} {} {} minecraft:air", n0[0] - 4, n0[1] + 6, n0[2] - 6, n0[0] + 8, n0[1] + 17, n0[2] + 4)))?;
    for (blocks, at, nether) in [(&ld.blocks, t, false), (&ld.nether_blocks, n0, true)] {
        for pass in 0..3 {
            for (p, s) in blocks {
                let k = if s.contains("obsidian") { 0 } else if s.contains("nether_portal") { 1 } else { 2 };
                if k != pass {
                    continue;
                }
                let c = format!("setblock {} {} {} {s}", at[0] + p[0], at[1] + p[1], at[2] + p[2]);
                srv.cmd_ok(&if nether { nc(c) } else { c })?;
            }
        }
    }
    for (p, nbt) in &ld.block_entities {
        srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", t[0] + p[0], t[1] + p[1], t[2] + p[2]))?;
    }
    for (p, nbt) in &ld.nether_block_entities {
        srv.cmd_ok(&nc(format!("data merge block {} {} {} {nbt}", n0[0] + p[0], n0[1] + p[1], n0[2] + p[2])))?;
    }
    let mut farm: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
    for c in copies(n) {
        farm.extend(place_module(&mut srv, &md, c, true)?);
    }
    let farm: Vec<(i32, i32)> = farm.into_iter().collect();
    println!(
        "  farm chunks: {farm:?}; the loader's chunk (1,1); the modules' pad portals left out (without the modules' Nether sides, an arrival from the loader's Nether portal can exit at a nearer pad: N = 4's first run lost the loop that way)"
    );
    // the loop runs 400 gt force-loaded, then the force-loads go and the player takes its place
    srv.step(400)?;
    let warm = portal_ticks_left(&std::fs::read_to_string(&srv.log)?);
    println!("  the loop during the force-loaded 400 gt: {} printed lines with the overworld's PORTAL ticket", warm.len());
    srv.cmd_ok("forceload remove 0 0 47 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove 0 0 15 15")?;
    if place == "nether" {
        srv.cmd_ok(&format!("execute in minecraft:the_nether run tp {bot} 200.5 70 200.5"))?;
    }
    let from = srv.game_time()? + 2;
    let pos = srv.cmd_long(&format!("data get entity {bot} Pos"))?;
    let dim = srv.cmd_long(&format!("data get entity {bot} Dimension"))?;
    println!(
        "  the player: {} in {}",
        pos.split("entity data: ").nth(1).unwrap_or(&pos).trim(),
        dim.split("entity data: ").nth(1).unwrap_or(&dim).trim()
    );
    srv.step(ticks)?;
    let riders = srv.cmd_long("execute if entity @e[tag=farmrider]")?;
    let log = std::fs::read_to_string(&srv.log)?;
    report(&log, &farm, from, &format!("(a) the loop running {ticks} gt with the player {place}"));
    println!("  riders alive at the end: {}", riders.trim());
    // (b) a save, stop and start with the player there
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let tk = ticket_lines(&log, "overworld");
    let before = tk.last().map(|(gt, m)| format!("gt {gt}: {m:?}")).unwrap_or_default();
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut srv = VanillaServer::start(&opts(Some(world)))?;
    let _ = std::fs::remove_dir_all(&old);
    let start_gt = srv.game_time()?;
    let pos = srv.cmd_long(&format!("data get entity {bot} Pos"))?;
    let dim = srv.cmd_long(&format!("data get entity {bot} Dimension"))?;
    println!(
        "  (b) after the restart the player is at {} in {}",
        pos.split("entity data: ").nth(1).unwrap_or(&pos).trim(),
        dim.split("entity data: ").nth(1).unwrap_or(&dim).trim()
    );
    srv.step(ticks.min(1200))?;
    let log = std::fs::read_to_string(&srv.log)?;
    let tk = ticket_lines(&log, "overworld");
    let first = tk.first().map(|(gt, m)| format!("gt {gt}: {m:?}")).unwrap_or_else(|| "no ticket line".into());
    let left = portal_ticks_left(&log);
    let renewed = left.windows(2).find(|w| w[1].0 >= start_gt && w[1].1 > w[0].1).map(|w| w[1].0 - start_gt);
    println!("  (b) the overworld's tickets before the stop: {before}");
    println!("  (b) at the start: {first}");
    println!(
        "  (b) the persisted PORTAL ticket's ticks left at the start: {}; the loop's first arrival renewing it: {}; the least ticks left after the start: {}",
        left.iter().find(|(gt, _)| *gt >= start_gt).map(|(_, l)| l.to_string()).unwrap_or_else(|| "no portal ticket".into()),
        renewed.map(|d| format!("{d} gt after the start")).unwrap_or_else(|| "none seen".into()),
        left.iter().filter(|(gt, _)| *gt >= start_gt).map(|(_, l)| *l).min().map(|v| v.to_string()).unwrap_or_else(|| "-".into())
    );
    let lines = parse(&log);
    for e in episodes(&lines).iter().filter(|e| farm.contains(&(e.cx, e.cz))) {
        println!(
            "    chunk {},{} load from tick {}: block-ticking at +{:?}, entities loaded at +{:?}, entity-ticking at +{:?}; {} ticks with its blocks ticking and its entities frozen ({} scheduled ticks run frozen)",
            e.cx, e.cz, e.start, e.bt, e.el, e.et, e.window, e.sched_frozen
        );
    }
    report(&log, &farm, start_gt, "(b) after the restart");
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
