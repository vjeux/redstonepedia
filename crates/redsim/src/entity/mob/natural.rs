//! Natural mobs and natural spawning (the terrain lane; brief `designs/briefs/terrain-and-spawning.md`
//! §4-6, read in the 26.3 source):
//!
//! * [`DEFS`]: the natural mob types' rows (`MobType::Natural(i)`), babies and slime sizes as rows of
//!   their own (`getDefaultDimensions` scaled: `getAgeScale` 0.5, `Slime.getSize`);
//! * [`KINDS`]: each spawnable type's `MobCategory`, `SpawnPlacementType`, spawn rule, cluster size,
//!   `removeWhenFarAway`, and what its `finalizeSpawn` adds (babies, jockeys, riders, sizes);
//! * [`tick_chunks`]: `ServerChunkCache.tickChunks`' spawning (`NaturalSpawner.createState`, the
//!   categories, `collectSpawningChunks`, the shuffle, `spawnForChunk`,
//!   `spawnCategoryForPosition`, `isValidSpawnPostitionForType`, `isValidPositionForMob`);
//! * [`check_despawn`]: `Mob.checkDespawn` (called by the entity phase before each mob's tick) and
//!   the `noActionTime` it reads.
//!
//! The game's `level.random` draws come from redsim's seeded stand-in ([`super::MobWorld::rng`]),
//! with the same distributions, as the mob layer's policy says. Natural mobs' walking is not
//! modeled: they stand where they spawn (their physics: gravity, fluids, damage), and the worst-case
//! checks (`hostile.rs`) refuse any farm their walking could change.

use super::types::{MobType, TypeDef};
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::terrain::biome::{self, Spawner};
use crate::terrain::blocks;
use crate::world::World;

// ------------------------------------------------------------------------------ the rows

const fn row(i: u16, id: &'static str, game_id: &'static str, width: f32, height: f32, eye: f32, max_health: f32, speed: f64, monster: bool, burns: bool, step: f32) -> TypeDef {
    TypeDef {
        ty: MobType::Natural(i),
        id,
        game_id,
        width,
        height,
        eye,
        riding_offset: 0.0,
        max_health,
        speed,
        follow_range: 16.0,
        step,
        knockback_resistance: 0.0,
        armor: 0.0,
        fire_immune: false,
        fall_immune: false,
        drowns: true,
        burns_in_daylight: burns,
        monster,
        loot: &[],
    }
}

/// The natural mob types' rows (`DEFS[i].ty == MobType::Natural(i)`): `EntityTypes` sizes and eye
/// heights, each class's `createAttributes` health and speed, the `burn_in_daylight` tag. Zombies
/// and witches are the mob layer's own rows (`MobType::Zombie`, `MobType::Witch`).
pub static DEFS: &[TypeDef] = &[
    // (mob-AI lane) loot: each pool's set_count uniform, in table order; pools behind a
    // killed_by_player or attacker condition draw nothing without that killer. Speeds: the
    // attribute as the code gives it (a float literal widened: the spiders' 0.3F, the zombies'
    // 0.23F; a baby's +50% on the base)
    TypeDef { loot: &[("minecraft:string", 0, 2)], ..row(0, "spider", "minecraft:spider", 1.4, 0.9, 0.65, 16.0, 0.3f32 as f64, true, false, 0.6) },
    TypeDef { loot: &[("minecraft:string", 0, 2)], ..row(1, "cave_spider", "minecraft:cave_spider", 0.7, 0.5, 0.45, 12.0, 0.3f32 as f64, true, false, 0.6) },
    row(2, "zombie_villager", "minecraft:zombie_villager", 0.6, 1.95, 1.74, 20.0, 0.23f32 as f64, true, true, 0.6),
    row(3, "zombie_horse", "minecraft:zombie_horse", 1.3964844, 1.6, 1.52, 25.0, 0.2, true, false, 1.0),
    TypeDef { loot: &[("minecraft:arrow", 0, 2), ("minecraft:bone", 0, 2)], ..row(4, "skeleton", "minecraft:skeleton", 0.6, 1.99, 1.74, 20.0, 0.25, true, true, 0.6) },
    TypeDef { loot: &[("minecraft:gunpowder", 0, 2)], ..row(5, "creeper", "minecraft:creeper", 0.6, 1.7, 1.445, 20.0, 0.25, true, false, 0.6) },
    row(6, "slime", "minecraft:slime", 0.52, 0.52, 0.325, 1.0, 0.2, true, false, 0.6),
    row(7, "enderman", "minecraft:enderman", 0.6, 2.9, 2.55, 40.0, 0.3, true, false, 1.0),
    row(8, "drowned", "minecraft:drowned", 0.6, 1.95, 1.74, 20.0, 0.23f32 as f64, true, true, 1.0),
    row(9, "sulfur_cube", "minecraft:sulfur_cube", 0.49, 0.49, 0.175, 1.0, 0.2, true, false, 0.6),
    row(10, "sheep", "minecraft:sheep", 0.9, 1.3, 1.235, 8.0, 0.23, false, false, 0.6),
    row(11, "pig", "minecraft:pig", 0.9, 0.9, 0.765, 10.0, 0.25, false, false, 0.6),
    row(12, "chicken", "minecraft:chicken", 0.4, 0.7, 0.644, 4.0, 0.25, false, false, 0.6),
    row(13, "cow", "minecraft:cow", 0.9, 1.4, 1.3, 10.0, 0.2, false, false, 0.6),
    row(14, "horse", "minecraft:horse", 1.3964844, 1.6, 1.52, 22.0, 0.225, false, false, 1.0),
    row(15, "donkey", "minecraft:donkey", 1.3964844, 1.5, 1.425, 22.0, 0.175, false, false, 1.0),
    row(16, "wolf", "minecraft:wolf", 0.6, 0.85, 0.68, 8.0, 0.3, false, false, 0.6),
    row(17, "rabbit", "minecraft:rabbit", 0.49, 0.6, 0.59, 3.0, 0.3, false, false, 0.6),
    row(18, "fox", "minecraft:fox", 0.6, 0.7, 0.4, 10.0, 0.3, false, false, 0.6),
    row(19, "turtle", "minecraft:turtle", 1.2, 0.4, 0.34, 30.0, 0.25, false, false, 1.0),
    row(20, "bat", "minecraft:bat", 0.5, 0.9, 0.45, 6.0, 0.1, false, false, 0.6),
    row(21, "glow_squid", "minecraft:glow_squid", 0.8, 0.8, 0.4, 10.0, 0.7, false, false, 0.6),
    row(22, "squid", "minecraft:squid", 0.8, 0.8, 0.4, 10.0, 0.7, false, false, 0.6),
    row(23, "salmon", "minecraft:salmon", 0.7, 0.4, 0.26, 3.0, 0.7, false, false, 0.6),
    row(24, "cod", "minecraft:cod", 0.5, 0.3, 0.195, 3.0, 0.7, false, false, 0.6),
    // babies (`getAgeScale` 0.5; a baby zombie's BABY_DIMENSIONS eye 0.93)
    row(25, "baby_zombie", "minecraft:zombie", 0.3, 0.975, 0.93, 20.0, 0.23f32 as f64 * 1.5, true, true, 0.6),
    row(26, "baby_zombie_villager", "minecraft:zombie_villager", 0.3, 0.975, 0.93, 20.0, 0.23f32 as f64 * 1.5, true, true, 0.6),
    row(27, "baby_drowned", "minecraft:drowned", 0.3, 0.975, 0.93, 20.0, 0.23f32 as f64 * 1.5, true, true, 1.0),
    row(28, "baby_sheep", "minecraft:sheep", 0.45, 0.65, 0.6175, 8.0, 0.23, false, false, 0.6),
    row(29, "baby_pig", "minecraft:pig", 0.45, 0.45, 0.3825, 10.0, 0.25, false, false, 0.6),
    row(30, "baby_chicken", "minecraft:chicken", 0.2, 0.35, 0.322, 4.0, 0.25, false, false, 0.6),
    row(31, "baby_cow", "minecraft:cow", 0.45, 0.7, 0.65, 10.0, 0.2, false, false, 0.6),
    row(32, "baby_horse", "minecraft:horse", 0.6982422, 0.8, 0.76, 22.0, 0.225, false, false, 1.0),
    row(33, "baby_donkey", "minecraft:donkey", 0.6982422, 0.75, 0.7125, 22.0, 0.175, false, false, 1.0),
    row(34, "baby_wolf", "minecraft:wolf", 0.3, 0.425, 0.34, 8.0, 0.3, false, false, 0.6),
    row(35, "baby_rabbit", "minecraft:rabbit", 0.245, 0.3, 0.295, 3.0, 0.3, false, false, 0.6),
    row(36, "baby_fox", "minecraft:fox", 0.3, 0.35, 0.2, 10.0, 0.3, false, false, 0.6),
    row(37, "baby_turtle", "minecraft:turtle", 0.36, 0.12, 0.102, 30.0, 0.25, false, false, 1.0),
    // slimes of size 2 and 4 (`Slime.getDefaultDimensions` = the type's scaled by the size)
    row(38, "slime_2", "minecraft:slime", 1.04, 1.04, 0.65, 4.0, 0.2, true, false, 0.6),
    row(39, "slime_4", "minecraft:slime", 2.08, 2.08, 1.3, 16.0, 0.2, true, false, 0.6),
    row(40, "sulfur_cube_2", "minecraft:sulfur_cube", 0.98, 0.98, 0.35, 4.0, 0.2, true, false, 0.6),
    row(41, "sulfur_cube_4", "minecraft:sulfur_cube", 1.96, 1.96, 0.7, 16.0, 0.2, true, false, 0.6),
    // a village's cats and bees (CREATURE: they count toward the cap; not in these biomes' lists)
    row(42, "cat", "minecraft:cat", 0.6, 0.7, 0.35, 10.0, 0.3, false, false, 0.6),
    row(43, "bee", "minecraft:bee", 0.7, 0.6, 0.3, 10.0, 0.3, false, false, 0.6),
];

