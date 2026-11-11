//! The target goals (26.3 `net.minecraft.world.entity.ai.goal.target`): `TargetGoal` (keep a
//! target while it stays valid, seen and in range; the reach cache), `NearestAttackableTargetGoal`
//! (the `nextInt(randomInterval)` gate, the nearest valid one) and `HurtByTargetGoal` (the last
//! attacker, alerting the same kind).

use super::super::target::{self, Conditions, Who};
use super::super::{ai, ai_mut, follow_range, with_nav};
use crate::entity::mob::{mob, types::MobType};
use crate::entity::{Aabb, mth};
use crate::world::World;

/// `TargetGoal`'s fields.
#[derive(Clone, PartialEq, Debug)]
pub struct TargetBase {
    pub must_see: bool,
    pub must_reach: bool,
    pub reach_cache: i32,
    pub reach_cache_time: i32,
    pub unseen_ticks: i32,
    pub target_mob: Option<u32>,
    pub unseen_memory_ticks: i32,
}

impl TargetBase {
    pub fn new(must_see: bool, must_reach: bool) -> TargetBase {
        TargetBase { must_see, must_reach, reach_cache: 0, reach_cache_time: 0, unseen_ticks: 0, target_mob: None, unseen_memory_ticks: 60 }
    }
}

/// `TargetGoal.canContinueToUse()`.
pub fn target_can_continue(w: &mut World, i: usize, b: &mut TargetBase) -> bool {
    let id = target::get_target(w, i).or(b.target_mob);
    let Some(t) = id.and_then(|id| target::who(w, id)) else { return false };
    if !target::can_attack(w, i, t) {
        return false;
    }
    let within = follow_range(w, i);
    if w.entities.list[i].pos.distance_to_sqr(target::pos_of(w, t)) > within * within {
        return false;
    }
    if b.must_see {
        if target::has_line_of_sight(w, i, t) {
            b.unseen_ticks = 0;
        } else {
            b.unseen_ticks += 1;
            if b.unseen_ticks > super::reduced_tick_delay(b.unseen_memory_ticks) {
                return false;
            }
        }
    }
    let tid = target::id_of(w, t);
    target::set_target(w, i, Some(tid));
    true
}

/// `TargetGoal.start()`.
pub fn target_start(b: &mut TargetBase) {
    b.reach_cache = 0;
    b.reach_cache_time = 0;
    b.unseen_ticks = 0;
}

/// `TargetGoal.stop()`.
pub fn target_stop(w: &mut World, i: usize, b: &mut TargetBase) {
    target::set_target(w, i, None);
    b.target_mob = None;
}

/// `TargetGoal.canAttack(target, conditions)`.
pub fn can_attack(w: &mut World, i: usize, b: &mut TargetBase, t: Who, cond: &Conditions) -> bool {
    if !cond.test(w, i, t) {
        return false;
    }
    // isWithinHome(target's block) (mob-AI 2026-11-11: a leashed llama's home, 5 around its
    // trader)
    let tp = target::pos_of(w, t);
    let tb = crate::pos::BlockPos::new(mth::floor(tp.x), mth::floor(tp.y), mth::floor(tp.z));
    if !super::stroll::within_home(w, i, tb) {
        return false;
    }
    if b.must_reach {
        b.reach_cache_time -= 1;
        if b.reach_cache_time <= 0 {
            b.reach_cache = 0;
        }
        if b.reach_cache == 0 {
            b.reach_cache = if can_reach(w, i, b, t) { 1 } else { 2 };
        }
        if b.reach_cache == 2 {
            return false;
        }
    }
    true
}

