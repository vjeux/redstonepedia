//! Boats (clk-misc, redsim M3): `AbstractBoat` (oak boat, oak chest boat) read in the 26.3
//! source, server side, no passengers (no paddling, no rotation: `controlBoat` runs on the
//! client only and `deltaRotation` stays 0).
//!
//! The tick, in the game's order: `oldStatus = status; status = getStatus()` (UNDER_WATER /
//! UNDER_FLOWING_WATER from the water above the box's top, IN_WATER from the water under its
//! bottom, else ON_LAND with the mean friction of the blocks under it, else IN_AIR),
//! `outOfControlTicks`, `Entity.tick` (`baseTick`: fluid pushing), `floatBoat` (snap onto the
//! water after a fall; else friction, gravity and buoyancy per status), `move(SELF, delta)`,
//! `applyEffectsFromBlocks` TWICE (plates, tripwire, bubble columns: [`crate::react`]),
//! `tickBubbleColumn` (60 gt above a column, then launched: 0.6 up or 0.7 down), then pushing
//! the entities around it.

use crate::entity::{self, Aabb, MoverType, Vec3, mth};
use crate::fluid::{self, FluidState};
use crate::pos::BlockPos;
use crate::world::World;

/// `EntityTypes.OAK_BOAT.sized(1.375F, 0.5625F)`.
pub const WIDTH: f32 = 1.375;
pub const HEIGHT: f32 = 0.5625;

/// `AbstractBoat.Status`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    InWater,
    UnderWater,
    UnderFlowingWater,
    OnLand,
    InAir,
}

/// The boat's own fields (`AbstractBoat`), server side.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BoatState {
    /// A chest boat (`ChestBoat`: a container hoppers fill and empty).
    pub chest: bool,
    /// `status` / `oldStatus` (null until the first tick).
    pub status: Option<Status>,
    pub old_status: Option<Status>,
    pub out_of_control_ticks: f32,
    pub delta_rotation: f32,
    pub water_level: f64,
    pub land_friction: f32,
    pub last_yd: f64,
    pub is_above_bubble_column: bool,
    pub bubble_column_direction_is_down: bool,
    /// `DATA_ID_BUBBLE_TIME`.
    pub bubble_time: i32,
}

impl Default for BoatState {
    fn default() -> BoatState {
        BoatState {
            chest: false,
            status: None,
            old_status: None,
            out_of_control_ticks: 0.0,
            delta_rotation: 0.0,
            water_level: 0.0,
            land_friction: 0.0,
            last_yd: 0.0,
            is_above_bubble_column: false,
            bubble_column_direction_is_down: false,
            bubble_time: 0,
        }
    }
}

/// `getDefaultGravity`.
pub const GRAVITY: f64 = 0.04;

fn st(w: &World, i: usize) -> BoatState {
    match &w.entities.list[i].kind {
        entity::EntityKind::Boat(b) => *b,
        _ => BoatState::default(),
    }
}

fn st_mut(w: &mut World, i: usize) -> &mut BoatState {
    match &mut w.entities.list[i].kind {
        entity::EntityKind::Boat(b) => b,
        _ => unreachable!("not a boat"),
    }
}

fn ceil(v: f64) -> i32 {
    v.ceil() as i32
}

fn water_at(w: &World, p: BlockPos) -> Option<FluidState> {
    let f = fluid::fluid_state(w, p);
    if f.fluid.is_water() { Some(f) } else { None }
}

/// `AbstractBoat.isUnderwater` (the status form): water above the box's top.
fn is_underwater(w: &World, bb: Aabb) -> Option<Status> {
    let max_y = bb.max_y + 0.001;
    let (x0, x1) = (mth::floor(bb.min_x), ceil(bb.max_x));
    let (y0, y1) = (mth::floor(bb.max_y), ceil(max_y));
    let (z0, z1) = (mth::floor(bb.min_z), ceil(bb.max_z));
    let mut under = false;
    for x in x0..x1 {
        for y in y0..y1 {
            for z in z0..z1 {
                let p = BlockPos::new(x, y, z);
                if let Some(f) = water_at(w, p)
                    && max_y < (p.y as f32 + f.height(w, p)) as f64
                {
                    if !f.is_source() {
                        return Some(Status::UnderFlowingWater);
                    }
                    under = true;
                }
            }
        }
    }
    if under { Some(Status::UnderWater) } else { None }
}

