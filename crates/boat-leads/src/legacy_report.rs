//! A known, already-hidden negative is evidence, not a valid clock and not a skipped test.
//! Every name/result/build count must still be present; new or changed failures are errors.
use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;

/// The legacy runner explicitly requests these positions and leaves all four rotations on.
pub(crate) const POSITIONS: usize = 4;
const ROTATIONS: usize = 4;

fn output_cell(reason: &str) -> Result<[i32; 3]> {
    let tail = reason.strip_prefix("the output cell [").context("not a non-free-output failure")?;
    let (cell, detail) = tail.split_once(']').context("output coordinate")?;
    let xyz: Vec<i32> = cell.split(',').map(|n| n.trim().parse()).collect::<std::result::Result<_, _>>()?;
    ensure!(xyz.len() == 3, "invalid output coordinates");
    let detail = detail.trim_start();
    ensure!((detail.starts_with("holds ") && detail.contains(": it must be an AIR cell"))
        || detail.starts_with("is entered during the cycle "), "unexpected output failure: {detail}");
    Ok([xyz[0], xyz[1], xyz[2]])
}

fn expected(names: &[String], baseline: &str, hidden: &BTreeMap<String, String>) -> Result<BTreeMap<String, Option<[i32; 3]>>> {
    let mut expected: BTreeMap<_, _> = names.iter().map(|n| (n.clone(), None)).collect();
    ensure!(expected.len() == names.len(), "duplicate inventory name");
    let mut summary = None;
    for line in baseline.lines().filter(|l| !l.trim().is_empty() && !l.starts_with(char::is_whitespace)) {
        if let Some(tail) = line.strip_prefix("INVALID ") {
            let (name, reason) = tail.split_once(": ").context("invalid baseline line")?;
            let slot = expected.get_mut(name).with_context(|| format!("unknown baseline name {name}"))?;
            ensure!(slot.is_none(), "duplicate negative {name}");
            let cell = output_cell(reason)?;
            let why = hidden.get(name).with_context(|| format!("negative {name} is not in registry"))?;
            ensure!(why.starts_with("no output:") && why.contains(&format!("the output cell {cell:?}")),
                "negative {name} was not already hidden for that output");
            *slot = Some(cell);
        } else if line.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            ensure!(summary.replace(line).is_none(), "multiple baseline summaries");
        } else {
            bail!("unexpected baseline outcome: {line}");
        }
    }
    let negative = expected.values().filter(|v| v.is_some()).count();
    let summary = summary.context("missing baseline summary")?;
    ensure!(summary.starts_with(&format!("{} of {} designs: ", names.len() - negative, names.len()))
        && summary.contains(&format!("(0 differ, {negative} not run to a line)")), "baseline summary mismatch: {summary}");
    Ok(expected)
}

struct Section {
    valid: bool,
    reason: String,
    lines: Vec<String>,
}

fn sections(game: &str) -> Result<BTreeMap<String, Section>> {
    let mut sections = BTreeMap::new();
    let mut current = None;
    for line in game.lines() {
        if let Some((valid, tail)) = line.strip_prefix("CLOCK: ").map(|s| (true, s))
            .or_else(|| line.strip_prefix("INVALID: ").map(|s| (false, s))) {
            let (name, reason) = tail.split_once(' ').context("game result without details")?;
            ensure!(sections.insert(name.into(), Section { valid, reason: reason.into(), lines: Vec::new() }).is_none(), "duplicate game result {name}");
            current = Some(name.to_string());
        } else if !line.trim().is_empty() {
            ensure!(line.starts_with(char::is_whitespace), "unexpected game output: {line}");
            let name = current.as_ref().context("game detail before result")?;
            sections.get_mut(name).unwrap().lines.push(line.into());
        }
    }
    Ok(sections)
}

/// Returns (positive records, known-negative records, actual builds). Static occupancy and
/// later entity entry are the same *non-free output* category, but both full reasons stay
/// in the raw report; no other category, coordinate, or newly hidden failure is accepted.
pub fn check(names: &[String], baseline: &str, game: &str, hidden: &BTreeMap<String, String>, robustness: &BTreeMap<String, String>) -> Result<(usize, usize, usize)> {
    let expected = expected(names, baseline, hidden)?;
    let actual = sections(game)?;
    ensure!(actual.len() == expected.len(), "{} game records for {} expected names", actual.len(), expected.len());
    let (mut valid, mut negative, mut builds) = (0, 0, 0);
    for (name, failure_cell) in expected {
        let section = actual.get(&name).with_context(|| format!("missing game result {name}"))?;
        let counts = section.lines.iter().find_map(|l| l.split_once(" builds periodic of ")).context("missing build count")?;
        let periodic: usize = counts.0.split_whitespace().last().context("periodic count")?.parse()?;
        let total: usize = counts.1.split_whitespace().next().context("total count")?.parse()?;
        ensure!(total > 0 && periodic <= total, "invalid build counts for {name}");
        let expected_total = POSITIONS * ROTATIONS;
        ensure!(total == expected_total, "wrong build grid for {name}: expected {expected_total} builds ({POSITIONS} positions x {ROTATIONS} rotations), got {total}");
        builds += total;
        match failure_cell {
            None => {
                let declared = robustness.get(&name).with_context(|| format!("missing robustness declaration for {name}"))?;
                ensure!(matches!(declared.as_str(), "robust" | "rotation-dependent" | "position-dependent"), "unknown registered robustness {declared}");
                let observed = section.reason.rsplit_once(", ").context("missing game robustness")?.1;
                ensure!(observed == declared, "robustness changed for {name}: {declared} -> {observed}");
                ensure!(section.valid && periodic > 0 && (declared != "robust" || periodic == total), "new failure or nonperiodic robust build for {name}");
                ensure!(section.lines.iter().any(|l| l.starts_with("  redsim AGREES (")), "missing exact model agreement for {name}");
                if declared != "robust" { println!("DECLARED DEPENDENCY: {name}: {periodic}/{total} periodic builds, {declared}"); }
                valid += 1;
            }
            Some(cell) => {
                ensure!(!section.valid && periodic == 0, "known negative {name} unexpectedly changed verdict");
                ensure!(output_cell(&section.reason)? == cell, "changed failure cell for {name}");
                negative += 1;
                println!("KNOWN NEGATIVE (already hidden): {name}: {}", section.reason);
            }
        }
    }
    println!("GAME EVIDENCE: {valid} positive + {negative} already-hidden negative records, {builds} builds; no records omitted");
    Ok((valid, negative, builds))
}

