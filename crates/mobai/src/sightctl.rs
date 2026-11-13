//! (tickexact 2026-11-13, G1 / ironspec 03:36 PT: a cheap confirmation, not a gate) `mctas mobai
//! sight-control`: on the official 26.3 server, a villager seated in a cart on a rail at (0,Y,0) and a
//! zombie in a boat 4 blocks east at (4.5,Y,0.5) (within the zombie's 8 in VillagerHostilesSensor),
//! night held; the column the eye-to-eye ray crosses at x 2 ((2,Y+1,0) and (2,Y+2,0)) is: open air
//! (the positive control), glass, stone, or an iron trapdoor open facing north (a partial collider
//! whose plate sits on the cell's south face, z 0.8125..1, off the ray at z 0.5). 60 gt each, every
//! tick: the brain agent's NEAREST_HOSTILE set / erase lines and VillagerPanicTrigger's lines for the
//! villager. Bars: open and the gap: NEAREST_HOSTILE set and a panic; glass and stone: neither.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:nearest_hostile,minecraft:walk_target";

struct Log {
    at: u64,
}

impl Log {
    fn drain(&mut self, srv: &VanillaServer) -> Result<Vec<String>> {
        let mut f = std::fs::File::open(&srv.log)?;
        f.seek(SeekFrom::Start(self.at))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let whole = s.rfind('\n').map(|i| i + 1).unwrap_or(0);
        self.at += whole as u64;
        Ok(s[..whole].lines().filter_map(|l| l.split("mctas brain agent: ").nth(1).map(|r| r.trim().to_string())).collect())
    }
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

/// One layout: (NEAREST_HOSTILE sets, panic lines) for the villager over `ticks`.
fn case(version: &str, jar: &std::path::Path, kind: &str, ticks: u32) -> Result<(usize, usize)> {
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    })?;
    for c in ["gamerule advance_time false", "gamerule spawn_mobs false", "time set 18000", "forceload add -16 -16 15 15"] {
        srv.cmd_ok(c)?;
    }
    srv.step(20)?;
    srv.cmd_ok(&format!("fill -3 {} -3 8 {} 3 minecraft:glass", Y - 1, Y - 1))?;
    srv.cmd_ok(&format!("setblock 0 {Y} 0 minecraft:rail"))?;
    let column = match kind {
        "open" => "minecraft:air",
        "glass" => "minecraft:glass",
        "solid" => "minecraft:stone",
        "gap" => "minecraft:iron_trapdoor[facing=north,half=bottom,open=true,powered=false,waterlogged=false]",
        other => anyhow::bail!("--cases open, glass, solid, gap; got {other}"),
    };
    for y in [Y + 1, Y + 2] {
        srv.cmd(&format!("setblock 2 {y} 0 {column}"))?;
    }
    srv.cmd_ok(&format!("summon minecraft:minecart 0.5 {Y} 0.5 {{Passengers:[{{id:\"minecraft:villager\",Tags:[\"sv\"],PersistenceRequired:1b}}]}}"))?;
    srv.cmd_ok(&format!("summon minecraft:oak_boat 4.5 {Y} 0.5 {{Passengers:[{{id:\"minecraft:zombie\",Tags:[\"sz\"],PersistenceRequired:1b}}]}}"))?;
    srv.step(2)?;
    let me = uuid_of(&mut srv, "@e[tag=sv,limit=1]")?;
    let eye = |srv: &mut VanillaServer, sel: &str| -> Result<String> {
        let out = srv.cmd_long(&format!("execute as {sel} at @s anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {{Tags:[\"eye\"]}}"))?;
        let _ = out;
        let p = srv.cmd_long("data get entity @e[tag=eye,limit=1] Pos")?;
        srv.cmd("kill @e[tag=eye]")?;
        Ok(p.split("entity data: ").nth(1).unwrap_or("?").trim().to_string())
    };
    let (ve, ze) = (eye(&mut srv, "@e[tag=sv,limit=1]")?, eye(&mut srv, "@e[tag=sz,limit=1]")?);
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let (mut sets, mut panics, mut lines) = (0usize, 0usize, Vec::new());
    for t in 1..=ticks {
        srv.step(1)?;
        for l in log.drain(&srv)? {
            if !l.contains(&me) {
                continue;
            }
            if l.starts_with("set minecraft:nearest_hostile") {
                sets += 1;
                lines.push(format!("t{t}: NEAREST_HOSTILE set"));
            } else if l.starts_with("erase minecraft:nearest_hostile") {
                lines.push(format!("t{t}: NEAREST_HOSTILE erased"));
            }
            if l.contains("VillagerPanicTrigger") {
                panics += 1;
                if panics == 1 {
                    lines.push(format!("t{t}: the panic (VillagerPanicTrigger)"));
                }
            }
        }
    }
    println!(
        "  {kind}: the column at x 2 = {column}; the villager's eye {ve}, the zombie's eye {ze} (frame y + {Y}); over {ticks} gt: NEAREST_HOSTILE sets {sets}, panic lines {panics}{}",
        if lines.is_empty() { String::new() } else { format!(": {}", lines.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }
    );
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok((sets, panics))
}

pub fn sight_control(version: &str, cases: &[String], ticks: u32) -> Result<()> {
    println!("== sight-control on the official {version} server: a seated villager and a zombie in a boat 4 east, night; the column the eye-to-eye ray crosses at x 2 varied; the brain agent's lines");
    let jar = mctas::bridge::brain_agent(version)?;
    let mut bad = Vec::new();
    for k in cases {
        let (sets, panics) = case(version, &jar, k, ticks)?;
        let sees = sets > 0 && panics > 0;
        let want = matches!(k.as_str(), "open" | "gap");
        if sees != want {
            bad.push(format!("{k}: {} (expected {})", if sees { "saw and panicked" } else { "no sight" }, if want { "sight and panic" } else { "none" }));
        }
    }
    println!("  every expectation: {}", if bad.is_empty() { "HELD".to_string() } else { format!("NOT HELD: {}", bad.join("; ")) });
    anyhow::ensure!(bad.is_empty(), "sight-control: {}", bad.join("; "));
    Ok(())
}
