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

/// (ironspec 23:06 / 23:21 PT) A measured per-visit distribution: (value, count) pairs, `v:n,v:n,...` (a bare `v`
/// counts 1).
pub fn parse_hist(s: &str) -> Result<Vec<(i64, u32)>, String> {
    let mut out = Vec::new();
    for w in s.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        let (v, n) = w.split_once(':').unwrap_or((w, "1"));
        let v: i64 = v.trim().parse().map_err(|e| format!("{w}: {e}"))?;
        let n: u32 = n.trim().parse().map_err(|e| format!("{w}: {e}"))?;
        out.push((v, n));
    }
    if out.iter().map(|x| x.1 as u64).sum::<u64>() == 0 {
        return Err(format!("{s}: an empty distribution"));
    }
    Ok(out)
}

/// A draw from a distribution by a 64-bit key (every visit its own draw; the same key, the same value).
pub fn hist_pick(h: &[(i64, u32)], key: u64) -> i64 {
    let total: u64 = h.iter().map(|x| x.1 as u64).sum();
    let mut z = key.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let mut i = (z ^ (z >> 31)) % total.max(1);
    for &(v, n) in h {
        if i < n as u64 {
            return v;
        }
        i -= n as u64;
    }
    h.last().map_or(0, |x| x.0)
}

/// The distribution's least and largest value.
pub fn hist_range(h: &[(i64, u32)]) -> (i64, i64) {
    (h.iter().map(|x| x.0).min().unwrap_or(0), h.iter().map(|x| x.0).max().unwrap_or(0))
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
    /// (ironspec 20:07 PT) Each lane's launch offset after the first's (gt; overrides `spacing`; offsets
    /// 0 / 14 / 24: lane 3 final at 14).
    pub offsets: Option<[i64; 3]>,
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
    /// (ii') (day-1, Nov 12 05:00, on ironspec's calm-REST hole in (ii)): a second ring window a lane,
    /// k/2 cycles after the first, on phases the ring already decodes (A u 9 + u 4, B u 2 + u 7, C u 5 +
    /// u 0): a late napper waits half a ring period at most.
    pub windows2: bool,
    /// (A) (ironspec Nov 12 05:14, after ironfarm's CANT_REACH stamp ≈ 1,650 under (ii)): a late latch a
    /// lane, set when the lane's window passes with its napper away and not yet on its siding; while
    /// set, the napper rejoins at the first pass at which it is already waiting (behind whichever cart
    /// passes), the latch reset by that release. Normal nights keep the predecessor's window.
    pub late_latch: bool,
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
    /// (ironspec 15:57 / 16:10: b8949c96's way-in) The divert to the queue's arrival detector, uniform in
    /// [lo, hi] gt; None: the older model (the stop's entry 60 gt after the divert, no queue).
    pub queue_leg: Option<(i64, i64)>,
    /// The queue's hold after the arrival (gt); the release also waits for the stop (R's RS).
    pub queue_hold: i64,
    /// The queue's release to the stop's entry (gt).
    pub to_entry: i64,
    /// (day-1's (i)) HELD sampled at the divert: a pick whose divert finds a napper held at the queue is
    /// not diverted (the cart rides on in its lane: the slot is lost).
    pub held_at_divert: bool,
    /// (ironspec 22:05 PT, the one-in-flight contract (b)) HELD set at the pick and reset at the queue's
    /// departure: no pick while another napper is between its pick (60 gt before its divert) and its release.
    pub held_at_pick: bool,
    /// (ironspec 23:06 / 23:21 PT) Entry -> R drawn per visit from this measured distribution (58f770cc's rows)
    /// instead of uniformly over `trip`.
    pub trip_hist: Option<Vec<(i64, u32)>>,
    /// (ironspec 23:37 PT (2)) The controller's pick window: (its first cycle from the night's start, its length
    /// in cycles); None: the whole night. The lanes' pick phases are anchored at its first cycle.
    pub pick_window: Option<(i64, i64)>,
    /// Every opportunity in the window picks (the controller has no per-lane count); false: a lane stops at its
    /// size (the model's idealised quota).
    pub no_quota: bool,
    /// (ironspec 00:12 PT (2)) R -> the siding's R (PRESENT) drawn per visit from this measured distribution
    /// (onechunk's record7d rows) instead of the structural return (r_leg + the slot's wait + post).
    pub ret_hist: Option<Vec<(i64, u32)>>,
    /// Per lane (A, B, C) R -> the siding's R distributions; overrides `ret_hist` for a lane that has one.
    pub ret_hist_lane: [Option<Vec<(i64, u32)>>; 3],
    /// (ironspec 01:12 PT (7)) The lanes' pick phases within the period (cycles from the window's first); None:
    /// l * k / lanes.
    pub pick_offsets: Option<[i64; 3]>,
    /// (ironspec 00:12 PT (2)) One forced skipped pick: on day D, the pick opportunity at cycle SLOT of the night
    /// (from the pick window's first cycle) is skipped as if its lane's napper were away.
    pub skip_pick: Option<(usize, i64)>,
    /// (G1 07:05 PT, b8949c96's rows) The late-sleeper distribution: each visit's lie-down "asleep at +N gt" past the
    /// stop's entry drawn from this histogram, the overrun past the normal lie-down (the second value, 89) added to its
    /// entry -> R (the leg's walk sink outlasting the descent; no HOME lost, only time).
    pub late_sleep: Option<(Vec<(i64, u32)>, i64)>,
    /// (Nov 12 16:50, today's return legs) Per lane: the merge hold's release -> PRESENT on the lane's
    /// siding (the rail's rise -> (4,0,7) 18, -> K(n) 79/94/110 (7b90c3f1), + 16 to PRESENT), replacing
    /// `post`; and the siding deadline: the predecessor's D pass `t_d` after its release (T_D 93/130/167),
    /// replacing the window cart's divert time. None: the older model.
    pub post_lane: Option<[i64; 3]>,
    pub t_d: Option<[i64; 3]>,
    /// (ironspec 01:27 PT (a)) The within-lane spreads, drawn per visit: the post (the departure ARM3 + 16..19 and the
    /// leg to the siding's R) up to `post_spread` gt later, and the predecessor's D press up to `t_d_spread` gt earlier
    /// (the queue occupancy 5..7).
    pub post_spread: i64,
    pub t_d_spread: i64,
    /// (ironspec 01:59 PT, the design of record: f1q2-run's SIDING = D's rise while PRESENT_l) The siding releases at any
    /// D press while the napper is present, not only at its lane's window.
    pub siding_any: bool,
    /// (ironspec 02:18 / 02:27 PT, B3's clocked stop exit, the catch window's candidate (2)) A6's release =
    /// AND(the wake edge latched, the lane's tap phi_l at or after its latest wake): R at the first tap tick at or
    /// after the visit's own R, the tap placed this many gt after the latest R the inputs allow (from the divert:
    /// the leg's worst + the hold + to_entry + entry -> R's worst) plus `exit_phase` for the lane; then the run-up's
    /// pickup, drawn per visit over `exit_pickup` (release -> R). None: off.
    pub clock_exit: Option<i64>,
    pub exit_phase: Option<[i64; 3]>,
    pub exit_pickup: (i64, i64),
    /// (day-1 10:37 PT, the hold form: the tap hold on the exit slope with the napper aboard, driven by a FREE-RUNNING
    /// per-lane pulse train) The napper reaches the hold this many gt before its natural R; the slope launches it at
    /// the first pulse of its lane's train at or after that arrival, so a wait of a period or more is launched by an
    /// EARLIER pulse (counted, and the napper leaves then). `gated_tap`: the earlier model, each visit's own tap only.
    pub hold_lead: i64,
    pub gated_tap: bool,
    /// (ironspec 09:51 PT, d_RS) RS_ready set this many gt after R: the stop is free only then.
    pub rs_delay: i64,
}
#[derive(Clone, Debug, Default)]
pub struct LaneStats {
    pub golems: f64,
    pub max_queue: usize,
    pub max_trip: i64,
    /// The shortest pick-to-back trip (G1 on 72d510183: an early napper's siding wait is bounded by the
    /// lap less this).
    pub min_trip: i64,
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
    /// The longest wait on a siding over the scored rejoins: from the napper's return (`back`) to the
    /// release of the cart it rejoins behind (ironspec Nov 12 04:48: the calm-REST bar < 1,000 gt).
    pub max_siding_wait: i64,
    /// The most nappers of one lane away at once (picked, not yet rejoined): 1 = a siding never
    /// double-occupied.
    pub max_lane_away: usize,
    /// Every scored rejoin's siding wait (return -> the release it rejoins behind) and whether it was
    /// late (a lap or more after its pick).
    pub waits: Vec<(i64, bool)>,
    /// Every scored rejoin's lane ride: from the release it rejoined behind to its own next release (the
    /// station's pocket), with the same late flag (farmopt 06:04: the napper's stretch to the pocket).
    pub rides: Vec<(i64, bool)>,
    /// (ironspec 16:10) Picks whose divert found a napper held at the queue (`held_at_divert`: the slot
    /// lost), the most nappers held at the queue at once, and the queue's waits (arrival -> release).
    pub held_skips: u32,
    pub queue_max: usize,
    pub queue_wait: (i64, i64),
    /// (ironspec 19:40 PT) The least spacing between two turns of one rider (its batch's release to its
    /// next), against the lock from the stamp's original tick (599 + the 20-gt re-stamp + 1 + 2).
    pub least_spacing: i64,
    /// (ironspec 19:53 PT (2)(d)) The least gap between two consecutive picks' diverts (gt).
    pub min_pick_gap: i64,
    /// (ironspec 20:53 PT) The injected late return (`hold_at`): its lane, cart and pick tick; and the golems
    /// per model day (from dusk), for the summons lost over the next day.
    pub hold_lane: Option<(usize, usize, i64)>,
    pub golems_day: std::collections::BTreeMap<i64, usize>,
    /// (ironspec 21:18 PT (2)) Every scored pick with the queue: (its divert, the queue's arrival, the stop's
    /// entry, R).
    pub seat_log: Vec<(i64, i64, i64, i64)>,
    /// (ironspec 22:18 PT) The least time from a napper's R to the next napper's lie-down (i64::MAX: none).
    pub liedown_gap_min: i64,
    /// (ironspec 23:21 PT) R -> back in lane (the rejoin) over the scored rejoins (least, largest; i64::MAX /
    /// i64::MIN: none), and the away latch's least margin: a rejoin against its lane's next pick (gt; a latch
    /// still set at the next pick is a skipped pick, counted in `busy_skips`).
    pub back_in_lane: (i64, i64),
    pub latch_margin_min: i64,
    /// (ironspec 23:37 PT (2)) Each lane's least pick opportunities in a scored night (usize::MAX: none scored).
    pub lane_opps_min: Vec<usize>,
    /// (ironspec 00:40 PT) Per lane: the least m = the predecessor's D press - PRESENT over the scored on-time rejoins,
    /// and its sample count (i64::MAX: none).
    pub m_lane: [(i64, usize); 3],
    /// (ironspec 01:12 PT (2)) Every scored on-time sample with m < 9: (lane, night, the pick's cycle in the night, m, the
    /// visit's R -> PRESENT).
    pub m_low: Vec<(usize, i64, i64, i64, i64)>,
    /// (ironspec 01:28 / 01:31 PT) Per lane over the on-time visits: the least late slack (Q-up before the sample it
    /// caught: the gate wait), the least early slack (Q-up after the previous sample: 100 - the wait), the count; the
    /// on-time visits' R -> PRESENT range; the late rejoins per lane.
    pub catch_lane: [(i64, i64, usize); 3],
    pub ret_range: (i64, i64),
    pub late_lane: [u32; 3],
    /// (ironspec 01:59 PT) Rejoins before the napper's window (an early catch under `siding_any`), per lane.
    pub early_lane: [u32; 3],
    /// (B3's clocked exit) Visits whose own R passed their tap (taking the next one), and the wait at the tap.
    pub clock_slips: u32,
    pub clock_waits: (i64, i64),
    /// (the free-running tap) Visits launched by an earlier pulse than their own tap (a wait at the hold of a period or
    /// more), per lane, and the wait at the hold (arrival -> launch) per lane.
    pub clock_early_lane: [u32; 3],
    pub clock_wait_lane: [(i64, i64); 3],
    /// (ironspec 04:07 PT, the chain audit) Whether the forced skipped pick (`skip_pick`) took: its slot's pick would
    /// have been made (an opportunity whose lane was free and not HELD), so the override changed its target.
    pub skip_took: bool,
}

