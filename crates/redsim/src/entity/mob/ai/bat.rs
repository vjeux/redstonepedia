//! The bat (`Bat`: no goals; `customServerAiStep` flies it): resting under a ceiling (a redstone
//! conductor above it) it may turn its head (1 in 200) and wakes when a player comes within 4 in
//! sight, or when its ceiling is gone; flying, it keeps a target cell (a new one when it has none,
//! when that cell is no longer air, 1 in 30, or within 2), pushes its motion toward it, faces its
//! motion, and hangs again 1 in 100 under a conductor. `Bat.tick` after the living tick: a resting
//! bat holds still at its ceiling (`setPosRaw`: its box stays until it moves), a flying one keeps
//! 0.6 of its vertical motion.

use crate::entity::Vec3;
use crate::entity::mob::ai::{ai_mut, target};
use crate::entity::mob::{mob, mob_mut};
use crate::pos::BlockPos;
use crate::world::World;

/// Is this mob a bat?
pub fn is_bat(w: &World, i: usize) -> bool {
    mob(w, i).ty.def().id == "bat"
}

fn conductor(w: &World, p: BlockPos) -> bool {
    crate::behavior::conductor_of(&w.get(p))
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

/// `Bat.customServerAiStep()`.
pub fn custom_server_ai_step(w: &mut World, i: usize) {
    let pos = w.entities.list[i].block_position();
    let above = pos.above();
    if mob(w, i).bat_resting {
        if conductor(w, above) {
            if own(w, i).next_int(200) == 0 {
                let h = own(w, i).next_int(360) as f32;
                ai_mut(w, i).y_head_rot = h;
            }
            // getNearestPlayer(BAT_RESTING_TARGETING: non-combat, within 4, in sight)
            let cond = target::Conditions::non_combat().range(4.0);
            let me = w.entities.list[i].pos;
            let players = target::players(w);
            if target::nearest(w, i, &players, &cond, me).is_some() {
                mob_mut(w, i).bat_resting = false;
            }
        } else {
            mob_mut(w, i).bat_resting = false;
        }
        return;
    }
    let me = w.entities.list[i].pos;
    let min_y = w.land.as_ref().map(|l| l.world_min_y).unwrap_or(-64);
    if let Some(t) = mob(w, i).bat_target
        && (!w.get(t).is_air() || t.y <= min_y)
    {
        mob_mut(w, i).bat_target = None;
    }
    let near = |t: BlockPos| Vec3::new(t.x as f64 + 0.5, t.y as f64 + 0.5, t.z as f64 + 0.5).distance_to_sqr(me) < 4.0;
    let renew = match mob(w, i).bat_target {
        None => true,
        Some(t) => own(w, i).next_int(30) == 0 || near(t),
    };
    if renew {
        let r = own(w, i);
        let x = me.x + r.next_int(7) as f64 - r.next_int(7) as f64;
        let y = me.y + r.next_int(6) as f64 - 2.0;
        let z = me.z + r.next_int(7) as f64 - r.next_int(7) as f64;
        mob_mut(w, i).bat_target = Some(BlockPos::new(x.floor() as i32, y.floor() as i32, z.floor() as i32));
    }
    let t = mob(w, i).bat_target.expect("set");
    let (dx, dy, dz) = (t.x as f64 + 0.5 - me.x, t.y as f64 + 0.1 - me.y, t.z as f64 + 0.5 - me.z);
    // Java's Math.signum: 0 for 0 (Rust's f64::signum gives 1.0 for +0.0)
    let sg = |v: f64| if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { v };
    let m = w.entities.list[i].delta_movement;
    let n = m.add((sg(dx) * 0.5 - m.x) * 0.1f32 as f64, (sg(dy) * 0.7f32 as f64 - m.y) * 0.1f32 as f64, (sg(dz) * 0.5 - m.z) * 0.1f32 as f64);
    w.entities.list[i].delta_movement = n;
    // `(float)(Mth.atan2(z, x) * 180.0F / (float)Math.PI) - 90.0F`: the bytecode's double product
    let y_rot_d = (crate::entity::mth::atan2(n.z, n.x) * 57.2957763671875) as f32 - 90.0;
    let e = &mut w.entities.list[i];
    let diff = crate::entity::mth::wrap_degrees(y_rot_d - e.y_rot);
    e.y_rot += diff;
    mob_mut(w, i).zza = 0.5;
    if own(w, i).next_int(100) == 0 && conductor(w, above) {
        mob_mut(w, i).bat_resting = true;
    }
}

/// `Bat.tick()` after `super.tick()`.
pub fn after_tick(w: &mut World, i: usize) {
    if w.entities.list[i].removed {
        return;
    }
    if mob(w, i).bat_resting {
        w.entities.list[i].delta_movement = Vec3::ZERO;
        let p = w.entities.list[i].pos;
        let h = mob(w, i).dims().1 as f64;
        w.entities.set_pos_raw(i, Vec3::new(p.x, p.y.floor() + 1.0 - h, p.z));
    } else {
        let d = w.entities.list[i].delta_movement;
        w.entities.list[i].delta_movement = Vec3::new(d.x, d.y * 0.6, d.z);
    }
}

/// `Bat.getAmbientSound()` after `Mob.baseTick`'s roll: resting, 3 in 4 nothing (`nextInt(4)`);
/// else the sound's pitch (`getVoicePitch`: two floats).
pub fn ambient_sound(w: &mut World, i: usize) {
    if mob(w, i).bat_resting && own(w, i).next_int(4) != 0 {
        return;
    }
    let r = own(w, i);
    r.next_float();
    r.next_float();
}
