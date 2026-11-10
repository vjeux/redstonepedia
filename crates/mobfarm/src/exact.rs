//! `mctas mobfarm compare`: a mob farm run in the game and in redsim side by side, compared tick
//! for tick (track tickexact; docs/mechanics.md "The bridge's seeds"; `redsim::exact`).
//!
//! The game side is the bridge: a worker of its own (26.3 keeps the clock, the weather and the game
//! rules server-wide), one world sandbox seeded with the run's seed, every random in it seeded from
//! that seed (bridge `Exact.java`), so the run repeats. The farm is built the way redsim's runner
//! builds it (`farm::Run`): the blocks raw in the spec's order, every wire's shape, a `self` update
//! of every block, the judge's chest; `SETTLE_TICKS` ticks; the daylight detectors, the design's
//! entities (summoned seeded, each mob with the run's follow-range bonus) and each villager's HOME
//! (its bed's POI taken) and LAST_SLEPT; then the run. Every tick both sides report a frame
//! (`redsim::exact::Frame`) and the compare keeps the first difference, its context (the level
//! random's draw distance; with `--probe` the game's draws of that tick, by call site, from a
//! second, identical run) and the fields redsim does not model yet.

use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Worker, WorkerConfig};
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, SETTLE_TICKS};
use redsim::exact::{Compare, EntityFrame, Frame, Watch};
use serde_json::{Value, json};

/// A compare's options.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: i64,
    /// Game ticks after the entities arrive.
    pub ticks: u32,
    pub weather: bool,
    /// On a difference, run again with the game's draw log around it.
    pub probe: bool,
    /// Ticks per traced step.
    pub batch: u32,
    /// Keep comparing after the first difference (to the end), to count the ticks run.
    pub full: bool,
    /// The bare scenario: no AFK player, the void's biome, `spawn_mobs` off (redsim without its
    /// AFK spot): the mob layer without natural spawning, fire spread or thunder.
    pub bare: bool,
    /// Print the game's draw log of this game time (every stream, by call site).
    pub draws_at: Option<i64>,
    /// A terrain sample (village-edge, plains, forest): the design built at its site.
    pub terrain: Option<String>,
    /// The day time the farm starts at (the steady state's when `None`).
    pub day0: Option<i64>,
    /// The judge's build `k` (`circuit::mobfarm::judge`: rotation k / 2 at the k % 2-th position
    /// in the void, the k-th site in a terrain sample) and its run `i`: the seed
    /// `JUDGE_SEED0 + 1000 k + i` and the judge's walking (random) and weather (a terrain
    /// sample's natural cycle).
    pub build: Option<(u32, u32)>,
    /// No mob spawning on either side (the bridge's `spawn_mobs` and custom spawners off; redsim's
    /// natural spawner off, its chunk shuffle still drawn, no custom spawners): the random ticks,
    /// weather and fire alone (terrain's regression check).
    pub no_spawning: bool,
    /// Only these kinds of difference count (`redsim::exact::Compare::only`: world, blocks,
    /// entities, or a category's name); empty: every one.
    pub only: Vec<String>,
}

/// The judge's seed of build `k`'s run `i` (`circuit::mobfarm::build_runs`).
pub fn judge_seed(k: u32, i: u32) -> i64 {
    redsim::entity::mob::farm::JUDGE_SEED0 + k as i64 * 1000 + i as i64
}

/// The judge's run options with the compare's day time.
pub fn opts_of(opts: &Opts) -> RunOpts {
    let mut o = match opts.build {
        // the judge's run: its seed, its random walking policy
        Some((k, i)) => redsim::entity::mob::farm::judge_opts(judge_seed(k, i), opts.ticks, redsim::entity::mob::farm::WalkMode::Random),
        None => run_opts(opts.seed, opts.ticks),
    };
    if let Some(d) = opts.day0 {
        o.day0 = d;
    }
    // the weather cycle on both sides (mobinfra): redsim's from its stationary state, handed to
    // the bridge at the start (`GameFarm::start_with`), then each side's cycle on its level random;
    // the judge's terrain runs have it on
    if weather_on(opts) {
        o.weather = redsim::weather::Plan::Cycle;
    }
    o
}

/// (tickexact 2026-11-10) `MCTAS_EXACT_RANDOM_TICKS=N`: the compare's `random_tick_speed` on both
/// sides (3 without it).
pub fn random_tick_speed() -> Option<u32> {
    std::env::var("MCTAS_EXACT_RANDOM_TICKS").ok().and_then(|v| v.parse().ok())
}

/// The compare runs the weather: asked for, or a judge's terrain run (the judge's weather).
pub fn weather_on(opts: &Opts) -> bool {
    opts.weather || (opts.build.is_some() && opts.terrain.is_some())
}

/// `MCTAS_EXACT_TRACE="N:T"` (N an entity number or `level`): whose draws to trace at game time T.
fn trace_probe() -> Option<(Option<u32>, i64)> {
    static P: std::sync::OnceLock<Option<(Option<u32>, i64)>> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        let v = std::env::var("MCTAS_EXACT_TRACE").ok()?;
        let (who, at) = v.split_once(':')?;
        Some((if who == "level" { None } else { Some(who.parse().ok()?) }, at.parse().ok()?))
    })
}

/// (a probe: `MCTAS_EXACT_TRACE="N:T"` prints redsim's draws of entity #N's own random during
/// game time T, each with its call site; N = level for the level random) Arms it before the tick.
/// Also `MCTAS_EXACT_EXPLAIN="type:x,y,z:T"`: redsim's natural-spawn checks of that type at that
/// cell before game time T's tick (`natural::explain`).
fn trace_start(w: &mut redsim::world::World, t: i64) {
    // (a probe: `MCTAS_EXACT_TS_CHECK="T0:T1"`: before each tick in T0..=T1, the sections'
    // ticking counts every write keeps against a recount of the cells)
    static TS: std::sync::OnceLock<Option<(i64, i64)>> = std::sync::OnceLock::new();
    if let Some((t0, t1)) = *TS.get_or_init(|| {
        let v = std::env::var("MCTAS_EXACT_TS_CHECK").ok()?;
        let (a, b) = v.split_once(':')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    }) && t >= t0
        && t <= t1
        && let Some(kept) = w.ticking_sections.as_ref()
    {
        let fresh = redsim::crops::TickingSections::build(w);
        for (s, a, b) in kept.differences(&fresh) {
            eprintln!("ticking sections t{t}: section {s:?} kept {a} recount {b}");
        }
        // and at the window's first tick: every distinct state of the world whose random ticking
        // redsim answers otherwise than the game's block table (`BlockState.isRandomlyTicking`)
        if t == t0 {
            for (p, b) in w.outer.iter() {
                if redsim::crops::ticks_in_section(b) {
                    eprintln!("ticking state t{t}: outside the box at {p}: {b}");
                }
            }
            let mut seen = std::collections::HashSet::new();
            let size = w.size;
            for (x, y, z) in (0..size[1]).flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| (x, y, z)))) {
                let b = w.get(w.min.offset(x, y, z));
                if !seen.insert(b) {
                    continue;
                }
                let name = b.to_string();
                let game = redsim::terrain::blocks::id_of(&name).map(|id| redsim::terrain::blocks::props(id).random_ticking);
                let ours = redsim::crops::ticks_in_section(&b);
                if game != Some(ours) {
                    eprintln!("ticking state t{t}: {name}: redsim {ours}, the game's table {game:?}");
                }
            }
        }
    }
    static EXPLAIN: std::sync::OnceLock<Option<(String, redsim::pos::BlockPos, i64)>> = std::sync::OnceLock::new();
    if let Some((ty, p, at)) = EXPLAIN
        .get_or_init(|| {
            let v = std::env::var("MCTAS_EXACT_EXPLAIN").ok()?;
            let f: Vec<&str> = v.split(':').collect();
            let (ty, c, at) = (f.first()?, f.get(f.len().checked_sub(2)?)?, f.last()?);
            let ty = if f.len() == 4 { format!("{ty}:{}", f[1]) } else { ty.to_string() };
            let c: Vec<i32> = c.split(',').filter_map(|s| s.parse().ok()).collect();
            Some((ty, redsim::pos::BlockPos::new(*c.first()?, *c.get(1)?, *c.get(2)?), at.parse().ok()?))
        })
        .as_ref()
        && *at == t
    {
        eprintln!("explain at game time {t}: {}", redsim::entity::mob::natural::explain(w, ty, *p));
    }
    let w: &redsim::world::World = w;
    let Some((who, at)) = trace_probe().filter(|(_, at)| *at == t) else { return };
    let addr = match who {
        None => w.level_random.as_ref().map(|r| r as *const _ as usize).unwrap_or(0),
        Some(id) => w.entities.list.iter().find(|e| e.id == id).and_then(|e| e.random.as_ref()).map(|r| r as *const _ as usize).unwrap_or(0),
    };
    eprintln!("redsim's draws at game time {at}:");
    redsim::rng::TRACE.store(addr, std::sync::atomic::Ordering::Relaxed);
}

/// `MCTAS_EXACT_ENTITY="N:T0:T1"`: entity #N's frames to print at game times T0..=T1.
fn entity_probe() -> Option<(&'static [u32], i64, i64)> {
    static P: std::sync::OnceLock<Option<(Vec<u32>, i64, i64)>> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let v = std::env::var("MCTAS_EXACT_ENTITY").ok()?;
        let f: Vec<&str> = v.split(':').collect();
        let ids: Vec<u32> = f.first()?.split(',').filter_map(|s| s.parse().ok()).collect();
        Some((ids, f.get(1)?.parse().ok()?, f.get(2)?.parse().ok()?))
    })
    .as_ref()
    .map(|(ids, a, b)| (ids.as_slice(), *a, *b))
}

/// The `MCTAS_EXACT_ENTITY` probe's lines for one compared tick: each probed entity's frame on
/// both sides (pos, motion, on ground, fire, health, hurt time, yaw, its memories).
fn print_probed(f: &Frame, ours: &Frame) {
    print_summons(f, ours);
    let Some((ids, t0, t1)) = entity_probe() else { return };
    if f.t < t0 || f.t > t1 {
        return;
    }
    for &n in ids {
        let g = f.entities.iter().find(|e| e.n == n);
        let o = ours.entities.iter().find(|e| e.n == n);
        eprintln!("t{} #{n} game:   {:?}", f.t, g.map(|e| (&e.ty, &e.item, e.pos, e.motion, e.on_ground, e.fire, e.health, e.hurt_time, e.yaw, &e.memories)));
        eprintln!("t{} #{n} redsim: {:?}", f.t, o.map(|e| (&e.ty, &e.item, e.pos, e.motion, e.on_ground, e.fire, e.health, e.hurt_time, e.yaw, &e.memories)));
        // (tickexact 2026-11-10) the game's other entities within 3 of it (who picks it up, who
        // pushes it); MCTAS_EXACT_NEAR=R widens it (whom it targets)
        if let Some(c) = g {
            let r: f64 = std::env::var("MCTAS_EXACT_NEAR").ok().and_then(|v| v.parse().ok()).unwrap_or(3.0);
            let near: Vec<String> = f
                .entities
                .iter()
                .filter(|e| e.n != n && (0..3).map(|k| (e.pos[k] - c.pos[k]).powi(2)).sum::<f64>() < r * r)
                .map(|e| format!("#{} {} at {:.2} {:.2} {:.2}", e.n, e.ty, e.pos[0], e.pos[1], e.pos[2]))
                .collect();
            if !near.is_empty() {
                eprintln!("t{} #{n} near (game): {}", f.t, near.join("; "));
            }
        }
    }
}

