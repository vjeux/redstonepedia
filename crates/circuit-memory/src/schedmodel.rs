//! The nightly scheduler's policy model (designs/briefs/farm-iron-carousel.md part 3, the sleep
//! routine; its scheduler is the day-1 lane's sub-part, ironspec's contract 2026-11-10): which
//! villager goes to the one bed at which counter advance, judged on timing alone before any
//! circuit is built. `mctas circuit memory sched-model --policy P`.
//!
//! The farm: 7 batches of 3 villagers (villager `3b + k`: lane b, level k), the counter's cycle
//! 0 2 4 6 5 3 1, slots on the 100-gt grid. After every advance the next batch is ready `--ready` gt
//! later and attempts at the first slot from then: it summons when all three of its villagers are
//! willing (gameTime - LAST_SLEPT < 24,000 strictly, the code's own condition) and the slot isn't a
//! random miss (`--miss`, repeated with `--burst`); the golem pulse rises 0..5 gt after the slot
//! and the counter advances 21 gt later; with no golem pulse before the fallback's edge, the
//! counter's fallback advances 106..108 gt after the previous advance (counter v7's numbers).
//!
//! A visit (ironfarm's yard): dispatched at an advance, for a villager of the batch whose line just
//! fell (it is coming back from the station); it naps (LAST_SLEPT) `--nap` gt later and the visit
//! ends `--visit` gt later; one visit at a time (one bed, one yard); a visit must end before its
//! batch's line rises again; a dispatch is allowed only when the day time of the advance is in
//! [`--dusk`, `--dawn` - the longest visit] (REST runs from 12000 to 10 the next day).
//!
//! The policies:
//! - `dusk21` (ironfarm's draft): from dusk, every `--every`-th line fall; the level is the visit
//!   index div 7; 21 visits, then nothing until the next dusk.
//! - `crt`: the night's first visit is the first fall of line 0 after dusk, then every `--every`-th
//!   advance, the level the visit index mod 3 (5, 7 and 3 coprime: 21 visits name every villager
//!   once); 21 visits, then nothing until the next dusk.
//! - `round`: every `--every`-th advance inside the window (a phase that only steps there), the
//!   lane's own pointer names the level and steps; never stops: a night with more than 21 visits
//!   gives some villagers a second nap (allowed: ironspec 2026-11-10), and the rotation carries
//!   over to the next night.
//! - `urgent`: like `round`, but the lane's villager with the oldest LAST_SLEPT goes (the best a
//!   per-villager memory could do: a bound, not a circuit).
//!
//! It prints, per villager, its naps a night (min..max), its nap day-time range, its worst gap
//! window (the longest stretch it was unwilling, and whether a slot of its batch fell in one), and
//! the totals: slots attempted, lost to an unwilling villager (target 0), random misses, visits
//! refused because the bed was busy, visits that overran their batch's release.

use crate::seqbench::Rng;
use anyhow::{Result, bail};

const DAY: u64 = 24_000;
/// The counter's cycle (counter v7): the lanes in the order their lines rise.
const CYCLE: [usize; 7] = [0, 2, 4, 6, 5, 3, 1];

#[derive(Clone, Debug)]
pub struct Opts {
    pub policy: String,
    pub days: u32,
    pub seed: u64,
    pub miss: f64,
    pub burst: f64,
    pub every: u32,
    pub dusk: u64,
    pub dawn: u64,
    pub nap: (u32, u32),
    pub visit: (u32, u32),
    pub ready: u64,
    /// Step the dispatch phase on every advance, day and night (default: only inside the window).
    pub phase_always: bool,
    pub verbose: bool,
    /// `block`: a night whose block starts within this many gt of dusk runs 22 visits (the head
    /// naps again at the end) and moves the head on.
    pub rotate: u64,
    /// `pring`: the window's last day time for a dispatch.
    pub pring_end: u64,
    /// The yard queues a napper dispatched while the bed is busy (it goes when the bed frees).
    pub queue: bool,
    /// `pring`: hold a dispatch while the yard is busy (the pointer stays).
    pub hold: bool,
    /// The share of visits whose nap slips by the HOME re-take (40..200 gt, ironfarm 2026-11-10),
    /// the yard's idle time with it.
    pub retake: f64,
    /// The share of visits drawn from the tail [`visit_body`, the longest] (0: uniform).
    pub visit_tail: f64,
    pub visit_body: u32,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { policy: "round".into(), days: 60, seed: 1, miss: 0.05, burst: 0.3, every: 5, dusk: 12_000, dawn: 24_010, nap: (135, 185), visit: (291, 486), ready: 40, phase_always: false, verbose: false, rotate: 200, pring_end: 22_965, queue: false, hold: false, retake: 0.0, visit_tail: 0.0, visit_body: 330 }
    }
}

