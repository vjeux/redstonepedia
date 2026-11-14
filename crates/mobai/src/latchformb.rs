//! (tickexact 2026-11-13, ironspec 22:55 / 22:58, day-1 23:37) `mctas mobai latch-rig --form-b <cells>`: day-1's P_HOLD
//! latch, FORM (b) (a one-level two-torch NOR latch, no descending dust, every wire and repeater on glass), laid from
//! day-1's standalone block list (a/1902620467569082) on the official 26.3 server at world = local + OFF, beside the
//! as-listed control (`latchrig.rs`). Its named cells (day-1's list): RLS_IN (-3,0,0) and ARM_IN (4,0,-3) (air at rest;
//! the rig sets a redstone block for the width), B2 (0,0,0), QBAR the wall torch (1,0,0), BASE (3,0,0), Q the wall torch
//! (4,0,0), the BRANCH repeater (0,0,-1), the RESET chain (five delay-2 repeaters (-2..2,0,-4)) and the RESET repeater
//! (3,0,-1), the DRIVE repeater (-1,0,3) and its PARK lamp (-1,0,4), the TAP repeater (5,0,0) and its lamp (6,0,0).
//! Every row lays the list (supports first) in RESET, or laid SET (QBAR dark, Q lit). Rows: RLS 1, 2, 3, 4, 8, 20, 24,
//! 30 gt; the branch replaced by glass, RLS 1 / 2 / 3; RLS 2 + ARM 1 / 2 / 3 / 4 / 5 gt at RLS + 8; laid SET held 60
//! gt; laid SET + ARM 1 / 2 / 3 / 4 / 5 gt. Every tick: B2's three input wires, the branch, QBAR, the line into BASE,
//! the reset, Q, the drive, PARK, the tap, TAP. Times are from the RLS command, which lands between ticks (one tick
//! earlier than day-1's start-of-tick frame: its Q on +4 is +3 here). The lamps add their own 4-gt turn-off delay.
//! Asserted (MCTAS_LATCH_RIG_PLANT=k inverts assert k): (1) laid RESET settles RESET; (2) form (b) LATCHES: every RLS
//! width 1..30 lights Q exactly [RLS+3, RLS+23); (3) the TAP repeater = Q + 2 (both edges); (4) the DRIVE lit exactly
//! [RLS+1, RLS+25) for every width <= 20; (5) laid SET holds 60 gt; (6) ARM of 1 or 2 gt does NOT clear Q (a 2-gt reset
//! ends before Q's read); (7) ARM of 3, 4 or 5 gt clears it (at RLS+8: Q off at +11; on a laid SET within 6 gt); (8) no
//! branch: RLS 1 never sets Q, 2 and 3 set it. Built in RESET (Q dark): a latch laid SET by setblock can leave the drive
//! repeater unpowered until its first neighbour update (a placement artifact; the build state is RESET).
//! (tickexact 2026-11-14, 58f770cc 02:01) `--laid <file>`: the same rows and asserts on the latch AS LAID in the re-laid
//! stop (58f770cc's `x,y,z state` list in the stop frame, moved into this rig's frame: rig = stop - (2,-2,-2)), its
//! outputs read where the stop routes them (the drive repeater stop (2,0,2), PARK = the park rail stop (1,0,3) powered
//! through the slime (2,0,3), the tap repeater stop (1,-2,4)); plus (9) the park rail powered exactly [RLS+1, RLS+25)
//! for every width <= 20 (58f770cc's 408-visit band [+2, +26) in the standing frame). Slime is laid with the supports
//! (the laid park rail, ascending east, leans on it and pops if laid first).

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

/// the rig's frame: world = local + OFF (the control's frame)
const OFF: [i32; 3] = [8, 105, 8];

fn w(p: [i32; 3]) -> [i32; 3] {
    [p[0] + OFF[0], p[1] + OFF[1], p[2] + OFF[2]]
}

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    let q = w(p);
    srv.cmd(&format!("setblock {} {} {} {state}", q[0], q[1], q[2]))?;
    Ok(())
}

