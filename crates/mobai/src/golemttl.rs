//! (tickexact 2026-11-13, ironspec 00:00 / 00:30 PT: TTL memories across a world open) `mctas mobai
//! golem-ttl`: on the official 26.3 server, one golem stand as golem-pad lays it (a glass floor, three
//! villagers in carts, their last sleep recent, panicking at a husk in a boat 5 away; a 5 x 3
//! cobblestone pad, the only cells a golem can spawn on; every golem killed on its 2nd tick). The
//! first golem's tick is T (a panic summon: game time % 100 == 0). With a restart, the world is
//! saved, stopped and started at T + `--restart-after` (restartcheck::reload); each villager's
//! GOLEM_DETECTED_RECENTLY (value and remaining ttl) is printed before the save and after the load,
//! and the next golem's tick relative to T. From T + 500 every slot is printed: the trio's memories
//! at the tick before it, and whether a golem came or the summon found no spot (the brain agent's
//! `no spot by` line from Villager.spawnGolemIfNeeded's trySpawnMob). At T the agent's
//! GOLEM_DETECTED_RECENTLY sets are printed in their order, each with its villager (v0..v2) and its
//! caller. `--boundary m`: at T + 100 the trio's memories are set (data merge) so that each clears
//! m ticks before the slot S = T + 400, and the restart (if any) comes at T + 200, inside the window.
//! (26.3: MemorySlot.tick counts a ttl down once per brain tick; Brain.pack saves the remaining ttl;
//! a world open's idle ticks run no brain.)

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:golem_detected_recently";