/// `checkInWater`: water under the box's bottom; sets `waterLevel` (the highest surface met).
fn check_in_water(w: &World, bb: Aabb, water_level: &mut f64) -> bool {
    let (x0, x1) = (mth::floor(bb.min_x), ceil(bb.max_x));
    let (y0, y1) = (mth::floor(bb.min_y), ceil(bb.min_y + 0.001));
    let (z0, z1) = (mth::floor(bb.min_z), ceil(bb.max_z));
    let mut in_water = false;
    *water_level = -f64::MAX;
    for x in x0..x1 {
        for y in y0..y1 {
            for z in z0..z1 {
                let p = BlockPos::new(x, y, z);
                if let Some(f) = water_at(w, p) {
                    // `float height = y + fluidState.getHeight(...)`: float arithmetic
                    let height = y as f32 + f.height(w, p);
                    *water_level = water_level.max(height as f64);
                    in_water |= bb.min_y < height as f64;
                }
            }
        }
    }
    in_water
}

/// `getGroundFriction`: the mean friction of the blocks whose collision shape meets the 0.001
/// thick slab under the box (lily pads excluded; NaN when there are none).
fn ground_friction(w: &World, bb: Aabb) -> f32 {
    let bx = Aabb::new(bb.min_x, bb.min_y - 0.001, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
    let (x0, x1) = (mth::floor(bx.min_x) - 1, ceil(bx.max_x) + 1);
    let (y0, y1) = (mth::floor(bx.min_y) - 1, ceil(bx.max_y) + 1);
    let (z0, z1) = (mth::floor(bx.min_z) - 1, ceil(bx.max_z) + 1);
    let mut friction = 0.0f32;
    let mut count = 0i32;
    for x in x0..x1 {
        for z in z0..z1 {
            let edges = (if x != x0 && x != x1 - 1 { 0 } else { 1 }) + (if z != z0 && z != z1 - 1 { 0 } else { 1 });
            if edges == 2 {
                continue;
            }
            for y in y0..y1 {
                if edges <= 0 || (y != y0 && y != y1 - 1) {
                    let p = BlockPos::new(x, y, z);
                    let b = w.get(p);
                    let hit = crate::behavior::collision_boxes(&b).iter().any(|c| {
                        let f = |v: i32| v as f64 / 16.0;
                        Aabb::new(x as f64 + f(c[0]), y as f64 + f(c[2]), z as f64 + f(c[4]), x as f64 + f(c[1]), y as f64 + f(c[3]), z as f64 + f(c[5])).intersects(bx)
                    });
                    if hit {
                        friction += block_friction(&b);
                        count += 1;
                    }
                }
            }
        }
    }
    friction / count as f32
}

/// `Block.getFriction`.
pub fn block_friction(b: &crate::block::Block) -> f32 {
    use crate::block::{Block, Simple};
    match *b {
        Block::Simple(Simple::PackedIce) => 0.98,
        Block::Simple(Simple::SlimeBlock) => 0.8,
        _ => 0.6,
    }
}

/// `getStatus` (sets `waterLevel` / `landFriction` on the way).
fn get_status(w: &mut World, i: usize) -> Status {
    let bb = w.entities.list[i].bb;
    if let Some(s) = is_underwater(w, bb) {
        st_mut(w, i).water_level = bb.max_y;
        return s;
    }
    let mut level = 0.0;
    let in_water = check_in_water(w, bb, &mut level);
    st_mut(w, i).water_level = level;
    if in_water {
        return Status::InWater;
    }
    let friction = ground_friction(w, bb);
    if friction > 0.0 {
        st_mut(w, i).land_friction = friction;
        Status::OnLand
    } else {
        Status::InAir
    }
}

/// `getWaterLevelAbove`.
fn water_level_above(w: &World, bb: Aabb, last_yd: f64) -> f32 {
    let (x0, x1) = (mth::floor(bb.min_x), ceil(bb.max_x));
    let (y0, y1) = (mth::floor(bb.max_y), ceil(bb.max_y - last_yd));
    let (z0, z1) = (mth::floor(bb.min_z), ceil(bb.max_z));
    'rows: for y in y0..y1 {
        let mut block_height = 0.0f32;
        for x in x0..x1 {
            for z in z0..z1 {
                let p = BlockPos::new(x, y, z);
                if let Some(f) = water_at(w, p) {
                    block_height = mth::jmaxf(block_height, f.height(w, p));
                }
                if block_height >= 1.0 {
                    continue 'rows;
                }
            }
        }
        if block_height < 1.0 {
            return y as f32 + block_height;
        }
    }
    (y1 + 1) as f32
}

