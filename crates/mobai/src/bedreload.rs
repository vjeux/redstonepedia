//! (tickexact 2026-11-11, the parent's shared-bed overload across reloads, for vjeux) Scene
//! bed-reload-<case> on the UNMODIFIED 26.3 dedicated server (mctas's `VanillaServer`: RCON, no
//! player), with a real save, stop and start where the bridge unloads and loads its sandbox (or,
//! with `--forceload`, the site's forced chunks released and forced again with nobody near):
//! the same blocks, villagers and timed acts as the bridge's scene, every villager tagged
//! `br<k>`. Printed at the scene's checkpoints: every villager's HOME (position and dimension)
//! and LAST_SLEPT, the bed POI's free tickets (from the saved `poi` region file) and whether the
//! bed is OCCUPIED; and for every nap whether the villager lay down. The server's own randoms
//! are not the bridge's, so the timings may differ by ticks; the claims, tickets and naps are
//! what is compared.

use crate::fixture::{BR_CLAIM0, BR_CLAIM_LEN, BR_NAP, BR_NAP0, BR_STEP, BR_TICKS, br_reload, scene_actions, scenes};
use anyhow::{Result, anyhow, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The site's offset in the server's world (away from the spawn chunks, which never unload).
const O: [i32; 3] = [512, 0, 512];

/// What happens at a tick of the run, in this order: the scene's acts, then the reads.
#[derive(Clone, Debug)]
enum Ev {
    Act(Vec<([i32; 3], String)>),
    Check(String),
    Nap(u32),
    NapEnd(u32),
    PreAirlock(u32),
    PreBed(u32),
    AirlockWatch(u32),
}

/// One villager's state as the server reports it.
struct VState {
    home: Option<(String, [i32; 3])>,
    last_slept: Option<i64>,
    sleeping: bool,
    /// CANT_REACH_WALK_TARGET_SINCE (a game time), if set.
    cant_reach: Option<i64>,
}

fn start(version: &str, world: Option<PathBuf>) -> Result<VanillaServer> {
    let mut srv = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        difficulty: "normal".into(),
        with_bot: false,
        tick_rate: 1000,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        world,
        ..Default::default()
    })?;
    // (the day advances with every stepped tick, as in the bridge's sandbox)
    srv.cmd_ok("gamerule advance_time true")?;
    Ok(srv)
}

/// The text after `key` in an SNBT dump, up to the end of its value's first number list or number.
fn snbt_after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    s.find(key).map(|i| &s[i + key.len()..])
}

fn villager(srv: &mut VanillaServer, k: usize) -> Result<Option<VState>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag=br{k},limit=1] Brain.memories"))?;
    if !out.contains("has the following entity data") {
        return Ok(None);
    }
    let home = snbt_after(&out, "\"minecraft:home\"").and_then(|h| {
        let h = &h[..h.find('}').map(|i| (i + 40).min(h.len())).unwrap_or(h.len())];
        let dim = snbt_after(h, "dimension: \"")?.split('"').next()?.to_string();
        let pos = snbt_after(h, "pos: [I;")?.split(']').next()?;
        let v: Vec<i32> = pos.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (v.len() == 3).then(|| (dim, [v[0] - O[0], v[1] - O[1], v[2] - O[2]]))
    });
    let last_slept = snbt_after(&out, "\"minecraft:last_slept\"").and_then(|h| {
        let v = snbt_after(h, "value: ")?;
        let num: String = v.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
        num.parse().ok()
    });
    let sl = srv.cmd_long(&format!("data get entity @e[tag=br{k},limit=1] sleeping_pos"))?;
    let cant_reach = snbt_after(&out, "\"minecraft:cant_reach_walk_target_since\"").and_then(|h| {
        let v = snbt_after(h, "value: ")?;
        let num: String = v.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
        num.parse().ok()
    });
    Ok(Some(VState { home, last_slept, sleeping: sl.contains("has the following entity data"), cant_reach }))
}

