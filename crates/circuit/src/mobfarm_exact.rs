//! The mob farms' tick-exact gate (tickexact, vjeux 2026-11-08 14:19: "why are we using averages,
//! why don't we have tick perfect precision?"). Every `circuit verify` and `circuit archive` of a
//! mob farm plays run 0 of every build its card averages over (the card's 8 builds in the void, each
//! site in a terrain sample) on the deterministic bridge (the 26.3 server with every random seeded
//! as redsim seeds it) and on redsim, tick for tick, by `mctas mobfarm compare --build K --run I`:
//! every entity, block, container, memory and random state compared every game tick for the
//! judge's whole run (its days, its weather). Each build's result is printed in the verdict: exact
//! for N ticks, or its first difference with the lane that owns it.
//!
//! A ratchet, not a freeze (the parent, 2026-11-08 15:59): a design whose RECORDED exactness
//! ([`RECORD`], written by `circuit archive` and `mctas mobfarm exact --record`) says every build
//! was exact FAILS when any build now differs (a regression is red); a design not yet exact does
//! not fail on it, and the unmodified official server's statistical gate ([`crate::mobfarm_game`])
//! decides as before. Every exact farm stays exact; every other one shows the tick where it
//! diverges. The site shows none of it.
//!
//! Each compare is a child `mctas` (the compare lives in the mobfarm crate, which builds on this
//! one), at most `MCTAS_MAX_WORKERS` at once.

use crate::Circuit;
use anyhow::{Result, anyhow};

/// The recorded exactness of the mob farms, keyed by the design's content hash (the site does not
/// read it): `hash`, `file`, `exact` (yes/no), `verdict`, `recorded`.
pub const RECORD: &str = "designs/circuits/mobfarm/exact.tsv";

/// One compared run.
#[derive(Clone, Debug)]
pub struct ExactRun {
    pub build: u32,
    pub run: u32,
    /// Game ticks compared (the settle and the run).
    pub ticks: u64,
    /// The first difference ("t2790: level random"), or why the compare did not run.
    pub first: Option<String>,
}

impl ExactRun {
    pub fn passed(&self) -> bool {
        self.first.is_none() && self.ticks > 0
    }

    /// One line of the verdict's table.
    pub fn line(&self) -> String {
        match &self.first {
            None => format!("  tick exact: build {} run {}: identical to the 26.3 bridge for {} game ticks", self.build, self.run, self.ticks),
            Some(f) => format!("  tick exact: build {} run {}: first difference {f} ({})", self.build, self.run, owner(f)),
        }
    }
}

/// The lane that owns a first difference (the compare's "what"), as the parent routes them: the
/// mob layer and brains to mob-AI, spawning, weather, fire and the harness to tick-exact.
pub fn owner(first: &str) -> &'static str {
    let what = first.split_once(": ").map(|(_, w)| w).unwrap_or(first);
    if what.starts_with("level random") || what.starts_with("randValue") {
        "the level random: its call site with `mctas mobai farm-trace --at T`"
    } else if what.starts_with("weather") || what.starts_with("day time") || what.starts_with("game time") || what.starts_with("AFK player") || what.starts_with("the compare") {
        "tick-exact"
    } else if what.starts_with("block") || what.starts_with("container") {
        "the block's mechanic in redsim"
    } else if what.contains("presence") || what.contains("entered/left") {
        "spawning (tick-exact)"
    } else if what.starts_with("entity") {
        "mob-AI"
    } else {
        "unrouted"
    }
}

/// The gate's result over the compared runs.
#[derive(Clone, Debug, Default)]
pub struct ExactReport {
    pub runs: Vec<ExactRun>,
    pub days: u32,
    pub seconds: f64,
}

impl ExactReport {
    pub fn passed(&self) -> bool {
        !self.runs.is_empty() && self.runs.iter().all(|r| r.passed())
    }

    /// The design's exactness in one line: "tick-exact: 8/8 builds, 961600 ticks" or "first
    /// difference: build 2 run 0 t201: entity #5 zombie head (mob-AI); 2/8 builds exact".
    pub fn summary(&self) -> String {
        let ok = self.runs.iter().filter(|r| r.passed()).count();
        let ticks: u64 = self.runs.iter().map(|r| r.ticks).sum();
        // the earliest first difference over the runs
        match self.runs.iter().filter(|r| !r.passed()).min_by_key(|r| r.ticks) {
            None => format!("tick-exact: {ok}/{} builds, {ticks} ticks", self.runs.len()),
            Some(r) => {
                let f = r.first.as_deref().unwrap_or("?");
                format!("first difference: build {} run {} {f} ({}); {ok}/{} builds exact", r.build, r.run, owner(f), self.runs.len())
            }
        }
    }
}

