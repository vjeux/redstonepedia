//! `mctas door drive <script>...`: drive a design by hand (place/remove blocks, use, step) and
//! print the z=0 plane as ASCII after every step. For debugging mechanisms before wiring them,
//! and for proving gadgets (`probe` + `trace` + `expect`, see designs/gadgets/).
//!
//! Script lines: `design <path|@name>`, `plane <z> <x0> <y0> <x1> <y1>`, `planex <x> <z0> <y0> <z1> <y1>`
//! (side view), `set x y z <state>`
//! (update mode, like a player), `clear x y z`, `use x y z`, `step <n>` (prints each tick),
//! `quiet <n>` (steps without printing), `print`.
//!
//! Probes: `probe <name> x y z` watches a block (`probe_signal <name> x y z` watches the best
//! signal the cell's neighbors give into it: what a piston or lamp there would see, shown as its
//! level, 0/+ in `expect_trace`); `trace <n>` steps n ticks printing one line per
//! tick with every probe's compact value (see [`probe_value`]: dust power, 1/0 for powered or lit
//! components, E/r for extended/retracted pistons, ~ moving, . air, # other solid);
//! `expect <name> <value>` checks a probe now; `expect_state x y z <text>` checks that the block
//! state contains `<text>`; `expect_trace <name> <values>` steps one tick per character of
//! `<values>` and checks the probe after each (one character per tick: dust 0 or +, others their
//! probe value, `?` = any).
//! A failed expectation makes the command exit non-zero.
//!
//! Change log: `box x0 y0 z0 x1 y1 z1` sets a 3D watch box, `changes <n>` steps n ticks printing
//! every block change inside it per tick, `mark <label>` prints a label and re-baselines the log.
//!
//! Block entities (item clocks): a design's `nbt:` lines are merged right after its raw paste
//! (`design` and `place`); `place <path> [dx dy dz]` places like the clock judge WITHOUT settling
//! (tick 0 = right after placement); `data x y z <snbt>` merges data now (the game's
//! `data merge block`); `items x y z` prints a block entity's data; `probe_items <name> x y z`
//! watches a container's total item count; `expect_be`/`reject_be x y z <text>` assert on a
//! block entity's printed data, `expect_no_be x y z` that there is none.

use anyhow::{Context, Result, bail};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};

fn cell(state: &str) -> char {
    let Ok(b) = BlockState::parse(state) else { return '?' };
    let arrow = |f: Option<&str>| match f {
        Some("up") => '^',
        Some("down") => 'v',
        Some("east") => '>',
        Some("west") => '<',
        Some("north") => 'n',
        Some("south") => 'u',
        _ => '*',
    };
    match b.short_name() {
        "air" => ' ',
        "stone" => '#',
        "slime_block" => 's',
        "honey_block" => 'y',
        "sticky_piston" | "piston" => {
            if b.get("extended") == Some("true") { 'P' } else { 'p' }
        }
        "piston_head" => arrow(b.get("facing")),
        "moving_piston" => '~',
        "observer" => {
            if b.get("powered") == Some("true") { 'O' } else { 'o' }
        }
        "repeater" => {
            if b.get("powered") == Some("true") { 'Q' } else { 'q' }
        }
        "comparator" => {
            if b.get("powered") == Some("true") { 'C' } else { 'c' }
        }
        "redstone_block" => 'R',
        "redstone_torch" | "redstone_wall_torch" => {
            if b.get("lit") == Some("true") { 'T' } else { 't' }
        }
        "redstone_lamp" | "copper_bulb" | "waxed_copper_bulb" => {
            if b.get("lit") == Some("true") { 'B' } else { 'b' }
        }
        "redstone_wire" => {
            if b.get("power").is_some_and(|p| p != "0") { '+' } else { '.' }
        }
        "lever" => {
            if b.get("powered") == Some("true") { 'L' } else { 'l' }
        }
        "target" => 'x',
        _ => '%',
    }
}

