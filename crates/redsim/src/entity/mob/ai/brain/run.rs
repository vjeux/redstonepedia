//! The villager brain's behaviours: `tryStart`, `tickOrStop` and `doStop` of each (26.3
//! `Behavior`, `DoNothing`, `OneShot`, `GateBehavior`, `TriggerGate` and the classes of
//! `VillagerGoalPackages`), read in the source. A behaviour the modeled worlds cannot reach (trading,
//! players' gifts, items, job sites, bells, raids, babies) records a model error when it would act.

use super::tree::{InteractKind, Kind, LookFilter, PoiSel, Retry};
use super::{Act, M, Status, Tracker, WalkTarget, brain, brain_mut, check, erase, ex, ex_mut, has, is_active, set_active_if_possible, update_activity_from_schedule};
use crate::entity::mob::ai::goal::stroll;
use crate::entity::mob::ai::{ai, ai_mut, head_rot_speed, max_head_x_rot, with_nav};
use crate::entity::mob::types::MobType;
use crate::entity::mob::{mob, villager};
use crate::entity::{EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

fn refuse(w: &mut World, why: &str) {
    let s = why.to_string();
    if !w.errors.contains(&s) {
        w.errors.push(s);
    }
}

/// The level random (mirrored when the world mirrors the bridge's; else the mob layer's stand-in).
pub fn level_rng(w: &mut World) -> &mut crate::rng::LegacyRandom {
    if w.level_random.is_some() {
        return w.level_random.as_mut().unwrap();
    }
    w.mob.rng_mut()
}

fn kind(w: &World, i: usize, n: usize) -> &Kind {
    &ex(w, i).nodes[n].kind
}

fn kind_mut(w: &mut World, i: usize, n: usize) -> &mut Kind {
    &mut ex_mut(w, i).nodes[n].kind
}

// ------------------------------------------------------------------------ trackers

/// `PositionTracker.currentPosition()`.
pub fn tracker_pos(w: &World, t: &Tracker) -> Option<Vec3> {
    Some(match *t {
        Tracker::Block(p) => Vec3::new(p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5),
        Tracker::Point(v) => v,
        Tracker::Entity { id, eye } => {
            let who = crate::entity::mob::ai::target::who(w, id)?;
            if eye { crate::entity::mob::ai::target::eye_of(w, who) } else { crate::entity::mob::ai::target::pos_of(w, who) }
        }
    })
}

/// `PositionTracker.currentBlockPosition()` (an entity's feet block).
pub fn tracker_block(w: &World, t: &Tracker) -> Option<BlockPos> {
    match *t {
        Tracker::Block(p) => Some(p),
        Tracker::Point(v) => Some(super::containing(v)),
        Tracker::Entity { id, .. } => {
            let who = crate::entity::mob::ai::target::who(w, id)?;
            Some(super::containing(crate::entity::mob::ai::target::pos_of(w, who)))
        }
    }
}

/// `PositionTracker.isVisibleBy(body)`.
fn tracker_visible(w: &mut World, i: usize, t: &Tracker) -> bool {
    match *t {
        Tracker::Entity { id, .. } => {
            let alive = crate::entity::mob::ai::target::who(w, id).is_some_and(|x| crate::entity::mob::ai::target::is_alive(w, x));
            alive && super::sense::visible_contains(w, i, id)
        }
        _ => true,
    }
}

/// `lookControl.setLookAt(pos)` at the mob's own speeds.
fn look_at(w: &mut World, i: usize, p: Vec3) {
    let (ys, xs) = (head_rot_speed(w, i) as f32, max_head_x_rot(w, i) as f32);
    ai_mut(w, i).look.set_look_at(p.x, p.y, p.z, ys, xs);
}

// ------------------------------------------------------------------------ dispatch

/// `BehaviorControl.tryStart`.
pub fn try_start(w: &mut World, i: usize, n: usize, t: i64) -> bool {
    let node = &ex(w, i).nodes[n];
    if let Some((min, max)) = node.durations() {
        let is_do_nothing = matches!(node.kind, Kind::DoNothing { .. });
        if !is_do_nothing && !(entry_ok(w, i, n) && extra_start(w, i, n)) {
            return false;
        }
        ex_mut(w, i).nodes[n].running = true;
        let d = min + level_rng(w).next_int(max + 1 - min);
        ex_mut(w, i).nodes[n].end = t + d as i64;
        if !is_do_nothing {
            start(w, i, n, t);
        }
        return true;
    }
    match node.kind {
        Kind::Gate { .. } => gate_try_start(w, i, n, t),
        Kind::TriggerGate => {
            trigger_gate(w, i, n, t);
            ex_mut(w, i).nodes[n].running = true;
            true
        }
        _ => {
            if trigger(w, i, n, t) {
                ex_mut(w, i).nodes[n].running = true;
                true
            } else {
                false
            }
        }
    }
}

/// `BehaviorControl.tickOrStop` (run whatever the status, as the game runs it over its list).
pub fn tick_or_stop(w: &mut World, i: usize, n: usize, t: i64) {
    let node = &ex(w, i).nodes[n];
    if node.durations().is_some() {
        if let Kind::DoNothing { .. } = node.kind {
            if t > node.end {
                do_stop(w, i, n, t);
            }
            return;
        }
        if !timed_out(w, i, n, t) && can_still_use(w, i, n, t) {
            tick(w, i, n, t);
        } else {
            do_stop(w, i, n, t);
        }
        return;
    }
    match node.kind {
        Kind::Gate { .. } => {
            let kids: Vec<usize> = ex(w, i).nodes[n].children.iter().map(|c| c.0).collect();
            for &c in &kids {
                if ex(w, i).nodes[c].running {
                    tick_or_stop(w, i, c, t);
                }
            }
            if !kids.iter().any(|&c| ex(w, i).nodes[c].running) {
                do_stop(w, i, n, t);
            }
        }
        _ => ex_mut(w, i).nodes[n].running = false,
    }
}

/// `BehaviorControl.doStop`.
pub fn do_stop(w: &mut World, i: usize, n: usize, t: i64) {
    ex_mut(w, i).nodes[n].running = false;
    let node = &ex(w, i).nodes[n];
    if node.durations().is_some() {
        if !matches!(node.kind, Kind::DoNothing { .. }) {
            stop(w, i, n, t);
        }
        return;
    }
    if let Kind::Gate { exit_erased, .. } = &node.kind {
        let erased = exit_erased.clone();
        let kids: Vec<usize> = node.children.iter().map(|c| c.0).collect();
        for c in kids {
            if ex(w, i).nodes[c].running {
                do_stop(w, i, c, t);
            }
        }
        for m in erased {
            erase(w, i, m);
        }
    }
}

/// `GateBehavior.tryStart`: the entry condition, the list shuffled (`SHUFFLED`), then `RUN_ONE`
/// (the first stopped child that starts) or `TRY_ALL`.
fn gate_try_start(w: &mut World, i: usize, n: usize, t: i64) -> bool {
    let (entry, shuffled, run_one) = match &ex(w, i).nodes[n].kind {
        Kind::Gate { entry, shuffled, run_one, .. } => (entry.clone(), *shuffled, *run_one),
        _ => unreachable!(),
    };
    if !entry.iter().all(|&(m, s)| check(w, i, m, s)) {
        return false;
    }
    ex_mut(w, i).nodes[n].running = true;
    if shuffled {
        shuffle(w, i, n);
    }
    let kids: Vec<usize> = ex(w, i).nodes[n].children.iter().map(|c| c.0).collect();
    for c in kids {
        if ex(w, i).nodes[c].running {
            continue;
        }
        if try_start(w, i, c, t) && run_one {
            break;
        }
    }
    true
}

/// `ShufflingList.shuffle()`: a new random weight per entry (`-pow(nextFloat, 1 / weight)`), then a
/// stable sort. (fdlibm's `pow`: the same on every platform, the wasm viewer's included.)
fn shuffle(w: &mut World, i: usize, n: usize) {
    let node = &mut ex_mut(w, i).nodes[n];
    let mut r = node.shuffle.take().expect("a shuffling list");
    for c in node.children.iter_mut() {
        let f = r.next_float();
        c.2 = -crate::entity::fdlibm::pow(f as f64, (1.0f32 / c.1 as f32) as f64);
    }
    node.children.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    node.shuffle = Some(r);
}

/// `TriggerGate.triggerGate`'s trigger: shuffled, each child's trigger until one fires (`RUN_ONE`).
fn trigger_gate(w: &mut World, i: usize, n: usize, t: i64) {
    shuffle(w, i, n);
    let kids: Vec<usize> = ex(w, i).nodes[n].children.iter().map(|c| c.0).collect();
    for c in kids {
        if trigger(w, i, c, t) {
            break;
        }
    }
}

// ------------------------------------------------------------------------ timed behaviours

/// `Behavior.hasRequiredMemories`.
fn entry_ok(w: &World, i: usize, n: usize) -> bool {
    use Status::*;
    let req: &[(M, Status)] = match kind(w, i, n) {
        Kind::LookAtTargetSink => &[(M::LookTarget, Present)],
        Kind::MoveToTargetSink { .. } => &[(M::CantReachSince, Registered), (M::Path, Absent), (M::WalkTarget, Present)],
        Kind::GoToPotentialJobSite => &[(M::PotentialJobSite, Present)],
        Kind::SleepInBed { .. } => &[(M::Home, Present)],
        Kind::GiveGiftToHero { .. } => &[(M::NearestVisiblePlayer, Present)],
        Kind::ShowTradesToPlayer => &[(M::InteractionTarget, Present)],
        Kind::TradeWithVillager => &[(M::InteractionTarget, Present), (M::NearestVisibleLiving, Present)],
        Kind::VillagerMakeLove => &[(M::BreedTarget, Present), (M::NearestVisibleLiving, Present)],
        Kind::JumpOnBed => &[(M::NearestBed, Present), (M::WalkTarget, Absent)],
        Kind::WorkAtPoi { .. } => &[(M::JobSite, Present)],
        Kind::HarvestFarmland => &[(M::LookTarget, Absent), (M::WalkTarget, Absent), (M::SecondaryJobSite, Present)],
        Kind::UseBonemeal => &[(M::LookTarget, Absent), (M::WalkTarget, Absent)],
        _ => &[],
    };
    req.iter().all(|&(m, s)| check(w, i, m, s))
}

/// `checkExtraStartConditions`.
fn extra_start(w: &mut World, i: usize, n: usize) -> bool {
    match kind(w, i, n).clone() {
        Kind::Swim => swim_condition(w, i),
        Kind::MoveToTargetSink { .. } => move_extra(w, i, n),
        Kind::LookAndFollowTradingPlayerSink => false,
        Kind::GoToPotentialJobSite => matches!(ex(w, i).active, Act::Idle | Act::Work | Act::Play),
        Kind::SleepInBed { .. } => sleep_extra(w, i),
        Kind::GiveGiftToHero { .. } => {
            refuse(w, "a villager's gift to a hero of the village is not modeled");
            false
        }
        Kind::ShowTradesToPlayer => {
            let tgt = ex(w, i).mem.interaction_target;
            if tgt.is_some_and(|id| id >= crate::entity::mob::ai::PLAYER_ID_BASE) {
                refuse(w, "a villager showing trades to a player is not modeled");
            }
            false
        }
        Kind::TradeWithVillager => target_is_valid_villager(w, i, ex(w, i).mem.interaction_target),
        Kind::VillagerMakeLove => {
            if target_is_valid_villager(w, i, ex(w, i).mem.breed_target) {
                refuse(w, "villager breeding is not modeled");
            }
            false
        }
        Kind::JumpOnBed => false,
        Kind::WorkAtPoi { .. } | Kind::HarvestFarmland => {
            refuse(w, "a villager at a job site is not modeled");
            false
        }
        Kind::UseBonemeal => use_bonemeal_extra(w, i),
        Kind::CelebrateVillagersSurvivedRaid => {
            refuse(w, "a raid is not modeled");
            false
        }
        _ => true,
    }
}

/// `UseBonemeal.checkExtraStartConditions`: every 10th tick and no recent session, bone meal in the
/// inventory (a villager has none here).
fn use_bonemeal_extra(_w: &mut World, _i: usize) -> bool {
    false
}

/// `BehaviorUtils.targetIsValid(brain, memory, VILLAGER)`: a living villager the brain sees.
fn target_is_valid_villager(w: &mut World, i: usize, id: Option<u32>) -> bool {
    let Some(id) = id else { return false };
    let Some(j) = crate::entity::index_of(w, id) else { return false };
    let ok = matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.ty == MobType::Villager && m.health > 0.0) && !w.entities.list[j].removed;
    ok && super::sense::visible_contains(w, i, id)
}

