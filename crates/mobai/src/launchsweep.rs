//! (tickexact 2026-11-13, ironspec 12:38 (2), G1 12:29) The launch dense sweep on the official
//! server: the decisive scene's blocks only (no entities: empty lanes, no queue), optionally with
//! `--sbend-straight`'s cells (ironfarm 12:32: c3 and its support air, c2 east-west, C north-south,
//! an east backstop). Trials run three at a time, one per lane (x0 = -1, 3, 7): a napper (a minecart
//! with a villager) summoned on F (x0 - 1.5, 25.0625, phase) heading north at the STORED speed v,
//! braked to rest on R (x0 - 2, 24, 7); its release (the R line's repeater input (x0 - 2, 23, 9) set
//! to a redstone block); its end-of-tick positions until it falls (y < 23.9: into the drop) or
//! stalls (still above 23.9 40 gt after leaving the rails). Printed per trial: v, the phase, the
//! rest z on R, the off-rail entry (the first tick below a rail's 24.0625 after the release: C's
//! glass on today's path, over the drop cell from c2 on the straight one) and its horizontal step,
//! the outcome; per speed the worst rest and its margin to the launch bar.

use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

fn pos_of(srv: &mut VanillaServer, sel: &str) -> Result<Option<[f64; 3]>> {
    let out = srv.cmd_long(&format!("data get entity {sel} Pos"))?;
    let Some(list) = out.split("data: [").nth(1) else { return Ok(None) };
    let v: Vec<f64> = list.trim_end_matches(']').split(',').filter_map(|s| s.trim().trim_end_matches('d').parse().ok()).collect();
    Ok(if v.len() == 3 { Some([v[0], v[1] - DY as f64, v[2]]) } else { None })
}

struct Trial {
    v: f64,
    phase: f64,
    rest_z: Option<f64>,
    entry: Option<(u32, [f64; 3], f64)>,
    fell: Option<u32>,
    last: Option<[f64; 3]>,
}

