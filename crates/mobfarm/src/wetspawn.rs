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
/// (ironspec Nov 12 22:34 (5a)) The 26.3 table from the game's own data (redsim's biome table,
/// 26.3's `Biome.getMobSettings`): every biome among `biomes` whose spawn lists hold a water mob or
/// the drowned, each spawner with its category, weight and group size, and the rules each species
/// spawns under (placement IN_WATER for all but the axolotl's ON_GROUND-like clay rule).
pub fn biome_table(biomes: &[String]) -> Vec<String> {
    const WATERY: [&str; 10] = ["drowned", "squid", "glow_squid", "dolphin", "cod", "salmon", "pufferfish", "tropical_fish", "axolotl", "guardian"];
    let mut out = vec![
        "the 26.3 table (redsim's terrain/biomes.tsv, 26.3's biome spawn settings; the rules as tick-exact cited them): biome: category type weight (group min-max)".to_string(),
        "  rules: drowned (monster; Drowned 139-156): IN_WATER, water below and at the cell, block light 0 (the overworld's monster limit; the sky and raw-brightness draws can pass at night or in a thunderstorm), y < sea level - 5 unless the biome is tagged more_frequent_drowned_spawns (river, frozen_river); glow squid (underground_water_creature; GlowSquid 111-119): y <= sea level - 33, raw brightness 0, a water block; squid, dolphin (water_creature; AgeableWaterCreature 66-79), cod, salmon, pufferfish, tropical fish (water_ambient; WaterAnimal 70-82): sea level - 13 <= y <= sea level, water below, a water block above; axolotl (axolotls; Axolotl 563-571): clay below".to_string(),
    ];
    for name in biomes {
        let Some(bio) = redsim::terrain::biome::biome(name) else { continue };
        let rows: Vec<String> = bio
            .spawners
            .iter()
            .filter(|s| WATERY.contains(&s.ty.trim_start_matches("minecraft:")))
            .map(|s| format!("{} {} {} ({}-{})", s.category, s.ty.trim_start_matches("minecraft:"), s.weight, s.min, s.max))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let tag = if bio.has_tag("minecraft:more_frequent_drowned_spawns") || bio.has_tag("more_frequent_drowned_spawns") { " [more_frequent_drowned_spawns: no depth rule]" } else { "" };
        out.push(format!("  {name}{tag}: {}", rows.join("; ")));
    }
    out
}

/// (farmopt Nov 13 00:27: b8949c96's C7 leak, which a static reading the stored water missed) The
/// settled water: the farm's world built by redsim and run through its settle (fluids scheduled
/// and flowed as the game's, the infinite-water rule included), every cell of the design's box
/// that holds water then; each cell whose water differs from the file's, and those cells' settled
/// states (the statics judge the settled water: a current stored as air is water above a column's
/// top).
#[allow(clippy::type_complexity)]
pub fn settled_wet(spec: &FarmSpec) -> Result<(Vec<BlockPos>, Vec<String>, usize, Vec<(BlockPos, Block)>), String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    let mut run = Run::start(spec, &o)?;
    // (ironspec Nov 13 00:27 (B): every fluid cell's tick scheduled by the build, counted)
    let scheduled = run.world.fluid_ticks.count();
    while run.settle_tick() {}
    let w = &run.world;
    let map: HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    let mut wet = Vec::new();
    let mut diffs = Vec::new();
    let mut changed: Vec<(BlockPos, Block)> = Vec::new();
    for y in spec.lo.y..=spec.hi.y {
        for z in spec.lo.z..=spec.hi.z {
            for x in spec.lo.x..=spec.hi.x {
                let p = BlockPos::new(x, y, z);
                let now_b = w.get(p);
                let now = redsim::fluid::fluid_state_of(&now_b).fluid.is_water();
                let file = map.get(&p).is_some_and(|b| redsim::fluid::fluid_state_of(b).fluid.is_water());
                if now {
                    wet.push(p);
                }
                if now != file {
                    diffs.push(format!("{x},{y},{z} {} -> {}", map.get(&p).map_or("air".to_string(), |b| b.to_string().trim_start_matches("minecraft:").to_string()), now_b.to_string().trim_start_matches("minecraft:")));
                    changed.push((p, now_b));
                }
            }
        }
    }
    Ok((wet, diffs, scheduled, changed))
}

