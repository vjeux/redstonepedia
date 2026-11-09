//! `LivingEntity` for mobs (ironfarm), read in the 26.3 source: `Mob.tick` = `LivingEntity.tick`
//! (`baseTick`, then `aiStep` unless removed), `LivingEntity.baseTick` (in a wall, air and
//! drowning, hurt and damage cooldowns, `tickDeath`), `Entity.baseTick`'s fire ticks,
//! `LivingEntity.aiStep` (the 0.003 floors, `applyInput`, `serverAiStep`, the jump, `travel`,
//! `applyEffectsFromBlocks`, `pushEntities`), `travel` (`travelInAir`, `travelInWater`,
//! `travelInLava`), `hurtServer` and `actuallyHurt` (the damage cooldown, armor), `die`
//! (`dropAllDeathLoot`: the loot at once, `Entity.spawnAtLocation`), `tickDeath` (removed 20 gt
//! later) and `checkFallDamage` / `causeFallDamage`.
//!
//! What a mob's AI would do is not modeled: `serverAiStep` runs the villager's brain
//! (`villager.rs`) and, for the other mobs, the walking input the judge gives them
//! ([`super::Walk`]: its robustness runs, never a guess of the game's goals).

use super::types::MobType;
use super::{Damage, MobEvent, MobState, mob, mob_mut};
use crate::block::{Block, Simple};
use crate::entity::{self, EntityKind, MoverType, Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// `LivingEntity.computeModifiedFriction(friction, modifier)` (modifiers at their default 1).
fn modified_friction(friction: f32) -> f32 {
    (1.0f32 - (1.0f32 - friction) * 1.0f32).clamp(0.0, 1.0)
}

/// `Mob.tick` of entity `i` (`LivingEntity.tick`; `updateControlFlags` changes no modeled state).
pub fn tick(w: &mut World, i: usize) {
    // ---- mob AI (the mob-AI lane): Creeper.tick's fuse before super.tick ----
    if super::ai::has_ai(w, i) && mob(w, i).ty.def().id == "creeper" {
        super::ai::goal::creeper::creeper_tick(w, i);
    }
    // LivingEntity.onClimbable (the block at its feet in BlockTags.CLIMBABLE; scaffolding is the
    // only one redsim has): climbing is not modeled, so it is refused wherever the mob came from,
    // and `isPushable` reads the flag. A spider's is its climbing flag (`Spider.onClimbable`).
    let spider = matches!(mob(w, i).ty.def().id, "spider" | "cave_spider");
    if spider {
        let c = mob(w, i).climbing;
        mob_mut(w, i).on_climbable = c;
    } else {
        let feet = w.entities.list[i].block_position();
        let climbing = matches!(w.get(feet), Block::Scaffolding { .. });
        mob_mut(w, i).on_climbable = climbing;
        if climbing {
            let why = "a mob in a climbable (scaffolding): climbing is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
    }
    base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    // ---- mob AI (the mob-AI lane): the rest of LivingEntity.baseTick and Mob.baseTick ----
    if super::ai::has_ai(w, i) {
        super::ai::after_base_tick(w, i);
    }
    // LivingEntity.tick: a sleeper whose bed is gone wakes up
    if let Some(bed) = mob(w, i).sleeping_pos
        && !matches!(w.get(bed), Block::Bed { .. })
    {
        super::villager::stop_sleeping(w, i);
    }
    if !w.entities.list[i].removed {
        ai_step(w, i);
    }
    // ---- mob AI (the mob-AI lane): tickHeadTurn, Mob.tick's updateControlFlags ----
    if super::ai::has_ai(w, i) {
        super::ai::after_ai_step(w, i);
    }
    // Spider.tick after super.tick: climbing while it collides horizontally
    if spider {
        super::ai::goal::spider::after_tick(w, i);
    }
    // Bat.tick after super.tick: still at its ceiling, or 0.6 of its vertical motion
    if super::ai::has_ai(w, i) && super::ai::bat::is_bat(w, i) {
        super::ai::bat::after_tick(w, i);
    }
    // AbstractCubeMob.tick after super.tick: the landing's draws
    if super::ai::has_ai(w, i) {
        super::ai::cube::after_tick(w, i);
    }
}

/// `LivingEntity.baseTick` (with `Entity.baseTick` inside).
fn base_tick(w: &mut World, i: usize) {
    // firstTick: a sleeper is put back on its bed
    if w.entities.list[i].first_tick
        && let Some(bed) = mob(w, i).sleeping_pos
    {
        if matches!(w.get(bed), Block::Bed { .. }) {
            super::villager::set_pos_to_bed(w, i, bed);
        } else {
            super::villager::stop_sleeping(w, i);
        }
    }
    let was_in_lava = mob(w, i).lava_height > 0.0;
    // ---- mob AI (the mob-AI lane): canSpawnSprintParticle's draw (iron golems) ----
    if super::ai::has_ai(w, i) {
        super::ai::before_base_tick(w, i);
    }
    // Entity.baseTick: computeSpeed, wasEyeInWater, updateFluidInteraction (water current 0.014),
    // checkBelowWorld, firstTick
    entity::base_tick(w, i);
    update_lava(w, i);
    if w.entities.list[i].removed {
        return;
    }
    fire_tick(w, i);
    if mob(w, i).lava_height > 0.0 {
        // Entity.baseTick: `if (this.isInLava()) this.fallDistance *= 0.5`
        w.entities.list[i].fall_distance *= 0.5;
    }
    let _ = was_in_lava;
    let ty = mob(w, i).ty;
    if !mob(w, i).dead {
        if is_in_wall(w, i) {
            hurt(w, i, Damage::InWall, 1.0);
        }
        air_tick(w, i, ty);
    }
    let m = mob_mut(w, i);
    if m.hurt_time > 0 {
        m.hurt_time -= 1;
    }
    if m.damage_cooldown > 0 {
        m.damage_cooldown -= 1;
    }
    if m.health <= 0.0 {
        tick_death(w, i);
    }
}

/// `Entity.baseTick`'s fire: a fire-immune mob is put out; else every 20th fire tick out of lava
/// hurts 1 (`onFire`), and the fire burns down by one.
fn fire_tick(w: &mut World, i: usize) {
    let (fire, immune, in_lava) = {
        let m = mob(w, i);
        (m.fire_ticks, m.ty.def().fire_immune, m.lava_height > 0.0)
    };
    if fire > 0 {
        if immune {
            mob_mut(w, i).fire_ticks = fire.min(0);
        } else {
            if fire % 20 == 0 && !in_lava {
                hurt(w, i, Damage::OnFire, 1.0);
            }
            mob_mut(w, i).fire_ticks -= 1;
        }
    }
}

/// `LivingEntity.baseTick`'s air: the eye in water (not a bubble column) lowers the air
/// (`decreaseAirSupply`: by 1; an iron golem's keeps it), 2 drowning damage at -20 (then 0);
/// otherwise it refills by 4 up to 300.
fn air_tick(w: &mut World, i: usize, ty: MobType) {
    let e = &w.entities.list[i];
    let eye_cell = BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y + mob(w, i).eye() as f64), mth::floor(e.pos.z));
    let eye_in_water = e.fluid.eye_in_water() && !matches!(w.get(eye_cell), Block::BubbleColumn { .. });
    if eye_in_water {
        if ty.def().drowns {
            let m = mob_mut(w, i);
            m.air -= 1;
            if m.air <= -20 {
                m.air = 0;
                hurt(w, i, Damage::Drown, 2.0);
            }
        }
    } else {
        let m = mob_mut(w, i);
        if m.air < 300 {
            m.air = (m.air + 4).min(300);
        }
    }
}

