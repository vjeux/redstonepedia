//! `mctas mobfarm compare`: a mob farm run in the game and in redsim side by side, compared tick
//! for tick (track tickexact; docs/mechanics.md "The bridge's seeds"; `redsim::exact`).
//!
//! The game side is the bridge: a worker of its own (26.3 keeps the clock, the weather and the game
//! rules server-wide), one world sandbox seeded with the run's seed, every random in it seeded from
//! that seed (bridge `Exact.java`), so the run repeats. The farm is built the way redsim's runner
//! builds it (`farm::Run`): the blocks raw in the spec's order, every wire's shape, a `self` update
//! of every block, the judge's chest; `SETTLE_TICKS` ticks; the daylight detectors, the design's
//! entities (summoned seeded, each mob with the run's follow-range bonus) and each villager's HOME
//! (its bed's POI taken) and LAST_SLEPT; then the run. Every tick both sides report a frame
//! (`redsim::exact::Frame`) and the compare keeps the first difference, its context (the level
//! random's draw distance; with `--probe` the game's draws of that tick, by call site, from a
//! second, identical run) and the fields redsim does not model yet.

use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Worker, WorkerConfig};
use redsim::entity::EntityKind;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, SETTLE_TICKS};
use redsim::exact::{Compare, EntityFrame, Frame, Watch};
use serde_json::{Value, json};

/// A compare's options.
#[derive(Clone, Debug)]
pub struct Opts {
    pub seed: i64,
    /// Game ticks after the entities arrive.
    pub ticks: u32,
    pub weather: bool,
    /// On a difference, run again with the game's draw log around it.
    pub probe: bool,
    /// Ticks per traced step.
    pub batch: u32,
    /// Keep comparing after the first difference (to the end), to count the ticks run.
    pub full: bool,
    /// The bare scenario: no AFK player, the void's biome, `spawn_mobs` off (redsim without its
    /// AFK spot): the mob layer without natural spawning, fire spread or thunder.
    pub bare: bool,
    /// Print the game's draw log of this game time (every stream, by call site).
    pub draws_at: Option<i64>,
    /// A terrain sample (village-edge, plains, forest): the design built at its site.
    pub terrain: Option<String>,
    /// The day time the farm starts at (the steady state's when `None`).
    pub day0: Option<i64>,
}

/// The judge's run options with the compare's day time.
fn opts_of(opts: &Opts) -> RunOpts {
    let mut o = run_opts(opts.seed, opts.ticks);
    if let Some(d) = opts.day0 {
        o.day0 = d;
    }
    o
}

/// The game's side alone over `seeds` (no redsim): each run's custom spawner events seen as
/// entities entering (a siege's zombies, a patrol's pillagers, phantoms, cats, a trader), to find
/// a seed where one fires (the siege and patrol fixtures).
pub fn scan(version: &str, design: &std::path::Path, opts: &Opts, seeds: std::ops::RangeInclusive<i64>, want: &str) -> Result<Vec<(i64, i64, usize)>> {
    let seeds: Vec<i64> = seeds.collect();
    let results = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let Some(&seed) = seeds.get(i) else { break };
                    let o2 = Opts { seed, ..opts.clone() };
                    let r = scan_one(version, design, &o2, want);
                    match r {
                        Ok(Some((t, n))) => {
                            println!("seed {seed}: {n} {want} entered from game time {t}");
                            results.lock().unwrap().push((seed, t, n));
                        }
                        Ok(None) => println!("seed {seed}: no {want}"),
                        Err(e) => println!("seed {seed}: {e:#}"),
                    }
                }
            });
        }
    });
    Ok(results.into_inner().unwrap())
}

fn scan_one(version: &str, design: &std::path::Path, opts: &Opts, want: &str) -> Result<Option<(i64, usize)>> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start(version, &c, &spec, &o, opts.weather, false, place)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    game.trace(SETTLE_TICKS as u32, false)?;
    while run.settle_tick() {}
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    let mut first = None;
    let mut count = 0usize;
    let arrival = SETTLE_TICKS + 1;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, false)? {
            // after the arrival (the design's own mobs enter there)
            if f.t <= arrival {
                continue;
            }
            let entered: Vec<&redsim::exact::EntityFrame> = f.entities.iter().filter(|e| f.entered.contains(&e.n) && e.ty.contains(want)).collect();
            if !entered.is_empty() {
                first.get_or_insert(f.t);
                count += entered.len();
            }
        }
        left -= n;
    }
    Ok(first.map(|t| (t, count)))
}