/// (ironspec Nov 13 00:27 (D)) The water-rule static: every water cell (source, flowing or
/// waterlogged) with what each face holds; FLAG every air or washed-away horizontal neighbour (a
/// spill: the water flows into it) and every air cell under water (it falls), and every dry cell
/// (air, washed away or a dry waterloggable block) with at least 2 horizontal water sources beside
/// it over a solid or source floor (FlowingFluid.getNewLiquid's source rule, the gamerule's default
/// `waterSourceConversion` true: it becomes a source, a dry waterloggable block waterlogged).
/// Returns the lines, the flags and the samples (the water cells).
pub fn water_rules(spec: &FarmSpec, extra: &[BlockPos]) -> (Vec<String>, usize, usize) {
    use redsim::pos::Dir;
    let map: HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    let name = |p: &BlockPos| map.get(p).map_or("air".to_string(), |b| b.to_string().trim_start_matches("minecraft:").to_string());
    let id = |p: &BlockPos| name(p).split('[').next().unwrap_or("").to_string();
    let air = |p: &BlockPos| map.get(p).is_none_or(|b| b.is_air());
    let fluid = |p: &BlockPos| -> Option<redsim::fluid::FluidState> {
        if extra.contains(p) {
            return Some(redsim::fluid::FluidState::source(false));
        }
        map.get(p).map(|b| redsim::fluid::fluid_state_of(b)).filter(|f| f.fluid.is_water())
    };
    let water = |p: &BlockPos| fluid(p).is_some();
    let source = |p: &BlockPos| fluid(p).is_some_and(|f| f.is_source());
    // (WASHED_AWAY_BY_FLUIDS's families a farm holds: water replaces them; the waterloggable
    // families: dry, they take only a source)
    const WASHED: [&str; 12] = ["torch", "redstone_wire", "repeater", "comparator", "lever", "button", "carpet", "pressure_plate", "tripwire", "flower_pot", "sapling", "short_grass"];
    const LOGGABLE: [&str; 16] = ["rail", "slab", "stairs", "trapdoor", "fence", "glass_pane", "iron_bars", "_wall", "sign", "lantern", "chain", "leaves", "scaffolding", "ladder", "lightning_rod", "chest"];
    let washed = |p: &BlockPos| {
        let i = id(p);
        WASHED.iter().any(|w| i.contains(w)) && !water(p)
    };
    let loggable_dry = |p: &BlockPos| {
        let i = id(p);
        LOGGABLE.iter().any(|w| i.contains(w)) && !water(p)
    };
    let mut cells: Vec<BlockPos> = spec.blocks.iter().map(|(p, _)| *p).chain(extra.iter().copied()).filter(|p| water(p)).collect();
    cells.sort_by_key(|p| (p.y, p.z, p.x));
    cells.dedup();
    let horiz = [Dir::North, Dir::South, Dir::West, Dir::East];
    let mut lines: Vec<String> = Vec::new();
    let mut flags: Vec<String> = Vec::new();
    let mut faces: Vec<String> = Vec::new();
    for w in &cells {
        let kind = match fluid(w) {
            Some(f) if f.is_source() && matches!(map.get(w), Some(Block::Water { .. })) => "source",
            Some(f) if f.is_source() => "waterlogged or column",
            _ => "flowing",
        };
        let mut row = Vec::new();
        for d in horiz.iter().chain([Dir::Down, Dir::Up].iter()) {
            let n = w.rel(*d);
            let what = if water(&n) { "water".to_string() } else { name(&n).split('[').next().unwrap_or("").to_string() };
            let mut tag = String::new();
            if horiz.contains(d) && (air(&n) || washed(&n)) {
                tag = " SPILL".to_string();
                flags.push(format!("SPILL {},{},{} ({kind}) flows {:?} into {},{},{} ({what})", w.x, w.y, w.z, d, n.x, n.y, n.z));
            }
            if *d == Dir::Down && air(&n) {
                tag = " FALLS".to_string();
                flags.push(format!("FALLS {},{},{} ({kind}) falls into {},{},{} (air)", w.x, w.y, w.z, n.x, n.y, n.z));
            }
            row.push(format!("{d:?} {what}{tag}"));
        }
        faces.push(format!("  WATER {},{},{} ({kind}): {}", w.x, w.y, w.z, row.join(", ")));
    }
    // (the source rule: each dry cell beside 2+ horizontal sources)
    let mut cand: Vec<BlockPos> = cells.iter().flat_map(|w| horiz.iter().map(move |d| w.rel(*d))).filter(|c| !water(c) && (air(c) || washed(c) || loggable_dry(c))).collect();
    cand.sort_by_key(|p| (p.y, p.z, p.x));
    cand.dedup();
    for c in &cand {
        let k = horiz.iter().filter(|d| source(&c.rel(**d))).count();
        let floor = c.rel(Dir::Down);
        let solid = map.get(&floor).is_some_and(|b| redsim::behavior::is_solid(b));
        if k >= 2 && (solid || source(&floor)) {
            flags.push(format!("SOURCE {},{},{} ({}) has {k} horizontal sources over {} ({}): it becomes a source (FlowingFluid.getNewLiquid)", c.x, c.y, c.z, name(c).split('[').next().unwrap_or(""), name(&floor).split('[').next().unwrap_or(""), if source(&floor) { "a source" } else { "solid" }));
        }
    }
    lines.push(format!(
        "water rules (ironspec Nov 13 00:27 (D): every water cell's faces; a spill into air or a washed-away block, a fall into air, and FlowingFluid.getNewLiquid's source rule at a dry cell between 2+ horizontal sources over a solid or source floor): {} water cells, {} flags{}",
        cells.len(),
        flags.len(),
        if cells.is_empty() { " (NOT JUDGED: 0 water cells)" } else if flags.is_empty() { " (PASS)" } else { " (FAIL)" }
    ));
    for f in &flags {
        lines.push(format!("  {f}"));
    }
    lines.extend(faces);
    (lines, flags.len(), cells.len())
}

