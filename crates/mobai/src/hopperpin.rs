//! (tickexact 2026-11-13, ironspec 19:03 (2) / G1 19:10: the hopper / dropper / comparator pins for day-1's
//! ctl-credit-bench) `mctas mobai hopper-pin`: on the official 26.3 server, five rows, every stepped tick printed:
//! - (1) a 1-gt pulse into a dropper (3 shears) facing an EMPTY hopper that faces a chest; a comparator reads
//!   the hopper: the dropper's fire tick, the hopper's `TransferCooldown` after the entry and its first push,
//!   the comparator's change ticks;
//! - (2) the same dropper into a hopper already holding one item (facing down into stone: it cannot eject): the
//!   insert sets no cooldown;
//! - (3) a FULL hopper (5 shears) under a chest of 2, facing an empty chest, unpowered at step 0: an eject and a
//!   suck in the same tick (a full hopper can suck only after its eject) and the 8-gt cycle after;
//! - (4) hoppers holding 0..5 shears (stack size 1) and their comparators' outputs;
//! - (5) a dry dropper pulsed: nothing moves.
//! Step k is the k-th stepped tick after the command that made the row's edge (the server is frozen between
//! steps; a tick scheduled at the command's game time + d runs in step d).

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 101;

fn count(srv: &mut VanillaServer, p: [i32; 3]) -> Result<usize> {
    let s = srv.cmd_long(&format!("data get block {} {} {} Items", p[0], p[1], p[2]))?;
    Ok(s.matches("minecraft:shears").count())
}

fn nbt_int(srv: &mut VanillaServer, p: [i32; 3], path: &str) -> Result<i64> {
    let s = srv.cmd_long(&format!("data get block {} {} {} {path}", p[0], p[1], p[2]))?;
    let tail = s.rsplit(':').next().unwrap_or("").trim();
    let num: String = tail.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    num.parse::<i64>().with_context(|| format!("no {path} at {p:?}: {s}"))
}

fn items(n: usize) -> String {
    let v: Vec<String> = (0..n).map(|i| format!("{{Slot:{i}b,id:\"minecraft:shears\",count:1}}")).collect();
    format!("{{Items:[{}]}}", v.join(","))
}

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

/// One dropper row: a dropper at `base` facing east holding `stock` shears, the hopper east of it (`hopper`
/// state with `pre` shears already in it), a chest east of the hopper, a comparator south of the hopper reading
/// it; a 1-gt pulse from a redstone block west of the dropper at step 0.
fn dropper_row(srv: &mut VanillaServer, label: &str, base: [i32; 3], stock: usize, hopper: &str, pre: usize) -> Result<()> {
    let [x, y, z] = base;
    let (dropper, hop, chest, comp, src) = ([x, y, z], [x + 1, y, z], [x + 2, y, z], [x + 1, y, z + 1], [x - 1, y, z]);
    set(srv, dropper, &format!("minecraft:dropper[facing=east]{}", items(stock)))?;
    set(srv, hop, &format!("{hopper}{}", items(pre)))?;
    set(srv, chest, "minecraft:chest[facing=north]")?;
    set(srv, comp, "minecraft:comparator[facing=north]")?;
    srv.step(10)?;
    println!("  ({label}) dropper {stock} shears at {dropper:?} facing east -> hopper {hop:?} `{hopper}` holding {pre} -> chest {chest:?}; comparator {comp:?} reads the hopper; a redstone block at {src:?} for step 1 only");
    println!("    before: hopper TransferCooldown {}, comparator {}", nbt_int(srv, hop, "TransferCooldown")?, nbt_int(srv, comp, "OutputSignal")?);
    set(srv, src, "minecraft:redstone_block")?;
    for k in 1..=18 {
        srv.step(1)?;
        if k == 1 {
            set(srv, src, "minecraft:air")?;
        }
        let trig = srv.test_block(dropper, "minecraft:dropper[triggered=true]")?;
        println!(
            "    step {k:>2}: dropper triggered={trig} items {} | hopper items {} TransferCooldown {} | chest items {} | comparator {}",
            count(srv, dropper)?,
            count(srv, hop)?,
            nbt_int(srv, hop, "TransferCooldown")?,
            count(srv, chest)?,
            nbt_int(srv, comp, "OutputSignal")?
        );
    }
    Ok(())
}

fn full_hopper_row(srv: &mut VanillaServer, base: [i32; 3]) -> Result<()> {
    let [x, y, z] = base;
    let (hop, top, front, power) = ([x, y, z], [x, y + 1, z], [x + 1, y, z], [x, y, z - 1]);
    set(srv, power, "minecraft:redstone_block")?;
    set(srv, hop, &format!("minecraft:hopper[facing=east,enabled=false]{}", items(5)))?;
    set(srv, top, &format!("minecraft:chest[facing=north]{}", items(2)))?;
    set(srv, front, "minecraft:chest[facing=north]")?;
    srv.step(10)?;
    println!("  (3) a FULL hopper {hop:?} (5 shears) facing east into an empty chest {front:?}, a chest of 2 above it {top:?}; held disabled by a redstone block {power:?}, removed at step 0");
    println!("    before: hopper {} top {} front {} TransferCooldown {}", count(srv, hop)?, count(srv, top)?, count(srv, front)?, nbt_int(srv, hop, "TransferCooldown")?);
    set(srv, power, "minecraft:air")?;
    for k in 1..=20 {
        srv.step(1)?;
        println!(
            "    step {k:>2}: hopper items {} TransferCooldown {} | top chest {} | front chest {}",
            count(srv, hop)?,
            nbt_int(srv, hop, "TransferCooldown")?,
            count(srv, top)?,
            count(srv, front)?
        );
    }
    Ok(())
}

pub fn hopper_pin(version: &str) -> Result<()> {
    println!("== hopper-pin on the official {version} server (ironspec 19:03 (2), G1 19:10): stepped ticks, each row's edge made by a command at step 0");
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        tick_rate: 1000,
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 47 63")?;
    srv.cmd_ok(&format!("fill -16 {} -16 47 {} 63 minecraft:stone", Y - 1, Y - 1))?;
    srv.step(20)?;
    dropper_row(&mut srv, "1", [0, Y, 0], 3, "minecraft:hopper[facing=east]", 0)?;
    dropper_row(&mut srv, "2", [0, Y, 10], 3, "minecraft:hopper[facing=down]", 1)?;
    full_hopper_row(&mut srv, [0, Y, 20])?;
    println!("  (4) hoppers facing down into stone holding n shears (stack size 1), each read by a comparator south of it (each comparator laid first, its hopper set after it: a comparator set beside an already filled hopper reads nothing until a neighbour update)");
    for n in 0..=5usize {
        let x = 2 * n as i32;
        set(&mut srv, [x, Y, 31], "minecraft:comparator[facing=north]")?;
    }
    srv.step(2)?;
    for n in 0..=5usize {
        let x = 2 * n as i32;
        set(&mut srv, [x, Y, 30], &format!("minecraft:hopper[facing=down]{}", items(n)))?;
    }
    srv.step(6)?;
    let mut vals = Vec::new();
    for n in 0..=5usize {
        let x = 2 * n as i32;
        vals.push(format!("{n} -> {}", nbt_int(&mut srv, [x, Y, 31], "OutputSignal")?));
    }
    println!("    comparator output by item count: {}", vals.join(", "));
    dropper_row(&mut srv, "5 (dry)", [0, Y, 40], 0, "minecraft:hopper[facing=east]", 0)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
