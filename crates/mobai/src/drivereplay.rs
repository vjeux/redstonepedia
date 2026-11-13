//! (tickexact 2026-11-13, ironspec 06:08 / 06:24 / 06:31 / 06:36 real msg-ids, onechunk's drive v1)
//! `mctas mobai drive-replay`: the composed night on the official 26.3 server, driven OPEN LOOP by
//! onechunk's f1q2-run act list (`--drive-out`) over its scene (`--scene-out`: the design with every
//! rider's cart as an entity line). The scene's blocks are pasted as bed-trap pastes them (+80 y,
//! fluids placed fresh), its entity lines summoned in file order, tagged e<N> (the vehicle) and
//! e<N>p<K> (its K-th passenger) so the drive's `eN.pK` names it. The day time at t = 0 is
//! --day-start, then let run (advance_time on), weather clear. Each act is applied before the tick
//! it names (t from the drive's t0): `home eN.pK x y z` (Brain HOME, memory only: no ticket),
//! `last-slept eN.pK d` (LAST_SLEPT = now + d), `set x y z STATE` (a setblock that must change its
//! cell, the override rule; before -> after printed the first time per cell). The brain agent (+poi)
//! names every line's villager.
//!
//! Bars, each with its numbers: (1) REST ticks at night (day time 12000..23999) of any villager whose
//! latest HOME path is not complete (partial, or none computed yet) -- a villager on a complete HOME
//! path is a napper's trip, excluded and counted; (2) no CANT_REACH stamp ever reaching 1,000 gt (the
//! oldest per villager printed, and each villager's stamp at every REST entry and at dawn); (3) no
//! villager in REST within 16 of the bed's centre (ValidateNearbyPoi's own measure) while the bed is
//! occupied and it isn't the sleeper; (4) no HOME lost (each loss printed with its mechanism and
//! position); (5) every set act changed its cell. OPEN LOOP: the drive carries no recorded state per
//! act, so G1's divergence check cannot run; the controller's coverage is NOT JUDGED by this row.

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const DY: i32 = 80;
const MEMS: &str = "minecraft:path,minecraft:walk_target,minecraft:nearest_hostile,minecraft:cant_reach_walk_target_since,minecraft:home,minecraft:last_slept,+poi,+sight,+hurt";

struct Act {
    t: u32,
    verb: String,
    args: Vec<String>,
}

/// The drive file: its t0 and its acts, sorted by t (stable: file order within a tick).
fn parse_drive(path: &Path) -> Result<(u32, Vec<Act>)> {
    let text = std::fs::read_to_string(path)?;
    let mut t0 = None;
    let mut acts = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if let Some(v) = l.strip_prefix("# t0 ") {
            t0 = Some(v.trim().parse::<u32>()?);
        } else if let Some(rest) = l.strip_prefix("act ") {
            let mut w = rest.split_whitespace();
            let t: u32 = w.next().ok_or_else(|| anyhow::anyhow!("line {}: act with no t", i + 1))?.parse()?;
            let verb = w.next().ok_or_else(|| anyhow::anyhow!("line {}: act with no verb", i + 1))?.to_string();
            let args: Vec<String> = w.map(|s| s.to_string()).collect();
            anyhow::ensure!(["home", "last-slept", "set"].contains(&verb.as_str()), "line {}: unknown act verb {verb}", i + 1);
            acts.push(Act { t, verb, args });
        } else if !l.is_empty() && !l.starts_with('#') {
            bail!("line {}: neither a comment nor an act: {l}", i + 1);
        }
    }
    let t0 = t0.ok_or_else(|| anyhow::anyhow!("{}: no `# t0` line", path.display()))?;
    acts.sort_by_key(|a| a.t);
    anyhow::ensure!(!acts.is_empty(), "{}: no acts", path.display());
    Ok((t0, acts))
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

