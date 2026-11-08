//! `mctas mobai fixture`: a mob-AI scene run in the game (a bridge sandbox, every random seeded
//! from the sandbox seed: bridge `Exact.java`) and in redsim (the mob AI on, the same streams),
//! compared tick for tick with `redsim::exact` (track tickexact's frames: position, motion,
//! rotations, health, the own random's state, the head and body yaw, the running goals, the path
//! and the target of every mob).
//!
//! A scene is a few blocks (raw, in order), the mobs (a seeded summon with their data), the
//! difficulty, the game rules and the day time; nothing else happens in the sandbox (natural
//! spawning and the weather off).

use anyhow::{Result, anyhow, bail};
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::entity::mob::natural::Difficulty;
use redsim::exact::{Compare, Watch};
use redsim::pos::BlockPos;
use redsim::world::World;
use serde_json::json;

/// A scene.
#[derive(Clone, Debug)]
pub struct Scene {
    pub name: String,
    /// Chunk box `[min_cx, min_cz, max_cx, max_cz]`.
    pub chunks: [i32; 4],
    /// Blocks (game state strings), placed raw in this order.
    pub blocks: Vec<([i32; 3], String)>,
    /// Mobs: type, feet, data.
    pub mobs: Vec<(String, [f64; 3], Option<String>)>,
    pub day_time: i64,
    pub difficulty: Difficulty,
    pub mob_griefing: bool,
    /// The watched box (block changes), inclusive.
    pub watch: ([i32; 3], [i32; 3]),
}

fn difficulty_name(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Peaceful => "peaceful",
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

/// A stone floor over `x0..x1`, `z0..z1` at height `y`.
fn floor(out: &mut Vec<([i32; 3], String)>, x0: i32, x1: i32, z0: i32, z1: i32, y: i32, state: &str) {
    for x in x0..x1 {
        for z in z0..z1 {
            out.push(([x, y, z], state.to_string()));
        }
    }
}

/// A wall from `(x0, z0)` to `(x1, z1)` (inclusive, one axis), `h` high above `y`.
fn wall(out: &mut Vec<([i32; 3], String)>, a: (i32, i32), b: (i32, i32), y: i32, h: i32, state: &str) {
    for x in a.0.min(b.0)..=a.0.max(b.0) {
        for z in a.1.min(b.1)..=a.1.max(b.1) {
            for dy in 1..=h {
                out.push(([x, y + dy, z], state.to_string()));
            }
        }
    }
}

/// The built-in scenes.
pub fn scenes() -> Vec<Scene> {
    let stone = "minecraft:stone";
    let mut v = Vec::new();
    // a zombie alone on a floor at night: strolls (LandRandomPos), looks around
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "zombie-stroll".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:zombie".into(), [24.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
        });
    }
    // a zombie and a golem walled off from it (the golem stands still: no AI modeled for it yet,
    // so the scene's golem never moves: NoAI)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (16, 4), (16, 40), 63, 3, stone);
        wall(&mut b, (30, 8), (30, 46), 63, 2, stone);
        floor(&mut b, 20, 26, 20, 26, 64, stone);
        v.push(Scene {
            name: "zombie-obstacles".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![("minecraft:zombie".into(), [6.5, 64.0, 6.5], Some("{PersistenceRequired:1b}".into()))],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
        });
    }
    // an iron golem and a zombie 12 apart at night: each targets the other (the golem: enemies
    // but creepers, every 3 ticks; the zombie: iron golems, mustSee), the chase, the hits
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        v.push(Scene {
            name: "golem-zombie".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:iron_golem".into(), [8.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:zombie".into(), [20.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
        });
    }
    // a zombie pathing to a villager penned in fences beyond two low walls (the walls and the
    // fences do not block the sight between their eyes): the target, the path around the walls,
    // the hits through the fence; the villager panics (its brain's PANIC)
    {
        let mut b = Vec::new();
        floor(&mut b, 0, 48, 0, 48, 63, stone);
        wall(&mut b, (16, 12), (16, 36), 63, 1, stone);
        wall(&mut b, (26, 20), (26, 44), 63, 1, stone);
        for (x, z) in [(35, 23), (36, 23), (37, 23), (35, 24), (37, 24), (35, 25), (36, 25), (37, 25)] {
            b.push(([x, 64, z], "minecraft:oak_fence".to_string()));
        }
        v.push(Scene {
            name: "zombie-villager".into(),
            chunks: [0, 0, 2, 2],
            blocks: b,
            mobs: vec![
                ("minecraft:zombie".into(), [6.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
                ("minecraft:villager".into(), [36.5, 64.0, 24.5], Some("{PersistenceRequired:1b}".into())),
            ],
            day_time: 18000,
            difficulty: Difficulty::Easy,
            mob_griefing: true,
            watch: ([0, 60, 0], [47, 70, 47]),
        });
    }
    v
}

/// The game's side of a scene.
pub struct GameScene {
    w: Worker,
    sb: u32,
    lo: [i32; 3],
    hi: [i32; 3],
}

impl GameScene {
    pub fn start(version: &str, s: &Scene, seed: i64, trace_randoms: bool) -> Result<GameScene> {
        let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 2048, ..Default::default() })?;
        let sb = w.create(&SandboxOptions { chunks: s.chunks, seed })?;
        w.call(json!({
            "op": "world", "sandbox": sb,
            "advance_time": true, "day_time": s.day_time,
            "advance_weather": false, "spawn_mobs": false, "spawn_monsters": false,
            "difficulty": difficulty_name(s.difficulty),
            "rules": {"mob_griefing": if s.mob_griefing { "true" } else { "false" }},
            "trace_randoms": trace_randoms,
        }))?;
        if !s.blocks.is_empty() {
            w.set_blocks(sb, &s.blocks, PlaceMode::Raw)?;
        }
        Ok(GameScene { w, sb, lo: s.watch.0, hi: s.watch.1 })
    }

    /// Summon the scene's mobs (seeded); their numbers.
    pub fn summon(&mut self, s: &Scene) -> Result<Vec<u64>> {
        let specs: Vec<serde_json::Value> = s.mobs.iter().map(|(ty, p, nbt)| json!({"type": ty, "pos": p, "nbt": nbt})).collect();
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        Ok(v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default())
    }

    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<redsim::exact::Frame>> {
        let v = self.w.call(json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}}))?;
        v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.iter().map(mobfarm::exact::parse_frame).collect()
    }

    /// The bridge's `mobstate` (debugging): each mob's controls and path, one line each.
    pub fn mob_state(&mut self) -> Result<Vec<String>> {
        let v = self.w.call(json!({"op": "mobstate", "sandbox": self.sb}))?;
        Ok(v["mobs"].as_array().map(|a| a.iter().map(|m| m.to_string()).collect()).unwrap_or_default())
    }
}

