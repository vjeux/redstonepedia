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

/// An entity's `Pos` (or the first number of another list) from `data get entity`, `None` when
/// the selector found nothing (an unloaded chunk's entity too).
fn data_list(srv: &mut VanillaServer, cmd: &str) -> Result<Option<Vec<f64>>> {
    let out = srv.cmd_long(cmd)?;
    Ok(out.split("has the following entity data: ").nth(1).map(|v| v.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(|c| c.trim().trim_end_matches(['d', 'f', 's', 'b', 'L']).parse().ok()).collect()))
}

/// `mctas mobai portal-probe --nether`: the portal pad's Nether side on the official server (the
/// bridge has none), printed as it happens: (A) a golem through the overworld room's portal into a
/// Nether portal built at the linked spot (x / 8, z / 8; the Nether's chunks released after the
/// build, so only the crossing's PORTAL ticket keeps them): its arrival position and PortalCooldown,
/// then its position every 20 ticks until it is no longer found (its chunk no longer loaded) or
/// `ticks` pass; (B) an iron ingot summoned into the Nether portal's cells inside that window: where
/// it comes out in the overworld and when; (C) a second golem through, onto lava at its feet beside
/// the Nether portal: its health each 10 ticks, its death, the drops.
pub fn nether(version: &str, ticks: u32) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    srv.cmd_ok("forceload add 0 0 47 47")?;
    for (p, s) in room() {
        srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0], p[1], p[2]))?;
    }
    // (a position filter keeps @e to the executing dimension: `worldLimited`)
    let n = |c: &str| format!("execute in minecraft:the_nether positioned 0 0 0 run {c}");
    // the Nether side: an obsidian floor and air above, a portal at x 3, z 2..3, y 64..66 (axis z,
    // the overworld portal's x 24, z 21..22 over 8), a lava pool at x -2..-1 for case C
    srv.cmd_ok(&n("forceload add -16 -16 15 15"))?;
    srv.cmd_ok(&n("fill -8 60 -8 12 72 12 minecraft:air"))?;
    srv.cmd_ok(&n("fill -8 63 -8 12 63 12 minecraft:obsidian"))?;
    for z in 1..=4 {
        // (the floor's obsidian already: a setblock that changes nothing reports a failure)
        srv.cmd(&n(&format!("setblock 3 63 {z} minecraft:obsidian")))?;
        srv.cmd_ok(&n(&format!("setblock 3 67 {z} minecraft:obsidian")))?;
    }
    for y in 64..=66 {
        srv.cmd_ok(&n(&format!("setblock 3 {y} 1 minecraft:obsidian")))?;
        srv.cmd_ok(&n(&format!("setblock 3 {y} 4 minecraft:obsidian")))?;
        for z in 2..=3 {
            srv.cmd_ok(&n(&format!("setblock 3 {y} {z} minecraft:nether_portal[axis=z]")))?;
        }
    }
    srv.step(5)?;
    srv.cmd_ok(&n("forceload remove -16 -16 15 15"))?;
    srv.step(100)?;
    let probe = |t: &str| format!("@e[type=minecraft:iron_golem,tag={t},limit=1,distance=0..]");
    // (A) the golem through
    srv.cmd_ok("summon minecraft:iron_golem 23.4 64 21.5 {PersistenceRequired:1b,Tags:[\"probeA\"]}")?;
    let t0 = srv.game_time()?;
    let mut arrived = None;
    let mut last: Option<Vec<f64>> = None;
    for k in 1..=ticks {
        srv.step(1)?;
        let pos = data_list(&mut srv, &n(&format!("data get entity {} Pos", probe("probeA"))))?;
        if arrived.is_none() && pos.is_some() {
            let cd = data_list(&mut srv, &n(&format!("data get entity {} PortalCooldown", probe("probeA"))))?;
            println!("A: in the Nether after {k} ticks (game time {}): pos {:?}, PortalCooldown {:?}", t0 + k as i64, pos, cd);
            println!("A: redsim {:?}", redsim_arrival(23.4, 21.5)?);
            arrived = Some(k);
            // (B) inside the ticket's window: an ingot into the Nether portal
            srv.cmd_ok(&n("summon minecraft:item 3.5 64.5 2.5 {Item:{id:\"minecraft:iron_ingot\",count:1},Tags:[\"probeB\"]}"))?;
            println!("B: redsim {:?}", redsim_item_back(0)?);
            println!("B2: redsim {:?}", redsim_item_back(60)?);
        }
        // (B2) the same 60 ticks later, the Nether's chunks long loaded
        if arrived.is_some_and(|a| k == a + 60) {
            srv.cmd_ok(&n("summon minecraft:item 3.5 64.5 2.5 {Item:{id:\"minecraft:iron_ingot\",count:1},Tags:[\"probeB2\"]}"))?;
        }
        if arrived.is_some_and(|a| k > a + 60 && k <= a + 90) {
            let ingot = data_list(&mut srv, "execute in minecraft:overworld positioned 0 0 0 run data get entity @e[type=minecraft:item,tag=probeB2,limit=1,distance=0..] Pos")?;
            if let Some(p) = ingot {
                println!("B2: the ingot in the overworld {} ticks after its summon: {p:?}", k - arrived.unwrap_or(0) - 60);
            }
        }
        if arrived.is_some() {
            let ingot = data_list(&mut srv, "execute in minecraft:overworld positioned 0 0 0 run data get entity @e[type=minecraft:item,tag=probeB,limit=1,distance=0..] Pos")?;
            if let Some(p) = ingot
                && k <= arrived.unwrap_or(0) + 40
            {
                println!("B: the ingot in the overworld {} ticks after its summon: {p:?}", k - arrived.unwrap_or(0));
            }
        }
        if k % 20 == 0 || (pos.is_none() && last.is_some()) {
            println!("A: t+{k}: {pos:?}");
        }
        if pos.is_none() && last.is_some() {
            println!("A: the golem is no longer found {k} ticks after its summon ({} after its arrival)", k - arrived.unwrap_or(0));
            break;
        }
        last = pos;
    }
    // (A2) the arrival position for other entry points, each golem through on its own: both sides
    // of the plane at farmopt's trench offsets (±1.0 from the plane's centre x 24.5: its box 0.2
    // into the portal cells), each near an end of the portal (the frame stops a box from reaching
    // the end cells diagonally), the middle; then (A5) through a second overworld portal 3 blocks
    // from the first (x 27, the same z): the linking (both map within the Nether's 16-block search)
    srv.cmd_ok("fill 27 63 20 27 67 23 minecraft:obsidian")?;
    srv.cmd_ok("fill 27 64 21 27 66 22 minecraft:nether_portal[axis=z]")?;
    for (tag, x, z) in [("probeA2", 23.5, 21.7), ("probeA3", 25.5, 21.7), ("probeA4", 23.5, 22.3), ("probeA5", 25.5, 22.3), ("probeA6", 25.5, 22.0), ("probeA7", 26.5, 21.9)] {
        srv.cmd_ok(&format!("summon minecraft:iron_golem {x} 64 {z} {{PersistenceRequired:1b,Tags:[\"{tag}\"]}}"))?;
        for k in 1..=6 {
            srv.step(1)?;
            if let Some(p) = data_list(&mut srv, &format!("execute in minecraft:overworld positioned 0 0 0 run data get entity {} Pos", probe(tag)))? {
                println!("{tag}: in the overworld after {k} ticks at {p:?}");
            }
            if let Some(p) = data_list(&mut srv, &n(&format!("data get entity {} Pos", probe(tag))))? {
                println!("{tag}: from ({x}, 64, {z}) in the Nether after {k} ticks at {p:?}");
                println!("{tag}: redsim {:?}", redsim_arrival(x, z)?);
                break;
            }
        }
    }
    // (C) lava: a golem standing in a lava source 2 deep, in each dimension (the Nether's area
    // forceloaded for this case): its health every 20 ticks until it dies
    srv.cmd_ok(&n("forceload add -16 -16 15 15"))?;
    for (dim, run) in [("overworld", "execute in minecraft:overworld positioned 0 0 0 run"), ("the_nether", "execute in minecraft:the_nether positioned 0 0 0 run")] {
        let (bx, bz) = if dim == "overworld" { (10, 40) } else { (-6, 8) };
        srv.cmd_ok(&format!("{run} fill {} 61 {} {} 62 {} minecraft:stone", bx - 2, bz - 2, bx + 2, bz + 2))?;
        srv.cmd(&format!("{run} fill {} 63 {} {} 66 {} minecraft:air", bx - 2, bz - 2, bx + 2, bz + 2))?;
        for y in 63..=64 {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    let edge = dx.abs() == 2 || dz.abs() == 2;
                    srv.cmd(&format!("{run} setblock {} {y} {} {}", bx + dx, bz + dz, if edge { "minecraft:stone" } else { "minecraft:lava" }))?;
                }
            }
        }
        let tag = format!("lava_{dim}");
        srv.cmd_ok(&format!("{run} summon minecraft:iron_golem {}.5 63 {}.5 {{PersistenceRequired:1b,Tags:[\"{tag}\"]}}", bx, bz))?;
        let sel = format!("@e[type=minecraft:iron_golem,tag={tag},limit=1,distance=0..]");
        let mut line = Vec::new();
        for k in 1..=400u32 {
            srv.step(1)?;
            let hp = data_list(&mut srv, &format!("{run} data get entity {sel} Health"))?;
            match hp.and_then(|v| v.first().copied()) {
                Some(h) if k % 20 == 0 => line.push(format!("{k}:{h:.0}")),
                Some(_) => {}
                None => {
                    line.push(format!("gone at {k}"));
                    break;
                }
            }
            let dt = data_list(&mut srv, &format!("{run} data get entity {sel} DeathTime"))?;
            if dt.and_then(|v| v.first().copied()).is_some_and(|d| d > 0.0) {
                line.push(format!("dying at {k}"));
                break;
            }
        }
        println!("C {dim}: {}", line.join(" "));
    }
    Ok(())
}

