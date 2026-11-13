//! The return judge (cc767b78; ironspec's L1‴ rows, farmopt 2026-11-12 03:22 with ironspec's
//! amendments 03:23, the measured bars 05:31): the judge of 7b90c3f1's return (`return-bench`: the
//! merge, the shared lift and the in-chunk split of F1 + Q2 at 24, `--row9`, in the L1 mode
//! `--game-ticks`). The lead's rows: per corner (the launch offsets with the measured departure spread,
//! the pocket release speed, the napper's delay) the control — the cold start: the return starts empty,
//! its latches idle, and nothing in it keys on world time, so every bar rides the batch's own launches
//! (REL) — and at the nominal corners a save/stop/start at every 1-gt phase in each of
//! `liveness::Restart::rows()`'s 10 orders (the bench's `--restart`), and logins of D gt at every phase
//! (the bench's `--login`, through `liveness::freeze32`): up to `--lead-login-max` (3) gt the lead's
//! (ironspec 09:26: a restart's riders officially miss 1 tick, the brief's bound <= 3 gt), longer the
//! pearl alternative's stated limit (failures by gap length, the first failing gap). The
//! bars: every cart in its own lane (no WRONG LANE, no NEVER), seated, unhurt, its eye never in a wall,
//! its rider's head under the roof's underside (`--head-max`, 28.0); every batch cart's delivery (stored,
//! entering its lane's merge cell: ironfarm's drop entry) in `--branch-band` (0.27..0.95: the floor
//! ironfarm's real first failure 0.22 + 0.05, the upper end ironfarm's upward sweep: no failure through
//! stored 1.00, less 0.05) and the napper's (stored, handed on by the 4th route toward the bed stop) in
//! `--napper-band` (0.1..0.3; with `--k-relay` its arrival on its siding's K cell in `--k-band`,
//! 0.10..0.44, read from the bench's trace of the napper), each
//! with its margin to both ends; every shaft-entry gap at least `--shaft-min`; the napper's window at
//! least `--napper-behind` behind the batch's third cart and `--napper-ahead` ahead of the next batch's
//! first; each junction's turn at least `--junction-min J:G` ahead of the next cart at it; box-test
//! pushes at most `--pushes-max` (0). Every gt bar keeps `--margin` (2) gt at the nominal corners. The
//! static gates on the emitted split file (the bench's `--out`): its own (parse(write()), every T's
//! verdict and sources, the detectors' Ts), and here `redsim::rails::would_pop` over every rail of the
//! file and every detector's reach set (the faces of its cell, of the cell under it, and for a sloped
//! detector of the cell above it) with each rail's connection count; a T in a reach set must be
//! declared (a driver on a face: a lever, a repeater or comparator pointing in, any source; a detector
//! alone is none); every switched T obeys ironspec's T rule (`t_rule`). A value the reader cannot read
//! is an error (nothing silently dropped).

use crate::liveness::Restart;
use crate::returnbench::{ReturnOpts, return_bench};
use std::collections::BTreeMap;

/// The bars.
#[derive(Clone, Debug)]
pub struct ReturnBars {
    /// The batch carts' delivery band (stored, entering their lane's merge cell).
    pub branch_band: (f64, f64),
    /// The napper's delivery band (stored, its hand-on by the 4th route).
    pub napper_band: (f64, f64),
    pub shaft_min: i64,
    pub napper_behind: i64,
    pub napper_ahead: i64,
    /// Per junction (its x), the least gap from its turn to the next cart at it.
    pub junction_min: BTreeMap<i32, i64>,
    pub pushes_max: usize,
    /// Every rider's head strictly under this (the roof's underside).
    pub head_max: f64,
    /// `--k-relay`: the returning napper's arrival on its siding's K cell (ironspec 08:46, widened 09:00:
    /// 0.10..0.44) in place of the 4th route's hand-on band.
    pub k_band: Option<(f64, f64)>,
    /// A departure off a pulse's rise (the bench's "NOT on the hold at any pulse") is a failure in itself:
    /// (c′) (ironspec 11:43: presence-gated, sensed at the hold and locked at E's rise) departs on the
    /// fired pulse; off only for (b) (ironspec 10:09, superseded: the taps keyed to the actual departure),
    /// where such a departure is judged by its slot margins and the state and only counted.
    pub scheduled_only: bool,
    /// Under the bench's `--pulse-gate` ((c′)): every napper run must carry the bench's `hold release:`
    /// line (a run without it fails: its hold bars would go unjudged; G1's CHANGES 1 on 67d161436), and a
    /// presence sample on a rolling cart at E's rise fails (the brief's (c′) rests on presence only ever
    /// sampled on a cart at rest; G1's note on 67d161436).
    pub hold_required: bool,
    /// The margin a nominal corner needs over each measured gt bar (the judge's `--margin`, 2), and the
    /// junctions' settle (and timed clear) sides in every row (ironspec 17:20 (b): >= the bar + 2).
    pub margin: i64,
    /// The physical junction bars by the phase of a switch's cause (ironspec 19:08/19:14, measured and final
    /// by tick-exact's official boundary row, 26.3, `mctas mobai t-switch`, farmopt 19:58): block-phase (a
    /// repeater or comparator output, a detector's release, the bench's between-tick lever: 1 gt) and
    /// entity-phase (a detector press, a box overlap: 2 gt); each switch's settle and clear >= its bar +
    /// `margin` in every row; a cause the bench does not name takes the entity-phase bar.
    pub junction_block_min: i64,
    pub junction_entity_min: i64,
    /// Day-1's junction driver (ironspec 19:38: its set and reset inputs through repeaters, an RS latch, a
    /// driver repeater into the junction's cell): a switch caused by a detector press lands this many gt
    /// after the press, (set edge, reset edge) = (4, 6), as a block-phase change. Applied to each per-cause
    /// entry the bench times from the press tick (settle - L, clear + L, the block-phase bar); None: the
    /// press entries as the bench prints them.
    pub driver_latency: Option<(i64, i64)>,
    /// Path (a) (ironspec 17:20 (a)): the shaft-entry napper bars retired; the napper's y-12 pass gaps
    /// behind lane 7's cart and ahead of the next batch's first barred at `y12_nap_min` (23) in every
    /// row, the batch carts' y-12 pairs at `y12_min`, from the bench's `path (a):` line.
    pub path_a: bool,
    pub y12_nap_min: i64,
    pub y12_min: i64,
    /// The column-order bar (ironspec 20:01, physical): lane 7's entry at y 12 at least this many gt after
    /// the preceding cart's box fully clears the mouth cell (the bench's mouth clearance), in every row.
    pub mouth_min: i64,
}

/// One run's reading of the bench's text.
#[derive(Clone, Debug, Default)]
pub struct ReturnRead {
    pub carts: usize,
    pub wrong: usize,
    pub never: usize,
    pub dismounted: usize,
    pub hurt: usize,
    pub eye: usize,
    /// Batch carts that took a siding's F or the 4th route (the bench's "handed on" speed) instead of
    /// their own branch (each a failure: their lane's number can match while their route does not).
    pub sided: usize,
    /// The batch carts' stored speed entering their branch (printed, not a bar).
    pub branch: Vec<f64>,
    /// The batch carts' stored speed entering their lane's merge cell (the delivery).
    pub merge: Vec<f64>,
    /// The napper's hand-on speed by the 4th route (stored).
    pub napper_entry: Option<f64>,
    /// `--k-relay`: the napper's stored speed on arriving on its siding's K cell (read from the bench's
    /// trace of the napper: the first tick in that cell).
    pub napper_k: Option<f64>,
    /// The riders' highest head y.
    pub head: Option<f64>,
    /// Per junction (its x), the least gap from its turn to the next cart at it.
    pub junction: BTreeMap<i32, i64>,
    pub shaft: Option<i64>,
    pub napper_behind: Option<i64>,
    pub napper_ahead: Option<i64>,
    /// The gaps line carries the napper's clause (its values may be `-`: the napper or the next batch's
    /// first never into the shaft, a failure).
    pub napper_clause: bool,
    /// The napper reached its lane by the lane's own branch (`branch speed V`), not handed on into its
    /// siding: it took a batch cart's path (a failure).
    pub napper_by_branch: bool,
    /// The bench's "(c)'s napper lift" line was read (`--napper-lift`).
    pub lift_seen: bool,
    /// From that line: whether the napper was on the hold at a pulse's rise (a scheduled departure);
    /// `Some(false)`: "NOT on the hold at any pulse" (a departure outside a scheduled window, or with a
    /// presence gate, never departed).
    pub napper_on_hold: Option<bool>,
    /// The bench's `hold release:` line ((c′), 7b90c3f1's amend), when printed.
    pub hold: Option<HoldRelease>,
    /// The bench's `path (a):` line: the napper's y-12 pass gaps behind and ahead, the batch carts' y-12
    /// pairs (lane 3 -> lane 7 in a batch; lane 7 -> the next batch's lane -1), the least distance
    /// between two carts in the column.
    pub saw_path_a: bool,
    pub y12_behind: Option<i64>,
    pub y12_ahead: Option<i64>,
    pub y12_pairs: Vec<i64>,
    /// Batch carts the path (a) line prints without a y-12 pass (`-`: never passed y 12).
    pub y12_missing: usize,
    /// The bench's `path (a) y-12 pairs` line: [lane -1 -> 3, 3 -> 7, 7 -> the napper, the napper -> the next
    /// lane -1, 7 -> the next lane -1 with no napper between], the column order wrong, and the same pairs at
    /// the exit top (the contact point; printed).
    pub saw_y12_pairs: bool,
    pub y12_bench: [Option<i64>; 5],
    pub y12_order_wrong: bool,
    pub y12_top: [Option<i64>; 4],
    /// The bench's mouth clearance on the `path (a):` line (ironspec 20:01): the least, over the batches, of
    /// lane 7's entry at y 12 after the preceding cart's box fully cleared the mouth cell (gt).
    pub mouth: Option<i64>,
    /// Each batch cart's y-12 pass after its batch's REL: from the `path (a):` line (`bB lane L +X`), keyed
    /// (batch, lane); and each cart line's "the rider's feet at y 12.01 at N" (N after the cart's own
    /// launch), keyed (cart index, batch, lane), for the station calibration check without path (a).
    pub y12_rel: BTreeMap<(i64, i32), i64>,
    pub feet12: BTreeMap<(usize, i64, i32), i64>,
    pub col_min: Option<f64>,
    /// The bench's `junction settle / clear` line: per junction its least settle (a switch to the first
    /// cart needing the new state) and least clear (the last cart needing the old state to the switch),
    /// the clear split into the timed switches and the cart-triggered ones (ironspec 18:16).
    pub saw_settle: bool,
    pub j_settle: BTreeMap<i32, i64>,
    pub j_clear: BTreeMap<i32, i64>,
    pub j_clear_cart: BTreeMap<i32, i64>,
    /// The bench's `junction settle / clear by cause` line (ironspec 19:14): one entry per switch cause
    /// (K(n) set / reset, J(-1) re-arm / set / reset, J(3) set / reset) with the phase of its cause and
    /// its least settle and least clear over the run.
    pub switches: Vec<SwitchGap>,
    pub touches: usize,
    pub pushes: usize,
    /// Per lane, the longest return (release to its branch flat, live gt).
    pub ret: BTreeMap<String, i64>,
    /// Lines the judge needs and did not find.
    pub missing: Vec<&'static str>,
    /// Values the judge could not read (each an error: nothing silently dropped).
    pub errors: Vec<String>,
}

/// The number right after `key` in `l` (spaces skipped); `None` when `key` is absent or no number
/// follows (a `-`).
fn num_after(l: &str, key: &str) -> Option<f64> {
    let i = l.find(key)? + key.len();
    let s: String = l[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    s.trim_end_matches('.').parse().ok()
}

/// [`num_after`], or an error recorded.
fn need(l: &str, key: &str, what: &str, errors: &mut Vec<String>) -> Option<f64> {
    let v = num_after(l, key);
    if v.is_none() {
        errors.push(format!("{what}: no number after {key:?} in {:?}", l.chars().take(140).collect::<String>()));
    }
    v
}

/// The bench's `hold release:` line ((c′), 7b90c3f1's amend): the pulses fired (and those while the
/// napper rested), the departure on a fired pulse, creeps, FIFO pops without a departure, fired pulses
/// without that cycle's departure, and each presence sample at E's rise.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HoldRelease {
    pub fired: usize,
    /// The fired pulses' numbers while the napper rested on the hold.
    pub fired_in_rest: Vec<usize>,
    pub skipped: usize,
    /// `Some(true)`: the departure on a fired pulse; `Some(false)`: NOT on a fired pulse; `None`: it
    /// never left.
    pub on_fired: Option<bool>,
    /// The departure's ARM3 offset, when on a fired pulse.
    pub departure_arm3: Option<i64>,
    pub creeps: usize,
    pub pops_without: usize,
    pub pulses_without: usize,
    /// Presence samples at E's rise: (rolling, speed); `rolling` true for "rolling on the hold".
    pub rolling_samples: Vec<f64>,
    /// The napper's second ARM3 rise (ironspec 11:49: its own pass over the arming detector, ≈ ARM3 + 31):
    /// `None` when the line has no such field; `Some(Some(true))` fired a pulse (a phantom),
    /// `Some(Some(false))` fired nothing, `Some(None)` no second rise in the run.
    pub second_rise: Option<Option<bool>>,
    /// Every E rise (arming rise + 2, fired or not) and every fired pulse's rail rise, when printed
    /// (path (a), ironspec 14:41), and the bench's own count of E rises within K gt after a fired pulse
    /// (K, the count).
    pub e_rises: Vec<i64>,
    pub fired_rises: Vec<i64>,
    pub e_within: Option<(i64, usize)>,
    /// The fired pulses' E rises (the resets: the tick the waiting bit is reset), when printed (G1's note
    /// on 7ef404af3, ironspec 18:01: the E rises counted in (reset, reset + k]).
    pub resets: Vec<i64>,
}

/// The count right after `key` (spaces skipped); an error when absent or not a number.
fn count_after(l: &str, key: &str, errors: &mut Vec<String>) -> usize {
    match l.find(key).map(|i| &l[i + key.len()..]).and_then(|s| s.trim_start().split(|c: char| !c.is_ascii_digit()).next().and_then(|d| d.parse::<usize>().ok())) {
        Some(n) => n,
        None => {
            errors.push(format!("hold release: no count after {key:?}"));
            0
        }
    }
}

/// Read the bench's `hold release:` line (the text after the prefix).
pub fn read_hold(rest: &str, errors: &mut Vec<String>) -> HoldRelease {
    let mut h = HoldRelease { fired: count_after(rest, "pulses fired ", errors), skipped: count_after(rest, "skipped ", errors), ..Default::default() };
    // (`(while the napper rested: #i[, #j …] | none)`)
    match rest.split_once("(while the napper rested: ").and_then(|(_, r)| r.split_once(')')) {
        Some((list, _)) if list.trim() == "none" => {}
        Some((list, _)) => {
            for p in list.split(", ") {
                match p.trim().trim_start_matches('#').parse::<usize>() {
                    Ok(i) => h.fired_in_rest.push(i),
                    Err(_) => errors.push(format!("hold release: a pulse number {p:?} unreadable")),
                }
            }
        }
        None => errors.push("hold release: no \"while the napper rested\" list".to_string()),
    }
    // (`the napper's departure on fired pulse #k at t T (ARM3 + X) | NOT on a fired pulse | none (it never left)`)
    match rest.split_once("the napper's departure ").map(|(_, r)| r) {
        Some(r) if r.starts_with("on fired pulse") => {
            h.on_fired = Some(true);
            h.departure_arm3 = r.split_once("(ARM3 + ").and_then(|(_, x)| x.split(|c: char| !(c.is_ascii_digit() || c == '-')).next()).and_then(|x| x.parse().ok());
            if h.departure_arm3.is_none() {
                errors.push("hold release: the departure's ARM3 offset unreadable".to_string());
            }
        }
        Some(r) if r.starts_with("NOT on a fired pulse") => h.on_fired = Some(false),
        Some(r) if r.starts_with("none") => h.on_fired = None,
        _ => errors.push("hold release: the departure unreadable".to_string()),
    }
    h.creeps = count_after(rest, "creeps ", errors);
    h.pops_without = count_after(rest, "FIFO pops without a departure ", errors);
    h.pulses_without = count_after(rest, "fired pulses without that cycle's departure ", errors);
    // (`the napper's second ARM3 rise at t T: fired #k | nothing` or `... none (no second rise)`)
    if let Some((_, r)) = rest.split_once("the napper's second ARM3 rise ") {
        let verdict = r.split_once(": ").map_or(r, |(_, v)| v).trim_start();
        h.second_rise = Some(if verdict.starts_with("fired") {
            Some(true)
        } else if verdict.starts_with("nothing") {
            Some(false)
        } else if r.starts_with("none") || verdict.starts_with("none") {
            None
        } else {
            errors.push(format!("hold release: the napper's second ARM3 rise unreadable: {:?}", r.chars().take(60).collect::<String>()));
            None
        });
    }
    // (`presence samples at E's rise: t …: at rest on the hold (v) | rolling on the hold (v) | not on the hold (v); …`)
    if let Some((_, samples)) = rest.split_once("presence samples at E's rise: ") {
        for s in samples.split("; ").filter(|s| s.contains("rolling on the hold")) {
            match s.split_once("rolling on the hold (").and_then(|(_, v)| v.split(')').next()).and_then(|v| v.trim().parse::<f64>().ok()) {
                Some(v) => h.rolling_samples.push(v),
                None => errors.push(format!("hold release: a rolling sample's speed unreadable in {s:?}")),
            }
        }
    }
    // (path (a), ironspec 14:41: `; E rises: t a, t b, …; fired pulse rises: t p, t q, …; E rises within K
    // gt after a fired pulse: N (t …)`; each list `none` when empty)
    let ticks = |key: &str, errors: &mut Vec<String>| -> Vec<i64> {
        let Some((_, r)) = rest.split_once(key) else { return Vec::new() };
        let list = r.split(';').next().unwrap_or("").trim();
        if list == "none" || list.is_empty() {
            return Vec::new();
        }
        list.split(", ")
            .filter_map(|t| match t.trim().trim_start_matches("t ").parse::<i64>() {
                Ok(x) => Some(x),
                Err(_) => {
                    errors.push(format!("hold release: a tick {t:?} unreadable after {key:?}"));
                    None
                }
            })
            .collect()
    };
    h.e_rises = ticks("; E rises: ", errors);
    h.fired_rises = ticks("; fired pulse rises: ", errors);
    h.resets = ticks("; fired E rises (resets): ", errors);
    if let Some((_, r)) = rest.split_once("E rises within ") {
        let k = r.split(' ').next().and_then(|x| x.parse::<i64>().ok());
        let n = r
            .split_once("after a fired pulse: ")
            .or_else(|| r.split_once("after a reset: "))
            .and_then(|(_, x)| x.split(|c: char| !c.is_ascii_digit()).next())
            .and_then(|x| x.parse::<usize>().ok());
        match (k, n) {
            (Some(k), Some(n)) => h.e_within = Some((k, n)),
            _ => errors.push(format!("hold release: the E rises within K line unreadable: {:?}", r.chars().take(80).collect::<String>())),
        }
    }
    h
}

