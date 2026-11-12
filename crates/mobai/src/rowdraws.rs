//! (tickexact 2026-11-12, onechunk via farmopt: real partials or a simulator gap?) `mctas mobai
//! row-draws`: on the official 26.3 server, the composition (the module, its parts, rails as
//! stated, cells cleared) with the lead pair and F3's pillager, and riders (villagers with HOME on
//! the module's bed) in three groups, each respawned every `--every` gt: (i) at rest on lane 3's
//! S-bend R, its cart at (1.5, 24.65, 7.59); (ii) rolling east on the split row along z 9 from
//! x -2, at each of `--row-speeds`, 12 gt apart; (iii) dropped onto lanes -1 and 3's top slopes
//! as ironfarm traces the entry (a cart at rest at (x+0.6, 24.0, 6.49)). Every stroll draw: its
//! tick, the rider's group and cell, the target cell, the first path the sink computed for it
//! (complete or PARTIAL, its end cell) and how long WALK_TARGET was held.

use crate::seatclear::{draw_seq_moving, module_mismatches, place_part, start_world_part, Draw};
use anyhow::Result;
use mctas::vanilla::VanillaServer;
use std::collections::BTreeMap;

const DY: i32 = 80;

fn pos(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(rest) = out.split("entity data: ").nth(1) else { return Ok(None) };
    let v: Vec<f64> = rest.trim().trim_matches(|c| c == '[' || c == ']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok((v.len() == 3).then(|| [v[0], v[1] - DY as f64, v[2]]))
}

fn uuid_of(srv: &mut VanillaServer, sel: &str) -> Result<String> {
    let u = srv.cmd_long(&format!("data get entity {sel} UUID"))?;
    let ints: Vec<u32> = u.split("[I;").nth(1).unwrap_or("").trim_end_matches(|c: char| c == ']' || c.is_whitespace()).split(',').filter_map(|s| s.trim().parse::<i32>().ok()).map(|v| v as u32).collect();
    anyhow::ensure!(ints.len() == 4, "the UUID of {sel}: {u}");
    let hex = format!("{:08x}{:08x}{:08x}{:08x}", ints[0], ints[1], ints[2], ints[3]);
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

#[allow(clippy::too_many_arguments)]
pub fn row_draws(version: &str, module: &std::path::Path, parts: &[std::path::PathBuf], air: &[[i32; 3]], row_speeds: &[f64], every: u32, ticks: u32, out: Option<&std::path::Path>) -> Result<()> {
    println!(
        "== row-draws on the official {version} server: {} with {}; riders at lane 3's R, on the split row (east from x -2 at {row_speeds:?}, 12 gt apart) and dropped onto lanes -1 and 3's tops; respawned every {every} gt; {ticks} gt",
        module.display(),
        parts.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
    );
    let (mut srv, mut agent, bed) = start_world_part(version, module, false, &[], None)?;
    for p in parts {
        place_part(&mut srv, p)?;
    }
    for a in air {
        srv.cmd_ok(&format!("setblock {} {} {} minecraft:air", a[0], a[1] + DY, a[2]))?;
    }
    srv.step(10)?;
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        let m = module_mismatches(&mut srv, f)?;
        println!("  module-check {}: {} cells differ{}", f.display(), m.len(), if m.is_empty() { String::new() } else { format!(" (first: {})", m.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) });
    }
    agent.drain(&srv)?;
    // the riders: (group, its cart's start, its motion x, the tick offset in each period)
    let mut groups: Vec<(String, [f64; 3], f64, u32)> = vec![("R lane 3".into(), [1.5, 24.65, 7.59], 0.0, 0)];
    for (i, v) in row_speeds.iter().enumerate() {
        for (j, off) in [0u32, 12, 24].iter().enumerate() {
            groups.push((format!("row {v} #{j}"), [-1.5, 25.0625, 9.5], *v, *off + 40 * i as u32));
        }
    }
    for x in [-1, 3] {
        groups.push((format!("top lane {x}"), [x as f64 + 0.6, 24.0, 6.49], 0.0, 0));
    }
    let k = groups.len();
    let mut uuids: Vec<Option<String>> = vec![None; k];
    let mut idx: BTreeMap<String, usize> = BTreeMap::new();
    let mut ev: Vec<Vec<(u32, String)>> = vec![Vec::new(); k];
    let mut blocks: Vec<BTreeMap<u32, [i32; 3]>> = vec![BTreeMap::new(); k];
    for t in 0..ticks {
        let phase = t % every;
        for (g, (_, start, vx, off)) in groups.iter().enumerate() {
            if phase == *off {
                // (re)spawn: the old cart and rider go
                srv.cmd(&format!("kill @e[tag=rd{g}]"))?;
                srv.cmd(&format!("kill @e[tag=rv{g}]"))?;
                srv.cmd_ok(&format!(
                    "summon minecraft:minecart {} {} {} {{Tags:[\"rd{g}\"],Motion:[{vx}d,0.0d,0.0d],Passengers:[{{id:\"minecraft:villager\",Tags:[\"rv{g}\"],PersistenceRequired:1b,Brain:{{memories:{{\"minecraft:home\":{{value:{{dimension:\"minecraft:overworld\",pos:[I;{},{},{}]}}}}}}}}}}]}}",
                    start[0],
                    start[1] + DY as f64,
                    start[2],
                    bed[0],
                    bed[1] + DY,
                    bed[2]
                ))?;
                let u = uuid_of(&mut srv, &format!("@e[tag=rv{g},limit=1]"))?;
                idx.insert(u.clone(), g);
                uuids[g] = Some(u);
            }
        }
        srv.step(1)?;
        for l in agent.drain(&srv)? {
            if let Some(&g) = l.rsplit(" @").next().and_then(|u| idx.get(u.trim())) {
                // only lines of the group's current rider count (an old one is dead by now)
                if uuids[g].as_deref() == l.rsplit(" @").next().map(|u| u.trim()) {
                    ev[g].push((t, l));
                }
            }
        }
        for g in 0..k {
            if let Some(p) = pos(&mut srv, &format!("@e[tag=rv{g},limit=1]"))? {
                blocks[g].insert(t, [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]);
            }
        }
    }
    let mut log: Vec<String> = Vec::new();
    let mut summary: BTreeMap<String, (u32, u32, u32, u32, u32, u32)> = BTreeMap::new();
    for g in 0..k {
        let seq: Vec<Draw> = draw_seq_moving(&ev[g], 0, [0, 0, 0], &blocks[g]);
        for (j, d) in seq.iter().enumerate() {
            let b = blocks[g].get(&d.t).copied().unwrap_or([0, 0, 0]);
            // a rider whose cart has left the module's frame (x -6..9, z -7..9; nothing laid past
            // its east edge, the cart falls): its draws are not the farm's
            let outside = b[0] < -6 || b[0] > 9 || b[2] < -7 || b[2] > 9;
            let next_t = seq.get(j + 1).map(|n| n.t).unwrap_or(u32::MAX);
            // the draw's own path: the first path set to the drawn cell before the next draw (a
            // path to anything else, the REST bed walk taking WALK_TARGET over, is not this draw's)
            let want = d.cell.map(|c| format!("BlockPos{{x={}, y={}, z={}}}", b[0] + c[0], b[1] + c[1] + DY, b[2] + c[2]));
            let path = want.as_ref().and_then(|want| {
                ev[g].iter().find(|(t, l)| *t >= d.t && *t < next_t && l.starts_with("set minecraft:path") && l.contains(want.as_str())).map(|(t, l)| {
                    let p = l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("");
                    (t - d.t, p.contains("reach true"), p.to_string())
                })
            });
            // when it never pathed to its cell: the first other path set in its window (what took over)
            let other = ev[g].iter().find(|(t, l)| *t >= d.t && *t < next_t && l.starts_with("set minecraft:path")).map(|(t, l)| format!("+{} {}", t - d.t, l.split(" = ").nth(1).unwrap_or("").split(" by ").next().unwrap_or("")));
            let e = summary.entry(groups[g].0.clone()).or_default();
            e.0 += 1;
            if outside {
                e.5 += 1;
            }
            match d.cell {
                None => {
                    e.1 += 1;
                    log.push(format!("t{} {} rider cell ({},{},{}){} | null", d.t, groups[g].0, b[0], b[1], b[2], if outside { " OUTSIDE the module" } else { "" }));
                }
                Some(c) => {
                    if outside {
                        // counted apart, whatever its path
                    } else if let Some((_, whole, _)) = &path {
                        if *whole {
                            e.2 += 1;
                        } else {
                            e.3 += 1;
                        }
                    } else if other.is_some() {
                        e.4 += 1;
                    }
                    log.push(format!(
                        "t{} {} rider cell ({},{},{}){} | target ({},{},{}) {} hold {} | path {}",
                        d.t,
                        groups[g].0,
                        b[0],
                        b[1],
                        b[2],
                        if outside { " OUTSIDE the module" } else { "" },
                        b[0] + c[0],
                        b[1] + c[1],
                        b[2] + c[2],
                        d.outcome,
                        d.hold.map(|h| format!("{h} gt")).unwrap_or_else(|| "open".into()),
                        match &path {
                            Some((dt, whole, p)) => format!("+{dt} {} ({p})", if *whole { "complete" } else { "PARTIAL" }),
                            None => match &other {
                                Some(o) => format!("never pathed to its cell; replaced by another path {o}"),
                                None => "never pathed".into(),
                            },
                        }
                    ));
                }
            }
        }
    }
    for (g, (n, nulls, whole, partial, replaced, outside)) in &summary {
        println!(
            "  {g}: {n} draws, {nulls} null, {whole} landed with a complete path to the drawn cell, {partial} with a PARTIAL one, {replaced} replaced before pathing to it (another walk target's path came first){}",
            if *outside > 0 { format!("; {outside} drawn after the cart left the module's frame (not counted)") } else { String::new() }
        );
    }
    for l in log.iter().filter(|l| l.contains("PARTIAL") || l.contains("replaced by")).take(40) {
        println!("  {}: {l}", if l.contains("PARTIAL") { "partial" } else { "replaced" });
    }
    if let Some(o) = out {
        std::fs::write(o, log.join("\n") + "\n")?;
        println!("  the per-draw log: {} draws in {}", log.len(), o.display());
    }
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