/// The design and its spec where the compare runs: lifted in the void, moved to its site in a
/// terrain sample (the site and the sample leaked: `RunOpts::site` is `'static`).
pub fn load_place(design: &std::path::Path, opts: &Opts) -> Result<(circuit::Circuit, FarmSpec, Place<'static>)> {
    match &opts.terrain {
        None => {
            let c = load_lifted(design)?;
            let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
            Ok((c, spec, Place::Void { bare: opts.bare }))
        }
        Some(name) => {
            let c = circuit::Circuit::load(design)?;
            let spec = circuit::mobfarm::spec_of(&c).map_err(|e| anyhow!(e))?;
            let sample = redsim::terrain::samples::sample(name).ok_or_else(|| anyhow!("no terrain sample {name}"))?;
            let site: &'static redsim::entity::mob::site::Site = Box::leak(Box::new(circuit::mobfarm::site_of(&c, &spec, name).map_err(|e| anyhow!(e))?));
            let terrain: &'static redsim::terrain::format::Terrain = Box::leak(Box::new(redsim::terrain::format::Terrain::decode(&std::fs::read(sample.file)?).map_err(|e| anyhow!(e))?));
            eprintln!("terrain {}: offset {:?}, {} blocks placed, {} dug, AFK at {:.1} {:.1} {:.1}", site.sample, site.offset, site.placed, site.dug, site.afk.x, site.afk.y, site.afk.z);
            let moved = site.spec(&spec);
            Ok((c, moved, Place::Terrain { site, terrain }))
        }
    }
}

/// The follow-range bonus of each design mob (`Mob.finalizeSpawn`'s `random_spawn_bonus`), a run
/// input both sides take: `triangle(0, 0.11485)` draws from the run seed ^ "bonus", in summon order.
pub fn follow_bonuses(seed: i64, n: usize) -> Vec<f64> {
    let mut r = redsim::rng::LegacyRandom::new(seed ^ 0x626f_6e75_73);
    (0..n).map(|_| r.triangle(0.0, 0.114_850_000_000_000_01)).collect()
}

/// The chunk box a farm's world needs: its box's chunks.
pub fn chunk_box(spec: &FarmSpec) -> [i32; 4] {
    [spec.lo.x.div_euclid(16), spec.lo.z.div_euclid(16), spec.hi.x.div_euclid(16), spec.hi.z.div_euclid(16)]
}

/// The game's side of a farm run.
pub struct GameFarm {
    w: Worker,
    sb: u32,
    lo: [i32; 3],
    hi: [i32; 3],
    /// The chunks the level ticks, in its order (the forced box and the AFK player's simulation
    /// square), for redsim's chunk ticks.
    pub ticking: Vec<[i32; 2]>,
    /// The chunks it spawns in, before the shuffle (`ChunkMap.collectSpawningChunks`: the
    /// player's block-ticking ring within 128 blocks), and the natural spawn chunk count (the mob
    /// cap's divisor).
    pub spawning: Vec<[i32; 2]>,
    pub spawn_chunk_count: u32,
    /// The chunks where entities may spawn and tick (`ServerLevel.canSpawnEntitiesInChunk`).
    pub entity_ticking: Vec<[i32; 2]>,
    /// The design's block states at the spec's (moved) cells.
    text: std::collections::HashMap<[i32; 3], String>,
    /// The chunks loaded at full status (`hasChunksAt`).
    pub full: Vec<[i32; 2]>,
    /// The level runs the five custom spawners.
    pub spawners: bool,
}

