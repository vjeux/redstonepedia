//! (tickexact 2026-11-14, ironspec 00:10 / G1 00:10 / ironfarm 00:08 / 00:15 / 00:22: the lift-top drift) `mctas mobai
//! lift-top <design>`: the return's shared lift as its design file lays it (f1q2-lift-split-path-a: soul sand (-5,1,7),
//! the bubble column (-5,2..22,7) in glass, the top-drift current (-5,23,7) fed by the source (-5,23,6), the exit booster
//! (-5,23,8) a powered rail ascending_south powered by the torch (-6,23,8)), pasted at its own coordinates in the
//! official server (`portal::paste`: blocks strict, then each fluid placed over air so its first flow tick is
//! scheduled) and settled 100 gt; risers summoned in the shaft (1 wide: a 0.98 cart's z is confined to 7.49..7.51) with
//! the column's rise (0, 0.695, 0). G1 00:45's event names: SURFACING (the box's bottom at y 23.30), ONTO THE BOOSTER
//! (the position inside (-5,23,8)), CLEAR OF THE TOP CELL (the box out of (-5,23,7); the riser is removed then).
//!
//! The push at the top is 26.3's `EntityFluidInteraction` (37-102, 210-227): every fluid cell the box (deflated by
//! 0.001) overlaps adds its flow, and for a non-player the SUM is normalized before x 0.014.
//!
//! Rows: (D) ironfarm's gate G: (-4,2,7) a detector rail east_west, a ridden cart rolled up the climb at 0.30 / 0.40 /
//! 0.48: its press and release against the cart's rise past y 12. (A) LONE RISERS at z0 7.490..7.510 by 0.005, ridden and
//! empty, 150 gt: the three events. (C) TWO RISERS rising together, the lower (ridden) s below, s = 0.60..0.95 by 0.05,
//! 1.0..1.5, 2.0; the upper 0.01 north, aligned and 0.01 south; ridden and empty above. (B) PAIRS: a lone riser held 100
//! gt under glass at the booster; the exit OPEN with a follower G = 1..14 gt behind; the booster glass from the leader's
//! arrival (y >= 22) for N gt with a follower G = 14 / 20 / 28 gt behind, N = G - 6 .. G + 6 by 2; each watched 300 gt
//! past the restore (or to both risers clear): JAM = a riser never clear; the events and THE MARGIN (the follower's
//! SURFACING - the leader's CLEAR) printed.
//! Asserted (MCTAS_LIFT_TOP_PLANT=k inverts assert k): (1) every lone riser in the shaft's band exits; (2) the first
//! run's bunched pairs: upper 0.01 north or aligned JAM at 0.70 and 0.80, not at 0.60 or 0.90 .. 2.0; 0.01 south never;
//! (3) a lone riser held 100 gt exits after the restore; (4) per follower gap the jamming blocks are an upper set in N;
//! (5) the foot detector presses for every entry speed, before the cart's rise past y 12; (6) (E) FOOT PAIRS (G1 00:59 /
//! 01:18: two ridden carts up the climb at 0.30 / 0.33 / 0.36 / 0.48, the follower 3/4..14 gt behind, offset 0 / 0.05 /
//! 0.10 / 0.15 east): every pair at >= 9 gt (0.30) / 8 (0.33) / 7 (0.36) / 6 (0.48) exits with >= 1.0 b at the leader's
//! SURFACING, and closer pairs can STALL the follower at the foot (it bumps the leader turning up into the shaft and
//! stops rail-bound on (-4,2,7), or is thrown back east onto the T): the first run's 0.48 / 3 gt and 0.30 / 8 gt stall.
//! (1- and 2-gt foot gaps are unreachable: two carts are 0.98 long.) A cart stays rail-bound while its POSITION's block
//! is the rail (OldMinecartBehavior.tick 50-65; moveAlongTrack's move has no vertical part, 199-201), so the column only
//! lifts it once its centre is past x -4.0. (7) (ironspec 03:07) every OPEN-EXIT pair (the exit open, N 0; the follower
//! G = 1..14 gt behind) exits, both risers: the brief's normal-running evidence (the 1-gt pair rises 0.695 b apart, 0.005
//! under the 0.70 jam band; printed). (cc767b78 02:47) MCTAS_LIFT_TOP_FULL_A=1 prints every tick of (A)'s rise from its
//! summon (y, z, vy, vz), not only the top.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const BOOSTER: [i32; 3] = [-5, 23, 8];
const BOOSTER_STATE: &str = "minecraft:powered_rail[powered=true,shape=ascending_south,waterlogged=false]";

