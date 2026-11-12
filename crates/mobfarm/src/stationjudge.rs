//! `mctas mobfarm station-judge` (cc767b78; farmopt 2026-11-12 00:53, ironspec's L1‴ rows): the
//! judge of dfb89499's F1 + Q2 station (`drop-bench --layout station ... --l1`). Under L1‴ the farm
//! never freezes in normal operation and a restart loads its chunks before its first tick, so the rows
//! are: the L1 mode (the controller and its timers on game ticks), a save/stop/start at every phase
//! (the bench's `--restart T,ORDER`: the shared helper's `liveness::restart` in the save order, its
//! reverse and 8 seeded orders), and the cold start
//! (the departing carts absent: `--dep-riders x,x,x`); for the pearl alternative also the load window
//! (the bench's login of D gt at every phase, the controller running through it). A run passes when
//! every cart landed on a braked pocket and is at rest before the next summon, with 0 wrong pockets,
//! contacts, riders hurt, unseated ticks, departing carts stuck, refill failures and trapdoors shut
//! onto their cart, and every launch at least `launch_min` gt after the one before (the lift's 12 + the
//! L1 bar's 4). Printed per REL: the failing phases per row, the least rest margin, landing lead and
//! launch spacing, the R line's switches while the entities were frozen; the timed edges'
//! thresholds: the same matrix at other widths of one edge each.

use crate::drop::{DropOpts, DropStats, bench};
use redsim::entity::mob::farm::FarmSpec;

/// A run's failures (empty: it passes).
pub fn fails(s: &DropStats, launch_min: i64) -> Vec<&'static str> {
    let mut v = Vec::new();
    if s.fall == i64::MAX {
        v.push("a cart never landed");
    }
    if s.brake_lead.is_some_and(|l| l < 0) {
        v.push("landed on a powered pocket");
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
    if s.refill_fail > 0 {
        v.push("the refill failed");
    }
    if s.shut_onto > 0 {
        v.push("a trapdoor shut onto its cart");
    }
    if launch_min > 0 && s.launch_gap.is_some_and(|g| g < launch_min) {
        v.push("a launch spacing under the bar");
    }
    v
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
}

/// The matrix.
#[derive(Clone, Debug)]
pub struct StationJudgeOpts {
    pub rels: Vec<i64>,
    pub starts: (i64, i64),
    /// Login lengths (empty: no login rows).
    pub ds: Vec<i64>,
    pub restarts: bool,
    pub cold: bool,
    pub launch_min: i64,
    pub jobs: usize,
}

/// The restart rows' orders: the save order, its reverse and 8 seeded orders (ironspec's
/// reload-order rule; the cold start is its own row, the fresh design).
pub fn restart_orders() -> Vec<crate::liveness::Restart> {
    use redsim::reload::Order;
    let mut v = vec![crate::liveness::Restart::Load(Order::Save), crate::liveness::Restart::Load(Order::Reverse)];
    v.extend((1..=8).map(|k| crate::liveness::Restart::Load(Order::Seed(k))));
    v
}

