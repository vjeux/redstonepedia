//! A chunk saved, unloaded and loaded again (tickexact; `clock verify --reload`, `trace-export
//! --reload-at`): what the game keeps, numbers again and re-registers when the sandbox's chunks go
//! and come back between two ticks with no game time passing.
//!
//! * Scheduled block and fluid ticks: each chunk's ticks are saved in sub-tick order with their
//!   delays and come back numbered -N..-1 (`LevelChunkTicks.pack` / `unpack`), so ticks of two
//!   chunks can tie and run in `LevelTicks`' container order ([`crate::ticks::LevelTicks::reload`]).
//! * Block entities: saved whole except what their `saveAdditional` leaves out (a hopper's
//!   `tickedGameTime`, a moving block's `progressO` and `lastTicked`), and registered again chunk
//!   by chunk in the chunk's block-entity map order (`registerAllBlockEntitiesAfterLevelLoad` over
//!   an `Object2ObjectOpenHashMap<BlockPos, BlockEntity>`, filled in its saved order), after any
//!   ticker of a chunk that stayed: the hoppers', the sensors' and the moving blocks' ticker orders.
//! * The chunks come back in the order the bridge forces them again (x, then z).
//! * Not modeled: entities across a reload (new objects, new ids and randoms) and a reload that
//!   interleaves block-entity kinds in an order redsim's phases cannot (hoppers, then sensors,
//!   then moving blocks): both are model errors.

use crate::pos::BlockPos;
use crate::world::World;

