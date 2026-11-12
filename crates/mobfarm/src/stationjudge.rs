//! `mctas mobfarm station-judge` (cc767b78; farmopt 2026-11-12 00:53, ironspec's L1‴ rows): the
//! judge of dfb89499's F1 + Q2 station (`drop-bench --layout station ... --l1`). Under L1‴ the farm
//! never freezes in normal operation and a restart loads its chunks before its first tick, so the rows
//! are: the L1 mode (the controller and its timers on game ticks), a save/stop/start at every phase
//! (the bench's `--restart T,ORDER`: the shared helper's `liveness::restart` in each of
//! `liveness::Restart::rows()`, the save order, its reverse and 8 seeded orders), and the cold start
//! (the fresh design with the departing carts absent: `--dep-riders x,x,x`); for the pearl alternative
//! also the load window (the bench's login of D gt at every phase, the controller running through it),
//! reported apart. A run passes when every cart landed on a braked pocket at least the L1 edge bar
//! (4 gt) before its re-brake and is at rest at least the rest bar (`--rest-bar`; the edge bar until
//! dfb89499's panic measure, ≥ 1 caller at every T over all 400 sensor-phase pairs, lands: ironspec
//! 2026-11-12 05:26 retired the 20-gt bar) before the next summon, with 0 wrong pockets, contacts,
//! riders hurt, unseated ticks, departing carts stuck, refill
//! failures and trapdoors shut onto their cart (and, with `launch_min`, every launch that far after the
//! one before). Printed per REL over its seeds: the lead's rows' failing phases per order, their least
//! rest margin, landing lead and launch spacing; the logins' failing phases and margins apart, and the
//! R line's switches while the entities were frozen; the timed edges' thresholds: the same matrix at
//! other widths of one edge each.

use crate::drop::{DropOpts, DropStats, bench};
use redsim::entity::mob::farm::FarmSpec;

/// The L1 edge bar (farmopt 2026-11-12 02:39, ironspec): every cart landed at least this long before
/// its pocket's re-brake (3 gt + 1 scheduled tick).
pub const EDGE_BAR: i64 = 4;

/// The rest margin's bar before the next summon until dfb89499's panic measure lands (ironspec
/// 2026-11-12 05:26: the stand's summon bar is the game's rule, ≥ 1 caller at every T over all 400
/// sensor-phase pairs; the 20-gt rest bar and the all-three rule are retired): the L1 edge bar, the
/// arrival at rest being a timed edge against the summon tick. The judge reports the summon bar as
/// pending.
pub const REST_BAR: i64 = EDGE_BAR;

