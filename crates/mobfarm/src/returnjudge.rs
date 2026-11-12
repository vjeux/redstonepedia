//! `mctas mobfarm return-judge` (cc767b78; ironspec's L1‴ rows; the bars farmopt set 2026-11-12 03:22,
//! ironspec's amendments 03:23): the judge of 7b90c3f1's return (`return-bench`: the merge, the shared
//! lift and the split of F1 + Q2 at 24, in the L1 mode `--game-ticks`). The lead's rows: per corner (the
//! release offsets, the pocket release speed, the napper's delay) the control, and at the nominal
//! corners a save/stop/start at every 1-gt phase (the bench's `--restart`; its orders as the bench
//! offers them) — the cold start is the control itself (the return starts empty, its latches idle, and
//! nothing in it keys on world time); the pearl alternative's logins of D gt at every phase are reported
//! and marked unverified while the bench's `--login` is not `liveness::freeze32`. The bars: every cart
//! in its own lane (no WRONG LANE, no NEVER), seated, unhurt, its eye never in a wall; every cart's
//! branch delivery stored in `--branch-band` (0.30..0.48) and the napper's delivery to the bed stop's
//! entry in `--napper-band` (0.1..0.3), each with its margin to both ends; every shaft-entry gap at
//! least `--shaft-min` (21) live gt; the napper's window at least `--napper-behind` (28) behind the
//! batch's third cart and `--napper-ahead` (21) ahead of the next batch's first; each junction's turn
//! at least its latch's switching time + 4 gt ahead of the next cart at it (`--junction-min J:G`);
//! box-test pushes at most `--pushes-max` (0; touches reported); the return time per lane printed.
//! What the bench or the tools do not print yet (the riders' heads under the roof at 28; the static
//! gates on the emitted split file: would-pop, the rail-T scan with each T's verdict, the detector
//! slope's neighbours, parse(write())) is listed as PENDING, and a verdict with anything pending is
//! not final.

use crate::returnbench::{ReturnOpts, return_bench};
use std::collections::BTreeMap;

/// The bars.
#[derive(Clone, Debug)]
pub struct ReturnBars {
    pub branch_band: (f64, f64),
    pub napper_band: (f64, f64),
    pub shaft_min: i64,
    pub napper_behind: i64,
    pub napper_ahead: i64,
    /// Per junction (its x), the least gap (its latch's switching time + 4); empty: pending.
    pub junction_min: BTreeMap<i32, i64>,
    pub pushes_max: usize,
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
    /// The batches' carts' branch speeds (the napper's apart).
    pub branch: Vec<f64>,
    pub napper_entry: Option<f64>,
    /// Per junction (its x), the least gap from its turn to the next cart at it.
    pub junction: BTreeMap<i32, i64>,
    pub shaft: Option<i64>,
    pub napper_behind: Option<i64>,
    pub napper_ahead: Option<i64>,
    pub touches: usize,
    pub pushes: usize,
    /// Per lane, the longest return (release to its branch flat, live gt).
    pub ret: BTreeMap<String, i64>,
    /// Lines the judge needs and did not find (an error: nothing missing passes).
    pub missing: Vec<&'static str>,
}