/// `Entity.isInWall` (not while sleeping): a suffocating block's collision shape meets the box of
/// `width * 0.8` around the eye, 1e-6 tall.
fn is_in_wall(w: &World, i: usize) -> bool {
    let e = &w.entities.list[i];
    let m = mob(w, i);
    if m.sleeping_pos.is_some() || e.no_physics {
        return false;
    }
    let half = (m.dims().0 * 0.8f32) as f64 / 2.0;
    let eye = e.pos.y + m.eye() as f64;
    let bb = entity::Aabb::new(e.pos.x - half, eye - 5.0e-7, e.pos.z - half, e.pos.x + half, eye + 5.0e-7, e.pos.z + half);
    for y in mth::floor(bb.min_y)..=mth::floor(bb.max_y) {
        for z in mth::floor(bb.min_z)..=mth::floor(bb.max_z) {
            for x in mth::floor(bb.min_x)..=mth::floor(bb.max_x) {
                let p = BlockPos::new(x, y, z);
                let s = w.get(p);
                if !s.is_air() && super::blocks::is_suffocating(&s) && entity::collide::boxes_at(w, &s, p).iter().any(|b| b.intersects(bb)) {
                    return true;
                }
            }
        }
    }
    false
}

/// `LivingEntity.tickDeath`: removed 20 gt after death.
fn tick_death(w: &mut World, i: usize) {
    let m = mob_mut(w, i);
    m.death_time += 1;
    if m.death_time >= 20 && !w.entities.list[i].removed {
        let id = w.entities.list[i].id;
        let t = w.time;
        w.entities.list[i].removed = true;
        w.mob.events.push(MobEvent::Removed { t, id });
    }
}

/// The lava tracker (`EntityFluidInteraction` for the LAVA tag): the highest lava top over the
/// box's bottom, from the cells the fluid box meets (deflated 0.001). Only lava sources exist here
/// (no current: a sealed source has no flow).
pub fn update_lava(w: &mut World, i: usize) {
    let e = &w.entities.list[i];
    let bx = e.bb.deflate(0.001, 0.001, 0.001);
    let ceil = |v: f64| v.ceil() as i32;
    let mut h = 0.0f64;
    for x in mth::floor(bx.min_x)..ceil(bx.max_x) {
        for y in mth::floor(bx.min_y)..ceil(bx.max_y) {
            for z in mth::floor(bx.min_z)..ceil(bx.max_z) {
                let p = BlockPos::new(x, y, z);
                if let Some(lh) = super::blocks::lava_height(w, p) {
                    let top = y as f64 + lh;
                    if top >= bx.min_y {
                        h = crate::entity::vec3::jmax(top - e.bb.min_y, h);
                    }
                }
            }
        }
    }
    mob_mut(w, i).lava_height = h;
}

