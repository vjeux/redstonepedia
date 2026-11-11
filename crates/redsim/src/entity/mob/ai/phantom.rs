//! The phantom (`Phantom`, mob-AI 2026-11-10): a flying monster the phantom spawner makes over a
//! player who has not slept for three days. Its goals: `PhantomAttackStrategyGoal` and
//! `PhantomSweepAttackGoal` (with a target), `PhantomCircleAroundAnchorGoal` (without one: circles
//! its anchor, a point 5 above where it spawned), `PhantomAttackPlayerTargetGoal` (a player within
//! 64 it may attack: never the creative AFK player; one it would target is a named model error).
//! `PhantomMoveControl` steers its motion straight at its move target (no navigation),
//! `PhantomLookControl` does nothing (its pitch stays the move control's), its body control sets
//! the head to the old body yaw and the body to its yaw, and `travel` is `travelFlying(0.2F)` (no
//! gravity, x 0.91 in the air).

use super::{ai, ai_mut};
use crate::entity::{Vec3, mth};
use crate::pos::BlockPos;
use crate::world::World;

/// `Phantom.AttackPhase`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Circle,
    Swoop,
}

/// A phantom's own state.
#[derive(Clone, PartialEq, Debug)]
pub struct PhantomState {
    /// `moveTargetPoint`.
    pub move_target: Vec3,
    /// `anchorPoint`.
    pub anchor: Option<BlockPos>,
    pub phase: Phase,
    /// `PhantomCircleAroundAnchorGoal`'s angle, distance, height and direction.
    pub angle: f32,
    pub distance: f32,
    pub height: f32,
    pub clockwise: f32,
    /// `PhantomMoveControl.speed`.
    pub speed: f32,
    /// `PhantomAttackPlayerTargetGoal.nextScanTick`.
    pub next_scan: i32,
}

impl Default for PhantomState {
    fn default() -> PhantomState {
        PhantomState { move_target: Vec3::ZERO, anchor: None, phase: Phase::Circle, angle: 0.0, distance: 0.0, height: 0.0, clockwise: 0.0, speed: 0.1, next_scan: super::goal::reduced_tick_delay(20) }
    }
}

/// The phantom's goals.
#[derive(Clone, PartialEq, Debug)]
pub enum PhantomGoal {
    AttackStrategy,
    SweepAttack,
    CircleAroundAnchor,
    AttackPlayerTarget,
}

impl PhantomGoal {
    pub fn name(&self) -> &'static str {
        match self {
            PhantomGoal::AttackStrategy => "PhantomAttackStrategyGoal",
            PhantomGoal::SweepAttack => "PhantomSweepAttackGoal",
            PhantomGoal::CircleAroundAnchor => "PhantomCircleAroundAnchorGoal",
            PhantomGoal::AttackPlayerTarget => "PhantomAttackPlayerTargetGoal",
        }
    }

    /// `PhantomMoveTargetGoal`'s MOVE; the others set none.
    pub fn flags(&self) -> u8 {
        match self {
            PhantomGoal::SweepAttack | PhantomGoal::CircleAroundAnchor => super::goal::MOVE,
            _ => 0,
        }
    }
}

/// Is this the phantom's row?
pub fn is_phantom_id(id: &str) -> bool {
    id == "phantom"
}

/// Is entity `i` a phantom with its AI?
pub fn is_phantom(w: &World, i: usize) -> bool {
    super::has_ai(w, i) && ai(w, i).phantom.is_some()
}

/// `Phantom.registerGoals`.
pub fn register(a: &mut super::MobAi) {
    use super::goal::Goal;
    a.goals.add(1, Goal::Phantom(PhantomGoal::AttackStrategy));
    a.goals.add(2, Goal::Phantom(PhantomGoal::SweepAttack));
    a.goals.add(3, Goal::Phantom(PhantomGoal::CircleAroundAnchor));
    a.targets.add(1, Goal::Phantom(PhantomGoal::AttackPlayerTarget));
    a.phantom = Some(PhantomState::default());
}

/// `Phantom.finalizeSpawn`'s anchor: its block position, 5 up.
pub fn set_anchor(w: &mut World, i: usize) {
    let p = w.entities.list[i].block_position();
    st_mut(w, i).anchor = Some(BlockPos::new(p.x, p.y + 5, p.z));
}

