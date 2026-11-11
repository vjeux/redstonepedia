//! The wandering trader and its trader llamas (mob-AI 2026-11-11). A 5-day judge run meets one in
//! about a third of its builds (`mctas mobai trader-tries`).
//!
//! * `WanderingTraderSpawner.spawn` (custom.rs): the meeting point within 48 of the player (else
//!   the player's block), ten tries at a cell on the MOTION_BLOCKING_NO_LEAVES heightmap within 48
//!   (the spawner's own random), room for it, then [`spawn_trader`] and twice [`spawn_llama`]
//!   within 4 of it, leashed to it; the trader's despawn delay 48000, its wander target and home
//!   (16) the reference.
//! * `WanderingTrader.registerGoals`: Float 0, UseItem 0 (an invisibility potion when it is dark
//!   and it is not invisible; milk when it is bright and it is), AvoidEntity 1 (zombies within 8:
//!   the illagers, the vex and the zoglin are not modeled), Panic 1 (0.5), WanderToPosition 2
//!   (2.0, 0.35), MoveTowardsRestriction 4 (0.35), WaterAvoidingRandomStroll 8 (0.35), Interact 9
//!   (a player within 3, probability 1), LookAtPlayer 10 (any mob within 8). TradeWithPlayer and
//!   LookAtTradingPlayer need a trading player: never. `aiStep`'s despawn delay.
//! * `TraderLlama`: Llama's goals (Float 0, RunAroundLikeCrazy 1, LlamaFollowCaravan 2: never
//!   while leashed, RangedAttack 3 (its spit is not modeled: a named error when it would start),
//!   Panic 3 (1.2), Breed 4, Tempt 5, FollowParent 6, WaterAvoidingRandomStroll 7 (0.7),
//!   LookAtPlayer 8 (6), RandomLookAround 9) and Panic 1 (2.0); the targets LlamaHurtBy 1,
//!   LlamaAttackWolf 2, DefendWanderingTrader 1, zombies 2 (not zombified piglins; seen), illagers
//!   2 (none modeled). `aiStep`'s despawn delay (its trader's less one while leashed to it).

use super::super::target;
use super::super::{ai, ai_mut, has_ai, with_nav};
use super::{Goal, MOVE};
use crate::entity::Vec3;
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

/// What the main hand holds (`UseItemGoal` puts its item there; a drink leaves its container).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Hand {
    #[default]
    Empty,
    Potion,
    Milk,
    Bottle,
    Bucket,
}

/// A wandering trader's or a trader llama's own state.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TraderState {
    /// A trader llama (else the trader).
    pub llama: bool,
    /// `WanderingTrader.wanderTarget`.
    pub wander_target: Option<BlockPos>,
    /// `despawnDelay`.
    pub despawn_delay: i32,
    /// The main hand.
    pub hand: Hand,
    /// `useItem` (a milk bucket: true) and `useItemRemaining`, while using it.
    pub using: Option<(bool, i32)>,
    /// A llama's `Llama.finalizeSpawn` strength (1..5) and variant (0..3).
    pub strength: i32,
    pub variant: i32,
    /// `Llama.didSpit`.
    pub did_spit: bool,
    /// (mob-AI 2026-11-11) A llama's `Llama.caravanHead` (the llama it follows) and
    /// `caravanTail` (the one following it), by id. A tail that left the world without its goal
    /// stopping stays (the game's reference does too), so its head keeps `hasCaravanTail`.
    pub caravan_head: Option<u32>,
    pub caravan_tail: Option<u32>,
    /// The INVISIBLE shared flag (`isInvisible()`): `updateInvisibilityStatus` sets it from the
    /// invisibility effect at the first `tickEffects` after the effects changed (`effectsDirty`),
    /// so a drink's effect shows a tick late (the trader drinks a second potion at dusk, a second
    /// milk at dawn).
    pub invisible: bool,
    pub effects_dirty: bool,
}

/// The trader's and the llamas' own goals.
#[derive(Clone, PartialEq, Debug)]
pub enum TraderGoal {
    /// `UseItemGoal`: the invisibility potion (false) or the milk bucket (true).
    UseItem { milk: bool },
    /// `WanderingTrader.WanderToPositionGoal(2.0, 0.35)`.
    WanderToPosition,
    /// `MoveTowardsRestrictionGoal(speed)`: the spot `DefaultRandomPos.getPosTowards` found.
    MoveTowardsRestriction { speed: f64, wanted: Vec3 },
    /// `LlamaFollowCaravanGoal`: its `speedModifier` (2.1F at first, the double 2.1 after a stop,
    /// x1.2 while the head is over 26 away, up to past 3) and `distCheckCounter`.
    LlamaFollowCaravan { speed: f64, dist_check: i32 },
    /// `RangedAttackGoal(1.25, 40, 20)`: the llama's spit (`Llama.performRangedAttack`).
    LlamaSpit(super::drowned::TridentAttack),
    /// `TraderLlamaDefendWanderingTraderGoal`: the trader's last attacker (its timestamp).
    DefendTrader { timestamp: i32, attacker: Option<u32>, base: super::targets::TargetBase },
}

