//! (tickexact 2026-11-12, ironspec 18:46: waterlogged rails as (C)'s descent's drag) `mctas mobai
//! water-rails`: on the official 26.3 server, two copies 8 apart in a void, each a straight descent
//! north of `--slopes` rails (ascending_south, the cart rolling down them north) and then `--flat`
//! flat north_south rails ending at a glass stop, every rail on glass between glass walls (the water
//! kept in); one copy dry, the other with every rail waterlogged. A cart with a villager riding is
//! summoned at rest on each copy's top slope (`--start` its motion north). Printed per copy, every
//! tick until it stops or `--ticks`: the cart's position and speed; the speed at the foot of the
//! slopes, the ticks to it, the speed over the flat and where it stopped.
//! From 26.3's OldMinecartBehavior / AbstractMinecart: in water a slope's slide is 0.0078125 x 0.2
//! a tick, the max speed 0.2 (else 0.4), the natural slowdown (0.997 ridden, 0.96 empty) times 0.95,
//! the off-track gravity 0.005 (else 0.04).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

/// The two copies (x 0 dry, x 8 wet): `wet_slopes` false keeps the wet copy's slopes (and its top
/// rail) dry, only its flats waterlogged.
fn build_rig(srv: &mut VanillaServer, slopes: i32, flat: i32, wet_slopes: bool) -> Result<()> {
    for (k, wet) in [false, true].into_iter().enumerate() {
        let x = k as i32 * 8;
        let ws = wet && wet_slopes;
        // slope i at (x, Y - i, -i), its high end south; the flat after it at y Y - slopes
        for i in 0..slopes {
            let (y, z) = (Y - i, -i);
            srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", y - 1))?;
            for wx in [x - 1, x + 1] {
                srv.cmd_ok(&format!("fill {wx} {} {z} {wx} {} {z} minecraft:glass", y - 1, y + 1))?;
            }
            srv.cmd_ok(&format!("setblock {x} {y} {z} minecraft:rail[shape=ascending_south,waterlogged={ws}]"))?;
        }
        let fy = Y - slopes;
        for j in 0..flat {
            let z = -slopes - j;
            srv.cmd_ok(&format!("setblock {x} {} {z} minecraft:glass", fy - 1))?;
            for wx in [x - 1, x + 1] {
                srv.cmd_ok(&format!("fill {wx} {} {z} {wx} {} {z} minecraft:glass", fy - 1, fy + 1))?;
            }
            srv.cmd_ok(&format!("setblock {x} {fy} {z} minecraft:rail[shape=north_south,waterlogged={wet}]"))?;
        }
        // the stop, and a flat rail at the top slope's high end (so it keeps its slope) on glass,
        // closed south by glass (the water kept in)
        srv.cmd_ok(&format!("fill {x} {fy} {} {x} {} {} minecraft:glass", -slopes - flat, fy + 1, -slopes - flat))?;
        srv.cmd_ok(&format!("setblock {x} {Y} 1 minecraft:glass"))?;
        for wx in [x - 1, x + 1] {
            srv.cmd_ok(&format!("fill {wx} {Y} 1 {wx} {} 1 minecraft:glass", Y + 2))?;
        }
        srv.cmd_ok(&format!("setblock {x} {} 1 minecraft:rail[shape=north_south,waterlogged={ws}]", Y + 1))?;
        srv.cmd_ok(&format!("fill {x} {} 2 {x} {} 2 minecraft:glass", Y + 1, Y + 2))?;
        // the slopes again, their shapes as stated (a slope placed before its neighbours reshapes)
        for i in 0..slopes {
            srv.cmd(&format!("setblock {x} {} {} minecraft:rail[shape=ascending_south,waterlogged={ws}]", Y - i, -i))?;
        }
    }
    Ok(())
}

fn rig_opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, keep_dir: true, world, ..Default::default() }
}

