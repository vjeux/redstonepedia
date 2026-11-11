//! The farm animals' goals (`Pig`, `AbstractCow`, `Chicken`, `Sheep`, adults and babies):
//! `PanicGoal` (refused once it would run: a hurt animal), `BreedGoal` (no animal is in love
//! without a player), `TemptGoal` (redsim's players hold nothing), `FollowParentGoal` (a baby walks
//! after the closest adult of its kind within 8 x 4 x 8 when 3 or more away), the sheep's
//! `EatBlockGoal` (short grass or a grass block under it: eaten at the 4th last tick of its 20), the
//! chicken's `aiStep` (falling at 0.6 of its fall speed, an adult laying an egg every 6000..11999
//! ticks), and `AgeableMob.aiStep`'s age (a baby grows up at 0: the adult's size).

use super::{Goal, JUMP, LOOK, MOVE, reduced_tick_delay};
use super::wolf::WolfState;
use crate::block::Block;
use crate::entity::EntityKind;
use crate::entity::mob::ai::{ai, ai_mut};
use crate::entity::mob::types::MobType;
use crate::entity::mob::{Damage, mob, mob_mut};
use crate::entity::Vec3;
use crate::pos::BlockPos;
use crate::world::World;

#[derive(Clone, PartialEq, Debug)]
pub enum AnimalGoal {
    /// `PanicGoal`: its speed, the position it runs to (`mount`: a horse's `MountPanicGoal`, not
    /// while a mob rides and steers it; `env`: a tamable animal's `TamableAnimalPanicGoal` of
    /// `#panic_environmental_causes` alone: the wolf's).
    Panic { speed: f64, pos: crate::entity::Vec3, mount: bool, env: bool },
    Breed,
    Tempt,
    /// `FollowParentGoal`: its speed, the parent and `timeToRecalcPath`.
    FollowParent { speed: f64, parent: Option<u32>, recalc: i32 },
    /// `EatBlockGoal.eatAnimationTick`.
    EatBlock { tick: i32 },
    /// A horse's `RunAroundLikeCrazyGoal` (an untamed horse with a rider: never without one).
    RunAround,
    /// A horse's `RandomStandGoal` and its `nextStand`.
    RandomStand { next: i32 },
}

impl AnimalGoal {
    pub fn flags(&self) -> u8 {
        match self {
            AnimalGoal::Panic { .. } | AnimalGoal::RunAround => MOVE,
            AnimalGoal::Breed | AnimalGoal::Tempt => MOVE | LOOK,
            AnimalGoal::FollowParent { .. } | AnimalGoal::RandomStand { .. } => 0,
            AnimalGoal::EatBlock { .. } => MOVE | LOOK | JUMP,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AnimalGoal::Panic { mount: false, .. } => "PanicGoal",
            AnimalGoal::Panic { mount: true, .. } => "MountPanicGoal",
            AnimalGoal::Breed => "BreedGoal",
            AnimalGoal::Tempt => "TemptGoal",
            AnimalGoal::FollowParent { .. } => "FollowParentGoal",
            AnimalGoal::EatBlock { .. } => "EatBlockGoal",
            AnimalGoal::RunAround => "RunAroundLikeCrazyGoal",
            AnimalGoal::RandomStand { .. } => "RandomStandGoal",
        }
    }
}

/// A horse's state (`AbstractHorse`): its attributes as `randomizeAttributes` drew them, eating
/// and standing (both make it immobile: no AI step), the tail's counter.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Horse {
    /// MOVEMENT_SPEED's base.
    pub speed: f64,
    /// JUMP_STRENGTH's base.
    pub jump: f64,
    /// MAX_HEALTH's base.
    pub max_health: f32,
    pub eating: bool,
    pub eating_counter: i32,
    pub stand_counter: i32,
    pub tail_counter: i32,
    /// `standAnim` and `standAnimO` (a rider's seat moves with them).
    pub stand_anim: f32,
    pub stand_anim_o: f32,
}