impl TraderGoal {
    pub fn name(&self) -> &'static str {
        match self {
            TraderGoal::UseItem { .. } => "UseItemGoal",
            TraderGoal::WanderToPosition => "WanderToPositionGoal",
            TraderGoal::MoveTowardsRestriction { .. } => "MoveTowardsRestrictionGoal",
            TraderGoal::LlamaFollowCaravan { .. } => "LlamaFollowCaravanGoal",
            TraderGoal::LlamaSpit(_) => "RangedAttackGoal",
            TraderGoal::DefendTrader { .. } => "TraderLlamaDefendWanderingTraderGoal",
        }
    }

    pub fn flags(&self) -> u8 {
        match self {
            TraderGoal::UseItem { .. } => 0,
            TraderGoal::WanderToPosition | TraderGoal::MoveTowardsRestriction { .. } | TraderGoal::LlamaFollowCaravan { .. } => MOVE,
            TraderGoal::LlamaSpit(_) => MOVE | super::LOOK,
            TraderGoal::DefendTrader { .. } => super::TARGET,
        }
    }

    /// `requiresUpdateEveryTick()`: the ranged attack's.
    pub fn every_tick(&self) -> bool {
        matches!(self, TraderGoal::LlamaSpit(_))
    }
}

pub fn is_trader_id(id: &str) -> bool {
    id == "wandering_trader"
}

pub fn is_llama_id(id: &str) -> bool {
    id == "trader_llama"
}

fn st(w: &World, i: usize) -> &TraderState {
    ai(w, i).trader.as_ref().expect("a trader or a trader llama")
}

fn st_mut(w: &mut World, i: usize) -> &mut TraderState {
    ai_mut(w, i).trader.as_mut().expect("a trader or a trader llama")
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

fn refuse(w: &mut World, why: &str) {
    let why = why.to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

/// `WanderingTrader.registerGoals` (and AbstractVillager's FIRE_IN_NEIGHBOR 16, FIRE -1).
pub fn register_trader(a: &mut super::super::MobAi) {
    use super::animal::AnimalGoal;
    use super::avoid::{AvoidClass, AvoidEntity};
    use super::look::{LookAtPlayer, LookType};
    use super::stroll::RandomStroll;
    let g = &mut a.goals;
    g.add(0, Goal::Float);
    g.add(0, Goal::Trader(TraderGoal::UseItem { milk: false }));
    g.add(0, Goal::Trader(TraderGoal::UseItem { milk: true }));
    g.add(1, Goal::AvoidEntity(AvoidEntity::new(AvoidClass::Zombie, 8.0, 0.5, 0.5)));
    g.add(1, Goal::Animal(AnimalGoal::Panic { speed: 0.5, pos: Vec3::ZERO, mount: false, env: false }));
    g.add(2, Goal::Trader(TraderGoal::WanderToPosition));
    g.add(4, Goal::Trader(TraderGoal::MoveTowardsRestriction { speed: 0.35, wanted: Vec3::ZERO }));
    g.add(8, Goal::RandomStroll(RandomStroll::water_avoiding(0.35)));
    g.add(9, Goal::LookAtPlayer(LookAtPlayer { probability: 1.0, interact: true, ..LookAtPlayer::new(LookType::Player, 3.0) }));
    g.add(10, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Mob, 8.0)));
    use crate::entity::mob::ai::path::path_type::PathType;
    a.malus.set(PathType::FireInNeighbor, 16.0);
    a.malus.set(PathType::Fire, -1.0);
    a.trader = Some(TraderState::default());
}

