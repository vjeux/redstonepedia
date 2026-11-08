//! `redsim-fuzz trace-compare <exact traces>...` (track clk-misc, the fluids + entities port's
//! gate): replay a design in redsim and compare it, tick by tick, with an exact real-game trace
//! (`mctas clock misc trace-export --exact`, designs/clocks/traces/exact/<stem>.trace).
//!
//! The design is built like `clock sim` / `clock verify` (raw placement, block-entity data, a
//! `self` update of every block; the design's `entity:` lines summoned once the blocks stand,
//! once redsim has entities), the kick's inputs are applied between ticks, and frame t (the state
//! after t ticks, the inputs due at t applied) is compared with the trace's line t: EVERY cell of
//! the trace's watched box (the trace lists every non-air cell at t = 0 and the changed cells
//! after that), then every entity (same count and order, type, position, motion and rotations
//! bit for bit). Prints `OK` (every tick identical), the first divergence, or what redsim does
//! not model yet.

use anyhow::{Context, Result, bail};
use clock::{Action, Clock, Placement};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::path::Path;

/// One entity of a trace line (exact values).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceEntity {
    pub id: u64,
    pub kind: String,
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// The game's network id (`getId()`), when the trace has it (`net_id`).
    pub net_id: Option<i32>,
}

/// A parsed exact trace.
pub struct Trace {
    pub name: String,
    pub design: String,
    pub period: u32,
    pub warmup: u32,
    pub from: [i32; 3],
    pub to: [i32; 3],
    /// Per tick: the cells that changed (all non-air cells at t = 0), and every entity.
    pub frames: Vec<(Vec<([i32; 3], String)>, Vec<TraceEntity>)>,
    /// The sandbox seed and forced chunk box `[minX, minZ, maxX, maxZ]` (header `seed`,
    /// `chunks`): with both, redsim mirrors the level random (`World::mirror_level_random`).
    pub seed: Option<i64>,
    pub chunks: Option<[i32; 4]>,
    /// The sandbox's `randomTickSpeed` game rule (header `random_tick_speed`, default 3): each
    /// forced chunk draws that many `nextInt(48)` per tick (a grass scene recorded at 0 has no
    /// random ticks, which redsim does not model).
    pub random_tick_speed: u32,
    /// A test scene's extra inputs (header `actions`, `trace-export --action` / `--circuit`):
    /// (tick, `use` / `use_entity` / `set` / `clear`, cell, the state for `set`), replayed after
    /// the design's own inputs of that tick.
    pub actions: Vec<(u32, String, [i32; 3], Option<String>)>,
    /// Per tick: the level random's state after the frame (`rng`, exact traces from a bridge
    /// with `random_state`), checked against the mirror every frame.
    pub rng: Vec<Option<i64>>,
    /// Per tick: `Level.randValue` after the frame (`rv`, a bridge that seeds it): with it, redsim
    /// runs the random ticks and checks the generator every frame.
    pub rv: Vec<Option<i32>>,
    /// The game's chunk tick order (header `chunk_order`).
    pub chunk_order: Option<Vec<[i32; 2]>>,
    /// Blocks the judge writes raw after the design (header `raw`: a rail judge's track and chests,
    /// `circuit::rig`), before frame 0.
    pub raw: Vec<([i32; 3], String)>,
    /// A rail judge's entity actions (header `script`: `[t, op, ...]`, `circuit::rig::GameRig`),
    /// replayed in order after the other inputs of their tick: `summon` (a batch of `[type, pos,
    /// nbt]`), `ride` / `kill` / `empty` (entities by network id), `fill` (a container's Items),
    /// `use` (a right-click).
    pub script: Vec<(u32, serde_json::Value)>,
}

fn f3(v: &serde_json::Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

fn i3(v: &serde_json::Value) -> Option<[i32; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_i64()? as i32, a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32])
}

