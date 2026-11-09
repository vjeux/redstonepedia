//! The squid and the glow squid (`Squid`, `GlowSquid`: water creatures with no navigation): their
//! goals (`SquidRandomMovementGoal`: a new swim direction 1 time in 25 or when it has none or left
//! the water, none after 100 idle ticks; `SquidFleeGoal` refused once it would run: a hurt squid),
//! `Squid.aiStep`'s tail (the tentacle cycle and its speed's draws, the swim impulse on the cycle's
//! last quarter, the body's turns; out of the water a fall at 0.98), `travel` (the motion alone),
//! `AgeableWaterCreature`'s air (300 in the water, one less a tick out of it) and its push by
//! fluids (none), and the glow squid's particle draws.

use super::{ai, ai_mut};
use crate::entity::mob::{mob, mob_mut};
use crate::entity::{EntityKind, Vec3};
use crate::world::World;

/// A squid's own state.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct SquidState {
    pub tentacle_movement: f32,
    pub tentacle_speed: f32,
    pub rotate_speed: f32,
    pub movement_vector: Vec3,
    pub x_body_rot: f32,
    pub z_body_rot: f32,
    /// The glow squid's `DATA_DARK_TICKS_REMAINING`.
    pub dark_ticks: i32,
    pub glow: bool,
}

/// The squid's goals.
#[derive(Clone, PartialEq, Debug)]
pub enum SquidGoal {
    RandomMovement,
    Flee,
}

impl SquidGoal {
    pub fn name(&self) -> &'static str {
        match self {
            SquidGoal::RandomMovement => "SquidRandomMovementGoal",
            SquidGoal::Flee => "SquidFleeGoal",
        }
    }

    pub fn every_tick(&self) -> bool {
        matches!(self, SquidGoal::Flee)
    }
}

/// Is this a squid row (`Squid`, `GlowSquid`, adult or baby)?
pub fn is_squid_id(id: &str) -> bool {
    matches!(id, "squid" | "glow_squid" | "baby_squid" | "baby_glow_squid")
}

/// `AgeableMob.aiStep`'s age of a squid (after `super.aiStep()`, before the squid's own tail): a
/// baby's counts up to 0 and grows it into its adult (the adult's box at the same feet;
/// `fudgePositionAfterSizeChange` only where nothing collides).
pub fn age_step(w: &mut World, i: usize) {
    let id = crate::entity::mob::mob(w, i).ty.def().id;
    if !is_squid_id(id) || crate::entity::mob::mob(w, i).health <= 0.0 || w.entities.list[i].removed {
        return;
    }
    let a = super::ai(w, i).age;
    if a < 0 {
        super::ai_mut(w, i).age = a + 1;
        if a + 1 >= 0
            && let Some(ty) = crate::entity::mob::types::MobType::from_id(id.trim_start_matches("baby_"))
        {
            crate::entity::mob::mob_mut(w, i).ty = ty;
            let pos = w.entities.list[i].pos;
            let (wd, h) = crate::entity::mob::mob(w, i).dims();
            let bb = crate::entity::make_bounding_box(pos, wd, h);
            w.entities.list[i].bb = bb;
            if !crate::entity::no_collision(w, i, bb.deflate(1.0e-7, 1.0e-7, 1.0e-7)) {
                let why = "a baby squid growing up against a block (fudgePositionAfterSizeChange) is not modeled".to_string();
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
            }
        }
    } else if a > 0 {
        super::ai_mut(w, i).age = a - 1;
    }
}

/// Is entity `i` a water creature (`AgeableWaterCreature`: not pushed by fluids, its own air)?
pub fn is_water_creature(w: &World, i: usize) -> bool {
    matches!(&w.entities.list[i].kind, EntityKind::Mob(m) if is_squid_id(m.ty.def().id))
}

/// `Squid.registerGoals`; the constructor's `tentacleSpeed` is drawn by [`ctor_draw`].
pub fn register(a: &mut super::MobAi, id: &str) {
    use super::goal::Goal;
    a.goals.add(0, Goal::Squid(SquidGoal::RandomMovement));
    a.goals.add(1, Goal::Squid(SquidGoal::Flee));
    a.squid = Some(SquidState { glow: id.ends_with("glow_squid"), ..SquidState::default() });
    // a natural baby spawns at age -24000 (AgeableMob.finalizeSpawn)
    a.age = if id.starts_with("baby_") { -24000 } else { 0 };
}

/// The squid's constructor draw: `tentacleSpeed = 1 / (nextFloat() + 1) * 0.2` (after the
/// entity's own).
pub fn ctor_draw(r: &mut crate::rng::LegacyRandom) -> f32 {
    1.0f32 / (r.next_float() + 1.0f32) * 0.2f32
}

fn sq(w: &World, i: usize) -> &SquidState {
    ai(w, i).squid.as_ref().expect("a squid")
}

fn sq_mut(w: &mut World, i: usize) -> &mut SquidState {
    ai_mut(w, i).squid.as_mut().expect("a squid")
}

fn own(w: &mut World, i: usize) -> &mut crate::rng::LegacyRandom {
    w.entities.list[i].random.as_mut().expect("random")
}

