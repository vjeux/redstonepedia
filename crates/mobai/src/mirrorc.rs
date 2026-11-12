//! (tickexact 2026-11-12, ironfarm via farmopt: the mirrored module's C) `mctas mobai mirror-c`: on
//! the official 26.3 server, the mirrored module's merge curve C (0,24,-4) (the unmirrored C
//! (x,24,7) mirrored by z' = 3 - z) with its neighbours as built: D (0,24,-5) a detector rail
//! ascending north into J (0,25,-6) (a plain T, arms W/E and down D), the row's rail W of C at
//! (-1,24,-4); and the placement orders: (a) ironfarm's: the helper rail at (0,24,-3) on a temporary
//! glass support (0,23,-3), C placed unpowered, the helper then its support removed; (b) the
//! unmirrored recipe (a redstone block at (1,24,-4) while C is placed, then removed): expected north_west,
//! the wrong shape. Each: C's shape after every step, then a ridden cart pressing and releasing D,
//! and a save, stop and start; module-check on the placed cells.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::PathBuf;

const DY: i32 = 80;

fn c_shape(srv: &mut VanillaServer, ox: i32) -> Result<String> {
    for s in ["south_west", "north_west", "south_east", "north_east", "north_south", "east_west", "ascending_north", "ascending_south", "ascending_east", "ascending_west"] {
        if srv.cmd_long(&format!("execute if block {ox} {} -4 minecraft:rail[shape={s}]", 24 + DY))?.contains("passed") {
            return Ok(s.to_string());
        }
    }
    Ok("not a rail".into())
}

fn set(srv: &mut VanillaServer, ox: i32, p: [i32; 3], s: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {s}", ox + p[0], p[1] + DY, p[2]))?;
    Ok(())
}

pub fn mirror_c(version: &str) -> Result<()> {
    let opts = |world: Option<PathBuf>| VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("forceload add -16 -16 31 15")?;
    srv.step(20)?;
    println!("== mirror-c on the official {version} server (y less {DY}): the mirrored C (0,24,-4), D (0,24,-5) ascending north into J (0,25,-6), the row's rail (-1,24,-4)");
    let mut lines = Vec::new();
    for (row, ox, powered) in [("(a) ironfarm's: the helper and its support, C unpowered", 0, false), ("(b) the unmirrored recipe: a redstone block east of C while it is placed", 10, true)] {
        srv.cmd_ok(&format!("fill {} {} -9 {} {} 0 minecraft:air", ox - 3, 21 + DY, ox + 3, 27 + DY))?;
        // the supports and the built neighbours (rails as stated)
        for p in [[0, 23, -5], [0, 24, -6], [-1, 24, -6], [1, 24, -6], [-1, 23, -4], [0, 23, -4]] {
            set(&mut srv, ox, p, "minecraft:glass")?;
        }
        set(&mut srv, ox, [-1, 25, -6], "minecraft:rail[shape=east_west] strict")?;
        set(&mut srv, ox, [1, 25, -6], "minecraft:rail[shape=east_west] strict")?;
        set(&mut srv, ox, [0, 25, -6], "minecraft:rail[shape=south_east] strict")?;
        set(&mut srv, ox, [0, 24, -5], "minecraft:detector_rail[shape=ascending_north,powered=false] strict")?;
        set(&mut srv, ox, [-1, 24, -4], "minecraft:rail[shape=east_west] strict")?;
        let mut steps = Vec::new();
        set(&mut srv, ox, [0, 23, -3], "minecraft:glass")?;
        set(&mut srv, ox, [0, 24, -3], "minecraft:rail[shape=north_south]")?;
        if powered {
            set(&mut srv, ox, [1, 24, -4], "minecraft:redstone_block")?;
        }
        set(&mut srv, ox, [0, 24, -4], "minecraft:rail")?;
        srv.step(2)?;
        steps.push(format!("C placed: {}", c_shape(&mut srv, ox)?));
        set(&mut srv, ox, [0, 24, -3], "minecraft:air")?;
        srv.step(2)?;
        steps.push(format!("the helper removed: {}", c_shape(&mut srv, ox)?));
        set(&mut srv, ox, [0, 23, -3], "minecraft:air")?;
        srv.step(2)?;
        steps.push(format!("its support removed: {}", c_shape(&mut srv, ox)?));
        if powered {
            set(&mut srv, ox, [1, 24, -4], "minecraft:air")?;
            srv.step(2)?;
            steps.push(format!("the redstone block removed: {}", c_shape(&mut srv, ox)?));
        }
        // D pressed by a ridden cart set down on it, then released
        srv.cmd_ok(&format!("summon minecraft:minecart {} {} -4.5 {{Tags:[\"mc{ox}\"],Passengers:[{{id:\"minecraft:villager\",NoAI:1b,Tags:[\"mv{ox}\"]}}]}}", ox as f64 + 0.5, 24.5625 + DY as f64))?;
        srv.cmd_ok(&format!("tp @e[tag=mc{ox}] {} {} -4.5", ox as f64 + 0.5, 24.5625 + DY as f64))?;
        srv.step(10)?;
        let pressed = srv.cmd_long(&format!("execute if block {ox} {} -5 minecraft:detector_rail[powered=true]", 24 + DY))?.contains("passed");
        steps.push(format!("D {}: C {}", if pressed { "pressed" } else { "NOT pressed" }, c_shape(&mut srv, ox)?));
        srv.cmd_ok(&format!("kill @e[tag=mc{ox}]"))?;
        srv.cmd_ok(&format!("kill @e[tag=mv{ox}]"))?;
        srv.step(30)?;
        steps.push(format!("D released: C {}", c_shape(&mut srv, ox)?));
        lines.push((row, ox, steps));
    }
    // a save, stop and start
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut srv = VanillaServer::start(&opts(Some(world)))?;
    let _ = std::fs::remove_dir_all(&old);
    srv.step(20)?;
    for (row, ox, steps) in &mut lines {
        steps.push(format!("after a restart: C {}", c_shape(&mut srv, *ox)?));
        println!("  {row}: {}", steps.join("; "));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
