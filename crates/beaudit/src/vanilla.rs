//! Scenario scripts on the UNMODIFIED official server (`mctas beaudit vanilla <file>...`).
//! One fresh server per file, time frozen, ticks only via `step`.
//!
//! Script lines (blank lines and `#` comments ignored):
//!
//! ```text
//! cmd <command>                  RCON command; fails the run on an error reply
//! raw <command>                  RCON command; prints the reply, never fails
//! tp <x> <y> <z> [<yaw> <pitch>] teleport the bot (feet), floats allowed
//! hand <item...>                 put an item in the bot's main hand (`hand -` empties it)
//! offhand <item...>              same, off hand (`offhand -` empties it)
//! sneak on|off                   bot sneaking flag
//! gamemode <mode>                the bot's gamemode (creative on join; survival is put on the ground)
//! use <x> <y> <z> <face> [hand]  right-click the block (face: down|up|north|south|west|east; hand: main|off)
//! break <x> <y> <z> <face>       start destroying the block (instant in creative)
//! step <n>                       advance exactly n game ticks
//! mark <label...>                print a label with the game time
//! print-block <x> <y> <z>        print the saved block state there (never fails)
//! print-data <x> <y> <z>         print `data get block` there (never fails)
//! restart                        save, stop the server, start a fresh one on a copy of the world
//! expect-block <x> <y> <z> <text>   saved block state there contains <text> (region readback)
//! reject-block <x> <y> <z> <text>   ... does not
//! expect-data <x> <y> <z> <text>    `data get block` output contains <text>
//! reject-data <x> <y> <z> <text>    ... does not (also true when there is no block entity)
//! expect-no-be <x> <y> <z>          the cell has no block entity
//! expect-cmd <text> :: <command>    the command's output contains <text>
//! ```
//!
//! Every `expect-*`/`reject-*` is one check; the run prints one PASS/FAIL line per file with
//! the failing lines spelled out, and exits nonzero if any check failed.

use anyhow::{Context, Result, bail};
use mctas::sim::Pos;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn face_id(name: &str) -> Result<u8> {
    Ok(match name {
        "down" => 0,
        "up" => 1,
        "north" => 2,
        "south" => 3,
        "west" => 4,
        "east" => 5,
        other => bail!("unknown face `{other}` (down|up|north|south|west|east)"),
    })
}

struct Run {
    srv: VanillaServer,
    checks: u32,
    failures: Vec<String>,
    world_for_restart: PathBuf,
}

impl Run {
    fn check(&mut self, lineno: usize, ok: bool, what: String) {
        self.checks += 1;
        if !ok {
            self.failures.push(format!("line {}: {what}", lineno + 1));
        }
    }

    fn block_state(&mut self, p: Pos) -> Result<String> {
        // Right after a (re)start the chunk may not be loaded yet and the snapshot's marker
        // setblock answers "Could not set the block": give the chunk tickets a few ticks.
        let mut last: Option<anyhow::Error> = None;
        for _ in 0..8 {
            match self.srv.snapshot(p, p) {
                Ok(snap) => return Ok(snap.region.get(p).unwrap_or("minecraft:air").to_string()),
                Err(e) => {
                    last = Some(e);
                    std::thread::sleep(Duration::from_millis(300));
                    let _ = self.srv.step(5);
                }
            }
        }
        Err(last.unwrap())
    }

    fn block_data(&mut self, p: Pos) -> Result<String> {
        Ok(self.srv.cmd(&format!("data get block {} {} {}", p[0], p[1], p[2]))?)
    }
}

fn start_server(version: &str, heap_mb: u32, world: Option<PathBuf>) -> Result<VanillaServer> {
    let opts = VanillaOptions { version: version.to_string(), heap_mb, keep_dir: true, world, ..VanillaOptions::default() };
    let mut srv = VanillaServer::start(&opts)?;
    // Park the bot above the origin and let the surrounding chunks arrive.
    srv.teleport_bot(0.5, 8.0, 0.5)?;
    srv.step(2)?;
    Ok(srv)
}

