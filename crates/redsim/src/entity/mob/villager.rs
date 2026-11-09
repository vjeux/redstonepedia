//! The villager (ironfarm), read in the 26.3 source: `Villager.BRAIN_PROVIDER` (its sensors and
//! activities), `Brain` (`tick` = forget outdated memories, tick sensors, start each non-running
//! behavior, tick each running one; `MemorySlot.tick`; `updateActivityFromSchedule` at most once
//! per 21 gt; the `VILLAGER_SCHEDULE` timeline), the sensors that feed the modeled behaviors
//! (`NearestLivingEntitySensor`, `VillagerHostilesSensor`, `GolemSensor`, `HurtBySensor`; start
//! delays `nextInt(scanRate)` from the villager's own random), and the behaviors that decide
//! golems and sleep: `VillagerPanicTrigger` (a `Behavior` of 60 gt), `VillagerCalmDown`,
//! `WakeUp`, `SleepInBed`, `UpdateActivityFromSchedule`; `Villager.spawnGolemIfNeeded`,
//! `wantsToSpawnGolem`, `golemSpawnConditionsMet`; `LivingEntity.startSleeping` /
//! `stopSleeping` (`AbstractBedBlock.findStandUpPosition`).
//!
//! Every other behavior (walking, looking, trading, gossip, job sites, breeding, bells, raids)
//! is not modeled: the judge refuses a design where one could change the outcome (the villagers
//! are confined, never calm outside REST in a cluster of 5, no job site or food in reach).

use super::types::MobType;
use super::{MobEvent, mob, mob_mut};
use crate::block::Block;
use crate::entity::{EntityKind, Vec3, mth};
use crate::pos::{BlockPos, Dir};
use crate::rng::LegacyRandom;
use crate::world::World;

/// The activities a villager's brain switches between (adult packages).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Activity {
    Idle,
    Work,
    Meet,
    Rest,
    Panic,
}

impl Activity {
    pub fn name(self) -> &'static str {
        match self {
            Activity::Idle => "idle",
            Activity::Work => "work",
            Activity::Meet => "meet",
            Activity::Rest => "rest",
            Activity::Panic => "panic",
        }
    }
}

/// The sensors of `Villager.BRAIN_PROVIDER`, in registration order, with their scan rates.
pub const SENSORS: [(&str, i64); 9] = [
    ("nearest_living_entities", 20),
    ("nearest_players", 20),
    ("nearest_items", 20),
    ("nearest_bed", 20),
    ("hurt_by", 20),
    ("villager_hostiles", 20),
    ("villager_babies", 20),
    ("secondary_pois", 40),
    ("golem_detected", 200),
];
const NEAREST_LIVING: usize = 0;
const HURT_BY: usize = 4;
const HOSTILES: usize = 5;
const GOLEM: usize = 8;

/// `GolemSensor.MEMORY_TIME_TO_LIVE`.
pub const GOLEM_MEMORY_TTL: i64 = 599;
/// `Villager.golemSpawnConditionsMet`: slept within this many gt.
pub const SLEPT_WITHIN: i64 = 24000;

/// `VillagerHostilesSensor.ACCEPTABLE_DISTANCE_FROM_HOSTILES` for the modeled types.
fn hostile_distance(ty: MobType) -> Option<f32> {
    match ty {
        MobType::Zombie => Some(8.0),
        _ => None,
    }
}

/// `NEAREST_VISIBLE_LIVING_ENTITIES`: the nearest-living list of the last scan with its lazy,
/// cached line-of-sight test (`NearestVisibleLivingEntities`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Visible {
    pub entities: Vec<(u32, MobType)>,
    pub los: Vec<(u32, bool)>,
}

