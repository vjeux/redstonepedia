//! `mctas mobfarm ...`: the generators and the judge's tools.

use anyhow::{Result, bail};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum MobfarmCmd {
    /// Write a layout's design file: `l1` (the obvious bar), `l2` (a pad around a wet hole, a drop
    /// out of range), `l2d` (the dry-hole module: middle and fastest tiers), `l3` (day 1: spawn
    /// pits that burn the golems in place), `l4` (two banks of fence-gated platforms), `l5` (a
    /// grid of l2d modules at a 27-block pitch).
    Gen {
        layout: String,
        /// Spawn pits (l3).
        #[arg(long, default_value_t = 8)]
        pits: i32,
        /// The pad's side, even (l2, l2d, l5).
        #[arg(long, default_value_t = 4)]
        pad: i32,
        /// The pad's floor height, the villagers' beds at 0 (l2; spawn cells reach -6).
        #[arg(long, default_value_t = -4, allow_hyphen_values = true)]
        floor: i32,
        /// Villagers, 3 or 4 (l2, l2d, l5; 4 uses L4's front: three trapdoors before the zombie).
        #[arg(long, default_value_t = 3)]
        villagers: i32,
        /// Modules per side of the grid (l5).
        #[arg(long, default_value_t = 2)]
        grid: i32,
        /// The tier the design is published in (`day-1`, `middle`, `fastest`: the kind's last
        /// word, `circuit: mob-farm iron_ingot middle`); none: a farm in no tier (catalog only).
        #[arg(long)]
        tier: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// The golem exit flood of a design (every position a golem can stand on, still and 8
    /// inputs), with its spawn spots.
    Exit {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
    },
    /// The built farm (water settled) along a row: each cell's block from x0 to x1 at y, z.
    Probe {
        design: PathBuf,
        #[arg(long, allow_hyphen_values = true, num_args = 4)]
        row: Vec<i32>,
    },
    /// Edit a design in place, its caption and every other line kept: `--chest` puts the farm's
    /// single chest at its `port output` (the download then holds it), `--how` sets its `how:`
    /// line (how it works in a few words: the end of its site title).
    Edit {
        design: PathBuf,
        #[arg(long)]
        chest: bool,
        #[arg(long)]
        how: Option<String>,
    },
    /// The built farm's lines of sight: each villager's eye to the zombie's eye (the hostile
    /// sensor's test), as built and with every trapdoor forced shut and forced open.
    Sight { design: PathBuf },
    /// One judge run of a design on redsim: its table.
    Run {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        #[arg(long, default_value_t = 1)]
        days: u32,
        /// Every golem stands still (default: a random policy each).
        #[arg(long)]
        still: bool,
    },
    /// The brief's game check: the design run in the real server for `--days`, its golems (time
    /// and cell) and the iron in the chest, against redsim's runs of the same design.
    Validate {
        design: PathBuf,
        #[arg(long, default_value_t = 3)]
        days: u32,
        /// Kill each golem at once (the spawn-spot statistics).
        #[arg(long)]
        kill: bool,
        /// Redsim seeds to compare with.
        #[arg(long, default_value_t = 32)]
        seeds: u32,
        /// Print each villager's brain memories every 1000 gt.
        #[arg(long)]
        trace: bool,
        /// Game runs (each its own sandbox seed; the game's entity randoms are unseeded).
        #[arg(long, default_value_t = 1)]
        repeat: u32,
    },
}

fn load(_version: &str, path: &PathBuf) -> Result<(circuit::Circuit, redsim::entity::mob::farm::FarmSpec)> {
    let c = circuit::Circuit::load(path)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    Ok((c, spec))
}

pub fn run(version: &str, cmd: MobfarmCmd) -> Result<()> {
    match cmd {
        MobfarmCmd::Gen { layout, pits, pad, floor, villagers, grid, tier, out } => {
            let (text, how) = match layout.as_str() {
                "l3" => (crate::layouts::l3(pits), "golems burn in lava pits".to_string()),
                "l1" => (crate::layouts::l1(), "water carries golems to a lava blade in the villagers' sight".to_string()),
                "l2" => (crate::layouts::l2(pad, floor, villagers), "golems sink through water out of sight to a lava blade".to_string()),
                "l2d" => (crate::layouts::l2d(villagers, pad), "golems fall out of sight to a lava blade".to_string()),
                "l5" => (crate::layouts::l5(villagers, pad, grid), format!("{} copies of the fastest module", grid * grid)),
                "l4" => (crate::layouts::l4(), "golems fall from fence-gated platforms to a lava blade".to_string()),
                "l6p-dry" => (crate::day1::l6p_dry(pad), "the bed-plate calm over a dry drop".to_string()),
                "l6p-pad" => (crate::day1::l6p_pad(pad), "golems burn in a corner of their pad".to_string()),
                "m3" => (crate::day1::m3_bedplate(pad), "golems fall out of sight to a lava blade".to_string()),
                "m2" => (crate::day1::m2_observer(pad), "golems fall out of sight to a lava blade".to_string()),
                other => bail!("no layout {other:?} (l1, l2, l2d, l3, l4, l5, l6p-dry, l6p-pad, m3, m2)"),
            };
            // (every farm holds its chest, so the download is the whole farm; its `how:` line ends
            // its site title)
            let text = crate::layouts::edit(&text, true, Some(&how))?;
            let text = match tier {
                Some(t) => crate::layouts::set_tier(&text, &t)?,
                None => text,
            };
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        MobfarmCmd::Exit { design, seed } => {
            let (_, spec) = load(version, &design)?;
            let refused = redsim::entity::mob::refuse::static_checks(&spec);
            for r in &refused {
                println!("refused: {r}");
            }
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32);
            let t0 = std::time::Instant::now();
            let rep = redsim::entity::mob::exit::golem_exit(&spec, seed, threads).map_err(|e| anyhow::anyhow!(e))?;
            println!("{} spawn spots, {} positions, {} runs, worst life {} gt, {:.1} s", rep.spawn_spots, rep.starts, rep.runs, rep.worst_life, t0.elapsed().as_secs_f64());
            for f in &rep.failures {
                println!("FAIL {f}");
            }
        }
        MobfarmCmd::Run { design, seed, days, still } => {
            let (_, spec) = load(version, &design)?;
            use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
            let walk = if still { WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still) } else { WalkMode::Random };
            let o = RunOpts::steady(seed, days * 24000, walk);
            let t0 = std::time::Instant::now();
            let r = run(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            // (the day time of a spawn: the mob layer starts at day0 - SETTLE_TICKS at game time 0)
            let night = r.spawn_times.iter().filter(|t| (o.day0 - redsim::entity::mob::farm::SETTLE_TICKS + **t).rem_euclid(24000) >= 12000).count();
            println!("golems by day (day time 0..12000) {}, by night {night}", r.spawn_times.len() - night);
            println!(
                "day0 {}: {} golems ({:.1}/h), {} iron ({:.0}/h), {} in the chest, kill {:?} gt, {} tries without a spot, {} sightings, {} sleeps, longest without sleep {} gt, {:.1} s",
                o.day0,
                r.golems,
                r.golems_per_hour(),
                r.iron_dropped,
                r.iron_per_hour(),
                r.iron_in_chest,
                r.lifetimes.iter().take(8).collect::<Vec<_>>(),
                r.losses.no_spot,
                r.losses.seen,
                r.sleeps,
                r.max_sleep_gap,
                t0.elapsed().as_secs_f64()
            );
            for f in r.failures.iter().chain(r.errors.iter()) {
                println!("FAIL {f}");
            }
        }
        MobfarmCmd::Validate { design, days, kill, seeds, trace, repeat } => validate_cmd(version, &design, days, kill, seeds, trace, repeat)?,
        MobfarmCmd::Probe { design, row } => {
            let (_, spec) = load(version, &design)?;
            let o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
            let w = redsim::entity::mob::farm::build(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            let [x0, x1, y, z] = [row[0], row[1], row[2], row[3]];
            for x in x0..=x1 {
                println!("{x} {y} {z}: {}", w.get(redsim::BlockPos::new(x, y, z)));
            }
        }
        MobfarmCmd::Edit { design, chest, how } => {
            let text = std::fs::read_to_string(&design)?;
            let out = crate::layouts::edit(&text, chest, how.as_deref())?;
            std::fs::write(&design, out)?;
            println!("{}", design.display());
        }
        MobfarmCmd::Sight { design } => sight_cmd(version, &design)?,
    }
    Ok(())
}

/// `mobfarm sight`: each villager's eye to the zombie's eye in the built farm, as built and with
/// every trapdoor of the farm forced shut, then forced open.
fn sight_cmd(version: &str, design: &PathBuf) -> Result<()> {
    use redsim::block::Block;
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    let (_, spec) = load(version, design)?;
    let o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
    let base = redsim::entity::mob::farm::build(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
    let eye = |w: &redsim::world::World, i: usize| redsim::entity::mob::villager::eye_pos(w, i);
    let Some(z) = base.entities.list.iter().position(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie)) else { bail!("no zombie") };
    let trapdoors: Vec<redsim::BlockPos> = spec.blocks.iter().filter(|(_, b)| matches!(b, Block::Trapdoor { .. })).map(|(p, _)| *p).collect();
    for (label, open) in [("as built", None), ("trapdoors shut", Some(false)), ("trapdoors open", Some(true))] {
        let mut w = base.clone();
        if let Some(o) = open {
            for p in &trapdoors {
                if let Block::Trapdoor { iron, facing, top, powered, .. } = w.get(*p) {
                    w.set_block(*p, Block::Trapdoor { iron, facing, top, open: o, powered }, redsim::world::flags::RAW);
                }
            }
        }
        let ze = eye(&w, z);
        println!("{label}: zombie eye {:.4} {:.4} {:.4}", ze.x, ze.y, ze.z);
        for (i, e) in w.entities.list.iter().enumerate() {
            if !matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager) {
                continue;
            }
            let ve = eye(&w, i);
            let hit = redsim::entity::mob::los::first_hit(&w, ve, ze);
            let what = match hit {
                None => "SEES the zombie".to_string(),
                Some(p) => format!("blind (stopped at {} {} {}: {})", p.x, p.y, p.z, w.get(p)),
            };
            println!("  villager at {:.2} {:.2} {:.2}, eye {:.4}: {what}", e.pos.x, e.pos.y, e.pos.z, ve.y);
        }
    }
    Ok(())
}

fn validate_cmd(version: &str, design: &PathBuf, days: u32, kill: bool, seeds: u32, trace: bool, repeat: u32) -> Result<()> {
    let (c, spec) = load(version, design)?;
    let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let mut all = crate::validate::GameRun::default();
    let mut game_rates = Vec::new();
    for r in 0..repeat.max(1) {
        let t0 = std::time::Instant::now();
        let g = crate::validate::game_run(&pool, &c, days, kill, trace, 1 + r as i64)?;
        println!("game run {r}: {} golems in {days} days ({:.1}/h), {} iron and {} poppies in the chest, {:.0} s", g.golems.len(), g.golems_per_hour(), g.iron_in_chest, g.poppies_in_chest, t0.elapsed().as_secs_f64());
        game_rates.push(g.golems_per_hour());
        all.ticks += g.ticks;
        all.golems.extend(g.golems);
        all.iron_in_chest += g.iron_in_chest;
        all.poppies_in_chest += g.poppies_in_chest;
    }
    let g = all;
    let gm = game_rates.iter().sum::<f64>() / game_rates.len() as f64;
    let gsd = if game_rates.len() > 1 { (game_rates.iter().map(|x| (x - gm) * (x - gm)).sum::<f64>() / (game_rates.len() - 1) as f64).sqrt() } else { 0.0 };
    println!("game: {} runs, golems/h mean {gm:.1} (95% of the mean {:.1}-{:.1}), {} iron and {} poppies in all", game_rates.len(), gm - 1.96 * gsd / (game_rates.len() as f64).sqrt(), gm + 1.96 * gsd / (game_rates.len() as f64).sqrt(), g.iron_in_chest, g.poppies_in_chest);
    let mut cells: std::collections::BTreeMap<[i32; 3], u32> = std::collections::BTreeMap::new();
    for (_, p) in &g.golems {
        *cells.entry(*p).or_default() += 1;
    }
    println!("game spawn cells: {cells:?}");
    // day (day time 10..12000) against night (REST), the game's day time starting at 1000
    let night = |day_time: i64| { let d = day_time.rem_euclid(24000); !(10..12000).contains(&d) };
    let g_night = g.golems.iter().filter(|(t, _)| night(1000 + *t as i64)).count();
    println!("game: {} golems by day, {g_night} by night", g.golems.len() - g_night);
    use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
    let jobs: Vec<RunOpts> = (0..seeds).map(|k| RunOpts::steady(1000 + k as i64, days * 24000, WalkMode::Random)).collect();
    let runs = circuit::mobfarm::run_all(&spec, &jobs);
    let rates: Vec<f64> = runs.iter().filter_map(|r| r.as_ref().ok()).map(|r| r.golems_per_hour()).collect();
    let m = rates.iter().sum::<f64>() / rates.len().max(1) as f64;
    let sd = (rates.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (rates.len().max(2) - 1) as f64).sqrt();
    println!("redsim: {} runs of {days} days: golems/h mean {m:.1}, sd per run {sd:.1}: one game run is expected within {:.1}-{:.1} (95%)", rates.len(), m - 1.96 * sd, m + 1.96 * sd);
    let (mut sd_day, mut sd_night) = (0usize, 0usize);
    for (j, r) in jobs.iter().zip(runs.iter()) {
        if let Ok(r) = r {
            for t in &r.spawn_times {
                if night(j.day0 + *t) { sd_night += 1 } else { sd_day += 1 }
            }
        }
    }
    println!("redsim: {sd_day} golems by day, {sd_night} by night (night share {:.2}; game {:.2})", sd_night as f64 / (sd_day + sd_night).max(1) as f64, g_night as f64 / g.golems.len().max(1) as f64);
    // the spawn cells: redsim's shares against the game's counts (Pearson's chi-square)
    let mut sim: std::collections::BTreeMap<[i32; 3], u32> = std::collections::BTreeMap::new();
    for r in runs.iter().filter_map(|r| r.as_ref().ok()) {
        for p in &r.spawn_cells {
            *sim.entry(*p).or_default() += 1;
        }
    }
    let total_sim: u32 = sim.values().sum();
    let n = g.golems.len() as f64;
    let mut chi = 0.0;
    let mut bins = 0;
    for (p, k) in &sim {
        let e = n * *k as f64 / total_sim.max(1) as f64;
        let o = *cells.get(p).unwrap_or(&0) as f64;
        if e > 0.0 {
            chi += (o - e) * (o - e) / e;
            bins += 1;
        }
        println!("  cell {p:?}: redsim share {:.3}, game {o} of {n} (expected {e:.1})", *k as f64 / total_sim.max(1) as f64);
    }
    let strays = cells.keys().filter(|p| !sim.contains_key(*p)).count();
    println!("spawn cells: chi-square {chi:.2} over {bins} cells ({} degrees of freedom; 5% critical value {:.1}); {strays} game cells redsim never spawned in", bins.max(1) - 1, chi_crit(bins.max(1) - 1));
    let _ = run;
    Ok(())
}

/// The chi-square distribution's 95% quantile for `df` degrees of freedom (Wilson-Hilferty).
fn chi_crit(df: usize) -> f64 {
    let k = df.max(1) as f64;
    let z = 1.6449;
    k * (1.0 - 2.0 / (9.0 * k) + z * (2.0 / (9.0 * k)).sqrt()).powi(3)
}
