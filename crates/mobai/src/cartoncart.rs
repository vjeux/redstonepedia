//! (tickexact 2026-11-14, G1 00:00 / 00:01, ironspec 00:02: the FIFO's place) `mctas mobai cart-on-cart`: on the official
//! 26.3 server (OldMinecartBehavior), a minecart at rest on a plain rail at a landing (empty, or carrying a villager), and
//! a second minecart carrying a villager dropped from rest straight onto it down a 1x1 glass shaft from h above the
//! rail's top; every tick both carts' y, the falling cart's fall_distance and both riders' Health. G1's reading: the
//! falling cart lands ON the resting cart (carts collide: `canVehicleCollide`), so it never reaches the rail cell where
//! `moveAlongTrack` resets the fall (OldMinecartBehavior 99), and `checkFallDamage` passes the fall to its rider through
//! the supporting block's `fallOn` (the rail's default, Block 486-489: floor(d - 3)). Controls: the same drop with no
//! cart at the landing (cart-fall's rail landing: the rider unhurt), and dry stone with no rail (floor(d - 3)).
//! Heights 3, 4, 8, 16, 34. Asserted (MCTAS_CART_ON_CART_PLANT=k inverts assert k): (1) every falling cart comes to rest
//! within 160 gt; (2) the no-cart rail landings leave the rider at 20 HP at every height; (3) dry stone at 8 takes
//! exactly 5 (the cart-fall control); (4) onto a RIDDEN cart the falling cart rests on the lower rider's head (y +
//! 2.0125: its feet at the cart's y + 0.0625, 1.95 tall) and its own rider ends at 20 / 20 / 17 / 9 HP at h 3 / 4 / 8 /
//! 16 and DIES at 34 (the death itself: the named rider's death line in the server log, G1 03:14), the lower rider at 20; (5) onto an EMPTY cart it ends on the rail (y + 0.0625), its rider at 20.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y0: i32 = 100;

fn num(srv: &mut VanillaServer, cmd: &str) -> Result<Option<f64>> {
    let out = srv.cmd_long(cmd)?;
    Ok(out.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches(|c: char| c.is_alphabetic()).parse::<f64>().ok()))
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_CART_ON_CART_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Landing {
    /// a cart resting on the rail, empty
    CartEmpty,
    /// a cart resting on the rail, ridden
    CartRidden,
    /// the rail alone (the control)
    Rail,
    /// dry stone, no rail (the damage control)
    Stone,
}

