//! (tickexact 2026-11-14, ironspec 05:03 (9)(ii) / G1 05:05 (1)(a): droppers and quasi-connectivity in the game)
//! `mctas mobai game-drive <design> <drive> [--cart]`: a gadget drive script (designs/gadgets/*.drive, crates/piston-door
//! drive.rs) replayed on the official 26.3 server: the design pasted at local + (10, 70, 10) with its `nbt:` lines, then
//! the script's commands in order: `set` / `clear` as the game's /setblock, `quiet n` as n game ticks, `probe_items` (the
//! container's item count), `probe` (a block's powered state: 1 / 0), `expect` and `expect_trace` checked against the
//! game. With `--cart` an ideal press of a detector rail (`set` of a redstone block into a cell that holds a detector rail)
//! is a REAL minecart summoned at rest on that rail, and the release (`set` of the detector rail back) removes it.
//! Every tick: every probe, and each dropper's `triggered` state. Exit 1 on any failed check (named by its script line).

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::BTreeMap;

const O: [i32; 3] = [10, 70, 10];

fn w(p: [i32; 3]) -> [i32; 3] {
    [p[0] + O[0], p[1] + O[1], p[2] + O[2]]
}

fn items(srv: &mut VanillaServer, p: [i32; 3]) -> Result<u32> {
    let q = w(p);
    let out = srv.cmd_long(&format!("data get block {} {} {} Items", q[0], q[1], q[2]))?;
    Ok(out.split("count:").skip(1).filter_map(|s| s.trim_start().split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse::<u32>().ok())).sum())
}