#[derive(Default, Clone)]
struct Villager {
    naps: Vec<u64>,
    last: Option<u64>,
    /// Gap windows that held a slot of its batch.
    lost: u32,
}

struct Visit {
    v: usize,
    end: u64,
}

struct Model<'a> {
    o: &'a Opts,
    rng: Rng,
    vill: Vec<Villager>,
    bed_free: u64,
    pending: Vec<Visit>,
    // policy state
    night: Option<u64>,
    falls: u32,
    visits: u32,
    started: bool,
    phase: u32,
    ptr: [usize; 7],
    /// `block`: the head (its batch's cycle position, its level) and tonight's block length.
    head: (usize, usize),
    block_len: u32,
    // totals
    refused: u32,
    held: u32,
    overran: u32,
    dispatched: u32,
}

impl Model<'_> {
    fn in_window(&self, t: u64) -> bool {
        let dt = t % DAY;
        dt >= self.o.dusk && dt + self.o.visit.1 as u64 <= self.o.dawn
    }

    fn night_of(t: u64) -> u64 {
        // night n: from day time 6000 of day n (no visit runs then) to 5999 of day n + 1
        (t + DAY - 6_000) / DAY
    }

    fn dispatch(&mut self, t: u64, lane: usize, level: usize) -> bool {
        // with --queue the napper waits at the yard's entry until the bed is free
        let t = if self.o.queue { t.max(self.bed_free) } else { t };
        if self.bed_free > t {
            self.refused += 1;
            return false;
        }
        let v = 3 * lane + level;
        let slip = if self.o.retake > 0.0 && self.rng.unit() < self.o.retake { self.rng.range(40, 200) as u64 } else { 0 };
        let nap = t + self.rng.range(self.o.nap.0, self.o.nap.1) as u64 + slip;
        // ironfarm's visits: median 303, a tail to 486 (a retried path): with --visit-tail p the
        // length is drawn from the upper part [--visit-body max, --visit max] with probability p
        let len = if self.o.visit_tail > 0.0 {
            if self.rng.unit() < self.o.visit_tail { self.rng.range(self.o.visit_body, self.o.visit.1) } else { self.rng.range(self.o.visit.0, self.o.visit_body) }
        } else {
            self.rng.range(self.o.visit.0, self.o.visit.1)
        };
        let end = t + len as u64 + slip;
        self.vill[v].naps.push(nap);
        self.vill[v].last = Some(nap);
        self.bed_free = end;
        self.pending.push(Visit { v, end });
        self.dispatched += 1;
        true
    }

    /// The line of `lane` fell at `t` (its batch is coming back).
    fn on_advance(&mut self, t: u64, lane: usize) {
        let n = Self::night_of(t);
        if self.night != Some(n) {
            self.night = Some(n);
            self.falls = 0;
            self.visits = 0;
            self.started = false;
        }
        let win = self.in_window(t);
        match self.o.policy.as_str() {
            "dusk21" => {
                if win {
                    if self.falls % self.o.every == 0 && self.visits < 21 {
                        let level = (self.visits / 7) as usize;
                        self.visits += 1;
                        let _ = self.dispatch(t, lane, level);
                    }
                    self.falls += 1;
                }
            }
            "crt" => {
                if win && !self.started && lane == 0 {
                    self.started = true;
                    self.phase = 0;
                }
                if self.started && self.visits < 21 {
                    if self.phase == 0 {
                        if win {
                            let level = (self.visits % 3) as usize;
                            let _ = self.dispatch(t, lane, level);
                        }
                        self.visits += 1;
                    }
                    self.phase = (self.phase + 1) % self.o.every;
                }
            }
            "round" | "urgent" => {
                if win || self.o.phase_always {
                    if self.phase == 0 && win {
                        let level = if self.o.policy == "urgent" {
                            (0..3).min_by_key(|k| self.vill[3 * lane + k].last.unwrap_or(0)).unwrap()
                        } else {
                            let l = self.ptr[lane];
                            self.ptr[lane] = (l + 1) % 3;
                            l
                        };
                        let _ = self.dispatch(t, lane, level);
                    }
                    self.phase = (self.phase + 1) % self.o.every;
                }
            }
            "pring" => {
                // the head pointer as a ring stepped once per visit: a visit whenever the line of
                // the batch it names falls inside the window [dusk, --pring-end]; each visit steps
                // it 5 batches on (so the next visit comes 5 advances later) and the level ring
                // one level on; nothing to count: a night with 21 visits leaves both where they
                // started, one with 22 or more moves the head on (the extra visits are the
                // first villagers' second naps), and the window's end decides how many
                let pos = CYCLE.iter().position(|&c| c == lane).unwrap();
                let dt = t % DAY;
                if dt >= self.o.dusk && dt <= self.o.pring_end && pos == self.head.0 {
                    // --hold: the yard still busy (ironfarm: idle ~280 gt after the wake), the
                    // pointer stays: the same villager goes at its batch's next fall
                    if self.o.hold && self.bed_free > t {
                        self.held += 1;
                        return;
                    }
                    let level = self.head.1;
                    let _ = self.dispatch(t, lane, level);
                    self.head = ((self.head.0 + self.o.every as usize) % 7, (self.head.1 + 1) % 3);
                }
            }
            "block" => {
                // the night's block starts at the first fall of the head batch's line after dusk;
                // visit i names (the batch 5i positions on in the cycle, the head's level + i);
                // 21 visits, or 22 when the start came within --rotate gt of dusk (the head naps
                // again at the end), after which the head moves on by one visit
                let pos = CYCLE.iter().position(|&c| c == lane).unwrap();
                if !self.started && self.visits == 0 && (t % DAY) >= self.o.dusk && pos == self.head.0 {
                    self.started = true;
                    self.phase = 0;
                    self.block_len = if (t % DAY) < self.o.dusk + self.o.rotate { 22 } else { 21 };
                    if self.o.verbose {
                        println!("  block: day {}, start day time {}, head lane {} level {}, {} visits", t / DAY, t % DAY, lane, self.head.1, self.block_len);
                    }
                }
                if self.started {
                    if self.phase == 0 {
                        let level = (self.head.1 + self.visits as usize) % 3;
                        debug_assert_eq!(pos, (self.head.0 + 5 * self.visits as usize) % 7);
                        let went = self.dispatch(t, lane, level);
                        self.visits += 1;
                        if self.visits == self.block_len {
                            self.started = false;
                            // the head moves on only if it really napped again at the end
                            if self.block_len == 22 && went {
                                self.head = ((self.head.0 + 5) % 7, (self.head.1 + 1) % 3);
                            }
                        }
                    }
                    self.phase = (self.phase + 1) % self.o.every;
                }
            }
            p => panic!("unknown policy {p}"),
        }
    }
}