/// A villager's brain.
#[derive(Clone, PartialEq, Debug)]
pub struct Brain {
    /// Each sensor's `timeToTick`.
    pub sensors: [i64; 9],
    /// `HOME` (the claimed bed's head).
    pub home: Option<BlockPos>,
    pub last_slept: Option<i64>,
    pub last_woken: Option<i64>,
    /// `GOLEM_DETECTED_RECENTLY`'s time to live (present when `Some`).
    pub golem_detected: Option<i64>,
    /// `NEAREST_HOSTILE`.
    pub nearest_hostile: Option<u32>,
    /// `HURT_BY` (present when `Some`: the game time of the damage).
    pub hurt_by: Option<i64>,
    /// `NEAREST_LIVING_ENTITIES` (ids and types, nearest first; empty = absent).
    pub nearest_living: Vec<(u32, MobType)>,
    pub visible: Visible,
    /// The active non-core activity (CORE is always active).
    pub activity: Activity,
    pub last_schedule_update: i64,
    /// `VillagerPanicTrigger`: running, and its `endTimestamp`.
    pub panic_running: bool,
    pub panic_end: i64,
    /// `SleepInBed`: running, and its `nextOkStartTime`.
    pub sleep_running: bool,
    pub sleep_next_ok: i64,
    /// `NEAREST_VISIBLE_LIVING_ENTITIES` has a value (an object: present even when empty).
    pub visible_present: bool,
    /// ---- the exact brain (the mob-AI lane, with the mob AI on): every behaviour ----
    pub ex: Option<Box<crate::entity::mob::ai::brain::Exact>>,
}

impl Brain {
    /// A new brain (`Brain`'s constructor: each sensor's `randomlyDelayStart(body.getRandom())` in
    /// registration order, the default activity IDLE), then `registerBrainGoals`'s schedule
    /// update at game time `t`, day time `day_time`.
    pub fn new(own: &mut LegacyRandom, t: i64, day_time: i64) -> Brain {
        let mut sensors = [0i64; 9];
        for (k, (_, rate)) in SENSORS.iter().enumerate() {
            sensors[k] = own.next_int(*rate as i32) as i64;
        }
        Brain {
            sensors,
            home: None,
            last_slept: None,
            last_woken: None,
            golem_detected: None,
            nearest_hostile: None,
            hurt_by: None,
            nearest_living: Vec::new(),
            visible: Visible::default(),
            activity: scheduled(day_time).filter(|a| *a != Activity::Work).unwrap_or(Activity::Idle),
            last_schedule_update: t,
            panic_running: false,
            panic_end: 0,
            sleep_running: false,
            sleep_next_ok: 0,
            visible_present: false,
            ex: None,
        }
    }

    /// `wantsToSpawnGolem(timestamp)` = `golemSpawnConditionsMet(gameTime) &&
    /// !GOLEM_DETECTED_RECENTLY`.
    pub fn wants_golem(&self, t: i64) -> bool {
        self.last_slept.is_some_and(|s| t - s < SLEPT_WITHIN) && self.golem_detected.is_none()
    }

    /// `VillagerPanicTrigger.isHurt || hasHostile`.
    pub fn scared(&self) -> bool {
        self.hurt_by.is_some() || self.nearest_hostile.is_some()
    }
}

/// The adult villager's activity at day time `day` (the `VILLAGER_SCHEDULE` timeline's track:
/// 10 IDLE, 2000 WORK, 9000 MEET, 11000 IDLE, 12000 REST, repeating every 24000).
pub fn scheduled(day: i64) -> Option<Activity> {
    let t = day.rem_euclid(24000);
    Some(if t < 10 {
        Activity::Rest
    } else if t < 2000 {
        Activity::Idle
    } else if t < 9000 {
        Activity::Work
    } else if t < 11000 {
        Activity::Meet
    } else if t < 12000 {
        Activity::Idle
    } else {
        Activity::Rest
    })
}

fn brain(w: &World, i: usize) -> &Brain {
    mob(w, i).brain.as_deref().expect("a villager has a brain")
}

fn brain_mut(w: &mut World, i: usize) -> &mut Brain {
    mob_mut(w, i).brain.as_deref_mut().expect("a villager has a brain")
}