fn is(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<bool> {
    srv.test_block(w(p), state)
}

fn power(srv: &mut VanillaServer, p: [i32; 3]) -> Result<u8> {
    for k in (1..=15u8).rev() {
        if is(srv, p, &format!("minecraft:redstone_wire[power={k}]"))? {
            return Ok(k);
        }
    }
    Ok(0)
}

/// The list's cells (`x,y,z=state`, a trailing comment allowed; air cells skipped).
pub fn read_cells(path: &std::path::Path) -> Result<Vec<([i32; 3], String)>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        let Some((c, rest)) = l.split_once('=') else { continue };
        let v: Vec<i32> = match c.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<Vec<_>, _>>() {
            Ok(v) if v.len() == 3 => v,
            _ => continue,
        };
        let state = rest.split_whitespace().next().unwrap_or("").to_string();
        if !state.starts_with("minecraft:") || state == "minecraft:air" {
            continue;
        }
        out.push(([v[0], v[1], v[2]], state));
    }
    anyhow::ensure!(!out.is_empty(), "{}: no cells", path.display());
    Ok(out)
}

#[derive(Clone, Copy)]
struct Opts {
    branch: bool,
    set_state: bool,
}

fn lay(srv: &mut VanillaServer, cells: &[([i32; 3], String)], o: Opts) -> Result<()> {
    let a = w([-6, -2, -7]);
    let b = w([9, 2, 7]);
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", a[0], a[1], a[2], b[0], b[1], b[2]))?;
    // supports first: glass, stone, and slime (an ascending rail leans on the block at its high end: the laid park rail
    // (stop (1,0,3) ascending_east) on the slime (2,0,3) pops if laid before it)
    let support = |s: &str| s.contains("glass") || s.contains("stone") || s.contains("slime");
    let mut placed: Vec<([i32; 3], String)> = cells.to_vec();
    for (p, s) in placed.iter_mut() {
        if !o.branch && *p == [0, 0, -1] {
            *s = "minecraft:glass".into();
        }
        // laid SET: QBAR (1,0,0) dark and Q (4,0,0) lit, written whole (a list may carry `lit` already)
        if o.set_state && *p == [1, 0, 0] {
            *s = "minecraft:redstone_wall_torch[facing=east,lit=false]".into();
        }
        if o.set_state && *p == [4, 0, 0] {
            *s = "minecraft:redstone_wall_torch[facing=east,lit=true]".into();
        }
    }
    for (p, s) in placed.iter().filter(|(_, s)| support(s)) {
        set(srv, *p, s)?;
    }
    for (p, s) in placed.iter().filter(|(_, s)| !support(s)) {
        set(srv, *p, s)?;
    }
    srv.step(40)?;
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct Row {
    rls: bool,
    arm: bool,
    b2_in: [u8; 2],
    branch: bool,
    qbar: bool,
    base_in: u8,
    reset: bool,
    q: bool,
    drive: bool,
    park: bool,
    tap: bool,
    tap_lamp: bool,
}

/// (tickexact 2026-11-14, 58f770cc 02:01) `--laid`: the latch as LAID in the re-laid stop, its outputs re-routed: the
/// drive repeater at stop (2,0,2) into the slime (2,0,3) under the park rail (1,0,3) (PARK = that rail powered), the tap
/// repeater at stop (1,-2,4) into the slime (1,-2,5); in this rig's frame (stop (x,y,z) = rig (x+2, y-2, z-2)) those are
/// (0,2,4), (-1,2,5) and (-1,0,6); there is no TAP lamp (its row repeats the tap)
static LAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn read(srv: &mut VanillaServer, rls: bool, arm: bool) -> Result<Row> {
    let laid = LAID.load(std::sync::atomic::Ordering::Relaxed);
    let tap = if laid { is(srv, [-1, 0, 6], "minecraft:repeater[powered=true]")? } else { is(srv, [5, 0, 0], "minecraft:repeater[powered=true]")? };
    Ok(Row {
        rls,
        arm,
        b2_in: [power(srv, [-1, 0, 0])?, power(srv, [0, 0, 1])?],
        branch: is(srv, [0, 0, -1], "minecraft:repeater[powered=true]")?,
        qbar: is(srv, [1, 0, 0], "minecraft:redstone_wall_torch[lit=true]")?,
        base_in: power(srv, [2, 0, 0])?,
        reset: is(srv, [3, 0, -1], "minecraft:repeater[powered=true]")?,
        q: is(srv, [4, 0, 0], "minecraft:redstone_wall_torch[lit=true]")?,
        drive: if laid { is(srv, [0, 2, 4], "minecraft:repeater[powered=true]")? } else { is(srv, [-1, 0, 3], "minecraft:repeater[powered=true]")? },
        park: if laid { is(srv, [-1, 2, 5], "minecraft:powered_rail[powered=true]")? } else { is(srv, [-1, 0, 4], "minecraft:redstone_lamp[lit=true]")? },
        tap,
        tap_lamp: if laid { tap } else { is(srv, [6, 0, 0], "minecraft:redstone_lamp[lit=true]")? },
    })
}

/// `--laid`'s file: `x,y,z state` lines in the stop frame (`#` comments), returned in the rig's frame (x - 2, y + 2, z + 2)
pub fn read_laid_cells(path: &std::path::Path) -> Result<Vec<([i32; 3], String)>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut out: Vec<([i32; 3], String)> = Vec::new();
    // (ironspec 03:29) nothing silently dropped: an unparsable line or a cell listed twice with two states fails; air
    // cells and same-state repeats are counted and printed
    let (mut bad, mut air, mut repeats): (Vec<String>, usize, usize) = (Vec::new(), 0, 0);
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let Some((c, s)) = l.split_once(' ') else {
            bad.push(l.to_string());
            continue;
        };
        let v: Vec<i32> = match c.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<Vec<_>, _>>() {
            Ok(v) if v.len() == 3 => v,
            _ => {
                bad.push(l.to_string());
                continue;
            }
        };
        let s = s.trim().to_string();
        if !s.starts_with("minecraft:") {
            bad.push(l.to_string());
            continue;
        }
        if s == "minecraft:air" {
            air += 1;
            continue;
        }
        let p = [v[0] - 2, v[1] + 2, v[2] + 2];
        match out.iter().find(|(q, _)| *q == p) {
            Some((_, prev)) if *prev == s => repeats += 1,
            Some((_, prev)) => bad.push(format!("{l} (the cell listed before as {prev})")),
            None => out.push((p, s)),
        }
    }
    anyhow::ensure!(bad.is_empty(), "{}: {} line(s) not read as `x,y,z minecraft:state` (or a cell given two states): {bad:?}", path.display(), bad.len());
    println!("  {}: {} cells read ({air} air cells skipped, {repeats} same-state repeats)", path.display(), out.len());
    anyhow::ensure!(!out.is_empty(), "{}: no cells", path.display());
    Ok(out)
}