fn nums(s: &str) -> Vec<f64> {
    s.split('[').nth(1).unwrap_or("").split(',').filter_map(|x| x.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect()
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_LIFT_TOP_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

/// One riser's state this tick: position and motion (None once gone).
fn read(srv: &mut VanillaServer, tag: &str) -> Result<Option<([f64; 3], [f64; 3])>> {
    let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
    let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
    Ok((p.len() == 3 && m.len() == 3).then(|| ([p[0], p[1], p[2]], [m[0], m[1], m[2]])))
}

fn summon(srv: &mut VanillaServer, tag: &str, y: f64, z0: f64, ridden: bool) -> Result<()> {
    let rider = if ridden { format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]") } else { String::new() };
    srv.cmd_ok(&format!("summon minecraft:minecart -4.5 {y:.4} {z0:.4} {{Tags:[\"{tag}\"],Motion:[0.0d,0.695d,0.0d]{rider}}}"))?;
    Ok(())
}

/// Away from the lift, then killed (a kill in place could drop an item into the column).
fn remove(srv: &mut VanillaServer, tag: &str) -> Result<()> {
    srv.cmd(&format!("tp @e[tag={tag}v] -40 -50 -40"))?;
    srv.cmd(&format!("kill @e[tag={tag}v]"))?;
    srv.cmd(&format!("tp @e[tag={tag}] -40 -50 -40"))?;
    srv.cmd(&format!("kill @e[tag={tag}]"))?;
    Ok(())
}

/// G1 00:45's shared event: CLEAR OF THE TOP CELL, the box (0.98 wide, 0.7 tall) out of (-5,23,7): its min z >= 8 or its
/// bottom >= 24 (the riser is removed then)
fn past_booster(p: [f64; 3]) -> bool {
    p[2] - 0.49 >= 8.0 || p[1] >= 24.0
}

/// SURFACING: the box's bottom at y 23.30
fn surfaced(p: [f64; 3]) -> bool {
    p[1] >= 23.2999
}

/// ONTO THE BOOSTER: the cart's position inside the booster cell (-5,23,8)
fn on_booster(p: [f64; 3]) -> bool {
    p[2] >= 8.0 && p[1] >= 23.0
}

#[derive(Debug, Clone)]
enum Fate {
    Exit(u32),
    Stuck([f64; 3]),
}

pub fn lift_top(version: &str, design: &std::path::Path) -> Result<()> {
    let d = mctas::design::Design::load(design)?;
    let blocks = d.block_list();
    println!("== lift-top on the official {version} server: {} ({} cells) pasted at its own coordinates", design.display(), blocks.len());
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
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
        (BOOSTER, BOOSTER_STATE),
        ([-6, 23, 8], "minecraft:redstone_torch"),
        ([-5, 23, 6], "minecraft:water[level=0]"),
        ([-5, 23, 7], "minecraft:water[level=1]"),
        ([-5, 22, 7], "minecraft:bubble_column[drag=false]"),
        ([-5, 13, 7], "minecraft:bubble_column[drag=false]"),
        ([-5, 1, 7], "minecraft:soul_sand"),
    ] {
        let out = srv.cmd(&format!("execute if block {} {} {} {s}", p[0], p[1], p[2]))?;
        anyhow::ensure!(out.contains("passed"), "the settled lift: ({},{},{}) is not {s}", p[0], p[1], p[2]);
    }
    println!("  settled: the booster (-5,23,8) on, the source (-5,23,6), the current (-5,23,7) at level 1, the bubble column to (-5,22,7)");
    let foot: Vec<String> = blocks.iter().filter(|(p, _)| (-7..=-1).contains(&p[0]) && (0..=3).contains(&p[1]) && (5..=9).contains(&p[2])).map(|(p, s)| format!("({},{},{}) {s}", p[0], p[1], p[2])).collect();
    println!("  the lift's foot as the file lays it (x -7..-1, y 0..3, z 5..9): {}", foot.join("; "));

    // (D) ironfarm's gate G (00:22): a detector rail at the foot entry (-4,2,7) (the file's rail east_west there, the
    // cart arriving up the climb (-2,0,7) -> (-3,1,7)); its press against the same cart's rise past y 12 (lane 7's
    // side entry)
    println!("(D) the foot detector: (-4,2,7) a detector rail east_west, a ridden cart rolled west onto the climb (-3,1,7); its press / release vs the cart's y 12");
    srv.cmd_ok("setblock -4 2 7 minecraft:detector_rail[shape=east_west,powered=false,waterlogged=false]")?;
    srv.step(5)?;
    let mut det: Vec<(f64, Option<u32>, Option<u32>, Option<u32>)> = Vec::new();
    for v in [0.30, 0.40, 0.48] {
        let tag = format!("ld{}", det.len());
        srv.cmd_ok(&format!("summon minecraft:minecart -2.2 1.8 7.5 {{Tags:[\"{tag}\"],Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]}}", -v))?;
        let (mut press, mut release, mut y12) = (None, None, None);
        let mut lines = Vec::new();
        for t in 1..=60u32 {
            srv.step(1)?;
            let on = srv.cmd("execute if block -4 2 7 minecraft:detector_rail[powered=true]")?.contains("passed");
            if on && press.is_none() {
                press = Some(t);
            }
            if !on && press.is_some() && release.is_none() {
                release = Some(t);
            }
            if let Some((p, m)) = read(&mut srv, &tag)? {
                if y12.is_none() && p[1] >= 12.0 {
                    y12 = Some(t);
                }
                if t <= 30 {
                    lines.push(format!("t{t} x {:.3} y {:.3} vx {:+.3} vy {:+.3}{}", p[0], p[1], m[0], m[1], if on { " D" } else { "" }));
                }
            }
            if y12.is_some() && release.is_some() {
                break;
            }
        }
        println!("  v {v:.2}: detector pressed t{} released t{}, the cart at y 12 t{} (press -> y 12: {} gt)", press.map_or("-".into(), |t| t.to_string()), release.map_or("-".into(), |t| t.to_string()), y12.map_or("-".into(), |t| t.to_string()), press.zip(y12).map_or("-".into(), |(a, b)| (b as i64 - a as i64).to_string()));
        for l in &lines {
            println!("      {l}");
        }
        remove(&mut srv, &tag)?;
        srv.cmd("kill @e[type=item]")?;
        srv.step(10)?;
        det.push((v, press, release, y12));
    }
    srv.cmd_ok("setblock -4 2 7 minecraft:rail[shape=east_west,waterlogged=false]")?;
    srv.step(5)?;

    // (E) G1 00:59: two FOOT carts 1 or 2 gt apart up the climb (the file's rail at (-4,2,7)), the slowest and fastest
    // entry speeds, the follower's start offset east by 0 / 0.05 / 0.10 / 0.15: the spacing at the leader's SURFACING
    println!("(E) foot pairs: two ridden carts rolled west onto the climb (-3,1,7) from x -2.2 (the follower dx further east) gap gt apart; the spacing at the leader's SURFACING, the least spacing in the shaft, the fates");
    let mut foot: Vec<(f64, u32, f64, Option<f64>, f64, bool)> = Vec::new();
    for v in [0.30, 0.33, 0.36, 0.48] {
        // (G1 00:59) the least foot gap two carts can hold on the climb is their length over the speed (0.98 / v: 3.3 gt
        // at 0.30, 2.04 at 0.48); a 1- or 2-gt follower would spawn inside the leader
        let gaps: Vec<u32> = if v < 0.4 { (4..=14).collect() } else { (3..=14).collect() };
        for gap in gaps {
            for dx in [0.0, 0.05, 0.10, 0.15] {
                let k = foot.len();
                let tags = [format!("lfa{k}"), format!("lfb{k}")];
                let mut alive = [false, false];
                let mut exited = [false, false];
                let mut spacing_at_surf: Option<f64> = None;
                let mut least = f64::MAX;
                let mut lead_surf = false;
                let mut lastp = [[f64::NAN; 3]; 2];
                let mut trace: Vec<String> = Vec::new();
                for t in 0..=260u32 {
                    for (i, (off, x0)) in [(0u32, -2.2), (gap, -2.2 + dx)].iter().enumerate() {
                        if t == *off {
                            srv.cmd_ok(&format!("summon minecraft:minecart {x0:.4} 1.8 7.5 {{Tags:[\"{}\"],Motion:[{:.4}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{}v\"]}}]}}", tags[i], -v, tags[i]))?;
                            alive[i] = true;
                        }
                    }
                    srv.step(1)?;
                    let mut now = [None, None];
                    let mut tl = format!("t{t}");
                    for i in 0..2 {
                        if !alive[i] {
                            continue;
                        }
                        let Some((p, _)) = read(&mut srv, &tags[i])? else { anyhow::bail!("{}: the cart vanished", tags[i]) };
                        now[i] = Some(p);
                        lastp[i] = p;
                        tl.push_str(&format!(" {} ({:.3},{:.3},{:.3})", if i == 0 { "lead" } else { "foll" }, p[0], p[1], p[2]));
                        if past_booster(p) {
                            exited[i] = true;
                            alive[i] = false;
                            remove(&mut srv, &tags[i])?;
                        }
                    }
                    if t <= 45 {
                        trace.push(tl);
                    }
                    if let (Some(a), Some(b)) = (now[0], now[1]) {
                        if a[0] < -4.0 && b[0] < -4.0 {
                            least = least.min(a[1] - b[1]);
                        }
                        if !lead_surf && surfaced(a) {
                            lead_surf = true;
                            spacing_at_surf = Some(a[1] - b[1]);
                        }
                    }
                    if exited[0] && exited[1] {
                        break;
                    }
                }
                let jam = !(exited[0] && exited[1]);
                println!(
                    "  v {v:.2} gap {gap} gt dx {dx:.2}: spacing at the leader's SURFACING {}, least in the shaft {}; {}",
                    spacing_at_surf.map_or("-".into(), |s| format!("{s:.3} b")),
                    if least < f64::MAX { format!("{least:.3} b") } else { "- (never both in the shaft)".into() },
                    if !jam {
                        "both exit".to_string()
                    } else if !exited[1] && lastp[1][0] > -1.5 {
                        format!("THROWN BACK: the follower knocked back east off the climb onto the T (-1,0,7) and off its north arm (a cart off its track), at ({:.3},{:.3},{:.3})", lastp[1][0], lastp[1][1], lastp[1][2])
                    } else if !exited[1] && lastp[1][1] < 3.0 {
                        format!("FOOT STALL: the follower never rises, at ({:.3},{:.3},{:.3})", lastp[1][0], lastp[1][1], lastp[1][2])
                    } else {
                        format!("TOP JAM: leader {:?}, follower {:?}", lastp[0], lastp[1])
                    }
                );
                if k == 0 {
                    for l in &trace {
                        println!("      {l}");
                    }
                }
                for tg in &tags {
                    remove(&mut srv, tg)?;
                }
                srv.cmd("kill @e[type=item]")?;
                srv.step(10)?;
                foot.push((v, gap, dx, spacing_at_surf, least, jam));
            }
        }
    }
    if std::env::var("MCTAS_LIFT_TOP_STOP_AFTER_E").is_ok() {
        anyhow::bail!("stopped after (E) (MCTAS_LIFT_TOP_STOP_AFTER_E): no assert judged");
    }

    // (A) the arrival-z sweep
    println!("(A) the arrival-z sweep: one riser alone from (-4.5, 13.0, z0), 150 gt");
    let mut sweep: Vec<(bool, f64, Fate, f64)> = Vec::new();
    let mut n = 0;
    for ridden in [true, false] {
        for k in 0..=4 {
            let zs = 7.49 + 0.005 * k as f64;
            let tag = format!("la{n}");
            n += 1;
            summon(&mut srv, &tag, 13.0, zs, ridden)?;
            let mut fate = None;
            let mut zmin = f64::MAX;
            let mut last = [0.0; 3];
            let mut top = Vec::new();
            let (mut top_t, mut boost_t) = (None, None);
            for t in 1..=150u32 {
                srv.step(1)?;
                let Some((p, m)) = read(&mut srv, &tag)? else { anyhow::bail!("{tag}: the riser vanished at t{t}") };
                last = p;
                // (cc767b78 02:47) MCTAS_LIFT_TOP_FULL_A=1: every tick of the rise from the summon, with its vertical speed
                let full_a = std::env::var("MCTAS_LIFT_TOP_FULL_A").is_ok();
                if p[1] >= 21.5 || full_a {
                    zmin = zmin.min(p[2]);
                    top.push(if full_a { format!("t{t} y {:.4} z {:.4} vy {:+.4} vz {:+.4}", p[1], p[2], m[1], m[2]) } else { format!("t{t} y {:.3} z {:.4} vz {:+.4}", p[1], p[2], m[2]) });
                }
                if top_t.is_none() && surfaced(p) {
                    top_t = Some(t);
                }
                if boost_t.is_none() && on_booster(p) {
                    boost_t = Some(t);
                }
                if past_booster(p) {
                    fate = Some(Fate::Exit(t));
                    break;
                }
            }
            let fate = fate.unwrap_or(Fate::Stuck(last));
            println!(
                "  {} z0 {zs:.3}: {:?}; SURFACING t{}, ONTO THE BOOSTER t{}, CLEAR OF THE TOP CELL {}; least z at the top {:.4}",
                if ridden { "RIDDEN" } else { "empty " },
                fate,
                top_t.map_or("-".into(), |t: u32| t.to_string()),
                boost_t.map_or("-".into(), |t: u32| t.to_string()),
                match &fate { Fate::Exit(t) => format!("t{t}"), Fate::Stuck(_) => "-".into() },
                zmin
            );
            for l in top.iter().take(14) {
                println!("      {l}");
            }
            remove(&mut srv, &tag)?;
            srv.cmd("kill @e[type=item]")?;
            srv.step(2)?;
            sweep.push((ridden, zs, fate, zmin));
        }
    }

    // (C) two bunched risers (ironfarm 00:15: the lower rider's head in the upper cart's box at the surface; ironspec
    // 00:19 / 00:20: spacings 0.6..1.5 by 0.1 (and 2.0), the upper cart 0.01 north, aligned and 0.01 south of the
    // lower one, both ridden AND an empty cart above a ridden one)
    println!("(C) two risers rising together, the lower one (ridden) s blocks below (upper from (-4.5, 15.0, zu), lower from (-4.5, 15.0 - s, 7.50)), 200 gt");
    let mut pairs: Vec<(bool, f64, f64, Fate, Fate, Option<u32>)> = Vec::new();
    for upper_ridden in [true, false] {
    for zu in [7.49, 7.50, 7.51] {
        for s in [0.60, 0.65, 0.70, 0.75, 0.80, 0.85, 0.90, 0.95, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 2.0] {
            let (tu, tl) = (format!("lcu{}", pairs.len()), format!("lcl{}", pairs.len()));
            summon(&mut srv, &tu, 15.0, zu, upper_ridden)?;
            summon(&mut srv, &tl, 15.0 - s, 7.50, true)?;
            let mut fu: Option<Fate> = None;
            let mut fl: Option<Fate> = None;
            let (mut lu, mut ll) = ([f64::NAN; 3], [f64::NAN; 3]);
            let mut top = Vec::new();
            let mut flip: Option<u32> = None;
            let mut seen_south = false;
            for t in 1..=200u32 {
                srv.step(1)?;
                let mut line = format!("t{t}");
                for (tag, f, last, name) in [(&tu, &mut fu, &mut lu, "upper"), (&tl, &mut fl, &mut ll, "lower")] {
                    if f.is_some() {
                        continue;
                    }
                    let Some((p, m)) = read(&mut srv, tag)? else { anyhow::bail!("{tag}: the riser vanished at t{t}") };
                    *last = p;
                    if p[1] >= 21.0 {
                        line.push_str(&format!(" {name} y {:.3} z {:.4} vz {:+.4}", p[1], p[2], m[2]));
                        if name == "upper" {
                            if m[2] > 0.0 {
                                seen_south = true;
                            } else if m[2] < 0.0 && seen_south && flip.is_none() {
                                flip = Some(t);
                            }
                        }
                    }
                    if past_booster(p) {
                        *f = Some(Fate::Exit(t));
                        remove(&mut srv, tag)?;
                    }
                }
                if line.contains(" y ") {
                    top.push(line);
                }
                if fu.is_some() && fl.is_some() {
                    break;
                }
            }
            let fu = fu.unwrap_or(Fate::Stuck(lu));
            let fl = fl.unwrap_or(Fate::Stuck(ll));
            println!("  {} above, upper z {zu:.2}, spacing {s:.1}: upper {fu:?}, lower {fl:?}; the upper's vz turned north at {}", if upper_ridden { "RIDDEN" } else { "EMPTY " }, flip.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into()));
            for l in top.iter().take(16) {
                println!("      {l}");
            }
            remove(&mut srv, &tu)?;
            remove(&mut srv, &tl)?;
            srv.cmd("kill @e[type=item]")?;
            srv.step(2)?;
            pairs.push((upper_ridden, zu, s, fu, fl, flip));
        }
    }
    }

    // (B) the exit-block control as PAIRS: the jam needs a follower under a held riser, so the block's length is
    // judged against the follower's gap G (riser 2 summoned G gt after riser 1, both from (-4.5, 13.0, 7.50))
    println!("(B) the exit-block pairs: riser 1, its follower G gt later (both ridden, z 7.50), the booster glass from riser 1's top arrival (y >= 22) for N gt, watched 300 gt past the restore; and a lone riser held 100 gt");
    let mut ctl: Vec<(u32, u32, bool, Vec<(usize, Fate)>)> = Vec::new();
    let mut rows_b: Vec<(u32, u32)> = vec![(0, 100)];
    // (ironspec 00:40 / 00:41) the exit OPEN (N 0: no block), follower gaps 4..14 gt
    for g in 1u32..=14 {
        rows_b.push((g, 0));
    }
    for g in [14u32, 20, 28] {
        for nb in [g - 6, g - 4, g - 2, g, g + 2, g + 4, g + 6] {
            rows_b.push((g, nb));
        }
    }
    for (g, nb) in rows_b {
        let offs: Vec<u32> = if g == 0 { vec![0] } else { vec![0, g] };
        let tags: Vec<String> = (0..offs.len()).map(|i| format!("lb{g}n{nb}x{i}")).collect();
        let mut fates: Vec<Option<Fate>> = vec![None; offs.len()];
        let mut alive = vec![false; offs.len()];
        let mut top_t: Vec<Option<u32>> = vec![None; offs.len()];
        let mut boost_t: Vec<Option<u32>> = vec![None; offs.len()];
        let mut least_gap: Option<f64> = None;
        let mut block_from: Option<u32> = None;
        let mut restored_at: Option<u32> = None;
        let mut t = 0u32;
        loop {
            for (i, o) in offs.iter().enumerate() {
                if *o == t {
                    summon(&mut srv, &tags[i], 13.0, 7.5, true)?;
                    alive[i] = true;
                }
            }
            srv.step(1)?;
            t += 1;
            let mut now: Vec<Option<[f64; 3]>> = vec![None; offs.len()];
            for i in 0..offs.len() {
                if !alive[i] {
                    continue;
                }
                let Some((p, _)) = read(&mut srv, &tags[i])? else { anyhow::bail!("{}: the riser vanished at t{t}", tags[i]) };
                now[i] = Some(p);
                if top_t[i].is_none() && surfaced(p) {
                    top_t[i] = Some(t);
                }
                if boost_t[i].is_none() && on_booster(p) {
                    boost_t[i] = Some(t);
                }
                if nb > 0 && i == 0 && block_from.is_none() && p[1] >= 22.0 {
                    block_from = Some(t);
                    srv.cmd_ok(&format!("setblock {} {} {} minecraft:glass", BOOSTER[0], BOOSTER[1], BOOSTER[2]))?;
                }
                if past_booster(p) {
                    fates[i] = Some(Fate::Exit(t));
                    alive[i] = false;
                    remove(&mut srv, &tags[i])?;
                } else {
                    fates[i] = Some(Fate::Stuck(p));
                }
            }
            if let (Some(a), Some(b)) = (now.first().copied().flatten(), now.get(1).copied().flatten()) {
                if a[1] >= 22.95 && a[2] < 8.0 {
                    let gap = a[1] - b[1];
                    least_gap = Some(least_gap.map_or(gap, |l: f64| l.min(gap)));
                }
            }
            if let Some(b) = block_from {
                if restored_at.is_none() && t >= b + nb {
                    srv.cmd_ok(&format!("setblock {} {} {} {BOOSTER_STATE}", BOOSTER[0], BOOSTER[1], BOOSTER[2]))?;
                    restored_at = Some(t);
                }
            }
            let last_summon = *offs.iter().max().unwrap_or(&0);
            if restored_at.is_some_and(|r| t >= r + 300) || (nb == 0 && (alive.iter().all(|a| !a) && t > last_summon || t > last_summon + 300)) || t > 900 {
                break;
            }
        }
        let fates: Vec<(usize, Fate)> = fates.into_iter().enumerate().map(|(i, f)| (i, f.unwrap_or(Fate::Stuck([f64::NAN; 3])))).collect();
        let jam = fates.iter().any(|(_, f)| matches!(f, Fate::Stuck(_)));
        let dwell = |i: usize| format!("SURFACING t{}, ONTO THE BOOSTER t{}, CLEAR OF THE TOP CELL {}", top_t[i].map_or("-".into(), |t| t.to_string()), boost_t[i].map_or("-".into(), |t| t.to_string()), match &fates[i].1 { Fate::Exit(t) => format!("t{t}"), Fate::Stuck(_) => "-".into() });
        // G1 00:45: THE MARGIN = the follower's SURFACING - the leader's CLEAR OF THE TOP CELL
        let margin = match (&fates[0].1, top_t.get(1).copied().flatten()) {
            (Fate::Exit(c), Some(s)) if offs.len() > 1 => format!("{:+} gt", s as i64 - *c as i64),
            _ => "-".into(),
        };
        println!(
            "  G {g:>2} N {nb:>3}: {}{}: {:?}; leader {}{}",
            if nb > 0 { format!("blocked t{} .. t{}; ", block_from.unwrap_or(0), restored_at.unwrap_or(0)) } else { "exit OPEN; ".into() },
            if jam { "JAM" } else { "RECOVERS" },
            fates,
            dwell(0),
            if offs.len() > 1 { format!("; follower {}; MARGIN (follower SURFACING - leader CLEAR) {margin}; least follower clearance while the leader is in the top cell {}", dwell(1), least_gap.map_or("- (never both there)".into(), |l| format!("{l:.3} b"))) } else { String::new() }
        );
        for tg in &tags {
            remove(&mut srv, tg)?;
        }
        srv.cmd("kill @e[type=item]")?;
        srv.cmd_ok(&format!("setblock {} {} {} {BOOSTER_STATE}", BOOSTER[0], BOOSTER[1], BOOSTER[2])).ok();
        srv.step(20)?;
        ctl.push((g, nb, jam, fates));
    }

    let jams_b: Vec<(u32, u32)> = ctl.iter().filter(|c| c.2).map(|c| (c.0, c.1)).collect();
    let jams_c: Vec<(bool, f64, f64)> = pairs.iter().filter(|p| matches!(p.3, Fate::Stuck(_)) || matches!(p.4, Fate::Stuck(_))).map(|p| (p.0, p.1, p.2)).collect();
    println!("  summary: lone risers {} of {} exit; exit-block pairs jammed (G, N) {jams_b:?}; bunched pairs jammed (upper ridden, upper z, spacing) {jams_c:?}", sweep.iter().filter(|s| matches!(s.2, Fate::Exit(_))).count(), sweep.len());
    for g in [14u32, 20, 28] {
        let first = ctl.iter().filter(|c| c.0 == g && c.2).map(|c| c.1).min();
        println!("    G {g}: the shortest jamming block {first:?}");
    }
    pin(1, sweep.iter().all(|s| matches!(s.2, Fate::Exit(_))), format!("(1) every lone riser in the shaft's band (z 7.49..7.51, ridden and empty) EXITS: {:?}", sweep.iter().map(|s| (s.0, s.1, s.2.clone())).collect::<Vec<_>>()))?;
    let jammed = |r: bool, zu: f64, s: f64| jams_c.iter().any(|(rr, z, ss)| *rr == r && (z - zu).abs() < 1e-9 && (ss - s).abs() < 1e-9);
    let first_run = [true, false].iter().all(|r| {
        [7.49, 7.50].iter().all(|zu| jammed(*r, *zu, 0.70) && jammed(*r, *zu, 0.80) && !jammed(*r, *zu, 0.60) && [0.90, 1.0, 1.5, 2.0].iter().all(|s| !jammed(*r, *zu, *s)))
            && [0.60, 0.70, 0.80, 0.90, 1.0, 1.5, 2.0].iter().all(|s| !jammed(*r, 7.51, *s))
    });
    pin(2, first_run, format!("(2) the first run's bunched pairs: upper 0.01 north or aligned JAM at 0.70 and 0.80, not at 0.60 or 0.90 .. 2.0; upper 0.01 south never (ridden and empty above): {jams_c:?}"))?;
    pin(3, ctl.iter().filter(|c| c.0 == 0).all(|c| !c.2), format!("(3) a LONE riser held 100 gt under the glass exits after the restore (no follower, no jam): {:?}", ctl.iter().filter(|c| c.0 == 0).map(|c| c.3.clone()).collect::<Vec<_>>()))?;
    let upper_set = [14u32, 20, 28].iter().all(|g| {
        let mut seen = false;
        ctl.iter().filter(|c| c.0 == *g).all(|c| {
            seen |= c.2;
            c.2 == seen
        })
    });
    pin(4, upper_set, format!("(4) for each follower gap G the jamming blocks are every N from the first jam up (no recovery above a jam): {jams_b:?}"))?;
    pin(5, det.iter().all(|(_, p, _, y)| p.zip(*y).is_some_and(|(a, b)| b > a)), format!("(5) the foot detector presses for every entry speed, before the cart's rise past y 12: {det:?}"))?;
    let foot_jam = |v: f64, g: u32, dx: f64| foot.iter().find(|f| (f.0 - v).abs() < 1e-9 && f.1 == g && (f.2 - dx).abs() < 1e-9).map(|f| f.5);
    let clean_from = |v: f64| if v < 0.315 { 9 } else if v < 0.345 { 8 } else if v < 0.4 { 7 } else { 6 };
    let clean_above = foot.iter().filter(|f| f.1 >= clean_from(f.0)).all(|f| !f.5 && f.3.is_some_and(|s| s >= 1.0));
    pin(6, clean_above && foot_jam(0.48, 3, 0.0) == Some(true) && foot_jam(0.30, 8, 0.0) == Some(true), format!("(6) the first run's foot pairs: every pair at >= 9 gt (0.30) / 8 (0.33) / 7 (0.36) / 6 (0.48) exits with >= 1.0 b at the leader's SURFACING; 0.48 at 3 gt and 0.30 at 8 gt stall the follower at the foot: {:?}", foot.iter().map(|f| (f.0, f.1, f.2, f.5)).collect::<Vec<_>>()))?;
    // (ironspec 03:07) the brief's evidence that normal running is jam-free: every open-exit pair exits; the 1-gt pair
    // rises 0.695 b apart (0.005 under the 0.70 jam band), printed as information
    let open: Vec<(u32, bool, Vec<(usize, Fate)>)> = ctl.iter().filter(|c| c.1 == 0 && (1..=14).contains(&c.0)).map(|c| (c.0, c.2, c.3.clone())).collect();
    println!("  the open-exit pairs (N 0): {:?}; the 1-gt pair rises 0.695 b apart (summoned 1 gt apart at 0.695 b/t), 0.005 under the 0.70 jam band", open.iter().map(|(g, j, f)| (*g, *j, f.len())).collect::<Vec<_>>());
    pin(7, open.len() == 14 && open.iter().all(|(_, jam, fates)| !*jam && fates.len() == 2 && fates.iter().all(|(_, f)| matches!(f, Fate::Exit(_)))), format!("(7) every open-exit pair (follower G = 1..14 gt, N 0) exits, both risers: {open:?}"))?;
    println!("  every known answer held (asserts 1..7)");
    Ok(())
}