/// `floatBoat`.
fn float_boat(w: &mut World, i: usize) {
    let s = st(w, i);
    let mut vspeed = -GRAVITY;
    let mut buoyancy = 0.0f64;
    let mut inv_friction = 0.05f32;
    let status = s.status;
    if s.old_status == Some(Status::InAir) && status != Some(Status::InAir) && status != Some(Status::OnLand) {
        let e = &w.entities.list[i];
        st_mut(w, i).water_level = e.pos.y + HEIGHT as f64;
        let e = &w.entities.list[i];
        let target_y = (water_level_above(w, e.bb, s.last_yd) - HEIGHT) as f64 + 0.101;
        let moved = e.bb.move_by(0.0, target_y - e.pos.y, 0.0);
        if entity::no_collision(w, i, moved) {
            let p = Vec3::new(w.entities.list[i].pos.x, target_y, w.entities.list[i].pos.z);
            w.entities.set_pos(i, p);
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.multiply(1.0, 0.0, 1.0);
            st_mut(w, i).last_yd = 0.0;
        }
        st_mut(w, i).status = Some(Status::InWater);
        return;
    }
    match status {
        Some(Status::InWater) => {
            buoyancy = (s.water_level - w.entities.list[i].pos.y) / HEIGHT as f64;
            inv_friction = 0.9;
        }
        Some(Status::UnderFlowingWater) => {
            vspeed = -7.0e-4;
            inv_friction = 0.9;
        }
        Some(Status::UnderWater) => {
            buoyancy = 0.01f32 as f64;
            inv_friction = 0.45;
        }
        Some(Status::InAir) => inv_friction = 0.9,
        Some(Status::OnLand) => inv_friction = s.land_friction,
        None => {}
    }
    let e = &mut w.entities.list[i];
    let m = e.delta_movement;
    e.delta_movement = Vec3::new(m.x * inv_friction as f64, m.y + vspeed, m.z * inv_friction as f64);
    if buoyancy > 0.0 {
        let d = e.delta_movement;
        e.delta_movement = Vec3::new(d.x, (d.y + buoyancy * (GRAVITY / 0.65)) * 0.75, d.z);
    }
    st_mut(w, i).delta_rotation *= inv_friction;
}

/// `onAboveBubbleColumn` (the boat's override, server side; the 1/100 splash sound and its
/// game event are not modeled).
pub fn on_above_bubble_column(w: &mut World, i: usize, drag_down: bool) {
    let s = st_mut(w, i);
    s.is_above_bubble_column = true;
    s.bubble_column_direction_is_down = drag_down;
    if s.bubble_time == 0 {
        s.bubble_time = 60;
    }
}

/// `tickBubbleColumn` (server side).
fn tick_bubble_column(w: &mut World, i: usize) {
    let s = st_mut(w, i);
    if !s.is_above_bubble_column {
        s.bubble_time = 0;
    }
    let mut bubble_time = s.bubble_time;
    if bubble_time > 0 {
        bubble_time -= 1;
        s.bubble_time = bubble_time;
        let diff = 60 - bubble_time - 1;
        let down = s.bubble_column_direction_is_down;
        if diff > 0 && bubble_time == 0 {
            s.bubble_time = 0;
            handle_bubble_column_effect(w, i, down);
        }
        st_mut(w, i).is_above_bubble_column = false;
    }
}

/// `handleBubbleColumnEffect`.
fn handle_bubble_column_effect(w: &mut World, i: usize, drag_down: bool) {
    let e = &mut w.entities.list[i];
    let m = e.delta_movement;
    e.delta_movement = if drag_down { m.add(0.0, -0.7, 0.0) } else { Vec3::new(m.x, 0.6, m.z) };
}

/// `checkFallDamage` (the boat's override, called at the end of `move`): remembers the last
/// vertical motion; fall distance unless above water.
pub fn check_fall_damage(w: &mut World, i: usize, ya: f64, on_ground: bool) {
    let dy = w.entities.list[i].delta_movement.y;
    st_mut(w, i).last_yd = dy;
    let below = w.entities.list[i].block_position().below();
    let water_below = water_at(w, below).is_some();
    let e = &mut w.entities.list[i];
    if on_ground {
        e.fall_distance = 0.0;
    } else if !water_below && ya < 0.0 {
        e.fall_distance -= ya as f32 as f64;
    }
}

/// `isUnderWater` (the boat's override).
pub fn is_under_water(w: &World, i: usize) -> bool {
    matches!(st(w, i).status, Some(Status::UnderWater) | Some(Status::UnderFlowingWater))
}

/// `AbstractBoat.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    let status = get_status(w, i);
    {
        let s = st_mut(w, i);
        s.old_status = s.status;
        s.status = Some(status);
        if status != Status::UnderWater && status != Status::UnderFlowingWater {
            s.out_of_control_ticks = 0.0;
        } else {
            s.out_of_control_ticks += 1.0;
        }
    }
    // the damage an explosion left decays (`setDamage(getDamage() - 1)`)
    {
        let e = &mut w.entities.list[i];
        if e.vehicle_damage > 0.0 {
            e.vehicle_damage -= 1.0;
        }
    }
    // `super.tick()` = `Entity.baseTick`: fluid heights and pushing, fire, firstTick
    entity::base_tick(w, i);
    float_boat(w, i);
    let dm = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, dm);
    entity::apply_effects_from_blocks(w, i);
    entity::apply_effects_from_blocks(w, i);
    tick_bubble_column(w, i);
    entity::push::boat_neighbors(w, i);
}
