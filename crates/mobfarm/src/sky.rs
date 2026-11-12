//! The sky variant (ironspec 2026-11-11, vjeux's stage correction: the carousel is the endgame
//! farm): the module floating over a terrain sample, nothing placed on the natural surface, priced
//! against the buried one. `mctas mobfarm sky <design> --terrain <sample> [--rod] [--remove BOX]
//! --out <file>` finds the spot whose module needs the least scaffold, writes the sky design (its
//! roof, its daylight detector, its rod, its `ground:` at the natural ground under it, so the
//! judge's bill prices its height) and prints what it adds and removes. The rules:
//! - every villager position at least 8 above the highest natural block (leaves included) in the
//!   ±8 columns around it: the golem search's floors (y+5 down to y-7) never reach natural ground
//!   or a tree; and the whole box above the natural blocks under it (no supports);
//! - an opaque roof of bottom slabs (no spawn floor, non-flammable): at least 3 above every
//!   villager and golem cell box (a strike on it reaches 3 below its cell), at least 6 above every
//!   villager position (its top is no golem floor in any search band), over the footprint and 3
//!   around it with no rod (a strike on the ground beside the module reaches 9 up), over the
//!   footprint alone with one lightning rod on it (every strike within 128 goes to the rod,
//!   `findLightningRod`, which also leaves no trap horse: `tickChunk` skips the trap on a rod);
//! - a daylight detector on the roof (the time signal; the sentinel room goes, `--remove`);
//! - one pearl stasis cell per chunk the module covers (the chunk loader), counted.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use redsim::entity::mob::hostile;
use redsim::pos::BlockPos;

/// What the sky variant does: one lightning rod on the roof, the boxes removed from the design
/// (design cells, inclusive: the sentinel room), the extra villager positions (`--positions`) and
/// whether the rails count as villager positions.
pub struct SkyOpts {
    pub rod: bool,
    pub remove: Vec<(BlockPos, BlockPos)>,
    pub positions: Vec<BlockPos>,
    pub rails: bool,
    /// The least height of every villager position over the highest natural block within this
    /// many columns (8: the golem search's band, the rule; 16: the golem cells out of a ground
    /// skeleton's bow, `RangedBowAttackGoal` 15, skeletons targeting golems, not villagers; the
    /// villagers out of a pillager's 15-block hostile sensing: G1 2026-11-12).
    pub range: i32,
    /// The design y of its pocket level and the highest world y it may take: the STANDARD
    /// LAYOUT'S design choice, a tool default and no game rule (ironspec 2026-11-12, after G1):
    /// the shared kill room's portal at the pads' y, the loop pair 13 or more levels away and the
    /// whole Nether side under the bedrock roof for survival access together need the pocket at
    /// y 110 or less. 26.3 itself links pre-built portals at any y (the teleport keeps its y;
    /// `PortalForcer.findClosestPortalPosition` searches the horizontal square and takes the
    /// 3D-closest portal; only `createPortal` clamps to the Nether's logical height, 128). Spots
    /// needing a higher lift are refused, unless `capped` is off.
    pub pocket: Option<(i32, i32)>,
    /// The cap applied (off: `--no-pocket-cap`, for a site that needs it, whose gate is then
    /// onechunk's `portal-links` on that site's own Nether cells, every pad and loop portal
    /// linking to its intended partner by 3D distance with a margin of 2 or more).
    pub capped: bool,
}

