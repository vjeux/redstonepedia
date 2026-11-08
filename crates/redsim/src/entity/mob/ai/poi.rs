//! Points of interest as the AI reads them (26.3 `PoiManager`, `PoiSection`, `PoiRecord`,
//! `PoiTypes`, `ServerLevel.isCloseToVillage` / `sectionsToVillage`).
//!
//! The index lists the POI blocks of the world (beds' heads: HOME, one ticket; bells: MEETING, 32;
//! the job sites, one each), rebuilt after a POI block changes. A ticket is taken when a villager
//! holds the POI in its HOME memory (beds: the only POIs the modeled villagers claim). A section
//! is a village centre when it holds a village POI with a ticket taken (`isVillageCenter`:
//! `IS_OCCUPIED`); `sectionsToVillage` is the distance tracker's level, the Chebyshev distance in
//! sections to the nearest centre, at most 7.

use super::brain::tree::PoiSel;
use crate::block::Block;
use crate::entity::EntityKind;
use crate::pos::BlockPos;
use crate::world::World;

/// A POI's type (the modeled ones).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoiKind {
    Home,
    Meeting,
    Job,
}

/// The POI type of a block, if it is one.
pub fn poi_of(b: &Block) -> Option<PoiKind> {
    match b {
        Block::Bed { head: true, .. } => Some(PoiKind::Home),
        Block::Composter { .. } | Block::BrewingStand { .. } => Some(PoiKind::Job),
        Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(*id);
            let name = p.block_id();
            if name.ends_with("_bed") {
                return p.name.contains("part=head").then_some(PoiKind::Home);
            }
            match name.trim_start_matches("minecraft:") {
                "bell" => Some(PoiKind::Meeting),
                "blast_furnace" | "smoker" | "cartography_table" | "brewing_stand" | "composter" | "barrel" | "fletching_table" | "cauldron" | "water_cauldron" | "lava_cauldron" | "powder_snow_cauldron" | "lectern" | "stonecutter" | "loom" | "smithing_table" | "grindstone" => Some(PoiKind::Job),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The POI index (built on first use, rebuilt after a POI block changes).
pub fn index(w: &mut World) -> &[(BlockPos, PoiKind)] {
    if w.mob.ai.pois.is_none() {
        let mut v = Vec::new();
        let (min, size) = (w.min, w.size);
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let p = BlockPos::new(min.x + x, min.y + y, min.z + z);
                    if let Some(k) = poi_of(&w.get(p)) {
                        v.push((p, k));
                    }
                }
            }
        }
        w.mob.ai.pois = Some(v);
    }
    w.mob.ai.pois.as_deref().unwrap()
}

/// `onBlockStateChange`'s POI update: a POI block placed or removed rebuilds the index (and a
/// light source the AI's light list).
pub fn on_block_changed(w: &mut World, old: &Block, new: &Block) {
    if poi_of(old).is_some() || poi_of(new).is_some() {
        w.mob.ai.pois = None;
    }
    if crate::entity::mob::sun::emission(old) != crate::entity::mob::sun::emission(new) {
        w.mob.ai.lights = None;
    }
}

/// The POIs a villager holds (taken tickets): its home, and with the exact brain its job site,
/// potential job site and meeting point.
pub fn claimed(w: &World) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        if let EntityKind::Mob(m) = &e.kind
            && let Some(b) = m.brain.as_deref()
        {
            let mut held: Vec<BlockPos> = b.home.into_iter().collect();
            if let Some(x) = b.ex.as_deref() {
                held.extend(x.mem.job_site);
                held.extend(x.mem.potential_job_site);
                held.extend(x.mem.meeting_point);
            }
            for h in held {
                if !out.contains(&h) {
                    out.push(h);
                }
            }
        }
    }
    out
}

/// `PoiManager.getInRange(any type, center, radius, IS_OCCUPIED)`'s positions (the order among
/// several is the POI manager's: callers that pick one refuse when there are several).
pub fn occupied_in_range(w: &mut World, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    let taken = claimed(w);
    let r2 = (radius as i64) * (radius as i64);
    index(w)
        .iter()
        .filter(|(p, _)| taken.contains(p))
        .filter(|(p, _)| {
            let (dx, dy, dz) = ((p.x - center.x) as i64, (p.y - center.y) as i64, (p.z - center.z) as i64);
            dx * dx + dy * dy + dz * dz <= r2
        })
        .map(|(p, _)| *p)
        .collect()
}

/// The claimed village POIs (the village centres' POIs).
pub fn claimed_village_pois(w: &World) -> Vec<BlockPos> {
    claimed(w).into_iter().filter(|p| poi_of(&w.get(*p)).is_some()).collect()
}

/// `SectionPos.of(pos)`.
fn section(p: BlockPos) -> (i32, i32, i32) {
    (p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16))
}

/// `PoiManager.sectionsToVillage(section)`.
pub fn sections_to_village_of(w: &World, s: (i32, i32, i32)) -> i32 {
    let mut best = 7;
    for p in claimed_village_pois(w) {
        let c = section(p);
        let d = (c.0 - s.0).abs().max((c.1 - s.1).abs()).max((c.2 - s.2).abs());
        best = best.min(d);
    }
    best
}

/// `PoiManager.sectionsToVillage(SectionPos.of(pos))`.
pub fn sections_to_village(w: &World, pos: BlockPos) -> i32 {
    sections_to_village_of(w, section(pos))
}

