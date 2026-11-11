//! The weather model's validations against the official 26.3 server (mobinfra; the task's
//! "Validation against the game"): `mctas mobfarm weather-stats` (the cycle's statistics over long
//! runs), `mctas mobfarm fire-burn` (how long lava's fire takes to burn a farm's flammable block,
//! iron-middle's oak fence gates), each the game's numbers beside redsim's.

use anyhow::{Result, anyhow, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::entity::mob::farm::{self, RunOpts, WalkMode};
use redsim::weather::{Plan, Weather};
use redsim::{Block, BlockPos, World};
use std::path::Path;

/// A plains void superflat (it rains there), frozen, the bot in, then `tick rate 10000`.
fn plains_server(version: &str, difficulty: &str) -> Result<VanillaServer> {
    let v = VanillaOptions { version: version.to_string(), heap_mb: 1536, difficulty: difficulty.into(), biome: "minecraft:plains".into(), view_distance: 6, simulation_distance: 6, ..Default::default() };
    let mut s = VanillaServer::start(&v)?;
    s.cmd_ok("tick rate 10000")?;
    Ok(s)
}

/// Raining and thundering now (`weather_check`: `Level.isRaining`, `isThundering`).
fn weather_now(s: &mut VanillaServer) -> Result<(bool, bool)> {
    let r = s.cmd("execute if predicate {type:\"minecraft:weather_check\",raining:true}")?.contains("passed");
    let t = s.cmd("execute if predicate {type:\"minecraft:weather_check\",thundering:true}")?.contains("passed");
    Ok((r, t))
}

/// The statistics of one weather record sampled every `dt` gt.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub samples: u64,
    pub raining: u64,
    pub thundering: u64,
    /// Complete spells (gt): raining, dry, thundering, not thundering (the first and the last of
    /// each kind dropped: they are cut by the record).
    pub rain: Vec<f64>,
    pub dry: Vec<f64>,
    pub thunder: Vec<f64>,
    pub calm: Vec<f64>,
}

impl Record {
    fn from(samples: &[(bool, bool)], dt: u32) -> Record {
        let mut r = Record { samples: samples.len() as u64, ..Default::default() };
        r.raining = samples.iter().filter(|s| s.0).count() as u64;
        r.thundering = samples.iter().filter(|s| s.1).count() as u64;
        let spells = |pick: &dyn Fn(&(bool, bool)) -> bool| -> (Vec<f64>, Vec<f64>) {
            let (mut on, mut off) = (Vec::new(), Vec::new());
            let mut runs: Vec<(bool, u64)> = Vec::new();
            for s in samples {
                let v = pick(s);
                match runs.last_mut() {
                    Some((b, n)) if *b == v => *n += 1,
                    _ => runs.push((v, 1)),
                }
            }
            if runs.len() > 2 {
                for (b, n) in &runs[1..runs.len() - 1] {
                    let len = (*n * dt as u64) as f64;
                    if *b { on.push(len) } else { off.push(len) }
                }
            }
            (on, off)
        };
        (r.rain, r.dry) = spells(&|s: &(bool, bool)| s.0);
        (r.thunder, r.calm) = spells(&|s: &(bool, bool)| s.1);
        r
    }

    fn merge(&mut self, o: &Record) {
        self.samples += o.samples;
        self.raining += o.raining;
        self.thundering += o.thundering;
        self.rain.extend(&o.rain);
        self.dry.extend(&o.dry);
        self.thunder.extend(&o.thunder);
        self.calm.extend(&o.calm);
    }
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
}

fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Mean with its 95% interval, `a +- b`.
fn ci(v: &[f64]) -> String {
    format!("{:.0} +- {:.0} (n {})", mean(v), 1.96 * sd(v) / (v.len().max(1) as f64).sqrt(), v.len())
}

/// The two means' difference in standard errors.
fn z(a: &[f64], b: &[f64]) -> f64 {
    let se = (sd(a).powi(2) / a.len().max(1) as f64 + sd(b).powi(2) / b.len().max(1) as f64).sqrt();
    (mean(a) - mean(b)) / se
}

