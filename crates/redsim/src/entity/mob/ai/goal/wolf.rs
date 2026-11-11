//! The wolf's own body (26.3 `Wolf`, an untamed one's), beside its goals (`animal::register`):
//!
//! * `Wolf.tick` after `super.tick()` (alive): in water or rain it gets wet (a shake under way is
//!   cancelled: entity event 56); a wet wolf shaking plays `WOLF_SHAKE` once (two floats of its
//!   own random) with the `ENTITY_ACTION` game event, its shake runs 0.05 a tick until 2, and past
//!   0.4 its splash particles (`(int)(sin((shake - 0.4) pi) x 7)` of them, two floats each: the
//!   server's `addParticle` draws them for nothing);
//! * `Wolf.aiStep` after `super.aiStep()`: a wet wolf on the ground not path finding starts to
//!   shake, then `updatePersistentAnger(level, true)` (the iron golem's: 20 to 39 s);
//! * its ambient sound: a growl when angry, else `nextInt(3)` picks the pant or the bark.

use super::super::{ai, ai_mut};
use crate::entity::mob::mob;
use crate::entity::{EntityKind, mth};
use crate::world::World;

/// A wolf's shake (`isWet`, `isShaking`, `shakeAnim`, `shakeAnimO`).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct WolfState {
    pub is_wet: bool,
    pub is_shaking: bool,
    pub shake_anim: f32,
    pub shake_anim_o: f32,
}

/// Is entity `i` a wolf with its AI?
pub fn is_wolf(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if m.ai.as_deref().is_some_and(|a| a.wolf.is_some()))
}

fn st(w: &World, i: usize) -> WolfState {
    ai(w, i).wolf.expect("a wolf")
}

fn set(w: &mut World, i: usize, s: WolfState) {
    ai_mut(w, i).wolf = Some(s);
}

/// `NeutralMob.isAngry()`: its anger's end still ahead.
pub fn is_angry(w: &World, i: usize) -> bool {
    let a = ai(w, i);
    a.anger_end > 0 && a.anger_end - w.time > 0
}

/// `Entity.isInWaterOrRain()`.
fn in_water_or_rain(w: &mut World, i: usize) -> bool {
    w.entities.list[i].was_touching_water || crate::weather::entity_in_rain(w, i)
}

/// `Wolf.tick()` after `super.tick()`.
pub fn after_tick(w: &mut World, i: usize) {
    if !is_wolf(w, i) || w.entities.list[i].removed || mob(w, i).health <= 0.0 {
        return;
    }
    let mut s = st(w, i);
    if in_water_or_rain(w, i) {
        s.is_wet = true;
        if s.is_shaking {
            // cancelShake (entity event 56)
            s.is_shaking = false;
            s.shake_anim = 0.0;
            s.shake_anim_o = 0.0;
        }
    } else if s.is_shaking {
        if s.shake_anim == 0.0 {
            // WOLF_SHAKE's pitch: (nextFloat() - nextFloat()) x 0.2 + 1; the ENTITY_ACTION event
            let r = w.entities.list[i].random.as_mut().expect("random");
            r.next_float();
            r.next_float();
            let p = w.entities.list[i].pos;
            w.game_event_at("entity_action", [p.x, p.y, p.z], None, false);
        }
        s.shake_anim_o = s.shake_anim;
        s.shake_anim += 0.05f32;
        if s.shake_anim_o >= 2.0 {
            s.is_wet = false;
            s.is_shaking = false;
            s.shake_anim_o = 0.0;
            s.shake_anim = 0.0;
        }
        if s.shake_anim > 0.4f32 {
            // the splash particles: two floats of its own random each
            let count = (mth::sin(((s.shake_anim - 0.4f32) * std::f32::consts::PI) as f64) * 7.0f32) as i32;
            if std::env::var("MCTAS_TRACE_SHAKE").is_ok() {
                eprintln!("shake t{}: #{} anim {} splashes {count}", w.time, w.entities.list[i].id, s.shake_anim);
            }
            let r = w.entities.list[i].random.as_mut().expect("random");
            for _ in 0..count {
                r.next_float();
                r.next_float();
            }
        }
    }
    set(w, i, s);
}

/// `Wolf.aiStep()` after `super.aiStep()`: the shake starts, then the anger is kept up to date.
pub fn after_ai_step(w: &mut World, i: usize) {
    if !is_wolf(w, i) || w.entities.list[i].removed {
        return;
    }
    let mut s = st(w, i);
    let path_finding = crate::entity::mob::ai::nav_of(w, i).is_in_progress();
    if s.is_wet && !s.is_shaking && !path_finding && w.entities.list[i].on_ground {
        s.is_shaking = true;
        s.shake_anim = 0.0;
        s.shake_anim_o = 0.0;
        set(w, i, s);
    }
    super::golem::update_anger(w, i);
}

/// `Wolf.getAmbientSound()`'s own draw: when not angry, `nextInt(3)` (the pant or the bark).
pub fn ambient_draw(w: &mut World, i: usize) {
    if !is_angry(w, i) {
        w.entities.list[i].random.as_mut().expect("random").next_int(3);
    }
}
