//! Item entities (redsim M5, clockinfra): `ItemEntity` (26.3), for items a design summons
//! (`entity: minecraft:item x y z {Item:{id:"minecraft:stone",count:1},Motion:[..],PickupDelay:10s}`).
//!
//! * `tick`: `Entity.tick` (= `baseTick`: fluid interaction), pickup delay, water movement (x0.99,
//!   a 5e-4 lift under 0.06) or gravity 0.04, `move` (skipped 3 ticks in 4 when resting on the
//!   ground: `(tickCount + id) % 4`, the game's network id, here the insertion id), the drag 0.98
//!   (times the block friction on the ground), the bounce (y x -0.5 on the ground), merging with
//!   neighbors every 2 ticks while moving between cells (40 at rest), age, a second
//!   `updateFluidInteraction`, despawn at 6000.
//! * Not reproducible in the game itself, so not modeled (a model note): an item stuck inside a
//!   block (`moveTowardsClosestSpace` draws from the entity's own unseeded random) and drops (the
//!   level random places them). Hoppers (blocks and minecarts) pull item entities
//!   (`redstone::container::item_entities_above`); stack sizes come from the container item list.

use crate::entity::{EntityKind, MoverType, Vec3, collide, mth};
use crate::world::World;

/// `EntityTypes.ITEM.sized(0.25F, 0.25F)`.
pub const WIDTH: f32 = 0.25;
pub const HEIGHT: f32 = 0.25;

/// The item entity's own state.
#[derive(Clone, PartialEq, Default)]
pub struct ItemState {
    /// The stack's item id (`minecraft:...`; empty = no item: discarded at its next tick).
    pub item: String,
    pub count: u8,
    /// `pickupDelay` (32767 = never).
    pub pickup_delay: i32,
    /// `age` (-32768 = never ages).
    pub age: i32,
    /// A shulker box item's contents (`redstone::container::Stack::payload`; clk-rt): 0 = none.
    pub payload: u16,
    /// `health` (5; explosions take it, clk-misc): gone at 0.
    pub health: i32,
    /// `remainingFireTicks` (tickexact: lava's 15 s, a fire's 8 s; 0 at rest).
    pub fire: i32,
    /// The block effects `checkInsideBlocks` collected this tick (the mobs' collector).
    pub inside: crate::entity::mob::Inside,
}

impl std::fmt::Debug for ItemState {
    /// As derived, without a zero payload or a full health (so states without them print as
    /// before they existed).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("ItemState");
        d.field("item", &self.item).field("count", &self.count).field("pickup_delay", &self.pickup_delay).field("age", &self.age);
        if self.payload != 0 {
            d.field("payload", &self.payload);
        }
        if self.health != 5 {
            d.field("health", &self.health);
        }
        if self.fire != 0 {
            d.field("fire", &self.fire);
        }
        d.finish()
    }
}

