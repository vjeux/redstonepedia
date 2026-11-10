//! Nether portals on the overworld side (the mob-AI lane, ironspec's carousel: golems moved away
//! through a portal). `NetherPortalBlock.entityInside` (its inside shape the default full block:
//! every portal cell the entity's deflated box meets) calls `Entity.setAsInsidePortal`: on its
//! cooldown the cooldown restarts (300), else the `PortalProcessor` is made or marked inside this
//! tick. `Entity.handlePortal` (in `baseTick`, before the sprint particle): the cooldown counts
//! down; a processor inside this tick for an entity that may use a portal (no passenger of
//! anything, alive) teleports at once (`getPortalTransitionTime` is 0 for all but players): the
//! cooldown set to 300, the entity leaves the overworld (removed, CHANGED_DIMENSION; its copy in
//! the Nether takes the next entity id). A processor not inside decays by 4 and is dropped at 0.
//! Without a modeled Nether (`World::nether`) what goes there stays there; with one (tickexact
//! 2026-11-09, ironspec's portal pad), the copy is placed there: the closest Nether portal within
//! 16 of the position over 8, `createDimensionTransition` (a living entity centred on the plane),
//! `findCollisionFreePosition`, the PORTAL ticket (300 gt) that keeps the Nether ticking; and back
//! (within 128 of the position times 8). Items cross too.

