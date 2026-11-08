//! Weathering copper (mobinfra 2026-11-08; the parent: "redsim models everything exactly"): the
//! copper blocks redsim models (bulbs, lightning rods, the waxed grate) as `WeatheringCopper`
//! sees them, read in the 26.3 source:
//!
//! * `WeatheringCopperBulbBlock` / `WeatheringLightningRodBlock.randomTick` ->
//!   `ChangeOverTimeBlock.changeOverTime`: with chance 0.05688889 (the level random's `nextFloat`)
//!   `getNextState`: every block within Manhattan distance 4 (`findBlocksInManhattan`, the block
//!   itself left out) that is weathering copper too (`isSameAgeingType`: unwaxed: waxed blocks are
//!   not `ChangeOverTimeBlock`s); a younger one aborts; else with `older` and `same` counted, the
//!   chance `((older + 1) / (older + same + 1))^2 * getChanceModifier()` (0.75 unaffected, else
//!   1), a second `nextFloat`, and the next stage (`setBlockAndUpdate`, the other properties kept).
//!   Randomly ticking while a next stage exists (`isRandomlyTicking`).
//! * `LightningBolt.clearCopperOnLightningStrike`: a struck weathering block goes back to
//!   unaffected; then (struck weathering or waxed copper) `nextInt(3) + 3` random walks of
//!   `nextInt(8) + 1` steps, each step the first weathering block among 10 random cells of the cube
//!   around (`BlockPos.randomInCube(random, 10, pos, 1)`: x, y, z each `nextInt(3)`, drawn lazily)
//!   losing a stage (`getPrevious`), the walk moving there; the level random throughout.

use crate::block::{Block, Oxid, Simple};
use crate::pos::BlockPos;
use crate::world::World;

/// A copper block's (stage, waxed); `None`: not copper.
pub fn weathering(b: &Block) -> Option<(Oxid, bool)> {
    match *b {
        Block::Bulb { oxid, waxed, .. } | Block::LightningRod { oxid, waxed, .. } => Some((oxid, waxed)),
        Block::Simple(Simple::WaxedCopperGrate) | Block::WaterloggedGrate => Some((Oxid::Unaffected, true)),
        _ => None,
    }
}

/// `b` at stage `o` (its other properties kept).
fn with_oxid(b: Block, o: Oxid) -> Block {
    match b {
        Block::Bulb { waxed, lit, powered, .. } => Block::Bulb { oxid: o, waxed, lit, powered },
        Block::LightningRod { facing, powered, waxed, .. } => Block::LightningRod { facing, powered, waxed, oxid: o },
        other => other,
    }
}

/// `WeatheringCopper` (unwaxed copper) and its stage.
fn weathering_copper(b: &Block) -> Option<Oxid> {
    match weathering(b) {
        Some((o, false)) => Some(o),
        _ => None,
    }
}

/// `isRandomlyTicking`: unwaxed, a next stage left.
pub fn randomly_ticking(b: &Block) -> bool {
    weathering_copper(b).is_some_and(|o| o.next().is_some())
}