// ------------------------------------------------------------------------- the categories

/// `MobCategory` (in its enum order, `MISC` last).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Category {
    Monster,
    Creature,
    Ambient,
    Axolotls,
    UndergroundWaterCreature,
    WaterCreature,
    WaterAmbient,
    Misc,
}

impl Category {
    pub const SPAWNING: [Category; 7] = [Category::Monster, Category::Creature, Category::Ambient, Category::Axolotls, Category::UndergroundWaterCreature, Category::WaterCreature, Category::WaterAmbient];

    /// `getMaxInstancesPerChunk`.
    pub fn max_instances(self) -> i32 {
        match self {
            Category::Monster => 70,
            Category::Creature => 10,
            Category::Ambient => 15,
            Category::Axolotls | Category::UndergroundWaterCreature | Category::WaterCreature => 5,
            Category::WaterAmbient => 20,
            Category::Misc => -1,
        }
    }

    pub fn friendly(self) -> bool {
        self != Category::Monster
    }

    pub fn persistent(self) -> bool {
        matches!(self, Category::Creature | Category::Misc)
    }

    /// `getDespawnDistance`.
    pub fn despawn_distance(self) -> f64 {
        if self == Category::WaterAmbient { 64.0 } else { 128.0 }
    }

    /// The JSON name (`getSerializedName`).
    pub fn name(self) -> &'static str {
        match self {
            Category::Monster => "monster",
            Category::Creature => "creature",
            Category::Ambient => "ambient",
            Category::Axolotls => "axolotls",
            Category::UndergroundWaterCreature => "underground_water_creature",
            Category::WaterCreature => "water_creature",
            Category::WaterAmbient => "water_ambient",
            Category::Misc => "misc",
        }
    }
}

/// `SpawnPlacementTypes`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Placement {
    OnGround,
    InWater,
    NoRestrictions,
}

/// The `SpawnPlacements` rule of a type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// `Monster.checkMonsterSpawnRules`.
    Monster,
    /// `Monster.checkAnyLightMonsterSpawnRules` (and `checkSulfurCubeSpawnRules`: always).
    AnyLight,
    /// `Animal.checkAnimalSpawnRules` (`ANIMALS_SPAWNABLE_ON`, brightness above 8).
    Animal,
    /// `Wolf` / `Rabbit` / `Fox` rules: the tag below and the animal brightness.
    AnimalTag(&'static str),
    /// `Turtle.checkTurtleSpawnRules`.
    Turtle,
    Drowned,
    Slime,
    Bat,
    GlowSquid,
    /// `WaterAnimal.checkSurfaceWaterAnimalSpawnRules`.
    SurfaceWater,
    Always,
}

/// Which `finalizeSpawn` (NATURAL) a type runs: each draws from the level random in the game's
/// order (`finalize_spawn`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Finalize {
    /// `CaveSpider.finalizeSpawn`: returns at once (no draws, not even `Mob`'s).
    Nothing,
    /// `Mob.finalizeSpawn` only: the follow range's triangle (2 nextDouble) and the left hand
    /// (nextFloat).
    Mob,
    /// `Zombie.finalizeSpawn` (zombies, zombie villagers): the loot pick-up, the group's baby, a
    /// baby's chicken jockey, door breaking, armor and weapon, their enchantments.
    Zombie,
    /// `Drowned.finalizeSpawn`: the zombie's with the drowned's own equipment (trident or rod), then
    /// the nautilus shell and the nautilus jockey.
    Drowned,
    /// `AbstractSkeleton.finalizeSpawn`: armor, the bow, their enchantments, the loot pick-up.
    Skeleton,
    /// `Spider.finalizeSpawn`: the skeleton rider one time in 100, the group's effect on Hard.
    Spider,
    /// `Slime.finalizeSpawn`: no babies (`AgeableMobGroupData(false)`); size 1, 2 or 4.
    Slime,
    /// `SulfurCube`: `AgeableMob`'s baby (chance 0.05 after the group's first), size 1 if a baby
    /// else 2.
    SulfurCube,
    /// `Squid.finalizeSpawn` (squids, glow squids): `AgeableMob` with chance 0.05.
    Squid,
    /// `Sheep.finalizeSpawn`: the colour (`SheepColorSpawnRules`), then `AgeableMob`.
    Sheep,
    /// Pigs, cows, chickens: the variant (`VariantUtils.selectVariantToSpawn`: one candidate in a
    /// biome), the sound variant (`sounds` entries in its registry), then `AgeableMob`.
    Farm { sounds: i32 },
    /// `Horse.finalizeSpawn`: the group's variant, the markings, `randomizeAttributes` (health,
    /// speed, jump), `AgeableMob` (0.05).
    Horse,
    /// Donkeys (`AbstractHorse`: `AgeableMobGroupData(0.2)`; `AbstractChestedHorse`'s health).
    Donkey,
    /// `Wolf.finalizeSpawn`: the pack's variant, the sound variant (7), no babies.
    Wolf,
    /// Cod (`AbstractSchoolingFish`): `Mob`'s, then the school (no draws).
    Cod,
    /// `Salmon.finalizeSpawn`: the variant from the salmon's own random, then the cod's.
    Salmon,
    /// `ZombieHorse.finalizeSpawn`: a zombie rider (its own `finalizeSpawn`), then `AbstractHorse`
    /// (0.2) with the zombie horse's jump and speed.
    ZombieHorse,
    /// Not modeled draw for draw: a model error when one spawns.
    Unmodeled,
}

/// A spawnable type.
#[derive(Clone, Copy, Debug)]
pub struct Kind {
    pub id: &'static str,
    pub ty: MobType,
    pub category: Category,
    pub placement: Placement,
    pub rule: Rule,
    /// `getMaxSpawnClusterSize`.
    pub cluster: u32,
    /// `removeWhenFarAway` (false: never despawns: animals).
    pub removes_far: bool,
    /// The baby's row (`None`: no baby).
    pub baby: Option<MobType>,
    pub finalize: Finalize,
    /// Its `checkSpawnObstruction` skips the liquid test (water animals, drowned).
    pub water_obstruction: bool,
    /// `Monster.getWalkTargetValue` (the light) or `Animal`'s (grass) gates `checkSpawnRules`.
    pub walk_target: WalkTarget,
}

/// `PathfinderMob.checkSpawnRules` = `getWalkTargetValue(pos) >= 0`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkTarget {
    /// Not a `PathfinderMob`, or 0 (passes).
    None,
    /// `Monster`: `-getPathfindingCostFromLightLevels`.
    Monster,
    /// `Animal`: grass below: 10, else `getPathfindingCostFromLightLevels`.
    Animal,
}

const fn kind(id: &'static str, ty: MobType, category: Category, placement: Placement, rule: Rule, cluster: u32, removes_far: bool, baby: Option<MobType>, finalize: Finalize, water: bool, walk: WalkTarget) -> Kind {
    Kind { id, ty, category, placement, rule, cluster, removes_far, baby, finalize, water_obstruction: water, walk_target: walk }
}

use Category as C;
use Finalize as F;
use Placement as P;
use WalkTarget as W;

/// The spawnable types (`SpawnPlacements` registrations, `EntityTypes` categories, the classes'
/// overrides).
pub static KINDS: &[Kind] = &[
    kind("minecraft:zombie", MobType::Zombie, C::Monster, P::OnGround, Rule::Monster, 4, true, Some(MobType::Natural(25)), F::Zombie, false, W::Monster),
    kind("minecraft:zombie_villager", MobType::Natural(2), C::Monster, P::OnGround, Rule::Monster, 4, true, Some(MobType::Natural(26)), F::Zombie, false, W::Monster),
    kind("minecraft:drowned", MobType::Natural(8), C::Monster, P::InWater, Rule::Drowned, 4, true, Some(MobType::Natural(27)), F::Drowned, true, W::Monster),
    kind("minecraft:zombie_horse", MobType::Natural(3), C::Monster, P::OnGround, Rule::Monster, 6, true, None, F::ZombieHorse, false, W::Animal),
    kind("minecraft:skeleton", MobType::Natural(4), C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Skeleton, false, W::Monster),
    kind("minecraft:creeper", MobType::Natural(5), C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Mob, false, W::Monster),
    kind("minecraft:spider", MobType::Natural(0), C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Spider, false, W::Monster),
    kind("minecraft:cave_spider", MobType::Natural(1), C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Nothing, false, W::Monster),
    kind("minecraft:enderman", MobType::Natural(7), C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Mob, false, W::Monster),
    kind("minecraft:witch", MobType::Witch, C::Monster, P::OnGround, Rule::Monster, 4, true, None, F::Mob, false, W::Monster),
    kind("minecraft:slime", MobType::Natural(6), C::Monster, P::OnGround, Rule::Slime, 4, true, None, F::Slime, false, W::None),
    kind("minecraft:sulfur_cube", MobType::Natural(9), C::Monster, P::OnGround, Rule::Always, 4, true, None, F::SulfurCube, false, W::None),
    kind("minecraft:sheep", MobType::Natural(10), C::Creature, P::OnGround, Rule::Animal, 4, false, Some(MobType::Natural(28)), F::Sheep, false, W::Animal),
    kind("minecraft:pig", MobType::Natural(11), C::Creature, P::OnGround, Rule::Animal, 4, false, Some(MobType::Natural(29)), F::Farm { sounds: 3 }, false, W::Animal),
    kind("minecraft:chicken", MobType::Natural(12), C::Creature, P::OnGround, Rule::Animal, 4, false, Some(MobType::Natural(30)), F::Farm { sounds: 2 }, false, W::Animal),
    kind("minecraft:cow", MobType::Natural(13), C::Creature, P::OnGround, Rule::Animal, 4, false, Some(MobType::Natural(31)), F::Farm { sounds: 2 }, false, W::Animal),
    kind("minecraft:horse", MobType::Natural(14), C::Creature, P::OnGround, Rule::Animal, 6, false, Some(MobType::Natural(32)), F::Horse, false, W::Animal),
    kind("minecraft:donkey", MobType::Natural(15), C::Creature, P::OnGround, Rule::Animal, 6, false, Some(MobType::Natural(33)), F::Donkey, false, W::Animal),
    kind("minecraft:wolf", MobType::Natural(16), C::Creature, P::OnGround, Rule::AnimalTag("minecraft:wolves_spawnable_on"), 8, false, Some(MobType::Natural(34)), F::Wolf, false, W::Animal),
    kind("minecraft:rabbit", MobType::Natural(17), C::Creature, P::OnGround, Rule::AnimalTag("minecraft:rabbits_spawnable_on"), 4, false, Some(MobType::Natural(35)), F::Unmodeled, false, W::Animal),
    kind("minecraft:fox", MobType::Natural(18), C::Creature, P::NoRestrictions, Rule::AnimalTag("minecraft:foxes_spawnable_on"), 4, false, Some(MobType::Natural(36)), F::Unmodeled, false, W::Animal),
    kind("minecraft:turtle", MobType::Natural(19), C::Creature, P::OnGround, Rule::Turtle, 4, false, Some(MobType::Natural(37)), F::Unmodeled, false, W::Animal),
    kind("minecraft:bat", MobType::Natural(20), C::Ambient, P::OnGround, Rule::Bat, 4, true, None, F::Mob, false, W::None),
    kind("minecraft:glow_squid", MobType::Natural(21), C::UndergroundWaterCreature, P::InWater, Rule::GlowSquid, 4, true, None, F::Squid, true, W::None),
    kind("minecraft:squid", MobType::Natural(22), C::WaterCreature, P::InWater, Rule::SurfaceWater, 4, true, None, F::Squid, true, W::None),
    // AbstractFish.getMaxSpawnClusterSize 8; a salmon's school is 5 (Salmon.getMaxSchoolSize)
    kind("minecraft:salmon", MobType::Natural(23), C::WaterAmbient, P::InWater, Rule::SurfaceWater, 5, true, None, F::Salmon, true, W::None),
    kind("minecraft:cod", MobType::Natural(24), C::WaterAmbient, P::InWater, Rule::SurfaceWater, 8, true, None, F::Cod, true, W::None),
    kind("minecraft:cat", MobType::Natural(42), C::Creature, P::OnGround, Rule::Always, 4, false, None, F::Unmodeled, false, W::Animal),
    kind("minecraft:bee", MobType::Natural(43), C::Creature, P::NoRestrictions, Rule::Always, 4, false, None, F::Unmodeled, false, W::Animal),
];