/// `Llama.registerGoals`, then `TraderLlama`'s additions (the goal lists keep insertion order).
pub fn register_llama(a: &mut super::super::MobAi) {
    use super::animal::{AnimalGoal, Horse};
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::stroll::RandomStroll;
    use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
    let an = Goal::Animal;
    let g = &mut a.goals;
    g.add(0, Goal::Float);
    g.add(1, an(AnimalGoal::RunAround));
    g.add(2, Goal::Trader(TraderGoal::LlamaFollowCaravan { speed: 2.1f32 as f64, dist_check: 0 }));
    g.add(3, Goal::Trader(TraderGoal::LlamaSpit(super::drowned::TridentAttack::default())));
    g.add(3, an(AnimalGoal::Panic { speed: 1.2, pos: Vec3::ZERO, mount: false, env: false }));
    g.add(4, an(AnimalGoal::Breed));
    g.add(5, an(AnimalGoal::Tempt));
    g.add(6, an(AnimalGoal::FollowParent { speed: 1.0, parent: None, recalc: 0 }));
    g.add(7, Goal::RandomStroll(RandomStroll::water_avoiding(0.7)));
    g.add(8, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
    g.add(9, Goal::RandomLookAround(RandomLookAround::default()));
    g.add(1, an(AnimalGoal::Panic { speed: 2.0, pos: Vec3::ZERO, mount: false, env: false }));
    let t = &mut a.targets;
    t.add(1, Goal::HurtByTarget(HurtByTarget { llama: true, ..HurtByTarget::new(false) }));
    t.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::LlamaWolf, 16, false, true)));
    t.add(1, Goal::Trader(TraderGoal::DefendTrader { timestamp: 0, attacker: None, base: super::targets::TargetBase::new(false, false) }));
    t.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::ZombieNotPiglin, 10, true, false)));
    // AbstractIllager (none modeled: its interval roll alone)
    t.add(2, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Illager, 10, true, false)));
    // createBaseChestedHorseAttributes: MAX_HEALTH 53 (until randomizeAttributes), MOVEMENT_SPEED
    // 0.175F, JUMP_STRENGTH 0.5
    a.horse = Some(Horse { speed: 0.175f32 as f64, jump: 0.5, max_health: 53.0, ..Horse::default() });
    a.trader = Some(TraderState { llama: true, despawn_delay: 47999, ..TraderState::default() });
}

/// Is entity `i` a wandering trader with its AI?
pub fn is_trader(w: &World, i: usize) -> bool {
    has_ai(w, i) && ai(w, i).trader.as_ref().is_some_and(|s| !s.llama)
}

/// Is entity `i` a trader llama with its AI?
pub fn is_llama(w: &World, i: usize) -> bool {
    has_ai(w, i) && ai(w, i).trader.as_ref().is_some_and(|s| s.llama)
}

/// `Entity.isInvisible()` of a trader: its INVISIBLE flag.
pub fn is_invisible(w: &World, i: usize) -> bool {
    has_ai(w, i) && ai(w, i).trader.as_ref().is_some_and(|s| s.invisible)
}

/// `LivingEntity.tickEffects`' tail for a trader: an invisibility that ran out this tick marks the
/// effects dirty (the flag follows at the next tick's sync, [`sync_flags`]).
pub fn after_tick_effects(w: &mut World, i: usize, invisibility_before: i32) {
    if !has_ai(w, i) || ai(w, i).trader.is_none() {
        return;
    }
    let now = mob(w, i).effects.invisibility;
    if invisibility_before > 0 && now == 0 {
        st_mut(w, i).effects_dirty = true;
    }
}

/// `ChunkMap.tick()`'s `sendChanges` → `updateDataBeforeSync` → `updateDirtyEffects` (after the
/// custom spawners, before any entity ticks): a trader whose effects changed takes its INVISIBLE
/// flag from its invisibility effect.
pub fn sync_flags(w: &mut World) {
    for i in 0..w.entities.list.len() {
        if w.entities.list[i].removed || !has_ai(w, i) {
            continue;
        }
        let Some(dirty) = ai(w, i).trader.as_ref().map(|s| s.effects_dirty) else { continue };
        if dirty {
            let now = mob(w, i).effects.invisibility > 0;
            let s = st_mut(w, i);
            s.invisible = now;
            s.effects_dirty = false;
        }
    }
}

/// `LivingEntity.isUsingItem()`.
pub fn is_using_item(w: &World, i: usize) -> bool {
    has_ai(w, i) && ai(w, i).trader.as_ref().is_some_and(|s| s.using.is_some())
}