/// `Swim`'s condition: in water deeper than the jump threshold, or in lava.
fn swim_condition(w: &World, i: usize) -> bool {
    let e = &w.entities.list[i];
    let m = mob(w, i);
    let threshold = if (m.eye() as f64) < 0.4 { 0.0 } else { 0.4 };
    e.fluid.water_height() > threshold || (!e.first_tick && m.lava_height > 0.0)
}

fn timed_out(w: &World, i: usize, n: usize, t: i64) -> bool {
    match kind(w, i, n) {
        Kind::LookAndFollowTradingPlayerSink | Kind::SleepInBed { .. } | Kind::JumpOnBed => false,
        _ => t > ex(w, i).nodes[n].end,
    }
}

/// `canStillUse`.
fn can_still_use(w: &mut World, i: usize, n: usize, _t: i64) -> bool {
    match kind(w, i, n).clone() {
        Kind::Swim => swim_condition(w, i),
        Kind::LookAtTargetSink => match ex(w, i).mem.look_target {
            Some(tr) => tracker_visible(w, i, &tr),
            None => false,
        },
        Kind::VillagerPanicTrigger => scared(w, i),
        Kind::MoveToTargetSink { .. } => move_can_still_use(w, i, n),
        Kind::GoToPotentialJobSite => has(w, i, M::PotentialJobSite),
        Kind::SleepInBed { .. } => {
            let Some(bed) = brain(w, i).home else { return false };
            let pos = w.entities.list[i].pos;
            is_active(w, i, Act::Rest) && pos.y > bed.y as f64 + 0.4 && villager::closer_to_center(bed, pos, 1.14)
        }
        Kind::TradeWithVillager => target_is_valid_villager(w, i, ex(w, i).mem.interaction_target),
        _ => false,
    }
}

/// `VillagerPanicTrigger.isHurt || hasHostile`.
fn scared(w: &World, i: usize) -> bool {
    has(w, i, M::HurtBy) || has(w, i, M::NearestHostile)
}