/// `MCTAS_EXACT_SUMMONS=1`: a probe of the golem summons on both sides: each iron golem entering
/// (`summon t<T> <side> golem #<n>`) and each villager's GOLEM_DETECTED_RECENTLY appearing or
/// going (`gdr t<T> <side> villager #<n>: set <value> | free`).
fn print_summons(f: &Frame, ours: &Frame) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("MCTAS_EXACT_SUMMONS").is_ok()) {
        return;
    }
    thread_local! {
        static HELD: std::cell::RefCell<[std::collections::BTreeMap<u32, String>; 2]> = Default::default();
    }
    HELD.with(|h| {
        let mut h = h.borrow_mut();
        for (side, (name, frame)) in [("game", f), ("redsim", ours)].into_iter().enumerate() {
            for n in &frame.entered {
                if let Some(e) = frame.entities.iter().find(|e| e.n == *n && e.ty.ends_with("iron_golem")) {
                    eprintln!("summon t{} {name} golem #{n} at ({:.1}, {:.1}, {:.1})", frame.t, e.pos[0], e.pos[1], e.pos[2]);
                }
            }
            for e in frame.entities.iter().filter(|e| e.ty.ends_with("villager")) {
                let now = e.memories.as_ref().and_then(|m| m.get("golem_detected_recently").cloned());
                let was = h[side].get(&e.n).cloned();
                match (&was, &now) {
                    (None, Some(v)) => eprintln!("gdr t{} {name} villager #{}: set {v}", frame.t, e.n),
                    (Some(_), None) => eprintln!("gdr t{} {name} villager #{}: free", frame.t, e.n),
                    _ => {}
                }
                match now {
                    Some(v) => h[side].insert(e.n, v),
                    None => h[side].remove(&e.n),
                };
            }
        }
    });
}

/// `MCTAS_EXACT_TALLY="x0,y0,z0,x1,y1,z1"` (world coordinates): a probe tallying, on each side,
/// the entity ticks and the distinct entities of each type inside that box after the arrival,
/// printed when the compare ends (a statistical side by side once the two runs have diverged).
fn tally_box() -> Option<[f64; 6]> {
    static P: std::sync::OnceLock<Option<[f64; 6]>> = std::sync::OnceLock::new();
    *P.get_or_init(|| {
        let v = std::env::var("MCTAS_EXACT_TALLY").ok()?;
        let c: Vec<f64> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        (c.len() == 6).then(|| [c[0].min(c[3]), c[1].min(c[4]), c[2].min(c[5]), c[0].max(c[3]), c[1].max(c[4]), c[2].max(c[5])])
    })
}

/// The tally's counts: per side, per type, (entity ticks, distinct entities).
#[derive(Default)]
struct Tally {
    sides: [std::collections::BTreeMap<String, (u64, std::collections::BTreeSet<u32>)>; 2],
    golems: [GolemLog; 2],
}

/// `MCTAS_EXACT_GOLEMS=1`: a probe following every iron golem on each side after the arrival:
/// when it entered, when it last stood in a farm villager's sensing box (`NearestLivingEntitySensor`:
/// the villager's box inflated 16 each way, against the golem's box), and every target it took
/// (`target` changes, with the target's type), printed when the compare ends.
fn golem_probe() -> bool {
    static P: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *P.get_or_init(|| std::env::var("MCTAS_EXACT_GOLEMS").is_ok_and(|v| v != "0"))
}

#[derive(Default)]
struct GolemLog {
    /// Per golem: (first tick seen, last tick inside a villager's box, ticks inside, last target,
    /// targets taken as (tick, target text, the target's type), inside when it first had a target).
    golems: std::collections::BTreeMap<u32, GolemTrack>,
    /// Ticks no golem stood in any farm villager's box.
    ticks_without: u64,
    ticks: u64,
}

#[derive(Default, Clone)]
struct GolemTrack {
    first: i64,
    last_inside: Option<i64>,
    inside_ticks: u64,
    target: Option<String>,
    targets: Vec<(i64, String, String)>,
}

impl GolemLog {
    fn add(&mut self, t: i64, frames: &[redsim::exact::EntityFrame]) {
        let villagers: Vec<&redsim::exact::EntityFrame> = frames.iter().filter(|e| e.ty == "villager").collect();
        let inside = |g: &redsim::exact::EntityFrame| {
            villagers.iter().any(|v| {
                let (dx, dz) = ((g.pos[0] - v.pos[0]).abs(), (g.pos[2] - v.pos[2]).abs());
                let (gy0, gy1, vy0, vy1) = (g.pos[1], g.pos[1] + 2.7, v.pos[1] - 16.0, v.pos[1] + 1.95 + 16.0);
                dx < 16.0 + 0.3 + 0.7 && dz < 16.0 + 0.3 + 0.7 && gy1 > vy0 && gy0 < vy1
            })
        };
        let mut any = false;
        for g in frames.iter().filter(|e| e.ty == "iron_golem") {
            let tr = self.golems.entry(g.n).or_insert_with(|| GolemTrack { first: t, ..GolemTrack::default() });
            if inside(g) {
                tr.last_inside = Some(t);
                tr.inside_ticks += 1;
                any = true;
            }
            if g.target != tr.target {
                if let Some(tg) = &g.target {
                    let n = tg.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).next().and_then(|s| s.parse::<u32>().ok());
                    let ty = n.and_then(|n| frames.iter().find(|e| e.n == n)).map(|e| e.ty.clone()).unwrap_or_else(|| format!("({tg})"));
                    tr.targets.push((t, tg.clone(), ty));
                }
                tr.target = g.target.clone();
            }
        }
        self.ticks += 1;
        if !any {
            self.ticks_without += 1;
        }
    }

    fn print(&self, side: &str) {
        let n = self.golems.len();
        let mut exits: Vec<i64> = self.golems.values().filter_map(|g| g.last_inside.map(|l| l - g.first)).collect();
        exits.sort();
        let median = exits.get(exits.len() / 2).copied().unwrap_or(0);
        let mean = if exits.is_empty() { 0.0 } else { exits.iter().sum::<i64>() as f64 / exits.len() as f64 };
        let mut by_type: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut with_target = 0;
        for g in self.golems.values() {
            if !g.targets.is_empty() {
                with_target += 1;
            }
            for (_, _, ty) in &g.targets {
                *by_type.entry(ty.clone()).or_default() += 1;
            }
        }
        println!("  {side}: {n} golems; time from entering to leaving the villagers' sensing boxes: median {median} gt, mean {mean:.0} gt; {} of {} ticks with no golem in any box; {with_target} golems took a target: {}", self.ticks_without, self.ticks, by_type.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
        for (id, g) in &self.golems {
            let tg: Vec<String> = g.targets.iter().map(|(t, _, ty)| format!("t{t} {ty}")).collect();
            println!("    #{id}: in at t{}, last inside t{}, {} ticks inside{}", g.first, g.last_inside.map(|l| l.to_string()).unwrap_or("-".into()), g.inside_ticks, if tg.is_empty() { String::new() } else { format!(", targets {}", tg.join(" ")) });
        }
    }
}

impl Tally {
    fn add(&mut self, side: usize, t: i64, frames: &[redsim::exact::EntityFrame]) {
        if golem_probe() {
            self.golems[side].add(t, frames);
        }
        let Some(b) = tally_box() else { return };
        for e in frames {
            let [x, y, z] = e.pos;
            if x >= b[0] && x <= b[3] && y >= b[1] && y <= b[4] && z >= b[2] && z <= b[5] {
                let s = self.sides[side].entry(e.ty.clone()).or_default();
                s.0 += 1;
                s.1.insert(e.n);
            }
        }
    }

    fn print(&self) {
        if golem_probe() {
            println!("golems after the arrival:");
            self.golems[0].print("game");
            self.golems[1].print("redsim");
        }
        let Some(b) = tally_box() else { return };
        println!("tally inside {b:?} after the arrival (entity ticks, distinct entities):");
        for (k, side) in ["game", "redsim"].iter().enumerate() {
            let rows: Vec<String> = self.sides[k].iter().map(|(t, (n, ids))| format!("{t} {n} ({})", ids.len())).collect();
            println!("  {side}: {}", rows.join(", "));
        }
    }
}

/// The game's side alone over `seeds` (no redsim): each run's custom spawner events seen as
/// entities entering (a siege's zombies, a patrol's pillagers, phantoms, cats, a trader), to find
/// a seed where one fires (the siege and patrol fixtures).
pub fn scan(version: &str, design: &std::path::Path, opts: &Opts, seeds: std::ops::RangeInclusive<i64>, want: &str) -> Result<Vec<(i64, i64, usize)>> {
    let seeds: Vec<i64> = seeds.collect();
    let results = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(&seed) = seeds.get(i) else { break };
                    let o2 = Opts { seed, ..opts.clone() };
                    let r = scan_one(version, design, &o2, want);
                    match r {
                        Ok(Some((t, n))) => {
                            println!("seed {seed}: {n} {want} entered from game time {t}");
                            results.lock().unwrap().push((seed, t, n));
                        }
                        Ok(None) => println!("seed {seed}: no {want}"),
                        Err(e) => println!("seed {seed}: {e:#}"),
                    }
                }
            });
        }
    });
    Ok(results.into_inner().unwrap())
}

fn scan_one(version: &str, design: &std::path::Path, opts: &Opts, want: &str) -> Result<Option<(i64, usize)>> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start(version, &c, &spec, &o, weather_on(opts), false, place)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    let mut first = None;
    let mut count = 0usize;
    // every frame but the arrival's (the design's own mobs enter there), the settle's included
    let arrival = SETTLE_TICKS + 1;
    let mut see = |f: &Frame| {
        if f.t == arrival {
            return;
        }
        let entered = f.entities.iter().filter(|e| f.entered.contains(&e.n) && e.ty.contains(want)).count();
        if entered > 0 {
            first.get_or_insert(f.t);
            count += entered;
        }
    };
    for f in game.trace(SETTLE_TICKS as u32, false)? {
        see(&f);
    }
    while run.settle_tick() {}
    let skip = mob_count(&run.world);
    arrive(&mut run)?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses_after(&mut run.world, &bonus, skip);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, false)? {
            see(&f);
        }
        left -= n;
    }
    Ok(first.map(|t| (t, count)))
}