/// redsim's side of cases B and B2: the arrival case's worlds (one portal each side), a golem
/// through first (its PORTAL ticket holds the Nether), `wait` ticks, then an iron ingot summoned
/// in the Nether portal's cells at (3.5, 64.5, 2.5): the ticks until it is in the overworld, and
/// where.
fn redsim_item_back(wait: u32) -> Result<Option<(u32, [f64; 3])>> {
    let mut w = arrival_worlds(false)?;
    redsim::entity::spawn(&mut w, "minecraft:iron_golem", [23.4, 64.0, 21.5], Some("{PersistenceRequired:1b}")).map_err(|e| anyhow!(e))?;
    for _ in 0..2 + wait {
        w.tick();
    }
    let n = w.nether.as_deref_mut().expect("the Nether");
    redsim::entity::spawn(n, "minecraft:item", [3.5, 64.5, 2.5], Some("{Item:{id:\"minecraft:iron_ingot\",count:1}}")).map_err(|e| anyhow!(e))?;
    for k in 1..=40 {
        w.tick();
        if let Some(e) = w.entities.list.iter().find(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Item(_))) {
            return Ok(Some((k, [e.pos.x, e.pos.y, e.pos.z])));
        }
    }
    if !w.errors.is_empty() {
        eprintln!("  redsim model errors: {:?}", w.errors);
    }
    Ok(None)
}

