//! The fish (`AbstractFish`, `AbstractSchoolingFish`: the salmon and the cod), 26.3 read line by
//! line:
//!
//! * the goals: `PanicGoal(1.25)` (the animals' one), `AvoidEntityGoal<Player>` (the AFK player is
//!   in creative: never avoided), `FishSwimGoal` (`RandomSwimmingGoal(1.0, 40)`: a follower does
//!   not swim at random; `BehaviorUtils.getRandomSwimmablePos(10, 7)`) and
//!   `FollowFlockLeaderGoal` (no flags; its countdown drawn by its constructor, the bridge's
//!   `replayConstructor` redoes it);
//! * `WaterBoundPathNavigation` with `SwimNodeEvaluator` (`path::navigation`, `path::walk`);
//! * `FishMoveControl` (0.005 up while the eyes are in water; the speed eased by 0.125 toward the
//!   wanted one, a vertical push, the yaw turned at most 90);
//! * `travelInWater` (0.01 input, the move, x 0.9, 0.005 down without a target), the flop out of
//!   the water (`aiStep`), `WaterAnimal`'s air and its push by fluids (none);
//! * the school (`AbstractSchoolingFish`): a pack's first fish leads the rest (`finalizeSpawn`'s
//!   group data), the 1-in-200 level-random check that a leader alone is alone, and following.

use super::control::{self, Operation};
use super::goal::stroll::{self, RandomStroll};
use super::{ai, ai_mut};
use crate::entity::mob::{mob, mob_mut};
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

/// A fish's own state (`AbstractSchoolingFish`).
#[derive(Clone, PartialEq, Debug)]
pub struct FishState {
    /// `getMaxSchoolSize()`: the salmon's 5, the cod's 8 (`AbstractFish.getMaxSpawnClusterSize`).
    pub max_school: i32,
    /// `leader`: the fish it follows (an entity id).
    pub leader: Option<u32>,
    /// `schoolSize`: 1 alone, one more per follower.
    pub school_size: i32,
}

/// The fish's own goals.
#[derive(Clone, PartialEq, Debug)]
pub enum FishGoal {
    /// `FollowFlockLeaderGoal`: `nextStartTick` (its constructor's draw, then the search's) and
    /// `timeToRecalcPath`.
    FollowLeader { next_start_tick: i32, time_to_recalc_path: i32 },
}

impl FishGoal {
    pub fn name(&self) -> &'static str {
        "FollowFlockLeaderGoal"
    }
}

/// A fish row with its AI modeled (`Salmon`, `Cod`).
pub fn is_fish_id(id: &str) -> bool {
    matches!(id, "salmon" | "cod")
}

/// Is entity `i` a fish with its AI?
pub fn is_fish(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ai.as_deref().is_some_and(|a| a.fish.is_some()))
}

/// `AbstractFish.registerGoals` then `AbstractSchoolingFish`'s; `WaterAnimal`'s WATER malus 0;
/// `createNavigation`: a `WaterBoundPathNavigation`.
pub fn register(a: &mut super::MobAi, id: &str) {
    use super::goal::Goal;
    use super::goal::animal::AnimalGoal;
    a.goals.add(0, Goal::Animal(AnimalGoal::Panic { speed: 1.25, pos: Vec3::ZERO, mount: false }));
    a.goals.add(2, Goal::AvoidEntity);
    a.goals.add(4, Goal::RandomStroll(RandomStroll::fish_swim()));
    a.goals.add(5, Goal::Fish(FishGoal::FollowLeader { next_start_tick: 0, time_to_recalc_path: 0 }));
    a.fish = Some(FishState { max_school: if id == "salmon" { 5 } else { 8 }, leader: None, school_size: 1 });
    a.nav.kind = super::path::navigation::NavKind::WaterBound;
    a.malus.set(super::path::path_type::PathType::Water, 0.0);
}

