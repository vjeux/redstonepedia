//! `GoalSelector`, `WrappedGoal`, `Goal` (26.3 `net.minecraft.world.entity.ai.goal`) and the goals
//! the modeled mobs register (`registerGoals`), one file per family. A goal is a [`Goal`] variant
//! holding its fields; [`can_use`], [`can_continue`], [`start`], [`stop`] and [`tick`] dispatch to
//! the family's code. While a selector runs it is taken out of its mob, so the goals see the rest of
//! the mob (its navigation, controls, target) through the world.

pub mod float;
pub mod animal;
pub mod witch;
pub mod enderman;
pub mod golem;
pub mod creeper;
pub mod skeleton;
pub mod spider;
pub mod look;
pub mod melee;
pub mod stroll;
pub mod targets;
pub mod zombie;
pub mod drowned;

use super::{MobAi, ai_mut};
use crate::entity::mob::types::MobType;
use crate::world::World;

/// `Goal.Flag`, in enum order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    Move = 0,
    Look = 1,
    Jump = 2,
    Target = 3,
}

pub const MOVE: u8 = 1 << Flag::Move as u8;
pub const LOOK: u8 = 1 << Flag::Look as u8;
pub const JUMP: u8 = 1 << Flag::Jump as u8;
pub const TARGET: u8 = 1 << Flag::Target as u8;

/// `Goal.reducedTickDelay(ticks)` = `positiveCeilDiv(ticks, 2)`.
pub fn reduced_tick_delay(ticks: i32) -> i32 {
    (ticks + 1).div_euclid(2)
}

/// A goal with its own fields.
#[derive(Clone, PartialEq, Debug)]
pub enum Goal {
    Float,
    RandomStroll(stroll::RandomStroll),
    LookAtPlayer(look::LookAtPlayer),
    RandomLookAround(look::RandomLookAround),
    MeleeAttack(melee::MeleeAttack),
    ZombieAttackTurtleEgg(zombie::TurtleEgg),
    SpearUse,
    MoveThroughVillage(zombie::MoveThroughVillage),
    BreakDoor(zombie::BreakDoor),
    MoveTowardsTarget(golem::MoveTowardsTarget),
    OfferFlower(golem::OfferFlower),
    DefendVillage,
    /// `SwellGoal` (the creeper's fuse).
    Swell,
    /// `AvoidEntityGoal` of cats or ocelots (none modeled).
    AvoidEntity,
    /// `RestrictSunGoal` (the skeleton's: the navigation avoids the sun while it is bright).
    RestrictSun,
    /// `FleeSunGoal` (a burning skeleton's shade).
    FleeSun(skeleton::FleeSun),
    /// `RangedBowAttackGoal` (its tick is refused).
    RangedBow,
    /// `LeapAtTargetGoal` (the spider's, 0.4 up).
    LeapAtTarget,
    /// A cube mob's goals (`AbstractCubeMob`, the sulfur cube's).
    Cube(super::cube::CubeGoal),
    /// A farm animal's goals (panic, breeding, tempting, its parent, a sheep's grass).
    Animal(animal::AnimalGoal),
    /// A squid's goals (its random swim, its flight).
    Squid(super::squid::SquidGoal),
    /// A witch's ranged attack and its raiders' healing target.
    Witch(witch::WitchGoal),
    /// An enderman's own goals (freeze, leave and take a block, look for a player).
    Enderman(enderman::EndermanGoal),
    /// A drowned's own goals (water by day, the beach and the swim up at night, the trident).
    Drowned(drowned::DrownedGoal),
    ResetUniversalAnger,
    HurtByTarget(targets::HurtByTarget),
    NearestAttackableTarget(targets::NearestAttackableTarget),
}

