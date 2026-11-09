//! The farm animals' goals (`Pig`, `AbstractCow`, `Chicken`, `Sheep`, adults and babies):
//! `PanicGoal` (refused once it would run: a hurt animal), `BreedGoal` (no animal is in love
//! without a player), `TemptGoal` (redsim's players hold nothing), `FollowParentGoal` (a baby walks
//! after the closest adult of its kind within 8 x 4 x 8 when 3 or more away), the sheep's
//! `EatBlockGoal` (short grass or a grass block under it: eaten at the 4th last tick of its 20), the
//! chicken's `aiStep` (falling at 0.6 of its fall speed, an adult laying an egg every 6000..11999
//! ticks), and `AgeableMob.aiStep`'s age (a baby grows up at 0: the adult's size).

use super::{Goal, JUMP, LOOK, MOVE, reduced_tick_delay};
use crate::block::Block;
use crate::entity::EntityKind;
use crate::entity::mob::ai::{ai, ai_mut};
use crate::entity::mob::types::MobType;
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

#[derive(Clone, PartialEq, Debug)]
pub enum AnimalGoal {
    Panic,
    Breed,
    Tempt,
    /// `FollowParentGoal`: its speed, the parent and `timeToRecalcPath`.
    FollowParent { speed: f64, parent: Option<u32>, recalc: i32 },
    /// `EatBlockGoal.eatAnimationTick`.
    EatBlock { tick: i32 },
}

impl AnimalGoal {
    pub fn flags(&self) -> u8 {
        match self {
            AnimalGoal::Panic => MOVE,
            AnimalGoal::Breed | AnimalGoal::Tempt => MOVE | LOOK,
            AnimalGoal::FollowParent { .. } => 0,
            AnimalGoal::EatBlock { .. } => MOVE | LOOK | JUMP,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AnimalGoal::Panic => "PanicGoal",
            AnimalGoal::Breed => "BreedGoal",
            AnimalGoal::Tempt => "TemptGoal",
            AnimalGoal::FollowParent { .. } => "FollowParentGoal",
            AnimalGoal::EatBlock { .. } => "EatBlockGoal",
        }
    }
}

/// The adult kind of a farm animal row (a baby's adult), or `None`.
pub fn adult_of(id: &str) -> Option<&'static str> {
    match id {
        "pig" | "baby_pig" => Some("pig"),
        "cow" | "baby_cow" => Some("cow"),
        "chicken" | "baby_chicken" => Some("chicken"),
        "sheep" | "baby_sheep" => Some("sheep"),
        _ => None,
    }
}

/// Is this a farm animal (adult or baby) whose goals are modeled?
pub fn is_animal(id: &str) -> bool {
    adult_of(id).is_some()
}

/// Is this farm animal a baby?
pub fn is_baby(id: &str) -> bool {
    is_animal(id) && id.starts_with("baby_")
}

/// `registerGoals` of the pig, cow, chicken and sheep (a baby's are its adult's).
pub fn register(a: &mut crate::entity::mob::ai::MobAi, id: &str) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::stroll::RandomStroll;
    let g = &mut a.goals;
    let an = |k: AnimalGoal| Goal::Animal(k);
    let follow = |speed: f64| Goal::Animal(AnimalGoal::FollowParent { speed, parent: None, recalc: 0 });
    match adult_of(id) {
        Some("pig") => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(3, an(AnimalGoal::Breed));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(5, follow(1.1));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        Some(k @ ("cow" | "chicken")) => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, follow(if k == "cow" { 1.25 } else { 1.1 }));
            g.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(7, Goal::RandomLookAround(RandomLookAround::default()));
        }
        Some("sheep") => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, follow(1.1));
            g.add(5, an(AnimalGoal::EatBlock { tick: 0 }));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        _ => {}
    }
    // a natural baby spawns at age -24000 (AgeableMob.finalizeSpawn); a summon's data sets it
    a.age = if is_baby(id) { -24000 } else { 0 };
}

fn refuse(w: &mut World, why: &str) {
    if !w.errors.iter().any(|e| e == why) {
        w.errors.push(why.to_string());
    }
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

fn is_grass_block(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).block_id() == "minecraft:grass_block",
        _ => b.to_string().starts_with("minecraft:grass_block"),
    }
}

fn is_short_grass(b: &Block) -> bool {
    match b {
        Block::Terrain(id) => crate::terrain::blocks::props(*id).block_id() == "minecraft:short_grass",
        _ => b.to_string().starts_with("minecraft:short_grass"),
    }
}

/// `Animal.getWalkTargetValue`: 10 on a grass block, else the light's cost
/// (`getPathfindingCostFromLightLevels`: the magic value - 0.5).
pub fn walk_target_value(w: &mut World, p: BlockPos) -> f64 {
    if is_grass_block(&w.get(p.below())) {
        return 10.0;
    }
    (super::stroll::light_magic(w, p) - 0.5f32) as f64
}

