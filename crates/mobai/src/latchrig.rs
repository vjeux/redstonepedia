//! (tickexact 2026-11-13, ironspec 21:42 / 22:07, G1 21:25 (2)) `mctas mobai latch-rig`: (g)'s feedback latch alone, laid
//! from 58f770cc's block list (22:07, the stop frame, 26.3's convention: a repeater's `facing` is its INPUT side) on the
//! official 26.3 server, in its stored RESET state (Q dark, Q-bar lit). Added only: glass under the park rail (1,0,4) (an
//! ascending rail needs a sturdy block below; the list leaves (1,-1,4) to the stop) and the command drivers: RLS = a
//! redstone block at the stand-in (0,-4,4) (in place of its dust) and at (-1,-2,2) (the 8-gt pair's input) together for
//! `w` gt; ARM = a redstone block at (4,-2,2) (ARM's diode input) for its width. Every row re-lays the latch first. Rows:
//! RLS widths 1, 2, 3, 4, 8, 20 with the branch (2,-4,2) as listed and the pair as listed (2 x delay 2 = 8 gt); the pair
//! sized (2 x delay 4 = 16 gt; ironspec 22:07: PARK >= 14, ~16 sized); the branch replaced by glass (the control); ARM
//! pulses of 2 and 4 gt and a held ARM at RLS + 2 / + 6 / - 6; the latch laid SET (Q lit, Q-bar dark) held, then cleared
//! by ARM, or by an RLS pulse. Every tick: RLS, B2's three inputs (the stand-in's dust (1,-4,4), the branch's dust
//! (2,-4,3), the feedback's last dust (3,-4,4)), Q-bar, Q, the park rail, the consumers' tap, the branch, the reset, the
//! pair, ARM's diode. The as-listed latch is the CONTROL (ironspec 22:58): its feedback (Q (2,-1,4) -> dust (3,-1,4) ->
//! (4,-2,4) -> (5,-3,4) -> (4,-4,4) -> (3,-4,4) -> B2's east face) descends a GLASS staircase, and 26.3's wire reads a
//! diagonal-above wire only when the neighbour cell is a conductor (RedstoneWireEvaluator.getIncomingWireSignal 29-47),
//! so the feedback dies at its first down-step and the latch never holds: Q follows B2's other two inputs (the
//! stand-in's dust and the branch). Asserted: (1) laid RESET settles RESET; (2) in every row the tap never powers and
//! B2's east input reads 0; (3) branch as listed, pair 8: Q and the park lit exactly [RLS+3, min(max(w, 2) + 5, 11));
//! (4) pair 16: [RLS+3, min(max(w, 2) + 5, 19)); (5) no branch: w = 1 never, w >= 2 [RLS+3, RLS+w+3); (6) laid SET
//! falls to RESET before the row starts; (7) the ARM rows and SET + RLS light Q exactly [RLS+3, RLS+7) (ARM finds Q
//! off). (Commands land between ticks, so Q lights at RLS+3 in this frame.) MCTAS_LATCH_RIG_PLANT=<k> inverts assert k.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

