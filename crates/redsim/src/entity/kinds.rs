//! Per-kind overrides the entity core calls (clk-misc): `maxUpStep`, `getGravity`,
//! `checkFallDamage`, read in the 26.3 source (`Entity` defaults, `AbstractBoat`,
//! `AbstractMinecart`, `LivingEntity` for armor stands).

use super::EntityKind;
use crate::block::Block;
use crate::world::World;

/// `Entity.maxUpStep()` (armor stands: the `step_height` attribute, 0.6).
pub fn max_up_step(w: &World, i: usize) -> f32 {
    match &w.entities.list[i].kind {
        EntityKind::ArmorStand(_) => 0.6,
        EntityKind::Mob(m) => super::mob::max_up_step(m),
        _ => 0.0,
    }
}

/// `Entity.getGravity()` (`getDefaultGravity` unless no gravity): 0.04, living entities 0.08,
/// minecarts in water 0.005.
pub fn gravity(w: &World, i: usize) -> f64 {
    let e = &w.entities.list[i];
    match e.kind {
        EntityKind::ArmorStand(_) => 0.08,
        EntityKind::Mob(_) => super::mob::gravity(),
        EntityKind::Minecart(_) if e.was_touching_water => 0.005,
        _ => 0.04,
    }
}

/// `Entity.getAirDrag()` as the restitution after a bounce reads it: 0.98F; boats 1.0F, minecarts
/// 0.95F, living entities `computeModifiedFriction(0.98F, 1)` (not an omnidirectional air mover).
pub fn air_drag(w: &World, i: usize) -> f64 {
    match w.entities.list[i].kind {
        EntityKind::Boat(_) => 1.0,
        EntityKind::Minecart(_) => 0.95f32 as f64,
        EntityKind::ArmorStand(_) => (1.0f32 - (1.0f32 - 0.98f32) * 1.0f32).clamp(0.0, 1.0) as f64,
        EntityKind::Mob(_) => super::mob::air_drag(),
        _ => 0.98f32 as f64,
    }
}

/// `checkFallDamage(ya, onGround, onState, pos)` at the end of `Entity.move`.
pub fn check_fall_damage(w: &mut World, i: usize, ya: f64, on_ground: bool) {
    match w.entities.list[i].kind {
        EntityKind::Boat(_) => super::boat::check_fall_damage(w, i, ya, on_ground),
        // ironfarm: `LivingEntity.checkFallDamage` and `causeFallDamage`
        EntityKind::Mob(_) => super::mob::living::check_fall_damage(w, i, ya, on_ground),
        _ => {
            // `Entity.checkFallDamage`: of the modeled blocks only farmland reacts to a fall
            // (`fallOn`: its trampling roll draws from the level random) and no modeled entity
            // takes fall damage that changes its motion
            let landed = {
                let e = &mut w.entities.list[i];
                if !e.was_touching_water && ya < 0.0 {
                    e.fall_distance -= ya as f32 as f64;
                }
                on_ground && e.fall_distance > 0.0
            };
            if landed {
                // onPos = getOnPosLegacy() (getOnPos(0.2F)), its state passed to fallOn
                let on = super::collide::get_on_pos(w, &w.entities.list[i], 0.2);
                let on_state = w.get(on);
                // farmland's trampling roll (the level random) for any entity; (mob-AI
                // 2026-11-11) a terrain sample's farmland too (Block::Terrain: an item landing on
                // a village's field, dust-pulse-dug10 village-edge b11 t54326)
                if super::mob::living::fall_on_factors(&on_state).0 {
                    crate::stems::fall_on(w);
                }
                // (mob-AI 2026-11-11) Block.fallOn's causeFallDamage: Entity's passes the fall on
                // to the passengers (a minecart's riders; no modeled vehicle here is
                // #fall_damage_immune)
                if !w.entities.list[i].passengers.is_empty() {
                    let distance = w.entities.list[i].fall_distance;
                    let (_, mult, modifier) = super::mob::living::fall_on_factors(&on_state);
                    super::mob::living::propagate_fall(w, i, distance * mult, modifier);
                }
                // the HIT_GROUND game event at the entity, its affected state the main
                // supporting block's (else onState); an item of #dampens_vibrations is not heard
                let e = &w.entities.list[i];
                let at = [e.pos.x, e.pos.y, e.pos.z];
                let affected = e.main_support.map(|p| w.get(p)).unwrap_or(on_state);
                let dampens = matches!(&e.kind, EntityKind::Item(s) if crate::cactus::item_dampens(&s.item));
                w.game_event_at("hit_ground", at, Some(affected), dampens);
            }
            if on_ground {
                w.entities.list[i].fall_distance = 0.0;
            }
        }
    }
}
