//! `mctas circuit memory loop-model`: the single-lane carousel's nightly sleep scheduling, modeled
//! (day-1 lane 2026-11-10; vjeux 06:01: one queue of 21 carts back to back, the front 3 released per
//! golem pulse or fallback, rejoining the tail). Every slot's batch (the front 3) must be willing
//! (gameTime - LAST_SLEPT < 24,000); a nap visit takes one cart of a batch coming back from the
//! station to the bed, and it rejoins the queue `--visit` gt later: at the tail (`--rejoin tail`, the
//! queue regroups) or in its own place (`--rejoin place`, its batch-mates wait). Policies:
//! `every`: a visit at every `--every`-th release inside the night window, the cart `k` of the batch
//! coming back (k cycling 0, 1, 2 per visit); `first-unvisited`: at every `--every`-th release, the
//! first cart of the coming-back batch that has not napped tonight (an oracle: what a marker would
//! buy). Prints lost slots, second naps and napless villager-nights.

use crate::seqbench::Rng;
use std::collections::VecDeque;

const DAY: u64 = 24_000;

#[derive(Clone, Debug)]
pub struct Opts {
    pub policy: String,
    pub days: u32,
    pub seed: u64,
    pub miss: f64,
    pub every: u32,
    pub dusk: u64,
    pub end: u64,
    pub visit: (u32, u32),
    pub nap: (u32, u32),
    pub rejoin: String,
    pub out: u64,
    pub verbose: bool,
}

