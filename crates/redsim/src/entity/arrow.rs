//! Arrows (the mob-AI lane): `Arrow`, `AbstractArrow` and `Projectile` read in the 26.3 source
//! (`EntityTypes.ARROW`: 0.5 x 0.5, eye height 0.13).
//!
//! - The shot (`AbstractSkeleton.performRangedAttack`): `new Arrow(level, mob, ..)` at the
//!   shooter's x, eye y less 0.1F and z, owned by it; `setBaseDamageFromMob(power)` (`power x 2 +
//!   triangle(difficulty x 0.11, 0.57425)` from the arrow's own random); `shoot(xd, yd + the
//!   horizontal distance x 0.2F, zd, 1.6F, 14 - difficulty x 4)` (the direction normalized, three
//!   `triangle(0, 0.0172275 x uncertainty)`, x 1.6, the rotations from it); `addFreshEntity`;
//!   then the shooter's sound pitch from its own random.
//! - The flight (`AbstractArrow.tick`): an arrow inside a block's collision box is in the ground;
//!   else its rotations ease toward its motion (`lerpRotation`), it checks it left its owner
//!   (`checkLeftOwner`), clips its motion against the blocks (`Block.COLLIDER`, no fluids) and the
//!   entities its swept box meets (`ProjectileUtil.getManyEntityHitResult`, `computeMargin`),
//!   nearest first, moves to the first hit and hits it; 0.6 inertia in water, 0.99 drag out of it,
//!   gravity 0.05; then `Projectile.tick` (its shoot game event once, `baseTick`).
//! - A block hit (`onHitBlock`): back 0.05 along its motion's signs, still, in the ground, its hit
//!   sound's pitch; in the ground it despawns at life 1200, or falls when its block goes
//!   (`shouldFall`, `startFalling`).
//! - An entity hit (`onHitEntity`): `ceil(|motion| x base damage)` as `arrow` damage from its
//!   owner (the knockback along the arrow's motion, the owner's last hurt mob), its hit sound,
//!   gone; a victim that takes no damage turns it back (`ProjectileDeflection.REVERSE`, x 0.2).
//!
//! Refused (model errors): an arrow reaching a player, a boat, a minecart, an armor stand or primed
//! TNT; a portal; a block that reacts to projectiles (`onProjectileHit`). Named (legacy): an
//! enderman's teleport (it takes no damage: the arrow turns back), a tipped arrow's effect.

use crate::block::Block;
use crate::entity::mob::ai::control;
use crate::entity::mob::los;
use crate::entity::{Aabb, EntityKind, Vec3, collide};
use crate::world::World;

/// `EntityTypes.ARROW.sized(0.5F, 0.5F).eyeHeight(0.13F)`.
pub const WIDTH: f32 = 0.5;
pub const HEIGHT: f32 = 0.5;
pub const EYE_HEIGHT: f32 = 0.13;

/// An arrow's own state (`AbstractArrow`, `Projectile`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ArrowState {
    pub owner: Option<u32>,
    pub left_owner: bool,
    pub left_owner_checked: bool,
    pub has_been_shot: bool,
    pub in_ground: bool,
    pub in_ground_time: i32,
    pub life: i32,
    pub shake_time: i32,
    pub base_damage: f64,
    /// `lastState`: the block it hit.
    pub last_state: Option<Block>,
    pub fire: i32,
    pub last_deflected_by: Option<u32>,
    /// A tipped arrow's effect (a stray's slowness, a bogged's poison).
    pub effect: Option<&'static str>,
    /// (mob-AI 2026-11-10) The bow's enchantments it was fired from (`firedFromWeapon`): power's
    /// damage, punch's knockback (flame set it on fire at its spawn); None: no weapon (a dispenser's).
    pub weapon_ench: Option<Vec<(&'static str, i32)>>,
    /// This tick's fluid effects (`FlowingFluid.entityInside`): lava's LAVA_IGNITE, water's
    /// EXTINGUISH ([`with_block_effects`]).
    pub inside_lava: bool,
    pub inside_water: bool,
    /// (mob-AI 2026-11-10) A thrown trident (`ThrownTrident`, a drowned's: no loyalty): 0.99
    /// inertia in water, one entity hit at most (none once it dealt damage), 8 damage, and it
    /// bounces back off what it hit.
    pub trident: bool,
    /// `ThrownTrident.dealtDamage`: it hit an entity, or lay in the ground over 4 ticks.
    pub dealt_damage: bool,
    /// (mob-AI 2026-11-11) A llama's spit (`LlamaSpit`: 0.25 x 0.25, [`tick_spit`]).
    pub spit: bool,
}

/// `EntityTypes.LLAMA_SPIT.sized(0.25F, 0.25F)` (eye height 0.85 of it).
pub const SPIT_WIDTH: f32 = 0.25;
pub const SPIT_HEIGHT: f32 = 0.25;
pub const SPIT_EYE_HEIGHT: f32 = 0.25 * 0.85;

/// `FluidState.entityInside` on an arrow (its sweep met the fluid's box): collected for
/// [`with_block_effects`].
pub fn fluid_inside(w: &mut World, i: usize, lava: bool) {
    let s = st_mut(w, i);
    if lava {
        s.inside_lava = true;
    } else {
        s.inside_water = true;
    }
}