/// `start`.
fn start(w: &mut World, i: usize, n: usize, t: i64) {
    match kind(w, i, n).clone() {
        Kind::VillagerPanicTrigger => {
            if scared(w, i) {
                if !is_active(w, i, Act::Panic) {
                    for m in [M::Path, M::WalkTarget, M::LookTarget, M::BreedTarget, M::InteractionTarget] {
                        erase(w, i, m);
                    }
                }
                set_active_if_possible(w, i, Act::Panic);
            }
        }
        Kind::MoveToTargetSink { .. } => move_start(w, i, n),
        Kind::SleepInBed { next_ok } => {
            if t > next_ok {
                // the doors it opened or passed through, closed before it lies down
                if ex(w, i).mem.doors_to_close.is_some() {
                    close_doors_passed(w, i, None, None);
                }
                let home = brain(w, i).home.expect("a home");
                if villager::start_sleeping(w, i, home) {
                    brain_mut(w, i).last_slept = Some(t);
                    let id = w.entities.list[i].id;
                    w.mob.events.push(crate::entity::mob::MobEvent::Slept { t, id });
                }
                erase(w, i, M::WalkTarget);
                erase(w, i, M::CantReachSince);
            }
        }
        Kind::TradeWithVillager => {
            // lockGazeAndWalkToEachOther(body, target, 0.5, 2); an unemployed villager wants no
            // trade items
            if let Some(tid) = ex(w, i).mem.interaction_target {
                lock_gaze_and_walk(w, i, tid, 0.5, 2);
            }
        }
        _ => {}
    }
}

/// `BehaviorUtils.lockGazeAndWalkToEachOther(a, b, speed, close)`: each looks at the other's eyes
/// and walks to it (entity trackers with the eye).
fn lock_gaze_and_walk(w: &mut World, i: usize, tid: u32, speed: f32, close: i32) {
    let me = w.entities.list[i].id;
    let Some(j) = crate::entity::index_of(w, tid) else { return };
    let has_ex = |w: &World, k: usize| matches!(&w.entities.list[k].kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.ex.is_some()));
    if !has_ex(w, j) {
        refuse(w, "a villager locking gaze with a villager without the exact brain is not modeled");
        return;
    }
    let to_target = Tracker::Entity { id: tid, eye: true };
    let to_me = Tracker::Entity { id: me, eye: true };
    ex_mut(w, i).mem.look_target = Some(to_target);
    ex_mut(w, j).mem.look_target = Some(to_me);
    ex_mut(w, i).mem.look_target = Some(to_target);
    ex_mut(w, i).mem.walk_target = Some(WalkTarget { target: to_target, speed, close });
    ex_mut(w, j).mem.look_target = Some(to_me);
    ex_mut(w, j).mem.walk_target = Some(WalkTarget { target: to_me, speed, close });
}

/// `Villager.gossip(level, target, timestamp)`: at most once in 1200 ticks for both, their
/// gossip (none without players: no draws) and `spawnGolemIfNeeded(level, timestamp, 5)`.
fn gossip(w: &mut World, i: usize, j: usize, t: i64) {
    let ok = |last: i64| t < last || t >= last + 1200;
    let (li, lj) = (ex(w, i).last_gossip, ex(w, j).last_gossip);
    if ok(li) && ok(lj) {
        ex_mut(w, i).last_gossip = t;
        ex_mut(w, j).last_gossip = t;
        crate::entity::mob::spawn::spawn_golem_if_needed(w, i, 5);
    }
}

/// `tick`.
fn tick(w: &mut World, i: usize, n: usize, t: i64) {
    match kind(w, i, n).clone() {
        Kind::Swim => {
            let f = w.entities.list[i].random.as_mut().expect("random").next_float();
            if f < 0.8 {
                ai_mut(w, i).jump = true;
            }
        }
        Kind::LookAtTargetSink => {
            if let Some(tr) = ex(w, i).mem.look_target
                && let Some(p) = tracker_pos(w, &tr)
            {
                look_at(w, i, p);
            }
        }
        Kind::VillagerPanicTrigger => {
            if t % 100 == 0 {
                crate::entity::mob::spawn::spawn_golem_if_needed(w, i, 3);
            }
        }
        Kind::MoveToTargetSink { .. } => move_tick(w, i, n, t),
        Kind::TradeWithVillager => {
            // within sqrt(5): the gaze again, the gossip; food and wheat: villagers carry nothing
            // here (an item pickup is refused)
            if let Some(tid) = ex(w, i).mem.interaction_target
                && let Some(j) = crate::entity::index_of(w, tid)
                && w.entities.list[i].pos.distance_to_sqr(w.entities.list[j].pos) <= 5.0
            {
                lock_gaze_and_walk(w, i, tid, 0.5, 2);
                if matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.brain.as_deref().is_some_and(|b| b.ex.is_some())) {
                    gossip(w, i, j, t);
                }
            }
        }
        Kind::GoToPotentialJobSite => refuse(w, "a villager walking to a potential job site is not modeled"),
        _ => {}
    }
}

/// `stop`.
fn stop(w: &mut World, i: usize, n: usize, t: i64) {
    match kind(w, i, n).clone() {
        Kind::LookAtTargetSink => erase(w, i, M::LookTarget),
        Kind::MoveToTargetSink { .. } => move_stop(w, i, n),
        Kind::SleepInBed { .. } => {
            if mob(w, i).sleeping_pos.is_some() {
                villager::stop_sleeping(w, i);
                if let Kind::SleepInBed { next_ok } = kind_mut(w, i, n) {
                    *next_ok = t + 40;
                }
            }
        }
        Kind::GoToPotentialJobSite => {
            refuse(w, "a villager's potential job site is not modeled");
            erase(w, i, M::PotentialJobSite);
        }
        Kind::TradeWithVillager => erase(w, i, M::InteractionTarget),
        Kind::VillagerMakeLove => erase(w, i, M::BreedTarget),
        _ => {}
    }
}

/// `SleepInBed.checkExtraStartConditions`.
fn sleep_extra(w: &World, i: usize) -> bool {
    if w.entities.list[i].vehicle.is_some() {
        return false;
    }
    let b = brain(w, i);
    let Some(home) = b.home else { return false };
    if let Some(woken) = b.last_woken {
        let since = w.time - woken;
        if since > 0 && since < 100 {
            return false;
        }
    }
    let pos = w.entities.list[i].pos;
    villager::closer_to_center(home, pos, 2.0) && matches!(w.get(home), crate::block::Block::Bed { occupied: false, .. })
}

// ------------------------------------------------------------------------ MoveToTargetSink

fn move_state(w: &mut World, i: usize, n: usize) -> (&mut i32, &mut Option<crate::entity::mob::ai::path::Path>, &mut Option<BlockPos>, &mut f32) {
    match kind_mut(w, i, n) {
        Kind::MoveToTargetSink { remaining_cooldown, path, last_target, speed, .. } => (remaining_cooldown, path, last_target, speed),
        _ => unreachable!(),
    }
}

/// `MoveToTargetSink.reachedTarget`: the target block within its close-enough Manhattan distance.
fn reached(w: &World, i: usize, wt: &WalkTarget) -> bool {
    let Some(tb) = tracker_block(w, &wt.target) else { return false };
    let me = w.entities.list[i].block_position();
    (tb.x - me.x).abs() + (tb.y - me.y).abs() + (tb.z - me.z).abs() <= wt.close
}

fn move_extra(w: &mut World, i: usize, n: usize) -> bool {
    {
        let (cool, _, _, _) = move_state(w, i, n);
        if *cool > 0 {
            *cool -= 1;
            return false;
        }
    }
    let wt = ex(w, i).mem.walk_target.expect("a walk target");
    let r = reached(w, i, &wt);
    if !r && try_compute_path(w, i, n, &wt) {
        let tb = tracker_block(w, &wt.target);
        *move_state(w, i, n).2 = tb;
        return true;
    }
    erase(w, i, M::WalkTarget);
    if r {
        erase(w, i, M::CantReachSince);
    }
    false
}

