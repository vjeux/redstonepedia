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
        /// Modules stacked (stack: M2 modules in one chunk column).
        #[arg(long, default_value_t = 2)]
        modules: i32,
        /// The stack's vertical pitch (stack).
        #[arg(long, default_value_t = 43)]
        pitch: i32,
        /// The regular cage (cross: blind U1 in the watchers' row in a pod like theirs; brief
        /// iteration 17).
        #[arg(long)]
        regular: bool,
        /// The stack's step in x and z (stack: side by side).
        #[arg(long, default_value_t = 0)]
        dx: i32,
        #[arg(long, default_value_t = 0)]
        dz: i32,
        /// LP: the gap from a lower cluster up to its upper one, and the pairs' period.
        #[arg(long, default_value_t = 15)]
        gap: i32,
        #[arg(long, default_value_t = 62)]
        period: i32,
        /// m2open: the hide's length in gt (the bed observer's delay line, 12..=48).
        #[arg(long, default_value_t = 48)]
        hide: i32,
        /// d1u: its kill pit's floor, the villagers' beds at 0 (default -13; at -19 or deeper a
        /// burning golem is out of every villager's 16-block golem sensor).
        #[arg(long, default_value_t = -13, allow_hyphen_values = true)]
        kill: i32,
        /// d1u, d1u-side, d1u-drop-side: build its access in (a spiral staircase dug from the
        /// chest to the surface, a fence-gate hatch: `day1::dug_access`).
        #[arg(long)]
        access: bool,
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
        /// A meta line to set, `key=value` (repeatable; an empty value removes it): `--meta
        /// sample=village-edge` judges a void design in that terrain sample.
        #[arg(long)]
        meta: Vec<String>,
    },
    /// The built farm's lines of sight: each villager's eye to the zombie's eye (the hostile
    /// sensor's test), standing and asleep on its bed (bed + 0.8875), as built and with every
    /// trapdoor forced shut and forced open. `--terrain S`: built into that sample at the judge's
    /// cheapest spot (the design's `terrain:` meta: natural rock for its walls).
    Sight {
        design: PathBuf,
        #[arg(long)]
        terrain: Option<String>,
    },
    /// The late stage's coupling, exactly: the villager clusters (villagers within each other's
    /// 10-box), and every golem spawn spot of a cluster against every foreign villager's follow
    /// cube at the largest follow range (16 x 1.11485), with the smallest margin (negative:
    /// a foreign villager lists that golem).
    Cubes { design: PathBuf },
    /// A design's build bill at a stage's prices, as the judge's `build minutes` reads it
    /// (circuit::mobfarm::site_bill, the void build): every item, the travel and height rows
    /// (brief farm-iron rev 9 iteration 16) and the collecting trip.
    Bill {
        design: PathBuf,
        /// day-1, middle or late.
        #[arg(long, default_value = "middle")]
        stage: String,
        /// Price it built into this terrain sample at its cheapest spot (village-edge, plains,
        /// forest): the spot's digging, seals and caps, its access stair (a terrain design's
        /// card prices it so).
        #[arg(long)]
        terrain: Option<String>,
    },
    /// A void farm dug into rock (vjeux 18:18: the underground version): its containment blocks
    /// give way to natural rock, every cell its golems (any input in the exit flood, the runs),
    /// items, water, villagers, zombie, sight lines and redstone need is dug open (`terrain:
    /// rock`), and the natural ground lies `--cover` layers above its top.
    Bury {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Rock layers between the farm's top and the natural ground layer.
        #[arg(long, default_value_t = 1)]
        cover: i32,
        /// Runs (each `--ticks`) whose entity boxes join the open cells.
        #[arg(long, default_value_t = 4)]
        seeds: u32,
        #[arg(long, default_value_t = 24000)]
        ticks: u32,
    },
    /// The farm built into a terrain sample, then the terrain track's worst-case checks for
    /// natural mobs (`hostile::check`): each channel's exposure (brief revision 8: a card line,
    /// never a refusal).
    Exposures {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        /// Every cell of each exposure, in design coordinates (where to light or seal).
        #[arg(long)]
        cells: bool,
    },
    /// The hostile checks' C1 against the game: at every interior cell of the farm built in a
    /// terrain sample, for every type listed there, C1's verdict (placement, spawn rule at its
    /// best draws and worst light, no collision) beside the game's own `SpawnPlacements`
    /// (`isSpawnPositionOk`, `noCollision`, `checkSpawnRules` over `--tries` randoms) in the
    /// compare's terrain sandbox.
    SpawnRules {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        #[arg(long, default_value_t = 200)]
        tries: u32,
        /// The game's clock for the check (the overworld's total ticks; default midnight, the
        /// darkest sky, what C1 assumes for a darkness rule).
        #[arg(long, default_value_t = 18000)]
        day_time: i64,
    },
    /// The design priced at each of the spots a player would pick in a terrain sample (the 12
    /// nearest valid ones, `site::price_all`): blocks placed, glass covers, natural blocks dug and
    /// their build seconds, cheapest first, with the time the cheap part (`price_spots`) and the
    /// covers (`covers_at`) take (`--repeat`: the cheap part timed over N calls).
    Spots {
        design: PathBuf,
        #[arg(long, default_value = "village-edge")]
        terrain: String,
        #[arg(long, default_value_t = 1)]
        repeat: u32,
        /// The caps for the panicking villagers' golem searches only (the judge caps every
        /// villager's, `site::covers_at`), to compare.
        #[arg(long)]
        panicking_only: bool,
        /// List the cheapest spot's capped cells in design coordinates.
        #[arg(long)]
        caps: bool,
    },
    /// mobinfra: the real-game gate (what `circuit verify` and `circuit archive` run for a mob
    /// farm): each build in the 26.3 server for `--days` of natural weather, a forced day of rain
    /// and one of thunder, on Hard, every must-have checked, the rate against redsim's.
    Gate {
        design: PathBuf,
        #[arg(long, default_value_t = 3)]
        days: u32,
        /// Only these builds (rotation.offset index, e.g. `0.0 1.1`); default: the judge's 8.
        #[arg(long, num_args = 0..)]
        builds: Vec<String>,
        #[arg(long)]
        no_rain_day: bool,
        #[arg(long)]
        no_thunder_day: bool,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Servers at once.
        #[arg(long, default_value_t = 8)]
        parallel: usize,
        /// The weather's warm-up range (gt), `MIN MAX`.
        #[arg(long, num_args = 2, default_values_t = [150000u32, 400000u32])]
        warmup: Vec<u32>,
        #[arg(long)]
        keep: bool,
        #[arg(long)]
        trace: bool,
        /// A terrain sample (`village-edge`...): the farm at its Site in the sample's kept world,
        /// `--parallel` runs of the one placement.
        #[arg(long)]
        terrain: Option<String>,
        /// The kept world folder (default `$MCTAS_HOME/terrain/worlds/<the sample's world>`).
        #[arg(long)]
        world: Option<PathBuf>,
        /// Natural monsters off in a terrain run (`spawn_monsters false`): an experiment that
        /// separates the farm from the natural mobs around it (mob-AI's).
        #[arg(long)]
        no_monsters: bool,
        /// A terrain sample's number of runs (default: `--parallel`), `--parallel` servers at
        /// once (each its own seed): enough runs for a narrow interval on the mean.
        #[arg(long, default_value_t = 0)]
        runs: usize,
    },
    /// redsim's over the same length.
    WeatherStats {
        #[arg(long, default_value_t = 200)]
        days: u32,
        #[arg(long, default_value_t = 8)]
        servers: usize,
        /// Sample every this many gt.
        #[arg(long, default_value_t = 100)]
        dt: u32,
        #[arg(long, default_value_t = 64)]
        sim_runs: u32,
    },
    /// mobinfra: lava's fire in a farm, the game's time to the first burned block beside redsim's.
    FireBurn {
        design: PathBuf,
        #[arg(long, default_value_t = 8)]
        servers: usize,
        #[arg(long, default_value_t = 48000)]
        max: u32,
        #[arg(long, default_value_t = 200)]
        seeds: u32,
    },
    /// mobinfra: bolts summoned in fixed scenarios (hits through roofs, the box's edge, a boat, a
    /// drop, a golem, the bolt's fire), the game's outcome beside redsim's.
    BoltCheck,
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
        /// Build it into a standard terrain sample (village-edge, plains, forest) with natural
        /// spawning on Hard around its AFK spot (the design's `ground:`, `afk:`, `terrain:` meta).
        #[arg(long)]
        terrain: Option<String>,
        /// Only tick the built farm for `--days` (no checks, no report): the time a game day takes.
        #[arg(long)]
        ticks_only: bool,
        /// Print every change of these cells' blocks (`x,y,z`, repeatable) with its game time and
        /// day time, and each villager's sleeping, instead of the report.
        #[arg(long)]
        watch: Vec<String>,
        /// After the report, the N cells where living golems spent the most ticks (design
        /// coordinates; the run's occupancy).
        #[arg(long, default_value_t = 0)]
        cells: usize,
        /// Instead of the report: tick the run for `--days` and print the mobs by type (the most
        /// alive at once, and alive every 1000 gt summed), and the time the entity ticks took.
        #[arg(long)]
        census: bool,
    },
    /// Each design's tick-exactness as `circuit verify` finds it: run 0 of every judge build on
    /// the deterministic bridge against redsim for the judge's days (each build's result and the
    /// lane owning its first difference); `--record` writes it to the ratchet's table
    /// (designs/circuits/mobfarm/exact.tsv: a design recorded exact fails a later verify that
    /// differs).
    Exact {
        designs: Vec<PathBuf>,
        #[arg(long)]
        record: bool,
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
    /// The exact compare (track tickexact): the design run in the game (the bridge, every random
    /// seeded from the run's seed) and in redsim side by side, every tick compared (blocks, every
    /// entity, the level random, randValue, each entity's random); prints the first difference
    /// with its context and the fields redsim does not model yet. `--twin`: the bridge against
    /// itself (the determinism check). `--probe`: on a difference, run again and print the game's
    /// draws of that tick by call site.
    Compare {
        design: PathBuf,
        #[arg(long, default_value_t = 1)]
        seed: i64,
        /// Game days after the settle and the entities' arrival.
        #[arg(long, default_value_t = 3.0)]
        days: f64,
        /// Game ticks (instead of --days).
        #[arg(long)]
        ticks: Option<u32>,
        /// Weather on (rain and thunder cycle).
        #[arg(long)]
        weather: bool,
        #[arg(long)]
        probe: bool,
        #[arg(long)]
        twin: bool,
        /// Run to the end after the first difference.
        #[arg(long)]
        full: bool,
        /// The bare scenario: no AFK player, the void's biome, spawn_mobs off (the mob layer
        /// without natural spawning, fire spread or thunder). Default: the real-game gate's.
        #[arg(long)]
        bare: bool,
        /// Print the game's draw log of this game time (every stream: "level", "e<n>" = entity
        /// n's own random; each call site with its count, in draw order).
        #[arg(long)]
        draws_at: Option<i64>,
        /// Build it into a terrain sample (village-edge, plains, forest) at its site, natural
        /// spawning on Hard around its AFK spot.
        #[arg(long)]
        terrain: Option<String>,
        /// The day time the farm starts at (default: the steady state's).
        #[arg(long)]
        day0: Option<i64>,
        /// Scan seeds FROM..=TO on the game's side alone for entities of this type entering (a
        /// siege's zombie, a patrol's pillager): the fixtures' seeds.
        #[arg(long, num_args = 3, value_names = ["TYPE", "FROM", "TO"])]
        scan: Option<Vec<String>>,
        /// Ticks per traced step.
        #[arg(long, default_value_t = 100)]
        batch: u32,
        /// The judge's build K (the card's: rotation K / 2 at the (K % 2)-th position in the void,
        /// the K-th site in a terrain sample), with `--run`: its seed, walking and weather.
        #[arg(long)]
        build: Option<u32>,
        /// With `--build`: the judge's run I of that build (seed JUDGE_SEED0 + 1000 K + I).
        #[arg(long, default_value_t = 0)]
        run: u32,
        /// No mob spawning on either side (natural and the custom spawners; the chunk shuffle
        /// still drawn): the random ticks, weather and fire alone.
        #[arg(long)]
        no_spawning: bool,
        /// Only these kinds of difference count, comma separated: `world` (times, the AFK player,
        /// the level random, randValue, weather), `blocks` (blocks, containers), `entities`, or
        /// a category's own name ("level random", "villager pos").
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
    },
}

