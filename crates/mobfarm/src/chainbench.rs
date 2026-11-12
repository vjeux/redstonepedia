//! The pausable timers' shared lockable chain (ironspec Nov 11 18:36, cc767b78's 1-tick rule): the
//! post-commit timers (the stand's fill, the batch receiver) ride one chain of 1-tick repeaters (2 gt a
//! stage) started on the committed release edge, one tap per timer; each chain repeater has a side
//! locker (a 1-tick repeater into its side) fed from one NOT-ALIVE bus, so a freeze holds every stage
//! and a stage whose tick fell while locked runs its 2 gt again: the lag after a thaw is at most one
//! stage. The bench drives the bus as a matched-delay trunk would (redstone blocks set in the same
//! tick at drivers every 29 bus cells, each powering 14 dust either way) and measures every stage.

use mctas::design::Design;
use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::pos::BlockPos;

const REP_CHAIN: &str = "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]";
const REP_LOCK: &str = "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]";
const DUST: &str = "minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]";

/// The chain's design: the trigger cell (0,1,0) (air; the bench sets a redstone block there, the
/// release edge), chain repeaters at (x,1,0) for x 1..=stages (input from -x), lockers at (x,1,-1)
/// (input from the bus at (x,1,-2), output into the chain repeater's north side), the bus's dust at
/// (x,1,-2) for x 0..=stages+1; stone under all.
pub fn design(stages: i32, mid_drivers: bool, terminator: bool) -> Design {
    let mut d = Design { name: format!("pausable-chain-{stages}"), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    for x in 1..=stages {
        d.blocks.insert([x, 1, 0], REP_CHAIN.into());
        d.blocks.insert([x, 0, 0], "minecraft:stone".into());
        d.blocks.insert([x, 1, -1], REP_LOCK.into());
        d.blocks.insert([x, 0, -1], "minecraft:stone".into());
    }
    // (G1 Nov 11 23:41: a dummy repeater after the last tap makes that tap's rise EXTREMELY_HIGH like
    // the interior stages' (a diode in front), so it keeps the wave's full width)
    if terminator {
        d.blocks.insert([stages + 1, 1, 0], REP_CHAIN.into());
        d.blocks.insert([stages + 1, 0, 0], "minecraft:stone".into());
    }
    for x in 0..=stages + 1 {
        d.blocks.insert([x, 1, -2], DUST.into());
        d.blocks.insert([x, 0, -2], "minecraft:stone".into());
    }
    // (the mid-gt bus, ironspec/G1 Nov 11 23:03: a repeater into the bus at each driver, its input
    // (x,1,-4) where `bench_pb` sets a redstone block 2 gt ahead, so the bus's edges land inside a gt
    // at the driver's priority; only for --pb-transit: bench scaffolding, the plain benches build the
    // chain alone)
    if mid_drivers {
        for x in drivers(stages) {
            d.blocks.insert([x, 1, -3], "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]".into());
            d.blocks.insert([x, 0, -3], "minecraft:stone".into());
            d.blocks.insert([x, 0, -4], "minecraft:stone".into());
        }
    }
    d.blocks.insert([0, 0, 0], "minecraft:stone".into());
    d.ports.insert("output".into(), [0, 1, 3]);
    d
}

/// The bus drivers' x (every 29 cells from 8: each powers 14 dust either way).
fn drivers(stages: i32) -> Vec<i32> {
    let mut v = Vec::new();
    let mut x = 8.min(stages + 1);
    loop {
        v.push(x);
        if x + 14 >= stages + 1 {
            break;
        }
        x += 29;
    }
    v
}

/// One trial: the trigger at t0; the bus driven (ALIVE low) over [t0 + f, t0 + f + len) if `freeze`;
/// every stage's rise time after t0 (None: never within the run).
fn trial(spec: &FarmSpec, stages: i32, freeze: Option<(i64, i64)>, ticks: i64) -> Result<Vec<Option<i64>>, String> {
    let ro = RunOpts { ai: false, ..RunOpts::steady(1, ticks as u32 + 20, WalkMode::Random) };
    let mut run = Run::start(spec, &ro)?;
    while run.settle_tick() {}
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let dust = Block::parse(DUST).map_err(|e| e.to_string())?;
    let drv = drivers(stages);
    run.world.set_block(BlockPos::new(0, 1, 0), on.clone(), redsim::world::flags::ALL);
    let mut rise: Vec<Option<i64>> = vec![None; stages as usize];
    for t in 0..ticks {
        if let Some((f, len)) = freeze {
            if t == f {
                for x in &drv {
                    run.world.set_block(BlockPos::new(*x, 1, -2), on.clone(), redsim::world::flags::ALL);
                }
            }
            if t == f + len {
                for x in &drv {
                    run.world.set_block(BlockPos::new(*x, 1, -2), dust.clone(), redsim::world::flags::ALL);
                }
            }
        }
        run.tick();
        for k in 0..stages {
            if rise[k as usize].is_none() && run.world.get(BlockPos::new(k + 1, 1, 0)).to_string().contains("powered=true") {
                rise[k as usize] = Some(t + 1);
            }
        }
    }
    Ok(rise)
}

/// The P/B wave under freezes in transit (ironspec and G1, Nov 11 23:03): one trigger; P set and the
/// chain's input held until stage N rises (P clears, B sets: a 2N-gt wave). Every phase of one
/// `len`-gt freeze while the wave is in transit, and pairs of freezes (the second begun d gt after the
/// first thaws, f1 and d on a `stride` grid); the bus driven between gts (redstone blocks, as before)
/// or mid-gt (repeaters into the bus, their edges inside a gt at the drivers' priority); both P-clear
/// orders: A, stage 1's fall scheduled after stage N+1's rise in the same gt (the bench clears P after
/// the tick stage N rose in: redsim, like the game, advances the time at the tick's start), and B,
/// stage 1's off-tick inserted before that tick (the rise's tick from A's run of the same freezes,
/// which P's clearing cannot move: it is upstream). Each schedule: broken (not every stage rises and
/// falls once) and every tap's width.
pub fn bench_pb(spec: &FarmSpec, stages: i32, len: i64, ns: &[i32], stride: i64, release_stage: i32) -> Result<Vec<String>, String> {
    use redsim::ticks::Priority;
    let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let dust = Block::parse(DUST).map_err(|e| e.to_string())?;
    let drv = drivers(stages);
    let s1 = BlockPos::new(1, 1, 0);
    let all = redsim::world::flags::ALL;
    let stride = stride.max(1) as usize;
    const TRIG: i64 = 4;
    let trial = |n: i32, mid: bool, pre_clear: Option<i64>, freezes: &[(i64, i64)], horizon: i64| -> Result<(Vec<Vec<(i64, bool)>>, Option<i64>), String> {
        let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
        let run_n = Run::start(spec, &ro);
        let mut run = run_n?;
        while run.settle_tick() {}
        // (ironspec Nov 12 00:12: hold-settled (SETTLED) drops when the release tap at stage
        // `release_stage` rises (the holds empty), so the chain's input P AND SETTLED AND NOT B falls
        // at the first of stage N's rise (P clears, B sets) and the release's)
        let n_eff = if release_stage > 0 { n.min(release_stage) } else { n };
        let n_cell = BlockPos::new(n_eff, 1, 0);
        let rep_kind = run.world.get(s1).kind();
        let mut was = vec![false; stages as usize];
        let mut tr: Vec<Vec<(i64, bool)>> = vec![Vec::new(); stages as usize];
        let (mut p_set, mut rose) = (true, None);
        for t in 0..horizon {
            // (the trigger at t = TRIG, so freezes from f = 2 also catch it and the wave's entry: G1
            // Nov 11 23:41)
            if t == TRIG {
                run.world.set_block(BlockPos::new(0, 1, 0), on.clone(), all);
            }
            for &(f, l) in freezes {
                if mid {
                    if t == f - 2 {
                        for x in &drv {
                            run.world.set_block(BlockPos::new(*x, 1, -4), on.clone(), all);
                        }
                    }
                    if t == f + l - 2 {
                        for x in &drv {
                            run.world.set_block(BlockPos::new(*x, 1, -4), air.clone(), all);
                        }
                    }
                } else {
                    if t == f {
                        for x in &drv {
                            run.world.set_block(BlockPos::new(*x, 1, -2), on.clone(), all);
                        }
                    }
                    if t == f + l {
                        for x in &drv {
                            run.world.set_block(BlockPos::new(*x, 1, -2), dust.clone(), all);
                        }
                    }
                }
            }
            if pre_clear == Some(t) {
                run.world.schedule_tick(s1, rep_kind, 3, Priority::ExtremelyHigh);
            }
            run.tick();
            if p_set && run.world.get(n_cell).to_string().contains("powered=true") {
                p_set = false;
                rose = Some(t);
                run.world.set_block(BlockPos::new(0, 1, 0), air.clone(), all);
            }
            for k in 0..stages as usize {
                let now = run.world.get(BlockPos::new(k as i32 + 1, 1, 0)).to_string().contains("powered=true");
                if now != was[k] {
                    tr[k].push((t + 1, now));
                    was[k] = now;
                }
            }
        }
        Ok((tr, rose))
    };
    // (carried: every stage one rise then one fall; its widths)
    let carried = |tr: &[Vec<(i64, bool)>]| -> (bool, Vec<i64>) {
        let ok = tr.iter().all(|v| v.len() == 2 && v[0].1 && !v[1].1);
        (ok, tr.iter().map(|v| if v.len() == 2 { v[1].0 - v[0].0 } else { -1 }).collect())
    };
    let mut out = vec![format!(
        "the P/B wave (P set by one trigger, the chain's input held until stage N rises: a 2N-gt wave) down {stages} stages under {len}-gt freezes in transit; one freeze at every phase, two freezes on a {stride}-gt grid (the second begun d gt after the first thaws); the bus driven between gts or mid-gt (repeaters into the bus); P-clear order A (stage 1's fall after stage N+1's rise in the same gt) and B (before it)"
    )];
    let fmt_least = |v: &[i64]| v.iter().map(|w| if *w == i64::MAX { "-".to_string() } else { w.to_string() }).collect::<Vec<_>>().join(" ");
    for &n in ns {
        let transit = 2 * stages as i64 + 2 * n as i64 + 6;
        for mid in [false, true] {
            // (per order: trials, broken, the first broken schedule, the least width per tap)
            let mut acc: Vec<(u64, u64, Option<String>, Vec<i64>, u64, u64, Option<String>, Vec<i64>)> = (0..2).map(|_| (0, 0, None, vec![i64::MAX; stages as usize], 0, 0, None, vec![i64::MAX; stages as usize])).collect();
            let mut record = |order: usize, two: bool, tr: &[Vec<(i64, bool)>], label: String| {
                let (ok, w) = carried(tr);
                let a = &mut acc[order];
                let (trials, broken, ex, least) = if two { (&mut a.4, &mut a.5, &mut a.6, &mut a.7) } else { (&mut a.0, &mut a.1, &mut a.2, &mut a.3) };
                *trials += 1;
                if !ok {
                    *broken += 1;
                    if ex.is_none() {
                        let bad: Vec<String> = tr.iter().enumerate().filter(|(_, v)| !(v.len() == 2 && v[0].1 && !v[1].1)).take(3).map(|(k, v)| format!("stage {} {} transitions", k + 1, v.len())).collect();
                        *ex = Some(format!("{label}: {}", bad.join(", ")));
                    }
                }
                for (k, wk) in w.iter().enumerate() {
                    if *wk >= 0 {
                        least[k] = least[k].min(*wk);
                    }
                }
            };
            let mut schedules: Vec<(bool, Vec<(i64, i64)>, String)> = Vec::new();
            for f in 2..TRIG + transit {
                schedules.push((false, vec![(f, len)], format!("one freeze at +{f} (the trigger at +{TRIG})")));
            }
            for f1 in (2..TRIG + transit).step_by(stride) {
                for dd in (0..transit).step_by(stride) {
                    schedules.push((true, vec![(f1, len), (f1 + len + dd, len)], format!("freezes at +{f1} and +{} (the trigger at +{TRIG})", f1 + len + dd)));
                }
            }
            for (two, fr, label) in &schedules {
                let horizon = TRIG + transit + fr.iter().map(|(_, l)| *l).sum::<i64>() + transit + 20;
                let (tr_a, rose) = trial(n, mid, None, fr, horizon)?;
                record(0, *two, &tr_a, label.clone());
                if n >= 2 {
                    let (tr_b, _) = trial(n, mid, rose, fr, horizon)?;
                    record(1, *two, &tr_b, label.clone());
                }
            }
            for (order, a) in acc.iter().enumerate() {
                if order == 1 && n < 2 {
                    continue;
                }
                out.push(format!(
                    "N {n} ({}-gt wave{}){}, bus {}, P-clear {}: one freeze ({} phases): broken {}{}; the least width per tap {}; two freezes ({} pairs): broken {}{}; the least width per tap {}",
                    2 * if release_stage > 0 { n.min(release_stage) } else { n },
                    match n { 1 => ": the control that must break under one freeze", 2 => ": the control that must break under two", _ => "" },
                    if release_stage > 0 { format!(", the release (SETTLED's drop) at stage {release_stage}{}", if release_stage < n { ": the control that must break" } else { "" }) } else { String::new() },
                    if mid { "mid-gt (repeaters)" } else { "between gts" },
                    if order == 0 { "A" } else { "B" },
                    a.0,
                    a.1,
                    a.2.as_ref().map_or(String::new(), |e| format!(" (first: {e})")),
                    fmt_least(&a.3),
                    a.4,
                    a.5,
                    a.6.as_ref().map_or(String::new(), |e| format!(" (first: {e})")),
                    fmt_least(&a.7)
                ));
            }
        }
    }
    out.push("scope: the lead (F1 + Q2) uses a portal-ticket loader with no lockers (its chain a plain repeater chain inside the <= 3-gt login window); this guards pearl-loaded designs and the brief's general rule".to_string());
    Ok(out)
}

/// The pulse train (G1 Nov 11 19:43): `pulses` trigger pulses `width` gt long every `every` gt (one
/// per release), a `len`-gt freeze begun at every phase of one period; each stage's rise and fall per
/// pulse against the paused ideal (start + 2(k+1) [+ width for the fall], + len when the freeze began
/// before it), and the waves' integrity (each stage rises and falls once per pulse). `pb`: the brief's
/// latched start (ironspec 20:45, G1 21:35): P set at each trigger and held, the chain's input on
/// while P is set, P cleared when stage 1 rises; so a freeze can only carry a start late (the wave is
/// then stage 1's 2-gt pulse, the nominal width 2).
pub fn bench_train(spec: &FarmSpec, stages: i32, len: i64, every: i64, width: i64, pulses: i64, pb: bool, pb_stage: i32) -> Result<Vec<String>, String> {
    // (P/B: P clears (and B sets) on stage `pb_stage`'s rise: 1 makes a 2-gt wave, which a freeze can
    // break mid-chain (stage k's fall and stage k+1's rise share a gt; the last stage drives no diode
    // and holds behind its locker while its predecessor's fall runs first: G1/ironspec Nov 11 22:48);
    // 2 makes a 4-gt wave, a stage always on between the falling and the rising one)
    let width = if pb { 2 * pb_stage.max(1) as i64 } else { width };
    let pb_cell = BlockPos::new(pb_stage.max(1), 1, 0);
    let horizon = every * pulses + 2 * stages as i64 + len + 40;
    let run_one = |freeze: Option<(i64, i64)>| -> Result<Vec<Vec<(i64, bool)>>, String> {
        let ro = RunOpts { ai: false, ..RunOpts::steady(1, horizon as u32 + 20, WalkMode::Random) };
        let mut run = Run::start(spec, &ro)?;
        while run.settle_tick() {}
        let on = Block::parse("minecraft:redstone_block").map_err(|e| e.to_string())?;
        let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
        let dust = Block::parse(DUST).map_err(|e| e.to_string())?;
        let drv = drivers(stages);
        let mut was = vec![false; stages as usize];
        let mut tr: Vec<Vec<(i64, bool)>> = vec![Vec::new(); stages as usize];
        let mut p_set = false;
        for t in 0..horizon {
            if t % every == 0 && t / every < pulses {
                run.world.set_block(BlockPos::new(0, 1, 0), on.clone(), redsim::world::flags::ALL);
                p_set = true;
            }
            if !pb && t % every == width && t / every < pulses {
                run.world.set_block(BlockPos::new(0, 1, 0), air.clone(), redsim::world::flags::ALL);
            }
            if let Some((f, l)) = freeze {
                if t == f {
                    for x in &drv {
                        run.world.set_block(BlockPos::new(*x, 1, -2), on.clone(), redsim::world::flags::ALL);
                    }
                }
                if t == f + l {
                    for x in &drv {
                        run.world.set_block(BlockPos::new(*x, 1, -2), dust.clone(), redsim::world::flags::ALL);
                    }
                }
            }
            run.tick();
            // (P/B: stage `pb_stage` has risen: the wave entered the chain, P clears)
            if pb && p_set && run.world.get(pb_cell).to_string().contains("powered=true") {
                p_set = false;
                run.world.set_block(BlockPos::new(0, 1, 0), air.clone(), redsim::world::flags::ALL);
            }
            for k in 0..stages as usize {
                let now = run.world.get(BlockPos::new(k as i32 + 1, 1, 0)).to_string().contains("powered=true");
                if now != was[k] {
                    tr[k].push((t + 1, now));
                    was[k] = now;
                }
            }
        }
        Ok(tr)
    };
    let free = run_one(None)?;
    let base = free[0].first().map_or(2, |x| x.0) - 2;
    let mut out = Vec::new();
    let ok_free = free.iter().all(|v| v.len() as i64 == 2 * pulses);
    out.push(format!("pulse train: {pulses} pulses {width} gt long every {every} gt down {stages} stages; unfrozen: every stage rises and falls once per pulse: {}", if ok_free { "yes" } else { "NO" }));
    let (mut rise, mut fall): (std::collections::BTreeMap<i64, u64>, std::collections::BTreeMap<i64, u64>) = Default::default();
    let mut broken = 0u64;
    let mut broken_ex: Option<String> = None;
    let (mut trig_phases, mut trig_dropped, mut trig_carried, mut trig_broken) = (0u64, 0u64, 0u64, 0u64);
    let mut trig_short = 0u64;
    let mut trig_lag: std::collections::BTreeMap<i64, u64> = std::collections::BTreeMap::new();
    let mut trig_width: std::collections::BTreeMap<i64, u64> = std::collections::BTreeMap::new();
    let mut trig_broken_ex: Option<String> = None;
    let mut phase_lines: Vec<String> = Vec::new();
    let mut tap_least: Vec<i64> = vec![i64::MAX; stages.max(0) as usize];
    // (the paused ideal of an edge due d live gt after its wave starts at s: the frozen time between s
    // and the edge is added; a freeze wholly before s delays nothing)
    let paused = |s: i64, d: i64, f: i64| -> i64 { s + d + if f < s + d { (f + len - f.max(s)).max(0) } else { 0 } };
    for f in 0..every + 2 * stages as i64 {
        let tr = run_one(Some((f, len)))?;
        // (a trigger pulse that overlaps the lock's effective window [f + 2, f + len + 2) (the lockers
        // take 2 gt) is a cycle-start pulse cut by the freeze (G1 Nov 11 20:19, ironspec 20:43): each
        // such phase, one line: dropped whole (every stage misses that wave); carried whole (every
        // stage carries it at full width or wider: its rise and fall lag against the paused ideal and
        // its width); carried late and shortened (every stage carries it, some narrower than the
        // trigger's width); broken (some stages carry it, some don't): 0. And each tap's least width)
        let over: Vec<i64> = (0..pulses).filter(|p| { let s = p * every; s < f + len + 2 && s + width > f + 2 }).collect();
        if !over.is_empty() {
            trig_phases += 1;
            let full = tr.iter().filter(|v| v.len() as i64 == 2 * pulses).count();
            let short = tr.iter().filter(|v| v.len() as i64 == 2 * (pulses - over.len() as i64)).count();
            if full == tr.len() {
                let (mut rl, mut rh, mut fl, mut fh, mut wl, mut wh) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN, i64::MAX, i64::MIN);
                for (k, v) in tr.iter().enumerate() {
                    for &p in &over {
                        let s = base + p * every;
                        let d = 2 * (k as i64 + 1);
                        let r = v[2 * p as usize].0 - paused(s, d, f);
                        let fa = v[2 * p as usize + 1].0 - paused(s, d + width, f);
                        let w = v[2 * p as usize + 1].0 - v[2 * p as usize].0;
                        *trig_lag.entry(r).or_default() += 1;
                        *trig_width.entry(w).or_default() += 1;
                        (rl, rh, fl, fh, wl, wh) = (rl.min(r), rh.max(r), fl.min(fa), fh.max(fa), wl.min(w), wh.max(w));
                        if let Some(t) = tap_least.get_mut(k) {
                            *t = (*t).min(w);
                        }
                    }
                }
                if wl < width {
                    trig_short += 1;
                } else {
                    trig_carried += 1;
                }
                phase_lines.push(format!("  freeze at +{f}: carried {} (rise lag {rl}..{rh}, fall lag {fl}..{fh} against the paused ideal; width {wl}..{wh} gt)", if wl < width { "late and shortened" } else { "whole" }));
            } else if short == tr.len() {
                trig_dropped += 1;
                phase_lines.push(format!("  freeze at +{f}: dropped whole"));
            } else {
                trig_broken += 1;
                let e = format!("freeze at +{f}: {full} stages carry every wave, {short} miss the cut one, {} other", tr.len() - full - short);
                phase_lines.push(format!("  {e}: BROKEN"));
                if trig_broken_ex.is_none() {
                    trig_broken_ex = Some(e);
                }
            }
            continue;
        }
        for (k, v) in tr.iter().enumerate() {
            if v.len() as i64 != 2 * pulses {
                broken += 1;
                if broken_ex.is_none() {
                    broken_ex = Some(format!("freeze at +{f}: stage {} has {} transitions (expected {})", k + 1, v.len(), 2 * pulses));
                }
                continue;
            }
            for p in 0..pulses {
                let d = 2 * (k as i64 + 1);
                let s = base + p * every;
                *rise.entry(v[2 * p as usize].0 - paused(s, d, f)).or_default() += 1;
                *fall.entry(v[2 * p as usize + 1].0 - paused(s, d + width, f)).or_default() += 1;
            }
        }
    }
    let fmt = |m: &std::collections::BTreeMap<i64, u64>| m.iter().map(|(l, n)| format!("{l}:{n}")).collect::<Vec<_>>().join(" ");
    out.push(format!("a {len}-gt freeze begun at every phase 0..{} with no trigger inside it: rise lag (lag:count) {}; fall lag {}; stages whose waves broke (not one rise and one fall per pulse) {broken}{}", every + 2 * stages as i64, fmt(&rise), fmt(&fall), broken_ex.map_or(String::new(), |e| format!(" (first: {e})"))));
    out.push(format!("phases whose trigger pulse overlaps the lock's effective window [f + 2, f + len + 2) (a cycle-start pulse cut by the freeze): {trig_phases}: dropped whole {trig_dropped}; carried whole {trig_carried}; carried late and shortened {trig_short} (all carried: rise lag against the paused ideal {}; width {}); broken (some stages carry it, some don't) {trig_broken}{}", fmt(&trig_lag), fmt(&trig_width), trig_broken_ex.map_or(String::new(), |e| format!(" (first: {e})"))));
    out.push(format!("the least carried width at each tap (stage 1..{stages}, gt): {}", tap_least.iter().map(|w| if *w == i64::MAX { "-".to_string() } else { w.to_string() }).collect::<Vec<_>>().join(" ")));
    out.extend(phase_lines);
    Ok(out)
}

