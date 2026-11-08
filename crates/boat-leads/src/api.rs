//! Positive/negative API controls against the official-code bridge (no player or flight claim).
use anyhow::{Result, ensure};
use mctas::design::EntitySpec;
use mctas::sim::{SandboxOptions, Worker, WorkerConfig};
use redsim::{BlockPos, World, entity};
use serde_json::json;

fn spec(kind: &str, x: f64) -> EntitySpec {
    EntitySpec { kind: kind.into(), pos: [x, 5.0, 0.0],
        nbt: (kind == "minecraft:item").then(|| "{Item:{id:\"minecraft:stone\",count:1}}".into()) }
}

pub fn run(version: &str) -> Result<()> {
    let mut game = Worker::spawn(&WorkerConfig { version: version.into(), ..Default::default() })?;
    let sb = game.create(&SandboxOptions::default())?;
    let mut sim = World::new(BlockPos::new(-16, 0, -16), [32, 32, 32]);
    let anchor = spec("minecraft:oak_boat", 0.0);
    let a = game.summon(sb, &[anchor.clone()])?.remove(0);
    let ai = entity::spawn(&mut sim, &anchor.kind, anchor.pos, None).map_err(anyhow::Error::msg)?;
    game.call(json!({"op":"leash", "sandbox":sb, "entity":a, "holder":null}))?;
    entity::leash::remove(&mut sim, ai).map_err(anyhow::Error::msg)?;
    println!("PASS API: live unlinked boat remove is a no-op in both");
    for kind in ["minecraft:armor_stand", "minecraft:minecart", "minecraft:item"] {
        let s = spec(kind, 2.0);
        let id = game.summon(sb, &[s.clone()])?.remove(0);
        let si = entity::spawn(&mut sim, &s.kind, s.pos, s.nbt.as_deref()).map_err(anyhow::Error::msg)?;
        ensure!(game.call(json!({"op":"leash", "sandbox":sb, "entity":id, "holder":null})).is_err(), "game accepted remove on {kind}");
        ensure!(entity::leash::remove(&mut sim, si).is_err(), "model accepted remove on {kind}");
        ensure!(game.call(json!({"op":"leash", "sandbox":sb, "entity":id, "holder":a})).is_err(), "game accepted attach on {kind}");
        ensure!(entity::leash::attach(&mut sim, si, ai).is_err(), "model accepted attach on {kind}");
        println!("PASS API: {kind} rejects attach and remove in both");
    }
    let b = spec("minecraft:oak_boat", 7.0);
    let bh = game.summon(sb, &[b.clone()])?.remove(0);
    let bi = entity::spawn(&mut sim, &b.kind, b.pos, None).map_err(anyhow::Error::msg)?;
    game.call(json!({"op":"leash", "sandbox":sb, "entity":a, "holder":bh}))?;
    entity::leash::attach(&mut sim, ai, bi).map_err(anyhow::Error::msg)?;
    game.exec(sb, &format!("kill {a}"))?;
    let drops = game.entities(sb)?.iter().filter(|e| e.kind == "minecraft:item" && e.state.contains("minecraft:lead")).count();
    ensure!(drops == 1, "game kill should drop exactly one lead, got {drops}");
    let error = entity::kill(&mut sim, ai).expect_err("model must refuse unsupported lead item motion");
    ensure!(error.contains("unseeded lead ItemEntity"), "unexpected model refusal: {error}");
    println!("PASS API: game kill dropped {drops} lead; model explicitly refuses its unseeded motion");
    game.call(json!({"op":"leash", "sandbox":sb, "entity":bh, "holder":null}))?;
    println!("PASS API: valid no-op still succeeds after the negative controls");
    game.drop_sandbox(sb)?;
    Ok(())
}
