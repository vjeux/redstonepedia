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
    /// (ironspec 12:54) The stale-hold back-pressure: no pick while the stop is held AND NOT REST_l (a
    /// stale napper on the stop's ledge outside REST) either.
    pub stale_backpressure: bool,
    /// (ironspec 12:13 / 12:23, day-1's option 2) The per-lane away latch: no pick from a lane while its
    /// napper is away (set at the pick, reset when it is back in its lane past K).
    pub lane_latch: bool,
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
    /// (G1's Y7 on f21c9f600) R -> the returning napper's last tick within 16 of the bed head (its climb out of the
    /// sphere), the exposure's end point: ironspec 09:03's estimate 53, pending the composed run-out (printed with the
    /// exposure).
    pub climb_exit: i64,
    /// (G1's Y7 on 10bcdf661, a CONTROL only) A planted gate defect: the stop is freed at the occupant's lie-down instead
    /// of its R's RS. The exposure, judged from the occupant's own R, must then see second nappers.
    pub gate_at_liedown: bool,
    /// The high re-phase (ironspec 09:49): (H, the C loaded, the window lo, hi): a rise of the detector's
    /// level to H (reached only around a clear midday, so weather only delays or prevents it) while C is
    /// outside [lo, hi] (the span a natural H rise can fall in) can only follow an undetected skip; it
    /// loads C at the next release, as trigger 2 does (the load never ahead of the clear clock).
    pub high: Option<(i64, i64, i64, i64)>,
    /// The H path's block delay: a load request due at a detector update reaches the pending latch this
    /// many gt later (`ctl-high-bench`: HI, E_H and the AND), so a release sooner does not take it.
    pub high_lag: i64,
    /// (ironspec 15:57: day-1's (i)) The back-pressure's HELD sampled at the divert (`to_divert` gt after
    /// the pick, where the divert hold samples DAY), not at the pick: a picked cart whose divert finds a
    /// napper held at the queue rides on in its lane (the slot lost, counted as deferred).
    pub held_at_divert: bool,
    /// The queue's release to the stop's entry (gt; b8949c96's way-in: 110 before (C)): the stop is held
    /// from the entry for its cycle, and the next release waits for it.
    pub to_entry: i64,
    /// (G1 on 6b9be93ea point 1, ironspec 20:55 PT (2)(1); the chain audit: through the classifier) The release bars'
    /// positive control: at this day time on day 1 a napper is put at the queue and the queue's release inputs (REST_l
    /// AND the DAY the hatch sees low) are forced true for its release only; its entry is classed by the model's own
    /// classifier, which both bars must count.
    pub inject_entry: Option<i64>,
    /// (ironspec 23:06 / 23:21 PT) The stop's cycle (entry -> R) drawn per visit from this measured distribution
    /// (58f770cc's rows) instead of the fixed `stop_cycle`.
    pub stop_hist: Option<Vec<(i64, u32)>>,
    /// (ironspec 23:21 / 23:37 PT, G1 on 9c081f615 (b)) The measured round trip, the pick -> back in its lane, per
    /// visit (onechunk's record7d rows), instead of the entry + `back_after`; never before the visit's R.
    pub back_hist: Option<Vec<(i64, u32)>>,
    /// (ironspec 23:33 PT (2)) One late return: the first release at or after this game time gets its stop
    /// cycle (and its return) this many gt longer.
    pub inject_late: Option<(i64, i64)>,
    /// (ironspec 00:12 PT (2), (B')) A per-lane count: a lane picks at most this many times in a window (None: every
    /// opportunity picks).
    pub lane_count: Option<u32>,
    /// (ironspec 00:12 PT (2)) One forced skipped pick: the first pick opportunity at or after this game time that
    /// would have picked is skipped as if its lane's napper were away (an opportunity that would not have picked
    /// anyway is left alone: the override must change its target, ironspec 04:07 PT).
    pub skip_at: Option<i64>,
    /// (ironspec 06:31 PT, the exposure rule's control) The queue's release ignores the stop's RS (R's ready): a
    /// release while the bed is occupied puts a second napper within 16 of it, which the exposure bar must count.
    pub ignore_stop: bool,
    /// (the adopted stop) Every visit's entry -> R (and a held stop's cycle) this much longer than the measured bands
    /// ([`RELEASE_SHIFT`]).
    pub release_shift: i64,
    /// (the adopted stop) A skip-woken sleeper's wake -> R ([`WAKE_TO_R`]).
    pub wake_to_r: i64,
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
            stale_hold: 0,
            backpressure: true,
            // (ironspec 12:54 added the stale-hold back-pressure, 13:51 dropped it: the lane latch covers it; the chain
            // audit: its bar (no pick while a stale napper holds the stop outside REST) is judged in every run either way)
            stale_backpressure: false,
            lane_latch: true,
            day_rise_load: None,
            back_after: 1_000,
            // (the chain audit: the cited stop cycle 260, walk 120 and nap 30, not 300 / 80 / 100)
            stop_cycle: 260,
            stop_walk: 120,
            stop_nap: 30,
            climb_exit: 53,
            gate_at_liedown: false,
            high: None,
            high_lag: 6,
            held_at_divert: false,
            to_entry: 0,
            inject_entry: None,
            stop_hist: None,
            back_hist: None,
            inject_late: None,
            lane_count: None,
            skip_at: None,
            ignore_stop: false,
            release_shift: RELEASE_SHIFT,
            wake_to_r: WAKE_TO_R,
        }
    }
}

/// (ironspec on f6e715e50 (a): the counter's stop moves to the adopted logic) The adopted stop's release against the
/// measured bands' (58f770cc's rows released the park at about the wake + 8, close 4 / delta 8): ctl-shutter's
/// sustained-presence release puts the park at the wake + 22 (RLS + 2, the trim), so every visit's entry -> R draws
/// this much longer.
pub const RELEASE_SHIFT: i64 = 14;
/// The adopted stop's wake -> R (ctl-shutter 17:24: the departure at the wake + 38..44, R 13..15 after it; the worst):
/// a skip-woken sleeper (class a) leaves through R this long after REST's end wakes it.
pub const WAKE_TO_R: i64 = 59;

/// A scripted event: a night skip (at the event's game time, after the 100 gt asleep: the day jumps
/// to the next 24,000 and the weather flags clear), or the weather flags set (vanilla `raining` /
/// `thundering`; the levels ramp 0.01 a gt toward them, thunder darkening only times rain).
#[derive(Clone, Copy, Debug)]
pub enum Ev {
    Skip,
    Wx { rain: bool, thunder: bool },
}

/// (G1 on 6b9be93ea point 1) An entry into the stop, classed for the bars: in REST; in a skip's stale window
/// (outside REST right after a skip: it waits on the ledge, counted apart); its walk outlasting REST's end
/// (counted apart); outside REST with no skip before it (a violation the bars count).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryClass {
    InRest,
    SkipStale,
    WalkPastRest,
    OutsideRest,
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
    /// (G1's Y17) Drawn pick -> back times shorter than the entry and the stop's cycle (floored to 1; bar 0).
    pub back_clamped: u32,
    /// (G1's Y9) 1 when a skip's DAY rise was still pending at the run's end.
    pub skip_unresolved: u32,
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
    /// The release model (`rest`): each entry into the stop as (the absolute day at the stop's entry, the
    /// gt it waited at the queue, its class).
    pub entries: Vec<(i64, i64, EntryClass)>,
    /// Stale admissions (ironspec 09:31: each waits on the stop's ledge until REST, naps, leaves): (the
    /// entry's day time, the day time the stop clears).
    pub stale_admits: Vec<(i64, i64)>,
    /// Picks the back-pressure deferred (a napper held at the queue).
    pub deferred: u32,
    /// (ironspec 12:54) Picks made while the stop held a stale napper outside REST (bar 0 under the
    /// stale-hold back-pressure).
    pub stale_picks: u32,
    /// Picks the per-lane away latch skipped (its lane's napper still away).
    pub lane_skips: u32,
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
    /// (G1 on 6ca670551) The latest entry made in REST, as night time (day time, + 24,000 after midnight).
    pub rest_entry_max: i64,
    /// (ironspec 12:13) The most riders away from one lane at once, at a release; the longest pick ->
    /// stop entry.
    pub lane_away_max: u32,
    pub pick_entry_max: i64,
    /// (ironspec 23:21 PT) The away latch's least margin: a lane's pick against its previous napper's return to the
    /// lane (gt; i64::MAX: none measured; a latch still set at the pick is a skipped pick, `lane_skips`).
    pub latch_margin_min: i64,
    /// The injected late return's release (game time), when one was made.
    pub late_at: Option<i64>,
    /// (G1 on 7897f4ba7 (1)) The forced skipped pick's release (game time), when one was made.
    pub skipped_at: Option<i64>,
    /// (G1 on 7897f4ba7 (1)) Skips that found the stop's last occupant already gone (after its R): gt since its entry.
    pub skip_empty: Vec<i64>,
    /// (ironspec 06:31 PT; G1's Y7) The exposure: lie-downs before the previous occupant's own climb out of the bed's
    /// sphere (bar 0), and the least margin (this lie-down - that climb out).
    pub exposure: u32,
    pub exposure_margin_min: i64,
    /// (the chain audit, hole 23) Each skip's stale window as measured: the gt from the skip to the hatch seeing DAY high
    /// (the first detector update reading DAY high, + 1, + the hatch's lag); an entry classed SkipStale lies inside its
    /// skip's window by construction.
    pub skip_windows: Vec<i64>,
    /// The injected entry's class (the classifier's own), when one was made.
    pub injected_class: Option<EntryClass>,
    /// Every pick's game time (the forced skip's test: the run without it picked at the skipped tick).
    pub pick_ts: Vec<i64>,
}

/// (G1 on 6b9be93ea point 1; the chain audit) An entry's class for the bars: outside REST inside its skip's measured
/// stale window is SkipStale; outside REST otherwise is OutsideRest (a violation); in REST with the walk reaching REST's
/// end (lying down at or after it) is WalkPastRest; else InRest.
pub fn entry_class(outside: bool, walk_past: bool, in_skip: bool) -> EntryClass {
    if outside && in_skip {
        EntryClass::SkipStale
    } else if outside {
        EntryClass::OutsideRest
    } else if walk_past {
        EntryClass::WalkPastRest
    } else {
        EntryClass::InRest
    }
}

/// (G1's audit hole 24) The latest-entry bar, consistent with the walk-past class: an entry lies down in REST only if its
/// walk ends strictly before REST's end (24,010), so the latest entry must be < 24,010 - the walk.
pub fn rest_entry_ok(rest_entry_max: i64, stop_walk: i64) -> bool {
    rest_entry_max + stop_walk < 24_010
}