/// `LivingEntity.aiStep` (and `Mob.aiStep`'s tail).
fn ai_step(w: &mut World, i: usize) {
    // ---- mob AI (the mob-AI lane): Monster.aiStep's updateNoActionTime first ----
    if super::ai::has_ai(w, i) {
        super::ai::update_no_action_time(w, i);
    }
    {
        let m = mob_mut(w, i);
        if m.no_jump_delay > 0 {
            m.no_jump_delay -= 1;
        }
    }
    {
        let e = &mut w.entities.list[i];
        let v = e.delta_movement;
        let dx = if v.x.abs() < 0.003 { 0.0 } else { v.x };
        let dy = if v.y.abs() < 0.003 { 0.0 } else { v.y };
        let dz = if v.z.abs() < 0.003 { 0.0 } else { v.z };
        e.delta_movement = Vec3::new(dx, dy, dz);
    }
    // applyInput
    {
        let m = mob_mut(w, i);
        m.xxa *= 0.98;
        m.zza *= 0.98;
    }
    if mob(w, i).dead {
        // isImmobile (dead or dying): no AI
        let m = mob_mut(w, i);
        m.jumping = false;
        m.xxa = 0.0;
        m.zza = 0.0;
    } else {
        server_ai_step(w, i);
        if w.entities.list[i].removed {
            return;
        }
    }
    jump(w, i);
    let input = {
        let m = mob(w, i);
        Vec3::new(m.xxa as f64, 0.0, m.zza as f64)
    };
    // `getControllingPassenger() instanceof Player` never holds here; else `canSimulateMovement()
    // && isEffectiveAi()`, both true for a server-side mob, riders included (`rideTick` zeroed its
    // motion; its vehicle puts it back on its seat after this tick)
    travel(w, i, input);
    // mobinfra: the fire ticks before the inside-block effects (`applyEffectsFromBlocks`'
    // `previousRemainingFireTicks`); a fire's cell sets `inside.fire` like any block's
    let fire_before = mob(w, i).fire_ticks;
    entity::apply_effects_from_blocks(w, i);
    apply_block_effects(w, i);
    // mobinfra: rain puts the fire out, then the not-burning reset (`applyEffectsFromBlocks`' tail)
    crate::weather::after_block_effects(w, i, fire_before);
    if w.entities.list[i].removed {
        return;
    }
    push_entities(w, i);
    // Mob.aiStep: burnUndead after LivingEntity.aiStep (mobinfra: zombies in daylight)
    super::sun::burn_undead(w, i);
}

/// `Mob.serverAiStep`: `noActionTime`, then (`customServerAiStep`) the villager's brain; the
/// walking input of a mob the judge drives ([`super::Walk`]).
fn server_ai_step(w: &mut World, i: usize) {
    mob_mut(w, i).no_action_time += 1;
    // ---- mob AI (the mob-AI lane): Mob.serverAiStep's selectors, navigation and controls ----
    if super::ai::has_ai(w, i) {
        super::ai::server_ai_step(w, i);
        return;
    }
    match mob(w, i).ty {
        MobType::Villager => super::villager::brain_tick(w, i),
        _ => super::walk::apply(w, i),
    }
}

/// `LivingEntity.aiStep`'s jump: from the ground (no jump delay), or in a liquid.
fn jump(w: &mut World, i: usize) {
    if !mob(w, i).jumping {
        mob_mut(w, i).no_jump_delay = 0;
        return;
    }
    let (in_lava, lava_h) = (mob(w, i).lava_height > 0.0, mob(w, i).lava_height);
    let e = &w.entities.list[i];
    let water_h = e.fluid.water_height();
    let fluid_h = if in_lava { lava_h } else { water_h };
    let in_water_with_height = e.fluid.in_water() && fluid_h > 0.0;
    let threshold = fluid_jump_threshold(mob(w, i));
    let on_ground = e.on_ground;
    if !in_water_with_height || (on_ground && !(fluid_h > threshold)) {
        if !in_lava || (on_ground && lava_h <= threshold) {
            if (on_ground || (in_water_with_height && fluid_h <= threshold)) && mob(w, i).no_jump_delay == 0 {
                jump_from_ground(w, i);
                mob_mut(w, i).no_jump_delay = 10;
            }
        } else {
            jump_in_liquid(w, i);
        }
    } else {
        jump_in_liquid(w, i);
    }
}

/// `Entity.getFluidJumpThreshold`: 0.4 if the eye is under 0.4 high, else 0.4.
fn fluid_jump_threshold(m: &MobState) -> f64 {
    if (m.eye() as f64) < 0.4 { 0.0 } else { 0.4 }
}

