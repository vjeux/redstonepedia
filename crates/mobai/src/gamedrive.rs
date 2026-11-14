//! (tickexact 2026-11-14, ironspec 05:03 (9)(ii) / G1 05:05 (1)(a): droppers and quasi-connectivity in the game)
//! `mctas mobai game-drive <design> <drive> [--cart]`: a gadget drive script (designs/gadgets/*.drive, crates/piston-door
//! drive.rs) replayed on the official 26.3 server: the design pasted at local + (10, 70, 10) with its `nbt:` lines (each
//! read back), then the script's commands in order: `set` / `clear` as the game's /setblock, `quiet n` / `step n` as n game
//! ticks, `probe_items` (the container's item count), `probe` (drive.rs's probe value where it is a flag: `powered` for
//! repeaters, comparators, observers, levers, buttons, rails, doors, trapdoors, note blocks, gates, heads and shelves;
//! `lit` for torches, lamps and copper bulbs; `triggered` for dispensers, droppers and crafters), `expect` and
//! `expect_trace` checked against the game (no `?` wildcard: every tick's value is exact). With `--cart` a press of a
//! detector rail (`set` of a redstone block into a cell that holds a detector rail) is a REAL minecart summoned at rest on
//! that rail, and its release (`set` of the rail's own rest state back) removes that cart.
//! (G1 06:54 (2)/(3)) Refused before the server starts, by line: `probe_signal` (drive.rs reads the best neighbour signal,
//! which the game's commands can't read), a `probe` of a block whose probe value isn't a flag (dust power, pistons …),
//! a `design` line naming another file or an offset, a `--cart` press of a rail already pressed, a release with no cart, a
//! `set` of a detector rail to any state but its rest state, a script with no check, and `--cart` with no press.
//! Every tick: every probe, and each dropper's `triggered` state. Exit 1 on any failed check (named by its script line).

use anyhow::{bail, Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::collections::{BTreeMap, BTreeSet};

const O: [i32; 3] = [10, 70, 10];

fn w(p: [i32; 3]) -> [i32; 3] {
    [p[0] + O[0], p[1] + O[1], p[2] + O[2]]
}

fn items(srv: &mut VanillaServer, p: [i32; 3]) -> Result<u32> {
    let q = w(p);
    let out = srv.cmd_long(&format!("data get block {} {} {} Items", q[0], q[1], q[2]))?;
    Ok(out.split("count:").skip(1).filter_map(|s| s.trim_start().split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse::<u32>().ok())).sum())
}

/// drive.rs's probe value of a block kind, where it is a flag: the property it reads (`probe_value`)
pub fn probe_flag(state: &str) -> Option<&'static str> {
    let id = state.split('[').next().unwrap_or(state);
    let n = id.strip_prefix("minecraft:").unwrap_or(id);
    match n {
        "redstone_torch" | "redstone_wall_torch" | "redstone_lamp" => Some("lit"),
        n if n.ends_with("copper_bulb") => Some("lit"),
        "repeater" | "comparator" | "observer" | "lever" | "stone_button" | "oak_button" => Some("powered"),
        n if n.ends_with("_rail") || n.ends_with("trapdoor") || n.ends_with("door") || n == "note_block" => Some("powered"),
        n if n.ends_with("fence_gate") || n.ends_with("_skull") || n.ends_with("_head") || n.ends_with("_shelf") => Some("powered"),
        "dispenser" | "dropper" | "crafter" => Some("triggered"),
        _ => None,
    }
}