/// One run: `days` days from day time `d0` at game time 0, releases at game times = `phase` mod 100,
/// detector updates at game times = `det_phase` mod `det_period`, the counter starting at C = `c0`.
pub fn run(o: &CounterOpts, days: i64, d0: i64, phase: i64, det_phase: i64, c0: i64, evs: &[(i64, Ev)]) -> RunOut {
    let mut out = RunOut { min_spacing: i64::MAX, inhibit_edge_gap: i64::MAX, latest_arrival: i64::MIN, stale_dt: (i64::MAX, i64::MIN), exit_gap_min: i64::MAX, liedown_gap_min: i64::MAX, rest_entry_max: i64::MIN, latch_margin_min: i64::MAX, exposure_margin_min: i64::MAX, ..Default::default() };
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
    let mut diverts: Vec<(i64, i64, usize, i64)> = Vec::new(); // pending divert decisions (game time, the lane's launch offset, the lane, the pick's game time)
    let mut arrivals: Vec<(i64, usize, i64)> = Vec::new(); // pending bed-stop arrivals (game time, the lane, the pick's game time)
    let mut skip_pending: Option<i64> = None; // a skip whose DAY rise is not yet seen
    // (the chain audit, hole 23: the last skip's stale window's end, measured: the hatch seeing DAY high)
    let mut skip_window_end: Option<i64> = None;
    // (the release bars' control: the injected napper's release inputs forced, once)
    let mut force_release = false;
    let mut held_at_hatch = 0u32; // nappers waiting at the shut hatch
    // (the release model: arrivals waiting at the queue (game times), REST_l, the riders away: picked
    // and not yet entered, then back `back_after` after the entry)
    let mut queue: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
    let mut rest_l = false;
    let mut stop_until = i64::MIN; // the stop is held until then (R's RS)
    // (G1's Y7) the current occupant's own climb out of the bed's 16-block sphere: its own R + `climb_exit`
    let mut occ_climb_out = i64::MIN;
    // (G1's Y9) the skip's excuse, while its DAY rise is pending, is bounded by the worst DAY-on lag the detector's own
    // curve allows after a skip (a skip lands at day time 0): the first day time at which the level reaches L, the
    // worst of clear, rain and thunder, + the detector's period (its update phase) + the DAY line's output lag (G1's Z7);
    // cited from `det_level` through [`skip_on_lag`]; a curve that never reaches L gives NO excuse (fail closed; the
    // report fails it NOT JUDGED)
    let skip_lag_bound = o.day_inhibit.map_or(0, |l| {
        let lags = skip_on_lag(l);
        if lags.iter().any(|x| x.1.is_none()) { 0 } else { lags.iter().filter_map(|x| x.1).max().unwrap_or(0) + o.det_period + DAY_OUT_LAG }
    });
    // (each lane's last napper: (its pick, its return to the lane), for the away latch's margin)
    let mut last_back: [Option<(i64, i64)>; 3] = [None; 3];
    // ((B') each lane's picks this window; the forced skip once)
    let mut lane_picks_w = [0u32; 3];
    let mut skip_used = false;
    let mut in_flight = 0u32;
    let mut back_at: Vec<i64> = Vec::new();
    // (ironspec 12:13: the riders away per lane: picked and not yet entered, then back `back_after`
    // after the entry; the queue's lanes and pick times beside its arrival times)
    let mut in_flight_lane = [0u32; 3];
    let mut back_lane: Vec<(i64, usize)> = Vec::new();
    let mut queue_meta: std::collections::VecDeque<(usize, i64)> = std::collections::VecDeque::new();
    // (ironspec 12:54: the stop held by a stale napper until then)
    let mut stale_until = i64::MIN;
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
            skip_window_end = None;
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
                    stop_until = t + 12_000 + o.stop_cycle + o.release_shift;
                    // (G1's Y7: the occupant's own exit moves with its own wake, recorded apart from the gate)
                    occ_climb_out = stop_until + o.climb_exit;
                    b'b'
                } else if since < o.stop_walk + o.stop_nap {
                    // (G1: the sleeper wakes at REST's end, day time 10, `rest_tail` gt after the skip; the adopted stop's
                    // sustained-presence release takes it out through R `wake_to_r` later: ctl-shutter)
                    stop_until = t + rest_tail + o.wake_to_r;
                    occ_climb_out = stop_until + o.climb_exit;
                    b'a'
                } else {
                    b'n'
                };
                out.skip_classes.push((since, class));
                // (the occupant's exit moved)
                last_exit = Some(stop_until);
            } else if let Some(te) = occupant {
                // (G1 on 7897f4ba7 (1): the stop's last occupant already left through R: the skip finds the stop empty)
                out.skip_empty.push(t - te);
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
                // (58f770cc: the gt from a skip to the first detector update reading DAY high; the chain audit, hole 23:
                // the skip's stale window ends when the hatch sees that update, + 1 + the hatch's lag; G1's Z7 on 10bcdf661:
                // the hatch reads the DAY line's OUTPUT, DAY_OUT_LAG after the detector's update)
                if hi_now && let Some(ts) = skip_pending.take() {
                    out.skip_lags.push(t - ts);
                    out.skip_windows.push(t + 1 + DAY_OUT_LAG + o.hatch_lag - ts);
                    skip_window_end = Some(t + 1 + DAY_OUT_LAG + o.hatch_lag);
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
            let (_, lane_off, lane_d, pick_t) = diverts.remove(0);
            let at_bed = dm + (o.to_bed - o.to_divert);
            let early = (at_bed.rem_euclid(24_000)) < o.bed_clear - o.early_tol;
            let day_inh = o.day_inhibit.is_some() && day_hi;
            // ((i): HELD sampled here: a napper held at the queue keeps this cart in its lane)
            let held = o.held_at_divert && o.rest.is_some() && o.backpressure && !queue.is_empty();
            if held {
                out.deferred += 1;
            }
            let inhibited = day_inh || held;
            if inhibited && o.rest.is_some() {
                // (not diverted: the napper rides on in its cart, back in its lane)
                in_flight = in_flight.saturating_sub(1);
                in_flight_lane[lane_d] = in_flight_lane[lane_d].saturating_sub(1);
            }
            if o.day_inhibit.is_some() {
                out.inhibit_edge_gap = out.inhibit_edge_gap.min(t - day_changed_t);
            }
            if day_inh && !early {
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
                arrivals.push((t + (o.to_bed - o.to_divert) + lane_off, lane_d, pick_t));
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
            // (the release bars' positive control, planted in the model's input (the chain audit): at day time X of day
            // 1 a napper is put at the queue and its release's inputs (REST_l, the DAY the hatch sees) forced for that
            // release only; its entry is classed below by the same classifier as every other)
            if let Some(x) = o.inject_entry
                && day.div_euclid(24_000) == 1
                && dm_now == x
            {
                queue.push_back(t);
                queue_meta.push_back((0, t));
                in_flight += 1;
                in_flight_lane[0] += 1;
                force_release = true;
            }
            // (the release model: every arrival waits at the queue; the head is released once it has
            // waited `stale_hold` AND REST_l AND the DAY the hatch sees is low)
            while let Some(i) = arrivals.iter().position(|&(a, _, _)| a == t) {
                let (_, ln, tp) = arrivals.remove(i);
                queue.push_back(t);
                queue_meta.push_back((ln, tp));
                out.queue_max = out.queue_max.max(queue.len() as u32);
            }
            let forced_now = force_release && queue.len() == 1;
            if let Some(&ta) = queue.front()
                && t >= ta + o.stale_hold
                && ((rest_l && !hatch_sees_hi) || forced_now)
                && (t >= stop_until || o.ignore_stop)
            {
                queue.pop_front();
                // (ironspec 06:31 PT: "a release while the bed is occupied: a second napper within 16 of it"; G1's Y7: judged
                // from the OCCUPANTS' OWN timelines, never the release gate's state: this napper inside the bed's 16-block
                // sphere from its stop entry (the release + `to_entry`, the way-in's measured release -> entry; 9fc3155f:
                // the positions within 16 of the head lie only on the stop route) against the previous occupant's own R +
                // its climb out of the sphere (`climb_exit`); G1's Z9 on 10bcdf661: no uncited lie-down offset)
                let my_in = t + o.to_entry;
                if occ_climb_out != i64::MIN {
                    out.exposure_margin_min = out.exposure_margin_min.min(my_in - occ_climb_out);
                    if my_in < occ_climb_out {
                        out.exposure += 1;
                    }
                }
                // (G1 on 6ca670551: every entry classed by its lie-down against REST's end, whatever the
                // cause: REST runs from day time 12,000 to 24,010 (day time 10 of the next day; after a skip
                // to day time 10); an entry outside REST (10..11,999), or one whose walk outlasts REST's end,
                // is on foot when REST ends: it waits on the stop's ledge for the next REST, holding the stop)
                let rest_end = if dm_now >= 12_000 {
                    t + (24_010 - dm_now)
                } else if dm_now < 10 {
                    t + (10 - dm_now)
                } else {
                    t
                };
                // (G1 on 6b9be93ea point 1, ironspec 20:55 PT (2)(1): the physical class (an entry outside REST, or
                // one whose walk outlasts REST's end, waits on the stop's ledge) is kept for the stop's model;
                // the bars' class keeps a skip's stale window apart from an entry outside REST with no skip
                // before it, which is a violation the bars count; the class is keyed on the stop's entry
                // (the release + `to_entry`, G1's note on dd1c4270f), not the release)
                let dm_entry = (day + o.to_entry).rem_euclid(24_000);
                let outside = (10..12_000).contains(&dm_entry);
                let walk_past = !outside && t + o.to_entry + o.stop_walk >= rest_end;
                let stale = outside || walk_past;
                // (the chain audit, hole 23: the skip's stale window as measured, from the skip to the hatch seeing DAY
                // high (`skip_window_end`), never an unsourced 200 gt; with no DAY line there is no excuse; G1's Y9: while
                // the DAY rise is still pending the excuse is BOUNDED by the curve's worst DAY-on lag after a skip + the
                // detector's period (`skip_lag_bound`) + 1 + the hatch's lag)
                let in_skip = o.day_inhibit.is_some() && last_skip_t >= 0 && t >= last_skip_t && ((skip_pending.is_some() && t < last_skip_t + skip_lag_bound + 1 + o.hatch_lag) || skip_window_end.is_some_and(|e| t < e));
                let class = entry_class(outside, walk_past, in_skip);
                if forced_now {
                    out.injected_class = Some(class);
                    force_release = false;
                }
                // (the latest entry made in REST, not in a skip's stale window: those are the stale class; G1 on
                // 12c106916: the stop's entry is the release + `to_entry`, so that is the time recorded)
                if (dm_now >= 12_000 || dm_now < 10) && !in_skip {
                    out.rest_entry_max = out.rest_entry_max.max(if dm_now < 10 { dm_now + 24_000 } else { dm_now } + o.to_entry);
                }
                out.entries.push((day + o.to_entry, t - ta, class));
                if in_skip {
                    out.stale_entries += 1;
                }
                in_flight = in_flight.saturating_sub(1);
                // (G1's note on 7ccd7737c: the queue's metadata is in step with the queue by construction; a
                // missing entry is a model error, never a default)
                let (ln, tp) = queue_meta.pop_front().expect("ctl-counter: the queue's metadata out of step with the queue");
                in_flight_lane[ln] = in_flight_lane[ln].saturating_sub(1);
                out.pick_entry_max = out.pick_entry_max.max(t - tp);
                occupant = Some(t);
                if stale {
                    // (it waits on the stop's ledge until the next REST, naps, leaves: the stop is held until
                    // then; its nap is that evening's)
                    let wait = (12_000 - dm_now).rem_euclid(24_000);
                    // (G1's Y7: the occupant's own R, recorded apart from the gate's hold; the gate frees the stop at
                    // R's RS, unless the planted gate defect `gate_at_liedown` frees it at the lie-down)
                    let own_r = t + wait + o.stop_cycle + o.release_shift;
                    occ_climb_out = own_r + o.climb_exit;
                    stop_until = if o.gate_at_liedown { t + o.to_entry + o.stop_walk } else { own_r };
                    stale_until = own_r;
                    out.stale_admits.push((dm_now, (dm_now + wait + o.stop_cycle + o.release_shift).rem_euclid(24_000)));
                    // (G1's audit hole 11: back in its lane from the measured round trip's R -> back part when `--back-hist`
                    // is given (the pick -> back less the entry and the cycle), else `back_after`)
                    let back = match &o.back_hist {
                        Some(h) => {
                            // (G1's Y17: a drawn pick -> back shorter than the entry and the cycle is a data inconsistency,
                            // counted (a failure in the bars), never silently floored)
                            let raw = crate::napper::hist_pick(h, (t as u64) ^ 0x5bd1_e995) - o.to_entry - o.stop_cycle;
                            out.back_clamped += u32::from(raw < 1);
                            own_r + raw.max(1)
                        }
                        None => own_r + o.back_after,
                    };
                    back_at.push(back);
                    back_lane.push((back, ln));
                    *out.night_naps.entry((day + wait).div_euclid(24_000)).or_default() += 1;
                } else {
                    // (ironspec 23:06 / 23:21 PT: the visit's entry -> R per visit from the measured distribution;
                    // ironspec 23:33 PT (2): one late return, the injected visit's cycle `add` longer)
                    let base_cyc = o.stop_hist.as_ref().map_or(o.stop_cycle, |h| crate::napper::hist_pick(h, t as u64)) + o.release_shift;
                    let mut cyc = base_cyc;
                    if let Some((at, add)) = o.inject_late
                        && out.late_at.is_none()
                        && t >= at
                    {
                        cyc += add;
                        out.late_at = Some(t);
                    }
                    // (G1's Y7: the occupant's own R from its own visit; the gate frees the stop at R's RS, unless the
                    // planted gate defect frees it at the lie-down)
                    let own_r = t + o.to_entry + cyc;
                    occ_climb_out = own_r + o.climb_exit;
                    stop_until = if o.gate_at_liedown { t + o.to_entry + o.stop_walk } else { own_r };
                    // (G1 on 9c081f615 (b): back in its lane from the measured round trip, the pick -> back, per
                    // visit, never before this visit's R; a late return comes back that much later)
                    let back = match &o.back_hist {
                        Some(h) => (tp + crate::napper::hist_pick(h, (t as u64) ^ 0x5bd1_e995) + o.release_shift).max(t + o.to_entry + base_cyc + 1) + (cyc - base_cyc),
                        None => t + o.to_entry + o.back_after + (cyc - base_cyc),
                    };
                    back_at.push(back);
                    back_lane.push((back, ln));
                    last_back[ln] = Some((tp, back));
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
                let liedown = if stale { t + (12_000 - dm_now).rem_euclid(24_000) } else { t + o.to_entry + o.stop_walk };
                if let Some(le) = last_exit {
                    out.exit_gap_min = out.exit_gap_min.min(stop_until - le);
                    out.liedown_gap_min = out.liedown_gap_min.min(liedown - le);
                }
                last_exit = Some(stop_until);
            }
        }
        while o.rest.is_none()
            && let Some(i) = arrivals.iter().position(|&(a, _, _)| a == t)
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
                back_lane.retain(|&(b, _)| b > t);
                for (l, &f) in in_flight_lane.iter().enumerate() {
                    let away = f + back_lane.iter().filter(|&&(_, x)| x == l).count() as u32;
                    out.lane_away_max = out.lane_away_max.max(away);
                }
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
                lane_picks_w = [0; 3];
            }
            in_window = w;
            // (a pick: the window open at a pick phase (the window's first release and every 3rd:
            // lanes A/B/C at offsets 0/3/6 of the 10-cycle); decided at its divert)
            if w && rephased_once && let Some(lane) = [0i64, 3, 6].iter().position(|&x| x == (c - o.window.0).rem_euclid(10)) {
                // ((B'): a lane past its count picks no more this window)
                let counted_out = o.lane_count.is_some_and(|n| lane_picks_w[lane] >= n);
                let stale_held = o.stale_backpressure && t < stop_until && !rest_l;
                let deferred_now = o.rest.is_some() && o.backpressure && ((!o.held_at_divert && in_flight > 0) || stale_held);
                let natural_held = o.lane_latch && (in_flight_lane[lane] > 0 || back_lane.iter().any(|&(b, x)| x == lane && b > t));
                // (one forced skipped pick, as if the lane's napper were away: counted with the latch's skips; ironspec
                // 04:07 PT: only an opportunity that would have picked takes it, so the override always changes its target)
                let would_pick = !counted_out && !deferred_now && !(o.rest.is_some() && natural_held);
                let forced = would_pick && o.rest.is_some() && o.skip_at.is_some_and(|at| t >= at) && !skip_used;
                if forced {
                    skip_used = true;
                    out.skipped_at = Some(t);
                }
                let lane_held = forced || natural_held;
                // (ironspec 23:21 PT: the away latch's margin, this pick against the return of the lane's previous napper;
                // negative: still away. The chain audit, hole 12: no tolerance window around one lane period: every pick
                // with a previous napper in its lane is measured)
                if let Some((_, b)) = last_back[lane] {
                    out.latch_margin_min = out.latch_margin_min.min(t - b);
                }
                // (ironspec 22:05 PT, the one-in-flight contract (b): HELD set at the pick and reset at the queue's
                // departure: no pick while any napper is picked and not yet released (`in_flight`, raised at the
                // pick, lowered at the release or a divert that keeps the cart in its lane); with (i), HELD is
                // instead sampled at the divert, below)
                if counted_out {
                    // ((B'): the lane's count reached: no pick, not a skip)
                } else if o.rest.is_some() && o.backpressure && ((!o.held_at_divert && in_flight > 0) || stale_held) {
                    // (G1's back-pressure; ironspec 12:54: nor while the stop is held outside REST)
                    out.deferred += 1;
                } else if o.rest.is_some() && lane_held {
                    // (the per-lane away latch: this lane's napper is still away)
                    out.lane_skips += 1;
                } else {
                    lane_picks_w[lane] += 1;
                    out.pick_ts.push(t);
                    if o.rest.is_some() && t < stale_until && !rest_l {
                        out.stale_picks += 1;
                    }
                    diverts.push((t + o.to_divert, lane as i64 * o.launch_step, lane, t));
                    if o.rest.is_some() {
                        in_flight += 1;
                        in_flight_lane[lane] += 1;
                        // (ironspec 12:54: riders away per lane at every instant: a lane's count rises only
                        // at its picks, so its maximum is read right after each)
                        back_lane.retain(|&(b, _)| b > t);
                        let away = in_flight_lane[lane] + back_lane.iter().filter(|&&(_, x)| x == lane).count() as u32;
                        out.lane_away_max = out.lane_away_max.max(away);
                    }
                }
            }
        }
        day += 1;
    }
    out.end_day = day;
    // (G1's Y9) a skip whose DAY rise was never seen by the run's end: its excuse can't be closed, so it is counted (a
    // failure in the bars)
    out.skip_unresolved = u32::from(skip_pending.is_some());
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
    /// (G1 on 6b9be93ea point 1) Entries counted apart from the bars: in a skip's stale window, or with the
    /// walk outlasting REST's end.
    pub entries_apart: u32,
    pub wait: (i64, i64, i64, u64),
    pub deferred: u32,
    pub stale_picks: u32,
    pub lane_skips: u32,
    /// (ironspec 23:21 PT) The away latch's least margin over the runs (gt; i64::MAX: none).
    pub latch_margin_min: i64,
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
    /// The latest entry made in REST (night time; G1 on 6ca670551).
    pub rest_entry_max: i64,
    /// The most riders away from one lane at once; the longest pick -> stop entry (ironspec 12:13).
    pub lane_away_max: u32,
    pub pick_entry_max: i64,
    /// (ironspec 06:31 PT; G1's Y7) Lie-downs before the previous occupant's own climb out of the bed's sphere, over the
    /// runs (bar 0), and the least margin.
    pub exposure: u32,
    pub exposure_margin_min: i64,
    /// (G1's Y9) Runs ending with a skip's DAY rise still pending (bar 0).
    pub skip_unresolved: u32,
    /// (G1's Y17) Floored pick -> back draws over the runs (bar 0).
    pub back_clamped: u32,
    /// (the chain audit) The skip stale windows measured (min, max gt; (MAX, MIN) when no skip), the entries counted
    /// apart by class (in a skip's window; walking past REST's end), and the injected control entries' classes.
    pub skip_window: (i64, i64),
    pub skip_stale: u32,
    pub walk_past: u32,
    pub injected: Vec<EntryClass>,
    /// The releases (entries) over the runs: the release bars' sample count (G1's audit hole 27).
    pub entries_n: u32,
    /// The complete nights judged (no skip cut them), over the runs: the nap bar's sample count.
    pub nights_n: u32,
    /// The cut nights' naps (a skip cut them short; ironspec 01:46 PT (b): priced, printed): (min, max).
    pub cut_naps: (u32, u32),
}