/// `AgeableMob.getAge()`.
fn age(w: &World, i: usize) -> i32 {
    ai(w, i).age
}

/// `FollowParentGoal.canUse`: a baby; the closest adult of its class in its box inflated 8, 4, 8
/// (`!(d > closest)`: the last of equals), 3 or more blocks away.
fn follow_can_use(w: &mut World, i: usize, parent: &mut Option<u32>) -> bool {
    if age(w, i) >= 0 {
        return false;
    }
    let kind = adult_of(mob(w, i).ty.def().id);
    let (me, pos) = (w.entities.list[i].id, w.entities.list[i].pos);
    let query = w.entities.list[i].bb.inflate(8.0, 4.0, 8.0);
    let found = w.get_entities(query, |e| matches!(&e.kind, EntityKind::Mob(m) if adult_of(m.ty.def().id) == kind));
    let mut closest: Option<(u32, f64)> = None;
    for j in found {
        let e = &w.entities.list[j];
        let adult = match &e.kind {
            EntityKind::Mob(m) => m.ai.as_deref().map_or(!is_baby(m.ty.def().id), |a| a.age >= 0),
            _ => false,
        };
        if !adult {
            continue;
        }
        let d = pos.distance_to_sqr(e.pos);
        if closest.is_none_or(|(_, c)| !(d > c)) {
            closest = Some((e.id, d));
        }
    }
    let _ = me;
    match closest {
        Some((id, d)) if d >= 9.0 => {
            *parent = Some(id);
            true
        }
        _ => false,
    }
}

/// `FollowParentGoal.canContinueToUse`: still a baby, the parent alive, between 3 and 16 away.
fn follow_can_continue(w: &mut World, i: usize, parent: Option<u32>) -> bool {
    if age(w, i) >= 0 {
        return false;
    }
    let Some(j) = parent.and_then(|p| crate::entity::index_of(w, p)) else { return false };
    let alive = matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.health > 0.0) && !w.entities.list[j].removed;
    if !alive {
        return false;
    }
    let d = w.entities.list[i].pos.distance_to_sqr(w.entities.list[j].pos);
    !(d < 9.0) && !(d > 256.0)
}

pub fn can_use(w: &mut World, i: usize, g: &mut AnimalGoal) -> bool {
    match g {
        AnimalGoal::Panic => {
            let m = mob(w, i);
            if m.hurt_time > 0 || m.fire_ticks > 0 {
                refuse(w, "a hurt or burning animal's PanicGoal is not modeled yet");
            }
            false
        }
        AnimalGoal::Breed | AnimalGoal::Tempt => false,
        AnimalGoal::FollowParent { parent, .. } => follow_can_use(w, i, parent),
        AnimalGoal::EatBlock { .. } => {
            // nextInt(adjustedTickDelay(a baby's 50, an adult's 1000)), then short grass at its
            // feet or a grass block under them
            let bound = reduced_tick_delay(if age(w, i) < 0 { 50 } else { 1000 });
            if own(w, i).next_int(bound) != 0 {
                return false;
            }
            let p = w.entities.list[i].block_position();
            is_short_grass(&w.get(p)) || is_grass_block(&w.get(p.below()))
        }
    }
}

pub fn can_continue(w: &mut World, i: usize, g: &mut AnimalGoal) -> bool {
    match g {
        AnimalGoal::EatBlock { tick } => *tick > 0,
        AnimalGoal::FollowParent { parent, .. } => follow_can_continue(w, i, *parent),
        _ => can_use(w, i, g),
    }
}

pub fn start(w: &mut World, i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::EatBlock { tick } => {
            // adjustedTickDelay(40) (not every tick: 20), the entity event, the navigation stopped
            *tick = reduced_tick_delay(40);
            crate::entity::mob::ai::with_nav(w, i, |_, _, nav, _| nav.stop());
        }
        AnimalGoal::FollowParent { recalc, .. } => *recalc = 0,
        _ => {}
    }
}

pub fn stop(_w: &mut World, _i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::EatBlock { tick } => *tick = 0,
        AnimalGoal::FollowParent { parent, .. } => *parent = None,
        _ => {}
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut AnimalGoal) {
    match g {
        AnimalGoal::FollowParent { speed, parent, recalc } => {
            *recalc -= 1;
            if *recalc <= 0 {
                *recalc = reduced_tick_delay(10);
                if let Some(j) = parent.and_then(|p| crate::entity::index_of(w, p)) {
                    // moveTo(parent, speed): createPath(its block position, 1)
                    let (s, tb) = (*speed, w.entities.list[j].block_position());
                    crate::entity::mob::ai::with_nav(w, i, |w, nm, nav, arena| nav.move_to_entity_default(w, nm, tb, s, arena));
                }
            }
        }
        AnimalGoal::EatBlock { tick } => eat_tick(w, i, tick),
        _ => {}
    }
}

