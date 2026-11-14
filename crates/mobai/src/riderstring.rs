//! (tickexact 2026-11-14, dfb89499 05:11 / 58f770cc 05:13 / G1 05:17 (5): does a SEATED RIDER press a tripwire string in
//! 26.3?) `mctas mobai rider-string`: on the official 26.3 server, a flat rail (x, Y, -6 .. 6) on stone and, one block
//! above it at z 1, a tripwire string (x, Y+1, 1) attached between hooks (x-1, Y+1, 1) facing east / (x+1, Y+1, 1) facing
//! west on glass: the string's band is y Y+1.0625 .. Y+1.15625; a cart resting on the rail tops out at Y+0.7625 (0.300
//! below the band, as 58f770cc's R string over R's rail); a villager seated in it reaches through the band. One copy per
//! row, 8 apart in x. Rows (many visits, G1 05:17 (5)): 8 RIDDEN carts at rest under the string with AI villagers and 8
//! with NoAI villagers (all summoned the same tick), RIDDEN carts (AI and NoAI) rolling under the string from 3.5 blocks
//! north at 0.10 / 0.15 / 0.20 / 0.40 b/t (two entry phases each, z0 -2.5 / -2.25), 2 EMPTY carts at rest and 2 rolling
//! at 0.40 (controls: the cart's box never reaches the band), and a no-cart control. Every tick for 60 gt: every string's
//! powered state, and the first resting AI rider's box lower edge (its feet y) against the band.
//! The 26.3 path: a seated rider ticks through `Entity.rideTick` (2444-2449: `tick()`, then `positionRider`);
//! `LivingEntity.aiStep` (3139-3141) runs `applyEffectsFromBlocks()` with no passenger or AI test (Entity 940-951: this
//! tick's moves, else oldPosition -> position), so its box is checked against the blocks it is inside
//! (Entity.checkInsideBlocks 1345-) and `TripWireBlock.entityInside` (138-150) presses an unpowered string; once pressed,
//! the string's own re-check (`checkPressed`, 160-164) reads every entity in its box (`level.getEntities(null, ...)`).
//! Asserted (the first run's answers; MCTAS_RIDER_STRING_PLANT=k inverts assert k, read strictly): (1) every resting
//! ridden cart's rider presses its string at t1, AI on and NoAI alike, and holds it pressed all 60 ticks (16 rows); (2) no
//! resting EMPTY cart ever presses its string (2 rows); (3) every rolling ridden cart presses its string while passing, AI
//! on and NoAI (16 rows); (4) the rolling empty carts and the no-cart control never press theirs (3 rows); (5) each
//! rolling AI rider first presses within one tick of its NoAI twin; (6) every ridden row's villager is read back SEATED in
//! its own cart every tick, with NoAI exactly where summoned NoAI; (7) every control cart is read back under the string
//! (the resting empties inside its cell all 60 ticks, the rolling ones passing through it).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

/// (G1 06:54 non-blocking) MCTAS_RIDER_STRING_PLANT read strictly: unset, or an assert number 1..=7; anything else fails
fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_RIDER_STRING_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_RIDER_STRING_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=7).contains(&k), "MCTAS_RIDER_STRING_PLANT={k}: rider-string has asserts 1..7");
            Ok(Some(k))
        }
    }
}

/// One copy: its label, the rider (Some(AI on) / None: empty), its speed (0: at rest; < 0: no cart), its entry z.
struct RowSpec {
    label: String,
    rider: Option<bool>,
    v: f64,
    z0: f64,
}