/// `LivingEntity.jumpFromGround`: up at the jump power (0.42, nobody sprints).
fn jump_from_ground(w: &mut World, i: usize) {
    let power = 0.42f32 * jump_factor(w, i);
    // AbstractCubeMob.jumpFromGround: the power whatever the motion (no threshold, no max)
    if super::ai::has_ai(w, i) && super::ai::cube::is_cube(w, i) {
        super::ai::cube::jump_from_ground(w, i, power);
        return;
    }
    if power > 1.0e-5 {
        let e = &mut w.entities.list[i];
        let v = e.delta_movement;
        e.delta_movement = Vec3::new(v.x, crate::entity::vec3::jmax(power as f64, v.y), v.z);
    }
}

/// `Entity.getBlockJumpFactor` (honey 0.5, else 1).
fn jump_factor(w: &World, i: usize) -> f32 {
    let e = &w.entities.list[i];
    let at = w.get(e.block_position());
    let below = w.get(entity::collide::get_on_pos(w, e, 0.500001));
    let f = |b: &Block| if matches!(b, Block::Simple(Simple::HoneyBlock)) { 0.5f32 } else { 1.0 };
    let here = f(&at);
    if here == 1.0 { f(&below) } else { here }
}

/// `LivingEntity.jumpInLiquid`: +0.04 up (`Mob.jumpInLiquid`: +0.3 for a mob whose navigation
/// cannot float; applied with the mob AI on).
fn jump_in_liquid(w: &mut World, i: usize) {
    let up = if super::ai::has_ai(w, i) && !super::ai::ai(w, i).nav.flags.can_float { 0.3 } else { 0.04f32 as f64 };
    let e = &mut w.entities.list[i];
    e.delta_movement = e.delta_movement.add(0.0, up, 0.0);
}

/// `LivingEntity.travel(input)`: in a liquid (`isInLiquid`, affected by fluids, not standing on
/// it), else in the air.
pub fn travel(w: &mut World, i: usize, input: Vec3) {
    let in_water = w.entities.list[i].fluid.in_water();
    let in_lava = mob(w, i).lava_height > 0.0;
    if in_water || in_lava {
        let e = &w.entities.list[i];
        let falling = e.delta_movement.y <= 0.0;
        let old_y = e.pos.y;
        let gravity = 0.08;
        if in_water {
            travel_in_water(w, i, input, gravity, falling, old_y);
        } else {
            travel_in_lava(w, i, input, gravity, falling, old_y);
        }
    } else {
        travel_in_air(w, i, input);
    }
}

/// `Entity.getInputVector(input, speed, yRot)` added to the motion (`moveRelative`).
fn move_relative(w: &mut World, i: usize, speed: f32, input: Vec3) {
    let len = input.length_sqr();
    if len < 1.0e-7 {
        return;
    }
    let mv = if len > 1.0 { input.normalize() } else { input }.scale(speed as f64);
    let y_rot = w.entities.list[i].y_rot;
    let s = mth::sin((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
    let c = mth::cos((y_rot * (std::f32::consts::PI / 180.0f32)) as f64);
    let d = Vec3::new(mv.x * c as f64 - mv.z * s as f64, mv.y, mv.z * c as f64 + mv.x * s as f64);
    let e = &mut w.entities.list[i];
    e.delta_movement = e.delta_movement.add_v(d);
}

/// `LivingEntity.travelInAir`: block friction on the ground, the input at the friction-scaled
/// speed (0.02 in the air), `move`, gravity after the move, drag 0.91 x friction / 0.98.
fn travel_in_air(w: &mut World, i: usize, input: Vec3) {
    let on_ground = w.entities.list[i].on_ground;
    let block_friction = if on_ground {
        let below = entity::collide::get_on_pos(w, &w.entities.list[i], 0.500001);
        modified_friction(super::blocks::friction(&w.get(below)))
    } else {
        1.0
    };
    let speed = if on_ground {
        let s = mob(w, i).speed;
        if block_friction as f64 > 0.6 { s * (0.21600002f32 / (block_friction * block_friction * block_friction)) } else { s }
    } else {
        0.02f32
    };
    move_relative(w, i, speed, input);
    // handleOnClimbable: a climbing mob (a spider on a wall) falls no faster than 0.15 and moves
    // at most 0.15 sideways; its fall distance resets
    let climbing = mob(w, i).on_climbable;
    if climbing {
        let c = 0.15f32 as f64;
        let e = &mut w.entities.list[i];
        e.fall_distance = 0.0;
        let d = e.delta_movement;
        e.delta_movement = Vec3::new(d.x.clamp(-c, c), d.y.max(-c), d.z.clamp(-c, c));
    }
    let d = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, d);
    let mut movement = w.entities.list[i].delta_movement;
    // and goes up 0.2 while it pushes against a wall (or jumps)
    if climbing && (w.entities.list[i].horizontal_collision || mob(w, i).jumping) {
        movement = Vec3::new(movement.x, 0.2, movement.z);
    }
    let movement_y = movement.y - 0.08;
    let friction = block_friction * modified_friction(0.91);
    let vertical = modified_friction(0.98);
    w.entities.list[i].delta_movement = Vec3::new(movement.x * friction as f64, movement_y * vertical as f64, movement.z * friction as f64);
}

/// `LivingEntity.getFluidFallingAdjustedMovement`.
fn fluid_falling_adjusted(gravity: f64, falling: bool, m: Vec3) -> Vec3 {
    if gravity != 0.0 {
        let yd = if falling && (m.y - 0.005).abs() >= 0.003 && (m.y - gravity / 16.0).abs() < 0.003 { -0.003 } else { m.y - gravity / 16.0 };
        Vec3::new(m.x, yd, m.z)
    } else {
        m
    }
}

/// `LivingEntity.travelInWater` (no water walker, no dolphin's grace, nobody sprints).
fn travel_in_water(w: &mut World, i: usize, input: Vec3, gravity: f64, falling: bool, old_y: f64) {
    let slow = 0.8f32;
    move_relative(w, i, 0.02, input);
    let d = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, d);
    let mut m = w.entities.list[i].delta_movement;
    // a climbing mob against a wall goes up 0.2 (a spider in water)
    if w.entities.list[i].horizontal_collision && mob(w, i).on_climbable {
        m = Vec3::new(m.x, 0.2, m.z);
    }
    let m = m.multiply(slow as f64, 0.8f32 as f64, slow as f64);
    w.entities.list[i].delta_movement = fluid_falling_adjusted(gravity, falling, m);
    jump_out_of_fluid(w, i, old_y);
}

