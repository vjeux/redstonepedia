//! Mob AI (the mob-AI lane, 2026-11-08; brief `designs/briefs/mob-ai.md`): what makes mobs walk,
//! read in the 26.3 source and modeled tick exact: path finding and navigation (`path/`), the
//! controls (`control.rs`), the goal selectors and goals (`goal/`), targeting (`target.rs`).
//! Off unless the world turns it on ([`super::MobWorld::ai`]), so the mob layer's other runs are
//! unchanged.
//!
//! [`server_ai_step`] is `Mob.serverAiStep` after its `noActionTime++` (which `living.rs` does):
//! the sight cache, the target and goal selectors (every tick or every other tick by
//! `(tickCount + id) % 2`), the navigation, the mob's own step (a villager's brain), then the move,
//! look and jump controls. [`after_ai_step`] is the rest of `LivingEntity.tick` after `aiStep`
//! (`tickHeadTurn` = `BodyRotationControl.clientTick`, the rotation range checks) and `Mob.tick`'s
//! `updateControlFlags` every 5 ticks.

pub mod attack;
pub mod bat;
pub mod brain;
pub mod cube;
pub mod squid;
pub mod fish;
pub mod phantom;
pub mod control;
pub mod goal;
pub mod path;
pub mod loot;
pub mod player_body;
pub mod poi;
pub mod target;

use super::types::MobType;
use super::{mob, mob_mut};
use crate::entity::{Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;
use control::{BodyRotationControl, LookControl, MoveControl, Operation};
use goal::GoalSelector;
use path::node::Arena;
use path::{Malus, NavKind, NavMob, Navigation};

/// `Sensing`: the line-of-sight answers of this tick (`seen`, `unseen`), cleared at each AI step.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Sensing {
    pub seen: Vec<u32>,
    pub unseen: Vec<u32>,
}

/// A goal mob's AI state (the `Mob` fields of the AI: navigation, controls, selectors, sensing,
/// target, head and body yaw).
#[derive(Clone, PartialEq, Debug)]
pub struct MobAi {
    pub nav: Navigation,
    pub move_ctl: MoveControl,
    pub look: LookControl,
    /// `JumpControl.jump`.
    pub jump: bool,
    pub body: BodyRotationControl,
    pub goals: GoalSelector,
    pub targets: GoalSelector,
    pub sensing: Sensing,
    /// `Mob.target` (an entity id; `getTarget` = `asValidTarget`).
    pub target: Option<u32>,
    pub y_head_rot: f32,
    pub y_body_rot: f32,
    pub y_head_rot_o: f32,
    pub y_body_rot_o: f32,
    /// `Mob.ambientSoundTime`.
    pub ambient_sound_time: i32,
    /// `Mob.isAggressive()` (`DATA_MOB_FLAGS` bit 4).
    pub aggressive: bool,
    /// `LivingEntity.isUsingItem()` and `useItemRemaining` (a skeleton drawing its bow).
    pub using_item: bool,
    pub use_item_remaining: i32,
    /// `getPathfindingMalus` overrides.
    pub malus: Malus,
    /// The search buffers (reused between searches).
    pub arena: Arena,
    /// `getLastHurtByMob` (id) and its timestamp (`lastHurtByMobTimestamp` = tickCount).
    pub last_hurt_by_mob: Option<u32>,
    pub last_hurt_by_mob_timestamp: i32,
    /// `Zombie.canBreakDoors`.
    pub can_break_doors: bool,
    /// `Zombie.drowningTracker` (a zombie whose `convertsInWater`): ticks its eye has been in water
    /// (`afflictionTime`, -1 once out) and the conversion's countdown once started.
    pub drown_time: i32,
    pub drown_conversion: Option<i32>,
    /// `lastHurtMob` (the last entity this one hit).
    pub last_hurt_mob: Option<u32>,
    /// The attacker of the last damage (`lastDamageSource.getEntity()`).
    pub last_damage_entity: Option<u32>,
    /// `NeutralMob`: the persistent anger target and its end time (-1: none).
    pub anger_target: Option<u32>,
    pub anger_end: i64,
    /// The creeper's fuse: `swell`, `DATA_SWELL_DIR` (-1 at creation) and SwellGoal's target.
    pub swell: i32,
    pub swell_dir: i32,
    pub swell_target: Option<u32>,
    /// (mob-AI 2026-11-11) The creeper's `DATA_IS_IGNITED` (`ignite()`: it swells on its own),
    /// `maxSwell` (`Fuse`, 30), `explosionRadius` (`ExplosionRadius`, 3) and `DATA_IS_POWERED`
    /// (`powered`: lightning, `Creeper.thunderHit`; its blast's radius doubled).
    pub creeper_ignited: bool,
    pub max_swell: i32,
    pub explosion_radius: i32,
    pub creeper_powered: bool,
    /// A cube mob's move control and landing state ([`cube`]).
    pub cube: Option<cube::CubeState>,
    /// A squid's tentacles and swim direction ([`squid`]).
    pub squid: Option<squid::SquidState>,
    /// A fish's school ([`fish`]).
    pub fish: Option<fish::FishState>,
    /// (mob-AI 2026-11-10) A phantom's anchor, move target and circling ([`phantom`]).
    pub phantom: Option<phantom::PhantomState>,
    /// A wolf's shake ([`goal::wolf`]).
    pub wolf: Option<goal::wolf::WolfState>,
    /// A cat's sprint and crouch ([`goal::cat`]).
    pub cat: Option<goal::cat::CatState>,
    /// An enderman's carried block and target change time.
    pub enderman: Option<goal::enderman::EndermanState>,
    /// (tickexact 2026-11-10) A drowned's `searchingForLand` (its swim up's) and the entity's
    /// `isSwimming` (`Drowned.updateSwimming` in `baseTick`: under water and wanting to swim).
    pub searching_for_land: bool,
    pub swimming: bool,
    /// (tickexact 2026-11-10, the portal pad) A zombified piglin's own state (its first anger
    /// sound's countdown, its alert interval): `ZombifiedPiglin`.
    pub piglin: Option<goal::zombie::PiglinState>,
    /// A witch drinking: the potion (0 water breathing, 1 fire resistance, 2 healing, 3 swiftness)
    /// and its `usingTime`.
    pub witch_drink: Option<(u8, i32)>,
    /// A horse's state (`AbstractHorse`: attributes, eating, standing).
    pub horse: Option<goal::animal::Horse>,
    /// A zombie holding an iron spear (a zombie horseman's, or 1 in 6 of the rare weapons).
    /// `LivingEntity.recentKineticEnemies` (a spear in use: each entity it stabbed and when).
    pub stabbed: Vec<(u32, i64)>,
    /// `AgeableMob.age` of a farm animal (0 an adult, a baby counts up from -24000).
    pub age: i32,
    /// (mob-AI 2026-11-11) `Mob.homePosition` and `homeRadius` (`None`: -1, no home): a wandering
    /// trader's (its spawner's reference, 16), a leashed mob's (`PathfinderMob.whenLeashedTo`: the
    /// holder's block, 5, every tick). The random positions pull toward it and stay inside it.
    pub home: Option<(crate::pos::BlockPos, i32)>,
    /// (mob-AI 2026-11-11) A wandering trader's or a trader llama's own state.
    pub trader: Option<goal::trader::TraderState>,
}

