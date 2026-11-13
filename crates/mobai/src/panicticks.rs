//! (tickexact 2026-11-12, ironspec 19:51 PT: the 60-gt behaviour's skip) `mctas mobai panic-ticks`:
//! on the official 26.3 server, `--villagers` villagers seated in carts 2 apart on a stone floor,
//! a husk in a boat 5 from them (in sight: they panic every tick), day time 13000 held; the brain
//! agent (with `+panic`) prints every entry of VillagerPanicTrigger's own `start` and `tick`
//! (villager, timestamp). Over `--ticks` gt (with `--restart-at` one save/stop/start), per
//! villager: its starts, the ticks with no `tick` call between its first start and the end (the
//! skips), each skip against the start before it (start + 61?), whether the restart after a skip
//! ticks at once, and every summon tick S (game time % 100 == 0) with or without a call.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 100;

fn opts(version: &str, jar: &std::path::Path, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}=minecraft:golem_detected_recently,+panic", jar.display()))),
        ..Default::default()
    }
}

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

pub fn panic_ticks(version: &str, villagers: u32, ticks: u32, restart_at: Option<u32>) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&opts(version, &jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("time set 13000")?;
    srv.cmd_ok("forceload add -16 -16 16 16")?;
    srv.step(20)?;
    srv.cmd_ok(&format!("fill -2 {} -2 8 {} {} minecraft:stone", Y - 1, Y - 1, 2 * villagers as i32 + 2))?;
    let mut uuids = Vec::new();
    for i in 0..villagers as i32 {
        let z = 2 * i;
        srv.cmd(&format!("setblock 0 {Y} {z} minecraft:rail"))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 0.5 {Y}.0625 {z}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"pt{i}\"]}}]}}"
        ))?;
        let u = srv.cmd_long(&format!("data get entity @e[tag=pt{i},limit=1] UUID"))?;
        let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
        anyhow::ensure!(ints.len() == 4, "the UUID of pt{i}: {u}");
        let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
        uuids.push(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]));
    }
    srv.cmd_ok(&format!("summon minecraft:oak_boat 5.5 {Y} {}.5 {{Passengers:[{{id:\"minecraft:husk\",PersistenceRequired:1b}}]}}", villagers as i32 - 1))?;
    println!(
        "== panic-ticks on the official {version} server: {villagers} villagers seated in carts, a husk in a boat in sight, day time 13000 held; VillagerPanicTrigger's start and tick entries every tick for {ticks} gt{}",
        restart_at.map(|r| format!("; a restart at t{r}")).unwrap_or_default()
    );
    let mut log = Log { at: 0 };
    let mut hooked = false;
    let mut starts: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut calls: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut restarted_at = None;
    let t0 = srv.game_time()?;
    for t in 1..=ticks {
        if restart_at == Some(t) {
            // the probe: the first panicking villager (panic-ticks reads game timestamps, so its
            // rows are immune to uncounted ticks; the check is printed for the record)
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, &jar, Some(w)), Some("@e[type=minecraft:villager,limit=1,sort=nearest,x=0,y=64,z=0]"))?;
            srv = s;
            println!("  t{t}: restart ({})", rl.describe());
            srv.cmd_ok("forceload add -16 -16 16 16")?;
            log.at = 0;
            restarted_at = Some(srv.game_time()?);
        }
        srv.step(1)?;
        for l in log.drain(&srv)? {
            if l.starts_with("VillagerPanicTrigger reports") {
                hooked = true;
                continue;
            }
            let Some(rest) = l.strip_prefix("panic ") else { continue };
            let mut it = rest.split(' ');
            let (Some(what), Some(_by), Some(who), Some(_at), Some(time)) = (it.next(), it.next(), it.next(), it.next(), it.next().and_then(|s| s.parse::<i64>().ok())) else { continue };
            let map = if what == "start" { &mut starts } else { &mut calls };
            map.entry(who.to_string()).or_default().push(time);
        }
    }
    let t_end = srv.game_time()?;
    anyhow::ensure!(hooked, "the brain agent never reported its VillagerPanicTrigger hook");
    for (i, u) in uuids.iter().enumerate() {
        let st = starts.get(u).cloned().unwrap_or_default();
        let ca = calls.get(u).cloned().unwrap_or_default();
        let Some(&first) = st.first() else {
            println!("  villager {i}: never started");
            continue;
        };
        let have: std::collections::BTreeSet<i64> = ca.iter().copied().collect();
        let skips: Vec<i64> = (first..=t_end).filter(|t| !have.contains(t)).collect();
        // each skip against the start before it, and whether the next start ticked at once
        let mut rel = Vec::new();
        for &s in &skips {
            let before = st.iter().copied().filter(|&x| x < s).max();
            let after = st.iter().copied().find(|&x| x > s);
            rel.push(format!(
                "{s} (start {} + {}; next start {}{})",
                before.map(|b| b.to_string()).unwrap_or("-".into()),
                before.map(|b| (s - b).to_string()).unwrap_or("-".into()),
                after.map(|a| format!("{a} = skip + {}", a - s)).unwrap_or("-".into()),
                after.map(|a| if have.contains(&a) { ", ticked at once" } else { ", NO tick at its start" }).unwrap_or("")
            ));
        }
        let summons: Vec<String> = (first..=t_end).filter(|t| t % 100 == 0).map(|s| format!("{s}{}", if have.contains(&s) { " call" } else { " NO CALL" })).collect();
        println!(
            "  villager {i}: {} starts (first {first}), {} tick calls from {first} to {t_end}; skips: {}; summon ticks: {}",
            st.len(),
            ca.len(),
            if rel.is_empty() { "none".to_string() } else { rel.join("; ") },
            summons.join(", ")
        );
        let periods: Vec<i64> = st.windows(2).map(|w| w[1] - w[0]).collect();
        println!("    start-to-start periods: {periods:?}");
    }
    if let Some(r) = restarted_at {
        println!("  (the restart: the world back at game time {r}; the server started at {t0})");
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