/// A model error (once).
fn error(w: &mut World, why: String) {
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

/// fastutil's `HashCommon.mix(int)`.
fn mix_int(x: i32) -> i32 {
    let h = x.wrapping_mul(0x9E37_79B9_u32 as i32);
    h ^ ((h as u32) >> 16) as i32
}

/// `Vec3i.hashCode()` (a `BlockPos` map key).
fn pos_hash(p: BlockPos) -> i32 {
    p.y.wrapping_add(p.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(p.x)
}

/// The walk order of a fresh `Object2ObjectOpenHashMap` (16 expected, load factor 0.75) after
/// `keys` were put in that order: the slots from the top down (its `values().forEach`), the
/// table grown as fastutil grows it.
pub fn object_map_order(keys: &[BlockPos]) -> Vec<BlockPos> {
    let mut n = 32usize;
    let mut table: Vec<Option<BlockPos>> = vec![None; n];
    let mut size = 0usize;
    let max_fill = |n: usize| (((n as f64) * 0.75).ceil() as usize).min(n - 1);
    let place = |table: &mut Vec<Option<BlockPos>>, n: usize, k: BlockPos| {
        let mask = n - 1;
        let mut pos = (mix_int(pos_hash(k)) as u32 as usize) & mask;
        while let Some(q) = table[pos] {
            if q == k {
                return false;
            }
            pos = (pos + 1) & mask;
        }
        table[pos] = Some(k);
        true
    };
    for &k in keys {
        if !place(&mut table, n, k) {
            continue;
        }
        let was = size;
        size += 1;
        if was >= max_fill(n) {
            // rehash(arraySize(size + 1)): the old table walked from the top down into the new one
            let new_n = (((size + 1) as f64 / 0.75).ceil() as usize).max(2).next_power_of_two();
            let mut new_table: Vec<Option<BlockPos>> = vec![None; new_n];
            for q in table.iter().rev().flatten() {
                place(&mut new_table, new_n, *q);
            }
            table = new_table;
            n = new_n;
        }
    }
    table.iter().rev().flatten().copied().collect()
}

/// The order a reloaded chunk registers its block entities in: its block-entity map filled in the
/// saved order, which is the walk order of the map it had (approximated as filled in cell order:
/// only a collision in the table makes the fill order matter).
fn registration_order(mut bes: Vec<BlockPos>) -> Vec<BlockPos> {
    bes.sort_by_key(|p| (p.y, p.z, p.x));
    object_map_order(&object_map_order(&bes))
}

/// The chunks of a forced box `[minX, minZ, maxX, maxZ]` in the order the bridge forces them
/// again (`Sandbox.reload`: x, then z).
pub fn box_chunks(b: [i32; 4]) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for cx in b[0]..=b[2] {
        for cz in b[1]..=b[3] {
            v.push((cx, cz));
        }
    }
    // (a bisect switch for the load order: MCTAS_RELOAD_ORDER=reverse)
    if std::env::var("MCTAS_RELOAD_ORDER").as_deref() == Ok("reverse") {
        v.reverse();
    }
    v
}

/// Save, unload and load again `chunks` (in that load order) between two ticks.
pub fn reload_chunks(w: &mut World, chunks: &[(i32, i32)]) {
    let set: std::collections::HashSet<(i32, i32)> = chunks.iter().copied().collect();
    let reloaded = |p: BlockPos| set.contains(&p.chunk());
    if w.entities.list.iter().any(|e| !e.removed && reloaded(e.block_position())) {
        let e = "a chunk reload with entities in it: entities across a reload (new objects, ids and randoms) are not modeled".to_string();
        if !w.errors.contains(&e) {
            w.errors.push(e);
        }
    }
    w.ticks.reload(chunks, chunks, 0);
    w.fluid_ticks.reload(chunks, chunks, 0);
    reregister_block_entities(w, chunks);
}

/// The block entities of `chunks` registered again (a chunk loaded back), chunk by chunk in its
/// block-entity map's order, after the tickers of the chunks that stayed.
fn reregister_block_entities(w: &mut World, chunks: &[(i32, i32)]) {
    let set: std::collections::HashSet<(i32, i32)> = chunks.iter().copied().collect();
    let reloaded = |p: BlockPos| set.contains(&p.chunk());
    // the block entities' registration order, chunk by chunk
    let mut order: Vec<BlockPos> = Vec::new();
    for &(cx, cz) in chunks {
        let mut bes = Vec::new();
        let (x0, z0) = ((cx * 16).max(w.min.x), (cz * 16).max(w.min.z));
        let (x1, z1) = ((cx * 16 + 15).min(w.min.x + w.size[0] - 1), (cz * 16 + 15).min(w.min.z + w.size[2] - 1));
        for y in w.min.y..w.min.y + w.size[1] {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let p = BlockPos::new(x, y, z);
                    if crate::behavior::has_block_entity(&w.get(p)) {
                        bes.push(p);
                    }
                }
            }
        }
        order.extend(registration_order(bes));
    }
    // redsim ticks its block entities by kind (hoppers and the other container tickers, then the
    // sensors, then the moving blocks): an order that interleaves them is not modeled
    let kind = |w: &World, p: BlockPos| -> Option<u8> {
        match w.get(p) {
            crate::block::Block::Sculk { .. } => Some(1),
            crate::block::Block::MovingPiston { .. } => Some(2),
            b if crate::behavior::has_block_entity(&b) => Some(0),
            _ => None,
        }
    };
    let live_kinds: Vec<u8> = order
        .iter()
        .filter(|p| match kind(w, **p) {
            Some(2) => w.piston.be(**p).is_some(),
            Some(1) => w.sculk.bes.iter().any(|(q, _)| q == *p),
            Some(0) => w.containers.map.contains_key(*p),
            _ => false,
        })
        .filter_map(|p| kind(w, *p))
        .collect();
    if live_kinds.windows(2).any(|k| k[1] < k[0]) {
        let e = "a chunk reload registers block entities of different kinds interleaved (redsim ticks hoppers, then sensors, then moving blocks)".to_string();
        if !w.errors.contains(&e) {
            w.errors.push(e);
        }
    }
    w.containers.reload_tickers(&reloaded, &order);
    w.sculk.reload_tickers(&reloaded, &order);
    w.piston.reload(&reloaded, &order);
}

/// (tickexact 2026-11-10, ironspec's fault rule) What a world's chunks hold while they are saved
/// and unloaded ([`unload_all`] until [`load_all`]).
#[derive(Clone, Debug)]
pub struct Away {
    /// The game time they left.
    pub since: i64,
    /// The chunks, in the order they load again (the bridge forces them back x, then z).
    pub chunks: Vec<(i32, i32)>,
    pub ticks: crate::ticks::SavedTicks<crate::block::Kind>,
    pub fluid_ticks: crate::ticks::SavedTicks<crate::fluid::Fluid>,
    /// The entities as their chunks saved them, in the order they come back (chunk by chunk in
    /// load order; in a chunk, its sections in key order and each section's entities in the order
    /// they entered it; a vehicle with its passengers after it: `saveAsPassenger`).
    pub entities: Vec<crate::entity::Entity>,
}

