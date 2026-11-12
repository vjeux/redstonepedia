//! The return judge (cc767b78; ironspec's L1‴ rows, farmopt 2026-11-12 03:22 with ironspec's
//! amendments 03:23, the measured bars 05:31): the judge of 7b90c3f1's return (`return-bench`: the
//! merge, the shared lift and the in-chunk split of F1 + Q2 at 24, `--row9`, in the L1 mode
//! `--game-ticks`). The lead's rows: per corner (the launch offsets with the measured departure spread,
//! the pocket release speed, the napper's delay) the control — the cold start: the return starts empty,
//! its latches idle, and nothing in it keys on world time, so every bar rides the batch's own launches
//! (REL) — and at the nominal corners a save/stop/start at every 1-gt phase in each of
//! `liveness::Restart::rows()`'s 10 orders (the bench's `--restart`); the pearl alternative's logins of
//! D gt at every phase (the bench's `--login`, through `liveness::freeze32`) are reported apart. The
//! bars: every cart in its own lane (no WRONG LANE, no NEVER), seated, unhurt, its eye never in a wall,
//! its rider's head under the roof's underside (`--head-max`, 28.0); every batch cart's delivery (stored,
//! entering its lane's merge cell: ironfarm's drop entry) in `--branch-band` (0.30..0.48) and the
//! napper's (stored, handed on by the 4th route toward the bed stop) in `--napper-band` (0.1..0.3), each
//! with its margin to both ends; every shaft-entry gap at least `--shaft-min`; the napper's window at
//! least `--napper-behind` behind the batch's third cart and `--napper-ahead` ahead of the next batch's
//! first; each junction's turn at least `--junction-min J:G` ahead of the next cart at it; box-test
//! pushes at most `--pushes-max` (0). Every gt bar keeps `--margin` (2) gt at the nominal corners. The
//! static gates on the emitted split file (the bench's `--out`): its own (parse(write()), every T's
//! verdict and sources, the detectors' Ts), and here `redsim::rails::would_pop` over every rail of the
//! file and every detector's face neighbours (the block under it among them) with their connection
//! counts. A value the reader cannot read is an error (nothing silently dropped).

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
    /// The batch carts' stored speed entering their branch (printed, not a bar).
    pub branch: Vec<f64>,
    /// The batch carts' stored speed entering their lane's merge cell (the delivery).
    pub merge: Vec<f64>,
    /// The napper's hand-on speed by the 4th route (stored).
    pub napper_entry: Option<f64>,
    /// The riders' highest head y.
    pub head: Option<f64>,
    /// Per junction (its x), the least gap from its turn to the next cart at it.
    pub junction: BTreeMap<i32, i64>,
    pub shaft: Option<i64>,
    pub napper_behind: Option<i64>,
    pub napper_ahead: Option<i64>,
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