/// A scenario over release phases (every `phase_step`), the given detector phases and every units
/// phase at the start.
pub fn scenario(o: &CounterOpts, evs: &[(i64, Ev)]) -> ScenOut {
    scenario_with(o, evs, 1, &DET_ALL)
}

/// (tick-exact Nov 12 11:16: the detector updates on game times ≡ 0 mod 20, which falls on the world's
/// own day-time phase, an input) Every detector update phase.
pub const DET_ALL: [i64; 20] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19];

pub fn scenario_with(o: &CounterOpts, evs: &[(i64, Ev)], phase_step: usize, det_phases: &[i64]) -> ScenOut {
    scenario_days(o, evs, phase_step, det_phases, 3)
}

/// (ironspec 00:12 PT (2), the slack test) ONE forced skipped pick in the counter model: the first pick opportunity at or
/// after every 100 gt of night 2 skipped as if its lane's napper were away (3 release phases x 10 starting units),
/// against the same run without it: the naps that night and the next. With (B') `lane_count` and a wider window the
/// lane may recover the pick later in the window.
pub fn skip_rows(o: &CounterOpts, v: &mut crate::verdict::Verdict) -> Vec<String> {
    // (G1 on 7897f4ba7 (1): each skip judged on its own night and the next, keyed by the skipped pick's own release, so a
    // skip that falls past night 2's window and lands on night 3's first pick is counted against night 3)
    let key = |t: i64| (20_000 + t).div_euclid(24_000);
    let (mut n, mut lost_night, mut lost_next, mut least_night, mut least_next, mut worst_at) = (0u32, 0i64, 0i64, u32::MAX, u32::MAX, 0i64);
    let mut unverified = 0u32;
    for phase in [0i64, 37, 71] {
        for c0 in 100i64..110 {
            let base = run(o, 5, 20_000, phase, 0, c0, &[]);
            let mut at = 40_000i64;
            while at < 52_000 {
                let oi = CounterOpts { skip_at: Some(at), ..o.clone() };
                let r = run(&oi, 5, 20_000, phase, 0, c0, &[]);
                at += 100;
                let Some(ts) = r.skipped_at else { continue };
                // (G1's Y13 on 10bcdf661: a skip counts as taken only when the pick lists show it: the run without it
                // picked at that tick and the run with it did not; anything else is a failure, never a sample)
                if !(base.pick_ts.contains(&ts) && !r.pick_ts.contains(&ts)) {
                    unverified += 1;
                    continue;
                }
                n += 1;
                let (k, kx) = (key(ts), key(ts) + 1);
                let b_n = base.night_naps.get(&k).copied().unwrap_or(0) as i64;
                let b_x = base.night_naps.get(&kx).copied().unwrap_or(0) as i64;
                let r_n = r.night_naps.get(&k).copied().unwrap_or(0);
                let r_x = r.night_naps.get(&kx).copied().unwrap_or(0);
                if b_n - r_n as i64 > lost_night {
                    lost_night = b_n - r_n as i64;
                    worst_at = ts;
                }
                lost_next = lost_next.max(b_x - r_x as i64);
                least_night = least_night.min(r_n);
                least_next = least_next.min(r_x);
            }
        }
    }
    let w = v.bar_n("the stress class: one skipped pick: the next night back to the run without it", n as usize, lost_next == 0);
    let wv = v.bar_n("the stress class: every self-reported skip verified by the pick lists (picked without it, not with it)", (n + unverified) as usize, unverified == 0);
    vec![format!(
        "  ONE SKIPPED PICK in the counter model, THE STRESS CLASS (a pick lost to a world open at the divert: priced in expectation, ironspec 01:46 PT (b); ironspec 00:12 PT (2), the slack test; {n} injections took (verified by the pick lists; {unverified} unverified: {wv}), each at the first opportunity at or after every 100 gt from night 2's dusk that would have picked (ironspec 04:07 PT: picked -> skipped), 3 release phases x 10 starting units, each judged on the skipped pick's own night and the next; the window C {}..{}, {}): naps that night least {} (at most {lost_night} fewer than without it, the worst at game time {worst_at}: priced), the next night least {} (at most {lost_next} fewer; the bar: back to the run without it: {w})",
        o.window.0,
        o.window.1,
        o.lane_count.map_or("every opportunity picks".to_string(), |c| format!("a lane stops at {c} picks ((B'))")),
        if least_night == u32::MAX { "NONE".to_string() } else { least_night.to_string() },
        if least_next == u32::MAX { "NONE".to_string() } else { least_next.to_string() }
    )]
}