/// `LivingEntity.travelInLava`.
fn travel_in_lava(w: &mut World, i: usize, input: Vec3, gravity: f64, falling: bool, old_y: f64) {
    move_relative(w, i, 0.02, input);
    let d = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, d);
    let threshold = fluid_jump_threshold(mob(w, i));
    if mob(w, i).lava_height <= threshold {
        let m = w.entities.list[i].delta_movement.multiply(0.5, 0.8f32 as f64, 0.5);
        w.entities.list[i].delta_movement = fluid_falling_adjusted(gravity, falling, m);
    } else {
        let m = w.entities.list[i].delta_movement.scale(0.5);
        w.entities.list[i].delta_movement = m;
    }
    if gravity != 0.0 {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, -gravity / 4.0, 0.0);
    }
    jump_out_of_fluid(w, i, old_y);
}

/// `LivingEntity.jumpOutOfFluid`: a horizontal collision with room 0.6 higher gives 0.3 up.
fn jump_out_of_fluid(w: &mut World, i: usize, old_y: f64) {
    let e = &w.entities.list[i];
    let m = e.delta_movement;
    if e.horizontal_collision {
        let bb = e.bb.move_by(m.x, m.y + 0.6f32 as f64 - e.pos.y + old_y, m.z);
        if entity::no_collision(w, i, bb) && !super::blocks::box_has_liquid(w, bb) {
            w.entities.list[i].delta_movement = Vec3::new(m.x, 0.3f32 as f64, m.z);
        }
    }
}

/// The block effects `checkInsideBlocks` collected this tick ([`entity_inside`]), applied in the
/// `InsideBlockEffectType` order: LAVA_IGNITE (15 s of fire) with `lavaHurt` after it (4), then
/// EXTINGUISH (water puts the fire out).
fn apply_block_effects(w: &mut World, i: usize) {
    let fx = std::mem::take(&mut mob_mut(w, i).inside);
    // `StepBasedCollector.applyAndClear`: every effect runs only while the entity is alive (a
    // lethal lavaHurt leaves the water's EXTINGUISH out: the dead golem keeps its 300 fire ticks)
    let alive = |w: &World, i: usize| !w.entities.list[i].removed && mob(w, i).health > 0.0;
    if fx.fire && alive(w, i) {
        // mobinfra: FIRE_IGNITE (`BaseFireBlock.fireIgnite`: -1 counts up to 0, from 0 8 s of
        // fire) then its 1 `inFire` damage, before LAVA_IGNITE
        let m = mob_mut(w, i);
        if !m.ty.def().fire_immune {
            if m.fire_ticks < 0 {
                m.fire_ticks += 1;
            }
            if m.fire_ticks >= 0 && m.fire_ticks < 160 {
                m.fire_ticks = 160;
            }
        }
        if alive(w, i) {
            hurt(w, i, Damage::InFire, 1.0);
        }
    }
    if fx.lava && alive(w, i) {
        let m = mob_mut(w, i);
        if !m.ty.def().fire_immune {
            // igniteForTicks(300) then lavaHurt: a hurt that lands plays GENERIC_BURN, its pitch
            // from the entity's own random (`2.0F + random.nextFloat() * 0.4F`)
            if m.fire_ticks < 300 {
                m.fire_ticks = 300;
            }
            if alive(w, i)
                && hurt(w, i, Damage::Lava, 4.0)
                && let Some(r) = w.entities.list[i].random.as_mut()
            {
                r.next_float();
            }
        }
    }
    if fx.water && alive(w, i) {
        let m = mob_mut(w, i);
        m.fire_ticks = m.fire_ticks.min(0);
    }
}

