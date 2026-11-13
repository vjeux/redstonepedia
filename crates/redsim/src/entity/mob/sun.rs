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
use crate::block::Block;
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
    // the `burn_in_daylight` entity type tag (zombies, zombie villagers, drowned, skeletons, ...:
    // the rows' flag)
    ty.def().burns_in_daylight
}

/// `LevelReader.getLightLevelDependentMagicValue` of a raw brightness (ambient light 0).
pub fn magic_value(raw: i32) -> f32 {
    let v = raw as f32 / 15.0f32;
    let curved = v / (4.0f32 - 3.0f32 * v);
    crate::weather::lerp(0.0, curved, 1.0)
}

/// The light a block gives off (`BlockState.getLightEmission`) (cc767b78, G1 12:07 / 12:38, ironspec
/// 12:54: the hand list this replaces lacked the plain torch's 14, the soul torch's 10, crying
/// obsidian's 10, the nether portal's 11, jack o'lanterns' and froglights' 15, and gave a sculk sensor
/// 1 only while active): the game's block table (`terrain::light::props_of`: the natural-state rows
/// read from the server, a terrain cell's own row, fire's), and for a modeled state the table doesn't
/// hold, [`modeled_emission`], which names every variant.
pub fn emission(b: &Block) -> i32 {
    match crate::terrain::light::props_of(b) {
        Ok(p) => p.emission as i32,
        Err(_) => modeled_emission(b),
    }
}