/// The design and its spec where the compare runs: lifted in the void, moved to its site in a
/// terrain sample (the site and the sample leaked: `RunOpts::site` is `'static`).
pub fn load_place(design: &std::path::Path, opts: &Opts) -> Result<(circuit::Circuit, FarmSpec, Place<'static>)> {
    match &opts.terrain {
        None => {
            // the judge's build k: rotation k / 2 at the (k % 2)-th position (then lifted)
            let c = match opts.build {
                Some((k, _)) => {
                    let off = clock::offsets(2)[(k % 2) as usize];
                    circuit::Circuit::load(design)?.turned_pure((k / 2) as u8)?.translated(off)?.translated([0, LIFT, 0])?
                }
                None => load_lifted(design)?,
            };
            let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
            Ok((c, spec, Place::Void { bare: opts.bare }))
        }
        Some(name) => {
            let c = circuit::Circuit::load(design)?;
            let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
            let sample = redsim::terrain::samples::sample(name).ok_or_else(|| anyhow!("no terrain sample {name}"))?;
            // the judge's build k: the k-th site (`terrain_sites`); else the first
            let site: &'static redsim::entity::mob::site::Site = match opts.build {
                Some((k, _)) => *circuit::mobfarm::terrain_sites(&c, &spec, sample).map_err(|e| anyhow!(e))?.get(k as usize).ok_or_else(|| anyhow!("no site {k} in {name}"))?,
                None => Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow!(e))?)),
            };
            let terrain: &'static redsim::terrain::format::Terrain = Box::leak(Box::new(redsim::terrain::format::Terrain::decode(&std::fs::read(sample.file)?).map_err(|e| anyhow!(e))?));
            eprintln!("terrain {}: offset {:?}, {} blocks placed, {} dug, AFK at {:.1} {:.1} {:.1}", site.sample, site.offset, site.placed, site.dug, site.afk.x, site.afk.y, site.afk.z);
            let moved = site.spec(&spec);
            Ok((c, moved, Place::Terrain { site, terrain }))
        }
    }
}

/// The follow-range bonus of each design mob (`Mob.finalizeSpawn`'s `random_spawn_bonus`), a run
/// input both sides take: `triangle(0, 0.11485)` draws from the run seed ^ "bonus", in summon order.
pub fn follow_bonuses(seed: i64, n: usize) -> Vec<f64> {
    let mut r = redsim::rng::LegacyRandom::new(seed ^ 0x626f_6e75_73);
    (0..n).map(|_| r.triangle(0.0, 0.114_850_000_000_000_01)).collect()
}

/// The chunk box a farm's world needs: its box's chunks.
pub fn chunk_box(spec: &FarmSpec) -> [i32; 4] {
    [spec.lo.x.div_euclid(16), spec.lo.z.div_euclid(16), spec.hi.x.div_euclid(16), spec.hi.z.div_euclid(16)]
}

/// The game's side of a farm run.
pub struct GameFarm {
    w: Worker,
    sb: u32,
    lo: [i32; 3],
    hi: [i32; 3],
    /// The chunks the level ticks, in its order (the forced box and the AFK player's simulation
    /// square), for redsim's chunk ticks.
    pub ticking: Vec<[i32; 2]>,
    /// The chunks it spawns in, before the shuffle (`ChunkMap.collectSpawningChunks`: the
    /// player's block-ticking ring within 128 blocks), and the natural spawn chunk count (the mob
    /// cap's divisor).
    pub spawning: Vec<[i32; 2]>,
    /// The chunks collectSpawningChunks walks, in its order, before its players' 128-block filter
    /// (redsim filters them each tick: the AFK player moves when pushed).
    pub spawn_candidates: Vec<[i32; 2]>,
    pub spawn_chunk_count: u32,
    /// The chunks where entities may spawn and tick (`ServerLevel.canSpawnEntitiesInChunk`).
    pub entity_ticking: Vec<[i32; 2]>,
    /// The design's block states at the spec's (moved) cells.
    text: std::collections::HashMap<[i32; 3], String>,
    /// The chunks loaded at full status (`hasChunksAt`).
    pub full: Vec<[i32; 2]>,
    /// The level runs the five custom spawners.
    pub spawners: bool,
    /// (mob-AI) The game JVM's POI holder identity hashes (the bridge's `identity_hashes`).
    pub poi_hashes: std::collections::HashMap<String, i32>,
    /// The server's respawn position (`getRespawnData`), which natural spawning keeps 24 blocks
    /// clear (`isRightDistanceToPlayerAndSpawnPoint`).
    pub respawn: Option<[i32; 3]>,
    /// The level spawns mobs naturally (`spawn_mobs`; off: `--no-spawning`, or the bare scenario).
    pub natural: bool,
}

/// The design's block states at the spec's cells (moved to the site in terrain).
fn design_text(c: &circuit::Circuit, spec: &FarmSpec, place: Place) -> std::collections::HashMap<[i32; 3], String> {
    let off = match place {
        Place::Terrain { site, .. } => site.offset,
        Place::Void { .. } => [0, 0, 0],
    };
    let _ = spec;
    c.clock.design.block_list().into_iter().map(|(p, s)| ([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s)).collect()
}

/// The cells a site clears for a void design (`site::Site::world_for` -> `place::clear`: every
/// non-air cell of the footprint's columns from the design's floor up, in its order).
fn cleared_cells(site: &redsim::entity::mob::site::Site, moved: &FarmSpec) -> Vec<[i32; 3]> {
    if !site.clear_box {
        return Vec::new();
    }
    let w = site.world.read().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let (mut lo, mut hi) = (moved.blocks[0].0, moved.blocks[0].0);
    for (p, _) in &moved.blocks {
        lo = redsim::BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = redsim::BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    redsim::terrain::place::footprint_cells(&w, lo, hi).into_iter().filter(|p| !w.get(*p).is_air()).map(pos).collect()
}

/// Put a terrain sample into the sandbox (the bridge's `terrain` op: each section's states and
/// saved light, the biome cells; its entities stay out like redsim's site world), then check the
/// level's blocks and light against the sample's, cell for cell (an import that differs fails).
pub fn import_terrain(w: &mut Worker, sb: u32, t: &redsim::terrain::format::Terrain) -> Result<()> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let [ox, oy, oz] = t.origin;
    let [sx, sy, sz] = t.size;
    let top = oy + sy - 1;
    let mut sections = Vec::new();
    for cx in ox.div_euclid(16)..=(ox + sx - 1).div_euclid(16) {
        for cz in oz.div_euclid(16)..=(oz + sz - 1).div_euclid(16) {
            for cy in oy.div_euclid(16)..=top.div_euclid(16) {
                let mut palette: Vec<String> = vec!["minecraft:air".into()];
                let mut ids: std::collections::HashMap<String, u16> = std::collections::HashMap::from([("minecraft:air".to_string(), 0)]);
                let mut cells = Vec::with_capacity(8192);
                let mut sky = vec![0u8; 2048];
                let mut block = vec![0u8; 2048];
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let (wx, wy, wz) = (cx * 16 + x, cy * 16 + y, cz * 16 + z);
                            let state = t.state(wx, wy, wz).unwrap_or("minecraft:air");
                            let id = *ids.entry(state.to_string()).or_insert_with(|| {
                                palette.push(state.to_string());
                                (palette.len() - 1) as u16
                            });
                            cells.extend_from_slice(&id.to_le_bytes());
                            // outside the box: open sky above it, dark below it
                            let s = t.saved_sky(wx, wy, wz).unwrap_or(if wy > top { 15 } else { 0 });
                            let b = t.saved_block(wx, wy, wz).unwrap_or(0);
                            let i = ((y << 8) | (z << 4) | x) as usize;
                            sky[i >> 1] |= (s & 15) << ((i & 1) * 4);
                            block[i >> 1] |= (b & 15) << ((i & 1) * 4);
                        }
                    }
                }
                sections.push(json!({"x": cx, "y": cy, "z": cz, "palette": palette, "cells": b64.encode(&cells), "sky": b64.encode(&sky), "block": b64.encode(&block)}));
            }
        }
    }
    let biome_cells: Vec<u8> = t.biomes.iter().flat_map(|v| v.to_le_bytes()).collect();
    let n = sections.len();
    let structures: Vec<Value> = t.structures.iter().map(|s| json!({"id": s.id, "min": s.min, "max": s.max, "pieces": s.pieces})).collect();
    let inhabited: Vec<Value> = t.inhabited.iter().map(|([x, z], v)| json!([x, z, v])).collect();
    let block_entities: Vec<Value> = t.block_entities.iter().map(|b| json!({"pos": b.pos, "snbt": b.snbt})).collect();
    let heights: Vec<String> = t.heightmaps.iter().map(|m| b64.encode(m.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())).collect();
    let maps: Vec<&str> = redsim::terrain::Heightmap::ALL.iter().take(t.heightmaps.len()).map(|m| m.key()).collect();
    let mut req = json!({
        "op": "terrain", "sandbox": sb, "sections": sections,
        "biomes": {"origin": t.biome_origin, "size": t.biome_size, "palette": t.biome_palette, "cells": b64.encode(&biome_cells)},
        "structures": structures, "inhabited": inhabited, "block_entities": block_entities, "zoom_seed": t.zoom_seed, "spawn": t.spawn,
        "world_seed": t.seed,
        "heightmaps": {"maps": maps, "origin": [ox, oz], "size": [sx, sz], "data": heights},
    });
    // (a bisect switch: the sandbox's own zoom seed)
    if std::env::var_os("MCTAS_EXACT_SANDBOX_ZOOM").is_some() {
        req.as_object_mut().map(|o| o.remove("zoom_seed"));
    }
    let v = w.call(req)?;
    eprintln!("terrain: {n} sections in {} chunks, {} structures, {} block entities, {} chunks' inhabited time", v["chunks"], v["structures"], v["block_entities"], t.inhabited.len());
    // the check: every cell's block and light as the level reads them
    let from = t.origin;
    let to = [ox + sx - 1, top, oz + sz - 1];
    let region = w.get_region(sb, from, to)?;
    let lb = w.call(json!({"op": "light_box", "sandbox": sb, "from": from, "to": to}))?;
    let sky = b64.decode(lb["sky"].as_str().unwrap_or(""))?;
    let blk = b64.decode(lb["block"].as_str().unwrap_or(""))?;
    let (mut bad_block, mut bad_sky, mut bad_light, mut first) = (0usize, 0usize, 0usize, None);
    // the sample's states as the game writes them back (its strings may leave out default
    // properties: "minecraft:deepslate" is deepslate[axis=y])
    let v = w.call(json!({"op": "normalize_states", "states": t.palette}))?;
    let canon: Vec<String> = v["states"].as_array().map(|a| a.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let mut i = 0usize;
    for y in oy..=top {
        for z in oz..oz + sz {
            for x in ox..ox + sx {
                let want = if t.contains(x, y, z) { canon[t.cells[t.index(x, y, z)] as usize].as_str() } else { "minecraft:air" };
                let got = region.get([x, y, z]).unwrap_or("?");
                if got != want {
                    bad_block += 1;
                    first.get_or_insert(format!("block {x} {y} {z}: game {got}, sample {want}"));
                }
                if t.saved_sky(x, y, z).is_some_and(|s| s != sky[i]) {
                    bad_sky += 1;
                    first.get_or_insert(format!("sky light {x} {y} {z}: game {}, sample {:?}", sky[i], t.saved_sky(x, y, z)));
                }
                if t.saved_block(x, y, z).is_some_and(|s| s != blk[i]) {
                    bad_light += 1;
                    first.get_or_insert(format!("block light {x} {y} {z}: game {}, sample {:?}", blk[i], t.saved_block(x, y, z)));
                }
                i += 1;
            }
        }
    }
    eprintln!("terrain import check: {} cells, {bad_block} blocks, {bad_sky} sky light and {bad_light} block light values differ from the sample", t.volume());
    if bad_block + bad_sky + bad_light > 0 {
        bail!("the terrain import differs from the sample: {}", first.unwrap_or_default());
    }
    Ok(())
}

