//! The overworld's five custom spawners (tickexact, read in 26.3): `PhantomSpawner`,
//! `PatrolSpawner`, `CatSpawner`, `VillageSiege`, `WanderingTraderSpawner`, built in that order by
//! `MinecraftServer` (line 422) and ticked by `ServerLevel.tickCustomSpawners(spawnEnemies)` at
//! the end of `ServerChunkCache.tickChunks` (after the block-ticking chunks) when `doMobSpawning`.
//! Every draw is the game's, in its order: the level random's ([`World::level_random`]), and the
//! trader spawner's own random ([`crate::rng::trader_seed`]); the bridge runs the same five
//! (`Sandbox.create(.., customSpawners)`; its siege seeds its zombie like every construction).
//!
//! Which farms reach each one (nothing is out of the game's scope):
//! * cats: every world with a player and mob spawning on draws every 1200 ticks; a cat spawns
//!   only on ground at the player's height near more than 4 occupied beds (a farm with 5 claimed
//!   beds) or in a swamp hut;
//! * the village siege: monsters on, at the day time 18000 of a night, 1 in 10, when the player
//!   stands in a village (within a section of an occupied bed, bell or claimed job site: an iron
//!   farm's own beds make one): 20 zombies walk in, 2 ticks apart;
//! * phantoms: monsters on and the rule, every 60-119 s at night, the player under the open sky
//!   above the sea level and 72000+ ticks without sleep (an AFK player never sleeps);
//! * patrols: monsters on and the rule, every 12000-13199 ticks by day, 1 in 5, a player more
//!   than 2 sections from any village (an iron farm's beds keep them away from its AFK spot);
//! * the wandering trader: the rule, every 24000 ticks a roll of 25/50/75% then 1 in 10, near a
//!   bell within 48 of a random player or the player.
//!
//! Spawning what is not modeled yet (a phantom, a pillager, a cat, a trader and its llamas) is a
//! model error ([`World::errors`]): the draws up to it are exact, what follows would not be.

use super::types::MobType;
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;
use crate::world::World;

/// The siege's state (`VillageSiege.State`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SiegeState {
    Tonight,
    Done,
}

#[derive(Clone, Debug)]
struct Siege {
    has_setup: bool,
    state: SiegeState,
    zombies_to_spawn: i32,
    next_spawn_time: i32,
    spawn: BlockPos,
}

#[derive(Clone, Debug)]
struct Trader {
    random: LegacyRandom,
    tick_delay: i32,
    /// `WanderingTraderData` (the server's saved data: 24000 and 25 in a new world).
    spawn_delay: i32,
    spawn_chance: i32,
}

/// The custom spawners of a world (`MobWorld::custom`); `None` there: the level runs none.
#[derive(Clone, Debug)]
pub struct Custom {
    /// `ServerChunkCache.spawnEnemies` (natural monsters on): phantoms, patrols and sieges need it.
    pub spawn_enemies: bool,
    /// The game rules `spawn_phantoms`, `spawn_patrols`, `spawn_wandering_traders`.
    pub phantoms: bool,
    pub patrols: bool,
    pub traders: bool,
    /// The chunks loaded at full status (`Level.hasChunk`: `hasChunksAt`), from the bridge.
    pub full_chunks: std::collections::BTreeSet<[i32; 2]>,
    /// The player's `TIME_SINCE_REST` (`ServerPlayer.doTick` adds one a tick it is not asleep,
    /// after the level's tick: the spawners read last tick's value).
    pub time_since_rest: i32,
    /// `Level.getSeaLevel()` (the phantoms' height test): a bridge sandbox's flat source reports
    /// -63, a terrain sample's sandbox its world's 63.
    pub sea_level: i32,
    phantom_next: i32,
    patrol_next: i32,
    cat_next: i32,
    siege: Siege,
    trader: Trader,
    /// What the spawners did (game time, what).
    pub log: Vec<(i64, String)>,
}

impl Custom {
    /// The five spawners as a new level builds them, in a world seeded `seed` (the trader
    /// spawner's own random).
    pub fn new(seed: i64, spawn_enemies: bool, phantoms: bool, patrols: bool, traders: bool) -> Custom {
        Custom {
            spawn_enemies,
            phantoms,
            patrols,
            traders,
            full_chunks: Default::default(),
            time_since_rest: 0,
            sea_level: -63,
            phantom_next: 0,
            patrol_next: 0,
            cat_next: 0,
            siege: Siege { has_setup: false, state: SiegeState::Done, zombies_to_spawn: 0, next_spawn_time: 0, spawn: BlockPos::new(0, 0, 0) },
            trader: Trader { random: LegacyRandom::new(crate::rng::trader_seed(seed)), tick_delay: 1200, spawn_delay: 24000, spawn_chance: 25 },
            log: Vec::new(),
        }
    }
}