/// The design's block states at the spec's cells (moved to the site in terrain).
fn design_text(c: &circuit::Circuit, spec: &FarmSpec, place: Place) -> std::collections::HashMap<[i32; 3], String> {
    let off = match place {
        Place::Terrain { site, .. } => site.offset,
        Place::Void { .. } => [0, 0, 0],
    };
    let _ = spec;
    c.clock.design.block_list().into_iter().map(|(p, s)| ([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s)).collect()
}

/// The cells a site clears for a void design (`site::Site::world_for` -> `place::clear`: every
/// non-air cell of the footprint's columns from the design's floor up, in its order).
fn cleared_cells(site: &redsim::entity::mob::site::Site, moved: &FarmSpec) -> Vec<[i32; 3]> {
    if !site.clear_box {
        return Vec::new();
    }
    let w = site.world.lock().map(|w| w.clone()).unwrap_or_else(|e| e.into_inner().clone());
    let (mut lo, mut hi) = (moved.blocks[0].0, moved.blocks[0].0);
    for (p, _) in &moved.blocks {
        lo = redsim::BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = redsim::BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    redsim::terrain::place::footprint_cells(&w, lo, hi).into_iter().filter(|p| !w.get(*p).is_air()).map(pos).collect()
}

/// Put a terrain sample into the sandbox (the bridge's `terrain` op: each section's states and
/// saved light, the biome cells; its entities stay out like redsim's site world), then check the
/// level's blocks and light against the sample's, cell for cell (an import that differs fails).
pub fn import_terrain(w: &mut Worker, sb: u32, t: &redsim::terrain::format::Terrain) -> Result<()> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let [ox, oy, oz] = t.origin;
    let [sx, sy, sz] = t.size;
    let top = oy + sy - 1;
    let mut sections = Vec::new();
    for cx in ox.div_euclid(16)..=(ox + sx - 1).div_euclid(16) {
        for cz in oz.div_euclid(16)..=(oz + sz - 1).div_euclid(16) {
            for cy in oy.div_euclid(16)..=top.div_euclid(16) {
                let mut palette: Vec<String> = vec!["minecraft:air".into()];
                let mut ids: std::collections::HashMap<String, u16> = std::collections::HashMap::from([("minecraft:air".to_string(), 0)]);
                let mut cells = Vec::with_capacity(8192);
                let mut sky = vec![0u8; 2048];
                let mut block = vec![0u8; 2048];
                for y in 0..16 {
                    for z in 0..16 {
                        for x in 0..16 {
                            let (wx, wy, wz) = (cx * 16 + x, cy * 16 + y, cz * 16 + z);
                            let state = t.state(wx, wy, wz).unwrap_or("minecraft:air");
                            let id = *ids.entry(state.to_string()).or_insert_with(|| {
                                palette.push(state.to_string());
                                (palette.len() - 1) as u16
                            });
                            cells.extend_from_slice(&id.to_le_bytes());
                            // outside the box: open sky above it, dark below it
                            let s = t.saved_sky(wx, wy, wz).unwrap_or(if wy > top { 15 } else { 0 });
                            let b = t.saved_block(wx, wy, wz).unwrap_or(0);
                            let i = ((y << 8) | (z << 4) | x) as usize;
                            sky[i >> 1] |= (s & 15) << ((i & 1) * 4);
                            block[i >> 1] |= (b & 15) << ((i & 1) * 4);
                        }
                    }
                }
                sections.push(json!({"x": cx, "y": cy, "z": cz, "palette": palette, "cells": b64.encode(&cells), "sky": b64.encode(&sky), "block": b64.encode(&block)}));
            }
        }
    }
    let biome_cells: Vec<u8> = t.biomes.iter().flat_map(|v| v.to_le_bytes()).collect();
    let n = sections.len();
    let structures: Vec<Value> = t.structures.iter().map(|s| json!({"id": s.id, "min": s.min, "max": s.max, "pieces": s.pieces})).collect();
    let inhabited: Vec<Value> = t.inhabited.iter().map(|([x, z], v)| json!([x, z, v])).collect();
    let block_entities: Vec<Value> = t.block_entities.iter().map(|b| json!({"pos": b.pos, "snbt": b.snbt})).collect();
    let v = w.call(json!({
        "op": "terrain", "sandbox": sb, "sections": sections,
        "biomes": {"origin": t.biome_origin, "size": t.biome_size, "palette": t.biome_palette, "cells": b64.encode(&biome_cells)},
        "structures": structures, "inhabited": inhabited, "block_entities": block_entities,
    }))?;
    eprintln!("terrain: {n} sections in {} chunks, {} structures, {} block entities, {} chunks' inhabited time", v["chunks"], v["structures"], v["block_entities"], t.inhabited.len());
    // the check: every cell's block and light as the level reads them
    let from = t.origin;
    let to = [ox + sx - 1, top, oz + sz - 1];
    let region = w.get_region(sb, from, to)?;
    let lb = w.call(json!({"op": "light_box", "sandbox": sb, "from": from, "to": to}))?;
    let sky = b64.decode(lb["sky"].as_str().unwrap_or(""))?;
    let blk = b64.decode(lb["block"].as_str().unwrap_or(""))?;
    let (mut bad_block, mut bad_sky, mut bad_light, mut first) = (0usize, 0usize, 0usize, None);
    // the sample's states as the game writes them back (its strings may leave out default
    // properties: "minecraft:deepslate" is deepslate[axis=y])
    let v = w.call(json!({"op": "normalize_states", "states": t.palette}))?;
    let canon: Vec<String> = v["states"].as_array().map(|a| a.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let mut i = 0usize;
    for y in oy..=top {
        for z in oz..oz + sz {
            for x in ox..ox + sx {
                let want = if t.contains(x, y, z) { canon[t.cells[t.index(x, y, z)] as usize].as_str() } else { "minecraft:air" };
                let got = region.get([x, y, z]).unwrap_or("?");
                if got != want {
                    bad_block += 1;
                    first.get_or_insert(format!("block {x} {y} {z}: game {got}, sample {want}"));
                }
                if t.saved_sky(x, y, z).is_some_and(|s| s != sky[i]) {
                    bad_sky += 1;
                    first.get_or_insert(format!("sky light {x} {y} {z}: game {}, sample {:?}", sky[i], t.saved_sky(x, y, z)));
                }
                if t.saved_block(x, y, z).is_some_and(|s| s != blk[i]) {
                    bad_light += 1;
                    first.get_or_insert(format!("block light {x} {y} {z}: game {}, sample {:?}", blk[i], t.saved_block(x, y, z)));
                }
                i += 1;
            }
        }
    }
    eprintln!("terrain import check: {} cells, {bad_block} blocks, {bad_sky} sky light and {bad_light} block light values differ from the sample", t.volume());
    if bad_block + bad_sky + bad_light > 0 {
        bail!("the terrain import differs from the sample: {}", first.unwrap_or_default());
    }
    Ok(())
}

fn pos(p: redsim::BlockPos) -> [i32; 3] {
    [p.x, p.y, p.z]
}

/// The real-game gate's AFK spot of a void farm (`circuit::mobfarm_game`): 60 blocks under the
/// chest, a stone over its head (redsim's `World::afk` is the same).
pub fn afk_spot(spec: &FarmSpec) -> [i32; 3] {
    [spec.output.x, spec.lo.y - 60, spec.output.z]
}

/// The gate's forced chunks of a void farm: its box and 8 blocks around (`forceload add`).
pub fn forced_chunks(spec: &FarmSpec) -> [i32; 4] {
    [(spec.lo.x - 8).div_euclid(16), (spec.lo.z - 8).div_euclid(16), (spec.hi.x + 8).div_euclid(16), (spec.hi.z + 8).div_euclid(16)]
}

/// Where a compare runs.
#[derive(Clone, Copy)]
pub enum Place<'a> {
    /// The void, in the real-game gate's scenario (`bare`: without its player, biome and
    /// `spawn_mobs`); the design lifted by [`LIFT`].
    Void { bare: bool },
    /// A terrain sample: the design moved to its site (`redsim::entity::mob::site`), natural
    /// spawning on Hard around its AFK spot.
    Terrain { site: &'a redsim::entity::mob::site::Site, terrain: &'a redsim::terrain::format::Terrain },
}

impl GameFarm {
    /// A fresh worker and world sandbox, with the farm built and its chest placed (before the
    /// settle). The void (`Place::Void`): the real-game gate's scenario (`circuit::mobfarm_game`: a
    /// plains void, Hard, `spawn_mobs` on and natural monsters off, phantoms, patrols and traders
    /// off, the clock and (with `weather`) the weather running, the AFK player creative under a
    /// stone 60 blocks under the chest, simulation and view distance 6); `bare`: no player, the
    /// void's biome, `spawn_mobs` off (the mob layer seen without natural spawning, fire spread or
    /// thunder). A terrain site: the sample imported ([`import_terrain`]), the design's box
    /// cleared like the site clears it, natural monsters on, the AFK player at the site's spot.
    pub fn start(version: &str, c: &circuit::Circuit, spec: &FarmSpec, o: &RunOpts, weather: bool, trace_randoms: bool, place: Place) -> Result<GameFarm> {
        Self::start_with(version, c, spec, o, weather, trace_randoms, place, true)
    }

    /// [`GameFarm::start`]; `spawners`: the level runs the five custom spawners (the real game's
    /// overworld does; the void gate's rules leave the cats, terrain's every one).
    #[allow(clippy::too_many_arguments)]
    pub fn start_with(version: &str, c: &circuit::Circuit, spec: &FarmSpec, o: &RunOpts, weather: bool, trace_randoms: bool, place: Place, spawners: bool) -> Result<GameFarm> {
        let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 4096, ..Default::default() })?;
        let (chunks, biome, bare) = match place {
            Place::Void { bare } => (forced_chunks(spec), if bare { Value::Null } else { json!("minecraft:plains") }, bare),
            Place::Terrain { terrain, .. } => {
                let [ox, _, oz] = terrain.origin;
                let [sx, _, sz] = terrain.size;
                ([ox.div_euclid(16), oz.div_euclid(16), (ox + sx - 1).div_euclid(16), (oz + sz - 1).div_euclid(16)], json!("minecraft:plains"), false)
            }
        };
        let spawners = spawners && !bare;
        // a terrain sample's sandbox reports its world's sea level (the overworld's 63)
        let sea_level = if matches!(place, Place::Terrain { .. }) { json!(63) } else { Value::Null };
        let v = w.call(json!({"op": "create", "chunks": chunks, "seed": o.seed, "biome": biome, "custom_spawners": spawners, "sea_level": sea_level}))?;
        let sb = v["sandbox"].as_u64().ok_or_else(|| anyhow!("create: no sandbox"))? as u32;
        let mut world = json!({
            "op": "world", "sandbox": sb,
            "advance_time": true, "day_time": o.day0 - SETTLE_TICKS,
            "advance_weather": weather, "spawn_mobs": !bare, "spawn_monsters": false,
            "rules": {"spawn_phantoms": "false", "spawn_patrols": "false", "spawn_wandering_traders": "false"},
            "difficulty": "hard",
            "trace_randoms": trace_randoms,
        });
        match place {
            Place::Void { bare: false } => {
                let bot = afk_spot(spec);
                w.set_blocks(sb, &[([bot[0], bot[1] + 3, bot[2]], "minecraft:stone")], PlaceMode::Raw)?;
                world["player"] = json!({"pos": [bot[0] as f64 + 0.5, bot[1] as f64, bot[2] as f64 + 0.5], "mode": "creative", "simulation": 6, "view": 6});
            }
            Place::Void { bare: true } => {}
            Place::Terrain { site, terrain } => {
                import_terrain(&mut w, sb, terrain)?;
                let cleared = cleared_cells(site, spec);
                let air: Vec<([i32; 3], &str)> = cleared.iter().map(|p| (*p, "minecraft:air")).collect();
                for part in air.chunks(4096) {
                    w.set_blocks(sb, part, PlaceMode::Raw)?;
                }
                world["spawn_monsters"] = json!(true);
                // the game's default rules: every custom spawner may fire
                world["rules"] = json!({"spawn_phantoms": "true", "spawn_patrols": "true", "spawn_wandering_traders": "true"});
                world["player"] = json!({"pos": [site.afk.x, site.afk.y, site.afk.z], "mode": "creative", "simulation": 6, "view": 6});
            }
        }
        let v = w.call(world).context("world sandbox")?;
        let full = v["full_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let ticking = v["ticking_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let spawning = v["spawning_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let entity_ticking = v["entity_ticking_chunks"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
        let spawn_chunk_count = v["spawn_chunk_count"].as_u64().unwrap_or(0) as u32;
        // the blocks in the spec's order (redsim's: sorted by x, y, z), the detectors held back
        let text = design_text(c, spec, place);
        let mut rest = Vec::new();
        for (p, b) in &spec.blocks {
            if matches!(b, redsim::block::Block::DaylightDetector { .. }) {
                continue;
            }
            let s = text.get(&pos(*p)).ok_or_else(|| anyhow!("no design block at {p}"))?;
            rest.push((pos(*p), s.clone()));
        }
        let mut g = GameFarm { w, sb, lo: pos(spec.lo), hi: pos(spec.hi), ticking, spawning, entity_ticking, spawn_chunk_count, text, full, spawners };
        g.place(&rest)?;
        let out = pos(spec.output);
        let here = g.w.get_region(sb, out, out)?;
        if !here.get(out).is_some_and(|s| s.starts_with("minecraft:chest")) {
            g.w.set_blocks(sb, &[(out, "minecraft:chest[facing=north,type=single,waterlogged=false]")], PlaceMode::Raw)?;
        }
        g.w.merge_block_entity(sb, out, "{Items:[]}")?;
        // the lists again, right before the first tick (the farm built, nothing ticked since)
        let v = g.w.call(json!({"op": "chunk_lists", "sandbox": sb}))?;
        let list = |k: &str| -> Vec<[i32; 2]> { v[k].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default() };
        g.ticking = list("ticking_chunks");
        g.spawning = list("spawning_chunks");
        g.entity_ticking = list("entity_ticking_chunks");
        g.full = list("full_chunks");
        g.spawn_chunk_count = v["spawn_chunk_count"].as_u64().unwrap_or(0) as u32;
        eprintln!("chunk lists at the first tick: {} ticking, {} spawning, {} entity ticking, {} loaded, spawn chunk count {}", g.ticking.len(), g.spawning.len(), g.entity_ticking.len(), g.full.len(), g.spawn_chunk_count);
        if std::env::var("MCTAS_EXACT_LISTS").is_ok() {
            eprintln!("entity ticking: {:?}", g.entity_ticking);
            eprintln!("ticking: {:?}", g.ticking);
        }
        Ok(g)
    }

    /// The judge's placement: raw writes in order, each wire's shape, a `self` update of each.
    fn place(&mut self, blocks: &[([i32; 3], String)]) -> Result<()> {
        self.w.set_blocks(self.sb, blocks, PlaceMode::Raw)?;
        let wires: Vec<[i32; 3]> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
        if !wires.is_empty() {
            self.w.update(self.sb, &wires, "shape")?;
        }
        let all: Vec<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
        self.w.update(self.sb, &all, "self")?;
        Ok(())
    }

    /// `ticks` traced ticks (with the draw log when `draws`).
    pub fn trace(&mut self, ticks: u32, draws: bool) -> Result<Vec<Frame>> {
        let v = self.w.call(json!({"op": "trace", "sandbox": self.sb, "ticks": ticks, "draws": draws, "box": {"from": self.lo, "to": self.hi}}))?;
        v["frames"].as_array().ok_or_else(|| anyhow!("trace: no frames"))?.iter().map(parse_frame).collect()
    }

    /// `Run::arrive`'s game side, after redsim's: the detectors, the entities (with redsim's
    /// summon order and the run's follow-range bonuses), each villager's HOME and LAST_SLEPT from
    /// redsim's world.
    pub fn arrive(&mut self, _c: &circuit::Circuit, spec: &FarmSpec, ours: &redsim::world::World, bonus: &[f64]) -> Result<()> {
        let detectors: Vec<([i32; 3], String)> = spec
            .blocks
            .iter()
            .filter(|(_, b)| matches!(b, redsim::block::Block::DaylightDetector { .. }))
            .map(|(p, _)| (pos(*p), self.text.get(&pos(*p)).cloned().unwrap_or_default()))
            .collect();
        if !detectors.is_empty() {
            self.place(&detectors)?;
        }
        let mut k = 0;
        let mut specs = Vec::new();
        for (ty, p, nbt) in &spec.entities {
            let mobs_here = 1 + nbt.as_deref().map(|n| n.matches("id:").count()).unwrap_or(0);
            let mut b = Vec::new();
            for _ in 0..mobs_here {
                b.push(bonus.get(k).copied());
                k += 1;
            }
            specs.push(json!({"type": ty, "pos": p, "nbt": nbt, "follow_bonus": b}));
        }
        let v = self.w.call(json!({"op": "summon", "sandbox": self.sb, "mobs": true, "entities": specs}))?;
        let game_ids: Vec<u64> = v["net_ids"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect()).unwrap_or_default();
        let first_mob = ours.entities.list.len().saturating_sub(game_ids.len());
        let ours_ids: Vec<u64> = ours.entities.list[first_mob..].iter().map(|e| e.id as u64).collect();
        if game_ids != ours_ids {
            eprintln!("note: the design's entities entered the game as {game_ids:?}, redsim as {ours_ids:?} (the worlds already differ)");
        }
        for (e, gid) in ours.entities.list[first_mob..].iter().zip(game_ids.iter()) {
            if let EntityKind::Mob(m) = &e.kind
                && let Some(b) = m.brain.as_deref()
            {
                let mut mem = serde_json::Map::new();
                if let Some(h) = b.home {
                    mem.insert("home".into(), json!([h.x, h.y, h.z]));
                }
                if let Some(t) = b.last_slept {
                    mem.insert("last_slept".into(), json!(t));
                }
                self.w.call(json!({"op": "memory", "sandbox": self.sb, "entity": gid, "memories": mem, "take_poi": true}))?;
            }
        }
        Ok(())
    }
}

/// A bridge frame (`trace` op) as a `redsim::exact::Frame`, its block states in redsim's text.
pub fn parse_frame(v: &Value) -> Result<Frame> {
    let i = |k: &str| v[k].as_i64();
    let mut f = Frame { t: i("t").ok_or_else(|| anyhow!("frame without t"))?, day: i("day"), rng: i("rng"), rv: i("rv").map(|x| x as i32), weather: v["weather"].as_str().map(str::to_string), ..Default::default() };
    for e in v["entities"].as_array().into_iter().flatten() {
        f.entities.push(parse_entity(e)?);
    }
    let ids = |k: &str| -> Vec<u32> { v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as u32).collect()).unwrap_or_default() };
    f.entered = ids("entered");
    f.left = ids("left");
    for b in v["blocks"].as_array().into_iter().flatten() {
        let p = [b[0].as_i64().unwrap_or(0) as i32, b[1].as_i64().unwrap_or(0) as i32, b[2].as_i64().unwrap_or(0) as i32];
        let s = b[3].as_str().unwrap_or("");
        let text = redsim::block::Block::parse(s).map(|x| x.to_string()).unwrap_or_else(|_| s.to_string());
        f.blocks.push((p, text));
    }
    for d in v["draws"].as_array().into_iter().flatten() {
        f.draws.push((d[0].as_str().unwrap_or("").to_string(), d[1].as_str().unwrap_or("").to_string(), d[2].as_u64().unwrap_or(0) as u32));
        f.draw_values.push(d[3].as_array().map(|a| a.iter().map(|x| x.as_i64().unwrap_or(0) as i32).collect()).unwrap_or_default());
    }
    f.spawning_shuffled = v["spawning_shuffled"].as_array().map(|a| a.iter().map(|c| [c[0].as_i64().unwrap_or(0) as i32, c[1].as_i64().unwrap_or(0) as i32]).collect()).unwrap_or_default();
    Ok(f)
}