fn pos(p: redsim::BlockPos) -> [i32; 3] {
    [p.x, p.y, p.z]
}

/// The real-game gate's AFK spot of a void farm (`circuit::mobfarm_game`): 60 blocks under the
/// chest, a stone over its head (redsim's `World::afk` is the same).
pub fn afk_spot(spec: &FarmSpec) -> [i32; 3] {
    [spec.output.x, spec.lo.y - 60, spec.output.z]
}

/// The gate's forced chunks of a void farm: its box and 8 blocks around (`forceload add`).
pub fn forced_chunks(spec: &FarmSpec) -> [i32; 4] {
    [(spec.lo.x - 8).div_euclid(16), (spec.lo.z - 8).div_euclid(16), (spec.hi.x + 8).div_euclid(16), (spec.hi.z + 8).div_euclid(16)]
}

/// Where a compare runs.
#[derive(Clone, Copy)]
pub enum Place<'a> {
    /// The void, in the real-game gate's scenario (`bare`: without its player, biome and
    /// `spawn_mobs`); the design lifted by [`LIFT`].
    Void { bare: bool },
    /// A terrain sample: the design moved to its site (`redsim::entity::mob::site`), natural
    /// spawning on Hard around its AFK spot.
    Terrain { site: &'a redsim::entity::mob::site::Site, terrain: &'a redsim::terrain::format::Terrain },
}

impl GameFarm {
    /// A fresh worker and world sandbox, with the farm built and its chest placed (before the
    /// settle). The void (`Place::Void`): the real-game gate's scenario (`circuit::mobfarm_game`: a
    /// plains void, Hard, `spawn_mobs` on and natural monsters off, phantoms, patrols and traders
    /// off, the clock and (with `weather`) the weather running, the AFK player creative under a
    /// stone 60 blocks under the chest, simulation and view distance 6); `bare`: no player, the
    /// void's biome, `spawn_mobs` off (the mob layer seen without natural spawning, fire spread or
    /// thunder). A terrain site: the sample imported ([`import_terrain`]), the design's box
    /// cleared like the site clears it, natural monsters on, the AFK player at the site's spot.
    pub fn start(version: &str, c: &circuit::Circuit, spec: &FarmSpec, o: &RunOpts, weather: bool, trace_randoms: bool, place: Place) -> Result<GameFarm> {
        Self::start_with(version, c, spec, o, weather, trace_randoms, place, true)
    }

    /// [`GameFarm::start`]; `spawning`: the level spawns mobs, naturally (`spawn_mobs`) and by its
    /// five custom spawners (the real game's overworld does; the void gate's rules leave the cats,
    /// terrain's every one). Off (`mobfarm compare --no-spawning`): neither, the chunk shuffle still
    /// drawn (terrain's regression check of the random ticks).
    #[allow(clippy::too_many_arguments)]
    pub fn start_with(version: &str, c: &circuit::Circuit, spec: &FarmSpec, o: &RunOpts, weather: bool, trace_randoms: bool, place: Place, spawn_on: bool) -> Result<GameFarm> {
        let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 4096, ..Default::default() })?;
        let (chunks, biome, bare) = match place {
            Place::Void { bare } => (forced_chunks(spec), if bare { Value::Null } else { json!("minecraft:plains") }, bare),
            Place::Terrain { terrain, .. } => {
                let [ox, _, oz] = terrain.origin;
                let [sx, _, sz] = terrain.size;
                ([ox.div_euclid(16), oz.div_euclid(16), (ox + sx - 1).div_euclid(16), (oz + sz - 1).div_euclid(16)], json!("minecraft:plains"), false)
            }
        };
        let spawners = spawn_on && !bare;
        // a terrain sample's sandbox reports its world's sea level (the overworld's 63)
        let sea_level = if matches!(place, Place::Terrain { .. }) { json!(63) } else { Value::Null };
        let v = w.call(json!({"op": "create", "chunks": chunks, "seed": o.seed, "biome": biome, "custom_spawners": spawners, "sea_level": sea_level}))?;
        let sb = v["sandbox"].as_u64().ok_or_else(|| anyhow!("create: no sandbox"))? as u32;
        let mut world = json!({
            "op": "world", "sandbox": sb,
            "advance_time": true, "day_time": o.day0 - SETTLE_TICKS,
            "advance_weather": weather, "spawn_mobs": !bare && spawn_on, "spawn_monsters": false,
            "rules": {"spawn_phantoms": "false", "spawn_patrols": "false", "spawn_wandering_traders": "false"},
            "difficulty": "hard",
            "trace_randoms": trace_randoms,
        });
        // redsim's starting weather (its stationary state, `farm::Run::start`'s), so both sides
        // begin equal without a shared draw
        if weather {
            let (clear, rain, thunder, raining, thundering, rain_level, thunder_level) = redsim::weather::Weather::new(o.weather, o.seed).bridge_state();
            world["weather"] = json!({"clear": clear, "rain": rain, "thunder": thunder, "raining": raining, "thundering": thundering, "rain_level": rain_level, "thunder_level": thunder_level});
        }
        match place {
            Place::Void { bare: false } => {
                let bot = afk_spot(spec);
                w.set_blocks(sb, &[([bot[0], bot[1] + 3, bot[2]], "minecraft:stone")], PlaceMode::Raw)?;
                world["player"] = json!({"pos": [bot[0] as f64 + 0.5, bot[1] as f64, bot[2] as f64 + 0.5], "mode": "creative", "simulation": 6, "view": 6});
            }
            Place::Void { bare: true } => {}
            Place::Terrain { site, terrain } => {
                import_terrain(&mut w, sb, terrain)?;
                let cleared = cleared_cells(site, spec);
                let air: Vec<([i32; 3], &str)> = cleared.iter().map(|p| (*p, "minecraft:air")).collect();
                for part in air.chunks(4096) {
                    w.set_blocks(sb, part, PlaceMode::Raw)?;
                }
                // the site's own blocks before the farm, as redsim's site world has them
                // (`Site::site_blocks`: the cobblestone seals, the torch or glass caps)
                let added: Vec<([i32; 3], String)> = site.site_blocks().into_iter().map(|(p, b)| (pos(p), b.to_string())).collect();
                let added: Vec<([i32; 3], &str)> = added.iter().map(|(p, s)| (*p, s.as_str())).collect();
                if !added.is_empty() {
                    w.set_blocks(sb, &added, PlaceMode::Raw)?;
                }
                world["spawn_monsters"] = json!(spawn_on);
                // the game's default rules: every custom spawner may fire
                world["rules"] = json!({"spawn_phantoms": "true", "spawn_patrols": "true", "spawn_wandering_traders": "true"});
                world["player"] = json!({"pos": [site.afk.x, site.afk.y, site.afk.z], "mode": "creative", "simulation": 6, "view": 6});
            }
        }
        let v = w.call(world).context("world sandbox")?;
        // (tickexact 2026-11-10) MCTAS_EXACT_RANDOM_TICKS=N: gamerule random_tick_speed N on both
        // sides (a stress of the random ticks; a portal's zombified piglins N/3 times as often)
        if let Some(n) = random_tick_speed() {
            let mut rules = json!({"op": "world", "sandbox": sb});
            rules["rules"] = json!({"random_tick_speed": n.to_string()});
            w.call(rules).context("random_tick_speed")?;
        }
        let full = v["full_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let ticking = v["ticking_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let spawning = v["spawning_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let entity_ticking = v["entity_ticking_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let spawn_chunk_count = v["spawn_chunk_count"].as_u64().unwrap_or(0) as u32;
        // the blocks in the spec's order (redsim's: sorted by x, y, z), the detectors held back
        let text = design_text(c, spec, place);
        let mut rest = Vec::new();
        for (p, b) in &spec.blocks {
            if matches!(b, redsim::block::Block::DaylightDetector { .. }) {
                continue;
            }
            let s = text.get(&pos(*p)).ok_or_else(|| anyhow!("no design block at {p}"))?;
            rest.push((pos(*p), s.clone()));
        }
        // (mob-AI) the POI holders' identity hashes: this JVM's, fixed for its life
        let hv = w.call(json!({"op": "identity_hashes"}))?;
        let poi_hashes: std::collections::HashMap<String, i32> = hv["poi_types"].as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_i64().map(|h| (k.clone(), h as i32))).collect()).unwrap_or_default();
        let mut g = GameFarm { w, sb, lo: pos(spec.lo), hi: pos(spec.hi), ticking, spawning, spawn_candidates: Vec::new(), entity_ticking, spawn_chunk_count, text, full, spawners, poi_hashes, respawn: None, natural: spawn_on && !bare };
        let bes: Vec<([i32; 3], String)> = spec.block_entities.iter().map(|(p, s)| (pos(*p), s.clone())).collect();
        g.place_with(&rest, &bes)?;
        let out = pos(spec.output);
        let here = g.w.get_region(sb, out, out)?;
        if !here.get(out).is_some_and(|s| s.starts_with("minecraft:chest")) {
            g.w.set_blocks(sb, &[(out, "minecraft:chest[facing=north,type=single,waterlogged=false]")], PlaceMode::Raw)?;
        }
        g.w.merge_block_entity(sb, out, "{Items:[]}")?;
        // the lists again, right before the first tick (the farm built, nothing ticked since)
        let v = g.w.call(json!({"op": "chunk_lists", "sandbox": sb}))?;
        let list = |k: &str| -> Vec<[i32; 2]> { v[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
        g.ticking = list("ticking_chunks");
        g.spawning = list("spawning_chunks");
        g.spawn_candidates = list("spawn_candidates");
        g.entity_ticking = list("entity_ticking_chunks");
        g.full = list("full_chunks");
        g.spawn_chunk_count = v["spawn_chunk_count"].as_u64().unwrap_or(0) as u32;
        g.respawn = v["respawn"].as_array().map(|a| [a[0].as_i64().unwrap_or(0) as i32, a[1].as_i64().unwrap_or(0) as i32, a[2].as_i64().unwrap_or(0) as i32]);
        eprintln!("chunk lists at the first tick: {} ticking, {} spawning, {} entity ticking, {} loaded, spawn chunk count {}, respawn {:?}", g.ticking.len(), g.spawning.len(), g.entity_ticking.len(), g.full.len(), g.spawn_chunk_count, g.respawn);
        if std::env::var("MCTAS_EXACT_LISTS").is_ok() {
            eprintln!("entity ticking: {:?}", g.entity_ticking);
            eprintln!("ticking: {:?}", g.ticking);
        }
        Ok(g)
    }

    /// The judge's placement: raw writes in order, each wire's shape, a `self` update of each.
    fn place(&mut self, blocks: &[([i32; 3], String)]) -> Result<()> {
        self.place_with(blocks, &[])
    }

    /// The judge's placement with the design's block-entity data (`nbt:` lines) merged after the
    /// raw writes and before the updates, like `Run::start` (`apply_block_entities`).
    fn place_with(&mut self, blocks: &[([i32; 3], String)], bes: &[([i32; 3], String)]) -> Result<()> {
        self.w.set_blocks(self.sb, blocks, PlaceMode::Raw)?;
        for (p, snbt) in bes {
            self.w.merge_block_entity(self.sb, *p, snbt)?;
        }
        let wires: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
        if !wires.is_empty() {
            self.w.update(self.sb, &wires, "shape")?;
        }
        let all: Vec<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
        self.w.update(self.sb, &all, "self")?;
        Ok(())
    }

    /// One bridge op on this farm's sandbox (its `"sandbox"` filled in): the terrain lane's probes
    /// (`spawnrules`).
    pub fn op(&mut self, mut v: Value) -> Result<Value> {
        v["sandbox"] = json!(self.sb);
        self.w.call(v)
    }

    /// `ticks` traced ticks (with the draw log when `draws`).
    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<Frame>> {
        let v = self.w.call(json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}}))?;
        v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.iter().map(parse_frame).collect()
    }

    /// The game's own golem spawn cells now, for golems summoned from each of `starts` (villagers'
    /// cells): the bridge's `golem_spots` (the game's `moveToPossibleSpawnPosition` scan of
    /// `trySpawnMob`'s 17 x 17 columns, then `checkSpawnRules` and `IronGolem.checkSpawnObstruction`
    /// with the level's entities in place; nothing drawn from a random, nothing added).
    pub fn golem_spots(&mut self, starts: &[[i32; 3]]) -> Result<Vec<[i32; 3]>> {
        let v = self.w.call(json!({"op": "golem_spots", "sandbox": self.sb, "starts": starts}))?;
        v["cells"]
            .as_array()
            .ok_or_else(|| anyhow!("golem_spots: no cells"))?
            .iter()
            .map(|c| {
                let a = c.as_array().ok_or_else(|| anyhow!("golem_spots: a cell is not [x,y,z]"))?;
                let n = |k: usize| a.get(k).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| anyhow!("golem_spots: a bad cell"));
                Ok([n(0)?, n(1)?, n(2)?])
            })
            .collect()
    }

    /// `Run::arrive`'s game side, after redsim's: the detectors, the entities (with redsim's
    /// summon order and the run's follow-range bonuses), each villager's HOME and LAST_SLEPT from
    /// redsim's world.
    pub fn arrive(&mut self, _c: &circuit::Circuit, spec: &FarmSpec, ours: &redsim::world::World, bonus: &[f64]) -> Result<()> {
        let detectors: Vec<([i32; 3], String)> = spec
            .blocks
            .iter()
            .filter(|(_, b)| matches!(b, redsim::block::Block::DaylightDetector { .. }))
            .map(|(p, _)| (pos(*p), self.text.get(&pos(*p)).cloned().unwrap_or_default()))
            .collect();
        if !detectors.is_empty() {
            self.place(&detectors)?;
        }
        let mut k = 0;
        let mut specs = Vec::new();
        for (ty, p, nbt) in &spec.entities {
            // the tree's mobs (the bridge gives `follow_bonus` to its mobs only, in tree order): the
            // root if it is a mob (a boat or a minecart is not), then each passenger `id:` that is
            // (mob-AI lane: counting the boat shifted every later mob's bonus by one)
            let is_mob = |id: &str| redsim::entity::mob::types::MobType::from_id(id.strip_prefix("minecraft:").unwrap_or(id)).is_some();
            let passengers = nbt.as_deref().map(|n| n.split("id:").skip(1).filter(|s| is_mob(s.trim_start_matches('"').split(['"', ',', '}']).next().unwrap_or(""))).count()).unwrap_or(0);
            let mobs_here = usize::from(is_mob(ty)) + passengers;
            let mut b = Vec::new();
            for _ in 0..mobs_here {
                b.push(bonus.get(k).copied());
                k += 1;
            }
            specs.push(json!({"type": ty, "pos": p, "nbt": nbt, "follow_bonus": b}));
        }
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        let game_ids: Vec<u64> = v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
        let first_mob = ours.entities.list.len().saturating_sub(game_ids.len());
        let ours_ids: Vec<u64> = ours.entities.list[first_mob..].iter().map(|e| e.id as u64).collect();
        if game_ids != ours_ids {
            eprintln!("note: the design's entities entered the game as {game_ids:?}, redsim as {ours_ids:?} (the worlds already differ)");
        }
        // (tickexact 2026-11-09) the HOMEs the entities loaded from their own data (a design's
        // `port home`): the game loaded them too, with no POI ticket taken
        let loaded: Vec<String> = specs.iter().filter_map(|s| s["nbt"].as_str()).filter(|n| n.contains("\"minecraft:home\"")).map(|n| n.to_string()).collect();
        let loaded_home = |h: redsim::BlockPos| loaded.iter().any(|n| n.contains(&format!("pos:[I;{},{},{}]", h.x, h.y, h.z)));
        for (e, gid) in ours.entities.list[first_mob..].iter().zip(game_ids.iter()) {
            if let EntityKind::Mob(m) = &e.kind
                && let Some(b) = m.brain.as_deref()
            {
                let mut mem = serde_json::Map::new();
                let mut take = false;
                if let Some(h) = b.home
                    && !loaded_home(h)
                {
                    mem.insert("home".into(), json!([h.x, h.y, h.z]));
                    take = true;
                }
                if let Some(t) = b.last_slept {
                    mem.insert("last_slept".into(), json!(t));
                }
                self.w.call(json!({"op": "memory", "sandbox": self.sb, "entity": gid, "memories": mem, "take_poi": take}))?;
            }
        }
        Ok(())
    }
}

