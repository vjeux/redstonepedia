//! The farm optimizer's core (designs/briefs/farm-optimizer.md, docs/farms-optimizer.md):
//! farm-agnostic. A farm plugs in by implementing [`Farm`]: its space (candidates with a
//! canonical key, enumerated valid under a [`spec::Spec`]), its staged evaluation (static,
//! quick, full: each adds metrics to the candidate's vector), and its proven bound (an
//! optimistic value of every metric an objective can name). [`search::run`] then answers a spec
//! with a merged Pareto front per fork, the bound front and the gap ([`report`]), reusing every
//! evaluation already in the run's [`journal`] so a changed objective re-ranks in seconds.

pub mod journal;
pub mod pareto;
pub mod pool;
pub mod report;
pub mod search;
pub mod spec;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A metric's value: exact (`lo == hi == v`), or measured with its 95% interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Value {
    pub v: f64,
    pub lo: f64,
    pub hi: f64,
}

impl Value {
    /// An exact value (counted, or derived from the game's rules without sampling).
    pub fn exact(v: f64) -> Value {
        Value { v, lo: v, hi: v }
    }

    /// A sampled mean with its 95% interval from `xs` (Student's t for small samples).
    pub fn sampled(xs: &[f64]) -> Value {
        let n = xs.len();
        if n == 0 {
            return Value::exact(f64::NAN);
        }
        let m = xs.iter().sum::<f64>() / n as f64;
        if n < 2 {
            return Value { v: m, lo: f64::NEG_INFINITY, hi: f64::INFINITY };
        }
        let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1) as f64;
        let half = t95(n - 1) * (var / n as f64).sqrt();
        Value { v: m, lo: m - half, hi: m + half }
    }

    /// Whether it carries an interval (a sampled value).
    pub fn is_exact(&self) -> bool {
        self.lo == self.hi
    }
}

/// Student's t two-sided 95% quantile for `df` degrees of freedom.
pub fn t95(df: usize) -> f64 {
    const T: [f64; 30] = [12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056, 2.052, 2.048, 2.045, 2.042];
    if df == 0 {
        f64::INFINITY
    } else if df <= 30 {
        T[df - 1]
    } else {
        1.96 + 2.4 / df as f64
    }
}

/// A candidate's metrics by name.
pub type Metrics = BTreeMap<String, Value>;

/// What one evaluation level returns: numeric metrics (objectives, constraints, the bound) and the
/// judge's text lines (a card's facts: "restarts after days away", "dims", ...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub metrics: Metrics,
    pub lines: BTreeMap<String, String>,
}

impl Evaluation {
    pub fn of(metrics: Metrics) -> Evaluation {
        Evaluation { metrics, lines: BTreeMap::new() }
    }
}

/// How far a candidate's evaluation went (the journal keeps the furthest).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Refused by the static stage (its note says why).
    Invalid,
    /// Static analysis only: exact counts and the bound.
    Static,
    /// Short simulator runs: estimates with intervals.
    Quick,
    /// The farm's whole judge.
    Full,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Invalid => "invalid",
            Level::Static => "static",
            Level::Quick => "quick",
            Level::Full => "full",
        }
    }
}

/// One of a farm's grammar parameters: its name, its values in words, what it means.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: &'static str,
    pub values: Vec<String>,
    pub doc: &'static str,
}

/// A farm the optimizer can search: its space, its evaluation, its bound.
pub trait Farm: Sync {
    /// A point of the space: the typed parameter vector.
    type Cand: Clone + Send + Sync;

    /// The farm's id on a spec's `farm:` line.
    fn id(&self) -> &'static str;
    /// The evaluator's version: journal entries of another version are not reused.
    fn version(&self) -> &'static str;
    /// The grammar's parameters (what `space:` lines narrow and `fork:` lines split on).
    fn params(&self) -> Vec<Param>;
    /// Every valid candidate of the space under `spec` (its stage limits and `space:` lines).
    fn enumerate(&self, spec: &spec::Spec) -> anyhow::Result<Vec<Self::Cand>>;
    /// The candidate's canonical key (stable across runs and machines).
    fn key(&self, c: &Self::Cand) -> String;
    /// A parameter's value in words (forks, reports).
    fn param(&self, c: &Self::Cand, name: &str) -> Option<String>;
    /// Geometric neighbours for local search (any of them may lie outside the enumeration).
    fn neighbors(&self, c: &Self::Cand, spec: &spec::Spec) -> Vec<Self::Cand>;
    /// The candidate as a design file (the judge's text format, preview-ready).
    fn design(&self, c: &Self::Cand, spec: &spec::Spec) -> anyhow::Result<String>;
    /// Static analysis: validity and exact metrics; `Err` refuses the candidate (why).
    fn eval_static(&self, c: &Self::Cand, spec: &spec::Spec) -> Result<Evaluation, String>;
    /// The proven optimistic value of every metric an objective may name, from the static
    /// metrics: no evaluation of the candidate can beat it.
    fn bound(&self, c: &Self::Cand, spec: &spec::Spec, st: &Metrics) -> Metrics;
    /// Short simulator runs (estimates with intervals).
    fn eval_quick(&self, c: &Self::Cand, spec: &spec::Spec, st: &Metrics) -> Result<Evaluation, String>;
    /// The farm's whole judge.
    fn eval_full(&self, c: &Self::Cand, spec: &spec::Spec, st: &Metrics) -> Result<Evaluation, String>;
    /// The metric that breaks ties on the first objective in the merge (fewer is better), if any.
    fn tie_metric(&self) -> Option<&'static str> {
        None
    }
    /// The card rows a stage shows (a `key \t slot \t line` file and the stage's key): front.md
    /// renders them first for each member (`{metric|round}`, `{metric|thousands}`, `{line}`).
    fn cards(&self, _spec: &spec::Spec) -> Option<(std::path::PathBuf, String)> {
        None
    }
    /// A farm and its N (iron brief rev 9, "A farm and its N"): candidates that differ only in how
    /// many copies of one unit they hold are ONE farm. `group` names the farm (the candidate's key
    /// without its count) and the count; `curve` the value whose best on the farm's curve picks the
    /// farm's own N (the smallest judged N within `CURVE_SLACK` of the best); the front then holds
    /// each farm once, at its own N. None: every candidate is its own farm.
    fn group(&self, _c: &Self::Cand) -> Option<(String, u32)> {
        None
    }
    fn curve(&self, _m: &Metrics) -> Option<f64> {
        None
    }
}

/// How far below its best a farm's curve value may be at its own N (10%).
pub const CURVE_SLACK: f64 = 0.10;
