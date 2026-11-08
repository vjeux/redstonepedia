//! `FloatGoal`: swim up in water deeper than the jump threshold, or in lava.

use crate::entity::mob::mob;
use crate::world::World;

/// `canUse()` = `isInFluidDeeperThan(getFluidJumpThreshold(), ENTITY_FLOATABLE) || isInLava()`.
pub fn can_use(w: &mut World, i: usize) -> bool {
    let e = &w.entities.list[i];
    let m = mob(w, i);
    let threshold = if (m.eye() as f64) < 0.4 { 0.0 } else { 0.4 };
    e.fluid.water_height() > threshold || (!e.first_tick && m.lava_height > 0.0)
}

/// `tick()`: a jump four times in five.
pub fn tick(w: &mut World, i: usize) {
    let r = w.entities.list[i].random.as_mut().map(|r| r.next_float()).unwrap_or(1.0);
    if r < 0.8 {
        super::super::ai_mut(w, i).jump = true;
    }
}