/// The arrival cases' worlds: the overworld room (with a second portal at x 27 when `second`), and
/// the probe's Nether build as its `World::nether`.
fn arrival_worlds(second: bool) -> Result<World> {
    let mut w = World::new(BlockPos::new(0, -64, 0), [48, 384, 48]);
    w.mob.seed(1, 6000);
    w.mob.ai.on = true;
    let put = |w: &mut World, p: [i32; 3], s: &str| -> Result<()> {
        let b = Block::parse(s).or_else(|_| redsim::terrain::land::block_of(s)).map_err(|e| anyhow!("{s}: {e}"))?;
        w.set_block(BlockPos::new(p[0], p[1], p[2]), b, redsim::world::flags::RAW);
        Ok(())
    };
    for (p, s) in room() {
        put(&mut w, p, &s)?;
    }
    for y in 63..=67 {
        for z in 20..=23 {
            if !second {
                break;
            }
            let inside = (64..=66).contains(&y) && (21..=22).contains(&z);
            put(&mut w, [27, y, z], if inside { "minecraft:nether_portal[axis=z]" } else { "minecraft:obsidian" })?;
        }
    }
    let mut n = World::new(BlockPos::new(-8, 0, -8), [21, 256, 21]);
    n.ultrawarm = true;
    n.mob.seed(2, 6000);
    n.mob.ai.on = true;
    for x in -8..=12 {
        for z in -8..=12 {
            put(&mut n, [x, 63, z], "minecraft:obsidian")?;
        }
    }
    for z in 1..=4 {
        put(&mut n, [3, 67, z], "minecraft:obsidian")?;
    }
    for y in 64..=66 {
        put(&mut n, [3, y, 1], "minecraft:obsidian")?;
        put(&mut n, [3, y, 4], "minecraft:obsidian")?;
        for z in 2..=3 {
            put(&mut n, [3, y, z], "minecraft:nether_portal[axis=z]")?;
        }
    }
    w.nether = Some(Box::new(n));
    Ok(w)
}