/// The bed POI's free tickets from the saved `poi` region file (`save-all flush` first): None
/// when no record is there.
fn poi_free(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<i64>> {
    srv.cmd_ok("save-all flush")?;
    let folder = srv.dim_dir("overworld").join("poi");
    let mut cache = std::collections::HashMap::new();
    let Some(root) = mctas::vanilla::read_region_root(&folder, p[0].div_euclid(16), p[2].div_euclid(16), &mut cache)? else { return Ok(None) };
    let sec = p[1].div_euclid(16).to_string();
    let Some(records) = root.get("Sections").and_then(|s| s.get(&sec)).and_then(|s| s.get("Records")) else { return Ok(None) };
    for r in records.as_list() {
        let pos: Vec<i64> = match r.get("pos") {
            Some(mctas::vanilla::nbt::Tag::IntArray(v)) => v.iter().map(|x| *x as i64).collect(),
            _ => continue,
        };
        if pos == [p[0] as i64, p[1] as i64, p[2] as i64] {
            // (PoiRecord's codec leaves out free_tickets at 0, its default)
            return Ok(Some(r.get("free_tickets").and_then(|t| t.as_i64()).unwrap_or(0)));
        }
    }
    Ok(None)
}

fn print_check(srv: &mut VanillaServer, t: u32, label: &str) -> Result<()> {
    let bed = [O[0] + 8, 64, O[2] + 8];
    let is_bed = srv.test_block(bed, "minecraft:red_bed")?;
    let occupied = is_bed && srv.test_block(bed, "minecraft:red_bed[occupied=true]")?;
    let free = poi_free(srv, bed)?;
    let mut homes: BTreeMap<String, u32> = BTreeMap::new();
    let (mut missing, mut slept, mut asleep) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..21 {
        match villager(srv, k)? {
            None => missing.push(format!("v{k} (not found)")),
            Some(v) => {
                match &v.home {
                    Some((dim, p)) => *homes.entry(format!("{} {} {} {dim}", p[0], p[1], p[2])).or_default() += 1,
                    None => missing.push(format!("v{k}")),
                }
                slept.push(format!("v{k} {}", v.last_slept.map(|x| x.to_string()).unwrap_or_else(|| "-".into())));
                if v.sleeping {
                    asleep.push(format!("v{k}"));
                }
            }
        }
    }
    let gt = srv.game_time()?;
    println!(
        "t {t} (game time {gt}; {label}): bed {}; POI free tickets {}; HOME held by {}/21 ({}){}{}; LAST_SLEPT: {}",
        if !is_bed { "GONE".to_string() } else if occupied { "OCCUPIED".to_string() } else { "free".to_string() },
        free.map(|f| f.to_string()).unwrap_or_else(|| "(no record)".into()),
        21 - missing.len(),
        homes.iter().map(|(h, n)| format!("{h} x{n}")).collect::<Vec<_>>().join(", "),
        if missing.is_empty() { String::new() } else { format!("; without HOME: {}", missing.join(" ")) },
        if asleep.is_empty() { String::new() } else { format!("; asleep: {}", asleep.join(" ")) },
        slept.join(", ")
    );
    Ok(())
}

/// Wait until all 21 villagers are found again: first in real time (up to 10 s, the server
/// loading the forced chunks' entities), then stepping single ticks (up to `max_ticks`); the
/// ticks stepped.
fn wait_villagers(srv: &mut VanillaServer, max_ticks: u32) -> Result<u32> {
    let t0 = std::time::Instant::now();
    let mut stepped = 0;
    while count_tagged(srv)? < 21 {
        if t0.elapsed() < std::time::Duration::from_secs(10) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }
        if stepped >= max_ticks {
            bail!("the villagers did not come back within {max_ticks} ticks");
        }
        srv.step(1)?;
        stepped += 1;
    }
    Ok(stepped)
}

/// How many of the 21 tagged villagers the server finds.
fn count_tagged(srv: &mut VanillaServer) -> Result<u32> {
    let mut n = 0;
    for k in 0..21 {
        n += srv.count_entities(&format!("@e[type=minecraft:villager,tag=br{k},limit=1]"))?.min(1);
    }
    Ok(n)
}

/// `setblock`, an identical block already there being no error.
fn setblock(srv: &mut VanillaServer, p: [i32; 3], st: &str) -> Result<()> {
    let out = srv.cmd(&format!("setblock {} {} {} {st}", p[0] + O[0], p[1] + O[1], p[2] + O[2]))?;
    if !out.contains("Changed the block") && !out.contains("Could not set the block") {
        bail!("setblock {p:?} {st}: {out}");
    }
    Ok(())
}

