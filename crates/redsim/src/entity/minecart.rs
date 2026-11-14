//! Minecarts (clk-misc, redsim M4): `AbstractMinecart` with `OldMinecartBehavior` (the default
//! movement, the experimental one is off), read in the 26.3 source; the hopper minecart's item
//! side (`MinecartHopper`) is not modeled yet.
//!
//! The tick, in the game's order (`AbstractMinecart.tick` does not run `Entity.baseTick`):
//! `checkBelowWorld`, `computeSpeed`, the behavior's tick (gravity; on a rail `moveAlongTrack`:
//! slope push, direction from the rail's exits, powered-rail halt, snap onto the rail's line,
//! `move` clamped to 0.4, slope exits, natural slowdown, height-change speed, corner
//! redirection, powered-rail boost or kick; off a rail `comeOffTrack`; then the blocks at its
//! position (`applyEffectsFromBlocks(pos, pos)`), the yaw from its motion with the flip rule),
//! then `updateFluidInteraction`.

use crate::block::{Block, RailShape};
use crate::entity::{self, MoverType, Vec3, mth};
use crate::pos::BlockPos;
use crate::rails;
use crate::world::World;

/// `EntityTypes.MINECART.sized(0.98F, 0.7F)` (hopper and chest minecarts too).
pub const WIDTH: f32 = 0.98;
pub const HEIGHT: f32 = 0.7;

/// The minecart's own fields.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MinecartState {
    /// A hopper minecart (`MinecartHopper`).
    pub hopper: bool,
    pub on_rails: bool,
    pub flipped: bool,
    /// A hopper minecart's `enabled` (an activator rail's power turns it off).
    pub enabled: bool,
    /// `getHurtTime()` (ironfarm, the carousel): a powered activator rail wobbles a plain cart
    /// (10, then down 1 a tick), and sets its damage to 50 (`vehicle_damage`) when it is 0.
    pub hurt_time: u8,
}

fn st_mut(w: &mut World, i: usize) -> &mut MinecartState {
    match &mut w.entities.list[i].kind {
        entity::EntityKind::Minecart(m) => m,
        _ => unreachable!("not a minecart"),
    }
}

fn st(w: &World, i: usize) -> MinecartState {
    match &w.entities.list[i].kind {
        entity::EntityKind::Minecart(m) => *m,
        _ => MinecartState::default(),
    }
}

fn in_water(w: &World, i: usize) -> bool {
    w.entities.list[i].was_touching_water
}

/// `getCurrentBlockPosOrRailBelow` (old movement).
fn current_block_pos_or_rail_below(w: &World, p: Vec3) -> BlockPos {
    let (xt, mut yt, zt) = (mth::floor(p.x), mth::floor(p.y), mth::floor(p.z));
    if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
        yt -= 1;
    }
    BlockPos::new(xt, yt, zt)
}

/// `OldMinecartBehavior.getPos(x, y, z)`: the point of the rail's line under (x, z), if on a rail.
fn get_pos(w: &World, x: f64, y: f64, z: f64) -> Option<Vec3> {
    let (xt, mut yt, zt) = (mth::floor(x), mth::floor(y), mth::floor(z));
    if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
        yt -= 1;
    }
    let s = w.get(BlockPos::new(xt, yt, zt));
    if !rails::is_rail(&s) {
        return None;
    }
    let (e0, e1) = rails::exits(rails::shape_of(&s));
    let x0 = xt as f64 + 0.5 + e0[0] as f64 * 0.5;
    let y0 = yt as f64 + 0.0625 + e0[1] as f64 * 0.5;
    let z0 = zt as f64 + 0.5 + e0[2] as f64 * 0.5;
    let x1 = xt as f64 + 0.5 + e1[0] as f64 * 0.5;
    let y1 = yt as f64 + 0.0625 + e1[1] as f64 * 0.5;
    let z1 = zt as f64 + 0.5 + e1[2] as f64 * 0.5;
    let xd = x1 - x0;
    let yd = (y1 - y0) * 2.0;
    let zd = z1 - z0;
    let progress = if xd == 0.0 {
        z - zt as f64
    } else if zd == 0.0 {
        x - xt as f64
    } else {
        let (xx, zz) = (x - x0, z - z0);
        (xx * xd + zz * zd) * 2.0
    };
    let x = x0 + xd * progress;
    let mut y = y0 + yd * progress;
    let z = z0 + zd * progress;
    if yd < 0.0 {
        y += 1.0;
    } else if yd > 0.0 {
        y += 0.5;
    }
    Some(Vec3::new(x, y, z))
}

fn set_pos(w: &mut World, i: usize, x: f64, y: f64, z: f64) {
    w.entities.set_pos(i, Vec3::new(x, y, z));
}

fn dm(w: &World, i: usize) -> Vec3 {
    w.entities.list[i].delta_movement
}

fn set_dm(w: &mut World, i: usize, v: Vec3) {
    w.entities.list[i].delta_movement = v;
}

/// `AbstractMinecart.move` (old movement): `Entity.move`, then the blocks at the new position.
fn cart_move(w: &mut World, i: usize, delta: Vec3) {
    entity::move_(w, i, MoverType::Self_, delta);
    apply_effects_from_blocks(w, i);
}