/// Compact value of a block for probes: dust power (0-15), 1/0 for powered/lit components,
/// E/r for extended/retracted pistons, h piston head, ~ moving block, . air, # anything else.
pub fn probe_value(state: &str) -> String {
    let Ok(b) = BlockState::parse(state) else { return "?".into() };
    let flag = |k: &str| if b.get(k) == Some("true") { "1" } else { "0" };
    match b.short_name() {
        "air" => ".".into(),
        "redstone_wire" => b.get("power").unwrap_or("0").to_string(),
        "sticky_piston" | "piston" => (if b.get("extended") == Some("true") { "E" } else { "r" }).into(),
        "piston_head" => "h".into(),
        "moving_piston" => "~".into(),
        "redstone_torch" | "redstone_wall_torch" | "redstone_lamp" => flag("lit").into(),
        n if n.ends_with("copper_bulb") => flag("lit").into(),
        "repeater" | "comparator" | "observer" | "lever" | "stone_button" | "oak_button" => flag("powered").into(),
        n if n.ends_with("_rail") || n.ends_with("trapdoor") || n.ends_with("door") || n == "note_block" => flag("powered").into(),
        n if n.ends_with("fence_gate") || n.ends_with("_skull") || n.ends_with("_head") || n.ends_with("_shelf") => flag("powered").into(),
        "dispenser" | "dropper" | "crafter" => flag("triggered").into(),
        "scaffolding" => b.get("distance").unwrap_or("?").to_string(),
        "target" | "sculk_sensor" | "calibrated_sculk_sensor" | "daylight_detector" => b.get("power").unwrap_or("0").to_string(),
        "slime_block" => "s".into(),
        "honey_block" => "y".into(),
        "redstone_block" => "R".into(),
        _ => "#".into(),
    }
}