/// An entity's UUID as the agent prints it (hyphenated hex).
fn uuid_hex(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

/// The entity line's NBT with the replay's tags: the vehicle e<n>, each passenger compound (`{id:"`)
/// e<n>p<k> in order. Returns the NBT and the passengers' kinds.
fn tagged(nbt: &str, n: usize) -> (String, Vec<String>) {
    let s = if nbt.trim().is_empty() { "{}".to_string() } else { nbt.trim().to_string() };
    let body = s.strip_prefix('{').unwrap_or(&s);
    let out = if body.trim_start().starts_with('}') { format!("{{Tags:[\"e{n}\"]{body}") } else { format!("{{Tags:[\"e{n}\"],{body}") };
    let mut kinds = Vec::new();
    let mut k = 0;
    let mut res = String::new();
    let mut rest = out.as_str();
    while let Some(i) = rest.find("{id:\"") {
        res.push_str(&rest[..i]);
        let after = &rest[i + 5..];
        let kind: String = after.chars().take_while(|c| *c != '"').collect();
        kinds.push(kind);
        res.push_str(&format!("{{Tags:[\"e{n}p{k}\"],id:\""));
        k += 1;
        rest = after;
    }
    res.push_str(rest);
    (res, kinds)
}

fn cell_state(srv: &mut VanillaServer, p: [i32; 3]) -> Result<String> {
    let q = [p[0], p[1] + DY, p[2]];
    let snap = srv.snapshot(q, q)?;
    Ok(snap.region.get(q).unwrap_or("?").to_string())
}

fn pos_of(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1] - DY as f64, v[2]]) } else { None })
}

fn fmt_pos(p: Option<[f64; 3]>) -> String {
    p.map(|p| format!("({:.2},{:.2},{:.2})", p[0], p[1], p[2])).unwrap_or_else(|| "?".into())
}

#[derive(Default)]
struct V {
    tag: String,
    gone: Option<String>,
    hurts: Vec<String>,
    /// the least air supply read (every 20 gt): (air, tick, position)
    min_air: Option<(i32, u32, String)>,
    activity: String,
    hostile: bool,
    /// the latest HOME path: (complete, start node)
    home_path: Option<(bool, String)>,
    stamp: Option<i64>,
    oldest: Option<(i64, u32)>,
    rest_partial: u32,
    rest_complete: u32,
    rest_entries: Vec<String>,
    stamps_armed: Vec<String>,
    home_lost: Vec<String>,
    home_set: Vec<String>,
    dawn: Vec<String>,
    least_occupied: Option<(f64, u32)>,
}

