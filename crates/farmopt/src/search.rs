//! The search (designs/briefs/farm-optimizer.md, "The engine"): a spec in, a merged Pareto front
//! per fork out. Levels: static (every candidate, in parallel), then best-first by the optimistic
//! vector: quick runs in batches, the full judge on the most promising measured candidates, and
//! after each full judge every candidate whose optimistic vector is weakly dominated by a
//! full-judged candidate's pessimistic vector is pruned (it can never be strictly better). Local
//! search then adds the front members' neighbours and repeats. Every evaluation goes through the
//! journal, so a rerun only evaluates what is new.

use crate::journal::{Entry, Journal};
use crate::pareto::{self, Pick};
use crate::spec::Spec;
use crate::{Evaluation, Farm, Level, Metrics, Value};
use anyhow::Result;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// How a search runs.
#[derive(Clone, Debug)]
pub struct Opts {
    /// The journal's file.
    pub journal: PathBuf,
    pub threads: usize,
    /// Quick evaluations at most (all forks), full judges at most per fork.
    pub quick_budget: usize,
    pub full_budget: usize,
    /// Candidates per quick batch, per full batch.
    pub quick_batch: usize,
    pub full_batch: usize,
    /// Rounds of local search around the front.
    pub local_rounds: usize,
    /// Stop after the quick level (no full judge: a preview of the fronts).
    pub no_full: bool,
    /// Print progress lines.
    pub verbose: bool,
    /// Journal keys containing any of these are forgotten first (evaluated again): a part of the
    /// evaluator changed for those candidates only.
    pub forget: Vec<String>,
    /// Prune nothing: every feasible candidate is measured (quick, then full within the budget),
    /// a part front's members kept whether or not one reference farm dominates them (ironspec
    /// iteration 20: part fronts per interface).
    pub exhaustive: bool,
}

/// Why a candidate left the search, or where it stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Fate {
    /// Still in the search (not pruned, not finished): undecided when the search ends.
    Open,
    /// Refused by the static stage, or failed a level (a must-have).
    Invalid(String),
    /// A constraint fails (its text).
    Infeasible(String),
    /// Its optimistic vector is weakly dominated by this full-judged candidate's pessimistic one.
    Pruned { by: String, level: Level },
    /// Judged in full and passed.
    Judged,
}

/// One candidate and everything known about it.
#[derive(Clone, Debug)]
pub struct Cand<C> {
    pub c: C,
    pub key: String,
    pub fork: String,
    pub statics: Metrics,
    /// The text lines of every level reached (a card's facts).
    pub lines: BTreeMap<String, String>,
    pub bound: Metrics,
    pub quick: Option<Metrics>,
    pub full: Option<Metrics>,
    pub fate: Fate,
    /// Found by local search (a neighbour of a front member).
    pub local: bool,
    /// The bar it must beat (`Farm::bar_of`), oriented on the spec's objectives.
    pub bar: Option<Vec<f64>>,
}

impl<C> Cand<C> {
    /// The furthest level reached.
    pub fn level(&self) -> Level {
        if self.full.is_some() {
            Level::Full
        } else if self.quick.is_some() {
            Level::Quick
        } else if matches!(self.fate, Fate::Invalid(_)) && self.statics.is_empty() {
            Level::Invalid
        } else {
            Level::Static
        }
    }

    /// The metrics of its furthest level over its static ones (the full judge's, the quick runs',
    /// or the static ones alone).
    pub fn metrics(&self) -> Metrics {
        let mut m = self.statics.clone();
        if let Some(q) = self.full.as_ref().or(self.quick.as_ref()) {
            m.extend(q.iter().map(|(k, v)| (k.clone(), *v)));
        }
        m
    }

    /// What it might reach at best: the quick runs' optimistic ends over the bound where they exist
    /// (both are valid optimistic values; the tighter is kept per metric), exact static values as
    /// they are.
    pub fn optimistic(&self) -> Metrics {
        let mut m = self.statics.clone();
        m.extend(self.bound.iter().map(|(k, v)| (k.clone(), *v)));
        if let Some(q) = self.full.as_ref().or(self.quick.as_ref()) {
            for (k, v) in q {
                let e = m.entry(k.clone()).or_insert(*v);
                *e = Value { v: v.v, lo: e.lo.max(v.lo).min(v.hi), hi: e.hi.min(v.hi).max(v.lo) };
            }
        }
        m
    }
}

