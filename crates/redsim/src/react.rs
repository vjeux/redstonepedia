//! Entity-reacting blocks (track clk-misc: the fluids + entities port's M3/M4), read in the 26.3
//! source: pressure plates (`BasePressurePlateBlock`, `PressurePlateBlock`,
//! `WeightedPressurePlateBlock`), tripwire (`TripWireBlock`) and tripwire hooks
//! (`TripWireHookBlock`).
//!
//! Entities press plates and strings through `entity_inside` (called by the entity core's
//! `check_inside_blocks` when an entity's box meets the block's entity-inside shape,
//! [`entity_inside_shape`]); the scheduled re-checks count the entities in the block's box with
//! [`entities_in`]. Until the entity core lands nothing is ever inside, so the blocks only do
//! what they do without entities (survive or pop, connect strings, attach hooks).

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// An axis-aligned box in block units `[x0, y0, z0, x1, y1, z1]` (Java's `AABB`, doubles).
pub type Bounds = [f64; 6];

fn moved(b: Bounds, p: BlockPos) -> Bounds {
    let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
    [b[0] + x, b[1] + y, b[2] + z, b[3] + x, b[4] + y, b[5] + z]
}

/// `pos.relative(d, n)`.
fn rel_n(p: BlockPos, d: Dir, n: i32) -> BlockPos {
    let s = d.step();
    p.offset(s[0] * n, s[1] * n, s[2] * n)
}

/// `Block.column(size, minY, maxY)` in block units.
fn column(size: f64, y0: f64, y1: f64) -> Bounds {
    let lo = (16.0 - size) / 2.0 / 16.0;
    let hi = (16.0 + size) / 2.0 / 16.0;
    [lo, y0 / 16.0, lo, hi, y1 / 16.0, hi]
}

/// Which entities a box query counts (`getEntitiesOfClass` / `getEntities` with
/// `!isIgnoringBlockTriggers`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Filter {
    /// Every entity (`Entity.class`).
    Any,
    /// `LivingEntity.class` (stone pressure plates).
    Living,
}

/// How many entities' boxes meet `b` (strictly, `AABB.intersects`): `World::get_entities`.
pub fn entities_in(w: &World, b: Bounds, f: Filter) -> usize {
    let bb = crate::entity::Aabb::new(b[0], b[1], b[2], b[3], b[4], b[5]);
    w.get_entities(bb, |e| match f {
        Filter::Any => true,
        Filter::Living => matches!(e.kind, crate::entity::EntityKind::ArmorStand(_) | crate::entity::EntityKind::Mob(_)),
    })
    .len()
}

/// `FluidState.entityInside` for a fluid an entity's sweep met (`Entity.checkInsideBlocks`'s
/// `collidedWithFluid`: the box at the step's start swept to its end meets the fluid's box):
/// a mob's LAVA_IGNITE or EXTINGUISH.
pub fn fluid_inside(w: &mut World, i: usize, fs: crate::fluid::FluidState, p: BlockPos) {
    let _ = p;
    match w.entities.list[i].kind {
        crate::entity::EntityKind::Mob(_) => crate::entity::mob::living::fluid_inside(w, i, fs.fluid.is_lava()),
        // a drop: lava burns it, water puts its fire out (tickexact 2026-11-08)
        crate::entity::EntityKind::Item(_) => crate::entity::item::fluid_inside(w, i, fs.fluid.is_lava()),
        _ => {}
    }
}

