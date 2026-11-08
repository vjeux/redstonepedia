//! Fire (mobinfra, 2026-11-08): `FireBlock` and `SoulFireBlock` as redsim blocks
//! (`Block::Fire`), and lava's fire (`LavaFluid.randomTick`): one model for every fire, a bolt's
//! (`LightningBolt.spawnFire`), lava's and the fire's own spread (ironfarm and the parent: "one
//! fire model"). Read in the 26.3 source.
//!
//! * Every draw is the level random's (`level.getRandom()`, the scheduled tick's and the random
//!   tick's `random`): the mirrored one when the world has it, else the weather's stand-in.
//! * Fire spreads, burns, and lava lights it only within `fire_spread_radius_around_player` (128)
//!   of a player (`ServerLevel.canSpreadFireAround`): the world's AFK player (`World::afk`;
//!   `None`, as in the bridge's sandboxes: no player, no fire's tick work, no lava fire).
//! * The game's quirks kept: lava ticks twice per random pick (`LiquidBlock.randomTick`, then the
//!   fluid state's); lava's level-ground fire takes the state of the flammable block's cell and is
//!   set above it; a fire that cannot survive is removed and its tick goes on with its old state
//!   (its age may set it back); every `checkBurnOut` draws whatever the neighbour.
//! * Not modeled (model errors): a fire beside obsidian (a nether portal frame), a burned TNT
//!   (primed).

use super::lightning::flammable;
use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::rng::LegacyRandom;
use crate::ticks::Priority;
use crate::world::{World, flags};

/// `fire_spread_radius_around_player` (the gamerule's default).
pub const SPREAD_RADIUS: f64 = 128.0;

/// The level random (mirrored) or the weather's stand-in.
fn rng(w: &mut World) -> &mut LegacyRandom {
    match &mut w.level_random {
        Some(r) => r,
        None => &mut w.weather.rng,
    }
}