/// `Brain.setActiveActivity(activity)` (when not already active).
fn set_activity(w: &mut World, i: usize, a: Activity) {
    if brain(w, i).activity != a {
        brain_mut(w, i).activity = a;
        let (t, id) = (w.time, w.entities.list[i].id);
        w.mob.events.push(MobEvent::Activity { t, id, activity: a });
    }
}

/// `Brain.setActiveActivityIfPossible`: WORK needs a `JOB_SITE` (none here), so it falls back to
/// the default activity, IDLE.
fn set_activity_if_possible(w: &mut World, i: usize, a: Activity) {
    set_activity(w, i, if a == Activity::Work { Activity::Idle } else { a });
}

/// `Brain.updateActivityFromSchedule(attributes, gameTime, pos)`.
fn update_activity_from_schedule(w: &mut World, i: usize) {
    let t = w.time;
    if t - brain(w, i).last_schedule_update > 20 {
        brain_mut(w, i).last_schedule_update = t;
        let a = scheduled(w.day_time()).unwrap_or(Activity::Idle);
        if brain(w, i).activity != a {
            set_activity_if_possible(w, i, a);
        }
    }
}

/// `Villager.customServerAiStep`: the brain's tick, then the villager's own `random.nextInt(100)`
/// (the raid check).
pub fn brain_tick(w: &mut World, i: usize) {
    // ---- the exact brain (the mob-AI lane) ----
    if brain(w, i).ex.is_some() {
        crate::entity::mob::ai::brain::tick(w, i);
        return;
    }
    forget_outdated_memories(w, i);
    tick_sensors(w, i);
    if w.entities.list[i].removed {
        return;
    }
    start_behaviors(w, i);
    tick_running_behaviors(w, i);
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_int(100);
    }
}

/// `MemorySlot.tick` of the slots with an expiry (here only `GOLEM_DETECTED_RECENTLY`): cleared
/// once its time to live is at most 0, else one less.
fn forget_outdated_memories(w: &mut World, i: usize) {
    let b = brain_mut(w, i);
    if let Some(ttl) = b.golem_detected {
        b.golem_detected = if ttl <= 0 { None } else { Some(ttl - 1) };
    }
}

/// `Brain.tickSensors`: every sensor in registration order (`Sensor.tick`: `--timeToTick <= 0`
/// runs it and waits `scanRate`).
fn tick_sensors(w: &mut World, i: usize) {
    for k in 0..SENSORS.len() {
        let fire = {
            let b = brain_mut(w, i);
            b.sensors[k] -= 1;
            if b.sensors[k] <= 0 {
                b.sensors[k] = SENSORS[k].1;
                true
            } else {
                false
            }
        };
        if fire {
            match k {
                NEAREST_LIVING => sense_nearest_living(w, i),
                HURT_BY => sense_hurt_by(w, i),
                HOSTILES => sense_hostiles(w, i),
                GOLEM => sense_golem(w, i),
                _ => {}
            }
        }
    }
}

/// `NearestLivingEntitySensor.doTick`: every alive living entity but itself whose box meets its
/// box inflated by its follow range, sorted by distance (stable, from the level's order).
fn sense_nearest_living(w: &mut World, i: usize) {
    let (me, pos, bb, range) = {
        let e = &w.entities.list[i];
        (e.id, e.pos, e.bb, mob(w, i).follow_range)
    };
    let query = bb.inflate(range, range, range);
    let mut found: Vec<(f64, u32, MobType)> = w
        .get_entities(query, |e| e.id != me && matches!(&e.kind, EntityKind::Mob(m) if !m.dead))
        .into_iter()
        .map(|j| {
            let e = &w.entities.list[j];
            let ty = mob(w, j).ty;
            (e.pos.distance_to_sqr(pos), e.id, ty)
        })
        .collect();
    found.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let list: Vec<(u32, MobType)> = found.into_iter().map(|(_, id, ty)| (id, ty)).collect();
    let b = brain_mut(w, i);
    b.visible = Visible { entities: list.clone(), los: Vec::new() };
    b.nearest_living = list;
}