/// The sky variant of `path` in terrain sample `t`: the report and the sky design's text.
pub fn sky(path: &Path, t: &str, o: &SkyOpts) -> Result<(String, String), String> {
    use redsim::entity::mob::site as ts;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut d = mctas::design::Design::parse(&text).map_err(|e| format!("{e:#}"))?;
    let inside = |p: [i32; 3]| o.remove.iter().any(|(a, b)| p[0] >= a.x && p[0] <= b.x && p[1] >= a.y && p[1] <= b.y && p[2] >= a.z && p[2] <= b.z);
    let before = d.blocks.len();
    d.blocks.retain(|p, _| !inside(*p));
    let removed = before - d.blocks.len();
    let ents_before = d.entities.len();
    d.entities.retain(|e| !inside([e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]));
    let ents_removed = ents_before - d.entities.len();
    // (the design as the sky variant starts it, loaded through the judge's own reader)
    let tmp = std::env::temp_dir().join(format!("mctas-sky-{}.txt", std::process::id()));
    std::fs::write(&tmp, d.to_text()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let c = circuit::Circuit::load(&tmp).map_err(|e| format!("{e:#}"))?;
    let spec = circuit::mobfarm::spec_of(&c).map_err(|e| format!("{e:#}"))?;
    let _ = std::fs::remove_file(&tmp);
    // the farm in the void: its villager positions (riders on every rail cell), its boxes
    let v = crate::sitecheck::place(&c, &spec, None, &o.positions, o.rails, None, None)?;
    let golem_boxes: Vec<_> = v.golem_cells.iter().map(|g| hostile::box_at(&hostile::GOLEM, *g, g.y as f64)).collect();
    let boxes: Vec<_> = v.villagers.iter().chain(v.carts.iter()).copied().chain(v.zombies.iter().flat_map(|z| z.boxes())).chain(golem_boxes.iter().copied()).collect();
    // each villager column's lowest position (the one the band binds); with a range over 8 (out
    // of a ground skeleton's bow), every golem cell too (a ranged target: skeletons attack golems)
    let mut low: HashMap<(i32, i32), i32> = HashMap::new();
    let golem_targets: &[BlockPos] = if o.range > 8 { &v.golem_cells } else { &[] };
    for q in v.positions.iter().chain(golem_targets.iter()) {
        let e = low.entry((q.x, q.z)).or_insert(i32::MAX);
        *e = (*e).min(q.y);
    }
    let sample = redsim::terrain::samples::sample(t).ok_or_else(|| format!("no terrain sample {t}"))?;
    let shared = ts::sample_world_shared(sample)?;
    let w = shared.read().unwrap_or_else(|e| e.into_inner());
    let (wx0, wz0, wsx, wsz) = (w.min.x, w.min.z, w.size[0], w.size[2]);
    // the highest natural block of each column (MOTION_BLOCKING: the ground, logs and leaves)
    let mut top = vec![0i32; (wsx * wsz) as usize];
    for z in 0..wsz {
        for x in 0..wsx {
            top[(z * wsx + x) as usize] = redsim::weather::motion_blocking_height(&w, wx0 + x, wz0 + z) - 1;
        }
    }
    let at = |x: i32, z: i32| -> Option<i32> { (x >= wx0 && z >= wz0 && x < wx0 + wsx && z < wz0 + wsz).then(|| top[((z - wz0) * wsx + (x - wx0)) as usize]) };
    // its window max over the columns within `range` of each column (8: the golem search's band)
    let r = o.range.max(8);
    let mut m8 = vec![i32::MIN; (wsx * wsz) as usize];
    for z in 0..wsz {
        for x in 0..wsx {
            let mut m = i32::MIN;
            for dz in -r..=r {
                for dx in -r..=r {
                    if let Some(h) = at(wx0 + x + dx, wz0 + z + dz) {
                        m = m.max(h);
                    }
                }
            }
            m8[(z * wsx + x) as usize] = m;
        }
    }
    let band = |x: i32, z: i32| -> Option<i32> { (x >= wx0 && z >= wz0 && x < wx0 + wsx && z < wz0 + wsz).then(|| m8[((z - wz0) * wsx + (x - wx0)) as usize]) };
    let (ground, _, _) = ts::params(&spec, c.clock.design.meta.get("ground").map(String::as_str), c.clock.design.meta.get("afk").map(String::as_str), c.clock.design.meta.get("terrain").map(String::as_str))?;
    let fp = ts::footprint(&spec, ground)?;
    let rule = crate::sitecheck::chunk_rule(&c)?;
    let (bx0, bx1, bz0, bz1, y0, y1) = (spec.lo.x, spec.hi.x, spec.lo.z, spec.hi.z, spec.lo.y, spec.hi.y);
    // every spot: the least lift (design to world y) the rules allow, and the air gap it leaves
    // under the module (the scaffold's height); the spot with the smallest gap
    let mut best: Option<(i32, i32, i32, i32, i32, String)> = None;
    for lz in (wz0 + 8)..=(wz0 + wsz - 9 - fp.size[1]) {
        for lx in (wx0 + 8)..=(wx0 + wsx - 9 - fp.size[0]) {
            let (sx, sz) = (lx - fp.lo.x, lz - fp.lo.z);
            if let Some(r) = &rule
                && !crate::sitecheck::chunk_ok(r, sx, sz)
            {
                continue;
            }
            let (mut off, mut why) = (i32::MIN, String::new());
            let mut ok = true;
            for ((x, z), vy) in &low {
                let Some(m) = band(x + sx, z + sz) else {
                    ok = false;
                    break;
                };
                if m + r - vy > off {
                    off = m + r - vy;
                    why = format!("the villager column ({},{}) at design y {vy}: the highest natural block within {r} columns at y {m}", x + sx, z + sz);
                }
            }
            if !ok {
                continue;
            }
            // (the footprint's columns as `--at` places it: the box's bottom over each one's
            // highest natural block; the ground layer `--at` puts it on, `place::ground_y`)
            for z in lz..lz + fp.size[1] {
                for x in lx..lx + fp.size[0] {
                    let Some(h) = at(x, z) else {
                        ok = false;
                        break;
                    };
                    if h + 1 - y0 > off {
                        off = h + 1 - y0;
                        why = format!("the box's bottom over the natural block at ({x},{h},{z})");
                    }
                }
            }
            if !ok {
                continue;
            }
            let gref = (lz..lz + fp.size[1]).flat_map(|z| (lx..lx + fp.size[0]).map(move |x| (x, z))).filter_map(|(x, z)| at(x, z)).max().unwrap_or(i32::MIN);
            let gap = y0 + off - gref - 1;
            if let Some((py, cap)) = o.pocket
                && o.capped
                && py + off > cap
            {
                continue;
            }
            if best.as_ref().is_none_or(|b| (gap, off) < (b.2, b.3)) {
                best = Some((lx, lz, gap, off, gref, why));
            }
        }
    }
    let (lx, lz, gap, off, gref, why) = best.ok_or_else(|| match o.pocket {
        Some((py, cap)) => format!("no spot in {t} for the sky module with its pocket level (design y {py}) at world y {cap} or lower (the standard layout's cap: its kill-room portal at the pads' y, the loop pair 13+ levels away, the Nether side under the roof; no game rule): a lower siting, or --no-pocket-cap with onechunk's portal-links on this site's own Nether cells as the gate (every pad and loop portal to its intended partner by 3D distance, margin 2 or more)"),
        None => format!("no spot in {t} for the sky module"),
    })?;
    let gy = redsim::terrain::place::ground_y(&w, [lx, lz], [lx + fp.size[0] - 1, lz + fp.size[1] - 1]);
    drop(w);
    // the roof
    let box_top = boxes.iter().map(|b| (b.max_y - 1e-9).ceil() as i32).max().unwrap_or(y1);
    let vtop = v.positions.iter().map(|q| q.y).max().unwrap_or(y1);
    let yr = (y1 + 1).max(box_top + 3).max(vtop + 6);
    let m = if o.rod { 0 } else { 3 };
    let slab = "minecraft:cobblestone_slab[type=bottom,waterlogged=false]".to_string();
    let mut roof = 0u32;
    for z in bz0 - m..=bz1 + m {
        for x in bx0 - m..=bx1 + m {
            if d.blocks.insert([x, yr, z], slab.clone()).is_none() {
                roof += 1;
            }
        }
    }
    let (cx, cz) = ((bx0 + bx1) / 2, (bz0 + bz1) / 2);
    if o.rod {
        d.blocks.insert([cx, yr + 1, cz], "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]".to_string());
    }
    d.blocks.insert([bx0, yr + 1, bz0], "minecraft:daylight_detector[inverted=false,power=0]".to_string());
    // the chunks the module covers at the spot (one pearl stasis cell each)
    let chunks: HashSet<(i32, i32)> = (lx..lx + fp.size[0]).flat_map(|x| (lz..lz + fp.size[1]).map(move |z| (x >> 4, z >> 4))).collect();
    // the module's own spawn-proofing: every dark floor inside its box a monster stands on (the
    // void-placed farm: an air cell over a full cube with air over it, not a golem cell, a track or
    // an entity's cell; under the roof, all are dark unless the design lights them)
    let golem: HashSet<BlockPos> = v.golem_cells.iter().copied().collect();
    let tracks: HashSet<BlockPos> = v.tracks.iter().copied().collect();
    let mut floors = 0u32;
    for y in y0..=yr {
        for z in bz0..=bz1 {
            for x in bx0..=bx1 {
                let q = BlockPos::new(x, y, z);
                if !v.w.contains(q.below()) || golem.contains(&q) || tracks.contains(&q) || v.interior.contains(&q) {
                    continue;
                }
                if v.w.get(q).is_air() && v.w.get(q.above()).is_air() && redsim::behavior::full_cube(&v.w.get(q.below())) {
                    floors += 1;
                }
            }
        }
    }
    // the sky design: its ground the layer `--at` puts it on (the bill prices its height from
    // there), no terrain dig, the stasis cell's level as its player spot
    let g = gy - off;
    d.meta.insert("ground".into(), g.to_string());
    d.meta.remove("terrain");
    d.meta.insert("afk".into(), format!("{}.5 {} {}.5", cx, yr - 1, cz));
    d.meta.insert("sky".into(), format!("{t} at ({lx},{lz}), lifted {off} (design y 0 at world y {off}), the box's bottom {gap} above the highest natural block under it, roof y {yr}{}", if o.rod { ", a lightning rod" } else { ", no rod (the roof 3 wider)" }));
    let report = format!(
        "sky variant in {t}: the footprint's min column ({lx},{lz}), lifted {off} (design y {y0}..{yr} at world y {}..{}), every villager position at least {r} over the highest natural block within {r} columns, the box's bottom {gap} blocks above the highest natural block under it (world y {gref}){}; binding: {why}\n\
         roof: bottom slabs at design y {yr} (world {}), {roof} slabs over {} columns ({}), at least 3 over every villager and golem box (their top {box_top}) and 6 over every villager position (the highest {vtop}){}\n\
         daylight detector on the roof at ({},{},{}); the sentinel room and the rest of --remove: {removed} blocks and {ents_removed} entities removed\n\
         pearl stasis cells: {} (one per chunk the module covers)\n\
         the module's own spawn-proofing: {floors} dark floors inside its box a monster stands on (a slab or carpet each, or light)\n\
         volume {} x {} x {} = {} blocks (box with its roof); the bill with its height: `mctas mobfarm bill <out> --stage late`; the site lines: `mctas mobfarm site-check <out> --terrain {t} --at {lx},{lz} --rails` (its ground layer then at world y {})",
        y0 + off,
        yr + off,
        match o.pocket {
            Some((py, cap)) if o.capped => format!(", the pocket level (design y {py}) at world y {} (the standard layout's cap {cap})", py + off),
            Some((py, cap)) if py + off > cap => format!(
                ", the pocket level (design y {py}) at world y {}, over the standard layout's {cap} (--no-pocket-cap): the gate is onechunk's portal-links on this site's own Nether cells, every pad and loop portal linking to its intended partner by 3D distance with a margin of 2 or more",
                py + off
            ),
            Some((py, _)) => format!(", the pocket level (design y {py}) at world y {}", py + off),
            None => String::new(),
        },
        yr + off,
        (bx1 - bx0 + 1 + 2 * m) * (bz1 - bz0 + 1 + 2 * m),
        if o.rod { "the footprint" } else { "the footprint and 3 around" },
        if o.rod { format!("; one lightning rod on it at ({cx},{},{cz}): every strike within 128 to it, its own strike box from {} up, no trap horse", yr + 1, yr - 1) } else { "; no rod: the roof's top is a strike surface, a skeleton trap's start (site-check's strike cells take it)".to_string() },
        bx0,
        yr + 1,
        bz0,
        chunks.len(),
        bx1 - bx0 + 1 + 2 * m,
        yr - y0 + 2,
        bz1 - bz0 + 1 + 2 * m,
        (bx1 - bx0 + 1 + 2 * m) * (yr - y0 + 2) * (bz1 - bz0 + 1 + 2 * m),
        gy,
    );
    Ok((report, d.to_text()))
}
