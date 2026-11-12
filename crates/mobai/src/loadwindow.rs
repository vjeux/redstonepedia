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