impl ItemState {
    /// From the summon NBT: `Item:{id:"..",count:N}`, `PickupDelay:Ns`, `Age:Ns`.
    pub fn from_nbt(nbt: Option<&str>) -> ItemState {
        let mut s = ItemState { item: String::new(), count: 1, pickup_delay: 0, age: 0, payload: 0, health: 5, ..Default::default() };
        let Some(n) = nbt else { return s };
        if let Some(i) = n.find("id:\"") {
            s.item = n[i + 4..].split('"').next().unwrap_or("").to_string();
        }
        let num = |key: &str| -> Option<i32> {
            let i = n.find(key)?;
            let v: String = n[i + key.len()..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            v.parse().ok()
        };
        if let Some(c) = num("count:") {
            s.count = c.clamp(0, 255) as u8;
        }
        if let Some(d) = num("PickupDelay:") {
            s.pickup_delay = d;
        }
        if let Some(a) = num("Age:") {
            s.age = a;
        }
        s
    }
}

fn state(w: &World, i: usize) -> ItemState {
    match &w.entities.list[i].kind {
        EntityKind::Item(s) => s.clone(),
        _ => ItemState::default(),
    }
}

fn set_state(w: &mut World, i: usize, s: ItemState) {
    w.entities.list[i].kind = EntityKind::Item(s);
}

/// `ItemEntity.hurtServer` (the modeled sources: lava, fire): the damage off its health (an int);
/// gone at 0 (`discard`). False for a fire-resistant item (netherite) and a gone one.
fn hurt(w: &mut World, i: usize, damage: f32) -> bool {
    if w.entities.list[i].removed || fire_resistant(&state(w, i).item) {
        return false;
    }
    let mut s = state(w, i);
    s.health = (s.health as f32 - damage) as i32;
    let gone = s.health <= 0;
    let (what, count) = (s.item.clone(), s.count as u32);
    set_state(w, i, s);
    if gone {
        w.entities.list[i].removed = true;
        // the judge's evidence: a drop burnt up (the farm counts its iron as burnt, iteration 11)
        let (t, item) = (w.time, w.entities.list[i].id);
        w.mob.events.push(crate::entity::mob::MobEvent::Burnt { t, item, what, count });
    }
    true
}

/// `ItemEntity.fireImmune`: the item cannot be hurt by fire (`DamageResistant` fire: the netherite
/// items and debris).
fn fire_resistant(item: &str) -> bool {
    let id = item.trim_start_matches("minecraft:");
    id.starts_with("netherite_") || id == "ancient_debris"
}

/// `StepBasedCollector.advanceStep` for an item.
pub fn advance_step(w: &mut World, i: usize, step: i32) {
    if let EntityKind::Item(s) = &mut w.entities.list[i].kind {
        s.inside.advance(step);
    }
}

/// `FluidState.entityInside` on an item (its sweep met the fluid's box): lava's LAVA_IGNITE and
/// `lavaHurt`, water's EXTINGUISH, collected for [`apply_block_effects`].
pub fn fluid_inside(w: &mut World, i: usize, lava: bool) {
    if let EntityKind::Item(s) = &mut w.entities.list[i].kind {
        if lava {
            let st = s.inside.now();
            st.lava = true;
            st.lava_cells = st.lava_cells.saturating_add(1);
        } else {
            s.inside.now().water = true;
        }
    }
}

/// `BaseFireBlock.entityInside` on an item: FIRE_IGNITE and its `inFire` hurt.
pub fn fire_inside(w: &mut World, i: usize) {
    if let EntityKind::Item(s) = &mut w.entities.list[i].kind {
        let st = s.inside.now();
        st.fire = true;
        st.fire_cells = st.fire_cells.saturating_add(1);
    }
}

/// `Entity.applyEffectsFromBlocks`' collector and tail for an item (tickexact 2026-11-08: a drop
/// that reaches lava burns in 2 ticks): each step's effects in `InsideBlockEffectType` order while
/// it lives (FIRE_IGNITE + 1 `inFire`; LAVA_IGNITE, 15 s, + `lavaHurt` 4, whose GENERIC_BURN plays
/// at the last hurt or every 10th tick: a float of its own random; EXTINGUISH), then the rain puts
/// the fire out, a fire gone out plays GENERIC_EXTINGUISH_FIRE (2 floats), and a fire not lit this
/// tick resets to 0.
fn apply_block_effects(w: &mut World, i: usize, fire_before: i32) {
    let fx = match &mut w.entities.list[i].kind {
        EntityKind::Item(s) => std::mem::take(&mut s.inside),
        _ => return,
    };
    let resistant = fire_resistant(&state(w, i).item);
    for step in fx.steps {
        if step.fire && !w.entities.list[i].removed {
            let mut s = state(w, i);
            if !resistant {
                if s.fire < 0 {
                    s.fire += 1;
                }
                if s.fire >= 0 && s.fire < 160 {
                    s.fire = 160;
                }
            }
            set_state(w, i, s);
            // each fire cell's `inFire` hurt (`runAfter`), while it lives
            for _ in 0..step.fire_cells.max(1) {
                if w.entities.list[i].removed {
                    break;
                }
                hurt(w, i, 1.0);
            }
        }
        if step.lava && !w.entities.list[i].removed && !resistant {
            let mut s = state(w, i);
            s.fire = s.fire.max(300);
            set_state(w, i, s);
            // each lava cell's `lavaHurt` (`runAfter`), while it lives
            for _ in 0..step.lava_cells.max(1) {
                if w.entities.list[i].removed {
                    break;
                }
                let mut s = state(w, i);
                s.fire = s.fire.max(300);
                set_state(w, i, s);
                if hurt(w, i, 4.0) {
                    let (health, tick) = (state(w, i).health, w.entities.list[i].tick_count);
                    if (health <= 0 || tick % 10 == 0)
                        && let Some(r) = w.entities.list[i].random.as_mut()
                    {
                        r.next_float();
                    }
                }
            }
        }
        if step.water && !w.entities.list[i].removed {
            let mut s = state(w, i);
            s.fire = s.fire.min(0);
            set_state(w, i, s);
        }
    }
    if w.entities.list[i].removed {
        return;
    }
    let was_on_fire = !resistant && fire_before > 0;
    if state(w, i).fire > 0 && crate::weather::entity_in_rain(w, i) {
        let mut s = state(w, i);
        s.fire = s.fire.min(0);
        set_state(w, i, s);
    }
    let mut s = state(w, i);
    let on_fire = !resistant && s.fire > 0;
    if was_on_fire
        && !on_fire
        && let Some(r) = w.entities.list[i].random.as_mut()
    {
        r.next_float();
        r.next_float();
    }
    if !on_fire && s.fire <= fire_before {
        s.fire = 0;
    }
    set_state(w, i, s);
}

/// `ItemEntity.isMergable`.
fn mergable(e: &crate::entity::Entity, s: &ItemState) -> bool {
    !e.removed && s.pickup_delay != 32767 && s.age != -32768 && s.age < 6000 && s.count < crate::redstone::container::max_stack_of(&s.item)
}

/// `ItemEntity.mergeWithNeighbours` / `tryToMerge` / `merge`: the smaller stack moves into the
/// larger one (candidates in list order).
fn merge_with_neighbours(w: &mut World, i: usize) {
    if !mergable(&w.entities.list[i], &state(w, i)) {
        return;
    }
    let bb = w.entities.list[i].bb.inflate(0.5, 0.0, 0.5);
    let others = w.get_entities(bb, |o| matches!(o.kind, EntityKind::Item(_)));
    for j in others {
        if j == i {
            continue;
        }
        let (a, b) = (state(w, i), state(w, j));
        if !mergable(&w.entities.list[j], &b) {
            continue;
        }
        let max = crate::redstone::container::max_stack_of(&a.item) as u32;
        if a.item != b.item || a.count as u32 + b.count as u32 > max {
            continue;
        }
        // tryToMerge: into this one unless the other is at least as large
        let (to, from) = if b.count < a.count { (i, j) } else { (j, i) };
        let (mut ts, mut fs) = (state(w, to), state(w, from));
        // merge(toStack, fromStack, 64): min(min(maxStack, 64) - count, from count)
        let delta = ((max.min(64)) as i32 - ts.count as i32).min(fs.count as i32).max(0);
        ts.count = (ts.count as i32 + delta) as u8;
        fs.count = (fs.count as i32 - delta) as u8;
        ts.pickup_delay = ts.pickup_delay.max(fs.pickup_delay);
        ts.age = ts.age.min(fs.age);
        let empty = fs.count == 0;
        set_state(w, to, ts);
        set_state(w, from, fs);
        if empty {
            w.entities.list[from].removed = true;
        }
        if w.entities.list[i].removed {
            break;
        }
    }
}

/// `ItemEntity.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    let mut s = state(w, i);
    if s.item.is_empty() || s.count == 0 {
        w.entities.list[i].removed = true;
        return;
    }
    // Entity.tick = baseTick
    crate::entity::base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    if s.pickup_delay > 0 && s.pickup_delay != 32767 {
        s.pickup_delay -= 1;
    }
    set_state(w, i, s);
    // baseTick's fire: an `onFire` hurt (1) every 20 ticks out of lava, then a tick less
    {
        let s = state(w, i);
        if s.fire > 0 {
            if fire_resistant(&s.item) {
                let mut s = s;
                s.fire = s.fire.min(0);
                set_state(w, i, s);
            } else {
                let in_lava = w.entities.list[i].fluid.lava_height() > 0.0;
                if s.fire % 20 == 0 && !in_lava {
                    hurt(w, i, 1.0);
                }
                if w.entities.list[i].removed {
                    return;
                }
                let mut s = state(w, i);
                s.fire -= 1;
                set_state(w, i, s);
            }
        }
    }
    let gravity = crate::entity::kinds::gravity(w, i);
    let old_movement;
    {
        let e = &mut w.entities.list[i];
        e.xo = e.pos.x;
        e.yo = e.pos.y;
        e.zo = e.pos.z;
        old_movement = e.delta_movement;
        if e.was_touching_water && e.fluid.water_height() > 0.1f32 as f64 {
            // setUnderwaterMovement: setFluidMovement(0.99F)
            let m = e.delta_movement;
            let k = 0.99f32 as f64;
            e.delta_movement = Vec3::new(m.x * k, m.y + if m.y < 0.06f32 as f64 { 5.0e-4f32 as f64 } else { 0.0 }, m.z * k);
        } else if !e.first_tick && e.fluid.lava_height() > 0.1f32 as f64 {
            // `isInLava()` with lava above 0.1: setUnderLavaMovement = setFluidMovement(0.95F)
            // (tickexact: redsim let a drop in lava fall)
            let m = e.delta_movement;
            let k = 0.95f32 as f64;
            e.delta_movement = Vec3::new(m.x * k, m.y + if m.y < 0.06f32 as f64 { 5.0e-4f32 as f64 } else { 0.0 }, m.z * k);
        } else if gravity != 0.0 {
            e.delta_movement = e.delta_movement.add(0.0, -gravity, 0.0);
        }
    }
    let bb = w.entities.list[i].bb.deflate(1.0e-7, 1.0e-7, 1.0e-7);
    let stuck = !crate::entity::no_collision(w, i, bb);
    w.entities.list[i].no_physics = stuck;
    if stuck {
        let b = w.entities.list[i].bb;
        let (x, z) = (w.entities.list[i].pos.x, w.entities.list[i].pos.z);
        if !move_towards_closest_space(w, i, x, (b.min_y + b.max_y) / 2.0, z) {
            w.errors.push("an item entity inside a block (moveTowardsClosestSpace: the entity's own random, unknown for this item) is not modeled".into());
            return;
        }
    }
    let (on_ground, hd, phase) = {
        let e = &w.entities.list[i];
        (e.on_ground, e.delta_movement.horizontal_distance_sqr(), (e.tick_count + e.network_id).rem_euclid(4))
    };
    if on_ground && !(hd > 1.0e-5f32 as f64) && phase != 0 {
        // applyEffectsFromBlocksForLastMovements: last tick's moves replayed
        let fire_before = state(w, i).fire;
        collide::apply_effects_for_last_movements(w, i);
        apply_block_effects(w, i, fire_before);
        if w.entities.list[i].removed {
            return;
        }
    } else {
        let d = w.entities.list[i].delta_movement;
        crate::entity::move_(w, i, MoverType::Self_, d);
        let fire_before = state(w, i).fire;
        crate::entity::apply_effects_from_blocks(w, i);
        apply_block_effects(w, i, fire_before);
        if w.entities.list[i].removed {
            return;
        }
        let air = 0.98f32;
        let mut ground = air;
        if w.entities.list[i].on_ground {
            let below = collide::get_on_pos(w, &w.entities.list[i], 0.999999);
            ground *= crate::entity::boat::block_friction(&w.get(below));
        }
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.multiply(ground as f64, air as f64, ground as f64);
        if e.on_ground && e.delta_movement.y < 0.0 {
            e.delta_movement = e.delta_movement.multiply(1.0, -0.5, 1.0);
        }
    }
    let moved = {
        let e = &w.entities.list[i];
        mth::floor(e.xo) != mth::floor(e.pos.x) || mth::floor(e.yo) != mth::floor(e.pos.y) || mth::floor(e.zo) != mth::floor(e.pos.z)
    };
    let rate = if moved { 2 } else { 40 };
    if w.entities.list[i].tick_count % rate == 0 {
        merge_with_neighbours(w, i);
    }
    if w.entities.list[i].removed {
        return;
    }
    let mut s = state(w, i);
    if s.age != -32768 {
        s.age += 1;
    }
    let age = s.age;
    set_state(w, i, s);
    crate::entity::fluid_push::update_fluid_interaction(w, i);
    let _ = old_movement;
    if age >= 6000 {
        w.entities.list[i].removed = true;
    }
}