/// Read a return bench's text (the `--row9` bench's lines).
pub fn read(text: &str, napper: bool) -> ReturnRead {
    let mut r = ReturnRead::default();
    let (mut saw_split, mut saw_pairs, mut saw_gaps, mut saw_head) = (false, false, false, false);
    for l in text.lines() {
        let is_napper = l.starts_with("napper ");
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
                // (`branch speed - (handed on by the 4th route at M stored, X/gt)`)
                match num_after(l, "handed on by the 4th route at ") {
                    Some(m) => r.napper_entry = Some(m),
                    None if arrived => r.errors.push(format!("{head}...: on its route without its hand-on speed")),
                    None => {}
                }
            } else {
                // (`branch speed V (entering the merge cell M)`)
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
                // (`X behind batch 0's third cart, Y ahead of batch 1's first`; `-` unmeasured)
                r.napper_behind = n.split(" behind").next().and_then(|x| x.trim().parse().ok());
                r.napper_ahead = n.split(", ").nth(1).and_then(|x| x.split(" ahead").next()).and_then(|x| x.trim().parse().ok());
            }
        }
    }
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
    if r.merge.is_empty() {
        v.push("no merge-cell delivery".into());
    }
    if let Some(x) = r.merge.iter().find(|x| **x < b.branch_band.0 || **x > b.branch_band.1) {
        v.push(format!("a delivery {x:.3} outside {:.2}..{:.2}", b.branch_band.0, b.branch_band.1));
    }
    if napper {
        match r.napper_entry {
            Some(x) if x < b.napper_band.0 || x > b.napper_band.1 => v.push(format!("the napper's delivery {x:.3} outside {:.2}..{:.2}", b.napper_band.0, b.napper_band.1)),
            None => v.push("no napper delivery".into()),
            _ => {}
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
    for (j, m) in &b.junction_min {
        match r.junction.get(j) {
            Some(g) if g < m => v.push(format!("J({j})'s gap {g} gt (bar {m})")),
            None => v.push(format!("J({j})'s gap unmeasured")),
            _ => {}
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
fn gt_margins(r: &ReturnRead, b: &ReturnBars, napper: bool) -> Vec<(String, Option<i64>)> {
    let mut v = vec![("shaft-entry gap".to_string(), r.shaft.map(|x| x - b.shaft_min))];
    if napper {
        v.push(("napper behind".to_string(), r.napper_behind.map(|x| x - b.napper_behind)));
        v.push(("napper ahead".to_string(), r.napper_ahead.map(|x| x - b.napper_ahead)));
    }
    for (j, m) in &b.junction_min {
        v.push((format!("J({j})"), r.junction.get(j).map(|g| g - m)));
    }
    v
}

/// One run of the matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Control,
    Restart(i64, Restart),
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
    /// The nominal corners: their controls' margins are held to `margin`; their restarts (every phase
    /// in `starts`, the 10 orders) and logins run.
    pub phase_corners: Vec<Corner>,
    pub starts: (i64, i64),
    pub restarts: bool,
    pub ds: Vec<i64>,
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
/// unreadable value is an error.
fn run_all<K: Clone + Send + Sync + std::fmt::Debug>(base: &ReturnOpts, runs: &[(K, ReturnOpts, bool)], jobs: usize) -> Result<Vec<(K, ReturnRead)>, String> {
    let mut results = Vec::new();
    for chunk in runs.chunks(jobs.max(1)) {
        let got: Vec<Result<(K, ReturnRead), String>> = std::thread::scope(|sc| {
            let hs: Vec<_> = chunk.iter().map(|(k, lo, nap)| sc.spawn(move || return_bench(lo).map(|t| (k.clone(), read(&t, *nap))))).collect();
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
    let _ = base;
    Ok(results)
}

/// The bench options for one corner and row.
fn opts(base: &ReturnOpts, c: &Corner, row: Row) -> ReturnOpts {
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
        Row::Restart(t, how) => lo.restart = Some((t, how)),
        Row::Login(t, d) => lo.login = Some((t, d)),
    }
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
    }
    let results = run_all(base, &runs, o.jobs)?;
    type Res = ((Corner, Row), ReturnRead);
    let fl = |c: &Corner, r: &ReturnRead| fails(r, &o.bars, c.napper.is_some());
    let mut out = Vec::new();
    let mut total = 0usize;
    // (the controls at every corner: failures, the bands' margins, the napper's window, the returns)
    let controls: Vec<&Res> = results.iter().filter(|(k, _)| k.1 == Row::Control).collect();
    let ctl_reads: Vec<&ReturnRead> = controls.iter().map(|(_, r)| r).collect();
    let ctl_fail: Vec<&&Res> = controls.iter().filter(|((c, _), r)| !fl(c, r).is_empty()).collect();
    total += ctl_fail.len();
    let all_merge: Vec<f64> = ctl_reads.iter().flat_map(|r| r.merge.clone()).collect();
    let all_branch: Vec<f64> = ctl_reads.iter().flat_map(|r| r.branch.clone()).collect();
    let all_nap: Vec<f64> = ctl_reads.iter().filter_map(|r| r.napper_entry).collect();
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
        "the controls ({} corners; each the cold start): failing {}; delivery (merge-cell entry, stored) {}; branch entry (printed) {}; the napper's delivery (its hand-on) {}; the highest head {:.3} (the roof's underside {:.1}, margin {:+.3}); the least shaft-entry gap {}; the napper's least window {} behind, {} ahead; the least junction gaps {}; box touches up to {}, pushes up to {}; the longest return per lane {}",
        controls.len(),
        ctl_fail.len(),
        margins(&all_merge, o.bars.branch_band),
        if all_branch.is_empty() { "-".to_string() } else { format!("{:.3}..{:.3}", all_branch.iter().copied().fold(f64::INFINITY, f64::min), all_branch.iter().copied().fold(f64::NEG_INFINITY, f64::max)) },
        margins(&all_nap, o.bars.napper_band),
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
        let short: Vec<&(String, Option<i64>)> = gm.iter().filter(|(_, m)| m.is_none_or(|m| m < o.margin)).collect();
        total += usize::from(!short.is_empty());
        out.push(format!(
            "nominal {}: the margins {} (required {} each){}",
            c.label(),
            gm.iter().map(|(k, m)| format!("{k} {}", m.map_or("-".to_string(), |x| format!("{x:+}")))).collect::<Vec<_>>().join(", "),
            o.margin,
            if short.is_empty() { String::new() } else { format!(" -> SHORT: {}", short.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ")) }
        ));
    }
    // (the nominal corners' restarts (the lead) and logins (reported))
    for c in &o.phase_corners {
        let mine: Vec<&Res> = results.iter().filter(|((cc, _), _)| cc == c).collect();
        let restarts: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Restart(..))).collect();
        let logins: Vec<&&Res> = mine.iter().filter(|((_, row), _)| matches!(row, Row::Login(..))).collect();
        let failing: Vec<&&&Res> = restarts.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).collect();
        total += failing.len();
        let rr: Vec<&ReturnRead> = restarts.iter().map(|(_, r)| r).collect();
        let lr: Vec<&ReturnRead> = logins.iter().map(|(_, r)| r).collect();
        let login_failing = logins.iter().filter(|((cc, _), r)| !fl(cc, r).is_empty()).count();
        let mg: Vec<f64> = rr.iter().flat_map(|r| r.merge.clone()).collect();
        let nap: Vec<f64> = rr.iter().filter_map(|r| r.napper_entry).collect();
        out.push(format!(
            "{}: {} restarts (every phase t {}..{} x {} orders): failing {}; delivery {}; the napper's delivery {}; the highest head {:.3}; the least shaft-entry gap {}; the napper's least window {} behind, {} ahead | the pearl alternative's logins ({} runs, through liveness::freeze32; reported, out of the lead's scope): failing {login_failing}; the least shaft-entry gap {}",
            c.label(),
            restarts.len(),
            o.starts.0,
            o.starts.1,
            orders.len(),
            failing.len(),
            margins(&mg, o.bars.branch_band),
            margins(&nap, o.bars.napper_band),
            rr.iter().filter_map(|r| r.head).fold(f64::NEG_INFINITY, f64::max),
            least(&rr, |r| r.shaft, o.bars.shaft_min),
            least(&rr, |r| r.napper_behind, o.bars.napper_behind),
            least(&rr, |r| r.napper_ahead, o.bars.napper_ahead),
            logins.len(),
            least(&lr, |r| r.shaft, o.bars.shaft_min),
        ));
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
    use redsim::block::Block;
    use redsim::pos::{BlockPos, Dir};
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
    w.place_raw(&cells);
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
        "static gates on the emitted split file ({path}): parse(write()) {} cells; would-pop {} of {} rails{}{}; {} T(s) (the bench's verdicts: {})",
        d.blocks.len(),
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
    // (every detector: its face neighbours, the block under it among them; a rail's potential
    // connections; a rail with 3 beside a detector is a T the detector's press re-settles)
    for p in &rails {
        let s = w.get(*p);
        if !s.to_string().contains("detector_rail") {
            continue;
        }
        let mut faces: Vec<String> = Vec::new();
        for dd in Dir::ALL {
            let q = p.rel(dd);
            let b = w.get(q);
            let name = b.to_string();
            let short = name.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
            if redsim::rails::is_rail(&b) {
                let n = redsim::rails::RailState::new(q, b).count_potential_connections(&w);
                let tee = redsim::rails::junction_at(&w, q).is_some();
                faces.push(format!("{dd:?} {short} {} ({n} connections{})", cell(q), if tee { ", a T" } else { "" }));
            } else {
                faces.push(format!("{dd:?} {short}"));
            }
        }
        let own = redsim::rails::RailState::new(*p, s.clone()).count_potential_connections(&w);
        out.push(format!("  detector {} {} ({own} connections): {}", cell(*p), redsim::rails::shape_of(&s).name(), faces.join("; ")));
    }
    Ok((out, fails))
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
            && self.napper.0 >= b.napper_band.0
            && self.napper.1 <= b.napper_band.1
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
            band(self.napper, b.napper_band),
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
            let nap: Vec<f64> = mine.iter().filter_map(|r| r.napper_entry).collect();
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
                lost: mine.iter().map(|r| r.wrong + r.never + r.dismounted + r.hurt + r.eye).sum(),
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
            "| s | D | shaft-entry gap, bar {} | napper behind, bar {} | napper ahead, bar {} | {} | delivery (merge-cell entry) | napper delivery (hand-on) | highest head | pushes | lost carts | meets (every gt margin >= {}) |",
            o.bars.shaft_min,
            o.bars.napper_behind,
            o.bars.napper_ahead,
            jh.join(" | "),
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

    fn bars() -> ReturnBars {
        ReturnBars { branch_band: (0.30, 0.48), napper_band: (0.1, 0.3), shaft_min: 14, napper_behind: 21, napper_ahead: 16, junction_min: [(3, 8), (-1, 8)].into_iter().collect(), pushes_max: 0, head_max: 28.0 }
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
        assert_eq!(gt_margins(&r, &bars(), true).iter().map(|(_, m)| m.unwrap()).collect::<Vec<_>>(), vec![9, 9, 8, 10, 9]);
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
        assert!(read(&T.replace("; the napper 30 behind batch 0's third cart, 24 ahead of batch 1's first", ""), true).missing.contains(&"the napper's window"));
        // (unreadable values: errors)
        for (from, to) in [("(entering the merge cell 0.360)", ""), ("handed on by the 4th route at 0.261", "handed on by the 4th route at x"), ("J(3) 17 gt", "J(3) - gt"), ("|dy| < 0.7) 6", "|dy| < 0.7) six"), ("head y 26.500", "head y -"), ("batch 1 23 / 24", "batch 1 23 / x")] {
            let e = read(&T.replace(from, to), true);
            assert!(!e.errors.is_empty(), "{from:?} -> {to:?} reads as an error");
        }
        // (a cart off its lane needs no delivery: the lane failure stands)
        let off = read(&T.replace("; lane 3, at its branch flat 153 live gt after release; branch speed 0.420 (entering the merge cell 0.360)", "; NEVER at a lane (ends at 1.00,2.00,3.00); branch speed -"), true);
        assert!(off.errors.is_empty(), "{:?}", off.errors);
        assert!(fails(&off, &bars(), true).iter().any(|x| x.contains("never at a lane")));
    }
}