/// `HurtBySensor.doTick`: `HURT_BY` while the last damage is under 41 gt old
/// (`LivingEntity.getLastDamageSource`).
fn sense_hurt_by(w: &mut World, i: usize) {
    let t = w.time;
    let last = mob(w, i).last_damage;
    brain_mut(w, i).hurt_by = last.filter(|(at, _)| t - at <= 40).map(|(at, _)| at);
}

/// The villager's eye position (`getEyeY` in its pose).
pub fn eye_pos(w: &World, i: usize) -> Vec3 {
    let e = &w.entities.list[i];
    Vec3::new(e.pos.x, e.pos.y + mob(w, i).eye() as f64, e.pos.z)
}

/// `VillagerHostilesSensor.doTick`: the first entity of the visible list (nearest first) that is a
/// hostile within its distance (feet to feet) and passes the cached line-of-sight test
/// (`Sensor.isEntityTargetable`: alive, within the follow range, `hasLineOfSight`).
fn sense_hostiles(w: &mut World, i: usize) {
    let me_pos = w.entities.list[i].pos;
    let eye = eye_pos(w, i);
    let range = mob(w, i).follow_range;
    let list = brain(w, i).visible.entities.clone();
    let mut found = None;
    for (id, ty) in list {
        let Some(limit) = hostile_distance(ty) else { continue };
        let Some(j) = crate::entity::index_of(w, id) else { continue };
        let tpos = w.entities.list[j].pos;
        if tpos.distance_to_sqr(me_pos) > (limit * limit) as f64 {
            continue;
        }
        let cached = brain(w, i).visible.los.iter().find(|(e, _)| *e == id).map(|(_, v)| *v);
        let seen = match cached {
            Some(v) => v,
            None => {
                let alive = !w.entities.list[j].removed && !mob(w, j).dead;
                let in_range = tpos.distance_to_sqr(me_pos) <= range.max(2.0) * range.max(2.0);
                let teye = Vec3::new(tpos.x, tpos.y + mob(w, j).eye() as f64, tpos.z);
                let v = alive && in_range && super::los::has_line_of_sight(w, eye, teye);
                brain_mut(w, i).visible.los.push((id, v));
                v
            }
        };
        if seen {
            found = Some(id);
            break;
        }
    }
    brain_mut(w, i).nearest_hostile = found;
}

/// The iron golems in villager `i`'s `NEAREST_LIVING_ENTITIES` as last stored (what its golem
/// sensor read): the judge's count of golems sensed by another cluster than their summoner's.
pub fn nearest_golems(w: &World, i: usize) -> Vec<u32> {
    brain(w, i).nearest_living.iter().filter(|(_, ty)| *ty == MobType::IronGolem).map(|(id, _)| *id).collect()
}

/// `GolemSensor.checkForNearbyGolem`: an iron golem in `NEAREST_LIVING_ENTITIES` (as last stored:
/// a golem that died since still counts) sets `GOLEM_DETECTED_RECENTLY` for 599 gt.
fn sense_golem(w: &mut World, i: usize) {
    if brain(w, i).nearest_living.iter().any(|(_, ty)| *ty == MobType::IronGolem) {
        golem_detected(w, i);
        let (t, id) = (w.time, w.entities.list[i].id);
        w.mob.events.push(MobEvent::GolemSeen { t, villager: id });
    }
}

/// `GolemSensor.golemDetected`.
pub fn golem_detected(w: &mut World, i: usize) {
    brain_mut(w, i).golem_detected = Some(GOLEM_MEMORY_TTL);
}