/// `AbstractMinecart.applyEffectsFromBlocks` (old movement): `applyEffectsFromBlocks(pos, pos)`
/// and the recorded moves dropped.
fn apply_effects_from_blocks(w: &mut World, i: usize) {
    let p = w.entities.list[i].pos;
    entity::collide::apply_effects_from_blocks_between(w, i, p, p);
    w.entities.list[i].movement_this_tick.clear();
}

fn is_conductor(w: &World, p: BlockPos) -> bool {
    crate::behavior::is_conductor(w, &w.get(p), p)
}

/// `OldMinecartBehavior.moveAlongTrack`.
fn move_along_track(w: &mut World, i: usize) {
    let p0 = w.entities.list[i].pos;
    let pos = current_block_pos_or_rail_below(w, p0);
    let s = w.get(pos);
    w.entities.list[i].fall_distance = 0.0;
    let (mut x, mut y, mut z) = (p0.x, p0.y, p0.z);
    let old_pos = get_pos(w, x, y, z);
    y = pos.y as f64;
    let (power_track, mut halt_track) = match s {
        Block::Rail { activator: false, powered, .. } => (powered, !powered),
        _ => (false, false),
    };
    let mut slide = 0.0078125;
    if in_water(w, i) {
        slide *= 0.2;
    }
    let m = dm(w, i);
    let shape = rails::shape_of(&s);
    match shape {
        RailShape::AscendingEast => {
            set_dm(w, i, m.add(-slide, 0.0, 0.0));
            y += 1.0;
        }
        RailShape::AscendingWest => {
            set_dm(w, i, m.add(slide, 0.0, 0.0));
            y += 1.0;
        }
        RailShape::AscendingNorth => {
            set_dm(w, i, m.add(0.0, 0.0, slide));
            y += 1.0;
        }
        RailShape::AscendingSouth => {
            set_dm(w, i, m.add(0.0, 0.0, -slide));
            y += 1.0;
        }
        _ => {}
    }
    let m = dm(w, i);
    let (e0, e1) = rails::exits(shape);
    let mut xd = (e1[0] - e0[0]) as f64;
    let mut zd = (e1[2] - e0[2]) as f64;
    let length = (xd * xd + zd * zd).sqrt();
    let flip = m.x * xd + m.z * zd;
    if flip < 0.0 {
        xd = -xd;
        zd = -zd;
    }
    let pow = entity::vec3::jmin(2.0, m.horizontal_distance());
    set_dm(w, i, Vec3::new(pow * xd / length, m.y, pow * zd / length));
    // (no passenger: no rider input)
    if halt_track {
        let speed = dm(w, i).horizontal_distance();
        if speed < 0.03 {
            set_dm(w, i, Vec3::ZERO);
        } else {
            let d = dm(w, i);
            set_dm(w, i, d.multiply(0.5, 0.0, 0.5));
        }
        halt_track = false;
    }
    let _ = halt_track;
    let x0 = pos.x as f64 + 0.5 + e0[0] as f64 * 0.5;
    let z0 = pos.z as f64 + 0.5 + e0[2] as f64 * 0.5;
    let x1 = pos.x as f64 + 0.5 + e1[0] as f64 * 0.5;
    let z1 = pos.z as f64 + 0.5 + e1[2] as f64 * 0.5;
    let xd = x1 - x0;
    let zd = z1 - z0;
    let progress = if xd == 0.0 {
        z - pos.z as f64
    } else if zd == 0.0 {
        x - pos.x as f64
    } else {
        let (xx, zz) = (x - x0, z - z0);
        (xx * xd + zz * zd) * 2.0
    };
    x = x0 + xd * progress;
    z = z0 + zd * progress;
    set_pos(w, i, x, y, z);
    // `isVehicle() ? 0.75 : 1.0` (clk-misc 2026-10-21: a rider, `/ride`)
    let ridden = !w.entities.list[i].passengers.is_empty();
    let scale = if ridden { 0.75 } else { 1.0 };
    let max_speed = if in_water(w, i) { 0.2 } else { 0.4 };
    let m = dm(w, i);
    cart_move(w, i, Vec3::new(mth::clamp(scale * m.x, -max_speed, max_speed), 0.0, mth::clamp(scale * m.z, -max_speed, max_speed)));
    let p = w.entities.list[i].pos;
    if e0[1] != 0 && mth::floor(p.x) - pos.x == e0[0] && mth::floor(p.z) - pos.z == e0[2] {
        set_pos(w, i, p.x, p.y + e0[1] as f64, p.z);
    } else if e1[1] != 0 && mth::floor(p.x) - pos.x == e1[0] && mth::floor(p.z) - pos.z == e1[2] {
        set_pos(w, i, p.x, p.y + e1[1] as f64, p.z);
    }
    // `applyNaturalSlowdown` (container minecarts: `AbstractMinecartContainer`'s, by emptiness)
    let nm = if st(w, i).hopper {
        let id = w.entities.list[i].id;
        let signal = w.containers.entities.get(&id).map(crate::redstone::container::signal_of).unwrap_or(0);
        let mut keep = 0.98f32 + (15 - signal) as f32 * 0.001f32;
        if in_water(w, i) {
            keep *= 0.95f32;
        }
        dm(w, i).multiply(keep as f64, 0.0, keep as f64)
    } else {
        // `getSlowdownFactor`: `isVehicle() ? 0.997 : 0.96`
        let slow = if ridden { 0.997 } else { 0.96 };
        let mut nm = dm(w, i).multiply(slow, 0.0, slow);
        if in_water(w, i) {
            nm = nm.scale(0.95f32 as f64);
        }
        nm
    };
    set_dm(w, i, nm);
    let p = w.entities.list[i].pos;
    let new_pos = get_pos(w, p.x, p.y, p.z);
    if let (Some(np), Some(op)) = (new_pos, old_pos) {
        let speed = (op.y - np.y) * 0.05;
        let v = dm(w, i);
        let other = v.horizontal_distance();
        if other > 0.0 {
            set_dm(w, i, v.multiply((other + speed) / other, 1.0, (other + speed) / other));
        }
        let p = w.entities.list[i].pos;
        set_pos(w, i, p.x, np.y, p.z);
    }
    let p = w.entities.list[i].pos;
    let (xn, zn) = (mth::floor(p.x), mth::floor(p.z));
    if xn != pos.x || zn != pos.z {
        let v = dm(w, i);
        let other = v.horizontal_distance();
        set_dm(w, i, Vec3::new(other * (xn - pos.x) as f64, v.y, other * (zn - pos.z) as f64));
    }
    if power_track {
        let v = dm(w, i);
        let speed = v.horizontal_distance();
        if speed > 0.01 {
            set_dm(w, i, v.add(v.x / speed * 0.06, 0.0, v.z / speed * 0.06));
        } else {
            let (mut dx, mut dz) = (v.x, v.z);
            if shape == RailShape::EastWest {
                if is_conductor(w, pos.rel(crate::pos::Dir::West)) {
                    dx = 0.02;
                } else if is_conductor(w, pos.rel(crate::pos::Dir::East)) {
                    dx = -0.02;
                }
            } else {
                if shape != RailShape::NorthSouth {
                    return;
                }
                if is_conductor(w, pos.rel(crate::pos::Dir::North)) {
                    dz = 0.02;
                } else if is_conductor(w, pos.rel(crate::pos::Dir::South)) {
                    dz = -0.02;
                }
            }
            set_dm(w, i, Vec3::new(dx, v.y, dz));
        }
    }
}

