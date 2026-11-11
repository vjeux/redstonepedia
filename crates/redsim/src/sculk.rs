//! Sculk sensors (`SculkSensorBlock`, `CalibratedSculkSensorBlock`, their block entities) and the
//! vibration system they listen with (`GameEventDispatcher`, `VibrationSystem.Listener` /
//! `Ticker` / `User`, `VibrationSelector`), read in the 26.3 source. OWNED BY AGENT SCULK.
//!
//! * A game event (`World::game_event*`) is posted at its block's center; an entity's
//!   (`World::game_event_at`, clk-misc 2026-10-21: HIT_GROUND when it lands, ENTITY_DAMAGE when a
//!   cactus hurts an item) at the entity's position, whose block stands for the range, the
//!   occlusion lines and `canReceiveVibration`, while the vibration's distance (travel time,
//!   power) is the exact one to the sensor's center; an item of #dampens_vibrations (wool) is never
//!   heard (`Entity.dampensVibrations`), and an entity standing on a sensor force-schedules a STEP
//!   vibration (`SculkSensorBlock.stepOn`, `step_on`: no range, occlusion or in-flight check).
//!   Every sensor whose
//!   block is within its listener radius (8, calibrated 16; `BlockPos.distSqr <= r*r`) handles
//!   it at once (`handleGameEvent`): ignored while a vibration is in flight; the event must be in
//!   the `vibrations` tag, its affected state must not dampen (`dampens_vibrations`: wool); the
//!   sensor must accept it (`canReceiveVibration`: a calibrated sensor with a non-zero back signal
//!   only takes that frequency; not its own block's destroy/place; frequency > 0; INACTIVE); and
//!   it must not be occluded (all 6 lines from the source center nudged 1e-5 toward each face
//!   cross a wool block, `isBlockInLine` voxel walk). Then it is a CANDIDATE of the sensor's
//!   selector, stamped with the game time: one per sensor, a later event in the same tick
//!   replaces it only if closer (same distance: higher frequency); an event of a later tick never
//!   replaces it.
//! * Block-entity phase (`VibrationSystem.Ticker.tick`, each sensor in ticker order): with no
//!   vibration in flight, a candidate from an EARLIER tick becomes the vibration (travel time
//!   floor(distance)) and the selector is cleared; then the travel time drops by one, and at 0 the
//!   vibration is received: an INACTIVE sensor activates with power
//!   `max(1, 15 - floor(15/r * distance))` (ACTIVE 30 gt, calibrated 10, then COOLDOWN 10 gt as
//!   scheduled ticks) and resonates adjacent amethyst blocks (`resonate_<frequency>` events).
//!   Every data change (`onDataChanged` -> `setChanged`) updates comparators around the sensor.
//! * Signals: weak power = `power` on every side (a calibrated sensor: not into its back), strong
//!   power only into the block below; comparators read the last frequency while ACTIVE.
//!
//! The bridge's idle check ignores vibrations in flight, and so does `World::idle`.

use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::World;

/// `SculkSensorBlock.ACTIVE_TICKS` (the calibrated sensor's `getActiveTicks` is 10).
pub const ACTIVE_TICKS: i32 = 30;
pub const CALIBRATED_ACTIVE_TICKS: i32 = 10;
/// `SculkSensorBlock.COOLDOWN_TICKS`.
pub const COOLDOWN_TICKS: i32 = 10;

pub const INACTIVE: u8 = 0;
pub const ACTIVE: u8 = 1;
pub const COOLDOWN: u8 = 2;

/// A vibration (`VibrationInfo`): the event, its distance (float), where it came from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Vib {
    pub event: &'static str,
    pub freq: u8,
    pub dist: f32,
    pub src: BlockPos,
}

/// `SculkSensorBlockEntity` + its `VibrationSystem.Data`.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct SculkBe {
    /// `lastVibrationFrequency` (what comparators read while ACTIVE).
    pub last_freq: u8,
    /// `currentVibration` (in flight).
    pub current: Option<Vib>,
    /// `travelTimeInTicks`.
    pub travel: i32,
    /// The selector's candidate with its game time (`VibrationSelector.currentVibrationData`).
    pub cand: Option<(Vib, i64)>,
}

#[derive(Clone, Default, Debug)]
pub struct State {
    /// Live block entities by position.
    pub bes: Vec<(BlockPos, SculkBe)>,
    /// Ticker list in tick order (position, live) and the pending ones.
    tickers: Vec<(BlockPos, bool)>,
    pending: Vec<(BlockPos, bool)>,
    ticking: bool,
}

