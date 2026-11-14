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

/// A block state mirrored in z (north and south swapped): facings, rail shapes, connections, door
/// hinges. (Stairs' shape, inner/outer left and right, is not mapped: the module has no stairs.)
pub(crate) fn mirror_z(s: &str) -> String {
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

/// One module copy: world = frame + (dx, dy, dz), or mirrored: world z = mz - frame z; `alt`: the
/// copy is of the second (already mirrored) module file.
#[derive(Clone, Copy)]
struct Copy {
    dx: i32,
    dy: i32,
    dz: i32,
    mirror: Option<i32>,
    alt: bool,
}

impl Copy {
    fn at(&self, p: [i32; 3]) -> [i32; 3] {
        match self.mirror {
            Some(mz) => [p[0] + self.dx, p[1] + self.dy, mz - p[2]],
            None => [p[0] + self.dx, p[1] + self.dy, p[2] + self.dz],
        }
    }
    fn atf(&self, p: [f64; 3]) -> [f64; 3] {
        match self.mirror {
            Some(mz) => [p[0] + self.dx as f64, p[1] + self.dy as f64, mz as f64 + 1.0 - p[2]],
            None => [p[0] + self.dx as f64, p[1] + self.dy as f64, p[2] + self.dz as f64],
        }
    }
}

fn copies(n: u32) -> Vec<Copy> {
    let mut v = vec![Copy { dx: 6, dy: DY, dz: 6, mirror: None, alt: false }];
    if n == 4 {
        v.push(Copy { dx: 38, dy: DY, dz: 6, mirror: None, alt: false });
        v.push(Copy { dx: 6, dy: DY, dz: 0, mirror: Some(41), alt: false });
        v.push(Copy { dx: 38, dy: DY, dz: 0, mirror: Some(41), alt: false });
    }
    v
}

/// onechunk's rev 3 recipe: the module at frame + (6, y0, 6) (and (38, y0, 6) at N = 4), the
/// mirrored module file at frame + (6, y0, 38) and (38, y0, 38); `self_mirror` (no mirrored file:
/// a part laid on the module, b8949c96's east re-lay): the module itself mirrored there instead,
/// z' = 3 - z as onechunk's mirrored file (frame + 38: world z 41 - z), every state mirrored.
fn copies_rev3(n: u32, y0: i32, self_mirror: bool) -> Vec<Copy> {
    let mut v = vec![Copy { dx: 6, dy: y0, dz: 6, mirror: None, alt: false }];
    if n == 4 {
        v.push(Copy { dx: 38, dy: y0, dz: 6, mirror: None, alt: false });
        if self_mirror {
            v.push(Copy { dx: 6, dy: y0, dz: 0, mirror: Some(41), alt: false });
            v.push(Copy { dx: 38, dy: y0, dz: 0, mirror: Some(41), alt: false });
        } else {
            v.push(Copy { dx: 6, dy: y0, dz: 38, mirror: None, alt: true });
            v.push(Copy { dx: 38, dy: y0, dz: 38, mirror: None, alt: true });
        }
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
    portal_ticks_left_at(log, "overworld", (1, 1))
}

/// `dim`'s PORTAL ticket on chunk `c` at each printed line: (game time, ticks left).
fn portal_ticks_left_at(log: &str, dim: &str, c: (i32, i32)) -> Vec<(i64, i64)> {
    portal_left_by_line(log, dim).into_iter().filter_map(|(gt, m)| m.get(&c).map(|l| (gt, *l))).collect()
}

/// (ironspec 16:48 (1)) every printed ticket line of `dim`: its game time and each chunk's PORTAL ticks left.
fn portal_left_by_line(log: &str, dim: &str) -> Vec<(i64, BTreeMap<(i32, i32), i64>)> {
    let mut out = Vec::new();
    for l in log.lines() {
        let Some(rest) = l.split("mctas-tickets ").nth(1) else { continue };
        if !rest.split(" gt ").next().unwrap_or("").contains(dim) {
            continue;
        }
        let Some(gt) = rest.split(" | ").next().and_then(|h| h.rsplit(' ').next()).and_then(|v| v.parse::<i64>().ok()) else { continue };
        let mut m = BTreeMap::new();
        for p in rest.split(" | ").skip(1) {
            let Some((cx, cz)) = p.split_whitespace().next().and_then(|c| c.split_once(',')) else { continue };
            let (Ok(cx), Ok(cz)) = (cx.parse::<i32>(), cz.parse::<i32>()) else { continue };
            for t in p.split("Ticket[").skip(1) {
                if t.starts_with("minecraft:portal ")
                    && let Some(left) = t.split("] with ").nth(1).and_then(|s| s.split(' ').next()).and_then(|v| v.parse::<i64>().ok())
                {
                    m.insert((cx, cz), left);
                }
            }
        }
        out.push((gt, m));
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
/// entities ticked a tick. Returns (chunk, entity-ticking ticks, frozen ticks, ticks) per chunk.
fn report(log: &str, farm: &[(i32, i32)], from: i64, label: &str) -> Vec<((i32, i32), u32, u32, usize)> {
    let lines: Vec<_> = parse(log).into_iter().filter(|l| l.1 >= from).collect();
    let tk = ticket_lines(log, "overworld");
    println!("  {label}: {} ticks", lines.len());
    let mut stats = Vec::new();
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
        stats.push((c, et, frozen, lines.len()));
    }
    stats
}

/// The lowest y of the group's twin portal (its portal blocks at z 2), from the group file.
fn lowest_twin_y(group: Option<&Path>) -> Result<i32> {
    let Some(g) = group else { return Ok(0) };
    let d = mctas::design::Design::load(g)?;
    d.blocks.iter().filter(|(p, s)| s.contains("nether_portal") && p[2] == 2).map(|(p, _)| p[1]).min().ok_or_else(|| anyhow::anyhow!("the group {} has no twin portal at z 2", g.display()))
}

#[allow(clippy::too_many_arguments)]
pub fn freeze_rows(version: &str, loader: &Path, module: &Path, n: u32, place: &str, ticks: u32, group: Option<&Path>, mirrored: Option<&Path>, pocket: i32) -> Result<()> {
    if n != 1 && n != 4 {
        bail!("--modules 1 or 4");
    }
    if place != "far" && place != "nether" && place != "logout" {
        bail!("--place far, nether or logout");
    }
    let ld = mctas::design::Design::load(loader)?;
    let md = mctas::design::Design::load(module)?;
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=-1,-1,3,3;ticks={TICKS};tickets=1;running=1", jar.display()));
    println!(
        "== freeze-rows on the official {version} server: the loader {} at its frame + (17,0,17) (chunk (1,1)), its Nether side at + (2,0,2); {n} module(s) of {}; the player {}; simulation and view distance 10",
        loader.display(),
        module.display(),
        match place {
            "far" => "at simulation distance + 1 (11 chunks) east of the easternmost farm or loader chunk column, in row 1",
            "logout" => "standing at the farm (above its first chunk) for 600 gt, then logging out (ironspec 00:30 PT: every farm chunk must stay entity-ticking under the loop's tickets alone)",
            _ => "in the Nether, far from the loader's Nether side",
        }
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
    let rev3 = group.is_some();
    // rev 3 (onechunk's recipe): modules at y0, the loader 3 lower; else everything at DY
    let y0 = if rev3 { pocket } else { DY };
    let far = [(east + 11) * 16 + 8, y0 + 1, 16 + 8];
    srv.cmd_ok(&format!("forceload add {} {} {} {}", far[0], far[2], far[0], far[2]))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:glass", far[0], far[1] - 1, far[2]))?;
    srv.cmd_ok(&format!("setworldspawn {} {} {}", far[0], far[1], far[2]))?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add 200 200 200 200")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 198 69 198 202 73 202 minecraft:air")?;
    srv.cmd_ok("execute in minecraft:the_nether run setblock 200 69 200 minecraft:glass")?;
    srv.teleport_bot(far[0] as f64 + 0.5, far[1] as f64, far[2] as f64 + 0.5)?;
    // the build, force-loaded
    srv.cmd_ok("forceload add 0 0 47 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 15 15")?;
    srv.step(20)?;
    let t = if rev3 { [17, y0 - 3, 17] } else { [17, DY, 17] };
    let n0 = [2, NY, 2];
    let nc = |c: String| format!("execute in minecraft:the_nether run {c}");
    if let Some(g) = group {
        // the Nether group, unmoved (Nether world coordinates): its twin is the loader's Nether side
        let gd = mctas::design::Design::load(g)?;
        let (lo, hi) = gd.blocks.keys().fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])], [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])]));
        srv.cmd_ok(&nc(format!("fill {} {} {} {} {} {} minecraft:air", lo[0] - 2, lo[1] - 2, lo[2] - 2, hi[0] + 2, (hi[1] + 2).min(126), hi[2] + 2)))?;
        for pass in 0..3 {
            for (p, s) in &gd.blocks {
                let k = if s.contains("obsidian") { 0 } else if s.contains("nether_portal") { 2 } else { 1 };
                if k != pass {
                    continue;
                }
                let mode = if s.contains("rail") { " strict" } else { "" };
                srv.cmd(&nc(format!("setblock {} {} {} {s}{mode}", p[0], p[1], p[2])))?;
            }
        }
        for (p, nbt) in &gd.block_entities {
            srv.cmd_ok(&nc(format!("data merge block {} {} {} {nbt}", p[0], p[1], p[2])))?;
        }
        let m: Vec<String> = gd.blocks.iter().filter(|(p, s)| !srv.cmd_long(&nc(format!("execute if block {} {} {} {s}", p[0], p[1], p[2]))).map(|o| o.contains("passed")).unwrap_or(false)).map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect();
        println!("  the Nether group {} placed unmoved; module-check: {} of {} cells differ{}", g.display(), m.len(), gd.blocks.len(), if m.is_empty() { String::new() } else { format!(": {}", m.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) });
    } else {
        srv.cmd_ok(&nc(format!("fill {} {} {} {} {} {} minecraft:air", n0[0] - 4, n0[1] + 6, n0[2] - 6, n0[0] + 8, n0[1] + 17, n0[2] + 4)))?;
    }
    let loader_parts: Vec<(&BTreeMap<[i32; 3], String>, [i32; 3], bool)> = if rev3 { vec![(&ld.blocks, t, false)] } else { vec![(&ld.blocks, t, false), (&ld.nether_blocks, n0, true)] };
    for (blocks, at, nether) in loader_parts {
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
    if !rev3 {
        for (p, nbt) in &ld.nether_block_entities {
            srv.cmd_ok(&nc(format!("data merge block {} {} {} {nbt}", n0[0] + p[0], n0[1] + p[1], n0[2] + p[2])))?;
        }
    }
    let alt_design = match mirrored {
        Some(m) if m.as_os_str() != "self" => Some(mctas::design::Design::load(m)?),
        _ => None,
    };
    let self_mirror = mirrored.is_some_and(|m| m.as_os_str() == "self");
    let mut farm: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
    let list = if rev3 { copies_rev3(n, y0, self_mirror) } else { copies(n) };
    for c in list {
        let d = if c.alt { alt_design.as_ref().ok_or_else(|| anyhow::anyhow!("N = 4 on rev 3 needs --mirrored"))? } else { &md };
        farm.extend(place_module(&mut srv, d, c, !rev3)?);
    }
    let farm: Vec<(i32, i32)> = farm.into_iter().collect();
    if rev3 {
        println!("  farm chunks: {farm:?}; the loader's chunk (1,1); the modules' pads kept (they link to the group's kill room)");
    } else {
        println!(
            "  farm chunks: {farm:?}; the loader's chunk (1,1); the modules' pad portals left out (without the modules' Nether sides, an arrival from the loader's Nether portal can exit at a nearer pad: N = 4's first run lost the loop that way)"
        );
    }
    // the loop runs 400 gt force-loaded, then the force-loads go and the player takes its place
    srv.step(400)?;
    let warm = portal_ticks_left(&std::fs::read_to_string(&srv.log)?);
    println!("  the loop during the force-loaded 400 gt: {} printed lines with the overworld's PORTAL ticket", warm.len());
    srv.cmd_ok("forceload remove 0 0 47 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 15 15")?;
    if place == "nether" {
        srv.cmd_ok(&format!("execute in minecraft:the_nether run tp {bot} 200.5 70 200.5"))?;
    }
    // (ironspec 00:30 PT) the logout row: the player stands at the farm, then logs out; every farm
    // chunk's status with the player there and after it left (the loop's tickets alone)
    if place == "logout" {
        let (cx, cz) = *farm.first().ok_or_else(|| anyhow::anyhow!("no farm chunk"))?;
        let at = [cx * 16 + 8, y0 + 32, cz * 16 + 8];
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:glass", at[0], at[1] - 1, at[2]))?;
        srv.teleport_bot(at[0] as f64 + 0.5, at[1] as f64, at[2] as f64 + 0.5)?;
        let here = srv.game_time()? + 2;
        srv.step(600)?;
        let pos = srv.cmd_long(&format!("data get entity {bot} Pos"))?;
        println!("  the player at the farm: {}", pos.split("entity data: ").nth(1).unwrap_or(&pos).trim());
        report(&std::fs::read_to_string(&srv.log)?, &farm, here, "(L1) 600 gt with the player standing at the farm");
        srv.bot = None;
        srv.step(20)?;
        let list = srv.cmd("list")?;
        println!("  the player logged out: `list` says {}", list.trim());
    }
    let from = srv.game_time()? + 2;
    let pos = srv.cmd_long(&format!("data get entity {bot} Pos"))?;
    let dim = srv.cmd_long(&format!("data get entity {bot} Dimension"))?;
    println!(
        "  the player: {} in {}",
        pos.split("entity data: ").nth(1).unwrap_or(&pos).trim(),
        dim.split("entity data: ").nth(1).unwrap_or(&dim).trim()
    );
    // rev 3: every loop item's crossings, read each tick: its last cell in the twin's portal before it
    // leaves the Nether, and where it turns up in the overworld (the loader's portal, a pad, other)
    let mut nether_cells: BTreeMap<String, u32> = BTreeMap::new();
    let mut arrivals: BTreeMap<String, u32> = BTreeMap::new();
    if rev3 {
        let items = |srv: &mut VanillaServer, dim: &str, sel: &str| -> Result<Vec<(String, [f64; 3])>> {
            let u = srv.cmd_long(&format!("execute in minecraft:{dim} as @e[type=minecraft:item,{sel}] run data get entity @s UUID"))?;
            let p = srv.cmd_long(&format!("execute in minecraft:{dim} as @e[type=minecraft:item,{sel}] run data get entity @s Pos"))?;
            let us: Vec<String> = u.split("entity data: ").skip(1).map(|s| s.split(']').next().unwrap_or("").to_string()).collect();
            let ps: Vec<[f64; 3]> = p
                .split("entity data: ")
                .skip(1)
                .filter_map(|s| {
                    let v: Vec<f64> = s.split(']').next()?.trim_start_matches('[').split(',').filter_map(|t| t.trim().trim_end_matches('d').parse().ok()).collect();
                    (v.len() == 3).then(|| [v[0], v[1], v[2]])
                })
                .collect();
            Ok(us.into_iter().zip(ps).collect())
        };
        let (ny0, ny1) = (lowest_twin_y(group)?, lowest_twin_y(group)? + 6);
        let nsel = format!("x=-3,y={},z=0,dx=8,dy={},dz=4", ny0 - 1, ny1 - ny0 + 2);
        let osel = format!("x=0,y={},z=0,dx=48,dy=24,dz=48", y0 - 12);
        let mut last_nether: BTreeMap<String, [f64; 3]> = BTreeMap::new();
        let mut seen_ow: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        // items that left the Nether box, waiting to turn up in the overworld (a crossing keeps
        // the entity's UUID: `restoreFrom` loads its saved data into the new dimension's copy)
        let mut gone: BTreeMap<String, ([f64; 3], u32)> = BTreeMap::new();
        for tick in 0..ticks {
            srv.step(1)?;
            let now_n = items(&mut srv, "the_nether", &nsel)?;
            let now_o = items(&mut srv, "overworld", &osel)?;
            let in_n: std::collections::BTreeSet<&String> = now_n.iter().map(|(u, _)| u).collect();
            for (u, p) in &last_nether {
                if !in_n.contains(u) {
                    gone.insert(u.clone(), (*p, tick));
                }
            }
            for (u, p) in &now_o {
                if !seen_ow.contains(u) {
                    let (x, z) = (p[0].floor() as i32, p[2].floor() as i32);
                    let wher = if (16..=22).contains(&x) && (16..=18).contains(&z) {
                        "the loader's portal".to_string()
                    } else if ((4..=13).contains(&x) || (36..=45).contains(&x)) && ((0..=5).contains(&z) || (42..=47).contains(&z)) {
                        format!("a pad ({x},{},{z})", p[1].floor() as i32)
                    } else {
                        format!("elsewhere ({x},{},{z})", p[1].floor() as i32)
                    };
                    match gone.remove(u) {
                        Some((q, _)) => {
                            // a crossing from the Nether: its last Nether cell, where it arrived
                            *nether_cells.entry(format!("({},{},{})", q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32)).or_default() += 1;
                            *arrivals.entry(format!("from the Nether: {wher}")).or_default() += 1;
                        }
                        None => *arrivals.entry(format!("new in the overworld (the loader's own dropper's or other): {wher}")).or_default() += 1,
                    }
                }
            }
            seen_ow = now_o.iter().map(|(u, _)| u.clone()).collect();
            last_nether = now_n.into_iter().collect();
            // an item gone from the Nether box for 20 gt without turning up in the overworld:
            // absorbed there (a hopper) or out of the boxes
            let stale: Vec<String> = gone.iter().filter(|(_, (_, t))| tick - *t > 20).map(|(u, _)| u.clone()).collect();
            for u in stale {
                if let Some((q, _)) = gone.remove(&u) {
                    *arrivals.entry(format!("left the Nether box and never seen in the overworld (absorbed or out of the boxes), last at ({},{},{})", q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32)).or_default() += 1;
                }
            }
        }
    } else {
        srv.step(ticks)?;
    }
    if rev3 {
        println!("  (a) Nether -> overworld crossings, by the item's last Nether cell (cell: crossings): {nether_cells:?}");
        println!("  (a) every item's first sight in the overworld: {arrivals:?}");
        // the Nether group's chunks: their best ticket level over the row (the load agent's
        // Nether ticket lines)
        let log = std::fs::read_to_string(&srv.log)?;
        let tk = ticket_lines(&log, "nether");
        let gd = mctas::design::Design::load(group.unwrap_or(Path::new("")))?;
        let chunks: std::collections::BTreeSet<(i32, i32)> = gd.blocks.keys().map(|p| (p[0].div_euclid(16), p[2].div_euclid(16))).collect();
        for c in chunks {
            let mut worst: Option<(i32, i64, String)> = None;
            for (gt, m) in tk.iter().filter(|(gt, _)| *gt >= from) {
                let b = best_level(m, c);
                let lv = b.as_ref().map(|b| b.0).unwrap_or(99);
                if worst.as_ref().is_none_or(|w| lv > w.0) {
                    worst = Some((lv, *gt, b.map(|b| b.1).unwrap_or_else(|| "no ticket in range".into())));
                }
            }
            println!(
                "  (a) the Nether group's chunk {},{}: worst best-ticket-level {} ({}; {}, at gt {})",
                c.0,
                c.1,
                worst.as_ref().map(|w| w.0.to_string()).unwrap_or_else(|| "-".into()),
                worst.as_ref().map(|w| status(Some(w.0))).unwrap_or("-"),
                worst.as_ref().map(|w| w.2.clone()).unwrap_or_default(),
                worst.as_ref().map(|w| w.1).unwrap_or(0)
            );
        }
    }
    let riders = srv.cmd_long("execute if entity @e[tag=farmrider]")?;
    let log = std::fs::read_to_string(&srv.log)?;
    let stats = report(&log, &farm, from, &format!("(a) the loop running {ticks} gt with the player {place}"));
    // (G1 on 7354e3025) the logout row's bar: every farm chunk entity-ticking at every tick after the
    // logout, no tick with its blocks running and its entities frozen; the control, chunk (3,3)
    // (Chebyshev 2 from the loader's portal ticket at (1,1): level 32, blocks only), must show the
    // drop through the same count, or the bar can't see one
    if place == "logout" {
        let bad: Vec<String> = stats.iter().filter(|(_, et, frozen, n)| (*et as usize) < *n || *frozen > 0 || *n == 0).map(|(c, et, frozen, n)| format!("({},{}) entity-ticking {et} of {n}, frozen {frozen}", c.0, c.1)).collect();
        let ctl = report(&log, &[(3, 3)], from, "(a) the control chunk (3,3), outside the loop's entity-ticking ring (level 32 from the portal ticket)");
        let caught = ctl.iter().all(|(_, et, _, n)| *n > 0 && (*et as usize) < *n);
        println!(
            "  the logout bar (every farm chunk entity-ticking every tick after the logout): {}; its control (3,3): {}",
            if bad.is_empty() { "HELD".to_string() } else { format!("FAILED: {}", bad.join("; ")) },
            if caught { "the drop caught" } else { "NOT CAUGHT" }
        );
        anyhow::ensure!(caught, "the logout bar's control (3,3) was not caught: the count can't see a chunk leaving entity-ticking: the run fails");
        anyhow::ensure!(bad.is_empty(), "a farm chunk left entity-ticking after the logout: {}: a loader defect, the run fails", bad.join("; "));
    }
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

/// (onechunk 16:18) One part of the composed farm, every cell at its world coordinate.
#[derive(Default)]
pub struct Part {
    pub name: String,
    pub blocks: BTreeMap<[i32; 3], String>,
    pub block_entities: BTreeMap<[i32; 3], String>,
    pub entities: Vec<mctas::design::EntitySpec>,
    pub nether_blocks: BTreeMap<[i32; 3], String>,
    pub nether_block_entities: BTreeMap<[i32; 3], String>,
}

/// A world-frame file as one part: its overworld cells as they are; its Nether part (`nether layer`s, or
/// the `nether` file's own cells) as they are.
pub(crate) fn world_part(world: &Path, nether: Option<&Path>) -> Result<Part> {
    let d = mctas::design::Design::load(world)?;
    let (nb, nbe) = match nether {
        Some(p) => {
            let n = mctas::design::Design::load(p)?;
            (n.blocks, n.block_entities)
        }
        None => (d.nether_blocks.clone(), d.nether_block_entities.clone()),
    };
    Ok(Part { name: world.display().to_string(), blocks: d.blocks, block_entities: d.block_entities, entities: d.entities, nether_blocks: nb, nether_block_entities: nbe })
}

/// (onechunk 16:18) A design laid by onechunk's transform: its overworld cells, entities (their NBT HOME
/// too) and block entities moved by `offset`; its Nether part relative to the Nether image of its own
/// `port nether-anchor` (anchor + offset; x and z floored by 8, y kept): each Nether cell at image + cell,
/// its y less `nether_ybase` (the module's bench placeholder sits at nether layers 64..68: 64; the
/// loader's twin: 0).
pub(crate) fn recipe_part(file: &Path, offset: [i32; 3], nether_ybase: i32, no_pads: bool) -> Result<Part> {
    let d = mctas::design::Design::load(file)?;
    let anchor = *d.ports.get("nether-anchor").ok_or_else(|| anyhow::anyhow!("{}: no `port nether-anchor`", file.display()))?;
    let mut o = d.clone();
    o.meta.clear();
    o.nether_blocks.clear();
    o.nether_block_entities.clear();
    o.nether_ports.clear();
    // (onechunk 16:42, freeze_rows' `no_pads`) the copy's overworld pad portal blocks left out
    let pads = if no_pads { o.blocks.iter().filter(|(_, s)| s.contains("nether_portal")).count() } else { 0 };
    if no_pads {
        o.blocks.retain(|_, s| !s.contains("nether_portal"));
    }
    crate::drivereplay::shift_design(&mut o, offset)?;
    let a = [anchor[0] + offset[0], anchor[1] + offset[1], anchor[2] + offset[2]];
    let image = [a[0].div_euclid(8), a[1], a[2].div_euclid(8)];
    let n = |p: &[i32; 3]| [image[0] + p[0], image[1] + p[1] - nether_ybase, image[2] + p[2]];
    Ok(Part {
        name: format!(
            "{} + ({},{},{}) (Nether image ({},{},{}), y base {nether_ybase}){}",
            file.display(),
            offset[0],
            offset[1],
            offset[2],
            image[0],
            image[1],
            image[2],
            if no_pads { format!(", its {pads} overworld portal block(s) left out") } else { String::new() }
        ),
        blocks: o.blocks,
        block_entities: o.block_entities,
        entities: o.entities,
        nether_blocks: d.nether_blocks.iter().map(|(p, s)| (n(p), s.clone())).collect(),
        nether_block_entities: d.nether_block_entities.iter().map(|(p, s)| (n(p), s.clone())).collect(),
    })
}

/// (G1 15:31, ironspec 14:33 / 14:35 / 14:42: the loader check) `mctas mobai loader-long`: the composed
/// farm as onechunk lays it, from WORLD-FRAME design files (`world`: its overworld cells at their world
/// coordinates, its Nether part (`nether layer`s) at its Nether world coordinates; `nether`: optional, a
/// second file whose cells are the Nether's), on the official 26.3 server with the load agent printing
/// every watched chunk every tick (the overworld's load lines; both levels' tickets). The phases: the
/// build force-loaded, the loop warming 400 gt; (L1) the player standing at `farm_afk` 600 gt; (L2) the
/// player in the Nether at `nether_afk` 600 gt; the player logs out; (a) `restart_at` gt with the loop's
/// tickets alone; a save, stop and start; (b) the rest of `ticks`; (c) the player logs in again, 600 gt.
/// Printed per phase: per listed overworld route chunk its worst best-ticket-level, the ticks it was
/// entity-ticking, the ticks its blocks ran with its entities frozen; per listed Nether chunk its worst
/// best-ticket-level; per side the PORTAL ticket's least ticks left (its margin before each arrival's
/// renewal). Bars (after the logout, (a) and (b)): every listed overworld chunk entity-ticking every
/// tick, frozen 0; every listed Nether chunk entity-ticking (level <= 31) at every ticket line; each
/// side's PORTAL ticket present at every line with ticks left > 0; the control chunk (Chebyshev 2 from
/// the overworld portal chunk) seen leaving entity-ticking (or the count can't see a drop).
#[allow(clippy::too_many_arguments)]
pub fn loader_long(
    version: &str,
    parts: &[Part],
    chunks: &[(i32, i32)],
    nchunks: &[(i32, i32)],
    portal: (i32, i32),
    nportal: (i32, i32),
    ticks: u32,
    restart_at: u32,
    farm_afk: [f64; 3],
    nether_afk: [f64; 3],
    watch: [i32; 4],
) -> Result<()> {
    anyhow::ensure!(restart_at > 0 && restart_at < ticks, "--restart-at inside 1..--ticks");
    anyhow::ensure!(!parts.is_empty(), "no part to lay");
    let jar = mctas::bridge::load_agent(version)?;
    // (G1 on X #2) the agent prints both levels' tickets every tick for its first `ticks` level ticks (a
    // fresh count after a restart): every phase's tick has its line, so a lapse can't hide between lines
    let agent = PathBuf::from(format!("{}=watch={},{},{},{};ticks={};tickets=1;running=1", jar.display(), watch[0], watch[1], watch[2], watch[3], ticks + 4000));
    let ctl = (portal.0 + 2, portal.1 + 2);
    println!(
        "== loader-long on the official {version} server (pause-when-empty-seconds=0, simulation and view distance 10): {}; route chunks {chunks:?}, Nether chunks {nchunks:?}; PORTAL tickets watched at overworld {portal:?} and Nether {nportal:?}; the control chunk {ctl:?} (Chebyshev 2 from the portal chunk); the player at the farm {farm_afk:?} 600 gt, in the Nether at {nether_afk:?} 600 gt, logged out for {ticks} gt with a restart after {restart_at}, then logging in 600 gt",
        parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join("; ")
    );
    let opts = |w: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: true,
        keep_dir: true,
        tick_rate: 1000,
        view_distance: 10,
        simulation_distance: 10,
        world: w,
        java_agent: Some(agent.clone()),
        ..Default::default()
    };
    let bot = opts(None).bot_name.clone();
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 18000")?;
    let nc = |c: String| format!("execute in minecraft:the_nether run {c}");
    // the build force-loaded: every overworld and Nether chunk of the parts
    let span = |b: &mut dyn Iterator<Item = [i32; 3]>| -> Option<[i32; 6]> {
        b.fold(None, |acc: Option<[i32; 6]>, p| {
            let a = acc.unwrap_or([p[0], p[1], p[2], p[0], p[1], p[2]]);
            Some([a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2]), a[3].max(p[0]), a[4].max(p[1]), a[5].max(p[2])])
        })
    };
    let ow = span(&mut parts.iter().flat_map(|p| p.blocks.keys().copied())).ok_or_else(|| anyhow::anyhow!("no overworld cells"))?;
    let ns = span(&mut parts.iter().flat_map(|p| p.nether_blocks.keys().copied()));
    srv.cmd_ok(&format!("forceload add {} {} {} {}", ow[0], ow[2], ow[3], ow[5]))?;
    if let Some(n) = ns {
        srv.cmd_ok(&nc(format!("forceload add {} {} {} {}", n[0], n[2], n[3], n[5])))?;
    }
    srv.step(20)?;
    // the Nether's own terrain cleared around the parts (and the player's spot) before they are laid
    if let Some(n) = ns {
        let lo = [n[0].min(nether_afk[0].floor() as i32) - 3, n[1].min(nether_afk[1].floor() as i32) - 3, n[2].min(nether_afk[2].floor() as i32) - 3];
        let hi = [n[3].max(nether_afk[0].floor() as i32) + 3, (n[4].max(nether_afk[1].floor() as i32) + 4).min(126), n[5].max(nether_afk[2].floor() as i32) + 3];
        srv.cmd_ok(&nc(format!("fill {} {} {} {} {} {} minecraft:air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2])))?;
    }
    let (mut ncells, mut ocells, mut nent) = (0usize, 0usize, 0usize);
    let mut differ: Vec<String> = Vec::new();
    for part in parts {
        for (blocks, in_nether) in [(&part.blocks, false), (&part.nether_blocks, true)] {
            for pass in 0..3 {
                for (p, s) in blocks {
                    let k = if s.contains("obsidian") { 0 } else if s.contains("nether_portal") { 1 } else { 2 };
                    if k != pass {
                        continue;
                    }
                    let mode = if s.contains("rail") { " strict" } else { "" };
                    let c = format!("setblock {} {} {} {s}{mode}", p[0], p[1], p[2]);
                    srv.cmd_ok(&if in_nether { nc(c) } else { c })?;
                }
            }
        }
        for (p, nbt) in &part.block_entities {
            srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", p[0], p[1], p[2]))?;
        }
        for (p, nbt) in &part.nether_block_entities {
            srv.cmd_ok(&nc(format!("data merge block {} {} {} {nbt}", p[0], p[1], p[2])))?;
        }
        for e in &part.entities {
            srv.cmd_ok(&format!("summon {} {} {} {} {}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.nbt.clone().unwrap_or_default()))?;
        }
        ocells += part.blocks.len();
        ncells += part.nether_blocks.len();
        nent += part.entities.len();
    }
    for part in parts {
        for (p, s) in &part.blocks {
            if !srv.cmd_long(&format!("execute if block {} {} {} {s}", p[0], p[1], p[2])).map(|o| o.contains("passed")).unwrap_or(false) {
                differ.push(format!("({},{},{}) {s}", p[0], p[1], p[2]));
            }
        }
        for (p, s) in &part.nether_blocks {
            if !srv.cmd_long(&nc(format!("execute if block {} {} {} {s}", p[0], p[1], p[2]))).map(|o| o.contains("passed")).unwrap_or(false) {
                differ.push(format!("Nether ({},{},{}) {s}", p[0], p[1], p[2]));
            }
        }
    }
    // (ironspec 16:48 (2), G1 16:47) every overworld cell whose state differs after placement, the file's state
    // against the game's, classified: SHAPE (a shape, facing, axis, face, half or connection property: a design
    // finding; a tripwire hook going attached=true -> false too) or STATE (power, triggered and the like)
    let props = |s: &str| -> BTreeMap<String, String> {
        s.split_once('[').map(|(_, r)| r.trim_end_matches(']').split(',').filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect()).unwrap_or_default()
    };
    let mut shape_n = 0usize;
    for part in parts {
        for (p, s) in &part.blocks {
            if !differ.iter().any(|d| d.starts_with(&format!("({},{},{}) ", p[0], p[1], p[2]))) {
                continue;
            }
            let snap = srv.snapshot(*p, *p)?;
            let game = snap.region.get(*p).unwrap_or("?").to_string();
            let (f, g) = (props(s), props(&game));
            let changed: Vec<String> = f.iter().filter(|(k, v)| g.get(*k) != Some(*v)).map(|(k, v)| format!("{k} {v} -> {}", g.get(k).map(|x| x.as_str()).unwrap_or("?"))).collect();
            let same_block = s.split('[').next() == game.split('[').next();
            let shape = !same_block
                || f.iter().any(|(k, v)| g.get(k) != Some(v) && matches!(k.as_str(), "shape" | "facing" | "axis" | "face" | "half" | "north" | "south" | "east" | "west" | "up" | "down" | "attachment" | "hinge" | "type"))
                || (s.contains("tripwire") && f.get("attached").map(|v| v.as_str()) == Some("true") && g.get("attached").map(|v| v.as_str()) == Some("false"));
            if shape {
                shape_n += 1;
            }
            println!("    differs ({},{},{}) {}: {}{}", p[0], p[1], p[2], if shape { "SHAPE" } else { "STATE" }, if same_block { changed.join(", ") } else { format!("{s} -> {game}") }, if same_block { format!(" (file {s})") } else { String::new() });
        }
    }
    println!("  update-stability: {} cell(s) differ after placement, {shape_n} of the SHAPE class (a design finding), {} of the STATE class", differ.len(), differ.len().saturating_sub(shape_n));
    println!("  built: {ocells} overworld cells, {ncells} Nether cells, {nent} entities; {} cells differ after the build{}", differ.len(), if differ.is_empty() { String::new() } else { format!(": {}", differ.iter().take(8).cloned().collect::<Vec<_>>().join("; ")) });
    srv.step(400)?;
    srv.cmd_ok(&format!("forceload remove {} {} {} {}", ow[0], ow[2], ow[3], ow[5]))?;
    if let Some(n) = ns {
        srv.cmd_ok(&nc(format!("forceload remove {} {} {} {}", n[0], n[2], n[3], n[5])))?;
    }
    // (G1 on X #2) per side over a phase's ticks (p0, p1]: the ticks with a printed ticket line, the ticks
    // whose line holds the PORTAL ticket with ticks left > 0, and its least ticks left
    let portal_bar = |log: &str, p0: i64, p1: i64| -> Vec<(&'static str, (i32, i32), usize, usize, Option<i64>)> {
        [("overworld", portal), ("nether", nportal)]
            .into_iter()
            .map(|(dim, c)| {
                let lines: std::collections::BTreeSet<i64> = ticket_lines(log, dim).into_iter().map(|(gt, _)| gt).filter(|gt| *gt > p0 && *gt <= p1).collect();
                let v: Vec<(i64, i64)> = portal_ticks_left_at(log, dim, c).into_iter().filter(|(gt, _)| *gt > p0 && *gt <= p1).collect();
                let with: std::collections::BTreeSet<i64> = v.iter().filter(|x| x.1 > 0).map(|x| x.0).collect();
                (dim, c, lines.len(), with.len(), v.iter().map(|x| x.1).min())
            })
            .collect()
    };
    let nreport = |log: &str, from: i64, to: i64| -> Vec<((i32, i32), i32)> {
        let tk = ticket_lines(log, "nether");
        nchunks
            .iter()
            .map(|&c| {
                let worst = tk.iter().filter(|(gt, _)| *gt >= from && *gt <= to).map(|(_, m)| best_level(m, c).map(|b| b.0).unwrap_or(99)).max().unwrap_or(99);
                (c, worst)
            })
            .collect()
    };
    // (L1) at the farm, (L2) in the Nether
    let pad = |srv: &mut VanillaServer, p: [f64; 3], in_nether: bool| -> Result<()> {
        let c = format!("setblock {} {} {} minecraft:glass", p[0].floor() as i32, p[1].floor() as i32 - 1, p[2].floor() as i32);
        srv.cmd_ok(&if in_nether { nc(c) } else { c }).map(|_| ())
    };
    pad(&mut srv, farm_afk, false)?;
    srv.teleport_bot(farm_afk[0], farm_afk[1], farm_afk[2])?;
    let mut bad: Vec<String> = Vec::new();
    // (G1 on X #1 / #2) every phase judged over its ticks (p0, p1], from the tick after its move: every
    // route chunk entity-ticking at every printed tick with a load line at every tick, frozen 0; every
    // Nether chunk at level <= 31; each side's PORTAL ticket on every tick's ticket line with ticks left
    // > 0, a ticket line at every tick; returned: the control chunk seen leaving entity-ticking
    let judge = |log: &str, p0: i64, p1: i64, label: &str, bad: &mut Vec<String>| -> bool {
        let want = (p1 - p0).max(0) as usize;
        let st = report(log, chunks, p0 + 1, label);
        for (c, et, frozen, n) in &st {
            if *n < want || (*et as usize) < *n || *frozen > 0 {
                bad.push(format!("{label}: chunk ({},{}) entity-ticking {et} of {n} printed ticks (of {want}), frozen {frozen}", c.0, c.1));
            }
        }
        let nr = nreport(log, p0 + 1, p1);
        for (c, lv) in &nr {
            if *lv > 31 {
                bad.push(format!("{label}: Nether chunk ({},{}) reached level {lv}", c.0, c.1));
            }
        }
        let mut sides = Vec::new();
        for (dim, c, lines, with, least) in portal_bar(log, p0, p1) {
            if lines < want || with < want || least.is_none_or(|m| m <= 0) {
                bad.push(format!("{label}: the {dim} PORTAL ticket at {c:?} on {with} of {lines} ticket lines ({lines} of {want} ticks printed), least ticks left {least:?}"));
            }
            sides.push(format!("{dim} PORTAL at {c:?} on {with} of {lines} ticket lines ({lines} of {want} ticks printed), least ticks left {}", least.map(|m| m.to_string()).unwrap_or_else(|| "- (no ticket)".into())));
        }
        println!("    {label}: {}; Nether chunks' worst levels {nr:?}", sides.join("; "));
        // (ironspec 16:48 (1)) each arrival's portal: a PORTAL ticket appearing or its ticks left rising
        // on a chunk (a renewal) is an arrival there; any chunk but the loader's / the twin's is a capture
        for dim in ["overworld", "nether"] {
            let want_c = if dim == "overworld" { portal } else { nportal };
            let mut prev: BTreeMap<(i32, i32), i64> = BTreeMap::new();
            let mut arrivals: BTreeMap<(i32, i32), Vec<i64>> = BTreeMap::new();
            for (gt, m) in portal_left_by_line(log, dim) {
                if gt <= p0 || gt > p1 {
                    continue;
                }
                for (c, left) in m {
                    if prev.get(&c).is_none_or(|p| left > *p) {
                        arrivals.entry(c).or_default().push(gt);
                    }
                    prev.insert(c, left);
                }
            }
            let caps: Vec<String> = arrivals.iter().filter(|(c, _)| **c != want_c).map(|(c, v)| format!("({},{}) {} arrival(s) from gt {}", c.0, c.1, v.len(), v[0])).collect();
            println!(
                "    {label}: {dim} arrivals by portal chunk: {}{}",
                arrivals.iter().map(|(c, v)| format!("({},{}) {}", c.0, c.1, v.len())).collect::<Vec<_>>().join(", "),
                if caps.is_empty() { String::new() } else { format!("; CAPTURED away from {want_c:?}: {}", caps.join("; ")) }
            );
            for c in caps {
                bad.push(format!("{label}: {dim} arrivals captured at {c}"));
            }
        }
        let ctl_st = report(log, &[ctl], p0 + 1, &format!("{label}: the control chunk {ctl:?}"));
        ctl_st.iter().all(|(_, et, _, n)| *n > 0 && (*et as usize) < *n)
    };
    // (L1) the player at the farm
    let l10 = srv.game_time()?;
    srv.step(600)?;
    let log = std::fs::read_to_string(&srv.log)?;
    judge(&log, l10, srv.game_time()?, "(L1) 600 gt with the player standing at the farm", &mut bad);
    // (L2) the player in the Nether at the twin
    pad(&mut srv, nether_afk, true)?;
    srv.cmd_ok(&format!("execute in minecraft:the_nether run tp {bot} {} {} {}", nether_afk[0], nether_afk[1], nether_afk[2]))?;
    let l20 = srv.game_time()?;
    srv.step(600)?;
    let log = std::fs::read_to_string(&srv.log)?;
    judge(&log, l20, srv.game_time()?, "(L2) 600 gt with the player in the Nether at the twin", &mut bad);
    // the logout, (a)
    srv.bot = None;
    let a0 = srv.game_time()?;
    srv.step(20)?;
    println!("  the player logged out: `list` says {}", srv.cmd("list")?.trim());
    srv.step(restart_at - 20)?;
    let log = std::fs::read_to_string(&srv.log)?;
    let a1 = srv.game_time()?;
    let caught_a = judge(&log, a0, a1, &format!("(a) {restart_at} gt logged out"), &mut bad);
    // a save, stop and start (the player stays logged out), (b)
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let old = srv.dir.clone();
    let wpath = old.join(&srv.level_name);
    drop(srv);
    let mut o = opts(Some(wpath));
    o.with_bot = false;
    let mut srv = VanillaServer::start(&o)?;
    let _ = std::fs::remove_dir_all(&old);
    let b0 = srv.game_time()?;
    srv.step(ticks - restart_at)?;
    let log = std::fs::read_to_string(&srv.log)?;
    let b1 = srv.game_time()?;
    let caught_b = judge(&log, b0, b1, &format!("(b) after the restart, {} gt logged out", ticks - restart_at), &mut bad);
    // (c) the player logs in again
    srv.bot = Some(mctas::vanilla::bot::Bot::connect(srv.game_port(), &bot)?);
    let c0 = srv.game_time()?;
    srv.step(600)?;
    let log = std::fs::read_to_string(&srv.log)?;
    judge(&log, c0, srv.game_time()?, "(c) 600 gt after the player logged in again", &mut bad);
    println!("    (c): the player at {}", srv.cmd_long(&format!("data get entity {bot} Pos"))?.split("entity data: ").nth(1).unwrap_or("-").trim());
    srv.bot = None;
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!(
        "  the loader bars (every phase, L1, L2, (a), (b), (c): every route chunk entity-ticking at every tick, frozen 0; every Nether chunk at level <= 31; both PORTAL tickets on every tick's line, ticks left > 0): {}; the control {ctl:?} caught in (a) {caught_a}, (b) {caught_b}",
        if bad.is_empty() { "HELD".to_string() } else { format!("FAILED: {}", bad.join("; ")) }
    );
    anyhow::ensure!(caught_a && caught_b, "the control chunk {ctl:?} was not seen leaving entity-ticking: the count can't see a drop: the run fails");
    anyhow::ensure!(bad.is_empty(), "the loader bars failed: {}", bad.join("; "));
    Ok(())
}
