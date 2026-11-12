//! (tickexact 2026-11-12, ironspec's mirror rule: day-1's latch rows) `mctas mobai mirror-bench`: on
//! the official 26.3 server, a bench piece (a design file, e.g. a day-1 bench's `--out`) placed as
//! stated and, beside it, its copy mirrored in z (z' = `--mirror-at` - z, every state mirrored);
//! a drive script applied to both at the same game ticks (the mirrored copy at its mirrored cells,
//! with mirrored states); each read cell of both copies every tick, and every tick whose reads
//! differ between the copies (a rail's shape compared through the mirror).
//!
//! The script: `t:x,y,z=STATE;...` (at tick t, before that tick runs, set the cell; `air` for
//! minecraft:air), repeated shifted by each of `--phases`. Reads: `name:x,y,z:prop;...` with prop a
//! boolean property (powered, lit, open) or `shape`; only the read cells are compared, so the reads
//! must cover every output the controller consumes (a difference in an unread cell is not seen).
//! The copies must stand at least 16 apart in z (checked).

use crate::freeze::mirror_z;
use anyhow::Result;
use mctas::vanilla::{VanillaOptions, VanillaServer};

const DY: i32 = 80;

/// A rail shape seen through the z mirror.
fn mirror_shape(s: &str) -> String {
    match s {
        "north_east" => "south_east",
        "south_east" => "north_east",
        "north_west" => "south_west",
        "south_west" => "north_west",
        "ascending_north" => "ascending_south",
        "ascending_south" => "ascending_north",
        other => other,
    }
    .to_string()
}

/// One read cell's value in the game.
fn read(srv: &mut VanillaServer, id: &str, p: [i32; 3], prop: &str) -> Result<String> {
    if prop == "shape" {
        for s in ["north_south", "east_west", "north_east", "north_west", "south_east", "south_west", "ascending_north", "ascending_south", "ascending_east", "ascending_west"] {
            if srv.cmd_long(&format!("execute if block {} {} {} {id}[shape={s}]", p[0], p[1] + DY, p[2]))?.contains("passed") {
                return Ok(s.to_string());
            }
        }
        return Ok("?".into());
    }
    let on = srv.cmd_long(&format!("execute if block {} {} {} {id}[{prop}=true]", p[0], p[1] + DY, p[2]))?.contains("passed");
    Ok(if on { "1" } else { "0" }.into())
}

pub struct Event {
    pub t: u32,
    pub p: [i32; 3],
    pub state: String,
}

pub struct Read {
    pub name: String,
    pub p: [i32; 3],
    pub prop: String,
}

