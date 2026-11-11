//! Targeting (26.3 `TargetingConditions`, `Sensing`, `Mob.getTarget` / `setTarget` /
//! `asValidTarget`, `LivingEntity.canAttack`, `hasLineOfSight`, `getVisibilityPercent`, the level's
//! `getNearestEntity` / `getNearestPlayer`). A target is an entity id, or a player's id
//! ([`super::PLAYER_ID_BASE`] + index).

use super::{GameMode, PLAYER_ID_BASE, ai, ai_mut};
use crate::entity::mob::types::MobType;
use crate::entity::mob::{los, mob};
use crate::entity::{Aabb, EntityKind, Vec3};
use crate::world::World;

/// Who a target id names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    /// An entity, by index in the list.
    Entity(usize),
    /// A player, by index in `MobWorld.ai.players`.
    Player(usize),
    /// (tickexact 2026-11-10) An entity of the other dimension, by index in [`World::foreign`]: a
    /// Nether mob's target in the overworld (a copy's saved NeutralMob anger target, which
    /// `readPersistentAngerSaveData` resolves in any dimension). Never in line of sight.
    Foreign(usize),
}

/// (tickexact 2026-11-10) An entity of the other dimension as a target sees it this tick (its
/// own world's state after its own tick: the Nether ticks after the overworld).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Foreign {
    pub id: u32,
    pub pos: Vec3,
    pub eye: Vec3,
    pub bb: Aabb,
    pub alive: bool,
    pub ty: Option<MobType>,
    pub in_water: bool,
}

/// (tickexact 2026-11-10) The snapshot of entity `id` of world `w` (for the other dimension's
/// [`World::foreign`]); `None` if `w` has no such entity.
pub fn foreign_of(w: &World, id: u32) -> Option<Foreign> {
    let j = crate::entity::index_of(w, id)?;
    let e = &w.entities.list[j];
    if e.removed {
        return None;
    }
    let (alive, ty) = match &e.kind {
        EntityKind::Mob(m) => (m.health > 0.0, Some(m.ty)),
        _ => (false, None),
    };
    Some(Foreign { id, pos: e.pos, eye: super::eye_pos(w, j), bb: e.bb, alive, ty, in_water: e.was_touching_water })
}

/// `EntityTypes.PLAYER`'s box and eye.
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_EYE: f32 = 1.62;

pub fn who(w: &World, id: u32) -> Option<Who> {
    if id >= PLAYER_ID_BASE {
        let k = (id - PLAYER_ID_BASE) as usize;
        return (k < w.mob.ai.players.len()).then_some(Who::Player(k));
    }
    crate::entity::index_of(w, id).filter(|&j| !w.entities.list[j].removed).map(Who::Entity).or_else(|| w.foreign.iter().position(|f| f.id == id).map(Who::Foreign))
}

/// A target's feet.
pub fn pos_of(w: &World, t: Who) -> Vec3 {
    match t {
        Who::Entity(j) => w.entities.list[j].pos,
        Who::Player(k) => w.mob.ai.players[k].pos,
        Who::Foreign(f) => w.foreign[f].pos,
    }
}

/// A target's eye (`getEyeY` with its x and z).
pub fn eye_of(w: &World, t: Who) -> Vec3 {
    match t {
        Who::Entity(j) => super::eye_pos(w, j),
        Who::Player(k) => {
            let p = w.mob.ai.players[k].pos;
            Vec3::new(p.x, p.y + PLAYER_EYE as f64, p.z)
        }
        Who::Foreign(f) => w.foreign[f].eye,
    }
}

/// A target's box (`getBoundingBox`, = `getHitbox`).
pub fn box_of(w: &World, t: Who) -> Aabb {
    match t {
        Who::Entity(j) => w.entities.list[j].bb,
        Who::Player(k) => crate::entity::make_bounding_box(w.mob.ai.players[k].pos, PLAYER_WIDTH, PLAYER_HEIGHT),
        Who::Foreign(f) => w.foreign[f].bb,
    }
}

