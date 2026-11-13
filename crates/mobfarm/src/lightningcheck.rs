//! (9fc3155f, ironspec Nov 12 22:56 (2iii), 22:59, 23:00) The lightning static: every strike the
//! game can aim at the farm, as redsim models 26.3's `findLightningTargetAround` and
//! `LightningBolt.tick` (crates/redsim/src/weather/lightning.rs): each column within 3 blocks of
//! the farm's box, its strike at the closest lightning rod topping its column within 128 (then
//! nothing else), else at a living entity whose box meets the column box (from its motion-blocking
//! top to the world's top, inflated by 3) and that sees the sky at its block (sky light 15: glass
//! passes it), else at the column's motion-blocking top. A bolt that falls (rain at its block)
//! hits every living entity in its box (x and z +-3, y -3..+9) and sets fires (its block and the
//! 27 around it where a fire survives). Checked: (a) no bolt's box meets a farm entity (every
//! villager, zombie, pillager and vehicle the design holds, and a rider at every rider position:
//! the rails and the positions files) or (b) the rod's own box; (c) no fire it can set touches a
//! flammable block or a farm entity; (d) no farm redstone in a struck rod's signal reach (8 gt:
//! its six neighbours, and its support's when the support conducts).
use redsim::entity::mob::farm::{FarmSpec, Run, RunOpts, WalkMode};
use redsim::entity::{Aabb, EntityKind, Vec3};
use redsim::pos::{BlockPos, Dir};
use redsim::weather::lightning as lt;

/// A redstone component by its block id (what a struck rod's 8-gt signal can act on).
fn redstone_part(name: &str) -> bool {
    ["redstone", "repeater", "comparator", "rail", "piston", "trapdoor", "_door", "lamp", "observer", "note_block", "dispenser", "dropper", "hopper", "tnt", "lever", "button", "target", "bell", "daylight_detector", "sculk", "lectern", "jukebox", "fence_gate"]
        .iter()
        .any(|k| name.contains(k))
}

pub struct Opts {
    /// Rider positions (feet cells) besides the design's entities.
    pub positions: Vec<BlockPos>,
    /// Every rail cell a rider position (a rider seated in a cart there).
    pub rails: bool,
}

/// The lines, the number of findings (a bolt hitting a farm entity, a fire, a redstone part in a
/// rod's reach) and the samples (the farm entities judged; 0 when no entity or no strike column:
/// NOT JUDGED, ironspec Nov 12 23:56).
pub fn check(spec: &FarmSpec, o: &Opts) -> Result<(Vec<String>, usize, usize), String> {
    check_parts(spec, o).map(|(l, ef, es, rf, _)| (l, ef + rf, es))
}