fn parse_entity(e: &Value) -> Result<EntityFrame> {
    let v3 = |k: &str| -> [f64; 3] { [e[k][0].as_f64().unwrap_or(f64::NAN), e[k][1].as_f64().unwrap_or(f64::NAN), e[k][2].as_f64().unwrap_or(f64::NAN)] };
    let f32s = |k: &str| e[k].as_str().and_then(|s| s.parse::<f32>().ok());
    let s = |k: &str| e[k].as_str().map(str::to_string);
    let i = |k: &str| e[k].as_i64().map(|x| x as i32);
    let memories = e["memories"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect());
    Ok(EntityFrame {
        n: e["n"].as_u64().ok_or_else(|| anyhow!("entity without n"))? as u32,
        ty: e["type"].as_str().unwrap_or("?").to_string(),
        pos: v3("pos"),
        motion: v3("motion"),
        yaw: f32s("yaw").unwrap_or(f32::NAN),
        pitch: f32s("pitch").unwrap_or(f32::NAN),
        on_ground: e["on_ground"].as_bool().unwrap_or(false),
        fire: i("fire"),
        air: i("air"),
        pose: s("pose"),
        vehicle: s("vehicle"),
        random: e["random"].as_i64(),
        item: s("item"),
        age: i("age"),
        head: f32s("head"),
        body: f32s("body"),
        health: f32s("health"),
        hurt_time: i("hurt_time"),
        death_time: i("death_time"),
        no_action: i("no_action"),
        memories,
        modeled_memories: None,
        all_memories: false,
        activities: s("activities"),
        behaviors: s("behaviors"),
        goals: s("goals"),
        path: s("path"),
        target: s("target"),
    })
}

