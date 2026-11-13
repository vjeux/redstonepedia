//! (9fc3155f, ironspec Nov 12 23:56: NO VACUOUS PASSES, every tool) Every bar prints its sample
//! count; a bar with 0 samples reads "NOT JUDGED (0 samples)" and fails the run unless the run's
//! declared scope excludes it: the design's `# scope-excludes: BAR=REASON; ...` (comment or
//! meta-line form) or a tool's `--scope-excludes BAR=REASON` flag, BAR the bar's short name as
//! the tool prints it.

/// (G1 Nov 13 12:52 (a)) Every bar a mobfarm tool judges, by the short name it prints: an
/// exclusion naming any other bar is refused when the scope is read (a typo or a stale name can
/// never quietly exclude nothing). A new bar is added here with its tool.
pub const BARS: &[&str] = &[
    "caller-sight",
    "gate",
    "late-paths",
    "lightning",
    "lightning-rods",
    "one-way",
    "pickup",
    "push-range",
    "rail-junctions",
    "rail-T",
    "sensor",
    "sight",
    "staleness",
    "suffocation",
    "swept-box",
    "switched-cell",
    "T-static",
    "water-rules",
    "wet-spawn",
];

/// The bars a run's declared scope excludes, each with its reason.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub excluded: Vec<(String, String)>,
    /// Whether each exclusion came from the tool's own `--scope-excludes` flag (one tool's run)
    /// rather than the design's `# scope-excludes:` line (shared by every tool).
    from_flag: Vec<bool>,
    /// (ironspec Nov 13 12:52 (a) on d9ec61312) the bars this run judged (every `verdict` call).
    seen: std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl Scope {
    /// The design text's `scope-excludes:` entries plus the flag's; an entry naming no bar of
    /// [`BARS`] is refused.
    pub fn from_text(text: &str, flags: &[String]) -> Result<Scope, String> {
        let mut entries: Vec<(String, bool)> = flags.iter().map(|f| (f.clone(), true)).collect();
        for l in text.lines() {
            let t = l.trim_start();
            let t = t.strip_prefix('#').map(str::trim_start).unwrap_or(t);
            if let Some(rest) = t.strip_prefix("scope-excludes:") {
                entries.extend(rest.split(';').map(str::trim).filter(|e| !e.is_empty()).map(|e| (e.to_string(), false)));
            }
        }
        let mut excluded = Vec::new();
        let mut from_flag = Vec::new();
        for (e, flag) in entries {
            let (bar, why) = e.split_once('=').ok_or_else(|| format!("scope-excludes {e}: BAR=REASON"))?;
            let (bar, why) = (bar.trim(), why.trim());
            if why.is_empty() {
                return Err(format!("scope-excludes {e}: a reason is required"));
            }
            if !BARS.contains(&bar) {
                return Err(format!("scope-excludes {bar}: no tool judges a bar of this name (G1 12:52 (a): an unknown exclusion fails; the bars: {})", BARS.join(", ")));
            }
            excluded.push((bar.to_string(), why.to_string()));
            from_flag.push(flag);
        }
        Ok(Scope { excluded, from_flag, seen: Default::default() })
    }

    /// (ironspec Nov 13 12:52 (a)) Each declared exclusion whose bar this run never judged, and
    /// whether the run fails on them: a `--scope-excludes` flag names one tool's run, so a flag
    /// exclusion this tool never judged FAILS; the design's line is shared by every tool, so its
    /// entries for another tool's bars are printed only.
    pub fn unused(&self) -> (Vec<String>, bool) {
        let seen = self.seen.borrow();
        let mut lines = Vec::new();
        let mut fail = false;
        for ((b, why), flag) in self.excluded.iter().zip(&self.from_flag) {
            if seen.contains(b) {
                continue;
            }
            if *flag {
                fail = true;
                lines.push(format!("scope-excludes {b}: UNUSED (the flag names a bar this tool did not judge: FAIL; its reason: {why})"));
            } else {
                lines.push(format!("scope-excludes {b}: not judged by this tool (the design's line, for another tool's bar; its reason: {why})"));
            }
        }
        (lines, fail)
    }

    /// The verdict of the bar `bar` judged over `samples` (named by `unit`, e.g. "rider places")
    /// with `fails` failures: the text to print and whether the run fails on it.
    pub fn verdict(&self, bar: &str, samples: usize, unit: &str, fails: usize) -> (String, bool) {
        self.seen.borrow_mut().insert(bar.to_string());
        if samples == 0 {
            // (G1 Nov 13 12:52, R1: an exclusion covers only an EMPTY bar: a failure with 0 samples
            // (a finding that needs no sample, a rod's fire with no farm entity) always fails)
            if fails > 0 {
                return (format!("NOT JUDGED (0 {unit}) with {fails} failure(s): FAIL (an exclusion never covers a failure)"), true);
            }
            return match self.excluded.iter().find(|(b, _)| b == bar) {
                Some((_, why)) => (format!("NOT JUDGED (0 {unit}; excluded by the declared scope: {why})"), false),
                None => (format!("NOT JUDGED (0 {unit}): FAIL"), true),
            };
        }
        if fails == 0 { (format!("{samples} {unit}: PASS"), false) } else { (format!("{samples} {unit}: FAIL ({fails})"), true) }
    }
}

