//! Bamboo (clk-misc, 2026-10-21: the farm judge's second crop, w86's bamboo harvesters):
//! `BambooStalkBlock` and `BambooSaplingBlock`, 26.3.
//!
//! A stalk (`age` 0 thin / 1 thick, `leaves` none / small / large, `stage` 1 = done) stands on
//! #supports_bamboo and breaks a tick after it cannot. Its random tick (stage 0 only) draws
//! `nextInt(3)` from the level random; on a 0, with air above and the raw brightness there at least
//! 9, a stalk under 16 tall grows a block (`growBamboo`: the leaves pattern, thick from the third
//! block, `stage` 1 at 15, or from 11 with `nextFloat() < 0.25`). A shoot grows a small-leaved stalk
//! the same way and turns into a stalk under one. Both are offset in their cell by the position's
//! seed (`OffsetType.XZ`): the stalk's 3 px collision column too, which entities hit
//! (`entity::collide::boxes_at`). Pistons pop both.

use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// `BambooLeaves` names by `Block::Bamboo::leaves`.
pub const LEAVES: [&str; 3] = ["none", "small", "large"];

/// `Mth.getSeed(x, y, z)` (`x * 3129871` overflows as an int, as in Java).
fn seed(x: i32, y: i32, z: i32) -> i64 {
    let s = (x.wrapping_mul(3_129_871) as i64) ^ (z as i64).wrapping_mul(116_129_781) ^ y as i64;
    let s = s.wrapping_mul(s).wrapping_mul(42_317_861).wrapping_add(s.wrapping_mul(11));
    s >> 16
}

/// `OffsetType.XZ`'s offset of a cell (`getMaxHorizontalOffset` 0.25): (x, z).
pub fn offset(p: BlockPos) -> (f64, f64) {
    offset_max(p, 0.25)
}

/// `OffsetType.XZ`'s offset of a cell with the block's `getMaxHorizontalOffset` (a speleothem's
/// 0.125: its base column's edge).
pub fn offset_max(p: BlockPos, max: f32) -> (f64, f64) {
    let s = seed(p.x, 0, p.z);
    let f = |v: i64| ((((v & 15) as f32 / 15.0f32) as f64 - 0.5) * 0.5).clamp(-max as f64, max as f64);
    (f(s), f(s >> 8))
}

/// The stalk's collision box at `p` (`SHAPE_COLLISION` = column(3, 0, 16), moved by the offset).
pub fn collision_box(p: BlockPos) -> crate::entity::Aabb {
    let (ox, oz) = offset(p);
    let (lo, hi) = (6.5 / 16.0, 9.5 / 16.0);
    let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
    crate::entity::Aabb::new(x + (lo + ox), y, z + (lo + oz), x + (hi + ox), y + 1.0, z + (hi + oz))
}

/// #supports_bamboo among redsim's blocks: #sand, #substrate_overworld (dirt, grass, mud), gravel,
/// bamboo and its shoot.
fn supports(b: &Block) -> bool {
    match b {
        Block::Simple(s) => matches!(s, Simple::Dirt | Simple::GrassBlock | Simple::Mud),
        Block::Falling { id } => matches!(crate::block::FALLING[*id as usize], "sand" | "red_sand" | "gravel"),
        Block::Bamboo { .. } | Block::BambooSapling => true,
        _ => false,
    }
}

/// `canSurvive` (both): on #supports_bamboo.
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    supports(&w.get(p.below()))
}

/// `BambooStalkBlock.updateShape`: a stalk that cannot stand breaks a tick later; under a thicker
/// stalk it turns thick (`cycle(AGE)`).
pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if !can_survive(w, p) {
        w.schedule_tick(p, Kind::Bamboo, 1, Priority::Normal);
    }
    if let (Dir::Up, Block::Bamboo { age: above, .. }, Block::Bamboo { age, leaves, stage }) = (d, ns, s)
        && above > age
    {
        return Block::Bamboo { age: (age + 1) % 2, leaves, stage };
    }
    s
}

/// `BambooSaplingBlock.updateShape`: gone (with its drop) when it cannot stand, a plain stalk under
/// a stalk.
pub fn sapling_update_shape(w: &World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if !can_survive(w, p) {
        return Block::Air;
    }
    if d == Dir::Up && matches!(ns, Block::Bamboo { .. }) {
        return Block::Bamboo { age: 0, leaves: 0, stage: 0 };
    }
    s
}

/// `BambooStalkBlock.tick`: `destroyBlock(pos, true)` when it cannot stand.
pub fn tick(w: &mut World, p: BlockPos) {
    if !can_survive(w, p) {
        w.destroy_block(p, true, crate::world::UPDATE_LIMIT);
    }
}

