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

pub fn load_window(version: &str, case: &str, runs: u32, design: Option<&std::path::Path>, mobs: Option<&str>) -> Result<()> {
    let busy = match case {
        "quiet" => false,
        "load" => true,
        "pearl" => return pearl_window(version, runs, false),
        "pearl-busy" => return pearl_window(version, runs, true),
        "portal-loop" => return portal_loop(version, runs),
        "item-loop" => return item_loop(version, runs),
        "item-loop-build" => return item_loop_build(version, runs, false, design, mobs, None),
        "item-loop-build-slab" => return item_loop_build(version, runs, true, design, mobs, None),
        "item-loop-extremes" => return item_loop_build(version, 0, false, design, None, Some((400, Delay::None))),
        // the corners fired `--runs` gt after the portals are built (no 400-gt run first), then
        // every nether_portal and obsidian block of both dimensions' search areas counted
        "item-loop-early" => return item_loop_build(version, 0, false, design, None, Some((runs.min(399), Delay::None))),
        // the same, the portals built 600 gt after the server's start (after start, or after building?)
        "item-loop-early-late" => return item_loop_build(version, 0, false, design, None, Some((runs.min(399), Delay::Portals(600)))),
        // the same, the chunks force-loaded (so generated) only 600 gt after the start, and the
        // portals built at once (the server's start, or the chunks' first load?)
        "item-loop-early-fresh" => return item_loop_build(version, 0, false, design, None, Some((runs.min(399), Delay::Chunks(600)))),
        _ => bail!("--case quiet, load, pearl, pearl-busy, portal-loop, item-loop, item-loop-build, item-loop-build-slab, item-loop-extremes, item-loop-early, item-loop-early-late or item-loop-early-fresh"),
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
    /// The emulated clock's cells; empty for a design with its own (real) clock.
    clock: Vec<[i32; 3]>,
    phase: i64,
    /// The firing column's droppers (the firing one and its feeders).
    droppers: Vec<[i32; 3]>,
}

/// Items in a container (`data get block ... Items`): the sum of its stacks' counts.
fn items_in(srv: &mut VanillaServer, dim: &str, p: [i32; 3]) -> Result<u32> {
    let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get block {} {} {} Items", p[0], p[1], p[2]))?;
    Ok(out.split("count: ").skip(1).filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok()).sum())
}

/// The item loop's item (`--item`; the designs carry cobblestone, replaced on paste).
pub static LOOP_ITEM: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn loop_item() -> &'static str {
    LOOP_ITEM.get().map(String::as_str).unwrap_or("minecraft:cobblestone")
}

/// A side's items: in its containers, and item entities in its dimension.
fn side_items(srv: &mut VanillaServer, s: &LoopSide) -> Result<(u32, u32)> {
    let mut held = 0;
    for p in &s.containers {
        held += items_in(srv, s.dim, *p)?;
    }
    // the loop's item entities in this dimension, by their stacks' counts (a stuck pair merges
    // into one entity; natural drops, a Nether vine, aren't the loop's; the controls' are tagged)
    let out = srv.cmd_long(&format!(
        "execute in minecraft:{} as @e[type=minecraft:item,distance=0..,tag=!ctl,nbt={{Item:{{id:\"{}\"}}}}] run data get entity @s Item.count",
        s.dim,
        loop_item()
    ))?;
    let flying = out.split("entity data: ").skip(1).filter_map(|v| v.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok()).sum();
    Ok((held, flying))
}

/// (tickexact 2026-11-12, ironspec: the gap rule's proof) The mob/pickup row: tagged mobs in the
/// loop's portals (an adult zombified piglin, a baby, a chicken jockey with a baby rider, loot
/// pickup on, empty hands), made to jump every 7 gt (every phase of the 200-gt clocks), checked
/// every 20 gt: each mob's dimension, whether its box stays inside its portal's plane, and
/// whether it holds anything.
struct MobRow {
    /// Each portal's interior: its dimension, min corner, max corner (exclusive).
    planes: Vec<(&'static str, [f64; 3], [f64; 3])>,
    /// Each mob's tag and half width.
    mobs: Vec<(String, f64)>,
    stats: std::cell::RefCell<MobStats>,
}

#[derive(Default)]
struct MobStats {
    checks: u64,
    jumps: u64,
    outside: Vec<String>,
    held: Vec<String>,
    gone: Vec<String>,
}

impl MobRow {
    fn jump(&self, srv: &mut VanillaServer) -> Result<()> {
        for (dim, _, _) in &self.planes {
            srv.cmd(&format!("execute in minecraft:{dim} as @e[tag=loopmob,distance=0..,nbt={{OnGround:1b}}] run data merge entity @s {{Motion:[0.0d,0.42d,0.0d]}}"))?;
        }
        self.stats.borrow_mut().jumps += 1;
        Ok(())
    }

