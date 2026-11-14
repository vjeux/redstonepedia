//! (loaderlay, Nov 14 2026; ironspec 01:17 (1b), G1 01:22 / 01:34 / 06:37) The lightning-rod coverage
//! across a loader layout's tiled copies, as redsim models 26.3's `ServerLevel.findLightningTargetAround`
//! / `findLightningRod` (crates/redsim/src/weather/lightning.rs). Tick-exact pinned the rod search from
//! 26.3 (01:28, 05:26 / 05:27), and redsim's `closest_rods` is the same (the unit test below):
//! - a 3-D sphere of radius 128 around the strike column's MOTION_BLOCKING top (`PoiManager.findClosest`
//!   167-178 -> `getInRange` 97-102 -> `getInSquare` 87-95, `distSqr(center) <= 128^2`, y included);
//! - only rods topping their column (`findLightningRod` 619-629: y = WORLD_SURFACE - 1);
//! - the closest by 3-D distance wins, and the strike goes to the block above it.
//!
//! The world is the built overworld: every part's cells, the loader's included, settled as 9fc3155f's
//! lightning check settles a farm (`Run::new_homeless`: its one state source). The site's surface is
//! materialized in it as a ground plane (G1 05:51, ironspec 05:52): `minecraft:stone` at the copies'
//! ground y (each copy's y plus its module's `ground:` layer, default -1, as the farm judge's site reads
//! it; `--ground-y` sets it) in every column of the strike area that holds no layout cell at or above
//! that y. So a column's strike top is its motion-blocking top in that world, and a rod below the ground
//! (under the plane) tops no column (G1 06:37 N5).
//!
//! THE STRIKE AREA (ironspec 05:25 (iii)) is the box over every chunk holding a layout cell, an entity
//! or a villager place, plus 4 columns beyond (a strike chosen just outside a chunk reaches 3 sideways
//! with its hit box, `LightningBolt.tick` 147-151, and up to 4 with the entity search's box around a
//! wide entity, `findLightningTargetAround` 637). THE PLACES are every rail cell (a rider seated there),
//! every entity of every part (carts with their riders, the scarer boats with their pillager or zombie,
//! golems: G1 06:37 (5), ironspec 06:43; a bolt destroys a cart or a boat, `Entity.thunderHit`
//! 2929-2936 -> `VehicleEntity.hurtServer` 35-58: 5 x 10 = 50 > 40, and frees its rider), and every place
//! of `--positions` files.
//!
//! The bars (each FAILS by name; each has a single-change twin among the tests):
//! - (a) every column of the strike area sends its strike to a credited rod. Then no strike lands on a
//!   column top and none is redirected to an entity anywhere a place could be: a place's columns within
//!   3 lie in the strike area (checked: an internal error otherwise);
//! - (b) every place is clear of every credited rod's own bolt box (`lightning::hit_box_at` the cell
//!   above the rod: x and z +-3, y -3..+9; `Villager.thunderHit` 746-760 converts with no sight test);
//! - (c) no credited rod's bolt box meets a loader cell (a bolt burns the loop's items);
//! - (d) every credited rod's chunk is loaded by the loader's overworld PORTAL ticket (level <= 33: the
//!   POI search reads loaded sections only, G1 05:26 (b)); NOT JUDGED without the loader's PORTAL chunk,
//!   which FAILS;
//! - no credited rod at all FAILS through (a).
//!
//! The boxes: a cart or a boat with its rider seated is the vehicle's footprint (minecart 0.98 wide,
//! boat 1.375: `EntityType` sizes) from its feet up 0.5625 + 1.95 (the seat's height at most, plus a
//! villager's 1.95: above the seated rider's top, a conservative box); an iron golem 1.4 x 2.7; any
//! other mob a villager's 0.6 x 1.95; a rail cell's or a positions place's rider the cell's footprint
//! from its floor up 0.5625 + 1.95.

use std::collections::{BTreeMap, BTreeSet};

use redsim::block::Block;
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{Aabb, Vec3};
use redsim::pos::BlockPos;
use redsim::weather::lightning as lt;

use crate::loaderlayout::{built, Laid, C};

