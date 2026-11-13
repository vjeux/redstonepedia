//! (tickexact 2026-11-12, ironspec 22:29 PT (2)) `mctas mobai load-count`: entity, block-entity and
//! block ticks counted by the game itself, on the official 26.3 server RUNNING at 20 TPS (no harness
//! stepping), across a world open and a login with load. In the watched chunk (37,37), every block
//! tick a repeating command-block chain (block ticks: scheduled ticks, before the level's entities
//! and block entities in ServerLevel.tick) reads an item's Age (it counts that item's entity ticks)
//! and a lit furnace's lit_time_remaining (it counts its block-entity ticks), and counts the block
//! ticks where either did not move since the previous block tick (an entity- or block-entity-free
//! tick, counted one tick late), the block ticks themselves, and the game time of the first block
//! tick after the counters were reset. Each case resets the counters, loads the chunk, lets the
//! server run, freezes it and reads them.
//! - world open: the chunk held by a forced ticket (`forceload`, saved with the world: a stand-in
//!   for the farm's own loader, an ender-pearl ticket in the built farm; 26.3 TicketType FORCED
//!   (timeout 0) vs ENDER_PEARL (timeout 40), both full-ticking), the server saved, stopped and
//!   started on its world;
//! - login: the chunk held by nothing (spawn chunks 0, no forced ticket) and unloaded, then the
//!   player logs in on it (its own player tickets load it).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const X: i32 = 600;
const Y: i32 = 80;
const Z: i32 = 600;

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        tick_rate: 20,
        frozen: false,
        keep_dir: true,
        world,
        command_blocks: true,
        ..Default::default()
    }
}