pub fn can_use(w: &mut World, i: usize, g: &mut TraderGoal) -> bool {
    match g {
        TraderGoal::UseItem { milk } => {
            let bright = super::zombie::is_bright_outside(w);
            let dark = !w.ultrawarm && !bright;
            if *milk { bright && is_invisible(w, i) } else { dark && !is_invisible(w, i) }
        }
        TraderGoal::WanderToPosition => st(w, i).wander_target.is_some_and(|p| too_far(w, i, p, 2.0)),
        TraderGoal::MoveTowardsRestriction { wanted, .. } => {
            let bp = w.entities.list[i].block_position();
            if super::stroll::within_home(w, i, bp) {
                return false;
            }
            let Some((home, _)) = super::stroll::home(w, i) else { return false };
            let towards = Vec3::new(home.x as f64 + 0.5, home.y as f64, home.z as f64 + 0.5);
            match super::stroll::default_pos_towards(w, i, 16, 7, towards, (std::f64::consts::PI / 2.0) as f32 as f64) {
                Some(p) => {
                    *wanted = p;
                    true
                }
                None => false,
            }
        }
        TraderGoal::LlamaFollowCaravan { .. } => caravan_can_use(w, i),
        TraderGoal::LlamaSpit(r) => spit_can_use(w, i, r),
        TraderGoal::DefendTrader { timestamp, attacker, base } => {
            let Some(holder) = w.entities.list[i].leash.map(|l| l.holder) else { return false };
            let Some(h) = crate::entity::index_of(w, holder) else { return false };
            if !is_trader(w, h) {
                return false;
            }
            let (by, ts) = (ai(w, h).last_hurt_by_mob, ai(w, h).last_hurt_by_mob_timestamp);
            *attacker = by;
            if ts == *timestamp {
                return false;
            }
            let Some(t) = by.and_then(|id| target::who(w, id)) else { return false };
            // TargetingConditions.DEFAULT (forCombat)
            let cond = target::Conditions::combat();
            super::targets::can_attack(w, i, base, t, &cond)
        }
    }
}

pub fn can_continue(w: &mut World, i: usize, g: &mut TraderGoal) -> bool {
    match g {
        TraderGoal::UseItem { .. } => is_using_item(w, i),
        TraderGoal::WanderToPosition => can_use(w, i, g),
        TraderGoal::MoveTowardsRestriction { .. } => !ai(w, i).nav.is_done(),
        TraderGoal::LlamaFollowCaravan { speed, dist_check } => caravan_can_continue(w, i, speed, dist_check),
        TraderGoal::LlamaSpit(r) => spit_can_use(w, i, r) || (r.target.and_then(|t| target::who(w, t)).is_some_and(|t| target::is_alive(w, t)) && !ai(w, i).nav.is_done()),
        TraderGoal::DefendTrader { base, .. } => super::targets::target_can_continue(w, i, base),
    }
}

/// `RangedAttackGoal.canUse()`: a live target, which it keeps.
fn spit_can_use(w: &mut World, i: usize, r: &mut super::drowned::TridentAttack) -> bool {
    let Some(t) = target::get_target(w, i) else { return false };
    if !target::who(w, t).is_some_and(|who| target::is_alive(w, who)) {
        return false;
    }
    r.target = Some(t);
    true
}

/// `RangedAttackGoal.tick()` (1.25, 40, 20): the sight time; within 20 and seen for 5 ticks the
/// navigation stops, else it walks to the target (1.25); it looks at the target (30, 30); when
/// the attack time runs out with the target in sight it spits ([`crate::entity::arrow::spit_from_llama`])
/// and waits `floor(dist / 20 x 0 + 40)`; a first wait of `floor(lerp(dist / 20, 40, 40))`.
fn spit_tick(w: &mut World, i: usize, g: &mut super::drowned::TridentAttack) {
    const RADIUS: f32 = 20.0;
    const INTERVAL: i32 = 40;
    let Some(t) = g.target.and_then(|id| target::who(w, id)) else { return };
    let tp = target::pos_of(w, t);
    let target_dist_sqr = w.entities.list[i].pos.distance_to_sqr(tp);
    let has_los = target::has_line_of_sight(w, i, t);
    if has_los {
        g.see_time += 1;
    } else {
        g.see_time = 0;
    }
    if !(target_dist_sqr > (RADIUS * RADIUS) as f64) && g.see_time >= 5 {
        with_nav(w, i, |_, _, nav, _| nav.stop());
    } else {
        let tb = super::melee::target_block(w, t);
        with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, tb, 1.25, arena));
    }
    let te = target::eye_of(w, t);
    ai_mut(w, i).look.set_look_at(te.x, te.y, te.z, 30.0, 30.0);
    g.attack_time -= 1;
    if g.attack_time == 0 {
        if !has_los {
            return;
        }
        let dist = (target_dist_sqr.sqrt() as f32) / RADIUS;
        let target::Who::Entity(j) = t else {
            refuse(w, "a trader llama spitting at a player is not modeled");
            return;
        };
        crate::entity::arrow::spit_from_llama(w, i, j);
        st_mut(w, i).did_spit = true;
        g.attack_time = crate::entity::mth::floor((dist * (INTERVAL - INTERVAL) as f32 + INTERVAL as f32) as f64);
    } else if g.attack_time < 0 {
        let d = target_dist_sqr.sqrt() / RADIUS as f64;
        g.attack_time = crate::entity::mth::floor(crate::entity::mth::lerp(d, INTERVAL as f64, INTERVAL as f64));
    }
}