fn level(w: &mut World) -> &mut LegacyRandom {
    if w.level_random.is_some() {
        return w.level_random.as_mut().unwrap();
    }
    w.mob.rng_mut()
}

fn model_error(w: &mut World, e: String) {
    if !w.errors.contains(&e) {
        w.errors.push(e);
    }
}

/// `ServerLevel.tickCustomSpawners(spawnEnemies)`: the five in order.
pub fn tick(w: &mut World) {
    let Some(mut c) = w.mob.custom.take() else { return };
    phantoms(w, &mut c);
    patrols(w, &mut c);
    cats(w, &mut c);
    siege(w, &mut c);
    trader(w, &mut c);
    w.mob.custom = Some(c);
}

/// The player's `doTick` after the level's tick: its `TIME_SINCE_REST` (an AFK player never
/// sleeps).
pub fn player_tick(w: &mut World) {
    if w.afk.is_some()
        && let Some(c) = w.mob.custom.as_mut()
    {
        c.time_since_rest = c.time_since_rest.saturating_add(1);
    }
}

fn player_block(w: &World) -> Option<BlockPos> {
    w.afk.map(|p| BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32))
}

/// `ServerLevel.getRandomPlayer()`: the one living player, drawn (`nextInt(1)`).
fn random_player(w: &mut World) -> Option<BlockPos> {
    let p = player_block(w)?;
    level(w).next_int(1);
    Some(p)
}

/// `Level.isBrightOutside()`.
fn bright_outside(w: &World) -> bool {
    w.sky_darken() < 4
}

/// `Level.hasChunksAt(x0, z0, x1, z1)`.
fn has_chunks_at(c: &Custom, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
    for cx in x0.div_euclid(16)..=x1.div_euclid(16) {
        for cz in z0.div_euclid(16)..=z1.div_euclid(16) {
            if !c.full_chunks.contains(&[cx, cz]) {
                return false;
            }
        }
    }
    true
}

/// The village POIs the game holds as occupied: the beds the villagers claimed (HOME), their
/// meeting points and job sites (the bridge loads no worldgen villagers, so a sample's own beds
/// and bells are free there and count for nothing).
fn occupied_village_pois(w: &World) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for e in &w.entities.list {
        if let EntityKind::Mob(m) = &e.kind
            && let Some(b) = m.brain.as_deref()
            && let Some(h) = b.home
        {
            out.push(h);
        }
    }
    out
}

/// `PoiManager.sectionsToVillage`: the 3D Chebyshev distance in sections to the nearest section
/// holding an occupied village POI (7: none within 6).
pub fn sections_to_village(w: &World, p: BlockPos) -> i32 {
    let sec = |q: BlockPos| (q.x.div_euclid(16), q.y.div_euclid(16), q.z.div_euclid(16));
    let (px, py, pz) = sec(p);
    occupied_village_pois(w).into_iter().map(|q| {
        let (x, y, z) = sec(q);
        (x - px).abs().max((y - py).abs()).max((z - pz).abs())
    }).filter(|d| *d <= 6).min().unwrap_or(7)
}

/// `ServerLevel.isCloseToVillage(pos, range)`.
fn close_to_village(w: &World, p: BlockPos, range: i32) -> bool {
    range <= 6 && sections_to_village(w, p) <= range
}

/// `DifficultyInstance(level difficulty, overworld clock, the chunk's inhabited time, moon
/// brightness).getEffectiveDifficulty()` (`ServerLevel.getCurrentDifficultyAt`): Hard; the
/// chunk's inhabited time = the ticks the natural spawner has run (every chunk around the AFK
/// player is a spawning chunk from the start).
pub fn effective_difficulty(w: &World, _p: BlockPos) -> f32 {
    let total = w.day_time() as f32;
    let inhabited = w.mob.natural.as_ref().map_or(0, |n| n.ticks) as f32;
    const MOON: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
    let moon = MOON[w.day_time().div_euclid(24000).rem_euclid(8) as usize];
    let mut scale = 0.75f32;
    let global = ((total + -72000.0) / 1440000.0).clamp(0.0, 1.0) * 0.25;
    scale += global;
    let mut local = 0.0f32;
    local += (inhabited / 3600000.0).clamp(0.0, 1.0) * 1.0;
    local += (moon * 0.25).clamp(0.0, global);
    scale += local;
    3.0 * scale
}