use crate::entity::{Aabb, EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::World;

/// The ticks a crossing keeps the Nether's entities ticking (the arrival tick included): the
/// PORTAL ticket's 300 and the chunk's unload after it, as measured (`tick_nether`).
pub const PORTAL_TICKS: i64 = 306;

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
    if !matches!(w.entities.list[i].kind, EntityKind::Mob(_) | EntityKind::Minecart(_) | EntityKind::Item(_)) {
        let why = "a boat or other entity inside a nether portal (its portal travel) is not modeled yet".to_string();
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
/// id); the overworld entities are removed (CHANGED_DIMENSION). With a modeled Nether
/// (`World::nether`, or this world the Nether), the copy goes there ([`cross`]).
fn leave(w: &mut World, i: usize) {
    let riders = w.entities.list[i].passengers.clone();
    w.entities.list[i].passengers.clear();
    let modeled = w.nether.is_some() || w.ultrawarm;
    if modeled && !riders.is_empty() {
        error(w, "a vehicle with riders through a nether portal into a modeled Nether (the riders' own crossings, remounted there) is not modeled yet".into());
    }
    for r in riders {
        if let Some(j) = crate::entity::index_of(w, r) {
            w.entities.list[j].vehicle = None;
            leave(w, j);
        }
    }
    let copy = w.entities.next_id;
    w.entities.next_id += 1;
    let t = w.time;
    let id = w.entities.list[i].id;
    w.entities.list[i].removed = true;
    if let EntityKind::Mob(_) = w.entities.list[i].kind {
        w.mob.events.push(crate::entity::mob::MobEvent::Removed { t, id });
    }
    if modeled {
        cross(w, i, copy);
    }
}

fn error(w: &mut World, why: String) {
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

// ---- the Nether (tickexact 2026-11-09, ironspec's portal pad; measured on the official server:
// `mctas mobai portal-probe --nether`) ----

/// A nether portal's rectangle (`BlockUtil.FoundRectangle`): its min corner, its cells along the
/// portal's axis and up, the axis (true: x).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub corner: BlockPos,
    pub w: i32,
    pub h: i32,
    pub x_axis: bool,
}

/// An entity through a portal out of a Nether ([`World::outgoing`]), for the overworld to place:
/// its copy (the next entity id), the target (its position scaled by 8), the source portal's
/// axis, its relative position there (`getRelativePortalPosition`).
#[derive(Clone, PartialEq, Debug)]
pub struct Crossing {
    pub entity: crate::entity::Entity,
    pub target: [f64; 3],
    pub x_axis: bool,
    pub rel: Vec3,
    /// The id of the entity that went through (the copy's is `entity.id`).
    pub from: u32,
}

/// The axis of the nether portal block at `p` (true: x); None when `p` holds none.
fn portal_axis(w: &World, p: BlockPos) -> Option<bool> {
    match w.get(p) {
        crate::block::Block::Terrain(id) => {
            let pr = crate::terrain::blocks::props(id);
            (pr.block_id() == "minecraft:nether_portal").then(|| pr.name.contains("axis=x"))
        }
        _ => None,
    }
}

/// `BlockUtil.getLargestRectangleAround(p, axis, 21, Y, 21, the same state)` for a portal that is
/// one whole rectangle (the game's search finds that rectangle); an irregular one is refused.
pub fn rect_at(w: &World, p: BlockPos) -> Result<Rect, String> {
    let s = w.get(p);
    let x_axis = portal_axis(w, p).ok_or_else(|| format!("no nether portal at {p}"))?;
    let step = |q: BlockPos, k: i32| if x_axis { q.offset(k, 0, 0) } else { q.offset(0, 0, k) };
    // BlockUtil.getLimit: the cells past `p` in one direction holding the same state, at most 21
    let limit = |f: &dyn Fn(BlockPos) -> BlockPos| {
        let (mut q, mut i) = (p, 0);
        while i < 21 && w.get(f(q)) == s {
            q = f(q);
            i += 1;
        }
        i
    };
    let neg = limit(&|q| step(q, -1));
    let pos = limit(&|q| step(q, 1));
    let down = limit(&|q| q.offset(0, -1, 0));
    let up = limit(&|q| q.offset(0, 1, 0));
    let corner = step(p, -neg).offset(0, -down, 0);
    let (wd, ht) = (neg + pos + 1, down + up + 1);
    for a in 0..wd {
        for y in 0..ht {
            if w.get(step(corner, a).offset(0, y, 0)) != s {
                return Err(format!("the nether portal at {p} is not one whole rectangle (BlockUtil.getLargestRectangleAround is modeled for whole ones)"));
            }
        }
    }
    Ok(Rect { corner, w: wd, h: ht, x_axis })
}

/// `PortalShape.getRelativePosition(rect, axis, pos, dims)`, then for a living entity
/// `LivingEntity.resetForwardDirectionOfRelativePortalPosition` (its offset across the plane
/// dropped: it arrives centred on the destination's plane).
pub fn relative(r: &Rect, pos: Vec3, dims: (f32, f32), living: bool) -> Vec3 {
    let (wd, ht) = (dims.0 as f64, dims.1 as f64);
    let ww = r.w as f64 - wd;
    let hh = r.h as f64 - ht;
    let rx = if ww > 0.0 {
        let (along, c) = if r.x_axis { (pos.x, r.corner.x as f64 + wd / 2.0) } else { (pos.z, r.corner.z as f64 + wd / 2.0) };
        ((along - c - 0.0) / (ww - 0.0)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let ry = if hh > 0.0 { ((pos.y - r.corner.y as f64 - 0.0) / (hh - 0.0)).clamp(0.0, 1.0) } else { 0.0 };
    let rz = if r.x_axis { pos.z - (r.corner.z as f64 + 0.5) } else { pos.x - (r.corner.x as f64 + 0.5) };
    Vec3::new(rx, ry, if living { 0.0 } else { rz })
}

/// `NetherPortalBlock.createDimensionTransition`'s position: the relative position mapped onto
/// the destination rectangle.
pub fn arrival(r: &Rect, rel: Vec3, dims: (f32, f32)) -> Vec3 {
    let (wd, ht) = (dims.0 as f64, dims.1 as f64);
    let dx = wd / 2.0 + (r.w as f64 - wd) * rel.x;
    let dy = (r.h as f64 - ht) * rel.y;
    let dz = 0.5 + rel.z;
    if r.x_axis {
        Vec3::new(r.corner.x as f64 + dx, r.corner.y as f64 + dy, r.corner.z as f64 + dz)
    } else {
        Vec3::new(r.corner.x as f64 + dz, r.corner.y as f64 + dy, r.corner.z as f64 + dx)
    }
}

/// `PortalShape.findCollisionFreePosition`: an entity at most 4 x 4 moved to the free position
/// closest to its centre within its width around it and 1 up (`CollisionGetter.findFreePosition`).
pub fn collision_free(w: &World, pos: Vec3, dims: (f32, f32)) -> Vec3 {
    if dims.0 > 4.0 || dims.1 > 4.0 {
        return pos;
    }
    let (wd, ht) = (dims.0 as f64, dims.1 as f64);
    let h2 = ht / 2.0;
    let c = pos.add(0.0, h2, 0.0);
    let allowed = Aabb::new(c.x - wd / 2.0, c.y - 0.0 / 2.0, c.z - wd / 2.0, c.x + wd / 2.0, c.y + 0.0 / 2.0 + 1.0, c.z + wd / 2.0).inflate(1.0e-6, 1.0e-6, 1.0e-6);
    match crate::entity::find_free_position(w, allowed, c, wd, ht, wd) {
        Some(q) => q.add(0.0, -h2, 0.0),
        None => pos,
    }
}

/// `PortalForcer.findClosestPortalPosition(target, toNether, border)`: the portal cells in the
/// square of `radius` around the target (every y of the world), the closest by `distSqr`, then the
/// lowest.
pub fn closest_portal(w: &World, target: BlockPos, radius: i32) -> Option<BlockPos> {
    let mut best: Option<(f64, i32, BlockPos)> = None;
    let (x0, x1) = ((target.x - radius).max(w.min.x), (target.x + radius).min(w.min.x + w.size[0] - 1));
    let (z0, z1) = ((target.z - radius).max(w.min.z), (target.z + radius).min(w.min.z + w.size[2] - 1));
    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in w.min.y..w.min.y + w.size[1] {
                let p = BlockPos::new(x, y, z);
                if portal_axis(w, p).is_some() {
                    let (dx, dy, dz) = ((x - target.x) as f64, (y - target.y) as f64, (z - target.z) as f64);
                    let d = dx * dx + dy * dy + dz * dz;
                    if best.is_none_or(|(bd, by, _)| d < bd || (d == bd && y < by)) {
                        best = Some((d, y, p));
                    }
                }
            }
        }
    }
    best.map(|b| b.2)
}

/// The copy of entity `i` (through its portal) leaving for the other world: into the modeled
/// Nether now (`World::nether`), or (this world the Nether) queued for the overworld.
fn cross(w: &mut World, i: usize, copy: u32) {
    let e = &w.entities.list[i];
    let Some(p) = e.portal else { return };
    let living = matches!(e.kind, EntityKind::Mob(_));
    let src = match rect_at(w, p.entry) {
        Ok(r) => r,
        Err(why) => return error(w, why),
    };
    let rel = relative(&src, e.pos, e.kind.dims(), living);
    let mut c = e.clone();
    c.id = copy;
    c.removed = false;
    c.portal = None;
    c.vehicle = None;
    c.passengers.clear();
    let from = w.entities.list[i].id;
    if w.ultrawarm {
        let target = [c.pos.x * 8.0, c.pos.y, c.pos.z * 8.0];
        w.outgoing.push(Crossing { entity: c, target, x_axis: src.x_axis, rel, from });
    } else {
        let target = [c.pos.x * 0.125, c.pos.y, c.pos.z * 0.125];
        let now = w.time;
        w.crossings.push((now, from, copy));
        let Some(n) = w.nether.as_deref_mut() else { return };
        place(n, Crossing { entity: c, target, x_axis: src.x_axis, rel, from }, 16, true);
    }
}

/// `getExitPortal` and the teleport in world `w`: the closest portal to the target within
/// `radius` (none: `PortalForcer.createPortal`, not modeled), the entity placed by
/// `createDimensionTransition` (yaw turned 90 when the axes differ; its motion kept), appended to
/// `w`'s entities; into the Nether, the PORTAL ticket on the found portal's chunk.
fn place(w: &mut World, c: Crossing, radius: i32, ticket: bool) {
    let t = BlockPos::new(c.target[0].floor() as i32, c.target[1].floor() as i32, c.target[2].floor() as i32);
    let Some(found) = closest_portal(w, t, radius) else {
        return error(w, format!("no nether portal within {radius} blocks of {t} (PortalForcer.createPortal, a portal made there, is not modeled)"));
    };
    let r = match rect_at(w, found) {
        Ok(r) => r,
        Err(why) => return error(w, why),
    };
    let mut e = c.entity;
    // the copy is a new entity (`EntityType.create`, then `restoreFrom` its saved data): its own
    // random is a new one, constructed in the destination
    if e.random.is_some() {
        e.random = Some(w.construct());
    }
    let dims = e.kind.dims();
    let pos = collision_free(w, arrival(&r, c.rel, dims), dims);
    // Entity.load gave the head and the body the saved yaw; the transition's relative rotation
    // then turns the yaw by 90 when the portals' axes differ
    let loaded = e.y_rot;
    if let EntityKind::Mob(m) = &mut e.kind
        && let Some(a) = m.ai.as_deref_mut()
    {
        a.y_head_rot = loaded;
        a.y_body_rot = loaded;
    }
    e.y_rot += if r.x_axis == c.x_axis { 0.0 } else { 90.0 };
    // snapTo: the position, the old position and rotation the same
    e.set_pos(pos);
    e.xo = pos.x;
    e.yo = pos.y;
    e.zo = pos.z;
    e.y_rot_o = e.y_rot;
    e.x_rot_o = e.x_rot;
    w.entities.insert(e);
    if ticket {
        let chunk = [found.x.div_euclid(16), found.z.div_euclid(16)];
        let now = w.time;
        match w.portal_tickets.iter_mut().find(|(c, _)| *c == chunk) {
            Some(t) => t.1 = now,
            None => w.portal_tickets.push((chunk, now)),
        }
    }
}

/// The Nether's level tick (after the overworld's, `MinecraftServer.tickChildren`): only while a
/// PORTAL ticket holds it (300 gt after the last crossing into it), its entities ticking within 1
/// chunk of the ticket's and its blocks within 2; then the entities that crossed out of it placed
/// in the overworld (the closest overworld portal within 128 of their position times 8).
pub fn tick_nether(w: &mut World) {
    let Some(mut n) = w.nether.take() else { return };
    let now = w.time;
    // (measured, `portal-probe --ticket`: an item beside the Nether portal kept ageing through 305
    // ticks after the crossing, then its chunk unloaded: the ticket's 300 gt, then the unload's
    // delay, which varies with the server's unload queue: 307 to 311 for a golem's chunk)
    n.portal_tickets.retain(|(_, made)| now - made < PORTAL_TICKS);
    if let Some(&([cx, cz], _)) = n.portal_tickets.first() {
        let (x0, x1) = (n.min.x.div_euclid(16), (n.min.x + n.size[0] - 1).div_euclid(16));
        let (z0, z1) = (n.min.z.div_euclid(16), (n.min.z + n.size[2] - 1).div_euclid(16));
        if (x0 - cx).abs() > 2 || (x1 - cx).abs() > 2 || (z0 - cz).abs() > 2 || (z1 - cz).abs() > 2 {
            error(w, format!("the Nether build reaches past the 5 x 5 chunks the PORTAL ticket at chunk {cx} {cz} keeps ticking (blocks there would not tick)"));
        }
        n.entity_chunks = Some([cx - 1, cz - 1, cx + 1, cz + 1]);
        n.time = now - 1;
        n.entities.next_id = w.entities.next_id;
        n.tick();
        w.entities.next_id = n.entities.next_id;
    }
    n.time = now;
    let out = std::mem::take(&mut n.outgoing);
    let errs = std::mem::take(&mut n.errors);
    w.nether = Some(n);
    for e in errs {
        error(w, format!("the Nether: {e}"));
    }
    for c in out {
        let now = w.time;
        w.crossings.push((now, c.from, c.entity.id));
        place(w, c, 128, false);
    }
}
