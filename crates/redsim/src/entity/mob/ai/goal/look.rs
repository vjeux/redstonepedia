//! `LookAtPlayerGoal` and `RandomLookAroundGoal`.

use super::super::target::{self, Conditions, Who};
use super::super::{ai_mut, eye_pos, head_rot_speed, max_head_x_rot};
use crate::entity::mob::mob;
use crate::entity::{Vec3, jmath};
use crate::world::World;

/// What a look goal looks at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LookType {
    Player,
    /// (mob-AI 2026-11-11) `Mob.class` (a wandering trader's): the mobs whose boxes meet its box
    /// grown by the distance (3 up and down), in the level's section order, the nearest
    /// (`getNearestEntity`).
    Mob,
}

/// `LookAtPlayerGoal`.
#[derive(Clone, PartialEq, Debug)]
pub struct LookAtPlayer {
    pub look_type: LookType,
    pub look_distance: f32,
    pub probability: f32,
    pub only_horizontal: bool,
    pub look_at: Option<u32>,
    pub look_time: i32,
    /// `InteractGoal` (a LookAtPlayerGoal holding LOOK and MOVE).
    pub interact: bool,
}

impl LookAtPlayer {
    pub fn new(look_type: LookType, look_distance: f32) -> LookAtPlayer {
        LookAtPlayer { look_type, look_distance, probability: 0.02, only_horizontal: false, look_at: None, look_time: 0, interact: false }
    }
}

fn draw_float(w: &mut World, i: usize) -> f32 {
    w.entities.list[i].random.as_mut().expect("a mob's own random").next_float()
}

fn draw_int(w: &mut World, i: usize, n: i32) -> i32 {
    w.entities.list[i].random.as_mut().expect("a mob's own random").next_int(n)
}

/// `canUse()`.
pub fn look_can_use(w: &mut World, i: usize, g: &mut LookAtPlayer) -> bool {
    if draw_float(w, i) >= g.probability {
        return false;
    }
    if let Some(t) = target::get_target(w, i) {
        g.look_at = Some(t);
    }
    let at = eye_pos(w, i);
    // forNonCombat().range(lookDistance), the player not riding this mob
    let cond = Conditions::non_combat().range(g.look_distance as f64);
    let list = match g.look_type {
        LookType::Player => target::players(w),
        LookType::Mob => {
            let d = g.look_distance as f64;
            let area = w.entities.list[i].bb.inflate(d, 3.0, d);
            target::mobs_in(w, area, |_| true)
        }
    };
    g.look_at = target::nearest(w, i, &list, &cond, at).map(|t| target::id_of(w, t));
    g.look_at.is_some()
}

/// `canContinueToUse()`.
pub fn look_can_continue(w: &mut World, i: usize, g: &mut LookAtPlayer) -> bool {
    let Some(t) = g.look_at.and_then(|id| target::who(w, id)) else { return false };
    if !target::is_alive(w, t) {
        return false;
    }
    let d = w.entities.list[i].pos.distance_to_sqr(target::pos_of(w, t));
    if d > (g.look_distance * g.look_distance) as f64 { false } else { g.look_time > 0 }
}

/// `start()`: look 40..79 ticks (reduced: the goal is not ticked every tick).
pub fn look_start(w: &mut World, i: usize, g: &mut LookAtPlayer) {
    let n = draw_int(w, i, 40);
    g.look_time = super::reduced_tick_delay(40 + n);
}

/// `tick()`.
pub fn look_tick(w: &mut World, i: usize, g: &mut LookAtPlayer) {
    let Some(t) = g.look_at.and_then(|id| target::who(w, id)) else { return };
    if target::is_alive(w, t) {
        let te = target::eye_of(w, t);
        let y = if g.only_horizontal { eye_pos(w, i).y } else { te.y };
        let (ys, xs) = (head_rot_speed(w, i) as f32, max_head_x_rot(w, i) as f32);
        ai_mut(w, i).look.set_look_at(te.x, y, te.z, ys, xs);
        g.look_time -= 1;
    }
}

/// `RandomLookAroundGoal`.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RandomLookAround {
    pub rel_x: f64,
    pub rel_z: f64,
    pub look_time: i32,
}

/// `canUse()`: one tick in fifty.
pub fn random_can_use(w: &mut World, i: usize, _g: &mut RandomLookAround) -> bool {
    draw_float(w, i) < 0.02
}

/// `start()`: a random direction (`Math.cos` / `Math.sin`: HotSpot's intrinsics), 20..39 ticks.
pub fn random_start(w: &mut World, i: usize, g: &mut RandomLookAround) {
    let d = w.entities.list[i].random.as_mut().expect("a mob's own random").next_double();
    let rnd = std::f64::consts::PI * 2.0 * d;
    g.rel_x = jmath::cos(rnd);
    g.rel_z = jmath::sin(rnd);
    g.look_time = 20 + draw_int(w, i, 20);
}

/// `tick()`.
pub fn random_tick(w: &mut World, i: usize, g: &mut RandomLookAround) {
    g.look_time -= 1;
    let e = &w.entities.list[i];
    let at = Vec3::new(e.pos.x + g.rel_x, e.pos.y + mob(w, i).eye() as f64, e.pos.z + g.rel_z);
    let (ys, xs) = (head_rot_speed(w, i) as f32, max_head_x_rot(w, i) as f32);
    ai_mut(w, i).look.set_look_at(at.x, at.y, at.z, ys, xs);
}

#[allow(dead_code)]
fn _who(_w: Who) {}