impl State {
    /// (tickexact) A chunk reload (`crate::reload`): the sensors' tickers in reloaded cells are
    /// removed with their chunk and come back as new ones in `order`, after the others (their
    /// vibration data is saved and loaded whole).
    pub fn reload_tickers(&mut self, reloaded: &dyn Fn(BlockPos) -> bool, order: &[BlockPos]) {
        let live: std::collections::HashSet<BlockPos> = self.tickers.iter().chain(self.pending.iter()).filter(|(p, l)| *l && reloaded(*p)).map(|(p, _)| *p).collect();
        for t in self.tickers.iter_mut().chain(self.pending.iter_mut()) {
            if reloaded(t.0) {
                t.1 = false;
            }
        }
        for p in order.iter().filter(|p| live.contains(p)) {
            if self.ticking { self.pending.push((*p, true)) } else { self.tickers.push((*p, true)) }
        }
    }

    /// Sensor block entities (by position) with the candidate's game time relative to `now`, and
    /// the ticker order, for [`crate::world::World::state_key`].
    pub(crate) fn key_into<H: std::hash::Hasher>(&self, now: i64, h: &mut H) {
        use std::hash::Hash;
        let vib = |v: &Vib, h: &mut H| (v.event, v.freq, v.dist.to_bits(), v.src).hash(h);
        let mut bes: Vec<&(BlockPos, SculkBe)> = self.bes.iter().collect();
        bes.sort_by_key(|(p, _)| *p);
        for (p, b) in bes {
            (p, b.last_freq, b.travel).hash(h);
            match &b.current {
                Some(v) => {
                    1u8.hash(h);
                    vib(v, h);
                }
                None => 0u8.hash(h),
            }
            match &b.cand {
                Some((v, t)) => {
                    1u8.hash(h);
                    vib(v, h);
                    (now - t).hash(h);
                }
                None => 0u8.hash(h),
            }
        }
        0xfeu8.hash(h);
        let order: Vec<BlockPos> = self.tickers.iter().chain(self.pending.iter()).filter(|(_, live)| *live).map(|(p, _)| *p).collect();
        order.hash(h);
    }

    pub fn is_empty(&self) -> bool {
        self.bes.is_empty()
    }
    pub fn be(&self, p: BlockPos) -> Option<&SculkBe> {
        self.bes.iter().find(|(q, _)| *q == p).map(|(_, b)| b)
    }
    fn be_mut(&mut self, p: BlockPos) -> Option<&mut SculkBe> {
        self.bes.iter_mut().find(|(q, _)| *q == p).map(|(_, b)| b)
    }
    /// Any vibration in flight or waiting in a selector (not part of the bridge's idle check;
    /// for tools that want to know a sensor is about to fire).
    pub fn busy(&self) -> bool {
        self.bes.iter().any(|(_, b)| b.current.is_some() || b.cand.is_some())
    }
}

/// `VibrationSystem.VIBRATION_FREQUENCY_FOR_EVENT` (0: not a vibration frequency).
pub fn frequency(event: &str) -> u8 {
    match event {
        "step" | "swim" | "flap" => 1,
        "projectile_land" | "hit_ground" | "splash" | "bounce" => 2,
        "item_interact_finish" | "projectile_shoot" | "instrument_play" => 3,
        "entity_action" | "elytra_glide" | "unequip" => 4,
        "entity_dismount" | "equip" => 5,
        "entity_interact" | "shear" | "entity_mount" => 6,
        "entity_damage" => 7,
        "drink" | "eat" => 8,
        "container_close" | "block_close" | "block_deactivate" | "block_detach" => 9,
        "container_open" | "block_open" | "block_activate" | "block_attach" | "prime_fuse" | "note_block_play" => 10,
        "block_change" => 11,
        "block_destroy" | "fluid_pickup" => 12,
        "block_place" | "fluid_place" => 13,
        "entity_place" | "lightning_strike" | "teleport" => 14,
        "entity_die" | "explode" => 15,
        _ => match event.strip_prefix("resonate_").and_then(|n| n.parse::<u8>().ok()) {
            Some(n) if (1..=15).contains(&n) => n,
            _ => 0,
        },
    }
}

/// The `vibrations` game event tag (every event with a frequency is in it; the sensor's own
/// `sculk_sensor_tendrils_clicking` is not).
pub fn is_vibration(event: &str) -> bool {
    frequency(event) > 0
}