/// redsim's side of an arrival case: [`arrival_worlds`], a golem summoned at (x, 64, z): the tick
/// it is in the Nether, its position there and its PortalCooldown.
fn redsim_arrival(x: f64, z: f64) -> Result<Option<(u32, [f64; 3], i32)>> {
    let mut w = arrival_worlds(true)?;
    redsim::entity::spawn(&mut w, "minecraft:iron_golem", [x, 64.0, z], Some("{PersistenceRequired:1b}")).map_err(|e| anyhow!(e))?;
    for k in 1..=6 {
        w.tick();
        let n = w.nether.as_deref().expect("the Nether");
        if let Some(e) = n.entities.list.iter().find(|e| !e.removed) {
            if !w.errors.is_empty() {
                eprintln!("  redsim model errors: {:?}", w.errors);
            }
            return Ok(Some((k, [e.pos.x, e.pos.y, e.pos.z], e.portal_cooldown)));
        }
    }
    if !w.errors.is_empty() {
        eprintln!("  redsim model errors: {:?}", w.errors);
    }
    Ok(None)
}

/// Clear a design's box (its blocks' bounds, `margin` more on each side) to air in `dim`, in fills
/// of at most 32768 cells: the world's own terrain stays out of the design's air cells.
fn clear_box(srv: &mut VanillaServer, dim: &str, blocks: &[(mctas::sim::Pos, String)], margin: i32) -> Result<()> {
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, _) in blocks {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] - margin);
            hi[i] = hi[i].max(p[i] + margin);
        }
    }
    let run = format!("execute in {dim} run");
    for y in lo[1]..=hi[1] {
        let out = srv.cmd(&format!("{run} fill {} {y} {} {} {y} {} minecraft:air", lo[0], lo[2], hi[0], hi[2]))?;
        if !out.contains("Successfully filled") && !out.contains("No blocks were filled") {
            anyhow::bail!("clearing y {y} in {dim}: {out}");
        }
    }
    Ok(())
}

