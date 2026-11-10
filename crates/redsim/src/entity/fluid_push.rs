//! Entity fluid interaction (redsim M2, clockinfra): `Entity.updateFluidInteraction` and
//! `EntityFluidInteraction` (26.3), the part of `baseTick` that measures how deep an entity is in
//! each fluid, whether its eyes are in it, and pushes it along the current.
//!
//! * Trackers per fluid TYPE (source water and flowing water are separate trackers; the WATER
//!   tag's height is the max of both), kept across ticks while they are non-zero.
//! * One current accumulator per tag with a current (water here): cells visited x, then y, then
//!   z; each cell's flow is scaled by the accumulator's height (the running max of the CURRENT
//!   tracker's height) while that is under 0.4.
//! * `applyTo` (non-players): the normalized sum times 0.014, raised to 0.0045 long when the
//!   entity is almost still horizontally.

use crate::entity::{Entity, Vec3, mth};
use crate::fluid::{self, Fluid};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `EntityFluidInteraction.Tracker`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tracker {
    pub fluid: Fluid,
    pub height: f64,
    pub eyes_inside: bool,
}

/// `EntityFluidInteraction.CurrentAccumulator`: one per fluid tag with a current (water and lava:
/// `Entity.FLUIDS_WITH_CURRENT`), reset every update.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Current {
    height: f64,
    flow: Vec3,
    count: i32,
}

/// `EntityFluidInteraction` (the trackers live across ticks; the current accumulators are reset
/// every update).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FluidInteraction {
    pub trackers: Vec<Tracker>,
    water: Current,
    lava: Current,
}

impl FluidInteraction {
    /// `getFluidHeight(LAVA)` (mobinfra: lava is a FluidState since flowing lava).
    pub fn lava_height(&self) -> f64 {
        let mut h = 0.0;
        for t in &self.trackers {
            if t.height > h && t.fluid.is_lava() {
                h = t.height;
            }
        }
        h
    }

    /// `getFluidHeight(WATER)`.
    pub fn water_height(&self) -> f64 {
        let mut h = 0.0;
        for t in &self.trackers {
            if t.height > h && t.fluid.is_water() {
                h = t.height;
            }
        }
        h
    }

    /// `isInFluid(WATER)`.
    pub fn in_water(&self) -> bool {
        self.water_height() > 0.0
    }

    /// `isEyeInFluid(WATER)`.
    pub fn eye_in_water(&self) -> bool {
        self.trackers.iter().any(|t| t.eyes_inside && t.fluid.is_water())
    }

    fn tracker(&mut self, f: Fluid) -> usize {
        if let Some(i) = self.trackers.iter().position(|t| t.fluid == f) {
            return i;
        }
        self.trackers.push(Tracker { fluid: f, height: 0.0, eyes_inside: false });
        self.trackers.len() - 1
    }
}

/// `FluidState.getHeightForCamera`: 1 for a source under a block whose bottom face is sturdy.
fn height_for_camera(w: &World, p: BlockPos, f: fluid::FluidState) -> f32 {
    if f.is_source() {
        let above = p.above();
        if crate::behavior::sturdy(w, &w.get(above), above, Dir::Down, crate::behavior::Support::Full) {
            return 1.0;
        }
    }
    f.height(w, p)
}

/// `EntityFluidInteraction.update(entity, ignoreCurrent)`: true when the box touches a fluid.
pub fn update(w: &World, e: &mut Entity, ignore_current: bool) -> bool {
    let fi = &mut e.fluid;
    fi.trackers.retain_mut(|t| {
        if t.height == 0.0 && !t.eyes_inside {
            return false;
        }
        t.height = 0.0;
        t.eyes_inside = false;
        true
    });
    fi.water = Current::default();
    fi.lava = Current::default();
    // getFluidInteractionBox: the box deflated by 0.001 (no vehicles here)
    let bx = e.bb.deflate(0.001, 0.001, 0.001);
    let (x0, y0, z0) = (mth::floor(bx.min_x), mth::floor(bx.min_y), mth::floor(bx.min_z));
    let ceil = |v: f64| v.ceil() as i32;
    let (x1, y1, z1) = (ceil(bx.max_x) - 1, ceil(bx.max_y) - 1, ceil(bx.max_z) - 1);
    let entity_y = e.bb.min_y;
    let eye_x = mth::floor(e.pos.x);
    let eye_y = e.pos.y + e.kind.eye_height() as f64;
    let eye_z = mth::floor(e.pos.z);
    let mut last: Option<Fluid> = None;
    let mut tracker = 0usize;
    let mut has_current = false;
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                let p = BlockPos::new(x, y, z);
                let f = fluid::fluid_state(w, p);
                if f.is_empty() {
                    continue;
                }
                let bottom = y as f64;
                let top = bottom + f.height(w, p) as f64;
                if top < bx.min_y {
                    continue;
                }
                if last != Some(f.fluid) {
                    last = Some(f.fluid);
                    tracker = fi.tracker(f.fluid);
                    if !ignore_current {
                        // both fluids have a current (Entity.FLUIDS_WITH_CURRENT: WATER, LAVA)
                        has_current = f.fluid.is_water() || f.fluid.is_lava();
                    }
                }
                if x == eye_x && z == eye_z && eye_y >= bottom {
                    let cam = bottom + height_for_camera(w, p, f) as f64;
                    if eye_y <= cam {
                        fi.trackers[tracker].eyes_inside = true;
                    }
                }
                let th = crate::entity::vec3::jmax(top - entity_y, fi.trackers[tracker].height);
                fi.trackers[tracker].height = th;
                if has_current {
                    let mut flow = fluid::flow(w, p, f);
                    let acc = if f.fluid.is_lava() { &mut fi.lava } else { &mut fi.water };
                    acc.height = crate::entity::vec3::jmax(th, acc.height);
                    if acc.height < 0.4 {
                        flow = flow.scale(acc.height);
                    }
                    acc.flow = acc.flow.add_v(flow);
                    acc.count += 1;
                }
            }
        }
    }
    last.is_some()
}

