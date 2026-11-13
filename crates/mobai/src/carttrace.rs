//! (tickexact 2026-11-12, the row-draws run's out-of-frame riders) `mctas mobai cart-trace`: on the
//! official 26.3 server, the composition (the module and its parts, rails as stated); a ridden cart
//! summoned at `--start` with `--motion`; every tick its position and motion, its rider's position,
//! and the block under the cart, for `--ticks` gt (or until both have stood still `--still` gt).

use crate::seatclear::{module_mismatches, place_part, start_world_part};
use anyhow::Result;
use mctas::vanilla::VanillaServer;

const DY: i32 = 80;

fn nums(s: &str) -> Vec<f64> {
    s.split("entity data: ").nth(1).unwrap_or("").trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|t| t.trim().trim_end_matches(['d', 'f']).parse().ok()).collect()
}

fn get(srv: &mut VanillaServer, sel: &str, what: &str) -> Result<Option<[f64; 3]>> {
    let v = nums(&srv.cmd_long(&format!("data get entity {sel} {what}"))?);
    Ok((v.len() == 3).then(|| [v[0], v[1], v[2]]))
}

#[allow(clippy::too_many_arguments)]
pub fn cart_trace(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], start: [f64; 3], motion: [f64; 3], ticks: u32, still: u32, probe: &[[i32; 3]], restart_at: &[u32]) -> Result<()> {
    println!(
        "== cart-trace on the official {version} server: {} with {}; a ridden cart from ({:.4},{:.4},{:.4}) at ({:+.4},{:+.4},{:+.4})",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        start[0],
        start[1],
        start[2],
        motion[0],
        motion[1],
        motion[2]
    );
    // `--module none`: no module (a bench world with no bed, laid as a part), the same server
    let mut srv = if module.as_os_str() == "none" {
        let jar = mctas::bridge::brain_agent(version)?;
        let mut s = VanillaServer::start(&mctas::vanilla::VanillaOptions {
            version: version.to_string(),
            difficulty: "normal".into(),
            with_bot: false,
            biome: "minecraft:plains".into(),
            tick_rate: 1000,
            keep_dir: true,
            java_agent: Some(std::path::PathBuf::from(format!("{}={}", jar.display(), crate::seatclear::MEMS))),
            ..Default::default()
        })?;
        s.cmd_ok("gamerule advance_time false")?;
        s.cmd_ok("gamerule spawn_mobs false")?;
        s.cmd_ok("time set 18000")?;
        s.cmd_ok("forceload add -16 -16 79 79")?;
        s.step(20)?;
        s
    } else {
        start_world_part(version, module, false, &[], None)?.0
    };
    for p in parts {
        place_part(&mut srv, p)?;
    }
    srv.step(10)?;
    let files: Vec<&std::path::Path> = std::iter::once(module).filter(|m| m.as_os_str() != "none").chain(parts.iter().map(|p| p.as_path())).collect();
    for f in &files {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) });
    }
    // the probed cells' blocks, as the game has them (each tested against the block ids the
    // composition can hold there: the file's state or air)
    let mut comp: std::collections::BTreeMap<[i32; 3], String> = std::collections::BTreeMap::new();
    for f in &files {
        for (p, s) in mctas::design::Design::load(f)?.blocks.iter() {
            comp.insert([p[0], p[1], p[2]], s.to_string());
        }
    }
    for c in probe {
        let s = comp.get(c).cloned().unwrap_or_else(|| "minecraft:air".into());
        let ok = srv.cmd_long(&format!("execute if block {} {} {} {s}", c[0], c[1] + DY, c[2]))?.contains("passed");
        println!("  probe ({},{},{}): the composition has {s}; the game {}", c[0], c[1], c[2], if ok { "agrees" } else { "DIFFERS" });
    }
    srv.cmd_ok(&format!(
        "summon minecraft:minecart {} {} {} {{Tags:[\"ct\"],Motion:[{}d,{}d,{}d],Passengers:[{{id:\"minecraft:villager\",NoAI:1b,Tags:[\"ctv\"]}}]}}",
        start[0],
        start[1] + DY as f64,
        start[2],
        motion[0],
        motion[1],
        motion[2]
    ))?;
    let mut last: Option<([f64; 3], [f64; 3])> = None;
    let mut quiet = 0;
    let mut prev_mo: Option<[f64; 3]> = None;
    // a restart needs the world kept across the stop: only the `--module none` server keeps it
    anyhow::ensure!(restart_at.is_empty() || module.as_os_str() == "none", "--restart-at needs --module none (the module's server deletes its world at the stop); lay the design with --parts");
    for t in 1..=ticks {
        // (7b90c3f1 / cc767b78 03:15 PT) a real save, stop and start before tick t, the cart as the
        // moving probe (restartcheck::reload: the game time's jump and whether the cart held)
        if restart_at.contains(&t) {
            let jar = mctas::bridge::brain_agent(version)?;
            let v = version.to_string();
            let (fresh, r) = crate::restartcheck::reload(
                srv,
                move |w| mctas::vanilla::VanillaOptions {
                    version: v,
                    difficulty: "normal".into(),
                    with_bot: false,
                    biome: "minecraft:plains".into(),
                    tick_rate: 1000,
                    keep_dir: true,
                    java_agent: Some(std::path::PathBuf::from(format!("{}={}", jar.display(), crate::seatclear::MEMS))),
                    world: Some(w),
                    ..Default::default()
                },
                Some("@e[tag=ct,limit=1]"),
            )?;
            srv = fresh;
            println!("  -- a world open before t{t}: {}", r.describe());
        }
        srv.step(1)?;
        let cart = get(&mut srv, "@e[tag=ct,limit=1]", "Pos")?;
        let mo = get(&mut srv, "@e[tag=ct,limit=1]", "Motion")?;
        let rider = get(&mut srv, "@e[tag=ctv,limit=1]", "Pos")?;
        let riding = srv.cmd_long("execute as @e[tag=ctv,limit=1] on vehicle if entity @s[tag=ct]")?.contains("passed");
        // the cell the cart's position is in holds water or a waterlogged block (what
        // updateFluidInteraction finds after the move, roughly: the flag itself isn't saved or
        // readable by command)
        let wet = ["minecraft:water", "minecraft:bubble_column"]
            .iter()
            .map(|b| srv.cmd_long(&format!("execute as @e[tag=ct,limit=1] at @s if block ~ ~ ~ {b}")).map(|o| o.contains("passed")))
            .collect::<Result<Vec<bool>>>()?
            .into_iter()
            .any(|x| x);
        let fmt = |p: Option<[f64; 3]>, dy: f64| p.map(|p| format!("({:.4},{:.4},{:.4})", p[0], p[1] - dy, p[2])).unwrap_or_else(|| "gone".into());
        let ratio = match (prev_mo, mo) {
            (Some(a), Some(b)) => {
                let (ha, hb) = ((a[0] * a[0] + a[2] * a[2]).sqrt(), (b[0] * b[0] + b[2] * b[2]).sqrt());
                if ha > 1e-6 { format!(" |vxz| x{:.4}", hb / ha) } else { String::new() }
            }
            _ => String::new(),
        };
        println!(
            "  t{t}: cart {} v {}{ratio} | its cell's fluid: {} | rider {}{}",
            fmt(cart, DY as f64),
            mo.map(|m| format!("({:+.4},{:+.4},{:+.4})", m[0], m[1], m[2])).unwrap_or_else(|| "-".into()),
            if wet { "water" } else { "none" },
            fmt(rider, DY as f64),
            if riding { "" } else { " NOT RIDING" }
        );
        prev_mo = mo;
        let now = (cart.unwrap_or([0.0; 3]), rider.unwrap_or([0.0; 3]));
        quiet = match last {
            Some(l) if (0..3).all(|i| (l.0[i] - now.0[i]).abs() < 1e-5 && (l.1[i] - now.1[i]).abs() < 1e-5) => quiet + 1,
            _ => 0,
        };
        last = Some(now);
        if cart.is_none() || quiet >= still {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