/// A finished search.
pub struct Outcome<C> {
    pub cands: Vec<Cand<C>>,
    pub forks: Vec<ForkFront>,
    pub quick_runs: usize,
    pub full_runs: usize,
    pub seconds: f64,
}

/// One fork's answer.
#[derive(Clone, Debug)]
pub struct ForkFront {
    pub fork: String,
    /// Indices into `Outcome::cands`.
    pub members: Vec<usize>,
    /// The merged front: (kept member, the members folded under it).
    pub merged: Vec<(usize, Vec<usize>)>,
    /// Candidates still open when the search stopped (their bounds count in the gap).
    pub open: Vec<usize>,
    /// Per objective: the best optimistic value of any valid feasible candidate of the fork (the
    /// ideal point), and the best measured mean on the front.
    pub ideal: Vec<f64>,
    /// Per objective: the candidate whose optimistic value is the bound (the gap's cause).
    pub ideal_by: Vec<Option<usize>>,
    pub best: Vec<f64>,
}

fn log(o: &Opts, s: &str) {
    if o.verbose {
        eprintln!("farmopt: {s}");
    }
}

/// The fork label of a candidate: its values of the spec's fork parameters.
fn fork_of<F: Farm>(farm: &F, c: &F::Cand, spec: &Spec) -> String {
    if spec.forks.is_empty() {
        return "all".into();
    }
    spec.forks.iter().map(|f| format!("{f}={}", farm.param(c, f).unwrap_or_else(|| "-".into()))).collect::<Vec<_>>().join(" ")
}

/// The first failing constraint among the metrics `m` holds, judged on `pick`'s end.
fn failing(spec: &Spec, m: &Metrics, exact_only: bool) -> Option<String> {
    for c in &spec.constraints {
        let Some(v) = m.get(&c.metric) else { continue };
        if exact_only && !v.is_exact() {
            continue;
        }
        let ok = if exact_only { c.holds(v.v) } else { c.may_hold(v.lo, v.hi) };
        if !ok {
            return Some(c.text());
        }
    }
    None
}

/// Static evaluation of new candidates (journal first), in parallel.
fn statics<F: Farm>(farm: &F, spec: &Spec, j: &Journal, cs: Vec<F::Cand>, local: bool, o: &Opts) -> Vec<Cand<F::Cand>> {
    let keyed: Vec<(String, F::Cand)> = cs.into_iter().map(|c| (farm.key(&c), c)).collect();
    let evals = crate::pool::par_map(&keyed, o.threads, |(key, c)| {
        if let Some(e) = j.at(key, Level::Static) {
            return Ok(Evaluation { metrics: e.metrics, lines: e.lines });
        }
        if let Some(e) = j.at(key, Level::Invalid) {
            return Err(e.note);
        }
        let r = farm.eval_static(c, spec);
        let _ = match &r {
            Ok(e) => j.put(key, Entry { level: Level::Static, metrics: e.metrics.clone(), lines: e.lines.clone(), note: String::new() }),
            Err(why) => j.put(key, Entry { level: Level::Invalid, metrics: Metrics::new(), lines: Default::default(), note: why.clone() }),
        };
        r
    });
    keyed
        .into_iter()
        .zip(evals)
        .map(|((key, c), r)| {
            let fork = fork_of(farm, &c, spec);
            let (statics, lines, bound, fate) = match r {
                Ok(e) => {
                    let b = farm.bound(&c, spec, &e.metrics);
                    let fate = match failing(spec, &e.metrics, true) {
                        Some(f) => Fate::Infeasible(f),
                        None => Fate::Open,
                    };
                    (e.metrics, e.lines, b, fate)
                }
                Err(why) => (Metrics::new(), BTreeMap::new(), Metrics::new(), Fate::Invalid(why)),
            };
            let bar = if matches!(fate, Fate::Open) { farm.bar_of(spec, &c).and_then(|m| pareto::oriented(&spec.objectives, &m, Pick::Optimistic)) } else { None };
            let mut cand = Cand { c, key, fork, statics, lines, bound, quick: None, full: None, fate, local, bar };
            for level in [Level::Quick, Level::Full] {
                if let Some(e) = j.at(&cand.key, level)
                    && e.note.is_empty()
                {
                    cand.lines.extend(e.lines);
                    if level == Level::Quick { cand.quick = Some(e.metrics) } else { cand.full = Some(e.metrics) }
                }
            }
            cand
        })
        .collect()
}