/// `MoveToTargetSink.tryComputePath`.
fn try_compute_path(w: &mut World, i: usize, n: usize, wt: &WalkTarget) -> bool {
    let Some(target) = tracker_block(w, &wt.target) else { return false };
    let path = with_nav(w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, target, 0, arena));
    *move_state(w, i, n).1 = path.clone();
    *move_state(w, i, n).3 = wt.speed;
    if reached(w, i, wt) {
        erase(w, i, M::CantReachSince);
    } else {
        let can_reach = path.as_ref().is_some_and(|p| p.reached);
        if can_reach {
            erase(w, i, M::CantReachSince);
        } else if !has(w, i, M::CantReachSince) {
            ex_mut(w, i).mem.cant_reach_since = Some(w.time);
        }
        if path.is_some() {
            return true;
        }
        let towards = Vec3::new(target.x as f64 + 0.5, target.y as f64, target.z as f64 + 0.5);
        if let Some(step) = stroll::default_pos_towards(w, i, 10, 7, towards, (std::f64::consts::PI / 2.0) as f32 as f64) {
            let p2 = with_nav(w, i, |w, nm, nav, arena| {
                let tb = super::containing(step);
                nav.create_path_pos(w, nm, tb, 0, arena)
            });
            let ok = p2.is_some();
            *move_state(w, i, n).1 = p2;
            return ok;
        }
    }
    false
}

fn move_start(w: &mut World, i: usize, n: usize) {
    let (path, speed) = {
        let (_, p, _, s) = move_state(w, i, n);
        (p.clone(), *s)
    };
    ex_mut(w, i).mem.path = path.clone();
    with_nav(w, i, |w, nm, nav, _| nav.move_to_path(w, nm, path, speed as f64));
}

fn move_can_still_use(w: &mut World, i: usize, n: usize) -> bool {
    let (has_path, has_last) = {
        let (_, p, l, _) = move_state(w, i, n);
        (p.is_some(), l.is_some())
    };
    if !(has_path && has_last) {
        return false;
    }
    let wt = ex(w, i).mem.walk_target;
    let spectator = wt.is_some_and(|x| matches!(x.target, Tracker::Entity { id, .. } if id >= crate::entity::mob::ai::PLAYER_ID_BASE && crate::entity::mob::ai::target::who(w, id).is_some_and(|t| matches!(t, crate::entity::mob::ai::target::Who::Player(k) if w.mob.ai.players[k].mode == crate::entity::mob::ai::GameMode::Spectator))));
    !ai(w, i).nav.is_done() && wt.is_some_and(|x| !reached(w, i, &x)) && !spectator
}

fn move_stop(w: &mut World, i: usize, n: usize) {
    if let Some(wt) = ex(w, i).mem.walk_target
        && !reached(w, i, &wt)
        && ai(w, i).nav.is_stuck
    {
        let c = level_rng(w).next_int(40);
        *move_state(w, i, n).0 = c;
    }
    with_nav(w, i, |_, _, nav, _| nav.stop());
    erase(w, i, M::WalkTarget);
    erase(w, i, M::Path);
    *move_state(w, i, n).1 = None;
}

fn move_tick(w: &mut World, i: usize, n: usize, t: i64) {
    let new_path = ai(w, i).nav.path.clone();
    let mine = move_state(w, i, n).1.as_ref().map(|p| p.uid);
    if mine != new_path.as_ref().map(|p| p.uid) {
        *move_state(w, i, n).1 = new_path.clone();
        ex_mut(w, i).mem.path = new_path.clone();
    }
    let last = *move_state(w, i, n).2;
    if new_path.is_some()
        && let Some(last) = last
    {
        let wt = ex(w, i).mem.walk_target.expect("a walk target");
        if let Some(cur) = tracker_block(w, &wt.target) {
            let (dx, dy, dz) = ((cur.x - last.x) as f64, (cur.y - last.y) as f64, (cur.z - last.z) as f64);
            if dx * dx + dy * dy + dz * dz > 4.0 && try_compute_path(w, i, n, &wt) {
                *move_state(w, i, n).2 = Some(cur);
                move_start(w, i, n);
            }
        }
    }
    let _ = t;
}

// ------------------------------------------------------------------------ one-shots