/// `Entity.moveTowardsClosestSpace(x, y, z)` of entity `i` (stuck inside a block): toward the
/// nearest of the cell's north, south, west, east and top faces whose neighbour is not a full
/// collision block (up when none), at `random.nextFloat() * 0.2 + 0.1` along that axis, the other
/// axes' motion times 0.75. False when the entity's own random is unknown (nothing changed).
fn move_towards_closest_space(w: &mut World, i: usize, x: f64, y: f64, z: f64) -> bool {
    use crate::pos::{BlockPos, Dir};
    let pos = BlockPos::new(mth::floor(x), mth::floor(y), mth::floor(z));
    let delta = [x - pos.x as f64, y - pos.y as f64, z - pos.z as f64];
    let mut closest_dir = Dir::Up;
    let mut closest = f64::MAX;
    for d in [Dir::North, Dir::South, Dir::West, Dir::East, Dir::Up] {
        let np = pos.rel(d);
        if crate::behavior::collision_full_block_at(w, &w.get(np), np) {
            continue;
        }
        let axis = match d {
            Dir::West | Dir::East => 0,
            Dir::Down | Dir::Up => 1,
            Dir::North | Dir::South => 2,
        };
        let positive = matches!(d, Dir::East | Dir::Up | Dir::South);
        let oriented = if positive { 1.0 - delta[axis] } else { delta[axis] };
        if oriented < closest {
            closest = oriented;
            closest_dir = d;
        }
    }
    let Some(r) = w.entities.list[i].random.as_mut() else { return false };
    let speed = r.next_float() * 0.2 + 0.1;
    let step: f32 = if matches!(closest_dir, Dir::East | Dir::Up | Dir::South) { 1.0 } else { -1.0 };
    let v = (step * speed) as f64;
    let m = w.entities.list[i].delta_movement;
    let (sx, sy, sz) = (m.x * 0.75, m.y * 0.75, m.z * 0.75);
    w.entities.list[i].delta_movement = match closest_dir {
        Dir::West | Dir::East => Vec3::new(v, sy, sz),
        Dir::Down | Dir::Up => Vec3::new(sx, v, sz),
        Dir::North | Dir::South => Vec3::new(sx, sy, v),
    };
    true
}