/// A bridge frame (`trace` op) as a `redsim::exact::Frame`, its block states in redsim's text.
pub fn parse_frame(v: &Value) -> Result<Frame> {
    let i = |k: &str| v[k].as_i64();
    let mut f = Frame { t: i("t").ok_or_else(|| anyhow!("frame without t"))?, day: i("day"), rng: i("rng"), rv: i("rv").map(|x| x as i32), weather: v["weather"].as_str().map(str::to_string), ..Default::default() };
    for e in v["entities"].as_array().into_iter().flatten() {
        f.entities.push(parse_entity(e)?);
    }
    let ids = |k: &str| -> Vec<u32> { v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default() };
    f.entered = ids("entered");
    f.left = ids("left");
    for b in v["blocks"].as_array().into_iter().flatten() {
        let p = [b[0].as_i64().unwrap_or(0) as i32, b[1].as_i64().unwrap_or(0) as i32, b[2].as_i64().unwrap_or(0) as i32];
        let s = b[3].as_str().unwrap_or("");
        let text = redsim::block::Block::parse(s).map(|x| x.to_string()).unwrap_or_else(|_| s.to_string());
        f.blocks.push((p, text));
    }
    for d in v["draws"].as_array().into_iter().flatten() {
        f.draws.push((d[0].as_str().unwrap_or("").to_string(), d[1].as_str().unwrap_or("").to_string(), d[2].as_u64().unwrap_or(0) as u32));
        f.draw_values.push(d[3].as_array().map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect()).unwrap_or_default());
    }
    f.spawning_shuffled = v["spawning_shuffled"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
    f.containers = v["containers"]
        .as_array()
        .map(|a| a.iter().map(|c| ([c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32, c[2].as_i64().unwrap_or(0) as i32], c[3].as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();
    f.containers.sort();
    f.player = v["player"].as_array().map(|a| [a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0), a[2].as_f64().unwrap_or(0.0)]);
    Ok(f)
}

fn parse_entity(e: &Value) -> Result<EntityFrame> {
    let v3 = |k: &str| -> [f64; 3] { [e[k][0].as_f64().unwrap_or(f64::NAN), e[k][1].as_f64().unwrap_or(f64::NAN), e[k][2].as_f64().unwrap_or(f64::NAN)] };
    let f32s = |k: &str| e[k].as_str().and_then(|s| s.parse::<f32>().ok());
    let s = |k: &str| e[k].as_str().map(str::to_string);
    let i = |k: &str| e[k].as_i64().map(|x| x as i32);
    let memories = e["memories"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect());
    Ok(EntityFrame {
        n: e["n"].as_u64().ok_or_else(|| anyhow!("entity without n"))? as u32,
        ty: e["type"].as_str().unwrap_or("?").to_string(),
        pos: v3("pos"),
        motion: v3("motion"),
        yaw: f32s("yaw").unwrap_or(f32::NAN),
        pitch: f32s("pitch").unwrap_or(f32::NAN),
        on_ground: e["on_ground"].as_bool().unwrap_or(false),
        fire: i("fire"),
        air: i("air"),
        pose: s("pose"),
        vehicle: s("vehicle"),
        random: e["random"].as_i64(),
        item: s("item"),
        age: i("age"),
        head: f32s("head"),
        body: f32s("body"),
        health: f32s("health"),
        hurt_time: i("hurt_time"),
        death_time: i("death_time"),
        no_action: i("no_action"),
        memories,
        modeled_memories: None,
        all_memories: false,
        activities: s("activities"),
        behaviors: s("behaviors"),
        goals: s("goals"),
        path: s("path"),
        target: s("target"),
    })
}

/// How high the exact compare lifts a void design (both sides): the gate's AFK spot, 60 blocks
/// under the farm, must lie inside the world (y -64..320).
pub const LIFT: i32 = 100;

/// A design, lifted by [`LIFT`].
pub fn load_lifted(design: &std::path::Path) -> Result<circuit::Circuit> {
    circuit::Circuit::load(design)?.translated([0, LIFT, 0])
}

/// The judge's run options for the exact compare: the steady state of `seed`, golems standing
/// still (the walking policies are the judge's robustness input, not the game's AI).
pub fn run_opts(seed: i64, ticks: u32) -> RunOpts {
    redsim::entity::mob::farm::judge_opts(seed, ticks, redsim::entity::mob::farm::WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still))
}

/// `Run::arrive` with the compare's natural spawner kept: a terrain site's arrival makes a fresh
/// spawner (`Run::arrive`: the judge spawns naturally from its arrival on), while the bridge's world
/// has spawned since its first settle tick with the game's chunk lists, which redsim's keeps.
pub fn arrive(run: &mut Run) -> Result<()> {
    while run.settle_tick() {}
    let natural = run.world.mob.natural.take();
    run.arrive().map_err(|e| anyhow!("{e}"))?;
    if natural.is_some() {
        run.world.mob.natural = natural;
    }
    Ok(())
}

/// redsim's side: the runner with the exact streams on (the level random mirrored on the
/// sandbox's forced box, random ticks at speed 3 from the bridge's randValue), its chunks ticked in
/// the game's order (the forced box and the AFK player's square, from the bridge), natural
/// spawning from the game's lists (the spawning chunks before the shuffle, the spawn chunk count,
/// the entity-ticking chunks): in the void the gate's (monsters off, plains), in terrain the site's
/// (monsters on); none in the bare scenario.
pub fn redsim_start(spec: &FarmSpec, o: &RunOpts, game: &GameFarm, bare: bool) -> Result<Run> {
    use redsim::entity::mob::natural::{Difficulty, Natural};
    let mut run = Run::start(spec, o).map_err(|e| anyhow!(e))?;
    match o.site {
        None => run.world.mirror_level_random(o.seed, forced_chunks(spec)),
        Some(_) => run.world.mirror_level_random(o.seed, game_chunks(game)),
    }
    run.world.random_ticks = Some(redsim::crops::RandomTicks::new(random_tick_speed().unwrap_or(3), o.seed));
    // the mob AI (the mob-AI lane): goals and brains (every mob seeded from w.exact_seed, which
    // mirror_level_random set); the game JVM's POI holder identity hashes (AcquirePoi's retry
    // markers' order)
    run.world.mob.ai.on = true;
    run.world.mob.ai.poi_hashes = Some(game.poi_hashes.clone());
    if !game.ticking.is_empty() {
        run.world.chunk_order = game.ticking.clone();
    }
    if bare {
        run.world.afk = None;
        run.world.mob.natural = None;
        return Ok(run);
    }
    // the void gate's stone over its AFK player (`GameFarm::start`), outside redsim's box
    if o.site.is_none() {
        let bot = afk_spot(spec);
        run.world.outside.push((redsim::BlockPos::new(bot[0], bot[1] + 3, bot[2]), redsim::Block::parse("minecraft:stone").map_err(|e| anyhow!(e))?));
    }
    // the bridge's AFK player is a real creative player: the brains sense it (NEAREST_PLAYERS)
    if let Some(a) = run.world.afk {
        run.world.mob.ai.players.push(redsim::entity::mob::ai::Player { pos: a, mode: redsim::entity::mob::ai::GameMode::Creative, invulnerable: false });
    }
    let base = match o.site {
        // a terrain site: monsters on around its AFK spot, the level's own biome outside the
        // sample (the bridge's terrain sandbox: plains), from the first settle tick like the
        // bridge's world (`Run::start` leaves a site's spawner to `Run::arrive`: see `arrive`)
        Some(s) => Natural { void_biome: Some("minecraft:plains"), ..Natural::new(vec![s.afk], Difficulty::Hard) },
        None => Natural { spawn_enemies: false, void_biome: Some("minecraft:plains"), ..Natural::new(vec![], Difficulty::Hard) },
    };
    let _ = run.world.mob.natural.take();
    let spawn_enemies = base.spawn_enemies;
    run.world.mob.natural = Some(Natural {
        spawn_order: game.spawning.clone(),
        spawn_candidates: game.spawn_candidates.clone(),
        spawn_chunk_count: Some(game.spawn_chunk_count as i32),
        entity_ticking: game.entity_ticking.clone(),
        // `--no-spawning`: the spawner off, its chunk shuffle still drawn (as the game's
        // tickChunks shuffles with spawn_mobs off)
        spawning: game.natural,
        // (the server's respawn position is the overworld's: a sandbox's own dimension skips
        // `isRightDistanceToPlayerAndSpawnPoint`'s 24-block check, `respawnData.dimension() ==
        // level.dimension()`, so the void needs none; a terrain site keeps its sample's spawn)
        ..base
    });
    if game.spawners {
        // the void gate's rules: no phantoms, patrols or traders; terrain: the game's defaults
        let all = o.site.is_some();
        let mut c = redsim::entity::mob::custom::Custom::new(o.seed, spawn_enemies, all, all, all);
        c.full_chunks = game.full.iter().copied().collect();
        if all {
            c.sea_level = 63;
        }
        run.world.mob.custom = Some(c);
    }
    Ok(run)
}

/// The box of the sandbox's ticking chunks (a terrain sandbox: the sample's).
fn game_chunks(game: &GameFarm) -> [i32; 4] {
    let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for [x, z] in &game.ticking {
        b = [b[0].min(*x), b[1].min(*z), b[2].max(*x), b[3].max(*z)];
    }
    b
}

/// Compare a farm design over `opts.ticks` game ticks after the settle; the compare's result.
pub fn compare(version: &str, design: &std::path::Path, opts: &Opts, draws_at: Option<i64>) -> Result<Compare> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start_with(version, &c, &spec, &o, weather_on(opts), draws_at.is_some() || opts.draws_at.is_some(), place, !opts.no_spawning)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    let mut watch = Watch::new(&run.world, spec.lo, spec.hi);
    let mut cmp = Compare { only: opts.only.clone(), ..Compare::default() };
    let window = |t0: i64, n: u32| [draws_at, opts.draws_at].iter().flatten().any(|d| *d >= t0 && *d < t0 + n as i64 + 3);
    let spawn_order = game.spawning.clone();
    let print = |f: &Frame| {
        if opts.draws_at == Some(f.t) {
            if !f.spawning_shuffled.is_empty() {
                println!("the spawning chunks in tickChunks' order at game time {} ({}): {}", f.t, f.spawning_shuffled.len(), f.spawning_shuffled.iter().map(|[x, z]| format!("{x},{z}")).collect::<Vec<_>>().join(" "));
                // the list before Util.shuffle (its swaps undone from the shuffle's draws) against
                // the one redsim keeps from the first tick
                if let Some(k) = f.draws.iter().position(|(_, site, _)| site.starts_with("ServerChunkCache.tickChunks:"))
                    && let Some(vals) = f.draw_values.get(k)
                {
                    let mut list = f.spawning_shuffled.clone();
                    let n = list.len();
                    if vals.len() + 1 == n {
                        let js: Vec<usize> = (2..=n).rev().zip(vals.iter()).map(|(i, v)| if i.is_power_of_two() { ((i as i64 * *v as i64) >> 31) as usize } else { (*v as usize) % i }).collect();
                        for (i, j) in (2..=n).rev().zip(js.iter()).collect::<Vec<_>>().into_iter().rev() {
                            list.swap(i - 1, *j);
                        }
                        match list.iter().zip(spawn_order.iter()).position(|(a, b)| a != b) {
                            None if list.len() == spawn_order.len() => println!("the list before the shuffle: the same as redsim's (the bridge's at the first tick)"),
                            at => println!("the list before the shuffle differs from redsim's at index {:?}: game {:?} | redsim {:?}", at, at.and_then(|i| list.get(i)), at.and_then(|i| spawn_order.get(i))),
                        }
                    }
                }
            }
            println!("the game's draws at game time {} (stream, call site, count, the raw next(bits) outputs; in draw order):", f.t);
            for (k, (s, site, n)) in f.draws.iter().enumerate() {
                let vals = f.draw_values.get(k).map(|v| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                println!("  {s} {site} x{n} [{vals}]");
            }
        }
    };
    let mut t: i64 = 0;
    let mut redsim_time = std::time::Duration::ZERO;
    let mut tick_time = std::time::Duration::ZERO;
    let mut changes = [0usize; 2];
    // the drops each side burnt (day-1's ask: the official server cannot see an item's death): an
    // item that left while burning (its last frame's fire above 0), counted by item, both sides
    let mut burnt: [std::collections::BTreeMap<String, u32>; 2] = Default::default();
    let mut last_items: [std::collections::BTreeMap<u32, (String, i32)>; 2] = Default::default();
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, window(t, n))? {
            print(&f);
            trace_start(&mut run.world, f.t);
            let r0 = std::time::Instant::now();
            run.settle_tick();
            redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
            tick_time += r0.elapsed();
            let ours = watch.frame(&run.world);
            redsim_time += r0.elapsed();
            print_probed(&f, &ours);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() && !opts.full && opts.draws_at.is_none_or(|d| d < t) {
            return Ok(cmp);
        }
    }
    let skip = mob_count(&run.world);
    arrive(&mut run)?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses_after(&mut run.world, &bonus, skip);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut tally = Tally::default();
    let mut left = opts.ticks;
    // (a probe: MCTAS_EXACT_MOBSTATE=T: right after the run's tick T, the game's mobs with a path,
    // each node (the bridge's `mobstate`), to set beside MCTAS_PATH_DUMP's redsim paths; the batch
    // is cut to end there)
    let mobstate_at: Option<i64> = std::env::var("MCTAS_EXACT_MOBSTATE").ok().and_then(|v| v.parse().ok());
    while left > 0 {
        let mut n = left.min(opts.batch);
        if let Some(ms) = mobstate_at
            && t < ms
            && t + n as i64 > ms
        {
            n = (ms - t) as u32;
        }
        for f in game.trace(n, window(t, n))? {
            print(&f);
            trace_start(&mut run.world, f.t);
            let r0 = std::time::Instant::now();
            run.tick();
            redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
            tick_time += r0.elapsed();
            let ours = watch.frame(&run.world);
            redsim_time += r0.elapsed();
            changes[0] += f.containers.len();
            changes[1] += ours.containers.len();
            for (side, frame) in [&f, &ours].into_iter().enumerate() {
                for n in &frame.left {
                    if let Some((item, fire)) = last_items[side].remove(n)
                        && fire > 0
                    {
                        let (id, count) = item.rsplit_once(' ').map(|(a, b)| (a.to_string(), b.parse::<u32>().unwrap_or(1))).unwrap_or((item.clone(), 1));
                        *burnt[side].entry(id).or_insert(0) += count;
                    }
                }
                for e in &frame.entities {
                    if let (Some(item), Some(fire)) = (&e.item, e.fire) {
                        last_items[side].insert(e.n, (item.clone(), fire));
                    }
                }
            }
            // (a probe: MCTAS_EXACT_ENTITY="N:T0:T1" prints entity #N's frame on both sides at each
            // game time T0..=T1)
            print_probed(&f, &ours);
            tally.add(0, f.t, &f.entities);
            tally.add(1, f.t, &ours.entities);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if mobstate_at == Some(t) {
            let v = game.w.call(json!({"op": "mobstate", "sandbox": game.sb}))?;
            for m in v["mobs"].as_array().into_iter().flatten() {
                if let Some(p) = m["path"].as_str() {
                    eprintln!("mobstate t{t}: {} at {} {} {}: path {p}", m["type"].as_str().unwrap_or("?"), m["x"].as_str().unwrap_or("?"), m["y"].as_str().unwrap_or("?"), m["z"].as_str().unwrap_or("?"));
                }
            }
            // MCTAS_EXACT_PATHNODES="N:X,Y,Z:REACH": entity #N's search toward the cell again, every
            // node the game's evaluator made (the bridge's `path_nodes`)
            if let Ok(spec) = std::env::var("MCTAS_EXACT_PATHNODES") {
                let parts: Vec<&str> = spec.split(':').collect();
                if let [n, xyz, reach] = parts[..] {
                    let c: Vec<i64> = xyz.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    let v = game.w.call(json!({"op": "path_nodes", "sandbox": game.sb, "entity": n.parse::<i64>().unwrap_or(0), "target": c, "reach": reach.parse::<i64>().unwrap_or(0)}))?;
                    eprintln!("game path t{t}: {} (max {})", v["path"].as_str().unwrap_or("none"), v["max_path_length"].as_str().unwrap_or("?"));
                    for node in v["nodes"].as_array().into_iter().flatten() {
                        eprintln!("game node t{t}: {}", node.as_str().unwrap_or(""));
                    }
                }
            }
        }
        if cmp.first.is_some() && !opts.full && opts.draws_at.is_none_or(|d| d < t) {
            break;
        }
    }
    eprintln!("redsim's side: {:.2} s for {} ticks ({:.1} us a tick: {:.1} us ticking, the rest its frames)", redsim_time.as_secs_f64(), t, redsim_time.as_secs_f64() * 1e6 / t.max(1) as f64, tick_time.as_secs_f64() * 1e6 / t.max(1) as f64);
    eprintln!("container changes after the arrival: the game {}, redsim {}", changes[0], changes[1]);
    tally.print();
    let burnt_text = |m: &std::collections::BTreeMap<String, u32>| if m.is_empty() { "none".to_string() } else { m.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ") };
    println!("drops burnt: the game {} | redsim {}", burnt_text(&burnt[0]), burnt_text(&burnt[1]));
    // (a probe: MCTAS_EXACT_SECTIONS=1: at the end, the randomly ticking sections of the ticking
    // chunks, the game's (`LevelChunkSection.isRandomlyTicking`) against redsim's TickingSections,
    // each section one side ticks and the other does not, with its randomly ticking cells on
    // redsim's side)
    if std::env::var_os("MCTAS_EXACT_SECTIONS").is_some() {
        let v = game.w.call(json!({"op": "ticking_sections", "sandbox": game.sb}))?;
        let theirs: std::collections::BTreeSet<[i32; 3]> = v["sections"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| Some([s[0].as_i64()? as i32, s[1].as_i64()? as i32, s[2].as_i64()? as i32])).collect())
            .unwrap_or_default();
        let w = &run.world;
        let mut ours = std::collections::BTreeSet::new();
        if let Some(ts) = w.ticking_sections.as_ref() {
            for [cx, cz] in &w.chunk_order {
                for sy in -4..=19 {
                    if ts.ticking(*cx, sy, *cz) {
                        ours.insert([*cx, sy, *cz]);
                    }
                }
            }
        }
        for s in theirs.difference(&ours) {
            eprintln!("ticking section [x, sy, z] {s:?}: the game ticks it, redsim does not");
        }
        for s in ours.difference(&theirs) {
            eprintln!("ticking section [x, sy, z] {s:?}: redsim ticks it, the game does not");
            let (lo, hi) = (redsim::BlockPos::new(s[0] * 16, s[1] * 16, s[2] * 16), redsim::BlockPos::new(s[0] * 16 + 15, s[1] * 16 + 15, s[2] * 16 + 15));
            let mut n = 0;
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        let b = w.get(redsim::BlockPos::new(x, y, z));
                        if redsim::crops::ticks_in_section(&b) && n < 4 {
                            n += 1;
                            eprintln!("  {x} {y} {z}: {b}");
                        }
                    }
                }
            }
        }
        eprintln!("ticking sections: the game {}, redsim {} (the ticking chunks'); redsim's box {} size {:?}, {} cells outside it held", theirs.len(), ours.len(), w.min, w.size, w.outer.len());
        eprintln!("redsim's world: land {}, {} errors{}", w.land.is_some(), w.errors.len(), w.errors.first().map(|e| format!(", the first: {e}")).unwrap_or_default());
    }
    // (a probe: MCTAS_EXACT_SPAWNSTATE=1: at the end, the last spawn tick's mob counts per category,
    // the game's (SpawnState.getMobCategoryCounts) against redsim's, and redsim's per player)
    if std::env::var_os("MCTAS_EXACT_SPAWNSTATE").is_some() {
        let v = game.w.call(json!({"op": "spawn_state", "sandbox": game.sb}))?;
        if let Some(n) = run.world.mob.natural.as_ref() {
            for c in redsim::entity::mob::natural::Category::SPAWNING {
                let theirs = v["counts"][c.name()].as_i64().unwrap_or(0);
                let ours = n.last_counts[c as usize];
                eprintln!("spawn state {}: the game {theirs} | redsim {ours}{}; redsim per player {:?}", c.name(), if theirs != ours as i64 { "  <- differs" } else { "" }, n.last_local.iter().map(|l| l[c as usize]).collect::<Vec<_>>());
            }
        }
        eprintln!("the game's mobs by category and type: {}", v["by_type"]);
        let mut ours: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        for e in &run.world.entities.list {
            if let redsim::entity::EntityKind::Mob(m) = &e.kind {
                let k = format!("{} {}{}{}", redsim::entity::mob::natural::category_of(m.ty).name(), m.ty.def().game_id, if m.persistence_required || e.vehicle.is_some() { " (persistent)" } else { "" }, if e.removed { " (removed)" } else { "" });
                *ours.entry(k).or_default() += 1;
            }
        }
        eprintln!("redsim's mobs by category and type: {ours:?}");
    }
    // (a probe: MCTAS_EXACT_CHUNKS=1: at the end, the game's spawning chunks (collectSpawningChunks'
    // list, before its shuffle) against redsim's (its candidates within 128 of a player))
    if std::env::var_os("MCTAS_EXACT_CHUNKS").is_some() {
        let v = game.w.call(json!({"op": "chunk_lists", "sandbox": game.sb}))?;
        let list = |k: &str| -> Vec<[i32; 2]> { v[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
        let theirs: std::collections::BTreeSet<[i32; 2]> = list("spawning_chunks").into_iter().collect();
        let ours: std::collections::BTreeSet<[i32; 2]> = run.world.mob.natural.as_ref().map(|n| n.spawn_candidates.iter().copied().filter(|c| n.players.iter().any(|q| { let (x, z) = ((c[0] * 16 + 8) as f64, (c[1] * 16 + 8) as f64); (x - q.x) * (x - q.x) + (z - q.z) * (z - q.z) < 16384.0 })).collect()).unwrap_or_default();
        eprintln!("spawning chunks: the game {}, redsim {}; the game's only {:?}; redsim's only {:?}", theirs.len(), ours.len(), theirs.difference(&ours).collect::<Vec<_>>(), ours.difference(&theirs).collect::<Vec<_>>());
        // and their order (the shuffle's input)
        let theirs_order = list("spawning_chunks");
        let ours_order: Vec<[i32; 2]> = run.world.mob.natural.as_ref().map(|n| n.spawn_candidates.iter().copied().filter(|c| n.players.iter().any(|q| { let (x, z) = ((c[0] * 16 + 8) as f64, (c[1] * 16 + 8) as f64); (x - q.x) * (x - q.x) + (z - q.z) * (z - q.z) < 16384.0 })).collect()).unwrap_or_default();
        if let Some(i) = theirs_order.iter().zip(&ours_order).position(|(a, b)| a != b) {
            eprintln!("spawning chunk order: first differs at {i}: the game {:?} | redsim {:?}", &theirs_order[i..(i + 4).min(theirs_order.len())], &ours_order[i..(i + 4).min(ours_order.len())]);
        } else {
            eprintln!("spawning chunk order: the same");
        }
    }
    // (a probe: MCTAS_EXACT_BLOCKS="x0,y0,z0,x1,y1,z1": each cell of the box whose block differs at
    // the end, the game's against redsim's; with `=all:` in front every cell is printed)
    if let Ok(b) = std::env::var("MCTAS_EXACT_BLOCKS") {
        let (all, b) = match b.strip_prefix("all:") {
            Some(rest) => (true, rest.to_string()),
            None => (false, b),
        };
        let v: Vec<i32> = b.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() == 6 {
            let (a, c) = ([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])], [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])]);
            if let Ok(region) = game.w.get_region(game.sb, a, c) {
                let mut differ = 0;
                for y in a[1]..=c[1] {
                    for z in a[2]..=c[2] {
                        for x in a[0]..=c[0] {
                            let gs = region.get([x, y, z]).map(|s| s.to_string()).unwrap_or_default();
                            let rs = run.world.get(redsim::BlockPos::new(x, y, z)).to_string();
                            if gs != rs || all {
                                differ += (gs != rs) as usize;
                                eprintln!("block {x} {y} {z}: game {gs} | redsim {rs}");
                            }
                        }
                    }
                }
                eprintln!("block box {a:?}..{c:?}: {differ} cells differ");
            }
        }
    }
    // (a probe: MCTAS_TRACE_TICKS_AT=T: at the end, each random-tick position redsim picked at
    // game time T (and the cell above) whose block or light differs between the game and redsim)
    let traced: Vec<[i32; 3]> = redsim::crops::TRACED_TICKS.lock().unwrap().clone();
    if !traced.is_empty() {
        use base64::Engine as _;
        let mut differ = 0;
        for p in &traced {
            let (from, to) = (*p, [p[0], p[1] + 1, p[2]]);
            let region = game.w.get_region(game.sb, from, to).ok();
            let r = game.w.call(json!({"op": "light_box", "sandbox": game.sb, "from": from, "to": to}))?;
            let dec = |k: &str| base64::engine::general_purpose::STANDARD.decode(r[k].as_str().unwrap_or("")).unwrap_or_default();
            let (sky, block) = (dec("sky"), dec("block"));
            for (i, y) in [p[1], p[1] + 1].into_iter().enumerate() {
                let q = redsim::BlockPos::new(p[0], y, p[2]);
                let gs = region.as_ref().and_then(|g| g.get([p[0], y, p[2]])).map(|s| s.to_string()).unwrap_or_default();
                let rs = run.world.get(q).to_string();
                let gl = (sky.get(i).copied().unwrap_or(0) as i32, block.get(i).copied().unwrap_or(0) as i32);
                let rl = (run.world.sky_light(q).unwrap_or(-1), run.world.block_light(q).unwrap_or(-1));
                if gs != rs || gl != rl {
                    differ += 1;
                    eprintln!("traced tick {} {} {}{}: the game {gs} light {gl:?} | redsim {rs} light {rl:?}", p[0], p[1], p[2], if i == 1 { " (above)" } else { "" });
                }
            }
        }
        eprintln!("traced random ticks: {} positions, {differ} cells differ", traced.len());
    }
    // (a probe: MCTAS_EXACT_LIGHT="x0,y0,z0,x1,y1,z1": each cell of the box whose sky or block light
    // differs at the end, the game's against redsim's)
    if let Ok(b) = std::env::var("MCTAS_EXACT_LIGHT") {
        let v: Vec<i32> = b.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if v.len() == 6 {
            let (from, to) = ([v[0], v[1], v[2]], [v[3], v[4], v[5]]);
            let r = game.w.call(json!({"op": "light_box", "sandbox": game.sb, "from": from, "to": to}))?;
            use base64::Engine as _;
            let dec = |k: &str| base64::engine::general_purpose::STANDARD.decode(r[k].as_str().unwrap_or("")).unwrap_or_default();
            let (sky, block) = (dec("sky"), dec("block"));
            let mut i = 0;
            let mut differ = 0;
            for y in from[1]..=to[1] {
                for z in from[2]..=to[2] {
                    for x in from[0]..=to[0] {
                        let p = redsim::BlockPos::new(x, y, z);
                        let (gs, gb) = (sky.get(i).copied().unwrap_or(0) as i32, block.get(i).copied().unwrap_or(0) as i32);
                        let (rs, rb) = (run.world.sky_light(p).unwrap_or(-1), run.world.block_light(p).unwrap_or(-1));
                        if (gs, gb) != (rs, rb) {
                            differ += 1;
                            eprintln!("light {x} {y} {z} ({}): the game's sky {gs} block {gb} | redsim's sky {rs} block {rb}", run.world.get(p));
                        }
                        i += 1;
                    }
                }
            }
            eprintln!("light box {from:?}..{to:?}: {differ} cells differ");
        }
    }
    // (a probe: MCTAS_EXACT_HEIGHTS="x,z;x,z": each column's WORLD_SURFACE height at the end on
    // both sides, the game's highest taken y and redsim's first free y - 1)
    if let Ok(cols) = std::env::var("MCTAS_EXACT_HEIGHTS") {
        let cols: Vec<[i32; 2]> = cols.split(';').filter_map(|s| s.split_once(',')).filter_map(|(x, z)| Some([x.trim().parse().ok()?, z.trim().parse().ok()?])).collect();
        let v = game.w.call(json!({"op": "heights", "sandbox": game.sb, "columns": cols}))?;
        for (c, g) in cols.iter().zip(v["heights"].as_array().cloned().unwrap_or_default()) {
            let ours = run.world.height(redsim::terrain::Heightmap::WorldSurface, c[0], c[1]) - 1;
            eprintln!("column {} {}: the game's WORLD_SURFACE {} | redsim's {ours}", c[0], c[1], g[2]);
            // the column's top blocks on both sides
            let top = ours.max(g[2].as_i64().unwrap_or(0) as i32);
            let (a, b) = ([c[0], top - 3, c[1]], [c[0], top + 1, c[1]]);
            if let Ok(region) = game.w.get_region(game.sb, a, b) {
                for y in (a[1]..=b[1]).rev() {
                    let gs = region.get([c[0], y, c[1]]).map(|s| s.to_string()).unwrap_or_default();
                    let rs = run.world.get(redsim::BlockPos::new(c[0], y, c[1])).to_string();
                    eprintln!("  y {y}: game {gs} | redsim {rs}");
                }
            }
        }
    }
    Ok(cmp)
}