/// A `OneShot`'s trigger (its memory conditions in order, then its body).
pub fn trigger(w: &mut World, i: usize, n: usize, t: i64) -> bool {
    use Status::*;
    match kind(w, i, n).clone() {
        Kind::InteractWithDoor { .. } => {
            if !has(w, i, M::Path) {
                return false;
            }
            interact_with_door(w, i, n)
        }
        Kind::WakeUp => {
            if !is_active(w, i, Act::Rest) && mob(w, i).sleeping_pos.is_some() {
                villager::stop_sleeping(w, i);
                true
            } else {
                false
            }
        }
        Kind::ReactToBell => {
            if has(w, i, M::HeardBellTime) {
                refuse(w, "a villager hearing a bell is not modeled");
            }
            false
        }
        Kind::SetRaidStatus => level_rng(w).next_int(20) == 0,
        Kind::ValidateNearbyPoi { poi, mem } => validate_nearby_poi(w, i, poi, mem),
        Kind::PoiCompetitorScan => {
            if has(w, i, M::JobSite) {
                refuse(w, "a villager with a job site is not modeled");
            }
            false
        }
        Kind::GoToWantedItem => false,
        Kind::AcquirePoi { .. } => acquire_poi(w, i, n, t),
        Kind::YieldJobSite | Kind::AssignProfessionFromJobSite => {
            if has(w, i, M::PotentialJobSite) {
                refuse(w, "a villager's potential job site is not modeled");
            }
            false
        }
        Kind::ResetProfession => false,
        Kind::SetWalkTargetFromBlockMemory { mem, speed, close, too_far, too_long } => {
            if !check(w, i, M::WalkTarget, Absent) || !has(w, i, mem) {
                return false;
            }
            walk_from_block_memory(w, i, mem, speed, close, too_far, too_long, t)
        }
        Kind::UpdateActivityFromSchedule => {
            update_activity_from_schedule(w, i);
            true
        }
        Kind::SetEntityLookTarget(f) => {
            if !check(w, i, M::LookTarget, Absent) || !has(w, i, M::NearestVisibleLiving) {
                return false;
            }
            set_entity_look_target(w, i, f)
        }
        Kind::InteractWith { kind: k, speed } => {
            if !check(w, i, M::WalkTarget, Absent) || !has(w, i, M::NearestVisibleLiving) {
                return false;
            }
            interact_with(w, i, k, speed)
        }
        Kind::VillageBoundRandomStroll { speed, xz, y } => {
            if !check(w, i, M::WalkTarget, Absent) {
                return false;
            }
            village_bound_stroll(w, i, speed, xz, y);
            true
        }
        Kind::SetWalkTargetFromLookTarget { speed, close } => {
            if !check(w, i, M::WalkTarget, Absent) || !has(w, i, M::LookTarget) {
                return false;
            }
            let lt = ex(w, i).mem.look_target.expect("a look target");
            ex_mut(w, i).mem.walk_target = Some(WalkTarget { target: lt, speed, close });
            true
        }
        Kind::SetLookAndInteract => {
            if !check(w, i, M::InteractionTarget, Absent) || !has(w, i, M::NearestVisibleLiving) {
                return false;
            }
            let me = w.entities.list[i].pos;
            let found = super::sense::find_closest(w, i, |w, id, _| id >= crate::entity::mob::ai::PLAYER_ID_BASE && crate::entity::mob::ai::target::who(w, id).is_some_and(|t| crate::entity::mob::ai::target::pos_of(w, t).distance_to_sqr(me) <= 16.0));
            if found.is_some() {
                refuse(w, "a villager interacting with a player is not modeled");
            }
            false
        }
        Kind::StrollAroundPoi { mem, .. } | Kind::StrollToPoi { mem, .. } => {
            if has(w, i, mem) {
                refuse(w, "a villager strolling around a job site or bell is not modeled");
            }
            false
        }
        Kind::StrollToPoiList { .. } => false,
        Kind::SetClosestHomeAsWalkTarget { last_update, mut batch } => {
            if !check(w, i, M::WalkTarget, Absent) || !check(w, i, M::Home, Absent) {
                return false;
            }
            if w.time - last_update < 20 {
                return false;
            }
            // poiManager.findClosest(HOME, pos, 48, ANY): none, or within 2 blocks: nothing to do
            let pos = w.entities.list[i].block_position();
            let d2 = |p: &BlockPos| {
                let (dx, dy, dz) = ((p.x - pos.x) as f64, (p.y - pos.y) as f64, (p.z - pos.z) as f64);
                dx * dx + dy * dy + dz * dz
            };
            let homes = crate::entity::mob::ai::poi::homes_any_ordered(w, pos, 48);
            let near = homes.iter().copied().min_by(|a, b| d2(a).total_cmp(&d2(b)));
            let Some(near) = near else { return false };
            if d2(&near) <= 4.0 {
                return false;
            }
            // lastUpdate = now + level random nextInt(20); findAllWithType's cache test (a position
            // cached until lastUpdate + 40 is skipped; at most 4 tried)
            let lu = w.time + level_rng(w).next_int(20) as i64;
            let mut tried = 0;
            let mut chosen = Vec::new();
            for p in homes {
                if batch.iter().any(|(q, _)| *q == p) {
                    continue;
                }
                tried += 1;
                if tried >= 5 {
                    continue;
                }
                batch.push((p, lu + 40));
                chosen.push(p);
            }
            // findPathToPois: one path to the set of positions (their HashSet's order), reach 1
            let path = if chosen.is_empty() {
                None
            } else {
                let hashes: Vec<i32> = chosen.iter().map(|p| (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x)).collect();
                let order = crate::entity::mob::ai::path::finder::java_hash_order(&hashes, crate::entity::mob::ai::path::finder::grown_table(chosen.len()));
                let targets: Vec<BlockPos> = order.iter().map(|&k| chosen[k]).collect();
                with_nav(w, i, |w, nm, nav, arena| nav.create_path_positions(w, nm, &targets, 1, arena))
            };
            match path {
                Some(p) if p.reached => {
                    if crate::entity::mob::ai::poi::poi_of(&w.get(p.target)).is_some() {
                        ex_mut(w, i).mem.walk_target = Some(WalkTarget { target: Tracker::Block(p.target), speed: 0.5, close: 1 });
                    }
                }
                _ => {
                    if tried < 5 {
                        batch.retain(|(_, until)| *until >= lu);
                    }
                }
            }
            if let Kind::SetClosestHomeAsWalkTarget { last_update, batch: b } = kind_mut(w, i, n) {
                *last_update = lu;
                *b = batch;
            }
            true
        }
        Kind::InsideBrownianWalk => {
            if !check(w, i, M::WalkTarget, Absent) {
                return false;
            }
            let pos = w.entities.list[i].block_position();
            match crate::weather::can_see_sky(w, pos) {
                Ok(true) => false,
                Ok(false) => {
                    // the 27 cells around (x fastest, then y, then z), `Collections.shuffle` with
                    // the world's one stream (the bridge seeds it per sandbox:
                    // `rng::collections_seed`), the first that does not see the sky and whose
                    // top face is full, while the body itself is free (`noCollision(body)`)
                    let mut poses = Vec::with_capacity(27);
                    for z in -1..=1 {
                        for y in -1..=1 {
                            for x in -1..=1 {
                                poses.push(pos.offset(x, y, z));
                            }
                        }
                    }
                    let r = crate::entity::mob::ai::collections_rng(w);
                    for k in (2..=poses.len()).rev() {
                        let j = r.next_int(k as i32) as usize;
                        poses.swap(k - 1, j);
                    }
                    let bb = w.entities.list[i].bb;
                    let body_free = crate::entity::collide::no_collision(w, i, bb);
                    for p in poses {
                        match crate::weather::can_see_sky(w, p) {
                            Ok(true) => continue,
                            Ok(false) => {}
                            Err(e) => {
                                refuse(w, &format!("InsideBrownianWalk: the sky at {p}: {e}"));
                                return false;
                            }
                        }
                        if !top_face_full(w, p) || !body_free {
                            continue;
                        }
                        ex_mut(w, i).mem.walk_target = Some(WalkTarget { target: Tracker::Block(p), speed: 0.5, close: 0 });
                        break;
                    }
                    true
                }
                Err(e) => {
                    refuse(w, &format!("InsideBrownianWalk: the sky at {pos}: {e}"));
                    false
                }
            }
        }
        Kind::GoToClosestVillage => {
            if !check(w, i, M::WalkTarget, Absent) {
                return false;
            }
            use crate::entity::mob::ai::poi;
            let pos = w.entities.list[i].block_position();
            if poi::is_village(w, pos) {
                return false;
            }
            let here = poi::sections_to_village(w, pos);
            let v = poi::village_sections(w);
            let mut target = None;
            for _ in 0..5 {
                let lp = crate::entity::mob::ai::goal::stroll::land_random_pos_by(w, i, 15, 7, |_, p| -(poi::sections_to_village_in(&v, poi::section(p)) as f64));
                if let Some(lp) = lp {
                    let d = poi::sections_to_village_in(&v, poi::section(super::containing(lp)));
                    if d < here {
                        target = Some(lp);
                        break;
                    }
                    if d == here {
                        target = Some(lp);
                    }
                }
            }
            if let Some(t) = target {
                ex_mut(w, i).mem.walk_target = Some(WalkTarget::at_point(t, 0.5, 4));
            }
            true
        }
        Kind::VillagerCalmDown => {
            let me = w.entities.list[i].pos;
            let by_entity_close = ex(w, i).mem.hurt_by_entity.and_then(|id| crate::entity::mob::ai::target::who(w, id)).is_some_and(|t| crate::entity::mob::ai::target::pos_of(w, t).distance_to_sqr(me) <= 36.0);
            let scared = has(w, i, M::HurtBy) || has(w, i, M::NearestHostile) || by_entity_close;
            if !scared {
                erase(w, i, M::HurtBy);
                erase(w, i, M::HurtByEntity);
                brain_mut(w, i).hurt_by = None;
                update_activity_from_schedule(w, i);
            }
            true
        }
        Kind::SetWalkTargetAwayFrom { mem, speed } => {
            if !has(w, i, mem) {
                return false;
            }
            walk_away_from(w, i, mem, speed)
        }
        Kind::PlayTagWithOtherKids | Kind::SocializeAtBell => false,
        Kind::RingBell | Kind::ResetRaidStatus | Kind::RaidSequence | Kind::LocateHidingPlace | Kind::SetHiddenState => {
            refuse(w, "a villager's raid or bell behaviour is not modeled");
            false
        }
        _ => false,
    }
}

