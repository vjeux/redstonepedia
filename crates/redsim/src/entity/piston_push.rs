//! Entities moved by moving blocks (clk-misc, 2026-10-20), read from 26.3's
//! `PistonMovingBlockEntity`: in a moving block's tick, before its progress advances by 0.5,
//! `moveCollidedEntities` then `moveStuckEntities` (honey).
//!
//! * `moveCollidedEntities`: the moving block's shape (`getCollisionRelatedBlockState`: a
//!   retracting base sweeps as a piston head, short once progress > 0.25; otherwise the moved
//!   state, a moving head included) at its current offset; every entity meeting the box swept
//!   by this tick's 0.5 (`PistonMath.getMovementArea`, `minmax` with the box) and not
//!   `IGNORE_ENTITY` is pushed by how deep the swept boxes reach into it (at most 0.5) plus
//!   0.01, along the movement. A moving SLIME block first sets the entity's motion on the
//!   movement axis to the direction's step (+-1). A retracting base then pulls an entity it
//!   left inside the base cell back out (`fixEntityWithinPistonBase`).
//! * `moveEntityByPiston`: `NOCLIP` = the push direction (moving blocks going that way lose
//!   their collision, `shape::moving_noclip`), `Entity.move(PISTON, ..)` (`limitPistonMovement`:
//!   one axis, at most 0.51 per axis per game tick, `collide.rs`), `applyEffectsFromBlocks(from,
//!   to)`, and the move's recording dropped (`removeLatestMovementRecording`).
//! * `moveStuckEntities`: a moving HONEY block drags, on a horizontal move, the entities standing
//!   on it (on the ground, supported by it or centered over its box up to 1.5 above).
//!
//! Every modeled kind (boats, minecarts, items, non-marker armor stands, primed TNT) is
//! `PUSH_PULL`, and falling blocks (`gravity.rs`) are pushed too (a block moved off its column
//! or given sideways motion by a slime bounce then takes the general move,
//! `gravity::move_general`); the targets go in id order, which stands in for `level.getEntities`
//! (one entity section: spawn order; entities in several sections would be ordered by section).
//! Pushes of different entities do not interact, so only the per-entity order matters.

use super::vec3::{jmax, jmin};
use super::{Aabb, MoverType, Vec3};
use crate::block::{Block, Simple};
use crate::piston::moving::MovingBe;
use crate::piston::shape::{self, B16};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `getExtendedProgress(progress)` (progress in halves).
fn extended_progress(be: &MovingBe) -> f64 {
    let p = be.progress as f32 * 0.5;
    (if be.extending { p - 1.0 } else { 1.0 - p }) as f64
}

/// `moveByPositionAndProgress(pos, box)`.
fn move_by_progress(p: BlockPos, b: Aabb, be: &MovingBe) -> Aabb {
    let cur = extended_progress(be);
    let st = be.direction.step();
    b.move_by(p.x as f64 + cur * st[0] as f64, p.y as f64 + cur * st[1] as f64, p.z as f64 + cur * st[2] as f64)
}

fn to_aabb(b: B16) -> Aabb {
    let f = |v: i32| v as f64 / 16.0;
    Aabb::new(f(b.0[0]), f(b.0[2]), f(b.0[4]), f(b.0[1]), f(b.0[3]), f(b.0[5]))
}

/// `getCollisionRelatedBlockState().getCollisionShape(level, pos)` (a moved terrain block's boxes
/// exact: tickexact 2026-11-11, cart-pole).
fn related_shape(be: &MovingBe) -> Vec<Aabb> {
    match be.moved {
        Block::Piston { facing, .. } if !be.extending && be.source => shape::head(facing, be.progress >= 1).into_iter().map(to_aabb).collect(),
        b => shape::block_shape_exact(b).into_iter().map(|c| Aabb::new(c[0], c[1], c[2], c[3], c[4], c[5])).collect(),
    }
}