/// 58f770cc's block list (22:07), verbatim in its frame
const CELLS: &str = "
2,-4,4=minecraft:stone
2,-3,4=minecraft:redstone_torch[lit=true]
2,-2,4=minecraft:stone
2,-1,4=minecraft:redstone_torch[lit=false]
2,0,4=minecraft:slime_block
1,0,4=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]
3,-2,4=minecraft:glass
3,-1,4=minecraft:redstone_wire
4,-3,4=minecraft:glass
4,-2,4=minecraft:redstone_wire
5,-4,4=minecraft:glass
5,-3,4=minecraft:redstone_wire
4,-5,4=minecraft:glass
4,-4,4=minecraft:redstone_wire
3,-5,4=minecraft:glass
3,-4,4=minecraft:redstone_wire
5,-4,3=minecraft:glass
5,-3,3=minecraft:repeater[delay=1,facing=south,locked=false,powered=false]
0,-5,4=minecraft:glass
0,-4,4=minecraft:redstone_wire
1,-5,4=minecraft:glass
1,-4,4=minecraft:redstone_wire
0,-5,3=minecraft:glass
0,-4,3=minecraft:redstone_wire
0,-5,2=minecraft:glass
0,-4,2=minecraft:redstone_wire
0,-5,1=minecraft:glass
0,-4,1=minecraft:redstone_wire
1,-5,1=minecraft:glass
1,-4,1=minecraft:redstone_wire
2,-5,1=minecraft:glass
2,-4,1=minecraft:redstone_wire
2,-5,2=minecraft:glass
2,-4,2=minecraft:repeater[delay=1,facing=north,locked=false,powered=false]
2,-5,3=minecraft:glass
2,-4,3=minecraft:redstone_wire
2,-3,3=minecraft:glass
2,-2,3=minecraft:repeater[delay=1,facing=north,locked=false,powered=false]
2,-3,2=minecraft:glass
2,-2,2=minecraft:redstone_wire
0,-3,2=minecraft:glass
0,-2,2=minecraft:repeater[delay=2,facing=west,locked=false,powered=false]
1,-3,2=minecraft:glass
1,-2,2=minecraft:repeater[delay=2,facing=west,locked=false,powered=false]
3,-3,2=minecraft:glass
3,-2,2=minecraft:repeater[delay=1,facing=east,locked=false,powered=false]
";

/// the rig's frame: world = local + OFF
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

#[derive(Clone, Copy)]
struct Opts {
    /// the pair's repeaters' delay (2 = as listed: 2 x 4 gt)
    pair_delay: u8,
    branch: bool,
    /// laid SET (Q lit, Q-bar dark) instead of RESET
    set_state: bool,
}

/// (tickexact 2026-11-14, cc767b78 01:06: the control's own recording) the as-listed latch's cells for scene
/// `latch-aslisted`: 58f770cc's list (22:07) plus the park rail's support (1,-1,4), every listed state replaced by its
/// settled RESET state where `LATCH_ASLISTED_SETTLED` names one; `branch` false: the branch (2,-4,2) glass
pub(crate) fn latch_aslisted_cells(branch: bool) -> Vec<([i32; 3], String)> {
    let mut v: Vec<([i32; 3], String)> = vec![([1, -1, 4], "minecraft:glass".to_string())];
    for l in CELLS.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some((c, s)) = l.split_once('=') else { continue };
        let p: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let p = [p[0], p[1], p[2]];
        let settled = LATCH_ASLISTED_SETTLED.iter().find(|(q, _)| *q == p).map(|(_, s)| s.to_string()).unwrap_or_else(|| s.to_string());
        v.push((p, if !branch && p == [2, -4, 2] { "minecraft:glass".to_string() } else { settled }));
    }
    if !branch {
        for (p, s) in LATCH_ASLISTED_NOBRANCH {
            if let Some(c) = v.iter_mut().find(|(q, _)| q == p) {
                c.1 = s.to_string();
            }
        }
    }
    v
}