/// Mark the outcome of a stored quick or full evaluation (a failure note in the journal turns
/// into `Invalid`; a passed full judge into `Judged`).
fn settle_stored<F: Farm>(spec: &Spec, j: &Journal, cs: &mut [Cand<F::Cand>]) {
    for c in cs.iter_mut() {
        if c.fate != Fate::Open {
            continue;
        }
        for level in [Level::Quick, Level::Full] {
            if let Some(e) = j.at(&c.key, level)
                && !e.note.is_empty()
            {
                c.fate = Fate::Invalid(format!("{}: {}", level.name(), e.note));
            }
        }
        if c.fate == Fate::Open && c.full.is_some() {
            c.fate = Fate::Judged;
        }
        if c.fate == Fate::Judged
            && let Some(f) = failing(spec, &c.metrics(), false)
        {
            c.fate = Fate::Infeasible(f);
        }
    }
}

/// Evaluate `idx` at `level` (journal first), in parallel; failures become `Invalid`.
fn evaluate<F: Farm>(farm: &F, spec: &Spec, j: &Journal, cs: &mut [Cand<F::Cand>], idx: &[usize], level: Level, threads: usize) {
    let jobs: Vec<(usize, F::Cand, String, Metrics)> = idx.iter().map(|&i| (i, cs[i].c.clone(), cs[i].key.clone(), cs[i].statics.clone())).collect();
    let rs = crate::pool::par_map(&jobs, threads, |(_, c, key, st)| {
        if let Some(e) = j.at(key, level) {
            return if e.note.is_empty() { Ok(Evaluation { metrics: e.metrics, lines: e.lines }) } else { Err(e.note) };
        }
        let r = if level == Level::Quick { farm.eval_quick(c, spec, st) } else { farm.eval_full(c, spec, st) };
        let _ = match &r {
            Ok(e) => j.put(key, Entry { level, metrics: e.metrics.clone(), lines: e.lines.clone(), note: String::new() }),
            Err(why) => j.put(key, Entry { level, metrics: Metrics::new(), lines: Default::default(), note: why.clone() }),
        };
        r
    });
    for ((i, ..), r) in jobs.iter().zip(rs) {
        let c = &mut cs[*i];
        match r {
            Ok(e) => {
                c.lines.extend(e.lines);
                if level == Level::Quick {
                    c.quick = Some(e.metrics);
                } else {
                    c.full = Some(e.metrics);
                    c.fate = Fate::Judged;
                }
                if let Some(f) = failing(spec, &c.metrics(), false) {
                    c.fate = Fate::Infeasible(f);
                }
            }
            Err(why) => c.fate = Fate::Invalid(format!("{}: {why}", level.name())),
        }
    }
}

/// Prune every open candidate of `fork` whose optimistic vector is weakly dominated by a judged
/// candidate's pessimistic vector; returns how many were pruned.
fn prune<C>(spec: &Spec, cs: &mut [Cand<C>], fork: &str) -> usize {
    // every measured feasible candidate prunes with its pessimistic end: the full judge's, or the
    // quick runs' 95% interval (a statistically valid lower end of the same metric)
    let judges: Vec<(String, Vec<f64>)> = cs
        .iter()
        .filter(|c| c.fork == fork && (c.fate == Fate::Judged || (c.fate == Fate::Open && c.quick.is_some())))
        .filter_map(|c| pareto::oriented(&spec.objectives, &c.metrics(), Pick::Pessimistic).map(|v| (c.key.clone(), v)))
        .collect();
    let mut n = 0;
    for c in cs.iter_mut().filter(|c| c.fork == fork && c.fate == Fate::Open) {
        let Some(opt) = pareto::oriented(&spec.objectives, &c.optimistic(), Pick::Optimistic) else { continue };
        if let Some((by, _)) = judges.iter().find(|(k, p)| *k != c.key && pareto::weakly_dominates(p, &opt)) {
            c.fate = Fate::Pruned { by: by.clone(), level: c.level() };
            n += 1;
            continue;
        }
        // the bar (ironspec iteration 12): a candidate that cannot beat it by the merge threshold
        // on one objective without losing by more than the threshold on another is only copies
        if let Some(bar) = &c.bar
            && !beats_bar(&opt, bar, spec.merge)
        {
            c.fate = Fate::Pruned { by: "the bar".into(), level: c.level() };
            n += 1;
        }
    }
    n
}

/// `v` beats `bar` (both oriented: larger is better) by more than `t` (a fraction of the bar's
/// magnitude) on some objective while losing by no more than `t` on any.
pub fn beats_bar(v: &[f64], bar: &[f64], t: f64) -> bool {
    let better = v.iter().zip(bar).any(|(a, b)| a - b > t * b.abs());
    let not_worse = v.iter().zip(bar).all(|(a, b)| b - a <= t * b.abs());
    better && not_worse
}

