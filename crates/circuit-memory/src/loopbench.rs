//! The single-lane loop scheduler's bench (day-1 lane 2026-11-10): `mctas circuit memory
//! loop-bench <design>`. The design runs on redsim with its two inputs driven directly (a redstone
//! block while high, stone while low): `release`, a 10-gt pulse at every release (a golem at the
//! slot, or the fallback after a missed slot: the farm of `sched-bench`), and `win`, the night window.
//! The queue keeps its order (ironfarm: the napper's batch waits at the bed on the first-in,
//! first-out return path), so release i sends batch i mod 7 to the station and the batch leaving at
//! release i is batch (i - 1) mod 7. A visit is read off the design: `v` rising, the napper slot j
//! from `l0..l2` at that moment; that batch's villager j naps `--nap` gt later. Every golem slot's
//! batch must be willing (gameTime - LAST_SLEPT < 24,000). It prints per villager its naps a night,
//! nap day-time range and worst gap window, and the totals; `--game N` replays the first N days'
//! input actions in the real game, every output probe compared every tick.

use crate::seqbench::Rng;
use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use std::path::Path;

const DAY: u64 = 24_000;

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
    pub pulse: u64,
    pub game_days: u32,
    /// The night window's source: `ideal` (high inside [dusk, end] of day time) or a normal daylight
    /// detector reading 0 under `clear`, `rain`, `thunder` or `cycle` (random weather spells of 0.5
    /// to 1.5 days, the rain and thunder levels ramping 0.01 a tick like the game's).
    pub weather: String,
    /// Repeat the run with this many chunk reloads at random ticks; the outputs must match.
    pub restarts: u32,
    /// Place the design raw (written states and block entities, no updates, no settle).
    pub raw: bool,
}

/// A normal daylight detector seeing the sky at day time `day` with these weather levels (redsim's
/// model of the game, as `sched-model --policy sensor` prints): its output.
pub fn detector_normal(day: i64, rain: f32, thunder: f32) -> i32 {
    let dark = redsim::weather::sky_darken_at(day, rain, thunder);
    let target = (15 - dark).max(0);
    if target == 0 {
        return 0;
    }
    let mut sun = redsim::daylight::sun_angle(day) * (std::f64::consts::PI / 180.0) as f32;
    let offset = if sun < std::f64::consts::PI as f32 { 0.0f32 } else { (std::f64::consts::PI * 2.0) as f32 };
    sun += (offset - sun) * 0.2f32;
    ((target as f32 * redsim::entity::mth::cos(sun as f64) + 0.5).floor() as i32).clamp(0, 15)
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { days: 60, seed: 1, miss: 0.01, burst: 0.3, dusk: 13_670, end: 22_330, nap: (115, 135), ready: 40, pulse: 10, game_days: 0, weather: "ideal".into(), restarts: 0, raw: false }
    }
}

pub fn bench(design: &Path, o: &Opts) -> Result<bool> {
    let mut recorded = Vec::new();
    let mut trace = Vec::new();
    let mut ok = run(design, o, &mut recorded, &[], &mut trace)?;
    if o.restarts > 0 {
        // the same run with chunk reloads at random ticks (a player leaving and coming back): the
        // outputs must change at the same ticks
        let mut rng = Rng(o.seed ^ 0x0e10_ad5);
        let ticks = (o.days as u64 * DAY) as u32;
        let mut reloads: Vec<u32> = (0..o.restarts).map(|_| rng.range(100, ticks - 100)).collect();
        reloads.sort_unstable();
        let (mut rec2, mut trace2) = (Vec::new(), Vec::new());
        run(design, o, &mut rec2, &reloads, &mut trace2)?;
        if trace2 == trace {
            println!("{} restarts (chunk reloads at random ticks): the same {} output changes, tick for tick", o.restarts, trace.len());
        } else {
            let i = trace.iter().zip(&trace2).position(|(a, b)| a != b).unwrap_or(trace.len().min(trace2.len()));
            println!("RESTARTS DIVERGE at output change {i}: {:?} vs {:?}", trace.get(i), trace2.get(i));
            ok = false;
        }
    }
    if o.game_days > 0 {
        let probes: Vec<String> = Circuit::load(design)?.clock.design.ports.keys().filter(|k| k.starts_with("output.")).cloned().collect();
        return Ok(crate::schedbench::game_check_named(design, (o.game_days as u64 * DAY) as u32, &recorded, &probes)? && ok);
    }
    Ok(ok)
}

