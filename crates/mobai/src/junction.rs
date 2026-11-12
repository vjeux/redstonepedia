//! (tickexact 2026-11-12, ironspec: split9's GO waits on it) `mctas mobai junction-probe`: on the
//! official 26.3 server, 7b90c3f1's in-chunk split junction: J at (0,25,9), a plain-rail T with arms
//! W (-1,25,9), E (1,25,9) and N down D; D at (0,24,8), a detector rail ascending south under J's
//! north arm (its low end (0,24,7) a plain rail north); everything on glass. J's driver as built:
//! a repeater at (0,25,10) pointing into J's cell (facing south: input from the south, output north)
//! with a lever at (0,25,11) as its input. Rows: a ridden cart presses and releases D with the driver
//! off, then on; the driver toggled while D is pressed; a save, stop and start in each driver state
//! and once while D is pressed. Every tick: J's shape, D's shape and power. Expected: J always at its
//! driver's settle shape (north_east off, north_west on), D always ascending_south.
//! `--mirror` (ironspec's mirrored rows): the same mirrored in z (z' = -z, every state mirrored, as
//! the N = 4 southern modules): J's arms W, E and S down D ascending north; expected south_east off,
//! south_west on.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::PathBuf;

const DY: i32 = 80;

/// A cell's z, mirrored or not.
fn mz(m: bool, z: i32) -> i32 {
    if m {
        -z
    } else {
        z
    }
}

/// A state, mirrored in z or not.
fn ms(m: bool, s: &str) -> String {
    if m {
        crate::freeze::mirror_z(s)
    } else {
        s.to_string()
    }
}

/// The probe's blocks in placement order (rails after their supports and neighbours; J last of the
/// rails, so it settles from its neighbours and its power as a placed rail does).
fn blocks(m: bool) -> Vec<([i32; 3], String)> {
    [
        ([0, 24, 9], "minecraft:glass"),
        ([-1, 24, 9], "minecraft:glass"),
        ([1, 24, 9], "minecraft:glass"),
        ([0, 23, 8], "minecraft:glass"),
        ([0, 23, 7], "minecraft:glass"),
        ([0, 24, 10], "minecraft:glass"),
        ([0, 24, 11], "minecraft:glass"),
        ([0, 25, 11], "minecraft:lever[face=floor,facing=north,powered=false]"),
        ([0, 25, 10], "minecraft:repeater[delay=1,facing=south,locked=false,powered=false]"),
        ([0, 24, 7], "minecraft:rail[shape=north_south]"),
        ([0, 24, 8], "minecraft:detector_rail[shape=ascending_south,powered=false]"),
        ([-1, 25, 9], "minecraft:rail[shape=east_west]"),
        ([1, 25, 9], "minecraft:rail[shape=east_west]"),
        ([0, 25, 9], "minecraft:rail[shape=north_east]"),
    ]
    .into_iter()
    .map(|(p, s)| ([p[0], p[1], mz(m, p[2])], ms(m, s)))
    .collect()
}

fn shape(srv: &mut VanillaServer, p: [i32; 3], kind: &str, shapes: &[&str]) -> Result<String> {
    for s in shapes {
        if srv.cmd_long(&format!("execute if block {} {} {} minecraft:{kind}[shape={s}]", p[0], p[1] + DY, p[2]))?.contains("passed") {
            return Ok(s.to_string());
        }
    }
    Ok("other".into())
}

fn state(srv: &mut VanillaServer, m: bool) -> Result<String> {
    let j = shape(srv, [0, 25, mz(m, 9)], "rail", &["north_east", "north_west", "east_west", "north_south", "south_east", "south_west", "ascending_north", "ascending_south", "ascending_east", "ascending_west"])?;
    let d = shape(srv, [0, 24, mz(m, 8)], "detector_rail", &["ascending_south", "north_south", "ascending_north", "east_west"])?;
    let dp = srv.cmd_long(&format!("execute if block 0 {} {} minecraft:detector_rail[powered=true]", 24 + DY, mz(m, 8)))?.contains("passed");
    let lever = srv.cmd_long(&format!("execute if block 0 {} {} minecraft:lever[powered=true]", 25 + DY, mz(m, 11)))?.contains("passed");
    let rep = srv.cmd_long(&format!("execute if block 0 {} {} minecraft:repeater[powered=true]", 25 + DY, mz(m, 10)))?.contains("passed");
    Ok(format!("J {j}; D {d} {}; lever {}, repeater {}", if dp { "pressed" } else { "released" }, if lever { "on" } else { "off" }, if rep { "on" } else { "off" }))
}

fn watch(srv: &mut VanillaServer, m: bool, label: &str, n: u32, bad: &mut Vec<String>) -> Result<()> {
    for t in 1..=n {
        srv.step(1)?;
        let s = state(srv, m)?;
        // J's settle shape follows its driver's output, the repeater (2 gt behind the lever)
        let on = s.ends_with("repeater on");
        let want_j = match (m, on) {
            (false, true) => "J north_west",
            (false, false) => "J north_east",
            (true, true) => "J south_west",
            (true, false) => "J south_east",
        };
        let want_d = if m { "D ascending_north" } else { "D ascending_south" };
        let ok = s.contains(want_d) && s.starts_with(want_j);
        if !ok {
            bad.push(format!("{label} t{t}: {s}"));
        }
        if t <= 4 || t == n || !ok {
            println!("  {label} t{t}: {s}{}", if ok { "" } else { "  <-- not the expected shapes" });
        }
    }
    Ok(())
}