/// Paste a design's blocks at their own coordinates in `dim` (`setblock .. strict`: no updates,
/// so a portal's cells stand before its frame is whole), then give every fluid its update (air,
/// then the fluid again: its first flow tick scheduled).
fn paste(srv: &mut VanillaServer, dim: &str, blocks: &[(mctas::sim::Pos, String)]) -> Result<()> {
    let run = format!("execute in {dim} run");
    let fluid = |s: &str| s.starts_with("minecraft:lava") || s.starts_with("minecraft:water");
    for (p, s) in blocks.iter().filter(|(_, s)| !fluid(s)) {
        let out = srv.cmd(&format!("{run} setblock {} {} {} {s} strict", p[0], p[1], p[2]))?;
        if !out.contains("Changed the block") && !out.contains("Could not set the block") {
            anyhow::bail!("setblock {p:?} {s} in {dim}: {out}");
        }
    }
    // the fluids last, each a plain setblock over air (its onPlace schedules its first flow tick:
    // a strict setblock or a fill schedules none, `--fluid-test`)
    for (p, s) in blocks.iter().filter(|(_, s)| fluid(s)) {
        srv.cmd(&format!("{run} setblock {} {} {} minecraft:air strict", p[0], p[1], p[2]))?;
        srv.cmd_ok(&format!("{run} setblock {} {} {} {s}", p[0], p[1], p[2]))?;
    }
    Ok(())
}

/// `mctas mobai portal-probe --fluid-test`: does a lava source pasted by [`paste`] spread on the
/// official server, in each dimension (the cells beside it after 40 ticks)?
pub fn fluid_test(version: &str) -> Result<()> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), ..Default::default() })?;
    for dim in ["minecraft:overworld", "minecraft:the_nether"] {
        let run = format!("execute in {dim} positioned 0 0 0 run");
        srv.cmd_ok(&format!("{run} forceload add -32 -32 47 47"))?;
        // high above any terrain (over the Nether's roof); one source per chunk, by a plain
        // setblock (and one by the paste, one by a fill, in chunk 1 1)
        srv.cmd(&format!("{run} fill -24 199 -24 40 199 40 minecraft:obsidian"))?;
        srv.step(40)?;
        let spots: [(i32, i32); 6] = [(4, 4), (20, 4), (4, 20), (-12, -12), (20, 20), (36, 36)];
        for (x, z) in spots {
            srv.cmd_ok(&format!("{run} setblock {x} 200 {z} minecraft:lava[level=0]"))?;
        }
        paste(&mut srv, dim, &[([24, 200, 24], "minecraft:lava[level=0]".to_string())])?;
        srv.cmd_ok(&format!("{run} fill 28 200 20 28 200 20 minecraft:lava[level=0]"))?;
        let checks: Vec<(i32, i32)> = spots.iter().map(|&(x, z)| (x, z + 1)).chain([(24, 25), (28, 21)]).collect();
        let mut first = vec![None; checks.len()];
        for t in 1..=60u32 {
            srv.step(1)?;
            for (k, (x, z)) in checks.iter().enumerate() {
                if first[k].is_none() {
                    let out = srv.cmd(&format!("{run} execute if block {x} 200 {z} minecraft:lava"))?;
                    if out.contains("passed") {
                        first[k] = Some(t);
                    }
                }
            }
        }
        println!("{dim}: the first spread by plain setblock at chunks (0 0) (1 0) (0 1) (-1 -1) (1 1) (2 2), then the paste and the fill in (1 1): {first:?} ticks");
    }
    Ok(())
}

