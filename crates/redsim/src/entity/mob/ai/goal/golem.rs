//! The iron golem's goals (`IronGolem.registerGoals`): the melee attack (following an unseen
//! target), moving toward its target, back to the village, strolling in the village (its choices
//! drawn from the level random), offering a flower, the looks; its targets (defending the village
//! from players the villagers hate, what hurt it, players it is angry at, enemies but creepers).

use super::look::{LookAtPlayer, LookType, RandomLookAround};
use super::melee::{MeleeAttack, MeleeKind};
use super::stroll::{self, RandomStroll};
use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
use super::{Goal, reduced_tick_delay};
use crate::entity::Vec3;
use crate::entity::mob::ai::target::{self, Who};
use crate::entity::mob::ai::{MobAi, ai, poi, with_nav};
use crate::entity::mob::types::MobType;
use crate::pos::BlockPos;
use crate::world::World;

/// `IronGolem.registerGoals()`.
pub fn register(ai: &mut MobAi) {
    ai.goals.add(1, Goal::MeleeAttack(MeleeAttack::new(MeleeKind::Melee, 1.0, true)));
    ai.goals.add(2, Goal::MoveTowardsTarget(MoveTowardsTarget { speed: 0.9, within: 32.0, target: None, wanted: Vec3::ZERO }));
    ai.goals.add(2, Goal::RandomStroll(RandomStroll { speed_modifier: 0.6, interval: 10, force_trigger: false, check_no_action_time: false, wanted: Vec3::ZERO, water_avoiding: None, variant: stroll::Variant::MoveBackToVillage }));
    ai.goals.add(4, Goal::RandomStroll(RandomStroll { speed_modifier: 0.6, interval: 240, force_trigger: false, check_no_action_time: false, wanted: Vec3::ZERO, water_avoiding: None, variant: stroll::Variant::GolemInVillage }));
    ai.goals.add(5, Goal::OfferFlower(OfferFlower::default()));
    ai.goals.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
    ai.goals.add(8, Goal::RandomLookAround(RandomLookAround::default()));
    ai.targets.add(1, Goal::DefendVillage);
    ai.targets.add(2, Goal::HurtByTarget(HurtByTarget::new(false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::AngryPlayer, 10, true, false)));
    ai.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::EnemyNotCreeper, 5, false, false)));
    ai.targets.add(4, Goal::ResetUniversalAnger);
}

// ----------------------------------------------------------------------- toward the target

/// `MoveTowardsTargetGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct MoveTowardsTarget {
    pub speed: f64,
    pub within: f32,
    pub target: Option<u32>,
    pub wanted: Vec3,
}

pub fn towards_can_use(w: &mut World, i: usize, g: &mut MoveTowardsTarget) -> bool {
    g.target = target::get_target(w, i);
    let Some(t) = g.target.and_then(|id| target::who(w, id)) else { return false };
    let tp = target::pos_of(w, t);
    if tp.distance_to_sqr(w.entities.list[i].pos) > (g.within * g.within) as f64 {
        return false;
    }
    let Some(p) = stroll::default_pos_towards(w, i, 16, 7, tp, (std::f64::consts::PI / 2.0) as f32 as f64) else { return false };
    g.wanted = p;
    true
}

pub fn towards_can_continue(w: &mut World, i: usize, g: &mut MoveTowardsTarget) -> bool {
    if ai(w, i).nav.is_done() {
        return false;
    }
    let Some(t) = g.target.and_then(|id| target::who(w, id)) else { return false };
    target::is_alive(w, t) && target::pos_of(w, t).distance_to_sqr(w.entities.list[i].pos) < (g.within * g.within) as f64
}

pub fn towards_start(w: &mut World, i: usize, g: &mut MoveTowardsTarget) {
    let (p, s) = (g.wanted, g.speed);
    with_nav(w, i, |w, nm, nav, arena| nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, s, arena));
}

// ----------------------------------------------------------------------- strolls

fn level_rng(w: &mut World) -> &mut crate::rng::LegacyRandom {
    crate::entity::mob::ai::brain::run::level_rng(w)
}

/// `MoveBackToVillageGoal.canUse`'s gate: not in a village.
pub fn back_to_village_gate(w: &World, i: usize) -> bool {
    !poi::is_village(w, w.entities.list[i].block_position())
}

