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
    /// Write the natural-state table (default `crates/redsim/src/terrain/blocks.tsv`) from the
    /// files' palettes: every state described by the game itself; `--check` compares instead.
    Blockinfo {
        files: Vec<PathBuf>,
        /// More states to describe (game strings).
        #[arg(long)]
        state: Vec<String>,
        #[arg(long, default_value = "crates/redsim/src/terrain/blocks.tsv")]
        out: PathBuf,
        #[arg(long)]
        check: bool,
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
        TerrainCmd::Blockinfo { files, state, out, check } => crate::blockinfo::run(version, &files, &state, &out, check)?,
        TerrainCmd::Find { seed, from, what } => {
            for s in seed {
                for line in worldgen::find(version, s, [from[0], from[1]], &what)? {
                    println!("seed {s}: {line}");
                }
            }
        }
        TerrainCmd::Info { file, palette, locate } => {
            let t = read(&file)?;
            print_info(&t, palette);
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