/// (ironspec 20:16 PT, b8949c96's restart bump) The wet copy with dry slopes and `flat` wet flats:
/// a row per restart tick (a save/stop/start at that tick, none for the first row), each cart's
/// speed and tick when its block z first reaches the end of the `measure`-th flat, and the wet
/// copy's speed every tick from 3 before the restart to 3 after.
pub fn water_restarts(version: &str, slopes: i32, flat: i32, measure: i32, restarts: &[u32], ticks: u32, every_tick: bool, both: bool) -> Result<()> {
    println!(
        "== water-rails restart rows on the official {version} server: {slopes} dry slopes then {flat} flats (dry copy / waterlogged copy), a ridden cart from rest on the top slope; the speed where the cart leaves the {measure}th flat (block z {}); one save/stop/start at each of t{restarts:?}",
        -slopes - measure
    );
    let mut rows: Vec<Option<u32>> = vec![None];
    rows.extend(restarts.iter().map(|&r| Some(r)));
    for restart in rows {
        // A load is N whole-chunk idle ticks (load-count's 20-TPS rows); at a world open, pending
        // ticks keep their save-time due times, so redstone pending across the load fires up to N
        // ahead of entities; a moved probe is uncounted harness ticks (a fresh server ticks before
        // the harness's RCON freeze). The row re-runs until the cart held across the load; with
        // `both` the moved attempts (a harness artefact) are caught too, printed after it, and
        // compared with it by world tick (the same trajectory).
        let mut attempt = 0;
        let mut moved: Vec<(u32, RestartRow)> = Vec::new();
        let mut held: Option<(u32, RestartRow)> = None;
        // with `both`, attempts continue (up to 16) until a moved attempt is caught as well
        let cap = if both { 16 } else { 8 };
        while attempt < cap {
            attempt += 1;
            let out = restart_row(version, slopes, flat, measure, restart, ticks)?;
            if out.extra.unwrap_or(0) == 0 {
                if held.is_none() {
                    held = Some((attempt, out));
                }
            } else {
                moved.push((attempt, out));
            }
            if held.is_some() && (!both || !moved.is_empty() || restart.is_none()) {
                break;
            }
        }
        match &held {
            Some((a, out)) => print_restart_row(restart, out, &format!("the row, attempt {a}"), every_tick),
            None => println!("  {}: no attempt held in {attempt}", restart.map(|r| format!("restart between t{} and t{r}", r - 1)).unwrap_or_default()),
        }
        for (a, m) in &moved {
            print_restart_row(restart, m, &format!("a harness artefact: uncounted harness ticks before the freeze, attempt {a}"), every_tick);
            // the two attempts compared by world tick (each attempt's game time from its own first
            // step: a fresh server also ticks a varying number of times before its freeze): the same
            // state at the same world tick means the moved attempt's extra ticks were ordinary ticks
            // the harness did not count
            if let Some((_, h)) = &held {
                let (mut n, mut dz, mut dv) = (0, 0.0f64, 0.0f64);
                let (bm, bh) = (m.full.first().map(|r| r.1).unwrap_or(0), h.full.first().map(|r| r.1).unwrap_or(0));
                for (_, gt, row) in &m.full {
                    if let (Some((p, v)), Some((_, _, Some((hp, hv))))) = (row, h.full.iter().find(|(_, g, _)| g - bh == gt - bm)) {
                        n += 1;
                        dz = dz.max((p[2] - hp[2]).abs()).max((p[1] - hp[1]).abs());
                        dv = dv.max((v[2] - hv[2]).abs()).max((v[1] - hv[1]).abs());
                    }
                }
                println!("    (attempt {a} against the row by world tick, the wet cart: {n} common ticks, max |dpos| {dz:.6}, max |dv| {dv:.6}{})", if dz < 1e-6 && dv < 1e-6 { ": the same trajectory" } else { "" });
            }
        }
        if both && restart.is_some() && moved.is_empty() {
            println!("    (no attempt moved in {attempt})");
        }
    }
    Ok(())
}

fn print_restart_row(restart: Option<u32>, out: &RestartRow, label: &str, every_tick: bool) {
    println!(
        "  {}: dry {}; WET {}{}{}",
        restart.map(|r| format!("restart between t{} and t{r} (t{r} the first tick after the load)", r - 1)).unwrap_or_else(|| "no restart".into()),
        out.cross[0].map(|(t, v)| format!("t{t} at {v:.4}")).unwrap_or("-".into()),
        out.cross[1].map(|(t, v)| format!("t{t} at {v:.4}")).unwrap_or("-".into()),
        match out.extra {
            Some(0) => format!("; the cart held still across the load ({label}; {} idle tick{} — game time +{}; entities held: the row's probe; blocks idle: load-count's measurement)", out.time_jump, if out.time_jump == 1 { "" } else { "s" }, out.time_jump),
            Some(e) => format!("; the cart MOVED across the load (~{e} tick(s); game time jump {}; {label})", out.time_jump),
            None => String::new(),
        },
        if out.around.is_empty() { String::new() } else { format!(" (wet around the restart: {})", out.around.join("; ")) }
    );
    if every_tick {
        // the wet copy every tick: t, world tick (the game time counted from this attempt's first
        // step = 1: a fresh server ticks a varying number of times before its freeze, so absolute
        // game times differ between attempts), x, y, z, vx, vy, vz, cell (x relative to its rail
        // column, y to Y)
        let base = out.full.first().map(|r| r.1 - 1).unwrap_or(0);
        for (t, gt, row) in &out.full {
            let w = gt - base;
            match row {
                Some((p, m)) => println!(
                    "    {t}, {w}, {:.6}, {:.6}, {:.6}, {:+.6}, {:+.6}, {:+.6}, ({},{},{})",
                    p[0],
                    p[1],
                    p[2],
                    m[0],
                    m[1],
                    m[2],
                    p[0].floor() as i32,
                    p[1].floor() as i32,
                    p[2].floor() as i32
                ),
                None => println!("    {t}, {w}, no cart"),
            }
        }
    }
}