/// The bench's `path (a):` line (7b90c3f1's path (a); ironspec 17:20 (a)): `… y-12 passes after REL: b0 lane
/// -1 +59, b0 lane 3 +80, b0 lane 7 +106, …, napper +133; the napper's y-12 gap behind N and ahead M; the
/// least distance between two carts in the column D (…)`. The batch pairs: lane 3 -> lane 7 in each batch,
/// and lane 7 -> the next batch's lane -1 (the batches 100 gt apart); with the napper between, that pair
/// holds both its gaps, so it stays well over the bar.
fn read_path_a(rest: &str, r: &mut ReturnRead) {
    r.saw_path_a = true;
    if let Some((_, list)) = rest.split_once("y-12 passes after REL: ") {
        let list = list.split(';').next().unwrap_or("");
        let mut by: BTreeMap<(i64, i32), i64> = BTreeMap::new();
        for e in list.split(", ").map(str::trim).filter(|e| !e.is_empty()) {
            if e.starts_with("napper") {
                continue;
            }
            // (`bB lane L +X`, or `bB lane L -`: the cart never passed y 12 (7b90c3f1's 5ec2749b3))
            let parts: Vec<&str> = e.split_whitespace().collect();
            let b = parts.first().and_then(|b| b.strip_prefix('b')).and_then(|b| b.parse::<i64>().ok());
            let lane = parts.get(2).and_then(|l| l.parse::<i32>().ok());
            match (b, lane, parts.get(3).copied()) {
                (Some(_), Some(_), Some("-")) if parts.get(1) == Some(&"lane") => r.y12_missing += 1,
                (Some(b), Some(lane), Some(x)) if parts.get(1) == Some(&"lane") && x.trim_start_matches('+').parse::<i64>().is_ok() => {
                    by.insert((b, lane), x.trim_start_matches('+').parse::<i64>().unwrap_or(0));
                }
                _ => r.errors.push(format!("path (a): a y-12 pass {e:?} unreadable")),
            }
        }
        let batches: std::collections::BTreeSet<i64> = by.keys().map(|(b, _)| *b).collect();
        r.y12_rel = by.clone();
        for b in &batches {
            if let (Some(x3), Some(x7)) = (by.get(&(*b, 3)), by.get(&(*b, 7))) {
                r.y12_pairs.push(x7 - x3);
            }
            if let (Some(x7), Some(xn)) = (by.get(&(*b, 7)), by.get(&(b + 1, -1))) {
                r.y12_pairs.push(100 + xn - x7);
            }
        }
    }
    // (a value or `-` (none measured: no error; the bars read it as unmeasured))
    let num = |s: &str| s.trim_start().split(|c: char| !(c.is_ascii_digit() || c == '-')).next().filter(|x| *x != "-").and_then(|x| x.parse::<i64>().ok());
    if let Some((_, g)) = rest.split_once("the napper's y-12 gap behind ") {
        r.y12_behind = num(g);
        r.y12_ahead = g.split_once(" and ahead ").and_then(|(_, a)| num(a));
    }
    r.col_min = rest.split_once("the least distance between two carts in the column ").and_then(|(_, d)| d.split(|c: char| !(c.is_ascii_digit() || c == '.')).next()).and_then(|x| x.parse::<f64>().ok());
    // (`… lane 7's mouth clearance N gt (…)` or `- (unmeasured)` (ironspec 20:01; 7b90c3f1's wording))
    if let Some((_, m)) = rest.split_once("mouth clearance ") {
        let m = m.trim_start();
        match num(m) {
            Some(n) => r.mouth = Some(n),
            None if m.starts_with('-') => r.mouth = None,
            None => r.errors.push(format!("path (a): the mouth clearance unreadable: {:?}", m.chars().take(40).collect::<String>())),
        }
    }
}

/// The bench's `path (a) y-12 pairs (bar >= N, G1): lane -1 -> lane 3 A (…), lane 3 -> lane 7 B, lane 7 ->
/// the napper C, the napper -> the next batch's lane -1 D, lane 7 -> the next batch's lane -1 with no
/// napper between E; the column order at y 12 WRONG | …; the same pairs at the exit top (…): lane -1 ->
/// lane 3 a, lane 3 -> lane 7 b, lane 7 -> the napper c, the napper -> the next lane -1 d` line
/// (7b90c3f1's 5ec2749b3): the very numbers its clean() bars (each the least over the run; `-` none).
fn read_y12_pairs(rest: &str, r: &mut ReturnRead) {
    r.saw_y12_pairs = true;
    let num = |s: &str| -> Option<i64> { s.trim_start().split(|c: char| !(c.is_ascii_digit() || c == '-')).next().filter(|x| !x.is_empty() && *x != "-").and_then(|x| x.parse::<i64>().ok()) };
    let (y12, top) = rest.split_once("; the same pairs at the exit top").map_or((rest, ""), |(a, b)| (a, b));
    let after = |hay: &str, key: &str| hay.split_once(key).and_then(|(_, x)| num(x));
    r.y12_bench = [
        after(y12, "lane -1 -> lane 3 "),
        after(y12, "lane 3 -> lane 7 "),
        after(y12, "lane 7 -> the napper "),
        after(y12, "the napper -> the next batch's lane -1 "),
        after(y12, "lane 7 -> the next batch's lane -1 with no napper between "),
    ];
    match y12.split_once("the column order at y 12 ") {
        Some((_, o)) => r.y12_order_wrong = o.trim_start().starts_with("WRONG"),
        None => r.errors.push("path (a) y-12 pairs: no column order".to_string()),
    }
    let top = top.split_once("): ").map_or(top, |(_, x)| x);
    r.y12_top = [after(top, "lane -1 -> lane 3 "), after(top, "lane 3 -> lane 7 "), after(top, "lane 7 -> the napper "), after(top, "the napper -> the next lane -1 ")];
}

/// One switch cause's least settle and clear (the bench's `junction settle / clear by cause` line).
#[derive(Clone, Debug, PartialEq)]
pub struct SwitchGap {
    /// The switch, e.g. "K(1) set", "J(-1) re-arm".
    pub name: String,
    /// Its cause as the bench names it, e.g. "tap", "lane 7's D press".
    pub cause: String,
    /// The phase of its cause is entity (a detector press, a box overlap); false: block.
    pub entity: bool,
    pub settle: Option<i64>,
    pub clear: Option<i64>,
}

/// Split `s` at `sep` outside parentheses.
fn split_top<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let (mut depth, mut start, mut skip) = (0i32, 0usize, 0usize);
    for (i, c) in s.char_indices() {
        if i < skip {
            continue;
        }
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && s[i..].starts_with(sep) {
            out.push(&s[start..i]);
            start = i + sep.len();
            skip = start;
        }
    }
    out.push(&s[start..]);
    out
}

/// A switch's settle and clear as the farm sees them, its phase, and the driver latency applied: with
/// day-1's driver (`driver_latency`, ironspec 19:38), a switch the bench prints as an entity-phase press (a
/// press reaching the rail directly, timed from the press tick) lands L gt after the press (L = the set
/// edge's or the reset edge's latency, from the switch's name) as a block-phase change (the driver's
/// repeater); a press the bench already routes through a driver is printed block-phase and taken as is.
fn effective(s: &SwitchGap, b: &ReturnBars) -> (Option<i64>, Option<i64>, bool, Option<i64>) {
    if let Some((set_l, reset_l)) = b.driver_latency
        && s.entity
        && s.cause.contains("press")
    {
        let l = if s.name.ends_with("reset") { reset_l } else { set_l };
        return (s.settle.map(|g| g - l), s.clear.map(|g| g + l), false, Some(l));
    }
    (s.settle, s.clear, s.entity, None)
}

/// The `by cause` entries: `K(1) set (tap, block): settle N gt (…), clear M gt (…); …` (`-` when none).
fn read_by_cause(body: &str, r: &mut ReturnRead) {
    let num = |s: &str| -> Option<i64> { s.trim_start().split(|c: char| !(c.is_ascii_digit() || c == '-')).next().filter(|x| !x.is_empty() && *x != "-").and_then(|x| x.parse::<i64>().ok()) };
    for e in split_top(body.trim(), "; ").into_iter().map(str::trim).filter(|e| !e.is_empty()) {
        let bad = |r: &mut ReturnRead| r.errors.push(format!("junction settle / clear by cause: an entry unreadable: {:?}", e.chars().take(100).collect::<String>()));
        let Some(open) = e.find(" (") else {
            bad(r);
            continue;
        };
        let mut depth = 0i32;
        let mut close = None;
        for (i, c) in e[open + 1..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + 1 + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            bad(r);
            continue;
        };
        let tag = &e[open + 2..close];
        let (cause, phase) = tag.rsplit_once(", ").map_or((tag, ""), |(c, p)| (c, p.trim()));
        let entity = match phase {
            "block" => false,
            "entity" => true,
            _ => {
                bad(r);
                continue;
            }
        };
        let vals: Vec<&str> = split_top(e[close + 1..].trim_start_matches(':').trim(), ", ");
        let settle_part = vals.iter().find(|p| p.starts_with("settle "));
        let clear_part = vals.iter().find(|p| p.starts_with("clear "));
        if settle_part.is_none() && clear_part.is_none() {
            bad(r);
            continue;
        }
        r.switches.push(SwitchGap {
            name: e[..open].trim().to_string(),
            cause: cause.trim().to_string(),
            entity,
            settle: settle_part.and_then(|p| num(&p["settle ".len()..])),
            clear: clear_part.and_then(|p| num(&p["clear ".len()..])),
        });
    }
}

/// The bench's `junction settle / clear (…): J(-1): settle N gt (…), clear M gt (…)[, cart-triggered
/// clear K gt (…)]; J(3): …` line (ironspec 17:20 (b), 18:16): per junction the least settle and the
/// least clear of its timed switches, and the cart-triggered switches' clear apart; `-` when none.
fn read_settle(rest: &str, r: &mut ReturnRead) {
    // (`junction settle / clear by cause (…): …` (ironspec 19:14): one entry per switch cause)
    if let Some(after) = rest.trim_start().strip_prefix("by cause") {
        let after = after.trim_start();
        let body = if after.starts_with('(') {
            let mut depth = 0i32;
            let mut end = None;
            for (i, c) in after.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            end.map(|i| after[i + 1..].trim_start_matches(':'))
        } else {
            after.strip_prefix(':')
        };
        match body {
            Some(b) => read_by_cause(b, r),
            None => r.errors.push(format!("junction settle / clear by cause unreadable: {:?}", rest.chars().take(120).collect::<String>())),
        }
        return;
    }
    r.saw_settle = true;
    let num = |s: &str| -> Option<i64> { s.trim_start().split(|c: char| !(c.is_ascii_digit() || c == '-')).next().filter(|x| *x != "-").and_then(|x| x.parse::<i64>().ok()) };
    let mut seen = false;
    for (i, _) in rest.match_indices("J(") {
        let seg = &rest[i + 2..];
        let Some((j, after)) = seg.split_once("): ") else { continue };
        let Ok(j) = j.parse::<i32>() else { continue };
        if !after.starts_with("settle ") {
            continue;
        }
        seen = true;
        let seg = after.split("; J(").next().unwrap_or(after);
        if let Some(n) = num(&seg["settle ".len()..]) {
            r.j_settle.insert(j, n);
        }
        // (the timed clear: `, clear M gt`; the cart-triggered one: `cart-triggered clear K gt`)
        if let Some((_, c)) = seg.split_once(", clear ")
            && let Some(n) = num(c)
        {
            r.j_clear.insert(j, n);
        }
        if let Some((_, c)) = seg.split_once("cart-triggered clear ")
            && let Some(n) = num(c)
        {
            r.j_clear_cart.insert(j, n);
        }
    }
    if !seen {
        r.errors.push(format!("junction settle / clear unreadable: {:?}", rest.chars().take(120).collect::<String>()));
    }
}

/// Read a return bench's text (the `--row9` bench's lines). `k_cell`: with `--k-relay`, the napper's
/// siding K cell, whose arrival speed is read from the bench's trace of the napper (`  t T: cart K at
/// x,y,z v vx,vy,vz on ...`: the first tick in that cell); a napper run without it is missing it.
pub fn read(text: &str, napper: bool, k_cell: Option<(i32, i32, i32)>) -> ReturnRead {
    let mut r = ReturnRead::default();
    let (mut saw_split, mut saw_pairs, mut saw_gaps, mut saw_head) = (false, false, false, false);
    let mut k_first: BTreeMap<usize, f64> = BTreeMap::new();
    let mut napper_idx: Option<usize> = None;
    for l in text.lines() {
        // (`  t T: cart K at x,y,z v vx,vy,vz on ...`: each traced cart's first tick in the K cell, kept
        // by its index; the napper's chosen after the loop by the index its own line names (G1's note on
        // c620a1429))
        if let Some(kc) = k_cell
            && let Some(after) = l.strip_prefix("  t ")
            && let Some((_, rest)) = after.split_once(": cart ")
            && let Some((idx, rest)) = rest.split_once(" at ")
            && let Some((pos, vel)) = rest.split_once(" v ")
        {
            let nums = |s: &str| s.split(',').map(|x| x.trim().parse::<f64>()).collect::<Result<Vec<_>, _>>();
            let vel = vel.split(" on ").next().unwrap_or("");
            match (idx.trim().parse::<usize>(), nums(pos), nums(vel)) {
                (Ok(i), Ok(p), Ok(v)) if p.len() == 3 && v.len() == 3 => {
                    if (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32) == kc {
                        k_first.entry(i).or_insert((v[0] * v[0] + v[2] * v[2]).sqrt());
                    }
                }
                _ => r.errors.push(format!("a trace line unreadable: {:?}", l.chars().take(120).collect::<String>())),
            }
        }
        // (`cart K (batch B, lane L): … (the rider's feet at y 12.01 at N)`: N after the cart's own launch)
        if let Some(rest) = l.strip_prefix("cart ")
            && let Some((head, tail)) = rest.split_once("): ")
            && let Some((_, n)) = tail.split_once("the rider's feet at y 12.01 at ")
        {
            let k = head.split(' ').next().and_then(|x| x.parse::<usize>().ok());
            let b = head.split_once("(batch ").and_then(|(_, x)| x.split(',').next()).and_then(|x| x.trim().parse::<i64>().ok());
            let lane = head.split_once("lane ").and_then(|(_, x)| x.trim().parse::<i32>().ok());
            let n = n.split(|c: char| !c.is_ascii_digit()).next().and_then(|x| x.parse::<i64>().ok());
            if let (Some(k), Some(b), Some(lane), Some(n)) = (k, b, lane, n) {
                r.feet12.insert((k, b, lane), n);
            }
        }
        let is_napper = l.starts_with("napper ");
        if is_napper {
            napper_idx = l["napper ".len()..].split(' ').next().and_then(|x| x.parse::<usize>().ok());
        }
        if (l.starts_with("cart ") || is_napper) && l.contains(": into the shaft") {
            r.carts += 1;
            let wrong = l.contains("(WRONG LANE)");
            let never = l.contains("NEVER at a lane");
            r.wrong += usize::from(wrong);
            r.never += usize::from(never);
            r.dismounted += usize::from(l.contains("DISMOUNTED"));
            if need(l, "; hurt ", "hurt", &mut r.errors).is_some_and(|h| h > 0.0) {
                r.hurt += 1;
            }
            if need(l, "eye-in-wall ticks ", "eye-in-wall ticks", &mut r.errors).is_some_and(|e| e > 0.0) {
                r.eye += 1;
            }
            let arrived = !wrong && !never;
            let head: String = l.chars().take(40).collect();
            if is_napper {
                // (`branch speed - (handed on by the 4th route at M stored, X/gt)`; a napper in its lane by
                // the lane's own branch (`branch speed V`, not handed on into its siding) took a batch cart's
                // path: a failure, not a reading (the ungated control's creeping napper, lane 7))
                match num_after(l, "handed on by the 4th route at ") {
                    Some(m) => r.napper_entry = Some(m),
                    None if arrived && num_after(l, "; branch speed ").is_some() => r.napper_by_branch = true,
                    None if arrived => r.errors.push(format!("{head}...: on its route without its hand-on speed")),
                    None => {}
                }
            } else {
                // (`branch speed V (entering the merge cell M)`; a batch cart the bench "hands on" (a
                // siding's F or the 4th route) took no branch of its own: a failure, not a reading)
                if arrived && l.contains("(handed on by the 4th route at ") {
                    r.sided += 1;
                } else {
                    match num_after(l, "; branch speed ") {
                        Some(v) => r.branch.push(v),
                        None if arrived => r.errors.push(format!("{head}...: in its lane without its branch speed")),
                        None => {}
                    }
                    match num_after(l, "(entering the merge cell ") {
                        Some(m) => r.merge.push(m),
                        None if arrived => r.errors.push(format!("{head}...: in its lane without its merge-cell entry speed")),
                        None => {}
                    }
                }
            }
            // (its lane, as the line names it, and the return: release to its branch flat)
            if let Some(lane) = l.split(": into the shaft").next().and_then(|h| h.rsplit_once(", ").map(|(_, x)| x.trim_end_matches(')').to_string()))
                && let Some(ret) = num_after(l, "at its branch flat ")
            {
                let e = r.ret.entry(lane).or_insert(0);
                *e = (*e).max(ret as i64);
            }
        }
        if (l.starts_with("  cart ") || l.starts_with("  napper ")) && l.contains(" speed pin: ") {
            saw_head = true;
            // (`...; the rider's highest head y H (the roof's underside 28.0)`)
            if let Some(h) = need(l, "the rider's highest head y ", "the rider's highest head", &mut r.errors) {
                r.head = Some(r.head.map_or(h, |x: f64| x.max(h)));
            }
        }
        if l.starts_with("split: ") {
            saw_split = true;
            match l.find("to the next cart at it: ") {
                Some(i) => {
                    let rest = l[i + 24..].split("; ").next().unwrap_or("");
                    for p in rest.split(", ") {
                        // (`J(3) 16 gt`)
                        let parsed = p.strip_prefix("J(").and_then(|x| x.split_once(") ")).and_then(|(j, g)| Some((j.parse::<i32>().ok()?, g.trim_end_matches(" gt").trim().parse::<i64>().ok()?)));
                        match parsed {
                            Some((j, g)) => {
                                r.junction.insert(j, g);
                            }
                            None => r.errors.push(format!("a junction gap {p:?} unreadable")),
                        }
                    }
                }
                None => r.errors.push("the split line has no junction gaps".to_string()),
            }
        }
        // (`path (a): …` (83878113f) or `path (a), in game ticks after REL (…): …` (5ec2749b3); its pairs line
        // apart)
        if let Some(rest) = l.strip_prefix("path (a) y-12 pairs ") {
            read_y12_pairs(rest, &mut r);
        } else if let Some(rest) = l.strip_prefix("path (a): ").or_else(|| l.strip_prefix("path (a), ").and_then(|x| x.split_once("): ").map(|(_, y)| y))) {
            read_path_a(rest, &mut r);
        }
        if let Some(rest) = l.strip_prefix("junction settle / clear") {
            read_settle(rest, &mut r);
        }
        if let Some(rest) = l.strip_prefix("hold release: ") {
            r.hold = Some(read_hold(rest, &mut r.errors));
        }
        if let Some(rest) = l.strip_prefix("(c)'s napper lift: ") {
            // (`... on the hold when a pulse (P gt) launched it: at x,y,z (on the hold) ...` or `...: NOT
            // on the hold at any pulse ...` / `(NOT on the hold)`)
            r.lift_seen = true;
            r.napper_on_hold = if rest.contains("NOT on the hold") {
                Some(false)
            } else if rest.contains("(on the hold)") {
                Some(true)
            } else {
                r.errors.push(format!("the napper lift line unreadable: {:?}", rest.chars().take(120).collect::<String>()));
                None
            };
        }
        if l.starts_with("pairs: ") {
            saw_pairs = true;
            r.touches = need(l, "|dy| < 0.7) ", "box touches", &mut r.errors).map_or(0, |x| x as usize);
            r.pushes = need(l, "against either yaw) ", "pushes", &mut r.errors).map_or(0, |x| x as usize);
        }
        if let Some(rest) = l.strip_prefix("shaft entry gaps (live gt): ") {
            saw_gaps = true;
            let (gaps, nap) = match rest.split_once("; the napper ") {
                Some((g, n)) => (g, Some(n)),
                None => (rest.trim_end_matches(';'), None),
            };
            let mut all: Vec<i64> = Vec::new();
            for p in gaps.split(", ").filter(|p| !p.trim().is_empty()) {
                // (`batch b a / c`)
                let nums = p.splitn(3, ' ').nth(2).and_then(|g| g.split(" / ").map(|n| n.trim().parse::<i64>()).collect::<Result<Vec<_>, _>>().ok());
                match nums {
                    Some(v) => all.extend(v),
                    None => r.errors.push(format!("a shaft-entry gap {p:?} unreadable")),
                }
            }
            r.shaft = all.into_iter().min();
            if let Some(n) = nap {
                // (`X behind batch B's third cart, Y ahead of the next batch's first`; `-` unmeasured: the
                // napper or the next batch's first never into the shaft (a failure); any other value an
                // error)
                r.napper_clause = true;
                let behind = n.split(" behind").next().unwrap_or("").trim();
                let ahead = n.split(", ").nth(1).and_then(|x| x.split(" ahead").next()).unwrap_or("").trim();
                for (v, slot, what) in [(behind, &mut r.napper_behind, "behind"), (ahead, &mut r.napper_ahead, "ahead")] {
                    match v.parse::<i64>() {
                        Ok(g) => *slot = Some(g),
                        Err(_) if v == "-" => {}
                        Err(_) => r.errors.push(format!("the napper's gap {what} {v:?} unreadable")),
                    }
                }
            }
        }
    }
    // (the napper's K arrival: its own index's first tick in the K cell)
    r.napper_k = napper_idx.and_then(|i| k_first.get(&i).copied());
    if r.carts == 0 {
        r.missing.push("the carts' lines");
    }
    if !saw_head {
        r.missing.push("the speed-pin lines (the riders' heads)");
    }
    if !saw_split {
        r.missing.push("the split line");
    }
    if !saw_pairs {
        r.missing.push("the pairs line");
    }
    if !saw_gaps {
        r.missing.push("the shaft entry gaps line");
    }
    if napper && !r.napper_clause {
        r.missing.push("the napper's window");
    }
    if napper && k_cell.is_some() && r.napper_k.is_none() && r.wrong == 0 && r.never == 0 && !r.napper_by_branch {
        r.missing.push("the napper's arrival on its K cell (the trace)");
    }
    r
}

