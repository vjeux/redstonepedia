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
    /// (onechunk's closed-loop drive) redsim's state just before this write: `carts …; holds …;
    /// pockets …`, names as eN / eN.pK
    expect: Option<String>,
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
            acts.push(Act { t, verb, args, expect: None });
        } else if let Some(rest) = l.strip_prefix("# expect ") {
            // `# expect t x y z: …` follows the set act it describes
            let (head, text) = rest.split_once(": ").ok_or_else(|| anyhow::anyhow!("line {}: an expect line with no ': '", i + 1))?;
            let h: Vec<&str> = head.split_whitespace().collect();
            let last = acts.last_mut().ok_or_else(|| anyhow::anyhow!("line {}: an expect line before any act", i + 1))?;
            anyhow::ensure!(
                h.len() == 4 && last.verb == "set" && h[0] == last.t.to_string() && h[1..] == last.args[..3],
                "line {}: the expect line ({head}) doesn't follow its set act (t{} {} {:?})",
                i + 1,
                last.t,
                last.verb,
                last.args
            );
            last.expect = Some(text.trim().to_string());
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

/// (G1's divergence check) every scene cart's block (frame y) and rider, from the game.
fn cart_states(srv: &mut VanillaServer, carts: &[String]) -> Result<Vec<(String, [i32; 3], Option<String>)>> {
    let mut out = Vec::new();
    for c in carts {
        let Some(p) = pos_of(srv, &format!("@e[tag={c},limit=1]"))? else { continue };
        let r = srv.cmd_long(&format!("execute as @e[tag={c},limit=1] on passengers run data get entity @s Tags"))?;
        let rider = r.split('"').find(|s| s.starts_with('e') && s.contains('p') && s[1..].chars().all(|ch| ch.is_ascii_digit() || ch == 'p')).map(|s| s.to_string());
        out.push((c.clone(), [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32], rider));
    }
    Ok(out)
}

/// The game's state in the drive's `# expect` form for a write at `cell` (names without the dot),
/// each scene index renamed through `names` (the drive's own numbering, where it differs).
fn expect_form(states: &[(String, [i32; 3], Option<String>)], cell: [i32; 3], names: &BTreeMap<usize, usize>) -> String {
    let rn = |s: &str| -> String {
        // eN or eNpK
        let body = &s[1..];
        let (n, rest) = match body.find('p') {
            Some(i) => (&body[..i], &body[i..]),
            None => (body, ""),
        };
        match n.parse::<usize>() {
            Ok(k) => format!("e{}{rest}", names.get(&k).copied().unwrap_or(k)),
            Err(_) => s.to_string(),
        }
    };
    let who = |c: &str, r: &Option<String>| match r {
        Some(r) => format!("{} with {}", rn(c), rn(r)),
        None => rn(c),
    };
    let col: Vec<String> = states
        .iter()
        .filter(|(_, b, _)| b[0] == cell[0] && b[2] == cell[2] && (cell[1] - 1..=cell[1] + 1).contains(&b[1]))
        .map(|(c, b, r)| format!("{} at y{:+}", who(c, r), b[1] - cell[1]))
        .collect();
    let at = |q: [i32; 3]| states.iter().find(|(_, b, _)| *b == q).map(|(c, _, r)| who(c, r)).unwrap_or_else(|| "none".into());
    format!(
        "carts {}; holds -1:{} 3:{} 7:{}; pockets -1:{} 3:{} 7:{}",
        if col.is_empty() { "none".to_string() } else { col.join(", ") },
        at([-1, 16, 0]),
        at([3, 16, 0]),
        at([7, 16, 0]),
        at([-1, 0, 0]),
        at([3, 0, 0]),
        at([7, 0, 0])
    )
}

#[derive(Default)]
struct Spell {
    start: u32,
    at: String,
    hostile: bool,
    wt_at_entry: Option<String>,
    sink_stop: Option<u32>,
    home_set: Option<(u32, String)>,
    home_path: Option<(bool, String)>,
    armed: Option<u32>,
    released: Option<(u32, String)>,
    sight: String,
}

impl Spell {
    fn describe(&self, end: Option<u32>) -> String {
        format!(
            "REST t{}..{} ({} gt) at {} hostile {}; WALK_TARGET at entry {}; the sink's stop {}; HOME targeted {}; its path {}; stamp armed {}; released {}{}",
            self.start,
            end.map(|e| format!("t{e}")).unwrap_or_else(|| "(open)".into()),
            end.map(|e| (e - self.start).to_string()).unwrap_or_else(|| "?".into()),
            self.at,
            self.hostile,
            self.wt_at_entry.clone().unwrap_or_else(|| "absent".into()),
            self.sink_stop.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            self.home_set.as_ref().map(|(t, a)| format!("t{t} (stamp age then {a})")).unwrap_or_else(|| "never in the spell".into()),
            self.home_path.as_ref().map(|(c, s)| format!("{} start {s}", if *c { "complete" } else { "PARTIAL" })).unwrap_or_else(|| "-".into()),
            self.armed.map(|t| format!("t{t}")).unwrap_or_else(|| "-".into()),
            self.released.as_ref().map(|(t, a)| format!("t{t} (stamp age then {a})")).unwrap_or_else(|| "-".into()),
            if self.sight.is_empty() { String::new() } else { format!("; sight: {}", self.sight) }
        )
    }
}

#[derive(Default)]
struct V {
    tag: String,
    gone: Option<String>,
    hurts: Vec<String>,
    /// WALK_TARGET held (its source) or absent
    wt: Option<String>,
    spell: Option<Spell>,
    spells: Vec<String>,
    /// at-entry judgements: (tick, stamp age or none) at each first SetWalkTargetFromBlockMemory run per spell
    entry_judged: Vec<(u32, Option<i64>)>,
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
    if !acts.iter().any(|a| a.expect.is_some()) {
        println!("  OPEN LOOP: the drive carries no recorded state per act, so the divergence check (the rider / cart each act assumes) is UNAVAILABLE: the controller's coverage is NOT JUDGED by this row; its bars judge the mechanics it reaches");
    } else {
        println!("  CLOSED-LOOP CHECKED: {} write(s) carry redsim's recorded state (`# expect`), compared with the game's before each write (exact: the carts in the act's column with their riders and block-y offsets, each hold's and pocket's cart and rider); acts are applied between ticks with the server frozen (`tick step`), as f1q2-run applies them after its tick", acts.iter().filter(|a| a.expect.is_some()).count());
    }
    let jar = mctas::bridge::brain_agent(version)?;
    // (cc767b78's t166 brake) MCTAS_REPLAY_PUSH=<from>-<to>: every minecart push (the cart's delta
    // before / after, both boxes, the pusher and its vehicle) and every seated villager's push list in
    // that window, entities named by their scene tags
    let push_window: Option<(u32, u32)> = match std::env::var("MCTAS_REPLAY_PUSH") {
        Ok(s) => {
            let (a, b) = s.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_PUSH=from-to, got {s}"))?;
            Some((a.trim().parse()?, b.trim().parse()?))
        }
        Err(_) => None,
    };
    let opts = |world: Option<std::path::PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        keep_dir: true,
        world,
        java_agent: Some(std::path::PathBuf::from(format!("{}={MEMS}{}", jar.display(), if push_window.is_some() { ",+push" } else { "" }))),
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    // (G1 on G #9 / H #2) the scene is summoned under the run's OWN day time: a brain takes its
    // schedule activity when it is built (registerBrainGoals) and then updates it at most once every
    // > 20 gt (Brain.updateActivityFromSchedule 327-335), so a night-built brain would sit in REST for
    // the run's first ~21 gt
    srv.cmd_ok(&format!("time set {day_start}"))?;
    crate::seatclear::place_blocks(&mut srv, scene)?;
    let d = mctas::design::Design::load(scene)?;
    let mut by_uuid: BTreeMap<String, usize> = BTreeMap::new();
    let mut vs: Vec<V> = Vec::new();
    let mut cart_tags: Vec<String> = Vec::new();
    let mut uuid_names: Vec<(String, String)> = Vec::new();
    let mut push_rows: Vec<String> = Vec::new();
    // (G1 on H #2) each villager's activity as its brain is built at the summon: the agent prints it
    // before the brain knows its owner ("@none"), so the lines between two summons are that entity's
    let mut slog = Log { at: std::fs::metadata(&srv.log)?.len() };
    let mut built: BTreeMap<String, String> = BTreeMap::new();
    for (n, e) in d.entities.iter().enumerate() {
        let (nbt, kinds) = tagged(e.nbt.as_deref().unwrap_or(""), n);
        let out = srv.cmd(&format!("summon {} {} {} {} {nbt}", e.kind, e.pos[0], e.pos[1] + DY as f64, e.pos[2]))?;
        anyhow::ensure!(out.contains("Summoned new"), "entity line {n} ({} at {:?}): {}", e.kind, e.pos, out.trim());
        let acts_built: Vec<String> = slog.drain(&srv)?.into_iter().filter_map(|l| l.strip_prefix("activity ").map(|r| r.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string())).collect();
        let villagers = kinds.iter().filter(|k| *k == "minecraft:villager").count();
        if villagers == 1
            && let Some(last) = acts_built.last()
        {
            built.insert(format!("e{n}p{}", kinds.iter().position(|k| k == "minecraft:villager").unwrap_or(0)), last.clone());
        }
        if e.kind.contains("minecart") {
            cart_tags.push(format!("e{n}"));
            if push_window.is_some() {
                uuid_names.push((uuid_hex(&mut srv, &format!("@e[tag=e{n},limit=1]"))?, format!("e{n}")));
            }
        }
        for (k, kind) in kinds.iter().enumerate() {
            if kind == "minecraft:villager" {
                let tag = format!("e{n}p{k}");
                let u = uuid_hex(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                if push_window.is_some() {
                    uuid_names.push((u.clone(), tag.clone()));
                }
                by_uuid.insert(u, vs.len());
                vs.push(V { tag, ..Default::default() });
            }
        }
    }
    println!("  the scene's entity lines summoned: {}; villagers named: {} ({})", d.entities.len(), vs.len(), vs.iter().map(|v| v.tag.as_str()).collect::<Vec<_>>().join(" "));
    // (G1 / ironspec 07:57 (2)) the scene's own `# eN` names asserted at t0: each tagged line's name is
    // its spawn index (the replay's tag), and the game's entity has the line's type and position
    {
        let text = std::fs::read_to_string(scene)?;
        let mut pending: Option<String> = None;
        let mut named: Vec<(String, String, [f64; 3])> = Vec::new();
        for l in text.lines() {
            let l = l.trim();
            if let Some(n) = l.strip_prefix("# e")
                && !n.is_empty()
                && n.chars().all(|c| c.is_ascii_digit())
            {
                pending = Some(format!("e{n}"));
            } else if let Some(rest) = l.strip_prefix("entity:") {
                let w: Vec<&str> = rest.split_whitespace().collect();
                anyhow::ensure!(w.len() >= 4, "a short entity line: {l}");
                let pos = [w[1].parse::<f64>()?, w[2].parse::<f64>()?, w[3].parse::<f64>()?];
                named.push((pending.take().unwrap_or_default(), w[0].to_string(), pos));
            }
        }
        let tagged_lines = named.iter().filter(|(n, _, _)| !n.is_empty()).count();
        if tagged_lines > 0 {
            anyhow::ensure!(tagged_lines == named.len(), "{} of {} entity lines carry a `# eN` name: all or none", tagged_lines, named.len());
            for (k, (name, kind, pos)) in named.iter().enumerate() {
                anyhow::ensure!(*name == format!("e{k}"), "entity line {k} is named {name}: the drive's names must be the spawn order");
                let p = pos_of(&mut srv, &format!("@e[tag=e{k},limit=1]"))?.ok_or_else(|| anyhow::anyhow!("{name}: not in the world at t0"))?;
                anyhow::ensure!((p[0] - pos[0]).abs() < 1e-6 && (p[1] - pos[1]).abs() < 1e-6 && (p[2] - pos[2]).abs() < 1e-6, "{name}: at ({:.6},{:.6},{:.6}) at t0, its line says {pos:?}", p[0], p[1], p[2]);
                let ty = srv.cmd_long(&format!("execute if entity @e[tag=e{k},type={kind},limit=1]"))?;
                anyhow::ensure!(crate::strict::classify_test("type", &ty)?, "{name}: not a {kind} at t0");
            }
            // 26.3 spawns a summoned vehicle before its passengers (ServerLevel.addFreshEntityWithPassengers:
            // getSelfAndPassengers in order), so the entity tick list runs e0, e0.p0, e1, … in file order
            println!("  the scene's `# eN` names: {} lines, each its spawn index, type and t0 position asserted; the game's tick order is the spawn order (each cart before its rider)", named.len());
        }
    }
    srv.cmd_ok(&format!("time set {day_start}"))?;
    srv.cmd_ok("weather clear 1000000")?;
    srv.cmd_ok("gamerule advance_time true")?;
    let gt0 = srv.game_time()?;
    let mut log = Log { at: 0 };
    let mut setup_lines: Vec<String>;
    let (mut home_set_t0, mut slept_set_t0) = (0usize, 0usize);
    // (G1 on G #6) the agent hooks this replay reads must have installed (N > 0), and a positive:
    // a scratch villager given 1 drowning damage must print its `hurt drown` line
    {
        // (G1 on G #9) the setup's own agent lines kept: each villager's activity before t0 comes from
        // them (the summon-time brain build, then the switch after the run's day is set)
        setup_lines = Log { at: 0 }.drain(&srv)?;
        let text = std::fs::read_to_string(&srv.log)?;
        for (what, needle) in [("+hurt", "LivingEntity reports hurtServer ("), ("+poi acquire", "AcquirePoi reports findPathToPois's path ("), ("+poi tickets", "PoiRecord reports acquireTicket and releaseTicket (")] {
            let n: Option<u32> = text.lines().find_map(|l| l.split(needle).nth(1)).and_then(|r| r.split_whitespace().next()).and_then(|s| s.parse().ok());
            println!("  agent hook {what}: {}", n.map(|n| format!("{n} installed")).unwrap_or_else(|| "NOT installed".into()));
            anyhow::ensure!(n.is_some_and(|n| n > 0), "the agent hook {what} did not install: its absence of lines would read as no events");
        }
        srv.cmd_ok(&format!("setblock 28 {} 28 minecraft:glass", 100 + DY))?;
        srv.cmd_ok(&format!("summon minecraft:villager 28.5 {} 28.5 {{NoAI:1b,Tags:[\"scratchv\"]}}", 101 + DY))?;
        let su = uuid_hex(&mut srv, "@e[tag=scratchv,limit=1]")?;
        log.at = std::fs::metadata(&srv.log)?.len();
        let out = srv.cmd("damage @e[tag=scratchv,limit=1] 1 minecraft:drown")?;
        // the damage command calls hurtServer at once: the line is read from the log without stepping
        // the world (a setup tick would put the scene one tick ahead of the drive's clock)
        let mut lines = Vec::new();
        let t_wait = std::time::Instant::now();
        while t_wait.elapsed() < std::time::Duration::from_secs(5) {
            lines.extend(log.drain(&srv)?);
            if lines.iter().any(|l: &String| l.starts_with("hurt drown")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        setup_lines.extend(lines.iter().cloned());
        let hit = lines.iter().any(|l| l.starts_with("hurt drown") && l.ends_with(&format!("@{su}")));
        srv.cmd("kill @e[tag=scratchv]")?;
        srv.cmd(&format!("setblock 28 {} 28 minecraft:air", 100 + DY))?;
        println!("  the +hurt positive (a scratch villager, `damage … 1 minecraft:drown`: {}): {}", out.trim(), if hit { "its `hurt drown` line printed" } else { "NO line" });
        anyhow::ensure!(hit, "the +hurt positive printed no `hurt drown` line: a drowning would go unseen");
    }
    setup_lines.extend(log.drain(&srv)?);
    for v in vs.iter_mut() {
        if let Some(a) = built.get(&v.tag) {
            v.activity = a.clone();
        }
    }
    for l in &setup_lines {
        let Some(&vi) = l.rsplit(" @").next().and_then(|u| by_uuid.get(u.trim())) else { continue };
        if let Some(rest) = l.strip_prefix("activity ") {
            vs[vi].activity = rest.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:").to_string();
        } else if l.starts_with("set minecraft:nearest_hostile") {
            vs[vi].hostile = true;
        } else if l.starts_with("erase minecraft:nearest_hostile") {
            vs[vi].hostile = false;
        }
    }
    println!(
        "  the brains before t0 (from the setup's own lines): {}",
        vs.iter().map(|v| format!("{}={}", v.tag, if v.activity.is_empty() { "?" } else { v.activity.as_str() })).collect::<Vec<_>>().join(" ")
    );
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
    let air_watch = std::env::var("MCTAS_REPLAY_AIR").ok();
    let mut air_rows: Vec<String> = Vec::new();
    let mut state_at: Option<(u32, Vec<(String, [i32; 3], Option<String>)>)> = None;
    let (mut expects_checked, mut first_div, mut divergences) = (0usize, None::<u32>, Vec::<String>::new());
    let plant_at: Option<u32> = std::env::var("MCTAS_REPLAY_PLANT_EXPECT").ok().map(|s| s.parse()).transpose()?;
    let mut planted: Option<u32> = None;
    // (tick of a diverged write, redsim's holds / pockets then, the tick the game first matched them)
    let mut pending: Vec<(u32, String, Option<u32>)> = Vec::new();
    let trace_spec: Option<(String, u32, u32)> = match std::env::var("MCTAS_REPLAY_TRACE") {
        Ok(s) => {
            let (who, w) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_TRACE=eN:from-to, got {s}"))?;
            let (a, b) = w.split_once('-').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_TRACE=eN:from-to, got {s}"))?;
            Some((who.to_string(), a.parse()?, b.parse()?))
        }
        Err(_) => None,
    };
    let mut trace_rows: Vec<String> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    let mut day_base: i64 = day_start;
    // MCTAS_REPLAY_NAMES="7=28,8-11=7-10,12-28=11-27": scene index -> the drive's eN, where the drive
    // numbers its entities in another order (its spawn order)
    let mut names: BTreeMap<usize, usize> = BTreeMap::new();
    if let Ok(spec) = std::env::var("MCTAS_REPLAY_NAMES") {
        for part in spec.split(',').filter(|p| !p.trim().is_empty()) {
            let (a, b) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("MCTAS_REPLAY_NAMES: a=b or a-b=c-d, got {part}"))?;
            let range = |s: &str| -> Result<(usize, usize)> {
                Ok(match s.split_once('-') {
                    Some((x, y)) => (x.trim().parse()?, y.trim().parse()?),
                    None => (s.trim().parse()?, s.trim().parse()?),
                })
            };
            let ((a0, a1), (b0, b1)) = (range(a)?, range(b)?);
            anyhow::ensure!(a1 >= a0 && a1 - a0 == b1.wrapping_sub(b0), "MCTAS_REPLAY_NAMES: {part}: ranges of different lengths");
            for k in 0..=a1 - a0 {
                names.insert(a0 + k, b0 + k);
            }
        }
        println!("  names: scene index -> the drive's eN: {}", names.iter().map(|(a, b)| format!("{a}->{b}")).collect::<Vec<_>>().join(" "));
    }
    for t in 0..=ticks {
        if loads.contains(&t) {
            reloaded = true;
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(Some(w)), None)?;
            srv = s;
            log.at = 0;
            srv.cmd_ok("forceload add -32 -32 31 31")?;
            srv.cmd_ok("gamerule advance_time true")?;
            srv.cmd_ok("weather clear 1000000")?;
            // (G1 on G #2) the clock re-based from the game after the load: `time query time` is the
            // overworld clock (26.3: "Clock minecraft:overworld is at N tick(s)"); the tick about to
            // run reads N + 1
            let clock = srv.cmd_long("time query time")?;
            let n: i64 = clock.split(" is at ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|s| s.parse().ok()).ok_or_else(|| anyhow::anyhow!("time query time: unreadable: {}", clock.trim()))?;
            day_base = n - t as i64;
            events.push(format!("t{t}: WORLD OPEN ({}); the clock {n} (day {})", rl.describe(), n.rem_euclid(24000)));
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
                    // (G1 on G #1, ironspec 08:21) each memory act read back: the game holds what the
                    // drive wrote
                    if a.verb == "home" {
                        anyhow::ensure!(a.args.len() == 4, "t{t}: home eN.pK x y z");
                        let p: Vec<i32> = a.args[1..].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}", p[0], p[1] + DY, p[2]))?;
                        let back = crate::strict::data_value(&mut srv, &format!("data get entity @e[tag={who},limit=1] Brain.memories.\"minecraft:home\".value.pos"))?;
                        let want = format!("[I; {}, {}, {}]", p[0], p[1] + DY, p[2]);
                        anyhow::ensure!(back.as_deref() == Some(want.as_str()), "t{t}: {who}'s HOME read back {back:?}, the act wrote {want}");
                        home_set_t0 += usize::from(t == 0);
                    } else {
                        anyhow::ensure!(a.args.len() == 2, "t{t}: last-slept eN.pK d");
                        let dt: i64 = a.args[1].parse()?;
                        let v = gt0 + t as i64 + dt;
                        srv.cmd_ok(&format!("data merge entity @e[tag={who},limit=1] {{Brain:{{memories:{{\"minecraft:last_slept\":{{value:{v}L}}}}}}}}"))?;
                        let back = crate::strict::data_value(&mut srv, &format!("data get entity @e[tag={who},limit=1] Brain.memories.\"minecraft:last_slept\".value"))?;
                        anyhow::ensure!(back.as_deref() == Some(format!("{v}L").as_str()), "t{t}: {who}'s LAST_SLEPT read back {back:?}, the act wrote {v}");
                        slept_set_t0 += usize::from(t == 0);
                    }
                }
                "set" => {
                    anyhow::ensure!(a.args.len() == 4, "t{t}: set x y z STATE");
                    let p: Vec<i32> = a.args[..3].iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
                    let cell = [p[0], p[1], p[2]];
                    // (G1's divergence check) redsim's recorded state before the write against the
                    // game's: carts in the act's column, each hold and pocket
                    if let Some(exp) = &a.expect {
                        if state_at.as_ref().is_none_or(|(st, _)| *st != t) {
                            state_at = Some((t, cart_states(&mut srv, &cart_tags)?));
                        }
                        let game = expect_form(&state_at.as_ref().expect("set above").1, cell, &names);
                        let mut want = exp.replace(".p", "p");
                        // (G1's control) MCTAS_REPLAY_PLANT_EXPECT=t: the first expected state at or
                        // after t is planted wrong and must be counted as a divergence
                        if plant_at.is_some_and(|p| t >= p) && planted.is_none() {
                            want = format!("{want} (planted)");
                            planted = Some(t);
                        }
                        expects_checked += 1;
                        // (G1) the divergence's size: when the game's holds and pockets first reach
                        // the state redsim had at each diverged write
                        let game_hp = game.split_once("; ").map(|(_, r)| r.to_string()).unwrap_or_default();
                        for p in pending.iter_mut() {
                            // (a state matched within 100 gt; later the lap repeats the same names)
                            if p.2.is_none() && t <= p.0 + 100 && p.1 == game_hp {
                                p.2 = Some(t);
                            }
                        }
                        if game != want {
                            first_div.get_or_insert(t);
                            divergences.push(format!("t{t} ({},{},{}): redsim {want} | game {game}", cell[0], cell[1], cell[2]));
                            let want_hp = want.split_once("; ").map(|(_, r)| r.to_string()).unwrap_or_default();
                            if want_hp != game_hp && pending.len() < 400 {
                                pending.push((t, want_hp, None));
                            }
                        }
                    }
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
        // (ironspec 08:21 / 08:23) the t0 state, before the first tick: every villager's HOME and
        // LAST_SLEPT as the drive's t0 acts wrote them (each read back), the bed's ticket free (no
        // ticket taken during the setup)
        if t == 0 {
            let taken = setup_lines.iter().filter(|l| l.starts_with("poi acquire")).count();
            println!(
                "  t0: the run's start day {day_start} (scene {}, drive {}); HOME written and read back for {home_set_t0} villager(s), LAST_SLEPT for {slept_set_t0}, of {}; tickets taken during the setup: {taken} (start state (i): the ticket free){}",
                scene.display(),
                drive.display(),
                vs.len(),
                if home_set_t0 == vs.len() { "" } else { " (NOT every villager holds HOME)" }
            );
            anyhow::ensure!(taken == 0, "t0: {taken} ticket(s) taken during the setup: not start state (i)");
            anyhow::ensure!(home_set_t0 == 0 || home_set_t0 == vs.len(), "t0: the drive gives HOME to {home_set_t0} of {} villagers", vs.len());
        }
        srv.step(1)?;
        let now = gt0 + t as i64 + 1;
        let day = (day_base + t as i64 + 1).rem_euclid(24000);
        for l in log.drain(&srv)? {
            // (G1 on G #6) a hook that couldn't read its values fails the run, never reads as "no event"
            if l.contains("(unread)") {
                unread.push(format!("t{t}: {l}"));
                continue;
            }
            if l.starts_with("push cart by ") || l.starts_with("pushlist ") {
                if push_window.is_some_and(|(a, b)| (a..=b).contains(&t)) {
                    let mut s = l.clone();
                    for (u, name) in &uuid_names {
                        s = s.replace(u.as_str(), name);
                    }
                    push_rows.push(format!("t{t}: {s}"));
                }
                continue;
            }
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
                // a REST spell closes when the activity leaves REST
                if vs[vi].activity == "rest" && act != "rest" {
                    if let Some(sp) = vs[vi].spell.take() {
                        let d = sp.describe(Some(t));
                        vs[vi].spells.push(d);
                    }
                }
                if act == "rest" {
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    let age = v.stamp.map(|s| (now - s).to_string()).unwrap_or_else(|| "none".into());
                    v.rest_entries.push(format!("t{t} at {} hostile {} stamp age {age}", fmt_pos(at), v.hostile));
                    v.spell = Some(Spell { start: t, at: fmt_pos(at), hostile: v.hostile, wt_at_entry: v.wt.clone(), ..Default::default() });
                }
                events.push(format!("t{t}: {tag} activity {act}"));
                vs[vi].activity = act;
            } else if l.starts_with("set minecraft:walk_target") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                let v = &mut vs[vi];
                v.wt = Some(by.clone());
                if by.starts_with("SetWalkTargetFromBlockMemory") {
                    let age = v.stamp.map(|s| now - s);
                    if let Some(sp) = v.spell.as_mut()
                        && sp.home_set.is_none()
                        && sp.released.is_none()
                    {
                        sp.home_set = Some((t, age.map(|a| a.to_string()).unwrap_or_else(|| "none".into())));
                        v.entry_judged.push((t, age));
                    }
                }
            } else if l.starts_with("erase minecraft:walk_target") {
                let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                let v = &mut vs[vi];
                v.wt = None;
                v.home_path = None;
                if by.starts_with("MoveToTargetSink.stop")
                    && let Some(sp) = v.spell.as_mut()
                    && sp.sink_stop.is_none()
                {
                    sp.sink_stop = Some(t);
                }
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
                    if let Some(sp) = vs[vi].spell.as_mut() {
                        sp.home_path = Some((complete, start.clone()));
                    }
                    vs[vi].home_path = Some((complete, start));
                } else {
                    // another path replaces the HOME one
                    vs[vi].home_path = None;
                }
            } else if l.starts_with("erase minecraft:path") {
                // (G1 on G #2) a finished, stopped or erased path is no longer a complete HOME path
                vs[vi].home_path = None;
            } else if l.contains("minecraft:cant_reach_walk_target_since") {
                if l.starts_with("set ") {
                    let s: Option<i64> = l.split(" = ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok());
                    anyhow::ensure!(s.is_some(), "t{t}: unparsable stamp line: {l}");
                    let at = pos_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
                    let v = &mut vs[vi];
                    v.stamp = s;
                    let by = body.split(" by ").nth(1).unwrap_or("?").to_string();
                    if by.starts_with("MoveToTargetSink")
                        && let Some(sp) = v.spell.as_mut()
                        && sp.armed.is_none()
                    {
                        sp.armed = Some(t);
                    }
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
                // a release by SetWalkTargetFromBlockMemory is that spell's first run of it
                if by.starts_with("SetWalkTargetFromBlockMemory") {
                    let v = &mut vs[vi];
                    let age = v.stamp.map(|s| now - s);
                    if let Some(sp) = v.spell.as_mut()
                        && sp.released.is_none()
                    {
                        let first_run = sp.home_set.is_none();
                        sp.released = Some((t, age.map(|a| a.to_string()).unwrap_or_else(|| "none".into())));
                        if first_run {
                            v.entry_judged.push((t, age));
                        }
                    }
                }
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
                if let Some(sp) = vs[vi].spell.as_mut()
                    && sp.sight.is_empty()
                {
                    sp.sight = what.clone();
                }
                events.push(format!("t{t}: {tag} sight at its REST entry: {what}"));
            }
        }
        // (G1 on G #9, on H #2) the brains' activity at t1 asserted: by day the run's schedule activity
        // (IDLE, or PANIC on a hostile); by night REST, or PANIC with a hostile sensed
        if t == 0 {
            // (G1 on I) night as the schedule's keyframes give it (Timelines 186-198: REST from 12,000
            // to the IDLE keyframe at 10 of the next day), the day time taken mod 24,000
            let d0 = day.rem_euclid(24_000);
            let night0 = d0 >= 12_000 || d0 < 10;
            let wrong: Vec<String> = vs
                .iter()
                .filter(|v| v.gone.is_none())
                .filter(|v| match v.activity.as_str() {
                    "idle" => night0,
                    "rest" => !night0,
                    "panic" => !v.hostile && night0,
                    _ => true,
                })
                .map(|v| format!("{} {}", v.tag, if v.activity.is_empty() { "(no activity line)" } else { v.activity.as_str() }))
                .collect();
            println!(
                "  the brains at t1 (day {day}, {}): {}",
                if night0 { "night: REST, or PANIC with a hostile sensed" } else { "day: IDLE or PANIC" },
                if wrong.is_empty() { "every villager on the schedule".to_string() } else { format!("NOT on the schedule: {}", wrong.join(", ")) }
            );
            anyhow::ensure!(wrong.is_empty(), "at t1 {} villager(s) not on the run's schedule activity: {}", wrong.len(), wrong.join(", "));
        }
        // (G1's air-model fidelity row) MCTAS_REPLAY_AIR=<tag>: that villager's air supply every tick
        // while it is inside the main lift's box (x -6..-3, y 0..25, z 5..9): its position, eye
        // height, whether its eye's block is a bubble column or other water, its vehicle
        if let Some(who) = air_watch.as_deref()
            && vs.iter().any(|v| v.tag == who && v.gone.is_none())
        {
            let at = pos_of(&mut srv, &format!("@e[tag={who},limit=1]"))?;
            if let Some(p) = at
                && (-6.0..-3.0).contains(&p[0])
                && (0.0..25.0).contains(&p[1])
                && (5.0..9.0).contains(&p[2])
            {
                let air = srv.cmd_long(&format!("data get entity @e[tag={who},limit=1] Air"))?;
                let a = air.split("data: ").nth(1).map(|s| s.trim().to_string()).unwrap_or_else(|| air.trim().to_string());
                // the eye's cell: a marker summoned at the eyes, read back, then tested by absolute
                // block coordinates (a forked `execute as … if block` prints no test reply)
                srv.cmd(&format!("execute as @e[tag={who},limit=1] at @s anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {{Tags:[\"airm\"]}}"))?;
                let eye = pos_of(&mut srv, "@e[type=minecraft:marker,tag=airm,limit=1]")?;
                srv.cmd("kill @e[type=minecraft:marker,tag=airm]")?;
                let e = eye.ok_or_else(|| anyhow::anyhow!("t{t}: {who}'s eye marker unreadable"))?;
                let (bx, by, bz) = (e[0].floor() as i32, e[1].floor() as i32 + DY, e[2].floor() as i32);
                let bubble = crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} minecraft:bubble_column"))?;
                let water = crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} minecraft:water"))?;
                let riding = crate::strict::test_on(&mut srv, &format!("execute as @e[tag={who},limit=1] on vehicle if entity @s"))?;
                air_rows.push(format!(
                    "t{t}: air {a} at {} eye y {:.3} (cell {bx},{},{bz}) eye in bubble_column {bubble} / water {water}; seated {riding}",
                    fmt_pos(at),
                    e[1],
                    by - DY
                ));
            }
        }
        // (cc767b78's divergence trace) MCTAS_REPLAY_TRACE=<eN>:<from>-<to>: that cart's position,
        // motion, rider and the rail block under it every tick in the window
        if let Some((whos, from, to)) = trace_spec.as_ref()
            && (*from..=*to).contains(&t)
        {
          for who in whos.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let p = pos_of(&mut srv, &format!("@e[tag={who},limit=1]"))?;
            let m = srv.cmd_long(&format!("data get entity @e[tag={who},limit=1] Motion"))?;
            let rider = srv.cmd_long(&format!("execute as @e[tag={who},limit=1] on passengers run data get entity @s Tags"))?;
            let mut rail = String::from("?");
            if let Some(q) = p {
                let (bx, by, bz) = (q[0].floor() as i32, q[1].floor() as i32 + DY, q[2].floor() as i32);
                for kind in ["minecraft:powered_rail[powered=true]", "minecraft:powered_rail[powered=false]", "minecraft:detector_rail", "minecraft:activator_rail", "minecraft:rail", "minecraft:water", "minecraft:bubble_column", "minecraft:air"] {
                    if crate::strict::test(&mut srv, &format!("execute if block {bx} {by} {bz} {kind}"))? {
                        rail = format!("{kind} at ({bx},{},{bz})", by - DY);
                        break;
                    }
                }
            }
            trace_rows.push(format!(
                "t{t}: {who} pos {} motion {} rider {} block {rail}",
                p.map(|q| format!("({:.6},{:.6},{:.6})", q[0], q[1], q[2])).unwrap_or_else(|| "gone".into()),
                m.split("data: ").nth(1).unwrap_or("?").trim(),
                rider.split('"').find(|s| s.starts_with('e') && s.contains('p')).unwrap_or("none")
            ));
          }
        }
        // per tick: the REST counters, the stamps' ages, the dawn reads (G1 on G #2: the night from
        // 11,990 in numerator and denominator alike)
        let night = day >= 11990;
        if night {
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
                    let log_text = std::fs::read_to_string(&srv.log)?;
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
                // (ironspec 06:28 platform: the drowned-rider trace) a rider losing air: where it
                // stands, its cart's position and motion, the lift-top exit booster (-5,23,8), every
                // cart within 3 blocks
                if a < 250 {
                    let at = pos_of(&mut srv, &format!("@e[tag={},limit=1]", v.tag))?;
                    let cart = srv.cmd_long(&format!("execute as @e[tag={},limit=1] on vehicle run data get entity @s Pos", v.tag))?;
                    let motion = srv.cmd_long(&format!("execute as @e[tag={},limit=1] on vehicle run data get entity @s Motion", v.tag))?;
                    let booster = crate::strict::test(&mut srv, &format!("execute if block -5 {} 8 minecraft:powered_rail[powered=true]", 23 + DY))?;
                    let near = srv.cmd_long(&format!("execute as @e[tag={},limit=1] at @s as @e[type=#minecraft:minecart,distance=..3] run data get entity @s Pos", v.tag))?;
                    let carts: Vec<String> = near.split("entity data: ").skip(1).map(|r| r.split(']').next().unwrap_or("").trim_start_matches('[').to_string()).collect();
                    events.push(format!(
                        "t{t}: {} AIR LOW {a} at {}; cart {} motion {}; the exit booster (-5,23,8) powered {booster}; carts within 3: {}",
                        v.tag,
                        fmt_pos(at),
                        cart.split("entity data: ").nth(1).unwrap_or("none").trim(),
                        motion.split("entity data: ").nth(1).unwrap_or("none").trim(),
                        carts.join(" | ")
                    ));
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
        // (G1 on G #2) the 16-block exposure sampled every tick for the villagers in REST
        if vs.iter().any(|v| v.activity == "rest" && v.gone.is_none()) {
            let occ = crate::strict::test(&mut srv, &format!("execute if block {} {} {} #minecraft:beds[occupied=true]", bed[0], bed[1] + DY, bed[2]))?;
            if occ {
                for v in vs.iter_mut() {
                    if v.gone.is_some() || v.activity != "rest" {
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
    if let Some((a, b)) = push_window {
        println!("  pushes t{a}..t{b} (+push: every AbstractMinecart.push with the cart's delta before -> after and both boxes; every seated villager's pushEntities list; in the game's own order within each tick): {} line(s)", push_rows.len());
        for r in &push_rows {
            println!("    {r}");
        }
    }
    let judged = night_ticks as u64 * vs.len() as u64;
    if let Some((who, from, to)) = trace_spec.as_ref() {
        println!("  {who}'s trace t{from}..t{to} (game side; position after each tick, the block at its feet):");
        for r in &trace_rows {
            println!("    {r}");
        }
        anyhow::ensure!(!trace_rows.is_empty(), "MCTAS_REPLAY_TRACE: no tick traced");
    }
    // (G1's divergence check) the replay counts only while the game matches redsim's recorded state
    println!(
        "  divergence check (redsim's `# expect` state before each write vs the game's: carts in the act's column, each hold and pocket): {} of {expects_checked} write(s) diverged{}",
        divergences.len(),
        match first_div {
            Some(f) => format!("; the FIRST at t{f}: every result after it is the open-loop drive's, not the design's"),
            None if expects_checked == 0 => " (the drive carries no `# expect` lines: OPEN LOOP, NOT JUDGED as a design run)".to_string(),
            None => " (faithful through the run)".to_string(),
        }
    );
    for d in divergences.iter().take(40) {
        println!("    {d}");
    }
    // the size of each hold / pocket divergence: the game reaching redsim's state that many gt later
    // (or never within the run)
    let lags: Vec<String> = pending.iter().take(60).map(|(t, _, m)| format!("t{t}: {}", m.map(|x| format!("+{} gt", x - t)).unwrap_or_else(|| "not within 100 gt".into()))).collect();
    if !lags.is_empty() {
        println!("  hold / pocket divergences, the game reaching redsim's state (first 60): {}", lags.join("; "));
    }
    if let Some(p) = plant_at {
        let caught = planted.is_some_and(|pt| divergences.iter().any(|d| d.starts_with(&format!("t{pt} ")) && d.contains("(planted)")));
        println!("  the planted control (a wrong expected state at the first write at or after t{p}): {}", match planted {
            Some(pt) if caught => format!("planted at t{pt}, CAUGHT"),
            Some(pt) => format!("planted at t{pt}, NOT caught"),
            None => "never planted (no expect line at or after it)".into(),
        });
        anyhow::ensure!(caught, "the planted divergence control failed: the comparison can't be trusted");
    }
    if let Some(who) = air_watch.as_deref() {
        println!("  {who}'s air supply per tick inside the main lift's box ({} tick(s)):", air_rows.len());
        for r in &air_rows {
            println!("    {r}");
        }
        anyhow::ensure!(!air_rows.is_empty(), "MCTAS_REPLAY_AIR={who}: no tick inside the lift's box: NOT JUDGED");
    }
    // (ironspec 06:35 / 06:37 platform) every REST spell: its length, WALK_TARGET at entry and its
    // source, the sink's stop, the HOME-target tick or "never in the spell", the path, any arming or
    // release; the at-entry stamp bar judged at each spell's first SetWalkTargetFromBlockMemory run
    let mut at_entry_bad: Vec<String> = Vec::new();
    let mut at_entry_n = 0usize;
    let mut at_entry_after = 0usize;
    let mut h_spells = 0usize;
    for v in vs.iter_mut() {
        if let Some(sp) = v.spell.take() {
            let d = sp.describe(None);
            v.spells.push(d);
        }
        println!("  {}: {} REST spell(s)", v.tag, v.spells.len());
        for s in &v.spells {
            println!("    {s}");
            // H = the holds (x -0.5 / 3.5 / 7.5, y 15.9..17.2, z 0..1)
            if let Some(p) = s.split(" at (").nth(1).and_then(|r| r.split(')').next()) {
                let c: Vec<f64> = p.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                if c.len() == 3 && [-0.5, 3.5, 7.5].iter().any(|x| (c[0] - x).abs() < 0.3) && (15.9..17.2).contains(&c[1]) && (0.0..1.0).contains(&c[2]) {
                    h_spells += 1;
                }
            }
        }
        for (t, age) in &v.entry_judged {
            // (ironspec 07:02 (1)) after the first divergence a result is the drift's, NOT JUDGED
            if first_div.is_some_and(|f| *t >= f) {
                at_entry_after += 1;
                continue;
            }
            at_entry_n += 1;
            if age.is_some_and(|a| a >= 1000) {
                at_entry_bad.push(format!("{} t{t} stamp age {}", v.tag, age.unwrap_or(0)));
            }
        }
    }
    println!(
        "  the at-entry stamp bar (< 1,000 gt at each spell's first SetWalkTargetFromBlockMemory run): {} of {at_entry_n} judged spell(s) at or over 1,000{}; spells at H: {h_spells}{}",
        at_entry_bad.len(),
        if at_entry_n == 0 { " (NOT JUDGED: 0 samples)".to_string() } else if at_entry_bad.is_empty() { " (HELD)".to_string() } else { format!(" (FAILED: {})", at_entry_bad.join("; ")) },
        first_div.map(|f| format!("; {at_entry_after} spell(s) from t{f} on NOT JUDGED (after the first divergence)")).unwrap_or_default()
    );
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
    anyhow::ensure!(unread.is_empty(), "{} agent line(s) unread (first: {}): the run fails", unread.len(), unread.first().map(|s| s.as_str()).unwrap_or(""));
    // (G1 on H #1) every `# expect` line must be checked (one that never ran, or slipped a name map,
    // isn't a pass), and an open-loop drive is no design run: it exits non-zero unless declared
    // fidelity-only (MCTAS_REPLAY_FIDELITY_ONLY=1, printed)
    let expect_lines = acts.iter().filter(|a| a.expect.is_some()).count();
    let fidelity_only = std::env::var_os("MCTAS_REPLAY_FIDELITY_ONLY").is_some();
    println!(
        "  run kind: {}; expect lines {expect_lines}, checked {expects_checked}",
        if expect_lines > 0 { "closed-loop checked (a design run)" } else if fidelity_only { "OPEN LOOP, declared fidelity-only (MCTAS_REPLAY_FIDELITY_ONLY): mechanics only, no design verdict" } else { "OPEN LOOP (no `# expect` lines): not a design run" }
    );
    anyhow::ensure!(expects_checked == expect_lines, "{expects_checked} of {expect_lines} `# expect` line(s) checked: the run fails");
    anyhow::ensure!(expect_lines > 0 || fidelity_only, "an OPEN-LOOP drive (no `# expect` lines) is no design run: declare it with MCTAS_REPLAY_FIDELITY_ONLY=1 for a mechanics-only row; the run fails");
    anyhow::ensure!(judged > 0, "no night tick judged: NOT JUDGED, the run fails");
    anyhow::ensure!(applied == acts.len(), "only {applied} of {} acts applied within {ticks} gt", acts.len());
    anyhow::ensure!(first_div.is_none() || expects_checked == 0, "the replay diverged from redsim's recorded state at t{}: every design result after it is NOT JUDGED (drift): the run fails as a design run", first_div.unwrap_or(0));
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
