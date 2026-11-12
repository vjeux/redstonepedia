//! (tickexact 2026-11-11, ironspec's chunk-loader study) `mctas mobai load-window`: on the
//! official 26.3 server with the load-window agent (`bridge/agent/.../LoadAgent.java`), what a
//! world load does to a forced farm chunk in its first ticks: from the server's first tick, per tick
//! and per farm chunk, whether its scheduled block ticks run, whether its block events and block
//! entities run, whether its entities are loaded and tick, and how many scheduled ticks and entity
//! ticks it ran. The window is the ticks where the chunk's redstone runs while its entities are
//! frozen. Cases: `quiet` (the two farm chunks forced, a void world), `load` (a real-terrain world
//! with a 25 x 25 forced square beside the farm loading at the same start, as a player's own
//! view-distance load competes with a pearl's login).

use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::PathBuf;

/// The test farm's chunks [x0, z0, x1, z1]: the farm (2, 2) and the sentinel's room beside it (3, 2).
const FARM: [i32; 4] = [2, 2, 3, 2];
/// Ticks the agent prints from the server's first tick.
const TICKS: u32 = 300;

/// One run's line per tick: (tick, game time, per chunk (cx, cz, st, bt, el, et, sched, ents)).
type Line = (u32, i64, Vec<(i32, i32, bool, bool, bool, bool, u32, u32)>);

fn parse(log: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for l in log.lines() {
        let Some(rest) = l.split("mctas-load tick ").nth(1) else { continue };
        let mut parts = rest.split(" | ");
        let head: Vec<&str> = parts.next().unwrap_or("").split_whitespace().collect();
        let (Some(tick), Some(gt)) = (head.first().and_then(|v| v.parse().ok()), head.get(2).and_then(|v| v.parse().ok())) else { continue };
        let mut chunks = Vec::new();
        for p in parts {
            let w: Vec<&str> = p.split_whitespace().collect();
            let Some((cx, cz)) = w.first().and_then(|c| c.split_once(',')) else { continue };
            let val = |k: &str| w.iter().position(|x| *x == k).and_then(|i| w.get(i + 1)).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            chunks.push((cx.parse().unwrap_or(0), cz.parse().unwrap_or(0), val("st") == 1, val("bt") == 1, val("el") == 1, val("et") == 1, val("sched"), val("ents")));
        }
        out.push((tick, gt, chunks));
    }
    out
}

/// The test farm, chunk by chunk: a stone floor, an observer clock (two observers facing each
/// other: a scheduled tick every 2 gt) and a few entities (a minecart on a rail, a villager in a
/// boat, an armor stand).
fn build(srv: &mut VanillaServer) -> Result<()> {
    for cx in FARM[0]..=FARM[2] {
        for cz in FARM[1]..=FARM[3] {
            let (x, z) = (cx * 16, cz * 16);
            srv.cmd_ok(&format!("fill {x} 63 {z} {} 63 {} minecraft:stone", x + 15, z + 15))?;
            srv.cmd_ok(&format!("setblock {} 64 {} minecraft:observer[facing=east]", x + 2, z + 2))?;
            srv.cmd_ok(&format!("setblock {} 64 {} minecraft:observer[facing=west]", x + 3, z + 2))?;
            srv.cmd_ok(&format!("setblock {} 64 {} minecraft:rail", x + 8, z + 8))?;
            srv.cmd_ok(&format!("summon minecraft:minecart {}.5 64.0625 {}.5", x + 8, z + 8))?;
            srv.cmd_ok(&format!("summon minecraft:oak_boat {}.5 64 {}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b}}]}}", x + 11, z + 11))?;
            srv.cmd_ok(&format!("summon minecraft:armor_stand {}.5 64 {}.5", x + 5, z + 12))?;
        }
    }
    Ok(())
}