/// `InteractWithDoor`: the doors (mob-interactable: wooden) on the path's previous and next nodes
/// opened (the previous one remembered even when already open, the next one when this opens it),
/// then the remembered doors it moved away from closed (`closeDoorsThatIHaveOpenedOrPassedThrough`).
/// Each door it opens or closes draws its sound's pitch from the level random.
fn interact_with_door(w: &mut World, i: usize, n: usize) -> bool {
    let Some(path) = ex(w, i).mem.path.clone() else { return false };
    let live = ai(w, i).nav.path.clone().filter(|p| p.uid == path.uid).unwrap_or(path);
    if live.not_started() || live.is_done() {
        return false;
    }
    let next = live.nodes[live.next_node_index].pos();
    if let Kind::InteractWithDoor { last_checked, remaining_cooldown } = kind_mut(w, i, n) {
        if *last_checked == Some(next) {
            *remaining_cooldown = 20;
        } else {
            *remaining_cooldown -= 1;
            if *remaining_cooldown > 0 {
                return false;
            }
        }
        *last_checked = Some(next);
    }
    let prev = live.nodes[live.next_node_index - 1].pos();
    let wooden = |b: &crate::block::Block| matches!(b, crate::block::Block::Door { iron: false, .. });
    let from = w.get(prev);
    if wooden(&from) {
        if let crate::block::Block::Door { open: false, .. } = from {
            set_door_open(w, i, prev, true);
        }
        remember_door(w, i, prev);
    }
    let to = w.get(next);
    if let crate::block::Block::Door { iron: false, open: false, .. } = to {
        set_door_open(w, i, next, true);
        remember_door(w, i, next);
    }
    if ex(w, i).mem.doors_to_close.is_some() {
        close_doors_passed(w, i, Some(prev), Some(next));
    }
    true
}

/// `rememberDoorToClose`: the set (made when absent) gains the door.
fn remember_door(w: &mut World, i: usize, p: BlockPos) {
    let d = ex_mut(w, i).mem.doors_to_close.get_or_insert_with(Vec::new);
    if !d.contains(&p) {
        d.push(p);
    }
}

/// `DoorBlock.setOpen(entity, level, state, pos, open)`: the door set with flags 10 (clients,
/// immediate; its other half follows by shape update), its sound's pitch from the level random,
/// the BLOCK_OPEN / BLOCK_CLOSE game event.
pub fn set_door_open(w: &mut World, i: usize, p: BlockPos, open: bool) {
    let crate::block::Block::Door { iron, facing, upper, hinge_right, open: was, powered } = w.get(p) else { return };
    if was == open {
        return;
    }
    w.set_block(p, crate::block::Block::Door { iron, facing, upper, hinge_right, open, powered }, crate::world::flags::CLIENTS | crate::world::flags::IMMEDIATE);
    // `playSound(entity, ..., level.getRandom().nextFloat() * 0.1F + 0.9F)`
    w.draw_sound_pitch();
    let _ = i;
    let at = [p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5];
    w.game_event_at(if open { "block_open" } else { "block_close" }, at, None, false);
}

/// `closeDoorsThatIHaveOpenedOrPassedThrough(level, body, from, to, doors, nearest)`: each
/// remembered door but the path's two nodes: forgotten when 3 or more from it, or no longer an
/// open wooden door, or another villager within 2 is coming through it; else closed. The set's
/// order (a HashSet) only matters with two doors to close at once: refused then.
fn close_doors_passed(w: &mut World, i: usize, from: Option<BlockPos>, to: Option<BlockPos>) {
    let doors = ex(w, i).mem.doors_to_close.clone().unwrap_or_default();
    let me = w.entities.list[i].pos;
    let closable: Vec<BlockPos> = doors
        .iter()
        .copied()
        .filter(|d| Some(*d) != from && Some(*d) != to)
        .filter(|d| villager::closer_to_center(*d, me, 3.0) && matches!(w.get(*d), crate::block::Block::Door { iron: false, open: true, .. }))
        .collect();
    if closable.len() > 1 {
        refuse(w, "a villager closing two doors at once (the HashSet's order) is not modeled");
    }
    let mut keep = Vec::new();
    for d in doors {
        if Some(d) == from || Some(d) == to {
            keep.push(d);
            continue;
        }
        if !villager::closer_to_center(d, me, 3.0) {
            continue;
        }
        if !matches!(w.get(d), crate::block::Block::Door { iron: false, open: true, .. }) {
            continue;
        }
        if other_villager_coming_through(w, i, d) {
            continue;
        }
        set_door_open(w, i, d, false);
    }
    ex_mut(w, i).mem.doors_to_close = Some(keep);
}

/// `areOtherMobsComingThroughDoor`: a villager among NEAREST_LIVING_ENTITIES within 2 of the
/// door's centre whose path's previous or next node is the door.
fn other_villager_coming_through(w: &World, i: usize, d: BlockPos) -> bool {
    let near: Vec<u32> = brain(w, i).nearest_living.iter().map(|(id, _)| *id).collect();
    near.iter().any(|id| {
        let Some(j) = crate::entity::index_of(w, *id) else { return false };
        let crate::entity::EntityKind::Mob(m) = &w.entities.list[j].kind else { return false };
        if m.ty != MobType::Villager || !villager::closer_to_center(d, w.entities.list[j].pos, 2.0) {
            return false;
        }
        let Some(x) = m.brain.as_deref().and_then(|b| b.ex.as_deref()) else { return false };
        let Some(pa) = x.mem.path.as_ref() else { return false };
        let live = m.ai.as_deref().and_then(|a| a.nav.path.as_ref()).filter(|n| n.uid == pa.uid).unwrap_or(pa);
        if live.is_done() || live.next_node_index == 0 {
            return false;
        }
        live.nodes[live.next_node_index - 1].pos() == d || live.nodes[live.next_node_index].pos() == d
    })
}

/// `ValidateNearbyPoi`: within 16 of the POI, a POI no longer there is forgotten (its ticket left
/// with its record); a bed OCCUPIED while this villager is awake is forgotten too, its ticket given
/// back unless a villager sleeps in that bed's cell (`bedIsOccupiedByVillager`).
fn validate_nearby_poi(w: &mut World, i: usize, poi: PoiSel, mem: M) -> bool {
    let pos = match mem {
        M::Home => brain(w, i).home,
        M::JobSite => ex(w, i).mem.job_site,
        M::PotentialJobSite => ex(w, i).mem.potential_job_site,
        M::MeetingPoint => ex(w, i).mem.meeting_point,
        _ => None,
    };
    let Some(p) = pos else { return false };
    let me = w.entities.list[i].pos;
    if !villager::closer_to_center(p, me, 16.0) {
        return false;
    }
    let exists = match poi {
        PoiSel::Home => matches!(w.get(p), crate::block::Block::Bed { head: true, .. }),
        _ => {
            refuse(w, "a villager's job site or bell memory is not modeled");
            true
        }
    };
    if !exists {
        erase(w, i, mem);
    } else if poi == PoiSel::Home && matches!(w.get(p), crate::block::Block::Bed { occupied: true, .. }) && mob(w, i).sleeping_pos.is_none() {
        erase(w, i, mem);
        let cell = crate::entity::Aabb::of_block(p);
        let sleeper = w.get_entities(cell, |e| matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.ty == crate::entity::mob::types::MobType::Villager && m.sleeping_pos.is_some()));
        if sleeper.is_empty() {
            crate::entity::mob::ai::poi::release(w, p);
        }
    }
    true
}

