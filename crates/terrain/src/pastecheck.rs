//! `mctas terrain paste-check`: a design pasted at a spot of a terrain sample, in redsim
//! (`terrain::build_world`, the footprint cleared, the design written raw, the light settled by the
//! engine) and in the official server on the very world the sample came from (the same cells by
//! `setblock ... strict`, a few ticks for its light engine, saved and read back): every block and
//! every light value of the box around the spot compared.

use crate::worldgen::{self, GenOpts};
use anyhow::{Context, Result, bail};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use redsim::pos::BlockPos;
use redsim::terrain::{Terrain, place};
use std::path::Path;

/// The paste: the cells to write in order (the footprint's air first, then the design), the box
/// they span, the blocks dug.
pub struct Paste {
    pub cells: Vec<(BlockPos, String)>,
    pub lo: BlockPos,
    pub hi: BlockPos,
    pub dug: u32,
    /// Design coordinates + this = world coordinates.
    pub offset: [i32; 3],
}

/// Build the redsim world around a spot and paste a design (its layer `ground_layer` one above the
/// spot's surface); returns the world and the cells written.
pub fn paste_redsim(t: &Terrain, design: &mctas::design::Design, at: [i32; 2], ground_layer: i32) -> Result<(redsim::World, Paste)> {
    let mut w = redsim::terrain::build_world(t).map_err(anyhow::Error::msg)?;
    let (dlo, dhi) = design.bounds().context("an empty design")?;
    let (lo_xz, hi_xz) = ([at[0] + dlo[0], at[1] + dlo[2]], [at[0] + dhi[0], at[1] + dhi[2]]);
    let ground = place::ground_y(&w, lo_xz, hi_xz);
    let dy = ground - ground_layer;
    let lo = BlockPos::new(lo_xz[0], dlo[1] + dy, lo_xz[1]);
    let hi = BlockPos::new(hi_xz[0], dhi[1] + dy, hi_xz[1]);
    if let Some(why) = place::refusal(&w, lo, hi) {
        bail!("spot refused: {why}");
    }
    let mut cells: Vec<(BlockPos, String)> = place::footprint_cells(&w, lo, hi).into_iter().filter(|p| !w.get(*p).is_air()).map(|p| (p, "minecraft:air".to_string())).collect();
    let dug = place::clear(&mut w, lo, hi);
    let mut blocks = Vec::new();
    for (p, s) in &design.blocks {
        let q = BlockPos::new(p[0] + at[0], p[1] + dy, p[2] + at[1]);
        let b = redsim::Block::parse(s).map_err(anyhow::Error::msg)?;
        blocks.push((q, b));
        cells.push((q, b.to_string()));
    }
    w.place_raw(&blocks);
    w.settle_light();
    Ok((w, Paste { cells, lo, hi, dug, offset: [at[0], dy, at[1]] }))
}

/// The same cells written in the official server on a copy of `world`; returns the box `lo..=hi`
/// read back (full height columns of radius `r` around the spot).
pub fn paste_game(version: &str, world: &Path, seed: i64, center: [i32; 2], r: i32, cells: &[(BlockPos, String)]) -> Result<Terrain> {
    let mut s = VanillaServer::start(&VanillaOptions {
        version: version.to_string(),
        heap_mb: 4096,
        world: Some(world.to_path_buf()),
        default_world: true,
        frozen: true,
        with_bot: false,
        keep_dir: false,
        view_distance: 2,
        simulation_distance: 2,
        ..VanillaOptions::default()
    })?;
    let (c0, c1) = ([(center[0] - r - 16).div_euclid(16), (center[1] - r - 16).div_euclid(16)], [(center[0] + r + 16).div_euclid(16), (center[1] + r + 16).div_euclid(16)]);
    s.cmd_ok(&format!("forceload add {} {} {} {}", c0[0] * 16, c0[1] * 16, c1[0] * 16 + 15, c1[1] * 16 + 15))?;
    for (p, st) in cells {
        let out = s.cmd(&format!("setblock {} {} {} {st} strict", p.x, p.y, p.z))?;
        if !out.contains("Changed the block") && !out.contains("Could not set the block") {
            bail!("setblock {p} {st}: {out}");
        }
    }
    // the light engine works between ticks
    s.step(40)?;
    let out = s.cmd("save-all flush")?;
    if !out.to_lowercase().contains("saved") {
        bail!("save-all flush: {out}");
    }
    let o = GenOpts { version: version.to_string(), seed, center, radius: r, margin_chunks: 0, days: 0, keep: false };
    worldgen::from_world(&s.dir.join(&s.level_name), &o)
}

/// Compare: blocks (the game's strings mapped like the sample's) and light, cell by cell, over the
/// game box; returns (cells, block diffs, sky diffs, block-light diffs, examples).
pub fn compare(w: &redsim::World, g: &Terrain) -> Result<(usize, usize, usize, usize, Vec<String>)> {
    let (mut cells, mut bd, mut sd, mut ld) = (0, 0, 0, 0);
    let mut ex = Vec::new();
    let land = w.land.as_ref().context("no land")?;
    let [ox, oy, oz] = g.origin;
    let [sx, sy, sz] = g.size;
    for y in oy..oy + sy {
        for z in oz..oz + sz {
            for x in ox..ox + sx {
                let p = BlockPos::new(x, y, z);
                if !w.contains(p) {
                    continue;
                }
                cells += 1;
                let i = g.index(x, y, z);
                let gname = &g.palette[g.cells[i] as usize];
                let gcanon = redsim::terrain::blocks::id_of(gname).map(|id| redsim::terrain::blocks::props(id).name.clone()).unwrap_or_else(|| gname.clone());
                let rb = w.get(p);
                if gcanon != rb.to_string() {
                    bd += 1;
                    if ex.len() < 12 {
                        ex.push(format!("block at {p}: redsim {rb}, game {gcanon}"));
                    }
                    continue;
                }
                let wi = ((y - w.min.y) * w.size[2] + (z - w.min.z)) * w.size[0] + (x - w.min.x);
                let (rs, rl) = (land.sky[wi as usize], land.block[wi as usize]);
                if rs != g.sky[i] {
                    sd += 1;
                    if ex.len() < 12 {
                        ex.push(format!("sky at {p}: redsim {rs}, game {} ({rb})", g.sky[i]));
                    }
                }
                if rl != g.block[i] {
                    ld += 1;
                    if ex.len() < 12 {
                        ex.push(format!("block light at {p}: redsim {rl}, game {} ({rb})", g.block[i]));
                    }
                }
            }
        }
    }
    Ok((cells, bd, sd, ld, ex))
}
