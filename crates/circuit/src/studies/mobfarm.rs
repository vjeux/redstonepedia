//! Mob farms (the ironfarm track, 2026-11-08; brief `designs/briefs/farm-iron.md`, the work in
//! `designs/circuits/mobfarm/tracks/ironfarm/NOTES.md`): kind `mob-farm <item>`, a farm whose
//! product drops from mobs. The first product is `iron_ingot`: iron golems that villagers summon
//! when a zombie scares them, killed and collected into the judge's chest at `port output`.
//!
//! Judged on redsim's mob layer (`redsim::entity::mob`; vjeux 2026-11-07: "Please implement
//! everything you need on redsim. We can't use the game code, it's way too slow"): `circuit sim`
//! is that judge; `circuit verify` (so `circuit archive`) also runs every build in the real game
//! (mobinfra 2026-11-08, `crate::mobfarm_game`: the 26.3 server, days of weather, a forced day of
//! rain and of thunder, the must-haves and the rate against redsim's), and a farm the game fails
//! does not pass.
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
use redsim::entity::mob::absence::Away;
use redsim::entity::mob::exit::{self, ExitReport};
use redsim::entity::mob::farm::{self, AwayPlan, AwayReport, FarmSpec, RunOpts, RunReport, WalkMode};
use redsim::entity::mob::walk::Walk;
use redsim::{Block, BlockPos};

/// The study's registration (crate::registry): `circuit: mob-farm iron_ingot [tier]`.
pub static STUDY: StudyDef = StudyDef {
    id: "mobfarm",
    title: "Mob farms",
    site_title: "Mob farms",
    site_intro: "Iron golems spawn when three villagers that slept in the last game day are scared by a zombie: one golem per 600 game ticks for each group of villagers at most, on a solid block within 8 blocks of them. These farms hold the villagers and a zombie in place, let the villagers sleep each night (a panicking villager never sleeps), kill each golem and carry its 3 to 5 iron ingots into a chest. Our test runs every farm for days of game time in every orientation, on Hard with the weather's rain, thunder and lightning, and checks that the villagers stay safe, the zombie stays put, every golem dies in the farm and every ingot reaches the chest; every build also runs in the real game, through a day of rain and a day of thunder. Three farms, one per need: the day-1 farm costs the least iron, the middle-ground farm gives the most iron for each block placed, the fastest farm the most iron for each group of villagers, and copies of it scale it up.",
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
        axis: |args, m| match tier(args) {
            Some((_, words)) => words.to_string(),
            None => {
                let n = m("modules");
                if n == "1" { "1 module".to_string() } else { format!("{n} modules") }
            }
        },
        runs: &["base", "still"],
        run: run_kind,
        site: Site {
            title: |k| {
                let w: Vec<&str> = k.split_whitespace().collect();
                match (w.get(1).copied(), w.get(2).copied()) {
                    (Some("iron_ingot") | None, Some("day-1")) => "Day-1 iron farm (the cheapest to build)".to_string(),
                    (Some("iron_ingot") | None, Some("middle")) => "Middle-ground iron farm (the most iron for each block placed)".to_string(),
                    (Some("iron_ingot") | None, Some("fastest")) => "Fastest iron farm (the most iron for each group of villagers; copies scale it)".to_string(),
                    (Some("iron_ingot") | None, _) => "Iron farm".to_string(),
                    (Some(item), _) => format!("Mob farm: {}", item.replace('_', " ")),
                }
            },
            summary: |_| "villagers scared by a zombie summon iron golems; the farm kills them and collects their iron in a chest".into(),
            metric: |m| match m {
                "iron per hour per block" => Some("iron ingots an hour for each block placed: the board's rank, and the middle-ground farm's"),
                "iron per hour" => Some("iron ingots an hour at the game's normal speed: the mean over every simulated day of every build"),
                "iron range" => Some("the 95% interval of that mean (how sure the number is), and how many simulated runs it is over"),
                "golems per hour" => Some("iron golems an hour"),
                "seconds per golem" => Some("the mean time between two golems"),
                "share of ceiling" => Some("the rate as a share of the game's limit: one golem per 600 game ticks for each group of villagers"),
                "modules" => Some("groups of villagers that summon golems independently (copies of the farm count one each)"),
                "iron per hour per module" => Some("iron ingots an hour for each group of villagers: the fastest farm's rank"),
                "kill" => Some("the longest a golem lived from any spot it can spawn or stand on, standing still or walking any of 8 ways"),
                "worst policy" => Some("the rate when every golem walks the same way, the worst of standing still and the 8 directions"),
                "restarts after a missed night" => Some("whether the farm starts again by itself after a night the villagers could not sleep"),
                "restarts after days away" => Some("whether the farm starts again by itself after the player was away for 3 days (its chunks unloaded, or loaded but not ticking), back by day or at night"),
                "misses" => Some("summons that found no spot for the golem, and the golems the villagers saw (each one delays the next summon)"),
                "bill" => Some("what the build costs in survival: iron ingots (hoppers, the lava bucket and every tool the build needs), quartz, villagers, beds"),
                "payback" => Some("the time the farm takes to give back the iron it cost"),
                "tools" => Some("the tools the build needs and the iron each costs: a bucket for lava and water, shears for leaves, an iron pickaxe for redstone dust"),
                "lava sources" => Some("lava source blocks: each one is a bucket trip from a lava pool"),
                "materials" => Some("the latest stage of the game its materials need: day 1, the Nether (quartz, slime, magma...) or the End"),
                "chunk columns" => Some("the fewest 16 x 16 chunk columns the build covers when placed on the chunk grid"),
                "storage" => Some("how long the farm's chests and hoppers hold its iron and poppies at this rate"),
                "hoppers per 1000 iron/h" => Some("hoppers for each 1000 iron an hour (each hopper costs the server work every tick)"),
                "real game" => Some("the farm run in the real game (the official 26.3 server, on Hard) in every orientation for game days with the weather on, then a day of rain and a day of thunder: it works in each, and its rate agrees with our simulation's"),
                "lightning" => Some("what thunderstorms cost the farm, summed exactly over every place a strike can land (the game's 1 in 100000 a chunk a tick while it rains and thunders, aimed at a lightning rod, a living mob that sees the sky or the column's top), over our test's runs: the iron the bolts burn, how often a bolt makes a villager a witch, breaks the zombie's boat, lights a fire beside a flammable block or brings a skeleton trap horse onto the farm, and the game days until one breaks the farm"),
                "thunder loss" => Some("iron lost for each hour of thunder: what the bolts burn and, when one breaks the farm, its rate until the end of that game day"),
                "rain" => Some("iron an hour on a clear day, a day of rain and a day of thunder (rain darkens the sky, which some calm devices read)"),
                "sunburn" => Some("whether the zombie can see the sky: a zombie that sees it burns in daylight"),
                "fire" => Some("whether lava's fire can burn one of the farm's blocks (with a player within 128 blocks, lava lights fires beside the blocks lava ignites)"),
                _ => None,
            },
        },
    }],
};