/// Is this a horse (`AbstractHorse`: horse, donkey, zombie or skeleton horse, a baby too; a trader
/// llama: mob-AI 2026-11-11)?
pub fn is_horse(id: &str) -> bool {
    matches!(adult_of(id), Some("horse" | "donkey" | "zombie_horse" | "skeleton_horse" | "trader_llama"))
}

/// `AbstractHorse.isImmobile`: eating or standing (dead: LivingEntity's, handled by the caller).
pub fn horse_immobile(w: &World, i: usize) -> bool {
    mob(w, i).ai.as_deref().and_then(|a| a.horse).is_some_and(|h| h.eating || h.stand_counter > 0)
}

/// `AbstractHorse.standIfPossible` (`canPerformRearing`: every horse here but a llama):
/// `setStanding(20)`, which stops its eating.
pub fn stand_if_possible(w: &mut World, i: usize) {
    if adult_of(mob(w, i).ty.def().id) == Some("trader_llama") {
        return;
    }
    if let Some(h) = mob_mut(w, i).ai.as_deref_mut().and_then(|a| a.horse.as_mut()) {
        h.eating = false;
        h.stand_counter = 20;
    }
}

/// `AbstractHorse.aiStep`'s first line, before `super.aiStep()`: `nextInt(200) == 0` moves the
/// tail.
pub fn horse_before_ai_step(w: &mut World, i: usize) {
    if !is_horse(mob(w, i).ty.def().id) {
        return;
    }
    if own(w, i).next_int(200) == 0
        && let Some(h) = mob_mut(w, i).ai.as_deref_mut().and_then(|a| a.horse.as_mut())
    {
        h.tail_counter = 1;
    }
}

/// `AbstractHorse.aiStep` after `super.aiStep()` (alive): 1 health in 900 (not dying), the grass
/// eating (`canEatGrass`: not eating, not ridden, 1 in 300, on a grass block: 50 ticks of it);
/// `followMommy` only for a bred baby (none here).
fn horse_after_ai_step(w: &mut World, i: usize) {
    if own(w, i).next_int(900) == 0 && mob(w, i).death_time == 0 {
        let max = mob(w, i).ai.as_deref().and_then(|a| a.horse).map(|h| h.max_health).unwrap_or(mob(w, i).ty.def().max_health);
        let m = mob_mut(w, i);
        m.health = (m.health + 1.0).min(max);
    }
    let vehicle = !w.entities.list[i].passengers.is_empty();
    let Some(h) = mob(w, i).ai.as_deref().and_then(|a| a.horse) else { return };
    // canEatGrass: a llama's is false (no roll, no eating)
    if adult_of(mob(w, i).ty.def().id) == Some("trader_llama") {
        return;
    }
    if !h.eating && !vehicle && own(w, i).next_int(300) == 0 {
        let below = w.entities.list[i].block_position().below();
        let grass = match w.get(below) {
            Block::Simple(crate::block::Simple::GrassBlock) => true,
            Block::Terrain(id) => crate::terrain::blocks::props(id).block_id() == "minecraft:grass_block",
            _ => false,
        };
        if grass && let Some(h) = mob_mut(w, i).ai.as_deref_mut().and_then(|a| a.horse.as_mut()) {
            h.eating = true;
        }
    }
    if let Some(h) = mob_mut(w, i).ai.as_deref_mut().and_then(|a| a.horse.as_mut())
        && h.eating
    {
        h.eating_counter += 1;
        if h.eating_counter > 50 {
            h.eating_counter = 0;
            h.eating = false;
        }
    }
}