#[allow(clippy::too_many_arguments)]
pub fn mirror_bench(version: &str, file: &std::path::Path, mirror_at: i32, script: &[Event], reads: &[Read], phases: &[u32], ticks: u32) -> Result<()> {
    let d = mctas::design::Design::load(file)?;
    let mz = |p: [i32; 3]| [p[0], p[1], mirror_at - p[2]];
    println!("== mirror-bench on the official {version} server: {} as stated and mirrored in z (z' = {mirror_at} - z); {} events, phases {phases:?}, {ticks} gt each", file.display(), script.len());
    let (lo, hi) = (d.blocks.iter().map(|(p, _)| p[2]).min().unwrap_or(0), d.blocks.iter().map(|(p, _)| p[2]).max().unwrap_or(0));
    let (mlo, mhi) = (mirror_at - hi, mirror_at - lo);
    // the copies must not reach each other (redstone, the drive cells' neighbours): 16 blocks apart
    let (slo, shi) = (script.iter().map(|e| e.p[2]).chain([lo]).min().unwrap_or(lo), script.iter().map(|e| e.p[2]).chain([hi]).max().unwrap_or(hi));
    let (m_lo, m_hi) = (mirror_at - shi, mirror_at - slo);
    let gap = if m_hi < slo { slo - m_hi } else if m_lo > shi { m_lo - shi } else { 0 };
    anyhow::ensure!(gap >= 16, "the copies are {gap} apart in z (the piece and its drive cells z {slo}..{shi}, mirrored {m_lo}..{m_hi}): move --mirror-at so they are at least 16 apart");
    let opts = VanillaOptions { version: version.to_string(), difficulty: "peaceful".into(), with_bot: false, keep_dir: false, tick_rate: 1000, world: None, ..Default::default() };
    let mut srv = VanillaServer::start(&opts)?;
    srv.cmd_ok(&format!("forceload add -48 {} 47 {}", lo.min(mlo) - 8, hi.max(mhi) + 8))?;
    srv.step(20)?;
    let mut total_diff = 0u32;
    for &phase in phases {
        // a fresh build per phase: the stated copy and the mirrored one (supports first, then the
        // rest, rails strict as stated)
        for pass in 0..3 {
            for (p, s) in d.blocks.iter() {
                let rank = if s.contains("rail") { 2 } else if s.contains("stone") || s.contains("glass") || s.contains("slime") { 0 } else { 1 };
                if rank != pass {
                    continue;
                }
                let strict = if rank == 2 { " strict" } else { "" };
                srv.cmd(&format!("setblock {} {} {} {s}{strict}", p[0], p[1] + DY, p[2]))?;
                let m = mz([p[0], p[1], p[2]]);
                srv.cmd(&format!("setblock {} {} {} {}{strict}", m[0], m[1] + DY, m[2], mirror_z(s)))?;
            }
        }
        srv.step(40)?;
        let ids: Vec<String> = reads.iter().map(|r| d.blocks.get(&r.p).map(|s| s.split('[').next().unwrap_or("").to_string()).unwrap_or_else(|| "minecraft:air".into())).collect();
        let mut diffs = Vec::new();
        let mut rows = Vec::new();
        for t in 0..ticks {
            for e in script.iter().filter(|e| e.t + phase == t) {
                let s = if e.state == "air" { "minecraft:air".to_string() } else { e.state.clone() };
                srv.cmd(&format!("setblock {} {} {} {s}", e.p[0], e.p[1] + DY, e.p[2]))?;
                let m = mz(e.p);
                srv.cmd(&format!("setblock {} {} {} {}", m[0], m[1] + DY, m[2], mirror_z(&s)))?;
            }
            srv.step(1)?;
            let mut a = Vec::new();
            let mut b = Vec::new();
            for (r, id) in reads.iter().zip(&ids) {
                a.push(read(&mut srv, id, r.p, &r.prop)?);
                let v = read(&mut srv, id, mz(r.p), &r.prop)?;
                b.push(if r.prop == "shape" { mirror_shape(&v) } else { v });
            }
            let same = a == b;
            if !same {
                diffs.push(t);
            }
            rows.push(format!(
                "    t{t}: {}{}",
                reads.iter().zip(a.iter().zip(&b)).map(|(r, (x, y))| if x == y { format!("{} {x}", r.name) } else { format!("{} {x}|{y}", r.name) }).collect::<Vec<_>>().join(", "),
                if same { "" } else { "  <-- the copies differ" }
            ));
        }
        total_diff += diffs.len() as u32;
        println!("  phase {phase}: {} ticks where the mirrored copy differs from the stated one{}", diffs.len(), if diffs.is_empty() { String::new() } else { format!(": {diffs:?}") });
        // the ticks around each script event and each difference
        let keep: std::collections::BTreeSet<u32> = script.iter().flat_map(|e| (e.t + phase).saturating_sub(1)..e.t + phase + 12).chain(diffs.iter().flat_map(|&t| t.saturating_sub(2)..t + 3)).collect();
        for (t, row) in rows.iter().enumerate() {
            if keep.contains(&(t as u32)) {
                println!("{row}");
            }
        }
        // clear both copies for the next phase
        for (p, _) in d.blocks.iter() {
            srv.cmd(&format!("setblock {} {} {} minecraft:air", p[0], p[1] + DY, p[2]))?;
            let m = mz([p[0], p[1], p[2]]);
            srv.cmd(&format!("setblock {} {} {} minecraft:air", m[0], m[1] + DY, m[2]))?;
        }
        for e in script {
            srv.cmd(&format!("setblock {} {} {} minecraft:air", e.p[0], e.p[1] + DY, e.p[2]))?;
            let m = mz(e.p);
            srv.cmd(&format!("setblock {} {} {} minecraft:air", m[0], m[1] + DY, m[2]))?;
        }
        srv.step(20)?;
    }
    println!("  over {} phases: {total_diff} ticks where the mirrored copy differs", phases.len());
    Ok(())
}