/// `ChangeOverTimeBlock.changeOverTime` of the block `s` at `p` (its random tick).
pub fn change_over_time(w: &mut World, s: Block, p: BlockPos) {
    let Some(age) = weathering_copper(&s) else { return };
    if crate::weather::fire::rng(w).next_float() >= 0.05688889f32 {
        return;
    }
    let (mut older, mut same) = (0i32, 0i32);
    for dy in -4..=4i32 {
        for dz in -4..=4i32 {
            for dx in -4..=4i32 {
                if dx.abs() + dy.abs() + dz.abs() > 4 || (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                if let Some(o) = weathering_copper(&w.get(p.offset(dx, dy, dz))) {
                    if (o as u8) < (age as u8) {
                        return;
                    }
                    if o as u8 > age as u8 {
                        older += 1;
                    } else {
                        same += 1;
                    }
                }
            }
        }
    }
    let chance = (older + 1) as f32 / (older + same + 1) as f32;
    let modifier = if age == Oxid::Unaffected { 0.75f32 } else { 1.0f32 };
    let actual = chance * chance * modifier;
    if crate::weather::fire::rng(w).next_float() < actual
        && let Some(next) = age.next()
    {
        w.set_block_and_update(p, with_oxid(s, next));
    }
}

/// `LightningBolt.clearCopperOnLightningStrike` at the struck block `struck`.
pub fn clear_on_strike(w: &mut World, struck: BlockPos) {
    let s = w.get(struck);
    let Some((_, waxed)) = weathering(&s) else { return };
    if !waxed {
        w.set_block_and_update(struck, with_oxid(s, Oxid::Unaffected));
    }
    let walks = crate::weather::fire::rng(w).next_int(3) + 3;
    for _ in 0..walks {
        let steps = crate::weather::fire::rng(w).next_int(8) + 1;
        let mut at = struck;
        for _ in 0..steps {
            match random_step(w, at) {
                Some(q) => at = q,
                None => break,
            }
        }
    }
}

/// `randomStepCleaningCopper`: the first weathering block among 10 random cells of the cube
/// around `p` loses a stage; the walk's next cell.
fn random_step(w: &mut World, p: BlockPos) -> Option<BlockPos> {
    for _ in 0..10 {
        let r = crate::weather::fire::rng(w);
        let (dx, dy, dz) = (r.next_int(3) - 1, r.next_int(3) - 1, r.next_int(3) - 1);
        let q = p.offset(dx, dy, dz);
        let s = w.get(q);
        if let Some(o) = weathering_copper(&s) {
            if let Some(prev) = o.previous() {
                w.set_block_and_update(q, with_oxid(s, prev));
            }
            return Some(q);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids of all eight stages of both blocks round-trip; only unwaxed, unoxidized ones tick.
    #[test]
    fn copper_names_and_ticking() {
        for name in ["copper_bulb", "exposed_copper_bulb", "weathered_copper_bulb", "oxidized_copper_bulb", "waxed_copper_bulb", "waxed_oxidized_copper_bulb"] {
            let b = Block::parse(&format!("minecraft:{name}[lit=false,powered=false]")).unwrap();
            assert!(b.to_string().starts_with(&format!("minecraft:{name}[")), "{name}: {b}");
        }
        for name in ["lightning_rod", "weathered_lightning_rod", "waxed_exposed_lightning_rod"] {
            let b = Block::parse(&format!("minecraft:{name}[facing=up,powered=false,waterlogged=false]")).unwrap();
            assert!(b.to_string().starts_with(&format!("minecraft:{name}[")), "{name}: {b}");
        }
        let tick = |s: &str| randomly_ticking(&Block::parse(s).unwrap());
        assert!(tick("minecraft:copper_bulb[lit=false,powered=false]"));
        assert!(tick("minecraft:weathered_lightning_rod[facing=up,powered=false,waterlogged=false]"));
        assert!(!tick("minecraft:oxidized_copper_bulb[lit=false,powered=false]"));
        assert!(!tick("minecraft:waxed_copper_bulb[lit=false,powered=false]"));
    }

    /// A younger copper block within Manhattan 4 holds an older one back; alone, an exposed bulb
    /// weathers with its chance (two draws of the level random).
    #[test]
    fn copper_weathers_by_its_neighbours() {
        let bulb = |o: Oxid| Block::Bulb { oxid: o, waxed: false, lit: false, powered: false };
        let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 16, 16]);
        w.put(BlockPos::new(0, 4, 0), bulb(Oxid::Exposed));
        w.put(BlockPos::new(2, 4, 1), bulb(Oxid::Unaffected));
        for _ in 0..2000 {
            change_over_time(&mut w, bulb(Oxid::Exposed), BlockPos::new(0, 4, 0));
        }
        assert_eq!(w.get(BlockPos::new(0, 4, 0)), bulb(Oxid::Exposed));
        // alone it weathers (1 in 0.0569 tries pass the first draw, then always: no neighbours)
        w.put(BlockPos::new(2, 4, 1), Block::Air);
        let mut n = 0;
        while w.get(BlockPos::new(0, 4, 0)) == bulb(Oxid::Exposed) && n < 2000 {
            change_over_time(&mut w, bulb(Oxid::Exposed), BlockPos::new(0, 4, 0));
            n += 1;
        }
        assert_eq!(w.get(BlockPos::new(0, 4, 0)), bulb(Oxid::Weathered), "after {n} ticks");
    }
}