/// `Entity.applyEffectsFromBlocks(movements)` on an arrow around `f` (the sweep): the collected
/// effects in `InsideBlockEffectType` order (LAVA_IGNITE: `igniteForSeconds(15)`, 300 ticks at
/// least, then `lavaHurt`, which a projectile does not take; EXTINGUISH; one step's worth: an arrow
/// meeting lava and water in different steps of one tick is not told apart), the rain puts the
/// fire out, a fire gone out plays GENERIC_EXTINGUISH_FIRE (two floats of its own random), a fire
/// not lit this tick resets to 0 (`-getFireImmuneTicks()`).
fn with_block_effects(w: &mut World, i: usize, f: impl FnOnce(&mut World)) {
    let fire_before = st(w, i).fire;
    {
        let s = st_mut(w, i);
        s.inside_lava = false;
        s.inside_water = false;
    }
    f(w);
    if w.entities.list[i].removed || !matches!(w.entities.list[i].kind, EntityKind::Arrow(_)) {
        return;
    }
    let (lava, water) = (st(w, i).inside_lava, st(w, i).inside_water);
    if lava && st(w, i).fire < 300 {
        st_mut(w, i).fire = 300;
    }
    if water {
        st_mut(w, i).fire = 0;
    }
    if st(w, i).fire > 0 && crate::weather::entity_in_rain(w, i) {
        st_mut(w, i).fire = 0;
    }
    let fire = st(w, i).fire;
    if fire_before > 0
        && fire <= 0
        && let Some(r) = w.entities.list[i].random.as_mut()
    {
        r.next_float();
        r.next_float();
    }
    if fire <= 0 && fire <= fire_before {
        st_mut(w, i).fire = 0;
    }
}

fn st(w: &World, i: usize) -> &ArrowState {
    match &w.entities.list[i].kind {
        EntityKind::Arrow(s) => s,
        _ => unreachable!("an arrow"),
    }
}

fn st_mut(w: &mut World, i: usize) -> &mut ArrowState {
    match &mut w.entities.list[i].kind {
        EntityKind::Arrow(s) => s,
        _ => unreachable!("an arrow"),
    }
}

fn refuse(w: &mut World, why: &str) {
    let why = why.to_string();
    if !w.errors.contains(&why) {
        w.errors.push(why);
    }
}

/// `RandomSource.triangle(mode, deviation)`.
fn triangle(r: &mut crate::rng::LegacyRandom, mode: f64, deviation: f64) -> f64 {
    mode + deviation * (r.next_double() - r.next_double())
}

/// `Mth.ceil(double)`.
fn jceil(v: f64) -> i32 {
    let t = v as i32;
    if v > t as f64 { t + 1 } else { t }
}

/// `Projectile.lerpRotation(rotO, rot)`.
fn lerp_rotation(mut rot_o: f32, rot: f32) -> f32 {
    while rot - rot_o < -180.0 {
        rot_o -= 360.0;
    }
    while rot - rot_o >= 180.0 {
        rot_o += 360.0;
    }
    rot_o + 0.2f32 * (rot - rot_o)
}

/// `AbstractSkeleton.performRangedAttack(target, power)` from mob `i` at entity `t` (the arrow's
/// part, then the shooter's sound): returns the arrow's id. `bow`: the bow's enchantments (the
/// arrow's constructor: flame's `projectile_spawned` ignites it, igniteForSeconds(100)).
pub fn shoot_from_mob(w: &mut World, i: usize, t: usize, power: f32, effect: Option<&'static str>, bow: &[(&'static str, i32)]) -> u32 {
    let fire = if bow.iter().any(|&(e, _)| e == "flame") { 2000 } else { 0 };
    let state = ArrowState { effect, weapon_ench: Some(bow.to_vec()), fire, ..Default::default() };
    launch(w, i, t, state, Some(power))
}

/// `Drowned.performRangedAttack(target, power)`: `new ThrownTrident(level, mob, its trident)` at the
/// thrower's x, eye y less 0.1F and z (its constructor's UUID), `spawnProjectileUsingShoot(xd,
/// yd + the horizontal distance x 0.2F, zd, 1.6F, 14 - difficulty x 4)` (the shot's three
/// triangles, then added), then the thrower's DROWNED_SHOOT pitch from its own random.
pub fn throw_trident_from_mob(w: &mut World, i: usize, t: usize) -> u32 {
    let state = ArrowState { trident: true, base_damage: 2.0, ..Default::default() };
    launch(w, i, t, state, None)
}

/// A mob's shot at mob `t`: the projectile at its x, eye y less 0.1F and z, owned by it; an
/// arrow's `setBaseDamageFromMob(power)` (`mob_power`); `shoot` toward the target's third of its
/// height; added; the shooter's sound pitch.
fn launch(w: &mut World, i: usize, t: usize, mut state: ArrowState, mob_power: Option<f32>) -> u32 {
    let shooter_pos = w.entities.list[i].pos;
    let eye_y = crate::entity::mob::ai::eye_pos(w, i).y;
    let start = Vec3::new(shooter_pos.x, eye_y - 0.1f32 as f64, shooter_pos.z);
    let at = w.entities.next_id;
    let mut own = w.entering_random(at);
    crate::rng::replay_uuid(&mut own);
    let difficulty = crate::entity::mob::ai::difficulty_id(w);
    // setBaseDamageFromMob(power)
    if let Some(power) = mob_power {
        state.base_damage = (power * 2.0f32) as f64 + triangle(&mut own, difficulty as f64 * 0.11, 0.57425);
    }
    // xd, target.getY(1/3) - arrow y, zd; shoot(xd, yd + distance x 0.2F, zd, 1.6F, uncertainty)
    let tp = w.entities.list[t].pos;
    let th = w.entities.list[t].kind.dims().1;
    let xd = tp.x - shooter_pos.x;
    let yd = tp.y + th as f64 * 0.3333333333333333 - start.y;
    let zd = tp.z - shooter_pos.z;
    let dist = (xd * xd + zd * zd).sqrt();
    let uncertainty = (14 - difficulty * 4) as f32;
    let n = Vec3::new(xd, yd + dist * 0.2f32 as f64, zd).normalize();
    let ex = triangle(&mut own, 0.0, 0.0172275 * uncertainty as f64);
    let ey = triangle(&mut own, 0.0, 0.0172275 * uncertainty as f64);
    let ez = triangle(&mut own, 0.0, 0.0172275 * uncertainty as f64);
    let m = n.add(ex, ey, ez).scale(1.6f32 as f64);
    let y_rot = control::atan2_degrees(m.x, m.z);
    let x_rot = control::atan2_degrees(m.y, m.horizontal_distance());
    let owner = w.entities.list[i].id;
    state.owner = Some(owner);
    let id = w.entities.add(EntityKind::Arrow(state), start, y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.delta_movement = m;
        e.x_rot = x_rot;
        e.y_rot_o = y_rot;
        e.x_rot_o = x_rot;
        e.random = Some(own);
    }
    // applyOnProjectileSpawned: an unenchanted bow and arrow do nothing; the shooter's sound
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
    }
    id
}