/// `PistonMath.getMovementArea(box, direction, amount)`.
fn movement_area(b: Aabb, d: Dir, amount: f64) -> Aabb {
    let delta = amount * if matches!(d, Dir::East | Dir::Up | Dir::South) { 1.0 } else { -1.0 };
    let (lo, hi) = (jmin(delta, 0.0), jmax(delta, 0.0));
    match d {
        Dir::West => Aabb::new(b.min_x + lo, b.min_y, b.min_z, b.min_x + hi, b.max_y, b.max_z),
        Dir::East => Aabb::new(b.max_x + lo, b.min_y, b.min_z, b.max_x + hi, b.max_y, b.max_z),
        Dir::Down => Aabb::new(b.min_x, b.min_y + lo, b.min_z, b.max_x, b.min_y + hi, b.max_z),
        Dir::Up => Aabb::new(b.min_x, b.max_y + lo, b.min_z, b.max_x, b.max_y + hi, b.max_z),
        Dir::North => Aabb::new(b.min_x, b.min_y, b.min_z + lo, b.max_x, b.max_y, b.min_z + hi),
        Dir::South => Aabb::new(b.min_x, b.min_y, b.max_z + lo, b.max_x, b.max_y, b.max_z + hi),
    }
}

/// `getMovement(boxToBeOutsideOf, movement, box)`.
fn get_movement(outside: Aabb, d: Dir, b: Aabb) -> f64 {
    match d {
        Dir::East => outside.max_x - b.min_x,
        Dir::West => b.max_x - outside.min_x,
        Dir::Up => outside.max_y - b.min_y,
        Dir::Down => b.max_y - outside.min_y,
        Dir::South => outside.max_z - b.min_z,
        Dir::North => b.max_z - outside.min_z,
    }
}

/// `Entity.getPistonPushReaction() != IGNORE_ENTITY` (every modeled kind pushes and pulls).
fn pushable(_e: &super::Entity) -> bool {
    true
}

/// The moving block at `p` ticks from `be.progress` to `be.progress + 0.5`: the entities it
/// moves (`PistonMovingBlockEntity.tick`, before `progress = newProgress`).
pub fn tick(w: &mut World, p: BlockPos, be: MovingBe) {
    if w.entities.list.iter().all(|e| e.removed) && w.gravity.entities.iter().all(|e| e.removed) {
        return;
    }
    move_collided_entities(w, p, be);
    move_stuck_entities(w, p, be);
}

/// An entity a moving block can move: one of the entity list, or a falling block (`gravity.rs`).
#[derive(Clone, Copy, Debug)]
enum Target {
    Main(usize),
    Falling(usize),
}

fn target_bb(w: &World, t: Target) -> Aabb {
    match t {
        Target::Main(i) => w.entities.list[i].bb,
        Target::Falling(j) => w.gravity.entities[j].bb(),
    }
}

/// The slime bounce: the motion's component on `axis` set to `v`.
fn set_axis_motion(w: &mut World, t: Target, axis: usize, v: f64) {
    match t {
        Target::Main(i) => {
            let e = &mut w.entities.list[i];
            e.delta_movement = e.delta_movement.with_axis(axis, v);
        }
        Target::Falling(j) => {
            let e = &mut w.gravity.entities[j];
            match axis {
                0 => e.vx = v,
                1 => e.vy = v,
                _ => e.vz = v,
            }
        }
    }
}

fn move_by_piston(w: &mut World, t: Target, piston_direction: Dir, delta: f64, movement: Dir) {
    match t {
        Target::Main(i) => move_entity_by_piston(w, i, piston_direction, delta, movement),
        Target::Falling(j) => move_falling_by_piston(w, j, piston_direction, delta, movement),
    }
}

/// `moveCollidedEntities(level, pos, progress + 0.5, self)`.
fn move_collided_entities(w: &mut World, p: BlockPos, be: MovingBe) {
    let movement = be.push_direction();
    let delta_progress = 0.5f64;
    let shapes = related_shape(&be);
    if shapes.is_empty() {
        return;
    }
    let bounds = shapes.iter().skip(1).fold(shapes[0], |a, b| a.minmax(*b));
    let aabb = move_by_progress(p, bounds, &be);
    let query = movement_area(aabb, movement, delta_progress).minmax(aabb);
    // the entities and falling blocks meeting the query box, in id (= spawn) order
    let mut found: Vec<(u32, Target)> = w.get_entities(query, pushable).into_iter().map(|i| (w.entities.list[i].id, Target::Main(i))).collect();
    for (j, e) in w.gravity.entities.iter().enumerate() {
        if !e.removed && e.bb().intersects(query) {
            found.push((e.id, Target::Falling(j)));
        }
    }
    if found.is_empty() {
        return;
    }
    found.sort_by_key(|(id, _)| *id);
    let bounce = be.moved == Block::Simple(Simple::SlimeBlock);
    let step = movement.step();
    let axis = match movement {
        Dir::East | Dir::West => 0,
        Dir::Up | Dir::Down => 1,
        Dir::North | Dir::South => 2,
    };
    for (_, t) in found {
        let removed = match t {
            Target::Main(i) => w.entities.list[i].removed,
            Target::Falling(j) => w.gravity.entities[j].removed,
        };
        if removed {
            continue;
        }
        if bounce {
            // `canSimulateMovement`: server-side entities with no controlling player
            set_axis_motion(w, t, axis, step[axis] as f64);
        }
        let mut delta = 0.0f64;
        for s in &shapes {
            let moving = movement_area(move_by_progress(p, *s, &be), movement, delta_progress);
            let bb = target_bb(w, t);
            if moving.intersects(bb) {
                delta = jmax(delta, get_movement(moving, movement, bb));
                if delta >= delta_progress {
                    break;
                }
            }
        }
        if !(delta <= 0.0) {
            let d = jmin(delta, delta_progress) + 0.01;
            move_by_piston(w, t, movement, d, movement);
            if !be.extending && be.source {
                fix_entity_within_piston_base(w, t, p, movement, delta_progress);
            }
        }
    }
}