pub fn run(o: &Opts) -> bool {
    let mut rng = Rng(o.seed);
    let mut queue: VecDeque<usize> = (0..21).collect();
    // batches out: (rejoin time, carts); the station holds the last released batch
    let mut back: Vec<(u64, Vec<usize>)> = Vec::new();
    let mut away: Vec<(u64, usize, Option<usize>)> = Vec::new(); // (rejoin time, villager, place index for `place`)
    let mut last_slept = [0u64; 21];
    let mut naps: Vec<Vec<u64>> = vec![Vec::new(); 21];
    let mut pending_naps: Vec<(u64, usize)> = Vec::new();
    let (mut attempts, mut lost, mut releases_in_window, mut k_cycle) = (0u32, 0u32, 0u32, 0usize);
    // `pring`: ironspec's re-key of the 7-lane policy to the release order: the release index mod 7
    // names the batch; a visit when the batch coming back is the pointer's, then the pointer moves
    // 4 releases on and the level ring 1 on
    let (mut release_index, mut p_ring, mut l_ring) = (0u64, 0u64, 0usize);
    let mut station_index = 0u64;
    let mut napped_tonight = [false; 21];
    let mut night = 0u64;
    let t0 = 6_000u64;
    let end = t0 + o.days as u64 * DAY;
    let mut t = t0;
    let mut station: Vec<usize> = Vec::new();
    while t < end {
        // the next release: a golem at the next slot (+21..26) unless missed, else the fallback
        let slot = (t + 40).div_ceil(100) * 100;
        let fall = t + 100 + rng.range(6, 8) as u64;
        let rise = slot + rng.range(0, 5) as u64 + 21;
        let next = if rise < fall && rng.unit() >= o.miss { rise } else { fall };
        // events up to `next`: naps land, carts rejoin
        pending_naps.retain(|&(at, v)| {
            if at <= next {
                naps[v].push(at);
                last_slept[v] = at;
                false
            } else {
                true
            }
        });
        back.sort_by_key(|b| b.0);
        while let Some(b) = back.first().filter(|b| b.0 <= next).cloned() {
            back.remove(0);
            for v in b.1 {
                queue.push_back(v);
            }
        }
        away.sort_by_key(|a| a.0);
        while let Some(a) = away.first().filter(|a| a.0 <= next).cloned() {
            away.remove(0);
            match a.2 {
                // `place`: right behind its batch-mates (the queue order never changes)
                Some(mate) if o.rejoin == "place" => match queue.iter().position(|v| *v == mate) {
                    Some(i) => queue.insert(i + 1, a.1),
                    None => queue.push_back(a.1),
                },
                _ => queue.push_back(a.1),
            }
        }
        t = next;
        let dt = t % DAY;
        let n = (t + DAY - 6_000) / DAY;
        if n != night {
            night = n;
            napped_tonight = [false; 21];
            releases_in_window = 0;
        }
        // the batch at the station comes back (to the tail, travel `out` gt), possibly minus a napper
        let leaving = std::mem::take(&mut station);
        let in_window = dt >= o.dusk && dt <= o.end;
        let mut visit: Option<usize> = None;
        if in_window && !leaving.is_empty() {
            releases_in_window += 1;
            if o.policy == "pring" {
                if station_index % 7 == p_ring {
                    visit = Some(l_ring % leaving.len());
                    p_ring = (p_ring + o.every as u64) % 7;
                    l_ring = (l_ring + 1) % 3;
                }
            } else if releases_in_window % o.every == 0 {
                visit = match o.policy.as_str() {
                    "every" => {
                        let k = k_cycle % leaving.len();
                        k_cycle += 1;
                        Some(k)
                    }
                    "k0" => Some(0),
                    "k1" => Some(1.min(leaving.len() - 1)),
                    "k2" => Some(2.min(leaving.len() - 1)),
                    _ => leaving.iter().position(|v| !napped_tonight[*v]),
                };
            }
        }
        let mut rest = Vec::new();
        let mut held = false;
        for (i, v) in leaving.iter().enumerate() {
            if Some(i) == visit {
                napped_tonight[*v] = true;
                pending_naps.push((t + rng.range(o.nap.0, o.nap.1) as u64, *v));
                let dur = rng.range(o.visit.0, o.visit.1) as u64;
                if o.rejoin == "batch" {
                    // the whole batch waits for its napper and rejoins the tail with it, in order
                    back.push((t + dur, leaving.clone()));
                    held = true;
                    break;
                }
                if o.rejoin == "exact" {
                    // the queue order never changes (the napper is back in its place before its
                    // batch's next turn, 700 gt later)
                    back.push((t + o.out, leaving.clone()));
                    held = true;
                    break;
                }
                // `place`: back right behind its batch-mates (the last of them still with it)
                let mate = leaving.iter().rev().find(|m| *m != v).copied();
                away.push((t + dur, *v, mate));
            } else {
                rest.push(*v);
            }
        }
        if !held && !rest.is_empty() {
            back.push((t + o.out, rest));
        }
        // the release: the front 3 go out; every one must be willing at the slot
        if queue.len() < 3 {
            lost += 1;
            attempts += 1;
            if o.verbose {
                println!("t={t} only {} carts queued", queue.len());
            }
            continue;
        }
        let batch: Vec<usize> = (0..3).map(|_| queue.pop_front().unwrap()).collect();
        station_index = release_index;
        release_index += 1;
        if next == rise {
            attempts += 1;
            if batch.iter().any(|v| t - last_slept[*v] >= DAY) {
                lost += 1;
            }
        }
        station = batch;
    }
    // report
    let night_of = |t: u64| (t + DAY - 6_000) / DAY;
    let (first, last) = (night_of(t0) + 1, night_of(end) - 1);
    let (mut second, mut missing, mut vn) = (0u32, 0u32, 0u32);
    let mut worst = i64::MIN;
    for ns in &naps {
        let mut per = std::collections::BTreeMap::<u64, u32>::new();
        for &x in ns {
            *per.entry(night_of(x)).or_default() += 1;
        }
        for n in first..=last {
            let c = per.get(&n).copied().unwrap_or(0);
            second += c.saturating_sub(1);
            missing += (c == 0) as u32;
            vn += 1;
        }
        let gap = std::iter::once(0u64).chain(ns.iter().copied()).zip(ns.iter().copied()).map(|(a, b)| b as i64 - a as i64 - DAY as i64).max().unwrap_or(0);
        worst = worst.max(gap);
    }
    println!(
        "loop-model {} every {} rejoin {} visit {}..{}: {} days, {} golem slots, {} lost; {} napless villager-nights, {} second naps in {}; worst gap window {}",
        o.policy,
        o.every,
        o.rejoin,
        o.visit.0,
        o.visit.1,
        o.days,
        attempts,
        lost,
        missing,
        second,
        vn,
        if worst > 0 { format!("{worst} gt") } else { "none".into() }
    );
    lost == 0 && missing == 0
}
