//! The stage spec (designs/briefs/farm-optimizer.md, "The loop"): what one stage of a farm
//! optimizes, one plain line per item. The stage-spec session writes it; the engine reads it.
//!
//! ```text
//! farm: iron
//! stage: middle
//! objective: max iron_per_hour_per_block
//! objective: min build_minutes
//! constraint: materials <= nether
//! constraint: modules in 2..4
//! fork: calm
//! merge: 5%
//! space: pad 4..10 by 2
//! space: calm observer,daylight
//! ```
//!
//! Any other `key: value` line is kept for the farm (`terrain: village-edge`, `quick: 4`).

use anyhow::{Result, anyhow, bail};
use std::collections::BTreeMap;

/// One objective: a metric and its direction.
#[derive(Clone, Debug, PartialEq)]
pub struct Objective {
    pub metric: String,
    pub max: bool,
}

/// A constraint's comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Le(f64),
    Ge(f64),
    In(f64, f64),
}

/// One constraint on a metric.
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub metric: String,
    pub op: Op,
}

impl Constraint {
    /// Whether `v` meets it.
    pub fn holds(&self, v: f64) -> bool {
        match self.op {
            Op::Le(x) => v <= x + 1e-9,
            Op::Ge(x) => v >= x - 1e-9,
            Op::In(a, b) => v >= a - 1e-9 && v <= b + 1e-9,
        }
    }

    /// Whether some value within `[lo, hi]` could meet it (an interval or a bound).
    pub fn may_hold(&self, lo: f64, hi: f64) -> bool {
        match self.op {
            Op::Le(x) => lo <= x + 1e-9,
            Op::Ge(x) => hi >= x - 1e-9,
            Op::In(a, b) => hi >= a - 1e-9 && lo <= b + 1e-9,
        }
    }

    pub fn text(&self) -> String {
        match self.op {
            Op::Le(x) => format!("{} <= {x}", self.metric),
            Op::Ge(x) => format!("{} >= {x}", self.metric),
            Op::In(a, b) => format!("{} in {a}..{b}", self.metric),
        }
    }
}

/// A stage spec.
#[derive(Clone, Debug, Default)]
pub struct Spec {
    pub farm: String,
    pub stage: String,
    pub objectives: Vec<Objective>,
    pub constraints: Vec<Constraint>,
    /// One front per value of each fork parameter (several: their product).
    pub forks: Vec<String>,
    /// Members within this relative distance on every objective are one farm.
    pub merge: f64,
    /// Parameter -> its allowed values in words (narrows the farm's grammar).
    pub space: BTreeMap<String, Vec<String>>,
    /// Every other line, for the farm.
    pub extra: BTreeMap<String, String>,
    /// The text it was read from.
    pub text: String,
}

/// Words a constraint may use for a categorical metric, and their codes (every farm's metrics
/// carry the code): the stage of the game a build's materials need.
pub fn word_code(metric: &str, word: &str) -> Option<f64> {
    match (metric, word) {
        ("materials", "day-1" | "day1" | "overworld") => Some(1.0),
        ("materials", "nether") => Some(2.0),
        ("materials", "end" | "any") => Some(3.0),
        _ => None,
    }
}

