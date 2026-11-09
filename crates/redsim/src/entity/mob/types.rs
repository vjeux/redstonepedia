//! The mob type registry (ironfarm, 2026-11-08): each modeled mob's dimensions, attributes and
//! rules, read in the 26.3 source (`EntityTypes` sizes and eye heights, each class's
//! `createAttributes`, the entity type tags). A new mob is a [`MobType`] variant and its
//! [`TypeDef`] row; the rules that read a row are in `living.rs`, `spawn.rs` and the kind's own
//! module.

/// A modeled mob.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum MobType {
    Villager,
    IronGolem,
    Zombie,
    /// What lightning turns a villager into (`Villager.thunderHit`): its dimensions and
    /// attributes only, no AI (a witch in a farm fails the run).
    Witch,
    /// A natural mob of the terrain mode (NaturalSpawner's): its row in
    /// [`super::natural::DEFS`] (the terrain track's table), by index.
    Natural(u16),
}

/// What a mob's death drops (`loot_table/entities/<id>.json`): `(item, min, max)` pools, each
/// one roll of `set_count uniform(min, max)` (`UniformGenerator.getInt` = `Mth.nextInt(random,
/// floor(min), floor(max))`), in pool order.
pub type Loot = &'static [(&'static str, i32, i32)];

/// One mob type's row.
#[derive(Debug)]
pub struct TypeDef {
    pub ty: MobType,
    /// The entity id (`minecraft:<id>`).
    pub id: &'static str,
    /// The same, with its namespace (`minecraft:villager`).
    pub game_id: &'static str,
    /// `EntityTypes.<X>.sized(width, height).eyeHeight(eye)` (default eye: height x 0.85).
    pub width: f32,
    pub height: f32,
    pub eye: f32,
    /// `.ridingOffset(..)` (how far a rider sits below its vehicle's attachment point).
    pub riding_offset: f32,
    /// `Attributes.MAX_HEALTH`.
    pub max_health: f32,
    /// `Attributes.MOVEMENT_SPEED`.
    pub speed: f64,
    /// `Attributes.FOLLOW_RANGE` (base, before `Mob.finalizeSpawn`'s random bonus).
    pub follow_range: f64,
    /// `Attributes.STEP_HEIGHT`.
    pub step: f32,
    /// `Attributes.KNOCKBACK_RESISTANCE`.
    pub knockback_resistance: f64,
    /// `Attributes.ARMOR`.
    pub armor: f32,
    /// `EntityType.Builder.fireImmune()`.
    pub fire_immune: bool,
    /// Tag `fall_damage_immune`.
    pub fall_immune: bool,
    /// Its `decreaseAirSupply` lowers the air (an iron golem's returns it unchanged: no drowning).
    pub drowns: bool,
    /// Tag `burn_in_daylight` (`Mob.burnUndead`).
    pub burns_in_daylight: bool,
    /// `MobCategory.MONSTER` (despawns away from players unless persistent).
    pub monster: bool,
    /// Its death loot.
    pub loot: Loot,
}

