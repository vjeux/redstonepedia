//! Sunburn (mobinfra, 2026-11-08), read in the 26.3 source: `Mob.aiStep` ends with
//! `burnUndead()` for the `burn_in_daylight` entity types (zombies among the modeled mobs), after
//! `LivingEntity.aiStep`. `isSunBurnTick`: the day timeline's `MONSTERS_BURN` (day time
//! 23460..12542), then `br = getLightLevelDependentMagicValue()` at the eye's block (the raw
//! brightness `max(block, sky - skyDarken)`, curved `v / (4 - 3v)`, the overworld's ambient light
//! 0), and it burns when `br > 0.5`, the mob's own `random.nextFloat() * 30 < (br - 0.4) * 2`, it
//! is neither in water, in rain nor in powder snow, and its eye's block sees the sky (sky light
//! 15). A helmet takes the burn instead (the judge's mobs wear none); otherwise
//! `igniteForSeconds(8)`, and the fire hurts 1 every 20 fire ticks (`Entity.baseTick`).
//!
//! Raw brightness 12 or more is `br > 0.5` in the game's float math (12 gives 0.50000006: 12/15
//! is 0.8000000119 and 4 - 3 x that 1.5999999), 11 gives 0.407: under the open sky that is a
//! darkening of 3 at most. So a mob under glass in rain (darkening 3, no rain at its block) still
//! burns now and then (`(br - 0.4) * 2 / 30` = 0.67% a tick), one in thunder (5) does not. Below
//! 12 from the sky only block light could lift `br`: with no light source within 15 blocks of the
//! eye it is 0 (exact); with one, the block light engine is needed (a model error until it lands).

use super::types::MobType;
use super::{mob, mob_mut};
use crate::block::{Block, Simple};
use crate::pos::BlockPos;
use crate::weather::WeatherEvent;
use crate::world::World;

/// The day timeline's `MONSTERS_BURN` (`BooleanModifier.OR` over false, keyframes 12542 false
/// and 23460 true, sampled with the step lerp: on from 23460 through 12541).
pub fn monsters_burn(day: i64) -> bool {
    let t = day.rem_euclid(24000);
    !(12542..23460).contains(&t)
}

/// `EntityTypeTags.BURN_IN_DAYLIGHT` among the modeled mob types.
pub fn burns_in_daylight(ty: MobType) -> bool {
    matches!(ty, MobType::Zombie)
}

/// `LevelReader.getLightLevelDependentMagicValue` of a raw brightness (ambient light 0).
pub fn magic_value(raw: i32) -> f32 {
    let v = raw as f32 / 15.0f32;
    let curved = v / (4.0f32 - 3.0f32 * v);
    crate::weather::lerp(0.0, curved, 1.0)
}

/// The light a block gives off (`lightEmission`) among the modeled blocks.
fn emission(b: &Block) -> i32 {
    match *b {
        Block::Lava { .. } => 15,
        Block::Torch { lit: true } | Block::WallTorch { lit: true, .. } => 7,
        Block::Lamp { lit: true } => 15,
        Block::Lantern { .. } => 15,
        Block::Furnace { lit: true, .. } => 13,
        Block::Bulb { lit: true, oxid, .. } => match oxid {
            crate::block::Oxid::Unaffected => 15,
            crate::block::Oxid::Exposed => 12,
            crate::block::Oxid::Weathered => 8,
            crate::block::Oxid::Oxidized => 4,
        },
        Block::RespawnAnchor { charges } => [0, 3, 7, 11, 15][charges.min(4) as usize],
        Block::Simple(Simple::Glowstone | Simple::SeaLantern | Simple::Shroomlight) => 15,
        Block::Simple(Simple::MagmaBlock) => 3,
        Block::Sculk { phase: 1, .. } => 1,
        _ => 0,
    }
}

/// A light source within 15 blocks (taxicab) of `p`: the block light there may not be 0.
fn source_near(w: &World, p: BlockPos) -> Option<(BlockPos, Block)> {
    for dy in -15..=15i32 {
        for dz in -15..=15i32 {
            for dx in -15..=15i32 {
                if dx.abs() + dy.abs() + dz.abs() > 15 {
                    continue;
                }
                let q = p.offset(dx, dy, dz);
                let b = w.get(q);
                if emission(&b) > dx.abs() + dy.abs() + dz.abs() {
                    return Some((q, b));
                }
            }
        }
    }
    None
}

