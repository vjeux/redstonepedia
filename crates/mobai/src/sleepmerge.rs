//! (tickexact 2026-11-13, day-1 12:27, G1 12:20 / 12:27) The reverse merge on the official server:
//! a villager with its HOME at a bed lies down at night; an observer faces the bed head (the stop's
//! sleep sensor: OCCUPIED flips at SleepInBed's start and at the wake); at the lie-down tick + k the
//! villager is hurt (`--mode damage`) or a hostile is summoned in its view (`--mode hostile`, a NoAI
//! zombie: seen at the hostile sensor's next scan), or nothing (`--mode none`, the control). Printed:
//! the lie-down and wake ticks, every observer pulse (its rising edges after the lie-down: one pulse
//! for two events is the merge), the villager's pose and path for `--watch` gt after the lie-down,
//! and with `--guard` a tripwire string in the cell above the bed head (the awake guard: a standing
//! villager's box against the string's slab, a sleeping one's 0.2-high box under it).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn num_after(out: &str, key: &str) -> Option<f64> {
    out.split(key).nth(1).and_then(|r| r.trim().trim_end_matches(['d', 'f']).parse().ok())
}

fn pos_of(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(list) = out.split("data: [").nth(1) else { return Ok(None) };
    let v: Vec<f64> = list.trim_end_matches(']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None })
}

pub fn sleep_merge(version: &str, mode: &str, k: u32, watch: u32, guard: bool, max_wait: u32) -> Result<()> {
    anyhow::ensure!(["none", "damage", "hostile"].contains(&mode), "--mode none|damage|hostile, got {mode}");
    let opts = VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.cmd_ok("time set 13000")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok(&format!("fill -4 {} -4 4 {} 4 minecraft:glass", Y - 1, Y - 1))?;
    for (a, b) in [((-4, -4), (4, -4)), ((-4, 4), (4, 4)), ((-4, -4), (-4, 4)), ((4, -4), (4, 4))] {
        srv.cmd_ok(&format!("fill {} {Y} {} {} {} {} minecraft:glass", a.0, a.1, b.0, Y + 2, b.1))?;
    }
    srv.cmd_ok(&format!("setblock 0 {Y} 1 minecraft:red_bed[facing=north,part=foot,occupied=false]"))?;
    srv.cmd_ok(&format!("setblock 0 {Y} 0 minecraft:red_bed[facing=north,part=head,occupied=false]"))?;
    // the observer faces the head from the east (its FACING west: it watches the head's cell)
    srv.cmd_ok(&format!("setblock 1 {Y} 0 minecraft:observer[facing=west,powered=false]"))?;
    if guard {
        for (x, s) in [
            (-2, "minecraft:glass"),
            (2, "minecraft:glass"),
            (-1, "minecraft:tripwire_hook[attached=true,facing=east,powered=false]"),
            (1, "minecraft:tripwire_hook[attached=true,facing=west,powered=false]"),
            (0, "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]"),
        ] {
            srv.cmd_ok(&format!("setblock {x} {} 0 {s}", Y + 1))?;
        }
    }
    let out = srv.cmd(&format!(
        "summon minecraft:villager -2.5 {Y} -2.5 {{Tags:[\"sv\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;0,{Y},0]}}}}}}}}}}"
    ))?;
    anyhow::ensure!(out.contains("Summoned new"), "the villager: {}", out.trim());
    println!(
        "== reverse merge on the official {version} server: a bed (head (0,{Y},0), foot (0,{Y},1)), an observer at (1,{Y},0) facing the head, a villager homed there at night; at the lie-down + {k} gt: {}{}",
        match mode {
            "damage" => "1 damage (`damage … 1 minecraft:generic`)",
            "hostile" => "a NoAI zombie summoned 3 blocks away in view",
            _ => "nothing (the control)",
        },
        if guard { format!("; the awake guard: a string at (0,{},0) (its slab {:.4}..{:.4}) on hooks east-west", Y + 1, (Y + 1) as f64 + 1.0 / 16.0, (Y + 1) as f64 + 2.5 / 16.0) } else { String::new() }
    );
    let occupied = |srv: &mut VanillaServer| crate::strict::test(srv, &format!("execute if block 0 {Y} 0 minecraft:red_bed[occupied=true]"));
    let mut lie = None::<u32>;
    for t in 0..max_wait {
        srv.step(1)?;
        if occupied(&mut srv)? {
            lie = Some(t);
            break;
        }
    }
    let Some(lie) = lie else { anyhow::bail!("the villager never lay down within {max_wait} gt") };
    println!("  t{lie}: LIE-DOWN (the head reads occupied=true after this tick)");
    let (mut wake, mut pulses, mut was_on, mut guard_sleep, mut guard_awake) = (None::<u32>, Vec::new(), false, 0u32, 0u32);
    let mut pulse_occ: Vec<(u32, bool)> = Vec::new();
    let mut last_occ = true;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for j in 0..=watch {
        let t = lie + j;
        if j > 0 {
            srv.step(1)?;
        }
        // +k = after the lie-down tick + k has run (k = 0: right after the lie-down tick itself), so the
        // event lands in the next tick's game time
        if j == k {
            match mode {
                "damage" => {
                    let o = srv.cmd("damage @e[tag=sv,limit=1] 1 minecraft:generic")?;
                    println!("  after t{t} (+{j}): damage: {}", o.trim());
                }
                "hostile" => {
                    // west of the bed: the observer (east of the head) would block the sleeper's line of sight
                    let o = srv.cmd(&format!("summon minecraft:zombie -2.5 {Y} -2.5 {{NoAI:1b,PersistenceRequired:1b,Tags:[\"sz\"]}}"))?;
                    println!("  after t{t} (+{j}): zombie: {}", o.trim());
                }
                _ => {}
            }
        }
        let occ = occupied(&mut srv)?;
        let on = crate::strict::test(&mut srv, &format!("execute if block 1 {Y} 0 minecraft:observer[powered=true]"))?;
        let g = guard && crate::strict::test(&mut srv, &format!("execute if block 0 {} 0 minecraft:tripwire[powered=true]", Y + 1))?;
        let p = pos_of(&mut srv, "@e[tag=sv,limit=1]")?;
        if on && !was_on {
            pulses.push(t);
            pulse_occ.push((t, occ));
        }
        was_on = on;
        last_occ = occ;
        if !occ && wake.is_none() {
            wake = Some(t);
            // (ironspec 12:45 (a)) the stand-up cell findStandUpPosition picked
            if let Some(p) = p {
                println!(
                    "  t{t} (+{j}): WAKE: the villager stands up at ({:.4},{:.4},{:.4}), the cell ({},{},{})",
                    p[0],
                    p[1],
                    p[2],
                    p[0].floor() as i32,
                    p[1].floor() as i32,
                    p[2].floor() as i32
                );
            }
        }
        if g {
            if occ {
                guard_sleep += 1;
            } else {
                guard_awake += 1;
            }
        }
        if let Some(p) = p {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        if j <= 12 || j % 10 == 0 || wake.is_some_and(|w| t <= w + 3) {
            let hurt = srv.cmd_long("data get entity @e[tag=sv,limit=1] HurtTime")?;
            println!(
                "  t{t} (+{j}): bed {} | observer {} | villager {} hurt time {}{}",
                if occ { "OCCUPIED" } else { "free" },
                if on { "POWERED" } else { "off" },
                p.map(|p| format!("({:.3},{:.4},{:.3})", p[0], p[1], p[2])).unwrap_or("-".into()),
                num_after(&hurt, "data: ").map(|v| v.to_string()).unwrap_or("-".into()),
                if guard { format!(" | guard {}", if g { "PRESSED" } else { "-" }) } else { String::new() }
            );
        }
    }
    println!(
        "  result: lie-down t{lie}; wake {}; observer pulses after the lie-down: {} at {:?}{}; the villager's path over the watch: x {:.3}..{:.3} y {:.3}..{:.3} z {:.3}..{:.3}{}",
        wake.map(|w| format!("t{w} (+{} gt)", w - lie)).unwrap_or("none in the watch".into()),
        pulses.len(),
        pulses,
        if wake.is_some() && pulses.len() == 1 { " (ONE pulse for the two events: MERGED)" } else { "" },
        lo[0],
        hi[0],
        lo[1],
        hi[1],
        lo[2],
        hi[2],
        if guard { format!("; the guard pressed on {guard_sleep} tick(s) while the bed was occupied and {guard_awake} while free") } else { String::new() }
    );
    // (G1 12:42) a T flip-flop SLEPT on the observer's pulses (0 before the lie-down), against the bed's
    // OCCUPIED at each pulse and at the end: the fault note's parity, shown
    let mut slept = false;
    for (n, (pt, occ)) in pulse_occ.iter().enumerate() {
        slept = !slept;
        println!(
            "  pulse {} at t{pt} (+{}): SLEPT -> {} | the bed {} | {}",
            n + 1,
            pt - lie,
            u8::from(slept),
            if *occ { "OCCUPIED" } else { "free" },
            if slept == *occ { "parity right" } else { "PARITY INVERTED (the latch reads the napper's state backwards)" }
        );
    }
    println!(
        "  end of the watch (t{}): SLEPT {} | the bed {} | {} (a measurement, no verdict)",
        lie + watch,
        u8::from(slept),
        if last_occ { "OCCUPIED" } else { "free" },
        if slept == last_occ { "parity right" } else { "PARITY INVERTED" }
    );
    Ok(())
}

/// (G1 13:37, the deciding row) Does a minecart passing P beside the bed seat a SLEEPING villager?
/// The stop's local cells: the bed facing south (foot (0,Y,-1), head (0,Y,0)), P (1,Y,0) a south_east
/// rail east of the head, a north-south approach (1,Y,1..6) and an east-west exit (2..7,Y,0), inside a
/// glass room; a villager homed to the head lies down at night; `settle` gt after the lie-down a cart is
/// summoned on the approach heading north at the STORED speed v. Printed per tick around the pass: the
/// cart's position, the villager's, the bed's OCCUPIED, whether the villager is the cart's passenger and
/// whether it still has a sleeping position; then the bed and the villager for `watch` gt.
pub fn sleep_pickup(version: &str, v: f64, settle: u32, watch: u32, max_wait: u32, stop: bool) -> Result<()> {
    let opts = VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.cmd_ok("time set 13000")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok(&format!("fill -3 {} -3 8 {} 7 minecraft:glass", Y - 1, Y - 1))?;
    for (a, b) in [((-3, -3), (8, -3)), ((-3, 7), (8, 7)), ((-3, -3), (-3, 7)), ((8, -3), (8, 7))] {
        srv.cmd_ok(&format!("fill {} {Y} {} {} {} {} minecraft:glass", a.0, a.1, b.0, Y + 2, b.1))?;
    }
    srv.cmd_ok(&format!("setblock 0 {Y} -1 minecraft:red_bed[facing=south,part=foot,occupied=false]"))?;
    srv.cmd_ok(&format!("setblock 0 {Y} 0 minecraft:red_bed[facing=south,part=head,occupied=false]"))?;
    srv.cmd_ok(&format!("fill 1 {Y} 1 1 {Y} 6 minecraft:rail[shape=north_south,waterlogged=false]"))?;
    srv.cmd_ok(&format!("fill 2 {Y} 0 7 {Y} 0 minecraft:rail[shape=east_west,waterlogged=false]"))?;
    srv.cmd_ok(&format!("setblock 1 {Y} 0 minecraft:rail[shape=south_east,waterlogged=false] strict"))?;
    anyhow::ensure!(crate::strict::test(&mut srv, &format!("execute if block 1 {Y} 0 minecraft:rail[shape=south_east]"))?, "P is not south_east");
    // (58f770cc 13:46, the stop's own approach with `--stop`) the napper walks in from the eject E north
    // of P (it left its cart there), the released cart comes WEST along the run-up flats into P and turns
    // south onto the hand-off; else the villager walks in from the south-west and the cart comes north
    let (vx, vz) = if stop { (1.5, -1.5) } else { (-1.5, 3.5) };
    let out = srv.cmd(&format!(
        "summon minecraft:villager {vx} {Y} {vz} {{Tags:[\"sv\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;0,{Y},0]}}}}}}}}}}"
    ))?;
    anyhow::ensure!(out.contains("Summoned new"), "the villager: {}", out.trim());
    println!("== the pickup of a sleeping villager on the official {version} server: the bed facing south (head (0,{Y},0)), P (1,{Y},0) south_east east of the head, the approach north-south at x 1; a cart summoned at (1.5, {}, 5.5) heading north at stored v {v:.3} {settle} gt after the lie-down", Y as f64 + 0.0625);
    let occupied = |srv: &mut VanillaServer| crate::strict::test(srv, &format!("execute if block 0 {Y} 0 minecraft:red_bed[occupied=true]"));
    let mut lie = None::<u32>;
    // (cc767b78 14:00, the post-lie-down drift) every tick's position and Motion, the last 3 before the
    // lie-down and 15 after it printed at full precision
    let rowf = |srv: &mut VanillaServer| -> Result<String> {
        let p = srv.cmd_long("data get entity @e[tag=sv,limit=1] Pos")?;
        let m = srv.cmd_long("data get entity @e[tag=sv,limit=1] Motion")?;
        Ok(format!("Pos {} Motion {}", p.split("data: ").nth(1).unwrap_or("-").trim(), m.split("data: ").nth(1).unwrap_or("-").trim()))
    };
    let mut ring: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for t in 0..max_wait {
        srv.step(1)?;
        let r = rowf(&mut srv)?;
        if occupied(&mut srv)? {
            for (i, row) in ring.iter().enumerate() {
                println!("  lie-down {}: {row}", i as i64 - ring.len() as i64);
            }
            println!("  lie-down +0 (t{t}, the head reads occupied): {r}");
            lie = Some(t);
            break;
        }
        ring.push_back(format!("t{t}: {r}"));
        if ring.len() > 3 {
            ring.pop_front();
        }
    }
    let Some(lie) = lie else { anyhow::bail!("the villager never lay down within {max_wait} gt") };
    for j in 1..=15u32 {
        srv.step(1)?;
        println!("  lie-down +{j}: {}", rowf(&mut srv)?);
    }
    srv.step(settle.saturating_sub(15).max(1))?;
    let sp = pos_of(&mut srv, "@e[tag=sv,limit=1]")?;
    println!("  t{lie}: LIE-DOWN; after {settle} gt the sleeper at {}", sp.map(|p| format!("({:.4},{:.4},{:.4})", p[0], p[1], p[2])).unwrap_or("-".into()));
    let out = if stop {
        // just east of P, so the cart enters P at about v (an empty cart loses 4 % a tick on the flats)
        srv.cmd(&format!("summon minecraft:minecart 2.45 {} 0.5 {{Tags:[\"sp\"],Motion:[{:?}d,0.0d,0.0d]}}", Y as f64 + 0.0625, -v))?
    } else {
        srv.cmd(&format!("summon minecraft:minecart 1.5 {} 5.5 {{Tags:[\"sp\"],Motion:[0.0d,0.0d,{:?}d]}}", Y as f64 + 0.0625, -v))?
    };
    anyhow::ensure!(out.contains("Summoned new"), "the cart: {}", out.trim());
    let (mut picked, mut freed) = (None::<u32>, None::<u32>);
    for j in 1..=watch {
        srv.step(1)?;
        let c = pos_of(&mut srv, "@e[tag=sp,limit=1]")?;
        let p = pos_of(&mut srv, "@e[tag=sv,limit=1]")?;
        let occ = occupied(&mut srv)?;
        let pass = srv.cmd_long("data get entity @e[tag=sp,limit=1] Passengers")?.contains("villager");
        let sleeping = srv.cmd_long("data get entity @e[tag=sv,limit=1] SleepingX")?.contains("data:");
        if pass && picked.is_none() {
            picked = Some(j);
        }
        if !occ && freed.is_none() {
            freed = Some(j);
        }
        if j <= 30 || j % 10 == 0 {
            println!(
                "  +{j}: cart {} | villager {} | bed {} | {} | {}",
                c.map(|p| format!("({:.3},{:.3},{:.3})", p[0], p[1], p[2])).unwrap_or("-".into()),
                p.map(|p| format!("({:.3},{:.4},{:.3})", p[0], p[1], p[2])).unwrap_or("-".into()),
                if occ { "OCCUPIED" } else { "free" },
                if pass { "the cart's PASSENGER" } else { "not riding" },
                if sleeping { "sleeping pos set" } else { "no sleeping pos" }
            );
        }
    }
    println!(
        "  result (one sample, AI rider): picked up {}; the bed freed {} (a measurement)",
        picked.map(|j| format!("at +{j} gt after the summon")).unwrap_or("NEVER (the cart passed P without seating it)".into()),
        freed.map(|j| format!("at +{j}")).unwrap_or("never in the watch".into())
    );
    Ok(())
}

/// (cc767b78 14:47 / 15:20, ironspec 14:49) A lie-down on the STOP's OWN CELLS: 58f770cc's bench file placed
/// whole at +80 (its stand-in levers as laid: the shutter's lever on, the hatch open and the zombie hidden;
/// the park's lever off), at night, a villager homed to the bed head (-3,0,4):
/// - `eject`: riding a cart summoned on the flat north of the arming detector (-2,0,1) heading south at the
///   stored speed `v`; the always-on activator E (-2,0,2) sets it down and SleepInBed fires;
/// - `hurt`: standing on P (-2,0,4), 1.00 from the head's centre, hurt (1 generic damage) before its first
///   tick: it panics, and lies down when the panic ends (HURT_BY gone 40 gt after the hit).
/// Printed per tick from the start to the lie-down + 15: the villager's Pos, Motion and OnGround, riding or
/// not, the head's OCCUPIED; the lie-down tick's move after setPosToBed; the settled position. Then
/// `release_after` gt after the lie-down the park's lever is pulled with NO shutter close (the sleeper stays
/// asleep) and the empty cart runs the run-up into P: per tick near the bed its position, the gap between
/// its box inflated 0.2 horizontally (OldMinecartBehavior's pickup box) and the sleeper's 0.2 x 0.2 box
/// (the larger of the x and z separations: 0 = overlapping), and whether it seats the sleeper.
pub fn sleep_stop(version: &str, bench: &std::path::Path, mode: &str, v: f64, release_after: u32, watch: u32, wake_yaw: Option<f32>) -> Result<()> {
    const DY: i32 = 80;
    anyhow::ensure!(mode == "eject" || mode == "hurt", "--mode eject|hurt, got {mode}");
    let opts = VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 13000")?;
    srv.cmd_ok("gamerule advance_time false")?;
    crate::seatclear::place_part(&mut srv, bench)?;
    let head = format!("-3 {DY} 4");
    anyhow::ensure!(crate::strict::test(&mut srv, &format!("execute if block {head} minecraft:red_bed[part=head,facing=south]"))?, "the bench's bed head is not at local (-3,0,4)");
    anyhow::ensure!(crate::strict::test(&mut srv, &format!("execute if block -2 {DY} 2 minecraft:activator_rail[powered=true]"))?, "the activator E (-2,0,2) is not powered");
    anyhow::ensure!(crate::strict::test(&mut srv, &format!("execute if block 3 {DY} 4 minecraft:lever[powered=false]"))?, "the park's stand-in lever (3,0,4) is not laid off");
    let villager = format!("id:\"minecraft:villager\",Tags:[\"sv\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;-3,{DY},4]}}}}}}}}");
    if mode == "eject" {
        let out = srv.cmd(&format!("summon minecraft:minecart -1.5 {} 0.5 {{Tags:[\"sp\"],Motion:[0.0d,0.0d,{v:?}d],Passengers:[{{{villager}}}]}}", DY as f64 + 0.0625))?;
        anyhow::ensure!(out.contains("Summoned new"), "the napper's cart: {}", out.trim());
    } else {
        let out = srv.cmd(&format!("summon minecraft:villager -1.5 {} 4.5 {{{}}}", DY as f64 + 0.0625, villager.trim_start_matches("id:\"minecraft:villager\",")))?;
        anyhow::ensure!(out.contains("Summoned new"), "the villager on P: {}", out.trim());
        srv.cmd_ok("damage @e[tag=sv,limit=1] 1 minecraft:generic")?;
        // the empty cart braked on the park (1,0,4), released later down the run-up into P
        let out = srv.cmd(&format!("summon minecraft:minecart 1.5 {} 4.5 {{Tags:[\"sp\"]}}", DY as f64 + 0.5625))?;
        anyhow::ensure!(out.contains("Summoned new"), "the park's cart: {}", out.trim());
    }
    println!(
        "== a lie-down on the stop's own cells ({mode}), official {version}: {} at +{DY}; {}; the shutter not closed (the sleeper stays asleep); the park released {release_after} gt after the last lie-down",
        bench.display(),
        if mode == "eject" { format!("the napper's cart summoned at (-1.5, {}, 0.5) heading south at stored {v:.3}", DY as f64 + 0.0625) } else { "the villager standing on P (-1.5, ., 4.5), hurt (1 generic) before its first tick; an empty cart braked on the park (1,0,4)".into() }
    );
    let field = |srv: &mut VanillaServer, sel: &str, f: &str| -> Result<String> { Ok(srv.cmd_long(&format!("data get entity {sel} {f}"))?.split("data: ").nth(1).unwrap_or("-").trim().to_string()) };
    let occupied = |srv: &mut VanillaServer| crate::strict::test(srv, &format!("execute if block {head} minecraft:red_bed[occupied=true]"));
    // every lie-down (the head's OCCUPIED rising) and wake (falling); `eject` stops at its first lie-down + 15,
    // `hurt` at its second (the re-sleep after the panic) + 15
    let (mut lies, mut wakes, mut off) = (Vec::<u32>::new(), Vec::<u32>::new(), None::<u32>);
    let mut was = false;
    let want = if mode == "eject" { 1 } else { 2 };
    let mut rows: Vec<String> = Vec::new();
    for t in 1..=600u32 {
        srv.step(1)?;
        let riding = mode == "eject" && srv.cmd_long("data get entity @e[tag=sp,limit=1] Passengers")?.contains("villager");
        if mode == "eject" && !riding && off.is_none() {
            off = Some(t);
        }
        let occ = occupied(&mut srv)?;
        rows.push(format!(
            "t{t}: Pos {} Motion {} OnGround {} | {}head {}",
            field(&mut srv, "@e[tag=sv,limit=1]", "Pos")?,
            field(&mut srv, "@e[tag=sv,limit=1]", "Motion")?,
            field(&mut srv, "@e[tag=sv,limit=1]", "OnGround")?,
            if mode == "eject" { if riding { "riding | " } else { "set down | " } } else { "" },
            if occ { "OCCUPIED" } else { "free" }
        ));
        if occ && !was {
            lies.push(t);
        }
        if !occ && was {
            wakes.push(t);
        }
        was = occ;
        if lies.len() >= want && t >= lies[want - 1] + 15 {
            break;
        }
    }
    anyhow::ensure!(!lies.is_empty(), "the villager never lay down within 600 gt (set down at {off:?})");
    for &l in &lies {
        for r in rows.iter().skip((l as usize).saturating_sub(4)).take(19) {
            println!("  {r}");
        }
        println!("  --");
    }
    println!("  lie-downs at {lies:?}, wakes at {wakes:?}");
    let lie = *lies.last().unwrap_or(&0);
    if let Some(o) = off {
        println!("  set down at t{o}, lay down at t{lie} ({} after the set-down)", lie as i64 - o as i64);
    }
    // the lie-down tick's move after setPosToBed: its position minus the head's centre (setPosToBed puts it
    // at x + 0.5, y + 0.6875, z + 0.5)
    let lie_row = rows.get(lie as usize - 1).cloned().unwrap_or_default();
    println!("  the lie-down tick t{lie} (setPosToBed puts the sleeper at (-2.5, {:.4}, 4.5); its move that tick is its Pos minus that): {lie_row}", DY as f64 + 0.6875);
    // (G1 15:47) the stand-up cell: AbstractBedBlock.findStandUpPosition takes `side` = the bed's clockwise
    // side unless the sleeper's yaw faces it: with --wake-yaw the sleeper's Rotation is set, it is hurt
    // (stopSleeping at once) and the cell it stands up on is printed, then the run ends
    if let Some(yaw) = wake_yaw {
        srv.step(20)?;
        srv.cmd_ok(&format!("data merge entity @e[tag=sv,limit=1] {{Rotation:[{yaw:?}f,0.0f]}}"))?;
        srv.cmd_ok("damage @e[tag=sv,limit=1] 1 minecraft:generic")?;
        let p = pos_of(&mut srv, "@e[tag=sv,limit=1]")?.ok_or_else(|| anyhow::anyhow!("the villager is gone"))?;
        let occ = occupied(&mut srv)?;
        println!(
            "  WAKE at yaw {yaw}: stood up at ({:.4}, {:.4}, {:.4}) = local cell ({},{},{}); the head {}",
            p[0],
            p[1],
            p[2],
            p[0].floor() as i32,
            p[1].floor() as i32 - DY,
            p[2].floor() as i32,
            if occ { "still OCCUPIED" } else { "free" }
        );
        return Ok(());
    }
    srv.step(release_after.saturating_sub(15).max(1))?;
    let sp = pos_of(&mut srv, "@e[tag=sv,limit=1]")?.ok_or_else(|| anyhow::anyhow!("the sleeper is gone"))?;
    println!("  the settled sleeper at ({:.6}, {:.4}, {:.6}): {:+.6} / {:+.6} off the head's centre (-2.5, 4.5)", sp[0], sp[1], sp[2], sp[0] + 2.5, sp[2] - 4.5);
    srv.cmd_ok(&format!("setblock 3 {DY} 4 minecraft:lever[face=wall,facing=east,powered=true]"))?;
    // a lever set by command updates only its own neighbours, not the attached conductor's (a pull would
    // update both): the park rail (1,0,4) beside the conductor (2,0,4) is set powered, as the pull powers it
    for shape in ["ascending_east", "ascending_west"] {
        if crate::strict::test(&mut srv, &format!("execute if block 1 {DY} 4 minecraft:powered_rail[shape={shape}]"))? {
            srv.cmd_ok(&format!("setblock 1 {DY} 4 minecraft:powered_rail[shape={shape},powered=true]"))?;
        }
    }
    if let Some(c) = pos_of(&mut srv, "@e[type=minecraft:minecart,limit=1,sort=nearest,x=1.5,y=80,z=4.5]")? {
        println!("  at the release: the cart nearest the park (1,0,4) at ({:.3}, {:.4}, {:.3}) (local y {:.4})", c[0], c[1], c[2], c[1] - DY as f64);
    }
    let (mut least, mut picked) = (f64::MAX, None::<u32>);
    // (G1 on W) the park rail set powered by command (a lever pull's conductor update emulated) may revert at
    // its next neighbour update: its powered state printed every tick until the cart has left the park cell
    let mut park_left = false;
    for j in 1..=watch {
        srv.step(1)?;
        if !park_left {
            let powered = crate::strict::test(&mut srv, &format!("execute if block 1 {DY} 4 minecraft:powered_rail[powered=true]"))?;
            let on_park = crate::strict::test(&mut srv, &format!("execute if entity @e[type=minecraft:minecart,x=1,y={DY},z=4,dx=0,dy=0,dz=0]"))?;
            println!("  release + {j}: the park rail (1,0,4) {} (the stand-in: set powered by command); a cart on the park cell: {on_park}", if powered { "powered" } else { "UNPOWERED" });
            park_left = !on_park;
        }
        let Some(c) = pos_of(&mut srv, "@e[type=minecraft:minecart,limit=1,sort=nearest,x=-1.5,y=80,z=4.5]")? else { continue };
        let gx = ((c[0] - sp[0]).abs() - (0.49 + 0.2 + 0.1)).max(0.0);
        let gz = ((c[2] - sp[2]).abs() - (0.49 + 0.2 + 0.1)).max(0.0);
        let gap = gx.max(gz);
        least = least.min(gap);
        let pass = srv.cmd_long("execute as @e[type=minecraft:minecart] on passengers run data get entity @s Tags")?.contains("sv");
        if pass && picked.is_none() {
            picked = Some(j);
        }
        if gap < 1.0 || pass {
            println!("  release + {j}: the nearest cart ({:.3}, {:.4}, {:.3}) | gap {gap:.4} | {} | head {}", c[0], c[1], c[2], if pass { "SEATED" } else { "not riding" }, if occupied(&mut srv)? { "OCCUPIED" } else { "free" });
        }
    }
    println!(
        "  result (one sample, AI villager): {}; the least gap between a cart's pickup box and the sleeper's box over the pass {} (a measurement)",
        picked.map(|j| format!("SEATED at release + {j}")).unwrap_or("never seated".into()),
        if least == f64::MAX { "- (no cart came near)".to_string() } else { format!("{least:.4}") }
    );
    Ok(())
}