/// The chunk box [x0, z0, x1, z1] (block coordinates) of a design's blocks, with a margin.
fn chunk_box(blocks: &[(mctas::sim::Pos, String)], margin: i32) -> [i32; 4] {
    let (mut x0, mut z0, mut x1, mut z1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (p, _) in blocks {
        x0 = x0.min(p[0]);
        z0 = z0.min(p[2]);
        x1 = x1.max(p[0]);
        z1 = z1.max(p[2]);
    }
    [x0 - margin, z0 - margin, x1 + margin, z1 + margin]
}

/// `mctas mobai portal-probe --nether --overworld <design> --nether-design <design>`: two
/// designs, each pasted at its own coordinates (the overworld's chunks forceloaded; the Nether's
/// forceloaded while it is pasted and settles 200 ticks, then released, so only the crossings'
/// PORTAL tickets hold it), the overworld design's entities summoned (each tagged `np<k>`), then
/// `ticks` ticks: every `every` ticks and at each event (a dimension change, a health change, a
/// death) each entity's dimension, position, motion, health and PortalCooldown; and every
/// `every` ticks the iron ingots lying in the Nether (count) and in the Nether design's
/// containers.
pub fn nether_designs(version: &str, ow: &std::path::Path, ne: &std::path::Path, ticks: u32, every: u32, stagger: u32) -> Result<()> {
    let od = mctas::design::Design::load(ow)?;
    let nd = mctas::design::Design::load(ne)?;
    let ob = od.block_list();
    let nb = nd.block_list();
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "easy".into(), ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    let [x0, z0, x1, z1] = chunk_box(&ob, 8);
    srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
    let [a0, c0, a1, c1] = chunk_box(&nb, 8);
    let nrun = "execute in minecraft:the_nether positioned 0 0 0 run";
    srv.cmd_ok(&format!("{nrun} forceload add {a0} {c0} {a1} {c1}"))?;
    // the forced chunks loaded whole before anything is placed in them
    srv.step(40)?;
    clear_box(&mut srv, "minecraft:overworld", &ob, 4)?;
    paste(&mut srv, "minecraft:overworld", &ob)?;
    clear_box(&mut srv, "minecraft:the_nether", &nb, 4)?;
    paste(&mut srv, "minecraft:the_nether", &nb)?;
    srv.step(200)?;
    // the Nether's fluids after settling: each fluid cell of the design and its 4 neighbours
    let mut fluid_cells = Vec::new();
    for (p, s) in nb.iter().filter(|(_, s)| s.starts_with("minecraft:lava") || s.starts_with("minecraft:water")) {
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            let q = [p[0] + dx, p[1], p[2] + dz];
            let out = srv.cmd(&format!("{nrun} execute if block {} {} {} {}", q[0], q[1], q[2], s.split('[').next().unwrap_or(s)))?;
            fluid_cells.push(format!("{q:?}:{}", if out.contains("passed") { "yes" } else { "no" }));
        }
    }
    if !fluid_cells.is_empty() {
        println!("nether fluid after settling: {}", fluid_cells.join(" "));
    }
    srv.cmd_ok(&format!("{nrun} forceload remove {a0} {c0} {a1} {c1}"))?;
    srv.step(40)?;
    let containers: Vec<mctas::sim::Pos> = nb.iter().filter(|(_, s)| s.starts_with("minecraft:chest") || s.starts_with("minecraft:hopper") || s.starts_with("minecraft:barrel")).map(|(p, _)| *p).collect();
    // the overworld design's entities, tagged; entity k summoned `stagger` x k ticks in
    let mut tags = Vec::new();
    let mut cmds = Vec::new();
    for (k, e) in od.entities.iter().enumerate() {
        let tag = format!("np{k}");
        let nbt = match e.nbt.as_deref().map(str::trim) {
            Some(n) if n.len() > 2 => format!("{},Tags:[\"{tag}\"]}}", n.strip_suffix('}').unwrap_or(n)),
            _ => format!("{{Tags:[\"{tag}\"]}}"),
        };
        cmds.push(format!("summon {} {} {} {} {nbt}", e.kind, e.pos[0], e.pos[1], e.pos[2]));
        tags.push((tag, e.kind.clone()));
    }
    for (k, c) in cmds.iter().enumerate() {
        if k as u32 * stagger == 0 {
            srv.cmd_ok(c)?;
        }
    }
    let t0 = srv.game_time()?;
    println!("summoned {} entities at game time {t0} (every {stagger} ticks)", tags.len());
    let dims = [("overworld", "execute in minecraft:overworld positioned 0 0 0 run"), ("nether", nrun)];
    let mut last: Vec<(Option<&str>, f64)> = vec![(Some("overworld"), -1.0); tags.len()];
    for k in 1..=ticks {
        srv.step(1)?;
        for (j, c) in cmds.iter().enumerate() {
            if j > 0 && j as u32 * stagger == k {
                srv.cmd_ok(c)?;
            }
        }
        for (j, (tag, _)) in tags.iter().enumerate() {
            if j as u32 * stagger > k {
                continue;
            }
            let sel = format!("@e[tag={tag},limit=1,distance=0..]");
            let mut found = None;
            for (dn, run) in dims {
                if let Some(p) = data_list(&mut srv, &format!("{run} data get entity {sel} Pos"))? {
                    found = Some((dn, run, p));
                    break;
                }
            }
            let Some((dn, run, p)) = found else {
                if last[j].0.is_some() {
                    println!("t+{k} {tag}: gone (dead, or its chunk unloaded)");
                    last[j].0 = None;
                }
                continue;
            };
            let hp = data_list(&mut srv, &format!("{run} data get entity {sel} Health"))?.and_then(|v| v.first().copied()).unwrap_or(-1.0);
            let event = last[j].0 != Some(dn) || (hp - last[j].1).abs() > 1e-9;
            if event || k % every == 0 {
                let v = data_list(&mut srv, &format!("{run} data get entity {sel} Motion"))?.unwrap_or_default();
                let cd = data_list(&mut srv, &format!("{run} data get entity {sel} PortalCooldown"))?.and_then(|v| v.first().copied()).unwrap_or(0.0);
                let f = |v: &[f64], i: usize| v.get(i).map(|x| format!("{x:.5}")).unwrap_or_default();
                println!("t+{k} {tag} {dn} pos {} {} {} v {} {} {} hp {hp} cd {cd}", f(&p, 0), f(&p, 1), f(&p, 2), f(&v, 0), f(&v, 1), f(&v, 2));
            }
            last[j] = (Some(dn), hp);
        }
        if k % every == 0 {
            let lying = srv.cmd(&format!("{nrun} execute if entity @e[type=minecraft:item,distance=0..]"))?;
            let mut held = 0u32;
            for c in &containers {
                let out = srv.cmd_long(&format!("{nrun} data get block {} {} {} Items", c[0], c[1], c[2]))?;
                held += out.matches("iron_ingot").count() as u32;
            }
            println!("t+{k} nether: items lying {} ; container slots holding iron {held}", lying.rsplit(':').next().unwrap_or("").trim());
        }
    }
    Ok(())
}