/// `DifficultyInstance.getSpecialMultiplier()`.
pub fn special_multiplier(effective: f32) -> f32 {
    if effective < 2.0 {
        0.0
    } else if effective > 4.0 {
        1.0
    } else {
        (effective - 2.0) / 2.0
    }
}

// ---------------------------------------------------------------------------- the spawners

/// `PhantomSpawner.tick`.
fn phantoms(w: &mut World, c: &mut Custom) {
    if !c.spawn_enemies || !c.phantoms {
        return;
    }
    c.phantom_next -= 1;
    if c.phantom_next > 0 {
        return;
    }
    let d = level(w).next_int(60);
    c.phantom_next += (60 + d) * 20;
    if w.sky_darken() < 5 {
        return;
    }
    let Some(p) = player_block(w) else { return };
    let sky = crate::weather::can_see_sky(w, p).unwrap_or(true);
    if p.y < c.sea_level || !sky {
        return;
    }
    let eff = effective_difficulty(w, p);
    let f = level(w).next_float() * 3.0;
    if eff <= f {
        return;
    }
    let value = c.time_since_rest.max(1);
    if level(w).next_int(value) < 72000 {
        return;
    }
    let (up, east, south) = {
        let r = level(w);
        (20 + r.next_int(15), -10 + r.next_int(21), -10 + r.next_int(21))
    };
    let at = BlockPos::new(p.x + east, p.y + up, p.z + south);
    if !super::natural::valid_empty(w, at, "minecraft:phantom") {
        return;
    }
    let size = 1 + level(w).next_int(3 + 1);
    c.log.push((w.time, format!("phantoms: {size} at {at}")));
    model_error(w, format!("gt {}: {size} phantoms spawn at {at} (phantoms are not modeled)", w.time));
}

/// `PatrolSpawner.tick`.
fn patrols(w: &mut World, c: &mut Custom) {
    if !c.spawn_enemies || !c.patrols {
        return;
    }
    c.patrol_next -= 1;
    if c.patrol_next > 0 {
        return;
    }
    let d = level(w).next_int(1200);
    c.patrol_next += 12000 + d;
    if !bright_outside(w) {
        return;
    }
    if level(w).next_int(5) != 0 {
        return;
    }
    let Some(p) = player_block(w) else { return };
    level(w).next_int(1);
    if close_to_village(w, p, 2) {
        return;
    }
    let (x, z) = {
        let r = level(w);
        let x = (24 + r.next_int(24)) * if r.next_bool() { -1 } else { 1 };
        let z = (24 + r.next_int(24)) * if r.next_bool() { -1 } else { 1 };
        (x, z)
    };
    let at = BlockPos::new(p.x + x, p.y, p.z + z);
    if !has_chunks_at(c, at.x - 10, at.z - 10, at.x + 10, at.z + 10) {
        return;
    }
    c.log.push((w.time, format!("patrol at {at}")));
    model_error(w, format!("gt {}: a pillager patrol tries to spawn at {at} (patrols are not modeled past this point)", w.time));
}

/// `CatSpawner.tick`.
fn cats(w: &mut World, c: &mut Custom) {
    c.cat_next -= 1;
    if c.cat_next > 0 {
        return;
    }
    c.cat_next = 1200;
    let Some(p) = random_player(w) else { return };
    let (x, z) = {
        let r = level(w);
        let x = (8 + r.next_int(24)) * if r.next_bool() { -1 } else { 1 };
        let z = (8 + r.next_int(24)) * if r.next_bool() { -1 } else { 1 };
        (x, z)
    };
    let at = BlockPos::new(p.x + x, p.y, p.z + z);
    if !has_chunks_at(c, at.x - 10, at.z - 10, at.x + 10, at.z + 10) {
        return;
    }
    // SpawnPlacements.isSpawnPositionOk(CAT): ON_GROUND
    if !(super::natural::valid_spawn_floor(w, at.below(), "minecraft:cat") && super::natural::valid_empty(w, at, "minecraft:cat") && super::natural::valid_empty(w, at.above(), "minecraft:cat")) {
        return;
    }
    if close_to_village(w, at, 2) {
        // spawnInVillage: more than 4 occupied HOMEs within 48, fewer than 5 cats near
        let homes = occupied_village_pois(w).iter().filter(|h| {
            let (dx, dy, dz) = ((h.x - at.x) as f64, (h.y - at.y) as f64, (h.z - at.z) as f64);
            dx * dx + dy * dy + dz * dz <= 48.0 * 48.0
        }).count();
        if homes > 4 {
            c.log.push((w.time, format!("cat at {at}")));
            model_error(w, format!("gt {}: a cat spawns at {at} (cats are not modeled)", w.time));
        }
    }
    // a swamp hut (StructureTags.CATS_SPAWN_IN): none in the standard samples
}

