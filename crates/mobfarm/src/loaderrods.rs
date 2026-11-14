//! (loaderlay, Nov 14 2026; ironspec 01:17 (1b), G1 01:22 / 01:34) The lightning-rod coverage across a
//! loader layout's tiled copies, as redsim models 26.3's `ServerLevel.findLightningTargetAround` /
//! `findLightningRod` (crates/redsim/src/weather/lightning.rs). Tick-exact pinned the rod search from
//! 26.3 at 01:28, and redsim's `closest_rods` is the same:
//! - a 3-D sphere of radius 128 around the strike column's MOTION_BLOCKING top (`PoiManager.findClosest`
//!   -> `getInSquare` filtered by `distSqr(center) <= 128^2`, y included);
//! - only rods topping their column (y = WORLD_SURFACE - 1);
//! - the closest by 3-D distance wins, and the strike goes to the block above it.
//!
//! The world is the built overworld: every part's cells, the loader's included, settled as 9fc3155f's
//! lightning check settles a farm (`Run::new_homeless`: its one state source). Under it lies the site's
//! surface: a solid top block at the copies' ground y (each copy's y plus its module's `ground:` layer,
//! default -1, as the farm judge's site reads it; `--ground-y` sets it). A column's strike top is the
//! higher of the layout's motion-blocking top and the surface's.
//!
//! Printed:
//! - every rod, and whether it tops its column;
//! - per part, its footprint columns (each holding one of its cells): whose rods take their strikes, and
//!   the worst column's 3-D distance. A footprint column with no rod within 128 FAILS, naming the copy
//!   that needs its own rod;
//! - every other column of the layout's box grown by 3 (the ground between and around the copies):
//!   to a rod, or else judged by its own bolt's box at its strike top against every villager and rider
//!   position (FAIL when they meet);
//! - every topping rod's own bolt box against the loader's cells (the loop's items fly there, and a
//!   bolt burns an item) and against every villager and rider position (FAIL when they meet);
//! - every rider place (G1 05:10 (2): a struck rider becomes a witch that stalls the stop), by part and
//!   level y: each rail cell and each cart, with every column within 3 of it sending its strike to a
//!   rod (none lands beside it, none is redirected to an entity), so a strike on a rider needs a
//!   second fault. A column beside a rider with no rod FAILS.
//!
//! The villager and rider positions are every part's entities and a rider at every rail cell. An
//! entity's box is a cart or a boat with its rider seated, a golem's, else a villager-sized mob's. A
//! rail rider is the cell's footprint, from the rail up 0.5625 + 1.95. A bolt's box is
//! `lightning::hit_box_at` the bottom centre of its block: x and z +-3, y -3..+9.

use std::collections::{BTreeMap, BTreeSet};

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{Aabb, Vec3};
use redsim::pos::BlockPos;
use redsim::weather::lightning as lt;

use crate::loaderlayout::{built, Laid, C};

/// An entity's box at its feet `p`: a cart or a boat with its rider seated (the rider's 1.95 over the
/// seat at most 0.5625 up), a golem's, else a villager-sized mob's.
fn entity_box(kind: &str, p: [f64; 3]) -> Aabb {
    let k = kind.trim_start_matches("minecraft:");
    let (w, h) = if k == "minecart" {
        (0.98, 0.5625 + 1.95)
    } else if k.ends_with("boat") || k.ends_with("raft") {
        (1.375, 0.5625 + 1.95)
    } else if k == "iron_golem" {
        (1.4, 2.7)
    } else {
        (0.6, 1.95)
    };
    Aabb::new(p[0] - w / 2.0, p[1], p[2] - w / 2.0, p[0] + w / 2.0, p[1] + h, p[2] + w / 2.0)
}

/// The bolt's box when it falls on block `b`.
fn bolt_box(b: C) -> Aabb {
    lt::hit_box_at(Vec3::new(b[0] as f64 + 0.5, b[1] as f64, b[2] as f64 + 0.5))
}