impl LaneOpts {
    /// The F1 + Q2 lead's lane model as benched (58f770cc's stop: trip 196..198, lie +80, night to
    /// 23,920; foot (c) with the hold window 55..99; ret 175; k 10; the controller's siding windows).
    pub fn lead() -> LaneOpts {
        LaneOpts {
            lanes: 3, per_lane: 8, k: 10, beds: 1, turns: 3, visits: 24, lane_exact: true, keep_place: false,
            skip_per_day: 0, lock: true, trip: (196, 198), sleep_at: 40, spacing: 22, offsets: None, cross_at: 60, days: 30,
            warmup: 5, seed: 7, night_gt: 11_489, day_gt: 24_000, dusk_at: 0, jump: None, jump_every: false,
            outlier: None, post: 10, foot: 2, r_leg: 40, window: (55, 99), det: 2, lie: 80, start_dt: 12_431,
            rephase_lag: 0, truncate_after_skip: None, hold_at: None, rule2: false, windows2: false, late_latch: false, log_releases: false,
            siding_window: true, ret: 175, lock_gt: 600, remark: 0.0,
            queue_leg: None, queue_hold: 0, to_entry: 0, held_at_divert: false, held_at_pick: false, trip_hist: None, pick_window: None, no_quota: false, ret_hist: None, ret_hist_lane: [None, None, None], pick_offsets: None, skip_pick: None, late_sleep: None, post_lane: None, t_d: None, post_spread: 0, t_d_spread: 0, siding_any: false, clock_exit: None, exit_phase: None, exit_pickup: (0, 0), hold_lead: 0, gated_tap: false, rs_delay: 0,
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
    // (ironspec 23:37 PT (2): each lane's pick opportunities this night, and their least over the scored nights)
    let mut lane_opps: Vec<usize> = vec![0; o.lanes];
    let mut lane_opps_min: Vec<usize> = vec![usize::MAX; o.lanes];
    // (ironspec 00:40 PT: per lane the least m = D - PRESENT and its count)
    let mut m_lane: [(i64, usize); 3] = [(i64::MAX, 0); 3];
    let mut m_low: Vec<(usize, i64, i64, i64, i64)> = Vec::new();
    let per_bed = o.beds >= o.lanes || o.turns > 0;
    let mut cur_lane = 0usize;
    // (strict rhythm, turns 3: the night's first cycle; the one bed's queue: when it frees)
    let mut night_c0: i64 = 0;
    let mut bed_free_at: i64 = i64::MIN;
    let mut bed_q: Vec<(i64, i64)> = Vec::new();
    let (mut max_queue, mut outliers) = (0usize, 0usize);
    let mut max_trip: i64 = 0;
    let mut min_trip: i64 = i64::MAX;
    let (mut latest_lie, mut late_naps, mut util_max) = (0i64, 0usize, 0f64);
    let mut night_busy: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut next_outlier: i64 = o.outlier.map_or(i64::MAX, |(every, _)| every / 2);
    let mut lane_len_hist: std::collections::BTreeMap<usize, u64> = std::collections::BTreeMap::new();
    let mut night_idx: Option<i64> = None;
    let (mut golems, mut lost_lock, mut lost_expiry, mut lost_short, mut lost_skip) = (0usize, 0usize, 0usize, 0usize, 0usize);
    // (the largest LAST_SLEPT age any villager reaches over the scored days, ironspec Nov 12 02:10)
    let mut max_age = 0i64;
    let mut hold_used = false;
    // (the injected late return's lane, cart and pick tick, for the late-return rows)
    let mut hold_lane: Option<(usize, usize, i64)> = None;
    // (every scored pick with the queue: (the divert, the queue's arrival, the stop's entry, R))
    let mut seat_log: Vec<(i64, i64, i64, i64)> = Vec::new();
    // (the previous napper's R, and the least time from an R to the next napper's lie-down)
    let (mut last_r, mut liedown_gap_min): (Option<i64>, i64) = (None, i64::MAX);
    // (ironspec 23:21 PT: each napper's R, its R -> back in lane, and the away latch's margin: the rejoin against its
    // lane's next pick)
    let mut r_of: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    // (ironspec 01:28 / 01:31 PT: each napper's gate wait (the catch window's late slack) and R -> PRESENT)
    let mut wait_of: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    let mut ret_of: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    let mut catch_lane: [(i64, i64, usize); 3] = [(i64::MAX, i64::MAX, 0); 3];
    let mut ret_range: (i64, i64) = (i64::MAX, i64::MIN);
    let mut late_lane: [u32; 3] = [0; 3];
    let mut early_lane: [u32; 3] = [0; 3];
    let mut clock_slips: u32 = 0;
    let mut clock_waits: (i64, i64) = (i64::MAX, i64::MIN);
    let mut clock_early_lane: [u32; 3] = [0; 3];
    let mut clock_wait_lane: [(i64, i64); 3] = [(i64::MAX, i64::MIN); 3];
    let mut skip_took = false;
    let (mut back_in_lane, mut latch_margin_min) = ((i64::MAX, i64::MIN), i64::MAX);
    // (a rejoin at `t`, `dc` cycles into the night: R -> back in lane, and the away latch's margin, the lane's next
    // pick opportunity strictly after this cycle (inside the night's pick window) against the rejoin; a pick
    // skipped while the napper was away is counted in `busy_skips`)
    let note_rejoin = |bil: &mut (i64, i64), lm: &mut i64, rr: Option<i64>, t: i64, l: usize, dc: i64, in_night: bool| {
        if let Some(rr) = rr {
            bil.0 = bil.0.min(t - rr);
            bil.1 = bil.1.max(t - rr);
        }
        let (wa, wn) = o.pick_window.unwrap_or((0, i64::MAX / 4));
        let k = o.k as i64;
        let off = o.pick_offsets.map_or((l * o.k / o.lanes) as i64, |p| p[l.min(2)]);
        let to_next = (off - (dc - wa) - 1).rem_euclid(k) + 1;
        if in_night && dc + to_next >= wa && dc + to_next < wa + wn {
            *lm = (*lm).min(to_next * 100);
        }
    };
    let mut busy_skips = 0u32;
    let mut max_siding_wait = 0i64;
    let mut max_lane_away = 0usize;
    let mut late_latch_set: Vec<bool> = vec![false; o.lanes];
    let (mut waits, mut rides): (Vec<(i64, bool)>, Vec<(i64, bool)>) = (Vec::new(), Vec::new());
    let mut rejoin_at: std::collections::HashMap<usize, (i64, bool)> = std::collections::HashMap::new();
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
    // (the queue: each napper's (arrival, release); HELD = a napper arrived and not yet released)
    let mut queue_busy: Vec<(i64, i64)> = Vec::new();
    // (HELD from each pick to its release, for `held_at_pick`)
    let mut held_busy: Vec<(i64, i64)> = Vec::new();
    let (mut held_skips, mut queue_max, mut queue_wait) = (0u32, 0usize, (i64::MAX, i64::MIN));
    let mut last_turn: Vec<Option<i64>> = vec![None; n];
    let mut least_spacing = i64::MAX;
    // (ironspec 19:53 PT (2)(d): the least gap between two picks' diverts, for the queue-holds-one margin)
    let (mut last_pick_t, mut min_pick_gap) = (None::<i64>, i64::MAX);
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
            lane_opps = vec![0; o.lanes];
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
                for (m, &v) in lane_opps_min.iter_mut().zip(lane_opps.iter()) {
                    *m = (*m).min(v);
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
            let t = t0 + o.cross_at + o.offsets.map_or(l as i64 * o.spacing, |a| a[l.min(2)]);
            if let Some((r, late)) = rejoin_at.remove(&cart)
                && scored
            {
                rides.push((t - r, late));
            }
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
                // (ironspec 23:37 PT (2): the controller's pick window, `pick_window` = (its first cycle from the
                // night's start, its length in cycles), the lanes' phases anchored at its first cycle; without
                // `no_quota` a lane stops at its size (the model's idealised quota), with it every opportunity in
                // the window picks, as the controller does)
                let (wa, wn) = o.pick_window.unwrap_or((0, i64::MAX / 4));
                let dc = c - night_c0;
                let opp = in_night && dc >= wa && dc < wa + wn && (dc - wa).rem_euclid(period) == o.pick_offsets.map_or((l * o.k / o.lanes) as i64, |p| p[l.min(2)]);
                if opp {
                    lane_opps[l] += 1;
                }
                let truncated = lag_day == Some(day) && o.truncate_after_skip.is_some_and(|n| c - night_c0 >= n);
                // ((i): HELD sampled at the divert: a napper held at the queue keeps this cart in its lane;
                // (b), ironspec 22:05 PT: HELD set at the pick, from another napper's pick to its release)
                let held_now = (o.held_at_divert && queue_busy.iter().any(|&(a, r)| a <= t && t < r)) || (o.held_at_pick && held_busy.iter().any(|&(p, r)| p <= t - 60 && t - 60 < r));
                // (ironspec 00:12 PT (2): one forced skipped pick, as if the lane's napper were away; ironspec 04:07 PT: only
                // an opportunity whose pick would have been made takes it, so the override always changes its target)
                let would_pick = opp && (o.no_quota || lane_picks[l] < o.per_lane) && !truncated && lane_free && !held_now;
                let forced_skip = would_pick && o.skip_pick.is_some_and(|(sd, slot)| day == sd as i64 && dc - wa == slot);
                if forced_skip {
                    skip_took = true;
                    if scored {
                        busy_skips += 1;
                    }
                }
                let due = opp && !forced_skip && (o.no_quota || lane_picks[l] < o.per_lane)
                    // (the window after a skip cut to its first N releases: a daytime thunderstorm
                    // skip leaves the count a day off, so only the window's tail is at night)
                    && !truncated;
                // (a pick due while the lane's previous napper is still away (in the bed or waiting on
                // its siding) is skipped: one napper per lane, the siding never double-occupied)
                if due && !lane_free && scored {
                    busy_skips += 1;
                }
                if due && lane_free && held_now && scored {
                    held_skips += 1;
                }
                due && lane_free && !held_now
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
                if scored {
                    if let Some(p) = last_pick_t {
                        min_pick_gap = min_pick_gap.min(t - p);
                    }
                    last_pick_t = Some(t);
                }
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
                let drawn = match &o.trip_hist {
                    // (ironspec 23:06 / 23:21 PT: entry -> R drawn per visit from a measured distribution)
                    Some(h) => hist_pick(h, rng.next()),
                    None => rng.range(o.trip.0, o.trip.1),
                };
                // (G1 07:05 PT: the late sleeper's overrun past the normal lie-down, drawn per visit)
                let drawn = drawn + o.late_sleep.as_ref().map_or(0, |(h, n0)| (hist_pick(h, rng.next()) - n0).max(0));
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
                                hold_lane = Some((l, cart, t));
                                add
                            }
                            _ => 0,
                        };
                    // (ironspec Nov 11 22:46: the trip range is the bed's cycle, entry -> R -> the
                    // returning napper at the lift's foot (the queue releases on that evidence); 60 gt
                    // from the divert to the stop's entry before it, `post` from the lift's foot to the
                    // lane hold after it)
                    let (arrive, start) = if let Some((ql, qh)) = o.queue_leg {
                        // (b8949c96's way-in: the arrival detector `leg` after the divert, held
                        // `queue_hold`, released when the stop is free (R's RS), the stop's entry
                        // `to_entry` later)
                        let qa = t + rng.range(ql, qh);
                        let rel = (qa + o.queue_hold).max(bed_free_at);
                        queue_busy.retain(|&(_, r)| r > t);
                        queue_busy.push((qa, rel));
                        // (HELD set at the pick, 60 gt before this divert, reset at the release)
                        held_busy.retain(|&(_, r)| r > t - 60);
                        held_busy.push((t - 60, rel));
                        if scored {
                            queue_max = queue_max.max(queue_busy.iter().filter(|&&(a, r)| a <= qa && qa < r).count());
                            queue_wait = (queue_wait.0.min(rel - qa), queue_wait.1.max(rel - qa));
                        }
                        (qa, rel + o.to_entry)
                    } else {
                        let arrive = t + 60;
                        (arrive, arrive.max(bed_free_at))
                    };
                    // (the bed queue: nappers arrived and still waiting at this arrival)
                    bed_q.retain(|(_, s)| *s > arrive);
                    let depth = bed_q.iter().filter(|(a, s)| *a <= arrive && *s > arrive).count() + usize::from(start > arrive);
                    max_queue = max_queue.max(depth);
                    let cycle = (drawn + extra).max(1);
                    // (the foot: where the queue releases and what the trip adds after R)
                    let r = start + cycle;
                    // (ironspec 02:18 / 02:27 PT, B3's clocked stop exit: the release waits for the lane's tap, placed
                    // after the latest R the inputs allow; a visit whose own R passes its tap (the stop busy) takes the
                    // next one; then the run-up's pickup)
                    let r = if let Some(mg) = o.clock_exit {
                        let latest = o.queue_leg.map_or(60, |(_, qh)| qh) + o.queue_hold + o.to_entry + o.trip_hist.as_ref().map_or(o.trip.1, |h| hist_range(h).1) + o.late_sleep.as_ref().map_or(0, |(h, n0)| (hist_range(h).1 - n0).max(0)) + mg + o.exit_phase.map_or(0, |p| p[l.min(2)]);
                        let tap0 = t + latest;
                        let li = l.min(2);
                        let tap = if o.gated_tap {
                            // (the earlier model: the visit's own tap, or a later one when its R passed it)
                            let mut tap = tap0;
                            while tap < r {
                                tap += 100;
                            }
                            tap
                        } else {
                            // (the free-running train: pulses at tap0 + 100 k for every integer k; the napper, at rest on the
                            // hold from `hold_lead` gt before its natural R, leaves on the first pulse at or after that)
                            let a = r - o.hold_lead;
                            tap0 + 100 * (a - tap0).div_euclid(100) + if (a - tap0).rem_euclid(100) == 0 { 0 } else { 100 }
                        };
                        if scored {
                            clock_slips += u32::from(tap > tap0);
                            clock_early_lane[li] += u32::from(tap < tap0);
                            let a = r - if o.gated_tap { 0 } else { o.hold_lead };
                            clock_waits = (clock_waits.0.min(tap - a), clock_waits.1.max(tap - a));
                            clock_wait_lane[li] = (clock_wait_lane[li].0.min(tap - a), clock_wait_lane[li].1.max(tap - a));
                        }
                        tap + if o.exit_pickup.1 > o.exit_pickup.0 { rng.range(o.exit_pickup.0, o.exit_pickup.1 + 1) } else { o.exit_pickup.0 }
                    } else {
                        r
                    };
                    // (ironspec 22:18 PT: R -> the next napper's lie-down, every matrix row: this napper's
                    // lie-down against the previous napper's R)
                    if scored && let Some(pr) = last_r {
                        liedown_gap_min = liedown_gap_min.min(start + o.lie - pr);
                    }
                    last_r = Some(r);
                    r_of.insert(cart, r);
                    // (ironspec 21:18 PT (2): the seat's occupancy against the batch's S, and the returns' R)
                    if scored && o.queue_leg.is_some() {
                        seat_log.push((t, arrive, start, r));
                    }
                    let (free, back) = match o.foot {
                        1 => {
                            let foot = r + o.r_leg;
                            let pass = foot + gate_wait(foot, o.window) + o.det;
                            (pass, pass + o.post)
                        }
                        2 => {
                            let hold = r + o.r_leg;
                            // (ironspec 00:12 PT (2): the measured return per visit, R -> the siding's R; the lane's
                            // own distribution when it has one)
                            match o.ret_hist_lane[l.min(2)].as_ref().or(o.ret_hist.as_ref()) {
                                Some(h) => (r, r + hist_pick(h, rng.next())),
                                None => {
                                    // (ironspec 01:28 / 01:31 PT: the catch window: the wait from Q-up to the sample it
                                    // catches is the late slack; the time since the previous sample, 100 - the wait, the
                                    // early slack; the within-lane spreads of the departure and the leg drawn per visit)
                                    let w = gate_wait(hold, o.window);
                                    wait_of.insert(cart, w);
                                    let spread = if o.post_spread > 0 { rng.range(0, o.post_spread) } else { 0 };
                                    let present = hold + w + o.post_lane.map_or(o.post, |p| p[l]) + spread;
                                    ret_of.insert(cart, present - r);
                                    (r, present)
                                }
                            }
                        }
                        _ => (r, r + o.post),
                    };
                    bed_free_at = free + o.rs_delay;
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
                    min_trip = min_trip.min(back - t);
                    back
                } else {
                    t + drawn
                };
                away.push((cart, back, pred));
                max_lane_away = max_lane_away.max(away.iter().filter(|a| lane_of(a.0) == l).count());
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
                if o.lane_exact && o.siding_window && o.rule2 && o.t_d.is_some() {
                    // (rule 2 with today's deadline, ironspec 00:12 PT (2): a napper rejoins only right behind its
                    // own predecessor, at a pass of the predecessor it is already on its siding for (back by the
                    // predecessor's D pass, `t_d` after its release); a late one waits for the predecessor's next
                    // pass, a lane lap later, so the lane's cyclic order is kept)
                    let tw = o.t_d.map_or(t, |td| t0 + td[l]);
                    while let Some(i) = away.iter().position(|a| a.2 == cart && a.1 <= tw && lane_of(a.0) == l) {
                        let (id, back_t, _) = away.remove(i);
                        let late = pick_cycle.get(&id).is_some_and(|pc| c > pc + o.per_lane as i64 - 1);
                        if scored && late {
                            late_nappers += 1;
                        }
                        if scored {
                            max_siding_wait = max_siding_wait.max(tw - back_t);
                            waits.push((tw - back_t, late));
                            note_rejoin(&mut back_in_lane, &mut latch_margin_min, r_of.get(&id).copied(), t, l, c - night_c0, in_night);
                        }
                        rejoin_at.insert(id, (t, late));
                        lanes[l].push_back(id);
                    }
                } else if o.lane_exact && o.siding_window && o.rule2 {
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
                        let (id, back_t, _) = away.remove(i);
                        let late = pick_cycle.get(&id).is_some_and(|pc| c > pc + lap);
                        if scored && late {
                            late_nappers += 1;
                        }
                        if scored {
                            max_siding_wait = max_siding_wait.max(t - back_t);
                            waits.push((t - back_t, late));
                            note_rejoin(&mut back_in_lane, &mut latch_margin_min, r_of.get(&id).copied(), t, l, c - night_c0, in_night);
                        }
                        rejoin_at.insert(id, (t, late));
                        lanes[l].push_back(id);
                    }
                } else if o.lane_exact && o.siding_window {
                    // (the controller's rule: only at the lane's phase window (the pick's offset + the
                    // lane's length - 1, mod k), behind whichever cart passes then; (ii') also k/2 cycles
                    // later)
                    let win = (o.pick_offsets.map_or((l * o.k / o.lanes) as i64, |p| p[l.min(2)]) + o.per_lane as i64 - 1).rem_euclid(o.k as i64);
                    // (the phases anchored at the pick window's first cycle, as the picks are)
                    let wa0 = o.pick_window.map_or(0, |w| w.0);
                    let ph = (c - night_c0 - wa0).rem_euclid(o.k as i64);
                    let at_window = ph == win || (o.windows2 && ph == (win + o.k as i64 / 2).rem_euclid(o.k as i64));
                    // (today's deadline: the predecessor's D pass, `t_d` after this release, up to `t_d_spread` earlier)
                    let tw = o.t_d.map_or(t, |td| t0 + td[l]) - if o.t_d_spread > 0 { rng.range(0, o.t_d_spread) } else { 0 };
                    // ((A): with the lane's late latch set, any pass takes a napper already waiting; ironspec 01:59 PT,
                    // the design of record: the siding releases at any D press while PRESENT (`siding_any`), so an early
                    // napper rejoins behind the cart ahead of its predecessor)
                    if at_window || o.siding_any || (o.late_latch && late_latch_set[l]) {
                        let mut took = false;
                        while let Some(i) = away.iter().position(|a| a.1 <= tw && lane_of(a.0) == l) {
                            let (id, back_t, _) = away.remove(i);
                            let late = pick_cycle.get(&id).is_some_and(|pc| c > pc + o.per_lane as i64 - 1);
                            let early = pick_cycle.get(&id).is_some_and(|pc| c < pc + o.per_lane as i64 - 1);
                            if scored && late {
                                late_nappers += 1;
                            }
                            if scored && early {
                                early_lane[l.min(2)] += 1;
                            }
                            if scored {
                                max_siding_wait = max_siding_wait.max(tw - back_t);
                                waits.push((tw - back_t, late || early));
                                if !late && !early {
                                    let ml = &mut m_lane[l.min(2)];
                                    *ml = (ml.0.min(tw - back_t), ml.1 + 1);
                                    // (ironspec 01:12 PT (2): every on-time sample under the bar, named)
                                    if tw - back_t < 9 {
                                        m_low.push((l, day, pick_cycle.get(&id).map_or(-1, |pc| pc - night_c0), tw - back_t, r_of.get(&id).map_or(-1, |rr| back_t - rr)));
                                    }
                                    // (the catch window's two slacks over the on-time visits)
                                    if let Some(&w) = wait_of.get(&id) {
                                        let cl = &mut catch_lane[l.min(2)];
                                        *cl = (cl.0.min(w), cl.1.min(100 - w), cl.2 + 1);
                                    }
                                    if let Some(&rp) = ret_of.get(&id) {
                                        ret_range = (ret_range.0.min(rp), ret_range.1.max(rp));
                                    }
                                } else if late {
                                    late_lane[l.min(2)] += 1;
                                }
                                note_rejoin(&mut back_in_lane, &mut latch_margin_min, r_of.get(&id).copied(), t, l, c - night_c0, in_night);
                            }
                            rejoin_at.insert(id, (t, late || early));
                            lanes[l].push_back(id);
                            took = true;
                        }
                        if o.late_latch {
                            if took {
                                late_latch_set[l] = false;
                            } else if at_window && away.iter().any(|a| lane_of(a.0) == l) {
                                // (the window passed with the lane's napper away, not on its siding)
                                late_latch_set[l] = true;
                            }
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
    let stats = LaneStats { golems: golems as f64 / days, max_queue, max_trip, min_trip: if min_trip == i64::MAX { 0 } else { min_trip }, latest_lie, late_naps, util_max, lost_expiry: lost_expiry as f64, max_age, nap_log, release_log, min_lane: if min_lane == usize::MAX { 0 } else { min_lane }, busy_skips, late_nappers: late_nappers as u32, max_siding_wait, max_lane_away, waits, rides, held_skips, queue_max, queue_wait, least_spacing: if least_spacing == i64::MAX { 0 } else { least_spacing }, min_pick_gap: if min_pick_gap == i64::MAX { 0 } else { min_pick_gap }, hold_lane, golems_day: golems_day.clone(), seat_log, liedown_gap_min, back_in_lane, latch_margin_min, lane_opps_min, m_lane, m_low, catch_lane, ret_range, late_lane, early_lane, clock_slips, clock_waits, clock_early_lane, clock_wait_lane, skip_took };
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

/// (ironspec 16:08 / 16:10) Nap coverage on the lane model (`LaneOpts::lead()`: 3 lanes x 8, strict
/// rhythm k 10, lane-exact re-entry at the controller's siding windows, the one stop), with b8949c96's
/// way-in (the divert to the queue's arrival `leg`, the queue's hold, the release to the stop's entry)
/// and HELD sampled at the pick's divert or not: per complete night, each villager's naps (0 / 1 / 2+),
/// double picks and the never-picked; the largest LAST_SLEPT age against 24,000; over `seeds` seeds.
/// (ironspec 23:06 / 23:21 / 23:33 / 23:37 PT) The coverage bench's measured inputs and candidate fixes.
#[derive(Clone, Debug, Default)]
pub struct CovExtra {
    /// Entry -> R drawn per visit from this measured distribution (58f770cc's rows) instead of uniformly.
    pub trip_hist: Option<Vec<(i64, u32)>>,
    /// The controller's pick window (first cycle from the night's start, length in cycles), its lanes' phases
    /// anchored there; None: the whole night.
    pub pick_window: Option<(i64, i64)>,
    /// Every opportunity in the window picks (the controller keeps no per-lane count).
    pub no_quota: bool,
    /// (ironspec 00:12 PT (2)) R -> the siding's R per visit from onechunk's measured rows, on TODAY'S rows.
    pub ret_hist: Option<Vec<(i64, u32)>>,
    /// Per lane (A, B, C) measured R -> the siding's R, overriding `ret_hist` for a lane that has one.
    pub ret_hist_lane: [Option<Vec<(i64, u32)>>; 3],
    /// (ironspec 00:12 PT (2)) Rule 2 on TODAY'S rows (a napper rejoins only behind its own predecessor; a late one
    /// waits a lane lap).
    pub rule2: bool,
    /// (A) the late latch on TODAY'S rows.
    pub late_latch: bool,
    /// The known-answer frame pin (ironspec 23:19 PT): b8949c96's real-rest trace, its release -> entry (gt).
    pub frame_pin: Option<i64>,
    /// (ironspec 01:12 PT (7)) Run the coprime-advance picker search instead of the rows.
    pub search: bool,
    /// (ironspec 01:26 PT) The return leg's R -> Q-up (R -> the lift exit + k_lock) on TODAY'S rows (None: 109).
    pub r_leg: Option<i64>,
    /// (ironspec 01:27 PT (a)) The within-lane spreads: the post up to this much later per visit, the predecessor's D
    /// press up to `t_d_spread` earlier.
    pub post_spread: i64,
    pub t_d_spread: i64,
    /// (ironspec 01:59 PT) TODAY'S rows release the siding at any D press while PRESENT (the design of record), not only
    /// at the lane's window.
    pub siding_any: bool,
    /// (ironspec 02:18 / 02:27 PT) B3's clocked stop exit on TODAY'S rows: the tap's margin after the latest R, each
    /// lane's extra phase, the run-up's pickup (release -> R).
    pub clock_exit: Option<i64>,
    pub exit_phase: Option<[i64; 3]>,
    pub exit_pickup: (i64, i64),
    /// (G1 07:05 PT) The late-sleeper distribution (b8949c96's rows: each visit's "asleep at +N gt") and the normal
    /// lie-down; the overrun past it is added to each visit's entry -> R.
    pub late_sleep: Option<(Vec<(i64, u32)>, i64)>,
    /// (day-1 10:37 PT) The tap hold: the napper at rest on the hold this many gt before its natural R; the earlier
    /// per-visit gated tap instead of the free-running train.
    pub hold_lead: i64,
    pub gated_tap: bool,
    /// (ironspec 09:51 PT) d_RS: RS_ready set this many gt after R.
    pub rs_delay: i64,
    /// (G1 10:37 PT) The budget's design terms beside the world-open shift: the clock's tolerance and the worst push
    /// shift (0 and printed "not measured" until measured).
    pub clock_tol: Option<i64>,
    pub push_shift: Option<i64>,
    /// (ironspec 07:40 / 07:41 PT, G1's conditions) Sweep each lane's tap phase over 0..99 at this step, the other
    /// lanes held at --exit-phase: each lane's feasible interval, its edges' controls.
    pub exit_phase_sweep: Option<i64>,
    /// (ironspec 10:42 PT (1)) The per-lane taps' wait margin beyond the world-open shift: the measured D-press spread +
    /// the push shift (unset: 0, printed "not measured"; NOT JUDGED in a design-of-record run).
    pub wait_margin: Option<i64>,
    /// (G1's structural (a), Z1 / Z4 on 10bcdf661) A design-of-record run: unmeasured terms make their bars NOT JUDGED.
    pub design_of_record: bool,
    /// (ironspec 14:22 PT, tick-exact 14:16 PT's farm-chunk row) The world-open / login lag a timed margin subtracts, k + j
    /// taken together (2 for an edge whose redstone and cart sit in different chunks; 1 within one chunk); unset: the old
    /// uncalibrated 7 ([`WORLD_OPEN_SHIFT`]), labelled.
    pub world_open_shift: Option<i64>,
}

/// (G1 07:05 PT) b8949c96's per-row files: every "asleep at +N gt" in them, as a histogram (a file without one is an
/// error: an empty draw would make the rows vacuous).
pub fn late_sleep_hist(paths: &[std::path::PathBuf]) -> Result<Vec<(i64, u32)>, String> {
    let mut counts: std::collections::BTreeMap<i64, u32> = Default::default();
    for p in paths {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let mut n = 0u32;
        for part in text.split("asleep at +").skip(1) {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() || !part[digits.len()..].starts_with(" gt") {
                return Err(format!("{}: an \"asleep at +\" not followed by N gt", p.display()));
            }
            *counts.entry(digits.parse::<i64>().map_err(|e| e.to_string())?).or_default() += 1;
            n += 1;
        }
        if n == 0 {
            return Err(format!("{}: no \"asleep at +N gt\" row", p.display()));
        }
    }
    Ok(counts.into_iter().collect())
}

/// (ironspec 01:12 PT (7)) The picker search on the lane model (with identities): per-lane pick period k, the window's
/// opportunities per lane, the lanes' phases and a per-lane count, from `base`. Normal night: 24 distinct, 0 doubles, no
/// lie-down past the night, picks >= 300 gt apart, one at the queue at a time. Robustness: every single disturbance at
/// every pick of one night (a skipped pick; a late rejoin of +2 gt, +1 cycle, +2 cycles): that night >= 23 distinct and
/// the next 24. Returns the report and the passing configurations' labels (the caller judges the design's).
pub fn picker_search(base: &dyn Fn(u64) -> LaneOpts, seeds: u64, wa: i64) -> (Vec<String>, Vec<String>) {
    let mut lines = vec![format!(
        "picker search (ironspec 01:12 PT (7), the lane model with identities; seeds 1..={seeds}; the window from cycle {wa} of the night): per configuration the normal night (24 distinct, 0 doubles, 0 lie-downs past the night, picks >= 300 gt apart, at most one at the queue) and the robustness classes (every single disturbance at every pick of night 9: a skipped pick, a late rejoin +2 gt, +100 gt (one cycle), +200 gt (two); that night >= 23 distinct and the next 24)"
    )];
    let d_inj = LaneOpts::lead().warmup as i64 + 4;
    let distinct = |st: &LaneStats, night: i64| {
        let mut seen = [false; 64];
        for &(nt, c, _) in &st.nap_log {
            if nt == night && c < 64 {
                seen[c] = true;
            }
        }
        seen.iter().filter(|&&x| x).count()
    };
    let mut passing: Vec<String> = Vec::new();
    for k in [9usize, 10, 11, 12] {
        let offsets_set: Vec<[i64; 3]> = {
            let a = [0, (k / 3) as i64, (2 * k / 3) as i64];
            let b = [0, k.div_ceil(3) as i64, (2 * k).div_ceil(3) as i64];
            if a == b { vec![a] } else { vec![a, b] }
        };
        for n_opp in [8i64, 9, 10, 11] {
            for offs in &offsets_set {
                for quota in [false, true] {
                    let cfg = |seed: u64| LaneOpts { k, pick_window: Some((wa, n_opp * k as i64)), no_quota: !quota, pick_offsets: Some(*offs), ..base(seed) };
                    // the normal night
                    let (mut least, mut dbl, mut late_naps, mut gap, mut qmax, mut lie) = (usize::MAX, 0usize, 0usize, i64::MAX, 0usize, 0i64);
                    for seed in 1..=seeds {
                        let o = cfg(seed);
                        let n = o.lanes * o.per_lane;
                        let st = simulate_lanes_scored(&o).1;
                        let mut per: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
                        for (nt, c, _) in &st.nap_log {
                            per.entry(*nt).or_default().push(*c);
                        }
                        for nt in (o.warmup as i64 + 1)..=(o.days as i64 - 2) {
                            let naps = per.get(&nt).cloned().unwrap_or_default();
                            let mut cnt = vec![0usize; n];
                            naps.iter().for_each(|&c| cnt[c] += 1);
                            let d = cnt.iter().filter(|&&x| x > 0).count();
                            least = least.min(d);
                            dbl = dbl.max(naps.len() - d);
                        }
                        late_naps += st.late_naps;
                        if st.min_pick_gap > 0 {
                            gap = gap.min(st.min_pick_gap);
                        }
                        qmax = qmax.max(st.queue_max);
                        lie = lie.max(st.latest_lie);
                    }
                    let normal_ok = least == 24 && dbl == 0 && late_naps == 0 && gap >= 300 && qmax <= 1;
                    let label = format!("k {k}, {n_opp} opportunities a lane (window {} cycles), phases {:?}, {}", n_opp * k as i64, offs, if quota { "a per-lane count of 8" } else { "no count" });
                    if !normal_ok {
                        lines.push(format!("  {label}: outside the normal night's bars (least distinct {}, doubles {dbl}, lie-downs past the night {late_naps}, least pick gap {}, at the queue at once {qmax})", if least == usize::MAX { 0 } else { least }, if gap == i64::MAX { "-".to_string() } else { gap.to_string() }));
                        continue;
                    }
                    // robustness: the worst (that night, the next) distinct per class
                    let mut classes: Vec<(String, usize, usize, u32)> = Vec::new();
                    let run_days = (d_inj + 3) as usize;
                    {
                        let (mut wn_, mut wx_, mut cnt) = (usize::MAX, usize::MAX, 0u32);
                        for seed in 1..=seeds.min(2) {
                            let b = LaneOpts { days: run_days, ..cfg(seed) };
                            for slot in 0..n_opp * k as i64 {
                                if !offs.contains(&(slot % k as i64)) {
                                    continue;
                                }
                                let st = simulate_lanes_scored(&LaneOpts { skip_pick: Some((d_inj as usize, slot)), ..b.clone() }).1;
                                cnt += 1;
                                wn_ = wn_.min(distinct(&st, d_inj));
                                wx_ = wx_.min(distinct(&st, d_inj + 1));
                            }
                        }
                        classes.push(("a skipped pick".to_string(), wn_, wx_, cnt));
                    }
                    for add in [2i64, 100, 200] {
                        let (mut wn_, mut wx_, mut cnt) = (usize::MAX, usize::MAX, 0u32);
                        for seed in 1..=seeds.min(2) {
                            let b = LaneOpts { days: run_days, ..cfg(seed) };
                            let mut hat = 0i64;
                            while hat < 11_000 {
                                let st = simulate_lanes_scored(&LaneOpts { hold_at: Some((d_inj as usize, hat, add)), ..b.clone() }).1;
                                hat += 100;
                                if st.hold_lane.is_none() {
                                    continue;
                                }
                                cnt += 1;
                                wn_ = wn_.min(distinct(&st, d_inj));
                                wx_ = wx_.min(distinct(&st, d_inj + 1));
                            }
                        }
                        classes.push((format!("a late rejoin +{add} gt"), wn_, wx_, cnt));
                    }
                    let passed: Vec<&str> = classes.iter().filter(|c| c.3 > 0 && c.1 >= 23 && c.2 == 24).map(|c| c.0.as_str()).collect();
                    let row = format!(
                        "  {label}: normal night passes (the latest lie-down at in_day {lie}, the least pick gap {gap} gt); robustness {}; classes passed {} of 4",
                        classes.iter().map(|c| format!("{}: that night least {} the next {} ({} runs{})", c.0, if c.1 == usize::MAX { "-".to_string() } else { c.1.to_string() }, if c.2 == usize::MAX { "-".to_string() } else { c.2.to_string() }, c.3, if c.3 > 0 && c.1 >= 23 && c.2 == 24 { ", within" } else if c.3 == 0 { ", no injection took" } else { ", outside" })).collect::<Vec<_>>().join("; "),
                        passed.len()
                    );
                    if passed.len() == 4 {
                        passing.push(label.clone());
                    }
                    lines.push(row);
                }
            }
        }
    }
    lines.push(format!(
        "=> configurations passing the normal night and all four robustness classes: {}",
        if passing.is_empty() { "NONE".to_string() } else { passing.join("; ") }
    ));
    (lines, passing)
}

/// (the chain fix) One coverage row's measurements, judged by the caller (the operating point's bars, a control's bite,
/// or printed for reference).
#[derive(Clone, Debug, Default)]
pub struct RowStats {
    pub nights: usize,
    pub least: usize,
    pub max_dbl: usize,
    pub max_never: usize,
    pub late: u32,
    pub late_naps: usize,
    pub qmax: usize,
    pub lost: f64,
    pub busy: u32,
    pub age: i64,
    pub trip_max: i64,
    pub shortest: usize,
    pub spacing: i64,
    pub pick_gap: i64,
    pub lmargin: i64,
    pub seat_n: usize,
    pub seat_margin: i64,
    pub has_td: bool,
    pub rule2: bool,
    pub m_acc: [(i64, usize); 3],
    pub catch_acc: [(i64, i64, usize); 3],
    pub late_acc: [u32; 3],
    pub early_acc: [u32; 3],
    pub seeds: u64,
    pub per_lane: usize,
    /// (the free-running tap) Whether the row runs the clocked exit; visits whose arrival passed their tap; visits launched
    /// by an earlier pulse, per lane; the wait at the hold per lane.
    pub clocked: bool,
    pub slips: u32,
    pub clock_early: [u32; 3],
    pub clock_wait: [(i64, i64); 3],
}

/// (G1's checklist (4)) `--exit-phase` / `--exit-pickup` / `--return-hist-lane`, parsed strictly: three phases or none;
/// a pickup range of two values (one value: a fixed pickup) or none; three `|`-separated lane histograms (an empty part
/// for a lane without one).
pub fn parse_exit_phase(v: &[i64]) -> Result<Option<[i64; 3]>, String> {
    match v {
        [] => Ok(None),
        [a, b, c] => Ok(Some([*a, *b, *c])),
        _ => Err(format!("--exit-phase takes exactly 3 values (lanes A, B, C), got {}", v.len())),
    }
}

pub fn parse_exit_pickup(v: &[i64]) -> Result<(i64, i64), String> {
    match v {
        [] => Ok((0, 0)),
        [a] if *a >= 0 => Ok((*a, *a)),
        [a, b] if *a >= 0 && a <= b => Ok((*a, *b)),
        _ => Err(format!("--exit-pickup takes LO,HI with 0 <= LO <= HI (or one value), got {v:?}")),
    }
}

#[allow(clippy::type_complexity)]
pub fn parse_hist_lanes(s: Option<&str>) -> Result<[Option<Vec<(i64, u32)>>; 3], String> {
    let mut v: [Option<Vec<(i64, u32)>>; 3] = [None, None, None];
    if let Some(s) = s {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != 3 {
            return Err(format!("--return-hist-lane takes exactly 3 '|'-separated parts (lanes A, B, C; an empty part for none), got {}", parts.len()));
        }
        for (i, part) in parts.iter().enumerate() {
            if !part.trim().is_empty() {
                v[i] = Some(parse_hist(part)?);
            }
        }
        if v.iter().all(|h| h.is_none()) {
            return Err("--return-hist-lane: every part empty".into());
        }
    }
    Ok(v)
}

/// (the chain fix) The operating point's bars, each over its own samples (0 samples: NOT JUDGED, a failure).
/// (G1's Z9 on 10bcdf661) The world-open term every timed margin subtracts: "world-open term from one ride: 0..7 gt,
/// uncalibrated" (ironspec's 08:46 budget; tick-exact's farm-chunk row replaces it with the measured k + j).
pub const WORLD_OPEN_SHIFT: i64 = 7;
/// (G1's Z9) The world open's lead on the catch window: a world open at N <= 2 entity-free ticks (the lead's
/// portal-ticket loader; ironspec 01:46 PT (b): +1 and +2 gt are normal operation).
pub const WORLD_OPEN_LEAD: i64 = 2;
/// (G1's Z9) The exit-phase sweep's catch bar: both slacks >= 20 (ironspec 10:42 PT (1): "catch >= 20 both sides").
pub const CATCH_MIN: i64 = 20;

fn judge_operating(st: &RowStats, leg: (i64, i64), what: &str, v: &mut crate::verdict::Verdict) -> String {
    let mut w = Vec::new();
    w.push(format!("24 distinct a night: {}", v.bar_n(&format!("{what}: 24 distinct nappers every complete night"), st.nights, st.least == 24)));
    w.push(format!("no double pick: {}", v.bar_n(&format!("{what}: no rider picked twice in a night"), st.nights, st.max_dbl == 0)));
    w.push(format!("none never picked: {}", v.bar_n(&format!("{what}: no rider left unpicked in a night"), st.nights, st.max_never == 0)));
    w.push(format!("at most one napper away per lane at a summon (the shortest lane >= {}): {}", st.per_lane.saturating_sub(1), v.bar_n(&format!("{what}: at most one napper away per lane at any summon (the shortest lane >= {})", st.per_lane.saturating_sub(1)), usize::from(st.shortest != usize::MAX), st.shortest != usize::MAX && st.shortest + 1 >= st.per_lane)));
    w.push(format!("one in flight under F1: {}", v.bar_n(&format!("{what}: one in flight under F1"), usize::from(st.pick_gap != i64::MAX), st.pick_gap != i64::MAX && st.pick_gap > 60 + leg.1 + 2)));
    w.push(format!("the away latch (a rejoin measured, no skipped pick): {}", v.bar_n(&format!("{what}: the away latch's margin, no pick skipped for an away napper"), usize::from(st.lmargin != i64::MAX), st.busy == 0)));
    w.push(format!("the stamp-age margin >= 3: {}", v.bar_n(&format!("{what}: the stamp-age margin >= 3"), usize::from(st.spacing != i64::MAX), st.spacing != i64::MAX && st.spacing - 32 - 599 >= 3)));
    w.push(format!("no summon lost to an expired LAST_SLEPT: {}", v.bar_n(&format!("{what}: no summon lost to an expired LAST_SLEPT"), st.nights, st.lost == 0.0 && st.age < 24_000)));
    w.push(format!("no lie-down past the night: {}", v.bar_n(&format!("{what}: no lie-down past the night"), st.nights, st.late_naps == 0)));
    w.push(format!("at most one held at the queue: {}", v.bar_n(&format!("{what}: at most one held at the queue"), st.nights, st.qmax <= 1)));
    // (on time: under the design of record's D-and-PRESENT release the deadline is the predecessor's D press, judged by the
    // catch window below; the older siding-window model's deadline is 700 gt after the pick's divert)
    if !st.has_td {
        w.push(format!("every napper back by its siding window (divert -> back <= 700): {}", v.bar_n(&format!("{what}: the longest divert -> back within 700 gt"), st.nights, st.trip_max <= 700)));
    } else {
        // (G1's Y18: dropped only through the scope mechanism, naming the bar and why)
        w.push(v.scope(&format!("{what}: the longest divert -> back within 700 gt"), "the design of record's D-and-PRESENT release: the deadline is the predecessor's D press, judged by the catch window below"));
    }
    w.push(format!("every seat stay inside its own stamp: {}", v.bar_n(&format!("{what}: every seat stay inside its own stamp"), st.seat_n, st.seat_margin > 0)));
    if st.clocked {
        // (day-1 10:37 PT: the tap hold launches whatever napper is on the slope at every pulse of its lane's train, so
        // a wait of a period or more is an early launch, a period before the napper's own tap)
        let early: u32 = st.clock_early.iter().sum();
        let wmax = st.clock_wait.iter().filter(|x| x.1 != i64::MIN).map(|x| x.1).max();
        w.push(format!(
            "the free-running tap: no napper launched by an earlier pulse (the largest wait at the hold {}, against the period 100): {}",
            wmax.map_or("-".to_string(), |m| m.to_string()),
            v.bar_n(&format!("{what}: no napper launched by an earlier pulse of its lane's free-running tap"), st.nights, early == 0)
        ));
    }
    if st.has_td {
        let worst = st.m_acc.iter().filter(|x| x.1 > 0).map(|x| x.0).min();
        let m_n = st.m_acc.iter().map(|x| x.1).min().unwrap_or(0);
        w.push(format!("m >= 10 under a world open's lead: {}", v.bar_n(&format!("{what}: m >= 10 under a world open's lead in every lane"), m_n, worst.is_some_and(|w| w - WORLD_OPEN_LEAD >= 10))));
        let c_n = st.catch_acc.iter().map(|x| x.2).min().unwrap_or(0);
        let catch_ok = st.catch_acc.iter().all(|&(lt, er, _)| lt - WORLD_OPEN_LEAD >= 2 && er - WORLD_OPEN_LEAD >= 2) && st.late_acc.iter().all(|&n| n == 0) && st.early_acc.iter().all(|&n| n == 0);
        w.push(format!("the catch window: {}", v.bar_n(&format!("{what}: the catch window (both slacks >= 2 after the lead, no late or early rejoin) in every lane"), c_n, catch_ok)));
    } else if st.rule2 {
        w.push(v.scope(&format!("{what}: m and the catch window"), "rule 2 rejoins only behind the napper's own predecessor (the siding window's D-and-PRESENT release does not apply)"));
        w.push(format!("no late rejoin: {}", v.bar_n(&format!("{what}: no late rejoin"), st.nights, st.late == 0)));
    } else {
        w.push(format!("no late rejoin: {}", v.bar_n(&format!("{what}: no late rejoin"), st.nights, st.late == 0)));
    }
    format!("  => {what}: {}", w.join("; "))
}

#[allow(clippy::too_many_arguments)]
pub fn coverage_bench(holds: &[i64], operating_hold: i64, leg: (i64, i64), to_entry: i64, post: [i64; 3], t_d: [i64; 3], slot: i64, trip: (i64, i64), lock: i64, offsets: [i64; 3], late: (&[i64], i64), days: usize, seeds: u64, x: &CovExtra, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    // (ironspec's 12 on the tenth, G1's checklist (6): no argument may make a bar vacuous)
    if holds.is_empty() {
        return Err("--holds: at least one hold".into());
    }
    if !holds.contains(&operating_hold) {
        return Err(format!("--operating-hold {operating_hold} is not among --holds {holds:?}"));
    }
    let warm = LaneOpts::lead().warmup;
    if days < warm + 4 {
        return Err(format!("--days {days}: at least the warm-up {warm} + 4, so two complete nights are scored"));
    }
    if seeds < 1 {
        return Err("--seeds: at least 1".into());
    }
    // (ironspec's hole on the tenth: R -> Q-up 109 = 89 + 20 holds for the hold chain's k_lock 42 only; the bench cannot
    // see k_lock, so the leg is given explicitly)
    let Some(_) = x.r_leg else {
        return Err("--r-leg is required: R -> Q-up depends on the hold chain's k_lock (109 = 89 + 20 at k_lock 42; the design of record's measured 95)".into());
    };
    // (G1's Y17: `lock` is not vacuous: TODAY'S rows take it as the model's `lock_gt`, rule 4a's self-lock, printed with
    // the stamp-age bar as "the model's lock")
    let mut lines = vec![format!(
        "nap coverage on the lane model (3 lanes x 8 = 24 villagers; each lane picks every k 10 cycles (the pick period, not k_lock / k_sw / K(x)) at offsets 0/3/6 from the night's start, at most 8 a night; a pick due while the lane's napper is away is skipped; the napper rejoins at its lane's siding window behind the cart then passing; the one stop; {days} days, {} warm-up, seeds 1..={seeds}; THE OPERATING POINT: the queue's hold {operating_hold} gt). The invariant: lane-exact re-entry keeps each lane's cyclic order, and a napper misses exactly one turn, so a lane's consecutive picks (10 cycles apart, 7 carts in rotation while one is away) advance 10 - 7 = 3 places in that order; 3 is coprime with 8, so 8 picks a night visit the lane's 8 riders once each. The invariant needs every slot taken and every napper back on its siding by its window (7 cycles after its pick, behind its predecessor): a skipped slot (the lane's napper still away, or HELD at the divert) moves the next pick 3 + 10 - 8 = 5 places on, and a late rejoin (a period later, behind another cart) shifts the order; either can pick a rider twice and leave another unpicked that night, which the rows count",
        warm
    )];
    let row = |label: &str, f: &dyn Fn(u64) -> LaneOpts| -> (String, RowStats) {
        let (mut nights, mut least, mut max_dbl, mut max_never, mut nights_never) = (0usize, usize::MAX, 0usize, 0usize, 0usize);
        let (mut v0, mut v1, mut v2) = (0usize, 0usize, 0usize);
        let (mut age, mut lost, mut held, mut busy, mut late, mut qmax, mut qw, mut late_naps, mut golems) = (0i64, 0f64, 0u32, 0u32, 0u32, 0usize, (i64::MAX, i64::MIN), 0usize, 0f64);
        let mut trip_max = 0i64;
        let (mut spacing, mut shortest) = (i64::MAX, usize::MAX);
        let mut pick_gap = i64::MAX;
        let mut lock_used = 0i64;
        // (ironspec 21:18 PT (2): the seat rider's stay against its batch's S + 599, and the overlaps with a
        // returning napper's passage through y -12..-1 (the official ride: R + 39..82, the cart's y in
        // [-12, -1], hold-pulse-bench --lift-trace 0.2))
        let (mut seat_n, mut seat_margin, mut seat_end) = (0usize, i64::MAX, (i64::MAX, i64::MIN));
        let (mut seat_ov, mut below_ov) = (0usize, 0usize);
        let mut liedown = i64::MAX;
        let (mut bil, mut lmargin) = ((i64::MAX, i64::MIN), i64::MAX);
        let (mut smargin, mut sm_n, mut has_td) = (i64::MAX, 0usize, false);
        let mut m_acc: [(i64, usize); 3] = [(i64::MAX, 0); 3];
        let mut catch_acc: [(i64, i64, usize); 3] = [(i64::MAX, i64::MAX, 0); 3];
        let mut ret_acc: (i64, i64) = (i64::MAX, i64::MIN);
        let mut late_acc: [u32; 3] = [0; 3];
        let mut early_acc: [u32; 3] = [0; 3];
        let mut slip_acc: u32 = 0;
        let mut cwait_acc: (i64, i64) = (i64::MAX, i64::MIN);
        let mut early_c: [u32; 3] = [0; 3];
        let mut wait_c: [(i64, i64); 3] = [(i64::MAX, i64::MIN); 3];
        let mut clocked = false;
        let (mut m_low_n, mut m_low_list) = (0usize, Vec::<String>::new());
        let mut opps = [usize::MAX; 3];
        let mut per_lane_n = 0usize;
        for seed in 1..=seeds {
            let o = f(seed);
            lock_used = o.lock_gt;
            per_lane_n = o.per_lane;
            has_td = o.t_d.is_some() && o.siding_window && !o.rule2;
            let n = o.lanes * o.per_lane;
            let st = simulate_lanes_scored(&o).1;
            liedown = liedown.min(st.liedown_gap_min);
            bil = (bil.0.min(st.back_in_lane.0), bil.1.max(st.back_in_lane.1));
            lmargin = lmargin.min(st.latch_margin_min);
            // (ironspec 23:44 PT: PRESENT before the predecessor's D pass, the least on-time siding wait)
            for &(w, late_w) in &st.waits {
                if !late_w {
                    smargin = smargin.min(w);
                    sm_n += 1;
                }
            }
            for (acc, m) in m_acc.iter_mut().zip(st.m_lane.iter()) {
                *acc = (acc.0.min(m.0), acc.1 + m.1);
            }
            for (acc, cl) in catch_acc.iter_mut().zip(st.catch_lane.iter()) {
                *acc = (acc.0.min(cl.0), acc.1.min(cl.1), acc.2 + cl.2);
            }
            ret_acc = (ret_acc.0.min(st.ret_range.0), ret_acc.1.max(st.ret_range.1));
            for (acc, &n) in late_acc.iter_mut().zip(st.late_lane.iter()) {
                *acc += n;
            }
            for (acc, &n) in early_acc.iter_mut().zip(st.early_lane.iter()) {
                *acc += n;
            }
            slip_acc += st.clock_slips;
            cwait_acc = (cwait_acc.0.min(st.clock_waits.0), cwait_acc.1.max(st.clock_waits.1));
            clocked = o.clock_exit.is_some();
            for li in 0..3 {
                early_c[li] += st.clock_early_lane[li];
                wait_c[li] = (wait_c[li].0.min(st.clock_wait_lane[li].0), wait_c[li].1.max(st.clock_wait_lane[li].1));
            }
            for &(l, nt, pc, m, ret) in &st.m_low {
                m_low_n += 1;
                if m_low_list.len() < 12 {
                    m_low_list.push(format!("seed {seed} lane {l} night {nt} pick cycle {pc} m {m} (R -> PRESENT {ret})"));
                }
            }
            for (m, &v) in opps.iter_mut().zip(st.lane_opps_min.iter()) {
                *m = (*m).min(v);
            }
            {
                let passages: Vec<(i64, i64)> = st.seat_log.iter().map(|&(_, _, _, r)| (r + 39, r + 82)).collect();
                for (i, &(dv, qa, entry, _)) in st.seat_log.iter().enumerate() {
                    // (the batch's summon S: the pick at its stand turn, 60 gt before the divert)
                    let s = dv - 60;
                    seat_n += 1;
                    seat_margin = seat_margin.min(s + 599 - entry);
                    seat_end = (seat_end.0.min(entry - s), seat_end.1.max(entry - s));
                    let hit = |a: i64, b: i64| passages.iter().enumerate().any(|(j, &(p0, p1))| j != i && a <= p1 && p0 <= b);
                    seat_ov += usize::from(hit(qa, entry));
                    below_ov += usize::from(hit(dv, qa));
                }
            }
            if st.min_pick_gap > 0 {
                pick_gap = pick_gap.min(st.min_pick_gap);
            }
            if st.least_spacing > 0 {
                spacing = spacing.min(st.least_spacing);
            }
            if st.min_lane > 0 {
                shortest = shortest.min(st.min_lane);
            }
            let mut per: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
            for (nt, c, _) in &st.nap_log {
                per.entry(*nt).or_default().push(*c);
            }
            // (G1 on dd1c4270f point 1: the complete nights come from the run's scored days, not from the nights
            // that happened to log a nap: a night with no nap counts, as 24 never-picked; the first scored
            // night and the last are dropped as possibly partial)
            let empty: Vec<usize> = Vec::new();
            let first = o.warmup as i64 + 1;
            let last = o.days as i64 - 2;
            let inner: Vec<&Vec<usize>> = (first..=last).map(|nt| per.get(&nt).unwrap_or(&empty)).collect();
            for naps in inner {
                let mut cnt = vec![0usize; n];
                naps.iter().for_each(|&c| cnt[c] += 1);
                let distinct = cnt.iter().filter(|&&x| x > 0).count();
                nights += 1;
                least = least.min(distinct);
                max_dbl = max_dbl.max(naps.len() - distinct);
                max_never = max_never.max(n - distinct);
                nights_never += usize::from(distinct < n);
                v0 += cnt.iter().filter(|&&x| x == 0).count();
                v1 += cnt.iter().filter(|&&x| x == 1).count();
                v2 += cnt.iter().filter(|&&x| x >= 2).count();
            }
            age = age.max(st.max_age);
            lost += st.lost_expiry;
            held += st.held_skips;
            busy += st.busy_skips;
            late += st.late_nappers;
            qmax = qmax.max(st.queue_max);
            qw = (qw.0.min(st.queue_wait.0), qw.1.max(st.queue_wait.1));
            late_naps += st.late_naps;
            golems += st.golems;
            trip_max = trip_max.max(st.max_trip);
        }
        let stats = RowStats {
            nights,
            least: if least == usize::MAX { 0 } else { least },
            max_dbl,
            max_never,
            late,
            late_naps,
            qmax,
            lost,
            busy,
            age,
            trip_max,
            shortest,
            spacing,
            pick_gap,
            lmargin,
            seat_n,
            seat_margin,
            has_td,
            rule2: f(1).rule2,
            m_acc,
            catch_acc,
            late_acc,
            early_acc,
            seeds,
            per_lane: per_lane_n,
            clocked,
            slips: slip_acc,
            clock_early: early_c,
            clock_wait: wait_c,
        };
        let text = format!(
            "  {label}: complete nights {nights}: distinct nappers a night least {} of 24, never-picked a night most {max_never} (nights with one or more never-picked {nights_never}), double picks a night most {max_dbl}; villager-nights napped 0 / 1 / 2+ times: {v0} / {v1} / {v2}; the largest LAST_SLEPT age {age} gt (margin to 24,000: {}); summons lost to an expired LAST_SLEPT {lost:.0} over all seeds; picks lost to HELD (deferred: at the divert, or from another napper's pick to its release) {held}, to the lane's napper still away {busy}; late rejoins {late}; lie-downs past the night {late_naps}; held at the queue at once at most {qmax}, its waits {}; the longest divert -> back at the lane's merge hold {trip_max} gt (the older siding-window model's deadline 700 gt after the pick's divert; under D-and-PRESENT the catch window is the deadline); the shortest lane at any summon {}; the stamp-age margin at a rider's next own-pocket S (ironspec 02:41 / 03:22 PT (5), each term with its source): the least lap {} gt (the model's least turn spacing of any rider: its lane's carts in rotation x 100) - 32 (the stamp's age short of the lap: onechunk's read, a 568-gt-old stamp at a 600-gt lap, farmopt 02:00) - 599 (GOLEM_DETECTED_RECENTLY's TTL) = {} (bar >= 3; the model's lock {lock_used}); the queue holds one (ironspec 19:53 PT (2)(d)): the least gap between two picks' diverts {} gt against the arrivals' spread {} + the longest stay at the queue {}: margin {}; golems a day {:.1}",
            if least == usize::MAX { 0 } else { least },
            24_000 - age,
            if qw.0 == i64::MAX { "-".to_string() } else { format!("{}..{} gt", qw.0, qw.1) },
            if shortest == usize::MAX { 0 } else { shortest },
            if spacing == i64::MAX { "-".to_string() } else { spacing.to_string() },
            if spacing == i64::MAX { "-".to_string() } else { (spacing - 32 - 599).to_string() },
            if pick_gap == i64::MAX { "-".to_string() } else { pick_gap.to_string() },
            leg.1 - leg.0,
            if qw.1 == i64::MIN { "-".to_string() } else { qw.1.to_string() },
            if pick_gap == i64::MAX || qw.1 == i64::MIN { "-".to_string() } else { (pick_gap - (leg.1 - leg.0) - qw.1).to_string() },
            golems / seeds as f64
        ) + &format!(
            "; ONE IN FLIGHT under F1 (ironspec 00:12 PT (3a), HELD set at the arrival): pick -> arrival at most {} gt (60 to the divert + the leg's worst {} + a world open's 2-tick lead) against the least pick spacing {}: margin {}",
            60 + leg.1 + 2,
            leg.1,
            if pick_gap == i64::MAX { "NONE".to_string() } else { pick_gap.to_string() },
            if pick_gap == i64::MAX { "-".to_string() } else { (pick_gap - (60 + leg.1 + 2)).to_string() }
        ) + &format!(
            "; R -> the next napper's lie-down at least {} gt (ironspec 22:18 PT; the inherited bar 20; the 16-ball exit is 58f770cc's measured R -> exit, set against this)",
            if liedown == i64::MAX { "-".to_string() } else { liedown.to_string() }
        ) + &format!(
            "; R -> back in lane (the rejoin) {} (onechunk's record7d smoke night: 346..404); the away latch's least margin, a rejoin against its lane's next pick opportunity: {} (a latch still set at the pick is a skipped pick, the {busy} above); each lane's pick opportunities a night least {} against its {per_lane_n} riders (slack {}){}",
            if bil.0 == i64::MAX { "-".to_string() } else { format!("{}..{} gt", bil.0, bil.1) },
            if lmargin == i64::MAX { "NONE measured".to_string() } else { format!("{lmargin} gt") },
            opps.iter().map(|&v| if v == usize::MAX { "-".to_string() } else { v.to_string() }).collect::<Vec<_>>().join(" / "),
            opps.iter().filter(|&&v| v != usize::MAX).map(|&v| v as i64 - per_lane_n as i64).min().map_or("NONE".to_string(), |s| if s <= 0 { format!("{s}: NO SLACK, any skipped pick costs a nap") } else { s.to_string() }),
            if !has_td {
                String::new()
            } else {
                // (ironspec 00:40 PT: the bar is the measured boundary + 2: m = D - PRESENT >= max(L_latch + 1,
                // settle - 14) + 2, L_latch 6 (the junction driver's worst), settle not yet measured: 9 while settle
                // <= 23)
                let per: Vec<String> = m_acc
                    .iter()
                    .enumerate()
                    .map(|(l, &(m, n))| if n == 0 { format!("lane {l}: no samples") } else { format!("lane {l} least {m} gt ({n} samples)") })
                    .collect();
                let worst = m_acc.iter().filter(|x| x.1 > 0).map(|x| x.0).min();
                // (ironspec 01:27 / 01:31 PT: the catch window per lane, its late and early slack, raw and under a world
                // open's 2-gt redstone lead; the bar >= 2 after the lead, judged by the caller on the operating point)
                let catch: Vec<String> = catch_acc
                    .iter()
                    .enumerate()
                    .map(|(l, &(lt, er, n))| if n == 0 { format!("lane {l}: no samples") } else { format!("lane {l} late slack {lt} (N = 2: {}), early slack {er} (N = 2: {}), {n} visits, late rejoins {}, early rejoins {}", lt - 2, er - 2, late_acc[l], early_acc[l]) })
                    .collect();
                format!(
                    "; m = the predecessor's D press - PRESENT over the on-time rejoins ({sm_n} samples): {} (ironspec 00:40 PT's bar m >= max(L_latch 6 + 1, settle - 14) + 2 = 10 with ironfarm's worst settle 22 (PRESENT -> at rest at an arrival of 0.10 on K, its worst phase); with a world open's 2-gt lead m - 2: {}); samples under 9: {m_low_n}{}; the catch window (ironspec 01:31 / 01:59 PT: Q-up before the sample it catches, the late slack, and after the previous sample, the early slack; under the design of record's D-and-PRESENT release an early catch rejoins behind the cart ahead of its predecessor, so both bind): {} (bar >= 2 after the lead, late and early classes 0); R -> PRESENT over the on-time visits {} (onechunk's record7d: 258..316)",
                    per.join(", "),
                    worst.map_or("no samples".to_string(), |w| format!("least {}", w - 2)),
                    if m_low_list.is_empty() { String::new() } else { format!(" (the first {}: {})", m_low_list.len(), m_low_list.join("; ")) },
                    catch.join(", "),
                    if ret_acc.0 == i64::MAX { "-".to_string() } else { format!("{}..{} gt", ret_acc.0, ret_acc.1) }
                ) + &if cwait_acc.0 == i64::MAX {
                    String::new()
                } else {
                    format!(
                        "; B3's clocked exit (ironspec 02:18 / 02:27 PT; the tap hold with the napper aboard driven by a free-running per-lane pulse train, day-1 10:37 PT, unless --gated-tap): the wait at the hold {}..{} gt (per lane: {}), visits whose arrival passed their tap (the stop busy: the next pulse) {slip_acc}, visits launched by an EARLIER pulse (a wait of a period or more) per lane {:?}",
                        cwait_acc.0,
                        cwait_acc.1,
                        wait_c.iter().map(|w| if w.0 == i64::MAX { "-".to_string() } else { format!("{}..{}", w.0, w.1) }).collect::<Vec<_>>().join(" / "),
                        early_c
                    )
                }
            }
        ) + &if seat_n == 0 {
            String::new()
        } else {
            format!(
                "; summon reach (ironspec 21:18 PT (2)): {seat_n} seat stays, each from the queue's arrival to the stop's entry, ending at its batch's S + {}..{} (the stamp's S + 599: least margin {}{}); stays overlapping another napper's passage through y -12..-1 (its R + 39..82): at the seat {seat_ov} of {seat_n} ({:.1} %), below the seat (the divert to the arrival) {below_ov} of {seat_n} ({:.1} %)",
                seat_end.0,
                seat_end.1,
                seat_margin,
                if seat_margin > 0 { ": every stay inside its own stamp" } else { ": A STAY OUTLIVES ITS STAMP" },
                100.0 * seat_ov as f64 / seat_n as f64,
                100.0 * below_ov as f64 / seat_n as f64
            )
        };
        (text, stats)
    };
    let h_op = operating_hold;
    // (the reference rows: printed, not judged; their scope declared once)
    lines.push(format!("  {}", v.scope("the reference rows (the lane model as benched, the pre-contract queue matrix, the no-queue row) and the cap matrix (the holds other than the operating point)", "printed for comparison; the operating point's row is the judged one")));
    lines.push(row("the lane model as benched (no queue: the stop's entry 60 gt after the divert; a reference row, not judged)", &|seed| LaneOpts { days, seed, ..LaneOpts::lead() }).0);
    for &h in holds {
        for held in [false, true] {
            let label = format!(
                "the real legs (the divert to the arrival detector {}..{} gt: pick -> arrival {}..{}), the queue's hold {h} gt, the release -> the stop's entry {to_entry} gt, HELD {}",
                leg.0,
                leg.1,
                leg.0 + 60,
                leg.1 + 60,
                if held { "sampled at the divert ((i))" } else { "not sampled" }
            );
            // (the pre-contract queue model: a reference matrix, not judged)
            lines.push(row(&format!("{label} (a reference row, not judged)"), &|seed| LaneOpts { days, seed, queue_leg: Some(leg), queue_hold: h, to_entry, held_at_divert: held, ..LaneOpts::lead() }).0);
        }
    }
    // (today's return legs: R -> rest on the merge hold 89 (tick-exact's official ride, the stop 9 down),
    // released by the napper's own pulse at REL + `slot` once at rest 20 gt before the rail's rise (k_lock 42:
    // the bit at H2 is set at rest + 10 and sampled at E + 2, the rail at E + 12; r_leg 109 = 89 + 20 holds for
    // k_lock 42 only), the rail's rise -> PRESENT `post` (7b90c3f1: 18 + 79 / 94 / 110, + 16, + 2 at k_sw 6),
    // the deadline the predecessor's D pass at its release + `t_d` (v6 §7: 93 / 130 / 167); HELD set at the
    // pick (ironspec 22:05 PT, the contract))
    let today = |seed: u64, q: Option<(i64, i64)>, h: i64, held: bool| LaneOpts {
        days,
        seed,
        queue_leg: q,
        queue_hold: h,
        to_entry: if q.is_some() { to_entry } else { 0 },
        held_at_divert: false,
        held_at_pick: held,
        r_leg: x.r_leg.unwrap_or(109),
        window: (slot, slot),
        post_lane: Some(post),
        t_d: Some(t_d),
        post_spread: x.post_spread,
        t_d_spread: x.t_d_spread,
        siding_any: x.siding_any,
        clock_exit: x.clock_exit,
        exit_phase: x.exit_phase,
        exit_pickup: x.exit_pickup,
        hold_lead: x.hold_lead,
        gated_tap: x.gated_tap,
        rs_delay: x.rs_delay,
        late_sleep: x.late_sleep.clone(),
        // (the batch's lanes launch s 12 apart: each lane's divert 12 gt after the one before)
        spacing: 12,
        offsets: Some(offsets),
        trip,
        lock_gt: lock,
        trip_hist: x.trip_hist.clone(),
        pick_window: x.pick_window,
        no_quota: x.no_quota,
        late_latch: x.late_latch,
        ret_hist: x.ret_hist.clone(),
        ret_hist_lane: x.ret_hist_lane.clone(),
        rule2: x.rule2,
        ..LaneOpts::lead()
    };
    // (ironspec 01:12 PT (7): the picker search instead of the rows; the chain fix: judged, the design's own picker (k 10,
    // phases 0/3/6, 8 opportunities a lane, no count) must be among the configurations passing every class)
    if x.search {
        let (search_lines, passing) = picker_search(&|seed| today(seed, Some(leg), h_op, true), seeds, x.pick_window.map_or(4, |w| w.0));
        lines.extend(search_lines);
        let design = "k 10, 8 opportunities a lane (window 80 cycles), phases [0, 3, 6], no count";
        let w = v.bar("the picker search: the design's picker passes the normal night and all four robustness classes", passing.iter().any(|p| p == design));
        lines.push(format!("=> the design's picker ({design}): {w}"));
        return Ok(lines);
    }
    // (ironspec 23:19 PT: the known-answer frame pin: b8949c96's real-rest trace mapped to this model's to_entry)
    if let Some(e) = x.frame_pin {
        lines.push(format!(
            "frame pin (ironspec 23:19 PT): b8949c96's real-rest trace on record7's built R1 (NOT-ready off before tick 1, the queue rail powered after the 4th tick, the stop's entry {e} gt after NOT-ready's fall); this model's release instant is R (the stop free) and NOT-ready falls (b) - (a) after it (line 7's hand worst 8, less 58f770cc's measured R-detector lead 2: + 6), so that rest's to_entry in this model is {e} + 8 - 2 = {}; the columns run: to_entry {to_entry}{}",
            e + 6,
            if to_entry == e + 6 { " (this rest)" } else { "" }
        ));
    }
    if let Some(h) = &x.trip_hist {
        let (a, b) = hist_range(h);
        lines.push(format!(
            "entry -> R drawn per visit from the measured distribution ({} visits, {a}..{b} gt: {}) instead of uniformly over {}..{}",
            h.iter().map(|v| v.1 as u64).sum::<u64>(),
            h.iter().map(|(v, n)| format!("{v}:{n}")).collect::<Vec<_>>().join(" "),
            trip.0,
            trip.1
        ));
    }
    if let Some(h) = &x.ret_hist {
        let (a, b) = hist_range(h);
        lines.push(format!(
            "R -> the siding's R (PRESENT) drawn per visit from the measured distribution ({} visits, {a}..{b} gt: {}) instead of R + 109 + the slot's wait + the post",
            h.iter().map(|v| v.1 as u64).sum::<u64>(),
            h.iter().map(|(v, n)| format!("{v}:{n}")).collect::<Vec<_>>().join(" ")
        ));
    }
    if let Some((h, n0)) = &x.late_sleep {
        let (a, b) = hist_range(h);
        let total: u64 = h.iter().map(|v| v.1 as u64).sum();
        let late: u64 = h.iter().filter(|(v, _)| *v > n0 + 6).map(|v| v.1 as u64).sum();
        lines.push(format!(
            "the late sleeper (G1 07:05 PT; b8949c96's rows, pending b8949c96's review and its onGround fix): each visit's lie-down drawn from {total} rows (asleep at +{a}..+{b} gt past the stop's entry), the overrun past the normal +{n0} added to its entry -> R: late (past +{}) {late} of {total} ({:.1} %), the overrun at most {} gt",
            n0 + 6,
            100.0 * late as f64 / total.max(1) as f64,
            (b - n0).max(0)
        ));
    }
    for (l, h) in x.ret_hist_lane.iter().enumerate() {
        if let Some(h) = h {
            let (a, b) = hist_range(h);
            lines.push(format!(
                "lane {l}'s own R -> the siding's R per visit ({} visits, {a}..{b} gt: {})",
                h.iter().map(|v| v.1 as u64).sum::<u64>(),
                h.iter().map(|(v, n)| format!("{v}:{n}")).collect::<Vec<_>>().join(" ")
            ));
        }
    }
    if x.rule2 {
        lines.push("rule 2 on TODAY'S rows: a napper rejoins only right behind its own predecessor, at a pass it is on its siding for (by the predecessor's D pass); a late one waits for the next pass, a lane lap later".to_string());
    }
    if let Some((a, n)) = x.pick_window {
        lines.push(format!(
            "the pick window: {n} cycles from cycle {a} of the night (the controller's C {}..{} at k 10: {} opportunities a lane), {}",
            141,
            141 + n,
            n / 10,
            if x.no_quota { "every opportunity picks (no per-lane count, as the controller)" } else { "a lane stops at its 8 (the model's quota)" }
        ));
    }
    // (G1 on 7897f4ba7 (a): the no-queue row is a reference, not a design configuration: with the stop's entry 60 gt after
    // the divert every napper is back a cycle or more before its predecessor's D press, so under the design of record's
    // D-and-PRESENT release every rejoin is early and m has no on-time sample there)
    lines.push(row(&format!("TODAY'S RETURN (lanes s 12 apart; the stop's entry -> R {}..{}; R -> merge hold 89, the napper's pulse at REL + {slot} with k_lock, -> PRESENT {}/{}/{}, the deadline at the predecessor's D pass + {}/{}/{}), no queue (the stop's entry 60 gt after the divert; a reference row, not judged: without the queue's legs every napper is back a cycle early, so under D-and-PRESENT every rejoin is early and m has no on-time sample)", trip.0, trip.1, post[0], post[1], post[2], t_d[0], t_d[1], t_d[2]), &|seed| today(seed, None, 0, false)).0);
    for &h in holds {
        let op = h == h_op;
        let (text, st) = row(&format!("TODAY'S RETURN with the real legs ({}..{} gt to the arrival detector), the queue's hold {h} gt, the release -> entry {to_entry} gt, HELD set at the pick (reset at the release); the cap assumes the A6 fix (entry -> R <= {}: record7e's official approach stalls break it, ironspec 01:00 PT){}", leg.0, leg.1, x.trip_hist.as_ref().map_or(trip.1, |h| hist_range(h).1), if op { "; THE OPERATING POINT (--operating-hold), judged" } else { "; the cap matrix, not judged" }), &|seed| today(seed, Some(leg), h, true));
        lines.push(text);
        if op {
            lines.push(judge_operating(&st, leg, &format!("normal operation, TODAY'S RETURN at the operating hold {h}"), v));
            // (ironspec 08:43 / 08:46 / 08:49 PT: THE STOP OCCUPANCY BUDGET, each term with its source and review status.
            // The stop is busy from a napper's queue release to its stop release R. With the clocked exit, napper i's R is
            // its tap (its divert + the latest + its lane's phase) + its post-tap pickup p_i, and napper i+1, released at
            // max(its divert + leg + hold, R_i), slips a period when its own release + to_entry + its entry -> R passes its
            // tap; with a wait, substituting R_i gives: to_entry + trip_{i+1} + p_i <= the spacing of the two diverts (the
            // latest, the clock's margin, the lane phase and the legs cancel). Without the clock the next napper waits
            // whenever the leg's spread + to_entry + the worst entry -> R passes that spacing. The world-open shift and
            // ironspec's 10-gt margin added; the denominator the NOMINAL least pick spacing (G1 09:11: a measured one only
            // once every reachable case is printed).)
            let worst = x.trip_hist.as_ref().map_or(trip.1, |hh| hist_range(hh).1) + x.late_sleep.as_ref().map_or(0, |(hh, n0)| (hist_range(hh).1 - n0).max(0));
            let shift = x.world_open_shift.unwrap_or(WORLD_OPEN_SHIFT);
            let (clk_txt, clk) = match x.clock_exit {
                Some(_) => (format!(" + the post-tap pickup's upper end {} (the clocked exit: the tap's phase, its margin and the legs cancel)", x.exit_pickup.1), x.exit_pickup.1),
                None => (format!(" + the leg's spread {} (no clocked exit: the stop releases at the natural R)", leg.1 - leg.0), leg.1 - leg.0),
            };
            let nominal = 300i64;
            let physical = to_entry + worst + clk + x.rs_delay;
            let phys_spare = nominal - physical;
            let (tol, push) = (x.clock_tol.unwrap_or(0), x.push_shift.unwrap_or(0));
            // (ironspec 10:42 PT (5), per-lane taps: napper i of lane P leaves at its tap (its divert + the latest + e_P) +
            // its post-tap term, napper i+1 of lane N is released no sooner (+ d_RS) and slips past its own tap (its divert
            // + the latest + e_N) unless to_entry + its entry -> R + the post-tap term + d_RS <= the two diverts' spacing -
            // (e_P - e_N): the spacing per consecutive lane pair from the pick phases (l k / lanes cycles) and the launch
            // offsets; with one shared absolute phase every pair's effective spacing is the pick spacing)
            let pair_txt: Vec<String>;
            let mut pair_min = phys_spare;
            if x.clock_exit.is_some() {
                let e = x.exit_phase.unwrap_or([0; 3]);
                let lo = LaneOpts::lead();
                let ph: Vec<i64> = (0..3).map(|l| (l * lo.k / 3) as i64).collect();
                let mut parts = Vec::new();
                for p in 0..3usize {
                    let n = (p + 1) % 3;
                    let dp = (ph[n] - ph[p]).rem_euclid(lo.k as i64);
                    let spacing = 100 * dp + offsets[n] - offsets[p];
                    let eff = spacing - (e[p] - e[n]);
                    pair_min = pair_min.min(eff - physical);
                    parts.push(format!("lane {p} -> lane {n}: the diverts {spacing} gt apart - the phase difference {} = {eff}, spare {}", e[p] - e[n], eff - physical));
                }
                pair_txt = parts;
            } else {
                pair_txt = vec!["no clocked exit: the nominal 300".to_string()];
            }
            let design_spare = pair_min - shift - tol - push;
            // (G1's Z4 on 10bcdf661: a design-of-record run can't pass the budget on unmeasured terms defaulting to 0)
            let unmeasured: Vec<&str> = [("--clock-tol", x.clock_tol.is_none()), ("--push-shift", x.push_shift.is_none())].iter().filter(|(_, u)| *u).map(|(n, _)| *n).collect();
            let w = if x.design_of_record && !unmeasured.is_empty() {
                v.fail(&format!("the stop occupancy budget: NOT JUDGED in a design-of-record run ({} not given)", unmeasured.join(", ")));
                format!("NOT JUDGED ({} not given in a design-of-record run): FAIL", unmeasured.join(", "))
            } else {
                v.bar("the stop occupancy budget: the least physical spare (over the lane pairs under the per-lane taps) less the world-open shift, the clock's tolerance and the worst push shift >= 10 (ironspec 08:46 / 10:42 PT, G1 10:37 PT)", design_spare >= 10).to_string()
            };
            lines.push(format!(
                "  => the stop occupancy budget (ironspec 08:43 / 08:46 / 10:42 PT, G1 10:37 PT; provisional: entry -> R from this run's stated band or distribution): the physical slip condition to_entry {to_entry} (a parameter of this run: the way-in's measured release -> entry, its owner's) + the worst entry -> R {worst} ({}){clk_txt} + d_RS {} = {physical}, against the nominal least pick spacing {nominal} (spare {phys_spare}; the least divert spacing measured in this run {} printed, not used: G1 09:11) and per lane pair under the lane phases: {}; the least physical spare {pair_min}; less the world-open shift {shift} ({}), the clock's tolerance {}, the worst push shift {}: the design spare {design_spare} against >= 10 ({w}); the design priced: {}",
                if x.world_open_shift.is_some() { "--world-open-shift: the farm-chunk row's k + j (tick-exact 14:16 PT; 2 across chunks, ironspec 14:22 PT)" } else { "\"world-open term from one ride: 0..7 gt, uncalibrated\"" },
                if x.trip_hist.is_some() { "the measured per-visit distribution's worst" } else { "the band's upper end" },
                x.rs_delay,
                if st.pick_gap == i64::MAX { "NONE".to_string() } else { st.pick_gap.to_string() },
                pair_txt.join("; "),
                x.clock_tol.map_or("0 (not measured)".to_string(), |c| c.to_string()),
                x.push_shift.map_or("0 (push -> time shift not measured)".to_string(), |p| p.to_string()),
                match x.clock_exit {
                    Some(mg) => format!(
                        "the clocked exit with δ {mg}, the lane phases {:?}, the post-tap term --exit-pickup {}..{}, {}, the hold lead {}, d_RS {}",
                        x.exit_phase.unwrap_or([0; 3]),
                        x.exit_pickup.0,
                        x.exit_pickup.1,
                        if x.gated_tap { "a per-visit gated tap (scope)" } else { "a free-running per-lane train" },
                        x.hold_lead,
                        x.rs_delay
                    ),
                    None => format!("no clocked exit, d_RS {}", x.rs_delay),
                }
            ));
            // (ironspec 10:42 PT (1)) per-lane taps: every wait at the hold < 100 - the margin (the measured D-press spread +
            // the push shift + the world-open shift); G1's Z1 on 10bcdf661: under --gated-tap the bar is scoped by name (a
            // design-of-record run refuses it); G1's Z4: an unmeasured margin is NOT JUDGED in a design-of-record run
            if x.clock_exit.is_some() && x.gated_tap {
                lines.push(format!("  => {}", v.scope("the per-lane wait bar and the free-running tap's early-launch control", "--gated-tap: the per-visit tap model, no free-running train")));
            }
            if x.clock_exit.is_some() && !x.gated_tap && x.design_of_record && x.wait_margin.is_none() {
                v.fail("the per-lane taps' wait bar: NOT JUDGED in a design-of-record run (--wait-margin, the measured D-press spread + push shift, not given)");
                lines.push("  => the per-lane taps' wait bar: NOT JUDGED (--wait-margin not given in a design-of-record run)".to_string());
            } else if x.clock_exit.is_some() && !x.gated_tap {
                let margin = shift + x.wait_margin.unwrap_or(0);
                let mut per = Vec::new();
                let mut ok = true;
                for (l, wl) in st.clock_wait.iter().enumerate() {
                    if wl.1 == i64::MIN {
                        per.push(format!("lane {l}: no samples"));
                        ok = false;
                    } else {
                        ok &= wl.1 < 100 - margin;
                        per.push(format!("lane {l} {}..{} (spare {})", wl.0, wl.1, 100 - margin - wl.1 - 1));
                    }
                }
                let w = v.bar_n("the per-lane taps: every wait at the hold < 100 - the margin (ironspec 10:42 PT (1))", st.nights, ok);
                lines.push(format!(
                    "  => the per-lane taps (ironspec 10:42 PT (1)): the wait at the hold per lane {} against < 100 - the margin {margin} (the world-open shift {shift} + {} ({})): {w}",
                    per.join(", "),
                    x.wait_margin.unwrap_or(0),
                    if x.wait_margin.is_some() { "--wait-margin: the measured D-press spread + push shift" } else { "the D-press spread and the push shift not measured" }
                ));
            }
        }
    }
    // (ironspec 16:20, the chain audit (hole 21): the positive controls run on the JUDGED configuration (today's legs at
    // the operating hold), each planting its defect in what its bar reads, through the same row: the napper keeping its
    // slot and k 9 must break the 24 distinct; R -> Q-up 100 gt later must give late rejoins (ironspec 01:26 PT (c)); HELD
    // not set and a hold longer than the pick spacing must put two at the queue; a control without complete nights is NOT
    // JUDGED, never a bite)
    let (t1, c1) = row("CONTROL: the napper keeps its slot (a bypass rejoining in place; picks advance 10 mod 8 = 2 places), on the operating point", &|seed| LaneOpts { keep_place: true, lane_exact: false, ..today(seed, Some(leg), h_op, true) });
    let w1 = v.control("the keep-slot picker breaks the 24 distinct", c1.nights, c1.least < 24);
    lines.push(format!("{t1}; {w1}"));
    let (t2, c2) = row("CONTROL: k 9 with lane-exact re-entry (picks advance 9 - 7 = 2 places), on the operating point", &|seed| LaneOpts { k: 9, ..today(seed, Some(leg), h_op, true) });
    let w2 = v.control("k 9 breaks the 24 distinct", c2.nights, c2.least < 24);
    lines.push(format!("{t2}; {w2}"));
    let base_leg = x.r_leg.unwrap_or(109);
    let (t3, c3) = row(&format!("CONTROL (ironspec 01:26 PT (c)): R -> Q-up forced 100 gt later ({} instead of {base_leg}: every napper misses the sample it would catch), on the operating point", base_leg + 100), &|seed| LaneOpts { r_leg: base_leg + 100, ..today(seed, Some(leg), h_op, true) });
    let w3 = v.control("R -> Q-up 100 gt later gives late rejoins", c3.nights, c3.late > 0);
    lines.push(format!("{t3}; {w3}"));
    let (t4, c4) = row("CONTROL: HELD never set and the queue's hold 400 gt (longer than the least pick spacing), on the operating point's legs", &|seed| LaneOpts { held_at_pick: false, held_at_divert: false, queue_hold: 400, ..today(seed, Some(leg), h_op, true) });
    // (G1's Y8 on f21c9f600: the control's single-change twin, the design with the same 400-gt hold and HELD set, run and
    // judged: the control then differs from it by HELD alone)
    let (t4t, c4t) = row("the control's twin: HELD set and the queue's hold 400 gt, on the operating point's legs", &|seed| LaneOpts { queue_hold: 400, ..today(seed, Some(leg), h_op, true) });
    let w4t = v.bar_n("the HELD control's twin (the 400-gt hold with HELD set): at most one held at the queue", c4t.nights, c4t.qmax <= 1);
    lines.push(format!("{t4t}; held at the queue at once at most {} ({w4t})", c4t.qmax));
    let w4 = v.control("without HELD and with a hold past the pick spacing two are held at the queue", c4.nights, c4.qmax >= 2);
    lines.push(format!("{t4}; {w4}"));
    // (day-1 10:37 PT) the free-running tap's positive: every lane's phase a period later, so each napper waits a period
    // more at the hold: the early-launch bar must see it (an earlier pulse launches them)
    if x.clock_exit.is_some() && !x.gated_tap {
        let e = x.exit_phase.unwrap_or([0; 3]);
        let (t5, c5) = row("CONTROL (day-1 10:37 PT): every lane's tap phase a period later (each napper waits a period more at the hold), on the operating point", &|seed| LaneOpts { exit_phase: Some([e[0] + 100, e[1] + 100, e[2] + 100]), ..today(seed, Some(leg), h_op, true) });
        let w5 = v.control("the free-running tap's early-launch bar sees a wait of a period", c5.nights, c5.clock_early.iter().sum::<u32>() > 0);
        lines.push(format!("{t5}; {w5}"));
        // (G1's Z5 on 10bcdf661: the wait cap's control, a single-change twin: every lane's phase later by the least
        // spare under the cap, so the worst wait reaches the cap (still under a period: no early launch) and the wait
        // bar's predicate must fail)
        let (_, op) = row("the operating point (for the wait cap's control)", &|seed| today(seed, Some(leg), h_op, true));
        let cap = 100 - x.world_open_shift.unwrap_or(WORLD_OPEN_SHIFT) - x.wait_margin.unwrap_or(0);
        let worst = op.clock_wait.iter().map(|w| w.1).max().unwrap_or(i64::MIN);
        if worst != i64::MIN && worst < cap {
            let bump = cap - worst;
            let (t6, c6) = row(&format!("CONTROL (G1's Z5): every lane's tap phase {bump} gt later (the worst wait to the cap {cap}), on the operating point"), &|seed| LaneOpts { exit_phase: Some([e[0] + bump, e[1] + bump, e[2] + bump]), ..today(seed, Some(leg), h_op, true) });
            let worst6 = c6.clock_wait.iter().map(|w| w.1).max().unwrap_or(i64::MIN);
            let w6 = v.control("the per-lane wait bar fails a wait at the cap", c6.nights, worst6 != i64::MIN && worst6 >= cap);
            lines.push(format!("{t6}; the worst wait {worst6} against < {cap} ({w6})"));
        } else {
            v.fail("the wait cap's control: NOT JUDGED (no operating waits, or the operating point already at the cap)");
        }
    }
    // (ironspec 07:40 / 07:41 PT, G1's conditions) THE EXIT-PHASE SWEEP: each lane's tap phase over the period, the other
    // lanes held; a phase is feasible for its lane when both of its slacks clear 20 after the lead, it has no late or early
    // rejoin and no early launch, and the run no slip; each feasible interval's edges get a control (the phase just
    // outside must fail the lane's catch-window bar)
    if let Some(step) = x.exit_phase_sweep {
        let base = x.exit_phase.unwrap_or([0; 3]);
        lines.push(format!(
            "THE EXIT-PHASE SWEEP (ironspec 07:40 / 07:41 / 10:42 PT, G1's conditions; the per-lane free-running taps, day-1 10:37 PT): each lane's tap phase over 0..99 step {step}, the other lanes at {base:?}, on the operating point; FEASIBLE: the lane's late and early slacks >= 20 after the 2-gt lead, no late or early rejoin, no early launch, every wait at the hold < 100 - the world-open shift {} - --wait-margin {}, no slip in the run (the world-open and push shifts not subtracted from the slacks: measured terms pending)",
            x.world_open_shift.unwrap_or(WORLD_OPEN_SHIFT),
            x.wait_margin.unwrap_or(0)
        ));
        for l in 0..3 {
            // (phase, late slack, early slack, samples, late rejoins, early rejoins, early launches, slips, the largest wait)
            let mut stats: Vec<(i64, i64, i64, usize, u32, u32, u32, u32, i64)> = Vec::new();
            for p in (0..100).step_by(step as usize) {
                let mut e = base;
                e[l] = p;
                let (_, s) = row(&format!("the sweep: lane {l} at phase {p}"), &|seed| LaneOpts { exit_phase: Some(e), ..today(seed, Some(leg), h_op, true) });
                let (lt, er, n) = s.catch_acc[l];
                stats.push((p, lt, er, n, s.late_acc[l], s.early_acc[l], s.clock_early[l], s.slips, s.clock_wait[l].1));
            }
            let wait_cap = 100 - x.world_open_shift.unwrap_or(WORLD_OPEN_SHIFT) - x.wait_margin.unwrap_or(0);
            let catch_ok = |s: &(i64, i64, i64, usize, u32, u32, u32, u32, i64)| s.3 > 0 && s.1 - WORLD_OPEN_LEAD >= CATCH_MIN && s.2 - WORLD_OPEN_LEAD >= CATCH_MIN && s.4 == 0 && s.5 == 0;
            let feasible = |s: &(i64, i64, i64, usize, u32, u32, u32, u32, i64)| catch_ok(s) && s.6 == 0 && s.7 == 0 && s.8 < wait_cap;
            let cells: Vec<String> = stats
                .iter()
                .map(|s| {
                    if s.3 == 0 {
                        format!("{}: no samples", s.0)
                    } else {
                        let mut c = format!("{}: {}/{} wait<={}", s.0, s.1 - 2, s.2 - 2, s.8);
                        if s.4 + s.5 > 0 {
                            c += &format!(" rejoins late {} early {}", s.4, s.5);
                        }
                        if s.6 > 0 {
                            c += &format!(" early launches {}", s.6);
                        }
                        if s.7 > 0 {
                            c += &format!(" slips {}", s.7);
                        }
                        c
                    }
                })
                .collect();
            let mut ivs: Vec<(usize, usize)> = Vec::new();
            for (i, s) in stats.iter().enumerate() {
                if feasible(s) {
                    match ivs.last_mut() {
                        Some(iv) if iv.1 + 1 == i => iv.1 = i,
                        _ => ivs.push((i, i)),
                    }
                }
            }
            let w = v.bar(&format!("the exit-phase sweep: lane {l} has a feasible tap phase"), !ivs.is_empty());
            let mut ctl: Vec<String> = Vec::new();
            // (G1's Z2 on 10bcdf661: the edges' controls were tautologies (an interval is maximal, so its neighbour is
            // infeasible by construction); now each edge and its neighbour are RE-RUN as single-change twins (only lane l's
            // phase differs) and judged through the REAL bars: the operating row's bars (`judge_operating`, a scratch verdict)
            // plus the sweep's own (the catch slacks >= CATCH_MIN after the world open's lead, no rejoin, no early launch, no
            // slip, the wait cap); the edge twin must pass every one, the neighbour twin must fail one, named)
            let real_bars = |st: &RowStats| -> Vec<String> {
                let mut scratch = crate::verdict::Verdict::new("the sweep's twin");
                let _ = judge_operating(st, leg, "twin", &mut scratch);
                let mut f: Vec<String> = scratch.fails().iter().map(|s| s.to_string()).collect();
                let (lt, er, n) = st.catch_acc[l];
                if n == 0 || lt - WORLD_OPEN_LEAD < CATCH_MIN || er - WORLD_OPEN_LEAD < CATCH_MIN {
                    f.push(format!("lane {l}'s catch slacks >= {CATCH_MIN} after the lead"));
                }
                if st.clock_early[l] > 0 {
                    f.push(format!("lane {l}: no early launch"));
                }
                if st.clock_wait[l].1 >= wait_cap {
                    f.push(format!("lane {l}: every wait < the cap {wait_cap}"));
                }
                f
            };
            for &(a, b) in &ivs {
                for (j, edge, side) in [(a.checked_sub(1), a, "below"), (if b + 1 < stats.len() { Some(b + 1) } else { None }, b, "above")] {
                    match j {
                        Some(j) => {
                            let twin = |p: i64| {
                                let mut e = base;
                                e[l] = p;
                                row(&format!("the sweep's twin: lane {l} at phase {p}"), &|seed| LaneOpts { exit_phase: Some(e), ..today(seed, Some(leg), h_op, true) }).1
                            };
                            let (pe, pn) = (stats[edge].0, stats[j].0);
                            let (fe, fn_) = (real_bars(&twin(pe)), real_bars(&twin(pn)));
                            let c = v.control(&format!("the exit-phase sweep: lane {l}'s edge {side} (phase {pe}) passes every real bar and its single-change twin (phase {pn}) fails one"), 1, fe.is_empty() && !fn_.is_empty());
                            ctl.push(format!(
                                "phase {pe} (edge, {side}): {} / phase {pn} (twin): {}: {c}",
                                if fe.is_empty() { "every bar passes".to_string() } else { format!("FAILS {}", fe.join(", ")) },
                                if fn_.is_empty() { "every bar passes".to_string() } else { format!("bound by {}", fn_.join(", ")) }
                            ));
                        }
                        None => ctl.push(format!("{side}: the period's end (the interval meets the sweep's edge; the wrap is not merged)")),
                    }
                }
            }
            lines.push(format!(
                "  lane {l}: per phase (late / early slack after the lead): {}; feasible intervals: {} ({w}); the edges' controls: {}",
                cells.join(", "),
                if ivs.is_empty() { "NONE".to_string() } else { ivs.iter().map(|&(a, b)| format!("{}..{}", stats[a].0, stats[b].0)).collect::<Vec<_>>().join(", ") },
                if ctl.is_empty() { "none (no interval)".to_string() } else { ctl.join("; ") }
            ));
        }
    }
    // (ironspec 20:53 PT; farmopt: one late return per lane at the worst point) one napper's stop cycle `add` gt
    // longer, injected at the first pick at or after every `step` of one mid-run night; against the same seed
    // without it: the picks skipped (the lane's napper still away, or HELD), the late rejoins, the villagers
    // unnapped that night and the next (of 24), the summons lost over that day and the next two
    let (adds, step) = late;
    // (the stress and late rows run at the operating point's hold)
    let h0 = h_op;
    let d_inj = LaneOpts::lead().warmup as i64 + 4;
    let distinct = |st: &LaneStats, night: i64| {
        let mut seen = [false; 64];
        for &(nt, c, _) in &st.nap_log {
            if nt == night && c < 64 {
                seen[c] = true;
            }
        }
        seen.iter().filter(|&&x| x).count()
    };
    let gdays = |st: &LaneStats| (d_inj..=d_inj + 2).map(|d| st.golems_day.get(&d).copied().unwrap_or(0) as i64).sum::<i64>();
    // (the pick slots of the design's lanes: k 10 at phases 0 / 3 / 6)
    let offsets_pick = |slot: i64| [0i64, 3, 6].contains(&(slot % 10));
    for (vname, latch, rule2) in [("", false, false), (" WITH (A) THE LATE LATCH", true, false), (" WITH RULE 2 (a late napper waits a lane lap and rejoins behind its predecessor)", false, true)] {
    for &add in adds {
        // per lane: (injections, the most skipped, the most late rejoins, the least distinct that night, the next,
        // the most summons lost, the worst injection's pick time)
        let mut per: [(u32, i64, i64, usize, usize, i64, i64); 3] = [(0, i64::MIN, i64::MIN, usize::MAX, usize::MAX, i64::MIN, 0); 3];
        let run_days = (d_inj + 5) as usize;
        let mut base_d = usize::MAX;
        for seed in 1..=seeds.min(3) {
            let base_o = LaneOpts { days: run_days, late_latch: latch, rule2, ..today(seed, Some(leg), h0, true) };
            let base = simulate_lanes_scored(&base_o).1;
            let (b_skip, b_late, b_g) = ((base.busy_skips + base.held_skips) as i64, base.late_nappers as i64, gdays(&base));
            let b_d0 = distinct(&base, d_inj);
            let b_d1 = distinct(&base, d_inj + 1);
            let mut hat = 0i64;
            while hat < 11_000 {
                let o = LaneOpts { hold_at: Some((d_inj as usize, hat, add)), ..base_o.clone() };
                let st = simulate_lanes_scored(&o).1;
                hat += step;
                let Some((l, _, t_pick)) = st.hold_lane else { continue };
                let l = l.min(2);
                let p = &mut per[l];
                p.0 += 1;
                let skip = (st.busy_skips + st.held_skips) as i64 - b_skip;
                let late_n = st.late_nappers as i64 - b_late;
                let lost = b_g - gdays(&st);
                if lost > p.5 {
                    p.6 = t_pick;
                }
                p.1 = p.1.max(skip);
                p.2 = p.2.max(late_n);
                p.3 = p.3.min(distinct(&st, d_inj));
                p.4 = p.4.min(distinct(&st, d_inj + 1));
                p.5 = p.5.max(lost);
            }
            base_d = base_d.min(b_d0.min(b_d1));
        }
        let lane_rows: Vec<String> = per
            .iter()
            .enumerate()
            .map(|(l, p)| {
                if p.0 == 0 {
                    format!("lane {l}: no pick took the injection")
                } else {
                    format!(
                        "lane {l} ({} injections): picks skipped {} more at most, late rejoins {} more at most, distinct nappers that night least {} of 24 (the next night {}), summons lost over that day and the next two at most {} (the worst at the pick at model tick {})",
                        p.0, p.1, p.2, p.3, p.4, p.5, p.6
                    )
                }
            })
            .collect();
        let worst_lost = per.iter().filter(|p| p.0 > 0).map(|p| p.5).max().unwrap_or(i64::MIN);
        let next_min = per.iter().filter(|p| p.0 > 0).map(|p| p.4).min().unwrap_or(usize::MAX);
        // (ironspec 23:33 PT (2): the bar for one late return: <= 1 summon lost, the following night back to 24
        // distinct, the lane's cyclic order restored within one lane cycle (here: the following night's 24 distinct
        // with no extra late rejoin is the order's proxy; the order itself is not compared); G1 on 7897f4ba7 (1),
        // ironspec 01:46 PT (b): +1 and +2 gt are normal operation (a world open at N <= 2), judged on the summons and the
        // next night; a longer return is the stress class: the next night judged, its summons priced)
        let normal = add <= 2;
        let next_ok = next_min == 24;
        // (ironspec's hole 7 on the tenth: the normal class judges that night's distinct count too)
        let night_min = per.iter().filter(|p| p.0 > 0).map(|p| p.3).min().unwrap_or(usize::MAX);
        let n_inj: usize = per.iter().map(|p| p.0 as usize).sum();
        let verdict = if normal {
            let w = v.bar_n(&format!("normal operation: a late return of +{add} gt{vname}: <= 1 summon lost, 24 distinct that night and the next"), n_inj, worst_lost <= 1 && night_min == 24 && next_ok);
            format!("NORMAL OPERATION (a world open at N <= 2): <= 1 lost, 24 that night and the next: {w}")
        } else {
            let w = v.bar_n(&format!("the stress class: a late return of +{add} gt{vname}: the next night 24"), n_inj, next_ok);
            format!("THE STRESS CLASS (excluded from normal operation by the catch window; its summons and that night priced): the next night 24: {w}")
        };
        lines.push(format!(
            "LATE RETURN +{add} gt{vname} (one napper's stop cycle {add} gt longer, injected at the first pick at or after every {step} gt of night {d_inj} from dusk, TODAY'S RETURN with the real legs, the queue's hold {h0} gt; seeds 1..={}; against the same seed without it, whose nights {d_inj} and {} have {} distinct at least): {}; => at most {} summon(s) lost per late return, the following night {} distinct at least; {verdict}",
            seeds.min(3),
            d_inj + 1,
            if base_d == usize::MAX { "-".to_string() } else { base_d.to_string() },
            lane_rows.join("; "),
            if worst_lost == i64::MIN { "-".to_string() } else { worst_lost.to_string() },
            if next_min == usize::MAX { "-".to_string() } else { next_min.to_string() }
        ));
    }
    }
    // (ironspec 00:12 PT (2): (B') as a slack candidate in its own right: ONE skipped pick a night, at every slot of
    // the window, on TODAY'S rows (the window and the count as given); against the same seed without it. The chain audit:
    // the controller's window is required (no unsourced default), and only an injection that took (the slot's pick would
    // have been made: picked -> skipped, ironspec 04:07 PT) counts as a sample)
    if let Some((wa, wn)) = x.pick_window {
        let run_days = (d_inj + 5) as usize;
        let (mut n, mut noop, mut worst_lost, mut least_night, mut least_next, mut worst_slot) = (0u32, 0u32, i64::MIN, usize::MAX, usize::MAX, -1i64);
        for seed in 1..=seeds.min(3) {
            let base_o = LaneOpts { days: run_days, ..today(seed, Some(leg), h0, true) };
            let base = simulate_lanes_scored(&base_o).1;
            let b_g = gdays(&base);
            for slot in 0..wn {
                if !offsets_pick(slot) {
                    continue;
                }
                let o = LaneOpts { skip_pick: Some((d_inj as usize, slot)), ..base_o.clone() };
                let st = simulate_lanes_scored(&o).1;
                if !st.skip_took {
                    noop += 1;
                    continue;
                }
                n += 1;
                let lost = b_g - gdays(&st);
                if lost > worst_lost {
                    worst_lost = lost;
                    worst_slot = slot;
                }
                least_night = least_night.min(distinct(&st, d_inj));
                least_next = least_next.min(distinct(&st, d_inj + 1));
            }
        }
        let w = v.bar_n("the stress class: one skipped pick: the next night 24", n as usize, least_next == 24);
        lines.push(format!(
            "ONE SKIPPED PICK a night, THE STRESS CLASS (a pick lost to a world open at the divert: priced in expectation, ironspec 01:46 PT (b); ironspec 00:12 PT (2), the slack test): the pick opportunity at each slot of night {d_inj}'s window (cycles {wa}..{} from the night's start, {}) skipped as if its lane's napper were away: {n} injections took (picked -> skipped), {noop} found the slot's pick not due (the lane's napper away or HELD: no override, not counted); seeds 1..={}; against the same seed: summons lost over that day and the next two at most {} (the worst at slot {worst_slot}: priced), that night's distinct least {}, the next night {} (the bar: 24 the next night: {w})",
            wa + wn,
            if x.no_quota { "every opportunity picks" } else { "a lane stops at its 8" },
            seeds.min(3),
            if worst_lost == i64::MIN { "-".to_string() } else { worst_lost.to_string() },
            if least_night == usize::MAX { "NONE".to_string() } else { least_night.to_string() },
            if least_next == usize::MAX { "NONE".to_string() } else { least_next.to_string() }
        ));
    } else {
        lines.push(format!("ONE SKIPPED PICK a night: {}", v.scope("the skipped-pick row", "no --pick-window: the slots are the controller's window's, which the run must give")));
    }
    // (ironspec 00:12 PT (2): class b's cost BEYOND the same skip with no napper held: a night skip at each in_day of
    // night d_inj, alone and with the napper picked just before it held through the day (its stop cycle 12,000 gt
    // longer: it waits for the next REST in the stop))
    {
        let run_days = (d_inj + 5) as usize;
        let (mut n, mut worst_extra, mut worst_skip, mut worst_at) = (0u32, i64::MIN, i64::MIN, 0i64);
        for seed in 1..=seeds.min(3) {
            let plain = LaneOpts { days: run_days, ..today(seed, Some(leg), h0, true) };
            let b_g = gdays(&simulate_lanes_scored(&plain).1);
            let mut at = 1_000i64;
            while at < 11_000 {
                let skip_o = LaneOpts { jump: Some((d_inj as usize, at)), ..plain.clone() };
                let s_g = gdays(&simulate_lanes_scored(&skip_o).1);
                let held_o = LaneOpts { hold_at: Some((d_inj as usize, (at - 300).max(0), 12_000)), ..skip_o.clone() };
                let h = simulate_lanes_scored(&held_o).1;
                at += 250;
                if h.hold_lane.is_none() {
                    continue;
                }
                n += 1;
                let extra = s_g - gdays(&h);
                if extra > worst_extra {
                    worst_extra = extra;
                    worst_at = at - 250;
                }
                worst_skip = worst_skip.max(b_g - s_g);
            }
        }
        lines.push(format!(
            "CLASS B (ironspec 00:12 PT (2)): a night skip at every 250 gt of night {d_inj} (in_day 1,000..10,750), alone and with the napper picked within 300 gt before it held in the stop through the day (stop cycle + 12,000 gt), {n} runs (seeds 1..={}): the skip alone costs at most {} summons over that day and the next two (against no skip); class b costs at most {} MORE than the same skip (the worst at in_day {worst_at}); {}",
            seeds.min(3),
            if worst_skip == i64::MIN { "-".to_string() } else { worst_skip.to_string() },
            if worst_extra == i64::MIN { "NONE (no run held a napper)".to_string() } else { worst_extra.to_string() },
            v.scope("class B's cost", "priced under the skip ruling, ironspec 00:12 / 01:46 PT")
        ));
    }
    Ok(lines)
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

    /// Every pick's rejoin in the release log: (rejoins checked, rejoins out of order, picks that
    /// never came back). A pick is a cart whose next release in its lane comes more than a lap later;
    /// in order means its lane releases it right behind the cart that went before it at its pick (P)
    /// and right before the one that went after it (S). A pick more than 4 laps before the log's end
    /// must reappear (G1 on 3ebd4e471: a model dropping a napper must fail).
    fn order_breaks(o: &LaneOpts) -> (usize, usize, usize) {
        let st = simulate_lanes_scored(o).1;
        let (mut checked, mut broken, mut lost) = (0usize, 0usize, 0usize);
        for l in 0..o.lanes {
            let seq: Vec<usize> = st.release_log.iter().filter(|r| r.1 == l).map(|r| r.2).collect();
            // (the lost check from the first release on, G1 on 72d510183; the order check needs a
            // cart before, so from the second)
            for i in 0..seq.len().saturating_sub(1) {
                let Some(off) = seq[i + 1..].iter().position(|&x| x == seq[i]) else {
                    if i + 4 * o.per_lane < seq.len() {
                        lost += 1;
                    }
                    continue;
                };
                let j = i + 1 + off;
                if i >= 1 && j > i + o.per_lane && j + 1 < seq.len() {
                    checked += 1;
                    if !(seq[j - 1] == seq[i - 1] && seq[j + 1] == seq[i + 1]) {
                        broken += 1;
                    }
                }
            }
        }
        (checked, broken, lost)
    }

    /// G1 on a1212942b: under rule 2 a napper rejoins right behind its predecessor, on time and a
    /// lane lap late (a +350 strand: P, N, S both ways); the ring windows' late rejoin, behind
    /// whichever cart passes 10 cycles on, is the control that must break the order. Every pick comes
    /// back.
    #[test]
    fn rule2_rejoins_behind_its_predecessor() {
        let o = LaneOpts { rule2: true, log_releases: true, days: 8, warmup: 2, ..LaneOpts::lead() };
        let (n, b, lost) = order_breaks(&o);
        assert!(n > 0, "no rejoin checked");
        assert_eq!(b, 0, "{b} of {n} on-time rejoins out of order");
        assert_eq!(lost, 0, "{lost} picks never came back");
        let late = LaneOpts { outlier: Some((24_000, 350)), ..o.clone() };
        assert!(simulate_lanes_scored(&late).1.late_nappers > 0, "the strand made no late rejoin");
        let (n2, b2, lost2) = order_breaks(&late);
        assert!(n2 > 0, "no late rejoin checked");
        assert_eq!(b2, 0, "{b2} of {n2} rejoins out of order with a lap-late napper");
        assert_eq!(lost2, 0, "{lost2} picks never came back with a lap-late napper");
        let ring = LaneOpts { rule2: false, ..late.clone() };
        let (n3, b3, _) = order_breaks(&ring);
        assert!(n3 > 0 && b3 > 0, "the control (ring windows, a late napper) kept the order: {b3} of {n3}");
    }

    /// (G1's checklist (4), ironspec's hole 8 on the tenth) the coverage bench's lists parse strictly.
    #[test]
    fn coverage_lists_parse_strictly() {
        assert_eq!(parse_exit_phase(&[]).unwrap(), None);
        assert_eq!(parse_exit_phase(&[1, 2, 3]).unwrap(), Some([1, 2, 3]));
        assert!(parse_exit_phase(&[1, 2]).is_err());
        assert_eq!(parse_exit_pickup(&[]).unwrap(), (0, 0));
        assert_eq!(parse_exit_pickup(&[4]).unwrap(), (4, 4));
        assert_eq!(parse_exit_pickup(&[0, 4]).unwrap(), (0, 4));
        assert!(parse_exit_pickup(&[4, 0]).is_err());
        assert!(parse_exit_pickup(&[1, 2, 3]).is_err());
        assert!(parse_hist_lanes(Some("100:1|200:1")).is_err());
        assert!(parse_hist_lanes(Some("||")).is_err());
        assert!(parse_hist_lanes(Some("100:1||x")).is_err());
        let ok = parse_hist_lanes(Some("100:1||200:2")).unwrap();
        assert!(ok[0].is_some() && ok[1].is_none() && ok[2].is_some());
    }

    /// (ironspec 04:07 PT) the forced skipped pick takes only an opportunity whose pick would have been made.
    #[test]
    fn the_forced_skip_reports_whether_it_took() {
        let o = LaneOpts { days: 10, ..LaneOpts::lead() };
        let took: Vec<bool> = (0..30i64).map(|slot| simulate_lanes_scored(&LaneOpts { skip_pick: Some((8, slot)), ..o.clone() }).1.skip_took).collect();
        assert!(took.iter().any(|&t| t), "no slot's skip took");
        assert!(took.iter().any(|&t| !t), "every slot's skip took (no slot without a due pick)");
    }

    /// (the chain fix) the operating point's judgement: a row with no complete night is NOT JUDGED, never a pass.
    #[test]
    fn an_empty_row_is_not_judged() {
        let mut v = crate::verdict::Verdict::new("t");
        let st = RowStats { least: 24, shortest: usize::MAX, spacing: i64::MAX, pick_gap: i64::MAX, lmargin: i64::MAX, seat_margin: i64::MAX, ..Default::default() };
        judge_operating(&st, (98, 102), "empty", &mut v);
        assert!(v.fails().iter().any(|f| f.contains("NOT JUDGED")));
        assert!(v.finish(false).is_err());
    }

    /// (day-1 10:37 PT) the free-running tap: a napper waiting at the hold a period or more leaves on an earlier pulse of
    /// its lane's train (counted); the per-visit gated tap never does.
    #[test]
    fn the_free_running_tap_launches_a_long_wait_on_an_earlier_pulse() {
        let base = LaneOpts { clock_exit: Some(5), exit_phase: Some([150, 150, 150]), days: 10, ..LaneOpts::lead() };
        let free = simulate_lanes_scored(&base).1;
        let gated = simulate_lanes_scored(&LaneOpts { gated_tap: true, ..base.clone() }).1;
        assert!(free.clock_early_lane.iter().sum::<u32>() > 0, "waits past a period launch on an earlier pulse: {:?}", free.clock_wait_lane);
        assert_eq!(gated.clock_early_lane.iter().sum::<u32>(), 0);
        let near = simulate_lanes_scored(&LaneOpts { exit_phase: Some([0, 0, 0]), ..base.clone() }).1;
        assert_eq!(near.clock_early_lane.iter().sum::<u32>(), 0, "a tap right after the latest R: every wait under a period: {:?}", near.clock_wait_lane);
    }
}