/// `level.getRawBrightness(pos, 0) >= 9`: the sky light there (`daylight::sky_light_bounds`:
/// blocks redsim's light model does not classify count as opaque for the low bound, as air for
/// the high one); a reading between them, or a light source near enough to matter, is a model
/// error (`None`).
pub(crate) fn bright_enough(w: &World, p: BlockPos) -> Option<bool> {
    let (lo, hi) = crate::daylight::sky_light_bounds_from(w, p, 9);
    if lo >= 9 {
        return Some(true);
    }
    if hi >= 9 {
        return None;
    }
    // block light: a source within 6 (15 - 9) could still light it
    for dy in -6..=6i32 {
        for dz in -6..=6i32 {
            for dx in -6..=6i32 {
                if dx.abs() + dy.abs() + dz.abs() <= 6 && emits_light(&w.get(p.offset(dx, dy, dz))) {
                    return None;
                }
            }
        }
    }
    Some(false)
}

/// Blocks of redsim's that give light (`lightLevel` > 0).
fn emits_light(b: &Block) -> bool {
    match b {
        Block::Simple(s) => matches!(s, Simple::Glowstone | Simple::SeaLantern | Simple::Shroomlight | Simple::MagmaBlock),
        Block::Lamp { lit } | Block::Bulb { lit, .. } => *lit,
        Block::Torch { lit } | Block::WallTorch { lit, .. } => *lit,
        Block::Lantern { .. } | Block::Furnace { lit: true, .. } => true,
        _ => false,
    }
}

/// `getHeightBelowUpToMax`: stalks under `p`, at most 16.
fn height_below(w: &World, p: BlockPos) -> i32 {
    let mut h = 0;
    while h < 16 && matches!(w.get(p.offset(0, -(h + 1), 0)), Block::Bamboo { .. }) {
        h += 1;
    }
    h
}

/// One draw from the level random (a model error without it).
fn level_random<'a>(w: &'a mut World) -> Option<&'a mut crate::rng::LegacyRandom> {
    if w.level_random.is_none() {
        let why = "bamboo's random tick draws from the level random (World::mirror_level_random)".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return None;
    }
    w.level_random.as_mut()
}

/// `randomTick` of a stalk (stage 0) or a shoot.
pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    if matches!(s, Block::Bamboo { stage: 1, .. }) {
        return;
    }
    let Some(r) = level_random(w) else { return };
    if r.next_int(3) != 0 || !w.get(p.above()).is_air() {
        return;
    }
    match bright_enough(w, p.above()) {
        Some(true) => {}
        Some(false) => return,
        None => {
            let why = format!("the light above the bamboo at {p} (redsim's light model does not classify a block near it)");
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return;
        }
    }
    if s == Block::BambooSapling {
        // BambooSaplingBlock.growBamboo
        w.set_block(p.above(), Block::Bamboo { age: 0, leaves: 1, stage: 0 }, flags::ALL);
        return;
    }
    let height = height_below(w, p) + 1;
    if height < 16 {
        grow(w, s, p, height);
    }
}

/// `BambooStalkBlock.growBamboo(state, level, pos, random, height)`.
fn grow(w: &mut World, s: Block, p: BlockPos, height: i32) {
    let Block::Bamboo { age, .. } = s else { return };
    let below = w.get(p.below());
    let two_below_pos = p.offset(0, -2, 0);
    let two_below = w.get(two_below_pos);
    let mut leaves = 0u8;
    if height >= 1 {
        match below {
            Block::Bamboo { leaves: bl, .. } if bl != 0 => {
                leaves = 2;
                if let Block::Bamboo { age: ta, stage: ts, .. } = two_below {
                    if let Block::Bamboo { age: ba, stage: bs, .. } = below {
                        w.set_block(p.below(), Block::Bamboo { age: ba, leaves: 1, stage: bs }, flags::ALL);
                    }
                    w.set_block(two_below_pos, Block::Bamboo { age: ta, leaves: 0, stage: ts }, flags::ALL);
                }
            }
            _ => leaves = 1,
        }
    }
    let new_age = if age != 1 && !matches!(two_below, Block::Bamboo { .. }) { 0 } else { 1 };
    // (height < 11 || !(nextFloat() < 0.25)) && height != 15 ? 0 : 1: the draw only from 11 up
    let stage = if height < 11 {
        0
    } else {
        let Some(r) = level_random(w) else { return };
        let f = r.next_float();
        if !(f < 0.25) && height != 15 { 0 } else { 1 }
    };
    w.set_block(p.above(), Block::Bamboo { age: new_age, leaves, stage }, flags::ALL);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Mth.getSeed` and the XZ offset: the origin's seed is 0 (both offsets at the -0.25 clamp),
    /// and a negative cell goes through the int overflow and the arithmetic shift without a panic.
    #[test]
    fn offset_seed_like_java() {
        assert_eq!(seed(0, 0, 0), 0);
        let (ox, oz) = offset(BlockPos::new(0, 1, 0));
        assert_eq!((ox, oz), (-0.25, -0.25));
        let s = seed(-1, 0, 3);
        let x = ((-1i32).wrapping_mul(3_129_871) as i64) ^ (3i64 * 116_129_781);
        assert_eq!(s, x.wrapping_mul(x).wrapping_mul(42_317_861).wrapping_add(x.wrapping_mul(11)) >> 16);
    }
}