/// `VillageSiege.tick` (the bridge's `ExactVillageSiege`).
fn siege(w: &mut World, c: &mut Custom) {
    if bright_outside(w) || !c.spawn_enemies {
        c.siege.state = SiegeState::Done;
        c.siege.has_setup = false;
        return;
    }
    if w.day_time().rem_euclid(24000) == 18000 {
        c.siege.state = if level(w).next_int(10) == 0 { SiegeState::Tonight } else { SiegeState::Done };
    }
    if c.siege.state == SiegeState::Done {
        return;
    }
    if !c.siege.has_setup {
        if !setup_siege(w, c) {
            return;
        }
        c.siege.has_setup = true;
    }
    if c.siege.next_spawn_time > 0 {
        c.siege.next_spawn_time -= 1;
    } else {
        c.siege.next_spawn_time = 2;
        if c.siege.zombies_to_spawn > 0 {
            siege_spawn(w, c);
            c.siege.zombies_to_spawn -= 1;
        } else {
            c.siege.state = SiegeState::Done;
        }
    }
}

/// `VillageSiege.tryToSetupSiege`.
fn setup_siege(w: &mut World, c: &mut Custom) -> bool {
    let Some(center) = player_block(w) else { return false };
    if !close_to_village(w, center, 1) {
        return false;
    }
    for _ in 0..10 {
        let angle = level(w).next_float() * (std::f64::consts::PI * 2.0) as f32;
        let sx = center.x + (crate::entity::mth::cos(angle as f64) * 32.0).floor() as i32;
        let sz = center.z + (crate::entity::mth::sin(angle as f64) * 32.0).floor() as i32;
        c.siege.spawn = BlockPos::new(sx, center.y, sz);
        if siege_spot(w, c.siege.spawn).is_some() {
            c.siege.next_spawn_time = 0;
            c.siege.zombies_to_spawn = 20;
            break;
        }
    }
    c.log.push((w.time, format!("siege set up at {} ({} zombies)", c.siege.spawn, c.siege.zombies_to_spawn)));
    true
}

/// `VillageSiege.findRandomSpawnPos`.
fn siege_spot(w: &mut World, pos: BlockPos) -> Option<Vec3> {
    for _ in 0..10 {
        let (x, z) = {
            let r = level(w);
            (pos.x + r.next_int(16) - 8, pos.z + r.next_int(16) - 8)
        };
        let y = w.height(crate::terrain::Heightmap::WorldSurface, x, z);
        let at = BlockPos::new(x, y, z);
        // Monster.checkMonsterSpawnRules(ZOMBIE, level, EVENT, pos, random): not Peaceful, dark
        // enough (EVENT does not skip the light test), a valid spawn floor
        if close_to_village(w, at, 1) && super::natural::dark_enough(w, at) && super::natural::valid_spawn_floor(w, at.below(), "minecraft:zombie") {
            return Some(Vec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5));
        }
    }
    None
}

/// `VillageSiege.trySpawn`: a zombie (`new Zombie(level)`: a construction), its `finalizeSpawn`
/// (EVENT), its snap yaw from the level random.
fn siege_spawn(w: &mut World, c: &mut Custom) {
    let Some(at) = siege_spot(w, c.siege.spawn) else { return };
    let mut own = w.construct();
    let origin_loaded = c.full_chunks.contains(&[0, 0]);
    let bonus = match finalize_siege_zombie(w, &mut own, origin_loaded) {
        Ok(b) => b,
        Err(e) => {
            model_error(w, format!("gt {}: the siege zombie's finalizeSpawn: {e}", w.time));
            0.0
        }
    };
    let yaw = level(w).next_float() * 360.0;
    let id = enter_spawned(w, MobType::Zombie, at, yaw, bonus);
    c.log.push((w.time, format!("siege zombie #{id} at {:.1} {:.1} {:.1}", at.x, at.y, at.z)));
}