#[cfg(test)]
mod tests {
    use super::*;
    const WHY: &str = "the output cell [0, 1, -1] holds stone: it must be an AIR cell";
    fn declared() -> BTreeMap<String, String> { BTreeMap::from([("good".into(), "robust".into()), ("bad".into(), "robust".into())]) }
    fn inputs() -> (Vec<String>, String, String, BTreeMap<String, String>) {
        let names = vec!["good".into(), "bad".into()];
        let baseline = format!("INVALID bad: {WHY}\n\n1 of 2 designs: redsim prints the judge's line (0 differ, 1 not run to a line)\n");
        let game = format!("CLOCK: good period 6 gt, robust\n  self-start; 16 builds periodic of 16 (1.0 s)\n  redsim AGREES (period, warmup)\nINVALID: bad {WHY}\n  self-start; 0 builds periodic of 16 (1.0 s)\n");
        let hidden = BTreeMap::from([("bad".into(), format!("no output: {WHY}"))]);
        (names, baseline, game, hidden)
    }
    #[test]
    fn retains_positive_and_declared_negative() {
        let (n, b, g, h) = inputs();
        assert_eq!(check(&n, &b, &g, &h, &declared()).unwrap(), (1, 1, 32));
    }
    #[test]
    fn rejects_changed_missing_duplicate_and_unhidden_failures() {
        let (n, b, g, h) = inputs();
        assert!(check(&n, &b, &g.replace("[0, 1, -1]", "[1, 1, -1]"), &h, &declared()).is_err());
        assert!(check(&n, &b, &g.replace(WHY, "timeout waiting for tick"), &h, &declared()).is_err());
        assert!(check(&n, &b, g.split("INVALID:").next().unwrap(), &h, &declared()).is_err());
        assert!(check(&n, &b, &(g.clone() + &g), &h, &declared()).is_err());
        assert!(check(&n, &b, &g, &BTreeMap::new(), &declared()).is_err());
        assert!(check(&n, &b, &g.replace("16 builds", "15 builds"), &h, &declared()).is_err());
        assert!(check(&n, &b, &g.replace("CLOCK: good", "INVALID: good"), &h, &declared()).is_err());
    }
    #[test]
    fn rejects_reduced_robust_grid() {
        let (n, b, g, h) = inputs();
        let g = g.replace("16 builds periodic of 16", "1 builds periodic of 1");
        assert!(check(&n, &b, &g, &h, &declared()).is_err());
    }
    #[test]
    fn rejects_reduced_dependent_grid() {
        let (n, b, g, h) = inputs();
        let g = g.replace(", robust", ", rotation-dependent")
            .replace("16 builds periodic of 16", "8 builds periodic of 8");
        let mut r = declared();
        r.insert("good".into(), "rotation-dependent".into());
        assert!(check(&n, &b, &g, &h, &r).is_err());
    }
    #[test]
    fn rejects_reduced_negative_grid() {
        let (n, b, g, h) = inputs();
        let g = g.replace("0 builds periodic of 16", "0 builds periodic of 1");
        assert!(check(&n, &b, &g, &h, &declared()).is_err());
    }
    #[test]
    fn dependent_is_allowed_only_when_already_declared_and_not_promoted() {
        let (n, b, g, h) = inputs();
        let g = g.replace(", robust", ", rotation-dependent").replace("16 builds", "8 builds");
        let mut r = declared();
        r.insert("good".into(), "rotation-dependent".into());
        assert_eq!(check(&n, &b, &g, &h, &r).unwrap(), (1, 1, 32));
        assert!(check(&n, &b, &g, &h, &declared()).is_err());
        assert!(check(&n, &b, &g.replace(", rotation-dependent", ", robust"), &h, &r).is_err());
        assert!(check(&n, &b, &g.replace("8 builds", "0 builds"), &h, &r).is_err());
    }
}
