//! Mobs (ironfarm, 2026-11-08; brief `designs/briefs/farm-iron.md`): villagers, iron golems and
//! zombies in redsim's entity list, ticked in its order like every other entity, with the rules
//! read in the 26.3 source:
//!
//! * `types.rs`: the type registry (sizes, attributes, immunities, loot);
//! * `living.rs`: `LivingEntity` (base tick, AI step, travel, damage, death, drops, pushing);
//! * `villager.rs`: the villager's `Brain` (memories, sensors, activities, the schedule), its
//!   panic and sleep behaviors, `spawnGolemIfNeeded`, sleeping and waking;
//! * `spawn.rs`: `SpawnUtil.trySpawnMob` (`LEGACY_IRON_GOLEM`) and `IronGolem.checkSpawnObstruction`;
//! * `los.rs`: line of sight (`ClipContext.Block.COLLIDER`);
//! * `blocks.rs`: the block properties these rules read (beds, lava, spawn checks);
//! * `walk.rs`: the walking input the judge gives a mob whose AI is not modeled (its robustness
//!   runs), never a guess of the game's goals.
//!
//! Exactness policy: the same formulas in the same order as the game for everything that sets a
//! rate or a pass/fail. Where the game draws from a random redsim cannot mirror (an entity's own
//! unseeded random; the level random that unmodeled behaviors also draw from), the draw comes from
//! [`MobWorld::rng`], seeded by the caller, with the same distribution.

pub mod absence;
// ---- mob AI (the mob-AI lane, `ai/`): path finding, navigation, controls, goals ----
pub mod ai;
pub mod blocks;
pub mod custom;
pub mod drive;
pub mod enchant;
pub mod exit;
pub mod farm;
pub mod hostile;
pub mod living;
pub mod loot;
pub mod los;
pub mod natural;
pub mod prices;
pub mod refuse;
pub mod site;
pub mod spawn;
pub mod sun;
pub mod types;
pub mod villager;
pub mod view;
pub mod walk;

#[cfg(test)]
mod tests;

/// The items the mob farms add to the item registry (`redstone::container`, collected by
/// build.rs): an iron golem's poppy.
pub const MODULE_ITEMS: &[(&str, u8)] = &[
    ("poppy", 64),
    // the natural mobs' loot (entity::mob::loot), the cooked forms a mob on fire drops, and the
    // player-only drops their tables name
    ("rotten_flesh", 64),
    ("red_mushroom", 64),
    ("arrow", 64),
    ("bone", 64),
    ("gunpowder", 64),
    ("string", 64),
    ("glowstone_dust", 64),
    ("sugar", 64),
    ("spider_eye", 64),
    ("glass_bottle", 64),
    ("stick", 64),
    ("redstone", 64),
    ("ender_pearl", 16),
    ("slime_ball", 64),
    ("ink_sac", 64),
    ("glow_ink_sac", 64),
    ("feather", 64),
    ("chicken", 64),
    ("cooked_chicken", 64),
    ("leather", 64),
    ("beef", 64),
    ("cooked_beef", 64),
    ("porkchop", 64),
    ("cooked_porkchop", 64),
    ("mutton", 64),
    ("cooked_mutton", 64),
    ("white_wool", 64),
    ("orange_wool", 64),
    ("magenta_wool", 64),
    ("light_blue_wool", 64),
    ("yellow_wool", 64),
    ("lime_wool", 64),
    ("pink_wool", 64),
    ("gray_wool", 64),
    ("light_gray_wool", 64),
    ("cyan_wool", 64),
    ("purple_wool", 64),
    ("blue_wool", 64),
    ("brown_wool", 64),
    ("green_wool", 64),
    ("red_wool", 64),
    ("black_wool", 64),
    ("rabbit_hide", 64),
    ("rabbit", 64),
    ("cooked_rabbit", 64),
    ("rabbit_foot", 64),
    ("seagrass", 64),
    ("bowl", 64),
    ("salmon", 64),
    ("cooked_salmon", 64),
    ("cod", 64),
    ("cooked_cod", 64),
    ("bone_meal", 64),
    ("iron_ingot", 64),
    ("carrot", 64),
    ("potato", 64),
    ("baked_potato", 64),
    ("copper_ingot", 64),
    ("music_disc_lava_chicken", 1),
    // (mob-AI 2026-11-11) a trader llama's dropped lead
    ("lead", 64),
    // (b8949c96 2026-11-12) a zombified piglin's loot (the kill-room guard's crowd); last, so the
    // ids before it stay
    ("gold_nugget", 64),
];

use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::rng::LegacyRandom;
use crate::world::World;
use types::MobType;

/// The damage sources the modeled rules deal (`DamageTypes`), with their tags.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Damage {
    Lava,
    OnFire,
    InWall,
    Cramming,
    Fall,
    Drown,
    HotFloor,
    /// mobinfra: a lightning bolt (`lightning_bolt`: not fire, armor counts).
    Lightning,
    /// mobinfra: standing in a fire a bolt set (`in_fire`: fire, armor counts).
    InFire,
    /// mob AI: a mob's melee hit (`mob_attack`: armor counts, knockback from the attacker, who is
    /// `MobWorld.ai.attacker` during the call).
    MobAttack,
    /// mob AI: an arrow's hit (`arrow`: a projectile; armor counts; its owner is
    /// `MobWorld.ai.attacker` and its motion `MobWorld.ai.projectile_motion` during the call: the
    /// knockback goes along the arrow's motion).
    Arrow,
    /// mob AI: a spear's stab (`spear`: armor counts; `#no_knockback`, the stab pushes on its
    /// own; `#is_player_attack`, so a panic cause; the attacker is `MobWorld.ai.attacker`).
    Spear,
    /// mob AI: a thrown trident's hit (`trident`: a projectile, a panic cause; armor counts; its
    /// owner is `MobWorld.ai.attacker` and its motion `MobWorld.ai.projectile_motion`, as an
    /// arrow's).
    Trident,
    /// (mob-AI 2026-11-10) `LivingEntity.onBelowWorld`: 64 below the world's bottom
    /// (`fell_out_of_world`'s `out_of_world`: armor and resistance bypassed, no knockback; the
    /// hurt cooldown still holds, `#bypasses_cooldown` being empty).
    OutOfWorld,
    /// (mob-AI 2026-11-11) A llama's spit (`spit`, message "mob": not `#is_projectile`, no panic
    /// cause; armor counts; its owner is `MobWorld.ai.attacker` and the spit's motion
    /// `MobWorld.ai.projectile_motion`: the knockback goes along it, `Projectile`'s
    /// `calculateHorizontalHurtKnockbackDirection`).
    Spit,
    /// (mob-AI 2026-11-11) An explosion's (`explosion`: `#is_explosion`, `#no_knockback`,
    /// `#panic_causes`; armor and blast protection count). A creeper's has the creeper as its
    /// attacker (`MobWorld.ai.attacker` during the call: `Explosion.getIndirectSourceEntity`).
    Explosion,
}

impl Damage {
    /// Caused by a living attacker (`MobWorld.ai.attacker` during the call): a melee hit, an
    /// arrow's or a trident's owner, a spear's holder, a spitting llama.
    pub fn by_attacker(self) -> bool {
        matches!(self, Damage::MobAttack | Damage::Arrow | Damage::Spear | Damage::Trident | Damage::Spit | Damage::Explosion)
    }

    /// Tag `no_knockback` among the attacker damages: no `dealDefaultKnockback` (a spear's stab
    /// pushes on its own; an explosion pushes everything it reaches in `hurtEntities`).
    pub fn no_knockback(self) -> bool {
        matches!(self, Damage::Spear | Damage::Explosion)
    }

    /// A projectile's (`#is_projectile` among the modeled): the knockback goes along its motion.
    pub fn by_projectile(self) -> bool {
        matches!(self, Damage::Arrow | Damage::Trident)
    }

    /// The direct entity is a `Projectile` (an arrow, a trident, a llama's spit): the knockback
    /// goes along its motion.
    pub fn knockback_along_projectile(self) -> bool {
        matches!(self, Damage::Arrow | Damage::Trident | Damage::Spit)
    }
}

impl Damage {
    /// Tag `is_fire`.
    pub fn is_fire(self) -> bool {
        matches!(self, Damage::Lava | Damage::OnFire | Damage::HotFloor | Damage::InFire)
    }

    /// Tag `bypasses_armor`.
    pub fn bypasses_armor(self) -> bool {
        matches!(self, Damage::OnFire | Damage::InWall | Damage::Cramming | Damage::Fall | Damage::Drown | Damage::OutOfWorld)
    }

    pub fn name(self) -> &'static str {
        match self {
            Damage::Lava => "lava",
            Damage::OnFire => "on_fire",
            Damage::InWall => "in_wall",
            Damage::Cramming => "cramming",
            Damage::Fall => "fall",
            Damage::Drown => "drown",
            Damage::HotFloor => "hot_floor",
            Damage::Lightning => "lightning_bolt",
            Damage::InFire => "in_fire",
            Damage::MobAttack => "mob_attack",
            Damage::Arrow => "arrow",
            Damage::Spear => "spear",
            Damage::Trident => "trident",
            Damage::OutOfWorld => "out_of_world",
            Damage::Spit => "spit",
            Damage::Explosion => "explosion",
        }
    }

    /// The damage type's `message_id` (`DamageSource.getMsgId()`, what the bridge's frames print).
    pub fn msg_id(self) -> &'static str {
        match self {
            Damage::Lava => "lava",
            Damage::OnFire => "onFire",
            Damage::InWall => "inWall",
            Damage::Cramming => "cramming",
            Damage::Fall => "fall",
            Damage::Drown => "drown",
            Damage::HotFloor => "hotFloor",
            Damage::Lightning => "lightningBolt",
            Damage::InFire => "inFire",
            Damage::MobAttack => "mob",
            Damage::Arrow => "arrow",
            Damage::Spear => "spear",
            Damage::Trident => "trident",
            Damage::OutOfWorld => "outOfWorld",
            Damage::Spit => "mob",
            Damage::Explosion => "explosion",
        }
    }
}

