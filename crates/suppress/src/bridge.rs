//! The judge rig in the real game (the bridge sandbox: the unmodified 26.3 server classes).
//! Same script as `judge::judge_one` at the game's own cap: place the rig minus the quasi
//! source like the door judge places designs (raw writes, wire shapes, a self update of
//! every block), settle, place the redstone block, flip the lever (timed: the lever-pull
//! cascades run inside the `use` call), read the middle and target blocks back, wake.

use anyhow::{Context, Result};
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker, WorkerConfig};
use redsim::block::Block;
use redsim::pos::BlockPos;
use std::time::Instant;

fn pos(p: BlockPos, at: BlockPos) -> Pos {
    [p.x + at.x, p.y + at.y, p.z + at.z]
}

fn block_at(w: &mut Worker, sb: u32, p: Pos) -> Result<String> {
    let r = w.get_region(sb, p, p)?;
    Ok(r.get(p).unwrap_or("?").to_string())
}

pub struct BridgeVerdict {
    pub name: String,
    pub middle_flipped: bool,
    pub target_on_edge: bool,
    pub target_on_wake: bool,
    pub lever_ms: u128,
    pub problems: Vec<String>,
}

pub fn bridge_one(w: &mut Worker, sb: u32, name: &str, field_n: i32, field: bool, at: BlockPos, arm: redsim::pos::Dir, mirror: bool) -> Result<BridgeVerdict> {
    let r = crate::judge::rig(field_n, field, arm, mirror);
    let mut problems = Vec::new();
    w.clear(sb)?;
    let blocks: Vec<(Pos, String)> = r.blocks.iter().map(|(p, b)| (pos(*p, at), b.to_string())).collect();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    w.step_until_idle(sb, 400)?;
    let (lever, middle, target, wake, rb) = (pos(r.lever, at), pos(r.middle, at), pos(r.target, at), pos(r.wake, at), pos(r.redstone, at));
    let st = block_at(w, sb, target)?;
    if !st.contains("extended=false") {
        problems.push(format!("target not retracted at rest: {st}"));
    }
    w.set_blocks(sb, &[(rb, "minecraft:redstone_block".to_string())], PlaceMode::Update)?;
    w.step(sb, 3)?;
    let st = block_at(w, sb, target)?;
    if !st.contains("extended=false") {
        problems.push(format!("placing the quasi source woke the target: {st}"));
    }
    let t0 = Instant::now();
    w.use_block(sb, lever)?;
    let lever_ms = t0.elapsed().as_millis();
    w.step(sb, 10)?;
    w.step_until_idle(sb, 400)?;
    let middle_st = block_at(w, sb, middle)?;
    let middle_flipped = middle_st.contains("powered=true");
    if !middle_flipped {
        problems.push(format!("middle did not flip: {middle_st}"));
    }
    let target_on_edge = block_at(w, sb, target)?.contains("extended=true");
    w.set_blocks(sb, &[(wake, "minecraft:stone".to_string())], PlaceMode::Update)?;
    w.step(sb, 10)?;
    w.step_until_idle(sb, 400)?;
    let target_on_wake = block_at(w, sb, target)?.contains("extended=true");
    if !target_on_wake {
        problems.push("target did not react to the wake update".into());
    }
    Ok(BridgeVerdict { name: name.to_string(), middle_flipped, target_on_edge, target_on_wake, lever_ms, problems })
}

pub fn run_bridge(version: &str, field_n: i32, arms: &[redsim::pos::Dir]) -> Result<Vec<BridgeVerdict>> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let at = BlockPos::new(0, 64, 0);
    let mut out = Vec::new();
    for &arm in arms {
        for (field, tag) in [(false, "control"), (true, "flood")] {
            let name = format!("bridge {tag} arm={} field={field_n}", arm.name());
            out.push(bridge_one(&mut w, sb, &name, field_n, field, at, arm, false).with_context(|| name.clone())?);
        }
    }
    Ok(out)
}
