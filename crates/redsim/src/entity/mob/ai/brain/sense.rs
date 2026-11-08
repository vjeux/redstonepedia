//! The villager's sensors (26.3 `Brain.tickSensors` in `BRAIN_PROVIDER`'s order, each
//! `Sensor.tick`: `--timeToTick <= 0` runs it and waits its scan rate): nearest living entities
//! (and the visible ones, with their lazy line-of-sight cache), players, wanted items, the nearest
//! bed (babies), what hurt it, hostiles, villager babies, secondary job sites, golems.

use super::{brain, brain_mut, ex, ex_mut};
use crate::entity::mob::ai::target::{self, Conditions, Who};
use crate::entity::mob::types::MobType;
use crate::entity::mob::villager::{self, SENSORS};
use crate::entity::mob::{Damage, mob};
use crate::entity::{EntityKind, Vec3};
use crate::world::World;

pub fn tick_sensors(w: &mut World, i: usize) {
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
        if !fire {
            continue;
        }
        match k {
            0 => nearest_living(w, i),
            1 => players(w, i),
            2 => items(w, i),
            3 => nearest_bed(w, i),
            4 => hurt_by(w, i),
            5 => hostiles(w, i),
            6 => babies(w, i),
            7 => secondary_pois(w, i),
            8 => golem(w, i),
            _ => {}
        }
    }
}

fn refuse(w: &mut World, why: &str) {
    let s = why.to_string();
    if !w.errors.contains(&s) {
        w.errors.push(s);
    }
}

/// `NearestLivingEntitySensor`: every alive living entity but itself whose box meets its box
/// inflated by the follow range, sorted by distance (stable over the level's order); the visible
/// list with a fresh line-of-sight cache.
fn nearest_living(w: &mut World, i: usize) {
    let (me, pos, bb, range) = {
        let e = &w.entities.list[i];
        (e.id, e.pos, e.bb, mob(w, i).follow_range)
    };
    let query = bb.inflate(range, range, range);
    let ids = w.get_entities(query, |e| e.id != me && matches!(&e.kind, EntityKind::Mob(m) if m.health > 0.0) || matches!(&e.kind, EntityKind::ArmorStand(_)));
    if ids.iter().any(|&j| matches!(w.entities.list[j].kind, EntityKind::ArmorStand(_))) {
        refuse(w, "an armor stand in a villager's sight (a living entity its sensors list) is not modeled");
    }
    let player_near = w.mob.ai.players.iter().any(|p| crate::entity::make_bounding_box(p.pos, target::PLAYER_WIDTH, target::PLAYER_HEIGHT).intersects(query));
    if player_near {
        refuse(w, "a player near a villager (its sensors' living entities) is not modeled yet");
    }
    let mut found: Vec<(f64, u32, MobType)> = ids
        .into_iter()
        .filter(|&j| matches!(w.entities.list[j].kind, EntityKind::Mob(_)))
        .map(|j| {
            let e = &w.entities.list[j];
            (pos.distance_to_sqr(e.pos), e.id, mob(w, j).ty)
        })
        .collect();
    found.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let list: Vec<(u32, MobType)> = found.into_iter().map(|(_, id, ty)| (id, ty)).collect();
    let b = brain_mut(w, i);
    b.visible = villager::Visible { entities: list.clone(), los: Vec::new() };
    b.visible_present = true;
    b.nearest_living = list;
}

/// `NearestVisibleLivingEntities`'s line-of-sight test (`Sensor.isEntityTargetable`: non-combat
/// conditions at the follow range, the mob's sight cache), cached per list.
pub fn visible_test(w: &mut World, i: usize, id: u32) -> bool {
    if let Some(v) = brain(w, i).visible.los.iter().find(|(e, _)| *e == id).map(|(_, v)| *v) {
        return v;
    }
    let v = match target::who(w, id) {
        Some(t) => {
            let range = mob(w, i).follow_range;
            Conditions::non_combat().range(range).test(w, i, t)
        }
        None => false,
    };
    brain_mut(w, i).visible.los.push((id, v));
    v
}

/// `NearestVisibleLivingEntities.findClosest(filter)`.
pub fn find_closest(w: &mut World, i: usize, filter: impl Fn(&World, u32, MobType) -> bool) -> Option<u32> {
    if !brain(w, i).visible_present {
        return None;
    }
    let list = brain(w, i).visible.entities.clone();
    for (id, ty) in list {
        if filter(w, id, ty) && visible_test(w, i, id) {
            return Some(id);
        }
    }
    None
}

/// `NearestVisibleLivingEntities.contains(entity)`.
pub fn visible_contains(w: &mut World, i: usize, id: u32) -> bool {
    brain(w, i).visible_present && brain(w, i).visible.entities.iter().any(|(e, _)| *e == id) && visible_test(w, i, id)
}