impl Goal {
    /// `getFlags()`.
    pub fn flags(&self) -> u8 {
        match self {
            Goal::Float => JUMP,
            Goal::RandomStroll(_) => MOVE,
            Goal::LookAtPlayer(_) => LOOK,
            Goal::RandomLookAround(_) => MOVE | LOOK,
            Goal::MeleeAttack(_) => MOVE | LOOK,
            Goal::ZombieAttackTurtleEgg(_) => MOVE | JUMP,
            Goal::SpearUse => MOVE | LOOK,
            Goal::MoveThroughVillage(_) => MOVE,
            Goal::BreakDoor(_) => 0,
            Goal::MoveTowardsTarget(_) => MOVE,
            Goal::OfferFlower(_) => MOVE | LOOK,
            Goal::DefendVillage => TARGET,
            Goal::Swell | Goal::AvoidEntity => MOVE,
            Goal::RestrictSun => 0,
            Goal::FleeSun(_) => MOVE,
            Goal::RangedBow => MOVE | LOOK,
            Goal::LeapAtTarget => JUMP | MOVE,
            Goal::Cube(c) => c.flags(),
            Goal::Animal(a) => a.flags(),
            Goal::Squid(_) => 0,
            Goal::Witch(g) => g.flags(),
            Goal::Enderman(g) => g.flags(),
            Goal::Drowned(g) => g.flags(),
            Goal::ResetUniversalAnger => 0,
            Goal::HurtByTarget(_) | Goal::NearestAttackableTarget(_) => TARGET,
        }
    }

    /// `requiresUpdateEveryTick()`.
    pub fn every_tick(&self) -> bool {
        if let Goal::Cube(c) = self {
            return c.every_tick();
        }
        if let Goal::Squid(s) = self {
            return s.every_tick();
        }
        if let Goal::Drowned(d) = self {
            return d.every_tick();
        }
        matches!(self, Goal::Float | Goal::RandomLookAround(_) | Goal::MeleeAttack(_) | Goal::ZombieAttackTurtleEgg(_) | Goal::BreakDoor(_) | Goal::Swell | Goal::RangedBow)
    }

    /// `isInterruptable()`.
    pub fn interruptable(&self) -> bool {
        true
    }

    /// `adjustedTickDelay(ticks)`.
    pub fn adjusted_tick_delay(&self, ticks: i32) -> i32 {
        if self.every_tick() { ticks } else { reduced_tick_delay(ticks) }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Goal::Float => "FloatGoal",
            Goal::RandomStroll(s) => match s.variant {
                stroll::Variant::MoveBackToVillage => "MoveBackToVillageGoal",
                stroll::Variant::GolemInVillage => "GolemRandomStrollInVillageGoal",
                stroll::Variant::Plain => {
                    if s.water_avoiding.is_some() {
                        "WaterAvoidingRandomStrollGoal"
                    } else {
                        "RandomStrollGoal"
                    }
                }
            },
            Goal::LookAtPlayer(_) => "LookAtPlayerGoal",
            Goal::RandomLookAround(_) => "RandomLookAroundGoal",
            Goal::MeleeAttack(m) => m.kind.name(),
            Goal::ZombieAttackTurtleEgg(_) => "ZombieAttackTurtleEggGoal",
            Goal::SpearUse => "SpearUseGoal",
            Goal::MoveThroughVillage(_) => "MoveThroughVillageGoal",
            Goal::BreakDoor(_) => "BreakDoorGoal",
            Goal::MoveTowardsTarget(_) => "MoveTowardsTargetGoal",
            Goal::OfferFlower(_) => "OfferFlowerGoal",
            Goal::DefendVillage => "DefendVillageTargetGoal",
            Goal::Swell => "SwellGoal",
            Goal::AvoidEntity => "AvoidEntityGoal",
            Goal::RestrictSun => "RestrictSunGoal",
            Goal::FleeSun(_) => "FleeSunGoal",
            Goal::RangedBow => "RangedBowAttackGoal",
            Goal::LeapAtTarget => "LeapAtTargetGoal",
            Goal::Cube(c) => c.name(),
            Goal::Animal(a) => a.name(),
            Goal::Squid(s) => s.name(),
            Goal::Witch(g) => g.name(),
            Goal::Enderman(g) => g.name(),
            Goal::Drowned(g) => g.name(),
            Goal::ResetUniversalAnger => "ResetUniversalAngerTargetGoal",
            Goal::HurtByTarget(_) => "HurtByTargetGoal",
            Goal::NearestAttackableTarget(n) => match n.target_type {
                targets::TargetType::SpiderPlayer | targets::TargetType::SpiderIronGolem => "SpiderTargetGoal",
                _ => "NearestAttackableTargetGoal",
            },
        }
    }
}