/// `AbstractMinecart.comeOffTrack`.
fn come_off_track(w: &mut World, i: usize) {
    let max = if in_water(w, i) { 0.2 } else { 0.4 };
    let m = dm(w, i);
    set_dm(w, i, Vec3::new(mth::clamp(m.x, -max, max), m.y, mth::clamp(m.z, -max, max)));
    if w.entities.list[i].on_ground {
        let d = dm(w, i);
        set_dm(w, i, d.scale(0.5));
    }
    let d = dm(w, i);
    cart_move(w, i, d);
    if !w.entities.list[i].on_ground {
        let d = dm(w, i);
        set_dm(w, i, d.scale(0.95f32 as f64));
    }
}

/// `AbstractMinecart.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    // AbstractMinecart.tick: the hurt wobble decays (`setHurtTime(getHurtTime() - 1)`), then the
    // damage an explosion or an activator rail left
    if st(w, i).hurt_time > 0 {
        st_mut(w, i).hurt_time -= 1;
    }
    {
        let e = &mut w.entities.list[i];
        if e.vehicle_damage > 0.0 {
            e.vehicle_damage -= 1.0;
        }
    }
    if w.entities.list[i].pos.y < (crate::gravity::MIN_Y - 64) as f64 {
        w.entities.list[i].removed = true;
        return;
    }
    entity::compute_speed(w, i);
    // AbstractMinecart.tick's handlePortal (the cart and its riders through a nether portal)
    if crate::entity::portal::handle_portal(w, i) {
        return;
    }
    // (tickexact 2026-11-11, the empty-cart census) a cart in lava (Entity.lavaHurt: 4 a tick) or
    // in a cactus's cell (CactusBlock.entityInside: 1 a contact tick) takes VehicleEntity damage
    // (+10 a point, destroyed past 40), which redsim does not model: refused, named
    {
        let p = w.entities.list[i].pos;
        let cells = |a: f64, b: f64| mth::floor(a)..=mth::floor(b);
        let mut hit = None;
        for x in cells(p.x - 0.49, p.x + 0.49) {
            for y in cells(p.y, p.y + 0.7) {
                for z in cells(p.z - 0.49, p.z + 0.49) {
                    if matches!(w.get(BlockPos::new(x, y, z)), Block::Lava { .. } | Block::Cactus { .. }) {
                        hit = Some(BlockPos::new(x, y, z));
                    }
                }
            }
        }
        if let Some(q) = hit {
            let why = format!("a minecart in lava or a cactus's cell ({q}): its VehicleEntity damage (lavaHurt 4 a tick, a cactus 1 a contact tick) is not modeled");
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
    }
    // OldMinecartBehavior.tick (server side)
    let g = entity::kinds::gravity(w, i);
    if g != 0.0 {
        let d = dm(w, i);
        set_dm(w, i, d.add(0.0, -g, 0.0));
    }
    let p = current_block_pos_or_rail_below(w, w.entities.list[i].pos);
    let s = w.get(p);
    let on_rails = rails::is_rail(&s);
    st_mut(w, i).on_rails = on_rails;
    if on_rails {
        move_along_track(w, i);
        // activateMinecart: a hopper minecart is enabled while the activator rail is unpowered; a
        // plain one on a powered activator rail throws its riders out (`ejectPassengers`, the
        // last first: each rider's `stopRiding`, a living one to `dismount_location`), then
        // wobbles when it is not wobbling already (hurtTime 10, damage 50)
        if let Block::Rail { activator: true, powered, .. } = s {
            if st(w, i).hopper {
                st_mut(w, i).enabled = !powered;
            } else if powered {
                let riders = w.entities.list[i].passengers.clone();
                for r in riders.iter().rev() {
                    if let Some(j) = entity::index_of(w, *r) {
                        entity::stop_riding(w, j);
                    }
                }
                if st(w, i).hurt_time == 0 {
                    st_mut(w, i).hurt_time = 10;
                    w.entities.list[i].vehicle_damage = 50.0;
                }
            }
        }
    } else {
        come_off_track(w, i);
    }
    apply_effects_from_blocks(w, i);
    let e = &mut w.entities.list[i];
    e.x_rot = 0.0;
    let x_diff = e.xo - e.pos.x;
    let z_diff = e.zo - e.pos.z;
    let flipped = st(w, i).flipped;
    let e = &mut w.entities.list[i];
    if x_diff * x_diff + z_diff * z_diff > 0.001 {
        e.y_rot = (mth::atan2(z_diff, x_diff) * 180.0 / std::f64::consts::PI) as f32;
        if flipped {
            e.y_rot += 180.0;
        }
    }
    let rot_diff = mth::wrap_degrees(e.y_rot - e.y_rot_o) as f64;
    let mut flip = false;
    if rot_diff < -170.0 || rot_diff >= 170.0 {
        e.y_rot += 180.0;
        flip = true;
    }
    e.x_rot %= 360.0;
    e.y_rot %= 360.0;
    if flip {
        let m = st_mut(w, i);
        m.flipped = !m.flipped;
    }
    // pushAndPickupEntities (OldMinecartBehavior; clk-misc 2026-10-20)
    push_and_pickup_entities(w, i);
    // (ironfarm, the carousel: `MCTAS_CART_TRACE=N` prints every cart's position and rider every
    // N gt)
    if let Some(n) = cart_trace()
        && w.time % n == 0
    {
        let e = &w.entities.list[i];
        eprintln!("cart {} t {}: {:.2} {:.2} {:.2} v {:.3} {:.3} riders {:?}", e.id, w.time, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.z, e.passengers);
    }
    entity::fluid_push::update_fluid_interaction(w, i);
    w.entities.list[i].first_tick = false;
    // MinecartHopper.tick: after the cart's tick, `tryConsumeItems` (consumedItemThisFrame is
    // reset each tick and only set here: one pull per tick)
    let m = st(w, i);
    if m.hopper && m.enabled && !w.entities.list[i].removed {
        crate::redstone::container::hopper_minecart_suck(w, i);
    }
}