/// `AbstractHorse.tick` after `super.tick()`: the stand and tail counters, then `standAnim`
/// (towards 1 while standing, else `(0.8 s^3 - s) x 0.6 - 0.05` down to 0).
pub fn horse_after_tick(w: &mut World, i: usize) {
    if let Some(h) = mob_mut(w, i).ai.as_deref_mut().and_then(|a| a.horse.as_mut()) {
        if h.stand_counter > 0 {
            h.stand_counter -= 1;
        }
        if h.tail_counter > 0 {
            h.tail_counter += 1;
            if h.tail_counter > 8 {
                h.tail_counter = 0;
            }
        }
        h.stand_anim_o = h.stand_anim;
        let s = h.stand_anim;
        if h.stand_counter > 0 {
            h.stand_anim = (s + ((1.0 - s) * 0.4 + 0.05)).min(1.0);
        } else {
            h.stand_anim = (s + ((0.8 * s * s * s - s) * 0.6 - 0.05)).max(0.0);
        }
    }
}

/// `AbstractHorse.getPassengerAttachmentPoint`'s stand shift: `(0, 0.15 standAnimO, -0.7
/// standAnimO).yRot(-yRot)` (an adult: scale 1).
pub fn horse_seat_shift(w: &World, vi: usize) -> crate::entity::Vec3 {
    let Some(h) = mob(w, vi).ai.as_deref().and_then(|a| a.horse) else { return crate::entity::Vec3::ZERO };
    let s = h.stand_anim_o as f64;
    if s == 0.0 {
        return crate::entity::Vec3::ZERO;
    }
    // x the scale: getScale() x getAgeScale() (a baby's 0.5)
    let scale = if is_baby(mob(w, vi).ty.def().id) { 0.5f32 } else { 1.0f32 } as f64;
    let (y, z) = (0.15 * s * scale, -0.7 * s * scale);
    let a = -w.entities.list[vi].y_rot * (std::f32::consts::PI / 180.0);
    let (c, sn) = (crate::entity::mth::cos(a as f64) as f64, crate::entity::mth::sin(a as f64) as f64);
    crate::entity::Vec3::new(z * sn, y, z * c)
}

/// The adult kind of a farm animal row (a baby's adult), or `None`.
pub fn adult_of(id: &str) -> Option<&'static str> {
    match id {
        "pig" | "baby_pig" => Some("pig"),
        "cow" | "baby_cow" => Some("cow"),
        "chicken" | "baby_chicken" => Some("chicken"),
        "sheep" | "baby_sheep" => Some("sheep"),
        "horse" | "baby_horse" => Some("horse"),
        "donkey" | "baby_donkey" => Some("donkey"),
        "zombie_horse" | "baby_zombie_horse" => Some("zombie_horse"),
        "skeleton_horse" => Some("skeleton_horse"),
        // (mob-AI 2026-11-10) the wolf (a TamableAnimal: an Animal; natural wolves are adults)
        "wolf" | "baby_wolf" => Some("wolf"),
        // (mob-AI 2026-11-10) a wild cat (a TamableAnimal; the CatSpawner's are adults)
        "cat" => Some("cat"),
        // (mob-AI 2026-11-11) a trader llama (an AbstractChestedHorse; the spawner's are adults)
        "trader_llama" => Some("trader_llama"),
        _ => None,
    }
}

/// Is this a farm animal (adult or baby) whose goals are modeled?
pub fn is_animal(id: &str) -> bool {
    adult_of(id).is_some()
}

/// Is this farm animal a baby?
pub fn is_baby(id: &str) -> bool {
    is_animal(id) && id.starts_with("baby_")
}