/// The script checked before the server starts (G1 06:54 (2)/(3)): every command known, every probe readable in the
/// game, a `design` line only naming the design given (no offset), the cart presses paired, at least one check. Returns
/// the number of checks and of cart presses.
pub fn check_script(script: &str, blocks: &BTreeMap<[i32; 3], String>, design: &std::path::Path, cart: bool) -> Result<(u32, u32)> {
    let design_bytes = std::fs::read(design).with_context(|| format!("read {}", design.display()))?;
    let design_md5 = crate::md5::md5_hex(&design_bytes);
    let mut checks = 0u32;
    let mut presses = 0u32;
    let mut pressed: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut probes: BTreeSet<String> = BTreeSet::new();
    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let n = lineno + 1;
        let arg = |i: usize| -> Result<&str> { parts.get(i).copied().with_context(|| format!("line {n}: `{line}` needs argument {i}")) };
        let int = |i: usize| -> Result<i32> { arg(i)?.parse::<i32>().with_context(|| format!("line {n}: `{line}`: argument {i} is not a number")) };
        match parts[0] {
            "design" => {
                let named = std::path::Path::new(arg(1)?);
                anyhow::ensure!(parts.len() == 2, "line {n}: `{line}`: a design line with an offset is refused (the design is pasted at the command's own place)");
                let same = if named.exists() {
                    std::fs::read(named).with_context(|| format!("line {n}: read {}", named.display()))? == design_bytes
                } else {
                    // a vendored copy `<stem>-<12 hex md5>.<ext>` stands for `<stem>.<ext>` when its md5 is the design's
                    let stem = named.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let given = design.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    given.strip_prefix(stem).and_then(|t| t.strip_prefix('-')).is_some_and(|h| h.len() == 12 && design_md5.starts_with(h))
                };
                anyhow::ensure!(same, "line {n}: `{line}` names a different design from {} (md5 {})", design.display(), &design_md5[..12]);
            }
            "nbt:" => bail!("line {n}: `{line}`: an `nbt:` line in a drive script is refused (the block entities come from the design's own `nbt:` lines, which are pasted and read back)"),
            "probe_signal" => bail!("line {n}: `{line}`: probe_signal reads the best neighbour signal in drive.rs, which the game's commands can't read: refused"),
            "probe_items" => {
                let p = [int(2)?, int(3)?, int(4)?];
                let s = blocks.get(&p).with_context(|| format!("line {n}: `{line}`: no block at local {p:?} in the design"))?;
                anyhow::ensure!(s.contains("dropper") || s.contains("dispenser") || s.contains("hopper") || s.contains("chest") || s.contains("barrel") || s.contains("crafter"), "line {n}: `{line}`: {s} holds no items");
                probes.insert(arg(1)?.to_string());
            }
            "probe" => {
                let p = [int(2)?, int(3)?, int(4)?];
                let s = blocks.get(&p).with_context(|| format!("line {n}: `{line}`: no block at local {p:?} in the design"))?;
                anyhow::ensure!(probe_flag(s).is_some(), "line {n}: `{line}`: the probe value of {s} isn't a flag the game can be asked for: refused");
                probes.insert(arg(1)?.to_string());
            }
            "set" | "clear" => {
                let p = [int(1)?, int(2)?, int(3)?];
                let state = if parts[0] == "clear" { "minecraft:air" } else { arg(4)? };
                let rest = blocks.get(&p);
                let on_detector = rest.is_some_and(|s| s.starts_with("minecraft:detector_rail"));
                if cart && on_detector && state == "minecraft:redstone_block" {
                    anyhow::ensure!(pressed.insert(p), "line {n}: `{line}`: a second press of {p:?} while its cart is still there");
                    presses += 1;
                } else if cart && on_detector && state.starts_with("minecraft:detector_rail") {
                    anyhow::ensure!(Some(&state.to_string()) == rest, "line {n}: `{line}`: a detector rail set to a state other than its rest state {}", rest.map(String::as_str).unwrap_or("-"));
                    anyhow::ensure!(pressed.remove(&p), "line {n}: `{line}`: a release of {p:?} with no cart pressing it");
                } else if cart && on_detector {
                    // (G1 08:16 non-blocking) with real carts a detector cell is only pressed or released
                    bail!("line {n}: `{line}`: with --cart a detector rail's cell {p:?} is only pressed (a redstone block) or released (its rest state)");
                }
            }
            "quiet" | "step" => {
                int(1)?;
            }
            "expect" => {
                anyhow::ensure!(probes.contains(arg(1)?), "line {n}: `{line}`: no probe {}", arg(1)?);
                arg(2)?;
                checks += 1;
            }
            "expect_trace" => {
                anyhow::ensure!(probes.contains(arg(1)?), "line {n}: `{line}`: no probe {}", arg(1)?);
                let want = arg(2)?;
                anyhow::ensure!(!want.contains('?'), "line {n}: expect_trace {} {want}: '?' is refused (every tick's value is exact: ironspec's no-wildcard ruling)", arg(1)?);
                checks += 1;
            }
            other => bail!("line {n}: script command {other} is not supported in the game replay"),
        }
    }
    anyhow::ensure!(checks > 0, "the script has no expect or expect_trace: a drive with no check proves nothing");
    anyhow::ensure!(!cart || presses > 0, "--cart was given but the script presses no detector rail");
    Ok((checks, presses))
}