/// The open candidates of `fork`, best optimistic first (the first objective, then the next, then
/// the key: deterministic).
fn open_order<C>(spec: &Spec, cs: &[Cand<C>], fork: &str) -> Vec<usize> {
    let mut v: Vec<(Vec<f64>, &str, usize)> = cs
        .iter()
        .enumerate()
        .filter(|(_, c)| c.fork == fork && c.fate == Fate::Open)
        .map(|(i, c)| (pareto::oriented(&spec.objectives, &c.optimistic(), Pick::Optimistic).unwrap_or_else(|| vec![f64::INFINITY; spec.objectives.len()]), c.key.as_str(), i))
        .collect();
    v.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(b.1)));
    // the optimistic Pareto layers, the first few: a candidate that could still be best on ANY
    // objective goes before one another open candidate beats on all of them (ordering by the first
    // objective alone starves the front's other end). In the first-objective order a candidate can
    // only be dominated by an earlier one, so each layer is one pass against its members so far.
    let mut out = Vec::with_capacity(v.len());
    let mut rest: Vec<(Vec<f64>, &str, usize)> = v;
    for _ in 0..4 {
        if rest.is_empty() {
            break;
        }
        let mut layer: Vec<(Vec<f64>, &str, usize)> = Vec::new();
        let mut next = Vec::new();
        for c in rest {
            if layer.iter().any(|l| pareto::dominates(&l.0, &c.0)) {
                next.push(c);
            } else {
                layer.push(c);
            }
        }
        // within a layer, the objectives take turns: the best remaining on each in order
        let k = spec.objectives.len().max(1);
        let by: Vec<Vec<usize>> = (0..k)
            .map(|o| {
                let mut ix: Vec<usize> = (0..layer.len()).collect();
                ix.sort_by(|&a, &b| layer[b].0[o].partial_cmp(&layer[a].0[o]).unwrap_or(std::cmp::Ordering::Equal).then(layer[a].1.cmp(layer[b].1)));
                ix
            })
            .collect();
        let mut taken = vec![false; layer.len()];
        let mut pos = vec![0usize; by.len()];
        let mut left = layer.len();
        while left > 0 {
            for (o, list) in by.iter().enumerate() {
                while pos[o] < list.len() {
                    let i = list[pos[o]];
                    pos[o] += 1;
                    if !taken[i] {
                        taken[i] = true;
                        left -= 1;
                        out.push(layer[i].2);
                        break;
                    }
                }
            }
        }
        rest = next;
    }
    out.extend(rest.into_iter().map(|(_, _, i)| i));
    out
}

