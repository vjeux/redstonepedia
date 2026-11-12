//! `mctas mobfarm lanes-judge` / `drop-judge` (cc767b78; farmopt 2026-11-11 22:03, the L1 rule): the
//! freeze-matrix judge of ironfarm's Q2 benches (the shared-release lanes, the per-lane escapement into
//! the drop cell). Under L1 (the farm kept loaded by a pearl: no ring, the controller on game ticks)
//! the perturbation besides real timing is the load window: a load runs block events up to 3 gt ahead
//! of the entities and at most 1 scheduled tick while they are frozen (tick-exact's `mobai
//! load-window`). Emulated as a real level-32 freeze of D gt (1..4: 3 gt of block events and 1
//! scheduled tick; redsim's level 32 runs every scheduled tick of the window, the game at most one:
//! stricter for a part's own timers, the same for bench-driven gates) at every 1-gt start over a
//! period around an opening, at every corner, in each mode. A run fails when it is worse than the
//! unfrozen control of its corner and mode on any bar: the "exactly 1" counts (each count other than 1
//! beyond the control's: a control's own artefacts, such as the last opening cut by the run's end at
//! a negative corner, cancel out), the shuts onto a cart (bar 0) and the cells not settled at a
//! release.

use std::collections::BTreeMap;

/// One mode: its label, the ring's Q (0: game ticks) and the width (0: the bench's own).
#[derive(Clone, Debug)]
pub struct Mode {
    pub label: String,
    pub ring_q: i64,
    pub width: i64,
}

impl Mode {
    /// `gt` (the bench's widths), `gt:W` (game-tick width W) or `ringQ:W` (ring-edge width W, the edges
    /// Q gt apart; 34: 3 and 4 alternately).
    pub fn parse(s: &str) -> Result<Mode, String> {
        if s == "gt" {
            return Ok(Mode { label: s.to_string(), ring_q: 0, width: 0 });
        }
        let (k, w) = s.split_once(':').ok_or(format!("--modes {s}: gt, gt:W or ringQ:W"))?;
        let width = w.trim().parse::<i64>().map_err(|e| format!("--modes {s}: {e}"))?;
        let ring_q = if k == "gt" { 0 } else { k.strip_prefix("ring").ok_or(format!("--modes {s}: gt, gt:W or ringQ:W"))?.parse::<i64>().map_err(|e| format!("--modes {s}: {e}"))? };
        Ok(Mode { label: s.to_string(), ring_q, width })
    }
}

/// The matrix.
#[derive(Clone, Debug)]
pub struct JudgeOpts {
    pub modes: Vec<Mode>,
    /// Freeze starts, gt relative to opening `k` (every 1 gt from .0 to .1).
    pub starts: (i64, i64),
    pub k: i64,
    pub ds: Vec<i64>,
    pub corners: Vec<i64>,
    pub jobs: usize,
}

/// A run's bars read from a bench's text.
#[derive(Clone, Debug, Default)]
pub struct Bars {
    /// (the family's label, a count other than 1) -> how many.
    pub wrong: BTreeMap<(String, usize), usize>,
    /// Each `... (the bar: 0): K` line's label -> K (gate and trapdoor shuts onto a cart, carts
    /// hanging, ...).
    pub zero: BTreeMap<String, usize>,
    /// Cells not settled at a release.
    pub unsettled: usize,
    /// The recovery path's retries (reported, not a bar: the bench's own login matrix leaves them
    /// out of its pass).
    pub retries: usize,
}

impl Bars {
    /// What this run has beyond `control` (each count saturating at 0): (the kind, how many more);
    /// empty when no worse.
    fn beyond(&self, control: &Bars) -> Vec<(String, usize)> {
        let mut v = Vec::new();
        for (k, n) in &self.wrong {
            let c = control.wrong.get(k).copied().unwrap_or(0);
            if *n > c {
                v.push((format!("{} {} carts", k.0, k.1), n - c));
            }
        }
        for (k, n) in &self.zero {
            let c = control.zero.get(k).copied().unwrap_or(0);
            if *n > c {
                v.push((k.clone(), n - c));
            }
        }
        if self.unsettled > control.unsettled {
            v.push(("cells not settled at a release".to_string(), self.unsettled - control.unsettled));
        }
        v
    }
}

fn first_num(s: &str) -> usize {
    s.split(|c: char| !c.is_ascii_digit()).next().and_then(|x| x.parse::<usize>().ok()).unwrap_or(0)
}