    fn check(&self, srv: &mut VanillaServer, t: i64) -> Result<()> {
        let mut st = self.stats.borrow_mut();
        st.checks += 1;
        for (tag, hw) in &self.mobs {
            let mut found = false;
            for (dim, lo, hi) in &self.planes {
                let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity @e[tag={tag},distance=0..,limit=1] Pos"))?;
                let Some(rest) = out.split("entity data: ").nth(1) else { continue };
                found = true;
                let p: Vec<f64> = rest.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
                if p.len() == 3 {
                    let inside = p[0] - hw >= lo[0] - 0.01 && p[0] + hw <= hi[0] + 0.01 && p[2] - hw >= lo[2] - 0.01 && p[2] + hw <= hi[2] + 0.01 && p[1] >= lo[1] - 0.5 && p[1] < hi[1];
                    if !inside && st.outside.len() < 20 {
                        st.outside.push(format!("t{t} {tag} in {dim} at ({:.3}, {:.3}, {:.3})", p[0], p[1], p[2]));
                    }
                }
                let hand = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity @e[tag={tag},distance=0..,limit=1] equipment"))?;
                if hand.contains(loop_item()) && st.held.len() < 20 {
                    st.held.push(format!("t{t} {tag} in {dim}: {}", hand.split("entity data: ").nth(1).unwrap_or("").trim()));
                }
                break;
            }
            if !found && st.gone.len() < 20 {
                st.gone.push(format!("t{t} {tag}"));
            }
        }
        // every piglin and zombified piglin in either dimension holding the loop's item (the
        // baby piglin's chain converts it, and the conversion may not keep its tags)
        for dim in ["overworld", "the_nether"] {
            for ty in ["minecraft:piglin", "minecraft:zombified_piglin"] {
                let out = srv.cmd_long(&format!("execute in minecraft:{dim} as @e[type={ty},distance=0..,tag=!pickctl] run data get entity @s equipment"))?;
                if out.contains(loop_item()) && st.held.len() < 20 {
                    st.held.push(format!("t{t} a {ty} in {dim} holds the loop's item"));
                }
            }
        }
        Ok(())
    }

    fn report(&self) -> String {
        let st = self.stats.borrow();
        format!(
            "mob/pickup row: {} checks, {} forced jumps; outside their portal: {}; holding anything: {}; gone: {}",
            st.checks,
            st.jumps,
            if st.outside.is_empty() { "none".to_string() } else { st.outside.join("; ") },
            if st.held.is_empty() { "none".to_string() } else { st.held.join("; ") },
            if st.gone.is_empty() { "none".to_string() } else { st.gone.join("; ") }
        )
    }
}

/// Droppers of the sides' firing columns left TRIGGERED outside their clock's pulse (a dropper is
/// TRIGGERED from its rising edge to its falling edge: past the pulse's 2 gt it must be clear).
fn stuck_triggered(srv: &mut VanillaServer, sides: &[LoopSide]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    // an emulated clock: past its pulse every dropper must be clear; a real clock: a dropper
    // TRIGGERED at every one of 40 consecutive ticks is stuck (a running clock clears it)
    let real = sides.iter().any(|s| s.clock.is_empty());
    let mut seen: Vec<Vec<bool>> = sides.iter().map(|s| vec![true; s.droppers.len()]).collect();
    for k in 0..if real { 40 } else { 1 } {
        if k > 0 {
            srv.step(1)?;
        }
        let t = srv.game_time()?;
        for (i, s) in sides.iter().enumerate() {
            if !s.clock.is_empty() && (t - s.phase).rem_euclid(200) < 4 {
                seen[i].iter_mut().for_each(|v| *v = false);
                continue;
            }
            for (j, p) in s.droppers.iter().enumerate() {
                if !srv.cmd(&format!("execute in minecraft:{} if block {} {} {} minecraft:dropper[triggered=true]", s.dim, p[0], p[1], p[2]))?.contains("passed") {
                    seen[i][j] = false;
                }
            }
        }
    }
    let t = srv.game_time()?;
    for (i, s) in sides.iter().enumerate() {
        for (j, p) in s.droppers.iter().enumerate() {
            if seen[i][j] {
                out.push(format!("t{t} {} {:?}", s.dim, p));
            }
        }
    }
    Ok(out)
}

/// Runs the item loop's clocks for `n` gt (each side's droppers powered for 2 gt every 200 gt at
/// its phase), sampling every 5 gt; returns the arrivals' game times per side (a side's total going
/// up) and the fewest and most items seen in all, and the clock pulses that failed (a side's
/// chunk not loaded). With a mob row, its mobs jump every 7 gt and are checked every 20 gt.
fn run_item_loop(srv: &mut VanillaServer, sides: &[LoopSide], n: i64, mobs: Option<&MobRow>) -> Result<(Vec<Vec<i64>>, u32, u32, Vec<String>)> {
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
        if let Some(m) = mobs
            && t % 7 == 0
        {
            m.jump(srv)?;
        }
        srv.step(1)?;
        if let Some(m) = mobs
            && (srv.game_time()? - t0) % 20 == 0
        {
            let now = srv.game_time()?;
            m.check(srv, now)?;
        }
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
        droppers: vec![[16, 64, 34], [16, 65, 34]],
    };
    let neth = LoopSide {
        dim: "the_nether",
        containers: vec![[3, 64, 3], [3, 65, 3], [0, 64, 3], [1, 64, 3], [2, 64, 3], [0, 64, 2], [1, 64, 2], [2, 64, 2]],
        clock: vec![[4, 65, 3], [4, 64, 3]],
        phase: 100,
        droppers: vec![[3, 64, 3], [3, 65, 3]],
    };
    let sides = [over, neth];
    // two cycles with the forced chunks, then without them
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 400, None)?;
    println!("built (forced): overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    srv.cmd_ok("forceload remove 0 16 47 63")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 15 15")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 1600, None)?;
    println!("no forced chunks, 1,600 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    for f in failed.iter().take(3) {
        println!("  {f}");
    }
    let srv = loop_restarts(srv, &opts, &sides, runs, [2, 65, 4], [17, 65, 33], None)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// The item loop's `runs` real saves, stops and starts, each at a different phase of the clocks
