//! (tickexact 2026-11-12, ironspec: the villager's golem detection range on 26.3) `mctas mobai
//! golem-range`: on the official 26.3 server, a villager seated in a cart (held on its rail) and an
//! iron golem held in place (NoAI, NoGravity) at a box gap from it (the golem's box edge to the
//! villager's, along one axis): horizontal and vertical gaps from `--gaps`, each pair its own copy
//! 256 blocks from the next; over `--ticks` gt (two or more GolemSensor and NearestLivingEntity
//! scans) each villager's GOLEM_DETECTED_RECENTLY sets and whether its NEAREST_LIVING_ENTITIES ever
//! held the golem.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:golem_detected_recently,minecraft:nearest_living_entities,minecraft:mobs";
/// The villager's box (0.6 wide, 1.95 tall) seated at the cart's position; the golem's (1.4 wide).
const V_HALF: f64 = 0.3;
const V_TALL: f64 = 1.95;
const G_HALF: f64 = 0.7;

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

pub fn golem_range(version: &str, gaps: &[f64], ticks: u32) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 6000")?;
    println!("== golem-range on the official {version} server: a villager seated in a cart and an iron golem (NoAI, NoGravity) at box gaps {gaps:?}, horizontal (east) and vertical (above); {ticks} gt");
    let mut rows: Vec<(String, f64, String, String)> = Vec::new();
    // every copy's chunks loaded first
    for k in 0..gaps.len() * 2 {
        let x0 = 256 * k as i32;
        srv.cmd_ok(&format!("forceload add {} -16 {} 16", x0 - 16, x0 + 64))?;
    }
    srv.step(40)?;
    let mut k = 0;
    for &gap in gaps {
        for axis in ["horizontal", "vertical"] {
            let x0 = 256 * k;
            k += 1;
            srv.cmd_ok(&format!("setblock {x0} {} 0 minecraft:stone", Y - 1))?;
            srv.cmd_ok(&format!("setblock {x0} {Y} 0 minecraft:rail"))?;
            let tag = format!("gv{k}");
            srv.cmd_ok(&format!("summon minecraft:minecart {x0}.5 {}.0625 0.5 {{Tags:[\"gc{k}\"],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}\"]}}]}}", Y))?;
            // the golem's centre: its box edge `gap` from the villager's
            let (gx, gy) = if axis == "horizontal" { (x0 as f64 + 0.5 + V_HALF + gap + G_HALF, Y as f64 + 0.0625) } else { (x0 as f64 + 0.5, Y as f64 + 0.0625 + V_TALL + gap) };
            srv.cmd_ok(&format!("summon minecraft:iron_golem {gx} {gy} 0.5 {{NoAI:1b,NoGravity:1b,PersistenceRequired:1b,Tags:[\"gg{k}\"]}}"))?;
            let u = uuid_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
            let g = uuid_of(&mut srv, &format!("@e[tag=gg{k},limit=1]"))?;
            rows.push((axis.to_string(), gap, u, g));
        }
    }
    let mut log_at = 0u64;
    let mut drain = |srv: &VanillaServer| -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(log_at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        log_at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    };
    drain(&srv)?;
    let mut detected: Vec<Option<u32>> = vec![None; rows.len()];
    let mut listed: Vec<Option<u32>> = vec![None; rows.len()];
    let mut scans = vec![0u32; rows.len()];
    for t in 1..=ticks {
        srv.step(1)?;
        for l in drain(&srv)? {
            let who = l.rsplit(" @").next().map(|u| u.trim()).unwrap_or("");
            let Some(i) = rows.iter().position(|r| r.2 == who) else { continue };
            if l.starts_with("set minecraft:golem_detected_recently") && detected[i].is_none() {
                detected[i] = Some(t);
            }
            if l.starts_with("set minecraft:nearest_living_entities") || l.starts_with("set minecraft:mobs") {
                scans[i] += 1;
                // the golem in the list: an IronGolem entry (the list prints entities by type and id)
                if listed[i].is_none() && (l.contains("IronGolem") || l.contains("Iron Golem") || l.contains("iron_golem")) {
                    listed[i] = Some(t);
                }
            }
        }
    }
    for (i, (axis, gap, _, _)) in rows.iter().enumerate() {
        println!(
            "  {axis} box gap {gap}: NEAREST_LIVING_ENTITIES set {} times, the golem in it {}; GOLEM_DETECTED_RECENTLY {}",
            scans[i],
            listed[i].map(|t| format!("from +{t} gt")).unwrap_or_else(|| "never".into()),
            detected[i].map(|t| format!("SET at +{t} gt")).unwrap_or_else(|| "never set".into())
        );
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