/// `mctas mobai latch-rig --laid <file>`: the same rows and asserts on the latch as laid in the stop
pub fn latch_rig_laid(version: &str, path: &std::path::Path) -> Result<()> {
    LAID.store(true, std::sync::atomic::Ordering::Relaxed);
    let cells = read_laid_cells(path)?;
    println!("== latch-rig --laid: 58f770cc's latch as LAID ({}, stop frame, moved into the rig's: rig = stop - (2,-2,-2)); outputs read at the drive repeater (0,2,4), the park rail (-1,2,5) and the tap repeater (-1,0,6) in the rig's frame", path.display());
    latch_rig_cells(version, path, cells)
}

const AT: i64 = 10;

struct FRun {
    label: String,
    rows: Vec<Row>,
}

impl FRun {
    fn spans(&self, f: impl Fn(&Row) -> bool) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        let mut start: Option<i64> = None;
        for (i, r) in self.rows.iter().enumerate() {
            let t = i as i64 - AT;
            match (f(r), start) {
                (true, None) => start = Some(t),
                (false, Some(s)) => {
                    out.push((s, t));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push((s, self.rows.len() as i64 - AT));
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn run(srv: &mut VanillaServer, cells: &[([i32; 3], String)], label: &str, o: Opts, rls_w: Option<i64>, arm: Option<(i64, i64)>, ticks: i64) -> Result<FRun> {
    lay(srv, cells, o)?;
    let mut rows = Vec::new();
    let (mut rls, mut arm_on) = (false, false);
    for t in 0..ticks {
        let rel = t - AT;
        if let Some(wd) = rls_w {
            if rel == 0 {
                set(srv, [-3, 0, 0], "minecraft:redstone_block")?;
                rls = true;
            }
            if rel == wd {
                set(srv, [-3, 0, 0], "minecraft:air")?;
                rls = false;
            }
        }
        if let Some((at, aw)) = arm {
            if rel == at {
                set(srv, [4, 0, -3], "minecraft:redstone_block")?;
                arm_on = true;
            }
            if rel == at + aw {
                set(srv, [4, 0, -3], "minecraft:air")?;
                arm_on = false;
            }
        }
        srv.step(1)?;
        rows.push(read(srv, rls, arm_on)?);
    }
    let r = FRun { label: label.to_string(), rows };
    println!("  {label}:");
    for (i, x) in r.rows.iter().enumerate() {
        let t = i as i64 - AT;
        if t < -2 {
            continue;
        }
        println!(
            "    RLS{t:+3}: rls {} arm {} | B2 in {:?} branch {} | Qbar {} base in {} reset {} | Q {} | drive {} PARK {} | tap {} TAP {}",
            x.rls as u8, x.arm as u8, x.b2_in, x.branch as u8, x.qbar as u8, x.base_in, x.reset as u8, x.q as u8, x.drive as u8, x.park as u8, x.tap as u8, x.tap_lamp as u8
        );
    }
    println!("    => Q {:?}; PARK {:?}; TAP {:?}; QBAR dark {:?}", r.spans(|x| x.q), r.spans(|x| x.park), r.spans(|x| x.tap_lamp), r.spans(|x| !x.qbar));
    Ok(r)
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_LATCH_RIG_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn latch_rig_form_b(version: &str, cells_path: &std::path::Path) -> Result<()> {
    let cells = read_cells(cells_path)?;
    latch_rig_cells(version, cells_path, cells)
}

fn latch_rig_cells(version: &str, cells_path: &std::path::Path, cells: Vec<([i32; 3], String)>) -> Result<()> {
    println!("== latch-rig --form-b on the official {version} server: day-1's P_HOLD latch form (b) from {} ({} cells), at world = local + {OFF:?}; times from the RLS command (between ticks: ONE TICK BEFORE the standing start-of-tick frame, where Q = [+4, +24) and the drive [+2, +26))", cells_path.display(), cells.len());
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("forceload add -16 -16 31 31")?;
    srv.step(20)?;
    let base = Opts { branch: true, set_state: false };
    lay(&mut srv, &cells, base)?;
    let settled = read(&mut srv, false, false)?;
    println!("  laid RESET, after 40 gt: {settled:?}");
    if LAID.load(std::sync::atomic::Ordering::Relaxed) {
        let probe = |srv: &mut VanillaServer, p: [i32; 3], states: &[&str]| -> Result<String> {
            for s in states {
                if is(srv, p, s)? {
                    return Ok(s.to_string());
                }
            }
            Ok("none of those".into())
        };
        let park = probe(&mut srv, [-1, 2, 5], &["minecraft:powered_rail[powered=true]", "minecraft:powered_rail[powered=false]", "minecraft:air"])?;
        let slime = probe(&mut srv, [0, 2, 5], &["minecraft:slime_block", "minecraft:air"])?;
        let drive = probe(&mut srv, [0, 2, 4], &["minecraft:repeater[facing=north]", "minecraft:repeater[facing=south]", "minecraft:air"])?;
        println!("  the laid outputs in RESET: the park rail (-1,2,5) {park}; the slime (0,2,5) {slime}; the drive (0,2,4) {drive}");
    }
    let mut widths = Vec::new();
    for wd in [1i64, 2, 3, 4, 8, 20, 24, 30] {
        widths.push((wd, run(&mut srv, &cells, &format!("RLS {wd} gt"), base, Some(wd), None, 70)?));
    }
    let mut nob = Vec::new();
    for wd in [1i64, 2, 3] {
        nob.push((wd, run(&mut srv, &cells, &format!("RLS {wd} gt, NO branch (glass)"), Opts { branch: false, ..base }, Some(wd), None, 60)?));
    }
    let mut arms = Vec::new();
    for aw in [1i64, 2, 3, 4, 5] {
        arms.push((aw, run(&mut srv, &cells, &format!("RLS 2 gt + ARM {aw} gt at RLS+8"), base, Some(2), Some((8, aw)), 70)?));
    }
    let set_hold = run(&mut srv, &cells, "laid SET, no input, 60 gt", Opts { set_state: true, ..base }, None, None, 70)?;
    let mut set_arms = Vec::new();
    for aw in [1i64, 2, 3, 4, 5] {
        set_arms.push((aw, run(&mut srv, &cells, &format!("laid SET, ARM {aw} gt at +0"), Opts { set_state: true, ..base }, None, Some((0, aw)), 50)?));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  summary (spans from the RLS command; the lamps add their 4-gt turn-off delay):");
    for (wd, r) in widths.iter().chain(nob.iter()) {
        println!("    {} (w {wd}): Q {:?} drive {:?} PARK lamp {:?} tap {:?} TAP lamp {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.drive), r.spans(|x| x.park), r.spans(|x| x.tap), r.spans(|x| x.tap_lamp));
    }
    for (aw, r) in &arms {
        println!("    {} (ARM {aw}): Q {:?} drive {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.drive));
    }
    for r in std::iter::once(&set_hold).chain(set_arms.iter().map(|(_, r)| r)) {
        println!("    {}: Q {:?} drive {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.drive));
    }
    pin(1, settled.qbar && !settled.q && !settled.park && !settled.tap_lamp, format!("(1) laid RESET it settles RESET: {settled:?}"))?;
    let qs: Vec<(i64, Vec<(i64, i64)>)> = widths.iter().map(|(wd, r)| (*wd, r.spans(|x| x.q))).collect();
    pin(2, qs.iter().all(|(_, s)| *s == vec![(3, 23)]), format!("(2) form (b) LATCHES: every RLS width 1..30 lights Q exactly [RLS+3, RLS+23) (set by the branch, held by the feedback, reset by the 20-gt chain): {qs:?}"))?;
    let shifted = |s: Vec<(i64, i64)>, d: i64| s.into_iter().map(|(a, b)| (a + d, b + d)).collect::<Vec<_>>();
    pin(3, widths.iter().all(|(_, r)| r.spans(|x| x.tap) == shifted(r.spans(|x| x.q), 2)), format!("(3) the TAP repeater = Q + 2 (both edges): {:?}", widths.iter().map(|(wd, r)| (*wd, r.spans(|x| x.tap))).collect::<Vec<_>>()))?;
    pin(4, widths.iter().filter(|(wd, _)| *wd <= 20).all(|(_, r)| r.spans(|x| x.drive) == vec![(1, 25)]), format!("(4) the DRIVE (PARK's repeater) lit exactly [RLS+1, RLS+25) for every RLS width <= 20 (24 gt; the lamp adds 4 gt): {:?}", widths.iter().map(|(wd, r)| (*wd, r.spans(|x| x.drive))).collect::<Vec<_>>()))?;
    pin(5, set_hold.spans(|x| x.q) == vec![(-AT, 70 - AT)], format!("(5) laid SET holds 60 gt with no input: Q {:?}", set_hold.spans(|x| x.q)))?;
    let arm_q: Vec<(i64, Vec<(i64, i64)>)> = arms.iter().map(|(aw, r)| (*aw, r.spans(|x| x.q))).chain(set_arms.iter().map(|(aw, r)| (100 + *aw, r.spans(|x| x.q)))).collect();
    pin(6, arms.iter().filter(|(aw, _)| *aw <= 2).all(|(_, r)| r.spans(|x| x.q) == vec![(3, 23)]) && set_arms.iter().filter(|(aw, _)| *aw <= 2).all(|(_, r)| r.spans(|x| x.q) == vec![(-AT, 50 - AT)]), format!("(6) ARM of 1 or 2 gt does NOT clear Q (a 2-gt reset ends before Q's read): {arm_q:?}"))?;
    pin(7, arms.iter().filter(|(aw, _)| *aw >= 3).all(|(_, r)| r.spans(|x| x.q) == vec![(3, 11)]) && set_arms.iter().filter(|(aw, _)| *aw >= 3).all(|(_, r)| r.spans(|x| x.q).first().is_some_and(|(a, b)| *a == -AT && *b > 0 && *b <= 6)), format!("(7) ARM of 3, 4 or 5 gt clears Q (at RLS+8: Q off at +11; on a laid SET within 6 gt): {arm_q:?}"))?;
    let nb: Vec<(i64, Vec<(i64, i64)>)> = nob.iter().map(|(wd, r)| (*wd, r.spans(|x| x.q))).collect();
    pin(8, nb.iter().all(|(wd, s)| if *wd == 1 { s.is_empty() } else { *s == vec![(3, 23)] }), format!("(8) no branch: RLS 1 gt never sets Q, 2 and 3 gt set it as with the branch: {nb:?}"))?;
    if LAID.load(std::sync::atomic::Ordering::Relaxed) {
        // (58f770cc 02:01) the stop's bench measured PARK lit [RLS + 2, RLS + 26) on all 408 visits in the standing frame:
        // [RLS + 1, RLS + 25) here
        pin(9, widths.iter().filter(|(wd, _)| *wd <= 20).all(|(_, r)| r.spans(|x| x.park) == vec![(1, 25)]), format!("(9) --laid: the park rail (stop (1,0,3)) powered exactly [RLS+1, RLS+25) for every RLS width <= 20 (58f770cc's 408-visit band [+2, +26) in the standing frame): {:?}", widths.iter().map(|(wd, r)| (*wd, r.spans(|x| x.park))).collect::<Vec<_>>()))?;
        println!("  every known answer held (asserts 1..9)");
        return Ok(());
    }
    println!("  every known answer held (asserts 1..8)");
    Ok(())
}
