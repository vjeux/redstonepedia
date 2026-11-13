//! (ironspec Nov 13 04:19 PT, G1's checklist; the chain fix) One failure list per tool: every judged bar, every
//! positive control and every override goes through a [`Verdict`], and the tool's process exits non-zero on any
//! miss. A bar over an empty sample set is NOT JUDGED and fails, unless the run's declared scope excludes it (printed
//! by [`Verdict::scope`]); a control fails unless it bites; an override (a forced input, an injected event, a stand-in,
//! an emulated line) prints its target's state before -> after and fails when it leaves the target unchanged.
//! `--report-only` prints the misses without the non-zero exit; it is never used for a cited result (ironspec 05:09).

/// The tool's failure list.
#[derive(Debug, Default)]
pub struct Verdict {
    pub tool: String,
    fails: Vec<String>,
    judged: usize,
    controls: usize,
    overrides: usize,
    scoped: Vec<String>,
}

impl Verdict {
    pub fn new(tool: &str) -> Self {
        Verdict { tool: tool.to_string(), ..Default::default() }
    }

    /// A judged bar: "pass", or "FAIL" with the miss recorded under `name`.
    pub fn bar(&mut self, name: &str, ok: bool) -> &'static str {
        self.judged += 1;
        if ok {
            "pass"
        } else {
            self.fails.push(name.to_string());
            "FAIL"
        }
    }

    /// A judged bar over `n` samples: 0 samples is NOT JUDGED and fails.
    pub fn bar_n(&mut self, name: &str, n: usize, ok: bool) -> String {
        if n == 0 {
            self.judged += 1;
            self.fails.push(format!("{name}: NOT JUDGED (0 samples)"));
            "NOT JUDGED (0 samples): FAIL".to_string()
        } else {
            self.bar(name, ok).to_string()
        }
    }

    /// A positive control: it must bite (and must have run: `n` its samples).
    pub fn control(&mut self, name: &str, n: usize, bites: bool) -> &'static str {
        self.controls += 1;
        if n == 0 {
            self.fails.push(format!("the control {name}: NOT JUDGED (0 samples)"));
            "NOT JUDGED (0 samples): FAIL"
        } else if bites {
            "the control bites"
        } else {
            self.fails.push(format!("the control {name} does not bite"));
            "FAIL: the control does not bite"
        }
    }

    /// An override: the target's state before -> after; an unchanged target fails the run (ironspec 04:07 PT).
    pub fn over(&mut self, what: &str, before: &str, after: &str) -> String {
        self.overrides += 1;
        if before == after {
            self.fails.push(format!("the override {what} leaves its target {before}"));
            format!("{what}: {before} -> {after} (FAIL: unchanged)")
        } else {
            format!("{what}: {before} -> {after}")
        }
    }

    /// A count the run's declared scope excludes from judgement: printed with its reason, never silently passed.
    pub fn scope(&mut self, what: &str, why: &str) -> String {
        self.scoped.push(format!("{what} ({why})"));
        format!("{what}: not judged by this run's declared scope ({why})")
    }

    /// A miss recorded directly (a model error, an unparsable value, a run that errored).
    pub fn fail(&mut self, what: &str) {
        self.judged += 1;
        self.fails.push(what.to_string());
    }

    pub fn fails(&self) -> &[String] {
        &self.fails
    }

    /// The run's verdict line; a tool that judged nothing fails (a report mode declares its scope instead).
    pub fn line(&self) -> String {
        if self.judged == 0 && self.controls == 0 && self.fails.is_empty() {
            return format!("=> {}: THE RUN'S VERDICT: NOTHING JUDGED (FAIL: a run must judge at least one bar)", self.tool);
        }
        let scope = if self.scoped.is_empty() { String::new() } else { format!("; out of scope, printed: {}", self.scoped.join("; ")) };
        if self.fails.is_empty() {
            format!("=> {}: THE RUN'S VERDICT: {} judged bar(s) pass, {} control(s) bite, {} override(s) change their targets{scope}", self.tool, self.judged, self.controls, self.overrides)
        } else {
            format!("=> {}: THE RUN'S VERDICT: {} judged miss(es): {}{scope}", self.tool, self.fails.len(), self.fails.join("; "))
        }
    }

    /// Print the verdict line and fail the process on any miss (or on nothing judged) unless `report_only`, whose
    /// output is marked not citable (G1's audit, hole 29).
    pub fn finish(&self, report_only: bool) -> Result<(), String> {
        println!("{}{}", if report_only { "REPORT-ONLY (not citable, ironspec 05:09 PT): " } else { "" }, self.line());
        let nothing = self.judged == 0 && self.controls == 0 && self.fails.is_empty();
        if (!self.fails.is_empty() || nothing) && !report_only {
            return Err(format!("{}: {} judged miss(es){} (--report-only prints them without failing the run; never for a cited result)", self.tool, self.fails.len(), if nothing { ", nothing judged" } else { "" }));
        }
        Ok(())
    }
}

/// (G1's checklist (4)) A list argument parsed strictly: exactly `n` values, or none when `optional`.
pub fn exact<T: Copy>(name: &str, v: &[T], n: usize, optional: bool) -> Result<Option<Vec<T>>, String> {
    if v.is_empty() && optional {
        return Ok(None);
    }
    if v.len() != n {
        return Err(format!("--{name} takes exactly {n} value(s), got {}", v.len()));
    }
    Ok(Some(v.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sets_fail_and_controls_must_bite() {
        let mut v = Verdict::new("t");
        assert_eq!(v.bar_n("x", 0, true), "NOT JUDGED (0 samples): FAIL");
        assert_eq!(v.control("c", 3, false), "FAIL: the control does not bite");
        assert_eq!(v.control("d", 0, true), "NOT JUDGED (0 samples): FAIL");
        assert_eq!(v.fails().len(), 3);
        assert!(v.finish(false).is_err());
        assert!(v.finish(true).is_ok());
    }

    #[test]
    fn an_unchanged_override_fails_and_nothing_judged_fails() {
        let mut v = Verdict::new("t");
        assert!(v.over("a", "air", "glass").ends_with("glass"));
        assert!(v.fails().is_empty());
        let empty = Verdict::new("e");
        assert!(empty.finish(false).is_err());
        v.over("b", "glass", "glass");
        assert_eq!(v.fails().len(), 1);
    }

    #[test]
    fn passing_bars_pass_and_exact_lists_are_strict() {
        let mut v = Verdict::new("t");
        assert_eq!(v.bar("a", true), "pass");
        assert_eq!(v.bar_n("b", 5, true), "pass");
        assert!(v.finish(false).is_ok());
        assert!(exact("x", &[1, 2], 3, true).is_err());
        assert!(exact::<i64>("x", &[], 3, true).unwrap().is_none());
        assert!(exact::<i64>("x", &[], 3, false).is_err());
        assert_eq!(exact("x", &[1, 2, 3], 3, false).unwrap(), Some(vec![1, 2, 3]));
    }
}