/// The settled world of the built overworld layout.
fn world_of(parts: &[Laid]) -> Result<redsim::world::World, String> {
    let (ow, _) = built(parts, false, &[])?;
    // (the farm judge's rule, circuit::studies::mobfarm::farm_block: a portal block is a terrain block)
    let block = |s: &str| -> Result<Block, String> { if s.starts_with("minecraft:nether_portal[") { redsim::terrain::land::block_of(s) } else { Block::parse(s) } };
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for c in ow.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k] - 5);
            hi[k] = hi[k].max(c[k] + 5);
        }
    }
    let blocks = ow.iter().map(|(c, (s, _))| Ok((BlockPos::new(c[0], c[1], c[2]), block(s)?))).collect::<Result<Vec<_>, String>>()?;
    let spec = FarmSpec {
        blocks,
        entities: Vec::new(),
        // (the runner's output chest: the box's corner, 5 off every cell, so in no column judged)
        output: BlockPos::new(lo[0], lo[1], lo[2]),
        lo: BlockPos::new(lo[0], lo[1], lo[2]),
        hi: BlockPos::new(hi[0], hi[1], hi[2]),
        block_entities: Vec::new(),
        rock: false,
        nether: None,
        land: false,
    };
    Ok(Run::new_homeless(&spec, &RunOpts::steady(0, 0, WalkMode::Random))?.world)
}