/// The save order of world `w`'s entities in `chunks` (see [`Away::entities`]): their indices.
fn save_order(w: &World, chunks: &[(i32, i32)]) -> Vec<usize> {
    let mut out = Vec::new();
    for &ch in chunks {
        let mut roots: Vec<usize> = (0..w.entities.list.len())
            .filter(|&i| {
                let e = &w.entities.list[i];
                !e.removed && e.vehicle.is_none() && e.block_position().chunk() == ch
            })
            .collect();
        roots.sort_by_key(|&i| {
            let e = &w.entities.list[i];
            (crate::entity::section_order(e.section), e.section_seq)
        });
        fn with_passengers(w: &World, i: usize, out: &mut Vec<usize>) {
            out.push(i);
            for p in &w.entities.list[i].passengers {
                if let Some(j) = crate::entity::index_of(w, *p) {
                    with_passengers(w, j, out);
                }
            }
        }
        for i in roots {
            with_passengers(w, i, &mut out);
        }
    }
    out
}

/// (tickexact 2026-11-10, ironspec's fault rule) Every chunk of world `w` (its forced box) saved
/// and unloaded between two ticks (the bridge's `unload`: a player leaving): their scheduled
/// ticks taken out with their trigger times (`LevelChunkTicks.pack`), their entities saved and
/// gone. Until [`load_all`] the level ticks on (game time and day time run; pending block events
/// wait for their chunk) and nothing of the chunks ticks. A bench's world (no sandbox box) unloads
/// the chunks its box meets. Not modeled (a model error): a world with players or falling blocks.
pub fn unload_all(w: &mut World) {
    if w.away.is_some() {
        return crate::reload::error(w, "an unload of a world already unloaded".to_string());
    }
    let b = w.entity_chunks.unwrap_or_else(|| {
        // (a bench's world, no bridge sandbox: the chunks its box meets)
        let (x1, z1) = (w.min.x + w.size[0] - 1, w.min.z + w.size[2] - 1);
        [w.min.x.div_euclid(16), w.min.z.div_euclid(16), x1.div_euclid(16), z1.div_euclid(16)]
    });
    if !w.mob.ai.players.is_empty() {
        return crate::reload::error(w, "an unload with a player (its tickets keep the chunks around it loaded) is not modeled".to_string());
    }
    if !w.gravity.entities.is_empty() {
        return crate::reload::error(w, "an unload with falling blocks is not modeled".to_string());
    }
    let chunks = box_chunks(b);
    let order = save_order(w, &chunks);
    let entities: Vec<crate::entity::Entity> = order.iter().map(|&i| w.entities.list[i].clone()).collect();
    for e in w.entities.list.iter_mut() {
        e.removed = true;
    }
    w.entities.list.clear();
    let ticks = w.ticks.unload(&chunks);
    let fluid_ticks = w.fluid_ticks.unload(&chunks);
    w.away = Some(Box::new(Away { since: w.time, chunks, ticks, fluid_ticks, entities }));
}

