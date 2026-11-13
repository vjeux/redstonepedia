//! (tickexact 2026-11-13, ironspec 06:31 / 06:50 real msg-ids, 58f770cc's (A) bench) `mctas mobai bed-reclaim`:
//! the official (A)-geometry re-claim row. On the official 26.3 server the bed-stop file is pasted at
//! +80 y (its entities summoned), `--remove` cells set to air (each must change: the no-head-glass
//! variant). A napper with HOME on the bed head is armed first in a sealed glass cell at `--arm-at`
//! by night (REST: its HOME path partial, its CANT_REACH stamp set: the stamp a strand carries), then
//! by day stood on `--day-pos` (the bed foot's top) for 1,300 gt (the stamp past 1,200), then the
//! clock runs into dusk: at the first REST tick SetWalkTargetFromBlockMemory's release branch erases
//! HOME and frees the ticket, and the core AcquirePoi tries to re-claim. Printed per run: the stamp,
//! the release, every AcquirePoi attempt's path (+poi's acquire hook: start, end, nodes, reach),
//! every ticket line, the re-claim and the lie-down, HOME at the end; the verdict per run is
//! "re-claimed at t" or "NOT re-claimed within N gt" (and NOT JUDGED when no stamp or no release).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const DY: i32 = 80;
const MEMS: &str = "minecraft:path,minecraft:walk_target,minecraft:cant_reach_walk_target_since,minecraft:home,minecraft:nearest_hostile,+poi";

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

