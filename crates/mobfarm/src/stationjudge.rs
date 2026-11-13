//! `mctas mobfarm station-judge` (cc767b78; farmopt 2026-11-12 00:53, ironspec's L1‴ rows): the
//! judge of dfb89499's F1 + Q2 station (`drop-bench --layout station ... --l1`). Under L1‴ the farm
//! never freezes in normal operation and a restart loads its chunks before its first tick, so the rows
//! are: the L1 mode (the controller and its timers on game ticks), a save/stop/start at every phase
//! (the bench's `--restart T,ORDER`: the shared helper's `liveness::restart` in each of
//! `liveness::Restart::rows()`, the save order, its reverse and 8 seeded orders), and the cold start
//! (the fresh design with the departing carts absent: `--dep-riders x,x,x`); for the pearl alternative
//! also the load window (the bench's login of D gt at every phase, the controller running through it),
//! reported apart. A run passes when every cart landed on a braked pocket at least the L1 edge bar
//! (4 gt) before its re-brake and is at rest before the next summon, with 0 wrong pockets, contacts,
//! riders hurt, unseated ticks, departing carts stuck, refill failures and trapdoors shut onto their cart
//! (and, with `launch_min`, every launch that far after the one before), and the stand summons at every
//! due S: the call gate (ironspec 19:51 / 20:03 / 20:07 / 20:14), which retired the rest bar outright:
//! >= 1 GolemTry from a pocket rider at every due S of every lead row, the callers counted willing and
//! ticking at the tick's start (VillagerPanicTrigger's official skip at s + 61 + 62k excluded), a reload's
//! re-phase within 20 gt before S allowed one miss per restart (the one-summon-per-disturbance bar);
//! the removed-rider controls (each pocket's rider removed in turn) must show the miss; the golem rate per
//! k beside 1 - m^k prints (its binomial 99 % interval), gating only once the bench's cells match the
//! design of record over the three riders' search volumes. Printed per REL over its seeds: the lead's
//! rows' failing phases per order, their least rest margin, landing lead and launch spacing, the call
//! gate's classes; the logins' failing phases and margins apart, and the R line's switches while the
//! entities were frozen; the timed edges' thresholds: the same matrix at other widths of one edge each.

use crate::drop::{DropOpts, DropStats, bench};
use redsim::entity::mob::farm::FarmSpec;

/// The L1 edge bar (farmopt 2026-11-12 02:39, ironspec): every cart landed at least this long before
/// its pocket's re-brake, so the pocket's rail is unpowered before the arriving cart lands. Inherited
/// (ironspec's bar audit 19:07 section 2, 19:24, 19:51): the pocket rail is a powered rail whose power
/// follows its driver in the same tick (PoweredRailBlock.updateState inside neighborChanged; redsim's
/// `redstone::blocks::rail::update_state`: G1), so the power-off latency is the M16bw repeater's scheduled
/// delay plus phase order, to be re-derived from day-1's ctl-pocket-bench (P's fall after the switch's
/// removal on the built (b) layout); the 4's "+ 1 for the re-shape" is not a real delay. It binds nothing
/// today (landing >= 31 gt after the re-brake); printed "inherited".
pub const EDGE_BAR: i64 = 4;

/// The window before a due S within which a restart's reload may re-phase a rider's panic and cost that
/// S its call (ironspec 20:07: a reload re-phase <= 20 gt counts against the one-summon-per-disturbance
/// bar): one such miss per restart row is allowed, a second fails.
pub const REPHASE_WINDOW: i64 = 20;