impl MobAi {
    /// A mob's AI as its constructor leaves it: the navigation (`createNavigation`, its budget from
    /// the FOLLOW_RANGE base value), the controls, `registerGoals`. `y_rot` is its yaw (the head
    /// and body start at it).
    pub fn new(ty: MobType, y_rot: f32) -> MobAi {
        let d = ty.def();
        let kind = if d.id == "spider" || d.id == "cave_spider" { NavKind::WallClimber } else { NavKind::Ground };
        let malus = if ty == MobType::Villager {
            Malus::villager()
        } else if goal::animal::is_animal(d.id) {
            // Animal's constructor: FIRE_IN_NEIGHBOR 16, FIRE -1; then the chicken's own
            // setPathfindingMalus(WATER, 0)
            use crate::entity::mob::ai::path::path_type::PathType;
            let mut m = Malus::default();
            m.set(PathType::FireInNeighbor, 16.0);
            m.set(PathType::Fire, -1.0);
            if goal::animal::adult_of(d.id) == Some("chicken") {
                m.set(PathType::Water, 0.0);
            }
            m
        } else if d.id == "enderman" {
            // the enderman's constructor: setPathfindingMalus(WATER, -1)
            let mut m = Malus::default();
            m.set(crate::entity::mob::ai::path::path_type::PathType::Water, -1.0);
            m
        } else if goal::drowned::is_drowned_id(d.id) {
            // the drowned's constructor: setPathfindingMalus(WATER, 0)
            let mut m = Malus::default();
            m.set(crate::entity::mob::ai::path::path_type::PathType::Water, 0.0);
            m
        } else {
            Malus::default()
        };
        // Drowned.createNavigation: an AmphibiousPathNavigation
        let kind = if goal::drowned::is_drowned_id(d.id) { NavKind::Amphibious } else { kind };
        let mut nav = Navigation::new(kind, d.follow_range);
        if ty == MobType::Villager {
            // AbstractVillager/Villager's constructor: doors, floating, a 48-block path budget
            // (setRequiredPathLength: floor(max(FOLLOW_RANGE, 48) x 16) nodes)
            nav.flags.can_open_doors = true;
            nav.flags.can_float = true;
            nav.required_path_length = 48.0;
            nav.max_visited_nodes = mth::floor(((d.follow_range as f32).max(48.0) * 16.0f32) as f64);
        }
        // (mob-AI 2026-11-11) Llama's constructor: setRequiredPathLength(40) (a trader llama's path
        // to its spit target reaches past its 16 follow range: d1-bed-plate plains b9 t38307)
        if d.id == "trader_llama" {
            nav.required_path_length = 40.0;
            nav.max_visited_nodes = mth::floor(((d.follow_range as f32).max(40.0) * 16.0f32) as f64);
        }
        let mut ai = MobAi {
            nav,
            move_ctl: MoveControl::default(),
            look: LookControl::default(),
            jump: false,
            body: BodyRotationControl::default(),
            goals: GoalSelector::default(),
            targets: GoalSelector::default(),
            sensing: Sensing::default(),
            target: None,
            y_head_rot: y_rot,
            y_body_rot: y_rot,
            y_head_rot_o: y_rot,
            y_body_rot_o: y_rot,
            ambient_sound_time: 0,
            aggressive: false,
            using_item: false,
            use_item_remaining: 0,
            malus,
            arena: Arena::default(),
            last_hurt_by_mob: None,
            last_hurt_by_mob_timestamp: 0,
            can_break_doors: false,
            drown_time: 0,
            drown_conversion: None,
            last_hurt_mob: None,
            last_damage_entity: None,
            anger_target: None,
            anger_end: -1,
            swell: 0,
            swell_dir: -1,
            swell_target: None,
            creeper_ignited: false,
            max_swell: 30,
            explosion_radius: 3,
            creeper_powered: false,
            cube: None,
            squid: None,
            fish: None,
            phantom: None,
            wolf: None,
            cat: None,
            enderman: None,
            searching_for_land: false,
            swimming: false,
            piglin: None,
            witch_drink: None,
            horse: None,
            stabbed: Vec::new(),
            age: 0,
            home: None,
            trader: None,
        };
        goal::register_goals(&mut ai, ty);
        if let Some(_) = cube::cube_size(d.id) {
            cube::register(&mut ai, d.id, y_rot);
        }
        if squid::is_squid_id(d.id) {
            squid::register(&mut ai, d.id);
        }
        if fish::is_fish_id(d.id) {
            fish::register(&mut ai, d.id);
        }
        if phantom::is_phantom_id(d.id) {
            phantom::register(&mut ai);
        }
        ai
    }
}

/// The AI of entity `i` (it must have one).
pub fn ai(w: &World, i: usize) -> &MobAi {
    mob(w, i).ai.as_deref().expect("a mob with AI")
}

pub fn ai_mut(w: &mut World, i: usize) -> &mut MobAi {
    mob_mut(w, i).ai.as_deref_mut().expect("a mob with AI")
}

/// Whether entity `i` is a mob with AI.
pub fn has_ai(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, crate::entity::EntityKind::Mob(m) if m.ai.is_some())
}

/// The identity hash of a POI type's registry holder (`System.identityHashCode`): the game's
/// JVM's when the bridge gave them, else a fixed stand-in per type (any JVM run's is as good:
/// the judge).
pub fn poi_holder_hash(w: &World, ty: &str) -> i32 {
    if let Some(h) = w.mob.ai.poi_hashes.as_ref().and_then(|m| m.get(ty)) {
        return *h;
    }
    // String.hashCode of the id, as a 31-bit identity hash
    ty.bytes().fold(0i32, |h, b| h.wrapping_mul(31).wrapping_add(b as i32)) & 0x7fff_ffff
}

/// `Collections.shuffle(List)`'s static random: in an exact world the bridge's per-sandbox seed
/// (`rng::collections_seed`), else a stand-in drawn once from the mob layer's random (the judge).
pub fn collections_rng(w: &mut World) -> &mut crate::rng::LegacyRandom {
    if w.mob.ai.collections.is_none() {
        let seed = match w.exact_seed {
            Some(s) => crate::rng::collections_seed(s),
            None => w.mob.rng_mut().next_long(),
        };
        w.mob.ai.collections = Some(crate::rng::LegacyRandom::new(seed));
    }
    w.mob.ai.collections.as_mut().expect("set")
}