/// redsim's side of `mobstate`: each AI mob's controls and path.
fn redsim_mob_state(w: &World) -> Vec<String> {
    let mut out = Vec::new();
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        if let EntityKind::Mob(m) = &e.kind
            && let Some(a) = m.ai.as_deref()
        {
            let path = a.nav.path.as_ref().map(|p| format!("{}:{}", p.next_node_index, p.nodes.iter().map(|n| format!(" {},{},{}", n.x, n.y, n.z)).collect::<String>()));
            out.push(format!(
                "{} x {:?} y {:?} z {:?} yaw {:?} move {:?} {:?} {:?} {:?} {:?} look {:?} {:?} {:?} {} path {:?}",
                m.ty.def().id, e.pos.x, e.pos.y, e.pos.z, e.y_rot, a.move_ctl.wanted_x, a.move_ctl.wanted_y, a.move_ctl.wanted_z, a.move_ctl.operation, a.move_ctl.speed_modifier, a.look.wanted_x, a.look.wanted_y, a.look.wanted_z, a.look.look_at_cooldown, path
            ));
        }
    }
    out
}

/// redsim's side of a scene: the world over the chunk box (y -64..319), the blocks raw, the mob
/// layer and its AI on with the bridge's seeding, the level random mirrored, random ticks at 3.
pub fn redsim_world(s: &Scene, seed: i64) -> Result<World> {
    let lo = BlockPos::new(s.chunks[0] * 16, -64, s.chunks[1] * 16);
    let size = [(s.chunks[2] - s.chunks[0] + 1) * 16, 384, (s.chunks[3] - s.chunks[1] + 1) * 16];
    let mut w = World::new(lo, size);
    w.mob.seed(seed, s.day_time);
    w.mob.ai.on = true;
    w.mob.ai.exact_seed = Some(seed);
    w.mob.ai.difficulty = s.difficulty;
    w.mob.ai.mob_griefing = s.mob_griefing;
    let mut list = Vec::new();
    for (p, st) in &s.blocks {
        list.push((BlockPos::new(p[0], p[1], p[2]), Block::parse(st).map_err(|e| anyhow!("{st}: {e}"))?));
    }
    w.place_raw(&list);
    w.mirror_level_random(seed, s.chunks);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(3, seed));
    Ok(w)
}

/// Summon the scene's mobs in redsim (no follow-range bonus: a summon has none).
pub fn redsim_summon(w: &mut World, s: &Scene) -> Result<Vec<u64>> {
    let mut ids = Vec::new();
    for (ty, p, nbt) in &s.mobs {
        let id = redsim::entity::spawn(w, ty, *p, nbt.as_deref()).map_err(|e| anyhow!(e))?;
        ids.push(id as u64);
    }
    for e in w.entities.list.iter_mut() {
        if let EntityKind::Mob(m) = &mut e.kind {
            m.follow_range = m.ty.def().follow_range;
        }
    }
    Ok(ids)
}