/// What the mob layer records, in order (the judge's evidence).
#[derive(Clone, PartialEq, Debug)]
pub enum MobEvent {
    /// A villager ran `spawnGolemIfNeeded` with enough willing villagers: `golem` is the new
    /// golem's id, or `None` when none of the 10 tries found a spot.
    GolemTry { t: i64, villager: u32, willing: u32, golem: Option<u32>, at: Option<[i32; 3]> },
    /// A villager's golem sensor saw a golem (`golemDetected`).
    GolemSeen { t: i64, villager: u32 },
    /// (tickexact 2026-11-11, ironspec's golem-lock line) A successful summon's
    /// `nearbyVillagers.forEach(GolemSensor::golemDetected)`: `villager` (in the summoner's box
    /// inflated by 10) got `GOLEM_DETECTED_RECENTLY` for 599 gt from `summoner`'s `golem`.
    GolemLocked { t: i64, villager: u32, summoner: u32, golem: u32 },
    Hurt { t: i64, id: u32, ty: MobType, src: Damage, amount: f32 },
    Death { t: i64, id: u32, ty: MobType, src: Damage, pos: [f64; 3] },
    Drop { t: i64, from: u32, item: u32, what: &'static str, count: u32 },
    /// An item entity burnt up: lava or a fire took its health to 0 (`ItemEntity.hurtServer`,
    /// then `discard`), the drop gone within its tick; the judge's burnt drops (brief rev 9
    /// iteration 11: a measured loss, not a lost drop).
    Burnt { t: i64, item: u32, what: String, count: u32 },
    /// A mob picked an item entity up (`Mob.pickUpItem`: the whole stack into its main hand).
    PickedUp { t: i64, by: u32, item: u32, what: String, count: u32 },
    Removed { t: i64, id: u32 },
    Slept { t: i64, id: u32 },
    Woke { t: i64, id: u32 },
    /// A villager gave up a POI memory (`Villager.releasePoi`: its HOME, job site or meeting point
    /// unreachable too long: SetWalkTargetFromBlockMemory).
    Released { t: i64, id: u32 },
    /// A villager's active activity changed (`Brain.setActiveActivity`).
    Activity { t: i64, id: u32, activity: villager::Activity },
}

/// The block effects `checkInsideBlocks` collected for a mob this tick (`InsideBlockEffectApplier`
/// `StepBasedCollector`): one entry per step of the sweep that met something, in the order the
/// steps came (each step's effects then apply in `InsideBlockEffectType` order: a step's water
/// before a later step's lava leaves the mob burning, tickexact 2026-11-08).
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Inside {
    pub steps: Vec<InsideStep>,
    /// The step of the last effect (`advanceStep`: a new step starts a new entry).
    pub last_step: Option<i32>,
}

/// One step's effects.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct InsideStep {
    pub lava: bool,
    pub water: bool,
    /// mobinfra: a fire a bolt set (`BaseFireBlock.entityInside`: FIRE_IGNITE, then 1 `inFire`).
    pub fire: bool,
    /// The lava and fire cells of the step: `StepBasedCollector` keeps the type's effect once a
    /// step but every cell's `runAfter` action (`lavaHurt`, the `inFire` hurt), so a box over 4
    /// lava cells takes 4 hits in one tick (tickexact 2026-11-09: drop-blades-2's drops burnt in
    /// one tick in the game; a mob's invulnerability absorbs the extra hits, an item has none).
    pub lava_cells: u8,
    pub fire_cells: u8,
}

impl Inside {
    /// `StepBasedCollector.advanceStep(step)`.
    pub fn advance(&mut self, step: i32) {
        if self.last_step != Some(step) || self.steps.is_empty() {
            self.last_step = Some(step);
            self.steps.push(InsideStep::default());
        }
    }

    /// The current step's effects (a step started if none was).
    pub fn now(&mut self) -> &mut InsideStep {
        if self.steps.is_empty() {
            self.steps.push(InsideStep::default());
        }
        self.steps.last_mut().unwrap()
    }
}

/// A mob's state (the `LivingEntity` / `Mob` fields the rules read and write).
#[derive(Clone, PartialEq, Debug)]
pub struct MobState {
    pub ty: MobType,
    pub health: f32,
    /// `dead` (set by `die`); the body is removed 20 gt later (`death_time`).
    pub dead: bool,
    pub death_time: i32,
    pub hurt_time: i32,
    /// `damageCooldownTime` and `lastHurt`.
    pub damage_cooldown: i32,
    pub last_hurt: f32,
    /// `remainingFireTicks`: `-getFireImmuneTicks()`, 0 for a mob in 26.3, at creation, at load
    /// (`Entity.load` reads `Fire` with default 0) and after a fire.
    pub fire_ticks: i32,
    /// `airSupply` (300).
    pub air: i32,
    /// `Entity.moveDist` and `nextStep` (the step and swim sounds' clock; the mob-AI lane).
    pub move_dist: f32,
    pub next_step: f32,
    /// `crystalSoundIntensity` and `lastCrystalSoundPlayTick` (a step on an amethyst block's chime).
    pub crystal_intensity: f32,
    pub last_crystal_tick: i32,
    /// `getSleepingPos()`: sleeping (pose SLEEPING) when set.
    pub sleeping_pos: Option<BlockPos>,
    /// (tickexact 2026-11-11) The pose STANDING while the sleeping pos stays: `Entity.startRiding`
    /// sets the pose STANDING (and `refreshDimensions`: the standing box) without `stopSleeping`,
    /// so a sleeper a minecart picks up off its bed rides as tall as awake, its bed still occupied,
    /// asleep for `isSleeping` until its WakeUp (or another stopSleeping) puts it back on its feet
    /// by the bed. Cleared by start/stop sleeping; not saved (a sleeper loads with pose SLEEPING).
    pub pose_standing: bool,
    /// `FOLLOW_RANGE` with `Mob.finalizeSpawn`'s random bonus.
    pub follow_range: f64,
    /// (tickexact 2026-11-10) A zombie's `SPAWN_REINFORCEMENTS_CHANCE`: `randomizeReinforcementsChance`
    /// (nextDouble x 0.1F; 0 for the drowned and zombified piglins), a leader's bonus, the charges
    /// of its calls (-0.05 each) and of being one (-0.05); 0 where its spawn's draws are not modeled.
    pub reinforcement_chance: f64,
    /// (tickexact 2026-11-10) A leader zombie's MAX_HEALTH modifier (`LEADER_ZOMBIE_BONUS`,
    /// ADD_MULTIPLIED_TOTAL: `nextDouble() * 3 + 1`; 0: none): its max health is the base x (1 +
    /// this), which `setHealth` clamps to (a leader's fall damage left it at 20).
    pub max_health_bonus: f64,
    /// (mob-AI 2026-11-10) A zombie's KNOCKBACK_RESISTANCE modifier (`RANDOM_SPAWN_BONUS`,
    /// ADD_VALUE: `nextDouble() * 0.05F`; 0: none), on top of its type's.
    pub knockback_bonus: f64,
    /// (mob-AI 2026-11-11, for the viewer's models) `Mob.finalizeSpawn`'s
    /// `setLeftHanded(nextFloat() < 0.05F)`: which arm holds a skeleton's bow.
    pub left_handed: bool,
    /// (mob-AI 2026-11-11, for the viewer's models) A cow's, pig's or chicken's variant
    /// (`VariantUtils.selectVariantToSpawn`: "warm" or "cold" by the biome's
    /// `#spawns_warm/cold_variant_farm_animals`, else "temperate"); a natural zombie villager's
    /// type (`finalizeVillagerType`: `VillagerType.byBiome`, else "plains"). None: not drawn.
    pub variant: Option<&'static str>,
    /// `xxa`, `zza`, `jumping`, `speed` (the walking input).
    pub xxa: f32,
    pub zza: f32,
    pub jumping: bool,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub no_action_time: i32,
    /// `persistenceRequired` (a name tag, or picked-up loot).
    pub persistence_required: bool,
    /// `LivingEntity.onClimbable()` at its last tick (scaffolding refused; a spider's climbing).
    pub on_climbable: bool,
    /// The spider's climbing flag (`Spider.setClimbing(horizontalCollision)` after its tick).
    pub climbing: bool,
    /// The bat's resting flag (set at construction) and its flight target.
    pub bat_resting: bool,
    pub bat_target: Option<BlockPos>,
    /// The chicken's `eggTime`.
    pub egg_time: i32,
    /// `Chicken.isChickenJockey`: set by a baby zombie's jockey roll (it stays when the rider
    /// leaves): no eggs, and it despawns far away like a monster (`removeWhenFarAway`).
    pub chicken_jockey: bool,
    /// A lightning trap's skeleton horse: `SkeletonHorse.trapTime` (`isTrap`).
    pub trap_time: Option<i32>,
    /// Spawned by the world (natural spawning, a jockey's rider, a portal's piglin, a trap horse),
    /// not brought by a design: an unmodeled behaviour of it runs on the legacy model, named.
    pub natural: bool,
    /// `Mob.canPickUpLoot`: a natural undead's `setCanPickUpLoot(nextFloat() < 0.55 x the special
    /// multiplier)` in finalizeSpawn; a summon's `CanPickUpLoot` data (`ai::loot`).
    pub can_pick_up_loot: bool,
    /// The main hand (`EquipmentSlot.MAINHAND`): its weapon from `populateDefaultEquipmentSlots`,
    /// or an item it picked up (`ai::loot`); None: empty.
    pub mainhand: Option<Held>,
    /// The helmet on its head (`EquipmentSlot.HEAD`): its max damage and damage (natural undead
    /// from `populateDefaultEquipmentSlots`); it takes the sun's burn.
    pub helmet: Option<(i32, i32)>,
    /// (mob-AI 2026-11-10) The armor it wears (`populateDefaultEquipmentSlots`'s pieces, one
    /// material, and their spawn enchantments): its ARMOR, ARMOR_TOUGHNESS and enchantment
    /// protection. The helmet piece counts while [`Self::helmet`] holds.
    pub armor: Armor,
    /// A sheep's `DyeColor` id (0 white; a natural one's from `SheepColorSpawnRules`): its wool.
    pub color: u8,
    /// Its dimensions' scale (`getDefaultDimensions(pose).scale(..)`): a salmon's variant's
    /// `boundingBoxScale` (small 0.5, medium 1, large 1.5); 1 for every other mob.
    pub scale: f32,
    /// The lava tracker's height (`getFluidHeight(LAVA)`; `isInLava` when positive).
    pub lava_height: f64,
    /// This tick's collected block effects.
    pub inside: Inside,
    /// The last damage: (game time, source).
    pub last_damage: Option<(i64, Damage)>,
    /// Its potion effects (a witch drinks them).
    pub effects: Effects,
    /// A villager's brain.
    pub brain: Option<Box<villager::Brain>>,
    /// The walking input the judge gives a mob whose AI is not modeled.
    pub walk: walk::Walk,
    /// ---- mob AI (the mob-AI lane): the goal mob's AI state, when the world's AI is on ----
    pub ai: Option<Box<ai::MobAi>>,
}

impl MobState {
    /// A fresh mob of type `ty` (as `EntityType.create` + `finalizeSpawn` leave it; `bonus` is
    /// the follow-range multiplier's `triangle(0, 0.11485)` draw).
    pub fn new(ty: MobType, bonus: f64) -> MobState {
        let d = ty.def();
        MobState {
            ty,
            health: d.max_health,
            dead: false,
            death_time: 0,
            hurt_time: 0,
            damage_cooldown: 0,
            last_hurt: 0.0,
            // `remainingFireTicks = -getFireImmuneTicks()`: 0 for a mob in 26.3
            fire_ticks: 0,
            air: 300,
            move_dist: 0.0,
            next_step: 1.0,
            crystal_intensity: 0.0,
            last_crystal_tick: 0,
            sleeping_pos: None,
            pose_standing: false,
            follow_range: d.follow_range * (1.0 + bonus),
            reinforcement_chance: 0.0,
            max_health_bonus: 0.0,
            knockback_bonus: 0.0,
            left_handed: false,
            variant: None,
            xxa: 0.0,
            zza: 0.0,
            jumping: false,
            speed: d.speed as f32,
            no_jump_delay: 0,
            no_action_time: 0,
            persistence_required: false,
            on_climbable: false,
            climbing: false,
            bat_resting: d.id == "bat",
            bat_target: None,
            egg_time: 0,
            chicken_jockey: false,
            trap_time: None,
            natural: false,
            can_pick_up_loot: false,
            mainhand: None,
            helmet: None,
            armor: Armor::default(),
            color: 0,
            scale: 1.0,
            lava_height: 0.0,
            inside: Inside::default(),
            last_damage: None,
            effects: Effects::default(),
            brain: None,
            walk: walk::Walk::default(),
            ai: None,
        }
    }