pub fn load_window(version: &str, case: &str, runs: u32) -> Result<()> {
    let busy = match case {
        "quiet" => false,
        "load" => true,
        "pearl" => return pearl_window(version, runs, false),
        "pearl-busy" => return pearl_window(version, runs, true),
        "portal-loop" => return portal_loop(version, runs),
        "item-loop" => return item_loop(version, runs),
        "item-loop-build" => return item_loop_build(version, runs, false),
        "item-loop-build-slab" => return item_loop_build(version, runs, true),
        _ => bail!("--case quiet, load, pearl or pearl-busy"),
    };
    let jar = mctas::bridge::load_agent(version)?;
    println!("== load-window {case} on the official {version} server: farm chunks {},{} to {},{} forced{}", FARM[0], FARM[1], FARM[2], FARM[3], if busy { ", a 25 x 25 forced square of real terrain loading at the same start" } else { "" });
    // the world, built once and saved
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        default_world: busy,
        frozen: false,
        ..Default::default()
    })?;
    srv.cmd_ok(&format!("forceload add {} {} {} {}", FARM[0] * 16, FARM[1] * 16, FARM[2] * 16 + 15, FARM[3] * 16 + 15))?;
    build(&mut srv)?;
    if busy {
        // (forceload takes at most 256 chunks a command: rows of 25 x 10, 10, 5)
        let (bx, bz) = (40, -12);
        for (z0, n) in [(0, 10), (10, 10), (20, 5)] {
            srv.cmd_ok(&format!("forceload add {} {} {} {}", bx * 16, (bz + z0) * 16, (bx + 24) * 16 + 15, (bz + z0 + n) * 16 - 1))?;
        }
        // (generated before the save: the start then loads them from disk)
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
    srv.cmd_ok("save-all flush")?;
    let world = srv.dir.join(&srv.level_name);
    let keep = srv.dir.clone();
    drop(srv);
    let mut worst = (0u32, 0u32);
    for run in 0..runs {
        let agent = PathBuf::from(format!("{}=watch={},{},{},{};ticks={TICKS}", jar.display(), FARM[0], FARM[1], FARM[2], FARM[3]));
        let srv = VanillaServer::start(&VanillaOptions {
            version: version.to_string(),
            difficulty: "normal".into(),
            with_bot: false,
            keep_dir: false,
            biome: "minecraft:plains".into(),
            world: Some(world.clone()),
            frozen: false,
            java_agent: Some(agent),
            ..Default::default()
        })?;
        let t0 = std::time::Instant::now();
        let mut lines = Vec::new();
        while t0.elapsed() < std::time::Duration::from_secs(90) {
            let log = std::fs::read_to_string(&srv.log).with_context(|| format!("read {}", srv.log.display()))?;
            lines = parse(&log);
            if lines.len() as u32 >= TICKS {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        if lines.is_empty() {
            let log = std::fs::read_to_string(&srv.log).unwrap_or_default();
            bail!("run {run}: no agent line in the log:\n{}", log.lines().filter(|l| l.contains("agent")).collect::<Vec<_>>().join("\n"));
        }
        println!("run {run}: {} ticks printed", lines.len());
        let n = lines[0].2.len();
        for k in 0..n {
            let (cx, cz) = (lines[0].2[k].0, lines[0].2[k].1);
            let first = |f: &dyn Fn(&(i32, i32, bool, bool, bool, bool, u32, u32)) -> bool| lines.iter().find(|l| l.2.get(k).is_some_and(f)).map(|l| l.0);
            let st = first(&|c| c.2);
            let bt = first(&|c| c.3);
            let el = first(&|c| c.4);
            let et = first(&|c| c.5);
            let sched1 = first(&|c| c.6 > 0);
            let ents1 = first(&|c| c.7 > 0);
            // the window: ticks with the chunk's scheduled ticks running and its entities frozen
            let window: Vec<&Line> = lines.iter().filter(|l| l.2.get(k).is_some_and(|c| c.2 && !c.5)).collect();
            let sched_frozen: u32 = lines.iter().filter_map(|l| l.2.get(k)).filter(|c| !c.5).map(|c| c.6).sum();
            let bt_frozen = lines.iter().filter(|l| l.2.get(k).is_some_and(|c| c.3 && !c.5)).count();
            let fmt = |v: Option<u32>| v.map(|t| format!("tick {t}")).unwrap_or_else(|| "never".into());
            println!(
                "  chunk {cx},{cz}: block events and block entities from {}, entities loaded from {}, scheduled ticks may run from {} (first ran {}), entities tick from {} (first ticked {}); window (scheduled ticks on, entities frozen) {} ticks, block events on with entities frozen {} ticks, scheduled ticks run while frozen {}",
                fmt(bt),
                fmt(el),
                fmt(st),
                fmt(sched1),
                fmt(et),
                fmt(ents1),
                window.len(),
                bt_frozen,
                sched_frozen
            );
            worst.0 = worst.0.max(window.len() as u32);
            worst.1 = worst.1.max(sched_frozen);
        }
        drop(srv);
    }
    println!("worst over {runs} run(s): window {} ticks, {} scheduled ticks run in a farm chunk while its entities were frozen", worst.0, worst.1);
    let _ = std::fs::remove_dir_all(&keep);
    Ok(())
}

/// One load of a farm chunk out of the agent's lines: from the first tick any of its flags is on
/// (after a tick with all off) until its entities tick.
#[derive(Debug, Default, Clone)]
struct Episode {
    cx: i32,
    cz: i32,
    start: u32,
    bt: Option<u32>,
    el: Option<u32>,
    st: Option<u32>,
    et: Option<u32>,
    /// Ticks with scheduled ticks or block events on and entities frozen.
    window: u32,
    /// Scheduled ticks the chunk ran with its entities frozen.
    sched_frozen: u32,
}

fn episodes(lines: &[Line]) -> Vec<Episode> {
    let mut out = Vec::new();
    let mut open: std::collections::BTreeMap<(i32, i32), Episode> = std::collections::BTreeMap::new();
    let mut was_off: std::collections::BTreeMap<(i32, i32), bool> = std::collections::BTreeMap::new();
    for (tick, _, chunks) in lines {
        for &(cx, cz, st, bt, el, et, sched, _) in chunks {
            let any = st || bt || el || et;
            let off = *was_off.get(&(cx, cz)).unwrap_or(&true);
            if any && off && !open.contains_key(&(cx, cz)) {
                open.insert((cx, cz), Episode { cx, cz, start: *tick, ..Default::default() });
            }
            if let Some(e) = open.get_mut(&(cx, cz)) {
                let rel = tick - e.start;
                if bt && e.bt.is_none() {
                    e.bt = Some(rel);
                }
                if el && e.el.is_none() {
                    e.el = Some(rel);
                }
                if st && e.st.is_none() {
                    e.st = Some(rel);
                }
                if !et {
                    if st || bt {
                        e.window += 1;
                    }
                    e.sched_frozen += sched;
                } else {
                    e.et = Some(rel);
                    out.push(open.remove(&(cx, cz)).unwrap());
                }
            }
            was_off.insert((cx, cz), !any);
        }
    }
    out.extend(open.into_values());
    out
}

/// (tickexact 2026-11-12, ironspec's loader study) The pearl loader at login: the bot owns one
/// stand-in ender pearl per farm chunk (NoGravity, at the chunk's centre: the same ENDER_PEARL
/// ticket as a pearl in stasis), stands far off (4,000 blocks), and logs out and in `runs` times;
/// each login respawns its pearls and their tickets (`ServerPlayer.loadAndSpawnEnderPearls`) and the
/// agent prints how each farm chunk comes back. `busy`: a real-terrain world with view and
/// simulation distance 12, so the bot's own login loads its 25 x 25 square at the same time.
fn pearl_window(version: &str, runs: u32, busy: bool) -> Result<()> {
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch={},{},{},{};ticks=0", jar.display(), FARM[0], FARM[1], FARM[2], FARM[3]));
    let dist = if busy { 12 } else { 3 };
    println!(
        "== load-window pearl{} on the official {version} server: farm chunks {},{} to {},{} held only by the bot's pearls, the bot 4,000 blocks off (view and simulation distance {dist}), {runs} logouts and logins",
        if busy { "-busy" } else { "" },
        FARM[0],
        FARM[1],
        FARM[2],
        FARM[3]
    );
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: true,
        keep_dir: false,
        biome: "minecraft:plains".into(),
        default_world: busy,
        frozen: false,
        view_distance: dist,
        simulation_distance: dist,
        java_agent: Some(agent),
        ..Default::default()
    })?;
    let name = "mctas_bot".to_string();
    // the bot far off, on a platform (a creative player still dies in the void, and its death
    // would discard its pearls)
    srv.cmd_ok("forceload add 4000 4000 4000 4000")?;
    srv.cmd_ok("fill 3996 99 3996 4004 99 4004 minecraft:stone")?;
    srv.teleport_bot(4000.5, 100.0, 4000.5)?;
    srv.cmd_ok("forceload remove 4000 4000 4000 4000")?;
    let uuid = {
        let out = srv.cmd_ok(&format!("data get entity {name} UUID"))?;
        let ints: Vec<i64> = out.split(|c: char| !(c.is_ascii_digit() || c == '-')).filter_map(|s| s.parse().ok()).collect();
        let n = ints.len();
        if n < 4 {
            bail!("the bot's UUID not in `{out}`");
        }
        ints[n - 4..].iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
    };
    let (x0, z0, x1, z1) = (FARM[0] * 16, FARM[1] * 16, FARM[2] * 16 + 15, FARM[3] * 16 + 15);
    srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
    build(&mut srv)?;
    for cx in FARM[0]..=FARM[2] {
        for cz in FARM[1]..=FARM[3] {
            srv.cmd_ok(&format!("summon minecraft:ender_pearl {}.5 70 {}.5 {{NoGravity:1b,Motion:[0.0,0.0,0.0],Owner:[I;{uuid}]}}", cx * 16 + 8, cz * 16 + 8))?;
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    srv.cmd_ok(&format!("forceload remove {x0} {z0} {x1} {z1}"))?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    let pearls = srv.count_entities("@e[type=minecraft:ender_pearl]")?;
    println!("pearls in the world after the forced chunks' release: {pearls}");
    let mut logins = Vec::new();
    for run in 0..runs {
        let before = std::fs::read_to_string(&srv.log)?.lines().count();
        srv.bot = None;
        // a full unload first: the old pearls' tickets (40 gt) expire and both farm chunks show
        // every flag off, or 120 s
        let t0 = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let log = std::fs::read_to_string(&srv.log)?;
            let lines = parse(&log.lines().skip(before).collect::<Vec<_>>().join("\n"));
            let off = lines.last().is_some_and(|l| l.2.iter().all(|c| !(c.2 || c.3 || c.4 || c.5)));
            if off {
                break;
            }
            if t0.elapsed() > std::time::Duration::from_secs(120) {
                println!("login {run}: the farm chunks didn't fully unload in 120 s after the logout");
                break;
            }
        }
        let before = std::fs::read_to_string(&srv.log)?.lines().count();
        srv.bot = Some(mctas::vanilla::bot::Bot::connect(srv.game_port(), &name).with_context(|| format!("login {run}"))?);
        std::thread::sleep(std::time::Duration::from_secs(if busy { 15 } else { 4 }));
        let log = std::fs::read_to_string(&srv.log)?;
        let lines = parse(&log.lines().skip(before).collect::<Vec<_>>().join("\n"));
        logins.push(lines);
    }
    let mut worst = (0u32, 0u32, 0u32);
    for (run, lines) in logins.iter().enumerate() {
        let eps = episodes(lines);
        let fmt = |v: Option<u32>| v.map(|t| format!("+{t}")).unwrap_or_else(|| "never".into());
        println!("login {run}: {} chunk loads", eps.len());
        for e in &eps {
            println!(
                "  chunk {},{} (from tick {}): block events and block entities {}, entities loaded {}, scheduled ticks {}, entities tick {}; window {} ticks, {} scheduled ticks run while its entities were frozen",
                e.cx,
                e.cz,
                e.start,
                fmt(e.bt),
                fmt(e.el),
                fmt(e.st),
                fmt(e.et),
                e.window,
                e.sched_frozen
            );
            worst.0 = worst.0.max(e.window);
            worst.1 = worst.1.max(e.sched_frozen);
            worst.2 = worst.2.max(e.et.unwrap_or(u32::MAX).min(100_000));
        }
    }
    println!("worst over {runs} login(s): window {} ticks, {} scheduled ticks run in a farm chunk while its entities were frozen, entities ticking {} ticks after the chunk's first flag", worst.0, worst.1, worst.2);
    Ok(())
}

/// Minecarts in `dim` (overworld or the_nether).
fn carts_in(srv: &mut VanillaServer, dim: &str) -> Result<u32> {
    let out = srv.cmd_long(&format!("execute in minecraft:{dim} if entity @e[type=minecraft:minecart,distance=0..]"))?;
    Ok(out.split("count: ").nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0))
}