/// `MCTAS_CART_TRACE`, read once.
fn cart_trace() -> Option<i64> {
    static N: std::sync::OnceLock<Option<i64>> = std::sync::OnceLock::new();
    *N.get_or_init(|| std::env::var("MCTAS_CART_TRACE").ok().and_then(|v| v.parse().ok()).filter(|n: &i64| *n > 0))
}

/// `entity.startRiding(cart)` (not forced) of mob `j` picked up by cart `i` (the carousel's
/// re-boarding): `canRide` (its boarding cooldown at 0) and `canAddPassenger` (the cart empty),
/// then the pose STANDING, the vehicle and its passengers, ENTITY_MOUNT at the cart's position.
/// The rider takes its seat when the cart ticks it (`positionRider`).
fn start_riding(w: &mut World, j: usize, i: usize) {
    if w.entities.list[j].boarding_cooldown > 0 || !w.entities.list[i].passengers.is_empty() {
        return;
    }
    let (rider, cart) = (w.entities.list[j].id, w.entities.list[i].id);
    w.entities.list[j].vehicle = Some(cart);
    w.entities.list[i].passengers.push(rider);
    // the pose STANDING: a sleeper picked up off its bed keeps its sleeping pos (no stopSleeping)
    // but rides with the standing box (refreshDimensions; fudgePositionAfterSizeChange's move is
    // undone by the seat taken in this same tick)
    if let entity::EntityKind::Mob(m) = &mut w.entities.list[j].kind
        && m.sleeping_pos.is_some()
    {
        m.pose_standing = true;
    }
    let vp = w.entities.list[i].pos;
    w.game_event_at("entity_mount", [vp.x, vp.y, vp.z], None, false);
}