/// The builds the judge runs (`mobfarm::judge`'s, in its order): 4 rotations x 2 positions in the
/// void; a terrain sample's sites.
pub fn builds(c: &Circuit, o: &crate::verify::Options) -> Result<u32> {
    if let Some(sample) = crate::mobfarm::terrain_sample(c).map_err(|e| anyhow!(e))? {
        let spec = crate::mobfarm::spec_of(c).map_err(|e| anyhow!(e))?;
        return Ok(crate::mobfarm::terrain_sites(c, &spec, sample).map_err(|e| anyhow!(e))?.len() as u32);
    }
    let turns = if o.rotations { 4 } else { 1 };
    Ok(turns * clock::offsets(o.positions.clamp(1, 2)).len() as u32)
}

/// Parse one compare's report: the ticks compared and the first difference.
pub fn parse_compare(out: &str) -> (u64, Option<String>) {
    if let Some(l) = out.lines().find(|l| l.starts_with("FIRST DIFFERENCE at game time ")) {
        // "FIRST DIFFERENCE at game time T (tick T of N compared): what"
        let t = l.trim_start_matches("FIRST DIFFERENCE at game time ").split(' ').next().unwrap_or("?").to_string();
        let what = l.split_once("): ").map(|(_, w)| w).unwrap_or("").to_string();
        let ticks = t.parse::<u64>().map(|t| t.saturating_sub(1)).unwrap_or(0);
        return (ticks, Some(format!("t{t}: {what}")));
    }
    if let Some(l) = out.lines().find(|l| l.ends_with(" ticks compared: 0 differences")) {
        let n = l.split(' ').next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
        return (n, None);
    }
    let tail: Vec<&str> = out.lines().rev().take(3).collect();
    (0, Some(format!("the compare did not run: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | "))))
}

/// Compare `runs` judge runs of every build on the bridge (`days` game days each).
pub fn gate(c: &Circuit, o: &crate::verify::Options, runs: u32, days: u32) -> Result<ExactReport> {
    let t0 = std::time::Instant::now();
    let n = builds(c, o)?;
    let terrain = crate::mobfarm::terrain_sample(c).map_err(|e| anyhow!(e))?.map(|s| s.name.to_string());
    let dir = mctas::paths::run_dir().join(format!("mobfarm-exact-{}-{}", std::process::id(), clock::catalog::content_hash(&c.clock)));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.txt", c.name()));
    std::fs::write(&path, c.clock.to_text())?;
    let exe = std::env::current_exe()?;
    let jobs: Vec<(u32, u32)> = (0..n).flat_map(|k| (0..runs).map(move |i| (k, i))).collect();
    let cap = std::env::var("MCTAS_MAX_WORKERS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|c| *c > 0).unwrap_or(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<ExactRun>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..cap.min(jobs.len()).max(1) {
            s.spawn(|| {
                loop {
                    let j = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(&(k, i)) = jobs.get(j) else { break };
                    let mut cmd = std::process::Command::new(&exe);
                    cmd.arg(format!("--version={}", o.version)).args(["mobfarm", "compare"]).arg(&path);
                    cmd.args(["--build", &k.to_string(), "--run", &i.to_string(), "--days", &days.to_string(), "--batch", "1000"]);
                    if let Some(t) = &terrain {
                        cmd.args(["--terrain", t]);
                    }
                    let (ticks, first) = match cmd.output() {
                        Ok(out) => parse_compare(&String::from_utf8_lossy(&out.stdout)),
                        Err(e) => (0, Some(format!("the compare did not start: {e}"))),
                    };
                    *results[j].lock().unwrap() = Some(ExactRun { build: k, run: i, ticks, first });
                }
            });
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    let runs: Vec<ExactRun> = results.into_iter().filter_map(|m| m.into_inner().unwrap()).collect();
    Ok(ExactReport { runs, days, seconds: t0.elapsed().as_secs_f64() })
}

/// One row of [`RECORD`].
#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    pub hash: String,
    pub file: String,
    pub exact: bool,
    pub verdict: String,
    pub recorded: String,
}

const HEADER: &str = "hash\tfile\texact\tverdict\trecorded";

/// Every recorded row (none when the table is missing).
pub fn read_record(root: &std::path::Path) -> Vec<Recorded> {
    let text = std::fs::read_to_string(root.join(RECORD)).unwrap_or_default();
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 5).then(|| Recorded { hash: f[0].into(), file: f[1].into(), exact: f[2] == "yes", verdict: f[3].into(), recorded: f[4].into() })
        })
        .collect()
}

