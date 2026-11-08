//! `mctas terrain gen`: the official 26.3 server generates an area of a seed's world once,
//! offline, and the saved chunks become an `.mcterrain` file (`redsim::terrain::format`).
//!
//! The server is the unmodified jar driven over RCON (`mctas::vanilla`), in a default world
//! (`minecraft:normal`, structures on). `forceload add` loads the area's chunks (a FORCED ticket
//! has the simulation flag: `TicketType.FORCED`, so they tick entities and the generated village
//! claims its beds and job sites); the area is the box plus a margin of chunks, so every kept
//! chunk had its neighbours generated and lit (`ChunkStatus` FULL, `isLightOn`). Mob spawning
//! stays off (the driver's game rules); `--days` runs the world that many game days with time
//! advancing (`tick sprint`), then everything is saved and read back.

use crate::region::{self, Chunk};
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::terrain::format::{Heightmap, Terrain};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// The overworld's height range (`DimensionType`: min y -64, height 384).
pub const MIN_Y: i32 = -64;
pub const HEIGHT: i32 = 384;

/// What to generate.
#[derive(Clone, Debug)]
pub struct GenOpts {
    pub version: String,
    pub seed: i64,
    /// The centre column of the kept box.
    pub center: [i32; 2],
    /// Half the kept box's side, in blocks.
    pub radius: i32,
    /// Chunks generated beyond the kept box on each side.
    pub margin_chunks: i32,
    /// Game days run after generation (time advancing, mob spawning off).
    pub days: u32,
    /// Keep the server's world folder (path printed).
    pub keep: bool,
}

/// `BiomeManager.obfuscateSeed`: Guava `Hashing.sha256().hashLong(seed).asLong()` (the seed's 8
/// little-endian bytes hashed; the first 8 bytes of the digest read little-endian).
pub fn zoom_seed(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(digest[..8].try_into().unwrap())
}

fn start(o: &GenOpts) -> Result<VanillaServer> {
    VanillaServer::start(&VanillaOptions {
        version: o.version.clone(),
        heap_mb: 6144,
        keep_dir: o.keep,
        default_world: true,
        seed: o.seed,
        frozen: false,
        with_bot: false,
        view_distance: 2,
        simulation_distance: 2,
        difficulty: "normal".into(),
        ..VanillaOptions::default()
    })
}

/// The kept box's chunk range (inclusive) and block box.
fn chunk_box(o: &GenOpts) -> ([i32; 2], [i32; 2]) {
    let (x0, x1) = (o.center[0] - o.radius, o.center[0] + o.radius - 1);
    let (z0, z1) = (o.center[1] - o.radius, o.center[1] + o.radius - 1);
    ([x0.div_euclid(16), z0.div_euclid(16)], [x1.div_euclid(16), z1.div_euclid(16)])
}

/// Forceload every chunk of the box and its margin, one row of chunks per command (the command
/// takes at most 256 chunks).
fn forceload(s: &mut VanillaServer, lo: [i32; 2], hi: [i32; 2]) -> Result<()> {
    for cz in lo[1]..=hi[1] {
        let mut cx = lo[0];
        while cx <= hi[0] {
            let end = (cx + 255).min(hi[0]);
            s.cmd_ok(&format!("forceload add {} {} {} {}", cx * 16, cz * 16, end * 16 + 15, cz * 16 + 15))?;
            cx = end + 1;
        }
    }
    Ok(())
}

