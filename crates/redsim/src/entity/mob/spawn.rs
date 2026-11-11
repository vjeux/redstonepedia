//! Golem spawning (ironfarm), read in the 26.3 source: `Villager.spawnGolemIfNeeded(level,
//! timestamp, villagersNeededToAgree)`, `SpawnUtil.trySpawnMob(IRON_GOLEM, MOB_SUMMONED, level,
//! villager.blockPosition(), 10, 8, 6, Strategy.LEGACY_IRON_GOLEM, false)`
//! (`moveToPossibleSpawnPosition`), `EntityType.create` (yaw) + `Mob.finalizeSpawn` (the
//! follow-range triangle, left-handedness) and `IronGolem.checkSpawnObstruction`. The level
//! random's draws are the seeded stand-ins ([`super::MobWorld::rng`]): the same distribution.

use super::types::MobType;
use super::{MobEvent, mob};
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

/// `Villager.spawnGolemIfNeeded(level, timestamp, needed)` of villager `i`: it must want a golem;
/// the villagers whose box meets its box inflated by 10 (itself included); at most 5 of those that
/// want one; with `needed` or more, `trySpawnMob`, and on success every villager in the box gets
/// `GOLEM_DETECTED_RECENTLY`.
pub fn spawn_golem_if_needed(w: &mut World, i: usize, needed: usize) {
    let t = w.time;
    if !w.mob.golem_spawning {
        return;
    }
    let wants = |w: &World, j: usize| mob(w, j).brain.as_deref().is_some_and(|b| b.wants_golem(t));
    if !wants(w, i) {
        return;
    }
    let bb = w.entities.list[i].bb.inflate(10.0, 10.0, 10.0);
    let nearby: Vec<usize> = w.get_entities(bb, |e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == MobType::Villager));
    let willing = nearby.iter().filter(|&&j| wants(w, j)).take(5).count();
    if willing < needed {
        return;
    }
    let start = w.entities.list[i].block_position();
    let villager = w.entities.list[i].id;
    let golem = try_spawn_golem(w, start);
    let at = golem.and_then(|id| crate::entity::index_of(w, id)).map(|g| {
        let b = w.entities.list[g].block_position();
        [b.x, b.y, b.z]
    });
    w.mob.events.push(MobEvent::GolemTry { t, villager, willing: willing as u32, golem, at });
    if let Some(g) = golem {
        for j in nearby {
            if mob(w, j).brain.is_some() {
                super::villager::golem_detected(w, j);
                let id = w.entities.list[j].id;
                w.mob.events.push(MobEvent::GolemLocked { t, villager: id, summoner: villager, golem: g });
            }
        }
    }
}

/// `SpawnUtil.trySpawnMob(IRON_GOLEM, .., start, 10, 8, 6, LEGACY_IRON_GOLEM, false)`: the id of
/// the golem it added, if any. Each try records the cell its column scan stopped at.
///
/// (tickexact) In an exact world (`World::exact_seed`) every draw is the game's, from the level
/// random in its order: each try's `Mth.randomBetweenInclusive(random, -8, 8)` twice; where the
/// column scan finds a cell, `EntityType.create` (a construction: `World::construct`), its snap
/// yaw (`wrapDegrees(nextFloat() * 360)`, the head and body set to it) and `Mob.finalizeSpawn`'s
/// follow-range triangle and left-handedness; a golem that passes `checkSpawnObstruction` enters
/// with its own random seeded as its entity number (`World::entering_random`, the constructor's
/// UUID and yaw replayed: an iron golem has no sensors). Elsewhere the seeded stand-ins of the
/// mob layer's random, unchanged.
pub fn try_spawn_golem(w: &mut World, start: BlockPos) -> Option<u32> {
    if w.exact_seed.is_some() && w.level_random.is_some() {
        return try_spawn_golem_exact(w, start);
    }
    let mut tries = Vec::new();
    let mut spawned = None;
    for _ in 0..10 {
        let (dx, dz) = {
            let r = w.mob.rng_mut();
            // Mth.randomBetweenInclusive(random, -8, 8) = nextInt(17) - 8
            (r.next_int(17) - 8, r.next_int(17) - 8)
        };
        let top = BlockPos::new(start.x + dx, start.y + 6, start.z + dz);
        let Some(cell) = possible_spawn_position(w, top, 6) else {
            tries.push(None);
            continue;
        };
        tries.push(Some(cell));
        // EntityType.create: yaw from the level random; finalizeSpawn: the follow-range triangle
        // and left-handedness, drawn whether or not the spawn goes on
        let (y_rot, bonus, seed) = {
            let r = w.mob.rng_mut();
            let y_rot = crate::entity::mth::wrap_degrees(r.next_float() * 360.0);
            let bonus = r.triangle(0.0, 0.11485000000000001);
            r.next_float();
            (y_rot, bonus, r.next_long())
        };
        let pos = Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5);
        if check_spawn_obstruction(w, cell, pos) {
            let mut st = super::MobState::new(MobType::IronGolem, bonus);
            st.walk = w.mob.golem_walk;
            // ---- mob AI (the mob-AI lane): its goals ----
            if w.mob.ai.on {
                st.ai = Some(Box::new(super::ai::MobAi::new(MobType::IronGolem, y_rot)));
                st.speed = 0.0;
            }
            let id = w.entities.add(EntityKind::Mob(Box::new(st)), pos, y_rot);
            if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
                e.random = Some(crate::rng::LegacyRandom::new(seed));
            }
            spawned = Some(id);
            break;
        }
    }
    w.mob.spawn_tries.push((start, tries, spawned.and_then(|id| crate::entity::index_of(w, id)).map(|g| w.entities.list[g].block_position())));
    spawned
}

