//! The night's napper picker (ironspec Nov 11 16:38/16:46; vjeux's gravity queue paper v0): with no
//! queue slots, nothing in the farm knows which villager rides which cart. Every loop here is FIFO
//! (hold, stand, climb, switch, tail chain, pressed run, release), so the carts pass any point in a
//! fixed cyclic order, 3 a golem cycle; the day (240 cycles, 720 passes = 34 x 21 + 6) rotates which
//! cart passes at a given time of day by 6 places. A nap re-enters its cart behind every cart that
//! passed meanwhile (gravity), or into its own travelling hole (the register: the napper keeps its
//! place). The bar is not just 21 naps a night: a villager helps summon only while gameTime -
//! LAST_SLEEP < 24,000 (`Villager.golemSpawnConditionsMet`, LAST_SLEEP set at lie-down), the summon
//! runs every 100 gt and needs all 3 of the stand's batch eligible, so a nap later in the night than
//! the night before loses every summon of that rider's batches in between. This models the order in
//! game ticks, continuously over days, and scores summons lost per day and each rider's drift.

use std::collections::VecDeque;

/// Who the switch diverts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picker {
    /// Crossings counted from dusk and from each pick; the first cart at count >= k once the bed is
    /// free.
    FromPick(usize),
    /// Crossings counted from dusk and from each sleeper's return; the k-th once the bed is free.
    FromReturn(usize),
    /// (b) A cart-anchored start: the night's first pick is the cart `offset` places after last
    /// night's last napper (a mod-`carts` count of passes since it re-entered), then every k-th from
    /// each pick.
    Anchored(usize, usize),
    /// Today's register: a visit every `every` releases from dusk, the visit taking the batch's cart
    /// (visit index mod 3); the napper keeps its place (`keep_place`).
    Register(usize),
}

impl Picker {
    pub fn name(self) -> String {
        match self {
            Picker::FromPick(k) => format!("every {k}th from the pick"),
            Picker::FromReturn(k) => format!("every {k}th from the return"),
            Picker::Anchored(k, o) => format!("cart-anchored start (+{o}), then every {k}th"),
            Picker::Register(e) => format!("register: a visit every {e} releases, batch cart j mod 3"),
        }
    }
}

/// The model's knobs.
#[derive(Clone, Debug)]
pub struct Opts {
    pub carts: usize,
    pub visits: usize,
    pub picker: Picker,
    /// The napper keeps its place: its slot travels as a hole until it drops back in (the register).
    pub keep_place: bool,
    /// (c) The sleeper re-enters after exactly this many passes since its pick (a gated merge), or at
    /// its return if later; else at its return.
    pub gate_m: Option<usize>,
    /// (c') The lap-exact gate: the sleeper re-enters right behind the cart that crossed the switch
    /// just before it, on that cart's next lap: after `carts - 1 - a` passes since its pick, `a` the
    /// nappers already away at the pick (it keeps its place in the order).
    pub gate_lap: bool,
    /// (58f770cc's) The sleeper re-enters just after the N-th batch following its pick has passed
    /// the switch: `batch - 1 - j + N * batch` passes, `j` its place in its batch.
    pub gate_batches: Option<usize>,
    /// With the lap-exact gate: the picker counts each re-entry as a pass (the napper's own missed
    /// crossing, a virtual hole), so its count keeps the register's slots without a hole in the stand.
    pub virtual_count: bool,
    /// (a negative control) Passes added to the lap-exact gate's count.
    pub gate_lap_off: i64,
    /// Releases skipped a day (fix (a): 2, so 714 = 34 x 21 carts pass a day).
    pub skip_per_day: usize,
    /// A nap's trip, the switch to its re-entry point, uniform in [lo, hi] gt; lie-down this many gt
    /// after the pick (LAST_SLEEP).
    pub trip: (i64, i64),
    pub sleep_at: i64,
    pub days: usize,
    pub warmup: usize,
    pub seed: u64,
    pub night_gt: i64,
    pub day_gt: i64,
    pub period: i64,
    pub batch: usize,
    pub spacing: i64,
    /// The batch passes the pick point this many gt into the cycle (its summon was at the cycle's
    /// start); dusk falls `dusk_at` gt into a cycle.
    pub cross_at: i64,
    pub dusk_at: i64,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo { lo } else { lo + (self.next() % ((hi - lo + 1) as u64)) as i64 }
    }
}