/// How high the exact compare lifts a void design (both sides): the gate's AFK spot, 60 blocks
/// under the farm, must lie inside the world (y -64..320).
pub const LIFT: i32 = 100;

/// A design, lifted by [`LIFT`].
pub fn load_lifted(design: &std::path::Path) -> Result<circuit::Circuit> {
    circuit::Circuit::load(design)?.translated([0, LIFT, 0])
}

/// The judge's run options for the exact compare: the steady state of `seed`, golems standing
/// still (the walking policies are the judge's robustness input, not the game's AI).
pub fn run_opts(seed: i64, ticks: u32) -> RunOpts {
    redsim::entity::mob::farm::judge_opts(seed, ticks, redsim::entity::mob::farm::WalkMode::Fixed(redsim::entity::mob::walk::Walk::Still))
}

/// redsim's side: the runner with the exact streams on (the level random mirrored on the
/// sandbox's forced box, random ticks at speed 3 from the bridge's randValue), its chunks ticked in
/// the game's order (the forced box and the AFK player's square, from the bridge), natural
/// spawning from the game's lists (the spawning chunks before the shuffle, the spawn chunk count,
/// the entity-ticking chunks): in the void the gate's (monsters off, plains), in terrain the site's
/// (monsters on); none in the bare scenario.
pub fn redsim_start(spec: &FarmSpec, o: &RunOpts, game: &GameFarm, bare: bool) -> Result<Run> {
    use redsim::entity::mob::natural::{Difficulty, Natural};
    let mut run = Run::start(spec, o).map_err(|e| anyhow!(e))?;
    match o.site {
        None => run.world.mirror_level_random(o.seed, forced_chunks(spec)),
        Some(_) => run.world.mirror_level_random(o.seed, game_chunks(game)),
    }
    run.world.random_ticks = Some(redsim::crops::RandomTicks::new(3, o.seed));
    if !game.ticking.is_empty() {
        run.world.chunk_order = game.ticking.clone();
    }
    if bare {
        run.world.afk = None;
        run.world.mob.natural = None;
        return Ok(run);
    }
    let base = match (o.site, run.world.mob.natural.take()) {
        (Some(_), Some(n)) => n,
        _ => Natural { spawn_enemies: false, void_biome: Some("minecraft:plains"), ..Natural::new(vec![], Difficulty::Hard) },
    };
    let spawn_enemies = base.spawn_enemies;
    run.world.mob.natural = Some(Natural { spawn_order: game.spawning.clone(), spawn_chunk_count: Some(game.spawn_chunk_count as i32), entity_ticking: game.entity_ticking.clone(), ..base });
    if game.spawners {
        // the void gate's rules: no phantoms, patrols or traders; terrain: the game's defaults
        let all = o.site.is_some();
        let mut c = redsim::entity::mob::custom::Custom::new(o.seed, spawn_enemies, all, all, all);
        c.full_chunks = game.full.iter().copied().collect();
        if all {
            c.sea_level = 63;
        }
        run.world.mob.custom = Some(c);
    }
    Ok(run)
}

