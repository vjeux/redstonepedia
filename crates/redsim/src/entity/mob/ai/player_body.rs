//! The AFK player's body (tickexact's ServerPlayer: `Player.getMoveSimulationType` is
//! `AUTHORITATIVE_SIDE_AND_SERVER` and `isEffectiveAi` holds on the server, so the server runs its
//! `LivingEntity.travel` with no input after the level's tick): a mob touching it pushes it
//! (`Entity.push`), it slides with the block's friction and falls, and pushes the mobs it touches
//! back (`pushEntities`). Its standing box is 0.6 x 1.8. What it would do beyond that (swimming,
//! stepping up, fall damage, a bouncy or slow block, picking items up) is named in the run's
//! legacy list.

use crate::entity::{Aabb, EntityKind, Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// The player's motion and whether it stands on the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub delta: Vec3,
    pub on_ground: bool,
}

/// `Player.STANDING_DIMENSIONS.makeBoundingBox(pos)` (half of 0.6F, 1.8F tall).
pub fn bb(pos: Vec3) -> Aabb {
    let w = (0.6f32 / 2.0f32) as f64;
    Aabb::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + 1.8f32 as f64, pos.z + w)
}

fn body(w: &mut World, k: usize) -> &mut Body {
    if w.mob.ai.bodies.len() < w.mob.ai.players.len() {
        w.mob.ai.bodies.resize(w.mob.ai.players.len(), Body::default());
    }
    &mut w.mob.ai.bodies[k]
}

/// The players a mob's box meets (`getPushableEntities`: spectators are not pushable).
pub fn touching(w: &World, bb_: Aabb) -> Vec<usize> {
    (0..w.mob.ai.players.len()).filter(|&k| w.mob.ai.players[k].mode != super::GameMode::Spectator && bb(w.mob.ai.players[k].pos).intersects(bb_)).collect()
}

/// `Entity.push(Entity)` between player `k` (this) and entity `j`: each takes 0.05 of the
/// direction away from the other (scaled down past a block's distance).
fn push_pair(w: &mut World, k: usize, j: usize, player_first: bool) {
    let p = w.mob.ai.players[k].pos;
    let e = w.entities.list[j].pos;
    let (ax, az, bx, bz) = if player_first { (p.x, p.z, e.x, e.z) } else { (e.x, e.z, p.x, p.z) };
    let mut x = bx - ax;
    let mut z = bz - az;
    let mut distance = x.abs().max(z.abs());
    if distance < 0.01f32 as f64 {
        return;
    }
    distance = distance.sqrt();
    x /= distance;
    z /= distance;
    let power = (1.0 / distance).min(1.0);
    x *= power;
    z *= power;
    x *= 0.05f32 as f64;
    z *= 0.05f32 as f64;
    // `this` takes (-x, -z), the other (x, z); a mob carrying a rider is not pushed
    let mob_pushable = w.entities.list[j].passengers.is_empty() && crate::entity::minecart::is_pushable(&w.entities.list[j]);
    let (pk, pe) = if player_first { ((-x, -z), (x, z)) } else { ((x, z), (-x, -z)) };
    let b = body(w, k);
    b.delta = b.delta.add(pk.0, 0.0, pk.1);
    if mob_pushable {
        let e = &mut w.entities.list[j];
        e.delta_movement = e.delta_movement.add(pe.0, 0.0, pe.1);
    }
}

/// A mob's `doPush(player)` = `player.push(mob)`.
pub fn pushed_by_mob(w: &mut World, k: usize, j: usize) {
    push_pair(w, k, j, true);
}

fn legacy(w: &mut World, what: &'static str) {
    w.mob.ai.legacy.insert(what);
}

/// The players' own ticks after the level's (`ServerPlayer.doTick` -> `LivingEntity.aiStep`):
/// the small motions dropped, `travelInAir` with no input (the block's friction on the ground,
/// gravity), then `pushEntities`.
pub fn tick(w: &mut World) {
    for k in 0..w.mob.ai.players.len() {
        if w.mob.ai.players[k].mode == super::GameMode::Spectator {
            continue;
        }
        let pos = w.mob.ai.players[k].pos;
        let mut b = *body(w, k);
        let mut d = b.delta;
        // LivingEntity.aiStep: a player's horizontal motion goes only when all of it is under
        // 0.003 (horizontalDistanceSqr < 9e-6), not axis by axis
        if d.x * d.x + d.z * d.z < 9.0e-6 {
            d.x = 0.0;
            d.z = 0.0;
        }
        if d.y.abs() < 0.003 {
            d.y = 0.0;
        }
        let here = bb(pos);
        if crate::entity::mob::blocks::box_has_liquid(w, here.deflate(0.001, 0.001, 0.001)) {
            legacy(w, "the AFK player in a fluid (its swimming and fluid pushing)");
        }
        // travelInAir: getBlockPosBelowThatAffectsMyMovement (0.5000001 down)
        let below = BlockPos::new(mth::floor(pos.x), mth::floor(pos.y - 0.5000001), mth::floor(pos.z));
        let block_friction = if b.on_ground { crate::entity::mob::blocks::friction(&w.get(below)) } else { 1.0f32 };
        let friction = block_friction * 0.91f32;
        let moved = crate::entity::collide::collide_box(w, here, d);
        let x_col = !mth::equal(d.x, moved.x);
        let z_col = !mth::equal(d.z, moved.z);
        let vertical = d.y != moved.y;
        if (x_col || z_col) && (b.on_ground || vertical && d.y < 0.0) {
            legacy(w, "the AFK player pushed against a block (its step up)");
        }
        let np = pos.add(moved.x, moved.y, moved.z);
        let on_ground = vertical && d.y < 0.0;
        let mut nd = Vec3::new(if x_col { 0.0 } else { d.x }, d.y, if z_col { 0.0 } else { d.z });
        if vertical {
            // Block.updateEntityMovementAfterFallOn (a plain block): the vertical motion stops
            nd = Vec3::new(nd.x, 0.0, nd.z);
        }
        // the bridge's AFK player has no gravity (setNoGravity) and a creative one flies
        // (Player.travel: its vertical motion back to 0.6 of what it was)
        let my = nd.y;
        let flying = w.mob.ai.players[k].mode == super::GameMode::Creative;
        let vy = if flying { d.y * 0.6 } else { my * 0.98f32 as f64 };
        b.delta = Vec3::new(nd.x * friction as f64, vy, nd.z * friction as f64);
        b.on_ground = on_ground;
        w.mob.ai.players[k].pos = np;
        // the other copies of the AFK player's position (the world's, natural spawning's)
        if np != pos {
            w.mob.ai.player_moved = true;
            if w.afk == Some(pos) {
                w.afk = Some(np);
            }
            if let Some(n) = w.mob.natural.as_mut() {
                for q in n.players.iter_mut() {
                    if *q == pos {
                        *q = np;
                    }
                }
            }
        }
        *body(w, k) = b;
        // pushEntities: each mob its box meets, `mob.push(player)`
        let others = w.get_entities(bb(np), |e| matches!(e.kind, EntityKind::Mob(_)) && !e.removed && crate::entity::minecart::is_pushable(e));
        for j in others {
            push_pair(w, k, j, false);
        }
        // Player.aiStep: the items within its box grown by (1, 0.5, 1) it would touch
        let reach = bb(np).inflate(1.0, 0.5, 1.0);
        if w.mob.ai.players[k].mode == super::GameMode::Survival && !w.get_entities(reach, |e| matches!(e.kind, EntityKind::Item(_)) && !e.removed).is_empty() {
            legacy(w, "the AFK player picking up an item");
        }
    }
}