fn load(_version: &str, path: &PathBuf) -> Result<(circuit::Circuit, redsim::entity::mob::farm::FarmSpec)> {
    let c = circuit::Circuit::load(path)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow::anyhow!(e))?;
    Ok((c, spec))
}

pub fn run(version: &str, cmd: MobfarmCmd) -> Result<()> {
    match cmd {
        MobfarmCmd::Gen { layout, pits, pad, floor, villagers, grid, modules, pitch, regular, dx, dz, gap, period, hide, kill, access, tier, out } => {
            let (text, how) = match layout.as_str() {
                "l3" => (crate::layouts::l3(pits), "golems burn in lava pits".to_string()),
                "l1" => (crate::layouts::l1(), "water carries golems to a lava blade in the villagers' sight".to_string()),
                "l2" => (crate::layouts::l2(pad, floor, villagers), "golems sink through water out of sight to a lava blade".to_string()),
                "l2d" => (crate::layouts::l2d(villagers, pad), "golems fall out of sight to a lava blade".to_string()),
                "l5" => (crate::layouts::l5(villagers, pad, grid), format!("{} copies of the fastest module", grid * grid)),
                "l4" => (crate::layouts::l4(), "golems fall from fence-gated platforms to a lava blade".to_string()),
                "l6p-dry" => (crate::day1::l6p_dry(pad), "the bed-plate calm over a dry drop".to_string()),
                "l6p-pad" => (crate::day1::l6p_pad(pad), "golems burn in a corner of their pad".to_string()),
                "d4" => (crate::day1::d4_two_scared(pad), "golems burn in a corner of their pit".to_string()),
                "d1u" => (crate::day1::d1u_with_access(pad, kill, access), if kill <= -19 { "golems burn in a pit too deep for the villagers to sense".to_string() } else { "golems burn in a corner of their pit".to_string() }),
                "d1u-side" => (crate::day1::d1u_side_with(pad, 0, access), "water carries golems sideways to a pit beyond the villagers' reach".to_string()),
                "d1u-drop-side" => (crate::day1::d1u_side_with(pad, 13, access), "golems drop below the villagers' reach, then water carries them sideways to a pit".to_string()),
                "m3" => (crate::day1::m3_bedplate(pad), "golems fall out of sight to a lava blade".to_string()),
                "m2" => (crate::day1::m2_observer(pad), "golems fall out of sight to a lava blade".to_string()),
                "m2open" => (crate::day1::m2_open_hide(pad, hide), "golems fall out of sight to a lava blade".to_string()),
                "m2open-rep" => (crate::day1::m2_open_rep(pad, hide), "golems fall out of sight to a lava blade".to_string()),
                "m2open-loop" => (crate::day1::m2_open_loop(pad, hide / 10, hide % 10), "golems fall out of sight to a lava blade".to_string()),
                "m2open-timer" => (crate::day1::m2_open_timer_cage(pad, hide as u32, crate::parts::Stage::Nether, regular)?, "golems fall out of sight to a lava blade".to_string()),
                "row" => (crate::day1::row(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules in a row share one collection")),
                "row-shared" => (crate::day1::row_shared(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules in a row share one kill chamber")),
                "row-channel" => (crate::day1::row_shared_form(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether, crate::day1::KillForm::Channel)?, format!("{modules} modules in a row send their golems to one kill chamber")),
                "tstack" => (crate::day1::stack(modules, pitch, pad, hide as u32, crate::parts::Stage::Nether)?, format!("{modules} modules stacked")),
                "cross" => (crate::day1::cross_channel_cage(pitch, if modules == 2 { 2 } else { 4 }, regular, pad, hide as u32, crate::parts::Stage::Nether)?, "farms around one kill chamber".to_string()),
                "stack" => (crate::day1::m2_stack(modules, [dx, pitch, dz], pad), format!("{modules} middle modules")),
                "lp" => (crate::day1::lp(modules / 2, gap, period, pad), format!("{modules} clusters in up/down pairs")),
                "twin" => (crate::day1::twin(pad, villagers - 4), "two clusters share a pad, a drop and a chamber".to_string()),
                "tz" => (crate::day1::tz(pad), "two clusters share one zombie and one calm".to_string()),
                "shared" => (crate::day1::stack_collect(modules, pitch, pad), format!("{modules} modules share one collection")),
                other => bail!("no layout {other:?} (l1, l2, l2d, l3, l4, l5, l6p-dry, l6p-pad, m3, m2, stack)"),
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
        MobfarmCmd::Run { design, seed, days, still, terrain, ticks_only, watch, cells, census } => {
            let (c, spec) = load(version, &design)?;
            use redsim::entity::mob::farm::{RunOpts, WalkMode, run};
            let walk = if still { WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still) } else { WalkMode::Random };
            let mut o = RunOpts::steady(seed, days * 24000, walk);
            let spec = match &terrain {
                Some(name) => {
                    let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
                    println!("terrain {}: offset {:?}, {} blocks placed + {} filled, {} dug, AFK at {:.1} {:.1} {:.1}; the cheapest of {} spots (block time {:.0}-{:.0} s)", site.sample, site.offset, site.placed, site.fills.len(), site.dug, site.afk.x, site.afk.y, site.afk.z, site.spots, site.build_range.0, site.build_range.1);
                    o.site = Some(site);
                    site.spec(&spec)
                }
                None => spec,
            };
            let t0 = std::time::Instant::now();
            if !watch.is_empty() {
                let cells: Vec<redsim::BlockPos> = watch
                    .iter()
                    .map(|s| {
                        let v: Vec<i32> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        if v.len() == 3 { Ok(redsim::BlockPos::new(v[0], v[1], v[2])) } else { Err(anyhow::anyhow!("--watch {s}: expected x,y,z")) }
                    })
                    .collect::<Result<_>>()?;
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let mut last: Vec<String> = cells.iter().map(|c| r.world.get(*c).to_string()).collect();
                let mut sleeping: Vec<(u32, bool)> = Vec::new();
                for (c, s) in cells.iter().zip(&last) {
                    println!("t {} day {}: {} {} {}: {s}", r.world.time, r.world.day_time().rem_euclid(24000), c.x, c.y, c.z);
                }
                for _ in 0..days * 24000 {
                    r.tick();
                    let (t, day) = (r.world.time, r.world.day_time().rem_euclid(24000));
                    for (k, c) in cells.iter().enumerate() {
                        let s = r.world.get(*c).to_string();
                        if s != last[k] {
                            println!("t {t} day {day}: {} {} {}: {s}", c.x, c.y, c.z);
                            last[k] = s;
                        }
                    }
                    for e in &r.world.entities.list {
                        if let redsim::entity::EntityKind::Mob(m) = &e.kind
                            && m.ty == redsim::entity::mob::types::MobType::Villager
                        {
                            let asleep = m.sleeping_pos.is_some();
                            match sleeping.iter_mut().find(|(id, _)| *id == e.id) {
                                Some((_, was)) if *was != asleep => {
                                    println!("t {t} day {day}: villager {} at {:.1} {:.1} {} ", e.id, e.pos.x, e.pos.z, if asleep { "lies down" } else { "wakes" });
                                    *was = asleep;
                                }
                                Some(_) => {}
                                None => sleeping.push((e.id, asleep)),
                            }
                        }
                    }
                }
                return Ok(());
            }
            if ticks_only {
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let built = t0.elapsed().as_secs_f64();
                for _ in 0..days * 24000 {
                    r.tick();
                }
                println!("built and settled in {built:.1} s; {days} game day(s) ticked in {:.1} s", t0.elapsed().as_secs_f64() - built);
                return Ok(());
            }
            if census {
                let mut r = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
                let built = t0.elapsed().as_secs_f64();
                let mut peak: std::collections::BTreeMap<String, usize> = Default::default();
                let mut sum: std::collections::BTreeMap<String, usize> = Default::default();
                for t in 0..days * 24000 {
                    r.tick();
                    let mut now: std::collections::BTreeMap<String, usize> = Default::default();
                    for e in &r.world.entities.list {
                        if let redsim::entity::EntityKind::Mob(m) = &e.kind
                            && !e.removed
                            && !m.dead
                        {
                            *now.entry(m.ty.def().id.to_string()).or_insert(0) += 1;
                        }
                    }
                    for (k, n) in &now {
                        let p = peak.entry(k.clone()).or_insert(0);
                        *p = (*p).max(*n);
                        if t % 1000 == 0 {
                            *sum.entry(k.clone()).or_insert(0) += n;
                        }
                    }
                }
                let mut v: Vec<_> = sum.iter().collect();
                v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                for (k, s) in v {
                    println!("{k}: peak {} alive, {s} summed over the 1000-gt samples", peak[k]);
                }
                println!("built and settled in {built:.1} s; {days} game day(s) ticked in {:.1} s; model errors: {}", t0.elapsed().as_secs_f64() - built, r.world.errors.join("; "));
                return Ok(());
            }
            let r = run(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            // (the day time of a spawn: the mob layer starts at day0 - SETTLE_TICKS at game time 0)
            let night = r.spawn_times.iter().filter(|t| (o.day0 - redsim::entity::mob::farm::SETTLE_TICKS + **t).rem_euclid(24000) >= 12000).count();
            println!("golems by day (day time 0..12000) {}, by night {night}", r.spawn_times.len() - night);
            println!("containers at the end: {}", r.contents.join("; "));
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
            if !r.releases.is_empty() {
                println!("POI releases (a villager's bed given up after 1200 gt unreachable) at gt {:?}", r.releases);
            }
            if !r.legacy_ai.is_empty() {
                println!("natural mobs on the legacy walk model (their AI not modeled yet): {}", r.legacy_ai.join(", "));
            }
            if cells > 0 {
                let total: u64 = r.golem_cells.iter().map(|(_, n)| *n as u64).sum();
                let mut v = r.golem_cells.clone();
                v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                println!("golem ticks by cell ({total} in all; the top {cells}):");
                for (c, n) in v.iter().take(cells) {
                    println!("  {} {} {}: {n} ({:.1}%)", c[0], c[1], c[2], 100.0 * *n as f64 / total.max(1) as f64);
                }
            }
        }
        MobfarmCmd::Validate { design, days, kill, seeds, trace, repeat } => validate_cmd(version, &design, days, kill, seeds, trace, repeat)?,
        MobfarmCmd::Exact { designs, record } => {
            let o = circuit::verify::Options { version: version.to_string(), ..Default::default() };
            for d in &designs {
                let c = circuit::Circuit::load(d)?;
                let g = circuit::mobfarm_exact::gate(&c, &o, 1, circuit::mobfarm::JUDGE_DAYS)?;
                println!("{}: {} ({:.0} s)", d.display(), g.summary(), g.seconds);
                for r in &g.runs {
                    println!("{}", r.line());
                }
                if record {
                    let r = circuit::mobfarm_exact::write_record(&std::env::current_dir()?, &c, &d.to_string_lossy(), &g)?;
                    println!("  recorded in {}: exact {}", circuit::mobfarm_exact::RECORD, if r.exact { "yes" } else { "no" });
                }
            }
        }
        MobfarmCmd::Probe { design, row } => {
            let (_, spec) = load(version, &design)?;
            let o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
            let w = redsim::entity::mob::farm::build(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
            let [x0, x1, y, z] = [row[0], row[1], row[2], row[3]];
            for x in x0..=x1 {
                println!("{x} {y} {z}: {}", w.get(redsim::BlockPos::new(x, y, z)));
            }
        }
        MobfarmCmd::Edit { design, chest, how, meta } => {
            let text = std::fs::read_to_string(&design)?;
            let out = crate::layouts::edit_meta(&text, chest, how.as_deref(), &meta)?;
            std::fs::write(&design, out)?;
            println!("{}", design.display());
        }
        MobfarmCmd::Sight { design, terrain } => sight_cmd(version, &design, terrain.as_deref())?,
        MobfarmCmd::Cubes { design } => cubes_cmd(version, &design)?,
        MobfarmCmd::Bury { design, out, cover, seeds, ticks } => {
            let (c, spec) = load(version, &design)?;
            let (cells, fluids) = crate::bury::envelope(&spec, &c.clock.design, seeds, ticks)?;
            let (d, removed, opened) = crate::bury::bury(&c.clock.design, &cells, &fluids, cover);
            std::fs::write(&out, d.to_text())?;
            println!("{}: {} open cells ({} dug open beyond its blocks), {} containment blocks left to the rock; ground layer {}", out.display(), cells.len(), opened, removed, d.meta.get("ground").cloned().unwrap_or_default());
        }
        MobfarmCmd::Bill { design, stage, terrain } => {
            let (c, spec) = load(version, &design)?;
            let ix = match stage.as_str() {
                "day-1" | "day1" => 0,
                "middle" => 1,
                "late" | "fastest" => 2,
                s => anyhow::bail!("--stage {s}: day-1, middle or late"),
            };
            let site = match &terrain {
                Some(t) => Some(circuit::mobfarm::site_of(&c, &spec, t).map_err(|e| anyhow::anyhow!(e))?),
                None => None,
            };
            let bill = circuit::mobfarm::site_bill(&c, site.as_ref(), ix).map_err(|e| anyhow::anyhow!(e))?;
            let mut v: Vec<(&String, &(u32, f64))> = bill.items.iter().collect();
            v.sort_by(|a, b| b.1.1.partial_cmp(&a.1.1).unwrap_or(std::cmp::Ordering::Equal));
            for (k, (n, s)) in v {
                println!("{k:<24} {n:>6} {s:>9.1} s");
            }
            let travel: f64 = bill.items.iter().filter(|(k, _)| k.starts_with("travel:") || k.starts_with("villager:") || k.as_str() == "place:edge").map(|(_, (_, s))| s).sum();
            println!("{}: {:.1} build minutes at {stage} prices ({:.1} of them travel and height); collecting {:.0} s per trip ({} layers each way)", c.name(), bill.seconds / 60.0, travel / 60.0, bill.collect_s, bill.collect_layers);
        }
        MobfarmCmd::SpawnRules { design, terrain, tries, day_time } => crate::spawnrules::run(version, &design, &terrain, tries, day_time)?,
        MobfarmCmd::Exposures { design, terrain, cells } => {
            let (c, spec) = load(version, &design)?;
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, &terrain).map_err(|e| anyhow::anyhow!(e))?));
            let moved = site.spec(&spec);
            // the farm after its settle (its water flowed), as the card's exposures line reads it
            let w = circuit::mobfarm::settled_world(site, &moved);
            let summoners = {
                let mut with = w.clone();
                for (k, p, n) in &moved.entities {
                    let _ = redsim::entity::spawn(&mut with, k, *p, n.as_deref());
                }
                redsim::entity::mob::site::villager_cells(&with)
            };
            let farm = circuit::mobfarm::hostile_farm(&w, &moved, site.afk, &summoners);
            let ex = redsim::entity::mob::hostile::check(&w, &farm, true);
            println!("{} in {} at offset {:?}: {} placed + {} filled + {} sealed, {} dug (the cheapest of {} spots, block time {:.0}-{:.0} s); {} golem cells, {} interior cells", design.display(), site.sample, site.offset, site.placed, site.fills.len(), site.seals.len(), site.dug, site.spots, site.build_range.0, site.build_range.1, farm.golem_cells.len(), farm.interior.len());
            // the village's saved mobs against the farm's villagers: a village villager within the
            // 10-box of a farm summoner (`spawnGolemIfNeeded`: inflate 10, 10, 10) counts toward
            // its 3; any golem within 16 of a farm villager stalls it (GolemSensor)
            if let Some(sample) = redsim::terrain::samples::sample(site.sample) {
                let ents = redsim::entity::mob::site::sample_entities(sample).map_err(|e| anyhow::anyhow!(e))?;
                let farm_v: Vec<[f64; 3]> = moved.entities.iter().filter(|(k, _, _)| k.trim_start_matches("minecraft:") == "villager").map(|(_, p, _)| *p).collect();
                let near = |p: &[f64; 3], r: f64, ry: f64| farm_v.iter().any(|v| (v[0] - p[0]).abs() < r && (v[2] - p[2]).abs() < r && (v[1] - p[1]).abs() < ry);
                for kind in ["villager", "iron_golem", "cat"] {
                    let of: Vec<&[f64; 3]> = ents.iter().filter(|(k, _)| k == kind).map(|(_, p)| p).collect();
                    let nearest = of.iter().map(|p| farm_v.iter().map(|v| ((v[0] - p[0]).powi(2) + (v[1] - p[1]).powi(2) + (v[2] - p[2]).powi(2)).sqrt()).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min);
                    println!("village {kind}s as saved: {} (nearest {:.1} blocks from a farm villager; {} within the 10-box of one, {} within 16)", of.len(), nearest, of.iter().filter(|p| near(p, 10.6, 11.95)).count(), of.iter().filter(|p| near(p, 16.0, 16.0)).count());
                }
            }
            if ex.is_empty() {
                println!("no exposure");
            }
            for e in ex {
                println!("{}", e.line());
                if cells {
                    let [ox, oy, oz] = site.offset;
                    println!("    design cells: {}", e.cells.iter().map(|c| format!("({},{},{})", c.x - ox, c.y - oy, c.z - oz)).collect::<Vec<_>>().join(" "));
                }
            }
        }
        MobfarmCmd::Spots { design, terrain, repeat, panicking_only, caps } => spots_cmd(version, &design, &terrain, repeat, !panicking_only, caps)?,
        MobfarmCmd::Gate { design, days, builds, no_rain_day, no_thunder_day, seed, parallel, warmup, keep, trace, terrain, world, no_monsters, runs } => {
            crate::gate::gate_cmd(version, &design, crate::gate::GateArgs { days, builds, rain_day: !no_rain_day, thunder_day: !no_thunder_day, seed, parallel, warmup: (warmup[0], warmup[1]), keep, trace, terrain, world, no_monsters, runs })?
        }
        MobfarmCmd::WeatherStats { days, servers, dt, sim_runs } => crate::weathercheck::weather_stats_cmd(version, days, servers, dt, sim_runs)?,
        MobfarmCmd::FireBurn { design, servers, max, seeds } => crate::weathercheck::fire_burn_cmd(version, &design, servers, max, seeds)?,
        MobfarmCmd::BoltCheck => crate::weathercheck::bolt_check_cmd(version)?,
        MobfarmCmd::Compare { design, seed, days, ticks, weather, probe, twin, full, bare, draws_at, terrain, day0, scan, batch, build, run, no_spawning, only } => {
            let ticks = ticks.unwrap_or((days * 24000.0).round() as u32);
            let opts = crate::exact::Opts { seed, ticks, weather, probe, batch: batch.max(1), full, bare, draws_at, terrain, day0, build: build.map(|k| (k, run)), no_spawning, only };
            if let Some(s) = scan {
                let (from, to): (i64, i64) = (s[1].parse()?, s[2].parse()?);
                let hits = crate::exact::scan(version, &design, &opts, from..=to, &s[0])?;
                println!("{} of {} seeds: {:?}", hits.len(), to - from + 1, hits);
                return Ok(());
            }
            compare_cmd(version, &design, &opts, twin)?;
        }
    }
    Ok(())
}

/// `mobfarm compare`: the exact compare (or the twin check), its report; a difference fails.
fn compare_cmd(version: &str, design: &std::path::Path, opts: &crate::exact::Opts, twin: bool) -> Result<()> {
    let t0 = std::time::Instant::now();
    let cmp = if twin { crate::exact::twin(version, design, opts, None)? } else { crate::exact::compare(version, design, opts, None)? };
    print!("{}", cmp.report());
    if let (Some(d), true, true) = (&cmp.first, opts.probe, twin) {
        crate::exact::twin(version, design, opts, Some(d.t))?;
    }
    if let (Some(d), true, false) = (&cmp.first, opts.probe, twin) {
        println!("probe: the game's draws around game time {} (a second, identical run)", d.t);
        let again = crate::exact::compare(version, design, opts, Some(d.t))?;
        if let Some(d2) = &again.first {
            for c in &d2.context {
                println!("  {c}");
            }
        }
    }
    println!("{:.1} s", t0.elapsed().as_secs_f64());
    if cmp.first.is_some() {
        bail!("{} differs", if twin { "the game against itself" } else { "redsim against the game" });
    }
    Ok(())
}

/// `mobfarm spots`: the design priced at each spot of a terrain sample, cheapest first.
fn spots_cmd(version: &str, design: &PathBuf, terrain: &str, repeat: u32, all: bool, list_caps: bool) -> Result<()> {
    use redsim::entity::mob::site;
    let (c, spec) = load(version, design)?;
    let d = &c.clock.design;
    let sample = redsim::terrain::samples::sample(terrain).ok_or_else(|| anyhow::anyhow!("no terrain sample {terrain}"))?;
    let (ground, _afk, clear_box) = site::params(&spec, d.meta.get("ground").map(String::as_str), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str)).map_err(|e| anyhow::anyhow!(e))?;
    let t0 = std::time::Instant::now();
    let w = site::sample_world(sample).map_err(|e| anyhow::anyhow!(e))?;
    let t_world = t0.elapsed().as_secs_f64();
    let fp = site::footprint(&spec, ground).map_err(|e| anyhow::anyhow!(e))?;
    let t1 = std::time::Instant::now();
    let spots = site::spots_for_spec(&w, sample, &spec, fp, clear_box);
    let t_spots = t1.elapsed().as_secs_f64();
    let t2 = std::time::Instant::now();
    let mut cheap = Vec::new();
    for _ in 0..repeat.max(1) {
        cheap = site::price_spots(&w, &spec, fp, ground, clear_box, &spots);
    }
    let t_cheap = t2.elapsed().as_secs_f64() / repeat.max(1) as f64;
    let t3 = std::time::Instant::now();
    let mut priced: Vec<site::Priced> = cheap.iter().map(|p| site::Priced { price: *p, covers: site::covers_from(&w, &spec, p.offset, clear_box, all), seals: site::seals_at(&w, &spec, p.offset, clear_box), clears: site::clears_at(&w, &spec, p.offset, clear_box) }).collect();
    let t_covers = t3.elapsed().as_secs_f64();
    priced.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
    println!("{} in {terrain} ({}): footprint {}x{}, {} deep, {} spots", design.display(), if clear_box { "its box cleared" } else { "its cells written, natural rock for the rest" }, fp.size[0], fp.size[1], fp.below, spots.len());
    for (k, p) in priced.iter().enumerate() {
        println!("  {k:2}: offset {:?}: {} placed + {} covers + {} seals ({} dug first) + {} cleared of fire ({} replaced with cobblestone), {} dug: {:.0} s", p.price.offset, p.price.placed, p.covers.len(), p.seals.len(), p.seals.iter().filter(|(_, d)| *d).count(), p.clears.len(), p.clears.iter().filter(|(_, b)| !b.is_air()).count(), p.price.dug, p.seconds());
        if list_caps && k == 0 {
            let o = p.price.offset;
            println!("      caps (design cells): {}", p.covers.iter().map(|c| format!("({},{},{})", c.x - o[0], c.y - o[1], c.z - o[2])).collect::<Vec<_>>().join(" "));
        }
    }
    println!("times: sample world {:.3} s (once a process), spots {:.4} s (per footprint and depth), cheap part {:.6} s a call, covers {:.4} s for {} spots", t_world, t_spots, t_cheap, t_covers, priced.len());
    Ok(())
}

/// `mobfarm cubes`: clusters by the spawn's 10-box (`Villager.spawnGolemIfNeeded`: the summoner's
/// box inflated by 10), each golem spawn spot (`exit::spawn_spots`) owned by the clusters whose
/// villagers' spawn search reaches it (8 columns, 6 up or down from the villager's block), and its
/// golem box (1.4 x 2.7 at the cell's center) against the follow cube of every villager of the
/// other clusters (`NearestLivingEntitySensor`: the villager's box inflated by FOLLOW_RANGE on all
/// three axes; the range at its largest, 16 x (1 + 0.11485)).
fn cubes_cmd(version: &str, design: &PathBuf) -> Result<()> {
    use redsim::entity::mob::farm::{RunOpts, WalkMode, build};
    use redsim::entity::{Aabb, EntityKind};
    const R: f64 = 16.0 * 1.11485;
    let (_, spec) = load(version, design)?;
    let w = build(&spec, &RunOpts::steady(1, 0, WalkMode::Random)).map_err(|e| anyhow::anyhow!(e))?;
    let vs: Vec<(Aabb, redsim::BlockPos)> = w
        .entities
        .list
        .iter()
        .filter(|e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == redsim::entity::mob::types::MobType::Villager))
        .map(|e| (e.bb, e.block_position()))
        .collect();
    // clusters: union-find over "inside the other's 10-box"
    let mut parent: Vec<usize> = (0..vs.len()).collect();
    fn root(p: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..vs.len() {
        for j in 0..vs.len() {
            if i != j && vs[i].0.inflate(10.0, 10.0, 10.0).intersects(vs[j].0) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let cluster: Vec<usize> = (0..vs.len()).map(|i| root(&mut parent, i)).collect();
    let mut ids: Vec<usize> = cluster.clone();
    ids.sort();
    ids.dedup();
    println!("{} villagers in {} clusters (by the spawn's 10-box)", vs.len(), ids.len());
    let spots = redsim::entity::mob::exit::spawn_spots(&w);
    let mut worst: Option<(f64, redsim::BlockPos, usize)> = None;
    let mut inside = 0;
    for c in &spots {
        let owners: Vec<usize> = (0..vs.len()).filter(|&i| (c.x - vs[i].1.x).abs() <= 8 && (c.z - vs[i].1.z).abs() <= 8 && (c.y - vs[i].1.y).abs() <= 6).map(|i| cluster[i]).collect();
        let (x, y, z) = (c.x as f64 + 0.5, c.y as f64, c.z as f64 + 0.5);
        let g = Aabb::new(x - 0.7, y, z - 0.7, x + 0.7, y + 2.7, z + 0.7);
        for (i, (vb, _)) in vs.iter().enumerate() {
            if owners.contains(&cluster[i]) {
                continue;
            }
            let cube = vb.inflate(R, R, R);
            let sep = [cube.min_x - g.max_x, g.min_x - cube.max_x, cube.min_y - g.max_y, g.min_y - cube.max_y, cube.min_z - g.max_z, g.min_z - cube.max_z].into_iter().fold(f64::NEG_INFINITY, f64::max);
            if sep < 0.0 {
                inside += 1;
            }
            if worst.is_none_or(|(s, _, _)| sep < s) {
                worst = Some((sep, *c, i));
            }
        }
    }
    println!("{} spawn spots; {} (spot, foreign villager) pairs inside the follow cube", spots.len(), inside);
    match worst {
        Some((sep, c, i)) => println!("smallest margin {sep:.4} blocks: the spot {} {} {} against villager {} (feet {:.4})", c.x, c.y, c.z, i, vs[i].0.min_y),
        None => println!("no spot near a foreign cluster"),
    }
    Ok(())
}

/// `mobfarm sight`: each villager's eye to the zombie's eye in the built farm, as built and with
/// every trapdoor of the farm forced shut, then forced open.
fn sight_cmd(version: &str, design: &PathBuf, terrain: Option<&str>) -> Result<()> {
    use redsim::block::Block;
    use redsim::entity::EntityKind;
    use redsim::entity::mob::types::MobType;
    let (c, spec) = load(version, design)?;
    let mut o = redsim::entity::mob::farm::RunOpts::steady(1, 0, redsim::entity::mob::farm::WalkMode::Random);
    let spec = match terrain {
        Some(name) => {
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow::anyhow!(e))?));
            o.site = Some(site);
            site.spec(&spec)
        }
        None => spec,
    };
    let run = redsim::entity::mob::farm::Run::new(&spec, &o).map_err(|e| anyhow::anyhow!(e))?;
    let own = run.own.clone();
    let mut base = run.world;
    base.entities.list.retain(|e| own.contains(&e.id));
    let eye = |w: &redsim::world::World, i: usize| redsim::entity::mob::villager::eye_pos(w, i);
    let zombies: Vec<usize> = base.entities.list.iter().enumerate().filter(|(_, e)| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Zombie)).map(|(i, _)| i).collect();
    if zombies.is_empty() {
        bail!("no zombie");
    }
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
        println!("{label}:");
        for (i, e) in w.entities.list.iter().enumerate() {
            let EntityKind::Mob(m) = &e.kind else { continue };
            if m.ty != MobType::Villager {
                continue;
            }
            // the nearest zombie (one per cluster)
            let z = *zombies.iter().min_by(|a, b| w.entities.list[**a].pos.distance_to_sqr(e.pos).total_cmp(&w.entities.list[**b].pos.distance_to_sqr(e.pos))).unwrap_or(&zombies[0]);
            let ze = eye(&w, z);
            // standing, then asleep on its bed (LivingEntity.startSleeping puts it at bed + 0.6875,
            // then gravity settles its 0.2 x 0.2 SLEEPING box on the bed's top, 0.5625: its eye
            // 0.2 above that)
            let mut eyes = vec![("standing", eye(&w, i))];
            if let Some(home) = m.brain.as_deref().and_then(|b| b.home) {
                eyes.push(("asleep", redsim::entity::Vec3::new(home.x as f64 + 0.5, home.y as f64 + 0.5625 + 0.2, home.z as f64 + 0.5)));
            }
            for (how, ve) in eyes {
                let what = match redsim::entity::mob::los::first_hit(&w, ve, ze) {
                    None => "SEES its zombie".to_string(),
                    Some(p) => format!("blind (stopped at {} {} {}: {})", p.x, p.y, p.z, w.get(p)),
                };
                println!("  villager at {:.2} {:.2} {:.2} {how}, eye {:.2} {:.4} {:.2}, zombie eye {:.2} {:.4} {:.2}: {what}", e.pos.x, e.pos.y, e.pos.z, ve.x, ve.y, ve.z, ze.x, ze.y, ze.z);
            }
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