fn run(design: &Path, o: &Opts, recorded: &mut Vec<(u32, clock::Action)>, reloads: &[u32], trace: &mut Vec<(u32, u8)>) -> Result<bool> {
    let c = Circuit::load(design)?;
    let chunks = {
        let (from, to) = circuit::run::watch_box(&c, 3);
        redsim::reload::box_chunks([from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)])
    };
    let mut ri = 0usize;
    let mut last_bits = 0u8;
    let find = |name: &str| -> Result<redsim::pos::BlockPos> {
        let p = c.clock.design.ports.get(name).ok_or_else(|| anyhow!("{}: no port {name}", design.display()))?;
        Ok(redsim::pos::BlockPos::new(p[0], p[1], p[2]))
    };
    let rel = find("input.release")?;
    let win = find("input.win")?;
    let vv = find("output.v")?;
    let ll: Vec<redsim::pos::BlockPos> = (0..3).map(|k| find(&format!("output.s{k}"))).collect::<Result<_>>()?;
    let cc = c.with_consumers().map_err(|e| anyhow!("{e:#}"))?;
    let (from, to) = circuit::run::watch_box(&c, 3);
    let on = redsim::Block::parse("minecraft:redstone_block").map_err(|e| anyhow!(e))?;
    let off = redsim::Block::parse("minecraft:stone").map_err(|e| anyhow!(e))?;
    let mut rng = Rng(o.seed);
    let t0: u64 = 6_000;
    let ticks = (o.days as u64 * DAY) as u32;
    // the farm: release i sends batch i mod 7 to the station; its golem attempt is the next slot
    let mut release_index: u64 = 0;
    let mut station: usize = 0; // the batch at the station (released at index 0 before the run)
    let mut last_adv: u64 = t0;
    let mut missed_last = false;
    let mut last_slept = [Some(0u64); 21];
    let mut naps: Vec<Vec<u64>> = vec![Vec::new(); 21];
    let mut pending_naps: Vec<(u64, usize)> = Vec::new();
    let (mut attempts, mut lost, mut misses, mut visits, mut bad) = (0u32, 0u32, 0u32, 0u32, 0u32);
    let mut lost_first: Option<u64> = None;
    let mut plan = |last_adv: u64, batch: usize, rng: &mut Rng, missed_last: &mut bool, last_slept: &[Option<u64>; 21], attempts: &mut u32, lost: &mut u32, lost_first: &mut Option<u64>, misses: &mut u32| -> u64 {
        let next_slot = (last_adv + o.ready).div_ceil(100) * 100;
        let fall = last_adv + 100 + rng.range(6, 8) as u64;
        let rise = next_slot + rng.range(0, 5) as u64;
        if rise + 21 < fall {
            *attempts += 1;
            let willing = (0..3).all(|k| last_slept[3 * batch + k].is_some_and(|s| next_slot - s < DAY));
            if !willing {
                *lost += 1;
                lost_first.get_or_insert(next_slot);
            }
            let miss = rng.unit() < if *missed_last { o.burst.max(o.miss) } else { o.miss };
            *missed_last = miss;
            if miss {
                *misses += 1;
            }
            if willing && !miss {
                return rise + 21;
            }
        }
        fall
    };
    let mut next_adv = plan(last_adv, station, &mut rng, &mut missed_last, &last_slept, &mut attempts, &mut lost, &mut lost_first, &mut misses);
    let mut pulse_end: Option<u64> = None;
    let mut leaving: Option<(usize, u64)> = None; // the batch that left at the latest release
    let mut v_was = false;
    let mut v_this_release = 0u32;
    let (mut d_expected, mut ring_errors, mut last_visit_t) = (0u64, 0u32, 0u32);
    // the visit window's checks: v (and the slot) up from the release T, held past T + 100, the slot
    // unchanged while v is up
    let mut v_levels: Vec<usize> = Vec::new();
    let mut v_rise_at: Option<(u64, u64)> = None;
    let mut v_fall_at: Option<u64> = None;
    // the weather and the night window's edges seen (day time it opens min..max, closes min..max)
    let mut wrng = Rng(o.seed ^ 0x5eed_0f_ea7e);
    let (mut spell, mut spell_end) = (0u8, 0u64);
    let (mut rain_level, mut thunder_level) = (0.0f32, 0.0f32);
    let mut win_prev = false;
    let mut win_edges = (u64::MAX, 0u64, u64::MAX, 0u64);
    let (mut short, mut slot_changes) = (0u32, 0u32);
    let mut window = (u64::MAX, 0u64, u64::MAX, 0u64);
    let cb = |t: u32, w: &mut redsim::world::World| -> bool {
        let g = t as u64 + t0;
        while ri < reloads.len() && reloads[ri] <= t {
            if reloads[ri] == t {
                redsim::reload::reload_chunks(w, &chunks);
            }
            ri += 1;
        }
        pending_naps.retain(|&(at, v)| {
            if at == g {
                naps[v].push(g);
                last_slept[v] = Some(g);
                false
            } else {
                true
            }
        });
        let mut set = |w: &mut redsim::world::World, p: redsim::pos::BlockPos, high: bool, recorded: &mut Vec<(u32, clock::Action)>| {
            let is_on = matches!(w.get(p), redsim::Block::RedstoneBlock);
            if is_on != high {
                w.set_block(p, if high { on } else { off }, redsim::world::flags::ALL);
                recorded.push((t, clock::Action::Place { pos: [p.x, p.y, p.z], state: if high { "minecraft:redstone_block".into() } else { "minecraft:stone".into() }, nbt: None }));
            }
        };
        if g == next_adv {
            // the D ring's token (generator layout: ring cell k at x = k < 7 ? 0 : 1, z = k < 7 ? k : 13 - k)
            // against the policy's D = P - R: the token at dropper i is D = -i mod 7
            let at: Vec<usize> = (0..14)
                .filter(|k| {
                    let (x, z) = if *k < 7 { (0, *k as i32) } else { (1, 13 - *k as i32) };
                    let p = redsim::pos::BlockPos::new(x, 0, z);
                    redsim::behavior::analog_output(w, &w.get(p), p, redsim::Dir::North) > 0
                })
                .collect();
            if ring_errors < 5 {
                let want_k = 2 * ((7 - d_expected) % 7) as usize;
                if at != vec![want_k] {
                    ring_errors += 1;
                    println!("ring check at t={t} (day {}, day time {}): token in ring cells {at:?}, policy wants cell {want_k} (D = {d_expected}); last visit release at t={}", g / DAY, g % DAY, last_visit_t);
                }
            }
            leaving = Some((station, g));
            release_index += 1;
            // the release steps D - 1 (a visit at this release resets it to 3 when `v` rises)
            d_expected = (d_expected + 6) % 7;
            station = (release_index % 7) as usize;
            last_adv = g;
            pulse_end = Some(g + o.pulse);
            v_this_release = 0;
            next_adv = plan(last_adv, station, &mut rng, &mut missed_last, &last_slept, &mut attempts, &mut lost, &mut lost_first, &mut misses);
        }
        let high = pulse_end.is_some_and(|e| g < e);
        set(w, rel, high, recorded);
        let dt = g % DAY;
        // the weather: spells of clear, rain or thunder, the levels ramping 0.01 a tick
        if o.weather == "cycle" && g >= spell_end {
            spell_end = g + wrng.range(12_000, 36_000) as u64;
            spell = (wrng.range(0, 2)) as u8;
        }
        let (rt, tt) = match (o.weather.as_str(), spell) {
            ("rain", _) | ("cycle", 1) => (1.0f32, 0.0f32),
            ("thunder", _) | ("cycle", 2) => (1.0, 1.0),
            _ => (0.0, 0.0),
        };
        rain_level += (rt - rain_level).clamp(-0.01, 0.01);
        thunder_level += (tt - thunder_level).clamp(-0.01, 0.01);
        let win_on = if o.weather == "ideal" { dt >= o.dusk && dt <= o.end } else { detector_normal(dt as i64, rain_level, thunder_level) == 0 };
        if win_on != win_prev {
            win_prev = win_on;
            if win_on {
                win_edges.0 = win_edges.0.min(dt);
                win_edges.1 = win_edges.1.max(dt);
            } else {
                win_edges.2 = win_edges.2.min(dt);
                win_edges.3 = win_edges.3.max(dt);
            }
        }
        set(w, win, win_on, recorded);
        let v = w.best_neighbor_signal(vv) > 0;
        let levels_now: Vec<usize> = (0..3).filter(|k| w.best_neighbor_signal(ll[*k]) > 0).collect();
        if v && !v_was {
            v_this_release += 1;
            v_rise_at = leaving.map(|l| (l.1, g));
            if v_this_release > 1 || leaving.is_none_or(|(_, at)| g > at + 40) {
                bad += 1;
            }
        }
        // the visit's napper slot: the slot line that rises during the window (power-on settling
        // in the first 40 ticks aside)
        if levels_now != v_levels && t < 40 {
            v_levels = levels_now.clone();
        }
        if levels_now != v_levels {
            match (levels_now.as_slice(), v_levels.as_slice()) {
                ([j], []) if v => match leaving {
                    Some((b, at)) if g <= at + 60 => {
                        visits += 1;
                        pending_naps.push((at + rng.range(o.nap.0, o.nap.1) as u64, 3 * b + j));
                        // the policy: this release's step was D - 1 from 0; the visit adds +4
                        d_expected = 3;
                        last_visit_t = t;
                    }
                    _ => bad += 1,
                },
                ([], [_]) => {
                    // a slot line falls: at most 12 gt after the window
                    if v || v_fall_at.is_none_or(|f| g > f + 12) {
                        slot_changes += 1;
                        if slot_changes <= 3 {
                            println!("slot line falls at t={t} (v {v}, window ended at {v_fall_at:?})");
                        }
                    }
                }
                _ => {
                    slot_changes += 1;
                    if slot_changes <= 3 {
                        println!("slot lines at t={t}: {:?} -> {:?} (v {v})", v_levels, levels_now);
                    }
                }
            }
            v_levels = levels_now.clone();
        }
        if !v && v_was {
            v_fall_at = Some(g);
            if let Some((rel_at, rise)) = v_rise_at.take() {
                window = (window.0.min(rise - rel_at), window.1.max(rise - rel_at), window.2.min(g - rel_at), window.3.max(g - rel_at));
                if g - rel_at < 85 {
                    short += 1;
                }
            }
        }
        v_was = v;
        let bits = (v as u8) | levels_now.iter().fold(0u8, |a, k| a | (2 << k));
        if bits != last_bits {
            trace.push((t, bits));
            last_bits = bits;
        }
        true
    };
    let res = if o.raw { crate::releasebench::drive_raw(&cc.clock, ticks, from, to, cb) } else { clock::sim::drive_live(&cc.clock, &[], ticks, from, to, cb).map_err(|(_, why)| why) };
    match res {
        Err(why) => bail!("the run failed: {why}"),
        Ok(Some((t, why))) if !why.is_empty() => bail!("the run stopped at tick {t}: {why}"),
        _ => {}
    }
    // the report
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
            "  villager {v:2} (batch {}, slot {}): naps a night {}..{}, nap day time {}..{}, worst gap window {}",
            v / 3,
            v % 3,
            counts.iter().min().copied().unwrap_or(0),
            counts.iter().max().copied().unwrap_or(0),
            dts.iter().min().copied().unwrap_or(0),
            dts.iter().max().copied().unwrap_or(0),
            if gap > 0 { format!("{gap} gt") } else { "none".into() }
        );
    }
    let (lo, hi) = c.clock.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    let minutes = circuit::mobfarm::site_bill(&c, None, 1).map(|b| format!("{:.1}", b.seconds / 60.0)).unwrap_or_else(|e| format!("? ({e})"));
    println!("{}: {} blocks, box {}x{}x{}, {} build minutes at middle prices", design.display(), c.clock.design.blocks.len(), hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1, minutes);
    println!(
        "night window ({}): opens at day time {}..{}, closes at {}..{}",
        o.weather, win_edges.0, win_edges.1, win_edges.2, win_edges.3
    );
    println!(
        "visit window: v rises {}..{} gt after its release and falls {}..{} gt after it ({} under 85); slot lines out of place {} times (one rises inside each window, falls within 12 gt of its end)",
        window.0, window.1, window.2, window.3, short, slot_changes
    );
    println!(
        "{}: {} days, seed {}: {} releases, {} golem slots ({} missed), {} lost to an unwilling villager{}; {} visits ({} visit signals out of place); {} villager-nights without a nap, {} second naps in {} villager-nights; worst gap window {}",
        design.display(),
        o.days,
        o.seed,
        release_index,
        attempts,
        misses,
        lost,
        lost_first.map(|t| format!(" (first at day {}, day time {})", t / DAY, t % DAY)).unwrap_or_default(),
        visits,
        bad,
        missing,
        second,
        vn,
        if worst > 0 { format!("{worst} gt") } else { "none".into() }
    );
    Ok(lost == 0 && bad == 0 && missing == 0 && short == 0 && slot_changes == 0 && ring_errors == 0)
}
