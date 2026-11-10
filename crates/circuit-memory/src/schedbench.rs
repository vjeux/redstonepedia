//! The nightly scheduler's bench (day-1 lane 2026-11-10; brief part 3; ironspec's contract):
//! `mctas circuit memory sched-bench <design>`. The scheduler design runs on redsim with the
//! counter driven ideal: its 7 lines (`line0..line6`, direct-driven input cells: a redstone block
//! while high) follow a farm on the 100-gt slot grid (golem advances at slot + 21..26, missed slots
//! with --miss/--burst, the counter's fallback at the previous advance + 106..108), each new line
//! 7 gt after the old one fell; the night window `win` is high inside [--dusk, --end] of day time.
//! A visit is read from the design: `v` rising names (the lane whose `p<b>` is high, the level whose
//! `l<k>` is high); the villager naps --nap gt later (LAST_SLEPT); every slot's batch must have all
//! three villagers willing (gameTime - LAST_SLEPT < 24,000). It prints, per villager, its naps a
//! night, its nap day-time range and worst gap window, and the totals: slots lost to an unwilling
//! villager (target 0), second naps, villager-nights without a nap, visits whose lane is not the
//! batch that just left.

use crate::seqbench::Rng;
use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use std::path::Path;

const DAY: u64 = 24_000;
const CYCLE: [usize; 7] = [0, 2, 4, 6, 5, 3, 1];

#[derive(Clone, Debug)]
pub struct Opts {
    pub days: u32,
    pub seed: u64,
    pub miss: f64,
    pub burst: f64,
    pub dusk: u64,
    pub end: u64,
    pub nap: (u32, u32),
    pub ready: u64,
    /// Days of the run replayed in the real game (0: none).
    pub game_days: u32,
    /// Print every change of the lines and the outputs over these ticks (debugging).
    pub watch: Option<(u32, u32)>,
    /// Extra cells whose signal --watch prints.
    pub probes: Vec<[i32; 3]>,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { days: 60, seed: 1, miss: 0.01, burst: 0.3, dusk: 13_670, end: 22_330, nap: (115, 135), ready: 40, game_days: 0, watch: None, probes: Vec::new() }
    }
}

pub fn bench(design: &Path, o: &Opts) -> Result<bool> {
    let mut recorded = Vec::new();
    let ok = bench_rec(design, o, &mut recorded)?;
    if o.game_days > 0 {
        return Ok(game_check(design, o, &recorded)? && ok);
    }
    Ok(ok)
}

/// The real game's check: the bench's own input actions (each line's and the window's cell set to
/// a redstone block or stone at its tick) for the first --game days, replayed on redsim and in the
/// game; every output probe's signal compared at every tick.
fn game_check(design: &Path, o: &Opts, recorded: &[(u32, clock::Action)]) -> Result<bool> {
    let c = Circuit::load(design)?;
    let ticks = (o.game_days as u64 * DAY) as u32;
    let actions: Vec<(u32, clock::Action)> = recorded.iter().filter(|(t, _)| *t <= ticks).cloned().collect();
    let names: Vec<String> = (0..7).map(|b| format!("output.p{b}")).chain((0..3).map(|k| format!("output.l{k}"))).chain(["output.v".to_string()]).collect();
    let probes: Vec<[i32; 3]> = names.iter().map(|n| c.clock.design.ports.get(n).copied().ok_or_else(|| anyhow!("no port {n}"))).collect::<Result<_>>()?;
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    // redsim: frame t after t ticks, the actions at t applied
    let mut sim: Vec<Vec<u8>> = Vec::with_capacity(ticks as usize + 1);
    let bp: Vec<redsim::pos::BlockPos> = probes.iter().map(|p| redsim::pos::BlockPos::new(p[0], p[1], p[2])).collect();
    let r = clock::sim::drive_live(&cc.clock, &actions, ticks, from, to, |_, w| {
        sim.push(bp.iter().map(|p| w.best_neighbor_signal(*p).clamp(0, 15) as u8).collect());
        true
    });
    match r {
        Err((_, why)) => bail!("redsim replay failed: {why}"),
        Ok(Some((t, why))) if !why.is_empty() => bail!("redsim replay stopped at {t}: {why}"),
        _ => {}
    }
    // the game
    let pool = circuit::verify::spawn_pool(&circuit::verify::Options { workers: 1, positions: 1, rotations: false, ..Default::default() })?;
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let mut acts: Vec<(u32, clock::Action)> = cc.clock.actions();
    acts.extend(actions.iter().cloned());
    acts.sort_by_key(|(t, _)| *t);
    let game = pool.map(vec![()], |w, _, _| -> Result<Vec<Vec<u8>>> {
        let sb = w.create(&mctas::sim::SandboxOptions { chunks, seed: 0 })?;
        clock::verify::place(w, sb, &cc.clock)?;
        let mut out: Vec<Vec<u8>> = Vec::with_capacity(ticks as usize + 1);
        let mut t = 0u32;
        let mut k = 0usize;
        loop {
            while k < acts.len() && acts[k].0 == t {
                clock::verify::apply(w, sb, &acts[k].1)?;
                k += 1;
            }
            if out.len() as u32 == t {
                out.push(w.signals(sb, &probes)?.into_iter().map(|(b, _)| b).collect());
            }
            if t >= ticks {
                break;
            }
            let next = acts.get(k).map(|a| a.0).unwrap_or(ticks).min(ticks).min(t + 2000);
            let st = w.step_probe(sb, next - t, false, &probes, None)?;
            for v in &st.signals {
                out.push(v.iter().step_by(2).copied().collect());
            }
            t = next;
        }
        let _ = w.drop_sandbox(sb);
        Ok(out)
    });
    let game = game.into_iter().next().ok_or_else(|| anyhow!("no game run"))??;
    // frame t of the game after an action tick is read after the actions; the step's frames are
    // before the next actions: compare probes only (inputs are not probes)
    let n = sim.len().min(game.len());
    for t in 1..n {
        if sim[t] != game[t] {
            let which: Vec<String> = (0..names.len()).filter(|i| sim[t][*i] != game[t][*i]).map(|i| format!("{} redsim {} game {}", names[i], sim[t][i], game[t][i])).collect();
            println!("DIVERGES at tick {t}: {}", which.join(", "));
            return Ok(false);
        }
    }
    println!("IDENTICAL: {} output probes x {n} ticks ({} days, {} input actions), redsim = the game", names.len(), o.game_days, actions.len());
    Ok(true)
}