/// The brief's tiers (designs/briefs/farm-iron.md revision 3), each a kind of its own so each
/// gets its own site page (vjeux via the decisions inbox 2026-11-08, after the crop farms' one
/// kind per crop): the word on the `circuit:` line, and in words.
pub const TIERS: [(&str, &str); 3] = [("day-1", "day 1"), ("middle", "middle ground"), ("fastest", "fastest")];

/// The tier a kind's words name (none: a farm in no tier, like the brief's obvious design).
pub fn tier(args: &[String]) -> Option<(&'static str, &'static str)> {
    args.get(1).and_then(|t| TIERS.iter().copied().find(|(w, _)| w == t))
}

/// Seeds each build runs.
pub const JUDGE_SEEDS: u32 = 8;
/// Game days per run.
pub const JUDGE_DAYS: u32 = 5;
/// Game days of the restart probe.
pub const RESTART_DAYS: u32 = 2;

/// `mob-farm <item> [tier]`: only `iron_ingot` so far; the tier one of [`TIERS`] (none: a farm
/// in no tier, published nowhere but the catalog).
fn parse_kind(w: &[&str]) -> Result<Vec<String>> {
    let item = match w.first().copied() {
        Some("iron_ingot") | Some("minecraft:iron_ingot") => "iron_ingot".to_string(),
        Some(other) => bail!("mob-farm: no farm for {other:?} yet (iron_ingot)"),
        None => bail!("mob-farm: give the product (`circuit: mob-farm iron_ingot`)"),
    };
    match w[1..] {
        [] => Ok(vec![item]),
        [t] if TIERS.iter().any(|(k, _)| *k == t) => Ok(vec![item, t.to_string()]),
        [t] => bail!("mob-farm: no tier {t:?} ({})", TIERS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")),
        _ => bail!("mob-farm: at most a product and a tier (`circuit: mob-farm iron_ingot middle`)"),
    }
}

/// The ports: `port output` (the farm's chest: a single chest in the design, or an air cell where
/// the judge puts one).
fn check_kind(c: &Circuit, _args: &[String]) -> Result<()> {
    let d = &c.clock.design;
    let Some(out) = d.ports.get("output") else { bail!("mob-farm: needs `port output: x y z` (the farm's chest, or an air cell for the judge's)") };
    match d.blocks.get(out) {
        None => Ok(()),
        Some(s) if s.trim_start_matches("minecraft:").starts_with("chest[") && s.contains("type=single") => Ok(()),
        Some(s) => bail!("mob-farm: port output {out:?} holds {s}: it must be the farm's single chest or an air cell (the judge puts its chest there)"),
    }
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The terrain site of a design in the standard sample `sample` (brief revision 8: day 1 is judged
/// in terrain), from its meta: `ground: <y>` (the design y lying at the ground layer; default -1,
/// a void design's floor under its villagers), `afk: x y z` (the AFK player's feet; default on top
/// of the build's middle) and `terrain: cells` (a design made for terrain: only its listed cells
/// are written, natural rock for the rest; otherwise its box is cleared first).
pub fn site_of(c: &Circuit, spec: &FarmSpec, sample: &str) -> std::result::Result<redsim::entity::mob::site::Site, String> {
    let d = &c.clock.design;
    let s = redsim::terrain::samples::sample(sample).ok_or_else(|| format!("no terrain sample {sample}"))?;
    let (ground, afk, clear_box) = redsim::entity::mob::site::params(spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str))?;
    redsim::entity::mob::site::Site::new(s, spec, ground, afk, clear_box)
}

/// The farm the terrain track's worst-case checks read (`hostile::Farm`), from a spec already
/// built into `w`: its blocks and beds, its villagers' standing and sleeping boxes and eyes, its
/// zombie and vehicle, the golem spawn cells its villagers' search finds inside its box, and the
/// interior (the cells those occupy, the golem cells and the two above them).
pub fn hostile_farm(w: &redsim::World, spec: &FarmSpec, afk: redsim::entity::Vec3) -> redsim::entity::mob::hostile::Farm {
    use redsim::entity::{Aabb, Vec3};
    let mut farm = redsim::entity::mob::hostile::Farm { afk, ..Default::default() };
    for (q, _) in &spec.blocks {
        farm.blocks.push(*q);
        if let Block::Bed { head: true, .. } = w.get(*q) {
            farm.beds.push(*q);
        }
    }
    for (kind, p, nbt) in &spec.entities {
        let (x, y, z) = (p[0], p[1], p[2]);
        let kind = kind.trim_start_matches("minecraft:");
        if kind == "villager" {
            farm.villagers.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3));
            farm.villagers.push(Aabb::new(x - 0.1, y, z - 0.1, x + 0.1, y + 0.2, z + 0.1));
            farm.villager_eyes.push(Vec3::new(x, y + 1.62, z));
            farm.villager_eyes.push(Vec3::new(x, y + 0.2, z));
        } else if kind.ends_with("_boat") || kind.ends_with("_raft") || kind.contains("minecart") {
            farm.zombie.push(Aabb::new(x - 0.6875, y, z - 0.6875, x + 0.6875, y + 0.5625, z + 0.6875));
            if nbt.as_deref().is_some_and(|n| n.contains("zombie")) {
                farm.zombie.push(Aabb::new(x - 0.3, y + 0.2, z - 0.3, x + 0.3, y + 2.15, z + 0.3));
            }
        } else if kind == "zombie" {
            farm.zombie.push(Aabb::new(x - 0.3, y, z - 0.3, x + 0.3, y + 1.95, z + 0.3));
        }
    }
    let starts: Vec<BlockPos> = farm.villagers.iter().step_by(2).map(|v| BlockPos::new(v.center().x.floor() as i32, v.min_y.floor() as i32, v.center().z.floor() as i32)).collect();
    for s in starts {
        for dz in -8..=8 {
            for dx in -8..=8 {
                if let Some(c) = redsim::entity::mob::spawn::possible_spawn_position(w, s.offset(dx, 6, dz), 6)
                    && (spec.lo.x..=spec.hi.x).contains(&c.x)
                    && (spec.lo.z..=spec.hi.z).contains(&c.z)
                    && !farm.golem_cells.contains(&c)
                {
                    farm.golem_cells.push(c);
                }
            }
        }
    }
    let mut interior: Vec<BlockPos> = Vec::new();
    for b in farm.villagers.iter().chain(farm.zombie.iter()) {
        for y in b.min_y.floor() as i32..=(b.max_y - 1.0e-6).floor() as i32 {
            for z in b.min_z.floor() as i32..=(b.max_z - 1.0e-6).floor() as i32 {
                for x in b.min_x.floor() as i32..=(b.max_x - 1.0e-6).floor() as i32 {
                    interior.push(BlockPos::new(x, y, z));
                }
            }
        }
    }
    for c in &farm.golem_cells {
        for dy in 0..3 {
            interior.push(c.offset(0, dy, 0));
        }
    }
    interior.sort_by_key(|p| (p.x, p.y, p.z));
    interior.dedup();
    farm.interior = interior;
    farm
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
    let block_entities = d.block_entities.iter().map(|(p, s)| (bp(*p), s.clone())).collect();
    Ok(FarmSpec { blocks, entities, output: bp(out), lo: bp(from), hi: bp(to), block_entities })
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
        JudgeOpts { seeds: JUDGE_SEEDS, days: JUDGE_DAYS, seed0: farm::JUDGE_SEED0, threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32) }
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
    let jobs: Vec<RunOpts> = (0..o.seeds).map(|k| farm::judge_opts(o.seed0 + build as i64 * 1000 + k as i64, o.days * 24000, walk)).collect();
    run_all(spec, &jobs).into_iter().collect()
}

