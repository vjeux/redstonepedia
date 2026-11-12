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
//!
//! Simplifications (G1 Nov 11 18:27): a summon here needs all 3 of the stand's batch eligible; the
//! game counts every villager wanting in the summoner's box inflated 10 (at most 5) and needs the
//! summoner itself panicking and wanting, so a waiting rider or a napper in reach can make up the 3
//! (queue-loop-run scores that rule exactly). Summons that find no spawn cell are not modelled. The
//! model's losses run about 0.7x the built W register's (21 against 30 a day; 36 against 48 with
//! fix (a) alone) while its patterns match (G1's v' = v - 4 and v + 14); its zeros hold.

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
    /// The lap-exact gate counted on the virtual clock: re-enter when `carts - 1` virtual slots
    /// (switch passes and other nappers' re-entries) have passed since the pick: one token delay line
    /// on the picker's own clock, no per-napper `a`.
    pub gate_virtual: bool,
    /// The bed counts as free only once the sleeper has re-entered the loop (58f770cc's picker-sim),
    /// not as soon as it is back from the tier.
    pub free_at_reentry: bool,
    /// The dusk re-arm's spread: each night's re-arm (the picker's count and the night's count
    /// reset) falls this many gt after true dusk, uniform in [lo, hi] (a late or early dusk pulse).
    pub rearm_jitter: (i64, i64),
    /// Fault rows (ironspec Nov 11 17:33/17:47):
    /// golem-less cycles: this share of cycles has no golem; with `count_golems` the day counter counts
    /// golem events (so each golem-less cycle before the re-arm delays it a cycle), else every beat.
    pub golemless: f64,
    pub count_golems: bool,
    /// A whole-column freeze (or reload) of LEN gt on day D, before the night's re-arm: no passes, the
    /// counter paused, the re-arm late by LEN.
    pub freeze: Option<(usize, i64)>,
    /// Players sleep on day D: dayTime jumps to dawn mid-night; the next wake reference comes under
    /// 200 cycles after the last one and is refused, so the next night is ALARM (no visits); from then
    /// on dusk falls at a new phase of the golem cycle.
    pub skip_night: Option<usize>,
    /// With `skip_night`: the observer-only sentinel's fallback (the early wake refused: ALARM the next
    /// night); off = a directional source that accepts the early wake.
    pub skip_alarm: bool,
    /// A missed (-1) or double (+1) pass edge on the virtual clock at day D's 6th pick (read before
    /// the 6th pick is made).
    pub pass_fault: Option<(usize, i64)>,
    /// The register's batch-cart index carried across nights (as the built W controller does: visit
    /// j's cart is (all visits so far) mod 3), not reset at dusk; with `visits` above the night's
    /// natural count (99), no 21 cap (the build's 22 a night). G1's calibration (Nov 11 18:27).
    pub register_carry: bool,
    /// One slow visit: the night's 5th pick on every EVERY-th night takes T gt: (EVERY, T).
    pub slow_visit: Option<(usize, i64)>,
    /// The carts the controller's gate assumes (21) when the loop holds fewer (`carts`: a lost cart,
    /// a stranded napper's cart held at the tier's exit); None: the loop's own count. Both the
    /// virtual gate and the lap gate use it.
    pub gate_carts: Option<usize>,
    /// Cart 0 has no rider (a riderless cart carried in the loop): never eligible, diverted like any
    /// pick when its turn comes (nobody sleeps; the tier's give-up timeout makes a normal trip); out
    /// of the rider scores.
    pub empty_cart: bool,
    /// The register diverts the stand's cart after its batch's summon (the built W), not before the
    /// stand: no hole while the napper is back before its next stand turn (G1 Nov 11 19:43).
    pub register_after: bool,
    /// The riderless cart's own trip (divert to tier-clear), when it differs from a napper's: the
    /// tier fires F at its give-up timeout (58f770cc: R 117..129 gt after entry for an arrived-empty
    /// cart); None: the normal range.
    pub empty_trip: Option<(i64, i64)>,
    /// Releases skipped a day (fix (a): 2, so 714 = 34 x 21 carts pass a day).
    pub skip_per_day: usize,
    /// Event-driven cycles (G1's lock-up catch, ironspec Nov 11 20:37/20:38): a cycle starts only at a
    /// golem event (a summon tick, gameTime % 100 = 0, where the stand's batch is all eligible, not
    /// golem-locked, and its pad roll succeeds) or at the watchdog (`watchdog` live gt after the last
    /// cycle start); a failed roll keeps the batch at the stand to retry at the next tick. Off: a
    /// cycle every `period` gt whatever the summon (the old model).
    pub events: bool,
    /// The watchdog's W (live gt after the last cycle start); None with `events`: no watchdog (the
    /// lock-up's positive control).
    pub watchdog: Option<i64>,
    /// A summon tick's pad roll: the chance the stand batch's summon at that tick finds its golem (per
    /// summon tick, not per villager: each wanting villager of the batch makes its own 10 tries and
    /// the first golem stops the rest, so P = 1 - (1 - p)^k over the batch's k wanting villagers;
    /// P = 0.99 measured on v4's pads).
    pub pad_roll: f64,
    /// The batch reaches the stand this many gt after its cycle start (its first summon tick is the
    /// first one at or after it).
    pub arrive: i64,
    /// Event-driven cycles only from this day on (a warm start: the farm ran healthy on fixed beats
    /// before it); 0 = from the start (the cold start).
    pub events_from: usize,
    /// A recovery picker mode (a candidate, off = None): after a watchdog fire inside the night
    /// window, the picker counts K' passes instead of k for the rest of that night.
    pub recover_k: Option<usize>,
    /// Rule 4a's self-lock (farmopt/51b2aacd Nov 11 21:26): a rider whose batch made a golem at T is
    /// golem-locked 600 gt and counts for a summon again only at ticks after T + 600 (a turn 600 gt
    /// later fails, 700 later counts).
    pub lock: bool,
    /// The lock's length: a rider counts again at ticks >= T + lock_gt (600: tick-exact's code read,
    /// Nov 11 21:49: free at exactly T + 600 in either tick order; 700: the old "600 = locked").
    pub lock_gt: i64,
    /// The chance a batch member is re-marked just after its summon (its lock restarts a tick later:
    /// a 600-gt turn then fails, a 700-gt one counts).
    pub remark: f64,
    /// Idle beats: in each group of L beats from the day's start the last I are idle (no release;
    /// the batch stays, locked by its own golem): the 7-beat cycle of 18 villagers (7,1), the 6-beat
    /// cycles of 15 (6,1) and 12 (6,2).
    pub idle: Option<(usize, usize)>,
    /// away at once (None: no cap).
    pub beds: usize,
    pub max_away: Option<usize>,
    /// A night skip (the players sleep): on day D (or every day from D with `jump_every`) at IN_DAY
    /// (the model's day starts at the re-arm, dayTime 13,670; a bed-time skip at dayTime 12,642 is
    /// in_day 22,972, in rain 12,010 is 22,340, a daytime thunder skip at 6,000 is 16,330), dayTime
    /// jumps to dawn (in_day 10,330, the next model day if IN_DAY is past it); game time and every
    /// LAST_SLEEP's age run on.
    pub jump: Option<(usize, i64)>,
    pub jump_every: bool,
    /// The player away (the farm unloaded) on day D from DT gt after dusk for LEN gt: no cycles, the
    /// nappers in the tier frozen with it (their returns LEN later), the watchdog's live count paused,
    /// game time and every LAST_SLEEP's age running on.
    pub absence: Option<(usize, i64, i64)>,
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
    /// Uniform in [0, 1).
    fn f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
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
    /// Event-driven cycles: golems made each day (every day from day 0), the watchdog's fires, the
    /// failed pad rolls, the first golem after the absence ends (gt after), and each night's distinct
    /// nappers (day, count).
    pub golems_by_day: Vec<usize>,
    pub wd_fires: usize,
    pub failed_rolls: usize,
    pub first_golem_after: Option<i64>,
    pub naps_by_night: Vec<(i64, usize)>,
    /// With `lock`: summons lost with every rider fresh but one golem-locked; summon ticks (scored days)
    /// and those with fewer than 21 carts present (circulating, not in the tier).
    pub lock_lost: usize,
    pub summon_ticks: usize,
    pub short_present: usize,
    /// The least spacing between a rider's turns (its batch's release to its next), scored days.
    pub least_spacing: i64,
    /// The scored days that lost more than the scheduled skips: (day, summons lost beyond them).
    pub bad_days: Vec<(i64, usize)>,
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
    let mut jit = 0i64;
    let mut alarm = false;
    let mut total_picks = 0usize;
    let mut shift = 0i64;
    let mut fault_done = false;
    let cycles_per_day = o.day_gt / o.period;
    let skip_from = cycles_per_day * 2 / 3;
    let total_cycles = cycles_per_day * o.days as i64;
    // (event-driven cycles: the last cycle start (the watchdog counts live gt from it), the stand
    // batch's first summon try, the absence's end (the cold start: 0), the day of the last held start
    // (fix (a)'s skips))
    let end_t = total_cycles * o.period;
    let mut last_start: i64 = -o.period;
    let mut stand_from: i64 = 0;
    let mut absence_done = false;
    let mut absence_end: Option<i64> = if o.events && o.absence.is_none() && o.events_from == 0 { Some(0) } else { None };
    let mut skip_day: i64 = i64::MIN;
    let mut night_wd = false;
    let mut locked_until: Vec<i64> = vec![i64::MIN; o.carts];
    let mut last_jump_day: i64 = i64::MIN;
    let mut last_turn: Vec<Option<i64>> = vec![None; o.carts];
    sc.least_spacing = i64::MAX;
    sc.golems_by_day = vec![0; o.days + 1];
    let mut t0: i64 = -o.period;
    loop {
        let mut golem = true;
        let ev = o.events && t0 + o.period >= o.events_from as i64 * o.day_gt;
        if ev {
            let w = o.watchdog.unwrap_or(i64::MAX / 4);
            let stand: Vec<usize> = fifo.iter().take(o.batch).copied().collect();
            let mut s = stand_from.div_euclid(100) * 100;
            if s < stand_from {
                s += 100;
            }
            let mut found = None;
            while s < last_start.saturating_add(w) && s < end_t {
                let elig = stand.len() == o.batch && stand.iter().all(|&b| !(o.empty_cart && b == 0) && last_sleep[b].is_some_and(|ls| s - ls < 24000) && (!o.lock || locked_until[b] <= s));
                if elig {
                    if (rng.next() % 1_000_000) as f64 / 1_000_000.0 < o.pad_roll {
                        found = Some(s);
                        if o.lock {
                            for &b in &stand {
                                let re = (rng.next() % 1_000_000) as f64 / 1_000_000.0 < o.remark;
                                locked_until[b] = s + o.lock_gt + i64::from(re);
                            }
                        }
                        break;
                    }
                    sc.failed_rolls += 1;
                }
                s += 100;
            }
            let next = found.unwrap_or(last_start.saturating_add(w));
            // (the player away from `at` for LEN gt: the farm frozen, game time running: the
            // watchdog's live count, the stand batch's tries and the tier's nappers resume LEN later)
            if let Some((d, dt, len)) = o.absence
                && !absence_done
            {
                let at = o.dusk_at + d as i64 * o.day_gt + dt;
                if next >= at {
                    absence_done = true;
                    absence_end = Some(at + len);
                    last_start += len;
                    stand_from = stand_from.max(at) + len;
                    for a in away.iter_mut() {
                        if a.1 > at {
                            a.1 += len;
                        }
                    }
                    continue;
                }
            }
            if next >= end_t {
                break;
            }
            t0 = next;
            golem = found.is_some();
            if golem {
                if let Some(di) = usize::try_from(t0.div_euclid(o.day_gt)).ok().filter(|d| *d < sc.golems_by_day.len()) {
                    sc.golems_by_day[di] += 1;
                }
                if let Some(e) = absence_end
                    && t0 >= e
                    && sc.first_golem_after.is_none()
                {
                    sc.first_golem_after = Some(t0 - e);
                }
                // (fix (a)'s skips, ironspec's P/B shape: from N_s each day, the first golem event's
                // start is held: stage 1 blocked `skip_per_day` beats (live gt), P latched, then the
                // held start fires; the batch stays at the stand through it (locked by its golem),
                // so `skip_per_day` summon ticks find no new batch and the day keeps 238 releases)
                let rel_s = t0 - o.dusk_at + shift;
                let (sd, sin) = (rel_s.div_euclid(o.day_gt), rel_s.rem_euclid(o.day_gt) / o.period);
                if o.skip_per_day > 0 && sin >= skip_from && skip_day != sd {
                    skip_day = sd;
                    day_lost[2] += o.skip_per_day;
                    t0 += o.skip_per_day as i64 * o.period;
                    golem = false;
                    if t0 >= end_t {
                        break;
                    }
                }
            } else {
                sc.wd_fires += 1;
            }
        } else {
            t0 += o.period;
            if t0 >= end_t {
                break;
            }
            last_start = t0;
            stand_from = t0 + o.arrive;
        }
        let rel = t0 - o.dusk_at + shift;
        let day = rel.div_euclid(o.day_gt);
        let in_day = rel.rem_euclid(o.day_gt);
        let cyc_in_day = in_day / o.period;
        // (a night skip: dayTime jumps to dawn, in_day 10,330, the next model day if the jump is past
        // it; the controller's day counter re-phases on the daylight rise that follows)
        let (day, in_day, cyc_in_day) = if let Some((d, at)) = o.jump
            && (day == d as i64 || o.jump_every && day >= d as i64)
            && in_day >= at
            && last_jump_day != day
        {
            last_jump_day = day;
            shift += if at >= 10_330 { o.day_gt + 10_330 - in_day } else { 10_330 - in_day };
            let rel = t0 - o.dusk_at + shift;
            (rel.div_euclid(o.day_gt), rel.rem_euclid(o.day_gt), rel.rem_euclid(o.day_gt) / o.period)
        } else {
            (day, in_day, cyc_in_day)
        };
        if day != cur_day {
            if cur_day >= o.warmup as i64 {
                sc.lost_expiry += day_lost[0] as f64;
                sc.lost_hole += day_lost[1] as f64;
                sc.lost_skip += day_lost[2] as f64;
                sc.worst_day = sc.worst_day.max(day_lost.iter().sum());
                sc.days += 1;
                let over = day_lost[0] + if o.keep_place { 0 } else { day_lost[1] } + day_lost[2].saturating_sub(o.skip_per_day);
                if over > 0 {
                    sc.bad_days.push((cur_day, over));
                }
            }
            day_lost = [0; 3];
            cur_day = day;
            jit = rng.range(o.rearm_jitter.0, o.rearm_jitter.1);
            // (a day counter that counts golem events: each golem-less cycle before the re-arm
            // (about 136 cycles from the wake reference) delays it a cycle)
            if o.count_golems && o.golemless > 0.0 {
                let mut late = 0i64;
                for _ in 0..136 {
                    if (rng.next() % 1_000_000) as f64 / 1_000_000.0 < o.golemless {
                        late += 1;
                    }
                }
                jit += late * o.period;
            }
            // (a freeze just before dusk: the counter paused, so the re-arm comes late by its length)
            if let Some((d, len)) = o.freeze
                && day == d as i64
            {
                jit += len;
            }
            // (the night after players slept: an observer-only sentinel refuses the early wake (under
            // 200 cycles), ALARM, no visits; a directional source (the detector, the plate) accepts it)
            alarm = o.skip_alarm && o.skip_night.is_some_and(|d| day == d as i64 + 1);
        }
        // (players sleep mid-night on day D: the night ends at the jump; from then on dusk comes earlier
        // in gameTime by day - night/2 - dusk's day time, a new phase of the golem cycle)
        if let Some(d) = o.skip_night
            && day == d as i64
            && in_day >= o.night_gt / 2
            && in_day < o.night_gt
            && shift == 0
        {
            shift = o.day_gt - o.night_gt / 2 - 13670;
        }
        let in_night = !alarm && in_day >= jit && in_day < o.night_gt && !(o.skip_night.is_some_and(|d| day == d as i64) && in_day >= o.night_gt / 2);
        // (frozen: no passes and no summons for the freeze's length before dusk; event-driven cycles
        // use `absence` instead)
        if let Some((d, len)) = o.freeze
            && !ev
            && day + 1 == d as i64
            && in_day >= o.day_gt - len
        {
            day_lost[2] += 1;
            continue;
        }
        if in_night && night_idx != Some(day) {
            night_idx = Some(day);
            night_picks.clear();
            night_releases = 0;
            count = 0;
            night_wd = false;
        }
        if in_night && ev && !golem {
            night_wd = true;
        }
        if !in_night && night_idx == Some(day) && night_picks.len() < usize::MAX {
            // (the night just ended: score it once)
            let mut seen = vec![false; o.carts];
            night_picks.iter().for_each(|&p| seen[p] = true);
            sc.naps_by_night.push((day, seen.iter().filter(|s| **s).count()));
            if day >= o.warmup as i64 {
                sc.least_distinct = sc.least_distinct.min(seen.iter().filter(|s| **s).count());
                if night_picks.len() < o.visits {
                    sc.short_nights += 1;
                }
            }
            night_idx = Some(-1 - day);
        }
        if ev {
            // (event-driven: fix (a)'s skips are the held start above; this start resets the
            // watchdog's count and brings the next batch to the stand)
            last_start = t0;
            stand_from = t0 + o.arrive;
        } else if o.skip_per_day > 0 && cyc_in_day >= skip_from && cyc_in_day < skip_from + o.skip_per_day as i64 {
            day_lost[2] += 1;
            continue;
        }
        // (idle beats: no release, the batch stays at the stand, locked by its own golem)
        if let Some((l, i)) = o.idle
            && cyc_in_day % l as i64 >= (l - i) as i64
        {
            day_lost[2] += 1;
            continue;
        }
        let mut members: Vec<Option<bool>> = Vec::new();
        // (rule 4a's self-lock: the batch's carts, whether a fresh rider was golem-locked; the carts
        // present (circulating, not in the tier) at this summon tick)
        let mut batch_ids: Vec<usize> = Vec::new();
        let mut lock_hit = false;
        if day >= o.warmup as i64 {
            sc.summon_ticks += 1;
            if o.carts.saturating_sub(away.len()) < 21 {
                sc.short_present += 1;
            }
        }
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
                // (each napper still away sees this re-entry as a virtual slot)
                if o.gate_virtual {
                    for a in away.iter_mut() {
                        a.2 += 1;
                    }
                }
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
            batch_ids.push(cart);
            if day >= o.warmup as i64
                && let Some(lt) = last_turn[cart]
            {
                sc.least_spacing = sc.least_spacing.min(t0 - lt);
            }
            last_turn[cart] = Some(t0);
            for a in away.iter_mut() {
                a.2 += 1;
            }
            anchor_passes += 1;
            count += 1;
            // (keep-place: the napper's own slot passes as a hole while it's away; each rider's
            // eligibility judged at the cycle's summon, before this pass's pick re-arms it; the bed
            // free once every napper is back from it, gated or not)
            let hole = o.keep_place && away.iter().any(|a| a.0 == cart);
            let fresh = !(o.empty_cart && cart == 0) && last_sleep[cart].is_some_and(|ls| t0 - ls < 24000);
            let unlocked = !o.lock || locked_until[cart] <= t0;
            if fresh && !unlocked {
                lock_hit = true;
            }
            let elig = fresh && unlocked;
            let in_bed = away.iter().filter(|a| a.1 > t).count();
            let bed_free = (if o.free_at_reentry { away.len() < o.beds.max(1) } else { in_bed < o.beds.max(1) }) && o.max_away.is_none_or(|m| away.len() < m);
            let want = in_night && bed_free && night_picks.len() < o.visits && !hole && match o.picker {
                Picker::FromPick(k) | Picker::FromReturn(k) => count >= if night_wd { o.recover_k.unwrap_or(k) } else { k },
                Picker::Anchored(k, off) => if night_picks.is_empty() { anchor.is_none() && count >= k || anchor.is_some() && anchor_passes % o.carts == off % o.carts } else { count >= k },
                Picker::Register(every) => j == (if o.register_carry { total_picks } else { night_picks.len() }) % 3 && night_releases % every == 0,
            };
            if want {
                // (a missed or double pass edge on the virtual clock at day D's 5th pick: the picker's
                // count and every in-flight napper's gate count off by one from here)
                if let Some((d, f)) = o.pass_fault
                    && day == d as i64
                    && night_picks.len() == 5
                    && !fault_done
                {
                    fault_done = true;
                    for a in away.iter_mut() {
                        a.2 = (a.2 as i64 + f).max(0) as usize;
                    }
                    count = (count as i64 + f).max(0) as usize;
                }
                night_picks.push(cart);
                total_picks += 1;
                // (the riderless cart is diverted like any pick (lap-exact re-entry on its token) but
                // nobody sleeps: the tier's F fires at its give-up timeout, so its trip is a normal
                // one; ironspec/farmopt Nov 11 19:40)
                if !(o.empty_cart && cart == 0) {
                    last_sleep[cart] = Some(t + o.sleep_at);
                    naps[cart].push(t + o.sleep_at);
                }
                let gate = if o.gate_virtual {
                    Some(o.gate_carts.unwrap_or(o.carts) - 1)
                } else if o.gate_lap {
                    Some((o.gate_carts.unwrap_or(o.carts) as i64 - 1 - away.len() as i64 + o.gate_lap_off).max(0) as usize)
                } else if let Some(nb) = o.gate_batches {
                    Some(o.batch - 1 - j + nb * o.batch)
                } else {
                    o.gate_m
                };
                let (tlo, thi) = if o.empty_cart && cart == 0 { o.empty_trip.unwrap_or(o.trip) } else { o.trip };
                let drawn = rng.range(tlo, thi);
                // (`slow_visit`: the night's 5th pick on every EVERY-th night takes T gt, e.g. the
                // tier's HOME-lost fault, ironspec Nov 11 18:49)
                let slow = o.slow_visit.is_some_and(|(every, _)| every > 0 && day >= 0 && day % every as i64 == 0) && night_picks.len() == 5;
                away.push((cart, t + if slow { o.slow_visit.map_or(drawn, |s| s.1) } else { drawn }, 0, gate));
                count = 0;
                // (the register diverts before the stand: its batch lacks the napper, unless
                // `register_after` (the built W: the stand's cart after its batch's summon, back in
                // its own slot before its next turn, else a hole then); gravity picks after the
                // summon, from the batch that made it)
                members.push(if o.keep_place && !o.register_after { None } else { Some(elig) });
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
        let ok = if ev { golem } else { members.len() == o.batch && members.iter().all(|m| *m == Some(true)) };
        if !ev && o.lock {
            if ok {
                for &c in &batch_ids {
                    let re = (rng.next() % 1_000_000) as f64 / 1_000_000.0 < o.remark;
                    locked_until[c] = t0 + o.lock_gt + i64::from(re);
                }
            } else if lock_hit && day >= o.warmup as i64 {
                sc.lock_lost += 1;
            }
        }
        // (fixed beats: a golem when the batch is all eligible; event-driven: counted at the event)
        if ok
            && !ev
            && let Some(di) = usize::try_from(t0.div_euclid(o.day_gt)).ok().filter(|d| *d < sc.golems_by_day.len())
        {
            sc.golems_by_day[di] += 1;
        }
        if !ok {
            if members.iter().any(|m| m.is_none()) { day_lost[1] += 1 } else { day_lost[0] += 1 }
        }
    }
    // (drift: each rider's nap later than 24,000 after its last one, over the scored days; a
    // scored night with no nap is a missed night)
    let from = o.dusk_at + o.warmup as i64 * o.day_gt;
    let scored_nights = o.days.saturating_sub(o.warmup + 1);
    for (ci, n) in naps.iter().enumerate() {
        // (the riderless cart has no rider: out of the rider scores)
        if o.empty_cart && ci == 0 {
            continue;
        }
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

/// Event-driven cycles' report: golems a day over the scored days, the watchdog's fires and the failed
/// rolls a day, and from the event (the absence's end, else the start): the first golem, each night's
/// distinct nappers, each day's golems, and the golems lost against `normal` a day (default: the mean
/// of the scored days before the event).
pub fn event_line(label: &str, o: &Opts, s: &Score, normal: Option<f64>) -> String {
    let g = &s.golems_by_day;
    let scored: Vec<usize> = g.iter().copied().skip(o.warmup).take(o.days.saturating_sub(o.warmup)).collect();
    let mean = |v: &[usize]| if v.is_empty() { 0.0 } else { v.iter().sum::<usize>() as f64 / v.len() as f64 };
    let ev_t = o.absence.map_or(0, |(d, dt, len)| o.dusk_at + d as i64 * o.day_gt + dt + len);
    let ev_day = (ev_t.div_euclid(o.day_gt)).max(0) as usize;
    let before: Vec<usize> = g.iter().copied().skip(o.warmup).take(ev_day.saturating_sub(o.warmup)).collect();
    let norm = normal.unwrap_or(mean(&before));
    let after: Vec<usize> = g.iter().copied().skip(ev_day).take(5).collect();
    let lost: f64 = after.iter().map(|&x| (norm - x as f64).max(0.0)).sum();
    let nights: Vec<(i64, usize)> = s.naps_by_night.iter().copied().filter(|(d, _)| *d >= ev_day as i64).take(5).collect();
    let full = nights.iter().position(|(_, n)| *n >= o.carts.min(o.visits));
    let days_n = o.days as f64;
    format!(
        "  {label}: golems a day {:.1} (scored days: least {}, most {}); watchdog fires {:.2} a day, failed pad rolls {:.2} a day; from the event (t {ev_t}, day {ev_day}): first golem {}; distinct nappers each night {}; golems each day {}; golems lost over those days {:.0} (against {:.1} a day); all {} napped in a night {}",
        mean(&scored),
        scored.iter().min().copied().unwrap_or(0),
        scored.iter().max().copied().unwrap_or(0),
        s.wd_fires as f64 / days_n,
        s.failed_rolls as f64 / days_n,
        s.first_golem_after.map_or("never".to_string(), |t| format!("{t} gt after")),
        nights.iter().map(|(d, n)| format!("n{d} {n}")).collect::<Vec<_>>().join(", "),
        after.iter().enumerate().map(|(i, x)| format!("d{} {x}", ev_day + i)).collect::<Vec<_>>().join(", "),
        lost,
        norm,
        o.carts.min(o.visits),
        full.map_or("not within 5 nights".to_string(), |i| format!("on the {} night from the event", ["1st", "2nd", "3rd", "4th", "5th"][i]))
    )
}

/// Q2 (design-space v0, Nov 11): `lanes` lanes in lockstep, one release line opening every lane's
/// gate at once: a batch is each lane's front cart; after its summon each cart returns to its own lane
/// (its back) unless the one shared divert on the returns (counting the `lanes` returns a cycle in lane
/// order) picks it; the napper re-enters its own lane: `lane_exact` = right behind the cart that went
/// before it in its lane, as that cart returns on its next lap (if the napper is back by then, else the
/// lap after); else at its lane's back once it is back. Fixed 100-gt beats; rule 4a's self-lock with
/// `lock`; fix (a)'s skips (no release: the batch stays, locked by its own golem).
#[derive(Clone, Debug)]
pub struct LaneOpts {
    pub lanes: usize,
    pub per_lane: usize,
    pub k: usize,
    /// Beds: 1 (shared, the picker counts the shared returns), or `lanes` (Z4: each lane owns a bed
    /// and picks its own riders on its own count of its returns, k then being the lane's count).
    pub beds: usize,
    /// One bed with per-lane counting (ironspec Nov 11 22:07): each lane's own count (k) picks its
    /// nappers, the one bed serving one napper at a time, the lanes taking turns: 1 = in sequence
    /// (lane A's quota, then B's, then C's), 2 = round-robin; 0 = off.
    pub turns: u8,
    pub visits: usize,
    pub lane_exact: bool,
    /// The napper keeps its slot in its lane (fixed slots, or an equal-length bypass rejoining in
    /// place): its slot passes as a hole if it is still away at its turn.
    pub keep_place: bool,
    pub skip_per_day: usize,
    pub lock: bool,
    pub trip: (i64, i64),
    pub sleep_at: i64,
    pub spacing: i64,
    pub cross_at: i64,
    pub days: usize,
    pub warmup: usize,
    pub seed: u64,
    pub night_gt: i64,
    pub day_gt: i64,
    pub dusk_at: i64,
    /// A night skip, as `Opts::jump`: (day D, in_day of the jump), every day from D with `jump_every`.
    pub jump: Option<(usize, i64)>,
    pub jump_every: bool,
    /// Strict rhythm: a strand outlier, one trip + ADD gt every EVERY gt of play: (EVERY, ADD).
    pub outlier: Option<(i64, i64)>,
    /// Strict rhythm: the lift's foot to the lane hold, gt (the trip range being the bed's cycle).
    pub post: i64,
    /// Strict rhythm: the napper's foot (ironspec/farmopt Nov 11 23:03): 0 = the trip range is the
    /// whole bed cycle and the queue releases at its end; 1 = (a) the foot gate: R (the trip range
    /// is entry -> R), then `r_leg` to the foot gate, which opens only inside `window` (gt after a
    /// release, mod 100), then `det` to the evidence detector, where the queue releases, then
    /// `post`; 2 = (c) the napper-only lift: the queue releases as the cart leaves R, then `r_leg`
    /// to the merge-level hold, released only inside `window`, then `post`.
    pub foot: u8,
    pub r_leg: i64,
    pub window: (i64, i64),
    pub det: i64,
    /// Strict rhythm: the stop's entry to the lie-down, gt (58f770cc Nov 11 23:10: the fastest 80).
    pub lie: i64,
    /// The day time of the model's in_day 0, the pick window's start (13,670; ironspec Nov 11 23:21:
    /// REST begins at 12,000, so ~12,150..12,200); dawn falls at in_day day_gt - start_dt.
    pub start_dt: i64,
    /// After a night skip, the next window opens this much later (gt): the counter re-phases on the
    /// detector's rise at day 170..188 instead of the first morning rise at 22,331 (tick-exact's
    /// daylight bridge, Nov 11 23:26), so that day's decodes land ~1,840 gt late.
    pub rephase_lag: i64,
    /// The window after a skip keeps only its first N releases' picks (the daytime thunderstorm skip
    /// as is: the count a day off, the window open by day under the daylight inhibit, its tail at night).
    pub truncate_after_skip: Option<i64>,
    /// A napper held at the bed queue (D, IN_DAY, ADD): the first pick at or after day D's IN_DAY
    /// gets +ADD gt on its trip (a skip's daylight napper waiting for dusk behind the NOT-DAY gate).
    pub hold_at: Option<(usize, i64, i64)>,
    /// Rule 2 (ironfarm, ironspec 03:09): a napper rejoins only at a pass of its predecessor it was
    /// already waiting for; a late one waits a lane lap for the next.
    pub rule2: bool,
    /// Record every release (cycle, lane, cart) in `LaneStats::release_log` (the lane-order test).
    pub log_releases: bool,
    /// Strict rhythm: the controller's siding rule (ironspec Nov 11 23:54): a napper rejoins only at
    /// its lane's phase window (its pick's offset + lanes' length - 1, mod k) behind the cart passing
    /// then; a late napper waits a whole window (one period) and rejoins behind that period's cart.
    pub siding_window: bool,
    /// A batch cart's time away from its lane after its release: the stand's cycle + the return to
    /// the lane's back (gt; ironfarm's lanes-bench --ret 150 + 100), for the lane's occupancy at
    /// every arrival.
    pub ret: i64,
    /// The self-lock's length (600: tick-exact's "free at exactly T+600"), as `Opts::lock_gt`.
    pub lock_gt: i64,
    /// The share of a batch's members re-marked just after their summon (locked until
    /// T + lock_gt + 1), as `Opts::remark`.
    pub remark: f64,
}

/// The strict rhythm's numbers beside its line (the strand-margin search reads them).
#[derive(Clone, Debug, Default)]
pub struct LaneStats {
    pub golems: f64,
    pub max_queue: usize,
    pub max_trip: i64,
    /// The latest lie-down's in_day (from the model's night start) over the scored nights.
    pub latest_lie: i64,
    /// Lie-downs after the night's end (the nap fails: LAST_SLEEP is not refreshed).
    pub late_naps: usize,
    /// The busiest night's bed occupancy (entry to the queue's release, summed) over the pick
    /// window (per_lane * k cycles).
    pub util_max: f64,
    /// Summons lost to an expired LAST_SLEPT over the scored days (golems), and the largest
    /// LAST_SLEPT age any villager reached (gt; >= 24,000 = out of the summons).
    pub lost_expiry: f64,
    pub max_age: i64,
    /// Every scored lie-down: (night, cart, in_day) — the order drift after an event against a run
    /// without it; with `log_releases`, every release (cycle, lane, cart) for the lane-order check.
    pub nap_log: Vec<(i64, usize, i64)>,
    pub release_log: Vec<(i64, usize, usize)>,
    /// The shortest lane at any scored summon.
    pub min_lane: usize,
    /// Picks due while the lane's napper was still away (skipped: never two on a siding).
    pub busy_skips: u32,
    pub late_nappers: u32,
}

impl LaneOpts {
    /// The F1 + Q2 lead's lane model as benched (58f770cc's stop: trip 196..198, lie +80, night to
    /// 23,920; foot (c) with the hold window 55..99; ret 175; k 10; the controller's siding windows).
    pub fn lead() -> LaneOpts {
        LaneOpts {
            lanes: 3, per_lane: 8, k: 10, beds: 1, turns: 3, visits: 24, lane_exact: true, keep_place: false,
            skip_per_day: 0, lock: true, trip: (196, 198), sleep_at: 40, spacing: 22, cross_at: 60, days: 30,
            warmup: 5, seed: 7, night_gt: 11_489, day_gt: 24_000, dusk_at: 0, jump: None, jump_every: false,
            outlier: None, post: 10, foot: 2, r_leg: 40, window: (55, 99), det: 2, lie: 80, start_dt: 12_431,
            rephase_lag: 0, truncate_after_skip: None, hold_at: None, rule2: false, log_releases: false,
            siding_window: true, ret: 175, lock_gt: 600, remark: 0.0,
        }
    }
}

/// The wait at a gate opening only inside `w` (gt after a release, mod 100).
fn gate_wait(x: i64, w: (i64, i64)) -> i64 {
    let ph = x.rem_euclid(100);
    if ph >= w.0 && ph <= w.1 {
        0
    } else if ph < w.0 {
        w.0 - ph
    } else {
        100 + w.0 - ph
    }
}

/// The lane model's report line.
pub fn simulate_lanes(o: &LaneOpts) -> String {
    simulate_lanes_scored(o).0
}

/// The lane model's report line and its numbers.
pub fn simulate_lanes_scored(o: &LaneOpts) -> (String, LaneStats) {
    let mut rng = Rng(o.seed);
    let n = o.lanes * o.per_lane;
    let mut lanes: Vec<VecDeque<usize>> = (0..o.lanes).map(|l| (0..o.per_lane).map(|i| l + i * o.lanes).collect()).collect();
    let lane_of = |c: usize| c % o.lanes;
    let mut last_sleep: Vec<Option<i64>> = vec![None; n];
    let mut locked_until: Vec<i64> = vec![i64::MIN; n];
    // (the sleepers: cart, back from the bed at, the cart it re-enters behind (lane-exact))
    let mut away: Vec<(usize, i64, usize)> = Vec::new();
    let (mut count, mut night_picks): (usize, Vec<usize>) = (0, Vec::new());
    let mut lane_count: Vec<usize> = vec![0; o.lanes];
    let mut lane_picks: Vec<usize> = vec![0; o.lanes];
    let per_bed = o.beds >= o.lanes || o.turns > 0;
    let mut cur_lane = 0usize;
    // (strict rhythm, turns 3: the night's first cycle; the one bed's queue: when it frees)
    let mut night_c0: i64 = 0;
    let mut bed_free_at: i64 = i64::MIN;
    let mut bed_q: Vec<(i64, i64)> = Vec::new();
    let (mut max_queue, mut outliers) = (0usize, 0usize);
    let mut max_trip: i64 = 0;
    let (mut latest_lie, mut late_naps, mut util_max) = (0i64, 0usize, 0f64);
    let mut night_busy: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut next_outlier: i64 = o.outlier.map_or(i64::MAX, |(every, _)| every / 2);
    let mut lane_len_hist: std::collections::BTreeMap<usize, u64> = std::collections::BTreeMap::new();
    let mut night_idx: Option<i64> = None;
    let (mut golems, mut lost_lock, mut lost_expiry, mut lost_short, mut lost_skip) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // (the largest LAST_SLEPT age any villager reaches over the scored days, ironspec Nov 12 02:10)
    let mut max_age = 0i64;
    let mut hold_used = false;
    let mut busy_skips = 0u32;
    let mut nap_log: Vec<(i64, usize, i64)> = Vec::new();
    let mut release_log: Vec<(i64, usize, usize)> = Vec::new();
    let (mut least_distinct, mut short_nights, mut min_lane, mut ticks_below) = (usize::MAX, 0usize, usize::MAX, 0usize);
    let mut scored_ticks = 0usize;
    let cycles_per_day = o.day_gt / 100;
    let skip_from = cycles_per_day * 2 / 3;
    let mut held = false;
    let (mut shift, mut last_jump_day) = (0i64, i64::MIN);
    let mut lag_day: Option<i64> = None;
    let (mut min_occ, mut late_nappers, mut max_inflight, mut max_fifo) = (usize::MAX, 0usize, 0usize, 0usize);
    let mut pick_cycle: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    let mut last_turn: Vec<Option<i64>> = vec![None; n];
    let mut least_spacing = i64::MAX;
    let mut golems_day: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for c in 0..cycles_per_day * o.days as i64 {
        let t0 = c * 100;
        let rel = t0 - o.dusk_at + shift;
        let (mut day, mut in_day) = (rel.div_euclid(o.day_gt), rel.rem_euclid(o.day_gt));
        let dawn_in = (o.day_gt - o.start_dt).rem_euclid(o.day_gt);
        if let Some((d, at)) = o.jump
            && (day == d as i64 || o.jump_every && day >= d as i64)
            && in_day >= at
            && last_jump_day != day
        {
            last_jump_day = day;
            shift += if at >= dawn_in { o.day_gt + dawn_in - in_day } else { dawn_in - in_day };
            let rel = t0 - o.dusk_at + shift;
            (day, in_day) = (rel.div_euclid(o.day_gt), rel.rem_euclid(o.day_gt));
            // (the next window after the landing at dawn is the next model day's)
            lag_day = Some(day + 1);
        }
        let scored = day >= o.warmup as i64;
        // (the day after a night skip: the window opens `rephase_lag` late)
        let lag = if lag_day == Some(day) { o.rephase_lag } else { 0 };
        let in_night = in_day >= lag && in_day < o.night_gt;
        if in_night && night_idx != Some(day) {
            night_idx = Some(day);
            night_picks.clear();
            count = 0;
            lane_count = vec![0; o.lanes];
            lane_picks = vec![0; o.lanes];
            cur_lane = 0;
            night_c0 = c;
        }
        if !in_night && night_idx == Some(day) {
            if scored {
                let mut seen = vec![false; n];
                night_picks.iter().for_each(|&p| seen[p] = true);
                let d = seen.iter().filter(|s| **s).count();
                least_distinct = least_distinct.min(d);
                if night_picks.len() < o.visits {
                    short_nights += 1;
                }
            }
            night_idx = Some(-1 - day);
        }
        // (the batch at the stand: each lane's front; it summons at t0 unless held by a skip, when
        // the same batch stays a beat, locked by its own golem)
        let batch: Vec<Option<usize>> = lanes.iter().map(|l| l.front().copied()).collect();
        let lens_min = lanes.iter().map(|l| l.len()).min().unwrap_or(0);
        if scored {
            scored_ticks += 1;
            min_lane = min_lane.min(lens_min);
            if lens_min + 1 < o.per_lane {
                ticks_below += 1;
            }
            for ln in lanes.iter() {
                *lane_len_hist.entry(ln.len()).or_default() += 1;
            }
        }
        let fresh = |b: usize| last_sleep[b].is_some_and(|ls| t0 - ls < 24000);
        if scored {
            for ls in last_sleep.iter().flatten() {
                max_age = max_age.max(t0 - ls);
            }
        }
        let unlocked = |b: usize| !o.lock || locked_until[b] <= t0;
        // (keep-place: a napper still away at its slot's turn is a hole)
        let all = batch.iter().all(|b| b.is_some_and(|b| !o.keep_place || !away.iter().any(|a| a.0 == b)));
        let ok = all && batch.iter().all(|b| b.is_some_and(|b| fresh(b) && unlocked(b)));
        if ok {
            if scored {
                golems += 1;
            }
            *golems_day.entry(day).or_default() += 1;
            for b in batch.iter().flatten() {
                // (ironspec Nov 12 00:10: the lock's length and the re-mark share as in `Opts`)
                let remarked = o.remark > 0.0 && rng.f64() < o.remark;
                locked_until[*b] = t0 + o.lock_gt + i64::from(remarked);
            }
        } else if scored {
            if held {
                lost_skip += 1;
            } else if !all {
                lost_short += 1;
            } else if batch.iter().flatten().any(|&b| fresh(b) && !unlocked(b)) {
                lost_lock += 1;
            } else {
                lost_expiry += 1;
            }
        }
        held = false;
        if o.skip_per_day > 0 && in_day / 100 >= skip_from && in_day / 100 < skip_from + o.skip_per_day as i64 {
            held = true;
            continue;
        }
        // (each rider's turn spacing: its batch's release to its next one)
        for b in batch.iter().flatten() {
            if scored && let Some(lt) = last_turn[*b] {
                least_spacing = least_spacing.min(t0 - lt);
            }
            last_turn[*b] = Some(t0);
        }
        // (the release: each lane's front leaves and returns in lane order past the shared divert)
        for l in 0..o.lanes {
            let Some(cart) = lanes[l].pop_front() else { continue };
            if o.log_releases {
                release_log.push((c, l, cart));
            }
            let t = t0 + o.cross_at + l as i64 * o.spacing;
            count += 1;
            lane_count[l] += 1;
            // (one shared bed: free once every napper is back from it; Z4: one napper per lane at a
            // time (in its bed or waiting to re-enter), farmopt Nov 11 21:40)
            let want = if o.turns == 3 {
                // (strict rhythm: lane l picks every k cycles at offset l*k/lanes from the night's
                // start, whatever the bed's state (the bed stop queues); one napper per lane. With
                // lane-exact re-entry the napper misses one turn, so a lane's picks advance k - (lane
                // length - 1) places: k must make that coprime with the lane length (8: k 8, 10))
                let lane_free = away.iter().all(|a| lane_of(a.0) != l);
                let period = o.k as i64;
                let due = in_night && lane_picks[l] < o.per_lane && (c - night_c0).rem_euclid(period) == (l * o.k / o.lanes) as i64
                    // (the window after a skip cut to its first N releases: a daytime thunderstorm
                    // skip leaves the count a day off, so only the window's tail is at night)
                    && !(lag_day == Some(day) && o.truncate_after_skip.is_some_and(|n| c - night_c0 >= n));
                // (a pick due while the lane's previous napper is still away (in the bed or waiting on
                // its siding) is skipped: one napper per lane, the siding never double-occupied)
                if due && !lane_free && scored {
                    busy_skips += 1;
                }
                due && lane_free
            } else if per_bed {
                let lane_free = away.iter().all(|a| lane_of(a.0) != l);
                // (one bed shared by turns: nobody in it, and this lane's turn)
                let bed_ok = o.turns == 0 || (away.iter().all(|a| a.1 <= t) && l == cur_lane);
                in_night && lane_free && bed_ok && lane_picks[l] < o.per_lane && lane_count[l] >= o.k
            } else {
                let bed_free = away.iter().all(|a| a.1 <= t);
                in_night && bed_free && night_picks.len() < o.visits && count >= o.k
            };
            if want {
                night_picks.push(cart);
                lane_picks[l] += 1;
                lane_count[l] = 0;
                // (the lane turn: round-robin moves on after each pick, the sequence once the lane's
                // quota is napped; lanes whose quota is done are passed over)
                if o.turns > 0 && (o.turns == 2 || lane_picks[l] >= o.per_lane) {
                    for step in 1..=o.lanes {
                        let nl = (l + step) % o.lanes;
                        if lane_picks[nl] < o.per_lane {
                            cur_lane = nl;
                            break;
                        }
                    }
                }
                let prev_sleep = last_sleep[cart];
                last_sleep[cart] = Some(t + o.sleep_at);
                let pred = lanes[l].back().copied().unwrap_or(cart);
                let drawn = rng.range(o.trip.0, o.trip.1);
                // (strict rhythm: the one bed queues: 60 gt to the bed, sleep (trip - 120) from when
                // the bed frees, 60 back; LAST_SLEEP at the lie-down)
                let back = if o.turns == 3 {
                    // (a strand outlier: one bed cycle + ADD every EVERY gt of play)
                    let extra = if let Some((every, add)) = o.outlier
                        && t >= next_outlier
                    {
                        next_outlier = t + every;
                        outliers += 1;
                        add
                    } else {
                        0
                    };
                    // (a napper held at the bed queue until dusk: the first pick at or after day D's
                    // IN_DAY gets +ADD on its trip (ironspec Nov 12 02:28: the queue's NOT-DAY gate))
                    let extra = extra
                        + match o.hold_at {
                            Some((hd, hat, add)) if !hold_used && day == hd as i64 && in_day >= hat => {
                                hold_used = true;
                                add
                            }
                            _ => 0,
                        };
                    // (ironspec Nov 11 22:46: the trip range is the bed's cycle, entry -> R -> the
                    // returning napper at the lift's foot (the queue releases on that evidence); 60 gt
                    // from the divert to the stop's entry before it, `post` from the lift's foot to the
                    // lane hold after it)
                    let arrive = t + 60;
                    let start = arrive.max(bed_free_at);
                    // (the bed queue: nappers arrived and still waiting at this arrival)
                    bed_q.retain(|(_, s)| *s > arrive);
                    let depth = bed_q.iter().filter(|(a, s)| *a <= arrive && *s > arrive).count() + usize::from(start > arrive);
                    max_queue = max_queue.max(depth);
                    let cycle = (drawn + extra).max(1);
                    // (the foot: where the queue releases and what the trip adds after R)
                    let r = start + cycle;
                    let (free, back) = match o.foot {
                        1 => {
                            let foot = r + o.r_leg;
                            let pass = foot + gate_wait(foot, o.window) + o.det;
                            (pass, pass + o.post)
                        }
                        2 => {
                            let hold = r + o.r_leg;
                            (r, hold + gate_wait(hold, o.window) + o.post)
                        }
                        _ => (r, r + o.post),
                    };
                    bed_free_at = free;
                    bed_q.push((arrive, start));
                    // (the lie-down, `lie` gt after the entry: past the night's end the nap fails)
                    let lrel = start + o.lie - o.dusk_at + shift;
                    let lie_in = lrel.rem_euclid(o.day_gt);
                    let lie_day = lrel.div_euclid(o.day_gt);
                    if lie_in >= o.night_gt {
                        last_sleep[cart] = prev_sleep;
                        if scored {
                            late_naps += 1;
                        }
                    } else {
                        last_sleep[cart] = Some(start + o.lie);
                        if scored {
                            latest_lie = latest_lie.max(lie_in);
                            nap_log.push((lie_day, cart, lie_in));
                        }
                    }
                    if scored {
                        *night_busy.entry(lie_day).or_default() += free - start;
                    }
                    max_trip = max_trip.max(back - t);
                    back
                } else {
                    t + drawn
                };
                away.push((cart, back, pred));
                pick_cycle.insert(cart, c);
                count = 0;
                if o.keep_place {
                    lanes[l].push_back(cart);
                }
            } else {
                lanes[l].push_back(cart);
                // (the lane's occupancy when this cart arrives at its back, `ret` gt after its
                // release: its lane's carts minus itself, the carts released after it and still away,
                // and the lane's nappers away then)
                if o.turns == 3 && scored {
                    let arr = t0 + o.ret;
                    let in_transit = ((o.ret - 1).max(0) / 100) as usize;
                    let nap_away = away.iter().filter(|a| lane_of(a.0) == l && a.1 > arr - o.ret).count();
                    let occ = o.per_lane.saturating_sub(1 + in_transit + nap_away);
                    min_occ = min_occ.min(occ);
                }
                // (lane-exact: a napper back from the bed re-enters right behind this cart, the one
                // that went before it in its lane)
                if o.lane_exact && o.siding_window && o.rule2 {
                    // (rule 2, ironspec 03:09/03:27: a napper rejoins K gt after its predecessor's merge
                    // pass only if it was already on its siding at that pass; one that arrives later
                    // waits for the predecessor's next pass, a lane lap later. The predecessor (released
                    // just before the pick) comes round at the pick + the lane's length - 1 releases
                    // (the napper's own release still in that lap), then every lane length - 1 (7 carts))
                    let lap = o.per_lane as i64 - 1;
                    while let Some(i) = away.iter().position(|a| {
                        a.1 <= t && lane_of(a.0) == l && pick_cycle.get(&a.0).is_some_and(|pc| {
                            let dd = c - pc;
                            dd >= lap && (dd - lap) % lap == 0
                        })
                    }) {
                        let (id, _, _) = away.remove(i);
                        if scored && pick_cycle.get(&id).is_some_and(|pc| c > pc + lap) {
                            late_nappers += 1;
                        }
                        lanes[l].push_back(id);
                    }
                } else if o.lane_exact && o.siding_window {
                    // (the controller's rule: only at the lane's phase window (the pick's offset + the
                    // lane's length - 1, mod k), behind whichever cart passes then)
                    let win = ((l * o.k / o.lanes) as i64 + o.per_lane as i64 - 1).rem_euclid(o.k as i64);
                    if (c - night_c0).rem_euclid(o.k as i64) == win {
                        while let Some(i) = away.iter().position(|a| a.1 <= t && lane_of(a.0) == l) {
                            let (id, _, _) = away.remove(i);
                            if scored && pick_cycle.get(&id).is_some_and(|pc| c > pc + o.per_lane as i64 - 1) {
                                late_nappers += 1;
                            }
                            lanes[l].push_back(id);
                        }
                    }
                } else if o.lane_exact {
                    while let Some(i) = away.iter().position(|a| a.2 == cart && a.1 <= t && lane_of(a.0) == l) {
                        let (id, _, _) = away.remove(i);
                        lanes[l].push_back(id);
                    }
                }
            }
            if scored {
                max_inflight = max_inflight.max(away.len());
                // (the lane FIFO holds a napper's lane from its pick to its split pass, about when it
                // is back at the merge level: those not yet back)
                max_fifo = max_fifo.max(away.iter().filter(|a| a.1 > t).count());
            }
            if o.keep_place {
                // (its slot never left the lane: back from the bed, it just takes its slot again)
                away.retain(|a| a.1 > t);
            } else if !o.lane_exact {
                while let Some(i) = away.iter().position(|a| a.1 <= t && lane_of(a.0) == l) {
                    let (id, _, _) = away.remove(i);
                    lanes[l].push_back(id);
                }
            }
        }
    }
    let days = o.days.saturating_sub(o.warmup).max(1) as f64;
    let window_gt = (o.per_lane * o.k) as f64 * 100.0;
    for busy in night_busy.values() {
        util_max = util_max.max(*busy as f64 / window_gt);
    }
    let stats = LaneStats { golems: golems as f64 / days, max_queue, max_trip, latest_lie, late_naps, util_max, lost_expiry: lost_expiry as f64, max_age, nap_log, release_log, min_lane: if min_lane == usize::MAX { 0 } else { min_lane }, busy_skips, late_nappers: late_nappers as u32 };
    let per_day: Vec<String> = golems_day.iter().filter(|(d, _)| **d >= o.warmup as i64 - 1).map(|(d, g)| format!("d{d} {g}")).collect();
    let lens: Vec<String> = lane_len_hist.iter().map(|(l, n)| format!("{l}:{n}")).collect();
    let foot = if o.turns == 3 {
        format!(
            "; the foot: {}; the latest lie-down at in_day {} of the night's {} (night margin {:.1}%), lie-downs past the night {}; the busiest night's bed occupancy {:.1}% of the {}-gt pick window (margin {:.1}%)",
            match o.foot {
                1 => format!("(a) the foot gate (R + {} gt, open {}..{} gt after a release, the queue released {} gt past it, post {})", o.r_leg, o.window.0, o.window.1, o.det, o.post),
                2 => format!("(c) the napper-only lift (the queue released as the cart leaves R; R + {} gt to the merge-level hold, released {}..{} gt after a release, post {})", o.r_leg, o.window.0, o.window.1, o.post),
                _ => format!("the trip range is the whole bed cycle, post {}", o.post),
            },
            latest_lie,
            o.night_gt,
            100.0 * (o.night_gt - latest_lie) as f64 / o.night_gt as f64,
            late_naps,
            100.0 * util_max,
            window_gt as i64,
            100.0 * (1.0 - util_max)
        ) + &format!(
            "; re-entry: {}; late nappers (rejoined a window late) {}; picks skipped because the lane's previous napper was still away (in the bed or on its siding: one a lane, never two on a siding) {}; the most nappers away at once {} (picked, not yet rejoined: one a lane at most, each in its own siding), the most not yet back at the merge level {} (the lane FIFO's slots: a lane id held from the pick to the split pass); a lane's lowest occupancy when a cart arrives at its back {} (carts queued in the lane, the hold's included; `ret` {} gt from a release to the lane's back)",
            if o.siding_window && o.rule2 { "rule 2 (a napper rejoins at a pass of its predecessor it was already waiting for; a late one waits a lane lap)" } else if o.siding_window { "the controller's siding rule (the lane's phase window AND the passing cart)" } else { "lane-exact (behind its predecessor's next pass)" },
            late_nappers,
            busy_skips,
            max_inflight,
            max_fifo,
            if min_occ == usize::MAX { 0 } else { min_occ },
            o.ret
        )
    } else {
        String::new()
    };
    let line = format!(
        "Q2 {} lanes x {} ({} villagers), {} bed(s){}, k {}{}, {}, {} skipped a day{}{}: golems a day {:.1}; lost a day: self-lock {:.2}, expired {:.2}, a lane empty {:.2}, held by a skip {:.2}; the shortest lane at any summon {} (summon ticks with a lane below {}: {:.1}%; lane lengths at the summons (length:lane-ticks) {}); the least turn spacing of any rider {} gt; fewest distinct nappers in a night {}, nights short of {}: {}; the deepest bed queue {} (strand outliers {}; the longest trip, the divert to the lane hold with the queue wait, {} gt){}; golems each day: {}",
        o.lanes, o.per_lane, n, o.beds,
        match o.turns { 1 => ", the lanes take the bed in sequence", 2 => ", the lanes take the bed round-robin", 3 => ", strict rhythm (lane l at cycles = l*k mod lanes*k; the bed queues)", _ => "" },
        o.k,
        if per_bed { " (each lane's own count)" } else { " (the shared returns)" },
        if o.keep_place { "the napper keeps its slot (fixed slots / equal-length bypass; a hole if away at its turn)" } else if o.lane_exact { "lane-exact re-entry" } else { "re-entry at its lane's back" },
        o.skip_per_day,
        if o.lock { ", the 600-gt self-lock" } else { "" },
        o.jump.map_or(String::new(), |(d, at)| format!(", a night skip at in_day {at} on day {d}{}", if o.jump_every { " and every day after" } else { "" })),
        golems as f64 / days,
        lost_lock as f64 / days,
        lost_expiry as f64 / days,
        lost_short as f64 / days,
        lost_skip as f64 / days,
        if min_lane == usize::MAX { 0 } else { min_lane },
        o.per_lane.saturating_sub(1),
        100.0 * ticks_below as f64 / scored_ticks.max(1) as f64,
        lens.join(" "),
        if least_spacing == i64::MAX { 0 } else { least_spacing },
        if least_distinct == usize::MAX { 0 } else { least_distinct },
        o.visits,
        short_nights,
        max_queue,
        outliers,
        max_trip,
        foot,
        per_day.join(", ")
    );
    (line, stats)
}

/// The report line for one row.
pub fn line(label: &str, o: &Opts, s: &Score) -> String {
    let riders: Vec<String> = s.per_rider.iter().map(|d| d.to_string()).collect();
    let bad: Vec<String> = s.bad_days.iter().map(|(d, n)| format!("day {d} {n}")).collect();
    format!(
        "{label}: {}{}{}{}; trip {}..{} gt; summons lost a day: expired LAST_SLEEP {:.2}, a visit's hole {:.2}, skipped {:.2} (worst day {}, of 240); days losing more than the skips: {}; worst drift {} gt ({} riders ever later), missed rider-nights {}; nights short of {}: {}, fewest distinct in a night {}; late gates {}, waiting room {} ({} days scored); per-rider worst drift: {}",
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
        if bad.is_empty() { "none".to_string() } else { bad.join(", ") },
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

#[cfg(test)]
mod lane_order_tests {
    use super::*;

    /// Every pick's rejoin in the release log: (rejoins checked, rejoins out of order). A pick is a
    /// cart whose next release in its lane comes more than a lap later; in order means its lane
    /// releases it right behind the cart that went before it at its pick (P) and right before the
    /// one that went after it (S).
    fn order_breaks(o: &LaneOpts) -> (usize, usize) {
        let st = simulate_lanes_scored(o).1;
        let (mut checked, mut broken) = (0usize, 0usize);
        for l in 0..o.lanes {
            let seq: Vec<usize> = st.release_log.iter().filter(|r| r.1 == l).map(|r| r.2).collect();
            for i in 1..seq.len().saturating_sub(1) {
                let Some(off) = seq[i + 1..].iter().position(|&x| x == seq[i]) else { continue };
                let j = i + 1 + off;
                if j > i + o.per_lane && j + 1 < seq.len() {
                    checked += 1;
                    if !(seq[j - 1] == seq[i - 1] && seq[j + 1] == seq[i + 1]) {
                        broken += 1;
                    }
                }
            }
        }
        (checked, broken)
    }

    /// G1 on a1212942b: under rule 2 a napper rejoins right behind its predecessor, on time and a
    /// lane lap late (a +350 strand: P, N, S both ways); the ring windows' late rejoin, behind
    /// whichever cart passes 10 cycles on, is the control that must break the order.
    #[test]
    fn rule2_rejoins_behind_its_predecessor() {
        let o = LaneOpts { rule2: true, log_releases: true, days: 8, warmup: 2, ..LaneOpts::lead() };
        let (n, b) = order_breaks(&o);
        assert!(n > 0, "no rejoin checked");
        assert_eq!(b, 0, "{b} of {n} on-time rejoins out of order");
        let late = LaneOpts { outlier: Some((24_000, 350)), ..o.clone() };
        assert!(simulate_lanes_scored(&late).1.late_nappers > 0, "the strand made no late rejoin");
        let (n2, b2) = order_breaks(&late);
        assert!(n2 > 0, "no late rejoin checked");
        assert_eq!(b2, 0, "{b2} of {n2} rejoins out of order with a lap-late napper");
        let ring = LaneOpts { rule2: false, ..late.clone() };
        let (n3, b3) = order_breaks(&ring);
        assert!(n3 > 0 && b3 > 0, "the control (ring windows, a late napper) kept the order: {b3} of {n3}");
    }
}