fn run_one(version: &str, heap_mb: u32, path: &Path, keep: bool) -> Result<(u32, Vec<String>)> {
    let script = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut run = Run { srv: start_server(version, heap_mb, None)?, checks: 0, failures: Vec::new(), world_for_restart: PathBuf::new() };
    run.world_for_restart = run.srv.dir.join(&run.srv.level_name);
    let first_dir = run.srv.dir.clone();
    let bot = run.srv.bot.as_ref().context("no bot")?.name.clone();

    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut split = line.splitn(2, char::is_whitespace);
        let verb = split.next().unwrap_or("");
        let rest = split.next().unwrap_or("").trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> {
            parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into)
        };
        let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        match verb {
            "cmd" => {
                run.srv.cmd_ok(rest)?;
            }
            "raw" => {
                let out = run.srv.cmd(rest)?;
                println!("  {rest} -> {}", out.trim());
            }
            "mark" => {
                println!("== {} (t={})", rest, run.srv.game_time()?);
            }
            "print-block" => {
                let p = pos(1)?;
                println!("  {p:?} = {}", run.block_state(p)?);
            }
            "print-data" => {
                let p = pos(1)?;
                println!("  {p:?} data: {}", run.block_data(p)?.trim());
            }
            "tp" => {
                let f = |i: usize| -> Result<f64> {
                    parts.get(i).with_context(|| format!("line {}: tp needs x y z [yaw pitch]", lineno + 1))?.parse::<f64>().map_err(Into::into)
                };
                let look = if parts.len() >= 6 { Some((f(4)? as f32, f(5)? as f32)) } else { None };
                run.srv.teleport_bot_to(f(1)?, f(2)?, f(3)?, look)?;
            }
            "hand" | "offhand" => {
                let slot = if verb == "hand" { "weapon.mainhand" } else { "weapon.offhand" };
                let item = if rest == "-" || rest.is_empty() { "minecraft:air" } else { rest };
                run.srv.cmd_ok(&format!("item replace entity {bot} {slot} with {item}"))?;
            }
            "sneak" => {
                run.srv.bot.as_mut().context("no bot")?.set_sneaking(rest == "on")?;
            }
            "gamemode" => {
                run.srv.cmd_ok(&format!("gamemode {rest} {bot}"))?;
                if rest == "survival" {
                    run.srv.bot.as_mut().context("no bot")?.set_on_ground(true)?;
                }
            }
            "use" => {
                let face = face_id(parts.get(4).context("use needs a face")?)?;
                let hand = match parts.get(5).copied().unwrap_or("main") {
                    "main" => 0,
                    "off" => 1,
                    other => bail!("line {}: unknown hand `{other}`", lineno + 1),
                };
                let seq = run.srv.bot.as_mut().context("no bot")?.use_item_on_hand(hand, pos(1)?, face, [0.5, 0.5, 0.5])?;
                run.srv.bot.as_ref().unwrap().wait_ack(seq, Duration::from_secs(10))?;
            }
            "break" => {
                let face = face_id(parts.get(4).context("break needs a face")?)?;
                let seq = run.srv.bot.as_mut().context("no bot")?.start_destroy(pos(1)?, face)?;
                run.srv.bot.as_ref().unwrap().wait_ack(seq, Duration::from_secs(10))?;
            }
            "step" => {
                run.srv.step(int(1)? as u32)?;
            }
            "restart" => {
                run.srv.cmd_ok("save-all flush")?;
                let world = run.world_for_restart.clone();
                let _ = run.srv.cmd("stop");
                std::thread::sleep(Duration::from_millis(500));
                let old = std::mem::replace(&mut run.srv, start_server(version, heap_mb, Some(world))?);
                drop(old);
                run.world_for_restart = run.srv.dir.join(&run.srv.level_name);
                println!("== restarted on the saved world (t={})", run.srv.game_time()?);
            }
            "expect-block" | "reject-block" => {
                let p = pos(1)?;
                let want = line.splitn(5, char::is_whitespace).nth(4).context("expect-block needs x y z and text")?.trim();
                let got = run.block_state(p)?;
                let t = run.srv.game_time()?;
                let has = got.contains(want);
                let ok = if verb == "expect-block" { has } else { !has };
                run.check(lineno, ok, format!("t={t} {verb} {p:?} containing `{want}`, got {got}"));
            }
            "expect-data" | "reject-data" => {
                let p = pos(1)?;
                let want = line.splitn(5, char::is_whitespace).nth(4).context("expect-data needs x y z and text")?.trim();
                let got = run.block_data(p)?;
                let t = run.srv.game_time()?;
                let has = got.contains(want);
                let ok = if verb == "expect-data" { has } else { !has };
                run.check(lineno, ok, format!("t={t} {verb} {p:?} containing `{want}`, got {}", got.trim()));
            }
            "expect-no-be" => {
                let p = pos(1)?;
                let got = run.block_data(p)?;
                let t = run.srv.game_time()?;
                let ok = got.contains("not a block entity") || got.contains("is not a block entity");
                run.check(lineno, ok, format!("t={t} expect-no-be {p:?}, data get said: {}", got.trim()));
            }
            "expect-cmd" => {
                let (want, command) = rest.split_once(" :: ").context("expect-cmd is `expect-cmd <text> :: <command>`")?;
                let got = run.srv.cmd(command.trim())?;
                run.check(lineno, got.contains(want.trim()), format!("expect-cmd `{}` containing `{}`, got {}", command.trim(), want.trim(), got.trim()));
            }
            other => bail!("line {}: unknown verb `{other}` (see beaudit::vanilla)", lineno + 1),
        }
    }

    let (checks, failures) = (run.checks, std::mem::take(&mut run.failures));
    let last_dir = run.srv.dir.clone();
    drop(run);
    if !keep {
        let _ = std::fs::remove_dir_all(&first_dir);
        if last_dir != first_dir {
            let _ = std::fs::remove_dir_all(&last_dir);
        }
    }
    Ok((checks, failures))
}

pub fn run_scenarios(version: &str, scenarios: &[PathBuf], keep: bool, heap_mb: u32) -> Result<()> {
    if scenarios.is_empty() {
        bail!("no scenario files given");
    }
    let mut failed = 0;
    for path in scenarios {
        match run_one(version, heap_mb, path, keep) {
            Ok((checks, failures)) if failures.is_empty() => println!("PASS {} ({checks} checks)", path.display()),
            Ok((checks, failures)) => {
                failed += 1;
                println!("FAIL {} ({} of {checks} checks failed)", path.display(), failures.len());
                for f in failures {
                    println!("  {f}");
                }
            }
            Err(e) => {
                failed += 1;
                println!("FAIL {} ({e:#})", path.display());
            }
        }
    }
    if failed > 0 {
        bail!("{failed} of {} scenarios failed", scenarios.len());
    }
    Ok(())
}