/// [`try_spawn_golem`] in an exact world: the game's draws, from the level random.
fn try_spawn_golem_exact(w: &mut World, start: BlockPos) -> Option<u32> {
    let mut tries = Vec::new();
    let mut spawned = None;
    for _ in 0..10 {
        let (dx, dz) = {
            let r = w.level_random.as_mut().expect("an exact world mirrors the level random");
            (r.next_int(17) - 8, r.next_int(17) - 8)
        };
        let top = BlockPos::new(start.x + dx, start.y + 6, start.z + dz);
        let Some(cell) = possible_spawn_position(w, top, 6) else {
            tries.push(None);
            continue;
        };
        tries.push(Some(cell));
        // EntityType.create(level, null, pos, MOB_SUMMONED, false, false): the construction, then
        // snapTo with the level random's yaw (head and body follow it), then finalizeSpawn
        let _ = w.construct();
        let (y_rot, bonus) = {
            let r = w.level_random.as_mut().expect("an exact world mirrors the level random");
            let y_rot = crate::entity::mth::wrap_degrees(r.next_float() * 360.0);
            let bonus = r.triangle(0.0, 0.11485000000000001);
            r.next_float();
            (y_rot, bonus)
        };
        let pos = Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5);
        if check_spawn_obstruction(w, cell, pos) {
            let mut st = super::MobState::new(MobType::IronGolem, bonus);
            st.walk = w.mob.golem_walk;
            // ---- mob AI (the mob-AI lane): its goals (head and body at the snap yaw) ----
            if w.mob.ai.on {
                st.ai = Some(Box::new(super::ai::MobAi::new(MobType::IronGolem, y_rot)));
                st.speed = 0.0;
            }
            // the golem enters (addFreshEntityWithPassengers): its own random seeded as its
            // number, the constructor's UUID and yaw replayed (the yaw thrown away: the snap set
            // the head and body); no sound draw (an iron golem has no ambient sound)
            let n = w.entities.next_id;
            let mut own = w.entering_random(n);
            crate::rng::replay_uuid(&mut own);
            own.next_float();
            let id = w.entities.add(EntityKind::Mob(Box::new(st)), pos, y_rot);
            if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
                e.random = Some(own);
            }
            spawned = Some(id);
            break;
        }
    }
    w.mob.spawn_tries.push((start, tries, spawned.and_then(|id| crate::entity::index_of(w, id)).map(|g| w.entities.list[g].block_position())));
    spawned
}