/// `LlamaHurtByTargetGoal.canContinueToUse()`: a llama that just spat lets go of its revenge
/// target (and forgets the spit); else HurtByTargetGoal's.
pub fn llama_hurt_by_can_continue(w: &mut World, i: usize) -> Option<bool> {
    if !is_llama(w, i) {
        return None;
    }
    if st(w, i).did_spit {
        st_mut(w, i).did_spit = false;
        return Some(false);
    }
    None
}

pub fn start(w: &mut World, i: usize, g: &mut TraderGoal) {
    match g {
        TraderGoal::UseItem { milk } => {
            // setItemSlot(MAINHAND, the item), startUsingItem(MAIN_HAND): its 32 ticks (the
            // Consumable's 1.6 s); the ITEM_INTERACT_START game event
            let s = st_mut(w, i);
            s.hand = if *milk { Hand::Milk } else { Hand::Potion };
            s.using = Some((*milk, 32));
            item_interact_event(w, i, "item_interact_start");
        }
        TraderGoal::MoveTowardsRestriction { speed, wanted } => {
            let (p, s) = (*wanted, *speed);
            with_nav(w, i, |w, nm, nav, arena| {
                nav.move_to_xyz(w, nm, p.x, p.y, p.z, 1, s, arena);
            });
        }
        TraderGoal::DefendTrader { timestamp, attacker, base } => {
            target::set_target(w, i, *attacker);
            if let Some(h) = w.entities.list[i].leash.map(|l| l.holder).and_then(|h| crate::entity::index_of(w, h))
                && is_trader(w, h)
            {
                *timestamp = ai(w, h).last_hurt_by_mob_timestamp;
            }
            super::targets::target_start(base);
        }
        _ => {}
    }
}

