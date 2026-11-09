//! The cube mobs (`AbstractCubeMob`: today the sulfur cube without a swallowed block; slimes and
//! magma cubes share it but are not registered yet): no navigation, a jumping move control
//! (`CubeMobMoveControl`: the yaw turns toward a chosen direction, a jump every 10..29 ticks on the
//! ground, a third of that when aggressive), its goals (`CubeMobFloatGoal`, the sulfur cube's
//! `SulfurCubeTemptGoal` and `SulfurCubeSearchForItemsGoal`, `CubeMobRandomDirectionGoal`,
//! `CubeMobKeepOnJumpingGoal`) and `AbstractCubeMob.tick`'s landing (its particles' and squish
//! sound's draws).

use super::goal::{JUMP, LOOK, MOVE, reduced_tick_delay};
use super::{ai, ai_mut, control};
use crate::entity::EntityKind;
use crate::entity::mob::{mob, mob_mut};
use crate::world::World;

/// A cube mob's own state (`CubeMobMoveControl`'s and `AbstractCubeMob`'s).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct CubeState {
    /// `CubeMobMoveControl.yRot`: the direction it turns to.
    pub yrot: f32,
    pub jump_delay: i32,
    pub aggressive: bool,
    /// `AbstractCubeMob.wasOnGround`.
    pub was_on_ground: bool,
    /// The cube's size (`ID_SIZE`).
    pub size: i32,
    /// `AgeableMob.age` (a baby: -24000 at its spawn, counting up).
    pub age: i32,
}

/// The cube mob types and their sizes.
pub fn cube_size(id: &str) -> Option<i32> {
    match id {
        "sulfur_cube" => Some(1),
        "sulfur_cube_2" => Some(2),
        _ => None,
    }
}

/// A cube mob's goals.
#[derive(Clone, PartialEq, Debug)]
pub enum CubeGoal {
    Float,
    Tempt,
    SearchItems,
    RandomDirection { chosen: f32, next: i32 },
    KeepOnJumping,
}

impl CubeGoal {
    pub fn flags(&self) -> u8 {
        match self {
            CubeGoal::Float | CubeGoal::KeepOnJumping => JUMP | MOVE,
            CubeGoal::Tempt | CubeGoal::SearchItems | CubeGoal::RandomDirection { .. } => LOOK,
        }
    }

    pub fn every_tick(&self) -> bool {
        matches!(self, CubeGoal::Float)
    }

    pub fn name(&self) -> &'static str {
        match self {
            CubeGoal::Float => "CubeMobFloatGoal",
            CubeGoal::Tempt => "SulfurCubeTemptGoal",
            CubeGoal::SearchItems => "SulfurCubeSearchForItemsGoal",
            CubeGoal::RandomDirection { .. } => "CubeMobRandomDirectionGoal",
            CubeGoal::KeepOnJumping => "CubeMobKeepOnJumpingGoal",
        }
    }
}

/// `SulfurCube.registerGoals` (AbstractCubeMob's, with the sulfur cube's behaviour goals and no
/// targets). The move control's direction starts at `180 * yaw / PI` (its constructor's formula,
/// with the yaw the entity's constructor drew).
pub fn register(a: &mut super::MobAi, id: &str, ctor_yaw: f32) {
    use super::goal::Goal;
    a.goals.add(1, Goal::Cube(CubeGoal::Float));
    a.goals.add(4, Goal::Cube(CubeGoal::RandomDirection { chosen: 0.0, next: 0 }));
    a.goals.add(5, Goal::Cube(CubeGoal::KeepOnJumping));
    a.goals.add(2, Goal::Cube(CubeGoal::Tempt));
    a.goals.add(3, Goal::Cube(CubeGoal::SearchItems));
    a.nav.flags.can_float = true;
    let size = cube_size(id).unwrap_or(1);
    a.cube = Some(CubeState { yrot: 180.0f32 * ctor_yaw / std::f32::consts::PI, size, age: if size == 1 { -24000 } else { 0 }, ..CubeState::default() });
}

pub fn is_cube(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ai.as_deref().is_some_and(|a| a.cube.is_some()))
}

fn cube(w: &World, i: usize) -> &CubeState {
    ai(w, i).cube.as_ref().expect("a cube mob")
}

