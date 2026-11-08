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
