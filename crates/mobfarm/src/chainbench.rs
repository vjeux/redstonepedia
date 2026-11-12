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
pub fn design(stages: i32) -> Design {
    let mut d = Design { name: format!("pausable-chain-{stages}"), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    for x in 1..=stages {
        d.blocks.insert([x, 1, 0], REP_CHAIN.into());
        d.blocks.insert([x, 0, 0], "minecraft:stone".into());
        d.blocks.insert([x, 1, -1], REP_LOCK.into());
        d.blocks.insert([x, 0, -1], "minecraft:stone".into());
    }
    for x in 0..=stages + 1 {
        d.blocks.insert([x, 1, -2], DUST.into());
        d.blocks.insert([x, 0, -2], "minecraft:stone".into());
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

/// The pulse train (G1 Nov 11 19:43): `pulses` trigger pulses `width` gt long every `every` gt (one
/// per release), a `len`-gt freeze begun at every phase of one period; each stage's rise and fall per
/// pulse against the paused ideal (start + 2(k+1) [+ width for the fall], + len when the freeze began
/// before it), and the waves' integrity (each stage rises and falls once per pulse).
pub fn bench_train(spec: &FarmSpec, stages: i32, len: i64, every: i64, width: i64, pulses: i64) -> Result<Vec<String>, String> {
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
        for t in 0..horizon {
            if t % every == 0 && t / every < pulses {
                run.world.set_block(BlockPos::new(0, 1, 0), on.clone(), redsim::world::flags::ALL);
            }
            if t % every == width && t / every < pulses {
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
    let (mut trig_phases, mut trig_waves, mut trig_dropped) = (0u64, 0u64, 0u64);
    // (the paused ideal of an edge due d live gt after its wave starts at s: the frozen time between s
    // and the edge is added; a freeze wholly before s delays nothing)
    let paused = |s: i64, d: i64, f: i64| -> i64 { s + d + if f < s + d { (f + len - f.max(s)).max(0) } else { 0 } };
    for f in 0..every + 2 * stages as i64 {
        let tr = run_one(Some((f, len)))?;
        // (a trigger pulse inside the freeze is a cycle-start pulse: rule 1 drops it or cuts it; those
        // phases are reported apart: how many stages still carry every wave)
        let trig_in = (0..pulses).any(|p| { let s = base + p * every; s < f + len && s + width > f });
        if trig_in {
            trig_phases += 1;
            for v in &tr {
                if v.len() as i64 == 2 * pulses {
                    trig_waves += 1;
                } else {
                    trig_dropped += 1;
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
    out.push(format!("phases with a trigger pulse inside the freeze (a cycle-start pulse: rule 1 drops it): {trig_phases}; stage-trials still carrying every wave {trig_waves}, missing a wave {trig_dropped}"));
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
    out.push(format!("the pause latency from a freeze: ALIVE's drop (rising edges 3..4 gt apart, the 5-gt stretcher: 1..5 gt after the freeze) + the bus trunk's delay + the lockers' 2 gt; a timed stage due inside it still fires"));
    let ctl = trial(spec, stages, Some((1, 200)), 2 * stages as i64 + 260)?;
    let during = ctl.iter().filter(|r| r.is_some_and(|t| t > 3 && t <= 201)).count();
    let first_after = ctl.iter().filter_map(|r| *r).filter(|t| *t > 201).min();
    out.push(format!("frozen control (the bus driven from t 1 for 200 gt): stages rising during the freeze after the lock took hold (t 4..201): {during}; the first rise after the thaw at t {:?}; the last stage at t {:?}", first_after, ctl.last().copied().flatten()));
    let blocks = 4 * stages + 2 * (stages + 2) + 1;
    out.push(format!("blocks: {blocks} ({stages} chain repeaters, {stages} lockers, {} bus dust, stone under all; plus the trunk feeding {} bus drivers with matched delay)", stages + 2, drivers(stages).len()));
    Ok(out)
}