/// `getResonanceEventByFrequency`.
pub fn resonance_event(freq: u8) -> &'static str {
    const R: [&str; 15] = [
        "resonate_1", "resonate_2", "resonate_3", "resonate_4", "resonate_5", "resonate_6", "resonate_7", "resonate_8", "resonate_9", "resonate_10",
        "resonate_11", "resonate_12", "resonate_13", "resonate_14", "resonate_15",
    ];
    R[(freq.clamp(1, 15) - 1) as usize]
}

/// `#minecraft:wool` (all 16 colours).
pub fn is_wool(s: &Block) -> bool {
    use Simple::*;
    matches!(
        s,
        Block::Simple(
            WhiteWool | OrangeWool | MagentaWool | LightBlueWool | YellowWool | LimeWool | PinkWool | GrayWool | LightGrayWool | CyanWool | PurpleWool | BlueWool | BrownWool | GreenWool | RedWool | BlackWool
        )
    )
}

/// `occludes_vibration_signals` = `#wool`.
pub fn occludes(s: &Block) -> bool {
    is_wool(s)
}

/// `dampens_vibrations` = wool, wool carpets, slabs, stairs (only wool is modeled).
pub fn dampens(s: &Block) -> bool {
    is_wool(s)
}

/// `vibration_resonators` = amethyst block.
pub fn is_resonator(s: &Block) -> bool {
    matches!(s, Block::Simple(Simple::AmethystBlock))
}

/// The listener radius (`getListenerRadius`).
pub fn radius(calibrated: bool) -> i32 {
    if calibrated { 16 } else { 8 }
}

fn dist_sqr(a: BlockPos, b: BlockPos) -> i64 {
    let (dx, dy, dz) = ((a.x - b.x) as i64, (a.y - b.y) as i64, (a.z - b.z) as i64);
    dx * dx + dy * dy + dz * dz
}

/// `(float) Math.sqrt(origin.distSqr(dest))` (the same value as `(float) Vec3.distanceTo` of
/// the two centers).
pub fn distance(a: BlockPos, b: BlockPos) -> f32 {
    (dist_sqr(a, b) as f64).sqrt() as f32
}

/// `getRedstoneStrengthForDistance`.
pub fn strength(dist: f32, radius: i32) -> u8 {
    let scale = 15.0f64 / radius as f64;
    (15 - (scale * dist as f64).floor() as i32).max(1) as u8
}

// ---------------------------------------------------------------------- occlusion

fn lerp(a: f64, p0: f64, p1: f64) -> f64 {
    p0 + a * (p1 - p0)
}

fn floor_i(v: f64) -> i32 {
    v.floor() as i32
}

fn frac(v: f64) -> f64 {
    v - (v.floor() as i64) as f64
}

fn sign(v: f64) -> i32 {
    if v == 0.0 {
        0
    } else if v > 0.0 {
        1
    } else {
        -1
    }
}

/// `BlockGetter.isBlockInLine` with the occlusion predicate (`traverseBlocks`).
fn wool_in_line(w: &World, from: [f64; 3], to: [f64; 3]) -> bool {
    if from == to {
        return false;
    }
    let tx_ = lerp(-1.0e-7, to[0], from[0]);
    let ty_ = lerp(-1.0e-7, to[1], from[1]);
    let tz_ = lerp(-1.0e-7, to[2], from[2]);
    let fx = lerp(-1.0e-7, from[0], to[0]);
    let fy = lerp(-1.0e-7, from[1], to[1]);
    let fz = lerp(-1.0e-7, from[2], to[2]);
    let (mut cx, mut cy, mut cz) = (floor_i(fx), floor_i(fy), floor_i(fz));
    if occludes(&w.get(BlockPos::new(cx, cy, cz))) {
        return true;
    }
    let (dx, dy, dz) = (tx_ - fx, ty_ - fy, tz_ - fz);
    let (sx, sy, sz) = (sign(dx), sign(dy), sign(dz));
    let tdx = if sx == 0 { f64::MAX } else { sx as f64 / dx };
    let tdy = if sy == 0 { f64::MAX } else { sy as f64 / dy };
    let tdz = if sz == 0 { f64::MAX } else { sz as f64 / dz };
    let mut tx = tdx * if sx > 0 { 1.0 - frac(fx) } else { frac(fx) };
    let mut ty = tdy * if sy > 0 { 1.0 - frac(fy) } else { frac(fy) };
    let mut tz = tdz * if sz > 0 { 1.0 - frac(fz) } else { frac(fz) };
    while tx <= 1.0 || ty <= 1.0 || tz <= 1.0 {
        if tx < ty {
            if tx < tz {
                cx += sx;
                tx += tdx;
            } else {
                cz += sz;
                tz += tdz;
            }
        } else if ty < tz {
            cy += sy;
            ty += tdy;
        } else {
            cz += sz;
            tz += tdz;
        }
        if occludes(&w.get(BlockPos::new(cx, cy, cz))) {
            return true;
        }
    }
    false
}