pub(crate) fn render(r: &Region, z: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let mut out = String::new();
    for y in (y0..=y1).rev() {
        out.push_str(&format!("{y:>4} "));
        for x in x0..=x1 {
            out.push(r.get([x, y, z]).map(cell).unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

/// Side view of the plane `x`, z from left to right (same cell characters as [`render`]).
pub(crate) fn render_x(r: &Region, x: i32, z0: i32, y0: i32, z1: i32, y1: i32) -> String {
    let mut out = String::new();
    for y in (y0..=y1).rev() {
        out.push_str(&format!("{y:>4} "));
        for z in z0..=z1 {
            out.push(r.get([x, y, z]).map(cell).unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

/// Outcome of one script: how many expectations ran and which failed.
#[derive(Default, Debug)]
pub struct DriveOutcome {
    pub checks: u32,
    pub failures: Vec<String>,
}

const PROBE_BLOCK: u8 = 0;
const PROBE_SIGNAL: u8 = 1;
const PROBE_ITEMS: u8 = 2;

/// A `probe_items` value: the total item count of a container's data (`-` without a block
/// entity, `?` when the data does not parse).
pub fn items_value(nbt: Option<&str>) -> String {
    use redsim::redstone::container::{Snbt, parse_snbt};
    let Some(nbt) = nbt else { return "-".into() };
    let Ok(Snbt::Compound(fields)) = parse_snbt(nbt) else { return "?".into() };
    let mut total = 0i64;
    for (k, v) in &fields {
        if let (true, Snbt::List(items)) = (k == "Items", v) {
            for it in items {
                if let Snbt::Compound(f) = it {
                    total += f.iter().find(|(k, _)| k == "count").map(|(_, v)| if let Snbt::Int(n) = v { *n } else { 0 }).unwrap_or(1);
                }
            }
        }
    }
    total.to_string()
}

/// Place a design with block-entity data: the judge's sequence (`video.rs::place`: raw paste,
/// wire shapes, a self update of every block, entities) with the `nbt:` lines merged right after
/// the raw paste ([`mctas::design::apply_block_entities`]); `settle` steps until idle like the
/// door judge, without it tick 0 is right after placement (clocks never settle).
pub fn place_with_block_entities(w: &mut Worker, sb: u32, d: &Design, settle: bool) -> Result<()> {
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    mctas::design::apply_block_entities(w, sb, d)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    if !d.entities.is_empty() {
        w.summon(sb, &d.entities)?;
    }
    if settle {
        w.step_until_idle(sb, 400)?;
    }
    Ok(())
}

/// Run a script; `verbose` prints planes and traces (otherwise only failures are reported).
pub fn run(w: &mut Worker, script: &str) -> Result<DriveOutcome> {
    run_opts(w, script, true)
}

pub fn run_opts(w: &mut Worker, script: &str, verbose: bool) -> Result<DriveOutcome> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = run_in(w, sb, script, verbose, None);
    let _ = w.drop_sandbox(sb);
    r
}

/// Run a script silently and return every cell it sweeps: each cell that is non-air at any tick
/// (within the design's box + 3), plus every neighbor of a slime/honey block or a moving piston
/// at any tick. `door route --swept-script` uses it to wire a mechanism that so far only works
/// with ideal (scripted) inputs.
pub fn swept(w: &mut Worker, script: &str) -> Result<std::collections::HashSet<Pos>> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut cells = std::collections::HashSet::new();
    let r = run_in(w, sb, script, false, Some(&mut cells));
    let _ = w.drop_sandbox(sb);
    r?;
    Ok(cells)
}

fn collect_cells(w: &mut Worker, sb: u32, bx: Option<(Pos, Pos)>, out: &mut std::collections::HashSet<Pos>) -> Result<()> {
    let Some((lo, hi)) = bx else { return Ok(()) };
    let r = w.get_region(sb, lo, hi)?;
    for (p, s) in r.iter() {
        if mctas::blockstate::is_air(s) {
            continue;
        }
        out.insert(p);
        if s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block") || s.contains("moving_piston") {
            for f in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                out.insert([p[0] + f[0], p[1] + f[1], p[2] + f[2]]);
            }
        }
    }
    Ok(())
}

fn run_in(w: &mut Worker, sb: u32, script: &str, verbose: bool, mut collect: Option<&mut std::collections::HashSet<Pos>>) -> Result<DriveOutcome> {
    let mut bx: Option<(Pos, Pos)> = None;
    let mut plane = (0, -6, -5, 8, 9);
    // `planex <x> <z0> <y0> <z1> <y1>`: show a side view (an x plane, z left to right) instead.
    let mut plane_x: Option<(i32, i32, i32, i32, i32)> = None;
    let mut tick = 0u32;
    let mut watch: Option<(Pos, Pos)> = None;
    let mut prev: Option<Region> = None;
    // Probe kinds: PROBE_BLOCK (the block's probe value), PROBE_SIGNAL, PROBE_ITEMS.
    let mut probes: Vec<(String, Pos, u8)> = Vec::new();
    let mut out = DriveOutcome::default();
    let show = |w: &mut Worker, label: &str, plane: (i32, i32, i32, i32, i32), plane_x: Option<(i32, i32, i32, i32, i32)>| -> Result<()> {
        if let Some((x, z0, y0, z1, y1)) = plane_x {
            let r = w.get_region(sb, [x, y0, z0], [x, y1, z1])?;
            println!("-- {label} (x={x}, z {z0}..{z1})\n{}", render_x(&r, x, z0, y0, z1, y1));
            return Ok(());
        }
        let (z, x0, y0, x1, y1) = plane;
        let r = w.get_region(sb, [x0, y0, z], [x1, y1, z])?;
        println!("-- {label}\n{}", render(&r, z, x0, y0, x1, y1));
        Ok(())
    };
    let read = |w: &mut Worker, p: Pos| -> Result<String> {
        let r = w.get_region(sb, p, p)?;
        Ok(r.get(p).unwrap_or("?").to_string())
    };
    // A probe's value: the block's probe value, for a `probe_signal` the best signal the cell's
    // neighbors give into it (0-15: what a piston or lamp there would see), for a `probe_items`
    // the container's item count ([`items_value`]).
    let value = |w: &mut Worker, p: Pos, kind: u8| -> Result<String> {
        match kind {
            PROBE_SIGNAL => Ok(w.signals(sb, &[p])?.first().map(|s| s.0).unwrap_or(0).to_string()),
            PROBE_ITEMS => Ok(items_value(w.block_entities(sb, p, p)?.first().map(|b| b.nbt.as_str()))),
            _ => Ok(probe_value(&read(w, p)?)),
        }
    };
    let trace_line = |w: &mut Worker, probes: &[(String, Pos, u8)], tick: u32| -> Result<String> {
        let mut line = format!("t={tick:<4}");
        for (name, p, kind) in probes {
            line.push_str(&format!(" {name}={}", value(w, *p, *kind)?));
        }
        Ok(line)
    };
    for (lineno, raw) in script.lines().enumerate() {
        // Comments: a line starting with '#', or " # " followed by text ('#' alone is a value).
        let line = raw.trim();
        let line = if line.starts_with('#') { "" } else { line.split(" # ").next().unwrap().trim() };
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> {
            parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into)
        };
        let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        match parts[0] {
            "design" => {
                let d: Design = crate::cli::load_design(std::path::Path::new(parts[1]))?;
                // Optional world offset (`design <path> dx dy dz`): reproduce position-dependent
                // behavior seen by the verifier; script coordinates stay design-relative only if
                // you add the offset yourself.
                let d = if parts.len() >= 5 { d.translated([int(2)?, int(3)?, int(4)?]) } else { d };
                if d.block_entities.is_empty() {
                    crate::video::place(w, sb, &d)?;
                } else {
                    place_with_block_entities(w, sb, &d, true)?;
                }
                if let Some((lo, hi)) = d.bounds() {
                    bx = Some(([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]));
                }
                if let Some(c) = collect.as_deref_mut() {
                    collect_cells(w, sb, bx, c)?;
                }
            }
            // `place <path> [dx dy dz]`: place a design like the clock judge (raw paste, its
            // `nbt:` block-entity data, wire shapes, self updates) WITHOUT settling: tick 0 is
            // right after placement (a clock never settles).
            "place" => {
                let d: Design = crate::cli::load_design(std::path::Path::new(parts.get(1).context("place needs a design")?))?;
                let d = if parts.len() >= 5 { d.translated([int(2)?, int(3)?, int(4)?]) } else { d };
                place_with_block_entities(w, sb, &d, false)?;
                if let Some((lo, hi)) = d.bounds() {
                    bx = Some(([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]));
                }
            }
            // `data x y z <snbt>`: merge block-entity data now, exactly like the game's
            // `data merge block x y z <snbt>` (container contents: setChanged notifies comparators).
            "data" => {
                let snbt = line.splitn(5, char::is_whitespace).nth(4).context("data needs x y z and SNBT")?.trim();
                w.merge_block_entity(sb, pos(1)?, snbt)?;
            }
            // `items x y z`: print the block entity's data there (container items, cooldown).
            "items" => {
                let p = pos(1)?;
                let be = w.block_entities(sb, p, p)?;
                if verbose {
                    match be.first() {
                        Some(b) => println!("  t={tick} {:?} {} {}", p, b.kind, b.nbt),
                        None => println!("  t={tick} {p:?} no block entity"),
                    }
                }
            }
            "plane" => {
                plane = (int(1)?, int(2)?, int(3)?, int(4)?, int(5)?);
                plane_x = None;
            }
            "planex" => plane_x = Some((int(1)?, int(2)?, int(3)?, int(4)?, int(5)?)),
            "set" => {
                let state = parts.get(4).context("set needs a state")?;
                w.set_blocks(sb, &[(pos(1)?, state.to_string())], PlaceMode::Update)?;
            }
            "clear" => {
                w.set_blocks(sb, &[(pos(1)?, "minecraft:air".to_string())], PlaceMode::Update)?;
            }
            // `setblock x y z <state>`: the game's own `/setblock` command (shape updates, then
            // neighbor updates: `SetBlockCommand.setBlock`), so `door vanilla-gadget` can replay the
            // very same call on the official server. Use it for ideal inputs in gadget scripts.
            "setblock" => {
                let state = parts.get(4).context("setblock needs a state")?;
                let p = pos(1)?;
                w.exec(sb, &format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
            }
            "use" => {
                w.use_block(sb, pos(1)?)?;
            }
            // `summon <type> x y z [snbt]`: a design entity (decimal coordinates), like a design's
            // `entity:` line.
            "summon" => {
                let rest = line.splitn(2, char::is_whitespace).nth(1).context("summon needs a type and a position")?;
                let e = mctas::design::EntitySpec::parse(rest)?;
                w.summon(sb, &[e])?;
            }
            // `entities`: print every entity (type, position, motion, at rest, state).
            "entities" => {
                if verbose {
                    for e in w.entities(sb)? {
                        println!(
                            "  t={tick} {} pos=[{}, {}, {}] motion=[{:.5}, {:.5}, {:.5}]{}{} {}",
                            e.kind,
                            e.pos[0],
                            e.pos[1],
                            e.pos[2],
                            e.motion[0],
                            e.motion[1],
                            e.motion[2],
                            if e.still { " still" } else { " moving" },
                            if e.design { "" } else { " (not the design's)" },
                            e.state
                        );
                    }
                }
            }
            // `etrace <n>`: step n ticks, printing every entity's position and motion after each.
            "etrace" => {
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    if verbose {
                        let ents = w.entities(sb)?;
                        let line: Vec<String> = ents
                            .iter()
                            .map(|e| {
                                format!(
                                    "{}@[{:.4},{:.4},{:.4}] v=[{:.4},{:.4},{:.4}]{}",
                                    e.kind.trim_start_matches("minecraft:"),
                                    e.pos[0],
                                    e.pos[1],
                                    e.pos[2],
                                    e.motion[0],
                                    e.motion[1],
                                    e.motion[2],
                                    if e.still { " still" } else { "" }
                                )
                            })
                            .collect();
                        let probes_line = trace_line(w, &probes, tick)?;
                        println!("{probes_line} {}", line.join(" "));
                    }
                }
            }
            // `expect_entity <type> x y z`: an entity of that type stands exactly there.
            "expect_entity" => {
                let kind = parts.get(1).context("expect_entity needs a type")?;
                let kind = if kind.contains(':') { kind.to_string() } else { format!("minecraft:{kind}") };
                let f = |i: usize| -> Result<f64> { Ok(parts.get(i).context("expect_entity needs x y z")?.parse::<f64>()?) };
                let want = [f(2)?, f(3)?, f(4)?];
                let ents = w.entities(sb)?;
                out.checks += 1;
                if !ents.iter().any(|e| e.kind == kind && (0..3).all(|i| (e.pos[i] - want[i]).abs() <= crate::ENTITY_POS_TOLERANCE)) {
                    let seen: Vec<String> = ents.iter().map(|e| format!("{} {:?}", e.kind, e.pos)).collect();
                    out.failures.push(format!("line {}: t={tick} expect {kind} at {want:?}, have {}", lineno + 1, seen.join("; ")));
                }
            }
            // `expect_entities <n>`: exactly n entities exist.
            "expect_entities" => {
                let n = int(1)? as usize;
                let ents = w.entities(sb)?;
                out.checks += 1;
                if ents.len() != n {
                    out.failures.push(format!("line {}: t={tick} expect {n} entities, have {}", lineno + 1, ents.len()));
                }
            }
            "step" => {
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    if let Some(c) = collect.as_deref_mut() {
                        collect_cells(w, sb, bx, c)?;
                    }
                    if verbose {
                        show(w, &format!("t={tick}"), plane, plane_x)?;
                    }
                }
            }
            "quiet" => {
                let n = int(1)? as u32;
                if let Some(c) = collect.as_deref_mut() {
                    for _ in 0..n {
                        w.step(sb, 1)?;
                        collect_cells(w, sb, bx, c)?;
                    }
                } else {
                    w.step(sb, n)?;
                }
                tick += n;
            }
            // `settle [max]`: step until nothing is pending (like the verifier between flips).
            "settle" => {
                let max = if parts.len() > 1 { int(1)? as u32 } else { 400 };
                let r = w.step_until_idle(sb, max)?;
                tick += r.ticked;
                if verbose {
                    println!("-- settled after {} ticks (t={tick})", r.ticked);
                }
            }
            "print" => {
                if verbose {
                    show(w, &format!("t={tick}"), plane, plane_x)?
                }
            }
            "probe" | "probe_signal" | "probe_items" => {
                let name = parts.get(1).context("probe needs a name")?.to_string();
                let kind = match parts[0] {
                    "probe_signal" => PROBE_SIGNAL,
                    "probe_items" => PROBE_ITEMS,
                    _ => PROBE_BLOCK,
                };
                probes.push((name, pos(2)?, kind));
            }
            "trace" => {
                if verbose {
                    println!("{}", trace_line(w, &probes, tick)?);
                }
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    if verbose {
                        println!("{}", trace_line(w, &probes, tick)?);
                    }
                }
            }
            "expect" => {
                let name = parts.get(1).context("expect needs a probe name")?;
                let want = parts.get(2).context("expect needs a value")?;
                let (_, p, signal) = probes.iter().find(|(n, _, _)| n == name).with_context(|| format!("no probe {name}"))?.clone();
                let got = value(w, p, signal)?;
                out.checks += 1;
                if got != *want {
                    out.failures.push(format!("line {}: t={tick} expect {name}={want}, got {got}", lineno + 1));
                }
            }
            "expect_state" => {
                let want = parts.get(4).context("expect_state needs text")?;
                let got = read(w, pos(1)?)?;
                out.checks += 1;
                if !got.contains(want) {
                    out.failures.push(format!("line {}: t={tick} expect state containing {want}, got {got}", lineno + 1));
                }
            }
            // `expect_be x y z <text>`: the block entity there exists and its data contains
            // <text>; `reject_be` the converse (also true with no block entity there);
            // `expect_no_be x y z`: no block entity there at all.
            "expect_be" | "reject_be" => {
                let want = line.splitn(5, char::is_whitespace).nth(4).context("expect_be needs x y z and text")?.trim();
                let p = pos(1)?;
                let be = w.block_entities(sb, p, p)?;
                let got = be.first().map(|b| format!("{} {}", b.kind, b.nbt)).unwrap_or_else(|| "<no block entity>".to_string());
                let has = got.contains(want);
                let ok = if parts[0] == "expect_be" { has } else { !has };
                out.checks += 1;
                if !ok {
                    out.failures.push(format!("line {}: t={tick} {} {p:?} containing {want}, got {got}", lineno + 1, parts[0]));
                }
            }
            "expect_no_be" => {
                let p = pos(1)?;
                let be = w.block_entities(sb, p, p)?;
                out.checks += 1;
                if let Some(b) = be.first() {
                    out.failures.push(format!("line {}: t={tick} expect_no_be {p:?}, got {} {}", lineno + 1, b.kind, b.nbt));
                }
            }
            "expect_trace" => {
                let name = parts.get(1).context("expect_trace needs a probe name")?;
                let want = parts.get(2).context("expect_trace needs values")?;
                let (_, p, signal) = probes.iter().find(|(n, _, _)| n == name).with_context(|| format!("no probe {name}"))?.clone();
                let mut got = String::new();
                for _ in want.chars() {
                    w.step(sb, 1)?;
                    tick += 1;
                    let v = value(w, p, signal)?;
                    // One character per tick: dust and signals are 0 or + (powered), others
                    // their probe value.
                    let c = match v.parse::<u32>() {
                        Ok(n) if signal == PROBE_SIGNAL => if n > 0 { '+' } else { '0' },
                        Ok(n) if v.len() > 1 || n > 1 => '+',
                        _ => v.chars().next().unwrap_or('?'),
                    };
                    got.push(c);
                    if verbose {
                        println!("{}", trace_line(w, &probes, tick)?);
                    }
                }
                out.checks += 1;
                let ok = want.chars().zip(got.chars()).all(|(a, b)| a == '?' || a == b);
                if !ok {
                    out.failures.push(format!("line {}: t={tick} expect_trace {name} {want}, got {got}", lineno + 1));
                }
            }
            // `edges <probe> <n>`: step n ticks and print the ticks where the probe rises (its value
            // goes from 0 / false to anything else) and the gaps between rises: a clock's period
            // measured in the game (`expect_gaps` checks them).
            "edges" | "expect_gaps" => {
                let name = parts.get(1).context("edges needs a probe name")?;
                let n = int(2)?;
                let (_, p, signal) = probes.iter().find(|(nm, _, _)| nm == name).with_context(|| format!("no probe {name}"))?.clone();
                let off = |v: &str| v == "0" || v == "false" || v == ".";
                let mut was_off = off(&value(w, p, signal)?);
                let mut rises: Vec<u32> = Vec::new();
                for _ in 0..n {
                    w.step(sb, 1)?;
                    tick += 1;
                    let now_off = off(&value(w, p, signal)?);
                    if was_off && !now_off {
                        rises.push(tick);
                    }
                    was_off = now_off;
                }
                let gaps: Vec<String> = rises.windows(2).map(|r| (r[1] - r[0]).to_string()).collect();
                if parts[0] == "edges" {
                    let at: Vec<String> = rises.iter().map(|t| t.to_string()).collect();
                    println!("edges {name}: rises at [{}] gaps [{}]", at.join(" "), gaps.join(" "));
                } else {
                    // `expect_gaps <probe> <n> <g1,g2,..>`: the last gaps must repeat the list.
                    let want: Vec<&str> = parts.get(3).context("expect_gaps needs gaps")?.split(',').collect();
                    out.checks += 1;
                    let ok = gaps.len() >= want.len() && gaps[gaps.len() - want.len()..].iter().zip(&want).all(|(a, b)| a == b);
                    if !ok {
                        out.failures.push(format!("line {}: t={tick} expect_gaps {name} {}, got [{}]", lineno + 1, want.join(","), gaps.join(" ")));
                    }
                }
            }
            "box" => {
                watch = Some((pos(1)?, pos(4)?));
                prev = None;
            }
            "changes" => {
                // Steps n ticks, printing every block change inside the `box` per tick (3D).
                let (lo, hi) = watch.context("`changes` needs a `box x0 y0 z0 x1 y1 z1` first")?;
                if prev.is_none() {
                    prev = Some(w.get_region(sb, lo, hi)?);
                }
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    let now = w.get_region(sb, lo, hi)?;
                    let before = prev.as_ref().unwrap();
                    let short = |s: &str| s.strip_prefix("minecraft:").unwrap_or(s).to_string();
                    let diffs: Vec<String> = now
                        .iter()
                        .filter(|(p, s)| before.get(*p) != Some(*s))
                        .map(|(p, s)| format!("{p:?} {} -> {}", short(before.get(p).unwrap_or("?")), short(s)))
                        .collect();
                    if verbose && !diffs.is_empty() {
                        println!("t={tick:<3} {}", diffs.join("\n      "));
                    }
                    prev = Some(now);
                }
            }
            "mark" => {
                // Re-baseline the change log (hides scripted edits) and print a label.
                if let Some((lo, hi)) = watch {
                    prev = Some(w.get_region(sb, lo, hi)?);
                }
                if verbose {
                    println!("== {} (t={tick})", parts[1..].join(" "));
                }
            }
            // `bechanges <n>`: step n ticks in one recorded request (`record.block_entities`) and
            // print, per tick, every block entity inside the `box` whose data changed (container
            // items, hopper cooldowns, comparator output signals).
            "bechanges" => {
                let (lo, hi) = watch.context("`bechanges` needs a `box x0 y0 z0 x1 y1 z1` first")?;
                let n = int(1)? as u32;
                let before = w.block_entities(sb, lo, hi)?;
                let r = w.step_record_be(sb, n, false, lo, hi)?;
                let mut last: std::collections::BTreeMap<Pos, String> = before.into_iter().map(|b| (b.pos, b.nbt)).collect();
                for dump in &r.block_entities {
                    tick += 1;
                    let now = mctas::sim::parse_block_entities(dump)?;
                    let mut diffs = Vec::new();
                    for b in &now {
                        if last.get(&b.pos) != Some(&b.nbt) {
                            diffs.push(format!("{:?} {}", b.pos, b.nbt));
                        }
                    }
                    if verbose && !diffs.is_empty() {
                        println!("t={tick:<3} {}", diffs.join("\n      "));
                    }
                    last = now.into_iter().map(|b| (b.pos, b.nbt)).collect();
                }
            }
            other => bail!("line {}: unknown script command {other}", lineno + 1),
        }
    }
    Ok(out)
}