/// `4..10 by 2`, `2..4`, `a,b,c` or a single word -> the values in words.
pub fn values(text: &str) -> Result<Vec<String>> {
    let t = text.trim();
    if let Some((range, step)) = t.split_once(" by ").map(|(a, b)| (a.trim(), b.trim().parse::<i64>())).or_else(|| t.contains("..").then_some((t, Ok(1)))) {
        let step = step.map_err(|e| anyhow!("{t}: step: {e}"))?;
        let (a, b) = range.split_once("..").ok_or_else(|| anyhow!("{t}: a range is a..b"))?;
        let (a, b): (i64, i64) = (a.trim().parse()?, b.trim().parse()?);
        if step <= 0 || b < a {
            bail!("{t}: an empty range");
        }
        return Ok((0..).map(|k| a + k * step).take_while(|v| *v <= b).map(|v| v.to_string()).collect());
    }
    Ok(t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

fn number(metric: &str, w: &str) -> Result<f64> {
    let w = w.trim();
    w.parse::<f64>().ok().or_else(|| word_code(metric, w)).ok_or_else(|| anyhow!("{metric}: not a number or a known word: {w:?}"))
}

fn constraint(v: &str) -> Result<Constraint> {
    let words: Vec<&str> = v.split_whitespace().collect();
    let [metric, op, rest @ ..] = words.as_slice() else { bail!("constraint: `<metric> <=|>=|in <value>`: {v:?}") };
    let rest = rest.join(" ");
    let metric = metric.to_string();
    let op = match *op {
        "<=" => Op::Le(number(&metric, &rest)?),
        ">=" => Op::Ge(number(&metric, &rest)?),
        "=" | "==" => {
            let x = number(&metric, &rest)?;
            Op::In(x, x)
        }
        "in" => {
            let (a, b) = rest.split_once("..").ok_or_else(|| anyhow!("constraint {v:?}: in a..b"))?;
            Op::In(number(&metric, a)?, number(&metric, b)?)
        }
        o => bail!("constraint {v:?}: no operator {o:?} (<=, >=, =, in)"),
    };
    Ok(Constraint { metric, op })
}

impl Spec {
    /// Parse a spec's text (`#` starts a comment).
    pub fn parse(text: &str) -> Result<Spec> {
        let mut s = Spec { merge: 0.05, text: text.to_string(), ..Default::default() };
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once(':').ok_or_else(|| anyhow!("line {}: `key: value`", n + 1))?;
            let (k, v) = (k.trim(), v.trim());
            match k {
                "farm" => s.farm = v.to_string(),
                "stage" => s.stage = v.to_string(),
                "objective" => {
                    let (dir, metric) = v.split_once(' ').ok_or_else(|| anyhow!("line {}: objective: max|min <metric>", n + 1))?;
                    let max = match dir {
                        "max" => true,
                        "min" => false,
                        d => bail!("line {}: objective direction {d:?} (max, min)", n + 1),
                    };
                    s.objectives.push(Objective { metric: metric.trim().to_string(), max });
                }
                "constraint" => s.constraints.push(constraint(v).map_err(|e| anyhow!("line {}: {e}", n + 1))?),
                "fork" => s.forks.extend(v.split(',').map(|f| f.trim().to_string()).filter(|f| !f.is_empty())),
                "merge" => s.merge = v.trim_end_matches('%').trim().parse::<f64>().map_err(|e| anyhow!("line {}: merge: {e}", n + 1))? / if v.ends_with('%') { 100.0 } else { 1.0 },
                "space" => {
                    let (p, vals) = v.split_once(' ').ok_or_else(|| anyhow!("line {}: space: <parameter> <values>", n + 1))?;
                    s.space.insert(p.trim().to_string(), values(vals).map_err(|e| anyhow!("line {}: {e}", n + 1))?);
                }
                _ => {
                    s.extra.insert(k.to_string(), v.to_string());
                }
            }
        }
        if s.farm.is_empty() || s.stage.is_empty() {
            bail!("a spec names its `farm:` and its `stage:`");
        }
        if s.objectives.is_empty() || s.objectives.len() > 4 {
            bail!("a spec has 1 to 4 objectives (it has {})", s.objectives.len());
        }
        Ok(s)
    }

    /// Whether the parameter `name` may take the value `v` (no `space:` line for it: any).
    pub fn allows(&self, name: &str, v: &str) -> bool {
        self.space.get(name).is_none_or(|vals| vals.iter().any(|x| x == v))
    }

    /// An extra line's value parsed, or `default`.
    pub fn extra_or<T: std::str::FromStr>(&self, key: &str, default: T) -> T {
        self.extra.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    /// A stable fingerprint of what the space and the evaluation depend on (not the objectives,
    /// the constraints or the merge: those only re-rank).
    pub fn space_key(&self) -> String {
        let mut s = format!("{}|{}", self.farm, self.stage);
        for (k, v) in &self.space {
            s.push_str(&format!("|{k}={}", v.join(",")));
        }
        for (k, v) in &self.extra {
            s.push_str(&format!("|{k}:{v}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_reads_back() {
        let s = Spec::parse("farm: iron\nstage: middle # the evening build\nobjective: max iron_per_hour_per_block\nobjective: min build_minutes\nconstraint: materials <= nether\nconstraint: modules in 2..4\nfork: calm\nmerge: 5%\nspace: pad 4..10 by 2\nspace: calm observer,daylight\nterrain: village-edge\n").unwrap();
        assert_eq!(s.farm, "iron");
        assert_eq!(s.objectives, vec![Objective { metric: "iron_per_hour_per_block".into(), max: true }, Objective { metric: "build_minutes".into(), max: false }]);
        assert_eq!(s.constraints[0], Constraint { metric: "materials".into(), op: Op::Le(2.0) });
        assert_eq!(s.constraints[1].op, Op::In(2.0, 4.0));
        assert_eq!(s.forks, vec!["calm".to_string()]);
        assert!((s.merge - 0.05).abs() < 1e-12);
        assert_eq!(s.space["pad"], vec!["4", "6", "8", "10"]);
        assert!(s.allows("calm", "daylight") && !s.allows("calm", "plate") && s.allows("kill", "pit"));
        assert_eq!(s.extra["terrain"], "village-edge");
        assert!(s.constraints[1].holds(3.0) && !s.constraints[1].holds(5.0) && s.constraints[1].may_hold(4.5, 6.0) == false);
    }

    #[test]
    fn a_bad_spec_says_why() {
        assert!(Spec::parse("farm: iron\nobjective: max x\n").is_err());
        assert!(Spec::parse("farm: iron\nstage: a\nobjective: up x\n").is_err());
        assert!(Spec::parse("farm: iron\nstage: a\nobjective: max x\nconstraint: x ~ 3\n").is_err());
        assert!(values("10..4").is_err());
    }
}