/// A run's failures (empty: it passes): the bars, with the landing lead held to `edge` gt (the L1
/// edge bar, [`EDGE_BAR`]). (The rest bar retired onto the call gate, ironspec 19:51 / 20:07: "rest >= 23"
/// is no bar; a cart still not at rest at S fails as before.)
pub fn fails(s: &DropStats, launch_min: i64, edge: i64) -> Vec<&'static str> {
    let mut v = Vec::new();
    if s.fall == i64::MAX {
        v.push("a cart never landed");
    }
    match s.brake_lead {
        Some(l) if l < 0 => v.push("landed on a powered pocket"),
        Some(l) if l < edge => v.push("landed within the edge bar of its pocket's re-brake"),
        _ => {}
    }
    if s.rest == i64::MIN {
        v.push("not all at rest");
    } else if s.rest < 0 {
        v.push("at rest after the next summon");
    }
    if s.wrong > 0 {
        v.push("a wrong pocket");
    }
    if s.contacts > 0 {
        v.push("a contact");
    }
    if s.hurt > 0 {
        v.push("a rider hurt");
    }
    if s.unseated > 0 {
        v.push("a rider unseated");
    }
    if s.stuck > 0 {
        v.push("a departing cart stuck");
    }
    // (golem-stamp path (a), ironspec 19:07: a departing rider inside a next-batch caller's box
    // after its move on S - 3 = T + 97 (the summon at game time % 100 == 0 stamps every villager in
    // the caller's 10-box), a verdict, G1 on 3939d2012)
    if s.lift_last.iter().any(|l| l.is_some_and(|t| t >= 97)) {
        v.push("a departing rider in a caller's box after S - 3");
    }
    // (ironspec 21:25 (4): a cart's move stopped by a block that is no declared stop or buffer)
    if s.undeclared_contacts > 0 {
        v.push("a cart's undeclared block contact");
    }
    // (ironspec 23:30: a sight ray crossing a stand-in's support between T and S, or the sight
    // check's positive control not flagging while a scarer is in sight range; a departing rider
    // missing without dep_absent)
    if s.sight_hit_count > 0 {
        v.push("a sight ray crossing a stand-in's support");
    }
    if s.missing_riders > 0 {
        v.push("a departing rider missing");
    }
    // (G1 on 82f9546f (c): the held slope rail off its window's state at a tick's end)
    if s.hold_breaks > 0 {
        v.push("the held slope rail off its window (a bench artefact)");
    }
    if s.refill_fail > 0 {
        v.push("the refill failed");
    }
    if s.oob > 0 {
        v.push("cells set outside the world's box (dropped)");
    }
    if s.r1_late > 0 {
        v.push("an R_1 launch after its pulse ended (or none)");
    }
    if s.r1_cell_late > 0 && s.r1_late == 0 {
        v.push("an R_1 cart off its cell only after its pulse ended (or never)");
    }
    if s.shut_onto > 0 {
        v.push("a trapdoor shut onto its cart");
    }
    if launch_min > 0 && s.launch_gap.is_some_and(|g| g < launch_min) {
        v.push("a launch spacing under the bar");
    }
    v
}

/// The call gate's misses in one run (ironspec 19:51 / 20:07): every due S with no GolemTry from a pocket
/// rider (`calls == 0`: drop-bench's `calls` are the stand's, each attributed to a pocket rider by its entity
/// id, `pocket_of`), each with its class; a golem born with no pocket rider's call (a call from elsewhere, G1
/// on d96461902) is still a miss; in a restart row one miss within [`REPHASE_WINDOW`] gt after the restart is
/// the one-summon-per-disturbance allowance (returned apart). Returns (failing misses, allowed re-phase
/// misses).
pub fn summon_misses(s: &DropStats) -> (Vec<String>, Vec<String>) {
    let (mut bad, mut allowed) = (Vec::new(), Vec::new());
    for d in &s.summon_due {
        if d.calls > 0 {
            continue;
        }
        let what = format!("S T+{}: {} (eligible {})", d.s, d.class, d.eligible);
        match d.restart_before {
            Some(t) if d.s - t <= REPHASE_WINDOW && allowed.is_empty() => allowed.push(format!("{what}, {} gt after the restart", d.s - t)),
            _ => bad.push(what),
        }
    }
    (bad, allowed)
}

/// The removed-rider control's verdict on one run (a pocket's rider removed): it bites when no due S got a
/// call and every one reads "fewer than 3 willing" (ironspec 19:51: the control shows the miss).
pub fn control_bites(s: &DropStats) -> Result<(), String> {
    if s.summon_due.is_empty() {
        return Err("no due S measured".into());
    }
    match s.summon_due.iter().find(|d| d.calls > 0 || d.born || d.class != "fewer than 3 willing") {
        Some(d) => Err(format!("S T+{}: {} call(s), {} (eligible {})", d.s, d.calls, d.class, d.eligible)),
        None => Ok(()),
    }
}