pub fn can_use(w: &mut World, i: usize, g: &mut SquidGoal) -> bool {
    match g {
        SquidGoal::RandomMovement => true,
        SquidGoal::Flee => {
            let hurt_by = ai(w, i).last_hurt_by_mob;
            if w.entities.list[i].was_touching_water && hurt_by.is_some() {
                let why = "a hurt squid's SquidFleeGoal (and its ink) is not modeled yet".to_string();
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
            }
            false
        }
    }
}

pub fn tick(w: &mut World, i: usize, g: &mut SquidGoal) {
    if !matches!(g, SquidGoal::RandomMovement) {
        return;
    }
    if mob(w, i).no_action_time > 100 {
        sq_mut(w, i).movement_vector = Vec3::ZERO;
        return;
    }
    let roll = own(w, i).next_int(super::goal::reduced_tick_delay(50)) == 0;
    let has = sq(w, i).movement_vector.length_sqr() > 1.0e-5f32 as f64;
    if roll || !w.entities.list[i].was_touching_water || !has {
        let angle = own(w, i).next_float() * (std::f64::consts::PI * 2.0) as f32;
        let y = -0.1f32 + own(w, i).next_float() * 0.2f32;
        let v = Vec3::new((crate::entity::mth::cos(angle as f64) * 0.2f32) as f64, y as f64, (crate::entity::mth::sin(angle as f64) * 0.2f32) as f64);
        sq_mut(w, i).movement_vector = v;
    }
}

/// `Squid.aiStep` after `super.aiStep()` (and `GlowSquid.aiStep`'s tail), before the head turn.
pub fn after_ai_step(w: &mut World, i: usize) {
    // (a dying squid runs it too: LivingEntity.tick calls aiStep whatever its health)
    if ai(w, i).squid.is_none() {
        return;
    }
    let tau = (std::f64::consts::PI * 2.0) as f32;
    let pi = std::f32::consts::PI;
    {
        let s = sq_mut(w, i);
        s.tentacle_movement += s.tentacle_speed;
    }
    if sq(w, i).tentacle_movement as f64 > std::f64::consts::PI * 2.0 {
        sq_mut(w, i).tentacle_movement -= tau;
        if own(w, i).next_int(10) == 0 {
            let s = ctor_draw(own(w, i));
            sq_mut(w, i).tentacle_speed = s;
        }
    }
    if w.entities.list[i].was_touching_water {
        let tm = sq(w, i).tentacle_movement;
        if tm < pi {
            let scale = tm / pi;
            if scale as f64 > 0.75 {
                let v = sq(w, i).movement_vector;
                w.entities.list[i].delta_movement = v;
                sq_mut(w, i).rotate_speed = 1.0;
            } else {
                sq_mut(w, i).rotate_speed *= 0.8;
            }
        } else {
            let d = w.entities.list[i].delta_movement;
            w.entities.list[i].delta_movement = d.scale(0.9);
            sq_mut(w, i).rotate_speed *= 0.99;
        }
        let m = w.entities.list[i].delta_movement;
        let horizontal = m.horizontal_distance();
        let deg = 180.0f32 / pi;
        let body = ai(w, i).y_body_rot;
        let body = body + (-(crate::entity::mth::atan2(m.x, m.z) as f32) * deg - body) * 0.1f32;
        ai_mut(w, i).y_body_rot = body;
        w.entities.list[i].y_rot = body;
        let s = sq_mut(w, i);
        s.z_body_rot += pi * s.rotate_speed * 1.5f32;
        s.x_body_rot += (-(crate::entity::mth::atan2(horizontal, m.y) as f32) * deg - s.x_body_rot) * 0.1f32;
    } else {
        let d = w.entities.list[i].delta_movement;
        let yd = d.y - 0.08;
        w.entities.list[i].delta_movement = Vec3::new(0.0, yd * 0.98f32 as f64, 0.0);
        let s = sq_mut(w, i);
        s.x_body_rot += (-90.0f32 - s.x_body_rot) * 0.02f32;
    }
    // GlowSquid.aiStep: its dark ticks run down; a GLOW particle at getRandomX(0.6),
    // getRandomY(), getRandomZ(0.6): three doubles of its own random
    if sq(w, i).glow {
        let s = sq_mut(w, i);
        if s.dark_ticks > 0 {
            s.dark_ticks -= 1;
        }
        let r = own(w, i);
        r.next_double();
        r.next_double();
        r.next_double();
    }
}

/// `AgeableWaterCreature.handleAirSupply(pre)` after `baseTick` (`LivingEntity.baseTick`'s own air:
/// it breathes under water, `+4` out of it): 300 in the water, else one less than before the tick
/// (at -20 drowning damage 2, its air 0).
pub fn handle_air_supply(w: &mut World, i: usize, pre: i32) -> bool {
    if w.entities.list[i].was_touching_water || mob(w, i).health <= 0.0 {
        mob_mut(w, i).air = 300;
        return false;
    }
    mob_mut(w, i).air = pre - 1;
    if mob(w, i).air == -20 {
        mob_mut(w, i).air = 0;
        return true;
    }
    false
}