/// The 9 worst-policy runs: still and the 8 directions, every golem of a run under that one
/// policy, all on the same seed.
pub fn worst_policy(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<(Walk, RunReport)>, String> {
    let policies = Walk::golem_policies();
    let jobs: Vec<RunOpts> = policies.iter().map(|p| farm::judge_opts(o.seed0 + 777, o.days * 24000, WalkMode::Fixed(*p))).collect();
    let runs: Vec<RunReport> = run_all(spec, &jobs).into_iter().collect::<std::result::Result<_, _>>()?;
    Ok(policies.into_iter().zip(runs).collect())
}

/// The restart probe: a run whose villagers have no `LAST_SLEPT` must spawn a golem within
/// [`RESTART_DAYS`] days.
pub fn restart_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<RunReport, String> {
    let mut ro = farm::judge_opts(o.seed0 + 999, RESTART_DAYS * 24000, WalkMode::Random);
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
    /// The player away for days, each way and back by day and at night (farms that claim to
    /// restart): [`absence_probe`].
    pub away: Vec<(Away, i64, RunReport, AwayReport)>,
    /// mobinfra: a natural day, a forced day of rain, a forced day of thunder: [`windows_probe`].
    pub windows: Vec<RunReport>,
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
        for (how, back, r, a) in &self.away {
            let when = away_when(*how, *back);
            if let Some(f) = r.failures.iter().chain(r.errors.iter()).next() {
                v.push(format!("{when}: {f}"));
            } else if a.first_golem.is_none() {
                v.push(format!("claims to restart, but {when} no golem spawned in the {AWAY_AFTER_DAYS} days after the return"));
            }
        }
        v
    }
}