pub fn load_trace(path: &Path) -> Result<Trace> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = text.lines();
    let h: serde_json::Value = serde_json::from_str(lines.next().context("empty trace")?)?;
    let bx = h["box"].as_array().context("header: box")?;
    let mut t = Trace {
        name: h["name"].as_str().unwrap_or("").to_string(),
        design: h["design"].as_str().context("header: design")?.to_string(),
        period: h["period"].as_u64().unwrap_or(0) as u32,
        warmup: h["warmup"].as_u64().unwrap_or(0) as u32,
        from: i3(&bx[0]).context("header: box")?,
        to: i3(&bx[1]).context("header: box")?,
        frames: Vec::new(),
        seed: h["seed"].as_i64(),
        chunks: h["chunks"].as_array().and_then(|a| {
            let v: Vec<i32> = a.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        }),
        random_tick_speed: h["random_tick_speed"].as_u64().unwrap_or(3) as u32,
        actions: h["actions"]
            .as_array()
            .map(|v| v.iter().filter_map(|a| Some((a.get(0)?.as_u64()? as u32, a.get(1)?.as_str()?.to_string(), i3(a.get(2)?)?, a.get(3).and_then(|s| s.as_str()).map(|s| s.to_string())))).collect())
            .unwrap_or_default(),
        rng: Vec::new(),
        rv: Vec::new(),
        chunk_order: h["chunk_order"].as_array().map(|a| a.iter().filter_map(|c| Some([c.get(0)?.as_i64()? as i32, c.get(1)?.as_i64()? as i32])).collect()),
        raw: h["raw"].as_array().map(|v| v.iter().filter_map(|b| Some(([b.get(0)?.as_i64()? as i32, b.get(1)?.as_i64()? as i32, b.get(2)?.as_i64()? as i32], b.get(3)?.as_str()?.to_string()))).collect()).unwrap_or_default(),
        script: h["script"].as_array().map(|v| v.iter().filter_map(|a| Some((a.get(0)?.as_u64()? as u32, serde_json::Value::Array(a.as_array()?[1..].to_vec())))).collect()).unwrap_or_default(),
    };
    for (i, line) in lines.enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).with_context(|| format!("line {}", i + 2))?;
        let mut blocks = Vec::new();
        for b in v["blocks"].as_array().context("blocks")? {
            let a = b.as_array().context("block")?;
            let p = [a[0].as_i64().context("x")? as i32, a[1].as_i64().context("y")? as i32, a[2].as_i64().context("z")? as i32];
            blocks.push((p, a[3].as_str().context("state")?.to_string()));
        }
        let mut ents = Vec::new();
        for e in v["entities"].as_array().context("entities")? {
            let motion = f3(&e["motion"]).context("entity motion: an exact trace (trace-export --exact) is needed")?;
            ents.push(TraceEntity {
                id: e["id"].as_u64().unwrap_or(0),
                kind: e["type"].as_str().unwrap_or("").to_string(),
                pos: f3(&e["pos"]).context("entity pos")?,
                motion,
                yaw: e["yaw"].as_f64().unwrap_or(0.0) as f32,
                pitch: e["pitch"].as_f64().unwrap_or(0.0) as f32,
                net_id: e["net_id"].as_i64().map(|n| n as i32),
            });
        }
        t.frames.push((blocks, ents));
        t.rng.push(v["rng"].as_i64());
        t.rv.push(v["rv"].as_i64().map(|n| n as i32));
    }
    Ok(t)
}

fn bp(p: [i32; 3]) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The game's state string through redsim's parser (so formatting differences never count), or
/// `?state` for a block redsim does not model.
fn canon(cache: &mut HashMap<String, String>, s: &str) -> String {
    if let Some(c) = cache.get(s) {
        return c.clone();
    }
    let c = match Block::parse(s) {
        Ok(b) => b.to_string(),
        Err(_) => format!("?{s}"),
    };
    cache.insert(s.to_string(), c.clone());
    c
}

/// The outcome for one trace.
pub enum Outcome {
    /// Every tick identical.
    Same { ticks: usize },
    /// The first divergence: the tick and up to a few differences.
    Diverges { tick: usize, diffs: Vec<String>, more: usize },
    /// redsim cannot build or run it yet.
    Unmodeled(String),
}

/// The design's entities as redsim entities (`Sandbox.summon`: created at their position, at
/// rest, in the design's order, once the blocks stand; `redsim::entity::spawn`, the clock sim's
/// path too); an error names what is missing.
fn summon(w: &mut World, c: &Clock) -> Result<(), String> {
    for e in &c.design.entities {
        // spawn names any data it does not model (Motion, Rotation; items' and containers' own)
        redsim::entity::spawn(w, &e.kind, e.pos, e.nbt.as_deref())?;
    }
    Ok(())
}

