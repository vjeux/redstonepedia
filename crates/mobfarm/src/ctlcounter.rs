//! The controller's day counter, re-phase logic (day-1, Nov 12 2026; ironspec's item 3): the units
//! ring (10 steps; u 0 identified by the unstackable token on the D8 tap, u 9 by it on a D7 tap) and
//! the tens, re-phased by trigger 1 (RISE: the daylight detector's 0 -> 1) and trigger 2 (its level
//! rising across 8, accepted only when C is in the gate). Two variants on the same rows (ironspec
//! 01:13):
//! - FF: at the next release the ring is fast-forwarded to the load's units digit at its own step
//!   rate (measured: >= 18 gt a step), the release's own step being the FF's first; the tens take the
//!   load at once; a release while the FF is busy (until its last pulse + one FF period) is held by a
//!   pending latch and stepped one FF period after the FF's last pulse;
//! - no-reset: the units free-run; at the first wrap (u 9 -> 0) after the trigger the tens take the
//!   load's tens instead of counting (the clear riding that wrap's carry, the carry suppressed).
//! A logic model at the gt level over scripted days: every release phase against the day, detector
//! update phases, every units phase at the trigger, normal days, skips and weather; the detector's
//! level is redsim's exact curve (`det_level`: tick-exact's table to the tick) with vanilla's rain and
//! thunder levels ramping 0.01 a gt toward their flags (thunder counts as thunder x rain; a skip clears
//! the flags and the levels ramp down after it). The bars: the night margin at the worst trip, the
//! first napper's arrival against clear beds, trigger 2's gate margins, the ring's pulse spacing, a
//! count error after every re-phase, false arms.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variant {
    Ff,
    NoReset,
}

/// The model's knobs.
#[derive(Clone, Debug)]
pub struct CounterOpts {
    pub variant: Variant,
    /// The FF clock's period (the ring steps correctly at >= 18 gt).
    pub ff_period: i64,
    /// The launch step (lanes A/B/C leave at REL + 0 / s / 2s): a pick's napper reaches the bed stop its
    /// lane's offset later (s 12, adopted Nov 12 06:05).
    pub launch_step: i64,
    /// The gt from a DAY change at the detector to the queue's hatch following it (the NOT-DAY gate's
    /// comparator 2 + the DAY line's delay to the queue, b8949c96's route).
    pub hatch_lag: i64,
    /// A release held while the FF is busy is stepped after it (false: absorbed).
    pub pend: bool,
    /// The window: C in [open, close).
    pub window: (i64, i64),
    /// Trigger 2 accepted when C >= gate.0 or C <= gate.1.
    pub gate: (i64, i64),
    /// C loaded by RISE and by trigger 2 (FF: units 0 or 9, the taps' two phases; no-reset: a
    /// multiple of 10, loaded at the first wrap).
    pub load1: i64,
    pub load2: i64,
    /// The detector updates every `det_period` gt (vanilla: game time % 20 == 0).
    pub det_period: i64,
    /// The first napper at the bed stop's entry this long after the window's first release (the lane
    /// model: the divert 60 gt after the release, the stop's entry 60 gt after that).
    pub to_bed: i64,
    /// Clear beds at this day time; the stop takes an arrival up to `early_tol` gt before it.
    pub bed_clear: i64,
    pub early_tol: i64,
    /// The latest lie-down this long after the window opens (the lane model's worst trip), and the
    /// night's end (the beds close) / length for the margin.
    pub last_nap: i64,
    pub night_end: i64,
    pub night_len: i64,
    /// Picks also need the daylight level < this (None: no inhibit; ironspec ruled L 6 from
    /// tick-exact's table), the level as read at the last detector update.
    /// Applied at the divert (the picked cart at the split's 4th route, `to_divert` gt after its
    /// release: the detector has updated since any skip by then), where the FIFO push also sits.
    pub day_inhibit: Option<i64>,
    pub to_divert: i64,
    /// The falling-8 arm (ironspec 01:57): a falling 8-crossing with C in `arm_band` arms trigger
    /// 2's acceptance for `arm` gt.
    pub arm: Option<i64>,
    pub arm_band: (i64, i64),
    /// The drop latch (ironspec 02:46): set by a fall of >= `drop_min` levels between two detector
    /// updates (tick-exact: the sun and every weather ramp change the level by at most 2 between
    /// readings, so 3 is a skip), cleared by trigger 2's acceptance, RISE and the window's opening.
    pub drop_min: Option<i64>,
    /// The queue's release (ironspec Nov 12 09:04 / 09:12): with Some((set, reset)) every bed-stop arrival
    /// waits at the queue cell and is released (its entry into the stop) once it has waited `stale_hold`
    /// gt AND REST_l (C >= set or C < reset: a level decoded from C like the gate, so a re-phase jumping C
    /// past an edge cannot strand it) AND the DAY the hatch sees is low. None: the plain hatch (open on
    /// DAY low).
    pub rest: Option<(i64, i64)>,
    pub stale_hold: i64,
    /// No pick while a napper is held at the queue (G1's back-pressure, ironspec 09:12).
    pub backpressure: bool,
    /// The clock's variant (a) (ironspec 09:11): every DAY rise (the level reaching L) loads C with this
    /// value at the next release, as trigger 2 does; None: (b), the gate's trigger 2 only.
    pub day_rise_load: Option<i64>,
    /// A napper is back in its lane this long after its entry (the nap and the ride back): for the
    /// riders away at a summon.
    pub back_after: i64,
    /// The stop's cycle: an entry holds the stop this long (the walk, the nap, the exit through R),
    /// and the next release waits for it (R's RS). A stale admission (an entry outside REST, or in a
    /// skip's stale window) waits on the stop's ledge until REST, naps, and leaves (ironspec 09:31): it
    /// holds the stop until REST's start + the cycle.
    pub stop_cycle: i64,
    /// The stop's walk (entry -> lying down) and nap (lying -> woken) inside the cycle (the exit is the
    /// rest): a skip during the walk leaves the napper on foot when REST ends (class b: it waits for the
    /// next REST on a no-stamp cell, holding the stop); during the nap it wakes it (class a: picked up,
    /// the stop's release on the wake edge); after it, nothing (ironspec 09:45).
    pub stop_walk: i64,
    pub stop_nap: i64,
    /// The high re-phase (ironspec 09:49): (H, the C loaded, the window lo, hi): a rise of the detector's
    /// level to H (reached only around a clear midday, so weather only delays or prevents it) while C is
    /// outside [lo, hi] (the span a natural H rise can fall in) can only follow an undetected skip; it
    /// loads C at the next release, as trigger 2 does (the load never ahead of the clear clock).
    pub high: Option<(i64, i64, i64, i64)>,
    /// The H path's block delay: a load request due at a detector update reaches the pending latch this
    /// many gt later (`ctl-high-bench`: HI, E_H and the AND), so a release sooner does not take it.
    pub high_lag: i64,
}

impl Default for CounterOpts {
    fn default() -> Self {
        CounterOpts {
            variant: Variant::Ff,
            ff_period: 18,
            launch_step: 12,
            hatch_lag: 2,
            pend: true,
            window: (141, 221),
            gate: (136, 15),
            load1: 0,
            load2: 19,
            det_period: 20,
            to_bed: 120,
            bed_clear: 12_523,
            early_tol: 60,
            last_nap: 8_200,
            night_end: 23_850,
            night_len: 11_619,
            day_inhibit: None,
            to_divert: 60,
            arm: None,
            arm_band: (0, 999),
            drop_min: None,
            rest: None,
            stale_hold: 80,
            backpressure: true,
            day_rise_load: None,
            back_after: 1_000,
            stop_cycle: 300,
            stop_walk: 80,
            stop_nap: 100,
            high: None,
            high_lag: 6,
        }
    }
}

/// A scripted event: a night skip (at the event's game time, after the 100 gt asleep: the day jumps
/// to the next 24,000 and the weather flags clear), or the weather flags set (vanilla `raining` /
/// `thundering`; the levels ramp 0.01 a gt toward them, thunder darkening only times rain).
#[derive(Clone, Copy, Debug)]
pub enum Ev {
    Skip,
    Wx { rain: bool, thunder: bool },
}

/// What one run saw.
#[derive(Clone, Debug, Default)]
pub struct RunOut {
    /// (day time, C) when the window opened (after the first re-phase).
    pub opens: Vec<(i64, i64)>,
    /// Trigger 2 events: (game time, day time, C at the event, accepted, gt since the last release).
    pub trig2: Vec<(i64, i64, i64, bool, i64)>,
    /// The gate's edges in game time: C becoming gate.1 + 1 (closing) and gate.0 (opening).
    pub gate_close_t: Vec<i64>,
    pub gate_open_t: Vec<i64>,
    /// FF runs: (start, last pulse, steps, releases held).
    pub ffs: Vec<(i64, i64, i64, i64)>,
    /// Releases held beyond the first (the latch holds one) or absorbed.
    pub pend_overflow: u32,
    pub absorbed: u32,
    /// The least spacing between two ring pulses (any kind).
    pub min_spacing: i64,
    /// After each re-phase completes: C minus (the load + the releases since the re-phase release).
    pub count_errors: Vec<i64>,
    /// Picks (window open, u a pick phase) whose napper would reach the bed before the stop takes
    /// it (a skip inside the window leaves it open into the morning until trigger 2's load).
    pub daylight_picks: u32,
    /// Picks the daylight inhibit removed at night (the window's own picks: must be 0).
    pub night_inhibited: u32,
    /// The least gap between a divert decision and the inhibit's last change (>= 3 + 1 tick).
    pub inhibit_edge_gap: i64,
    /// Good picks (not inhibited, the napper at the bed inside the night) per night, keyed by the
    /// night's dusk day number; nights a skip cut short are listed in `cut_nights`.
    pub night_picks: std::collections::BTreeMap<i64, u32>,
    pub cut_nights: std::collections::BTreeSet<i64>,
    /// Trigger 2 accepted through an arm; and those with no skip in the 340 gt before it.
    pub arm_accepts: u32,
    pub false_arms: u32,
    /// Drop-latch sets; and those with no skip since the previous detector update (a natural drop).
    pub latch_sets: u32,
    pub false_latches: u32,
    /// The DAY level's (>= L) falls to off, as day times (the coupling to the window's opening).
    pub day_off: Vec<i64>,
    /// The absolute day time at the run's end (for the complete nights).
    pub end_day: i64,
    /// The latest bed-stop arrival the queue's hatch opens for (not inhibited, and the DAY the hatch sees
    /// low when it arrives: the NOT-DAY gate), as a night day time: 12,000..23,999, or 24,000 + t past the
    /// wrap (ironspec Nov 12 06:50: bar <= 23,900, a dawn wake with the hatch open seats the napper in the
    /// stop's zombie boat). Its day time is the clock's at the arrival (a skip in transit included).
    /// Arrivals with DAY high wait at the shut hatch (held nappers) and are not counted. i64::MIN: none.
    pub latest_arrival: i64,
    /// Hatch-open arrivals at a day time before 12,000: the post-skip stale window (DAY still low until
    /// the detector's first update after the skip and the gate's lag).
    pub stale_opens: u32,
    /// Their day times: (min, max); (i64::MAX, i64::MIN) when none.
    pub stale_dt: (i64, i64),
    /// Held nappers (arrived at a shut hatch) entering when the hatch next sees DAY low: their day
    /// times (dusk, or by day when a storm drops DAY; G1 on c768c3d30: option A's rows need them).
    pub held_entries: Vec<i64>,
    /// Per skip: the gt from the skip to the first detector update reading DAY high (58f770cc's
    /// `--day-on`, before the DAY_LV comparator, the DAY out repeater and any line).
    pub skip_lags: Vec<i64>,
    /// The release model (`rest`): each entry into the stop as (the absolute day at the entry, the gt it
    /// waited at the queue, stale: outside REST or in a skip's stale window).
    pub entries: Vec<(i64, i64, bool)>,
    /// Stale admissions (ironspec 09:31: each waits on the stop's ledge until REST, naps, leaves): (the
    /// entry's day time, the day time the stop clears).
    pub stale_admits: Vec<(i64, i64)>,
    /// Picks the back-pressure deferred (a napper held at the queue).
    pub deferred: u32,
    /// The most nappers held at the queue at once; the most riders away (picked and not yet back in
    /// their lane) at a release.
    pub queue_max: u32,
    pub away_max: u32,
    /// The day times REST_l rose at.
    pub rest_rises: Vec<i64>,
    /// Naps by night: entries inside REST (12,000..24,009), keyed by the night's dusk day number.
    pub night_naps: std::collections::BTreeMap<i64, u32>,
    /// C at the first release after each DAY rise, with the rise's day time (variant (a)'s load: never
    /// ahead of the clear clock at any rise).
    pub c_at_day_rise: Vec<(i64, i64)>,
    /// Entries within 200 gt after a skip (into a skip's stale window: the hold must leave none).
    pub stale_entries: u32,
    /// Skips with a napper in the stop (ironspec 09:45): (gt since its entry, class: b'b' on foot -> waits
    /// for the next REST holding the stop, b'a' asleep -> woken and picked up, b'n' already leaving).
    pub skip_classes: Vec<(i64, u8)>,
    /// The high re-phases (ironspec 09:49) and those with no skip in the 24,000 gt before (false).
    pub h_rephases: u32,
    pub false_h: u32,
    /// C at the first release after each H rise (the high re-phase's load, from a clear run).
    pub c_at_h_rise: Vec<i64>,
    /// The least gap between two consecutive nappers' departures from the stop (R) (ironspec 10:10).
    pub exit_gap_min: i64,
    /// The least time from a napper's R to the next napper's lie-down (ironspec 10:52: the departing
    /// napper's time to leave the 16-ball).
    pub liedown_gap_min: i64,
}