/// `TargetGoal.canReach(target)`.
fn can_reach(w: &mut World, i: usize, b: &mut TargetBase, t: Who) -> bool {
    let n = w.entities.list[i].random.as_mut().expect("random").next_int(5);
    b.reach_cache_time = super::reduced_tick_delay(10 + n);
    let tp = target::pos_of(w, t);
    let tb = crate::pos::BlockPos::new(mth::floor(tp.x), mth::floor(tp.y), mth::floor(tp.z));
    let path = with_nav(w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, tb, 0, arena));
    let Some(last) = path.and_then(|p| p.end_node()) else { return false };
    let xx = last.x - tb.x;
    let zz = last.z - tb.z;
    ((xx * xx + zz * zz) as f64) <= 2.25
}

/// What a `NearestAttackableTargetGoal` looks for (its class and selector).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetType {
    Player,
    /// `Player` with the iron golem's `isAngryAt` selector (its anger target).
    AngryPlayer,
    /// `AbstractVillager` (villagers, wandering traders).
    Villager,
    IronGolem,
    /// `Turtle` with `BABY_ON_LAND_SELECTOR` (no turtles are modeled: never found).
    BabyTurtle,
    /// `Endermite` (none are modeled: never found).
    Endermite,
    /// `Mob` with `Enemy && !Creeper` (the iron golem's).
    EnemyNotCreeper,
    /// `SpiderTargetGoal` of players / iron golems: only in the dark (the brightness at the
    /// spider's eye under 0.5, checked before the interval's roll).
    SpiderPlayer,
    SpiderIronGolem,
    /// `Player` with the drowned's `okTarget` selector (by day only a player in water).
    DrownedPlayer,
    /// `Axolotl` (none are modeled: never found).
    Axolotl,
    /// (tickexact 2026-11-10) `Player` with the slime's selector: within 4 up or down of it.
    SlimePlayer,
    /// (mob-AI 2026-11-10) `Animal` with the wolf's `PREY_SELECTOR`: sheep, rabbits, foxes.
    WolfPrey,
    /// `AbstractSkeleton` (skeletons, strays, bogged, parched, wither skeletons): the wolf's.
    Skeleton,
    /// `Rabbit` (the cat's prey).
    Rabbit,
    /// (mob-AI 2026-11-11) `Zombie` (its subclasses: husks, drowned, zombie villagers) but not a
    /// zombified piglin: a trader llama's.
    ZombieNotPiglin,
    /// `Wolf`, not tame: `LlamaAttackWolfGoal` (its follow distance a quarter).
    LlamaWolf,
    /// `AbstractIllager` (pillagers, vindicators, evokers, illusioners: none modeled, never found).
    Illager,
}

/// `NearestAttackableTargetGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct NearestAttackableTarget {
    pub base: TargetBase,
    pub target_type: TargetType,
    pub random_interval: i32,
    pub found: Option<u32>,
    /// `NonTameRandomTargetGoal` (a tamable animal's: only untamed, and it keeps its target while
    /// its own conditions hold).
    pub non_tame: bool,
}

impl NearestAttackableTarget {
    /// `new NearestAttackableTargetGoal<>(mob, type, randomInterval, mustSee, mustReach, selector)`.
    pub fn new(target_type: TargetType, interval: i32, must_see: bool, must_reach: bool) -> NearestAttackableTarget {
        NearestAttackableTarget { base: TargetBase::new(must_see, must_reach), target_type, random_interval: super::reduced_tick_delay(interval), found: None, non_tame: false }
    }

    /// `new NonTameRandomTargetGoal<>(mob, type, mustSee false, selector)`: interval 10.
    pub fn non_tame(target_type: TargetType) -> NearestAttackableTarget {
        NearestAttackableTarget { non_tame: true, ..NearestAttackableTarget::new(target_type, 10, false, false) }
    }
}

/// `NonTameRandomTargetGoal.canContinueToUse()`: its own conditions on the target it found
/// (`targetConditions.test`: the combat test, the follow range its last search set, the line of
/// sight, its selector), and no `setTarget` (unlike `TargetGoal`'s).
pub fn non_tame_can_continue(w: &mut World, i: usize, g: &mut NearestAttackableTarget) -> bool {
    let Some(t) = g.found.and_then(|id| target::who(w, id)) else { return false };
    let selected = match t {
        Who::Entity(_) | Who::Foreign(_) => target::type_of(w, t).is_some_and(|ty| selects(g.target_type, ty)),
        Who::Player(_) => false,
    };
    let d = follow_range(w, i);
    selected && Conditions::combat().range(d).test(w, i, t)
}