/// `Llama.spit(target)` from llama `i` at mob `t` (`performRangedAttack`): `new LlamaSpit(level,
/// llama)` (its constructor's UUID; owned by it, at (width + 1) / 2 in front of its body's yaw, its
/// eye y less 0.1F), `spawnProjectileUsingShoot(xd, yd + the horizontal distance x 0.2F, zd,
/// 1.5F, 10)` (the shot's three triangles, then added), the LLAMA_SPIT sound's pitch (two of the
/// llama's own floats). Returns the spit's id.
pub fn spit_from_llama(w: &mut World, i: usize, t: usize) -> u32 {
    let lp = w.entities.list[i].pos;
    let width = w.entities.list[i].kind.dims().0;
    let body = crate::entity::mob::ai::ai(w, i).y_body_rot;
    let eye_y = crate::entity::mob::ai::eye_pos(w, i).y;
    let r = (width + 1.0f32) as f64 * 0.5;
    let a = body * (std::f64::consts::PI / 180.0) as f32;
    let start = Vec3::new(lp.x - r * crate::entity::mth::sin(a as f64) as f64, eye_y - 0.1f32 as f64, lp.z + r * crate::entity::mth::cos(a as f64) as f64);
    let at = w.entities.next_id;
    let mut own = w.entering_random(at);
    crate::rng::replay_uuid(&mut own);
    let tp = w.entities.list[t].pos;
    let th = w.entities.list[t].kind.dims().1;
    let xd = tp.x - lp.x;
    let yd = tp.y + th as f64 * 0.3333333333333333 - start.y;
    let zd = tp.z - lp.z;
    let dist = (xd * xd + zd * zd).sqrt() * 0.2f32 as f64;
    let n = Vec3::new(xd, yd + dist, zd).normalize();
    let u = 0.0172275 * 10.0f32 as f64;
    let ex = triangle(&mut own, 0.0, u);
    let ey = triangle(&mut own, 0.0, u);
    let ez = triangle(&mut own, 0.0, u);
    let m = n.add(ex, ey, ez).scale(1.5f32 as f64);
    let y_rot = control::atan2_degrees(m.x, m.z);
    let x_rot = control::atan2_degrees(m.y, m.horizontal_distance());
    let state = ArrowState { spit: true, owner: Some(w.entities.list[i].id), ..Default::default() };
    let id = w.entities.add(EntityKind::Arrow(state), start, y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.delta_movement = m;
        e.x_rot = x_rot;
        e.y_rot_o = y_rot;
        e.x_rot_o = x_rot;
        e.random = Some(own);
    }
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
        r.next_float();
    }
    id
}

/// `LlamaSpit.tick()` of spit `i`: `Projectile.tick` (the shoot game event once, checkLeftOwner,
/// `baseTick`); `ProjectileUtil.getHitResultOnMoveVector` (its motion clipped at the first block's
/// collider, then the nearest entity it can hit whose box grown by the margin the segment
/// crosses) and its hit; its rotations eased toward its motion; gone when no cell its box touches
/// is air, or in water; else 0.99 drag, gravity 0.06 and the move by last tick's motion (no
/// collision).
fn tick_spit(w: &mut World, i: usize) {
    if !st(w, i).has_been_shot {
        let p = w.entities.list[i].pos;
        w.game_event_at("projectile_shoot", [p.x, p.y, p.z], None, false);
        st_mut(w, i).has_been_shot = true;
    }
    check_left_owner(w, i);
    base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    st_mut(w, i).left_owner_checked = false;
    let movement = w.entities.list[i].delta_movement;
    let from = w.entities.list[i].pos;
    let to = from.add_v(movement);
    let bh = los::clip_collider(w, from, to);
    let end = if bh.miss { to } else { bh.location };
    let entity_hit = find_hit_entity_spit(w, i, from, end);
    if let Some((j, at)) = entity_hit {
        hit_entity_spit(w, i, j, at);
    } else if !bh.miss {
        let state = w.get(bh.pos);
        if !state.is_air() {
            if reacts_to_projectiles(&state) {
                refuse(w, "a llama's spit hitting a block that reacts to projectiles is not modeled");
            }
            w.entities.list[i].removed = true;
            let at = [bh.pos.x as f64 + 0.5, bh.pos.y as f64 + 0.5, bh.pos.z as f64 + 0.5];
            w.game_event_at("projectile_land", at, Some(state), false);
        }
    }
    let (x, y, z) = (from.x + movement.x, from.y + movement.y, from.z + movement.z);
    {
        let e = &mut w.entities.list[i];
        let m = e.delta_movement;
        let y_rot = control::atan2_degrees(m.x, m.z);
        let x_rot = control::atan2_degrees(m.y, m.horizontal_distance());
        e.x_rot = lerp_rotation(e.x_rot, x_rot);
        e.y_rot = lerp_rotation(e.y_rot, y_rot);
    }
    // findBlocksIn(box): the cells from the box's min corner's to its max corner's; none air
    let bb = w.entities.list[i].bb;
    let (x0, y0, z0) = (bb.min_x.floor() as i32, bb.min_y.floor() as i32, bb.min_z.floor() as i32);
    let (x1, y1, z1) = (bb.max_x.floor() as i32, bb.max_y.floor() as i32, bb.max_z.floor() as i32);
    let mut any_air = false;
    'cells: for cx in x0..=x1 {
        for cy in y0..=y1 {
            for cz in z0..=z1 {
                if w.get(crate::pos::BlockPos::new(cx, cy, cz)).is_air() {
                    any_air = true;
                    break 'cells;
                }
            }
        }
    }
    if !any_air || w.entities.list[i].was_touching_water {
        w.entities.list[i].removed = true;
        return;
    }
    let e = &mut w.entities.list[i];
    e.delta_movement = movement.scale(0.99f32 as f64).add(0.0, -0.06, 0.0);
    w.entities.set_pos(i, Vec3::new(x, y, z));
}