pub fn rider_string(version: &str) -> Result<()> {
    println!("== rider-string on the official {version} server: a flat rail (x,{Y},z), a hooked tripwire string one block above at z 1 (band y {}.0625 .. {}.15625); a resting cart's box tops out at {}.7625", Y + 1, Y + 1, Y);
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 6000")?;
    let mut rows: Vec<RowSpec> = Vec::new();
    for ai in [true, false] {
        for i in 0..8 {
            rows.push(RowSpec { label: format!("RIDDEN ({}) at rest #{}", if ai { "AI" } else { "NoAI" }, i + 1), rider: Some(ai), v: 0.0, z0: 1.5 });
        }
    }
    for ai in [true, false] {
        for v in [0.10, 0.15, 0.20, 0.40] {
            for z0 in [-2.5, -2.25] {
                rows.push(RowSpec { label: format!("RIDDEN ({}) rolling at {v:.2} from z {z0}", if ai { "AI" } else { "NoAI" }), rider: Some(ai), v, z0 });
            }
        }
    }
    for i in 0..2 {
        rows.push(RowSpec { label: format!("EMPTY at rest #{} (control)", i + 1), rider: None, v: 0.0, z0: 1.5 });
        rows.push(RowSpec { label: format!("EMPTY rolling at 0.40 #{} (control)", i + 1), rider: None, v: 0.40, z0: -2.5 });
    }
    rows.push(RowSpec { label: "no cart (control)".into(), rider: None, v: -1.0, z0: 0.0 });
    let n = rows.len() as i32;
    srv.cmd_ok(&format!("forceload add -8 -16 {} 16", n * 8 + 8))?;
    srv.step(20)?;
    for k in 0..n {
        let x = k * 8;
        srv.cmd(&format!("fill {} {} -7 {} {} 7 minecraft:air", x - 3, Y - 1, x + 3, Y + 4))?;
        srv.cmd(&format!("fill {x} {} -7 {x} {} 7 minecraft:stone", Y - 1, Y - 1))?;
        srv.cmd(&format!("fill {x} {Y} -7 {x} {Y} 7 minecraft:rail[shape=north_south]"))?;
        srv.cmd(&format!("setblock {} {} 1 minecraft:glass", x - 2, Y + 1))?;
        srv.cmd(&format!("setblock {} {} 1 minecraft:glass", x + 2, Y + 1))?;
        srv.cmd(&format!("setblock {} {} 1 minecraft:tripwire_hook[attached=false,facing=east,powered=false]", x - 1, Y + 1))?;
        srv.cmd(&format!("setblock {} {} 1 minecraft:tripwire_hook[attached=false,facing=west,powered=false]", x + 1, Y + 1))?;
        srv.cmd(&format!("setblock {x} {} 1 minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]", Y + 1))?;
    }
    srv.step(20)?;
    for k in 0..n {
        let x = k * 8;
        let attached = srv.test_block([x, Y + 1, 1], "minecraft:tripwire[attached=true,powered=false]")?;
        anyhow::ensure!(attached, "copy x {x}: the string is not attached and unpowered before the rows (the hooks)");
    }
    for (k, r) in rows.iter().enumerate() {
        let x = k as i32 * 8;
        let tag = format!("rs{k}");
        if r.v >= 0.0 {
            let passenger = match r.rider {
                Some(ai) => format!(",Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]{}}}]", if ai { "" } else { ",NoAI:1b" }),
                None => String::new(),
            };
            srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 {Y}.0625 {} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,{:.4}d]{passenger}}}", r.z0, r.v))?;
        }
    }
    println!("  {} copies: every string attached and unpowered before the carts are summoned (t0)", n);
    plant()?;
    let mut seen: Vec<Vec<bool>> = vec![Vec::new(); rows.len()];
    // (G1 06:54 (5), ironspec 07:03 (4)) read back every tick: each ridden row's villager still SEATED in its cart, and
    // each control cart's z (a resting empty under the string's cell z 1 .. 2, a rolling one passing through it)
    let mut seated: Vec<u32> = vec![0; rows.len()];
    let mut in_cell: Vec<u32> = vec![0; rows.len()];
    let mut noai: Vec<Option<bool>> = vec![None; rows.len()];
    let mut noai_ok: Vec<u32> = vec![0; rows.len()];
    for (k, r) in rows.iter().enumerate() {
        if r.rider.is_some() {
            let out = srv.cmd_long(&format!("data get entity @e[tag=rs{k}v,limit=1] NoAI"))?;
            noai[k] = Some(out.contains("1b"));
        }
    }
    for t in 1..=60u32 {
        srv.step(1)?;
        let mut line = String::new();
        for (k, s) in seen.iter_mut().enumerate() {
            let x = k as i32 * 8;
            let on = srv.test_block([x, Y + 1, 1], "minecraft:tripwire[powered=true]")?;
            s.push(on);
            line.push(if on { 'P' } else { '.' });
        }
        for (k, r) in rows.iter().enumerate() {
            if r.rider.is_some() {
                let p = srv.cmd_long(&format!("data get entity @e[tag=rs{k},limit=1] Passengers"))?;
                if p.contains(&format!("\"rs{k}v\"")) {
                    seated[k] += 1;
                }
                // (G1 08:16 non-blocking) the NoAI flag read back every tick, not once
                let n = srv.cmd_long(&format!("data get entity @e[tag=rs{k}v,limit=1] NoAI"))?;
                if n.contains("1b") == (r.rider == Some(false)) {
                    noai_ok[k] += 1;
                }
            } else if r.v >= 0.0 {
                let z = srv.cmd_long(&format!("data get entity @e[tag=rs{k},limit=1] Pos[2]"))?;
                let z: Option<f64> = z.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('d').parse().ok());
                if z.is_some_and(|z| (1.0..2.0).contains(&z)) {
                    in_cell[k] += 1;
                }
            }
        }
        let feet = srv.cmd_long("data get entity @e[tag=rs0v,limit=1] Pos[1]")?;
        let feet: Option<f64> = feet.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches('d').parse().ok());
        println!(
            "    t{t:>2}: {line}   (rider #1's feet y {}: {} the band's floor {}.0625)",
            feet.map(|f| format!("{f:.4}")).unwrap_or_else(|| "-".into()),
            feet.map(|f| if f < (Y + 1) as f64 + 0.0625 { "below" } else { "above" }).unwrap_or("-"),
            Y + 1
        );
    }
    let first = |v: &Vec<bool>| v.iter().position(|&b| b).map(|f| f as u32 + 1);
    for (k, (r, s)) in rows.iter().zip(&seen).enumerate() {
        let back = match r.rider {
            Some(_) => format!("; its villager seated on {} of 60 ticks, NoAI read back {}", seated[k], if noai[k] == Some(true) { "1b" } else { "absent (AI on)" }),
            None if r.v >= 0.0 => format!("; the cart's z inside the string's cell on {} of 60 ticks", in_cell[k]),
            None => String::new(),
        };
        println!("  => {}: pressed on {} of 60 ticks{}{back}", r.label, s.iter().filter(|&&b| b).count(), first(s).map(|f| format!(", first at t{f}")).unwrap_or_else(|| ", NEVER".into()));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let class = |pred: &dyn Fn(&RowSpec) -> bool| rows.iter().zip(&seen).filter(|(r, _)| pred(r)).map(|(_, s)| first(s)).collect::<Vec<_>>();
    for (name, ai) in [("AI", true), ("NoAI", false)] {
        println!("  first-press ticks, resting {name}: {:?}; rolling {name}: {:?}", class(&|r| r.rider == Some(ai) && r.v == 0.0), class(&|r| r.rider == Some(ai) && r.v > 0.0));
    }
    let resting: Vec<&Vec<bool>> = rows.iter().zip(&seen).filter(|(r, _)| r.rider.is_some() && r.v == 0.0).map(|(_, s)| s).collect();
    let resting_empty: Vec<&Vec<bool>> = rows.iter().zip(&seen).filter(|(r, _)| r.rider.is_none() && r.v == 0.0).map(|(_, s)| s).collect();
    let rolling_ridden: Vec<&Vec<bool>> = rows.iter().zip(&seen).filter(|(r, _)| r.rider.is_some() && r.v > 0.0).map(|(_, s)| s).collect();
    let never: Vec<&Vec<bool>> = rows.iter().zip(&seen).filter(|(r, _)| r.rider.is_none() && r.v != 0.0).map(|(_, s)| s).collect();
    pin(1, resting.len() == 16 && resting.iter().all(|s| s.iter().all(|&b| b)), "(1) every resting ridden cart's rider presses its string at t1, AI on and NoAI alike, and holds it pressed all 60 ticks (16 rows)".into())?;
    pin(2, resting_empty.len() == 2 && resting_empty.iter().all(|s| s.iter().all(|&b| !b)), "(2) no resting EMPTY cart ever presses its string (its box tops out 0.300 below the band; 2 rows)".into())?;
    pin(3, rolling_ridden.len() == 16 && rolling_ridden.iter().all(|s| s.iter().any(|&b| b)), "(3) every rolling ridden cart presses its string while passing, AI on and NoAI (16 rows)".into())?;
    pin(4, never.len() == 3 && never.iter().all(|s| s.iter().all(|&b| !b)), "(4) the rolling empty carts and the no-cart control never press theirs (3 rows)".into())?;
    // (G1 06:54 non-blocking) the AI-vs-NoAI comparison, asserted as the game shows it over runs: each rolling AI row
    // first presses within ONE tick of its NoAI twin (identical in most runs; an AI rider's own move control can nudge
    // its cart, one row a tick early in one plant run of car 12)
    let ai_ticks = class(&|r| r.rider == Some(true) && r.v > 0.0);
    let noai_ticks = class(&|r| r.rider == Some(false) && r.v > 0.0);
    let within_one = ai_ticks.len() == 8 && noai_ticks.len() == 8 && ai_ticks.iter().zip(&noai_ticks).all(|(a, b)| matches!((a, b), (Some(a), Some(b)) if a.abs_diff(*b) <= 1));
    pin(5, within_one, format!("(5) every rolling AI rider first presses within one tick of its NoAI twin: {ai_ticks:?} / {noai_ticks:?}"))?;
    // the read-backs: every ridden row's villager seated all 60 ticks with its NoAI flag as summoned
    let back_ok = rows.iter().enumerate().filter(|(_, r)| r.rider.is_some()).all(|(k, r)| seated[k] == 60 && noai_ok[k] == 60 && noai[k] == Some(r.rider == Some(false)));
    pin(6, back_ok, "(6) every ridden row's villager is read back SEATED in its own cart on all 60 ticks, NoAI 1b exactly where summoned NoAI".into())?;
    // the controls sat or passed under the string: a resting empty in the string's cell all 60 ticks, a rolling one in it
    // on at least one tick
    let ctl_ok = rows.iter().enumerate().filter(|(_, r)| r.rider.is_none() && r.v >= 0.0).all(|(k, r)| if r.v == 0.0 { in_cell[k] == 60 } else { in_cell[k] >= 1 });
    pin(7, ctl_ok, "(7) every control cart is read back under the string: the resting empties inside its cell (z 1 .. 2) on all 60 ticks, the rolling ones passing through it".into())?;
    println!("  every known answer held (asserts 1..7)");
    Ok(())
}
