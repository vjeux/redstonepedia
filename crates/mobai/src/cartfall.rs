//! (tickexact 2026-11-13, farmopt / ironspec 22:12-22:23, G1 22:15-22:19: the x-9 way-in drop) `mctas mobai cart-fall`: on
//! the official 26.3 server (a default world: no MINECART_IMPROVEMENTS feature, so OldMinecartBehavior; AbstractMinecart
//! 104-107, 605-607), a villager seated in a minecart dropped from rest down a 1x1 glass shaft onto each landing; every
//! tick the cart's y and fall_distance, the villager's y (seated = the rider's y equals the cart's: the seat is zero),
//! Health and its own fall_distance. 26.3's lines: OldMinecartBehavior.tick (50-64) applies gravity (0.04 dry, 0.005 in
//! water: AbstractMinecart 268-270), then `getCurrentBlockPosOrRailBelow` (AbstractMinecart 300-314: the cart's own cell,
//! or the cell below it when that is a rail); on a rail `moveAlongTrack`, which begins with `resetFallDistance()`
//! (OldMinecartBehavior 99) and puts the cart on the rail; off rails `comeOffTrack` (AbstractMinecart 375-387): `move`,
//! then x 0.95 drag when not on ground. So the stored fall speed tends to 0.04 x 0.95 / 0.05 = 0.76 and the move per tick
//! (gravity added before the move, drag after) is 0.8 x (1 - 0.95^n) < 0.8 blocks. `Entity.move` calls
//! `checkFallDamage` (1590-1610: the fall added; on landing the supporting block's `fallOn`, getOnPosLegacy = the main
//! supporting block), whose default (Block 486-489) is `causeFallDamage(d, 1.0)`; a minecart is not in FALL_DAMAGE_IMMUNE
//! (data tag) and AbstractMinecart / VehicleEntity override nothing (only MinecartTNT, to explode), so
//! `Entity.causeFallDamage` (1616-1630) passes the fall to its passengers (LivingEntity 1796-1821, 1855-1866: floor(d -
//! 3) x the block's multiplier); slime passes x 0 (SlimeBlock 15-19), hay x 0.2 (HayBlock 17-19). Water resets fall
//! distance (Entity 1689-1691). The rider's own fall distance cannot grow: `rideTick` zeroes its motion before its tick
//! (Entity 2444-2450). So a cart falling < 0.8 blocks a tick always starts some tick in the cell above the rail, is put on
//! the rail there with its fall reset, and no landing's `fallOn` runs.
//! Columns: rail landings (a plain rail on stone at 31 and 31.3; rev 3's arrival detector on GLASS at 34 and 34.5; a
//! detector on stone, 34; the queue's powered rail on slime with a torch under, 6 and 8); controls: dry stone 4 (1
//! damage), 8 (5), 31 (fatal); dry glass 34 (rev 3's column with its landing rail removed: fatal); bare slime 8 (0); a
//! water source at the landing, 34 (0).
//! Asserted (MCTAS_CART_FALL_PLANT=k inverts assert k): (1) every rail landing leaves the rider unhurt and seated on
//! every row; (2) dry stone 4 takes exactly 1 and 8 exactly 5; (3) dry stone 31 and dry glass 34 kill the rider; (4)
//! bare slime takes 0; (5) the water landing takes 0, its fall reset while the cart is still above the floor; (6) no
//! rider's own fall_distance ever leaves 0; (7) the dry columns never move down 0.8 blocks or more in a tick (0.8 x (1 -
//! 0.95^n)); (8) every rail
//! landing's reset is the rail lookup: on the row before it the cart is 0.5 .. 2 above the landing (the rail's cell or
//! the one above), and on the reset row it sits on the rail (+0.0625).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y0: i32 = 70;

#[derive(Clone, Copy, PartialEq)]
enum Land {
    Rail,
    Detector,
    DetectorOnGlass,
    BoosterOnSlime,
    DryStone,
    DryGlass,
    BareSlime,
    Water,
}