/// The live entities of redsim's world, in tick order.
fn entities_of(w: &World) -> Vec<TraceEntity> {
    use redsim::entity::EntityKind;
    // every entity in id (spawn) order: falling blocks ([`redsim::gravity`]) share the id counter
    let mut out: Vec<(u32, TraceEntity)> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| {
            (
                e.id,
                TraceEntity {
                    id: e.id as u64,
                    kind: match &e.kind {
                        EntityKind::Boat(b) => if b.chest { "minecraft:oak_chest_boat" } else { "minecraft:oak_boat" }.to_string(),
                        EntityKind::Minecart(m) => if m.hopper { "minecraft:hopper_minecart" } else { "minecraft:minecart" }.to_string(),
                        EntityKind::Item(_) => "minecraft:item".to_string(),
                        EntityKind::ArmorStand(_) => "minecraft:armor_stand".to_string(),
                        EntityKind::PrimedTnt(_) => "minecraft:tnt".to_string(),
                        EntityKind::Mob(m) => m.ty.game_id().to_string(),
                    },
                    pos: [e.pos.x, e.pos.y, e.pos.z],
                    motion: [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z],
                    yaw: e.y_rot,
                    pitch: e.x_rot,
                    net_id: Some(e.network_id),
                },
            )
        })
        .collect();
    out.extend(w.gravity.entities.iter().filter(|f| !f.removed).map(|f| {
        (
            f.id,
            TraceEntity {
                id: f.id as u64,
                kind: "minecraft:falling_block".to_string(),
                pos: [f.x, f.y, f.z],
                motion: [f.vx, f.vy, f.vz],
                yaw: 0.0,
                pitch: 0.0,
                net_id: Some(f.id as i32),
            },
        )
    }));
    out.sort_by_key(|(id, _)| *id);
    out.into_iter().map(|(_, e)| e).collect()
}

fn entity_diffs(want: &[TraceEntity], have: &[TraceEntity], out: &mut Vec<String>) {
    if want.len() != have.len() {
        out.push(format!("{} entities in the game, {} in redsim", want.len(), have.len()));
    }
    for (i, (g, r)) in want.iter().zip(have).enumerate() {
        let short = |k: &str| k.trim_start_matches("minecraft:").to_string();
        if g.kind != r.kind {
            out.push(format!("entity {i}: {} in the game, {} in redsim", short(&g.kind), short(&r.kind)));
            continue;
        }
        let bits = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| a[k].to_bits() == b[k].to_bits() || (a[k] == 0.0 && b[k] == 0.0));
        if !bits(g.pos, r.pos) {
            out.push(format!("{} {i} pos: game {:?}, redsim {:?}", short(&g.kind), g.pos, r.pos));
        }
        if !bits(g.motion, r.motion) {
            out.push(format!("{} {i} motion: game {:?}, redsim {:?}", short(&g.kind), g.motion, r.motion));
        }
        if g.yaw.to_bits() != r.yaw.to_bits() || g.pitch.to_bits() != r.pitch.to_bits() {
            out.push(format!("{} {i} rotation: game ({}, {}), redsim ({}, {})", short(&g.kind), g.yaw, g.pitch, r.yaw, r.pitch));
        }
        // the bridge numbers each sandbox's entities from 0 in the order they enter (the game's
        // tick order; `(tickCount + id) % 4` paces an item at rest): redsim's ids must agree
        if let (Some(a), Some(b)) = (g.net_id, r.net_id)
            && a != b
        {
            out.push(format!("{} {i} network id: game {a}, redsim {b}", short(&g.kind)));
        }
    }
}