fn uuid_hex(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

#[allow(clippy::too_many_arguments)]
pub fn bed_reclaim(version: &str, file: &Path, remove: &[[i32; 3]], day_pos: [f64; 3], home: [i32; 3], arm_at: [f64; 3], runs: u32, after: u32) -> Result<()> {
    anyhow::ensure!(runs >= 1, "--runs 0: no sample (NOT JUDGED)");
    println!(
        "== bed-reclaim on the official {version} server: {} (+{DY} y){}; the napper armed by night in a glass cell at ({},{},{}), then stood by day on ({},{},{}) for 1300 gt, then into dusk and {after} gt on; HOME ({},{},{}); {runs} run(s)",
        file.display(),
        if remove.is_empty() { String::new() } else { format!(", removed: {}", remove.iter().map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" ")) },
        arm_at[0],
        arm_at[1],
        arm_at[2],
        day_pos[0],
        day_pos[1],
        day_pos[2],
        home[0],
        home[1],
        home[2]
    );
    let jar = mctas::bridge::brain_agent(version)?;
    let mut verdicts = Vec::new();
    for run in 1..=runs {
        let mut srv = VanillaServer::start(&VanillaOptions {
            version: version.to_string(),
            difficulty: "normal".into(),
            with_bot: false,
            biome: "minecraft:plains".into(),
            tick_rate: 1000,
            java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
            ..Default::default()
        })?;
        srv.cmd_ok("forceload add -32 -32 47 47")?;
        srv.cmd_ok("gamerule advance_time false")?;
        srv.cmd_ok("time set 13800")?;
        crate::seatclear::place_part(&mut srv, file)?;
        for p in remove {
            let before = srv.cmd_long(&format!("execute if block {} {} {} minecraft:air", p[0], p[1] + DY, p[2]))?;
            anyhow::ensure!(!before.contains("Test passed"), "--remove ({},{},{}): already air: the override changes nothing", p[0], p[1], p[2]);
            let out = srv.cmd(&format!("setblock {} {} {} minecraft:air", p[0], p[1] + DY, p[2]))?;
            anyhow::ensure!(out.contains("Changed the block"), "--remove ({},{},{}): {}", p[0], p[1], p[2], out.trim());
            if run == 1 {
                println!("  removed ({},{},{}) -> air", p[0], p[1], p[2]);
            }
        }
        // the arming cell: glass floor, walls and roof around a 1x2 space
        let (ax, ay, az) = (arm_at[0].floor() as i32, arm_at[1].floor() as i32 + DY, arm_at[2].floor() as i32);
        srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:glass hollow", ax - 1, ay - 1, az - 1, ax + 1, ay + 2, az + 1))?;
        srv.cmd_ok(&format!(
            "summon minecraft:villager {} {} {} {{Tags:[\"rcn\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}",
            arm_at[0],
            arm_at[1] + DY as f64,
            arm_at[2],
            home[0],
            home[1] + DY,
            home[2]
        ))?;
        let me = uuid_hex(&mut srv, "@e[tag=rcn,limit=1]")?;
        let mut log = Log { at: 0 };
        let mut lines: Vec<String> = Vec::new();
        let mut stamp = None;
        for t in 1..=300u32 {
            srv.step(1)?;
            for l in log.drain(&srv)? {
                if l.contains(&me) && l.starts_with("set minecraft:cant_reach_walk_target_since") && stamp.is_none() {
                    stamp = Some(t);
                    lines.push(format!("arm t{t}: {}", l.split(" @").next().unwrap_or(&l)));
                }
            }
            if stamp.is_some() {
                break;
            }
        }
        let Some(_) = stamp else {
            verdicts.push(format!("run {run}: NOT JUDGED (no stamp armed in the cell within 300 gt)"));
            let _ = std::fs::remove_dir_all(&srv.dir);
            continue;
        };
        // by day, first switched to IDLE inside the cell (a villager still in REST on the bed foot's top
        // would reach its HOME target and clear the stamp, or lie down), then stood on the bed foot's
        // top, the stamp aging past 1,200 (day held)
        srv.cmd_ok("time set 6000")?;
        srv.step(60)?;
        let mut idle = false;
        let mut cleared = false;
        for l in log.drain(&srv)? {
            if l.contains(&me) {
                if l.starts_with("activity idle") {
                    idle = true;
                }
                if l.starts_with("erase minecraft:cant_reach_walk_target_since (had it)") {
                    cleared = true;
                }
            }
        }
        if !idle || cleared {
            verdicts.push(format!("run {run}: NOT JUDGED (by day in the cell: idle {idle}, the stamp cleared {cleared})"));
            let _ = std::fs::remove_dir_all(&srv.dir);
            continue;
        }
        srv.cmd_ok(&format!("tp @e[tag=rcn,limit=1] {} {} {}", day_pos[0], day_pos[1] + DY as f64, day_pos[2]))?;
        srv.cmd_ok("data merge entity @e[tag=rcn,limit=1] {Motion:[0.0d,0.0d,0.0d]}")?;
        srv.step(1300)?;
        let mut early = Vec::new();
        for l in log.drain(&srv)? {
            if l.contains(&me) && (l.contains("minecraft:home") || l.contains("cant_reach") || l.starts_with("poi ") || l.starts_with("activity ")) {
                early.push(l.split(" @").next().unwrap_or(&l).to_string());
            }
        }
        let pos = srv.cmd_long("data get entity @e[tag=rcn,limit=1] Pos")?;
        lines.push(format!("by day: {} line(s) about HOME / the stamp / tickets {:?}; at dusk the napper at {}", early.len(), early, pos.split("data: ").nth(1).unwrap_or("?").trim()));
        // into dusk
        srv.cmd_ok("time set 11950")?;
        srv.cmd_ok("gamerule advance_time true")?;
        let (mut released, mut claimed, mut slept) = (None, None, None);
        let mut attempts = 0u32;
        for t in 1..=after {
            srv.step(1)?;
            for l in log.drain(&srv)? {
                if !l.contains(&me) {
                    if l.starts_with("poi ") {
                        lines.push(format!("+{t}: (another brain) {l}"));
                    }
                    continue;
                }
                let body = l.split(" @").next().unwrap_or(&l).to_string();
                if body.starts_with("erase minecraft:home") && released.is_none() {
                    released = Some(t);
                    lines.push(format!("+{t}: {body}"));
                } else if body.starts_with("acquire path") {
                    // AcquirePoi's lambda serves HOME and MEETING alike: only the attempts after the
                    // release are HOME's (a villager with HOME runs only MEETING's)
                    if released.is_some() {
                        attempts += 1;
                        if attempts <= 12 {
                            lines.push(format!("+{t}: {body}"));
                        }
                    }
                } else if body.starts_with("set minecraft:home") && claimed.is_none() && released.is_some() {
                    claimed = Some(t);
                    lines.push(format!("+{t}: {body}"));
                } else if body.starts_with("poi ") || body.starts_with("activity ") || body.contains("cant_reach") {
                    lines.push(format!("+{t}: {body}"));
                }
            }
            if slept.is_none() && t % 5 == 0 && crate::strict::has_data(&mut srv, "data get entity @e[tag=rcn,limit=1] sleeping_pos")? {
                slept = Some(t);
                lines.push(format!("+{t}: ASLEEP"));
            }
        }
        let home_end = crate::strict::has_data(&mut srv, "data get entity @e[tag=rcn,limit=1] Brain.memories.\"minecraft:home\"")?;
        let _ = std::fs::remove_dir_all(&srv.dir);
        println!("  run {run}:");
        for l in &lines {
            println!("    {l}");
        }
        verdicts.push(match (released, claimed) {
            (None, _) => format!("run {run}: NOT JUDGED (no release within {after} gt of the dusk)"),
            (Some(r), Some(c)) => format!("run {run}: released +{r}, re-claimed +{c} ({} gt later) after {attempts} AcquirePoi attempt(s); asleep {}; HOME at the end {home_end}", c - r, slept.map(|s| format!("+{s}")).unwrap_or_else(|| "never".into())),
            (Some(r), None) => format!("run {run}: released +{r}, NOT re-claimed within {after} gt ({attempts} AcquirePoi attempt(s)); asleep {}; HOME at the end {home_end}", slept.map(|s| format!("+{s}")).unwrap_or_else(|| "never".into())),
        });
    }
    for v in &verdicts {
        println!("  {v}");
    }
    let judged = verdicts.iter().filter(|v| !v.contains("NOT JUDGED")).count();
    println!("  {judged} of {runs} run(s) judged");
    anyhow::ensure!(judged > 0, "no run judged: the run fails");
    Ok(())
}