/// `VibrationSystem.Listener.isOccluded`: every one of the 6 nudged lines is blocked.
pub fn is_occluded(w: &World, src: BlockPos, dst: BlockPos) -> bool {
    let from = [src.x as f64 + 0.5, src.y as f64 + 0.5, src.z as f64 + 0.5];
    let to = [dst.x as f64 + 0.5, dst.y as f64 + 0.5, dst.z as f64 + 0.5];
    let eps = 1.0e-5f32 as f64;
    for d in Dir::ALL {
        let s = d.step();
        let nudged = [from[0] + eps * s[0] as f64, from[1] + eps * s[1] as f64, from[2] + eps * s[2] as f64];
        if !wool_in_line(w, nudged, to) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------- block entity bookkeeping

/// The block entity appears (`LevelChunk.setBlockState` creating it): its listener registers and
/// its ticker joins the list.
pub fn create(w: &mut World, pos: BlockPos) {
    if w.sculk.be(pos).is_some() {
        return;
    }
    w.sculk.bes.push((pos, SculkBe::default()));
    let st = &mut w.sculk;
    if st.ticking { st.pending.push((pos, true)) } else { st.tickers.push((pos, true)) }
}

/// The block entity goes (`LevelChunk.removeBlockEntity`).
pub fn remove(w: &mut World, pos: BlockPos) {
    let st = &mut w.sculk;
    if let Some(i) = st.bes.iter().position(|(q, _)| *q == pos) {
        st.bes.remove(i);
        for t in st.tickers.iter_mut().chain(st.pending.iter_mut()) {
            if t.0 == pos {
                t.1 = false;
            }
        }
    }
}

// ---------------------------------------------------------------------- game events

/// `GameEventDispatcher.post` to every sensor (the only listeners modeled), with the event's
/// affected state (`GameEvent.Context.affectedState`; no source entity ever).
pub fn dispatch(w: &mut World, event: &'static str, pos: BlockPos, affected: Option<Block>) {
    let at = [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5];
    dispatch_inner(w, event, pos, at, affected);
}

/// `GameEventDispatcher.post` of an event at an exact position (an entity's: `HIT_GROUND`,
/// `ENTITY_DAMAGE`; clk-misc 2026-10-21), with its affected state and whether its source entity
/// dampens vibrations (`Entity.dampensVibrations`: an item of #dampens_vibrations). The sensors in
/// range, the occlusion lines and `canReceiveVibration` read the event's block
/// (`BlockPos.containing`); the vibration's distance (its travel time and power) is the exact one,
/// from the position to the sensor's center.
pub fn dispatch_at(w: &mut World, event: &'static str, at: [f64; 3], affected: Option<Block>, source_dampens: bool) {
    // isValidVibration: a source entity that dampens vibrations is never heard
    if source_dampens {
        return;
    }
    let pos = BlockPos::new(at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
    dispatch_inner(w, event, pos, at, affected);
}

/// `(float) Vec3.distanceTo` from `at` to the center of `sensor`.
fn distance_to_center(at: [f64; 3], sensor: BlockPos) -> f32 {
    let (dx, dy, dz) = (at[0] - (sensor.x as f64 + 0.5), at[1] - (sensor.y as f64 + 0.5), at[2] - (sensor.z as f64 + 0.5));
    (dx * dx + dy * dy + dz * dz).sqrt() as f32
}

/// `VibrationSelector.addCandidate` of sensor `i`.
fn add_candidate(w: &mut World, i: usize, v: Vib) {
    let time = w.time;
    let be = &mut w.sculk.bes[i].1;
    let replace = match be.cand {
        None => true,
        Some((prev, t)) => {
            if t != time {
                false
            } else if v.dist < prev.dist {
                true
            } else if v.dist > prev.dist {
                false
            } else {
                v.freq > prev.freq
            }
        }
    };
    if replace {
        be.cand = Some((v, time));
    }
}

/// `SculkSensorBlock.stepOn` (clk-misc 2026-10-21): an entity standing on an INACTIVE sensor
/// force-schedules a STEP vibration from its position (`Listener.forceScheduleVibration`: no
/// range, occlusion or in-flight check; `canReceiveVibration` with the sensor's own state).
pub fn step_on(w: &mut World, sensor: BlockPos, at: [f64; 3]) {
    let Block::Sculk { calibrated, facing, phase, .. } = w.get(sensor) else { return };
    if phase != INACTIVE {
        return;
    }
    let Some(i) = w.sculk.bes.iter().position(|(q, _)| *q == sensor) else { return };
    let freq = frequency("step");
    if calibrated {
        let back = facing.opposite();
        let filter = w.signal(sensor.rel(back), back);
        if filter != 0 && freq as i32 != filter {
            return;
        }
    }
    let src = BlockPos::new(at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32);
    let v = Vib { event: "step", freq, dist: distance_to_center(at, sensor), src };
    add_candidate(w, i, v);
}

fn dispatch_inner(w: &mut World, event: &'static str, pos: BlockPos, at: [f64; 3], affected: Option<Block>) {
    if !is_vibration(event) {
        return;
    }
    if affected.is_some_and(|a| dampens(&a)) {
        return;
    }
    let freq = frequency(event);
    for i in 0..w.sculk.bes.len() {
        let (sp, be) = w.sculk.bes[i];
        let Block::Sculk { calibrated, facing, phase, .. } = w.get(sp) else { continue };
        let r = radius(calibrated) as i64;
        if dist_sqr(sp, pos) > r * r {
            continue;
        }
        if be.current.is_some() {
            continue;
        }
        // canReceiveVibration
        if calibrated {
            let back = facing.opposite();
            let filter = w.signal(sp.rel(back), back);
            if filter != 0 && freq as i32 != filter {
                continue;
            }
        }
        if sp == pos && (event == "block_destroy" || event == "block_place") {
            continue;
        }
        if phase != INACTIVE {
            continue;
        }
        if is_occluded(w, pos, sp) {
            continue;
        }
        let v = Vib { event, freq, dist: distance_to_center(at, sp), src: pos };
        add_candidate(w, i, v);
    }
}

// ---------------------------------------------------------------------- the block

fn with(s: Block, phase: u8, power: u8) -> Block {
    match s {
        Block::Sculk { calibrated, facing, .. } => Block::Sculk { calibrated, facing, phase, power },
        _ => s,
    }
}

/// `SculkSensorBlock.updateNeighbours`: the sensor's neighbors, then the neighbors of the block
/// below it.
fn update_neighbours(w: &mut World, pos: BlockPos, kind: Kind) {
    w.update_neighbors_at(pos, kind);
    w.update_neighbors_at(pos.below(), kind);
}

/// `SculkSensorBlock.activate` (no source entity).
fn activate(w: &mut World, pos: BlockPos, s: Block, power: u8, freq: u8) {
    let Block::Sculk { calibrated, .. } = s else { return };
    let kind = s.kind();
    w.set_block_and_update(pos, with(s, ACTIVE, power));
    w.schedule_tick(pos, kind, if calibrated { CALIBRATED_ACTIVE_TICKS } else { ACTIVE_TICKS }, Priority::Normal);
    update_neighbours(w, pos, kind);
    // tryResonateVibration: Direction.values() order
    for d in Dir::ALL {
        let q = pos.rel(d);
        let qs = w.get(q);
        if is_resonator(&qs) {
            w.game_event_state(resonance_event(freq), q, qs);
        }
    }
    // the sensor's own `sculk_sensor_tendrils_clicking` is no vibration (frequency 0)
    w.game_event("sculk_sensor_tendrils_clicking", pos);
    // playSound(SCULK_CLICKING, pitch level.getRandom().nextFloat() * 0.2F + 0.8F) (never
    // waterlogged here)
    w.draw_sound_pitch();
}

/// The scheduled tick (`SculkSensorBlock.tick`): ACTIVE -> COOLDOWN (`deactivate`), COOLDOWN ->
/// INACTIVE.
pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
    let Block::Sculk { phase, .. } = s else { return };
    let kind = s.kind();
    if phase == ACTIVE {
        w.set_block_and_update(pos, with(s, COOLDOWN, 0));
        w.schedule_tick(pos, kind, COOLDOWN_TICKS, Priority::Normal);
        update_neighbours(w, pos, kind);
    } else if phase == COOLDOWN {
        let Block::Sculk { power, .. } = s else { return };
        w.set_block_and_update(pos, with(s, INACTIVE, power));
        // playSound(SCULK_CLICKING_STOP, a level-random pitch)
        w.draw_sound_pitch();
    }
}

/// `SculkSensorBlock.onPlace`: a new sensor placed with power and no pending tick loses it.
pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block) {
    if let Block::Sculk { phase, power, .. } = s {
        if old.kind() != s.kind() && power > 0 && !w.has_scheduled_tick(pos, s.kind()) {
            w.set_block(pos, with(s, phase, 0), crate::world::flags::CLIENTS | crate::world::flags::KNOWN_SHAPE);
        }
    }
}

/// `SculkSensorBlock.affectNeighborsAfterRemoval`.
pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos) {
    if let Block::Sculk { phase: ACTIVE, .. } = s {
        update_neighbours(w, pos, s.kind());
    }
}