/// `entityInside` of the blocks the mob's sweep meets (`react::entity_inside` calls it for a mob):
/// lava (`LavaFluid.entityInside`, when the box meets the lava's fluid box) and water
/// (`WaterFluid.entityInside`: EXTINGUISH).
pub fn entity_inside(w: &mut World, i: usize, s: Block, p: BlockPos) {
    match s {
        Block::Lava { .. } => {
            let h = super::blocks::lava_height(w, p).unwrap_or(0.0);
            let lava = entity::Aabb::new(p.x as f64, p.y as f64, p.z as f64, (p.x + 1) as f64, p.y as f64 + h, (p.z + 1) as f64);
            if w.entities.list[i].bb.intersects(lava) {
                mob_mut(w, i).inside.lava = true;
            }
        }
        Block::Water { .. } | Block::BubbleColumn { .. } | Block::WaterloggedGrate | Block::Kelp { .. } | Block::KelpPlant => {
            mob_mut(w, i).inside.water = true;
        }
        // mobinfra: a fire's cell (`BaseFireBlock.entityInside`: the full cell)
        Block::Fire { .. } => {
            mob_mut(w, i).inside.fire = true;
        }
        _ => {}
    }
}

/// `MagmaBlock.stepOn`: 1 hot-floor damage to a mob on it (not sneaking, not fire immune, no
/// frost walker).
pub fn step_on(w: &mut World, i: usize, s: Block) {
    if matches!(s, Block::Simple(Simple::MagmaBlock)) && !mob(w, i).ty.def().fire_immune {
        hurt(w, i, Damage::HotFloor, 1.0);
    }
}

/// `LivingEntity.hurtServer(level, source, damage)` for the modeled sources (no attacker, no
/// shield, no knockback: none of them has a source position). True when it took damage.
pub fn hurt(w: &mut World, i: usize, src: Damage, amount: f32) -> bool {
    if w.entities.list[i].removed {
        return false;
    }
    let ty = mob(w, i).ty;
    let def = ty.def();
    // isInvulnerableTo: fire immunity, fall immunity
    if (src.is_fire() && def.fire_immune) || (src == Damage::Fall && def.fall_immune) {
        return false;
    }
    if mob(w, i).health <= 0.0 {
        return false;
    }
    if mob(w, i).sleeping_pos.is_some() {
        super::villager::stop_sleeping(w, i);
    }
    mob_mut(w, i).no_action_time = 0;
    let damage = amount.max(0.0);
    let took_full;
    {
        let m = mob(w, i);
        if m.damage_cooldown as f32 > 10.0 {
            if damage <= m.last_hurt {
                return false;
            }
            let d = damage - m.last_hurt;
            actually_hurt(w, i, src, d);
            mob_mut(w, i).last_hurt = damage;
            took_full = false;
        } else {
            let m = mob_mut(w, i);
            m.last_hurt = damage;
            m.damage_cooldown = 20;
            actually_hurt(w, i, src, damage);
            let m = mob_mut(w, i);
            m.hurt_time = 10;
            took_full = true;
        }
    }
    let _ = took_full;
    let t = w.time;
    let id = w.entities.list[i].id;
    w.mob.events.push(MobEvent::Hurt { t, id, ty, src, amount: damage });
    mob_mut(w, i).last_damage = Some((t, src));
    // ---- mob AI (the mob-AI lane): the attacker, the knockback, the sounds' pitch draws ----
    super::ai::attack::after_hurt(w, i, src, took_full);
    if mob(w, i).health <= 0.0 {
        die(w, i, src);
    }
    true
}

/// `LivingEntity.actuallyHurt`: armor (`CombatRules.getDamageAfterAbsorb`, unless the source
/// bypasses armor), then the health.
fn actually_hurt(w: &mut World, i: usize, src: Damage, mut dmg: f32) {
    let armor = mob(w, i).ty.def().armor;
    if !src.bypasses_armor() && armor > 0.0 {
        let toughness = 2.0f32;
        let real = (armor - dmg / toughness).clamp(armor * 0.2f32, 20.0f32);
        dmg *= 1.0f32 - real / 25.0f32;
    }
    if dmg != 0.0 {
        // setHealth: clamped to [0, max]
        let max = mob(w, i).ty.def().max_health;
        let m = mob_mut(w, i);
        m.health = (m.health - dmg).clamp(0.0, max);
    }
}

/// `LivingEntity.die`: dead; a sleeper wakes; the loot drops at once (`dropAllDeathLoot`: no
/// player kill, so no experience); pose DYING (the body stays 20 gt: `tickDeath`).
fn die(w: &mut World, i: usize, src: Damage) {
    if w.entities.list[i].removed || mob(w, i).dead {
        return;
    }
    if mob(w, i).sleeping_pos.is_some() {
        super::villager::stop_sleeping(w, i);
    }
    mob_mut(w, i).dead = true;
    let (t, id, ty, pos) = {
        let e = &w.entities.list[i];
        (w.time, e.id, mob(w, i).ty, e.pos)
    };
    w.mob.events.push(MobEvent::Death { t, id, ty, src, pos: [pos.x, pos.y, pos.z] });
    // ---- mob AI (the mob-AI lane): the killer's `killedEntity` (a zombie's villager conversion) ----
    if w.mob.ai.on {
        super::ai::attack::killed_entity(w, i, src);
    }
    w.game_event_at("entity_die", [pos.x, pos.y, pos.z], None, false);
    drop_loot(w, i);
}