/// Days the absence probe stays away, and runs after the return.
pub const AWAY_DAYS: i64 = 3;
pub const AWAY_AFTER_DAYS: i64 = 2;

fn away_when(how: Away, back: i64) -> String {
    format!("after {AWAY_DAYS} days away ({}, back {})", how.name(), if back == 6000 { "by day" } else { "at night" })
}

/// The absence probe (brief, Stages: Middle): a day of the steady state, then [`AWAY_DAYS`] days away
/// with the chunks unloaded or loaded but not ticking, back at noon or at midnight, then
/// [`AWAY_AFTER_DAYS`] days: the farm must spawn again by itself.
pub fn absence_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<(Away, i64, RunReport, AwayReport)>, String> {
    let cases: Vec<(Away, i64)> = [Away::Unloaded, Away::NotTicking].into_iter().flat_map(|h| [(h, 6000), (h, 18000)]).collect();
    let ro = farm::judge_opts(o.seed0 + 555, 24000, WalkMode::Random);
    let out: Vec<std::result::Result<(RunReport, AwayReport), String>> = std::thread::scope(|s| {
        let hs: Vec<_> = cases.iter().map(|&(how, back_at)| s.spawn(move || farm::run_away(spec, &ro, AwayPlan { before: 24000, days: AWAY_DAYS, back_at, how, after: AWAY_AFTER_DAYS * 24000 }))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("absence run panicked".into()))).collect()
    });
    cases.into_iter().zip(out).map(|((how, back), r)| r.map(|(rep, a)| (how, back, rep, a))).collect()
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
    let away = if claims_restart(c) { absence_probe(spec, o)? } else { Vec::new() };
    let windows = windows_probe(spec, o)?;
    Ok(BaseChecks { exit, worst, restart, away, windows })
}

