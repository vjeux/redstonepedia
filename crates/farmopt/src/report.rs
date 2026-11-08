//! The answer of a search (designs/briefs/farm-optimizer.md, "The loop"): `summary.md`,
//! `front.md`, `ranked.tsv` and one design file per merged member, regenerated each run.

use crate::search::{Cand, Fate, Outcome};
use crate::spec::Spec;
use crate::{Farm, Metrics, Value};
use anyhow::Result;
use std::fmt::Write as _;
use std::path::Path;

/// A value as a card reads it: the mean, and its 95% interval when sampled.
pub fn show(v: &Value) -> String {
    let p = |x: f64| if x.abs() >= 100.0 { format!("{x:.0}") } else if x.abs() >= 10.0 { format!("{x:.1}") } else { format!("{x:.2}") };
    if v.is_exact() || !v.lo.is_finite() || !v.hi.is_finite() { p(v.v) } else { format!("{} ({}-{})", p(v.v), p(v.lo), p(v.hi)) }
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' }).collect()
}

/// An oriented objective value back in its own direction.
fn unorient(max: bool, x: f64) -> f64 {
    if max { x } else { -x }
}

fn objective_cells(spec: &Spec, m: &Metrics) -> Vec<String> {
    spec.objectives.iter().map(|o| m.get(&o.metric).map(show).unwrap_or_else(|| "-".into())).collect()
}

fn why<C>(c: &Cand<C>) -> String {
    match &c.fate {
        Fate::Open => "open: undecided when the search stopped".into(),
        Fate::Invalid(w) => format!("invalid: {w}"),
        Fate::Infeasible(w) => format!("infeasible: {w}"),
        Fate::Pruned { by, level } => format!("pruned at {} by {by}", level.name()),
        Fate::Judged => "judged".into(),
    }
}

/// Write the answer into `dir` (created); returns the files written.
pub fn write<F: Farm>(farm: &F, spec: &Spec, out: &Outcome<F::Cand>, dir: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let mut files = Vec::new();
    // one design file per merged member (and its runner-ups), by fork and rank
    let mut design_of: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    for f in &out.forks {
        let fd = dir.join(slug(&f.fork));
        std::fs::create_dir_all(&fd)?;
        for (rank, (kept, under)) in f.merged.iter().enumerate() {
            for (k, i) in std::iter::once(kept).chain(under.iter()).enumerate() {
                let name = if k == 0 { format!("{}-{}.txt", rank + 1, slug(&out.cands[*i].key)) } else { format!("{}-runner-{}-{}.txt", rank + 1, k, slug(&out.cands[*i].key)) };
                let path = fd.join(&name);
                // the design as judged (stored with its numbers) when the journal has it, else
                // generated now
                let text = match out.cands[*i].lines.get("design_text") {
                    Some(t) => t.clone(),
                    None => farm.design(&out.cands[*i].c, spec)?,
                };
                std::fs::write(&path, text)?;
                let rel = format!("{}/{}", slug(&f.fork), name);
                design_of.insert(*i, rel.clone());
                files.push(rel);
            }
        }
    }
    let cards = farm.cards(spec).map(|(path, key)| card_rows(&path, &key)).unwrap_or_default();
    std::fs::write(dir.join("front.md"), front_md(spec, out, &design_of, &cards))?;
    std::fs::write(dir.join("summary.md"), summary_md(farm, spec, out))?;
    std::fs::write(dir.join("ranked.tsv"), ranked_tsv(farm, spec, out))?;
    std::fs::write(dir.join("spec.txt"), &spec.text)?;
    files.extend(["front.md", "summary.md", "ranked.tsv", "spec.txt"].map(String::from));
    Ok(files)
}

/// The card rows of `key` in a `key \t slot \t line` file (none when it cannot be read).
pub fn card_rows(path: &Path, key: &str) -> Vec<String> {
    std::fs::read_to_string(path).map(|t| t.lines().filter_map(|l| l.split('\t').collect::<Vec<_>>().get(..3).filter(|f| f[0] == key).map(|f| f[2].to_string())).collect()).unwrap_or_default()
}

