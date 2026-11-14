//! (tickexact 2026-11-12, ironspec; raw and asserted 2026-11-14 for cc767b78 08:52 (1) / ironspec 08:53a, 08:56a (2),
//! 08:58a (2): SEATED_FEET_OFFSET's citation) `mctas mobai seat-offset`: on the official 26.3 server, a seated villager's
//! position beside its cart's, both read RAW in the same tick (the server paused between ticks; `data get entity … Pos`,
//! the doubles exactly as the server prints them): on a flat rail, on an ascending one (the brake), off-rail on a closed
//! bottom-half trapdoor, all at rest for 40 gt; then the flat cart pushed east at 0.4 and the slope cart pushed up at 0.6
//! (it climbs and rolls back), every tick for 14 gt. 26.3: `AbstractMinecart.getPassengerAttachmentPoint` (179-182) gives a
//! Villager `LOWERED_PASSENGER_ATTACHMENT = (0, 0, 0)` (51), the villager has no riding offset (EntityTypes 1068), and
//! `Entity.positionRider` (2458-2462) places a passenger at the riding position minus its vehicle attachment: the villager's
//! feet AT its cart's position. (ironspec 08:58a (2): a boat rider's offset is its own) a zombie and a villager each alone
//! in an oak boat at rest on stone, the rider's position minus the boat's printed every tick.
//! Asserted (MCTAS_SEAT_OFFSET_PLANT=k inverts assert k, read strictly 1..=3; a planted run that never reaches its assert
//! fails, "control planted nothing"): (1) on every read of every minecart row (54 reads a row at rest and moving) the
//! villager is read back seated in its cart and its raw position equals the cart's, all three doubles printed identically;
//! (2) both moving carts moved on every one of their 14 ticks (the equality holds while moving, not only at rest); (3) each
//! boat rider is read back seated on all 40 reads and its offset from the boat is one constant (its value printed).

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_SEAT_OFFSET_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_SEAT_OFFSET_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=3).contains(&k), "MCTAS_SEAT_OFFSET_PLANT={k}: seat-offset has asserts 1..3");
            Ok(Some(k))
        }
    }
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

/// The doubles of a `data get entity … Pos` reply exactly as the server prints them
fn raw(srv: &mut VanillaServer, sel: &str) -> Result<Option<String>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    Ok(out.split("entity data: ").nth(1).map(|s| s.trim().to_string()).filter(|s| s.starts_with('[') && s.ends_with(']')))
}

fn parse(s: &str) -> Option<[f64; 3]> {
    let v: Vec<f64> = s.trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches('d').parse().ok()).collect();
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

/// One read of a row: (rider raw == vehicle raw, seated, the vehicle's raw position, the rider minus the vehicle)
fn read(srv: &mut VanillaServer, tag: &str) -> Result<(bool, bool, Option<String>, Option<[f64; 3]>)> {
    let c = raw(srv, &format!("@e[tag={tag},limit=1]"))?;
    let v = raw(srv, &format!("@e[tag={tag}v,limit=1]"))?;
    let seated = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Passengers"))?.contains(&format!("\"{tag}v\""));
    let off = match (c.as_deref().and_then(parse), v.as_deref().and_then(parse)) {
        (Some(c), Some(v)) => Some([v[0] - c[0], v[1] - c[1], v[2] - c[2]]),
        _ => None,
    };
    Ok((c.is_some() && c == v, seated, c, off))
}

