//! Official-server scenarios for the ranked POI/entity desyncs (NOTES.md SS5). Commands set the
//! scene and read markers; every desync step is a survival action (bot block break, piston push,
//! villager brain claims). Save/rollback appears once, to classify a store mismatch (R5).

use anyhow::{Context, Result, anyhow, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::{Path, PathBuf};
use std::time::Duration;

const FLOOR_Y: i32 = 63;
const STAND_Y: i32 = 64;

pub fn run_all(version: &str, keep_dir: bool) -> Result<()> {
    let names = ["bed-double", "job-double", "piston-push", "phantom-workstation"];
    let mut failed = Vec::new();
    for name in names {
        match run_one(version, name, keep_dir) {
            Ok(()) => {}
            Err(e) => {
                println!("FAIL {name}: {e:#}");
                failed.push(name);
            }
        }
    }
    if failed.is_empty() {
        println!("ALL PASS");
        Ok(())
    } else {
        bail!("scenarios failed: {}", failed.join(", "))
    }
}

pub fn run_one(version: &str, name: &str, keep_dir: bool) -> Result<()> {
    match name {
        "bed-double" => bed_double(version, keep_dir),
        "job-double" => job_double(version, keep_dir),
        "piston-push" => piston_push(version, keep_dir),
        "phantom-workstation" => phantom_workstation(version, keep_dir),
        other => bail!("unknown scenario `{other}` (bed-double, job-double, piston-push, phantom-workstation)"),
    }
}

// ---- shared helpers -----------------------------------------------------------

fn start(version: &str, world: Option<PathBuf>, keep_dir: bool) -> Result<VanillaServer> {
    let opts = VanillaOptions { version: version.to_string(), keep_dir, world, ..Default::default() };
    VanillaServer::start(&opts)
}

/// A flat stone floor at FLOOR_Y (stand on STAND_Y) and the bot parked in the middle so every
/// entity used here is inside simulation distance.
fn platform(srv: &mut VanillaServer) -> Result<()> {
    srv.cmd_ok(&format!("fill -24 {FLOOR_Y} -16 48 {FLOOR_Y} 16 minecraft:stone"))?;
    srv.cmd_ok("time set 3000")?;
    srv.teleport_bot(12.5, (STAND_Y + 2) as f64, 0.5)?;
    Ok(())
}

fn summon_villager(srv: &mut VanillaServer, tag: &str, x: i32, z: i32) -> Result<()> {
    let out = srv.cmd(&format!(
        "summon minecraft:villager {x} {STAND_Y} {z} {{Tags:[\"{tag}\"]}}"
    ))?;
    if !out.contains("Summoned") {
        bail!("summon {tag}: {out}");
    }
    Ok(())
}

fn tp_tag(srv: &mut VanillaServer, tag: &str, x: i32, z: i32) -> Result<()> {
    srv.cmd_ok(&format!("tp @e[type=minecraft:villager,tag={tag}] {x} {STAND_Y} {z}"))?;
    Ok(())
}

fn sel(tag: &str) -> String {
    format!("@e[type=minecraft:villager,tag={tag},limit=1]")
}

/// The `pos` of a GlobalPos brain memory (e.g. `minecraft:home`), parsed from `data get`.
fn memory_pos(srv: &mut VanillaServer, tag: &str, module: &str) -> Result<Option<[i32; 3]>> {
    let s = sel(tag);
    let out = srv.cmd(&format!("data get entity {s} Brain.memories"))?;
    let Some(at) = out.find(module) else { return Ok(None) };
    let rest = &out[at..];
    let Some(pos_at) = rest.find("pos:") else { return Ok(None) };
    let rest = &rest[pos_at + 4..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else { return Ok(None) };
    if close < open {
        return Ok(None);
    }
    let nums: Vec<i32> = rest[open..=close]
        .split(|c: char| !(c.is_ascii_digit() || c == '-'))
        .filter(|p| !p.is_empty() && *p != "-")
        .filter_map(|p| p.parse().ok())
        .collect();
    if nums.len() == 3 {
        Ok(Some([nums[0], nums[1], nums[2]]))
    } else {
        Ok(None)
    }
}

fn profession(srv: &mut VanillaServer, tag: &str) -> Result<String> {
    let s = sel(tag);
    let out = srv.cmd(&format!("data get entity {s} VillagerData.profession"))?;
    for p in [
        "minecraft:none", "minecraft:nitwit", "minecraft:armorer", "minecraft:butcher", "minecraft:cartographer",
        "minecraft:cleric", "minecraft:farmer", "minecraft:fisherman", "minecraft:fletcher", "minecraft:leatherworker",
        "minecraft:librarian", "minecraft:mason", "minecraft:shepherd", "minecraft:toolsmith", "minecraft:weaponsmith",
    ] {
        if out.contains(p) {
            return Ok(p.to_string());
        }
    }
    Err(anyhow!("profession of {tag}: {out}"))
}

fn long_field(srv: &mut VanillaServer, tag: &str, field: &str) -> Result<i64> {
    let s = sel(tag);
    let out = srv.cmd(&format!("data get entity {s} {field}"))?;
    let tail = out.rsplit(':').next().unwrap_or("");
    let num: String = tail.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
    num.parse().map_err(|_| anyhow!("{field} of {tag}: {out}"))
}

fn has_memory(srv: &mut VanillaServer, tag: &str, module: &str) -> Result<bool> {
    let s = sel(tag);
    let out = srv.cmd(&format!("data get entity {s} Brain.memories"))?;
    Ok(out.contains(module))
}

/// Step in chunks until `memory_pos(tag, module) == pos`, or give up after `max_ticks`.
fn wait_memory(srv: &mut VanillaServer, tag: &str, module: &str, pos: [i32; 3], max_ticks: u32) -> Result<bool> {
    let mut waited = 0;
    loop {
        if memory_pos(srv, tag, module)? == Some(pos) {
            return Ok(true);
        }
        if waited >= max_ticks {
            return Ok(false);
        }
        srv.step(20)?;
        waited += 20;
    }
}

/// The bot (a real network player) breaks a block: the survival block-change path.
fn bot_break(srv: &mut VanillaServer, pos: [i32; 3]) -> Result<()> {
    srv.teleport_bot_to(pos[0] as f64 + 0.5, (pos[1] + 2) as f64, pos[2] as f64 + 2.5, Some((180.0, 45.0)))?;
    let seq = srv.bot.as_mut().context("no bot")?.start_destroy(pos, 1)?;
    srv.bot.as_ref().context("no bot")?.wait_ack(seq, Duration::from_secs(10))?;
    srv.step(2)?;
    Ok(())
}

fn setblock(srv: &mut VanillaServer, pos: [i32; 3], state: &str) -> Result<()> {
    srv.cmd_ok(&format!("setblock {} {} {} {}", pos[0], pos[1], pos[2], state))?;
    Ok(())
}

fn lock_profession(srv: &mut VanillaServer, tag: &str) -> Result<()> {
    let s = sel(tag);
    srv.cmd_ok(&format!("data merge entity {s} {{Xp:10}}"))?;
    Ok(())
}

/// Villager offers are generated lazily on first trade, so `Offers` is absent until then; give
/// the villager one real offer with `uses:1` so `needsToRestock()` (uses > 0) can ever pass.
fn force_used_offer(srv: &mut VanillaServer, tag: &str) -> Result<i64> {
    let s = sel(tag);
    srv.cmd_ok(&format!(
        "data merge entity {s} {{Offers:{{Recipes:[{{buy:{{id:\"minecraft:emerald\",count:1}},sell:{{id:\"minecraft:paper\",count:1}},uses:1,maxUses:12}}]}}}}"
    ))?;
    long_field(srv, tag, "Offers.Recipes[0].uses")
}

// ---- R1: break+replace a bed under a distant claimant ------------------------

fn bed_double(version: &str, keep_dir: bool) -> Result<()> {
    let head = [2, STAND_Y, 0];
    let foot = [2, STAND_Y, 1];
    let (first, second) = {
        let mut srv = start(version, None, true)?;
        platform(&mut srv)?;
        setblock(&mut srv, foot, "minecraft:white_bed[part=foot,facing=north,occupied=false]")?;
        setblock(&mut srv, head, "minecraft:white_bed[part=head,facing=north,occupied=false]")?;
        summon_villager(&mut srv, "A", 5, 3)?;
        if !wait_memory(&mut srv, "A", "minecraft:home", head, 1200)? {
            bail!("A never claimed the bed (home={:?})", memory_pos(&mut srv, "A", "minecraft:home")?);
        }
        tp_tag(&mut srv, "A", 32, 0)?; // 30 blocks away: outside ValidateNearbyPoi's 16
        srv.step(5)?;
        bot_break(&mut srv, head)?; // survival break of the claimed bed
        srv.step(5)?;
        setblock(&mut srv, foot, "minecraft:white_bed[part=foot,facing=north,occupied=false]")?;
        setblock(&mut srv, head, "minecraft:white_bed[part=head,facing=north,occupied=false]")?;
        srv.step(5)?;
        summon_villager(&mut srv, "B", 5, -3)?;
        if !wait_memory(&mut srv, "B", "minecraft:home", head, 1200)? {
            bail!("B never claimed the replaced bed");
        }
        let a = memory_pos(&mut srv, "A", "minecraft:home")?;
        let b = memory_pos(&mut srv, "B", "minecraft:home")?;
        srv.cmd_ok("save-all flush")?;
        let world = srv.dir.join(&srv.level_name);
        ( (a, b), world )
    };
    let (a1, b1) = first;
    println!("bed-double: after replace A.home={a1:?} B.home={b1:?}");
    if a1 != Some(head) || b1 != Some(head) {
        bail!("expected double HOME at {head:?}");
    }
    // Persistence: restart on the saved world and re-read both memories.
    let mut srv = start(version, Some(second), keep_dir)?;
    srv.step(40)?;
    let a2 = memory_pos(&mut srv, "A", "minecraft:home")?;
    let b2 = memory_pos(&mut srv, "B", "minecraft:home")?;
    println!("bed-double: after restart A.home={a2:?} B.home={b2:?}");
    if a2 != Some(head) || b2 != Some(head) {
        bail!("double HOME did not survive save/load");
    }
    println!("PASS bed-double (two HOME memories, one bed ticket, survives restart)");
    Ok(())
}

// ---- R2: break+replace a workstation under a distant claimant ---------------

fn job_double(version: &str, keep_dir: bool) -> Result<()> {
    let site = [2, STAND_Y, 0];
    let mut srv = start(version, None, keep_dir)?;
    platform(&mut srv)?;
    setblock(&mut srv, site, "minecraft:lectern[facing=north,has_book=false,powered=false]")?;
    summon_villager(&mut srv, "A", 5, 3)?;
    if !wait_memory(&mut srv, "A", "minecraft:job_site", site, 1600)? {
        bail!("A never took the job site (job={:?}, profession={})", memory_pos(&mut srv, "A", "minecraft:job_site")?, profession(&mut srv, "A").unwrap_or_default());
    }
    if profession(&mut srv, "A")? != "minecraft:librarian" {
        bail!("A profession is {}", profession(&mut srv, "A")?);
    }
    lock_profession(&mut srv, "A")?;
    tp_tag(&mut srv, "A", 32, 0)?;
    srv.step(5)?;
    bot_break(&mut srv, site)?;
    srv.step(5)?;
    setblock(&mut srv, site, "minecraft:lectern[facing=north,has_book=false,powered=false]")?;
    srv.step(5)?;
    summon_villager(&mut srv, "B", 5, -3)?;
    if !wait_memory(&mut srv, "B", "minecraft:job_site", site, 3000)? {
        bail!(
            "B never took the replaced job site (B.potential={:?} B.job={:?} B.profession={} A.job={:?} lectern_present={})",
            memory_pos(&mut srv, "B", "minecraft:potential_job_site")?,
            memory_pos(&mut srv, "B", "minecraft:job_site")?,
            profession(&mut srv, "B").unwrap_or_default(),
            memory_pos(&mut srv, "A", "minecraft:job_site")?,
            srv.test_block(site, "minecraft:lectern")?
        );
    }
    let a_job = memory_pos(&mut srv, "A", "minecraft:job_site")?;
    let b_job = memory_pos(&mut srv, "B", "minecraft:job_site")?;
    println!("job-double: separated A.job={a_job:?} ({}) B.job={b_job:?} ({})", profession(&mut srv, "A")?, profession(&mut srv, "B")?);
    if a_job != Some(site) || b_job != Some(site) {
        bail!("expected double JOB_SITE at {site:?}");
    }
    // Mitigation check (informational): bring A back in sensor range; PoiCompetitorScan should
    // erase the lower-XP villager's JOB_SITE (both are XP-locked to 10 only for A).
    tp_tag(&mut srv, "A", 6, 0)?;
    srv.step(200)?;
    println!(
        "job-double: after reunion A.job={:?} B.job={:?} (competitor-scan mitigation)",
        memory_pos(&mut srv, "A", "minecraft:job_site")?,
        memory_pos(&mut srv, "B", "minecraft:job_site")?
    );
    println!("PASS job-double (two JOB_SITE memories while separated)");
    Ok(())
}

// ---- R4: piston-push a claimed workstation ------------------------------------

fn piston_push(version: &str, keep_dir: bool) -> Result<()> {
    let site = [2, STAND_Y, 0];
    let landed = [3, STAND_Y, 0];
    let mut srv = start(version, None, keep_dir)?;
    platform(&mut srv)?;
    setblock(&mut srv, site, "minecraft:composter[level=0]")?;
    summon_villager(&mut srv, "A", 5, 3)?;
    if !wait_memory(&mut srv, "A", "minecraft:job_site", site, 1600)? {
        bail!("A never took the composter job site");
    }
    if profession(&mut srv, "A")? != "minecraft:farmer" {
        bail!("A profession is {}", profession(&mut srv, "A")?);
    }
    lock_profession(&mut srv, "A")?;
    tp_tag(&mut srv, "A", 30, 10)?;
    srv.step(5)?;
    // Survival pusher: piston + lever on top (placed as scaffolding, flipped by the bot).
    setblock(&mut srv, [1, STAND_Y, 0], "minecraft:piston[facing=east,extended=false]")?;
    setblock(&mut srv, [1, STAND_Y + 1, 0], "minecraft:lever[face=floor,facing=north,powered=false]")?;
    srv.teleport_bot_to(1.5, (STAND_Y + 2) as f64, 2.5, Some((180.0, 45.0)))?;
    srv.bot_use([1, STAND_Y + 1, 0], 1)?;
    srv.step(10)?;
    if !srv.test_block(landed, "minecraft:composter")? {
        bail!("composter did not land at {landed:?}");
    }
    summon_villager(&mut srv, "B", 7, 3)?;
    if !wait_memory(&mut srv, "B", "minecraft:job_site", landed, 3000)? {
        bail!("B never claimed the pushed composter (fresh ticket not offered)");
    }
    tp_tag(&mut srv, "A", 8, 0)?;
    srv.step(400)?;
    let a_job = memory_pos(&mut srv, "A", "minecraft:job_site")?;
    let a_prof = profession(&mut srv, "A")?;
    println!("piston-push: A.job={a_job:?} A.profession={a_prof} B.job={:?}", memory_pos(&mut srv, "B", "minecraft:job_site")?);
    if a_job.is_some() || a_prof != "minecraft:farmer" {
        bail!("expected A jobless-but-farmer after the steal");
    }
    println!("PASS piston-push (ticket reset at landing cell, job stolen)");
    Ok(())
}

// ---- R5 (classification): phantom workstation after a poi/region rollback -----

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_tree(&p, &q)?;
        } else if e.file_name() != "session.lock" {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}

fn overworld_dir(world: &Path) -> PathBuf {
    let modern = world.join("dimensions").join("minecraft").join("overworld");
    if modern.exists() { modern } else { world.to_path_buf() }
}

fn copy_mcas(from_dir: &Path, to_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(to_dir)?;
    for e in std::fs::read_dir(from_dir)? {
        let e = e?;
        if e.file_name().to_string_lossy().ends_with(".mca") {
            std::fs::copy(e.path(), to_dir.join(e.file_name()))?;
        }
    }
    Ok(())
}

fn phantom_workstation(version: &str, keep_dir: bool) -> Result<()> {
    let site = [2, STAND_Y, 0];
    // Phase 1: claim a lectern; snapshot the region files from BEFORE the lectern existed.
    let (world, region_before) = {
        let mut srv = start(version, None, true)?;
        platform(&mut srv)?;
        srv.step(20)?;
        srv.cmd_ok("save-all flush")?;
        let world = srv.dir.join(&srv.level_name);
        let before = srv.dir.join("region-before");
        copy_mcas(&overworld_dir(&world).join("region"), &before)?;
        setblock(&mut srv, site, "minecraft:lectern[facing=north,has_book=false,powered=false]")?;
        summon_villager(&mut srv, "A", 5, 3)?;
        if !wait_memory(&mut srv, "A", "minecraft:job_site", site, 1600)? {
            bail!("A never took the lectern job site");
        }
        lock_profession(&mut srv, "A")?;
        force_used_offer(&mut srv, "A")?;
        srv.cmd_ok("save-all flush")?;
        (world, before)
    };
    // Phase 2 (CONTROLLED ROLLBACK, classification only): blocks revert, poi+entities stay.
    let rolled = world.with_file_name("world-rolled");
    if rolled.exists() {
        std::fs::remove_dir_all(&rolled)?;
    }
    copy_tree(&world, &rolled)?;
    copy_mcas(&region_before, &overworld_dir(&rolled).join("region"))?;
    // Phase 3: restart on the mismatched stores.
    let mut srv = start(version, Some(rolled), keep_dir)?;
    srv.cmd_ok("gamerule advance_time true")?;
    srv.cmd_ok("time set 2000")?;
    srv.teleport_bot(12.5, (STAND_Y + 2) as f64, 0.5)?;
    srv.step(20)?;
    if srv.test_block(site, "minecraft:lectern")? {
        bail!("rollback failed: lectern block still present");
    }
    let job = memory_pos(&mut srv, "A", "minecraft:job_site")?;
    let prof = profession(&mut srv, "A")?;
    let restock0 = long_field(&mut srv, "A", "LastRestock")?;
    println!("phantom-workstation: block absent, A.job={job:?} profession={prof} LastRestock={restock0}");
    if job != Some(site) || prof != "minecraft:librarian" {
        bail!("phantom claim did not survive the rollback (POI section was rebuilt)");
    }
    // Pen A next to the phantom site so WorkAtPoi can fire; watch for a restock at air.
    for (x, z) in [(1, 1), (2, 1), (3, 1), (1, -1), (2, -1), (3, -1), (1, 0), (3, 0)] {
        setblock(&mut srv, [x, STAND_Y, z], "minecraft:glass")?;
        setblock(&mut srv, [x, STAND_Y + 1, z], "minecraft:glass")?;
    }
    srv.cmd_ok(&format!("tp @e[type=minecraft:villager,tag=A] 2.5 {STAND_Y} 0.5"))?;
    let uses = force_used_offer(&mut srv, "A").unwrap_or(-1);
    println!("phantom-workstation: trade uses forced to {uses} (needsRestock needs >0)");
    let mut restocked = false;
    let mut worked = false;
    for _ in 0..300 {
        srv.step(20)?;
        worked |= has_memory(&mut srv, "A", "minecraft:last_worked_at_poi")?;
        if long_field(&mut srv, "A", "LastRestock")? > restock0 || long_field(&mut srv, "A", "RestocksToday")? > 0 {
            restocked = true;
            break;
        }
        if memory_pos(&mut srv, "A", "minecraft:job_site")?.is_none() {
            break;
        }
    }
    println!(
        "phantom-workstation: worked_at_air={worked} restocked_at_air={restocked} A.job={:?} LastRestock={} uses_now={}",
        memory_pos(&mut srv, "A", "minecraft:job_site")?,
        long_field(&mut srv, "A", "LastRestock")?,
        long_field(&mut srv, "A", "Offers.Recipes[0].uses").unwrap_or(-1)
    );
    if !restocked {
        bail!("no phantom restock observed within budget");
    }
    println!("PASS phantom-workstation (classification: valid POI section is never rebuilt)");
    Ok(())
}