/// Wait until every chunk of the range is saved at status FULL with its light on.
fn wait_generated(s: &mut VanillaServer, lo: [i32; 2], hi: [i32; 2]) -> Result<()> {
    let t0 = Instant::now();
    let dim = s.dim_dir("overworld");
    loop {
        s.cmd("save-all flush")?;
        let mut cache = HashMap::new();
        let mut missing = 0;
        for cx in lo[0]..=hi[0] {
            for cz in lo[1]..=hi[1] {
                match region::read_chunk(&dim, cx, cz, &mut cache, HEIGHT)? {
                    Some(c) if c.status == "minecraft:full" && c.light_on => {}
                    _ => missing += 1,
                }
            }
        }
        if missing == 0 {
            eprintln!("terrain: {} chunks generated in {:.0} s", (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1), t0.elapsed().as_secs_f64());
            return Ok(());
        }
        if t0.elapsed() > Duration::from_secs(1200) {
            bail!("{missing} chunks still not generated after 20 min");
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Run `days` game days with time advancing (`tick sprint`).
fn run_days(s: &mut VanillaServer, days: u32) -> Result<()> {
    if days == 0 {
        return Ok(());
    }
    let t0 = s.game_time()?;
    let ticks = days as i64 * 24000;
    s.cmd_ok("gamerule advance_time true")?;
    s.cmd_ok(&format!("tick sprint {ticks}"))?;
    let start = Instant::now();
    loop {
        let t = s.game_time()?;
        if t >= t0 + ticks {
            break;
        }
        if start.elapsed() > Duration::from_secs(3600) {
            bail!("the sprint of {ticks} ticks did not finish within an hour (at {})", t - t0);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    s.cmd_ok("gamerule advance_time false")?;
    eprintln!("terrain: ran {days} game days in {:.0} s", start.elapsed().as_secs_f64());
    Ok(())
}


/// Generate the area and read it back; the server's world folder is copied to `keep_world` when
/// given (the real-game checks start from that very world).
pub fn generate(o: &GenOpts, keep_world: Option<&std::path::Path>) -> Result<Terrain> {
    let mut s = start(o)?;
    let (clo, chi) = chunk_box(o);
    let (glo, ghi) = ([clo[0] - o.margin_chunks, clo[1] - o.margin_chunks], [chi[0] + o.margin_chunks, chi[1] + o.margin_chunks]);
    forceload(&mut s, glo, ghi)?;
    wait_generated(&mut s, glo, ghi)?;
    run_days(&mut s, o.days)?;
    let out = s.cmd("save-all flush")?;
    if !out.to_lowercase().contains("saved") {
        bail!("save-all flush: {out}");
    }
    // no more writes: the cache and the kept world are the same saved state (the server ran on
    // while the old code read the regions and copied the folder: villagers got into their beds in
    // between)
    s.cmd("save-off")?;
    let world = s.dir.join(&s.level_name);
    if let Some(dest) = keep_world {
        copy_world(&world, dest)?;
        eprintln!("terrain: world copied to {}", dest.display());
    }
    // the times are level.dat's (the saved state's), read by from_world
    from_world(&world, o)
}

/// Copy a saved world folder (no `session.lock`).
pub fn copy_world(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_world(&p, &q)?;
        } else if e.file_name() != "session.lock" {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}

/// Read the kept box of a saved world folder (one `gen` wrote, or its cached copy).
pub fn from_world(world: &std::path::Path, o: &GenOpts) -> Result<Terrain> {
    let modern = world.join("dimensions").join("minecraft").join("overworld");
    let dim = if modern.exists() { modern } else { world.to_path_buf() };
    let (clo, chi) = chunk_box(o);
    let mut t = assemble(&dim, o, clo, chi)?;
    t.seed = o.seed;
    t.zoom_seed = zoom_seed(o.seed);
    t.spawn = region::read_spawn(world)?;
    let (gt, dt) = region::read_times(world)?;
    t.game_time = gt;
    t.day_time = dt;
    t.notes = vec![
        ("generator".into(), format!("official server {} (mctas terrain gen)", o.version)),
        ("seed".into(), o.seed.to_string()),
        ("center".into(), format!("{} {}", o.center[0], o.center[1])),
        ("radius".into(), o.radius.to_string()),
        ("margin_chunks".into(), o.margin_chunks.to_string()),
        ("days".into(), o.days.to_string()),
    ];
    Ok(t)
}

/// Read the kept box (full height) from the saved dimension.
fn assemble(dim: &std::path::Path, o: &GenOpts, clo: [i32; 2], chi: [i32; 2]) -> Result<Terrain> {
    let origin = [o.center[0] - o.radius, MIN_Y, o.center[1] - o.radius];
    let size = [2 * o.radius, HEIGHT, 2 * o.radius];
    let mut cache = HashMap::new();
    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    for cx in clo[0]..=chi[0] {
        for cz in clo[1]..=chi[1] {
            let c = region::read_chunk(dim, cx, cz, &mut cache, HEIGHT)?.with_context(|| format!("chunk {cx},{cz} was not saved"))?;
            chunks.insert((cx, cz), c);
        }
    }
    // the biome cells the box reads reach one cell past it: the margin's ring of chunks
    let mut ring: HashMap<(i32, i32), Chunk> = HashMap::new();
    for cx in clo[0] - 1..=chi[0] + 1 {
        for cz in clo[1] - 1..=chi[1] + 1 {
            if !chunks.contains_key(&(cx, cz))
                && let Some(c) = region::read_chunk(dim, cx, cz, &mut cache, HEIGHT)?
            {
                ring.insert((cx, cz), c);
            }
        }
    }
    let top_section = (MIN_Y + HEIGHT).div_euclid(16);
    let mut t = Terrain { world_min_y: MIN_Y, world_height: HEIGHT, origin, size, ..Terrain::default() };
    let mut index: HashMap<String, u16> = HashMap::new();
    let n = (size[0] * size[1] * size[2]) as usize;
    t.cells.reserve(n);
    t.sky.reserve(n);
    t.block.reserve(n);
    let mut absent = 0usize;
    for y in origin[1]..origin[1] + size[1] {
        for z in origin[2]..origin[2] + size[2] {
            for x in origin[0]..origin[0] + size[0] {
                let c = &chunks[&(x.div_euclid(16), z.div_euclid(16))];
                let s = c.state(x, y, z);
                let id = match index.get(s) {
                    Some(&i) => i,
                    None => {
                        let i = t.palette.len() as u16;
                        t.palette.push(s.to_string());
                        index.insert(s.to_string(), i);
                        i
                    }
                };
                t.cells.push(id);
                let (sky, from_absent) = c.sky_light(x, y, z, top_section);
                absent += from_absent as usize;
                t.sky.push(sky);
                t.block.push(c.block_light(x, y, z));
            }
        }
    }
    eprintln!("terrain: {absent} sky light values read through absent layers (SkyLightSectionStorage.getLightValue)");
    for map in Heightmap::ALL {
        let mut v = Vec::with_capacity((size[0] * size[2]) as usize);
        for z in origin[2]..origin[2] + size[2] {
            for x in origin[0]..origin[0] + size[0] {
                let c = &chunks[&(x.div_euclid(16), z.div_euclid(16))];
                let h = c.heightmaps[map.index()].as_ref().with_context(|| format!("chunk at {x},{z} has no {} heightmap", map.key()))?;
                v.push((h[(x.rem_euclid(16) + 16 * z.rem_euclid(16)) as usize] + MIN_Y) as i16);
            }
        }
        t.heightmaps.push(v);
    }
    biomes(&mut t, &chunks, &ring)?;
    for c in chunks.values() {
        for s in &c.structures {
            if !t.structures.contains(s) {
                t.structures.push(s.clone());
            }
        }
    }
    t.structures.sort_by(|a, b| (a.id.as_str(), a.min).cmp(&(b.id.as_str(), b.min)));
    let mut ecache = HashMap::new();
    for cx in clo[0]..=chi[0] {
        for cz in clo[1]..=chi[1] {
            for e in region::read_entities(dim, cx, cz, &mut ecache)? {
                if (0..3).all(|a| e.pos[a] >= origin[a] as f64 && e.pos[a] < (origin[a] + size[a]) as f64) {
                    t.entities.push(e);
                }
            }
        }
    }
    t.entities.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    // the chunks' inhabited time and block entities inside the box (INHB, BENT)
    let mut keys: Vec<&(i32, i32)> = chunks.keys().collect();
    keys.sort();
    for k in keys {
        let c = &chunks[k];
        if (k.0 * 16 + 15) >= origin[0] && k.0 * 16 < origin[0] + size[0] && (k.1 * 16 + 15) >= origin[2] && k.1 * 16 < origin[2] + size[2] {
            t.inhabited.push(([k.0, k.1], c.inhabited));
        }
        for (p, snbt) in &c.block_entities {
            if (0..3).all(|a| p[a] >= origin[a] && p[a] < origin[a] + size[a]) {
                t.block_entities.push(redsim::terrain::format::BlockEntity { pos: *p, snbt: snbt.clone() });
            }
        }
    }
    t.block_entities.sort_by_key(|b| (b.pos[1], b.pos[2], b.pos[0]));
    Ok(t)
}

/// The biome cells the box reads (`Terrain::biome_box_for`); cells outside the kept chunks (the
/// box's 1-cell fringe) come from the margin chunks, which were generated too.
fn biomes(t: &mut Terrain, chunks: &HashMap<(i32, i32), Chunk>, ring: &HashMap<(i32, i32), Chunk>) -> Result<()> {
    let (bo, bs) = Terrain::biome_box_for(t.origin, t.size);
    t.biome_origin = bo;
    t.biome_size = bs;
    let mut index: HashMap<String, u16> = HashMap::new();
    for qy in bo[1]..bo[1] + bs[1] {
        for qz in bo[2]..bo[2] + bs[2] {
            for qx in bo[0]..bo[0] + bs[0] {
                let (bx, bz) = (qx * 4, qz * 4);
                // LevelChunk.getNoiseBiome clamps the quart y into the chunk's range
                let qyc = qy.clamp(MIN_Y / 4, (MIN_Y + HEIGHT) / 4 - 1);
                let key = (bx.div_euclid(16), bz.div_euclid(16));
                let name = match chunks.get(&key).or_else(|| ring.get(&key)) {
                    Some(c) => match c.sections.get(&(qyc * 4).div_euclid(16)) {
                        Some(s) => s.biome(qx.rem_euclid(4), qyc.rem_euclid(4), qz.rem_euclid(4)).to_string(),
                        None => "?".to_string(),
                    },
                    None => "?".to_string(),
                };
                let id = match index.get(&name) {
                    Some(&i) => i,
                    None => {
                        let i = t.biome_palette.len() as u16;
                        t.biome_palette.push(name.clone());
                        index.insert(name, i);
                        i
                    }
                };
                t.biomes.push(id);
            }
        }
    }
    Ok(())
}

/// `mctas terrain probe`: the game's own light reading (`LightPredicate`: `getMaxLocalRawBrightness`,
/// at noon so the sky darkens nothing) at cells of a saved world, by bisection on
/// `execute if predicate` location checks.
pub fn probe_light(version: &str, world: &std::path::Path, cells: &[[i32; 3]]) -> Result<Vec<u8>> {
    Ok(probe_light_at(version, world, cells, &[6000])?.remove(0))
}

/// `getMaxLocalRawBrightness` (the light with the sky's darkening) of cells at day times: per
/// time, per cell (a tick runs after each `time set` so the level's `skyDarken` follows).
pub fn probe_light_at(version: &str, world: &std::path::Path, cells: &[[i32; 3]], times: &[i64]) -> Result<Vec<Vec<u8>>> {
    let mut s = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        heap_mb: 4096,
        world: Some(world.to_path_buf()),
        default_world: true,
        frozen: true,
        with_bot: false,
        view_distance: 2,
        simulation_distance: 2,
        ..VanillaOptions::default()
    })?;
    for c in cells {
        s.cmd_ok(&format!("forceload add {} {}", c[0], c[2]))?;
    }
    let mut all = Vec::new();
    for t in times {
        s.cmd_ok(&format!("time set {t}"))?;
        s.step(1)?;
        let mut out = Vec::new();
        for c in cells {
            let (mut lo, mut hi) = (0u8, 15u8);
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                let r = s.cmd(&format!(
                    "execute positioned {} {} {} if predicate {{type:\"minecraft:location_check\",predicate:{{light:{{light:{{min:{mid}}}}}}}}}",
                    c[0] as f64 + 0.5,
                    c[1] as f64 + 0.5,
                    c[2] as f64 + 0.5
                ))?;
                if r.contains("Test passed") {
                    lo = mid;
                } else if r.contains("Test failed") {
                    hi = mid - 1;
                } else {
                    bail!("light probe at {c:?}: {r}");
                }
            }
            out.push(lo);
        }
        all.push(out);
    }
    Ok(all)
}

/// `mctas terrain biome-check`: redsim's `biome_at` (the noise biomes read back and
/// `BiomeManager.getBiome`'s zoom) against the game's `execute if biome` at seeded random cells
/// of the area; returns (cells, mismatches, examples).
pub fn biome_check(version: &str, world: &std::path::Path, t: &Terrain, samples: usize) -> Result<(usize, usize, Vec<String>)> {
    let w = redsim::terrain::build_world(t).map_err(anyhow::Error::msg)?;
    let mut s = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        heap_mb: 4096,
        world: Some(world.to_path_buf()),
        default_world: true,
        frozen: true,
        with_bot: false,
        view_distance: 2,
        simulation_distance: 2,
        ..VanillaOptions::default()
    })?;
    let mut r = redsim::rng::LegacyRandom::new(77);
    let (mut bad, mut ex) = (0, Vec::new());
    let mut n = 0;
    while n < samples {
        let x = t.origin[0] + 8 + r.next_int(t.size[0] - 16);
        let z = t.origin[2] + 8 + r.next_int(t.size[2] - 16);
        let y = t.origin[1] + r.next_int(t.size[1]);
        let p = redsim::pos::BlockPos::new(x, y, z);
        let Some(ours) = w.biome_at(p) else { continue };
        s.cmd_ok(&format!("forceload add {x} {z}"))?;
        let out = s.cmd(&format!("execute if biome {x} {y} {z} {ours}"))?;
        if !out.contains("Test passed") {
            bad += 1;
            if ex.len() < 10 {
                ex.push(format!("{p}: redsim {ours}, the game says {out}"));
            }
        }
        s.cmd_ok(&format!("forceload remove {x} {z}"))?;
        n += 1;
    }
    Ok((n, bad, ex))
}

/// `mctas terrain find`: the nearest structures and biomes of a seed's world (the game's own
/// `/locate`), from a point.
pub fn find(version: &str, seed: i64, from: [i32; 2], what: &[String]) -> Result<Vec<String>> {
    let o = GenOpts { version: version.to_string(), seed, center: from, radius: 16, margin_chunks: 0, days: 0, keep: false };
    let mut s = start(&o)?;
    let mut out = Vec::new();
    for w in what {
        let (kind, id) = w.split_once(':').filter(|(k, _)| *k == "structure" || *k == "biome").map(|(k, i)| (k.to_string(), i.to_string())).unwrap_or(("structure".into(), w.clone()));
        let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
        let r = s.cmd(&format!("execute positioned {} 64 {} run locate {kind} {id}", from[0], from[1]))?;
        out.push(format!("{kind} {id}: {r}"));
    }
    Ok(out)
}