/// Run a scene on both sides for `ticks` ticks; the compare. `trace`: at that game time, the game's
/// draws of the tick (kept in the compare's `probe`) and redsim's draws of the level random (or of
/// the entity's own random) printed by call site.
#[allow(clippy::too_many_arguments)]
pub fn compare(version: &str, s: &Scene, seed: i64, ticks: u32, batch: u32, full: bool, probe: bool, trace: Option<(i64, Option<u32>)>) -> Result<Compare> {
    let mut game = GameScene::start(version, s, seed, probe || trace.is_some())?;
    let mut ours = redsim_world(s, seed)?;
    let game_ids = game.summon(s)?;
    let our_ids = redsim_summon(&mut ours, s)?;
    if game_ids != our_ids {
        bail!("the game's mobs entered as {game_ids:?}, redsim's as {our_ids:?}");
    }
    let mut watch = Watch::new(&ours, BlockPos::new(s.watch.0[0], s.watch.0[1], s.watch.0[2]), BlockPos::new(s.watch.1[0], s.watch.1[1], s.watch.1[2]));
    let mut cmp = Compare { probe_at: trace.map(|t| t.0), ..Default::default() };
    let show = std::env::var("MOBAI_SHOW").is_ok();
    let show_until: Option<i64> = std::env::var("MOBAI_SHOW_UNTIL").ok().and_then(|v| v.parse().ok());
    let mut left = ticks;
    let mut first = true;
    let state_range: Option<(i64, i64)> = std::env::var("MOBAI_STATE").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    });
    let mut done: i64 = 0;
    while left > 0 {
        // one tick per traced step inside the state window (the controls between ticks)
        let in_window = state_range.is_some_and(|(a, b)| done + 1 >= a && done < b);
        let n = match state_range {
            _ if in_window => 1,
            Some((a, _)) if done + 1 < a => (left.min(batch) as i64).min(a - 1 - done).max(1) as u32,
            _ => left.min(batch),
        };
        let frames = game.trace(n, probe || trace.is_some())?;
        done += n as i64;
        let game_state = if in_window { game.mob_state()? } else { Vec::new() };
        for f in frames {
            if first {
                // the sandbox's clock: redsim's game time before the first traced tick
                ours.time = f.t - 1;
                if let Some(d) = f.day {
                    ours.mob.day_offset = d - f.t;
                }
                first = false;
            }
            let tracing = trace.filter(|(at, _)| *at == ours.time + 1);
            if let Some((at, ent)) = tracing {
                let addr = match ent {
                    None => ours.level_random.as_ref().map(|r| r as *const _ as usize).unwrap_or(0),
                    Some(id) => ours.entities.list.iter().find(|e| e.id == id).and_then(|e| e.random.as_ref()).map(|r| r as *const _ as usize).unwrap_or(0),
                };
                eprintln!("redsim's draws at game time {at}:");
                redsim::rng::TRACE.store(addr, std::sync::atomic::Ordering::Relaxed);
            }
            ours.tick();
            redsim::rng::TRACE.store(0, std::sync::atomic::Ordering::Relaxed);
            let o = watch.frame(&ours);
            if show && o.t % 100 == 0 {
                for e in &o.entities {
                    eprintln!("t {} #{} {} pos {:?} goals {:?} path {:?} target {:?} no_action {:?}", o.t, e.n, e.ty, e.pos, e.goals, e.path, e.target, e.no_action);
                }
            }
            if let Some(n) = show_until
                && o.t <= n
            {
                for (g, e) in f.entities.iter().zip(o.entities.iter()) {
                    eprintln!("t {} #{} game: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?} | redsim: yaw {} head {:?} body {:?} pitch {} goals {:?} path {:?} pos {:?}", o.t, e.n, g.yaw, g.head, g.body, g.pitch, g.goals, g.path, g.pos, e.yaw, e.head, e.body, e.pitch, e.goals, e.path, e.pos);
                }
            }
            cmp.tick(&f, &o);
            if in_window {
                for l in &game_state {
                    eprintln!("state t {} game   {l}", o.t);
                }
                for l in redsim_mob_state(&ours) {
                    eprintln!("state t {} redsim {l}", o.t);
                }
            }
        }
        left -= n;
        if cmp.first.is_some() && !full {
            break;
        }
    }
    if !ours.errors.is_empty() {
        eprintln!("redsim model errors: {:?}", ours.errors);
    }
    Ok(cmp)
}

/// `mctas mobai fixture <name>`.
#[allow(clippy::too_many_arguments)]
pub fn run(version: &str, name: &str, seed: i64, ticks: u32, full: bool, probe: bool, trace_at: Option<i64>, entity: Option<u32>) -> Result<()> {
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}; scenes: {}", all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")))?;
    let t0 = std::time::Instant::now();
    let c = compare(version, s, seed, ticks, 100, full, probe, trace_at.map(|t| (t, entity)))?;
    if let Some(t) = trace_at {
        println!("the game's draws at game time {t}:");
        for l in &c.probe {
            println!("  {l}");
        }
    }
    print!("{}", c.report());
    eprintln!("{} in {:.1} s", s.name, t0.elapsed().as_secs_f64());
    Ok(())
}
