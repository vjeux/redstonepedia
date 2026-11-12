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
//! update phases, every units phase at the trigger, normal days, skips and rain; the bars: the night
//! margin at the worst trip, the first napper's arrival against clear beds, trigger 2's gate margins,
//! the ring's pulse spacing, a count error after every re-phase.

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
    /// The detector in a clear day: RISE (0 -> 1) at `rise_day`, 0 again from `fall_day`, the
    /// 8-crossing rising at `eight_day` (and falling at 12,000 - it).
    pub rise_day: i64,
    pub fall_day: i64,
    pub eight_day: i64,
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
    /// Picks also need the daylight level < this (None: no inhibit); the level from a linear model
    /// (1 at `rise_day`, 8 at `eight_day`, mirrored at dusk), rounded at each detector update.
    /// Applied at the divert (the picked cart at the split's 4th route, `to_divert` gt after its
    /// release: the detector has updated since any skip by then), where the FIFO push also sits.
    pub day_inhibit: Option<i64>,
    pub to_divert: i64,
}

/// The linear daylight-level model (day time mod 24,000): 0 at night, 1 at RISE rising to 8 at the
/// 8-crossing, 8 or more by day, mirrored at dusk.
pub fn level(o: &CounterOpts, dm: i64) -> f64 {
    let ramp = (24_000 + o.eight_day - o.rise_day) as f64; // 1 -> 8 over this
    let dusk8 = 12_000 - o.eight_day;
    if dm >= o.eight_day && dm < dusk8 {
        8.0 + 7.0 * ((dm.min(12_000 - dm) - o.eight_day).max(0) as f64 / (6_000 - o.eight_day) as f64)
    } else if dm >= dusk8 && dm < o.fall_day {
        8.0 - 7.0 * (dm - dusk8) as f64 / ramp
    } else if dm >= o.fall_day && dm < o.rise_day {
        0.0
    } else {
        let since = if dm >= o.rise_day { dm - o.rise_day } else { dm + 24_000 - o.rise_day };
        1.0 + 7.0 * since as f64 / ramp
    }
}

impl Default for CounterOpts {
    fn default() -> Self {
        CounterOpts {
            variant: Variant::Ff,
            ff_period: 18,
            pend: true,
            window: (141, 221),
            gate: (136, 15),
            load1: 0,
            load2: 19,
            rise_day: 22_331,
            fall_day: 13_669,
            eight_day: 170,
            det_period: 20,
            to_bed: 120,
            bed_clear: 12_523,
            early_tol: 60,
            last_nap: 8_200,
            night_end: 23_850,
            night_len: 11_619,
            day_inhibit: None,
            to_divert: 60,
        }
    }
}

/// A scripted event: a night skip (at the event's game time, after the 100 gt asleep), a rain spell
/// from the event's game time for `len` gt (the level held under 8; its end a rising crossing when
/// the sky is bright), or weather darkening the level by `dark` for `len` gt (a skip clears it).
#[derive(Clone, Copy, Debug)]
pub enum Ev {
    Skip,
    Rain { len: i64 },
    Weather { len: i64, dark: i64 },
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
}