/// One run: `days` days from day time `d0` at game time 0, releases at game times = `phase` mod 100,
/// detector updates at game times = `det_phase` mod `det_period`, the counter starting at C = `c0`.
pub fn run(o: &CounterOpts, days: i64, d0: i64, phase: i64, det_phase: i64, c0: i64, evs: &[(i64, Ev)]) -> RunOut {
    let mut out = RunOut { min_spacing: i64::MAX, inhibit_edge_gap: i64::MAX, latest_arrival: i64::MIN, stale_dt: (i64::MAX, i64::MIN), exit_gap_min: i64::MAX, liedown_gap_min: i64::MAX, ..Default::default() };
    let mut last_exit: Option<i64> = None;
    let horizon = days * 24_000;
    let mut day = d0;
    let (mut u, mut tens) = (c0.rem_euclid(10), c0.div_euclid(10));
    let c_of = |u: i64, tens: i64| tens * 10 + u;
    let (mut level_hi, mut eight_hi) = (true, false);
    let mut level_int = 0i64;
    // the weather (vanilla: flags, and levels ramping 0.01 a gt toward them)
    let (mut rain_on, mut thunder_on) = (false, false);
    let (mut rain_lv, mut thunder_lv) = (0f32, 0f32);
    let mut day_hi = false; // level >= the inhibit's L, as last updated
    let mut day_changed_t = -1_000i64;
    let mut diverts: Vec<(i64, i64)> = Vec::new(); // pending divert decisions (game time, the lane's launch offset)
    let mut arrivals: Vec<i64> = Vec::new(); // pending bed-stop arrivals (game times)
    let mut skip_pending: Option<i64> = None; // a skip whose DAY rise is not yet seen
    let mut held_at_hatch = 0u32; // nappers waiting at the shut hatch
    // (the release model: arrivals waiting at the queue (game times), REST_l, the riders away: picked
    // and not yet entered, then back `back_after` after the entry)
    let mut queue: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
    let mut rest_l = false;
    let mut stop_until = i64::MIN; // the stop is held until then (R's RS)
    let mut in_flight = 0u32;
    let mut back_at: Vec<i64> = Vec::new();
    // (variant (a): a DAY rise's load pending; the rise's day time waiting for its release's C)
    let mut day_pend: Option<i64> = None;
    let mut rise_dm: Option<i64> = None;
    // (the high re-phase: the level at H as last updated, a load pending; an H rise waiting for its
    // release's C)
    let mut h_hi = false;
    let mut h_pend: Option<(i64, i64)> = None; // (the tick the request reaches the pending latch, the load)
    let mut h_rise_seen = false;
    // (the stop's occupant: its entry's game time, while the stop is held)
    let mut occupant: Option<i64> = None;
    // (DAY's level changes as (update tick, level): the hatch sees an update at tick u from u + 1 (the
    // detector updates in the block-entity phase, after entities move) plus the gate's lag)
    let mut day_hist: Vec<(i64, bool)> = vec![(i64::MIN / 2, false)];
    let mut check_at: Option<i64> = None; // the count check at the end of the FF's busy span
    let mut armed_until = -1i64;
    let mut arm_latch = false;
    let mut last_skip_t = -1_000_000i64;
    let (mut rise_pend, mut trig2_pend) = (false, false);
    // FF: (started, next pulse, steps, held releases, target units)
    let mut ff: Option<(i64, i64, i64, i64, i64)> = None;
    let mut busy_until = -1i64; // the FF's last pulse + one period
    let mut pend_at: Option<i64> = None;
    let mut held = 0i64;
    // no-reset: the tens to take at the next wrap
    let mut clear_at_wrap: Option<i64> = None;
    let mut in_window = false;
    let mut rephase: Option<i64> = None; // the load being checked
    let mut releases_since = 0i64;
    let mut ev_i = 0usize;
    let mut rephased_once = false;
    let mut last_pulse = -1_000i64;
    let mut pulse = |t: i64, out: &mut RunOut| {
        out.min_spacing = out.min_spacing.min(t - last_pulse);
        last_pulse = t;
    };
    for t in 0..horizon {
        // (vanilla's tick: the weather flags change and the levels ramp (advanceWeatherCycle), then
        // the sleep check (the day jumps, the flags clear), then tickTime, then the block entities
        // (the detector reads the day time after the jump + 1 and the levels before they fall))
        let mut skip_now = false;
        while ev_i < evs.len() && evs[ev_i].0 <= t {
            match evs[ev_i].1 {
                Ev::Skip => skip_now = true,
                Ev::Wx { rain, thunder } => {
                    rain_on = rain;
                    thunder_on = thunder;
                }
            }
            ev_i += 1;
        }
        thunder_lv = (thunder_lv + if thunder_on { 0.01 } else { -0.01 }).clamp(0.0, 1.0);
        rain_lv = (rain_lv + if rain_on { 0.01 } else { -0.01 }).clamp(0.0, 1.0);
        if skip_now {
            // (a skip at night cuts that night short; keyed by its dusk's day number)
            if day.rem_euclid(24_000) >= 12_000 {
                out.cut_nights.insert(day.div_euclid(24_000));
            } else if day.rem_euclid(24_000) < 1_000 {
                out.cut_nights.insert(day.div_euclid(24_000) - 1);
            } else {
                // (a skip by day: that day's night never happens)
                out.cut_nights.insert(day.div_euclid(24_000));
            }
            last_skip_t = t;
            skip_pending = Some(t);
            // (a napper in the stop at the skip (ironspec 09:45): on foot (walking) when REST ends ->
            // waits for the next REST on a no-stamp cell, holding the stop until REST + a cycle; asleep
            // -> woken and picked up, the stop's release on the wake edge (its exit); leaving -> nothing)
            if let Some(te) = occupant
                && t < stop_until
            {
                let since = t - te;
                // (after a skip REST runs on to day time 10: the skip lands at day time 1, so a napper whose
                // lie-down falls within 9 gt still lies down and naps: 58f770cc 10:07)
                let rest_tail = 9;
                let class = if since + rest_tail < o.stop_walk {
                    stop_until = t + 12_000 + o.stop_cycle;
                    b'b'
                } else if since < o.stop_walk + o.stop_nap {
                    stop_until = t + (o.stop_cycle - o.stop_walk - o.stop_nap).max(0);
                    b'a'
                } else {
                    b'n'
                };
                out.skip_classes.push((since, class));
                // (the occupant's exit moved)
                last_exit = Some(stop_until);
            }
            day = (day.div_euclid(24_000) + 1) * 24_000 + 1;
            // (vanilla resets the weather on waking only if isRaining(): the rain level > 0.2)
            if rain_lv > 0.2 {
                rain_on = false;
                thunder_on = false;
            }
        }
        let dm = day.rem_euclid(24_000);
        // the detector
        if (t - det_phase).rem_euclid(o.det_period) == 0 {
            let prev_level = level_int;
            level_int = det_level(dm, rain_lv, thunder_lv * rain_lv);
            if let Some(k) = o.drop_min
                && prev_level - level_int >= k
            {
                arm_latch = true;
                out.latch_sets += 1;
                if t - last_skip_t >= o.det_period {
                    out.false_latches += 1;
                }
            }
            if let Some(l) = o.day_inhibit {
                let hi_now = level_int >= l;
                // (58f770cc: the gt from a skip to the first detector update reading DAY high)
                if hi_now && let Some(ts) = skip_pending.take() {
                    out.skip_lags.push(t - ts);
                }
                if hi_now != day_hi {
                    day_hi = hi_now;
                    day_changed_t = t;
                    day_hist.push((t, hi_now));
                    if !hi_now {
                        out.day_off.push(dm);
                    } else {
                        // (a DAY rise: variant (a) loads C at the next release; the rise's day time is
                        // kept for the C that release shows)
                        rise_dm = Some(dm);
                        if let Some(v) = o.day_rise_load {
                            day_pend = Some(v);
                        }
                    }
                }
            }
            let hi = level_int >= 1;
            let eight = level_int >= 8;
            // (the high re-phase: a rise to H with C outside the natural window)
            if let Some((h, load, lo, hi_c)) = o.high {
                let h_now = level_int >= h;
                if h_now && !h_hi {
                    h_rise_seen = true;
                    let c = c_of(u, tens);
                    if c < lo || c > hi_c {
                        h_pend = Some((t + o.high_lag, load));
                        out.h_rephases += 1;
                        if t - last_skip_t > 24_000 {
                            out.false_h += 1;
                        }
                    }
                }
                h_hi = h_now;
            }
            if hi && !level_hi {
                rise_pend = true;
                arm_latch = false;
            }
            if eight && !eight_hi {
                let c = c_of(u, tens);
                let by_gate = c >= o.gate.0 || c <= o.gate.1;
                let by_arm = (o.arm.is_some() && t < armed_until) || (o.drop_min.is_some() && arm_latch);
                let acc = by_gate || by_arm;
                if by_arm && !by_gate {
                    out.arm_accepts += 1;
                    if t - last_skip_t > 340 {
                        out.false_arms += 1;
                    }
                }
                // (`since`: gt since the release that set C; a release in this same tick has not
                // stepped yet when the detector updates, so 0 means a full period)
                let since = match (t - phase).rem_euclid(100) {
                    0 => 100,
                    s => s,
                };
                out.trig2.push((t, dm, c, acc, since));
                if acc {
                    trig2_pend = true;
                    rise_pend = false;
                    armed_until = -1;
                    arm_latch = false;
                }
            }
            // (the arm: a falling 8-crossing with C in the band)
            if let Some(span) = o.arm
                && !eight
                && eight_hi
            {
                let c = c_of(u, tens);
                if c >= o.arm_band.0 && c <= o.arm_band.1 {
                    armed_until = t + span;
                }
            }
            level_hi = hi;
            eight_hi = eight;
        }
        // the FF's pulses
        if let Some((st, next, steps, hd, target)) = ff
            && t == next
        {
            pulse(t, &mut out);
            u = (u + 1) % 10;
            let steps = steps + 1;
            if u == target {
                out.ffs.push((st, t, steps, hd));
                ff = None;
                busy_until = t + o.ff_period;
                if held > 0 {
                    pend_at = Some(t + o.ff_period);
                } else {
                    // (G1 on 100874aa8: a release may still be held in the trailing busy span, so
                    // the count is checked at its end (or at the pending step))
                    check_at = Some(busy_until);
                }
            } else {
                ff = Some((st, t + o.ff_period, steps, hd, target));
            }
        }
        // the pending step
        if pend_at == Some(t) {
            pulse(t, &mut out);
            u = (u + 1) % 10;
            if u == 0 {
                tens += 1;
            }
            held = 0;
            pend_at = None;
            check_at = None;
            if let Some(load) = rephase.take() {
                out.count_errors.push(c_of(u, tens) - (load + releases_since));
            }
        }
        // the count check at the end of the FF's busy span (nothing held)
        if check_at == Some(t) {
            check_at = None;
            if pend_at.is_none()
                && let Some(load) = rephase.take()
            {
                out.count_errors.push(c_of(u, tens) - (load + releases_since));
            }
        }
        // the divert decisions due now (the inhibit read at the split; the napper at the bed's
        // entry `to_bed - to_divert` later)
        if diverts.first().map(|d| d.0) == Some(t) {
            let (_, lane_off) = diverts.remove(0);
            let at_bed = dm + (o.to_bed - o.to_divert);
            let early = (at_bed.rem_euclid(24_000)) < o.bed_clear - o.early_tol;
            let inhibited = o.day_inhibit.is_some() && day_hi;
            if inhibited && o.rest.is_some() {
                // (not diverted: the napper rides on in its cart, back in its lane)
                in_flight = in_flight.saturating_sub(1);
            }
            if o.day_inhibit.is_some() {
                out.inhibit_edge_gap = out.inhibit_edge_gap.min(t - day_changed_t);
            }
            if inhibited && !early {
                out.night_inhibited += 1;
            }
            if early && !inhibited {
                out.daylight_picks += 1;
            }
            // (the night's naps: the napper at the bed inside the night, not inhibited)
            let arrive_abs = day + (o.to_bed - o.to_divert);
            let dm_a = arrive_abs.rem_euclid(24_000);
            if !inhibited && dm_a >= o.bed_clear - o.early_tol && dm_a < o.night_end {
                *out.night_picks.entry(arrive_abs.div_euclid(24_000)).or_default() += 1;
            }
            // (the bed-stop arrival, its lane's launch offset added: judged at its own tick, below)
            if !inhibited {
                arrivals.push(t + (o.to_bed - o.to_divert) + lane_off);
            }
        }
        // the arrivals due now: the hatch opens for the napper if the DAY it sees is low (or there is no
        // inhibit): the level as of an update at least `hatch_lag` + 1 gt ago (the gate's comparator and
        // its line; an update in this tick lands after entities move). Its day time is the running clock's
        // now, a skip since the decision included (G1 on 5a1f0cd74): a night day time, past the wrap as
        // 24,000 + t. A hatch-open arrival before 12,000 is one in the post-skip stale window (DAY still
        // low before the detector's first update after the skip, or before the gate passes it).
        let hatch_sees_hi = day_hist.iter().rev().find(|(u, _)| *u + 1 + o.hatch_lag <= t).is_some_and(|(_, hi)| *hi);
        let dm_now = day.rem_euclid(24_000);
        // REST_l: a level decoded from C (C >= set or C < reset), the clock's REST
        if let Some((rs, rr)) = o.rest {
            let c_now = c_of(u, tens);
            let now = c_now >= rs || c_now < rr;
            if now && !rest_l {
                out.rest_rises.push(dm_now);
            }
            rest_l = now;
        }
        if o.rest.is_some() {
            // (the release model: every arrival waits at the queue; the head is released once it has
            // waited `stale_hold` AND REST_l AND the DAY the hatch sees is low)
            while let Some(i) = arrivals.iter().position(|&a| a == t) {
                arrivals.remove(i);
                queue.push_back(t);
                out.queue_max = out.queue_max.max(queue.len() as u32);
            }
            if let Some(&ta) = queue.front()
                && t >= ta + o.stale_hold
                && rest_l
                && !hatch_sees_hi
                && t >= stop_until
            {
                queue.pop_front();
                // (stale: outside REST, or right after a skip with the day time before 12,000)
                let stale = (10..12_000).contains(&dm_now) || (t - last_skip_t < 200 && dm_now < 12_000);
                out.entries.push((day, t - ta, stale));
                if t - last_skip_t < 200 {
                    out.stale_entries += 1;
                }
                in_flight = in_flight.saturating_sub(1);
                occupant = Some(t);
                if stale {
                    // (it waits on the stop's ledge until REST, naps, leaves: the stop is held until then;
                    // its nap is that evening's)
                    let wait = (12_000 - dm_now).max(0);
                    stop_until = t + wait + o.stop_cycle;
                    out.stale_admits.push((dm_now, (dm_now + wait + o.stop_cycle).rem_euclid(24_000)));
                    back_at.push(stop_until + o.back_after);
                    *out.night_naps.entry(day.div_euclid(24_000)).or_default() += 1;
                } else {
                    stop_until = t + o.stop_cycle;
                    back_at.push(t + o.back_after);
                    // (a nap: an entry inside REST, 12,000..24,009, keyed by its dusk's day number)
                    if dm_now >= 12_000 {
                        *out.night_naps.entry(day.div_euclid(24_000)).or_default() += 1;
                    } else if dm_now < 10 {
                        *out.night_naps.entry(day.div_euclid(24_000) - 1).or_default() += 1;
                    }
                }
                // (consecutive departures from the stop: this napper's exit against the previous one's;
                // ironspec 10:52, HOLD dropped: this napper's lie-down against the previous one's R, the
                // time the departing napper has to leave the 16-ball; a stale napper lies down at REST)
                let liedown = if stale { t + (12_000 - dm_now).max(0) } else { t + o.stop_walk };
                if let Some(le) = last_exit {
                    out.exit_gap_min = out.exit_gap_min.min(stop_until - le);
                    out.liedown_gap_min = out.liedown_gap_min.min(liedown - le);
                }
                last_exit = Some(stop_until);
            }
        }
        while o.rest.is_none()
            && let Some(i) = arrivals.iter().position(|&a| a == t)
        {
            arrivals.remove(i);
            if o.day_inhibit.is_none() || !hatch_sees_hi {
                let night_dt = if dm_now >= 12_000 { dm_now } else { dm_now + 24_000 };
                out.latest_arrival = out.latest_arrival.max(night_dt);
                if dm_now < 12_000 {
                    out.stale_opens += 1;
                    out.stale_dt = (out.stale_dt.0.min(dm_now), out.stale_dt.1.max(dm_now));
                }
            } else {
                held_at_hatch += 1;
            }
        }
        // (held nappers enter when the hatch next sees DAY low: at dusk, or by day when a storm drops
        // DAY below L)
        if held_at_hatch > 0 && o.day_inhibit.is_some() && !hatch_sees_hi {
            for _ in 0..held_at_hatch {
                out.held_entries.push(dm_now);
            }
            held_at_hatch = 0;
        }
        // the release
        if (t - phase).rem_euclid(100) == 0 {
            let busy = ff.is_some() || t < busy_until || pend_at.is_some();
            if busy && o.variant == Variant::Ff {
                if !o.pend {
                    out.absorbed += 1;
                } else if held == 0 {
                    held = 1;
                    if ff.is_none() && pend_at.is_none() {
                        // (the FF already stopped: its next slot steps it)
                        pend_at = Some(busy_until);
                    }
                } else {
                    out.pend_overflow += 1;
                }
                releases_since += 1;
                if let Some((st, next, steps, hd, target)) = ff {
                    ff = Some((st, next, steps, hd + 1, target));
                }
            } else if (trig2_pend || rise_pend || day_pend.is_some() || h_pend.is_some_and(|(r, _)| t >= r)) && o.variant == Variant::Ff {
                let load = if let Some(v) = day_pend.take() {
                    v
                } else if let Some((r, v)) = h_pend
                    && t >= r
                {
                    h_pend = None;
                    v
                } else if trig2_pend {
                    o.load2
                } else {
                    o.load1
                };
                trig2_pend = false;
                rise_pend = false;
                rephased_once = true;
                releases_since = 0;
                tens = load.div_euclid(10);
                let target = load.rem_euclid(10);
                if u == target {
                    out.ffs.push((t, t, 0, 0));
                    out.count_errors.push(c_of(u, tens) - load);
                } else {
                    // the release's own step is the FF's first pulse
                    rephase = Some(load);
                    pulse(t, &mut out);
                    u = (u + 1) % 10;
                    if u == target {
                        out.ffs.push((t, t, 1, 0));
                        busy_until = t + o.ff_period;
                        out.count_errors.push(c_of(u, tens) - load);
                        rephase = None;
                    } else {
                        ff = Some((t, t + o.ff_period, 1, 0, target));
                    }
                }
            } else {
                if (trig2_pend || rise_pend) && o.variant == Variant::NoReset {
                    let load = if trig2_pend { o.load2 } else { o.load1 };
                    trig2_pend = false;
                    rise_pend = false;
                    clear_at_wrap = Some(load);
                }
                pulse(t, &mut out);
                u = (u + 1) % 10;
                releases_since += 1;
                if u == 0 {
                    if let Some(load) = clear_at_wrap.take() {
                        tens = load.div_euclid(10);
                        rephased_once = true;
                        releases_since = 0;
                        out.count_errors.push(c_of(u, tens) - load);
                    } else {
                        tens += 1;
                    }
                }
            }
            let c = c_of(u, tens);
            // (the C this release shows after a DAY rise: variant (a)'s load, the clear clock's value)
            if let Some(rd) = rise_dm.take() {
                out.c_at_day_rise.push((rd, c));
            }
            if h_rise_seen {
                h_rise_seen = false;
                out.c_at_h_rise.push(c);
            }
            if o.rest.is_some() {
                back_at.retain(|&b| b > t);
                out.away_max = out.away_max.max(in_flight + back_at.len() as u32);
            }
            if c == o.gate.1 + 1 {
                out.gate_close_t.push(t);
            }
            if c == o.gate.0 {
                out.gate_open_t.push(t);
            }
            let w = ff.is_none() && c >= o.window.0 && c < o.window.1;
            if w && !in_window && rephased_once {
                out.opens.push((dm, c));
            }
            if w && !in_window {
                arm_latch = false;
            }
            in_window = w;
            // (a pick: the window open at a pick phase (the window's first release and every 3rd:
            // lanes A/B/C at offsets 0/3/6 of the 10-cycle); decided at its divert)
            if w && rephased_once && let Some(lane) = [0i64, 3, 6].iter().position(|&x| x == (c - o.window.0).rem_euclid(10)) {
                if o.rest.is_some() && o.backpressure && !queue.is_empty() {
                    // (G1's back-pressure: no pick while a napper is held at the queue)
                    out.deferred += 1;
                } else {
                    diverts.push((t + o.to_divert, lane as i64 * o.launch_step));
                    if o.rest.is_some() {
                        in_flight += 1;
                    }
                }
            }
        }
        day += 1;
    }
    out.end_day = day;
    out
}