/// The golem rate per k (the eligible count at S) over `dues`, beside 1 - m^k, m = (1 - 40..44/289)^10
/// (the spot odds of the pad), with its binomial 99 % (Wilson) interval: (the line, whether the interval
/// misses the range for any k).
pub fn golem_rate(dues: &[&crate::drop::SummonDue]) -> (String, bool) {
    let range = |k: i32| (1.0 - (1.0f64 - 40.0 / 289.0).powi(10).powi(k), 1.0 - (1.0f64 - 44.0 / 289.0).powi(10).powi(k));
    let mut miss = false;
    let parts: Vec<String> = (0..=3)
        .rev()
        .map(|k| {
            let at: Vec<&&crate::drop::SummonDue> = dues.iter().filter(|d| d.eligible as i32 == k).collect();
            let born = at.iter().filter(|d| d.born).count();
            if k == 0 || at.is_empty() {
                return format!("k {k}: {} S, {born} born", at.len());
            }
            let (lo, hi) = range(k);
            let (n, p, z) = (at.len() as f64, born as f64 / at.len() as f64, 2.576);
            let c = (p + z * z / (2.0 * n)) / (1.0 + z * z / n);
            let h = z / (1.0 + z * z / n) * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
            let out = c + h < lo || c - h > hi;
            miss |= out;
            format!("k {k}: {born} / {} born ({:.1} %, 99 % interval {:.1}..{:.1}) against {:.1}..{:.1} %{}", at.len(), 100.0 * p, 100.0 * (c - h), 100.0 * (c + h), 100.0 * lo, 100.0 * hi, if out { ": MISSES" } else { "" })
        })
        .collect();
    (parts.join("; "), miss)
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

/// One run of the matrix: the REL and the fault.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Control,
    /// A save/stop/start before this tick (bench ticks from T) in this order (the shared helper's
    /// `liveness::restart`).
    Restart(i64, crate::liveness::Restart),
    /// A login (level-32 freeze) of D gt from this tick.
    Login(i64, i64),
    /// The cold start: the departing carts absent.
    Cold,
    /// The call gate's removed-rider control: the control run with this pocket's rider removed (it must
    /// show the miss at every due S).
    NoRider(usize),
}

impl Row {
    /// The lead's rows (they decide): the control, the cold start, the restarts.
    fn lead(&self) -> bool {
        !matches!(self, Row::Login(..) | Row::NoRider(_))
    }
}

/// The matrix.
#[derive(Clone, Debug)]
pub struct StationJudgeOpts {
    pub rels: Vec<i64>,
    /// The bench's seeds: every row runs at each.
    pub seeds: Vec<i64>,
    pub starts: (i64, i64),
    /// Login lengths (empty: no login rows).
    pub ds: Vec<i64>,
    pub restarts: bool,
    pub cold: bool,
    pub launch_min: i64,
    /// The landing lead's bar (gt; [`EDGE_BAR`]).
    pub edge: i64,
    /// The call gate (ironspec 19:51): every run with the bench's direct summon check, the misses failing
    /// the lead's rows, the removed-rider controls run per REL.
    pub summon_gate: bool,
    pub jobs: usize,
}