/// Best-first branch and bound on one fork until nothing is open or the budgets end.
fn search_fork<F: Farm>(farm: &F, spec: &Spec, j: &Journal, cs: &mut [Cand<F::Cand>], fork: &str, o: &Opts, spent: &mut (usize, usize)) {
    // the full budget counts this run's judges (a resumed run's stored ones are free)
    let stored_full = cs.iter().filter(|c| c.fork == fork && c.full.is_some()).count();
    let mut full_here = 0usize;
    let mut last_full_at = spent.0;
    log(o, &format!("{fork}: {stored_full} judged in full already (stored); this run's budget {}", o.full_budget));
    if !o.exhaustive {
        prune(spec, cs, fork);
    }
    loop {
        let order = open_order(spec, cs, fork);
        let Some(&top) = order.first() else { break };
        // the full judge keeps pace with the quick runs: after every quick batch, the best measured
        // candidates in the layered order (best on any objective first) get it, so the front
        // forms while the open set is still being measured (a loose bound keeps unmeasured
        // candidates on top of the order for the whole quick budget otherwise)
        let interleave = !o.no_full && full_here < o.full_budget && cs[top].quick.is_none() && spent.0 >= last_full_at + 2 * o.quick_batch && order.iter().any(|&i| cs[i].quick.is_some());
        if interleave {
            last_full_at = spent.0;
            let take: Vec<usize> = order.iter().copied().filter(|&i| cs[i].quick.is_some()).take(o.full_batch.min(o.full_budget - full_here)).collect();
            log(o, &format!("{fork}: full judge on {} between quick batches (best measured {})", take.len(), cs[take[0]].key));
            evaluate(farm, spec, j, cs, &take, Level::Full, (o.threads / take.len().max(1)).max(1));
            full_here += take.len();
            spent.1 += take.len();
            let failed: std::collections::HashSet<String> = take.iter().filter(|&&i| matches!(cs[i].fate, Fate::Invalid(_) | Fate::Infeasible(_))).map(|&i| cs[i].key.clone()).collect();
            if !failed.is_empty() {
                for c in cs.iter_mut() {
                    if matches!(&c.fate, Fate::Pruned { by, .. } if failed.contains(by)) {
                        c.fate = Fate::Open;
                    }
                }
            }
            let n = if o.exhaustive { 0 } else { prune(spec, cs, fork) };
            if n > 0 {
                log(o, &format!("{fork}: {n} pruned"));
            }
            // the quick runs go on below in the next round
            if spent.0 >= o.quick_budget {
                continue;
            }
        }
        let order = open_order(spec, cs, fork);
        let Some(&top) = order.first() else { break };
        if cs[top].quick.is_none() {
            // the best open candidate is unmeasured: quick runs on the best unmeasured ones
            if spent.0 >= o.quick_budget {
                break;
            }
            let take: Vec<usize> = order.iter().copied().filter(|&i| cs[i].quick.is_none()).take(o.quick_batch.min(o.quick_budget - spent.0)).collect();
            log(o, &format!("{fork}: quick runs on {} (best {})", take.len(), cs[take[0]].key));
            evaluate(farm, spec, j, cs, &take, Level::Quick, o.threads);
            spent.0 += take.len();
        } else if !o.no_full && full_here < o.full_budget {
            // the best open candidate is measured: the full judge on the best measured ones
            let take: Vec<usize> = order.iter().copied().filter(|&i| cs[i].quick.is_some()).take(o.full_batch.min(o.full_budget - full_here)).collect();
            log(o, &format!("{fork}: full judge on {} (best {})", take.len(), cs[take[0]].key));
            evaluate(farm, spec, j, cs, &take, Level::Full, (o.threads / take.len().max(1)).max(1));
            full_here += take.len();
            spent.1 += take.len();
            // a candidate the full judge refuses prunes nothing: reopen what its quick runs pruned
            let failed: std::collections::HashSet<String> = take.iter().filter(|&&i| matches!(cs[i].fate, Fate::Invalid(_) | Fate::Infeasible(_))).map(|&i| cs[i].key.clone()).collect();
            if !failed.is_empty() {
                for c in cs.iter_mut() {
                    if matches!(&c.fate, Fate::Pruned { by, .. } if failed.contains(by)) {
                        c.fate = Fate::Open;
                    }
                }
            }
        } else if o.no_full {
            // no full judge: the measured candidates at the top leave the open set as they are
            for i in order.iter().copied().filter(|&i| cs[i].quick.is_some()).take(o.quick_batch).collect::<Vec<_>>() {
                cs[i].fate = Fate::Judged;
            }
        } else if spent.0 < o.quick_budget && order.iter().any(|&i| cs[i].quick.is_none()) {
            // the full budget is spent: the quick runs go on over the unmeasured open candidates
            // (best first), so their measured upper ends replace their bounds in the gap and prune
            // what they can
            let take: Vec<usize> = order.iter().copied().filter(|&i| cs[i].quick.is_none()).take(o.quick_batch.min(o.quick_budget - spent.0)).collect();
            log(o, &format!("{fork}: quick runs on {} after the full budget (best {})", take.len(), cs[take[0]].key));
            evaluate(farm, spec, j, cs, &take, Level::Quick, o.threads);
            spent.0 += take.len();
        } else {
            break;
        }
        let n = if o.exhaustive { 0 } else { prune(spec, cs, fork) };
        if n > 0 {
            log(o, &format!("{fork}: {n} pruned"));
        }
    }
}

