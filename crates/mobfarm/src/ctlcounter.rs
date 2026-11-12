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
}

impl Default for CounterOpts {
    fn default() -> Self {
        CounterOpts {
            variant: Variant::Ff,
            ff_period: 18,
            launch_step: 12,
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
    /// The latest bed-stop arrival the queue's hatch opens for (not inhibited, and DAY low when it
    /// arrives: the NOT-DAY gate), as a night day time: 12,000..23,999, or 24,000 + t past midnight
    /// (ironspec Nov 12 06:50: bar <= 23,900, a dawn wake with the hatch open seats the napper in the
    /// stop's zombie boat). Arrivals with DAY high wait at the shut hatch until dusk (held nappers) and
    /// are not counted. i64::MIN: none.
    pub latest_arrival: i64,
}

/// One run: `days` days from day time `d0` at game time 0, releases at game times = `phase` mod 100,
/// detector updates at game times = `det_phase` mod `det_period`, the counter starting at C = `c0`.
pub fn run(o: &CounterOpts, days: i64, d0: i64, phase: i64, det_phase: i64, c0: i64, evs: &[(i64, Ev)]) -> RunOut {
    let mut out = RunOut { min_spacing: i64::MAX, inhibit_edge_gap: i64::MAX, latest_arrival: i64::MIN, ..Default::default() };
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
    let mut arrivals: Vec<(i64, i64)> = Vec::new(); // pending bed-stop arrivals (game time, absolute day time)
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
            }
            last_skip_t = t;
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
                if hi_now != day_hi {
                    day_hi = hi_now;
                    day_changed_t = t;
                    if !hi_now {
                        out.day_off.push(dm);
                    }
                }
            }
            let hi = level_int >= 1;
            let eight = level_int >= 8;
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
            // (the bed-stop arrival, its lane's launch offset added, judged when it happens: the hatch
            // opens for it only while DAY is low)
            if !inhibited {
                arrivals.push((t + (o.to_bed - o.to_divert) + lane_off, arrive_abs + lane_off));
            }
        }
        // the arrivals due now: with DAY low (or no inhibit) the queue's hatch opens for the napper, so
        // its day time counts against the dawn bar (a night day time; past midnight as 24,000 + t); with
        // DAY high it waits at the shut hatch until dusk (a held napper)
        while let Some(i) = arrivals.iter().position(|a| a.0 == t) {
            let (_, abs) = arrivals.remove(i);
            if o.day_inhibit.is_none() || !day_hi {
                let dm_l = abs.rem_euclid(24_000);
                let night_dt = if dm_l >= 12_000 { dm_l } else { dm_l + 24_000 };
                out.latest_arrival = out.latest_arrival.max(night_dt);
            }
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
            } else if (trig2_pend || rise_pend) && o.variant == Variant::Ff {
                let load = if trig2_pend { o.load2 } else { o.load1 };
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
                diverts.push((t + o.to_divert, lane as i64 * o.launch_step));
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
}

/// A scenario over release phases (every `phase_step`), the given detector phases and every units
/// phase at the start.
pub fn scenario(o: &CounterOpts, evs: &[(i64, Ev)]) -> ScenOut {
    scenario_with(o, evs, 1, &[0, 7, 13, 19])
}

