//! The player away for days (brief farm-iron.md, Stages: Middle, "restarts by itself after the
//! player is away for days"), both ways it happens in the game:
//!
//! - [`Away::Unloaded`]: the player far away, the farm's chunks saved and loaded again. Every
//!   entity comes back as a new object: a fresh random (`Entity.random`, never saved; redsim draws
//!   its seed from the mob layer's random), and a villager's brain rebuilt by
//!   `Villager.readAdditionalSaveData` -> `refreshBrain` -> `registerBrainGoals` (each sensor's
//!   `randomlyDelayStart`, the schedule's activity at the day time: [`villager::Brain::new`]),
//!   keeping only the memories registered with a codec (`HOME`, `LAST_SLEPT`, `LAST_WOKEN`,
//!   `GOLEM_DETECTED_RECENTLY` with its time left; not `NEAREST_HOSTILE`, the sensed lists or
//!   `HURT_BY`), and no behavior running. A sleeping villager stays asleep (`SleepingX/Y/Z`).
//!   Block ticks are saved with their delays (`SavedTick`) and come back as pending.
//! - [`Away::NotTicking`]: the player beyond the simulation distance but within the view distance:
//!   the chunks stay loaded and nothing in them ticks (no entity, brain, block entity or block
//!   tick: memories keep their time left), while the game time runs on.
//!
//! Either way the game time (and day time) advances by the absence.

use super::{mob_mut, villager};
use crate::entity::EntityKind;
use crate::rng::LegacyRandom;
use crate::world::World;

/// How the player was away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Away {
    Unloaded,
    NotTicking,
}

impl Away {
    pub fn name(self) -> &'static str {
        match self {
            Away::Unloaded => "chunks unloaded",
            Away::NotTicking => "loaded, not ticking",
        }
    }
}

/// The farm's world after `ticks` game ticks away. `Err` when a block tick is pending: the
/// caller starts the absence at a quiet tick (a pending tick's delay would move with
/// [`Away::Unloaded`] and come due at once with [`Away::NotTicking`]; neither is needed then).
pub fn away(w: &mut World, ticks: i64, how: Away) -> Result<(), String> {
    let pending = w.ticks.count() + w.fluid_ticks.count();
    if pending > 0 {
        return Err(format!("{pending} block ticks pending at gt {}", w.time));
    }
    w.time += ticks;
    if how == Away::NotTicking {
        return Ok(());
    }
    let (t, day) = (w.time, w.day_time());
    for i in 0..w.entities.list.len() {
        if !matches!(w.entities.list[i].kind, EntityKind::Mob(_)) {
            continue;
        }
        let mut own = LegacyRandom::new(w.mob.rng_mut().next_long());
        let m = mob_mut(w, i);
        if let Some(old) = m.brain.take() {
            let mut b = villager::Brain::new(&mut own, t, day);
            b.home = old.home;
            b.last_slept = old.last_slept;
            b.last_woken = old.last_woken;
            b.golem_detected = old.golem_detected;
            m.brain = Some(Box::new(b));
        }
        w.entities.list[i].random = Some(own);
    }
    Ok(())
}