/// One entry of a trace's `script` (`[op, ...]`, its tick removed): see [`Trace::script`].
fn apply_script(w: &mut World, op: &serde_json::Value) -> Result<(), String> {
    let a = op.as_array().ok_or("not an array")?;
    let name = a.first().and_then(|v| v.as_str()).ok_or("no op")?;
    // an entity by the game's network id (redsim's match: trace-compare checks them)
    let ent = |w: &World, v: Option<&serde_json::Value>| -> Result<u32, String> {
        let n = v.and_then(|v| v.as_i64()).ok_or("no entity id")? as i32;
        w.entities.list.iter().find(|e| e.network_id == n).map(|e| e.id).ok_or(format!("no entity with network id {n}"))
    };
    match name {
        "summon" => {
            for s in a.get(1).and_then(|v| v.as_array()).ok_or("summon: no list")? {
                let kind = s.get(0).and_then(|v| v.as_str()).ok_or("summon: no type")?;
                let pos = s.get(1).and_then(f3).ok_or("summon: no position")?;
                let nbt = s.get(2).and_then(|v| v.as_str());
                redsim::entity::spawn(w, kind, pos, nbt)?;
            }
            Ok(())
        }
        "ride" => {
            let (r, v) = (ent(w, a.get(1))?, ent(w, a.get(2))?);
            redsim::entity::ride(w, r, v)
        }
        "kill" => {
            let id = ent(w, a.get(1))?;
            redsim::entity::kill(w, id)
        }
        "empty" => {
            let id = ent(w, a.get(1))?;
            if let Some(c) = w.containers.entities.get_mut(&id) {
                for s in c.slots.iter_mut() {
                    *s = None;
                }
            }
            Ok(())
        }
        "fill" => {
            let p = a.get(1).and_then(i3).ok_or("fill: no cell")?;
            let items = a.get(2).and_then(|v| v.as_str()).ok_or("fill: no items")?;
            let v = redsim::redstone::container::entity_items(&format!("{{Items:{items}}}"))?;
            redsim::redstone::container::set_contents(w, bp(p), &v);
            Ok(())
        }
        "use" => {
            let p = a.get(1).and_then(i3).ok_or("use: no cell")?;
            if w.use_block(bp(p)) { Ok(()) } else { Err(format!("a right-click on {} is not modeled", w.get(bp(p)))) }
        }
        other => Err(format!("unknown script op {other}")),
    }
}