/// `Brain.startEachNonRunningBehavior` over the modeled behaviors, in the brain's order: priority
/// 0 (CORE: `VillagerPanicTrigger`, `WakeUp`; PANIC: `VillagerCalmDown`, in the activity map's
/// order), priority 3 (REST: `SleepInBed`), priority 99 (`UpdateActivityFromSchedule` of WORK,
/// MEET, REST and IDLE).
fn start_behaviors(w: &mut World, i: usize) {
    let t = w.time;
    for act in priority0_order() {
        match act {
            None => {
                // CORE: VillagerPanicTrigger (index 3), then WakeUp (index 4)
                if !brain(w, i).panic_running && brain(w, i).scared() {
                    // Behavior.tryStart: no required memories; start (PANIC); end = t + 60
                    let b = brain_mut(w, i);
                    b.panic_running = true;
                    b.panic_end = t + 60;
                    set_activity_if_possible(w, i, Activity::Panic);
                }
                if brain(w, i).activity != Activity::Rest && mob(w, i).sleeping_pos.is_some() {
                    stop_sleeping(w, i);
                }
            }
            Some(Activity::Panic) if brain(w, i).activity == Activity::Panic => {
                // VillagerCalmDown (HURT_BY_ENTITY is never set: the modeled damage has no attacker)
                if !brain(w, i).scared() {
                    brain_mut(w, i).hurt_by = None;
                    update_activity_from_schedule(w, i);
                }
            }
            _ => {}
        }
    }
    if brain(w, i).activity == Activity::Rest && !brain(w, i).sleep_running && sleep_can_start(w, i) {
        brain_mut(w, i).sleep_running = true;
        sleep_start(w, i);
    }
    if brain(w, i).activity != Activity::Panic {
        update_activity_from_schedule(w, i);
    }
}

/// The order `Brain`'s `HashMap<Activity, ..>` gives the priority-0 activities (CORE, PANIC,
/// PRE_RAID, RAID, HIDE: buckets of `name.hashCode()` spread, 16 buckets); `None` = CORE. Only
/// CORE and PANIC carry modeled behaviors.
pub fn priority0_order() -> Vec<Option<Activity>> {
    let mut keys: Vec<(u32, usize, Option<Activity>)> = [("core", None), ("panic", Some(Activity::Panic)), ("pre_raid", Some(Activity::Idle)), ("raid", Some(Activity::Idle)), ("hide", Some(Activity::Idle))]
        .iter()
        .enumerate()
        .map(|(k, (name, a))| {
            let h = java_string_hash(name);
            ((h ^ (h >> 16)) & 15, k, *a)
        })
        .collect();
    keys.sort_by_key(|(bucket, k, _)| (*bucket, *k));
    keys.into_iter().filter(|(_, k, _)| *k < 2).map(|(_, _, a)| a).collect()
}

/// `String.hashCode()`.
pub fn java_string_hash(s: &str) -> u32 {
    s.chars().fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32))
}

/// `SleepInBed.checkExtraStartConditions` (and its memory requirements): not riding, a HOME,
/// not woken 1..99 gt ago, the bed within 2 of its center, a bed there, not occupied.
fn sleep_can_start(w: &World, i: usize) -> bool {
    let b = brain(w, i);
    let Some(home) = b.home else { return false };
    if w.entities.list[i].vehicle.is_some() {
        return false;
    }
    if let Some(woken) = b.last_woken {
        let since = w.time - woken;
        if since > 0 && since < 100 {
            return false;
        }
    }
    let pos = w.entities.list[i].pos;
    closer_to_center(home, pos, 2.0) && matches!(w.get(home), Block::Bed { occupied: false, .. })
}

/// `Vec3i.closerToCenterThan(pos, d)`: the cell's center within `d` (strictly).
pub fn closer_to_center(cell: BlockPos, pos: Vec3, d: f64) -> bool {
    let c = Vec3::new(cell.x as f64 + 0.5, cell.y as f64 + 0.5, cell.z as f64 + 0.5);
    c.distance_to_sqr(pos) < d * d
}