/// `WrappedGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct WrappedGoal {
    pub priority: i32,
    pub goal: Goal,
    pub running: bool,
}

/// `GoalSelector`: goals in insertion order, the flag locks, the disabled flags.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct GoalSelector {
    pub goals: Vec<WrappedGoal>,
    /// `lockedFlags`: the index of the goal holding each flag.
    pub locked: [Option<usize>; 4],
    pub disabled: u8,
}

impl GoalSelector {
    /// `addGoal(priority, goal)`.
    pub fn add(&mut self, priority: i32, goal: Goal) {
        self.goals.push(WrappedGoal { priority, goal, running: false });
    }

    /// `setControlFlag(flag, enabled)`.
    pub fn set_control_flag(&mut self, f: Flag, enabled: bool) {
        if enabled {
            self.disabled &= !(1 << f as u8);
        } else {
            self.disabled |= 1 << f as u8;
        }
    }

    /// The running goals' names by priority (the harness compares them).
    pub fn running_names(&self) -> Vec<(i32, &'static str)> {
        self.goals.iter().filter(|g| g.running).map(|g| (g.priority, g.goal.name())).collect()
    }
}

/// `goalCanBeReplacedForAllFlags(goal, lockedFlags)`.
fn can_replace_all(sel: &GoalSelector, k: usize) -> bool {
    let g = &sel.goals[k];
    for f in 0..4 {
        if g.goal.flags() & (1 << f) == 0 {
            continue;
        }
        if let Some(h) = sel.locked[f] {
            let holder = &sel.goals[h];
            // canBeReplacedBy: interruptable and a lower priority number
            if !(holder.goal.interruptable() && g.priority < holder.priority) {
                return false;
            }
        }
    }
    true
}

/// Run one of mob `i`'s selectors (`targets`: the target selector): `tickRunningGoals(false)` on
/// the off ticks, else the full `tick()`.
pub fn tick_selector(w: &mut World, i: usize, targets: bool, every_other: bool) {
    let mut sel = {
        let a = ai_mut(w, i);
        std::mem::take(if targets { &mut a.targets } else { &mut a.goals })
    };
    if every_other {
        tick_running(w, i, &mut sel, false);
    } else {
        full_tick(w, i, &mut sel);
    }
    let a = ai_mut(w, i);
    let slot = if targets { &mut a.targets } else { &mut a.goals };
    // goals a goal added meanwhile (none modeled) would be lost: the slot is empty while taken
    debug_assert!(slot.goals.is_empty());
    *slot = sel;
}

/// `GoalSelector.tick()`.
fn full_tick(w: &mut World, i: usize, sel: &mut GoalSelector) {
    for k in 0..sel.goals.len() {
        if sel.goals[k].running && ((sel.goals[k].goal.flags() & sel.disabled) != 0 || !can_continue(w, i, &mut sel.goals[k].goal)) {
            stop_wrapped(w, i, sel, k);
        }
    }
    for f in 0..4 {
        if let Some(h) = sel.locked[f]
            && !sel.goals[h].running
        {
            sel.locked[f] = None;
        }
    }
    for k in 0..sel.goals.len() {
        if !sel.goals[k].running && (sel.goals[k].goal.flags() & sel.disabled) == 0 && can_replace_all(sel, k) && can_use(w, i, &mut sel.goals[k].goal) {
            let flags = sel.goals[k].goal.flags();
            for f in 0..4 {
                if flags & (1 << f) == 0 {
                    continue;
                }
                if let Some(h) = sel.locked[f] {
                    stop_wrapped(w, i, sel, h);
                }
                sel.locked[f] = Some(k);
            }
            start_wrapped(w, i, sel, k);
        }
    }
    tick_running(w, i, sel, true);
}