/// `registerGoals` of the pig, cow, chicken and sheep (a baby's are its adult's).
pub fn register(a: &mut crate::entity::mob::ai::MobAi, id: &str) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::stroll::RandomStroll;
    let g = &mut a.goals;
    let an = |k: AnimalGoal| Goal::Animal(k);
    let follow = |speed: f64| Goal::Animal(AnimalGoal::FollowParent { speed, parent: None, recalc: 0 });
    match adult_of(id) {
        Some("pig") => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic { speed: 1.25, pos: crate::entity::Vec3::ZERO, mount: false, env: false }));
            g.add(3, an(AnimalGoal::Breed));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(5, follow(1.1));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        Some(k @ ("cow" | "chicken")) => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic { speed: if k == "cow" { 2.0 } else { 1.4 }, pos: crate::entity::Vec3::ZERO, mount: false, env: false }));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, follow(if k == "cow" { 1.25 } else { 1.1 }));
            g.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(7, Goal::RandomLookAround(RandomLookAround::default()));
        }
        Some("sheep") => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic { speed: 1.25, pos: crate::entity::Vec3::ZERO, mount: false, env: false }));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, follow(1.1));
            g.add(5, an(AnimalGoal::EatBlock { tick: 0 }));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        // Wolf.registerGoals (an untamed wolf's: SitWhenOrderedTo 2, FollowOwner 6, OwnerHurtBy 1
        // and OwnerHurt 1-2 never run untamed; WolfAvoidEntity<Llama> 3 finds no llama; Beg 9 no
        // player holding a bone or meat; ResetUniversalAnger 8 never without the universal-anger
        // rule): Float 1, TamableAnimalPanic 1 (1.5, environmental causes), LeapAtTarget 4 (0.4),
        // MeleeAttack 5 (1.0, following unseen), Breed 7, WaterAvoidingRandomStroll 8 (1.0),
        // LookAtPlayer 10 (8), RandomLookAround 10; the targets HurtBy 3 (alerting wolves), the
        // player it is angry at 4, prey 5 (sheep, rabbits, foxes), baby turtles 6, skeletons 7
        Some("wolf") => {
            use super::melee::{MeleeAttack, MeleeKind};
            use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
            g.add(1, Goal::Float);
            g.add(1, an(AnimalGoal::Panic { speed: 1.5, pos: crate::entity::Vec3::ZERO, mount: false, env: true }));
            g.add(4, Goal::LeapAtTarget(0.4));
            g.add(5, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Melee, 1.0, true)));
            g.add(7, an(AnimalGoal::Breed));
            g.add(8, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(10, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
            g.add(10, Goal::RandomLookAround(RandomLookAround::default()));
            let t = &mut a.targets;
            t.add(3, Goal::HurtByTarget(HurtByTarget::new(true)));
            t.add(4, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::AngryPlayer, 10, true, false)));
            t.add(5, Goal::NearestAttackableTarget(NearestAttackableTarget::non_tame(TargetType::WolfPrey)));
            t.add(6, Goal::NearestAttackableTarget(NearestAttackableTarget::non_tame(TargetType::BabyTurtle)));
            t.add(7, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Skeleton, 10, false, false)));
            // the wolf's constructor: POWDER_SNOW and ON_TOP_OF_POWDER_SNOW -1
            a.malus.set(crate::entity::mob::ai::path::path_type::PathType::PowderSnow, -1.0);
            a.malus.set(crate::entity::mob::ai::path::path_type::PathType::OnTopOfPowderSnow, -1.0);
            a.wolf = Some(WolfState::default());
        }
        // (mob-AI 2026-11-10) a wild cat (goal::cat)
        Some("cat") => super::cat::register(a),
        // AbstractHorse.registerGoals, then addBehaviourGoals (the goal selector keeps insertion
        // order): RunAroundLikeCrazy 1, Breed 2, FollowParent 4 (1.0), WaterAvoidingRandomStroll 6
        // (0.7), LookAtPlayer 7 (6), RandomLookAround 8, RandomStand 9 (canPerformRearing: every
        // horse here; nextStand starts at -400, the ambient stand interval); then the horse's and
        // the donkey's Float 0, MountPanic 1 (1.2), Tempt 3 (1.25); the zombie horse's Float 0,
        // Tempt 3; the skeleton horse's none
        Some(k @ ("horse" | "donkey" | "zombie_horse" | "skeleton_horse")) => {
            g.add(1, an(AnimalGoal::RunAround));
            g.add(2, an(AnimalGoal::Breed));
            g.add(4, follow(1.0));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(0.7)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
            g.add(9, an(AnimalGoal::RandomStand { next: -400 }));
            match k {
                "horse" | "donkey" => {
                    g.add(0, Goal::Float);
                    g.add(1, an(AnimalGoal::Panic { speed: 1.2, pos: crate::entity::Vec3::ZERO, mount: true, env: false }));
                    g.add(3, an(AnimalGoal::Tempt));
                }
                "zombie_horse" => {
                    g.add(0, Goal::Float);
                    g.add(3, an(AnimalGoal::Tempt));
                }
                _ => {}
            }
            // the base attributes (createBaseHorseAttributes: MAX_HEALTH 53, MOVEMENT_SPEED 0.225F,
            // JUMP_STRENGTH 0.7; a donkey's 0.175F and 0.5; a zombie horse's 25 and 0.2F; a
            // skeleton horse's 15 and 0.2F) until finalizeSpawn's randomizeAttributes
            let (max_health, speed, jump) = match k {
                "horse" => (53.0, 0.225f32 as f64, 0.7),
                "donkey" => (53.0, 0.175f32 as f64, 0.5),
                "zombie_horse" => (25.0, 0.225f32 as f64, 0.7),
                _ => (15.0, 0.2f32 as f64, 0.7),
            };
            a.horse = Some(Horse { speed, jump, max_health, ..Horse::default() });
        }
        _ => {}
    }
    // a natural baby spawns at age -24000 (AgeableMob.finalizeSpawn); a summon's data sets it
    a.age = if is_baby(id) { -24000 } else { 0 };
}