    /// `getDimensions(pose)`: (width, height).
    pub fn dims(&self) -> (f32, f32) {
        if self.sleeping_pos.is_some() && !self.pose_standing {
            (types::SLEEPING.0, types::SLEEPING.1)
        } else {
            let d = self.ty.def();
            if self.scale != 1.0 {
                // EntityDimensions.scale(f): the width and height x f
                return (d.width * self.scale, d.height * self.scale);
            }
            (d.width, d.height)
        }
    }

    /// `getEyeHeight()` in its pose.
    pub fn eye(&self) -> f32 {
        if self.sleeping_pos.is_some() && !self.pose_standing { types::SLEEPING.2 } else { self.ty.def().eye * self.scale }
    }
}

/// A mob's potion effects (the ones a witch drinks; a spider's spawn effect): each its remaining
/// duration in ticks (0: none; `i32::MAX`: infinite, `MobEffectInstance(effect, -1)`); an instant
/// health waits for the next `tickEffects`. Every modeled effect has amplifier 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Effects {
    pub water_breathing: i32,
    pub fire_resistance: i32,
    pub speed: i32,
    pub instant_health: bool,
    /// (tickexact 2026-11-10) A spider's spawn effects (SpiderEffectsGroupData): strength (+3
    /// ATTACK_DAMAGE), regeneration (heals 1 every 50 ticks of its tickCount), invisibility
    /// (`getVisibilityPercent`).
    pub strength: i32,
    pub regeneration: i32,
    pub invisibility: i32,
}

/// The mob layer's level state.
#[derive(Clone, Debug)]
pub struct MobWorld {
    /// The seeded stand-in for the randoms the game draws that redsim cannot mirror (see the module
    /// doc). `None`: the mob layer is off (a mob in the world is a model error).
    pub rng: Option<LegacyRandom>,
    /// Day time = `day_offset + game time` (`ServerLevel.tickTime` advances both by one a tick
    /// while `advance_time` holds).
    pub day_offset: i64,
    /// Game rule `maxEntityCramming`.
    pub cramming: i32,
    /// The record (cleared by whoever reads it).
    pub events: Vec<MobEvent>,
    /// A golem spawn attempt's columns and outcome for the judge's spawn-spot statistics:
    /// (villager block, the cell each try ended at (`None`: no spot in the column), the spot it
    /// spawned at).
    pub spawn_tries: Vec<(BlockPos, Vec<Option<BlockPos>>, Option<BlockPos>)>,
    /// The walking input every new iron golem gets (the judge's robustness runs: [`walk::Walk`]).
    pub golem_walk: walk::Walk,
    /// Villagers spawn golems (`false`: the judge's golem exit flood, which adds its own golem).
    pub golem_spawning: bool,
    /// Natural spawning and despawning (the terrain lane: [`natural::Natural`]); `None`: off.
    pub natural: Option<natural::Natural>,
    /// (tickexact) The overworld's custom spawners ([`custom::Custom`]); `None`: the level runs none.
    pub custom: Option<custom::Custom>,
    /// ---- mob AI (the mob-AI lane): on/off, difficulty, players, game rules ----
    pub ai: ai::AiWorld,
    /// (tickexact 2026-11-11, ironspec's push rule: counted at the push itself) Every push a mob's
    /// `pushEntities` gives a minecart (`AbstractMinecart.push(mob)` past its 0.01 threshold; never
    /// the mob's own vehicle): `Some` logs them (whoever reads it drains it,
    /// [`crate::entity::minecart::take_cart_pushes`]), `None` (the default) logs nothing.
    pub cart_pushes: Option<Vec<CartPush>>,
    /// (cc767b78 2026-11-11, the riderless merge in a pressed queue, for tick-exact's bridge) Every
    /// minecart-on-minecart `push` (`AbstractMinecart.push(otherMinecart)`): `Some` logs them, with
    /// the alignment test and the motions before and after (whoever reads it drains it,
    /// [`crate::entity::minecart::take_cart_cart_pushes`]), `None` (the default) logs nothing.
    pub cart_cart_pushes: Option<Vec<CartCartPush>>,
}

/// One minecart-on-minecart push ([`MobWorld::cart_cart_pushes`]): cart `this` pushed by cart
/// `other` (`AbstractMinecart.push(other)`, then `pushOtherMinecart`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CartCartPush {
    pub t: i64,
    /// The pushed cart (`this`) and the pushing one (`other`).
    pub this: u32,
    pub other: u32,
    /// The horizontal distance between their centres.
    pub dist: f64,
    /// `|dir · facing|`, the alignment test (the push applies at 0.8 or more; NaN when not reached).
    pub dot: f64,
    /// What happened: "merged" (centres under 0.01 apart: no push, `dd < 1e-4`), "misaligned"
    /// (dot under 0.8: no push) or "applied".
    pub what: &'static str,
    /// The two motions (x, z) before and after: this, then other.
    pub before: [f64; 4],
    pub after: [f64; 4],
}

/// One push of a minecart by a mob ([`MobWorld::cart_pushes`]): `AbstractMinecart.push(mob)`'s
/// `this.push(-xa, 0, -za)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CartPush {
    pub t: i64,
    /// The pushing mob's id.
    pub mob: u32,
    /// The pushed cart's id.
    pub cart: u32,
    /// The motion added to the cart (x, z): 0.05 b/t away from the mob at 1 apart or more, 0.05 / d
    /// nearer (`pow` = min(1, 1 / d) on the unit direction, then 0.1 and 0.5).
    pub push: [f64; 2],
    /// The horizontal distance between their centres.
    pub dist: f64,
}

impl Default for MobWorld {
    fn default() -> MobWorld {
        MobWorld { rng: None, day_offset: 0, cramming: 24, events: Vec::new(), spawn_tries: Vec::new(), golem_walk: walk::Walk::Still, golem_spawning: true, natural: None, custom: None, ai: ai::AiWorld::default(), cart_pushes: None, cart_cart_pushes: None }
    }
}

impl MobWorld {
    /// The seeded random (a world with mobs must set one: [`MobWorld::seed`]).
    pub fn rng_mut(&mut self) -> &mut LegacyRandom {
        self.rng.get_or_insert_with(|| LegacyRandom::new(0x6d6f_6266_6172_6d))
    }

    /// Turn the mob layer on with this seed and day time at game time 0.
    pub fn seed(&mut self, seed: i64, day_time_at_zero: i64) {
        self.rng = Some(LegacyRandom::new(seed));
        self.day_offset = day_time_at_zero;
    }
}

impl World {
    /// The level's day time (`getDayTime`).
    pub fn day_time(&self) -> i64 {
        self.mob.day_offset + self.time
    }
}

/// The mob state of entity `i` (panics if it is not a mob).
pub fn mob(w: &World, i: usize) -> &MobState {
    match &w.entities.list[i].kind {
        EntityKind::Mob(m) => m,
        k => panic!("entity {i} is not a mob: {}", kind_name(k)),
    }
}

/// The mob state of entity `i`, mutable.
pub fn mob_mut(w: &mut World, i: usize) -> &mut MobState {
    match &mut w.entities.list[i].kind {
        EntityKind::Mob(m) => m,
        _ => panic!("entity {i} is not a mob"),
    }
}

/// A short name of an entity kind (for messages).
pub fn kind_name(k: &EntityKind) -> &'static str {
    match k {
        EntityKind::Boat(_) => "boat",
        EntityKind::Minecart(_) => "minecart",
        EntityKind::Item(_) => "item",
        EntityKind::ArmorStand(_) => "armor stand",
        EntityKind::PrimedTnt(_) => "primed TNT",
        EntityKind::Mob(m) => m.ty.def().id,
        EntityKind::Arrow(_) => "arrow",
    }
}

/// The entity core's spawn hook: a mob (`spawn`), or a boat or minecart carrying mobs (`summon
/// <vehicle> x y z {..,Passengers:[{id:"minecraft:zombie"}]}`: the vehicle through the core's own
/// spawn, then each passenger mounted). `None`: not for the mob layer.
pub fn spawn_hook(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>) -> Option<Result<u32, String>> {
    if let Some(r) = spawn(w, type_id, pos, nbt) {
        return Some(r);
    }
    let n = nbt?;
    let start = n.find("Passengers:[")?;
    let list_start = start + "Passengers:[".len();
    let mut depth = 1i32;
    let mut end = None;
    for (k, ch) in n[list_start..].char_indices() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(list_start + k);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else { return Some(Err(format!("{type_id}: unclosed Passengers list"))) };
    // (a raft by its id's end, the namespace stripped: "minecraft:" itself holds "raft", G1 on
    // ff000679f)
    if type_id.strip_prefix("minecraft:").unwrap_or(type_id).ends_with("_raft") {
        return Some(Err(format!("{type_id}: a raft's seat (0.5) is not modeled; use a wooden boat or a minecart")));
    }
    let list = &n[list_start..end];
    // the vehicle's own data without the Passengers entry
    let mut rest = format!("{}{}", &n[..start], &n[end + 1..]);
    rest = rest.replace("{,", "{").replace(",}", "}").replace(",,", ",");
    let rest = if rest.trim() == "{}" { None } else { Some(rest) };
    let vehicle = match crate::entity::spawn(w, type_id, pos, rest.as_deref()) {
        Ok(v) => v,
        Err(e) => return Some(Err(e)),
    };
    for item in split_compounds(list) {
        let Some(i) = item.find("id:") else { return Some(Err(format!("{type_id}: a passenger without an id: {item}"))) };
        let pid: String = item[i + 3..].trim_start().trim_start_matches('"').chars().take_while(|c| *c != '"' && *c != ',' && *c != '}').collect();
        // the passenger's own data without its id, through the same contract as a summoned mob
        // (`spawn`: PersistenceRequired and Rotation only; NoAI and anything else refused)
        let id_end = i + 3 + item[i + 3..].find(pid.as_str()).map(|k| k + pid.len()).unwrap_or(0);
        let id_end = if item[id_end..].starts_with('"') { id_end + 1 } else { id_end };
        let mut rest = format!("{}{}", &item[..i], &item[id_end..]);
        rest = rest.replace("{,", "{").replace(",}", "}").replace(",,", ",");
        let rest = if rest.trim() == "{}" { None } else { Some(rest) };
        if MobType::from_id(&pid).is_none() {
            return Some(Err(format!("{type_id}: passenger {pid} is not modeled")));
        }
        let rider = match spawn_inner(w, &pid, pos, rest.as_deref(), true) {
            Some(Ok(r)) => r,
            Some(Err(e)) => return Some(Err(format!("{type_id}'s passenger: {e}"))),
            None => return Some(Err(format!("{type_id}: passenger {pid} is not modeled"))),
        };
        if let Err(e) = mount(w, rider, vehicle) {
            return Some(Err(e));
        }
    }
    Some(Ok(vehicle))
}