/// `ProjectileUtil.getEntityHitResult` for a spit: over its box swept by its motion and grown by
/// 1 (the level's section order), each entity it can hit whose box grown by the margin the
/// segment crosses, the nearest clip point from `from` (the first of a tie).
fn find_hit_entity_spit(w: &mut World, i: usize, from: Vec3, to: Vec3) -> Option<(usize, Vec3)> {
    let e = &w.entities.list[i];
    let d = e.delta_movement;
    let area = e.bb.expand_towards(d.x, d.y, d.z).inflate(1.0, 1.0, 1.0);
    let m = margin(e.tick_count) as f64;
    let id = e.id;
    let candidates = w.get_entities(area, |x| x.id != id);
    let mut best: Option<(usize, Vec3, f64)> = None;
    let trace = std::env::var("MCTAS_TRACE_SPIT").is_ok();
    if trace {
        let e = &w.entities.list[i];
        eprintln!("spit t{} #{}: tick_count {} margin {m} from {:?} to {:?} candidates {:?}", w.time, e.id, e.tick_count, (from.x, from.y, from.z), (to.x, to.y, to.z), candidates.iter().map(|&j| w.entities.list[j].id).collect::<Vec<_>>());
    }
    for j in candidates {
        if !can_hit(w, i, j) {
            continue;
        }
        let hit = collide::clip(w.entities.list[j].bb.inflate(m, m, m), from, to);
        if trace {
            let b = w.entities.list[j].bb;
            eprintln!("spit t{}:   #{} bb ({}, {}, {})..({}, {}, {}) clip {:?}", w.time, w.entities.list[j].id, b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z, hit.map(|p| (p.x, p.y, p.z)));
        }
        if let Some(p) = hit {
            let dd = from.distance_to_sqr(p);
            if best.as_ref().is_none_or(|b| dd < b.2) {
                best = Some((j, p, dd));
            }
        }
    }
    refuse_players_in_the_way(w, from, to, m);
    best.map(|(j, p, _)| (j, p))
}

/// `LlamaSpit.onHitEntity` (no entity here deflects it), then the land game event: 1 `spit`
/// damage from its owner (the knockback along the spit's motion); it flies on.
fn hit_entity_spit(w: &mut World, i: usize, j: usize, at: Vec3) {
    if !matches!(w.entities.list[j].kind, EntityKind::Mob(_)) {
        refuse(w, "a llama's spit hitting a boat, a minecart, an armor stand or primed TNT is not modeled");
        return;
    }
    let owner = st(w, i).owner.filter(|&o| crate::entity::index_of(w, o).is_some_and(|k| !w.entities.list[k].removed && matches!(w.entities.list[k].kind, EntityKind::Mob(_))));
    if owner.is_some() {
        let motion = w.entities.list[i].delta_movement;
        let prev = w.mob.ai.attacker;
        w.mob.ai.attacker = owner;
        w.mob.ai.projectile_motion = Some(motion);
        let hurt = crate::entity::mob::living::hurt(w, j, crate::entity::mob::Damage::Spit, 1.0);
        w.mob.ai.projectile_motion = None;
        w.mob.ai.attacker = prev;
        // EnchantmentHelper.doPostAttackEffects: thorns armor's effect on the llama
        if hurt
            && let EntityKind::Mob(m) = &w.entities.list[j].kind
            && m.armor.has(m.helmet.is_some(), "thorns")
        {
            refuse(w, "a llama's spit on thorns armor (its post-attack effect) is not modeled");
        }
    }
    w.game_event_at("projectile_land", [at.x, at.y, at.z], None, false);
}