/// The light of each redsim-modeled block by the game's block definitions (every variant named, no
/// default: a new one must say its light), for the states the block table doesn't hold: the copper
/// bulbs (Blocks 5331-5349, `WeatheringCopperCollection.registerBlocks`: `litBlockEmission`, 15 / 12 /
/// 8 / 4 by weathering when lit, the waxed ones the same), the respawn anchor (Blocks 4955-4965:
/// `getScaledChargeLevel(state, 15)`, 0 / 3 / 7 / 11 / 15 by charge) and the sculk sensors, calibrated
/// or not, in every phase (Blocks 5202-5210: `lightLevel(state -> 1)`; the ACTIVE-only part is
/// `emissiveRendering`, client rendering), all as tick-exact read them in the 26.3 source on
/// 2026-11-13; the others as the game's `lightLevel`s (`Blocks`): a lit redstone torch 7, a lit lamp
/// 15, the end portal frame 1, lanterns 15 and soul lanterns 10, torches 14 and soul torches 10, jack
/// o'lanterns and froglights 15, a lit campfire 15 (soul 10), the end rod 14, a live (waterlogged) sea
/// pickle 3 + 3 x its count, a lit candle 3 x its count, a lit furnace 13, the brewing stand 1, lava 15,
/// fire 15 (soul fire 10), glowstone, sea lanterns and shroomlights 15, crying obsidian 10, magma 3;
/// every other modeled block 0.
pub fn modeled_emission(b: &Block) -> i32 {
    use crate::block::{Oxid, Simple as S};
    match *b {
        Block::Terrain(id) => crate::terrain::blocks::props(id).emission as i32,
        Block::Simple(s) => match s {
            S::Glowstone | S::SeaLantern | S::Shroomlight => 15,
            S::CryingObsidian => 10,
            S::MagmaBlock => 3,
            // (every other one named: a new Simple variant must say its light, G1 14:55 C1-H1)
            S::Stone
            | S::SmoothStone
            | S::Cobblestone
            | S::Obsidian
            | S::Glass
            | S::TintedGlass
            | S::WhiteWool
            | S::OakPlanks
            | S::GoldBlock
            | S::IronBlock
            | S::Clay
            | S::EmeraldBlock
            | S::PackedIce
            | S::SoulSand
            | S::SlimeBlock
            | S::HoneyBlock
            | S::WaxedCopperGrate
            | S::Granite
            | S::PolishedGranite
            | S::Diorite
            | S::Andesite
            | S::MossyCobblestone
            | S::StoneBricks
            | S::Tuff
            | S::Calcite
            | S::Bricks
            | S::Sandstone
            | S::NetherBricks
            | S::EndStone
            | S::Blackstone
            | S::QuartzBlock
            | S::Prismarine
            | S::PurpurBlock
            | S::Terracotta
            | S::CoalBlock
            | S::RawIronBlock
            | S::MudBricks
            | S::WhiteConcrete
            | S::BlackConcrete
            | S::DiamondBlock
            | S::LapisBlock
            | S::NetheriteBlock
            | S::SoulSoil
            | S::DriedKelpBlock
            | S::NetherWartBlock
            | S::WarpedWartBlock
            | S::AmethystBlock
            | S::Bookshelf
            | S::CraftingTable
            | S::SprucePlanks
            | S::OrangeWool
            | S::BlackWool
            | S::Pumpkin
            | S::WhiteStainedGlass
            | S::BlackStainedGlass
            | S::Mud
            | S::Dirt
            | S::GrassBlock
            | S::Bedrock
            | S::MagentaWool
            | S::LightBlueWool
            | S::YellowWool
            | S::LimeWool
            | S::PinkWool
            | S::GrayWool
            | S::LightGrayWool
            | S::CyanWool
            | S::PurpleWool
            | S::BlueWool
            | S::BrownWool
            | S::GreenWool
            | S::RedWool
            | S::Melon => 0,
        },
        Block::Torch { lit } | Block::WallTorch { lit, .. } => {
            if lit {
                7
            } else {
                0
            }
        }
        Block::Lamp { lit } => {
            if lit {
                15
            } else {
                0
            }
        }
        Block::Bulb { lit, oxid, .. } => match (lit, oxid) {
            (false, _) => 0,
            (true, Oxid::Unaffected) => 15,
            (true, Oxid::Exposed) => 12,
            (true, Oxid::Weathered) => 8,
            (true, Oxid::Oxidized) => 4,
        },
        Block::EndPortalFrame { .. } => 1,
        Block::RespawnAnchor { charges } => match charges {
            0 => 0,
            1 => 3,
            2 => 7,
            3 => 11,
            4 => 15,
            // (refused, not clamped: G1 14:55 C1-H2; the parser admits 0..=4)
            _ => panic!("a respawn anchor with {charges} charges (the game's are 0..=4)"),
        },
        Block::Sculk { .. } => 1,
        Block::Lantern { .. } | Block::JackOLantern { .. } | Block::Froglight { .. } => 15,
        Block::SoulLantern { .. } => 10,
        Block::PlainTorch { soul } | Block::PlainWallTorch { soul, .. } => {
            if soul {
                10
            } else {
                14
            }
        }
        Block::Campfire { lit, soul, .. } => match (lit, soul) {
            (false, _) => 0,
            (true, false) => 15,
            (true, true) => 10,
        },
        Block::EndRod { .. } => 14,
        Block::SeaPickle { pickles, waterlogged } => {
            if waterlogged {
                3 + 3 * pickles as i32
            } else {
                0
            }
        }
        Block::Candle { candles, lit } => {
            if lit {
                3 * candles as i32
            } else {
                0
            }
        }
        Block::Furnace { lit, .. } => {
            if lit {
                13
            } else {
                0
            }
        }
        Block::BrewingStand { .. } => 1,
        Block::Lava { .. } => 15,
        Block::Fire { soul, .. } => {
            if soul {
                10
            } else {
                15
            }
        }
        Block::Air
        | Block::Glazed { .. }
        | Block::Wire { .. }
        | Block::RedstoneBlock
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Observer { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Trapdoor { .. }
        | Block::Door { .. }
        | Block::FenceGate { .. }
        | Block::Rail { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::Hopper { .. }
        | Block::Slab { .. }
        | Block::Leaves { .. }
        | Block::Scaffolding { .. }
        | Block::Log { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. }
        | Block::Cake { .. }
        | Block::Composter { .. }
        | Block::Piston { .. }
        | Block::PistonHead { .. }
        | Block::MovingPiston { .. }
        | Block::Falling { .. }
        | Block::Plate { .. }
        | Block::Tripwire { .. }
        | Block::TripwireHook { .. }
        | Block::WallBanner { .. }
        | Block::StoneSlab { .. }
        | Block::AnySlab { .. }
        | Block::PlainRail { .. }
        | Block::DetectorRail { .. }
        | Block::BigDripleaf { .. }
        | Block::ChiseledBookshelf { .. }
        | Block::DaylightDetector { .. }
        | Block::PowderSnow
        | Block::Jukebox { .. }
        | Block::Water { .. }
        | Block::BubbleColumn { .. }
        | Block::Tnt { .. }
        | Block::CoralFan { .. }
        | Block::CoralWallFan { .. }
        | Block::ShulkerBox { .. }
        | Block::Chest { .. }
        | Block::Fence { .. }
        | Block::Pane { .. }
        | Block::SugarCane { .. }
        | Block::WaterloggedGrate
        | Block::Bamboo { .. }
        | Block::BambooSapling
        | Block::Kelp { .. }
        | Block::KelpPlant
        | Block::Farmland { .. }
        | Block::Stem { .. }
        | Block::AttachedStem { .. }
        | Block::Cactus { .. }
        | Block::CactusFlower
        | Block::Bed { .. }
        | Block::Sign { .. }
        | Block::LightningRod { .. } => 0,
    }
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
        // a helmet takes it: hurtAndBreak(nextInt(2) of its own random), gone at its max (no
        // burn); (mob-AI 2026-11-10) an unbreaking helmet's `item_damage` (RemoveBinomial, the
        // level random: a point stays off with chance 2L / (10 + 5(L - 1)), armor's)
        if let Some((max, dmg)) = mob(w, i).helmet {
            let mut add = w.entities.list[i].random.as_mut().map_or(0, |r| r.next_int(2));
            let unbreaking = mob(w, i).armor.ench[0].iter().find(|&&(e, _)| e == "unbreaking").map(|&(_, l)| l);
            if add > 0
                && let Some(l) = unbreaking
                && let Some(r) = w.level_random.as_mut()
            {
                let chance = (2.0f32 + 2.0f32 * (l - 1) as f32) / (10.0f32 + 5.0f32 * (l - 1) as f32);
                if r.next_float() < chance {
                    add -= 1;
                }
            }
            let d = dmg + add;
            mob_mut(w, i).helmet = if d >= max { None } else { Some((max, d)) };
            return;
        }
        // igniteForSeconds(8): igniteForTicks(160) (LivingEntity's: x BURNING_TIME)
        let t160 = super::ignite_ticks(w, i, 160);
        let m = mob_mut(w, i);
        if m.fire_ticks < t160 {
            m.fire_ticks = t160;
        }
        let t = w.time;
        w.weather.events.push(WeatherEvent::Sunburn { t, entity: id });
    }
}

/// `Mob.isSunBurnTick`, in the game's order (tick-exact lane 2026-11-08): the brightness at the
/// eye (`getLightLevelDependentMagicValue`), whether it stands in water, rain or powder snow, then
/// `br > 0.5`, the mob's own random's `nextFloat` (drawn whenever br passes 0.5, whether it then
/// burns or not), and `canSeeSky` last.
fn is_sun_burn_tick(w: &mut World, i: usize) -> bool {
    if !monsters_burn(w.day_time()) {
        return false;
    }
    let e = &w.entities.list[i];
    let eye_y = e.pos.y + mob(w, i).eye() as f64;
    let eye = BlockPos::new(e.pos.x.floor() as i32, eye_y.floor() as i32, e.pos.z.floor() as i32);
    let note = |w: &mut World, why: String| {
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    };
    let raw = match raw_brightness(w, eye) {
        Ok(v) => v,
        Err(e) => {
            note(w, format!("sunburn: the light at the zombie's eye {eye}: {e}"));
            return false;
        }
    };
    let br = magic_value(raw);
    let sheltered = in_water_or_rain(w, i) || in_powder_snow(w, i);
    if br <= 0.5f32 {
        return false;
    }
    let roll = match w.entities.list[i].random.as_mut() {
        Some(r) => r.next_float(),
        None => w.mob.rng_mut().next_float(),
    };
    if roll * 30.0f32 >= (br - 0.4f32) * 2.0f32 || sheltered {
        return false;
    }
    match crate::weather::can_see_sky(w, eye) {
        Ok(v) => v,
        Err(e) => {
            note(w, format!("sunburn: the sky light at the zombie's eye {eye}: {e}"));
            false
        }
    }
}

/// `getMaxLocalRawBrightness` at `p` (max of the sky light less the darkening and the block
/// light) where only 12 and up matter (raw 12 is the first brightness over 0.5): a terrain
/// world's light engine (exact); else the void's sky light (`daylight::sky_light_bounds_from`)
/// and block light (`daylight::block_light_from`) over the cubes such readings come from. Below 12
/// it is only a lower bound.
fn raw_brightness(w: &World, p: BlockPos) -> Result<i32, String> {
    let darken = w.sky_darken();
    if w.land.is_some() {
        return w.raw_brightness(p, darken);
    }
    let (lo, hi) = crate::daylight::sky_light_bounds_from(w, p, (12 + darken).min(15));
    if lo != hi && hi - darken >= 12 {
        return Err(format!("the sky light reads {lo}..{hi} (an unclassified block within its reach)"));
    }
    let block = crate::daylight::block_light_cached(w, p, 12)?;
    Ok((lo - darken).max(block).max(0))
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

    /// (cc767b78, G1 14:55 C1-H3) The table's emission through a real path: a zombie's sunburn
    /// brightness (`raw_brightness` at its eye) in a land-less world under the open sky in a
    /// thunderstorm (the sky's light darkened under the burn's 12): with no light source, under 12;
    /// with a plain torch one step from the eye cell (14, which the old hand list lacked and read as 0),
    /// 13, over 0.5 in the burn's curve; a lit redstone torch there (7) leaves it under 12.
    #[test]
    fn a_torch_lifts_the_sunburn_brightness() {
        let mut w = World::new(BlockPos::new(-6, -2, -6), [13, 8, 13]);
        let stone = Block::parse("stone").unwrap();
        for x in -4..=4 {
            for z in -4..=4 {
                w.set_block(BlockPos::new(x, 0, z), stone, crate::world::flags::ALL);
            }
        }
        w.weather.on = true;
        (w.weather.o_rain_level, w.weather.rain_level, w.weather.o_thunder_level, w.weather.thunder_level) = (1.0, 1.0, 1.0, 1.0);
        let eye = BlockPos::new(0, 2, 0);
        let without = raw_brightness(&w, eye).unwrap();
        assert!(without < 12, "the thunderstorm's sky alone: {without}");
        assert!(magic_value(without) <= 0.5);
        let beside = BlockPos::new(1, 2, 0);
        w.place_raw(&[(beside, Block::parse("redstone_torch[lit=true]").unwrap())]);
        let redstone = raw_brightness(&w, eye).unwrap();
        assert!(redstone < 12, "a lit redstone torch (7) beside it: {redstone}");
        w.place_raw(&[(beside, Block::parse("torch").unwrap())]);
        assert_eq!(raw_brightness(&w, eye), Ok(13), "a torch (14) one step away");
        assert!(magic_value(13) > 0.5, "over the burn's 0.5");
    }

    /// (cc767b78, G1 12:07 / 12:38, ironspec 12:54) Every emitter's light from the game's block table,
    /// a row each: the ones the old hand list lacked (the plain and soul torch, their wall forms, crying
    /// obsidian, the nether portal, jack o'lanterns, froglights) and the ones it had (lava, fire, the
    /// redstone torch lit and out, the lamp lit and out, lanterns, the furnace lit and out, glowstone,
    /// sea lanterns, shroomlights, magma, the copper bulb by oxidation, the respawn anchor by charge).
    #[test]
    fn every_emitter_from_the_block_table() {
        let rows: &[(&str, i32)] = &[
            ("minecraft:torch", 14),
            ("minecraft:wall_torch[facing=north]", 14),
            ("minecraft:soul_torch", 10),
            ("minecraft:soul_wall_torch[facing=north]", 10),
            ("minecraft:crying_obsidian", 10),
            ("minecraft:jack_o_lantern[facing=north]", 15),
            ("minecraft:ochre_froglight[axis=y]", 15),
            ("minecraft:verdant_froglight[axis=y]", 15),
            ("minecraft:pearlescent_froglight[axis=y]", 15),
            ("minecraft:lava[level=0]", 15),
            ("minecraft:fire[age=0,east=false,north=false,south=false,up=false,west=false]", 15),
            ("minecraft:soul_fire", 10),
            ("minecraft:redstone_torch[lit=true]", 7),
            ("minecraft:redstone_torch[lit=false]", 0),
            ("minecraft:redstone_lamp[lit=true]", 15),
            ("minecraft:redstone_lamp[lit=false]", 0),
            ("minecraft:lantern[hanging=false,waterlogged=false]", 15),
            ("minecraft:soul_lantern[hanging=false,waterlogged=false]", 10),
            ("minecraft:furnace[facing=north,lit=true]", 13),
            ("minecraft:furnace[facing=north,lit=false]", 0),
            ("minecraft:glowstone", 15),
            ("minecraft:sea_lantern", 15),
            ("minecraft:shroomlight", 15),
            ("minecraft:magma_block", 3),
            ("minecraft:copper_bulb[lit=true,powered=false]", 15),
            ("minecraft:exposed_copper_bulb[lit=true,powered=false]", 12),
            ("minecraft:weathered_copper_bulb[lit=true,powered=false]", 8),
            ("minecraft:oxidized_copper_bulb[lit=true,powered=false]", 4),
            ("minecraft:copper_bulb[lit=false,powered=false]", 0),
            ("minecraft:respawn_anchor[charges=0]", 0),
            ("minecraft:respawn_anchor[charges=2]", 7),
            ("minecraft:respawn_anchor[charges=4]", 15),
            ("minecraft:end_rod[facing=up]", 14),
            ("minecraft:sea_pickle[pickles=2,waterlogged=true]", 9),
            ("minecraft:sea_pickle[pickles=4,waterlogged=true]", 15),
            ("minecraft:sea_pickle[pickles=2,waterlogged=false]", 0),
            ("minecraft:candle[candles=3,lit=true,waterlogged=false]", 9),
            ("minecraft:candle[candles=3,lit=false,waterlogged=false]", 0),
            ("minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]", 1),
            ("minecraft:end_portal_frame[eye=false,facing=north]", 1),
            ("minecraft:campfire[facing=north,lit=true,signal_fire=false,waterlogged=false]", 15),
            ("minecraft:soul_campfire[facing=north,lit=true,signal_fire=false,waterlogged=false]", 10),
            ("minecraft:campfire[facing=north,lit=false,signal_fire=false,waterlogged=false]", 0),
            ("minecraft:blast_furnace[facing=north,lit=true]", 13),
            ("minecraft:sculk_sensor[power=0,sculk_sensor_phase=active,waterlogged=false]", 1),
            ("minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]", 1),
            ("minecraft:sculk_sensor[power=0,sculk_sensor_phase=cooldown,waterlogged=false]", 1),
            ("minecraft:calibrated_sculk_sensor[facing=north,power=0,sculk_sensor_phase=inactive,waterlogged=false]", 1),
            ("minecraft:stone", 0),
            ("minecraft:air", 0),
        ];
        let mut bad = Vec::new();
        for &(s, want) in rows {
            match Block::parse(s) {
                Ok(b) => {
                    let got = emission(&b);
                    if got != want {
                        bad.push(format!("{s}: {got}, the game's {want}"));
                    }
                    // (where the table holds the state, redsim's own model agrees with it)
                    if let Ok(row) = crate::terrain::light::props_of(&b)
                        && row.emission as i32 != modeled_emission(&b)
                    {
                        bad.push(format!("{s}: the table's {} but the model's {}", row.emission, modeled_emission(&b)));
                    }
                }
                Err(e) => bad.push(format!("{s}: {e}")),
            }
        }
        // (a block redsim doesn't model, as a terrain world holds it: the nether portal, its table row)
        match crate::terrain::blocks::id_of("minecraft:nether_portal[axis=x]").map(Block::Terrain) {
            Some(b) if emission(&b) == 11 => {}
            got => bad.push(format!("minecraft:nether_portal[axis=x] (a terrain cell): {:?}, the game's 11", got.map(|b| emission(&b)))),
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