/// `SleepInBed.start`: past `nextOkStartTime`, `startSleeping(home)` and `LAST_SLEPT`.
fn sleep_start(w: &mut World, i: usize) {
    let t = w.time;
    if t > brain(w, i).sleep_next_ok
        && let Some(home) = brain(w, i).home
        && start_sleeping(w, i, home)
    {
        brain_mut(w, i).last_slept = Some(t);
        let id = w.entities.list[i].id;
        w.mob.events.push(MobEvent::Slept { t, id });
    }
}

/// `Brain.tickEachRunningBehavior` over the modeled behaviors (`getRunningBehaviors` order:
/// priority 0 before 3): `VillagerPanicTrigger` (`tickOrStop`: stopped past its end or when not
/// scared, else `tick`: `spawnGolemIfNeeded(3)` at multiples of 100), then `SleepInBed` (never
/// times out; stopped when REST is over, the villager is below the bed + 0.4 or 1.14 from it).
fn tick_running_behaviors(w: &mut World, i: usize) {
    let t = w.time;
    if brain(w, i).panic_running {
        if t > brain(w, i).panic_end || !brain(w, i).scared() {
            brain_mut(w, i).panic_running = false;
        } else if t % 100 == 0 {
            super::spawn::spawn_golem_if_needed(w, i, 3);
        }
    }
    if brain(w, i).sleep_running {
        let keep = match brain(w, i).home {
            None => false,
            Some(bed) => {
                let pos = w.entities.list[i].pos;
                brain(w, i).activity == Activity::Rest && pos.y > bed.y as f64 + 0.4 && closer_to_center(bed, pos, 1.14)
            }
        };
        if !keep {
            brain_mut(w, i).sleep_running = false;
            if mob(w, i).sleeping_pos.is_some() {
                stop_sleeping(w, i);
                brain_mut(w, i).sleep_next_ok = t + 40;
            }
        }
    }
}

/// `AbstractBedBlock.getSleepHeight`: the bed shape's top, 9/16.
const SLEEP_HEIGHT: f64 = 0.5625;

/// `LivingEntity.setPosToBed`: on the bed, 0.125 above its top.
pub fn set_pos_to_bed(w: &mut World, i: usize, bed: BlockPos) {
    w.entities.set_pos(i, Vec3::new(bed.x as f64 + 0.5, bed.y as f64 + SLEEP_HEIGHT + 0.125, bed.z as f64 + 0.5));
}

/// `LivingEntity.startSleeping(bed)`: the bed occupied, the mob on it, pose SLEEPING (0.2 x 0.2),
/// motion zero. False when there is no bed.
pub fn start_sleeping(w: &mut World, i: usize, bed: BlockPos) -> bool {
    let Block::Bed { colour, facing, head, .. } = w.get(bed) else { return false };
    if w.entities.list[i].vehicle.is_some() {
        crate::entity::stop_riding(w, i);
    }
    set_pos_to_bed(w, i, bed);
    w.set_block(bed, Block::Bed { colour, facing, head, occupied: true }, crate::world::flags::ALL);
    mob_mut(w, i).sleeping_pos = Some(bed);
    let p = w.entities.list[i].pos;
    w.entities.set_pos(i, p);
    w.entities.list[i].delta_movement = Vec3::ZERO;
    true
}