#[cfg(test)]
mod tests {
    use super::Scope;

    /// 0 samples fail unless the scope names the bar; samples print with the verdict.
    #[test]
    fn no_vacuous_pass() {
        let s = Scope::from_text("# scope-excludes: one-way=no dust in this part (a rail-only module)\nname: x\n", &[]).unwrap();
        assert_eq!(s.verdict("one-way", 0, "dust cells", 0), ("NOT JUDGED (0 dust cells; excluded by the declared scope: no dust in this part (a rail-only module))".to_string(), false));
        assert_eq!(s.verdict("rail-T", 0, "Ts", 0), ("NOT JUDGED (0 Ts): FAIL".to_string(), true));
        assert_eq!(s.verdict("rail-T", 12, "Ts", 0), ("12 Ts: PASS".to_string(), false));
        assert_eq!(s.verdict("rail-T", 12, "Ts", 1), ("12 Ts: FAIL (1)".to_string(), true));
        assert!(Scope::from_text("scope-excludes: rail-T=\n", &[]).is_err());
        // (G1 12:52 (a): an exclusion naming no bar any tool judges is refused, a misspelt one too)
        let e = Scope::from_text("# scope-excludes: one-way=no dust; rail-Tz=a misspelt bar\n", &[]).unwrap_err();
        assert!(e.contains("rail-Tz: no tool judges a bar of this name"), "{e}");
        // (the design's line is shared: its entry for another tool's bar prints, never fails; the
        // tool's own flag naming a bar it did not judge FAILS)
        let s2 = Scope::from_text("# scope-excludes: one-way=no dust; rail-junctions=no junction in this part\n", &["sight=no pillager in this part".to_string()]).unwrap();
        let _ = s2.verdict("one-way", 0, "dust cells", 0);
        let (u, fail) = s2.unused();
        assert!(fail && u.len() == 2 && u.iter().any(|l| l.starts_with("scope-excludes sight: UNUSED")) && u.iter().any(|l| l.starts_with("scope-excludes rail-junctions: not judged by this tool")), "{u:?}");
        let s3 = Scope::from_text("# scope-excludes: rail-junctions=no junction in this part\n", &[]).unwrap();
        let _ = s3.verdict("one-way", 4, "dust cells", 0);
        assert!(!s3.unused().1, "a design-line entry for another tool's bar never fails");
        // (G1 12:52 R1: an excluded bar with 0 samples and a failure FAILS)
        assert!(s.verdict("one-way", 0, "dust cells", 1).1, "an exclusion never covers a failure");
    }

    /// Every bar name a tool passes to `verdict` is in [`super::BARS`] (a bar missing there could
    /// never be excluded): the CLI's verdict calls, read from its source.
    #[test]
    fn every_verdict_bar_is_registered() {
        for src in [include_str!("cli.rs")] {
            for (i, _) in src.match_indices("verdict(\"") {
                let rest = &src[i + "verdict(\"".len()..];
                let name = &rest[..rest.find('"').unwrap_or(0)];
                assert!(super::BARS.contains(&name), "the bar {name} is judged but not in BARS");
            }
        }
    }
}