/// (ironspec 23:33 PT (2)) One late return in the counter model: each `add` injected at the first napper's release
/// at or after every 100 gt of the night after a warm night (normal days, 3 release phases x 10 starting units),
/// against the same run without it: the naps that night and the next (of 24, the counter's view: entries in REST)
/// and the picks the per-lane away latch skipped. (G1 on 7897f4ba7 (1), ironspec 01:46 PT (b): +1 and +2 gt are
/// normal operation (a world open at N <= 2), judged on both nights; a longer return is the stress class: the next
/// night judged back to the run without it, that night's loss priced.)
pub fn late_rows(o: &CounterOpts, adds: &[i64], v: &mut crate::verdict::Verdict) -> Vec<String> {
    let mut lines = Vec::new();
    // (with d0 20,000 at game time 0, night key 2's dusk is at game time 40,000; each injection is judged on its own
    // release's night and the next)
    let key = |t: i64| (20_000 + t).div_euclid(24_000);
    for &add in adds {
        let (mut n, mut lost_night, mut lost_next, mut extra_skips, mut least_night, mut least_next) = (0u32, 0i64, 0i64, 0i64, u32::MAX, u32::MAX);
        let mut worst_at = 0i64;
        for phase in [0i64, 37, 71] {
            for c0 in 100i64..110 {
                let base = run(o, 5, 20_000, phase, 0, c0, &[]);
                let mut at = 40_000i64;
                while at < 52_000 {
                    let oi = CounterOpts { inject_late: Some((at, add)), ..o.clone() };
                    let r = run(&oi, 5, 20_000, phase, 0, c0, &[]);
                    at += 100;
                    let Some(tl) = r.late_at else { continue };
                    n += 1;
                    let (k, kx) = (key(tl), key(tl) + 1);
                    let b_n = base.night_naps.get(&k).copied().unwrap_or(0) as i64;
                    let b_x = base.night_naps.get(&kx).copied().unwrap_or(0) as i64;
                    let r_n = r.night_naps.get(&k).copied().unwrap_or(0);
                    let r_x = r.night_naps.get(&kx).copied().unwrap_or(0);
                    if b_n - r_n as i64 > lost_night {
                        worst_at = tl;
                    }
                    lost_night = lost_night.max(b_n - r_n as i64);
                    lost_next = lost_next.max(b_x - r_x as i64);
                    least_night = least_night.min(r_n);
                    least_next = least_next.min(r_x);
                    extra_skips = extra_skips.max(r.lane_skips as i64 - base.lane_skips as i64);
                }
            }
        }
        let normal = add <= 2;
        let verdict = if normal {
            let w = v.bar_n(&format!("normal operation: a late return of +{add} gt: that night and the next as without it"), n as usize, lost_night == 0 && lost_next == 0);
            format!("NORMAL OPERATION (a world open at N <= 2): that night and the next as without it: {w}")
        } else {
            let w = v.bar_n(&format!("the stress class: a late return of +{add} gt: the next night back to the run without it"), n as usize, lost_next == 0);
            format!("THE STRESS CLASS (a whole-cycle late return, excluded from normal operation by the catch window: that night's loss priced): the next night back to the run without it: {w}")
        };
        lines.push(format!(
            "  LATE RETURN +{add} gt in the counter model ({n} injections: one napper's stop cycle and return {add} gt longer, at the first release at or after every 100 gt from night 2's dusk, 3 release phases x 10 starting units, each judged on its own release's night and the next; against the same run without it): naps that night least {} (at most {lost_night} fewer than without it, the worst injected at game time {worst_at}), the next night least {} (at most {lost_next} fewer); picks the away latch skipped at most {extra_skips} more; {verdict}",
            if least_night == u32::MAX { "-".to_string() } else { least_night.to_string() },
            if least_next == u32::MAX { "-".to_string() } else { least_next.to_string() }
        ));
    }
    lines
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
        latch_margin_min: i64::MAX,
        rest_entry_max: i64::MIN,
        exposure_margin_min: i64::MAX,
        skip_window: (i64::MAX, i64::MIN),
        cut_naps: (u32::MAX, 0),
        ..Default::default()
    };
    for phase in (0..100).step_by(phase_step) {
        for &det_phase in det_phases {
            for c0 in 100i64..110 {
                s.runs += 1;
                let r = run(o, days, 20_000, phase, det_phase, c0, evs);
                s.exposure += r.exposure;
                s.exposure_margin_min = s.exposure_margin_min.min(r.exposure_margin_min);
                s.skip_unresolved += r.skip_unresolved;
                s.back_clamped += r.back_clamped;
                for &w in &r.skip_windows {
                    s.skip_window = (s.skip_window.0.min(w), s.skip_window.1.max(w));
                }
                if let Some(c) = r.injected_class {
                    s.injected.push(c);
                }
                s.entries_n += r.entries.len() as u32;
                for k in &r.cut_nights {
                    if let Some(&n) = r.night_naps.get(k) {
                        s.cut_naps = (s.cut_naps.0.min(n), s.cut_naps.1.max(n));
                    }
                }
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
                for &(d, w, class) in &r.entries {
                    let dm = d.rem_euclid(24_000);
                    s.wait = (s.wait.0.min(w), s.wait.1.max(w), s.wait.2 + w, s.wait.3 + 1);
                    if w > s.wait_max_at.0 {
                        s.wait_max_at = (w, (d - o.to_entry - w).rem_euclid(24_000), dm);
                    }
                    // (G1 on 6b9be93ea point 1: a skip's stale window and a walk past REST's end are counted
                    // apart; an entry outside REST with no skip before it counts against both bars)
                    match class {
                        EntryClass::SkipStale => {
                            s.entries_apart += 1;
                            s.skip_stale += 1;
                            continue;
                        }
                        EntryClass::WalkPastRest => {
                            s.entries_apart += 1;
                            s.walk_past += 1;
                            continue;
                        }
                        EntryClass::OutsideRest => s.outside_rest += 1,
                        EntryClass::InRest => {}
                    }
                    let n = if dm < 10 { dm + 24_000 } else { dm };
                    s.entry_min = s.entry_min.min(n);
                    s.entry_max = s.entry_max.max(n);
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
                s.rest_entry_max = s.rest_entry_max.max(r.rest_entry_max);
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
                s.stale_picks += r.stale_picks;
                s.lane_skips += r.lane_skips;
                s.latch_margin_min = s.latch_margin_min.min(r.latch_margin_min);
                s.queue_max = s.queue_max.max(r.queue_max);
                s.away_max = s.away_max.max(r.away_max);
                s.lane_away_max = s.lane_away_max.max(r.lane_away_max);
                s.pick_entry_max = s.pick_entry_max.max(r.pick_entry_max);
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
                        s.nights_n += 1;
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
/// (G1's Y9 / Z6 on 10bcdf661) The first day time the detector's level reaches `l` after a skip (day time 0), per steady
/// weather: None when the curve never reaches it within the day (the excuse can't be bounded: the model gives none and
/// the report fails NOT JUDGED). One function for the model and the report.
pub fn skip_on_lag(l: i64) -> [(&'static str, Option<i64>); 3] {
    [("clear", 0.0f32, 0.0f32), ("rain", 1.0, 0.0), ("thunder", 1.0, 1.0)].map(|(n, r, th)| (n, (0..12_000).find(|&d| det_level(d, r, th) >= l)))
}

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
/// 0/5/10/15 at every d. (The chain fix, G1's audit holes 1 / 27, the exclusions (b)) every bar through `v`: the false
/// arms and latches over the runs, the naps a night (24, weather's shortfalls judged), the earliest entry against
/// REST's 12,000 and the latest against REST's end, entries outside REST, the queue, the false H re-phases over the H
/// rises; an empty sample set is NOT JUDGED.
pub fn weather_sweep(o: &CounterOpts, step: i64, v: &mut crate::verdict::Verdict) -> Result<Vec<String>, String> {
    if step < 1 || step > 6_000 {
        return Err(format!("--sweep-step {step}: 1..=6,000 (every weather transition at least four times a day)"));
    }
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
    let mut total_runs = 0usize;
    let mut arr_all = i64::MIN;
    let (mut rel_emin, mut rel_emax, mut rel_qmax, mut rel_outside, mut rel_entries) = (i64::MAX, i64::MIN, 0u32, 0u32, 0usize);
    for (kname, rel) in &kinds {
        let (mut fa, mut fl, mut runs) = (0u32, 0u32, 0usize);
        let mut least = u32::MAX;
        let mut nights = 0usize;
        let mut arr_max = i64::MIN;
        let (mut qmax, mut emin, mut emax, mut outside, mut n_entries) = (0u32, i64::MAX, i64::MIN, 0u32, 0usize);
        let mut apart = 0u32;
        let mut walk = 0u32;
        let (mut hr, mut fh, mut h_rises) = (0u32, 0u32, 0usize);
        let mut bad_d: Vec<i64> = Vec::new();
        let mut nap_bad_d: Vec<i64> = Vec::new();
        for d in (0..24_000).step_by(step as usize) {
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
                    h_rises += r.c_at_h_rise.len();
                    for &(dd, _w, class) in &r.entries {
                        // (G1 on 6b9be93ea point 1: the skip window and the walk past REST apart; an entry
                        // outside REST with no skip counts against both bars)
                        n_entries += 1;
                        match class {
                            EntryClass::SkipStale => {
                                apart += 1;
                                continue;
                            }
                            // (G1's Y2: a walk past REST's end is judged, never only printed apart; G1's Y2 / Z8 on
                            // 10bcdf661: it also enters the latest-entry bar's sample, so that bar can fail)
                            EntryClass::WalkPastRest => {
                                apart += 1;
                                walk += 1;
                            }
                            EntryClass::OutsideRest => outside += 1,
                            EntryClass::InRest => {}
                        }
                        let dm = dd.rem_euclid(24_000);
                        let nt = if dm < 10 { dm + 24_000 } else { dm };
                        emin = emin.min(nt);
                        emax = emax.max(nt);
                    }
                    bad |= r.false_arms + r.false_latches > 0;
                    let last_complete = (r.end_day - 23_850).div_euclid(24_000);
                    for k in 1..=last_complete {
                        let naps = if o.rest.is_some() { r.night_naps.get(&k) } else { r.night_picks.get(&k) };
                        let n = naps.copied().unwrap_or(0);
                        nights += 1;
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
        total_runs += runs;
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
        let w_fa = v.bar_n(&format!("weather ({kname}): 0 false arms and latches"), runs, fa + fl == 0);
        let w_naps = v.bar_n(&format!("weather ({kname}): 24 naps every complete night"), nights, least == 24);
        let rel_txt = if o.rest.is_some() {
            let w_e = v.bar_n(&format!("weather ({kname}): the earliest entry at or after REST's 12,000"), n_entries, emin != i64::MAX && emin >= 12_000);
            let w_l = v.bar_n(&format!("weather ({kname}): the latest entry lies down before REST's end"), n_entries, emax != i64::MIN && rest_entry_ok(emax, o.stop_walk));
            let w_w = v.bar_n(&format!("weather ({kname}): no entry walks past REST's end (G1's Y2)"), n_entries, walk == 0);
            let w_o = v.bar_n(&format!("weather ({kname}): no entry outside REST with no skip before it"), n_entries, outside == 0);
            let w_q = v.bar_n(&format!("weather ({kname}): at most one held at the gate"), runs, qmax <= 1);
            let w_h = if o.high.is_some() { format!("; H re-phases {hr} over {h_rises} H rises (false {fh}: {})", v.bar_n(&format!("weather ({kname}): no false H re-phase"), h_rises, fh == 0)) } else { String::new() };
            format!(
                "the release: the earliest entry {} ({w_e}), the latest {} ({w_l}), walking past REST's end {walk} ({w_w}), outside REST with no skip before it {outside} ({w_o}), counted apart in a skip's stale window {}, held at the gate at once at most {qmax} ({w_q}){w_h}",
                if emin == i64::MAX { "NONE".to_string() } else { emin.to_string() },
                if emax == i64::MIN { "NONE".to_string() } else { emax.to_string() },
                apart - walk
            )
        } else {
            format!("the latest bed-stop arrival {} ({})", if arr_max == i64::MIN { "-".to_string() } else { arr_max.to_string() }, v.scope("the plain hatch's latest arrival", "the release model is off in this run (--no-release)"))
        };
        lines.push(format!(
            "  {kname} at every d (0..24,000 step {step}; {runs} runs): false arms {fa}, false latches {fl} (at d {bands}; {w_fa}); naps a night least {} of 24 over {nights} complete nights (a night short of 24 at d {nap_bands}; {w_naps}); {rel_txt}",
            if least == u32::MAX { "NONE".to_string() } else { least.to_string() }
        ));
        rel_emin = rel_emin.min(emin);
        rel_emax = rel_emax.max(emax);
        rel_qmax = rel_qmax.max(qmax);
        rel_outside += outside;
        rel_entries += n_entries;
    }
    // (ironspec 07:18 PT: the DAY edges compared exactly with tick-exact's official off-edges, with +-1 controls: the
    // detector's level falls below L at dusk at these day times in steady clear, rain and thunder; G1's Y1 on f21c9f600:
    // the controls run the SAME predicate on a SHIFTED INPUT (the detector's curve read one tick early and one tick
    // late), which must fail it)
    let l_day = o.day_inhibit.ok_or("the weather sweep's DAY-off rows need the DAY line's level (--day-inhibit / the design's L): no silent default")?;
    let mut dayoff_txt: Vec<String> = Vec::new();
    for (wname, rain, thunder, official) in [("clear", 0.0f32, 0.0f32, 12_230i64), ("rain", 1.0, 0.0, 12_002), ("thunder", 1.0, 1.0, 11_556)] {
        let off_of = |shift: i64| (9_000..14_000).find(|&d| det_level(d + shift, rain, thunder) < l_day);
        // (G1's Y1 on 10bcdf661: ONE predicate, called by the bar on the real curve and by the controls on the shifted ones)
        let pred = |o: Option<i64>| o == Some(official);
        let off = off_of(0);
        let w = v.bar(&format!("weather sweep: the {wname} DAY-off equals tick-exact's official {official}"), pred(off));
        let (early, late) = (off_of(1), off_of(-1));
        let c = v.control(&format!("weather sweep: the {wname} DAY-off bar fails the detector's curve shifted one tick either way"), usize::from(early.is_some() && late.is_some()), !pred(early) && !pred(late));
        dayoff_txt.push(format!(
            "{wname} {} against {official} ({w}; the curve shifted a tick: {} / {}: {c})",
            off.map_or("NONE".to_string(), |d| d.to_string()),
            early.map_or("NONE".to_string(), |d| d.to_string()),
            late.map_or("NONE".to_string(), |d| d.to_string())
        ));
    }
    // (G1's Y2, the walk-past bar's control: the stop's walk lengthened past REST (12,100 gt, a single change), so every
    // entry made in REST outlasts REST's end: the walk-past bar must see them)
    let walk_ctl = if o.rest.is_some() {
        let oc = CounterOpts { stop_walk: 12_100, ..o.clone() };
        let r = run(&oc, 3, 20_000, 0, 0, 100, &[]);
        let walks = r.entries.iter().filter(|e| matches!(e.2, EntryClass::WalkPastRest)).count();
        format!("CONTROL (G1's Y2): the stop's walk lengthened to 12,100 gt: {walks} entries walk past REST's end ({})", v.control("the walk-past bar sees an entry whose walk outlasts REST", r.entries.len(), walks > 0))
    } else {
        v.scope("the walk-past control", "the release model is off in this run (--no-release)")
    };
    lines.push(walk_ctl);
    lines.insert(0, format!(
        "weather sweep (G1 Nov 12 02:43; the arm: {}; the latch: {}; L {}): every rain or thunder onset and end at every day time, the vanilla ramps (0.01/gt, thunder x rain), the detector on redsim's exact curve; its DAY-off per steady weather against tick-exact's official rows: {}; every kind's bars judged below",
        o.arm.map_or("off".to_string(), |s| format!("falling 8 for {s} gt")),
        o.drop_min.map_or("off".to_string(), |k| format!("a fall >= {k} between readings")),
        o.day_inhibit.map_or("off".to_string(), |l| l.to_string()),
        dayoff_txt.join("; ")
    ));
    lines.push(format!("  => false arms + latches over every kind {total} in {total_runs} runs"));
    if o.rest.is_some() {
        lines.push(format!(
            "  => the release over every weather kind and d ({rel_entries} entries): the earliest bed-stop entry {}, the latest {}; entries outside REST with no skip before them {rel_outside}; held at the gate at once at most {rel_qmax}",
            if rel_emin == i64::MAX { "NONE".to_string() } else { rel_emin.to_string() },
            if rel_emax == i64::MIN { "NONE".to_string() } else { rel_emax.to_string() }
        ));
    } else {
        lines.push(format!(
            "  => the plain hatch: the latest bed-stop arrival over every weather kind and d {}",
            if arr_all == i64::MIN { "-".to_string() } else { arr_all.to_string() }
        ));
    }
    Ok(lines)
}

/// Merge one scenario's release-model fields into a sweep's.
fn merge_release(a: &mut ScenOut, s: &ScenOut) {
    a.entry_min = a.entry_min.min(s.entry_min);
    a.entry_max = a.entry_max.max(s.entry_max);
    a.outside_rest += s.outside_rest;
    a.entries_apart += s.entries_apart;
    a.stale_entries += s.stale_entries;
    a.wait = (a.wait.0.min(s.wait.0), a.wait.1.max(s.wait.1), a.wait.2 + s.wait.2, a.wait.3 + s.wait.3);
    if s.wait_max_at.0 > a.wait_max_at.0 {
        a.wait_max_at = s.wait_max_at;
    }
    a.deferred += s.deferred;
    a.stale_picks += s.stale_picks;
    a.lane_skips += s.lane_skips;
    a.latch_margin_min = a.latch_margin_min.min(s.latch_margin_min);
    a.queue_max = a.queue_max.max(s.queue_max);
    a.away_max = a.away_max.max(s.away_max);
    a.lane_away_max = a.lane_away_max.max(s.lane_away_max);
    a.pick_entry_max = a.pick_entry_max.max(s.pick_entry_max);
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
    a.rest_entry_max = a.rest_entry_max.max(s.rest_entry_max);
    a.exposure += s.exposure;
    a.exposure_margin_min = a.exposure_margin_min.min(s.exposure_margin_min);
    a.skip_unresolved += s.skip_unresolved;
    a.back_clamped += s.back_clamped;
    a.skip_stale += s.skip_stale;
    a.walk_past += s.walk_past;
    a.entries_n += s.entries_n;
    a.skip_window = (a.skip_window.0.min(s.skip_window.0), a.skip_window.1.max(s.skip_window.1));
    for (k, &(lo, hi)) in &s.naps_by_key {
        let e = a.naps_by_key.entry(*k).or_insert((u32::MAX, 0));
        *e = (e.0.min(lo), e.1.max(hi));
    }
}

/// (G1 on 7897f4ba7 (1); the chain fix) A bar's word: a judged bar goes through the tool's verdict over its `n` samples
/// (0 samples: NOT JUDGED, a failure); an unjudged one (the skip class's priced bars, or the rejected comparison variant)
/// prints how it falls against the same bar, marked as not judged.
fn verdict_word(ok: bool, judged: bool, why_not: &str, name: &str, n: usize, v: &mut crate::verdict::Verdict) -> String {
    if judged {
        v.bar_n(name, n, ok)
    } else if n == 0 {
        format!("no samples; not judged ({why_not})")
    } else if ok {
        format!("within the bar; not judged ({why_not})")
    } else {
        format!("outside the bar; not judged ({why_not})")
    }
}

/// (G1 on 7897f4ba7 (1)) The counter's bars over one class of scenarios, each with its worst scenario named. `judged`:
/// the design variant's normal operation judges every bar; its skip class judges the structural bars (the window, the
/// gate, the ring, the counts, false arms) and prices the nap and early-pick bars under the skip ruling (ironspec 00:12 /
/// 01:46 PT); the rejected comparison variant is printed, never judged. (The chain fix, holes 3 / 27) every judged bar
/// over its own sample count: no sample is NOT JUDGED.
fn class_bars(o: &CounterOpts, ss: &[(String, ScenOut)], class: &str, mode: u8, v: &mut crate::verdict::Verdict) -> String {
    // (mode 0: normal operation, all judged; 1: the skip class, structural judged, the rest priced; 2: the comparison)
    let structural = mode <= 1;
    let priced = mode == 0;
    let why = if mode == 1 { "the skip class: priced under the skip ruling, ironspec 01:46 PT (b)" } else { "the rejected comparison variant" };
    let worst = |f: &dyn Fn(&ScenOut) -> Option<i64>, lowest: bool| -> (Option<i64>, String) {
        let mut best: (Option<i64>, String) = (None, "-".to_string());
        for (label, s) in ss {
            if let Some(v) = f(s) {
                let better = match best.0 {
                    None => true,
                    Some(b) => (lowest && v < b) || (!lowest && v > b),
                };
                if better {
                    best = (Some(v), label.clone());
                }
            }
        }
        best
    };
    let count = |f: &dyn Fn(&ScenOut) -> bool| ss.iter().filter(|(_, s)| f(s)).count();
    let runs: usize = ss.iter().map(|(_, s)| s.runs).sum();
    let night = worst(&|s| (s.open_min != i64::MAX).then(|| ((o.night_end - (s.open_max + o.last_nap)) as f64 / o.night_len as f64 * 1000.0) as i64), true);
    let early = worst(&|s| (s.open_min != i64::MAX).then(|| s.open_min + o.to_bed - (o.bed_clear - o.early_tol)), true);
    let naps = worst(&|s| (s.min_night != u32::MAX).then_some(s.min_night as i64), true);
    let gate = worst(&|s| (s.hi.0 + s.lo.0 + s.rej.0 > 0).then(|| s.hi.1.min(s.lo.1).min(s.rej.1)), true);
    let spacing = worst(&|s| (s.min_spacing != i64::MAX).then_some(s.min_spacing), true);
    let daylight = worst(&|s| Some(s.daylight as i64), false);
    let counts = worst(&|s| Some(s.cerr_bad as i64), false);
    let falses = worst(&|s| Some((s.false_arms + s.false_latches) as i64), false);
    let n_day: u32 = ss.iter().map(|(_, s)| s.daylight).sum();
    let n_counts: usize = ss.iter().map(|(_, s)| s.cerr_bad).sum();
    let n_counted: usize = ss.iter().map(|(_, s)| s.cerr_n).sum();
    let n_false: u32 = ss.iter().map(|(_, s)| s.false_arms + s.false_latches).sum();
    let n_inh: u32 = ss.iter().map(|(_, s)| s.night_inh).sum();
    let n_nights: usize = ss.iter().map(|(_, s)| s.nights_n as usize).sum();
    let n_trig: usize = ss.iter().map(|(_, s)| s.hi.0 + s.lo.0 + s.rej.0).sum();
    let opened = count(&|s| s.open_min != i64::MAX);
    let at = |w: &(Option<i64>, String)| format!(" (worst: {})", w.1);
    let mut out = format!("  => bars, {class} ({} scenario(s), {runs} runs: {}): ", ss.len(), ss.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("; "));
    let ok_naps = naps.0.is_some_and(|v| v >= 24);
    out += &format!("every night 24 naps: least {} over {n_nights} nights{} ({}); ", naps.0.map_or("-".to_string(), |v| v.to_string()), at(&naps), verdict_word(ok_naps, priced, why, &format!("{class}: naps {}", at(&naps)), n_nights, v));
    let ok_night = night.0.is_some_and(|v| v > 0);
    out += &format!("night margin {}{} ({}); ", night.0.map_or("-".to_string(), |v| format!("{:.1}%", v as f64 / 10.0)), at(&night), verdict_word(ok_night, structural, why, &format!("{class}: night margin{}", at(&night)), opened, v));
    let ok_early = early.0.is_some_and(|v| v >= 0);
    out += &format!("the first napper's arrival {} gt{} ({}); ", early.0.map_or("-".to_string(), |v| v.to_string()), at(&early), verdict_word(ok_early, priced, why, &format!("{class}: the first napper's arrival{}", at(&early)), opened, v));
    out += &format!("picks before the beds take them {n_day}{} ({}); ", at(&daylight), verdict_word(n_day == 0, priced, why, &format!("{class}: picks before the beds{}", at(&daylight)), runs, v));
    if let Some(l) = o.day_inhibit {
        let edge = worst(&|s| (s.edge_gap != i64::MAX).then_some(s.edge_gap), true);
        let coupling = worst(&|s| (s.day_off_margin != i64::MAX).then_some(s.day_off_margin), true);
        out += &format!("night picks the level < {l} inhibit removed {n_inh} ({}); ", verdict_word(n_inh == 0, structural, why, &format!("{class}: night picks inhibited"), runs, v));
        out += &format!("the inhibit's edge >= 3 gt + 1 tick from a divert: least {}{} ({}); ", edge.0.map_or("-".to_string(), |v| v.to_string()), at(&edge), verdict_word(edge.0.is_some_and(|v| v >= 4), priced, why, &format!("{class}: the inhibit's edge{}", at(&edge)), count(&|s| s.edge_gap != i64::MAX), v));
        out += &format!("DAY off >= 20 gt before the window opens: least {}{} ({}); ", coupling.0.map_or("-".to_string(), |v| v.to_string()), at(&coupling), verdict_word(coupling.0.is_some_and(|v| v >= 20), priced, why, &format!("{class}: DAY off before the window{}", at(&coupling)), count(&|s| s.day_off_margin != i64::MAX), v));
    }
    out += &format!("gate margins >= 1 release over {n_trig} trigger-2 events: least {} gt{} ({}); ", gate.0.map_or("-".to_string(), |v| v.to_string()), at(&gate), verdict_word(gate.0.is_some_and(|v| v >= 100), structural, why, &format!("{class}: gate margins{}", at(&gate)), n_trig, v));
    out += &format!("ring pulses >= {} gt apart: least {}{} ({}); ", o.ff_period, spacing.0.map_or("-".to_string(), |v| v.to_string()), at(&spacing), verdict_word(spacing.0.is_some_and(|v| v >= o.ff_period), structural, why, &format!("{class}: ring pulse spacing{}", at(&spacing)), count(&|s| s.min_spacing != i64::MAX), v));
    out += &format!("count errors {n_counts} of {n_counted} re-phases checked{} ({}); ", at(&counts), verdict_word(n_counts == 0, structural, why, &format!("{class}: count errors{}", at(&counts)), n_counted, v));
    out += &format!("false arms + latches {n_false}{} ({})", at(&falses), verdict_word(n_false == 0, structural, why, &format!("{class}: false arms + latches{}", at(&falses)), runs, v));
    out
}

/// (G1 on 7897f4ba7 (1)) The release's bars over one class (as `class_bars`): the structural bars (no entry outside REST
/// not after a skip, one held at the gate, one away per lane, the latest entry made in REST, the exposure, the stale-hold
/// picks) are judged in both classes of the design variant; the naps, the away latch's skipped picks and the walk past
/// REST's end are priced in the skip class. (The chain fix) every judged bar over its sample count.
fn class_release(o: &CounterOpts, ss: &[(String, ScenOut)], class: &str, mode: u8, v: &mut crate::verdict::Verdict) -> String {
    let structural = mode <= 1;
    let priced = mode == 0;
    let o_climb = o.climb_exit;
    let why = if mode == 1 { "the skip class: priced under the skip ruling, ironspec 01:46 PT (b)" } else { "the rejected comparison variant" };
    let mut a = ScenOut {
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
        latch_margin_min: i64::MAX,
        rest_entry_max: i64::MIN,
        exposure_margin_min: i64::MAX,
        skip_window: (i64::MAX, i64::MIN),
        ..Default::default()
    };
    for (_, s) in ss {
        merge_release(&mut a, s);
    }
    let runs: usize = ss.iter().map(|(_, s)| s.runs).sum();
    let entries = a.entries_n as usize;
    let named = |f: &dyn Fn(&ScenOut) -> bool| -> String {
        let v: Vec<&str> = ss.iter().filter(|(_, s)| f(s)).map(|(l, _)| l.as_str()).collect();
        if v.is_empty() { String::new() } else { format!(" (in: {})", v.join("; ")) }
    };
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let ok_entry = a.entry_min != i64::MAX && a.entry_min >= 12_000;
    let ok_rest = a.rest_entry_max != i64::MIN && rest_entry_ok(a.rest_entry_max, o.stop_walk);
    let mut out = format!("  => the release's bars, {class} ({} scenario(s) and sweeps, {runs} runs, {entries} entries): ", ss.len());
    out += &format!("the earliest bed-stop entry at dusk {} (bar >= 12,000: {}); ", m(a.entry_min, i64::MAX), verdict_word(ok_entry, structural, why, &format!("{class}: the earliest entry{}", named(&|s| s.entry_min < 12_000)), entries, v));
    out += &format!("entries outside REST not after a skip (outside its measured stale window) {} ({}); ", a.outside_rest, verdict_word(a.outside_rest == 0, structural, why, &format!("{class}: entries outside REST{}", named(&|s| s.outside_rest > 0)), entries, v));
    out += &format!(
        "entries in a skip's measured stale window {} (the windows {} gt: the skip to the hatch seeing DAY high), stale admissions {} (each waits on the stop's ledge until REST and naps that evening: option A's outcome); ",
        a.skip_stale,
        if a.skip_window.0 == i64::MAX { "-".to_string() } else { format!("{}..{}", a.skip_window.0, a.skip_window.1) },
        a.stale_admit_n
    );
    out += &format!("entries whose walk reaches REST's end {} ({}); ", a.walk_past, verdict_word(a.walk_past == 0, priced, why, &format!("{class}: entries walking past REST's end{}", named(&|s| s.walk_past > 0)), entries, v));
    out += &format!("the least gap between consecutive departures from the stop {} gt; the least R -> the next lie-down {} gt; ", m(a.exit_gap_min, i64::MAX), m(a.liedown_gap_min, i64::MAX));
    out += &format!("the latest entry made in REST at night time {} (bar < 24,010 - the walk {} = {}: {}); ", m(a.rest_entry_max, i64::MIN), o.stop_walk, 24_010 - o.stop_walk, verdict_word(ok_rest, structural, why, &format!("{class}: the latest entry in REST{}", named(&|s| s.rest_entry_max != i64::MIN && !rest_entry_ok(s.rest_entry_max, o.stop_walk))), entries, v));
    out += &format!(
        "lie-downs before the previous occupant's own climb out of the bed's 16-block sphere (the exposure, ironspec 06:31 PT; G1's Y7: from the occupants' own timelines, the climb out = its R + {} (ironspec 09:03's estimate, pending the composed run-out)) {} ({}), the least margin {}; ",
        o_climb,
        a.exposure,
        verdict_word(a.exposure == 0, structural, why, &format!("{class}: the exposure{}", named(&|s| s.exposure > 0)), entries, v),
        if a.exposure_margin_min == i64::MAX { "-".to_string() } else { format!("{} gt", a.exposure_margin_min) }
    );
    out += &format!(
        "runs ending with a skip's DAY rise still pending (G1's Y9: an excuse that can't close) {} ({}); ",
        a.skip_unresolved,
        v.bar_n(&format!("{class}: no run ends with a skip's DAY rise pending"), ss.iter().map(|(_, s)| s.runs).sum(), a.skip_unresolved == 0)
    );
    out += &format!(
        "drawn pick -> back times floored (G1's Y17: shorter than the entry and the stop's cycle, a data inconsistency) {} ({}); ",
        a.back_clamped,
        v.bar_n(&format!("{class}: no drawn pick -> back floored"), ss.iter().map(|(_, s)| s.runs).sum(), a.back_clamped == 0)
    );
    out += &format!("riders away from one lane at once at most {} (bar 1: {}); ", a.lane_away_max, verdict_word(a.lane_away_max <= 1, structural, why, &format!("{class}: two away from one lane{}", named(&|s| s.lane_away_max > 1)), runs, v));
    out += &format!("held at the gate at once at most {} (bar 1: {}); ", a.queue_max, verdict_word(a.queue_max <= 1, structural, why, &format!("{class}: two held at the gate{}", named(&|s| s.queue_max > 1)), runs, v));
    out += &format!("picks deferred {}; ", a.deferred);
    out += &format!("picks made while a stale napper held the stop outside REST {} (ironspec 12:54 PT, bar 0: {}); ", a.stale_picks, verdict_word(a.stale_picks == 0, structural, why, &format!("{class}: picks while a stale napper held the stop{}", named(&|s| s.stale_picks > 0)), runs, v));
    out += &format!(
        "picks the per-lane away latch skipped {} (its least margin, a lane's pick against its previous napper's return, {}; a latch still set at the pick is a skipped pick, bar 0: {}); ",
        a.lane_skips,
        m(a.latch_margin_min, i64::MAX),
        verdict_word(a.lane_skips == 0, priced, why, &format!("{class}: picks the away latch skipped{}", named(&|s| s.lane_skips > 0)), runs, v)
    );
    let nights: usize = ss.iter().map(|(_, s)| s.nights_n as usize).sum();
    out += &format!("naps a night least {} of 24 over {nights} complete nights ({})", if a.min_naps == u32::MAX { "-".to_string() } else { a.min_naps.to_string() }, verdict_word(a.min_naps >= 24 && a.min_naps != u32::MAX, priced, why, &format!("{class}: naps{}", named(&|s| s.min_naps < 24)), nights, v));
    out
}

/// One scenario's release-model line (`rest`).
fn release_line(s: &ScenOut) -> String {
    let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
    let mean = if s.wait.3 > 0 { format!("{:.0}", s.wait.2 as f64 / s.wait.3 as f64) } else { "-".to_string() };
    format!(
        "    release (REST_l AND the stale-window hold AND NOT DAY after it AND the stop clear): entries {}: the earliest at dusk at day time {} (bar >= 12,000: {}), the latest {} (the latest made in REST, stale or not, at night time {}), {} outside REST with no skip before it (10..11,999, keyed on the stop's entry: {}); counted apart (a skip's stale window or a walk past REST's end) {}; stale admissions {} (in a skip's stale window, day times {}: each waits on the stop's ledge until REST and naps that evening, the stop clear at day time {}); the least gap between consecutive departures from the stop {} gt; the least time from a napper's R to the next napper's lie-down {} gt; the wait at the queue {}..{} gt (mean {mean}); picks deferred by the back-pressure {} (picks made while a stale napper held the stop outside REST {}; picks the per-lane away latch skipped {}); held at the gate at once at most {} (bar 1); riders away at a release at most {} (from one lane at any instant at most {}; the longest pick to stop entry {} gt); REST_l rose at day time {}..{} (+ {} transient rises by day, DAY high: no release); naps a night (complete nights no skip cut) least {} of 24",
        s.wait.3,
        if s.entry_min == i64::MAX { "no entries".to_string() } else { s.entry_min.to_string() },
        if s.entry_min != i64::MAX && s.entry_min >= 12_000 { "within; judged in the class line" } else { "OUTSIDE; judged in the class line" },
        m(s.entry_max, i64::MIN),
        m(s.rest_entry_max, i64::MIN),
        s.outside_rest,
        if s.outside_rest == 0 { "none; judged in the class line" } else { "SOME; judged in the class line" },
        s.entries_apart,
        s.stale_admit_n,
        if s.stale_admit_n == 0 { "-".to_string() } else { format!("{}..{}", s.stale_admit_dt.0, s.stale_admit_dt.1) },
        if s.stale_admit_n == 0 { "-".to_string() } else { format!("{}..{}", s.stale_clear_dt.0, s.stale_clear_dt.1) },
        m(s.exit_gap_min, i64::MAX),
        m(s.liedown_gap_min, i64::MAX),
        m(s.wait.0, i64::MAX),
        m(s.wait.1, i64::MIN),
        s.deferred,
        s.stale_picks,
        s.lane_skips,
        s.queue_max,
        s.away_max,
        s.lane_away_max,
        s.pick_entry_max,
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
pub fn high_params(o: &CounterOpts, h: i64) -> Result<((i64, i64, i64, i64), String), String> {
    // (G1 on 6b9be93ea point 4: an H the clear curve never reaches, or no clear H rise in the probe, is an
    // error, never a silent 0 that inverts the window)
    let t_h = (0..24_000).find(|&d| det_level(d, 0.0, 0.0) >= h).ok_or_else(|| format!("H {h}: the clear daylight curve never reaches it"))?;
    let t_h_end = (t_h..24_000).take_while(|&d| det_level(d, 0.0, 0.0) >= h).last().unwrap_or(t_h);
    let probe = CounterOpts { variant: Variant::Ff, high: Some((h, 0, i64::MIN, i64::MAX)), ..o.clone() };
    let s = scenario_with(&probe, &[], 1, &DET_ALL);
    let (cmin, cmax) = s.c_h;
    if cmin == i64::MAX || cmax == i64::MIN {
        return Err(format!("H {h}: no clear H rise reached a release in the probe"));
    }
    let load = (0..=cmin).rev().find(|c| c.rem_euclid(10) == 0 || c.rem_euclid(10) == 9).ok_or_else(|| format!("H {h}: no load C at or below {cmin}"))?;
    let span = (t_h_end - t_h + 99) / 100;
    let (lo, hi) = (cmin - 2, cmax + span + 2);
    Ok((
        (h, load, lo, hi),
        format!(
            "the high re-phase (ironspec 09:49): H {h}, the clear level >= {h} over day time {t_h}..{t_h_end} (rain tops at 12, thunder at 10: weather only delays or prevents an H rise); C at the release after the clear H rise {cmin}..{cmax} => load C {load} (units 0 or 9 for the FF's taps; never ahead), window C {lo}..{hi} (the natural span + 2); an H rise with C outside it re-phases at the next release"
        ),
    ))
}

/// (the chain audit, hole 25) The DAY out's lag behind a detector update: the DAY_LV comparator (a comparator: 2 gt)
/// and the output repeater (delay 1: 2 gt), vanilla's diode delays; derived, the DAY line's real-cell bench (line 1,
/// pending) measures it.
const DAY_OUT_LAG: i64 = 2 + 2;

/// (the chain audit, hole 25) The high re-phase's stated limit for the build guide (ironspec 09:49 / 10:18 PT, H 15
/// adopted with it): a daytime storm skip may lose naps the post-skip night only at skip day times 500..1,750.
const H_STATED_LIMIT: (i64, i64) = (500, 1_750);

/// The release model's report (`rest`): the clear dusk and dawn edges, the daytime storm skip through
/// the day under (b) and (a) (the earliest entry, the most held at once, the held napper's wait, the
/// deferred picks, the post-skip night's naps), the back-pressure against a no-back-pressure control,
/// and restarts. (The chain fix) every bar and control through `v`.
fn release_report(base: &CounterOpts, scen: &[(&str, Vec<(i64, Ev)>)], v: &mut crate::verdict::Verdict) -> Vec<String> {
    let mut lines = vec![format!("  (the adopted stop, ironspec 15:37 / 16:00 / 16:32 (4) and ironspec's note (a) on f6e715e50: no T_nolie / T_fault / T_open; every visit's entry -> R the measured band + {} (ctl-shutter's sustained-presence release: the park at the wake + 22 against the bands' about the wake + 8); a skip-woken sleeper (class a) out through R at its wake + {} (ctl-shutter's worst); class b (on foot at REST's end) holds the stop to the next REST + a visit; the next napper held at R1 until the occupant rides out past R (rider-keyed RS_ready))", base.release_shift, base.wake_to_r)];
    let o = CounterOpts { variant: Variant::Ff, ..base.clone() };
    // (G1's Z5 on 10bcdf661: the new bars' controls, each a planted defect through the same model run: a DAY line whose
    // level the curve never reaches leaves a skip's DAY rise pending to the run's end (skip_unresolved); a drawn
    // pick -> back shorter than the entry and the cycle is floored (back_clamped))
    {
        let c1 = CounterOpts { day_inhibit: Some(99), ..o.clone() };
        let s1 = scenario_days(&c1, &[(2_000, Ev::Skip)], 5, &[0, 13], 4);
        let w1 = v.control("a skip whose DAY rise never comes (L 99) leaves the run ending with it pending", s1.runs, s1.skip_unresolved > 0);
        let c2 = CounterOpts { inject_entry: Some(6_000), back_hist: Some(vec![(1i64, 1u32)]), ..o.clone() };
        let s2 = scenario_days(&c2, &[], 5, &[0, 13], 4);
        let w2 = v.control("a pick -> back draw of 1 gt (shorter than the entry and the cycle) is counted as floored", s2.runs, s2.back_clamped > 0);
        lines.push(format!(
            "CONTROLS (G1's Z5): the DAY line at L 99 with a skip: {} of {} runs end with the skip's DAY rise pending ({w1}); a 1-gt pick -> back draw on the injected stale entry: {} draws floored ({w2})",
            s1.skip_unresolved, s1.runs, s2.back_clamped
        ));
    }
    // (G1 on 6b9be93ea point 1, ironspec 20:55 PT (2)(1); the chain audit: planted in the model's input) the release
    // bars' positive control: on normal days a napper put at the queue at day time 6,000 of day 1 with its release's
    // inputs forced; the model's own classifier must class it outside REST and both bars must see it
    {
        let inj = CounterOpts { inject_entry: Some(6_000), ..o.clone() };
        let s = scenario_days(&inj, &[], 5, &[0, 13], 4);
        let classed = s.injected.iter().all(|c| *c == EntryClass::OutsideRest);
        let bites_min = s.entry_min != i64::MAX && s.entry_min < 12_000;
        let bites_out = s.outside_rest > 0;
        // (G1's Y13: the injections counted against the runs, never self-reported: every run made its one injection)
        let all_injected = s.injected.len() == s.runs;
        let w = v.control("the injected entry by day (6,000 on day 1): one injection in every run, classed outside REST, seen by both release bars", s.injected.len(), all_injected && classed && bites_min && bites_out);
        lines.push(format!(
            "CONTROL: the release bars' positive control (a napper put at the queue at day time 6,000 of day 1, its release's REST_l and DAY inputs forced; {} injections over {} runs): classed {} by the model's classifier; the earliest entry {}, outside REST {} ({w})",
            s.injected.len(),
            s.runs,
            if classed { "outside REST every time".to_string() } else { format!("{:?}", s.injected.iter().take(3).collect::<Vec<_>>()) },
            if s.entry_min == i64::MAX { "no entries".to_string() } else { s.entry_min.to_string() },
            s.outside_rest
        ));
        // (G1's audit hole 22, the second control: an entry outside REST long after a skip's measured stale window is
        // OutsideRest, never excused: a night-0 skip lands at day time 1 of day 1, the napper injected at 1,000)
        let inj2 = CounterOpts { inject_entry: Some(1_000), ..o.clone() };
        let s2 = scenario_days(&inj2, &[(2_000, Ev::Skip)], 5, &[0, 13], 4);
        let classed2 = s2.injected.iter().all(|c| *c == EntryClass::OutsideRest);
        let w2 = v.control("an entry 1,000 gt after a skip (past its measured stale window): one injection in every run, classed outside REST", s2.injected.len(), s2.injected.len() == s2.runs && classed2 && s2.outside_rest > 0);
        lines.push(format!(
            "CONTROL: an entry injected at day time 1,000 of day 1, after a night-0 skip (its measured stale windows {} gt; {} injections over {} runs): classed {} ({w2})",
            if s2.skip_window.0 == i64::MAX { "-".to_string() } else { format!("{}..{}", s2.skip_window.0, s2.skip_window.1) },
            s2.injected.len(),
            s2.runs,
            if classed2 { "outside REST every time".to_string() } else { format!("{:?}", s2.injected.iter().take(3).collect::<Vec<_>>()) }
        ));
    }
    // (G1's Y16: the DAY line's level is the design's, never a silent default)
    let Some(l) = o.day_inhibit else {
        v.fail("the release report's DAY rows: NOT JUDGED (no DAY line level in this run: --day-inhibit)");
        lines.push("the release report's DAY rows: NOT JUDGED (no DAY line level: --day-inhibit)".to_string());
        return lines;
    };
    let first = |from: i64, to: i64, want_hi: bool| (from..to).find(|&d| (det_level(d, 0.0, 0.0) >= l) == want_hi);
    if let Some(t5) = first(9_000, 14_000, false) {
        let w = v.bar("the clear dusk's DAY fall reaches the hatch at or after REST's 12,000", t5 + DAY_OUT_LAG >= 12_000);
        lines.push(format!(
            "the clear dusk's DAY fall ((b)'s condition, ironspec 09:12): the clear level < L {l} from day time {t5}; read at the earliest detector phase (an update at {t5}) the DAY out (-5,14,6) falls at {} ({DAY_OUT_LAG} gt DERIVED: the DAY_LV comparator 2 + the output repeater 2, vanilla's diode delays; the latest phase 19 gt later), the hatch 2 + the line after it: against REST's 12,000 a margin of {} gt ({w})",
            t5 + DAY_OUT_LAG,
            t5 + DAY_OUT_LAG - 12_000
        ));
    } else {
        v.fail("the clear dusk's DAY fall: NOT JUDGED (the clear curve never falls below L between 9,000 and 14,000)");
    }
    let t6 = first(20_000, 24_000, true);
    // (G1's Y9) the skip excuse's bound while its DAY rise is pending, from the curve (a skip lands at day time 0)
    let on_after_skip = skip_on_lag(l);
    if let Some((n, _)) = on_after_skip.iter().find(|x| x.1.is_none()) {
        v.fail(&format!("the skip excuse's bound: NOT JUDGED (the {n} curve never reaches L {l} in a day after a skip: no excuse is given, fail closed)"));
    }
    lines.push(format!(
        "the skip excuse's bound while a skip's DAY rise is pending (G1's Y9 / Z6 / Z7): the first day time the level reaches L {l} after a skip (day time 0): {} => the worst + the detector's period {} + the DAY line's output lag {DAY_OUT_LAG} + 1 + the hatch's lag {} = {} gt after the skip; past it, an entry outside REST is counted as one",
        on_after_skip.iter().map(|(n, d)| format!("{n} {}", d.map_or("NEVER".to_string(), |d| d.to_string()))).collect::<Vec<_>>().join(", "),
        o.det_period,
        o.hatch_lag,
        on_after_skip.iter().filter_map(|x| x.1).max().unwrap_or(0) + o.det_period + DAY_OUT_LAG + 1 + o.hatch_lag
    ));
    let normal = scenario_with(&o, &[], 5, &[0, 7, 13, 19]);
    let load_a = if normal.c_rise.0 == i64::MAX { None } else { Some(normal.c_rise.0) };
    lines.push(format!(
        "the clear dawn's DAY rise: the clear level >= L {l} from day time {}, the DAY out {} at the earliest phase; the C the clock shows at the release after it on normal days {}..{} (the rise read at day time {}..{}) => (a)'s load C {} (the clear-weather edge time, its earliest phase: never ahead of the clear clock; ironspec 09:19)",
        t6.map_or("-".to_string(), |t| t.to_string()),
        t6.map_or("-".to_string(), |t| (t + DAY_OUT_LAG).to_string()),
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
        let (mut misses, mut entries) = (Vec::new(), 0usize);
        for d in (1_000..=11_000).step_by(1_000) {
            let evs = vec![(4_000 + (d - 3_000).max(0), Ev::Wx { rain: true, thunder: true }), (4_000 + d, Ev::Skip)];
            let s = scenario_days(ov, &evs, 5, &[0, 13], 4);
            let nk = |k: i64| s.naps_by_key.get(&k).map_or("-".to_string(), |(a, b)| format!("{a}..{b}"));
            let lost = s.naps_by_key.get(&2).map_or(0, |(a, _)| 24u32.saturating_sub(*a));
            entries += s.entries_n as usize;
            // (G1 on 7897f4ba7 (1): the skip class judges its structural bars; the post-skip night's naps are priced)
            let ok = s.entry_min != i64::MAX && s.entry_min >= 12_000 && s.outside_rest == 0 && s.queue_max <= 1;
            if !ok {
                misses.push(d);
            }
            lines.push(format!(
                "  skip at {d}: the earliest entry {} ({}), outside REST {}; held at the gate at once at most {}; the longest wait at the queue {} gt (arrived at day time {}, entered at {}); picks deferred {}; riders away at a release at most {}; the post-skip night's naps {} of 24 (lost up to {lost}: priced, ironspec 01:46 PT (b)), the night after {}; drop-latch sets {} (each re-phases the clock at the next 7 -> 8 edge)",
                if s.entry_min == i64::MAX { "-".to_string() } else { s.entry_min.to_string() },
                if ok { "within the structural bars" } else { "OUTSIDE the structural bars" },
                s.outside_rest,
                s.queue_max,
                s.wait_max_at.0, s.wait_max_at.1, s.wait_max_at.2,
                s.deferred,
                s.away_max,
                nk(2),
                nk(3),
                s.latch_sets
            ));
        }
        let w = v.bar_n(&format!("the skip class, the daytime thunderstorm skips, {vname}: every skip's entries at dusk, none outside REST, one held at most"), entries, misses.is_empty());
        lines.push(format!("  => {vname}: {w}{}", if misses.is_empty() { String::new() } else { format!(" (the skips at {misses:?})") }));
    }
    // (ironspec 09:49: (b) + the high re-phase, the daytime storm skip at every 250 gt over 0..12,000:
    // 24 naps the post-skip night; an uncovered band printed with its nap cost, judged against its stated limit)
    if let Some((h, load, lo, hi)) = o.high {
        let mut cells: Vec<String> = Vec::new();
        let mut short: Vec<(i64, u32)> = Vec::new();
        let (mut hr, mut fh, mut emin, mut entries) = (0u32, 0u32, i64::MAX, 0usize);
        for d in (0..12_000).step_by(250) {
            let evs = vec![(4_000 + (d - 3_000).max(0), Ev::Wx { rain: true, thunder: true }), (4_000 + d, Ev::Skip)];
            let s = scenario_days(&o, &evs, 5, &[0, 13], 4);
            let naps = s.naps_by_key.get(&2).map_or(0, |(a, _)| *a);
            hr += s.h_rephases;
            fh += s.false_h;
            emin = emin.min(s.entry_min);
            entries += s.entries_n as usize;
            cells.push(format!("{d}:{naps}"));
            if naps < 24 {
                short.push((d, naps));
            }
        }
        let inside = short.iter().all(|(d, _)| (H_STATED_LIMIT.0..=H_STATED_LIMIT.1).contains(d));
        let w_lim = v.bar(&format!("(b) + H: every skip time that loses naps lies inside the stated limit {}..{}", H_STATED_LIMIT.0, H_STATED_LIMIT.1), inside);
        let w_e = v.bar_n("the skip class, (b) + H: the earliest entry at dusk", entries, emin != i64::MAX && emin >= 12_000);
        lines.push(format!(
            "(b) + the high re-phase (H {h}, load C {load}, window C {lo}..{hi}): the daytime thunderstorm skip at every 250 gt of 0..11,750 (the storm from 3,000 gt before the skip or from day 0), the post-skip night's least naps (skip day time:naps) {}; H re-phases {hr} (false {fh}); the earliest entry {} ({w_e}); {}",
            cells.join(" "),
            if emin == i64::MAX { "-".to_string() } else { emin.to_string() },
            if short.is_empty() {
                format!("every skip time 24 naps ({w_lim})")
            } else {
                format!(
                    "uncovered: {} (naps lost up to {} at a skip there; against the stated limit {}..{}: {w_lim})",
                    short.iter().map(|(d, n)| format!("{d} ({n})")).collect::<Vec<_>>().join(", "),
                    short.iter().map(|(_, n)| 24 - n).max().unwrap_or(0),
                    H_STATED_LIMIT.0,
                    H_STATED_LIMIT.1
                )
            }
        ));
        // (G1 on 6b9be93ea point 4) the H path's positive controls: normal days with the window moved off the
        // natural C at the H rise (no skip: every rise is a false re-phase, which the count must see), and one
        // daytime skip with the true window (the re-phase the count must see)
        let off = CounterOpts { high: Some((h, load, hi + 10, hi + 20)), ..o.clone() };
        let s_off = scenario_days(&off, &[], 5, &[0, 13], 4);
        // (the skip's control isolates H: the drop latch and the falling-8 arm re-phase a daytime skip themselves, which
        // would leave H nothing to see)
        let h_only = CounterOpts { drop_min: None, arm: None, ..o.clone() };
        let s_skip = scenario_days(&h_only, &[(4_000 + 6_000, Ev::Skip)], 5, &[0, 13], 4);
        let w1 = v.control("the H window moved off the natural C: the false re-phase count sees them", s_off.runs, s_off.false_h > 0);
        let w2 = v.control("one daytime skip with the true H window (the drop latch and the arm left out): the re-phase count sees it", s_skip.runs, s_skip.h_rephases > 0);
        lines.push(format!(
            "  CONTROL: the H path's positive controls: the window moved off the natural C (C {}..{}), no skip: false re-phases {} ({w1}); one daytime skip at day time 6,000 with the true window, the drop latch and the arm left out (their own re-phase would leave H nothing to see): re-phases {} ({w2})",
            hi + 10,
            hi + 20,
            s_off.false_h,
            s_skip.h_rephases
        ));
    }
    // (ironspec 09:45: a skip at every gt offset of a napper's walk and nap in the stop, clear and
    // thunder: the outcome class, the picks deferred, the post-skip night's naps)
    let plain = run(&o, 3, 20_000, 0, 0, 100, &[]);
    match plain.entries.iter().find(|(d, _, _)| d.div_euclid(24_000) == 1 && d.rem_euclid(24_000) >= 12_000).map(|(d, _, _)| d - 20_000) {
        None => v.fail("the skip-during-a-stay rows: NOT JUDGED (the reference run has no entry on night 1)"),
        Some(te) => {
            for (wname, thunder) in [("clear", false), ("a thunderstorm from 2,000 gt before", true)] {
                let (mut a, mut b, mut n) = (0u32, 0u32, 0u32);
                let mut unclassed: Vec<i64> = Vec::new();
                let mut empty = 0u32;
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
                    // (G1's note on 6b9be93ea: a skip whose napper no class covers is printed, never dropped; G1 on
                    // 7897f4ba7 (1): a skip after the visit's own R, its cycle drawn per visit, finds the stop empty)
                    if r.skip_classes.is_empty() {
                        if r.skip_empty.is_empty() {
                            unclassed.push(since);
                        } else {
                            empty += 1;
                        }
                    }
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
                    for &(dd, w, class) in &r.entries {
                        bwait = bwait.max(w);
                        if !matches!(class, EntryClass::SkipStale | EntryClass::WalkPastRest) {
                            let dm = dd.rem_euclid(24_000);
                            emin = emin.min(if dm < 10 { dm + 24_000 } else { dm });
                        }
                    }
                }
                let w_cl = v.bar_n(&format!("the skip class, a skip during a stay ({wname}): every skip classed"), span as usize, unclassed.is_empty());
                let w_e = v.bar_n(&format!("the skip class, a skip during a stay ({wname}): the earliest entry at dusk"), usize::from(emin != i64::MAX), emin != i64::MAX && emin >= 12_000);
                lines.push(format!(
                    "a skip at every gt of a napper's stay in the stop ({wname}; the napper entered at night (game time {te}); the stop's walk {} gt, nap {} gt, exit {} gt (the cited 120 / 30, the cycle {}); skips at {span} offsets): class a (asleep: woken, picked up, the stop's release on the wake edge) {a}; class b (on foot when REST ends: waits for the next REST on a no-stamp cell, holding the stop until REST + a cycle) {b}; already leaving {n}; the stop already empty (the skip after the visit's own R, its cycle drawn per visit) {empty}; unclassed {}{} ({w_cl}); picks deferred by the back-pressure meanwhile {deferred}; the longest wait at the queue {bwait} gt; the post-skip night's naps {}..{} of 24 (priced, ironspec 01:46 PT (b)); the earliest entry at dusk {} ({w_e})",
                    o.stop_walk,
                    o.stop_nap,
                    o.stop_cycle - o.stop_walk - o.stop_nap,
                    o.stop_cycle,
                    unclassed.len(),
                    if unclassed.is_empty() { String::new() } else { format!(" (skips at the entry + {})", unclassed.iter().take(5).map(|s| s.to_string()).collect::<Vec<_>>().join(", ")) },
                    if nmin == u32::MAX { 0 } else { nmin },
                    nmax,
                    if emin == i64::MAX { "no entries".to_string() } else { emin.to_string() }
                ));
            }
        }
    }
    // (ironspec 09:19: the back-pressure on its own merits, against a no-back-pressure control; the chain audit: the
    // one-held, one-away and exposure bars hold by construction under the design's settings, so each has a control that
    // must break it through the same scenarios. At the design's rhythm the stop clears before the next arrival (its
    // entry -> R against the pick spacing), so the held and exposure controls also stall the stop (its cycle 600 gt, as a
    // stalled stay): without the back-pressure two are then held; ignoring R's ready, a release lands in the occupied bed)
    lines.push("the back-pressure against a no-back-pressure control (each scripted scenario; 20 release phases x 2 detector phases x 10 units phases):".to_string());
    let stalled = CounterOpts { stop_hist: None, stop_cycle: 600, ..o.clone() };
    let ctl = CounterOpts { backpressure: false, ..stalled.clone() };
    let ctl_latch = CounterOpts { lane_latch: false, ..o.clone() };
    let ctl_stop = CounterOpts { ignore_stop: true, ..stalled.clone() };
    let (mut held2, mut away2, mut exposed, mut nruns) = (0u32, 0u32, 0u32, 0usize);
    // (G1's Y8 on f21c9f600: each control's single-change TWIN, the design under the same 600-gt stall, run and judged:
    // the held and exposure controls then differ from it by exactly one knob)
    let (mut twin_held, mut twin_exposed, mut twin_runs) = (0u32, 0u32, 0usize);
    for (label, evs) in scen {
        let on = scenario_with(&o, evs, 5, &[0, 13]);
        let twin = scenario_with(&stalled, evs, 5, &[0, 13]);
        let off = scenario_with(&ctl, evs, 5, &[0, 13]);
        let nl = scenario_with(&ctl_latch, evs, 5, &[0, 13]);
        let ns = scenario_with(&ctl_stop, evs, 5, &[0, 13]);
        nruns += off.runs;
        twin_runs += twin.runs;
        twin_held = twin_held.max(twin.queue_max);
        twin_exposed += twin.exposure;
        held2 = held2.max(off.queue_max);
        away2 = away2.max(nl.lane_away_max);
        exposed += ns.exposure;
        let n = |x: u32| if x == u32::MAX { "-".to_string() } else { x.to_string() };
        lines.push(format!(
            "  {label}: held at the gate at once at most {} (the design with the stop stalled to 600 gt: {}; CONTROL that twin without the back-pressure: {}), picks deferred {}, naps a night least {} of 24 (control {}), the earliest entry {} (control {}); riders away from one lane at most {} (CONTROL without the away latch {}); lie-downs before the occupant's climb out {} (the stalled twin {}; CONTROL the twin ignoring R's ready: {})",
            on.queue_max, twin.queue_max, off.queue_max, on.deferred, n(on.min_naps), n(off.min_naps),
            if on.entry_min == i64::MAX { "-".to_string() } else { on.entry_min.to_string() },
            if off.entry_min == i64::MAX { "-".to_string() } else { off.entry_min.to_string() },
            on.lane_away_max, nl.lane_away_max, on.exposure, twin.exposure, ns.exposure
        ));
    }
    let w_tw = v.bar_n("the design under the 600-gt stall (the controls' twin): at most one held, no exposure", twin_runs, twin_held <= 1 && twin_exposed == 0);
    lines.push(format!("  => the controls' twin (the design with the stop stalled to 600 gt): held at most {twin_held}, exposures {twin_exposed} ({w_tw})"));
    let w_h = v.control("without the back-pressure two are held at the gate (the one-held bar can fail)", nruns, held2 >= 2);
    let w_a = v.control("without the away latch two are away from one lane (the one-away bar can fail)", nruns, away2 >= 2);
    let w_x = v.control("ignoring R's ready releases into an occupied bed (the exposure bar can fail)", nruns, exposed > 0);
    lines.push(format!("  => the controls: held {held2} ({w_h}); away {away2} ({w_a}); exposure {exposed} ({w_x})"));
    // (G1's Y7 on 10bcdf661: the exposure is judged from the OCCUPANT's own R, so a gate defect that frees the stop
    // early must show it: the stalled twin (the design under the 600-gt stall, judged above) with the gate keyed on the
    // occupant's lie-down (its entry + the model's walk), a single change)
    {
        let bug = CounterOpts { gate_at_liedown: true, ..stalled.clone() };
        let (mut ex, mut n, mut least) = (0u32, 0usize, i64::MAX);
        for (_, evs) in scen {
            let s = scenario_with(&bug, evs, 5, &[0, 13]);
            ex += s.exposure;
            n += s.runs;
            least = least.min(s.exposure_margin_min);
        }
        let w_g = v.control("a gate freeing the stop at the occupant's lie-down instead of its R's RS gives lie-downs before the occupant's own climb out", n, ex > 0);
        lines.push(format!(
            "  => CONTROL (G1's Y7): the stalled twin with its gate keyed on the occupant's lie-down (a planted defect, a single change, no ignore_stop): exposures {ex}, the least margin {} ({w_g})",
            if least == i64::MAX { "-".to_string() } else { format!("{least} gt") }
        ));
    }
    lines.push(format!(
        "restarts: {}",
        v.scope("a restart row in the counter model", "argued, not measured: a server stop halts the day time and every block tick together, and the lead's portal-ticket loader keeps the farm's chunks entity-ticking whenever the server runs, so a restart moves no edge against the clock; logins (L1, <= 3 gt) are the box's block benches' (piece 8)")
    ));
    lines
}

/// The bench: both variants, the scripted scenarios, ironspec's bars per variant. (G1 on 7897f4ba7 (1)) Every verdict
/// is split by class: normal operation (the scenarios without a skip) judged; the skip class (a skip in the scenario
/// or sweep) judged on its structural bars and priced on its naps; the no-reset variant printed as the rejected
/// comparison. (The chain fix) every bar and control through `v`, which fails the run.
pub fn bench(base: &CounterOpts, v: &mut crate::verdict::Verdict) -> Vec<String> {
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
        let mut rel_rest_max = i64::MIN;
        let mut rel_lane_away = 0u32;
        let mut rel_pick_entry = 0i64;
        let mut rel_stale_picks = 0u32;
        let mut rel_lane_skips = 0u32;
        let mut rel_latch_margin = i64::MAX;
        let mut rel_normal_naps = u32::MAX;
        let mut rel_add = |s: &ScenOut| {
            rel_latch_margin = rel_latch_margin.min(s.latch_margin_min);
            rel_entry_min = rel_entry_min.min(s.entry_min);
            rel_exit = rel_exit.min(s.exit_gap_min);
            rel_liedown = rel_liedown.min(s.liedown_gap_min);
            rel_rest_max = rel_rest_max.max(s.rest_entry_max);
            rel_lane_away = rel_lane_away.max(s.lane_away_max);
            rel_pick_entry = rel_pick_entry.max(s.pick_entry_max);
            rel_stale_picks += s.stale_picks;
            rel_lane_skips += s.lane_skips;
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
        // (G1 on 7897f4ba7 (1): the scenarios by class)
        let mut norm_ss: Vec<(String, ScenOut)> = Vec::new();
        let mut skip_ss: Vec<(String, ScenOut)> = Vec::new();
        for (label, evs) in &scen {
            let s = scenario(o, evs);
            if evs.iter().any(|(_, e)| matches!(e, Ev::Skip)) {
                skip_ss.push((label.to_string(), s.clone()));
            } else {
                norm_ss.push((label.to_string(), s.clone()));
            }
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
                    latch_margin_min: i64::MAX,
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
                    skip_ss.push((format!("a skip at {skip_day}, {wname} from 11,500"), s.clone()));
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
            skip_ss.push(("a daytime thunderstorm skip (6,000; the storm from 3,000)".to_string(), s.clone()));
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
        // (G1 on 7897f4ba7 (1): the verdicts by class; the design variant judged, the no-reset variant the comparison)
        let design = o.variant == Variant::Ff;
        lines.push(class_bars(o, &norm_ss, "NORMAL OPERATION (the scenarios without a skip)", if design { 0 } else { 2 }, v));
        lines.push(class_bars(o, &skip_ss, "THE SKIP CLASS (the scenarios and sweeps with a skip: structural bars judged, naps and early picks priced under the skip ruling)", if design { 1 } else { 2 }, v));
        let _ = (worst_night, worst_early, worst_gate, worst_spacing, bad_counts, daylight_total, night_inh_total, worst_edge, normal_min_night, false_total, worst_coupling);
        lines.push(format!("  (skips inside the window: {daylight_held} picks held at the queue until dusk: priced, ironspec 02:28)"));
        if let Some((rs, rr)) = o.rest {
            // (ironspec 09:19: the current bars replace the dawn bar (<= 23,900): option A's outcome over
            // the stale window (58f770cc's rows; here the entries inside one, which the hold must leave
            // at 0), the earliest bed-stop entry >= 12,000, one napper held at most)
            let m = |x: i64, none: i64| if x == none { "-".to_string() } else { x.to_string() };
            lines.push(format!(
                "  the release over every scenario and sweep above, both classes together (for reference; its verdicts are the class lines that follow) (REST_l = C >= {rs} or C < {rr}; every napper held >= {} gt at the queue; NOT DAY as the hatch sees it after the hold; the stop clear (R's RS; the stop's cycle {} gt); the back-pressure {}): the earliest bed-stop entry at dusk {} (bar >= 12,000: {}); entries outside REST not after a skip {rel_outside} ({}); stale admissions {rel_stale} (entered in a skip's stale window at day time {}; each waits on the stop's ledge until REST, naps that evening and leaves, the stop clear at day time {}: ironspec 09:31, option A's outcome rows are 58f770cc's); the least gap between consecutive departures from the stop {} gt (inherited, no measured failure: ironspec 10:10's hold bar, its longest wait + 12 + clear + 20, against it with 7b90c3f1's trip spread); the least time from a napper's R to the next napper's lie-down {} gt (inherited bar 20, retired by ironspec 19:07 PT for (a1) measured directly: ironspec 10:52, HOLD dropped: the departing napper leaves the 16-ball within it, 58f770cc's R-to-ball-exit); the latest entry made in REST at night time {} (the stop's entry: the release + to_entry, G1 on 12c106916; G1: a later one is on foot when REST ends, classed stale, holding the stop to the next REST; entries in a skip's stale window are that class; bar < 24,010 - stop_walk = {}: {}); riders away from one lane at once at most {} (ironspec 12:13: bar 1, the lane's 7 of 8 in rotation at every summon, read at every pick; back in lane {}, the longest pick to stop entry {} gt: {}); held at the gate at once at most {rel_queue} (bar 1: {}); picks deferred {rel_deferred}; picks made while a stale napper held the stop outside REST {rel_stale_picks} ({}); picks the per-lane away latch skipped {rel_lane_skips} (ironspec 23:21 PT: the away latch's least margin, a lane's pick against the return of the napper picked one lane period before, {}: a latch still set at the pick is a skipped pick: {}); each lane's pick opportunities in the window C {}..{}: {} against its 8 riders (slack {}); normal nights' naps least {} of 24 ({})",
                o.stale_hold,
                match &o.stop_hist {
                    Some(h) => {
                        let (a, b) = crate::napper::hist_range(h);
                        format!("drawn per visit from the measured distribution, {a}..{b}")
                    }
                    None => o.stop_cycle.to_string(),
                },
                if !o.backpressure { "OFF".to_string() } else { format!("on{}{}{}{}", if o.held_at_divert { " (HELD sampled at the divert, day-1's (i))" } else { " (HELD sampled at the pick)" }, if o.to_entry > 0 { format!(", the release {} gt before the stop's entry", o.to_entry) } else { String::new() }, if o.stale_backpressure { ", with the stale hold (ironspec 12:54: no pick while the queue is occupied OR the stop is held AND NOT REST_l)" } else { "" }, if o.lane_latch { ", with the per-lane away latch (no pick from a lane while its napper is away)" } else { "" }) },
                m(rel_entry_min, i64::MAX),
                if rel_entry_min != i64::MAX && rel_entry_min >= 12_000 { "within" } else { "outside: see the class lines" },
                if rel_outside == 0 { "within" } else { "outside: see the class lines" },
                if rel_stale == 0 { "-".to_string() } else { format!("{}..{}", rel_admit_dt.0, rel_admit_dt.1) },
                if rel_stale == 0 { "-".to_string() } else { format!("{}..{}", rel_clear_dt.0, rel_clear_dt.1) },
                m(rel_exit, i64::MAX),
                m(rel_liedown, i64::MAX),
                m(rel_rest_max, i64::MIN),
                24_010 - o.stop_walk,
                if rel_rest_max != i64::MIN && rest_entry_ok(rel_rest_max, o.stop_walk) { "within" } else { "outside: see the class lines" },
                rel_lane_away,
                match &o.back_hist {
                    Some(h) => {
                        let (a, b) = crate::napper::hist_range(h);
                        format!("the measured round trip, the pick -> back in its lane, per visit ({a}..{b} gt; never before the visit's R)")
                    }
                    None => format!("{} gt after the entry", o.back_after),
                },
                rel_pick_entry,
                if rel_lane_away <= 1 { "within" } else { "outside: see the class lines" },
                if rel_queue <= 1 { "within" } else { "outside: see the class lines" },
                if !o.stale_backpressure { "the stale-hold back-pressure off" } else if rel_stale_picks == 0 { "ironspec 12:54, bar 0 under the stale-hold back-pressure: within" } else { "ironspec 12:54, bar 0: outside: see the class lines" },
                if rel_latch_margin == i64::MAX { "-".to_string() } else { format!("{rel_latch_margin} gt") },
                if rel_lane_skips == 0 { "bar 0: within" } else { "bar 0: outside, see the class lines" },
                o.window.0,
                o.window.1,
                {
                    let opps: Vec<i64> = [0i64, 3, 6].iter().map(|&off| (o.window.1 - o.window.0 - off + 9).div_euclid(10)).collect();
                    opps.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" / ")
                },
                {
                    let s = [0i64, 3, 6].iter().map(|&off| (o.window.1 - o.window.0 - off + 9).div_euclid(10) - 8).min().unwrap_or(0);
                    if s <= 0 { format!("{s}: NO SLACK, any skipped pick costs a nap") } else { s.to_string() }
                },
                if rel_normal_naps == u32::MAX { "-".to_string() } else { rel_normal_naps.to_string() },
                if rel_normal_naps >= 24 && rel_normal_naps != u32::MAX { "within" } else { "outside: see the class lines" }
            ));
            lines.push(class_release(o, &norm_ss, "NORMAL OPERATION (the scenarios without a skip)", if design { 0 } else { 2 }, v));
            lines.push(class_release(o, &skip_ss, "THE SKIP CLASS (the scenarios and sweeps with a skip)", if design { 1 } else { 2 }, v));
        } else {
            let sc = v.scope(&format!("{vname}: the release bars"), "--no-release: the plain hatch, no queue release in this run");
            lines.push(format!("  {sc}"));
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
        lines.extend(release_report(base, &scen, v));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The design's release settings for the model's tests: REST_l C >= 138 or < 15, the day inhibit at L 6, the
    /// queue's hold 16, the release 112 gt before the stop's entry.
    fn design() -> CounterOpts {
        CounterOpts { day_inhibit: Some(6), rest: Some((138, 15)), stale_hold: 16, to_entry: 112, ..Default::default() }
    }

    /// (G1's audit hole 24, the boundary's edge control) an entry whose walk ends exactly at REST's end (24,010) is
    /// classed walk-past and fails the latest-entry bar; one tick earlier passes both.
    #[test]
    fn walk_past_and_the_latest_entry_bar_agree_at_the_boundary() {
        let walk = 120;
        let rest_end = 24_010;
        let at_edge = 24_010 - walk;
        assert!(at_edge + walk >= rest_end);
        assert_eq!(entry_class(false, at_edge + walk >= rest_end, false), EntryClass::WalkPastRest);
        assert!(!rest_entry_ok(at_edge, walk));
        assert_eq!(entry_class(false, at_edge - 1 + walk >= rest_end, false), EntryClass::InRest);
        assert!(rest_entry_ok(at_edge - 1, walk));
        // (outside REST: excused only inside a skip's window)
        assert_eq!(entry_class(true, false, true), EntryClass::SkipStale);
        assert_eq!(entry_class(true, false, false), EntryClass::OutsideRest);
    }

    /// (G1's audit hole 22) the injected control entry goes through the model's own classifier: by day with no skip it is
    /// classed outside REST; long after a skip's measured window, outside REST too.
    #[test]
    fn the_injected_entry_is_classed_by_the_model() {
        let o = CounterOpts { inject_entry: Some(6_000), ..design() };
        let r = run(&o, 3, 20_000, 0, 0, 100, &[]);
        assert_eq!(r.injected_class, Some(EntryClass::OutsideRest));
        assert!(r.entries.iter().any(|e| e.2 == EntryClass::OutsideRest));
        let o2 = CounterOpts { inject_entry: Some(1_000), ..design() };
        let r2 = run(&o2, 3, 20_000, 0, 0, 100, &[(2_000, Ev::Skip)]);
        assert!(!r2.skip_windows.is_empty());
        assert_eq!(r2.injected_class, Some(EntryClass::OutsideRest));
    }

    /// (ironspec 04:07 PT) the forced skip takes an opportunity that would have picked: the run without it picked at the
    /// skipped tick, the run with it did not (the later dynamics differ, so the latch's skip count is no invariant).
    #[test]
    fn the_forced_skip_always_changes_its_target() {
        let o = design();
        let base = run(&o, 4, 20_000, 0, 0, 100, &[]);
        let oi = CounterOpts { skip_at: Some(40_000), ..design() };
        let r = run(&oi, 4, 20_000, 0, 0, 100, &[]);
        let ts = r.skipped_at.expect("the forced skip took no opportunity");
        assert!(ts >= 40_000);
        assert!(base.pick_ts.contains(&ts), "the run without the skip did not pick at {ts}");
        assert!(!r.pick_ts.contains(&ts), "the run with the skip still picked at {ts}");
    }

    /// (ironspec 06:31 PT; G1's Y7 / Y8) the exposure, judged from the occupants' own timelines: 0 with R's ready in the
    /// release, at the design's rhythm and with the stop stalled to 600 gt (the control's single-change twin), and positive
    /// when that stalled twin ignores R's ready or its gate frees the stop at the occupant's lie-down (the controls).
    #[test]
    fn the_exposure_and_its_control() {
        let r = run(&design(), 3, 20_000, 0, 0, 100, &[]);
        assert_eq!(r.exposure, 0);
        let stalled = CounterOpts { stop_hist: None, stop_cycle: 600, ..design() };
        let twin = run(&stalled, 3, 20_000, 0, 0, 100, &[]);
        assert_eq!(twin.exposure, 0);
        let c = run(&CounterOpts { ignore_stop: true, ..stalled.clone() }, 3, 20_000, 0, 0, 100, &[]);
        assert!(c.exposure > 0);
        let g = run(&CounterOpts { gate_at_liedown: true, ..stalled }, 3, 20_000, 0, 0, 100, &[]);
        assert!(g.exposure > 0, "a gate freeing the stop at the lie-down must expose the next napper");
    }
}