/// `Entity.isInWaterOrRain`: touching water (`wasTouchingWater`) or rain at its feet's block or at
/// the block of its box's top (`isInRain`).
pub fn in_water_or_rain(w: &mut World, i: usize) -> bool {
    if w.entities.list[i].fluid.in_water() {
        return true;
    }
    crate::weather::entity_in_rain(w, i)
}

/// In powder snow (`isInPowderSnow` this tick; the box meets a powder snow block).
fn in_powder_snow(w: &World, i: usize) -> bool {
    let bb = w.entities.list[i].bb;
    let (x0, y0, z0) = (bb.min_x.floor() as i32, bb.min_y.floor() as i32, bb.min_z.floor() as i32);
    let (x1, y1, z1) = (bb.max_x.floor() as i32, bb.max_y.floor() as i32, bb.max_z.floor() as i32);
    (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| matches!(w.get(BlockPos::new(x, y, z)), Block::PowderSnow))))
}

/// `Mob.burnUndead` of mob `i` (the caller: the end of its `aiStep`).
pub fn burn_undead(w: &mut World, i: usize) {
    let m = mob(w, i);
    if !burns_in_daylight(m.ty) || m.dead || m.health <= 0.0 {
        return;
    }
    if is_sun_burn_tick(w, i) {
        let id = w.entities.list[i].id;
        let m = mob_mut(w, i);
        // igniteForSeconds(8): igniteForTicks(160)
        if m.fire_ticks < 160 {
            m.fire_ticks = 160;
        }
        let t = w.time;
        w.weather.events.push(WeatherEvent::Sunburn { t, entity: id });
    }
}

/// `Mob.isSunBurnTick`.
fn is_sun_burn_tick(w: &mut World, i: usize) -> bool {
    if !monsters_burn(w.day_time()) {
        return false;
    }
    let e = &w.entities.list[i];
    let eye_y = e.pos.y + mob(w, i).eye() as f64;
    let eye = BlockPos::new(e.pos.x.floor() as i32, eye_y.floor() as i32, e.pos.z.floor() as i32);
    // canSeeSky decides first here (no burn without it whatever br is: the draw it skips is the
    // stand-in random's)
    let sky = match crate::weather::can_see_sky(w, eye) {
        Ok(v) => v,
        Err(e) => {
            let why = format!("sunburn: the sky light at the zombie's eye {eye}: {e}");
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return false;
        }
    };
    if !sky {
        return false;
    }
    let sky_part = 15 - w.sky_darken();
    let raw = if sky_part >= 12 {
        sky_part
    } else if let Some((q, b)) = source_near(w, eye) {
        let why = format!("sunburn: the block light at the zombie's eye {eye} (the light of {b} at {q}) under the sky at a darkening of {}: the block light engine is not modeled", 15 - sky_part);
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return false;
    } else {
        sky_part
    };
    let br = magic_value(raw);
    if br <= 0.5f32 {
        return false;
    }
    let roll = match w.entities.list[i].random.as_mut() {
        Some(r) => r.next_float(),
        None => w.mob.rng_mut().next_float(),
    };
    if roll * 30.0f32 >= (br - 0.4f32) * 2.0f32 {
        return false;
    }
    !(in_water_or_rain(w, i) || in_powder_snow(w, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The burn window and the light cut: raw 12 burns (br 0.50000006 in float math), 11 does not.
    #[test]
    fn burn_window_and_light_cut() {
        assert!(monsters_burn(0) && monsters_burn(6000) && monsters_burn(12541) && monsters_burn(23460));
        assert!(!monsters_burn(12542) && !monsters_burn(18000) && !monsters_burn(23459));
        assert_eq!(magic_value(15), 1.0);
        assert_eq!(magic_value(12), 0.500_000_06);
        assert!(magic_value(12) > 0.5 && magic_value(11) < 0.5);
        // in rain at noon the open sky gives 15 - 3 = 12 (a mob under glass can burn), thunder 10
        assert_eq!(crate::weather::sky_darken_at(6000, 1.0, 0.0), 3);
        assert_eq!(crate::weather::sky_darken_at(6000, 1.0, 1.0), 5);
    }
}
