//! (tickexact 2026-11-12, ironspec 19:20/19:34 PT (b)(c)(e)) `mctas mobai gossip-row`: on the
//! official 26.3 server, two villagers seated in carts on rails 1 apart (`--layout side`: x 0 and x
//! 1; `line`: z 0 and z 1), at day time `--day-time` (100: IDLE), held. Villager A carries a seeded
//! gossip (minor_positive 50 about a made-up UUID); every tick both villagers' Gossips are read:
//! the first tick B holds A's entry is a gossip (Villager.gossip: B took A's gossips; both
//! lastGossipTimes set to that tick). Then a second entry is seeded on A: without a restart B must
//! not take it before the first gossip + 1200 (the lastGossipTime gate); `--restart-after N`
//! restarts the world N gt after the first gossip (lastGossipTime is not saved: B takes the second
//! entry at the next TradeWithVillager tick). The brain agent logs INTERACTION_TARGET and
//! GOLEM_DETECTED_RECENTLY writes.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::io::{Read as _, Seek as _, SeekFrom};

const Y: i32 = 100;
const MEMS: &str = "minecraft:interaction_target,minecraft:golem_detected_recently";
const T1: &str = "[I;11,22,33,44]";
const T2: &str = "[I;55,66,77,88]";
const T3: &str = "[I;99,98,97,96]";
const T4: &str = "[I;95,94,93,92]";

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

fn holds(srv: &mut VanillaServer, tag: &str, target: &str) -> Result<bool> {
    let g = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Gossips"))?;
    let ints = target.trim_start_matches("[I;").trim_end_matches(']').replace(',', ", ");
    Ok(g.contains(&format!("[I; {ints}]")))
}

pub fn gossip_row(version: &str, layout: &str, day_time: i64, ticks: u32, restart_after: Option<u32>) -> Result<()> {
    let jar = mctas::bridge::brain_agent(version)?;
    let mut srv = VanillaServer::start(&opts(version, &jar, None))?;
    srv.cmd_ok("gamerule advance_time false")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok(&format!("time set {day_time}"))?;
    srv.cmd_ok("forceload add -16 -16 16 16")?;
    srv.step(20)?;
    // `standing`: the positive control, two villagers on foot 1 apart, each fenced in by glass
    let standing = layout == "standing";
    let cells: [[i32; 2]; 2] = if layout == "line" { [[0, 0], [0, 1]] } else { [[0, 0], [1, 0]] };
    if standing {
        srv.cmd_ok(&format!("fill -1 {} -1 2 {} 1 minecraft:glass", Y - 1, Y + 2))?;
        srv.cmd_ok(&format!("fill 0 {Y} 0 1 {} 0 minecraft:air", Y + 1))?;
    }
    for (k, c) in cells.iter().enumerate() {
        let gossip = format!(",Gossips:[{{Type:\"minor_positive\",Value:50,Target:{}}}]", if k == 0 { T1 } else { T3 });
        if standing {
            srv.cmd_ok(&format!("summon minecraft:villager {}.5 {Y} {}.5 {{PersistenceRequired:1b,Tags:[\"gv{k}\"]{gossip}}}", c[0], c[1]))?;
            continue;
        }
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:stone", c[0], Y - 1, c[1]))?;
        srv.cmd_ok(&format!("setblock {} {Y} {} minecraft:rail[shape=north_south]", c[0], c[1]))?;
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {}.5 {}.0625 {}.5 {{Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"gv{k}\"]{gossip}}}]}}",
            c[0], Y, c[1]
        ))?;
    }
    anyhow::ensure!(holds(&mut srv, "gv0", T1)? && holds(&mut srv, "gv1", T3)?, "the seeded gossips did not read back: {}", srv.cmd_long("data get entity @e[tag=gv0,limit=1] Gossips")?.trim());
    println!(
        "== gossip-row on the official {version} server: two villagers {} at {cells:?} (y {Y}), day time {day_time} held; A (gv0) seeded with a minor_positive gossip (read back); {ticks} gt{}",
        if standing { "on foot in a glass pen (the positive control)" } else { "seated in carts" },
        restart_after.map(|r| format!("; a restart {r} gt after the first gossip")).unwrap_or_default()
    );
    let mut log = Log { at: 0 };
    log.drain(&srv)?;
    let (mut first, mut second, mut restarted, mut seeded2) = (None::<u32>, None::<u32>, None::<u32>, false);
    let (mut first_how, mut second_how) = ("", "");
    let mut interact: Vec<String> = Vec::new();
    let mut stamps = 0u32;
    for t in 1..=ticks {
        if let (Some(f), Some(r)) = (first, restart_after)
            && restarted.is_none()
            && t == f + r
        {
            // the probe: villager A (seated riders hold still; a standing one may wander)
            let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, &jar, Some(w)), Some("@e[tag=gv0,limit=1]"))?;
            srv = s;
            println!("  t{t}: restart ({})", rl.describe());
            srv.cmd_ok("forceload add -16 -16 16 16")?;
            log.at = 0;
            restarted = Some(t);
        }
        srv.step(1)?;
        for l in log.drain(&srv)? {
            if l.contains("interaction_target") && interact.len() < 12 {
                interact.push(format!("t{t} {l}"));
            }
            if l.starts_with("set minecraft:golem_detected_recently") {
                stamps += 1;
            }
        }
        // a gossip either way: B took A's entry (B ran gossip with A) or A took B's
        let took = |srv: &mut VanillaServer, a: &str, b: &str| -> Result<Option<&'static str>> {
            Ok(if holds(srv, "gv1", a)? {
                Some("B took A's entry")
            } else if holds(srv, "gv0", b)? {
                Some("A took B's entry")
            } else {
                None
            })
        };
        if first.is_none() {
            if let Some(how) = took(&mut srv, T1, T3)? {
                first = Some(t);
                first_how = how;
                // second entries on both, for the 1200-gt gate (and the reset on a restart)
                srv.cmd_ok(&format!("data modify entity @e[tag=gv0,limit=1] Gossips append value {{Type:\"minor_positive\",Value:50,Target:{T2}}}"))?;
                srv.cmd_ok(&format!("data modify entity @e[tag=gv1,limit=1] Gossips append value {{Type:\"minor_positive\",Value:50,Target:{T4}}}"))?;
                seeded2 = true;
            }
        } else if seeded2 && second.is_none() {
            if let Some(how) = took(&mut srv, T2, T4)? {
                second = Some(t);
                second_how = how;
                if restart_after.is_none() || restarted.is_some() {
                    break;
                }
            }
        }
    }
    println!(
        "  the first gossip: {}; the second (entries seeded on both at that tick): {}{}; GOLEM_DETECTED_RECENTLY stamps {stamps}",
        first.map(|t| format!("t{t} ({first_how})")).unwrap_or_else(|| format!("never in {ticks} gt")),
        second.map(|t| format!("t{t} ({second_how}){}", first.map(|f| format!(", first + {}", t - f)).unwrap_or_default())).unwrap_or_else(|| "never".into()),
        restarted.map(|t| format!("; restarted at t{t}")).unwrap_or_default()
    );
    println!("  INTERACTION_TARGET writes (first 12): {interact:?}");
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