/// The bars of a bench's text: its "exactly 1" families (`carts per lane per opening (exactly 1): 0
/// carts: 3, 1 carts: 87`; `carts per G0 opening (exactly 1, ...): 1: 60; per G_P opening: 1: 60`;
/// `carts per cell per release (exactly 1): 1: 60; cells not settled (string held) at a release: 0;
/// ...`; `carts per D per release (exactly 1): 1: 36; D not settled at a release: 0; R-line retries:
/// 0`), its `(the bar: 0): K` lines.
pub fn bars(text: &str) -> Bars {
    let mut b = Bars::default();
    for l in text.lines() {
        if let Some(i) = l.find(" (the bar: 0): ") {
            *b.zero.entry(l[..i].to_string()).or_default() += first_num(&l[i + 15..]);
            continue;
        }
        if !l.contains("(exactly 1") {
            continue;
        }
        for seg in l.split("; ") {
            if let Some(i) = seg.find("not settled") {
                if let Some(j) = seg[i..].find(": ") {
                    b.unsettled += first_num(&seg[i + j + 2..]);
                }
                continue;
            }
            if let Some(i) = seg.find("retries: ") {
                b.retries += first_num(&seg[i + 9..]);
                continue;
            }
            // (the label: up to "): " for the family's first segment, up to ": " for a "per ..." one)
            let (label, pairs) = if let Some(i) = seg.find("): ") {
                (seg[..i + 1].to_string(), &seg[i + 3..])
            } else if seg.starts_with("per ")
                && let Some(i) = seg.find(": ")
            {
                (seg[..i].to_string(), &seg[i + 2..])
            } else {
                continue;
            };
            for p in pairs.split(", ") {
                let Some((k, n)) = p.split_once(": ") else { continue };
                let k = k.trim_end_matches(" carts").trim();
                if let (Ok(k), Ok(n)) = (k.parse::<usize>(), n.trim().parse::<usize>())
                    && k != 1
                {
                    *b.wrong.entry((label.clone(), k)).or_default() += n;
                }
            }
        }
    }
    b
}

/// Ranges of sorted offsets ("a..b, c").
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

/// The judge: `run(mode, corner, freezes)` runs the bench (its freezes at absolute bench ticks) and
/// returns its text; `warm` and `period` place opening `k` (warm + corner + k x period).
pub fn judge<F>(title: &str, warm: i64, period: i64, o: &JudgeOpts, run: F) -> Result<Vec<String>, String>
where
    F: Fn(&Mode, i64, Vec<(i64, i64)>) -> Result<String, String> + Sync,
{
    let mut out = vec![format!(
        "{title} judge (cc767b78, bench: the L1 load window as a real level-32 freeze of D gt {:?} at every start {}..{} gt from opening {}, corners {:?}; a run fails when worse than its corner's unfrozen control)",
        o.ds, o.starts.0, o.starts.1, o.k, o.corners
    )];
    for m in &o.modes {
        let mut runs: Vec<(i64, Option<(i64, i64)>)> = Vec::new();
        for &c in &o.corners {
            runs.push((c, None));
            for &d in &o.ds {
                for off in o.starts.0..=o.starts.1 {
                    runs.push((c, Some((d, off))));
                }
            }
        }
        let mut results: Vec<((i64, Option<(i64, i64)>), Bars)> = Vec::new();
        for chunk in runs.chunks(o.jobs.max(1)) {
            let got: Vec<Result<((i64, Option<(i64, i64)>), Bars), String>> = std::thread::scope(|sc| {
                let hs: Vec<_> = chunk
                    .iter()
                    .map(|&(c, f)| {
                        let fr = f.map(|(d, off)| vec![(warm + c + o.k * period + off, d)]).unwrap_or_default();
                        let run = &run;
                        sc.spawn(move || run(m, c, fr).map(|t| ((c, f), bars(&t))))
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().expect("a judge thread panicked")).collect()
            });
            for r in got {
                results.push(r?);
            }
        }
        let control = |c: i64| results.iter().find(|((cc, f), _)| *cc == c && f.is_none()).map(|(_, b)| b.clone()).unwrap_or_default();
        let controls: Vec<String> = o
            .corners
            .iter()
            .map(|&c| {
                let b = control(c);
                let own: Vec<String> = b.beyond(&Bars::default()).iter().map(|(k, n)| format!("{n} x {k}")).collect();
                format!("corner {c:+}: {}", if own.is_empty() { "clean".to_string() } else { own.join(", ") })
            })
            .collect();
        let mut fails: Vec<(i64, i64, i64, Vec<(String, usize)>)> = Vec::new();
        for ((c, f), b) in &results {
            if let Some((d, off)) = f {
                let extra = b.beyond(&control(*c));
                if !extra.is_empty() {
                    fails.push((*c, *d, *off, extra));
                }
            }
        }
        let n = results.iter().filter(|(k, _)| k.1.is_some()).count();
        let retry_runs: Vec<(i64, i64, i64)> = results
            .iter()
            .filter_map(|((c, f), b)| f.filter(|_| b.retries > control(*c).retries).map(|(d, off)| (*c, d, off)))
            .collect();
        out.push(format!(
            "mode {}: {n} runs; failing {}; runs with recovery retries beyond the control (not a bar) {}{}; the unfrozen controls (their own bars) {}",
            m.label,
            fails.len(),
            retry_runs.len(),
            if retry_runs.is_empty() { String::new() } else { format!(" (e.g. {})", retry_runs.iter().take(4).map(|(c, d, off)| format!("corner {c:+} D {d} start {off}")).collect::<Vec<_>>().join(", ")) },
            controls.join(", ")
        ));
        for &c in &o.corners {
            for &d in &o.ds {
                let mut offs: Vec<i64> = fails.iter().filter(|f| f.0 == c && f.1 == d).map(|f| f.2).collect();
                offs.sort();
                if !offs.is_empty() {
                    let kinds: std::collections::BTreeSet<String> = fails.iter().filter(|f| f.0 == c && f.1 == d).flat_map(|f| f.3.iter().map(|(k, _)| k.clone())).collect();
                    out.push(format!("  corner {c:+}, D {d}: failing starts {} ({})", bands(&offs), kinds.into_iter().collect::<Vec<_>>().join(", ")));
                }
            }
        }
    }
    Ok(out)
}