fn bench_rec(design: &Path, o: &Opts, recorded: &mut Vec<(u32, clock::Action)>) -> Result<bool> {
    let c = Circuit::load(design)?;
    let find = |name: &str| -> Result<redsim::pos::BlockPos> {
        let p = c.clock.design.ports.get(name).ok_or_else(|| anyhow!("{}: no port {name}", design.display()))?;
        Ok(redsim::pos::BlockPos::new(p[0], p[1], p[2]))
    };
    let lines: Vec<redsim::pos::BlockPos> = (0..7).map(|b| find(&format!("input.line{b}"))).collect::<Result<_>>()?;
    let win = find("input.win")?;
    let pp: Vec<redsim::pos::BlockPos> = (0..7).map(|b| find(&format!("output.p{b}"))).collect::<Result<_>>()?;
    let ll: Vec<redsim::pos::BlockPos> = (0..3).map(|k| find(&format!("output.l{k}"))).collect::<Result<_>>()?;
    let vv = find("output.v")?;
    let step_probe = c.clock.design.ports.get("output.step").map(|p| redsim::pos::BlockPos::new(p[0], p[1], p[2]));
    let mut watched = String::new();
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    let on = redsim::Block::parse("minecraft:redstone_block").map_err(|e| anyhow!(e))?;
    // an input cell holds stone while low (a torch may hang on it), a redstone block while high
    let off = redsim::Block::parse("minecraft:stone").map_err(|e| anyhow!(e))?;
    let mut rng = Rng(o.seed);
    let miss_count = std::cell::Cell::new(0u32);
    // the farm, gameTime = t + t0 (start at noon of day 0)
    let t0: u64 = 6_000;
    let ticks = (o.days as u64 * DAY) as u32;
    let mut cur = 0usize; // index into CYCLE: the batch selected by the last advance
    let mut last_adv: u64 = t0;
    let mut next_adv: u64 = 0; // set below
    let mut missed_last = false;
    let mut want = [false; 7];
    let mut rise_at: Option<(usize, u64)> = None;
    let mut last_slept = [Some(0u64); 21];
    let mut naps: Vec<Vec<u64>> = vec![Vec::new(); 21];
    let mut pending_naps: Vec<(u64, usize)> = Vec::new();
    let (mut attempts, mut lost, mut wrong_lane, mut visits) = (0u32, 0u32, 0u32, 0u32);
    let mut lost_first: Option<u64> = None;
    let mut v_was = false;
    // the visit armed (lane, level, the dispatch tick once its line falls); the hump window's
    // checks: no gap at the dispatch, >= 100 gt past it, a dispatch inside every window
    let mut pending: Option<(usize, usize, Option<u64>)> = None;
    let (mut gaps, mut short, mut nodispatch) = (0u32, 0u32, 0u32);
    let mut window = (u64::MAX, 0u64);
    let mut last_fell: Option<(usize, u64)> = None;
    // schedule the first advance: the batch at position 0 is "selected" at t0 (its line up)
    want[CYCLE[0]] = true;
    let mut plan = |last_adv: u64, cur: usize, rng: &mut Rng, missed_last: &mut bool, last_slept: &[Option<u64>; 21], attempts: &mut u32, lost: &mut u32, lost_first: &mut Option<u64>| -> u64 {
        let next_slot = (last_adv + o.ready).div_ceil(100) * 100;
        let fall = last_adv + 100 + rng.range(6, 8) as u64;
        let rise = next_slot + rng.range(0, 5) as u64;
        if rise + 21 < fall {
            *attempts += 1;
            let lane = CYCLE[cur];
            let willing = (0..3).all(|k| last_slept[3 * lane + k].is_some_and(|s| next_slot - s < DAY));
            if !willing {
                *lost += 1;
                lost_first.get_or_insert(next_slot);
            }
            let miss = rng.unit() < if *missed_last { o.burst.max(o.miss) } else { o.miss };
            if miss {
                miss_count.set(miss_count.get() + 1);
            }
            *missed_last = miss;
            if willing && !miss {
                return rise + 21;
            }
        }
        fall
    };
    next_adv = plan(last_adv, cur, &mut rng, &mut missed_last, &last_slept, &mut attempts, &mut lost, &mut lost_first);
    let res = clock::sim::drive_live(&cc.clock, &[], ticks, from, to, |t, w| {
        let g = t as u64 + t0;
        // naps land
        pending_naps.retain(|&(at, v)| {
            if at == g {
                naps[v].push(g);
                last_slept[v] = Some(g);
                false
            } else {
                true
            }
        });
        // the counter
        if g == next_adv {
            let lane = CYCLE[cur];
            want[lane] = false;
            last_fell = Some((lane, g));
            // the visit armed on this lane dispatches now: its villager naps --nap later, and the
            // window must already be up (no gap at the line's fall)
            if let Some((b, k, None)) = pending {
                if b == lane {
                    pending = Some((b, k, Some(g)));
                    pending_naps.push((g + rng.range(o.nap.0, o.nap.1) as u64, 3 * b + k));
                    if !v_was {
                        gaps += 1;
                    }
                }
            }
            cur = (cur + 1) % 7;
            rise_at = Some((CYCLE[cur], g + 7));
            last_adv = g;
            next_adv = plan(last_adv, cur, &mut rng, &mut missed_last, &last_slept, &mut attempts, &mut lost, &mut lost_first);
        }
        if let Some((lane, at)) = rise_at {
            if at == g {
                want[lane] = true;
                rise_at = None;
            }
        }
        let dt = g % DAY;
        let win_on = dt >= o.dusk && dt <= o.end;
        for (b, p) in lines.iter().enumerate() {
            let is_on = matches!(w.get(*p), redsim::Block::RedstoneBlock);
            if want[b] != is_on {
                w.set_block(*p, if want[b] { on } else { off }, redsim::world::flags::ALL);
                recorded.push((t, clock::Action::Place { pos: [p.x, p.y, p.z], state: if want[b] { "minecraft:redstone_block".into() } else { "minecraft:stone".into() }, nbt: None }));
            }
        }
        let win_is = matches!(w.get(win), redsim::Block::RedstoneBlock);
        if win_on != win_is {
            w.set_block(win, if win_on { on } else { off }, redsim::world::flags::ALL);
            recorded.push((t, clock::Action::Place { pos: [win.x, win.y, win.z], state: if win_on { "minecraft:redstone_block".into() } else { "minecraft:stone".into() }, nbt: None }));
        }
        // a visit: `v` rising names (b, k); the window (`v` high) must cover the line's fall and
        // last >= 100 gt past it
        let v = w.best_neighbor_signal(vv) > 0;
        if v && !v_was {
            let lanes: Vec<usize> = (0..7).filter(|b| w.best_neighbor_signal(pp[*b]) > 0).collect();
            let levels: Vec<usize> = (0..3).filter(|k| w.best_neighbor_signal(ll[*k]) > 0).collect();
            if lanes.len() == 1 && levels.len() == 1 && pending.is_none() {
                let (b, k) = (lanes[0], levels[0]);
                visits += 1;
                if want[b] {
                    pending = Some((b, k, None));
                } else if let Some((l, at)) = last_fell.filter(|(l, at)| *l == b && g <= at + 60) {
                    // the window opened after the line fell (a gap at the dispatch)
                    pending = Some((l, k, Some(at)));
                    pending_naps.push((at + rng.range(o.nap.0, o.nap.1) as u64, 3 * b + k));
                    gaps += 1;
                } else {
                    wrong_lane += 1;
                }
            } else {
                wrong_lane += 1;
            }
        }
        if !v && v_was {
            match pending.take() {
                Some((_, _, Some(at))) => {
                    let m = g - at;
                    window = (window.0.min(m), window.1.max(m));
                    if m < 100 {
                        short += 1;
                    }
                }
                Some((_, _, None)) => nodispatch += 1,
                None => {}
            }
        }
        v_was = v;
        if let Some((a, z)) = o.watch {
            if t >= a && t <= z {
                let bit = |p: redsim::pos::BlockPos| if w.best_neighbor_signal(p) > 0 { '1' } else { '.' };
                let s: String = format!(
                    "lines {} win {} p {} l {} v {} step {} probes {:?}",
                    (0..7).map(|b| if want[b] { char::from(b'0' + b as u8) } else { '.' }).collect::<String>(),
                    if win_on { 1 } else { 0 },
                    pp.iter().map(|p| bit(*p)).collect::<String>(),
                    ll.iter().map(|p| bit(*p)).collect::<String>(),
                    if v { 1 } else { 0 },
                    step_probe.map(bit).unwrap_or('?'),
                    o.probes.iter().map(|q| w.best_neighbor_signal(redsim::pos::BlockPos::new(q[0], q[1], q[2]))).collect::<Vec<i32>>()
                );
                if s != watched {
                    println!("t={t} day time {}: {s}", g % DAY);
                    watched = s;
                }
            }
        }
        true
    });
    match res {
        Err((_, why)) => bail!("the run failed: {why}"),
        Ok(Some((t, why))) if !why.is_empty() => bail!("the run stopped at tick {t}: {why}"),
        _ => {}
    }
    // the report
    let bounds = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    let size = [bounds.1[0] - bounds.0[0] + 1, bounds.1[1] - bounds.0[1] + 1, bounds.1[2] - bounds.0[2] + 1];
    let minutes = circuit::mobfarm::site_bill(&c, None, 1).map(|b| format!("{:.1}", b.seconds / 60.0)).unwrap_or_else(|e| format!("? ({e})"));
    println!(
        "{}: {} blocks, box {}x{}x{}, {} build minutes at middle prices; hump window: up at the line's fall {} times, {} gaps, lasts {}..{} gt past the fall ({} under 100), {} windows with no dispatch",
        design.display(),
        c.clock.design.blocks.len(),
        size[0],
        size[1],
        size[2],
        minutes,
        visits.saturating_sub(gaps + nodispatch),
        gaps,
        window.0,
        window.1,
        short,
        nodispatch
    );
    let night_of = |t: u64| (t + DAY - 6_000) / DAY;
    let (first, last) = (night_of(t0) + 1, night_of(t0 + ticks as u64) - 1);
    let (mut second, mut missing, mut vn) = (0u32, 0u32, 0u32);
    let mut worst = i64::MIN;
    for (v, ns) in naps.iter().enumerate() {
        let mut per = std::collections::BTreeMap::<u64, u32>::new();
        for &n in ns {
            *per.entry(night_of(n)).or_default() += 1;
        }
        let counts: Vec<u32> = (first..=last).map(|n| per.get(&n).copied().unwrap_or(0)).collect();
        second += counts.iter().map(|c| c.saturating_sub(1)).sum::<u32>();
        missing += counts.iter().filter(|c| **c == 0).count() as u32;
        vn += counts.len() as u32;
        let gap = std::iter::once(0u64).chain(ns.iter().copied()).zip(ns.iter().copied()).map(|(a, b)| b as i64 - a as i64 - DAY as i64).max().unwrap_or(0);
        worst = worst.max(gap);
        let dts: Vec<u64> = ns.iter().map(|n| n % DAY).collect();
        println!(
            "  villager {v:2} (lane {}, level {}): naps a night {}..{}, nap day time {}..{}, worst gap window {}",
            v / 3,
            v % 3,
            counts.iter().min().copied().unwrap_or(0),
            counts.iter().max().copied().unwrap_or(0),
            dts.iter().min().copied().unwrap_or(0),
            dts.iter().max().copied().unwrap_or(0),
            if gap > 0 { format!("{gap} gt") } else { "none".into() }
        );
    }
    println!(
        "{}: {} days, seed {}: {} slots attempted ({} missed), {} lost to an unwilling villager{}; {} visits ({} not the batch that just left); {} villager-nights without a nap, {} second naps in {} villager-nights; worst gap window {}",
        design.display(),
        o.days,
        o.seed,
        attempts,
        miss_count.get(),
        lost,
        lost_first.map(|t| format!(" (first at day {}, day time {})", t / DAY, t % DAY)).unwrap_or_default(),
        visits,
        wrong_lane,
        missing,
        second,
        vn,
        if worst > 0 { format!("{worst} gt") } else { "none".into() }
    );
    Ok(lost == 0 && wrong_lane == 0 && missing == 0 && gaps == 0 && short == 0 && nodispatch == 0)
}
