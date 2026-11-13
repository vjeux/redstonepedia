//! (9fc3155f, ironspec Nov 12 23:56: NO VACUOUS PASSES, every tool) Every bar prints its sample
//! count; a bar with 0 samples reads "NOT JUDGED (0 samples)" and fails the run unless the run's
//! declared scope excludes it: the design's `# scope-excludes: BAR=REASON; ...` (comment or
//! meta-line form) or a tool's `--scope-excludes BAR=REASON` flag, BAR the bar's short name as
//! the tool prints it.

/// The bars a run's declared scope excludes, each with its reason.
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub excluded: Vec<(String, String)>,
}

impl Scope {
    /// The design text's `scope-excludes:` entries plus the flag's.
    pub fn from_text(text: &str, flags: &[String]) -> Result<Scope, String> {
        let mut entries: Vec<String> = flags.to_vec();
        for l in text.lines() {
            let t = l.trim_start();
            let t = t.strip_prefix('#').map(str::trim_start).unwrap_or(t);
            if let Some(rest) = t.strip_prefix("scope-excludes:") {
                entries.extend(rest.split(';').map(str::trim).filter(|e| !e.is_empty()).map(str::to_string));
            }
        }
        let mut excluded = Vec::new();
        for e in entries {
            let (bar, why) = e.split_once('=').ok_or_else(|| format!("scope-excludes {e}: BAR=REASON"))?;
            let why = why.trim();
            if why.is_empty() {
                return Err(format!("scope-excludes {e}: a reason is required"));
            }
            excluded.push((bar.trim().to_string(), why.to_string()));
        }
        Ok(Scope { excluded })
    }

    /// The verdict of the bar `bar` judged over `samples` (named by `unit`, e.g. "rider places")
    /// with `fails` failures: the text to print and whether the run fails on it.
    pub fn verdict(&self, bar: &str, samples: usize, unit: &str, fails: usize) -> (String, bool) {
        if samples == 0 {
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
    }
}
