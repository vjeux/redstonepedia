//! (mob-AI 2026-11-10) The cat (26.3 `Cat`, a `TamableAnimal`): a wild one, as the CatSpawner
//! makes them in villages (`natural::spawn_cat`).
//!
//! - Goals (`Cat.registerGoals`): Float 1, TamableAnimalPanic 1 (1.5, `#panic_causes`), LeapAtTarget
//!   8 (0.3F), OcelotAttack 9, Breed 10, WaterAvoidingRandomStroll 11 (0.8, 1.0000001E-5F),
//!   LookAtPlayer 12 (10). A wild cat's never-starting goals are left out: SitWhenOrderedTo 2,
//!   CatRelaxOnOwner 3, CatTempt 4 (no player holds cat food), CatLieOnBlock 5, FollowOwner 6 and
//!   CatSitOnBlock 7 (each tame only, tested first: no draw). Targets: NonTameRandomTarget of
//!   rabbits and of baby turtles on land (1).
//! - `customServerAiStep`: the pose and sprint from the move control: crouching at a speed
//!   modifier of 0.6, sprinting at 1.33 (`setSprinting`: MOVEMENT_SPEED x 1.3F,
//!   `SPEED_MODIFIER_SPRINTING`), else standing; while sprinting (and not crouching, in water or
//!   lava) `Entity.baseTick` spawns a sprint particle: two `nextDouble`s of its own random.
//! - Its ambient sound is the stray one (no draw); its tick's tempt sound and lie-down purr never
//!   play wild.
//! - It despawns far away past 2400 ticks (`natural::removes_far_now`).

use super::super::target::{self, Who};
use super::super::{MobAi, ai, ai_mut, with_nav};
use crate::entity::mob::mob;
use crate::world::World;

/// A cat's own state: the sprint and crouch its `customServerAiStep` set.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CatState {
    pub sprinting: bool,
    pub crouching: bool,
}

pub fn is_cat(w: &World, i: usize) -> bool {
    mob(w, i).ai.as_deref().is_some_and(|a| a.cat.is_some())
}

/// `Cat.registerGoals()` for a wild cat.
pub fn register(a: &mut MobAi) {
    use super::look::{LookAtPlayer, LookType};
    use super::stroll::RandomStroll;
    use super::targets::{NearestAttackableTarget, TargetType};
    use super::Goal;
    let an = |g: super::animal::AnimalGoal| Goal::Animal(g);
    let g = &mut a.goals;
    g.add(1, Goal::Float);
    g.add(1, an(super::animal::AnimalGoal::Panic { speed: 1.5, pos: crate::entity::Vec3::ZERO, mount: false, env: false }));
    g.add(8, Goal::LeapAtTarget(0.3));
    g.add(9, Goal::OcelotAttack(OcelotAttack::default()));
    g.add(10, an(super::animal::AnimalGoal::Breed));
    g.add(11, Goal::RandomStroll(RandomStroll { water_avoiding: Some(1.0000001e-5), ..RandomStroll::water_avoiding(0.8) }));
    g.add(12, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 10.0)));
    let t = &mut a.targets;
    t.add(1, Goal::NearestAttackableTarget(NearestAttackableTarget::non_tame(TargetType::Rabbit)));
    t.add(1, Goal::NearestAttackableTarget(NearestAttackableTarget::non_tame(TargetType::BabyTurtle)));
    a.cat = Some(CatState::default());
}

/// `Cat.customServerAiStep()`: from the move control (`hasWanted`: a MOVE_TO), crouching at 0.6,
/// sprinting at 1.33, else standing and not sprinting.
pub fn custom_server_ai_step(w: &mut World, i: usize) {
    let mc = ai(w, i).move_ctl;
    let wanted = mc.operation == super::super::control::Operation::MoveTo;
    let (crouching, sprinting) = if !wanted {
        (false, false)
    } else if mc.speed_modifier == 0.6 {
        (true, false)
    } else if mc.speed_modifier == 1.33 {
        (false, true)
    } else {
        (false, false)
    };
    ai_mut(w, i).cat = Some(CatState { sprinting, crouching });
}

/// The MOVEMENT_SPEED factor of its sprint (`SPEED_MODIFIER_SPRINTING`, ADD_MULTIPLIED_TOTAL 0.3F).
pub fn speed_factor(w: &World, i: usize) -> f64 {
    match ai(w, i).cat {
        Some(c) if c.sprinting => 1.0 + 0.3f32 as f64,
        _ => 1.0,
    }
}

/// `Entity.canSpawnSprintParticle()`: sprinting, not in water (last tick's), not crouching, not in
/// lava, alive.
pub fn sprint_particle(w: &World, i: usize) -> bool {
    let Some(c) = ai(w, i).cat else { return false };
    let m = mob(w, i);
    c.sprinting && !c.crouching && !w.entities.list[i].was_touching_water && m.lava_height <= 0.0 && m.health > 0.0 && !m.dead
}

/// `OcelotAttackGoal` (the cat's): its target and attack time.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OcelotAttack {
    pub target: Option<u32>,
    pub attack_time: i32,
}

/// `canUse()`: a target (kept).
pub fn ocelot_can_use(w: &mut World, i: usize, g: &mut OcelotAttack) -> bool {
    match target::get_target(w, i) {
        Some(t) => {
            g.target = Some(t);
            true
        }
        None => false,
    }
}

/// `canContinueToUse()`: the kept target alive, within 15, and the navigation going or canUse.
pub fn ocelot_can_continue(w: &mut World, i: usize, g: &mut OcelotAttack) -> bool {
    let Some(t) = g.target.and_then(|id| target::who(w, id)) else { return false };
    if !target::is_alive(w, t) {
        return false;
    }
    if w.entities.list[i].pos.distance_to_sqr(target::pos_of(w, t)) > 225.0 {
        return false;
    }
    !super::super::nav_of(w, i).is_done() || ocelot_can_use(w, i, g)
}

/// `stop()`: no target, the navigation stopped.
pub fn ocelot_stop(w: &mut World, i: usize, g: &mut OcelotAttack) {
    g.target = None;
    with_nav(w, i, |_, _, nav, _| nav.stop());
}

/// `tick()` (every tick): it looks at the target (30, 30); its speed: 1.33 between its melee
/// reach (twice its width, squared) and 4, 0.6 within 15, else 0.8; `moveTo(target, speed)`; the
/// attack time counts down, and in reach at 0 it bites (`doHurtTarget`) and waits 20.
pub fn ocelot_tick(w: &mut World, i: usize, g: &mut OcelotAttack) {
    let Some(t) = g.target.and_then(|id| target::who(w, id)) else { return };
    let te = target::eye_of(w, t);
    ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
    let width = mob(w, i).dims().0;
    let reach = (width * 2.0f32 * width * 2.0f32) as f64;
    let dist = w.entities.list[i].pos.distance_to_sqr(target::pos_of(w, t));
    let speed = if dist > reach && dist < 16.0 {
        1.33
    } else if dist < 225.0 {
        0.6
    } else {
        0.8
    };
    let tb = super::melee::target_block(w, t);
    with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, tb, speed, arena));
    g.attack_time = (g.attack_time - 1).max(0);
    if !(dist > reach) && g.attack_time <= 0 {
        g.attack_time = 20;
        match t {
            // (an entity of another dimension: do_hurt_target names it as not modeled)
            Who::Entity(_) | Who::Foreign(_) => {
                super::super::attack::do_hurt_target(w, i, t);
            }
            Who::Player(_) => {
                let why = "a cat attacking a player is not modeled".to_string();
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
            }
        }
    }
}
