//! (tickexact 2026-11-14, dfb89499 09:30: candidate A's landing reshapes when its rails are PLACED) `mctas mobai
//! rail-place`: on the official 26.3 server, two plain rails one block apart in height on lane x -1 (A's lanes -1 / 3): the
//! flat (x,0,2) north_south on its support (x,-1,2), and the landing (x,-1,3) north_south on its support (x,-2,3), set
//! three ways in three copies: the upper first then the lower, the lower first then the upper (each `setblock` a real
//! placement: `BaseRailBlock.onPlace` 60-64 -> `updateState` 66-73 -> `updateDir` 107-114 -> `RailState.place`, which joins a
//! rail to its neighbours at its own height, above and below and can lift either), and both `setblock … strict` (the
//! paste's form). Each copy's two rail shapes are read back from the game (`execute if block` over the ten rail shapes).
//! Asserted (MCTAS_RAIL_PLACE_PLANT=k inverts assert k, read strictly 1..=2; a planted run that never reaches its assert
//! fails, "control planted nothing"): (1) placed in either order, the landing (x,-1,3) ends `ascending_north` and the flat
//! (x,0,2) stays `north_south` (dfb89499's reading of RailState); (2) set strict, both keep `north_south`.

use anyhow::{bail, Result};
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 100;
const SHAPES: [&str; 10] = ["north_south", "east_west", "ascending_east", "ascending_west", "ascending_north", "ascending_south", "south_east", "south_west", "north_west", "north_east"];

fn plant() -> Result<Option<u32>> {
    match std::env::var("MCTAS_RAIL_PLACE_PLANT") {
        Err(_) => Ok(None),
        Ok(s) => {
            let k: u32 = s.trim().parse().map_err(|_| anyhow::anyhow!("MCTAS_RAIL_PLACE_PLANT={s:?} is not an assert number"))?;
            anyhow::ensure!((1..=5).contains(&k), "MCTAS_RAIL_PLACE_PLANT={k}: rail-place has asserts 1..5 (1..2 by default, 3..5 with --join)");
            Ok(Some(k))
        }
    }
}

fn pin(k: u32, cond: bool, what: String) -> Result<()> {
    let planted = plant()? == Some(k);
    anyhow::ensure!(cond != planted, "[assert {k}{}] {what}", if planted { ", PLANTED: its expectation inverted" } else { "" });
    Ok(())
}