/// The daylight detector's readings across a day in clear weather, rain and thunder (redsim's
/// model of the game: the day timeline's sky light, the weather layer, the sun angle), for a
/// detector seeing the sky (sky light 15): the day times each reading starts and ends, so a night
/// window can be cut where the weather doesn't move it.
pub fn sensor_table() {
    let read = |day: i64, rain: f32, thunder: f32, inverted: bool| -> i32 {
        let dark = redsim::weather::sky_darken_at(day, rain, thunder);
        let mut target = (15 - dark).max(0);
        let mut sun = redsim::daylight::sun_angle(day) * (std::f64::consts::PI / 180.0) as f32;
        if inverted {
            target = 15 - target;
        } else if target > 0 {
            let offset = if sun < std::f64::consts::PI as f32 { 0.0f32 } else { (std::f64::consts::PI * 2.0) as f32 };
            sun += (offset - sun) * 0.2f32;
            // Math.round(float)
            target = (target as f32 * redsim::entity::mth::cos(sun as f64) + 0.5).floor() as i32;
        }
        target.clamp(0, 15)
    };
    for (name, rain, thunder) in [("clear", 0.0f32, 0.0f32), ("rain", 1.0, 0.0), ("thunder", 1.0, 1.0)] {
        for inverted in [true, false] {
            let mut runs: Vec<(i64, i64, i32)> = Vec::new();
            for day in 0..24_000i64 {
                let v = read(day, rain, thunder, inverted);
                match runs.last_mut() {
                    Some((_, end, last)) if *last == v => *end = day,
                    _ => runs.push((day, day, v)),
                }
            }
            let line: Vec<String> = runs.iter().map(|(a, b, v)| format!("{v}@{a}..{b}")).collect();
            println!("{name:7} {}: {}", if inverted { "inverted" } else { "normal  " }, line.join(" "));
        }
    }
}