/// `AbstractMinecart.getDismountLocationForPassenger` of cart `i` for a living rider `rw` wide
/// and `rh` tall standing (the only dismount pose of a villager, a zombie or a golem): around the
/// cart's block in `DismountHelper.offsetsForDirection(getMotionDirection())`'s order, at its level,
/// one up, one down, the first cell with a floor under 1 (`getBlockFloorHeight`, climbables and open
/// trapdoors empty) and no block in the rider's box there; else `Entity`'s (its x and z, its box's
/// top: the ceiling check only picks a pose).
pub fn dismount_location(w: &World, i: usize, rw: f64, rh: f64) -> Vec3 {
    let e = &w.entities.list[i];
    // getDirection: `Direction.fromYRot(yRot)`, 2D data values south, west, north, east;
    // OldMinecartBehavior.getMotionDirection: its clockwise (flipped: the opposite's clockwise)
    let d2 = [crate::pos::Dir::South, crate::pos::Dir::West, crate::pos::Dir::North, crate::pos::Dir::East][(mth::floor(e.y_rot as f64 / 90.0 + 0.5) & 3) as usize];
    let forward = if st(w, i).flipped { d2.opposite().clockwise() } else { d2.clockwise() };
    let right = forward.clockwise();
    let (left, back) = (right.opposite(), forward.opposite());
    let s = |d: crate::pos::Dir| (d.step()[0], d.step()[2]);
    let add = |a: (i32, i32), b: (i32, i32)| (a.0 + b.0, a.1 + b.1);
    let offsets = [s(right), s(left), add(s(back), s(right)), add(s(back), s(left)), add(s(forward), s(right)), add(s(forward), s(left)), s(back), s(forward)];
    let base = BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y), mth::floor(e.pos.z));
    let reach = rw.min(1.0) / 2.0;
    for dy in [0, 1, -1] {
        for (dx, dz) in offsets {
            let p = BlockPos::new(base.x + dx, base.y + dy, base.z + dz);
            let floor = match crate::entity::mob::villager::non_climbable_top(w, p) {
                Some(top) => top,
                None => match crate::entity::mob::villager::non_climbable_top(w, p.below()) {
                    Some(b) if b >= 1.0 => b - 1.0,
                    _ => f64::NEG_INFINITY,
                },
            };
            if !(floor.is_finite() && floor < 1.0) {
                continue;
            }
            let at = Vec3::new(p.x as f64 + 0.5, p.y as f64 + floor, p.z as f64 + 0.5);
            let bb = crate::entity::Aabb::new(at.x - reach, at.y, at.z - reach, at.x + reach, at.y + rh, at.z + reach);
            if !crate::entity::collide::block_collisions(w, bb).iter().any(|(_, boxes)| boxes.iter().any(|b| b.intersects(bb))) {
                return at;
            }
        }
    }
    Vec3::new(e.pos.x, e.bb.max_y, e.pos.z)
}

/// `isPushable()` of a modeled entity: boats and minecarts (armor stands and items are not; living
/// mobs are not modeled).
pub fn is_pushable(e: &entity::Entity) -> bool {
    match &e.kind {
        entity::EntityKind::Boat(_) | entity::EntityKind::Minecart(_) => true,
        // a mob (ironfarm, agreed with boat-leads): LivingEntity.isPushable = isAlive &&
        // !isSpectator && !onClimbable; redsim mobs are never spectators and every entity ticks
        entity::EntityKind::Mob(m) => !m.dead && !m.on_climbable,
        _ => false,
    }
}

/// `OldMinecartBehavior.pushAndPickupEntities` of cart `i` (clk-misc, 2026-10-20). A rideable cart
/// (a plain minecart) moving at least 0.1 per tick sideways picks up the pushable entities in its box
/// grown by 0.2 (boats: `startRiding`, not modeled) and pushes other minecarts; otherwise every other
/// minecart in that box is pushed (`entity.push(this cart)`: [`push_by`] on THAT cart). Entities in list
/// order (`level.getEntities`).
fn push_and_pickup_entities(w: &mut World, i: usize) {
    let hitbox = w.entities.list[i].bb.inflate(0.2f32 as f64, 0.0, 0.2f32 as f64);
    let rideable = !st(w, i).hopper;
    let moving = w.entities.list[i].delta_movement.horizontal_distance_sqr() >= 0.01;
    let is_cart = |e: &entity::Entity| matches!(e.kind, entity::EntityKind::Minecart(_));
    if rideable && moving {
        let found: Vec<usize> = w.get_entities(hitbox, is_pushable).into_iter().filter(|&j| j != i).collect();
        for j in found {
            if is_cart(&w.entities.list[j]) {
                push_by(w, j, i);
                continue;
            }
            // (not a player, an iron golem or a minecart, the cart carrying nobody, the entity
            // riding nothing): `entity.startRiding(cart)`; otherwise `entity.push(cart)`
            let golem = matches!(&w.entities.list[j].kind, entity::EntityKind::Mob(m) if matches!(m.ty, crate::entity::mob::types::MobType::IronGolem));
            let boards = !golem && w.entities.list[i].passengers.is_empty() && w.entities.list[j].vehicle.is_none();
            match &w.entities.list[j].kind {
                entity::EntityKind::Mob(_) if boards => start_riding(w, j, i),
                entity::EntityKind::Mob(m) => {
                    // `LivingEntity.push(entity)`: nothing while it sleeps
                    if m.sleeping_pos.is_none() {
                        entity::push::generic(w, j, i);
                    }
                }
                _ if boards => w.errors.push("a moving minecart picking up a boat (startRiding: passengers) is not modeled".into()),
                _ => entity::push::generic(w, j, i),
            }
        }
    } else {
        let found: Vec<usize> = w.get_entities(hitbox, |e| is_cart(e)).into_iter().filter(|&j| j != i).collect();
        for j in found {
            push_by(w, j, i);
        }
    }
}

