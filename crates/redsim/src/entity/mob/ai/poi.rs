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
        // a natural state's kind from its row, once per id (the index scans every cell)
        Block::Terrain(id) => {
            static TABLE: std::sync::OnceLock<Vec<Option<PoiKind>>> = std::sync::OnceLock::new();
            let t = TABLE.get_or_init(|| (0..crate::terrain::blocks::table().props.len()).map(|k| terrain_poi(k as u16)).collect());
            t.get(*id as usize).copied().flatten()
        }
        _ => None,
    }
}

fn terrain_poi(id: u16) -> Option<PoiKind> {
    let p = crate::terrain::blocks::props(id);
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

/// The POI type's registry id of a block (`PoiTypes`: a bed's head is home, a bell the meeting
/// point, each workstation its profession's), if it is one.
pub fn poi_type_name(b: &Block) -> Option<&'static str> {
    let job = |name: &str| -> Option<&'static str> {
        Some(match name {
            "blast_furnace" => "minecraft:armorer",
            "smoker" => "minecraft:butcher",
            "cartography_table" => "minecraft:cartographer",
            "brewing_stand" => "minecraft:cleric",
            "composter" => "minecraft:farmer",
            "barrel" => "minecraft:fisherman",
            "fletching_table" => "minecraft:fletcher",
            "cauldron" | "water_cauldron" | "lava_cauldron" | "powder_snow_cauldron" => "minecraft:leatherworker",
            "lectern" => "minecraft:librarian",
            "stonecutter" => "minecraft:mason",
            "loom" => "minecraft:shepherd",
            "smithing_table" => "minecraft:toolsmith",
            "grindstone" => "minecraft:weaponsmith",
            "bell" => "minecraft:meeting",
            _ => return None,
        })
    };
    match b {
        Block::Bed { head: true, .. } => Some("minecraft:home"),
        Block::Composter { .. } => Some("minecraft:farmer"),
        Block::BrewingStand { .. } => Some("minecraft:cleric"),
        Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(*id);
            let name = p.block_id();
            if name.ends_with("_bed") {
                return p.name.contains("part=head").then_some("minecraft:home");
            }
            job(name.trim_start_matches("minecraft:"))
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

/// `PoiManager`'s tickets: a record's taken tickets live with the record (its own state in the POI data),
/// taken and given back only by `take` / `release` (AcquirePoi, a bench's explicit take): a memory never
/// takes one. A HOME (or job site, meeting point) memory set from NBT, set directly or kept through a world
/// load takes no ticket, as in the game (ironspec 06:31 (2) / 06:42 real: a summoned villager's HOME leaves
/// its bed's ticket free). A POI block broken and replaced is a fresh record with none taken while its old
/// claimant keeps its memory (corrupt-poi-entity R1).
fn ensure_tickets(w: &mut World) -> &mut std::collections::BTreeMap<(i32, i32, i32), u32> {
    w.mob.ai.poi_tickets.get_or_insert_with(std::collections::BTreeMap::new)
}

/// `PoiManager.take`: a ticket of the record at `p` taken.
pub fn take(w: &mut World, p: BlockPos) {
    *ensure_tickets(w).entry((p.x, p.y, p.z)).or_insert(0) += 1;
    note_village_centres(w);
}

/// (tickexact 2026-11-11) `PoiRecord.getFreeTickets` of the record at `p` (None: no POI there):
/// its kind's tickets (a home or a job site 1, a meeting point 32) less the taken ones.
pub fn free_tickets(w: &mut World, p: BlockPos) -> Option<u32> {
    let kind = poi_of(&w.get(p))?;
    let max = if kind == PoiKind::Meeting { 32 } else { 1 };
    let held = ensure_tickets(w).get(&(p.x, p.y, p.z)).copied().unwrap_or(0);
    Some(max - held.min(max))
}

/// (tickexact 2026-11-10, ironfarm's stranded napper) `PoiManager.take(acquirable, (type, pos) ->
/// pos.equals(p), p, 1)` as AcquirePoi calls it: the record at `p`, when its kind is the
/// selection's and it has a free ticket (homes and job sites hold 1, a meeting point 32), takes
/// one; else nothing (the caller sets its memory anyway, as the game does).
pub fn take_if_free(w: &mut World, sel: PoiSel, p: BlockPos) -> bool {
    let Some(kind) = poi_of(&w.get(p)) else { return false };
    let matches = match sel {
        PoiSel::Home => kind == PoiKind::Home,
        PoiSel::Meeting => kind == PoiKind::Meeting,
        PoiSel::AcquirableJob => kind == PoiKind::Job,
        PoiSel::HeldJob => false,
    };
    let max = if kind == PoiKind::Meeting { 32 } else { 1 };
    let held = ensure_tickets(w).get(&(p.x, p.y, p.z)).copied().unwrap_or(0);
    if !matches || held >= max {
        return false;
    }
    take(w, p);
    true
}

/// `PoiManager.release`: a ticket of the record at `p` given back (a record that is gone has none).
pub fn release(w: &mut World, p: BlockPos) {
    if let Some(n) = ensure_tickets(w).get_mut(&(p.x, p.y, p.z))
        && *n > 0
    {
        *n -= 1;
    }
}

/// `onBlockStateChange`'s POI update: a POI block placed or removed updates the index in place
/// (it stays in the build's y, z, x order; a bed's `occupied` changing is no change of kind), a
/// light source the AI's light list.
pub fn on_block_changed(w: &mut World, pos: BlockPos, old: &Block, new: &Block) {
    let (was, now) = (poi_of(old), poi_of(new));
    // PoiManager.add's order, kept for the record sets' collision chains; a removed record takes
    // its tickets with it (a re-added one starts with none taken)
    if was != now {
        if was.is_some() {
            ensure_tickets(w).remove(&(pos.x, pos.y, pos.z));
        }
        if now.is_some() {
            let n = w.mob.ai.poi_next;
            w.mob.ai.poi_seq.insert(pos, n);
            w.mob.ai.poi_next += 1;
        } else {
            w.mob.ai.poi_seq.remove(&pos);
        }
    }
    if was != now
        && let Some(v) = w.mob.ai.pois.as_mut()
    {
        let key = |p: &BlockPos| (p.y, p.z, p.x);
        if was.is_some()
            && let Ok(k) = v.binary_search_by(|(p, _)| key(p).cmp(&key(&pos)))
        {
            v.remove(k);
        }
        if let Some(kind) = now {
            let k = v.binary_search_by(|(p, _)| key(p).cmp(&key(&pos))).unwrap_or_else(|k| k);
            v.insert(k, (pos, kind));
        }
    }
    if crate::entity::mob::sun::emission(old) != crate::entity::mob::sun::emission(new) {
        w.mob.ai.lights = None;
    }
    if !w.mob.ai.sky_cache.is_empty() && old != new {
        w.mob.ai.sky_cache.clear();
    }
    if !w.mob.ai.block_cache.is_empty() && old != new {
        w.mob.ai.block_cache.clear();
    }
}

/// The POIs whose records have a ticket taken (`poi_tickets`; none before the first take): a memory alone
/// claims nothing (see [`ensure_tickets`]).
pub fn claimed(w: &World) -> Vec<BlockPos> {
    match w.mob.ai.poi_tickets.as_ref() {
        Some(t) => t.iter().filter(|(_, n)| **n > 0).map(|(k, _)| BlockPos::new(k.0, k.1, k.2)).collect(),
        None => Vec::new(),
    }
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
pub fn section(p: BlockPos) -> (i32, i32, i32) {
    (p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16))
}

/// `PoiManager.sectionsToVillage(section)`.
pub fn sections_to_village_of(w: &World, s: (i32, i32, i32)) -> i32 {
    sections_to_village_in(&village_sections(w), s)
}

/// What `sectionsToVillage` measures from: the sections of the claimed village POIs, and the
/// sections the distance tracker ever stored (`AiWorld::village_seen`). Callers asking about many
/// sections take them once.
#[derive(Clone, Debug)]
pub struct Villages {
    pub centres: Vec<(i32, i32, i32)>,
    pub seen: std::sync::Arc<std::collections::HashSet<(i32, i32, i32)>>,
}

/// The village centres now, and the tracker's memory.
pub fn village_sections(w: &World) -> Villages {
    Villages { centres: claimed_village_pois(w).into_iter().map(section).collect(), seen: w.mob.ai.village_seen.clone() }
}

/// (mob-AI 2026-11-11) A run of `PoiManager.DistanceTracker`: every section within 5 of a village
/// centre is stored from then on (`AiWorld::village_seen`). The game's tracker runs in
/// `PoiManager.tick` (each tick, after the chunks' ticks) and in every `sectionsToVillage`; this
/// is called at the first of those and when a ticket is taken (a claim lasts longer than a tick).
pub fn note_village_centres(w: &mut World) {
    let centres: Vec<(i32, i32, i32)> = claimed_village_pois(w).into_iter().map(section).collect();
    for c in centres {
        if !w.mob.ai.village_noted.insert(c) {
            continue;
        }
        let seen = std::sync::Arc::make_mut(&mut w.mob.ai.village_seen);
        for dx in -5..=5 {
            for dy in -5..=5 {
                for dz in -5..=5 {
                    seen.insert((c.0 + dx, c.1 + dy, c.2 + dz));
                }
            }
        }
    }
}

/// `sectionsToVillage` against precomputed [`village_sections`]: the Chebyshev distance in sections
/// up to 5, else 6 for a section the tracker ever stored, else 7. PoiManager's DistanceTracker is a
/// SectionTracker of 7 levels (clamped to 0..6) whose decreases spread from a section only below
/// level 5 (`checkNeighborsAfterUpdate`: `level < levelCount - 2`), so a section 6 away is never
/// reached and keeps the map's default 7. When a centre goes, the increase path stores
/// `levelCount - 1` = 6 in every section it had reached (`setLevel` keeps levels up to 6), and
/// nothing ever removes them (tickexact's carousel siding t10117 and station t2471: a zombie's
/// MoveThroughVillageGoal still sees the village after its last bed is released).
pub fn sections_to_village_in(v: &Villages, s: (i32, i32, i32)) -> i32 {
    let mut best = 7;
    for c in &v.centres {
        let d = (c.0 - s.0).abs().max((c.1 - s.1).abs()).max((c.2 - s.2).abs());
        if d <= 5 {
            best = best.min(d);
        }
    }
    if best == 7 && v.seen.contains(&s) { 6 } else { best }
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
    let v = village_sections(w);
    let d0 = sections_to_village_in(&v, center);
    let mut best: Option<((i32, i32, i32), i32)> = None;
    // `Cursor3D`: x fastest, then y, then z
    for z in center.2 - radius..=center.2 + radius {
        for y in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                let s = (x, y, z);
                let d = sections_to_village_in(&v, s);
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
/// `PoiManager.findAllWithType(HOME, .., center, radius, ANY)`'s positions in the manager's order.
pub fn homes_any_ordered(w: &mut World, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    pois_ordered(w, center, radius, |k, _| k == PoiKind::Home)
}

pub fn free_pois_ordered(w: &mut World, sel: PoiSel, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    let want = |k: PoiKind| match sel {
        PoiSel::Home => k == PoiKind::Home,
        PoiSel::Meeting => k == PoiKind::Meeting,
        PoiSel::AcquirableJob => k == PoiKind::Job,
        PoiSel::HeldJob => false,
    };
    pois_ordered(w, center, radius, |k, taken| want(k) && !taken)
}

/// `PoiManager.getInRange(any type, center, radius, IS_OCCUPIED)` in the manager's order (see
/// [`free_pois_ordered`]): the claimed POIs within `radius`.
pub fn occupied_in_range_ordered(w: &mut World, center: BlockPos, radius: i32) -> Vec<BlockPos> {
    pois_ordered(w, center, radius, |_, taken| taken)
}

/// `PoiManager.getInRange(predicate, center, radius, occupancy)`' first position of a type (any
/// occupancy): `find(MEETING, any, center, radius, ANY)`, the wandering trader spawner's.
pub fn first_meeting_within(w: &mut World, center: BlockPos, radius: i32) -> Option<BlockPos> {
    pois_ordered(w, center, radius, |k, _| k == PoiKind::Meeting).into_iter().next()
}

/// The POIs `keep(kind, full)` keeps within `radius` of `center`, in the POI manager's order (`full`:
/// no free ticket left: homes and job sites hold 1, a meeting point 32 (tickexact 2026-11-11: a bell
/// one villager took is still free to the next)).
fn pois_ordered(w: &mut World, center: BlockPos, radius: i32, keep: impl Fn(PoiKind, bool) -> bool) -> Vec<BlockPos> {
    let held: Vec<(BlockPos, u32)> = match w.mob.ai.poi_tickets.as_ref() {
        Some(t) => t.iter().map(|(k, n)| (BlockPos::new(k.0, k.1, k.2), *n)).collect(),
        None => Vec::new(),
    };
    let full = |p: BlockPos, kind: PoiKind| held.iter().find(|(q, _)| *q == p).map_or(0, |(_, n)| *n) >= if kind == PoiKind::Meeting { 32 } else { 1 };
    let all: Vec<(BlockPos, PoiKind)> = index(w).to_vec();
    let cr = radius.div_euclid(16) + 1;
    let (ccx, ccz) = (center.x.div_euclid(16), center.z.div_euclid(16));
    // the records of each section by POI type (`PoiSection.byType`: a HashMap of the type holders,
    // each a HashSet of records), for the map's and each set's table order
    let mut sections: std::collections::BTreeMap<(i32, i32, i32), Vec<(&'static str, Vec<BlockPos>)>> = std::collections::BTreeMap::new();
    for (p, _) in &all {
        let (cx, cz) = (p.x.div_euclid(16), p.z.div_euclid(16));
        if (cx - ccx).abs() > cr || (cz - ccz).abs() > cr {
            continue;
        }
        let ty = poi_type_name(&w.get(*p)).unwrap_or("minecraft:home");
        // key: chunk z, chunk x, section y (rangeClosed walks x fastest, then z; sections upward)
        let types = sections.entry((cz, cx, p.y.div_euclid(16))).or_default();
        match types.iter_mut().find(|(t, _)| *t == ty) {
            Some((_, ps)) => ps.push(*p),
            None => types.push((ty, vec![*p])),
        }
    }
    let r2 = (radius as i64) * (radius as i64);
    let mut out = Vec::new();
    for (_, types) in sections {
        // byType's iteration: its holders by their (identity) hashes in the map's table
        let th: Vec<i32> = types.iter().map(|(t, _)| super::poi_holder_hash(w, t)).collect();
        let torder = super::path::finder::java_hash_order(&th, super::path::finder::grown_table(types.len()));
        for ti in torder {
            // the set's colliding records in PoiManager.add's order: the world's first scan (no
            // number, in its order) before the POIs placed since, in placement order
            let mut ps = types[ti].1.clone();
            ps.sort_by_key(|p| w.mob.ai.poi_seq.get(p).map_or(0, |n| n + 1));
            let ps = &ps;
            let hashes: Vec<i32> = ps.iter().map(|p| (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x)).collect();
            let order = super::path::finder::java_hash_order(&hashes, super::path::finder::grown_table(ps.len()));
            for k in order {
                let p = ps[k];
                let kind = all.iter().find(|(q, _)| *q == p).map(|(_, k)| *k).unwrap();
                if !keep(kind, full(p, kind)) {
                    continue;
                }
                let (dx, dy, dz) = ((p.x - center.x) as i64, (p.y - center.y) as i64, (p.z - center.z) as i64);
                if dx * dx + dy * dy + dz * dz <= r2 {
                    out.push(p);
                }
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

#[cfg(test)]
mod ticket_tests {
    use super::*;

    /// A world with a bed (its head at (0,0,2)) on a stone floor, the mob AI on, at day time `day`.
    fn bed_world(day: i64) -> (World, BlockPos) {
        let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
        let stone = Block::parse("minecraft:stone").unwrap();
        for x in -6..=6 {
            for z in -6..=6 {
                w.set_block(BlockPos::new(x, -1, z), stone.clone(), crate::world::flags::ALL);
            }
        }
        w.set_block(BlockPos::new(0, 0, 1), Block::parse("minecraft:red_bed[facing=south,occupied=false,part=foot]").unwrap(), crate::world::flags::ALL);
        w.set_block(BlockPos::new(0, 0, 2), Block::parse("minecraft:red_bed[facing=south,occupied=false,part=head]").unwrap(), crate::world::flags::ALL);
        w.mob.seed(7, day);
        w.mob.ai.on = true;
        (w, BlockPos::new(0, 0, 2))
    }

    const HOME: &str = "{Brain:{memories:{\"minecraft:home\":{value:{dimension:\"minecraft:overworld\",pos:[I;0,0,2]}}}}}";

    /// ironspec 06:31 (2) / 06:42 real: a HOME memory takes no ticket on any path that sets it. (1) A villager
    /// summoned with HOME NBT (`entity::spawn`, 58f770cc's and b8949c96's path): its bed's free tickets stay 1
    /// whether the tickets were read before the summon or not (the old lazy count from the memories read 0 when
    /// they weren't). (2) A HOME set directly on the brain: 1. (3) A world load (every chunk saved, unloaded and
    /// loaded): the reloaded villager's HOME takes none, 1; a ticket AcquirePoi took (`take_if_free`) is the
    /// record's and stays taken through the load, 0.
    #[test]
    fn a_home_memory_takes_no_ticket() {
        // (1) the summon, the tickets untouched before it (b8949c96's bench) and read before it (58f770cc's)
        for read_first in [false, true] {
            let (mut w, bed) = bed_world(13_800);
            if read_first {
                assert_eq!(free_tickets(&mut w, bed), Some(1));
            }
            let v = crate::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, -2.5], Some(HOME)).unwrap();
            let i = crate::entity::index_of(&w, v).unwrap();
            assert_eq!(crate::entity::mob::ai::brain::brain(&w, i).home, Some(bed), "the HOME loaded");
            assert_eq!(free_tickets(&mut w, bed), Some(1), "summoned with HOME NBT (tickets read first: {read_first})");
            w.tick();
            assert_eq!(free_tickets(&mut w, bed), Some(1), "and after its first tick");
        }
        // (2) the memory set directly
        let (mut w, bed) = bed_world(13_800);
        let v = crate::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, -2.5], None).unwrap();
        let i = crate::entity::index_of(&w, v).unwrap();
        crate::entity::mob::ai::brain::brain_mut(&mut w, i).home = Some(bed);
        assert_eq!(free_tickets(&mut w, bed), Some(1), "a HOME set directly");
        // (3) a world load: the summoned villager's HOME through it, and a taken ticket kept
        let (mut w, bed) = bed_world(13_800);
        crate::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, -2.5], Some(HOME)).unwrap();
        crate::reload::unload_all(&mut w);
        crate::reload::load_all(&mut w);
        assert_eq!(free_tickets(&mut w, bed), Some(1), "a HOME through a world load");
        assert!(take_if_free(&mut w, PoiSel::Home, bed), "AcquirePoi's take");
        assert_eq!(free_tickets(&mut w, bed), Some(0));
        crate::reload::unload_all(&mut w);
        crate::reload::load_all(&mut w);
        assert_eq!(free_tickets(&mut w, bed), Some(0), "the record's taken ticket kept through the load");
    }

    /// G1, the order dependence itself (the old lazy count): a freshly placed bed starts at free 1 (the
    /// game's new POI record), and a villager whose HOME is that bed, set by a summon's NBT or directly, leaves
    /// it at 1 whether the tickets were (a) touched before the HOME was set or (b) first read after it; (c) an
    /// explicit `take` makes it 0 in both orders.
    #[test]
    fn ticket_reads_are_order_independent() {
        let (mut fresh, bed) = bed_world(13_800);
        assert_eq!(free_tickets(&mut fresh, bed), Some(1), "a freshly placed bed: free 1");
        for summon in [true, false] {
            for touched_first in [true, false] {
                let (mut w, bed) = bed_world(13_800);
                if touched_first {
                    let _ = claimed(&w);
                    release(&mut w, bed);
                }
                let v = crate::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, -2.5], if summon { Some(HOME) } else { None }).unwrap();
                if !summon {
                    let i = crate::entity::index_of(&w, v).unwrap();
                    crate::entity::mob::ai::brain::brain_mut(&mut w, i).home = Some(bed);
                }
                assert_eq!(free_tickets(&mut w, bed), Some(1), "HOME by {} (tickets touched first: {touched_first})", if summon { "NBT" } else { "a direct set" });
                assert!(!claimed(&w).contains(&bed), "claimed lists no memory");
                take(&mut w, bed);
                assert_eq!(free_tickets(&mut w, bed), Some(0), "an explicit take (tickets touched first: {touched_first})");
            }
        }
    }

    /// ironspec 07:57 (5), G1 07:59: AcquirePoi claims a bed only when its path reaches it and the record has a
    /// free ticket, so of six HOME-less villagers within 48 of one free bed EXACTLY ONE claims it (HOME set, the
    /// ticket taken: free 0) while the full bed drops out of the others' search; with the bed sealed off (no
    /// path reaches it) NONE claims and the ticket stays free. The claimant and its tick printed.
    #[test]
    fn acquire_poi_claims_one_bed_once() {
        for sealed in [false, true] {
            let (mut w, bed) = bed_world(6_000);
            if sealed {
                let stone = Block::parse("minecraft:stone").unwrap();
                for (x, z) in [(-1, 0), (0, 0), (1, 0), (-1, 1), (1, 1), (-1, 2), (1, 2), (-1, 3), (0, 3), (1, 3)] {
                    for y in 0..=2 {
                        w.set_block(BlockPos::new(x, y, z), stone.clone(), crate::world::flags::ALL);
                    }
                }
                for z in 1..=2 {
                    w.set_block(BlockPos::new(0, 1, z), stone.clone(), crate::world::flags::ALL);
                }
            }
            let ids: Vec<u32> = (0..6).map(|k| crate::entity::spawn(&mut w, "minecraft:villager", [-3.5 + k as f64, 0.0, -3.5], None).unwrap()).collect();
            let mut claims: Vec<(u32, i64)> = Vec::new();
            for _ in 0..300 {
                w.tick();
                for id in &ids {
                    if let Some(i) = crate::entity::index_of(&w, *id)
                        && crate::entity::mob::ai::brain::brain(&w, i).home == Some(bed)
                        && !claims.iter().any(|(c, _)| c == id)
                    {
                        claims.push((*id, w.time));
                    }
                }
            }
            println!("sealed {sealed}: claims (villager id, tick) {claims:?}, the bed's free tickets {:?}", free_tickets(&mut w, bed));
            if sealed {
                assert!(claims.is_empty(), "no path reaches the sealed bed: no claim");
                assert_eq!(free_tickets(&mut w, bed), Some(1), "its ticket free");
            } else {
                assert_eq!(claims.len(), 1, "exactly one claim on one bed");
                assert_eq!(free_tickets(&mut w, bed), Some(0), "the claimant took the ticket");
            }
        }
    }
}