impl Land {
    fn rail(self) -> bool {
        matches!(self, Land::Rail | Land::Detector | Land::DetectorOnGlass | Land::BoosterOnSlime)
    }
}

struct Col {
    label: &'static str,
    land: Land,
    h: f64,
}

const COLS: [Col; 14] = [
    Col { label: "(0) rail on stone, 31 (V's way-in)", land: Land::Rail, h: 31.0 },
    Col { label: "(1) rail on stone, 31.3 (another phase)", land: Land::Rail, h: 31.3 },
    Col { label: "(2) detector rail on GLASS, 34 (rev 3's arrival detector as laid)", land: Land::DetectorOnGlass, h: 34.0 },
    Col { label: "(3) detector rail on GLASS, 34.5 (another phase)", land: Land::DetectorOnGlass, h: 34.5 },
    Col { label: "(4) detector rail on stone, 34", land: Land::Detector, h: 34.0 },
    Col { label: "(5) powered rail on slime + torch, 6 (the queue's booster)", land: Land::BoosterOnSlime, h: 6.0 },
    Col { label: "(6) powered rail on slime + torch, 8", land: Land::BoosterOnSlime, h: 8.0 },
    Col { label: "(7) control: dry stone, 4", land: Land::DryStone, h: 4.0 },
    Col { label: "(8) control: dry stone, 8", land: Land::DryStone, h: 8.0 },
    Col { label: "(9) control: dry stone, 31", land: Land::DryStone, h: 31.0 },
    Col { label: "(10) control: dry GLASS, 34 (rev 3's column, its landing rail removed)", land: Land::DryGlass, h: 34.0 },
    Col { label: "(11) control: bare slime, 8", land: Land::BareSlime, h: 8.0 },
    Col { label: "(12) control: a water source at the landing, 34", land: Land::Water, h: 34.0 },
    Col { label: "(13) control: dry stone, 34", land: Land::DryStone, h: 34.0 },
];

fn num(srv: &mut VanillaServer, cmd: &str) -> Result<Option<f64>> {
    let out = srv.cmd_long(cmd)?;
    Ok(out.split("entity data: ").nth(1).and_then(|s| s.trim().trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse().ok()))
}