/// `AbstractMinecart.push(Entity entity)` on cart `j` (this) by entity `i`: another minecart
/// (`pushOtherMinecart`), or any other entity not riding it (a mob's `pushEntities` touching the
/// cart: the cart pushed by `(-xa, 0, -za)` and the entity by a quarter of the opposite).
fn push_by(w: &mut World, j: usize, i: usize) {
    let (this, other) = (&w.entities.list[j], &w.entities.list[i]);
    if this.removed || other.removed || this.no_physics || other.no_physics || this.passengers.contains(&other.id) {
        return;
    }
    let mut xa = other.pos.x - this.pos.x;
    let mut za = other.pos.z - this.pos.z;
    let mut dd = xa * xa + za * za;
    if !(dd >= 1.0e-4f32 as f64) {
        // (cc767b78, the riderless merge: a cart-on-cart push skipped because the centres
        // coincide, logged when asked)
        if matches!(other.kind, entity::EntityKind::Minecart(_)) && w.mob.cart_cart_pushes.is_some() {
            let (tm, om) = (this.delta_movement, other.delta_movement);
            let p = crate::entity::mob::CartCartPush { t: w.time, this: this.id, other: other.id, dist: dd.sqrt(), dot: f64::NAN, what: "merged", before: [tm.x, tm.z, om.x, om.z], after: [tm.x, tm.z, om.x, om.z] };
            if let Some(log) = w.mob.cart_cart_pushes.as_mut() {
                log.push(p);
            }
        }
        return;
    }
    dd = dd.sqrt();
    xa /= dd;
    za /= dd;
    let mut pow = 1.0 / dd;
    if pow > 1.0 {
        pow = 1.0;
    }
    xa *= pow;
    za *= pow;
    xa *= 0.1f32 as f64;
    za *= 0.1f32 as f64;
    xa *= 0.5;
    za *= 0.5;
    if matches!(w.entities.list[i].kind, entity::EntityKind::Minecart(_)) {
        push_other_minecart(w, j, i, xa, za);
    } else {
        // (tickexact 2026-11-11, ironspec's push rule) a mob's push logged when asked
        if matches!(w.entities.list[i].kind, entity::EntityKind::Mob(_)) && w.mob.cart_pushes.is_some() {
            let p = crate::entity::mob::CartPush { t: w.time, mob: w.entities.list[i].id, vehicle: w.entities.list[i].vehicle, cart: w.entities.list[j].id, push: [-xa, -za], dist: dd };
            if let Some(log) = w.mob.cart_pushes.as_mut() {
                log.push(p);
            }
        }
        // `this.push(-xa, 0, -za); entity.push(xa / 4, 0, za / 4)` (Entity.push: the delta; its
        // hasImpulse only syncs clients)
        let c = &mut w.entities.list[j];
        c.delta_movement = c.delta_movement.add(-xa, 0.0, -za);
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(xa / 4.0, 0.0, za / 4.0);
    }
}

/// A mob's `pushEntities` meeting cart `j`: `doPush` = `cart.push(mob)` ([`push_by`]).
pub fn pushed_by_mob(w: &mut World, j: usize, i: usize) {
    push_by(w, j, i);
}

/// (tickexact 2026-11-11, ironspec's push rule) Start logging every push a mob gives a minecart
/// (`MobWorld::cart_pushes`; a diagnostic, no model state).
pub fn log_cart_pushes(w: &mut World) {
    if w.mob.cart_pushes.is_none() {
        w.mob.cart_pushes = Some(Vec::new());
    }
}

/// The mob-on-cart pushes logged since the last call (empty unless [`log_cart_pushes`] ran).
pub fn take_cart_pushes(w: &mut World) -> Vec<crate::entity::mob::CartPush> {
    w.mob.cart_pushes.as_mut().map(std::mem::take).unwrap_or_default()
}

/// `AbstractMinecart.pushOtherMinecart(otherMinecart, xa, za)` on cart `j` (this), `i` the other
/// (no furnace minecarts are modeled; old movement: the direction between the carts).
fn push_other_minecart(w: &mut World, j: usize, i: usize, xa: f64, za: f64) {
    let (this, other) = (&w.entities.list[j], &w.entities.list[i]);
    let dir = Vec3::new(other.pos.x - this.pos.x, 0.0, other.pos.z - this.pos.z).normalize();
    let r = this.y_rot * ((std::f64::consts::PI / 180.0) as f32);
    let facing = Vec3::new(mth::cos(r as f64) as f64, 0.0, mth::sin(r as f64) as f64).normalize();
    let dot = dir.dot(facing).abs();
    // (cc767b78, the riderless merge: every cart-on-cart push logged when asked, with its
    // alignment test and the motions before and after)
    let logging = w.mob.cart_cart_pushes.is_some();
    let rec = logging.then(|| (w.time, this.id, other.id, ((other.pos.x - this.pos.x).powi(2) + (other.pos.z - this.pos.z).powi(2)).sqrt(), [this.delta_movement.x, this.delta_movement.z, other.delta_movement.x, other.delta_movement.z]));
    let log = |w: &mut World, what: &'static str| {
        if let Some((t, a, b, dist, before)) = rec {
            let (ma, mb) = (w.entities.list[j].delta_movement, w.entities.list[i].delta_movement);
            let p = crate::entity::mob::CartCartPush { t, this: a, other: b, dist, dot, what, before, after: [ma.x, ma.z, mb.x, mb.z] };
            if let Some(l) = w.mob.cart_cart_pushes.as_mut() {
                l.push(p);
            }
        }
    };
    if dot < 0.8f32 as f64 {
        log(w, "misaligned");
        return;
    }
    let movement = this.delta_movement;
    let entity_movement = other.delta_movement;
    let xdd = (entity_movement.x + movement.x) / 2.0;
    let zdd = (entity_movement.z + movement.z) / 2.0;
    let push = |v: Vec3, x: f64, z: f64| if x.is_finite() && z.is_finite() { v.add(x, 0.0, z) } else { v };
    w.entities.list[j].delta_movement = push(movement.multiply(0.2, 1.0, 0.2), xdd - xa, zdd - za);
    w.entities.list[i].delta_movement = push(entity_movement.multiply(0.2, 1.0, 0.2), xdd + xa, zdd + za);
    log(w, "applied");
}

