//! TNT and the dead coral fans of TNT dupers (track tnt, study 9; docs/circuits.md), read from the
//! 26.3 source.
//!
//! * `TntBlock` 44-97: `onPlace` (a different old block) and `neighborChanged` prime the TNT when
//!   it has a neighbor signal, then `removeBlock(pos, false)` (flags 3). `prime` adds a
//!   `PrimedTnt` (here: a record in `World::primed`, the entity's motion draws from
//!   `level.random`, and the judge removes primed TNT before it could explode) and emits
//!   `PRIME_FUSE` with no source entity. `UNSTABLE` only matters to players breaking it;
//!   `wasExploded` and burning projectiles never happen in redsim (no explosions, no projectiles).
//!   TNT is not a conductor (`isRedstoneConductor(Blocks::never)`), a full cube, pushable, and
//!   drops itself unless unstable (`VanillaBlockLoot`).
//! * Dead coral fans (`BaseCoralPlantTypeBlock` 65-87, `BaseCoralWallFanBlock` 51-74): pushable
//!   (no `POPPED`: slime drags them along), no collision, forced solid, drop nothing without silk
//!   touch (`dropWhenSilkTouch`), BASEDRUM. A floor fan needs a sturdy top face below and pops
//!   only on a DOWN shape update; a wall fan needs the `facing` face of the block behind it and
//!   pops only on the update from that side. A waterlogged fan schedules a water tick on every
//!   shape update first (`fluid::waterlogged_update_shape`). Dead fans schedule no dying tick.
//!
//! Why dupers work: `PistonBaseBlock.moveBlocks` copies the pushed states first (274) and blanks
//! the sources last (324); a fan riding the structure pops when the block under it turns
//! `moving_piston` (the flag-324 write updates shapes before the block entity exists: no support),
//! its `destroyBlock` updates a TNT still standing at its source, and a powered TNT primes while
//! its moving copy lands later.

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// The dead coral fan kinds (`dead_<kind>_coral_fan`), by `Block::CoralFan::kind`.
pub const DEAD_CORALS: [&str; 5] = ["tube", "brain", "bubble", "fire", "horn"];

/// The kick (`nextDouble()`) of the next TNT entity: the injected one (`World::tnt_kicks`, by
/// spawn index) or the level random's (drawn whenever it is mirrored, so later draws stay in
/// step) or `tnt_kick`. `None`: TNT are records only (no `tnt_kick`).
fn next_kick(w: &mut World) -> Option<f64> {
    let k = w.tnt_spawned as usize;
    w.tnt_spawned += 1;
    let drawn = w.rng().map(|r| r.next_double());
    let base = w.tnt_kick?;
    Some(w.tnt_kicks.get(k).copied().or(drawn).unwrap_or(base))
}

/// `TntBlock.prime(level, pos)` with no source: the primed TNT and `PRIME_FUSE`. The `PrimedTnt`
/// is an entity only with `World::tnt_kick` (its kick: `next_kick`); else a record that still
/// takes its entity id (later entities keep the game's ids).
fn prime(w: &mut World, p: BlockPos) -> bool {
    w.primed.push((w.time, p));
    // `new PrimedTnt(...)` draws its kick from the level random (`nextDouble()`)
    match next_kick(w) {
        Some(u) => {
            let pos = crate::entity::Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5);
            crate::entity::primed_tnt::spawn(w, pos, u);
        }
        None => w.entities.next_id += 1,
    }
    w.game_event("prime_fuse", p);
    true
}

/// `TntBlock.wasExploded` (an explosion destroyed the TNT block at `p`): `new PrimedTnt(level, x +
/// 0.5, y, z + 0.5, ..)` (its kick drawn first), then `setFuse(getRandomShortFuse(80, random))` =
/// `nextInt(20) + 10` (or the injected `World::chain_fuses`), `addFreshEntity` (it ticks from
/// the next tick).
pub fn was_exploded(w: &mut World, p: BlockPos) {
    w.chain_primed.push((w.time, p));
    let kick = next_kick(w);
    let k = w.chain_spawned as usize;
    w.chain_spawned += 1;
    let drawn = w.rng().map(|r| r.next_int(20) + 10);
    let fuse = w.chain_fuses.get(k).copied().or(drawn);
    match (kick, fuse) {
        (Some(u), Some(f)) => {
            let pos = crate::entity::Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5);
            crate::entity::primed_tnt::spawn(w, pos, u);
            if let Some(e) = w.entities.list.last_mut()
                && let crate::entity::EntityKind::PrimedTnt(s) = &mut e.kind
            {
                s.fuse = f;
            }
        }
        (Some(_), None) => {
            w.entities.next_id += 1;
            w.errors.push(format!("explosion: the TNT it primes at {p} needs a fuse (World::chain_fuses or the mirrored level random)"));
        }
        (None, _) => w.entities.next_id += 1,
    }
}

/// `TntBlock.onPlace`.
pub fn on_place(w: &mut World, p: BlockPos, old: Block) {
    if old.kind() != Kind::Tnt && w.has_neighbor_signal(p) && prime(w, p) {
        w.remove_block(p, false);
    }
}

/// `TntBlock.neighborChanged`.
pub fn neighbor_changed(w: &mut World, p: BlockPos) {
    if w.has_neighbor_signal(p) && prime(w, p) {
        w.remove_block(p, false);
    }
}

/// `canSurvive` of a dead coral fan (floor or wall).
pub fn fan_can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
    match *s {
        Block::CoralFan { .. } => {
            let below = p.below();
            bh::sturdy(w, &w.get(below), below, Dir::Up, Support::Full)
        }
        Block::CoralWallFan { facing, .. } => {
            let q = p.rel(facing.opposite());
            bh::sturdy(w, &w.get(q), q, facing, Support::Full)
        }
        _ => true,
    }
}

/// `updateShape` of a dead coral fan (floor or wall).
pub fn fan_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir) -> Block {
    crate::fluid::waterlogged_update_shape(w, &s, p);
    let pops = match s {
        Block::CoralFan { .. } => d == Dir::Down,
        Block::CoralWallFan { facing, .. } => d.opposite() == facing,
        _ => false,
    };
    if pops && !fan_can_survive(w, &s, p) { Block::Air } else { s }
}

#[cfg(test)]
mod coral_tests {
    use crate::block::Block;

    /// ironspec 08:05, tick-exact's citation (Blocks 4160-4168 / 4218; BlockBehaviour.calculateSolid 488-510):
    /// a DEAD coral fan, floor or wall, is solid (forceSolidOn over noCollision), a LIVE one isn't (noCollision,
    /// no force flag). redsim models the dead fans only: each of the five reads solid (`is_solid`'s default
    /// arm, the forceSolidOn), and a live fan, floor or wall, of every colour is refused at parse ("not
    /// modeled"), so no redsim world holds one for `is_solid` to answer.
    #[test]
    fn coral_fans_dead_solid_live_refused() {
        for kind in super::DEAD_CORALS {
            let dead = Block::parse(&format!("minecraft:dead_{kind}_coral_fan[waterlogged=false]")).unwrap();
            let dead_wall = Block::parse(&format!("minecraft:dead_{kind}_coral_wall_fan[facing=north,waterlogged=false]")).unwrap();
            assert!(crate::behavior::is_solid(&dead) && crate::behavior::is_solid(&dead_wall), "dead {kind}: solid");
            for live in [format!("minecraft:{kind}_coral_fan[waterlogged=true]"), format!("minecraft:{kind}_coral_wall_fan[facing=north,waterlogged=true]")] {
                assert!(Block::parse(&live).is_err(), "{live}: refused, not modeled");
            }
        }
    }
}