/// `getSignal(state, level, pos, direction)`: `power` on every side; a calibrated sensor gives
/// nothing when asked with `direction == facing`, i.e. to the block behind it (its frequency
/// input at `pos + facing.opposite()`).
pub fn get_signal(s: &Block, d: Dir) -> i32 {
    match *s {
        Block::Sculk { calibrated, facing, power, .. } => {
            if calibrated && d == facing {
                0
            } else {
                power as i32
            }
        }
        _ => 0,
    }
}

/// `getDirectSignal`: only into the block below.
pub fn get_direct_signal(s: &Block, d: Dir) -> i32 {
    if d == Dir::Up { get_signal(s, Dir::Up) } else { 0 }
}

/// `getAnalogOutputSignal`: the last vibration's frequency while ACTIVE.
pub fn analog_output(w: &World, s: &Block, p: BlockPos) -> i32 {
    match *s {
        Block::Sculk { phase: ACTIVE, .. } => w.sculk.be(p).map(|b| b.last_freq as i32).unwrap_or(0),
        _ => 0,
    }
}

// ---------------------------------------------------------------------- block-entity phase

/// `onDataChanged` -> `BlockEntity.setChanged`: comparators around the sensor re-check.
fn data_changed(w: &mut World, pos: BlockPos) {
    let s = w.get(pos);
    if !s.is_air() {
        w.update_neighbour_for_output_signal(pos, s.kind());
    }
}