/// (tickexact 2026-11-10, ironspec's fault rule) The chunks [`unload_all`] saved, loaded again
/// between two ticks (the bridge's `load`: the player back). Their scheduled ticks come back
/// with their delays (the time away added, `unpack`) numbered -N..-1 chunk by chunk; their block
/// entities register again; their entities come back as new objects (`EntityType.create` + their
/// saved data: `entity::restore_fresh`) in their save order, each the world's next entity (new
/// ids), seeded as the bridge seeds a loaded entity (its constructor's draws redone on the
/// entering random; its saved UUID, yaw and head kept), passengers seated again; a NeutralMob's
/// saved anger target resolved among the entities already back.
pub fn load_all(w: &mut World) {
    let Some(away) = w.away.take() else {
        return crate::reload::error(w, "a load of a world that is not unloaded".to_string());
    };
    let Away { since, chunks, mut ticks, mut fluid_ticks, entities } = *away;
    let delta = w.time - since;
    w.ticks.load(&chunks, &mut ticks, delta);
    w.fluid_ticks.load(&chunks, &mut fluid_ticks, delta);
    reregister_block_entities(w, &chunks);
    // new ids in load order
    let mut ids: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for e in &entities {
        ids.insert(e.id, w.entities.next_id);
        w.entities.next_id += 1;
    }
    // (an id renumbered before follows to its newest)
    for v in w.renumbered.values_mut() {
        if let Some(n) = ids.get(v) {
            *v = *n;
        }
    }
    for (o, n) in &ids {
        w.renumbered.insert(*o, *n);
    }
    let mut back: Vec<u32> = Vec::new();
    for mut e in entities {
        let old_anger = match &e.kind {
            crate::entity::EntityKind::Mob(m) => m.ai.as_deref().and_then(|a| a.anger_target),
            _ => None,
        };
        let id = ids[&e.id];
        e.id = id;
        e.network_id = id as i32;
        e.removed = false;
        e.vehicle = e.vehicle.and_then(|v| ids.get(&v).copied());
        e.passengers = e.passengers.iter().filter_map(|p| ids.get(p).copied()).collect();
        crate::entity::restore_fresh(&mut e);
        // readPersistentAngerSaveData: the anger target by its UUID (kept across the reload), the
        // target set only if that entity is already back (an earlier chunk's)
        if let crate::entity::EntityKind::Mob(m) = &mut e.kind
            && let Some(a) = m.ai.as_deref_mut()
        {
            a.anger_target = old_anger.map(|t| ids.get(&t).copied().unwrap_or(t));
            a.target = a.anger_target.filter(|t| back.contains(t));
        }
        let egg = match &e.kind {
            crate::entity::EntityKind::Mob(m) => Some(m.egg_time),
            _ => None,
        };
        let item = matches!(e.kind, crate::entity::EntityKind::Item(_));
        let living = matches!(e.kind, crate::entity::EntityKind::Mob(_));
        w.entities.insert(e);
        if w.exact_seed.is_some() {
            if living {
                crate::entity::mob::seed_entered(w, id);
                // (the chicken's egg timer is saved: EggLayTime loads over the constructor's)
                if let (Some(t), Some(i)) = (egg, crate::entity::index_of(w, id))
                    && let crate::entity::EntityKind::Mob(m) = &mut w.entities.list[i].kind
                {
                    m.egg_time = t;
                }
                rebuild_villager_brain(w, id);
            } else {
                let mut own = w.entering_random(id);
                crate::rng::replay_uuid(&mut own);
                if item {
                    // bobOffs and the yaw (the saved yaw loads over it)
                    own.next_float();
                    own.next_float();
                }
                if let Some(i) = crate::entity::index_of(w, id) {
                    w.entities.list[i].random = Some(own);
                }
            }
        }
        back.push(id);
    }
}

/// (tickexact 2026-11-10, ironspec's fault rule: a partial unload, the edge of the loaded area
/// through the part as the player leaves) The forced tickets of the chunks of box `b` [x0, z0, x1,
/// z1] dropped (the bridge's `unload` of those chunks), between two ticks. Each chunk's simulation
/// level rises to 31 plus its distance to the nearest chunk still forced (`World::chunk_level`):
/// at 32 its blocks tick (scheduled ticks, block events, block entities) but no entity and no
/// random tick, at 33 nothing ticks; its entities stay the same objects and its scheduled ticks
/// their trigger times (the overdue ones run when it ticks again). Not modeled yet (a model
/// error): a chunk past 33 (its entities saved and unloaded, its blocks inaccessible: the part
/// wider than the loaded ring), every chunk dropped (the whole part: [`unload_all`]), natural
/// spawning or a player.
pub fn unload_some(w: &mut World, b: [i32; 4]) {
    let Some([x0, z0, x1, z1]) = w.entity_chunks else {
        return error(w, "a partial unload of a world without a forced chunk box".to_string());
    };
    if w.mob.natural.is_some() || !w.mob.ai.players.is_empty() {
        return error(w, "a partial unload with natural spawning or a player is not modeled".to_string());
    }
    for x in b[0].max(x0)..=b[2].min(x1) {
        for z in b[1].max(z0)..=b[3].min(z1) {
            if !w.unforced.contains(&(x, z)) {
                w.unforced.push((x, z));
            }
        }
    }
    if (x0..=x1).all(|x| (z0..=z1).all(|z| w.unforced.contains(&(x, z)))) {
        w.unforced.clear();
        return error(w, "a partial unload of every chunk: the whole part is `unload all`".to_string());
    }
    if (x0..=x1).any(|x| (z0..=z1).any(|z| w.chunk_level(x, z) > 33)) {
        error(w, "a partial unload leaving a chunk of the part past the loaded levels (its entities saved and unloaded) is not modeled".to_string());
    }
}