/// Record a design's exactness (its row replaced), in hash order.
pub fn write_record(root: &std::path::Path, c: &Circuit, file: &str, g: &ExactReport) -> Result<Recorded> {
    let hash = clock::catalog::content_hash(&c.clock);
    let row = Recorded { hash: hash.clone(), file: file.to_string(), exact: g.passed(), verdict: g.summary(), recorded: chrono_now() };
    let mut rows: Vec<Recorded> = read_record(root).into_iter().filter(|r| r.hash != hash).collect();
    rows.push(row.clone());
    rows.sort_by(|a, b| a.file.cmp(&b.file).then(a.hash.cmp(&b.hash)));
    let mut out = String::from(HEADER);
    out.push('\n');
    for r in &rows {
        out.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", r.hash, r.file, if r.exact { "yes" } else { "no" }, r.verdict.replace('\t', " "), r.recorded));
    }
    std::fs::write(root.join(RECORD), out)?;
    Ok(row)
}

/// The UTC minute, as the archive's tables write it ("2026-11-08T23:59Z").
fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (s.div_euclid(86400), s.rem_euclid(86400));
    // civil from days (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}Z", rem / 3600, rem % 3600 / 60)
}

thread_local! {
    /// The last gate report of this thread's `confirm` (for `circuit archive` to record it).
    static LAST: std::cell::RefCell<Option<(String, ExactReport)>> = const { std::cell::RefCell::new(None) };
}

/// The gate report `confirm` made for this design on this thread (`circuit archive` records it).
pub fn last_report(c: &Circuit) -> Option<ExactReport> {
    let hash = clock::catalog::content_hash(&c.clock);
    LAST.with(|l| l.borrow().as_ref().filter(|(h, _)| *h == hash).map(|(_, g)| g.clone()))
}

/// The gate on a judged card: every build's run 0 compared on the bridge for the judge's days,
/// each result a line of the build's table; a design recorded exact fails if any build differs
/// now, any other design keeps its verdict (the official-server gate decides after).
pub fn confirm(c: &Circuit, o: &crate::verify::Options, mut rep: crate::verify::VerifyReport) -> Result<crate::verify::VerifyReport> {
    let g = gate(c, o, o.exact_runs.max(1), crate::mobfarm::JUDGE_DAYS)?;
    let hash = clock::catalog::content_hash(&c.clock);
    LAST.with(|l| *l.borrow_mut() = Some((hash.clone(), g.clone())));
    let lines: Vec<String> = std::iter::once(format!("  tick exact: {} (the 26.3 bridge, run 0 of each build, {} days, {:.0} s)", g.summary(), g.days, g.seconds)).chain(g.runs.iter().map(|r| r.line())).collect();
    for b in rep.builds.iter_mut() {
        if let Ok(k) = b.result.as_mut() {
            k.outcome.table.extend(lines.iter().cloned());
        }
    }
    let root = std::env::current_dir()?;
    if let Some(r) = read_record(&root).into_iter().find(|r| r.hash == hash)
        && r.exact
        && !g.passed()
    {
        let why = format!("tick-exactness regressed: recorded \"{}\" ({}), now {}", r.verdict, r.recorded, g.summary());
        for b in rep.builds.iter_mut() {
            b.result = Err(why.clone());
        }
        return Ok(crate::verify::verdict(rep));
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_reports() {
        assert_eq!(parse_compare("x\n24200 ticks compared: 0 differences\n"), (24200, None));
        let (t, f) = parse_compare("FIRST DIFFERENCE at game time 2790 (tick 2790 of 3200 compared): level random\n  game: 1\n");
        assert_eq!((t, f.as_deref()), (2789, Some("t2790: level random")));
        assert!(parse_compare("Error: boom\n").1.unwrap().starts_with("the compare did not run"));
    }

    #[test]
    fn summary_and_owner() {
        let ok = ExactRun { build: 0, run: 0, ticks: 120200, first: None };
        let bad = ExactRun { build: 2, run: 0, ticks: 200, first: Some("t201: entity #5 zombie head".into()) };
        let g = ExactReport { runs: vec![ok.clone(), ok.clone()], days: 5, seconds: 1.0 };
        assert_eq!(g.summary(), "tick-exact: 2/2 builds, 240400 ticks");
        let g = ExactReport { runs: vec![ok, bad], days: 5, seconds: 1.0 };
        assert_eq!(g.summary(), "first difference: build 2 run 0 t201: entity #5 zombie head (mob-AI); 1/2 builds exact");
        assert_eq!(owner("t3: weather"), "tick-exact");
        assert!(chrono_now().ends_with('Z'));
    }
}