/// `MoveBackToVillageGoal.getPosition()`: toward the section closest to the village within 2.
pub fn back_to_village_position(w: &mut World, i: usize) -> Option<Vec3> {
    let p = w.entities.list[i].block_position();
    let s = (p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16));
    let best = poi::section_closest_to_village(w, s, 2);
    if best == s {
        return None;
    }
    let c = Vec3::new((best.0 * 16 + 8) as f64 + 0.5, (best.1 * 16 + 8) as f64, (best.2 * 16 + 8) as f64 + 0.5);
    stroll::default_pos_towards(w, i, 10, 7, c, (std::f64::consts::PI / 2.0) as f32 as f64)
}

/// `GolemRandomStrollInVillageGoal.getPosition()` (its choices from the level random).
pub fn golem_village_position(w: &mut World, i: usize) -> Option<Vec3> {
    if level_rng(w).next_float() < 0.3 {
        return stroll::land_random_pos(w, i, 10, 7);
    }
    let target = if level_rng(w).next_float() < 0.7 {
        towards_villager_who_wants_golem(w, i).or_else(|| towards_poi(w, i))
    } else {
        towards_poi(w, i).or_else(|| towards_villager_who_wants_golem(w, i))
    };
    target.or_else(|| stroll::land_random_pos(w, i, 10, 7))
}

/// `LandRandomPos.getPosTowards(mob, h, v, towardsPos)`.
fn land_pos_towards(w: &mut World, i: usize, h: i32, v: i32, towards: Vec3) -> Option<Vec3> {
    let me = w.entities.list[i].pos;
    // getPosAway(mob, 0, h, v, mob.position() - (towards - mob.position())): toward = away from the
    // mirror point; LandRandomPos.getPosTowards uses getPosInDirection with the direction itself
    stroll::land_pos_in_direction(w, i, 0.0, h as f64, v, Vec3::new(towards.x - me.x, towards.y - me.y, towards.z - me.z))
}

fn towards_villager_who_wants_golem(w: &mut World, i: usize) -> Option<Vec3> {
    let t = w.time;
    let area = w.entities.list[i].bb.inflate(32.0, 32.0, 32.0);
    let villagers: Vec<Who> = target::mobs_in(w, area, |ty| ty == MobType::Villager)
        .into_iter()
        .filter(|v| match v {
            Who::Entity(j) => crate::entity::mob::mob(w, *j).brain.as_deref().is_some_and(|b| b.wants_golem(t)),
            _ => false,
        })
        .collect();
    if villagers.is_empty() {
        return None;
    }
    let k = level_rng(w).next_int(villagers.len() as i32) as usize;
    let tp = target::pos_of(w, villagers[k]);
    land_pos_towards(w, i, 10, 7, tp)
}

fn towards_poi(w: &mut World, i: usize) -> Option<Vec3> {
    let p = w.entities.list[i].block_position();
    let s = (p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16));
    let mut sections = Vec::new();
    let v = poi::village_sections(w);
    for z in s.2 - 2..=s.2 + 2 {
        for y in s.1 - 2..=s.1 + 2 {
            for x in s.0 - 2..=s.0 + 2 {
                if poi::sections_to_village_in(&v, (x, y, z)) == 0 {
                    sections.push((x, y, z));
                }
            }
        }
    }
    if sections.is_empty() {
        return None;
    }
    let k = level_rng(w).next_int(sections.len() as i32) as usize;
    let sec = sections[k];
    let center = BlockPos::new(sec.0 * 16 + 8, sec.1 * 16 + 8, sec.2 * 16 + 8);
    // getInRange(any type, centre, 8, IS_OCCUPIED) in the POI manager's order
    let pois = poi::occupied_in_range_ordered(w, center, 8);
    if pois.is_empty() {
        return None;
    }
    let k = level_rng(w).next_int(pois.len() as i32) as usize;
    let tp = pois[k];
    land_pos_towards(w, i, 10, 7, Vec3::new(tp.x as f64 + 0.5, tp.y as f64, tp.z as f64 + 0.5))
}

// ----------------------------------------------------------------------- the flower

/// `OfferFlowerGoal`.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OfferFlower {
    pub entity: Option<u32>,
    pub tick: i32,
}