/// The bench: the unfrozen chain (every stage's delay against 2 gt a stage), a `len`-gt freeze begun at
/// every phase 0..=2 * stages + 2 (each stage's lag against an ideal pausable timer: 2k + len when the
/// freeze began before 2k, else 2k), and the frozen control (the bus driven from t0 + 1 for 200 gt).
pub fn bench(spec: &FarmSpec, stages: i32, len: i64) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let horizon = 2 * stages as i64 + len + 40;
    let free = trial(spec, stages, None, horizon)?;
    let mut off: Vec<String> = Vec::new();
    for (k, r) in free.iter().enumerate() {
        let ideal = 2 * (k as i64 + 1);
        if *r != Some(ideal) {
            off.push(format!("stage {} at {:?} (ideal {ideal})", k + 1, r));
        }
    }
    out.push(format!("unfrozen: {} stages, the last at t {:?} (ideal {}); stages off the 2-gt step: {}", stages, free.last().copied().flatten(), 2 * stages, if off.is_empty() { "none".to_string() } else { off.iter().take(8).cloned().collect::<Vec<_>>().join("; ") }));
    let base = free[0].unwrap_or(2) - 2;
    let (mut lo, mut hi) = (i64::MAX, i64::MIN);
    let mut hist: std::collections::BTreeMap<i64, u64> = std::collections::BTreeMap::new();
    // (G1 Nov 11 19:43: the lag by how far after the bus rose each stage was due: d = 2(k+1) - f
    // = 0, 1, 2 (the lockers take their own 2 gt to lock), 3..; d <= 0 is a stage already risen)
    let mut by_d: std::collections::BTreeMap<i64, std::collections::BTreeMap<i64, u64>> = std::collections::BTreeMap::new();
    let mut worst: Vec<String> = Vec::new();
    let mut missing = 0u64;
    for f in 0..=(2 * stages as i64 + 2) {
        let r = trial(spec, stages, Some((f, len)), horizon)?;
        for (k, m) in r.iter().enumerate() {
            let ideal = base + 2 * (k as i64 + 1) + if f < 2 * (k as i64 + 1) { len } else { 0 };
            match m {
                Some(m) => {
                    let lag = m - ideal;
                    *hist.entry(lag).or_default() += 1;
                    let d = (2 * (k as i64 + 1) - f).clamp(-1, 4);
                    *by_d.entry(d).or_default().entry(lag).or_default() += 1;
                    if lag > hi {
                        hi = lag;
                        worst.push(format!("freeze at +{f}: stage {} at {m} (ideal {ideal}, lag {lag})", k + 1));
                    }
                    lo = lo.min(lag);
                }
                None => missing += 1,
            }
        }
    }
    let h: Vec<String> = hist.iter().map(|(l, n)| format!("{l}:{n}")).collect();
    out.push(format!("a {len}-gt freeze begun at every phase 0..={} (each stage against an ideal pausable timer): lag {lo}..{hi} gt over {} stage-trials (lag:count {}); stages that never rose {missing}; the worst: {}", 2 * stages + 2, hist.values().sum::<u64>(), h.join(" "), worst.last().cloned().unwrap_or_default()));
    let rows: Vec<String> = by_d.iter().map(|(d, m)| {
        let label = match d { -1 => "risen before the bus (d <= 0)".to_string(), 4 => "due 4+ gt after".to_string(), d => format!("due {d} gt after") };
        format!("{label}: {}", m.iter().map(|(l, n)| format!("{l}:{n}")).collect::<Vec<_>>().join(" "))
    }).collect();
    out.push(format!("the lag by when each stage was due after the bus rose (lag:count; a stage due within the lockers' own 2 gt fires unpaused, lag -{len}): {}", rows.join("; ")));
    out.push(format!("the pause latency from a freeze: ALIVE's drop (as built, ring-bench --alive: the comparator's 2 gt + the observer's 2 gt and its 2-gt pulse, so 2 + S to 6 + S gt after the freeze, S the stretcher's hold; the software ALIVE 1..5) + the bus trunk's delay + the lockers' 2 gt; a timed stage due inside it still fires"));
    let ctl = trial(spec, stages, Some((1, 200)), 2 * stages as i64 + 260)?;
    let during = ctl.iter().filter(|r| r.is_some_and(|t| t > 3 && t <= 201)).count();
    let first_after = ctl.iter().filter_map(|r| *r).filter(|t| *t > 201).min();
    out.push(format!("frozen control (the bus driven from t 1 for 200 gt): stages rising during the freeze after the lock took hold (t 4..201): {during}; the first rise after the thaw at t {:?}; the last stage at t {:?}", first_after, ctl.last().copied().flatten()));
    let blocks = 4 * stages + 2 * (stages + 2) + 1;
    out.push(format!("blocks: {blocks} ({stages} chain repeaters, {stages} lockers, {} bus dust, stone under all; plus the trunk feeding {} bus drivers with matched delay)", stages + 2, drivers(stages).len()));
    Ok(out)
}