/// A spawnable type by its game id.
pub fn kind_of(game_id: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.id == game_id)
}

/// The spawn data of a mob already in the world (its row, its baby or size rows included).
pub fn kind_of_type(ty: MobType) -> Option<&'static Kind> {
    let game = ty.def().game_id;
    KINDS.iter().find(|k| k.id == game)
}

/// The mob category of any modeled mob (villagers and golems are MISC).
pub fn category_of(ty: MobType) -> Category {
    kind_of_type(ty).map(|k| k.category).unwrap_or(Category::Misc)
}

// --------------------------------------------------------------------------- the settings

/// The game's difficulty.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

/// One record of the spawner.
#[derive(Clone, Debug, PartialEq)]
pub enum NaturalEvent {
    /// A mob spawned: game time, its game id (babies and sizes: the game's id), its cell, the
    /// entity, and the spawn's position in its cluster (1-based).
    Spawn { t: i64, ty: &'static str, at: BlockPos, entity: u32, baby: bool },
    /// A mob despawned (`checkDespawn`'s discard): game time, entity, its game id, why.
    Despawn { t: i64, entity: u32, ty: &'static str, far: bool },
}

/// The natural spawning state of a world (`MobWorld.natural`).
#[derive(Clone, Debug)]
pub struct Natural {
    /// The players' feet positions (non-spectators: they count for spawning and despawning);
    /// empty: the world's AFK player (`World::afk`) when it has one.
    pub players: Vec<Vec3>,
    pub difficulty: Difficulty,
    /// Run the spawner (false: despawning only).
    pub spawning: bool,
    /// `ServerChunkCache.spawnEnemies` (false: the monster category never spawns, the bridge's
    /// "monsters off" sandboxes).
    pub spawn_enemies: bool,
    /// The level's own biome: a world without terrain's, and a terrain world's outside its
    /// sample's biome cells (the bridge's sandboxes: their flat source's plains; the real world's
    /// land beyond the sample is not modeled).
    pub void_biome: Option<&'static str>,
    /// The level's lowest y (`getMinY`) for a world without terrain (the overworld's -64).
    pub world_min_y: i32,
    /// The level's respawn point for a world without terrain (`isRightDistanceToPlayerAndSpawnPoint`
    /// keeps 24 blocks around it free); terrain worlds read theirs. A bridge sandbox has none (its
    /// dimension is not the respawn dimension).
    pub world_spawn: Option<BlockPos>,
    /// A bridge sandbox's `collectSpawningChunks` list before the shuffle, in the game's order
    /// (empty: redsim computes the spawning chunks).
    pub spawn_order: Vec<[i32; 2]>,
    /// The sandbox's `getNaturalSpawnChunkCount()` (None: redsim's count).
    pub spawn_chunk_count: Option<i32>,
    /// The chunks where `canSpawnEntitiesInChunk` holds (entity-ticking); empty: every spawning
    /// chunk.
    pub entity_ticking: Vec<[i32; 2]>,
    /// Riders a `finalizeSpawn` put on a mob (rider, vehicle): passengers, so persistent.
    pub riders: Vec<(u32, u32)>,
    pub log: Vec<NaturalEvent>,
    /// Ticks the spawner has run (the spawning chunks' inhabited time: `incrementInhabitedTime`).
    pub ticks: u64,
}

impl Natural {
    pub fn new(players: Vec<Vec3>, difficulty: Difficulty) -> Natural {
        Natural { players, difficulty, spawning: true, spawn_enemies: true, void_biome: None, world_min_y: -64, world_spawn: None, spawn_order: Vec::new(), spawn_chunk_count: None, entity_ticking: Vec::new(), riders: Vec::new(), log: Vec::new(), ticks: 0 }
    }
}

/// The random the spawner draws from: the mirrored level random (`level.random`, the bridge's)
/// when the world has one, else the mob layer's stand-in.
fn rng(w: &mut World) -> &mut crate::rng::LegacyRandom {
    if w.level_random.is_some() {
        return w.level_random.as_mut().unwrap();
    }
    super::MobWorld::rng_mut(&mut w.mob)
}

/// The players natural spawning sees: the declared ones, else the world's AFK player.
fn players_of(w: &World, nat: &Natural) -> Vec<Vec3> {
    if !nat.players.is_empty() {
        return nat.players.clone();
    }
    w.afk.iter().copied().collect()
}

// ----------------------------------------------------------------------------- the light

/// `Level.getSkyDarken()`: the level's day time (`World::day_time`, whatever drives the mob layer)
/// with this level's rain and thunder.
pub fn darken(w: &World) -> i32 {
    let day = w.day_time();
    if w.weather.on {
        crate::weather::sky_darken_at(day, w.weather.rain(1.0), w.weather.thunder(1.0))
    } else {
        crate::daylight::sky_darken(day)
    }
}

/// `getMaxLocalRawBrightness(pos)` with the level's darkening (or `skyDarken` given).
fn brightness(w: &mut World, p: BlockPos, sky_darken: i32) -> i32 {
    match w.raw_brightness(p, sky_darken) {
        Ok(v) => v,
        Err(e) => {
            if !w.errors.contains(&e) {
                w.errors.push(e);
            }
            15
        }
    }
}

/// `getLightLevelDependentMagicValue(pos)` (ambient light 0 in the overworld).
fn magic(w: &mut World, p: BlockPos) -> f32 {
    let d = darken(w);
    let v = brightness(w, p, d) as f32 / 15.0;
    v / (4.0 - 3.0 * v)
}

// ------------------------------------------------------------------------------ the rules

fn state_props(w: &World, p: BlockPos) -> Option<&'static blocks::Props> {
    let b = w.get(p);
    match b {
        crate::block::Block::Terrain(id) => Some(blocks::props(id)),
        crate::block::Block::Air => blocks::id_of("minecraft:air").map(blocks::props),
        _ => blocks::id_of(&b.to_string()).map(blocks::props),
    }
}

/// `BlockState.isValidSpawn(level, below, type)`.
pub(crate) fn valid_spawn_floor(w: &World, below: BlockPos, type_id: &str) -> bool {
    let col = blocks::type_index(type_id);
    match (state_props(w, below), col) {
        (Some(p), Some(c)) => p.valid_spawn.get(c).copied().unwrap_or(false),
        _ => false,
    }
}

fn has_tag(w: &World, p: BlockPos, tag: &str) -> bool {
    state_props(w, p).is_some_and(|s| s.has_tag(tag))
}

fn is_water(w: &World, p: BlockPos) -> bool {
    // redsim's FluidState is water's (lava is its own block)
    !crate::fluid::fluid_state(w, p).is_empty()
}

/// `NaturalSpawner.isValidEmptySpawnBlock` for a type.
pub(crate) fn valid_empty(w: &World, p: BlockPos, type_id: &str) -> bool {
    let b = w.get(p);
    if crate::behavior::full_cube(&b) || crate::behavior::is_signal_source(w, &b) {
        return false;
    }
    let fluid = !crate::fluid::fluid_state(w, p).is_empty() || matches!(b, crate::block::Block::Lava { .. }) || state_props(w, p).is_some_and(|s| s.fluid != blocks::FluidKind::Empty);
    if fluid {
        return false;
    }
    if has_tag(w, p, "minecraft:prevent_mob_spawning_inside") {
        return false;
    }
    let col = blocks::type_index(type_id);
    !matches!((state_props(w, p), col), (Some(s), Some(c)) if s.dangerous.get(c).copied().unwrap_or(false))
}

/// `SpawnPlacements.isSpawnPositionOk`.
fn placement_ok(w: &World, k: &Kind, p: BlockPos) -> bool {
    match k.placement {
        Placement::NoRestrictions => true,
        Placement::InWater => is_water(w, p) && !crate::behavior::conductor_of(&w.get(p.above())),
        Placement::OnGround => valid_spawn_floor(w, p.below(), k.id) && valid_empty(w, p, k.id) && valid_empty(w, p.above(), k.id),
    }
}

/// `Monster.isDarkEnoughToSpawn`.
pub(crate) fn dark_enough(w: &mut World, p: BlockPos) -> bool {
    let sky = match w.sky_light(p) {
        Ok(v) => v,
        Err(e) => {
            w.errors.push(e);
            return false;
        }
    };
    let r = rng(w).next_int(32);
    if sky > r {
        return false;
    }
    let block = w.block_light(p).unwrap_or(15);
    if block > 0 {
        return false;
    }
    // Level.isThundering: the thunder level above 0.9 (then the light test darkens by 10)
    let thundering = w.is_thundering();
    let b = if thundering { brightness(w, p, 10) } else { let d = darken(w); brightness(w, p, d) };
    let sample = rng(w).next_int(8);
    b <= sample
}

/// `SpawnPlacements.checkSpawnRules(type, level, NATURAL, pos, level.random)`.
fn spawn_rules(w: &mut World, k: &Kind, p: BlockPos, nat: &Natural) -> bool {
    let not_peaceful = nat.difficulty != Difficulty::Peaceful;
    match k.rule {
        Rule::Always => true,
        Rule::AnyLight => valid_spawn_floor(w, p.below(), k.id),
        Rule::Monster => dark_enough(w, p) && valid_spawn_floor(w, p.below(), k.id),
        Rule::Animal => has_tag(w, p.below(), "minecraft:animals_spawnable_on") && brightness(w, p, 0) > 8,
        Rule::AnimalTag(tag) => has_tag(w, p.below(), tag) && brightness(w, p, 0) > 8,
        Rule::Turtle => p.y < 63 + 4 && has_tag(w, p.below(), "minecraft:turtles_spawnable_on") && brightness(w, p, 0) > 8,
        Rule::Drowned => {
            if !is_water(w, p.below()) {
                return false;
            }
            let frequent = w.biome_at(p).and_then(biome::biome).is_some_and(|b| b.has_tag("minecraft:more_frequent_drowned_spawns"));
            let can = not_peaceful && dark_enough(w, p) && is_water(w, p);
            if frequent {
                rng(w).next_int(15) == 0 && can
            } else {
                rng(w).next_int(40) == 0 && p.y < 63 - 5 && can
            }
        }
        Rule::Slime => {
            if !not_peaceful {
                return false;
            }
            let b = w.biome_at(p).and_then(biome::biome);
            if let Some(b) = b
                && b.has_tag("minecraft:allows_surface_slime_spawns")
                && p.y > 50
                && p.y < 70
            {
                let chance = b.surface_slime_chance;
                let f = rng(w).next_float();
                let d = darken(w);
                let light = brightness(w, p, d);
                let r = rng(w).next_int(8);
                if f < chance && light <= r {
                    return valid_spawn_floor(w, p.below(), k.id);
                }
            }
            let seed = w.land.as_ref().map(|l| l.seed).unwrap_or(0);
            let slime_chunk = slime_chunk(p.x.div_euclid(16), p.z.div_euclid(16), seed);
            if rng(w).next_int(10) == 0 && slime_chunk && p.y < 40 {
                return valid_spawn_floor(w, p.below(), k.id);
            }
            false
        }
        Rule::Bat => {
            if p.y >= w.height(crate::terrain::Heightmap::WorldSurface, p.x, p.z) {
                return false;
            }
            if rng(w).next_int(2) == 1 {
                return false;
            }
            let d = darken(w);
            let light = brightness(w, p, d);
            let r = rng(w).next_int(4);
            if light > r {
                return false;
            }
            has_tag(w, p.below(), "minecraft:bats_spawnable_on") && valid_spawn_floor(w, p.below(), k.id)
        }
        Rule::GlowSquid => p.y <= 63 - 33 && brightness(w, p, 0) == 0 && matches!(w.get(p), crate::block::Block::Water { .. }),
        Rule::SurfaceWater => p.y >= 63 - 13 && p.y <= 63 && is_water(w, p.below()) && matches!(w.get(p.above()), crate::block::Block::Water { .. }),
    }
}

/// Can `SpawnPlacements.checkSpawnRules` pass for type `k` at `p` at all: [`spawn_rules`] with
/// every static part exact (floor tags, biome, heights, the slime chunk, water) and every draw
/// at its best outcome, the light at its worst for the rule: a darkness rule (monsters, drowned,
/// surface slimes, bats) at the darkest sky (darkening 11, the night's: thunder's darkening 10
/// is lighter), a brightness rule (animals, turtles) without darkening (it reads none). The
/// hostile checks' C1 reads it, so a type the game can never spawn at a cell is never listed.
pub fn rules_can_pass(w: &World, k: &Kind, p: BlockPos, difficulty: Difficulty) -> bool {
    let not_peaceful = difficulty != Difficulty::Peaceful;
    let block = w.block_light(p).unwrap_or(0);
    // the darkest the rule can see (an unknown light: dark, so the exposure is listed)
    let darkest = w.raw_brightness(p, 11).unwrap_or(0);
    // without darkening (an unknown light: bright)
    let raw = w.raw_brightness(p, 0).unwrap_or(15);
    let tag = |t: &str| has_tag(w, p.below(), t);
    match k.rule {
        Rule::Always => true,
        Rule::AnyLight => valid_spawn_floor(w, p.below(), k.id),
        // isDarkEnoughToSpawn: sky > nextInt(32) never holds at 31; block light 0 (the overworld's
        // limit); the brightness under the darkening against UniformInt(0, 7)'s 7
        Rule::Monster => not_peaceful && block == 0 && darkest <= 7 && valid_spawn_floor(w, p.below(), k.id),
        Rule::Animal => tag("minecraft:animals_spawnable_on") && raw > 8,
        Rule::AnimalTag(t) => tag(t) && raw > 8,
        Rule::Turtle => p.y < 63 + 4 && tag("minecraft:turtles_spawnable_on") && raw > 8,
        Rule::Drowned => {
            let frequent = w.biome_at(p).and_then(biome::biome).is_some_and(|b| b.has_tag("minecraft:more_frequent_drowned_spawns"));
            not_peaceful && is_water(w, p.below()) && is_water(w, p) && block == 0 && darkest <= 7 && (frequent || p.y < 63 - 5)
        }
        Rule::Slime => {
            if !not_peaceful || !valid_spawn_floor(w, p.below(), k.id) {
                return false;
            }
            let surface = w.biome_at(p).and_then(biome::biome).is_some_and(|b| b.has_tag("minecraft:allows_surface_slime_spawns") && b.surface_slime_chance > 0.0) && p.y > 50 && p.y < 70 && darkest <= 7;
            let seed = w.land.as_ref().map(|l| l.seed).unwrap_or(0);
            surface || (p.y < 40 && slime_chunk(p.x.div_euclid(16), p.z.div_euclid(16), seed))
        }
        Rule::Bat => p.y < w.height(crate::terrain::Heightmap::WorldSurface, p.x, p.z) && darkest <= 3 && tag("minecraft:bats_spawnable_on") && valid_spawn_floor(w, p.below(), k.id),
        Rule::GlowSquid => p.y <= 63 - 33 && raw == 0 && matches!(w.get(p), crate::block::Block::Water { .. }),
        Rule::SurfaceWater => p.y >= 63 - 13 && p.y <= 63 && is_water(w, p.below()) && matches!(w.get(p.above()), crate::block::Block::Water { .. }),
    }
}

/// `WorldgenRandom.seedSlimeChunk(x, z, seed, 987234911).nextInt(10) == 0`.
pub fn slime_chunk(x: i32, z: i32, seed: i64) -> bool {
    let (x, z) = (x as i64, z as i64);
    let s = seed.wrapping_add(x.wrapping_mul(x).wrapping_mul(4987142)).wrapping_add(x.wrapping_mul(5947611)).wrapping_add(z.wrapping_mul(z).wrapping_mul(4392871)).wrapping_add(z.wrapping_mul(389711)) ^ 987234911;
    crate::rng::LegacyRandom::new(s).next_int(10) == 0
}

// ---------------------------------------------------------------------------- the spawner

/// The box of a type at a spawn position (`EntityType.getSpawnAABB`: its default dimensions).
/// `EntityType.getSpawnAABB(x, y, z)`: the type's base box scaled by its `spawnDimensionsScale`
/// (slime and magma cube 4, sulfur cube 2, every other type 1: `EntityTypes`), whatever size the
/// mob gets in `finalizeSpawn`.
fn spawn_box(ty: MobType, x: f64, y: f64, z: f64) -> Aabb {
    let d = ty.def();
    let scale = match d.game_id {
        "minecraft:slime" | "minecraft:magma_cube" => 4.0f32,
        "minecraft:sulfur_cube" => 2.0,
        _ => 1.0,
    };
    let hw = (scale * d.width / 2.0) as f64;
    Aabb::new(x - hw, y, z - hw, x + hw, y + (scale * d.height) as f64, z + hw)
}

/// The constructed mob's own box (`getBoundingBox` before `finalizeSpawn`: an adult, a cube mob at
/// size 1), for `checkSpawnObstruction` and the liquid test.
fn entity_box(ty: MobType, x: f64, y: f64, z: f64) -> Aabb {
    let d = ty.def();
    let hw = d.width as f64 / 2.0;
    Aabb::new(x - hw, y, z - hw, x + hw, y + d.height as f64, z + hw)
}

/// `CollisionGetter.noCollision(box)` (no entity): no block shape, no boat.
fn no_collision(w: &World, bb: Aabb) -> bool {
    crate::entity::collide::block_collisions(w, bb).is_empty() && w.get_entities(bb.inflate(1.0e-7, 1.0e-7, 1.0e-7), |e| matches!(e.kind, EntityKind::Boat(_))).is_empty()
}

fn player_chunk_dist2(p: Vec3, cx: i32, cz: i32) -> f64 {
    let (x, z) = ((cx * 16 + 8) as f64, (cz * 16 + 8) as f64);
    (x - p.x) * (x - p.x) + (z - p.z) * (z - p.z)
}

/// The chunks of the world box (only whole chunks inside it are loaded).
fn loaded_chunk(w: &World, cx: i32, cz: i32) -> bool {
    cx * 16 >= w.min.x && cz * 16 >= w.min.z && cx * 16 + 15 < w.min.x + w.size[0] && cz * 16 + 15 < w.min.z + w.size[2]
}

/// `NaturalSpawner.SpawnState` (the counts) and the local caps.
struct State {
    chunk_count: i32,
    counts: [i32; 8],
    /// Per player: per category.
    local: Vec<[i32; 8]>,
}

fn players_near_chunk(nat: &Natural, cx: i32, cz: i32) -> Vec<usize> {
    (0..nat.players.len()).filter(|&i| player_chunk_dist2(nat.players[i], cx, cz) < 16384.0).collect()
}

fn create_state(w: &World, nat: &Natural) -> State {
    // `getNaturalSpawnChunkCount`: the chunks within 8 of a player's chunk; a terrain box smaller
    // than that area loads only some of them, and the count is the loaded ones (so a cropped sample
    // keeps the game's mobs per chunk; a box covering the whole area is the game's count, 289 for
    // one player); a bridge sandbox (its ticking chunks known: `World::chunk_order`) counts the
    // whole square, as the game's ticket tracker does
    let bridge = !w.chunk_order.is_empty();
    let mut chunks = std::collections::HashSet::new();
    for p in &nat.players {
        let (pcx, pcz) = ((p.x.floor() as i32).div_euclid(16), (p.z.floor() as i32).div_euclid(16));
        for cz in pcz - 8..=pcz + 8 {
            for cx in pcx - 8..=pcx + 8 {
                if bridge || loaded_chunk(w, cx, cz) {
                    chunks.insert((cx, cz));
                }
            }
        }
    }
    let mut st = State { chunk_count: nat.spawn_chunk_count.unwrap_or(chunks.len() as i32), counts: [0; 8], local: vec![[0; 8]; nat.players.len()] };
    for e in &w.entities.list {
        if e.removed {
            continue;
        }
        let EntityKind::Mob(m) = &e.kind else { continue };
        if m.persistence_required || e.vehicle.is_some() {
            continue;
        }
        let cat = category_of(m.ty);
        if cat == Category::Misc {
            continue;
        }
        let b = e.block_position();
        let (cx, cz) = (b.x.div_euclid(16), b.z.div_euclid(16));
        if !bridge && !loaded_chunk(w, cx, cz) {
            continue;
        }
        st.counts[cat as usize] += 1;
        for pi in players_near_chunk(nat, cx, cz) {
            st.local[pi][cat as usize] += 1;
        }
    }
    st
}

/// `ServerChunkCache.tickChunks`' spawning, once a tick (the world tick's chunk slot).
pub fn tick_chunks(w: &mut World) {
    // the spawning chunks' inhabited time (`tickSpawningChunk`: every tick, spawning or not)
    if let Some(n) = w.mob.natural.as_mut() {
        n.ticks += 1;
    }
    let Some(mut nat) = w.mob.natural.clone() else { return };
    nat.players = players_of(w, &nat);
    if nat.players.is_empty() {
        return;
    }
    // (the mob-AI lane) the game collects and shuffles the spawning chunks every tick, spawn_mobs
    // or not (`doMobSpawning` only empties the categories): the shuffle's draws stay
    let spawning = nat.spawning;
    if let Some(l) = w.land.as_ref()
        && w.min.y > l.world_min_y
        && spawning
    {
        let e = "natural spawning needs the whole height of the world (the terrain box starts above its min y)".to_string();
        if !w.errors.contains(&e) {
            w.errors.push(e);
        }
        return;
    }
    let mut st = create_state(w, &nat);
    let spawn_persistent = w.time % 400 == 0;
    let enemies = nat.difficulty != Difficulty::Peaceful && nat.spawn_enemies;
    let cats: Vec<Category> = Category::SPAWNING
        .into_iter()
        .filter(|c| spawning && (enemies || c.friendly()) && (spawn_persistent || !c.persistent()) && st.counts[*c as usize] < c.max_instances() * st.chunk_count / 289)
        .collect();
    // collectSpawningChunks: the ticking chunks within 128 of a player — in a bridge sandbox the
    // game's own order (`World::chunk_order`), in a terrain box the candidate square (Chebyshev 8
    // of a player's chunk) that is loaded — then Util.shuffle with the level random
    let mut chunks: Vec<(i32, i32)> = Vec::new();
    if !nat.spawn_order.is_empty() {
        // the bridge's own list (collectSpawningChunks, before the shuffle)
        chunks.extend(nat.spawn_order.iter().map(|c| (c[0], c[1])));
    } else if !w.chunk_order.is_empty() {
        for c in &w.chunk_order {
            if nat.players.iter().any(|q| player_chunk_dist2(*q, c[0], c[1]) < 16384.0) {
                chunks.push((c[0], c[1]));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for p in nat.players.iter().filter(|_| w.chunk_order.is_empty()) {
        let (pcx, pcz) = ((p.x.floor() as i32).div_euclid(16), (p.z.floor() as i32).div_euclid(16));
        for cz in pcz - 8..=pcz + 8 {
            for cx in pcx - 8..=pcx + 8 {
                if seen.insert((cx, cz)) && loaded_chunk(w, cx, cz) && nat.players.iter().any(|q| player_chunk_dist2(*q, cx, cz) < 16384.0) {
                    chunks.push((cx, cz));
                }
            }
        }
    }
    let r = rng(w);
    for i in (2..=chunks.len()).rev() {
        let j = r.next_int(i as i32) as usize;
        chunks.swap(i - 1, j);
    }
    // `tickSpawningChunk` for each chunk of the shuffled list: in an entity-ticking chunk the
    // thunder roll (`ServerLevel.tickThunder`, mobinfra's `weather::lightning::thunder_chunk`),
    // then `spawnForChunk` where a category may spawn
    for (cx, cz) in chunks {
        let ticking = nat.entity_ticking.is_empty() || nat.entity_ticking.iter().any(|c| c[0] == cx && c[1] == cz);
        if ticking {
            crate::weather::lightning::thunder_chunk(w, cx, cz);
        }
        if cats.is_empty() || !ticking {
            continue;
        }
        for &cat in &cats {
            let local_ok = players_near_chunk(&nat, cx, cz).iter().any(|&pi| st.local[pi][cat as usize] < cat.max_instances());
            if local_ok {
                spawn_category_for_chunk(w, &nat, &mut st, cat, cx, cz);
            }
        }
    }
}

fn spawn_category_for_chunk(w: &mut World, nat: &Natural, st: &mut State, cat: Category, cx: i32, cz: i32) {
    // `getRandomPosWithin`: x, z in the chunk, y between the level's bottom and the column's
    // WORLD_SURFACE height (an empty column, or one outside redsim's box: the bottom)
    let min_y = w.land.as_ref().map(|l| l.world_min_y).unwrap_or(nat.world_min_y);
    let r = rng(w);
    let x = cx * 16 + r.next_int(16);
    let z = cz * 16 + r.next_int(16);
    let in_box = x >= w.min.x && z >= w.min.z && x < w.min.x + w.size[0] && z < w.min.z + w.size[2];
    let h = if in_box { w.height(crate::terrain::Heightmap::WorldSurface, x, z) } else { min_y };
    let top = if w.land.is_none() && h <= w.min.y { min_y } else { h };
    let y = rng(w).next_int(top - min_y + 1) + min_y;
    if y >= min_y + 1 {
        spawn_category_for_position(w, nat, st, cat, cx, cz, BlockPos::new(x, y, z));
    }
}

/// The nearest player's squared distance to a point (3D), if any.
fn nearest_player(nat: &Natural, x: f64, y: f64, z: f64) -> Option<f64> {
    nat.players.iter().map(|p| (p.x - x) * (p.x - x) + (p.y - y) * (p.y - y) + (p.z - z) * (p.z - z)).min_by(|a, b| a.partial_cmp(b).unwrap())
}

fn right_distance(w: &World, nat: &Natural, pos: BlockPos, d2: f64, cx: i32, cz: i32) -> bool {
    if d2 <= 576.0 {
        return false;
    }
    if let Some(s) = w.land.as_ref().map(|l| l.spawn).or(nat.world_spawn) {
        let (dx, dy, dz) = (s.x as f64 + 0.5 - (pos.x as f64 + 0.5), s.y as f64 + 0.5 - pos.y as f64, s.z as f64 + 0.5 - (pos.z as f64 + 0.5));
        if dx * dx + dy * dy + dz * dz < 576.0 {
            return false;
        }
    }
    let (pcx, pcz) = (pos.x.div_euclid(16), pos.z.div_euclid(16));
    (pcx == cx && pcz == cz) || spawnable_chunk(w, pcx, pcz)
}

/// `Level.canSpawnEntitiesInChunk`: a ticking chunk (the bridge sandbox's, when known), else a
/// chunk of the terrain box.
fn spawnable_chunk(w: &World, cx: i32, cz: i32) -> bool {
    if let Some(n) = w.mob.natural.as_ref()
        && !n.entity_ticking.is_empty()
    {
        return n.entity_ticking.iter().any(|c| c[0] == cx && c[1] == cz);
    }
    if !w.chunk_order.is_empty() {
        return w.chunk_order.iter().any(|c| c[0] == cx && c[1] == cz);
    }
    loaded_chunk(w, cx, cz)
}

/// The biome at a cell: the terrain's, or (a world without terrain, or a cell outside the sample's
/// biome cells) the level's own (`Natural::void_biome`: the bridge sandbox's flat source).
fn biome_name(w: &World, pos: BlockPos) -> Option<String> {
    if w.land.is_some()
        && let Some(b) = w.biome_at(pos)
    {
        return Some(b.to_string());
    }
    w.mob.natural.as_ref().and_then(|n| n.void_biome).map(|s| s.to_string())
}

/// `NaturalSpawner.mobsAt` (`ChunkGenerator.getMobsAt`): a structure's spawn override when the
/// cell is in it (its piece or its whole box, as the override says), else the biome's list
/// (`minecraft:gameplay/natural_mob_spawns` at the fuzzed biome).
pub fn mobs_at(w: &World, cat: Category, pos: BlockPos) -> Vec<Spawner> {
    let inside = |b: &[i32; 6]| pos.x >= b[0] && pos.y >= b[1] && pos.z >= b[2] && pos.x <= b[3] && pos.y <= b[4] && pos.z <= b[5];
    if let Some(l) = w.land.as_ref() {
        for s in &l.structures {
            if let Some(o) = biome::overrides(&s.id).iter().find(|o| o.category == cat.name()) {
                let hit = if o.piece { s.pieces.iter().any(inside) } else { inside(&[s.min[0], s.min[1], s.min[2], s.max[0], s.max[1], s.max[2]]) };
                if hit {
                    return o.spawns.clone();
                }
            }
        }
    }
    biome_name(w, pos).and_then(|b| biome::biome(&b)).map(|b| b.mobs(cat.name()).cloned().collect()).unwrap_or_default()
}

/// `getRandomSpawnMobAt`: the list at the cell (`WeightedList.getRandom`).
fn random_spawn_mob_at(w: &mut World, cat: Category, pos: BlockPos) -> Option<Spawner> {
    let b = biome_name(w, pos).and_then(|b| biome::biome(&b))?;
    if cat == Category::WaterAmbient && b.has_tag("minecraft:reduce_water_ambient_spawns") && rng(w).next_float() < 0.98 {
        return None;
    }
    let list = mobs_at(w, cat, pos);
    let total: i32 = list.iter().map(|s| s.weight).sum();
    if total <= 0 {
        return None;
    }
    let mut pick = rng(w).next_int(total);
    for s in list {
        pick -= s.weight;
        if pick < 0 {
            return Some(s);
        }
    }
    None
}

fn spawn_category_for_position(w: &mut World, nat: &Natural, st: &mut State, cat: Category, cx: i32, cz: i32, start: BlockPos) {
    if crate::behavior::conductor_of(&w.get(start)) {
        return;
    }
    let mut cluster = 0u32;
    for _group in 0..3 {
        let (mut x, mut z) = (start.x, start.z);
        let mut data: Option<Spawner> = None;
        let mut group = GroupData::default();
        let mut max = (rng(w).next_float() * 4.0).ceil() as i32;
        let mut group_size = 0;
        let mut ll = 0;
        while ll < max {
            ll += 1;
            let r = rng(w);
            x += r.next_int(6) - r.next_int(6);
            z += r.next_int(6) - r.next_int(6);
            let pos = BlockPos::new(x, start.y, z);
            let (xx, zz) = (x as f64 + 0.5, z as f64 + 0.5);
            let Some(d2) = nearest_player(nat, xx, start.y as f64, zz) else { continue };
            if !right_distance(w, nat, pos, d2, cx, cz) {
                continue;
            }
            if data.is_none() {
                match random_spawn_mob_at(w, cat, pos) {
                    Some(s) => {
                        max = if s.max > s.min { rng(w).next_int(s.max - s.min + 1) + s.min } else { s.min };
                        data = Some(s);
                    }
                    None => break,
                }
            }
            let s = data.clone().unwrap();
            let Some(k) = kind_of(&s.ty) else {
                let e = format!("natural spawning: {} is not modeled", s.ty);
                if !w.errors.contains(&e) {
                    w.errors.push(e);
                }
                continue;
            };
            if !valid_spawn_position_for_type(w, nat, cat, k, &s, pos, d2) {
                continue;
            }
            // getMobForSpawn: EntityType.create (the bridge's construction seeds count every
            // candidate, the ones isValidPositionForMob rejects included)
            if w.exact_seed.is_some() {
                let _ = w.construct();
            }
            // the mob: snapTo(xx, y, zz, random yaw, 0)
            let yaw = rng(w).next_float() * 360.0;
            // `isValidPositionForMob` sees the new mob as constructed (an adult, a slime of size
            // 1); `finalizeSpawn` then draws its baby flag or size
            if !valid_position_for_mob(w, k, k.ty, Vec3::new(xx, start.y as f64, zz), d2) {
                continue;
            }
            let f = finalize_spawn(w, nat, k, &mut group, pos);
            cluster += 1;
            group_size += 1;
            let id = spawn_mob(w, k, &f, Vec3::new(xx, start.y as f64, zz), yaw);
            st.counts[cat as usize] += 1;
            for pi in players_near_chunk(nat, pos.x.div_euclid(16), pos.z.div_euclid(16)) {
                st.local[pi][cat as usize] += 1;
            }
            let t = w.time;
            if let Some(n) = w.mob.natural.as_mut() {
                n.log.push(NaturalEvent::Spawn { t, ty: k.id, at: pos, entity: id, baby: f.baby });
            }
            if cluster >= k.cluster {
                return;
            }
            // isMaxGroupSizeReached: false for every modeled type
            let _ = group_size;
        }
    }
}

/// `isValidSpawnPostitionForType`.
fn valid_spawn_position_for_type(w: &mut World, nat: &Natural, cat: Category, k: &Kind, s: &Spawner, pos: BlockPos, d2: f64) -> bool {
    if cat == Category::Misc {
        return false;
    }
    let far = k.category.despawn_distance();
    if d2 > far * far {
        return false;
    }
    // still listed at this position (`canSpawnMobAt`)
    let listed = mobs_at(w, cat, pos).iter().any(|t| t == s);
    if !listed {
        return false;
    }
    if !placement_ok(w, k, pos) {
        return false;
    }
    if !spawn_rules(w, k, pos, nat) {
        return false;
    }
    no_collision(w, spawn_box(k.ty, pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5))
}

/// The group data `finalizeSpawn` passes along a group.
#[derive(Default)]
struct GroupData {
    /// `ZombieGroupData`: is baby (once drawn; `canSpawnJockey` true).
    zombie: Option<bool>,
    /// `AgeableMobGroupData`: (the group size so far, the baby chance, babies at all).
    ageable: Option<(u32, f32, bool)>,
    /// `SpiderEffectsGroupData` made (its effect drawn on Hard).
    spider: bool,
    /// `HorseGroupData` made (its variant drawn).
    horse: bool,
    /// `WolfPackData` made (its variant drawn).
    wolf: bool,
}

/// What a `finalizeSpawn` decided that redsim adds: the row (baby, size) and a jockey.
struct Finalized {
    ty: MobType,
    baby: bool,
    jockey: Jockey,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Jockey {
    None,
    /// A baby zombie rides the first chicken within its box inflated by (5, 3, 5) that is not
    /// ridden (none: it walks).
    NearChicken,
    /// A baby zombie rides a new chicken (its `finalizeSpawn` drawn inside the zombie's).
    NewChicken,
    /// A skeleton rides the spider (its `finalizeSpawn` drawn inside the spider's).
    SpiderSkeleton,
    /// A zombie rides the zombie horse.
    HorseZombie,
    /// A baby zombie rides the zombie horse.
    HorseBabyZombie,
}

/// `DifficultyInstance.getSpecialMultiplier` at the current time (`ServerLevel.getCurrentDifficultyAt`:
/// the overworld clock, the chunk's inhabited time (the ticks the spawner has run near a player:
/// the samples' chunks were never near one), the moon's brightness (by phase; it only counts after
/// 72000 ticks)).
fn special_multiplier(w: &World, nat: &Natural) -> f32 {
    let base = match nat.difficulty {
        Difficulty::Peaceful => return 0.0,
        Difficulty::Easy => 1.0f32,
        Difficulty::Normal => 2.0,
        Difficulty::Hard => 3.0,
    };
    let total = w.day_time() as f32;
    let global = ((total - 72000.0) / 1440000.0).clamp(0.0, 1.0) * 0.25;
    let mut local = (nat.ticks as f32 / 3600000.0).clamp(0.0, 1.0) * if nat.difficulty == Difficulty::Hard { 1.0 } else { 0.75 };
    const MOON: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
    let moon = MOON[(w.day_time().div_euclid(24000).rem_euclid(8)) as usize];
    local += (moon * 0.25).clamp(0.0, global);
    if nat.difficulty == Difficulty::Easy {
        local *= 0.5;
    }
    let effective = base * (0.75 + global + local);
    if effective < 2.0 {
        0.0
    } else if effective > 4.0 {
        1.0
    } else {
        (effective - 2.0) / 2.0
    }
}

/// A model error (once per run): something the game does that redsim does not model draw for draw.
fn model_error(w: &mut World, e: String) {
    if !w.errors.contains(&e) {
        w.errors.push(e);
    }
}

/// `Mob.finalizeSpawn`: the follow range's `triangle(0, 0.11485)` (2 nextDouble) and the left hand
/// (nextFloat), on the level random.
fn mob_base(w: &mut World) {
    let r = rng(w);
    r.next_double();
    r.next_double();
    r.next_float();
}

/// `AgeableMob.finalizeSpawn`'s baby roll (before `Mob`'s draws): after the group's first, a baby
/// with the group data's chance; the group data is made by the group's first mob.
fn ageable(w: &mut World, g: &mut GroupData, should: bool, chance: f32) -> bool {
    let (n, ch, sh) = g.ageable.get_or_insert((0, chance, should));
    let baby = *sh && *n > 0 && rng(w).next_float() <= *ch;
    *n += 1;
    baby
}

/// `Mob.populateDefaultEquipmentSlots`' armor: (the pieces it gives — a prefix of head, chest,
/// legs, feet (`EQUIPMENT_POPULATION_ORDER`) — and their material: `getEquipmentForSlot`'s type).
fn mob_armor(w: &mut World, special: f32, hard: bool) -> (usize, u8) {
    let r = rng(w);
    if r.next_float() >= 0.15 * special {
        return (0, 0);
    }
    let mut material = r.next_int(3);
    for _ in 0..3 {
        if r.next_float() < 0.1087 {
            material += 1;
        }
    }
    let partial = if hard { 0.1 } else { 0.25 };
    let mut n = 0;
    for i in 0..4 {
        if i > 0 && r.next_float() < partial {
            break;
        }
        n += 1;
    }
    (n, material as u8)
}

/// `Mob.populateDefaultEquipmentEnchantments`: the weapon (0.25), then each armor piece (0.5) in
/// `EquipmentSlot.VALUES` order (feet, legs, chest, head), times the special multiplier, each
/// enchanted by `MOB_SPAWN_EQUIPMENT` (`enchant::mob_spawn_equipment`, its draws in order).
fn enchant_draws(w: &mut World, special: f32, weapon: Option<super::enchant::Item>, armor: (usize, u8)) {
    if let Some(item) = weapon
        && rng(w).next_float() < 0.25 * special
    {
        super::enchant::mob_spawn_equipment(rng(w), item, special);
    }
    for slot in (0..4u8).rev() {
        if (slot as usize) < armor.0 && rng(w).next_float() < 0.5 * special {
            super::enchant::mob_spawn_equipment(rng(w), super::enchant::Item::Armor { slot, material: armor.1 }, special);
        }
    }
}

/// `Zombie.finalizeSpawn` (`drowned`: with `Drowned`'s equipment and its extra rolls); returns
/// (baby, jockey).
fn zombie_finalize(w: &mut World, nat: &Natural, g: &mut GroupData, drowned: bool, pos: BlockPos) -> (bool, Jockey) {
    let special = special_multiplier(w, nat);
    let hard = nat.difficulty == Difficulty::Hard;
    mob_base(w);
    rng(w).next_float(); // setCanPickUpLoot
    let baby = match g.zombie {
        Some(b) => b,
        None => {
            let b = rng(w).next_float() < 0.05; // getSpawnAsBabyOdds
            g.zombie = Some(b);
            b
        }
    };
    let mut jockey = Jockey::None;
    if baby {
        if rng(w).next_float() < 0.05 {
            jockey = Jockey::NearChicken;
        } else if rng(w).next_float() < 0.05 {
            // the new chicken: constructed, its finalizeSpawn (JOCKEY, no group data) here
            if w.exact_seed.is_some() {
                let _ = w.construct();
            }
            let mut cg = GroupData::default();
            farm_finalize(w, &mut cg, 2);
            jockey = Jockey::NewChicken;
        }
    }
    rng(w).next_float(); // setCanBreakDoors
    use super::enchant::Item;
    let (armor, weapon) = if drowned {
        // Drowned.populateDefaultEquipmentSlots (no armor): a trident (10 in 16) or a rod
        if rng(w).next_float() > 0.9 {
            let trident = rng(w).next_int(16) < 10;
            ((0, 0), Some(if trident { Item::Trident } else { Item::FishingRod }))
        } else {
            ((0, 0), None)
        }
    } else {
        let armor = mob_armor(w, special, hard);
        // Zombie.populateDefaultEquipmentSlots: an iron sword, spear (1 in 6 each) or shovel
        let weapon = if rng(w).next_float() < if hard { 0.05 } else { 0.01 } {
            Some(match rng(w).next_int(6) {
                0 => Item::IronSword,
                1 => Item::IronSpear,
                _ => Item::IronShovel,
            })
        } else {
            None
        };
        (armor, weapon)
    };
    enchant_draws(w, special, weapon, armor);
    // SpecialDates.isHalloween: the server's date is outside it (no draw); handleAttributes draws
    // from the zombie's own random
    if drowned {
        rng(w).next_float(); // the nautilus shell (0.03)
        if weapon == Some(Item::Trident) && rng(w).next_float() < 0.5 && !baby {
            // ... && !level.getBiome(pos).is(MORE_FREQUENT_DROWNED_SPAWNS) (rivers: no jockey)
            let frequent = biome_name(w, pos).and_then(|b| biome::biome(&b)).is_some_and(|b| b.has_tag("minecraft:more_frequent_drowned_spawns"));
            if !frequent {
                model_error(w, "natural spawning: a drowned's zombie nautilus jockey is not modeled".into());
            }
        }
    }
    (baby, jockey)
}

/// `AbstractSkeleton.finalizeSpawn`.
fn skeleton_finalize(w: &mut World, nat: &Natural) {
    let special = special_multiplier(w, nat);
    let hard = nat.difficulty == Difficulty::Hard;
    mob_base(w);
    let armor = mob_armor(w, special, hard);
    enchant_draws(w, special, Some(super::enchant::Item::Bow), armor);
    rng(w).next_float(); // setCanPickUpLoot
}

/// A pig's, cow's or chicken's `finalizeSpawn`: the variant (one candidate), the sound variant,
/// `AgeableMob`, `Mob`; returns baby.
fn farm_finalize(w: &mut World, g: &mut GroupData, sounds: i32) -> bool {
    rng(w).next_int(1);
    rng(w).next_int(sounds);
    let baby = ageable(w, g, true, 0.05);
    mob_base(w);
    baby
}

/// `finalizeSpawn(NATURAL)` of a spawned mob, every level-random draw in the game's order (the
/// mob's own random's draws — a zombie's `handleAttributes`, a salmon's variant — come from its
/// construction, the tick-exact lane's `World::construct`).
fn finalize_spawn(w: &mut World, nat: &Natural, k: &Kind, g: &mut GroupData, pos: BlockPos) -> Finalized {
    let special = special_multiplier(w, nat);
    let hard = nat.difficulty == Difficulty::Hard;
    let mut baby = false;
    let mut jockey = Jockey::None;
    let mut ty = k.ty;
    match k.finalize {
        Finalize::Nothing => {}
        Finalize::Mob | Finalize::Cod => mob_base(w),
        Finalize::Salmon => mob_base(w),
        Finalize::Zombie | Finalize::Drowned => {
            (baby, jockey) = zombie_finalize(w, nat, g, k.finalize == Finalize::Drowned, pos);
        }
        Finalize::Skeleton => skeleton_finalize(w, nat),
        Finalize::Spider => {
            mob_base(w);
            if rng(w).next_int(100) == 0 {
                // the skeleton rider: constructed, its finalizeSpawn (NATURAL, no group data)
                if w.exact_seed.is_some() {
                    let _ = w.construct();
                }
                skeleton_finalize(w, nat);
                jockey = Jockey::SpiderSkeleton;
            }
            if !g.spider {
                g.spider = true;
                if hard && rng(w).next_float() < 0.1 * special {
                    // the group's effect (speed, strength, regeneration, invisibility): only the
                    // spiders' unmodeled AI reads it
                    rng(w).next_int(5);
                }
            }
        }
        Finalize::Slime => {
            ageable(w, g, false, 0.05);
            mob_base(w);
            let r = rng(w);
            let mut scale = r.next_int(3);
            if scale < 2 && r.next_float() < 0.5 * special {
                scale += 1;
            }
            ty = match scale {
                0 => MobType::Natural(6),
                1 => MobType::Natural(38),
                _ => MobType::Natural(39),
            };
        }
        Finalize::SulfurCube => {
            baby = ageable(w, g, true, 0.05);
            mob_base(w);
            // SulfurCube.setSpawnSize: 1 for a baby, else 2
            ty = if baby { MobType::Natural(9) } else { MobType::Natural(40) };
        }
        Finalize::Squid => {
            baby = ageable(w, g, true, 0.05);
            mob_base(w);
        }
        Finalize::Sheep => {
            // SheepColorSpawnRules: 100 = 5 + 5 + 5 + 3 + 82, the last a 499 + 1 list
            if rng(w).next_int(100) >= 18 {
                rng(w).next_int(500);
            }
            baby = ageable(w, g, true, 0.05);
            mob_base(w);
        }
        Finalize::Farm { sounds } => baby = farm_finalize(w, g, sounds),
        Finalize::Horse => {
            if !g.horse {
                g.horse = true;
                rng(w).next_int(7); // Variant.values()
                g.ageable.get_or_insert((0, 0.05, true)); // HorseGroupData(true)
            }
            rng(w).next_int(5); // Markings.values()
            let r = rng(w);
            r.next_int(8);
            r.next_int(9);
            for _ in 0..6 {
                r.next_double(); // speed, then jump strength: 3 each
            }
            baby = ageable(w, g, true, 0.2);
            mob_base(w);
        }
        Finalize::Donkey => {
            g.ageable.get_or_insert((0, 0.2, true)); // AgeableMobGroupData(0.2)
            let r = rng(w);
            r.next_int(8);
            r.next_int(9);
            baby = ageable(w, g, true, 0.2);
            mob_base(w);
        }
        Finalize::Wolf => {
            if !g.wolf {
                g.wolf = true;
                rng(w).next_int(1); // the biome's variant
                g.ageable.get_or_insert((0, 0.05, false)); // WolfPackData: no babies
            }
            rng(w).next_int(7); // the sound variant
            baby = ageable(w, g, false, 0.05);
            mob_base(w);
        }
        Finalize::ZombieHorse => {
            // the zombie rider: constructed, its finalizeSpawn (NATURAL, no group data); a baby's
            // chicken would be left behind when it mounts the horse
            if w.exact_seed.is_some() {
                let _ = w.construct();
            }
            let mut zg = GroupData::default();
            let (zbaby, zj) = zombie_finalize(w, nat, &mut zg, false, pos);
            if zj != Jockey::None {
                model_error(w, "natural spawning: a zombie horse rider's chicken jockey is not modeled".into());
            }
            jockey = if zbaby { Jockey::HorseBabyZombie } else { Jockey::HorseZombie };
            g.ageable.get_or_insert((0, 0.2, true));
            let r = rng(w);
            for _ in 0..6 {
                r.next_double(); // jump, then speed: 3 each
            }
            baby = ageable(w, g, true, 0.2);
            mob_base(w);
        }
        Finalize::Unmodeled => model_error(w, format!("natural spawning: {}'s finalizeSpawn is not modeled draw for draw", k.id)),
    }
    if baby && k.finalize != Finalize::SulfurCube {
        match k.baby {
            Some(b) => ty = b,
            // a baby squid: its smaller box is not modeled (nor its AI)
            None => {}
        }
    }
    Finalized { ty, baby, jockey }
}

/// `isValidPositionForMob`: the despawn distance, `checkSpawnRules` (the walk target value) and
/// `checkSpawnObstruction`.
fn valid_position_for_mob(w: &mut World, k: &Kind, ty: MobType, pos: Vec3, d2: f64) -> bool {
    let far = k.category.despawn_distance();
    if d2 > far * far && k.removes_far {
        return false;
    }
    let cell = BlockPos::new(pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32);
    let walk_ok = match k.walk_target {
        WalkTarget::None => true,
        WalkTarget::Monster => -(magic(w, cell) - 0.5) >= 0.0,
        WalkTarget::Animal => {
            let grass = state_props(w, cell.below()).is_some_and(|s| s.block_id() == "minecraft:grass_block");
            grass || magic(w, cell) - 0.5 >= 0.0
        }
    };
    if !walk_ok {
        return false;
    }
    let bb = entity_box(ty, pos.x, pos.y, pos.z);
    if !k.water_obstruction && super::blocks::box_has_liquid(w, bb) {
        return false;
    }
    super::spawn::is_unobstructed(w, bb)
}

/// Add the mob (and what its `finalizeSpawn` adds: a jockey's chicken, a spider's skeleton, a
/// zombie horse's rider).
fn spawn_mob(w: &mut World, k: &Kind, f: &Finalized, pos: Vec3, yaw: f32) -> u32 {
    // a new chicken enters during the zombie's finalizeSpawn, before the zombie
    let chicken = (f.jockey == Jockey::NewChicken).then(|| {
        let c = super::add(w, MobType::Natural(12), pos, false);
        super::seed_entered(w, c);
        c
    });
    let id = super::add(w, f.ty, pos, false);
    super::seed_entered(w, id);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        // snapTo keeps the drawn yaw as it is (0..360, not wrapped to -180..180)
        e.y_rot = yaw;
    }
    let ride = |w: &mut World, rider: u32, vehicle: u32| {
        if let Some(n) = w.mob.natural.as_mut() {
            n.riders.push((rider, vehicle));
        }
    };
    match f.jockey {
        Jockey::None => {}
        Jockey::NewChicken => ride(w, id, chicken.unwrap()),
        Jockey::NearChicken => {
            // the first chicken in the zombie's box inflated by (5, 3, 5) that nobody rides
            let bb = entity_box(f.ty, pos.x, pos.y, pos.z);
            let riders: Vec<u32> = w.mob.natural.as_ref().map(|n| n.riders.iter().map(|r| r.1).collect()).unwrap_or_default();
            let near = w.entities.list.iter().find(|e| {
                !e.removed
                    && e.id != id
                    && matches!(&e.kind, EntityKind::Mob(m) if m.ty.def().game_id == "minecraft:chicken")
                    && !riders.contains(&e.id)
                    && e.pos.x > bb.min_x - 5.0
                    && e.pos.x < bb.max_x + 5.0
                    && e.pos.y > bb.min_y - 3.0
                    && e.pos.y < bb.max_y + 3.0
                    && e.pos.z > bb.min_z - 5.0
                    && e.pos.z < bb.max_z + 5.0
            });
            if let Some(c) = near.map(|e| e.id) {
                ride(w, id, c);
            }
        }
        Jockey::SpiderSkeleton => {
            let r = super::add(w, MobType::Natural(4), pos, true);
            super::seed_entered(w, r);
            ride(w, r, id);
        }
        Jockey::HorseZombie | Jockey::HorseBabyZombie => {
            let rty = if f.jockey == Jockey::HorseBabyZombie { MobType::Natural(25) } else { MobType::Zombie };
            let r = super::add(w, rty, pos, true);
            super::seed_entered(w, r);
            ride(w, r, id);
        }
    }
    let _ = k;
    id
}

/// Why a type can or cannot spawn at a cell (each check of `isValidSpawnPostitionForType` and
/// `isValidPositionForMob` with its own answer; the random gates reported as their thresholds).
pub fn explain(w: &mut World, type_id: &str, p: BlockPos) -> String {
    let Some(k) = kind_of(type_id) else { return format!("{type_id}: not modeled") };
    let biome = w.biome_at(p).map(|s| s.to_string()).unwrap_or_else(|| "-".into());
    let listed = biome::biome(&biome).is_some_and(|b| b.mobs(k.category.name()).any(|s| s.ty == type_id));
    let below = w.get(p.below());
    let floor = valid_spawn_floor(w, p.below(), k.id);
    let (e0, e1) = (valid_empty(w, p, k.id), valid_empty(w, p.above(), k.id));
    let sky = w.sky_light(p).unwrap_or(-1);
    let block = w.block_light(p).unwrap_or(-1);
    let d = darken(w);
    let bright = brightness(w, p, d);
    let bb = spawn_box(k.ty, p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5);
    let free = no_collision(w, bb);
    let liquid = super::blocks::box_has_liquid(w, bb);
    let unob = super::spawn::is_unobstructed(w, bb);
    format!(
        "{type_id} at {p}: biome {biome} (listed {listed}); below {below} (valid floor {floor}); feet {} empty {e0}, head {} empty {e1}; sky {sky} block {block} brightness {bright} (darken {d}); no collision {free}; liquid {liquid}; unobstructed {unob}",
        w.get(p),
        w.get(p.above())
    )
}

// --------------------------------------------------------------------------- despawning

/// `Mob.checkDespawn` for entity `i` (the entity phase calls it before the entity's tick), then
/// the `noActionTime` its AI step adds (`Mob.serverAiStep` +1, `Monster.updateNoActionTime` +2 in
/// light above 0.5).
pub fn check_despawn(w: &mut World, i: usize) {
    let Some(nat) = w.mob.natural.clone() else { return };
    let (ty, persistent, pos, eye) = match &w.entities.list[i].kind {
        EntityKind::Mob(m) if !m.dead => (m.ty, m.persistence_required, w.entities.list[i].pos, m.eye()),
        _ => return,
    };
    let Some(k) = kind_of_type(ty) else { return };
    let id = w.entities.list[i].id;
    if nat.difficulty == Difficulty::Peaceful && k.category == Category::Monster {
        discard(w, i, id, k, false);
        return;
    }
    let passenger = w.entities.list[i].vehicle.is_some() || nat.riders.iter().any(|(r, _)| *r == id);
    let persistent = persistent || passenger;
    if let Some(d2) = nearest_player(&nat, pos.x, pos.y, pos.z) {
        let far = k.category.despawn_distance();
        if !persistent && d2 > far * far && k.removes_far {
            discard(w, i, id, k, true);
            return;
        }
        let no_action = match &w.entities.list[i].kind {
            EntityKind::Mob(m) => m.no_action_time,
            _ => 0,
        };
        let roll = if !persistent && no_action > 600 { w.entities.list[i].random.as_mut().map(|r| r.next_int(800) == 0).unwrap_or(false) } else { false };
        if roll && d2 > 1024.0 && k.removes_far {
            discard(w, i, id, k, false);
            return;
        } else if d2 < 1024.0
            && let EntityKind::Mob(m) = &mut w.entities.list[i].kind
        {
            m.no_action_time = 0;
        }
    }
    // `Monster.updateNoActionTime` (+2 in light above 0.5; `Mob.serverAiStep`'s +1 is the mob
    // layer's, living.rs); a mob with the AI on runs it in its aiStep (mob AI lane)
    let cell = BlockPos::new(pos.x.floor() as i32, (pos.y + eye as f64).floor() as i32, pos.z.floor() as i32);
    let lit = k.category == Category::Monster && !super::ai::has_ai(w, i) && magic(w, cell) > 0.5;
    if lit && let EntityKind::Mob(m) = &mut w.entities.list[i].kind {
        m.no_action_time += 2;
    }
}

fn discard(w: &mut World, i: usize, id: u32, k: &Kind, far: bool) {
    w.entities.list[i].removed = true;
    let t = w.time;
    if let Some(n) = w.mob.natural.as_mut() {
        n.log.push(NaturalEvent::Despawn { t, entity: id, ty: k.id, far });
        // its riders dismount (no longer passengers)
        let freed: Vec<u32> = n.riders.iter().filter(|(_, v)| *v == id).map(|(r, _)| *r).collect();
        n.riders.retain(|(r, v)| *v != id && *r != id);
        for r in freed {
            if let Some(j) = crate::entity::index_of(w, r)
                && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
            {
                m.persistence_required = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_indexed() {
        for (i, d) in DEFS.iter().enumerate() {
            assert_eq!(d.ty, MobType::Natural(i as u16), "{}", d.id);
        }
        for k in KINDS {
            assert_eq!(k.ty.def().game_id, k.id);
        }
    }

    #[test]
    fn slime_chunks_follow_the_seed() {
        // about one chunk in ten
        let n = (0..100).flat_map(|x| (0..100).map(move |z| (x, z))).filter(|&(x, z)| slime_chunk(x, z, 2)).count();
        assert!((800..1200).contains(&n), "{n}");
    }
}