/// What [`spots_check`] found.
#[derive(Clone, Debug, Default)]
pub struct SpotsCheck {
    /// Times the game's cells were compared with the view's (as the mobs arrive).
    pub looks: u32,
    /// The cells the view drew.
    pub most: usize,
    /// Golems the game spawned in its run, each checked against the view's cells.
    pub spawns: u32,
    /// Game ticks the game ran after the arrival.
    pub ticks: i64,
    /// Every difference: `x y z drawn, the game finds no golem spawn there`, `x y z the game's
    /// golem spawn cell, not drawn`, `the game spawned a golem at x y z, not drawn`.
    pub differences: Vec<String>,
}

/// A view's golem spawn cells against the GAME's (mobviewer 2026-11-08, vjeux: "why is the
/// spawning ui not respecting the game mechanics?"): the farm built in the game and in redsim the
/// same way and settled tick for tick ([`compare`]'s setup); as the mobs arrive (both worlds the
/// same, block for block and entity for entity), the game's own cells from the villagers the view
/// summons from ([`GameFarm::golem_spots`]: the game's scan and `checkSpawnObstruction`, the farm's
/// entities in place) against the view's; then the game runs on alone for `opts.ticks` ticks
/// (redsim's randoms do not follow it that far yet: track tickexact) and every golem it spawns must
/// stand on a cell the view drew. `view(world, spec)`: its summoning villagers' cells and the cells
/// it draws, read on redsim's world (`spec`: the farm as the run built it, moved to its site or
/// lifted).
pub fn spots_check(version: &str, design: &std::path::Path, opts: &Opts, mut view: impl FnMut(&redsim::world::World, &FarmSpec) -> (Vec<[i32; 3]>, std::collections::HashSet<[i32; 3]>)) -> Result<SpotsCheck> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start(version, &c, &spec, &o, opts.weather, false, place)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    let mut watch = Watch::new(&run.world, spec.lo, spec.hi);
    let mut cmp = Compare::default();
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, false)? {
            run.settle_tick();
            cmp.tick(&f, &watch.frame(&run.world));
        }
        left -= n;
    }
    // (the randoms may part during the settle, track tickexact; what the spawn cells read is the
    // blocks and the farm's entities as they arrive, each placed the same way on both sides: the
    // cells say whether they match)
    let parted = cmp.first.as_ref().map(|d| format!("{d:?}"));
    let skip = mob_count(&run.world);
    arrive(&mut run)?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses_after(&mut run.world, &bonus, skip);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut out = SpotsCheck::default();
    let fmt = |c: &[i32; 3]| format!("{} {} {}", c[0], c[1], c[2]);
    let (starts, drawn) = view(&run.world, &spec);
    out.most = drawn.len();
    let game_cells: std::collections::HashSet<[i32; 3]> = game.golem_spots(&starts)?.into_iter().collect();
    out.looks = 1;
    let why = parted.as_deref().map(|d| format!(" (the settle's randoms parted first: {d})")).unwrap_or_default();
    for c in drawn.difference(&game_cells) {
        out.differences.push(format!("as the mobs arrive: {} drawn, the game finds no golem spawn there{why}", fmt(c)));
    }
    for c in game_cells.difference(&drawn) {
        out.differences.push(format!("as the mobs arrive: {} the game's golem spawn cell, not drawn{why}", fmt(c)));
    }
    let mut t: i64 = 0;
    while t < opts.ticks as i64 {
        let n = (opts.ticks as i64 - t).min(opts.batch as i64) as u32;
        for f in game.trace(n, false)? {
            for k in &f.entered {
                if let Some(e) = f.entities.iter().find(|e| e.n == *k && e.ty.ends_with("iron_golem")) {
                    let cell = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
                    out.spawns += 1;
                    if !drawn.contains(&cell) {
                        out.differences.push(format!("game time {}: the game spawned a golem at {}, not drawn", f.t, fmt(&cell)));
                    }
                }
            }
        }
        t += n as i64;
    }
    out.ticks = t;
    Ok(out)
}