/// `getAttributeValue(MOVEMENT_SPEED)`.
pub fn movement_speed(w: &World, i: usize) -> f64 {
    // a horse's MOVEMENT_SPEED as randomizeAttributes drew it
    let base = ai(w, i).horse.map(|h| h.speed).unwrap_or(mob(w, i).ty.def().speed);
    // an enderman with a target: SPEED_MODIFIER_ATTACKING (+0.15)
    if ai(w, i).enderman.is_some() {
        return base + goal::enderman::speed_bonus(w, i);
    }
    // an angry adult zombified piglin: SPEED_MODIFIER_ATTACKING (+0.05, ADD_VALUE)
    if ai(w, i).piglin.is_some_and(|p| p.attacking) {
        return base + 0.05;
    }
    // a witch drinking: -0.25 (ADD_VALUE); the speed effect: x (1 + 0.2F) (ADD_MULTIPLIED_TOTAL);
    // a cat's sprint: x (1 + 0.3F)
    let add = if ai(w, i).witch_drink.is_some() { -0.25 } else { 0.0 };
    let v = base + add;
    if ai(w, i).cat.is_some() {
        return v * goal::cat::speed_factor(w, i);
    }
    if mob(w, i).effects.speed > 0 {
        return v * (1.0 + 0.2f32 as f64);
    }
    v
}

/// `getAttributeValue(FOLLOW_RANGE)`.
pub fn follow_range(w: &World, i: usize) -> f64 {
    mob(w, i).follow_range
}

/// (mob-AI 2026-11-10) `Mob.onAttributeUpdated(FOLLOW_RANGE)`: `getNavigation()
/// .updatePathfinderMaxVisitedNodes()`, its search budget `floor(max(FOLLOW_RANGE,
/// requiredPathLength) x 16)` (the constructor's was the base value's: a natural mob's finalizeSpawn
/// bonus changes it; forest build 2 t11684: a drowned's 540 against 560). `own`: the mob's own
/// navigation (`Mob.finalizeSpawn`'s bonus comes before a jockey mounts); else `getNavigation()`'s:
/// a mob riding one it steers updates that one's, by that one's follow range.
pub fn refresh_path_budget(w: &mut World, id: u32, own: bool) {
    let Some(i) = crate::entity::index_of(w, id) else { return };
    if !has_ai(w, i) {
        return;
    }
    let j = if own { i } else { controlled_vehicle(w, i).unwrap_or(i) };
    if !has_ai(w, j) {
        return;
    }
    let fr = mob(w, j).follow_range as f32;
    let a = ai_mut(w, j);
    a.nav.max_visited_nodes = crate::entity::mth::floor((fr.max(a.nav.required_path_length) * 16.0f32) as f64);
}

/// `Mob.getMaxFallDistance()` (the creeper's own rule: its health minus 1 with a target).
pub fn max_fall_distance(w: &World, i: usize) -> i32 {
    let m = mob(w, i);
    let a = ai(w, i);
    if target::get_target(w, i).is_none() {
        return 3;
    }
    if m.ty.def().id == "creeper" {
        return mth::floor((m.health - 1.0 + 3.0f32) as f64);
    }
    // getMaxHealth(): a horse's (a trader llama's) drawn max, a leader zombie's bonus (mob-AI
    // 2026-11-11: a llama's 53 base made its path to a spit target 11 below partial, d1-bed-plate
    // plains b9 t39150)
    let max = crate::entity::mob::living::max_health(m);
    let _ = a;
    let mut sacrifice = (m.health - max * 0.33f32) as i32;
    sacrifice -= (3 - difficulty_id(w)) * 4;
    if sacrifice < 0 {
        sacrifice = 0;
    }
    mth::floor((sacrifice as f32 + 3.0f32) as f64)
}

/// `Difficulty.getId()` (peaceful 0 .. hard 3) of the world (the natural spawner's setting, else
/// the AI world's).
pub fn difficulty_id(w: &World) -> i32 {
    use super::natural::Difficulty;
    match w.mob.natural.as_ref().map(|n| n.difficulty).unwrap_or(w.mob.ai.difficulty) {
        Difficulty::Peaceful => 0,
        Difficulty::Easy => 1,
        Difficulty::Normal => 2,
        Difficulty::Hard => 3,
    }
}

/// A player the AI sees (the AFK player of a farm, the harness's fake player): its feet, its
/// game mode. Players are not in the entity list; their ids are [`PLAYER_ID_BASE`] + index.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Player {
    pub pos: Vec3,
    pub mode: GameMode,
    pub invulnerable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
    Spectator,
}

/// The ids players take in targets and sight caches.
pub const PLAYER_ID_BASE: u32 = 0xF000_0000;

/// The AI's level state (`MobWorld.ai`).
#[derive(Clone, Debug)]
pub struct AiWorld {
    /// Mobs added while this holds get their AI (`Mob` constructor: `registerGoals`).
    pub on: bool,
    pub difficulty: super::natural::Difficulty,
    pub players: Vec<Player>,
    /// `GameRules.MOB_GRIEFING`.
    pub mob_griefing: bool,
    /// The attacker of the hit `living::hurt` is dealing (set by [`attack::do_hurt_target`]).
    pub attacker: Option<u32>,
    /// The motion of the arrow whose hit `living::hurt` is dealing (`arrow::hit_entity`): the
    /// knockback's direction (`Projectile.calculateHorizontalHurtKnockbackDirection`).
    pub projectile_motion: Option<crate::entity::Vec3>,
    /// The POI index (`poi::index`; `None`: to rebuild).
    pub pois: Option<Vec<(crate::pos::BlockPos, poi::PoiKind)>>,
    /// The order POIs were added (`PoiManager.add` on a POI block placed: a section's record set
    /// keeps colliding records in this order); a POI of the world's first scan has none.
    pub poi_seq: std::collections::HashMap<crate::pos::BlockPos, u64>,
    pub poi_next: u64,
    /// The POI records' taken tickets (`poi::take`/`release`; `None`: still the memories' claims).
    pub poi_tickets: Option<std::collections::BTreeMap<(i32, i32, i32), u32>>,
    /// The light sources of a design world (`goal::stroll::light_magic`; `None`: to rebuild).
    pub lights: Option<Vec<(crate::pos::BlockPos, i32)>>,
    /// The sky light of a design world's cells asked so far (cleared when a block changes).
    pub sky_cache: std::collections::HashMap<crate::pos::BlockPos, i32>,
    /// `Collections.shuffle(List)`'s one static random ([`collections_rng`]).
    pub collections: Option<crate::rng::LegacyRandom>,
    /// The block light of a design world's cells asked so far, by the sky part it had to beat
    /// (cleared when a block changes).
    pub block_cache: std::collections::HashMap<(crate::pos::BlockPos, i32), i32>,
    /// Each POI type's registry holder identity hash (the game's JVM's: the bridge's
    /// `identity_hashes`; `None`: a stand-in per type, [`poi_holder_hash`]).
    pub poi_hashes: Option<std::collections::HashMap<String, i32>>,
    /// A summon is adding the mob (the farm's own): an unmodeled AI is refused, not run on the
    /// legacy walk model.
    pub strict: bool,
    /// The ambient natural mob types run on the legacy walk model (their AI not modeled yet).
    pub legacy: std::collections::BTreeSet<&'static str>,
    /// The players' bodies (`player_body`: their motion, pushed by mobs), by player index.
    pub bodies: Vec<player_body::Body>,
    /// A player's body has moved it at least once (natural spawning then filters the ticking
    /// chunks by its position each tick).
    pub player_moved: bool,
    /// (mob-AI 2026-11-11) `PoiManager.DistanceTracker`'s memory: the sections it ever stored, i.e.
    /// those within 5 of a village centre at one of its runs. Its levels are clamped to 0..6 and
    /// `setLevel` keeps any level up to 6, so when the last centre near a section goes, the
    /// increase path stores 6 there. The section reads 6, not the map's default 7, from then on
    /// (until a centre comes within 5 again). Also the centres already counted.
    pub village_seen: std::sync::Arc<std::collections::HashSet<(i32, i32, i32)>>,
    pub village_noted: std::collections::HashSet<(i32, i32, i32)>,
}

