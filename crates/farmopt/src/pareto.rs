//! Pareto fronts: dominance on oriented objective vectors (every axis larger-is-better), the
//! non-dominated set, the epsilon merge (vjeux 2026-11-08 10:56: "I also don't want hundreds of
//! tiny variations that don't meaningfully affect the farm") and the pruning test of the branch
//! and bound.

use crate::spec::Objective;
use crate::{Metrics, Value};

/// Which end of a value an objective vector takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    /// The mean (the front's ranking).
    Mean,
    /// The end worst for the objective (the lower end of a max objective's interval): what a
    /// candidate surely reaches, for pruning others.
    Pessimistic,
    /// The end best for the objective: what a candidate might reach.
    Optimistic,
}

fn end(v: &Value, max: bool, pick: Pick) -> f64 {
    match (pick, max) {
        (Pick::Mean, _) => v.v,
        (Pick::Pessimistic, true) | (Pick::Optimistic, false) => v.lo,
        (Pick::Pessimistic, false) | (Pick::Optimistic, true) => v.hi,
    }
}

/// The objective vector of `m`, oriented (min objectives negated); None when a metric is missing
/// or not a number.
pub fn oriented(objs: &[Objective], m: &Metrics, pick: Pick) -> Option<Vec<f64>> {
    objs.iter()
        .map(|o| {
            let v = m.get(&o.metric)?;
            let x = end(v, o.max, pick);
            if x.is_nan() { None } else { Some(if o.max { x } else { -x }) }
        })
        .collect()
}

/// `a` dominates `b`: at least as good on every axis, better on one.
pub fn dominates(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b).all(|(x, y)| x >= y) && a.iter().zip(b).any(|(x, y)| x > y)
}

/// `a` is at least as good as `b` on every axis (a candidate whose best is weakly dominated by
/// another's worst can never be strictly better than it: pruned).
pub fn weakly_dominates(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b).all(|(x, y)| x >= y)
}

/// The non-dominated points (their indices into `pts`), in input order. Equal points are all kept
/// (the merge folds them).
pub fn front(pts: &[Vec<f64>]) -> Vec<usize> {
    (0..pts.len()).filter(|&i| !(0..pts.len()).any(|j| j != i && dominates(&pts[j], &pts[i]))).collect()
}

/// Whether `m` is within the relative threshold `eps` of `best` on every axis (|m - best| <=
/// eps |best|).
pub fn within(best: &[f64], m: &[f64], eps: f64) -> bool {
    best.iter().zip(m).all(|(b, x)| (x - b).abs() <= eps * b.abs().max(1e-9))
}

/// The merge of a front (ironspec's rule, greedy around the best): the members sorted by the
/// first objective (ties: `tie`, smaller first, then the input order), the best takes every member
/// within `eps` of IT on every objective as its runner-ups, then the best of the rest does the
/// same, and so on (no chaining: a runner-up never takes others). Returns (kept index, its
/// runner-ups), indices into `pts`.
pub fn merge(pts: &[Vec<f64>], members: &[usize], eps: f64, tie: &[f64]) -> Vec<(usize, Vec<usize>)> {
    let mut order: Vec<usize> = members.to_vec();
    order.sort_by(|&a, &b| pts[b][0].partial_cmp(&pts[a][0]).unwrap_or(std::cmp::Ordering::Equal).then(tie.get(a).partial_cmp(&tie.get(b)).unwrap_or(std::cmp::Ordering::Equal)));
    let mut left: Vec<usize> = order;
    let mut kept: Vec<(usize, Vec<usize>)> = Vec::new();
    while let Some(&best) = left.first() {
        let (under, rest): (Vec<usize>, Vec<usize>) = left[1..].iter().partition(|&&i| within(&pts[best], &pts[i], eps));
        kept.push((best, under));
        left = rest;
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominance_and_the_front() {
        let pts = vec![vec![3.0, -10.0], vec![2.0, -5.0], vec![1.0, -12.0], vec![3.0, -10.0], vec![2.5, -5.0]];
        assert!(dominates(&pts[0], &pts[2]) && !dominates(&pts[0], &pts[3]) && weakly_dominates(&pts[0], &pts[3]));
        assert_eq!(front(&pts), vec![0, 3, 4]);
    }

    #[test]
    fn the_merge_folds_near_duplicates() {
        // two front members 1% apart on both axes are one farm; a third, 30% better on the second
        // axis, stays
        let pts = vec![vec![100.0, -50.0], vec![99.0, -49.6], vec![80.0, -35.0]];
        assert_eq!(front(&pts), vec![0, 1, 2]);
        let m = merge(&pts, &[0, 1, 2], 0.05, &[]);
        assert_eq!(m, vec![(0, vec![1]), (2, vec![])]);
        assert_eq!(merge(&pts, &[0, 1, 2], 0.0, &[]).len(), 3);
        // no chaining: 100 takes 95 (5%), but not 90.5 even though 95 is within 5% of 90.5
        let chain = vec![vec![100.0], vec![95.0], vec![90.5]];
        assert_eq!(merge(&chain, &[0, 1, 2], 0.05, &[]), vec![(0, vec![1]), (2, vec![])]);
        // ties on the first objective go to the smaller tie metric
        let tied = vec![vec![10.0, -5.0], vec![10.0, -5.2]];
        assert_eq!(merge(&tied, &[0, 1], 0.1, &[300.0, 200.0]), vec![(1, vec![0])]);
    }

    #[test]
    fn oriented_vectors_take_the_right_end() {
        let objs = vec![Objective { metric: "rate".into(), max: true }, Objective { metric: "minutes".into(), max: false }];
        let mut m = Metrics::new();
        m.insert("rate".into(), Value { v: 10.0, lo: 8.0, hi: 12.0 });
        m.insert("minutes".into(), Value { v: 30.0, lo: 28.0, hi: 33.0 });
        assert_eq!(oriented(&objs, &m, Pick::Pessimistic), Some(vec![8.0, -33.0]));
        assert_eq!(oriented(&objs, &m, Pick::Optimistic), Some(vec![12.0, -28.0]));
        assert_eq!(oriented(&objs, &m, Pick::Mean), Some(vec![10.0, -30.0]));
        m.remove("rate");
        assert_eq!(oriented(&objs, &m, Pick::Mean), None);
    }
}