/// The top-level `{...}` items of an SNBT list's inside.
fn split_compounds(list: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut cur) = (0i32, String::new());
    for ch in list.chars() {
        match ch {
            '{' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' => {
                depth -= 1;
                cur.push(ch);
                if depth == 0 {
                    out.push(std::mem::take(&mut cur).trim().to_string());
                }
            }
            ',' if depth == 0 => {}
            _ => {
                if depth > 0 {
                    cur.push(ch);
                }
            }
        }
    }
    out
}

/// (tickexact 2026-11-09) An entity's SNBT moved by `d` blocks: a loaded HOME memory's position
/// (`"minecraft:home":{value:{...,pos:[I;x,y,z]}}`, a design's `port home`) moves with it.
pub fn shift_home_nbt(nbt: &Option<String>, d: [i32; 3]) -> Option<String> {
    let n = nbt.as_ref()?;
    // every loaded HOME in the data (a vehicle's villager passengers each hold one)
    let mut out = String::new();
    let mut rest = n.as_str();
    while let Some(h) = rest.find("\"minecraft:home\"") {
        let Some(p) = rest[h..].find("pos:[I;").map(|q| h + q + "pos:[I;".len()) else { break };
        let Some(e) = rest[p..].find(']').map(|q| p + q) else { break };
        let v: Vec<i32> = rest[p..e].split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() != 3 {
            out.push_str(&rest[..e]);
            rest = &rest[e..];
            continue;
        }
        out.push_str(&rest[..p]);
        out.push_str(&format!("{},{},{}", v[0] + d[0], v[1] + d[1], v[2] + d[2]));
        rest = &rest[e..];
    }
    out.push_str(rest);
    Some(out)
}

/// The index of the bracket closing the one at `open` (`[` or `{`) in `t`, both kinds nested.
fn bracket_end(t: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, c) in t[open..].char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + k);
                }
            }
            _ => {}
        }
    }
    None
}

/// `getAttributeBaseValue(ATTACK_DAMAGE)` of a mob type ([`ai::attack::base_attack_damage`]).
fn base_attack_damage_of(ty: MobType) -> f32 {
    ai::attack::base_attack_damage_by_id(ty.def().id)
}

/// `summon <mob> x y z [nbt]` (the entity core's spawn hook): `Some` for a modeled mob type, else
/// `None` (the caller goes on). NBT is refused except `PersistenceRequired` and `Rotation` (the
/// yaw; a design turned by the judge carries one) and the keys below. The mob's own random and its
/// follow-range bonus come from [`MobWorld::rng`]: the judge models a mob the player brought in
/// (its `finalizeSpawn` ran where it spawned), not a `/summon` with data.
pub fn spawn(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>) -> Option<Result<u32, String>> {
    spawn_inner(w, type_id, pos, nbt, false)
}