fn model_error(w: &mut World, why: String) {
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

/// `FireBlock.getIgniteOdds(state)` (`setFlammable`; a waterlogged block 0).
pub fn ignite_odds(b: &Block) -> i32 {
    flammable(b).map_or(0, |o| o.0)
}

/// `FireBlock.getBurnOdds(state)`.
pub fn burn_odds(b: &Block) -> i32 {
    flammable(b).map_or(0, |o| o.1)
}

/// `FireBlock.canBurn`: ignite odds over 0.
pub fn can_burn(b: &Block) -> bool {
    ignite_odds(b) > 0
}

/// `BlockBehaviour.Properties.ignitedByLava` among redsim's blocks (Blocks.java, 26.3): planks,
/// logs, leaves, wool, beds, signs, wooden fences, fence gates, trapdoors, doors and pressure
/// plates, chests, note blocks, jukeboxes, composters, bamboo and its shoot, bookshelves, crafting
/// tables, TNT, daylight detectors, banners, cactus flowers. Not: wooden buttons, scaffolding,
/// targets, dried kelp and coal blocks (which fire burns all the same).
pub fn ignited_by_lava(b: &Block) -> bool {
    match *b {
        Block::Simple(s) => matches!(
            s,
            Simple::OakPlanks
                | Simple::SprucePlanks
                | Simple::Bookshelf
                | Simple::CraftingTable
                | Simple::WhiteWool
                | Simple::OrangeWool
                | Simple::BlackWool
                | Simple::MagentaWool
                | Simple::LightBlueWool
                | Simple::YellowWool
                | Simple::LimeWool
                | Simple::PinkWool
                | Simple::GrayWool
                | Simple::LightGrayWool
                | Simple::CyanWool
                | Simple::PurpleWool
                | Simple::BlueWool
                | Simple::BrownWool
                | Simple::GreenWool
                | Simple::RedWool
        ),
        Block::Log { .. }
        | Block::Leaves { .. }
        | Block::Bed { .. }
        | Block::Sign { .. }
        | Block::FenceGate { .. }
        | Block::Chest { .. }
        | Block::Note { .. }
        | Block::Jukebox { .. }
        | Block::Composter { .. }
        | Block::Bamboo { .. }
        | Block::BambooSapling
        | Block::ChiseledBookshelf { .. }
        | Block::Tnt { .. }
        | Block::DaylightDetector { .. }
        | Block::WallBanner { .. }
        | Block::CactusFlower => true,
        Block::Fence { kind, .. } => crate::fence::wooden(kind),
        Block::Trapdoor { iron, .. } | Block::Door { iron, .. } => !iron,
        Block::Plate { kind, .. } => kind == crate::react::plate::OAK,
        _ => false,
    }
}

/// `#minecraft:soul_fire_base_blocks`: soul sand, soul soil.
pub fn soul_base(b: &Block) -> bool {
    matches!(b, Block::Simple(Simple::SoulSand | Simple::SoulSoil))
}

/// `#minecraft:infiniburn_overworld`: netherrack, magma blocks (redsim has magma).
pub fn infiniburn(b: &Block) -> bool {
    matches!(b, Block::Simple(Simple::MagmaBlock))
}

/// `ServerLevel.canSpreadFireAround(pos)`: a non-spectator player within 128 of the block's corner
/// (`ChunkMap.anyPlayerCloseEnoughTo`: `player.position().distanceTo(new Vec3(pos)) < 128`).
pub fn can_spread_fire_around(w: &World, p: BlockPos) -> bool {
    match w.afk {
        Some(a) => {
            let (dx, dy, dz) = (a.x - p.x as f64, a.y - p.y as f64, a.z - p.z as f64);
            (dx * dx + dy * dy + dz * dz).sqrt() < SPREAD_RADIUS
        }
        None => false,
    }
}

/// `isFaceSturdy(UP)` of the block at `p` (the full support).
fn sturdy_up(w: &World, p: BlockPos) -> bool {
    let b = w.get(p);
    crate::behavior::sturdy(w, &b, p, Dir::Up, crate::behavior::Support::Full)
}

/// `FireBlock.isValidFireLocation`: a block it can burn on one of its six sides.
pub fn is_valid_fire_location(w: &World, p: BlockPos) -> bool {
    Dir::ALL.iter().any(|d| can_burn(&w.get(p.rel(*d))))
}

/// The sides a fire at `p` burns toward (`FireBlock.getStateForPlacement`): none over a floor it
/// can burn or stand on, else north, east, south, west, up where it can burn (bits 0..4).
fn fire_sides(w: &World, p: BlockPos) -> u8 {
    let below = p.below();
    if can_burn(&w.get(below)) || sturdy_up(w, below) {
        return 0;
    }
    let mut sides = 0u8;
    for (k, d) in [Dir::North, Dir::East, Dir::South, Dir::West, Dir::Up].into_iter().enumerate() {
        if can_burn(&w.get(p.rel(d))) {
            sides |= 1 << k;
        }
    }
    sides
}

/// `BaseFireBlock.getState(level, pos)`: soul fire over a soul fire base, else fire (age 0).
pub fn state_at(w: &World, p: BlockPos) -> Block {
    if soul_base(&w.get(p.below())) { Block::Fire { age: 0, sides: 0, soul: true } } else { Block::Fire { age: 0, sides: fire_sides(w, p), soul: false } }
}

/// `FireBlock.getStateWithAge(level, pos, age)`: [`state_at`] with `age` when it is fire.
fn state_with_age(w: &World, p: BlockPos, age: i32) -> Block {
    match state_at(w, p) {
        Block::Fire { soul: false, sides, .. } => Block::Fire { age: age as u8, sides, soul: false },
        s => s,
    }
}

/// `canSurvive`: soul fire on a soul fire base; fire on a sturdy floor or beside a block it burns.
pub fn can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
    match *s {
        Block::Fire { soul: true, .. } => soul_base(&w.get(p.below())),
        _ => sturdy_up(w, p.below()) || is_valid_fire_location(w, p),
    }
}

/// `updateShape`: air when it cannot survive; else soul fire as it is, fire with its sides (or
/// soul fire, over a new soul fire base) and its age.
pub fn update_shape(w: &World, s: Block, p: BlockPos) -> Block {
    match s {
        Block::Fire { soul: true, .. } => {
            if can_survive(w, &s, p) {
                s
            } else {
                Block::Air
            }
        }
        Block::Fire { age, .. } => {
            if can_survive(w, &s, p) {
                state_with_age(w, p, age as i32)
            } else {
                Block::Air
            }
        }
        _ => s,
    }
}

/// `BaseFireBlock.onPlace` (a new fire: a portal frame around it is a model error; one that
/// cannot survive goes) and `FireBlock.onPlace` (its first tick in 30..39 gt, drawn even when it
/// went).
pub fn on_place(w: &mut World, s: Block, p: BlockPos, old: Block) {
    let Block::Fire { soul, .. } = s else { return };
    let same = matches!(old, Block::Fire { soul: o, .. } if o == soul);
    if !same {
        if Dir::ALL.iter().any(|d| matches!(w.get(p.rel(*d)), Block::Simple(Simple::Obsidian | Simple::CryingObsidian))) {
            model_error(w, format!("a fire at {p} beside obsidian: a nether portal's frame is not modeled"));
        }
        if !can_survive(w, &s, p) {
            w.remove_block(p, false);
        }
    }
    if !soul {
        let delay = 30 + rng(w).next_int(10);
        w.schedule_tick(p, Kind::Fire(false), delay, Priority::Normal);
    }
}

