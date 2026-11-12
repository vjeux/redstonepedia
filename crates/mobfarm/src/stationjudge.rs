//! `mctas mobfarm station-judge` (cc767b78; farmopt 2026-11-11 23:54, the L1 rule): the freeze-matrix
//! judge of dfb89499's F1 + Q2 station (`drop-bench --layout station`). The L1 load window (a load runs
//! block events up to 3 gt ahead of the entities and at most 1 scheduled tick while they are frozen)
//! is emulated as the bench's own login (`--login T,D`: a real level-32 freeze, its live clock pausing)
//! of D gt (1..4) at every 1-gt start (bench ticks from T, the summon), at every REL corner. A run
//! passes when every cart landed, each landed on a braked pocket (lead >= 0), every arriving cart is at
//! rest before the next summon (margin >= 0), with 0 wrong pockets, 0 contacts, 0 riders hurt, 0
//! unseated ticks, 0 departing carts stuck and 0 refill failures. The timed edges' thresholds: the same
//! matrix at other widths (the trapdoor pulse, the re-brake, the R line's delay and length).

use crate::drop::{DropOpts, DropStats, bench};
use redsim::entity::mob::farm::FarmSpec;

/// A run's failures (empty: it passes).
pub fn fails(s: &DropStats) -> Vec<&'static str> {
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
    v
}

/// Ranges of sorted starts ("a..b, c").
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

/// The matrix.
#[derive(Clone, Debug)]
pub struct StationJudgeOpts {
    pub rels: Vec<i64>,
    pub starts: (i64, i64),
    pub ds: Vec<i64>,
    pub jobs: usize,
}

/// The judge over one base (its rel and logins replaced): per REL its control, failing starts per D;
/// the least rest margin and brake lead over the matrix. Returns (lines, failing runs).
pub fn judge(fspec: &FarmSpec, base: &DropOpts, o: &StationJudgeOpts) -> Result<(Vec<String>, usize), String> {
    let mut runs: Vec<(i64, Option<(i64, i64)>)> = Vec::new();
    for &r in &o.rels {
        runs.push((r, None));
        for &d in &o.ds {
            for t in o.starts.0..=o.starts.1 {
                runs.push((r, Some((t, d))));
            }
        }
    }
    let mut results: Vec<((i64, Option<(i64, i64)>), DropStats)> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<((i64, Option<(i64, i64)>), DropStats), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&(r, f)| {
                    let mut lo = base.clone();
                    lo.rel = r;
                    lo.logins = f.map(|x| vec![x]).unwrap_or_default();
                    lo.trace = false;
                    sc.spawn(move || bench(fspec, &lo).map(|(_, s)| ((r, f), s)))
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
        let mine: Vec<&((i64, Option<(i64, i64)>), DropStats)> = results.iter().filter(|((rr, _), _)| *rr == r).collect();
        let control = mine.iter().find(|((_, f), _)| f.is_none()).map(|(_, s)| fails(s)).unwrap_or_default();
        let failing: Vec<&&((i64, Option<(i64, i64)>), DropStats)> = mine.iter().filter(|((_, f), s)| f.is_some() && !fails(s).is_empty()).collect();
        total += failing.len() + usize::from(!control.is_empty());
        let rest = mine.iter().map(|(_, s)| s.rest).min().unwrap_or(i64::MIN);
        let lead = mine.iter().filter_map(|(_, s)| s.brake_lead).min();
        let rest_c = mine.iter().find(|((_, f), _)| f.is_none()).map(|(_, s)| s.rest);
        out.push(format!(
            "REL T+{r}: {} runs; failing {}; the control {}; the least rest margin before the next summon {} (unfrozen {}); the least landing lead over its pocket's re-brake {}",
            mine.len() - 1,
            failing.len(),
            if control.is_empty() { "clean".to_string() } else { format!("FAILS ({})", control.join(", ")) },
            if rest == i64::MIN { "FAIL".to_string() } else { format!("{rest} gt") },
            rest_c.map_or("-".into(), |x| if x == i64::MIN { "FAIL".to_string() } else { format!("{x} gt") }),
            lead.map_or("-".into(), |l| format!("{l} gt"))
        ));
        for &d in &o.ds {
            let mut ts: Vec<i64> = failing.iter().filter_map(|((_, f), _)| f.filter(|(_, dd)| *dd == d).map(|(t, _)| t)).collect();
            ts.sort();
            if !ts.is_empty() {
                let kinds: std::collections::BTreeSet<&str> = failing.iter().filter(|((_, f), _)| f.is_some_and(|(_, dd)| dd == d)).flat_map(|(_, s)| fails(s)).collect();
                out.push(format!("  D {d}: failing login starts T+{} ({})", bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
            }
        }
    }
    Ok((out, total))
}