/// The score over the scored days.
#[derive(Clone, Debug, Default)]
pub struct Score {
    pub days: usize,
    /// Summons lost a day: to an expired LAST_SLEEP, to a visit's hole in the batch, to a skipped
    /// release; and the worst day's total.
    pub lost_expiry: f64,
    pub lost_hole: f64,
    pub lost_skip: f64,
    pub worst_day: usize,
    /// The largest night-to-night drift of any rider (gt later than 24,000 after its last nap), and
    /// the riders that ever drifted later.
    pub worst_drift: i64,
    pub drifting_riders: usize,
    /// Nights short of the count, the fewest distinct riders in a night, nights a gated re-entry was
    /// late (the trip longer than its M passes).
    pub short_nights: usize,
    pub least_distinct: usize,
    pub late_gates: usize,
    /// The most nappers back from the bed and waiting at the gate at once (the waiting room).
    pub waiting_max: usize,
    /// Rider-nights in the scored days with no nap.
    pub missed: usize,
    /// Each rider's worst night-to-night drift (gt, 0 = never later).
    pub per_rider: Vec<i64>,
}

/// The loop in continuous time over `days` days.
pub fn simulate(o: &Opts) -> Score {
    let mut rng = Rng(o.seed);
    let mut fifo: VecDeque<usize> = (0..o.carts).collect();
    let mut last_sleep: Vec<Option<i64>> = vec![None; o.carts];
    let mut naps: Vec<Vec<i64>> = vec![Vec::new(); o.carts];
    // (the sleepers: id, physical return gt, passes since its pick, its gate's pass count)
    let mut away: Vec<(usize, i64, usize, Option<usize>)> = Vec::new();
    let mut count = 0usize;
    let mut anchor: Option<usize> = None;
    let mut anchor_passes = 0usize;
    let mut night_picks: Vec<usize> = Vec::new();
    let mut night_idx: Option<i64> = None;
    let mut night_releases = 0usize;
    let mut sc = Score { least_distinct: usize::MAX, ..Default::default() };
    let mut day_lost = [0usize; 3];
    let mut cur_day: i64 = 0;
    let cycles_per_day = o.day_gt / o.period;
    let skip_from = cycles_per_day * 2 / 3;
    let total_cycles = cycles_per_day * o.days as i64;
    for c in 0..total_cycles {
        let t0 = c * o.period;
        let rel = t0 - o.dusk_at;
        let day = rel.div_euclid(o.day_gt);
        let in_day = rel.rem_euclid(o.day_gt);
        let cyc_in_day = in_day / o.period;
        if day != cur_day {
            if cur_day >= o.warmup as i64 {
                sc.lost_expiry += day_lost[0] as f64;
                sc.lost_hole += day_lost[1] as f64;
                sc.lost_skip += day_lost[2] as f64;
                sc.worst_day = sc.worst_day.max(day_lost.iter().sum());
                sc.days += 1;
            }
            day_lost = [0; 3];
            cur_day = day;
        }
        let in_night = in_day < o.night_gt;
        if in_night && night_idx != Some(day) {
            night_idx = Some(day);
            night_picks.clear();
            night_releases = 0;
            count = 0;
        }
        if !in_night && night_idx == Some(day) && night_picks.len() < usize::MAX {
            // (the night just ended: score it once)
            if day >= o.warmup as i64 {
                let mut seen = vec![false; o.carts];
                night_picks.iter().for_each(|&p| seen[p] = true);
                sc.least_distinct = sc.least_distinct.min(seen.iter().filter(|s| **s).count());
                if night_picks.len() < o.visits {
                    sc.short_nights += 1;
                }
            }
            night_idx = Some(-1 - day);
        }
        if o.skip_per_day > 0 && cyc_in_day >= skip_from && cyc_in_day < skip_from + o.skip_per_day as i64 {
            day_lost[2] += 1;
            continue;
        }
        let mut members: Vec<Option<bool>> = Vec::new();
        for j in 0..o.batch {
            let t = t0 + o.cross_at + j as i64 * o.spacing;
            // (the sleepers back: gravity re-enters at the back, behind every cart that passed; a
            // gated one after exactly M passes since its pick, or at its return if that is later;
            // several may wait at the gate, in pick order)
            let mut i = 0;
            while i < away.len() {
                let (id, back, passes, gate) = away[i];
                let due = match gate { Some(m) => passes >= m && back <= t, None => back <= t };
                if !due {
                    i += 1;
                    continue;
                }
                if let Some(m) = gate
                    && passes > m
                {
                    sc.late_gates += 1;
                }
                if !o.keep_place {
                    fifo.push_back(id);
                }
                away.remove(i);
                anchor = Some(id);
                anchor_passes = 0;
                if let Picker::FromReturn(_) = o.picker {
                    count = 0;
                }
                if o.virtual_count && in_night {
                    count += 1;
                }
            }
            sc.waiting_max = sc.waiting_max.max(away.iter().filter(|a| a.1 <= t).count());
            let Some(cart) = fifo.pop_front() else { continue };
            for a in away.iter_mut() {
                a.2 += 1;
            }
            anchor_passes += 1;
            count += 1;
            // (keep-place: the napper's own slot passes as a hole while it's away; each rider's
            // eligibility judged at the cycle's summon, before this pass's pick re-arms it; the bed
            // free once every napper is back from it, gated or not)
            let hole = o.keep_place && away.iter().any(|a| a.0 == cart);
            let elig = last_sleep[cart].is_some_and(|ls| t0 - ls < 24000);
            let bed_free = away.iter().all(|a| a.1 <= t);
            let want = in_night && bed_free && night_picks.len() < o.visits && !hole && match o.picker {
                Picker::FromPick(k) | Picker::FromReturn(k) => count >= k,
                Picker::Anchored(k, off) => if night_picks.is_empty() { anchor.is_none() && count >= k || anchor.is_some() && anchor_passes % o.carts == off % o.carts } else { count >= k },
                Picker::Register(every) => j == night_picks.len() % 3 && night_releases % every == 0,
            };
            if want {
                night_picks.push(cart);
                last_sleep[cart] = Some(t + o.sleep_at);
                naps[cart].push(t + o.sleep_at);
                let gate = if o.gate_lap {
                    Some((o.carts as i64 - 1 - away.len() as i64 + o.gate_lap_off).max(0) as usize)
                } else if let Some(nb) = o.gate_batches {
                    Some(o.batch - 1 - j + nb * o.batch)
                } else {
                    o.gate_m
                };
                away.push((cart, t + rng.range(o.trip.0, o.trip.1), 0, gate));
                count = 0;
                // (the register diverts before the stand: its batch lacks the napper; gravity picks
                // after the summon, from the batch that made it)
                members.push(if o.keep_place { None } else { Some(elig) });
                if o.keep_place {
                    fifo.push_back(cart);
                }
            } else {
                members.push(if hole { None } else { Some(elig) });
                fifo.push_back(cart);
            }
        }
        if in_night {
            night_releases += 1;
        }
        let ok = members.len() == o.batch && members.iter().all(|m| *m == Some(true));
        if !ok {
            if members.iter().any(|m| m.is_none()) { day_lost[1] += 1 } else { day_lost[0] += 1 }
        }
    }
    // (drift: each rider's nap later than 24,000 after its last one, over the scored days; a
    // scored night with no nap is a missed night)
    let from = o.dusk_at + o.warmup as i64 * o.day_gt;
    let scored_nights = o.days.saturating_sub(o.warmup + 1);
    for n in &naps {
        let mut worst = 0i64;
        for w in n.windows(2) {
            if w[1] >= from {
                worst = worst.max(w[1] - w[0] - 24000);
            }
        }
        let in_scored = n.iter().filter(|&&t| t >= from).count();
        sc.missed += scored_nights.saturating_sub(in_scored);
        if worst > 0 {
            sc.drifting_riders += 1;
        }
        sc.worst_drift = sc.worst_drift.max(worst);
        sc.per_rider.push(worst);
    }
    if sc.days > 0 {
        sc.lost_expiry /= sc.days as f64;
        sc.lost_hole /= sc.days as f64;
        sc.lost_skip /= sc.days as f64;
    }
    if sc.least_distinct == usize::MAX {
        sc.least_distinct = 0;
    }
    sc
}