/// Run the model; prints the report and returns whether no slot was lost and every villager napped
/// every night.
pub fn run(o: &Opts) -> Result<bool> {
    if o.policy == "sensor" {
        sensor_table();
        return Ok(true);
    }
    if !["dusk21", "crt", "round", "urgent", "block", "pring"].contains(&o.policy.as_str()) {
        bail!("--policy: dusk21, crt, round, urgent, block or pring");
    }
    let mut m = Model {
        o,
        rng: Rng(o.seed),
        vill: vec![Villager::default(); 21],
        bed_free: 0,
        pending: Vec::new(),
        night: None,
        falls: 0,
        visits: 0,
        started: false,
        phase: 0,
        ptr: [0; 7],
        head: (0, 0),
        block_len: 21,
        refused: 0,
        held: 0,
        overran: 0,
        dispatched: 0,
    };
    // start at noon of day 0 with every villager rested at midnight before (willing until 24000)
    let start: u64 = 6_000;
    for v in m.vill.iter_mut() {
        v.last = Some(0);
    }
    let end = start + o.days as u64 * DAY;
    let mut cur = 0usize; // index into CYCLE: the batch selected by the last advance
    let mut last_adv = start;
    let mut missed_last = false;
    let (mut attempts, mut lost_unwilling, mut random_miss, mut fallbacks) = (0u32, 0u32, 0u32, 0u32);
    let mut lost_at: Vec<(u64, usize)> = Vec::new();
    while last_adv < end {
        let next_slot = (last_adv + o.ready).div_ceil(100) * 100;
        let m_fall = m.rng.range(6, 8) as u64;
        let fall_line = last_adv + 100 + m_fall;
        let rise = next_slot + m.rng.range(0, 5) as u64;
        let lane = CYCLE[cur];
        let adv;
        if rise + 21 < fall_line {
            attempts += 1;
            let willing: Vec<usize> = (0..3).filter(|k| m.vill[3 * lane + k].last.is_some_and(|s| next_slot - s < DAY)).collect();
            let miss = m.rng.unit() < if missed_last { o.burst.max(o.miss) } else { o.miss };
            if willing.len() < 3 {
                lost_unwilling += 1;
                lost_at.push((next_slot, lane));
                for k in 0..3 {
                    if !willing.contains(&k) {
                        m.vill[3 * lane + k].lost += 1;
                        if o.verbose {
                            let v = &m.vill[3 * lane + k];
                            println!(
                                "  lost slot at {next_slot} (day {}, day time {}): villager {} (lane {lane}, level {k}) last napped at {:?} (day time {:?}), naps {:?}",
                                next_slot / DAY,
                                next_slot % DAY,
                                3 * lane + k,
                                v.last,
                                v.last.map(|s| s % DAY),
                                v.naps.iter().rev().take(3).map(|n| (n / DAY, n % DAY)).collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
            if miss {
                random_miss += 1;
            }
            missed_last = miss;
            if willing.len() == 3 && !miss {
                adv = rise + 21;
            } else {
                adv = fall_line;
                fallbacks += 1;
            }
        } else {
            adv = fall_line;
            fallbacks += 1;
        }
        // the batch of `lane` leaves: its line falls at `adv`; the next batch's line rises
        cur = (cur + 1) % 7;
        let rising = CYCLE[cur];
        // a napper of the rising batch must be back
        let mut keep = Vec::new();
        for p in m.pending.drain(..) {
            if p.v / 3 == rising && p.end > adv {
                m.overran += 1;
            } else if p.end > adv {
                keep.push(p);
            }
        }
        m.pending = keep;
        m.on_advance(adv, lane);
        last_adv = adv;
    }
    // per villager
    let mut ok = lost_unwilling == 0 && m.refused == 0 && m.overran == 0;
    let first_night = Model::night_of(start) + 1;
    let last_night = Model::night_of(end) - 1;
    let mut worst_gap_all: i64 = i64::MIN;
    let mut missing_nights = 0u32;
    let mut second_naps = 0u32;
    let mut villager_nights = 0u32;
    for (v, vi) in m.vill.iter().enumerate() {
        let mut per_night = std::collections::BTreeMap::<u64, u32>::new();
        for &n in &vi.naps {
            *per_night.entry(Model::night_of(n)).or_default() += 1;
        }
        let counts: Vec<u32> = (first_night..=last_night).map(|n| per_night.get(&n).copied().unwrap_or(0)).collect();
        let (cmin, cmax) = (counts.iter().min().copied().unwrap_or(0), counts.iter().max().copied().unwrap_or(0));
        missing_nights += counts.iter().filter(|&&c| c == 0).count() as u32;
        second_naps += counts.iter().map(|&c| c.saturating_sub(1)).sum::<u32>();
        villager_nights += counts.len() as u32;
        let dts: Vec<u64> = vi.naps.iter().map(|n| n % DAY).collect();
        let (dmin, dmax) = (dts.iter().min().copied().unwrap_or(0), dts.iter().max().copied().unwrap_or(0));
        let worst_gap = std::iter::once(0u64).chain(vi.naps.iter().copied()).zip(vi.naps.iter().copied()).map(|(a, b)| b as i64 - a as i64 - DAY as i64).max().unwrap_or(0);
        worst_gap_all = worst_gap_all.max(worst_gap);
        if o.verbose || vi.lost > 0 {
            println!(
                "  villager {v:2} (lane {}, level {}): naps a night {cmin}..{cmax}, nap day time {dmin}..{dmax}, worst gap window {} gt, slots lost {}",
                v / 3,
                v % 3,
                if worst_gap > 0 { worst_gap.to_string() } else { "none".into() },
                vi.lost
            );
        }
    }
    if missing_nights > 0 {
        ok = false;
    }
    println!(
        "{}: {} days, every {}: {} slots attempted, {} lost to an unwilling villager{}, {} random misses, {} fallback advances; {} visits ({} refused: bed busy, {} held for the yard, {} overran their batch's release); {} villager-nights without a nap, {} second naps in {} villager-nights; worst gap window {} gt",
        o.policy,
        o.days,
        o.every,
        attempts,
        lost_unwilling,
        lost_at.first().map(|(t, l)| format!(" (first at {t}, day time {}, lane {l})", t % DAY)).unwrap_or_default(),
        random_miss,
        fallbacks,
        m.dispatched,
        m.refused,
        m.held,
        m.overran,
        missing_nights,
        second_naps,
        villager_nights,
        if worst_gap_all > 0 { worst_gap_all.to_string() } else { "none".into() }
    );
    Ok(ok)
}