/// (mob-AI 2026-11-10) `LivingEntity.getHitbox()`: a rider's box starts no lower than its seat
/// (`vehicle.getPassengerRidingPosition(this).y`: the vehicle's y plus its passenger attachment;
/// a spider jockey's skeleton is hit only above the spider's 0.765, forest build 3 t20115).
pub fn hitbox_of(w: &World, t: Who) -> Aabb {
    let bb = box_of(w, t);
    let Who::Entity(j) = t else { return bb };
    let Some(vi) = w.entities.list[j].vehicle.and_then(|v| crate::entity::index_of(w, v)) else { return bb };
    let crate::entity::EntityKind::Mob(m) = &w.entities.list[j].kind else { return bb };
    let Some((seat, _)) = crate::entity::mob::rider_seat(&w.entities.list[vi].kind, m) else { return bb };
    let y = w.entities.list[vi].pos.y + seat;
    Aabb::new(bb.min_x, y.max(bb.min_y), bb.min_z, bb.max_x, bb.max_y, bb.max_z)
}

pub fn id_of(w: &World, t: Who) -> u32 {
    match t {
        Who::Entity(j) => w.entities.list[j].id,
        Who::Player(k) => PLAYER_ID_BASE + k as u32,
        Who::Foreign(f) => w.foreign[f].id,
    }
}

/// `isAlive()`: not removed, health above 0 (a player is always alive here).
pub fn is_alive(w: &World, t: Who) -> bool {
    match t {
        Who::Entity(j) => {
            let e = &w.entities.list[j];
            !e.removed && matches!(&e.kind, EntityKind::Mob(m) if m.health > 0.0)
        }
        Who::Player(_) => true,
        Who::Foreign(f) => w.foreign[f].alive,
    }
}

/// `canBeSeenByAnyone()`: not a spectator, alive.
fn can_be_seen_by_anyone(w: &World, t: Who) -> bool {
    match t {
        Who::Player(k) => w.mob.ai.players[k].mode != GameMode::Spectator,
        e => is_alive(w, e),
    }
}

/// `canBeSeenAsEnemy()`: not invulnerable (a creative player's abilities are), seen by anyone.
fn can_be_seen_as_enemy(w: &World, t: Who) -> bool {
    match t {
        Who::Player(k) => {
            let p = w.mob.ai.players[k];
            p.mode == GameMode::Survival && !p.invulnerable && can_be_seen_by_anyone(w, t)
        }
        e => can_be_seen_by_anyone(w, e),
    }
}

/// `Mob.canAttack(target)`: not a ghast; a player not on Peaceful; `canBeSeenAsEnemy`.
pub fn can_attack(w: &World, _i: usize, t: Who) -> bool {
    if matches!(t, Who::Player(_)) && super::difficulty_id(w) == 0 {
        return false;
    }
    can_be_seen_as_enemy(w, t)
}

/// `Mob.asValidTarget(target)`.
pub fn as_valid_target(w: &World, i: usize, t: Option<u32>) -> Option<u32> {
    let id = t?;
    let who = who(w, id)?;
    if let Who::Player(k) = who
        && w.mob.ai.players[k].mode != GameMode::Survival
    {
        return None;
    }
    if !can_attack(w, i, who) {
        return None;
    }
    Some(id)
}

/// `Mob.getTarget()`.
pub fn get_target(w: &World, i: usize) -> Option<u32> {
    as_valid_target(w, i, ai(w, i).target)
}

/// `Mob.setTarget(target)` (an enderman's override keeps its target change time: the tick count, 0
/// without one).
pub fn set_target(w: &mut World, i: usize, t: Option<u32>) {
    let v = as_valid_target(w, i, t);
    let before = ai(w, i).target;
    // (tickexact 2026-11-10) ZombifiedPiglin.setTarget: a first target draws its anger-sound
    // delay and alert interval
    super::goal::zombie::piglin_on_set_target(w, i, before, v);
    ai_mut(w, i).target = v;
    let now = w.entities.list[i].tick_count;
    if let Some(s) = ai_mut(w, i).enderman.as_mut() {
        s.target_change_time = if v.is_some() { now } else { 0 };
    }
}