/// An `{Items:[{Slot:…,id:"…",count:…}, …]}` fill as a set of normalized stacks (`Slot=…;count=…;id=…`, keys sorted,
/// spaces dropped); anything else (other keys, components, a stack without its slot, id or count) is refused
pub fn item_set(snbt: &str) -> Result<BTreeSet<String>> {
    let s: String = snbt.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = s.strip_prefix("{Items:[").and_then(|t| t.strip_suffix("]}")).with_context(|| format!("not an {{Items:[…]}} fill: {snbt}"))?;
    let mut out = BTreeSet::new();
    if inner.is_empty() {
        return Ok(out);
    }
    for stack in inner.split("},{") {
        let body = stack.trim_start_matches('{').trim_end_matches('}');
        anyhow::ensure!(!body.contains('{') && !body.contains('['), "a stack with nested data (components) is not supported: {stack}");
        let mut kv: Vec<(String, String)> = Vec::new();
        for pair in body.split(',') {
            let (k, v) = pair.split_once(':').with_context(|| format!("a stack field without a value: {pair}"))?;
            // (G1 car-14 non-blocking) a byte suffix is trimmed from NUMBERS only (`0b` -> `0`), never from an id
            let v = v.trim_matches('"');
            let v = match v.strip_suffix(['b', 'B']) {
                Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '-') => n,
                _ => v,
            };
            kv.push((k.trim_matches('"').to_string(), v.to_string()));
        }
        kv.sort();
        let keys: Vec<&str> = kv.iter().map(|(k, _)| k.as_str()).collect();
        anyhow::ensure!(keys == ["Slot", "count", "id"], "a stack must carry exactly Slot, id and count: {stack}");
        out.insert(kv.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(";"));
    }
    Ok(out)
}

