//! Nether portals on the overworld side (the mob-AI lane, ironspec's carousel: golems moved away
//! through a portal). `NetherPortalBlock.entityInside` (its inside shape the default full block:
//! every portal cell the entity's deflated box meets) calls `Entity.setAsInsidePortal`: on its
//! cooldown the cooldown restarts (300), else the `PortalProcessor` is made or marked inside this
//! tick. `Entity.handlePortal` (in `baseTick`, before the sprint particle): the cooldown counts
//! down; a processor inside this tick for an entity that may use a portal (no passenger of
//! anything, alive) teleports at once (`getPortalTransitionTime` is 0 for all but players): the
//! cooldown set to 300, the entity leaves the overworld (removed, CHANGED_DIMENSION; its copy in
//! the Nether takes the next entity id). A processor not inside decays by 4 and is dropped at 0.
//! The Nether itself is not modeled: what goes there stays there.

use crate::entity::EntityKind;
use crate::pos::BlockPos;
use crate::world::World;

/// `PortalProcessor`: inside this tick, its time, its entry cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PortalProc {
    pub inside: bool,
    pub time: i32,
    pub entry: BlockPos,
}

/// `Entity.canUsePortal(false)`: not a passenger, alive.
fn can_use_portal(w: &World, i: usize) -> bool {
    let e = &w.entities.list[i];
    if e.vehicle.is_some() || e.removed {
        return false;
    }
    match &e.kind {
        EntityKind::Mob(m) => m.health > 0.0 && !m.dead,
        _ => true,
    }
}

/// `NetherPortalBlock.entityInside` → `Entity.setAsInsidePortal(portal, pos)`.
pub fn entity_inside(w: &mut World, i: usize, pos: BlockPos) {
    if !matches!(w.entities.list[i].kind, EntityKind::Mob(_) | EntityKind::Minecart(_)) {
        let why = "an item, boat or other entity inside a nether portal (its portal travel) is not modeled yet".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return;
    }
    if !can_use_portal(w, i) {
        return;
    }
    let e = &mut w.entities.list[i];
    if e.portal_cooldown > 0 {
        e.portal_cooldown = 300;
        return;
    }
    match e.portal.as_mut() {
        None => e.portal = Some(PortalProc { inside: true, time: 0, entry: pos }),
        Some(p) if !p.inside => {
            p.entry = pos;
            p.inside = true;
        }
        Some(_) => {}
    }
}

/// `Entity.handlePortal()` of entity `i`; true when it left the overworld.
pub fn handle_portal(w: &mut World, i: usize) -> bool {
    {
        let e = &mut w.entities.list[i];
        if e.portal_cooldown > 0 {
            e.portal_cooldown -= 1;
        }
    }
    let Some(mut p) = w.entities.list[i].portal else { return false };
    let allowed = can_use_portal(w, i);
    let go = if !p.inside {
        p.time = (p.time - 4).max(0);
        false
    } else {
        p.inside = false;
        let t = p.time;
        if allowed {
            p.time += 1;
        }
        allowed && t >= 0
    };
    if go {
        w.entities.list[i].portal = Some(p);
        w.entities.list[i].portal_cooldown = 300;
        leave(w, i);
        return true;
    }
    w.entities.list[i].portal = if !p.inside && p.time <= 0 { None } else { Some(p) };
    false
}

/// The teleport to the Nether (`teleportCrossDimension`): the passengers go first (each its own
/// teleport: a copy constructed there, the next entity id), then the entity's own copy (the next
/// id); the overworld entities are removed (CHANGED_DIMENSION).
fn leave(w: &mut World, i: usize) {
    let riders = w.entities.list[i].passengers.clone();
    w.entities.list[i].passengers.clear();
    for r in riders {
        if let Some(j) = crate::entity::index_of(w, r) {
            w.entities.list[j].vehicle = None;
            leave(w, j);
        }
    }
    w.entities.next_id += 1;
    let t = w.time;
    let id = w.entities.list[i].id;
    w.entities.list[i].removed = true;
    if let EntityKind::Mob(_) = w.entities.list[i].kind {
        w.mob.events.push(crate::entity::mob::MobEvent::Removed { t, id });
    }
}