/// Runs of the rain and thunder probe.
pub const WINDOW_RUNS: u32 = 16;

/// The rain and thunder probe (mobinfra; the card's `rain` line): [`WINDOW_RUNS`] runs of a
/// natural day, then a forced day of rain, then a forced day of thunder (`weather::Plan::Windows`,
/// from the cycle's stationary state, as the game gate runs them).
pub fn windows_probe(spec: &FarmSpec, o: &JudgeOpts) -> std::result::Result<Vec<RunReport>, String> {
    let plan = redsim::weather::Plan::Windows { at: 24000, rain: 24000, thunder: 24000 };
    let jobs: Vec<RunOpts> = (0..WINDOW_RUNS).map(|k| farm::judge_opts(o.seed0 + 4242 + k as i64, 72000, WalkMode::Random).with_weather(plan)).collect();
    run_all(spec, &jobs).into_iter().collect()
}

/// "about every N game days" (or "never") for a rate a game day.
fn every(per_day: f64) -> String {
    if per_day <= 0.0 { "never".to_string() } else { format!("about every {} game days", days(1.0 / per_day)) }
}

fn days(d: f64) -> String {
    if d >= 9.5 { format!("{d:.0}") } else { format!("{d:.1}") }
}

/// A small expected count with two significant digits (0.0041, 0.27, 3.6, 12).
fn small(x: f64) -> String {
    if x >= 9.5 {
        format!("{x:.0}")
    } else if x >= 0.095 || x == 0.0 {
        format!("{x:.1}")
    } else {
        let digits = (-(x.log10().floor()) as usize) + 1;
        format!("{x:.digits$}")
    }
}

/// The card's `lightning` line (one sentence, `;` between its parts: the site splits a card's
/// metrics at ", ").
pub fn lightning_line(m: &redsim::weather::lightning::Measure) -> String {
    let s = redsim::weather::storms();
    let per_day = s.share * 24000.0;
    let mut v = vec![
        format!("about {} iron lost per thunderstorm", small(m.iron * s.mean_len)),
        format!("a witch {}", every(m.witch * per_day)),
        format!("the zombie's boat broken {}", every(m.vehicle * per_day)),
        format!("a fire beside a flammable block {}", every(m.fire * per_day)),
        format!("a skeleton trap horse on the farm {}", every(m.trap * per_day)),
    ];
    if m.breaks > 0.0 {
        let first = [(m.witch, "a villager made a witch"), (m.vehicle, "the zombie's boat broken"), (m.fire, "a fire")].into_iter().max_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1).unwrap_or("");
        v.push(format!("the farm breaks in about {} game days ({first} first)", days(1.0 / (m.breaks * per_day))));
    } else {
        v.push("lightning never breaks the farm".to_string());
    }
    v.push(format!("thunderstorms {:.1}% of the time ({:.0} gt each on average)", 100.0 * s.share, s.mean_len));
    v.join("; ")
}

/// Iron lost for each hour of thunder: the bolts' and, when one breaks the farm, its rate `iron`
/// (an hour) until the end of that game day (half a day on average: 1/6 h).
pub fn thunder_loss(m: &redsim::weather::lightning::Measure, iron: f64) -> f64 {
    72000.0 * (m.iron + m.breaks * iron / 6.0)
}