fn refuse(w: &mut World, why: &str) {
    if !w.errors.iter().any(|e| e == why) {
        w.errors.push(why.to_string());
    }
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

/// The mob's own random (for the mob layer's horse hooks).
pub fn own_random(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    own(w, i)
}

fn is_grass_block(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).block_id() == "minecraft:grass_block",
        _ => b.to_string().starts_with("minecraft:grass_block"),
    }
}

fn is_short_grass(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).block_id() == "minecraft:short_grass",
        _ => b.to_string().starts_with("minecraft:short_grass"),
    }
}

/// `Animal.getWalkTargetValue`: 10 on a grass block, else the light's cost
/// (`getPathfindingCostFromLightLevels`: the magic value - 0.5).
pub fn walk_target_value(w: &mut World, p: BlockPos) -> f64 {
    if is_grass_block(&w.get(p.below())) {
        return 10.0;
    }
    (super::stroll::light_magic(w, p) - 0.5f32) as f64
}

/// `AgeableMob.getAge()`.
fn age(w: &World, i: usize) -> i32 {
    ai(w, i).age
}

/// `FollowParentGoal.canUse`: a baby; the closest adult of its class in its box inflated 8, 4, 8
/// (`!(d > closest)`: the last of equals), 3 or more blocks away.
fn follow_can_use(w: &mut World, i: usize, parent: &mut Option<u32>) -> bool {
    if age(w, i) >= 0 {
        return false;
    }
    let kind = adult_of(mob(w, i).ty.def().id);
    let (me, pos) = (w.entities.list[i].id, w.entities.list[i].pos);
    let query = w.entities.list[i].bb.inflate(8.0, 4.0, 8.0);
    let found = w.get_entities(query, |e| matches!(&e.kind, EntityKind::Mob(m) if adult_of(m.ty.def().id) == kind));
    let mut closest: Option<(u32, f64)> = None;
    for j in found {
        let e = &w.entities.list[j];
        let adult = match &e.kind {
            EntityKind::Mob(m) => m.ai.as_deref().map_or(!is_baby(m.ty.def().id), |a| a.age >= 0),
            _ => false,
        };
        if !adult {
            continue;
        }
        let d = pos.distance_to_sqr(e.pos);
        if closest.is_none_or(|(_, c)| !(d > c)) {
            closest = Some((e.id, d));
        }
    }
    let _ = me;
    match closest {
        Some((id, d)) if d >= 9.0 => {
            *parent = Some(id);
            true
        }
        _ => false,
    }
}