/// A run's failures against the bars (empty: it passes). A missing line or an unreadable value is a
/// failure too (the judge also refuses such a run outright).
pub fn fails(r: &ReturnRead, b: &ReturnBars, napper: bool) -> Vec<String> {
    let mut v = Vec::new();
    for m in &r.missing {
        v.push(format!("the bench's text lacks {m}"));
    }
    for e in &r.errors {
        v.push(format!("unreadable: {e}"));
    }
    for (n, what) in [(r.wrong, "wrong lane(s)"), (r.never, "never at a lane"), (r.sided, "batch cart(s) off their branch (into a siding's F or the 4th route)"), (r.dismounted, "dismounted"), (r.hurt, "hurt"), (r.eye, "eye in a wall")] {
        if n > 0 {
            v.push(format!("{n} {what}"));
        }
    }
    if r.merge.is_empty() {
        v.push("no merge-cell delivery".into());
    }
    if let Some(x) = r.merge.iter().find(|x| **x < b.branch_band.0 || **x > b.branch_band.1) {
        v.push(format!("a delivery {x:.3} outside {:.2}..{:.2}{}", b.branch_band.0, b.branch_band.1, if *x > b.branch_band.1 { " (the top inherited)" } else { "" }));
    }
    if napper {
        match b.k_band {
            // (`--k-relay`: the napper's arrival on its siding's K cell against the S-bend's band; its
            // F entry (the bench's "handed on" speed) is printed, not barred)
            Some(kb) => match r.napper_k {
                Some(x) if x < kb.0 || x > kb.1 => v.push(format!("the napper's arrival on its K cell {x:.3} outside {:.2}..{:.2} (inherited)", kb.0, kb.1)),
                None => v.push("no napper arrival on its K cell".into()),
                _ => {}
            },
            None => match r.napper_entry {
                Some(x) if x < b.napper_band.0 || x > b.napper_band.1 => v.push(format!("the napper's delivery {x:.3} outside {:.2}..{:.2} (inherited)", b.napper_band.0, b.napper_band.1)),
                None => v.push("no napper delivery".into()),
                _ => {}
            },
        }
        if r.napper_behind.is_none() || r.napper_ahead.is_none() {
            v.push("the napper's window unmeasured (it or the next batch's first never into the shaft)".into());
        }
        // (ironspec 10:02: the napper never rides with a batch; its departure only at a scheduled pulse)
        if r.napper_by_branch {
            v.push("the napper into its lane by the lane's own branch, not handed on into its siding".into());
        }
        if !b.path_a && (r.napper_ahead.is_some_and(|g| g < 0) || r.napper_behind.is_some_and(|g| g < 0)) {
            v.push(format!("the napper rode with a batch (behind {}, ahead {})", r.napper_behind.map_or("-".into(), |g| g.to_string()), r.napper_ahead.map_or("-".into(), |g| g.to_string())));
        }
        // ((c′), ironspec 11:43/11:45, from the bench's `hold release:` line: exactly one fired pulse per
        // rest, the departure on a fired pulse, 0 creeps, 0 FIFO pops without a departure, 0 fired pulses
        // without that cycle's departure; under --pulse-gate the line is required and a rolling presence
        // sample fails (G1's CHANGES 1 and note on 67d161436))
        if let Some(h) = &r.hold {
            if h.fired_in_rest.len() != 1 {
                v.push(format!("{} fired pulses while the napper rested (exactly one)", h.fired_in_rest.len()));
            }
            match h.on_fired {
                Some(true) => {}
                Some(false) => v.push("the napper's departure not on a fired pulse".into()),
                None => v.push("the napper never left the hold".into()),
            }
            for (n, what) in [(h.creeps, "creep(s) (the napper off the hold with no pulse)"), (h.pops_without, "FIFO pop(s) without a departure"), (h.pulses_without, "fired pulse(s) without that cycle's departure")] {
                if n > 0 {
                    v.push(format!("{n} {what}"));
                }
            }
            if h.second_rise == Some(Some(true)) {
                v.push("the napper's second ARM3 rise fired a pulse (a phantom: presence must be live AND delayed)".into());
            }
            if b.hold_required && !h.rolling_samples.is_empty() {
                v.push(format!("presence sampled on a rolling cart at E's rise ({} sample(s), up to {:.3})", h.rolling_samples.len(), h.rolling_samples.iter().copied().fold(0.0, f64::max)));
            }
        } else if b.hold_required {
            v.push("no hold release line (--pulse-gate: the hold's bars unjudged)".into());
        } else if b.scheduled_only && r.napper_on_hold == Some(false) {
            v.push("the napper did not depart on a fired pulse (not on the hold at any pulse's rise: it never departed, or left the hold without one)".into());
        }
        if b.path_a {
            // (path (a), ironspec 17:20 (a): "shaft entry" retired for the napper; its y-12 pass gaps behind
            // lane 7's cart and ahead of the next batch's first at y12_nap_min each, in every row)
            for (g, side) in [(r.y12_behind, "behind lane 7's cart"), (r.y12_ahead, "ahead of the next batch's first")] {
                match g {
                    Some(g) if g < b.y12_nap_min => v.push(format!("the napper's y-12 gap {side} {g} gt (bar {}, the column bar: physical 14 + 2, ironspec 20:01)", b.y12_nap_min)),
                    None => v.push(format!("the napper's y-12 gap {side} unmeasured")),
                    _ => {}
                }
            }
        } else {
            if r.napper_behind.is_some_and(|g| g < b.napper_behind) {
                v.push(format!("the napper {} gt behind the third cart (bar {}, the column bar: physical 14 + 2, ironspec 20:01)", r.napper_behind.unwrap_or(0), b.napper_behind));
            }
            if r.napper_ahead.is_some_and(|g| g < b.napper_ahead) {
                v.push(format!("the napper {} gt ahead of the next batch (bar {}, the column bar: physical 14 + 2, ironspec 20:01)", r.napper_ahead.unwrap_or(0), b.napper_ahead));
            }
        }
    }
    // (path (a): the batch carts' y-12 pairs at y12_min, the napper's at y12_nap_min, every cart through y 12
    // and the column in order: from the bench's pairs line (the numbers its clean() bars: lane 3 -> lane 7,
    // lane 7 -> the next lane -1 with no napper between; the napper's two pairs; the foot pair lane -1 ->
    // lane 3 barred by the shaft entry), else the judge's own from the y-12 passes)
    if b.path_a {
        let cb = "the column bar: physical 14 + 2, ironspec 20:01";
        if r.saw_y12_pairs {
            for (g, what, bar) in [(r.y12_bench[1], "lane 3 -> lane 7", b.y12_min), (r.y12_bench[4], "lane 7 -> the next batch's lane -1", b.y12_min), (r.y12_bench[2], "lane 7 -> the napper", b.y12_nap_min), (r.y12_bench[3], "the napper -> the next batch's lane -1", b.y12_nap_min)] {
                if let Some(g) = g
                    && g < bar
                {
                    v.push(format!("the y-12 pair {what} {g} gt (bar {bar}, {cb})"));
                }
            }
            if r.y12_order_wrong {
                v.push("the column order at y 12 wrong".into());
            }
        } else if let Some(g) = r.y12_pairs.iter().min()
            && *g < b.y12_min
        {
            v.push(format!("a batch pair's y-12 gap {g} gt (bar {}, {cb})", b.y12_min));
        }
        if r.y12_missing > 0 {
            v.push(format!("{} batch cart(s) never passed y 12", r.y12_missing));
        }
    }
    // (path (a): the column-order bar (ironspec 20:01, physical): lane 7's entry at y 12 at least mouth_min
    // gt after the preceding cart's box fully cleared the mouth cell)
    if b.path_a {
        match r.mouth {
            Some(g) if g < b.mouth_min => v.push(format!("lane 7's mouth clearance {g} gt (bar {}, physical: the column order, ironspec 20:01)", b.mouth_min)),
            None => v.push("lane 7's mouth clearance unmeasured".into()),
            _ => {}
        }
    }
    match r.shaft {
        Some(g) if g < b.shaft_min + b.margin => v.push(format!("a shaft-entry gap of {g} gt (bar {} + {}: held at {} on every foot pair, ironspec 19:38)", b.shaft_min, b.margin, b.shaft_min + b.margin)),
        None => v.push("no shaft-entry gap".into()),
        _ => {}
    }
    // (the junctions (ironspec 17:20 (b), 19:08: every switch on both sides >= its physical bar + margin in
    // every row; block-phase (the timed switches) `junction_min`, entity-phase (detector-triggered)
    // `junction_entity_min`): with the bench's settle / clear line, the settle (its cause unnamed: the
    // entity-phase bar, the larger), the timed clear and the cart-triggered clear; else the split line's
    // least gap)
    for (j, m) in &b.junction_min {
        // (with the per-cause line, the switches are barred below, not per junction)
        if !r.switches.is_empty() {
            break;
        }
        if r.saw_settle {
            // (the per-junction line: the settle's cause unnamed (the entity-phase bar), the clear at the
            // timed re-arm block-phase, the cart-triggered clear entity-phase)
            let sb = b.junction_entity_min + b.margin;
            match r.j_settle.get(j) {
                Some(g) if *g < sb => v.push(format!("J({j})'s settle {g} gt (entity-phase bar {} + {}, its cause unnamed)", b.junction_entity_min, b.margin)),
                None if r.junction.get(j).is_none() => v.push(format!("J({j})'s settle unmeasured")),
                _ => {}
            }
            if let Some(g) = r.j_clear.get(j)
                && *g < b.junction_block_min + b.margin
            {
                v.push(format!("J({j})'s clear {g} gt (block-phase bar {} + {})", b.junction_block_min, b.margin));
            }
            if let Some(g) = r.j_clear_cart.get(j)
                && *g < b.junction_entity_min + b.margin
            {
                v.push(format!("J({j})'s cart-triggered clear {g} gt (entity-phase bar {} + {})", b.junction_entity_min, b.margin));
            }
            continue;
        }
        match r.junction.get(j) {
            Some(g) if *g < m + b.margin => v.push(format!("J({j})'s gap {g} gt (bar {m} + {})", b.margin)),
            None => v.push(format!("J({j})'s gap unmeasured")),
            _ => {}
        }
    }
    // (ironspec 19:14: each switch's settle and clear >= the bar of its cause's phase + margin in every
    // row; a press timed from its press tick, the clear from the old-state cart's last tick starting in
    // the cell, as the bench measures them)
    for s in &r.switches {
        let (settle, clear, entity, lat) = effective(s, b);
        let (bar, phase) = if entity { (b.junction_entity_min, "entity") } else { (b.junction_block_min, "block") };
        let how = lat.map_or(String::new(), |l| format!(", with day-1's driver +{l}"));
        for (g, side) in [(settle, "settle"), (clear, "clear")] {
            if let Some(g) = g
                && g < bar + b.margin
            {
                v.push(format!("{} ({}{how})'s {side} {g} gt ({phase}-phase bar {bar} + {})", s.name, s.cause, b.margin));
            }
        }
    }
    if r.pushes > b.pushes_max {
        v.push(format!("{} box-test pushes", r.pushes));
    }
    match r.head {
        Some(h) if h >= b.head_max => v.push(format!("a rider's head at {h:.3} (the roof's underside {:.1})", b.head_max)),
        None => v.push("no rider's head measured".into()),
        _ => {}
    }
    v
}

/// Each gt bar's margin in a run (value - bar), the junctions with a bar.
fn gt_margins(r: &ReturnRead, b: &ReturnBars, napper: bool) -> Vec<(String, Option<i64>, i64)> {
    // (each: the bar's name, the run's margin over it, the margin a nominal corner needs (`margin`
    // for the measured gt bars; 0 for the y-12 gaps, whose bar ironspec gave with its margin (17:20) and
    // which every row already bars); the junctions from their settle and clear sides when the bench
    // prints them (ironspec 17:20 (b): each side >= the bar + 2 in every row))
    let mut v = vec![("shaft-entry gap".to_string(), r.shaft.map(|x| x - b.shaft_min), b.margin)];
    if napper && b.path_a {
        v.push(("napper's y-12 gap behind".to_string(), r.y12_behind.map(|x| x - b.y12_nap_min), 0));
        v.push(("napper's y-12 gap ahead".to_string(), r.y12_ahead.map(|x| x - b.y12_nap_min), 0));
    } else if napper {
        // (the column bar 16 holds its margin (physical 14 + 2, ironspec 20:01): 0 over it at the corners)
        v.push(("napper behind".to_string(), r.napper_behind.map(|x| x - b.napper_behind), 0));
        v.push(("napper ahead".to_string(), r.napper_ahead.map(|x| x - b.napper_ahead), 0));
    }
    if b.path_a {
        v.push(("lane 7's mouth clearance".to_string(), r.mouth.map(|x| x - b.mouth_min), 0));
    }
    if !r.switches.is_empty() {
        // (the per-cause line: each switch's settle and clear against its phase's bar)
        for s in &r.switches {
            let (settle, clear, entity, _) = effective(s, b);
            let bar = if entity { b.junction_entity_min } else { b.junction_block_min };
            if let Some(g) = settle {
                v.push((format!("{} settle", s.name), Some(g - bar), b.margin));
            }
            if let Some(g) = clear {
                v.push((format!("{} clear", s.name), Some(g - bar), b.margin));
            }
        }
        return v;
    }
    for (j, m) in &b.junction_min {
        if r.saw_settle {
            v.push((format!("J({j}) settle"), r.j_settle.get(j).or_else(|| r.junction.get(j)).map(|g| g - b.junction_entity_min), b.margin));
            // (a junction without a timed or a cart-triggered switch in the run prints `-`: no margin)
            if let Some(g) = r.j_clear.get(j) {
                v.push((format!("J({j}) clear"), Some(g - b.junction_block_min), b.margin));
            }
            if let Some(g) = r.j_clear_cart.get(j) {
                v.push((format!("J({j}) cart-triggered clear"), Some(g - b.junction_entity_min), b.margin));
            }
        } else {
            v.push((format!("J({j})"), r.junction.get(j).map(|g| g - m), b.margin));
        }
    }
    v
}

/// One run of the matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Control,
    Restart(i64, Restart),
    Login(i64, i64),
    /// The bench's ±2 pattern `code` (0..729) over batches 0 and 1's releases (its `--jitter`).
    Jitter(usize),
    /// The napper leaving R at this tick (its `--napper-r`: the late-arrival rows), with one more batch
    /// so a late napper always has a next batch to be judged against.
    Late(i64),
    /// A late row with a login: the napper from R at `.0`, a `.2`-gt login at `.0 + .1`.
    LateLogin(i64, i64, i64),
    /// The presence gate's positive control: the nominal corner with `--gate-delayed-only`.
    GateControl,
    /// The ungated-pulse control (ironspec 14:41): a late row (the napper from R at this tick, one more
    /// batch) without `--pulse-gate`.
    UngatedLate(i64),
}

/// The station calibration rows (ironspec 19:52): per table lane, batch 0's y-12 pass after REL (the
/// `path (a):` line's, else the cart line's feet time + the station lag + the cart's offset, the carts in
/// the bench's launch order k % 3), the table's value, and the source.
pub fn station_check(r: &ReturnRead, table: &[(i32, i64)], offsets: &[i64], station_lag: i64) -> Vec<(i32, Option<i64>, i64, &'static str)> {
    table
        .iter()
        .map(|(lane, t)| {
            if let Some(v) = r.y12_rel.get(&(0, *lane)) {
                return (*lane, Some(*v), *t, "path (a)");
            }
            let v = r.feet12.iter().find(|((_, b, l), _)| *b == 0 && l == lane).and_then(|((k, _, _), n)| offsets.get(k % 3).map(|off| n + station_lag + off));
            (*lane, v, *t, "the riders' feet")
        })
        .collect()
}

/// A row's short label.
pub fn row_label(row: &Row) -> String {
    match row {
        Row::Control => "the control".to_string(),
        Row::Restart(t, how) => format!("a restart at t {t} ({})", how.label()),
        Row::Login(t, d) => format!("a {d}-gt login at t {t}"),
        Row::Jitter(code) => format!("jitter [{}]", jitter_of(*code).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")),
        Row::Late(r) => format!("the napper from R at t {r}"),
        Row::LateLogin(r, off, d) => format!("the napper from R at t {r} with a {d}-gt login at +{off}"),
        Row::GateControl => "the gate's positive control (--gate-delayed-only)".to_string(),
        Row::UngatedLate(r) => format!("the ungated control: the napper from R at t {r}"),
    }
}

/// The bench's jitter vector for `code`: each of batches 0 and 1's six releases shifted -2, 0 or +2
/// (the bench's own `jitter_sweep` order).
pub fn jitter_of(code: usize) -> Vec<i64> {
    let mut j = Vec::new();
    let mut c = code;
    for _ in 0..6 {
        j.push([-2i64, 0, 2][c % 3]);
        c /= 3;
    }
    j
}

/// A failure's kind with its numbers replaced by N (to group failures across runs).
fn kind(s: &str) -> String {
    let mut out = String::new();
    let mut in_num = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() || (in_num && ch == '.') {
            if !in_num {
                out.push('N');
                in_num = true;
            }
        } else {
            in_num = false;
            out.push(ch);
        }
    }
    out
}

/// Ranges of sorted phases ("a..b, c").
fn bands(v: &[i64]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        out.push(if i == j { format!("{}", v[i]) } else { format!("{}..{}", v[i], v[j]) });
        i = j + 1;
    }
    out.join(", ")
}

/// A corner: the offsets, the release speed, the napper's delay.
#[derive(Clone, Debug, PartialEq)]
pub struct Corner {
    pub offsets: Vec<i64>,
    pub speed: f64,
    pub napper: Option<i64>,
}

impl Corner {
    fn label(&self) -> String {
        format!("offsets {:?}, release {}, napper {}", self.offsets, self.speed, self.napper.map_or("none".to_string(), |d| format!("D {d}")))
    }
}

/// The matrix.
#[derive(Clone, Debug)]
pub struct ReturnJudgeOpts {
    /// Every corner runs its control.
    pub corners: Vec<Corner>,
    /// The nominal corners: their controls' margins are held to `margin`; their restarts (every phase
    /// in `starts`, the 10 orders) and logins run.
    pub phase_corners: Vec<Corner>,
    pub starts: (i64, i64),
    pub restarts: bool,
    pub ds: Vec<i64>,
    /// Logins of at most this many gt are the lead's (ironspec 09:26: the lead's freeze is a restart,
    /// riders officially miss 1 tick, the brief's bound <= 3 gt): their failures count; longer logins
    /// are the pearl alternative's, printed as its stated limit (failures by gap length, the first
    /// failing gap).
    pub lead_login_max: i64,
    /// The bench's 729 ±2 jitter patterns at each nominal corner (lead rows; ironspec 10:02).
    pub jitter: bool,
    /// The late-arrival rows: the napper leaving R at every tick in this range at each nominal corner
    /// (lead rows: its departure only at a scheduled pulse, never with a batch; ironspec 10:02).
    pub late: Option<(i64, i64)>,
    /// Whether the bench's K/J(-1) taps follow the napper's actual departure (7b90c3f1's `--day1-taps
    /// --dep-keyed`); when not, the late family's verdict names the bench's emulation as the cause of
    /// its failures (G1's note on 1a962db4a), so they are not read as the design's.
    pub taps_follow_departure: bool,
    /// E's rise relative to ARM3 (7b90c3f1's (c′) amend: ARM3 + 2): the napper's departure relative to
    /// E's rise is the `hold release:` line's ARM3 offset less this.
    pub e_rise: i64,
    /// The positive control of the presence gate (ironspec 11:49): each nominal corner with the bench's
    /// `--gate-delayed-only` (the live term removed) must fire the phantom pulse at the napper's second
    /// ARM3 rise; a control that does not is a failure of the judge's own check.
    pub gate_control: bool,
    /// The ungated-pulse control (ironspec 14:41, replacing the phantom control under path (a)): each
    /// nominal corner's late rows run without `--pulse-gate` must fail (a pulse onto an empty hold, a pop
    /// without a departure, a launch before rest); a corner whose ungated late rows all pass means the
    /// release bars cannot see an ungated pulse: a failure.
    pub ungated_control: bool,
    /// The arrivals x freezes rows (R_FROM, R_TO, OFF_FROM, OFF_TO): the napper leaving R at every tick
    /// R_FROM..=R_TO with a login of each of the lead's lengths at every offset OFF_FROM..=OFF_TO after
    /// its start (ironspec 11:43/11:48: the departure on the fired pulse at every freeze offset and every
    /// arrival phase), with one more batch like the late rows.
    pub late_logins: Option<(i64, i64, i64, i64)>,
    /// The station calibration check (ironspec 19:52): the bench's `--offsets` (with `--station-lag`) is a
    /// calibration of dfb89499's real station, not the design offsets; at the release speed
    /// `station_speed`, each lane's y-12 pass after REL in the control row must be within 1 gt of
    /// dfb89499's table (lane, gt); a run whose calibration misses is refused, not judged.
    pub station_table: Vec<(i32, i64)>,
    pub station_speed: Option<f64>,
    pub bars: ReturnBars,
    /// Every gt bar's margin at the nominal corners.
    pub margin: i64,
    pub jobs: usize,
}

