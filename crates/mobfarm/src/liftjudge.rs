//! `mctas mobfarm lift-judge` (cc767b78; farmopt 2026-11-11 23:42, the L1 rule): the freeze-matrix
//! judge of 7b90c3f1's lift bench (the shared bubble lift with Q2's merge3 / split3). The L1 load
//! window (a load runs block events up to 3 gt ahead of the entities and at most 1 scheduled tick
//! while they are frozen) is emulated as the bench's own real level-32 freeze (`--login T,D`: its
//! chunks at level 32, the spare chunk west still forced, the launches waiting it out) of D gt (1..4)
//! at every 1-gt start, at every corner (the launch spacing x the entry speed). The pass (farmopt's
//! line): every `cart k:` line without NEVER or DISMOUNTED, `hurt 0.0`, `eye-in-block ticks 0`; the
//! `split:` line without WRONG LANE; the fastest branch speed <= 0.30; the hardest contact <= 0.2.

use crate::liftbench::{LiftOpts, lift_bench};

/// The bars' limits.
const BRANCH_MAX: f64 = 0.30;
const CONTACT_MAX: f64 = 0.2;

/// One run's verdict: the failures and the two measured margins.
#[derive(Clone, Debug, Default)]
pub struct LiftVerdict {
    pub fails: Vec<String>,
    pub branch: Option<f64>,
    pub contact: Option<f64>,
}

fn num_after(l: &str, key: &str) -> Option<f64> {
    let i = l.find(key)? + key.len();
    let s: String = l[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    s.parse().ok()
}

/// The verdict of a lift bench's text.
pub fn verdict(text: &str) -> LiftVerdict {
    let mut v = LiftVerdict::default();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("cart ")
            && rest.contains(": met the column")
        {
            let k = rest.split(':').next().unwrap_or("?");
            if l.contains("NEVER") {
                v.fails.push(format!("cart {k} never on the exit rail"));
            }
            if l.contains("DISMOUNTED") {
                v.fails.push(format!("cart {k} dismounted"));
            }
            if num_after(l, "hurt ").is_some_and(|h| h > 0.0) {
                v.fails.push(format!("cart {k} hurt"));
            }
            if num_after(l, "eye-in-block ticks ").is_some_and(|e| e > 0.0) {
                v.fails.push(format!("cart {k} eye in a block"));
            }
        }
        if l.starts_with("split:") && l.contains("WRONG LANE") {
            v.fails.push("a wrong lane".to_string());
        }
        if l.starts_with("branch speeds")
            && let Some(b) = num_after(l, "the fastest ")
        {
            v.branch = Some(b);
            if b > BRANCH_MAX {
                v.fails.push(format!("branch speed {b:.3}"));
            }
        }
        if l.starts_with("pairs:")
            && let Some(c) = num_after(l, "the hardest contact (centres within 1.0) ")
        {
            v.contact = Some(c);
            if c > CONTACT_MAX {
                v.fails.push(format!("contact {c:.3}"));
            }
        }
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
pub struct LiftJudgeOpts {
    pub spacings: Vec<i64>,
    pub speeds: Vec<f64>,
    pub starts: (i64, i64),
    pub ds: Vec<i64>,
    pub jobs: usize,
}

/// The judge over one base: per corner (spacing x speed) its control, its failing starts per D, the
/// worst branch speed and contact; returns (lines, failing runs).
pub fn judge(base: &LiftOpts, o: &LiftJudgeOpts) -> Result<(Vec<String>, usize), String> {
    let mut runs: Vec<(i64, f64, Option<(i64, i64)>)> = Vec::new();
    for &s in &o.spacings {
        for &v in &o.speeds {
            runs.push((s, v, None));
            for &d in &o.ds {
                for t in o.starts.0..=o.starts.1 {
                    runs.push((s, v, Some((t, d))));
                }
            }
        }
    }
    let mut results: Vec<((i64, f64, Option<(i64, i64)>), LiftVerdict)> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<((i64, f64, Option<(i64, i64)>), LiftVerdict), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|&(s, v, f)| {
                    let mut lo = base.clone();
                    lo.spacing = s;
                    lo.speed = v;
                    lo.freeze = f;
                    lo.trace = None;
                    sc.spawn(move || lift_bench(&lo).map(|t| ((s, v, f), verdict(&t))))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a lift judge thread panicked")).collect()
        });
        for r in got {
            results.push(r?);
        }
    }
    let mut out = Vec::new();
    let mut total_fail = 0usize;
    for &s in &o.spacings {
        for &v in &o.speeds {
            let mine: Vec<&((i64, f64, Option<(i64, i64)>), LiftVerdict)> = results.iter().filter(|((ss, vv, _), _)| *ss == s && *vv == v).collect();
            let control = mine.iter().find(|((_, _, f), _)| f.is_none()).map(|(_, r)| r.clone()).unwrap_or_default();
            let frozen: Vec<&&((i64, f64, Option<(i64, i64)>), LiftVerdict)> = mine.iter().filter(|((_, _, f), _)| f.is_some()).collect();
            let failing: Vec<&&&((i64, f64, Option<(i64, i64)>), LiftVerdict)> = frozen.iter().filter(|(_, r)| !r.fails.is_empty()).collect();
            total_fail += failing.len() + usize::from(!control.fails.is_empty());
            let bmax = mine.iter().filter_map(|(_, r)| r.branch).fold(f64::NEG_INFINITY, f64::max);
            let cmax = mine.iter().filter_map(|(_, r)| r.contact).fold(f64::NEG_INFINITY, f64::max);
            out.push(format!(
                "spacing {s}, speed {v}: {} runs; failing {}; the control {}; the fastest branch speed over the corner {} (bar {BRANCH_MAX}); the hardest contact {} (bar {CONTACT_MAX})",
                frozen.len(),
                failing.len(),
                if control.fails.is_empty() { "clean".to_string() } else { format!("FAILS ({})", control.fails.join(", ")) },
                if bmax.is_finite() { format!("{bmax:.3}") } else { "-".into() },
                if cmax.is_finite() { format!("{cmax:.3}") } else { "-".into() }
            ));
            for &d in &o.ds {
                let mut ts: Vec<i64> = failing.iter().filter_map(|((_, _, f), _)| f.filter(|(_, dd)| *dd == d).map(|(t, _)| t)).collect();
                ts.sort();
                if !ts.is_empty() {
                    let kinds: std::collections::BTreeSet<String> = failing
                        .iter()
                        .filter(|((_, _, f), _)| f.is_some_and(|(_, dd)| dd == d))
                        .flat_map(|(_, r)| r.fails.iter().map(|x| x.split_whitespace().filter(|w| !w.chars().all(|c| c.is_ascii_digit() || c == '.')).collect::<Vec<_>>().join(" ")))
                        .collect();
                    out.push(format!("  D {d}: failing starts {} ({})", bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
                }
            }
        }
    }
    Ok((out, total_fail))
}