/// `FollowFlockLeaderGoal.nextStartTick(mob)`: `reducedTickDelay(200 + nextInt(200) % 20)` (its
/// constructor's, after the entity's own constructor draws).
pub fn next_start_tick(r: &mut crate::rng::LegacyRandom) -> i32 {
    super::goal::reduced_tick_delay(200 + r.next_int(200) % 20)
}

/// Set the `FollowFlockLeaderGoal`'s countdown (the constructor's draw, done again).
pub fn set_next_start_tick(a: &mut super::MobAi, v: i32) {
    for g in a.goals.goals.iter_mut() {
        if let super::goal::Goal::Fish(FishGoal::FollowLeader { next_start_tick, .. }) = &mut g.goal {
            *next_start_tick = v;
        }
    }
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("a mob's own random")
}

fn fish(w: &World, j: usize) -> Option<&FishState> {
    match &w.entities.list[j].kind {
        EntityKind::Mob(m) => m.ai.as_deref().and_then(|a| a.fish.as_ref()),
        _ => None,
    }
}

fn fish_mut(w: &mut World, j: usize) -> Option<&mut FishState> {
    match &mut w.entities.list[j].kind {
        EntityKind::Mob(m) => m.ai.as_deref_mut().and_then(|a| a.fish.as_mut()),
        _ => None,
    }
}

/// `Entity.isAlive()` of entity `j`: not removed, a living one above 0 health.
fn alive(w: &World, j: usize) -> bool {
    let e = &w.entities.list[j];
    !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.health > 0.0)
}

/// `isFollower()`: a leader that is alive.
pub fn is_follower(w: &World, j: usize) -> bool {
    fish(w, j).and_then(|f| f.leader).and_then(|l| crate::entity::index_of(w, l)).is_some_and(|k| alive(w, k))
}

/// `hasFollowers()`: a school of more than itself.
fn has_followers(w: &World, j: usize) -> bool {
    fish(w, j).is_some_and(|f| f.school_size > 1)
}

/// `canBeFollowed()`: followers, and room for one more.
fn can_be_followed(w: &World, j: usize) -> bool {
    fish(w, j).is_some_and(|f| f.school_size > 1 && f.school_size < f.max_school)
}

/// A probe: `MCTAS_TRACE_SCHOOL=1` prints each school change (who follows whom, when).
fn trace_school(w: &World, what: &str, j: usize, k: Option<usize>) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("MCTAS_TRACE_SCHOOL").is_ok()) {
        return;
    }
    let size = |x: usize| fish(w, x).map_or(0, |f| f.school_size);
    match k {
        Some(k) => eprintln!("school t{}: #{} {what} #{} (its school {})", w.time, w.entities.list[j].id, w.entities.list[k].id, size(k)),
        None => eprintln!("school t{}: #{} {what}", w.time, w.entities.list[j].id),
    }
}

/// `startFollowing(leader)`: the leader's school grows by one.
fn start_following(w: &mut World, j: usize, leader: usize) {
    let lid = w.entities.list[leader].id;
    if let Some(f) = fish_mut(w, j) {
        f.leader = Some(lid);
    }
    if let Some(l) = fish_mut(w, leader) {
        l.school_size += 1;
    }
    trace_school(w, "follows", j, Some(leader));
}

/// `stopFollowing()`: the leader's school shrinks by one; no leader.
fn stop_following(w: &mut World, j: usize) {
    let leader = fish(w, j).and_then(|f| f.leader);
    let k = leader.and_then(|l| crate::entity::index_of(w, l));
    if let Some(k) = k
        && let Some(l) = fish_mut(w, k)
    {
        l.school_size -= 1;
    }
    if let Some(f) = fish_mut(w, j) {
        f.leader = None;
    }
    trace_school(w, "stops following", j, k);
}

/// `getEntitiesOfClass(this.getClass(), box)`: the fish of entity `i`'s type whose box meets
/// `bb`, in the level's order.
fn same_class_in(w: &World, i: usize, bb: crate::entity::Aabb) -> Vec<usize> {
    let ty = mob(w, i).ty;
    w.get_entities(bb, |e| matches!(&e.kind, EntityKind::Mob(m) if m.ty == ty))
}