/// `mctas mobai bed-reload-vanilla --case <day|asleep|woken|broken|none> [--forceload]`.
pub fn bed_reload_vanilla(version: &str, case: &str, forceload: bool) -> Result<()> {
    let name = format!("bed-reload-{case}");
    let all = scenes();
    let s = all.iter().find(|s| s.name == name).ok_or_else(|| anyhow!("no scene {name}"))?;
    let mut timeline: BTreeMap<u32, Vec<Ev>> = BTreeMap::new();
    for (t, acts) in scene_actions(&name) {
        timeline.entry(t).or_default().push(Ev::Act(acts));
    }
    timeline.entry(BR_CLAIM0 + BR_STEP * 20 + BR_CLAIM_LEN + 30).or_default().push(Ev::Check("after the 21 claims and the last break-and-replace".into()));
    if let Some((u, l)) = br_reload(case) {
        timeline.entry(u - 1).or_default().push(Ev::Check(format!("just before the {} (at {u})", if forceload { "forced chunks' release" } else { "save, stop and start" })));
        timeline.entry(l + 1).or_default().push(Ev::Check(format!("after the {} (the bridge's load at {l})", if forceload { "chunks forced again" } else { "restart" })));
    }
    for k in 0..21 {
        timeline.entry(BR_NAP0 + BR_NAP * k + 20).or_default().push(Ev::Nap(k));
        timeline.entry(BR_NAP0 + BR_NAP * k + 230).or_default().push(Ev::NapEnd(k));
        // (villager k in the room just before its airlock, and in the airlock just before the bed:
        // a HOME lost in between shown with its CANT_REACH_WALK_TARGET_SINCE)
        timeline.entry(BR_NAP0 + BR_NAP * k - 41).or_default().push(Ev::PreAirlock(k));
        timeline.entry(BR_NAP0 + BR_NAP * k - 1).or_default().push(Ev::PreBed(k));
        timeline.entry(BR_NAP0 + BR_NAP * k - 40).or_default().push(Ev::AirlockWatch(k));
    }
    timeline.entry(BR_TICKS - 10).or_default().push(Ev::Check("after the night's naps".into()));
    println!("== {name} on the vanilla {version} server ({})", if forceload { "the forced chunks released and forced again, nobody near" } else { "a real save, stop and start" });
    let mut srv = start(version, None)?;
    srv.cmd_ok(&format!("time set {}", s.day_time))?;
    let (x0, z0, x1, z1) = (O[0] + s.chunks[0] * 16, O[2] + s.chunks[1] * 16, O[0] + s.chunks[2] * 16 + 15, O[2] + s.chunks[3] * 16 + 15);
    srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
    srv.step(5)?;
    // the floor in one fill, then every other block of the scene (placed with updates)
    srv.cmd_ok(&format!("fill {} 63 {} {} 63 {} minecraft:stone", O[0], O[2], O[0] + 47, O[2] + 79))?;
    for (p, st) in &s.blocks {
        if p[1] == 63 && st == "minecraft:stone" {
            continue;
        }
        setblock(&mut srv, *p, st)?;
    }
    for (k, (ty, p, nbt)) in s.mobs.iter().enumerate() {
        let tag = format!("Tags:[\"br{k}\"]");
        let nbt = match nbt.as_deref() {
            Some(n) if n.trim() != "{}" => format!("{{{tag},{}", &n.trim()[1..]),
            _ => format!("{{{tag}}}"),
        };
        srv.cmd_ok(&format!("summon {ty} {} {} {} {nbt}", p[0] + O[0] as f64, p[1] + O[1] as f64, p[2] + O[2] as f64))?;
    }
    let mut t: u32 = 0;
    let mut naps: Vec<Option<bool>> = vec![None; 21];
    let mut pre: Vec<Option<(bool, Option<i64>)>> = vec![None; 21];
    let mut airlock_loss: Vec<Option<u32>> = vec![None; 21];
    let mut world_runs = 1;
    // (the ticks a vanilla unload or load took beyond the bridge's: every later act keeps its
    // distance from the reload)
    let mut shift: u32 = 0;
    for (at, evs) in timeline {
        let at = at + shift;
        if at > t {
            srv.step(at - t)?;
            t = at;
        }
        let t_before = t;
        // (ticks stepped inside the group as part of the schedule: the airlock watch's)
        let mut watched: u32 = 0;
        for ev in evs {
            match ev {
                Ev::Act(acts) => {
                    for (p, a) in acts {
                        if a == "unload" {
                            if forceload {
                                srv.cmd_ok(&format!("forceload remove {x0} {z0} {x1} {z1}"))?;
                                // (the chunks go when the chunk system unloads them: stepped until
                                // no villager is found)
                                let mut stepped = 0;
                                while count_tagged(&mut srv)? > 0 {
                                    if stepped >= 600 {
                                        bail!("the site's chunks did not unload within 600 ticks of their release");
                                    }
                                    srv.step(1)?;
                                    stepped += 1;
                                }
                                t += stepped;
                                println!("t {t}: the site's forced chunks released (no player anywhere); every villager unloaded (saved) {stepped} ticks after the release");
                            } else {
                                srv.cmd_ok("save-on")?;
                                srv.cmd_ok("save-all flush")?;
                                let old = srv.dir.clone();
                                let world = old.join(&srv.level_name);
                                let gt = srv.game_time()?;
                                drop(srv);
                                srv = start(version, Some(world))?;
                                let _ = std::fs::remove_dir_all(&old);
                                world_runs += 1;
                                let stepped = wait_villagers(&mut srv, 200)?;
                                t += stepped;
                                println!("t {t}: saved, stopped (game time {gt}) and started again on the saved world (game time {}; all 21 villagers found after {stepped} ticks)", srv.game_time()?);
                            }
                        } else if a == "load" {
                            if forceload {
                                srv.cmd_ok(&format!("forceload add {x0} {z0} {x1} {z1}"))?;
                                let stepped = wait_villagers(&mut srv, 200)?;
                                t += stepped;
                                println!("t {t}: the site's chunks forced again: all 21 villagers found after {stepped} ticks");
                            }
                        } else if let Some(rest) = a.strip_prefix("tp:id") {
                            let f: Vec<&str> = rest.split(':').collect();
                            let (k, x, y, z): (usize, f64, f64, f64) = (f[0].parse()?, f[1].parse()?, f[2].parse()?, f[3].parse()?);
                            srv.cmd_ok(&format!("tp @e[tag=br{k},limit=1] {} {} {}", x + O[0] as f64, y + O[1] as f64, z + O[2] as f64))?;
                        } else {
                            setblock(&mut srv, p, &a)?;
                        }
                    }
                }
                Ev::Check(label) => {
                    print_check(&mut srv, t, &label)?;
                }
                Ev::Nap(k) => {
                    naps[k as usize] = villager(&mut srv, k as usize)?.map(|v| v.sleeping);
                }
                Ev::PreAirlock(k) => {
                    pre[k as usize] = villager(&mut srv, k as usize)?.map(|v| (v.home.is_some(), v.cant_reach));
                }
                Ev::PreBed(k) => {
                    // (watched tick by tick through the airlock: when a HOME is lost there; the
                    // CANT_REACH_WALK_TARGET_SINCE memory has no codec, so entity data never shows it)
                    if let (Some((true, _)), Some(v)) = (pre[k as usize], villager(&mut srv, k as usize)?)
                        && v.home.is_none()
                    {
                        println!("t {t}: v{k} lost HOME in the airlock (at {})", airlock_loss[k as usize].map(|x| format!("{x} gt after it came in")).unwrap_or_else(|| "?".into()));
                    }
                }
                Ev::AirlockWatch(k) => {
                    // from 1 to 38 gt after villager k came into the airlock: its HOME every tick
                    for d in 1..=38u32 {
                        if villager(&mut srv, k as usize)?.is_some_and(|v| v.home.is_none()) {
                            airlock_loss[k as usize] = Some(d);
                            break;
                        }
                        srv.step(1)?;
                        t += 1;
                        watched += 1;
                    }
                }
                Ev::NapEnd(k) => {
                    // (after villager k's nap, before the next one is brought beside the bed)
                    let bed = [O[0] + 8, 64, O[2] + 8];
                    let free = poi_free(&mut srv, bed)?;
                    let occupied = srv.test_block(bed, "minecraft:red_bed[occupied=true]")?;
                    let mut without = Vec::new();
                    let mut slept_k = None;
                    for j in 0..21 {
                        let v = villager(&mut srv, j)?;
                        if j == k as usize {
                            slept_k = v.as_ref().and_then(|v| v.last_slept);
                        }
                        if v.as_ref().is_none_or(|v| v.home.is_none()) {
                            without.push(format!("v{j}"));
                        }
                    }
                    println!(
                        "t {t} after v{k}'s nap: asleep 20 gt after it came {}; its LAST_SLEPT {}; HOME held by {}/21{}; POI free tickets {}; bed {}",
                        match naps[k as usize] { Some(true) => "yes", Some(false) => "NO", None => "(not found)" },
                        slept_k.map(|x| x.to_string()).unwrap_or_else(|| "-".into()),
                        21 - without.len(),
                        if without.is_empty() { String::new() } else { format!(" (without: {})", without.join(" ")) },
                        free.map(|f| f.to_string()).unwrap_or_else(|| "(no record)".into()),
                        if occupied { "OCCUPIED" } else { "free" }
                    );
                }
            }
        }
        shift += t - t_before - watched;
    }
    let lay = naps.iter().filter(|n| **n == Some(true)).count();
    println!(
        "naps: {lay}/21 villagers asleep 20 gt after being brought beside the bed: {}",
        naps.iter().enumerate().map(|(k, n)| format!("v{k} {}", match n { Some(true) => "asleep", Some(false) => "AWAKE", None => "not found" })).collect::<Vec<_>>().join(", ")
    );
    println!("(server runs on this world: {world_runs})");
    let dir = srv.dir.clone();
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