fn stop_wrapped(w: &mut World, i: usize, sel: &mut GoalSelector, k: usize) {
    if sel.goals[k].running {
        sel.goals[k].running = false;
        stop(w, i, &mut sel.goals[k].goal);
    }
}

fn start_wrapped(w: &mut World, i: usize, sel: &mut GoalSelector, k: usize) {
    if !sel.goals[k].running {
        sel.goals[k].running = true;
        start(w, i, &mut sel.goals[k].goal);
    }
}

/// `tickRunningGoals(force)`.
fn tick_running(w: &mut World, i: usize, sel: &mut GoalSelector, force: bool) {
    for k in 0..sel.goals.len() {
        if sel.goals[k].running && (force || sel.goals[k].goal.every_tick()) {
            tick(w, i, &mut sel.goals[k].goal);
        }
    }
}

/// `canUse()`.
pub fn can_use(w: &mut World, i: usize, g: &mut Goal) -> bool {
    match g {
        Goal::Float => float::can_use(w, i),
        Goal::RandomStroll(s) => stroll::can_use(w, i, s),
        Goal::LookAtPlayer(l) => look::look_can_use(w, i, l),
        Goal::RandomLookAround(l) => look::random_can_use(w, i, l),
        Goal::MeleeAttack(m) => melee::can_use(w, i, m),
        Goal::ZombieAttackTurtleEgg(t) => zombie::turtle_can_use(w, i, t),
        Goal::SpearUse => zombie::spear_can_use(w, i),
        Goal::MoveThroughVillage(v) => zombie::village_can_use(w, i, v),
        Goal::BreakDoor(d) => zombie::door_can_use(w, i, d),
        Goal::MoveTowardsTarget(g) => golem::towards_can_use(w, i, g),
        Goal::OfferFlower(g) => golem::flower_can_use(w, i, g),
        Goal::DefendVillage => golem::defend_can_use(w, i),
        Goal::Swell => creeper::swell_can_use(w, i),
        Goal::AvoidEntity => false,
        Goal::RestrictSun => skeleton::restrict_sun_can_use(w, i),
        Goal::FleeSun(g) => skeleton::flee_sun_can_use(w, i, g),
        Goal::RangedBow => skeleton::bow_can_use(w, i),
        Goal::LeapAtTarget => spider::leap_can_use(w, i),
        Goal::Cube(c) => super::cube::can_use(w, i, c),
        Goal::Animal(a) => animal::can_use(w, i, a),
        Goal::Squid(s) => super::squid::can_use(w, i, s),
        Goal::Witch(g) => witch::can_use(w, i, g),
        Goal::Enderman(g) => enderman::can_use(w, i, g),
        Goal::Drowned(g) => drowned::can_use(w, i, g),
        Goal::ResetUniversalAnger => false,
        Goal::HurtByTarget(h) => targets::hurt_by_can_use(w, i, h),
        Goal::NearestAttackableTarget(n) => targets::nearest_can_use(w, i, n),
    }
}