/// A mob whose AI is not modeled: no goals and no targets (villagers have their brain, bats their
/// own flight).
pub fn unmodeled(ty: MobType, a: &MobAi) -> bool {
    ty != MobType::Villager && ty.def().id != "bat" && a.goals.goals.is_empty() && a.targets.goals.is_empty()
}

impl Default for AiWorld {
    fn default() -> AiWorld {
        AiWorld { on: false, difficulty: super::natural::Difficulty::Normal, players: Vec::new(), mob_griefing: true, attacker: None, projectile_motion: None, pois: None, poi_seq: std::collections::HashMap::new(), poi_next: 0, poi_tickets: Some(std::collections::BTreeMap::new()), lights: None, sky_cache: std::collections::HashMap::new(), collections: None, block_cache: std::collections::HashMap::new(), poi_hashes: None, strict: false, legacy: std::collections::BTreeSet::new(), bodies: Vec::new(), player_moved: false, village_seen: Default::default(), village_noted: std::collections::HashSet::new() }
    }
}

/// What the navigation reads of mob `i` now.
pub fn nav_mob(w: &World, i: usize) -> NavMob {
    let e = &w.entities.list[i];
    let m = mob(w, i);
    let (width, height) = m.dims();
    NavMob {
        pos: e.pos,
        bb: e.bb,
        width,
        height,
        max_up_step: m.ty.def().step,
        on_ground: e.on_ground,
        in_liquid: e.fluid.in_water() || m.lava_height > 0.0,
        in_floatable: e.fluid.in_water(),
        is_passenger: e.vehicle.is_some(),
        speed: m.speed,
        malus: ai(w, i).malus,
        max_fall: max_fall_distance(w, i),
        follow_range: m.follow_range,
        game_time: w.time,
    }
}

/// Run `f` on mob `i`'s navigation with its search buffers (taken out of the mob for the call).
pub fn with_nav<R>(w: &mut World, i: usize, f: impl FnOnce(&World, &NavMob, &mut Navigation, &mut Arena) -> R) -> R {
    // `getNavigation()`: a mob controlling a mob vehicle uses the vehicle's
    let i = nav_owner(w, i);
    with_own_nav(w, i, f)
}

/// The mob's own `navigation` field (`Mob.serverAiStep` ticks it, whoever steers it).
pub fn with_own_nav<R>(w: &mut World, i: usize, f: impl FnOnce(&World, &NavMob, &mut Navigation, &mut Arena) -> R) -> R {
    let nm = nav_mob(w, i);
    let a = ai_mut(w, i);
    let mut nav = std::mem::replace(&mut a.nav, Navigation::new(NavKind::Ground, 0.0));
    let mut arena = std::mem::take(&mut a.arena);
    let r = f(w, &nm, &mut nav, &mut arena);
    let a = ai_mut(w, i);
    a.nav = nav;
    a.arena = arena;
    r
}

/// `Entity.getControlledVehicle()` of a mob: its vehicle when it is that vehicle's controlling
/// passenger (`Mob.getControllingPassenger`: the first passenger, a mob, of a vehicle mob with AI).
pub fn controlled_vehicle(w: &World, i: usize) -> Option<usize> {
    let v = w.entities.list[i].vehicle?;
    let vi = crate::entity::index_of(w, v)?;
    let ve = &w.entities.list[vi];
    let crate::entity::EntityKind::Mob(vm) = &ve.kind else { return None };
    (vm.ai.is_some() && ve.passengers.first() == Some(&w.entities.list[i].id) && matches!(w.entities.list[i].kind, crate::entity::EntityKind::Mob(_))).then_some(vi)
}

/// `Mob.getControllingPassenger()` is a mob (its first passenger: a mob with AI controls it).
pub fn controlled_by_mob(w: &World, i: usize) -> bool {
    let e = &w.entities.list[i];
    e.passengers.first().and_then(|p| crate::entity::index_of(w, *p)).is_some_and(|pi| matches!(w.entities.list[pi].kind, crate::entity::EntityKind::Mob(_)))
}

/// Whose navigation (and move control) `getNavigation()` answers for mob `i`.
pub fn nav_owner(w: &World, i: usize) -> usize {
    controlled_vehicle(w, i).unwrap_or(i)
}

/// `getNavigation()` of mob `i`.
pub fn nav_of(w: &World, i: usize) -> &Navigation {
    &ai(w, nav_owner(w, i)).nav
}

pub fn nav_of_mut(w: &mut World, i: usize) -> &mut Navigation {
    let j = nav_owner(w, i);
    &mut ai_mut(w, j).nav
}

/// `Mob.serverAiStep` after `noActionTime++`.
pub fn server_ai_step(w: &mut World, i: usize) {
    {
        let a = ai_mut(w, i);
        a.sensing.seen.clear();
        a.sensing.unseen.clear();
    }
    let e = &w.entities.list[i];
    let id_based = e.tick_count.wrapping_add(e.network_id);
    let every_other = id_based % 2 != 0 && e.tick_count > 1;
    goal::tick_selector(w, i, true, every_other);
    goal::tick_selector(w, i, false, every_other);
    // navigation.tick() (its own, whoever steers it), its wanted position on getMoveControl()
    // (a rider's: the vehicle's it controls)
    let wanted = with_own_nav(w, i, |w, nm, nav, arena| nav.tick(w, nm, arena));
    if let Some((x, y, z, s)) = wanted {
        let mc = controlled_vehicle(w, i).filter(|&v| has_ai(w, v)).unwrap_or(i);
        ai_mut(w, mc).move_ctl.set_wanted_position(x, y, z, s);
    }
    custom_server_ai_step(w, i);
    if w.entities.list[i].removed {
        return;
    }
    move_control_tick(w, i);
    look_control_tick(w, i);
    jump_control_tick(w, i);
}

/// `customServerAiStep`: the villager's brain (and its raid draw); the bat's flight; nothing for
/// the goal mobs.
fn custom_server_ai_step(w: &mut World, i: usize) {
    if mob(w, i).ty == MobType::Villager {
        super::villager::brain_tick(w, i);
    } else if bat::is_bat(w, i) {
        bat::custom_server_ai_step(w, i);
    } else if ai(w, i).enderman.is_some() {
        goal::enderman::custom_server_ai_step(w, i);
    } else if ai(w, i).piglin.is_some() {
        goal::zombie::piglin_custom_server_ai_step(w, i);
    } else if ai(w, i).cat.is_some() {
        goal::cat::custom_server_ai_step(w, i);
    }
}