fn st(w: &World, i: usize) -> &PhantomState {
    ai(w, i).phantom.as_ref().expect("a phantom")
}

fn st_mut(w: &mut World, i: usize) -> &mut PhantomState {
    ai_mut(w, i).phantom.as_mut().expect("a phantom")
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

fn refuse(w: &mut World, why: &str) {
    let why = why.to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

pub fn can_use(w: &mut World, i: usize, g: &mut PhantomGoal) -> bool {
    let target = super::target::get_target(w, i);
    match g {
        PhantomGoal::AttackStrategy => {
            if target.is_some() {
                refuse(w, "a phantom with a target (its attack strategy and swoops) is not modeled");
            }
            false
        }
        PhantomGoal::SweepAttack => target.is_some() && st(w, i).phase == Phase::Swoop,
        PhantomGoal::CircleAroundAnchor => target.is_none() || st(w, i).phase == Phase::Circle,
        PhantomGoal::AttackPlayerTarget => {
            if st(w, i).next_scan > 0 {
                st_mut(w, i).next_scan -= 1;
                return false;
            }
            st_mut(w, i).next_scan = super::goal::reduced_tick_delay(60);
            // getNearbyPlayers(forCombat().range(64), box grown by 16, 64, 16): a creative player
            // is never an enemy
            let area = w.entities.list[i].bb.inflate(16.0, 64.0, 16.0);
            let cond = super::target::Conditions::combat().range(64.0);
            let found = super::target::players(w).into_iter().any(|p| super::target::box_of(w, p).intersects(area) && cond.test(w, i, p));
            if found {
                refuse(w, "a phantom targeting a player is not modeled");
            }
            false
        }
    }
}

/// `canContinueToUse()` (the circle goal's is its `canUse`).
pub fn can_continue(w: &mut World, i: usize, g: &mut PhantomGoal) -> bool {
    match g {
        PhantomGoal::CircleAroundAnchor => can_use(w, i, g),
        _ => false,
    }
}

pub fn start(w: &mut World, i: usize, g: &mut PhantomGoal) {
    if let PhantomGoal::CircleAroundAnchor = g {
        let distance = 5.0f32 + own(w, i).next_float() * 10.0f32;
        let height = -4.0f32 + own(w, i).next_float() * 9.0f32;
        let clockwise = if own(w, i).next_bool() { 1.0f32 } else { -1.0f32 };
        let s = st_mut(w, i);
        s.distance = distance;
        s.height = height;
        s.clockwise = clockwise;
        select_next(w, i);
    }
}

pub fn stop(w: &mut World, i: usize, g: &mut PhantomGoal) {
    if let PhantomGoal::SweepAttack = g {
        super::target::set_target(w, i, None);
        st_mut(w, i).phase = Phase::Circle;
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut PhantomGoal) {
    if !matches!(g, PhantomGoal::CircleAroundAnchor) {
        return;
    }
    let delay = super::goal::reduced_tick_delay;
    if own(w, i).next_int(delay(350)) == 0 {
        let h = -4.0f32 + own(w, i).next_float() * 9.0f32;
        st_mut(w, i).height = h;
    }
    if own(w, i).next_int(delay(250)) == 0 {
        let s = st_mut(w, i);
        s.distance += 1.0;
        if s.distance > 15.0 {
            s.distance = 5.0;
            s.clockwise = -s.clockwise;
        }
    }
    if own(w, i).next_int(delay(450)) == 0 {
        let a = own(w, i).next_float() * 2.0f32 * std::f64::consts::PI as f32;
        st_mut(w, i).angle = a;
        select_next(w, i);
    }
    let pos = w.entities.list[i].pos;
    if st(w, i).move_target.distance_to_sqr(pos) < 4.0 {
        select_next(w, i);
    }
    let bp = w.entities.list[i].block_position();
    if st(w, i).move_target.y < pos.y && !w.get(bp.below()).is_air() {
        let s = st_mut(w, i);
        s.height = s.height.max(1.0);
        select_next(w, i);
    }
    if st(w, i).move_target.y > pos.y && !w.get(bp.above()).is_air() {
        let s = st_mut(w, i);
        s.height = s.height.min(-1.0);
        select_next(w, i);
    }
}

/// `PhantomCircleAroundAnchorGoal.selectNext`: 15 degrees further round the anchor (its own block
/// when it has none).
fn select_next(w: &mut World, i: usize) {
    if st(w, i).anchor.is_none() {
        let p = w.entities.list[i].block_position();
        st_mut(w, i).anchor = Some(p);
    }
    let s = st_mut(w, i);
    s.angle += s.clockwise * 15.0f32 * (std::f64::consts::PI / 180.0) as f32;
    let a = s.anchor.expect("set");
    let x = s.distance * mth::cos(s.angle as f64);
    let z = s.distance * mth::sin(s.angle as f64);
    s.move_target = Vec3::new(a.x as f64 + x as f64, a.y as f64 + (-4.0f32 + s.height) as f64, a.z as f64 + z as f64);
}

/// `Mth.approach(current, target, increment)`.
fn approach(current: f32, target: f32, increment: f32) -> f32 {
    let inc = increment.abs();
    if current < target { mth::clampf(current + inc, current, target) } else { mth::clampf(current - inc, target, current) }
}

/// `Mth.approachDegrees(current, target, increment)`.
fn approach_degrees(current: f32, target: f32, increment: f32) -> f32 {
    let difference = mth::wrap_degrees(target - current);
    approach(current, current + difference, increment)
}

/// `PhantomMoveControl.tick()`.
pub fn move_control_tick(w: &mut World, i: usize) {
    if w.entities.list[i].horizontal_collision {
        w.entities.list[i].y_rot += 180.0;
        st_mut(w, i).speed = 0.1;
    }
    let target = st(w, i).move_target;
    let pos = w.entities.list[i].pos;
    let mut tdx = target.x - pos.x;
    let tdy = target.y - pos.y;
    let mut tdz = target.z - pos.z;
    let mut sd = (tdx * tdx + tdz * tdz).sqrt();
    if sd.abs() <= 1.0e-5f32 as f64 {
        return;
    }
    let scale = 1.0 - (tdy * 0.7f32 as f64).abs() / sd;
    tdx *= scale;
    tdz *= scale;
    sd = (tdx * tdx + tdz * tdz).sqrt();
    let sd2 = (tdx * tdx + tdz * tdz + tdy * tdy).sqrt();
    let prev = w.entities.list[i].y_rot;
    let angle = mth::atan2(tdz, tdx) as f32;
    let a = mth::wrap_degrees(prev + 90.0f32);
    let b = mth::wrap_degrees(angle * (180.0f32 / std::f64::consts::PI as f32));
    let y_rot = approach_degrees(a, b, 4.0) - 90.0f32;
    w.entities.list[i].y_rot = y_rot;
    ai_mut(w, i).y_body_rot = y_rot;
    let speed = st(w, i).speed;
    let speed = if mth::wrap_degrees(y_rot - prev).abs() < 3.0 { approach(speed, 1.8, 0.005f32 * (1.8f32 / speed)) } else { approach(speed, 0.2, 0.025) };
    st_mut(w, i).speed = speed;
    // the bytecode's folded double product (atan2 x 57.2957763671875, negated, then the float)
    let x_rot_d = (-(mth::atan2(-tdy, sd) * 57.2957763671875)) as f32;
    w.entities.list[i].x_rot = x_rot_d;
    let move_angle = y_rot + 90.0f32;
    let rad = (std::f64::consts::PI / 180.0) as f32;
    let txd = (speed * mth::cos((move_angle * rad) as f64)) as f64 * (tdx / sd2).abs();
    let tzd = (speed * mth::sin((move_angle * rad) as f64)) as f64 * (tdz / sd2).abs();
    let tyd = (speed * mth::sin((x_rot_d * rad) as f64)) as f64 * (tdy / sd2).abs();
    let m = w.entities.list[i].delta_movement;
    w.entities.list[i].delta_movement = Vec3::new(m.x + (txd - m.x) * 0.2, m.y + (tyd - m.y) * 0.2, m.z + (tzd - m.z) * 0.2);
}

/// `PhantomBodyRotationControl.clientTick()`: the head takes the body's old yaw, the body the
/// entity's yaw.
pub fn body_rotation_tick(w: &mut World, i: usize) {
    let y_rot = w.entities.list[i].y_rot;
    let a = ai_mut(w, i);
    a.y_head_rot = a.y_body_rot;
    a.y_body_rot = y_rot;
}
