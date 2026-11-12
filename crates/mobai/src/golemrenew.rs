//! (tickexact 2026-11-12, ironspec: does a summoner's GOLEM_DETECTED_RECENTLY get renewed after its
//! own summon?) `mctas mobai golem-renew`: on the official 26.3 server, stands 64 apart, each three
//! villagers seated in carts 2 apart (LAST_SLEPT set at their summon, so they may summon) and a husk
//! in a boat 5 from them (in sight: they panic; a husk does not burn); a panicking villager's
//! VillagerPanicTrigger tries the summon at every game time % 100 == 0, the iron golem lands on the
//! open ground around and is removed after `--golem-ticks` whole ticks (the farm's golem leaves
//! through its pad's portal on its 2nd tick: 2), so the villagers' NEAREST_LIVING_ENTITIES can catch
//! it only then. Every villager's GOLEM_DETECTED_RECENTLY sets and erasures are logged (the brain
//! agent) from the stand's summon T to T + 710: a set after T is a renewal (GolemSensor, every
//! 200 gt, reading a living list that caught the golem), and the summon at T + 600 (the stamps set
//! at T with 599 gt to live are gone by then) takes place only if none was renewed. Each round
//! summons every stand afresh (new sensor phases); `--rounds` rounds.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 64;
const MEMS: &str = "minecraft:golem_detected_recently";
const SPACING: i32 = 64;

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

/// One stand's round: its villagers' UUIDs, its summon tick, each villager's set ticks after it,
/// whether the summon at T + 600 took place.
#[derive(Default)]
struct Trial {
    villagers: Vec<String>,
    summon: Option<i64>,
    sets: BTreeMap<String, Vec<i64>>,
    erases: BTreeMap<String, Vec<i64>>,
    /// The ticks a new golem appeared at this stand.
    golems: Vec<i64>,
    resummon: Option<bool>,
}