/// The box of the sandbox's ticking chunks (a terrain sandbox: the sample's).
fn game_chunks(game: &GameFarm) -> [i32; 4] {
    let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for [x, z] in &game.ticking {
        b = [b[0].min(*x), b[1].min(*z), b[2].max(*x), b[3].max(*z)];
    }
    b
}

/// Compare a farm design over `opts.ticks` game ticks after the settle; the compare's result.
pub fn compare(version: &str, design: &std::path::Path, opts: &Opts, draws_at: Option<i64>) -> Result<Compare> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut game = GameFarm::start(version, &c, &spec, &o, opts.weather, draws_at.is_some() || opts.draws_at.is_some(), place)?;
    let mut run = redsim_start(&spec, &o, &game, opts.bare)?;
    let mut watch = Watch::new(&run.world, spec.lo, spec.hi);
    let mut cmp = Compare::default();
    let window = |t0: i64, n: u32| [draws_at, opts.draws_at].iter().flatten().any(|d| *d >= t0 && *d < t0 + n as i64 + 3);
    let print = |f: &Frame| {
        if opts.draws_at == Some(f.t) {
            if !f.spawning_shuffled.is_empty() {
                println!("the spawning chunks in tickChunks' order at game time {} ({}): {}", f.t, f.spawning_shuffled.len(), f.spawning_shuffled.iter().map(|[x, z]| format!("{x},{z}")).collect::<Vec<_>>().join(" "));
            }
            println!("the game's draws at game time {} (stream, call site, count, the raw next(bits) outputs; in draw order):", f.t);
            for (k, (s, site, n)) in f.draws.iter().enumerate() {
                let vals = f.draw_values.get(k).map(|v| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                println!("  {s} {site} x{n} [{vals}]");
            }
        }
    };
    let mut t: i64 = 0;
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, window(t, n))? {
            print(&f);
            run.settle_tick();
            let ours = watch.frame(&run.world);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() && !opts.full && opts.draws_at.is_none_or(|d| d < t) {
            return Ok(cmp);
        }
    }
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    game.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        for f in game.trace(n, window(t, n))? {
            print(&f);
            run.tick();
            let ours = watch.frame(&run.world);
            cmp.tick(&f, &ours);
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() && !opts.full && opts.draws_at.is_none_or(|d| d < t) {
            break;
        }
    }
    Ok(cmp)
}

