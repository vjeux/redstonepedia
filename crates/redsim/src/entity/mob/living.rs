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

/// `onClimbable()` of mob `i` as it stands now: a spider's climbing flag (`Spider.onClimbable`);
/// else `LivingEntity.onClimbable`, the block at its feet in #minecraft:climbable (a natural ladder
/// or vine; scaffolding, whose climbing is not modeled, is refused; a trapdoor used as a ladder
/// is not modeled either).
pub(crate) fn climbable_now(w: &mut World, i: usize) -> bool {
    if matches!(mob(w, i).ty.def().id, "spider" | "cave_spider") {
        return mob(w, i).climbing;
    }
    let feet = w.entities.list[i].block_position();
    match w.get(feet) {
        Block::Scaffolding { .. } => {
            let why = "a mob in a climbable (scaffolding): climbing is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            true
        }
        Block::Terrain(id) => crate::terrain::blocks::props(id).has_tag("minecraft:climbable"),
        _ => false,
    }
}

/// `Mob.tick` of entity `i` (`LivingEntity.tick`; `updateControlFlags` changes no modeled state).
pub fn tick(w: &mut World, i: usize) {
    // ---- mob AI (the mob-AI lane): Creeper.tick's fuse before super.tick ----
    if super::ai::has_ai(w, i) && mob(w, i).ty.def().id == "creeper" {
        super::ai::goal::creeper::creeper_tick(w, i);
    }
    // LivingEntity.onClimbable (the block at its feet in BlockTags.CLIMBABLE: the natural ladders
    // and vines; scaffolding, whose climbing is not modeled, is refused), read again where the
    // game reads it (`climbable_now`); `isPushable` reads the flag. A spider's is its climbing
    // flag (`Spider.onClimbable`).
    let c = climbable_now(w, i);
    mob_mut(w, i).on_climbable = c;
    let pre_air = mob(w, i).air;
    base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    // ---- mob AI (the mob-AI lane): the rest of LivingEntity.baseTick and Mob.baseTick ----
    if super::ai::has_ai(w, i) {
        super::ai::after_base_tick(w, i);
        // AgeableWaterCreature.baseTick's tail: handleAirSupply(the air before the tick) (a fish:
        // WaterAnimal's, the same)
        if (super::ai::squid::is_water_creature(w, i) || super::ai::fish::is_fish(w, i)) && super::ai::squid::handle_air_supply(w, i, pre_air) {
            hurt(w, i, Damage::Drown, 2.0);
        }
        // LivingEntity.tick after super.tick(): updatingUsingItem (a skeleton's drawn bow)
        if !w.entities.list[i].removed {
            super::ai::goal::skeleton::updating_using_item(w, i);
        }
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
    // SkeletonHorse.aiStep's tail: a trap horse no one keeps is discarded once its trapTime
    // passes 18000; SkeletonTrapGoal springing (a player within 10) is not modeled: named
    if let Some(t) = mob(w, i).trap_time
        && !w.entities.list[i].removed
    {
        let near = w.mob.ai.players.iter().any(|p| p.pos.distance_to_sqr(w.entities.list[i].pos) < 100.0);
        if near {
            w.mob.ai.legacy.insert("a skeleton trap horse's trap springing (its four horsemen)");
        }
        if !mob(w, i).persistence_required {
            mob_mut(w, i).trap_time = Some(t + 1);
            if t >= 18000 {
                let (tt, id) = (w.time, w.entities.list[i].id);
                w.entities.list[i].removed = true;
                w.mob.events.push(super::MobEvent::Removed { t: tt, id });
                // discard (setRemoved, DISCARDED destroys): it stops riding, its riders too
                crate::entity::stop_riding(w, i);
                crate::entity::eject_passengers(w, i);
                return;
            }
        }
    }
    // ---- mob AI (the mob-AI lane): tickHeadTurn, Mob.tick's updateControlFlags ----
    if super::ai::has_ai(w, i) {
        super::ai::after_ai_step(w, i);
    }
    // Spider.tick after super.tick: climbing while it collides horizontally
    if matches!(mob(w, i).ty.def().id, "spider" | "cave_spider") {
        super::ai::goal::spider::after_tick(w, i);
    }
    // Bat.tick after super.tick: still at its ceiling, or 0.6 of its vertical motion
    if super::ai::has_ai(w, i) && super::ai::bat::is_bat(w, i) {
        super::ai::bat::after_tick(w, i);
    }
    // AbstractHorse.tick after super.tick: the stand and tail counters
    if super::ai::has_ai(w, i) && super::ai::ai(w, i).horse.is_some() {
        super::ai::goal::animal::horse_after_tick(w, i);
    }
    // AbstractCubeMob.tick after super.tick: the landing's draws
    if super::ai::has_ai(w, i) {
        super::ai::cube::after_tick(w, i);
    }
    // Zombie.tick after super.tick: the drowning tracker (its conversion into a drowned)
    if super::ai::has_ai(w, i) && !w.entities.list[i].removed {
        super::ai::zombie_drowning(w, i);
    }
    // AbstractSchoolingFish.tick after super.tick: a lone leader's school forgotten
    if super::ai::has_ai(w, i) && super::ai::fish::is_fish(w, i) {
        super::ai::fish::school_tick(w, i);
    }
    // Wolf.tick after super.tick: wet in water or rain, the shake's draws
    if super::ai::has_ai(w, i) {
        super::ai::goal::wolf::after_tick(w, i);
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
    // Entity.baseTick's handlePortal (a mob through a nether portal leaves the overworld)
    if crate::entity::portal::handle_portal(w, i) {
        return;
    }
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
    // tickEffects: an instant health heals 4 (`heal`), regeneration heals 1 when its tick comes
    // (an infinite one's: the mob's tickCount % 50; `RegenerationMobEffect`), the others count down
    {
        let tick_count = w.entities.list[i].tick_count;
        let m = mob_mut(w, i);
        if m.effects.regeneration > 0 && tick_count % 50 == 0 {
            let max = max_health(m);
            if m.health < max && m.health > 0.0 {
                m.health = (m.health + 1.0).min(max);
            }
        }
        let e = &mut m.effects;
        let heal = std::mem::take(&mut e.instant_health);
        for d in [&mut e.water_breathing, &mut e.fire_resistance, &mut e.speed, &mut e.strength, &mut e.regeneration, &mut e.invisibility] {
            if *d > 0 && *d != i32::MAX {
                *d -= 1;
            }
        }
        if heal && m.health > 0.0 {
            let max = max_health(m);
            m.health = (m.health + 4.0).min(max);
        }
    }
    if mob(w, i).health <= 0.0 {
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

/// `EntityTypeTags.CAN_BREATHE_UNDER_WATER` (26.3): `#undead` (`#skeletons`, `#zombies`, the
/// wither, the phantom) and the water animals, the armor stand, the copper golem, the nautilus.
pub fn breathes_under_water(game_id: &str) -> bool {
    matches!(
        game_id,
        "minecraft:skeleton"
            | "minecraft:stray"
            | "minecraft:wither_skeleton"
            | "minecraft:skeleton_horse"
            | "minecraft:bogged"
            | "minecraft:parched"
            | "minecraft:zombie_horse"
            | "minecraft:camel_husk"
            | "minecraft:zombie"
            | "minecraft:zombie_villager"
            | "minecraft:zombified_piglin"
            | "minecraft:zoglin"
            | "minecraft:drowned"
            | "minecraft:husk"
            | "minecraft:zombie_nautilus"
            | "minecraft:wither"
            | "minecraft:phantom"
            | "minecraft:axolotl"
            | "minecraft:frog"
            | "minecraft:guardian"
            | "minecraft:elder_guardian"
            | "minecraft:turtle"
            | "minecraft:glow_squid"
            | "minecraft:cod"
            | "minecraft:pufferfish"
            | "minecraft:salmon"
            | "minecraft:squid"
            | "minecraft:tropical_fish"
            | "minecraft:tadpole"
            | "minecraft:armor_stand"
            | "minecraft:copper_golem"
            | "minecraft:nautilus"
    )
}

/// `LivingEntity.baseTick`'s air: the eye in water (not a bubble column) lowers the air
/// (`decreaseAirSupply`: by 1; an iron golem's keeps it), 2 drowning damage at -20 (then 0);
/// otherwise it refills by 4 up to 300.
fn air_tick(w: &mut World, i: usize, ty: MobType) {
    let e = &w.entities.list[i];
    let eye_cell = BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y + mob(w, i).eye() as f64), mth::floor(e.pos.z));
    let in_column = match w.get(eye_cell) {
        Block::BubbleColumn { .. } => true,
        Block::Terrain(id) => crate::terrain::blocks::props(id).block_id() == "minecraft:bubble_column",
        _ => false,
    };
    let eye_in_water = e.fluid.eye_in_water() && !in_column;
    if eye_in_water {
        // canBreatheUnderwater (#can_breathe_under_water: the undead, the water animals...) and
        // hasWaterBreathing (a water breathing effect: a witch's potion) keep its air
        if ty.def().drowns && !breathes_under_water(ty.def().game_id) && mob(w, i).effects.water_breathing == 0 {
            let m = mob_mut(w, i);
            m.air -= 1;
            if m.air <= -20 {
                m.air = 0;
                hurt(w, i, Damage::Drown, 2.0);
            }
        }
        // (mob-AI 2026-11-10) a rider whose vehicle dismounts underwater
        // (#dismounts_underwater: a chicken, a spider, a zombie horse...) gets off
        // (`stopRiding`, then `dismountVehicle`); bed-plate-two-clusters village-edge build 1
        // t66853, a baby drowned's chicken in deep water
        let vehicle_dismounts = w.entities.list[i].vehicle.and_then(|v| entity::index_of(w, v)).is_some_and(|vi| match &w.entities.list[vi].kind {
            EntityKind::Mob(vm) => {
                let id = vm.ty.def().id;
                matches!(id.strip_prefix("baby_").unwrap_or(id), "camel" | "chicken" | "donkey" | "happy_ghast" | "horse" | "llama" | "mule" | "pig" | "ravager" | "spider" | "strider" | "trader_llama" | "zombie_horse")
            }
            _ => false,
        });
        if vehicle_dismounts && !w.entities.list[i].removed {
            entity::stop_riding(w, i);
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
        // (tickexact 2026-11-10) AbstractCubeMob.remove: a dead cube bigger than 1 splits first
        if super::ai::has_ai(w, i) && super::ai::cube::is_cube(w, i) {
            super::ai::cube::split_on_death(w, i);
        }
        let id = w.entities.list[i].id;
        let t = w.time;
        w.entities.list[i].removed = true;
        w.mob.events.push(MobEvent::Removed { t, id });
        // Entity.setRemoved (KILLED destroys): it stops riding, then its passengers stop riding it
        // (a dead passenger leaves its boat's seats: the other takes the single seat)
        crate::entity::stop_riding(w, i);
        crate::entity::eject_passengers(w, i);
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
    // ---- mob AI (the mob-AI lane): AbstractHorse.aiStep's tail roll before super.aiStep;
    // Witch.aiStep's potion rolls before super.aiStep, then Monster.aiStep's updateNoActionTime ----
    if super::ai::has_ai(w, i) {
        super::ai::goal::animal::horse_before_ai_step(w, i);
        super::ai::goal::witch::before_ai_step(w, i);
        // Enderman.aiStep before super.aiStep: updatePersistentAnger(level, true)
        if super::ai::ai(w, i).enderman.is_some() {
            super::ai::goal::golem::update_anger(w, i);
        }
        super::ai::update_no_action_time(w, i);
        // AbstractFish.aiStep: the flop out of the water before super.aiStep
        if super::ai::fish::is_fish(w, i) {
            super::ai::fish::flop(w, i);
        }
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
    // isImmobile: dead or dying (AbstractHorse: only when also ridden and saddled, so a dying horse
    // keeps its AI step); a horse eating or standing
    let horse = super::ai::has_ai(w, i) && super::ai::ai(w, i).horse.is_some();
    let immobile = if horse { super::ai::goal::animal::horse_immobile(w, i) } else { mob(w, i).dead };
    if immobile {
        // isImmobile: no AI
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
    // LivingEntity.aiStep's last line: a water-sensitive mob (an enderman) in water or rain drowns
    // for 1
    if super::ai::has_ai(w, i) && super::ai::ai(w, i).enderman.is_some() && !w.entities.list[i].removed {
        super::ai::goal::enderman::water_tick(w, i);
    }
    // Mob.aiStep: burnUndead after LivingEntity.aiStep (mobinfra: zombies in daylight)
    super::sun::burn_undead(w, i);
    // Mob.aiStep's looting (the mob AI on): the item entities it picks up (`ai::loot`)
    if w.mob.ai.on && mob(w, i).can_pick_up_loot && !w.entities.list[i].removed {
        super::ai::loot::pick_up(w, i);
    }
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

/// `Entity.getFluidJumpThreshold`: 0 if the eye is under 0.4 high, else 0.4; a sulfur cube's
/// `getBbHeight() * 0.2` (`SulfurCube.getFluidJumpThreshold`; tickexact 2026-11-10: a size-2 cube
/// on the floor of 0.11 of water jumps from the ground, plains t1120).
fn fluid_jump_threshold(m: &MobState) -> f64 {
    if m.ty.def().game_id == "minecraft:sulfur_cube" {
        return m.ty.def().height as f64 * 0.2;
    }
    if (m.eye() as f64) < 0.4 { 0.0 } else { 0.4 }
}

/// `LivingEntity.jumpFromGround`: up at the jump power (0.42, nobody sprints).
fn jump_from_ground(w: &mut World, i: usize) {
    // JUMP_STRENGTH (0.42; a horse's as randomizeAttributes drew it) x the block's jump factor
    let strength = if super::ai::has_ai(w, i) { super::ai::ai(w, i).horse.map(|h| h.jump as f32).unwrap_or(0.42f32) } else { 0.42f32 };
    let power = strength * jump_factor(w, i);
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
    // Squid.travel: the motion alone (its aiStep sets it)
    if super::ai::has_ai(w, i) && super::ai::squid::is_water_creature(w, i) {
        let d = w.entities.list[i].delta_movement;
        entity::move_(w, i, MoverType::Self_, d);
        return;
    }
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
    // handleOnClimbable (onClimbable now): a climbing mob (a spider on a wall, a mob in a vine or
    // on a ladder) falls no faster than 0.15 and moves at most 0.15 sideways; its fall distance
    // resets
    let climbing = climbable_now(w, i);
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
    // and goes up 0.2 while it pushes against a wall (or jumps) in one (onClimbable after the
    // move, at its new feet)
    let climbing_after = climbable_now(w, i);
    mob_mut(w, i).on_climbable = climbing_after;
    if climbing_after && (w.entities.list[i].horizontal_collision || mob(w, i).jumping) {
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
    // AbstractFish.travelInWater: 0.01 of the input, the move, the motion x 0.9, then 0.005 down
    // without a target
    if super::ai::has_ai(w, i) && super::ai::fish::is_fish(w, i) {
        move_relative(w, i, 0.01, input);
        let d = w.entities.list[i].delta_movement;
        entity::move_(w, i, MoverType::Self_, d);
        let no_target = super::ai::ai(w, i).target.is_none();
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.scale(0.9);
        if no_target {
            e.delta_movement = e.delta_movement.add(0.0, -0.005, 0.0);
        }
        return;
    }
    // (tickexact 2026-11-10) Drowned.travelInWater: under water and wanting to swim, 0.01 of the
    // input, the move, the motion x 0.9 (no gravity, no jump out)
    if super::ai::has_ai(w, i) && super::ai::goal::drowned::is_drowned(w, i) && super::ai::goal::drowned::is_under_water(w, i) && super::ai::goal::drowned::wants_to_swim(w, i) {
        move_relative(w, i, 0.01, input);
        let d = w.entities.list[i].delta_movement;
        entity::move_(w, i, MoverType::Self_, d);
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.scale(0.9);
        return;
    }
    // getWaterSlowDown (0.9 sprinting: a cat's chase), WATER_MOVEMENT_EFFICIENCY (depth strider
    // boots: halved off the ground) toward 0.54600006 and the mob's speed
    let (mut slow, mut speed) = (if super::ai::goal::cat::is_sprinting(w, i) { 0.9f32 } else { 0.8f32 }, 0.02f32);
    {
        let m = mob(w, i);
        let mut walker = m.armor.water_movement_efficiency(m.helmet.is_some()) as f32;
        if !w.entities.list[i].on_ground {
            walker *= 0.5;
        }
        if walker > 0.0 {
            slow += (0.54600006f32 - slow) * walker;
            speed += (m.speed - speed) * walker;
        }
    }
    move_relative(w, i, speed, input);
    let d = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, d);
    let mut m = w.entities.list[i].delta_movement;
    // a climbing mob against a wall goes up 0.2 (a spider in water; onClimbable after the move)
    if w.entities.list[i].horizontal_collision && climbable_now(w, i) {
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

/// The block effects `checkInsideBlocks` collected this tick ([`entity_inside`], [`fluid_inside`]),
/// applied step by step (`StepBasedCollector.flushStep`), each step's in the
/// `InsideBlockEffectType` order: FIRE_IGNITE with its `inFire` hurt, LAVA_IGNITE (15 s of fire)
/// with `lavaHurt` after it (4), then EXTINGUISH (water puts the fire out).
fn apply_block_effects(w: &mut World, i: usize) {
    let fx = std::mem::take(&mut mob_mut(w, i).inside);
    // `StepBasedCollector.applyAndClear`: every effect runs only while the entity is alive (a
    // lethal lavaHurt leaves the water's EXTINGUISH out: the dead golem keeps its 300 fire ticks)
    let alive = |w: &World, i: usize| !w.entities.list[i].removed && mob(w, i).health > 0.0;
    for step in fx.steps {
        if step.fire && alive(w, i) {
            // mobinfra: FIRE_IGNITE (`BaseFireBlock.fireIgnite`: -1 counts up to 0, from 0 8 s of
            // fire) then its 1 `inFire` damage, before LAVA_IGNITE
            let t160 = super::ignite_ticks(w, i, 160);
            let m = mob_mut(w, i);
            if !m.ty.def().fire_immune {
                if m.fire_ticks < 0 {
                    m.fire_ticks += 1;
                }
                if m.fire_ticks >= 0 && m.fire_ticks < t160 {
                    m.fire_ticks = t160;
                }
            }
            if alive(w, i) {
                hurt(w, i, Damage::InFire, 1.0);
            }
        }
        if step.lava && alive(w, i) {
            let t300 = super::ignite_ticks(w, i, 300);
            let m = mob_mut(w, i);
            if !m.ty.def().fire_immune {
                // igniteForTicks(300) (x BURNING_TIME) then lavaHurt: a hurt that lands plays
                // GENERIC_BURN, its pitch from the entity's own random (`2.0F + random.nextFloat()
                // * 0.4F`)
                if m.fire_ticks < t300 {
                    m.fire_ticks = t300;
                }
                if alive(w, i)
                    && hurt(w, i, Damage::Lava, 4.0)
                    && let Some(r) = w.entities.list[i].random.as_mut()
                {
                    r.next_float();
                }
            }
        }
        if step.water && alive(w, i) {
            let m = mob_mut(w, i);
            m.fire_ticks = m.fire_ticks.min(0);
        }
    }
}

/// `StepBasedCollector.advanceStep`: the sweep's step of the effects that follow.
pub fn advance_step(w: &mut World, i: usize, step: i32) {
    mob_mut(w, i).inside.advance(step);
}

/// `entityInside` of the blocks the mob's sweep meets (`react::entity_inside` calls it for a mob):
/// a fire's cell. The fluids' effects come from [`fluid_inside`].
pub fn entity_inside(w: &mut World, i: usize, s: Block, p: BlockPos) {
    let _ = p;
    match s {
        Block::Fire { .. } => {
            mob_mut(w, i).inside.now().fire = true;
        }
        _ => {}
    }
}

/// `FluidState.entityInside` on a mob whose sweep met the fluid's box (`collidedWithFluid`):
/// lava's LAVA_IGNITE with `lavaHurt` after it (`LavaFluid.entityInside`), water's EXTINGUISH
/// (`WaterFluid.entityInside`), collected for [`apply_block_effects`].
pub fn fluid_inside(w: &mut World, i: usize, lava: bool) {
    if lava {
        mob_mut(w, i).inside.now().lava = true;
    } else {
        mob_mut(w, i).inside.now().water = true;
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
/// `hurtServer` of mob `i` (an enderman's override: after the hurt, a damage no living entity
/// caused teleports it nine times in ten).
pub fn hurt(w: &mut World, i: usize, src: Damage, amount: f32) -> bool {
    let r = hurt_inner(w, i, src, amount);
    if !w.entities.list[i].removed && super::ai::has_ai(w, i) && super::ai::ai(w, i).enderman.is_some() {
        super::ai::goal::enderman::after_hurt(w, i, src);
    }
    // AbstractHorse.hurtServer: hurt, 1 in 3 it stands
    if r && super::ai::has_ai(w, i) && super::ai::ai(w, i).horse.is_some() && super::ai::goal::animal::own_random(w, i).next_int(3) == 0 {
        super::ai::goal::animal::stand_if_possible(w, i);
    }
    r
}

fn hurt_inner(w: &mut World, i: usize, src: Damage, amount: f32) -> bool {
    if w.entities.list[i].removed {
        return false;
    }
    let ty = mob(w, i).ty;
    let def = ty.def();
    // isInvulnerableTo: fire immunity, fall immunity
    if (src.is_fire() && def.fire_immune) || (src == Damage::Fall && def.fall_immune) {
        return false;
    }
    // (mob-AI 2026-11-10) Bat.hurtServer: a resting bat stops resting before LivingEntity's hurt
    // (forest build 3 t49127: a hanging bat drowned for 2 and flew off)
    if mob(w, i).bat_resting {
        mob_mut(w, i).bat_resting = false;
    }
    // hurtServer: a fire damage does nothing under fire resistance
    if src.is_fire() && mob(w, i).effects.fire_resistance > 0 {
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
    // (tickexact 2026-11-10) Zombie.hurtServer after the hurt (zombies, zombie villagers, husks,
    // drowned, zombified piglins): the reinforcement roll on Hard with a target, and the call
    if super::natural::is_zombie_class(ty) && super::ai::has_ai(w, i) {
        let attacker = if src.by_attacker() { w.mob.ai.attacker } else { None };
        super::natural::zombie_reinforcements(w, i, attacker);
    }
    true
}

/// `LivingEntity.actuallyHurt`: armor (`CombatRules.getDamageAfterAbsorb`, unless the source
/// bypasses armor: ARMOR and ARMOR_TOUGHNESS with the worn pieces'), the armor's enchantment
/// protection (`getDamageAfterMagicAbsorb`: `CombatRules.getDamageAfterMagicAbsorb`), then the
/// health.
fn actually_hurt(w: &mut World, i: usize, src: Damage, mut dmg: f32) {
    let (armor, toughness, protection) = {
        let m = mob(w, i);
        let helmet = m.helmet.is_some();
        (m.ty.def().armor + m.armor.value(helmet), m.armor.toughness(helmet), m.armor.protection(helmet, src))
    };
    if !src.bypasses_armor() && armor > 0.0 {
        let toughness = 2.0f32 + toughness / 4.0f32;
        let real = (armor - dmg / toughness).clamp(armor * 0.2f32, 20.0f32);
        dmg *= 1.0f32 - real / 25.0f32;
    }
    if dmg > 0.0 && protection > 0.0 {
        dmg *= 1.0f32 - protection.clamp(0.0, 20.0) / 25.0f32;
    }
    if dmg != 0.0 {
        // setHealth: clamped to [0, getMaxHealth()] (a horse's drawn max, a leader zombie's bonus)
        let max = max_health(mob(w, i));
        let m = mob_mut(w, i);
        m.health = (m.health - dmg).clamp(0.0, max);
    }
}

/// `LivingEntity.getMaxHealth()`: the type's, a horse's as `randomizeAttributes` drew it, a leader
/// zombie's with its `LEADER_ZOMBIE_BONUS` (ADD_MULTIPLIED_TOTAL).
pub fn max_health(m: &MobState) -> f32 {
    if let Some(h) = m.ai.as_deref().and_then(|a| a.horse) {
        return h.max_health;
    }
    if m.max_health_bonus > 0.0 {
        return (m.ty.def().max_health as f64 * (1.0 + m.max_health_bonus)) as f32;
    }
    m.ty.def().max_health
}

/// `LivingEntity.die`: dead; a sleeper wakes; the loot drops at once (`dropAllDeathLoot`: no
/// player kill, so no experience); pose DYING (the body stays 20 gt: `tickDeath`).
fn die(w: &mut World, i: usize, src: Damage) {
    if w.entities.list[i].removed || mob(w, i).dead {
        return;
    }
    // (mob-AI 2026-11-10) Villager.die: releaseAllPois before LivingEntity.die (the witnesses'
    // gossip about the murderer moves nothing)
    if mob(w, i).ty == super::types::MobType::Villager && mob(w, i).brain.as_deref().is_some_and(|b| b.ex.is_some()) {
        super::ai::brain::run::release_all_pois(w, i);
    }
    if mob(w, i).sleeping_pos.is_some() {
        super::villager::stop_sleeping(w, i);
    }
    // (mob-AI 2026-11-10) LivingEntity.die: stopUsingItem (a dying spear zombie stabs no more;
    // forest build 1 t11260)
    super::ai::goal::skeleton::stop_using_item(w, i);
    mob_mut(w, i).dead = true;
    let (t, id, ty, pos) = {
        let e = &w.entities.list[i];
        (w.time, e.id, mob(w, i).ty, e.pos)
    };
    w.mob.events.push(MobEvent::Death { t, id, ty, src, pos: [pos.x, pos.y, pos.z] });
    // ---- mob AI (the mob-AI lane): the killer's `killedEntity` (a zombie's villager conversion:
    // then no ENTITY_DIE game event and no loot) ----
    if w.mob.ai.on && !super::ai::attack::killed_entity(w, i, src) {
        return;
    }
    w.game_event_at("entity_die", [pos.x, pos.y, pos.z], None, false);
    drop_loot(w, i, src);
    // dropCustomDeathLoot (after the loot table, when it drops loot): a picked-up item in hand
    if drops_loot(w, i) {
        super::ai::loot::drop_hand(w, i);
    }
}

/// The dying mob's facts its loot table reads (`loot::Victim`).
fn victim(w: &World, i: usize, src: Damage) -> super::loot::Victim {
    let m = mob(w, i);
    let e = &w.entities.list[i];
    let rides_zombie_horse = e
        .vehicle
        .and_then(|v| w.entities.list.iter().find(|x| x.id == v))
        .is_some_and(|x| matches!(&x.kind, EntityKind::Mob(vm) if vm.ty.def().game_id == "minecraft:zombie_horse"));
    let cube_size = match m.ty.def().id {
        "slime" => Some(1),
        "slime_2" => Some(2),
        "slime_4" => Some(4),
        _ => None,
    };
    let wool = (m.ty.def().game_id == "minecraft:sheep").then(|| super::loot::wool_of(m.color));
    let killer_skeleton = src.by_attacker() && w.mob.ai.attacker.and_then(|a| w.entities.list.iter().find(|x| x.id == a)).is_some_and(|x| matches!(&x.kind, EntityKind::Mob(km) if matches!(km.ty.def().id, "skeleton" | "stray" | "bogged" | "wither_skeleton")));
    super::loot::Victim { on_fire: m.fire_ticks > 0 && !m.ty.def().fire_immune, cube_size, rides_zombie_horse, wool, killed_by_skeleton: killer_skeleton, lightning: src == Damage::Lightning }
}

/// `LivingEntity.shouldDropLoot`: no baby drops anything; `Monster.shouldDropLoot`: a monster drops
/// whatever its age (tickexact 2026-11-10: a baby zombie's death draws the zombie table's sequence,
/// d1u-deep t37298). The cube mobs are no `Monster` (an `AgeableMob`).
fn drops_loot(w: &World, i: usize) -> bool {
    let id = mob(w, i).ty.def().id;
    if mob(w, i).ty.def().monster && super::ai::cube::cube_size(id).is_none() {
        return true;
    }
    !(id.starts_with("baby_") || super::ai::goal::animal::is_baby(id))
}

/// `dropFromLootTable`: each pool's count (`set_count uniform`), each non-empty stack an item at
/// the mob's position (`Entity.spawnAtLocation`: `new ItemEntity(level, x, y, z, stack)` with the
/// item's own random motion (+-0.1, 0.2, +-0.1) and yaw, pickup delay 10). The loot random and
/// the item's own random are the seeded stand-ins (`MobWorld::rng`).
fn drop_loot(w: &mut World, i: usize, src: Damage) {
    if !drops_loot(w, i) {
        return;
    }
    if w.exact_seed.is_some() && w.loot_salt.is_some() {
        return drop_loot_exact(w, i, src);
    }
    let (pos, ty) = (w.entities.list[i].pos, mob(w, i).ty);
    let src_id = w.entities.list[i].id;
    // a natural mob's table (pools, weighted entries): every stack first, then the items
    if let Some(pools) = super::loot::table(ty.def().game_id) {
        let v = victim(w, i, src);
        let stacks = match super::loot::roll(pools, &v, w.mob.rng_mut()) {
            Ok(s) => s,
            Err(why) => {
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
                return;
            }
        };
        for (item, n) in stacks {
            if n <= 0 {
                continue;
            }
            let (mx, mz, y_rot, own) = {
                let r = w.mob.rng_mut();
                let seed = r.next_long();
                let mut own = crate::rng::LegacyRandom::new(seed);
                own.next_float();
                let y_rot = own.next_float() * 360.0;
                let mx = own.next_double() * 0.2 - 0.1;
                let mz = own.next_double() * 0.2 - 0.1;
                (mx, mz, y_rot, own)
            };
            let state = crate::entity::item::ItemState { item: item.to_string(), count: n as u8, pickup_delay: 10, age: 0, payload: 0, health: 5, ..Default::default() };
            let id = w.entities.add(EntityKind::Item(state), Vec3::new(pos.x, pos.y, pos.z), y_rot);
            if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
                e.delta_movement = Vec3::new(mx, 0.2, mz);
                e.random = Some(own);
            }
            let t = w.time;
            w.mob.events.push(MobEvent::Drop { t, from: src_id, item: id, what: item, count: n as u32 });
        }
        return;
    }
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
        let state = crate::entity::item::ItemState { item: item.to_string(), count: n as u8, pickup_delay: 10, age: 0, payload: 0, health: 5, ..Default::default() };
        let id = w.entities.add(EntityKind::Item(state), Vec3::new(pos.x, pos.y, pos.z), y_rot);
        if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
            e.delta_movement = Vec3::new(mx, 0.2, mz);
            e.random = Some(own);
        }
        let t = w.time;
        w.mob.events.push(MobEvent::Drop { t, from: src_id, item: id, what: item, count: n as u32 });
    }
}

/// `Entity.spawnAtLocation(stack)` of a mob's item outside its loot table (a chicken's egg):
/// `new ItemEntity(level, x, y, z, stack)` at its position, the item's own random as an entering
/// entity's (the bridge's seeding in an exact world: its UUID replayed), bobOffs, yaw and drift.
pub fn spawn_item(w: &mut World, i: usize, item: &'static str, count: u8) {
    let pos = w.entities.list[i].pos;
    let src_id = w.entities.list[i].id;
    spawn_item_at(w, pos, src_id, item, count);
}

/// [`spawn_item`] at a position, for any entity `src_id` (a falling block that breaks).
pub fn spawn_item_at(w: &mut World, pos: Vec3, src_id: u32, item: &'static str, count: u8) {
    let at = w.entities.next_id;
    let mut own = w.entering_random(at);
    if w.exact_seed.is_some() {
        crate::rng::replay_uuid(&mut own);
    }
    own.next_float();
    let y_rot = own.next_float() * 360.0;
    let mx = own.next_double() * 0.2 - 0.1;
    let mz = own.next_double() * 0.2 - 0.1;
    let state = crate::entity::item::ItemState { item: item.to_string(), count, pickup_delay: 10, age: 0, payload: 0, health: 5, ..Default::default() };
    let id = w.entities.add(EntityKind::Item(state), Vec3::new(pos.x, pos.y, pos.z), y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.delta_movement = Vec3::new(mx, 0.2, mz);
        e.random = Some(own);
    }
    let t = w.time;
    w.mob.events.push(MobEvent::Drop { t, from: src_id, item: id, what: item, count: count as u32 });
}

/// [`drop_loot`] in an exact world (tickexact): `LivingEntity.dropFromLootTable` with the table's
/// random sequence (`getLootTableSeed()` is 0: `level.getRandomSequence(entities/<id>)`, the
/// bridge's sandboxes salt them from their seed: `World::loot_salt`), each pool's `set_count
/// uniform(lo, hi)` (`Mth.nextInt(random, lo, hi)`; the other pools' conditions, a killer's
/// looting and a zombie horse, draw nothing), and each non-empty stack through
/// `spawnAtLocation` (`new ItemEntity(level, x, y, z, stack)`): the item enters with its own random
/// seeded as its entity number, the constructor's UUID, bobOffs, yaw and drift replayed (the
/// drift kept: x and z, 0.2 up).
fn drop_loot_exact(w: &mut World, i: usize, src: Damage) {
    let (pos, ty) = (w.entities.list[i].pos, mob(w, i).ty);
    let src_id = w.entities.list[i].id;
    // the table's random sequence is named by the entity type (a baby's rows share it)
    let key = format!("minecraft:entities/{}", ty.def().game_id.strip_prefix("minecraft:").unwrap_or(ty.def().game_id));
    let salt = w.loot_salt.unwrap_or(0);
    // a natural mob's table: getRandomItems makes every stack, then spawnAtLocation drops each
    let stacks: Vec<(&'static str, i32)> = if let Some(pools) = super::loot::table(ty.def().game_id) {
        let v = victim(w, i, src);
        let r = w.loot_sequences.entry(key.clone()).or_insert_with(|| crate::stems::LootSequence::new(salt, &key));
        match super::loot::roll(pools, &v, r) {
            Ok(s) => s,
            Err(why) => {
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
                return;
            }
        }
    } else {
        ty.def()
            .loot
            .iter()
            .map(|&(item, lo, hi)| {
                let r = w.loot_sequences.entry(key.clone()).or_insert_with(|| crate::stems::LootSequence::new(salt, &key));
                (item, if lo >= hi { lo } else { lo + r.next_int(hi - lo + 1) })
            })
            .collect()
    };
    for (item, n) in stacks {
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
        let state = crate::entity::item::ItemState { item: item.to_string(), count: n as u8, pickup_delay: 10, age: 0, payload: 0, health: 5, ..Default::default() };
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
    // the AFK players its box meets (the mob AI's player bodies; (mob-AI 2026-11-10) a rider's
    // too: LivingEntity.aiStep's pushEntities runs in its rideTick; bed-plate-dug12 plains build 2
    // t60407, a zombie horseman's rider pushed the AFK player)
    let players = if w.mob.ai.on { super::ai::player_body::touching(w, bb) } else { Vec::new() };
    if others.is_empty() && players.is_empty() {
        return;
    }
    let cram = w.mob.cramming;
    // the cramming roll is the mob's own random (the stand-in without the mob AI's seeding)
    let roll = |w: &mut World| if super::ai::has_ai(w, i) { w.entities.list[i].random.as_mut().expect("random").next_int(4) } else { w.mob.rng_mut().next_int(4) };
    if cram > 0 && (others.len() + players.len()) as i32 > cram - 1 && roll(w) == 0 {
        let count = others.iter().filter(|&&j| w.entities.list[j].vehicle.is_none()).count() as i32 + players.len() as i32;
        if count > cram - 1 {
            hurt(w, i, Damage::Cramming, 6.0);
        }
    }
    for k in players {
        super::ai::player_body::pushed_by_mob(w, k, i);
    }
    for j in others {
        match &w.entities.list[j].kind {
            EntityKind::Mob(_) => {}
            // `doPush(cart)` = `AbstractMinecart.push(mob)`
            EntityKind::Minecart(_) => {
                crate::entity::minecart::pushed_by_mob(w, j, i);
                continue;
            }
            // `doPush(boat)` = `AbstractBoat.push(mob)`: `Entity.push` when the mob's box is no
            // higher than the boat's
            EntityKind::Boat(_) => {
                if w.entities.list[i].bb.min_y <= w.entities.list[j].bb.min_y {
                    crate::entity::push::generic(w, j, i);
                }
                continue;
            }
            k => {
                let why = format!("a mob touching a {} (its push) is not modeled", super::kind_name(k));
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
                continue;
            }
        }
        if super::ai::has_ai(w, i) {
            super::ai::before_do_push(w, i, j);
        }
        // `doPush(e)` = `e.push(this)`, and `LivingEntity.push(Entity)` does nothing for a
        // sleeper: neither moves (tickexact 2026-11-09, golem-summon: a panicking villager
        // walking over a sleeping one's bed)
        if mob(w, j).sleeping_pos.is_some() {
            continue;
        }
        push_pair(w, j, i);
        // (tickexact 2026-11-10) AbstractCubeMob.push(golem): a slime bigger than 1 hurts an iron
        // golem pushing it (dealDamage: its melee range, sight, ATTACK_DAMAGE): not modeled, named
        if mob(w, i).ty == MobType::IronGolem && super::ai::has_ai(w, j) && super::ai::cube::is_cube(w, j) && super::ai::cube::can_deal_damage(w, j) {
            let why = "a slime hurting an iron golem that pushes it (AbstractCubeMob.dealDamage) is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
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

/// `Entity.isPushable` of the entities a mob (or the AFK player) can push.
pub(crate) fn pushable(e: &entity::Entity) -> bool {
    match &e.kind {
        // (tickexact 2026-11-10) AbstractHorse.isPushable: not while it carries a rider, nothing
        // else (a zombie jockey's horse: a creeper pushing it got the push back, d1u-deep t7324)
        EntityKind::Mob(m) if super::ai::goal::animal::is_horse(m.ty.def().id) => e.passengers.is_empty(),
        // a bat is never pushed (`Bat.isPushable`); a climbing mob is not (`LivingEntity.isPushable`:
        // `!onClimbable()`, a spider's its climbing flag as Spider.tick last set it)
        EntityKind::Mob(m) => {
            let climbing = if matches!(m.ty.def().id, "spider" | "cave_spider") { m.climbing } else { m.on_climbable };
            !m.dead && m.ty.def().id != "bat" && !climbing
        }
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
        // each side only if it is pushable itself (`this.isPushable()`, `entity.isPushable()`: a
        // climbing spider is pushed by nobody, though it pushes others) and carries no one
        if w.entities.list[j].passengers.is_empty() && pushable(&w.entities.list[j]) {
            let e = &mut w.entities.list[j];
            e.delta_movement = e.delta_movement.add(-xa, 0.0, -za);
        }
        if w.entities.list[i].passengers.is_empty() && pushable(&w.entities.list[i]) {
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
    // a chicken takes no fall damage (`Chicken.causeFallDamage`: false), its fall distance kept
    let chicken = super::ai::goal::animal::adult_of(mob(w, i).ty.def().id) == Some("chicken") && super::ai::has_ai(w, i);
    if !w.entities.list[i].was_touching_water {
        crate::entity::fluid_push::update_fluid_interaction(w, i);
        // the same trackers give isInLava and the lava's height: travelInLava's shallow test
        // after this move reads the box the move left (a golem falling into its kill cell)
        update_lava(w, i);
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
        if !matches!(on_state, Block::Simple(Simple::SlimeBlock)) && !chicken {
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
    // a horse: SAFE_FALL_DISTANCE 6, FALL_DAMAGE_MULTIPLIER 0.5 (createBaseHorseAttributes)
    if super::ai::goal::animal::is_horse(mob(w, i).ty.def().id) {
        let dmg = mth::floor((distance + 1.0e-6 - 6.0) * 1.0 * 0.5);
        if dmg > 0 {
            hurt(w, i, Damage::Fall, dmg as f32);
        }
        return;
    }
    let dmg = mth::floor(distance + 1.0e-6 - 3.0);
    if dmg > 0 {
        hurt(w, i, Damage::Fall, dmg as f32);
    }
}