/// The card's `fire` line: whether lava's random tick can ever burn one of the farm's blocks.
fn fire_line(spec: &FarmSpec, runs: &[RunReport]) -> String {
    let Ok(run) = farm::Run::new(spec, &RunOpts::steady(0, 0, WalkMode::Random)) else { return "never".to_string() };
    match redsim::weather::fire::lava_can_burn(&run.world) {
        None => "never".to_string(),
        Some((lava, at, q, b)) => {
            let name = b.to_string();
            let name = name.split('[').next().unwrap_or("").trim_start_matches("minecraft:").to_string();
            let hours: f64 = runs.iter().map(|r| r.hours()).sum();
            format!("lava at {} {} {} can light a fire at {} {} {} beside the {name} at {} {} {} (not within the runs' {hours:.0} h)", lava.x, lava.y, lava.z, at.x, at.y, at.z, q.x, q.y, q.z)
        }
    }
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
        for (how, back, r, a) in &b.away {
            let first = a.first_golem.map(|t| format!("first golem {t} gt after the return")).unwrap_or_else(|| "no golem after the return".into());
            table.push(format!("{}; {first}, {} golems", run_line(&away_when(*how, *back), r), a.golems));
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
        let firsts: Vec<f64> = bc.away.iter().filter_map(|(_, _, _, a)| a.first_golem).map(|t| t as f64 / 72000.0).collect();
        let away = if bc.away.is_empty() || firsts.len() < bc.away.len() {
            "no".to_string()
        } else {
            format!("yes (unloaded or not ticking, back by day or at night: the first golem {:.1}-{:.1} h after the return)", firsts.iter().cloned().fold(f64::INFINITY, f64::min), firsts.iter().cloned().fold(0.0, f64::max))
        };
        metrics.push(("restarts after days away".to_string(), away));
        // mobinfra: the weather's tradeoffs, measured (vjeux 2026-11-08 08:26: "You can't expect
        // day 1 farm to pass everything. It's all about tradeoffs")
        let (golems_all, iron_all): (u32, u32) = all.iter().fold((0, 0), |(g, i), r| (g + r.golems, i + r.iron_dropped));
        let per_golem = if golems_all > 0 { iron_all as f64 / golems_all as f64 } else { 4.0 };
        if !bc.windows.is_empty() {
            let rate = |a: i64| {
                let a = farm::SETTLE_TICKS + a;
                mean(&bc.windows.iter().map(|r| r.spawn_times.iter().filter(|t| (a..a + 24000).contains(*t)).count() as f64 * 3.0 * per_golem).collect::<Vec<_>>())
            };
            metrics.push(("rain".to_string(), format!("clear day {:.0} iron/h; rain day {:.0} iron/h; thunder day {:.0} iron/h", rate(0), rate(24000), rate(48000))));
        }
        if let Ok((m, sky)) = redsim::weather::lightning::measure_farm(spec, runs) {
            metrics.push(("lightning".to_string(), lightning_line(&m)));
            metrics.push(("thunder loss".to_string(), format!("{:.1}", thunder_loss(&m, iron))));
            metrics.push(("sunburn".to_string(), if sky { "the zombie sees the sky: it burns on its first clear day".to_string() } else { "never (the zombie never sees the sky)".to_string() }));
        }
        metrics.push(("fire".to_string(), fire_line(spec, all)));
    }
    metrics.push(("misses".to_string(), format!("{no_spot} of {tries} tries found no spot, {seen} golem sightings")));
    metrics.push(("bill".to_string(), b.text()));
    metrics.push(("payback".to_string(), format!("{payback:.0} min")));
    // the build effort and day 1's rank (brief revision 8, Stages: Day 1): 2 s a block placed
    // (gathering included), 1 s a block dug (none in the void), 120 s a lava trip, 180 s a
    // villager moved; then the minutes to own a 24-iron kit at the card's rate
    let placed = c.clock.design.blocks.values().filter(|s| !matches!(s.split('[').next().unwrap_or("").trim_start_matches("minecraft:"), "air" | "cave_air" | "void_air")).count();
    let build_s = 2.0 * placed as f64 + 120.0 * b.lava as f64 + 180.0 * b.villagers as f64;
    metrics.push(("build time".to_string(), format!("{:.0} min ({placed} placed, 0 dug, {} lava trips, {} villagers moved)", build_s / 60.0, b.lava, b.villagers)));
    if iron > 0.0 {
        metrics.push(("time to an iron kit".to_string(), format!("{:.0} min (the build, then (bill + 24) / rate)", build_s / 60.0 + (b.iron + 24.0) / iron * 60.0)));
    }
    // the stage lines (brief, Stages: What every card reports)
    metrics.push(("tools".to_string(), b.tools_text()));
    metrics.push(("lava sources".to_string(), b.lava.to_string()));
    metrics.push(("materials".to_string(), b.stage_text()));
    metrics.push(("chunk columns".to_string(), chunk_columns(c).to_string()));
    let poppies = mean(&all.iter().map(|r| r.poppies_dropped as f64 / r.hours()).collect::<Vec<_>>());
    let (slots, hoppers) = storage_slots(c);
    let hours = if iron + poppies > 0.0 { (slots as f64 - 2.0).max(0.0) * 64.0 / (iron + poppies) } else { 0.0 };
    metrics.push(("storage".to_string(), format!("{hours:.1} h ({slots} slots; {poppies:.0} poppies/h)")));
    metrics.push(("hoppers per 1000 iron/h".to_string(), format!("{:.1}", if iron > 0.0 { hoppers as f64 * 1000.0 / iron } else { 0.0 })));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The fewest chunk columns the design's footprint covers, over every placement on the chunk grid.
pub fn chunk_columns(c: &Circuit) -> u32 {
    let (lo, hi) = c.bounds();
    let cover = |a: i32, b: i32| (0..16).map(|o| ((b + o).div_euclid(16) - (a + o).div_euclid(16) + 1) as u32).min().unwrap_or(1);
    cover(lo[0], hi[0]) * cover(lo[2], hi[2])
}

/// The slots the farm can store its drops in (every chest, barrel and hopper of the design, and
/// the judge's chest on an air output), and its hoppers.
pub fn storage_slots(c: &Circuit) -> (u32, u32) {
    let d = &c.clock.design;
    let (mut slots, mut hoppers) = (0u32, 0u32);
    for s in d.blocks.values() {
        match s.split('[').next().unwrap_or("").trim_start_matches("minecraft:") {
            "chest" | "trapped_chest" | "barrel" => slots += 27,
            "hopper" => {
                slots += 5;
                hoppers += 1;
            }
            _ => {}
        }
    }
    if d.ports.get("output").is_some_and(|o| !d.blocks.contains_key(o)) {
        slots += 27;
    }
    (slots, hoppers)
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
    /// The iron, the tools the build needs counted (`tools`).
    pub iron: f64,
    pub quartz: u32,
    pub villagers: u32,
    pub beds: u32,
    pub zombies: u32,
    /// The tools the build needs and their iron (brief, Stages: the bucket for lava or water, the
    /// shears for leaves, an iron pickaxe for redstone dust).
    pub tools: Vec<(&'static str, u32)>,
    /// Lava sources (each one bucket trip).
    pub lava: u32,
    /// The materials that need the Nether (quartz, slime, magma, glowstone, obsidian...) and the
    /// End (shulker boxes, end rods...), with their counts.
    pub nether: std::collections::BTreeMap<&'static str, u32>,
    pub end: std::collections::BTreeMap<&'static str, u32>,
}

impl Bill {
    pub fn text(&self) -> String {
        let s = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!("{:.0} iron, {} quartz, {}, {}, {}", self.iron.ceil(), self.quartz, s(self.villagers, "villager", "villagers"), s(self.beds, "bed", "beds"), s(self.zombies, "zombie", "zombies"))
    }

    /// The tools line: `bucket 3 + shears 2` (none: `none`; ` + `, since a card's metrics are
    /// separated by commas).
    pub fn tools_text(&self) -> String {
        if self.tools.is_empty() {
            return "none".into();
        }
        self.tools.iter().map(|(t, n)| format!("{t} {n}")).collect::<Vec<_>>().join(" + ")
    }

    /// The latest stage its materials need, with the blocks that need it (`day 1`, `Nether:
    /// quartz in 2 blocks + slime in 4 blocks`).
    pub fn stage_text(&self) -> String {
        let list = |m: &std::collections::BTreeMap<&str, u32>| m.iter().map(|(k, n)| format!("{k} in {n} block{}", if *n == 1 { "" } else { "s" })).collect::<Vec<_>>().join(" + ");
        if !self.end.is_empty() {
            format!("End: {}", list(&self.end))
        } else if !self.nether.is_empty() {
            format!("Nether: {}", list(&self.nether))
        } else {
            "day 1".into()
        }
    }
}

/// Blocks whose recipe needs redstone dust (an iron pickaxe: `redstone_ore` is `needs_iron_tool`).
const DUST: &[&str] = &["redstone_wire", "redstone_torch", "redstone_wall_torch", "repeater", "comparator", "observer", "piston", "sticky_piston", "dispenser", "dropper", "note_block", "redstone_lamp", "target", "powered_rail", "detector_rail", "activator_rail", "crafter", "redstone_block"];

/// The materials of a block that need the Nether or the End (brief, Stages: the middle stage has
/// the Nether, the late one the End).
fn stage_materials(id: &str) -> (Option<&'static str>, Option<&'static str>) {
    match id {
        "comparator" | "observer" | "daylight_detector" | "quartz_block" | "smooth_quartz" | "quartz_slab" => (Some("quartz"), None),
        "slime_block" | "sticky_piston" => (Some("slime"), None),
        "magma_block" => (Some("magma"), None),
        "glowstone" | "redstone_lamp" => (Some("glowstone"), None),
        "obsidian" | "crying_obsidian" => (Some("obsidian"), None),
        "soul_sand" | "soul_soil" => (Some("soul sand"), None),
        "netherrack" | "nether_bricks" => (Some("netherrack"), None),
        id if id.ends_with("shulker_box") => (None, Some("shulker box")),
        "end_rod" => (None, Some("end rod")),
        "purpur_block" | "end_stone" | "end_stone_bricks" => (None, Some("end stone")),
        _ => (None, None),
    }
}

/// The survival cost of a design's blocks and entities: iron by recipe (hopper 5, piston 1,
/// iron trapdoor 4, minecart 5, rails 6 per 16, tripwire hooks 1 per 2) and the tools the build
/// needs (a bucket 3 for the lava and water, shears 2 for leaves, an iron pickaxe 3 for redstone
/// dust), quartz (daylight detector 3, observer 1, comparator 1), villagers, beds, zombies, lava
/// sources, and the Nether and End materials (brief, Stages).
pub fn bill(c: &Circuit) -> Bill {
    let d = &c.clock.design;
    let mut b = Bill::default();
    let (mut bucket, mut leaves, mut dust) = (false, false, false);
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
            "crafter" => b.iron += 5.0,
            "daylight_detector" => b.quartz += 3,
            "observer" | "comparator" => b.quartz += 1,
            "lava" => {
                bucket = true;
                if s.contains("level=0") {
                    b.lava += 1;
                }
            }
            "water" => bucket = true,
            id if id.ends_with("_bed") && s.contains("part=head") => b.beds += 1,
            _ => {}
        }
        leaves |= id.ends_with("_leaves");
        dust |= DUST.contains(&id);
        let (nether, end) = stage_materials(id);
        if let Some(m) = nether {
            *b.nether.entry(m).or_default() += 1;
        }
        if let Some(m) = end {
            *b.end.entry(m).or_default() += 1;
        }
    }
    for (need, tool, iron) in [(bucket, "bucket", 3), (leaves, "shears", 2), (dust, "iron pickaxe", 3)] {
        if need {
            b.iron += iron as f64;
            b.tools.push((tool, iron));
        }
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
        // mobinfra (vjeux 2026-11-08 08:26, "It's all about tradeoffs"): lightning is measured on
        // the card, never a refusal (`redsim::weather::lightning::check_farm` lists the bolts
        // that can fall on a build; their rates per thunderstorm become the card's lines)
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

/// `circuit verify` (and so `circuit archive`): the redsim judge (the card), confirmed in the real
/// 26.3 server by the mob farms' game gate (mobinfra, `crate::mobfarm_game`: each build for game
/// days with the weather on, on Hard).
pub fn verify_in_pool(_pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let rep = sim_verify(c, o).map_err(|e| anyhow!(e))?;
    crate::mobfarm_game::confirm(c, &o.version, rep)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The tier is the kind's last word (one site page per tier): day-1, middle, fastest, or none.
    #[test]
    fn the_tier_is_part_of_the_kind() {
        assert_eq!(parse_kind(&["iron_ingot", "middle"]).unwrap(), vec!["iron_ingot".to_string(), "middle".to_string()]);
        assert_eq!(parse_kind(&["minecraft:iron_ingot"]).unwrap(), vec!["iron_ingot".to_string()]);
        assert!(parse_kind(&["iron_ingot", "slowest"]).is_err());
        assert!(parse_kind(&["iron_ingot", "middle", "extra"]).is_err());
        let args = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let m = |k: &str| if k == "modules" { "4".to_string() } else { String::new() };
        let axis = STUDY.kinds[0].axis;
        assert_eq!(axis(&args(&["iron_ingot", "day-1"]), &m), "day 1");
        assert_eq!(axis(&args(&["iron_ingot", "fastest"]), &m), "fastest");
        assert_eq!(axis(&args(&["iron_ingot"]), &m), "4 modules");
        let title = STUDY.kinds[0].site.title;
        assert!(title("mob-farm iron_ingot fastest").starts_with("Fastest iron farm"));
        assert_eq!(title("mob-farm iron_ingot"), "Iron farm");
    }
}
