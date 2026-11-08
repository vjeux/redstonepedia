//! What the 3D viewer draws of the mob layer (mobviewer 2026-11-08): one read-only entry per mob,
//! in entity order, from the same world the judge runs ([`super::farm::Run`]).

use super::types::MobType;
use super::villager::Activity;
use crate::block::Block;
use crate::entity::EntityKind;
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// A mob's pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MobPose {
    Standing,
    /// In bed (`getSleepingPos()`, pose SLEEPING): the bed's head cell and its facing.
    Sleeping { bed_head: BlockPos, facing: Dir },
    /// On a vehicle (the zombie in its boat): the vehicle's entity id.
    Riding { vehicle: u32 },
}

/// One mob as the viewer draws it. Its position, previous position and pitch are the core
/// entity's (`w.entities.list[index]`: `pos`, `xo`/`yo`/`zo`, `x_rot`).
#[derive(Clone, Debug)]
pub struct MobView {
    /// Its index in `w.entities.list`.
    pub index: usize,
    pub id: u32,
    pub ty: MobType,
    /// The body yaw and its previous tick's. The mob layer keeps one yaw (`Entity.y_rot`,
    /// `y_rot_o`): no separate `yBodyRot`.
    pub body_yaw: f32,
    pub body_yaw_o: f32,
    /// The head yaw and its previous tick's: the same yaw (no `yHeadRot`: LookControl is not
    /// modeled, a villager does not turn its head to the zombie).
    pub head_yaw: f32,
    pub head_yaw_o: f32,
    pub pose: MobPose,
    pub dead: bool,
    pub death_time: i32,
    pub hurt_time: i32,
    /// Burning (`remainingFireTicks > 0`: the client's `displayFireAnimation`).
    pub on_fire: bool,
    /// A villager's activity (its brain's: Rest, Panic, Idle...).
    pub activity: Option<Activity>,
}

/// Every mob of the world, in entity order. No walk animation: the mob layer keeps no
/// `WalkAnimationState`, so the viewer derives it from the position deltas, as the client does.
pub fn view(w: &World) -> Vec<MobView> {
    let mut out = Vec::new();
    for (index, e) in w.entities.list.iter().enumerate() {
        let EntityKind::Mob(m) = &e.kind else { continue };
        let pose = match (m.sleeping_pos, e.vehicle) {
            (Some(bed_head), _) => {
                let facing = match w.get(bed_head) {
                    Block::Bed { facing, .. } => facing,
                    _ => Dir::North,
                };
                MobPose::Sleeping { bed_head, facing }
            }
            (None, Some(vehicle)) => MobPose::Riding { vehicle },
            (None, None) => MobPose::Standing,
        };
        out.push(MobView {
            index,
            id: e.id,
            ty: m.ty,
            body_yaw: e.y_rot,
            body_yaw_o: e.y_rot_o,
            head_yaw: e.y_rot,
            head_yaw_o: e.y_rot_o,
            pose,
            dead: m.dead,
            death_time: m.death_time,
            hurt_time: m.hurt_time,
            on_fire: m.fire_ticks > 0,
            activity: m.brain.as_deref().map(|b| b.activity),
        });
    }
    out
}
