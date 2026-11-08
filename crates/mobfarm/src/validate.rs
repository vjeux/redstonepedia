//! The brief's one-off game checks (`mctas mobfarm validate`): a mob farm built in the real 26.3
//! server through the bridge, run for game days, against redsim's runs of the same design.
//!
//! The design's blocks are pasted like the judge pastes them; the judge's chest goes at
//! `port output`; time runs (`advance_time`), weather stays clear, natural mob spawning is off.
//! Each villager is summoned standing where the design puts it, persistent, with a `LAST_SLEPT`
//! memory at the end of the last REST (the judge's steady state); it claims its bed itself. The
//! zombie rides its boat (`Passengers`). Every 20 gt the run lists the entities: each new iron
//! golem's spawn time and cell (`--kill`: then it is killed at once, for the spawn-spot
//! statistics). At the end it reads the chest. A `/summon` with data skips `finalizeSpawn`, so
//! the game's villagers have a follow range of exactly 16 (redsim draws the bonus a villager
//! brought in by the player has): noted in the comparison.

use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use mctas::sim::{PlaceMode, Pool, SandboxOptions};

/// What one game run saw.
#[derive(Debug, Default)]
pub struct GameRun {
    pub ticks: u32,
    /// Each golem's first sighting: game ticks since the start, its block position.
    pub golems: Vec<(u32, [i32; 3])>,
    pub iron_in_chest: u32,
    pub poppies_in_chest: u32,
}

impl GameRun {
    pub fn golems_per_hour(&self) -> f64 {
        self.golems.len() as f64 * 72000.0 / self.ticks as f64
    }
}

fn count_in(nbt: &str, item: &str) -> u32 {
    // `{id:"minecraft:iron_ingot",count:5}` entries of an `Items` list
    let mut total = 0;
    let key = format!("id:\"minecraft:{item}\"");
    let mut rest = nbt;
    while let Some(k) = rest.find(&key) {
        let after = &rest[k + key.len()..];
        let entry_end = after.find('}').unwrap_or(after.len());
        let entry = &after[..entry_end];
        let before = &rest[..k];
        let entry_start = before.rfind('{').unwrap_or(0);
        let whole = format!("{}{}", &rest[entry_start..k], entry);
        let n = whole.split("count:").nth(1).map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
        total += n;
        rest = &after[entry_end..];
    }
    total
}

/// Run a mob farm in the game for `days` game days.
pub fn game_run(pool: &Pool, c: &Circuit, days: u32, kill: bool, trace: bool, seed: i64) -> Result<GameRun> {
    let d = &c.clock.design;
    let out = *d.ports.get("output").ok_or_else(|| anyhow!("no port output"))?;
    let (from, to) = circuit::run::watch_box(c, 4);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let mut w = pool.worker(0);
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    let r = (|| -> Result<GameRun> {
        let blocks = d.block_list();
        w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
        let positions: Vec<_> = blocks.iter().map(|(p, _)| *p).collect();
        w.update(sb, &positions, "self")?;
        w.set_blocks(sb, &[(out, "minecraft:chest[facing=north,type=single,waterlogged=false]")], PlaceMode::Raw)?;
        // the sandbox level keeps its own game rules (frozen time); `time add` moves every level's
        // day time, so the run advances it by hand, 20 gt with every 20 gt step
        for cmd in ["weather clear", "time set 1000"] {
            w.exec(sb, cmd).map_err(|e| anyhow!("{cmd}: {e:#}"))?;
        }
        let t0 = w.status(sb)?.time;
        let slept = t0 - 990;
        for e in &d.entities {
            let k = e.kind.trim_start_matches("minecraft:");
            let [x, y, z] = e.pos;
            let cmd = match k {
                "villager" => format!("summon minecraft:villager {x} {y} {z} {{PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:last_slept\":{{value:{slept}L}}}}}}}}"),
                _ if k.ends_with("boat") => format!("summon minecraft:{k} {x} {y} {z} {{Passengers:[{{id:\"minecraft:zombie\",PersistenceRequired:1b,IsBaby:0b}}]}}"),
                other => bail!("validate: entity {other} not handled"),
            };
            w.exec(sb, &cmd).map_err(|e| anyhow!("{cmd}: {e:#}"))?;
        }
        let mut run = GameRun { ticks: days * 24000, ..Default::default() };
        let mut seen: Vec<String> = Vec::new();
        let mut t = 0;
        while t < run.ticks {
            w.step(sb, 20)?;
            w.exec(sb, "time add 20")?;
            t += 20;
            let ents = w.entities(sb)?;
            if trace && t % 1000 == 0 {
                for e in ents.iter().filter(|e| e.kind.ends_with("villager")) {
                    let mem = |k: &str| e.state.find(&format!("\"minecraft:{k}\"")).map(|i| e.state[i..].chars().take(90).collect::<String>()).unwrap_or_else(|| "-".into());
                    println!("t {t} villager y {:.4}: home {} | last_slept {} | golem {}", e.pos[1], mem("home"), mem("last_slept"), mem("golem_detected_recently"));
                }
            }
            for e in ents {
                if e.kind.ends_with("iron_golem") && !seen.contains(&e.uuid) {
                    seen.push(e.uuid.clone());
                    run.golems.push((t, [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
                }
            }
            if kill {
                w.exec(sb, "kill @e[type=minecraft:iron_golem]")?;
            }
        }
        for be in w.block_entities(sb, out, out)? {
            run.iron_in_chest += count_in(&be.nbt, "iron_ingot");
            run.poppies_in_chest += count_in(&be.nbt, "poppy");
        }
        Ok(run)
    })();
    let _ = w.drop_sandbox(sb);
    r
}
