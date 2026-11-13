//! (tickexact 2026-11-13, ironspec 02:27 PT: B3's bed stop) `mctas mobai player-sleep`: on the official
//! 26.3 server, running freely at 20 TPS (a sleeping player's counter ticks with the server, frozen
//! or not), the bot lies down in a bed at night, sleeps ~60 gt, and the world closes one of three
//! ways with the bot in bed at the save:
//! - `restart`: `save-all flush`, then `stop` with the bot still connected
//!   (MinecraftServer.stopServer saves every player before removing it: a dedicated server's
//!   restart);
//! - `quit`: the bot logs out from its bed (ServerPlayer.disconnect wakes it, PlayerList.remove
//!   saves it), then the save and stop: singleplayer's save-and-quit;
//! - `crash`: `save-all flush`, then the process killed without a stop (the stored player asleep).
//!
//! The world opens again and the bot logs back in. A command-block chain in a forced chunk stamps,
//! every tick from the world's first: the game time, the first tick with a player present (the
//! join), whether that player is asleep at its join tick, the ticks it spends asleep, its lie-down
//! tick and the night skip's tick (the day time jumping). Then, awake, the bot lies down again and
//! the skip's tick is measured against that post-load lie-down (isSleepingLongEnough: the sleep
//! counter reaching 100). Bars: no player asleep at its first tick after a world open; the skip
//! at least 100 gt after the first post-load lie-down.

use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;
const BED: [i32; 3] = [2, Y + 1, 2];

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), with_bot: true, tick_rate: 20, keep_dir: true, frozen: false, world, ..Default::default() }
}