/// One scenario's results, over every release phase x detector phase x starting units phase.
#[derive(Clone, Debug, Default)]
pub struct ScenOut {
    pub runs: usize,
    pub open_min: i64,
    pub open_max: i64,
    pub hi: (usize, i64),
    pub lo: (usize, i64),
    pub rej: (usize, i64),
    pub ff_max: i64,
    pub ff_steps_max: i64,
    pub held_max: i64,
    pub ovf: u32,
    pub absorbed: u32,
    pub min_spacing: i64,
    pub cerr_bad: usize,
    pub cerr_n: usize,
    pub daylight: u32,
    pub night_inh: u32,
    pub edge_gap: i64,
    /// The fewest good naps over the complete nights no skip cut (G1 on 100874aa8: 24 a night).
    pub min_night: u32,
    pub false_arms: u32,
    pub arm_accepts: u32,
    pub latch_sets: u32,
    pub false_latches: u32,
    /// The least gap from the DAY level's last fall (L) to the window's opening, day times (ironspec:
    /// >= 20 gt before the first scheduled napper can reach the queue).
    pub day_off_margin: i64,
    /// The latest bed-stop arrival over the runs (RunOut::latest_arrival; i64::MIN: none).
    pub latest_arrival: i64,
    /// Hatch-open arrivals in the post-skip stale window, over the runs.
    pub stale_opens: u32,
    /// The skip -> DAY-high lags over the runs: (min, max); (i64::MAX, i64::MIN) when no skip.
    pub skip_lag: (i64, i64),
    /// G1 on c768c3d30: the stale-window opens' day times (min, max); the held nappers' entries (count,
    /// day times min / max, those by day: before 12,000).
    pub stale_dt: (i64, i64),
    pub held_n: u32,
    pub held_dt: (i64, i64),
    pub held_by_day: u32,
    /// The release model (`rest`): the earliest and latest entry as night day times (past midnight
    /// 24,000 + t; an entry at 10..11,999 is itself: before REST), the entries outside REST, the wait at
    /// the queue (min, max, sum, count), deferred picks, the most held at once, the most riders away at
    /// a release, REST_l's evening rises (min, max day time) and its transient rises by day (during an
    /// FF re-phase, DAY high), the fewest naps over the complete nights no skip cut, entries in a skip's
    /// stale window.
    pub entry_min: i64,
    pub entry_max: i64,
    pub outside_rest: u32,
    pub wait: (i64, i64, i64, u64),
    pub deferred: u32,
    pub queue_max: u32,
    pub away_max: u32,
    pub rest_rise: (i64, i64),
    pub rest_rise_day: u32,
    pub min_naps: u32,
    pub stale_entries: u32,
    /// C at the release after each DAY rise (min, max) and the rises' day times (min, max).
    pub c_rise: (i64, i64),
    pub rise_dt: (i64, i64),
    /// The release model's naps per night key (min, max over the runs), complete or not, cut or not.
    pub naps_by_key: std::collections::BTreeMap<i64, (u32, u32)>,
    /// The longest wait at the queue: (wait, arrival day time, entry day time).
    pub wait_max_at: (i64, i64, i64),
    /// Stale admissions (ironspec 09:31): the count, their entry day times and the stop-clear day times.
    pub stale_admit_n: u32,
    pub stale_admit_dt: (i64, i64),
    pub stale_clear_dt: (i64, i64),
    /// The high re-phases and the false ones (no skip in the 24,000 gt before); C at the release after
    /// an H rise (min, max).
    pub h_rephases: u32,
    pub false_h: u32,
    pub c_h: (i64, i64),
    /// Skips with a napper in the stop, by class (a asleep, b on foot, n leaving).
    pub class_a: u32,
    pub class_b: u32,
    pub class_n: u32,
    /// The least gap between consecutive departures from the stop (ironspec 10:10).
    pub exit_gap_min: i64,
    /// The least time from a napper's R to the next napper's lie-down (ironspec 10:52).
    pub liedown_gap_min: i64,
}

