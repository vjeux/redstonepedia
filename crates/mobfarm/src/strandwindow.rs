//! (ironspec 06:46 / 06:59 / G1) R1's strand window from a visit file: per visit, the window in which
//! a night skip strands the napper (its stamp armed and the leg's partial-path sink still running at
//! the skip, that sink's stop coming at least `lag` gt later, so REST, lingering behind Brain's
//! schedule gate, can't re-set HOME first), and P(strand | a skip) = the mean window over the visit
//! spacing at the stop. Every input printed with its validation: the per-row release, the stamp's
//! age at the release, the first COMPLETE path compute after it (the sink's stop and restart), the
//! implied sink duration (the age + that offset, against the game's 150 + U(0..100)), and where the
//! window ends.
//!
//! The visit file is b8949c96's `way-cell --visit` output: per visit a line `(visit) phase +k, hold h:
//! summoned at game time G (...), released before game time R (...); the stamp at the release ARMED
//! a gt old | cleared; the paths the ride set: release + t at Some([x, y, z]): COMPLETE|PARTIAL
//! (...)[ [past the entry]]; ...` and a second line with `at the entry +E gt after the release`.

/// One visit's inputs.
#[derive(Debug, Clone, Default)]
pub struct Visit {
    pub phase: i64,
    pub hold: i64,
    /// The stamp's age at the release (None: cleared at the release).
    pub armed: Option<i64>,
    /// Every path compute after the release: (offset, complete, past the entry).
    pub paths: Vec<(i64, bool, bool)>,
    /// The cart at the stop's entry, gt after the release.
    pub entry: Option<i64>,
}

fn num_after(s: &str, key: &str) -> Option<i64> {
    let i = s.find(key)? + key.len();
    let t: String = s[i..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    t.parse().ok()
}

/// Parse a visit file's rows.
pub fn parse(text: &str) -> Vec<Visit> {
    let mut out: Vec<Visit> = Vec::new();
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("(visit) phase +") else { continue };
        let phase = rest.split(',').next().and_then(|x| x.trim().parse().ok()).unwrap_or(-1);
        let hold = num_after(l, "hold ").unwrap_or(-1);
        if l.contains("summoned at game time") {
            let mut v = Visit { phase, hold, ..Default::default() };
            v.armed = if l.contains("the stamp at the release ARMED") { num_after(l, "the stamp at the release ARMED") } else { None };
            // (the computes: `release + t at Some(..): COMPLETE|PARTIAL (..)[ [past the entry]]`,
            // each entry its own chunk between `release + ` markers)
            if let Some(i) = l.find("the paths the ride set: ") {
                for chunk in l[i + "the paths the ride set: ".len()..].split("release + ").skip(1) {
                    let off: i64 = match chunk.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse() {
                        Ok(o) => o,
                        Err(_) => continue,
                    };
                    let complete = chunk.contains("): COMPLETE");
                    if !complete && !chunk.contains("): PARTIAL") {
                        continue;
                    }
                    v.paths.push((off, complete, chunk.contains("[past the entry]")));
                }
            }
            out.push(v);
        } else if let Some(e) = num_after(l, "at the entry +")
            && let Some(v) = out.iter_mut().rev().find(|v| v.phase == phase && v.hold == hold && v.entry.is_none())
        {
            v.entry = Some(e);
        }
    }
    out
}

/// One visit's strand window (gt) and the offset its stamp's exposure ends at: from the release to
/// the first COMPLETE compute after it (the leg sink's stop), less `lag`; capped at the lie-down when
/// `lie_after_entry` is given (the stop's lie-down that many gt after the entry, (A)'s at the eject).
pub fn window(v: &Visit, lag: i64, lie_after_entry: Option<i64>) -> (i64, Option<i64>, &'static str) {
    if v.armed.is_none() {
        return (0, None, "cleared at the release");
    }
    let first_complete = v.paths.iter().find(|p| p.1).map(|p| p.0);
    let lie = lie_after_entry.and_then(|d| v.entry.map(|e| e + d));
    let (end, why) = match (first_complete, lie) {
        (Some(c), Some(l)) if l < c => (Some(l), "the lie-down"),
        (Some(c), _) => (Some(c), "the first complete compute"),
        (None, Some(l)) => (Some(l), "the lie-down"),
        (None, None) => (None, "no complete compute and no lie-down recorded"),
    };
    (end.map_or(0, |e| (e - lag).max(0)), end, why)
}