/// The forced tickets of the chunks of box `b` back ([`unload_some`]'s other half). A chunk whose
/// entities tick again puts them back in the level's tick list at its end (`EntityTickList`:
/// `stopTicking` took them out when it stopped, `startTicking` appends them), chunk by chunk in
/// the box's order, each chunk's sections in key order and each section's entities in the order
/// they entered it.
pub fn load_some(w: &mut World, b: [i32; 4]) {
    let was: Vec<(i32, i32)> = (b[0]..=b[2]).flat_map(|x| (b[1]..=b[3]).map(move |z| (x, z))).filter(|&(x, z)| w.chunk_level(x, z) > 31).collect();
    w.unforced.retain(|&(x, z)| !(x >= b[0] && x <= b[2] && z >= b[1] && z <= b[3]));
    let back: Vec<(i32, i32)> = was.into_iter().filter(|&(x, z)| w.chunk_level(x, z) <= 31).collect();
    if back.is_empty() {
        return;
    }
    let mut moved: Vec<crate::entity::Entity> = Vec::new();
    for ch in back {
        let mut here: Vec<usize> = (0..w.entities.list.len()).filter(|&i| !w.entities.list[i].removed && w.entities.list[i].block_position().chunk() == ch).collect();
        here.sort_by_key(|&i| {
            let e = &w.entities.list[i];
            (crate::entity::section_order(e.section), e.section_seq)
        });
        for i in here {
            moved.push(w.entities.list[i].clone());
            w.entities.list[i].removed = true;
        }
    }
    w.entities.list.retain(|e| !e.removed);
    w.entities.list.extend(moved);
}

/// A loaded villager's brain (`Villager.readAdditionalSaveData` -> `refreshBrain`: `makeBrain` of
/// its packed memories, then `registerBrainGoals`), as the bridge's seeding of a loaded entity
/// leaves it: the live brain's 9 sensor delays drawn from its seeded random (after the UUID and
/// the yaw), no behavior running, the schedule's activity at the game time (WORK and MEET without
/// their memories: IDLE), and only the memories registered with a codec kept (HOME, LAST_SLEPT,
/// LAST_WOKEN, GOLEM_DETECTED_RECENTLY with its time left, JOB_SITE, POTENTIAL_JOB_SITE,
/// MEETING_POINT, SECONDARY_JOB_SITE; not the sensed ones, NEAREST_HOSTILE, HURT_BY or the walk
/// and look targets). Its exact brain's shuffling lists keyed by the entity's seed, generation 0
/// (the bridge seeds the live brain of a loaded villager as the first brain it saw).
fn rebuild_villager_brain(w: &mut World, id: u32) {
    let Some(i) = crate::entity::index_of(w, id) else { return };
    let (t, day) = (w.time, w.day_time());
    let es = w.exact_seed.map(|s| crate::rng::entity_seed(s, id));
    let ai_on = w.mob.ai.on;
    let e = &mut w.entities.list[i];
    let crate::entity::EntityKind::Mob(m) = &mut e.kind else { return };
    let Some(old) = m.brain.take() else { return };
    let Some(own) = e.random.as_mut() else {
        m.brain = Some(old);
        return;
    };
    let mut b = crate::entity::mob::villager::Brain::new(own, t, day);
    b.home = old.home;
    b.last_slept = old.last_slept;
    b.last_woken = old.last_woken;
    b.golem_detected = old.golem_detected;
    if ai_on && let (Some(ox), Some(es)) = (old.ex.as_deref(), es) {
        use crate::entity::mob::ai::brain::{Act, Exact, scheduled};
        // (generation 0: the bridge seeds a loaded villager's live brain as the first it saw)
        let mut x = Exact::new(ox.baby, es, 0);
        x.mem.job_site = ox.mem.job_site;
        x.mem.potential_job_site = ox.mem.potential_job_site;
        x.mem.meeting_point = ox.mem.meeting_point;
        x.mem.secondary_job_site = ox.mem.secondary_job_site.clone();
        x.last_gossip = ox.last_gossip;
        x.last_schedule_update = t;
        let sched = scheduled(day);
        x.active = match sched {
            Act::Work if x.mem.job_site.is_some() => Act::Work,
            Act::Meet if x.mem.meeting_point.is_some() => Act::Meet,
            Act::Work | Act::Meet => Act::Idle,
            a => a,
        };
        b.activity = x.active.simple();
        b.last_schedule_update = t;
        b.ex = Some(Box::new(x));
    }
    m.brain = Some(Box::new(b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_map_walks_from_the_top_slot() {
        let keys = [BlockPos::new(0, 0, 0), BlockPos::new(1, 0, 0), BlockPos::new(0, 1, 0), BlockPos::new(5, 3, 2)];
        let order = object_map_order(&keys);
        assert_eq!(order.len(), 4);
        let slots: Vec<usize> = order.iter().map(|p| (mix_int(pos_hash(*p)) as u32 as usize) & 31).collect();
        let mut sorted = slots.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(slots, sorted);
    }
}