/// `AcquirePoi`: the memory to validate and the one to acquire absent, an adult when it must be,
/// then every 20..39 ticks (the first schedule drawn on the first try) a search of the 5 closest
/// POIs with a free ticket within 48 and a path to them.
fn acquire_poi(w: &mut World, i: usize, n: usize, t: i64) -> bool {
    let (poi, validate, acquire, only_adult, next) = match kind(w, i, n) {
        Kind::AcquirePoi { poi, validate, acquire, only_adult, next_scheduled, .. } => (*poi, *validate, *acquire, *only_adult, *next_scheduled),
        _ => unreachable!(),
    };
    if validate != acquire && has(w, i, validate) {
        return false;
    }
    if has(w, i, acquire) {
        return false;
    }
    if only_adult && ex(w, i).baby {
        return false;
    }
    if next == 0 {
        let r = level_rng(w).next_int(20) as i64;
        set_next(w, i, n, w.time + r);
        return false;
    }
    if w.time < next {
        return false;
    }
    let r = level_rng(w).next_int(20) as i64;
    set_next(w, i, n, t + 20 + r);
    if let Kind::AcquirePoi { batch, .. } = kind_mut(w, i, n) {
        batch.retain(|(_, m)| t - m.previous < 400);
    }
    // findAllClosestFirstWithType: every free POI in range runs the retry cache's test in the
    // manager's order (a due marker retries: markAttempt draws), then closest first, 5 of them,
    // then the behaviour's own test (a home: a bed nobody sleeps in)
    let me = w.entities.list[i].block_position();
    let mut passed = Vec::new();
    for p in crate::entity::mob::ai::poi::free_pois_ordered(w, poi, me, 48) {
        let marker = match kind(w, i, n) {
            Kind::AcquirePoi { batch, .. } => batch.iter().position(|(q, _)| *q == p),
            _ => None,
        };
        match marker {
            None => passed.push(p),
            Some(k) => {
                let due = matches!(kind(w, i, n), Kind::AcquirePoi { batch, .. } if t >= batch[k].1.next);
                if due {
                    let add = level_rng(w).next_int(40);
                    if let Kind::AcquirePoi { batch, .. } = kind_mut(w, i, n) {
                        mark_attempt(&mut batch[k].1, t, add);
                    }
                    passed.push(p);
                }
            }
        }
    }
    let d2 = |p: &BlockPos| {
        let (dx, dy, dz) = ((p.x - me.x) as f64, (p.y - me.y) as f64, (p.z - me.z) as f64);
        dx * dx + dy * dy + dz * dz
    };
    passed.sort_by(|a, b| d2(a).total_cmp(&d2(b)));
    passed.truncate(5);
    let valid: Vec<BlockPos> = passed
        .into_iter()
        .filter(|p| match poi {
            // a bed (a design's or a natural state's) nobody sleeps in
            PoiSel::Home => match w.get(*p) {
                crate::block::Block::Bed { occupied, .. } => !occupied,
                crate::block::Block::Terrain(id) => {
                    let props = crate::terrain::blocks::props(id);
                    props.block_id().ends_with("_bed") && props.name.contains("occupied=false")
                }
                _ => false,
            },
            _ => true,
        })
        .collect();
    if valid.is_empty() {
        return true;
    }
    // findPathToPois: one path to the set of positions (a HashSet<BlockPos>), reach = the largest
    // validRange
    let hashes: Vec<i32> = valid.iter().map(|p| (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x)).collect();
    let order = crate::entity::mob::ai::path::finder::java_hash_order(&hashes, crate::entity::mob::ai::path::finder::grown_table(valid.len()));
    let targets: Vec<BlockPos> = order.iter().map(|&k| valid[k]).collect();
    let reach = crate::entity::mob::ai::poi::valid_range(poi).max(1);
    let path = with_nav(w, i, |w, nm, nav, arena| nav.create_path_positions(w, nm, &targets, reach, arena));
    match path {
        Some(p) if p.reached && poi != PoiSel::Home => {
            // a job site or a meeting point taken: the profession and the meeting point's
            // behaviours are not modeled
            let _ = p;
            refuse(w, "a villager acquiring a job site or a meeting point is not modeled yet");
        }
        Some(p) if p.reached => {
            let target = p.target;
            // poiManager.getType(target).ifPresent(..): only a target that is a POI (createPath
            // can hand back the current path, whose target is another, when its targetPos is in
            // the set: ironfarm 2026-11-10, a napper's reused stroll path to a rail cell); then
            // poiManager.take (the record at the target, if this selection's and free), the home
            // memory (set even when the take found nothing), the event 14 (client particles), the
            // retry cache cleared
            if crate::entity::mob::ai::poi::poi_of(&w.get(target)).is_some() {
                crate::entity::mob::ai::poi::take_if_free(w, poi, target);
                brain_mut(w, i).home = Some(target);
                if let Kind::AcquirePoi { batch, .. } = kind_mut(w, i, n) {
                    batch.clear();
                }
            }
        }
        _ => {
            // new retry markers, walking poiPositions (a HashSet<Pair<Holder<PoiType>,
            // BlockPos>> filled closest first) in its order: a Pair's hash is
            // 31 * (31 + the holder's identity hash) + the position's (the game JVM's holder
            // hashes: the bridge's `identity_hashes`; a stand-in per type in the judge);
            // computeIfAbsent draws for each position the cache lacks
            let hashes: Vec<i32> = valid
                .iter()
                .map(|p| {
                    let ty = crate::entity::mob::ai::poi::poi_type_name(&w.get(*p)).unwrap_or("minecraft:home");
                    let h = crate::entity::mob::ai::poi_holder_hash(w, ty);
                    let ph = (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x);
                    31i32.wrapping_mul(31i32.wrapping_add(h)).wrapping_add(ph)
                })
                .collect();
            let order = crate::entity::mob::ai::path::finder::java_hash_order(&hashes, crate::entity::mob::ai::path::finder::grown_table(valid.len()));
            for q in order.into_iter().map(|k| valid[k]) {
                if matches!(kind(w, i, n), Kind::AcquirePoi { batch, .. } if batch.iter().any(|(b, _)| *b == q)) {
                    continue;
                }
                let add = level_rng(w).next_int(40);
                let mut m = Retry { previous: 0, next: 0, delay: 0 };
                mark_attempt(&mut m, t, add);
                if let Kind::AcquirePoi { batch, .. } = kind_mut(w, i, n) {
                    batch.push((q, m));
                }
            }
        }
    }
    true
}

/// `JitteredLinearRetry.markAttempt(timestamp)`: the delay grows by 40..79 (`add` = the drawn
/// nextInt(40)) up to 400.
fn mark_attempt(m: &mut Retry, t: i64, add: i32) {
    m.previous = t;
    m.delay = (m.delay + add + 40).min(400);
    m.next = t + m.delay as i64;
}

fn set_next(w: &mut World, i: usize, n: usize, v: i64) {
    if let Kind::AcquirePoi { next_scheduled, .. } = kind_mut(w, i, n) {
        *next_scheduled = v;
    }
}

