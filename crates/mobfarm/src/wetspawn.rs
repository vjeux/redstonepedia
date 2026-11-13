//! (9fc3155f, ironspec Nov 12 19:41 (3), tick-exact's 26.3 citations) The wet-cell spawn static:
//! every cell of the design holding water (its fluid state water: a water block, a bubble column,
//! kelp, a waterlogged block) as an IN_WATER spawn candidate for each water mob and the drowned,
//! with the game's static tests (the y bands, the light, the odds and the world border are taken as
//! passing: the worst case):
//! - placement (SpawnPlacementTypes IN_WATER): the fluid at the cell is water and the cell above is
//!   not a redstone conductor;
//! - `noCollision` of the spawn box at (x + 0.5, y, z + 0.5): the cell's own collision shape empty
//!   (a waterlogged rail has none; the boxes reaching the cell above, the drowned's, are not
//!   tested: a candidate is kept, the worst case);
//! - the species rules: squid and dolphin (AgeableWaterCreature) and cod, salmon, pufferfish and
//!   tropical fish (WaterAnimal): the fluid below is water and the block above is a water block;
//!   glow squid: the block itself is a water block; drowned: the fluid below is water; guardian:
//!   water at the cell and the fluid below water; axolotl: clay below.
use redsim::block::Block;
use redsim::entity::mob::farm::FarmSpec;
use redsim::pos::BlockPos;
use std::collections::HashMap;

/// The species and the test each needs beyond the placement and the box.
pub const SPECIES: [(&str, &str); 10] = [
    ("squid", "surface"),
    ("dolphin", "surface"),
    ("cod", "surface"),
    ("salmon", "surface"),
    ("pufferfish", "surface"),
    ("tropical_fish", "surface"),
    ("glow_squid", "water_block"),
    ("drowned", "water_below"),
    ("guardian", "water_below"),
    ("axolotl", "clay_below"),
];

/// Each wet cell's verdict per species: the candidates (every static test passed) and, per
/// species, the count failing each test. `wet`: cells counted as holding water whatever the
/// design's parse keeps (a waterlogged rail: redsim's rails carry no water, so a design's
/// `waterlogged=true` rail is named here; its state stays the rail, its shape none). `declared`:
/// the design's `# water-spawn: CELLS=REASON (evidence)` exceptions (G1 on fdf7b4302: the owner's
/// stated basis, light or the site's biome and y): a candidate there is printed apart with its
/// reason and does not fail.
pub fn scan(spec: &FarmSpec, wet_cells: &[BlockPos], declared: &[(BlockPos, String)]) -> Vec<String> {
    let map: HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    let water = |p: &BlockPos| wet_cells.contains(p) || map.get(p).is_some_and(|b| redsim::fluid::fluid_state_of(b).fluid.is_water());
    let water_block = |p: &BlockPos| map.get(p).is_some_and(|b| matches!(b, Block::Water { .. }));
    let conductor = |p: &BlockPos| map.get(p).is_some_and(|b| redsim::behavior::conductor_of(b));
    let clay = |p: &BlockPos| map.get(p).is_some_and(|b| b.to_string() == "minecraft:clay");
    let mut wet: Vec<BlockPos> = spec.blocks.iter().filter(|(p, _)| water(p)).map(|(p, _)| *p).chain(wet_cells.iter().copied()).collect();
    wet.sort_by_key(|p| (p.y, p.z, p.x));
    wet.dedup();
    let mut cands: Vec<(String, BlockPos)> = Vec::new();
    let mut fails: HashMap<(&str, &str), usize> = HashMap::new();
    for p in &wet {
        let (above, below) = (BlockPos::new(p.x, p.y + 1, p.z), BlockPos::new(p.x, p.y - 1, p.z));
        let placed = !conductor(&above);
        let free = map.get(p).is_none_or(|b| redsim::behavior::collision_boxes(b).is_empty());
        for (sp, rule) in SPECIES {
            let why = if !placed {
                Some("the cell above a conductor")
            } else if !free {
                Some("a collision shape in the cell")
            } else {
                match rule {
                    "surface" if !water(&below) => Some("no water below"),
                    "surface" if !water_block(&above) => Some("no water block above"),
                    "water_block" if !water_block(p) => Some("the cell not a water block"),
                    "water_below" if !water(&below) => Some("no water below"),
                    "clay_below" if !clay(&below) => Some("no clay below"),
                    _ => None,
                }
            };
            match why {
                Some(w) => *fails.entry((sp, w)).or_default() += 1,
                None => cands.push((sp.to_string(), *p)),
            }
        }
    }
    let (excused, failing): (Vec<(String, BlockPos)>, Vec<(String, BlockPos)>) = cands.iter().cloned().partition(|(_, p)| declared.iter().any(|(d, _)| d == p));
    let mut lines = vec![format!(
        "wet-cell spawns (ironspec Nov 12 19:41 (3): each water-holding cell an IN_WATER candidate for every water mob and the drowned; the y bands, light and odds taken as passing): {} wet cells, {} candidates, {} at declared `water-spawn:` cells{}",
        wet.len(),
        failing.len(),
        excused.len(),
        if failing.is_empty() { " (PASS)" } else { " (FAIL)" }
    )];
    for (sp, _) in SPECIES {
        let mut why: Vec<(&str, usize)> = fails.iter().filter(|((s, _), _)| *s == sp).map(|((_, w), n)| (*w, *n)).collect();
        why.sort();
        let here: Vec<String> = failing.iter().filter(|(s, _)| s == sp).map(|(_, p)| format!("{},{},{}", p.x, p.y, p.z)).collect();
        let decl = excused.iter().filter(|(s, _)| s == sp).count();
        lines.push(format!(
            "  {sp}: {} candidates{}{}; refused: {}",
            here.len(),
            if here.is_empty() { String::new() } else { format!(" ({})", here.join(" ")) },
            if decl == 0 { String::new() } else { format!(", {decl} at declared cells") },
            if why.is_empty() { "none".to_string() } else { why.iter().map(|(w, n)| format!("{w} {n}")).collect::<Vec<_>>().join(", ") }
        ));
    }
    // (each declared cell once, with the species it would admit and the owner's reason)
    let mut seen: Vec<BlockPos> = Vec::new();
    for (_, p) in &excused {
        if seen.contains(p) {
            continue;
        }
        seen.push(*p);
        let sps: Vec<&str> = excused.iter().filter(|(_, q)| q == p).map(|(s, _)| s.as_str()).collect();
        let why = declared.iter().find(|(d, _)| d == p).map(|(_, r)| r.as_str()).unwrap_or("");
        lines.push(format!("  DECLARED {},{},{} ({}): {why}", p.x, p.y, p.z, sps.join(" ")));
    }
    lines
}