/// `ServerLevel.isCloseToVillage(pos, radius)`.
pub fn is_close_to_village(w: &World, pos: BlockPos, radius: i32) -> bool {
    radius <= 6 && sections_to_village(w, pos) <= radius
}

/// `ServerLevel.isVillage(pos)` (= close within 1 section).
pub fn is_village(w: &World, pos: BlockPos) -> bool {
    is_close_to_village(w, pos, 1)
}

/// `BehaviorUtils.findSectionClosestToVillage(level, center, radius)`: among the sections of the
/// cube (`SectionPos.cube`: x, then y, then z... as `betweenClosedStream` walks), one closer to a
/// village with the smallest distance (the first such), else the centre.
pub fn section_closest_to_village(w: &World, center: (i32, i32, i32), radius: i32) -> (i32, i32, i32) {
    let d0 = sections_to_village_of(w, center);
    let mut best: Option<((i32, i32, i32), i32)> = None;
    // `Cursor3D`: x fastest, then y, then z
    for z in center.2 - radius..=center.2 + radius {
        for y in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                let s = (x, y, z);
                let d = sections_to_village_of(w, s);
                if d < d0 && best.is_none_or(|(_, b)| d < b) {
                    best = Some((s, d));
                }
            }
        }
    }
    best.map(|(s, _)| s).unwrap_or(center)
}

/// The POIs of a selection with a free ticket within `radius` (a square of chunks, then the
/// distance): what `AcquirePoi`'s search can find.
pub fn free_pois(w: &mut World, sel: PoiSel, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    free_pois_ordered(w, sel, center, radius)
}

/// `PoiManager.getInRange(type, center, radius, HAS_SPACE)` in the manager's order: the chunks of
/// `ChunkPos.rangeClosed(center's chunk, floorDiv(radius, 16) + 1)` (x fastest, then z), each
/// chunk's sections bottom up, in a section the types in registry order (job sites, then homes,
/// then meeting points; the bridge iterates `byType` so) and a type's records in their HashSet's
/// order (the position's hash), then those within `radius` (`distSqr <= radius^2`).
pub fn free_pois_ordered(w: &mut World, sel: PoiSel, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    let taken = claimed(w);
    let want = |k: PoiKind| match sel {
        PoiSel::Home => k == PoiKind::Home,
        PoiSel::Meeting => k == PoiKind::Meeting,
        PoiSel::AcquirableJob => k == PoiKind::Job,
        PoiSel::HeldJob => false,
    };
    let type_rank = |k: PoiKind| match k {
        PoiKind::Job => 0,
        PoiKind::Home => 1,
        PoiKind::Meeting => 2,
    };
    let all: Vec<(BlockPos, PoiKind)> = index(w).to_vec();
    let cr = radius.div_euclid(16) + 1;
    let (ccx, ccz) = (center.x.div_euclid(16), center.z.div_euclid(16));
    // the records of each (section, type) set, for its HashSet's table size and order
    let mut groups: std::collections::BTreeMap<(i32, i32, i32, i32), Vec<BlockPos>> = std::collections::BTreeMap::new();
    for (p, k) in &all {
        let (cx, cz) = (p.x.div_euclid(16), p.z.div_euclid(16));
        if (cx - ccx).abs() > cr || (cz - ccz).abs() > cr {
            continue;
        }
        // key: chunk z, chunk x, section y, type (rangeClosed walks x fastest, then z)
        groups.entry((cz, cx, p.y.div_euclid(16), type_rank(*k))).or_default().push(*p);
    }
    let r2 = (radius as i64) * (radius as i64);
    let mut out = Vec::new();
    for ((_, _, _, rank), ps) in groups {
        let hashes: Vec<i32> = ps.iter().map(|p| (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x)).collect();
        let order = super::path::finder::java_hash_order(&hashes, super::path::finder::grown_table(ps.len()));
        for k in order {
            let p = ps[k];
            let kind = all.iter().find(|(q, _)| *q == p).map(|(_, k)| *k).unwrap();
            debug_assert_eq!(type_rank(kind), rank);
            if !want(kind) || taken.contains(&p) {
                continue;
            }
            let (dx, dy, dz) = ((p.x - center.x) as i64, (p.y - center.y) as i64, (p.z - center.z) as i64);
            if dx * dx + dy * dy + dz * dz <= r2 {
                out.push(p);
            }
        }
    }
    out
}

/// `PoiTypes`' `validRange` of a selection's types (home and job sites 1, the meeting point 6).
pub fn valid_range(sel: PoiSel) -> i32 {
    match sel {
        PoiSel::Meeting => 6,
        _ => 1,
    }
}

#[allow(dead_code)]
fn free_pois_unordered(w: &mut World, sel: PoiSel, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    let taken = claimed(w);
    let want = |k: PoiKind| match sel {
        PoiSel::Home => k == PoiKind::Home,
        PoiSel::Meeting => k == PoiKind::Meeting,
        PoiSel::AcquirableJob => k == PoiKind::Job,
        PoiSel::HeldJob => false,
    };
    let r2 = (radius as i64) * (radius as i64);
    index(w)
        .iter()
        .filter(|(p, k)| want(*k) && !taken.contains(p))
        .filter(|(p, _)| {
            let (dx, dy, dz) = ((p.x - center.x) as i64, (p.y - center.y) as i64, (p.z - center.z) as i64);
            dx.abs() <= radius as i64 && dz.abs() <= radius as i64 && dx * dx + dy * dy + dz * dz <= r2
        })
        .map(|(p, _)| *p)
        .collect()
}