/// Give redsim's mobs the run's follow-range bonuses (in id order): `AttributeInstance`'s value,
/// base + base x bonus (`ADD_MULTIPLIED_BASE`).
pub fn set_bonuses(w: &mut redsim::world::World, bonus: &[f64]) {
    set_bonuses_after(w, bonus, 0);
}

/// [`set_bonuses`] for the farm's mobs only: the mobs after the first `skip` (a terrain run's
/// natural mobs, spawned before the arrival, keep their own finalizeSpawn bonus; the bridge gives
/// `follow_bonus` to the summoned mobs only, in order).
pub fn set_bonuses_after(w: &mut redsim::world::World, bonus: &[f64], skip: usize) {
    let mut k = 0;
    let mut seen = 0;
    for e in w.entities.list.iter_mut() {
        if let EntityKind::Mob(m) = &mut e.kind {
            seen += 1;
            if seen <= skip {
                continue;
            }
            let base = m.ty.def().follow_range;
            if let Some(b) = bonus.get(k) {
                m.follow_range = base + base * b;
            }
            k += 1;
        }
    }
}

/// The mobs a world holds (a terrain run's natural ones before the arrival).
pub fn mob_count(w: &redsim::world::World) -> usize {
    w.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count()
}

/// The bridge against itself: the same farm run twice in two workers, frame for frame (every
/// field the game reports), the determinism check of the bridge's seeding.
pub fn twin(version: &str, design: &std::path::Path, opts: &Opts, draws_at: Option<i64>) -> Result<Compare> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut a = GameFarm::start_with(version, &c, &spec, &o, weather_on(opts), draws_at.is_some(), place, !opts.no_spawning)?;
    let mut b = GameFarm::start_with(version, &c, &spec, &o, weather_on(opts), draws_at.is_some(), place, !opts.no_spawning)?;
    if a.ticking != b.ticking {
        println!("twin: the two runs tick different chunks: {} vs {} (first {:?} vs {:?})", a.ticking.len(), b.ticking.len(), a.ticking.first(), b.ticking.first());
    }
    let mut run = redsim_start(&spec, &o, &a, opts.bare)?;
    let mut cmp = Compare::default();
    let window = |t0: i64, n: u32| draws_at.is_some_and(|d| d >= t0 && d < t0 + n as i64 + 3);
    let mut t = 0i64;
    let show = |x: &Frame, y: &Frame| {
        if draws_at == Some(x.t) {
            let count = |f: &Frame| {
                let mut m: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
                for (s, site, n) in &f.draws {
                    *m.entry(format!("{s} {site}")).or_default() += n;
                }
                m
            };
            let (ca, cb) = (count(x), count(y));
            println!("twin draws at game time {}: call sites whose counts differ (run A, run B):", x.t);
            for k in ca.keys().chain(cb.keys()).collect::<std::collections::BTreeSet<_>>() {
                let (p, q) = (ca.get(k).copied().unwrap_or(0), cb.get(k).copied().unwrap_or(0));
                if p != q {
                    println!("  {k}: {p} vs {q}");
                }
            }
        }
    };
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, window(t, n))?, b.trace(n, window(t, n))?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            show(x, y);
            cmp.tick(x, &as_ours(y));
        }
        t += n as i64;
        left -= n;
    }
    while run.settle_tick() {}
    let skip = mob_count(&run.world);
    arrive(&mut run)?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses_after(&mut run.world, &bonus, skip);
    a.arrive(&c, &spec, &run.world, &bonus)?;
    b.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, window(t, n))?, b.trace(n, window(t, n))?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            show(x, y);
            cmp.tick(x, &as_ours(y));
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() {
            break;
        }
    }
    Ok(cmp)
}

/// A game frame on the compare's redsim side (the twin check): every memory counts as modeled.
fn as_ours(f: &Frame) -> Frame {
    let mut g = f.clone();
    for e in &mut g.entities {
        e.all_memories = true;
        e.ty = redsim::exact::normalize_type(&e.ty);
    }
    g
}
