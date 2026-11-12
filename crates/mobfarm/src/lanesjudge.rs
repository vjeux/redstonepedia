//! `mctas mobfarm lanes-judge` / `escape-judge` (cc767b78; farmopt 2026-11-11 22:03, the L1 rule; G1's
//! review 2026-11-12 00:12): the freeze-matrix judge of ironfarm's Q2 benches (the shared-release lanes,
//! the per-lane escapement into the drop cell). Under L1 (the farm kept loaded by a pearl: no ring, the
//! controller on game ticks) the perturbation besides real timing is the load window: a load runs block
//! events up to 3 gt ahead of the entities and at most 1 scheduled tick while they are frozen
//! (tick-exact's `mobai load-window`). Emulated as a real level-32 freeze of D gt (1..4: 3 gt of block
//! events and 1 scheduled tick; redsim's level 32 runs every scheduled tick of the window, the game at
//! most one: stricter for a part's own timers, the same for bench-driven gates) at every 1-gt start
//! over a period around an opening, at every corner, in each mode.
//!
//! The bars, each against the unfrozen control of the run's corner and mode: the "exactly 1" counts
//! (each count other than 1 beyond the control's), every `(the bar: 0)` line (gate and trapdoor shuts
//! onto a cart, carts hanging), the arrivals at the lanes' backs above the bench's limit (the
//! gravity-run exception: <= 0.46 all ridden), the cells not settled at a release, and the riders' eye
//! blocks (a block not in the control's set). Recovery retries are exempt by name (reported). Nothing
//! missing or dirty passes (G1 2026-11-12 00:12 and 02:26): each judge names the families its bench
//! must print (a control lacking one is an error, as is a run lacking a family its control carries),
//! and a mode whose control is not clean fails unless an `artefact: <label>: <why>` line of that
//! control (printed by the bench only when it detects the condition) names a label each piece of the
//! dirt starts with.

use std::collections::{BTreeMap, BTreeSet};

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
    /// Each bar-0 count by its label: every `... (the bar: 0): K` line, and the arrivals at the lanes'
    /// backs above the bench's limit (`arrivals above 0.46`).
    pub zero: BTreeMap<String, usize>,
    /// Cells not settled at a release.
    pub unsettled: usize,
    /// The recovery path's retries (exempt by name: a recovery, reported).
    pub retries: usize,
    /// The riders' eye blocks per place (`in D`, `on R`): the block names seen.
    pub eyes: BTreeMap<String, BTreeSet<String>>,
    /// The artefacts the bench names: `artefact: <label>: <why>` lines, (the label, why); each covers
    /// the control's dirt whose kind starts with its label.
    pub artefacts: Vec<(String, String)>,
    /// Every family the text carries (the exactly-1 families, the bar-0 lines, the arrivals, the
    /// unsettled count, the retries, the eye blocks): a run lacking one its control carries is an error.
    pub families: BTreeSet<String>,
}

impl Bars {
    /// What this run has beyond `control` (each count saturating at 0, an eye block not in the
    /// control's set): (the kind, how many more); empty when no worse.
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
        for (place, names) in &self.eyes {
            let known = control.eyes.get(place);
            for n in names {
                if !known.is_some_and(|k| k.contains(n)) {
                    v.push((format!("a rider's eye in {n} {place} (not in the control's set)"), 1));
                }
            }
        }
        v
    }

    /// A control's own bars (its eye blocks are the baseline, not a bar).
    fn own(&self) -> Vec<(String, usize)> {
        let base = Bars { eyes: self.eyes.clone(), ..Bars::default() };
        self.beyond(&base)
    }
}

fn first_num(s: &str) -> usize {
    s.trim_start().split(|c: char| !c.is_ascii_digit()).next().and_then(|x| x.parse::<usize>().ok()).unwrap_or(0)
}

