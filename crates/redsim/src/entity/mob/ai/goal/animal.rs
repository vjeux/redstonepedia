//! The farm animals' goals (`Pig`, `AbstractCow`, `Chicken`, `Sheep`: the adults; their babies'
//! `FollowParentGoal` and growing up are not modeled yet, so babies stay on the legacy walk model):
//! `PanicGoal` (refused once it would run: a hurt animal), `BreedGoal` (no animal is in love
//! without a player), `TemptGoal` (redsim's players hold nothing), `FollowParentGoal` (an adult
//! never), the sheep's `EatBlockGoal` (short grass or a grass block under it: eaten after 18 ticks
//! of its 20), and the chicken's `aiStep` (falling at 0.6 of its fall speed, laying an egg every
//! 6000..11999 ticks).

use super::{Goal, JUMP, LOOK, MOVE, reduced_tick_delay};
use crate::block::Block;
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

#[derive(Clone, PartialEq, Debug)]
pub enum AnimalGoal {
    Panic,
    Breed,
    Tempt,
    FollowParent,
    /// `EatBlockGoal.eatAnimationTick`.
    EatBlock { tick: i32 },
}

impl AnimalGoal {
    pub fn flags(&self) -> u8 {
        match self {
            AnimalGoal::Panic => MOVE,
            AnimalGoal::Breed | AnimalGoal::Tempt => MOVE | LOOK,
            AnimalGoal::FollowParent => 0,
            AnimalGoal::EatBlock { .. } => MOVE | LOOK | JUMP,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            AnimalGoal::Panic => "PanicGoal",
            AnimalGoal::Breed => "BreedGoal",
            AnimalGoal::Tempt => "TemptGoal",
            AnimalGoal::FollowParent => "FollowParentGoal",
            AnimalGoal::EatBlock { .. } => "EatBlockGoal",
        }
    }
}

/// Is this an adult farm animal whose goals are modeled?
pub fn is_animal(id: &str) -> bool {
    matches!(id, "pig" | "cow" | "chicken" | "sheep")
}

/// `registerGoals` of the pig, cow, chicken and sheep.
pub fn register(a: &mut crate::entity::mob::ai::MobAi, id: &str) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::stroll::RandomStroll;
    let g = &mut a.goals;
    let an = |k: AnimalGoal| Goal::Animal(k);
    match id {
        "pig" => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(3, an(AnimalGoal::Breed));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(4, an(AnimalGoal::Tempt));
            g.add(5, an(AnimalGoal::FollowParent));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        "cow" | "chicken" => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, an(AnimalGoal::FollowParent));
            g.add(5, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(6, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(7, Goal::RandomLookAround(RandomLookAround::default()));
        }
        "sheep" => {
            g.add(0, Goal::Float);
            g.add(1, an(AnimalGoal::Panic));
            g.add(2, an(AnimalGoal::Breed));
            g.add(3, an(AnimalGoal::Tempt));
            g.add(4, an(AnimalGoal::FollowParent));
            g.add(5, an(AnimalGoal::EatBlock { tick: 0 }));
            g.add(6, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
            g.add(7, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 6.0)));
            g.add(8, Goal::RandomLookAround(RandomLookAround::default()));
        }
        _ => {}
    }
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

pub fn can_use(w: &mut World, i: usize, g: &mut AnimalGoal) -> bool {
    match g {
        AnimalGoal::Panic => {
            let m = mob(w, i);
            if m.hurt_time > 0 || m.fire_ticks > 0 {
                refuse(w, "a hurt or burning animal's PanicGoal is not modeled yet");
            }
            false
        }
        AnimalGoal::Breed | AnimalGoal::Tempt | AnimalGoal::FollowParent => false,
        AnimalGoal::EatBlock { .. } => {
            // nextInt(adjustedTickDelay(adult 1000): 500) first, then short grass at its feet or
            // a grass block under them
            if own(w, i).next_int(reduced_tick_delay(1000)) != 0 {
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
        _ => can_use(w, i, g),
    }
}

pub fn start(w: &mut World, i: usize, g: &mut AnimalGoal) {
    if let AnimalGoal::EatBlock { tick } = g {
        // adjustedTickDelay(40) (not every tick: 20), the entity event, the navigation stopped
        *tick = reduced_tick_delay(40);
        crate::entity::mob::ai::with_nav(w, i, |_, _, nav, _| nav.stop());
    }
}

pub fn stop(_w: &mut World, _i: usize, g: &mut AnimalGoal) {
    if let AnimalGoal::EatBlock { tick } = g {
        *tick = 0;
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut AnimalGoal) {
    let AnimalGoal::EatBlock { tick } = g else { return };
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

/// `Sheep.ate()`: its wool back (no draw).
fn ate(_w: &mut World, _i: usize) {}

/// `Chicken`'s constructor draw: `eggTime = nextInt(6000) + 6000` (after the entity's own).
pub fn chicken_egg_time(r: &mut crate::rng::LegacyRandom) -> i32 {
    r.next_int(6000) + 6000
}

/// `Chicken.aiStep` after `super.aiStep()`: a falling chicken keeps 0.6 of its fall speed; an
/// adult lays an egg when its timer runs out (the CHICKEN_LAY gift table's egg, its sound's
/// pitch, a new timer).
pub fn chicken_after_ai_step(w: &mut World, i: usize) {
    if mob(w, i).ty.def().id != "chicken" {
        return;
    }
    let e = &mut w.entities.list[i];
    if !e.on_ground && e.delta_movement.y < 0.0 {
        let d = e.delta_movement;
        e.delta_movement = crate::entity::Vec3::new(d.x, d.y * 0.6, d.z);
    }
    let t = mob(w, i).egg_time - 1;
    mob_mut(w, i).egg_time = t;
    if t <= 0 && !mob(w, i).dead && w.entities.list[i].vehicle.is_none() {
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