pub fn golem_renew(version: &str, stands: u32, rounds: u32, golem_ticks: u32) -> Result<()> {
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
    println!(
        "== golem-renew on the official {version} server: {stands} stands x {rounds} rounds, each three villagers seated in carts (LAST_SLEPT set) panicking at a husk in a boat; each golem removed after {golem_ticks} whole ticks; GOLEM_DETECTED_RECENTLY logged from the summon T to T + 710"
    );
    for k in 0..stands as i32 {
        let x0 = SPACING * k;
        srv.cmd_ok(&format!("forceload add {} -16 {} 24", x0 - 16, x0 + 16))?;
    }
    // every stand's chunks loaded before anything is placed there
    for _ in 0..200 {
        srv.step(5)?;
        let mut loaded = true;
        for k in 0..stands as i32 {
            let x0 = SPACING * k;
            for (x, z) in [(x0 - 10, -10), (x0 + 10, 14), (x0, 0)] {
                if !srv.cmd_long(&format!("execute if loaded {x} {Y} {z}"))?.contains("passed") {
                    loaded = false;
                }
            }
        }
        if loaded {
            break;
        }
    }
    // each stand's floor (the world is a void): stone under the villagers and all around them, the
    // ground the golem's spawn attempts (within 8 of the summoner) land on
    for k in 0..stands as i32 {
        let x0 = SPACING * k;
        srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:stone", x0 - 10, Y - 1, -10, x0 + 10, Y - 1, 14))?;
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
    let mut all: Vec<Trial> = Vec::new();
    for round in 0..rounds {
        // a fresh round: every stand's mobs summoned again (new sensor phases), once the last
        // round's are gone (a dying villager stays in the world 20 gt and counts in a summoner's
        // or a gossiper's villager list: 3 new + 3 dying = a 5-villager gossip summon)
        srv.cmd_ok("kill @e[type=!minecraft:player]")?;
        srv.step(30)?;
        let now = srv.game_time()?;
        let mut trials: Vec<Trial> = Vec::new();
        for k in 0..stands as i32 {
            let x0 = SPACING * k;
            let mut t = Trial::default();
            for (i, dz) in [0, 2, 4].iter().enumerate() {
                // (the rail stays from the last round: a refused set is the same rail)
                srv.cmd(&format!("setblock {x0} {Y} {dz} minecraft:rail"))?;
                let tag = format!("gr{round}s{k}v{i}");
                srv.cmd_ok(&format!(
                    "summon minecraft:minecart {x0}.5 {}.0625 {dz}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}\"],Brain:{{memories:{{\"minecraft:last_slept\":{{value:{now}L}}}}}}}}]}}",
                    Y
                ))?;
                t.villagers.push(uuid_of(&mut srv, &format!("@e[tag={tag},limit=1]"))?);
            }
            srv.cmd_ok(&format!("summon minecraft:oak_boat {}.5 {Y} 2.5 {{Passengers:[{{id:\"minecraft:husk\",PersistenceRequired:1b}}]}}", x0 + 5))?;
            trials.push(t);
        }
        drain(&srv)?;
        // step until every stand has summoned and run 650 gt past it (or 1200 gt without a summon)
        let start = srv.game_time()?;
        loop {
            srv.step(1)?;
            let t = srv.game_time()?;
            for l in drain(&srv)? {
                let Some(who) = l.rsplit(" @").next().map(|u| u.trim().to_string()) else { continue };
                let erase = l.starts_with("erase minecraft:golem_detected_recently");
                if !l.starts_with("set minecraft:golem_detected_recently") && !erase {
                    continue;
                }
                for tr in trials.iter_mut() {
                    if tr.villagers.contains(&who) {
                        if erase {
                            tr.erases.entry(who.clone()).or_default().push(t);
                            continue;
                        }
                        if tr.summon.is_none() {
                            tr.summon = Some(t);
                        }
                        tr.sets.entry(who.clone()).or_default().push(t);
                    }
                }
            }
            // every new golem: its tick and stand (a summon)
            let fresh = srv.cmd_long("execute as @e[type=minecraft:iron_golem,tag=!ga1] run data get entity @s Pos[0]")?;
            if std::env::var_os("MCTAS_GOLEM_RENEW_DEBUG").is_some() && !fresh.trim().is_empty() {
                eprintln!("t {t} new golems: {}", fresh.trim());
            }
            for part in fresh.split("entity data: ").skip(1) {
                let num: String = part.chars().take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.').collect();
                if let Ok(x) = num.parse::<f64>() {
                    let k = (x / SPACING as f64).round() as usize;
                    if let Some(tr) = trials.get_mut(k) {
                        tr.golems.push(t);
                    }
                }
            }
            // the golems leave after `golem_ticks` whole ticks (tags count their ticks)
            for age in (1..=golem_ticks).rev() {
                if age == golem_ticks {
                    srv.cmd(&format!("kill @e[type=minecraft:iron_golem,tag=ga{age}]"))?;
                } else {
                    srv.cmd(&format!("tag @e[type=minecraft:iron_golem,tag=ga{age}] add ga{}", age + 1))?;
                }
            }
            srv.cmd("tag @e[type=minecraft:iron_golem,tag=!ga1] add ga1")?;
            let finished = trials.iter().all(|tr| tr.summon.is_some_and(|s| t >= s + 710)) || t - start > 1200 + 760;
            if finished {
                break;
            }
        }
        for tr in trials.iter_mut() {
            if let Some(s) = tr.summon {
                // the summon at T + 600: golemDetected written to the stand's villagers at T + 600
                // (the summoner's tick writes all three brains; the agent names the summoner), and a
                // new golem at the stand then
                tr.resummon = Some(tr.villagers.iter().any(|v| tr.sets.get(v).is_some_and(|ts| ts.contains(&(s + 600)))) && tr.golems.contains(&(s + 600)));
                if std::env::var_os("MCTAS_GOLEM_RENEW_DEBUG").is_some() {
                    let rel = |v: &[i64]| v.iter().map(|x| format!("{:+}", x - s)).collect::<Vec<_>>().join(" ");
                    eprintln!("  stand trial T={s}: golems at {}", rel(&tr.golems));
                    for v in &tr.villagers {
                        eprintln!("    {}: sets {} | erases {}", &v[..8], rel(tr.sets.get(v).map(|x| x.as_slice()).unwrap_or(&[])), rel(tr.erases.get(v).map(|x| x.as_slice()).unwrap_or(&[])));
                    }
                }
            }
        }
        all.extend(trials);
        eprintln!("round {} done ({} trials with a summon so far)", round + 1, all.iter().filter(|t| t.summon.is_some()).count());
    }
    // the tally
    let summoned: Vec<&Trial> = all.iter().filter(|t| t.summon.is_some()).collect();
    let (mut vt, mut renewed, mut ticks) = (0u32, 0u32, Vec::new());
    for tr in &summoned {
        let s = tr.summon.unwrap_or(0);
        for v in &tr.villagers {
            vt += 1;
            let after: Vec<i64> = tr.sets.get(v).map(|ts| ts.iter().copied().filter(|&x| x > s && x < s + 600).collect()).unwrap_or_default();
            if !after.is_empty() {
                renewed += 1;
                ticks.push(format!("T+{}", after.iter().map(|x| (x - s).to_string()).collect::<Vec<_>>().join("/")));
            }
        }
    }
    let blocked = summoned.iter().filter(|t| t.resummon == Some(false)).count();
    // the blocked summons with no renewal: what else stopped them (each trial's writes and golems)
    let renewed_stand = |tr: &Trial| {
        let s = tr.summon.unwrap_or(0);
        tr.villagers.iter().any(|v| tr.sets.get(v).is_some_and(|ts| ts.iter().any(|&x| x > s && x < s + 600)))
    };
    let unexplained: Vec<&&Trial> = summoned.iter().filter(|t| t.resummon == Some(false) && !renewed_stand(t)).collect();
    for tr in &unexplained {
        let s = tr.summon.unwrap_or(0);
        let rel = |v: &[i64]| v.iter().map(|x| format!("{:+}", x - s)).collect::<Vec<_>>().join(" ");
        println!("  blocked with no renewal: T={s}, golems at {}; writes by villager: {}", rel(&tr.golems), tr.villagers.iter().map(|v| format!("{} [{}]", &v[..8], rel(tr.sets.get(v).map(|x| x.as_slice()).unwrap_or(&[])))).collect::<Vec<_>>().join("; "));
    }
    println!("  {} stand-trials, {} with a summon at T (a game time % 100 == 0: {}); {} without", all.len(), summoned.len(), summoned.iter().filter(|t| t.summon.unwrap_or(1) % 100 == 0).count(), all.len() - summoned.len());
    println!(
        "  villager-trials with GOLEM_DETECTED_RECENTLY renewed after T (before T + 600): {renewed} of {vt} ({:.2}%){}",
        if vt > 0 { 100.0 * renewed as f64 / vt as f64 } else { 0.0 },
        if ticks.is_empty() { String::new() } else { format!("; renewals at {}", ticks.join(" ")) }
    );
    println!(
        "  the summon at T + 600: took place in {} of {} stand-trials; blocked in {blocked} ({} with a renewed villager, {} without)",
        summoned.len() - blocked,
        summoned.len(),
        blocked - unexplained.len(),
        unexplained.len()
    );
    // (ironspec 12:55) every summon tick after T at which the stand wanted (each villager's last
    // stamp, the last summon or its own renewal, 599 gt old or more): the outcome, a new golem or
    // none (trySpawnMob found no spot: it writes nothing, so the stand still wants 100 gt later)
    let (mut wanted, mut ok, mut failed, mut edge) = (0u32, 0u32, Vec::new(), Vec::new());
    for tr in &summoned {
        let s = tr.summon.unwrap_or(0);
        for k in [6i64, 7] {
            let t = s + 100 * k;
            let last_summon = tr.golems.iter().copied().filter(|&g| g < t).max().unwrap_or(s);
            let ages: Vec<i64> = tr
                .villagers
                .iter()
                .map(|v| {
                    let own = tr.sets.get(v).and_then(|ts| ts.iter().copied().filter(|&x| x < t && x > s && !tr.golems.contains(&x)).max()).unwrap_or(i64::MIN);
                    t - last_summon.max(own)
                })
                .collect();
            // a stamp 600 gt old is gone; one 599 gt old (an own renewal the tick after the summon)
            // is gone only if that villager's brain ticked before the summoner's that tick
            if ages.iter().all(|&a| a >= 599) {
                wanted += 1;
                if tr.golems.contains(&t) {
                    ok += 1;
                } else if ages.iter().any(|&a| a == 599) {
                    edge.push(format!("T={s} at T+{}: stamps {} gt old, no golem", 100 * k, ages.iter().map(|a| a.to_string()).collect::<Vec<_>>().join("/")));
                } else {
                    failed.push(format!("T={s} at T+{}: stamps {} gt old, no golem", 100 * k, ages.iter().map(|a| a.to_string()).collect::<Vec<_>>().join("/")));
                }
            }
        }
    }
    println!(
        "  summon ticks at T + 600 and T + 700 where every stamp was 599+ gt old: {wanted}; a golem: {ok}; none with a 599-gt stamp (a renewal at T+1, still held when the summoner ticked first): {}{}; none with every stamp 600+ gt old (trySpawnMob found no spot): {}{}",
        edge.len(),
        if edge.is_empty() { String::new() } else { format!(" ({})", edge.join("; ")) },
        failed.len(),
        if failed.is_empty() { String::new() } else { format!(" ({})", failed.join("; ")) }
    );
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