/// Give redsim's mobs the run's follow-range bonuses (in id order): `AttributeInstance`'s value,
/// base + base x bonus (`ADD_MULTIPLIED_BASE`).
pub fn set_bonuses(w: &mut redsim::world::World, bonus: &[f64]) {
    let mut k = 0;
    for e in w.entities.list.iter_mut() {
        if let EntityKind::Mob(m) = &mut e.kind {
            let base = m.ty.def().follow_range;
            if let Some(b) = bonus.get(k) {
                m.follow_range = base + base * b;
            }
            k += 1;
        }
    }
}

/// The bridge against itself: the same farm run twice in two workers, frame for frame (every
/// field the game reports), the determinism check of the bridge's seeding.
pub fn twin(version: &str, design: &std::path::Path, opts: &Opts, draws_at: Option<i64>) -> Result<Compare> {
    let (c, spec, place) = load_place(design, opts)?;
    let mut o = opts_of(opts);
    if let Place::Terrain { site, .. } = place {
        o.site = Some(site);
    }
    let mut a = GameFarm::start(version, &c, &spec, &o, opts.weather, draws_at.is_some(), place)?;
    let mut b = GameFarm::start(version, &c, &spec, &o, opts.weather, draws_at.is_some(), place)?;
    if a.ticking != b.ticking {
        println!("twin: the two runs tick different chunks: {} vs {} (first {:?} vs {:?})", a.ticking.len(), b.ticking.len(), a.ticking.first(), b.ticking.first());
    }
    let mut run = redsim_start(&spec, &o, &a, opts.bare)?;
    let mut cmp = Compare::default();
    let window = |t0: i64, n: u32| draws_at.is_some_and(|d| d >= t0 && d < t0 + n as i64 + 3);
    let mut t = 0i64;
    let show = |x: &Frame, y: &Frame| {
        if draws_at == Some(x.t) {
            let count = |f: &Frame| {
                let mut m: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
                for (s, site, n) in &f.draws {
                    *m.entry(format!("{s} {site}")).or_default() += n;
                }
                m
            };
            let (ca, cb) = (count(x), count(y));
            println!("twin draws at game time {}: call sites whose counts differ (run A, run B):", x.t);
            for k in ca.keys().chain(cb.keys()).collect::<std::collections::BTreeSet<_>>() {
                let (p, q) = (ca.get(k).copied().unwrap_or(0), cb.get(k).copied().unwrap_or(0));
                if p != q {
                    println!("  {k}: {p} vs {q}");
                }
            }
        }
    };
    let mut left = SETTLE_TICKS as u32;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, window(t, n))?, b.trace(n, window(t, n))?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            show(x, y);
            cmp.tick(x, &as_ours(y));
        }
        t += n as i64;
        left -= n;
    }
    while run.settle_tick() {}
    run.arrive().map_err(|e| anyhow!(e))?;
    let mobs = run.world.entities.list.iter().filter(|e| matches!(e.kind, EntityKind::Mob(_))).count();
    let bonus = follow_bonuses(opts.seed, mobs);
    set_bonuses(&mut run.world, &bonus);
    a.arrive(&c, &spec, &run.world, &bonus)?;
    b.arrive(&c, &spec, &run.world, &bonus)?;
    let mut left = opts.ticks;
    while left > 0 {
        let n = left.min(opts.batch);
        let (fa, fb) = (a.trace(n, window(t, n))?, b.trace(n, window(t, n))?);
        for (x, y) in fa.iter().zip(fb.iter()) {
            show(x, y);
            cmp.tick(x, &as_ours(y));
        }
        t += n as i64;
        left -= n;
        if cmp.first.is_some() {
            break;
        }
    }
    Ok(cmp)
}

/// A game frame on the compare's redsim side (the twin check): every memory counts as modeled.
fn as_ours(f: &Frame) -> Frame {
    let mut g = f.clone();
    for e in &mut g.entities {
        e.all_memories = true;
        e.ty = redsim::exact::normalize_type(&e.ty);
    }
    g
}