/// `moveEntityByPiston` for a falling block: `Entity.move(PISTON)` = `limitPistonMovement` and
/// the general move ([`crate::gravity::move_general`]) under `NOCLIP`.
fn move_falling_by_piston(w: &mut World, j: usize, piston_direction: Dir, delta: f64, movement: Dir) {
    w.entities.noclip = Some(piston_direction);
    let st = movement.step();
    let v = Vec3::new(delta * st[0] as f64, delta * st[1] as f64, delta * st[2] as f64);
    let time = w.time;
    let mut e = w.gravity.entities[j];
    let v = super::collide::limit_piston(&mut e.piston_deltas, &mut e.piston_deltas_time, time, v);
    if !(v.x == 0.0 && v.y == 0.0 && v.z == 0.0) {
        crate::gravity::move_general(w, &mut e, v);
    }
    w.gravity.entities[j] = e;
    w.entities.noclip = None;
}

/// `moveEntityByPiston(pistonDirection, entity, delta, movement)`.
fn move_entity_by_piston(w: &mut World, i: usize, piston_direction: Dir, delta: f64, movement: Dir) {
    w.entities.noclip = Some(piston_direction);
    let prev = w.entities.list[i].pos;
    let st = movement.step();
    super::move_(w, i, MoverType::Piston, Vec3::new(delta * st[0] as f64, delta * st[1] as f64, delta * st[2] as f64));
    let now = w.entities.list[i].pos;
    super::collide::apply_effects_from_blocks_between(w, i, prev, now);
    w.entities.list[i].movement_this_tick.pop();
    w.entities.noclip = None;
}

/// `fixEntityWithinPistonBase(pos, entity, direction, deltaProgress)`.
fn fix_entity_within_piston_base(w: &mut World, t: Target, p: BlockPos, direction: Dir, delta_progress: f64) {
    let bb = target_bb(w, t);
    let cell = Aabb::of_block(p);
    if bb.intersects(cell) {
        let opposite = direction.opposite();
        let delta = get_movement(cell, opposite, bb) + 0.01;
        let delta_intersected = get_movement(cell, opposite, bb.intersect(cell)) + 0.01;
        if (delta - delta_intersected).abs() < 0.01 {
            let d = jmin(delta, delta_progress) + 0.01;
            move_by_piston(w, t, direction, d, opposite);
        }
    }
}

/// `moveStuckEntities(level, pos, progress + 0.5, self)`: honey drags what stands on it.
fn move_stuck_entities(w: &mut World, p: BlockPos, be: MovingBe) {
    if be.moved != Block::Simple(Simple::HoneyBlock) {
        return;
    }
    let movement = be.push_direction();
    if matches!(movement, Dir::Up | Dir::Down) {
        return;
    }
    let sticky_top = shape::block_shape(be.moved).iter().map(|b| b.0[3]).max().unwrap_or(0) as f64 / 16.0;
    let aabb = move_by_progress(p, Aabb::new(0.0, sticky_top, 0.0, 1.0, 1.5000010000000001, 1.0), &be);
    let delta_progress = 0.5f64;
    let found = w.get_entities(aabb, |e| {
        e.on_ground && (e.main_support == Some(p) || (e.pos.x >= aabb.min_x && e.pos.x <= aabb.max_x && e.pos.z >= aabb.min_z && e.pos.z <= aabb.max_z))
    });
    for i in found {
        move_entity_by_piston(w, i, movement, delta_progress, movement);
    }
}