/// `Villager.stopSleeping` = `LivingEntity.stopSleeping` (the bed freed; the position from
/// `findStandUpPosition`, else above the bed + 0.1; facing the bed; pose STANDING) then
/// `LAST_WOKEN`.
pub fn stop_sleeping(w: &mut World, i: usize) {
    let Some(bed) = mob(w, i).sleeping_pos else { return };
    if let Block::Bed { colour, facing, head, .. } = w.get(bed) {
        w.set_block(bed, Block::Bed { colour, facing, head, occupied: false }, crate::world::flags::ALL);
        let ty = mob(w, i).ty;
        let yaw = w.entities.list[i].y_rot;
        let stand = find_stand_up_position(w, ty, bed, facing, yaw).unwrap_or(Vec3::new(bed.x as f64 + 0.5, bed.y as f64 + 1.0 + 0.1, bed.z as f64 + 0.5));
        let look = Vec3::new(bed.x as f64 + 0.5, bed.y as f64, bed.z as f64 + 0.5).subtract_v(stand).normalize();
        // the bytecode: `Mth.wrapDegrees(Mth.atan2(z, x) * (double)Mth.RAD_TO_DEG - 90.0)`
        let yaw = mth::wrap_degrees_d(mth::atan2(look.z, look.x) * 57.2957763671875 - 90.0) as f32;
        mob_mut(w, i).sleeping_pos = None;
        w.entities.set_pos(i, stand);
        w.entities.list[i].y_rot = yaw;
        w.entities.list[i].x_rot = 0.0;
    } else {
        mob_mut(w, i).sleeping_pos = None;
        let p = w.entities.list[i].pos;
        w.entities.set_pos(i, p);
    }
    let t = w.time;
    let id = w.entities.list[i].id;
    if let Some(b) = mob_mut(w, i).brain.as_deref_mut() {
        b.last_woken = Some(t);
    }
    w.mob.events.push(MobEvent::Woke { t, id });
}

/// `Direction.isFacingAngle(yaw)`.
fn is_facing_angle(d: Dir, yaw: f32) -> bool {
    let r = yaw * (std::f32::consts::PI / 180.0f32);
    let dx = -mth::sin(r as f64);
    let dz = mth::cos(r as f64);
    let s = d.step();
    s[0] as f32 * dx + s[2] as f32 * dz > 0.0f32
}

/// `AbstractBedBlock.findStandUpPosition(type, level, bed, forward, yaw)` (not a bunk bed: one
/// with a bed under it is refused by the judge): the cells around the bed, then on it, safe ones
/// first (`DismountHelper.findSafeDismountLocation`).
pub fn find_stand_up_position(w: &World, ty: MobType, pos: BlockPos, forward: Dir, yaw: f32) -> Option<Vec3> {
    let right = forward.clockwise();
    let side = if is_facing_angle(right, yaw) { right.opposite() } else { right };
    stand_up_with_side(w, ty, pos, forward, side)
}

/// [`find_stand_up_position`] with the side chosen (the judge checks both: the side follows the
/// villager's yaw, which unmodeled look behaviors turn).
pub fn stand_up_with_side(w: &World, ty: MobType, pos: BlockPos, forward: Dir, side: Dir) -> Option<Vec3> {
    let (f, s) = (forward.step(), side.step());
    let (fx, fz, sx, sz) = (f[0], f[2], s[0], s[2]);
    let offsets: [(i32, i32); 12] = [
        (sx, sz),
        (sx - fx, sz - fz),
        (sx - fx * 2, sz - fz * 2),
        (-fx * 2, -fz * 2),
        (-sx - fx * 2, -sz - fz * 2),
        (-sx - fx, -sz - fz),
        (-sx, -sz),
        (-sx + fx, -sz + fz),
        (fx, fz),
        (sx + fx, sz + fz),
        (0, 0),
        (-fx, -fz),
    ];
    for dangerous in [true, false] {
        for (dx, dz) in offsets {
            if let Some(v) = safe_dismount_location(w, ty, BlockPos::new(pos.x + dx, pos.y, pos.z + dz), dangerous) {
                return Some(v);
            }
        }
    }
    None
}

/// A block's collision shape top (`VoxelShape.max(Y)`) in the cell, `None` when empty;
/// `DismountHelper.nonClimbableShape` (open trapdoors and climbables count as empty).
fn non_climbable_top(w: &World, p: BlockPos) -> Option<f64> {
    let s = w.get(p);
    if matches!(s, Block::Trapdoor { open: true, .. } | Block::Scaffolding { .. }) {
        return None;
    }
    let boxes = crate::entity::collide::boxes_at(w, &s, p);
    boxes.iter().map(|b| b.max_y - p.y as f64).fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
}

