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
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
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
}

fn pos(p: redsim::BlockPos) -> [i32; 3] {
    [p.x, p.y, p.z]
}

impl GameFarm {
    /// A fresh worker and world sandbox with the farm built and its chest placed (before the
    /// settle).
    pub fn start(version: &str, c: &circuit::Circuit, spec: &FarmSpec, o: &RunOpts, weather: bool, trace_randoms: bool) -> Result<GameFarm> {
        let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 2048, ..Default::default() })?;
        let sb = w.create(&SandboxOptions { chunks: chunk_box(spec), seed: o.seed })?;
        w.call(json!({
            "op": "world", "sandbox": sb,
            "advance_time": true, "day_time": o.day0 - SETTLE_TICKS,
            "advance_weather": weather, "spawn_mobs": false, "spawn_monsters": false,
            "trace_randoms": trace_randoms,
        }))
        .context("world sandbox")?;
        // the blocks in the spec's order (redsim's: sorted by x, y, z), the detectors held back
        let text: std::collections::HashMap<[i32; 3], String> = c.clock.design.block_list().into_iter().collect();
        let mut rest = Vec::new();
        for (p, b) in &spec.blocks {
            if matches!(b, redsim::block::Block::DaylightDetector { .. }) {
                continue;
            }
            let s = text.get(&pos(*p)).ok_or_else(|| anyhow!("no design block at {p}"))?;
            rest.push((pos(*p), s.clone()));
        }
        let mut g = GameFarm { w, sb, lo: pos(spec.lo), hi: pos(spec.hi) };
        g.place(&rest)?;
        let out = pos(spec.output);
        let here = g.w.get_region(sb, out, out)?;
        if !here.get(out).is_some_and(|s| s.starts_with("minecraft:chest")) {
            g.w.set_blocks(sb, &[(out, "minecraft:chest[facing=north,type=single,waterlogged=false]")], PlaceMode::Raw)?;
        }
        g.w.merge_block_entity(sb, out, "{Items:[]}")?;
        Ok(g)
    }

    /// The judge's placement: raw writes in order, each wire's shape, a `self` update of each.
    fn place(&mut self, blocks: &[([i32; 3], String)]) -> Result<()> {
        self.w.set_blocks(self.sb, blocks, PlaceMode::Raw)?;
        let wires: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
        if !wires.is_empty() {
            self.w.update(self.sb, &wires, "shape")?;
        }
        let all: Vec<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
        self.w.update(self.sb, &all, "self")?;
        Ok(())
    }

    /// `ticks` traced ticks (with the draw log when `draws`).
    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<Frame>> {
        let v = self.w.call(json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}}))?;
        v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.iter().map(parse_frame).collect()
    }

    /// `Run::arrive`'s game side, after redsim's: the detectors, the entities (with redsim's
    /// summon order and the run's follow-range bonuses), each villager's HOME and LAST_SLEPT from
    /// redsim's world.
    pub fn arrive(&mut self, c: &circuit::Circuit, spec: &FarmSpec, ours: &redsim::world::World, bonus: &[f64]) -> Result<()> {
        let text: std::collections::HashMap<[i32; 3], String> = c.clock.design.block_list().into_iter().collect();
        let detectors: Vec<([i32; 3], String)> = spec
            .blocks
            .iter()
            .filter(|(_, b)| matches!(b, redsim::block::Block::DaylightDetector { .. }))
            .map(|(p, _)| (pos(*p), text.get(&pos(*p)).cloned().unwrap_or_default()))
            .collect();
        if !detectors.is_empty() {
            self.place(&detectors)?;
        }
        let mut k = 0;
        let mut specs = Vec::new();
        for (ty, p, nbt) in &spec.entities {
            let mobs_here = 1 + nbt.as_deref().map(|n| n.matches("id:").count()).unwrap_or(0);
            let mut b = Vec::new();
            for _ in 0..mobs_here {
                b.push(bonus.get(k).copied());
                k += 1;
            }
            specs.push(json!({"type": ty, "pos": p, "nbt": nbt, "follow_bonus": b}));
        }
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        let game_ids: Vec<u64> = v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
        let ours_ids: Vec<u64> = ours.entities.list.iter().map(|e| e.id as u64).collect();
        if game_ids != ours_ids {
            bail!("the game's entities entered as {game_ids:?}, redsim's as {ours_ids:?}");
        }
        for e in &ours.entities.list {
            if let EntityKind::Mob(m) = &e.kind
                && let Some(b) = m.brain.as_deref()
            {
                let mut mem = serde_json::Map::new();
                if let Some(h) = b.home {
                    mem.insert("home".into(), json!([h.x, h.y, h.z]));
                }
                if let Some(t) = b.last_slept {
                    mem.insert("last_slept".into(), json!(t));
                }
                self.w.call(json!({"op": "memory", "sandbox": self.sb, "entity": e.id, "memories": mem, "take_poi": true}))?;
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
    }
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

/// The judge's run options for the exact compare: the steady state of `seed`, golems standing
/// still (the walking policies are the judge's robustness input, not the game's AI).
pub fn run_opts(seed: i64, ticks: u32) -> RunOpts {
    redsim::entity::mob::farm::judge_opts(seed, ticks, redsim::entity::mob::farm::WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still))
}

/// redsim's side: the runner with the exact streams on (the level random mirrored on the
/// sandbox's chunk box, random ticks at speed 3 from the bridge's randValue).
pub fn redsim_start(spec: &FarmSpec, o: &RunOpts) -> Result<Run> {
    let mut run = Run::start(spec, o).map_err(|e| anyhow!(e))?;
    run.world.mirror_level_random(o.seed, chunk_box(spec));
    run.world.random_ticks = Some(redsim::crops::RandomTicks::new(3, o.seed));
    Ok(run)
}

/// Compare a farm design over `opts.ticks` game ticks after the settle; the compare's result.
pub fn compare(version: &str, design: &std::path::Path, opts: &Opts, draws_at: Option<i64>) -> Result<Compare> {
    let c = circuit::Circuit::load(design)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let o = run_opts(opts.seed, opts.ticks);
    let mut run = redsim_start(&spec, &o)?;
    let mut game = GameFarm::start(version, &c, &spec, &o, opts.weather, draws_at.is_some())?;
    let mut watch = Watch::new(&run.world, spec.lo, spec.hi);
    let mut cmp = Compare::default();
    let window = |t0: i64, n: u32| draws_at.is_some_and(|d| d >= t0 && d < t0 + n as i64 + 3);
    let mut t: i64 = 0;
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, window(t, n))? {
            run.settle_tick();
            let ours = watch.frame(&run.world);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() && !opts.full {
            return Ok(cmp);
        }
    }
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, window(t, n))? {
            run.tick();
            let ours = watch.frame(&run.world);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() && !opts.full {
            break;
        }
    }
    Ok(cmp)
}