/// One run: `days` days from day time `d0` at game time 0, releases at game times = `phase` mod 100,
/// detector updates at game times = `det_phase` mod `det_period`, the counter starting at C = `c0`.
pub fn run(o: &CounterOpts, days: i64, d0: i64, phase: i64, det_phase: i64, c0: i64, evs: &[(i64, Ev)]) -> RunOut {
    let mut out = RunOut { min_spacing: i64::MAX, inhibit_edge_gap: i64::MAX, ..Default::default() };
    let horizon = days * 24_000;
    let mut day = d0;
    let (mut u, mut tens) = (c0.rem_euclid(10), c0.div_euclid(10));
    let c_of = |u: i64, tens: i64| tens * 10 + u;
    let (mut level_hi, mut eight_hi) = (true, false);
    let mut level_int = 0i64;
    let mut dark_until = -1i64;
    let mut dark = 0i64;
    let mut day_hi = false; // level >= the inhibit's L, as last updated
    let mut day_changed_t = -1_000i64;
    let mut diverts: Vec<i64> = Vec::new(); // pending divert decisions (game times)
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
    let mut rain_until = -1i64;
    let mut rephased_once = false;
    let mut last_pulse = -1_000i64;
    let mut pulse = |t: i64, out: &mut RunOut| {
        out.min_spacing = out.min_spacing.min(t - last_pulse);
        last_pulse = t;
    };
    for t in 0..horizon {
        while ev_i < evs.len() && evs[ev_i].0 <= t {
            match evs[ev_i].1 {
                Ev::Skip => {
                    day = (day.div_euclid(24_000) + 1) * 24_000;
                    // (sleeping through the night clears the weather)
                    dark_until = -1;
                    rain_until = -1;
                }
                Ev::Rain { len } => rain_until = t + len,
                Ev::Weather { len, dark: d } => {
                    dark_until = t + len;
                    dark = d;
                }
            }
            ev_i += 1;
        }
        let dm = day.rem_euclid(24_000);
        // the detector
        if (t - det_phase).rem_euclid(o.det_period) == 0 {
            let d = if t < dark_until { dark } else { 0 };
            level_int = (level(o, dm).round() as i64 - d).max(0);
            if let Some(l) = o.day_inhibit {
                let hi_now = level_int >= l;
                if hi_now != day_hi {
                    day_hi = hi_now;
                    day_changed_t = t;
                }
            }
            let hi = !(dm >= o.fall_day && dm < o.rise_day);
            let eight = dm >= o.eight_day && dm < 12_000 - o.eight_day && t >= rain_until;
            if hi && !level_hi {
                rise_pend = true;
            }
            if eight && !eight_hi {
                let c = c_of(u, tens);
                let acc = c >= o.gate.0 || c <= o.gate.1;
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
                } else if let Some(load) = rephase.take() {
                    out.count_errors.push(c_of(u, tens) - (load + releases_since));
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
            if let Some(load) = rephase.take() {
                out.count_errors.push(c_of(u, tens) - (load + releases_since));
            }
        }
        // the divert decisions due now (the inhibit read at the split; the napper at the bed's
        // entry `to_bed - to_divert` later)
        if diverts.first() == Some(&t) {
            diverts.remove(0);
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
            in_window = w;
            // (a pick: the window open at a pick phase (the window's first release and every 3rd:
            // lanes A/B/C at offsets 0/3/6 of the 10-cycle); decided at its divert)
            if w && rephased_once && [0, 3, 6].contains(&(c - o.window.0).rem_euclid(10)) {
                diverts.push(t + o.to_divert);
            }
        }
        day += 1;
    }
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
}

/// A scenario over release phases (every `phase_step`), the given detector phases and every units
/// phase at the start.
pub fn scenario(o: &CounterOpts, evs: &[(i64, Ev)]) -> ScenOut {
    scenario_with(o, evs, 1, &[0, 7, 13, 19])
}

pub fn scenario_with(o: &CounterOpts, evs: &[(i64, Ev)], phase_step: usize, det_phases: &[i64]) -> ScenOut {
    let mut s = ScenOut { open_min: i64::MAX, open_max: i64::MIN, hi: (0, i64::MAX), lo: (0, i64::MAX), rej: (0, i64::MAX), min_spacing: i64::MAX, edge_gap: i64::MAX, ..Default::default() };
    for phase in (0..100).step_by(phase_step) {
        for &det_phase in det_phases {
            for c0 in 100i64..110 {
                s.runs += 1;
                let r = run(o, 3, 20_000, phase, det_phase, c0, evs);
                s.edge_gap = s.edge_gap.min(r.inhibit_edge_gap);
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

/// The bench: both variants, the scripted scenarios, ironspec's bars per variant.
pub fn bench(base: &CounterOpts) -> Vec<String> {
    let mut lines = Vec::new();
    // day 2's day time X is game time 4,000 + X (the runs start at day 20,000, before RISE)
    let scen: Vec<(&str, Vec<(i64, Ev)>)> = vec![
        ("normal days", vec![]),
        ("a bed-time clear skip (12,623)", vec![(4_000 + 12_623, Ev::Skip)]),
        ("a bed-time rain skip (12,102)", vec![(4_000 + 12_102, Ev::Skip)]),
        ("a skip at 16,000", vec![(4_000 + 16_000, Ev::Skip)]),
        ("the latest clear skip (23,585)", vec![(4_000 + 23_585, Ev::Skip)]),
        ("rain 4,000..6,000 (a mid-day clearing)", vec![(4_000 + 4_000, Ev::Rain { len: 2_000 })]),
        ("rain 9,000..11,800 (a dusk clearing)", vec![(4_000 + 9_000, Ev::Rain { len: 2_800 })]),
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
        for (label, evs) in &scen {
            let s = scenario(o, evs);
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
                "  {label}: the window opens at day {}..{} (night margin at the worst trip {:.1}%; the first napper {} gt {} the bar); picks whose napper reaches the bed before the stop takes it {} (of {} runs); trigger 2 accepted high {} (least {} gt inside), accepted low {} (least {} gt inside), rejected {} (least {} gt outside); FF longest {} gt ({} steps, {} held), overflows {}, absorbed {}; ring pulses at least {} gt apart; count errors {} of {}",
                m(s.open_min), if s.open_max == i64::MIN { "-".to_string() } else { s.open_max.to_string() },
                night, early.abs(), if early >= 0 { "inside" } else { "BEFORE" },
                s.daylight, s.runs,
                s.hi.0, m(s.hi.1), s.lo.0, m(s.lo.1), s.rej.0, m(s.rej.1),
                s.ff_max, s.ff_steps_max, s.held_max, s.ovf, s.absorbed, m(s.min_spacing), s.cerr_bad, s.cerr_n
            ));
            daylight_total += s.daylight;
            night_inh_total += s.night_inh;
        }
        // (ironspec 01:38: every skip time inside the window x weather; the FF variant only)
        if o.variant == Variant::Ff {
            for dark in [0i64, 3, 5] {
                let (mut dl, mut ni, mut eg, mut runs) = (0u32, 0u32, i64::MAX, 0usize);
                let (mut omin, mut omax) = (i64::MAX, i64::MIN);
                let mut gate_min = i64::MAX;
                for skip_day in (12_500..=20_500).step_by(250) {
                    // weather from day 11,500 until the skip (a skip clears it)
                    let evs = vec![(4_000 + 11_500, Ev::Weather { len: skip_day - 11_500, dark }), (4_000 + skip_day, Ev::Skip)];
                    let s = scenario_with(o, &evs, 5, &[0, 13]);
                    dl += s.daylight;
                    ni += s.night_inh;
                    eg = eg.min(s.edge_gap);
                    runs += s.runs;
                    omin = omin.min(s.open_min);
                    omax = omax.max(s.open_max);
                    for g in [s.hi.1, s.lo.1, s.rej.1] {
                        gate_min = gate_min.min(g);
                    }
                }
                daylight_total += dl;
                night_inh_total += ni;
                worst_gate = worst_gate.min(gate_min);
                let night = (o.night_end - (omax + o.last_nap)) as f64 / o.night_len as f64 * 100.0;
                worst_night = worst_night.min(night);
                lines.push(format!(
                    "  a skip at every 250 gt of the window (12,500..20,500), weather darkening {dark} until the skip ({runs} runs: 20 release phases x 2 detector phases x 10 units phases x 33 skip times): picks reaching the bed early {dl}; night picks inhibited {ni}; the least gap from a divert to the inhibit's last change {}; the window opens at day {}..{} (night margin {:.1}%); gate margins least {} gt",
                    if eg == i64::MAX { "-".to_string() } else { eg.to_string() }, omin, omax, night, gate_min
                ));
                if o.day_inhibit.is_some() {
                    worst_edge = worst_edge.min(eg);
                }
            }
            // a daytime thunderstorm skip (players may sleep in thunder by day): the gate rejects its
            // trigger 2 (C ~ 79), so the count stays a day's phase off until the next RISE
            let evs = vec![(4_000 + 3_000, Ev::Weather { len: 3_000, dark: 5 }), (4_000 + 6_000, Ev::Skip)];
            let s = scenario_with(o, &evs, 5, &[0, 13]);
            lines.push(format!(
                "  a daytime thunderstorm skip (6,000): picks reaching the bed early {}; night picks inhibited {}; the window opens at day {}..{}",
                s.daylight, s.night_inh, s.open_min, s.open_max
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
            "  => bars: night margin {:.1}% ({}); the first napper's arrival {} gt ({}); picks before the beds take them {} ({}); {}gate margins >= 1 release: least {} gt ({}); ring pulses >= {} gt apart: least {} ({}); count errors {} ({})",
            worst_night, if pass_night { "pass" } else { "FAIL" },
            worst_early, if pass_early { "pass" } else { "FAIL" },
            daylight_total, if daylight_total == 0 { "pass" } else { "FAIL" },
            match o.day_inhibit {
                Some(l) => format!(
                    "night picks the level < {l} inhibit removed {} ({}); the inhibit's edge >= 3 gt + 1 tick from a divert: least {} ({}); ",
                    night_inh_total, if night_inh_total == 0 { "pass" } else { "FAIL" },
                    if worst_edge == i64::MAX { "-".to_string() } else { worst_edge.to_string() }, if pass_edge { "pass" } else { "FAIL" }
                ),
                None => String::new(),
            },
            worst_gate, if pass_gate { "pass" } else { "FAIL" },
            o.ff_period, worst_spacing, if pass_spacing { "pass" } else { "FAIL" },
            bad_counts, if pass_counts { "pass" } else { "FAIL" }
        ));
    }
    lines
}