/// `dropFromLootTable`: each pool's count (`set_count uniform`), each non-empty stack an item at
/// the mob's position (`Entity.spawnAtLocation`: `new ItemEntity(level, x, y, z, stack)` with the
/// item's own random motion (+-0.1, 0.2, +-0.1) and yaw, pickup delay 10). The loot random and
/// the item's own random are the seeded stand-ins (`MobWorld::rng`).
fn drop_loot(w: &mut World, i: usize) {
    if w.exact_seed.is_some() && w.loot_salt.is_some() {
        return drop_loot_exact(w, i);
    }
    let (pos, ty) = (w.entities.list[i].pos, mob(w, i).ty);
    let src_id = w.entities.list[i].id;
    for &(item, lo, hi) in ty.def().loot {
        let n = {
            let r = w.mob.rng_mut();
            lo + r.next_int(hi - lo + 1)
        };
        if n <= 0 {
            continue;
        }
        let (mx, mz, y_rot, own) = {
            let r = w.mob.rng_mut();
            let seed = r.next_long();
            let mut own = crate::rng::LegacyRandom::new(seed);
            // ItemEntity(EntityType, Level): bobOffs, yRot; then the motion
            own.next_float();
            let y_rot = own.next_float() * 360.0;
            let mx = own.next_double() * 0.2 - 0.1;
            let mz = own.next_double() * 0.2 - 0.1;
            (mx, mz, y_rot, own)
        };
        let state = crate::entity::item::ItemState { item: item.to_string(), count: n as u8, pickup_delay: 10, age: 0, payload: 0, health: 5 };
        let id = w.entities.add(EntityKind::Item(state), Vec3::new(pos.x, pos.y, pos.z), y_rot);
        if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
            e.delta_movement = Vec3::new(mx, 0.2, mz);
            e.random = Some(own);
        }
        let t = w.time;
        w.mob.events.push(MobEvent::Drop { t, from: src_id, item: id, what: item, count: n as u32 });
    }
}

/// [`drop_loot`] in an exact world (tickexact): `LivingEntity.dropFromLootTable` with the table's
/// random sequence (`getLootTableSeed()` is 0: `level.getRandomSequence(entities/<id>)`, the
/// bridge's sandboxes salt them from their seed: `World::loot_salt`), each pool's `set_count
/// uniform(lo, hi)` (`Mth.nextInt(random, lo, hi)`; the other pools' conditions, a killer's
/// looting and a zombie horse, draw nothing), and each non-empty stack through
/// `spawnAtLocation` (`new ItemEntity(level, x, y, z, stack)`): the item enters with its own random
/// seeded as its entity number, the constructor's UUID, bobOffs, yaw and drift replayed (the
/// drift kept: x and z, 0.2 up).
fn drop_loot_exact(w: &mut World, i: usize) {
    let (pos, ty) = (w.entities.list[i].pos, mob(w, i).ty);
    let src_id = w.entities.list[i].id;
    let key = format!("minecraft:entities/{}", ty.def().id);
    let salt = w.loot_salt.unwrap_or(0);
    for &(item, lo, hi) in ty.def().loot {
        let n = {
            let r = w.loot_sequences.entry(key.clone()).or_insert_with(|| crate::stems::LootSequence::new(salt, &key));
            if lo >= hi { lo } else { lo + r.next_int(hi - lo + 1) }
        };
        if n <= 0 {
            continue;
        }
        let at = w.entities.next_id;
        let mut own = w.entering_random(at);
        crate::rng::replay_uuid(&mut own);
        own.next_float();
        let y_rot = own.next_float() * 360.0;
        let mx = own.next_double() * 0.2 - 0.1;
        let mz = own.next_double() * 0.2 - 0.1;
        let state = crate::entity::item::ItemState { item: item.to_string(), count: n as u8, pickup_delay: 10, age: 0, payload: 0, health: 5 };
        let id = w.entities.add(EntityKind::Item(state), Vec3::new(pos.x, pos.y, pos.z), y_rot);
        if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
            e.delta_movement = Vec3::new(mx, 0.2, mz);
            e.random = Some(own);
        }
        let t = w.time;
        w.mob.events.push(MobEvent::Drop { t, from: src_id, item: id, what: item, count: n as u32 });
    }
}

/// `LivingEntity.pushEntities`: the pushable entities whose box meets this one's (alive mobs;
/// boats and minecarts are a model error here), the cramming rule (more than `maxEntityCramming -
/// 1` non-passengers: 6 damage on `random.nextInt(4) == 0`), then `doPush` each (`Entity.push`).
fn push_entities(w: &mut World, i: usize) {
    // a bat pushes nothing (`Bat.pushEntities`)
    if mob(w, i).ty.def().id == "bat" {
        return;
    }
    let bb = w.entities.list[i].bb;
    let me = w.entities.list[i].id;
    // EntitySelector.pushableBy(this): not the mob itself, not anything riding the same root
    // vehicle (isPassengerOfSameVehicle: a zombie never pushes its own boat)
    let root = root_vehicle(w, me);
    let others = w.get_entities(bb, |e| e.id != me && pushable(e) && root_vehicle(w, e.id) != root);
    if others.is_empty() {
        return;
    }
    let cram = w.mob.cramming;
    // the cramming roll is the mob's own random (the stand-in without the mob AI's seeding)
    let roll = |w: &mut World| if super::ai::has_ai(w, i) { w.entities.list[i].random.as_mut().expect("random").next_int(4) } else { w.mob.rng_mut().next_int(4) };
    if cram > 0 && others.len() as i32 > cram - 1 && roll(w) == 0 {
        let count = others.iter().filter(|&&j| w.entities.list[j].vehicle.is_none()).count() as i32;
        if count > cram - 1 {
            hurt(w, i, Damage::Cramming, 6.0);
        }
    }
    for j in others {
        if !matches!(w.entities.list[j].kind, EntityKind::Mob(_)) {
            let why = format!("a mob touching a {} (its push) is not modeled", super::kind_name(&w.entities.list[j].kind));
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            continue;
        }
        if super::ai::has_ai(w, i) {
            super::ai::before_do_push(w, i, j);
        }
        push_pair(w, j, i);
    }
}