fn margins(xs: &[f64], band: (f64, f64)) -> String {
    if xs.is_empty() {
        return "-".into();
    }
    let lo = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    format!("{lo:.3}..{hi:.3} (margins {:+.3} over {:.2}, {:+.3} under {:.2})", lo - band.0, band.0, band.1 - hi, band.1)
}

/// The least of an `Option<i64>` field over runs, with its margin to `bar`.
fn least(rs: &[&ReturnRead], f: impl Fn(&ReturnRead) -> Option<i64>, bar: i64) -> String {
    rs.iter().filter_map(|r| f(r)).min().map_or("-".into(), |g| format!("{g} gt ({:+})", g - bar))
}

/// Runs `runs` on `jobs` threads, each through the bench and the reader; any missing line or
/// unreadable value is an error. With `--k-relay` the napper's K cell is read from its trace.
fn run_all<K: Clone + Send + Sync + std::fmt::Debug>(base: &ReturnOpts, runs: &[(K, ReturnOpts, bool)], jobs: usize) -> Result<Vec<(K, ReturnRead)>, String> {
    let kc = base.k_relay.then_some((base.napper_lane - 2, 25, 9));
    let mut results = Vec::new();
    for chunk in runs.chunks(jobs.max(1)) {
        let got: Vec<Result<(K, ReturnRead), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|(k, lo, nap)| {
                    sc.spawn(move || {
                        return_bench(lo).map(|t| {
                            let mut rd = read(&t, *nap, kc);
                            // (`--napper-lift`: the bench's napper lift line is read for every napper run;
                            // `--path-a`: its path (a) line, whose y-12 gaps the bars read)
                            if lo.napper_lift && *nap && !rd.lift_seen {
                                rd.missing.push("the napper lift line");
                            }
                            if lo.path_a && !rd.saw_path_a {
                                rd.missing.push("the path (a) line");
                            }
                            (k.clone(), rd)
                        })
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a return judge thread panicked")).collect()
        });
        for r in got {
            let (k, rd) = r?;
            if !rd.missing.is_empty() || !rd.errors.is_empty() {
                return Err(format!("{k:?}: the bench's text lacks {:?}; unreadable: {:?}", rd.missing, rd.errors));
            }
            results.push((k, rd));
        }
    }
    Ok(results)
}

/// The bench options for one corner and row (with `--k-relay` and a napper, the napper's trace for
/// its K cell).
fn opts(base: &ReturnOpts, c: &Corner, row: Row) -> ReturnOpts {
    let mut lo = base.clone();
    lo.offsets = c.offsets.clone();
    lo.speed = c.speed;
    lo.napper_delay = c.napper;
    lo.out = None;
    lo.restart = None;
    lo.login = None;
    match row {
        Row::Control => {}
        Row::Restart(t, how) => lo.restart = Some((t, how)),
        Row::Login(t, d) => lo.login = Some((t, d)),
        Row::Jitter(code) => lo.jitter = jitter_of(code),
        Row::Late(r) => {
            lo.napper_r = r;
            lo.batches = base.batches + 1;
            lo.ticks = base.ticks + 100;
        }
        Row::LateLogin(r, off, d) => {
            lo.napper_r = r;
            lo.login = Some((r + off, d));
            lo.batches = base.batches + 1;
            lo.ticks = base.ticks + 100;
        }
        Row::GateControl => lo.gate_delayed_only = true,
        Row::UngatedLate(r) => {
            lo.pulse_gate = None;
            lo.napper_r = r;
            lo.batches = base.batches + 1;
            lo.ticks = base.ticks + 100;
        }
    }
    // (the napper is the cart after the batches: its index set once the batches are)
    lo.trace = (lo.k_relay && c.napper.is_some()).then_some(lo.batches * 3);
    lo
}

/// The judge. Returns (lines, failing runs in the lead's rows (with nominal margins short of
/// `margin`), the bars it could not judge).
pub fn judge(base: &ReturnOpts, o: &ReturnJudgeOpts) -> Result<(Vec<String>, usize, f64), String> {
    let orders = Restart::rows();
    let mut runs: Vec<((Corner, Row), ReturnOpts, bool)> = Vec::new();
    let mut push = |c: &Corner, row: Row| runs.push(((c.clone(), row), opts(base, c, row), c.napper.is_some()));
    for c in &o.corners {
        push(c, Row::Control);
    }
    // (the station calibration check's corner: a control at the station's speed, when no corner has it)
    let station_corner: Option<Corner> = o.station_speed.filter(|_| !o.station_table.is_empty()).map(|s| {
        o.corners.iter().chain(o.phase_corners.iter()).find(|c| (c.speed - s).abs() < 1e-9).cloned().unwrap_or_else(|| Corner { offsets: base.offsets.clone(), speed: s, napper: o.corners.first().and_then(|c| c.napper) })
    });
    if let Some(sc) = &station_corner
        && !o.corners.contains(sc)
        && !o.phase_corners.contains(sc)
    {
        push(sc, Row::Control);
    }
    for c in &o.phase_corners {
        if !o.corners.contains(c) {
            push(c, Row::Control);
        }
        for t in o.starts.0..=o.starts.1 {
            if o.restarts {
                for how in &orders {
                    push(c, Row::Restart(t, *how));
                }
            }
            for &d in &o.ds {
                push(c, Row::Login(t, d));
            }
        }
        if o.jitter {
            for code in 0..729 {
                push(c, Row::Jitter(code));
            }
        }
        if let Some((a, b)) = o.late {
            for r in a..=b {
                push(c, Row::Late(r));
            }
        }
        if let Some((ra, rb, oa, ob)) = o.late_logins {
            for r in ra..=rb {
                for off in oa..=ob {
                    for &d in o.ds.iter().filter(|d| **d <= o.lead_login_max) {
                        push(c, Row::LateLogin(r, off, d));
                    }
                }
            }
        }
        if o.gate_control {
            push(c, Row::GateControl);
        }
        if let (true, Some((a, b))) = (o.ungated_control, o.late) {
            for r in a..=b {
                push(c, Row::UngatedLate(r));
            }
        }
    }
    let results = run_all(base, &runs, o.jobs)?;
    type Res = ((Corner, Row), ReturnRead);
    let fl = |c: &Corner, r: &ReturnRead| fails(r, &o.bars, c.napper.is_some());
    let mut out = Vec::new();
    let mut total = 0usize;
    // (the station calibration (ironspec 19:52): checked first; a calibration that misses dfb89499's table
    // refuses the run)
    if let Some(sc) = &station_corner {
        let Some((_, r)) = results.iter().find(|((c, row), _)| c == sc && *row == Row::Control) else {
            return Err(format!("the station calibration check: no control row at {}", sc.label()));
        };
        let rows = station_check(r, &o.station_table, &sc.offsets, base.station_lag);
        let ok = rows.iter().all(|(_, v, t, _)| v.is_some_and(|v| (v - t).abs() <= 1));
        let line = format!(
            "the station calibration (ironspec 19:52; the bench's --offsets {:?} --station-lag {} against dfb89499's table, the y-12 pass after REL in the control at release {}, within 1 gt): {}",
            sc.offsets,
            base.station_lag,
            sc.speed,
            rows.iter().map(|(lane, v, t, src)| format!("lane {lane} {} (table {t}, {src}){}", v.map_or("-".to_string(), |v| format!("+{v}")), if v.is_some_and(|v| (v - t).abs() <= 1) { "" } else { " MISS" })).collect::<Vec<_>>().join(", ")
        );
        if !ok {
            return Err(format!("{line}: the calibration does not reproduce the station; the run is refused"));
        }
        out.push(line);
    }
    // (the controls at every corner: failures, the bands' margins, the napper's window, the returns)
    let controls: Vec<&Res> = results.iter().filter(|(k, _)| k.1 == Row::Control).collect();
    let ctl_reads: Vec<&ReturnRead> = controls.iter().map(|(_, r)| r).collect();
    let ctl_fail: Vec<&&Res> = controls.iter().filter(|((c, _), r)| !fl(c, r).is_empty()).collect();
    total += ctl_fail.len();
    let all_merge: Vec<f64> = ctl_reads.iter().flat_map(|r| r.merge.clone()).collect();
    let all_branch: Vec<f64> = ctl_reads.iter().flat_map(|r| r.branch.clone()).collect();
    // (the napper's delivery: with --k-relay its arrival on its K cell against the S-bend's band, its F
    // entry printed; else the 4th route's hand-on)
    // (the bench's `hold release:` line over a set of runs ((c′), ironspec 11:43/11:45/11:49): the runs
    // carrying it, those with other than exactly one fired pulse in the rest, departures not on a fired
    // pulse, creeps, FIFO pops and fired pulses without that cycle's departure, phantoms at the napper's
    // second ARM3 rise, and the rolling presence samples (printed, not barred: the close is day-1's))
    let hold_text = |rs: &[&ReturnRead]| -> String {
        let hs: Vec<&HoldRelease> = rs.iter().filter_map(|r| r.hold.as_ref()).collect();
        if hs.is_empty() {
            return "the hold release: no line".to_string();
        }
        let rolling: Vec<f64> = hs.iter().flat_map(|h| h.rolling_samples.iter().copied()).collect();
        let deps: Vec<i64> = hs.iter().filter_map(|h| h.departure_arm3).collect();
        format!(
            "the hold release ({} runs): other than one fired pulse in the rest {}, not on a fired pulse {}, never left {}, creeps {}, FIFO pops without a departure {}, fired pulses without that cycle's departure {}, phantoms at the napper's second ARM3 rise {} (second rises read {}), the departure ARM3 + {}, rolling presence samples {}{}",
            hs.len(),
            hs.iter().filter(|h| h.fired_in_rest.len() != 1).count(),
            hs.iter().filter(|h| h.on_fired == Some(false)).count(),
            hs.iter().filter(|h| h.on_fired.is_none()).count(),
            hs.iter().map(|h| h.creeps).sum::<usize>(),
            hs.iter().map(|h| h.pops_without).sum::<usize>(),
            hs.iter().map(|h| h.pulses_without).sum::<usize>(),
            hs.iter().filter(|h| h.second_rise == Some(Some(true))).count(),
            hs.iter().filter(|h| matches!(h.second_rise, Some(Some(_)))).count(),
            match (deps.iter().min(), deps.iter().max()) {
                (Some(a), Some(b)) => format!("{a}..{b}"),
                _ => "-".to_string(),
            },
            rolling.len(),
            if rolling.is_empty() { String::new() } else { format!(" (up to {:.3})", rolling.iter().copied().fold(0.0, f64::max)) }
        ) + &{
            // (path (a), ironspec 14:41: the E rises within k of a fired pulse, the bench's count summed over
            // the runs printing it (0 everywhere: the live term comes out); the judge's own count from the
            // two tick lists beside it)
            let ew: Vec<(i64, usize)> = hs.iter().filter_map(|h| h.e_within).collect();
            if ew.is_empty() {
                String::new()
            } else {
                // (the ruled window (G1's note on 7ef404af3, ironspec 18:01): E rises in (reset, reset + k],
                // the reset the fired pulse's E rise (ARM3 + 2): the bench's printed resets, else the last E
                // rise at or before each fired pulse's rail rise)
                let resets_of = |h: &HoldRelease| -> Vec<i64> {
                    if !h.resets.is_empty() {
                        return h.resets.clone();
                    }
                    h.fired_rises.iter().filter_map(|p| h.e_rises.iter().filter(|e| **e <= *p).max().copied()).collect()
                };
                let mine: usize = hs
                    .iter()
                    .filter_map(|h| h.e_within.map(|(k, _)| (k, h)))
                    .map(|(k, h)| {
                        let rs = resets_of(h);
                        h.e_rises.iter().filter(|e| rs.iter().any(|r| **e > *r && **e <= *r + k)).count()
                    })
                    .sum();
                let printed = hs.iter().filter(|h| !h.resets.is_empty()).count();
                let ks: std::collections::BTreeSet<i64> = ew.iter().map(|(k, _)| *k).collect();
                format!(
                    ", E rises within k ({}) gt after a reset (the fired pulse's E rise) {} in {} runs (the judge's count from the tick lists: {}; the resets printed in {} runs, else the last E rise before each fired pulse)",
                    ks.iter().map(|k| k.to_string()).collect::<Vec<_>>().join("/"),
                    ew.iter().map(|(_, n)| n).sum::<usize>(),
                    ew.len(),
                    mine,
                    printed
                )
            }
        }
    };
    let napper_text = |rs: &[&ReturnRead]| -> String {
        let entry: Vec<f64> = rs.iter().filter_map(|r| r.napper_entry).collect();
        match o.bars.k_band {
            Some(kb) => {
                let k: Vec<f64> = rs.iter().filter_map(|r| r.napper_k).collect();
                format!(
                    "the napper's arrival on its K cell {} (the band inherited, ironspec's audit 19:07 section 2); its F entry (printed) {}",
                    margins(&k, kb),
                    if entry.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", entry.iter().copied().fold(f64::INFINITY, f64::min), entry.iter().copied().fold(f64::NEG_INFINITY, f64::max)) }
                )
            }
            None => format!("the napper's delivery (its hand-on) {} (the band inherited)", margins(&entry, o.bars.napper_band)),
        }
    };
    let head = ctl_reads.iter().filter_map(|r| r.head).fold(f64::NEG_INFINITY, f64::max);
    let mut ret_max: BTreeMap<String, i64> = BTreeMap::new();
    for r in &ctl_reads {
        for (l, t) in &r.ret {
            let e = ret_max.entry(l.clone()).or_insert(0);
            *e = (*e).max(*t);
        }
    }
    let mut jmin: BTreeMap<i32, i64> = BTreeMap::new();
    for r in &ctl_reads {
        for (j, g) in &r.junction {
            let e = jmin.entry(*j).or_insert(i64::MAX);
            *e = (*e).min(*g);
        }
    }
    out.push(format!(
        "the controls ({} corners; each the cold start): failing {}; delivery (merge-cell entry, stored) {} (the floor physical: ironfarm's first failure 0.22 + 0.05; the top inherited: no failure through 1.00 - 0.05); branch entry (printed) {}; {}; the highest head {:.3} (the roof's underside {:.1}, margin {:+.3}); the least shaft-entry gap {}; the napper's least window {} behind, {} ahead; the least junction gaps {}; box touches up to {}, pushes up to {}; the longest return per lane {}",
        controls.len(),
        ctl_fail.len(),
        margins(&all_merge, o.bars.branch_band),
        if all_branch.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", all_branch.iter().copied().fold(f64::INFINITY, f64::min), all_branch.iter().copied().fold(f64::NEG_INFINITY, f64::max)) },
        napper_text(&ctl_reads),
        head,
        o.bars.head_max,
        o.bars.head_max - head,
        least(&ctl_reads, |r| r.shaft, o.bars.shaft_min),
        least(&ctl_reads, |r| r.napper_behind, o.bars.napper_behind),
        least(&ctl_reads, |r| r.napper_ahead, o.bars.napper_ahead),
        jmin.iter().map(|(j, g)| format!("J({j}) {g} gt{}", o.bars.junction_min.get(j).map_or(" (no bar)".to_string(), |b| format!(" ({:+})", g - b)))).collect::<Vec<_>>().join(", "),
        ctl_reads.iter().map(|r| r.touches).max().unwrap_or(0),
        ctl_reads.iter().map(|r| r.pushes).max().unwrap_or(0),
        ret_max.iter().map(|(l, t)| format!("{l} {t} gt")).collect::<Vec<_>>().join(", ")
    ));
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for ((c, _), r) in &ctl_fail {
        for k in fl(c, r).iter().map(|k| kind(k)).collect::<std::collections::BTreeSet<_>>() {
            *by_kind.entry(k).or_default() += 1;
        }
    }
    if !by_kind.is_empty() {
        out.push(format!("  the controls' failures by kind (corners): {}", by_kind.iter().map(|(k, n)| format!("{k}: {n}")).collect::<Vec<_>>().join("; ")));
    }
    // (the nominal corners: every gt bar's margin at least `margin`)
    for c in &o.phase_corners {
        let Some((_, r)) = results.iter().find(|((cc, row), _)| cc == c && *row == Row::Control) else { continue };
        let gm = gt_margins(r, &o.bars, c.napper.is_some());
        let short: Vec<&(String, Option<i64>, i64)> = gm.iter().filter(|(_, m, need)| m.is_none_or(|m| m < *need)).collect();
        total += usize::from(!short.is_empty());
        out.push(format!(
            "nominal {}: the margins {} (required {} each, the y-12 gaps 0 over their bar){}",
            c.label(),
            gm.iter().map(|(k, m, _)| format!("{k} {}", m.map_or("-".to_string(), |x| format!("{x:+}")))).collect::<Vec<_>>().join(", "),
            o.margin,
            if short.is_empty() { String::new() } else { format!(" -> SHORT: {}", short.iter().map(|(k, _, _)| k.as_str()).collect::<Vec<_>>().join(", ")) }
        ));
    }
    // (the nominal corners' restarts and the logins of up to `lead_login_max` gt (the lead), the longer
    // logins (the pearl alternative's stated limit))
    for c in &o.phase_corners {
        let mine: Vec<&Res> = results.iter().filter(|((cc, _), _)| cc == c).collect();
        let restarts: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Restart(..))).collect();
        let logins: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Login(..))).collect();
        let failing: Vec<&&&Res> = restarts.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).collect();
        total += failing.len();
        let rr: Vec<&ReturnRead> = restarts.iter().map(|(_, r)| r).collect();
        let mg: Vec<f64> = rr.iter().flat_map(|r| r.merge.clone()).collect();
        // (the logins by gap length: runs, failing, the failing phases)
        let mut by_d: BTreeMap<i64, (usize, Vec<i64>, std::collections::BTreeSet<String>)> = BTreeMap::new();
        for ((cc, row), r) in logins.iter().map(|x| &***x) {
            if let Row::Login(t, d) = row {
                let e = by_d.entry(*d).or_default();
                e.0 += 1;
                let f = fl(cc, r);
                if !f.is_empty() {
                    e.1.push(*t);
                    e.2.extend(f.iter().map(|k| kind(k)));
                }
            }
        }
        let lead_login_fail: usize = by_d.iter().filter(|(d, _)| **d <= o.lead_login_max).map(|(_, e)| e.1.len()).sum();
        total += lead_login_fail;
        let lead_logins: usize = by_d.iter().filter(|(d, _)| **d <= o.lead_login_max).map(|(_, e)| e.0).sum();
        let lr: Vec<&ReturnRead> = logins.iter().filter(|((_, row), _)| matches!(row, Row::Login(_, d) if *d <= o.lead_login_max)).map(|(_, r)| r).collect();
        out.push(format!(
            "{}: {} restarts (every phase t {}..{} x {} orders): failing {}; delivery {}; {}; the highest head {:.3}; the least shaft-entry gap {}; the napper's least window {} behind, {} ahead; the lead's logins of 1..{} gt ({} runs, through liveness::freeze32): failing {lead_login_fail}, the least shaft-entry gap {}",
            c.label(),
            restarts.len(),
            o.starts.0,
            o.starts.1,
            orders.len(),
            failing.len(),
            margins(&mg, o.bars.branch_band),
            napper_text(&rr),
            rr.iter().filter_map(|r| r.head).fold(f64::NEG_INFINITY, f64::max),
            least(&rr, |r| r.shaft, o.bars.shaft_min),
            least(&rr, |r| r.napper_behind, o.bars.napper_behind),
            least(&rr, |r| r.napper_ahead, o.bars.napper_ahead),
            o.lead_login_max,
            lead_logins,
            least(&lr, |r| r.shaft, o.bars.shaft_min),
        ));
        for (d, (n, ts, kinds)) in by_d.iter().filter(|(d, _)| **d <= o.lead_login_max) {
            if !ts.is_empty() {
                let mut ts = ts.clone();
                ts.sort();
                out.push(format!("  the lead's logins of {d} gt failing at t {} of {n} ({})", bands(&ts), kinds.iter().cloned().collect::<Vec<_>>().join(", ")));
            }
        }
        let pearl: Vec<(&i64, &(usize, Vec<i64>, std::collections::BTreeSet<String>))> = by_d.iter().filter(|(d, _)| **d > o.lead_login_max).collect();
        if !pearl.is_empty() {
            let first = pearl.iter().find(|(_, e)| !e.1.is_empty()).map(|(d, _)| **d);
            out.push(format!(
                "  the pearl alternative's stated limit (logins over {} gt; reported, out of the lead's scope): {}; the first failing gap {}",
                o.lead_login_max,
                pearl.iter().map(|(d, e)| format!("{d} gt: failing {} of {}{}", e.1.len(), e.0, if e.2.is_empty() { String::new() } else { format!(" ({})", e.2.iter().cloned().collect::<Vec<_>>().join(", ")) })).collect::<Vec<_>>().join("; "),
                first.map_or("none".to_string(), |d| format!("{d} gt"))
            ));
        }
        // (the jitter rows: the bench's 729 ±2 patterns; the late rows: the napper leaving R at every tick
        // (lead rows, ironspec 10:02); each with its failing count, the napper's least window and the
        // departures outside a scheduled window)
        for (fam, is_fam) in [
            ("jitter (the bench's 729 ±2 patterns over batches 0 and 1)", (|r: &Row| matches!(r, Row::Jitter(_))) as fn(&Row) -> bool),
            ("late arrivals (the napper leaving R at every tick, so it reaches the hold at every phase relative to E's rise; with one more batch)", |r: &Row| matches!(r, Row::Late(_))),
            ("arrivals x freezes (the napper leaving R at every tick of a cycle, a login of each of the lead's lengths at every offset after its start; with one more batch)", |r: &Row| matches!(r, Row::LateLogin(..))),
        ] {
            let rows: Vec<&&Res> = mine.iter().filter(|((_, row), _)| is_fam(row)).collect();
            if rows.is_empty() {
                continue;
            }
            let bad: Vec<&&&Res> = rows.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).collect();
            total += bad.len();
            let rs: Vec<&ReturnRead> = rows.iter().map(|(_, r)| r).collect();
            let off_window = rs.iter().filter(|r| r.napper_on_hold == Some(false)).count();
            let rode = rs.iter().filter(|r| r.napper_ahead.is_some_and(|g| g < 0) || r.napper_behind.is_some_and(|g| g < 0)).count();
            let kinds: std::collections::BTreeSet<String> = bad.iter().flat_map(|((cc, _), r)| fl(cc, r).into_iter().map(|k| kind(&k))).collect();
            let which: Vec<String> = bad
                .iter()
                .take(12)
                .map(|((_, row), _)| match row {
                    Row::Jitter(code) => format!("[{}]", jitter_of(*code).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")),
                    Row::Late(r) => format!("R at t {r}"),
                    Row::LateLogin(r, off, d) => format!("R {r} +{off}/{d}gt"),
                    _ => String::new(),
                })
                .collect();
            out.push(format!(
                "  {fam}: {} runs, failing {}; departures off a pulse's rise {off_window}{}; the napper riding with a batch {rode}; {}; the napper's least window {} behind, {} ahead; the least shaft-entry gap {}; delivery {}; {}{}",
                rows.len(),
                bad.len(),
                if o.bars.scheduled_only { " (each a failure: (c′) departs on the fired pulse, ironspec 11:43)" } else { " (counted: judged by their slot margins and the state, (b))" },
                hold_text(&rs),
                least(&rs, |r| r.napper_behind, o.bars.napper_behind),
                least(&rs, |r| r.napper_ahead, o.bars.napper_ahead),
                least(&rs, |r| r.shaft, o.bars.shaft_min),
                margins(&rs.iter().flat_map(|r| r.merge.clone()).collect::<Vec<_>>(), o.bars.branch_band),
                napper_text(&rs),
                if bad.is_empty() {
                    String::new()
                } else {
                    format!(
                        " — failing: {} ({}){}",
                        which.join(" "),
                        kinds.into_iter().collect::<Vec<_>>().join(", "),
                        if (fam.starts_with("late") || fam.starts_with("arrivals")) && !o.taps_follow_departure {
                            " — the bench's emulated taps cannot judge a late napper; these rows wait for the fired-pulse-keyed taps (7b90c3f1's (c′) amend: --day1-taps keyed to the presence-gated fired pulse)"
                        } else {
                            ""
                        }
                    )
                }
            ));
        }
        for how in &orders {
            let mut ts: Vec<i64> = failing.iter().filter_map(|((_, row), _)| if let Row::Restart(t, h) = row { (h == how).then_some(*t) } else { None }).collect();
            ts.sort();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<String> = failing.iter().filter(|((_, row), _)| matches!(row, Row::Restart(_, h) if h == how)).flat_map(|((cc, _), r)| fl(cc, r).into_iter().map(|k| kind(&k))).collect();
                out.push(format!("  restarts ({}) failing at t {} ({})", how.label(), bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
        // (where the restarts' delivery is least and most: the rows nearest the band's ends)
        let ends = |pick_min: bool| -> String {
            let mut best: Option<(f64, Row)> = None;
            for ((_, row), r) in restarts.iter().map(|x| &***x) {
                for v in &r.merge {
                    if best.is_none_or(|(b, _)| if pick_min { *v < b } else { *v > b }) {
                        best = Some((*v, *row));
                    }
                }
            }
            match best {
                Some((v, Row::Restart(t, how))) => format!("{v:.3} at t {t} ({})", how.label()),
                _ => "-".into(),
            }
        };
        if !restarts.is_empty() {
            out.push(format!("  the restarts' least delivery {}, the most {}", ends(true), ends(false)));
        }
    }
    // (the presence gate's positive control (ironspec 11:49): each corner's run with the live term removed
    // must fire the phantom pulse at the napper's second ARM3 rise; one that does not means the check
    // cannot see a phantom: a failure)
    for ((c, _), r) in results.iter().filter(|((_, row), _)| *row == Row::GateControl) {
        let bit = r.hold.as_ref().is_some_and(|h| h.second_rise == Some(Some(true)) || h.pulses_without > 0);
        if !bit {
            total += 1;
        }
        out.push(format!(
            "the presence gate's positive control at {} (the bench's --gate-delayed-only: the live term removed): {}",
            c.label(),
            match &r.hold {
                None => "FAIL: no hold release line".to_string(),
                Some(h) => format!(
                    "the napper's second ARM3 rise {}; fired pulses without that cycle's departure {}: {}",
                    match h.second_rise {
                        Some(Some(true)) => "fired a pulse",
                        Some(Some(false)) => "fired nothing",
                        Some(None) => "absent",
                        None => "not printed",
                    },
                    h.pulses_without,
                    if bit { "the phantom, so the check bites" } else { "FAIL: no phantom, the control did not bite" }
                ),
            }
        ));
    }
    // (the ungated-pulse control (ironspec 14:41): each corner's late rows without --pulse-gate must
    // fail; a corner whose ungated rows all pass is a failure of the release bars themselves)
    for c in &o.phase_corners {
        let rows: Vec<&Res> = results.iter().filter(|((cc, row), _)| cc == c && matches!(row, Row::UngatedLate(_))).collect();
        if rows.is_empty() {
            continue;
        }
        let bad: Vec<&&Res> = rows.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).collect();
        if bad.is_empty() {
            total += 1;
        }
        let kinds: std::collections::BTreeSet<String> = bad.iter().flat_map(|((cc, _), r)| fl(cc, r).into_iter().map(|k| kind(&k))).collect();
        let rs: Vec<&ReturnRead> = rows.iter().map(|(_, r)| r).collect();
        out.push(format!(
            "the ungated-pulse control at {} (the late rows without --pulse-gate): {} of {} fail ({}); {}: {}",
            c.label(),
            bad.len(),
            rows.len(),
            kinds.into_iter().collect::<Vec<_>>().join(", "),
            hold_text(&rs),
            if bad.is_empty() { "FAIL: none failed, the control did not bite" } else { "the release bars see an ungated pulse" }
        ));
    }
    // (the hold release over every lead row ((c′)): one line for ironspec's bar)
    let lead_rows: Vec<&ReturnRead> = results.iter().filter(|((_, row), _)| !matches!(row, Row::GateControl | Row::UngatedLate(_)) && !matches!(row, Row::Login(_, d) if *d > o.lead_login_max)).map(|(_, r)| r).collect();
    if o.bars.hold_required || lead_rows.iter().any(|r| r.hold.is_some()) {
        // (the runs made beside those carrying the line, so the counts reconcile: 3 controls, one a
        // corner, plus each corner's families; G1's CHANGES 1 on 67d161436)
        out.push(format!("over the lead's rows ({} runs made, {} carrying the hold release line), {}", lead_rows.len(), lead_rows.iter().filter(|r| r.hold.is_some()).count(), hold_text(&lead_rows)));
    }
    // (the junction switches by cause over every lead row (ironspec 19:14/19:38): each switch's least
    // settle and clear as the farm sees them (day-1's driver applied to the press entries), its phase and
    // its bar + margin)
    let mut sw: BTreeMap<String, (Option<i64>, Option<i64>, bool, Option<i64>, String)> = BTreeMap::new();
    for r in &lead_rows {
        for s in &r.switches {
            let (settle, clear, entity, lat) = effective(s, &o.bars);
            let e = sw.entry(s.name.clone()).or_insert((None, None, entity, lat, s.cause.clone()));
            e.0 = match (e.0, settle) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            e.1 = match (e.1, clear) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
    }
    if !sw.is_empty() {
        let n = lead_rows.iter().filter(|r| !r.switches.is_empty()).count();
        out.push(format!(
            "the junction switches by cause over the lead's rows ({n} runs): {}",
            sw.iter()
                .map(|(name, (s, c, entity, lat, cause))| {
                    let bar = if *entity { o.bars.junction_entity_min } else { o.bars.junction_block_min };
                    let f = |g: &Option<i64>| g.map_or("-".to_string(), |g| format!("{g} ({:+})", g - bar - o.bars.margin));
                    format!("{name} ({cause}{}, {}-phase, bar {} + {}): settle {}, clear {}", lat.map_or(String::new(), |l| format!(" + day-1's driver {l}")), if *entity { "entity" } else { "block" }, bar, o.bars.margin, f(s), f(c))
                })
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    // ((c′), ironspec 11:48: the napper's departure tick relative to E's rise over every arrival phase and
    // freeze in the lead's scope (the controls, the restarts, the lead's logins, the jitter and the late
    // rows), min..max, and at both ends of that spread K set/reset, the J(-1) re-arm and the lane-7 merge
    // separation passing (the end rows are lead rows, so a failure there is already a failing run; this
    // names them))
    let spread: Vec<(i64, &Res)> = results
        .iter()
        .filter(|((_, row), _)| !matches!(row, Row::GateControl | Row::UngatedLate(_)) && !matches!(row, Row::Login(_, d) if *d > o.lead_login_max))
        .filter_map(|res| res.1.hold.as_ref().and_then(|h| h.departure_arm3).map(|x| (x - o.e_rise, res)))
        .collect();
    if let (Some(lo), Some(hi)) = (spread.iter().min_by_key(|(d, _)| *d), spread.iter().max_by_key(|(d, _)| *d)) {
        let end = |(d, ((c, row), r)): &(i64, &Res)| -> String {
            let k_ok = r.wrong == 0 && r.never == 0 && r.sided == 0 && (o.bars.k_band.is_none() || r.napper_k.is_some_and(|x| o.bars.k_band.is_some_and(|kb| x >= kb.0 && x <= kb.1)));
            let j_ok = o.bars.junction_min.get(&-1).is_none_or(|m| r.junction.get(&-1).is_some_and(|g| g >= m));
            let m_ok = r.napper_behind.is_some_and(|g| g >= o.bars.napper_behind) && r.pushes <= o.bars.pushes_max;
            let ok = |b: bool| if b { "ok" } else { "FAIL" };
            format!(
                "E{:+} at {} {} (K set/reset {}, the J(-1) re-arm {}, the lane-7 merge separation {} (the napper {} behind))",
                d,
                c.label(),
                row_label(row),
                ok(k_ok),
                ok(j_ok),
                ok(m_ok),
                r.napper_behind.map_or("-".into(), |g| format!("{g} gt"))
            )
        };
        out.push(format!(
            "the napper's departure relative to E's rise (ARM3 + {}) over every arrival phase and freeze in the lead's rows ({} runs): E{:+}..E{:+}; the earliest {}; the latest {}",
            o.e_rise,
            spread.len(),
            lo.0,
            hi.0,
            end(lo),
            end(hi)
        ));
    }
    // (the most delivered over the lead's rows: the controls and the restarts)
    let most = results
        .iter()
        .filter(|((_, row), _)| matches!(row, Row::Control | Row::Restart(..)))
        .flat_map(|(_, r)| r.merge.iter().copied())
        .fold(f64::NEG_INFINITY, f64::max);
    Ok((out, total, most))
}

/// The static gates on the emitted split file: the bench's `--out` (its own gates: parse(write()),
/// every T's verdict and sources, the detectors' Ts; it refuses to write a file that fails them),
/// then here the file read back, `redsim::rails::would_pop` over every rail, and every detector's
/// face neighbours (the block under it among them) with each rail's potential connections. Returns
/// the lines and the failures.
pub fn static_gates(base: &ReturnOpts, path: &str) -> Result<(Vec<String>, usize), String> {
    let mut lo = base.clone();
    lo.out = Some(path.to_string());
    lo.trace = None;
    lo.restart = None;
    lo.login = None;
    match return_bench(&lo) {
        Err(e) if e.contains("static gates fail") => return Ok((vec![format!("static gates: FAIL (the bench's --out): {e}")], 1)),
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    file_gates(path)
}

/// The file's own gates ([`static_gates`] after the bench writes it; also on a committed design file):
/// parse, would-pop over every rail, the bench's T verdicts classified, every detector's reach set,
/// ironspec's rule on every switched T.
pub fn file_gates(path: &str) -> Result<(Vec<String>, usize), String> {
    use redsim::block::Block;
    use redsim::pos::{BlockPos, Dir};
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let d = mctas::design::Design::parse(&text).map_err(|e| format!("{path}: parse(write()): {e}"))?;
    let mut fails = 0usize;
    let mut out = Vec::new();
    // (the file's cells pasted raw into their own world, as `mctas design would-pop` does)
    let Some(first) = d.blocks.keys().next() else { return Err(format!("{path}: no cells")) };
    let (mut lo3, mut hi3) = (*first, *first);
    for p in d.blocks.keys() {
        for i in 0..3 {
            lo3[i] = lo3[i].min(p[i]);
            hi3[i] = hi3[i].max(p[i]);
        }
    }
    let mut w = redsim::world::World::new(BlockPos::new(lo3[0] - 1, lo3[1] - 1, lo3[2] - 1), [hi3[0] - lo3[0] + 3, hi3[1] - lo3[1] + 3, hi3[2] - lo3[2] + 3]);
    let mut cells = Vec::new();
    let mut unparsed = Vec::new();
    for (p, s) in &d.blocks {
        match Block::parse(s) {
            Ok(b) => cells.push((BlockPos::new(p[0], p[1], p[2]), b)),
            Err(_) => unparsed.push(format!("({},{},{}) {s}", p[0], p[1], p[2])),
        }
    }
    // (a checked placement (ironspec's audit 11:05: `place_raw` skips a cell outside the world's box, and
    // panics on one after onechunk's 3ecc39fee): every cell counted against the box, only the cells inside
    // it placed, the world's model errors read after; any out-of-box cell or error is a failure (G1's
    // note on 67d161436: so the count prints))
    let out_of_box: Vec<String> = cells.iter().filter(|(p, _)| !w.contains(*p)).map(|(p, b)| format!("({},{},{}) {b}", p.x, p.y, p.z)).collect();
    let errors0 = w.errors.len();
    let inside: Vec<(BlockPos, Block)> = cells.iter().filter(|(p, _)| w.contains(*p)).cloned().collect();
    w.place_raw(&inside);
    let place_errors: Vec<String> = w.errors[errors0..].to_vec();
    fails += out_of_box.len() + place_errors.len();
    let cell = |p: BlockPos| format!("({},{},{})", p.x, p.y, p.z);
    let rails: Vec<BlockPos> = cells.iter().filter(|(_, b)| redsim::rails::is_rail(b)).map(|(p, _)| *p).collect();
    let pops: Vec<String> = rails.iter().filter(|p| redsim::rails::would_pop(&w, **p)).map(|p| cell(*p)).collect();
    fails += pops.len() + unparsed.len();
    let ts: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("# T ")).collect();
    let verdict_kinds = {
        let mut k: BTreeMap<&str, usize> = BTreeMap::new();
        for t in &ts {
            let v = if t.contains("switched by its declared lever") {
                "switched by its declared lever"
            } else if t.contains("placement order") {
                "placement order"
            } else if t.contains("settled (unpowered)") {
                "settled"
            } else {
                "UNCLASSIFIED"
            };
            *k.entry(v).or_default() += 1;
        }
        k
    };
    fails += verdict_kinds.get("UNCLASSIFIED").copied().unwrap_or(0);
    out.push(format!(
        "static gates on the split file ({path}): parse {} cells; out-of-box cells {}{}; would-pop {} of {} rails{}{}; {} T(s) (the bench's verdicts: {})",
        d.blocks.len(),
        out_of_box.len() + place_errors.len(),
        if out_of_box.is_empty() && place_errors.is_empty() { String::new() } else { format!(" (FAIL: {})", out_of_box.iter().chain(place_errors.iter()).take(6).cloned().collect::<Vec<_>>().join("; ")) },
        pops.len(),
        rails.len(),
        if pops.is_empty() { String::new() } else { format!(" ({})", pops.join(", ")) },
        if unparsed.is_empty() { String::new() } else { format!("; cells redsim can't parse (unjudged): {}", unparsed.join(", ")) },
        ts.len(),
        verdict_kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")
    ));
    for t in &ts {
        out.push(format!("  T {t}"));
    }
    // (every detector's reach set (G1 06:33): its state change updates the neighbours of its own cell
    // and of the cell under it, and a sloped detector's also reaches the cell above it (rails.rs sends a
    // slope's update there: tick-exact's rail-t scene, the T at a sloped detector's top); every rail in
    // the set with its potential connections; a T there must be one the design declares (a driver on a
    // face, [`drivers`]: a lever, a repeater pointing in, any source but a detector), else the
    // detector's press re-settles it: a failure)
    for p in &rails {
        let s = w.get(*p);
        if !s.to_string().contains("detector_rail") {
            continue;
        }
        let shape = redsim::rails::shape_of(&s);
        let mut centres = vec![*p, p.below()];
        if shape.is_ascending() {
            centres.push(p.above());
        }
        let mut reach: std::collections::BTreeSet<BlockPos> = std::collections::BTreeSet::new();
        for c in &centres {
            reach.insert(*c);
            for dd in Dir::ALL {
                reach.insert(c.rel(dd));
            }
        }
        reach.remove(p);
        let mut faces: Vec<String> = Vec::new();
        for dd in Dir::ALL {
            let b = w.get(p.rel(dd));
            faces.push(format!("{dd:?} {}", b.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("")));
        }
        let (mut in_reach, mut undeclared) = (Vec::new(), Vec::new());
        for q in &reach {
            let b = w.get(*q);
            if !redsim::rails::is_rail(&b) {
                continue;
            }
            let n = redsim::rails::RailState::new(*q, b).count_potential_connections(&w);
            let tee = redsim::rails::junction_at(&w, *q).is_some();
            let declared = drivers(&w, *q).iter().any(|x| !x.2);
            in_reach.push(format!("{} ({n} connections{})", cell(*q), if tee { if declared { ", a declared T" } else { ", an UNDECLARED T" } } else { "" }));
            if tee && !declared {
                undeclared.push(cell(*q));
            }
        }
        fails += undeclared.len();
        let own = redsim::rails::RailState::new(*p, s.clone()).count_potential_connections(&w);
        out.push(format!(
            "  detector {} {} ({own} connections): faces {}; its reach ({} cells: the faces of its cell, of the cell under it{}) holds the rails {}{}",
            cell(*p),
            shape.name(),
            faces.join(", "),
            reach.len(),
            if shape.is_ascending() { " and of the cell above it" } else { "" },
            if in_reach.is_empty() { "none".to_string() } else { in_reach.join(", ") },
            if undeclared.is_empty() { String::new() } else { format!(" -> FAIL: undeclared T(s) {}", undeclared.join(", ")) }
        ));
    }
    // (ironspec's T rule on every switched T: [`t_rule`])
    for p in &rails {
        let Some((line, bad)) = t_rule(&w, *p) else { continue };
        fails += usize::from(!bad.is_empty());
        out.push(format!("  {line}: {}", if bad.is_empty() { "ok".to_string() } else { format!("FAIL ({})", bad.join("; ")) }));
    }
    Ok((out, fails))
}

/// The drivers on the faces of the cell `p` (G1's note on 5af88c7da: day-1's built J/K driver is a
/// repeater into the T, not a lever over it): every face neighbour that powers `p` now or would in its
/// powered state (a lever, a repeater or comparator pointing in, a torch, a redstone block, a detector
/// rail: any source, read through redsim's own `signal` with the neighbour set powered), and every
/// component that strongly powers a conductor on a face of `p` now or would in its powered state (a
/// repeater into the stone under it). Each: (what and where, whether it powers `p` now, whether it is a
/// detector rail).
pub fn drivers(w: &redsim::world::World, p: redsim::pos::BlockPos) -> Vec<(String, bool, bool)> {
    use redsim::block::Block;
    use redsim::pos::Dir;
    let name = |b: &Block| b.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
    let cell = |q: redsim::pos::BlockPos| format!("({},{},{})", q.x, q.y, q.z);
    // (the block in its powered state, if it has one)
    let lit = |b: &Block| -> Option<Block> {
        let t = b.to_string();
        let u = t.replace("powered=false", "powered=true").replace("lit=false", "lit=true").replace("power=0,", "power=15,").replace("power=0]", "power=15]");
        if u == t {
            return None;
        }
        Block::parse(&u).ok()
    };
    // (a repeater or comparator whose output faces the cell asking: its `facing` is the side away from
    // its output (the input's), so one at `q = p.rel(d)` with facing `d` drives `p`; recognised by its
    // shape too, since a comparator set `powered=true` without its block entity may output 0 in redsim
    // (G1's note on c620a1429))
    let diode_into = |b: &Block, d: Dir| -> bool {
        let t = b.to_string();
        (t.contains("repeater") || t.contains("comparator")) && t.contains(&format!("facing={}", d.name()))
    };
    let mut out = Vec::new();
    for d in Dir::ALL {
        let q = p.rel(d);
        // (only cells inside the world's box: a trial placement outside it would be skipped (ironspec's
        // audit 11:05), and outside reads air anyway)
        if !w.contains(q) {
            continue;
        }
        let s = w.get(q);
        if redsim::behavior::is_conductor(w, &s, q) {
            for e in Dir::ALL {
                let r = q.rel(e);
                if r == p || !w.contains(r) {
                    continue;
                }
                let sr = w.get(r);
                let now = w.direct_signal(r, e) > 0;
                let could = now
                    || diode_into(&sr, e)
                    || lit(&sr).is_some_and(|on| {
                        let mut w2 = w.clone();
                        w2.place_raw(&[(r, on)]);
                        w2.direct_signal(r, e) > 0
                    });
                if could {
                    out.push((format!("{} {} via {} {}", name(&sr), cell(r), name(&s), cell(q)), now, name(&sr).contains("detector")));
                }
            }
        } else {
            let now = w.signal(q, d) > 0;
            let could = now
                || diode_into(&s, d)
                || lit(&s).is_some_and(|on| {
                    let mut w2 = w.clone();
                    w2.place_raw(&[(q, on)]);
                    w2.signal(q, d) > 0
                });
            if could {
                out.push((format!("{} {}", name(&s), cell(q)), now, name(&s).contains("detector")));
            }
        }
    }
    out
}

/// ironspec's T rule (farmopt 2026-11-12 07:29) on the rail at `p` when it is a switched T (a T with a
/// driver on a face ([`drivers`]): a lever, a repeater or comparator pointing in, any source; a
/// detector rail alone is no declared switch, the detector reach gate's): it stores the shape the game
/// settles it to for the power it gets; its two settle shapes (powered, unpowered) are the two curves
/// through its stem (the shapes the routes use: the stem with the entry side to turn into the branch,
/// with the far side to pass along the row); a stored non-settle shape fails (the game settles an
/// arms-N/W/E T north_east unpowered, north_west powered). A T powered with no declared driver the gate
/// recognises fails. Returns its description and failures; `None` if it is no switched T.
pub fn t_rule(w: &redsim::world::World, p: redsim::pos::BlockPos) -> Option<(String, Vec<String>)> {
    use redsim::pos::Dir;
    let j = redsim::rails::junction_at(w, p)?;
    let dr = drivers(w, p);
    let declared: Vec<&(String, bool, bool)> = dr.iter().filter(|x| !x.2).collect();
    let mut bad: Vec<String> = Vec::new();
    if declared.is_empty() {
        if !j.signal {
            return None;
        }
        bad.push(format!("powered with no driver the gate recognises (drivers on its faces: {})", if dr.is_empty() { "none".to_string() } else { dr.iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", ") }));
    }
    let on = j.signal;
    let want = if on { j.powered } else { j.unpowered };
    if j.stored != j.unpowered && j.stored != j.powered {
        bad.push(format!("stored {}, which no settle gives", j.stored.name()));
    } else if j.stored != want {
        bad.push(format!("stored {}, the other settle than its power ({}) gives", j.stored.name(), if on { "on" } else { "off" }));
    }
    let curve = |a: Dir, b: Dir| -> Option<String> {
        let ns = [a, b].into_iter().find(|d| matches!(d, Dir::North | Dir::South))?;
        let ew = [a, b].into_iter().find(|d| matches!(d, Dir::West | Dir::East))?;
        Some(format!("{}_{}", if ns == Dir::North { "north" } else { "south" }, if ew == Dir::West { "west" } else { "east" }))
    };
    // (the stem: the arm whose opposite is not an arm; the routes' curves: the stem with each other arm)
    let stems: Vec<Dir> = j.arms.iter().copied().filter(|a| !j.arms.contains(&a.opposite())).collect();
    let routes: Vec<String> = match stems.as_slice() {
        [s] => j.arms.iter().filter(|a| *a != s).filter_map(|a| curve(*s, *a)).collect(),
        _ => Vec::new(),
    };
    let settles = [j.unpowered.name().to_string(), j.powered.name().to_string()];
    if routes.len() != 2 || !routes.iter().all(|r| settles.contains(r)) || settles[0] == settles[1] {
        bad.push(format!("its settle shapes {} / {} are not the routes' curves through its stem ({})", settles[0], settles[1], if routes.is_empty() { "no single stem".to_string() } else { routes.join(", ") }));
    }
    // (RailBlock re-shapes a plain rail on a power change only with exactly 3 potential connections (a rail
    // in a horizontal neighbour's column at y-1..y+1, a detector rail included): a 4th disables the switch
    // (tick-exact's official boundary row, farmopt 19:58))
    let pot = redsim::rails::RailState::new(p, w.get(p)).count_potential_connections(w);
    if pot != 3 {
        let cols: Vec<String> = Dir::HORIZONTAL
            .iter()
            .flat_map(|d| [-1, 0, 1].into_iter().map(move |dy| p.rel(*d).offset(0, dy, 0)))
            .filter(|q| redsim::rails::is_rail(&w.get(*q)))
            .map(|q| format!("({},{},{}) {}", q.x, q.y, q.z, w.get(q).to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("")))
            .collect();
        bad.push(format!("{pot} potential connections (exactly 3 switch it: a 4th disables its switching): the rails in its neighbour columns {}", cols.join(", ")));
    }
    let line = format!(
        "switched T ({},{},{}): arms {}, driven by {} ({}), stored {}; settles {} unpowered, {} powered; the routes' curves through its stem {}",
        p.x,
        p.y,
        p.z,
        j.arms.iter().map(|a| a.name()).collect::<Vec<_>>().join("/"),
        if declared.is_empty() { "nothing declared".to_string() } else { declared.iter().map(|x| format!("{}{}", x.0, if x.1 { " (on)" } else { " (off)" })).collect::<Vec<_>>().join(", ") },
        if on { "powered" } else { "unpowered" },
        j.stored.name(),
        j.unpowered.name(),
        j.powered.name(),
        if routes.is_empty() { "-".to_string() } else { routes.join(", ") }
    );
    Some((line, bad))
}

/// The solve's inputs (ironspec 2026-11-12 04:10..04:12): the launch steps to try (offsets 0, s, 2s),
/// the napper's delays, the release speeds (the measured release-speed range: its ends and middle),
/// the measured departure spread (each launch from `spread.0` to `spread.1` gt off its nominal), the
/// margin every gt bar must keep beyond them.
#[derive(Clone, Debug)]
pub struct SolveOpts {
    pub steps: Vec<i64>,
    pub ds: Vec<i64>,
    pub speeds: Vec<f64>,
    pub spread: (i64, i64),
    pub margin: i64,
    pub bars: ReturnBars,
    /// Print every (step, D) row, not only each step's best D (the per-D table).
    pub all_rows: bool,
    pub jobs: usize,
}

/// One (step, D) of the solve over its corners: the least value of each gt bar, the pass/fail bars.
#[derive(Clone, Debug)]
pub struct SolveRow {
    pub step: i64,
    pub d: i64,
    pub corners: usize,
    pub shaft: Option<i64>,
    pub behind: Option<i64>,
    pub ahead: Option<i64>,
    pub junction: BTreeMap<i32, i64>,
    /// The batch carts' delivery (merge-cell entry, stored): (least, most).
    pub delivery: (f64, f64),
    /// The napper's delivery (its hand-on, stored): (least, most); infinite when unmeasured.
    pub napper: (f64, f64),
    pub head: f64,
    pub pushes: usize,
    /// Wrong lanes, never at a lane, batch carts off their branch, dismounted, hurt, eye in a wall
    /// (summed over the corners).
    pub lost: usize,
}

impl SolveRow {
    /// Each gt bar's least margin (value - bar).
    pub fn gt_margins(&self, b: &ReturnBars) -> Vec<(String, Option<i64>)> {
        let mut v = vec![
            ("shaft-entry gap".to_string(), self.shaft.map(|x| x - b.shaft_min)),
            ("napper behind".to_string(), self.behind.map(|x| x - b.napper_behind)),
            ("napper ahead".to_string(), self.ahead.map(|x| x - b.napper_ahead)),
        ];
        for (j, m) in &b.junction_min {
            v.push((format!("J({j})"), self.junction.get(j).map(|g| g - m)));
        }
        v
    }

    /// The least of the gt margins (None when a bar has no value).
    pub fn least_margin(&self, b: &ReturnBars) -> Option<i64> {
        let m = self.gt_margins(b);
        if m.iter().any(|(_, x)| x.is_none()) {
            return None;
        }
        m.iter().filter_map(|(_, x)| *x).min()
    }

    /// The pass/fail bars clean: no lost cart, no push, every delivery in its band, every head under
    /// the roof.
    pub fn clean(&self, b: &ReturnBars) -> bool {
        self.lost == 0
            && self.pushes <= b.pushes_max
            && self.delivery.0 >= b.branch_band.0
            && self.delivery.1 <= b.branch_band.1
            && self.napper.0 >= b.k_band.unwrap_or(b.napper_band).0
            && self.napper.1 <= b.k_band.unwrap_or(b.napper_band).1
            && self.head < b.head_max
    }

    pub fn meets(&self, b: &ReturnBars, margin: i64) -> bool {
        self.clean(b) && self.least_margin(b).is_some_and(|m| m >= margin)
    }

    /// The row as a markdown table row (the per-D table): s, D, each gt bar's value (margin), the
    /// deliveries, the highest head, pushes, lost carts, whether it meets the bars.
    fn md(&self, b: &ReturnBars, margin: i64) -> String {
        let gt = |v: Option<i64>, bar: i64| v.map_or("-".to_string(), |x| format!("{x} ({:+})", x - bar));
        let band = |r: (f64, f64)| if r.0.is_finite() { if (r.1 - r.0).abs() < 0.0005 { format!("{:.3}", r.0) } else { format!("{:.3}..{:.3}", r.0, r.1) } } else { "-".to_string() };
        let js: Vec<String> = b.junction_min.iter().map(|(j, m)| gt(self.junction.get(j).copied(), *m)).collect();
        format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.3} | {} | {} | {} |",
            self.step,
            self.d,
            gt(self.shaft, b.shaft_min),
            gt(self.behind, b.napper_behind),
            gt(self.ahead, b.napper_ahead),
            js.join(" | "),
            band(self.delivery),
            band(self.napper),
            self.head,
            self.pushes,
            self.lost,
            if self.meets(b, margin) { "yes" } else { "" }
        )
    }

    fn line(&self, b: &ReturnBars) -> String {
        let gm: Vec<String> = self.gt_margins(b).iter().map(|(k, m)| format!("{k} {}", m.map_or("-".to_string(), |x| format!("{x:+}")))).collect();
        let jn: Vec<String> = self.junction.iter().map(|(j, g)| format!("J({j}) {g} gt")).collect();
        let band = |r: (f64, f64), bb: (f64, f64)| if r.0.is_finite() { format!("{:.3}..{:.3} (margins {:+.3} / {:+.3})", r.0, r.1, r.0 - bb.0, bb.1 - r.1) } else { "-".to_string() };
        format!(
            "s {} D {}: margins {}; the least values: shaft {}, behind {}, ahead {}; junctions {}; delivery {}; the napper's delivery {}; the highest head {:.3}; pushes {}; lost carts {}",
            self.step,
            self.d,
            gm.join(", "),
            self.shaft.map_or("-".into(), |x| format!("{x} gt")),
            self.behind.map_or("-".into(), |x| format!("{x} gt")),
            self.ahead.map_or("-".into(), |x| format!("{x} gt")),
            jn.join(", "),
            band(self.delivery, b.branch_band),
            band(self.napper, b.k_band.unwrap_or(b.napper_band)),
            self.head,
            self.pushes,
            self.lost
        )
    }
}

/// The solve: for each launch step and napper delay, the controls at every corner (the departure
/// spread on each launch x the release speeds); per step its best delay (clean first, then the
/// largest least gt margin, then the smallest delay) with its margins and every delay that meets the
/// bars; then the smallest step that meets every bar with `margin` gt to spare.
pub fn solve(base: &ReturnOpts, o: &SolveOpts) -> Result<Vec<String>, String> {
    let mut deltas = vec![o.spread.0, 0, o.spread.1];
    deltas.sort();
    deltas.dedup();
    let mut runs: Vec<((i64, i64), ReturnOpts, bool)> = Vec::new();
    for &s in &o.steps {
        for &d in &o.ds {
            for &da in &deltas {
                for &db in &deltas {
                    for &dc in &deltas {
                        for &sp in &o.speeds {
                            let c = Corner { offsets: vec![da, s + db, 2 * s + dc], speed: sp, napper: Some(d) };
                            runs.push(((s, d), opts(base, &c, Row::Control), true));
                        }
                    }
                }
            }
        }
    }
    let results = run_all(base, &runs, o.jobs)?;
    let mut rows: Vec<SolveRow> = Vec::new();
    for &s in &o.steps {
        for &d in &o.ds {
            let mine: Vec<&ReturnRead> = results.iter().filter(|(k, _)| *k == (s, d)).map(|(_, r)| r).collect();
            let mut junction: BTreeMap<i32, i64> = BTreeMap::new();
            for r in &mine {
                for (j, g) in &r.junction {
                    let e = junction.entry(*j).or_insert(i64::MAX);
                    *e = (*e).min(*g);
                }
            }
            let range = |xs: Vec<f64>| (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            let nap: Vec<f64> = mine.iter().filter_map(|r| if o.bars.k_band.is_some() { r.napper_k } else { r.napper_entry }).collect();
            rows.push(SolveRow {
                step: s,
                d,
                corners: mine.len(),
                shaft: mine.iter().filter_map(|r| r.shaft).min(),
                behind: mine.iter().filter_map(|r| r.napper_behind).min(),
                ahead: mine.iter().filter_map(|r| r.napper_ahead).min(),
                junction,
                delivery: range(mine.iter().flat_map(|r| r.merge.clone()).collect()),
                // (a corner without the napper's delivery leaves the row unclean)
                napper: if nap.len() == mine.len() { range(nap) } else { (f64::NEG_INFINITY, f64::INFINITY) },
                head: mine.iter().filter_map(|r| r.head).fold(f64::NEG_INFINITY, f64::max),
                pushes: mine.iter().map(|r| r.pushes).max().unwrap_or(0),
                lost: mine.iter().map(|r| r.wrong + r.never + r.sided + r.dismounted + r.hurt + r.eye).sum(),
            });
        }
    }
    let mut out = vec![format!(
        "the solve (cc767b78, bench, tool unreviewed): launch steps {:?} (offsets 0, s, 2s), napper D {}..{}, release speeds {:?}, the departure spread {}..{} gt on each launch ({} corners a row); the bars: shaft-entry gap {}, napper behind {}, ahead {}, junctions {}; a row meets them when every gt bar keeps {} gt of margin beyond the corners and the pass/fail bars are clean (no lost cart, no push, deliveries in {:.2}..{:.2}, the napper's in {:.2}..{:.2}, heads under {:.1})",
        o.steps,
        o.ds.first().copied().unwrap_or(0),
        o.ds.last().copied().unwrap_or(0),
        o.speeds,
        o.spread.0,
        o.spread.1,
        deltas.len().pow(3) * o.speeds.len(),
        o.bars.shaft_min,
        o.bars.napper_behind,
        o.bars.napper_ahead,
        if o.bars.junction_min.is_empty() { "none".to_string() } else { o.bars.junction_min.iter().map(|(j, m)| format!("J({j}) {m}")).collect::<Vec<_>>().join(", ") },
        o.margin,
        o.bars.branch_band.0,
        o.bars.branch_band.1,
        o.bars.napper_band.0,
        o.bars.napper_band.1,
        o.bars.head_max
    )];
    let mut first_ok: Option<&SolveRow> = None;
    let mut table: Vec<String> = Vec::new();
    if o.all_rows {
        let jh: Vec<String> = o.bars.junction_min.iter().map(|(j, m)| format!("J({j}) gap, bar {m}")).collect();
        table.push(format!(
            "| s | D | shaft-entry gap, bar {} | napper behind, bar {} | napper ahead, bar {} | {} | delivery (merge-cell entry) | {} | highest head | pushes | lost carts | meets (every gt margin >= {}) |",
            o.bars.shaft_min,
            o.bars.napper_behind,
            o.bars.napper_ahead,
            jh.join(" | "),
            match o.bars.k_band {
                Some(kb) => format!("napper's arrival on its K cell, band {:.2}..{:.2}", kb.0, kb.1),
                None => format!("napper delivery (hand-on), band {:.2}..{:.2}", o.bars.napper_band.0, o.bars.napper_band.1),
            },
            o.margin
        ));
        table.push(format!("|{}", "---|".repeat(11 + o.bars.junction_min.len())));
    }
    for &s in &o.steps {
        let mine: Vec<&SolveRow> = rows.iter().filter(|r| r.step == s).collect();
        let ok: Vec<i64> = mine.iter().filter(|r| r.meets(&o.bars, o.margin)).map(|r| r.d).collect();
        if o.all_rows {
            for r in &mine {
                table.push(r.md(&o.bars, o.margin));
            }
        }
        let best = mine.iter().max_by(|a, b| {
            let ka = (a.clean(&o.bars), a.least_margin(&o.bars).unwrap_or(i64::MIN), -a.d);
            let kb = (b.clean(&o.bars), b.least_margin(&o.bars).unwrap_or(i64::MIN), -b.d);
            ka.cmp(&kb)
        });
        if let Some(b) = best {
            out.push(format!("{}{}", b.line(&o.bars), if ok.is_empty() { " -> NO delay meets the bars at this step".to_string() } else { format!(" -> meets the bars at D {}", ok.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")) }));
        }
        if first_ok.is_none() && !ok.is_empty() {
            first_ok = mine.iter().filter(|r| r.meets(&o.bars, o.margin)).max_by_key(|r| (r.least_margin(&o.bars).unwrap_or(i64::MIN), -r.d)).copied();
        }
    }
    out.push(match first_ok {
        Some(r) => format!("the smallest step that meets every bar with {} gt to spare: s {} with the napper at D {} (least gt margin {:+})", o.margin, r.step, r.d, r.least_margin(&o.bars).unwrap_or(0)),
        None => format!("NO step in {:?} meets every bar with {} gt to spare", o.steps, o.margin),
    });
    if !table.is_empty() {
        out.push("the per-D table (each value the least over the corners, its margin to the bar in brackets; deliveries the range over the corners):".to_string());
        out.extend(table);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader without a K cell (the pre-K-relay bench).
    fn read(t: &str, napper: bool) -> ReturnRead {
        super::read(t, napper, None)
    }

    fn bars() -> ReturnBars {
        ReturnBars { branch_band: (0.30, 0.48), napper_band: (0.1, 0.3), shaft_min: 14, napper_behind: 21, napper_ahead: 16, junction_min: [(3, 8), (-1, 8)].into_iter().collect(), pushes_max: 0, head_max: 28.0, k_band: None, scheduled_only: true, hold_required: false, margin: 2, junction_block_min: 1, junction_entity_min: 2, driver_latency: None, path_a: false, y12_nap_min: 16, y12_min: 16, mouth_min: 2 }
    }

    const T: &str = "cart 0 (batch 0, lane -1): into the shaft 30 live gt after release (the rider's feet at y 12.01 at 44); lane -1, at its branch flat 132 live gt after release; branch speed 0.410 (entering the merge cell 0.350); air 300; seated; hurt 0.0; eye-in-wall ticks 0\n  cart 0 speed pin: merge row stored >= 0.791, moved >= 0.200/gt; split row stored >= 0.230, moved >= 0.172/gt; the rider's highest head y 26.900 (the roof's underside 28.0)\ncart 1 (batch 0, lane 3): into the shaft 31 live gt after release; lane 3, at its branch flat 153 live gt after release; branch speed 0.420 (entering the merge cell 0.360); air 300; seated; hurt 0.0; eye-in-wall ticks 0\n  cart 1 speed pin: merge row -; split row -; the rider's highest head y 27.012 (the roof's underside 28.0)\nnapper 3 (batch 0, the 4th route): into the shaft 80 live gt after release; the 4th route; branch speed - (handed on by the 4th route at 0.261 stored, 0.196/gt); air 300; seated; hurt 0.0; eye-in-wall ticks 0\n  napper 3 speed pin: merge row -; split row -; the rider's highest head y 26.500 (the roof's underside 28.0)\nsplit: 0 wrong lane(s) of 3; the batch carts' stored speed entering their branch 0.410..0.420, entering their lane's merge cell 0.350..0.360 (ironfarm's delivery band 0.30..0.48, margins 0.050 / 0.120); the least gap from a junction's turn to the next cart at it: J(3) 17 gt, J(-1) 18 gt; the shaft's closest pair 3.100\npairs: box touches (the game's push test: inflated 0.2 sideways, |dy| < 0.7) 6 pair-ticks, the hardest closing 0.081 (t 120, carts 0 and 1 at 1.00,2.00,3.00); where a push acts (|cos| >= 0.8 against either yaw) 0\nshaft entry gaps (live gt): batch 0 23 / 23, batch 1 23 / 24; the napper 30 behind batch 0's third cart, 24 ahead of batch 1's first\n";

    /// The reader on the `--row9` bench's own lines: a clean run passes; a wrong lane, a gap under
    /// the bar, a delivery out of its band, a short napper window, a push and a head at the roof each
    /// fail; a missing line or an unreadable value is an error, never a pass.
    #[test]
    fn reads_the_bench() {
        let r = read(T, true);
        assert!(r.missing.is_empty() && r.errors.is_empty(), "{:?} {:?}", r.missing, r.errors);
        assert_eq!((r.carts, r.wrong, r.touches, r.pushes, r.shaft), (3, 0, 6, 0, Some(23)));
        assert_eq!(r.junction, [(3, 17), (-1, 18)].into_iter().collect());
        assert_eq!((r.napper_behind, r.napper_ahead, r.napper_entry), (Some(30), Some(24), Some(0.261)));
        assert_eq!((r.branch.clone(), r.merge.clone()), (vec![0.41, 0.42], vec![0.35, 0.36]));
        assert_eq!(r.head, Some(27.012));
        assert_eq!(r.ret.get("lane 3"), Some(&153));
        assert!(fails(&r, &bars(), true).is_empty(), "{:?}", fails(&r, &bars(), true));
        assert_eq!(gt_margins(&r, &bars(), true).iter().map(|(_, m, _)| m.unwrap()).collect::<Vec<_>>(), vec![9, 9, 8, 10, 9]);
        let bad = read(
            &T.replace("; lane 3,", "; lane 7 (WRONG LANE),")
                .replace("batch 1 23 / 24", "batch 1 23 / 13")
                .replace("against either yaw) 0", "against either yaw) 2")
                .replace("merge cell 0.350", "merge cell 0.290")
                .replace("the napper 30 behind", "the napper 20 behind")
                .replace("head y 27.012", "head y 28.000"),
            true,
        );
        let f = fails(&bad, &bars(), true);
        for want in ["wrong lane", "shaft-entry gap of 13", "pushes", "delivery 0.290", "20 gt behind", "head at 28.000"] {
            assert!(f.iter().any(|x| x.contains(want)), "{want:?} in {f:?}");
        }
        assert!(read(&T.replace("split: ", "splat: "), true).missing.contains(&"the split line"));
        assert!(!fails(&read(&T.replace("split: ", "splat: "), true), &bars(), true).is_empty(), "a missing line fails");
        assert!(read(&T.replace("; the napper 30 behind batch 0's third cart, 24 ahead of batch 1's first", ""), true).missing.contains(&"the napper's window"));
        // (the napper's clause with `-` (it never into the shaft): no missing line, a failure)
        let dash = read(&T.replace("the napper 30 behind batch 0's third cart, 24 ahead of batch 1's first", "the napper - behind batch -'s third cart, - ahead of the next batch's first"), true);
        assert!(dash.missing.is_empty() && dash.errors.is_empty(), "{:?} {:?}", dash.missing, dash.errors);
        assert!(fails(&dash, &bars(), true).iter().any(|f| f.contains("window unmeasured")), "{:?}", fails(&dash, &bars(), true));
        assert!(!read(&T.replace("the napper 30 behind", "the napper x behind"), true).errors.is_empty());
        // (unreadable values: errors, and each run fails (the strict reader's positive control))
        for (from, to) in [("(entering the merge cell 0.360)", ""), ("handed on by the 4th route at 0.261", "handed on by the 4th route at x"), ("J(3) 17 gt", "J(3) - gt"), ("|dy| < 0.7) 6", "|dy| < 0.7) six"), ("head y 26.500", "head y -"), ("batch 1 23 / 24", "batch 1 23 / x")] {
            let e = read(&T.replace(from, to), true);
            assert!(!e.errors.is_empty(), "{from:?} -> {to:?} reads as an error");
            assert!(fails(&e, &bars(), true).iter().any(|f| f.starts_with("unreadable: ")), "{from:?} -> {to:?} fails the run");
        }
        // (a cart off its lane needs no delivery: the lane failure stands)
        let off = read(&T.replace("; lane 3, at its branch flat 153 live gt after release; branch speed 0.420 (entering the merge cell 0.360)", "; NEVER at a lane (ends at 1.00,2.00,3.00); branch speed -"), true);
        assert!(off.errors.is_empty(), "{:?}", off.errors);
        assert!(fails(&off, &bars(), true).iter().any(|x| x.contains("never at a lane")));
        // (a batch cart the bench "hands on" (into a siding's F or the 4th route) took no branch of its
        // own: a failure, not an unreadable line)
        let sided = read(&T.replace("branch speed 0.420 (entering the merge cell 0.360)", "branch speed - (handed on by the 4th route at 0.303 stored, 0.227/gt)"), true);
        assert!(sided.errors.is_empty() && sided.sided == 1, "{:?} {}", sided.errors, sided.sided);
        assert!(fails(&sided, &bars(), true).iter().any(|x| x.contains("off their branch")), "{:?}", fails(&sided, &bars(), true));
    }

    /// The K re-lay (ironspec 08:46, the band widened 09:00): the napper's arrival on its siding's K
    /// cell read from the bench's trace of the napper (the first tick in that cell) and barred against
    /// --k-band (0.10..0.44) in place of the 4th route's hand-on; a napper run without the K cell in its
    /// trace is missing it; an unreadable trace line is an error.
    #[test]
    fn reads_the_k_cell() {
        let kb = ReturnBars { k_band: Some((0.10, 0.44)), ..bars() };
        let trace = "  t 400: cart 3 at 0.500,25.062,9.500 v 0.300,0.000,0.000 on rail[shape=east_west,waterlogged=false]\n  t 401: cart 3 at 1.010,25.062,9.500 v 0.296,-0.000,0.000 on rail[shape=north_east,waterlogged=false]\n  t 402: cart 3 at 1.300,25.062,9.200 v 0.000,0.000,-0.290 on rail[shape=north_east,waterlogged=false]\n";
        let t = format!("{T}{trace}");
        let r = super::read(&t, true, Some((1, 25, 9)));
        assert!(r.missing.is_empty() && r.errors.is_empty(), "{:?} {:?}", r.missing, r.errors);
        assert_eq!(r.napper_k, Some(0.296));
        assert!(fails(&r, &kb, true).is_empty(), "{:?}", fails(&r, &kb, true));
        // (the 4th route's hand-on is no bar under the K re-lay; 0.15 on K: in the band; 0.08 and 0.46:
        // failures)
        let at = |v: &str| super::read(&t.replace("v 0.296,-0.000,0.000", &format!("v {v},-0.000,0.000")), true, Some((1, 25, 9)));
        assert!(fails(&at("0.150"), &kb, true).is_empty(), "{:?}", fails(&at("0.150"), &kb, true));
        assert!(fails(&at("0.080"), &kb, true).iter().any(|f| f.contains("arrival on its K cell 0.080")), "{:?}", fails(&at("0.080"), &kb, true));
        assert!(fails(&at("0.460"), &kb, true).iter().any(|f| f.contains("arrival on its K cell 0.460")), "{:?}", fails(&at("0.460"), &kb, true));
        // (no K cell in the trace: missing; an unreadable trace line: an error)
        assert!(super::read(&t, true, Some((5, 25, 9))).missing.iter().any(|m| m.contains("K cell")));
        assert!(!super::read(&t.replace("v 0.296,-0.000,0.000", "v x,0,0"), true, Some((1, 25, 9))).errors.is_empty());
        // (another cart's tick in the K cell is not the napper's: keyed on the napper's index, its own
        // line's "napper 3")
        let other = format!("{T}  t 300: cart 1 at 1.200,25.062,9.500 v 0.050,0.000,0.000 on rail[shape=north_east,waterlogged=false]\n{trace}");
        assert_eq!(super::read(&other, true, Some((1, 25, 9))).napper_k, Some(0.296));
    }

    /// The napper lift line (ironspec 10:02): "on the hold when a pulse launched it ... (on the hold)" is
    /// a scheduled departure; "NOT on the hold at any pulse" a departure outside a scheduled window (a
    /// failure); an unreadable one an error. The napper's shaft entry inside a batch (a negative gap) is
    /// "the napper rode with a batch".
    #[test]
    fn napper_departure_and_batch() {
        let lift = |s: &str| format!("{T}(c)'s napper lift: the napper from R at t 0, on the hold when a pulse (12 gt) launched it: {s} (4 pulse(s) in the run); the hold rail's first rise ARM3 + 12\n");
        let ok = read(&lift("at 9.500,0.062,8.510 (on the hold)"), true);
        assert!(ok.lift_seen && ok.napper_on_hold == Some(true) && ok.errors.is_empty());
        assert!(fails(&ok, &bars(), true).is_empty(), "{:?}", fails(&ok, &bars(), true));
        let late = read(&lift("NOT on the hold at any pulse"), true);
        assert_eq!(late.napper_on_hold, Some(false));
        assert!(fails(&late, &bars(), true).iter().any(|f| f.contains("did not depart on a fired pulse")), "{:?}", fails(&late, &bars(), true));
        // ((b), ironspec 10:09: the timers keyed to the actual departure: an off-rise departure is no
        // failure in itself)
        let b_design = ReturnBars { scheduled_only: false, ..bars() };
        assert!(fails(&late, &b_design, true).is_empty(), "{:?}", fails(&late, &b_design, true));
        assert!(!read(&lift("somewhere"), true).errors.is_empty());
        assert!(!read(T, true).lift_seen);
        let rode = read(&T.replace("the napper 30 behind batch 0's third cart, 24 ahead of batch 1's first", "the napper 100 behind batch 0's third cart, -45 ahead of the next batch's first"), true);
        // (a napper in its lane by the lane's own branch, not handed on into its siding: a failure, read,
        // not an error (the ungated control's creeping napper on lane 7))
        let by_branch = read(&T.replace("branch speed - (handed on by the 4th route at 0.261 stored, 0.196/gt)", "branch speed 0.293 (entering the merge cell 0.359)"), true);
        assert!(by_branch.errors.is_empty() && by_branch.napper_by_branch, "{:?}", by_branch.errors);
        assert!(fails(&by_branch, &bars(), true).iter().any(|f| f.contains("by the lane's own branch")), "{:?}", fails(&by_branch, &bars(), true));
        assert!(fails(&rode, &bars(), true).iter().any(|f| f.contains("rode with a batch (behind 100, ahead -45)")), "{:?}", fails(&rode, &bars(), true));
    }

    /// The bench's `hold release:` line ((c′), 7b90c3f1's amend; ironspec 11:43/11:45): one fired pulse
    /// in the rest, the departure on it, 0 creeps / pops / pulses without a departure passes; two fired
    /// pulses in the rest, a departure not on a fired pulse, a napper that never left, a creep, a pop and
    /// a pulse without that cycle's departure each fail; a rolling presence sample is read (printed, not
    /// barred); an unreadable count is an error.
    #[test]
    fn reads_the_hold_release() {
        let good = "pulses fired 2 (while the napper rested: #1), skipped 1; the napper's departure on fired pulse #1 at t 215 (ARM3 + 13); creeps 0 (the napper off the hold with no pulse); FIFO pops without a departure 0; fired pulses without that cycle's departure 0; presence samples at E's rise: t 200: at rest on the hold (0.000); t 300: not on the hold (0.000)";
        let with = |s: &str| format!("{T}hold release: {s}\n");
        let r = read(&with(good), true);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let h = r.hold.clone().expect("the hold line");
        assert_eq!((h.fired, h.fired_in_rest.clone(), h.skipped, h.on_fired, h.departure_arm3, h.creeps, h.pops_without, h.pulses_without), (2, vec![1], 1, Some(true), Some(13), 0, 0, 0));
        assert!(fails(&r, &bars(), true).is_empty(), "{:?}", fails(&r, &bars(), true));
        for (from, to, want) in [
            ("(while the napper rested: #1)", "(while the napper rested: #1, #2)", "2 fired pulses while the napper rested"),
            ("(while the napper rested: #1)", "(while the napper rested: none)", "0 fired pulses while the napper rested"),
            ("on fired pulse #1 at t 215 (ARM3 + 13)", "NOT on a fired pulse", "not on a fired pulse"),
            ("on fired pulse #1 at t 215 (ARM3 + 13)", "none (it never left)", "never left the hold"),
            ("creeps 0", "creeps 1", "1 creep"),
            ("FIFO pops without a departure 0", "FIFO pops without a departure 2", "2 FIFO pop"),
            ("fired pulses without that cycle's departure 0", "fired pulses without that cycle's departure 1 (rising at t 300)", "1 fired pulse(s) without that cycle's departure"),
        ] {
            let b = read(&with(&good.replace(from, to)), true);
            assert!(b.errors.is_empty(), "{to:?}: {:?}", b.errors);
            assert!(fails(&b, &bars(), true).iter().any(|f| f.contains(want)), "{to:?} -> {want:?} in {:?}", fails(&b, &bars(), true));
        }
        let rolling = read(&with(&good.replace("t 200: at rest on the hold (0.000)", "t 200: rolling on the hold (0.031)")), true);
        assert_eq!(rolling.hold.map(|h| h.rolling_samples), Some(vec![0.031]));
        assert!(!read(&with(&good.replace("creeps 0", "creeps x")), true).errors.is_empty());
        // (ironspec 11:49: the napper's second ARM3 rise fires nothing; fired is a phantom, a failure)
        let second = |v: &str| read(&with(&format!("{good}; the napper's second ARM3 rise at t 246: {v}")), true);
        assert_eq!(second("nothing").hold.and_then(|h| h.second_rise), Some(Some(false)));
        assert!(fails(&second("nothing"), &bars(), true).is_empty());
        assert!(fails(&second("fired #3"), &bars(), true).iter().any(|f| f.contains("second ARM3 rise fired a pulse")), "{:?}", fails(&second("fired #3"), &bars(), true));
        assert!(!second("maybe").errors.is_empty());
        // (under --pulse-gate (G1's CHANGES 1 on 67d161436): a napper run without the hold line fails, and a
        // rolling presence sample fails; without the gate, neither)
        let gated = ReturnBars { hold_required: true, ..bars() };
        assert!(fails(&read(T, true), &gated, true).iter().any(|f| f.contains("no hold release line")), "{:?}", fails(&read(T, true), &gated, true));
        assert!(fails(&read(T, true), &bars(), true).is_empty());
        assert!(fails(&r, &gated, true).is_empty(), "{:?}", fails(&r, &gated, true));
        let rolling = read(&with(&good.replace("t 200: at rest on the hold (0.000)", "t 200: rolling on the hold (0.031)")), true);
        assert!(fails(&rolling, &gated, true).iter().any(|f| f.contains("presence sampled on a rolling cart at E's rise (1 sample(s), up to 0.031)")), "{:?}", fails(&rolling, &gated, true));
        assert!(fails(&rolling, &bars(), true).is_empty());
        // (path (a), ironspec 14:41: the E rise and fired pulse tick lists and the bench's count read)
        let pa = read(&with(&format!("{good}; E rises: t 146, t 246, t 290; fired pulse rises: t 256; E rises within 42 gt after a fired pulse: 1 (t 290)")), true);
        assert!(pa.errors.is_empty(), "{:?}", pa.errors);
        let h = pa.hold.expect("the hold line");
        assert_eq!((h.e_rises.clone(), h.fired_rises.clone(), h.e_within), (vec![146, 246, 290], vec![256], Some((42, 1))));
        let none = read(&with(&format!("{good}; E rises: none; fired pulse rises: none; E rises within 42 gt after a fired pulse: 0")), true);
        assert!(none.errors.is_empty() && none.hold.as_ref().is_some_and(|h| h.e_rises.is_empty() && h.e_within == Some((42, 0))), "{:?}", none.errors);
        assert!(!read(&with(&format!("{good}; E rises: t x")), true).errors.is_empty());
        // (the resets (ironspec 18:01): the fired pulses' E rises read; the bench's count "after a reset")
        let rs = read(&with(&format!("{good}; E rises: t 146, t 246; fired E rises (resets): t 246; fired pulse rises: t 256; E rises within 42 gt after a reset: 0")), true);
        assert!(rs.errors.is_empty(), "{:?}", rs.errors);
        assert_eq!(rs.hold.map(|h| (h.resets, h.e_within)), Some((vec![246], Some((42, 0)))));
    }

    /// Path (a)'s line and the junction settle / clear line (ironspec 17:20, 18:16): the napper's y-12 gaps
    /// and the batch pairs read and barred (the shaft-entry napper bars retired under --path-a); the
    /// settle and the timed clear barred at the bar + 2, the cart-triggered clear read, not barred.
    #[test]
    fn reads_path_a_and_settle() {
        let pa = "path (a): ARM3 - REL per batch [51, 51, 51]; y-12 passes after REL: b0 lane -1 +55, b0 lane 3 +74, b0 lane 7 +101, b1 lane -1 +55, b1 lane 3 +74, b1 lane 7 +101, napper +127; the napper's y-12 gap behind 26 and ahead 28; lane 7's mouth clearance 4 gt (batch 1); the least distance between two carts in the column 9.526 (carts 0 and 1 at t 182)\n";
        let sc = "junction settle / clear (least per junction): J(-1): settle 11 gt (its set at t 245, then cart 3 (batch 1, lane -1) entering at stored 0.315), clear 12 gt (cart 2 (batch 0, lane 7) out of the cell, then its reset at t 233), cart-triggered clear 5 gt (cart 1 (batch 0, lane 3) out of the cell, then its set at t 160); J(3): settle 25 gt (its set at t 150, then cart 1 (batch 0, lane 3) entering at stored 0.315), clear -\n";
        let r = read(&format!("{T}{pa}{sc}"), true);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.saw_path_a && r.saw_settle);
        assert_eq!((r.y12_behind, r.y12_ahead, r.col_min), (Some(26), Some(28), Some(9.526)));
        // (lane 3 -> 7: 27 a batch; lane 7 -> the next lane -1: 100 + 55 - 101 = 54)
        assert_eq!(r.y12_pairs, vec![27, 54, 27]);
        assert_eq!((r.j_settle.get(&-1), r.j_clear.get(&-1), r.j_clear_cart.get(&-1), r.j_settle.get(&3), r.j_clear.get(&3)), (Some(&11), Some(&12), Some(&5), Some(&25), None));
        let pb = ReturnBars { path_a: true, ..bars() };
        // (the shaft-entry napper bars retired: T's napper is 30 behind / 24 ahead, but a short shaft-entry
        // napper gap would not count under path (a))
        assert!(fails(&r, &pb, true).is_empty(), "{:?}", fails(&r, &pb, true));
        let short = read(&format!("{}{}{sc}", T.replace("the napper 30 behind", "the napper 8 behind"), pa.replace("behind 26 and ahead 28", "behind 15 and ahead 28")), true);
        let f = fails(&short, &pb, true);
        assert!(f.iter().any(|x| x.contains("y-12 gap behind lane 7's cart 15 gt (bar 16, the column bar")) && !f.iter().any(|x| x.contains("behind the third cart")), "{f:?}");
        let pair = read(&format!("{T}{}{sc}", pa.replace("b1 lane 7 +101", "b1 lane 7 +89")), true);
        assert!(fails(&pair, &pb, true).iter().any(|x| x.contains("a batch pair's y-12 gap 15 gt (bar 16, the column bar")), "{:?}", fails(&pair, &pb, true));
        // (the column-order bar (ironspec 20:01): lane 7's mouth clearance >= 2; missing under path (a):
        // unmeasured)
        assert_eq!(r.mouth, Some(4));
        let m1 = read(&format!("{T}{}{sc}", pa.replace("mouth clearance 4 gt", "mouth clearance 1 gt")), true);
        assert!(fails(&m1, &pb, true).iter().any(|x| x.contains("lane 7's mouth clearance 1 gt (bar 2, physical")), "{:?}", fails(&m1, &pb, true));
        let m0 = read(&format!("{T}{}{sc}", pa.replace("; lane 7's mouth clearance 4 gt (batch 1)", "")), true);
        assert!(fails(&m0, &pb, true).iter().any(|x| x.contains("lane 7's mouth clearance unmeasured")), "{:?}", fails(&m0, &pb, true));
        // (7b90c3f1's 5ec2749b3 wording: the path (a) line's new prefix, `-` values (unmeasured, no error), a
        // cart without a y-12 pass, and the pairs line read and barred in place of the judge's own pairs)
        let pa2 = "path (a), in game ticks after REL (the tick a state first shows at a tick's end): lane 7's slope rail rises [95, 95, 95]; ARM3 rises [51, 51, 51]; the napper's hold rail rises REL + 63 (batch 0's REL); y-12 passes after REL: b0 lane -1 +55, b0 lane 3 +74, b0 lane 7 +101, b1 lane -1 +55, b1 lane 3 +74, b1 lane 7 +101, napper +127; the napper's y-12 gap behind 26 and ahead 28; lane 7's mouth clearance 5 gt (batch 0: lane 3's box out of the mouth cell, feet at y 13, at t 210; lane 7 into the column 5 gt later); the least distance between two carts in the column 9.526 (carts 0 and 1 at t 182)\n";
        let pairs = "path (a) y-12 pairs (bar >= 16, G1): lane -1 -> lane 3 19 (the foot pair: the shaft entry's gap bars it), lane 3 -> lane 7 27, lane 7 -> the napper 26, the napper -> the next batch's lane -1 28, lane 7 -> the next batch's lane -1 with no napper between 54; the column order at y 12 -1, 3, 7 per batch, the napper after its batch's lane 7; the same pairs at the exit top (the contact point, the exit booster's cell): lane -1 -> lane 3 17, lane 3 -> lane 7 25, lane 7 -> the napper 24, the napper -> the next lane -1 30\n";
        let n = read(&format!("{T}{pa2}{pairs}{sc}"), true);
        assert!(n.errors.is_empty(), "{:?}", n.errors);
        assert!(n.saw_path_a && n.saw_y12_pairs && !n.y12_order_wrong);
        assert_eq!((n.mouth, n.y12_behind, n.y12_ahead), (Some(5), Some(26), Some(28)));
        assert_eq!((n.y12_bench, n.y12_top), ([Some(19), Some(27), Some(26), Some(28), Some(54)], [Some(17), Some(25), Some(24), Some(30)]));
        assert!(fails(&n, &pb, true).is_empty(), "{:?}", fails(&n, &pb, true));
        let un = read(&format!("{T}{}{pairs}{sc}", pa2.replace("5 gt (batch 0: lane 3's box out of the mouth cell, feet at y 13, at t 210; lane 7 into the column 5 gt later)", "- (unmeasured)").replace("ahead 28", "ahead - (no batch after it in the run)")), true);
        assert!(un.errors.is_empty(), "{:?}", un.errors);
        let fu = fails(&un, &pb, true);
        assert!(fu.iter().any(|x| x.contains("mouth clearance unmeasured")) && fu.iter().any(|x| x.contains("y-12 gap ahead of the next batch's first unmeasured")), "{fu:?}");
        let p15 = read(&format!("{T}{pa2}{}{sc}", pairs.replace("lane 3 -> lane 7 27", "lane 3 -> lane 7 15")), true);
        assert!(fails(&p15, &pb, true).iter().any(|x| x.contains("the y-12 pair lane 3 -> lane 7 15 gt (bar 16")), "{:?}", fails(&p15, &pb, true));
        let wrong = read(&format!("{T}{pa2}{}{sc}", pairs.replace("the column order at y 12 -1, 3, 7 per batch", "the column order at y 12 WRONG")), true);
        assert!(fails(&wrong, &pb, true).iter().any(|x| x.contains("the column order at y 12 wrong")), "{:?}", fails(&wrong, &pb, true));
        let miss = read(&format!("{T}{}{pairs}{sc}", pa2.replace("b1 lane 7 +101", "b1 lane 7 -")), true);
        assert!(miss.errors.is_empty() && fails(&miss, &pb, true).iter().any(|x| x.contains("1 batch cart(s) never passed y 12")), "{:?} {:?}", miss.errors, fails(&miss, &pb, true));
        let set9 = read(&format!("{T}{pa}{}", sc.replace("settle 11 gt", "settle 3 gt")), true);
        assert!(fails(&set9, &pb, true).iter().any(|x| x.contains("J(-1)'s settle 3 gt (entity-phase bar 2 + 2, its cause unnamed)")), "{:?}", fails(&set9, &pb, true));
        let clr9 = read(&format!("{T}{pa}{}", sc.replace("clear 12 gt", "clear 2 gt")), true);
        assert!(fails(&clr9, &pb, true).iter().any(|x| x.contains("J(-1)'s clear 2 gt (block-phase bar 1 + 2)")), "{:?}", fails(&clr9, &pb, true));
        let cart3 = read(&format!("{T}{pa}{}", sc.replace("cart-triggered clear 5 gt", "cart-triggered clear 3 gt")), true);
        assert!(fails(&cart3, &pb, true).iter().any(|x| x.contains("J(-1)'s cart-triggered clear 3 gt (entity-phase bar 2 + 2)")), "{:?}", fails(&cart3, &pb, true));
        // (a path (a) run without its y-12 gaps: unmeasured, a failure)
        let none = read(&format!("{T}path (a): ARM3 - REL per batch [51]\n{sc}"), true);
        assert!(fails(&none, &pb, true).iter().any(|x| x.contains("unmeasured")), "{:?}", fails(&none, &pb, true));
        assert!(!read(&format!("{T}junction settle / clear: nothing\n"), true).errors.is_empty());
    }

    /// The station calibration check (ironspec 19:52): batch 0's y-12 passes per lane from the path (a)
    /// line, else from the riders' feet times + the station lag + the cart's offset; a lane with neither is
    /// none (the judge refuses).
    #[test]
    fn station_calibration_check() {
        // (the path (a) line's passes first; else a cart's feet time + the station lag + its offset (k % 3);
        // a lane with neither: none (the check refuses))
        let pa = "path (a): ARM3 - REL per batch [51]; y-12 passes after REL: b0 lane -1 +55, b0 lane 3 +74, b0 lane 7 +101, napper +127; the napper's y-12 gap behind 26 and ahead 28; the least distance between two carts in the column 9.5\n";
        let r = read(&format!("{T}{pa}"), true);
        assert_eq!(station_check(&r, &[(-1, 55), (3, 74)], &[0, 10, 24], 11), vec![(-1, Some(55), 55, "path (a)"), (3, Some(74), 74, "path (a)")]);
        let plain = read(T, true);
        assert_eq!(plain.feet12.get(&(0, 0, -1)), Some(&44));
        assert_eq!(station_check(&plain, &[(-1, 55), (3, 74)], &[0, 10, 24], 11)[0], (-1, Some(55), 55, "the riders' feet"));
        assert_eq!(station_check(&plain, &[(-1, 55)], &[0, 10, 24], 9)[0].1, Some(53));
        assert_eq!(station_check(&plain, &[(5, 60)], &[0, 10, 24], 11)[0].1, None);
    }

    /// The per-cause line (ironspec 19:14): each switch barred at the bar of its cause's phase + 2 on both
    /// sides (block 1 -> 3, entity 2 -> 4); `-` is no such switch in the run; an unknown phase is an error.
    #[test]
    fn reads_switches_by_cause() {
        let by = "junction settle / clear by cause (ironspec 19:14; a press timed from the press tick): K(1) set (tap, block): settle 7 gt (its set at t 300, then cart 9 (batch 2, napper) entering at stored 0.307), clear 4 gt (cart 8 (batch 2, lane 7) out of the cell, then its set at t 300); K(1) reset (tap, block): settle 9 gt (the next batch cart in), clear 3 gt (the napper out); J(-1) re-arm (tap, block): settle 11 gt (its set at t 331, then cart 9 (batch 3, lane -1) entering at stored 0.315), clear 6 gt (cart 8 (batch 2, lane 7) out of the cell, then its set at t 331); J(-1) set (lane 7's D press, entity): settle 8 gt (…), clear 5 gt (…); J(3) reset (lane 3's D press, entity): settle -, clear 4 gt (…)\n";
        let r = read(&format!("{T}{by}"), true);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.switches.len(), 5);
        assert_eq!((r.switches[1].name.as_str(), r.switches[1].cause.as_str(), r.switches[1].entity, r.switches[1].settle, r.switches[1].clear), ("K(1) reset", "tap", false, Some(9), Some(3)));
        assert_eq!((r.switches[4].name.as_str(), r.switches[4].entity, r.switches[4].settle, r.switches[4].clear), ("J(3) reset", true, None, Some(4)));
        assert!(fails(&r, &bars(), true).is_empty(), "{:?}", fails(&r, &bars(), true));
        // (a block-phase clear of 2 and an entity-phase clear of 3 each fail; the split line's legacy
        // J gaps are not barred beside the per-cause line)
        let b2 = read(&format!("{T}{}", by.replace("clear 3 gt (the napper out)", "clear 2 gt (the napper out)")), true);
        assert!(fails(&b2, &bars(), true).iter().any(|x| x.contains("K(1) reset (tap)'s clear 2 gt (block-phase bar 1 + 2)")), "{:?}", fails(&b2, &bars(), true));
        let e3 = read(&format!("{T}{}", by.replace("clear 5 gt (…)", "clear 3 gt (…)")), true);
        assert!(fails(&e3, &bars(), true).iter().any(|x| x.contains("J(-1) set (lane 7's D press)'s clear 3 gt (entity-phase bar 2 + 2)")), "{:?}", fails(&e3, &bars(), true));
        let legacy = read(&format!("{}{by}", T.replace("J(-1) 17 gt", "J(-1) 1 gt")), true);
        assert!(!fails(&legacy, &bars(), true).iter().any(|x| x.contains("J(-1)'s gap")), "{:?}", fails(&legacy, &bars(), true));
        assert!(!read(&format!("{T}{}", by.replace("(tap, block)", "(tap, sideways)")), true).errors.is_empty());
        // (day-1's driver (ironspec 19:38): an entity-phase press entry lands +4 after a set edge, +6 after a
        // reset edge, block-phase: J(-1) set settle 8 -> 4, clear 5 -> 9; J(3) reset clear 4 -> 10; the taps
        // and a press the bench already routes through a driver (block) are not moved)
        let dl = ReturnBars { driver_latency: Some((4, 6)), ..bars() };
        let s = |name: &str| r.switches.iter().find(|s| s.name == name).cloned().expect(name);
        assert_eq!(effective(&s("J(-1) set"), &dl), (Some(4), Some(9), false, Some(4)));
        assert_eq!(effective(&s("J(3) reset"), &dl), (None, Some(10), false, Some(6)));
        assert_eq!(effective(&s("K(1) set"), &dl), (Some(7), Some(4), false, None));
        assert!(fails(&r, &dl, true).is_empty(), "{:?}", fails(&r, &dl, true));
        let s6 = read(&format!("{T}{}", by.replace("settle 8 gt (…)", "settle 6 gt (…)")), true);
        assert!(fails(&s6, &dl, true).iter().any(|x| x.contains("J(-1) set (lane 7's D press, with day-1's driver +4)'s settle 2 gt (block-phase bar 1 + 2)")), "{:?}", fails(&s6, &dl, true));
        let routed = read(&format!("{T}{}", by.replace("(lane 7's D press, entity)", "(lane 7's D press via the driver, block)")), true);
        assert_eq!(effective(routed.switches.iter().find(|s| s.name == "J(-1) set").expect("set"), &dl), (Some(8), Some(5), false, None));
    }

    /// ironspec's T rule bites (a positive control): a switched T with arms N/W/E (a row along x, its
    /// stem north) stored east_west fails (no settle gives it); stored north_east with its lever off
    /// passes (the unpowered settle); stored north_west with its lever off fails (the other settle);
    /// stored north_west with its lever on passes; a T with no driver on a face is no switched T. The
    /// driver may be any source on a face (G1's note on 5af88c7da: day-1's J/K driver is a repeater
    /// into the T): a repeater pointing into the T (on: north_west passes, north_east fails; off: the
    /// reverse), a repeater into the stone under the T (on: north_west passes); a repeater pointing
    /// away from the T drives nothing.
    #[test]
    fn t_rule_bites() {
        use redsim::block::Block;
        use redsim::pos::BlockPos;
        let tee = |stored: &str, extra: &[([i32; 3], String)]| -> Option<(String, Vec<String>)> {
            let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
            for x in -1..=1 {
                blocks.insert([x, -1, 0], "minecraft:stone".into());
            }
            blocks.insert([0, -1, -1], "minecraft:stone".into());
            blocks.insert([0, -1, 1], "minecraft:glass".into());
            blocks.insert([0, -2, 1], "minecraft:glass".into());
            blocks.insert([-1, 0, 0], "minecraft:rail[shape=east_west,waterlogged=false]".into());
            blocks.insert([1, 0, 0], "minecraft:rail[shape=east_west,waterlogged=false]".into());
            blocks.insert([0, 0, -1], "minecraft:rail[shape=north_south,waterlogged=false]".into());
            blocks.insert([0, 0, 0], format!("minecraft:rail[shape={stored},waterlogged=false]"));
            for (k, s) in extra {
                blocks.insert(*k, s.clone());
            }
            let mut w = redsim::world::World::new(BlockPos::new(-3, -3, -3), [7, 7, 7]);
            let cells: Vec<(BlockPos, Block)> = blocks.iter().map(|(k, s)| (BlockPos::new(k[0], k[1], k[2]), Block::parse(s).expect("a block"))).collect();
            w.place_raw(&cells);
            assert!(cells.iter().all(|(p, _)| w.contains(*p)) && w.errors.is_empty(), "the test's cells all inside its world: {:?}", w.errors);
            t_rule(&w, BlockPos::new(0, 0, 0))
        };
        let lever = |on: bool| vec![([0, 1, 0], format!("minecraft:lever[face=ceiling,facing=north,powered={on}]")), ([0, 2, 0], "minecraft:glass".to_string())];
        let (line, bad) = tee("east_west", &lever(false)).expect("a switched T");
        assert!(bad.iter().any(|b| b.contains("which no settle gives")), "{line}: {bad:?}");
        let (line, bad) = tee("north_east", &lever(false)).expect("a switched T");
        assert!(bad.is_empty(), "{line}: {bad:?}");
        assert!(line.contains("settles north_east unpowered, north_west powered") && (line.contains("north_west, north_east") || line.contains("north_east, north_west")), "{line}");
        let (line, bad) = tee("north_west", &lever(false)).expect("a switched T");
        assert!(bad.iter().any(|b| b.contains("the other settle")), "{line}: {bad:?}");
        let (line, bad) = tee("north_west", &lever(true)).expect("a switched T");
        assert!(bad.is_empty(), "{line}: {bad:?}");
        assert!(tee("north_east", &[]).is_none());
        // (a repeater on the T's south face pointing into it: its back (facing) south, its output north)
        let rep = |on: bool, facing: &str| vec![([0, 0, 1], format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered={on}]"))];
        let (line, bad) = tee("north_west", &rep(true, "south")).expect("a repeater-driven T");
        assert!(bad.is_empty() && line.contains("repeater (0,0,1) (on)"), "{line}: {bad:?}");
        let (line, bad) = tee("north_east", &rep(true, "south")).expect("a repeater-driven T");
        assert!(bad.iter().any(|b| b.contains("the other settle")), "{line}: {bad:?}");
        let (line, bad) = tee("north_east", &rep(false, "south")).expect("a repeater-driven T, off");
        assert!(bad.is_empty() && line.contains("repeater (0,0,1) (off)"), "{line}: {bad:?}");
        let (line, bad) = tee("north_west", &rep(false, "south")).expect("a repeater-driven T, off");
        assert!(bad.iter().any(|b| b.contains("the other settle")), "{line}: {bad:?}");
        assert!(tee("north_east", &rep(false, "north")).is_none(), "a repeater pointing away drives nothing");
        // (a repeater into the stone under the T, from the south (day-1's hold release drive))
        let under = vec![([0, -1, 1], "minecraft:repeater[delay=1,facing=south,locked=false,powered=true]".to_string())];
        let (line, bad) = tee("north_west", &under).expect("a T driven through its support");
        assert!(bad.is_empty() && line.contains("via stone (0,-1,0)"), "{line}: {bad:?}");
        // (an idle comparator pointing into the T is a driver by its shape (G1's note on c620a1429): the
        // T is a switched T, judged, not skipped)
        let comp = vec![([0, 0, 1], "minecraft:comparator[facing=south,mode=compare,powered=false]".to_string())];
        let (line, bad) = tee("north_east", &comp).expect("a comparator-driven T");
        assert!(bad.is_empty() && line.contains("comparator (0,0,1) (off)"), "{line}: {bad:?}");
        let (line, bad) = tee("east_west", &comp).expect("a comparator-driven T");
        assert!(bad.iter().any(|b| b.contains("which no settle gives")), "{line}: {bad:?}");
        assert!(tee("north_east", &[([0, 0, 1], "minecraft:comparator[facing=north,mode=compare,powered=false]".to_string())]).is_none(), "a comparator pointing away drives nothing");
        // (a detector rail in the T's 4th neighbour column (south, one down: (0,-1,1)) is a 4th potential
        // connection: the T's switching disabled (tick-exact's boundary row, farmopt 19:58), a failure
        // naming it; the lever-switched T itself unchanged)
        let mut fourth = lever(false);
        fourth.push(([0, -1, 1], "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]".to_string()));
        fourth.push(([0, -2, 1], "minecraft:stone".to_string()));
        let (line, bad) = tee("north_east", &fourth).expect("a switched T with a 4th neighbour");
        assert!(bad.iter().any(|b| b.contains("4 potential connections") && b.contains("(0,-1,1) detector_rail")), "{line}: {bad:?}");
    }
}