fn opts(version: &str, jar: &std::path::Path, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
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

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

fn golems(srv: &mut VanillaServer) -> Result<usize> {
    Ok(srv.cmd_long("execute as @e[type=minecraft:iron_golem,tag=!gt_old] run data get entity @s Pos")?.matches("entity data: ").count())
}

fn memories(srv: &mut VanillaServer) -> Result<String> {
    let mut out = Vec::new();
    for i in 0..3 {
        let s = srv.cmd_long(&format!("data get entity @e[tag=gtv{i},limit=1] Brain.memories.\"minecraft:golem_detected_recently\""))?;
        // (G1 on 6ec22ffd8 #6) strict: a failed reply fails, never reads as "absent"
        let v = if let Some(r) = s.split("entity data: ").nth(1) {
            r.trim().to_string()
        } else if s.contains("Found no elements matching") {
            "none".to_string()
        } else {
            anyhow::bail!("v{i}'s golem_detected_recently: unreadable reply: {}", s.trim());
        };
        out.push(format!("v{i} {v}"));
    }
    Ok(out.join("; "))
}

/// The ttls in a `memories` string, in order (v0, v1, v2): each must be present and parse (G1 on G
/// #5), so an unreadable value can't pass a before / after comparison as "both absent".
fn ttls(m: &str) -> Result<Vec<i64>> {
    let v: Vec<i64> = m
        .split("; ")
        .map(|p| {
            let r = p.split("ttl:").nth(1).ok_or_else(|| anyhow::anyhow!("no ttl in `{p}`"))?;
            let digits: String = r.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<i64>().map_err(|_| anyhow::anyhow!("an unparsable ttl in `{p}`"))
        })
        .collect::<Result<_>>()?;
    anyhow::ensure!(v.len() == 3, "{} ttl(s) in `{m}` (the trio's three expected)", v.len());
    Ok(v)
}

/// A brain-agent line with its villager named (v0..v2) instead of its UUID.
fn named(l: &str, uuids: &[String]) -> String {
    let mut s = l.to_string();
    for (i, u) in uuids.iter().enumerate() {
        s = s.replace(&format!("@{u}"), &format!("[v{i}]")).replace(u.as_str(), &format!("v{i}"));
    }
    s
}

/// One run: the first golem's game time T, the next golem's offset from T (None if none came), the
/// notes, whether the restart happened, and the trio's ttls before / after the load.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn run(version: &str, jar: &std::path::Path, restart_after: Option<u32>, boundary: Option<u32>, ticks: u32, covered: bool, edit_after_load: bool) -> Result<(Option<i64>, Option<i64>, Vec<String>, bool, Option<(Vec<i64>, Vec<i64>)>, usize)> {
    let mut srv = VanillaServer::start(&opts(version, jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("time set 1000")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 16 24")?;
    srv.step(200)?;
    srv.cmd_ok(&format!("fill -10 {} -10 10 {} 14 minecraft:glass", Y - 1, Y - 1))?;
    // the pad (golem-pad's): 5 x 3 cobblestone on the glass, the only cells a golem can spawn on
    // (LEGACY_IRON_GOLEM never spawns on glass); `covered` (the no-spot control) leaves it glass
    if !covered {
        srv.cmd_ok(&format!("fill 2 {Y} 0 4 {Y} 4 minecraft:cobblestone"))?;
    }
    let mut uuids = Vec::new();
    for (i, dz) in [0, 2, 4].iter().enumerate() {
        srv.cmd(&format!("setblock 0 {Y} {dz} minecraft:rail"))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart 0.5 {}.0625 {dz}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"gtv{i}\"],Brain:{{memories:{{\"minecraft:last_slept\":{{value:900L}}}}}}}}]}}",
            Y
        ))?;
        uuids.push(uuid_of(&mut srv, &format!("@e[tag=gtv{i},limit=1]"))?);
    }
    srv.cmd_ok(&format!("summon minecraft:oak_boat -4.5 {Y} 2.5 {{Passengers:[{{id:\"minecraft:husk\",PersistenceRequired:1b}}]}}"))?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let mut notes = Vec::new();
    let (mut first, mut next) = (None::<i64>, None::<i64>);
    let mut restarted = false;
    let mut across: Option<(Vec<i64>, Vec<i64>)> = None;
    let mut no_spot_lines = 0usize;
    let restart_at = if boundary.is_some() { restart_after.map(|_| 200) } else { restart_after };
    for _ in 0..ticks {
        let gt = srv.game_time()?;
        if let Some(t0) = first {
            // the boundary row's memories, set at T + 100 so each clears m ticks before T + 400
            if let Some(m) = boundary
                && gt == t0 + 100
            {
                let l = 400 - m as i64 - 100 - 1;
                for i in 0..3 {
                    srv.cmd_ok(&format!("data merge entity @e[tag=gtv{i},limit=1] {{Brain:{{memories:{{\"minecraft:golem_detected_recently\":{{value:1b,ttl:{l}L}}}}}}}}"))?;
                }
                notes.push(format!("T + 100: the trio's memories set to ttl {l} (each clears at its brain tick T + {}, {m} before the slot T + 400): {}", 100 + l + 1, memories(&mut srv)?));
            }
            if let Some(r) = restart_at
                && !restarted
                && gt == t0 + r as i64
            {
                let before = memories(&mut srv)?;
                notes.push(format!("before the save (T + {r}): {before}"));
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, jar, Some(w)), None)?;
                srv = s;
                log = Log { at: 0 };
                log.drain(&srv)?;
                srv.cmd_ok("forceload add -16 -16 16 24")?;
                notes.push(format!("the load: {}", rl.describe()));
                // (G1 on G #5) the ttl-edit control: one ttl changed after the load, which the
                // before / after comparison must catch
                if edit_after_load {
                    let t0v = ttls(&memories(&mut srv)?)?[0];
                    srv.cmd_ok(&format!("data merge entity @e[tag=gtv0,limit=1] {{Brain:{{memories:{{\"minecraft:golem_detected_recently\":{{value:1b,ttl:{}L}}}}}}}}", t0v - 7))?;
                    notes.push(format!("CONTROL: v0's ttl edited after the load ({t0v} -> {})", t0v - 7));
                }
                let after = memories(&mut srv)?;
                notes.push(format!("after the load (game time {}): {after}", srv.game_time()?));
                across = Some((ttls(&before)?, ttls(&after)?));
                restarted = true;
            }
            // from T + 500 (or the boundary's slot), the trio's memories the tick before each slot
            let watch_from = if boundary.is_some() { t0 + 300 } else { t0 + 500 };
            if gt >= watch_from && (gt + 1).rem_euclid(100) == 0 && next.is_none() {
                notes.push(format!("slot T + {}: the tick before it: {}", gt + 1 - t0, memories(&mut srv)?));
            }
        }
        srv.step(1)?;
        let t = srv.game_time()?;
        let lines = log.drain(&srv)?;
        no_spot_lines += lines.iter().filter(|l| l.starts_with("no spot by ")).count();
        if covered && t > 600 {
            break;
        }
        srv.cmd("kill @e[type=minecraft:iron_golem,tag=gt_old]")?;
        let born = golems(&mut srv)? > 0;
        if let Some(t0) = first
            && next.is_none()
            && t.rem_euclid(100) == 0
            && t > t0
        {
            let spot: Vec<String> = lines.iter().filter(|l| l.starts_with("no spot by ")).map(|l| named(l, &uuids)).collect();
            notes.push(format!("slot T + {}: {}", t - t0, if born { "a golem".to_string() } else if spot.is_empty() { "no golem, no summon call failed for a spot (no call)".to_string() } else { format!("no golem: {}", spot.join(", ")) }));
        }
        if born {
            match first {
                None => {
                    first = Some(t);
                    notes.push(format!("the first golem at game time {t} (% 100 = {})", t.rem_euclid(100)));
                    let sets: Vec<String> = lines.iter().filter(|l| l.contains("golem_detected_recently")).map(|l| named(l, &uuids)).collect();
                    notes.push(format!("T: the agent's lines in order: {}", sets.join(" | ")));
                    notes.push(format!("T: memories after the tick: {}", memories(&mut srv)?));
                }
                Some(t0) if next.is_none() => {
                    next = Some(t - t0);
                    let out = srv.cmd("tag @e[type=minecraft:iron_golem,tag=!gt_old] add gt_old")?;
                    anyhow::ensure!(out.contains("Added tag"), "tagging the golem: {}", out.trim());
                    break;
                }
                _ => {}
            }
            // (G1 on 6ec22ffd8 #6) the setup's replies read: an untagged golem would count twice
            let out = srv.cmd("tag @e[type=minecraft:iron_golem,tag=!gt_old] add gt_old")?;
            anyhow::ensure!(out.contains("Added tag"), "tagging the golem: {}", out.trim());
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok((first, next, notes, restarted, across, no_spot_lines))
}