/// `PlayerSensor`.
fn players(w: &mut World, i: usize) {
    let range = mob(w, i).follow_range;
    let pos = w.entities.list[i].pos;
    let mut near: Vec<(f64, usize)> = w
        .mob
        .ai
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.mode != crate::entity::mob::ai::GameMode::Spectator)
        .map(|(k, p)| (pos.distance_to_sqr(p.pos), k))
        .filter(|(d, _)| *d < range * range)
        .collect();
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let ids: Vec<u32> = near.iter().map(|(_, k)| crate::entity::mob::ai::PLAYER_ID_BASE + *k as u32).collect();
    let mut visible = Vec::new();
    for (_, k) in &near {
        if Conditions::non_combat().range(range).test(w, i, Who::Player(*k)) {
            visible.push(crate::entity::mob::ai::PLAYER_ID_BASE + *k as u32);
        }
    }
    let mut attackable = Vec::new();
    for &id in &visible {
        if let Some(t) = target::who(w, id)
            && Conditions::combat().range(range).test(w, i, t)
        {
            attackable.push(id);
        }
    }
    let x = ex_mut(w, i);
    x.mem.nearest_players = ids;
    x.mem.nearest_visible_player = visible.first().copied();
    x.mem.nearest_visible_attackable_players = attackable.clone();
    x.mem.nearest_visible_attackable_player = attackable.first().copied();
}

/// The items a villager wants (`Villager.wantsToPickUp`: `WANTED_ITEMS`, its profession's requested
/// items; an unemployed villager's are none).
fn wanted(item: &str) -> bool {
    matches!(item.trim_start_matches("minecraft:"), "bread" | "potato" | "carrot" | "wheat" | "wheat_seeds" | "beetroot" | "beetroot_seeds" | "torchflower_seeds" | "pitcher_pod")
}

/// `NearestItemSensor`.
fn items(w: &mut World, i: usize) {
    let (pos, bb) = (w.entities.list[i].pos, w.entities.list[i].bb);
    let ids = w.get_entities(bb.inflate(32.0, 16.0, 32.0), |e| matches!(e.kind, EntityKind::Item(_)));
    let mut items: Vec<(f64, usize)> = ids.into_iter().map(|j| (pos.distance_to_sqr(w.entities.list[j].pos), j)).collect();
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut found = None;
    for (_, j) in items {
        let EntityKind::Item(it) = &w.entities.list[j].kind else { continue };
        if !wanted(&it.item) {
            continue;
        }
        refuse(w, "an item a villager wants in its sight (NearestItemSensor, GoToWantedItem) is not modeled");
        found = Some(w.entities.list[j].id);
        break;
    }
    ex_mut(w, i).mem.nearest_visible_wanted_item = found;
}

/// `NearestBedSensor` (babies only).
fn nearest_bed(w: &mut World, i: usize) {
    if ex(w, i).baby {
        refuse(w, "a baby villager's nearest-bed sensor is not modeled");
    }
}

/// `HurtBySensor`: the last damage under 41 ticks old, and its attacker.
fn hurt_by(w: &mut World, i: usize) {
    let t = w.time;
    let last = mob(w, i).last_damage;
    let attacker = mob(w, i).ai.as_deref().and_then(|a| a.last_damage_entity);
    match last.filter(|(at, _)| t - at <= 40) {
        Some((_, src)) => {
            let by = if src == Damage::MobAttack { attacker } else { None };
            let x = ex_mut(w, i);
            x.mem.hurt_by = Some((src, by));
            if let Some(a) = by {
                x.mem.hurt_by_entity = Some(a);
            }
        }
        None => ex_mut(w, i).mem.hurt_by = None,
    }
    if let Some(a) = ex(w, i).mem.hurt_by_entity
        && !target::who(w, a).is_some_and(|t| target::is_alive(w, t))
    {
        ex_mut(w, i).mem.hurt_by_entity = None;
    }
    brain_mut(w, i).hurt_by = ex(w, i).mem.hurt_by.map(|_| t);
}

/// `VillagerHostilesSensor.ACCEPTABLE_DISTANCE_FROM_HOSTILES`.
pub fn hostile_distance(ty: MobType) -> Option<f32> {
    match ty.def().id {
        "drowned" | "husk" | "zombie" | "zombie_villager" | "baby_zombie" | "baby_zombie_villager" | "baby_drowned" | "vex" => Some(8.0),
        "evoker" | "illusioner" | "ravager" => Some(12.0),
        "pillager" => Some(15.0),
        "vindicator" | "zoglin" => Some(10.0),
        _ => None,
    }
}

/// `VillagerHostilesSensor`: the closest visible hostile within its distance.
fn hostiles(w: &mut World, i: usize) {
    let me = w.entities.list[i].pos;
    let found = find_closest(w, i, |w, id, ty| {
        let Some(limit) = hostile_distance(ty) else { return false };
        let Some(j) = crate::entity::index_of(w, id) else { return false };
        w.entities.list[j].pos.distance_to_sqr(me) <= (limit * limit) as f64
    });
    brain_mut(w, i).nearest_hostile = found;
}

/// `VillagerBabiesSensor`: the visible villager babies (none are modeled).
fn babies(w: &mut World, i: usize) {
    ex_mut(w, i).mem.visible_villager_babies.clear();
}

/// `SecondaryPoiSensor`: an unemployed villager has no secondary job sites.
fn secondary_pois(w: &mut World, i: usize) {
    ex_mut(w, i).mem.secondary_job_site.clear();
}

/// `GolemSensor`.
fn golem(w: &mut World, i: usize) {
    if brain(w, i).nearest_living.iter().any(|(_, ty)| *ty == MobType::IronGolem) {
        villager::golem_detected(w, i);
        let (t, id) = (w.time, w.entities.list[i].id);
        w.mob.events.push(crate::entity::mob::MobEvent::GolemSeen { t, villager: id });
    }
}

#[allow(dead_code)]
fn _v(_: Vec3) {}