/// The judge over one base (its rel, seed, restarts, logins and dep_absent replaced; `cold_spec` the
/// design with the departing carts' riders absent). The restart rows run in each of the shared
/// helper's orders (`liveness::Restart::rows()`: the save order, its reverse and 8 seeded orders; the
/// cold start is its own row, the fresh design). Returns (lines, failing runs in the lead's rows).
pub fn judge(spec: &FarmSpec, cold_spec: &FarmSpec, base: &DropOpts, o: &StationJudgeOpts) -> Result<(Vec<String>, usize), String> {
    let orders = crate::liveness::Restart::rows();
    let mut runs: Vec<(i64, i64, Row)> = Vec::new();
    for &r in &o.rels {
        for &sd in &o.seeds {
            runs.push((r, sd, Row::Control));
            if o.cold {
                runs.push((r, sd, Row::Cold));
            }
            for t in o.starts.0..=o.starts.1 {
                if o.restarts {
                    for how in &orders {
                        runs.push((r, sd, Row::Restart(t, *how)));
                    }
                }
                for &d in &o.ds {
                    runs.push((r, sd, Row::Login(t, d)));
                }
            }
        }
        // (the call gate's removed-rider controls: each pocket's rider removed in turn, at the first seed)
        if o.summon_gate {
            for p in 0..3 {
                runs.push((r, o.seeds.first().copied().unwrap_or(0), Row::NoRider(p)));
            }
        }
    }
    type Res = ((i64, i64, Row), DropStats);
    let mut results: Vec<Res> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<Res, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&(r, sd, row)| {
                    let mut lo = base.clone();
                    lo.rel = r;
                    lo.seed = sd;
                    lo.trace = false;
                    lo.restarts = Vec::new();
                    lo.logins = Vec::new();
                    lo.dep_absent = [false; 3];
                    lo.summon_check = o.summon_gate;
                    lo.remove_rider = None;
                    let mut sp = spec;
                    match row {
                        Row::Control => {}
                        Row::Restart(t, how) => lo.restarts = vec![(t, how)],
                        Row::Login(t, d) => lo.logins = vec![(t, d)],
                        Row::Cold => {
                            lo.dep_absent = [true; 3];
                            sp = cold_spec;
                        }
                        Row::NoRider(p) => lo.remove_rider = Some(p),
                    }
                    sc.spawn(move || bench(sp, &lo).map(|(_, s)| ((r, sd, row), s)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a station judge thread panicked")).collect()
        });
        for r in got {
            results.push(r?);
        }
    }
    // (a lead row also fails the call gate on a due S with no call (beyond a restart's one re-phase miss))
    let fl = |s: &DropStats| -> Vec<&'static str> {
        let mut v = fails(s, o.launch_min, o.edge);
        if o.summon_gate && !summon_misses(s).0.is_empty() {
            v.push("a due S with no call from the stand");
        }
        v
    };
    let gt = |v: Option<i64>| v.map_or("-".to_string(), |x| if x == i64::MIN { "FAIL".to_string() } else { format!("{x} gt") });
    let mut out = Vec::new();
    let mut total = 0usize;
    for &r in &o.rels {
        let mine: Vec<&Res> = results.iter().filter(|((rr, _, _), _)| *rr == r).collect();
        let lead: Vec<&&Res> = mine.iter().filter(|((_, _, row), _)| row.lead()).collect();
        let logins: Vec<&&Res> = mine.iter().filter(|((_, _, row), _)| matches!(row, Row::Login(..))).collect();
        // (the control and the cold start at every seed: their failures together)
        let union = |want: Row| -> Option<Vec<&'static str>> {
            let rows: Vec<&&Res> = mine.iter().filter(|((_, _, row), _)| *row == want).collect();
            if rows.is_empty() {
                return None;
            }
            let set: std::collections::BTreeSet<&'static str> = rows.iter().flat_map(|(_, s)| fl(s)).collect();
            Some(set.into_iter().collect())
        };
        let control = union(Row::Control).unwrap_or_default();
        let cold = union(Row::Cold);
        let failing: Vec<&&&Res> = lead.iter().filter(|((_, _, row), s)| *row != Row::Control && !fl(s).is_empty()).collect();
        let control_fails = mine.iter().filter(|((_, _, row), s)| *row == Row::Control && !fl(s).is_empty()).count();
        let login_failing = logins.iter().filter(|(_, s)| !fl(s).is_empty()).count();
        total += failing.len() + control_fails;
        // (the margins over the lead's rows only, the logins' apart: no number in the verdict from a
        // pearl row)
        let rest = lead.iter().map(|(_, s)| s.rest).min();
        let rest_c = mine.iter().filter(|((_, _, row), _)| *row == Row::Control).map(|(_, s)| s.rest).min();
        let lead_min = lead.iter().filter_map(|(_, s)| s.brake_lead).min();
        // (no vacuous passes, ironspec 23:56: the edge bar's samples are the lead rows that measured a
        // landing lead; none fails the REL as NOT JUDGED)
        let lead_n = lead.iter().filter(|(_, s)| s.brake_lead.is_some()).count();
        let lead_judged = if lead_n == 0 {
            total += 1;
            "NOT JUDGED (0 samples: no lead row measured a landing lead)".to_string()
        } else {
            format!("{} over {lead_n} rows", gt(lead_min))
        };
        let gap = lead.iter().filter_map(|(_, s)| s.launch_gap).min();
        let gap_c = mine.iter().filter(|((_, _, row), _)| *row == Row::Control).filter_map(|(_, s)| s.launch_gap).min();
        let rest_l = logins.iter().map(|(_, s)| s.rest).min();
        let lead_l = logins.iter().filter_map(|(_, s)| s.brake_lead).min();
        let gap_l = logins.iter().filter_map(|(_, s)| s.launch_gap).min();
        let r_frozen = logins.iter().filter(|(_, s)| s.r_frozen > 0).count();
        let shut = logins.iter().filter(|(_, s)| s.shut_onto > 0).count();
        let n_restarts = lead.iter().filter(|((_, _, row), _)| matches!(row, Row::Restart(..))).count();
        out.push(format!(
            "REL T+{r} ({} seed(s)): the lead's rows (the control, the cold start, {n_restarts} restarts): failing {}; the control {}; the cold start {}; the least rest margin {} (unfrozen {}; printed: the rest bar retired onto the call gate, ironspec 19:51 / 20:07); the least landing lead {} (the bar {} gt, inherited (ironspec 19:24 / 19:51): its basis the M16bw repeater's delay + phase order); the least launch spacing {} (unfrozen {}; reported, the bar {}) | the pearl alternative's logins ({} runs; reported, out of the lead's scope): failing {login_failing}; the least rest margin {}, landing lead {}, launch spacing {}; runs with a trapdoor shut onto its cart {shut}; runs with the R line switching while the entities were frozen {r_frozen}",
            o.seeds.len(),
            failing.len() + control_fails,
            if control.is_empty() { "clean".to_string() } else { format!("FAILS ({})", control.join(", ")) },
            match cold {
                None => "-".to_string(),
                Some(c) if c.is_empty() => "clean".to_string(),
                Some(c) => format!("FAILS ({})", c.join(", ")),
            },
            gt(rest),
            gt(rest_c),
            lead_judged,
            o.edge,
            gt(gap),
            gt(gap_c),
            if o.launch_min > 0 { format!("{} gt", o.launch_min) } else { "none".to_string() },
            logins.len(),
            gt(rest_l),
            gt(lead_l),
            gt(gap_l),
        ));
        // (the call gate over the lead's rows: the due S, the misses (failing, and the restarts' allowed
        // re-phase misses), the classes; the removed-rider controls; the golem rate per k, printed only:
        // it validates the bench's pad until its cells match the design of record over the three riders'
        // search volumes, ironspec 20:14)
        if o.summon_gate {
            let dues: Vec<&crate::drop::SummonDue> = lead.iter().flat_map(|(_, s)| s.summon_due.iter()).collect();
            let mut classes: Vec<(&str, usize)> = Vec::new();
            for d in &dues {
                match classes.iter_mut().find(|c| c.0 == d.class) {
                    Some(c) => c.1 += 1,
                    None => classes.push((d.class, 1)),
                }
            }
            let (mut bad, mut allowed) = (Vec::new(), 0usize);
            for ((_, sd, row), s) in lead.iter().map(|x| &***x) {
                let (b, a) = summon_misses(s);
                allowed += a.len();
                bad.extend(b.into_iter().map(|m| format!("{row:?} seed {sd}: {m}")));
            }
            let controls: Vec<String> = mine
                .iter()
                .filter_map(|((_, _, row), s)| if let Row::NoRider(p) = row { Some((p, s)) } else { None })
                .map(|(p, s)| match control_bites(s) {
                    Ok(()) => format!("P{p} removed: the miss at every due S ({})", s.summon_due.len()),
                    Err(e) => {
                        total += 1;
                        format!("P{p} removed: FAILS to show the miss ({e})")
                    }
                })
                .collect();
            let (rate, miss) = golem_rate(&dues);
            let thr = dues.iter().all(|d| d.calls == 0) || lead.iter().flat_map(|(_, s)| s.summon_calls.iter()).all(|c| c.threshold.is_some());
            // (no vacuous passes, ironspec 23:56: a lead row with no due S measured is NOT JUDGED and fails,
            // as does a REL with none at all)
            let unjudged = lead.iter().filter(|(_, s)| s.summon_due.is_empty()).count();
            total += unjudged + usize::from(dues.is_empty());
            out.push(format!(
                "  the call gate (>= 1 GolemTry from a pocket rider at every due S, the callers willing and ticking at the tick's start): {}, {} missed{}{}{}; classes {}; the removed-rider controls: {}; the golem rate (printed only: it validates the bench's pad until the bench's cells match the design of record, ironspec 20:14): {rate}{}{}",
                if dues.is_empty() { "NOT JUDGED (0 samples: no due S over the lead's rows)".to_string() } else { format!("{} due S over {} lead rows", dues.len(), lead.len()) },
                bad.len(),
                if bad.is_empty() { String::new() } else { format!(" ({})", bad.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) },
                if allowed > 0 { format!(", {allowed} allowed (a restart's re-phase within {REPHASE_WINDOW} gt before S, one per disturbance)") } else { String::new() },
                if unjudged > 0 { format!("; {unjudged} lead rows NOT JUDGED (0 due S measured), failing") } else { String::new() },
                classes.iter().map(|(c, n)| format!("{c} {n}")).collect::<Vec<_>>().join(", "),
                if controls.is_empty() { "none run".to_string() } else { controls.join("; ") },
                if miss { " (an interval misses its range: printed, not barred)" } else { "" },
                if thr { "" } else { "; the calls' threshold not yet carried by redsim's GolemTry (onechunk's fields): the panic path's 3 assumed" }
            ));
        }
        for how in &orders {
            let mut ts: Vec<i64> = failing.iter().filter_map(|((_, _, row), _)| if let Row::Restart(t, h) = row { (h == how).then_some(*t) } else { None }).collect();
            ts.sort();
            ts.dedup();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<&str> = failing.iter().filter(|((_, _, row), _)| matches!(row, Row::Restart(_, h) if h == how)).flat_map(|(_, s)| fl(s)).collect();
                out.push(format!("  restarts in the {} order: failing at T+{} ({})", how.label(), bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
        for &d in &o.ds {
            let bad: Vec<&&&Res> = logins.iter().filter(|((_, _, row), s)| matches!(row, Row::Login(_, dd) if *dd == d) && !fl(s).is_empty()).collect();
            let mut ts: Vec<i64> = bad.iter().filter_map(|((_, _, row), _)| if let Row::Login(t, _) = row { Some(*t) } else { None }).collect();
            ts.sort();
            ts.dedup();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<&str> = bad.iter().flat_map(|(_, s)| fl(s)).collect();
                out.push(format!("  logins of {d} gt (the pearl alternative, reported): failing at T+{} ({})", bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
    }
    Ok((out, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bars bite (positive controls): a run landed 2 gt before its re-brake fails at the edge bar of
    /// 4 and passes at 2; a powered pocket fails; a cart at rest after the next summon fails; a rest margin
    /// of 3 gt no longer fails (the rest bar retired onto the call gate, ironspec 19:51 / 20:07); the
    /// measured margins pass.
    #[test]
    fn edge_bar() {
        let ok = DropStats { rest: 34, brake_lead: Some(23), fall: 30, ..Default::default() };
        assert!(fails(&ok, 0, EDGE_BAR).is_empty());
        let near_summon = DropStats { rest: 3, ..ok.clone() };
        assert!(fails(&near_summon, 0, EDGE_BAR).is_empty(), "the rest margin is printed, not barred");
        let near_rebrake = DropStats { brake_lead: Some(2), ..ok.clone() };
        assert_eq!(fails(&near_rebrake, 0, EDGE_BAR), vec!["landed within the edge bar of its pocket's re-brake"]);
        assert!(fails(&near_rebrake, 0, 2).is_empty());
        let powered = DropStats { brake_lead: Some(-1), ..ok.clone() };
        assert_eq!(fails(&powered, 0, EDGE_BAR), vec!["landed on a powered pocket"]);
        let late = DropStats { rest: -2, ..ok.clone() };
        assert_eq!(fails(&late, 0, EDGE_BAR), vec!["at rest after the next summon"]);
        assert!(Row::Restart(5, crate::liveness::Restart::rows()[0]).lead() && Row::Cold.lead() && !Row::Login(5, 2).lead() && !Row::NoRider(0).lead());
    }

    fn due(s: i64, eligible: u8, calls: u8, born: bool, class: &'static str, restart_before: Option<i64>) -> crate::drop::SummonDue {
        crate::drop::SummonDue { s, eligible, calls, born, class, restart_before, ..Default::default() }
    }

    /// The call gate bites (ironspec 19:51 / 20:07): a due S with no call fails, whatever its class; a call
    /// (born or a spot miss) passes; in a restart row one miss within 20 gt after the restart is allowed (the
    /// one-summon-per-disturbance bar) and a second fails, as does one 21 gt after; the removed-rider control
    /// bites only when every due S reads "fewer than 3 willing" with no call.
    #[test]
    fn call_gate() {
        let called = DropStats { summon_due: vec![due(100, 3, 1, true, "golem born", None), due(200, 3, 2, false, "spot miss", None)], ..Default::default() };
        assert_eq!(summon_misses(&called), (vec![], vec![]));
        let missed = DropStats { summon_due: vec![due(100, 3, 1, true, "golem born", None), due(200, 2, 0, false, "fewer than 3 willing", None)], ..Default::default() };
        let (bad, allowed) = summon_misses(&missed);
        assert_eq!((bad.len(), allowed.len()), (1, 0));
        assert!(bad[0].contains("S T+200: fewer than 3 willing (eligible 2)"), "{bad:?}");
        // (a golem born at S with no pocket rider's call, a call from elsewhere: still a miss)
        let elsewhere = DropStats { summon_due: vec![due(100, 3, 0, true, "golem born", None)], ..Default::default() };
        assert_eq!(summon_misses(&elsewhere).0.len(), 1, "a golem with no pocket rider's call misses");
        let ok = DropStats { rest: 34, brake_lead: Some(23), fall: 30, ..missed.clone() };
        assert!(fails(&ok, 0, EDGE_BAR).is_empty(), "the gate is the judge's (`fl`), not `fails`'");
        // (a restart at T+150: one miss at S T+170 (20 gt after) allowed; a second at T+270 fails; one at
        // S T+171 (21 gt after) fails)
        let rephase = DropStats { summon_due: vec![due(170, 1, 0, false, "no panicking caller", Some(150))], ..Default::default() };
        assert_eq!((summon_misses(&rephase).0.len(), summon_misses(&rephase).1.len()), (0, 1));
        let twice = DropStats { summon_due: vec![due(170, 1, 0, false, "no panicking caller", Some(150)), due(190, 1, 0, false, "no panicking caller", Some(150))], ..Default::default() };
        assert_eq!((summon_misses(&twice).0.len(), summon_misses(&twice).1.len()), (1, 1));
        let late = DropStats { summon_due: vec![due(171, 1, 0, false, "no panicking caller", Some(150))], ..Default::default() };
        assert_eq!((summon_misses(&late).0.len(), summon_misses(&late).1.len()), (1, 0));
        // (the removed-rider control)
        let bites = DropStats { summon_due: vec![due(100, 2, 0, false, "fewer than 3 willing", None), due(200, 2, 0, false, "fewer than 3 willing", None)], ..Default::default() };
        assert_eq!(control_bites(&bites), Ok(()));
        assert!(control_bites(&called).is_err(), "a control with a call doesn't bite");
        let other = DropStats { summon_due: vec![due(100, 2, 0, false, "no caller ticking", None)], ..Default::default() };
        assert!(control_bites(&other).is_err(), "a miss of another class doesn't show the removed rider");
        assert!(control_bites(&DropStats::default()).is_err(), "no due S measured");
    }

    /// The golem rate per k (printed): at k = 1 (1 - m, m = (1 - 40..44/289)^10: 77.4..80.8 %) 79 of 100
    /// born fits and 100 of 100 misses (its 99 % interval 95..100 %); k = 0 only counts.
    #[test]
    fn golem_rate_interval() {
        let at = |n: usize, born: usize, k: u8| -> Vec<crate::drop::SummonDue> { (0..n).map(|i| due(i as i64, k, 1, i < born, if i < born { "golem born" } else { "spot miss" }, None)).collect() };
        let fit = at(100, 79, 1);
        let (line, miss) = golem_rate(&fit.iter().collect::<Vec<_>>());
        assert!(!miss && line.contains("k 1: 79 / 100 born"), "{line}");
        let all = at(100, 100, 1);
        let (line, miss) = golem_rate(&all.iter().collect::<Vec<_>>());
        assert!(miss && line.contains(": MISSES"), "{line}");
        let none = at(5, 0, 0);
        assert_eq!(golem_rate(&none.iter().collect::<Vec<_>>()).0.split("; ").last(), Some("k 0: 5 S, 0 born"));
    }
}
