//! `mctas mobfarm gate` (mobinfra): the mob farms' real-game gate from the command line
//! (`circuit::mobfarm_game`), each build's run and the verdict printed.

use anyhow::{Result, anyhow, bail};
use std::path::Path;

/// The command's arguments.
pub struct GateArgs {
    pub days: u32,
    pub builds: Vec<String>,
    pub rain_day: bool,
    pub thunder_day: bool,
    pub seed: i64,
    pub parallel: usize,
    pub warmup: (u32, u32),
    pub keep: bool,
    pub trace: bool,
    pub terrain: Option<String>,
    pub world: Option<std::path::PathBuf>,
}

/// `mctas mobfarm gate <design>`.
pub fn gate_cmd(version: &str, design: &Path, a: GateArgs) -> Result<()> {
    let c = circuit::Circuit::load(design)?;
    let mut o = circuit::mobfarm_game::GateOpts::new(version);
    o.days = a.days;
    o.rain_day = a.rain_day;
    o.thunder_day = a.thunder_day;
    o.seed = a.seed;
    o.parallel = a.parallel;
    o.warmup = a.warmup;
    o.keep = a.keep;
    o.trace = a.trace;
    o.terrain = a.terrain;
    o.world = a.world;
    // a terrain sample: `--builds K...` picks runs of its one placement by index (each its own
    // seed: run 6 of D1U's gate is the one to look at again)
    if o.terrain.is_some() {
        for b in &a.builds {
            let k: u8 = b.parse().map_err(|_| anyhow!("in a terrain sample --builds takes run indices (0..--parallel), e.g. 6"))?;
            o.builds.push((k, [0, 0, 0]));
        }
    }
    let all = circuit::mobfarm_game::judge_builds();
    for b in a.builds.iter().filter(|_| o.terrain.is_none()) {
        let (t, k) = b.split_once('.').ok_or_else(|| anyhow!("--builds takes rotation.offset, e.g. 0.0"))?;
        let (t, k): (u8, usize) = (t.parse()?, k.parse()?);
        let Some(x) = all.iter().find(|(tt, _)| *tt == t).and_then(|_| all.iter().filter(|(tt, _)| *tt == t).nth(k)) else { bail!("no build {b}") };
        o.builds.push(*x);
    }
    let t0 = std::time::Instant::now();
    let rep = circuit::mobfarm_game::gate(&c, &o)?;
    for b in &rep.builds {
        let ws: Vec<String> = b
            .windows
            .iter()
            .map(|w| format!("{}: {:.1} golems/h ({} golems, raining {}%, thundering {}%)", w.name, w.golems_per_hour(), w.golems, 100 * w.rain_samples / w.samples.max(1), 100 * w.thunder_samples / w.samples.max(1)))
            .collect();
        println!(
            "build rotation {} offset {:?} seed {}: {} golems, {} dead, iron {} in the chest + {} in containers + {} loose{} ({:.2} per dead golem), {} poppies, {} bolts seen, {:.0} s{}",
            b.turns,
            b.offset,
            b.seed,
            b.golems,
            b.deaths,
            b.iron_chest,
            b.iron_containers,
            b.iron_loose,
            if b.iron_start > 0 { format!(" - {} the design placed", b.iron_start) } else { String::new() },
            b.iron() as f64 / b.deaths.max(1) as f64,
            b.poppies,
            b.bolts_seen.len(),
            b.seconds,
            if b.passed() { String::new() } else { format!(": FAIL {}", b.failures.join("; ")) }
        );
        for w in ws {
            println!("  {w}");
        }
        if !b.changed.is_empty() {
            println!("  changed: {} cells: {}", b.changed.len(), b.changed.iter().take(6).cloned().collect::<Vec<_>>().join("; "));
        }
    }
    println!("redsim: {} runs of the same schedule", rep.sim_runs);
    println!("{}: {}", if rep.passed() { "PASS" } else { "FAIL" }, rep.line());
    for f in &rep.failures {
        println!("  {f}");
    }
    println!("{:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}