/// `Zombie.finalizeSpawn(level, getCurrentDifficultyAt(origin), EVENT, null)` of a siege zombie:
/// `new Zombie(level)` stands at the origin when its difficulty is read (no chunk there in a
/// sandbox: no inhabited time, no moon). The level random's draws in order (Mob's follow-range
/// triangle and left-handedness, the loot pickup, the baby odds, the doors, the armor and weapon
/// rolls), then `handleAttributes` from its own random (`own`: the construction seed). Returns the
/// follow-range bonus. A path redsim does not model yet (a baby, armor, a weapon, a leader's
/// bonuses, the doubled follow range) is an `Err` after its draws.
fn finalize_siege_zombie(w: &mut World, own: &mut LegacyRandom, origin_loaded: bool) -> Result<f64, String> {
    let special = if origin_loaded {
        special_multiplier(effective_difficulty(w, BlockPos::new(0, 0, 0)))
    } else {
        let total = w.day_time() as f32;
        let global = ((total + -72000.0) / 1440000.0).clamp(0.0, 1.0) * 0.25;
        special_multiplier(3.0 * (0.75 + global))
    };
    let r = level(w);
    let bonus = r.triangle(0.0, 0.11485000000000001);
    r.next_float();
    // canPickUpLoot
    r.next_float();
    // ZombieGroupData(getSpawnAsBabyOdds(random), true)
    if r.next_float() < 0.05 {
        return Err("a baby zombie (not modeled)".into());
    }
    // setCanBreakDoors
    r.next_float();
    // Mob.populateDefaultEquipmentSlots, then Zombie's weapon roll
    if r.next_float() < 0.15 * special {
        return Err("an armored zombie (not modeled)".into());
    }
    if r.next_float() < 0.05 {
        return Err("a zombie with a weapon (not modeled)".into());
    }
    // populateDefaultEquipmentEnchantments: no item, no draw; Halloween (the real date): not now
    // handleAttributes(special, EVENT) from its own random
    own.next_double();
    own.next_double();
    let follow = own.next_double() * 1.5 * special as f64;
    if follow > 1.0 {
        return Err("a zombie's spawn follow-range bonus (not modeled)".into());
    }
    if own.next_float() < special * 0.05 {
        own.next_double();
        own.next_double();
        return Err("a leader zombie (not modeled)".into());
    }
    Ok(bonus)
}

/// A mob a custom spawner made enters the level (`addFreshEntityWithPassengers`): its own random
/// seeded as its entity number, the constructor's UUID and yaw replayed (the head keeps that yaw:
/// the snap set only the body's).
fn enter_spawned(w: &mut World, ty: MobType, pos: Vec3, yaw: f32, bonus: f64) -> u32 {
    let st = super::MobState::new(ty, bonus);
    let n = w.entities.next_id;
    let mut own = w.entering_random(n);
    crate::rng::replay_uuid(&mut own);
    own.next_float();
    let id = w.entities.add(EntityKind::Mob(Box::new(st)), pos, yaw);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.random = Some(own);
    }
    id
}

/// `WanderingTraderSpawner.tick`: its own random; the player from the level's.
fn trader(w: &mut World, c: &mut Custom) {
    if !c.traders {
        return;
    }
    c.trader.tick_delay -= 1;
    if c.trader.tick_delay > 0 {
        return;
    }
    c.trader.tick_delay = 1200;
    c.trader.spawn_delay -= 1200;
    if c.trader.spawn_delay > 0 {
        return;
    }
    c.trader.spawn_delay = 24000;
    let chance = c.trader.spawn_chance;
    c.trader.spawn_chance = (chance + 25).clamp(25, 75);
    if c.trader.random.next_int(100) > chance {
        return;
    }
    // spawn(): getRandomPlayer (the level random), then 1 in 10
    if random_player(w).is_none() {
        c.trader.spawn_chance = 25;
        return;
    }
    if c.trader.random.next_int(10) != 0 {
        return;
    }
    c.log.push((w.time, "a wandering trader tries to spawn".into()));
    model_error(w, format!("gt {}: a wandering trader tries to spawn (traders are not modeled past this point)", w.time));
}