fn lever(srv: &mut VanillaServer, m: bool, on: bool) -> Result<()> {
    // (the lever may already be in that state: then nothing changes)
    srv.cmd(&format!("setblock 0 {} {} {}", 25 + DY, mz(m, 11), ms(m, &format!("minecraft:lever[face=floor,facing=north,powered={on}]"))))?;
    Ok(())
}

fn press(srv: &mut VanillaServer, m: bool) -> Result<()> {
    srv.cmd_ok(&format!("summon minecraft:minecart 0.5 {} {} {{Tags:[\"jp\"],Passengers:[{{id:\"minecraft:villager\",NoAI:1b}}]}}", 24.5625 + DY as f64, mz(m, 8) as f64 + 0.5))?;
    Ok(())
}

fn release(srv: &mut VanillaServer) -> Result<()> {
    srv.cmd_ok("kill @e[tag=jp]")?;
    srv.cmd_ok("kill @e[type=minecraft:villager]")?;
    Ok(())
}

pub fn junction_probe(version: &str, m: bool) -> Result<()> {
    let opts = |world: Option<PathBuf>| VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -16 -16 15 31")?;
    srv.step(20)?;
    let (z0, z1) = if m { (-13, -4) } else { (4, 13) };
    srv.cmd_ok(&format!("fill -3 {} {z0} 3 {} {z1} minecraft:air", DY + 22, DY + 27))?;
    for (p, s) in blocks(m) {
        srv.cmd_ok(&format!("setblock {} {} {} {s}", p[0], p[1] + DY, p[2]))?;
    }
    srv.step(5)?;
    // module-check: each placed block against what was placed
    let mut diffs = Vec::new();
    for (p, s) in blocks(m) {
        if !srv.cmd_long(&format!("execute if block {} {} {} {s}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            diffs.push(format!("({},{},{}) {s}", p[0], p[1], p[2]));
        }
    }
    println!(
        "== junction-probe{} on the official {version} server (y less {DY}); module-check: {} of {} placed blocks differ: {}",
        if m { " (mirrored in z)" } else { "" },
        diffs.len(),
        blocks(m).len(),
        diffs.join("; ")
    );
    println!("  after placing: {}", state(&mut srv, m)?);
    let mut bad = Vec::new();
    // 1. the driver off: press and release D
    press(&mut srv, m)?;
    watch(&mut srv, m, "off, D pressed", 20, &mut bad)?;
    release(&mut srv)?;
    watch(&mut srv, m, "off, D released", 20, &mut bad)?;
    // 2. the driver on: press and release D
    lever(&mut srv, m, true)?;
    watch(&mut srv, m, "driver switched on", 10, &mut bad)?;
    press(&mut srv, m)?;
    watch(&mut srv, m, "on, D pressed", 20, &mut bad)?;
    release(&mut srv)?;
    watch(&mut srv, m, "on, D released", 20, &mut bad)?;
    // 3. the driver toggled while D is pressed
    lever(&mut srv, m, false)?;
    watch(&mut srv, m, "driver switched off", 10, &mut bad)?;
    press(&mut srv, m)?;
    watch(&mut srv, m, "off, D pressed", 10, &mut bad)?;
    lever(&mut srv, m, true)?;
    watch(&mut srv, m, "D pressed, driver switched on", 10, &mut bad)?;
    lever(&mut srv, m, false)?;
    watch(&mut srv, m, "D pressed, driver switched off", 10, &mut bad)?;
    release(&mut srv)?;
    watch(&mut srv, m, "off, D released", 10, &mut bad)?;
    // 4. a save, stop and start: driver off; driver on; driver off with D pressed
    for (label, on, pressed) in [("driver off", false, false), ("driver on", true, false), ("driver off, D pressed", false, true)] {
        lever(&mut srv, m, on)?;
        if pressed {
            press(&mut srv, m)?;
        }
        watch(&mut srv, m, &format!("{label}, before the stop"), 10, &mut bad)?;
        srv.cmd_ok("save-on")?;
        srv.cmd_ok("save-all flush")?;
        let old = srv.dir.clone();
        let world = old.join(&srv.level_name);
        drop(srv);
        srv = VanillaServer::start(&opts(Some(world)))?;
        let _ = std::fs::remove_dir_all(&old);
        println!("  {label}: restarted; at the first read: {}", state(&mut srv, m)?);
        watch(&mut srv, m, &format!("{label}, after the start"), 20, &mut bad)?;
        if pressed {
            release(&mut srv)?;
            watch(&mut srv, m, "after the start, D released", 10, &mut bad)?;
        }
    }
    println!(
        "  ticks off the expected shapes (J {} with the driver off, {} on; D {}): {}{}",
        if m { "south_east" } else { "north_east" },
        if m { "south_west" } else { "north_west" },
        if m { "ascending_north" } else { "ascending_south" },
        bad.len(),
        if bad.is_empty() { String::new() } else { format!(": {}", bad.join("; ")) }
    );
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