/// `canUse()`: in daylight, one tick in 8000 (the golem's own random), a villager within 6 seen.
pub fn flower_can_use(w: &mut World, i: usize, g: &mut OfferFlower) -> bool {
    if !super::zombie::is_bright_outside(w) {
        return false;
    }
    if w.entities.list[i].random.as_mut().expect("random").next_int(8000) != 0 {
        return false;
    }
    let area = w.entities.list[i].bb.inflate(6.0, 2.0, 6.0);
    let cond = target::Conditions::non_combat().range(6.0);
    let at = w.entities.list[i].pos;
    // getNearestEntity(tag CANDIDATE_FOR_IRON_GOLEM_GIFT, ...): the first strictly nearest
    let mut best = f64::MAX;
    let mut out = None;
    for t in target::mobs_in(w, area, |ty| ty == MobType::Villager) {
        if cond.test(w, i, t) {
            let d = target::pos_of(w, t).distance_to_sqr(at);
            if d < best {
                best = d;
                out = Some(target::id_of(w, t));
            }
        }
    }
    g.entity = out;
    g.entity.is_some()
}

pub fn flower_start(_w: &mut World, _i: usize, g: &mut OfferFlower) {
    g.tick = reduced_tick_delay(400);
}

pub fn flower_tick(w: &mut World, i: usize, g: &mut OfferFlower) {
    if let Some(t) = g.entity.and_then(|id| target::who(w, id)) {
        let te = target::eye_of(w, t);
        crate::entity::mob::ai::ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
    }
    g.tick -= 1;
}

// ----------------------------------------------------------------------- targets

/// `DefendVillageTargetGoal.canUse()`: a player some villager around hates (reputation -100 or
/// less); no player has a reputation here, but the sight tests run on the villagers in range.
pub fn defend_can_use(w: &mut World, i: usize) -> bool {
    let area = w.entities.list[i].bb.inflate(10.0, 8.0, 10.0);
    let cond = target::Conditions::combat().range(64.0);
    for t in target::mobs_in(w, area, |ty| ty == MobType::Villager) {
        cond.test(w, i, t);
    }
    let players: Vec<Who> = target::players(w).into_iter().filter(|p| target::box_of(w, *p).intersects(area)).collect();
    if !players.is_empty() {
        let why = "an iron golem near a player (villager reputation) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    false
}

/// `IronGolem.aiStep`'s anger (`NeutralMob.updatePersistentAnger(level, true)`): a new target
/// starts the anger timer (`PERSISTENT_ANGER_TIME`: 400..780 ticks from its own random), and so
/// does any target while it stays (`stayAngryIfTargetPresent`).
pub fn update_anger(w: &mut World, i: usize) {
    let tgt = target::get_target(w, i);
    let a = crate::entity::mob::ai::ai_mut(w, i);
    let prev_raw = a.target;
    let anger_target = a.anger_target;
    if let Some(p) = prev_raw
        && anger_target == Some(p)
    {
        let dying = target::who(w, p).is_some_and(|t| !target::is_alive(w, t));
        if dying {
            stop_being_angry(w, i);
            return;
        }
    }
    if let Some(t) = tgt {
        let new = anger_target != Some(t);
        if new {
            crate::entity::mob::ai::ai_mut(w, i).anger_target = Some(t);
        }
        // startPersistentAngerTimer: PERSISTENT_ANGER_TIME (UniformInt 400..780) from its own random
        let r = w.entities.list[i].random.as_mut().expect("random").next_int(780 - 400 + 1) + 400;
        crate::entity::mob::ai::ai_mut(w, i).anger_end = w.time + r as i64;
    }
    let a = crate::entity::mob::ai::ai(w, i);
    let angry = a.anger_end > 0 && a.anger_end - w.time > 0;
    let valid_player = tgt.is_some_and(|id| id >= crate::entity::mob::ai::PLAYER_ID_BASE && crate::entity::mob::ai::difficulty_id(w) != 0);
    if anger_target.is_some() && !angry && !valid_player {
        stop_being_angry(w, i);
    }
}

/// `NeutralMob.stopBeingAngry()`.
fn stop_being_angry(w: &mut World, i: usize) {
    let tc = w.entities.list[i].tick_count;
    let a = crate::entity::mob::ai::ai_mut(w, i);
    a.last_hurt_by_mob = None;
    a.last_hurt_by_mob_timestamp = tc;
    a.anger_target = None;
    a.target = None;
    a.anger_end = -1;
}