fn score(srv: &mut VanillaServer, who: &str) -> Result<i64> {
    let s = srv.cmd(&format!("scoreboard players get {who} lc"))?;
    // "#efree has 3 [lc]"; (G1 on 6ec22ffd8 #2) an unreadable reply fails, never reads as -1
    s.split(" has ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).ok_or_else(|| anyhow::anyhow!("score {who}: unreadable reply: {}", s.trim()))
}

fn reset(srv: &mut VanillaServer) -> Result<()> {
    for who in ["#efree", "#befree", "#b", "#bfirst", "#fire"] {
        srv.cmd_ok(&format!("scoreboard players set {who} lc 0"))?;
    }
    Ok(())
}

fn loaded(srv: &mut VanillaServer) -> Result<bool> {
    // (G1 on 6ec22ffd8 #8) strict: an error reply fails instead of reading as unloaded
    crate::strict::test(srv, &format!("execute if loaded {X} {Y} {Z}"))
}

/// The counters after the server ran, frozen to read them: (block ticks, entity-free, block-entity-
/// free, the pending repeater's stamp, the window intact: block ticks = game ticks since the first);
/// a case with no block tick is NOT JUDGED.
fn read(srv: &mut VanillaServer, label: &str, reset_gt: i64) -> Result<(i64, i64, i64, i64, bool)> {
    srv.cmd_ok("tick freeze")?;
    let gt = srv.game_time()?;
    let (efree, befree, b, bfirst, fire) = (score(srv, "#efree")?, score(srv, "#befree")?, score(srv, "#b")?, score(srv, "#bfirst")?, score(srv, "#fire")?);
    let intact = bfirst > 0 && b == gt - bfirst + 1;
    println!(
        "  {label}: {}game time {} ticks after the reset; the first block tick at reset + {}; block ticks {b} (= game ticks since it: {}{}); entity-free ticks {efree}; block-entity-free ticks {befree}{}",
        if b == 0 { "NOT JUDGED (0 samples: no block tick): " } else { "" },
        gt - reset_gt,
        if bfirst > 0 { (bfirst - reset_gt).to_string() } else { "-".into() },
        if bfirst > 0 { (gt - bfirst + 1).to_string() } else { "-".into() },
        if bfirst > 0 && !intact { ": the window is NOT intact" } else { "" },
        if fire > 0 { format!("; the pending repeater's command block fired at reset + {} (= the first block tick + {})", fire - reset_gt, fire - bfirst) } else { String::new() }
    );
    Ok((b, efree, befree, fire, intact))
}

pub fn load_count(version: &str, cases: u32, run_ms: u64) -> Result<()> {
    // (G1 on 6ec22ffd8 #3) no 0-sample run
    anyhow::ensure!(cases >= 1, "--cases 0: no sample (NOT JUDGED)");
    println!(
        "== load-count on the official {version} server at 20 TPS: the chunk at ({X},{Y},{Z}); per block tick a command-block chain reads an item's Age and a lit furnace's lit_time_remaining and counts the entity- and block-entity-free ticks (counted one tick late), the block ticks and the first block tick's game time; {cases} world opens (a forced ticket) and {cases} logins (the player's tickets), each read {run_ms} ms after the load"
    );
    let mut srv = VanillaServer::start(&opts(version, None))?;
    let mut not_judged = 0u32;
    let mut bad: Vec<String> = Vec::new();
    srv.cmd_ok("tick freeze")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    for rule in ["spawn_chunk_radius", "spawnChunkRadius"] {
        if srv.cmd(&format!("gamerule {rule} 0"))?.to_ascii_lowercase().contains("set to") {
            break;
        }
    }
    for rule in ["respawn_radius", "spawn_radius"] {
        if srv.cmd(&format!("gamerule {rule} 0"))?.to_ascii_lowercase().contains("set to") {
            break;
        }
    }
    srv.cmd_ok("gamerule command_block_output false").or_else(|_| srv.cmd_ok("gamerule commandBlockOutput false"))?;
    srv.cmd_ok(&format!("forceload add {X} {Z}"))?;
    srv.cmd_ok("scoreboard objectives add lc dummy")?;
    for who in ["#age", "#agep", "#lit", "#litp"] {
        srv.cmd_ok(&format!("scoreboard players set {who} lc 0"))?;
    }
    reset(&mut srv)?;
    srv.step(5)?;
    // a floor; the item (Age from -20000: no despawn for 22 min), the furnace lit for 30000 gt
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:stone", X - 2, Y - 1, Z - 2, X + 10, Y - 1, Z + 4))?;
    srv.cmd_ok(&format!("summon minecraft:item {}.5 {Y} {}.5 {{Item:{{id:\"minecraft:stone\",count:1}},Age:-20000s,PickupDelay:32767s,Tags:[\"lci\"]}}", X, Z + 3))?;
    srv.cmd_ok(&format!("setblock {} {Y} {} minecraft:furnace[lit=true]{{lit_time_remaining:30000,lit_total_time:30000}}", X + 2, Z + 3))?;
    let fx = format!("{} {Y} {}", X + 2, Z + 3);
    // the chain along +x at z: a repeating command block, then chain command blocks (always active)
    let cmds = [
        "execute store result score #age lc run data get entity @e[tag=lci,limit=1] Age".to_string(),
        "execute if score #age lc = #agep lc run scoreboard players add #efree lc 1".to_string(),
        "scoreboard players operation #agep lc = #age lc".to_string(),
        format!("execute store result score #lit lc run data get block {fx} lit_time_remaining"),
        "execute if score #lit lc = #litp lc run scoreboard players add #befree lc 1".to_string(),
        "scoreboard players operation #litp lc = #lit lc".to_string(),
        "scoreboard players add #b lc 1".to_string(),
        "execute if score #bfirst lc matches 0 store result score #bfirst lc run time query gametime".to_string(),
    ];
    for (i, c) in cmds.iter().enumerate() {
        let kind = if i == 0 { "repeating_command_block" } else { "chain_command_block" };
        srv.cmd_ok(&format!("setblock {} {Y} {Z} minecraft:{kind}[facing=east]{{auto:1b,Command:\"{}\"}}", X + i as i32, c.replace('"', "\\\"")))?;
    }
    srv.cmd_ok(&format!("setworldspawn {} {Y} {}", X + 4, Z + 2))?;
    // (ironspec 23:10 (2)) a scheduled tick pending at the save: a repeater (delay 4 = 8 gt) whose
    // input gets a redstone block just before each save, its output into an impulse command block
    // that stamps the game time of its first firing after the reset
    let (rx, rz) = (X + 6, Z + 3);
    srv.cmd_ok(&format!("setblock {rx} {Y} {rz} minecraft:repeater[facing=west,delay=4]"))?;
    srv.cmd_ok(&format!(
        "setblock {} {Y} {rz} minecraft:command_block[facing=east]{{Command:\"execute if score #fire lc matches 0 store result score #fire lc run time query gametime\"}}",
        rx + 1
    ))?;
    let source = format!("{} {Y} {rz}", rx - 1);
    // (logins) a 96-gt line pending across the unload: 12 repeaters (delay 4) from a source at its
    // east end west to an impulse command block stamping #fire2; the chain's last block (below)
    // stamps every block tick's game time into #lastb
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:stone", X + 10, Y - 1, Z + 1, X + 24, Y - 1, Z + 1))?;
    let line_end = X + 23;
    for i in 0..12 {
        srv.cmd_ok(&format!("setblock {} {Y} {} minecraft:repeater[facing=east,delay=4]", line_end - 1 - i, Z + 1))?;
    }
    srv.cmd_ok(&format!(
        "setblock {} {Y} {} minecraft:command_block[facing=west]{{Command:\"execute if score #fire2 lc matches 0 store result score #fire2 lc run time query gametime\"}}",
        line_end - 13,
        Z + 1
    ))?;
    srv.cmd_ok(&format!("setblock {} {Y} {Z} minecraft:chain_command_block[facing=east]{{auto:1b,Command:\"execute store result score #lastb lc run time query gametime\"}}", X + cmds.len() as i32))?;
    let source2 = format!("{line_end} {Y} {}", Z + 1);
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    srv.cmd_ok("tick freeze")?;
    let (age, lit) = (score(&mut srv, "#age")?, score(&mut srv, "#lit")?);
    println!("  built: the chain runs (#age {age}, #lit {lit}, #b {})", score(&mut srv, "#b")?);
    // world opens
    for k in 1..=cases {
        // the repeater settled off (its input cleared, 1 s of ticks), then the counters reset and the
        // repeater's input set: its tick (8 gt) is pending at the save
        // (a setblock to the same state "could not set the block": the reply is not checked)
        srv.cmd(&format!("setblock {source} minecraft:air"))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        srv.cmd_ok("tick freeze")?;
        reset(&mut srv)?;
        let gt0 = srv.game_time()?;
        srv.cmd_ok(&format!("setblock {source} minecraft:redstone_block"))?;
        srv.cmd_ok("save-on")?;
        srv.cmd_ok("save-all flush")?;
        let old = srv.dir.clone();
        let world = old.join(&srv.level_name);
        drop(srv);
        srv = VanillaServer::start(&opts(version, Some(world)))?;
        let _ = std::fs::remove_dir_all(&old);
        std::thread::sleep(std::time::Duration::from_millis(run_ms));
        let (b, efree, befree, fire, intact) = read(&mut srv, &format!("world open {k}"), gt0)?;
        if b == 0 {
            not_judged += 1;
        } else {
            // (G1 on 6ec22ffd8 #2) the measurement itself barred: the window intact, a whole-chunk
            // idle (entity-free = block-entity-free) of 1..=2 ticks (the load model's world open)
            if !intact {
                bad.push(format!("world open {k}: the window is not intact"));
            }
            if efree != befree || !(1..=2).contains(&efree) {
                bad.push(format!("world open {k}: entity-free {efree}, block-entity-free {befree} (the model: equal, 1..=2)"));
            }
        }
        if fire <= 0 {
            not_judged += 1;
            println!("    the pending repeater: NOT JUDGED (0 samples: no stamp)");
        } else if fire - gt0 != 9 {
            bad.push(format!("world open {k}: the pending repeater stamped at save + {} (the model: due at save + 8, stamped at save + 9)", fire - gt0));
        }
    }
    // logins: per case the chunk held (forced) while the 96-gt line is set off, the forced ticket
    // removed, the chunk unloaded, the player logs in on it
    for k in 1..=cases {
        srv.cmd_ok(&format!("forceload add {X} {Z}"))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(1500));
        srv.cmd_ok("tick freeze")?;
        srv.cmd(&format!("setblock {source2} minecraft:air"))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(6000));
        srv.cmd_ok("tick freeze")?;
        srv.cmd_ok("scoreboard players set #fire2 lc 0")?;
        let gt_s = srv.game_time()?;
        srv.cmd_ok(&format!("setblock {source2} minecraft:redstone_block"))?;
        srv.cmd_ok("forceload remove all")?;
        srv.cmd_ok("tick unfreeze")?;
        let t0 = std::time::Instant::now();
        while loaded(&mut srv)? && t0.elapsed() < std::time::Duration::from_secs(30) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::ensure!(!loaded(&mut srv)?, "login {k}: the chunk is still loaded after 30 s");
        std::thread::sleep(std::time::Duration::from_millis(1000));
        srv.cmd_ok("tick freeze")?;
        let (lastb, fire_before) = (score(&mut srv, "#lastb")?, score(&mut srv, "#fire2")?);
        reset(&mut srv)?;
        let gt0 = srv.game_time()?;
        srv.cmd_ok("tick unfreeze")?;
        srv.bot = Some(mctas::vanilla::bot::Bot::connect(srv.game_port(), "mctas_bot")?);
        // (G1 on 6ec22ffd8 #4) the join's own game time, so the first block tick is also stated from
        // the join (the reset's count includes the connection time)
        let gt_join = srv.game_time()?;
        std::thread::sleep(std::time::Duration::from_millis(run_ms.max(7000)));
        let (bl, efree, befree, _, intact) = read(&mut srv, &format!("login {k} (the bot joined at reset + {})", gt_join - gt0), gt0)?;
        if bl == 0 {
            not_judged += 1;
        } else {
            let bf = score(&mut srv, "#bfirst")?;
            println!("    login {k}: the first block tick at join + {}", bf - gt_join);
            if !intact {
                bad.push(format!("login {k}: the window is not intact"));
            }
            if efree != befree || !(7..=18).contains(&efree) {
                bad.push(format!("login {k}: entity-free {efree}, block-entity-free {befree} (the model: equal, 7..=18)"));
            }
        }
        let (b, f2) = (score(&mut srv, "#bfirst")?, score(&mut srv, "#fire2")?);
        let l = lastb - gt_s;
        if fire_before > 0 {
            not_judged += 1;
            println!("    the 96-gt line: NOT JUDGED (0 samples): it finished before the unload (at source + {}), no pending tick across this login", fire_before - gt_s);
        } else if f2 > 0 && b > 0 {
            let total = l + (f2 - b + 1);
            println!(
                "    the 96-gt line (source at {gt_s}): loaded block ticks before the unload {l} (the last at {lastb}); after the login the first block tick at {b}, the line's command block at {f2}: loaded ticks before + after = {total} (96 + 1 if the pending ticks keep their loaded-time spacing; less if re-based earlier, more if later)"
            );
            if total != 97 {
                bad.push(format!("login {k}: the 96-gt line's loaded ticks before + after = {total} (the model: 97)"));
            }
        } else {
            not_judged += 1;
            println!("    the 96-gt line: NOT JUDGED (0 samples): no stamp after the login (#fire2 {f2}, #bfirst {b})");
        }
        srv.bot = None;
    }
    // (G1 on 6ec22ffd8 #2) the zero control: a steady held window (the chunk forced, no load) must
    // count 0 entity-free and 0 block-entity-free ticks through the same chain
    srv.cmd_ok(&format!("forceload add {X} {Z}"))?;
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    srv.cmd_ok("tick freeze")?;
    reset(&mut srv)?;
    let gtz = srv.game_time()?;
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    let (bz, efz, befz, _, intactz) = read(&mut srv, "zero control (a steady held window: no tick may count)", gtz)?;
    anyhow::ensure!(bz > 1 && intactz && efz == 0 && befz == 0, "the zero control FAILED: block ticks {bz}, entity-free {efz}, block-entity-free {befz}, intact {intactz} (each count must be 0)");
    // the positive control: the item's Age frozen (-32768 never advances) and the furnace unlit for
    // 2 s with the chunk held: both counters must count every block tick
    srv.cmd_ok(&format!("forceload add {X} {Z}"))?;
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    srv.cmd_ok("tick freeze")?;
    srv.cmd_ok("data merge entity @e[tag=lci,limit=1] {Age:-32768s}")?;
    srv.cmd_ok(&format!("data merge block {fx} {{lit_time_remaining:0}}"))?;
    reset(&mut srv)?;
    let gt0 = srv.game_time()?;
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    let (b, efree, befree, _, _) = read(&mut srv, "positive control (Age frozen, furnace out: every block tick must count)", gt0)?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    // the control: both counters count every block tick but the first (counted one tick late)
    anyhow::ensure!(b > 1 && efree == b - 1 && befree == b - 1, "the positive control FAILED: block ticks {b}, entity-free {efree}, block-entity-free {befree} (each must be {})", b - 1);
    println!("  bars (each window intact; world opens: idle 1..=2 whole-chunk ticks, the pending repeater at save + 9; logins: idle 7..=18, the 96-gt line 97): {}", if bad.is_empty() { "HELD".to_string() } else { format!("FAILED: {}", bad.join("; ")) });
    // (ironspec 23:56 PT: no vacuous passes)
    anyhow::ensure!(not_judged == 0, "{not_judged} measurement(s) NOT JUDGED (0 samples): the run fails");
    anyhow::ensure!(bad.is_empty(), "{} bar(s) failed: {}", bad.len(), bad.join("; "));
    Ok(())
}