/// [`spawn`], `seated` when the mob is a vehicle's passenger (a pillager is modeled only so).
fn spawn_inner(w: &mut World, type_id: &str, pos: [f64; 3], nbt: Option<&str>, seated: bool) -> Option<Result<u32, String>> {
    if !seated && MobType::from_id(type_id) == Some(MobType::Pillager) {
        return Some(Err(format!("{type_id}: a pillager on its own is not modeled (its AI): seat it in a boat")));
    }
    let mut ty = MobType::from_id(type_id)?;
    let mut persistent = false;
    let mut can_break_doors = false;
    // (mob-AI 2026-11-11) Creeper.readAdditionalSaveData: ignited, Fuse, ExplosionRadius
    let mut creeper_ignited = false;
    let mut creeper_fuse: Option<i32> = None;
    let mut creeper_radius: Option<i32> = None;
    let mut creeper_powered = false;
    let mut bat_flags = 0;
    let mut cube_size: Option<i32> = None;
    let mut animal_age: Option<i32> = None;
    let mut sheep_color: Option<u8> = None;
    let mut portal_cooldown: Option<i32> = None;
    let mut chicken_jockey = false;
    let mut can_pick_up_loot = false;
    let mut mainhand: Option<Held> = None;
    let mut egg_lay_time: Option<i32> = None;
    let mut yaw = None;
    // (tickexact 2026-11-09, ironspec's one-bed carousel) a villager's loaded memories: HOME (a
    // GlobalPos: the R1 double claim's shared bed, set for many villagers by the design file) and
    // LAST_SLEPT; `Villager.readAdditionalSaveData` builds its brain from them (no POI ticket taken)
    let mut home: Option<crate::pos::BlockPos> = None;
    let mut last_slept: Option<i64> = None;
    // (tickexact 2026-11-11) JOB_SITE, POTENTIAL_JOB_SITE and MEETING_POINT (GlobalPos, like HOME)
    let mut job_site: Option<crate::pos::BlockPos> = None;
    let mut potential_job_site: Option<crate::pos::BlockPos> = None;
    let mut meeting_point: Option<crate::pos::BlockPos> = None;
    // (tickexact 2026-11-10, the spider-effects scene) a loaded mob's effects (`active_effects`:
    // amplifier 0, a duration or -1 for infinite) and the one attribute modifier redsim models with
    // them (strength's ATTACK_DAMAGE +3, which a load restores from `attributes`, not the effect)
    let mut effects = Effects::default();
    let mut strength_modifier = false;
    let mut loaded_health: Option<f32> = None;
    if let Some(n) = nbt {
        let mut t = n.trim().to_string();
        if let Some(s) = t.find("active_effects:[") {
            let open = s + "active_effects:".len();
            let Some(e) = bracket_end(&t, open) else { return Some(Err(format!("entity {type_id}: unclosed active_effects in {n}"))) };
            for entry in t[open + 1..e].split('}').map(|x| x.trim_start_matches([',', ' ']).trim_start_matches('{')).filter(|x| !x.trim().is_empty()) {
                let (mut id, mut amp, mut dur) = (None, 0i32, None);
                for kv in entry.split(',').map(str::trim).filter(|x| !x.is_empty()) {
                    match kv.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                        Some(("id", v)) => id = Some(v.trim_matches('"').trim_start_matches("minecraft:").to_string()),
                        Some(("amplifier", v)) => amp = v.trim_end_matches(['b', 'B']).parse().unwrap_or(-1),
                        Some(("duration", v)) => dur = v.parse::<i32>().ok(),
                        Some(("ambient", "0b" | "false")) | Some(("show_particles", "1b" | "true")) | Some(("show_icon", "1b" | "true")) => {}
                        _ => return Some(Err(format!("entity {type_id}: an effect with {kv} is not modeled"))),
                    }
                }
                let d = match dur {
                    Some(-1) => i32::MAX,
                    Some(d) if d > 0 => d,
                    _ => return Some(Err(format!("entity {type_id}: an effect needs a duration (-1: infinite): {entry}"))),
                };
                if amp != 0 {
                    return Some(Err(format!("entity {type_id}: an effect of amplifier {amp} is not modeled")));
                }
                match id.as_deref() {
                    Some("speed") => effects.speed = d,
                    Some("strength") => effects.strength = d,
                    Some("regeneration") => effects.regeneration = d,
                    Some("invisibility") => effects.invisibility = d,
                    Some("water_breathing") => effects.water_breathing = d,
                    Some("fire_resistance") => effects.fire_resistance = d,
                    other => return Some(Err(format!("entity {type_id}: the effect {other:?} is not modeled"))),
                }
            }
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        if let Some(s) = t.find("attributes:[") {
            let open = s + "attributes:".len();
            let Some(e) = bracket_end(&t, open) else { return Some(Err(format!("entity {type_id}: unclosed attributes in {n}"))) };
            let body: String = t[open..=e].chars().filter(|c| !c.is_whitespace()).collect();
            let base = base_attack_damage_of(ty);
            let want = format!("[{{id:\"minecraft:attack_damage\",base:{base:?}d,modifiers:[{{id:\"minecraft:effect.strength\",amount:3.0d,operation:\"add_value\"}}]}}]");
            if body != want {
                return Some(Err(format!("entity {type_id}: only strength's ATTACK_DAMAGE modifier is modeled in attributes ({want}): {body}")));
            }
            strength_modifier = true;
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        if (effects.strength > 0) != strength_modifier {
            return Some(Err(format!("entity {type_id}: a loaded strength effect and its ATTACK_DAMAGE modifier go together (a load restores the modifier from attributes)")));
        }
        if let Some(s) = t.find("Brain:{") {
            // the compound to its matching brace
            let mut depth = 0;
            let mut end = None;
            for (k, c) in t[s..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(s + k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(e) = end else { return Some(Err(format!("entity {type_id}: unclosed Brain in {n}"))) };
            let brain = t[s..=e].to_string();
            if ty.def().id != "villager" {
                return Some(Err(format!("entity {type_id}: loaded Brain memories are modeled for villagers only")));
            }
            let mut known = 0;
            // (a GlobalPos memory: `"minecraft:<key>":{value:{dimension:"minecraft:overworld",pos:[I;x,y,z]}}`)
            let global = |key: &str| -> Option<Result<crate::pos::BlockPos, String>> {
                let h = brain.find(&format!("\"minecraft:{key}\""))?;
                let rest = &brain[h..];
                let end = rest.find("]}").map_or(rest.len(), |e| e + 2);
                let one = &rest[..end];
                let p = one.find("pos:[I;").map(|q| q + "pos:[I;".len());
                let v: Vec<i32> = p.and_then(|p| one[p..].split(']').next()).map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
                Some(if v.len() != 3 || !one.contains("minecraft:overworld") {
                    Err(format!("entity {type_id}: a {key} memory needs {{value:{{dimension:\"minecraft:overworld\",pos:[I;x,y,z]}}}}: {brain}"))
                } else {
                    Ok(crate::pos::BlockPos::new(v[0], v[1], v[2]))
                })
            };
            if let Some(h) = brain.find("\"minecraft:home\"") {
                let p = brain[h..].find("pos:[I;").map(|q| h + q + "pos:[I;".len());
                let v: Vec<i32> = p.and_then(|p| brain[p..].split(']').next()).map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
                if v.len() != 3 || !brain[h..].contains("minecraft:overworld") {
                    return Some(Err(format!("entity {type_id}: a HOME memory needs {{value:{{dimension:\"minecraft:overworld\",pos:[I;x,y,z]}}}}: {brain}")));
                }
                home = Some(crate::pos::BlockPos::new(v[0], v[1], v[2]));
                known += 1;
            }
            // (tickexact 2026-11-11, ironspec's job-site rows: a villager brought in still remembering
            // a job site, a potential one or a meeting point)
            for (key, slot) in [("job_site", &mut job_site), ("potential_job_site", &mut potential_job_site), ("meeting_point", &mut meeting_point)] {
                match global(key) {
                    Some(Ok(p)) => {
                        *slot = Some(p);
                        known += 1;
                    }
                    Some(Err(e)) => return Some(Err(e)),
                    None => {}
                }
            }
            if let Some(h) = brain.find("\"minecraft:last_slept\"") {
                let v = brain[h..].find("value:").and_then(|q| brain[h + q + 6..].split(['}', ',']).next()).and_then(|s| s.trim().trim_end_matches(['L', 'l']).parse::<i64>().ok());
                let Some(v) = v else { return Some(Err(format!("entity {type_id}: a LAST_SLEPT memory needs {{value:<game time>L}}: {brain}"))) };
                last_slept = Some(v);
                known += 1;
            }
            if brain.matches("\":{value:").count() != known {
                return Some(Err(format!("entity {type_id}: only HOME, LAST_SLEPT, JOB_SITE, POTENTIAL_JOB_SITE and MEETING_POINT memories are modeled when loaded: {brain}")));
            }
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        if let Some(s) = t.find("Rotation:[") {
            let Some(e) = t[s..].find(']').map(|e| s + e) else { return Some(Err(format!("entity {type_id}: unclosed Rotation in {n}"))) };
            let first = t[s + "Rotation:[".len()..e].split(',').next().unwrap_or("").trim().trim_end_matches(['f', 'F', 'd', 'D']).to_string();
            match first.parse::<f32>() {
                Ok(v) => yaw = Some(v),
                Err(_) => return Some(Err(format!("entity {type_id}: bad Rotation in {n}"))),
            }
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        // (mob AI) `equipment:{mainhand:{id:"..",count:N}}`: the main hand only (a skeleton's bow)
        if let Some(s) = t.find("equipment:{") {
            let mut depth = 0;
            let mut end = None;
            for (k, c) in t[s..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(s + k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(e) = end else { return Some(Err(format!("entity {type_id}: unclosed equipment in {n}"))) };
            let eq = t[s + "equipment:{".len()..e].to_string();
            let Some(body) = eq.trim().strip_prefix("mainhand:{").and_then(|b| b.strip_suffix('}')) else {
                return Some(Err(format!("entity {type_id}: only a main-hand item is modeled in equipment: {eq}")));
            };
            let mut id = None;
            let mut count = 1u8;
            for part in body.split(',') {
                match part.trim().split_once(':') {
                    Some(("id", v)) => id = Some(v.trim().trim_matches('"').to_string()),
                    Some(("count", v)) => count = v.trim().trim_end_matches(['b', 'B']).parse().unwrap_or(1),
                    _ => return Some(Err(format!("entity {type_id}: a main-hand item with {part} is not modeled"))),
                }
            }
            let Some(id) = id else { return Some(Err(format!("entity {type_id}: a main-hand item needs an id: {eq}"))) };
            let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
            mainhand = Some(Held { item: id, count, guaranteed: false, enchanted: false, ench: Vec::new() });
            t = format!("{}{}", &t[..s], &t[e + 1..]);
        }
        let inner = t.trim().strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or(&t).trim().to_string();
        for kv in inner.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match kv.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("PersistenceRequired", v)) => persistent = matches!(v, "1b" | "true" | "1"),
                // ---- mob AI (the mob-AI lane): a zombie that breaks doors ----
                Some(("CanBreakDoors", v)) if w.mob.ai.on && matches!(ty.def().id, "zombie" | "baby_zombie") => can_break_doors = matches!(v, "1b" | "true" | "1"),
                Some(("ignited", v)) if w.mob.ai.on && ty.def().id == "creeper" => creeper_ignited = matches!(v, "1b" | "true" | "1"),
                Some(("Fuse", v)) if w.mob.ai.on && ty.def().id == "creeper" => creeper_fuse = v.trim_end_matches(['s', 'S']).parse::<i32>().ok(),
                Some(("ExplosionRadius", v)) if w.mob.ai.on && ty.def().id == "creeper" => creeper_radius = v.trim_end_matches(['b', 'B']).parse::<i32>().ok(),
                Some(("powered", v)) if w.mob.ai.on && ty.def().id == "creeper" => creeper_powered = matches!(v, "1b" | "true" | "1"),
                // a bat's flags (resting: bit 1)
                Some(("BatFlags", v)) if ty.def().id == "bat" => bat_flags = v.trim_end_matches(['b', 'B']).parse::<i32>().unwrap_or(0),
                // a sulfur cube's size (`Size` = size - 1)
                Some(("Size", v)) if ty.def().game_id == "minecraft:sulfur_cube" => cube_size = v.trim_end_matches(['b', 'B']).parse::<i32>().ok().map(|s| s + 1),
                // a farm animal's age (a baby below 0)
                Some(("Age", v)) if ai::goal::animal::is_animal(ty.def().id) => animal_age = v.parse::<i32>().ok(),
                // a sheep's colour (`DyeColor` id)
                Some(("Color", v)) if ty.def().game_id == "minecraft:sheep" => sheep_color = v.trim_end_matches(['b', 'B']).parse::<u8>().ok(),
                // `Entity.load`'s portal cooldown
                Some(("PortalCooldown", v)) => portal_cooldown = v.parse::<i32>().ok(),
                // a chicken's jockey flag and egg timer (Chicken.readAdditionalSaveData)
                Some(("IsChickenJockey", v)) if ty.def().game_id == "minecraft:chicken" => chicken_jockey = matches!(v, "1b" | "true" | "1"),
                Some(("EggLayTime", v)) if ty.def().game_id == "minecraft:chicken" => egg_lay_time = v.parse::<i32>().ok(),
                // `Mob.readAdditionalSaveData`'s CanPickUpLoot (the mob AI's loot pickup)
                Some(("CanPickUpLoot", v)) if w.mob.ai.on => can_pick_up_loot = matches!(v, "1b" | "true" | "1"),
                // (tickexact 2026-11-10) `LivingEntity.readAdditionalSaveData`'s Health (setHealth:
                // clamped to the max health)
                Some(("Health", v)) => match v.trim_end_matches(['f', 'F']).parse::<f32>() {
                    Ok(h) => loaded_health = Some(h),
                    Err(_) => return Some(Err(format!("entity {type_id}: bad Health {v}"))),
                },
                _ => return Some(Err(format!("entity {type_id} with {kv} in its data is not modeled by redsim"))),
            }
        }
    }
    // a loaded sulfur cube of size 2 (an adult: no Age); size 1 loaded is an adult of the baby's
    // size, not modeled
    if ty.def().game_id == "minecraft:sulfur_cube" {
        match cube_size.unwrap_or(1) {
            2 => ty = MobType::from_id("sulfur_cube_2").expect("a row"),
            s => return Some(Err(format!("a summoned sulfur cube of size {s} (an adult of that size) is not modeled"))),
        }
    }
    // a farm animal loaded with a negative Age is its baby (a wolf's baby is not modeled)
    if let Some(a) = animal_age
        && a < 0
        && let Some(adult) = ai::goal::animal::adult_of(ty.def().id)
    {
        ty = match MobType::from_id(&format!("baby_{adult}")) {
            Some(t) => t,
            None => return Some(Err(format!("a summoned baby {adult} is not modeled"))),
        };
    }
    // a summon (the farm's own mob, a scene's): an unmodeled AI fails closed
    let strict = std::mem::replace(&mut w.mob.ai.strict, true);
    let id = add(w, ty, Vec3::new(pos[0], pos[1], pos[2]), persistent);
    w.mob.ai.strict = strict;
    if let Some(a) = animal_age
        && let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
        && let Some(x) = m.ai.as_deref_mut()
    {
        x.age = a;
    }
    if let Some(c) = sheep_color
        && let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
    {
        m.color = c & 15;
    }
    if can_pick_up_loot
        && let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
    {
        m.can_pick_up_loot = true;
    }
    // loaded effects (an invisible one is invisible from the sync before the next entity tick:
    // `updateDataBeforeSync`)
    if effects != Effects::default()
        && let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
    {
        m.effects = effects;
    }
    if let Some(h) = loaded_health
        && let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
    {
        m.health = h.clamp(0.0, crate::entity::mob::living::max_health(m));
    }
    // a loaded main hand (AbstractSkeleton.readAdditionalSaveData: reassessWeaponGoal)
    if let Some(j) = crate::entity::index_of(w, id) {
        if let Some(h) = mainhand
            && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
        {
            m.mainhand = Some(h);
        }
        ai::goal::skeleton::reassess_weapon_goal(w, j);
    }
    if let Some(c) = portal_cooldown
        && let Some(j) = crate::entity::index_of(w, id)
    {
        w.entities.list[j].portal_cooldown = c;
    }
    if (home.is_some() || last_slept.is_some() || job_site.is_some() || potential_job_site.is_some() || meeting_point.is_some())
        && let Some(j) = crate::entity::index_of(w, id)
    {
        // a loaded Brain builds the live brain again (`makeBrain(data)`): its 9 sensors draw their
        // start delays once more from the villager's own random (tickexact 2026-11-09, bed-shared:
        // the game's 9 more draws)
        let mut delays = [0i64; 9];
        if let Some(r) = w.entities.list[j].random.as_mut() {
            for (k, (_, rate)) in crate::entity::mob::villager::SENSORS.iter().enumerate() {
                delays[k] = r.next_int(*rate as i32) as i64;
            }
        }
        let fresh = w.entities.list[j].random.is_some();
        let day = w.day_time();
        if let EntityKind::Mob(m) = &mut w.entities.list[j].kind
            && let Some(b) = m.brain.as_deref_mut()
        {
            if fresh {
                b.sensors = delays;
            }
            if home.is_some() {
                b.home = home;
            }
            if last_slept.is_some() {
                b.last_slept = last_slept;
            }
            // (the job site, potential job site and meeting point live in the exact brain; its
            // registerBrainGoals' updateActivityFromSchedule then sees them: WORK with a JOB_SITE,
            // MEET with a MEETING_POINT, IDLE in their place otherwise)
            if job_site.is_some() || potential_job_site.is_some() || meeting_point.is_some() {
                match b.ex.as_deref_mut() {
                    Some(x) => {
                        x.mem.job_site = job_site;
                        x.mem.potential_job_site = potential_job_site;
                        x.mem.meeting_point = meeting_point;
                        let sched = ai::brain::scheduled(day);
                        x.active = match sched {
                            ai::brain::Act::Work if job_site.is_some() => ai::brain::Act::Work,
                            ai::brain::Act::Meet if meeting_point.is_some() => ai::brain::Act::Meet,
                            ai::brain::Act::Work | ai::brain::Act::Meet => ai::brain::Act::Idle,
                            a => a,
                        };
                        b.activity = x.active.simple();
                    }
                    None => w.errors.push(format!("entity {type_id}: a job site, potential job site or meeting point memory needs the mob AI on")),
                }
            }
        }
    }
    if let Some(j) = crate::entity::index_of(w, id)
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
    {
        if chicken_jockey {
            m.chicken_jockey = true;
        }
        if let Some(t) = egg_lay_time {
            m.egg_time = t;
        }
    }
    // a loaded bat rests only if its BatFlags say so (`readAdditionalSaveData`, default 0: a
    // summoned bat flies)
    if ty.def().id == "bat"
        && let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let EntityKind::Mob(m) = &mut e.kind
    {
        m.bat_resting = bat_flags & 1 != 0;
    }
    // a summoned mob faces its `Rotation`, else 0 (`Entity.load` reads it with the default
    // Vec2.ZERO): the constructor's random yaw is drawn and thrown away. Entity.load gives the
    // head and the body the loaded yaw, then ends with `setRot(getYRot(), getXRot())`, which
    // keeps the yaw's remainder by 360 (Java's: the dividend's sign), the head and the body as
    // loaded (tickexact 2026-11-09: a turned build's -450 is the game's -90 from its first tick)
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        let loaded = yaw.unwrap_or(0.0);
        e.y_rot = loaded % 360.0;
        e.y_rot_o = e.y_rot;
        // ---- mob AI (the mob-AI lane): Entity.load: the head and the body take the loaded yaw ----
        if let EntityKind::Mob(m) = &mut e.kind
            && let Some(a) = m.ai.as_deref_mut()
        {
            a.y_head_rot = loaded;
            a.y_body_rot = loaded;
        }
    }
    if can_break_doors
        && let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let EntityKind::Mob(m) = &mut e.kind
        && let Some(a) = m.ai.as_deref_mut()
    {
        ai::goal::zombie::set_can_break_doors(a, true);
    }
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let EntityKind::Mob(m) = &mut e.kind
        && let Some(a) = m.ai.as_deref_mut()
    {
        a.creeper_ignited |= creeper_ignited;
        a.creeper_powered |= creeper_powered;
        if let Some(f) = creeper_fuse {
            a.max_swell = f;
        }
        if let Some(r) = creeper_radius {
            a.explosion_radius = r;
        }
    }
    Some(Ok(id))
}

/// Add a mob of type `ty` at `pos` (`addFreshEntity`): its yaw, follow-range bonus and own random
/// drawn from [`MobWorld::rng`]; a villager gets its brain. Returns its id.
pub fn add(w: &mut World, ty: MobType, pos: Vec3, persistent: bool) -> u32 {
    // (a probe: MCTAS_TRACE_ADD=<id>: the call stack that creates mob #id)
    if std::env::var("MCTAS_TRACE_ADD").ok().and_then(|v| v.parse::<u32>().ok()) == Some(w.entities.next_id) {
        eprintln!("add t{}: #{} {:?}\n{}", w.time, w.entities.next_id, ty, std::backtrace::Backtrace::force_capture());
    }
    let (y_rot, bonus, seed) = {
        let r = w.mob.rng_mut();
        let y_rot = crate::entity::mth::wrap_degrees(r.next_float() * 360.0);
        let bonus = r.triangle(0.0, 0.11485000000000001);
        (y_rot, bonus, r.next_long())
    };
    let mut st = MobState::new(ty, bonus);
    st.persistence_required = persistent;
    // ---- mob AI (the mob-AI lane): `registerGoals`; `LivingEntity.speed` starts at 0 (a seated
    // pillager's goals are not modeled: it has none, and its own check errors when they'd act) ----
    if w.mob.ai.on && ty != MobType::Pillager {
        st.ai = Some(Box::new(ai::MobAi::new(ty, y_rot)));
        st.speed = 0.0;
    }
    let mut own = LegacyRandom::new(seed);
    // ---- the bridge's seeding in an exact world (`rng::entity_seed`, the mob-AI lane's): the
    // constructor's draws redone: the UUID, the LivingEntity yaw (`nextFloat() * (float)(PI * 2)`,
    // the head with it, the body at 0: a summon's data then sets all three, a natural spawn's snap
    // only the yaw) ----
    if let Some(s) = w.exact_seed {
        own = crate::rng::entity_random(s, w.entities.next_id);
        let ctor_yaw = own.next_float() * (std::f64::consts::PI * 2.0) as f32;
        if let Some(a) = st.ai.as_deref_mut() {
            a.y_head_rot = ctor_yaw;
            a.y_body_rot = 0.0;
            // a cube mob's move control takes its direction from the constructor's yaw
            if let Some(c) = a.cube.as_mut() {
                c.yrot = 180.0f32 * ctor_yaw / std::f32::consts::PI;
            }
        }
    }
    // the chicken's constructor: its egg timer (6000..11999)
    if ai::goal::animal::adult_of(ty.def().id) == Some("chicken") {
        st.egg_time = ai::goal::animal::chicken_egg_time(&mut own);
    }
    // the squid's constructor: its tentacles' speed
    if ai::squid::is_squid_id(ty.def().id) {
        let s = ai::squid::ctor_draw(&mut own);
        if let Some(q) = st.ai.as_deref_mut().and_then(|a| a.squid.as_mut()) {
            q.tentacle_speed = s;
        }
    }
    // a schooling fish's constructor (registerGoals): FollowFlockLeaderGoal's countdown
    if ai::fish::is_fish_id(ty.def().id) {
        let v = ai::fish::next_start_tick(&mut own);
        if let Some(a) = st.ai.as_deref_mut() {
            ai::fish::set_next_start_tick(a, v);
        }
    }
    // ---- mob AI (the mob-AI lane): a mob whose goals are not modeled (villagers have their brain,
    // bats their own flight): a summoned one (the farm's own: `MobAi::strict`) is refused, never
    // run without them; an ambient natural one runs on the legacy walk model, its type named in
    // the run's report (the parent's rule, 2026-11-08) ----
    if let Some(a) = st.ai.as_deref()
        && ai::unmodeled(ty, a)
    {
        if w.mob.ai.strict {
            let why = format!("the AI of {} (its goals) is not modeled", ty.game_id());
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        } else {
            st.ai = None;
            st.speed = ty.def().speed as f32;
            w.mob.ai.legacy.insert(ty.game_id());
        }
    }
    if ty == MobType::Villager {
        st.brain = Some(Box::new(villager::Brain::new(&mut own, w.time, w.day_time())));
        // ---- mob AI (the mob-AI lane): the exact brain, the live brain's sensors drawn again (its
        // shuffling lists keyed by the bridge's entity seed in an exact world, else by the stand-in
        // own random's seed: the judge's runs) ----
        if w.mob.ai.on
            && let Some(b) = st.brain.as_deref_mut()
        {
            let es = match w.exact_seed {
                Some(s) => crate::rng::entity_seed(s, w.entities.next_id),
                None => seed,
            };
            ai::brain::attach(b, &mut own, es, w.time, w.day_time());
        }
    }
    let id = w.entities.add(EntityKind::Mob(Box::new(st)), pos, y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.random = Some(own);
    }
    id
}

/// One mob's tick (the entity core's dispatch hook).
pub fn tick(w: &mut World, i: usize) {
    living::tick(w, i);
}

/// (tickexact) In an exact world, a mob that just entered the level (`addFreshEntity`) gets the
/// bridge's entering random: its number's seed ([`World::entering_random`]) with its constructor's
/// draws replayed (the UUID's two longs, `LivingEntity`'s yaw). A mob with brain sensors replays
/// their `nextInt(scanRate)` too (none of the natural types redsim spawns has them). Nothing
/// without an exact world (the judge's stand-in stays).
/// A natural mob's helmet from its `finalizeSpawn` (`armor`: 0 leather, 1 gold, 2 chain, 3 iron,
/// 4 diamond; the helmet's durability: 55, 77, 165, 165, 363).
pub fn set_helmet(w: &mut World, id: u32, armor: Option<u8>) {
    let Some(a) = armor else { return };
    // `getEquipmentForSlot(HEAD, type)`: 0 leather, 1 copper, 2 golden, 3 chainmail, 4 iron, 5
    // diamond; a helmet's durability is 11 x its material's (5, 11, 7, 15, 15, 33)
    let max = match a {
        0 => 55,
        1 => 121,
        2 => 77,
        3 | 4 => 165,
        _ => 363,
    };
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let crate::entity::EntityKind::Mob(m) = &mut e.kind
    {
        m.helmet = Some((max, 0));
    }
}

/// (mob-AI 2026-11-10) The armor `populateDefaultEquipmentSlots` dressed mob `id` in (none: no
/// change).
pub fn set_armor(w: &mut World, id: u32, armor: &Armor) {
    if armor.pieces == 0 {
        return;
    }
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let crate::entity::EntityKind::Mob(m) = &mut e.kind
    {
        m.armor = armor.clone();
    }
}

/// (mob-AI 2026-11-10) A mob's worn armor: `Mob.populateDefaultEquipmentSlots`'s pieces from the
/// head down in one material (`getEquipmentForSlot`: 0 leather, 1 copper, 2 golden, 3 chainmail,
/// 4 iron, 5 diamond), each piece's `populateDefaultEquipmentEnchantments` enchantments. Mob
/// armor never wears from hits (`LivingEntity.hurtArmor` does nothing for them); only the sun
/// wears the helmet (`MobState::helmet`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Armor {
    /// Pieces worn: 0 none, 1 a helmet, 2 and a chestplate, 3 and leggings, 4 and boots.
    pub pieces: u8,
    pub material: u8,
    /// Each piece's enchantments (bare id, level), by slot: 0 head, 1 chest, 2 legs, 3 feet.
    pub ench: [Vec<(&'static str, i32)>; 4],
}

impl Armor {
    /// `ArmorMaterials`' defense of the piece in slot `s` (0 head, 1 chest, 2 legs, 3 feet).
    fn defense(&self, s: usize) -> f32 {
        // (head, chest, legs, feet) of leather, copper, golden, chainmail, iron, diamond
        const D: [[f32; 4]; 6] = [[1.0, 3.0, 2.0, 1.0], [2.0, 4.0, 3.0, 1.0], [2.0, 5.0, 3.0, 1.0], [2.0, 5.0, 4.0, 1.0], [2.0, 6.0, 5.0, 2.0], [3.0, 8.0, 6.0, 3.0]];
        D[(self.material as usize).min(5)][s]
    }

    /// The slots worn now: the helmet's while it holds (`helmet`: the sun's wear), the others'.
    fn worn(&self, helmet: bool) -> impl Iterator<Item = usize> + '_ {
        (0..self.pieces as usize).filter(move |&s| s != 0 || helmet)
    }

    /// The ARMOR the pieces add.
    pub fn value(&self, helmet: bool) -> f32 {
        self.worn(helmet).map(|s| self.defense(s)).sum()
    }

    /// The ARMOR_TOUGHNESS the pieces add (a diamond piece's 2).
    pub fn toughness(&self, helmet: bool) -> f32 {
        if self.material == 5 { self.worn(helmet).count() as f32 * 2.0 } else { 0.0 }
    }

    /// `EnchantmentHelper.getDamageProtection`: the worn pieces' `damage_protection` against
    /// `src` (protection 1 a level, fire/blast/projectile protection 2, feather falling 3).
    pub fn protection(&self, helmet: bool, src: Damage) -> f32 {
        let mut p = 0.0f32;
        for s in self.worn(helmet) {
            for &(id, level) in &self.ench[s] {
                let l = level as f32;
                p += match id {
                    "protection" => l,
                    "fire_protection" if src.is_fire() => 2.0 * l,
                    "projectile_protection" if src.by_projectile() => 2.0 * l,
                    "feather_falling" if src == Damage::Fall => 3.0 * l,
                    // (mob-AI 2026-11-11) blast protection's damage_protection on #is_explosion
                    "blast_protection" if src == Damage::Explosion => 2.0 * l,
                    _ => 0.0,
                };
            }
        }
        p
    }

    /// Whether a worn piece carries enchantment `id`.
    pub fn has(&self, helmet: bool, id: &str) -> bool {
        self.worn(helmet).any(|s| self.ench[s].iter().any(|&(e, _)| e == id))
    }

    /// The `attributes` effect's modifier sum of enchantment `id` over the worn pieces, each
    /// `LevelBasedValue.linear(base, base)` at its level (a float, as the modifier's amount).
    fn linear_sum(&self, helmet: bool, id: &str, base: f32) -> f64 {
        let mut sum = 0.0f64;
        for s in self.worn(helmet) {
            for &(e, level) in &self.ench[s] {
                if e == id {
                    sum += (base + base * (level - 1) as f32) as f64;
                }
            }
        }
        sum
    }

    /// WATER_MOVEMENT_EFFICIENCY: depth strider's 0.33333334F a level (ADD_VALUE on 0).
    pub fn water_movement_efficiency(&self, helmet: bool) -> f64 {
        self.linear_sum(helmet, "depth_strider", 0.33333334)
    }

    /// BURNING_TIME: fire protection's -0.15F a level (ADD_MULTIPLIED_BASE on 1).
    pub fn burning_time(&self, helmet: bool) -> f64 {
        1.0 + self.linear_sum(helmet, "fire_protection", -0.15)
    }

    /// (mob-AI 2026-11-11) EXPLOSION_KNOCKBACK_RESISTANCE: blast protection's 0.15F a level
    /// (ADD_VALUE on 0), clamped to the attribute's 0..1.
    pub fn explosion_knockback_resistance(&self, helmet: bool) -> f64 {
        self.linear_sum(helmet, "blast_protection", 0.15).clamp(0.0, 1.0)
    }
}

/// `LivingEntity.igniteForTicks(ticks)`'s length: `Mth.ceil(ticks x BURNING_TIME)` (mob `i`'s
/// fire protection shortens it).
pub fn ignite_ticks(w: &World, i: usize, ticks: i32) -> i32 {
    let m = mob(w, i);
    let bt = m.armor.burning_time(m.helmet.is_some());
    if bt == 1.0 { ticks } else { (ticks as f64 * bt).ceil() as i32 }
}


/// An item in a mob's main hand: its id and count, whether it drops for sure when the mob dies
/// (`setGuaranteedDrop`: an item it picked up; a spawn weapon's 0.085 drops only on a player's
/// kill), whether it is enchanted (a spawn weapon's `populateDefaultEquipmentEnchantments`).
/// (mob-AI 2026-11-10) Its enchantments (bare id, level). A mob's hits never wear it
/// (`Mob.doHurtTarget` calls only `ItemStack.hurtEnemy`; the wear is `postHurtEnemy`'s, a
/// player's).
#[derive(Clone, PartialEq, Debug)]
pub struct Held {
    pub item: String,
    pub count: u8,
    pub guaranteed: bool,
    pub enchanted: bool,
    pub ench: Vec<(&'static str, i32)>,
}

impl Held {
    /// The level of enchantment `id` on it (0: none).
    pub fn level(&self, id: &str) -> i32 {
        self.ench.iter().find(|&&(e, _)| e == id).map_or(0, |&(_, l)| l)
    }
}

/// finalizeSpawn's hand of a natural mob: `setCanPickUpLoot`'s roll and the main-hand weapon
/// `populateDefaultEquipmentSlots` gave it (and its enchantments).
pub fn set_hand(w: &mut World, id: u32, loot: bool, weapon: Option<(&'static str, bool)>, ench: &[(&'static str, i32)]) {
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id)
        && let crate::entity::EntityKind::Mob(m) = &mut e.kind
    {
        m.can_pick_up_loot = loot;
        m.mainhand = weapon.map(|(item, enchanted)| Held { item: item.to_string(), count: 1, guaranteed: false, enchanted, ench: ench.to_vec() });
    }
    // setItemSlot -> onEquipItem: a skeleton reassesses its weapon goal (its bow)
    if let Some(j) = crate::entity::index_of(w, id) {
        ai::goal::skeleton::reassess_weapon_goal(w, j);
    }
}

/// Returns the constructor's yaw draw (`setYRot(nextFloat() x 2 pi)`: the head keeps it until the
/// mob's first turn), in an exact world.
pub fn seed_entered(w: &mut World, id: u32) -> Option<f32> {
    if w.exact_seed.is_none() {
        return None;
    }
    let mut own = w.entering_random(id);
    crate::rng::replay_uuid(&mut own);
    // the bridge's replayConstructor: the LivingEntity yaw, then a type's own constructor draws
    // (a cube mob's move control keeps the yaw as degrees; a chicken's egg timer, a squid's
    // tentacle speed)
    let yaw = own.next_float() * (std::f64::consts::PI * 2.0) as f32;
    let ty = w.entities.list.iter().rev().find(|e| e.id == id).and_then(|e| match &e.kind {
        crate::entity::EntityKind::Mob(m) => Some(m.ty),
        _ => None,
    });
    let egg = ty.filter(|t| ai::goal::animal::adult_of(t.def().id) == Some("chicken")).map(|_| ai::goal::animal::chicken_egg_time(&mut own));
    let tentacle = ty.filter(|t| ai::squid::is_squid_id(t.def().id)).map(|_| ai::squid::ctor_draw(&mut own));
    // a schooling fish's: FollowFlockLeaderGoal's countdown (registerGoals in Mob's constructor)
    let school = ty.filter(|t| ai::fish::is_fish_id(t.def().id)).map(|_| ai::fish::next_start_tick(&mut own));
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.random = Some(own);
        if let crate::entity::EntityKind::Mob(m) = &mut e.kind {
            if let Some(t) = egg {
                m.egg_time = t;
            }
            if let Some(a) = m.ai.as_deref_mut() {
                if let Some(c) = a.cube.as_mut() {
                    c.yrot = 180.0f32 * yaw / std::f32::consts::PI;
                }
                if let (Some(q), Some(s)) = (a.squid.as_mut(), tentacle) {
                    q.tentacle_speed = s;
                }
                if let Some(v) = school {
                    ai::fish::set_next_start_tick(a, v);
                }
            }
        }
    }
    Some(yaw)
}

/// `Entity.applyMovementEmissionAndPlaySound` for a mob that is no passenger (the mob-AI lane):
/// `moveDist` grows by the horizontal move x 0.6 (climbing is refused elsewhere); past `nextStep`
/// over a block that is not air, a mob on the ground steps (its step sounds draw nothing) and one
/// in water swims: `playSwimSound`'s pitch draws two floats from its own random.
/// `Entity.isStateClimbable`: #climbable (ladders, vines, scaffolding, the nether and cave vines)
/// or powder snow.
fn is_state_climbable(b: &crate::block::Block) -> bool {
    match b {
        crate::block::Block::Scaffolding { .. } | crate::block::Block::PowderSnow => true,
        crate::block::Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(*id);
            p.has_tag("minecraft:climbable") || p.block_id() == "minecraft:powder_snow"
        }
        _ => false,
    }
}

