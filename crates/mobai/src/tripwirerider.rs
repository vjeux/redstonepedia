//! (tickexact 2026-11-12, ironspec: the hold release's cheaper check) `mctas mobai tripwire-rider`:
//! on the official 26.3 server, does a villager seated in a cart trip a tripwire string over the
//! cart's cell? A straight north-south rail on stone at y 64 through z 0..12, the string at
//! (0,65,6) (the cell over the rail's (0,64,6)) between hooks (0,65,5) facing south and (0,65,7)
//! facing north (each toward the string), each on stone behind it, the string placed last
//! (attached). Rows, each in a fresh
//! copy 20 blocks east: (i) a ridden cart at rest on (0,64,6), (ii) a ridden cart rolling through at
//! `--speed`, the controls (iii) an empty cart at rest and rolling, (iv) a villager standing in the
//! string's cell, (v) a villager walking through it (its stroll toward a target past it). Every
//! tick: the hooks' and the string's powered states, the cart's and the rider's positions.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 64;

fn powered(srv: &mut VanillaServer, x: i32, y: i32, z: i32, id: &str) -> Result<bool> {
    Ok(srv.cmd_long(&format!("execute if block {x} {y} {z} {id}[powered=true]"))?.contains("passed"))
}

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1], v[2]]))
}

/// One copy of the scene at x = `x`.
fn build(srv: &mut VanillaServer, x: i32) -> Result<()> {
    srv.cmd_ok(&format!("fill {} {} -2 {} {} 14 minecraft:air", x - 2, Y - 1, x + 2, Y + 4))?;
    srv.cmd_ok(&format!("fill {x} {} 0 {x} {} 12 minecraft:stone", Y - 1, Y - 1))?;
    srv.cmd_ok(&format!("fill {x} {Y} 0 {x} {Y} 12 minecraft:rail[shape=north_south]"))?;
    // the hooks on stone behind them, one block up (over the rail line's neighbours)
    srv.cmd_ok(&format!("setblock {x} {} 4 minecraft:stone", Y + 1))?;
    srv.cmd_ok(&format!("setblock {x} {} 8 minecraft:stone", Y + 1))?;
    srv.cmd_ok(&format!("setblock {x} {} 5 minecraft:tripwire_hook[facing=south]", Y + 1))?;
    srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:tripwire_hook[facing=north]", Y + 1))?;
    srv.cmd_ok(&format!("setblock {x} {} 6 minecraft:tripwire", Y + 1))?;
    srv.step(5)?;
    Ok(())
}

pub fn tripwire_rider(version: &str, speed: f64, ticks: u32) -> Result<()> {
    let opts = VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, keep_dir: false, tick_rate: 1000, world: None, ..Default::default() };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -16 -16 175 31")?;
    srv.step(20)?;
    println!("== tripwire-rider on the official {version} server: a rail on stone at y {Y}, the string at y {} over the rail's cell z 6 between hooks at z 5 and 7", Y + 1);
    // (name, summon at z, motion z, passenger)
    let rows: [(&str, f64, f64, Option<&str>); 6] = [
        ("(i) a ridden cart at rest under the string", 6.5, 0.0, Some("minecart")),
        ("(ii) a ridden cart rolling through", 1.5, speed, Some("minecart")),
        ("(iii-a) an empty cart at rest under the string", 6.5, 0.0, None),
        ("(iii-b) an empty cart rolling through", 1.5, speed, None),
        ("(iv) a villager standing on the rail under the string (its body in the string's cell)", 6.5, 0.0, Some("villager")),
        ("(v) a villager dropped into the string's cell from 2 blocks up", 6.5, 0.0, Some("walker")),
    ];
    for (k, (name, z0, vz, who)) in rows.iter().enumerate() {
        let x = 20 * k as i32;
        build(&mut srv, x)?;
        let s1 = powered(&mut srv, x, Y + 1, 5, "minecraft:tripwire_hook")?;
        let s2 = powered(&mut srv, x, Y + 1, 7, "minecraft:tripwire_hook")?;
        let st = powered(&mut srv, x, Y + 1, 6, "minecraft:tripwire")?;
        println!("  {name} (x {x}): before: hooks {}/{}, string {}", s1 as u8, s2 as u8, st as u8);
        let tag = format!("tw{k}");
        match who {
            Some("minecart") => srv.cmd_ok(&format!(
                "summon minecraft:minecart {}.5 {}.0625 {z0} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,{vz}d],Passengers:[{{id:\"minecraft:villager\",PersistenceRequired:1b,Tags:[\"{tag}v\"]}}]}}",
                x, Y
            ))?,
            None => srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {}.0625 {z0} {{Tags:[\"{tag}\"],Motion:[0.0d,0.0d,{vz}d]}}", x, Y))?,
            Some("villager") => srv.cmd_ok(&format!("summon minecraft:villager {}.5 {} {z0} {{PersistenceRequired:1b,Tags:[\"{tag}v\"]}}", x, Y))?,
            _ => {
                // a villager moving through the string's cell: dropped into it from 2 blocks up
                // (the stones behind the hooks block a walk along the rail line at head height)
                srv.cmd_ok(&format!("summon minecraft:villager {}.5 {} {z0} {{PersistenceRequired:1b,Tags:[\"{tag}v\"]}}", x, Y + 3))?
            }
        };
        let (mut first, mut n_on) = (None, 0u32);
        for t in 1..=ticks {
            srv.step(1)?;
            let h1 = powered(&mut srv, x, Y + 1, 5, "minecraft:tripwire_hook")?;
            let h2 = powered(&mut srv, x, Y + 1, 7, "minecraft:tripwire_hook")?;
            let s = powered(&mut srv, x, Y + 1, 6, "minecraft:tripwire")?;
            if s || h1 || h2 {
                n_on += 1;
                first.get_or_insert(t);
            }
            let c = pos(&mut srv, &format!("@e[tag={tag},limit=1]"))?;
            let v = pos(&mut srv, &format!("@e[tag={tag}v,limit=1]"))?;
            if t <= 3 || t % 5 == 0 || s || h1 || h2 {
                println!(
                    "    t{t}: hooks {}/{} string {} | cart {} | villager {}",
                    h1 as u8,
                    h2 as u8,
                    s as u8,
                    c.map(|p| format!("({:.3},{:.4},{:.3})", p[0], p[1] - Y as f64, p[2])).unwrap_or_else(|| "-".into()),
                    v.map(|p| format!("({:.3},{:.4},{:.3})", p[0], p[1] - Y as f64, p[2])).unwrap_or_else(|| "-".into())
                );
            }
        }
        println!("  {name}: the string or a hook powered on {n_on} of {ticks} ticks{}", first.map(|t| format!(", first at t{t}")).unwrap_or_default());
        srv.cmd(&format!("kill @e[tag={tag}]"))?;
        srv.cmd(&format!("kill @e[tag={tag}v]"))?;
    }
    Ok(())
}