/// A scenario over release phases (every `phase_step`), the given detector phases and every units
/// phase at the start.
pub fn scenario(o: &CounterOpts, evs: &[(i64, Ev)]) -> ScenOut {
    scenario_with(o, evs, 1, &[0, 7, 13, 19])
}

pub fn scenario_with(o: &CounterOpts, evs: &[(i64, Ev)], phase_step: usize, det_phases: &[i64]) -> ScenOut {
    scenario_days(o, evs, phase_step, det_phases, 3)
}

/// `scenario_with` over `days` days (the storm-skip rows need the night after the post-skip night).
pub fn scenario_days(o: &CounterOpts, evs: &[(i64, Ev)], phase_step: usize, det_phases: &[i64], days: i64) -> ScenOut {
    let mut s = ScenOut {
        open_min: i64::MAX,
        open_max: i64::MIN,
        hi: (0, i64::MAX),
        lo: (0, i64::MAX),
        rej: (0, i64::MAX),
        min_spacing: i64::MAX,
        edge_gap: i64::MAX,
        min_night: u32::MAX,
        day_off_margin: i64::MAX,
        latest_arrival: i64::MIN,
        skip_lag: (i64::MAX, i64::MIN),
        stale_dt: (i64::MAX, i64::MIN),
        held_dt: (i64::MAX, i64::MIN),
        entry_min: i64::MAX,
        entry_max: i64::MIN,
        wait: (i64::MAX, i64::MIN, 0, 0),
        rest_rise: (i64::MAX, i64::MIN),
        min_naps: u32::MAX,
        c_rise: (i64::MAX, i64::MIN),
        rise_dt: (i64::MAX, i64::MIN),
        stale_admit_dt: (i64::MAX, i64::MIN),
        stale_clear_dt: (i64::MAX, i64::MIN),
        c_h: (i64::MAX, i64::MIN),
        exit_gap_min: i64::MAX,
        liedown_gap_min: i64::MAX,
        ..Default::default()
    };
    for phase in (0..100).step_by(phase_step) {
        for &det_phase in det_phases {
            for c0 in 100i64..110 {
                s.runs += 1;
                let r = run(o, days, 20_000, phase, det_phase, c0, evs);
                s.latest_arrival = s.latest_arrival.max(r.latest_arrival);
                s.stale_opens += r.stale_opens;
                s.stale_dt = (s.stale_dt.0.min(r.stale_dt.0), s.stale_dt.1.max(r.stale_dt.1));
                for &dm in &r.held_entries {
                    s.held_n += 1;
                    s.held_dt = (s.held_dt.0.min(dm), s.held_dt.1.max(dm));
                    if dm < 12_000 {
                        s.held_by_day += 1;
                    }
                }
                for &(d, w, stale) in &r.entries {
                    let dm = d.rem_euclid(24_000);
                    s.wait = (s.wait.0.min(w), s.wait.1.max(w), s.wait.2 + w, s.wait.3 + 1);
                    if w > s.wait_max_at.0 {
                        s.wait_max_at = (w, (d - w).rem_euclid(24_000), dm);
                    }
                    if stale {
                        continue;
                    }
                    let n = if dm < 10 { dm + 24_000 } else { dm };
                    s.entry_min = s.entry_min.min(n);
                    s.entry_max = s.entry_max.max(n);
                    if (10..12_000).contains(&dm) {
                        s.outside_rest += 1;
                    }
                }
                for &(e, c) in &r.stale_admits {
                    s.stale_admit_n += 1;
                    s.stale_admit_dt = (s.stale_admit_dt.0.min(e), s.stale_admit_dt.1.max(e));
                    s.stale_clear_dt = (s.stale_clear_dt.0.min(c), s.stale_clear_dt.1.max(c));
                }
                s.h_rephases += r.h_rephases;
                s.false_h += r.false_h;
                s.exit_gap_min = s.exit_gap_min.min(r.exit_gap_min);
                s.liedown_gap_min = s.liedown_gap_min.min(r.liedown_gap_min);
                for &c in &r.c_at_h_rise {
                    s.c_h = (s.c_h.0.min(c), s.c_h.1.max(c));
                }
                for &(_since, class) in &r.skip_classes {
                    match class {
                        b'a' => s.class_a += 1,
                        b'b' => s.class_b += 1,
                        _ => s.class_n += 1,
                    }
                }
                if o.rest.is_some() {
                    let last_key = (r.end_day - 12_000).div_euclid(24_000);
                    for k in 1..=last_key {
                        let n = r.night_naps.get(&k).copied().unwrap_or(0);
                        let e = s.naps_by_key.entry(k).or_insert((u32::MAX, 0));
                        *e = (e.0.min(n), e.1.max(n));
                    }
                }
                s.deferred += r.deferred;
                s.queue_max = s.queue_max.max(r.queue_max);
                s.away_max = s.away_max.max(r.away_max);
                s.stale_entries += r.stale_entries;
                for &dm in &r.rest_rises {
                    if (11_000..14_000).contains(&dm) {
                        s.rest_rise = (s.rest_rise.0.min(dm), s.rest_rise.1.max(dm));
                    } else {
                        s.rest_rise_day += 1;
                    }
                }
                for &(rd, c) in &r.c_at_day_rise {
                    s.c_rise = (s.c_rise.0.min(c), s.c_rise.1.max(c));
                    s.rise_dt = (s.rise_dt.0.min(rd), s.rise_dt.1.max(rd));
                }
                for &lag in &r.skip_lags {
                    s.skip_lag = (s.skip_lag.0.min(lag), s.skip_lag.1.max(lag));
                }
                s.edge_gap = s.edge_gap.min(r.inhibit_edge_gap);
                s.false_arms += r.false_arms;
                s.arm_accepts += r.arm_accepts;
                s.latch_sets += r.latch_sets;
                s.false_latches += r.false_latches;
                for (dm_open, _c) in &r.opens {
                    if let Some(off) = r.day_off.iter().filter(|&&d| d >= 9_000 && d <= *dm_open).max() {
                        s.day_off_margin = s.day_off_margin.min(dm_open - off);
                    }
                }
                // (complete nights from key 1 (the run starts inside key 0's night) whose end the run
                // reached, none cut by a skip)
                let last_complete = (r.end_day - 23_850).div_euclid(24_000);
                for k in 1..=last_complete {
                    if !r.cut_nights.contains(&k) {
                        s.min_night = s.min_night.min(r.night_picks.get(&k).copied().unwrap_or(0));
                        if o.rest.is_some() {
                            s.min_naps = s.min_naps.min(r.night_naps.get(&k).copied().unwrap_or(0));
                        }
                    }
                }
                for (st, last, steps, held) in &r.ffs {
                    s.ff_max = s.ff_max.max(last - st);
                    s.ff_steps_max = s.ff_steps_max.max(*steps);
                    s.held_max = s.held_max.max(*held);
                }
                s.ovf += r.pend_overflow;
                s.absorbed += r.absorbed;
                s.min_spacing = s.min_spacing.min(r.min_spacing);
                for e in &r.count_errors {
                    s.cerr_n += 1;
                    if *e != 0 {
                        s.cerr_bad += 1;
                    }
                }
                for (dm, _c) in r.opens.iter() {
                    s.open_min = s.open_min.min(*dm);
                    s.open_max = s.open_max.max(*dm);
                }
                s.daylight += r.daylight_picks;
                s.night_inh += r.night_inhibited;
                // (the gate margins from C itself: a load can jump C past an edge, so the edges' own
                // times would mislead; `since` = gt since the release that set C)
                for (_tt, _dm, c, _acc, since) in &r.trig2 {
                    if *c <= o.gate.1 {
                        let m = (o.gate.1 + 1 - c) * 100 - since;
                        s.lo = (s.lo.0 + 1, s.lo.1.min(m));
                    } else if *c >= o.gate.0 {
                        let m = (c - o.gate.0) * 100 + since;
                        s.hi = (s.hi.0 + 1, s.hi.1.min(m));
                    } else {
                        let since_close = (c - (o.gate.1 + 1)) * 100 + since;
                        let to_open = (o.gate.0 - c) * 100 - since;
                        s.rej = (s.rej.0 + 1, s.rej.1.min(since_close.min(to_open)));
                    }
                }
            }
        }
    }
    s
}

/// The siding windows (ironspec 01:13, option 1): each lane's 100-gt window [R(c) + a, R(c+1) + a),
/// its edges taken from the release chain's taps (a in 0..=a_max, even: 2 gt a stage), placed on
/// the predecessor's pass at R(pick + 7) + T_pb. (The predecessor is released at pick - 1; its lap
/// still holds the napper's own release at the pick, then the lane's 6 others, so it comes round at
/// pick + 7, then every 7 with the napper away. v4/v5 had pick + 6: one cycle early, the napper
/// rejoining behind the cart before its predecessor.) Picks at u = window_open + 0/3/6. Prints each
/// lane's window phase u, tap a, the margins to the window's edges, and the ring's token set (u 0 the
/// unstackable: the reset).
pub fn sidings(window_open: i64, t_pb: [i64; 3], a_max: i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut tokens = std::collections::BTreeSet::new();
    tokens.insert(0i64);
    let mut worst = i64::MAX;
    let names = ["A (x -1)", "B (x 3)", "C (x 7)"];
    let mut parts = Vec::new();
    for (l, off) in [0i64, 3, 6].iter().enumerate() {
        let pick_u = (window_open + off).rem_euclid(10);
        tokens.insert(pick_u);
        // the pass, in release units after the pick: 7 + T_pb / 100
        let base = 7 + t_pb[l].div_euclid(100);
        let q = t_pb[l].rem_euclid(100);
        // candidates: the window in cycle `base` (pass at +q) or `base - 1` (pass at +q + 100)
        let mut best: Option<(i64, i64, i64, i64)> = None; // (min margin, c_rel, a, before)
        for a in (0..=a_max).step_by(2) {
            for (c_rel, at) in [(base, q), (base - 1, q + 100)] {
                let before = at - a;
                let after = a + 100 - at;
                if before <= 0 || after <= 0 {
                    continue;
                }
                let m = before.min(after);
                if best.is_none_or(|b| m > b.0) {
                    best = Some((m, c_rel, a, before));
                }
            }
        }
        let Some((m, c_rel, a, before)) = best else {
            parts.push(format!("{}: no window", names[l]));
            continue;
        };
        let win_u = (pick_u + c_rel).rem_euclid(10);
        tokens.insert(win_u);
        worst = worst.min(m);
        parts.push(format!(
            "{} pick u {pick_u}, T_pb {}: window u {win_u} (the {}th release after the pick) + tap {a} gt (stage {}), the pass {before} gt after the window opens and {} before it closes",
            names[l], t_pb[l], c_rel, a / 2, 100 - before
        ));
    }
    let toks: Vec<String> = tokens.iter().map(|t| t.to_string()).collect();
    lines.push(format!(
        "sidings (window C {window_open}.., picks u {}/{}/{}; chain taps 0..{a_max} gt): {}; the ring's tokens at u {{{}}} ({} tokens, u 0 the unstackable; the event counter {} events a period, {} bulbs); the least margin {worst} gt",
        window_open.rem_euclid(10), (window_open + 3).rem_euclid(10), (window_open + 6).rem_euclid(10),
        parts.join("; "), toks.join(", "), tokens.len(), tokens.len(),
        if tokens.len() <= 4 { 2 } else if tokens.len() <= 8 { 3 } else { 4 }
    ));
    lines
}