/// An entity's box at its feet `p` (the module header's sizes).
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

/// A rider at cell `c`: the cell's footprint from its floor up 0.5625 + 1.95.
fn cell_rider_box(c: &C) -> Aabb {
    Aabb::new(c[0] as f64, c[1] as f64, c[2] as f64, (c[0] + 1) as f64, c[1] as f64 + 0.5625 + 1.95, (c[2] + 1) as f64)
}

/// The bolt's box when it falls on block `b`.
fn bolt_box(b: C) -> Aabb {
    lt::hit_box_at(Vec3::new(b[0] as f64 + 0.5, b[1] as f64, b[2] as f64 + 0.5))
}

/// (ironspec 05:25 (iii)) THE STRIKE AREA: the box over every chunk holding a layout cell, an entity or a
/// villager place, plus 4 columns beyond: (x0, x1, z0, z1), inclusive.
pub fn strike_area(parts: &[Laid]) -> (i32, i32, i32, i32) {
    let (mut x0, mut x1, mut z0, mut z1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for p in parts {
        let cols = p.blocks.keys().map(|c| (c[0], c[2])).chain(p.entities.iter().map(|(_, e)| (e[0].floor() as i32, e[2].floor() as i32))).chain(p.positions.iter().map(|c| (c[0], c[2])));
        for (x, z) in cols {
            let (cx, cz) = (x.div_euclid(16) * 16, z.div_euclid(16) * 16);
            (x0, x1, z0, z1) = (x0.min(cx - 4), x1.max(cx + 15 + 4), z0.min(cz - 4), z1.max(cz + 15 + 4));
        }
    }
    (x0, x1, z0, z1)
}

/// (G1 05:51, ironspec 05:52) The ground plane: the cells at y `ground` of every strike-area column that
/// holds no cell of `cells` at or above `ground`.
pub fn ground_plane<'a>(parts: &[Laid], ground: i32, cells: impl Iterator<Item = &'a C>) -> Vec<C> {
    let (x0, x1, z0, z1) = strike_area(parts);
    let high: BTreeSet<(i32, i32)> = cells.filter(|c| c[1] >= ground).map(|c| (c[0], c[2])).collect();
    let mut out = Vec::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            if !high.contains(&(x, z)) {
                out.push([x, ground, z]);
            }
        }
    }
    out
}

