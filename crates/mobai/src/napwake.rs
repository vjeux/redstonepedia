//! (tickexact 2026-11-12, farmopt/ironspec: A6's wake) `mctas mobai nap-wake`: on the official 26.3
//! server, the A6 stop alone (its file as committed: the hatch hiding, both levers on, the iron
//! trapdoors open, the zombie in its boat), at night (day time 13800, held); each trial a fresh
//! villager stood on the bed top (-3,1,4) with HOME the bed's head (-3,0,4): it lies down (SleepInBed),
//! and `--flip-after` gt after the lie-down F switches both levers off (the trapdoors close flat,
//! the hatch reveals); then every tick its NEAREST_HOSTILE (the brain agent), its sleep (sleeping_pos)
//! and, once it wakes, the cell it stands up on. Between trials the villager goes and the levers go
//! back on. `--trials` trials (each a fresh villager: new sensor phases).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const DY: i32 = 80;
const MEMS: &str = "minecraft:nearest_hostile";

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn levers(srv: &mut VanillaServer, on: bool) -> Result<()> {
    for p in [[-4, 0, 3], [-4, 1, 5]] {
        srv.cmd(&format!("setblock {} {} {} minecraft:lever[face=floor,facing=north,powered={on}]", p[0], p[1] + DY, p[2]))?;
    }
    Ok(())
}

pub fn nap_wake(version: &str, file: &std::path::Path, trials: u32, flip_after: u32, ticks: u32) -> Result<()> {
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
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.step(40)?;
    crate::seatclear::place_part(&mut srv, file)?;
    println!(
        "== nap-wake on the official {version} server: {} alone (the hatch hiding, the zombie in its boat), day time 13800; {trials} trials: a villager on the bed top, HOME the bed's head, the levers off {flip_after} gt after it lies down, {ticks} gt watched",
        file.display()
    );
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
    let (mut slept, mut woke, mut seen, mut leaks) = (0u32, 0u32, 0u32, 0u32);
    let mut cells: std::collections::BTreeMap<String, u32> = Default::default();
    for i in 0..trials {
        srv.cmd_ok("time set 13800")?;
        levers(&mut srv, true)?;
        srv.step(5)?;
        let tag = format!("nap{i}");
        srv.cmd_ok(&format!(
            "summon minecraft:villager -2.5 {} 4.5 {{PersistenceRequired:1b,Tags:[\"{tag}\"],Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;-3,{},4]}}}}}}}}}}",
            0.5625 + DY as f64,
            DY
        ))?;
        let sel = format!("@e[tag={tag},limit=1]");
        let who = uuid_of(&mut srv, &sel)?;
        drain(&srv)?;
        // the lie-down
        let mut lie = None;
        for t in 1..=400u32 {
            srv.step(1)?;
            for l in drain(&srv)? {
                if std::env::var_os("MCTAS_NAP_DEBUG").is_some() && l.ends_with(&format!("@{who}")) {
                    eprintln!("    t+{t} {l}");
                }
            }
            if std::env::var_os("MCTAS_NAP_DEBUG").is_some() && (t % 100 == 0 || t <= 12) {
                eprintln!("    t+{t} memories: {}", srv.cmd_long(&format!("data get entity {sel} Brain.memories"))?.trim());
                eprintln!("    t+{t} pos: {} sleeping: {}", srv.cmd_long(&format!("data get entity {sel} Pos"))?.trim(), srv.cmd_long(&format!("data get entity {sel} sleeping_pos"))?.trim());
            }
            if !srv.cmd_long(&format!("data get entity {sel} sleeping_pos"))?.contains("entity data") {
                continue;
            }
            lie = Some(t);
            break;
        }
        let Some(lie) = lie else {
            println!("  trial {i}: never lay down in 400 gt");
            srv.cmd(&format!("kill {sel}"))?;
            continue;
        };
        slept += 1;
        // the wait to F, the hatch hiding: any NEAREST_HOSTILE then is a leak
        let mut early = None::<u32>;
        for t in 1..=flip_after {
            srv.step(1)?;
            for l in drain(&srv)? {
                if l.starts_with("set minecraft:nearest_hostile") && l.ends_with(&format!("@{who}")) && early.is_none() {
                    early = Some(t);
                }
            }
        }
        if let Some(t) = early {
            println!("  trial {i}: NEAREST_HOSTILE set at lie-down + {t}, BEFORE F (the hatch hiding)");
            leaks += 1;
        }
        levers(&mut srv, false)?;
        let (mut hostile, mut wake, mut stand) = (None::<u32>, None::<u32>, String::new());
        for t in 1..=ticks {
            srv.step(1)?;
            for l in drain(&srv)? {
                if l.starts_with("set minecraft:nearest_hostile") && l.ends_with(&format!("@{who}")) && hostile.is_none() {
                    hostile = Some(t);
                }
            }
            if wake.is_none() && !srv.cmd_long(&format!("data get entity {sel} sleeping_pos"))?.contains("entity data") {
                wake = Some(t);
                let p = nums(&srv.cmd_long(&format!("data get entity {sel} Pos"))?);
                if p.len() == 3 {
                    stand = format!("({:.3},{:.3},{:.3}) cell ({},{},{})", p[0], p[1] - DY as f64, p[2], p[0].floor() as i32, (p[1] - DY as f64).floor() as i32, p[2].floor() as i32);
                    *cells.entry(format!("({},{},{})", p[0].floor() as i32, (p[1] - DY as f64).floor() as i32, p[2].floor() as i32)).or_default() += 1;
                }
                break;
            }
        }
        if hostile.is_some() {
            seen += 1;
        }
        if wake.is_some() {
            woke += 1;
        }
        println!(
            "  trial {i}: lay down at +{lie}; levers off at lie-down + {flip_after}; NEAREST_HOSTILE {}; {}",
            hostile.map(|t| format!("set at F + {t}")).unwrap_or_else(|| "never set".into()),
            wake.map(|t| format!("woke at F + {t} (lie-down + {}), stood up at {stand}", t + flip_after)).unwrap_or_else(|| format!("asleep after {ticks} gt")),
        );
        srv.cmd(&format!("kill {sel}"))?;
        srv.step(25)?;
    }
    println!("  {slept} of {trials} lay down; NEAREST_HOSTILE before F (hatch hiding) in {leaks}; after F in {seen}; woke in {woke}; stand-up cells {cells:?}");
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