/// `DismountHelper.findSafeDismountLocation(type, level, blockPos, checkDangerous)`.
fn safe_dismount_location(w: &World, ty: MobType, p: BlockPos, check_dangerous: bool) -> Option<Vec3> {
    let fire_immune = ty.def().fire_immune;
    if check_dangerous && super::blocks::is_dangerous(&w.get(p), fire_immune) {
        return None;
    }
    // BlockGetter.getBlockFloorHeight
    let floor = match non_climbable_top(w, p) {
        Some(top) => top,
        None => match non_climbable_top(w, p.below()) {
            Some(b) if b >= 1.0 => b - 1.0,
            _ => f64::NEG_INFINITY,
        },
    };
    if !(floor.is_finite() && floor < 1.0) {
        return None;
    }
    if check_dangerous && floor <= 0.0 && super::blocks::is_dangerous(&w.get(p.below()), fire_immune) {
        return None;
    }
    let pos = Vec3::new(p.x as f64 + 0.5, p.y as f64 + floor, p.z as f64 + 0.5);
    let d = ty.def();
    let bb = crate::entity::make_bounding_box(pos, d.width, d.height);
    if crate::entity::collide::block_collisions(w, bb).iter().any(|(_, boxes)| boxes.iter().any(|b| b.intersects(bb))) {
        return None;
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_keyframes() {
        assert_eq!(scheduled(0), Some(Activity::Rest));
        assert_eq!(scheduled(10), Some(Activity::Idle));
        assert_eq!(scheduled(1999), Some(Activity::Idle));
        assert_eq!(scheduled(2000), Some(Activity::Work));
        assert_eq!(scheduled(9000), Some(Activity::Meet));
        assert_eq!(scheduled(11000), Some(Activity::Idle));
        assert_eq!(scheduled(12000), Some(Activity::Rest));
        assert_eq!(scheduled(24009), Some(Activity::Rest));
        assert_eq!(scheduled(24010), Some(Activity::Idle));
    }

    #[test]
    fn string_hash_and_priority0_order() {
        assert_eq!(java_string_hash("core"), 3059615);
        // both CORE and PANIC are present in the order, each once
        let o = priority0_order();
        assert_eq!(o.len(), 2);
        assert!(o.contains(&None) && o.contains(&Some(Activity::Panic)));
    }

    #[test]
    fn golem_memory_expires_after_600_gt() {
        // set at T after the villager's own brain tick: gone at the start of T+600's tick
        let mut b = Brain::new(&mut LegacyRandom::new(1), 0, 6000);
        b.last_slept = Some(0);
        b.golem_detected = Some(GOLEM_MEMORY_TTL);
        let mut gone_at = None;
        for dt in 1..=700 {
            let ttl = b.golem_detected.unwrap();
            b.golem_detected = if ttl <= 0 { None } else { Some(ttl - 1) };
            if b.golem_detected.is_none() {
                gone_at = Some(dt);
                break;
            }
        }
        assert_eq!(gone_at, Some(600));
        assert!(b.wants_golem(600));
        assert!(!b.wants_golem(24000));
    }

    /// The review's follow box: a golem (1.4 wide) is listed when its box meets the villager's
    /// (0.6 wide) inflated by its follow range R: |dx| < R + 1.0 (0.3 + 0.7).
    #[test]
    fn follow_box_is_range_plus_one() {
        for (dx, listed) in [(16.99, true), (17.01, false)] {
            let mut w = World::new(BlockPos::new(-24, -8, -24), [48, 16, 48]);
            w.mob.seed(1, 6000);
            let v = super::super::add(&mut w, MobType::Villager, Vec3::new(0.5, 0.0, 0.5), true);
            let g = super::super::add(&mut w, MobType::IronGolem, Vec3::new(0.5 + dx, 0.0, 0.5), true);
            let i = crate::entity::index_of(&w, v).unwrap();
            mob_mut(&mut w, i).follow_range = 16.0;
            sense_nearest_living(&mut w, i);
            assert_eq!(brain(&w, i).nearest_living.iter().any(|(id, _)| *id == g), listed, "dx {dx}");
        }
    }
}