pub fn seat_offset(version: &str) -> Result<()> {
    plant()?;
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 18000")?;
    srv.cmd_ok("forceload add -16 -16 47 47")?;
    srv.cmd_ok(&format!("fill -4 {} -4 40 {} 20 minecraft:air", DY - 2, DY + 12))?;
    srv.step(20)?;
    // flat: a straight rail on stone, x 0..30 at z 0
    srv.cmd_ok(&format!("fill 0 {} 0 30 {} 0 minecraft:stone", DY - 1, DY - 1))?;
    srv.cmd_ok(&format!("fill 0 {DY} 0 30 {DY} 0 minecraft:rail[shape=east_west] strict"))?;
    // ascending: rails stepping up east at z 6 from x 0 (y DY) to x 6 (y DY+6), each on stone; the lowest an unpowered
    // powered rail (a brake that holds a cart at rest on the slope)
    for i in 0..=6 {
        srv.cmd_ok(&format!("setblock {i} {} 6 minecraft:stone", DY + i - 1))?;
        let s = if i == 0 { "minecraft:powered_rail[shape=ascending_east,powered=false]".to_string() } else { "minecraft:rail[shape=ascending_east]".to_string() };
        srv.cmd_ok(&format!("setblock {i} {} 6 {s} strict", DY + i))?;
    }
    srv.cmd_ok(&format!("setblock 7 {} 6 minecraft:stone", DY + 6))?;
    // off-rail: a closed bottom-half oak trapdoor (as H), the cart resting on it
    srv.cmd_ok(&format!("setblock 12 {} 10 minecraft:stone", DY - 1))?;
    srv.cmd_ok(&format!("setblock 12 {DY} 10 minecraft:oak_trapdoor[half=bottom,open=false,facing=north]"))?;
    // the boats on stone
    srv.cmd_ok(&format!("fill 18 {} 14 26 {} 18 minecraft:stone", DY - 1, DY - 1))?;
    let carts = [("flat rail", "flat", 2.5, DY as f64, 0.5), ("ascending rail (the brake)", "slope", 0.5, DY as f64 + 0.5, 6.5), ("off-rail on a closed bottom-half trapdoor", "trap", 12.5, DY as f64 + 0.5, 10.5)];
    for (_, tag, x, y, z) in carts {
        srv.cmd_ok(&format!("summon minecraft:minecart {x} {y} {z} {{Tags:[\"{tag}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"{tag}v\"],NoAI:1b}}]}}"))?;
    }
    let boats = [("a ZOMBIE alone in an oak boat on stone", "bz", 19.5, "minecraft:zombie"), ("a VILLAGER alone in an oak boat on stone", "bv", 24.5, "minecraft:villager")];
    for (_, tag, x, mob) in boats {
        srv.cmd_ok(&format!("summon minecraft:oak_boat {x} {DY}.0 16.5 {{Tags:[\"{tag}\"],Rotation:[0.0f,0.0f],Passengers:[{{id:\"{mob}\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]}}"))?;
    }
    println!("== seat-offset on the official {version} server (y as the server prints it; the rails at y {DY}): a seated rider's raw position beside its vehicle's");
    let mut cart_equal = [0u32; 3];
    let mut cart_seated = [0u32; 3];
    let mut boat_seated = [0u32; 2];
    let mut boat_offsets: [std::collections::BTreeSet<String>; 2] = Default::default();
    // at rest: 40 reads
    for t in 1..=40u32 {
        srv.step(1)?;
        for (k, (label, tag, ..)) in carts.iter().enumerate() {
            let (eq, seated, c, _) = read(&mut srv, tag)?;
            cart_equal[k] += u32::from(eq);
            cart_seated[k] += u32::from(seated);
            if t == 1 || t == 40 {
                let v = raw(&mut srv, &format!("@e[tag={tag}v,limit=1]"))?;
                println!("    t{t:>2} {label}, at rest: cart {} | villager {}", c.as_deref().unwrap_or("gone"), v.as_deref().unwrap_or("gone"));
            }
        }
        for (j, (label, tag, ..)) in boats.iter().enumerate() {
            let (_, seated, b, off) = read(&mut srv, tag)?;
            boat_seated[j] += u32::from(seated);
            if let Some(o) = off {
                boat_offsets[j].insert(format!("({:+.6}, {:+.6}, {:+.6})", o[0], o[1], o[2]));
            }
            if t == 1 || t == 40 {
                let v = raw(&mut srv, &format!("@e[tag={tag}v,limit=1]"))?;
                println!("    t{t:>2} {label}: boat {} | rider {}", b.as_deref().unwrap_or("gone"), v.as_deref().unwrap_or("gone"));
            }
        }
    }
    // moving: the flat cart pushed east, the slope cart pushed up the slope (it climbs and rolls back down)
    srv.cmd_ok("data merge entity @e[tag=flat,limit=1] {Motion:[0.4d,0.0d,0.0d]}")?;
    srv.cmd_ok(&format!("setblock 0 {DY} 6 minecraft:rail[shape=ascending_east] strict"))?;
    srv.cmd_ok("data merge entity @e[tag=slope,limit=1] {Motion:[0.6d,0.0d,0.0d]}")?;
    let mut moved = [0u32; 2];
    let mut prev: [Option<String>; 2] = [raw(&mut srv, "@e[tag=flat,limit=1]")?, raw(&mut srv, "@e[tag=slope,limit=1]")?];
    for t in 1..=14u32 {
        srv.step(1)?;
        for (k, (label, tag)) in [("flat, moving", "flat"), ("ascending, moving", "slope")].iter().enumerate() {
            let (eq, seated, c, _) = read(&mut srv, tag)?;
            cart_equal[k] += u32::from(eq);
            cart_seated[k] += u32::from(seated);
            if c.is_some() && c != prev[k] {
                moved[k] += 1;
            }
            let v = raw(&mut srv, &format!("@e[tag={tag}v,limit=1]"))?;
            println!("    t{t:>2} {label}: cart {} | villager {}", c.as_deref().unwrap_or("gone"), v.as_deref().unwrap_or("gone"));
            prev[k] = c;
        }
        // the trapdoor cart keeps being read at rest, 54 reads like the others
        let (eq, seated, ..) = read(&mut srv, "trap")?;
        cart_equal[2] += u32::from(eq);
        cart_seated[2] += u32::from(seated);
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    for (k, (label, ..)) in carts.iter().enumerate() {
        println!("  => {label}: villager == cart (raw) on {} of 54 reads; seated on {}", cart_equal[k], cart_seated[k]);
    }
    println!("  => moving: the flat cart moved on {} of 14 ticks, the slope cart on {}", moved[0], moved[1]);
    for (j, (label, ..)) in boats.iter().enumerate() {
        println!("  => {label}: seated on {} of 40 reads; the rider's position minus the boat's (raw doubles): {:?}", boat_seated[j], boat_offsets[j]);
    }
    pin(1, cart_equal.iter().all(|&n| n == 54) && cart_seated.iter().all(|&n| n == 54), format!("(1) on every one of the 54 reads of every minecart row the villager is read back seated and its raw position equals its cart's (feet at the cart's y: offset 0): equal {cart_equal:?}, seated {cart_seated:?}"))?;
    pin(2, moved == [14, 14], format!("(2) both moving carts moved on every one of their 14 ticks: {moved:?}"))?;
    pin(3, boat_seated == [40, 40] && boat_offsets.iter().all(|o| o.len() == 1), format!("(3) each boat rider is read back seated on all 40 reads and its offset from the boat is one constant: {boat_offsets:?}"))?;
    if let Some(k) = plant()? {
        bail!("control planted nothing: MCTAS_SEAT_OFFSET_PLANT={k} never failed its assert");
    }
    println!("  every known answer held (asserts 1..3)");
    Ok(())
}
