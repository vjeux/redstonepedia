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