pub fn cart_on_cart(version: &str) -> Result<()> {
    println!("== cart-on-cart on the official {version} server (OldMinecartBehavior): a ridden minecart dropped from rest down a 1x1 glass shaft onto a cart at rest on a plain rail (empty or ridden), the rail alone, or dry stone; heights above the landing's top (the rail's block top y {Y0})");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 95 15")?;
    srv.step(20)?;
    let mut cols: Vec<(Landing, f64)> = Vec::new();
    for l in [Landing::CartEmpty, Landing::CartRidden, Landing::Rail] {
        for h in [3.0, 4.0, 8.0, 16.0, 34.0] {
            cols.push((l, h));
        }
    }
    cols.push((Landing::Stone, 8.0));
    for (i, (l, h)) in cols.iter().enumerate() {
        let x = i as i32 * 4;
        srv.cmd_ok(&format!("setblock {x} {} 0 minecraft:stone", Y0 - 1))?;
        if *l != Landing::Stone {
            srv.cmd_ok(&format!("setblock {x} {Y0} 0 minecraft:rail[shape=north_south]"))?;
        }
        let top = Y0 + (*h as i32) + 3;
        srv.cmd_ok(&format!("fill {} {Y0} -1 {} {top} 1 minecraft:glass hollow", x - 1, x + 1))?;
        srv.cmd(&format!("fill {x} {} 0 {x} {top} 0 minecraft:air", Y0 + 1))?;
        if *l == Landing::Stone {
            srv.cmd(&format!("setblock {x} {Y0} 0 minecraft:air"))?;
        } else {
            srv.cmd_ok(&format!("setblock {x} {Y0} 0 minecraft:rail[shape=north_south]")).ok();
        }
    }
    srv.step(5)?;
    for (i, (l, _)) in cols.iter().enumerate() {
        let x = i as i32 * 4;
        match l {
            Landing::CartEmpty => {
                srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 {}.0625 0.5 {{Tags:[\"ccl{i}\"]}}", Y0))?;
            }
            Landing::CartRidden => {
                srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 {}.0625 0.5 {{Tags:[\"ccl{i}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"cclv{i}\"],PersistenceRequired:1b}}]}}", Y0))?;
            }
            _ => {}
        }
    }
    srv.step(10)?;
    for (i, (l, h)) in cols.iter().enumerate() {
        let x = i as i32 * 4;
        let base = if *l == Landing::Stone { Y0 as f64 } else { Y0 as f64 + 0.0625 };
        srv.cmd_ok(&format!("summon minecraft:minecart {x}.5 {:.4} 0.5 {{Tags:[\"ccf{i}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"ccfv{i}\"],CustomName:\"ccfv{i}\",PersistenceRequired:1b}}]}}", base + h))?;
    }
    let n = cols.len();
    let mut rest: Vec<Option<(u32, f64)>> = vec![None; n];
    let mut prev: Vec<Option<f64>> = vec![None; n];
    let mut still = vec![0u32; n];
    let mut hp_f: Vec<Option<f64>> = vec![None; n];
    // (G1 03:14) the falling rider's DEATH itself, not only "no Health any more" (which a dismount or a removal would
    // also give): the server log's death line for the named rider (below)
    let mut died_f = vec![false; n];
    let mut hp_l: Vec<Option<f64>> = vec![None; n];
    let mut fd_max = vec![0.0f64; n];
    let mut ly: Vec<Option<f64>> = vec![None; n];
    for t in 1..=160u32 {
        srv.step(1)?;
        for i in 0..n {
            let y = num(&mut srv, &format!("data get entity @e[tag=ccf{i},limit=1] Pos[1]"))?;
            let fd = num(&mut srv, &format!("data get entity @e[tag=ccf{i},limit=1] fall_distance"))?.unwrap_or(0.0);
            fd_max[i] = fd_max[i].max(fd);
            hp_f[i] = num(&mut srv, &format!("data get entity @e[tag=ccfv{i},limit=1] Health"))?;
            if cols[i].0 == Landing::CartRidden {
                hp_l[i] = num(&mut srv, &format!("data get entity @e[tag=cclv{i},limit=1] Health"))?;
            }
            if matches!(cols[i].0, Landing::CartEmpty | Landing::CartRidden) {
                ly[i] = num(&mut srv, &format!("data get entity @e[tag=ccl{i},limit=1] Pos[1]"))?;
            }
            if let (Some(y), Some(p)) = (y, prev[i]) {
                if (y - p).abs() < 1e-9 {
                    still[i] += 1;
                    if still[i] == 5 && rest[i].is_none() {
                        rest[i] = Some((t - 5, y));
                    }
                } else {
                    still[i] = 0;
                }
            }
            prev[i] = y;
        }
    }
    // (G1 03:14) the death itself: an entity selector never matches a dead entity, so the Health-0 read can't be made;
    // each falling rider is NAMED (CustomName = its tag), and the server logs a named entity's death with its cause
    // ("Named entity Villager['ccfvN'/..] died: .."): read from the server's own log
    let log = std::fs::read_to_string(srv.dir.join("logs").join("latest.log")).unwrap_or_default();
    let mut death_line: Vec<Option<String>> = vec![None; n];
    for (i, dl) in death_line.iter_mut().enumerate() {
        *dl = log.lines().find(|l| l.contains("Named entity") && l.contains(&format!("['ccfv{i}'/")) && l.contains(" died: ")).map(|l| l.split(" died: ").nth(1).unwrap_or("").to_string());
        if dl.is_some() {
            died_f[i] = true;
        }
    }
    let mut out = Vec::new();
    for (i, (l, h)) in cols.iter().enumerate() {
        let line = format!(
            "  {l:?} h {h:>4}: the falling cart rests {} (fall_distance max {:.3}); its rider {}; {}",
            rest[i].map_or("NEVER".into(), |(t, y)| format!("from t{t} at y {:.4} ({:+.4} above the rail block's top)", y, y - Y0 as f64)),
            fd_max[i],
            hp_f[i].map_or_else(|| format!("GONE{}", death_line[i].as_ref().map(|d| format!(" (the server log: died: {d})")).unwrap_or_else(|| " (no death in the server log)".into())), |h| format!("{h:.1} HP")),
            match l {
                Landing::CartEmpty => format!("the resting cart at y {}", ly[i].map_or("-".into(), |y| format!("{y:.4}"))),
                Landing::CartRidden => format!("the resting cart at y {}, its rider {}", ly[i].map_or("-".into(), |y| format!("{y:.4}")), hp_l[i].map_or("GONE (dead)".into(), |h| format!("{h:.1} HP"))),
                _ => String::new(),
            }
        );
        println!("{line}");
        out.push((*l, *h, rest[i], hp_f[i], hp_l[i]));
    }
    pin(1, out.iter().all(|o| o.2.is_some()), format!("(1) every falling cart comes to rest within 160 gt: {:?}", out.iter().map(|o| (o.0, o.1, o.2)).collect::<Vec<_>>()))?;
    pin(2, out.iter().filter(|o| o.0 == Landing::Rail).all(|o| o.3 == Some(20.0)), format!("(2) the rail landings with no cart leave the rider at 20 HP: {:?}", out.iter().filter(|o| o.0 == Landing::Rail).map(|o| (o.1, o.3)).collect::<Vec<_>>()))?;
    pin(3, out.iter().filter(|o| o.0 == Landing::Stone).all(|o| o.3 == Some(15.0)), format!("(3) dry stone at 8 takes exactly 5: {:?}", out.iter().filter(|o| o.0 == Landing::Stone).map(|o| o.3).collect::<Vec<_>>()))?;
    // (G1 02:09) the headline rows, pinned
    let ridden: Vec<(f64, Option<(u32, f64)>, Option<f64>, Option<f64>)> = out.iter().filter(|o| o.0 == Landing::CartRidden).map(|o| (o.1, o.2, o.3, o.4)).collect();
    let want = |h: f64| -> Option<f64> {
        match h as i32 {
            3 | 4 => Some(20.0),
            8 => Some(17.0),
            16 => Some(9.0),
            _ => None,
        }
    };
    // (G1 03:14) the h-34 rider's death asserted as a death (its death line in the server log), not as "no Health"
    let ridden_died: Vec<(f64, bool)> = cols.iter().enumerate().filter(|(_, c)| c.0 == Landing::CartRidden).map(|(i, c)| (c.1, died_f[i])).collect();
    pin(
        4,
        ridden.len() == 5
            && ridden.iter().zip(&ridden_died).all(|((h, rest, hp, low), (_, died))| rest.is_some_and(|(_, y)| (y - Y0 as f64 - 2.0125).abs() < 1e-3) && (if *h as i32 == 34 { *died && hp.is_none() } else { *hp == want(*h) && !*died }) && *low == Some(20.0)),
        format!("(4) onto a RIDDEN cart the falling cart rests on the lower rider's head (y + 2.0125) and its own rider ends at 20 / 20 / 17 / 9 HP at h 3 / 4 / 8 / 16 and DIES at 34 (its death line in the server log, \"died: ..\"), the lower rider at 20: {ridden:?}; deaths {ridden_died:?}"),
    )?;
    let empty: Vec<(f64, Option<(u32, f64)>, Option<f64>)> = out.iter().filter(|o| o.0 == Landing::CartEmpty).map(|o| (o.1, o.2, o.3)).collect();
    pin(5, empty.len() == 5 && empty.iter().all(|(_, rest, hp)| rest.is_some_and(|(_, y)| (y - Y0 as f64 - 0.0625).abs() < 1e-3) && *hp == Some(20.0)), format!("(5) onto an EMPTY cart the falling cart ends on the rail (y + 0.0625) and its rider at 20 at every height: {empty:?}"))?;
    println!("  every known answer held (asserts 1..5)");
    Ok(())
}