/// The settled world of the built overworld layout over its ground plane.
fn world_of(parts: &[Laid], ground: i32) -> Result<redsim::world::World, String> {
    let (ow, _) = built(parts, false, &[])?;
    // (the farm judge's rule, circuit::studies::mobfarm::farm_block: a portal block is a terrain block)
    let block = |s: &str| -> Result<Block, String> { if s.starts_with("minecraft:nether_portal[") { redsim::terrain::land::block_of(s) } else { Block::parse(s) } };
    let plane = ground_plane(parts, ground, ow.keys());
    let (x0, x1, z0, z1) = strike_area(parts);
    let (mut lo, mut hi) = ([x0 - 1, ground - 2, z0 - 1], [x1 + 1, ground + 2, z1 + 1]);
    for c in ow.keys() {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k] - 5);
            hi[k] = hi[k].max(c[k] + 5);
        }
    }
    let stone = Block::parse("minecraft:stone")?;
    let mut blocks = ow.iter().map(|(c, (s, _))| Ok((BlockPos::new(c[0], c[1], c[2]), block(s)?))).collect::<Result<Vec<_>, String>>()?;
    blocks.extend(plane.iter().map(|c| (BlockPos::new(c[0], c[1], c[2]), stone.clone())));
    let spec = FarmSpec {
        blocks,
        entities: Vec::new(),
        // (the runner's output chest: the box's lowest corner, below the ground plane, so in no strike)
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
    let w = world_of(parts, ground)?;
    let owner = |c: &C| ow.get(c).map(|(_, pi)| parts[*pi].name.clone()).unwrap_or_else(|| "no part".into());
    let topping: BTreeSet<C> = lt::rods(&w).into_iter().map(|p| [p.x, p.y, p.z]).collect();
    let all: Vec<C> = ow.iter().filter(|(_, (s, _))| s.contains("lightning_rod")).map(|(c, _)| *c).collect();
    // (the places: (part, kind, its cell, its box, its name))
    let mut places: Vec<(usize, String, C, Aabb, String)> = Vec::new();
    for (pi, p) in parts.iter().enumerate() {
        for (c, st) in &p.blocks {
            if st.contains("rail[") {
                places.push((pi, "rail".into(), *c, cell_rider_box(c), format!("a rider on {}'s rail ({},{},{})", p.name, c[0], c[1], c[2])));
            }
        }
        for (kind, e) in &p.entities {
            let k = kind.trim_start_matches("minecraft:").to_string();
            places.push((pi, k.clone(), [e[0].floor() as i32, e[1].floor() as i32, e[2].floor() as i32], entity_box(kind, *e), format!("{}'s {k} at ({:.2},{:.2},{:.2})", p.name, e[0], e[1], e[2])));
        }
        for c in &p.positions {
            places.push((pi, "place".into(), *c, cell_rider_box(c), format!("{}'s villager place ({},{},{})", p.name, c[0], c[1], c[2])));
        }
    }
    let rod_of = |x: i32, t: i32, z: i32| lt::rod_targets(&w, BlockPos::new(x, t, z)).into_iter().next().map(|r| [r.x, r.y - 1, r.z]);
    let dist = |r: C, x: i32, t: i32, z: i32| (((r[0] - x).pow(2) + (r[1] - t).pow(2) + (r[2] - z).pow(2)) as f64).sqrt();
    let (x0, x1, z0, z1) = strike_area(parts);
    let mut s = String::from(
        "lightning rods (ironspec 01:17 (1b) / 05:18 (3) / 05:22 (5) / 05:25 (iii) / 06:43, G1 01:22 / 01:34 / 05:10 / 05:26 / 06:37; 26.3 as tick-exact quoted it (05:26 / 05:27 / 06:48): a thunder roll picks a column of a ticking chunk (ServerLevel.tickThunder 550-583); findLightningTargetAround (631-649) takes the column's MOTION_BLOCKING top (632), then the nearest lightning rod (findLightningRod 619-629 -> PoiManager.findClosest 167-178 -> getInRange 97-102: distSqr <= 128^2, a 3-D radius; getInSquare 87-95), credited only where rodPos.y == getHeight(WORLD_SURFACE) - 1, the strike on rodPos.above(1); with no rod, a living entity that sees the sky in the column box inflated 3 (637 / 638 / 640); the bolt's hit box x and z +-3, y -3..+9 (LightningBolt.tick 147-151); a villager whose box meets it converts to a witch, no sight test (Villager.thunderHit 746-760); a cart or a boat it meets is destroyed and its rider freed (Entity.thunderHit 2929-2936 -> VehicleEntity.hurtServer 35-58: 50 > 40); redsim's findLightningRod is the same rule (unit test); the built overworld settled as the lightning check settles it, over its ground plane):\n",
    );
    let plane = ground_plane(parts, ground, ow.keys());
    s.push_str(&format!(
        "  the strike area (every chunk holding a layout cell, an entity or a villager place, plus 4 columns beyond): x {x0}..{x1}, z {z0}..{z1}; the ground plane: minecraft:stone at y {ground} under its {} columns that hold no layout cell at or above y {ground}; {} places (every rail cell, every entity of every part, {} positions-file places)\n",
        plane.len(),
        places.len(),
        parts.iter().map(|p| p.positions.len()).sum::<usize>()
    ));
    // (d) the credited rods' chunks
    let level_of = |c: &C, pc: [i32; 2]| 30 + (c[0].div_euclid(16) - pc[0]).abs().max((c[2].div_euclid(16) - pc[1]).abs());
    s.push_str(&format!(
        "  rods (credited only where the game's test credits them, rodPos.y == WORLD_SURFACE - 1: equal to it, so stricter than or equal; each with its chunk's level from the loader's overworld PORTAL ticket): {}\n",
        if all.is_empty() {
            "none".to_string()
        } else {
            all.iter()
                .map(|c| {
                    let load = match portal {
                        Some(pc) => {
                            let level = level_of(c, pc);
                            format!("chunk ({},{}) level {level}{}", c[0].div_euclid(16), c[2].div_euclid(16), if level <= 33 { " (loaded)" } else { " (NOT LOADED by the loader)" })
                        }
                        None => format!("chunk ({},{}) (its level NOT JUDGED: no loader PORTAL chunk)", c[0].div_euclid(16), c[2].div_euclid(16)),
                    };
                    format!("{}'s ({},{},{}){}, {load}", owner(c), c[0], c[1], c[2], if topping.contains(c) { " tops its column" } else { " does NOT top its column (no strike goes to it)" })
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    let mut ok = true;
    let unloaded: Vec<&C> = match portal {
        Some(pc) => topping.iter().filter(|c| level_of(c, pc) > 33).collect(),
        None => Vec::new(),
    };
    let load_pass = portal.is_some() && unloaded.is_empty();
    ok &= load_pass;
    s.push_str(&format!(
        "  (d) every credited rod's chunk loaded by the loader (level <= 33; the POI search reads loaded sections only, G1 05:26 (b)): {}\n",
        match portal {
            None => "NOT JUDGED (no loader PORTAL chunk: a loader leg fails): FAIL".to_string(),
            Some(_) if unloaded.is_empty() => "PASS".to_string(),
            Some(_) => format!("{} credited rods in chunks the loader doesn't load: FAIL", unloaded.len()),
        }
    ));
    // (a) every strike-area column to a credited rod
    let mut col: BTreeMap<(i32, i32), (i32, Option<C>)> = BTreeMap::new();
    for z in z0..=z1 {
        for x in x0..=x1 {
            let t = redsim::weather::motion_blocking_height(&w, x, z);
            col.insert((x, z), (t, rod_of(x, t, z)));
        }
    }
    let mut foot: BTreeMap<(i32, i32), BTreeSet<usize>> = BTreeMap::new();
    for (pi, p) in parts.iter().enumerate() {
        for c in p.blocks.keys() {
            foot.entry((c[0], c[2])).or_default().insert(pi);
        }
    }
    let uncovered: Vec<(i32, i32, i32)> = col.iter().filter(|(_, (_, r))| r.is_none()).map(|(&(x, z), &(t, _))| (x, t, z)).collect();
    for (pi, p) in parts.iter().enumerate() {
        let cols: Vec<(i32, i32)> = foot.iter().filter(|(_, ps)| ps.contains(&pi)).map(|(c, _)| *c).collect();
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        let mut far: Option<(f64, (i32, i32), i32, C)> = None;
        let mut none = 0;
        for &(x, z) in &cols {
            match col[&(x, z)] {
                (t, Some(rod)) => {
                    *by.entry(owner(&rod)).or_default() += 1;
                    let d = dist(rod, x, t, z);
                    if far.is_none_or(|f| d > f.0) {
                        far = Some((d, (x, z), t, rod));
                    }
                }
                _ => none += 1,
            }
        }
        s.push_str(&format!(
            "  {}: {} footprint columns; their strikes go to {}; the worst column {}{}\n",
            p.name,
            cols.len(),
            if by.is_empty() { "no rod".to_string() } else { by.iter().map(|(k, v)| format!("{k}'s rod {v}")).collect::<Vec<_>>().join(", ") },
            far.map_or("-".to_string(), |(d, c, t, r)| format!("({},{}) (its top {t}) at {d:.2} from the rod ({},{},{})", c.0, c.1, r[0], r[1], r[2])),
            if none > 0 { format!("; {none} with no rod within 128: {} needs its own rod", p.name) } else { String::new() }
        ));
    }
    let area_pass = uncovered.is_empty();
    ok &= area_pass;
    let worst = col.iter().filter_map(|(&(x, z), &(t, r))| r.map(|r| (dist(r, x, t, z), (x, z), t, r))).max_by(|a, b| a.0.total_cmp(&b.0));
    s.push_str(&format!(
        "  (a) every strike-area column to a credited rod: {} of {} columns (the worst {}): {}\n",
        col.len() - uncovered.len(),
        col.len(),
        worst.map_or("-".to_string(), |(d, c, t, r)| format!("({},{}) (its top {t}) at {d:.2} from ({},{},{})", c.0, c.1, r[0], r[1], r[2])),
        if area_pass {
            "PASS".to_string()
        } else {
            format!(
                "{} with no rod within 128 ({}{}): FAIL",
                uncovered.len(),
                uncovered.iter().take(6).map(|(x, t, z)| {
                    let hits: Vec<&str> = places.iter().filter(|pl| pl.3.intersects(bolt_box([*x, *t, *z]))).map(|pl| pl.4.as_str()).collect();
                    format!("({x},{t},{z}){}", if hits.is_empty() { String::new() } else { format!(" meets {}", hits.join(", ")) })
                }).collect::<Vec<_>>().join("; "),
                if uncovered.len() > 6 { "; ..." } else { "" }
            )
        }
    ));
    // (the places: each one's columns within 3, the rods taking them (implied by (a): they lie in the
    // strike area), and (b) its clearance from every credited rod's bolt box)
    let gap = |a: Aabb, b: Aabb| -> f64 {
        let d = |lo1: f64, hi1: f64, lo2: f64, hi2: f64| (lo2 - hi1).max(lo1 - hi2).max(0.0);
        let (dx, dy, dz) = (d(a.min_x, a.max_x, b.min_x, b.max_x), d(a.min_y, a.max_y, b.min_y, b.max_y), d(a.min_z, a.max_z, b.min_z, b.max_z));
        (dx * dx + dy * dy + dz * dz).sqrt()
    };
    let rod_boxes: Vec<(C, Aabb)> = topping.iter().map(|r| (*r, bolt_box([r[0], r[1] + 1, r[2]]))).collect();
    // (part, kind, level y) -> (places, covering rods, the worst rod distance, columns with no rod, the least clearance)
    let mut table: BTreeMap<(usize, String, i32), (usize, BTreeSet<String>, f64, usize, f64)> = BTreeMap::new();
    let mut in_box: Vec<String> = Vec::new();
    let mut open = 0;
    for (pi, kind, c, bx, name) in &places {
        let clear = rod_boxes.iter().map(|(_, b)| gap(*bx, *b)).fold(f64::INFINITY, f64::min);
        for (r, b) in &rod_boxes {
            if bx.intersects(*b) {
                in_box.push(format!("{name} in the box of the rod ({},{},{})", r[0], r[1], r[2]));
            }
        }
        let e = table.entry((*pi, kind.clone(), c[1])).or_insert((0, BTreeSet::new(), 0.0, 0, f64::INFINITY));
        e.0 += 1;
        e.4 = e.4.min(clear);
        for dx in -3..=3 {
            for dz in -3..=3 {
                let (x, z) = (c[0] + dx, c[2] + dz);
                match col.get(&(x, z)) {
                    Some(&(t, Some(rod))) => {
                        e.1.insert(owner(&rod));
                        e.2 = e.2.max(dist(rod, x, t, z));
                    }
                    _ => {
                        e.3 += 1;
                        open += 1;
                    }
                }
            }
        }
    }
    // (a place's columns within 3 lie in the strike area (its chunk plus 4), so (a) passing covers them)
    if area_pass && open > 0 {
        return Err(format!("internal: (a) passed but {open} columns within 3 of a place send no strike to a rod (outside the strike area?)"));
    }
    for (pi, p) in parts.iter().enumerate() {
        let lines: Vec<String> = table
            .iter()
            .filter(|((q, _, _), _)| *q == pi)
            .map(|((_, kind, y), (k, by, worst, un, clear))| {
                format!(
                    "{kind} y {y}: {k} (columns within 3 to {}, worst {worst:.2}{}; clear of the rods' bolt boxes by {}{})",
                    if by.is_empty() { "no rod".to_string() } else { by.iter().map(|o| format!("{o}'s rod")).collect::<Vec<_>>().join(" + ") },
                    if *un > 0 { format!(", {un} with no rod") } else { String::new() },
                    if clear.is_finite() { format!("{clear:.2}") } else { "- (no rod)".to_string() },
                    if *clear <= 0.0 { ": IN a rod's bolt box" } else { "" }
                )
            })
            .collect();
        if !lines.is_empty() {
            s.push_str(&format!("  {}'s places (rail cells, entities, positions-file places; by kind and level): {}\n", p.name, lines.join("; ")));
        }
    }
    let clear_pass = !places.is_empty() && in_box.is_empty();
    ok &= clear_pass;
    s.push_str(&format!(
        "  (b) every place clear of every credited rod's bolt box ({} places): {}\n",
        places.len(),
        if places.is_empty() {
            "no place: NOT JUDGED: FAIL".to_string()
        } else if in_box.is_empty() {
            format!("the least clearance {:.2}: PASS", table.values().map(|v| v.4).fold(f64::INFINITY, f64::min))
        } else {
            format!("{}: FAIL", in_box.join("; "))
        }
    ));
    // (c) the rods' own bolt boxes against the loader's cells
    let cell_box = |c: &C| Aabb::new(c[0] as f64, c[1] as f64, c[2] as f64, (c[0] + 1) as f64, (c[1] + 1) as f64, (c[2] + 1) as f64);
    if let Some(l) = parts.iter().find(|p| p.name == "loader") {
        let hits: Vec<String> = rod_boxes
            .iter()
            .filter_map(|(r, b)| {
                let k = l.blocks.keys().filter(|c| cell_box(c).intersects(*b)).count();
                (k > 0).then(|| format!("({},{},{}) {k} loader cells", r[0], r[1], r[2]))
            })
            .collect();
        ok &= hits.is_empty();
        s.push_str(&format!(
            "  (c) the rods' bolt boxes against the loader's {} cells: {}\n",
            l.blocks.len(),
            if hits.is_empty() { "none meets them: PASS".to_string() } else { format!("{}: FAIL (a bolt there burns the loop's items)", hits.join(", ")) }
        ));
    }
    s.push_str(&format!(
        "  lightning verdict: {}\n",
        if ok {
            "PASS (every strike-area column goes to a credited rod, so no strike lands on a column top or is redirected to an entity there; every place (rail cells, every entity, positions-file places) clear of every credited rod's bolt box; no rod's box on the loader; every credited rod's chunk loaded)"
        } else {
            "FAIL"
        }
    ));
    Ok((s, ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part named `name` with a 3 x 3 glass slab at y 80 around (x, z), its rod on top when `rod`, a
    /// villager place 5 east on the ground (y 71), standing on the ground at y 70.
    fn part(name: &str, x: i32, z: i32, rod: bool) -> Laid {
        let mut l = Laid { name: name.into(), ground: 70, positions: vec![[x + 5, 71, z]], ..Default::default() };
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

    /// The verdict line of bar `key` ("(a)", "(b)", ...).
    fn bar<'a>(t: &'a str, key: &str) -> &'a str {
        t.lines().find(|l| l.trim_start().starts_with(key)).unwrap_or_else(|| panic!("no {key} line in\n{t}"))
    }

    #[test]
    fn every_strike_column_goes_to_a_topping_rod_within_128() {
        // (a rod on copy1 covers copy2 at 100 blocks, and the ground between them)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), part("copy2", 100, 0, false)], Some([0, 0])).unwrap();
        assert!(ok && t.contains("copy1's (0,81,0) tops its column") && t.contains("copy2: 9 footprint columns; their strikes go to copy1's rod 9") && bar(&t, "(a)").ends_with("PASS"), "{t}");
    }

    #[test]
    fn a_copy_beyond_reach_or_a_covered_rod_fails() {
        // (copy2 at 140: beyond 128, it needs its own rod)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), part("copy2", 140, 0, false)], Some([0, 0])).unwrap();
        assert!(!ok && t.contains("copy2 needs its own rod") && bar(&t, "(a)").ends_with("FAIL"), "{t}");
        // (a cell above the rod: it tops no column, no strike goes to it)
        let mut c = part("copy1", 0, 0, true);
        c.blocks.insert([0, 83, 0], "minecraft:glass".into());
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && t.contains("does NOT top its column") && t.contains("copy1 needs its own rod"), "{t}");
        // (a rod under the ground plane: N5, it tops no column)
        let mut c = part("copy1", 0, 0, false);
        c.blocks.insert([10, 65, 0], "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]".into());
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && t.contains("copy1's (10,65,0) does NOT top its column"), "{t}");
        // (no rod at all)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, false)], Some([0, 0])).unwrap();
        assert!(!ok && t.contains("loader's overworld PORTAL ticket): none\n"), "{t}");
        // (copies on different grounds refuse without --ground-y)
        let mut c2 = part("copy2", 40, 0, false);
        c2.ground = 60;
        assert!(rods_text(&[part("copy1", 0, 0, true), c2], Some([0, 0])).unwrap_err().contains("--ground-y"));
    }

    #[test]
    fn the_strike_area_bar_alone_fails_on_a_far_place() {
        // (G1 06:37 (4), (a)'s single-change twin: copy1 with its rod, plus one villager place at x 124
        // whose own columns within 3 still reach the rod (127.43 < 128) but whose chunk's strike area,
        // to x 131, reaches past 128: only (a) fails)
        let mut c = part("copy1", 0, 0, true);
        c.positions.push([124, 71, 0]);
        let (t, ok) = rods_text(&[c.clone()], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(a)").ends_with("FAIL") && bar(&t, "(b)").ends_with("PASS") && bar(&t, "(d)").ends_with("PASS"), "{t}");
        c.positions.pop();
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(ok && bar(&t, "(a)").ends_with("PASS"), "{t}");
    }

    #[test]
    fn a_place_or_a_boat_in_a_rods_bolt_box_fails_only_the_clearance_bar() {
        // (G1 06:37 (4), (b)'s single-change twin: a place beside the rod)
        let mut c = part("copy1", 0, 0, true);
        c.positions.push([2, 81, 2]);
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(b)").ends_with("FAIL") && bar(&t, "(b)").contains("copy1's villager place (2,81,2) in the box of the rod (0,81,0)") && bar(&t, "(a)").ends_with("PASS"), "{t}");
        // (ironspec 06:43: a scarer boat 3 east of the rod; 6 east is clear)
        let mut c = part("copy1", 0, 0, true);
        c.entities.push(("minecraft:oak_boat".into(), [3.2, 81.0, 0.5]));
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(b)").contains("copy1's oak_boat at (3.20,81.00,0.50) in the box of the rod (0,81,0)") && t.contains("oak_boat y 81: 1"), "{t}");
        let mut c = part("copy1", 0, 0, true);
        c.entities.push(("minecraft:oak_boat".into(), [6.5, 81.0, 0.5]));
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(ok && bar(&t, "(b)").ends_with("PASS"), "{t}");
        // (a rail on the slab 1 east and 1 south of the rod: a rider there)
        let mut c = part("copy1", 0, 0, true);
        c.blocks.insert([1, 81, 1], "minecraft:rail[shape=north_south,waterlogged=false]".into());
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(b)").contains("a rider on copy1's rail (1,81,1) in the box of the rod (0,81,0)"), "{t}");
        // (no place at all: NOT JUDGED)
        let mut c = part("copy1", 0, 0, true);
        c.positions.clear();
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(b)").contains("no place: NOT JUDGED: FAIL"), "{t}");
    }

    #[test]
    fn a_rod_box_on_the_loader_fails_only_its_bar() {
        // ((c)'s single-change twin)
        let mut l = part("loader", 20, 0, false);
        l.positions.clear();
        l.blocks.insert([2, 84, 0], "minecraft:hopper[enabled=true,facing=down]".into());
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true), l], Some([0, 0])).unwrap();
        assert!(!ok && bar(&t, "(c)").contains("(0,81,0) 1 loader cells: FAIL") && bar(&t, "(a)").ends_with("PASS") && bar(&t, "(b)").ends_with("PASS"), "{t}");
    }

    #[test]
    fn a_rod_outside_the_loaders_chunks_is_not_credited() {
        // ((d)'s single-change twin; G1 05:26 (b): the rod's chunk (0,0) at Chebyshev 1 from the loader's
        // PORTAL chunk (1,1): level 31, loaded; at Chebyshev 4 from (4,0): level 34, not loaded: FAIL)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true)], Some([1, 1])).unwrap();
        assert!(ok && t.contains("chunk (0,0) level 31 (loaded)"), "{t}");
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true)], Some([4, 0])).unwrap();
        assert!(!ok && t.contains("chunk (0,0) level 34 (NOT LOADED by the loader)") && bar(&t, "(d)").ends_with("FAIL") && bar(&t, "(a)").ends_with("PASS"), "{t}");
        // (G1 06:37 N3: no PORTAL chunk: the load NOT JUDGED fails)
        let (t, ok) = rods_text(&[part("copy1", 0, 0, true)], None).unwrap();
        assert!(!ok && bar(&t, "(d)").contains("NOT JUDGED"), "{t}");
    }

    #[test]
    fn the_places_table_shows_every_place_with_its_rods_and_clearance() {
        // (a rail 2.5 off the rod's bolt box and a positions-file place 1.5 off it)
        let mut c = part("copy1", 0, 0, true);
        for x in -1..=1 {
            c.blocks.insert([x, 80, 6], "minecraft:glass".into());
        }
        c.blocks.insert([0, 81, 6], "minecraft:rail[shape=east_west,waterlogged=false]".into());
        c.positions = vec![[1, 81, 5]];
        let (t, ok) = rods_text(&[c], Some([0, 0])).unwrap();
        assert!(ok && t.contains("place y 81: 1 (columns within 3 to copy1's rod, worst") && t.contains("clear of the rods' bolt boxes by 1.50)") && t.contains("rail y 81: 1 (columns within 3 to copy1's rod, worst") && t.contains("clear of the rods' bolt boxes by 2.50)"), "{t}");
    }

    #[test]
    fn the_four_copy_layout_of_record_passes_every_lightning_bar() {
        // (the in-tree loader of record with rev 3 at the four offsets of record, rev 3's places on
        // every copy: the rod list of record takes every strike-area column, every place clear)
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |p: &str| mctas::design::Design::parse(&std::fs::read_to_string(root.join(p)).unwrap()).unwrap();
        let m = read("designs/farms/iron/carousel-assembly/wip/onechunk/module/f1q2-module-record7f13b.txt");
        let l = read("designs/farms/iron/carousel-assembly/wip/loaderlay/loader-x8.txt");
        let places: Vec<C> = std::fs::read_to_string(root.join("designs/farms/iron/carousel-assembly/wip/loaderlay/rev3-nonrail.positions.txt"))
            .unwrap()
            .lines()
            .filter(|s| !s.trim().is_empty() && !s.starts_with('#'))
            .map(|s| {
                let v: Vec<i32> = s.split_whitespace().map(|n| n.parse().unwrap()).collect();
                [v[0], v[1], v[2]]
            })
            .collect();
        let mut parts: Vec<Laid> = [[6, 80, 6], [38, 80, 6], [6, 80, 38], [38, 80, 38]]
            .iter()
            .enumerate()
            .map(|(k, o)| {
                let mut p = crate::loaderlayout::lay(&format!("copy{}", k + 1), "m", &m, *o, 64).unwrap();
                p.positions = places.iter().map(|c| [c[0] + o[0], c[1] + o[1], c[2] + o[2]]).collect();
                p
            })
            .collect();
        parts.push(crate::loaderlayout::lay("loader", "l", &l, [25, 80, 19], 0).unwrap());
        let (t, ok) = rods_text(&parts, Some([1, 1])).unwrap();
        assert!(ok && ["(a)", "(b)", "(c)", "(d)"].iter().all(|k| bar(&t, k).ends_with("PASS")) && t.contains("copy4's (32,109,32) tops its column, chunk (2,2) level 31 (loaded)"), "{t}");
        // (the 20 scarer boats among the places)
        assert_eq!(parts.iter().map(|p| p.entities.iter().filter(|(k, _)| k.ends_with("boat")).count()).sum::<usize>(), 20, "{t}");
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