pub fn movement_emission(w: &mut World, i: usize, movement: Vec3) {
    // (mob-AI 2026-11-10) applyMovementEmissionAndPlaySound: the whole move (x 0.6) counts on a
    // climbable supporting block, the sideways part elsewhere (forest build 3 t12500..t13750: a
    // wolf through a vine counted 0.364 more)
    let moved = (movement.length() * 0.6f32 as f64) as f32;
    let moved_sideways = (movement.horizontal_distance() * 0.6f32 as f64) as f32;
    let e = &w.entities.list[i];
    let supporting = crate::entity::collide::get_on_pos(w, e, 1.0e-5);
    let supporting_state = w.get(supporting);
    let climbing = is_state_climbable(&supporting_state);
    let m = mob_mut(w, i);
    m.move_dist += if climbing { moved } else { moved_sideways };
    if !(m.move_dist > m.next_step) || supporting_state.is_air() {
        return;
    }
    let e = &w.entities.list[i];
    let (on_ground, in_water) = (e.on_ground, e.was_touching_water);
    // vibrationAndSoundEffectsFromBlock: a block that is not air, `(onGround || climbable) &&
    // !isSwimming()` (a swimming drowned on the bottom makes the swim sound instead:
    // cross-4-underground t12882); first on the effect cell (`getOnPosLegacy`: 0.2 down) with the
    // sound, then on the supporting cell when it is another (no sound)
    let swimming = mob(w, i).ai.as_deref().is_some_and(|a| a.swimming);
    let effect = crate::entity::collide::get_on_pos(w, e, 0.2);
    let effect_state = w.get(effect);
    let steps_on = |s: &crate::block::Block| !s.is_air() && (on_ground || is_state_climbable(s)) && !swimming;
    let mut produced = false;
    if steps_on(&effect_state) {
        produced = true;
        // walkingStepSound on the effect cell for a mob whose steps sound: an amethyst block
        // (`#crystal_sound_blocks`) 20 ticks after the last chime draws the chime's pitch from
        // its own random
        let tick = e.tick_count;
        let sounds = !matches!(mob(w, i).ty.def().id, "bat" | "squid" | "glow_squid" | "baby_squid" | "baby_glow_squid");
        if sounds && tick >= mob(w, i).last_crystal_tick + 20 {
            let crystal = match &effect_state {
                crate::block::Block::Terrain(id) => crate::terrain::blocks::props(*id).has_tag("minecraft:crystal_sound_blocks"),
                crate::block::Block::Air => false,
                b => crate::terrain::blocks::id_of(&b.to_string()).is_some_and(|id| crate::terrain::blocks::props(id).has_tag("minecraft:crystal_sound_blocks")),
            };
            if crystal {
                let m = mob_mut(w, i);
                let elapsed = tick - m.last_crystal_tick;
                m.crystal_intensity *= 0.997f64.powi(elapsed) as f32;
                m.crystal_intensity = (m.crystal_intensity + 0.07).min(1.0);
                m.last_crystal_tick = tick;
                if let Some(r) = w.entities.list[i].random.as_mut() {
                    r.next_float();
                }
            }
        }
    }
    if supporting != effect && steps_on(&supporting_state) {
        produced = true;
    }
    if produced {
        let m = mob_mut(w, i);
        m.next_step = (m.move_dist as i32 + 1) as f32;
    } else if in_water {
        let m = mob_mut(w, i);
        m.next_step = (m.move_dist as i32 + 1) as f32;
        // a bat's and a squid's movement emit events only (`getMovementEmission`: EVENTS, no swim
        // sound)
        if !matches!(mob(w, i).ty.def().id, "bat" | "squid" | "glow_squid" | "baby_squid" | "baby_glow_squid")
            && let Some(r) = w.entities.list[i].random.as_mut()
        {
            r.next_float();
            r.next_float();
        }
    }
}