pub fn scenario_with(o: &CounterOpts, evs: &[(i64, Ev)], phase_step: usize, det_phases: &[i64]) -> ScenOut {
    let mut s = ScenOut { open_min: i64::MAX, open_max: i64::MIN, hi: (0, i64::MAX), lo: (0, i64::MAX), rej: (0, i64::MAX), min_spacing: i64::MAX, edge_gap: i64::MAX, min_night: u32::MAX, day_off_margin: i64::MAX, latest_arrival: i64::MIN, ..Default::default() };
    for phase in (0..100).step_by(phase_step) {
        for &det_phase in det_phases {
            for c0 in 100i64..110 {
                s.runs += 1;
                let r = run(o, 3, 20_000, phase, det_phase, c0, evs);
                s.latest_arrival = s.latest_arrival.max(r.latest_arrival);
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
    for (kname, rel) in &kinds {
        let (mut fa, mut fl, mut runs) = (0u32, 0u32, 0usize);
        let mut least = u32::MAX;
        let mut arr_max = i64::MIN;
        let mut bad_d: Vec<i64> = Vec::new();
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
                    bad |= r.false_arms + r.false_latches > 0;
                    let last_complete = (r.end_day - 23_850).div_euclid(24_000);
                    for k in 1..=last_complete {
                        least = least.min(r.night_picks.get(&k).copied().unwrap_or(0));
                    }
                }
            }
            if bad {
                bad_d.push(d);
            }
        }
        total += fa + fl;
        arr_all = arr_all.max(arr_max);
        let bands = {
            // (contiguous d runs with a false arm or latch)
            let mut v: Vec<String> = Vec::new();
            let mut i = 0;
            while i < bad_d.len() {
                let mut j = i;
                while j + 1 < bad_d.len() && bad_d[j + 1] == bad_d[j] + step {
                    j += 1;
                }
                v.push(if i == j { format!("{}", bad_d[i]) } else { format!("{}..{}", bad_d[i], bad_d[j]) });
                i = j + 1;
            }
            if v.is_empty() { "none".to_string() } else { v.join(", ") }
        };
        lines.push(format!(
            "  {kname} at every d (0..24,000 step {step}; {runs} runs): false arms {fa}, false latches {fl} (at d {bands}); naps a night least {} of 24; the latest bed-stop arrival {}",
            if least == u32::MAX { "-".to_string() } else { least.to_string() },
            if arr_max == i64::MIN { "-".to_string() } else { arr_max.to_string() }
        ));
    }
    lines.insert(0, format!(
        "weather sweep (G1 Nov 12 02:43; the arm: {}; the latch: {}; L {}): every rain or thunder onset and end at every day time, the vanilla ramps (0.01/gt, thunder x rain), the detector on redsim's exact curve; bar: 0 false arms",
        o.arm.map_or("off".to_string(), |s| format!("falling 8 for {s} gt")),
        o.drop_min.map_or("off".to_string(), |k| format!("a fall >= {k} between readings")),
        o.day_inhibit.map_or("off".to_string(), |l| l.to_string())
    ));
    lines.push(format!("  => false arms + latches {total} ({})", if total == 0 { "pass" } else { "FAIL" }));
    lines.push(format!(
        "  => the latest bed-stop arrival over every weather kind and d {} (bar <= 23,900, ironspec 06:50: {})",
        if arr_all == i64::MIN { "-".to_string() } else { arr_all.to_string() },
        if arr_all <= 23_900 { "pass" } else { "FAIL" }
    ));
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
        // (ironspec 06:50: the latest bed-stop arrival over every scenario, bar <= 23,900)
        let mut worst_arrival = i64::MIN;
        let arr = |x: i64| if x == i64::MIN { "-".to_string() } else { x.to_string() };
        for (label, evs) in &scen {
            let s = scenario(o, evs);
            worst_arrival = worst_arrival.max(s.latest_arrival);
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
                "  {label}: the window opens at day {}..{} (night margin at the worst trip {:.1}%; the first napper {} gt {} the bar); naps a night (complete nights no skip cut) least {} of 24; picks whose napper reaches the bed before the stop takes it {} (of {} runs); trigger 2 accepted high {} (least {} gt inside), accepted low {} (least {} gt inside), rejected {} (least {} gt outside); FF longest {} gt ({} steps, {} held), overflows {}, absorbed {}; ring pulses at least {} gt apart; count errors {} of {}; arm accepts {} (false {}), latch sets {} (false {}); DAY off {} gt before the window opens; the latest bed-stop arrival {}",
                m(s.open_min), if s.open_max == i64::MIN { "-".to_string() } else { s.open_max.to_string() },
                night, early.abs(), if early >= 0 { "inside" } else { "BEFORE" },
                if s.min_night == u32::MAX { "-".to_string() } else { s.min_night.to_string() },
                s.daylight, s.runs,
                s.hi.0, m(s.hi.1), s.lo.0, m(s.lo.1), s.rej.0, m(s.rej.1),
                s.ff_max, s.ff_steps_max, s.held_max, s.ovf, s.absorbed, m(s.min_spacing), s.cerr_bad, s.cerr_n,
                s.arm_accepts, s.false_arms, s.latch_sets, s.false_latches, m(s.day_off_margin), arr(s.latest_arrival)
            ));
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
                for skip_day in (12_500..=20_500).step_by(250) {
                    // the weather from day 11,500 until the skip (a skip clears it)
                    let mut evs = Vec::new();
                    if let Some((r, th)) = w {
                        evs.push((4_000 + 11_500, wx(r, th)));
                    }
                    evs.push((4_000 + skip_day, Ev::Skip));
                    let s = scenario_with(o, &evs, 5, &[0, 13]);
                    arr_max = arr_max.max(s.latest_arrival);
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
                let night = (o.night_end - (omax + o.last_nap)) as f64 / o.night_len as f64 * 100.0;
                worst_night = worst_night.min(night);
                lines.push(format!(
                    "  a skip at every 250 gt of the window (12,500..20,500), {wname} from day 11,500 until the skip ({runs} runs: 20 release phases x 2 detector phases x 10 units phases x 33 skip times): picks reaching the bed early {dl} ({:.1}%; each held at the queue's NOT-DAY gate until dusk, ironspec 02:28: +10..21 golems a held napper); night picks inhibited {ni}; the least gap from a divert to the inhibit's last change {} (a divert in the skip's own update: the held case); the window opens at day {}..{} (night margin {:.1}%); gate margins least {} gt; false arms + latches {fa}; the latest bed-stop arrival {}",
                    dl as f64 / runs.max(1) as f64 * 100.0,
                    if eg == i64::MAX { "-".to_string() } else { eg.to_string() }, omin, omax, night, gate_min, arr(arr_max)
                ));
            }
            // a daytime thunderstorm skip (a day skip happens only in thunder): with no arm the gate
            // rejects its trigger 2 (C ~ 79), so the count stays a day's phase off until the next RISE
            let evs = vec![(4_000 + 3_000, wx(true, true)), (4_000 + 6_000, Ev::Skip)];
            let s = scenario_with(o, &evs, 5, &[0, 13]);
            worst_arrival = worst_arrival.max(s.latest_arrival);
            lines.push(format!(
                "  a daytime thunderstorm skip (6,000; the storm from 3,000): picks reaching the bed early {}; night picks inhibited {}; the window opens at day {}..{}; arm accepts {} (false {}), latch sets {} (false {}); the latest bed-stop arrival {}",
                s.daylight, s.night_inh, s.open_min, s.open_max, s.arm_accepts, s.false_arms, s.latch_sets, s.false_latches, arr(s.latest_arrival)
            ));
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
        lines.push(format!(
            "  => the latest bed-stop arrival the hatch opens for (DAY low at the arrival) over every scenario above (normal days, the scripted skips and weather, a skip at every 250 gt of the window in clear / rain / a thunderstorm, the daytime storm skip; the napper's lane offset 0 / {} / {} added; arrivals with DAY high wait at the shut hatch until dusk: the held nappers) {} (bar <= 23,900, ironspec 06:50: {})",
            o.launch_step, 2 * o.launch_step,
            arr(worst_arrival),
            if worst_arrival <= 23_900 { "pass" } else { "FAIL" }
        ));
    }
    lines
}