/// `FollowParentGoal.canContinueToUse`: still a baby, the parent alive, between 3 and 16 away.
fn follow_can_continue(w: &mut World, i: usize, parent: Option<u32>) -> bool {
    if age(w, i) >= 0 {
        return false;
    }
    let Some(j) = parent.and_then(|p| crate::entity::index_of(w, p)) else { return false };
    let alive = matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.health > 0.0) && !w.entities.list[j].removed;
    if !alive {
        return false;
    }
    let d = w.entities.list[i].pos.distance_to_sqr(w.entities.list[j].pos);
    !(d < 9.0) && !(d > 256.0)
}

pub fn can_use(w: &mut World, i: usize, g: &mut AnimalGoal) -> bool {
    match g {
        AnimalGoal::Panic { speed, pos, mount, env } => {
            // PanicGoal.canUse: shouldPanic (the last damage within 40 ticks, of #panic_causes;
            // a wolf's: of #panic_environmental_causes; MountPanicGoal: not while a mob rides and
            // steers it); burning, the closest water within 5 (lookForWater, from a cell without
            // a collision shape); else DefaultRandomPos.getPos(5, 4)
            let now = w.time;
            let env_only = *env;
            let panics = mob(w, i).last_damage.is_some_and(|(t, s)| {
                now - t <= 40
                    && (matches!(s, Damage::Lava | Damage::OnFire | Damage::HotFloor | Damage::Lightning | Damage::InFire)
                        || (!env_only && matches!(s, Damage::MobAttack | Damage::Arrow | Damage::Spear | Damage::Trident)))
            });
            if !panics || (*mount && crate::entity::mob::ai::controlled_by_mob(w, i)) {
                return false;
            }
            let _ = speed;
            if mob(w, i).fire_ticks > 0
                && let Some(p) = look_for_water(w, i, 5)
            {
                *pos = Vec3::new(p.x as f64, p.y as f64, p.z as f64);
                return true;
            }
            match super::stroll::default_pos(w, i, 5, 4) {
                Some(v) => {
                    *pos = v;
                    true
                }
                None => false,
            }
        }
        AnimalGoal::Breed | AnimalGoal::Tempt => false,
        // RunAroundLikeCrazyGoal.canUse: untamed with a rider (a player's taming; a mob rider
        // steers it, its MOVE flag off, so this is never asked then)
        AnimalGoal::RunAround => {
            if !w.entities.list[i].passengers.is_empty() {
                w.mob.ai.legacy.insert("a ridden horse running around (RunAroundLikeCrazyGoal)");
            }
            false
        }
        // RandomStandGoal.canUse: nextStand counts up from -400; past 0, nextInt(1000) < it resets
        // it and stands 1 in 10 when the horse can move
        AnimalGoal::RandomStand { next } => {
            *next += 1;
            if *next > 0 && own(w, i).next_int(1000) < *next {
                *next = -400;
                !horse_immobile(w, i) && own(w, i).next_int(10) == 0
            } else {
                false
            }
        }
        AnimalGoal::FollowParent { parent, .. } => follow_can_use(w, i, parent),
        AnimalGoal::EatBlock { .. } => {
            // nextInt(adjustedTickDelay(a baby's 50, an adult's 1000)), then short grass at its
            // feet or a grass block under them
            let bound = reduced_tick_delay(if age(w, i) < 0 { 50 } else { 1000 });
            if own(w, i).next_int(bound) != 0 {
                return false;
            }
            let p = w.entities.list[i].block_position();
            is_short_grass(&w.get(p)) || is_grass_block(&w.get(p.below()))
        }
    }
}

/// `PanicGoal.lookForWater(level, mob, xz)`: from a cell without a collision shape, the first water
/// cell of `BlockPos.findClosestMatch(pos, xz, 1)` (`withinManhattan`'s order).
fn look_for_water(w: &World, i: usize, xz: i32) -> Option<BlockPos> {
    let me = w.entities.list[i].block_position();
    if !crate::entity::collide::boxes_at(w, &w.get(me), me).is_empty() {
        return None;
    }
    within_manhattan(me, xz, 1, xz).into_iter().find(|p| crate::fluid::fluid_state(w, *p).fluid.is_water())
}