/// `Sensing.hasLineOfSight(target)` (cached for this AI step). An entity of another level is never
/// in sight (`LivingEntity.hasLineOfSight`: `entity.level() != this.level()`).
pub fn has_line_of_sight(w: &mut World, i: usize, t: Who) -> bool {
    if matches!(t, Who::Foreign(_)) {
        return false;
    }
    let id = id_of(w, t);
    if ai(w, i).sensing.seen.contains(&id) {
        return true;
    }
    if ai(w, i).sensing.unseen.contains(&id) {
        return false;
    }
    let from = super::eye_pos(w, i);
    let to = eye_of(w, t);
    let v = los::has_line_of_sight(w, from, to);
    let a = ai_mut(w, i);
    if v {
        a.sensing.seen.push(id);
    } else {
        a.sensing.unseen.push(id);
    }
    v
}

/// `getVisibilityPercent(level, targeter)`: an invisible mob (a spider's spawn effect) 0.7 x its
/// armour cover (at least 0.1: an invisible spider wears none, so 0.7 x 0.1F); no sneaking or mob
/// heads modeled.
pub fn visibility_percent(w: &World, t: Who, _i: usize) -> f64 {
    match t {
        Who::Entity(j) if matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.effects.invisibility > 0) => 0.7 * (0.1f32 as f64),
        _ => 1.0,
    }
}

/// `TargetingConditions`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Conditions {
    pub combat: bool,
    pub range: f64,
    pub check_line_of_sight: bool,
    pub test_invisible: bool,
}

impl Conditions {
    pub fn combat() -> Conditions {
        Conditions { combat: true, range: -1.0, check_line_of_sight: true, test_invisible: true }
    }

    pub fn non_combat() -> Conditions {
        Conditions { combat: false, range: -1.0, check_line_of_sight: true, test_invisible: true }
    }

    pub fn range(mut self, r: f64) -> Conditions {
        self.range = r;
        self
    }

    pub fn ignore_line_of_sight(mut self) -> Conditions {
        self.check_line_of_sight = false;
        self
    }

    pub fn ignore_invisibility(mut self) -> Conditions {
        self.test_invisible = false;
        self
    }

    /// `test(level, targeter, target)` with a selector already applied by the caller.
    pub fn test(&self, w: &mut World, i: usize, t: Who) -> bool {
        if let Who::Entity(j) = t
            && j == i
        {
            return false;
        }
        if !can_be_seen_by_anyone(w, t) {
            return false;
        }
        if self.combat && !can_attack(w, i, t) {
            return false;
        }
        if self.range > 0.0 {
            let m = if self.test_invisible { visibility_percent(w, t, i) } else { 1.0 };
            let vd = (self.range * m).max(2.0);
            let tp = pos_of(w, t);
            let d = w.entities.list[i].pos.distance_to_sqr(tp);
            if d > vd * vd {
                return false;
            }
        }
        if self.check_line_of_sight && !has_line_of_sight(w, i, t) {
            return false;
        }
        true
    }
}

/// `getNearestEntity(list, conditions, source, x, y, z)`: the first nearest that passes.
pub fn nearest(w: &mut World, i: usize, list: &[Who], cond: &Conditions, at: Vec3) -> Option<Who> {
    let mut best = -1.0f64;
    let mut out = None;
    for &t in list {
        if cond.test(w, i, t) {
            let d = pos_of(w, t).distance_to_sqr(at);
            if best == -1.0 || d < best {
                best = d;
                out = Some(t);
            }
        }
    }
    out
}

/// `level.players()` as targets.
pub fn players(w: &World) -> Vec<Who> {
    (0..w.mob.ai.players.len()).map(Who::Player).collect()
}

/// `getEntitiesOfClass(class, box, any)`: the mobs of the types `want` accepts whose box meets
/// `bb`, in the level's section order.
pub fn mobs_in(w: &World, bb: Aabb, want: impl Fn(MobType) -> bool) -> Vec<Who> {
    w.get_entities(bb, |e| matches!(&e.kind, EntityKind::Mob(m) if want(m.ty))).into_iter().map(Who::Entity).collect()
}

/// The mob type of a target, `None` for a player.
pub fn type_of(w: &World, t: Who) -> Option<MobType> {
    match t {
        Who::Entity(j) => match &w.entities.list[j].kind {
            EntityKind::Mob(m) => Some(m.ty),
            _ => None,
        },
        Who::Player(_) => None,
        Who::Foreign(f) => w.foreign[f].ty,
    }
}

/// Whether mob `i`'s type is in the given set of type ids.
pub fn is_type(w: &World, i: usize, ids: &[&str]) -> bool {
    ids.contains(&mob(w, i).ty.def().id)
}