/// The spec the spawn statics judge (G1 Nov 13 02:21: the scan's "a water block above" read the
/// file's blocks, so a current the settle makes, stored as air, was missed): the design with every
/// cell the settle changed in its settled state; with the settle's lines (the changed cells) and
/// the fluid ticks the build scheduled.
pub fn settled_spec(spec: &FarmSpec) -> Result<(FarmSpec, Vec<BlockPos>, Vec<String>, usize), String> {
    let (wet, diffs, scheduled, changed) = settled_wet(spec)?;
    let mut s = spec.clone();
    for (p, b) in &changed {
        s.blocks.retain(|(q, _)| q != p);
        if !b.is_air() {
            s.blocks.push((*p, *b));
        }
    }
    Ok((s, wet, diffs, scheduled))
}

/// The water-holding cells the wet-cell statics judge (their samples, ironspec Nov 12 23:56):
/// every cell whose block holds water, plus the `--wet` cells.
pub fn wet_cells_of(spec: &FarmSpec, extra: &[BlockPos]) -> Vec<BlockPos> {
    let mut wet: Vec<BlockPos> = spec.blocks.iter().filter(|(p, b)| extra.contains(p) || redsim::fluid::fluid_state_of(b).fluid.is_water()).map(|(p, _)| *p).chain(extra.iter().copied()).collect();
    wet.sort_by_key(|p| (p.y, p.z, p.x));
    wet.dedup();
    wet
}