/// One official server's weather record: `days` game days sampled every `dt` gt (an empty world:
/// nothing but the weather and the bot).
fn game_record(version: &str, days: u32, dt: u32) -> Result<Record> {
    let mut s = plains_server(version, "peaceful")?;
    s.cmd_ok("gamerule advance_weather true")?;
    let n = days as u64 * 24000 / dt as u64;
    let mut samples = Vec::with_capacity(n as usize);
    for _ in 0..n {
        s.step(dt)?;
        samples.push(weather_now(&mut s)?);
    }
    Ok(Record::from(&samples, dt))
}

/// redsim's record over the same length: the cycle from a fresh world (`Weather` on, its timers at
/// 0, the game's start), sampled the same way.
fn sim_record(seed: i64, days: u32, dt: u32) -> Record {
    let mut w = Weather { on: true, rng: redsim::rng::LegacyRandom::new(seed), ..Weather::default() };
    let _ = Plan::Cycle;
    let n = days as u64 * 24000 / dt as u64;
    let mut samples = Vec::with_capacity(n as usize);
    for _ in 0..n {
        for _ in 0..dt {
            w.advance();
        }
        samples.push((w.is_raining(), w.is_raining() && w.is_thundering()));
    }
    Record::from(&samples, dt)
}

/// `mctas mobfarm weather-stats`.
pub fn weather_stats_cmd(version: &str, days: u32, servers: usize, dt: u32, sim_runs: u32) -> Result<()> {
    let t0 = std::time::Instant::now();
    let games: Vec<Result<Record>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..servers).map(|_| sc.spawn(|| game_record(version, days, dt))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("a server panicked")))).collect()
    });
    let mut game = Record::default();
    let mut game_shares = Vec::new();
    for g in games {
        let g = g?;
        game_shares.push(g.raining as f64 / g.samples.max(1) as f64);
        game.merge(&g);
    }
    let mut sim = Record::default();
    let mut sim_shares = Vec::new();
    for k in 0..sim_runs {
        let r = sim_record(0x7765_6174 + k as i64, days, dt);
        sim_shares.push(r.raining as f64 / r.samples.max(1) as f64);
        sim.merge(&r);
    }
    let pct = |a: u64, b: u64| 100.0 * a as f64 / b.max(1) as f64;
    println!("weather over {servers} official servers x {days} game days (sampled every {dt} gt) and {sim_runs} redsim runs of the same length, from a fresh world:");
    println!("  raining: game {:.2}% (per server sd {:.2}), redsim {:.2}% (sd {:.2}), z {:.2}", pct(game.raining, game.samples), 100.0 * sd(&game_shares), pct(sim.raining, sim.samples), 100.0 * sd(&sim_shares), z(&game_shares, &sim_shares));
    println!("  thundering: game {:.2}%, redsim {:.2}%", pct(game.thundering, game.samples), pct(sim.thundering, sim.samples));
    for (name, g, s) in [("rain spell", &game.rain, &sim.rain), ("dry spell", &game.dry, &sim.dry), ("thunder spell", &game.thunder, &sim.thunder), ("between thunder", &game.calm, &sim.calm)] {
        println!("  {name} (gt): game {}, redsim {}, z {:.2}", ci(g), ci(s), z(g, s));
    }
    println!("{:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}

/// One game trial of lava's fire: the farm's settled blocks pasted (no mobs, no weather, the day
/// clock stopped), the bot 60 blocks under the chest; the first game tick (sampled every 20) a
/// flammable block of the build is gone, and what it was.
fn game_burn(version: &str, blocks: &[([i32; 3], String)], watch: &[([i32; 3], String)], output: [i32; 3], lo: [i32; 3], hi: [i32; 3], max: u32) -> Result<Option<(u32, String)>> {
    let mut s = plains_server(version, "hard")?;
    for c in ["gamerule advance_weather false", "gamerule advance_time false", "gamerule spawn_mobs false", "weather clear"] {
        s.cmd_ok(c)?;
    }
    let bot = [output[0], lo[1] - 60, output[2]];
    s.cmd_ok(&format!("forceload add {} {} {} {}", lo[0] - 8, lo[2] - 8, hi[0] + 8, hi[2] + 8))?;
    s.cmd_ok(&format!("setblock {} {} {} minecraft:stone strict", bot[0], bot[1] + 3, bot[2]))?;
    s.teleport_bot(bot[0] as f64 + 0.5, bot[1] as f64, bot[2] as f64 + 0.5)?;
    for (p, st) in blocks {
        let out = s.cmd(&format!("setblock {} {} {} {st} strict", p[0], p[1], p[2]))?;
        if !out.contains("Changed the block") {
            bail!("setblock {p:?} {st}: {out}");
        }
    }
    let mut t = 0;
    while t < max {
        s.step(20)?;
        t += 20;
        for (p, id) in watch {
            if !id.contains("lava") && !s.cmd(&format!("execute if block {} {} {} {id}", p[0], p[1], p[2]))?.contains("passed") {
                return Ok(Some((t, format!("{} at {} {} {}", id.trim_start_matches("minecraft:"), p[0], p[1], p[2]))));
            }
        }
    }
    Ok(None)
}

/// One redsim trial: the same blocks pasted raw into a fresh world with the farm judge's AFK
/// player and random ticks, ticked until a flammable block burns.
fn sim_burn(spec: &farm::FarmSpec, cells: &[(BlockPos, Block)], seed: i64, max: u32) -> Option<(u32, String)> {
    let size = [spec.hi.x - spec.lo.x + 1, spec.hi.y - spec.lo.y + 1, spec.hi.z - spec.lo.z + 1];
    let mut w = World::new(spec.lo, size);
    w.place_raw(cells);
    w.afk = Some(redsim::entity::Vec3::new(spec.output.x as f64 + 0.5, (spec.lo.y - 60) as f64, spec.output.z as f64 + 0.5));
    w.random_ticks = Some(redsim::crops::RandomTicks::new(3, seed));
    w.weather = Weather::new(Plan::Off, seed);
    for t in 1..=max {
        w.tick();
        if let Some(redsim::weather::WeatherEvent::Burned { at, what, .. }) = w.weather.events.iter().find(|e| matches!(e, redsim::weather::WeatherEvent::Burned { .. })) {
            return Some((t, format!("{} at {} {} {}", what.split('[').next().unwrap_or(what).trim_start_matches("minecraft:"), at[0], at[1], at[2])));
        }
        w.weather.events.clear();
    }
    None
}

/// `mctas mobfarm fire-burn <design>`: lava's fire in the game and in redsim, the time to the
/// first burned block.
pub fn fire_burn_cmd(version: &str, design: &Path, servers: usize, max: u32, seeds: u32) -> Result<()> {
    let t0 = std::time::Instant::now();
    let c = circuit::Circuit::load(design)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let o = RunOpts::steady(1, 0, WalkMode::Random);
    let mut run = farm::Run::start(&spec, &o).map_err(|e| anyhow!(e))?;
    while run.settle_tick() {}
    let mut cells: Vec<(BlockPos, Block)> = Vec::new();
    let mut blocks: Vec<([i32; 3], String)> = Vec::new();
    let mut watch: Vec<([i32; 3], String)> = Vec::new();
    for y in spec.lo.y..=spec.hi.y {
        for z in spec.lo.z..=spec.hi.z {
            for x in spec.lo.x..=spec.hi.x {
                let p = BlockPos::new(x, y, z);
                let b = run.world.get(p);
                // the settled build without its fires (none at rest) and detectors (none yet)
                if b.is_air() || matches!(b, Block::Fire { .. }) {
                    continue;
                }
                cells.push((p, b));
                let st = b.to_string();
                if redsim::weather::lightning::flammable(&b).is_some() {
                    watch.push(([x, y, z], st.split('[').next().unwrap_or(&st).to_string()));
                }
                blocks.push(([x, y, z], st));
            }
        }
    }
    if watch.is_empty() {
        bail!("nothing flammable in the build");
    }
    let output = [spec.output.x, spec.output.y, spec.output.z];
    let (lo, hi) = ([spec.lo.x, spec.lo.y, spec.lo.z], [spec.hi.x, spec.hi.y, spec.hi.z]);
    let games: Vec<Result<Option<(u32, String)>>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..servers).map(|_| sc.spawn(|| game_burn(version, &blocks, &watch, output, lo, hi, max))).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow!("a server panicked")))).collect()
    });
    let mut game_t = Vec::new();
    for g in games {
        match g? {
            Some((t, what)) => {
                println!("game: {what} burned at gt {t}");
                game_t.push(t as f64);
            }
            None => println!("game: nothing burned in {max} gt"),
        }
    }
    let sims: Vec<Option<(u32, String)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..seeds).map(|k| {
            let (spec, cells) = (&spec, &cells);
            sc.spawn(move || sim_burn(spec, cells, 0x6669_7265 + k as i64, max))
        }).collect();
        hs.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    });
    let mut sim_t: Vec<f64> = sims.iter().flatten().map(|(t, _)| *t as f64).collect();
    let unburned = sims.iter().filter(|s| s.is_none()).count();
    let mut firsts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (_, what) in sims.iter().flatten() {
        *firsts.entry(what.clone()).or_default() += 1;
    }
    sim_t.sort_by(f64::total_cmp);
    let med = |v: &[f64]| if v.is_empty() { f64::NAN } else { v[v.len() / 2] };
    let mut g_sorted = game_t.clone();
    g_sorted.sort_by(f64::total_cmp);
    println!("game: {} of {servers} burned within {max} gt, first burn mean {} gt, median {:.0}", game_t.len(), ci(&game_t), med(&g_sorted));
    println!("redsim: {} of {seeds} burned within {max} gt, first burn mean {} gt, median {:.0} (sampled per tick; the game per 20 gt: +10 on average)", sim_t.len(), ci(&sim_t), med(&sim_t));
    println!("redsim's first blocks: {}", firsts.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(", "));
    if unburned > 0 {
        println!("redsim: {unburned} runs without a burn");
    }
    println!("z (game - redsim - 10) {:.2}", {
        let shifted: Vec<f64> = sim_t.iter().map(|t| t + 10.0).collect();
        z(&game_t, &shifted)
    });
    println!("{:.0} s", t0.elapsed().as_secs_f64());
    Ok(())
}