pub fn launch_sweep(version: &str, scene: &std::path::Path, speeds: &[f64], phases: &[f64], sbend_straight: bool, bar: f64, trace: bool) -> Result<()> {
    let opts = VanillaOptions {
        version: version.to_string(),
        difficulty: "peaceful".into(),
        with_bot: false,
        biome: "minecraft:plains".into(),
        tick_rate: 1000,
        ..Default::default()
    };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok("forceload add -32 -32 31 31")?;
    srv.cmd_ok("time set 6000")?;
    srv.cmd_ok("gamerule advance_time false")?;
    crate::seatclear::place_blocks(&mut srv, scene)?;
    let lanes = [-1i32, 3, 7];
    if sbend_straight {
        for x0 in lanes {
            for (x, y, z, s) in [
                (x0 - 1, 24, 7, "minecraft:air"),
                (x0 - 1, 23, 7, "minecraft:air"),
                (x0 - 1, 24, 6, "minecraft:rail[shape=east_west,waterlogged=false]"),
                (x0, 24, 7, "minecraft:rail[shape=north_south,waterlogged=false]"),
                (x0 + 1, 24, 6, "minecraft:glass"),
            ] {
                srv.cmd_ok(&format!("setblock {x} {} {z} {s}", y + DY))?;
            }
            for (x, z, s) in [(x0 - 1, 6, "minecraft:rail[shape=east_west]"), (x0, 7, "minecraft:rail[shape=north_south]"), (x0 - 2, 6, "minecraft:rail[shape=south_east]")] {
                let ok = crate::strict::test(&mut srv, &format!("execute if block {x} {} {z} {s}", 24 + DY))?;
                anyhow::ensure!(ok, "--sbend-straight: ({x},24,{z}) is not {s} after the edit");
            }
        }
    }
    println!(
        "== launch sweep on the official {version} server: {} (blocks only, empty lanes){}; napper on F heading north at stored v, rests on R, released by its repeater's input; {} speeds x {} phases",
        scene.display(),
        if sbend_straight { ", --sbend-straight's cells (c3 + support air, c2 east-west, C north-south, east backstop): checked" } else { ", today's path" },
        speeds.len(),
        phases.len()
    );
    let mut queue: Vec<(f64, f64)> = Vec::new();
    for &v in speeds {
        for &ph in phases {
            queue.push((v, ph));
        }
    }
    let mut done: Vec<Trial> = Vec::new();
    for batch in queue.chunks(3) {
        let mut trials: Vec<Trial> = batch.iter().map(|&(v, phase)| Trial { v, phase, rest_z: None, entry: None, fell: None, last: None }).collect();
        for (i, tr) in trials.iter().enumerate() {
            let x = lanes[i] as f64 - 1.5;
            let out = srv.cmd(&format!(
                "summon minecraft:minecart {x} {} {} {{Tags:[\"ls{i}\"],Motion:[0.0d,0.0d,{:?}d],Passengers:[{{id:\"minecraft:villager\",Tags:[\"ls{i}p\"],PersistenceRequired:1b}}]}}",
                25.0625 + DY as f64,
                tr.phase,
                -tr.v
            ))?;
            anyhow::ensure!(out.contains("Summoned new"), "lane {}: {}", lanes[i], out.trim());
        }
        // to rest on R: 40 gt is ample (the slowest arrival rests in ~12)
        for t in 0..40u32 {
            srv.step(1)?;
            if trace {
                for i in 0..trials.len() {
                    let p = pos_of(&mut srv, &format!("@e[tag=ls{i},limit=1]"))?;
                    let m = srv.cmd_long(&format!("data get entity @e[tag=ls{i},limit=1] Motion"))?;
                    println!(
                        "    rest t{t} lane {}: {} motion {}",
                        lanes[i],
                        p.map(|p| format!("({:.4},{:.4},{:.4})", p[0], p[1], p[2])).unwrap_or("-".into()),
                        m.split("data: ").nth(1).unwrap_or("-").trim()
                    );
                }
            }
        }
        for (i, tr) in trials.iter_mut().enumerate() {
            tr.rest_z = pos_of(&mut srv, &format!("@e[tag=ls{i},limit=1]"))?.map(|p| p[2]);
        }
        for i in 0..trials.len() {
            srv.cmd_ok(&format!("setblock {} {} 9 minecraft:redstone_block", lanes[i] - 2, 23 + DY))?;
        }
        let mut prev: Vec<Option<[f64; 3]>> = vec![None; trials.len()];
        for t in 0..80u32 {
            srv.step(1)?;
            for (i, tr) in trials.iter_mut().enumerate() {
                let p = pos_of(&mut srv, &format!("@e[tag=ls{i},limit=1]"))?;
                if let Some(p) = p {
                    if tr.entry.is_none() && p[1] < 24.0625 - 1e-6 && p[1] > 23.9 {
                        let step = prev[i].map(|q| ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()).unwrap_or(f64::NAN);
                        tr.entry = Some((t, p, step));
                    }
                    if tr.fell.is_none() && p[1] < 23.9 {
                        tr.fell = Some(t);
                    }
                    tr.last = Some(p);
                }
                prev[i] = p;
            }
        }
        for (i, tr) in trials.iter().enumerate() {
            println!(
                "  lane {:>2}: stored v {:.3} phase z {:.2}: rest z {} | off-rail {} | {}",
                lanes[i],
                tr.v,
                tr.phase,
                tr.rest_z.map(|z| format!("{z:.4}")).unwrap_or("-".into()),
                tr.entry.map(|(t, p, s)| format!("+{t} gt at ({:.3},{:.3},{:.3}) step {s:.3}", p[0], p[1], p[2])).unwrap_or("never".into()),
                match (tr.fell, tr.last) {
                    (Some(f), _) => format!("FELL +{f} gt"),
                    (None, Some(l)) => format!("STALL: last ({:.3},{:.3},{:.3})", l[0], l[1], l[2]),
                    (None, None) => "LOST".into(),
                }
            );
        }
        for i in 0..trials.len() {
            srv.cmd_ok(&format!("tp @e[tag=ls{i}p] 0 {} 0", DY - 40))?;
            srv.cmd(&format!("kill @e[tag=ls{i}p]"))?;
            srv.cmd(&format!("kill @e[tag=ls{i}]"))?;
            srv.cmd_ok(&format!("setblock {} {} 9 minecraft:air", lanes[i] - 2, 23 + DY))?;
        }
        srv.cmd("kill @e[type=minecraft:item]")?;
        srv.step(10)?;
        done.extend(trials);
    }
    println!("  per speed (the worst rest over the phases and lanes; the launch bar z >= {bar:.3}):");
    let mut worst_all: Option<(f64, f64)> = None;
    for &v in speeds {
        let rows: Vec<&Trial> = done.iter().filter(|t| (t.v - v).abs() < 1e-9).collect();
        let worst = rows.iter().filter_map(|t| t.rest_z).fold(f64::MAX, f64::min);
        let stalls = rows.iter().filter(|t| t.fell.is_none()).count();
        let steps: Vec<f64> = rows.iter().filter_map(|t| t.entry.map(|e| e.2)).collect();
        let smin = steps.iter().cloned().fold(f64::MAX, f64::min);
        println!(
            "    v {v:.3}: {} trials, worst rest z {worst:.4} (margin {:+.4}), least entry step {smin:.3}, stalls {stalls}",
            rows.len(),
            worst - bar
        );
        if worst_all.is_none_or(|(_, w)| worst < w) {
            worst_all = Some((v, worst));
        }
    }
    let stalled: Vec<String> = done.iter().filter(|t| t.fell.is_none()).map(|t| format!("v {:.3} phase {:.2} rest {:.4}", t.v, t.phase, t.rest_z.unwrap_or(f64::NAN))).collect();
    println!(
        "  summary: {} trials, {} stall(s) {:?}; the worst rest z {} ",
        done.len(),
        stalled.len(),
        stalled,
        worst_all.map(|(v, w)| format!("{w:.4} at v {v:.3} (margin {:+.4} to {bar:.3})", w - bar)).unwrap_or("-".into())
    );
    let _ = std::fs::remove_dir_all(&srv.dir);
    Ok(())
}