fn num_after(l: &str, key: &str) -> Option<f64> {
    let i = l.find(key)? + key.len();
    let s: String = l[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    s.trim_end_matches('.').parse().ok()
}

/// Read a return bench's text.
pub fn read(text: &str, napper: bool) -> ReturnRead {
    let mut r = ReturnRead::default();
    let (mut saw_split, mut saw_pairs, mut saw_gaps) = (false, false, false);
    for l in text.lines() {
        let is_napper = l.starts_with("napper ");
        if (l.starts_with("cart ") || is_napper) && l.contains(": into the shaft") {
            r.carts += 1;
            if l.contains("(WRONG LANE)") {
                r.wrong += 1;
            }
            if l.contains("NEVER at a lane") {
                r.never += 1;
            }
            if l.contains("DISMOUNTED") {
                r.dismounted += 1;
            }
            if num_after(l, "; hurt ").is_some_and(|h| h > 0.0) {
                r.hurt += 1;
            }
            if num_after(l, "eye-in-wall ticks ").is_some_and(|e| e > 0.0) {
                r.eye += 1;
            }
            let v = num_after(l, "; branch speed ");
            if is_napper {
                r.napper_entry = v;
            } else if let Some(v) = v {
                r.branch.push(v);
            }
            // (its lane, as the line names it, and the return: release to its branch flat)
            if let (Some(lane), Some(ret)) = (l.split(": into the shaft").next().and_then(|h| h.rsplit_once(", ").map(|(_, x)| x.trim_end_matches(')').to_string())), num_after(l, "at its branch flat ")) {
                let e = r.ret.entry(lane).or_insert(0);
                *e = (*e).max(ret as i64);
            }
        }
        if l.starts_with("split: ") {
            saw_split = true;
            if let Some(i) = l.find("to the next cart at it: ") {
                let rest = l[i + 24..].split("; ").next().unwrap_or("");
                for p in rest.split(", ") {
                    // (`J(3) 16 gt`)
                    if let Some((j, g)) = p.strip_prefix("J(").and_then(|x| x.split_once(") "))
                        && let (Ok(j), Ok(g)) = (j.parse::<i32>(), g.trim_end_matches(" gt").trim().parse::<i64>())
                    {
                        r.junction.insert(j, g);
                    }
                }
            }
        }
        if l.starts_with("pairs: ") {
            saw_pairs = true;
            r.touches = num_after(l, "|dy| < 0.7) ").map_or(0, |x| x as usize);
            r.pushes = num_after(l, "against either yaw) ").map_or(0, |x| x as usize);
        }
        if let Some(rest) = l.strip_prefix("shaft entry gaps (live gt): ") {
            saw_gaps = true;
            let (gaps, nap) = match rest.split_once("; the napper ") {
                Some((g, n)) => (g, Some(n)),
                None => (rest, None),
            };
            r.shaft = gaps
                .split(", ")
                .filter_map(|p| p.splitn(3, ' ').nth(2))
                .flat_map(|g| g.split(" / ").filter_map(|n| n.trim().parse::<i64>().ok()).collect::<Vec<_>>())
                .min();
            if let Some(n) = nap {
                // (`X behind batch 0's third cart, Y ahead of batch 1's first`)
                r.napper_behind = n.split(" behind").next().and_then(|x| x.trim().parse().ok());
                r.napper_ahead = n.split(", ").nth(1).and_then(|x| x.split(" ahead").next()).and_then(|x| x.trim().parse().ok());
            }
        }
    }
    if r.carts == 0 {
        r.missing.push("the carts' lines");
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
    if napper && (r.napper_behind.is_none() || r.napper_ahead.is_none()) {
        r.missing.push("the napper's window");
    }
    r
}

/// A run's failures against the bars (empty: it passes).
pub fn fails(r: &ReturnRead, b: &ReturnBars, napper: bool) -> Vec<String> {
    let mut v = Vec::new();
    for (n, what) in [(r.wrong, "wrong lane(s)"), (r.never, "never at a lane"), (r.dismounted, "dismounted"), (r.hurt, "hurt"), (r.eye, "eye in a wall")] {
        if n > 0 {
            v.push(format!("{n} {what}"));
        }
    }
    if r.branch.is_empty() {
        v.push("no branch speed".into());
    }
    for x in &r.branch {
        if *x < b.branch_band.0 || *x > b.branch_band.1 {
            v.push(format!("a branch delivery {x:.3} outside {:.2}..{:.2}", b.branch_band.0, b.branch_band.1));
            break;
        }
    }
    if napper {
        if let Some(x) = r.napper_entry
            && (x < b.napper_band.0 || x > b.napper_band.1)
        {
            v.push(format!("the napper's delivery {x:.3} outside {:.2}..{:.2}", b.napper_band.0, b.napper_band.1));
        }
        if r.napper_behind.is_some_and(|g| g < b.napper_behind) {
            v.push(format!("the napper {} gt behind the third cart (bar {})", r.napper_behind.unwrap_or(0), b.napper_behind));
        }
        if r.napper_ahead.is_some_and(|g| g < b.napper_ahead) {
            v.push(format!("the napper {} gt ahead of the next batch (bar {})", r.napper_ahead.unwrap_or(0), b.napper_ahead));
        }
    }
    match r.shaft {
        Some(g) if g < b.shaft_min => v.push(format!("a shaft-entry gap of {g} gt (bar {})", b.shaft_min)),
        None => v.push("no shaft-entry gap".into()),
        _ => {}
    }
    for (j, g) in &r.junction {
        if let Some(m) = b.junction_min.get(j)
            && g < m
        {
            v.push(format!("J({j})'s gap {g} gt (bar {m})"));
        }
    }
    if r.pushes > b.pushes_max {
        v.push(format!("{} box-test pushes", r.pushes));
    }
    v
}

/// One run of the matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Control,
    Restart(i64),
    Login(i64, i64),
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
    /// The corners whose restarts and logins run at every phase.
    pub phase_corners: Vec<Corner>,
    pub starts: (i64, i64),
    pub restarts: bool,
    pub ds: Vec<i64>,
    pub bars: ReturnBars,
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

/// The judge. Returns (lines, failing runs in the lead's rows, the bars it could not judge).
pub fn judge(base: &ReturnOpts, o: &ReturnJudgeOpts) -> Result<(Vec<String>, usize, Vec<&'static str>), String> {
    let mut runs: Vec<(Corner, Row)> = Vec::new();
    for c in &o.corners {
        runs.push((c.clone(), Row::Control));
    }
    for c in &o.phase_corners {
        if !o.corners.contains(c) {
            runs.push((c.clone(), Row::Control));
        }
        for t in o.starts.0..=o.starts.1 {
            if o.restarts {
                runs.push((c.clone(), Row::Restart(t)));
            }
            for &d in &o.ds {
                runs.push((c.clone(), Row::Login(t, d)));
            }
        }
    }
    type Res = ((Corner, Row), ReturnRead);
    let mut results: Vec<Res> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<Res, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|(c, row)| {
                    let mut lo = base.clone();
                    lo.offsets = c.offsets.clone();
                    lo.speed = c.speed;
                    lo.napper_delay = c.napper;
                    lo.trace = None;
                    lo.out = None;
                    lo.restart = None;
                    lo.login = None;
                    match row {
                        Row::Control => {}
                        Row::Restart(t) => lo.restart = Some(*t),
                        Row::Login(t, d) => lo.login = Some((*t, *d)),
                    }
                    let (c, row) = (c.clone(), *row);
                    sc.spawn(move || return_bench(&lo).map(|t| {
                        let r = read(&t, c.napper.is_some());
                        ((c, row), r)
                    }))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a return judge thread panicked")).collect()
        });
        for r in got {
            results.push(r?);
        }
    }
    for ((c, row), r) in &results {
        if !r.missing.is_empty() {
            return Err(format!("{}, {row:?}: the bench's text lacks {:?}", c.label(), r.missing));
        }
    }
    // (a napper corner whose delivery the bench does not print: pending, never a pass)
    let napper_unprinted = results.iter().any(|((c, _), r)| c.napper.is_some() && r.napper_entry.is_none());
    let fl = |c: &Corner, r: &ReturnRead| fails(r, &o.bars, c.napper.is_some());
    let mut out = Vec::new();
    let mut total = 0usize;
    // (the controls at every corner: failures, the bands' margins, the napper's window, the returns)
    let controls: Vec<&Res> = results.iter().filter(|(k, _)| k.1 == Row::Control).collect();
    let ctl_fail: Vec<&&Res> = controls.iter().filter(|((c, _), r)| !fl(c, r).is_empty()).collect();
    total += ctl_fail.len();
    let all_branch: Vec<f64> = controls.iter().flat_map(|(_, r)| r.branch.clone()).collect();
    let all_nap: Vec<f64> = controls.iter().filter_map(|(_, r)| r.napper_entry).collect();
    let mut ret_max: BTreeMap<String, i64> = BTreeMap::new();
    for (_, r) in &controls {
        for (l, t) in &r.ret {
            let e = ret_max.entry(l.clone()).or_insert(0);
            *e = (*e).max(*t);
        }
    }
    out.push(format!(
        "the controls ({} corners; each the cold start): failing {}; branch delivery {}; the napper's delivery {}; the least shaft-entry gap {}; the napper's least window {} behind, {} ahead; the least junction gaps {}; box touches up to {}, pushes up to {}; the longest return per lane {}",
        controls.len(),
        ctl_fail.len(),
        margins(&all_branch, o.bars.branch_band),
        margins(&all_nap, o.bars.napper_band),
        controls.iter().filter_map(|(_, r)| r.shaft).min().map_or("-".into(), |g| format!("{g} gt")),
        controls.iter().filter_map(|(_, r)| r.napper_behind).min().map_or("-".into(), |g| format!("{g} gt")),
        controls.iter().filter_map(|(_, r)| r.napper_ahead).min().map_or("-".into(), |g| format!("{g} gt")),
        {
            let mut m: BTreeMap<i32, i64> = BTreeMap::new();
            for (_, r) in &controls {
                for (j, g) in &r.junction {
                    let e = m.entry(*j).or_insert(i64::MAX);
                    *e = (*e).min(*g);
                }
            }
            m.iter().map(|(j, g)| format!("J({j}) {g} gt{}", o.bars.junction_min.get(j).map_or(" (bar pending)".to_string(), |b| format!(" (bar {b})")))).collect::<Vec<_>>().join(", ")
        },
        controls.iter().map(|(_, r)| r.touches).max().unwrap_or(0),
        controls.iter().map(|(_, r)| r.pushes).max().unwrap_or(0),
        ret_max.iter().map(|(l, t)| format!("{l} {t} gt")).collect::<Vec<_>>().join(", ")
    ));
    // (the controls' failures grouped by kind, and the offsets whose every corner is clean)
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for ((c, _), r) in &ctl_fail {
        for k in fl(c, r).iter().map(|k| kind(k)).collect::<std::collections::BTreeSet<_>>() {
            *by_kind.entry(k).or_default() += 1;
        }
    }
    if !by_kind.is_empty() {
        out.push(format!("  the controls' failures by kind (corners): {}", by_kind.iter().map(|(k, n)| format!("{k}: {n}")).collect::<Vec<_>>().join("; ")));
    }
    let mut offs: Vec<Vec<i64>> = Vec::new();
    for ((c, _), _) in &controls {
        if !offs.contains(&c.offsets) {
            offs.push(c.offsets.clone());
        }
    }
    let clean: Vec<String> = offs.iter().filter(|o3| controls.iter().filter(|((c, _), _)| &c.offsets == *o3).all(|((c, _), r)| fl(c, r).is_empty())).map(|o3| format!("{o3:?}")).collect();
    out.push(format!("  offsets clean at every release speed and napper delay: {} of {}{}", clean.len(), offs.len(), if clean.is_empty() { String::new() } else { format!(" ({})", clean.join(" ")) }));
    // (the phase corners: the restarts decide, the logins reported)
    for c in &o.phase_corners {
        let mine: Vec<&Res> = results.iter().filter(|((cc, _), _)| cc == c).collect();
        let restarts: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Restart(_))).collect();
        let logins: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Login(..))).collect();
        let failing: Vec<&&&Res> = restarts.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).collect();
        total += failing.len();
        let login_failing = logins.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).count();
        let br: Vec<f64> = restarts.iter().flat_map(|(_, r)| r.branch.clone()).collect();
        out.push(format!(
            "{}: {} restarts at every phase: failing {}; branch delivery {}; the least shaft-entry gap {} | the pearl alternative's logins ({} runs; reported, UNVERIFIED: the bench's --login is not liveness::freeze32): failing {login_failing}",
            c.label(),
            restarts.len(),
            failing.len(),
            margins(&br, o.bars.branch_band),
            restarts.iter().filter_map(|(_, r)| r.shaft).min().map_or("-".into(), |g| format!("{g} gt")),
            logins.len()
        ));
        let mut ts: Vec<i64> = failing.iter().filter_map(|((_, row), _)| if let Row::Restart(t) = row { Some(*t) } else { None }).collect();
        ts.sort();
        if !ts.is_empty() {
            let kinds: std::collections::BTreeSet<String> = failing.iter().flat_map(|((cc, _), r)| fl(cc, r).into_iter().map(|k| kind(&k))).collect();
            out.push(format!("  restarts failing at t {} ({})", bands(&ts), kinds.into_iter().collect::<Vec<_>>().join(", ")));
        }
    }
    let mut pending: Vec<&'static str> = Vec::new();
    if napper_unprinted {
        pending.push("the napper's delivery to the bed stop's entry (0.1..0.3; the bench prints its branch speed as '-')");
    }
    Ok((out, total, pending))
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
    pub branch: (f64, f64),
    pub pushes: usize,
    /// Wrong lanes, never at a lane, dismounted, hurt, eye in a wall (summed over the corners).
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
        for (j, g) in &self.junction {
            if let Some(m) = b.junction_min.get(j) {
                v.push((format!("J({j})"), Some(g - m)));
            }
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

    /// The pass/fail bars clean: no lost cart, no push, every branch delivery in its band.
    pub fn clean(&self, b: &ReturnBars) -> bool {
        self.lost == 0 && self.pushes <= b.pushes_max && self.branch.0 >= b.branch_band.0 && self.branch.1 <= b.branch_band.1
    }

    pub fn meets(&self, b: &ReturnBars, margin: i64) -> bool {
        self.clean(b) && self.least_margin(b).is_some_and(|m| m >= margin)
    }

    fn line(&self, b: &ReturnBars) -> String {
        let gm: Vec<String> = self.gt_margins(b).iter().map(|(k, m)| format!("{k} {}", m.map_or("-".to_string(), |x| format!("{x:+}")))).collect();
        let jn: Vec<String> = self.junction.iter().map(|(j, g)| format!("J({j}) {g} gt{}", if b.junction_min.contains_key(j) { "" } else { " (bar pending)" })).collect();
        format!(
            "s {} D {}: margins {}; the least values: shaft {}, behind {}, ahead {}; junctions {}; branch delivery {:.3}..{:.3} (margins {:+.3} / {:+.3}); pushes {}; lost carts {}",
            self.step,
            self.d,
            gm.join(", "),
            self.shaft.map_or("-".into(), |x| format!("{x} gt")),
            self.behind.map_or("-".into(), |x| format!("{x} gt")),
            self.ahead.map_or("-".into(), |x| format!("{x} gt")),
            jn.join(", "),
            self.branch.0,
            self.branch.1,
            self.branch.0 - b.branch_band.0,
            b.branch_band.1 - self.branch.1,
            self.pushes,
            self.lost
        )
    }
}