/// The bars of a bench's text: its "exactly 1" families (`carts per lane per opening (exactly 1): 0
/// carts: 3, 1 carts: 87`; `carts per G0 opening (exactly 1, ...): 1: 60; per G_P opening: 1: 60`;
/// `carts per cell per release (exactly 1): 1: 60; cells not settled (string held) at a release: 0;
/// ...`; `carts per D per release (exactly 1): 1: 36; D not settled at a release: 0; R-line retries:
/// 0`), its `(the bar: 0): K` lines, `arrivals at the lanes' backs (...): N, M above X, ...`, `the
/// riders' eye blocks: in D a x3; on R b x2`, `artefact: ...`.
pub fn bars(text: &str) -> Bars {
    let mut b = Bars::default();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("artefact: ") {
            // (`artefact: <label>: <why>`; a line without its label covers nothing)
            if let Some((label, why)) = rest.rsplit_once(": ") {
                b.artefacts.push((label.trim().to_string(), why.trim().to_string()));
            }
            continue;
        }
        if let Some(i) = l.find(" (the bar: 0): ") {
            let label = l[..i].to_string();
            *b.zero.entry(label.clone()).or_default() += first_num(&l[i + 15..]);
            b.families.insert(label);
        }
        if l.starts_with("arrivals at the lanes' backs") {
            // (`...: N, M above X, the hardest H`)
            if let Some(i) = l.find(" above ") {
                let head = &l[..i];
                let m = head.rsplit(", ").next().map(first_num).unwrap_or(0);
                let lim: String = l[i + 7..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                let label = format!("arrivals at the lanes' backs above {lim}");
                *b.zero.entry(label.clone()).or_default() += m;
                b.families.insert(label);
            }
        }
        if let Some(rest) = l.strip_prefix("the riders' eye blocks: ") {
            for part in rest.split("; ") {
                let (place, list) = match part.split_once(' ') {
                    Some((a, r)) if a == "in" || a == "on" => match r.split_once(' ') {
                        Some((p, list)) => (format!("{a} {p}"), list),
                        None => continue,
                    },
                    _ => continue,
                };
                let names: BTreeSet<String> = list.split(", ").filter_map(|x| x.rsplit_once(" x").map(|(n, _)| n.trim().to_string())).filter(|n| !n.is_empty()).collect();
                b.eyes.entry(place).or_default().extend(names);
            }
            b.families.insert("the riders' eye blocks".to_string());
        }
        if !l.contains("(exactly 1") {
            continue;
        }
        for seg in l.split("; ") {
            if let Some(i) = seg.find("not settled") {
                if let Some(j) = seg[i..].find(": ") {
                    b.unsettled += first_num(&seg[i + j + 2..]);
                    b.families.insert("not settled at a release".to_string());
                }
                continue;
            }
            if let Some(i) = seg.find("retries: ") {
                b.retries += first_num(&seg[i + 9..]);
                b.families.insert("retries".to_string());
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
            b.families.insert(label.clone());
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
/// returns its text; `warm` and `period` place opening `k` (warm + corner + k x period). `expected`:
/// the families this judge's bench must print (G1 2026-11-12 02:26: a line both the control and the
/// runs drop would otherwise go unjudged): an error when a control lacks one, or a run lacks a family
/// its control carries. A control's own dirt fails the mode unless an `artefact: <label>: <why>` line
/// of that control names a label the dirt starts with (the bench prints it only when it detects the
/// condition); dirt no artefact covers fails.
pub fn judge<F>(title: &str, warm: i64, period: i64, expected: &[&str], o: &JudgeOpts, run: F) -> Result<Vec<String>, String>
where
    F: Fn(&Mode, i64, Vec<(i64, i64)>) -> Result<String, String> + Sync,
{
    let mut out = vec![format!(
        "{title} judge (cc767b78, bench, tool unreviewed: the L1 load window as a real level-32 freeze of D gt {:?} at every start {}..{} gt from opening {}, corners {:?}, each through liveness::freeze32, which asserts every part chunk reads level 32; a run fails when worse than its corner's unfrozen control; a mode fails when a control is not clean and the bench names no artefact)",
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
        let control = |c: i64| results.iter().find(|((cc, f), _)| *cc == c && f.is_none()).map(|(_, b)| b.clone());
        // (nothing missing: every control carries every family this judge expects, every run every
        // family its control carries)
        for ((c, f), b) in &results {
            let Some(ctl) = control(*c) else { return Err(format!("mode {}: corner {c:+} has no control", m.label)) };
            let absent: Vec<&&str> = expected.iter().filter(|e| !ctl.families.contains(**e)).collect();
            if !absent.is_empty() {
                return Err(format!("mode {}: corner {c:+}'s control lacks {:?}, which this judge expects (it carries {:?})", m.label, absent, ctl.families));
            }
            let missing: Vec<&String> = ctl.families.difference(&b.families).collect();
            if !missing.is_empty() {
                return Err(format!("mode {}: corner {c:+}, freeze {f:?}: the run's text lacks {:?}, which its control carries", m.label, missing));
            }
        }
        // (nothing dirty: a control's own bars fail the mode unless an artefact line names each)
        let mut dirty: Vec<String> = Vec::new();
        let controls: Vec<String> = o
            .corners
            .iter()
            .map(|&c| {
                let b = control(c).unwrap_or_default();
                let own = b.own();
                if own.is_empty() {
                    return format!("corner {c:+}: clean");
                }
                let mut parts: Vec<String> = Vec::new();
                let mut uncovered = false;
                for (k, n) in &own {
                    match b.artefacts.iter().find(|(label, _)| k.starts_with(label.as_str())) {
                        Some((label, why)) => parts.push(format!("{n} x {k} (the bench's artefact {label}: {why})")),
                        None => {
                            uncovered = true;
                            parts.push(format!("{n} x {k} (NO ARTEFACT)"));
                        }
                    }
                }
                if uncovered {
                    dirty.push(format!("corner {c:+}"));
                    format!("corner {c:+}: NOT CLEAN ({})", parts.join(", "))
                } else {
                    format!("corner {c:+}: {}", parts.join(", "))
                }
            })
            .collect();
        let mut fails: Vec<(i64, i64, i64, Vec<(String, usize)>)> = Vec::new();
        let mut retry_runs = 0usize;
        for ((c, f), b) in &results {
            if let Some((d, off)) = f {
                let ctl = control(*c).unwrap_or_default();
                let extra = b.beyond(&ctl);
                if !extra.is_empty() {
                    fails.push((*c, *d, *off, extra));
                }
                if b.retries > ctl.retries {
                    retry_runs += 1;
                }
            }
        }
        let n = results.iter().filter(|(k, _)| k.1.is_some()).count();
        out.push(format!(
            "mode {}: {}; {n} runs; failing {}; recovery retries beyond the control (exempt by name) in {retry_runs} runs; the unfrozen controls {}",
            m.label,
            if !dirty.is_empty() {
                format!("FAILS: the controls of {} are not clean and no artefact line covers it", dirty.join(", "))
            } else if fails.is_empty() {
                "PASSES".to_string()
            } else {
                "FAILS".to_string()
            },
            fails.len(),
            controls.join(", ")
        ));
        for &c in &o.corners {
            for &d in &o.ds {
                let mut offs: Vec<i64> = fails.iter().filter(|f| f.0 == c && f.1 == d).map(|f| f.2).collect();
                offs.sort();
                if !offs.is_empty() {
                    let kinds: BTreeSet<String> = fails.iter().filter(|f| f.0 == c && f.1 == d).flat_map(|f| f.3.iter().map(|(k, _)| k.clone())).collect();
                    out.push(format!("  corner {c:+}, D {d}: failing starts {} ({})", bands(&offs), kinds.into_iter().collect::<Vec<_>>().join(", ")));
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_family() {
        let t = "carts per D per release (exactly 1): 1: 34, 0: 2; D not settled at a release: 1; R-line retries: 3\n\
                 trapdoor shuts onto a cart (the bar: 0): 2 (t 5: x; y)\n\
                 carts hanging over the rail end's support column (the bar: 0): 0\n\
                 arrivals at the lanes' backs (returns): 40, 3 above 0.46, the hardest 0.501\n\
                 the riders' eye blocks: in D tripwire x30, air x6; on R air x36\n";
        let b = bars(t);
        assert_eq!(b.wrong.get(&("carts per D per release (exactly 1)".to_string(), 0)), Some(&2));
        assert_eq!(b.unsettled, 1);
        assert_eq!(b.retries, 3);
        assert_eq!(b.zero.get("trapdoor shuts onto a cart"), Some(&2));
        assert_eq!(b.zero.get("carts hanging over the rail end's support column"), Some(&0));
        assert_eq!(b.zero.get("arrivals at the lanes' backs above 0.46"), Some(&3));
        assert!(b.eyes.get("in D").is_some_and(|s| s.contains("tripwire") && s.contains("air")));
        assert!(b.eyes.get("on R").is_some_and(|s| s.contains("air")));
        assert!(b.families.contains("not settled at a release") && b.families.contains("retries") && b.families.contains("the riders' eye blocks"));
        // (a run worse than its control: one more arrival above the limit, a new eye block)
        let worse = bars(&t.replace("3 above 0.46", "4 above 0.46").replace("on R air x36", "on R air x35, glass x1"));
        let kinds: Vec<String> = worse.beyond(&b).into_iter().map(|(k, _)| k).collect();
        assert!(kinds.iter().any(|k| k.starts_with("arrivals")) && kinds.iter().any(|k| k.contains("glass")));
        // (a run lacking a family its control carries)
        let lacking = bars(&t.replace("arrivals at the lanes' backs (returns): 40, 3 above 0.46, the hardest 0.501\n", ""));
        assert!(b.families.difference(&lacking.families).count() == 1);
    }

    /// The lanes bench's 0.2 arrival bar bites (a positive control on its own line's format): a run
    /// with one more arrival above 0.2 than its control fails on that bar; at equal counts it passes.
    #[test]
    fn lanes_arrival_bar() {
        let ctl = "carts per lane per opening (exactly 1): 1 carts: 24\n\
                   gate shuts onto a cart (the bar: 0): 0\n\
                   arrivals at the lanes' backs (the intakes and the trains closing up): 30, 0 above 0.2, the hardest 0.180\n";
        let c = bars(ctl);
        for f in LANES_FAMILIES {
            assert!(c.families.contains(*f), "the lanes bench's family {f:?}");
        }
        let run = bars(&ctl.replace("30, 0 above 0.2, the hardest 0.180", "31, 1 above 0.2, the hardest 0.241"));
        assert_eq!(run.beyond(&c), vec![("arrivals at the lanes' backs above 0.2".to_string(), 1)]);
        assert!(bars(ctl).beyond(&c).is_empty());
    }

    /// An artefact line covers only the dirt whose kind starts with its label.
    #[test]
    fn artefact_labels() {
        let t = "carts per cell per release (exactly 1): 1: 57, 0: 3; cells not settled (string held) at a release: 0\n\
                 gate and trapdoor shuts onto a cart (the bar: 0): 1\n\
                 artefact: carts per cell per release (exactly 1): the last release is cut by the run's end\n";
        let b = bars(t);
        assert_eq!(b.artefacts, vec![("carts per cell per release (exactly 1)".to_string(), "the last release is cut by the run's end".to_string())]);
        let own = b.own();
        let covered: Vec<bool> = own.iter().map(|(k, _)| b.artefacts.iter().any(|(l, _)| k.starts_with(l.as_str()))).collect();
        assert_eq!(own.len(), 2);
        assert_eq!(covered.iter().filter(|x| **x).count(), 1, "the shut onto a cart is not covered: {own:?}");
    }
}

/// The families each judge's bench must print (G1 2026-11-12 02:26), as `bars()` names them.
pub const LANES_FAMILIES: &[&str] = &["carts per lane per opening (exactly 1)", "gate shuts onto a cart", "arrivals at the lanes' backs above 0.2"];
/// The escape bench's switched-R lane (`--r-pulse`).
pub const ESCAPE_SWITCHED_FAMILIES: &[&str] = &[
    "carts per D per release (exactly 1)",
    "not settled at a release",
    "retries",
    "trapdoor shuts onto a cart",
    "carts hanging over the rail end's support column",
    "arrivals at the lanes' backs above 0.46",
    "the riders' eye blocks",
];
/// The escape bench's G0/G_P lane.
pub const ESCAPE_GATES_FAMILIES: &[&str] = &[
    "carts per G0 opening (exactly 1, after the warm-up)",
    "per G_P opening",
    "carts per cell per release (exactly 1)",
    "not settled at a release",
    "gate and trapdoor shuts onto a cart",
    "arrivals at the lanes' backs above 0.46",
];
