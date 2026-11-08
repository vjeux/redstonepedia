//! Mob farms (the ironfarm track, 2026-11-08; brief `designs/briefs/farm-iron.md`, the work in
//! `designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`): kind `mob-farm <item>`, a farm whose
//! product drops from mobs. The first product is `iron_ingot`: iron golems that villagers summon
//! when a zombie scares them, killed and collected into the judge's chest at `port output`.
//!
//! Judged entirely on redsim's mob layer (`redsim::entity::mob`; vjeux 2026-11-07: "Please
//! implement everything you need on redsim. We can't use the game code, it's way too slow"),
//! both by `circuit verify` and `circuit sim`; the game checks are the brief's one-off validations
//! (`mctas mobfarm validate`).
//!
//! * Before the runs, on the base build: the refusals (`mob::refuse`), the golem exit flood
//!   (`mob::exit`: from every position a golem can stand on, still and 8 inputs must kill it
//!   within 1200 gt), the 9 worst-policy runs on one seed, and the restart probe for a design
//!   that claims it (`restart: yes`).
//! * Each build (4 rotations x 2 positions) runs [`JUDGE_SEEDS`] seeds of [`JUDGE_DAYS`] game
//!   days from the steady state (a day time in the day, `LAST_SLEPT` at the end of the last
//!   REST), each golem walking under a random policy; every must-have is checked as it runs
//!   (`mob::farm`).
//! * The card's rate is the mean over every build's runs, with its 95% interval. Builds agree
//!   when their own means are within 20% (`Agree::Rate`).
//!
//! Metrics: iron per hour per block (the board's rank), iron per hour, golems per hour, seconds
//! per golem, the share of the ceiling (one golem per 600 gt per villager cluster), the worst
//! policy's rate, the misses, the exit flood, the bill (iron, quartz, villagers, beds) and the
//! payback.

use crate::check::Outcome;
use crate::registry::{Agree, KindDef, RunAt, Site, StudyDef, Twin};
use crate::stimulus::Schedule;
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow, bail};
use mctas::sim::Pool;
use redsim::entity::mob::exit::{self, ExitReport};
use redsim::entity::mob::farm::{self, FarmSpec, RunOpts, RunReport, WalkMode};
use redsim::entity::mob::walk::Walk;
use redsim::{Block, BlockPos};

/// The study's registration (crate::registry): `circuit: mob-farm iron_ingot`.
pub static STUDY: StudyDef = StudyDef {
    id: "mobfarm",
    title: "Mob farms",
    site_title: "Mob farms",
    site_intro: "Iron golems spawn when three villagers that slept in the last game day are scared by a zombie: one golem per 600 game ticks for each group of villagers at most, on a solid block within 8 blocks of them. These farms hold the villagers and a zombie in place, let the villagers sleep each night (a panicking villager never sleeps), kill each golem and carry its 3 to 5 iron ingots into a chest. Our test runs every farm for days of game time on our own simulation of the game's mob rules, in every orientation, and checks that the villagers stay safe, the zombie stays put, every golem dies in the farm and every ingot reaches the chest.",
    site_uses: "Iron for hoppers, rails and beacons without mining: AFK near the farm and empty its chest.",
    site_blurb: "Iron golem farms: villagers, a zombie and beds, rated in iron ingots per hour.",
    kinds: &[KindDef {
        word: "mob-farm",
        parse: parse_kind,
        inputs: &[],
        check: check_kind,
        verify: verify_in_pool,
        twin: Twin::Exact(sim_verify),
        agree: Agree::Rate { key: "iron per hour", tol: 0.2 },
        ticks: |_| JUDGE_DAYS * 24000,
        axis: |_, m| {
            let n = m("modules");
            if n == "1" { "1 module".to_string() } else { format!("{n} modules") }
        },
        runs: &["base", "still"],
        run: run_kind,
        site: Site {
            title: |k| match k.split_whitespace().nth(1) {
                Some("iron_ingot") | None => "Iron farm".to_string(),
                Some(item) => format!("Mob farm: {}", item.replace('_', " ")),
            },
            summary: |_| "villagers scared by a zombie summon iron golems; the farm kills them and collects their iron in a chest".into(),
            metric: |m| match m {
                "iron" => Some("iron ingots per hour at the game's normal speed: the mean over every simulated day, with its 95% interval"),
                "golems" => Some("iron golems per hour"),
                "seconds" => Some("the mean time between two golems"),
                "share" => Some("the rate as a share of the game's limit: one golem per 600 game ticks for each group of villagers"),
                "modules" => Some("groups of villagers that summon golems independently"),
                "worst" => Some("the rate when every golem walks the same way, the worst of standing still and the 8 directions"),
                "bill" => Some("what the build costs in survival: iron ingots (hoppers, pistons, the lava bucket...), quartz, villagers, beds"),
                "payback" => Some("the time the farm takes to give back the iron it cost"),
                _ => None,
            },
        },
    }],
};