/// ONE STATE SOURCE (G1 Nov 13 02:23 (b)): every condition below (the cell's water, the water below,
/// the water block above, the conductor above, the collision, the clay) reads `spec`, the world the
/// caller judges: the CLI passes [`settled_spec`] (the design with every cell the settle changed in
/// its settled state); nothing here reads the design file. `wet_cells`: cells counted as holding
/// water besides (a `--wet` cell).
pub fn scan(spec: &FarmSpec, wet_cells: &[BlockPos], declared: &[(BlockPos, String)]) -> Vec<String> {
    let map: HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    let water = |p: &BlockPos| wet_cells.contains(p) || map.get(p).is_some_and(|b| redsim::fluid::fluid_state_of(b).fluid.is_water());
    let water_block = |p: &BlockPos| map.get(p).is_some_and(|b| matches!(b, Block::Water { .. }));
    let conductor = |p: &BlockPos| map.get(p).is_some_and(|b| redsim::behavior::conductor_of(b));
    let clay = |p: &BlockPos| map.get(p).is_some_and(|b| b.to_string() == "minecraft:clay");
    let wet = wet_cells_of(spec, wet_cells);
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

/// (ironspec Nov 12 22:34 (5) / 22:36 / 22:54) The site a wet-spawn evaluation assumes: the
/// allowed biomes, the sea level and the absolute y of the module's y 0 (`y0`: MIN..MAX).
#[derive(Clone, Debug)]
pub struct Site {
    pub biomes: Vec<String>,
    pub sea_level: i32,
    pub y0: (i32, i32),
}

/// The nether's and the end's biomes (the overworld: every other row of 26.3's biome table).
const NOT_OVERWORLD: [&str; 11] = ["minecraft:basalt_deltas", "minecraft:crimson_forest", "minecraft:nether_wastes", "minecraft:soul_sand_valley", "minecraft:warped_forest", "minecraft:end_barrens", "minecraft:end_highlands", "minecraft:end_midlands", "minecraft:small_end_islands", "minecraft:the_end", "minecraft:the_void"];

/// Every biome id of 26.3's table (redsim's `terrain/biomes.tsv`).
fn all_biomes() -> Vec<String> {
    redsim::terrain::biome::TSV.lines().skip(2).filter_map(|l| l.split('\t').next()).filter(|n| n.starts_with("minecraft:")).map(str::to_string).collect()
}

/// Every overworld biome id of 26.3's table (all but the nether's and the end's).
pub fn overworld_biomes() -> Vec<String> {
    all_biomes().into_iter().filter(|n| !NOT_OVERWORLD.contains(&n.as_str())).collect()
}

impl Site {
    /// `biomes=overworld,!minecraft:river,...; sea_level=63; y0=-36..` (the header's `# site:`
    /// or the `--site` flag): `overworld` every overworld biome, `!id` excluded; `y0=A..B` (or
    /// `A..`: up to the build limit) the module's y 0 at absolute A..B.
    pub fn parse(s: &str) -> Result<Site, String> {
        let mut site = Site { biomes: Vec::new(), sea_level: 63, y0: (i32::MIN, i32::MIN) };
        let mut include: Vec<String> = Vec::new();
        let mut exclude: Vec<String> = Vec::new();
        for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=').ok_or_else(|| format!("site {part}: key=value"))?;
            match k.trim() {
                "biomes" => {
                    for b in v.split(',').map(str::trim).filter(|b| !b.is_empty()) {
                        match b.strip_prefix('!') {
                            Some(x) => exclude.push(x.to_string()),
                            None => include.push(b.to_string()),
                        }
                    }
                }
                "sea_level" => site.sea_level = v.trim().parse().map_err(|e| format!("site sea_level {v}: {e}"))?,
                "y0" => {
                    let (a, b) = v.split_once("..").ok_or_else(|| format!("site y0 {v}: A..B or A.."))?;
                    let a: i32 = a.trim().parse().map_err(|e| format!("site y0 {v}: {e}"))?;
                    let b: i32 = if b.trim().is_empty() { 320 } else { b.trim().parse().map_err(|e| format!("site y0 {v}: {e}"))? };
                    site.y0 = (a.min(b), a.max(b));
                }
                other => return Err(format!("site: unknown key {other} (biomes, sea_level, y0)")),
            }
        }
        if site.y0.0 == i32::MIN {
            return Err("site: y0=A..B (the module's y 0 at absolute A..B) is required".to_string());
        }
        let table = all_biomes();
        for b in include {
            if b == "overworld" {
                site.biomes.extend(table.iter().filter(|n| !NOT_OVERWORLD.contains(&n.as_str())).cloned());
            } else if table.contains(&b) {
                site.biomes.push(b);
            } else {
                return Err(format!("site: {b} is not a biome of 26.3's table"));
            }
        }
        site.biomes.retain(|b| !exclude.contains(b));
        site.biomes.sort();
        site.biomes.dedup();
        if site.biomes.is_empty() {
            return Err("site: no biome allowed (biomes=overworld,!id,... or a list)".to_string());
        }
        Ok(site)
    }

    /// The design's `# site:` line (comment or meta-line form), if any.
    pub fn from_text(text: &str) -> Result<Option<Site>, String> {
        for l in text.lines() {
            let t = l.trim_start();
            let t = t.strip_prefix('#').map(str::trim_start).unwrap_or(t);
            if let Some(rest) = t.strip_prefix("site:") {
                return Site::parse(rest).map(Some);
            }
        }
        Ok(None)
    }
}