/// Steps `n` ticks in steps of 5, recording each tick-5 sample of the overworld's cart count;
/// returns the longest gap between two samples where the count rose (an arrival from the Nether).
fn watch_loop(srv: &mut VanillaServer, n: u32, last_arrival: &mut Option<i64>, worst_gap: &mut i64) -> Result<()> {
    let mut before = carts_in(srv, "overworld")?;
    let mut done = 0;
    while done < n {
        srv.step(5)?;
        done += 5;
        let now_c = carts_in(srv, "overworld")?;
        if now_c > before {
            let t = srv.game_time()?;
            if let Some(l) = *last_arrival {
                *worst_gap = (*worst_gap).max(t - l);
            }
            *last_arrival = Some(t);
        }
        before = now_c;
    }
    Ok(())
}

/// (tickexact 2026-11-12, ironspec's portal-loop loader) An overworld portal (frame x 39..42,
/// z 40, portal blocks x 40..41, y 64..66) linked to the Nether portal the first crossing makes,
/// with 3 empty minecarts placed in it 100 gt apart: each bounces every 300 gt (its portal
/// cooldown), so each side gets an arrival about every 200 gt and keeps its PORTAL ticket (300 gt,
/// radius 3: the 3 x 3 chunks around the arrival entity-ticking). No other loader once built. The
/// agent watches the 3 x 3 chunks around the overworld portal's chunk (2, 2). Then `runs` real
/// saves, stops and starts at different phases of the loop: per restart, the agent's first ticks
/// (does prepareLevels wait for the persisted PORTAL tickets) and whether the loop keeps running
/// (the longest gap between overworld arrivals afterwards, against the ticket's 300 gt).
fn portal_loop(version: &str, runs: u32) -> Result<()> {
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=1,1,3,3;ticks=200", jar.display()));
    println!("== load-window portal-loop on the official {version} server: 3 empty carts bouncing through an overworld portal at chunk (2, 2), {runs} restarts");
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        world,
        java_agent: Some(agent.clone()),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add 16 16 63 63")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 31 31")?;
    srv.step(20)?;
    // both portals 5 wide (x 40..44 at z 40, y 64..66; the Nether's at x 5..9, z 5, y 70..72 on a
    // platform in a cleared box), so a cart keeps its column across: the carts sit in columns
    // 40, 42 and 44 (1.02 apart, beyond the carts' 0.2 push reach)
    srv.cmd_ok("fill 32 63 32 47 63 47 minecraft:stone")?;
    srv.cmd_ok("fill 39 63 40 45 63 40 minecraft:obsidian")?;
    srv.cmd_ok("fill 39 67 40 45 67 40 minecraft:obsidian")?;
    srv.cmd_ok("fill 39 64 40 39 66 40 minecraft:obsidian")?;
    srv.cmd_ok("fill 45 64 40 45 66 40 minecraft:obsidian")?;
    srv.cmd_ok("fill 40 64 40 44 66 40 minecraft:nether_portal[axis=x]")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 0 69 0 14 75 10 minecraft:air")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 0 68 0 14 68 10 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 4 69 5 10 69 5 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 4 73 5 10 73 5 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 4 70 5 4 72 5 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 10 70 5 10 72 5 minecraft:obsidian")?;
    srv.cmd_ok("execute in minecraft:the_nether run fill 5 70 5 9 72 5 minecraft:nether_portal[axis=x]")?;
    for (k, x) in [(0, 40), (1, 42), (2, 44)] {
        srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 64 40.5 {{Tags:[\"loop{k}\"]}}"))?;
        srv.step(100)?;
    }
    srv.step(200)?;
    let where_now = |srv: &mut VanillaServer| -> Result<String> {
        let mut s = String::new();
        for k in 0..3 {
            let p = srv.cmd_long(&format!("data get entity @e[tag=loop{k},limit=1] Pos"))?;
            let d = srv.cmd_long(&format!("execute in minecraft:the_nether if entity @e[tag=loop{k},distance=0..]"))?;
            s.push_str(&format!(" loop{k} {} {}", if d.contains("passed") { "nether" } else { "overworld?" }, p.split("data: ").nth(1).unwrap_or("(gone)").trim()));
        }
        Ok(s)
    };
    println!("after the build:{}", where_now(&mut srv)?);
    srv.cmd_ok("forceload remove 16 16 63 63")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 31 31")?;
    let mut last = None;
    let mut worst_gap = 0i64;
    watch_loop(&mut srv, 1200, &mut last, &mut worst_gap)?;
    println!(
        "built: carts overworld {} nether {}; the longest gap between overworld arrivals over 1,200 gt with no other loader: {worst_gap} gt",
        carts_in(&mut srv, "overworld")?,
        carts_in(&mut srv, "the_nether")?
    );
    for run in 0..runs {
        // a different phase of the loop each time
        srv.step(70 + 90 * run)?;
        srv.cmd_ok("save-on")?;
        srv.cmd_ok("save-all flush")?;
        let old = srv.dir.clone();
        let world = old.join(&srv.level_name);
        let gt = srv.game_time()?;
        drop(srv);
        srv = VanillaServer::start(&opts(Some(world)))?;
        let _ = std::fs::remove_dir_all(&old);
        let log = std::fs::read_to_string(&srv.log)?;
        let lines = parse(&log);
        let n = lines.len();
        let eps = episodes(&lines);
        let worst_window = eps.iter().map(|e| e.window).max().unwrap_or(0);
        let worst_frozen = eps.iter().map(|e| e.sched_frozen).max().unwrap_or(0);
        let ready0 = lines.first().map(|l| l.2.iter().filter(|c| c.2 && c.5).count()).unwrap_or(0);
        println!(
            "restart {run} (saved at game time {gt}): the agent's first tick has {ready0} of {} watched chunks with scheduled ticks and entities on; {n} lines; worst window {worst_window} ticks, {worst_frozen} scheduled ticks run with entities frozen",
            lines.first().map(|l| l.2.len()).unwrap_or(0)
        );
        let mut last = None;
        let mut worst_gap = 0i64;
        watch_loop(&mut srv, 1200, &mut last, &mut worst_gap)?;
        let log = std::fs::read_to_string(&srv.log)?;
        let lines = parse(&log);
        let drops = lines.iter().skip(1).filter(|l| l.2.iter().any(|c| !c.5)).count();
        println!(
            "  after it, 1,200 gt: carts overworld {} nether {}; the longest gap between overworld arrivals {worst_gap} gt; {drops} agent lines with a watched chunk not entity-ticking",
            carts_in(&mut srv, "overworld")?,
            carts_in(&mut srv, "the_nether")?
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// The item loop's side: its dimension, its containers (the firing dropper, the feeding droppers,
/// the hoppers), its clock's redstone-block cells (behind every dropper in the firing column) and
/// its phase.
struct LoopSide {
    dim: &'static str,
    containers: Vec<[i32; 3]>,
    clock: Vec<[i32; 3]>,
    phase: i64,
}

/// Items in a container (`data get block ... Items`): the sum of its stacks' counts.
fn items_in(srv: &mut VanillaServer, dim: &str, p: [i32; 3]) -> Result<u32> {
    let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get block {} {} {} Items", p[0], p[1], p[2]))?;
    Ok(out.split("count: ").skip(1).filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok()).sum())
}

/// A side's items: in its containers, and item entities in its dimension.
fn side_items(srv: &mut VanillaServer, s: &LoopSide) -> Result<(u32, u32)> {
    let mut held = 0;
    for p in &s.containers {
        held += items_in(srv, s.dim, *p)?;
    }
    // the loop's cobblestone item entities in this dimension, by their stacks' counts (a stuck
    // pair merges into one entity; natural drops, a Nether vine, aren't the loop's)
    let out = srv.cmd_long(&format!(
        "execute in minecraft:{} as @e[type=minecraft:item,distance=0..,nbt={{Item:{{id:\"minecraft:cobblestone\"}}}}] run data get entity @s Item.count",
        s.dim
    ))?;
    let flying = out.split("entity data: ").skip(1).filter_map(|v| v.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok()).sum();
    Ok((held, flying))
}

/// Runs the item loop's clocks for `n` gt (each side's droppers powered for 2 gt every 200 gt at
/// its phase), sampling every 5 gt; returns the arrivals' game times per side (a side's total going
/// up) and the fewest and most items seen in all, and the clock pulses that failed (a side's
/// chunk not loaded).
fn run_item_loop(srv: &mut VanillaServer, sides: &[LoopSide], n: i64) -> Result<(Vec<Vec<i64>>, u32, u32, Vec<String>)> {
    let mut arrivals = vec![Vec::new(); sides.len()];
    let mut prev: Vec<u32> = Vec::new();
    for s in sides {
        let (h, f) = side_items(srv, s)?;
        prev.push(h + f);
    }
    let (mut lo, mut hi) = (u32::MAX, 0u32);
    let mut failed = Vec::new();
    let t0 = srv.game_time()?;
    while srv.game_time()? < t0 + n {
        let t = srv.game_time()?;
        for s in sides {
            let m = (t - s.phase).rem_euclid(200);
            if m == 0 || m == 2 {
                let block = if m == 0 { "minecraft:redstone_block" } else { "minecraft:air" };
                // removed top down, so each dropper is updated after its last source is gone (a
                // dropper also powered through the cell above it, quasi-connectivity, gets no
                // update when that source goes and would stay TRIGGERED, never firing again)
                let mut cells = s.clock.clone();
                cells.sort_by_key(|c| if m == 0 { c[1] } else { -c[1] });
                for c in &cells {
                    let out = srv.cmd(&format!("execute in minecraft:{} run setblock {} {} {} {block}", s.dim, c[0], c[1], c[2]))?;
                    if out.contains("not loaded") {
                        failed.push(format!("t{t} {} {:?}: {out}", s.dim, c));
                    }
                }
            }
        }
        srv.step(1)?;
        if (srv.game_time()? - t0) % 5 == 0 {
            let mut all = 0;
            for (k, s) in sides.iter().enumerate() {
                let (h, f) = side_items(srv, s)?;
                let tot = h + f;
                if tot > prev[k] {
                    arrivals[k].push(srv.game_time()?);
                }
                prev[k] = tot;
                all += tot;
            }
            lo = lo.min(all);
            hi = hi.max(all);
        }
    }
    Ok((arrivals, lo, hi, failed))
}

fn gaps(a: &[i64]) -> String {
    let g: Vec<i64> = a.windows(2).map(|w| w[1] - w[0]).collect();
    match (g.iter().min(), g.iter().max()) {
        (Some(lo), Some(hi)) => format!("{} arrivals, gaps {lo}..{hi} gt", a.len()),
        _ => format!("{} arrivals", a.len()),
    }
}

/// (tickexact 2026-11-12, ironspec: onechunk's L1''' item loop, explore/farmopt-onechunk2
/// 8cf95510c, on the official server) Two same-axis 5 x 3 Nether portals, the overworld's
/// interior x 15..19 y 65..67 z 33 and the Nether's x 0..4 y 65..67 z 4 (at the overworld's /8);
/// each side's firing dropper on the portal's bottom row one off the plane, facing it (overworld
/// column 1 (16, 65, 34) facing north; Nether column 3 (3, 65, 3) facing south), fed by a dropper
/// under it facing up, a 2 x 3 hopper floor into the feeder on the arrivals' side, a glass
/// backstop 3 off the plane; 3 cobblestone per side. Each side's clock is emulated (as in
/// onechunk's bench): redstone blocks beside both droppers for 2 gt every 200 gt, the Nether's half
/// a period after the overworld's. No player online; no forced chunks once built. The agent
/// watches the overworld portal's chunks (0..1, 2). Then `runs` real saves, stops and starts at
/// different phases of the clocks: per restart, the overworld chunks at the first tick, whether
/// the Nether side is loaded right after the start, and the loop's arrivals and items after it.
fn item_loop(version: &str, runs: u32) -> Result<()> {
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=0,0,1,2;ticks=40;tickets=1;running=1", jar.display()));
    println!("== load-window item-loop on the official {version} server: onechunk's L1''' item loop (two 5 x 3 same-axis portals, dropper-fed, hopper floors, 6 items), no player online, {runs} restarts");
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        world,
        java_agent: Some(agent.clone()),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add 0 16 47 63")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 15 15")?;
    srv.step(20)?;
    // the overworld side
    srv.cmd_ok("fill 14 64 33 20 64 33 minecraft:obsidian")?;
    srv.cmd_ok("fill 14 68 33 20 68 33 minecraft:obsidian")?;
    srv.cmd_ok("fill 14 65 33 14 67 33 minecraft:obsidian")?;
    srv.cmd_ok("fill 20 65 33 20 67 33 minecraft:obsidian")?;
    srv.cmd_ok("fill 15 65 33 19 67 33 minecraft:nether_portal[axis=x]")?;
    srv.cmd_ok("setblock 16 64 34 minecraft:dropper[facing=up]{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:2}]}")?;
    srv.cmd_ok("setblock 16 65 34 minecraft:dropper[facing=north]{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}")?;
    srv.cmd_ok("fill 17 64 34 19 64 34 minecraft:hopper[facing=west]")?;
    srv.cmd_ok("fill 17 64 35 19 64 35 minecraft:hopper[facing=north]")?;
    srv.cmd_ok("fill 17 65 36 20 67 36 minecraft:glass")?;
    // the Nether side, in a cleared box
    let n = |c: &str| format!("execute in minecraft:the_nether run {c}");
    srv.cmd_ok(&n("fill -4 60 -2 8 72 8 minecraft:air"))?;
    srv.cmd_ok(&n("fill -1 64 4 5 64 4 minecraft:obsidian"))?;
    srv.cmd_ok(&n("fill -1 68 4 5 68 4 minecraft:obsidian"))?;
    srv.cmd_ok(&n("fill -1 65 4 -1 67 4 minecraft:obsidian"))?;
    srv.cmd_ok(&n("fill 5 65 4 5 67 4 minecraft:obsidian"))?;
    srv.cmd_ok(&n("fill 0 65 4 4 67 4 minecraft:nether_portal[axis=x]"))?;
    srv.cmd_ok(&n("setblock 3 64 3 minecraft:dropper[facing=up]{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:2}]}"))?;
    srv.cmd_ok(&n("setblock 3 65 3 minecraft:dropper[facing=south]{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}"))?;
    srv.cmd_ok(&n("fill 0 64 3 2 64 3 minecraft:hopper[facing=east]"))?;
    srv.cmd_ok(&n("fill 0 64 2 2 64 2 minecraft:hopper[facing=south]"))?;
    srv.cmd_ok(&n("fill -1 65 1 2 67 1 minecraft:glass"))?;
    let over = LoopSide {
        dim: "overworld",
        containers: vec![[16, 64, 34], [16, 65, 34], [17, 64, 34], [18, 64, 34], [19, 64, 34], [17, 64, 35], [18, 64, 35], [19, 64, 35]],
        clock: vec![[15, 65, 34], [15, 64, 34]],
        phase: 0,
    };
    let neth = LoopSide {
        dim: "the_nether",
        containers: vec![[3, 64, 3], [3, 65, 3], [0, 64, 3], [1, 64, 3], [2, 64, 3], [0, 64, 2], [1, 64, 2], [2, 64, 2]],
        clock: vec![[4, 65, 3], [4, 64, 3]],
        phase: 100,
    };
    let sides = [over, neth];
    // two cycles with the forced chunks, then without them
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 400)?;
    println!("built (forced): overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    srv.cmd_ok("forceload remove 0 16 47 63")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 15 15")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 1600)?;
    println!("no forced chunks, 1,600 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    for f in failed.iter().take(3) {
        println!("  {f}");
    }
    let srv = loop_restarts(srv, &opts, &sides, runs, [2, 65, 4], [17, 65, 33])?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// The item loop's `runs` real saves, stops and starts, each at a different phase of the clocks