/// The judge over one base (its rel, restarts, logins and dep_absent replaced; `cold_spec` the design
/// with the departing carts' riders absent). Returns (lines, failing runs).
pub fn judge(spec: &FarmSpec, cold_spec: &FarmSpec, base: &DropOpts, o: &StationJudgeOpts) -> Result<(Vec<String>, usize), String> {
    let mut runs: Vec<(i64, Row)> = Vec::new();
    for &r in &o.rels {
        runs.push((r, Row::Control));
        if o.cold {
            runs.push((r, Row::Cold));
        }
        for t in o.starts.0..=o.starts.1 {
            if o.restarts {
                for how in restart_orders() {
                    runs.push((r, Row::Restart(t, how)));
                }
            }
            for &d in &o.ds {
                runs.push((r, Row::Login(t, d)));
            }
        }
    }
    let mut results: Vec<((i64, Row), DropStats)> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<((i64, Row), DropStats), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&(r, row)| {
                    let mut lo = base.clone();
                    lo.rel = r;
                    lo.trace = false;
                    lo.restarts = Vec::new();
                    lo.logins = Vec::new();
                    lo.dep_absent = [false; 3];
                    let mut sp = spec;
                    match row {
                        Row::Control => {}
                        Row::Restart(t, how) => lo.restarts = vec![(t, how)],
                        Row::Login(t, d) => lo.logins = vec![(t, d)],
                        Row::Cold => {
                            lo.dep_absent = [true; 3];
                            sp = cold_spec;
                        }
                    }
                    sc.spawn(move || bench(sp, &lo).map(|(_, s)| ((r, row), s)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a station judge thread panicked")).collect()
        });
        for r in got {
            results.push(r?);
        }
    }
    let mut out = Vec::new();
    let mut total = 0usize;
    for &r in &o.rels {
        let mine: Vec<&((i64, Row), DropStats)> = results.iter().filter(|((rr, _), _)| *rr == r).collect();
        let fl = |row: &Row, s: &DropStats| -> Vec<&'static str> {
            let _ = row;
            fails(s, o.launch_min)
        };
        let control = mine.iter().find(|((_, row), _)| *row == Row::Control).map(|((_, row), s)| fl(row, s)).unwrap_or_default();
        let cold = mine.iter().find(|((_, row), _)| *row == Row::Cold).map(|((_, row), s)| fl(row, s));
        // (the lead's rows decide: the control, the cold start, the restarts; the logins are the pearl
        // alternative's, reported)
        let failing: Vec<&&((i64, Row), DropStats)> = mine.iter().filter(|((_, row), s)| matches!(row, Row::Cold | Row::Restart(..)) && !fl(row, s).is_empty()).collect();
        let login_failing = mine.iter().filter(|((_, row), s)| matches!(row, Row::Login(..)) && !fl(row, s).is_empty()).count();
        total += failing.len() + usize::from(!control.is_empty());
        let rest = mine.iter().filter(|((_, row), _)| !matches!(row, Row::Login(..))).map(|(_, s)| s.rest).min().unwrap_or(i64::MIN);
        let rest_l = mine.iter().filter(|((_, row), _)| matches!(row, Row::Login(..))).map(|(_, s)| s.rest).min();
        let lead = mine.iter().filter_map(|(_, s)| s.brake_lead).min();
        let gap = mine.iter().filter_map(|(_, s)| s.launch_gap).min();
        let gap_c = mine.iter().find(|((_, row), _)| *row == Row::Control).and_then(|(_, s)| s.launch_gap);
        let rest_c = mine.iter().find(|((_, row), _)| *row == Row::Control).map(|(_, s)| s.rest);
        let r_frozen: usize = mine.iter().filter(|(_, s)| s.r_frozen > 0).count();
        let shut: usize = mine.iter().filter(|(_, s)| s.shut_onto > 0).count();
        out.push(format!(
            "REL T+{r}: the lead's rows (the control, the cold start, {} restarts): failing {}; the control {}; the cold start {}; the least rest margin {} (unfrozen {}; under the logins {}); the least landing lead {}; the least launch spacing {} (unfrozen {}; reported, the bar {}); the pearl alternative's logins (reported, out of the lead's scope): failing {login_failing}, runs with a trapdoor shut onto its cart {shut}, runs with the R line switching while the entities were frozen {r_frozen}",
            mine.iter().filter(|((_, row), _)| matches!(row, Row::Restart(..))).count(),
            failing.len(),
            if control.is_empty() { "clean".to_string() } else { format!("FAILS ({})", control.join(", ")) },
            match cold {
                None => "-".to_string(),
                Some(c) if c.is_empty() => "clean".to_string(),
                Some(c) => format!("FAILS ({})", c.join(", ")),
            },
            if rest == i64::MIN { "FAIL".to_string() } else { format!("{rest} gt") },
            rest_c.map_or("-".into(), |x| if x == i64::MIN { "FAIL".to_string() } else { format!("{x} gt") }),
            rest_l.map_or("-".into(), |x| if x == i64::MIN { "FAIL".to_string() } else { format!("{x} gt") }),
            lead.map_or("-".into(), |l| format!("{l} gt")),
            gap.map_or("-".into(), |g| format!("{g} gt")),
            gap_c.map_or("-".into(), |g| format!("{g} gt")),
            o.launch_min
        ));
        // (the failing phases per row kind, and each kind's least launch spacing)
        let least_gap = |pred: &dyn Fn(&Row) -> bool| mine.iter().filter(|((_, row), _)| pred(row)).filter_map(|(_, s)| s.launch_gap).min().map_or("-".to_string(), |g| format!("{g} gt"));
        out.push(format!(
            "  the least launch spacing per row: the control {}, the cold start {}, restarts {}, {}",
            least_gap(&|r| *r == Row::Control),
            least_gap(&|r| *r == Row::Cold),
            least_gap(&|r| matches!(r, Row::Restart(..))),
            o.ds.iter().map(|d| format!("logins of {d} gt {}", least_gap(&|r| matches!(r, Row::Login(_, dd) if dd == d)))).collect::<Vec<_>>().join(", ")
        ));
        for how in restart_orders() {
            let mut ts: Vec<i64> = failing.iter().filter_map(|((_, row), _)| if let Row::Restart(t, h) = row { (*h == how).then_some(*t) } else { None }).collect();
            ts.sort();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<&str> = failing.iter().filter(|((_, row), _)| matches!(row, Row::Restart(_, h) if *h == how)).flat_map(|((_, row), s)| fl(row, s)).collect();
                out.push(format!("  restarts in the {} order: failing at T+{} ({})", how.label(), bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
        for &d in &o.ds {
            let bad: Vec<&&((i64, Row), DropStats)> = mine.iter().filter(|((_, row), s)| matches!(row, Row::Login(_, dd) if *dd == d) && !fl(row, s).is_empty()).collect();
            let mut ts: Vec<i64> = bad.iter().filter_map(|((_, row), _)| if let Row::Login(t, _) = row { Some(*t) } else { None }).collect();
            ts.sort();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<&str> = bad.iter().flat_map(|((_, row), s)| fl(row, s)).collect();
                out.push(format!("  logins of {d} gt (the pearl alternative, reported): failing at T+{} ({})", bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
    }
    Ok((out, total))
}