/// `maxUpStep()` (`Attributes.STEP_HEIGHT`).
pub fn max_up_step(m: &MobState) -> f32 {
    m.ty.def().step
}

/// `getGravity()` (living: `Attributes.GRAVITY` 0.08).
pub fn gravity() -> f64 {
    0.08
}

/// `getAirDrag()`: `computeModifiedFriction(0.98F, 1)` (not an omnidirectional air mover).
pub fn air_drag() -> f64 {
    (1.0f32 - (1.0f32 - 0.98f32) * 1.0f32).clamp(0.0, 1.0) as f64
}

/// The y of a mob's seat on a vehicle of kind `v` above the vehicle's position: the vehicle's
/// passenger attachment (a minecart's 0.1875; a wooden boat's `rideHeight` = height / 3 = 0.1875;
/// rafts, whose seat is height x 0.8888889 = 0.5, are refused when mounting: `BoatState` does not
/// tell them apart) minus the rider's vehicle attachment (`ridingOffset`: a zombie sits 0.7 lower).
pub fn rider_offset(v: &EntityKind, rider: &MobState) -> Option<f64> {
    rider_seat(v, rider).map(|(seat, ro)| seat + ro)
}

/// [`rider_offset`]'s two parts: the vehicle's passenger attachment's y and the rider's
/// `ridingOffset` (its vehicle attachment, negated). `Entity.positionRider` takes the vehicle's
/// position plus the first, then minus the rider's attachment: `(y + seat) + ridingOffset` in that
/// order (tickexact 2026-11-10: `y + (seat + ridingOffset)` rounded a chicken jockey's baby zombie
/// 2 ulps off, d1u-deep t11517).
pub fn rider_seat(v: &EntityKind, rider: &MobState) -> Option<(f64, f64)> {
    // (floats where the game's attachment is a float, a double where it is one: the chicken's Vec3)
    let seat: f64 = match v {
        // `AbstractMinecart.getPassengerAttachmentPoint`: a villager (or a wandering trader) sits
        // lowered, at the cart's own y (`LOWERED_PASSENGER_ATTACHMENT`, 0)
        EntityKind::Minecart(_) if matches!(rider.ty, MobType::Villager) => 0.0,
        EntityKind::Minecart(_) => 0.1875f32 as f64,
        EntityKind::Boat(_) => (0.5625f32 / 3.0f32) as f64,
        // a spider's passenger attachment (`passengerAttachments(0.765F)`: straight up, the same at
        // every yaw)
        EntityKind::Mob(m) if m.ty.def().id == "spider" => 0.765f32 as f64,
        // an adult horse's (`passengerAttachments`; its stand shift: `ride_tick`)
        EntityKind::Mob(m) if matches!(m.ty.def().id, "zombie_horse" | "skeleton_horse") => 1.31875f32 as f64,
        // a baby zombie horse's (ZombieHorse.BABY_DIMENSIONS: height - 0.25, then x 0.7)
        EntityKind::Mob(m) if m.ty.def().id == "baby_zombie_horse" => ((1.6f32 - 0.25f32) * 0.7f32) as f64,
        EntityKind::Mob(m) if m.ty.def().id == "horse" => 1.44375f32 as f64,
        EntityKind::Mob(m) if m.ty.def().id == "donkey" => 1.1125f32 as f64,
        // a chicken's new Vec3(0, 0.7, -0.1) (its -0.1 turned with its yaw: `ride_tick`); a
        // baby's 0.375F
        EntityKind::Mob(m) if m.ty.def().id == "chicken" => 0.7,
        EntityKind::Mob(m) if m.ty.def().id == "baby_chicken" => 0.375f32 as f64,
        _ => return None,
    };
    Some((seat, rider.ty.def().riding_offset as f64))
}