/// `FireBlock.isNearRain`: rain at the cell or at one of its four sides.
fn is_near_rain(w: &mut World, p: BlockPos) -> bool {
    w.is_raining_at(p) || w.is_raining_at(p.rel(Dir::West)) || w.is_raining_at(p.rel(Dir::East)) || w.is_raining_at(p.rel(Dir::North)) || w.is_raining_at(p.rel(Dir::South))
}

/// `FireBlock.getIgniteOdds(level, pos)`: an air cell's highest ignite odds among its six sides.
fn ignite_odds_at(w: &World, p: BlockPos) -> i32 {
    if !w.get(p).is_air() {
        return 0;
    }
    Dir::ALL.iter().map(|d| ignite_odds(&w.get(p.rel(*d)))).max().unwrap_or(0)
}

/// The game's difficulty id (`Difficulty.getId`: Hard 3).
fn difficulty_id(w: &World) -> i32 {
    match w.weather.lightning.difficulty {
        super::lightning::Difficulty::Peaceful => 0,
        super::lightning::Difficulty::Easy => 1,
        super::lightning::Difficulty::Normal => 2,
        super::lightning::Difficulty::Hard => 3,
    }
}

/// The event of a block that burned away (the judge's record).
fn burned(w: &mut World, p: BlockPos, b: &Block, into_fire: bool) {
    let t = w.time;
    w.weather.events.push(super::WeatherEvent::Burned { t, at: [p.x, p.y, p.z], what: b.to_string(), into_fire });
}

/// `FireBlock.checkBurnOut(level, pos, chance, random, age)`.
fn check_burn_out(w: &mut World, p: BlockPos, chance: i32, age: i32) {
    let old = w.get(p);
    let odds = burn_odds(&old);
    if rng(w).next_int(chance) < odds {
        let to_fire = rng(w).next_int(age + 10) < 5 && !w.is_raining_at(p);
        if to_fire {
            let new_age = (age + rng(w).next_int(5) / 4).min(15);
            let f = state_with_age(w, p, new_age);
            w.set_block(p, f, flags::ALL);
        } else {
            w.remove_block(p, false);
        }
        burned(w, p, &old, to_fire);
        if matches!(old, Block::Tnt { .. }) {
            model_error(w, format!("a fire burned the TNT at {p}: TntBlock.prime is not modeled here"));
        }
        // a block holding lava back (gone, or fire, which lava washes away): the lava flows now
        // (flowing lava is not modeled; the run has failed on the burn already)
        if let Some(q) = Dir::ALL.iter().map(|d| p.rel(*d)).find(|q| matches!(w.get(*q), Block::Lava { .. })) {
            model_error(w, format!("a fire burned the {} at {p} beside the lava at {q}: the lava flows (flowing lava is not modeled)", old.to_string().split('[').next().unwrap_or("block").trim_start_matches("minecraft:")));
        }
    }
}

/// `FireBlock.tick`: the next tick, then (a player near) survival, rain, age, burn-out, spread.
pub fn tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::Fire { age, soul: false, .. } = s else { return };
    let delay = 30 + rng(w).next_int(10);
    w.schedule_tick(p, Kind::Fire(false), delay, Priority::Normal);
    if !can_spread_fire_around(w, p) {
        return;
    }
    if !can_survive(w, &s, p) {
        w.remove_block(p, false);
    }
    let infini = infiniburn(&w.get(p.below()));
    let age = age as i32;
    if !infini && w.is_raining() && is_near_rain(w, p) && rng(w).next_float() < 0.2f32 + age as f32 * 0.03f32 {
        w.remove_block(p, false);
        let t = w.time;
        w.weather.events.push(super::WeatherEvent::FireOut { t, at: [p.x, p.y, p.z] });
        return;
    }
    let new_age = 15.min(age + rng(w).next_int(3) / 2);
    if age != new_age {
        let st = match s {
            Block::Fire { sides, .. } => Block::Fire { age: new_age as u8, sides, soul: false },
            other => other,
        };
        w.set_block(p, st, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
    }
    if !infini {
        if !is_valid_fire_location(w, p) {
            if !sturdy_up(w, p.below()) || age > 3 {
                w.remove_block(p, false);
            }
            return;
        }
        if age == 15 && rng(w).next_int(4) == 0 && !can_burn(&w.get(p.below())) {
            w.remove_block(p, false);
            return;
        }
    }
    // INCREASED_FIRE_BURNOUT: off in plains
    check_burn_out(w, p.rel(Dir::East), 300, age);
    check_burn_out(w, p.rel(Dir::West), 300, age);
    check_burn_out(w, p.below(), 250, age);
    check_burn_out(w, p.above(), 250, age);
    check_burn_out(w, p.rel(Dir::North), 300, age);
    check_burn_out(w, p.rel(Dir::South), 300, age);
    let difficulty = difficulty_id(w);
    for xx in -1..=1 {
        for zz in -1..=1 {
            for yy in -1..=4 {
                if xx == 0 && yy == 0 && zz == 0 {
                    continue;
                }
                let rate = if yy > 1 { 100 + (yy - 1) * 100 } else { 100 };
                let tp = p.offset(xx, yy, zz);
                let ignite = ignite_odds_at(w, tp);
                if ignite > 0 {
                    let odds = (ignite + 40 + difficulty * 7) / (age + 30);
                    if odds > 0 && rng(w).next_int(rate) <= odds && (!w.is_raining() || !is_near_rain(w, tp)) {
                        let spread_age = 15.min(age + rng(w).next_int(5) / 4);
                        let f = state_with_age(w, tp, spread_age);
                        w.set_block(tp, f, flags::ALL);
                        let t = w.time;
                        w.weather.events.push(super::WeatherEvent::Fire { t, at: [tp.x, tp.y, tp.z], flammable_near: None });
                    }
                }
            }
        }
    }
}