/// One skip at day time `d` (on the run's second calendar day): the naps (min, max over release
/// phases x detector phases) that night and the night after, the daylight picks, and the detector
/// phases (the first reading's tick after the skip, 0..19) at which every release phase gave that
/// night all 24 naps (a bit mask). "That night" is the night after the jump for a skip by day (d <
/// 12,000: a thunderstorm, the only weather a day skip happens in, on from d - 2,000), the cut night
/// for a skip at night (`rain_at_night`: rain from 11,000, the rain-dusk bed time; else clear).
pub fn skip_naps(o: &CounterOpts, d: i64, rain_at_night: bool) -> ((u32, u32), (u32, u32), u32, u32) {
    // (the run starts at day time 20,000 (key 0); key 1's day time d is game time 4,000 + d)
    let t_skip = 4_000 + d;
    let mut evs = Vec::new();
    if d < 12_000 {
        evs.push((t_skip - 2_000, Ev::Wx { rain: true, thunder: true }));
    } else if rain_at_night {
        evs.push((4_000 + 11_000, Ev::Wx { rain: true, thunder: false }));
    }
    evs.push((t_skip, Ev::Skip));
    let that_key = if d < 12_000 { 2 } else { 1 };
    let (mut that, mut after) = ((u32::MAX, 0u32), (u32::MAX, 0u32));
    let mut daylight = 0u32;
    let mut covered = 0u32;
    for k in 0..o.det_period {
        // (the detector phase whose first reading after the skip lands k gt after it)
        let det_phase = (t_skip + k).rem_euclid(o.det_period);
        let mut all = true;
        for phase in (0..100).step_by(20) {
            let r = run(o, 4, 20_000, phase, det_phase, 100, evs.as_slice());
            let a = r.night_picks.get(&that_key).copied().unwrap_or(0);
            let b = r.night_picks.get(&(that_key + 1)).copied().unwrap_or(0);
            that = (that.0.min(a), that.1.max(a));
            after = (after.0.min(b), after.1.max(b));
            daylight += r.daylight_picks;
            all &= a >= 24;
        }
        if all {
            covered |= 1 << k;
        }
    }
    (that, after, daylight, covered)
}

/// The detector's reading at every tick for `len` gt after a thunderstorm skip at day time `d`
/// (the levels at 1.0 before it, falling 0.01 a gt after it): (gt after the skip, level) changes.
pub fn post_skip_levels(d: i64, len: i64) -> Vec<(i64, i64)> {
    let _ = d;
    let mut v = Vec::new();
    let mut last = -1;
    for k in 0..len {
        // (the skip tick reads day 1 with the levels still at 1.0; each tick after, both fall 0.01)
        let lv = (1.0f32 - 0.01 * k as f32).max(0.0);
        let l = det_level(k + 1, lv, lv * lv);
        if l != last {
            v.push((k, l));
            last = l;
        }
    }
    v
}

/// The daylight detector's power under the open sky at day time `day` with these weather levels
/// (`getRainLevel(1)`, `getThunderLevel(1)` = thunder x rain): redsim's
/// `DaylightDetectorBlock.updateSignalStrength` (sky light 15 - getSkyDarken, times the cosine of
/// the sun's angle eased 20% toward 0 / 2 pi, rounded).
pub fn det_level(day: i64, rain: f32, thunder: f32) -> i64 {
    // (G1 on a1212942b: the day time wraps, as tick-exact's level() does)
    let day = day.rem_euclid(24_000);
    let target = (15 - redsim::weather::sky_darken_at(day, rain, thunder)).max(0);
    if target <= 0 {
        return 0;
    }
    let mut sun = redsim::daylight::sun_angle(day) * (std::f64::consts::PI / 180.0) as f32;
    let offset = if sun < std::f64::consts::PI as f32 { 0.0f32 } else { (std::f64::consts::PI * 2.0) as f32 };
    sun += (offset - sun) * 0.2f32;
    let v = target as f32 * redsim::entity::mth::cos(sun as f64);
    ((v as f64 + 0.5).floor() as i64).clamp(0, 15)
}

/// The detector's curve per weather: every change of level over a day (day time -> level), and the
/// levels where the controller's thresholds sit.
pub fn daylight_table() -> Vec<String> {
    let mut lines = Vec::new();
    for (name, rain, thunder) in [("clear", 0.0f32, 0.0f32), ("rain", 1.0, 0.0), ("thunder", 1.0, 1.0)] {
        let mut changes = Vec::new();
        let mut last = det_level(0, rain, thunder);
        changes.push(format!("0:{last}"));
        for day in 1..24_000 {
            let l = det_level(day, rain, thunder);
            if l != last {
                changes.push(format!("{day}:{l}"));
                last = l;
            }
        }
        let range = |a: i64, b: i64| {
            let v: Vec<i64> = (a..=b).map(|d| det_level(d, rain, thunder)).collect();
            (*v.iter().min().unwrap(), *v.iter().max().unwrap())
        };
        let (w_lo, w_hi) = range(12_370, 12_550);
        let (m_lo, m_hi) = range(0, 290);
        let (n_lo, n_hi) = range(3_000, 9_000);
        lines.push(format!(
            "{name}: changes (day:level) {}; at the window's opening (12,370..12,550) {w_lo}..{w_hi}; the morning after a skip (0..290) {m_lo}..{m_hi}; mid-day (3,000..9,000) {n_lo}..{n_hi}",
            changes.join(" ")
        ));
    }
    // the largest one-update (20 gt) drop of the level while the weather ramps (rain and thunder
    // levels move 0.01 a gt; thunder counts as thunder x rain), at every day time
    let mut worst = (0i64, 0i64, "");
    for day in (0..24_000).step_by(20) {
        for (kind, r0, t0, dr, dt) in [("rain onset", 0.0f32, 0.0f32, 0.2f32, 0.0f32), ("thunder onset with rain", 1.0, 0.0, 0.0, 0.2), ("rain + thunder onset together", 0.0, 0.0, 0.2, 0.2)] {
            let mut r = r0;
            let mut th = t0;
            for _ in 0..6 {
                let a = det_level(day, r, th * r);
                let r2 = (r + dr).min(1.0);
                let t2 = (th + dt).min(1.0);
                let b = det_level(day + 20, r2, t2 * r2);
                if a - b > worst.0 {
                    worst = (a - b, day, kind);
                }
                r = r2;
                th = t2;
            }
        }
    }
    lines.push(format!("the largest drop in one detector update from weather ramping (0.01/gt, thunder x rain): {} levels (day {}, {})", worst.0, worst.1, worst.2));
    let seq: Vec<String> = post_skip_levels(0, 200).iter().map(|(k, l)| format!("{k}:{l}")).collect();
    lines.push(format!("after a thunderstorm skip (the skip tick reads day 1 with the levels at 1.0, both falling 0.01 a gt after it): gt after the skip:level {}", seq.join(" ")));
    lines
}

/// G1's weather sweep (Nov 12 02:43): every rain / thunder onset and end at every day time d (step
/// `step`) on day 2, the vanilla ramps; per kind: trigger 2 accepted through an arm with no skip
/// (false arms), the drop latch set with no skip (false latches) — bar 0 — and the naps a night left
/// (the least over the complete nights, against 24). Runs: release phases 0/50 x detector phases
/// 0/5/10/15 at every d.
pub fn weather_sweep(o: &CounterOpts, step: i64) -> Vec<String> {
    let mut lines = Vec::new();
    // (kind, (gt from the transition, rain flag, thunder flag))
    let kinds: Vec<(&str, Vec<(i64, bool, bool)>)> = vec![
        ("rain onset", vec![(0, true, false), (12_000, false, false)]),
        ("rain end", vec![(-12_000, true, false), (0, false, false)]),
        ("rain onset with the thunder flag on (a storm starting)", vec![(-1_000, false, true), (0, true, true), (12_000, false, false)]),
        ("rain end with the thunder flag on (a storm ending)", vec![(-12_000, true, true), (0, false, true), (1_000, false, false)]),
        ("thunder onset in rain", vec![(-6_000, true, false), (0, true, true), (6_000, false, false)]),
        ("thunder end in rain", vec![(-6_000, true, true), (0, true, false), (6_000, false, false)]),
        // (G1 on a1212942b: both flags on in one tick from clear: /weather thunder, or both timers
        // flipping together)
        ("rain and thunder on together from clear", vec![(0, true, true), (12_000, false, false)]),
    ];
    let mut total = 0u32;
    let mut arr_all = i64::MIN;
    let (mut rel_emin, mut rel_qmax, mut rel_outside) = (i64::MAX, 0u32, 0u32);
    for (kname, rel) in &kinds {
        let (mut fa, mut fl, mut runs) = (0u32, 0u32, 0usize);
        let mut least = u32::MAX;
        let mut arr_max = i64::MIN;
        let (mut qmax, mut emin, mut outside) = (0u32, i64::MAX, 0u32);
        let (mut hr, mut fh) = (0u32, 0u32);
        let mut bad_d: Vec<i64> = Vec::new();
        let mut nap_bad_d: Vec<i64> = Vec::new();
        for d in (0..24_000).step_by(step.max(1) as usize) {
            // (day 2's day time d is game time 28,000 + d: the runs start at day 20,000)
            let t0 = 28_000 + d;
            let evs: Vec<(i64, Ev)> = rel.iter().map(|&(dt, r, th)| (t0 + dt, Ev::Wx { rain: r, thunder: th })).collect();
            let mut bad = false;
            for phase in [0i64, 50] {
                for det_phase in [0i64, 5, 10, 15] {
                    let r = run(o, 3, 20_000, phase, det_phase, 100, &evs);
                    runs += 1;
                    fa += r.false_arms;
                    fl += r.false_latches;
                    arr_max = arr_max.max(r.latest_arrival);
                    qmax = qmax.max(r.queue_max);
                    hr += r.h_rephases;
                    fh += r.false_h;
                    for &(dd, _w, stale) in &r.entries {
                        if stale {
                            continue;
                        }
                        let dm = dd.rem_euclid(24_000);
                        emin = emin.min(if dm < 10 { dm + 24_000 } else { dm });
                        if (10..12_000).contains(&dm) {
                            outside += 1;
                        }
                    }
                    bad |= r.false_arms + r.false_latches > 0;
                    let last_complete = (r.end_day - 23_850).div_euclid(24_000);
                    for k in 1..=last_complete {
                        let naps = if o.rest.is_some() { r.night_naps.get(&k) } else { r.night_picks.get(&k) };
                        let n = naps.copied().unwrap_or(0);
                        least = least.min(n);
                        if n < 24 && nap_bad_d.last() != Some(&d) {
                            nap_bad_d.push(d);
                        }
                    }
                }
            }
            if bad {
                bad_d.push(d);
            }
        }
        total += fa + fl;
        arr_all = arr_all.max(arr_max);
        // (contiguous d runs)
        let band_str = |v: &[i64]| {
            let mut out: Vec<String> = Vec::new();
            let mut i = 0;
            while i < v.len() {
                let mut j = i;
                while j + 1 < v.len() && v[j + 1] == v[j] + step {
                    j += 1;
                }
                out.push(if i == j { format!("{}", v[i]) } else { format!("{}..{}", v[i], v[j]) });
                i = j + 1;
            }
            if out.is_empty() { "none".to_string() } else { out.join(", ") }
        };
        let bands = band_str(&bad_d);
        let nap_bands = band_str(&nap_bad_d);
        lines.push(format!(
            "  {kname} at every d (0..24,000 step {step}; {runs} runs): false arms {fa}, false latches {fl} (at d {bands}); naps a night least {} of 24 (a night short of 24 at d {nap_bands}); {}",
            if least == u32::MAX { "-".to_string() } else { least.to_string() },
            if o.rest.is_some() {
                format!(
                    "the release: the earliest entry {} ({}), outside REST {outside}, held at the gate at once at most {qmax} ({}){}",
                    if emin == i64::MAX { "-".to_string() } else { emin.to_string() },
                    if emin >= 12_000 { "pass" } else { "FAIL" },
                    if qmax <= 1 { "pass" } else { "FAIL" },
                    if o.high.is_some() { format!("; H re-phases {hr} (false {fh}: {})", if fh == 0 { "pass" } else { "FAIL" }) } else { String::new() }
                )
            } else {
                format!("the latest bed-stop arrival {}", if arr_max == i64::MIN { "-".to_string() } else { arr_max.to_string() })
            }
        ));
        rel_emin = rel_emin.min(emin);
        rel_qmax = rel_qmax.max(qmax);
        rel_outside += outside;
    }
    lines.insert(0, format!(
        "weather sweep (G1 Nov 12 02:43; the arm: {}; the latch: {}; L {}): every rain or thunder onset and end at every day time, the vanilla ramps (0.01/gt, thunder x rain), the detector on redsim's exact curve; bar: 0 false arms",
        o.arm.map_or("off".to_string(), |s| format!("falling 8 for {s} gt")),
        o.drop_min.map_or("off".to_string(), |k| format!("a fall >= {k} between readings")),
        o.day_inhibit.map_or("off".to_string(), |l| l.to_string())
    ));
    lines.push(format!("  => false arms + latches {total} ({})", if total == 0 { "pass" } else { "FAIL" }));
    if o.rest.is_some() {
        lines.push(format!(
            "  => the release over every weather kind and d: the earliest bed-stop entry {} (bar >= 12,000: {}); entries outside REST {rel_outside}; held at the gate at once at most {rel_qmax} (bar 1: {})",
            if rel_emin == i64::MAX { "-".to_string() } else { rel_emin.to_string() },
            if rel_emin >= 12_000 { "pass" } else { "FAIL" },
            if rel_qmax <= 1 { "pass" } else { "FAIL" }
        ));
    } else {
        lines.push(format!(
            "  => the plain hatch: the latest bed-stop arrival over every weather kind and d {}",
            if arr_all == i64::MIN { "-".to_string() } else { arr_all.to_string() }
        ));
    }
    lines
}

