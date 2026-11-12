//! (tickexact 2026-11-12, ironspec and farmopt: ironfarm's top-slope placement step, so the build
//! guide's order is official) `mctas mobai rail-slope`: on the official 26.3 server, ironfarm's lane
//! top slope S at (0,22,6), designed ascending south (its high end toward (0,23,7)), with a plain
//! rail L at its low end (0,21,5) on a support and two more rails north of L; S on a support at
//! (0,21,6). The guide's order: glass G at (0,22,7) (S's high side); a temporary helper rail H at
//! (0,23,7) on G; then S, which sees L one level down north and H one level up south
//! (`RailState.place`: north and south -> NORTH_SOUTH, then a rail above the south neighbour ->
//! ASCENDING_SOUTH); then H removed. Printed: S's shape after each step; after H's removal (S's
//! high side is G, a sturdy top, so `shouldBeRemoved` keeps S); after a neighbour update (a stone
//! set and cleared beside S); after farmopt's next step (glass placed where H stood, the merge's
//! support); through a ridden cart rolling down S onto L; after a save, stop and start. The
//! control: S placed the same way with no H takes the flat shape toward its low end
//! (north_south).

use crate::railt::{cart, shape};
use anyhow::{Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::PathBuf;

fn s(srv: &mut VanillaServer, ox: i32, x: i32, y: i32, z: i32, block: &str) -> Result<()> {
    srv.cmd_ok(&format!("setblock {} {y} {z} {block}", ox + x))?;
    Ok(())
}

/// The low end and its supports, S's support and G; `helper`: H on G before S.
fn build(srv: &mut VanillaServer, ox: i32, helper: bool) -> Result<String> {
    for z in 3..=5 {
        s(srv, ox, 0, 20, z, "minecraft:stone")?;
        s(srv, ox, 0, 21, z, "minecraft:rail")?;
    }
    s(srv, ox, 0, 21, 6, "minecraft:stone")?;
    s(srv, ox, 0, 22, 7, "minecraft:glass")?;
    if helper {
        s(srv, ox, 0, 23, 7, "minecraft:rail")?;
    }
    s(srv, ox, 0, 22, 6, "minecraft:rail")?;
    shape(srv, [ox, 22, 6], "minecraft:rail")
}

pub fn rail_slope(version: &str) -> Result<()> {
    println!("== rail-slope on the official {version} server: ironfarm's lane top slope (0,22,6), designed ascending south");
    let opts = |world: Option<PathBuf>| VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        keep_dir: true,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        world,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts(None))?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 47 31")?;
    srv.step(20)?;
    let sp = [0, 22, 6];
    let mut steps = Vec::new();
    let mut say = |srv: &mut VanillaServer, what: &str| -> Result<String> {
        let sh = shape(srv, sp, "minecraft:rail")?;
        println!("  {what}: S {sh}");
        steps.push(sh.clone());
        Ok(sh)
    };
    let h = build(&mut srv, 0, true)?;
    println!("  H (the helper at (0,23,7)) {}", shape(&mut srv, [0, 23, 7], "minecraft:rail")?);
    println!("  S placed with L below north and H above south: S {h}");
    srv.step(2)?;
    say(&mut srv, "two ticks later")?;
    s(&mut srv, 0, 0, 23, 7, "minecraft:air")?;
    srv.step(2)?;
    say(&mut srv, "H removed (G, a sturdy top, on S's high side)")?;
    s(&mut srv, 0, 1, 22, 6, "minecraft:stone")?;
    srv.step(1)?;
    s(&mut srv, 0, 1, 22, 6, "minecraft:air")?;
    srv.step(2)?;
    say(&mut srv, "a neighbour update (a stone set and cleared east of S)")?;
    s(&mut srv, 0, 0, 23, 7, "minecraft:glass")?;
    srv.step(2)?;
    say(&mut srv, "glass placed where H stood (farmopt's next step: the merge's support)")?;
    // a ridden cart set on S's upper half rolls down it (north) onto L and beyond
    cart(&mut srv, [0.5, 22.6, 6.7], [0.0, 0.0], "slope")?;
    let mut path = Vec::new();
    let mut changed = 0;
    let mut prev = shape(&mut srv, sp, "minecraft:rail")?;
    for t in 1..=40 {
        srv.step(1)?;
        let now = shape(&mut srv, sp, "minecraft:rail")?;
        if now != prev {
            changed += 1;
            println!("  the cart +{t}: S {now}");
            prev = now;
        }
        if t % 5 == 0 {
            let out = srv.cmd_long("data get entity @e[type=minecraft:minecart,tag=slope,limit=1] Pos")?;
            path.push(format!("t{t} {}", out.split("entity data: ").nth(1).unwrap_or("gone").trim()));
        }
    }
    println!("  the cart down S: S changed {changed} times; path {}", path.join(" "));
    say(&mut srv, "after the cart")?;
    srv.cmd_ok("save-on")?;
    srv.cmd_ok("save-all flush")?;
    let old = srv.dir.clone();
    let world = old.join(&srv.level_name);
    drop(srv);
    let mut srv = VanillaServer::start(&opts(Some(world)))?;
    let _ = std::fs::remove_dir_all(&old);
    srv.step(1)?;
    say(&mut srv, "after a save, stop and start")?;
    // the control: no helper
    let c = build(&mut srv, 20, false)?;
    srv.step(2)?;
    let c2 = shape(&mut srv, [20, 22, 6], "minecraft:rail")?;
    println!("  control (S placed with no H): S {c}, two ticks later {c2}");
    let _ = std::fs::remove_dir_all(&srv.dir);
    let kept = h == "ascending_south" && steps.iter().all(|s| s == "ascending_south");
    println!("verdict: S {} through every step; the control without H {c2}", if kept { "kept ascending_south" } else { "did NOT keep ascending_south" });
    if c2 == "ascending_south" {
        bail!("rail-slope: the control without the helper ascends too: the probe can't show the helper's effect");
    }
    Ok(())
}
