//! The judge rig on the official dedicated server (the `mctas::vanilla` harness: the
//! unmodified 26.3 server jar, RCON, a bot player clicking the lever).
//!
//! The point of this run is the *real-game* demonstration that the rig geometry suppresses
//! when the count can reach the cap: `max-chained-neighbor-updates` is a stock
//! `server.properties` knob (`DedicatedServerProperties`), so the same official server is
//! booted with it lowered to a value the rig's small field reaches (6,580 counted calls in
//! redsim). At the shipped default (1,000,000) the identical script shows the target
//! reacting - run with `--cap 1000000` for that side. Blocks are placed with vanilla
//! `fill`/`setblock` commands (a stand-in for hand placement, as in the bedrock study);
//! the lever flip and the timing are the bot's real click between stepped ticks.

use anyhow::{Context, Result};
use mctas::sim::Pos;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::pos::{BlockPos, Dir};
use std::time::Instant;

fn p3(p: BlockPos, at: BlockPos) -> Pos {
    [p.x + at.x, p.y + at.y, p.z + at.z]
}

fn set(srv: &mut VanillaServer, p: Pos, state: &str) -> Result<()> {
    let out = srv.cmd(&format!("setblock {} {} {} {}", p[0], p[1], p[2], state))?;
    anyhow::ensure!(!out.contains("Could not"), "setblock {p:?} {state} -> {out}");
    Ok(())
}

fn fill(srv: &mut VanillaServer, a: Pos, b: Pos, state: &str) -> Result<()> {
    let out = srv.cmd(&format!("fill {} {} {} {} {} {} {}", a[0], a[1], a[2], b[0], b[1], b[2], state))?;
    anyhow::ensure!(!out.contains("Too many"), "fill {a:?}..{b:?} {state} -> {out}");
    Ok(())
}

fn flip(srv: &mut VanillaServer, lever: Pos, on: bool) -> Result<()> {
    srv.bot_use(lever, 1)?;
    let t0 = Instant::now();
    loop {
        srv.step(1)?;
        if srv.test_block(lever, &format!("minecraft:lever[powered={on}]"))? {
            return Ok(());
        }
        anyhow::ensure!(t0.elapsed().as_secs() < 20, "bot click did not flip the lever at {lever:?}");
    }
}

pub struct VanillaVerdict {
    pub cap: u32,
    pub control: bool,
    pub middle_flipped: bool,
    pub target_on_edge: bool,
    pub target_on_wake: bool,
}

fn run_one(srv: &mut VanillaServer, at: BlockPos, field_n: i32, control: bool) -> Result<(bool, bool, bool)> {
    let r = crate::judge::rig(field_n, !control, Dir::East, false);
    let h = field_n / 2;
    // Clear, then field (or head wire only for the control).
    fill(srv, p3(BlockPos::new(-h - 2, 0, -h - 2), at), p3(BlockPos::new(h + 4, 7, h + 2), at), "minecraft:air")?;
    if control {
        set(srv, p3(BlockPos::new(0, 0, 0), at), "minecraft:stone")?;
        set(srv, p3(BlockPos::new(0, 1, 0), at), &redsim::redstone::wire::CROSS.to_string())?;
    } else {
        fill(srv, p3(BlockPos::new(-h, 0, -h), at), p3(BlockPos::new(h, 0, h), at), "minecraft:stone")?;
        fill(srv, p3(BlockPos::new(-h, 1, -h), at), p3(BlockPos::new(h, 1, h), at), &redsim::redstone::wire::CROSS.to_string())?;
    }
    // The rig blocks (skip the field cells, already filled).
    for (p, b) in &r.blocks {
        if p.y <= 1 {
            continue;
        }
        set(srv, p3(*p, at), &b.to_string())?;
    }
    srv.step(5)?;
    let (lever, middle, target, wake) = (p3(r.lever, at), p3(r.middle, at), p3(r.target, at), p3(r.wake, at));
    anyhow::ensure!(srv.test_block(target, "minecraft:piston[extended=false]")?, "target not retracted at rest");
    set(srv, p3(r.redstone, at), "minecraft:redstone_block")?;
    srv.step(3)?;
    anyhow::ensure!(srv.test_block(target, "minecraft:piston[extended=false]")?, "quasi source placement woke the target");
    // Bot platform + stance, then the real click.
    set(srv, [lever[0], lever[1] - 1, lever[2] + 3], "minecraft:stone")?;
    srv.teleport_bot(lever[0] as f64 + 0.5, lever[1] as f64, lever[2] as f64 + 3.5)?;
    srv.step(2)?;
    flip(srv, lever, true).context("lever on")?;
    srv.step(10)?;
    let middle_flipped = srv.test_block(middle, "minecraft:note_block[powered=true]")?;
    let target_on_edge = srv.test_block(target, "minecraft:piston[extended=true]")?;
    set(srv, wake, "minecraft:stone")?;
    srv.step(10)?;
    let target_on_wake = srv.test_block(target, "minecraft:piston[extended=true]")?;
    Ok((middle_flipped, target_on_edge, target_on_wake))
}

pub fn run_vanilla(version: &str, cap: u32, field_n: i32) -> Result<Vec<VanillaVerdict>> {
    let opts = VanillaOptions {
        version: version.to_string(),
        extra_properties: vec![("max-chained-neighbor-updates".into(), cap.to_string())],
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    let at = BlockPos::new(0, 64, 0);
    let mut out = Vec::new();
    for control in [true, false] {
        let (m, e, wk) = run_one(&mut srv, at, field_n, control)?;
        out.push(VanillaVerdict { cap, control, middle_flipped: m, target_on_edge: e, target_on_wake: wk });
    }
    Ok(out)
}