/// Merge one scenario's release-model fields into a sweep's.
fn merge_release(a: &mut ScenOut, s: &ScenOut) {
    a.entry_min = a.entry_min.min(s.entry_min);
    a.entry_max = a.entry_max.max(s.entry_max);
    a.outside_rest += s.outside_rest;
    a.stale_entries += s.stale_entries;
    a.wait = (a.wait.0.min(s.wait.0), a.wait.1.max(s.wait.1), a.wait.2 + s.wait.2, a.wait.3 + s.wait.3);
    if s.wait_max_at.0 > a.wait_max_at.0 {
        a.wait_max_at = s.wait_max_at;
    }
    a.deferred += s.deferred;
    a.queue_max = a.queue_max.max(s.queue_max);
    a.away_max = a.away_max.max(s.away_max);
    a.rest_rise = (a.rest_rise.0.min(s.rest_rise.0), a.rest_rise.1.max(s.rest_rise.1));
    a.rest_rise_day += s.rest_rise_day;
    a.min_naps = a.min_naps.min(s.min_naps);
    a.stale_admit_n += s.stale_admit_n;
    a.stale_admit_dt = (a.stale_admit_dt.0.min(s.stale_admit_dt.0), a.stale_admit_dt.1.max(s.stale_admit_dt.1));
    a.stale_clear_dt = (a.stale_clear_dt.0.min(s.stale_clear_dt.0), a.stale_clear_dt.1.max(s.stale_clear_dt.1));
    a.h_rephases += s.h_rephases;
    a.false_h += s.false_h;
    a.class_a += s.class_a;
    a.class_b += s.class_b;
    a.class_n += s.class_n;
    a.exit_gap_min = a.exit_gap_min.min(s.exit_gap_min);
    a.liedown_gap_min = a.liedown_gap_min.min(s.liedown_gap_min);
    for (k, &(lo, hi)) in &s.naps_by_key {
        let e = a.naps_by_key.entry(*k).or_insert((u32::MAX, 0));
        *e = (e.0.min(lo), e.1.max(hi));
    }
}

/// One scenario's release-model line (`rest`).
fn release_line(s: &ScenOut) -> String {
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let mean = if s.wait.3 > 0 { format!("{:.0}", s.wait.2 as f64 / s.wait.3 as f64) } else { "-".to_string() };
    format!(
        "    release (REST_l AND the stale-window hold AND NOT DAY after it AND the stop clear): entries {}: the earliest at dusk at day time {} (bar >= 12,000), the latest {}, {} outside REST (10..11,999, not after a skip); stale admissions {} (in a skip's stale window, day times {}: each waits on the stop's ledge until REST and naps that evening, the stop clear at day time {}); the least gap between consecutive departures from the stop {} gt; the least time from a napper's R to the next napper's lie-down {} gt; the wait at the queue {}..{} gt (mean {mean}); picks deferred by the back-pressure {}; held at the gate at once at most {} (bar 1); riders away at a release at most {}; REST_l rose at day time {}..{} (+ {} transient rises by day, DAY high: no release); naps a night (complete nights no skip cut) least {} of 24",
        s.wait.3,
        m(s.entry_min, i64::MAX),
        m(s.entry_max, i64::MIN),
        s.outside_rest,
        s.stale_admit_n,
        if s.stale_admit_n == 0 { "-".to_string() } else { format!("{}..{}", s.stale_admit_dt.0, s.stale_admit_dt.1) },
        if s.stale_admit_n == 0 { "-".to_string() } else { format!("{}..{}", s.stale_clear_dt.0, s.stale_clear_dt.1) },
        m(s.exit_gap_min, i64::MAX),
        m(s.liedown_gap_min, i64::MAX),
        m(s.wait.0, i64::MAX),
        m(s.wait.1, i64::MIN),
        s.deferred,
        s.queue_max,
        s.away_max,
        m(s.rest_rise.0, i64::MAX),
        m(s.rest_rise.1, i64::MIN),
        s.rest_rise_day,
        if s.min_naps == u32::MAX { "-".to_string() } else { s.min_naps.to_string() }
    )
}

/// The high re-phase's parameters for threshold `h` (ironspec 09:49): the clear curve's first and last
/// day time at level >= h (the span a natural H rise can fall in: weather only delays or prevents
/// it), C at the release after the H rise on normal days (every release and detector phase), the load
/// (the largest C <= that minimum whose units are 0 or 9, the FF's two taps: never ahead of the clear
/// clock), and the window [lo, hi] = [the least C at the rise - 2, the most + the span in releases + 2].
/// Returns ((h, load, lo, hi), a description).
pub fn high_params(o: &CounterOpts, h: i64) -> ((i64, i64, i64, i64), String) {
    let t_h = (0..24_000).find(|&d| det_level(d, 0.0, 0.0) >= h).unwrap_or(0);
    let t_h_end = (t_h..24_000).take_while(|&d| det_level(d, 0.0, 0.0) >= h).last().unwrap_or(t_h);
    let probe = CounterOpts { variant: Variant::Ff, high: Some((h, 0, i64::MIN, i64::MAX)), ..o.clone() };
    let s = scenario_with(&probe, &[], 5, &[0, 7, 13, 19]);
    let (cmin, cmax) = s.c_h;
    let load = (0..=cmin).rev().find(|c| c.rem_euclid(10) == 0 || c.rem_euclid(10) == 9).unwrap_or(0);
    let span = (t_h_end - t_h + 99) / 100;
    let (lo, hi) = (cmin - 2, cmax + span + 2);
    (
        (h, load, lo, hi),
        format!(
            "the high re-phase (ironspec 09:49): H {h}, the clear level >= {h} over day time {t_h}..{t_h_end} (rain tops at 12, thunder at 10: weather only delays or prevents an H rise); C at the release after the clear H rise {cmin}..{cmax} => load C {load} (units 0 or 9 for the FF's taps; never ahead), window C {lo}..{hi} (the natural span + 2); an H rise with C outside it re-phases at the next release"
        ),
    )
}

/// The release model's own rows (ironspec Nov 12 09:04..09:19): the clear dusk's DAY fall and the clear
/// dawn's DAY rise at the earliest detector phase ((b)'s condition), (a)'s load (the clear clock's C at
/// the dawn DAY rise: the clear-weather edge time), the daytime thunderstorm skip at every 1,000 gt of
/// the day under (b) and (a) (the earliest entry, the most held at once, the held napper's wait, the
/// deferred picks, the post-skip night's naps), the back-pressure against a no-back-pressure control,
/// and restarts.
fn release_report(base: &CounterOpts, scen: &[(&str, Vec<(i64, Ev)>)]) -> Vec<String> {
    let mut lines = Vec::new();
    let o = CounterOpts { variant: Variant::Ff, ..base.clone() };
    let l = o.day_inhibit.unwrap_or(6);
    let first = |from: i64, to: i64, want_hi: bool| (from..to).find(|&d| (det_level(d, 0.0, 0.0) >= l) == want_hi);
    // (the DAY out (-5,14,6) follows a detector update by the DAY_LV comparator and its repeater)
    let out_lag = 4;
    let pf = |ok: bool| if ok { "pass" } else { "FAIL" };
    if let Some(t5) = first(9_000, 14_000, false) {
        lines.push(format!(
            "the clear dusk's DAY fall ((b)'s condition, ironspec 09:12): the clear level < L {l} from day time {t5}; read at the earliest detector phase (an update at {t5}) the DAY out (-5,14,6) falls at {} ({out_lag} gt: the DAY_LV comparator and the output repeater; the latest phase 19 gt later), the hatch 2 + the line after it: against REST's 12,000 a margin of {} gt ({})",
            t5 + out_lag,
            t5 + out_lag - 12_000,
            pf(t5 + out_lag >= 12_000)
        ));
    }
    let t6 = first(20_000, 24_000, true);
    let normal = scenario_with(&o, &[], 5, &[0, 7, 13, 19]);
    let load_a = if normal.c_rise.0 == i64::MAX { None } else { Some(normal.c_rise.0) };
    lines.push(format!(
        "the clear dawn's DAY rise: the clear level >= L {l} from day time {}, the DAY out {} at the earliest phase; the C the clock shows at the release after it on normal days {}..{} (the rise read at day time {}..{}) => (a)'s load C {} (the clear-weather edge time, its earliest phase: never ahead of the clear clock; ironspec 09:19)",
        t6.map_or("-".to_string(), |t| t.to_string()),
        t6.map_or("-".to_string(), |t| (t + out_lag).to_string()),
        normal.c_rise.0, normal.c_rise.1, normal.rise_dt.0, normal.rise_dt.1,
        load_a.map_or("-".to_string(), |c| c.to_string())
    ));
    let b_plain = CounterOpts { high: None, ..o.clone() };
    let mut variants: Vec<(&str, CounterOpts)> = vec![("(b), the gate's trigger 2 only", b_plain.clone())];
    if let Some(c) = load_a {
        variants.push(("(a), every DAY rise loads C", CounterOpts { day_rise_load: Some(c), ..b_plain.clone() }));
    }
    for (vname, ov) in &variants {
        lines.push(format!(
            "the daytime thunderstorm skip at every 1,000 gt of the day, {vname} (the storm from 3,000 gt before the skip or from day 0, the skip clearing it; 20 release phases x 2 detector phases x 10 units phases, 4 days; the post-skip night is the jump's day's):"
        ));
        for d in (1_000..=11_000).step_by(1_000) {
            let evs = vec![(4_000 + (d - 3_000).max(0), Ev::Wx { rain: true, thunder: true }), (4_000 + d, Ev::Skip)];
            let s = scenario_days(ov, &evs, 5, &[0, 13], 4);
            let nk = |k: i64| s.naps_by_key.get(&k).map_or("-".to_string(), |(a, b)| format!("{a}..{b}"));
            let lost = s.naps_by_key.get(&2).map_or(0, |(a, _)| 24u32.saturating_sub(*a));
            lines.push(format!(
                "  skip at {d}: the earliest entry {} ({}), outside REST {}; held at the gate at once at most {} ({}); the longest wait at the queue {} gt (arrived at day time {}, entered at {}); picks deferred {}; riders away at a release at most {}; the post-skip night's naps {} of 24 (lost up to {lost}), the night after {}; drop-latch sets {} (each re-phases the clock at the next 7 -> 8 edge)",
                if s.entry_min == i64::MAX { "-".to_string() } else { s.entry_min.to_string() },
                pf(s.entry_min >= 12_000),
                s.outside_rest,
                s.queue_max,
                pf(s.queue_max <= 1),
                s.wait_max_at.0, s.wait_max_at.1, s.wait_max_at.2,
                s.deferred,
                s.away_max,
                nk(2),
                nk(3),
                s.latch_sets
            ));
        }
    }
    // (ironspec 09:49: (b) + the high re-phase, the daytime storm skip at every 250 gt over 0..12,000:
    // 24 naps the post-skip night; an uncovered band printed with its nap cost)
    if let Some((h, load, lo, hi)) = o.high {
        let mut cells: Vec<String> = Vec::new();
        let mut short: Vec<(i64, u32)> = Vec::new();
        let (mut hr, mut fh, mut emin) = (0u32, 0u32, i64::MAX);
        for d in (0..12_000).step_by(250) {
            let evs = vec![(4_000 + (d - 3_000).max(0), Ev::Wx { rain: true, thunder: true }), (4_000 + d, Ev::Skip)];
            let s = scenario_days(&o, &evs, 5, &[0, 13], 4);
            let naps = s.naps_by_key.get(&2).map_or(0, |(a, _)| *a);
            hr += s.h_rephases;
            fh += s.false_h;
            emin = emin.min(s.entry_min);
            cells.push(format!("{d}:{naps}"));
            if naps < 24 {
                short.push((d, naps));
            }
        }
        lines.push(format!(
            "(b) + the high re-phase (H {h}, load C {load}, window C {lo}..{hi}): the daytime thunderstorm skip at every 250 gt of 0..11,750 (the storm from 3,000 gt before the skip or from day 0), the post-skip night's least naps (skip day time:naps) {}; H re-phases {hr} (false {fh}); the earliest entry {} ({}); {}",
            cells.join(" "),
            if emin == i64::MAX { "-".to_string() } else { emin.to_string() },
            pf(emin >= 12_000),
            if short.is_empty() {
                "every skip time 24 naps (pass)".to_string()
            } else {
                format!(
                    "uncovered: {} (naps lost up to {} at a skip there; a stated limit for the build guide)",
                    short.iter().map(|(d, n)| format!("{d} ({n})")).collect::<Vec<_>>().join(", "),
                    short.iter().map(|(_, n)| 24 - n).max().unwrap_or(0)
                )
            }
        ));
    }
    // (ironspec 09:45: a skip at every gt offset of a napper's walk and nap in the stop, clear and
    // thunder: the outcome class, the picks deferred, the post-skip night's naps)
    let plain = run(&o, 3, 20_000, 0, 0, 100, &[]);
    if let Some(te) = plain.entries.iter().find(|(d, _, _)| d.div_euclid(24_000) == 1 && d.rem_euclid(24_000) >= 12_000).map(|(d, _, _)| d - 20_000) {
        for (wname, thunder) in [("clear", false), ("a thunderstorm from 2,000 gt before", true)] {
            let (mut a, mut b, mut n) = (0u32, 0u32, 0u32);
            let mut deferred = 0u32;
            let (mut nmin, mut nmax) = (u32::MAX, 0u32);
            let (mut bwait, mut emin) = (0i64, i64::MAX);
            let span = o.stop_walk + o.stop_nap + 20;
            for since in 0..span {
                let mut evs = Vec::new();
                if thunder {
                    evs.push((te + since - 2_000, Ev::Wx { rain: true, thunder: true }));
                }
                evs.push((te + since, Ev::Skip));
                let r = run(&o, 4, 20_000, 0, 0, 100, &evs);
                for &(_s, class) in &r.skip_classes {
                    match class {
                        b'a' => a += 1,
                        b'b' => b += 1,
                        _ => n += 1,
                    }
                }
                deferred += r.deferred;
                let k = r.night_naps.get(&2).copied().unwrap_or(0);
                nmin = nmin.min(k);
                nmax = nmax.max(k);
                for &(dd, w, stale) in &r.entries {
                    bwait = bwait.max(w);
                    if !stale {
                        let dm = dd.rem_euclid(24_000);
                        emin = emin.min(if dm < 10 { dm + 24_000 } else { dm });
                    }
                }
            }
            lines.push(format!(
                "a skip at every gt of a napper's stay in the stop ({wname}; the napper entered at night (game time {te}); the stop's walk {} gt, nap {} gt, exit {} gt, 58f770cc's figures pending; skips at {span} offsets): class a (asleep: woken, picked up, the stop's release on the wake edge) {a}; class b (on foot when REST ends: waits for the next REST on a no-stamp cell, holding the stop until REST + a cycle) {b}; already leaving {n}; picks deferred by the back-pressure meanwhile {deferred}; the longest wait at the queue {bwait} gt; the post-skip night's naps {}..{} of 24; the earliest entry at dusk {} ({})",
                o.stop_walk,
                o.stop_nap,
                o.stop_cycle - o.stop_walk - o.stop_nap,
                if nmin == u32::MAX { 0 } else { nmin },
                nmax,
                if emin == i64::MAX { "-".to_string() } else { emin.to_string() },
                pf(emin >= 12_000)
            ));
        }
    }
    // (ironspec 09:19: the back-pressure on its own merits, against a no-back-pressure control)
    lines.push("the back-pressure against a no-back-pressure control (each scripted scenario; 20 release phases x 2 detector phases x 10 units phases):".to_string());
    let ctl = CounterOpts { backpressure: false, ..o.clone() };
    for (label, evs) in scen {
        let on = scenario_with(&o, evs, 5, &[0, 13]);
        let off = scenario_with(&ctl, evs, 5, &[0, 13]);
        let n = |x: u32| if x == u32::MAX { "-".to_string() } else { x.to_string() };
        lines.push(format!(
            "  {label}: held at the gate at once at most {} (control {}), picks deferred {}, naps a night least {} of 24 (control {}), the earliest entry {} (control {})",
            on.queue_max, off.queue_max, on.deferred, n(on.min_naps), n(off.min_naps),
            if on.entry_min == i64::MAX { "-".to_string() } else { on.entry_min.to_string() },
            if off.entry_min == i64::MAX { "-".to_string() } else { off.entry_min.to_string() }
        ));
    }
    lines.push("restarts: a server stop halts the day time and every block tick together, and the lead's portal-ticket loader keeps the farm's chunks entity-ticking whenever the server runs, so a restart moves no edge against the clock in this model; logins (L1, <= 3 gt) are the box's block benches' (piece 8)".to_string());
    lines
}