pub fn drive_replay(version: &str, scene: &Path, drive: &Path, ticks: u32, day_start: i64, bed: [i32; 3], loads: &[u32]) -> Result<()> {
    let (t0, acts) = parse_drive(drive)?;
    println!(
        "== drive-replay on the official {version} server: the scene {} (+{DY} y), the drive {} ({} acts, t0 {t0}) OPEN LOOP, the day time {day_start} at t 0, {ticks} gt (past the drive's last act the controller is idle), weather clear; the bed head ({},{},{}); world opens (save, stop, start) before t {loads:?}",
        scene.display(),
        drive.display(),
        acts.len(),
        bed[0],
        bed[1],
        bed[2]
    );
    println!("  OPEN LOOP: the drive carries no recorded state per act, so the divergence check (the rider / cart each act assumes) is UNAVAILABLE: the controller's coverage is NOT JUDGED by this row; its bars judge the mechanics it reaches");
    let jar = mctas::bridge::brain_agent(version)?;
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}", jar.display()))),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.cmd_ok("time set 13800")?;
    crate::seatclear::place_blocks(&mut srv, scene)?;
    let d = mctas::design::Design::load(scene)?;
    let mut by_uuid: BTreeMap<String, usize> = BTreeMap::new();
    let mut vs: Vec<V> = Vec::new();
    for (n, e) in d.entities.iter().enumerate() {
        let (nbt, kinds) = tagged(e.nbt.as_deref().unwrap_or(""), n);
        let out = srv.cmd(&format!("summon {} {} {} {} {nbt}", e.kind, e.pos[0], e.pos[1] + DY as f64, e.pos[2]))?;
        anyhow::ensure!(out.contains("Summoned new"), "entity line {n} ({} at {:?}): {}", e.kind, e.pos, out.trim());
        for (k, kind) in kinds.iter().enumerate() {
            if kind == "minecraft:villager" {
                let tag = format!("e{n}p{k}");
                let u = uuid_hex(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                by_uuid.insert(u, vs.len());
                vs.push(V { tag, ..Default::default() });
            }
        }
    }
    println!("  the scene's entity lines summoned: {}; villagers named: {} ({})", d.entities.len(), vs.len(), vs.iter().map(|v| v.tag.as_str()).collect::<Vec<_>>().join(" "));
    srv.cmd_ok(&format!("time set {day_start}"))?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule advance_time true")?;
    let gt0 = srv.game_time()?;
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let centre = [bed[0] as f64 + 0.5, bed[1] as f64 + 0.5, bed[2] as f64 + 0.5];
    let home_target = format!("BlockPos{{x={}, y={}, z={}}}", bed[0], bed[1] + DY, bed[2]);
    let mut noop: Vec<String> = Vec::new();
    let mut first_seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut events: Vec<String> = Vec::new();
    let mut collisions: Vec<String> = Vec::new();
    let (mut ai, mut applied, mut night_ticks) = (0usize, 0usize, 0u32);
    // (ironspec 07:30 / G1) after each world open, every line of every villager for 60 gt (the blip)
    let mut detail_until: Option<u32> = None;
    let mut detail: Vec<String> = Vec::new();
    // (ironfarm / ironspec 07:30 (3)(a)) each lane's cart count (y 16..26, z 0..8 of its column) and
    // the slope-top V (y >= 21.5, z >= 5.5) every 20 gt
    let lanes = [-1i32, 3, 7];
    let mut lane_max = [0u32; 3];
    let mut v_ticks = [0u32; 3];
    let mut lane_hist: Vec<String> = Vec::new();
    let mut activity_rows: Vec<String> = Vec::new();
    let mut reloaded = false;
    for t in 0..=ticks {
        if loads.contains(&t) {
            reloaded = true;
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(Some(w)), None)?;
            srv = s;
            log.at = 0;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            srv.cmd_ok("gamerule advance_time true")?;
            srv.cmd_ok("weather clear 1000000")?;
            let day_now = srv.cmd_long("time query daytime")?;
            events.push(format!("t{t}: WORLD OPEN ({}); {}", rl.describe(), day_now.trim()));
            detail_until = Some(t + 60);
            // a world open wipes every codec-less memory: the tracked activity, hostile, stamp, path
            for v in vs.iter_mut() {
                v.stamp = None;
                v.hostile = false;
                v.home_path = None;
            }
        }
        while ai < acts.len() && acts[ai].t == t {
            let a = &acts[ai];
            match a.verb.as_str() {
                "home" | "last-slept" => {
                    let who = a.args.first().ok_or_else(|| anyhow::anyhow!("t{t}: {} with no entity", a.verb))?.replace('.', "");
                    if a.verb == "home" {
                        anyhow::ensure!(a.args.len() == 4, "t{t}: home eN.pK x y z");
                        let p: Vec<i32> = a.args[1..].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}", p[0], p[1] + DY, p[2]))?;
                    } else {
                        anyhow::ensure!(a.args.len() == 2, "t{t}: last-slept eN.pK d");
                        let dt: i64 = a.args[1].parse()?;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:last_slept\":{{value:{}L}}}}}}}}", gt0 + t as i64 + dt))?;
                    }
                }
                "set" => {
                    anyhow::ensure!(a.args.len() == 4, "t{t}: set x y z STATE");
                    let p: Vec<i32> = a.args[..3].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                    let cell = [p[0], p[1], p[2]];
                    // (the harness's snapshot toggles a save marker per chunk, tracked per server
                    // instance: after a world open a new instance can't read it; not read then)
                    let before = if first_seen.insert(cell) { Some(if reloaded { "(not read after a world open)".to_string() } else { cell_state(&mut srv, cell)? }) } else { None };
                    let out = srv.cmd(&format!("setblock {} {} {} {}", p[0], p[1] + DY, p[2], a.args[3]))?;
                    if !out.contains("Changed the block") {
                        // (G1) a no-op act is a DIVERGENCE signal: the game's cell already held the
                        // act's state, so (as redsim's write changed it) the two worlds differ here
                        noop.push(format!("t{t} DIVERGENCE at ({},{},{}): the game's cell already held {} before the act ({})", p[0], p[1], p[2], a.args[3], out.trim()));
                    }
                    if let Some(b) = before {
                        println!("  first act at ({},{},{}), t{t}: {b} -> {}", p[0], p[1], p[2], a.args[3]);
                    }
                }
                _ => unreachable!(),
            }
            applied += 1;
            ai += 1;
        }
        srv.step(1)?;
        let now = gt0 + t as i64 + 1;
        let day = (day_start + t as i64 + 1).rem_euclid(24000);
        for l in log.drain(&srv)? {
            let Some(&vi) = l.rsplit(" @").next().and_then(|u| by_uuid.get(u.trim())) else {
                if l.starts_with("poi ") {
                    events.push(format!("t{t}: (no named villager's brain) {l}"));
                }
                continue;
            };
            let tag = vs[vi].tag.clone();
            let body = l.split(" @").next().unwrap_or(&l).to_string();
            if detail_until.is_some_and(|u| t <= u) {
                detail.push(format!("t{t}: {tag} {body}"));
            }
            if let Some(rest) = l.strip_prefix("activity ") {
                // Activity's toString is its bare name ("rest", "panic", "idle")
                let act = rest.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string();
                if act == "rest" {
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    let age = v.stamp.map(|s| (now - s).to_string()).unwrap_or_else(|| "none".into());
                    v.rest_entries.push(format!("t{t} at {} hostile {} stamp age {age}", fmt_pos(at), v.hostile));
                }
                events.push(format!("t{t}: {tag} activity {act}"));
                vs[vi].activity = act;
            } else if l.starts_with("set minecraft:nearest_hostile") {
                vs[vi].hostile = true;
            } else if l.starts_with("erase minecraft:nearest_hostile") {
                vs[vi].hostile = false;
            } else if l.starts_with("set minecraft:path = path to") {
                if l.contains(&home_target) {
                    let complete = l.contains("reach true");
                    let start = l.split(" start ").nth(1).and_then(|s| s.split(" end").next()).unwrap_or("?").to_string();
                    if !complete && vs[vi].activity == "rest" {
                        events.push(format!("t{t}: {tag} PARTIAL HOME path in REST, start {start}"));
                    }
                    vs[vi].home_path = Some((complete, start));
                }
            } else if l.contains("minecraft:cant_reach_walk_target_since") {
                if l.starts_with("set ") {
                    let s: Option<i64> = l.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
                    anyhow::ensure!(s.is_some(), "t{t}: unparsable stamp line: {l}");
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    v.stamp = s;
                    let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                    v.stamps_armed.push(format!("t{t} day {day} {} at {} by {by}", v.activity.trim_start_matches("minecraft:"), fmt_pos(at)));
                } else if l.starts_with("erase ") && l.contains("(had it)") {
                    vs[vi].stamp = None;
                }
            } else if l.starts_with("erase minecraft:home") {
                let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                // the bed's state and who sleeps in it at the erase (ValidateNearbyPoi's case)
                let occ = crate::strict::test(&mut srv, &format!("execute if block {} {} {} #minecraft:beds[occupied=true]", bed[0], bed[1] + DY, bed[2]))?;
                let mut sleepers = Vec::new();
                for w in vs.iter() {
                    if w.gone.is_none() && srv.count_entities(&format!("@e[tag={}]", w.tag))? > 0 && crate::strict::has_data(&mut srv, &format!("data get entity @e[tag={},limit=1] sleeping_pos", w.tag))? {
                        sleepers.push(w.tag.clone());
                    }
                }
                let dist = at.map(|p| ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt());
                let what = format!("t{t} by {by} at {} (distance to the bed's centre {}; the bed occupied {occ}; asleep: {})", fmt_pos(at), dist.map(|d| format!("{d:.2}")).unwrap_or_else(|| "?".into()), if sleepers.is_empty() { "nobody".to_string() } else { sleepers.join(" ") });
                vs[vi].home_lost.push(what.clone());
                events.push(format!("t{t}: {tag} HOME ERASED {}", what.trim_start_matches(&format!("t{t} "))));
            } else if l.starts_with("set minecraft:home") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                if !by.contains("makeBrain") && !by.contains("refreshBrain") {
                    vs[vi].home_set.push(format!("t{t} by {by}"));
                    events.push(format!("t{t}: {tag} HOME set by {by}"));
                }
            } else if l.starts_with("poi ") {
                events.push(format!("t{t}: {tag} {body}"));
            } else if body.starts_with("hurt ") {
                // (ironspec 06:22 platform) every damage a villager takes, drowning above all
                vs[vi].hurts.push(format!("t{t}: {body}"));
                events.push(format!("t{t}: {tag} {body}"));
            } else if let Some(s) = body.strip_prefix("sight at rest ") {
                // (ironspec 06:50 real) the REST entrant's Monsters within 16: distance, its own
                // hasLineOfSight, the clip's blocking cell
                let what = s.split_once("): ").map(|(_, r)| r).unwrap_or(s).to_string();
                if let Some(last) = vs[vi].rest_entries.last_mut() {
                    last.push_str(&format!("; sight: {what}"));
                }
                events.push(format!("t{t}: {tag} sight at its REST entry: {what}"));
            }
        }
        // per tick: the REST counters, the stamps' ages, the dawn reads
        let night = day >= 11990;
        if night && day >= 12000 {
            night_ticks += 1;
        }
        // (ironspec 06:13 platform (3)) every villager's activity per tick across day 11,990..12,040, and
        // the REST set at every 20-gt schedule tick through the night
        if (11990..=12040).contains(&day) || (night && day % 20 == 0) {
            let rest: Vec<&str> = vs.iter().filter(|v| v.activity == "rest" && v.gone.is_none()).map(|v| v.tag.as_str()).collect();
            if (11990..=12040).contains(&day) {
                activity_rows.push(format!("day {day}: {}", vs.iter().map(|v| format!("{}={}", v.tag, if v.activity.is_empty() { "?" } else { v.activity.as_str() })).collect::<Vec<_>>().join(" ")));
            } else if !rest.is_empty() {
                activity_rows.push(format!("day {day}: in REST {}", rest.join(" ")));
            }
        }
        for v in vs.iter_mut() {
            if let Some(s) = v.stamp {
                let age = now - s;
                if v.oldest.is_none_or(|(a, _)| age > a) {
                    v.oldest = Some((age, t));
                }
            }
            if night && v.activity == "rest" {
                match &v.home_path {
                    Some((true, _)) => v.rest_complete += 1,
                    _ => v.rest_partial += 1,
                }
            }
            if day == 0 {
                v.dawn.push(format!("t{t}: stamp {}", v.stamp.map(|s| format!("age {}", now - s)).unwrap_or_else(|| "none".into())));
            }
        }
        if t % 20 == 0 {
            // (a rider lost) every villager's presence and air supply; a disappearance with the
            // server log's death line
            for v in vs.iter_mut() {
                if v.gone.is_some() {
                    continue;
                }
                let air = srv.cmd_long(&format!("data get entity @e[tag={},limit=1] Air", v.tag))?;
                if air.contains("No entity was found") {
                    let log_text = std::fs::read_to_string(&srv.log).unwrap_or_default();
                    let deaths: Vec<&str> = log_text.lines().filter(|l| l.contains("died, message")).collect();
                    let why = deaths.last().map(|l| l.split("]: ").last().unwrap_or(l).to_string()).unwrap_or_else(|| "no death line in the log".into());
                    v.gone = Some(format!("t{t}: {why}"));
                    events.push(format!("t{t}: {} GONE ({why})", v.tag));
                    continue;
                }
                let a: i32 = air.split("data: ").nth(1).and_then(|s| s.trim().trim_end_matches('s').parse().ok()).ok_or_else(|| anyhow::anyhow!("{} Air unreadable: {}", v.tag, air.trim()))?;
                if a < 300 && v.min_air.as_ref().is_none_or(|m| a < m.0) {
                    let at = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))?;
                    v.min_air = Some((a, t, fmt_pos(at)));
                }
            }
            let mut row = Vec::new();
            for (i, &x) in lanes.iter().enumerate() {
                let n = srv.count_entities(&format!("@e[type=minecraft:minecart,x={x},y={},z=0,dx=0.99,dy=10,dz=8]", 16 + DY))?;
                let v = srv.count_entities(&format!("@e[type=minecraft:minecart,x={x},y={},z=5.5,dx=0.99,dy=4,dz=2.5]", 21.5 + DY as f64))?;
                lane_max[i] = lane_max[i].max(n);
                if v > 0 {
                    v_ticks[i] += 20;
                }
                row.push(format!("{n}{}", if v > 0 { "V" } else { "" }));
            }
            if t % 200 == 0 {
                lane_hist.push(format!("t{t}:{}", row.join("/")));
            }
        }
        if t % 100 == 0 {
            let occ = crate::strict::test(&mut srv, &format!("execute if block {} {} {} #minecraft:beds[occupied=true]", bed[0], bed[1] + DY, bed[2]))?;
            if occ {
                for v in vs.iter_mut() {
                    if v.gone.is_some() {
                        continue;
                    }
                    let Some(p) = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))? else { continue };
                    let dist = ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2)).sqrt();
                    if v.least_occupied.is_none_or(|(d, _)| dist < d) {
                        v.least_occupied = Some((dist, t));
                    }
                    if dist < 16.0 && v.activity == "rest" && !crate::strict::has_data(&mut srv, &format!("data get entity @e[tag={},limit=1] sleeping_pos", v.tag))? {
                        collisions.push(format!("t{t}: {} in REST at {dist:.2} from the occupied bed", v.tag));
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  acts applied: {applied} of {}; night ticks (day time 12000..23999) in the run: {night_ticks}", acts.len());
    for e in &events {
        println!("  {e}");
    }
    for d in &detail {
        println!("  after a world open: {d}");
    }
    for a in &activity_rows {
        println!("  activity {a}");
    }
    println!("  lanes (-1 / 3 / 7): the most carts in a lane at once {lane_max:?}; ticks with a cart at the slope top V {v_ticks:?}; every 200 gt (count, V marked): {}", lane_hist.join(" "));
    let (mut partial, mut complete, mut lost, mut old) = (0u32, 0u32, 0usize, 0usize);
    for v in &vs {
        partial += v.rest_partial;
        complete += v.rest_complete;
        lost += v.home_lost.len();
        if v.oldest.is_some_and(|(a, _)| a >= 1000) {
            old += 1;
        }
        println!(
            "  {}: night REST ticks without a complete HOME path {} (with one, a trip: {}); REST entries {:?}; stamps armed {:?}; oldest stamp {}; at dawn {:?}; HOME erased {:?}; HOME set {:?}; least distance to the occupied bed {}",
            v.tag,
            v.rest_partial,
            v.rest_complete,
            v.rest_entries,
            v.stamps_armed,
            v.oldest.map(|(a, t)| format!("{a} gt (t{t})")).unwrap_or_else(|| "none".into()),
            v.dawn,
            v.home_lost,
            v.home_set,
            v.least_occupied.map(|(d, t)| format!("{d:.2} (t{t})")).unwrap_or_else(|| "- (never occupied at a read)".into())
        );
    }
    let judged = night_ticks as u64 * vs.len() as u64;
    let gone: Vec<String> = vs.iter().filter_map(|v| v.gone.as_ref().map(|g| format!("{} {g}", v.tag))).collect();
    println!("  villagers lost: {} {:?}", gone.len(), gone);
    let drowning: Vec<String> = vs.iter().flat_map(|v| v.hurts.iter().filter(|h| h.contains("hurt drown")).map(move |h| format!("{} {h}", v.tag))).collect();
    println!(
        "  air: the least air supply read per villager (every 20 gt, < 300 only): {}; drowning damage taken: {} {:?}",
        vs.iter().filter_map(|v| v.min_air.as_ref().map(|m| format!("{} {} at t{} {}", v.tag, m.0, m.1, m.2))).collect::<Vec<_>>().join("; "),
        drowning.len(),
        drowning.iter().take(12).collect::<Vec<_>>()
    );
    println!(
        "  bars: (1) night REST ticks without a complete HOME path: {partial} of {judged} villager-ticks judged (trips excluded: {complete}) ({}); (2) villagers whose stamp reached 1,000 gt: {old} ({}); (3) collisions: {} ({}); (4) HOME erased: {lost} ({}); (5) no-op acts: {} ({})",
        if partial == 0 { "HELD" } else { "FAILED" },
        if old == 0 { "HELD" } else { "FAILED" },
        collisions.len(),
        if collisions.is_empty() { "HELD" } else { "FAILED" },
        if lost == 0 { "HELD" } else { "FAILED" },
        noop.len(),
        if noop.is_empty() { "HELD" } else { "FAILED" }
    );
    for c in &collisions {
        println!("    collision {c}");
    }
    for n in &noop {
        println!("    no-op act {n}");
    }
    anyhow::ensure!(judged > 0, "no night tick judged: NOT JUDGED, the run fails");
    anyhow::ensure!(applied == acts.len(), "only {applied} of {} acts applied within {ticks} gt", acts.len());
    anyhow::ensure!(partial == 0 && old == 0 && collisions.is_empty() && lost == 0 && noop.is_empty() && gone.is_empty() && drowning.is_empty(), "a bar failed (REST without a complete HOME path {partial}, stamps >= 1000 {old}, collisions {}, HOME erased {lost}, no-op acts {}, villagers lost {}, drowning hits {})", collisions.len(), noop.len(), gone.len(), drowning.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tagged;

    #[test]
    fn tags_vehicle_and_passengers() {
        let (s, k) = tagged("{Rotation:[0f,0f],Passengers:[{id:\"minecraft:villager\"}]}", 7);
        assert_eq!(s, "{Tags:[\"e7\"],Rotation:[0f,0f],Passengers:[{Tags:[\"e7p0\"],id:\"minecraft:villager\"}]}");
        assert_eq!(k, vec!["minecraft:villager".to_string()]);
        let (s, k) = tagged("", 3);
        assert_eq!(s, "{Tags:[\"e3\"]}");
        assert!(k.is_empty());
    }
}