fn powered(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<bool> {
    let name = state.split('[').next().unwrap_or(state);
    srv.test_block(w(p), &format!("{name}[powered=true]"))
}

pub fn game_drive(version: &str, design: &std::path::Path, drive: &std::path::Path, cart: bool) -> Result<()> {
    let d = mctas::design::Design::load(design)?;
    let blocks: BTreeMap<[i32; 3], String> = d.block_list().into_iter().collect();
    let script = std::fs::read_to_string(drive).with_context(|| format!("read {}", drive.display()))?;
    println!("== game-drive on the official {version} server: {} ({} cells) at local + {O:?}, the script {}{}", design.display(), blocks.len(), drive.display(), if cart { "; ideal detector presses replaced by a REAL minecart at rest on the rail" } else { "" });
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add 0 0 31 31")?;
    srv.step(20)?;
    srv.cmd(&format!("fill {} {} {} {} {} {} minecraft:air", O[0] - 4, O[1] - 2, O[2] - 4, O[0] + 12, O[1] + 8, O[2] + 12))?;
    // the rest state as the design writes it (the portal paste: every cell placed, then the `nbt:` lines), checked cell by cell
    let world_blocks: Vec<([i32; 3], String)> = blocks.iter().map(|(p, s)| (w(*p), s.clone())).collect();
    crate::portal::paste(&mut srv, "minecraft:overworld", &world_blocks)?;
    for line in script.lines().chain(std::fs::read_to_string(design)?.lines()) {
        if let Some(rest) = line.trim().strip_prefix("nbt: ") {
            let mut it = rest.splitn(4, ' ');
            let (x, y, z, nbt) = (it.next(), it.next(), it.next(), it.next());
            if let (Some(x), Some(y), Some(z), Some(nbt)) = (x, y, z, nbt) {
                let p = [x.parse::<i32>()?, y.parse::<i32>()?, z.parse::<i32>()?];
                let q = w(p);
                srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", q[0], q[1], q[2]))?;
            }
        }
    }
    srv.step(2)?;
    for (p, s) in &blocks {
        anyhow::ensure!(srv.test_block(w(*p), s)?, "the rest state differs from the design at local {p:?} ({s})");
    }
    println!("  the rest state matches the design in all {} cells", blocks.len());
    let droppers: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:dropper") || s.starts_with("minecraft:dispenser")).map(|(p, _)| *p).collect();
    let mut probes: Vec<(String, [i32; 3], bool)> = Vec::new();
    let (mut tick, mut checks, mut failures): (u32, u32, Vec<String>) = (0, 0, Vec::new());
    let mut cart_n = 0u32;
    let mut cart_at: BTreeMap<[i32; 3], String> = BTreeMap::new();
    let mut line_of = |srv: &mut VanillaServer, probes: &[(String, [i32; 3], bool)], tick: u32| -> Result<String> {
        let mut s = format!("    t{tick:>3}:");
        for (name, p, is_items) in probes {
            let v = if *is_items { items(srv, *p)? } else { u32::from(powered(srv, *p, blocks.get(p).map(String::as_str).unwrap_or("minecraft:stone"))?) };
            s.push_str(&format!(" {name}={v}"));
        }
        for p in &droppers {
            let trig = srv.test_block(w(*p), &format!("{}[triggered=true]", blocks[p].split('[').next().unwrap_or("minecraft:dropper")))?;
            s.push_str(&format!(" | ({},{},{}) {}", p[0], p[1], p[2], if trig { "TRIGGERED" } else { "-" }));
        }
        Ok(s)
    };
    let value = |srv: &mut VanillaServer, probes: &[(String, [i32; 3], bool)], name: &str| -> Result<u32> {
        let (_, p, is_items) = probes.iter().find(|(n, _, _)| n == name).with_context(|| format!("no probe {name}"))?.clone();
        Ok(if is_items { items(srv, p)? } else { u32::from(powered(srv, p, blocks.get(&p).map(String::as_str).unwrap_or("minecraft:stone"))?) })
    };
    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> { parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into) };
        match parts[0] {
            "design" | "nbt:" => {}
            "probe_items" | "probe" | "probe_signal" => probes.push((parts[1].to_string(), [int(2)?, int(3)?, int(4)?], parts[0] == "probe_items")),
            "set" | "clear" => {
                let p = [int(1)?, int(2)?, int(3)?];
                let state = if parts[0] == "clear" { "minecraft:air".to_string() } else { parts.get(4).context("set needs a state")?.to_string() };
                let q = w(p);
                let on_detector = blocks.get(&p).is_some_and(|s| s.starts_with("minecraft:detector_rail"));
                if cart && on_detector && state == "minecraft:redstone_block" {
                    cart_n += 1;
                    let tag = format!("gd{cart_n}");
                    srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {}.0625 {}.5 {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,0.0d]}}", q[0], q[1], q[2]))?;
                    cart_at.insert(p, tag);
                    println!("  (line {}) t{tick}: PRESS: a minecart at rest on the detector rail ({},{},{})", lineno + 1, p[0], p[1], p[2]);
                } else if cart && on_detector && state.starts_with("minecraft:detector_rail") {
                    if let Some(tag) = cart_at.remove(&p) {
                        srv.cmd(&format!("execute as @e[tag={tag}] run tp @s {} {} {}", q[0], q[1] - 40, q[2]))?;
                        srv.cmd(&format!("kill @e[tag={tag}]"))?;
                        srv.cmd("kill @e[type=minecraft:item]")?;
                    }
                    println!("  (line {}) t{tick}: RELEASE: the cart removed (the detector rail re-checks on its own)", lineno + 1);
                } else {
                    let out = srv.cmd(&format!("setblock {} {} {} {state}", q[0], q[1], q[2]))?;
                    println!("  (line {}) t{tick}: setblock ({},{},{}) {state}{}", lineno + 1, p[0], p[1], p[2], if out.contains("Could not") { " (unchanged)" } else { "" });
                }
            }
            "quiet" | "step" => {
                for _ in 0..int(1)? {
                    srv.step(1)?;
                    tick += 1;
                    let s = line_of(&mut srv, &probes, tick)?;
                    println!("{s}");
                }
            }
            "expect" => {
                let (name, want) = (parts[1], parts[2]);
                let got = value(&mut srv, &probes, name)?;
                checks += 1;
                let ok = got.to_string() == want;
                println!("  (line {}) t{tick}: expect {name}={want}: got {got} {}", lineno + 1, if ok { "ok" } else { "FAIL" });
                if !ok {
                    failures.push(format!("line {}: t{tick} expect {name}={want}, got {got}", lineno + 1));
                }
            }
            "expect_trace" => {
                let (name, want) = (parts[1], parts[2]);
                let mut got = String::new();
                for _ in want.chars() {
                    srv.step(1)?;
                    tick += 1;
                    let v = value(&mut srv, &probes, name)?;
                    got.push(if v > 1 { '+' } else { char::from_digit(v, 10).unwrap_or('?') });
                    let s = line_of(&mut srv, &probes, tick)?;
                    println!("{s}");
                }
                checks += 1;
                let ok = want.chars().zip(got.chars()).all(|(a, b)| a == '?' || a == b);
                println!("  (line {}) t{tick}: expect_trace {name} {want}: got {got} {}", lineno + 1, if ok { "ok" } else { "FAIL" });
                if !ok {
                    failures.push(format!("line {}: t{tick} expect_trace {name} {want}, got {got}", lineno + 1));
                }
            }
            other => anyhow::bail!("line {}: script command {other} is not supported in the game replay", lineno + 1),
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    println!("  {checks} checks, {} failed{}", failures.len(), if failures.is_empty() { String::new() } else { format!(": {failures:?}") });
    anyhow::ensure!(failures.is_empty(), "{} of {checks} checks failed in the game: {failures:?}", failures.len());
    Ok(())
}