/// A redsim world holding a design's blocks (its box with a margin), placed like the judge places
/// a design and settled.
fn design_world(blocks: &[(mctas::sim::Pos, String)], y_range: (i32, i32), ultrawarm: bool, seed: i64) -> Result<World> {
    let [x0, z0, x1, z1] = chunk_box(blocks, 8);
    let mut w = World::new(BlockPos::new(x0, y_range.0, z0), [x1 - x0 + 1, y_range.1 - y_range.0, z1 - z0 + 1]);
    w.ultrawarm = ultrawarm;
    w.mob.seed(seed, 6000);
    w.mob.ai.on = true;
    let mut placed = Vec::new();
    for (p, s) in blocks {
        let b = Block::parse(s).or_else(|_| redsim::terrain::land::block_of(s)).map_err(|e| anyhow!("{s}: {e}"))?;
        placed.push((BlockPos::new(p[0], p[1], p[2]), b));
    }
    w.place_like_judge(&placed);
    Ok(w)
}

/// `mctas mobai portal-probe --nether --overworld <design> --nether-design <design> --redsim`: the
/// same run on redsim, printing the same lines: the Nether (its own world, `World::nether`)
/// settles 200 ticks alone first, then the overworld design's entities are summoned and followed
/// through the portals (`World::crossings`).
pub fn nether_designs_redsim(ow: &std::path::Path, ne: &std::path::Path, ticks: u32, every: u32, stagger: u32) -> Result<()> {
    let od = mctas::design::Design::load(ow)?;
    let nd = mctas::design::Design::load(ne)?;
    let mut w = design_world(&od.block_list(), (-64, 320), false, 1)?;
    let mut n = design_world(&nd.block_list(), (0, 256), true, 2)?;
    for _ in 0..200 {
        n.tick();
    }
    let nb = nd.block_list();
    let containers: Vec<BlockPos> = nb.iter().filter(|(_, s)| s.starts_with("minecraft:chest") || s.starts_with("minecraft:hopper") || s.starts_with("minecraft:barrel")).map(|(p, _)| BlockPos::new(p[0], p[1], p[2])).collect();
    n.time = w.time;
    w.nether = Some(Box::new(n));
    let mut ids: Vec<(String, u32)> = Vec::new();
    let spawn_k = |w: &mut World, k: usize| -> Result<u32> {
        let e = &od.entities[k];
        redsim::entity::spawn(w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|x| anyhow!("entity {k}: {x}"))
    };
    for k in 0..od.entities.len() {
        // (an entity summoned later holds its place: id 0 until then)
        let id = if k as u32 * stagger == 0 { spawn_k(&mut w, k)? } else { 0 };
        ids.push((format!("np{k}"), id));
    }
    println!("summoned {} entities (redsim, every {stagger} ticks)", ids.len());
    let mut last: Vec<(Option<&str>, f64)> = vec![(Some("overworld"), -1.0); ids.len()];
    for k in 1..=ticks {
        w.tick();
        for j in 1..ids.len() {
            if j as u32 * stagger == k {
                ids[j].1 = spawn_k(&mut w, j)?;
            }
        }
        for (j, (tag, id)) in ids.iter_mut().enumerate() {
            if j as u32 * stagger > k {
                continue;
            }
            // follow the entity through its crossings
            while let Some(&(_, _, to)) = w.crossings.iter().find(|(_, from, _)| *from == *id) {
                *id = to;
            }
            let n = w.nether.as_deref().expect("the Nether");
            let found = w.entities.list.iter().find(|e| e.id == *id && !e.removed).map(|e| ("overworld", e)).or_else(|| n.entities.list.iter().find(|e| e.id == *id && !e.removed).map(|e| ("nether", e)));
            let Some((dn, e)) = found else {
                if last[j].0.is_some() {
                    println!("t+{k} {tag}: gone (dead or removed)");
                    last[j].0 = None;
                }
                continue;
            };
            let hp = match &e.kind {
                redsim::entity::EntityKind::Mob(m) if !m.dead => m.health as f64,
                redsim::entity::EntityKind::Mob(_) => 0.0,
                _ => -1.0,
            };
            let event = last[j].0 != Some(dn) || (hp - last[j].1).abs() > 1e-9;
            if event || k % every == 0 {
                let (p, v) = (e.pos, e.delta_movement);
                println!("t+{k} {tag} {dn} pos {:.5} {:.5} {:.5} v {:.5} {:.5} {:.5} hp {hp} cd {}", p.x, p.y, p.z, v.x, v.y, v.z, e.portal_cooldown);
            }
            last[j] = (Some(dn), hp);
        }
        if k % every == 0 {
            let n = w.nether.as_deref().expect("the Nether");
            let lying = n.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, redsim::entity::EntityKind::Item(_))).count();
            let held = containers.iter().map(|c| n.containers.map.get(c).map(|be| be.slots.iter().flatten().filter(|s| redsim::redstone::container::item_name(s.item).contains("iron_ingot")).count()).unwrap_or(0)).sum::<usize>();
            println!("t+{k} nether: items lying {lying} ; container slots holding iron {held}");
        }
    }
    if !w.errors.is_empty() {
        println!("redsim model errors: {:?}", w.errors);
    }
    Ok(())
}