/// One bolt scenario: blocks and entities around an origin, and where the bolt is summoned
/// (relative to the origin).
struct Scenario {
    name: &'static str,
    blocks: Vec<([i32; 3], &'static str)>,
    entities: Vec<(&'static str, [f64; 3], Option<&'static str>)>,
    bolt: [i32; 3],
}

fn scenarios() -> Vec<Scenario> {
    let roof = |y: i32| (-1..=1).flat_map(move |x| (-1..=1).map(move |z| ([x, y, z], "minecraft:stone"))).collect::<Vec<_>>();
    let villager = vec![("minecraft:villager", [0.5, 0.0, 0.5], None)];
    vec![
        Scenario { name: "villager in the open, the bolt on it", blocks: vec![], entities: villager.clone(), bolt: [0, 0, 0] },
        Scenario { name: "villager, a stone roof 2 above its feet, the bolt on the roof", blocks: roof(2), entities: villager.clone(), bolt: [0, 3, 0] },
        Scenario { name: "villager, a stone roof 6 above its feet, the bolt on the roof", blocks: roof(6), entities: villager.clone(), bolt: [0, 7, 0] },
        Scenario { name: "villager, the bolt 3 blocks east", blocks: vec![], entities: villager.clone(), bolt: [3, 0, 0] },
        Scenario { name: "villager, the bolt 4 blocks east", blocks: vec![], entities: villager, bolt: [4, 0, 0] },
        Scenario { name: "zombie in a boat, the bolt on it", blocks: vec![], entities: vec![("minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"))], bolt: [0, 0, 0] },
        // (tickexact 2026-11-09, carousel-gate's station) the rider of a destroyed boat under a
        // roof: LivingEntity.dismountVehicle's free-position search moves it below the roof
        Scenario {
            name: "zombie in a boat, a stone roof 2 above, the bolt on the roof",
            blocks: roof(2),
            entities: vec![("minecraft:oak_boat", [0.5, 0.0, 0.5], Some("{Passengers:[{id:\"minecraft:zombie\"}]}"))],
            bolt: [0, 3, 0],
        },
        Scenario { name: "an iron ingot 2 east of the bolt", blocks: vec![], entities: vec![("minecraft:item", [2.5, 0.0, 0.5], Some("{Item:{id:\"minecraft:iron_ingot\",count:1}}"))], bolt: [0, 0, 0] },
        Scenario { name: "iron golem, the bolt on it", blocks: vec![], entities: vec![("minecraft:iron_golem", [0.5, 0.0, 0.5], None)], bolt: [0, 0, 0] },
        // (tickexact 2026-11-11, ironspec's lightning reads (c)) a lightning rod topping a 1-wide
        // stone pillar, the bolt on the block above it (where a strike the rod draws lands):
        // LightningBolt.spawnFire's fire at the bolt's block needs a sturdy floor (the rod's top is
        // not) or a flammable neighbour, its 4 more sources (Normal/Hard) each a random cell within
        // 1 under the same test; the rod powered (onLightningStrike, 8 gt)
        Scenario { name: "a lightning rod on a bare 1-wide pillar, the bolt on the rod", blocks: rod_pillar(&[]), entities: vec![], bolt: [0, 3, 0] },
        Scenario { name: "a lightning rod on the pillar, oak planks beside the block above it", blocks: rod_pillar(&[([1, 3, 0], "minecraft:oak_planks")]), entities: vec![], bolt: [0, 3, 0] },
        Scenario {
            name: "a lightning rod on the pillar, stone all round the rod's top",
            blocks: rod_pillar(&[([1, 2, 0], "minecraft:stone"), ([-1, 2, 0], "minecraft:stone"), ([0, 2, 1], "minecraft:stone"), ([0, 2, -1], "minecraft:stone"), ([1, 2, 1], "minecraft:stone"), ([1, 2, -1], "minecraft:stone"), ([-1, 2, 1], "minecraft:stone"), ([-1, 2, -1], "minecraft:stone")]),
            entities: vec![],
            bolt: [0, 3, 0],
        },
    ]
}