/// `Entity.getRootVehicle()`'s id: follow the vehicles up from entity `id`.
fn root_vehicle(w: &World, id: u32) -> u32 {
    let mut cur = id;
    for _ in 0..8 {
        match w.entities.list.iter().find(|e| e.id == cur).and_then(|e| e.vehicle) {
            Some(v) => cur = v,
            None => break,
        }
    }
    cur
}

/// `Entity.isPushable` of the entities a mob can push.
fn pushable(e: &entity::Entity) -> bool {
    match &e.kind {
        // a bat is never pushed (`Bat.isPushable`)
        EntityKind::Mob(m) => !m.dead && m.ty.def().id != "bat",
        EntityKind::Boat(_) | EntityKind::Minecart(_) => true,
        _ => false,
    }
}

/// `Entity.push(Entity entity)` with `this` = `j` (the other) and `entity` = `i` (the pusher):
/// both pushed apart by 0.05 / sqrt(distance) along their horizontal offset.
fn push_pair(w: &mut World, j: usize, i: usize) {
    let (a, b) = (w.entities.list[j].pos, w.entities.list[i].pos);
    if w.entities.list[j].vehicle.is_some() && w.entities.list[j].vehicle == w.entities.list[i].vehicle {
        return;
    }
    let mut xa = b.x - a.x;
    let mut za = b.z - a.z;
    let mut dd = xa.abs().max(za.abs());
    if dd >= 0.01f32 as f64 {
        dd = dd.sqrt();
        xa /= dd;
        za /= dd;
        let pow = (1.0 / dd).min(1.0);
        xa *= pow;
        za *= pow;
        xa *= 0.05f32 as f64;
        za *= 0.05f32 as f64;
        if w.entities.list[j].passengers.is_empty() {
            let e = &mut w.entities.list[j];
            e.delta_movement = e.delta_movement.add(-xa, 0.0, -za);
        }
        if w.entities.list[i].passengers.is_empty() {
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.add(xa, 0.0, za);
        }
    }
}

/// `LivingEntity.checkFallDamage` (the kinds hook): the fluid interaction again when not in water,
/// then `Entity.checkFallDamage` (the fall distance grows while falling out of water; on landing
/// `Block.fallOn` = `causeFallDamage(distance, 1, fall)`, a bed halves it; HIT_GROUND; reset).
pub fn check_fall_damage(w: &mut World, i: usize, ya: f64, on_ground: bool) {
    // a bat neither falls nor counts its fall (`Bat.checkFallDamage` does nothing)
    if mob(w, i).ty.def().id == "bat" {
        return;
    }
    if !w.entities.list[i].was_touching_water {
        crate::entity::fluid_push::update_fluid_interaction(w, i);
    }
    let landed = {
        let e = &mut w.entities.list[i];
        if !e.was_touching_water && ya < 0.0 {
            e.fall_distance -= ya as f32 as f64;
        }
        on_ground && e.fall_distance > 0.0
    };
    if landed {
        let on = entity::collide::get_on_pos(w, &w.entities.list[i], 0.2);
        let on_state = w.get(on);
        let distance = w.entities.list[i].fall_distance;
        let distance = if matches!(on_state, Block::Bed { .. }) { distance * 0.5 } else { distance };
        if !matches!(on_state, Block::Simple(Simple::SlimeBlock)) {
            cause_fall_damage(w, i, distance);
        }
        let e = &w.entities.list[i];
        let at = [e.pos.x, e.pos.y, e.pos.z];
        let affected = e.main_support.map(|p| w.get(p)).unwrap_or(on_state);
        w.game_event_at("hit_ground", at, Some(affected), false);
    }
    if on_ground {
        w.entities.list[i].fall_distance = 0.0;
    }
}

/// `LivingEntity.causeFallDamage(distance, 1, fall)`: `floor(distance + 1e-6 - 3)` damage unless
/// immune (`calculateFallDamage`).
fn cause_fall_damage(w: &mut World, i: usize, distance: f64) {
    if mob(w, i).ty.def().fall_immune {
        return;
    }
    let dmg = mth::floor(distance + 1.0e-6 - 3.0);
    if dmg > 0 {
        hurt(w, i, Damage::Fall, dmg as f32);
    }
}