/// `AbstractArrow.tick()` of arrow `i`.
pub fn tick(w: &mut World, i: usize) {
    if st(w, i).spit {
        tick_spit(w, i);
        return;
    }
    // ThrownTrident.tick before AbstractArrow's: over 4 ticks in the ground it has dealt damage
    // (no loyalty: nothing returns it)
    if st(w, i).trident && st(w, i).in_ground_time > 4 {
        st_mut(w, i).dealt_damage = true;
    }
    let physics = !w.entities.list[i].no_physics;
    let movement = w.entities.list[i].delta_movement;
    let bp = w.entities.list[i].block_position();
    let state = w.get(bp);
    if !state.is_air() && physics {
        let pos = w.entities.list[i].pos;
        if collide::boxes_at(w, &state, bp).iter().any(|b| collide::aabb_contains(*b, pos)) {
            w.entities.list[i].delta_movement = Vec3::ZERO;
            st_mut(w, i).in_ground = true;
        }
    }
    {
        let s = st_mut(w, i);
        if s.shake_time > 0 {
            s.shake_time -= 1;
        }
    }
    // isInWaterOrRain: clearFire
    if st(w, i).fire > 0 && (w.entities.list[i].was_touching_water || w.is_raining_at(bp)) {
        st_mut(w, i).fire = 0;
    }
    if st(w, i).in_ground && physics {
        if st(w, i).last_state != Some(state) && should_fall(w, i) {
            start_falling(w, i);
        } else {
            let s = st_mut(w, i);
            s.life += 1;
            if s.life >= 1200 {
                w.entities.list[i].removed = true;
            }
        }
        st_mut(w, i).in_ground_time += 1;
        if !w.entities.list[i].removed {
            with_block_effects(w, i, |w| collide::apply_effects_from_blocks(w, i));
        }
        return;
    }
    st_mut(w, i).in_ground_time = 0;
    let original = w.entities.list[i].pos;
    if w.entities.list[i].was_touching_water {
        // getWaterInertia: an arrow's 0.6, a trident's 0.99
        let inertia = if st(w, i).trident { 0.99f32 } else { 0.6f32 };
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.scale(inertia as f64);
    }
    let y_rot = control::atan2_degrees(movement.x, movement.z);
    let x_rot = control::atan2_degrees(movement.y, movement.horizontal_distance());
    {
        let e = &mut w.entities.list[i];
        e.x_rot = lerp_rotation(e.x_rot, x_rot);
        e.y_rot = lerp_rotation(e.y_rot, y_rot);
    }
    check_left_owner(w, i);
    if physics {
        let hit = los::clip_collider(w, original, original.add_v(movement));
        step_move_and_hit(w, i, hit);
    }
    if w.entities.list[i].removed {
        return;
    }
    if !w.entities.list[i].was_touching_water {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.scale(0.99f32 as f64);
    }
    if physics && !st(w, i).in_ground {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, -0.05, 0.0);
    }
    // Projectile.tick: the shoot game event the first time, checkLeftOwner, Entity.tick
    if !st(w, i).has_been_shot {
        let p = w.entities.list[i].pos;
        w.game_event_at("projectile_shoot", [p.x, p.y, p.z], None, false);
        st_mut(w, i).has_been_shot = true;
    }
    check_left_owner(w, i);
    base_tick(w, i);
    if !w.entities.list[i].removed {
        st_mut(w, i).left_owner_checked = false;
    }
}

/// `Entity.baseTick` for an arrow: its portal (refused), the shared base tick, its fire (an
/// `onFire` hurt every 20 ticks out of lava does nothing to a projectile; a tick less).
fn base_tick(w: &mut World, i: usize) {
    if w.entities.list[i].portal.is_some() {
        refuse(w, "an arrow in a portal is not modeled");
    }
    crate::entity::base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    let s = st_mut(w, i);
    if s.fire > 0 {
        s.fire -= 1;
    }
}

/// `Projectile.checkLeftOwner()`.
fn check_left_owner(w: &mut World, i: usize) {
    let s = st(w, i);
    if s.left_owner || s.left_owner_checked {
        return;
    }
    let left = outside_owner_collision_range(w, i);
    let s = st_mut(w, i);
    s.left_owner = left;
    s.left_owner_checked = true;
}

/// The root vehicle of entity `j` (`getRootVehicle`), by id.
fn root_vehicle(w: &World, j: usize) -> u32 {
    let mut cur = j;
    for _ in 0..16 {
        let Some(v) = w.entities.list[cur].vehicle else { break };
        match crate::entity::index_of(w, v) {
            Some(k) => cur = k,
            None => break,
        }
    }
    w.entities.list[cur].id
}

/// `isPickable()` of entity `j`: a living entity, a boat, a minecart, an armor stand or primed
/// TNT not removed.
fn pickable(w: &World, j: usize) -> bool {
    let e = &w.entities.list[j];
    !e.removed && !matches!(e.kind, EntityKind::Item(_) | EntityKind::Arrow(_))
}

/// `canBeHitByProjectile()`: alive (a living one with health) and pickable.
fn hittable(w: &World, j: usize) -> bool {
    pickable(w, j) && !matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.health <= 0.0)
}

/// `isOutsideOwnerCollisionRange()`: none of the owner's root vehicle and its passengers (the
/// pickable ones) meets the arrow's box swept by its motion and grown by 1.
fn outside_owner_collision_range(w: &World, i: usize) -> bool {
    let Some(owner) = st(w, i).owner else { return true };
    let Some(o) = crate::entity::index_of(w, owner).filter(|&o| !w.entities.list[o].removed) else { return true };
    let e = &w.entities.list[i];
    let d = e.delta_movement;
    let area = e.bb.expand_towards(d.x, d.y, d.z).inflate(1.0, 1.0, 1.0);
    let root = root_vehicle(w, o);
    !w.entities.list.iter().enumerate().any(|(k, x)| !x.removed && root_vehicle(w, k) == root && pickable(w, k) && area.intersects(x.bb))
}

/// `canHitEntity(entity)`: alive and pickable, and not of the owner's vehicle (itself included)
/// until the arrow has left it.
fn can_hit(w: &World, i: usize, j: usize) -> bool {
    if j == i || !hittable(w, j) {
        return false;
    }
    let s = st(w, i);
    match s.owner.and_then(|o| crate::entity::index_of(w, o)).filter(|&o| !w.entities.list[o].removed) {
        None => true,
        Some(o) => s.left_owner || root_vehicle(w, o) != root_vehicle(w, j),
    }
}

/// `ProjectileUtil.computeMargin(arrow)`.
fn margin(tick_count: i32) -> f32 {
    0.0f32.max(0.3f32.min((tick_count - 2) as f32 / 20.0f32))
}