pub fn game_drive(version: &str, design: &std::path::Path, drive: &std::path::Path, cart: bool) -> Result<()> {
    let d = mctas::design::Design::load(design)?;
    let blocks: BTreeMap<[i32; 3], String> = d.block_list().into_iter().collect();
    let script = std::fs::read_to_string(drive).with_context(|| format!("read {}", drive.display()))?;
    let (planned, presses) = check_script(&script, &blocks, design, cart)?;
    let md5 = |p: &std::path::Path| -> Result<String> { Ok(crate::md5::md5_hex(&std::fs::read(p)?)[..12].to_string()) };
    println!(
        "== game-drive on the official {version} server: {} (md5 {}, {} cells) at local + {O:?}, the script {} (md5 {}): {planned} checks{}",
        design.display(),
        md5(design)?,
        blocks.len(),
        drive.display(),
        md5(drive)?,
        if cart { format!("; each of its {presses} detector presses a REAL minecart at rest on the rail") } else { String::new() }
    );
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add 0 0 31 31")?;
    srv.step(20)?;
    let (lo, hi) = ([O[0] - 4, O[1] - 2, O[2] - 4], [O[0] + 12, O[1] + 8, O[2] + 12]);
    srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:air", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]))?;
    // the rest state as the design writes it (the portal paste: every cell placed, then the `nbt:` lines, each read back)
    let world_blocks: Vec<([i32; 3], String)> = blocks.iter().map(|(p, s)| (w(*p), s.clone())).collect();
    crate::portal::paste(&mut srv, "minecraft:overworld", &world_blocks)?;
    for (p, nbt) in &d.block_entities {
        let q = w(*p);
        // (G1 08:16 (2)) only an `{Items:[…]}` fill is supported, and it is read back item for item (slot, id, count)
        let want = item_set(nbt).with_context(|| format!("the design's nbt at local {p:?}: {nbt}"))?;
        srv.cmd_ok(&format!("data merge block {} {} {} {nbt}", q[0], q[1], q[2]))?;
        let back = srv.cmd_long(&format!("data get block {} {} {} Items", q[0], q[1], q[2]))?;
        let got = if back.contains("Found no elements") { BTreeSet::new() } else { item_set(&format!("{{Items:{}}}", back.split("block data: ").nth(1).unwrap_or("").trim())).with_context(|| format!("the read-back at local {p:?}: {back}"))? };
        anyhow::ensure!(got == want, "the block entity at local {p:?} read back {got:?}, the design writes {want:?}");
        println!("  nbt at local {p:?}: {nbt} (read back: {} item stack(s), each slot / id / count equal)", got.len());
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
    let read = |srv: &mut VanillaServer, p: [i32; 3], is_items: bool| -> Result<u32> {
        if is_items {
            return items(srv, p);
        }
        let s = &blocks[&p];
        let flag = probe_flag(s).context("a probe whose value isn't a flag")?;
        let id = s.split('[').next().unwrap_or(s);
        Ok(u32::from(srv.test_block(w(p), &format!("{id}[{flag}=true]"))?))
    };
    let line_of = |srv: &mut VanillaServer, probes: &[(String, [i32; 3], bool)], tick: u32| -> Result<String> {
        let mut s = format!("    t{tick:>3}:");
        for (name, p, is_items) in probes {
            let v = read(srv, *p, *is_items)?;
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
        read(srv, p, is_items)
    };
    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let n = lineno + 1;
        let arg = |i: usize| -> Result<&str> { parts.get(i).copied().with_context(|| format!("line {n}: `{line}` needs argument {i}")) };
        let int = |i: usize| -> Result<i32> { arg(i)?.parse::<i32>().with_context(|| format!("line {n}: `{line}`: argument {i} is not a number")) };
        match parts[0] {
            "design" => {}
            "probe_items" | "probe" => probes.push((arg(1)?.to_string(), [int(2)?, int(3)?, int(4)?], parts[0] == "probe_items")),
            "set" | "clear" => {
                let p = [int(1)?, int(2)?, int(3)?];
                let state = if parts[0] == "clear" { "minecraft:air".to_string() } else { arg(4)?.to_string() };
                let q = w(p);
                let on_detector = blocks.get(&p).is_some_and(|s| s.starts_with("minecraft:detector_rail"));
                if cart && on_detector && state == "minecraft:redstone_block" {
                    cart_n += 1;
                    let tag = format!("gd{cart_n}");
                    srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {}.0625 {}.5 {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,0.0d]}}", q[0], q[1], q[2]))?;
                    cart_at.insert(p, tag);
                    println!("  (line {n}) t{tick}: PRESS: a minecart at rest on the detector rail ({},{},{})", p[0], p[1], p[2]);
                } else if cart && on_detector && state.starts_with("minecraft:detector_rail") {
                    let tag = cart_at.remove(&p).with_context(|| format!("line {n}: a release of {p:?} with no cart"))?;
                    srv.cmd_ok(&format!("execute as @e[tag={tag}] run tp @s {} {} {}", q[0], q[1] - 40, q[2]))?;
                    srv.cmd_ok(&format!("kill @e[tag={tag}]"))?;
                    // its dropped item, below the gadget only
                    srv.cmd(&format!("kill @e[type=minecraft:item,x={},y={},z={},dx=2,dy=4,dz=2]", q[0] - 1, q[1] - 42, q[2] - 1))?;
                    println!("  (line {n}) t{tick}: RELEASE: the cart removed (the detector rail re-checks on its own)");
                } else {
                    let out = srv.cmd(&format!("setblock {} {} {} {state}", q[0], q[1], q[2]))?;
                    println!("  (line {n}) t{tick}: setblock ({},{},{}) {state}{}", p[0], p[1], p[2], if out.contains("Could not") { " (unchanged)" } else { "" });
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
                let (name, want) = (arg(1)?, arg(2)?);
                let got = value(&mut srv, &probes, name)?;
                checks += 1;
                let ok = got.to_string() == want;
                println!("  (line {n}) t{tick}: expect {name}={want}: got {got} {}", if ok { "ok" } else { "FAIL" });
                if !ok {
                    failures.push(format!("line {n}: t{tick} expect {name}={want}, got {got}"));
                }
            }
            "expect_trace" => {
                let (name, want) = (arg(1)?, arg(2)?);
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
                let ok = want == got;
                println!("  (line {n}) t{tick}: expect_trace {name} {want}: got {got} {}", if ok { "ok" } else { "FAIL" });
                if !ok {
                    failures.push(format!("line {n}: t{tick} expect_trace {name} {want}, got {got}"));
                }
            }
            other => bail!("line {n}: script command {other} is not supported in the game replay"),
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    anyhow::ensure!(checks == planned, "{checks} checks ran, {planned} planned");
    println!("  {checks} checks, {} failed{}", failures.len(), if failures.is_empty() { String::new() } else { format!(": {failures:?}") });
    anyhow::ensure!(failures.is_empty(), "{} of {checks} checks failed in the game: {failures:?}", failures.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenes() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scenes")
    }

    fn gadget() -> (std::path::PathBuf, BTreeMap<[i32; 3], String>) {
        let p = scenes().join("ironfarm-arm3-interlock-0c7958d76b4f.txt");
        let d = mctas::design::Design::load(&p).expect("the vendored ARM3 gadget");
        (p, d.block_list().into_iter().collect())
    }

    /// the vendored ARM3 drives pass the pre-pass: the block form 34 checks, the real-cart form 15 with its presses
    #[test]
    fn the_arm3_drives_check() {
        let (p, b) = gadget();
        let block = std::fs::read_to_string(scenes().join("ironfarm-arm3-interlock-4bb2283578f4.drive")).expect("the block drive");
        let cart = std::fs::read_to_string(scenes().join("ironfarm-arm3-interlock-cart-15b39b371cda.gamedrive")).expect("the cart drive");
        assert_eq!(check_script(&block, &b, &p, false).expect("the block drive checks").0, 34);
        let (checks, presses) = check_script(&cart, &b, &p, true).expect("the cart drive checks");
        assert_eq!(checks, 15);
        assert_eq!(presses, 3, "the cart drive's presses");
    }

    /// each refusal by name (G1 06:54 (2)/(3), ironspec 06:43 (1)(c))
    #[test]
    fn the_refusals() {
        let (p, b) = gadget();
        let head = "probe_items a 3 1 1\nprobe q 2 1 2\n";
        let err = |s: &str, cart: bool| check_script(s, &b, &p, cart).expect_err("refused").to_string();
        assert!(err(&format!("{head}expect_trace q 0?1\n"), false).contains("'?' is refused"));
        assert!(err("probe_signal s 2 1 2\nexpect s 0\n", false).contains("probe_signal"));
        assert!(err("probe s 1 1 1\nexpect s 0\n", false).contains("isn't a flag"));
        assert!(err(&format!("design designs/gadgets/other.txt\n{head}expect a 1\n"), false).contains("different design"));
        assert!(err(&format!("design designs/gadgets/ironfarm-arm3-interlock.txt 1 0 0\n{head}expect a 1\n"), false).contains("offset"));
        assert!(err(head, false).contains("no expect"));
        assert!(err(&format!("{head}expect a 1\n"), true).contains("presses no detector rail"));
        assert!(err(&format!("{head}set 3 1 0 minecraft:redstone_block\nset 3 1 0 minecraft:redstone_block\nexpect a 1\n"), true).contains("second press"));
        assert!(err(&format!("{head}set 3 1 0 minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]\nexpect a 1\n"), true).contains("no cart pressing it"));
        assert!(err(&format!("{head}set 3 1 0 minecraft:redstone_block\nset 3 1 0 minecraft:detector_rail[powered=true,shape=east_west,waterlogged=false]\nexpect a 1\n"), true).contains("rest state"));
        assert!(err(&format!("{head}expect\n"), false).contains("needs argument 1"));
        // (G1 08:16 (2)) a drive script's own `nbt:` line is refused; with --cart a detector cell is only pressed or
        // released, so clearing a pressed rail is refused
        assert!(err(&format!("{head}nbt: 3 1 1 {{Items:[]}}\nexpect a 1\n"), false).contains("`nbt:` line in a drive script is refused"));
        assert!(err(&format!("{head}set 3 1 0 minecraft:redstone_block\nclear 3 1 0\nexpect a 1\n"), true).contains("only pressed"));
        // the vendored copy stands for designs/gadgets/ironfarm-arm3-interlock.txt by the md5 in its name
        assert!(check_script(&format!("design designs/gadgets/ironfarm-arm3-interlock.txt\n{head}expect a 1\n"), &b, &p, false).is_ok());
    }

    /// (G1 08:16 (2)) the read-back compares stacks by slot, id and count: order and spacing don't matter, a count or a
    /// slot does; an empty fill is the empty set; other SNBT is refused
    #[test]
    fn item_sets() {
        let a = item_set("{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}").expect("a fill");
        let b = item_set("{Items:[{count: 1, id: \"minecraft:stone\", Slot: 0b}]}").expect("the game's print");
        assert_eq!(a, b);
        assert_ne!(a, item_set("{Items:[{Slot:0b,id:\"minecraft:stone\",count:2}]}").expect("count 2"));
        assert_ne!(a, item_set("{Items:[{Slot:1b,id:\"minecraft:stone\",count:1}]}").expect("slot 1"));
        assert!(item_set("{Items:[]}").expect("empty").is_empty());
        assert!(item_set("{Items:[{id:\"minecraft:stone\",count:1}]}").is_err(), "a stack without its slot");
        assert!(item_set("{CustomName:\"x\"}").is_err(), "not an Items fill");
        assert!(item_set("{Items:[{Slot:0b,id:\"minecraft:stone\",count:1,components:{}}]}").is_err(), "components");
        // an id ending in b keeps its b
        let tub = item_set("{Items:[{Slot:0b,id:\"minecraft:tnt_minecart_tub\",count:1}]}").expect("an id ending in b");
        assert!(tub.iter().next().is_some_and(|s| s.contains("id=minecraft:tnt_minecart_tub")), "{tub:?}");
    }
}