/// The design's `water-spawn:` exceptions (comment `# water-spawn: ...` or meta-line form), each
/// entry `x,y,z+x,y,z...=REASON (evidence)`, entries `;`-separated.
pub fn declared_from_text(text: &str) -> Result<Vec<(BlockPos, String)>, String> {
    let key = "water-spawn:";
    let lines: String = text
        .lines()
        .filter_map(|l| {
            let t = l.trim_start();
            let t = t.strip_prefix('#').map(str::trim_start).unwrap_or(t);
            t.strip_prefix(key).map(|rest| format!("# {key}{rest}\n"))
        })
        .collect();
    let mut out = Vec::new();
    for e in crate::pushscan::header_entries(&lines, key) {
        let (cs, why) = e.split_once('=').ok_or_else(|| format!("water-spawn {e}: x,y,z+...=REASON"))?;
        for c in cs.split('+') {
            let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|err| format!("water-spawn {e}: {err}"))?;
            if v.len() != 3 {
                return Err(format!("water-spawn {e}: x,y,z+...=REASON"));
            }
            out.push((BlockPos::new(v[0], v[1], v[2]), why.trim().to_string()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_of(cells: &[(BlockPos, &str)]) -> FarmSpec {
        let blocks = cells.iter().map(|(p, b)| (*p, Block::parse(b).unwrap())).collect();
        FarmSpec { blocks, entities: Vec::new(), output: BlockPos::new(4, 4, 4), lo: BlockPos::new(-4, -4, -4), hi: BlockPos::new(4, 6, 4), block_entities: Vec::new(), rock: false, nether: None, land: false }
    }

    /// A waterlogged slab over glass with air above (a single wet layer): no species passes; a water
    /// column three deep: the surface species, the glow squid, the drowned and the guardian pass at
    /// its middle cell, the axolotl not (no clay).
    #[test]
    fn wet_layer_and_water_column() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let layer = spec_of(&[(b(0, 0, 0), "minecraft:glass"), (b(0, 1, 0), "minecraft:smooth_stone_slab[type=bottom,waterlogged=true]")]);
        let l = scan(&layer, &[], &[]);
        assert!(l[0].contains("1 wet cells, 0 candidates"), "{l:?}");
        // (a waterlogged rail named wet over glass, air above: the roll's case: 0 candidates)
        let roll = spec_of(&[(b(0, 0, 0), "minecraft:glass"), (b(0, 1, 0), "minecraft:rail[shape=east_west,waterlogged=false]")]);
        let r = scan(&roll, &[b(0, 1, 0)], &[]);
        assert!(r[0].contains("1 wet cells, 0 candidates"), "{r:?}");
        assert!(r.iter().any(|x| x.starts_with("  glow_squid: 0 candidates; refused: the cell not a water block 1")), "{r:?}");
        let column = spec_of(&[(b(0, 0, 0), "minecraft:water[level=0]"), (b(0, 1, 0), "minecraft:water[level=0]"), (b(0, 2, 0), "minecraft:water[level=0]")]);
        let c = scan(&column, &[], &[]);
        assert!(c.iter().any(|r| r.starts_with("  squid: 1 candidates (0,1,0)")), "{c:?}");
        // (the glow squid needs only the cell a water block: every cell of the column)
        assert!(c.iter().any(|r| r.starts_with("  glow_squid: 3 candidates (0,0,0 0,1,0 0,2,0)")), "{c:?}");
        assert!(c.iter().any(|r| r.starts_with("  axolotl: 0 candidates")), "{c:?}");
        // (G1 on fdf7b4302: a declared `water-spawn:` cell is printed apart and does not fail; the
        // header in its comment and meta-line forms)
        let decl = declared_from_text("# water-spawn: 0,0,0+0,1,0+0,2,0=lit to 1 (bench), a site rule (evidence)\nname: x\n").unwrap();
        assert_eq!(decl.len(), 3);
        assert_eq!(declared_from_text("water-spawn: 0,1,0=the meta form\n").unwrap(), vec![(b(0, 1, 0), "the meta form".to_string())]);
        let d = scan(&column, &[], &decl);
        assert!(d[0].contains("0 candidates, ") && d[0].ends_with("(PASS)"), "{d:?}");
        assert!(d.iter().any(|r| r.starts_with("  DECLARED 0,1,0 (") && r.ends_with("lit to 1 (bench), a site rule (evidence)")), "{d:?}");
        let partial = scan(&column, &[], &decl[..1]);
        assert!(partial[0].ends_with("(FAIL)"), "an undeclared cell still fails: {partial:?}");
    }
}