/// The solve: for each launch step and napper delay, the controls at every corner (the departure
/// spread on each launch x the release speeds); per step its best delay (the largest least gt margin,
/// the smallest delay on a tie) with its margins and every delay that meets the bars; then the
/// smallest step that meets every bar with `margin` gt to spare.
pub fn solve(base: &ReturnOpts, o: &SolveOpts) -> Result<Vec<String>, String> {
    let mut deltas = vec![o.spread.0, 0, o.spread.1];
    deltas.sort();
    deltas.dedup();
    let mut runs: Vec<(i64, i64, Vec<i64>, f64)> = Vec::new();
    for &s in &o.steps {
        for &d in &o.ds {
            for &da in &deltas {
                for &db in &deltas {
                    for &dc in &deltas {
                        for &sp in &o.speeds {
                            runs.push((s, d, vec![da, s + db, 2 * s + dc], sp));
                        }
                    }
                }
            }
        }
    }
    type Res = ((i64, i64), ReturnRead);
    let mut results: Vec<Res> = Vec::new();
    for chunk in runs.chunks(o.jobs.max(1)) {
        let got: Vec<Result<Res, String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk
                .iter()
                .map(|(s, d, offs, sp)| {
                    let mut lo = base.clone();
                    lo.offsets = offs.clone();
                    lo.speed = *sp;
                    lo.napper_delay = Some(*d);
                    lo.trace = None;
                    lo.out = None;
                    lo.restart = None;
                    lo.login = None;
                    let (s, d) = (*s, *d);
                    sc.spawn(move || return_bench(&lo).map(|t| ((s, d), read(&t, true))))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("a solve thread panicked")).collect()
        });
        for r in got {
            let r = r?;
            if !r.1.missing.is_empty() {
                return Err(format!("s {} D {}: the bench's text lacks {:?}", r.0.0, r.0.1, r.1.missing));
            }
            results.push(r);
        }
    }
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
            let br: Vec<f64> = mine.iter().flat_map(|r| r.branch.clone()).collect();
            rows.push(SolveRow {
                step: s,
                d,
                corners: mine.len(),
                shaft: mine.iter().filter_map(|r| r.shaft).min(),
                behind: mine.iter().filter_map(|r| r.napper_behind).min(),
                ahead: mine.iter().filter_map(|r| r.napper_ahead).min(),
                junction,
                branch: (br.iter().copied().fold(f64::INFINITY, f64::min), br.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
                pushes: mine.iter().map(|r| r.pushes).max().unwrap_or(0),
                lost: mine.iter().map(|r| r.wrong + r.never + r.dismounted + r.hurt + r.eye).sum(),
            });
        }
    }
    let mut out = vec![format!(
        "the solve (cc767b78, bench, tool unreviewed): launch steps {:?} (offsets 0, s, 2s), napper D {}..{}, release speeds {:?}, the departure spread {}..{} gt on each launch ({} corners a row); a row meets the bars when every gt bar keeps {} gt of margin beyond the corners and the pass/fail bars are clean",
        o.steps,
        o.ds.first().copied().unwrap_or(0),
        o.ds.last().copied().unwrap_or(0),
        o.speeds,
        o.spread.0,
        o.spread.1,
        deltas.len().pow(3) * o.speeds.len(),
        o.margin
    )];
    let mut first_ok: Option<&SolveRow> = None;
    for &s in &o.steps {
        let mine: Vec<&SolveRow> = rows.iter().filter(|r| r.step == s).collect();
        let ok: Vec<i64> = mine.iter().filter(|r| r.meets(&o.bars, o.margin)).map(|r| r.d).collect();
        // (the best delay: clean first, then the largest least margin, then the smallest delay)
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
        None => format!("NO step in {:?} meets every bar with {} gt to spare (ironspec: the napper's bars re-derived from their mechanism)", o.steps, o.margin),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars() -> ReturnBars {
        ReturnBars { branch_band: (0.30, 0.48), napper_band: (0.1, 0.3), shaft_min: 21, napper_behind: 28, napper_ahead: 21, junction_min: [(3, 16), (-1, 16)].into_iter().collect(), pushes_max: 0 }
    }

    /// The reader on the bench's own lines: a clean run passes; a wrong lane, a gap under the bar, a
    /// branch delivery out of its band, a short napper window and a push each fail; a text without the
    /// split line is missing it.
    #[test]
    fn reads_the_bench() {
        let t = "cart 0 (batch 0, lane -1): into the shaft 30 live gt after release; lane -1, at its branch flat 132 live gt after release; branch speed 0.410; air 300; seated; hurt 0.0; eye-in-wall ticks 0\n\
                 cart 1 (batch 0, lane 3): into the shaft 31 live gt after release; lane 3, at its branch flat 153 live gt after release; branch speed 0.420; air 300; seated; hurt 0.0; eye-in-wall ticks 0\n\
                 napper 3 (batch 0, the 4th route): into the shaft 80 live gt after release; the 4th route, at its branch flat 160 live gt after release; branch speed 0.200; air 300; seated; hurt 0.0; eye-in-wall ticks 0\n\
                 split: 0 wrong lane(s) of 3; the fastest branch speed 0.420 (ironfarm's bar 0.30, margin -0.120); the least gap from a junction's turn to the next cart at it: J(3) 16 gt, J(-1) 18 gt; the shaft's closest pair 3.100\n\
                 pairs: box touches (the game's push test: inflated 0.2 sideways, |dy| < 0.7) 6 pair-ticks, the hardest closing 0.081 (t 120, carts 0 and 1 at 1.00,2.00,3.00); where a push acts (|cos| >= 0.8 against either yaw) 0\n\
                 shaft entry gaps (live gt): batch 0 21 / 22, batch 1 21 / 21; the napper 32 behind batch 0's third cart, 26 ahead of batch 1's first\n";
        let r = read(t, true);
        assert!(r.missing.is_empty(), "{:?}", r.missing);
        assert_eq!((r.carts, r.wrong, r.touches, r.pushes, r.shaft), (3, 0, 6, 0, Some(21)));
        assert_eq!(r.junction, [(3, 16), (-1, 18)].into_iter().collect());
        assert_eq!((r.napper_behind, r.napper_ahead, r.napper_entry), (Some(32), Some(26), Some(0.2)));
        assert_eq!(r.branch, vec![0.41, 0.42]);
        assert_eq!(r.ret.get("lane 3"), Some(&153));
        assert!(fails(&r, &bars(), true).is_empty(), "{:?}", fails(&r, &bars(), true));
        let bad = read(
            &t.replace("; lane 3,", "; lane 7 (WRONG LANE),")
                .replace("batch 1 21 / 21", "batch 1 21 / 19")
                .replace("against either yaw) 0", "against either yaw) 2")
                .replace("branch speed 0.410", "branch speed 0.290")
                .replace("the napper 32 behind", "the napper 27 behind"),
            true,
        );
        let f = fails(&bad, &bars(), true);
        for want in ["wrong lane", "shaft-entry gap of 19", "pushes", "branch delivery 0.290", "27 gt behind"] {
            assert!(f.iter().any(|x| x.contains(want)), "{want:?} in {f:?}");
        }
        assert!(read(&t.replace("split: ", "splat: "), true).missing.contains(&"the split line"));
        assert!(read(&t.replace("; the napper 32 behind batch 0's third cart, 26 ahead of batch 1's first", ""), true).missing.contains(&"the napper's window"));
        // (the napper's delivery unprinted ("-"): no failure, the judge lists it pending)
        let unprinted = read(&t.replace("branch speed 0.200", "branch speed -"), true);
        assert!(unprinted.missing.is_empty() && unprinted.napper_entry.is_none());
    }
}