/// (ironspec Nov 12 22:34 (5)) The wet cells under a declared site: each allowed biome's water mobs
/// and drowned (26.3's spawn lists, redsim's biome table) at each water-holding cell, with the
/// species' rules as cited by tick-exact: drowned (Drowned 139-156): water below and at the cell,
/// dark enough (block light 0, the overworld's monster limit; the sky and raw-brightness draws
/// can pass at night or in a thunderstorm), and in a biome without the more-frequent-drowned tag
/// y < sea level - 5; glow squid (GlowSquid 111-119): y <= sea level - 33, raw brightness 0 (sky
/// and block light 0) and the cell a water block; squid, dolphin, cod, salmon, pufferfish,
/// tropical fish (AgeableWaterCreature 66-79, WaterAnimal 70-82): sea level - 13 <= y <= sea
/// level, water below and a water block above; axolotl (Axolotl 563-571): clay below. Each cell's
/// light as composed (the module built alone under the open sky: its sky light, its block light,
/// the raw brightness at night (sky - 11) and in a thunderstorm (sky - 10)). Returns the lines
/// and the candidates outside the declared `water-spawn:` cells. ONE STATE SOURCE, as [`scan`]:
/// every condition and the light read `spec`, the settled world the caller passes.
pub fn site_scan(spec: &FarmSpec, wet_cells: &[BlockPos], declared: &[(BlockPos, String)], site: &Site) -> Result<(Vec<String>, usize), String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let run = if spec.has_bed() { Run::new(spec, &RunOpts::steady(0, 0, WalkMode::Random)) } else { Run::new_homeless(spec, &RunOpts::steady(0, 0, WalkMode::Random)) }?;
    let w = run.world;
    let map: HashMap<BlockPos, &Block> = spec.blocks.iter().map(|(p, b)| (*p, b)).collect();
    let water = |p: &BlockPos| wet_cells.contains(p) || map.get(p).is_some_and(|b| redsim::fluid::fluid_state_of(b).fluid.is_water());
    let water_block = |p: &BlockPos| map.get(p).is_some_and(|b| matches!(b, Block::Water { .. }));
    let conductor = |p: &BlockPos| map.get(p).is_some_and(|b| redsim::behavior::conductor_of(b));
    let clay = |p: &BlockPos| map.get(p).is_some_and(|b| b.to_string() == "minecraft:clay");
    let mut wet: Vec<BlockPos> = spec.blocks.iter().filter(|(p, _)| water(p)).map(|(p, _)| *p).chain(wet_cells.iter().copied()).collect();
    wet.sort_by_key(|p| (p.x, p.z, p.y));
    wet.dedup();
    // (each wet cell's light as composed)
    let mut light: HashMap<BlockPos, (i32, i32)> = HashMap::new();
    for p in &wet {
        let sky = redsim::daylight::sky_light(&w, *p)?;
        let blk = redsim::daylight::block_light_cached(&w, *p, 0)?;
        light.insert(*p, (sky, blk));
    }
    let sea = site.sea_level;
    let (a, b) = site.y0;
    let mut lines = vec![format!(
        "wet-cell spawns at the declared site (ironspec Nov 12 22:34 (5); 26.3's biome spawn lists): {} allowed biomes, sea level {sea}, the module's y 0 at absolute {a}..{b}; {} wet cells",
        site.biomes.len(),
        wet.len()
    )];
    // (ironspec Nov 13 02:41: every biome covered printed with the registry's count, so a missing
    // biome can't pass silently; the water-animal scope read from the spawn tables)
    let table = all_biomes();
    let overworld = overworld_biomes();
    lines.push(format!(
        "  biomes covered: {} of the table's {} ({} overworld, {} nether or end; overworld not covered: {}): {}",
        site.biomes.len(),
        table.len(),
        overworld.len(),
        table.len() - overworld.len(),
        {
            let missing: Vec<&String> = overworld.iter().filter(|o| !site.biomes.contains(o)).collect();
            if missing.is_empty() { "none".to_string() } else { missing.iter().map(|m| m.trim_start_matches("minecraft:")).collect::<Vec<_>>().join(", ") }
        },
        site.biomes.iter().map(|b| b.trim_start_matches("minecraft:")).collect::<Vec<_>>().join(", ")
    ));
    let animal_scope: Vec<&str> = overworld
        .iter()
        .filter(|n| redsim::terrain::biome::biome(n).is_some_and(|bio| bio.mobs("water_creature").next().is_some() || bio.mobs("water_ambient").next().is_some()))
        .map(|n| n.trim_start_matches("minecraft:"))
        .collect();
    lines.push(format!("  the water-animal scope (overworld biomes whose 26.3 lists hold a water_creature or water_ambient entry): {} — {}", animal_scope.len(), animal_scope.join(", ")));
    // (the light table: runs of cells in a column, sky / block, night and thunder raw brightness)
    let mut runs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < wet.len() {
        let p = wet[i];
        let (s, bl) = light[&p];
        let mut j = i;
        while j + 1 < wet.len() && wet[j + 1].x == p.x && wet[j + 1].z == p.z && wet[j + 1].y == wet[j].y + 1 && light[&wet[j + 1]] == (s, bl) {
            j += 1;
        }
        let q = wet[j];
        runs.push(format!("({},{}..{},{}) sky {s} block {bl} night {} thunder {}", p.x, p.y, q.y, p.z, (s - 11).max(bl).max(0), (s - 10).max(bl).max(0)));
        i = j + 1;
    }
    lines.push(format!("  light as composed (the module alone under the open sky): {}", runs.join("; ")));
    let mut failing = 0usize;
    for name in &site.biomes {
        let Some(bio) = redsim::terrain::biome::biome(name) else { continue };
        let river = bio.has_tag("minecraft:more_frequent_drowned_spawns") || bio.has_tag("more_frequent_drowned_spawns");
        let mut per: Vec<(String, Vec<BlockPos>)> = Vec::new();
        for cat in ["monster", "water_creature", "water_ambient", "underground_water_creature", "axolotls"] {
            for sp in bio.mobs(cat) {
                let ty = sp.ty.trim_start_matches("minecraft:");
                let rule = match ty {
                    "drowned" => "drowned",
                    "glow_squid" => "glow_squid",
                    "squid" | "dolphin" | "cod" | "salmon" | "pufferfish" | "tropical_fish" => "surface",
                    "axolotl" => "axolotl",
                    "guardian" | "elder_guardian" => "guardian",
                    _ => continue,
                };
                let mut cells = Vec::new();
                for p in &wet {
                    let (above, below) = (BlockPos::new(p.x, p.y + 1, p.z), BlockPos::new(p.x, p.y - 1, p.z));
                    if conductor(&above) || !map.get(p).is_none_or(|bk| redsim::behavior::collision_boxes(bk).is_empty()) {
                        continue;
                    }
                    let (sky, blk) = light[p];
                    let ok = match rule {
                        "drowned" => water(&below) && water(p) && blk == 0 && (river || a + p.y < sea - 5),
                        "glow_squid" => water_block(p) && a + p.y <= sea - 33 && sky == 0 && blk == 0,
                        "surface" => water(&below) && water_block(&above) && a + p.y <= sea && b + p.y >= sea - 13,
                        "axolotl" => clay(&below),
                        _ => water(&below) && water(p),
                    };
                    if ok {
                        cells.push(*p);
                    }
                }
                if !cells.is_empty() {
                    per.push((ty.to_string(), cells));
                }
            }
        }
        if per.is_empty() {
            continue;
        }
        let mut biome_fail = 0usize;
        let mut parts = Vec::new();
        for (ty, cells) in &per {
            let (dec, open): (Vec<&BlockPos>, Vec<&BlockPos>) = cells.iter().partition(|p| declared.iter().any(|(d, _)| d == *p));
            biome_fail += open.len();
            parts.push(format!(
                "{ty} {} cells{}{}",
                open.len(),
                if open.is_empty() { String::new() } else { format!(" ({})", open.iter().take(8).map(|p| format!("{},{},{}", p.x, p.y, p.z)).collect::<Vec<_>>().join(" ")) },
                if dec.is_empty() { String::new() } else { format!(", {} at declared cells", dec.len()) }
            ));
        }
        failing += biome_fail;
        lines.push(format!("  {} {name}{}: {}", if biome_fail > 0 { "FAIL" } else { "DECLARED" }, if river { " (more-frequent-drowned: no depth rule)" } else { "" }, parts.join("; ")));
    }
    lines[0].push_str(&format!(": {failing} candidates{}", if failing == 0 { " (PASS)" } else { " (FAIL)" }));
    Ok((lines, failing))
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

    /// (ironspec Nov 13 00:27 (D)) The water-rule static: a dry rail between two sources over glass
    /// becomes a source (the C7 leak's cell); over air it does not; the spills listed.
    #[test]
    fn water_rule_source_between_two_sources() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let mut cells = vec![(b(0, 0, 0), "minecraft:water[level=0]"), (b(2, 0, 0), "minecraft:water[level=0]"), (b(1, 0, 0), "minecraft:rail[shape=east_west,waterlogged=false]")];
        for x in 0..=2 {
            cells.push((b(x, -1, 0), "minecraft:glass"));
            cells.push((b(x, 0, -1), "minecraft:glass"));
            cells.push((b(x, 0, 1), "minecraft:glass"));
        }
        cells.push((b(-1, 0, 0), "minecraft:glass"));
        cells.push((b(3, 0, 0), "minecraft:glass"));
        let (l, n, s) = water_rules(&spec_of(&cells), &[]);
        assert_eq!(s, 2, "{l:?}");
        assert!(n == 1 && l.iter().any(|x| x.starts_with("  SOURCE 1,0,0 (rail) has 2 horizontal sources over glass (solid)")), "{l:?}");
        // (the floor under the rail air: no source; the water above air now falls)
        let open: Vec<(BlockPos, &str)> = cells.iter().copied().filter(|(p, _)| *p != b(1, -1, 0)).collect();
        let (l2, _, _) = water_rules(&spec_of(&open), &[]);
        assert!(!l2.iter().any(|x| x.contains("SOURCE 1,0,0")), "{l2:?}");
        // (a spill: a water cell beside air)
        let spill: Vec<(BlockPos, &str)> = cells.iter().copied().filter(|(p, _)| *p != b(-1, 0, 0)).collect();
        let (l3, _, _) = water_rules(&spec_of(&spill), &[]);
        assert!(l3.iter().any(|x| x.starts_with("  SPILL 0,0,0 (source) flows West into -1,0,0 (air)")), "{l3:?}");
    }

    /// (G1 Nov 13 02:23 (a), the general positive control through the scan's own path) Open water
    /// the official rules allow is FOUND: a water column three deep (stored water blocks) at an ocean
    /// site with its middle cell in sea level - 13..sea level admits the squid and the cod there.
    #[test]
    fn surface_water_found_at_an_ocean_site() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let mut cells = vec![(b(0, 0, 0), "minecraft:water[level=0]"), (b(0, 1, 0), "minecraft:water[level=0]"), (b(0, 2, 0), "minecraft:water[level=0]"), (b(0, -1, 0), "minecraft:glass")];
        for y in 0..=2 {
            for (x, z) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                cells.push((b(x, y, z), "minecraft:glass"));
            }
        }
        let column = spec_of(&cells);
        let ocean = Site::parse("biomes=minecraft:ocean; sea_level=63; y0=60..60").unwrap();
        let (s, _, _, _) = settled_spec(&column).unwrap();
        let (l, n) = site_scan(&s, &[], &[], &ocean).unwrap();
        assert!(n >= 2 && l.iter().any(|x| x.starts_with("  FAIL minecraft:ocean") && x.contains("squid 1 cells (0,1,0)") && x.contains("cod 1 cells (0,1,0)")), "{l:?}");
    }

    /// (G1 Nov 13 02:21: the positive control through the scan's own path) A cell the official rules
    /// allow is FOUND: a water column's top whose water block above exists only after the settle (a
    /// source beside the air over it flows into it, as record7f's lift currents) admits the squid
    /// at an ocean site with the cell in sea level - 13..sea level, judged on `settled_spec` as the
    /// CLI judges; on the file's blocks it is missed (the bug this control pins).
    #[test]
    fn settled_current_admits_the_surface_rule() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let mut cells = vec![(b(0, -1, 0), "minecraft:water[level=0]"), (b(0, 0, 0), "minecraft:water[level=0]"), (b(1, 1, 0), "minecraft:water[level=0]")];
        // (the column's walls and floor, the source's floor and walls but its west side, open
        // onto the air over the column)
        for (x, y, z) in [(0, -2, 0), (-1, -1, 0), (1, -1, 0), (0, -1, -1), (0, -1, 1), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1), (2, 1, 0), (1, 1, -1), (1, 1, 1), (-1, 1, 0), (0, 1, -1), (0, 1, 1)] {
            cells.push((b(x, y, z), "minecraft:glass"));
        }
        let spec = spec_of(&cells);
        let ocean = Site::parse("biomes=minecraft:ocean; sea_level=63; y0=60..60").unwrap();
        let (s, _, diffs, _) = settled_spec(&spec).unwrap();
        assert!(diffs.iter().any(|d| d.starts_with("0,1,0 air -> water")), "the settle makes the current: {diffs:?}");
        let (l, n) = site_scan(&s, &[], &[], &ocean).unwrap();
        assert!(n >= 1 && l.iter().any(|x| x.starts_with("  FAIL minecraft:ocean") && x.contains("squid 1 cells (0,0,0)")), "{l:?}");
        let (_, n_file) = site_scan(&spec, &[], &[], &ocean).unwrap();
        assert_eq!(n_file, 0, "on the file's blocks the current is air: no water block above");
    }

    /// (ironspec Nov 12 22:36) The site evaluation: a dark water column (water below and at, block
    /// light 0) admits drowned at a river site at any height (the positive control), none at an
    /// ocean site far above sea level - 5, and drowned again when the module may sit low.
    #[test]
    fn site_river_fails_high_ocean_passes() {
        let b = |x, y, z| BlockPos::new(x, y, z);
        let column = spec_of(&[(b(0, 0, 0), "minecraft:water[level=0]"), (b(0, 1, 0), "minecraft:water[level=0]"), (b(0, 2, 0), "minecraft:water[level=0]"), (b(0, -1, 0), "minecraft:glass")]);
        let river = Site::parse("biomes=minecraft:river; sea_level=63; y0=100..").unwrap();
        let (l, n) = site_scan(&column, &[], &[], &river).unwrap();
        assert!(n >= 2 && l.iter().any(|x| x.starts_with("  FAIL minecraft:river (more-frequent-drowned") && x.contains("drowned 2 cells")), "{l:?}");
        let high = Site::parse("biomes=minecraft:ocean; sea_level=63; y0=100..").unwrap();
        let (h, m) = site_scan(&column, &[], &[], &high).unwrap();
        assert_eq!(m, 0, "{h:?}");
        let low = Site::parse("biomes=minecraft:ocean; sea_level=63; y0=0..").unwrap();
        let (_, k) = site_scan(&column, &[], &[], &low).unwrap();
        assert!(k >= 2, "an ocean site with the module low: drowned below sea level - 5");
        let ow = Site::parse("biomes=overworld,!minecraft:river,!minecraft:frozen_river; y0=64..").unwrap();
        assert!(!ow.biomes.contains(&"minecraft:river".to_string()) && ow.biomes.contains(&"minecraft:plains".to_string()) && !ow.biomes.contains(&"minecraft:nether_wastes".to_string()));
        assert!(Site::parse("biomes=minecraft:not_a_biome; y0=0..").is_err());
        // (ironspec Nov 13 02:41: the covered biomes and the registry's count printed, the uncovered
        // overworld biomes named; the water-animal scope from the tables)
        let (lo, _) = site_scan(&column, &[], &[], &ow).unwrap();
        assert!(lo.iter().any(|x| x.starts_with("  biomes covered: ") && x.contains("overworld not covered: frozen_river, river)")), "{lo:?}");
        assert!(lo.iter().any(|x| x.starts_with("  the water-animal scope") && x.contains("lush_caves") && x.contains("mangrove_swamp") && x.contains("warm_ocean") && !x.contains("plains")), "{lo:?}");
    }
}