/// The report: every file's rows, each row's inputs (with --rows), the validation of the inputs and
/// the strand probability per skip for each spacing.
pub fn report(files: &[std::path::PathBuf], spacing: &[i64], lag: i64, lie_after_entry: Option<i64>, rows: bool) -> anyhow::Result<Vec<String>> {
    use std::collections::BTreeMap;
    let mut out = Vec::new();
    let mut all: Vec<(String, Visit)> = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(f)?;
        let vs = parse(&text);
        anyhow::ensure!(!vs.is_empty(), "{}: no `(visit) phase +k, hold h: summoned at ...` rows", f.display());
        out.push(format!("{}: {} visit rows", f.display(), vs.len()));
        all.extend(vs.into_iter().map(|v| (f.display().to_string(), v)));
    }
    let (mut armed, mut dur_in, mut dur_lo, mut dur_hi, mut no_entry) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut windows: Vec<i64> = Vec::new();
    let mut ends: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut partials: BTreeMap<i64, usize> = BTreeMap::new();
    let mut partial_rows = 0usize;
    let mut first_complete: BTreeMap<i64, usize> = BTreeMap::new();
    for (f, v) in &all {
        let (w, end, why) = window(v, lag, lie_after_entry);
        windows.push(w);
        if v.armed.is_some() {
            armed += 1;
            *ends.entry(why).or_default() += 1;
        }
        let fc = v.paths.iter().find(|p| p.1).map(|p| p.0);
        if let (Some(a), Some(c)) = (v.armed, fc) {
            *first_complete.entry(c).or_default() += 1;
            match a + c {
                d if (150..=250).contains(&d) => dur_in += 1,
                d if d < 150 => dur_lo += 1,
                _ => dur_hi += 1,
            }
        }
        let ps: Vec<i64> = v.paths.iter().filter(|p| !p.1).map(|p| p.0).collect();
        if !ps.is_empty() {
            partial_rows += 1;
        }
        for p in &ps {
            *partials.entry(*p).or_default() += 1;
        }
        if lie_after_entry.is_some() && v.entry.is_none() {
            no_entry += 1;
        }
        if rows {
            out.push(format!(
                "{f} phase +{} hold {}: the stamp at the release {}; computes after it {}; the first complete {}; the implied sink duration (the age + that offset) {}; the entry {}; window {w} gt (ends {}: {why})",
                v.phase,
                v.hold,
                v.armed.map_or("cleared".to_string(), |a| format!("ARMED {a} gt old")),
                v.paths.iter().map(|p| format!("+{} {}{}", p.0, if p.1 { "complete" } else { "PARTIAL" }, if p.2 { " (past the entry)" } else { "" })).collect::<Vec<_>>().join(", "),
                fc.map_or("none".to_string(), |c| format!("+{c}")),
                v.armed.zip(fc).map_or("-".to_string(), |(a, c)| format!("{}", a + c)),
                v.entry.map_or("-".to_string(), |e| format!("+{e}")),
                end.map_or("-".to_string(), |e| format!("+{e}")),
            ));
        }
    }
    let n = all.len();
    let mut aw: Vec<i64> = all.iter().zip(&windows).filter(|(v, _)| v.1.armed.is_some()).map(|(_, w)| *w).collect();
    aw.sort();
    let mean_all = windows.iter().sum::<i64>() as f64 / n.max(1) as f64;
    out.push(format!("inputs: {n} visits; the stamp ARMED at the release in {armed} ({:.1} %), cleared in {}", 100.0 * armed as f64 / n.max(1) as f64, n - armed));
    out.push(format!("validation, the implied sink duration (the stamp's age at the release + the first complete compute's offset) against the game's 150 + U(0..100): in 150..250 {dur_in}, under 150 {dur_lo} (a stop before the duration's end: a stuck check, or a restart at the seat), over 250 {dur_hi} (a later sink: a restart at the seat after a stuck stop)"));
    out.push(format!("validation, the first complete compute after the release (offset: rows): {first_complete:?}"));
    out.push(format!("validation, PARTIAL computes after the release: {partial_rows} rows; by offset {partials:?}"));
    if lie_after_entry.is_some() {
        out.push(format!("the lie-down at the entry + {} gt; rows with no entry recorded: {no_entry} (their window ends at the first complete compute)", lie_after_entry.unwrap_or(0)));
    }
    out.push(format!("each armed row's window ends at: {ends:?}"));
    out.push(format!(
        "the window (from the release to the end, less the {lag}-gt lag), armed rows: min {}, median {}, max {}; the mean over ALL visits {mean_all:.2} gt",
        aw.first().copied().unwrap_or(0),
        aw.get(aw.len() / 2).copied().unwrap_or(0),
        aw.last().copied().unwrap_or(0)
    ));
    for s in spacing {
        out.push(format!("P(strand | a night skip) at a visit spacing of {s} gt: {mean_all:.2} / {s} = {:.4} ({:.2} %)", mean_all / *s as f64, 100.0 * mean_all / *s as f64));
    }
    // (G1's C18: the 0-gt windows counted apart, and the validation judged, not printed only)
    let zero = windows.iter().filter(|w| **w == 0).count();
    out.push(format!("rows with a 0-gt window: {zero} (the stamp cleared at the release: {}; armed but its window inside the lag: {})", n - armed, zero.saturating_sub(n - armed)));
    if dur_lo + dur_hi > 0 {
        out.push(format!("FAILED: {} armed rows' implied sink duration outside the game's 150..250 (under {dur_lo}, over {dur_hi}): the rows don't fit the sink's model", dur_lo + dur_hi));
    }
    if lie_after_entry.is_some() && no_entry > 0 {
        out.push(format!("FAILED: {no_entry} rows with no entry recorded under --lie-after-entry"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_visit_row_and_its_window() {
        let t = "(visit) phase +1, hold 95: summoned at game time 201 (arrival detector at summon + 103), released before game time 399 (NOT-ready off); the stamp at the release ARMED 196 gt old; the paths the ride set: release + 40 at Some([8, -23, -1]): PARTIAL (2 nodes, target (-3,-26,3), first (7,-24,-1) (7,-28,0); walk target Block(BlockPos { x: -3, y: -27, z: 3 }); activity Rest); release + 253 at Some([1, -26, -5]): COMPLETE (14 nodes, target (-3,-26,3); walk target Block(BlockPos { x: -3, y: -27, z: 3 }); activity Rest) [past the entry]\n(visit) phase +1, hold 95: arrival at t 102; at the entry +83 gt after the release at 0.2111 b/t: the stamp none\n(visit) phase +0, hold 110: summoned at game time 200 (x), released before game time 413 (y); the stamp at the release cleared; the paths the ride set: release + 167 at Some([1, -26, -5]): COMPLETE (14 nodes) [past the entry]\n";
        let v = parse(t);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].armed, Some(196));
        assert_eq!(v[0].paths, vec![(40, false, false), (253, true, true)]);
        assert_eq!(v[0].entry, Some(83));
        assert_eq!(v[1].armed, None);
        // A6's own stop: the window runs to the first complete compute, less the lag
        assert_eq!(window(&v[0], 21, None).0, 253 - 21);
        // (A)'s lie-down 30 gt after the entry comes first
        assert_eq!(window(&v[0], 21, Some(30)).0, 83 + 30 - 21);
        assert_eq!(window(&v[1], 21, None).0, 0);
    }
}
