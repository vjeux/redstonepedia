//! Derived/saved world-data staleness probes (designs/briefs/corrupt-derived.md,
//! designs/corrupt-derived/NOTES.md). Each probe runs the UNMODIFIED official server on a
//! disposable void world, performs a sequence whose every game-visible step has a survival
//! equivalent (named in the brief), and reads the saved region files back — persistence
//! claims are only ever made from disk, across a real stop/restart cycle.

pub mod cli;

use anyhow::{Context, Result, bail};
use mctas::vanilla::nbt::Tag;
use mctas::vanilla::{VanillaOptions, VanillaServer, read_chunk_tag};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The dimension folder holding `region/` for a running server (26.x layout, legacy fallback —
/// the same resolution `VanillaServer::snapshot` uses).
fn dim_dir(server: &VanillaServer) -> PathBuf {
    let modern = server.dir.join(&server.level_name).join("dimensions").join("minecraft").join("overworld");
    if modern.exists() {
        modern
    } else {
        server.dir.join(&server.level_name)
    }
}

/// The saved chunk's NBT root from a dimension folder, with a fresh region cache.
fn chunk_tag(dim: &Path, cx: i32, cz: i32) -> Result<Tag> {
    let mut cache = HashMap::new();
    read_chunk_tag(dim, cx, cz, &mut cache)?.with_context(|| format!("chunk {cx},{cz} was never saved under {}", dim.display()))
}

/// One light nibble (0..15) of a saved chunk: sections carry `BlockLight`/`SkyLight` byte
/// arrays of 2048 bytes; cell index is `(y&15)<<8 | (z&15)<<4 | (x&15)`, even = low nibble
/// (the game's `DataLayer` layout). Missing section/array reads as 0.
fn light_at(tag: &Tag, layer: &str, x: i32, y: i32, z: i32) -> u8 {
    let section_y = y.div_euclid(16);
    for sec in tag.get("sections").map(|t| t.as_list()).unwrap_or(&[]) {
        if sec.get("Y").and_then(|t| t.as_i64()).unwrap_or(i64::MAX) as i32 != section_y {
            continue;
        }
        let data = sec.get(layer).map(|t| t.as_byte_array()).unwrap_or(&[]);
        if data.is_empty() {
            return 0;
        }
        let idx = ((y.rem_euclid(16) * 16 + z.rem_euclid(16)) * 16 + x.rem_euclid(16)) as usize;
        let byte = data[idx / 2] as u8;
        return if idx % 2 == 0 { byte & 0xf } else { byte >> 4 };
    }
    0
}

/// Aggregate over every saved nibble of one light layer in a chunk: (sum, cells > 0, cells == 15).
fn light_stats(tag: &Tag, layer: &str) -> (u64, u64, u64) {
    let (mut sum, mut lit, mut full) = (0u64, 0u64, 0u64);
    for sec in tag.get("sections").map(|t| t.as_list()).unwrap_or(&[]) {
        for &b in sec.get(layer).map(|t| t.as_byte_array()).unwrap_or(&[]) {
            for nib in [(b as u8) & 0xf, (b as u8) >> 4] {
                sum += nib as u64;
                lit += (nib > 0) as u64;
                full += (nib == 15) as u64;
            }
        }
    }
    (sum, lit, full)
}

fn start(version: &str, world: Option<PathBuf>, keep: bool) -> Result<VanillaServer> {
    let opts = VanillaOptions { version: version.to_string(), keep_dir: keep, world, view_distance: 3, ..Default::default() };
    let mut server = VanillaServer::start(&opts)?;
    // The harness starts with save-off; unload/autosave behavior is the subject here.
    server.cmd_ok("save-on")?;
    Ok(server)
}

/// Teleport the bot far away and step until the origin chunks have unloaded (their ticket
/// levels rise, `ChunkMap.processUnloads` saves + drops them during stepped ticks). 3000
/// blocks is far beyond the loaded halo at any view distance.
fn leave_and_unload(server: &mut VanillaServer, ticks: u32) -> Result<()> {
    server.teleport_bot(3000.5, 80.0, 3000.5)?;
    server.step(ticks)?;
    Ok(())
}

// ---------- P1: light backlog vs unload save ----------

/// One chunk column of sections (4096 blocks) per fill command; each block written
/// queues one `checkBlock` light task, and the engine applies at most 1000 queued tasks
/// per batch. Setup only: the survival equivalent is a piston light-lag machine doing
/// the same `setBlockState` writes (brief L1).
const LIGHT_Y: &str = "64 0 79"; // reused as "<x0> <y0> <z0> <x1> <y1> <z1>" pieces below

fn light_phase_stats(dim: &Path, cx: i32, cz: i32, label: &str) -> Result<(u64, u64, u64)> {
    let tag = chunk_tag(dim, cx, cz)?;
    let (sum, lit, full) = light_stats(&tag, "BlockLight");
    let center = light_at(&tag, "BlockLight", cx * 16 + 8, 72, cz * 16 + 8);
    println!("  {label} chunk {cx},{cz}: BlockLight sum={sum} lit={lit} full15={full} center={center} isLightOn={}", tag.get("isLightOn").and_then(|t| t.as_i64()).unwrap_or(0));
    Ok((sum, lit, full))
}

fn fill_box(server: &mut VanillaServer, cx: i32, cz: i32, block: &str) -> Result<()> {
    let (x, z) = (cx * 16, cz * 16);
    server.cmd_ok(&format!("fill {x} 64 {z} {} 79 {} {block}", x + 15, z + 15))?;
    Ok(())
}

pub fn probe_light(version: &str, fills: usize, chunk_radius: i32, leave_ticks: u32, keep: bool) -> Result<()> {
    let _ = LIGHT_Y;
    let mut verdict = Vec::new();
    for (name, settle_ticks) in [("control", 300u32), ("subject", 1u32)] {
        println!("[light/{name}] settle_ticks={settle_ticks} fills={fills} chunk_radius={chunk_radius} leave_ticks={leave_ticks}");
        let mut server = start(version, None, true)?;
        server.teleport_bot(8.5, 90.0, 8.5)?;
        server.step(10)?;
        for i in 0..fills {
            let block = if i % 2 == 0 { "minecraft:glowstone" } else { "minecraft:air" };
            let block = if i + 1 == fills { "minecraft:glowstone" } else { block };
            for cx in -chunk_radius..=chunk_radius {
                for cz in -chunk_radius..=chunk_radius {
                    fill_box(&mut server, cx, cz, block)?;
                }
            }
        }
        let t_after_fill = server.game_time()?;
        // Mid-light save arm: snapshot whatever the engine holds right now (the autosave
        // coincidence). Only read for the subject (control would read the same queue).
        if name == "subject" {
            server.cmd_ok("save-all flush")?;
            let dim = dim_dir(&server);
            light_phase_stats(&dim, 0, 0, &format!("immediate save-all at gametime {t_after_fill}"))?;
        }
        if settle_ticks > 0 {
            server.step(settle_ticks)?;
        }
        leave_and_unload(&mut server, leave_ticks)?;
        let dim = dim_dir(&server);
        let world = server.dir.join(&server.level_name);
        server.cmd_ok("save-all flush")?;
        let mut unloaded = (0, 0, 0);
        for cx in -chunk_radius..=chunk_radius {
            for cz in -chunk_radius..=chunk_radius {
                let s = light_phase_stats(&dim, cx, cz, "after unload")?;
                if (cx, cz) == (0, 0) {
                    unloaded = s;
                }
            }
        }
        drop(server);

        // Persistence: restart on the saved world, load the chunk, save again via snapshot
        // (its marker at y=-64 dirties the chunk without touching light at y>=64).
        let mut server = start(version, Some(world), keep)?;
        server.teleport_bot(8.5, 90.0, 8.5)?;
        server.step(60)?;
        server.snapshot([0, 60, 0], [15, 82, 15])?;
        let dim = dim_dir(&server);
        let reloaded = light_phase_stats(&dim, 0, 0, "after restart+load")?;
        // Poke: one light-differing change inside the volume re-propagates locally.
        server.cmd_ok("setblock 8 72 8 minecraft:air")?;
        server.cmd_ok("setblock 8 72 8 minecraft:glowstone")?;
        server.step(60)?;
        server.snapshot([0, 60, 0], [15, 82, 15])?;
        let poked = light_phase_stats(&dim, 0, 0, "after poke")?;
        if !keep {
            let _ = std::fs::remove_dir_all(&server.dir);
        }
        drop(server);
        verdict.push((name, unloaded, reloaded, poked));
    }
    let (c, s) = (&verdict[0], &verdict[1]);
    println!("[light] control chunk 0,0 lit cells after unload: {}", c.1.1);
    println!("[light] subject chunk 0,0 lit cells after unload: {} (after reload {}, after poke {})", s.1.1, s.2.1, s.3.1);
    if s.1.1 < c.1.1 {
        println!("[light] MARKER: subject saved {} fewer lit cells than control — unload snapshotted pending light; reload kept it ({}), one poke healed locally ({})", c.1.1 - s.1.1, s.2.1, s.3.1);
    } else {
        println!("[light] MARKER: no difference — the queue drained before the unload snapshot (batches also run inline while tasks are submitted, and applied light re-dirties the chunk; see NOTES for the measured fences)");
    }
    Ok(())
}

// ---------- P2: inhabited time rollback ----------

fn inhabited_of(dim: &Path, cx: i32, cz: i32) -> Result<i64> {
    let tag = chunk_tag(dim, cx, cz)?;
    Ok(tag.get("InhabitedTime").and_then(|t| t.as_i64()).unwrap_or(0))
}

pub fn probe_inhabited(version: &str, ticks: u32, keep: bool) -> Result<()> {
    let (subject, control) = ((3, 0), (-3, 0));
    let mut server = start(version, None, true)?;
    server.teleport_bot(8.5, 90.0, 8.5)?;
    server.step(20)?;
    // Baseline: a snapshot dirties (marker flip) and save-all writes both chunks with
    // their current inhabited time.
    server.snapshot([-48, 60, -16], [63, 82, 15])?;
    let dim = dim_dir(&server);
    let (b_subject, b_control) = (inhabited_of(&dim, subject.0, subject.1)?, inhabited_of(&dim, control.0, control.1)?);
    println!("[inhabited] baseline after first save: subject={b_subject} control={b_control}");
    server.step(ticks)?;
    // One block write in the control chunk only: it banks its grown inhabited time on the
    // next save; the subject chunk stays clean.
    server.cmd_ok("setblock -48 70 0 minecraft:stone")?;
    leave_and_unload(&mut server, 200)?;
    server.cmd_ok("save-all flush")?;
    let (f_subject, f_control) = (inhabited_of(&dim, subject.0, subject.1)?, inhabited_of(&dim, control.0, control.1)?);
    if !keep {
        let _ = std::fs::remove_dir_all(&server.dir);
    }
    drop(server);
    println!("[inhabited] after {ticks} spawning ticks + unload: subject={f_subject} (grew {}), control={f_control} (grew {})", f_subject - b_subject, f_control - b_control);
    if f_control - b_control >= ticks as i64 * 3 / 4 && f_subject - b_subject <= (ticks as i64) / 4 {
        println!("[inhabited] MARKER: control banked its growth, subject rolled back — increments persist only piggybacked on an unrelated chunk dirtying");
    } else {
        println!("[inhabited] MARKER: unexpected values — see line above (subject growth means something else dirtied it; control shortfall means spawning set or unload timing differs)");
    }
    if keep {
        println!("[inhabited] world kept at {}", dim.display());
    }
    Ok(())
}

// ---------- P4: heightmap audit (negative control) ----------

fn block_id(entry: &Tag) -> String {
    let s = mctas::vanilla::nbt::block_state_string(entry);
    let name = s.split('[').next().unwrap_or("?");
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

fn is_air(id: &str) -> bool {
    matches!(id, "air" | "cave_air" | "void_air")
}

/// Block id per cell of one section, decoded from the paletted container the same way
/// `mctas::vanilla`'s chunk reader does. Missing section reads as all air.
fn section_blocks(sec: &Tag) -> Vec<String> {
    let Some(bs) = sec.get("block_states") else { return vec!["air".to_string(); 4096] };
    let palette: Vec<String> = bs.get("palette").map(|t| t.as_list()).unwrap_or(&[]).iter().map(block_id).collect();
    let data = bs.get("data").map(|t| t.as_long_array()).unwrap_or(&[]);
    if palette.len() <= 1 || data.is_empty() {
        return vec![palette.first().cloned().unwrap_or_else(|| "air".to_string()); 4096];
    }
    let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(4) as usize;
    let per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;
    (0..4096)
        .map(|idx| {
            let word = data.get(idx / per_long).copied().unwrap_or(0) as u64;
            let v = ((word >> ((idx % per_long) * bits)) & mask) as usize;
            palette.get(v).cloned().unwrap_or_else(|| "?".to_string())
        })
        .collect()
}

/// Unpack one heightmap's 256 values (BitStorage, no long-boundary padding in 26.3's
/// `SimpleBitStorage` — entries may straddle longs, matching `Heightmap`'s storage).
fn unpack_heightmap(longs: &[i64], bits: usize) -> Vec<i64> {
    let mask = (1u64 << bits) - 1;
    (0..256)
        .map(|i| {
            let start = i * bits;
            let word = start / 64;
            let off = start % 64;
            let lo = longs.get(word).copied().unwrap_or(0) as u64 >> off;
            let hi = if off + bits > 64 { (longs.get(word + 1).copied().unwrap_or(0) as u64) << (64 - off) } else { 0 };
            ((lo | hi) & mask) as i64
        })
        .collect()
}

/// Resolve a user path to a dimension folder: the folder itself, a level folder
/// (containing `dimensions/minecraft/overworld`), or a server dir (level `world`).
fn resolve_dim(path: &Path) -> Result<PathBuf> {
    let candidates = [
        path.to_path_buf(),
        path.join("dimensions").join("minecraft").join("overworld"),
        path.join("world").join("dimensions").join("minecraft").join("overworld"),
        path.join("world"),
    ];
    for c in candidates {
        if c.join("region").is_dir() {
            return Ok(c);
        }
    }
    bail!("no region/ folder under {} (expected a dimension, level, or server dir)", path.display())
}

pub fn audit_heightmaps(world: &Path, min_y: i32, height: i32) -> Result<()> {
    let dim = resolve_dim(world)?;
    let bits = (32 - (height as u32).leading_zeros()) as usize; // ceil-log2(height+1) for 384 -> 9
    let mut cache = HashMap::new();
    let (mut chunk_count, mut columns, mut mismatches) = (0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    let mut regions: Vec<(i32, i32)> = Vec::new();
    for entry in std::fs::read_dir(dim.join("region"))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("r.").and_then(|s| s.strip_suffix(".mca")) {
            if let Some((x, z)) = rest.split_once('.') {
                regions.push((x.parse().unwrap_or(0), z.parse().unwrap_or(0)));
            }
        }
    }
    for (rx, rz) in regions {
        for cx in rx * 32..rx * 32 + 32 {
            for cz in rz * 32..rz * 32 + 32 {
                let Some(tag) = read_chunk_tag(&dim, cx, cz, &mut cache)? else { continue };
                let Some(stored_raw) = tag.get("Heightmaps").and_then(|h| h.get("WORLD_SURFACE")) else { continue };
                let stored = unpack_heightmap(stored_raw.as_long_array(), bits);
                // Scan blocks top-down per column.
                let mut highest = vec![min_y - 1; 256];
                let mut sections: Vec<(i32, Vec<String>)> = Vec::new();
                for sec in tag.get("sections").map(|t| t.as_list()).unwrap_or(&[]) {
                    let y = sec.get("Y").and_then(|t| t.as_i64()).unwrap_or(0) as i32;
                    sections.push((y, section_blocks(sec)));
                }
                sections.sort_by_key(|(y, _)| -*y);
                for (sy, blocks) in &sections {
                    for (idx, id) in blocks.iter().enumerate() {
                        if is_air(id) {
                            continue;
                        }
                        let col = (idx % 16) + ((idx / 16) % 16) * 16;
                        let y = sy * 16 + (idx / 256) as i32;
                        if y > highest[col] {
                            highest[col] = y;
                        }
                    }
                }
                chunk_count += 1;
                for col in 0..256 {
                    columns += 1;
                    let stored_highest = stored[col] + min_y as i64 - 1;
                    if stored_highest != highest[col] as i64 {
                        mismatches += 1;
                        if examples.len() < 5 {
                            examples.push(format!(
                                "chunk {cx},{cz} column {},{}: stored highest y={stored_highest}, scanned y={}",
                                col % 16,
                                col / 16,
                                highest[col]
                            ));
                        }
                    }
                }
            }
        }
    }
    println!("[audit] {}: {chunk_count} chunks, {columns} columns, {mismatches} WORLD_SURFACE mismatches", dim.display());
    for e in examples {
        println!("[audit]   {e}");
    }
    if mismatches == 0 {
        println!("[audit] MARKER: saved heightmaps agree with saved blocks everywhere (vanilla live code keeps them exact; only out-of-band changes can desync them, and load would trust the desync)");
    }
    Ok(())
}