/// (the clocks running up to the save): per restart, each side's tickets at the save and at the
/// first tick, the agent's first-tick chunk flags, the first scheduled tick and the first entity
/// tick in each watched overworld chunk (G1: "no load window" measured per restart), whether
/// each side (its probe cell) is loaded right after the start, and the loop's arrivals, items and
/// failed pulses over the 1,200 gt after it.
fn loop_restarts(mut srv: VanillaServer, opts: &dyn Fn(Option<PathBuf>) -> VanillaOptions, sides: &[LoopSide], runs: u32, nether_probe: [i32; 3], over_probe: [i32; 3]) -> Result<VanillaServer> {
    for run in 0..runs {
        // a different phase of the clocks each time (the clocks keep running up to the save)
        let _ = run_item_loop(&mut srv, sides, 37 + 76 * run as i64)?;
        srv.cmd_ok("save-on")?;
        srv.cmd_ok("save-all flush")?;
        let old = srv.dir.clone();
        let world = old.join(&srv.level_name);
        let gt = srv.game_time()?;
        let tickets_of = |log: &str, dim: &str, last: bool| -> String {
            let mut it = log.lines().filter(|l| l.contains("mctas-tickets") && l.contains(dim));
            let l = if last { it.last() } else { it.next() };
            l.map(|l| l.split("mctas-tickets ").nth(1).unwrap_or(l).replace("ResourceKey[minecraft:dimension / ", "").replace("] gt", " gt").replace(" ticks left ( out of", "/")).unwrap_or("-".into())
        };
        let before = std::fs::read_to_string(&srv.log)?;
        println!("  tickets at the save: {} || {}", tickets_of(&before, "minecraft:overworld", true), tickets_of(&before, "minecraft:the_nether", true));
        drop(srv);
        srv = VanillaServer::start(&opts(Some(world)))?;
        let _ = std::fs::remove_dir_all(&old);
        let after = std::fs::read_to_string(&srv.log)?;
        println!("  tickets at the first tick: {} || {}", tickets_of(&after, "minecraft:overworld", false), tickets_of(&after, "minecraft:the_nether", false));
        let p = nether_probe;
        let nloaded = srv.cmd(&format!("execute in minecraft:the_nether if loaded {} {} {}", p[0], p[1], p[2]))?;
        let p = over_probe;
        let oloaded = srv.cmd(&format!("execute in minecraft:overworld if loaded {} {} {}", p[0], p[1], p[2]))?;
        let lines = parse(&after);
        let first = lines.first().map(|l| l.2.iter().map(|c| format!("{},{} st{} bt{} el{} et{}", c.0, c.1, c.2 as u8, c.3 as u8, c.4 as u8, c.5 as u8)).collect::<Vec<_>>().join("; ")).unwrap_or("-".into());
        let gt2 = srv.game_time()?;
        println!(
            "restart {run} (saved at game time {gt}, the overworld clock at phase {}): the agent's first tick: {first}; right after the start (game time {gt2}): the Nether side {}, the overworld side {}",
            gt.rem_euclid(200),
            if nloaded.contains("passed") { "loaded" } else { "NOT loaded" },
            if oloaded.contains("passed") { "loaded" } else { "NOT loaded" }
        );
        // per watched chunk with any: its first scheduled tick and its first entity tick among
        // the ticks that ran (the agent's `running=1`: line 0 is the server's first tick, frozen
        // or not; every later line a tick that ran), read once the loop has run
        let (a, lo, hi, failed) = run_item_loop(&mut srv, sides, 1200)?;
        let lines = parse(&std::fs::read_to_string(&srv.log)?);
        let mut firsts = Vec::new();
        let ran: Vec<&Line> = lines.iter().skip(1).collect();
        if let (Some(l0), Some(r0)) = (lines.first(), ran.first()) {
            for (k, c) in l0.2.iter().enumerate() {
                let fs = ran.iter().find(|l| l.2.get(k).is_some_and(|c| c.6 > 0)).map(|l| l.1);
                let fe = ran.iter().find(|l| l.2.get(k).is_some_and(|c| c.7 > 0)).map(|l| l.1);
                if fs.is_some() || fe.is_some() {
                    firsts.push(format!("{},{}: scheduled from gt {}, entities from gt {}", c.0, c.1, fs.map(|t| t.to_string()).unwrap_or("-".into()), fe.map(|t| t.to_string()).unwrap_or("-".into())));
                }
            }
            println!("  the first tick that ran after the start: gt {}; {}", r0.1, if firsts.is_empty() { "no scheduled or entity tick in the printed ticks".to_string() } else { firsts.join("; ") });
        } else {
            println!("  no tick ran in the printed ticks");
        }
        println!("  after it, 1,200 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
        for f in failed.iter().take(3) {
            println!("    {f}");
        }
    }
    Ok(srv)
}

/// The build's loop variant (ironspec: from the brief's rules): its design file.
const BUILD_LOOP: &str = "designs/farms/iron/carousel-assembly/wip/tickexact/loader/l1-build-loop.txt";

/// (tickexact 2026-11-12, ironspec: the build's loop variant, G1 on ebb3ce8d7: the bridged
/// geometry wasn't the build's) `--case item-loop-build` (`-slab`: a bottom slab in each arrivals'
/// pocket, its gap 0.5): the L1''' loop as the brief builds it, from its design file (BUILD_LOOP,
/// pasted with its frame's 0 0 0 at 15 55 33, its Nether part at the anchor's /8, 1 55 4): each side
/// firing through its portal's top row, both faces of each portal closed, the arrivals coming out
/// of the twin's top row into a dead-end pocket over the collecting hopper. The clocks emulated
/// behind the droppers (2 gt every 200 gt, the Nether half a period later); an observer clock and
/// two entities in the overworld chunk (1, 2) for the first scheduled and entity ticks; no player
/// online, no forced chunks once built; then `runs` real restarts at different clock phases.
fn item_loop_build(version: &str, runs: u32, slab: bool) -> Result<()> {
    let d = mctas::design::Design::load(std::path::Path::new(BUILD_LOOP))?;
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=0,0,1,2;ticks=40;tickets=1;running=1", jar.display()));
    println!(
        "== load-window item-loop-build{} on the official {version} server: the build's L1''' loop ({BUILD_LOOP}), no player online, {runs} restarts",
        if slab { "-slab" } else { "" }
    );
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        world,
        java_agent: Some(agent.clone()),
        ..Default::default()
    };
    let t: [i32; 3] = [15, 55, 33];
    let n0 = [t[0].div_euclid(8), t[1], t[2].div_euclid(8)];
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add 0 0 31 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 31 31")?;
    srv.step(20)?;
    let n = |c: &str| format!("execute in minecraft:the_nether run {c}");
    srv.cmd_ok(&n(&format!("fill {} {} {} {} {} {} minecraft:air", n0[0] - 4, n0[1] + 6, n0[2] - 5, n0[0] + 8, n0[1] + 16, n0[2] + 4)))?;
    // the frames, then the portals, then the rest; then the containers' items
    let place = |srv: &mut VanillaServer, blocks: &std::collections::BTreeMap<[i32; 3], String>, at: [i32; 3], nether: bool| -> Result<()> {
        for pass in 0..3 {
            for (p, s) in blocks {
                let k = if s.contains("obsidian") { 0 } else if s.contains("nether_portal") { 1 } else { 2 };
                if k != pass {
                    continue;
                }
                let c = format!("setblock {} {} {} {s}", at[0] + p[0], at[1] + p[1], at[2] + p[2]);
                srv.cmd_ok(&if nether { n(&c) } else { c })?;
            }
        }
        Ok(())
    };
    place(&mut srv, &d.blocks, t, false)?;
    place(&mut srv, &d.nether_blocks, n0, true)?;
    for (p, nbt) in &d.block_entities {
        srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", t[0] + p[0], t[1] + p[1], t[2] + p[2]))?;
    }
    for (p, nbt) in &d.nether_block_entities {
        srv.cmd_ok(&n(&format!("data merge block {} {} {} {nbt}", n0[0] + p[0], n0[1] + p[1], n0[2] + p[2])))?;
    }
    // the slab variant: a bottom slab in each arrivals' pocket (overworld 3 12 1, Nether 1 12 -1)
    if slab {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:smooth_stone_slab[type=bottom]", t[0] + 3, t[1] + 12, t[2] + 1))?;
        srv.cmd_ok(&n(&format!("setblock {} {} {} minecraft:smooth_stone_slab[type=bottom]", n0[0] + 1, n0[1] + 12, n0[2] - 1)))?;
    }
    // the first-ticks markers in the overworld chunk (1, 2): an observer clock, an armor stand
    // and a minecart on a rail, on a stone floor
    srv.cmd_ok("fill 22 63 38 30 63 46 minecraft:stone")?;
    srv.cmd_ok("setblock 24 64 40 minecraft:observer[facing=east]")?;
    srv.cmd_ok("setblock 25 64 40 minecraft:observer[facing=west]")?;
    srv.cmd_ok("setblock 28 64 44 minecraft:rail")?;
    srv.cmd_ok("summon minecraft:minecart 28.5 64.0625 44.5")?;
    srv.cmd_ok("summon minecraft:armor_stand 26.5 64 42.5")?;
    // the sides: their containers (every dropper and hopper) and their clock cells (behind every
    // dropper in the firing dropper's column, against its facing: each powered directly, since a
    // dropper's quasi-connectivity needs a block update a neighbour dropper's TRIGGERED change
    // (flag 2) doesn't give)
    let side = |blocks: &std::collections::BTreeMap<[i32; 3], String>, at: [i32; 3], dim: &'static str, phase: i64| -> Result<LoopSide> {
        let containers: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.contains("dropper") || s.contains("hopper")).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2]]).collect();
        let (fp, fs) = blocks.iter().find(|(_, s)| s.contains("dropper") && !s.contains("facing=up")).ok_or_else(|| anyhow::anyhow!("{dim}: no firing dropper"))?;
        let dz = if fs.contains("facing=north") { 1 } else if fs.contains("facing=south") { -1 } else { bail!("{dim}: the firing dropper faces neither north nor south: {fs}") };
        let clock: Vec<[i32; 3]> = blocks.iter().filter(|(p, s)| s.contains("dropper") && p[0] == fp[0] && p[2] == fp[2]).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2] + dz]).collect();
        Ok(LoopSide { dim, containers, clock, phase })
    };
    let sides = [side(&d.blocks, t, "overworld", 0)?, side(&d.nether_blocks, n0, "the_nether", 100)?];
    where_items(&mut srv, &sides, "built")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 400)?;
    println!("built (forced): overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    where_items(&mut srv, &sides, "after 400 gt forced")?;
    srv.cmd_ok("forceload remove 0 0 31 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 31 31")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 1600)?;
    println!("no forced chunks, 1,600 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    for f in failed.iter().take(3) {
        println!("  {f}");
    }
    let probe_n = [n0[0] + 2, n0[1] + 11, n0[2]];
    let probe_o = [t[0] + 2, t[1] + 11, t[2]];
    let mut srv = loop_restarts(srv, &opts, &sides, runs, probe_n, probe_o)?;
    // where the loop's items are at the end: each side's containers, and every item entity
    where_items(&mut srv, &sides, "at the end")?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// Where the loop's items are: each side's containers (each one's count), and every item entity