/// `SpawnUtil.moveToPossibleSpawnPosition(level, rangeY, searchPos, strategy)` from the cell `top`
/// (= start + (dx, rangeY, dz)): down from `top - 1` to `top - 1 - 2 * rangeY`, the first cell
/// whose block passes `LEGACY_IRON_GOLEM` with the block above it; the spawn cell is above it.
pub fn possible_spawn_position(w: &World, top: BlockPos, range_y: i32) -> Option<BlockPos> {
    // (the column read by index when it lies in the box: tickexact 2026-11-09, the spot search's
    // column walks were 4% of a day-1 farm page's first frame)
    if let (Some(it), Some(_)) = (w.index(top), w.index(top.offset(0, -(2 * range_y + 1), 0))) {
        let stride = (w.size[0] * w.size[2]) as usize;
        let mut above = w.cells[it];
        let mut i = it;
        for k in 0..(2 * range_y + 1) {
            i -= stride;
            let here = w.cells[i];
            if super::blocks::legacy_golem_can_spawn_on(&here, &above) {
                return Some(top.offset(0, -k, 0));
            }
            above = here;
        }
        return None;
    }
    let mut above = w.get(top);
    let mut p = top;
    for _ in 0..(2 * range_y + 1) {
        p = p.below();
        let here = w.get(p);
        if super::blocks::legacy_golem_can_spawn_on(&here, &above) {
            return Some(p.above());
        }
        above = here;
    }
    None
}

/// `IronGolem.checkSpawnObstruction(level)` of a golem at `pos` (its cell `cell`): the block below
/// can be stood on, the two cells above are valid empty spawn blocks (no fluid), the feet cell too
/// (its fluid ignored), and no entity that blocks building meets its box.
pub fn check_spawn_obstruction(w: &World, cell: BlockPos, pos: Vec3) -> bool {
    obstruction(w, cell, pos).is_ok()
}

/// [`check_spawn_obstruction`] with the first rule that fails.
pub fn obstruction(w: &World, cell: BlockPos, pos: Vec3) -> Result<(), &'static str> {
    if !super::blocks::entity_can_stand_on(w, cell.below()) {
        return Err("the block below cannot be stood on");
    }
    for k in 1..3 {
        if !super::blocks::is_valid_empty_spawn_block(w, cell.offset(0, k, 0), true, false) {
            return Err("the 2 cells above the feet are not empty (a block, a fluid, a rail or a dangerous block)");
        }
    }
    if !super::blocks::is_valid_empty_spawn_block(w, cell, false, false) {
        return Err("the feet cell is not empty (a block, a rail or a dangerous block)");
    }
    let d = MobType::IronGolem.def();
    let bb = crate::entity::make_bounding_box(pos, d.width, d.height);
    if !is_unobstructed(w, bb) {
        return Err("an entity is in the golem's box");
    }
    Ok(())
}

/// Every column the golem search of villager `i` can pick (`trySpawnMob(IRON_GOLEM, .., 10, 8, 6,
/// LEGACY_IRON_GOLEM)`: the 17 x 17 columns around its cell, each scanned from y + 5 down to
/// y - 7) with the cell the scan stops at and whether a golem spawns there
/// (`checkSpawnObstruction`); `Err` names the first rule that fails, and a column with no cell
/// gives its lowest scanned cell. For the viewer (a live world: recompute when the farm changes).
pub fn golem_spawn_cells(w: &World, i: usize) -> Vec<(BlockPos, Result<(), &'static str>)> {
    golem_spawn_cells_from(w, w.entities.list[i].block_position())
}

/// [`golem_spawn_cells`] of a search started at the villager cell `start`.
pub fn golem_spawn_cells_from(w: &World, start: BlockPos) -> Vec<(BlockPos, Result<(), &'static str>)> {
    let mut out = Vec::with_capacity(289);
    for dz in -8..=8 {
        for dx in -8..=8 {
            let top = BlockPos::new(start.x + dx, start.y + 6, start.z + dz);
            match possible_spawn_position(w, top, 6) {
                None => out.push((top.offset(0, -13, 0), Err("no cell in the column: no solid block with air or a fluid above it from y+5 to y-7 (LEGACY_IRON_GOLEM)"))),
                Some(cell) => out.push((cell, obstruction(w, cell, Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5)))),
            }
        }
    }
    out
}

/// `EntityGetter.isUnobstructed(null, Shapes.create(box))`: no unremoved entity with
/// `blocksBuilding` (living entities, dying ones included until removed; boats, minecarts, primed
/// TNT, falling blocks, armor stands that are not markers) meets the box.
pub fn is_unobstructed(w: &World, bb: crate::entity::Aabb) -> bool {
    let blocking = w.get_entities(bb, |e| matches!(e.kind, EntityKind::Mob(_) | EntityKind::Boat(_) | EntityKind::Minecart(_) | EntityKind::ArmorStand(_) | EntityKind::PrimedTnt(_)));
    blocking.is_empty() && !w.gravity.entities.iter().any(|f| !f.removed && f.bb().intersects(bb))
}
