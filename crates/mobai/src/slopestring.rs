//! (tickexact 2026-11-13, ironspec 20:31 / 20:43, G1 20:35: the press TEST on a slope) `mctas mobai slope-string`:
//! on the official 26.3 server, R_1's geometry in a fresh world: a south-ascending rail (the slot-1 cell) on stone with
//! its neighbours (a flat rail north of it, the next ascending rail south of it), the slot-1 string one cell above
//! the slot cell between hooks one cell east and west on glass one further out (the record's F1 layout at (x,18,1),
//! hooks (x-1,18,1) / (x+1,18,1), glass (x+-2,18,1)), each row in a fresh copy 10 blocks apart. One EMPTY minecart
//! per row (none in the control); every tick for 40 gt: the string's and both hooks' powered states and the cart's
//! position (frame y given with the slot cell at 17). Rows:
//! - (ii) AT REST on the unpowered powered rail (R_1) at z 1.28 (the drawn rest), 1.79, 1.856, 1.89 (the game's
//!   measured slot-1 band, V4): summoned at the rail's own slope y (cell y + (z - 1) + 0.0625) with no motion;
//! - (i) MOVING THROUGH: on a plain ascending rail (no braking), an empty cart summoned on the flat rail north of it
//!   moving south (up the slope) at 0.4 and 0.6 b/t, and one summoned on the upper ascending rail moving north (down
//!   the slope) at 0.2 b/t;
//! - the control: no cart.

use anyhow::{Context, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 101;

fn set(srv: &mut VanillaServer, p: [i32; 3], state: &str) -> Result<()> {
    srv.cmd(&format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
    Ok(())
}

fn powered(srv: &mut VanillaServer, p: [i32; 3], block: &str) -> Result<bool> {
    srv.test_block(p, &format!("{block}[powered=true]"))
}

fn cart_pos(srv: &mut VanillaServer, tag: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
    let v: Vec<f64> = out.split('[').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().trim_end_matches(|c: char| c == ']' || c == 'd' || c.is_whitespace()).parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1], v[2]]) } else { None })
}

/// One row's cart: where it is summoned (world x offset added) and its motion.
struct Cart {
    pos: [f64; 3],
    motion: [f64; 3],
}