/// in its dimension (its id, count and position).
fn where_items(srv: &mut VanillaServer, sides: &[LoopSide], when: &str) -> Result<()> {
    for s in sides {
        let mut held = Vec::new();
        for p in &s.containers {
            held.push(format!("{:?} {}", p, items_in(srv, s.dim, *p)?));
        }
        let pos = srv.cmd_long(&format!("execute in minecraft:{} as @e[type=minecraft:item,distance=0..] run data get entity @s Pos", s.dim))?;
        let cnt = srv.cmd_long(&format!("execute in minecraft:{} as @e[type=minecraft:item,distance=0..] run data get entity @s Item.count", s.dim))?;
        let ids = srv.cmd_long(&format!("execute in minecraft:{} as @e[type=minecraft:item,distance=0..] run data get entity @s Item.id", s.dim))?;
        let ids: Vec<String> = ids.split("entity data: ").skip(1).map(|v| v.split('"').nth(1).unwrap_or("?").to_string()).collect();
        let pos: Vec<String> = pos.split("entity data: ").skip(1).map(|v| v.split(']').next().unwrap_or("").trim_start_matches('[').replace('d', "")).collect();
        let cnt: Vec<String> = cnt.split("entity data: ").skip(1).map(|v| v.chars().take_while(|c| c.is_ascii_digit()).collect()).collect();
        let ents: Vec<String> = (0..pos.len()).map(|k| format!("{} {} at ({})", cnt.get(k).map(String::as_str).unwrap_or("?"), ids.get(k).map(String::as_str).unwrap_or("?"), pos[k])).collect();
        println!("{when}, {}: containers {}; item entities {}", s.dim, held.join(", "), if ents.is_empty() { "none".to_string() } else { ents.join(", ") });
    }
    Ok(())
}