/// `BlockPos.withinManhattan(origin, rx, ry, rz)`: by Manhattan depth, then x from -maxX, y from
/// -maxY, each z and its mirror.
pub fn within_manhattan(o: BlockPos, rx: i32, ry: i32, rz: i32) -> Vec<BlockPos> {
    let mut out = Vec::new();
    let max_depth = rx + ry + rz;
    for depth in 0..=max_depth {
        let max_x = rx.min(depth);
        for x in -max_x..=max_x {
            let max_y = ry.min(depth - x.abs());
            for y in -max_y..=max_y {
                let z = depth - x.abs() - y.abs();
                if z <= rz {
                    out.push(BlockPos::new(o.x + x, o.y + y, o.z + z));
                    if z != 0 {
                        out.push(BlockPos::new(o.x + x, o.y + y, o.z - z));
                    }
                }
            }
        }
    }
    out
}

pub fn can_continue(w: &mut World, i: usize, g: &mut AnimalGoal) -> bool {
    match g {
        AnimalGoal::EatBlock { tick } => *tick > 0,
        AnimalGoal::FollowParent { parent, .. } => follow_can_continue(w, i, *parent),
        // PanicGoal.canContinueToUse: the navigation still going
        AnimalGoal::Panic { .. } => !crate::entity::mob::ai::nav_of(w, i).is_done(),
        // RandomStandGoal.canContinueToUse: false (it stands in start, then stops)
        AnimalGoal::RandomStand { .. } => false,
        _ => can_use(w, i, g),
    }
}

pub fn start(w: &mut World, i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::EatBlock { tick } => {
            // adjustedTickDelay(40) (not every tick: 20), the entity event, the navigation stopped
            *tick = reduced_tick_delay(40);
            crate::entity::mob::ai::with_nav(w, i, |_, _, nav, _| nav.stop());
        }
        AnimalGoal::FollowParent { recalc, .. } => *recalc = 0,
        // PanicGoal.start: navigation.moveTo(the position, its speed)
        AnimalGoal::Panic { speed, pos, .. } => {
            let (p, s) = (*pos, *speed);
            crate::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, s, arena));
        }
        // RandomStandGoal.start: standIfPossible (its stand sound draws nothing)
        AnimalGoal::RandomStand { .. } => stand_if_possible(w, i),
        _ => {}
    }
}

pub fn stop(_w: &mut World, _i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::EatBlock { tick } => *tick = 0,
        AnimalGoal::FollowParent { parent, .. } => *parent = None,
        _ => {}
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::FollowParent { speed, parent, recalc } => {
            *recalc -= 1;
            if *recalc <= 0 {
                *recalc = reduced_tick_delay(10);
                if let Some(j) = parent.and_then(|p| crate::entity::index_of(w, p)) {
                    // moveTo(parent, speed): createPath(its block position, 1)
                    let (s, tb) = (*speed, w.entities.list[j].block_position());
                    crate::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, tb, s, arena));
                }
            }
        }
        AnimalGoal::EatBlock { tick } => eat_tick(w, i, tick),
        _ => {}
    }
}

fn eat_tick(w: &mut World, i: usize, tick: &mut i32) {
    *tick = (*tick - 1).max(0);
    if *tick != reduced_tick_delay(4) {
        return;
    }
    let p = w.entities.list[i].block_position();
    let griefing = w.mob.ai.mob_griefing;
    if is_short_grass(&w.get(p)) {
        if griefing {
            // destroyBlock(pos, false): no drops
            w.destroy_block(p, false, 512);
        }
        ate(w, i);
    } else if is_grass_block(&w.get(p.below())) {
        if griefing {
            let dirt = Block::parse("minecraft:dirt").expect("dirt");
            w.set_block(p.below(), dirt, 2);
        }
        ate(w, i);
    }
}