/// One copy at x = `x`: the rails (the slot cell an unpowered powered rail, or a plain rail when `plain`), the string
/// set, the cart (none for the control). Returns the ticks (of 40) the string read powered.
fn copy(srv: &mut VanillaServer, x: i32, label: &str, plain: bool, cart: Option<Cart>) -> Result<usize> {
    let slope = if plain { "minecraft:rail[shape=ascending_south]" } else { "minecraft:powered_rail[shape=ascending_south,powered=false]" };
    set(srv, [x, Y - 1, -1], "minecraft:stone")?;
    set(srv, [x, Y - 1, 0], "minecraft:stone")?;
    set(srv, [x, Y - 1, 1], "minecraft:stone")?;
    set(srv, [x, Y, 2], "minecraft:stone")?;
    set(srv, [x, Y + 1, 3], "minecraft:stone")?;
    set(srv, [x, Y, -1], "minecraft:rail[shape=north_south]")?;
    set(srv, [x, Y, 0], "minecraft:rail[shape=north_south]")?;
    set(srv, [x, Y, 1], slope)?;
    set(srv, [x, Y + 1, 2], slope)?;
    set(srv, [x, Y + 2, 3], "minecraft:rail[shape=north_south]")?;
    // the string one cell above the slot cell, hooks east / west, glass beyond them; the string laid last
    set(srv, [x - 2, Y + 1, 1], "minecraft:glass")?;
    set(srv, [x + 2, Y + 1, 1], "minecraft:glass")?;
    set(srv, [x - 1, Y + 1, 1], "minecraft:tripwire_hook[attached=false,facing=east,powered=false]")?;
    set(srv, [x + 1, Y + 1, 1], "minecraft:tripwire_hook[attached=false,facing=west,powered=false]")?;
    set(srv, [x, Y + 1, 1], "minecraft:tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=false]")?;
    srv.step(5)?;
    let attached = srv.test_block([x, Y + 1, 1], "minecraft:tripwire[attached=true]")?;
    let tag = format!("slopecart{x}");
    if let Some(c) = &cart {
        srv.cmd_ok(&format!(
            "summon minecraft:minecart {} {} {} {{Tags:[\"{tag}\"],Motion:[{:.4}d,{:.4}d,{:.4}d]}}",
            x as f64 + c.pos[0],
            c.pos[1],
            c.pos[2],
            c.motion[0],
            c.motion[1],
            c.motion[2]
        ))?;
    }
    println!("  copy x {x}: {label}; the slot cell {}; the string attached {attached}", if plain { "a plain ascending rail" } else { "R_1, an unpowered powered rail" });
    let mut on = 0usize;
    let mut first: Option<u32> = None;
    let mut off_rows: Vec<u32> = Vec::new();
    for t in 1..=40u32 {
        srv.step(1)?;
        let s = powered(srv, [x, Y + 1, 1], "minecraft:tripwire")?;
        let he = powered(srv, [x - 1, Y + 1, 1], "minecraft:tripwire_hook")?;
        let hw = powered(srv, [x + 1, Y + 1, 1], "minecraft:tripwire_hook")?;
        if s {
            on += 1;
            first.get_or_insert(t);
        } else if first.is_some() {
            off_rows.push(t);
        }
        let p = if cart.is_some() { cart_pos(srv, &tag)? } else { None };
        println!(
            "    t{t:>2}: string powered {s} | hooks {he} / {hw} | cart {}",
            p.map(|p| format!("z {:.4} frame y {:.4} (box top {:.4}; the band 18.0625..18.1563)", p[2], p[1] - Y as f64 + 17.0, p[1] - Y as f64 + 17.7)).unwrap_or_else(|| "-".into())
        );
    }
    println!(
        "    => the string powered on {on} of 40 ticks{}{}",
        first.map(|f| format!(", first at t{f}")).unwrap_or_default(),
        if off_rows.is_empty() { String::new() } else { format!("; unpowered after the first press on rows {off_rows:?}") }
    );
    Ok(on)
}

pub fn slope_string(version: &str) -> Result<()> {
    println!("== slope-string on the official {version} server (ironspec 20:31 / 20:43, G1 20:35): an EMPTY cart on a south-ascending slot cell under a string one cell above it, at rest and moving through; every tick printed");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -16 95 15")?;
    srv.step(20)?;
    let rest = |z: f64| Cart { pos: [0.5, Y as f64 + (z - 1.0) + 0.0625, z], motion: [0.0, 0.0, 0.0] };
    let mut sums = Vec::new();
    let rows: Vec<(i32, String, bool, Option<Cart>)> = vec![
        (0, "(ii) at rest z 1.280 (the drawn rest)".into(), false, Some(rest(1.28))),
        (10, "(ii) at rest z 1.790".into(), false, Some(rest(1.79))),
        (20, "(ii) at rest z 1.856".into(), false, Some(rest(1.856))),
        (30, "(ii) at rest z 1.890".into(), false, Some(rest(1.89))),
        (40, "(i) moving through, up the slope from the flat rail at 0.4 b/t".into(), true, Some(Cart { pos: [0.5, Y as f64 + 0.0625, -0.5], motion: [0.0, 0.0, 0.4] })),
        (50, "(i) moving through, up the slope from the flat rail at 0.6 b/t".into(), true, Some(Cart { pos: [0.5, Y as f64 + 0.0625, -0.5], motion: [0.0, 0.0, 0.6] })),
        (60, "(i) moving through, down the slope from the upper ascending rail at 0.2 b/t".into(), true, Some(Cart { pos: [0.5, Y as f64 + 1.5625, 2.5], motion: [0.0, 0.0, -0.2] })),
        (70, "the control: no cart".into(), false, None),
    ];
    for (x, label, plain, cart) in rows {
        let on = copy(&mut srv, x, &label, plain, cart).with_context(|| format!("copy x {x}"))?;
        sums.push(format!("{label} -> {on}/40"));
    }
    println!("  summary (string powered ticks of 40): {}", sums.join("; "));
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