/// A card row with its placeholders filled: `{metric|round}`, `{metric|thousands}`, `{metric}` (a
/// number as [`show`] prints it, or a text line); a placeholder nothing measured reads
/// `{name: not measured}`.
pub fn render(template: &str, m: &Metrics, lines: &std::collections::BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(a) = rest.find('{') {
        out.push_str(&rest[..a]);
        let Some(b) = rest[a..].find('}') else { break };
        let inner = &rest[a + 1..a + b];
        let (name, fmt) = inner.split_once('|').unwrap_or((inner, ""));
        let filled = match (m.get(name), lines.get(name)) {
            (Some(v), _) => match fmt {
                "round" => format!("{:.0}", v.v),
                "thousands" => {
                    let n = v.v.round() as i64;
                    let s = n.abs().to_string();
                    let mut g = String::new();
                    for (k, ch) in s.chars().enumerate() {
                        if k > 0 && (s.len() - k) % 3 == 0 {
                            g.push(',');
                        }
                        g.push(ch);
                    }
                    if n < 0 { format!("-{g}") } else { g }
                }
                _ => show(v),
            },
            (None, Some(t)) => t.clone(),
            (None, None) => format!("{{{name}: not measured}}"),
        };
        out.push_str(&filled);
        rest = &rest[a + b + 1..];
    }
    out.push_str(rest);
    out
}

fn front_md<C>(spec: &Spec, out: &Outcome<C>, design_of: &std::collections::HashMap<usize, String>, cards: &[String]) -> String {
    let mut s = format!("# {} {}: the merged fronts\n\nGenerated by `mctas farmopt run` (designs/briefs/farm-optimizer.md); do not edit. Objectives: {}. Merge: {:.0}%.\n", spec.farm, spec.stage, spec.objectives.iter().map(|o| format!("{} {}", if o.max { "max" } else { "min" }, o.metric)).collect::<Vec<_>>().join(", "), spec.merge * 100.0);
    for f in &out.forks {
        let _ = write!(s, "\n## Fork {}\n\n", f.fork);
        let heads: Vec<String> = spec.objectives.iter().map(|o| o.metric.clone()).collect();
        let _ = writeln!(s, "| # | candidate | {} | design | runner-ups |", heads.join(" | "));
        let _ = writeln!(s, "|---|---|{}---|---|", "---|".repeat(heads.len()));
        for (rank, (kept, under)) in f.merged.iter().enumerate() {
            let c = &out.cands[*kept];
            let runners: Vec<String> = under.iter().map(|u| format!("{} ({})", out.cands[*u].key, objective_cells(spec, &out.cands[*u].metrics()).join(", "))).collect();
            let _ = writeln!(s, "| {} | {} | {} | {} | {} |", rank + 1, c.key, objective_cells(spec, &c.metrics()).join(" | "), design_of.get(kept).cloned().unwrap_or_default(), if runners.is_empty() { "-".into() } else { runners.join("; ") });
        }
        let _ = writeln!(s, "\nBound (the ideal point: each objective's best optimistic value over every valid feasible candidate of the fork) and the gap to the front's best:\n");
        let _ = writeln!(s, "| objective | front's best | bound | gap |");
        let _ = writeln!(s, "|---|---|---|---|");
        for (a, o) in spec.objectives.iter().enumerate() {
            let (b, i) = (unorient(o.max, f.best[a]), unorient(o.max, f.ideal[a]));
            let gap = if i.abs() > 1e-12 && b.is_finite() && i.is_finite() { format!("{:.1}%", 100.0 * (f.ideal[a] - f.best[a]).abs() / i.abs()) } else { "-".into() };
            let _ = writeln!(s, "| {} {} | {} | {} | {gap} |", if o.max { "max" } else { "min" }, o.metric, show(&Value::exact(b)), show(&Value::exact(i)));
        }
        if !f.open.is_empty() {
            let _ = writeln!(s, "\n{} candidates were still open (undecided) when the search stopped; their bounds count in the gap.", f.open.len());
        }
        // `beside: max <metric>`: the best judged candidate of the fork on another metric, listed
        // when it is not on the front
        if let Some((dir, metric)) = spec.extra.get("beside").and_then(|b| b.split_once(' ')) {
            let sign = if dir == "min" { -1.0 } else { 1.0 };
            let best = out.cands.iter().enumerate().filter(|(_, c)| c.fork == f.fork && c.fate == Fate::Judged).filter_map(|(i, c)| c.metrics().get(metric.trim()).map(|v| (i, sign * v.v))).max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = best
                && !f.members.contains(&i)
            {
                let c = &out.cands[i];
                let _ = writeln!(s, "\nBeside the front, the best on {dir} {metric}: {} ({}).", c.key, objective_cells(spec, &c.metrics()).join(", "));
            }
        }
        for (kept, _) in &f.merged {
            let c = &out.cands[*kept];
            let _ = writeln!(s, "\n### {}\n", c.key);
            let m = c.metrics();
            for row in cards {
                let _ = writeln!(s, "- {}", render(row, &m, &c.lines));
            }
            if !cards.is_empty() {
                s.push('\n');
            }
            for (k, v) in &m {
                let _ = writeln!(s, "- {k}: {}", show(v));
            }
            for (k, t) in c.lines.iter().filter(|(k, _)| k.as_str() != "design_text") {
                let _ = writeln!(s, "- {k}: {t}");
            }
        }
    }
    s
}