/// `AbstractSchoolingFish.canRandomSwim()`: not following a live leader (`FishSwimGoal`'s gate).
pub fn can_random_swim(w: &World, i: usize) -> bool {
    !is_follower(w, i)
}

/// A natural spawn's school (`AbstractSchoolingFish.finalizeSpawn`): the pack's first fish is the
/// group data's leader, each later one starts following it.
pub fn join_pack(w: &mut World, id: u32, leader: &mut Option<u32>) {
    let Some(j) = crate::entity::index_of(w, id) else { return };
    if fish(w, j).is_none() {
        return;
    }
    match leader.and_then(|l| crate::entity::index_of(w, l)) {
        Some(k) => start_following(w, j, k),
        None => *leader = Some(id),
    }
}

// ----------------------------------------------------------------------------- the goal

pub fn can_use(w: &mut World, i: usize, g: &mut FishGoal) -> bool {
    let FishGoal::FollowLeader { next_start_tick, .. } = g;
    if has_followers(w, i) {
        return false;
    }
    if is_follower(w, i) {
        return true;
    }
    if *next_start_tick > 0 {
        *next_start_tick -= 1;
        return false;
    }
    *next_start_tick = next_start_tick_of(w, i);
    // the candidates: a leader with room or a fish that follows no one (as the list is made)
    let bb = w.entities.list[i].bb.inflate(8.0, 8.0, 8.0);
    let list: Vec<usize> = same_class_in(w, i, bb).into_iter().filter(|&j| can_be_followed(w, j) || !is_follower(w, j)).collect();
    let leader = list.iter().copied().find(|&j| can_be_followed(w, j)).unwrap_or(i);
    // addFollowers(list.stream().filter(!isFollower)): limit(max - schoolSize) counted before
    // the leader itself is skipped; each element tested as the stream reaches it
    let room = fish(w, leader).map_or(0, |f| f.max_school - f.school_size);
    let mut taken = 0;
    for &j in &list {
        if taken >= room {
            break;
        }
        if is_follower(w, j) {
            continue;
        }
        taken += 1;
        if j != leader {
            start_following(w, j, leader);
        }
    }
    is_follower(w, i)
}

fn next_start_tick_of(w: &mut World, i: usize) -> i32 {
    next_start_tick(own(w, i))
}

/// `canContinueToUse()`: still following, within 11 of the leader.
pub fn can_continue(w: &mut World, i: usize, _g: &mut FishGoal) -> bool {
    if !is_follower(w, i) {
        return false;
    }
    let l = fish(w, i).and_then(|f| f.leader).and_then(|l| crate::entity::index_of(w, l)).expect("a follower's leader");
    w.entities.list[i].pos.distance_to_sqr(w.entities.list[l].pos) <= 121.0
}

pub fn start(_w: &mut World, _i: usize, g: &mut FishGoal) {
    let FishGoal::FollowLeader { time_to_recalc_path, .. } = g;
    *time_to_recalc_path = 0;
}

pub fn stop(w: &mut World, i: usize, _g: &mut FishGoal) {
    stop_following(w, i);
}

/// `tick()`: every `adjustedTickDelay(10)` (5) ticks, `pathToLeader()`: `moveTo(leader, 1.0)`.
pub fn tick(w: &mut World, i: usize, g: &mut FishGoal) {
    let FishGoal::FollowLeader { time_to_recalc_path, .. } = g;
    *time_to_recalc_path -= 1;
    if *time_to_recalc_path > 0 {
        return;
    }
    *time_to_recalc_path = super::goal::reduced_tick_delay(10);
    if !is_follower(w, i) {
        return;
    }
    let l = fish(w, i).and_then(|f| f.leader).and_then(|l| crate::entity::index_of(w, l)).expect("a follower's leader");
    let target = w.entities.list[l].block_position();
    super::with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity(w, nm, target, 1, 1.0, arena));
}

// ----------------------------------------------------------------------------- the body