/// `CurrentAccumulator.applyTo(entity, scale)` for a non-player entity (the water or the lava one).
fn apply_current(e: &mut Entity, lava: bool, scale: f64) {
    let acc = if lava { e.fluid.lava } else { e.fluid.water };
    if acc.count == 0 || acc.flow.length_sqr() < 1.0e-5f32 as f64 {
        return;
    }
    let old = e.delta_movement;
    let mut impulse = acc.flow.normalize().scale(scale);
    if old.x.abs() < 0.003 && old.z.abs() < 0.003 && impulse.length() < 0.0045000000000000005 {
        impulse = impulse.normalize().scale(0.0045000000000000005);
    }
    e.delta_movement = e.delta_movement.add_v(impulse);
}

/// `Entity.updateFluidInteraction` of entity `i` (called by `baseTick`): the trackers, the fall
/// distance reset in water, `wasTouchingWater`, the water current (0.014; every modeled entity
/// is pushed by fluids). Returns `inAnyFluid`.
pub fn update_fluid_interaction(w: &mut World, i: usize) -> bool {
    // the entity leaves the list while it reads the world and comes back to its place below
    // (O(1): a clone of a villager's brain every tick was a fifth of the mob AI's time); nothing
    // here reads the entity list
    let mut e = w.entities.list.swap_remove(i);
    // AgeableWaterCreature.isPushedByFluid: false (a squid with its AI: no current, none applied;
    // WaterAnimal's too: a fish); Drowned.isPushedByFluid: not while it swims (last tick's
    // updateSwimming); AbstractArrow: not while in the ground
    let pushed = !matches!(&e.kind, crate::entity::EntityKind::Mob(m) if m.ai.is_some() && (crate::entity::mob::ai::squid::is_squid_id(m.ty.def().id) || m.ai.as_deref().is_some_and(|a| a.swimming || a.fish.is_some())))
        && !matches!(&e.kind, crate::entity::EntityKind::Arrow(a) if crate::entity::arrow::in_ground(a));
    let in_any = update(w, &mut e, !pushed);
    let in_water = e.fluid.in_water();
    if in_water {
        e.fall_distance = 0.0;
        // `doWaterSplashEffect` on entering water (not on the first tick): its sound's pitch and
        // its bubble and splash particles draw from the entity's own random (2 floats, then 3 and
        // 2 doubles per particle, 1 + width x 20 of each kind); the particles and the sound go to
        // the clients, the SPLASH game event to sculk sensors
        if !e.was_touching_water && !e.first_tick {
            let width = e.kind.dims().0;
            if let Some(r) = e.random.as_mut() {
                r.next_float();
                r.next_float();
                let n = 1.0f32 + width * 20.0f32;
                let mut k = 0;
                while (k as f32) < n {
                    r.next_double();
                    r.next_double();
                    r.next_double();
                    k += 1;
                }
                let mut k = 0;
                while (k as f32) < n {
                    r.next_double();
                    r.next_double();
                    k += 1;
                }
            }
            if !w.sculk.is_empty() {
                let why = "an entity's splash (GameEvent.SPLASH) near sculk sensors is not modeled".to_string();
                if !w.errors.contains(&why) {
                    w.errors.push(why);
                }
            }
        }
    }
    e.was_touching_water = in_water;
    e.fluid_height = [e.fluid.water_height(), e.fluid.lava_height()];
    if in_water && pushed {
        apply_current(&mut e, false, 0.014);
    }
    // the lava current (FAST_LAVA in an ultra-warm dimension: 0.007; else 0.0023333333333333335)
    if e.fluid.lava_height() > 0.0 && pushed {
        apply_current(&mut e, true, if w.ultrawarm { 0.007 } else { 0.0023333333333333335 });
    }
    w.entities.list.push(e);
    let last = w.entities.list.len() - 1;
    w.entities.list.swap(i, last);
    in_any
}