/// The measured front of a fork (judged, feasible), merged (ties on the first objective to the
/// smaller `tie` metric).
fn front_of<C>(spec: &Spec, cs: &[Cand<C>], fork: &str, tie: Option<&str>, own: &dyn Fn(usize) -> bool) -> ForkFront {
    let judged: Vec<usize> = (0..cs.len()).filter(|&i| cs[i].fork == fork && cs[i].fate == Fate::Judged && own(i)).collect();
    let pts: Vec<Vec<f64>> = judged.iter().map(|&i| pareto::oriented(&spec.objectives, &cs[i].metrics(), Pick::Mean).unwrap_or_else(|| vec![f64::NEG_INFINITY; spec.objectives.len()])).collect();
    let ties: Vec<f64> = judged.iter().map(|&i| tie.and_then(|t| cs[i].metrics().get(t).map(|v| v.v)).unwrap_or(0.0)).collect();
    let on: Vec<usize> = pareto::front(&pts);
    let merged_local = pareto::merge(&pts, &on, spec.merge, &ties);
    let members: Vec<usize> = on.iter().map(|&k| judged[k]).collect();
    let merged: Vec<(usize, Vec<usize>)> = merged_local.into_iter().map(|(k, under)| (judged[k], under.into_iter().map(|u| judged[u]).collect())).collect();
    let open: Vec<usize> = (0..cs.len()).filter(|&i| cs[i].fork == fork && cs[i].fate == Fate::Open).collect();
    let k = spec.objectives.len();
    let mut ideal = vec![f64::NEG_INFINITY; k];
    let mut ideal_by = vec![None; k];
    for (ci, c) in cs.iter().enumerate().filter(|(_, c)| c.fork == fork && !matches!(c.fate, Fate::Invalid(_) | Fate::Infeasible(_))) {
        if let Some(v) = pareto::oriented(&spec.objectives, &c.optimistic(), Pick::Optimistic) {
            for a in 0..k {
                if v[a] > ideal[a] {
                    ideal[a] = v[a];
                    ideal_by[a] = Some(ci);
                }
            }
        }
    }
    let mut best = vec![f64::NEG_INFINITY; k];
    for &i in &members {
        if let Some(v) = pareto::oriented(&spec.objectives, &cs[i].metrics(), Pick::Mean) {
            for a in 0..k {
                best[a] = best[a].max(v[a]);
            }
        }
    }
    ForkFront { fork: fork.to_string(), members, merged, open, ideal, ideal_by, best }
}