/// The settled RESET states that differ from the list (from `mctas mobai scene-lint latch-aslisted --all`).
pub(crate) const LATCH_ASLISTED_SETTLED: &[([i32; 3], &str)] = &[
    ([0, -4, 1], "minecraft:redstone_wire[east=side,north=none,power=0,south=side,west=none]"),
    ([0, -4, 2], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([0, -4, 3], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([0, -4, 4], "minecraft:redstone_wire[east=side,north=side,power=0,south=none,west=none]"),
    ([1, -4, 1], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([1, -4, 4], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([2, -4, 1], "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]"),
    ([2, -4, 3], "minecraft:redstone_wire[east=none,north=side,power=0,south=side,west=none]"),
    ([2, -2, 2], "minecraft:redstone_wire[east=side,north=none,power=0,south=side,west=side]"),
    ([3, -4, 4], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([3, -1, 4], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([4, -4, 4], "minecraft:redstone_wire[east=up,north=none,power=0,south=none,west=side]"),
    ([4, -2, 4], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=up]"),
    ([5, -3, 4], "minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=up]"),
];

/// The no-branch copy's other differences (the wires beside the branch's glass).
pub(crate) const LATCH_ASLISTED_NOBRANCH: &[([i32; 3], &str)] = &[
    ([2, -4, 1], "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]"),
    ([2, -4, 3], "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"),
];

fn lay(srv: &mut VanillaServer, o: Opts) -> Result<()> {
    let a = w([-3, -7, -1]);
    let b = w([7, 2, 7]);
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", a[0], a[1], a[2], b[0], b[1], b[2]))?;
    let mut cells: Vec<([i32; 3], String)> = Vec::new();
    for l in CELLS.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (c, s) = l.split_once('=').context("cell")?;
        let v: Vec<i32> = c.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
        let mut s = s.to_string();
        if !o.branch && v == [2, -4, 2] {
            s = "minecraft:glass".into();
        }
        if v == [0, -2, 2] || v == [1, -2, 2] {
            s = s.replace("delay=2", &format!("delay={}", o.pair_delay));
        }
        if o.set_state && v == [2, -3, 4] {
            s = "minecraft:redstone_torch[lit=false]".into();
        }
        if o.set_state && v == [2, -1, 4] {
            s = "minecraft:redstone_torch[lit=true]".into();
        }
        cells.push(([v[0], v[1], v[2]], s));
    }
    // supports (glass, stone) first, then the rest; the park rail's support added
    set(srv, [1, -1, 4], "minecraft:glass")?;
    for (p, s) in cells.iter().filter(|(_, s)| s.contains("glass") || s.contains("stone") || s.contains("slime")) {
        set(srv, *p, s)?;
    }
    for (p, s) in cells.iter().filter(|(_, s)| !(s.contains("glass") || s.contains("stone") || s.contains("slime"))) {
        set(srv, *p, s)?;
    }
    srv.step(30)?;
    Ok(())
}

/// One tick's reads.
#[derive(Clone, Debug, Default)]
struct Row {
    rls: bool,
    b2_in: [u8; 3],
    qbar: bool,
    q: bool,
    park: bool,
    tap: bool,
    branch: bool,
    reset: bool,
    pair: [bool; 2],
    arm: bool,
}

fn read(srv: &mut VanillaServer, rls: bool) -> Result<Row> {
    Ok(Row {
        rls,
        b2_in: [power(srv, [1, -4, 4])?, power(srv, [2, -4, 3])?, power(srv, [3, -4, 4])?],
        qbar: is(srv, [2, -3, 4], "minecraft:redstone_torch[lit=true]")?,
        q: is(srv, [2, -1, 4], "minecraft:redstone_torch[lit=true]")?,
        park: is(srv, [1, 0, 4], "minecraft:powered_rail[powered=true]")?,
        tap: is(srv, [5, -3, 3], "minecraft:repeater[powered=true]")?,
        branch: is(srv, [2, -4, 2], "minecraft:repeater[powered=true]")?,
        reset: is(srv, [2, -2, 3], "minecraft:repeater[powered=true]")?,
        pair: [is(srv, [0, -2, 2], "minecraft:repeater[powered=true]")?, is(srv, [1, -2, 2], "minecraft:repeater[powered=true]")?],
        arm: is(srv, [3, -2, 2], "minecraft:repeater[powered=true]")?,
    })
}

/// A row's drive: RLS for `rls_w` gt from tick `RLS_AT` (None: no RLS); ARM from `RLS_AT + arm.0` for `arm.1` gt.
const RLS_AT: i64 = 10;

struct Run {
    label: String,
    rows: Vec<Row>,
}

impl Run {
    /// the [start, end) ticks (relative to the RLS command) over which `f` held
    fn spans(&self, f: impl Fn(&Row) -> bool) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        let mut start: Option<i64> = None;
        for (i, r) in self.rows.iter().enumerate() {
            let t = i as i64 - RLS_AT;
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
            out.push((s, self.rows.len() as i64 - RLS_AT));
        }
        out
    }
}

fn run(srv: &mut VanillaServer, label: &str, o: Opts, rls_w: Option<i64>, arm: Option<(i64, i64)>, ticks: i64) -> Result<Run> {
    lay(srv, o)?;
    let mut rows = Vec::new();
    let mut rls = false;
    for t in 0..ticks {
        let rel = t - RLS_AT;
        // commands go in before tick t (between t - 1 and t)
        if let Some(wd) = rls_w {
            if rel == 0 {
                set(srv, [0, -4, 4], "minecraft:redstone_block")?;
                set(srv, [-1, -2, 2], "minecraft:redstone_block")?;
                rls = true;
            }
            if rel == wd {
                set(srv, [0, -4, 4], "minecraft:redstone_wire")?;
                set(srv, [-1, -2, 2], "minecraft:air")?;
                rls = false;
            }
        }
        if let Some((at, aw)) = arm {
            if rel == at {
                set(srv, [4, -2, 2], "minecraft:redstone_block")?;
            }
            if aw > 0 && rel == at + aw {
                set(srv, [4, -2, 2], "minecraft:air")?;
            }
        }
        srv.step(1)?;
        let r = read(srv, rls)?;
        rows.push(r);
    }
    let run = Run { label: label.to_string(), rows };
    println!("  {label}:");
    for (i, r) in run.rows.iter().enumerate() {
        let t = i as i64 - RLS_AT;
        if t < -2 && !r.q && r.qbar && arm.is_none_or(|(a, _)| a >= -2) {
            continue;
        }
        println!(
            "    RLS{t:+3}: rls {} | B2 in {:?} | Qbar {} Q {} | park {} tap {} | branch {} reset {} pair {:?} arm {}",
            r.rls as u8, r.b2_in, r.qbar as u8, r.q as u8, r.park as u8, r.tap as u8, r.branch as u8, r.reset as u8, r.pair.map(|b| b as u8), r.arm as u8
        );
    }
    println!("    => Q lit {:?}; park powered {:?}; tap powered {:?}", run.spans(|r| r.q), run.spans(|r| r.park), run.spans(|r| r.tap));
    Ok(run)
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_LATCH_RIG_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn latch_rig(version: &str) -> Result<()> {
    println!("== latch-rig on the official {version} server: (g)'s latch alone from 58f770cc's block list (22:07), at world = local + {OFF:?}; RLS / ARM driven by redstone blocks set by command before the tick named (RLS+0 = the RLS command)");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("forceload add -16 -16 31 31")?;
    srv.step(20)?;
    let base = Opts { pair_delay: 2, branch: true, set_state: false };
    // (1) the stored state settles as listed
    lay(&mut srv, base)?;
    let settled = read(&mut srv, false)?;
    println!("  laid RESET, after 30 gt: {settled:?}");
    let mut r8 = Vec::new();
    for wd in [1i64, 2, 3, 4, 8, 20] {
        r8.push((wd, run(&mut srv, &format!("RLS {wd} gt, branch as listed, pair 8 gt"), base, Some(wd), None, 50)?));
    }
    let mut r16 = Vec::new();
    for wd in [1i64, 2, 4, 20] {
        r16.push((wd, run(&mut srv, &format!("RLS {wd} gt, branch as listed, pair SIZED 16 gt"), Opts { pair_delay: 4, ..base }, Some(wd), None, 50)?));
    }
    let mut rnb = Vec::new();
    for wd in [1i64, 2, 3, 4] {
        rnb.push((wd, run(&mut srv, &format!("RLS {wd} gt, NO branch (glass), pair 8 gt"), Opts { branch: false, ..base }, Some(wd), None, 40)?));
    }
    let mut rarm = Vec::new();
    for (at, aw) in [(2i64, 2i64), (2, 4), (2, 0), (6, 2), (6, 4), (-6, 2)] {
        rarm.push(((at, aw), run(&mut srv, &format!("RLS 2 gt + ARM at RLS{at:+} for {}", if aw == 0 { "good (held)".to_string() } else { format!("{aw} gt") }), base, Some(2), Some((at, aw)), 40)?));
    }
    let set_hold = run(&mut srv, "laid SET, no input, 40 gt", Opts { set_state: true, ..base }, None, None, 40)?;
    let set_arm = run(&mut srv, "laid SET, ARM 4 gt at +0", Opts { set_state: true, ..base }, None, Some((0, 4)), 40)?;
    let set_rls = run(&mut srv, "laid SET, RLS 2 gt", Opts { set_state: true, ..base }, Some(2), None, 40)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  summary (Q lit spans, ticks from the RLS command):");
    for (wd, r) in r8.iter().chain(r16.iter()).chain(rnb.iter()) {
        println!("    {} (w {wd}): Q {:?} park {:?} tap {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.park), r.spans(|x| x.tap));
    }
    for ((at, aw), r) in &rarm {
        println!("    {} (ARM {at:+} / {aw}): Q {:?} park {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.park));
    }
    for r in [&set_hold, &set_arm, &set_rls] {
        println!("    {}: Q {:?} park {:?}", r.label, r.spans(|x| x.q), r.spans(|x| x.park));
    }
    pin(1, settled.qbar && !settled.q && !settled.park && !settled.tap, format!("(1) laid RESET it settles RESET (Q-bar lit, Q dark, park and tap off): {settled:?}"))?;
    // the as-listed latch is the CONTROL (ironspec 22:58): its feedback descends a glass staircase, which 26.3's wire
    // never carries downward (RedstoneWireEvaluator.getIncomingWireSignal 29-47), so it never holds
    let all: Vec<&Run> = r8.iter().map(|(_, r)| r).chain(r16.iter().map(|(_, r)| r)).chain(rnb.iter().map(|(_, r)| r)).chain(rarm.iter().map(|(_, r)| r)).chain([&set_hold, &set_arm, &set_rls]).collect();
    pin(2, all.iter().all(|r| r.rows.iter().all(|x| !x.tap && x.b2_in[2] == 0)), format!("(2) the feedback never carries: in every row the tap never powers and B2's east input (3,-4,4) reads 0 on every tick: {:?}", all.iter().map(|r| (r.label.clone(), r.spans(|x| x.tap), r.rows.iter().map(|x| x.b2_in[2]).max())).filter(|(_, t, m)| !t.is_empty() || *m != Some(0)).collect::<Vec<_>>()))?;
    let follows = |r: &Run, end: i64| r.spans(|x| x.q) == vec![(3, end)] && r.spans(|x| x.park) == vec![(3, end)];
    pin(3, r8.iter().all(|(wd, r)| follows(r, (wd.max(&2) + 5).min(11))), format!("(3) branch as listed, pair 8: Q (and the park) lit exactly [RLS+3, min(max(w, 2) + 5, 11)): {:?}", r8.iter().map(|(wd, r)| (*wd, r.spans(|x| x.q), r.spans(|x| x.park))).collect::<Vec<_>>()))?;
    pin(4, r16.iter().all(|(wd, r)| follows(r, (wd.max(&2) + 5).min(19))), format!("(4) the pair sized to 16 gt: Q (and the park) lit exactly [RLS+3, min(max(w, 2) + 5, 19)): {:?}", r16.iter().map(|(wd, r)| (*wd, r.spans(|x| x.q))).collect::<Vec<_>>()))?;
    pin(5, rnb.iter().all(|(wd, r)| if *wd == 1 { r.spans(|x| x.q).is_empty() } else { follows(r, wd + 3) }), format!("(5) no branch: RLS 1 gt never lights Q; w >= 2 lights it exactly [RLS+3, RLS+w+3): {:?}", rnb.iter().map(|(wd, r)| (*wd, r.spans(|x| x.q))).collect::<Vec<_>>()))?;
    pin(6, set_hold.spans(|x| x.q).is_empty() && set_arm.spans(|x| x.q).is_empty(), format!("(6) laid SET it falls to RESET before the row starts: Q {:?} / {:?}", set_hold.spans(|x| x.q), set_arm.spans(|x| x.q)))?;
    pin(7, rarm.iter().all(|(_, r)| follows(r, 7)) && follows(&set_rls, 7), format!("(7) ARM changes nothing (Q is off by then): every RLS 2 + ARM row and the SET + RLS row light Q exactly [RLS+3, RLS+7): {:?}", rarm.iter().map(|((at, aw), r)| (*at, *aw, r.spans(|x| x.q))).collect::<Vec<_>>()))?;
    println!("  every known answer of the CONTROL held (asserts 1..7): the as-listed latch never holds");
    Ok(())
}
