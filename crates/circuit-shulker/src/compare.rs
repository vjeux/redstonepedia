//! `mctas circuit shulker compare`: replay real-game shulker traces (designs/circuits/shulker/
//! traces/exact/*.trace, from `mctas circuit shulker trace --exact`) in redsim, every cell and
//! entity compared every tick: the gate of redsim's shulker port. The replay is the judge's run
//! (crates/circuit/src/shulker.rs `run_in`, stepping 1 gt as when recording) through redsim's
//! own driver of the judge's chests (`redsim::redstone::shulker::Chests`, which the in-browser
//! viewer runs too) with the judge's supply: the design placed as the judge places it, the
//! chests (raw), the settle, the box supply and the source filled, the start lever on, then each
//! gt: one tick, the frame, the judge's visit (outputs, sinks and rejects emptied; the sources
//! topped up to 27 stacks, compacted into slots 0..). The drops come from the trace's seed
//! (`World::drop_seed`, as the bridge draws them).

use anyhow::{Context, Result, bail};
use circuit::shulker::{Scenario, Stack as JStack};
use circuit::{Circuit, InputHow, Pos};
use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::redstone::container::{self, Stack};
use redsim::redstone::shulker::Chests;
use redsim::world::World;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One trace's result.
pub enum Outcome {
    Ok(u32),
    Diverged(u32, Vec<String>),
    Unmodeled(String),
}

struct Frame {
    blocks: Vec<(Pos, String)>,
    entities: Vec<Value>,
    /// The level random's state after the frame (`rng`, when the trace has it).
    rng: Option<i64>,
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn pos3(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

/// The game's state strings, canonical (redsim's spelling when it parses).
fn canon(cache: &mut HashMap<String, String>, s: &str) -> String {
    cache.entry(s.to_string()).or_insert_with(|| Block::parse(s).map(|b| b.to_string()).unwrap_or_else(|_| s.to_string())).clone()
}

fn entity_rows(w: &World) -> Vec<(String, [f64; 3], [f64; 3], f32, f32, i32)> {
    use redsim::entity::EntityKind;
    w.entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| {
            let kind = match &e.kind {
                EntityKind::Boat(b) => if b.chest { "minecraft:oak_chest_boat" } else { "minecraft:oak_boat" }.to_string(),
                EntityKind::Minecart(m) => if m.hopper { "minecraft:hopper_minecart" } else { "minecraft:minecart" }.to_string(),
                EntityKind::Item(_) => "minecraft:item".to_string(),
                EntityKind::ArmorStand(_) => "minecraft:armor_stand".to_string(),
                EntityKind::PrimedTnt(_) => "minecraft:tnt".to_string(),
                EntityKind::Mob(m) => m.ty.game_id().to_string(),
            };
            (kind, [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z], e.y_rot, e.x_rot, e.network_id)
        })
        .collect()
}

fn entity_diffs(game: &[Value], w: &World, out: &mut Vec<String>) {
    let ours = entity_rows(w);
    if game.len() != ours.len() {
        out.push(format!("{} entities in the game, {} in redsim", game.len(), ours.len()));
    }
    let bits = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| a[k].to_bits() == b[k].to_bits() || (a[k] == 0.0 && b[k] == 0.0));
    for (i, (g, r)) in game.iter().zip(&ours).enumerate() {
        let kind = g["type"].as_str().unwrap_or("");
        if kind != r.0 {
            out.push(format!("entity {i}: {kind} in the game, {} in redsim", r.0));
            continue;
        }
        let (gp, gm) = (pos3(&g["pos"]).unwrap_or_default(), pos3(&g["motion"]).unwrap_or_default());
        if !bits(gp, r.1) {
            out.push(format!("{kind} {i} pos: game {gp:?}, redsim {:?}", r.1));
        }
        if !bits(gm, r.2) {
            out.push(format!("{kind} {i} motion: game {gm:?}, redsim {:?}", r.2));
        }
        let (gy, gx) = (g["yaw"].as_f64().unwrap_or(0.0) as f32, g["pitch"].as_f64().unwrap_or(0.0) as f32);
        if gy.to_bits() != r.3.to_bits() || gx.to_bits() != r.4.to_bits() {
            out.push(format!("{kind} {i} rotation: game ({gy}, {gx}), redsim ({}, {})", r.3, r.4));
        }
        if let Some(n) = g["net_id"].as_i64()
            && n as i32 != r.5
        {
            out.push(format!("{kind} {i} network id: game {n}, redsim {}", r.5));
        }
    }
}