/// (the clocks running up to the save): per restart, each side's tickets at the save and at the
/// first tick, the agent's first-tick chunk flags, the first scheduled tick and the first entity
/// tick in each watched overworld chunk (G1: "no load window" measured per restart), whether
/// each side (its probe cell) is loaded right after the start, and the loop's arrivals, items and
/// failed pulses over the 1,200 gt after it.
fn loop_restarts(mut srv: VanillaServer, opts: &dyn Fn(Option<PathBuf>) -> VanillaOptions, sides: &[LoopSide], runs: u32, nether_probe: [i32; 3], over_probe: [i32; 3], mobs: Option<&MobRow>) -> Result<VanillaServer> {
    for run in 0..runs {
        // a different phase of the clocks each time (the clocks keep running up to the save)
        let _ = run_item_loop(&mut srv, sides, 37 + 76 * run as i64, mobs)?;
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
        let (a, lo, hi, failed) = run_item_loop(&mut srv, sides, 1200, mobs)?;
        let stuck = stuck_triggered(&mut srv, sides)?;
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
        println!(
            "  after it, 1,200 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}; droppers stuck TRIGGERED after the run: {}",
            gaps(&a[0]),
            gaps(&a[1]),
            failed.len(),
            if stuck.is_empty() { "none".to_string() } else { stuck.join(", ") }
        );
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
/// item-loop-early*: what waits before the corners' world is built (ironspec: no magic offsets).
#[derive(Clone, Copy)]
enum Delay {
    /// built at once after the start and the force-loads
    None,
    /// the portals built this many gt after the force-loads
    Portals(u32),
    /// the force-loads (so the chunks' generation) this many gt after the start
    Chunks(u32),
}

fn item_loop_build(version: &str, runs: u32, slab: bool, design: Option<&std::path::Path>, mobs: Option<&str>, corners: Option<(u32, Delay)>) -> Result<()> {
    let design = design.unwrap_or(std::path::Path::new(BUILD_LOOP));
    let d = mctas::design::Design::load(design)?;
    let jar = mctas::bridge::load_agent(version)?;
    let agent = PathBuf::from(format!("{}=watch=0,0,1,2;ticks=40;tickets=1;running=1", jar.display()));
    println!(
        "== load-window item-loop-build{} on the official {version} server: the L1''' loop of {}{}, no player online, {runs} restarts",
        if slab { "-slab" } else { "" },
        design.display(),
        match mobs {
            Some(m) => format!(", with the mob/pickup row ({m})"),
            None => String::new(),
        }
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
    // item-loop-early-fresh: the wait comes before the force-loads
    if let Some((_, Delay::Chunks(pre))) = corners {
        srv.step(pre)?;
        println!("the chunks force-loaded {pre} gt after the start (game time {})", srv.game_time()?);
    }
    srv.cmd_ok("forceload add 0 0 31 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload add -16 -16 31 31")?;
    srv.step(20)?;
    if let Some((_, Delay::Portals(pre))) = corners {
        srv.step(pre)?;
        println!("the portals built {pre} gt later than usual (game time {})", srv.game_time()?);
    }
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
        let nbt = nbt.replace("minecraft:cobblestone", loop_item());
        srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", t[0] + p[0], t[1] + p[1], t[2] + p[2]))?;
    }
    for (p, nbt) in &d.nether_block_entities {
        let nbt = nbt.replace("minecraft:cobblestone", loop_item());
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
    // (flag 2) doesn't give); none for a design with its own clock (observers: onechunk's)
    let side = |blocks: &std::collections::BTreeMap<[i32; 3], String>, at: [i32; 3], dim: &'static str, phase: i64| -> Result<LoopSide> {
        let containers: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.contains("dropper") || s.contains("hopper")).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2]]).collect();
        let (fp, fs) = blocks.iter().find(|(_, s)| s.contains("dropper") && !s.contains("facing=up")).ok_or_else(|| anyhow::anyhow!("{dim}: no firing dropper"))?;
        let dz = if fs.contains("facing=north") { 1 } else if fs.contains("facing=south") { -1 } else { bail!("{dim}: the firing dropper faces neither north nor south: {fs}") };
        let droppers: Vec<[i32; 3]> = blocks.iter().filter(|(p, s)| s.contains("dropper") && p[0] == fp[0] && p[2] == fp[2]).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2]]).collect();
        let real = blocks.values().any(|s| s.contains("observer"));
        let clock: Vec<[i32; 3]> = if real { Vec::new() } else { droppers.iter().map(|p| [p[0], p[1], p[2] + dz]).collect() };
        Ok(LoopSide { dim, containers, clock, phase, droppers })
    };
    let sides = [side(&d.blocks, t, "overworld", 0)?, side(&d.nether_blocks, n0, "the_nether", 100)?];
    // the mob/pickup row: in each portal's bottom row an adult zombified piglin, a baby and a
    // chicken jockey (a baby rider), loot pickup on, no equipment (summoned with data, so no
    // finalizeSpawn and no golden sword), the adult beside the arrivals' pocket's column; and a
    // positive control, a zombified piglin with an item at its feet on the markers' floor
    let row = if let Some(mode) = mobs {
        if mode != "empty" && mode != "armed" && mode != "mixed" {
            bail!("--mobs empty, armed or mixed");
        }
        // the loop runs 400 gt first: an entity sent through these command-built portals within
        // tens of gt of their building was lost (the harness's open question), not one after
        let _ = run_item_loop(&mut srv, &sides, 400, None)?;
        let interior = |blocks: &std::collections::BTreeMap<[i32; 3], String>, at: [i32; 3]| -> Result<([f64; 3], [f64; 3])> {
            let ps: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.contains("nether_portal")).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2]]).collect();
            let lo = [0, 1, 2].map(|k| ps.iter().map(|p| p[k]).min().unwrap_or(0) as f64);
            let hi = [0, 1, 2].map(|k| ps.iter().map(|p| p[k]).max().unwrap_or(0) as f64 + 1.0);
            if ps.is_empty() {
                bail!("no portal in the design");
            }
            Ok((lo, hi))
        };
        let (olo, ohi) = interior(&d.blocks, t)?;
        let (nlo, nhi) = interior(&d.nether_blocks, n0)?;
        let mut tags = Vec::new();
        // armed: the golden sword every portal piglin spawns with
        // (ZombifiedPiglin.populateDefaultEquipmentSlots), in the summon's data so it holds it from
        // its first tick (a mob summoned in a portal crosses on that tick); loot pickup forced on.
        // mixed: the babies empty-handed (the deciding row), the adults and the jockey riders armed
        let sword = ",equipment:{mainhand:{id:\"minecraft:golden_sword\",count:1}}";
        let (eq_adult, eq_baby, eq_rider) = match mode {
            "armed" => (sword, sword, sword),
            "mixed" => (sword, "", sword),
            _ => ("", "", ""),
        };
        for (k, (dim, lo, cols)) in [("overworld", olo, [3.5, 2.5, 4.5]), ("the_nether", nlo, [1.5, 2.5, 0.5])].into_iter().enumerate() {
            let b = 4 * k;
            let (y, z) = (lo[1], lo[2] + 0.5);
            let c = |cmd: String| if dim == "overworld" { cmd } else { format!("execute in minecraft:the_nether run {cmd}") };
            srv.cmd_ok(&c(format!("summon minecraft:zombified_piglin {} {y} {z} {{Tags:[\"loopmob\",\"lm{b}\"],CanPickUpLoot:1b,PersistenceRequired:1b{eq_adult}}}", lo[0] + cols[0])))?;
            srv.cmd_ok(&c(format!("summon minecraft:zombified_piglin {} {y} {z} {{Tags:[\"loopmob\",\"lm{}\"],IsBaby:1b,CanPickUpLoot:1b,PersistenceRequired:1b{eq_baby}}}", lo[0] + cols[1], b + 1)))?;
            srv.cmd_ok(&c(format!(
                "summon minecraft:chicken {} {y} {z} {{Tags:[\"loopmob\",\"lm{}\"],IsChickenJockey:1b,PersistenceRequired:1b,Passengers:[{{id:\"minecraft:zombified_piglin\",Tags:[\"loopmob\",\"lm{}\"],IsBaby:1b,CanPickUpLoot:1b,PersistenceRequired:1b{eq_rider}}}]}}",
                lo[0] + cols[2],
                b + 2,
                b + 3
            )))?;
            tags.extend([(format!("lm{b}"), 0.3), (format!("lm{}", b + 1), 0.15), (format!("lm{}", b + 2), 0.2), (format!("lm{}", b + 3), 0.15)]);
        }
        // each mob's hands at the start
        for (tag, _) in &tags {
            let mut hands = String::from("not found");
            for dim in ["overworld", "the_nether"] {
                let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity @e[tag={tag},distance=0..,limit=1] equipment"))?;
                if out.contains("entity data") {
                    hands = format!("{dim}: {}", out.split("entity data: ").nth(1).unwrap_or("").trim());
                } else if out.contains("Found no elements") && hands == "not found" {
                    hands = format!("{dim}: empty");
                }
            }
            println!("  {tag} at the start: {hands}");
        }
        srv.cmd_ok("summon minecraft:zombified_piglin 24.5 64 45.5 {Tags:[\"pickctl\"],CanPickUpLoot:1b,PersistenceRequired:1b,NoAI:0b}")?;
        srv.cmd_ok(&format!("summon minecraft:item 24.5 64.1 45.5 {{Item:{{id:\"{}\",count:1}},PickupDelay:0s,Tags:[\"ctl\"]}}", loop_item()))?;
        // the armed row's own control: a sword-holder with the loop's item at its feet keeps its sword
        if mode != "empty" {
            srv.cmd_ok("summon minecraft:zombified_piglin 28.5 64 40.5 {Tags:[\"pickctl2\"],CanPickUpLoot:1b,PersistenceRequired:1b,equipment:{mainhand:{id:\"minecraft:golden_sword\",count:1}}}")?;
            srv.cmd_ok(&format!("summon minecraft:item 28.5 64.1 40.5 {{Item:{{id:\"{}\",count:1}},PickupDelay:0s,Tags:[\"ctl\"]}}", loop_item()))?;
        }
        // mixed: a baby piglin in the Nether portal's bottom row (the natural chain: it crosses
        // at once and zombifies in the overworld after 300 gt, its hand empty); the checks sweep
        // every piglin and zombified piglin in both dimensions
        if mode == "mixed" {
            srv.cmd_ok(&format!("execute in minecraft:the_nether run summon minecraft:piglin {} {} {} {{Tags:[\"loopmob\",\"lm8\"],IsBaby:1b,PersistenceRequired:1b}}", nlo[0] + 3.5, nlo[1], nlo[2] + 0.5))?;
        }
        Some(MobRow { planes: vec![("overworld", olo, ohi), ("the_nether", nlo, nhi)], mobs: tags, stats: Default::default() })
    } else {
        None
    };
    // `--case item-loop-early`: the corners fired right after building (ironspec's hypothesis
    // for the early loss: a crossing before the twin's POI is findable builds a new portal)
    if let Some(w) = corners.map(|c| c.0).filter(|w| *w < 400) {
        for (blocks, at, dim) in [(&d.blocks, t, "overworld"), (&d.nether_blocks, n0, "the_nether")] {
            for (p, s) in blocks.iter() {
                if s.contains("observer") {
                    srv.cmd_ok(&format!("execute in minecraft:{dim} run setblock {} {} {} minecraft:glass", at[0] + p[0], at[1] + p[1], at[2] + p[2]))?;
                }
            }
        }
        if w > 0 {
            srv.step(w)?;
        }
        println!("the corners fired {w} gt after the portals were built");
        let bad = fire_corners(&mut srv, &d, t, n0)?;
        where_items(&mut srv, &sides, "after the corners")?;
        // G1: hidden, not lost? An entity added to a section whose chunk is not yet FULL is stored
        // but invisible to selectors; 600 gt later the chunk is promoted and it shows up
        srv.step(600)?;
        where_items(&mut srv, &sides, "600 gt after the corners")?;
        // every obsidian block in each dimension's portal search area (createPortal: within 16 of
        // the scaled point, the whole column from the bottom to the logical height's top), counted
        // by replacing them with air in slices of at most 32,768 blocks (this ends the run). Portal
        // blocks can't be counted that way: replacing one slice collapses the rest of a portal.
        for (dim, x0, x1, z0, z1, y0, y1) in [("overworld", -16, 47, 0, 63, -64, 319), ("the_nether", -16, 31, -16, 31, 0, 127)] {
            for block in ["minecraft:obsidian"] {
                let mut total = 0u64;
                let rows = (32768 / ((x1 - x0 + 1) * (z1 - z0 + 1))).max(1);
                let mut y = y0;
                while y <= y1 {
                    let ye = (y + rows - 1).min(y1);
                    let out = srv.cmd_long(&format!("execute in minecraft:{dim} run fill {x0} {y} {z0} {x1} {ye} {z1} minecraft:air replace {block}"))?;
                    total += out.split_whitespace().find_map(|w| w.parse::<u64>().ok()).unwrap_or(0);
                    y = ye + 1;
                }
                println!("{dim}: {total} {block} blocks in x {x0}..{x1}, y {y0}..{y1}, z {z0}..{z1}");
            }
        }
        let log = std::fs::read_to_string(&srv.log).unwrap_or_default();
        let warns: Vec<&str> = log.lines().filter(|l| l.contains("WARN") || l.contains("ERROR")).collect();
        println!("server log: {} WARN/ERROR lines; the last ones:", warns.len());
        for l in warns.iter().rev().take(12).rev() {
            println!("  {}", &l[..l.len().min(240)]);
        }
        let _ = std::fs::remove_dir_all(&srv.dir);
        println!("{bad} corners lost or touching");
        return Ok(());
    }
    where_items(&mut srv, &sides, "built")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 400, row.as_ref())?;
    println!("built (forced): overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    where_items(&mut srv, &sides, "after 400 gt forced")?;
    // `--case item-loop-extremes`: the clocks stopped (a real clock's observers taken out; an
    // emulated one simply not pulsed), the loop's flights left to land, then the dispense
    // support's corners fired from each side through this same world
    if corners.is_some() {
        for (blocks, at, dim) in [(&d.blocks, t, "overworld"), (&d.nether_blocks, n0, "the_nether")] {
            for (p, s) in blocks.iter() {
                if s.contains("observer") {
                    srv.cmd_ok(&format!("execute in minecraft:{dim} run setblock {} {} {} minecraft:glass", at[0] + p[0], at[1] + p[1], at[2] + p[2]))?;
                }
            }
        }
        srv.step(100)?;
        let bad = fire_corners(&mut srv, &d, t, n0)?;
        where_items(&mut srv, &sides, "after the corners")?;
        let _ = std::fs::remove_dir_all(&srv.dir);
        if bad > 0 {
            bail!("{bad} dispense corners lost or touching");
        }
        return Ok(());
    }
    if row.is_some() {
        let ctl = srv.cmd_long("data get entity @e[tag=pickctl,limit=1] equipment.mainhand")?;
        println!("pickup control (a zombified piglin with the same data, the loop's item at its feet): {}", if ctl.contains(loop_item()) { "taken" } else { "NOT taken" });
        if mobs.is_some_and(|m| m != "empty") {
            let ctl2 = srv.cmd_long("data get entity @e[tag=pickctl2,limit=1] equipment")?;
            println!("armed control (a golden-sword holder, loot pickup on, the loop's item at its feet): {}", if ctl2.contains(loop_item()) { "TAKEN" } else { "kept its sword" });
        }
    }
    srv.cmd_ok("forceload remove 0 0 31 47")?;
    srv.cmd_ok("execute in minecraft:the_nether run forceload remove -16 -16 31 31")?;
    let (a, lo, hi, failed) = run_item_loop(&mut srv, &sides, 1600, row.as_ref())?;
    println!("no forced chunks, 1,600 gt: overworld {}, nether {}; items in all {lo}..{hi}; failed pulses {}", gaps(&a[0]), gaps(&a[1]), failed.len());
    for f in failed.iter().take(3) {
        println!("  {f}");
    }
    let probe_n = [n0[0] + 2, n0[1] + 11, n0[2]];
    let probe_o = [t[0] + 2, t[1] + 11, t[2]];
    let mut srv = loop_restarts(srv, &opts, &sides, runs, probe_n, probe_o, row.as_ref())?;
    // where the loop's items are at the end: each side's containers, and every item entity
    where_items(&mut srv, &sides, "at the end")?;
    if let Some(r) = &row {
        println!("{}", r.report());
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}

/// (tickexact 2026-11-12, ironspec: zero loss over the dropper's bounded launch distribution)
/// `--case item-loop-extremes`: the loop's design pasted with its clocks left out (no observer),
/// then from each side's firing dropper one scripted item per corner of `spawnItem`'s support
/// (DefaultDispenseItemBehavior, accuracy 6: forward 0.2..0.3 +- 0.103365, sideways 0 +- 0.103365,
/// up 0.2 +- 0.103365; so forward 0.096635 / 0.403365 x sideways -+0.103365 x up 0.096635 /
/// 0.303365) and the centre (0.25, 0, 0.2), spawned where the dropper spawns its item (the face
/// centre 0.7 out, 0.15625 lower) with that motion; tracked every tick across the portal: where it
/// crosses the arrival side's exit face (its sideways position against the pocket's opening, its
/// bottom against the pocket's floor), whether it comes back into the portal cell, and where it
/// ends (a hopper takes it, or it is left as an entity).
fn fire_corners(srv: &mut VanillaServer, d: &mctas::design::Design, t: [i32; 3], n0: [i32; 3]) -> Result<usize> {
    let n = |c: &str| format!("execute in minecraft:the_nether run {c}");
    // each side: its dimension, its firing dropper (absolute) and facing (dz), its portal interior
    // (min, max inclusive), its blocks and frame offset
    struct Part<'a> {
        dim: &'static str,
        fire: [i32; 3],
        dz: i32,
        lo: [i32; 3],
        hi: [i32; 3],
        blocks: &'a std::collections::BTreeMap<[i32; 3], String>,
        at: [i32; 3],
    }
    let mk = |blocks: &std::collections::BTreeMap<[i32; 3], String>, at: [i32; 3], dim: &'static str| -> Result<([i32; 3], i32, [i32; 3], [i32; 3])> {
        let (fp, fs) = blocks.iter().find(|(_, s)| s.contains("dropper") && !s.contains("facing=up")).ok_or_else(|| anyhow::anyhow!("{dim}: no firing dropper"))?;
        let dz = if fs.contains("facing=north") { -1 } else if fs.contains("facing=south") { 1 } else { bail!("{dim}: the firing dropper faces neither north nor south") };
        let ps: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.contains("nether_portal")).map(|(p, _)| [at[0] + p[0], at[1] + p[1], at[2] + p[2]]).collect();
        let lo = [0, 1, 2].map(|k| ps.iter().map(|p| p[k]).min().unwrap_or(0));
        let hi = [0, 1, 2].map(|k| ps.iter().map(|p| p[k]).max().unwrap_or(0));
        Ok(([at[0] + fp[0], at[1] + fp[1], at[2] + fp[2]], dz, lo, hi))
    };
    let (of, odz, olo, ohi) = mk(&d.blocks, t, "overworld")?;
    let (nf, ndz, nlo, nhi) = mk(&d.nether_blocks, n0, "the_nether")?;
    let parts = [
        Part { dim: "overworld", fire: of, dz: odz, lo: olo, hi: ohi, blocks: &d.blocks, at: t },
        Part { dim: "the_nether", fire: nf, dz: ndz, lo: nlo, hi: nhi, blocks: &d.nether_blocks, at: n0 },
    ];
    let dev = 0.0172275 * 6.0;
    let mut launches: Vec<(f64, f64, f64)> = Vec::new();
    for fwd in [0.2 - dev, 0.3 + dev] {
        for side in [-dev, dev] {
            for up in [0.2 - dev, 0.2 + dev] {
                launches.push((fwd, side, up));
            }
        }
    }
    launches.push((0.25, 0.0, 0.2));
    let mut bad = 0;
    for (k, from) in parts.iter().enumerate() {
        let to = &parts[1 - k];
        // the arrival side's exit face: the items keep the launch's direction (same-axis portals)
        let face_z = if from.dz < 0 { to.lo[2] as f64 } else { to.hi[2] as f64 + 1.0 };
        let face_cell_z = if from.dz < 0 { to.lo[2] - 1 } else { to.hi[2] + 1 };
        // the pocket: the open face cells in the portal's top two rows, and the floor under them
        let open: Vec<i32> = (to.lo[0]..=to.hi[0]).filter(|x| [to.hi[1], to.hi[1] - 1].iter().all(|y| !to.blocks.contains_key(&[x - to.at[0], y - to.at[1], face_cell_z - to.at[2]]))).collect();
        let (ox0, ox1) = (open.iter().copied().min().unwrap_or(0) as f64, open.iter().copied().max().unwrap_or(-1) as f64 + 1.0);
        let floor = (to.hi[1] - 1) as f64;
        println!("from the {} (firing dropper {:?}): the {} exit face at z {face_z}, the pocket open over x {ox0}..{ox1}, its floor y {floor}", from.dim, from.fire, to.dim);
        for (fwd, side, up) in &launches {
            let f = from.fire;
            let (sx, sy, sz) = (f[0] as f64 + 0.5, f[1] as f64 + 0.5 - 0.15625, f[2] as f64 + 0.5 + 0.7 * from.dz as f64);
            let (mx, mz) = (*side, fwd * from.dz as f64);
            let c = format!("summon minecraft:item {sx} {sy} {sz} {{Item:{{id:\"{}\",count:1}},Tags:[\"ext\"],Motion:[{mx}d,{up}d,{mz}d]}}", loop_item());
            srv.cmd_ok(&if from.dim == "overworld" { c } else { n(&c) })?;
            let mut track: Vec<(&'static str, [f64; 3])> = Vec::new();
            let mut gone = None;
            let mut misses = 0;
            for tick in 1..=120 {
                srv.step(1)?;
                let mut found = None;
                for dim in ["overworld", "the_nether"] {
                    let out = srv.cmd_long(&format!("execute in minecraft:{dim} run data get entity @e[type=minecraft:item,tag=ext,distance=0..,limit=1] Pos"))?;
                    if let Some(rest) = out.split("entity data: ").nth(1) {
                        let p: Vec<f64> = rest.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
                        if p.len() == 3 {
                            found = Some((dim, [p[0], p[1], p[2]]));
                        }
                    }
                }
                // (a dimension change leaves the item in neither level for a tick or so)
                match found {
                    Some(f) => {
                        track.push(f);
                        misses = 0;
                    }
                    None => {
                        misses += 1;
                        if misses >= 5 {
                            gone = Some(tick - 4);
                            break;
                        }
                    }
                }
            }
            // the crossing: the ticks in the arrival dimension whose box spans the face plane
            let arr: Vec<[f64; 3]> = track.iter().filter(|(dim, _)| *dim == to.dim).map(|(_, p)| *p).collect();
            let crossing: Vec<&[f64; 3]> = arr.iter().filter(|p| p[2] - 0.125 < face_z && p[2] + 0.125 > face_z).collect();
            let beyond = |p: &[f64; 3]| if from.dz < 0 { p[2] + 0.125 <= face_z } else { p[2] - 0.125 >= face_z };
            let first_beyond = arr.iter().position(beyond);
            let back_in = first_beyond.is_some_and(|i| arr[i..].iter().any(|p| !beyond(p)));
            let side_hit = crossing.iter().any(|p| p[0] - 0.125 < ox0 || p[0] + 0.125 > ox1);
            let low_hit = crossing.iter().any(|p| p[1] < floor);
            // taken before a sampled tick showed it spanning the face: a hopper under the pocket
            // reaches up through the floor (its suck box tops out 1 block above it); then the last
            // position must be over the opening and above the floor
            let taken_at_face = crossing.is_empty()
                && gone.is_some()
                && arr.last().is_some_and(|p| p[0] - 0.125 >= ox0 && p[0] + 0.125 <= ox1 && p[1] >= floor && (p[2] - face_z).abs() < 0.5);
            let centre = (to.lo[0] as f64) + (from.fire[0] - from.lo[0]) as f64 + 0.5;
            let at_face = crossing
                .first()
                .map(|p| format!("x {:.3} ({:+.3} from the column's centre), bottom y {:.3}", p[0], p[0] - centre, p[1]))
                .unwrap_or(if taken_at_face { "taken at the face by a hopper under the pocket, over the opening".into() } else { "never crossed".into() });
            let (xmin, xmax) = arr.iter().filter(|p| beyond(p)).fold((f64::NAN, f64::NAN), |(a, b), p| (if a.is_nan() { p[0] } else { a.min(p[0]) }, if b.is_nan() { p[0] } else { b.max(p[0]) }));
            let last = track.last().map(|(dim, p)| format!("{dim} ({:.3}, {:.3}, {:.3})", p[0], p[1], p[2])).unwrap_or("-".into());
            let ok = gone.is_some() && (crossing.len() > 0 || taken_at_face) && !side_hit && !low_hit && !back_in;
            if !ok {
                bad += 1;
            }
            println!(
                "  forward {fwd:.6} sideways {side:+.6} up {up:.6}: at the face {at_face}{}{}{}; in the pocket x {:.3}..{:.3}; {} ({})",
                if side_hit { "; MEETS THE FACE BESIDE THE POCKET" } else { "" },
                if low_hit { "; BELOW THE POCKET'S FLOOR AT THE FACE" } else { "" },
                if back_in { "; BACK INTO THE PORTAL CELL" } else { "" },
                xmin,
                xmax,
                match gone { Some(g) => format!("taken by a hopper by tick {g}"), None => "NOT TAKEN within 120 gt".to_string() },
                last
            );
        }
    }
    println!("{bad} launches lost or touching the face beside the pocket, below its floor or back in the portal cell");
    Ok(bad)
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