pub fn stop(w: &mut World, i: usize, g: &mut TraderGoal) {
    match g {
        TraderGoal::UseItem { .. } => {
            // setItemSlot(MAINHAND, EMPTY); the finishing sound's pitch (nextFloat * 0.2 + 0.9)
            st_mut(w, i).hand = Hand::Empty;
            own(w, i).next_float();
        }
        TraderGoal::WanderToPosition => {
            st_mut(w, i).wander_target = None;
            ai_mut(w, i).nav.stop();
        }
        TraderGoal::DefendTrader { base, .. } => super::targets::target_stop(w, i, base),
        // RangedAttackGoal.stop: the target, sight and attack times reset
        TraderGoal::LlamaSpit(r) => *r = super::drowned::TridentAttack::default(),
        // LlamaFollowCaravanGoal.stop: leaveCaravan; the speed back to the double 2.1
        TraderGoal::LlamaFollowCaravan { speed, .. } => {
            leave_caravan(w, i);
            *speed = 2.1;
        }
        _ => {}
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut TraderGoal) {
    if let TraderGoal::LlamaSpit(r) = g {
        spit_tick(w, i, r);
        return;
    }
    if let TraderGoal::LlamaFollowCaravan { speed, .. } = g {
        caravan_tick(w, i, *speed);
        return;
    }
    if let TraderGoal::WanderToPosition = g {
        let Some(wp) = st(w, i).wander_target else { return };
        if !ai(w, i).nav.is_done() {
            return;
        }
        let pos = w.entities.list[i].pos;
        let (x, y, z) = if too_far(w, i, wp, 10.0) {
            let d = Vec3::new(wp.x as f64 - pos.x, wp.y as f64 - pos.y, wp.z as f64 - pos.z).normalize();
            let t = d.scale(10.0).add(pos.x, pos.y, pos.z);
            (t.x, t.y, t.z)
        } else {
            (wp.x as f64, wp.y as f64, wp.z as f64)
        };
        with_nav(w, i, |w, nm, nav, arena| {
            nav.move_to_xyz(w, nm, x, y, z, 1, 0.35, arena);
        });
    }
}

/// `WanderToPositionGoal.isTooFarAway(pos, distance)`: `!pos.closerToCenterThan(position, d)`.
fn too_far(w: &World, i: usize, p: BlockPos, d: f64) -> bool {
    let e = w.entities.list[i].pos;
    let (dx, dy, dz) = (p.x as f64 + 0.5 - e.x, p.y as f64 + 0.5 - e.y, p.z as f64 + 0.5 - e.z);
    !(dx * dx + dy * dy + dz * dz < d * d)
}

/// (mob-AI 2026-11-11) Whether entity `j` is a llama with the caravan's state (`e.is(LLAMA) ||
/// e.is(TRADER_LLAMA)`: redsim's llamas are the traders').
fn is_caravan_llama(w: &World, j: usize) -> bool {
    let e = &w.entities.list[j];
    !e.removed && matches!(&e.kind, crate::entity::EntityKind::Mob(m) if is_llama_id(m.ty.def().id)) && has_ai(w, j) && ai(w, j).trader.is_some()
}

fn caravan_of(w: &World, id: u32) -> Option<usize> {
    crate::entity::index_of(w, id).filter(|&j| is_caravan_llama(w, j))
}

/// `Llama.inCaravan()`.
fn in_caravan(w: &World, j: usize) -> bool {
    st(w, j).caravan_head.is_some()
}

/// `Llama.hasCaravanTail()`.
fn has_caravan_tail(w: &World, j: usize) -> bool {
    st(w, j).caravan_tail.is_some()
}

/// `isLeashed()`: a leash with a holder.
fn leashed(w: &World, j: usize) -> bool {
    w.entities.list[j].leash.is_some()
}

/// `Llama.leaveCaravan()`: the head forgets its tail, the llama its head.
fn leave_caravan(w: &mut World, i: usize) {
    if let Some(h) = st(w, i).caravan_head.and_then(|id| caravan_of(w, id)) {
        st_mut(w, h).caravan_tail = None;
    }
    st_mut(w, i).caravan_head = None;
}

/// `LlamaFollowCaravanGoal.firstIsLeashed(currentMob, counter)`: up the caravan (at most 8 heads)
/// to a leashed head.
fn first_is_leashed(w: &World, j: usize, counter: i32) -> bool {
    if counter > 8 {
        return false;
    }
    let Some(h) = st(w, j).caravan_head else { return false };
    let Some(h) = caravan_of(w, h) else {
        // a head that left the world: the game still reads its (cleared) leash
        return false;
    };
    if leashed(w, h) { true } else { first_is_leashed(w, h, counter + 1) }
}

fn dist_sqr(w: &World, a: usize, b: usize) -> f64 {
    let (p, q) = (w.entities.list[a].pos, w.entities.list[b].pos);
    let (dx, dy, dz) = (p.x - q.x, p.y - q.y, p.z - q.z);
    dx * dx + dy * dy + dz * dz
}

/// `LlamaFollowCaravanGoal.canUse()`: an unleashed llama out of any caravan joins the nearest
/// llama around it (its box grown 9, 4, 9; the level's order, the last of a tie) that heads a
/// caravan without a tail, else the nearest leashed one without a tail, unless it is within 2 or
/// its caravan has no leashed head within 8.
fn caravan_can_use(w: &mut World, i: usize) -> bool {
    if leashed(w, i) || in_caravan(w, i) {
        return false;
    }
    let id = w.entities.list[i].id;
    let area = w.entities.list[i].bb.inflate(9.0, 4.0, 9.0);
    let llamas: Vec<usize> = w.get_entities(area, |x| x.id != id && matches!(&x.kind, crate::entity::EntityKind::Mob(m) if m.ty.def().id == "llama" || is_llama_id(m.ty.def().id)));
    if llamas.iter().any(|&j| !is_caravan_llama(w, j) && !w.entities.list[j].removed) {
        refuse(w, "a natural llama in a trader llama's LlamaFollowCaravanGoal is not modeled");
        return false;
    }
    let mut closest: Option<usize> = None;
    let mut closest_d = f64::MAX;
    for &j in &llamas {
        if is_caravan_llama(w, j) && in_caravan(w, j) && !has_caravan_tail(w, j) {
            let d = dist_sqr(w, i, j);
            if !(d > closest_d) {
                closest_d = d;
                closest = Some(j);
            }
        }
    }
    if closest.is_none() {
        for &j in &llamas {
            if is_caravan_llama(w, j) && leashed(w, j) && !has_caravan_tail(w, j) {
                let d = dist_sqr(w, i, j);
                if !(d > closest_d) {
                    closest_d = d;
                    closest = Some(j);
                }
            }
        }
    }
    let Some(c) = closest else { return false };
    if closest_d < 4.0 {
        return false;
    }
    if !leashed(w, c) && !first_is_leashed(w, c, 1) {
        return false;
    }
    // joinCaravan(closest)
    let cid = w.entities.list[c].id;
    st_mut(w, i).caravan_head = Some(cid);
    st_mut(w, c).caravan_tail = Some(id);
    true
}

/// `LlamaFollowCaravanGoal.canContinueToUse()`: in a caravan whose head lives and leads up to a
/// leashed llama; over 26 from the head the speed grows by 1.2 while at most 3 (the check counter
/// at 20), then the goal ends when the counter is out.
fn caravan_can_continue(w: &mut World, i: usize, speed: &mut f64, dist_check: &mut i32) -> bool {
    let Some(h) = st(w, i).caravan_head.and_then(|id| crate::entity::index_of(w, id)) else { return false };
    let alive = !w.entities.list[h].removed && matches!(&w.entities.list[h].kind, crate::entity::EntityKind::Mob(m) if m.health > 0.0);
    if !alive || !first_is_leashed(w, i, 0) {
        return false;
    }
    if dist_sqr(w, i, h) > 676.0 {
        if *speed <= 3.0 {
            *speed *= 1.2;
            *dist_check = super::reduced_tick_delay(40);
            return true;
        }
        if *dist_check == 0 {
            return false;
        }
    }
    if *dist_check > 0 {
        *dist_check -= 1;
    }
    true
}

/// `LlamaFollowCaravanGoal.tick()`: toward the head, 2 short of it (`distanceTo`, a float), at the
/// goal's speed (a leash knot's llama stays: none here).
fn caravan_tick(w: &mut World, i: usize, speed: f64) {
    let Some(h) = st(w, i).caravan_head.and_then(|id| crate::entity::index_of(w, id)) else { return };
    let (p, q) = (w.entities.list[i].pos, w.entities.list[h].pos);
    let (xd, yd, zd) = ((p.x - q.x) as f32, (p.y - q.y) as f32, (p.z - q.z) as f32);
    let distance = (xd * xd + yd * yd + zd * zd).sqrt();
    let delta = Vec3::new(q.x - p.x, q.y - p.y, q.z - p.z).normalize().scale((distance as f64 - 2.0).max(0.0));
    let (x, y, z) = (p.x + delta.x, p.y + delta.y, p.z + delta.z);
    with_nav(w, i, |w, nm, nav, arena| {
        nav.move_to_xyz(w, nm, x, y, z, 1, speed, arena);
    });
}

fn item_interact_event(w: &mut World, i: usize, ev: &'static str) {
    let p = w.entities.list[i].pos;
    w.game_event_at(ev, [p.x, p.y, p.z], None, false);
}

/// `LivingEntity.updatingUsingItem` of a trader (after `super.tick()`): the item still in hand,
/// `onUseTick` (the Consumable's particles and sound past its first 7 ticks, every 4: four draws
/// of its own random), then the last tick's `completeUsingItem`: `onConsume` (the sound's four
/// draws again, the potion's invisibility for 3600 ticks or the milk clearing the effects, the
/// DRINK game event), its container left in hand, `stopUsingItem`.
pub fn updating_using_item(w: &mut World, i: usize) {
    if !has_ai(w, i) {
        return;
    }
    let Some((milk, remaining)) = ai(w, i).trader.as_ref().and_then(|s| s.using) else { return };
    let in_hand = st(w, i).hand == if milk { Hand::Milk } else { Hand::Potion };
    if !in_hand {
        stop_using(w, i);
        return;
    }
    let used = 32 - remaining;
    if used > 7 && remaining % 4 == 0 {
        emit_sound_draws(w, i);
    }
    let left = remaining - 1;
    st_mut(w, i).using = Some((milk, left));
    if left == 0 {
        emit_sound_draws(w, i);
        if milk {
            let e = &mut mob_mut(w, i).effects;
            *e = crate::entity::mob::Effects::default();
        } else {
            mob_mut(w, i).effects.invisibility = 3600;
        }
        st_mut(w, i).effects_dirty = true;
        item_interact_event(w, i, "drink");
        st_mut(w, i).hand = if milk { Hand::Bucket } else { Hand::Bottle };
        stop_using(w, i);
    }
}

/// `Consumable.emitParticlesAndSounds`: nextBoolean, a triangle (two floats) and a float between.
fn emit_sound_draws(w: &mut World, i: usize) {
    let r = own(w, i);
    r.next_bool();
    r.next_float();
    r.next_float();
    r.next_float();
}

/// `stopUsingItem`: the ITEM_INTERACT_FINISH game event when it was using.
fn stop_using(w: &mut World, i: usize) {
    let was = is_using_item(w, i);
    st_mut(w, i).using = None;
    if was {
        item_interact_event(w, i, "item_interact_finish");
    }
}

/// `WanderingTrader.aiStep`'s `maybeDespawn` (after `super.aiStep()`): not trading, the delay
/// counted down to 0 discards it. `TraderLlama.maybeDespawn`: while it may despawn (not tamed,
/// leashed to nothing but the trader, no player riding it, not age locked, not persistent), its
/// delay is its trader's less one when leashed to it, else its own less one; at 0 it lets go of
/// the lead and is discarded.
pub fn after_ai_step(w: &mut World, i: usize) {
    if !has_ai(w, i) || ai(w, i).trader.is_none() || w.entities.list[i].removed {
        return;
    }
    if st(w, i).llama {
        if mob(w, i).persistence_required {
            return;
        }
        let holder = w.entities.list[i].leash.map(|l| l.holder).and_then(|h| crate::entity::index_of(w, h));
        let to_trader = holder.filter(|&h| is_trader(w, h));
        if holder.is_some() && to_trader.is_none() {
            return;
        }
        let d = match to_trader {
            Some(h) => st(w, h).despawn_delay - 1,
            None => st(w, i).despawn_delay - 1,
        };
        st_mut(w, i).despawn_delay = d;
        if d <= 0 {
            w.entities.list[i].leash = None;
            discard(w, i);
        }
    } else {
        let d = st(w, i).despawn_delay;
        if d > 0 {
            st_mut(w, i).despawn_delay = d - 1;
            if d - 1 == 0 {
                discard(w, i);
            }
        }
    }
}

fn discard(w: &mut World, i: usize) {
    let (t, id) = (w.time, w.entities.list[i].id);
    w.entities.list[i].removed = true;
    w.mob.events.push(crate::entity::mob::MobEvent::Removed { t, id });
    crate::entity::stop_riding(w, i);
    crate::entity::eject_passengers(w, i);
}

/// The spawner's trader at cell `at` (`EntityTypes.WANDERING_TRADER.spawn(level, at, EVENT)`):
/// constructed, at the cell's bottom centre with a yaw from the level random (its head and body
/// too), `finalizeSpawn` (AgeableMobGroupData(false): no baby roll; Mob's follow-range triangle
/// and left-handed roll), added, its ambient sound's pitch (two of its own floats). Returns its id.
pub fn spawn_trader(w: &mut World, at: BlockPos) -> Option<u32> {
    spawn_one(w, at, "minecraft:wandering_trader")
}

/// A trader llama at cell `at` (`EntityTypes.TRADER_LLAMA.spawn(level, at, EVENT)`): as the
/// trader, its `finalizeSpawn` drawing (the level random) its strength (`nextFloat() < 0.04` gives
/// 5 else 3, then `1 + nextInt` of it), its variant (one of four), its max health (15 + nextInt(8)
/// + nextInt(9)) before Mob's draws. Returns its id.
pub fn spawn_llama(w: &mut World, at: BlockPos) -> Option<u32> {
    spawn_one(w, at, "minecraft:trader_llama")
}

fn spawn_one(w: &mut World, at: BlockPos, game_id: &str) -> Option<u32> {
    use crate::entity::mob::natural;
    let Some(k) = natural::kind_of(game_id) else {
        refuse(w, "a wandering trader's row is missing");
        return None;
    };
    let llama = is_llama_id(k.ty.def().id);
    let (id, extra) = natural::spawn_event_mob(w, k, at, |w| {
        if llama {
            let l = natural::level_rng(w);
            let max = if l.next_float() < 0.04f32 { 5 } else { 3 };
            let strength = 1 + l.next_int(max);
            let variant = l.next_int(4);
            let health = 15.0f32 + l.next_int(8) as f32 + l.next_int(9) as f32;
            Some((strength, variant, health))
        } else {
            None
        }
    });
    let i = crate::entity::index_of(w, id)?;
    if let Some((strength, variant, health)) = extra {
        let s = st_mut(w, i);
        s.strength = strength;
        s.variant = variant;
        if let Some(h) = ai_mut(w, i).horse.as_mut() {
            h.max_health = health;
        }
        mob_mut(w, i).health = health;
    }
    // EntityType.spawn: playAmbientSound after the add (getVoicePitch: two floats)
    own(w, i).next_float();
    own(w, i).next_float();
    Some(id)
}

/// The spawner's finishing touches on a spawned trader: `setDespawnDelay(48000)`,
/// `setWanderTarget(reference)`, `setHomeTo(reference, 16)`.
pub fn set_trader_home(w: &mut World, id: u32, reference: BlockPos) {
    let Some(i) = crate::entity::index_of(w, id) else { return };
    let s = st_mut(w, i);
    s.despawn_delay = 48000;
    s.wander_target = Some(reference);
    ai_mut(w, i).home = Some((reference, 16));
}