fn score(srv: &mut VanillaServer, name: &str) -> Result<i64> {
    let out = srv.cmd(&format!("scoreboard players get {name} bt"))?;
    Ok(out.split(" has ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0))
}

fn nbt_num(srv: &mut VanillaServer, bot: &str, path: &str) -> Result<Option<i64>> {
    let out = srv.cmd(&format!("data get entity {bot} {path}"))?;
    Ok(out.split("entity data: ").nth(1).and_then(|r| r.trim().trim_end_matches(['s', 'b', 'L']).parse::<f64>().ok()).map(|v| v as i64))
}

/// Whether the player sleeps: 26.3 stores LivingEntity's sleeping position as `sleeping_pos`
/// (LivingEntity.addAdditionalSaveData), present only while it sleeps.
fn asleep(srv: &mut VanillaServer, bot: &str) -> Result<bool> {
    Ok(srv.cmd(&format!("data get entity {bot} sleeping_pos"))?.contains("entity data"))
}

fn wait_until(srv: &mut VanillaServer, what: &str, secs: u64, mut f: impl FnMut(&mut VanillaServer) -> Result<bool>) -> Result<()> {
    let t0 = std::time::Instant::now();
    while !f(srv)? {
        if t0.elapsed().as_secs() > secs {
            bail!("{what}: not within {secs} s");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

/// The command-block chain (a repeating block and its chain, facing east at y Y - 5 in chunk 0,0).
fn lay_chain(srv: &mut VanillaServer) -> Result<()> {
    srv.cmd_ok("scoreboard objectives add bt dummy")?;
    let cmds = [
        "execute store result score #gt bt run time query gametime",
        "execute store result score #day bt run time query time",
        // a tick with no player (the world's first ticks after an open, before the login)
        "execute unless entity @a run scoreboard players set #empty bt 1",
        // the join: the first tick with a player after a tick without one
        "execute if entity @a if score #empty bt matches 1 unless score #join bt matches 1.. run scoreboard players operation #join bt = #gt bt",
        // asleep at that tick
        "execute if score #gt bt = #join bt as @a if data entity @s sleeping_pos run scoreboard players set #asleep1 bt 1",
        // ticks asleep since the join
        "execute if score #join bt matches 1.. as @a if data entity @s sleeping_pos run scoreboard players add #asleepn bt 1",
        // the lie-down: the first tick asleep since the reset (after the join)
        "execute if score #join bt matches 1.. as @a if data entity @s sleeping_pos unless score #lie bt matches 1.. run scoreboard players operation #lie bt = #gt bt",
        // the skip: the day time moving by anything but 0..2 in a tick
        "scoreboard players operation #dd bt = #day bt",
        "scoreboard players operation #dd bt -= #prev bt",
        "execute if score #prev bt matches 1.. unless score #dd bt matches 0..2 unless score #skip bt matches 1.. run scoreboard players operation #skip bt = #gt bt",
        "scoreboard players operation #prev bt = #day bt",
    ];
    for (i, c) in cmds.iter().enumerate() {
        let kind = if i == 0 { "repeating_command_block" } else { "chain_command_block" };
        srv.cmd_ok(&format!("setblock {} {} 0 minecraft:{kind}[facing=east]{{Command:\"{c}\",auto:1b}}", i, Y - 5))?;
    }
    Ok(())
}

fn reset(srv: &mut VanillaServer) -> Result<()> {
    for s in ["#empty", "#join", "#asleep1", "#asleepn", "#lie", "#skip", "#prev"] {
        srv.cmd_ok(&format!("scoreboard players set {s} bt 0"))?;
    }
    Ok(())
}

/// Before a lie-down measured with the player already present: only the lie-down and skip stamps
/// cleared, the join kept (or set: the chain stamps a lie-down only after a join).
fn reset_lie(srv: &mut VanillaServer) -> Result<()> {
    for s in ["#lie", "#skip", "#prev"] {
        srv.cmd_ok(&format!("scoreboard players set {s} bt 0"))?;
    }
    srv.cmd_ok("execute unless score #join bt matches 1.. run scoreboard players set #join bt 1")?;
    Ok(())
}

fn lie_down(srv: &mut VanillaServer, bot: &str) -> Result<()> {
    srv.teleport_bot(BED[0] as f64 + 0.5, Y as f64 + 1.0, BED[2] as f64 - 1.5)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    srv.bot_use(BED, 1)?;
    if let Err(e) = wait_until(srv, "the bot lying down", 10, |s| asleep(s, bot)) {
        let foot = srv.cmd(&format!("execute if block {} {} {} minecraft:red_bed[part=foot] run say foot ok", BED[0], BED[1], BED[2]))?;
        let head = srv.cmd(&format!("execute if block {} {} {} minecraft:red_bed[part=head] run say head ok", BED[0], BED[1], BED[2] + 1))?;
        let pos = srv.cmd(&format!("data get entity {bot} Pos"))?;
        let time = srv.cmd("time query time")?;
        let log = std::fs::read_to_string(&srv.log).unwrap_or_default();
        let tail: Vec<&str> = log.lines().rev().take(12).collect();
        println!("  lie-down failed: foot {foot:?}; head {head:?}; the bot {pos:?}; time {time:?}; the log's tail (newest first): {}", tail.join(" | "));
        return Err(e);
    }
    Ok(())
}

/// One row: (asleep at the first tick after the open, skip - post-load lie-down).
fn row(version: &str, how: &str) -> Result<(bool, Option<i64>)> {
    let bot = VanillaOptions::default().bot_name;
    let mut srv = VanillaServer::start(&opts(version, None))?;
    for c in ["gamerule advance_time true", "gamerule spawn_mobs false", "difficulty peaceful", "forceload add 0 0 15 15", "save-on"] {
        srv.cmd_ok(c)?;
    }
    srv.cmd_ok(&format!("fill -2 {Y} -2 6 {Y} 6 minecraft:glass"))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=south,part=foot]", BED[0], BED[1], BED[2]))?;
    srv.cmd_ok(&format!("setblock {} {} {} minecraft:red_bed[facing=south,part=head]", BED[0], BED[1], BED[2] + 1))?;
    lay_chain(&mut srv)?;
    srv.cmd_ok("time set 13000")?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    reset(&mut srv)?;
    reset_lie(&mut srv)?;
    lie_down(&mut srv, &bot)?;
    // ~60 gt asleep (the skip comes at 100), then the save
    wait_until(&mut srv, "SleepTimer 60", 10, |s| Ok(nbt_num(s, &bot, "SleepTimer")?.is_some_and(|t| t >= 60)))?;
    let lie0 = score(&mut srv, "#lie")?;
    let timer = nbt_num(&mut srv, &bot, "SleepTimer")?.unwrap_or(-1);
    let gt0 = srv.game_time()?;
    reset(&mut srv)?;
    let world = srv.dir.join(&srv.level_name);
    let old = srv.dir.clone();
    match how {
        "restart" => {
            srv.cmd_ok("save-all flush")?;
            srv.stop_with_bot()?;
        }
        "quit" => {
            srv.bot = None;
            std::thread::sleep(std::time::Duration::from_millis(1000));
            srv.cmd_ok("save-all flush")?;
            srv.stop_with_bot()?;
        }
        "crash" => {
            srv.cmd_ok("save-all flush")?;
            srv.kill_hard()?;
        }
        other => bail!("--how restart, quit or crash, got {other}"),
    }
    drop(srv);
    println!("  {how}: lay down at gt {lie0}; at the save (gt {gt0}) SleepTimer {timer}, the bot in bed; the world closed ({how})");
    let mut srv = VanillaServer::start(&opts(version, Some(world)))?;
    let _ = std::fs::remove_dir_all(&old);
    // VanillaServer::start turns the daylight cycle off: the skip moves the day time only with it on
    srv.cmd_ok("gamerule advance_time true")?;
    // let the chain read the first ticks with the player, then read it
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let (join, asleep1, asleepn) = (score(&mut srv, "#join")?, score(&mut srv, "#asleep1")?, score(&mut srv, "#asleepn")?);
    let (sx, st) = (asleep(&mut srv, &bot)?, nbt_num(&mut srv, &bot, "SleepTimer")?);
    let skip_early = score(&mut srv, "#skip")?;
    println!(
        "  {how}: after the open the player joined at gt {join}: asleep at its first tick: {}; ticks asleep since: {asleepn}; now sleeping_pos {}, SleepTimer {}; a skip so far: {}",
        if asleep1 == 1 { "YES" } else { "no" },
        if sx { "present (asleep)" } else { "absent" },
        st.map(|v| v.to_string()).unwrap_or_else(|| "none".into()),
        if skip_early > 0 { format!("gt {skip_early}") } else { "none".into() }
    );
    if join == 0 {
        bail!("{how}: NOT JUDGED (0 samples): no tick with the player stamped after the open");
    }
    // (b) the post-load lie-down and the skip
    srv.cmd_ok("time set 13000")?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    reset_lie(&mut srv)?;
    lie_down(&mut srv, &bot)?;
    wait_until(&mut srv, "the night skip", 20, |s| Ok(score(s, "#skip")? > 0))?;
    let (lie, skip) = (score(&mut srv, "#lie")?, score(&mut srv, "#skip")?);
    println!("  {how}: the post-load lie-down at gt {lie}; the night skipped at gt {skip}: {} gt after the lie-down", skip - lie);
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok((asleep1 == 1, (lie > 0).then_some(skip - lie)))
}

pub fn player_sleep(version: &str, hows: &[String]) -> Result<()> {
    println!(
        "== player-sleep on the official {version} server at 20 TPS: a bed at ({},{},{}), the bot in it ~60 gt, the world closed with the bot in bed at the save ({}), opened again; a command-block chain stamps the player's first tick, whether it is asleep there, its lie-down and the night skip",
        BED[0],
        BED[1],
        BED[2],
        hows.join(", ")
    );
    let mut bad = Vec::new();
    for how in hows {
        let (asleep, gap) = row(version, how)?;
        if asleep {
            bad.push(format!("{how}: a player ASLEEP at its first tick after the world open"));
        }
        match gap {
            Some(g) if g >= 100 => {}
            Some(g) => bad.push(format!("{how}: the skip {g} gt after the post-load lie-down (< 100)")),
            None => bad.push(format!("{how}: NOT JUDGED (0 samples): no post-load lie-down stamped")),
        }
    }
    println!("  bars (no player asleep at its first tick after a world open; the skip >= 100 gt after the first post-load lie-down): {}", if bad.is_empty() { "HELD".to_string() } else { bad.join("; ") });
    anyhow::ensure!(bad.is_empty(), "player-sleep: {}", bad.join("; "));
    Ok(())
}