fn load(path: &Path) -> Result<(Value, Vec<Frame>)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = text.lines();
    let header: Value = serde_json::from_str(lines.next().context("empty trace")?)?;
    let mut frames = Vec::new();
    for l in lines {
        let v: Value = serde_json::from_str(l)?;
        let blocks = v["blocks"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|b| {
                        let b = b.as_array()?;
                        Some(([b[0].as_i64()? as i32, b[1].as_i64()? as i32, b[2].as_i64()? as i32], b[3].as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        frames.push(Frame { blocks, entities: v["entities"].as_array().cloned().unwrap_or_default(), rng: v["rng"].as_i64() });
    }
    Ok((header, frames))
}

/// Replay one trace.
pub fn compare_one(path: &Path, max: usize) -> Result<Outcome> {
    let (h, frames) = load(path)?;
    if h["exact"].as_bool() == Some(false) {
        bail!("not an exact trace");
    }
    let design = h["design"].as_str().context("header design")?;
    let c = Circuit::load(Path::new(design))?;
    let b = h["box"].as_array().context("header box")?;
    let from = pos3(&b[0]).map(|v| [v[0] as i32, v[1] as i32, v[2] as i32]).context("box")?;
    let to = pos3(&b[1]).map(|v| [v[0] as i32, v[1] as i32, v[2] as i32]).context("box")?;
    let seed = h["seed"].as_i64().context("header seed (re-record the trace)")?;
    let scen = h["scenario"].as_str().unwrap_or("");
    let loader = scen.starts_with("load");
    let item = scen.rsplit(' ').next().unwrap_or("");
    let stack = match item {
        "iron_shovel" => 1,
        "snowball" => 16,
        _ => 64,
    };
    let sc = Scenario::with_stack(loader, stack);
    let mut w = match clock::sim::world_for(&c.clock, from, to) {
        Ok(w) => w,
        Err(e) => return Ok(Outcome::Unmodeled(e)),
    };
    w.drop_seed = Some(seed);
    // the level random (clk-misc): the sandbox was made from the watch box (game_run_recorded:
    // chunks = the box's chunk range) with this seed
    let chunks = h["chunks"]
        .as_array()
        .and_then(|a| {
            let v: Vec<i32> = a.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect();
            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
        })
        .unwrap_or([from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)]);
    w.mirror_level_random(seed, chunks);
    // the judge's chests (raw), the settle, the box supply and the source, the lever: redsim's
    // driver (`redstone::shulker::Chests`, the viewer's too) with the judge's own supply
    let role = |r: &str| -> Vec<BlockPos> { c.stores.iter().filter(|s| s.role == r).map(|s| bp(s.pos)).collect() };
    let outs: Vec<BlockPos> = c.outputs.iter().map(|o| bp(o.pos)).collect();
    let supply: Vec<Stack> = sc.supply().iter().map(|s| rstack(&mut w, s)).collect::<Result<_>>()?;
    let queue: Vec<Stack> = if loader { sc.source_stacks().iter().map(|s| rstack(&mut w, s)).collect::<Result<_>>()? } else { Vec::new() };
    let mut chests = Chests::judged(loader, role("items"), role("boxes"), outs, role("reject"), supply, queue);
    chests.place(&mut w);
    for _ in 0..c.clock.free_from() + circuit::stimulus::SETTLE {
        w.tick();
    }
    chests.start(&mut w);
    for i in &c.inputs {
        let mode = i.name == "mode" || i.name.starts_with("mode.");
        if i.how == InputHow::Lever && !mode && !w.use_block(bp(i.pos)) {
            return Ok(Outcome::Unmodeled(format!("the lever at {:?}", i.pos)));
        }
    }
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut expect: HashMap<Pos, String> = HashMap::new();
    for (t, f) in frames.iter().enumerate() {
        if t > 0 {
            w.tick();
        }
        if let Some(e) = w.errors.first() {
            return Ok(Outcome::Unmodeled(format!("t={t}: {e}")));
        }
        for (p, s) in &f.blocks {
            expect.insert(*p, canon(&mut cache, s));
        }
        let mut diffs = Vec::new();
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    let p = [x, y, z];
                    let game = expect.get(&p).cloned().unwrap_or_else(|| Block::Air.to_string());
                    let ours = w.get(bp(p)).to_string();
                    if game != ours {
                        diffs.push(format!("{x},{y},{z}: game {game}, redsim {ours}"));
                    }
                }
            }
        }
        entity_diffs(&f.entities, &w, &mut diffs);
        // the level random (clk-misc), every frame of a trace that records it
        if let (Some(game), Some(r)) = (f.rng, w.level_random.as_ref()) {
            if let Some(d) = redsim::rng::state_diff(game, r.state()) {
                diffs.push(d);
            }
        }
        if !diffs.is_empty() {
            diffs.truncate(max);
            return Ok(Outcome::Diverged(t as u32, diffs));
        }
        // the judge's visit (after each step's frame: none before the first step)
        if t > 0 {
            chests.visit(&mut w);
        }
    }
    Ok(Outcome::Ok(frames.len().saturating_sub(1) as u32))
}

/// A judge stack as a redsim one (a box's contents registered as its payload).
fn rstack(w: &mut World, s: &JStack) -> Result<Stack> {
    let item = container::item_id(&s.id).with_context(|| format!("item {} is not a redsim item", s.id))?;
    let mut contents = Vec::new();
    for (i, x) in s.contents.iter().enumerate() {
        if !x.contents.is_empty() {
            bail!("a box inside a box");
        }
        contents.push((i, Stack::new(container::item_id(&x.id).with_context(|| format!("item {}", x.id))?, x.count as u8)));
    }
    if s.name.is_some() {
        bail!("a named box is not modeled by redsim");
    }
    Ok(Stack { item, count: s.count as u8, payload: container::new_payload(w, &contents) })
}

/// Every trace: one line each (OK, the first divergence, UNMODELED); an error when any is not OK.
pub fn compare(traces: &[PathBuf], max: usize) -> Result<()> {
    let mut bad = 0;
    for t in traces {
        let name = t.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        match compare_one(t, max) {
            Ok(Outcome::Ok(n)) => println!("OK {name}: {n} ticks identical"),
            Ok(Outcome::Diverged(at, d)) => {
                bad += 1;
                println!("DIVERGED {name} at t={at}: {}", d.join("; "));
            }
            Ok(Outcome::Unmodeled(why)) => {
                bad += 1;
                println!("UNMODELED {name}: {why}");
            }
            Err(e) => {
                bad += 1;
                println!("ERROR {name}: {e:#}");
            }
        }
    }
    if bad > 0 {
        bail!("{bad} of {} traces not identical", traces.len());
    }
    Ok(())
}