/// Seeds each build runs.
pub const JUDGE_SEEDS: u32 = 8;
/// Game days per run.
pub const JUDGE_DAYS: u32 = 5;
/// Game days of the restart probe.
pub const RESTART_DAYS: u32 = 2;

/// `mob-farm <item>`: only `iron_ingot` so far.
fn parse_kind(w: &[&str]) -> Result<Vec<String>> {
    match w.first().copied() {
        Some("iron_ingot") | Some("minecraft:iron_ingot") => Ok(vec!["iron_ingot".into()]),
        Some(other) => bail!("mob-farm: no farm for {other:?} yet (iron_ingot)"),
        None => bail!("mob-farm: give the product (`circuit: mob-farm iron_ingot`)"),
    }
}

/// The ports: `port output` (an air cell: the judge's chest).
fn check_kind(c: &Circuit, _args: &[String]) -> Result<()> {
    let d = &c.clock.design;
    let Some(out) = d.ports.get("output") else { bail!("mob-farm: needs `port output: x y z` (an air cell: the judge's chest)") };
    if d.blocks.contains_key(out) {
        bail!("mob-farm: port output {out:?} must be an air cell (the judge puts its chest there)");
    }
    Ok(())
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The redsim farm of a (turned, moved) design: its blocks, entities, output chest and box.
pub fn spec_of(c: &Circuit) -> std::result::Result<FarmSpec, String> {
    let d = &c.clock.design;
    let out = *d.ports.get("output").ok_or("no `port output`")?;
    let (from, to) = crate::run::watch_box(c, 4);
    let mut blocks = Vec::new();
    for (q, s) in d.block_list() {
        blocks.push((bp(q), Block::parse(&s).map_err(|e| format!("not modeled by redsim: {e}"))?));
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let entities = d.entities.iter().map(|e| (e.kind.clone(), e.pos, e.nbt.clone())).collect();
    Ok(FarmSpec { blocks, entities, output: bp(out), lo: bp(from), hi: bp(to) })
}

/// What the judge runs.
#[derive(Clone, Copy, Debug)]
pub struct JudgeOpts {
    pub seeds: u32,
    pub days: u32,
    /// The first seed (build k adds k x 1000).
    pub seed0: i64,
    /// Threads for the exit flood.
    pub threads: usize,
}

impl Default for JudgeOpts {
    fn default() -> JudgeOpts {
        JudgeOpts { seeds: JUDGE_SEEDS, days: JUDGE_DAYS, seed0: 0x6972_6f6e, threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32) }
    }
}

/// Run several farm runs at once (one thread each).
pub fn run_all(spec: &FarmSpec, jobs: &[RunOpts]) -> Vec<std::result::Result<RunReport, String>> {
    std::thread::scope(|s| {
        let hs: Vec<_> = jobs.iter().map(|j| s.spawn(move || farm::run(spec, j))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a run panicked".to_string()))).collect()
    })
}

/// Build `build`'s runs: the refusals first, then [`JudgeOpts::seeds`] steady-state runs.
pub fn build_runs(spec: &FarmSpec, o: &JudgeOpts, build: u32, walk: WalkMode) -> std::result::Result<Vec<RunReport>, String> {
    let refused = redsim::entity::mob::refuse::static_checks(spec);
    if !refused.is_empty() {
        return Err(format!("refused: {}", refused.join("; ")));
    }
    let jobs: Vec<RunOpts> = (0..o.seeds).map(|k| RunOpts::steady(o.seed0 + build as i64 * 1000 + k as i64, o.days * 24000, walk)).collect();
    run_all(spec, &jobs).into_iter().collect()
}

/// The 9 worst-policy runs: still and the 8 directions, every golem of a run under that one
/// policy, all on the same seed.
pub fn worst_policy(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<(Walk, RunReport)>, String> {
    let policies = Walk::golem_policies();
    let jobs: Vec<RunOpts> = policies.iter().map(|p| RunOpts::steady(o.seed0 + 777, o.days * 24000, WalkMode::Fixed(*p))).collect();
    let runs: Vec<RunReport> = run_all(spec, &jobs).into_iter().collect::<std::result::Result<_, _>>()?;
    Ok(policies.into_iter().zip(runs).collect())
}

/// The restart probe: a run whose villagers have no `LAST_SLEPT` must spawn a golem within
/// [`RESTART_DAYS`] days.
pub fn restart_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<RunReport, String> {
    let mut ro = RunOpts::steady(o.seed0 + 999, RESTART_DAYS * 24000, WalkMode::Random);
    ro.slept = false;
    farm::run(spec, &ro)
}

/// Whether the design claims to restart after a missed night (`restart: yes`).
pub fn claims_restart(c: &Circuit) -> bool {
    c.clock.design.meta.get("restart").is_some_and(|v| v.trim() == "yes")
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// One table line per run.
fn run_line(label: &str, r: &RunReport) -> String {
    let fail = if r.passed() { String::new() } else { format!(": FAIL {}", r.failures.iter().chain(r.errors.iter()).cloned().collect::<Vec<_>>().join("; ")) };
    format!(
        "  {label}: {:.0} iron/h, {:.1} golems/h, {} golems in {:.1} h, kill {:.0} gt, {} tries without a spot, {} sightings, {} sleeps, longest without sleep {} gt{fail}",
        r.iron_per_hour(),
        r.golems_per_hour(),
        r.golems,
        r.hours(),
        mean(&r.lifetimes.iter().map(|v| *v as f64).collect::<Vec<_>>()),
        r.losses.no_spot,
        r.losses.seen,
        r.sleeps,
        r.max_sleep_gap,
    )
}

/// What only the base build checks: the exit flood, the worst policy, the restart probe.
#[derive(Clone, Debug, Default)]
pub struct BaseChecks {
    pub exit: ExitReport,
    pub worst: Vec<(Walk, RunReport)>,
    pub restart: Option<RunReport>,
}

impl BaseChecks {
    /// Why the base checks fail (empty: they pass).
    pub fn failures(&self) -> Vec<String> {
        let mut v: Vec<String> = self.exit.failures.iter().map(|f| format!("golem exit: {f}")).collect();
        for (p, r) in &self.worst {
            if let Some(f) = r.failures.iter().chain(r.errors.iter()).next() {
                v.push(format!("every golem walking {}: {f}", p.name()));
            }
        }
        if let Some(r) = &self.restart
            && r.golems == 0
        {
            v.push(format!("claims to restart after a missed night, but no golem spawned in {RESTART_DAYS} days without a recent sleep"));
        }
        v
    }
}

/// The base checks of a farm.
pub fn base_checks(c: &Circuit, spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<BaseChecks, String> {
    let refused = redsim::entity::mob::refuse::static_checks(spec);
    if !refused.is_empty() {
        return Err(format!("refused: {}", refused.join("; ")));
    }
    let exit = exit::golem_exit(spec, o.seed0, o.threads)?;
    let worst = if exit.failures.is_empty() { worst_policy(spec, o)? } else { Vec::new() };
    let restart = if claims_restart(c) { Some(restart_probe(spec, o)?) } else { None };
    Ok(BaseChecks { exit, worst, restart })
}

/// A build's outcome from its runs (`pooled`: every build's runs, for the base build's card;
/// `base`: the base checks, on the base build).
pub fn outcome(c: &Circuit, spec: &FarmSpec, runs: &[RunReport], pooled: &[RunReport], base: Option<&BaseChecks>) -> Outcome {
    let mut table: Vec<String> = runs.iter().enumerate().map(|(k, r)| run_line(&format!("run {k}"), r)).collect();
    if let Some(b) = base {
        table.push(format!("  golem exit: {} spawn spots, {} positions, {} drift runs, every golem dead within {} gt", b.exit.spawn_spots, b.exit.starts, b.exit.runs, b.exit.worst_life));
        for (p, r) in &b.worst {
            table.push(run_line(&format!("every golem {}", p.name()), r));
        }
        if let Some(r) = &b.restart {
            table.push(run_line("restart probe (no recent sleep)", r));
        }
    }
    let mut why: Vec<String> = runs.iter().filter(|r| !r.passed()).flat_map(|r| r.failures.iter().chain(r.errors.iter()).cloned()).take(3).collect();
    if let Some(b) = base {
        why.extend(b.failures());
    }
    if !why.is_empty() {
        return Outcome { pass: false, why: why.join("; "), metrics: Vec::new(), table };
    }
    let all = if pooled.is_empty() { runs } else { pooled };
    let rates: Vec<f64> = all.iter().map(|r| r.iron_per_hour()).collect();
    let own = mean(&runs.iter().map(|r| r.iron_per_hour()).collect::<Vec<_>>());
    let iron = if base.is_some() { mean(&rates) } else { own };
    let half = 1.96 * sd(&rates) / (rates.len() as f64).sqrt();
    let golems = mean(&all.iter().map(|r| r.golems_per_hour()).collect::<Vec<_>>());
    let modules = villager_clusters(spec);
    let ceiling = 120.0 * modules as f64;
    let b = bill(c);
    let payback = if iron > 0.0 { b.iron / iron * 60.0 } else { f64::INFINITY };
    let no_spot: u32 = all.iter().map(|r| r.losses.no_spot).sum();
    let seen: u32 = all.iter().map(|r| r.losses.seen).sum();
    let tries = no_spot + all.iter().map(|r| r.golems).sum::<u32>();
    let lifetimes: Vec<f64> = all.iter().flat_map(|r| r.lifetimes.iter().map(|v| *v as f64)).collect();
    let mut metrics = vec![
        ("iron per hour per block".to_string(), format!("{:.2}", iron / c.blocks().max(1) as f64)),
        ("iron per hour".to_string(), format!("{iron:.0}")),
        ("iron range".to_string(), format!("95% {:.0}-{:.0} ({} runs)", iron - half, iron + half, rates.len())),
        ("golems per hour".to_string(), format!("{golems:.1}")),
        ("seconds per golem".to_string(), format!("{:.1}", if golems > 0.0 { 3600.0 / golems } else { 0.0 })),
        ("share of ceiling".to_string(), format!("{:.0}%", 100.0 * golems / ceiling)),
        ("modules".to_string(), modules.to_string()),
        ("iron per hour per module".to_string(), format!("{:.0}", iron / modules as f64)),
        ("kill".to_string(), format!("{:.0} gt", mean(&lifetimes))),
    ];
    if let Some(bc) = base {
        if let Some((p, r)) = bc.worst.iter().min_by(|a, b| a.1.iron_per_hour().total_cmp(&b.1.iron_per_hour())) {
            metrics.push(("worst policy".to_string(), format!("{:.0} iron/h ({})", r.iron_per_hour(), p.name())));
        }
        metrics.push(("restarts after a missed night".to_string(), if bc.restart.is_some() { "yes".to_string() } else { "no (hide the zombie by hand the first night)".to_string() }));
    }
    metrics.push(("misses".to_string(), format!("{no_spot} of {tries} tries found no spot, {seen} golem sightings")));
    metrics.push(("bill".to_string(), b.text()));
    metrics.push(("payback".to_string(), format!("{payback:.0} min")));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The villager clusters of a farm (villagers within each other's 10-block search box, joined).
pub fn villager_clusters(spec: &FarmSpec) -> usize {
    let pos: Vec<[f64; 3]> = spec.entities.iter().filter(|(k, _, _)| k.ends_with("villager")).map(|(_, p, _)| *p).collect();
    let n = pos.len();
    let mut group: Vec<usize> = (0..n).collect();
    for a in 0..n {
        for b in 0..n {
            let near = (0..3).all(|k| (pos[a][k] - pos[b][k]).abs() < 10.0 + if k == 1 { 1.95 } else { 0.6 });
            if near {
                let (ga, gb) = (group[a], group[b]);
                for g in group.iter_mut() {
                    if *g == gb {
                        *g = ga;
                    }
                }
            }
        }
    }
    group.sort();
    group.dedup();
    group.len().max(1)
}

/// What a build costs in survival.
#[derive(Clone, Debug, Default)]
pub struct Bill {
    pub iron: f64,
    pub quartz: u32,
    pub villagers: u32,
    pub beds: u32,
    pub zombies: u32,
}

impl Bill {
    pub fn text(&self) -> String {
        format!("{:.0} iron, {} quartz, {} villagers, {} beds, {} zombie", self.iron.ceil(), self.quartz, self.villagers, self.beds, self.zombies)
    }
}

/// The survival cost of a design's blocks and entities: iron by recipe (hopper 5, piston 1,
/// iron trapdoor 4, minecart 5, rails 6 per 16, tripwire hooks 1 per 2, a bucket 3 for the lava
/// and water), quartz (daylight detector 3, observer 1, comparator 1), villagers, beds, zombies.
pub fn bill(c: &Circuit) -> Bill {
    let d = &c.clock.design;
    let mut b = Bill::default();
    let mut bucket = false;
    for s in d.blocks.values() {
        let id = s.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        match id {
            "hopper" => b.iron += 5.0,
            "piston" | "sticky_piston" => b.iron += 1.0,
            "iron_trapdoor" => b.iron += 4.0,
            "iron_block" => b.iron += 9.0,
            "rail" => b.iron += 6.0 / 16.0,
            "detector_rail" | "activator_rail" => b.iron += 1.0,
            "tripwire_hook" => b.iron += 0.5,
            "heavy_weighted_pressure_plate" => b.iron += 2.0,
            "iron_door" if s.contains("half=lower") => b.iron += 2.0,
            "daylight_detector" => b.quartz += 3,
            "observer" | "comparator" => b.quartz += 1,
            "lava" | "water" => bucket = true,
            id if id.ends_with("_bed") && s.contains("part=head") => b.beds += 1,
            _ => {}
        }
    }
    if bucket {
        b.iron += 3.0;
    }
    for e in &d.entities {
        match e.kind.trim_start_matches("minecraft:") {
            "villager" => b.villagers += 1,
            "zombie" => b.zombies += 1,
            "minecart" => b.iron += 5.0,
            "hopper_minecart" => b.iron += 10.0,
            _ => {}
        }
        if let Some(n) = &e.nbt
            && n.contains("minecraft:zombie")
        {
            b.zombies += 1;
        }
    }
    b
}

fn kind_of(c: &Circuit) -> Result<Kind> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if !matches!(&kind, Kind::Study(_)) {
        bail!("{}: not a mob farm", kind.name());
    }
    Ok(kind)
}

/// The design's box (blocks only).
fn dims(c: &Circuit) -> [i32; 3] {
    let (lo, hi) = c.bounds();
    [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
}

/// The whole judge on redsim: every build (rotations x positions), each with its seeds.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    judge(c, o, &JudgeOpts::default())
}

/// [`sim_verify`] with the judge's run options.
pub fn judge(c: &Circuit, o: &Options, jo: &JudgeOpts) -> std::result::Result<VerifyReport, String> {
    let kind = kind_of(c).map_err(|e| e.to_string())?;
    let offsets = clock::offsets(o.positions.clamp(1, 2));
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut jobs = Vec::new();
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            let rc = tc.translated(*off).map_err(|e| format!("{e:#}"))?;
            let spec = spec_of(&rc)?;
            jobs.push((*t, *off, rc, spec));
        }
    }
    let base = base_checks(&jobs[0].2, &jobs[0].3, jo);
    let runs: Vec<std::result::Result<Vec<RunReport>, String>> = std::thread::scope(|s| {
        let hs: Vec<_> = jobs.iter().enumerate().map(|(k, (_, _, _, spec))| s.spawn(move || build_runs(spec, jo, k as u32, WalkMode::Random))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("a build panicked".to_string()))).collect()
    });
    let pooled: Vec<RunReport> = runs.iter().filter_map(|r| r.as_ref().ok()).flatten().cloned().collect();
    let mut builds = Vec::new();
    for (k, ((t, off, rc, spec), r)) in jobs.iter().zip(runs).enumerate() {
        let result = match (k, r, &base) {
            (_, Err(e), _) => Err(e),
            (0, Ok(_), Err(e)) => Err(e.clone()),
            (0, Ok(runs), Ok(b)) => Ok(BuildOk { outcome: outcome(rc, spec, &runs, &pooled, Some(b)), dims: dims(rc), family: family.clone() }),
            (_, Ok(runs), _) => Ok(BuildOk { outcome: outcome(rc, spec, &runs, &[], None), dims: dims(rc), family: family.clone() }),
        };
        builds.push(Build { turns: *t, offset: *off, result });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = jo.days * 24000;
    Ok(crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() }))
}

/// `circuit verify`: the same redsim judge (the mob farms' authority; the game checks are the
/// brief's one-off validations).
pub fn verify_in_pool(_pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    sim_verify(c, o).map_err(|e| anyhow!(e))
}

/// `mctas circuit run`: one build's runs (`base`: golems walk at random; `still`: they all stand
/// still), with the base checks.
fn run_kind(_pool: Option<&Pool>, c: &Circuit, at: &RunAt) -> Result<Outcome> {
    let walk = match at.run {
        "base" => WalkMode::Random,
        "still" => WalkMode::Fixed(Walk::Still),
        r => bail!("mob-farm: no run {r:?} (base, still)"),
    };
    let rc = c.turned_pure(at.turns)?.translated(at.offset)?;
    let spec = spec_of(&rc).map_err(|e| anyhow!(e))?;
    let jo = JudgeOpts::default();
    let base = base_checks(&rc, &spec, &jo).map_err(|e| anyhow!(e))?;
    let runs = build_runs(&spec, &jo, 0, walk).map_err(|e| anyhow!(e))?;
    Ok(outcome(&rc, &spec, &runs, &[], Some(&base)))
}