/// `MoveControl.tick()`.
pub fn move_control_tick(w: &mut World, i: usize) {
    if ai(w, i).cube.is_some() {
        return cube::move_control_tick(w, i);
    }
    // (mob-AI 2026-11-10) Phantom.PhantomMoveControl
    if ai(w, i).phantom.is_some() {
        return phantom::move_control_tick(w, i);
    }
    // AbstractFish.FishMoveControl
    if ai(w, i).fish.is_some() {
        return fish::move_control_tick(w, i);
    }
    // DrownedMoveControl.tick: swimming (it wants to and is in water) replaces MoveControl's tick;
    // else 0.008 more fall off the ground first
    if goal::drowned::is_drowned(w, i) && goal::drowned::move_control_before(w, i) {
        return;
    }
    let op = ai(w, i).move_ctl.operation;
    match op {
        Operation::Strafe => {
            let speed = movement_speed(w, i) as f32;
            let mc = ai(w, i).move_ctl;
            let speed_modified = mc.speed_modifier as f32 * speed;
            let mut xa = mc.strafe_forwards;
            let mut za = mc.strafe_right;
            let mut dist = mth::sqrt(xa * xa + za * za);
            if dist < 1.0 {
                dist = 1.0;
            }
            dist = speed_modified / dist;
            xa *= dist;
            za *= dist;
            let y_rot = w.entities.list[i].y_rot;
            let sin = mth::sin((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
            let cos = mth::cos((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
            let dx = xa * cos - za * sin;
            let dz = za * cos + xa * sin;
            if !is_walkable(w, i, dx, dz) {
                let a = ai_mut(w, i);
                a.move_ctl.strafe_forwards = 1.0;
                a.move_ctl.strafe_right = 0.0;
            }
            let (f, r) = (ai(w, i).move_ctl.strafe_forwards, ai(w, i).move_ctl.strafe_right);
            let m = mob_mut(w, i);
            m.speed = speed_modified;
            m.zza = f;
            m.xxa = r;
            ai_mut(w, i).move_ctl.operation = Operation::Wait;
        }
        Operation::MoveTo => {
            ai_mut(w, i).move_ctl.operation = Operation::Wait;
            let mc = ai(w, i).move_ctl;
            let e = &w.entities.list[i];
            let xd = mc.wanted_x - e.pos.x;
            let zd = mc.wanted_z - e.pos.z;
            let yd = mc.wanted_y - e.pos.y;
            let dd = xd * xd + yd * yd + zd * zd;
            if dd < 2.5000003e-7f32 as f64 {
                mob_mut(w, i).zza = 0.0;
                return;
            }
            let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
            let y_rot = control::rotlerp(w.entities.list[i].y_rot, y_rot_d, 90.0);
            w.entities.list[i].y_rot = y_rot;
            let speed = (mc.speed_modifier * movement_speed(w, i)) as f32;
            {
                let m = mob_mut(w, i);
                m.speed = speed;
                m.zza = speed;
            }
            let e = &w.entities.list[i];
            let pos = e.block_position();
            let s = w.get(pos);
            let top = path::walk::local_shape_top(w, &s, pos);
            let step = mob(w, i).ty.def().step as f64;
            let width = mob(w, i).dims().0;
            let jump_up = yd > step && xd * xd + zd * zd < 1.0f32.max(width) as f64;
            let in_block = top.is_some_and(|t| e.pos.y < t + pos.y as f64) && !is_door_or_fence(&s);
            if jump_up || in_block {
                let a = ai_mut(w, i);
                a.jump = true;
                a.move_ctl.operation = Operation::Jumping;
            }
        }
        Operation::Jumping => {
            // Mob.setSpeed: the speed and the forward input
            let speed = (ai(w, i).move_ctl.speed_modifier * movement_speed(w, i)) as f32;
            {
                let m = mob_mut(w, i);
                m.speed = speed;
                m.zza = speed;
            }
            let e = &w.entities.list[i];
            let in_liquid = e.fluid.in_water() || mob(w, i).lava_height > 0.0;
            if e.on_ground || in_liquid {
                ai_mut(w, i).move_ctl.operation = Operation::Wait;
            }
        }
        Operation::Wait => {
            mob_mut(w, i).zza = 0.0;
        }
    }
}

/// `BlockTags.DOORS` or `BlockTags.FENCES` (the jump test skips them).
fn is_door_or_fence(s: &crate::block::Block) -> bool {
    use crate::block::Block;
    match s {
        Block::Door { .. } | Block::Fence { .. } => true,
        Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(*id);
            p.has_tag("minecraft:doors") || p.has_tag("minecraft:fences") || p.classes.iter().any(|c| c == "DoorBlock" || c == "FenceBlock")
        }
        _ => false,
    }
}

/// `MoveControl.isWalkable(dx, dz)`: the static path type at the strafe target is WALKABLE.
fn is_walkable(w: &World, i: usize, dx: f32, dz: f32) -> bool {
    let e = &w.entities.list[i];
    let p = BlockPos::new(mth::floor(e.pos.x + dx as f64), mth::floor(e.pos.y), mth::floor(e.pos.z + dz as f64));
    let arena = &mut Arena::default();
    let eval = path::WalkEval::new(w, nav_mob(w, i).to_path_mob(), ai(w, i).nav.flags, arena);
    eval.path_type_static(p.x, p.y, p.z) == path::PathType::Walkable
}

/// `LookControl.tick()`.
pub fn look_control_tick(w: &mut World, i: usize) {
    // (mob-AI 2026-11-10) Phantom.PhantomLookControl.tick: nothing (no pitch reset either)
    if ai(w, i).phantom.is_some() {
        return;
    }
    // resetXRotOnTick
    w.entities.list[i].x_rot = 0.0;
    let lc = ai(w, i).look;
    if lc.look_at_cooldown > 0 {
        ai_mut(w, i).look.look_at_cooldown -= 1;
        let e = &w.entities.list[i];
        let eye_y = e.pos.y + mob(w, i).eye() as f64;
        let xd = lc.wanted_x - e.pos.x;
        let zd = lc.wanted_z - e.pos.z;
        let yd = lc.wanted_y - eye_y;
        if zd.abs() > 1.0e-5f32 as f64 || xd.abs() > 1.0e-5f32 as f64 {
            let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
            let a = ai_mut(w, i);
            a.y_head_rot = control::rotate_towards(a.y_head_rot, y_rot_d, lc.y_max_rot_speed);
        }
        let sd = (xd * xd + zd * zd).sqrt();
        if yd.abs() > 1.0e-5f32 as f64 || sd.abs() > 1.0e-5f32 as f64 {
            let x_rot_d = -control::atan2_degrees(yd, sd);
            let cur = w.entities.list[i].x_rot;
            set_x_rot(w, i, control::rotate_towards(cur, x_rot_d, lc.x_max_rot_angle));
        }
    } else {
        let a = ai_mut(w, i);
        a.y_head_rot = control::rotate_towards(a.y_head_rot, a.y_body_rot, 10.0);
    }
    // clampHeadRotationToBody (`getNavigation().isDone()`: a rider asks its vehicle's)
    if !nav_of(w, i).is_done() {
        let max = max_head_y_rot(w, i) as f32;
        let a = ai_mut(w, i);
        a.y_head_rot = control::rotate_if_necessary(a.y_head_rot, a.y_body_rot, max);
    }
}

/// `Entity.setXRot`: `Math.clamp(xRot % 360, -90, 90)`.
pub fn set_x_rot(w: &mut World, i: usize, x: f32) {
    if x.is_finite() {
        w.entities.list[i].x_rot = (x % 360.0).clamp(-90.0, 90.0);
    }
}

/// `Mob.getMaxHeadYRot()` (75).
pub fn max_head_y_rot(_w: &World, _i: usize) -> i32 {
    75
}

/// `Mob.getMaxHeadXRot()` (40; a cube mob's 0).
pub fn max_head_x_rot(w: &World, i: usize) -> i32 {
    if ai(w, i).cube.is_some() { 0 } else { 40 }
}

/// `Mob.getHeadRotSpeed()` (10).
pub fn head_rot_speed(_w: &World, _i: usize) -> i32 {
    10
}

/// `JumpControl.tick()`.
pub fn jump_control_tick(w: &mut World, i: usize) {
    let j = std::mem::replace(&mut ai_mut(w, i).jump, false);
    mob_mut(w, i).jumping = j;
}

/// `BodyRotationControl.clientTick()` (`Mob.tickHeadTurn`).
pub fn body_rotation_tick(w: &mut World, i: usize) {
    if ai(w, i).phantom.is_some() {
        return phantom::body_rotation_tick(w, i);
    }
    let e = &w.entities.list[i];
    let xd = e.pos.x - e.xo;
    let zd = e.pos.z - e.zo;
    let moving = xd * xd + zd * zd > 2.5000003e-7f32 as f64;
    let y_rot = e.y_rot;
    let max = max_head_y_rot(w, i) as f32;
    let carrying_mob = e.passengers.first().and_then(|p| crate::entity::index_of(w, *p)).is_some_and(|j| matches!(w.entities.list[j].kind, crate::entity::EntityKind::Mob(_)));
    let a = ai_mut(w, i);
    if moving {
        a.y_body_rot = y_rot;
        a.y_head_rot = control::rotate_if_necessary(a.y_head_rot, a.y_body_rot, max);
        a.body.last_stable_y_head_rot = a.y_head_rot;
        a.body.head_stable_time = 0;
    } else if !carrying_mob {
        if (a.y_head_rot - a.body.last_stable_y_head_rot).abs() > 15.0 {
            a.body.head_stable_time = 0;
            a.body.last_stable_y_head_rot = a.y_head_rot;
            a.y_body_rot = control::rotate_if_necessary(a.y_body_rot, a.y_head_rot, max);
        } else {
            a.body.head_stable_time += 1;
            if a.body.head_stable_time > 10 {
                let since = a.body.head_stable_time - 10;
                let frac = mth::clampf(since as f32 / 10.0, 0.0, 1.0);
                let remaining = max * (1.0 - frac);
                a.y_body_rot = control::rotate_if_necessary(a.y_body_rot, a.y_head_rot, remaining);
            }
        }
    }
}

/// The rest of `LivingEntity.tick` after `aiStep` for a mob with AI (`tickHeadTurn`, the rotation
/// range checks) and `Mob.tick`'s `updateControlFlags` every 5 ticks.
/// `Zombie.tick`'s `drowningTracker.tick()` (`ConversionTracker`; zombies and baby zombies:
/// `convertsInWater`, not zombie villagers or drowned): its eye in water 600 ticks starts a
/// 300-tick conversion, out of water resets both; then the conversion into a drowned
/// (`natural::zombie_to_drowned`). It runs after the living tick, with the eye state of this
/// tick's base tick (no draws).
pub fn zombie_drowning(w: &mut World, i: usize) {
    if !matches!(mob(w, i).ty.def().id, "zombie" | "baby_zombie") || mob(w, i).health <= 0.0 {
        return;
    }
    let eye = w.entities.list[i].fluid.eye_in_water();
    let a = ai_mut(w, i);
    if !eye {
        a.drown_time = -1;
        a.drown_conversion = None;
        return;
    }
    match a.drown_conversion.as_mut() {
        Some(c) => {
            *c -= 1;
            if *c < 0 {
                // the conversion: a drowned in its place (natural::zombie_to_drowned)
                super::natural::zombie_to_drowned(w, i);
            }
        }
        None => {
            a.drown_time += 1;
            if a.drown_time >= 600 {
                a.drown_conversion = Some(300);
            }
        }
    }
}

pub fn after_ai_step(w: &mut World, i: usize) {
    // inside aiStep, before tickHeadTurn: AgeableMob.aiStep's age (a grown baby's box and its
    // fudge move, which the body's turn sees), Chicken.aiStep's tail (the slow fall, the egg)
    goal::animal::after_ai_step(w, i);
    // Wolf.aiStep's tail (after Animal's): the shake starts, the persistent anger
    goal::wolf::after_ai_step(w, i);
    // AgeableMob.aiStep's age (a baby squid's), then Squid.aiStep's tail (after
    // LivingEntity.aiStep): the tentacles, the swim impulse, the turns
    squid::age_step(w, i);
    squid::after_ai_step(w, i);
    // IronGolem.aiStep's tail (after Mob.aiStep): attackAnimationTick/offerFlowerTick (client
    // looks) and the persistent anger update
    if mob(w, i).ty == MobType::IronGolem {
        goal::golem::update_anger(w, i);
    }
    body_rotation_tick(w, i);
    {
        let e = &mut w.entities.list[i];
        while e.y_rot - e.y_rot_o < -180.0 {
            e.y_rot_o -= 360.0;
        }
        while e.y_rot - e.y_rot_o >= 180.0 {
            e.y_rot_o += 360.0;
        }
    }
    {
        let a = ai_mut(w, i);
        while a.y_body_rot - a.y_body_rot_o < -180.0 {
            a.y_body_rot_o -= 360.0;
        }
        while a.y_body_rot - a.y_body_rot_o >= 180.0 {
            a.y_body_rot_o += 360.0;
        }
    }
    {
        let e = &mut w.entities.list[i];
        while e.x_rot - e.x_rot_o < -180.0 {
            e.x_rot_o -= 360.0;
        }
        while e.x_rot - e.x_rot_o >= 180.0 {
            e.x_rot_o += 360.0;
        }
    }
    {
        let a = ai_mut(w, i);
        while a.y_head_rot - a.y_head_rot_o < -180.0 {
            a.y_head_rot_o -= 360.0;
        }
        while a.y_head_rot - a.y_head_rot_o >= 180.0 {
            a.y_head_rot_o += 360.0;
        }
    }
    if mob(w, i).sleeping_pos.is_some() {
        w.entities.list[i].x_rot = 0.0;
    }
    if w.entities.list[i].tick_count % 5 == 0 {
        update_control_flags(w, i);
    }
}

/// `Mob.updateControlFlags()`: a mob controlling this one turns its MOVE, JUMP and LOOK goals off;
/// JUMP is off in a boat too.
fn update_control_flags(w: &mut World, i: usize) {
    let in_boat = w.entities.list[i].vehicle.and_then(|v| crate::entity::index_of(w, v)).is_some_and(|vi| matches!(w.entities.list[vi].kind, crate::entity::EntityKind::Boat(_)));
    let no_controller = !controlled_by_mob(w, i);
    let a = ai_mut(w, i);
    a.goals.set_control_flag(goal::Flag::Move, no_controller);
    a.goals.set_control_flag(goal::Flag::Jump, no_controller && !in_boat);
    a.goals.set_control_flag(goal::Flag::Look, no_controller);
}

/// `Entity.baseTick`'s `canSpawnSprintParticle()` (before the fluid update): the iron golem's
/// override draws `nextInt(5)` from its own random whenever it moves sideways; on a hit,
/// `spawnSprintParticle` draws two `nextDouble`s (the particle's x and z) unless the block 0.2
/// below (`getOnPosLegacy`) renders invisible (air, liquids, bubble columns, moving pistons...).
pub fn before_base_tick(w: &mut World, i: usize) {
    // a sprinting cat (Entity.canSpawnSprintParticle: no draw of its own) spawns one too
    let cat_sprint = goal::cat::sprint_particle(w, i);
    if mob(w, i).ty == MobType::IronGolem || cat_sprint {
        let d = w.entities.list[i].delta_movement;
        let golem = mob(w, i).ty == MobType::IronGolem;
        if cat_sprint || (golem && d.x * d.x + d.z * d.z > 2.5000003e-7f32 as f64 && w.entities.list[i].random.as_mut().expect("random").next_int(5) == 0) {
            let p = crate::entity::collide::get_on_pos(w, &w.entities.list[i], 0.2);
            let invisible = matches!(w.get(p), crate::block::Block::Air | crate::block::Block::Water { .. } | crate::block::Block::Lava { .. } | crate::block::Block::BubbleColumn { .. } | crate::block::Block::MovingPiston { .. });
            if !invisible {
                let r = w.entities.list[i].random.as_mut().expect("random");
                r.next_double();
                r.next_double();
            }
        }
    }
}

/// `IronGolem.doPush(entity)` before the push itself: an enemy (not a creeper) it pushes becomes
/// its target one time in 20 (its own random).
pub fn before_do_push(w: &mut World, i: usize, j: usize) {
    if mob(w, i).ty != MobType::IronGolem {
        return;
    }
    let ty = mob(w, j).ty;
    if !goal::targets::is_enemy(ty) {
        return;
    }
    if w.entities.list[i].random.as_mut().expect("random").next_int(20) == 0 {
        let id = w.entities.list[j].id;
        target::set_target(w, i, Some(id));
    }
}

/// The rest of `LivingEntity.baseTick` (the attacker memory, the old head and body yaw) and
/// `Mob.baseTick`'s ambient sound roll.
pub fn after_base_tick(w: &mut World, i: usize) {
    attack::forget_attacker(w, i);
    {
        let a = ai_mut(w, i);
        a.y_head_rot_o = a.y_head_rot;
        a.y_body_rot_o = a.y_body_rot;
    }
    ambient_sound_tick(w, i);
}

/// `Mob.checkDespawn()` where natural spawning (natural.rs, which owns it there) is off and the
/// mob AI has players: the nearest non-spectator player within 32 resets `noActionTime`; a mob
/// that is not persistent and would despawn (beyond 128, or the 1-in-800 roll past 600 idle
/// ticks beyond 32: that roll is drawn) is refused.
pub fn check_despawn(w: &mut World, i: usize) {
    // (a dying mob too: ServerLevel.tickNonPassenger's checkDespawn runs for every entity; a
    // player within 32 resets its noActionTime)
    let m = mob(w, i);
    if difficulty_id(w) == 0 && m.ty.def().monster {
        refuse_once(w, "a monster in a peaceful world (discarded) is not modeled");
        return;
    }
    // `isPersistenceRequired() || requiresCustomPersistence()` (a passenger or a leashed mob)
    let persistent = m.persistence_required || w.entities.list[i].vehicle.is_some();
    let no_action = m.no_action_time;
    // `removeWhenFarAway`: natural spawning's per-kind rule (an animal never, a fish not from a
    // bucket...), else villagers and golems never
    let removes_far = match super::natural::kind_of_type(m.ty) {
        Some(k) => super::natural::removes_far_now(w, i, k),
        None => !matches!(m.ty, MobType::Villager | MobType::IronGolem),
    };
    let p = w.entities.list[i].pos;
    // `getNearestPlayer(this, -1.0)` (no spectators): the AI's players, the AFK player, natural
    // spawning's players
    let mut players: Vec<Vec3> = w.mob.ai.players.iter().filter(|q| q.mode != GameMode::Spectator).map(|q| q.pos).collect();
    players.extend(w.afk.iter().copied());
    if let Some(n) = w.mob.natural.as_ref() {
        players.extend(n.players.iter().copied());
    }
    let near = players.iter().map(|q| q.distance_to_sqr(p)).fold(None, |a: Option<f64>, d| Some(a.map_or(d, |a| a.min(d))));
    let Some(d2) = near else { return };
    // the category's despawn distance (128; 64 for water ambient), the no-despawn distance 32
    let far = super::natural::category_of(m.ty).despawn_distance();
    let ty = m.ty;
    let id = w.entities.list[i].id;
    if !persistent && d2 > far * far && removes_far {
        super::natural::discard_as(w, i, id, ty.game_id(), true);
        return;
    }
    let roll = !persistent && no_action > 600 && w.entities.list[i].random.as_mut().expect("random").next_int(800) == 0;
    if roll && d2 > 32.0 * 32.0 && removes_far {
        super::natural::discard_as(w, i, id, ty.game_id(), false);
    } else if d2 < 32.0 * 32.0 {
        mob_mut(w, i).no_action_time = 0;
    }
}

fn refuse_once(w: &mut World, why: &str) {
    if !w.errors.iter().any(|e| e == why) {
        w.errors.push(why.to_string());
    }
}

/// `Monster.updateNoActionTime()` (first in `Monster.aiStep`): bright light at the eye counts
/// double.
pub fn update_no_action_time(w: &mut World, i: usize) {
    // a Monster's (the cube mobs spawn as monsters but are AgeableMobs; the phantom is a Mob
    // implementing Enemy, not a Monster: mob-AI 2026-11-11)
    if !mob(w, i).ty.def().monster || ai(w, i).cube.is_some() || ai(w, i).phantom.is_some() {
        return;
    }
    // Raider.updateNoActionTime (a witch): +2 whatever the light
    if mob(w, i).ty == MobType::Witch {
        mob_mut(w, i).no_action_time += 2;
        return;
    }
    let e = &w.entities.list[i];
    let p = BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y + mob(w, i).eye() as f64), mth::floor(e.pos.z));
    let br = goal::stroll::light_magic(w, p);
    if br > 0.5 {
        mob_mut(w, i).no_action_time += 2;
    }
}

/// `ServerLevel.sendBlockUpdated` (a block set with flag 2): when the collision shape changed,
/// every navigating mob whose path runs near asks for a new one (`shouldRecomputePath`,
/// `recomputePath`; a search per mob, independent of the order).
pub fn send_block_updated(w: &mut World, pos: BlockPos, old: crate::block::Block, new: crate::block::Block) {
    let a = crate::entity::collide::boxes_at(w, &old, pos);
    let b = crate::entity::collide::boxes_at(w, &new, pos);
    if a == b {
        return;
    }
    let mut todo = Vec::new();
    for i in 0..w.entities.list.len() {
        if w.entities.list[i].removed || !has_ai(w, i) {
            continue;
        }
        let nm = nav_mob(w, i);
        if ai(w, i).nav.should_recompute_path(&nm, pos) {
            todo.push(i);
        }
    }
    for i in todo {
        with_nav(w, i, |w, nm, nav, arena| nav.recompute_path(w, nm, arena));
    }
}

/// `setOldPosAndRot`'s head and body part (`commonTick`): the old head and body yaw.
pub fn common_tick(w: &mut World, i: usize) {
    let a = ai_mut(w, i);
    a.y_head_rot_o = a.y_head_rot;
    a.y_body_rot_o = a.y_body_rot;
}

/// `Mob.baseTick`'s ambient sound roll (after `LivingEntity.baseTick`): `nextInt(1000) <
/// ambientSoundTime++`; when it fires, the time resets to `-getAmbientSoundInterval()` and the
/// sound's pitch (`getVoicePitch`: two `nextFloat`) is drawn when the mob has an ambient sound.
pub fn ambient_sound_tick(w: &mut World, i: usize) {
    if w.entities.list[i].removed || mob(w, i).health <= 0.0 {
        return;
    }
    let t = ai(w, i).ambient_sound_time;
    let roll = match w.entities.list[i].random.as_mut() {
        Some(r) => r.next_int(1000),
        None => return,
    };
    ai_mut(w, i).ambient_sound_time = t + 1;
    if roll < t {
        let interval = ambient_interval(w, i);
        ai_mut(w, i).ambient_sound_time = -interval;
        if bat::is_bat(w, i) {
            bat::ambient_sound(w, i);
        } else if has_ambient_sound(w, i) {
            // a wolf's sound is picked with its own draw first (not angry: nextInt(3))
            if goal::wolf::is_wolf(w, i) {
                goal::wolf::ambient_draw(w, i);
            }
            if let Some(r) = w.entities.list[i].random.as_mut() {
                r.next_float();
                r.next_float();
            }
        }
    }
}

/// `getAmbientSoundInterval()`: 120 for an Animal, an AbstractGolem and an AgeableWaterCreature
/// (the squids), else Mob's 80 (`resetAmbientSoundTime` sets the timer to minus it).
pub fn ambient_interval(w: &World, i: usize) -> i32 {
    let id = mob(w, i).ty.def().id;
    // AbstractHorse.getAmbientSoundInterval: 400
    if goal::animal::is_horse(id) {
        return 400;
    }
    // WaterAnimal.getAmbientSoundInterval: 120 (the fish)
    if goal::animal::is_animal(id) || squid::is_squid_id(id) || fish::is_fish_id(id) || mob(w, i).ty == MobType::IronGolem { 120 } else { 80 }
}

/// `getAmbientSound() != null`: zombies, creepers? (no: `Creeper` has none), skeletons, spiders;
/// a villager unless asleep; an iron golem has none.
fn has_ambient_sound(w: &World, i: usize) -> bool {
    let m = mob(w, i);
    match m.ty {
        MobType::IronGolem => false,
        MobType::Villager => m.sleeping_pos.is_none(),
        MobType::Zombie | MobType::Witch => true,
        // the creeper and the cube mobs (AbstractCubeMob: none) have no ambient sound
        MobType::Natural(_) => !matches!(m.ty.def().id, "creeper" | "sulfur_cube" | "sulfur_cube_2" | "slime" | "slime_2" | "slime_4"),
    }
}

impl NavMob {
    pub fn to_path_mob(&self) -> path::PathMob {
        path::PathMob { pos: self.pos, bb: self.bb, width: self.width, height: self.height, max_up_step: self.max_up_step, on_ground: self.on_ground, in_floatable: self.in_floatable, malus: self.malus, max_fall: self.max_fall }
    }
}

/// The eye position of entity `j` (`getEyeY`).
pub fn eye_pos(w: &World, j: usize) -> Vec3 {
    let e = &w.entities.list[j];
    Vec3::new(e.pos.x, e.pos.y + e.kind.eye_height() as f64, e.pos.z)
}

/// `Mob.lookAt(entity, yMax, xMax)` of mob `i` at living entity `j`: the yaw and pitch toward
/// its eye, each turned at most by its maximum (`Mob.rotlerp`).
pub fn mob_look_at(w: &mut World, i: usize, j: usize, y_max: f32, x_max: f32) {
    let (me, it) = (eye_pos(w, i), eye_pos(w, j));
    let (xd, yd, zd) = (it.x - me.x, it.y - me.y, it.z - me.z);
    let sd = (xd * xd + zd * zd).sqrt();
    let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
    let x_rot_d = -control::atan2_degrees(yd, sd);
    let e = &mut w.entities.list[i];
    e.x_rot = mob_rotlerp(e.x_rot, x_rot_d, x_max);
    e.y_rot = mob_rotlerp(e.y_rot, y_rot_d, y_max);
}

/// `Mob.rotlerp(a, b, max)` (not wrapped into [0, 360], unlike `MoveControl.rotlerp`).
fn mob_rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let diff = crate::entity::mth::wrap_degrees(b - a).clamp(-max, max);
    a + diff
}