/// `SetWalkTargetFromBlockMemory`.
#[allow(clippy::too_many_arguments)]
fn walk_from_block_memory(w: &mut World, i: usize, mem: M, speed: f32, close: i32, too_far: i32, too_long: i64, t: i64) -> bool {
    let target = match mem {
        M::Home => brain(w, i).home,
        M::JobSite => ex(w, i).mem.job_site,
        M::MeetingPoint => ex(w, i).mem.meeting_point,
        _ => None,
    }
    .expect("a block memory");
    let cant = ex(w, i).mem.cant_reach_since;
    let me = w.entities.list[i].block_position();
    let dist = |a: BlockPos, b: BlockPos| (a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs();
    if cant.is_none_or(|c| w.time - c <= too_long) {
        if dist(target, me) > too_far {
            let towards = Vec3::new(target.x as f64 + 0.5, target.y as f64, target.z as f64 + 0.5);
            let mut tries = 0;
            loop {
                let p = stroll::default_pos_towards(w, i, 15, 7, towards, (std::f64::consts::PI / 2.0) as f32 as f64);
                let me = w.entities.list[i].block_position();
                if let Some(p) = p
                    && dist(super::containing(p), me) <= too_far
                {
                    ex_mut(w, i).mem.walk_target = Some(WalkTarget::at_point(p, speed, close));
                    break;
                }
                tries += 1;
                if tries == 1000 {
                    // releasePoi (the record's ticket back), then the memory erased
                    let id = w.entities.list[i].id;
                    w.mob.events.push(crate::entity::mob::MobEvent::Released { t, id });
                    release_poi(w, i, mem);
                    erase(w, i, mem);
                    ex_mut(w, i).mem.cant_reach_since = Some(t);
                    return true;
                }
            }
        } else if dist(target, me) > close {
            ex_mut(w, i).mem.walk_target = Some(WalkTarget { target: Tracker::Block(target), speed, close });
        }
    } else {
        // too long unreachable: releasePoi (the record's ticket back; no passenger check), then the
        // memory erased
        let id = w.entities.list[i].id;
        w.mob.events.push(crate::entity::mob::MobEvent::Released { t, id });
        release_poi(w, i, mem);
        erase(w, i, mem);
        ex_mut(w, i).mem.cant_reach_since = Some(t);
    }
    true
}

/// `Villager.releasePoi(memory)`: the memory's POI, still there and of the memory's kind, gives a
/// ticket back (`PoiManager.release`: `PoiRecord.releaseTicket`, at most its max; a record shared
/// by two claims after a replace gives back once per release). The memory itself stays.
fn release_poi(w: &mut World, i: usize, mem: M) {
    let pos = match mem {
        M::Home => brain(w, i).home,
        M::JobSite => ex(w, i).mem.job_site,
        M::PotentialJobSite => ex(w, i).mem.potential_job_site,
        M::MeetingPoint => ex(w, i).mem.meeting_point,
        _ => None,
    };
    let Some(p) = pos else { return };
    let kind_ok = match (mem, crate::entity::mob::ai::poi::poi_of(&w.get(p))) {
        (M::Home, Some(crate::entity::mob::ai::poi::PoiKind::Home)) => true,
        (M::MeetingPoint, Some(crate::entity::mob::ai::poi::PoiKind::Meeting)) => true,
        (M::JobSite | M::PotentialJobSite, Some(crate::entity::mob::ai::poi::PoiKind::Job)) => true,
        _ => false,
    };
    if kind_ok {
        crate::entity::mob::ai::poi::release(w, p);
    }
}

/// `SetEntityLookTarget`: the closest visible entity of the filter within 8 (not its passenger).
fn set_entity_look_target(w: &mut World, i: usize, f: LookFilter) -> bool {
    let me = w.entities.list[i].pos;
    let my_id = w.entities.list[i].id;
    let found = super::sense::find_closest(w, i, |w, id, ty| {
        let Some(t) = crate::entity::mob::ai::target::who(w, id) else { return false };
        let ty = if id >= crate::entity::mob::ai::PLAYER_ID_BASE { None } else { Some(ty) };
        super::tree::look_accepts(f, ty)
            && crate::entity::mob::ai::target::pos_of(w, t).distance_to_sqr(me) <= 64.0
            && !w.entities.list.iter().any(|e| e.id == id && e.vehicle == Some(my_id))
    });
    match found {
        Some(id) => {
            ex_mut(w, i).mem.look_target = Some(Tracker::Entity { id, eye: true });
            true
        }
        None => false,
    }
}

/// `InteractWith.of(type, 8, ...)`: the closest visible valid target within 8 becomes the
/// interaction (or breeding) target, the look target and the walk target.
fn interact_with(w: &mut World, i: usize, k: InteractKind, speed: f32) -> bool {
    if k == InteractKind::BreedVillager {
        // selfFilter AgeableMob::canBreed: food to share (no villager carries food here)
        return false;
    }
    let me = w.entities.list[i].pos;
    let valid = |ty: MobType| match k {
        InteractKind::Villager => ty == MobType::Villager,
        InteractKind::Cat => ty.def().id == "cat",
        InteractKind::BreedVillager => false,
    };
    // entities.contains(isTargetValid): the first valid one that passes the sight test
    let any = super::sense::find_closest(w, i, |_, id, ty| id < crate::entity::mob::ai::PLAYER_ID_BASE && valid(ty));
    if any.is_none() {
        return false;
    }
    let closest = super::sense::find_closest(w, i, |w, id, ty| id < crate::entity::mob::ai::PLAYER_ID_BASE && crate::entity::index_of(w, id).is_some_and(|j| w.entities.list[j].pos.distance_to_sqr(me) <= 64.0) && valid(ty));
    if let Some(id) = closest {
        let x = ex_mut(w, i);
        x.mem.interaction_target = Some(id);
        x.mem.look_target = Some(Tracker::Entity { id, eye: true });
        x.mem.walk_target = Some(WalkTarget { target: Tracker::Entity { id, eye: false }, speed, close: 2 });
    }
    true
}

/// `VillageBoundRandomStroll`: in a village, a land position around; else toward the section
/// closest to the village (within 2 sections).
fn village_bound_stroll(w: &mut World, i: usize, speed: f32, xz: i32, y: i32) {
    let me = w.entities.list[i].block_position();
    let p = if crate::entity::mob::ai::poi::is_village(w, me) {
        stroll::land_random_pos(w, i, xz, y)
    } else {
        let s = (me.x.div_euclid(16), me.y.div_euclid(16), me.z.div_euclid(16));
        let best = crate::entity::mob::ai::poi::section_closest_to_village(w, s, 2);
        if best != s {
            let c = Vec3::new((best.0 * 16 + 8) as f64 + 0.5, (best.1 * 16 + 8) as f64, (best.2 * 16 + 8) as f64 + 0.5);
            stroll::default_pos_towards(w, i, xz, y, c, (std::f64::consts::PI / 2.0) as f32 as f64)
        } else {
            stroll::land_random_pos(w, i, xz, y)
        }
    };
    ex_mut(w, i).mem.walk_target = p.map(|p| WalkTarget::at_point(p, speed, 0));
}

/// `SetWalkTargetAwayFrom.entity(memory, speed, 6, false)`.
fn walk_away_from(w: &mut World, i: usize, mem: M, speed: f32) -> bool {
    let target = ex(w, i).mem.walk_target;
    if target.is_some() {
        return false;
    }
    let id = match mem {
        M::NearestHostile => brain(w, i).nearest_hostile,
        M::HurtByEntity => ex(w, i).mem.hurt_by_entity,
        _ => None,
    };
    let Some(avoid) = id.and_then(|id| crate::entity::mob::ai::target::who(w, id)).map(|t| crate::entity::mob::ai::target::pos_of(w, t)) else { return false };
    let me = w.entities.list[i].pos;
    if !(me.distance_to_sqr(avoid) < 36.0) {
        return false;
    }
    for _ in 0..10 {
        if let Some(p) = stroll::land_pos_away(w, i, 0.0, 16.0, 7, avoid) {
            ex_mut(w, i).mem.walk_target = Some(WalkTarget::at_point(p, speed, 0));
            break;
        }
    }
    true
}

/// `Level.loadedAndEntityCanStandOn(pos, entity)`: the block's collision shape covers its whole
/// top face (`Block.isFaceFull(shape, UP)`: one box reaching the top across the cell).
fn top_face_full(w: &World, p: BlockPos) -> bool {
    let s = w.get(p);
    crate::entity::collide::boxes_at(w, &s, p).iter().any(|b| b.min_x <= p.x as f64 && b.max_x >= (p.x + 1) as f64 && b.min_z <= p.z as f64 && b.max_z >= (p.z + 1) as f64 && b.max_y >= (p.y + 1) as f64)
}
