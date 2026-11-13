//! (tickexact 2026-11-12, ironspec: the pocket backs and the hold's stop) `mctas mobai rail-kick`:
//! on the official 26.3 server, one copy per (block, cart) 8 apart in x: a north_south powered rail
//! on stone, unpowered, the block at its north end (`--blocks`: slime, glass, stone...), a cart at
//! rest on it (empty, and with a villager); at `--power-at` a redstone block beside the rail (east)
//! powers it. Printed per copy: the cart's velocity the first tick after the power, and whether it
//! leaves the rail's cell within `--watch` gt (and where it is then). `--restarts`: rows with one
//! save/stop/start at these ticks before the power (the carts at rest).

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const Y: i32 = 64;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f', 's', 'b']).parse().ok()).collect()
}

fn opts(version: &str, world: Option<std::path::PathBuf>) -> VanillaOptions {
    VanillaOptions { version: version.to_string(), difficulty: "normal".into(), with_bot: false, keep_dir: true, tick_rate: 1000, world, ..Default::default() }
}

pub fn rail_kick(version: &str, blocks: &[String], power_at: u32, watch: u32, restarts: &[u32]) -> Result<()> {
    println!(
        "== rail-kick on the official {version} server: a north_south powered rail on stone, the block at its north end {blocks:?}, a cart at rest on it (empty / a villager); a redstone block east of the rail at t{power_at}; restarts at t{restarts:?} (before the power)"
    );
    let mut rows: Vec<Option<u32>> = vec![None];
    rows.extend(restarts.iter().map(|&r| Some(r)));
    for restart in rows {
        let mut srv = VanillaServer::start(&opts(version, None))?;
        srv.cmd_ok("gamerule spawn_mobs false")?;
        srv.cmd_ok("forceload add -16 -16 127 31")?;
        srv.step(20)?;
        let mut copies = Vec::new();
        for (bi, b) in blocks.iter().enumerate() {
            for (ci, ridden) in [false, true].into_iter().enumerate() {
                let x = (bi * 2 + ci) as i32 * 8;
                let z = 0;
                srv.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:stone", x - 1, Y - 1, z - 2, x + 1, Y - 1, z + 2))?;
                srv.cmd_ok(&format!("setblock {x} {Y} {z} minecraft:powered_rail[shape=north_south,powered=false]"))?;
                srv.cmd_ok(&format!("setblock {x} {Y} {} minecraft:{b}", z - 1))?;
                let tag = format!("rk{bi}_{ci}");
                let pass = if ridden { ",Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}]" } else { "" };
                srv.cmd_ok(&format!("summon minecraft:minecart {}.5 {}.0625 {}.5 {{Tags:[\"{tag}\"]{pass}}}", x, Y, z))?;
                copies.push((b.clone(), ridden, x, z, tag));
            }
        }
        let mut restarted = false;
        let mut reload_note: Option<crate::restartcheck::Reload> = None;
        let mut first: Vec<Option<[f64; 3]>> = vec![None; copies.len()];
        let mut left: Vec<Option<(u32, [f64; 3])>> = vec![None; copies.len()];
        for t in 1..=power_at + watch {
            if t == power_at {
                for (_, _, x, z, _) in &copies {
                    srv.cmd_ok(&format!("setblock {} {Y} {z} minecraft:redstone_block", x + 1))?;
                }
            }
            srv.step(1)?;
            for (i, (_, _, x, z, tag)) in copies.iter().enumerate() {
                if t < power_at {
                    continue;
                }
                let p = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Pos"))?);
                let m = nums(&srv.cmd_long(&format!("data get entity @e[tag={tag},limit=1] Motion"))?);
                if p.len() != 3 || m.len() != 3 {
                    continue;
                }
                if t == power_at {
                    first[i] = Some([m[0], m[1], m[2]]);
                }
                if left[i].is_none() && (p[0].floor() as i32 != *x || p[2].floor() as i32 != *z) {
                    left[i] = Some((t - power_at, [p[0], p[1], p[2]]));
                }
            }
            if let Some(r) = restart
                && !restarted
                && t == r
            {
                let probe = copies.first().map(|c| format!("@e[tag={},limit=1]", c.4));
                let (s, rl) = crate::restartcheck::reload(srv, |w| opts(version, Some(w)), probe.as_deref())?;
                srv = s;
                srv.cmd_ok("forceload add -16 -16 127 31")?;
                restarted = true;
                reload_note = Some(rl);
            }
        }
        println!(
            "  {}{}:",
            restart.map(|r| format!("a restart at t{r} (at rest)")).unwrap_or_else(|| "no restart".into()),
            reload_note.map(|x| format!(" ({})", x.describe())).unwrap_or_default()
        );
        for (i, (b, ridden, x, z, _)) in copies.iter().enumerate() {
            println!(
                "    north end {b}, {}: the first tick's velocity {}; {}",
                if *ridden { "a villager riding" } else { "empty" },
                first[i].map(|m| format!("({:+.4},{:+.4},{:+.4})", m[0], m[1], m[2])).unwrap_or_else(|| "-".into()),
                left[i]
                    .map(|(t, p)| format!("LEFT the cell at power + {t} ({:.3},{:.4},{:.3})", p[0] - *x as f64, p[1] - Y as f64, p[2] - *z as f64))
                    .unwrap_or_else(|| format!("stayed in its cell over {watch} gt"))
            );
        }
        let _ = std::fs::remove_dir_all(&srv.dir);
    }
    Ok(())
}
