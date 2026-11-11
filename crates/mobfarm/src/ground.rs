//! (tickexact 2026-11-11, ironspec's surroundings question) `mctas mobfarm ground`: a design's terrain
//! baked into a new design file, in the design's own coordinates, so every runner and check that
//! builds a void world from a design (queue-loop-run, siding-bench, flee-check) runs it in its built
//! surroundings unchanged. The site is the judge's (`redsim::entity::mob::site`: the sample's spots
//! for the design's footprint, priced, build K at the K-th cheapest), its ground line the design y at
//! the ground layer (the price tool's default -1); the cells around the design's box out to a margin
//! take the sample's natural blocks there, the box itself as the site digs it (a void design's box
//! cleared, a `terrain: cells` design only its listed cells written). Blocks only: no lighting, no
//! natural spawning (the HOME rows need the standable cells and the paths).

use anyhow::{Result, anyhow};
use redsim::entity::mob::farm::FarmSpec;
use redsim::entity::mob::site as ts;
use redsim::pos::BlockPos;

/// What `ground` wrote.
pub struct Grounded {
    pub sample: &'static str,
    pub offset: [i32; 3],
    pub added: usize,
    pub box_lo: BlockPos,
    pub box_hi: BlockPos,
}

/// The design at `design` (its spec `spec`, its meta for `terrain:`), grounded in `sample` with the
/// ground line `ground` at build `build`, `margin` cells of terrain round its box; written to `out`.
pub fn ground(design: &std::path::Path, spec: &FarmSpec, sample_name: &str, ground_y: i32, build: usize, margin: i32, out: &std::path::Path) -> Result<Grounded> {
    let text = std::fs::read_to_string(design)?;
    let mut d = mctas::design::Design::parse(&text)?;
    let (mut lo, mut hi) = (spec.blocks[0].0, spec.blocks[0].0);
    for (p, _) in &spec.blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    // (`flat`: the price tool's ground, no sample: the ground layer at `ground_y` (grass), three
    // layers of dirt under it, stone below; the design's box dug out (a void design's) or as listed)
    if sample_name == "flat" {
        let clear_box = !matches!(d.meta.get("terrain").map(|s| s.trim()), Some("cells") | Some("rock"));
        let mut added = 0;
        for y in lo.y - margin..=ground_y.min(hi.y + margin) {
            let state = if y == ground_y {
                "minecraft:grass_block[snowy=false]"
            } else if y >= ground_y - 3 {
                "minecraft:dirt"
            } else {
                "minecraft:stone"
            };
            for z in lo.z - margin..=hi.z + margin {
                for x in lo.x - margin..=hi.x + margin {
                    let inside = x >= lo.x && x <= hi.x && y >= lo.y && y <= hi.y && z >= lo.z && z <= hi.z;
                    if (inside && clear_box) || d.blocks.contains_key(&[x, y, z]) {
                        continue;
                    }
                    d.blocks.insert([x, y, z], state.to_string());
                    added += 1;
                }
            }
        }
        d.meta.insert("grounded".into(), format!("flat (the price tool's ground), ground line y {ground_y}, {margin} cells round the box, its box {}", if clear_box { "dug out" } else { "as listed" }));
        d.save(out)?;
        return Ok(Grounded { sample: "flat", offset: [0, 0, 0], added, box_lo: lo, box_hi: hi });
    }
    let sample = redsim::terrain::samples::sample(sample_name).ok_or_else(|| anyhow!("no terrain sample {sample_name} (flat, village-edge, plains, forest)"))?;
    let g = ground_y.to_string();
    let (gnd, afk, clear_box) = ts::params(spec, Some(&g), d.meta.get("afk").map(String::as_str), d.meta.get("terrain").map(String::as_str)).map_err(|e| anyhow!(e))?;
    let shared = ts::sample_world_shared(sample).map_err(|e| anyhow!(e))?;
    let fp = ts::footprint(spec, gnd).map_err(|e| anyhow!(e))?;
    let priced = {
        let w = shared.read().unwrap_or_else(|e| e.into_inner());
        let spots = ts::spots_for_spec(&w, sample, spec, fp, clear_box);
        if spots.is_empty() {
            return Err(anyhow!("no spot for a {}x{} footprint {} deep in {}", fp.size[0], fp.size[1], fp.below, sample.name));
        }
        ts::price_all(&w, spec, fp, gnd, clear_box, &spots)
    };
    if build >= priced.len() {
        return Err(anyhow!("--build {build}: {} spots in {}", priced.len(), sample.name));
    }
    let site = ts::Site::at_priced(shared.clone(), sample, priced, build, afk, clear_box);
    let [ox, oy, oz] = site.offset;
    let w = shared.read().unwrap_or_else(|e| e.into_inner());
    let (wmin, wsize) = (w.min, w.size);
    let in_world = |q: BlockPos| q.x >= wmin.x && q.y >= wmin.y && q.z >= wmin.z && q.x < wmin.x + wsize[0] && q.y < wmin.y + wsize[1] && q.z < wmin.z + wsize[2];
    let mut added = 0;
    for y in lo.y - margin..=hi.y + margin {
        for z in lo.z - margin..=hi.z + margin {
            for x in lo.x - margin..=hi.x + margin {
                let inside = x >= lo.x && x <= hi.x && y >= lo.y && y <= hi.y && z >= lo.z && z <= hi.z;
                // (a void design's box is cleared by the dig; a `terrain: cells` design keeps the
                // natural blocks round its own cells)
                if inside && clear_box {
                    continue;
                }
                if d.blocks.contains_key(&[x, y, z]) {
                    continue;
                }
                let q = BlockPos::new(x + ox, y + oy, z + oz);
                if !in_world(q) {
                    continue;
                }
                let b = w.get(q);
                if b.is_air() {
                    continue;
                }
                d.blocks.insert([x, y, z], b.to_string());
                added += 1;
            }
        }
    }
    drop(w);
    d.meta.insert("grounded".into(), format!("{} build {build} (offset {ox} {oy} {oz}), ground line y {gnd}, {margin} cells round the box, its box {}", sample.name, if clear_box { "cleared" } else { "as listed" }));
    d.save(out)?;
    Ok(Grounded { sample: sample.name, offset: site.offset, added, box_lo: lo, box_hi: hi })
}
