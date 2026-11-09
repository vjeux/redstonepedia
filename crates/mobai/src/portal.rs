//! `mctas mobai portal-probe`: nether portal travel on the official 26.3 server (the bridge has no
//! Nether), each case's mob counts per tick in the overworld and in the Nether after its summon,
//! beside redsim's model of the same case (`redsim::entity::portal`): which tick the mob leaves.
//! A case: a 2-wide, 3-tall portal (axis z) in an obsidian frame on a stone floor, one mob (or a
//! minecart with a villager) summoned at a position, optional NBT.

use anyhow::{Result, anyhow};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::World;

/// A probe case: name, entity id, position, NBT, the selector of the mob to count.
struct Case {
    name: &'static str,
    id: &'static str,
    pos: [f64; 3],
    nbt: &'static str,
    count: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        // the golem's box (1.4 wide) meets the portal cells at x 24 by 0.1
        Case { name: "touching", id: "minecraft:iron_golem", pos: [23.4, 64.0, 21.5], nbt: "{PersistenceRequired:1b}", count: "minecraft:iron_golem" },
        // its box ends at 23.9: no contact
        Case { name: "beside", id: "minecraft:iron_golem", pos: [23.2, 64.0, 21.5], nbt: "{PersistenceRequired:1b}", count: "minecraft:iron_golem" },
        // 2e-5 in: the box deflated by 1e-5 still meets the cell
        Case { name: "graze-in", id: "minecraft:iron_golem", pos: [23.30002, 64.0, 21.5], nbt: "{PersistenceRequired:1b}", count: "minecraft:iron_golem" },
        // 5e-6 in: deflated, it does not
        Case { name: "graze-out", id: "minecraft:iron_golem", pos: [23.300005, 64.0, 21.5], nbt: "{PersistenceRequired:1b}", count: "minecraft:iron_golem" },
        // on its portal cooldown: touching restarts it (300), it never leaves while it touches
        Case { name: "cooldown", id: "minecraft:iron_golem", pos: [23.4, 64.0, 21.5], nbt: "{PersistenceRequired:1b,PortalCooldown:5}", count: "minecraft:iron_golem" },
        // a villager in a minecart whose box (0.98) meets the portal: the cart and its rider
        Case { name: "cart", id: "minecraft:minecart", pos: [23.6, 64.0, 21.5], nbt: "{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b}]}", count: "minecraft:villager" },
    ]
}

const BLOCKS: &[([i32; 3], &str)] = &[];

/// The portal room's blocks (relative to the case's origin).
fn room() -> Vec<([i32; 3], String)> {
    let mut b = Vec::new();
    for x in 14..=34 {
        for z in 12..=32 {
            b.push(([x, 63, z], "minecraft:stone".to_string()));
        }
    }
    for z in 20..=23 {
        b.push(([24, 63, z], "minecraft:obsidian".to_string()));
        b.push(([24, 67, z], "minecraft:obsidian".to_string()));
    }
    for y in 64..=66 {
        b.push(([24, y, 20], "minecraft:obsidian".to_string()));
        b.push(([24, y, 23], "minecraft:obsidian".to_string()));
        for z in 21..=22 {
            b.push(([24, y, z], "minecraft:nether_portal[axis=z]".to_string()));
        }
    }
    let _ = BLOCKS;
    b
}

/// The game's side of a case at origin `o` (x offset): the overworld and Nether counts after
/// each of `ticks` ticks.
fn game_case(srv: &mut VanillaServer, c: &Case, o: i32, ticks: u32) -> Result<Vec<(u32, u32)>> {
    srv.cmd_ok(&format!("forceload add {} 0 {} 47", o, o + 47))?;
    for (p, s) in room() {
        srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0] + o, p[1], p[2]))?;
    }
    srv.cmd_ok(&format!("summon {} {} {} {} {}", c.id, c.pos[0] + o as f64, c.pos[1], c.pos[2], c.nbt))?;
    // a position filter keeps @e to the executing dimension (`worldLimited`)
    let sel = |dim: &str| format!("execute in {dim} positioned 0 0 0 if entity @e[type={},tag=!seen,distance=0..]", c.count);
    let mut out = Vec::new();
    for _ in 0..ticks {
        srv.step(1)?;
        let ow = count(srv, &sel("minecraft:overworld"))?;
        let ne = count(srv, &sel("minecraft:the_nether"))?;
        out.push((ow, ne));
    }
    // tag what this case made, so the next case counts only its own
    srv.cmd(&format!("execute in minecraft:overworld run tag @e[type={}] add seen", c.count))?;
    srv.cmd(&format!("execute in minecraft:the_nether run tag @e[type={}] add seen", c.count))?;
    Ok(out)
}

fn count(srv: &mut VanillaServer, cmd: &str) -> Result<u32> {
    let r = srv.cmd(cmd)?;
    // "Test passed, count: N" / "Test failed"
    Ok(r.rsplit(':').next().and_then(|s| s.trim().parse().ok()).unwrap_or(0))
}

/// redsim's side of a case: the overworld count after each tick (the Nether is not modeled: a mob
/// that left counts there).
fn redsim_case(c: &Case, ticks: u32) -> Result<Vec<(u32, u32)>> {
    let mut w = World::new(BlockPos::new(0, -64, 0), [48, 384, 48]);
    w.mob.seed(1, 6000);
    w.mob.ai.on = true;
    for (p, s) in room() {
        let b = Block::parse(&s).or_else(|_| redsim::terrain::land::block_of(&s)).map_err(|e| anyhow!("{s}: {e}"))?;
        w.set_block(BlockPos::new(p[0], p[1], p[2]), b, redsim::world::flags::RAW);
    }
    redsim::entity::spawn(&mut w, c.id, c.pos, Some(c.nbt)).map_err(|e| anyhow!(e))?;
    let ty = c.count.trim_start_matches("minecraft:");
    let alive = |w: &World| w.entities.list.iter().filter(|e| !e.removed && matches!(&e.kind, redsim::entity::EntityKind::Mob(m) if m.ty.def().id == ty)).count() as u32;
    let total = alive(&w);
    let mut out = Vec::new();
    for _ in 0..ticks {
        w.tick();
        let ow = alive(&w);
        out.push((ow, total - ow));
    }
    if !w.errors.is_empty() {
        eprintln!("  redsim model errors: {:?}", w.errors);
    }
    Ok(out)
}

pub fn run(version: &str, ticks: u32) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    for (k, c) in cases().iter().enumerate() {
        let o = (k as i32) * 64;
        let g = game_case(&mut srv, c, o, ticks)?;
        let r = redsim_case(c, ticks)?;
        let fmt = |v: &[(u32, u32)]| v.iter().map(|(a, b)| format!("{a}/{b}")).collect::<Vec<_>>().join(" ");
        let same = g == r;
        println!("{} {}: game  {}", if same { "SAME" } else { "DIFF" }, c.name, fmt(&g));
        println!("{} {}: redsim {}", if same { "    " } else { "    " }, c.name, fmt(&r));
    }
    Ok(())
}