/// (tickexact 2026-11-11) A lightning rod (facing up) on a 1-wide stone pillar at the scenario's
/// origin (stone at y 0 and 1, the rod at y 2), and more blocks.
fn rod_pillar(more: &[([i32; 3], &'static str)]) -> Vec<([i32; 3], &'static str)> {
    let mut v = vec![([0, 0, 0], "minecraft:stone"), ([0, 1, 0], "minecraft:stone"), ([0, 2, 0], "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]")];
    v.extend_from_slice(more);
    v
}

/// A scenario's lightning rod, if it has one.
fn rod_of(x: &Scenario) -> Option<[i32; 3]> {
    x.blocks.iter().find(|(_, s)| s.starts_with("minecraft:lightning_rod")).map(|(p, _)| *p)
}

/// What a scenario looks like 5 ticks after its bolt.
#[allow(clippy::too_many_arguments)]
fn outcome(villagers: usize, witches: usize, boats: usize, zombie_hp: Option<f32>, zombie_at: Option<[f64; 3]>, items: usize, golem_hp: Option<f32>, golem_burning: bool, fire: bool) -> String {
    let mut v = Vec::new();
    if villagers + witches > 0 {
        v.push(format!("{villagers} villager, {witches} witch"));
    }
    if let Some(h) = zombie_hp {
        // (the rider of a destroyed vehicle: where its dismount put it, from the scenario's
        // origin, and the vehicle's dropped item, burned or not)
        let at = zombie_at.map(|p| format!(" at {:.4},{:.4},{:.4}", p[0], p[1], p[2])).unwrap_or_default();
        v.push(format!("{boats} boat, zombie health {h:.0}{at}, {items} item"));
    }
    if let Some(h) = golem_hp {
        v.push(format!("golem health {h:.0}{}", if golem_burning { " burning" } else { "" }));
    }
    if villagers + witches == 0 && zombie_hp.is_none() && golem_hp.is_none() {
        v.push(format!("{items} item"));
    }
    v.push(format!("fire at the bolt {}", if fire { "yes" } else { "no" }));
    v.join(", ")
}

/// (tickexact 2026-11-11, ironspec's weather gap: the card's "clear day" against the headline) The
/// golems a game day, day by day, at the judge's site `build` of the design, under three weather
/// plans with the judge's run options: the card's windows probe (`Windows`: a natural day, then a
/// forced day of rain, then of thunder; its seeds), the headline's natural cycle (`Cycle`; the build
/// runs' seeds, `days` long) and no weather (`Off`; the same seeds). Each day's golems an hour over
/// the runs, with a 95% interval.
pub fn weather_days_cmd(spec: &farm::FarmSpec, site: &'static redsim::entity::mob::site::Site, build: u32, runs: u32, days: u32) -> Result<()> {
    let moved = site.spec(spec);
    let seed0 = farm::JUDGE_SEED0;
    let plans: Vec<(&str, Plan, Vec<i64>, u32)> = vec![
        ("windows (the card's: day 1 natural, 2 rain, 3 thunder)", Plan::Windows { at: 24000, rain: 24000, thunder: 24000 }, (0..runs).map(|k| seed0 + 4242 + k as i64).collect(), 3),
        ("cycle (the headline's)", Plan::Cycle, (0..runs).map(|k| seed0 + build as i64 * 1000 + k as i64).collect(), days),
        ("off (no weather)", Plan::Off, (0..runs).map(|k| seed0 + build as i64 * 1000 + k as i64).collect(), days),
    ];
    for (name, plan, seeds, d) in plans {
        let jobs: Vec<RunOpts> = seeds.iter().map(|s| farm::judge_opts_in(*s, d * 24000, WalkMode::Random, Some(site)).with_weather(plan)).collect();
        let reports: Vec<farm::RunReport> = circuit::mobfarm::run_all(&moved, &jobs).into_iter().collect::<std::result::Result<_, _>>().map_err(|e| anyhow!(e))?;
        let mut line = format!("{name}, {} runs x {d} days:", reports.len());
        for day in 0..d as i64 {
            let a = farm::SETTLE_TICKS + day * 24000;
            let per: Vec<f64> = reports.iter().map(|r| r.spawn_times.iter().filter(|t| (a..a + 24000).contains(*t)).count() as f64 * 3.0).collect();
            let n = per.len() as f64;
            let mean = per.iter().sum::<f64>() / n;
            let sd = (per.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
            line.push_str(&format!(" day {} {mean:.1} golems/h (+-{:.1})", day + 1, 1.96 * sd / n.sqrt()));
        }
        let mean_all = reports.iter().map(|r| r.golems_per_hour()).sum::<f64>() / reports.len() as f64;
        line.push_str(&format!("; whole runs {mean_all:.1} golems/h"));
        println!("{line}");
    }
    Ok(())
}

/// `mctas mobfarm bolt-check`: bolts summoned in fixed scenarios (hits through roofs, the box's
/// edge, a boat, a drop, a golem, the bolt's fire), the game's outcome beside redsim's.
pub fn bolt_check_cmd(version: &str) -> Result<()> {
    let sc = scenarios();
    let origin = |k: usize| [k as i32 * 24, 0, 0];
    // the game: one server, night (no sunburn), Hard, the bot under the middle within 128
    let mut s = plains_server(version, "hard")?;
    for c in ["gamerule advance_weather false", "gamerule advance_time false", "gamerule spawn_mobs false", "weather clear", "time set 18000"] {
        s.cmd_ok(c)?;
    }
    let mid = origin(sc.len() / 2)[0];
    s.cmd_ok(&format!("forceload add -16 -16 {} 16", origin(sc.len())[0]))?;
    s.cmd_ok(&format!("setblock {mid} -57 0 minecraft:stone strict"))?;
    s.teleport_bot(mid as f64 + 0.5, -60.0, 0.5)?;
    // let the forced chunks load to entity ticking before anything is summoned into them
    s.step(60)?;
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        for dx in -3..=3 {
            for dz in -3..=3 {
                s.cmd_ok(&format!("setblock {} -1 {} minecraft:stone strict", o[0] + dx, dz))?;
            }
        }
        for (p, st) in &x.blocks {
            s.cmd_ok(&format!("setblock {} {} {} {st} strict", o[0] + p[0], p[1], p[2]))?;
        }
        for (ty, p, nbt) in &x.entities {
            let data = nbt.map(|n| format!(" {n}")).unwrap_or_default();
            s.cmd_ok(&format!("summon {ty} {} {} {}{data}", o[0] as f64 + p[0], p[1], p[2]))?;
        }
    }
    s.step(1)?;
    // every summoned entity is there before the bolts fall
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        for (ty, _, _) in &x.entities {
            let n = s.count_entities(&format!("@e[type={ty},x={},y=-3,z=-6,dx=12,dy=14,dz=12]", o[0] - 6))?;
            if n == 0 {
                bail!("{}: the game lost the summoned {ty} before the bolt", x.name);
            }
        }
    }
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        s.cmd_ok(&format!("summon minecraft:lightning_bolt {} {} {}", (o[0] + x.bolt[0]) as f64 + 0.5, x.bolt[1], x.bolt[2] as f64 + 0.5))?;
    }
    // the bolts hit at their first tick (a vehicle destroyed, its rider dismounted): the riders'
    // positions then, before their own AI moves them (they tick before the bolt, summoned last)
    s.step(1)?;
    let mut riders_at = Vec::new();
    for k in 0..sc.len() {
        let o = origin(k);
        let zpos = s.cmd_long(&format!("execute as @e[type=minecraft:zombie,x={},y=-3,z=-6,dx=12,dy=14,dz=12] run data get entity @s Pos", o[0] - 6))?;
        riders_at.push(zpos.split("has the following entity data: ").nth(1).and_then(|v| {
            let v: Vec<f64> = v.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(|c| c.trim().trim_end_matches('d').parse().ok()).collect();
            (v.len() == 3).then(|| [v[0] - o[0] as f64, v[1], v[2]])
        }));
    }
    s.step(4)?;
    let mut game = Vec::new();
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        let sel = |ty: &str| format!("@e[type=minecraft:{ty},x={},y=-3,z=-6,dx=12,dy=14,dz=12]", o[0] - 6);
        let villagers = s.count_entities(&sel("villager"))? as usize;
        let witches = s.count_entities(&sel("witch"))? as usize;
        let boats = s.count_entities(&sel("oak_boat"))? as usize;
        let items = s.count_entities(&sel("item"))? as usize;
        let hp = |s: &mut VanillaServer, ty: &str| -> Result<Option<f32>> {
            let out = s.cmd_long(&format!("execute as {} run data get entity @s Health", sel(ty)))?;
            Ok(out.split("has the following entity data: ").nth(1).and_then(|v| v.trim().trim_end_matches('f').parse::<f32>().ok()))
        };
        let zombie_hp = hp(&mut s, "zombie")?;
        let zombie_at = riders_at[k];
        let golem_hp = hp(&mut s, "iron_golem")?;
        let fire_ticks = s.cmd_long(&format!("execute as {} run data get entity @s Fire", sel("iron_golem")))?;
        let golem_burning = fire_ticks.split("has the following entity data: ").nth(1).and_then(|v| v.trim().trim_end_matches('s').parse::<i32>().ok()).is_some_and(|f| f > 0);
        let b = [o[0] + x.bolt[0], x.bolt[1], x.bolt[2]];
        let fire = s.cmd(&format!("execute if block {} {} {} minecraft:fire", b[0], b[1], b[2]))?.contains("passed");
        let mut line = outcome(villagers, witches, boats, zombie_hp, zombie_at, items, golem_hp, golem_burning, fire);
        // (a rod scenario: the rod powered, and the fires within 1 of the bolt's block)
        if let Some(r) = rod_of(x) {
            let powered = s.cmd(&format!("execute if block {} {} {} minecraft:lightning_rod[powered=true]", o[0] + r[0], r[1], r[2]))?.contains("passed");
            let mut fires = 0;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        fires += s.cmd(&format!("execute if block {} {} {} minecraft:fire", b[0] + dx, b[1] + dy, b[2] + dz))?.contains("passed") as usize;
                    }
                }
            }
            line.push_str(&format!(", rod powered {} [info: fires within 1 of the bolt {fires}]", if powered { "yes" } else { "no" }));
        }
        game.push(line);
    }
    drop(s);
    // redsim: the same world, the same night
    let mut w = World::new(BlockPos::new(-8, -4, -8), [origin(sc.len())[0] + 16, 20, 17]);
    w.mob.seed(1, 18000);
    w.afk = Some(redsim::entity::Vec3::new(mid as f64 + 0.5, -60.0, 0.5));
    let place = |w: &mut World, p: [i32; 3], st: &str| -> Result<()> {
        let b = Block::parse(st).map_err(|e| anyhow!(e))?;
        w.set_block(BlockPos::new(p[0], p[1], p[2]), b, redsim::world::flags::RAW);
        Ok(())
    };
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        for dx in -3..=3 {
            for dz in -3..=3 {
                place(&mut w, [o[0] + dx, -1, dz], "minecraft:stone")?;
            }
        }
        for (p, st) in &x.blocks {
            place(&mut w, [o[0] + p[0], p[1], p[2]], st)?;
        }
        for (ty, p, nbt) in &x.entities {
            redsim::entity::spawn(&mut w, ty, [o[0] as f64 + p[0], p[1], p[2]], *nbt).map_err(|e| anyhow!("{ty}: {e}"))?;
        }
    }
    // the game's tick between the summons and the bolts
    w.tick();
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        redsim::weather::lightning::summon_bolt(&mut w, BlockPos::new(o[0] + x.bolt[0], x.bolt[1], x.bolt[2]));
    }
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    // the riders' positions after the bolts' first tick, as in the game
    w.tick();
    let riders_at: Vec<Option<[f64; 3]>> = (0..sc.len())
        .map(|k| {
            let o = origin(k);
            w.entities
                .list
                .iter()
                .filter(|e| !e.removed && (e.pos.x - (o[0] as f64 + 0.5)).abs() < 6.0)
                .find(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie))
                .map(|e| [e.pos.x - o[0] as f64, e.pos.y, e.pos.z])
        })
        .collect();
    for _ in 0..4 {
        w.tick();
    }
    let mut sim = Vec::new();
    for (k, x) in sc.iter().enumerate() {
        let o = origin(k);
        let near = |e: &redsim::entity::Entity| !e.removed && (e.pos.x - (o[0] as f64 + 0.5)).abs() < 6.0;
        let mobs = |ty: MobType| w.entities.list.iter().filter(|e| near(e) && matches!(&e.kind, EntityKind::Mob(m) if m.ty == ty)).count();
        let hp = |ty: MobType| w.entities.list.iter().filter(|e| near(e)).find_map(|e| if let EntityKind::Mob(m) = &e.kind { (m.ty == ty).then_some(m.health) } else { None });
        let boats = w.entities.list.iter().filter(|e| near(e) && matches!(e.kind, EntityKind::Boat(_))).count();
        let items = w.entities.list.iter().filter(|e| near(e) && matches!(e.kind, EntityKind::Item(_))).count();
        let golem_burning = w.entities.list.iter().filter(|e| near(e)).any(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::IronGolem && m.fire_ticks > 0));
        let b = BlockPos::new(o[0] + x.bolt[0], x.bolt[1], x.bolt[2]);
        let fire = matches!(w.get(b), Block::Fire { .. });
        let mut line = outcome(mobs(MobType::Villager), mobs(MobType::Witch), boats, hp(MobType::Zombie), riders_at[k], items, hp(MobType::IronGolem), golem_burning, fire);
        if let Some(r) = rod_of(x) {
            let powered = w.get(BlockPos::new(o[0] + r[0], r[1], r[2])).to_string().contains("powered=true");
            let mut fires = 0;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        fires += matches!(w.get(BlockPos::new(b.x + dx, b.y + dy, b.z + dz)), Block::Fire { .. }) as usize;
                    }
                }
            }
            line.push_str(&format!(", rod powered {} [info: fires within 1 of the bolt {fires}]", if powered { "yes" } else { "no" }));
        }
        sim.push(line);
    }
    // a model error is a difference too (a rider's dismount, a vehicle's drop)
    for e in &w.errors {
        println!("redsim error: {e}");
    }
    let mut same = 0;
    for (k, x) in sc.iter().enumerate() {
        // (a rod scenario's fire count rides after " [info": the bolt's 4 extra fire sources are
        // its own random draws, a vanilla summon's random seeded from the clock: not compared)
        let fixed = |s: &str| s.split(" [info").next().unwrap_or("").to_string();
        let ok = fixed(&game[k]) == fixed(&sim[k]);
        same += ok as usize;
        println!("{} {}: game: {} | redsim: {}", if ok { "SAME" } else { "DIFF" }, x.name, game[k], sim[k]);
    }
    println!("{same} of {} scenarios the same{}", sc.len(), if w.errors.is_empty() { "" } else { ", redsim reported model errors" });
    Ok(())
}