/// `BehaviorUtils.getRandomSwimmablePos(mob, h, v)`: `DefaultRandomPos.getPos`, again (10 more
/// times at most) while its cell cannot be swum through (`isPathfindable(WATER)`).
pub fn swimmable_pos(w: &mut World, i: usize, h: i32, v: i32) -> Option<Vec3> {
    let mut pos = stroll::default_random_pos(w, i, h, v);
    let mut attempts = 0;
    while let Some(p) = pos {
        let cell = BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let swimmable = super::path::walk::Level { w }.pathfindable_water(cell);
        if swimmable || attempts >= 10 {
            break;
        }
        attempts += 1;
        pos = stroll::default_random_pos(w, i, h, v);
    }
    pos
}

/// `FishMoveControl.tick()` (the operation is never set back to WAIT).
pub fn move_control_tick(w: &mut World, i: usize) {
    if w.entities.list[i].fluid.eye_in_water() {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, 0.005, 0.0);
    }
    let mc = ai(w, i).move_ctl;
    if mc.operation == Operation::MoveTo && !super::nav_of(w, i).is_done() {
        let target = (mc.speed_modifier * super::movement_speed(w, i)) as f32;
        let cur = mob(w, i).speed;
        // Mob.setSpeed(Mth.lerp(0.125F, speed, target)): the speed and the forward input
        let speed = cur + 0.125f32 * (target - cur);
        {
            let m = mob_mut(w, i);
            m.speed = speed;
            m.zza = speed;
        }
        let p = w.entities.list[i].pos;
        let (xd, yd, zd) = (mc.wanted_x - p.x, mc.wanted_y - p.y, mc.wanted_z - p.z);
        if yd != 0.0 {
            let dd = (xd * xd + yd * yd + zd * zd).sqrt();
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.add(0.0, speed as f64 * (yd / dd) * 0.1, 0.0);
        }
        if xd != 0.0 || zd != 0.0 {
            let y_rot_d = control::atan2_degrees(zd, xd) - 90.0f32;
            let y_rot = control::rotlerp(w.entities.list[i].y_rot, y_rot_d, 90.0);
            w.entities.list[i].y_rot = y_rot;
            ai_mut(w, i).y_body_rot = y_rot;
        }
    } else {
        let m = mob_mut(w, i);
        m.speed = 0.0;
        m.zza = 0.0;
    }
}

/// `AbstractFish.aiStep`'s head: out of the water on the ground after a vertical collision, a
/// flop (two floats of its own random for the push, two for the flop sound's pitch).
pub fn flop(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    if e.was_touching_water || !e.on_ground || !e.vertical_collision {
        return;
    }
    let r = own(w, i);
    let x = (r.next_float() * 2.0f32 - 1.0f32) * 0.05f32;
    let z = (r.next_float() * 2.0f32 - 1.0f32) * 0.05f32;
    r.next_float();
    r.next_float();
    let e = &mut w.entities.list[i];
    e.delta_movement = e.delta_movement.add(x as f64, 0.4f32 as f64, z as f64);
    e.on_ground = false;
}

/// `AbstractSchoolingFish.tick()` after `super.tick()`: a fish with followers, 1 time in 200 (the
/// level random), finds itself alone among its kind within 8 and forgets its school.
pub fn school_tick(w: &mut World, i: usize) {
    // (a fish checkDespawn just discarded still runs this: `tickNonPassenger` has no removal test)
    if !has_followers(w, i) {
        return;
    }
    let roll = match w.level_random.as_mut() {
        Some(r) => r.next_int(200),
        None => w.mob.rng_mut().next_int(200),
    };
    trace_school(w, "draws its school check", i, None);
    if roll != 1 {
        return;
    }
    let bb = w.entities.list[i].bb.inflate(8.0, 8.0, 8.0);
    if same_class_in(w, i, bb).len() <= 1
        && let Some(f) = fish_mut(w, i)
    {
        f.school_size = 1;
        trace_school(w, "is alone: its school forgotten", i, None);
    }
}