/// (cc767b78) Start logging every minecart-on-minecart push (`MobWorld::cart_cart_pushes`; a
/// diagnostic, no model state).
pub fn log_cart_cart_pushes(w: &mut World) {
    if w.mob.cart_cart_pushes.is_none() {
        w.mob.cart_cart_pushes = Some(Vec::new());
    }
}

/// The minecart-on-minecart pushes logged since the last call (empty unless
/// [`log_cart_cart_pushes`] ran).
pub fn take_cart_cart_pushes(w: &mut World) -> Vec<crate::entity::mob::CartCartPush> {
    w.mob.cart_cart_pushes.as_mut().map(std::mem::take).unwrap_or_default()
}

#[cfg(test)]
mod slope_rest_tests {
    use crate::block::Block;
    use crate::pos::BlockPos;
    use crate::world::World;

    /// One row of tick-exact's AH slope-string rig (crates/mobai/src/slopestring.rs, Nov 13, the official 26.3 server):
    /// R_1's geometry, the slot cell (0,17,1) ascending south on stone (an unpowered powered rail, or a plain rail when
    /// `slot_plain`), a flat rail north of it, the next ascending rail south (likewise `upper_plain`), the slot-1 string
    /// one cell above (0,18,1) between attached hooks on glass; an empty cart summoned at `cart` (position, motion), and
    /// optionally the hold full (a ridden cart north of the slot at z `hold`). Per tick for `ticks`: the string powered,
    /// the cart's y and z.
    fn rig_row(slot_plain: bool, upper_plain: bool, cart: Option<([f64; 3], [f64; 3])>, hold: Option<f64>, ticks: usize) -> Vec<(bool, f64, f64)> {
        let mut w = World::new(BlockPos::new(-4, 14, -3), [9, 8, 9]);
        let b = |s: &str| Block::parse(s).unwrap();
        let raw = crate::world::flags::RAW;
        let rail = |plain: bool| if plain { "minecraft:rail[shape=ascending_south,waterlogged=false]" } else { "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]" };
        for (p, s) in [
            ([0, 16, -1], "minecraft:stone"),
            ([0, 16, 0], "minecraft:stone"),
            ([0, 16, 1], "minecraft:stone"),
            ([0, 17, 2], "minecraft:stone"),
            ([0, 18, 3], "minecraft:stone"),
            ([0, 17, -1], "minecraft:rail[shape=north_south,waterlogged=false]"),
            ([0, 17, 0], "minecraft:rail[shape=north_south,waterlogged=false]"),
            ([0, 17, 1], rail(slot_plain)),
            ([0, 18, 2], rail(upper_plain)),
            ([0, 19, 3], "minecraft:rail[shape=north_south,waterlogged=false]"),
            ([-2, 18, 1], "minecraft:glass"),
            ([2, 18, 1], "minecraft:glass"),
            ([-1, 18, 1], "minecraft:tripwire_hook[attached=true,facing=east,powered=false]"),
            ([1, 18, 1], "minecraft:tripwire_hook[attached=true,facing=west,powered=false]"),
            ([0, 18, 1], "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]"),
        ] {
            w.set_block(BlockPos::new(p[0], p[1], p[2]), b(s), raw);
        }
        let id = cart.map(|(pos, m)| {
            let nbt = format!("{{Motion:[{:?}d,{:?}d,{:?}d]}}", m[0], m[1], m[2]);
            crate::entity::spawn(&mut w, "minecraft:minecart", pos, Some(&nbt)).unwrap()
        });
        if let Some(hz) = hold {
            crate::entity::spawn(&mut w, "minecraft:minecart", [0.5, 17.0625, hz], Some("{Passengers:[{id:\"minecraft:villager\",PersistenceRequired:1b,NoAI:1b}]}")).unwrap();
            super::log_cart_pushes(&mut w);
        }
        let mut out = Vec::new();
        for t in 0..ticks {
            w.tick();
            if hold.is_some() && t < 2 {
                for p in super::take_cart_pushes(&mut w) {
                    println!("  tick {}: mob push {p:?}", t + 1);
                }
            }
            let (y, z) = id.and_then(|id| crate::entity::index_of(&w, id)).map_or((0.0, 0.0), |i| (w.entities.list[i].pos.y, w.entities.list[i].pos.z));
            let on = w.get(BlockPos::new(0, 18, 1)).to_string().contains("powered=true");
            out.push((on, y, z));
        }
        out
    }