/// The plain rail's shape at `p` as the game holds it (None: not a plain rail)
fn shape(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<&'static str>> {
    for s in SHAPES {
        if srv.test_block(p, &format!("minecraft:rail[shape={s}]"))? {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub fn rail_place(version: &str, join: bool) -> Result<()> {
    plant()?;
    if join {
        return join_rows(version);
    }
    println!("== rail-place on the official {version} server: lane x -1's flat (x,0,2) and landing (x,-1,3), placed in each order and set strict (y as A's module, the module's y 0 at {Y})");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -8 32 16")?;
    srv.cmd_ok(&format!("fill -16 {} -8 32 {} 16 minecraft:air", Y - 4, Y + 4))?;
    srv.step(20)?;
    // (label, x offset, order: true = the upper first, strict)
    let copies = [("placed: the flat (x,0,2) first, then the landing (x,-1,3)", 0, true, false), ("placed: the landing (x,-1,3) first, then the flat (x,0,2)", 8, false, false), ("set strict (the paste's form), the flat first", 16, true, true)];
    let mut got = Vec::new();
    for (label, dx, upper_first, strict) in copies {
        let x = -1 + dx;
        let (up, low) = ([x, Y, 2], [x, Y - 1, 3]);
        srv.cmd_ok(&format!("setblock {x} {} 2 minecraft:stone", Y - 1))?;
        srv.cmd_ok(&format!("setblock {x} {} 3 minecraft:stone", Y - 2))?;
        srv.step(2)?;
        let place = |srv: &mut VanillaServer, p: [i32; 3]| -> Result<()> {
            srv.cmd_ok(&format!("setblock {} {} {} minecraft:rail[shape=north_south,waterlogged=false]{}", p[0], p[1], p[2], if strict { " strict" } else { "" }))?;
            srv.step(2)
        };
        if upper_first {
            place(&mut srv, up)?;
            place(&mut srv, low)?;
        } else {
            place(&mut srv, low)?;
            place(&mut srv, up)?;
        }
        srv.step(10)?;
        let (su, sl) = (shape(&mut srv, up)?, shape(&mut srv, low)?);
        println!("  {label}: the flat (x,0,2) reads {}, the landing (x,-1,3) reads {}", su.unwrap_or("not a plain rail"), sl.unwrap_or("not a plain rail"));
        got.push((su, sl));
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    let placed_ok = got[..2].iter().all(|&(u, l)| u == Some("north_south") && l == Some("ascending_north"));
    pin(1, placed_ok, format!("(1) placed in either order the landing (x,-1,3) ends ascending_north and the flat (x,0,2) stays north_south: {:?}", &got[..2]))?;
    pin(2, got[2] == (Some("north_south"), Some("north_south")), format!("(2) set strict, both keep north_south: {:?}", got[2]))?;
    if let Some(k) = plant()? {
        bail!("control planted nothing: MCTAS_RAIL_PLACE_PLANT={k} never failed its assert");
    }
    println!("  every known answer held (asserts 1..2)");
    Ok(())
}

/// (dfb89499 / farmopt 09:59: the station row's JOIN T) `rail-place --join`: the T at (-1,0,7) with its three rails (north
/// (-1,0,6), or the restored slope's rail one block lower (-1,-1,6); west (-2,0,7); east (0,0,7)), each on stone, and LIFT's
/// slime (-1,-1,7) under the T on an always-lit torch (-1,-2,7) on glass (-1,-3,7) (the torch powers the slime above it
/// strongly, so the T reads a neighbour signal); placed by hand (`setblock`, a real placement) in two orders: the three
/// neighbours then the T, and the T then its neighbours; the T's shape read back POWERED, then with the torch removed
/// (`RailBlock.updateState` re-runs the T's direction when a signal-source neighbour changes and it has 3 possible
/// connections). Asserted (MCTAS_RAIL_PLACE_PLANT=3 inverts it): (3) dfb89499's reading: with the flat north rail, the T is
/// `north_west` powered and `north_east` unpowered, in both orders.
fn join_rows(version: &str) -> Result<()> {
    println!("== rail-place --join on the official {version} server: the station's JOIN T (-1,0,7) on LIFT's slime and lit torch, its three rails placed in two orders, read powered and with the torch removed (the module's y 0 at {Y})");
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -8 48 24")?;
    srv.cmd_ok(&format!("fill -16 {} -8 48 {} 24 minecraft:air", Y - 5, Y + 4))?;
    srv.step(20)?;
    // (label, x offset, the T first, the north rail one block lower, the torch: 0 first (before the rails), 1 last (after
    // the rails: rev 3's "build: the torch last"), 2 never (the unpowered control))
    let copies = [
        ("flat north, the torch first, the neighbours, then the T", 0, false, false, 0u8),
        ("flat north, the torch first, the T, then its neighbours", 10, true, false, 0),
        ("the slope's north rail (-1,-1,6), the torch first, the neighbours, then the T", 20, false, true, 0),
        ("the slope's north rail (-1,-1,6), the torch first, the T, then its neighbours", 30, true, true, 0),
        ("flat north, the neighbours, the T, then the torch LAST", 40, false, false, 1),
        ("flat north, the T, its neighbours, then the torch LAST", 50, true, false, 1),
        ("flat north, NO torch (the unpowered control)", 60, false, false, 2),
    ];
    let mut flat_reads = Vec::new();
    let mut control: Option<Option<&str>> = None;
    for (label, dx, t_first, slope, torch) in copies {
        let x = -1 + dx;
        // LIFT's column under the T: glass, the lit torch (first, last or never), the slime
        srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:glass", Y - 3))?;
        if torch == 0 {
            srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:redstone_torch[lit=true]", Y - 2))?;
        }
        srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:slime_block", Y - 1))?;
        let north = if slope { [x, Y - 1, 6] } else { [x, Y, 6] };
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:stone", north[0], north[1] - 1, north[2]))?;
        srv.cmd_ok(&format!("setblock {} {} 7 minecraft:stone", x - 1, Y - 1))?;
        srv.cmd_ok(&format!("setblock {} {} 7 minecraft:stone", x + 1, Y - 1))?;
        srv.step(4)?;
        let t = [x, Y, 7];
        let neighbours = [north, [x - 1, Y, 7], [x + 1, Y, 7]];
        let place = |srv: &mut VanillaServer, p: [i32; 3]| -> Result<()> {
            srv.cmd_ok(&format!("setblock {} {} {} minecraft:rail[shape=north_south,waterlogged=false]", p[0], p[1], p[2]))?;
            srv.step(2)
        };
        if t_first {
            place(&mut srv, t)?;
        }
        for n in neighbours {
            place(&mut srv, n)?;
        }
        if !t_first {
            place(&mut srv, t)?;
        }
        if torch == 1 {
            srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:redstone_torch[lit=true]", Y - 2))?;
        }
        srv.step(10)?;
        if torch == 2 {
            anyhow::ensure!(srv.test_block([x, Y - 2, 7], "minecraft:air")?, "{label}: the control has something under the slime");
            let unpowered = shape(&mut srv, t)?;
            println!("  {label}: the T (-1,0,7) reads {}", unpowered.unwrap_or("not a plain rail"));
            control = Some(unpowered);
            continue;
        }
        anyhow::ensure!(srv.test_block([x, Y - 2, 7], "minecraft:redstone_torch[lit=true]")?, "{label}: the torch under the slime is not lit");
        let powered = shape(&mut srv, t)?;
        let ns: Vec<Option<&str>> = neighbours.iter().map(|&p| shape(&mut srv, p)).collect::<Result<_>>()?;
        srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:air", Y - 2))?;
        srv.step(10)?;
        let unpowered = shape(&mut srv, t)?;
        println!(
            "  {label}: the T (-1,0,7) reads {} POWERED (torch lit), {} with the torch removed; its neighbours north {} / west {} / east {}",
            powered.unwrap_or("not a plain rail"),
            unpowered.unwrap_or("not a plain rail"),
            ns[0].unwrap_or("-"),
            ns[1].unwrap_or("-"),
            ns[2].unwrap_or("-")
        );
        if !slope {
            flat_reads.push((powered, unpowered));
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    pin(
        3,
        flat_reads.len() == 4 && flat_reads.iter().all(|&(p, u)| p == Some("north_west") && u == Some("north_east")) && control == Some(Some("north_east")),
        format!("(3) with the flat north rail the T is north_west powered and north_east once the torch is removed, the torch placed first or LAST, the T first or last ({flat_reads:?}); with no torch at all it is north_east ({control:?})"),
    )?;
    // (ironspec 09:58a (3)) the traffic through the as-built HELD T (powered, north_west): a departure from the NORTH turns
    // WEST; a WESTBOUND cart from the east passes WEST, not up the departure
    let (north_ok, west_ok) = join_traffic(version)?;
    pin(4, north_ok, "(4) through the powered T a cart from the NORTH (heading south) turns WEST".into())?;
    pin(5, west_ok, "(5) through the powered T a WESTBOUND cart from the east passes WEST (never up the north line)".into())?;
    if let Some(k) = plant()? {
        bail!("control planted nothing: MCTAS_RAIL_PLACE_PLANT={k} never failed its assert (--join runs asserts 3..5)");
    }
    println!("  every known answer held (asserts 3..5)");
    Ok(())
}

/// The traffic rows of `--join`: the T on its powered slime with a north line (-1,0,1..6), a west line (-6..-2,0,7) and
/// an east line (0..4,0,7), all on stone; a cart from the north end heading south at 0.4, then (alone) a cart from the
/// east end heading west at 0.4; each tracked 80 gt, where it ends printed. Returns (the north cart ended on the west
/// line, the eastern cart ended on the west line and never entered the north line)
fn join_traffic(version: &str) -> Result<(bool, bool)> {
    let mut srv = VanillaServer::start(&VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, tick_rate: 1000, ..Default::default() })?;
    srv.cmd_ok("gamerule spawn_mobs false")?;
    srv.cmd_ok("forceload add -16 -8 16 16")?;
    srv.cmd_ok(&format!("fill -16 {} -8 16 {} 16 minecraft:air", Y - 5, Y + 4))?;
    srv.step(20)?;
    let x = -1;
    srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:glass", Y - 3))?;
    srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:redstone_torch[lit=true]", Y - 2))?;
    srv.cmd_ok(&format!("setblock {x} {} 7 minecraft:slime_block", Y - 1))?;
    srv.cmd_ok(&format!("fill {x} {} 1 {x} {} 6 minecraft:stone", Y - 1, Y - 1))?;
    srv.cmd_ok(&format!("fill -6 {} 7 -2 {} 7 minecraft:stone", Y - 1, Y - 1))?;
    srv.cmd_ok(&format!("fill 0 {} 7 4 {} 7 minecraft:stone", Y - 1, Y - 1))?;
    srv.step(4)?;
    for z in 1..=6 {
        srv.cmd_ok(&format!("setblock {x} {Y} {z} minecraft:rail[shape=north_south,waterlogged=false]"))?;
        srv.step(1)?;
    }
    for xx in (-6..=-2).chain(0..=4) {
        srv.cmd_ok(&format!("setblock {xx} {Y} 7 minecraft:rail[shape=east_west,waterlogged=false]"))?;
        srv.step(1)?;
    }
    srv.cmd_ok(&format!("setblock {x} {Y} 7 minecraft:rail[shape=north_south,waterlogged=false]"))?;
    srv.step(10)?;
    let t_shape = shape(&mut srv, [x, Y, 7])?;
    println!("  traffic: the T (-1,0,7) on its lit torch reads {}", t_shape.unwrap_or("not a plain rail"));
    let track = |srv: &mut VanillaServer, tag: &str| -> Result<(Vec<[f64; 3]>, Option<[f64; 3]>)> {
        let mut path = Vec::new();
        for _ in 0..80 {
            srv.step(1)?;
            let out = srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?;
            let v: Vec<f64> = out.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
            if v.len() == 3 {
                path.push([v[0], v[1], v[2]]);
            }
        }
        let last = path.last().copied();
        Ok((path, last))
    };
    srv.cmd_ok(&format!("summon minecraft:minecart {:.1} {Y}.0625 1.5 {{Tags:[\"jn\"],Motion:[0.0d,0.0d,0.4d]}}", x as f64 + 0.5))?;
    let (pn, ln) = track(&mut srv, "jn")?;
    srv.cmd("kill @e[tag=jn]")?;
    srv.step(5)?;
    srv.cmd_ok(&format!("summon minecraft:minecart 4.5 {Y}.0625 7.5 {{Tags:[\"je\"],Motion:[-0.4d,0.0d,0.0d]}}"))?;
    let (pe, le) = track(&mut srv, "je")?;
    let _ = std::fs::remove_dir_all(&srv.dir);
    let fmt = |p: Option<[f64; 3]>| p.map(|q| format!("({:.4}, {:.4}, {:.4})", q[0], q[1] - Y as f64, q[2])).unwrap_or_else(|| "gone".into());
    // the T's cell is x -1 (its centre x -0.5): the west line's cells are x -6 .. -2, the north line's x -1 at z 1 .. 6
    let on_west = |p: Option<[f64; 3]>| p.is_some_and(|q| q[0] < -1.0 && (q[2] - 7.5).abs() < 0.01 && (q[1] - (Y as f64 + 0.0625)).abs() < 0.01);
    let entered_north = pe.iter().any(|q| q[2] < 7.0 && q[0] > -1.0 && q[0] < 0.0);
    println!("  traffic: the cart from the NORTH (heading south at 0.4) ends at {} after 80 gt ({} reads)", fmt(ln), pn.len());
    println!("  traffic: the WESTBOUND cart from the east (0.4) ends at {} after 80 gt; it entered the north line: {entered_north}", fmt(le));
    Ok((t_shape == Some("north_west") && on_west(ln), t_shape == Some("north_west") && on_west(le) && !entered_north))
}