fn summary_md<F: Farm>(farm: &F, spec: &Spec, out: &Outcome<F::Cand>) -> String {
    let mut s = format!("# {} {}: search summary\n\nGenerated by `mctas farmopt run`; do not edit. Evaluator {}; {} candidates; {} quick evaluations and {} full judges this run; {:.0} s.\n\n", spec.farm, spec.stage, farm.version(), out.cands.len(), out.quick_runs, out.full_runs, out.seconds);
    for f in &out.forks {
        let census = crate::search::census(&out.cands, &f.fork);
        let _ = writeln!(s, "## Fork {}\n", f.fork);
        for (k, n) in &census {
            let _ = writeln!(s, "- {k}: {n}");
        }
        let _ = writeln!(s, "- exact front: {} members; merged: {}\n", f.members.len(), f.merged.len());
        let mut reasons: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for c in out.cands.iter().filter(|c| c.fork == f.fork) {
            if let Fate::Invalid(w) | Fate::Infeasible(w) = &c.fate {
                let head: String = w.chars().take(90).collect();
                *reasons.entry(head).or_default() += 1;
            }
        }
        if !reasons.is_empty() {
            let mut v: Vec<(String, usize)> = reasons.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            let _ = writeln!(s, "Why candidates left (top 10):\n");
            for (w, n) in v.into_iter().take(10) {
                let _ = writeln!(s, "- {n} x {w}");
            }
            s.push('\n');
        }
    }
    s
}

fn ranked_tsv<F: Farm>(farm: &F, spec: &Spec, out: &Outcome<F::Cand>) -> String {
    let params: Vec<&'static str> = farm.params().iter().map(|p| p.name).collect();
    let objs: Vec<String> = spec.objectives.iter().map(|o| o.metric.clone()).collect();
    let mut s = format!("key\tfork\tlevel\tfate\t{}\t{}\t{}\n", objs.join("\t"), objs.iter().map(|o| format!("bound {o}")).collect::<Vec<_>>().join("\t"), params.join("\t"));
    // the measured candidates (quick or full) only: the rest are counted in summary.md (a space of
    // millions would not fit a repository file)
    let mut order: Vec<usize> = (0..out.cands.len()).filter(|&i| out.cands[i].quick.is_some() || out.cands[i].full.is_some()).collect();
    let key = |c: &Cand<F::Cand>| crate::pareto::oriented(&spec.objectives, &c.metrics(), crate::pareto::Pick::Mean).unwrap_or_else(|| vec![f64::NEG_INFINITY; objs.len()]);
    order.sort_by(|&a, &b| key(&out.cands[b]).partial_cmp(&key(&out.cands[a])).unwrap_or(std::cmp::Ordering::Equal).then(out.cands[a].key.cmp(&out.cands[b].key)));
    for i in order {
        let c = &out.cands[i];
        let m = c.metrics();
        let b = c.optimistic();
        let _ = writeln!(
            s,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c.key,
            c.fork,
            c.level().name(),
            why(c),
            objective_cells(spec, &m).join("\t"),
            spec.objectives.iter().map(|o| b.get(&o.metric).map(|v| show(&Value::exact(if o.max { v.hi } else { v.lo }))).unwrap_or_else(|| "-".into())).collect::<Vec<_>>().join("\t"),
            params.iter().map(|p| farm.param(&c.c, p).unwrap_or_default()).collect::<Vec<_>>().join("\t")
        );
    }
    s
}
