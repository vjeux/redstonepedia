//! The four controls (26.3 `net.minecraft.world.entity.ai.control`): `MoveControl` (the yaw
//! toward the wanted position, the speed into `zza`, the jump test), `LookControl` (the head yaw and
//! pitch toward a point), `JumpControl` and `BodyRotationControl` (the body yaw following the head
//! or the motion). Their state lives in [`super::MobAi`]; the functions here change the entity and
//! its mob state like the game's `tick`s.

use crate::entity::mth;

/// `Control.rotateTowards(from, to, maxRot)`.
#[inline]
pub fn rotate_towards(from: f32, to: f32, max_rot: f32) -> f32 {
    let diff = mth::wrap_degrees(to - from);
    let clamped = mth::clampf(diff, -max_rot, max_rot);
    from + clamped
}

/// `Mth.rotateIfNecessary(base, target, maxAngleDiff)`.
#[inline]
pub fn rotate_if_necessary(base: f32, target: f32, max_diff: f32) -> f32 {
    let delta = mth::wrap_degrees(target - base);
    let clamped = mth::clampf(delta, -max_diff, max_diff);
    target - clamped
}

/// `(float)Math.PI`.
pub const PI_F: f32 = std::f32::consts::PI;

/// The controls' angle formula `(float)(Mth.atan2(y, x) * 180.0F / (float)Math.PI)`.
#[inline]
pub fn atan2_degrees(y: f64, x: f64) -> f32 {
    (mth::atan2(y, x) * 180.0f32 as f64 / PI_F as f64) as f32
}

/// `MoveControl.Operation`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Operation {
    #[default]
    Wait,
    MoveTo,
    Strafe,
    Jumping,
}

/// `MoveControl`'s state.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MoveControl {
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
    pub speed_modifier: f64,
    pub strafe_forwards: f32,
    pub strafe_right: f32,
    pub operation: Operation,
}

impl MoveControl {
    pub fn has_wanted(&self) -> bool {
        self.operation == Operation::MoveTo
    }

    /// `setWantedPosition(x, y, z, speedModifier)`.
    pub fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed;
        if self.operation != Operation::Jumping {
            self.operation = Operation::MoveTo;
        }
    }

    /// `strafe(forwards, right)`.
    pub fn strafe(&mut self, forwards: f32, right: f32) {
        self.operation = Operation::Strafe;
        self.strafe_forwards = forwards;
        self.strafe_right = right;
        self.speed_modifier = 0.25;
    }
}

/// `MoveControl.rotlerp(a, b, max)` (kept in [0, 360]).
pub fn rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let mut diff = mth::wrap_degrees(b - a);
    if diff > max {
        diff = max;
    }
    if diff < -max {
        diff = -max;
    }
    let mut result = a + diff;
    if result < 0.0 {
        result += 360.0;
    } else if result > 360.0 {
        result -= 360.0;
    }
    result
}

/// `LookControl`'s state.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct LookControl {
    pub y_max_rot_speed: f32,
    pub x_max_rot_angle: f32,
    pub look_at_cooldown: i32,
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
}

impl LookControl {
    /// `setLookAt(x, y, z, yMaxRotSpeed, xMaxRotAngle)`.
    pub fn set_look_at(&mut self, x: f64, y: f64, z: f64, y_max: f32, x_max: f32) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.y_max_rot_speed = y_max;
        self.x_max_rot_angle = x_max;
        self.look_at_cooldown = 2;
    }

    pub fn is_looking_at_target(&self) -> bool {
        self.look_at_cooldown > 0
    }
}

/// `BodyRotationControl`'s state.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct BodyRotationControl {
    pub head_stable_time: i32,
    pub last_stable_y_head_rot: f32,
}
