//! `mctas terrain ...`.

use crate::worldgen::{self, GenOpts};
use anyhow::{Context, Result};
use clap::Subcommand;
use redsim::terrain::format::{Heightmap, Terrain};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum TerrainCmd {
    /// Generate an area of a seed's world with the official server and save it as an
    /// `.mcterrain` file (default: `$MCTAS_HOME/terrain/<seed>_<x>_<z>_r<radius>.mcterrain`).
    Gen {
        #[arg(long, allow_hyphen_values = true)]
        seed: i64,
        /// The centre column, `X,Z`.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, required = true)]
        at: Vec<i32>,
        /// Half the kept box's side, in blocks.
        #[arg(long, default_value_t = 64)]
        radius: i32,
        /// Chunks generated beyond the kept box on each side (their light and neighbours settle).
        #[arg(long, default_value_t = 2)]
        margin: i32,
        /// Game days run after generation (the village claims beds and job sites; no mob spawning).
        #[arg(long, default_value_t = 1)]
        days: u32,
        /// Copy the server's world folder to `$MCTAS_HOME/terrain/worlds/<name>` (the real-game
        /// checks start from it).
        #[arg(long)]
        keep: bool,
        /// Read an existing saved world folder instead of running the server.
        #[arg(long)]
        world: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A top-down map of a file around a column: each column's top block as a letter.
    Map {
        file: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        around: Vec<i32>,
        #[arg(long, default_value_t = 48)]
        radius: i32,
    },
    /// Compare redsim's light engine with the light the game saved in a file (every cell of the
    /// box less `--margin` columns on each side).
    Light {
        file: PathBuf,
        #[arg(long, default_value_t = 16)]
        margin: i32,
        /// Print one column instead: `X,Z` (every y from `--from` to `--to`).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        column: Vec<i32>,
        #[arg(long, default_value_t = -64, allow_hyphen_values = true)]
        from: i32,
        #[arg(long, default_value_t = 100, allow_hyphen_values = true)]
        to: i32,
    },
    /// The game's own light reading (`getMaxLocalRawBrightness` at noon) at cells of a saved
    /// world: `--at X,Y,Z` repeated.
    Probe {
        #[arg(long)]
        world: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        at: Vec<i32>,
        /// Day times to probe at (the light with the sky's darkening; default 6000), each next to
        /// redsim's `daylight::sky_darken`.
        #[arg(long, value_delimiter = ',')]
        times: Vec<i64>,
    },
    /// Paste a design at a spot in redsim and in the official server (the sample's own world, the
    /// same cells by `setblock ... strict`) and compare every block and light value around it.
    /// Write the slab fixture (every slab redsim models: each material's bottom, top and double
    /// slabs dry and its bottom and top slabs waterlogged, a cell apart on two levels over a stone
    /// floor) for `paste-check` against the official server.
    SlabFixture {
        #[arg(long, default_value = "designs/terrain/fixtures/slabs.txt")]
        out: PathBuf,
    },
    PasteCheck {
        /// The sample (a cached full area or a crop).
        file: PathBuf,
        /// The sample's saved world folder (`$MCTAS_HOME/terrain/worlds/<name>`).
        #[arg(long)]
        world: PathBuf,
        #[arg(long)]
        design: PathBuf,
        /// The world column of the design's (0, 0).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        at: Vec<i32>,
        /// The design layer that stands on the surface.
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        ground: i32,
        /// Columns compared around the spot.
        #[arg(long, default_value_t = 24)]
        radius: i32,
    },
    /// Paste a design at a spot of a sample and run the worst-case checks for natural mobs
    /// (entry, sight, reach, the village golem, the golem search, creepers, endermen, fire, bed
    /// claims, lightning, natural spawns inside).
    Hostile {
        file: PathBuf,
        #[arg(long)]
        design: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        at: Vec<i32>,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        ground: i32,
        /// The AFK player's feet in design coordinates.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        afk: Vec<f64>,
    },
    /// The spot a footprint takes in a sample (the judge's deterministic spot search: no refusal,
    /// the fewest blocks dug, then the nearest to the sample's start).
    Spot {
        /// A standard sample's name (village-edge, plains, forest) or a file.
        sample: String,
        /// The footprint's columns (x, z).
        #[arg(long, value_delimiter = ',')]
        size: Vec<i32>,
        /// Layers of the design box under its ground layer.
        #[arg(long, default_value_t = 0)]
        below: i32,
        /// Override the search's start column and radius.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        near: Vec<i32>,
        #[arg(long)]
        radius: Option<i32>,
    },
    /// redsim's biome_at against the game's `execute if biome` at random cells of an area.
    BiomeCheck {
        file: PathBuf,
        #[arg(long)]
        world: PathBuf,
        #[arg(long, default_value_t = 300)]
        samples: usize,
    },
    /// Explain, cell by cell, whether a type can naturally spawn there in redsim.
    SpawnExplain {
        file: PathBuf,
        #[arg(long = "type")]
        ty: String,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        at: Vec<f64>,
        #[arg(long, default_value_t = 14000)]
        day_time: i64,
    },
    /// Natural spawning, the official server vs redsim: nights around an AFK player on the same
    /// saved world, the mobs alive at each night's end compared per type (Welch, Holm).
    SpawnCheck {
        /// The world's full-height area (`terrain gen` output).
        area: PathBuf,
        /// The saved world folder it came from.
        #[arg(long)]
        world: PathBuf,
        /// The AFK player's feet position.
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        afk: Vec<f64>,
        #[arg(long, default_value_t = 1200)]
        ticks: u32,
        #[arg(long, default_value_t = 13000)]
        day_time: i64,
        #[arg(long, default_value_t = 4)]
        nights: u32,
        #[arg(long, default_value_t = 16)]
        seeds: u32,
        #[arg(long, default_value_t = 144)]
        radius: i32,
        /// Also write every mob (game and redsim) here.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Take the game's nights from an earlier run's `--out` dump (redsim only is run).
        #[arg(long)]
        game_dump: Option<PathBuf>,
        /// Print redsim's counts every 100 ticks.
        #[arg(long)]
        trace: bool,
        /// Turn spawning off after this many ticks (a multiple of 100; 0: never).
        #[arg(long, default_value_t = 0)]
        fill: u32,
    },
    /// Write the biome table (default `crates/redsim/src/terrain/biomes.tsv`) from the game's biome
    /// data and tags.
    Biomes {
        #[arg(long, default_value = "crates/redsim/src/terrain/biomes.tsv")]
        out: PathBuf,
    },
    /// Print a saved chunk's sections (light layers present, palette sizes) from a world folder.
    Chunk {
        #[arg(long)]
        world: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        chunk: Vec<i32>,
    },
    /// Write the natural-state table (default `crates/redsim/src/terrain/blocks.tsv`) from the
    /// files' palettes: every state described by the game itself; `--check` compares instead.
    Blockinfo {
        files: Vec<PathBuf>,
        /// More states to describe (game strings).
        #[arg(long)]
        state: Vec<String>,
        /// Block ids whose every state joins the table (the designs' block ids join too).
        #[arg(long)]
        block: Vec<String>,
        /// Design files whose states are pasted into terrain (their palettes, and every state of
        /// their block ids, join the table).
        #[arg(long)]
        design: Vec<PathBuf>,
        #[arg(long, default_value = "crates/redsim/src/terrain/blocks.tsv")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
        /// Keep every state the table already has (a regenerated table only grows).
        #[arg(long)]
        keep_existing: bool,
        /// Every state of every slab redsim models (`redsim::slab::SLABS`: the slab family's
        /// light, odds, resistance and spawn floors come from their rows).
        #[arg(long)]
        slabs: bool,
    },
    /// The game's `/locate` from a point: `structure:village_plains`, `biome:forest`, ... (one
    /// server per seed: `--seed 1,2,3`).
    Find {
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        seed: Vec<i64>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_value = "0,0")]
        from: Vec<i32>,
        what: Vec<String>,
    },
    /// A file's summary: box, states, biomes, structures, entities, light.
    Info {
        file: PathBuf,
        /// Print every palette entry with its cell count.
        #[arg(long)]
        palette: bool,
        /// Print the positions of the states whose name contains this text.
        #[arg(long)]
        locate: Option<String>,
        /// List the palette's randomly ticking states (the natural-state table's flag).
        #[arg(long)]
        ticking: bool,
        /// Compare with another file: the box, every cell's state, the saved light, the biomes,
        /// the heightmaps, structures, entities, block entities and inhabited times.
        #[arg(long)]
        same_as: Option<PathBuf>,
        /// List every saved entity with its position.
        #[arg(long)]
        entities: bool,
    },
    /// Cut a box out of a file (`--lo X,Y,Z --hi X,Y,Z`, inclusive), or a column box around a
    /// centre (`--around X,Z --radius R`) from `--below` blocks under the lowest surface to
    /// `--above` blocks over the highest WORLD_SURFACE.
    Crop {
        file: PathBuf,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        lo: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        hi: Vec<i32>,
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        around: Vec<i32>,
        #[arg(long, default_value_t = 48)]
        radius: i32,
        #[arg(long, default_value_t = 16)]
        below: i32,
        #[arg(long, default_value_t = 4)]
        above: i32,
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(version: &str, cmd: TerrainCmd) -> Result<()> {
    match cmd {
        TerrainCmd::Gen { seed, at, radius, margin, days, keep, world, out } => {
            anyhow::ensure!(at.len() == 2, "--at takes X,Z");
            let center = [at[0], at[1]];
            let o = GenOpts { version: version.to_string(), seed, center, radius, margin_chunks: margin, days, keep };
            let stem = format!("{seed}_{}_{}_r{radius}", center[0], center[1]);
            let t = match world {
                Some(w) => worldgen::from_world(&w, &o)?,
                None => {
                    let dest = crate::cache_dir().join("worlds").join(&stem);
                    worldgen::generate(&o, keep.then_some(dest.as_path()))?
                }
            };
            let out = out.unwrap_or_else(|| crate::cache_dir().join(format!("{stem}.mcterrain")));
            write(&t, &out)?;
            print_info(&t, false);
        }
        TerrainCmd::Map { file, around, radius } => {
            anyhow::ensure!(around.len() == 2, "--around takes X,Z");
            print!("{}", crate::map::top_map(&read(&file)?, [around[0], around[1]], radius));
        }
        TerrainCmd::Light { file, margin, column, from, to } => {
            let t = read(&file)?;
            if column.len() == 2 {
                let g = redsim::terrain::light::grid_of_terrain(&t).map_err(anyhow::Error::msg)?;
                let (sky, block) = redsim::terrain::light::compute(&g);
                for y in (from..=to).rev() {
                    let (x, z) = (column[0], column[1]);
                    if !t.contains(x, y, z) {
                        continue;
                    }
                    let i = t.index(x, y, z);
                    println!("{y:>4} sky ours {:>2} game {:>2}  block ours {:>2} game {:>2}  {}", sky[i], t.sky[i], block[i], t.block[i], t.palette[t.cells[i] as usize]);
                }
                return Ok(());
            }
            let started = std::time::Instant::now();
            let c = redsim::terrain::light::compare_with_saved(&t, margin).map_err(anyhow::Error::msg)?;
            println!(
                "{} cells ({} open): sky differs in {} open / {} opaque cells, block light in {} open / {} opaque ({:.1} s)",
                c.cells,
                c.open_cells,
                c.sky_diff_open,
                c.sky_diff_opaque,
                c.block_diff_open,
                c.block_diff_opaque,
                started.elapsed().as_secs_f64()
            );
            for (x, y, z, ours, game, which) in &c.examples {
                println!("  {which} at {x} {y} {z}: ours {ours}, game {game}  ({})", t.state(*x, *y, *z).unwrap_or("?"));
            }
        }
        TerrainCmd::Probe { world, at, times } => {
            anyhow::ensure!(at.len() % 3 == 0 && !at.is_empty(), "--at takes X,Y,Z triples");
            let cells: Vec<[i32; 3]> = at.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            let times = if times.is_empty() { vec![6000] } else { times };
            let all = worldgen::probe_light_at(version, &world, &cells, &times)?;
            for (t, row) in times.iter().zip(all) {
                for (c, l) in cells.iter().zip(row) {
                    println!("day {t}: {} {} {}: light {l} (redsim skyDarken {})", c[0], c[1], c[2], redsim::daylight::sky_darken(*t));
                }
            }
        }
        TerrainCmd::SlabFixture { out } => {
            let mut states = Vec::new();
            for (i, _) in redsim::slab::all().enumerate() {
                for (kind, wet) in [(0u8, false), (1, false), (2, false), (0, true), (1, true)] {
                    states.push(redsim::slab::state_name(i as u8, kind, wet));
                }
            }
            let mut d = mctas::design::Design { name: "slabs".into(), ..Default::default() };
            d.meta.insert("circuit".into(), "fixture slabs".into());
            d.meta.insert("how".into(), "every slab redsim models, for mctas terrain paste-check".into());
            let side = 25;
            for x in 0..side {
                for z in 0..side {
                    d.blocks.insert([x, 0, z], "minecraft:stone".into());
                }
            }
            // a cell apart (no two slabs touch), on layers 1 and 3
            let spots: Vec<[i32; 3]> = [1, 3].iter().flat_map(|&y| (0..side).step_by(2).flat_map(move |x| (0..side).step_by(2).map(move |z| [x, y, z]))).collect();
            anyhow::ensure!(spots.len() >= states.len(), "{} slab states, {} spots", states.len(), spots.len());
            for (s, p) in states.iter().zip(&spots) {
                d.blocks.insert(*p, s.clone());
            }
            let text = format!(
                "# The slab fixture (terrain lane, `mctas terrain slab-fixture`): every slab redsim models\n# (redsim::slab::SLABS: bottom, top and double, dry; bottom and top, waterlogged), a cell apart\n# on two levels over a stone floor, for `mctas terrain paste-check` against the official server.\n{}",
                d.to_text()
            );
            std::fs::write(&out, text)?;
            println!("wrote {} ({} slab states)", out.display(), states.len());
        }
        TerrainCmd::PasteCheck { file, world, design, at, ground, radius } => {
            anyhow::ensure!(at.len() == 2, "--at takes X,Z");
            let t = read(&file)?;
            let spot = [at[0], at[1]];
            // redsim on a crop around the spot (the light settles inside the box)
            let lowest = t.heightmaps.get(Heightmap::OceanFloor.index()).and_then(|h| h.iter().min().copied()).unwrap_or(0) as i32;
            let crop = t.crop([spot[0] - radius - 20, (lowest - 24).max(t.origin[1]), spot[1] - radius - 20], [spot[0] + radius + 20, t.origin[1] + t.size[1] - 1, spot[1] + radius + 20]).map_err(anyhow::Error::msg)?;
            let des = mctas::design::Design::load(&design)?;
            let (w, paste) = crate::pastecheck::paste_redsim(&crop, &des, spot, ground)?;
            println!("redsim: footprint {}..{}, {} blocks dug, {} cells written", paste.lo, paste.hi, paste.dug, paste.cells.len());
            let g = crate::pastecheck::paste_game(version, &world, t.seed, spot, radius, &paste.cells)?;
            let (cells, bd, sd, ld, ex) = crate::pastecheck::compare(&w, &g)?;
            println!("{cells} cells compared: {bd} blocks differ, sky light differs in {sd}, block light in {ld}");
            for e in ex {
                println!("  {e}");
            }
        }
        TerrainCmd::Hostile { file, design, at, ground, afk } => {
            anyhow::ensure!(at.len() == 2 && afk.len() == 3, "--at takes X,Z and --afk X,Y,Z");
            let t = read(&file)?;
            let des = mctas::design::Design::load(&design)?;
            let t0 = std::time::Instant::now();
            let (paste, farm, exposures) = crate::hostilecheck::run(&t, &des, [at[0], at[1]], ground, [afk[0], afk[1], afk[2]])?;
            println!(
                "footprint {}..{}, {} blocks dug; farm: {} blocks, {} villager boxes, {} interior cells, {} golem cells, {} beds ({:.1} s)",
                paste.lo,
                paste.hi,
                paste.dug,
                farm.blocks.len(),
                farm.villagers.len(),
                farm.interior.len(),
                farm.golem_cells.len(),
                farm.beds.len(),
                t0.elapsed().as_secs_f64()
            );
            if exposures.is_empty() {
                println!("no exposure: no natural mob can reach, see, attack or enter the farm");
            }
            for e in exposures {
                println!("exposure {}", e.line());
            }
        }
        TerrainCmd::Spot { sample, size, below, near: near_arg, radius: radius_arg } => {
            anyhow::ensure!(size.len() == 2, "--size takes X,Z");
            let (path, near, radius) = match redsim::terrain::samples::sample(&sample) {
                Some(s) => (PathBuf::from(s.file), s.near, s.radius),
                None => (PathBuf::from(&sample), [0, 0], 16),
            };
            let t = read(&path)?;
            let near = if redsim::terrain::samples::sample(&sample).is_some() { near } else { [t.origin[0] + t.size[0] / 2 - size[0] / 2, t.origin[2] + t.size[2] / 2 - size[1] / 2] };
            let near = if near_arg.len() == 2 { [near_arg[0], near_arg[1]] } else { near };
            let radius = radius_arg.unwrap_or(radius);
            let w = redsim::terrain::build_world(&t).map_err(anyhow::Error::msg)?;
            match redsim::terrain::place::best_spot(&w, [size[0], size[1]], below, near, radius) {
                Some(s) => println!("{}: footprint {}x{} at min column {},{}; ground layer y {}; {} blocks dug", sample, size[0], size[1], s.lo_xz[0], s.lo_xz[1], s.ground, s.dug),
                None => {
                    for (why, n) in redsim::terrain::place::spot_tally(&w, [size[0], size[1]], below, near, radius).iter().take(8) {
                        println!("  {n:6} {why}");
                    }
                    let ground = redsim::terrain::place::ground_y(&w, near, [near[0] + size[0] - 1, near[1] + size[1] - 1]);
                    let why = redsim::terrain::place::refusal(&w, redsim::pos::BlockPos::new(near[0], ground - below, near[1]), redsim::pos::BlockPos::new(near[0] + size[0] - 1, ground, near[1] + size[1] - 1));
                    println!("{sample}: no spot within {radius} of {near:?} passes (at {near:?} itself: {why:?})")
                }
            }
        }
        TerrainCmd::BiomeCheck { file, world, samples } => {
            let t = read(&file)?;
            let (n, bad, ex) = worldgen::biome_check(version, &world, &t, samples)?;
            println!("{n} cells: {bad} biomes differ");
            for e in ex {
                println!("  {e}");
            }
        }
        TerrainCmd::SpawnExplain { file, ty, at, day_time } => {
            anyhow::ensure!(at.len() % 3 == 0 && !at.is_empty(), "--at takes X,Y,Z triples");
            let t = read(&file)?;
            let mut w = redsim::terrain::build_world(&t).map_err(anyhow::Error::msg)?;
            w.mob.seed(1, day_time);
            for c in at.chunks(3) {
                let p = redsim::pos::BlockPos::new(c[0].floor() as i32, c[1].floor() as i32, c[2].floor() as i32);
                println!("{}", redsim::entity::mob::natural::explain(&mut w, &ty, p));
            }
        }
        TerrainCmd::SpawnCheck { area, world, afk, ticks, day_time, nights, seeds, radius, out, game_dump, trace, fill } => {
            anyhow::ensure!(afk.len() == 3, "--afk takes X,Y,Z");
            let o = crate::spawncheck::Opts { version: version.to_string(), afk: [afk[0], afk[1], afk[2]], ticks, day_time, radius, trace, fill };
            let text = crate::spawncheck::run(&o, &area, &world, nights, seeds, out.as_deref(), game_dump.as_deref())?;
            print!("{text}");
        }
        TerrainCmd::Biomes { out } => {
            let text = crate::biomes::table(version)?;
            std::fs::write(&out, &text)?;
            println!("wrote {} ({} biomes)", out.display(), text.lines().count() - 2);
            let st = crate::biomes::structures_table(version)?;
            let path = out.with_file_name("structures.tsv");
            std::fs::write(&path, &st)?;
            println!("wrote {} ({} structures)", path.display(), st.lines().count() - 2);
        }
        TerrainCmd::Chunk { world, chunk } => {
            anyhow::ensure!(chunk.len() == 2, "--chunk takes CX,CZ");
            let dim = world.join("dimensions").join("minecraft").join("overworld");
            let mut cache = std::collections::HashMap::new();
            let c = crate::region::read_chunk(&dim, chunk[0], chunk[1], &mut cache, crate::worldgen::HEIGHT)?.context("chunk not saved")?;
            println!("status {} light_on {}", c.status, c.light_on);
            let mut ys: Vec<&i32> = c.sections.keys().collect();
            ys.sort();
            for y in ys {
                let s = &c.sections[y];
                let sky = s.sky_light.as_ref().map(|l| {
                    let vals: Vec<u8> = (0..4096).map(|i| crate::region::nibble(l, i % 16, i / 256, (i / 16) % 16)).collect();
                    format!("sky min {} max {}", vals.iter().min().unwrap(), vals.iter().max().unwrap())
                });
                println!("section {y:>3}: palette {:>3}, {}, block light {}", s.palette.len(), sky.unwrap_or_else(|| "no sky layer".into()), s.block_light.is_some());
            }
        }
        TerrainCmd::Blockinfo { files, mut state, mut block, design, out, check, keep_existing, slabs } => {
            if slabs {
                block.extend(redsim::slab::all().map(|m| format!("minecraft:{m}_slab")));
            }
            if keep_existing && let Ok(text) = std::fs::read_to_string(&out) {
                state.extend(text.lines().filter(|l| l.starts_with("minecraft:")).map(|l| l.split('\t').next().unwrap_or(l).to_string()));
            }
            for d in &design {
                let des = mctas::design::Design::load(d)?;
                for s in des.blocks.values() {
                    block.push(s.split('[').next().unwrap_or(s).to_string());
                    state.push(s.clone());
                }
            }
            block.sort();
            block.dedup();
            // air, and the blocks whose light is the same in every state (redstone dust's 1296 and
            // tripwire's 128: light.rs reads any of their rows)
            block.retain(|b| !matches!(b.as_str(), "minecraft:air" | "minecraft:redstone_wire" | "minecraft:tripwire"));
            state.extend(crate::blockinfo::expand(version, &block)?);
            crate::blockinfo::run(version, &files, &state, &out, check)?
        }
        TerrainCmd::Find { seed, from, what } => {
            for s in seed {
                for line in worldgen::find(version, s, [from[0], from[1]], &what)? {
                    println!("seed {s}: {line}");
                }
            }
        }
        TerrainCmd::Info { file, palette, locate, ticking, same_as, entities } => {
            let t = read(&file)?;
            print_info(&t, palette);
            if entities {
                for e in &t.entities {
                    println!("  entity {} at {:.2} {:.2} {:.2}", e.id, e.pos[0], e.pos[1], e.pos[2]);
                }
            }
            if let Some(other) = same_as {
                let o = read(&other)?;
                for line in compare_files(&t, &o) {
                    println!("{line}");
                }
            }
            if ticking {
                let mut counts = vec![0usize; t.palette.len()];
                for c in &t.cells {
                    counts[*c as usize] += 1;
                }
                for (i, s) in t.palette.iter().enumerate() {
                    if let Some(id) = redsim::terrain::blocks::id_of(s)
                        && redsim::terrain::blocks::props(id).random_ticking
                    {
                        println!("randomly ticking: {s} ({} cells)", counts[i]);
                    }
                }
            }
            if let Some(text) = locate {
                let ids: Vec<bool> = t.palette.iter().map(|s| s.contains(&text)).collect();
                let [ox, oy, oz] = t.origin;
                let [sx, sy, sz] = t.size;
                for y in oy..oy + sy {
                    for z in oz..oz + sz {
                        for x in ox..ox + sx {
                            let c = t.cells[t.index(x, y, z)] as usize;
                            if ids[c] {
                                println!("{x} {y} {z} {}", t.palette[c]);
                            }
                        }
                    }
                }
                for e in t.entities.iter().filter(|e| e.id.contains(&text)) {
                    println!("entity {} {:.2} {:.2} {:.2}", e.id, e.pos[0], e.pos[1], e.pos[2]);
                }
            }
        }
        TerrainCmd::Crop { file, lo, hi, around, radius, below, above, out } => {
            let t = read(&file)?;
            let (lo, hi) = if around.len() == 2 {
                column_box(&t, [around[0], around[1]], radius, below, above)?
            } else {
                anyhow::ensure!(lo.len() == 3 && hi.len() == 3, "give --around X,Z or --lo X,Y,Z --hi X,Y,Z");
                ([lo[0], lo[1], lo[2]], [hi[0], hi[1], hi[2]])
            };
            let c = t.crop(lo, hi).map_err(anyhow::Error::msg)?;
            write(&c, &out)?;
            print_info(&c, false);
        }
    }
    Ok(())
}

/// The box `--around` names: the columns within `radius`, from `below` under the lowest
/// OCEAN_FLOOR to `above` over the highest WORLD_SURFACE of those columns.
fn column_box(t: &Terrain, c: [i32; 2], radius: i32, below: i32, above: i32) -> Result<([i32; 3], [i32; 3])> {
    let (mut lowest, mut highest) = (i32::MAX, i32::MIN);
    for z in c[1] - radius..c[1] + radius {
        for x in c[0] - radius..c[0] + radius {
            let floor = t.height(Heightmap::OceanFloor, x, z).with_context(|| format!("column {x},{z} is outside the file"))?;
            let top = t.height(Heightmap::WorldSurface, x, z).unwrap_or(floor);
            lowest = lowest.min(floor);
            highest = highest.max(top);
        }
    }
    Ok(([c[0] - radius, lowest - below, c[1] - radius], [c[0] + radius - 1, highest + above, c[1] + radius - 1]))
}

pub fn read(path: &std::path::Path) -> Result<Terrain> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Terrain::decode(&bytes).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

pub fn write(t: &Terrain, path: &std::path::Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = t.encode();
    std::fs::write(path, &bytes)?;
    eprintln!("terrain: wrote {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}

/// `info --same-as`: what differs between two files (cells by state name, so palettes may differ in
/// order); one line per part, "same" when nothing differs.
fn compare_files(a: &Terrain, b: &Terrain) -> Vec<String> {
    let mut out = Vec::new();
    if a.origin != b.origin || a.size != b.size {
        out.push(format!("box: {:?}+{:?} vs {:?}+{:?}", a.origin, a.size, b.origin, b.size));
        return out;
    }
    let cells = a.cells.iter().zip(&b.cells).filter(|(x, y)| a.palette[**x as usize] != b.palette[**y as usize]).count();
    out.push(format!("cells: {} of {} differ", cells, a.cells.len()));
    let mut shown = 0;
    for (i, (x, y)) in a.cells.iter().zip(&b.cells).enumerate() {
        if a.palette[*x as usize] != b.palette[*y as usize] && shown < 40 {
            let (sx, sz) = (a.size[0] as usize, a.size[2] as usize);
            let (px, pz, py) = (i % sx, (i / sx) % sz, i / (sx * sz));
            out.push(format!("  {},{},{}: {} vs {}", a.origin[0] + px as i32, a.origin[1] + py as i32, a.origin[2] + pz as i32, a.palette[*x as usize], b.palette[*y as usize]));
            shown += 1;
        }
    }
    let sky = a.sky.iter().zip(&b.sky).filter(|(x, y)| x != y).count();
    let block = a.block.iter().zip(&b.block).filter(|(x, y)| x != y).count();
    out.push(format!("saved light: sky {} / block {} cells differ (sizes {}/{} vs {}/{})", sky, block, a.sky.len(), a.block.len(), b.sky.len(), b.block.len()));
    let biomes = if a.biome_origin == b.biome_origin && a.biome_size == b.biome_size {
        a.biomes.iter().zip(&b.biomes).filter(|(x, y)| a.biome_palette[**x as usize] != b.biome_palette[**y as usize]).count()
    } else {
        usize::MAX
    };
    out.push(format!("biome cells: {} differ", if biomes == usize::MAX { "the box".to_string() } else { biomes.to_string() }));
    out.push(format!("heightmaps: {}", if a.heightmaps == b.heightmaps { "same" } else { "differ" }));
    out.push(format!("structures: {}", if a.structures == b.structures { "same".to_string() } else { format!("{} vs {}", a.structures.len(), b.structures.len()) }));
    out.push(format!("entities: {}", if a.entities == b.entities { "same".to_string() } else { format!("{} vs {}", a.entities.len(), b.entities.len()) }));
    out.push(format!("block entities: {} vs {}", a.block_entities.len(), b.block_entities.len()));
    out.push(format!("inhabited chunks: {} vs {}", a.inhabited.len(), b.inhabited.len()));
    out.push(format!("spawn/times: {}", if a.spawn == b.spawn && a.game_time == b.game_time && a.day_time == b.day_time { "same" } else { "differ" }));
    out
}

fn print_info(t: &Terrain, palette: bool) {
    println!("seed {}  spawn {:?}  game time {}  day time {}", t.seed, t.spawn, t.game_time, t.day_time);
    println!("box {:?} + {:?} ({} cells), {} states, {} biomes", t.origin, t.size, t.volume(), t.palette.len(), t.biome_palette.len());
    for (k, v) in &t.notes {
        println!("  {k}: {v}");
    }
    let mut counts = vec![0usize; t.palette.len()];
    for &c in &t.cells {
        counts[c as usize] += 1;
    }
    if palette {
        let mut order: Vec<usize> = (0..t.palette.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(counts[i]));
        for i in order {
            println!("  {:>9}  {}", counts[i], t.palette[i]);
        }
    }
    let mut biomes: BTreeMap<&str, usize> = BTreeMap::new();
    for &b in &t.biomes {
        *biomes.entry(t.biome_palette[b as usize].as_str()).or_default() += 1;
    }
    println!("biome cells: {}", biomes.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
    for s in &t.structures {
        println!("structure {} {:?}..{:?}", s.id, s.min, s.max);
    }
    let mut ents: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &t.entities {
        *ents.entry(e.id.as_str()).or_default() += 1;
    }
    println!("entities: {}", ents.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", "));
    if let (Some(lo), Some(hi)) = (t.heightmaps.first().and_then(|h| h.iter().min()), t.heightmaps.first().and_then(|h| h.iter().max())) {
        println!("WORLD_SURFACE {lo}..{hi}");
    }
}