/// Give redsim's mobs the run's follow-range bonuses (in id order): `AttributeInstance`'s value,
/// base + base x bonus (`ADD_MULTIPLIED_BASE`).
pub fn set_bonuses(w: &mut redsim::world::World, bonus: &[f64]) {
    let mut k = 0;
    for e in w.entities.list.iter_mut() {
        if let EntityKind::Mob(m) = &mut e.kind {
            let base = m.ty.def().follow_range;
            if let Some(b) = bonus.get(k) {
                m.follow_range = base + base * b;
            }
            k += 1;
        }
    }
}

/// The bridge against itself: the same farm run twice in two workers, frame for frame (every
/// field the game reports), the determinism check of the bridge's seeding.
pub fn twin(version: &str, design: &std::path::Path, opts: &Opts) -> Result<Compare> {
    let c = circuit::Circuit::load(design)?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
    let o = run_opts(opts.seed, opts.ticks);
    let mut run = redsim_start(&spec, &o)?;
    let mut a = GameFarm::start(version, &c, &spec, &o, opts.weather, false)?;
    let mut b = GameFarm::start(version, &c, &spec, &o, opts.weather, false)?;
    let mut cmp = Compare::default();
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, false)?, b.trace(n, false)?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            cmp.tick(x, &as_ours(y));
        }
        left -= n;
    }
    while run.settle_tick() {}
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    a.arrive(&c, &spec, &run.world, &bonus)?;
    b.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, false)?, b.trace(n, false)?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            cmp.tick(x, &as_ours(y));
        }
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