/// `place_fire(w, p)` (the API ironfarm's lava side was written against, kept for any caller):
/// [`state_at`] set with `setBlockAndUpdate` on an air cell where it survives
/// (`LightningBolt.spawnFire`'s test). True when a fire was set.
pub fn place_fire(w: &mut World, p: BlockPos) -> bool {
    let f = state_at(w, p);
    if !w.get(p).is_air() || !can_survive(w, &f, p) {
        return false;
    }
    w.set_block(p, f, flags::ALL);
    let t = w.time;
    w.weather.events.push(super::WeatherEvent::Fire { t, at: [p.x, p.y, p.z], flammable_near: None });
    true
}

/// `LavaFluid.isFlammable`: a loaded block that `ignitedByLava`.
fn lava_flammable(w: &World, p: BlockPos) -> bool {
    ignited_by_lava(&w.get(p))
}

/// `LavaFluid.hasFlammableNeighbours`.
fn has_flammable_neighbours(w: &World, p: BlockPos) -> bool {
    Dir::ALL.iter().any(|d| lava_flammable(w, p.rel(*d)))
}

/// `LavaFluid.randomTick` (a player near): `nextInt(3)` steps up with a random x and z of -1..1
/// each, lighting the first air cell beside an `ignitedByLava` block and stopping at a
/// `#blocks_lava_fire_spread` (`#blocks_motion`) block; with no step, 3 tries beside the lava at
/// its level, lighting the air over an `ignitedByLava` block (with that block's cell's state).
pub fn lava_random_tick(w: &mut World, p: BlockPos) {
    if !can_spread_fire_around(w, p) {
        return;
    }
    let passes = rng(w).next_int(3);
    if passes > 0 {
        let mut tp = p;
        for _ in 0..passes {
            let dx = rng(w).next_int(3) - 1;
            let dz = rng(w).next_int(3) - 1;
            tp = tp.offset(dx, 1, dz);
            let st = w.get(tp);
            if st.is_air() {
                if has_flammable_neighbours(w, tp) {
                    let f = state_at(w, tp);
                    w.set_block(tp, f, flags::ALL);
                    let t = w.time;
                    w.weather.events.push(super::WeatherEvent::Fire { t, at: [tp.x, tp.y, tp.z], flammable_near: Some(format!("lava at {p}")) });
                    return;
                }
            } else if super::blocks_motion(&st) {
                return;
            }
        }
    } else {
        for _ in 0..3 {
            let dx = rng(w).next_int(3) - 1;
            let dz = rng(w).next_int(3) - 1;
            let tp = p.offset(dx, 0, dz);
            if w.get(tp.above()).is_air() && lava_flammable(w, tp) {
                let f = state_at(w, tp);
                w.set_block(tp.above(), f, flags::ALL);
                let t = w.time;
                let q = tp.above();
                w.weather.events.push(super::WeatherEvent::Fire { t, at: [q.x, q.y, q.z], flammable_near: Some(format!("lava at {p}")) });
            }
        }
    }
}

/// The fire cell a box meets (`getEntityInsideCollisionShape`: the full cell; the box deflated by
/// 1e-5 as `checkInsideBlocks` does), if any.
pub fn fire_in(w: &World, bb: crate::entity::Aabb) -> Option<BlockPos> {
    let d = 1.0e-5f32 as f64;
    let b = bb.deflate(d, d, d);
    for y in b.min_y.floor() as i32..=b.max_y.floor() as i32 {
        for z in b.min_z.floor() as i32..=b.max_z.floor() as i32 {
            for x in b.min_x.floor() as i32..=b.max_x.floor() as i32 {
                let p = BlockPos::new(x, y, z);
                if matches!(w.get(p), Block::Fire { .. }) {
                    return Some(p);
                }
            }
        }
    }
    None
}