/// The bench: both variants, the scripted scenarios, ironspec's bars per variant.
pub fn bench(base: &CounterOpts) -> Vec<String> {
    let mut lines = Vec::new();
    // day 2's day time X is game time 4,000 + X (the runs start at day 20,000, before RISE)
    let wx = |rain: bool, thunder: bool| Ev::Wx { rain, thunder };
    let scen: Vec<(&str, Vec<(i64, Ev)>)> = vec![
        ("normal days", vec![]),
        ("a bed-time clear skip (12,623)", vec![(4_000 + 12_623, Ev::Skip)]),
        ("a bed-time rain skip (rain from 11,000, the skip at 12,102)", vec![(4_000 + 11_000, wx(true, false)), (4_000 + 12_102, Ev::Skip)]),
        ("a skip at 16,000", vec![(4_000 + 16_000, Ev::Skip)]),
        ("the latest clear skip (23,585)", vec![(4_000 + 23_585, Ev::Skip)]),
        ("rain 4,000..6,000 (a mid-day clearing)", vec![(4_000 + 4_000, wx(true, false)), (4_000 + 6_000, wx(false, false))]),
        ("rain 9,000..11,800 (a dusk clearing)", vec![(4_000 + 9_000, wx(true, false)), (4_000 + 11_800, wx(false, false))]),
        ("a storm 9,000..11,800 (a dusk clearing)", vec![(4_000 + 9_000, wx(true, true)), (4_000 + 11_800, wx(false, false))]),
        // (ironspec 09:04: the release's bar over rain and thunder dusks and a storm through dawn)
        ("rain through dusk (10,000..14,000)", vec![(4_000 + 10_000, wx(true, false)), (4_000 + 14_000, wx(false, false))]),
        ("a storm through dusk (10,000..14,000)", vec![(4_000 + 10_000, wx(true, true)), (4_000 + 14_000, wx(false, false))]),
        ("a storm through dawn (20,000..2,000, no dawn DAY edge until it ends)", vec![(4_000 + 20_000, wx(true, true)), (4_000 + 26_000, wx(false, false))]),
    ];
    let variants: Vec<(&str, CounterOpts)> = vec![
        ("FF + pending latch", CounterOpts { variant: Variant::Ff, ..base.clone() }),
        ("no-reset (the tens restart at the first wrap)", CounterOpts { variant: Variant::NoReset, load2: (base.load2 / 10) * 10, ..base.clone() }),
    ];
    for (vname, o) in &variants {
        lines.push(format!(
            "{vname}: window C {}..{}, gate C >= {} or <= {}, RISE loads C {}, trigger 2 loads C {}{}; the first napper at the bed {} gt after the window opens (bar: >= {} - {} = {}); the latest lie-down {} gt after it (the night ends {}); every scenario {} runs (100 release phases x 4 detector phases x 10 units phases at the start)",
            o.window.0, o.window.1, o.gate.0, o.gate.1, o.load1, o.load2,
            if o.variant == Variant::Ff { format!(" (FF every {} gt, pending latch {})", o.ff_period, if o.pend { "on" } else { "off" }) } else { " (at the first wrap after the trigger)".to_string() },
            o.to_bed, o.bed_clear, o.early_tol, o.bed_clear - o.early_tol, o.last_nap, o.night_end, 100 * 4 * 10
        ));
        let mut worst_night = f64::MAX;
        let mut worst_early = i64::MAX;
        let mut worst_gate = i64::MAX;
        let mut worst_spacing = i64::MAX;
        let mut bad_counts = 0usize;
        let mut daylight_total = 0u32;
        let mut night_inh_total = 0u32;
        let mut worst_edge = i64::MAX;
        let mut normal_min_night = 0u32;
        let mut false_total = 0u32;
        let mut worst_coupling = i64::MAX;
        let mut daylight_held = 0u32;
        // (ironspec 06:50: the latest bed-stop arrival over every scenario, bar <= 23,900; G1 on
        // 5a1f0cd74: the hatch-open arrivals in the post-skip stale window)
        let mut worst_arrival = i64::MIN;
        let mut stale_total = 0u32;
        let mut lag_all = (i64::MAX, i64::MIN);
        // (G1 on c768c3d30: the stale opens' day times and the held nappers' entries, the plain hatch)
        let mut stale_dt_all = (i64::MAX, i64::MIN);
        let (mut held_n_all, mut held_day_all, mut held_dt_all) = (0u32, 0u32, (i64::MAX, i64::MIN));
        // (the release model's bars over every scenario and sweep)
        let (mut rel_entry_min, mut rel_outside, mut rel_stale, mut rel_queue, mut rel_deferred) = (i64::MAX, 0u32, 0u32, 0u32, 0u32);
        let mut rel_admit_dt = (i64::MAX, i64::MIN);
        let mut rel_clear_dt = (i64::MAX, i64::MIN);
        let mut rel_exit = i64::MAX;
        let mut rel_liedown = i64::MAX;
        let mut rel_normal_naps = u32::MAX;
        let mut rel_add = |s: &ScenOut| {
            rel_entry_min = rel_entry_min.min(s.entry_min);
            rel_exit = rel_exit.min(s.exit_gap_min);
            rel_liedown = rel_liedown.min(s.liedown_gap_min);
            rel_outside += s.outside_rest;
            rel_stale += s.stale_admit_n;
            rel_admit_dt = (rel_admit_dt.0.min(s.stale_admit_dt.0), rel_admit_dt.1.max(s.stale_admit_dt.1));
            rel_clear_dt = (rel_clear_dt.0.min(s.stale_clear_dt.0), rel_clear_dt.1.max(s.stale_clear_dt.1));
            rel_queue = rel_queue.max(s.queue_max);
            rel_deferred += s.deferred;
            stale_dt_all = (stale_dt_all.0.min(s.stale_dt.0), stale_dt_all.1.max(s.stale_dt.1));
            held_n_all += s.held_n;
            held_day_all += s.held_by_day;
            held_dt_all = (held_dt_all.0.min(s.held_dt.0), held_dt_all.1.max(s.held_dt.1));
        };
        let arr = |x: i64| if x == i64::MIN { "-".to_string() } else { x.to_string() };
        let lag = |l: (i64, i64)| if l.1 == i64::MIN { "-".to_string() } else { format!("{}..{}", l.0, l.1) };
        for (label, evs) in &scen {
            let s = scenario(o, evs);
            worst_arrival = worst_arrival.max(s.latest_arrival);
            stale_total += s.stale_opens;
            lag_all = (lag_all.0.min(s.skip_lag.0), lag_all.1.max(s.skip_lag.1));
            let m = |x: i64| if x == i64::MAX { "-".to_string() } else { x.to_string() };
            let (night, early) = if s.open_min == i64::MAX {
                (f64::NAN, i64::MAX)
            } else {
                let night = (o.night_end - (s.open_max + o.last_nap)) as f64 / o.night_len as f64 * 100.0;
                let early = s.open_min + o.to_bed - (o.bed_clear - o.early_tol);
                (night, early)
            };
            if night.is_finite() {
                worst_night = worst_night.min(night);
            }
            worst_early = worst_early.min(early);
            for g in [s.hi.1, s.lo.1, s.rej.1] {
                worst_gate = worst_gate.min(g);
            }
            worst_spacing = worst_spacing.min(s.min_spacing);
            bad_counts += s.cerr_bad;
            lines.push(format!(
                "  {label}: the window opens at day {}..{} (night margin at the worst trip {:.1}%; the first napper {} gt {} the bar); naps a night (complete nights no skip cut) least {} of 24; picks whose napper reaches the bed before the stop takes it {} (of {} runs); trigger 2 accepted high {} (least {} gt inside), accepted low {} (least {} gt inside), rejected {} (least {} gt outside); FF longest {} gt ({} steps, {} held), overflows {}, absorbed {}; ring pulses at least {} gt apart; count errors {} of {}; arm accepts {} (false {}), latch sets {} (false {}); DAY off {} gt before the window opens; the latest bed-stop arrival {} (hatch-open arrivals after a skip, in its stale window: {}); a skip to DAY high at the detector {} gt",
                m(s.open_min), if s.open_max == i64::MIN { "-".to_string() } else { s.open_max.to_string() },
                night, early.abs(), if early >= 0 { "inside" } else { "BEFORE" },
                if s.min_night == u32::MAX { "-".to_string() } else { s.min_night.to_string() },
                s.daylight, s.runs,
                s.hi.0, m(s.hi.1), s.lo.0, m(s.lo.1), s.rej.0, m(s.rej.1),
                s.ff_max, s.ff_steps_max, s.held_max, s.ovf, s.absorbed, m(s.min_spacing), s.cerr_bad, s.cerr_n,
                s.arm_accepts, s.false_arms, s.latch_sets, s.false_latches, m(s.day_off_margin), arr(s.latest_arrival), s.stale_opens, lag(s.skip_lag)
            ));
            rel_add(&s);
            if o.rest.is_some() {
                lines.push(release_line(&s));
                if *label == "normal days" {
                    rel_normal_naps = s.min_naps;
                }
            }
            daylight_total += s.daylight;
            night_inh_total += s.night_inh;
            false_total += s.false_arms + s.false_latches;
            if o.day_inhibit.is_some() {
                worst_coupling = worst_coupling.min(s.day_off_margin);
                worst_edge = worst_edge.min(s.edge_gap);
            }
            if *label == "normal days" {
                normal_min_night = s.min_night;
            }
        }
        // (ironspec 01:38: every skip time inside the window x weather; the FF variant only)
        if o.variant == Variant::Ff {
            for (wname, w) in [("clear", None), ("rain", Some((true, false))), ("a thunderstorm", Some((true, true)))] {
                let (mut dl, mut ni, mut eg, mut runs) = (0u32, 0u32, i64::MAX, 0usize);
                let (mut omin, mut omax) = (i64::MAX, i64::MIN);
                let mut gate_min = i64::MAX;
                let mut fa = 0u32;
                let mut arr_max = i64::MIN;
                let mut stale = 0u32;
                let mut lag_w = (i64::MAX, i64::MIN);
                let mut sweep = ScenOut {
                    entry_min: i64::MAX,
                    entry_max: i64::MIN,
                    wait: (i64::MAX, i64::MIN, 0, 0),
                    rest_rise: (i64::MAX, i64::MIN),
                    min_naps: u32::MAX,
                    stale_dt: (i64::MAX, i64::MIN),
                    held_dt: (i64::MAX, i64::MIN),
                    stale_admit_dt: (i64::MAX, i64::MIN),
                    stale_clear_dt: (i64::MAX, i64::MIN),
                    c_h: (i64::MAX, i64::MIN),
                    exit_gap_min: i64::MAX,
                    liedown_gap_min: i64::MAX,
                    ..Default::default()
                };
                for skip_day in (12_500..=20_500).step_by(250) {
                    // the weather from day 11,500 until the skip (a skip clears it)
                    let mut evs = Vec::new();
                    if let Some((r, th)) = w {
                        evs.push((4_000 + 11_500, wx(r, th)));
                    }
                    evs.push((4_000 + skip_day, Ev::Skip));
                    let s = scenario_with(o, &evs, 5, &[0, 13]);
                    rel_add(&s);
                    merge_release(&mut sweep, &s);
                    arr_max = arr_max.max(s.latest_arrival);
                    stale += s.stale_opens;
                    lag_w = (lag_w.0.min(s.skip_lag.0), lag_w.1.max(s.skip_lag.1));
                    dl += s.daylight;
                    ni += s.night_inh;
                    eg = eg.min(s.edge_gap);
                    runs += s.runs;
                    omin = omin.min(s.open_min);
                    omax = omax.max(s.open_max);
                    fa += s.false_arms + s.false_latches;
                    for g in [s.hi.1, s.lo.1, s.rej.1] {
                        gate_min = gate_min.min(g);
                    }
                }
                daylight_held += dl;
                night_inh_total += ni;
                false_total += fa;
                worst_gate = worst_gate.min(gate_min);
                worst_arrival = worst_arrival.max(arr_max);
                stale_total += stale;
                lag_all = (lag_all.0.min(lag_w.0), lag_all.1.max(lag_w.1));
                let night = (o.night_end - (omax + o.last_nap)) as f64 / o.night_len as f64 * 100.0;
                worst_night = worst_night.min(night);
                lines.push(format!(
                    "  a skip at every 250 gt of the window (12,500..20,500), {wname} from day 11,500 until the skip ({runs} runs: 20 release phases x 2 detector phases x 10 units phases x 33 skip times): picks reaching the bed early {dl} ({:.1}%; each held at the queue's NOT-DAY gate until dusk, ironspec 02:28: +10..21 golems a held napper); night picks inhibited {ni}; the least gap from a divert to the inhibit's last change {} (a divert in the skip's own update: the held case); the window opens at day {}..{} (night margin {:.1}%); gate margins least {} gt; false arms + latches {fa}; the latest bed-stop arrival {} (hatch-open arrivals in a skip's stale window: {stale}); a skip to DAY high at the detector {} gt",
                    dl as f64 / runs.max(1) as f64 * 100.0,
                    if eg == i64::MAX { "-".to_string() } else { eg.to_string() }, omin, omax, night, gate_min, arr(arr_max), lag(lag_w)
                ));
                if o.rest.is_some() {
                    lines.push(release_line(&sweep));
                }
            }
            // a daytime thunderstorm skip (a day skip happens only in thunder): with no arm the gate
            // rejects its trigger 2 (C ~ 79), so the count stays a day's phase off until the next RISE
            let evs = vec![(4_000 + 3_000, wx(true, true)), (4_000 + 6_000, Ev::Skip)];
            let s = scenario_with(o, &evs, 5, &[0, 13]);
            rel_add(&s);
            worst_arrival = worst_arrival.max(s.latest_arrival);
            stale_total += s.stale_opens;
            lag_all = (lag_all.0.min(s.skip_lag.0), lag_all.1.max(s.skip_lag.1));
            lines.push(format!(
                "  a daytime thunderstorm skip (6,000; the storm from 3,000): picks reaching the bed early {}; night picks inhibited {}; the window opens at day {}..{}; arm accepts {} (false {}), latch sets {} (false {}); the latest bed-stop arrival {} (stale-window opens {}); a skip to DAY high at the detector {} gt",
                s.daylight, s.night_inh, s.open_min, s.open_max, s.arm_accepts, s.false_arms, s.latch_sets, s.false_latches, arr(s.latest_arrival), s.stale_opens, lag(s.skip_lag)
            ));
            if o.rest.is_some() {
                lines.push(release_line(&s));
            }
            daylight_total += s.daylight;
        }
        let pass_edge = o.day_inhibit.is_none() || worst_edge >= 4;
        let pass_night = worst_night > 0.0;
        let pass_early = worst_early >= 0;
        let pass_gate = worst_gate >= 100;
        let pass_spacing = worst_spacing >= o.ff_period;
        let pass_counts = bad_counts == 0;
        lines.push(format!(
            "  => bars (normal operation: the scripted scenarios): every normal night 24 naps: least {normal_min_night} ({}); night margin {:.1}% ({}); the first napper's arrival {} gt ({}); picks before the beds take them {} ({}); {}gate margins >= 1 release: least {} gt ({}); ring pulses >= {} gt apart: least {} ({}); count errors {} ({}); false arms + latches {false_total} ({}); skips inside the window: {daylight_held} picks held at the queue until dusk (priced)",
            if normal_min_night >= 24 { "pass" } else { "FAIL" },
            worst_night, if pass_night { "pass" } else { "FAIL" },
            worst_early, if pass_early { "pass" } else { "FAIL" },
            daylight_total, if daylight_total == 0 { "pass" } else { "FAIL" },
            match o.day_inhibit {
                Some(l) => format!(
                    "night picks the level < {l} inhibit removed {} ({}); the inhibit's edge >= 3 gt + 1 tick from a divert: least {} ({}); DAY off >= 20 gt before the window opens: least {} ({}); ",
                    night_inh_total, if night_inh_total == 0 { "pass" } else { "FAIL" },
                    if worst_edge == i64::MAX { "-".to_string() } else { worst_edge.to_string() }, if pass_edge { "pass" } else { "FAIL" },
                    if worst_coupling == i64::MAX { "-".to_string() } else { worst_coupling.to_string() }, if worst_coupling >= 20 { "pass" } else { "FAIL" }
                ),
                None => String::new(),
            },
            worst_gate, if pass_gate { "pass" } else { "FAIL" },
            o.ff_period, worst_spacing, if pass_spacing { "pass" } else { "FAIL" },
            bad_counts, if pass_counts { "pass" } else { "FAIL" },
            if false_total == 0 { "pass" } else { "FAIL" }
        ));
        if let Some((rs, rr)) = o.rest {
            // (ironspec 09:19: the current bars replace the dawn bar (<= 23,900): option A's outcome over
            // the stale window (58f770cc's rows; here the entries inside one, which the hold must leave
            // at 0), the earliest bed-stop entry >= 12,000, one napper held at most)
            let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
            lines.push(format!(
                "  => the release's bars over every scenario and sweep above (REST_l = C >= {rs} or C < {rr}; every napper held >= {} gt at the queue; NOT DAY as the hatch sees it after the hold; the stop clear (R's RS; the stop's cycle {} gt); the back-pressure {}): the earliest bed-stop entry at dusk {} (bar >= 12,000: {}); entries outside REST not after a skip {rel_outside} ({}); stale admissions {rel_stale} (entered in a skip's stale window at day time {}; each waits on the stop's ledge until REST, naps that evening and leaves, the stop clear at day time {}: ironspec 09:31, option A's outcome rows are 58f770cc's); the least gap between consecutive departures from the stop {} gt (ironspec 10:10: the hold's bar, its longest wait + 12 + clear + 20, against it with 7b90c3f1's trip spread); the least time from a napper's R to the next napper's lie-down {} gt (ironspec 10:52, HOLD dropped: the departing napper leaves the 16-ball within it, 58f770cc's R-to-ball-exit against the bar 20); held at the gate at once at most {rel_queue} (bar 1: {}); picks deferred {rel_deferred}; normal nights' naps least {} of 24 ({})",
                o.stale_hold,
                o.stop_cycle,
                if o.backpressure { "on" } else { "OFF" },
                m(rel_entry_min, i64::MAX),
                if rel_entry_min >= 12_000 { "pass" } else { "FAIL" },
                if rel_outside == 0 { "pass" } else { "FAIL" },
                if rel_stale == 0 { "-".to_string() } else { format!("{}..{}", rel_admit_dt.0, rel_admit_dt.1) },
                if rel_stale == 0 { "-".to_string() } else { format!("{}..{}", rel_clear_dt.0, rel_clear_dt.1) },
                m(rel_exit, i64::MAX),
                m(rel_liedown, i64::MAX),
                if rel_queue <= 1 { "pass" } else { "FAIL" },
                if rel_normal_naps == u32::MAX { "-".to_string() } else { rel_normal_naps.to_string() },
                if rel_normal_naps >= 24 && rel_normal_naps != u32::MAX { "pass" } else { "FAIL" }
            ));
        } else {
            lines.push(format!(
                "  => the plain hatch (no release model): the latest bed-stop arrival it opens for (the DAY the hatch sees low: an update >= {} + 1 gt before; the day time the clock's at the arrival, a skip in transit included) over every scenario above (the napper's lane offset 0 / {} / {} added) {}; hatch-open arrivals in a skip's stale window {} (day times {}; each after VILLAGER_DEFAULT's REST ends at 10: an IDLE entry that cannot nap before dusk, its risk HOME over a day-long wait in the stop, G1 on c768c3d30); held nappers entering when the hatch next sees DAY low {} (day times {}; by day, a storm dropping DAY: {})",
                o.hatch_lag,
                o.launch_step, 2 * o.launch_step,
                arr(worst_arrival),
                stale_total,
                if stale_dt_all.1 == i64::MIN { "-".to_string() } else { format!("{}..{}", stale_dt_all.0, stale_dt_all.1) },
                held_n_all,
                if held_dt_all.1 == i64::MIN { "-".to_string() } else { format!("{}..{}", held_dt_all.0, held_dt_all.1) },
                held_day_all
            ));
        }
        lines.push(format!(
            "  => after a skip, DAY high at the detector {} gt later (every skip of every scenario and sweep above; the detector updates every {} gt, the weather's levels ramping down 0.01/gt after a rain or storm skip); at the box's DAY out (-5,14,6) add the DAY_LV comparator and the output repeater (4 gt), at the queue's hatch the line to it",
            lag(lag_all),
            o.det_period
        ));
    }
    if base.rest.is_some() {
        lines.extend(release_report(base, &scen));
    }
    lines
}