fn build(srv: &mut VanillaServer, i: usize, c: &Col) -> Result<()> {
    let x = i as i32 * 4;
    let top = Y0 + c.h.ceil() as i32 + 3;
    srv.cmd_ok(&format!("fill {} {} -1 {} {top} 1 minecraft:glass", x - 1, Y0 - 3, x + 1))?;
    srv.cmd_ok(&format!("fill {x} {Y0} 0 {x} {top} 0 minecraft:air"))?;
    let set = |srv: &mut VanillaServer, y: i32, s: &str| -> Result<()> { srv.cmd_ok(&format!("setblock {x} {y} 0 {s}")).map(|_| ()) };
    match c.land {
        Land::Rail => {
            set(srv, Y0 - 1, "minecraft:stone")?;
            set(srv, Y0, "minecraft:rail[shape=north_south]")?;
        }
        Land::Detector => {
            set(srv, Y0 - 1, "minecraft:stone")?;
            set(srv, Y0, "minecraft:detector_rail[shape=north_south]")?;
        }
        Land::DetectorOnGlass => {
            // (the shaft's fill already put glass under the landing)
            set(srv, Y0, "minecraft:detector_rail[shape=north_south]")?;
        }
        Land::BoosterOnSlime => {
            set(srv, Y0 - 3, "minecraft:stone")?;
            set(srv, Y0 - 2, "minecraft:redstone_torch")?;
            set(srv, Y0 - 1, "minecraft:slime_block")?;
            set(srv, Y0, "minecraft:powered_rail[shape=north_south]")?;
        }
        Land::DryStone => set(srv, Y0 - 1, "minecraft:stone")?,
        Land::DryGlass => {}
        Land::BareSlime => set(srv, Y0 - 1, "minecraft:slime_block")?,
        Land::Water => {
            set(srv, Y0 - 1, "minecraft:stone")?;
            set(srv, Y0, "minecraft:water")?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Seen {
    max_speed: f64,
    rider_fd_max: f64,
    /// (row, the cart's height above the landing on the row before, on the reset row)
    reset: Option<(u32, f64, f64)>,
    health_end: Option<f64>,
    unseated: Option<u32>,
    seated_end: bool,
    rest: Option<u32>,
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = std::env::var("MCTAS_CART_FALL_PLANT").ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

pub fn cart_fall(version: &str) -> Result<()> {
    println!("== cart-fall on the official {version} server (a default world: OldMinecartBehavior): a villager seated in a minecart dropped from rest down a 1x1 glass shaft; the landing's top at y {Y0}; heights are the cart's start above it");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 63 15")?;
    srv.step(20)?;
    for (i, c) in COLS.iter().enumerate() {
        build(&mut srv, i, c)?;
    }
    srv.step(5)?;
    for (i, c) in COLS.iter().enumerate() {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {}.5 {} 0.5 {{Tags:[\"cfc{i}\"],Passengers:[{{id:\"minecraft:villager\",Tags:[\"cfv{i}\"],PersistenceRequired:1b}}]}}",
            i * 4,
            Y0 as f64 + c.h
        ))?;
    }
    let n = COLS.len();
    let mut seen: Vec<Seen> = (0..n).map(|_| Seen::default()).collect();
    let mut prev_y: Vec<Option<f64>> = vec![None; n];
    let mut still: Vec<u32> = vec![0; n];
    let mut fd_prev: Vec<f64> = vec![0.0; n];
    for tick in 1..=140u32 {
        srv.step(1)?;
        let mut line = format!("  t{tick:>3}:");
        for i in 0..n {
            let y = num(&mut srv, &format!("data get entity @e[tag=cfc{i},limit=1] Pos[1]"))?;
            let fd = num(&mut srv, &format!("data get entity @e[tag=cfc{i},limit=1] fall_distance"))?.unwrap_or(0.0);
            let ry = num(&mut srv, &format!("data get entity @e[tag=cfv{i},limit=1] Pos[1]"))?;
            let hp = num(&mut srv, &format!("data get entity @e[tag=cfv{i},limit=1] Health"))?;
            let rfd = num(&mut srv, &format!("data get entity @e[tag=cfv{i},limit=1] fall_distance"))?.unwrap_or(0.0);
            let s = &mut seen[i];
            let is_reset = fd == 0.0 && fd_prev[i] > 0.0 && s.reset.is_none();
            if let (Some(y), Some(p)) = (y, prev_y[i]) {
                let dy = p - y;
                if !is_reset {
                    s.max_speed = s.max_speed.max(dy);
                }
                if dy.abs() < 1e-9 {
                    still[i] += 1;
                    if s.rest.is_none() {
                        s.rest = Some(tick - 1);
                    }
                } else {
                    still[i] = 0;
                }
            }
            if is_reset && let (Some(y), Some(p)) = (y, prev_y[i]) {
                s.reset = Some((tick, p - Y0 as f64, y - Y0 as f64));
            }
            fd_prev[i] = fd;
            s.rider_fd_max = s.rider_fd_max.max(rfd);
            s.health_end = hp;
            if hp.is_some() && s.unseated.is_none() && !(y.is_some() && ry.is_some() && (y.unwrap() - ry.unwrap()).abs() < 1e-6) {
                s.unseated = Some(tick);
            }
            prev_y[i] = y;
            line.push_str(&format!(
                " | {i}: y {} fd {fd:.2} hp {} rfd {rfd:.1}",
                y.map(|y| format!("{:.3}", y - Y0 as f64)).unwrap_or_else(|| "-".into()),
                hp.map(|h| format!("{h:.0}")).unwrap_or_else(|| "-".into())
            ));
        }
        println!("{line}");
        if still.iter().all(|s| *s >= 15) {
            break;
        }
    }
    for (i, s) in seen.iter_mut().enumerate() {
        let seated = srv.cmd_long(&format!("execute as @e[tag=cfv{i},limit=1] on vehicle if entity @s[type=minecraft:minecart]"))?;
        s.seated_end = seated.contains("Test passed");
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    for (c, s) in COLS.iter().zip(seen.iter()) {
        println!(
            "  {}: at rest from t{}; fastest fall {:.4} b/t (the snap row aside); the cart's fall_distance reset {}; the rider: health at the end {}, seated at the end {}, unseated on {}; its own fall_distance max {:.2}",
            c.label,
            s.rest.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
            s.max_speed,
            s.reset.map(|(t, b, a)| format!("on t{t} (the cart {b:+.3} above the landing the row before, {a:+.4} on the row)")).unwrap_or_else(|| "never".into()),
            s.health_end.map(|h| format!("{h:.1}")).unwrap_or_else(|| "gone (dead)".into()),
            s.seated_end,
            s.unseated.map(|t| format!("t{t}")).unwrap_or_else(|| "no row".into()),
            s.rider_fd_max
        );
    }
    let rails: Vec<usize> = (0..n).filter(|&i| COLS[i].land.rail()).collect();
    let dry: Vec<usize> = (0..n).filter(|&i| matches!(COLS[i].land, Land::DryStone | Land::DryGlass | Land::BareSlime)).collect();
    pin(1, rails.iter().all(|&i| seen[i].health_end == Some(20.0) && seen[i].seated_end && seen[i].unseated.is_none()), format!("(1) every rail landing leaves the rider unhurt (20) and seated on every row: {:?}", rails.iter().map(|&i| (seen[i].health_end, seen[i].seated_end, seen[i].unseated)).collect::<Vec<_>>()))?;
    pin(2, seen[7].health_end == Some(19.0) && seen[8].health_end == Some(15.0), format!("(2) dry stone 4 takes exactly 1, dry stone 8 exactly 5: health {:?} / {:?}", seen[7].health_end, seen[8].health_end))?;
    pin(3, [9usize, 10, 13].iter().all(|&i| seen[i].health_end.is_none_or(|h| h <= 0.0)), format!("(3) dry stone 31 / 34 and dry glass 34 kill the rider: health {:?}", [9usize, 10, 13].iter().map(|&i| seen[i].health_end).collect::<Vec<_>>()))?;
    pin(4, seen[11].health_end == Some(20.0), format!("(4) bare slime takes 0: health {:?}", seen[11].health_end))?;
    pin(5, seen[12].health_end == Some(20.0) && seen[12].reset.is_some_and(|(_, _, a)| a > 0.0), format!("(5) the water landing takes 0, its fall reset while the cart is still above the floor: health {:?}, reset {:?}", seen[12].health_end, seen[12].reset))?;
    pin(6, seen.iter().all(|s| s.rider_fd_max == 0.0), format!("(6) no rider's own fall_distance leaves 0: {:?}", seen.iter().map(|s| s.rider_fd_max).collect::<Vec<_>>()))?;
    pin(7, dry.iter().all(|&i| seen[i].max_speed < 0.8), format!("(7) the dry columns never move down 0.8 blocks or more in a tick (0.8 x (1 - 0.95^n)): {:?}", dry.iter().map(|&i| format!("{:.4}", seen[i].max_speed)).collect::<Vec<_>>()))?;
    pin(8, rails.iter().all(|&i| seen[i].reset.is_some_and(|(_, b, a)| (0.5..2.0).contains(&b) && (a - 0.0625).abs() < 1e-6)), format!("(8) every rail landing's reset is the rail lookup (the cart 0.5 .. 2 above the landing the row before, on the rail +0.0625 on the reset row): {:?}", rails.iter().map(|&i| seen[i].reset).collect::<Vec<_>>()))?;
    println!("  every known answer held (asserts 1..8)");
    Ok(())
}