/// `BlockBehaviour.entityInside` for the blocks an entity's sweep meets (called by the entity
/// core's `checkInsideBlocks`; `precise`: the entity's box at its target meets the cell).
pub fn entity_inside(w: &mut World, i: usize, s: Block, p: BlockPos, precise: bool) {
    // a fire's cell on a mob (`BaseFireBlock.entityInside`)
    if matches!(w.entities.list[i].kind, crate::entity::EntityKind::Mob(_)) {
        crate::entity::mob::living::entity_inside(w, i, s, p);
    }
    // a fire's cell on a drop: FIRE_IGNITE and 1 `inFire`
    if matches!(w.entities.list[i].kind, crate::entity::EntityKind::Item(_)) && matches!(s, Block::Fire { .. }) {
        crate::entity::item::fire_inside(w, i);
    }
    match s {
        Block::Plate { .. } => plate::entity_inside(w, s, p),
        Block::Tripwire { .. } => tripwire::entity_inside(w, s, p),
        Block::DetectorRail { .. } => crate::rails::detector::entity_inside(w, s, p),
        Block::BigDripleaf { .. } => dripleaf::entity_inside(w, i, s, p),
        // ---- cactus farms (clk-misc): 1 damage a tick ----
        Block::Cactus { .. } => crate::cactus::entity_inside(w, i),
        // ---- open-top hoppers (clk-misc): an item in the suck box runs the hopper ----
        Block::Hopper { .. } => crate::redstone::container::hopper_entity_inside(w, i, p),
        // BubbleColumnBlock.entityInside: above the column's top (nothing with a collision shape
        // or a fluid above) the entity is launched, inside it is pushed
        Block::BubbleColumn { drag } => {
            if precise {
                let above = w.get(p.above());
                let nothing_above = crate::behavior::collision_boxes(&above).is_empty() && crate::fluid::fluid_state(w, p.above()).is_empty();
                if nothing_above {
                    on_above_bubble_column(w, i, drag);
                } else {
                    on_inside_bubble_column(w, i, drag);
                }
            }
        }
        // a natural cobweb (`WebBlock.entityInside`: makeStuckInBlock, the fall distance reset;
        // the next move is scaled by (0.25, 0.05, 0.25) and the motion zeroed); a sweet berry
        // bush's and powder snow's stuck (with their damage and freezing) are refused
        Block::Terrain(id) => {
            let name = crate::terrain::blocks::props(id).block_id();
            match name {
                "minecraft:cobweb" => {
                    let e = &mut w.entities.list[i];
                    e.fall_distance = 0.0;
                    e.stuck_speed = crate::entity::Vec3::new(0.25, 0.05f32 as f64, 0.25);
                }
                "minecraft:sweet_berry_bush" | "minecraft:powder_snow" => {
                    let why = format!("an entity inside a natural {name} (its stuck speed, damage or freezing) is not modeled yet");
                    if !w.errors.contains(&why) {
                        w.errors.push(why);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// `Entity.onAboveBubbleColumn` (boats override it: `AbstractBoat.onAboveBubbleColumn`).
fn on_above_bubble_column(w: &mut World, i: usize, drag: bool) {
    if matches!(w.entities.list[i].kind, crate::entity::EntityKind::Boat(_)) {
        crate::entity::boat::on_above_bubble_column(w, i, drag);
        return;
    }
    // `handleOnAboveBubbleColumn`
    let e = &mut w.entities.list[i];
    let m = e.delta_movement;
    let yd = if drag { crate::entity::vec3::jmax(-0.9, m.y - 0.03) } else { crate::entity::vec3::jmin(1.8, m.y + 0.1) };
    e.delta_movement = crate::entity::Vec3::new(m.x, yd, m.z);
    // sendBubbleColumnParticles: 2 x (splash + bubble) at random spots in the cell, drawn from
    // the level random (8 nextDouble)
    if let Some(r) = w.rng() {
        for _ in 0..8 {
            r.next_double();
        }
    }
}

/// `Entity.onInsideBubbleColumn` (`handleOnInsideBubbleColumn`; the fall distance is reset).
fn on_inside_bubble_column(w: &mut World, i: usize, drag: bool) {
    let e = &mut w.entities.list[i];
    let m = e.delta_movement;
    let yd = if drag { crate::entity::vec3::jmax(-0.3, m.y - 0.03) } else { crate::entity::vec3::jmin(0.7, m.y + 0.06) };
    e.delta_movement = crate::entity::Vec3::new(m.x, yd, m.z);
    e.fall_distance = 0.0;
}

/// `getEntityInsideCollisionShape` where it is not the full cell: the box an entity's box must
/// meet for `entity_inside` to run (tripwire: its outline).
pub fn entity_inside_shape(s: &Block) -> Option<Bounds> {
    match *s {
        Block::Tripwire { .. } => Some(tripwire::shape(s)),
        _ => None,
    }
}

// ------------------------------------------------------------------------ pressure plates

pub mod plate {
    use super::*;

    pub const OAK: u8 = 0;
    pub const STONE: u8 = 1;
    pub const LIGHT: u8 = 2;
    pub const HEAVY: u8 = 3;
    pub const NAMES: [&str; 4] = ["oak_pressure_plate", "stone_pressure_plate", "light_weighted_pressure_plate", "heavy_weighted_pressure_plate"];

    /// `BasePressurePlateBlock.TOUCH_AABB`: `column(14, 0, 4)`.
    pub fn touch_aabb() -> Bounds {
        column(14.0, 0.0, 4.0)
    }

    /// `getPressedTime`.
    fn pressed_time(kind: u8) -> i32 {
        if kind >= LIGHT { 10 } else { 20 }
    }

    /// `getSignalForState`.
    pub fn signal(s: &Block) -> i32 {
        match *s {
            Block::Plate { power, .. } => power as i32,
            _ => 0,
        }
    }

    /// `setSignalForState`: wooden / stone plates store `powered` (15 when on).
    fn with_signal(kind: u8, signal: i32) -> Block {
        let power = if kind >= LIGHT { signal as u8 } else if signal > 0 { 15 } else { 0 };
        Block::Plate { kind, power }
    }

    /// `getSignalStrength`.
    fn signal_strength(w: &World, kind: u8, p: BlockPos) -> i32 {
        let b = moved(touch_aabb(), p);
        match kind {
            OAK => if entities_in(w, b, Filter::Any) > 0 { 15 } else { 0 },
            STONE => if entities_in(w, b, Filter::Living) > 0 { 15 } else { 0 },
            _ => {
                let max = if kind == LIGHT { 15 } else { 150 };
                let count = entities_in(w, b, Filter::Any).min(max);
                if count > 0 {
                    let percent = count.min(max) as f32 / max as f32;
                    (percent * 15.0f32).ceil() as i32
                } else {
                    0
                }
            }
        }
    }

    /// `canSurvive`: `canSupportRigidBlock(below) || canSupportCenter(below, UP)`.
    pub fn can_survive(w: &World, p: BlockPos) -> bool {
        let below = p.below();
        let b = w.get(below);
        bh::sturdy(w, &b, below, Dir::Up, Support::Rigid) || bh::sturdy(w, &b, below, Dir::Up, Support::Center)
    }

    /// `updateShape`: pops when the block below can no longer hold it.
    pub fn update_shape(w: &World, s: Block, p: BlockPos, d: Dir) -> Block {
        if d == Dir::Down && !can_survive(w, p) { Block::Air } else { s }
    }

    fn update_neighbours(w: &mut World, p: BlockPos, k: Kind) {
        w.update_neighbors_at(p, k);
        w.update_neighbors_at(p.below(), k);
    }

    /// `checkPressed`.
    fn check_pressed(w: &mut World, p: BlockPos, s: Block, old: i32) {
        let Block::Plate { kind, .. } = s else { return };
        let signal = signal_strength(w, kind, p);
        let (was, is) = (old > 0, signal > 0);
        if old != signal {
            w.set_block(p, with_signal(kind, signal), flags::CLIENTS);
            update_neighbours(w, p, s.kind());
        }
        if !is && was {
            w.game_event("block_deactivate", p);
        } else if is && !was {
            w.game_event("block_activate", p);
        }
        if is {
            w.schedule_tick(p, s.kind(), pressed_time(kind), Priority::Normal);
        }
    }

    /// `tick`.
    pub fn tick(w: &mut World, s: Block, p: BlockPos) {
        let sig = signal(&s);
        if sig > 0 {
            check_pressed(w, p, s, sig);
        }
    }

    /// `entityInside` (the entity is not ignoring block triggers: boats, minecarts, armor stands).
    pub fn entity_inside(w: &mut World, s: Block, p: BlockPos) {
        let sig = signal(&s);
        if sig == 0 {
            check_pressed(w, p, s, sig);
        }
    }

    /// `affectNeighborsAfterRemoval`.
    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
        if !moved && signal(&s) > 0 {
            update_neighbours(w, p, s.kind());
        }
    }

    /// `getDirectSignal`: the block below only.
    pub fn direct_signal(s: &Block, d: Dir) -> i32 {
        if d == Dir::Up { signal(s) } else { 0 }
    }
}

// ------------------------------------------------------------------------ tripwire

pub mod tripwire {
    use super::*;

    /// `getShape`: attached `column(16, 1, 2.5)`, else `column(16, 0, 8)`.
    pub fn shape(s: &Block) -> Bounds {
        match *s {
            Block::Tripwire { attached: true, .. } => column(16.0, 1.0, 2.5),
            _ => column(16.0, 0.0, 8.0),
        }
    }

    fn powered(s: &Block) -> bool {
        matches!(*s, Block::Tripwire { powered: true, .. })
    }

    fn with_powered(s: Block, on: bool) -> Block {
        match s {
            Block::Tripwire { attached, disarmed, north, east, south, west, .. } => Block::Tripwire { powered: on, attached, disarmed, north, east, south, west },
            x => x,
        }
    }

    /// `shouldConnectTo`.
    pub fn should_connect_to(ns: &Block, d: Dir) -> bool {
        match *ns {
            Block::TripwireHook { facing, .. } => facing == d.opposite(),
            Block::Tripwire { .. } => true,
            _ => false,
        }
    }

    /// `updateShape`: a horizontal neighbor sets that side's connection.
    pub fn update_shape(s: Block, d: Dir, ns: &Block) -> Block {
        let Block::Tripwire { powered, attached, disarmed, mut north, mut east, mut south, mut west } = s else { return s };
        let c = should_connect_to(ns, d);
        match d {
            Dir::North => north = c,
            Dir::East => east = c,
            Dir::South => south = c,
            Dir::West => west = c,
            _ => return s,
        }
        Block::Tripwire { powered, attached, disarmed, north, east, south, west }
    }

    /// `onPlace`.
    pub fn on_place(w: &mut World, s: Block, p: BlockPos, old: Block) {
        if old.kind() != Kind::Tripwire {
            update_source(w, p, s);
        }
    }

    /// `affectNeighborsAfterRemoval`.
    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
        if !moved {
            update_source(w, p, with_powered(s, true));
        }
    }

    /// `updateSource`: the hooks at either end (looking south and west only) recompute.
    fn update_source(w: &mut World, p: BlockPos, s: Block) {
        for d in [Dir::South, Dir::West] {
            for i in 1..42 {
                let tp = rel_n(p, d, i);
                let b = w.get(tp);
                if let Block::TripwireHook { facing, .. } = b {
                    if facing == d.opposite() {
                        hook::calculate_state(w, tp, b, false, true, i, Some(s));
                    }
                    break;
                }
                if b.kind() != Kind::Tripwire {
                    break;
                }
            }
        }
    }

    /// `entityInside` (the entity is not ignoring block triggers).
    pub fn entity_inside(w: &mut World, s: Block, p: BlockPos) {
        if !powered(&s) && !w.has_scheduled_tick(p, Kind::Tripwire) {
            check_pressed(w, p, true);
        }
    }

    /// `tick`.
    pub fn tick(w: &mut World, p: BlockPos) {
        let s = w.get(p);
        if powered(&s) {
            let any = entities_in(w, moved(shape(&s), p), Filter::Any) > 0;
            check_pressed(w, p, any);
        }
    }

    /// `checkPressed(level, pos, entities)`: `any` = some entity there does not ignore block
    /// triggers.
    fn check_pressed(w: &mut World, p: BlockPos, any: bool) {
        let mut s = w.get(p);
        let was = powered(&s);
        let should = any;
        if should != was {
            s = with_powered(s, should);
            w.set_block_and_update(p, s);
            update_source(w, p, s);
        }
        if should {
            w.schedule_tick(p, Kind::Tripwire, 10, Priority::Normal);
        } else if was {
            w.schedule_tick(p, Kind::Tripwire, 1, Priority::Normal);
        }
    }
}

// ------------------------------------------------------------------------ big dripleaf

pub mod dripleaf {
    use super::*;

    /// `DELAY_UNTIL_NEXT_TILT_STATE` (none: no tick).
    fn delay(tilt: u8) -> Option<i32> {
        match tilt {
            1 | 2 => Some(10),
            3 => Some(100),
            _ => None,
        }
    }

    fn tilt_of(s: &Block) -> u8 {
        match *s {
            Block::BigDripleaf { tilt, .. } => tilt,
            _ => 0,
        }
    }

    /// `canSurvive`: on a dripleaf, a stem or `#supports_big_dripleaf` (dirt, clay, mud among the
    /// modeled blocks).
    pub fn can_survive(w: &World, p: BlockPos) -> bool {
        use crate::block::Simple;
        matches!(w.get(p.below()), Block::BigDripleaf { .. } | Block::Simple(Simple::Dirt) | Block::Simple(Simple::Clay) | Block::Simple(Simple::Mud))
    }

    /// `updateShape`.
    pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: &Block) -> Block {
        if d == Dir::Down && !can_survive(w, p) {
            return Block::Air;
        }
        crate::fluid::waterlogged_update_shape(w, &s, p);
        if d == Dir::Up && matches!(ns, Block::BigDripleaf { .. }) {
            w.errors.push("a big dripleaf turning into a stem is not modeled".into());
        }
        s
    }

    /// `setTilt`: flags 2, a vibration for partial and full.
    fn set_tilt(w: &mut World, s: Block, p: BlockPos, tilt: u8) {
        let Block::BigDripleaf { facing, tilt: prev, waterlogged } = s else { return };
        w.set_block(p, Block::BigDripleaf { facing, tilt, waterlogged }, flags::CLIENTS);
        if tilt >= 2 && tilt != prev {
            w.game_event("block_change", p);
        }
    }

    /// `setTiltAndScheduleTick` (a tilt sound draws its pitch from the level random:
    /// `Mth.randomBetween(level.getRandom(), 0.8, 1.2)`, after the write, before the tick).
    fn set_tilt_and_schedule(w: &mut World, s: Block, p: BlockPos, tilt: u8, sound: bool) {
        set_tilt(w, s, p, tilt);
        if sound {
            w.draw_sound_pitch();
        }
        if let Some(d) = delay(tilt) {
            w.schedule_tick(p, Kind::BigDripleaf, d, Priority::Normal);
        }
    }

    /// `resetTilt`: tilt 0, and the tilt-up sound (a level-random pitch) when it was tilted.
    fn reset_tilt(w: &mut World, s: Block, p: BlockPos) {
        set_tilt(w, s, p, 0);
        if tilt_of(&s) != 0 {
            w.draw_sound_pitch();
        }
    }

    /// `entityInside`: an entity standing on the leaf (`canEntityTilt`: on the ground, above
    /// 11/16) tilts it, unless powered.
    pub fn entity_inside(w: &mut World, i: usize, s: Block, p: BlockPos) {
        let e = &w.entities.list[i];
        let can_tilt = e.on_ground && e.pos.y > (p.y as f32 + 0.6875f32) as f64;
        if tilt_of(&s) == 0 && can_tilt && !w.has_neighbor_signal(p) {
            set_tilt_and_schedule(w, s, p, 1, false);
        }
    }

    /// `tick`.
    pub fn tick(w: &mut World, s: Block, p: BlockPos) {
        if w.has_neighbor_signal(p) {
            reset_tilt(w, s, p);
            return;
        }
        match tilt_of(&s) {
            1 => set_tilt_and_schedule(w, s, p, 2, true),
            2 => set_tilt_and_schedule(w, s, p, 3, true),
            3 => reset_tilt(w, s, p),
            _ => {}
        }
    }

    /// `neighborChanged`: power resets the tilt.
    pub fn neighbor_changed(w: &mut World, s: Block, p: BlockPos) {
        if w.has_neighbor_signal(p) {
            reset_tilt(w, s, p);
        }
    }

    /// `getCollisionShape`: the leaf (none when fully tilted).
    pub fn collision_boxes(s: &Block) -> Vec<crate::behavior::Box16> {
        match tilt_of(s) {
            0 | 1 => vec![[0, 16, 11, 15, 0, 16]],
            2 => vec![[0, 16, 11, 13, 0, 16]],
            _ => vec![],
        }
    }
}

// ------------------------------------------------------------------------ tripwire hooks

pub mod hook {
    use super::*;

    /// `canSurvive`: horizontal, and the block behind holds a full face.
    pub fn can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
        let Block::TripwireHook { facing, .. } = *s else { return true };
        let rel = p.rel(facing.opposite());
        let b = w.get(rel);
        facing.is_horizontal() && bh::sturdy(w, &b, rel, facing, Support::Full)
    }

    /// `updateShape`.
    pub fn update_shape(w: &World, s: Block, p: BlockPos, d: Dir) -> Block {
        match s {
            Block::TripwireHook { facing, .. } if d.opposite() == facing && !can_survive(w, &s, p) => Block::Air,
            _ => s,
        }
    }

    /// `tick`.
    pub fn tick(w: &mut World, s: Block, p: BlockPos) {
        calculate_state(w, p, s, false, true, -1, None);
    }

    /// `affectNeighborsAfterRemoval`.
    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
        if !moved {
            on_removed(w, s, p);
        }
    }

    /// `onRemoved`.
    fn on_removed(w: &mut World, s: Block, p: BlockPos) {
        let Block::TripwireHook { facing, attached, powered } = s else { return };
        if attached || powered {
            calculate_state(w, p, s, true, false, -1, None);
        }
        if powered {
            notify_neighbors(w, p, facing);
        }
    }

    /// `ownSignal`.
    pub fn signal(s: &Block) -> i32 {
        if matches!(*s, Block::TripwireHook { powered: true, .. }) { 15 } else { 0 }
    }

    /// `getDirectSignal`: the block it hangs on.
    pub fn direct_signal(s: &Block, d: Dir) -> i32 {
        match *s {
            Block::TripwireHook { powered: true, facing, .. } if facing == d => 15,
            _ => 0,
        }
    }

    /// `notifyNeighbors`: the hook's neighbors, then the neighbors of the block it hangs on.
    fn notify_neighbors(w: &mut World, p: BlockPos, d: Dir) {
        let front = d.opposite();
        w.update_neighbors_at(p, Kind::TripwireHook);
        w.update_neighbors_at(p.rel(front), Kind::TripwireHook);
    }

    /// `emitState`: the game events (the sounds: fixed pitches, except the detach sound's
    /// level-random one).
    fn emit_state(w: &mut World, p: BlockPos, attached: bool, powered: bool, was_attached: bool, was_powered: bool) {
        if powered && !was_powered {
            w.game_event("block_activate", p);
        } else if !powered && was_powered {
            w.game_event("block_deactivate", p);
        } else if attached && !was_attached {
            w.game_event("block_attach", p);
        } else if !attached && was_attached {
            w.draw_sound_pitch();
            w.game_event("block_detach", p);
        }
    }

    /// `calculateState(level, pos, state, isBeingDestroyed, canUpdate, wireSource,
    /// wireSourceState)`.
    pub fn calculate_state(w: &mut World, p: BlockPos, s: Block, destroyed: bool, can_update: bool, wire_source: i32, wire_source_state: Option<Block>) {
        let Block::TripwireHook { facing: direction, attached: was_attached, powered: was_powered } = s else { return };
        let mut attached = !destroyed;
        let mut powered = false;
        let mut receiver = 0i32;
        let mut wire_states: [Option<Block>; 42] = [None; 42];
        for i in 1..42i32 {
            let tp = rel_n(p, direction, i);
            let mut ws = w.get(tp);
            if let Block::TripwireHook { facing, .. } = ws {
                if facing == direction.opposite() {
                    receiver = i;
                }
                break;
            }
            if ws.kind() != Kind::Tripwire && i != wire_source {
                wire_states[i as usize] = None;
                attached = false;
            } else {
                if i == wire_source {
                    ws = wire_source_state.unwrap_or(ws);
                }
                let (armed, wire_powered) = match ws {
                    Block::Tripwire { disarmed, powered, .. } => (!disarmed, powered),
                    _ => (true, false),
                };
                powered |= armed && wire_powered;
                wire_states[i as usize] = Some(ws);
                if i == wire_source {
                    w.schedule_tick(p, Kind::TripwireHook, 10, Priority::Normal);
                    attached &= armed;
                }
            }
        }
        attached &= receiver > 1;
        powered &= attached;
        // `block.defaultBlockState()` with ATTACHED and POWERED set: FACING is the default (north)
        let new_state = |facing: Dir| Block::TripwireHook { facing, attached, powered };
        if receiver > 0 {
            let tp = rel_n(p, direction, receiver);
            let opposite = direction.opposite();
            w.set_block_and_update(tp, new_state(opposite));
            notify_neighbors(w, tp, opposite);
            if w.get(p).kind() != Kind::TripwireHook {
                on_removed(w, new_state(Dir::North), p);
                return;
            }
            emit_state(w, tp, attached, powered, was_attached, was_powered);
        }
        emit_state(w, p, attached, powered, was_attached, was_powered);
        if !destroyed {
            w.set_block_and_update(p, new_state(direction));
            if can_update {
                notify_neighbors(w, p, direction);
            }
        }
        if was_attached != attached {
            for i in 1..receiver {
                let tp = rel_n(p, direction, i);
                if let Some(wd) = wire_states[i as usize] {
                    let cur = w.get(tp);
                    if matches!(cur.kind(), Kind::Tripwire | Kind::TripwireHook) {
                        let nd = match wd {
                            Block::Tripwire { powered, disarmed, north, east, south, west, .. } => Block::Tripwire { powered, attached, disarmed, north, east, south, west },
                            Block::TripwireHook { facing, powered, .. } => Block::TripwireHook { facing, attached, powered },
                            x => x,
                        };
                        w.set_block_and_update(tp, nd);
                    }
                }
            }
        }
    }
}