/// (ironspec 13:37 / 13:52 / 14:26, G1 13:50: the farm-chunk k and j) The same counters on the decisive
/// scene's own chunks with the farm's entities loaded: the scene placed at +80 with its entities (24 carts,
/// 24 riders, the scarers), its four chunks (-1..0, -1..0) forced (the loader's stand-in), and one probe
/// item per chunk at y 30 under the farm (an item counts its own entity ticks in its Age; a chunk's
/// entities load together). In EACH chunk a repeating command-block chain runs on that chunk's block ticks
/// and, per probe (every chunk's): counts the block ticks where the probe's Age did not move since the
/// previous block tick or the probe was not loaded (k: those entities behind this chunk's blocks, counted
/// one tick late) and stores its Age at this chunk's first block tick after the reset (j = that Age minus
/// its Age at the save / unload: the entity ticks that ran before this chunk's first block tick). A
/// repeater pending at each save stamps where the first overdue scheduled tick lands. Events: `cases`
/// world opens (save, stop, start on the world) and `cases` logins (the forced tickets removed, the chunks
/// unloaded, the player joins on the farm's top).
pub fn load_count_farm(version: &str, scene: &std::path::Path, cases: u32, run_ms: u64) -> Result<()> {
    anyhow::ensure!(cases >= 1, "--cases 0: no sample (NOT JUDGED)");
    const DY: i32 = 80;
    const CY: i32 = 30;
    let probes: [(i32, i32); 4] = [(-8, -8), (-8, 8), (8, -8), (8, 12)];
    println!(
        "== load-count on the farm's chunks, official {version} at 20 TPS: {} at +{DY} with its entities, chunks (-1..0, -1..0) forced; probe items at y {CY} in each chunk {probes:?}; {cases} world opens and {cases} logins, each read {run_ms} ms after the load",
        scene.display()
    );
    let mut srv = VanillaServer::start(&opts(version, None))?;
    srv.cmd_ok("tick freeze")?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    for rule in ["spawn_chunk_radius", "spawnChunkRadius"] {
        if srv.cmd(&format!("gamerule {rule} 0"))?.to_ascii_lowercase().contains("set to") {
            break;
        }
    }
    for rule in ["respawn_radius", "spawn_radius"] {
        if srv.cmd(&format!("gamerule {rule} 0"))?.to_ascii_lowercase().contains("set to") {
            break;
        }
    }
    srv.cmd_ok("gamerule command_block_output false").or_else(|_| srv.cmd_ok("gamerule commandBlockOutput false"))?;
    srv.cmd_ok("forceload add -16 -16 15 15")?;
    srv.step(5)?;
    crate::seatclear::place_part(&mut srv, scene)?;
    srv.cmd_ok("scoreboard objectives add lc dummy")?;
    for c in 0..probes.len() {
        srv.cmd_ok(&format!("scoreboard players set #z{c} lc 0"))?;
        srv.cmd_ok(&format!("scoreboard players set #b{c} lc 0"))?;
        srv.cmd_ok(&format!("scoreboard players set #s{c} lc 0"))?;
        for p in 0..probes.len() {
            for v in ["a", "q", "e", "f", "n"] {
                srv.cmd_ok(&format!("scoreboard players set #{v}{c}{p} lc 0"))?;
            }
        }
    }
    srv.cmd_ok("scoreboard players set #fire lc 0")?;
    srv.cmd_ok(&format!("fill -16 {} -16 15 {} 15 minecraft:stone", CY - 1, CY - 1))?;
    for (k, (x, z)) in probes.iter().enumerate() {
        srv.cmd_ok(&format!("summon minecraft:item {x}.5 {CY} {z}.5 {{Item:{{id:\"minecraft:stone\",count:1}},Age:-20000s,PickupDelay:32767s,Tags:[\"lcp{k}\"]}}"))?;
    }
    // per chain c (a chunk) and probe p (a chunk): a block tick of c with p absent (its chunk's entities not
    // loaded yet) or with p's Age unmoved counts as entity-free; p's Age at c's first block tick (or its
    // absence there) is stored
    let chunks: Vec<(i32, i32)> = probes.iter().map(|(x, z)| (x.div_euclid(16), z.div_euclid(16))).collect();
    let np = probes.len();
    for c in 0..np {
        let mut cmds: Vec<String> = Vec::new();
        for p in 0..np {
            cmds.push(format!("execute unless entity @e[tag=lcp{p}] run scoreboard players add #e{c}{p} lc 1"));
            cmds.push(format!("execute if score #s{c} lc matches 0 unless entity @e[tag=lcp{p}] run scoreboard players set #n{c}{p} lc 1"));
            cmds.push(format!("execute if entity @e[tag=lcp{p}] store result score #a{c}{p} lc run data get entity @e[tag=lcp{p},limit=1] Age"));
            cmds.push(format!("execute if entity @e[tag=lcp{p}] if score #a{c}{p} lc = #q{c}{p} lc run scoreboard players add #e{c}{p} lc 1"));
            cmds.push(format!("execute if entity @e[tag=lcp{p}] run scoreboard players operation #q{c}{p} lc = #a{c}{p} lc"));
            cmds.push(format!("execute if score #s{c} lc matches 0 if entity @e[tag=lcp{p}] run scoreboard players operation #f{c}{p} lc = #a{c}{p} lc"));
        }
        cmds.push(format!("scoreboard players add #b{c} lc 1"));
        cmds.push(format!("execute if score #s{c} lc matches 0 store result score #s{c} lc run time query gametime"));
        // the chain inside chunk c: east along z oz+1 (x ox..ox+15), then west along z oz+2
        let (ox, oz) = (chunks[c].0 * 16, chunks[c].1 * 16);
        for (i, cmd) in cmds.iter().enumerate() {
            let (dx, dz, facing) = if i < 15 { (i as i32, 1, "east") } else if i == 15 { (15, 1, "south") } else { (15 - (i as i32 - 16), 2, "west") };
            anyhow::ensure!(dx >= 0, "the chain overflows chunk {:?}", chunks[c]);
            let kind = if i == 0 { "repeating_command_block" } else { "chain_command_block" };
            srv.cmd_ok(&format!("setblock {} {CY} {} minecraft:{kind}[facing={facing}]{{auto:1b,Command:\"{}\"}}", ox + dx, oz + dz, cmd.replace('"', "\\\"")))?;
        }
    }
    // a repeater (delay 4 = 8 gt) whose input gets a redstone block just before each save, into an impulse
    // command block stamping its first firing after the reset (where the first overdue tick lands)
    srv.cmd_ok(&format!("setblock 6 {CY} 6 minecraft:repeater[facing=west,delay=4]"))?;
    srv.cmd_ok(&format!("setblock 7 {CY} 6 minecraft:command_block[facing=east]{{Command:\"execute if score #fire lc matches 0 store result score #fire lc run time query gametime\"}}"))?;
    let source = format!("5 {CY} 6");
    // (ironspec 14:53) two ticks due INSIDE the idle, in this same chunk: an impulse command block powered
    // with the source (its own tick due at save + 1, stamping #c1) and a repeater (delay 1: its tick due at
    // save + 2) whose output powers a second impulse command block (stamping #c2 one tick after the repeater)
    srv.cmd_ok(&format!("setblock 6 {CY} 9 minecraft:command_block[facing=east]{{Command:\"execute if score #c1 lc matches 0 store result score #c1 lc run time query gametime\"}}"))?;
    srv.cmd_ok(&format!("setblock 5 {CY} 10 minecraft:repeater[facing=north,delay=1]"))?;
    srv.cmd_ok(&format!("setblock 5 {CY} 11 minecraft:command_block[facing=east]{{Command:\"execute if score #c2 lc matches 0 store result score #c2 lc run time query gametime\"}}"))?;
    let source2 = format!("5 {CY} 9");
    for p in ["#c1", "#c2"] {
        srv.cmd_ok(&format!("scoreboard players set {p} lc 0"))?;
    }
    // the login spawn: a glass pad on the farm's top
    srv.cmd_ok(&format!("setblock 4 {} 4 minecraft:glass", DY + 30))?;
    srv.cmd_ok(&format!("setworldspawn 4 {} 4", DY + 31))?;
    srv.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    srv.cmd_ok("tick freeze")?;
    println!(
        "  built: the chains run (block ticks per chunk {:?}, probe ages read in chunk (0,0)'s chain {:?})",
        (0..np).map(|c| score(&mut srv, &format!("#b{c}"))).collect::<Result<Vec<_>>>()?,
        (0..np).map(|p| score(&mut srv, &format!("#a{}{p}", np - 1))).collect::<Result<Vec<_>>>()?
    );
    let mut not_judged = 0u32;
    let reset_counts = |srv: &mut VanillaServer| -> Result<()> {
        for p in 0..np {
            srv.cmd_ok(&format!("execute store result score #z{p} lc run data get entity @e[tag=lcp{p},limit=1] Age"))?;
        }
        for c in 0..np {
            srv.cmd_ok(&format!("scoreboard players set #b{c} lc 0"))?;
            srv.cmd_ok(&format!("scoreboard players set #s{c} lc 0"))?;
            for p in 0..np {
                srv.cmd_ok(&format!("scoreboard players set #e{c}{p} lc 0"))?;
                srv.cmd_ok(&format!("scoreboard players set #f{c}{p} lc 0"))?;
                srv.cmd_ok(&format!("scoreboard players set #n{c}{p} lc 0"))?;
                srv.cmd_ok(&format!("scoreboard players operation #q{c}{p} lc = #z{p} lc"))?;
            }
        }
        srv.cmd_ok("scoreboard players set #fire lc 0")?;
        srv.cmd_ok("scoreboard players set #c1 lc 0")?;
        srv.cmd_ok("scoreboard players set #c2 lc 0")?;
        Ok(())
    };
    // per event: (same-chunk worst k, j; cross-chunk worst k, j; entity-vs-entity skew; the chunks' first
    // block ticks' spread)
    let mut read_counts = |srv: &mut VanillaServer, label: &str, gt0: i64| -> Result<[i64; 6]> {
        srv.cmd_ok("tick freeze")?;
        let fire = score(srv, "#fire")?;
        let (c1, c2) = (score(srv, "#c1")?, score(srv, "#c2")?);
        let mut w = [0i64; 6];
        let mut lines: Vec<String> = Vec::new();
        let mut firsts: Vec<i64> = Vec::new();
        for c in 0..np {
            let (b, s) = (score(srv, &format!("#b{c}"))?, score(srv, &format!("#s{c}"))?);
            if b == 0 || s <= 0 {
                not_judged += 1;
                lines.push(format!("blocks of {:?}: NO block tick (NOT JUDGED)", chunks[c]));
                continue;
            }
            firsts.push(s);
            let mut per: Vec<String> = Vec::new();
            let mut offs: Vec<i64> = Vec::new();
            for p in 0..np {
                let (e, f, z, n) = (score(srv, &format!("#e{c}{p}"))?, score(srv, &format!("#f{c}{p}"))?, score(srv, &format!("#z{p}"))?, score(srv, &format!("#n{c}{p}"))?);
                // absent at c's first block tick: p's entities had not run before c's blocks (j 0; k counts it)
                let j = if n == 1 { 0 } else { f - z };
                let (ik, ij) = if c == p { (0, 1) } else { (2, 3) };
                w[ik] = w[ik].max(e);
                w[ij] = w[ij].max(j);
                offs.push(j - e);
                per.push(format!("{:?} k{e} j{j}{}", chunks[p], if n == 1 { " (absent)" } else { "" }));
            }
            let skew = offs.iter().max().unwrap_or(&0) - offs.iter().min().unwrap_or(&0);
            w[4] = w[4].max(skew);
            lines.push(format!("blocks of {:?} (first at reset + {}): {}", chunks[c], s - gt0, per.join(", ")));
        }
        let spread = firsts.iter().max().unwrap_or(&0) - firsts.iter().min().unwrap_or(&0);
        w[5] = w[5].max(spread);
        println!(
            "  {label}: the pending repeater {}; the in-idle pair (same chunk): {}; the chunks' first block ticks within {spread} gt\n    {}",
            if fire > 0 { format!("stamped at save + {} (due at save + 8)", fire - gt0) } else { "-".into() },
            if c1 > 0 && c2 > 0 {
                format!(
                    "the command block due at save + 1 ran at save + {}, the repeater due at save + 2 at save + {} (its command block at save + {}): {}",
                    c1 - gt0,
                    c2 - 1 - gt0,
                    c2 - gt0,
                    if c1 == c2 - 1 { "ONE tick" } else { "different ticks" }
                )
            } else {
                "-".into()
            },
            lines.join("\n    ")
        );
        Ok(w)
    };
    let fold = |acc: &mut [i64; 6], w: [i64; 6]| {
        for i in 0..6 {
            acc[i] = acc[i].max(w[i]);
        }
    };
    let mut opens = [0i64; 6];
    for c in 1..=cases {
        srv.cmd(&format!("setblock {source} minecraft:air"))?;
        srv.cmd(&format!("setblock {source2} minecraft:air"))?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(1000));
        srv.cmd_ok("tick freeze")?;
        reset_counts(&mut srv)?;
        let gt0 = srv.game_time()?;
        srv.cmd_ok(&format!("setblock {source} minecraft:redstone_block"))?;
        srv.cmd_ok(&format!("setblock {source2} minecraft:redstone_block"))?;
        srv.cmd_ok("save-on")?;
        srv.cmd_ok("save-all flush")?;
        let old = srv.dir.clone();
        let world = old.join(&srv.level_name);
        drop(srv);
        srv = VanillaServer::start(&opts(version, Some(world)))?;
        let _ = std::fs::remove_dir_all(&old);
        std::thread::sleep(std::time::Duration::from_millis(run_ms));
        let w = read_counts(&mut srv, &format!("world open {c}"), gt0)?;
        fold(&mut opens, w);
    }
    let mut logins = [0i64; 6];
    for c in 1..=cases {
        srv.cmd(&format!("setblock {source} minecraft:air"))?;
        srv.cmd_ok("forceload add -16 -16 15 15")?;
        srv.cmd_ok("tick unfreeze")?;
        std::thread::sleep(std::time::Duration::from_millis(1500));
        srv.cmd_ok("tick freeze")?;
        reset_counts(&mut srv)?;
        let gt0 = srv.game_time()?;
        srv.cmd_ok("forceload remove all")?;
        srv.cmd_ok("tick unfreeze")?;
        let t0 = std::time::Instant::now();
        let any_loaded = |srv: &mut VanillaServer| -> Result<bool> {
            for (cx, cz) in &chunks {
                if crate::strict::test(srv, &format!("execute if loaded {} {CY} {}", cx * 16 + 1, cz * 16 + 1))? {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        while any_loaded(&mut srv)? && t0.elapsed() < std::time::Duration::from_secs(30) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::ensure!(!any_loaded(&mut srv)?, "login {c}: a farm chunk is still loaded after 30 s");
        std::thread::sleep(std::time::Duration::from_millis(1000));
        // the ages at the unload: the probes are not loaded now, so the reset's #z (read while held) stands
        srv.bot = Some(mctas::vanilla::bot::Bot::connect(srv.game_port(), "mctas_bot")?);
        std::thread::sleep(std::time::Duration::from_millis(run_ms.max(7000)));
        let w = read_counts(&mut srv, &format!("login {c}"), gt0)?;
        fold(&mut logins, w);
        srv.bot = None;
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let say = |w: &[i64; 6]| format!("same chunk worst k {} j {}; cross-chunk (one chunk's blocks vs another's entities) worst k {} j {}; entity-vs-entity skew across chunks {}; the chunks' first block ticks within {} gt", w[0], w[1], w[2], w[3], w[4], w[5]);
    println!("  summary (samples {cases} per kind; a measurement): world opens: {}; logins: {}", say(&opens), say(&logins));
    anyhow::ensure!(not_judged == 0, "{not_judged} chunk-event(s) with no block tick: NOT JUDGED, the run fails");
    Ok(())
}