/// `Sheep.ate()`: its wool back; a baby ages 60 s (`ageUp(60)`: +1200, at most 0).
fn ate(w: &mut World, i: usize) {
    let a = age(w, i);
    if a < 0 {
        set_age(w, i, (a + 1200).min(0));
    }
}

/// `AgeableMob.setAge`: crossing 0 up grows the baby into its adult (`refreshDimensions`: the
/// adult's box at the same feet; `fudgePositionAfterSizeChange` keeps them where nothing
/// collides, the only case modeled).
fn set_age(w: &mut World, i: usize, new: i32) {
    let old = age(w, i);
    ai_mut(w, i).age = new;
    if old < 0 && new >= 0 {
        grow_up(w, i);
    }
}

fn grow_up(w: &mut World, i: usize) {
    let Some(adult) = adult_of(mob(w, i).ty.def().id) else { return };
    let Some(ty) = MobType::from_id(adult) else { return };
    let (ow, oh) = mob(w, i).dims();
    mob_mut(w, i).ty = ty;
    // refreshDimensions: the adult's box at the same feet (reapplyPosition), then
    // fudgePositionAfterSizeChange (a grown box finds the free position closest to its old centre)
    let pos = w.entities.list[i].pos;
    let (wd, h) = mob(w, i).dims();
    let bb = crate::entity::make_bounding_box(pos, wd, h);
    w.entities.list[i].bb = bb;
    if !w.entities.list[i].first_tick {
        crate::entity::fudge_position_after_size_change(w, i, ow, oh);
    }
}

/// `AgeableMob.aiStep`'s age after `LivingEntity.aiStep`: a baby's counts up to 0 (an adult's
/// breeding cooldown down to 0); then the chicken's tail.
pub fn after_ai_step(w: &mut World, i: usize) {
    if !is_animal(mob(w, i).ty.def().id) || mob(w, i).health <= 0.0 || w.entities.list[i].removed {
        return;
    }
    let a = age(w, i);
    if a < 0 {
        set_age(w, i, a + 1);
    } else if a > 0 {
        set_age(w, i, a - 1);
    }
    chicken_after_ai_step(w, i);
    if is_horse(mob(w, i).ty.def().id) {
        horse_after_ai_step(w, i);
    }
}

/// `Chicken`'s constructor draw: `eggTime = nextInt(6000) + 6000` (after the entity's own).
pub fn chicken_egg_time(r: &mut crate::rng::LegacyRandom) -> i32 {
    r.next_int(6000) + 6000
}

/// `Chicken.aiStep` after `super.aiStep()`: a falling chicken keeps 0.6 of its fall speed; an
/// adult lays an egg when its timer runs out (the CHICKEN_LAY gift table's egg, its sound's
/// pitch, a new timer; a baby's timer stands still).
fn chicken_after_ai_step(w: &mut World, i: usize) {
    if adult_of(mob(w, i).ty.def().id) != Some("chicken") {
        return;
    }
    let e = &mut w.entities.list[i];
    if !e.on_ground && e.delta_movement.y < 0.0 {
        let d = e.delta_movement;
        e.delta_movement = crate::entity::Vec3::new(d.x, d.y * 0.6, d.z);
    }
    // (ServerLevel, alive) not a baby, not a chicken jockey, then --eggTime <= 0
    if age(w, i) < 0 || mob(w, i).chicken_jockey {
        return;
    }
    let t = mob(w, i).egg_time - 1;
    mob_mut(w, i).egg_time = t;
    if t <= 0 {
        // dropFromGiftLootTable(CHICKEN_LAY): the variant's egg (a temperate chicken's: the
        // plains samples'), no draw in the table; spawnAtLocation; the sound's pitch (two floats)
        crate::entity::mob::living::spawn_item(w, i, "minecraft:egg", 1);
        let r = own(w, i);
        r.next_float();
        r.next_float();
        let next = chicken_egg_time(own(w, i));
        mob_mut(w, i).egg_time = next;
    }
}