/// A run's failures (empty: it passes): the bars, with the landing lead held to `edge` gt (the L1
/// edge bar, [`EDGE_BAR`]) and the rest margin before the next summon to `rest_bar` gt ([`REST_BAR`]).
pub fn fails(s: &DropStats, launch_min: i64, edge: i64, rest_bar: i64) -> Vec<&'static str> {
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
    } else if s.rest < rest_bar {
        v.push("at rest within the rest bar of the next summon");
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

impl Row {
    /// The lead's rows (they decide): the control, the cold start, the restarts.
    fn lead(&self) -> bool {
        !matches!(self, Row::Login(..))
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
    /// The rest margin's bar before the next summon (gt; [`REST_BAR`]).
    pub rest_bar: i64,
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
                    sc.spawn(move || bench(sp, &lo).map(|(_, s)| ((r, sd, row), s)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a station judge thread panicked")).collect()
        });
        for r in got {
            results.push(r?);
        }
    }
    let fl = |s: &DropStats| -> Vec<&'static str> { fails(s, o.launch_min, o.edge, o.rest_bar) };
    let gt = |v: Option<i64>| v.map_or("-".to_string(), |x| if x == i64::MIN { "FAIL".to_string() } else { format!("{x} gt") });
    let mut out = Vec::new();
    let mut total = 0usize;
    for &r in &o.rels {
        let mine: Vec<&Res> = results.iter().filter(|((rr, _, _), _)| *rr == r).collect();
        let lead: Vec<&&Res> = mine.iter().filter(|((_, _, row), _)| row.lead()).collect();
        let logins: Vec<&&Res> = mine.iter().filter(|((_, _, row), _)| !row.lead()).collect();
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
        let gap = lead.iter().filter_map(|(_, s)| s.launch_gap).min();
        let gap_c = mine.iter().filter(|((_, _, row), _)| *row == Row::Control).filter_map(|(_, s)| s.launch_gap).min();
        let rest_l = logins.iter().map(|(_, s)| s.rest).min();
        let lead_l = logins.iter().filter_map(|(_, s)| s.brake_lead).min();
        let gap_l = logins.iter().filter_map(|(_, s)| s.launch_gap).min();
        let r_frozen = logins.iter().filter(|(_, s)| s.r_frozen > 0).count();
        let shut = logins.iter().filter(|(_, s)| s.shut_onto > 0).count();
        let n_restarts = lead.iter().filter(|((_, _, row), _)| matches!(row, Row::Restart(..))).count();
        out.push(format!(
            "REL T+{r} ({} seed(s)): the lead's rows (the control, the cold start, {n_restarts} restarts): failing {}; the control {}; the cold start {}; the least rest margin {} (unfrozen {}; the bar {} gt); the least landing lead {} (the bar {} gt); the least launch spacing {} (unfrozen {}; reported, the bar {}) | the pearl alternative's logins ({} runs; reported, out of the lead's scope): failing {login_failing}; the least rest margin {}, landing lead {}, launch spacing {}; runs with a trapdoor shut onto its cart {shut}; runs with the R line switching while the entities were frozen {r_frozen}",
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
            o.rest_bar,
            gt(lead_min),
            o.edge,
            gt(gap),
            gt(gap_c),
            if o.launch_min > 0 { format!("{} gt", o.launch_min) } else { "none".to_string() },
            logins.len(),
            gt(rest_l),
            gt(lead_l),
            gt(gap_l),
        ));
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

    /// The bars bite (positive controls): a run landed 2 gt before its re-brake fails at the edge bar
    /// of 4; a run at rest 19 gt before the summon fails at a rest bar of 20 and passes at 19 (farmopt
    /// 2026-11-12 05:19: the rest is held to `--rest-bar`, its own bar); a run at rest 3 gt before it
    /// fails at the default rest bar (the edge bar, until the panic measure lands); the measured
    /// margins pass.
    #[test]
    fn edge_bar() {
        let ok = DropStats { rest: 34, brake_lead: Some(23), fall: 30, ..Default::default() };
        assert!(fails(&ok, 0, EDGE_BAR, REST_BAR).is_empty());
        let rest19 = DropStats { rest: 19, ..ok.clone() };
        assert_eq!(fails(&rest19, 0, EDGE_BAR, 20), vec!["at rest within the rest bar of the next summon"]);
        assert!(fails(&rest19, 0, EDGE_BAR, 19).is_empty());
        assert!(fails(&rest19, 0, EDGE_BAR, REST_BAR).is_empty());
        let near_summon = DropStats { rest: 3, ..ok.clone() };
        assert_eq!(fails(&near_summon, 0, EDGE_BAR, REST_BAR), vec!["at rest within the rest bar of the next summon"]);
        assert!(fails(&near_summon, 0, EDGE_BAR, 0).is_empty());
        let near_rebrake = DropStats { brake_lead: Some(2), ..ok.clone() };
        assert_eq!(fails(&near_rebrake, 0, EDGE_BAR, REST_BAR), vec!["landed within the edge bar of its pocket's re-brake"]);
        assert!(fails(&near_rebrake, 0, 2, REST_BAR).is_empty());
        let powered = DropStats { brake_lead: Some(-1), ..ok.clone() };
        assert_eq!(fails(&powered, 0, EDGE_BAR, REST_BAR), vec!["landed on a powered pocket"]);
        let late = DropStats { rest: -2, ..ok.clone() };
        assert_eq!(fails(&late, 0, EDGE_BAR, REST_BAR), vec!["at rest after the next summon"]);
        assert!(Row::Restart(5, crate::liveness::Restart::rows()[0]).lead() && Row::Cold.lead() && !Row::Login(5, 2).lead());
    }
}