/// Replay `trace` in redsim and compare.
pub fn compare(trace: &Trace, max_diffs: usize) -> Result<Outcome> {
    let c = Clock::load(Path::new(&trace.design)).with_context(|| format!("load {}", trace.design))?;
    let mut blocks = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for (p, s) in &c.design.blocks {
        match Block::parse(s) {
            Ok(b) => blocks.push((bp(*p), b)),
            Err(e) => {
                if !bad.contains(&e) {
                    bad.push(e)
                }
            }
        }
    }
    if !bad.is_empty() {
        return Ok(Outcome::Unmodeled(format!("blocks: {}", bad.join("; "))));
    }
    // the clock judge's lamp in an air output cell (clock::sim::judge_lamp; the usable-output
    // rule of 2026-10-21 moved many clocks' outputs to an air cell beside a tap), when the game run
    // placed it there (its first frame holds the lamp; traces recorded before the move do not)
    if let Some(op) = clock::sim::judge_lamp(&c)
        && trace.frames.first().is_some_and(|(cells, _)| cells.iter().any(|(p, s)| *p == op && s.starts_with("minecraft:redstone_lamp")))
        && let Ok(b) = Block::parse(clock::sim::JUDGE_LAMP)
    {
        blocks.push((bp(op), b));
    }
    let (from, to) = (trace.from, trace.to);
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut w = World::new(bp(from), size);
    // TNT entities (their kick replaced: each new one takes the game's state when first seen)
    w.tnt_kick = Some(0.0);
    // the level random, mirrored from the sandbox's seed and forced chunks (before placement:
    // placement can draw, e.g. a powered door's sound), and the bridge's seeded drops (popResource
    // items: `rng::Drop` from the same seed)
    if let (Some(seed), Some(chunks)) = (trace.seed, trace.chunks) {
        w.mirror_level_random(seed, chunks);
        // the sandbox's randomTickSpeed rule (3 unless the trace says otherwise)
        w.tick_speed = trace.random_tick_speed;
        w.drop_seed = Some(seed);
        // a trace from a bridge that seeds Level.randValue (its frames carry `rv`): the random
        // ticks run, mirrored from the bridge's seed, in the game's chunk order (the header's
        // `chunk_order`, which redsim's must match)
        if trace.rv.iter().any(|v| v.is_some()) {
            w.random_ticks = Some(redsim::crops::RandomTicks::new(trace.random_tick_speed, seed));
        }
        if let Some(game) = &trace.chunk_order
            && *game != w.chunk_order
        {
            return Ok(Outcome::Diverges { tick: 0, diffs: vec![format!("chunk tick order: game {game:?}, redsim {:?} (crops::tick_order of the forced box {chunks:?})", w.chunk_order)], more: 0 });
        }
    }
    let last = c.withheld().map(bp);
    let be: Vec<(BlockPos, &str)> = c.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).filter(|(p, _)| Some(*p) != last).collect();
    match c.placement {
        Placement::Judge => {
            let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
            w.place_raw(&silent);
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
            w.place_updates(&silent);
        }
        Placement::Player => {
            for (p, b) in blocks.iter().filter(|(p, _)| Some(*p) != last) {
                w.set_block(*p, *b, flags::ALL);
            }
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
        }
    }
    if let Err(e) = summon(&mut w, &c) {
        return Ok(Outcome::Unmodeled(e));
    }
    // a rail judge's raw writes after the design (its track, its chests)
    if !trace.raw.is_empty() {
        let mut raw = Vec::new();
        for (p, s) in &trace.raw {
            match Block::parse(s) {
                Ok(b) => raw.push((bp(*p), b)),
                Err(e) => return Ok(Outcome::Unmodeled(format!("the judge's raw block {s}: {e}"))),
            }
        }
        w.place_raw(&raw);
    }
    let mut script: Vec<(u32, serde_json::Value)> = trace.script.clone();
    let mut actions = c.actions();
    // a test scene's extra inputs (header `actions`), after the design's own at the same tick
    for (t, kind, pos, state) in &trace.actions {
        let a = match (kind.as_str(), state) {
            ("use", _) => Action::Use { pos: *pos },
            ("use_entity", _) => Action::UseEntity { pos: *pos },
            ("set", Some(st)) => Action::Place { pos: *pos, state: st.clone(), nbt: None },
            ("clear", _) => Action::Clear { pos: *pos },
            ("shape_set", Some(st)) => Action::ShapeSet { pos: *pos, state: st.clone() },
            ("update", _) => Action::Update { pos: *pos },
            // (the event's name: the judge's fixed set, leaked to the 'static the action holds)
            ("game_event", Some(ev)) => Action::GameEvent { pos: *pos, event: Box::leak(ev.clone().into_boxed_str()) },
            ("drop", Some(item)) => Action::Drop { pos: *pos, item: item.clone() },
            (other, _) => return Ok(Outcome::Unmodeled(format!("a trace action {other:?} is not known"))),
        };
        actions.push((*t, a));
    }
    actions.sort_by_key(|(t, _)| *t);
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut expect: HashMap<[i32; 3], String> = HashMap::new();
    // primed TNT (track tnt): redsim records primes, not PrimedTnt entities (their motion draws
    // from level.random); the trace's TNT entities are compared as spawns (new ids per tick, by
    // the cell they stand in) with redsim's primes of that tick, and left out of the entity check
    let mut tnt_seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut sim_tnt_seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut primed_seen = 0usize;
    let mut chain_seen = 0usize;
    // redsim's level-random draws per frame (the last few, for a drift report)
    let mut sim_rng: Vec<(usize, String)> = Vec::new();
    let mut sim_rng_prev: Option<i64> = None;
    for (t, (changes, ents)) in trace.frames.iter().enumerate() {
        // the exporter's order: the inputs due at t, then (t > 0) one tick, then the frame
        while actions.first().is_some_and(|(at, _)| *at as usize == t) {
            let (_, a) = actions.remove(0);
            match a {
                Action::Place { pos, state, nbt } => {
                    let b = Block::parse(&state).map_err(|e| anyhow::anyhow!(e))?;
                    w.set_block(bp(pos), b, flags::ALL);
                    if let Some(n) = nbt {
                        redsim::redstone::container::apply_block_entities(&mut w, [(bp(pos), n.as_str())]).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
                    }
                }
                Action::UseEntity { pos } => {
                    if !redsim::entity::frame::use_frame(&mut w, bp(pos)) {
                        return Ok(Outcome::Unmodeled(format!("no single item frame at {pos:?} to turn")));
                    }
                }
                Action::Use { pos } => {
                    if !w.use_block(bp(pos)) {
                        return Ok(Outcome::Unmodeled(format!("a right-click on {} is not modeled", w.get(bp(pos)))));
                    }
                }
                Action::Clear { pos } => {
                    w.set_block(bp(pos), Block::Air, flags::ALL);
                }
                Action::ShapeSet { pos, state } => {
                    let b = Block::parse(&state).map_err(|e| anyhow::anyhow!(e))?;
                    w.set_block(bp(pos), b, flags::CLIENTS);
                }
                Action::Update { pos } => {
                    let src = w.get(bp(pos)).kind();
                    w.update_neighbors_at(bp(pos), src);
                }
                Action::GameEvent { pos, event } => {
                    w.game_event(event, bp(pos));
                }
                Action::Drop { pos, item } => {
                    let (kind, at, nbt) = Action::drop_entity(pos, &item);
                    if let Err(e) = redsim::entity::spawn(&mut w, &kind, at, Some(&nbt)) {
                        return Ok(Outcome::Unmodeled(format!("a dropped item: {e}")));
                    }
                }
            }
        }
        // a rail judge's entity actions due at t, in order
        while script.first().is_some_and(|(at, _)| *at as usize == t) {
            let (_, op) = script.remove(0);
            if let Err(e) = apply_script(&mut w, &op) {
                return Ok(Outcome::Unmodeled(format!("tick {t}: script {op}: {e}")));
            }
        }
        if t > 0 {
            w.tick();
        }
        for (p, s) in changes {
            expect.insert(*p, canon(&mut cache, s));
        }
        let mut diffs = Vec::new();
        let mut n_diff = 0usize;
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    let p = [x, y, z];
                    let game = expect.get(&p).cloned().unwrap_or_else(|| Block::Air.to_string());
                    let ours = w.get(bp(p)).to_string();
                    if game != ours {
                        n_diff += 1;
                        if diffs.len() < max_diffs {
                            diffs.push(format!("{x},{y},{z}: game {game}, redsim {ours}"));
                        }
                    }
                }
            }
        }
        let mut ed = Vec::new();
        // (a TNT is first seen one tick after it spawned and may already have been pushed one
        // cell by the machine: counts per tick must match, cells within 1)
        let is_tnt = |e: &&TraceEntity| e.kind.trim_start_matches("minecraft:") == "tnt";
        let new_game: Vec<&TraceEntity> = ents.iter().filter(is_tnt).filter(|e| tnt_seen.insert(e.id)).collect();
        let mut game_spawns: Vec<[i32; 3]> = new_game.iter().map(|e| [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]).collect();
        let mut sim_spawns: Vec<[i32; 3]> = w.primed[primed_seen..].iter().chain(&w.chain_primed[chain_seen..]).map(|(_, p)| [p.x, p.y, p.z]).collect();
        primed_seen = w.primed.len();
        chain_seen = w.chain_primed.len();
        game_spawns.sort();
        sim_spawns.sort();
        let near = |a: &[i32; 3], b: &[i32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() <= 1);
        if game_spawns.len() != sim_spawns.len() || !game_spawns.iter().all(|g| sim_spawns.iter().any(|s| near(g, s))) {
            ed.push(format!("primed TNT: game {game_spawns:?}, redsim {sim_spawns:?}"));
        }
        // redsim's TNT entities (World::tnt_kick): without the level random the game's random kick
        // is unknown, so each new TNT takes the game's state at its first sighting; from then on it
        // is compared bit for bit (with the level random mirrored, the kick is the game's own:
        // nothing is injected)
        let new_sim: Vec<usize> = (0..w.entities.list.len()).filter(|&k| matches!(w.entities.list[k].kind, redsim::entity::EntityKind::PrimedTnt(_)) && !w.entities.list[k].removed && sim_tnt_seen.insert(w.entities.list[k].id)).collect();
        if w.level_random.is_none() {
            for (g, &k) in new_game.iter().zip(&new_sim) {
                let e = &mut w.entities.list[k];
                e.set_pos(redsim::entity::Vec3::new(g.pos[0], g.pos[1], g.pos[2]));
                e.delta_movement = redsim::entity::Vec3::new(g.motion[0], g.motion[1], g.motion[2]);
            }
        }
        // item frames (redsim::entity::frame: static comparator inputs, World::frames): left out of
        // the moving-entity check, compared by their cells (their rotations show through the
        // comparators reading them, in the block frames)
        let frame_cells: Vec<[i32; 3]> = {
            let mut v: Vec<[i32; 3]> = ents.iter().filter(|e| e.kind == "minecraft:item_frame").map(|e| [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]).collect();
            v.sort();
            v
        };
        let sim_frames: Vec<[i32; 3]> = {
            let mut v: Vec<[i32; 3]> = w.frames.iter().map(|f| [f.pos.x, f.pos.y, f.pos.z]).collect();
            v.sort();
            v
        };
        if frame_cells != sim_frames {
            ed.push(format!("item frames: game in cells {frame_cells:?}, redsim {sim_frames:?}"));
        }
        let moving: Vec<TraceEntity> = ents.iter().filter(|e| e.kind != "minecraft:item_frame").cloned().collect();
        entity_diffs(&moving, &entities_of(&w), &mut ed);
        // the level random after this frame (exact traces with `rng`): a drift shows on the tick
        // it starts, not later through behavior; with it, the draws of the last frames on both
        // sides and redsim's explosions so far
        if let (Some(game), Some(r)) = (trace.rng.get(t).copied().flatten(), w.level_random.as_ref()) {
            if let Some(d) = redsim::rng::state_diff(game, r.state()) {
                ed.push(d);
                let draws = |a: Option<i64>, b: i64| a.and_then(|a| redsim::rng::lcg_steps(a, b, 100_000)).map(|k| k.to_string()).unwrap_or("?".into());
                let lo = t.saturating_sub(6);
                let game_draws: Vec<String> = (lo.max(1)..=t).map(|k| format!("t{k}: {}", draws(trace.rng.get(k - 1).copied().flatten(), trace.rng.get(k).copied().flatten().unwrap_or(0)))).collect();
                ed.push(format!("game draws per frame: {}", game_draws.join(", ")));
                let sim_draws: Vec<String> = sim_rng.iter().filter(|(k, _)| *k >= lo.max(1)).map(|(k, n)| format!("t{k}: {n}")).collect();
                ed.push(format!("redsim draws per frame: {}, then {} at t{t}", sim_draws.join(", "), draws(sim_rng_prev, r.state())));
                for b in &w.explosions {
                    ed.push(format!("redsim explosion at game time {} center ({:.4}, {:.4}, {:.4}): {} positions, {} destroyed {:?}", b.time, b.center.x, b.center.y, b.center.z, b.positions, b.destroyed.len(), b.destroyed.iter().take(6).map(|(p, s)| format!("{p} {s}")).collect::<Vec<_>>()));
                }
            }
        }
        if let Some(r) = w.level_random.as_ref() {
            let s = r.state();
            if let Some(prev) = sim_rng_prev {
                sim_rng.push((t, redsim::rng::lcg_steps(prev, s, 100_000).map(|k| k.to_string()).unwrap_or("?".into())));
                if sim_rng.len() > 8 {
                    sim_rng.remove(0);
                }
            }
            sim_rng_prev = Some(s);
        }
        // Level.randValue after this frame (traces with `rv`): the random ticks' positions
        if let (Some(game), Some(rt)) = (trace.rv.get(t).copied().flatten(), w.random_ticks.as_ref())
            && game != rt.value
        {
            ed.push(format!("Level.randValue: game {game}, redsim {} (the random ticks' or precipitation picks' count differs)", rt.value));
        }
        n_diff += ed.len();
        for d in ed {
            if diffs.len() < max_diffs {
                diffs.push(d);
            }
        }
        if n_diff > 0 {
            let shown = diffs.len();
            if let Some(e) = w.errors.first() {
                diffs.push(format!("(redsim's model note this tick: {e})"));
            }
            return Ok(Outcome::Diverges { tick: t, diffs, more: n_diff - shown });
        }
        if let Some(e) = w.errors.first() {
            return Ok(Outcome::Unmodeled(format!("tick {t}: {e}")));
        }
    }
    Ok(Outcome::Same { ticks: trace.frames.len() })
}

/// Compare every trace; one line each; Err if any is not identical.
pub fn run(paths: &[std::path::PathBuf], max_diffs: usize) -> Result<()> {
    let (mut same, mut total) = (0, 0);
    for p in paths {
        total += 1;
        let t = load_trace(p)?;
        match compare(&t, max_diffs)? {
            Outcome::Same { ticks } => {
                same += 1;
                println!("OK {}: {ticks} ticks identical", t.name);
            }
            Outcome::Diverges { tick, diffs, more } => {
                println!("DIVERGES {} at tick {tick} (warmup {}, period {}):", t.name, t.warmup, t.period);
                for d in diffs {
                    println!("  {d}");
                }
                if more > 0 {
                    println!("  ... and {more} more");
                }
            }
            Outcome::Unmodeled(why) => println!("UNMODELED {}: {why}", t.name),
        }
    }
    println!("{same} of {total} traces tick-identical");
    if same < total {
        bail!("{} traces differ or are not modeled", total - same);
    }
    Ok(())
}