/// `VibrationSystem.Ticker.tick` for every sensor, in ticker order.
pub fn tick_all(w: &mut World) {
    if w.sculk.tickers.is_empty() && w.sculk.pending.is_empty() {
        return;
    }
    w.sculk.ticking = true;
    let pending = std::mem::take(&mut w.sculk.pending);
    w.sculk.tickers.extend(pending);
    let mut i = 0;
    while i < w.sculk.tickers.len() {
        let (p, live) = w.sculk.tickers[i];
        if !live {
            w.sculk.tickers.remove(i);
            continue;
        }
        // (tickexact 2026-11-10, a partial unload: no block entity ticks where blocks do not)
        if !w.block_ticks_at(p) {
            i += 1;
            continue;
        }
        tick_one(w, p);
        i += 1;
    }
    w.sculk.ticking = false;
}

fn tick_one(w: &mut World, p: BlockPos) {
    let time = w.time;
    let Some(be) = w.sculk.be_mut(p) else { return };
    if be.current.is_none() {
        if let Some((v, t)) = be.cand {
            if t < time {
                be.current = Some(v);
                be.travel = (v.dist as f64).floor() as i32;
                be.cand = None;
                data_changed(w, p);
            }
        }
    }
    let Some(be) = w.sculk.be_mut(p) else { return };
    let Some(v) = be.current else { return };
    let mut changed = be.travel > 0;
    be.travel = (be.travel - 1).max(0);
    if be.travel <= 0 {
        receive(w, p, v);
        if let Some(be) = w.sculk.be_mut(p) {
            be.current = None;
        }
        changed = true;
    }
    if changed {
        data_changed(w, p);
    }
}

/// `receiveVibration` -> `onReceiveVibration`: an INACTIVE sensor activates.
fn receive(w: &mut World, p: BlockPos, v: Vib) {
    let s = w.get(p);
    let Block::Sculk { calibrated, phase: INACTIVE, .. } = s else { return };
    if let Some(be) = w.sculk.be_mut(p) {
        be.last_freq = v.freq;
    }
    let power = strength(distance(v.src, p), radius(calibrated));
    activate(w, p, s, power, v.freq);
}