/// `canContinueToUse()`.
pub fn can_continue(w: &mut World, i: usize, g: &mut Goal) -> bool {
    match g {
        Goal::Float => float::can_use(w, i),
        Goal::RandomStroll(s) => stroll::can_continue(w, i, s),
        Goal::LookAtPlayer(l) => look::look_can_continue(w, i, l),
        Goal::RandomLookAround(l) => l.look_time >= 0,
        Goal::MeleeAttack(m) => melee::can_continue(w, i, m),
        Goal::ZombieAttackTurtleEgg(t) => zombie::turtle_can_continue(w, i, t),
        Goal::SpearUse => false,
        Goal::MoveThroughVillage(v) => zombie::village_can_continue(w, i, v),
        Goal::BreakDoor(d) => zombie::door_can_continue(w, i, d),
        Goal::MoveTowardsTarget(g) => golem::towards_can_continue(w, i, g),
        Goal::OfferFlower(g) => g.tick > 0,
        Goal::DefendVillage | Goal::ResetUniversalAnger => false,
        Goal::Swell => creeper::swell_can_use(w, i),
        Goal::AvoidEntity => false,
        Goal::RestrictSun => skeleton::restrict_sun_can_use(w, i),
        Goal::FleeSun(_) => !super::nav_of(w, i).is_done(),
        Goal::RangedBow => skeleton::bow_can_continue(w, i),
        Goal::LeapAtTarget => spider::leap_can_continue(w, i),
        // Goal.canContinueToUse: canUse()
        Goal::Cube(c) => super::cube::can_use(w, i, c),
        Goal::Animal(a) => animal::can_continue(w, i, a),
        Goal::Squid(s) => super::squid::can_use(w, i, s),
        Goal::Witch(g) => witch::can_use(w, i, g),
        // Goal.canContinueToUse: canUse()
        Goal::Enderman(g) => enderman::can_use(w, i, g),
        Goal::Drowned(g) => drowned::can_continue(w, i, g),
        Goal::HurtByTarget(h) => targets::target_can_continue(w, i, &mut h.base),
        Goal::NearestAttackableTarget(n) => targets::target_can_continue(w, i, &mut n.base),
    }
}

/// `start()`.
pub fn start(w: &mut World, i: usize, g: &mut Goal) {
    match g {
        Goal::Float => {}
        Goal::RandomStroll(s) => stroll::start(w, i, s),
        Goal::LookAtPlayer(l) => look::look_start(w, i, l),
        Goal::RandomLookAround(l) => look::random_start(w, i, l),
        Goal::MeleeAttack(m) => melee::start(w, i, m),
        Goal::ZombieAttackTurtleEgg(t) => zombie::turtle_start(w, i, t),
        Goal::SpearUse => {}
        Goal::MoveThroughVillage(v) => zombie::village_start(w, i, v),
        Goal::BreakDoor(d) => zombie::door_start(w, i, d),
        Goal::MoveTowardsTarget(g) => golem::towards_start(w, i, g),
        Goal::OfferFlower(g) => golem::flower_start(w, i, g),
        Goal::DefendVillage | Goal::ResetUniversalAnger => {}
        Goal::Swell => creeper::swell_start(w, i),
        Goal::AvoidEntity => {}
        Goal::RestrictSun => super::nav_of_mut(w, i).avoid_sun = true,
        Goal::FleeSun(g) => skeleton::flee_sun_start(w, i, g),
        Goal::RangedBow => {}
        Goal::LeapAtTarget => spider::leap_start(w, i),
        Goal::Cube(_) => {}
        Goal::Animal(a) => animal::start(w, i, a),
        Goal::Squid(_) => {}
        Goal::Witch(_) => {}
        Goal::Enderman(_) => {}
        Goal::Drowned(g) => drowned::start(w, i, g),
        Goal::HurtByTarget(h) => targets::hurt_by_start(w, i, h),
        Goal::NearestAttackableTarget(n) => targets::nearest_start(w, i, n),
    }
}