/// The mob types a `NearestAttackableTargetGoal` of type `t` looks for (its class and selector);
/// the player kinds are found among the players.
fn selects(t: TargetType, ty: MobType) -> bool {
    match t {
        TargetType::Villager => ty == MobType::Villager || ty.def().id == "wandering_trader",
        TargetType::IronGolem | TargetType::SpiderIronGolem => ty == MobType::IronGolem,
        TargetType::BabyTurtle | TargetType::Endermite | TargetType::Axolotl | TargetType::Illager => false,
        TargetType::EnemyNotCreeper => is_enemy(ty),
        TargetType::WolfPrey => matches!(ty.game_id(), "minecraft:sheep" | "minecraft:rabbit" | "minecraft:fox"),
        TargetType::Skeleton => matches!(ty.game_id(), "minecraft:skeleton" | "minecraft:stray" | "minecraft:bogged" | "minecraft:parched" | "minecraft:wither_skeleton"),
        TargetType::Rabbit => ty.game_id() == "minecraft:rabbit",
        TargetType::ZombieNotPiglin => matches!(ty.game_id(), "minecraft:zombie" | "minecraft:husk" | "minecraft:drowned" | "minecraft:zombie_villager"),
        TargetType::LlamaWolf => ty.game_id() == "minecraft:wolf",
        TargetType::Player | TargetType::AngryPlayer | TargetType::SpiderPlayer | TargetType::DrownedPlayer | TargetType::SlimePlayer => false,
    }
}

/// `Enemy` among the modeled mobs, creepers aside (the iron golem's and the zombie's selectors).
pub fn is_enemy(ty: MobType) -> bool {
    ty.def().monster && ty.def().id != "creeper"
}

/// `canUse()`.
pub fn nearest_can_use(w: &mut World, i: usize, g: &mut NearestAttackableTarget) -> bool {
    if matches!(g.target_type, TargetType::SpiderPlayer | TargetType::SpiderIronGolem) && super::spider::eye_magic(w, i) >= 0.5f32 {
        return false;
    }
    if g.random_interval > 0 && w.entities.list[i].random.as_mut().expect("random").next_int(g.random_interval) != 0 {
        return false;
    }
    find_target(w, i, g);
    g.found.is_some()
}

/// `findTarget()`.
fn find_target(w: &mut World, i: usize, g: &mut NearestAttackableTarget) {
    // getFollowDistance (LlamaAttackWolfGoal's: a quarter of it)
    let d = if g.target_type == TargetType::LlamaWolf { follow_range(w, i) * 0.25 } else { follow_range(w, i) };
    let cond = Conditions::combat().range(d);
    let at = super::super::eye_pos(w, i);
    let list: Vec<Who> = match g.target_type {
        TargetType::Player | TargetType::SpiderPlayer => target::players(w),
        TargetType::SlimePlayer => {
            let y = w.entities.list[i].pos.y;
            target::players(w).into_iter().filter(|p| (target::pos_of(w, *p).y - y).abs() <= 4.0).collect()
        }
        TargetType::DrownedPlayer => target::players(w)
            .into_iter()
            .filter(|p| {
                let pp = target::pos_of(w, *p);
                let cell = crate::pos::BlockPos::new(crate::entity::mth::floor(pp.x), crate::entity::mth::floor(pp.y), crate::entity::mth::floor(pp.z));
                super::drowned::ok_target(w, crate::fluid::fluid_state(w, cell).fluid.is_water())
            })
            .collect(),
        TargetType::AngryPlayer => {
            let anger = ai(w, i).anger_target;
            target::players(w).into_iter().filter(|p| Some(target::id_of(w, *p)) == anger).collect()
        }
        t => {
            let area: Aabb = w.entities.list[i].bb.inflate(d, d, d);
            target::mobs_in(w, area, |ty| selects(t, ty))
        }
    };
    g.found = target::nearest(w, i, &list, &cond, at).map(|t| target::id_of(w, t));
}

