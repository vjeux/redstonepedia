//! The walking input of a mob whose AI is not modeled (ironfarm). The game's goals and
//! navigation end in `MoveControl.tick` (the yaw toward the next node, `Mob.setSpeed(modifier x
//! MOVEMENT_SPEED)`, which sets `zza`) and `JumpControl` (a jump when the path climbs). Redsim
//! does not guess them: the judge runs a mob under each of several inputs ([`Walk`]: standing
//! still, and walking each of 8 ways at the fastest goal speed, jumping when blocked) and requires
//! the same outcome under every one. A real golem's choices are a sequence of such inputs; the
//! policies bound them, they do not reproduce them.

use super::{mob, mob_mut};
use crate::world::World;

/// A walking policy.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Walk {
    /// No input (`xxa = zza = 0`, no jump).
    #[default]
    Still,
    /// Walk with yaw `yaw` at `modifier` x the mob's `MOVEMENT_SPEED`, jumping from the ground when
    /// a horizontal collision stops it.
    Toward { yaw: f32, modifier: f32 },
}

impl Walk {
    /// The policies the judge runs for an iron golem: still, then the 8 directions at its fastest
    /// goal speed (`MeleeAttackGoal` 1.0).
    pub fn golem_policies() -> Vec<Walk> {
        let mut v = vec![Walk::Still];
        for k in 0..8 {
            v.push(Walk::Toward { yaw: k as f32 * 45.0, modifier: 1.0 });
        }
        v
    }

    pub fn name(&self) -> String {
        match self {
            Walk::Still => "still".into(),
            Walk::Toward { yaw, modifier } => format!("walk yaw {yaw} x{modifier}"),
        }
    }
}

/// `Mob.serverAiStep`'s controls for a mob driven by the judge: `MoveControl` (yaw, `setSpeed`,
/// `zza`) and `JumpControl`.
pub fn apply(w: &mut World, i: usize) {
    let walk = mob(w, i).walk;
    match walk {
        Walk::Still => {
            let m = mob_mut(w, i);
            m.xxa = 0.0;
            m.zza = 0.0;
            m.jumping = false;
        }
        Walk::Toward { yaw, modifier } => {
            let blocked = w.entities.list[i].horizontal_collision && w.entities.list[i].on_ground;
            w.entities.list[i].y_rot = yaw;
            let m = mob_mut(w, i);
            let speed = (modifier as f64 * m.ty.def().speed) as f32;
            m.speed = speed;
            m.zza = speed;
            m.xxa = 0.0;
            m.jumping = blocked;
        }
    }
}
