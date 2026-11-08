use crate::scenario::Scenario;
use anyhow::{Context, Result, bail, ensure};
use circuit::rig::{GameRig, Recorder, Rig};
use mctas::sim::{PlaceMode, SandboxOptions, Worker};
use redsim_fuzz::trace_compare::{self, Outcome};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// One official-code sandbox, with all setup recorded and no change to boat/leash physics.
pub fn record(w: &mut Worker, scene: &Scenario, out: &Path) -> Result<PathBuf> {
    scene.validate()?;
    std::fs::create_dir_all(out)?;
    let design = out.join(format!("{}-fixture.txt", scene.name));
    let origin = scene.origin;
    // A passive floor cell keeps the legacy Clock parser's nonempty-design contract;
    // it is already present in the exact raw floor and is not an extra game block.
    std::fs::write(&design, format!("name: boat-lead-fixture\norigin: {} {} {}\npalette:\n  S minecraft:stone\nlayer 0:\n  S\n", origin[0], origin[1], origin[2]))?;
    let from = [origin[0] - 16, origin[1] - 2, origin[2] - 16];
    let to = [origin[0] + 15, origin[1] + 16, origin[2] + 15];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let result = record_in(w, sb, scene, out, &design, from, to, chunks);
    let cleanup = w.drop_sandbox(sb);
    let path = result?;
    cleanup?;
    Ok(path)
}

fn record_in(w: &mut Worker, sb: u32, scene: &Scenario, out: &Path, design: &Path, from: [i32; 3], to: [i32; 3], chunks: [i32; 4]) -> Result<PathBuf> {
    let y = scene.origin[1];
    let state = format!("minecraft:{}", scene.floor);
    let mut raw: Vec<Value> = Vec::new();
    for z in from[2]..=to[2] {
        for x in from[0]..=to[0] { raw.push(json!([x, y, z, state])); }
    }
    w.fill(sb, [from[0], y, from[2]], [to[0], y, to[2]], &state, PlaceMode::Raw)?;
    let order = w.ticking_chunks(sb)?;
    let mut rig = GameRig { w, sb, rec: Some(Recorder::new(from, to)) };
    let entities = rig.summon(&scene.specs())?;
    for [a, b] in &scene.links { rig.leash(&entities[*a], Some(&entities[*b]))?; }
    rig.rec.as_mut().unwrap().frame(rig.w, sb)?;
    for tick in 1..=scene.ticks {
        for e in scene.events.iter().filter(|e| e.tick == tick) {
            rig.leash(&entities[e.entity], e.holder.map(|h| entities[h].as_str()))?;
        }
        rig.step(1)?;
    }
    let path = out.join(format!("{}.trace", scene.name));
    rig.rec.as_ref().unwrap().write(&path, json!({
        "trace": 1, "name": scene.name, "design": design, "period": 0, "warmup": 0,
        "box": [from,to], "chunks": chunks, "seed": 0, "random_tick_speed": 3, "chunk_order": order, "raw": raw,
        "evidence": "official server code, direct fixture attachment; not survival assembly or a flight claim"
    }))?;
    std::fs::write(out.join(format!("{}.json", scene.name)), serde_json::to_string_pretty(scene)?)?;
    Ok(path)
}

/// Strict exact comparison and a compact trajectory report; a frozen entity is a failure.
pub fn compare(path: &Path) -> Result<bool> {
    let trace = trace_compare::load_trace(path)?;
    let outcome = trace_compare::compare(&trace, 8)?;
    let ok = match outcome {
        Outcome::Same { ticks } => { println!("EXACT {}: {ticks} frames, positions/motion/rotations/leashes/blocks/random agree", trace.name); true }
        Outcome::Diverges { tick, diffs, more } => {
            println!("DIVERGES {} at tick {tick}: {} (+{more})", trace.name, diffs.join("; ")); false
        }
        Outcome::Unmodeled(why) => { println!("UNMODELED {}: {why}", trace.name); false }
    };
    let chunks = trace.chunks.context("missing chunks")?;
    if trace.name == "head-on-collision" {
        let closest = trace.frames.iter().filter_map(|(_, e)| (e.len() >= 2).then(|| (e[0].pos[0] - e[1].pos[0]).abs())).fold(f64::INFINITY, f64::min);
        println!("CONTACT {}: closest x separation {closest}", trace.name);
        ensure!(closest <= redsim::entity::boat::WIDTH as f64 + 1e-12, "head-on fixture never actually contacted");
    }
    for (t, (_, entities)) in trace.frames.iter().enumerate() {
        for e in entities {
            let (cx, cz) = ((e.pos[0].floor() as i32).div_euclid(16), (e.pos[2].floor() as i32).div_euclid(16));
            ensure!((chunks[0]..=chunks[2]).contains(&cx) && (chunks[1]..=chunks[3]).contains(&cz), "{} entity left loaded chunks at {t}", trace.name);
        }
    }
    if let (Some(first), Some(last)) = (trace.frames.first(), trace.frames.last()) {
        let initial: Vec<_> = first.1.iter().map(|e| e.pos[1]).collect();
        let final_y: Vec<_> = last.1.iter().map(|e| e.pos[1]).collect();
        let intact = last.1.iter().filter(|e| e.leash_holder.is_some()).count();
        println!("TRAJECTORY {}: y {:?} -> {:?}, intact leads {intact} (not a sustained-flight verdict)", trace.name, initial, final_y);
    }
    Ok(ok)
}

/// Save the pristine binary's raw report. The negative is a real previous-model refusal,
/// never a transformed snapshot or a different simulated motion fed into the new model.
pub fn baseline(binary: &Path, paths: &[PathBuf], out: &Path) -> Result<()> {
    let result = std::process::Command::new(binary).arg("trace-compare").args(paths).output().with_context(|| format!("run {}", binary.display()))?;
    let text = format!("status: {}\nstdout:\n{}\nstderr:\n{}", result.status, String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    std::fs::write(out.join("pristine-baseline.txt"), &text)?;
    println!("PRISTINE BASELINE:\n{text}");
    ensure!(text.contains("UNMODELED") || text.contains("DIVERGES"), "expected pristine model to expose missing leash/push support");
    Ok(())
}

pub fn require_all(paths: &[PathBuf]) -> Result<()> {
    let mut failed = 0;
    for path in paths { if !compare(path)? { failed += 1; } }
    if failed != 0 { bail!("{failed}/{} exact fixture comparisons failed", paths.len()); }
    println!("CALIBRATION: {}/{} exact fixture comparisons passed", paths.len(), paths.len());
    Ok(())
}