/// `start()`.
pub fn nearest_start(w: &mut World, i: usize, g: &mut NearestAttackableTarget) {
    target::set_target(w, i, g.found);
    target_start(&mut g.base);
}

/// `HurtByTargetGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct HurtByTarget {
    pub base: TargetBase,
    pub alert_same_type: bool,
    pub timestamp: i32,
    /// (mob-AI 2026-11-11) `Llama.LlamaHurtByTargetGoal` (a llama that just spat lets go).
    pub llama: bool,
}

impl HurtByTarget {
    pub fn new(alert: bool) -> HurtByTarget {
        HurtByTarget { base: TargetBase::new(true, false), alert_same_type: alert, timestamp: 0, llama: false }
    }
}

/// `canUse()`: a new attack by a mob it may target (no line of sight needed).
pub fn hurt_by_can_use(w: &mut World, i: usize, g: &mut HurtByTarget) -> bool {
    let a = ai(w, i);
    let (ts, by) = (a.last_hurt_by_mob_timestamp, a.last_hurt_by_mob);
    let Some(t) = by.and_then(|id| target::who(w, id)) else { return false };
    if ts == g.timestamp {
        return false;
    }
    let cond = Conditions::combat().ignore_line_of_sight().ignore_invisibility();
    can_attack(w, i, &mut g.base, t, &cond)
}

/// `start()`.
pub fn hurt_by_start(w: &mut World, i: usize, g: &mut HurtByTarget) {
    let by = ai(w, i).last_hurt_by_mob;
    target::set_target(w, i, by);
    g.base.target_mob = target::get_target(w, i);
    g.timestamp = ai(w, i).last_hurt_by_mob_timestamp;
    g.base.unseen_memory_ticks = 300;
    if g.alert_same_type {
        alert_others(w, i);
    }
    target_start(&mut g.base);
}

/// `alertOthers()`: the mobs of the same class around (a zombie's: every zombie kind but zombified
/// piglins) with no target take the attacker.
fn alert_others(w: &mut World, i: usize) {
    let within = follow_range(w, i);
    let p = w.entities.list[i].pos;
    let area = Aabb::new(p.x, p.y, p.z, p.x + 1.0, p.y + 1.0, p.z + 1.0).inflate(within, 10.0, within);
    let ty = mob(w, i).ty;
    // getEntitiesOfClass(this.mob.getClass()): its Java class and the subclasses (a zombie's, a
    // baby's too: husks, drowned, zombie villagers, zombified piglins), less `toIgnoreAlert` by
    // exact class (the Zombie family's setAlertOthers(ZombifiedPiglin.class))
    let class = java_class(ty.def().id);
    let zombie_alert = matches!(class, "zombie" | "husk" | "drowned" | "zombie_villager");
    let family = |t: MobType| {
        let c = java_class(t.def().id);
        (c == class || (class == "zombie" && matches!(c, "husk" | "drowned" | "zombie_villager" | "zombified_piglin"))) && !(zombie_alert && c == "zombified_piglin")
    };
    let Some(by) = ai(w, i).last_hurt_by_mob else { return };
    for t in target::mobs_in(w, area, family) {
        let Who::Entity(j) = t else { continue };
        if j == i || !super::super::has_ai(w, j) || target::get_target(w, j).is_some() {
            continue;
        }
        target::set_target(w, j, Some(by));
    }
}

#[allow(dead_code)]
fn _a(w: &mut World, i: usize) {
    let _ = ai_mut(w, i);
}

/// A mob row's Java class, by its adult row (a baby's class is its adult's: `baby_zombie` is a
/// `Zombie`).
fn java_class(id: &str) -> &str {
    id.strip_prefix("baby_").unwrap_or(id)
}