/// The coverage lines and the verdict. `portal`: the loader's overworld PORTAL chunk (the chunk levels
/// a rod's load status reads), when its legs pass.
pub fn rods_text(parts: &[Laid], portal: Option<[i32; 2]>) -> Result<(String, bool), String> {
    let copies: Vec<&Laid> = parts.iter().filter(|p| p.name != "loader").collect();
    let grounds: BTreeSet<i32> = copies.iter().map(|p| p.ground).collect();
    let [ground] = grounds.iter().copied().collect::<Vec<_>>()[..] else {
        return Err(format!("the copies stand on different grounds {grounds:?} (or none): pass --ground-y"));
    };
    let (ow, _) = built(parts, false, &[])?;
    let w = world_of(parts)?;
    let owner = |c: &C| ow.get(c).map(|(_, pi)| parts[*pi].name.clone()).unwrap_or_else(|| "no part".into());
    let topping: BTreeSet<C> = lt::rods(&w).into_iter().map(|p| [p.x, p.y, p.z]).collect();
    let all: Vec<C> = ow.iter().filter(|(_, (s, _))| s.contains("lightning_rod")).map(|(c, _)| *c).collect();
    // (the villager and rider positions: every part's entities, a rider at every rail cell, and every
    // place of its positions file (`--positions`: a villager standing or seated there))
    let place_box = |c: &C| Aabb::new(c[0] as f64, c[1] as f64, c[2] as f64, (c[0] + 1) as f64, c[1] as f64 + 0.5625 + 1.95, (c[2] + 1) as f64);
    let mut people: Vec<(Aabb, String)> = Vec::new();
    for p in parts {
        for (kind, e) in &p.entities {
            people.push((entity_box(kind, *e), format!("{}'s {} at ({:.2},{:.2},{:.2})", p.name, kind.trim_start_matches("minecraft:"), e[0], e[1], e[2])));
        }
        for (c, st) in &p.blocks {
            if st.contains("rail[") {
                people.push((place_box(c), format!("a rider on {}'s rail ({},{},{})", p.name, c[0], c[1], c[2])));
            }
        }
        for c in &p.positions {
            people.push((place_box(c), format!("{}'s villager place ({},{},{})", p.name, c[0], c[1], c[2])));
        }
    }
    let meets = |b: Aabb| -> Vec<String> { people.iter().filter(|(e, _)| e.intersects(b)).map(|(_, n)| n.clone()).collect() };
    let top = |x: i32, z: i32| redsim::weather::motion_blocking_height(&w, x, z).max(ground + 1);
    let rod_of = |x: i32, t: i32, z: i32| lt::rod_targets(&w, BlockPos::new(x, t, z)).into_iter().next().map(|r| [r.x, r.y - 1, r.z]);
    let dist = |r: C, x: i32, t: i32, z: i32| (((r[0] - x).pow(2) + (r[1] - t).pow(2) + (r[2] - z).pow(2)) as f64).sqrt();
    let mut s = String::from(
        "lightning rods (ironspec 01:17 (1b) / 05:18 (3) / 05:22 (5) / 05:25 (iii), G1 01:22 / 01:34 / 05:10 / 05:26; 26.3 as tick-exact quoted it (05:26): a thunder roll picks a column of a ticking chunk (ServerLevel.tickThunder 550-583); findLightningTargetAround (631-649) takes the column's MOTION_BLOCKING top, then the nearest lightning rod (findLightningRod 619-629 -> PoiManager.findClosest 167-178 -> getInRange 97-102: distSqr <= 128^2, a 3-D radius; getInSquare 87-95), credited only where rodPos.y == getHeight(WORLD_SURFACE) - 1, the strike on rodPos.above(1); with no rod, a living entity that sees the sky in the column box inflated 3; the bolt's hit box x and z +-3, y -3..+9 (LightningBolt.tick 143-157); a villager whose box meets it converts to a witch, no sight test (Villager.thunderHit 746-760); redsim's findLightningRod is the same rule (unit test); the built overworld settled as the lightning check settles it):\n",
    );
    s.push_str(&format!(
        "  the site's surface: a solid top block at y {ground} under every column (the layout's own cells over it); {} villager and rider positions (the parts' entities, a rider at every rail cell, {} places from positions files)\n",
        people.len(),
        parts.iter().map(|p| p.positions.len()).sum::<usize>()
    ));
    s.push_str(&format!(
        "  rods (credited only where the game's test credits them, rodPos.y == WORLD_SURFACE - 1: equal to it, so stricter than or equal; each with its chunk's level from the loader's overworld PORTAL ticket: a rod's POI is found only in a loaded chunk, level <= 33): {}\n",
        if all.is_empty() {
            "none".to_string()
        } else {
            all.iter()
                .map(|c| {
                    let ch = [c[0].div_euclid(16), c[2].div_euclid(16)];
                    let load = match portal {
                        Some(pc) => {
                            let level = 30 + (ch[0] - pc[0]).abs().max((ch[1] - pc[1]).abs());
                            format!("chunk ({},{}) level {level}{}", ch[0], ch[1], if level <= 33 { " (loaded)" } else { " (NOT LOADED by the loader)" })
                        }
                        None => format!("chunk ({},{}) (its level not judged: no loader ticket)", ch[0], ch[1]),
                    };
                    format!("{}'s ({},{},{}){}, {load}", owner(c), c[0], c[1], c[2], if topping.contains(c) { " tops its column" } else { " does NOT top its column (no strike goes to it)" })
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    // (G1 05:26 (b): a credited rod's chunk must be loaded by the loader's ticket)
    let unloaded = match portal {
        Some(pc) => topping.iter().filter(|c| 30 + (c[0].div_euclid(16) - pc[0]).abs().max((c[2].div_euclid(16) - pc[1]).abs()) > 33).count(),
        None => 0,
    };
    let mut ok = unloaded == 0;
    if unloaded > 0 {
        s.push_str(&format!("  {unloaded} credited rods in chunks the loader doesn't load: FAIL\n"));
    }
    // (every column judged once: the parts' footprints and the 3 around them, each column's strike top
    // and the rod its strike goes to)
    let mut foot: BTreeSet<(i32, i32)> = BTreeSet::new();
    for p in parts {
        foot.extend(p.blocks.keys().map(|c| (c[0], c[2])));
    }
    let (mut x0, mut x1, mut z0, mut z1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    let carts = parts.iter().flat_map(|p| p.entities.iter().map(|(_, e)| (e[0].floor() as i32, e[2].floor() as i32)).chain(p.positions.iter().map(|c| (c[0], c[2]))));
    // (ironspec 05:25 (iii): THE STRIKE AREA is every chunk holding a layout cell or a villager place,
    // plus 4 columns beyond its edges: a strike chosen just outside a chunk reaches 3 sideways with its
    // hit box (LightningBolt.tick 143-157) and up to 4 with the entity search's inflated box around a
    // wide entity (findLightningTargetAround 631-649); every column in it is judged)
    for (x, z) in foot.iter().copied().chain(carts) {
        let (cx, cz) = (x.div_euclid(16) * 16, z.div_euclid(16) * 16);
        (x0, x1, z0, z1) = (x0.min(cx - 4), x1.max(cx + 15 + 4), z0.min(cz - 4), z1.max(cz + 15 + 4));
    }
    let mut col: BTreeMap<(i32, i32), (i32, Option<C>)> = BTreeMap::new();
    for z in z0..=z1 {
        for x in x0..=x1 {
            let t = top(x, z);
            col.insert((x, z), (t, rod_of(x, t, z)));
        }
    }
    for p in parts {
        let cols: BTreeSet<(i32, i32)> = p.blocks.keys().map(|c| (c[0], c[2])).collect();
        let mut far: Option<(f64, (i32, i32), i32, C)> = None;
        let mut none: Vec<String> = Vec::new();
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for &(x, z) in &cols {
            let (t, r) = col[&(x, z)];
            let Some(rod) = r else {
                let hit = meets(bolt_box([x, t, z]));
                none.push(format!("({x},{z}){}", if hit.is_empty() { String::new() } else { format!(" (its bolt meets {})", hit.join(", ")) }));
                continue;
            };
            let d = dist(rod, x, t, z);
            *by.entry(owner(&rod)).or_default() += 1;
            if far.is_none_or(|f| d > f.0) {
                far = Some((d, (x, z), t, rod));
            }
        }
        // (zero samples never pass)
        if !none.is_empty() || cols.is_empty() {
            ok = false;
        }
        s.push_str(&format!(
            "  {}: {} footprint columns; their strikes go to {}; the worst column {}; no rod within 128: {}\n",
            p.name,
            cols.len(),
            if by.is_empty() { "no rod".to_string() } else { by.iter().map(|(k, v)| format!("{k}'s rod {v}")).collect::<Vec<_>>().join(", ") },
            far.map_or("- (NOT JUDGED: FAIL)".to_string(), |(d, c, t, r)| format!("({},{}) (its top {t}) at {d:.2} from the rod ({},{},{})", c.0, c.1, r[0], r[1], r[2])),
            if none.is_empty() {
                "none".to_string()
            } else {
                format!("{} FAIL ({}{}): {} needs its own rod", none.len(), none.iter().take(4).cloned().collect::<Vec<_>>().join(" "), if none.len() > 4 { " ..." } else { "" }, p.name)
            }
        ));
    }
    // (G1 01:34 (1): the ground between and around the copies, every column of the box grown by 3)
    let (mut n, mut to_rod, mut harmless) = (0, 0, 0);
    let mut far: Option<(f64, (i32, i32), i32, C)> = None;
    let mut bad: Vec<String> = Vec::new();
    for (&(x, z), &(t, r)) in &col {
        if foot.contains(&(x, z)) {
            continue;
        }
        n += 1;
        if let Some(rod) = r {
            to_rod += 1;
            let d = dist(rod, x, t, z);
            if far.is_none_or(|f| d > f.0) {
                far = Some((d, (x, z), t, rod));
            }
            continue;
        }
        let hit = meets(bolt_box([x, t, z]));
        if hit.is_empty() {
            harmless += 1;
        } else {
            bad.push(format!("({x},{t},{z}) meets {}", hit.join(", ")));
        }
    }
    if !bad.is_empty() {
        ok = false;
    }
    s.push_str(&format!(
        "  the strike area beyond the footprints (every chunk holding a layout cell or a villager place, plus 4 columns beyond its edges: x {x0}..{x1}, z {z0}..{z1}; {n} columns off every footprint): {to_rod} to a rod (the worst {}), {harmless} with no rod within 128 whose bolt meets no position, {}\n",
        far.map_or("-".to_string(), |(d, c, t, r)| format!("({},{}) (its top {t}) at {d:.2} from ({},{},{})", c.0, c.1, r[0], r[1], r[2])),
        if bad.is_empty() { "none meeting one: PASS".to_string() } else { format!("{} whose bolt meets one: FAIL ({}{})", bad.len(), bad.iter().take(3).cloned().collect::<Vec<_>>().join("; "), if bad.len() > 3 { " ..." } else { "" }) }
    ));
    // (G1 05:10 (2): a struck rider becomes a witch that stalls the stop; so every rider position is
    // shown out of reach, place by place: every column within 3 of it (a bolt's box reaches 3
    // sideways) sends its strike to a rod, so no strike lands beside it and none is redirected to an
    // entity (the rod search runs first), and no rod's own box meets it (below). The places: every rail
    // cell (a rider seated in a cart there) and every cart the design holds, by part and level y)
    // (ironspec 05:18 (3): EVERY villager place, each with its coverage (every column within 3 of it
    // sending its strike to a rod topping its column) and its clearance (the gap between its box and
    // the nearest rod's own bolt box))
    let gap = |a: Aabb, b: Aabb| -> f64 {
        let d = |lo1: f64, hi1: f64, lo2: f64, hi2: f64| (lo2 - hi1).max(lo1 - hi2).max(0.0);
        let (dx, dy, dz) = (d(a.min_x, a.max_x, b.min_x, b.max_x), d(a.min_y, a.max_y, b.min_y, b.max_y), d(a.min_z, a.max_z, b.min_z, b.max_z));
        (dx * dx + dy * dy + dz * dz).sqrt()
    };
    let rod_boxes: Vec<Aabb> = topping.iter().map(|r| bolt_box([r[0], r[1] + 1, r[2]])).collect();
    // (part, kind, level y) -> (places, covering rods, the worst rod distance, columns with no rod, the least clearance)
    let mut places: BTreeMap<(usize, &'static str, i32), (usize, BTreeMap<String, usize>, f64, usize, f64)> = BTreeMap::new();
    for (pi, p) in parts.iter().enumerate() {
        let mut cells: BTreeSet<(&'static str, C)> = p.blocks.iter().filter(|(_, st)| st.contains("rail[")).map(|(c, _)| ("rail", *c)).collect();
        cells.extend(p.entities.iter().filter(|(k, _)| k.trim_start_matches("minecraft:") == "minecart").map(|(_, e)| ("cart", [e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32])));
        cells.extend(p.positions.iter().map(|c| ("place", *c)));
        for (kind, c) in cells {
            let clear = rod_boxes.iter().map(|b| gap(place_box(&c), *b)).fold(f64::INFINITY, f64::min);
            let e = places.entry((pi, kind, c[1])).or_insert((0, BTreeMap::new(), 0.0, 0, f64::INFINITY));
            e.0 += 1;
            e.4 = e.4.min(clear);
            for dx in -3..=3 {
                for dz in -3..=3 {
                    let (x, z) = (c[0] + dx, c[2] + dz);
                    match col.get(&(x, z)) {
                        Some(&(t, Some(rod))) => {
                            *e.1.entry(owner(&rod)).or_default() += 1;
                            e.2 = e.2.max(dist(rod, x, t, z));
                        }
                        _ => e.3 += 1,
                    }
                }
            }
        }
    }
    let open: usize = places.values().map(|v| v.3).sum();
    let struck: usize = places.values().filter(|v| v.4 <= 0.0).count();
    if open > 0 || struck > 0 {
        ok = false;
    }
    for (pi, p) in parts.iter().enumerate() {
        let lines: Vec<String> = places
            .iter()
            .filter(|((q, _, _), _)| *q == pi)
            .map(|((_, kind, y), (k, by, worst, un, clear))| {
                format!(
                    "{kind}s y {y}: {k} ({}, worst {worst:.2}; clear of the rods' bolt boxes by {}{})",
                    by.keys().map(|o| format!("{o}'s rod")).collect::<Vec<_>>().join(" + "),
                    if clear.is_finite() { format!("{clear:.2}") } else { "- (no rod)".to_string() },
                    if *un > 0 { format!("; {un} columns with no rod: FAIL") } else if *clear <= 0.0 { "; IN a rod's bolt box: FAIL".to_string() } else { String::new() }
                )
            })
            .collect();
        if !lines.is_empty() {
            s.push_str(&format!("  {}'s villager places (rail cells, carts, positions-file places; by kind and level): {}\n", p.name, lines.join("; ")));
        }
    }
    s.push_str(&format!(
        "  every villager place out of a strike's reach (ironspec 05:18 (3), G1 05:10 (2): a struck villager becomes a witch that stalls the stop): {}\n",
        if open == 0 && struck == 0 {
            "yes: every column within 3 of each sends its strike to a rod topping its column, and each is clear of every rod's bolt box, so a strike on one needs a second fault (a rod gone): PASS".to_string()
        } else {
            format!("{open} columns beside a place send no strike to a rod, {struck} place groups inside a rod's bolt box: FAIL")
        }
    ));
    // (the rods' own bolt boxes: the loop's items fly in the loader's cells; no villager or rider)
    let cell_box = |c: &C| Aabb::new(c[0] as f64, c[1] as f64, c[2] as f64, (c[0] + 1) as f64, (c[1] + 1) as f64, (c[2] + 1) as f64);
    if let Some(l) = parts.iter().find(|p| p.name == "loader") {
        let mut hits = Vec::new();
        for r in &topping {
            let b = bolt_box([r[0], r[1] + 1, r[2]]);
            let k = l.blocks.keys().filter(|c| cell_box(c).intersects(b)).count();
            if k > 0 {
                hits.push(format!("({},{},{}) {k} loader cells", r[0], r[1], r[2]));
            }
        }
        if !hits.is_empty() {
            ok = false;
        }
        s.push_str(&format!(
            "  the rods' own bolt boxes (x and z +-3, y -3..+9 around the cell above each) against the loader's {} cells: {}\n",
            l.blocks.len(),
            if hits.is_empty() { "none meets them: PASS".to_string() } else { format!("{}: FAIL (a bolt there burns the loop's items)", hits.join(", ")) }
        ));
    }
    let mut ehits = Vec::new();
    for r in &topping {
        for h in meets(bolt_box([r[0], r[1] + 1, r[2]])) {
            ehits.push(format!("{h} in the box of the rod ({},{},{})", r[0], r[1], r[2]));
        }
    }
    if !ehits.is_empty() {
        ok = false;
    }
    s.push_str(&format!(
        "  the rods' own bolt boxes against the {} villager and rider positions: {}\n",
        people.len(),
        if ehits.is_empty() { "none meets them: PASS".to_string() } else { format!("{}: FAIL", ehits.join("; ")) }
    ));
    s.push_str(&format!("  lightning verdict: {}\n", if ok { "PASS (every footprint column's strike goes to a rod topping its column, every rider position out of a strike's reach, no other strike meets a villager or a rider, no rod's box on the loader or a villager or a rider)" } else { "FAIL" }));
    Ok((s, ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part named `name` with a 3 x 3 glass slab at y 80 around (x, z), its rod on top when `rod`,
    /// standing on the ground at y 70.
    fn part(name: &str, x: i32, z: i32, rod: bool) -> Laid {
        let mut l = Laid { name: name.into(), ground: 70, ..Default::default() };
        for dx in -1..=1 {
            for dz in -1..=1 {
                l.blocks.insert([x + dx, 80, z + dz], "minecraft:glass".into());
            }
        }
        if rod {
            l.blocks.insert([x, 81, z], "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]".into());
        }
        l
    }

    #[test]
    fn every_column_goes_to_a_topping_rod_within_128() {
        // (a rod on copy1 covers copy2 at 100 blocks, and the ground between them)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), part("copy2", 100, 0, false)], None).unwrap();
        assert!(ok && t.contains("copy1's (0,81,0) tops its column") && t.contains("copy2: 9 footprint columns; their strikes go to copy1's rod 9") && t.contains("none meeting one: PASS"), "{t}");
    }

    #[test]
    fn a_copy_beyond_reach_or_a_covered_rod_fails() {
        // (copy2 at 140: beyond 128, it needs its own rod)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), part("copy2", 140, 0, false)], None).unwrap();
        assert!(!ok && t.contains("copy2 needs its own rod"), "{t}");
        // (a cell above the rod: it tops no column, no strike goes to it)
        let mut c = part("copy1", 0, 0, true);
        c.blocks.insert([0, 83, 0], "minecraft:glass".into());
        let (t, ok) = rods_text(&[c], None).unwrap();
        assert!(!ok && t.contains("does NOT top its column") && t.contains("copy1 needs its own rod"), "{t}");
        // (no rod at all)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, false)], None).unwrap();
        assert!(!ok && t.contains("level <= 33): none\n"), "{t}");
        // (copies on different grounds refuse without --ground-y)
        let mut c2 = part("copy2", 40, 0, false);
        c2.ground = 60;
        assert!(rods_text(&[part("copy1", 0, 0, true), c2], None).unwrap_err().contains("--ground-y"));
    }

    #[test]
    fn a_rod_box_on_the_loader_or_a_rider_fails() {
        let mut l = part("loader", 20, 0, false);
        l.blocks.insert([2, 84, 0], "minecraft:hopper[enabled=true,facing=down]".into());
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), l], None).unwrap();
        assert!(!ok && t.contains("(0,81,0) 1 loader cells: FAIL"), "{t}");
        // (a cart with its rider 3 east of the rod: its box meets the bolt's)
        let mut c = part("copy1", 0, 0, true);
        c.entities.push(("minecraft:minecart".into(), [3.2, 81.0, 0.5]));
        let (t, ok) = rods_text(&[c], None).unwrap();
        assert!(!ok && t.contains("copy1's minecart at (3.20,81.00,0.50) in the box of the rod (0,81,0)"), "{t}");
        // (a rail on the slab 1 east and 1 south of the rod: a rider there)
        let mut c = part("copy1", 0, 0, true);
        c.blocks.insert([1, 81, 1], "minecraft:rail[shape=north_south,waterlogged=false]".into());
        let (t, ok) = rods_text(&[c], None).unwrap();
        assert!(!ok && t.contains("a rider on copy1's rail (1,81,1) in the box of the rod (0,81,0)"), "{t}");
        // (6 east: clear)
        let mut c = part("copy1", 0, 0, true);
        c.entities.push(("minecraft:minecart".into(), [6.5, 81.0, 0.5]));
        let (t, ok) = rods_text(&[c], None).unwrap();
        assert!(ok && t.contains("against the 1 villager and rider positions: none meets them: PASS"), "{t}");
    }

    #[test]
    fn a_rod_outside_the_loaders_chunks_is_not_credited() {
        // (G1 05:26 (b): the rod's chunk (0,0) at Chebyshev 1 from the loader's PORTAL chunk (1,1):
        // level 31, loaded; at Chebyshev 4 from (4,0): level 34, not loaded: FAIL)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true)], Some([1, 1])).unwrap();
        assert!(ok && t.contains("chunk (0,0) level 31 (loaded)"), "{t}");
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true)], Some([4, 0])).unwrap();
        assert!(!ok && t.contains("chunk (0,0) level 34 (NOT LOADED by the loader)") && t.contains("1 credited rods in chunks the loader doesn't load: FAIL"), "{t}");
    }

    #[test]
    fn every_rider_place_is_out_of_reach_or_fails() {
        // (G1 05:10 (2), ironspec 05:18 (3): a rail 2.5 off the rod's bolt box and a positions-file place
        // 1.5 off it: every column within 3 of each goes to copy1's rod, each clear of the bolt box)
        let mut c = part("copy1", 0, 0, true);
        for x in -1..=1 {
            c.blocks.insert([x, 80, 6], "minecraft:glass".into());
        }
        c.blocks.insert([0, 81, 6], "minecraft:rail[shape=east_west,waterlogged=false]".into());
        c.positions = vec![[1, 81, 5]];
        let (t, ok) = rods_text(&[c.clone()], None).unwrap();
        assert!(ok && t.contains("copy1's villager places (rail cells, carts, positions-file places; by kind and level): places y 81: 1 (copy1's rod, worst") && t.contains("clear of the rods' bolt boxes by 1.50)") && t.contains("rails y 81: 1 (copy1's rod, worst") && t.contains("clear of the rods' bolt boxes by 2.50)") && t.contains("1 places from positions files") && t.contains("needs a second fault (a rod gone): PASS"), "{t}");
        // (a place inside the rod's bolt box)
        let mut inside = c.clone();
        inside.positions = vec![[2, 81, 2]];
        let (t, ok) = rods_text(&[inside], None).unwrap();
        assert!(!ok && t.contains("IN a rod's bolt box: FAIL") && t.contains("1 place groups inside a rod's bolt box: FAIL") && t.contains("copy1's villager place (2,81,2) in the box of the rod (0,81,0)"), "{t}");
        // (the same rail 140 out with no rod near: the columns beside it send no strike to a rod)
        let mut far = part("copy2", 140, 0, false);
        far.blocks.insert([140, 81, 0], "minecraft:rail[shape=east_west,waterlogged=false]".into());
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), far], None).unwrap();
        assert!(!ok && t.contains("copy2's villager places") && t.contains("49 columns with no rod: FAIL") && t.contains("send no strike to a rod"), "{t}");
    }

    #[test]
    fn a_ground_strike_beside_a_rider_fails() {
        // (G1 01:34 (1): copy2 at 200 has no rod within 128 of its ground; a cart standing on the ground
        // 2 beyond its slab is in the box of the ground strike next to it)
        let mut c2 = part("copy2", 200, 0, true);
        c2.blocks.retain(|_, s| !s.contains("lightning_rod"));
        c2.entities.push(("minecraft:minecart".into(), [203.5, 71.0, 0.5]));
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), c2], None).unwrap();
        assert!(!ok && t.contains("whose bolt meets one: FAIL") && t.contains("copy2's minecart at (203.50,71.00,0.50)"), "{t}");
    }

    /// (G1 01:34 (2); tick-exact's 26.3 pin, 01:28: `ServerLevel.findLightningRod` 619-629,
    /// `PoiManager.findClosest` 167-178 -> `getInRange` 97-102 -> `getInSquare` 87-95) redsim's
    /// `lightning::rod_targets` is the pinned rule: a 3-D sphere of 128 around the strike column's
    /// heightmap top (y included: `distSqr(center) <= 128^2` after the square |dx|, |dz| <= 128); only
    /// rods topping their column (y = WORLD_SURFACE - 1); the closest by 3-D distance; the strike on
    /// the block above it.
    #[test]
    fn redsim_find_lightning_rod_is_the_pinned_rule() {
        let rod = Block::parse("minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]").unwrap();
        let glass = Block::parse("minecraft:glass").unwrap();
        let fresh = || redsim::world::World::new(BlockPos::new(-2, 60, -2), [140, 20, 8]);
        let all = redsim::world::flags::ALL;
        let c = BlockPos::new(0, 64, 0);
        // (128 away in x at the centre's y: distSqr 128^2, inside; the strike on the block above)
        let mut w = fresh();
        w.set_block(BlockPos::new(128, 64, 0), rod.clone(), all);
        assert_eq!(lt::rod_targets(&w, c), vec![BlockPos::new(128, 65, 0)]);
        // (1 higher: 128^2 + 1, outside the sphere though inside the square: no rod)
        let mut w = fresh();
        w.set_block(BlockPos::new(128, 65, 0), rod.clone(), all);
        assert!(lt::rod_targets(&w, c).is_empty());
        // (a rod with a block over it tops no column: skipped, the farther topping rod wins)
        let mut w = fresh();
        w.set_block(BlockPos::new(5, 64, 0), rod.clone(), all);
        w.set_block(BlockPos::new(5, 70, 0), glass.clone(), all);
        w.set_block(BlockPos::new(20, 64, 0), rod.clone(), all);
        assert_eq!(lt::rod_targets(&w, c), vec![BlockPos::new(20, 65, 0)]);
        // (the closest by 3-D distance: 10 sideways (100) beats 6 sideways and 9 up (117))
        let mut w = fresh();
        w.set_block(BlockPos::new(10, 64, 0), rod.clone(), all);
        w.set_block(BlockPos::new(6, 73, 0), rod, all);
        assert_eq!(lt::rod_targets(&w, c), vec![BlockPos::new(10, 65, 0)]);
    }
}