fn eat_tick(w: &mut World, i: usize, tick: &mut i32) {
    *tick = (*tick - 1).max(0);
    if *tick != reduced_tick_delay(4) {
        return;
    }
    let p = w.entities.list[i].block_position();
    let griefing = w.mob.ai.mob_griefing;
    if is_short_grass(&w.get(p)) {
        if griefing {
            // destroyBlock(pos, false): no drops
            w.destroy_block(p, false, 512);
        }
        ate(w, i);
    } else if is_grass_block(&w.get(p.below())) {
        if griefing {
            let dirt = Block::parse("minecraft:dirt").expect("dirt");
            w.set_block(p.below(), dirt, 2);
        }
        ate(w, i);
    }
}

/// `Sheep.ate()`: its wool back; a baby ages 60 s (`ageUp(60)`: +1200, at most 0).
fn ate(w: &mut World, i: usize) {
    let a = age(w, i);
    if a < 0 {
        set_age(w, i, (a + 1200).min(0));
    }
}

/// `AgeableMob.setAge`: crossing 0 up grows the baby into its adult (`refreshDimensions`: the
/// adult's box at the same feet; `fudgePositionAfterSizeChange` keeps them where nothing
/// collides, the only case modeled).
fn set_age(w: &mut World, i: usize, new: i32) {
    let old = age(w, i);
    ai_mut(w, i).age = new;
    if old < 0 && new >= 0 {
        grow_up(w, i);
    }
}

fn grow_up(w: &mut World, i: usize) {
    let Some(adult) = adult_of(mob(w, i).ty.def().id) else { return };
    let Some(ty) = MobType::from_id(adult) else { return };
    mob_mut(w, i).ty = ty;
    let pos = w.entities.list[i].pos;
    let (wd, h) = mob(w, i).dims();
    let bb = crate::entity::make_bounding_box(pos, wd, h);
    w.entities.list[i].bb = bb;
    if !crate::entity::no_collision(w, i, bb.deflate(1.0e-7, 1.0e-7, 1.0e-7)) {
        refuse(w, "a baby animal growing up against a block (fudgePositionAfterSizeChange) is not modeled");
    }
}

/// `AgeableMob.aiStep`'s age after `LivingEntity.aiStep`: a baby's counts up to 0 (an adult's
/// breeding cooldown down to 0); then the chicken's tail.
pub fn after_ai_step(w: &mut World, i: usize) {
    if !is_animal(mob(w, i).ty.def().id) || mob(w, i).health <= 0.0 || w.entities.list[i].removed {
        return;
    }
    let a = age(w, i);
    if a < 0 {
        set_age(w, i, a + 1);
    } else if a > 0 {
        set_age(w, i, a - 1);
    }
    chicken_after_ai_step(w, i);
}

/// `Chicken`'s constructor draw: `eggTime = nextInt(6000) + 6000` (after the entity's own).
pub fn chicken_egg_time(r: &mut crate::rng::LegacyRandom) -> i32 {
    r.next_int(6000) + 6000
}

/// `Chicken.aiStep` after `super.aiStep()`: a falling chicken keeps 0.6 of its fall speed; an
/// adult lays an egg when its timer runs out (the CHICKEN_LAY gift table's egg, its sound's
/// pitch, a new timer; a baby's timer stands still).
fn chicken_after_ai_step(w: &mut World, i: usize) {
    if adult_of(mob(w, i).ty.def().id) != Some("chicken") {
        return;
    }
    let e = &mut w.entities.list[i];
    if !e.on_ground && e.delta_movement.y < 0.0 {
        let d = e.delta_movement;
        e.delta_movement = crate::entity::Vec3::new(d.x, d.y * 0.6, d.z);
    }
    if age(w, i) < 0 || w.entities.list[i].vehicle.is_some() {
        return;
    }
    let t = mob(w, i).egg_time - 1;
    mob_mut(w, i).egg_time = t;
    if t <= 0 {
        // dropFromGiftLootTable(CHICKEN_LAY): the variant's egg (a temperate chicken's: the
        // plains samples'), no draw in the table; spawnAtLocation; the sound's pitch (two floats)
        crate::entity::mob::living::spawn_item(w, i, "minecraft:egg", 1);
        let r = own(w, i);
        r.next_float();
        r.next_float();
        let next = chicken_egg_time(own(w, i));
        mob_mut(w, i).egg_time = next;
    }
}