/// The report line for one row.
pub fn line(label: &str, o: &Opts, s: &Score) -> String {
    let riders: Vec<String> = s.per_rider.iter().map(|d| d.to_string()).collect();
    format!(
        "{label}: {}{}{}{}; trip {}..{} gt; summons lost a day: expired LAST_SLEEP {:.2}, a visit's hole {:.2}, skipped {:.2} (worst day {}, of 240); worst drift {} gt ({} riders ever later), missed rider-nights {}; nights short of {}: {}, fewest distinct in a night {}; late gates {}, waiting room {} ({} days scored); per-rider worst drift: {}",
        o.picker.name(),
        if o.keep_place { ", the napper keeps its place" } else { ", gravity re-entry" },
        if o.gate_lap { format!(", lap-exact gate{}", if o.virtual_count { " + the virtual count" } else { "" }) } else if let Some(nb) = o.gate_batches { format!(", re-entry after the {nb}th batch") } else { o.gate_m.map_or(String::new(), |m| format!(", gated after {m} passes")) },
        if o.skip_per_day > 0 { format!(", {} releases skipped a day", o.skip_per_day) } else { String::new() },
        o.trip.0,
        o.trip.1,
        s.lost_expiry,
        s.lost_hole,
        s.lost_skip,
        s.worst_day,
        s.worst_drift,
        s.drifting_riders,
        s.missed,
        o.visits,
        s.short_nights,
        s.least_distinct,
        s.late_gates,
        s.waiting_max,
        s.days,
        riders.join(" ")
    )
}