/// Every modeled type.
pub static TYPES: &[TypeDef] = &[
    // EntityTypes.VILLAGER.sized(0.6F, 1.95F).eyeHeight(1.62F); Villager.createAttributes:
    // Mob.createMobAttributes() (follow range 16) + MOVEMENT_SPEED 0.5; no loot
    TypeDef {
        ty: MobType::Villager,
        id: "villager",
        game_id: "minecraft:villager",
        width: 0.6,
        height: 1.95,
        eye: 1.62,
        riding_offset: 0.0,
        max_health: 20.0,
        speed: 0.5,
        follow_range: 16.0,
        step: 0.6,
        knockback_resistance: 0.0,
        armor: 0.0,
        fire_immune: false,
        fall_immune: false,
        drowns: true,
        burns_in_daylight: false,
        monster: false,
        loot: &[],
    },
    // EntityTypes.IRON_GOLEM.sized(1.4F, 2.7F) (eye 2.7 x 0.85 = 2.295); IronGolem.createAttributes:
    // MAX_HEALTH 100, MOVEMENT_SPEED 0.25, KNOCKBACK_RESISTANCE 1, ATTACK_DAMAGE 15, STEP_HEIGHT 1;
    // fall_damage_immune; decreaseAirSupply returns the supply; loot_table/entities/iron_golem.json
    TypeDef {
        ty: MobType::IronGolem,
        id: "iron_golem",
        game_id: "minecraft:iron_golem",
        width: 1.4,
        height: 2.7,
        eye: 2.295,
        riding_offset: 0.0,
        max_health: 100.0,
        speed: 0.25,
        follow_range: 16.0,
        step: 1.0,
        knockback_resistance: 1.0,
        armor: 0.0,
        fire_immune: false,
        fall_immune: true,
        drowns: false,
        burns_in_daylight: false,
        monster: false,
        loot: &[("minecraft:poppy", 0, 2), ("minecraft:iron_ingot", 3, 5)],
    },
    // EntityTypes.ZOMBIE.sized(0.6F, 1.95F).eyeHeight(1.74F).ridingOffset(-0.7F);
    // Zombie.createAttributes: FOLLOW_RANGE 35, MOVEMENT_SPEED 0.23, ATTACK_DAMAGE 3, ARMOR 2;
    // tag burn_in_daylight; loot_table/entities/zombie.json (rotten flesh 0..2; the rare iron,
    // carrot or potato needs a player kill)
    TypeDef {
        ty: MobType::Zombie,
        id: "zombie",
        game_id: "minecraft:zombie",
        width: 0.6,
        height: 1.95,
        eye: 1.74,
        riding_offset: -0.7,
        max_health: 20.0,
        speed: 0.23f32 as f64,
        follow_range: 35.0,
        step: 0.6,
        knockback_resistance: 0.0,
        armor: 2.0,
        fire_immune: false,
        fall_immune: false,
        drowns: true,
        burns_in_daylight: true,
        monster: true,
        loot: &[("minecraft:rotten_flesh", 0, 2)],
    },
    // EntityTypes.WITCH.sized(0.6F, 1.95F).eyeHeight(1.62F), MobCategory.MONSTER;
    // Witch.createAttributes: Monster.createMonsterAttributes() (follow range 16) + MAX_HEALTH 26,
    // MOVEMENT_SPEED 0.25 (no AI and no loot modeled: a witch in a farm fails its run)
    TypeDef {
        ty: MobType::Witch,
        id: "witch",
        game_id: "minecraft:witch",
        width: 0.6,
        height: 1.95,
        eye: 1.62,
        riding_offset: 0.0,
        max_health: 26.0,
        speed: 0.25,
        follow_range: 16.0,
        step: 0.6,
        knockback_resistance: 0.0,
        armor: 0.0,
        fire_immune: false,
        fall_immune: false,
        drowns: true,
        burns_in_daylight: false,
        monster: true,
        loot: &[],
    },
];

impl MobType {
    /// Its row (a natural mob's in the terrain track's table).
    pub fn def(self) -> &'static TypeDef {
        // (an index by type built once, each type's first row as the search found it: tickexact
        // 2026-11-08, a linear search ran at every mob's every property read)
        static INDEX: std::sync::OnceLock<crate::fxhash::FxHashMap<MobType, &'static TypeDef>> = std::sync::OnceLock::new();
        let index = INDEX.get_or_init(|| {
            let mut m = crate::fxhash::FxHashMap::default();
            for d in TYPES.iter().chain(super::natural::DEFS.iter()) {
                m.entry(d.ty).or_insert(d);
            }
            m
        });
        match index.get(&self) {
            Some(d) => d,
            None => TYPES.iter().chain(super::natural::DEFS.iter()).find(|d| d.ty == self).expect("every MobType has a TypeDef row"),
        }
    }

    /// The type of an entity id (`minecraft:` optional), if modeled.
    pub fn from_id(id: &str) -> Option<MobType> {
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        TYPES.iter().chain(super::natural::DEFS.iter()).find(|d| d.id == id).map(|d| d.ty)
    }

    /// The game's entity id, `minecraft:<id>`.
    pub fn game_id(self) -> &'static str {
        self.def().game_id
    }
}

/// `LivingEntity.SLEEPING_DIMENSIONS`: `fixed(0.2F, 0.2F).withEyeHeight(0.2F)`.
pub const SLEEPING: (f32, f32, f32) = (0.2, 0.2, 0.2);