/// `Entity.startRiding(vehicle)` as `summon <vehicle> .. {Passengers:[{id:<mob>}]}` sets it up:
/// the mob joins the vehicle's passengers and takes its seat at the vehicle's next tick.
pub fn mount(w: &mut World, rider: u32, vehicle: u32) -> Result<(), String> {
    let (Some(pi), Some(vi)) = (crate::entity::index_of(w, rider), crate::entity::index_of(w, vehicle)) else { return Err(format!("mount: no entity {rider} or {vehicle}")) };
    if !matches!(w.entities.list[pi].kind, EntityKind::Mob(_)) || rider_offset(&w.entities.list[vi].kind, mob(w, pi)).is_none() {
        return Err("mount: a mob in a boat or minecart only".into());
    }
    if !w.entities.list[vi].passengers.is_empty() || w.entities.list[pi].vehicle.is_some() {
        return Err("mount: one passenger per vehicle".into());
    }
    w.entities.list[pi].vehicle = Some(vehicle);
    w.entities.list[vi].passengers.push(rider);
    let off = rider_offset(&w.entities.list[vi].kind, mob(w, pi)).unwrap_or(0.0);
    let vp = w.entities.list[vi].pos;
    w.entities.set_pos(pi, vp.add(0.0, off, 0.0));
    Ok(())
}

/// `AbstractBoat.getPassengerAttachmentPoint`'s horizontal part for rider `j` of boat `vi`: the
/// single passenger's offset (`getSinglePassengerXOffset`: a chest boat's 0.15, a boat's 0), with
/// two passengers the first 0.2 forward and the second 0.6 back (an `Animal` 0.2 further forward),
/// as `Vec3(0, y, f).yRot(-yRot * PI / 180)` turns it (the float tables, widened).
fn boat_seat_offset(w: &World, vi: usize, j: usize) -> (f64, f64) {
    let v = &w.entities.list[vi];
    let EntityKind::Boat(b) = &v.kind else { return (0.0, 0.0) };
    let mut f = if b.chest { 0.15f32 } else { 0.0f32 };
    if v.passengers.len() > 1 {
        let idx = v.passengers.iter().position(|&p| p == w.entities.list[j].id).unwrap_or(0);
        f = if idx == 0 { 0.2 } else { -0.6 };
        if java_animal(mob(w, j).ty.def().id) {
            f += 0.2;
        }
    }
    if f == 0.0 {
        return (0.0, 0.0);
    }
    let a = -v.y_rot * (std::f32::consts::PI / 180.0);
    let (c, s) = (crate::entity::mth::cos(a as f64), crate::entity::mth::sin(a as f64));
    (f as f64 * s as f64, f as f64 * c as f64)
}

/// Is boat `vi` a chest boat (one seat)?
fn b_chest(w: &World, vi: usize) -> bool {
    matches!(&w.entities.list[vi].kind, EntityKind::Boat(b) if b.chest)
}

/// Java's `Animal` subclasses among the modeled mob types (the boat's seat and sideways turn):
/// the farm animals, the horses (an `AbstractHorse`), the wolf and cat (`TamableAnimal`), the
/// rabbit, fox, turtle and bee, and their babies.
pub fn java_animal(id: &str) -> bool {
    matches!(id.strip_prefix("baby_").unwrap_or(id), "sheep" | "pig" | "chicken" | "cow" | "horse" | "donkey" | "zombie_horse" | "wolf" | "rabbit" | "fox" | "turtle" | "cat" | "bee")
}

/// `rideTick` of a riding mob: its motion zeroed, its own tick, then its seat
/// (`positionRider`), its fall distance reset (`LivingEntity.rideTick`).
pub fn ride_tick(w: &mut World, j: usize) {
    w.entities.list[j].delta_movement = Vec3::ZERO;
    living::tick(w, j);
    if let Some(v) = w.entities.list[j].vehicle
        && let Some(vi) = crate::entity::index_of(w, v)
        && let Some((seat, ro)) = rider_seat(&w.entities.list[vi].kind, mob(w, j))
    {
        let vp = w.entities.list[vi].pos;
        let (dx, dz) = boat_seat_offset(w, vi, j);
        // a horse's stand shift (AbstractHorse.getPassengerAttachmentPoint); an adult chicken's
        // -0.1 behind, turned by its yaw (EntityAttachments.get: yRot(-yRot))
        let vid = match &w.entities.list[vi].kind {
            EntityKind::Mob(vm) => vm.ty.def().id,
            _ => "",
        };
        let shift = if ai::goal::animal::is_horse(vid) {
            ai::goal::animal::horse_seat_shift(w, vi)
        } else if vid == "chicken" {
            let a = -w.entities.list[vi].y_rot * (std::f32::consts::PI / 180.0);
            let (c, s) = (crate::entity::mth::cos(a as f64) as f64, crate::entity::mth::sin(a as f64) as f64);
            Vec3::new(-0.1 * s, 0.0, -0.1 * c)
        } else {
            Vec3::ZERO
        };
        // getPassengerRidingPosition (the position plus the attachment), minus the rider's own
        // vehicle attachment (0, -ridingOffset, 0)
        let at = vp.add(dx + shift.x, seat + shift.y, dz + shift.z);
        w.entities.set_pos(j, Vec3::new(at.x, at.y + ro, at.z));
        // AbstractHorse.positionRider / Chicken.positionRider: the rider's body takes the vehicle's
        if ai::goal::animal::is_horse(vid) || matches!(vid, "chicken" | "baby_chicken") {
            if let Some(vb) = mob(w, vi).ai.as_deref().map(|a| a.y_body_rot)
                && let Some(a) = mob_mut(w, j).ai.as_deref_mut()
            {
                a.y_body_rot = vb;
            }
        }
        // AbstractSkeleton.rideTick / Drowned.rideTick: the body takes the controlled vehicle's
        let id = mob(w, j).ty.def().id;
        if matches!(id, "skeleton" | "stray" | "bogged" | "drowned" | "baby_drowned")
            && ai::controlled_vehicle(w, j) == Some(vi)
            && let Some(vb) = mob(w, vi).ai.as_deref().map(|a| a.y_body_rot)
            && let Some(a) = mob_mut(w, j).ai.as_deref_mut()
        {
            a.y_body_rot = vb;
        }
        // `AbstractBoat.positionRider` (the mob-AI lane): the rider turns with the boat
        // (deltaRotation), then `clampRotation`: a mob's body and yaw take the boat's yaw and its
        // head comes within `getMaxHeadYRot` of the body (`clampHeadRotationToBody`)
        if let EntityKind::Boat(b) = &w.entities.list[vi].kind {
            let dr = b.delta_rotation;
            let boat_yaw = w.entities.list[vi].y_rot;
            let limit = ai::max_head_y_rot(w, j) as f32;
            w.entities.list[j].y_rot = boat_yaw;
            if let Some(a) = mob_mut(w, j).ai.as_deref_mut() {
                let head = a.y_head_rot + dr;
                a.y_body_rot = boat_yaw;
                let delta = crate::entity::mth::wrap_degrees(a.y_body_rot - head);
                let target = delta.clamp(-limit, limit);
                a.y_head_rot = head + delta - target;
            }
            // an Animal in a full boat sits sideways (90 or 270 by its id's parity)
            let full = w.entities.list[vi].passengers.len() == if b_chest(w, vi) { 1 } else { 2 };
            if full && java_animal(mob(w, j).ty.def().id) {
                let turn = if w.entities.list[j].id % 2 == 0 { 90.0f32 } else { 270.0f32 };
                if let Some(a) = mob_mut(w, j).ai.as_deref_mut() {
                    a.y_body_rot += turn;
                    a.y_head_rot += turn;
                }
            }
        }
    }
    w.entities.list[j].fall_distance = 0.0;
}