    /// The game's summon at rest on the slot cell: the rail's own slope y (cell y + (z - 1) + 0.0625), no motion.
    fn rest(z: f64) -> Option<([f64; 3], [f64; 3])> {
        Some(([0.5, 17.0 + (z - 1.0) + 0.0625, z], [0.0; 3]))
    }

    /// The pressed ticks (1-based) of a row.
    fn pressed(rows: &[(bool, f64, f64)]) -> Vec<usize> {
        rows.iter().enumerate().filter(|(_, r)| r.0).map(|(t, _)| t + 1).collect()
    }

    /// tick-exact's AH game rows (Nov 13, `mctas mobai slope-string`, its asserts 1..9), replayed in redsim:
    /// - (ii) at rest: the cart stays where it was summoned; the string pressed from t1 and released only at its 10-gt
    ///   re-checks t11 / t22 / t33 at the drawn rest z 1.28 (its end box 0.02 under the band: pressed by
    ///   AbstractMinecart.move's applyEffectsFromBlocks at moveAlongTrack's RAISED position, y + 1 before the slope snap;
    ///   the re-check reads the end box), every tick at the measured rests 1.79 / 1.856 / 1.89;
    /// - (i) moving through on plain rails: one press held to its re-check, t5..t14 up at 0.4 and 0.6 b/t, t2..t11 down
    ///   at 0.2 b/t;
    /// - (iii) rolling in from rest at z 2.6 on the plain rail above R_1: it comes to rest on R_1 (z 1.6..2.0) and, once
    ///   pressed, the string never releases;
    /// - the control (no cart): never pressed.
    #[test]
    fn the_ah_slope_string_rows_match_the_game() {
        let at_rest = rig_row(false, false, rest(1.28), None, 40);
        assert_eq!(pressed(&at_rest).len(), 37, "(ii) z 1.28: 37 of 40");
        assert!(!at_rest[10].0 && !at_rest[21].0 && !at_rest[32].0 && at_rest[0].0, "(ii) z 1.28: pressed from t1, released at t11 / t22 / t33");
        assert!(at_rest.iter().all(|r| r.2 == 1.28), "(ii) z 1.28: the cart stays");
        for z in [1.79, 1.856, 1.89] {
            let rows = rig_row(false, false, rest(z), None, 40);
            assert_eq!(pressed(&rows).len(), 40, "(ii) z {z}: every tick");
            assert!(rows.iter().all(|r| r.2 == z), "(ii) z {z}: the cart stays");
        }
        for v in [0.4, 0.6] {
            let rows = rig_row(true, true, Some(([0.5, 17.0625, -0.5], [0.0, 0.0, v])), None, 40);
            assert_eq!(pressed(&rows), (5..=14).collect::<Vec<_>>(), "(i) up at {v}: one press t5..t14");
        }
        let down = rig_row(true, true, Some(([0.5, 18.5625, 2.5], [0.0, 0.0, -0.2])), None, 40);
        assert_eq!(pressed(&down), (2..=11).collect::<Vec<_>>(), "(i) down at 0.2: one press t2..t11");
        let roll = rig_row(false, true, Some(([0.5, 18.6625, 2.6], [0.0; 3])), None, 40);
        let p = pressed(&roll);
        let first = *p.first().expect("(iii) pressed");
        assert!(p == (first..=40).collect::<Vec<_>>(), "(iii) once pressed, never released: {p:?}");
        let rz = roll.last().unwrap().2;
        assert!((1.6..=2.0).contains(&rz), "(iii) rests on R_1: z {rz}");
        assert!(pressed(&rig_row(false, false, None, None, 40)).is_empty(), "the control: never pressed");
    }

    /// Prints the rows, and the rest at z 1.28 with the hold full (a ridden cart north of the slot): the push question
    /// tick-exact's recording (ii) decides.
    #[test]
    #[ignore]
    fn slope_rest_rig_rows() {
        for z in [1.28, 1.79, 1.856, 1.89] {
            let rows = rig_row(false, false, rest(z), None, 40);
            let last = rows.last().copied().unwrap();
            println!("z {z}: pressed {}/40; the cart at the end y {:.4} z {:.4}", pressed(&rows).len(), last.1, last.2);
        }
        for hz in [0.49, 0.50, 0.30] {
            let rows = rig_row(false, false, rest(1.28), Some(hz), 40);
            let last = rows.last().copied().unwrap();
            println!("z 1.28 with the hold's ridden cart at z {hz}: pressed {}/40; the slot cart at the end y {:.4} z {:.4}", pressed(&rows).len(), last.1, last.2);
        }
        let roll = rig_row(false, true, Some(([0.5, 18.6625, 2.6], [0.0; 3])), None, 40);
        println!("(iii) rolling in: pressed {:?}; rests at z {:.4}", pressed(&roll), roll.last().unwrap().2);
    }
}