/// `stop()`.
pub fn stop(w: &mut World, i: usize, g: &mut Goal) {
    match g {
        Goal::Float => {}
        Goal::RandomStroll(_) => super::with_nav(w, i, |_, _, nav, _| nav.stop()),
        Goal::LookAtPlayer(l) => l.look_at = None,
        Goal::RandomLookAround(_) => {}
        Goal::MeleeAttack(m) => melee::stop(w, i, m),
        Goal::ZombieAttackTurtleEgg(t) => zombie::turtle_stop(w, i, t),
        Goal::SpearUse => {}
        Goal::MoveThroughVillage(v) => zombie::village_stop(w, i, v),
        Goal::BreakDoor(d) => zombie::door_stop(w, i, d),
        Goal::MoveTowardsTarget(g) => g.target = None,
        Goal::OfferFlower(g) => g.entity = None,
        Goal::DefendVillage | Goal::ResetUniversalAnger => {}
        Goal::Swell => super::ai_mut(w, i).swell_target = None,
        Goal::AvoidEntity => {}
        Goal::RestrictSun => super::nav_of_mut(w, i).avoid_sun = false,
        Goal::FleeSun(_) | Goal::RangedBow | Goal::LeapAtTarget | Goal::Cube(_) => {}
        Goal::Animal(a) => animal::stop(w, i, a),
        Goal::Squid(_) => {}
        Goal::Witch(_) => {}
        Goal::Enderman(_) => {}
        Goal::Drowned(d) => drowned::stop(w, i, d),
        Goal::HurtByTarget(h) => targets::target_stop(w, i, &mut h.base),
        Goal::NearestAttackableTarget(n) => targets::target_stop(w, i, &mut n.base),
    }
}

/// `tick()`.
pub fn tick(w: &mut World, i: usize, g: &mut Goal) {
    match g {
        Goal::Float => float::tick(w, i),
        Goal::RandomStroll(_) => {}
        Goal::LookAtPlayer(l) => look::look_tick(w, i, l),
        Goal::RandomLookAround(l) => look::random_tick(w, i, l),
        Goal::MeleeAttack(m) => melee::tick(w, i, m),
        Goal::ZombieAttackTurtleEgg(t) => zombie::turtle_tick(w, i, t),
        Goal::SpearUse => {}
        Goal::MoveThroughVillage(_) => {}
        Goal::BreakDoor(d) => zombie::door_tick(w, i, d),
        Goal::OfferFlower(g) => golem::flower_tick(w, i, g),
        Goal::MoveTowardsTarget(_) | Goal::DefendVillage | Goal::ResetUniversalAnger => {}
        Goal::Swell => creeper::swell_tick(w, i),
        Goal::AvoidEntity => {}
        Goal::RangedBow => skeleton::bow_tick(w),
        Goal::RestrictSun | Goal::FleeSun(_) | Goal::LeapAtTarget => {}
        Goal::Cube(c) => super::cube::tick(w, i, c),
        Goal::Animal(a) => animal::tick(w, i, a),
        Goal::Squid(s) => super::squid::tick(w, i, s),
        Goal::Witch(_) => {}
        Goal::Enderman(g) => enderman::tick(w, i, g),
        Goal::Drowned(d) => drowned::tick(w, i, d),
        Goal::HurtByTarget(_) | Goal::NearestAttackableTarget(_) => {}
    }
}

/// `registerGoals()` of each modeled type.
pub fn register_goals(ai: &mut MobAi, ty: MobType) {
    match ty.def().id {
        "zombie" | "baby_zombie" | "zombie_villager" | "baby_zombie_villager" => zombie::register(ai),
        "zombified_piglin" | "baby_zombified_piglin" => zombie::register_zombified_piglin(ai),
        "drowned" | "baby_drowned" => drowned::register(ai),
        "iron_golem" => golem::register(ai),
        "creeper" => creeper::register(ai),
        "skeleton" | "stray" | "bogged" => skeleton::register(ai),
        "spider" | "cave_spider" => spider::register(ai),
        "witch" => witch::register(ai),
        "enderman" => enderman::register(ai),
        id if animal::is_animal(id) => animal::register(ai, id),
        _ => {}
    }
    // FloatGoal's constructor: `mob.getNavigation().setCanFloat(true)`
    if ai.goals.goals.iter().any(|g| matches!(g.goal, Goal::Float)) {
        ai.nav.flags.can_float = true;
    }
}