/// Answer a spec: every fork's merged front.
pub fn run<F: Farm>(farm: &F, spec: &Spec, o: &Opts) -> Result<Outcome<F::Cand>> {
    let t0 = std::time::Instant::now();
    let j = Journal::open(&o.journal, farm.version())?;
    for pat in &o.forget {
        let n = j.forget(pat)?;
        log(o, &format!("forgot {n} journal keys containing {pat:?}"));
    }
    let all = farm.enumerate(spec)?;
    // the bars first (the copies bar's judge runs once, before the parallel statics ask for it)
    for line in farm.bars(spec) {
        log(o, &line);
    }
    // one candidate per key (a grammar may reach one build two ways, e.g. a single module at
    // every row pitch)
    let mut keys_seen: HashSet<String> = HashSet::new();
    let all: Vec<F::Cand> = all.into_iter().filter(|c| keys_seen.insert(farm.key(c))).collect();
    log(o, &format!("{} candidates enumerated; journal {} ({} keys)", all.len(), j.path().display(), j.len()));
    let mut cs = statics(farm, spec, &j, all, false, o);
    settle_stored::<F>(spec, &j, &mut cs);
    log(o, &format!("static: {} valid, {} feasible", cs.iter().filter(|c| !matches!(c.fate, Fate::Invalid(_))).count(), cs.iter().filter(|c| c.fate == Fate::Open || c.fate == Fate::Judged).count()));
    let mut spent = (0usize, 0usize);
    let mut seen: HashSet<String> = cs.iter().map(|c| c.key.clone()).collect();
    for round in 0..=o.local_rounds {
        let forks: Vec<String> = cs.iter().map(|c| c.fork.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for f in &forks {
            search_fork(farm, spec, &j, &mut cs, f, o, &mut spent);
        }
        if round == o.local_rounds {
            break;
        }
        // local search: the neighbours of every front member not seen yet
        let mut fresh: Vec<F::Cand> = Vec::new();
        for f in &forks {
            for &i in &front_of(spec, &cs, f, farm.tie_metric(), &|_| true).members {
                for n in farm.neighbors(&cs[i].c, spec) {
                    if seen.insert(farm.key(&n)) {
                        fresh.push(n);
                    }
                }
            }
        }
        log(o, &format!("local round {}: {} new neighbours", round + 1, fresh.len()));
        if fresh.is_empty() {
            break;
        }
        let mut more = statics(farm, spec, &j, fresh, true, o);
        settle_stored::<F>(spec, &j, &mut more);
        cs.extend(more);
    }
    let forks: Vec<String> = cs.iter().map(|c| c.fork.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let own = own_n(farm, spec, &mut cs);
    let fronts = forks.iter().map(|f| front_of(spec, &cs, f, farm.tie_metric(), &|i| own.contains(&i))).collect();
    Ok(Outcome { cands: cs, forks: fronts, quick_runs: spent.0, full_runs: spent.1, seconds: t0.elapsed().as_secs_f64() })
}

/// Each farm at its own N (`Farm::group` / `Farm::curve`): of a farm's judged candidates, the
/// smallest N whose curve value is within `CURVE_SLACK` of the farm's best; its `curve` line lists
/// every judged N (the objectives at each). Candidates without a group are their own farms.
fn own_n<F: Farm>(farm: &F, spec: &Spec, cs: &mut [Cand<F::Cand>]) -> std::collections::HashSet<usize> {
    let mut groups: BTreeMap<(String, String), Vec<(u32, usize)>> = BTreeMap::new();
    let mut own = std::collections::HashSet::new();
    for (i, c) in cs.iter().enumerate() {
        if c.fate != Fate::Judged {
            continue;
        }
        match farm.group(&c.c) {
            Some((g, n)) => groups.entry((c.fork.clone(), g)).or_default().push((n, i)),
            None => {
                own.insert(i);
            }
        }
    }
    for (_, mut v) in groups {
        v.sort();
        let vals: Vec<f64> = v.iter().map(|&(_, i)| farm.curve(&cs[i].metrics()).unwrap_or(f64::NEG_INFINITY)).collect();
        let best = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let pick = v.iter().zip(&vals).find(|(_, val)| **val >= best * (1.0 - crate::CURVE_SLACK) - 1e-12).map(|(&(_, i), _)| i).unwrap_or(v[0].1);
        own.insert(pick);
        let text = v
            .iter()
            .map(|&(n, i)| {
                let m = cs[i].metrics();
                let objs: Vec<String> = spec.objectives.iter().map(|o| m.get(&o.metric).map(|x| format!("{:.1}", x.v)).unwrap_or_else(|| "-".into())).collect();
                format!("N={n}: {}", objs.join(" / "))
            })
            .collect::<Vec<_>>()
            .join("; ");
        let names: Vec<&str> = spec.objectives.iter().map(|o| o.metric.as_str()).collect();
        cs[pick].lines.insert("curve".into(), format!("{text} ({}; own N = the smallest within {:.0}% of the best {} on the curve)", names.join(" / "), crate::CURVE_SLACK * 100.0, "curve value"));
    }
    own
}

/// Counts per fate, for the summary.
pub fn census<C>(cs: &[Cand<C>], fork: &str) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for c in cs.iter().filter(|c| c.fork == fork) {
        let k = match &c.fate {
            Fate::Open => "open (undecided)",
            Fate::Invalid(_) => "invalid",
            Fate::Infeasible(_) => "infeasible",
            Fate::Pruned { level: Level::Static, .. } | Fate::Pruned { level: Level::Invalid, .. } => "pruned by bound",
            Fate::Pruned { .. } => "pruned after quick runs",
            Fate::Judged => "judged",
        };
        *m.entry(k).or_default() += 1;
        if c.quick.is_some() {
            *m.entry("quick-run").or_default() += 1;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Spec;

    /// A toy farm: candidates (a, b) in 0..10 x 0..10; "rate" = a + b (quick: the same with a
    /// 0.5 interval, full exact), "cost" = a + 10 (static, exact). Bound: rate <= a + b + 1.
    /// Local search steps b up to 10, one past the enumeration.
    struct Toy;

    impl Farm for Toy {
        type Cand = (u32, u32);
        fn id(&self) -> &'static str {
            "toy"
        }
        fn version(&self) -> &'static str {
            "toy-1"
        }
        fn params(&self) -> Vec<crate::Param> {
            vec![]
        }
        fn enumerate(&self, _: &Spec) -> Result<Vec<(u32, u32)>> {
            Ok((0..10).flat_map(|a| (0..10).map(move |b| (a, b))).collect())
        }
        fn key(&self, c: &(u32, u32)) -> String {
            format!("a{}-b{}", c.0, c.1)
        }
        fn param(&self, c: &(u32, u32), name: &str) -> Option<String> {
            (name == "odd").then(|| (c.0 % 2).to_string())
        }
        fn neighbors(&self, c: &(u32, u32), _: &Spec) -> Vec<(u32, u32)> {
            if c.1 < 10 { vec![(c.0, c.1 + 1)] } else { vec![] }
        }
        fn design(&self, c: &(u32, u32), _: &Spec) -> Result<String> {
            Ok(format!("toy {c:?}"))
        }
        fn eval_static(&self, c: &(u32, u32), _: &Spec) -> std::result::Result<Evaluation, String> {
            if c.1 == 3 {
                return Err("b = 3 is refused".into());
            }
            Ok(Evaluation::of(BTreeMap::from([("cost".to_string(), Value::exact((c.0 + 10) as f64))])))
        }
        fn bound(&self, c: &(u32, u32), _: &Spec, _: &Metrics) -> Metrics {
            BTreeMap::from([("rate".to_string(), Value { v: (c.0 + c.1 + 1) as f64, lo: f64::NEG_INFINITY, hi: (c.0 + c.1 + 1) as f64 })])
        }
        fn eval_quick(&self, c: &(u32, u32), _: &Spec, _: &Metrics) -> std::result::Result<Evaluation, String> {
            let r = (c.0 + c.1) as f64;
            Ok(Evaluation::of(BTreeMap::from([("rate".to_string(), Value { v: r, lo: r - 0.5, hi: r + 0.5 })])))
        }
        fn eval_full(&self, c: &(u32, u32), _: &Spec, _: &Metrics) -> std::result::Result<Evaluation, String> {
            Ok(Evaluation::of(BTreeMap::from([("rate".to_string(), Value::exact((c.0 + c.1) as f64))])))
        }
    }

    fn opts(name: &str) -> Opts {
        let dir = std::env::temp_dir().join(format!("farmopt-search-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Opts { journal: dir.join("journal.tsv"), threads: 4, quick_budget: 1000, full_budget: 100, quick_batch: 8, full_batch: 2, local_rounds: 1, no_full: false, verbose: false, forget: Vec::new(), exhaustive: false }
    }

    #[test]
    fn the_bar_must_be_beaten_by_the_threshold_without_losing_by_more() {
        // oriented (larger is better): rate 444, -build 24.7 for the bar
        let bar = [444.0, -24.7];
        // 10% faster to build at 95% of the rate: beats it
        assert!(beats_bar(&[422.0, -22.0], &bar, 0.10));
        // only 5% faster to build: not by the threshold
        assert!(!beats_bar(&[444.0, -23.5], &bar, 0.10));
        // 20% faster to build but 15% slower: loses by more than the threshold
        assert!(!beats_bar(&[377.0, -19.0], &bar, 0.10));
        // 12% more iron at 5% more build: beats it
        assert!(beats_bar(&[500.0, -25.9], &bar, 0.10));
    }

    #[test]
    fn the_toy_front_is_found_and_most_is_pruned() {
        let spec = Spec::parse("farm: toy\nstage: s\nobjective: max rate\nobjective: min cost\nmerge: 0%\n").unwrap();
        let o = opts("front");
        let out = run(&Toy, &spec, &o).unwrap();
        let f = &out.forks[0];
        // the front: for every a, the largest b, which local search found one past the
        // enumeration (10): rate a + 10 at cost a + 10, all 10 trade off
        let mut keys: Vec<String> = f.members.iter().map(|&i| out.cands[i].key.clone()).collect();
        keys.sort();
        let mut want: Vec<String> = (0..10).map(|a| format!("a{a}-b10")).collect();
        want.sort();
        assert_eq!(keys, want);
        assert!(f.open.is_empty());
        let c = census(&out.cands, "all");
        assert_eq!(c["invalid"], 10);
        assert!(c.get("judged").copied().unwrap_or(0) < 40, "{c:?}");
        // a rerun evaluates nothing new (the journal) and gives the same front
        let again = run(&Toy, &spec, &o).unwrap();
        assert_eq!(again.forks[0].members.len(), 10);
        assert_eq!((again.quick_runs, again.full_runs), (0, 0));
        let _ = std::fs::remove_dir_all(o.journal.parent().unwrap());
    }

    #[test]
    fn forks_and_merging() {
        let spec = Spec::parse("farm: toy\nstage: s\nobjective: max rate\nobjective: min cost\nfork: odd\nmerge: 30%\nconstraint: cost <= 15\n").unwrap();
        let o = opts("forks");
        let out = run(&Toy, &spec, &o).unwrap();
        assert_eq!(out.forks.len(), 2);
        for f in &out.forks {
            // a <= 5 by the constraint; the merge folds neighbours within 30%
            assert!(f.merged.len() < f.members.len(), "{} {}", f.merged.len(), f.members.len());
            for &i in &f.members {
                assert!(out.cands[i].c.0 <= 5 && out.cands[i].c.1 == 10);
            }
        }
        let _ = std::fs::remove_dir_all(o.journal.parent().unwrap());
    }
}