/// (G1 Nov 13 12:52, R1) [`check`] split into its two bars: the ENTITY bar (the bolts' hits on
/// farm entities and their fires, over the farm entities judged) and the ROD bar (a redstone part
/// in a struck rod's reach, over the design's rods): a rod's finding needs no farm entity, so it
/// never hides behind an entity bar with 0 samples. Returns the lines, the entity bar's fails and
/// samples, the rod bar's fails and samples.
pub fn check_parts(spec: &FarmSpec, o: &Opts) -> Result<(Vec<String>, usize, usize, usize, usize), String> {
    let run = if spec.has_bed() { Run::new(spec, &RunOpts::steady(0, 0, WalkMode::Random)) } else { Run::new_homeless(spec, &RunOpts::steady(0, 0, WalkMode::Random)) }?;
    // (ironspec Nov 13 00:27 (A): the positive control plants the defect in what the row reads, a
    // design copy without its rod, through this same path: no flag removes cells here; G1 02:23
    // (b), ONE STATE SOURCE: `Run::new` is `Run::start` and every settle tick, so the strike
    // columns' tops, the sky and the fires read the settled world, the fluids flowed as the game's)
    let w = run.world;
    let rods0 = lt::rods(&w);
    // (the farm's entities: each living mob and each vehicle, by its box)
    let mut protect: Vec<(Aabb, BlockPos, String)> = Vec::new();
    for e in &w.entities.list {
        let what = match &e.kind {
            EntityKind::Mob(m) => format!("the {:?} at {:.1},{:.1},{:.1}", m.ty, e.pos.x, e.pos.y, e.pos.z).to_lowercase(),
            EntityKind::Boat(_) => format!("the boat at {:.1},{:.1},{:.1}", e.pos.x, e.pos.y, e.pos.z),
            EntityKind::Minecart(_) => format!("the cart at {:.1},{:.1},{:.1}", e.pos.x, e.pos.y, e.pos.z),
            _ => continue,
        };
        let living = matches!(e.kind, EntityKind::Mob(_));
        protect.push((e.bb, e.block_position(), if living { what } else { format!("{what} (a vehicle)") }));
    }
    // (a rider at every rider position: a villager seated in a cart at a rail cell, its feet at most
    // 0.5625 over the cell; a feet cell from the positions files: a villager standing there)
    let mut riders: Vec<BlockPos> = o.positions.clone();
    if o.rails {
        for (p, b) in &spec.blocks {
            if b.to_string().contains("rail[") {
                riders.push(*p);
            }
        }
    }
    riders.sort_by_key(|p| (p.y, p.z, p.x));
    riders.dedup();
    for c in &riders {
        protect.push((Aabb::new(c.x as f64, c.y as f64, c.z as f64, (c.x + 1) as f64, c.y as f64 + 0.5625 + 1.95, (c.z + 1) as f64), *c, format!("a rider at {},{},{}", c.x, c.y, c.z)));
    }
    let sees_sky = |p: BlockPos| redsim::weather::can_see_sky(&w, p).unwrap_or(false);
    let statics: Vec<(BlockPos, Aabb, String)> = protect.iter().filter(|(_, p, what)| !what.ends_with("(a vehicle)") && sees_sky(*p)).map(|(bb, p, what)| (*p, *bb, what.clone())).collect();
    // (every strike: the columns within 3 of the farm's box)
    let mut bolts: Vec<(BlockPos, String, bool)> = Vec::new();
    let (x0, x1) = (w.min.x - 3, w.min.x + w.size[0] + 2);
    let (z0, z1) = (w.min.z - 3, w.min.z + w.size[2] + 2);
    for z in z0..=z1 {
        for x in x0..=x1 {
            let center = BlockPos::new(x, redsim::weather::motion_blocking_height(&w, x, z), z);
            let rods = lt::rod_targets(&w, center);
            let mut add = |p: BlockPos, why: String, rod: bool| {
                if !bolts.iter().any(|(q, _, _)| *q == p) {
                    bolts.push((p, why, rod));
                }
            };
            if !rods.is_empty() {
                for r in rods {
                    add(r, format!("strikes near {x} {z} go to the rod under {r}"), true);
                }
                continue;
            }
            let search = Aabb::new(center.x as f64, center.y as f64, center.z as f64, (center.x + 1) as f64, (lt::WORLD_MAX_Y + 2) as f64, (center.z + 1) as f64).inflate(3.0, 3.0, 3.0);
            let mut aimed = false;
            for (p, bb, what) in &statics {
                if bb.intersects(search) {
                    add(*p, format!("a strike near {x} {z} aims at {what} (it sees the sky)"), false);
                    aimed = true;
                }
            }
            if !aimed {
                add(center, format!("a strike at the top of column {x} {z}"), false);
            }
        }
    }
    let rain = |p: BlockPos| redsim::weather::motion_blocking_height(&w, p.x, p.z) <= p.y && sees_sky(p);
    let mut lines = Vec::new();
    let mut findings = 0usize;
    let mut hits: Vec<String> = Vec::new();
    let mut fires: Vec<String> = Vec::new();
    let mut struck_entities: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let n_rod_bolts = bolts.iter().filter(|(_, _, r)| *r).count();
    for (b, why, is_rod) in &bolts {
        if !rain(*b) {
            continue;
        }
        let hit = lt::hit_box_at(Vec3::new(b.x as f64 + 0.5, b.y as f64, b.z as f64 + 0.5));
        for (bb, _, what) in &protect {
            if hit.intersects(*bb) {
                struck_entities.insert(what.clone());
                hits.push(format!("{why}: the bolt at {},{},{} hits {what}", b.x, b.y, b.z));
            }
        }
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let p = b.offset(dx, dy, dz);
                    if !w.get(p).is_air() {
                        continue;
                    }
                    let fire = redsim::weather::fire::state_at(&w, p);
                    if !redsim::weather::fire::can_survive(&w, &fire, p) {
                        continue;
                    }
                    // (ironspec Nov 12 22:59 (c): at a rod, no ignition may survive in its 3x3x3 at
                    // all, whatever burns nearby)
                    if *is_rod {
                        fires.push(format!("{why}: the bolt at the rod's {},{},{} can set a fire at {},{},{} (an ignition surviving in the rod's 3x3x3)", b.x, b.y, b.z, p.x, p.y, p.z));
                    }
                    // (a fire that survives: the flammable blocks it can reach, its 6 neighbours
                    // (burn) and FireBlock.tick's spread box, x and z +-1, y -1..+4)
                    let mut seen_q: Vec<BlockPos> = Vec::new();
                    for dy2 in -1..=4 {
                        for dz2 in -1..=1 {
                            for dx2 in -1..=1 {
                                let q = p.offset(dx2, dy2, dz2);
                                if q == p || seen_q.contains(&q) {
                                    continue;
                                }
                                seen_q.push(q);
                                if lt::flammable(&w.get(q)).is_some() {
                                    fires.push(format!("{why}: the bolt at {},{},{} can set a fire at {},{},{} that reaches {} at {},{},{}", b.x, b.y, b.z, p.x, p.y, p.z, w.get(q).to_string().trim_start_matches("minecraft:"), q.x, q.y, q.z));
                                }
                            }
                        }
                    }
                    for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
                        let q = p.rel(d);
                        if seen_q.contains(&q) {
                            continue;
                        }
                        if lt::flammable(&w.get(q)).is_some() {
                            fires.push(format!("{why}: the bolt at {},{},{} can set a fire at {},{},{} next to {} at {},{},{}", b.x, b.y, b.z, p.x, p.y, p.z, w.get(q).to_string().trim_start_matches("minecraft:"), q.x, q.y, q.z));
                        }
                    }
                    let cell = Aabb::of_block(p);
                    for (bb, _, what) in &protect {
                        if cell.intersects(*bb) {
                            fires.push(format!("{why}: the bolt at {},{},{} can set a fire at {},{},{} under {what}", b.x, b.y, b.z, p.x, p.y, p.z));
                        }
                    }
                }
            }
        }
    }
    hits.sort();
    hits.dedup();
    fires.sort();
    fires.dedup();
    // (d) a struck rod's signal: its six neighbours, and its support's when the support conducts
    let mut reach: Vec<String> = Vec::new();
    for r in lt::rods(&w) {
        let s = w.get(r);
        let facing = s.to_string().split("facing=").nth(1).and_then(|t| t.split([',', ']']).next()).unwrap_or("up").to_string();
        let back = match facing.as_str() {
            "up" => Dir::Down,
            "down" => Dir::Up,
            "north" => Dir::South,
            "south" => Dir::North,
            "west" => Dir::East,
            _ => Dir::West,
        };
        let support = r.rel(back);
        let mut cells: Vec<BlockPos> = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East].iter().map(|d| r.rel(*d)).collect();
        if redsim::behavior::conductor_of(&w.get(support)) {
            cells.extend([Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East].iter().map(|d| support.rel(*d)).filter(|q| *q != r));
        }
        for q in cells {
            let name = w.get(q).to_string();
            if redstone_part(&name) {
                reach.push(format!("the rod at {},{},{} (facing {facing}, on {}) reaches {} at {},{},{}", r.x, r.y, r.z, w.get(support).to_string().trim_start_matches("minecraft:"), name.trim_start_matches("minecraft:"), q.x, q.y, q.z));
            }
        }
    }
    findings += hits.len() + fires.len() + reach.len();
    let samples = if protect.is_empty() || bolts.is_empty() { 0 } else { protect.len() };
    lines.push(format!(
        "lightning (ironspec Nov 12 22:56 / 22:59 / 23:00; redsim's model of 26.3's findLightningTargetAround and LightningBolt.tick): {} rods, {} riders placed besides the design's {} entities ({} farm entities judged), {} strike targets ({} at a rod) from {} strike columns; {} entities struck, {} hits, {} fires, {} redstone parts in a rod's reach{}",
        rods0.len(),
        riders.len(),
        protect.len() - riders.len(),
        protect.len(),
        bolts.len(),
        n_rod_bolts,
        (x1 - x0 + 1) * (z1 - z0 + 1),
        struck_entities.len(),
        hits.len(),
        fires.len(),
        reach.len(),
        if samples == 0 { " (NOT JUDGED: 0 samples)" } else if findings == 0 { " (PASS)" } else { " (FAIL)" }
    ));
    for r in &rods0 {
        lines.push(format!("  ROD {},{},{}: {}", r.x, r.y, r.z, w.get(*r).to_string().trim_start_matches("minecraft:")));
    }
    for e in &struck_entities {
        lines.push(format!("  STRUCK {e}"));
    }
    for h in hits.iter().take(24) {
        lines.push(format!("  HIT {h}"));
    }
    for f in fires.iter().take(12) {
        lines.push(format!("  FIRE {f}"));
    }
    for r in &reach {
        lines.push(format!("  ROD-REACH {r}"));
    }
    Ok((lines, hits.len() + fires.len(), samples, reach.len(), rods0.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use redsim::block::Block;

    fn spec_of(cells: &[(BlockPos, &str)], entities: Vec<(String, [f64; 3], Option<String>)>) -> FarmSpec {
        let blocks = cells.iter().map(|(p, b)| (*p, Block::parse(b).unwrap())).collect();
        FarmSpec { blocks, entities, output: BlockPos::new(11, 6, 11), lo: BlockPos::new(-12, -2, -12), hi: BlockPos::new(12, 8, 12), block_entities: Vec::new(), rock: false, nether: None, land: false }
    }

    /// (ironspec Nov 12 23:00) A villager under the open sky: a lightning rod topping its column 14
    /// blocks away takes every strike and its own box misses the villager; the positive control (the
    /// same spec without the rod, through the same path, ironspec Nov 13 00:27 (A)) lists the
    /// villager struck.
    #[test]
    fn rod_takes_the_strikes() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let cells = [(b(0, 0, 0), "minecraft:glass"), (b(10, 0, 10), "minecraft:glass"), (b(10, 1, 10), "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]")];
        let ents = vec![("minecraft:villager".to_string(), [0.5, 1.0, 0.5], None)];
        let opts = Opts { positions: Vec::new(), rails: false };
        let spec = spec_of(&cells, ents.clone());
        let (l, n, s) = check(&spec, &opts).unwrap();
        assert_eq!((n, s), (0, 1), "{l:?}");
        let bare = spec_of(&cells[..2], ents.clone());
        let (l2, n2, _) = check(&bare, &opts).unwrap();
        assert!(n2 > 0 && l2.iter().any(|x| x.starts_with("  STRUCK the villager")), "{l2:?}");
        // (no farm entity at all: nothing judged)
        let empty = spec_of(&cells, Vec::new());
        let (le, _, se) = check(&empty, &opts).unwrap();
        assert!(se == 0 && le[0].ends_with("(NOT JUDGED: 0 samples)"), "{le:?}");
        // (the rod beside full blocks: a fire survives on their tops, (c) fails)
        let mut on_stone: Vec<(BlockPos, &str)> = cells.to_vec();
        on_stone.extend([(b(9, 0, 10), "minecraft:stone"), (b(11, 0, 10), "minecraft:stone")]);
        let (ls, ns, _) = check(&spec_of(&on_stone, ents), &opts).unwrap();
        assert!(ns > 0 && ls.iter().any(|x| x.contains("an ignition surviving in the rod's 3x3x3")), "{ls:?}");
    }
}