// ------------------------------------------------------------------ the exact compare's fields

/// The running goals as the bridge prints them (`Exact.entityFrame`): `"<prio>:<Class>"` and
/// `"t<prio>:<Class>"` for the target selector, sorted as Java sorts strings.
pub fn frame_goals(a: &MobAi) -> String {
    let mut v: Vec<String> = a.goals.goals.iter().filter(|g| g.running).map(|g| format!("{}:{}", g.priority, g.goal.name())).collect();
    v.extend(a.targets.goals.iter().filter(|g| g.running).map(|g| format!("t{}:{}", g.priority, g.goal.name())));
    v.sort();
    v.join(" ")
}

/// The path as the bridge prints it: `"path x y z next/count reach|partial"`, or `"none"`.
pub fn frame_path(a: &MobAi) -> String {
    match &a.nav.path {
        None => "none".into(),
        Some(p) => format!("path {} {} {} {}/{} {}", p.target.x, p.target.y, p.target.z, p.next_node_index, p.node_count(), if p.reached { "reach" } else { "partial" }),
    }
}

/// The target as the bridge prints it (`getTarget()`, a valid one): `"#n"` or `"none"`.
pub fn frame_target(w: &World, i: usize) -> String {
    match target::get_target(w, i) {
        Some(id) => frame_ref(id),
        None => "none".into(),
    }
}

/// An entity's reference in the exact frames (the bridge's `Exact.ref`: its number in the
/// sandbox; the AFK player's id is the sandbox's numbering + 2^20 - 4).
pub fn frame_ref(id: u32) -> String {
    if id >= PLAYER_ID_BASE {
        format!("#{}", (1i64 << 20) - 4 - (id - PLAYER_ID_BASE) as i64)
    } else {
        format!("#{id}")
    }
}