/// `findHitEntities(from, to)`: `getManyEntityHitResult` over the arrow's box swept by its motion
/// and grown by 1 (the level's section order): the entities its segment crosses (their box, else
/// their box grown by the margin when a straight line from that point to their centre, cut at
/// the first block, reaches their box), with the hit points.
fn find_hit_entities(w: &mut World, i: usize, from: Vec3, to: Vec3) -> Vec<(usize, Vec3)> {
    let e = &w.entities.list[i];
    let d = e.delta_movement;
    let area = e.bb.expand_towards(d.x, d.y, d.z).inflate(1.0, 1.0, 1.0);
    let m = margin(e.tick_count) as f64;
    let id = e.id;
    let candidates = w.get_entities(area, |x| x.id != id);
    let mut out = Vec::new();
    for j in candidates {
        if !can_hit(w, i, j) {
            continue;
        }
        let bb = w.entities.list[j].bb;
        if let Some(p) = collide::clip(bb, from, to) {
            out.push((j, p));
        } else if m > 0.0
            && let Some(outside) = collide::clip(bb.inflate(m, m, m), from, to)
        {
            let mut toward = bb.center();
            let bh = los::clip_collider(w, outside, toward);
            if !bh.miss {
                toward = bh.location;
            }
            if collide::clip(bb, outside, toward).is_some() {
                out.push((j, outside));
            }
        }
    }
    refuse_players_in_the_way(w, from, to, m);
    out
}

/// A player in the way (the AFK player, a scene's): not modeled.
fn refuse_players_in_the_way(w: &mut World, from: Vec3, to: Vec3, m: f64) {
    let seg = Aabb::new(from.x.min(to.x), from.y.min(to.y), from.z.min(to.z), from.x.max(to.x), from.y.max(to.y), from.z.max(to.z)).inflate(m + 0.3, m + 0.9, m + 0.3);
    let players: Vec<Vec3> = w.mob.ai.players.iter().filter(|p| p.mode != crate::entity::mob::ai::GameMode::Spectator).map(|p| p.pos).collect();
    for p in players {
        let pb = crate::entity::make_bounding_box(p, crate::entity::mob::ai::target::PLAYER_WIDTH, crate::entity::mob::ai::target::PLAYER_HEIGHT);
        if pb.intersects(seg) && (collide::clip(pb.inflate(m, m, m), from, to).is_some() || collide::aabb_contains(pb.inflate(m, m, m), from)) {
            refuse(w, "an arrow reaching a player is not modeled");
        }
    }
}

/// `ThrownTrident.findHitEntities(from, to)`: `findHitEntity` alone, none once it dealt damage
/// (`ProjectileUtil.getEntityHitResult`: over the same swept box, each entity's box grown by the
/// margin clipped by the segment, the nearest clip point from `from`, the first of a tie).
fn find_hit_entity_trident(w: &mut World, i: usize, from: Vec3, to: Vec3) -> Vec<(usize, Vec3)> {
    if st(w, i).dealt_damage {
        return Vec::new();
    }
    let e = &w.entities.list[i];
    let d = e.delta_movement;
    let area = e.bb.expand_towards(d.x, d.y, d.z).inflate(1.0, 1.0, 1.0);
    let m = margin(e.tick_count) as f64;
    let id = e.id;
    let candidates = w.get_entities(area, |x| x.id != id);
    let mut best: Option<(usize, Vec3, f64)> = None;
    for j in candidates {
        if !can_hit(w, i, j) {
            continue;
        }
        if let Some(p) = collide::clip(w.entities.list[j].bb.inflate(m, m, m), from, to) {
            let dd = from.distance_to_sqr(p);
            if best.as_ref().is_none_or(|b| dd < b.2) {
                best = Some((j, p, dd));
            }
        }
    }
    refuse_players_in_the_way(w, from, to, m);
    best.map(|(j, p, _)| vec![(j, p)]).unwrap_or_default()
}