fn cube_mut(w: &mut World, i: usize) -> &mut CubeState {
    ai_mut(w, i).cube.as_mut().expect("a cube mob")
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

fn in_liquid(w: &World, i: usize) -> bool {
    // `isInLiquid()`: in water (wasTouchingWater) or in lava (its fluid height above 0)
    let e = &w.entities.list[i];
    e.was_touching_water || e.fluid_height[1] > 0.0
}

/// `CubeMobMoveControl.setDirection`.
fn set_direction(w: &mut World, i: usize, yrot: f32, aggressive: bool) {
    let c = cube_mut(w, i);
    c.yrot = yrot;
    c.aggressive = aggressive;
}

/// `CubeMobMoveControl.setWantedMovement`.
fn set_wanted_movement(w: &mut World, i: usize, speed: f64) {
    let m = &mut ai_mut(w, i).move_ctl;
    m.speed_modifier = speed;
    m.operation = control::Operation::MoveTo;
}

pub fn can_use(w: &mut World, i: usize, g: &mut CubeGoal) -> bool {
    match g {
        CubeGoal::Float => in_liquid(w, i),
        // TemptGoal: no player holds a swallowable item (redsim's players hold nothing)
        CubeGoal::Tempt => false,
        CubeGoal::SearchItems => search_can_use(w, i),
        CubeGoal::RandomDirection { .. } => ai(w, i).target.is_none() && (w.entities.list[i].on_ground || in_liquid(w, i)),
        CubeGoal::KeepOnJumping => w.entities.list[i].vehicle.is_none(),
    }
}

/// `SulfurCubeSearchForItemsGoal.canUse`: an adult with no pickup timer looks for the nearest
/// swallowable item within 8 (a block item: chasing one is not modeled; the farms' drops are not).
fn search_can_use(w: &mut World, i: usize) -> bool {
    if mob(w, i).ty.def().id == "sulfur_cube" {
        return false; // a baby
    }
    let bb = w.entities.list[i].bb.inflate(8.0, 8.0, 8.0);
    let items = w.get_entities(bb, |e| matches!(&e.kind, EntityKind::Item(s) if !swallow_safe(&s.item)));
    if !items.is_empty() {
        let why = "a sulfur cube near an item it may swallow (SulfurCubeSearchForItemsGoal) is not modeled".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
    }
    false
}

/// Items that are not in `#sulfur_cube_swallowable` (its archetypes' blocks): the farms' drops.
fn swallow_safe(item: &str) -> bool {
    let id = item.trim_start_matches("minecraft:");
    matches!(id, "iron_ingot" | "poppy" | "rotten_flesh" | "string" | "spider_eye" | "bone" | "arrow" | "gunpowder" | "feather" | "egg" | "porkchop" | "beef" | "chicken" | "mutton" | "leather" | "carrot" | "potato" | "poisonous_potato" | "wheat" | "wheat_seeds" | "beetroot" | "beetroot_seeds" | "sugar_cane" | "copper_ingot" | "gold_ingot" | "emerald" | "bread" | "apple")
}

pub fn tick(w: &mut World, i: usize, g: &mut CubeGoal) {
    match g {
        CubeGoal::Float => {
            if own(w, i).next_float() < 0.8 {
                ai_mut(w, i).jump = true;
            }
            set_wanted_movement(w, i, 1.2);
        }
        CubeGoal::RandomDirection { chosen, next } => {
            *next -= 1;
            if *next <= 0 {
                let d = 40 + own(w, i).next_int(60);
                *next = reduced_tick_delay(d);
                *chosen = own(w, i).next_int(360) as f32;
            }
            set_direction(w, i, *chosen, false);
        }
        CubeGoal::KeepOnJumping => set_wanted_movement(w, i, 1.0),
        CubeGoal::Tempt | CubeGoal::SearchItems => {}
    }
}

/// `CubeMobMoveControl.tick()` (`SulfurCubeMobMoveControl`: the same without a body item).
pub fn move_control_tick(w: &mut World, i: usize) {
    let yrot = cube(w, i).yrot;
    let y = control::rotlerp(w.entities.list[i].y_rot, yrot, 90.0);
    w.entities.list[i].y_rot = y;
    {
        let a = ai_mut(w, i);
        a.y_head_rot = y;
        a.y_body_rot = y;
    }
    if ai(w, i).move_ctl.operation != control::Operation::MoveTo {
        mob_mut(w, i).zza = 0.0;
        return;
    }
    ai_mut(w, i).move_ctl.operation = control::Operation::Wait;
    let speed = (ai(w, i).move_ctl.speed_modifier * super::movement_speed(w, i)) as f32;
    if !w.entities.list[i].on_ground {
        let m = mob_mut(w, i);
        m.speed = speed;
        m.zza = speed;
        return;
    }
    {
        let m = mob_mut(w, i);
        m.speed = speed;
        m.zza = speed;
    }
    let delay = cube(w, i).jump_delay;
    cube_mut(w, i).jump_delay = delay - 1;
    if delay <= 0 {
        // getJumpDelay: 10..29 (a third when aggressive), the jump, its sound's pitch
        // (getSoundPitch: two floats; doPlayJumpSound: size > 0)
        let mut d = own(w, i).next_int(20) + 10;
        if cube(w, i).aggressive {
            d /= 3;
        }
        cube_mut(w, i).jump_delay = d;
        ai_mut(w, i).jump = true;
        if cube(w, i).size > 0 {
            let r = own(w, i);
            r.next_float();
            r.next_float();
        }
    } else {
        let m = mob_mut(w, i);
        m.xxa = 0.0;
        m.zza = 0.0;
        m.speed = 0.0;
    }
}

/// `AbstractCubeMob.tick()` after `super.tick()`: landing, the goo particles (`width * 2 * 16`
/// of them, two floats each) and the squish sound's pitch (two floats).
pub fn after_tick(w: &mut World, i: usize) {
    if w.entities.list[i].removed || !is_cube(w, i) {
        return;
    }
    // AgeableMob.aiStep: a baby's age counts up; at 0 it grows (setSize 2: not modeled)
    if cube(w, i).age < 0 {
        cube_mut(w, i).age += 1;
        if cube(w, i).age == 0 {
            let why = "a baby sulfur cube growing up (setSize 2) is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
    }
    let on_ground = w.entities.list[i].on_ground;
    if on_ground && !cube(w, i).was_on_ground {
        let size = mob(w, i).dims().0 * 2.0f32;
        let n = size * 16.0f32;
        let mut k = 0;
        while (k as f32) < n {
            let r = own(w, i);
            r.next_float();
            r.next_float();
            k += 1;
        }
        let r = own(w, i);
        r.next_float();
        r.next_float();
    }
    cube_mut(w, i).was_on_ground = on_ground;
}

/// `AbstractCubeMob.jumpFromGround`: the jump power, whatever the motion was (no max).
pub fn jump_from_ground(w: &mut World, i: usize, power: f32) {
    let e = &mut w.entities.list[i];
    let v = e.delta_movement;
    e.delta_movement = crate::entity::Vec3::new(v.x, power as f64, v.z);
}