struct RestartRow {
    cross: [Option<(u32, f64)>; 2],
    around: Vec<String>,
    full: Vec<(u32, i64, Option<([f64; 3], [f64; 3])>)>,
    /// The ticks the loaded world ran before the freeze took hold (None without a restart): 0 when
    /// the wet cart's position is the same across the load.
    extra: Option<i64>,
    /// The game time's jump across the load (a tick may count with the entities not yet ticking).
    time_jump: i64,
}

fn restart_row(version: &str, slopes: i32, flat: i32, measure: i32, restart: Option<u32>, ticks: u32) -> Result<RestartRow> {
    let mut srv = VanillaServer::start(&rig_opts(version, None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -64 31 16")?;
    srv.step(20)?;
    build_rig(&mut srv, slopes, flat, false)?;
    srv.step(10)?;
    for k in 0..2 {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {}.5 {}.5625 0.5 {{Tags:[\"wr{k}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}}]}}",
            k * 8,
            Y
        ))?;
    }
    let mut r = RestartRow { cross: [None, None], around: Vec::new(), full: Vec::new(), extra: None, time_jump: 0 };
    // (G1 on 94eb02e76) a restarted server can tick before the harness freezes it (uncounted harness
    // ticks): the game time is read before the save and after the load, and the wet cart (moving at
    // the save) says whether any of those ticks ran it
    for t in 1..=ticks {
        if restart == Some(t) {
            srv.cmd_ok("save-on")?;
            srv.cmd_ok("save-all flush")?;
            let before = srv.game_time()?;
            let pos_before = srv.cmd_long("data get entity @e[tag=wr1,limit=1] Pos")?;
            let old = srv.dir.clone();
            let world = old.join(&srv.level_name);
            drop(srv);
            srv = VanillaServer::start(&rig_opts(version, Some(world)))?;
            let after = srv.game_time()?;
            // a load is N whole-chunk idle ticks (load-count's 20-TPS rows), so a held cart across a
            // jump of N is N idle ticks; a cart that moved means uncounted harness ticks
            let pos_after = srv.cmd_long("data get entity @e[tag=wr1,limit=1] Pos")?;
            r.extra = Some(if pos_after.trim() == pos_before.trim() { 0 } else { (after - before).max(1) });
            r.time_jump = after - before;
            let _ = std::fs::remove_dir_all(&old);
            srv.cmd_ok("forceload add -16 -64 31 16")?;
        }
        srv.step(1)?;
        let gt = srv.game_time()?;
        for k in 0..2 {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Pos"))?);
            let m = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Motion"))?);
            if p.len() != 3 || m.len() != 3 {
                if k == 1 {
                    r.full.push((t, gt, None));
                }
                continue;
            }
            let v = m[0].hypot(m[2]);
            if r.cross[k].is_none() && (p[2].floor() as i32) < -slopes - measure + 1 {
                r.cross[k] = Some((t, v));
            }
            if k == 1 {
                r.full.push((t, gt, Some(([p[0] - 8.0, p[1] - Y as f64, p[2]], [m[0], m[1], m[2]]))));
            }
            if k == 1
                && let Some(rt) = restart
                && t + 3 >= rt
                && t <= rt + 3
            {
                r.around.push(format!("t{t} z {:.3} v {v:.4}", p[2]));
            }
        }
        if r.cross.iter().all(|c| c.is_some()) && restart.map(|rt| t > rt + 3).unwrap_or(true) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(r)
}

pub fn water_rails(version: &str, slopes: i32, flat: i32, start: f64, ticks: u32, every_tick: bool) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -64 31 16")?;
    srv.step(20)?;
    println!(
        "== water-rails on the official {version} server: a descent north of {slopes} slopes then {flat} flat rails, on glass between glass walls; a ridden cart from rest on the top slope (motion {start} north); dry vs waterlogged"
    );
    build_rig(&mut srv, slopes, flat, true)?;
    srv.step(10)?;
    for (k, wet) in [false, true].into_iter().enumerate() {
        let x = k as i32 * 8;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {x}.5 {}.5625 0.5 {{Tags:[\"wr{k}\"],Motion:[0.0d,0.0d,{}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}}]}}",
            Y,
            -start
        ))?;
        let _ = wet;
    }
    // (b8949c96's unit test: every tick's position and full motion, `--every-tick`)
    let mut rows: [Vec<(u32, [f64; 3], f64)>; 2] = [Vec::new(), Vec::new()];
    // (G1's note: a tick whose `data get` fails prints "no cart" instead of vanishing)
    let mut full: [Vec<(u32, Option<([f64; 3], [f64; 3])>)>; 2] = [Vec::new(), Vec::new()];
    for t in 1..=ticks {
        srv.step(1)?;
        for (k, row) in rows.iter_mut().enumerate() {
            let p = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Pos"))?);
            let m = nums(&srv.cmd_long(&format!("data get entity @e[tag=wr{k},limit=1] Motion"))?);
            if p.len() == 3 && m.len() == 3 {
                let rel = [p[0] - (k as f64 * 8.0), p[1] - Y as f64, p[2]];
                row.push((t, rel, m[0].hypot(m[2])));
                full[k].push((t, Some((rel, [m[0], m[1], m[2]]))));
            } else {
                full[k].push((t, None));
            }
        }
    }
    for (k, row) in rows.iter().enumerate() {
        let label = if k == 0 { "dry" } else { "WATERLOGGED" };
        let foot = row.iter().find(|(_, p, _)| p[2] < -(slopes as f64) + 0.5 + 0.0001 - 1.0 + 1.0 && p[1] <= -(slopes as f64) + 0.07);
        let stop = row.windows(2).find(|w| w[0].1 == w[1].1 && w[1].2 < 0.001).map(|w| w[1]);
        println!(
            "  {label}: at the foot of the slopes (on the flat) {}; stopped {}",
            foot.map(|(t, p, s)| format!("t{t} at z {:.3}, speed {s:.4}", p[2])).unwrap_or_else(|| "never".into()),
            stop.map(|(t, p, _)| format!("at t{t}, z {:.3}", p[2])).unwrap_or_else(|| format!("not within {ticks} gt")),
        );
        if every_tick {
            // the copy's own frame: x relative to its rail line (x 0.5 the rail's centre), y relative to Y
            println!("    t, x, y, z, vx, vy, vz, cell (x relative to the copy's rail column, y to {Y})");
            for (t, row) in &full[k] {
                match row {
                    Some((p, m)) => println!(
                        "    {t}, {:.6}, {:.6}, {:.6}, {:+.6}, {:+.6}, {:+.6}, ({},{},{})",
                        p[0],
                        p[1],
                        p[2],
                        m[0],
                        m[1],
                        m[2],
                        p[0].floor() as i32,
                        p[1].floor() as i32,
                        p[2].floor() as i32
                    ),
                    None => println!("    {t}, no cart"),
                }
            }
        } else {
            let path: Vec<String> = row.iter().filter(|(t, _, _)| *t <= 12 || t % 5 == 0).map(|(t, p, s)| format!("t{t} y {:.3} z {:.3} v {s:.4}", p[1], p[2])).collect();
            println!("    {}", path.join("; "));
        }
    }
    // every water cell around the wet copy that is not a source (the flowing water the slopes'
    // sources spill), by cell, relative to the copy
    let mut flowing = Vec::new();
    for y in (Y - slopes - 3)..=(Y + 3) {
        for z in (-slopes - flat - 1)..=3 {
            for dx in -3..=3 {
                let x = 8 + dx;
                let out = srv.cmd_long(&format!("execute if block {x} {y} {z} minecraft:water"))?;
                if out.contains("passed") {
                    let src = srv.cmd_long(&format!("execute if block {x} {y} {z} minecraft:water[level=0]"))?.contains("passed");
                    if !src {
                        flowing.push(format!("({dx},{},{z})", y - Y));
                    }
                }
            }
        }
    }
    println!("  the wet copy's flowing (non-source) water cells within 3 of its rail column, relative (x to the column, y to {Y}): {} {flowing:?}", flowing.len());
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