/// `stepMoveAndHit(blockHitResult)`.
fn step_move_and_hit(w: &mut World, i: usize, block_hit: los::BlockHit) {
    loop {
        if w.entities.list[i].removed {
            return;
        }
        let initial = w.entities.list[i].pos;
        let trident = st(w, i).trident;
        let mut hits = if trident { find_hit_entity_trident(w, i, initial, block_hit.location) } else { find_hit_entities(w, i, initial, block_hit.location) };
        // nearest first by the entities' positions (a stable sort)
        hits.sort_by(|a, b| {
            let da = initial.distance_to_sqr(w.entities.list[a.0].pos);
            let db = initial.distance_to_sqr(w.entities.list[b.0].pos);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let next = hits.first().map(|h| h.1).unwrap_or(block_hit.location);
        w.entities.set_pos(i, next);
        with_block_effects(w, i, |w| collide::apply_effects_from_blocks_between(w, i, initial, next));
        if w.entities.list[i].portal.is_some() {
            refuse(w, "an arrow in a portal is not modeled");
        }
        if hits.is_empty() {
            if !w.entities.list[i].removed && !block_hit.miss {
                hit_block(w, i, block_hit);
            }
            return;
        }
        if !w.entities.list[i].removed && !w.entities.list[i].no_physics {
            for (j, at) in hits {
                if trident {
                    hit_entity_trident(w, i, j, at);
                } else {
                    hit_entity(w, i, j, at);
                }
                if w.entities.list[i].removed {
                    break;
                }
            }
        }
        // no piercing (a bow's arrows): one round
        return;
    }
}

/// `hitTargetOrDeflectSelf` for a block, then `onHit` -> `AbstractArrow.onHitBlock`.
fn hit_block(w: &mut World, i: usize, hit: los::BlockHit) {
    let state = w.get(hit.pos);
    if state.is_air() {
        return;
    }
    st_mut(w, i).last_state = Some(state);
    // Projectile.onHitBlock: `state.onProjectileHit` (only some blocks react)
    if reacts_to_projectiles(&state) {
        refuse(w, "an arrow hitting a block that reacts to projectiles (target, bell, TNT, chorus flower, pointed dripstone, decorated pot, candle, campfire) is not modeled");
    }
    let m = w.entities.list[i].delta_movement;
    let sign = |v: f64| if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { v };
    let offset = Vec3::new(sign(m.x), sign(m.y), sign(m.z)).scale(0.05f32 as f64);
    let p = w.entities.list[i].pos.subtract_v(offset);
    w.entities.set_pos(i, p);
    w.entities.list[i].delta_movement = Vec3::ZERO;
    // the hit sound's pitch
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
    }
    let s = st_mut(w, i);
    s.in_ground = true;
    s.shake_time = 7;
    let at = [hit.pos.x as f64 + 0.5, hit.pos.y as f64 + 0.5, hit.pos.z as f64 + 0.5];
    w.game_event_at("projectile_land", at, Some(state), false);
}

/// The blocks with an `onProjectileHit` (26.3): targets, bells, TNT, chorus flowers, pointed
/// dripstone, decorated pots, candles and campfires.
fn reacts_to_projectiles(s: &Block) -> bool {
    let Block::Terrain(id) = s else {
        return matches!(s, Block::Tnt { .. });
    };
    let name = &crate::terrain::blocks::props(*id).name;
    let n = name.trim_start_matches("minecraft:");
    let base = n.split('[').next().unwrap_or(n);
    matches!(base, "target" | "bell" | "tnt" | "chorus_flower" | "pointed_dripstone" | "decorated_pot" | "campfire" | "soul_campfire") || base.ends_with("candle")
}

/// `hitTargetOrDeflectSelf` for an entity (no entity here deflects) -> `onHit` ->
/// `AbstractArrow.onHitEntity`, then the land game event.
fn hit_entity(w: &mut World, i: usize, j: usize, at: Vec3) {
    let vic = w.entities.list[j].id;
    let is_mob = matches!(w.entities.list[j].kind, EntityKind::Mob(_));
    if !is_mob {
        refuse(w, "an arrow hitting a boat, a minecart, an armor stand or primed TNT is not modeled");
        return;
    }
    let motion = w.entities.list[i].delta_movement;
    let pow = motion.length() as f32;
    // (mob-AI 2026-11-10) a fired-from weapon's EnchantmentHelper.modifyDamage on the base damage
    // as a float (power's 1 + 0.5 a level), then ceil(pow x it)
    let arrow_damage = match st(w, i).weapon_ench.clone() {
        Some(ench) => {
            let victim = crate::entity::mob::mob(w, j).ty.def().id;
            crate::entity::mob::ai::attack::enchant_damage(&ench, victim, true, st(w, i).base_damage as f32) as f64
        }
        None => st(w, i).base_damage,
    };
    let damage = jceil((pow as f64 * arrow_damage).clamp(0.0, 2.147483647e9));
    let owner = st(w, i).owner.filter(|&o| crate::entity::index_of(w, o).is_some_and(|k| !w.entities.list[k].removed));
    // the owner's setLastHurtMob
    if let Some(o) = owner
        && let Some(k) = crate::entity::index_of(w, o)
        && crate::entity::mob::ai::has_ai(w, k)
    {
        crate::entity::mob::ai::ai_mut(w, k).last_hurt_mob = Some(vic);
    }
    let fire_before = match &w.entities.list[j].kind {
        EntityKind::Mob(m) => m.fire_ticks,
        _ => 0,
    };
    // a burning arrow: `igniteForSeconds(5)` (100 ticks at least, x BURNING_TIME) before the hurt
    let t100 = if matches!(w.entities.list[j].kind, EntityKind::Mob(_)) { crate::entity::mob::ignite_ticks(w, j, 100) } else { 100 };
    if st(w, i).fire > 0
        && let EntityKind::Mob(m) = &mut w.entities.list[j].kind
        && m.fire_ticks < t100
    {
        m.fire_ticks = t100;
    }
    let enderman = matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.ty.def().id == "enderman");
    let hurt = if enderman && w.entities.list[j].vehicle.is_none() {
        // Enderman.hurtServer: a projectile (not riding) makes it teleport (up to 64 tries), and
        // it takes nothing
        crate::entity::mob::ai::goal::enderman::repeatedly_try_to_teleport(w, j);
        false
    } else {
        let prev = w.mob.ai.attacker;
        w.mob.ai.attacker = owner;
        w.mob.ai.projectile_motion = Some(motion);
        let r = crate::entity::mob::living::hurt(w, j, crate::entity::mob::Damage::Arrow, damage as f32);
        w.mob.ai.projectile_motion = None;
        w.mob.ai.attacker = prev;
        r
    };
    if hurt {
        // (mob-AI 2026-11-10) doKnockback: punch's knockback along the arrow's sideways motion
        // (x 0.6 x the knockback resistance's complement), `push(x, 0.1, z)`
        let punch = st(w, i).weapon_ench.as_ref().map_or(0, |e| e.iter().find(|&&(id, _)| id == "punch").map_or(0, |&(_, l)| l));
        if punch > 0 && matches!(w.entities.list[j].kind, EntityKind::Mob(_)) {
            let res = (1.0 - crate::entity::mob::ai::attack::knockback_resistance(w, j)).max(0.0);
            let flat = Vec3::new(motion.x, 0.0, motion.z);
            let len = flat.length();
            if len >= 1.0e-5f32 as f64 {
                let s = (1.0f32 + 1.0f32 * (punch - 1) as f32) as f64 * 0.6 * res;
                let (mx, mz) = (flat.x / len * s, flat.z / len * s);
                if mx * mx + mz * mz > 0.0 {
                    let e = &mut w.entities.list[j];
                    e.delta_movement = e.delta_movement.add(mx, 0.1, mz);
                }
            }
        }
        if let Some(e) = st(w, i).effect {
            w.mob.ai.legacy.insert(e);
        }
        // (mob-AI 2026-11-10) doPostHurtEnemy: thorns armor's post-attack effect on the shooter
        if let EntityKind::Mob(m) = &w.entities.list[j].kind
            && m.armor.has(m.helmet.is_some(), "thorns")
        {
            let why = "an arrow on thorns armor (its post-attack effect) is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
        // the hit sound's pitch, then gone
        if let Some(r) = w.entities.list[i].random.as_mut() {
            r.next_float();
        }
        w.entities.list[i].removed = true;
    } else if !enderman {
        // projectileReceivesSideEffectsOnHit(false) (an enderman's is false: the arrow flies on)
        if let EntityKind::Mob(m) = &mut w.entities.list[j].kind {
            m.fire_ticks = fire_before;
        }
        // deflect(REVERSE, victim, owner, false, 0.2): x -0.1, the yaw turned 170..190
        let rot = 170.0f32 + w.entities.list[i].random.as_mut().map_or(0.0, |r| r.next_float()) * 20.0f32;
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.multiply(-0.2 * 0.5, -0.2 * 0.5, -0.2 * 0.5);
        e.y_rot += rot;
        e.y_rot_o += rot;
        if w.entities.list[i].delta_movement.length_sqr() < 1.0e-7 {
            w.entities.list[i].removed = true;
        }
    }
    w.game_event_at("projectile_land", [at.x, at.y, at.z], None, false);
}

/// `ThrownTrident.onHitEntity` (a plain trident: no enchantment's damage or effects), then the
/// land game event: 8 `trident` damage from its owner (the knockback along its motion); it has
/// dealt damage either way; a victim that takes the side effects (any but an enderman not hurt)
/// hears TRIDENT_HIT (a fixed pitch) and turns it back (`deflect(REVERSE, (0.02, 0.2, 0.02))`:
/// x -0.01, -0.1, -0.01, the yaw turned 170..190 by its own random).
fn hit_entity_trident(w: &mut World, i: usize, j: usize, at: Vec3) {
    if !matches!(w.entities.list[j].kind, EntityKind::Mob(_)) {
        refuse(w, "a trident hitting a boat, a minecart, an armor stand or primed TNT is not modeled");
        return;
    }
    let owner = st(w, i).owner.filter(|&o| crate::entity::index_of(w, o).is_some_and(|k| !w.entities.list[k].removed));
    st_mut(w, i).dealt_damage = true;
    let enderman = matches!(&w.entities.list[j].kind, EntityKind::Mob(m) if m.ty.def().id == "enderman");
    let hurt = if enderman && w.entities.list[j].vehicle.is_none() {
        crate::entity::mob::ai::goal::enderman::repeatedly_try_to_teleport(w, j);
        false
    } else {
        let motion = w.entities.list[i].delta_movement;
        let prev = w.mob.ai.attacker;
        w.mob.ai.attacker = owner;
        w.mob.ai.projectile_motion = Some(motion);
        let r = crate::entity::mob::living::hurt(w, j, crate::entity::mob::Damage::Trident, 8.0);
        w.mob.ai.projectile_motion = None;
        w.mob.ai.attacker = prev;
        r
    };
    if !enderman || hurt {
        let rot = 170.0f32 + w.entities.list[i].random.as_mut().map_or(0.0, |r| r.next_float()) * 20.0f32;
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.multiply(-0.02 * 0.5, -0.2 * 0.5, -0.02 * 0.5);
        e.y_rot += rot;
        e.y_rot_o += rot;
    }
    w.game_event_at("projectile_land", [at.x, at.y, at.z], None, false);
}

/// `shouldFall()`: in the ground with no collision in a box 0.06 around its position.
fn should_fall(w: &World, i: usize) -> bool {
    let p = w.entities.list[i].pos;
    let bb = Aabb::new(p.x, p.y, p.z, p.x, p.y, p.z).inflate(0.06, 0.06, 0.06);
    st(w, i).in_ground && !collide::collides_with_blocks(w, bb) && w.get_entities(bb.inflate(1.0e-7, 1.0e-7, 1.0e-7), |e| matches!(e.kind, EntityKind::Boat(_))).is_empty()
}

/// `startFalling()`: out of the ground, its motion scaled by three `nextFloat() * 0.2F`, life 0.
fn start_falling(w: &mut World, i: usize) {
    st_mut(w, i).in_ground = false;
    let (a, b, c) = match w.entities.list[i].random.as_mut() {
        Some(r) => (r.next_float() * 0.2f32, r.next_float() * 0.2f32, r.next_float() * 0.2f32),
        None => (0.0, 0.0, 0.0),
    };
    let e = &mut w.entities.list[i];
    e.delta_movement = e.delta_movement.multiply(a as f64, b as f64, c as f64);
    st_mut(w, i).life = 0;
}

/// The arrow's fire for the frames.
pub fn fire(s: &ArrowState) -> i32 {
    s.fire
}

/// Whether the arrow is in the ground (`isPushedByFluid` is its opposite).
pub fn in_ground(s: &ArrowState) -> bool {
    s.in_ground
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotations_ease_a_fifth_of_the_way() {
        assert_eq!(lerp_rotation(0.0, 10.0), 2.0);
        // across the wrap: 350 toward 10 goes up by 4
        assert_eq!(lerp_rotation(350.0, 10.0), -6.0);
    }

    #[test]
    fn the_margin_grows_to_0_3() {
        assert_eq!(margin(1), 0.0);
        assert_eq!(margin(2), 0.0);
        assert_eq!(margin(4), 0.1);
        assert_eq!(margin(100), 0.3);
    }
}