pub fn golem_ttl(version: &str, restart_after: u32, runs: u32, ticks: u32, boundary: Option<u32>) -> Result<()> {
    println!(
        "== golem-ttl on the official {version} server: one golem stand (a glass floor, three panicking villagers in carts, a husk in a boat 5 away, a 5 x 3 cobblestone pad; every golem killed on its 2nd tick); T the first golem's game time; {}; the next golem's tick relative to T; {runs} run(s) each",
        match boundary {
            Some(m) => format!("BOUNDARY rows: the trio's memories set at T + 100 to clear {m} tick(s) before the slot T + 400; with a restart at T + 200 (inside the window) and without"),
            None => format!("with a restart at T + {restart_after} (save, stop, start) and without"),
        }
    );
    // (G1 on 6ec22ffd8 #3 / #9) no 0-sample run; the boundary's ttl must be a positive count
    anyhow::ensure!(runs >= 1, "--runs 0: no sample (NOT JUDGED)");
    if let Some(m) = boundary {
        anyhow::ensure!(m < 299, "--boundary {m}: the memories' ttl 400 - m - 101 would be {} (it must be positive): refused", 400 - m as i64 - 101);
    }
    let jar = mctas::bridge::brain_agent(version)?;
    // (G1 #6) the no-spot control: the pad left glass, the trio panicking: the first summon must print
    // the agent's `no spot by` line, or "no spot (no call)" can't be told from a missing call
    let (_, _, _, _, _, ctl_spots) = run(version, &jar, None, None, 700, true, false)?;
    println!("  the no-spot control (the pad covered: glass): `no spot by` lines {ctl_spots}");
    anyhow::ensure!(ctl_spots > 0, "the no-spot control printed no `no spot by` line: the 'no call' reading can't be trusted: the run fails");
    // (G1 on G #5) the ttl-edit control: a restart row whose v0 ttl is edited after the load must be
    // caught by the before / after comparison
    let (_, _, ctl_notes, ctl_restarted, ctl_across, _) = run(version, &jar, Some(restart_after.min(300)), None, restart_after.min(300) + 400, false, true)?;
    let ctl_caught = ctl_restarted && ctl_across.as_ref().is_some_and(|(b, a)| b != a);
    println!("  the ttl-edit control (v0's ttl changed after the load): {}{}", if ctl_caught { "CAUGHT" } else { "NOT caught" }, ctl_across.map(|(b, a)| format!(" ({b:?} -> {a:?})")).unwrap_or_default());
    if !ctl_caught {
        for n in ctl_notes {
            println!("    {n}");
        }
    }
    anyhow::ensure!(ctl_caught, "the ttl-edit control was not caught: the across-the-load bar can't be trusted: the run fails");
    let mut summary = Vec::new();
    let mut not_judged = 0u32;
    let mut bad: Vec<String> = Vec::new();
    let mut nexts: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    for (row, (label, restart)) in [("control (no restart)", None), ("restart", Some(restart_after))].into_iter().enumerate() {
        for k in 1..=runs {
            let (first, next, notes, restarted, across, _) = run(version, &jar, restart, boundary, ticks, false, false)?;
            println!(
                "  {label}, run {k}: the first golem at {}; the next golem at T + {}",
                first.map(|t| format!("game time {t}")).unwrap_or_else(|| "none".into()),
                next.map(|d| d.to_string()).unwrap_or_else(|| format!("- (none within {ticks} gt): NOT JUDGED (0 samples)"))
            );
            for n in notes {
                println!("    {n}");
            }
            summary.push(format!("{label} {}", next.map(|d| format!("T+{d}")).unwrap_or("-".into())));
            if next.is_none() {
                not_judged += 1;
            }
            // (G1 #5 / #6) the bars: a restart row restarted; T and the next golem on the 100-gt
            // slots; the trio's ttls unchanged across the load
            if restart.is_some() && !restarted {
                bad.push(format!("{label} run {k}: the restart never happened"));
            }
            if let Some(t) = first
                && t.rem_euclid(100) != 0
            {
                bad.push(format!("{label} run {k}: T = {t} is not a panic slot (% 100 = {})", t.rem_euclid(100)));
            }
            if let Some(d) = next {
                if d.rem_euclid(100) != 0 {
                    bad.push(format!("{label} run {k}: the next golem at T + {d}, not a slot"));
                }
                nexts[row].push(d);
            }
            if let Some((b, a)) = across
                && b != a
            {
                bad.push(format!("{label} run {k}: the trio's ttls changed across the load: {b:?} -> {a:?}"));
            }
        }
    }
    println!("  next golem: {}", summary.join(", "));
    // (G1 #6) "a world open costs exactly one slot": every restart row's next golem one slot (100 gt)
    // after every control row's (the control's offset is the same in every run)
    if boundary.is_none() && !nexts[0].is_empty() && !nexts[1].is_empty() {
        let c = nexts[0][0];
        if nexts[0].iter().any(|&d| d != c) {
            bad.push(format!("the control rows disagree: {:?}", nexts[0]));
        }
        if nexts[1].iter().any(|&d| d != c + 100) {
            bad.push(format!("the restart rows' next golem {:?}, not one slot after the control's T + {c}", nexts[1]));
        }
        println!("  a world open costs exactly one summon slot (restart = control + 100): {}", if bad.iter().any(|b| b.contains("one slot") || b.contains("disagree")) { "NOT shown" } else { "HELD" });
    }
    // (G1 on G #5) the boundary rows' cross-row bar: the restart inside the window moves nothing (the
    // memories' ttls are saved), so every restart row's next golem equals the control rows'
    if boundary.is_some() && !nexts[0].is_empty() && !nexts[1].is_empty() {
        let c = nexts[0][0];
        if nexts[0].iter().chain(nexts[1].iter()).any(|&d| d != c) {
            bad.push(format!("the boundary rows' next golems differ: control {:?}, restart {:?}", nexts[0], nexts[1]));
        }
    }
    println!("  bars: {}", if bad.is_empty() { "HELD".to_string() } else { format!("FAILED: {}", bad.join("; ")) });
    // (ironspec 23:56 PT: no vacuous passes)
    anyhow::ensure!(not_judged == 0, "{not_judged} row(s) NOT JUDGED (0 samples): the run fails");
    anyhow::ensure!(bad.is_empty(), "{} bar(s) failed: {}", bad.len(), bad.join("; "));
    Ok(())
}
