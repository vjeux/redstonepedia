//! The dispatcher: every `Block`/`BlockBehaviour` method the engine calls, by block family, plus
//! the static properties (conductor, face sturdiness, signal source, analog output, push
//! reaction, block entity, note block instrument).

use crate::block::{Block, Face, Instrument, Kind, PushReaction, Simple};
use crate::pos::{BlockPos, Dir};
use crate::redstone::attach::{attached, button, torch};
use crate::redstone::blocks::{bulb, composter, dispenser, hopper, indicator, lamp, leaves, note, observer, rail, scaffolding, target};
use crate::redstone::{diode, wire};
use crate::world::World;

/// `SupportType`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Support {
    Full,
    Center,
    Rigid,
}

// ---------------------------------------------------------------------- static properties

/// Has any collision shape (blocks a walking player; the judge's `collides` flag).
pub fn collides(s: &Block) -> bool {
    match *s {
        // terrain: the game's own collision shape (the natural-state table)
        Block::Terrain(id) => !crate::terrain::blocks::props(id).boxes.is_empty(),
        Block::Air
        | Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Rail { .. } => false,
        // an open fence gate has no collision box
        Block::FenceGate { open, .. } => !open,
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::WallBanner { .. } => false,
        Block::PlainRail { .. } | Block::DetectorRail { .. } => false,
        Block::Water { .. } | Block::BubbleColumn { .. } => false,
        // ---- TNT dupers (track tnt): dead coral fans have no collision ----
        Block::CoralFan { .. } | Block::CoralWallFan { .. } => false,
        // ---- crop farms: noCollision (a bamboo stalk has its 3 px column) ----
        Block::SugarCane { .. } | Block::BambooSapling | Block::Kelp { .. } | Block::KelpPlant => false,
        Block::Stem { .. } | Block::AttachedStem { .. } => false,
        Block::CactusFlower => false,
        // ---- mob farms (ironfarm): lava has no collision ----
        Block::Lava { .. } | Block::Sign { .. } => false,
        // ---- fire (mobinfra): noCollision ----
        Block::Fire { .. } => false,
        // ---- placed light sources (terrain lane): the game's collision shape ----
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => light_row(s).is_some_and(|p| !p.boxes.is_empty()),
        _ => true,
    }
}

/// `isCollisionShapeFullBlock` (the judge's `full` flag: a door cell must be a full cube).
pub fn full_cube(s: &Block) -> bool {
    match *s {
        Block::Terrain(id) => crate::terrain::blocks::props(id).full_collision,
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => light_row(s).is_some_and(|p| p.full_collision),
        Block::Simple(x) => x.full() && !matches!(x, Simple::SoulSand | Simple::HoneyBlock | Simple::Mud),
        Block::Glazed { .. }
        | Block::RedstoneBlock
        | Block::Observer { .. }
        | Block::Lamp { .. }
        | Block::Bulb { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::RespawnAnchor { .. }
        | Block::Log { .. }
        | Block::Leaves { .. }
        | Block::Falling { .. } => true,
        Block::Slab { kind } => kind == 2,
        Block::Piston { extended, .. } => !extended,
        // ---- entity-reacting blocks (clk-misc) ----
        Block::StoneSlab { kind } => kind == 2,
        // ---- the slab family (terrain lane) ----
        Block::AnySlab { kind, .. } => kind == 2,
        Block::ChiseledBookshelf { .. } | Block::Jukebox { .. } | Block::Furnace { .. } => true,
        // ---- TNT dupers (track tnt) ----
        Block::Tnt { .. } => true,
        // ---- shulker study (clk-rt): a closed shulker box's dynamic shape is the full block ----
        Block::ShulkerBox { .. } => true,
        // ---- crop farms (webrender): the grate's full cube, with its water ----
        Block::WaterloggedGrate => true,
        _ => false,
    }
}

/// `BlockStateBase.isCollisionShapeFullBlock(level, pos)` (clk-misc, `moveTowardsClosestSpace`):
/// [`full_cube`]'s cached flag, or for a moving piston (a dynamic shape, no cache) its shape at
/// `pos` (a retracting base's moving block is the full block until its head clears it).
pub fn collision_full_block_at(w: &World, s: &Block, p: BlockPos) -> bool {
    match s {
        Block::MovingPiston { .. } => {
            let boxes: Vec<[i32; 6]> = crate::piston::shape::moving(w, p).into_iter().map(|b| b.0).filter(|b| b[0] < b[1] && b[2] < b[3] && b[4] < b[5]).collect();
            // Block.isShapeFullBlock: the union is exactly the cell (nothing outside, no hole)
            if boxes.iter().any(|b| b[0] < 0 || b[1] > 16 || b[2] < 0 || b[3] > 16 || b[4] < 0 || b[5] > 16) {
                return false;
            }
            let mut filled = [[0u32; 16]; 16];
            for b in &boxes {
                for row in filled.iter_mut().take(b[3] as usize).skip(b[2] as usize) {
                    for m in row.iter_mut().take(b[5] as usize).skip(b[4] as usize) {
                        *m |= ((1u32 << b[1]) - 1) & !((1u32 << b[0]) - 1);
                    }
                }
            }
            filled.iter().all(|row| row.iter().all(|m| *m == 0xffff))
        }
        _ => full_cube(s),
    }
}

/// `BlockState.isSolid()` (`calculateSolid`): forced on, or a collision box whose mean size is
/// at least 0.7291666 (a 3 px slab is exactly that) or a full block tall.
pub fn is_solid(s: &Block) -> bool {
    match *s {
        Block::Terrain(id) => crate::terrain::blocks::props(id).solid,
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => light_row(s).is_some_and(|p| p.solid),
        Block::Air
        | Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Rail { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. }
        // dynamic shape: no shape cache, so `calculateSolid` says no (NULL_CACHE)
        | Block::Scaffolding { .. }
        // fluids: no collision
        | Block::Water { .. }
        | Block::BubbleColumn { .. } => false,
        // ---- entity-reacting blocks (clk-misc) ----
        // no collision and not forced solid (plates, wall banners and lanterns are forceSolidOn)
        Block::Tripwire { .. } | Block::TripwireHook { .. } => false,
        Block::PlainRail { .. } | Block::DetectorRail { .. } => false,
        // forceSolidOff
        Block::BigDripleaf { .. } => false,
        // dynamic shape (no shape cache) and no collision without an entity
        Block::PowderSnow => false,
        // ---- crop farms (webrender): sugar cane has no collision ----
        Block::SugarCane { .. } => false,
        // ---- crop farms (clk-misc): kelp has no collision and is not forced solid (bamboo and
        // its shoot are: forceSolidOn) ----
        Block::Kelp { .. } | Block::KelpPlant => false,
        // stems: no collision (farmland's 15 px column is solid)
        Block::Stem { .. } | Block::AttachedStem { .. } => false,
        // a cactus flower has no collision (a cactus's 14 x 15 px column is solid)
        Block::CactusFlower => false,
        // ---- mob farms (ironfarm): lava has no collision (a bed's 9 px box averages 0.85: solid) ----
        Block::Lava { .. } => false,
        // (mob-AI 2026-11-10) a sign has no collision but is forceSolidOn: solid (a skeleton's
        // stroll target on a wall sign is moved above it: d1-bed-plate plains t37233)
        Block::Sign { .. } => true,
        // ---- fire (mobinfra): no collision, not forced solid ----
        Block::Fire { .. } => false,
        _ => true,
    }
}

/// `isRedstoneConductor`.
/// `isRedstoneConductor` (every modeled block's predicate depends on the state alone).
pub fn is_conductor(_w: &World, s: &Block, _p: BlockPos) -> bool {
    conductor_of(s)
}

/// `isRedstoneConductor` without a world (for static models: the search tools' enumerators).
pub fn conductor_of(s: &Block) -> bool {
    match *s {
        Block::Terrain(id) => crate::terrain::blocks::props(id).conductor,
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => light_row(s).is_some_and(|p| p.conductor),
        Block::Simple(x) => x.conductor(),
        Block::Glazed { .. } | Block::Lamp { .. } | Block::Note { .. } | Block::Target { .. } | Block::Dispenser { .. } | Block::Crafter { .. } => true,
        Block::Log { .. } | Block::RespawnAnchor { .. } => true,
        Block::Falling { .. } => true, // pistmodel
        Block::Slab { kind } => kind == 2,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::is_conductor(s),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::StoneSlab { kind } => kind == 2,
        // ---- the slab family (terrain lane) ----
        Block::AnySlab { kind, .. } => kind == 2,
        Block::ChiseledBookshelf { .. } | Block::Jukebox { .. } | Block::Furnace { .. } => true,
        // ---- shulker study (clk-rt): the default predicate, isCollisionShapeFullBlock (closed) ----
        Block::ShulkerBox { .. } => true,
        _ => false,
    }
}

/// `isFaceSturdy(level, pos, face, support)`: the block's support shape covers that face.
pub fn sturdy(w: &World, s: &Block, p: BlockPos, face: Dir, support: Support) -> bool {
    match *s {
        // pistons' collision boxes (a retracting base keeps its extended box): pistmodel
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::sturdy_at(w, s, p, face, support),
        _ => sturdy_of(s, face, support),
    }
}

/// `isFaceSturdy` without a world (for static models; a moving piston's box needs its block
/// entity, see `sturdy`).
pub fn sturdy_of(s: &Block, face: Dir, support: Support) -> bool {
    match *s {
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => {
            let k = match support {
                Support::Full => 0,
                Support::Center => 1,
                Support::Rigid => 2,
            };
            light_row(s).is_some_and(|p| p.sturdy[k][crate::terrain::blocks::dir_index(face)])
        }
        Block::Terrain(id) => {
            let k = match support {
                Support::Full => 0,
                Support::Center => 1,
                Support::Rigid => 2,
            };
            crate::terrain::blocks::props(id).sturdy[k][crate::terrain::blocks::dir_index(face)]
        }
        Block::Simple(Simple::HoneyBlock) => face == Dir::Down && support == Support::Center,
        Block::Simple(x) => x.full(),
        Block::Glazed { .. }
        | Block::RedstoneBlock
        | Block::Observer { .. }
        | Block::Lamp { .. }
        | Block::Bulb { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::RespawnAnchor { .. }
        | Block::Log { .. }
        | Block::Falling { .. } => true,
        Block::Repeater { .. } | Block::Comparator { .. } => face == Dir::Down,
        Block::Trapdoor { facing, top, open, .. } => {
            let full = if open {
                facing.opposite()
            } else if top {
                Dir::Up
            } else {
                Dir::Down
            };
            face == full
        }
        Block::Door { facing, hinge_right, open, .. } => {
            let d = if open {
                if hinge_right { facing.counter_clockwise() } else { facing.clockwise() }
            } else {
                facing
            };
            face == d.opposite()
        }
        Block::FenceGate { open, .. } => !open && face == Dir::Up && support == Support::Center,
        Block::Hopper { facing, .. } => (face == Dir::Up && support == Support::Rigid) || (face == Dir::Down && facing == Dir::Down && support == Support::Center),
        Block::Slab { kind } => kind == 2 || (kind == 0 && face == Dir::Down) || (kind == 1 && face == Dir::Up),
        Block::Skull { .. } => face == Dir::Down && support == Support::Center,
        Block::WallSkull { facing, .. } => face == facing.opposite() && support == Support::Center,
        Block::Cake { bites } => face == Dir::Down && support == Support::Center && bites <= 3,
        Block::EndPortalFrame { eye, .. } => face == Dir::Down || (eye && face == Dir::Up && support == Support::Center),
        Block::Composter { .. } => face != Dir::Up || support == Support::Rigid,
        // support shape = collision shape with the empty context = SHAPE_STABLE: the top plate
        // (14-16 px) and four 2x2 px legs
        Block::Scaffolding { .. } => face == Dir::Up,
        // SculkSensorBlock.SHAPE = column(16, 0, 8): only the bottom face is full
        Block::Sculk { .. } => face == Dir::Down,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::sturdy(s, face, support),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::StoneSlab { kind } => kind == 2 || (kind == 0 && face == Dir::Down) || (kind == 1 && face == Dir::Up),
        // ---- the slab family (terrain lane) ----
        Block::AnySlab { kind, .. } => kind == 2 || (kind == 0 && face == Dir::Down) || (kind == 1 && face == Dir::Up),
        Block::ChiseledBookshelf { .. } | Block::Jukebox { .. } | Block::Furnace { .. } => true,
        Block::DaylightDetector { .. } => face == Dir::Down,
        // ---- TNT dupers (track tnt) ----
        Block::Tnt { .. } => true,
        // ---- shulker study (clk-rt): a closed box supports with the full block; a chest's
        // bottom (1..15 px) covers the center column only ----
        Block::ShulkerBox { .. } => true,
        Block::Chest { .. } => face == Dir::Down && support == Support::Center,
        // ---- brewer study (track sorter): the base (column 14, 0..2 px) covers the bottom's
        // center; the rod ends at 14 px (no top support) ----
        Block::BrewingStand { .. } => face == Dir::Down && support == Support::Center,
        // ---- fences (clk-misc): support shape = the 24 px post + arms ----
        Block::Fence { .. } => crate::fence::sturdy(s, face, support),
        // ---- crop farms (webrender): the grate's full cube ----
        Block::WaterloggedGrate => true,
        // ---- stem farms (clk-misc): farmland's column(16, 0, 15) covers the bottom face only ----
        Block::Farmland { .. } => face == Dir::Down,
        // ---- cactus farms (clk-misc): column(14, 0, 15): only the center of the bottom face ----
        Block::Cactus { .. } => face == Dir::Down && support == Support::Center,
        // ---- weather (mobinfra): a rod's 4 x 4 px ends cover the center of its two end faces ----
        Block::LightningRod { facing, .. } => (face == facing || face == facing.opposite()) && support == Support::Center,
        _ => false,
    }
}

/// `isSignalSource`.
pub fn is_signal_source(w: &World, s: &Block) -> bool {
    match *s {
        Block::Terrain(id) => crate::terrain::blocks::props(id).signal,
        Block::Wire { .. } => w.wire_signal.get(),
        Block::RedstoneBlock
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Observer { .. }
        | Block::Target { .. }
        | Block::Sculk { .. } => true,
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } | Block::TripwireHook { .. } | Block::DetectorRail { .. } | Block::DaylightDetector { .. } | Block::Jukebox { .. } => true,
        // ---- weather (mobinfra) ----
        Block::LightningRod { .. } => true,
        _ => false,
    }
}

pub fn is_diode(s: &Block) -> bool {
    matches!(s, Block::Repeater { .. } | Block::Comparator { .. })
}

/// `getSignal(state, level, pos, direction)`.
pub fn get_signal(w: &World, s: &Block, p: BlockPos, d: Dir) -> i32 {
    match *s {
        Block::Wire { .. } => wire::get_signal(w, s, p, d),
        Block::RedstoneBlock => 15,
        Block::Torch { .. } | Block::WallTorch { .. } => torch::get_signal(s, d),
        Block::Lever { .. } | Block::Button { .. } => attached::get_signal(s),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::get_signal(w, s, p, d),
        Block::Observer { .. } => observer::get_signal(s, d),
        Block::Target { power } => power as i32,
        Block::Sculk { .. } => crate::sculk::get_signal(s, d),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::signal(s),
        Block::TripwireHook { .. } => crate::react::hook::signal(s),
        Block::DetectorRail { .. } => crate::rails::detector::signal(s),
        // ownSignal: its power, every side
        Block::DaylightDetector { power, .. } => power as i32,
        // JukeboxBlock.ownSignal: 15 while a song plays
        Block::Jukebox { .. } => crate::redstone::container::jukebox_signal(w, p),
        // ---- weather (mobinfra): a struck rod, 15 on every side ----
        Block::LightningRod { .. } => crate::weather::lightning::rod_signal(s, None),
        _ => 0,
    }
}

/// `getDirectSignal` (strong power).
pub fn get_direct_signal(w: &World, s: &Block, p: BlockPos, d: Dir) -> i32 {
    match *s {
        Block::Wire { .. } => wire::get_direct_signal(w, s, p, d),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::get_direct_signal(s, d),
        Block::Lever { .. } | Block::Button { .. } => attached::get_direct_signal(s, d),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::get_signal(w, s, p, d),
        Block::Observer { .. } => observer::get_signal(s, d),
        Block::Sculk { .. } => crate::sculk::get_direct_signal(s, d),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::direct_signal(s, d),
        Block::TripwireHook { .. } => crate::react::hook::direct_signal(s, d),
        Block::DetectorRail { .. } => crate::rails::detector::direct_signal(s, d),
        // the daylight detector's power is weak only (getDirectSignal: 0)
        Block::DaylightDetector { .. } | Block::Jukebox { .. } => 0,
        // ---- weather (mobinfra): a struck rod strongly powers the block it hangs on ----
        Block::LightningRod { .. } => crate::weather::lightning::rod_signal(s, Some(d)),
        _ => 0,
    }
}

/// `shouldRedstoneWireConnectTo(state, level, pos, direction)`.
pub fn should_wire_connect(w: &World, s: &Block, _p: BlockPos, d: Option<Dir>) -> bool {
    match *s {
        Block::Wire { .. } => true,
        Block::Repeater { facing, .. } => Some(facing) == d || Some(facing.opposite()) == d,
        Block::Observer { facing, .. } => Some(facing) == d,
        _ => is_signal_source(w, s) && d.is_some(),
    }
}

pub fn has_analog_output(s: &Block) -> bool {
    matches!(
        s,
        Block::Bulb { .. }
            | Block::Dispenser { .. }
            | Block::Crafter { .. }
            | Block::Hopper { .. }
            | Block::Cake { .. }
            | Block::EndPortalFrame { .. }
            | Block::Composter { .. }
            | Block::RespawnAnchor { .. }
            | Block::Sculk { .. }
    ) || matches!(s, Block::DetectorRail { .. } | Block::ChiseledBookshelf { .. } | Block::Jukebox { .. }) // ---- entity-reacting blocks (clk-misc) ----
        || matches!(s, Block::ShulkerBox { .. } | Block::Chest { .. }) // ---- shulker study (clk-rt) ----
        || matches!(s, Block::Furnace { .. }) // ---- smelter study (track sorter) ----
        || matches!(s, Block::BrewingStand { .. }) // ---- brewer study (track sorter) ----
}

/// `getAnalogOutputSignal` (containers are empty: 0).
pub fn analog_output(w: &World, s: &Block, p: BlockPos, _d: Dir) -> i32 {
    match *s {
        Block::Bulb { .. } => match w.get(p) {
            Block::Bulb { lit: true, .. } => 15,
            _ => 0,
        },
        Block::Cake { bites } => (7 - bites as i32) * 2,
        Block::EndPortalFrame { eye, .. } => {
            if eye {
                15
            } else {
                0
            }
        }
        Block::Composter { level } => level as i32,
        Block::Sculk { .. } => crate::sculk::analog_output(w, s, p),
        Block::RespawnAnchor { charges } => (charges as f32 / 4.0 * 15.0).floor() as i32,
        Block::Dispenser { .. } | Block::Hopper { .. } | Block::Crafter { .. } => crate::redstone::container::analog_output(w, p),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::ChiseledBookshelf { .. } | Block::Jukebox { .. } => crate::redstone::container::analog_output(w, p),
        Block::DetectorRail { .. } => crate::rails::detector::analog_output(w, s, p),
        // ---- shulker study (clk-rt) ----
        Block::ShulkerBox { .. } | Block::Chest { .. } => crate::redstone::container::analog_output(w, p),
        // ---- smelter study (track sorter) ----
        Block::Furnace { .. } => crate::redstone::container::analog_output(w, p),
        Block::BrewingStand { .. } => crate::redstone::container::analog_output(w, p),
        _ => 0,
    }
}

/// `BlockState.instrument()`.
pub fn instrument(s: &Block) -> Instrument {
    match *s {
        Block::Simple(x) => x.instrument(),
        Block::Glazed { .. } | Block::Observer { .. } | Block::Dispenser { .. } | Block::Slab { .. } | Block::EndPortalFrame { .. } | Block::RespawnAnchor { .. } => {
            Instrument::Basedrum
        }
        Block::Note { .. } | Block::Log { .. } | Block::Composter { .. } => Instrument::Bass,
        Block::Falling { .. } => Instrument::Snare, // pistmodel
        Block::Trapdoor { iron: false, .. } | Block::Door { iron: false, .. } | Block::FenceGate { .. } => Instrument::Bass,
        // (the wall variant has fresh properties: harp)
        Block::Skull { .. } => Instrument::Skeleton,
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { kind: 0, .. } | Block::WallBanner { .. } | Block::ChiseledBookshelf { .. } | Block::DaylightDetector { .. } | Block::Jukebox { .. } => Instrument::Bass,
        Block::Plate { kind: 1, .. } | Block::StoneSlab { .. } => Instrument::Basedrum,
        // ---- the slab family (terrain lane): its base's instrument ----
        Block::AnySlab { mat, .. } => {
            if crate::slab::wood(mat) {
                Instrument::Bass
            } else {
                Instrument::Basedrum
            }
        }
        // ---- smelter study (track sorter): a furnace is stone (BASEDRUM) ----
        Block::Furnace { .. } => Instrument::Basedrum,
        // ---- TNT dupers (track tnt) ----
        Block::CoralFan { .. } | Block::CoralWallFan { .. } => Instrument::Basedrum,
        // ---- fences (clk-misc): BASS, the nether brick fence BASEDRUM ----
        Block::Fence { kind, .. } => {
            if crate::fence::wooden(kind) {
                Instrument::Bass
            } else {
                Instrument::Basedrum
            }
        }
        _ => Instrument::Harp,
    }
}

/// `getPistonPushReaction` (pistons themselves: see `piston::push_reaction`).
pub fn push_reaction(s: &Block) -> PushReaction {
    match *s {
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => match light_row(s).map(|p| p.push.as_str()) {
            Some("DESTROY") => PushReaction::Popped,
            Some("BLOCK") => PushReaction::Immoveable,
            Some("PUSH_ONLY") => PushReaction::Push,
            Some("IGNORE") => PushReaction::IgnoreEntity,
            _ => PushReaction::PushPull,
        },
        Block::Simple(x) => x.push_reaction(),
        Block::Glazed { .. } => PushReaction::Push,
        Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. }
        | Block::Cake { .. }
        | Block::Leaves { .. }
        | Block::Scaffolding { .. }
        | Block::Door { .. } => PushReaction::Popped,
        // LiquidBlock / BubbleColumnBlock: PushReaction.DESTROY (no loot: nothing drops)
        Block::Water { .. } | Block::BubbleColumn { .. } => PushReaction::Popped,
        Block::RespawnAnchor { .. } => PushReaction::Immoveable,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::push_reaction(s),
        // ---- entity-reacting blocks (clk-misc) ----
        // (wall banners: NORMAL, but a block entity makes them immovable)
        Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::Lantern { .. } | Block::BigDripleaf { .. } => PushReaction::Popped,
        // ---- shulker study (clk-rt): shulkerBoxProperties' PushReaction.POPPED (a chest stays
        // NORMAL: its block entity makes it immovable) ----
        Block::ShulkerBox { .. } => PushReaction::Popped,
        // ---- crop farms (webrender): sugar cane is PushReaction.DESTROY; the grate's own ----
        Block::SugarCane { .. } => PushReaction::Popped,
        Block::WaterloggedGrate => Simple::WaxedCopperGrate.push_reaction(),
        // ---- crop farms (clk-misc): bamboo, its shoot and kelp are PushReaction.POPPED ----
        Block::Bamboo { .. } | Block::BambooSapling | Block::Kelp { .. } | Block::KelpPlant => PushReaction::Popped,
        // ---- stem farms (clk-misc): stems are POPPED (farmland is NORMAL) ----
        Block::Stem { .. } | Block::AttachedStem { .. } => PushReaction::Popped,
        // ---- cactus farms (clk-misc): a cactus and its flower are POPPED ----
        Block::Cactus { .. } | Block::CactusFlower => PushReaction::Popped,
        // ---- mob farms (ironfarm): lava (LiquidBlock), beds and signs are PushReaction.DESTROY ----
        Block::Lava { .. } | Block::Bed { .. } | Block::Sign { .. } => PushReaction::Popped,
        // ---- fire (mobinfra): PushReaction.DESTROY (no loot) ----
        Block::Fire { .. } => PushReaction::Popped,
        _ => PushReaction::PushPull,
    }
}

/// Leaves a drop (an item entity) when destroyed with drops: false for blocks without a loot
/// table (`noLootTable`: piston heads, moving pistons, end portal frames) and air.
pub fn drops_item(s: &Block) -> bool {
    !matches!(s, Block::Air | Block::PistonHead { .. } | Block::MovingPiston { .. } | Block::EndPortalFrame { .. } | Block::Water { .. } | Block::BubbleColumn { .. })
        // ---- TNT dupers (track tnt): VanillaBlockLoot: TNT drops itself unless unstable, dead
        // coral fans only with silk touch
        && !matches!(s, Block::Tnt { unstable: true } | Block::CoralFan { .. } | Block::CoralWallFan { .. })
        // ---- fire (mobinfra): noLootTable ----
        && !matches!(s, Block::Fire { .. })
}

/// Destroy speed -1 (unbreakable: pistons cannot move it).
pub fn unbreakable(s: &Block) -> bool {
    matches!(s, Block::EndPortalFrame { .. } | Block::MovingPiston { .. })
}

/// `hasBlockEntity`.
pub fn has_block_entity(s: &Block) -> bool {
    matches!(
        s,
        Block::Comparator { .. } | Block::Dispenser { .. } | Block::Crafter { .. } | Block::Hopper { .. } | Block::Skull { .. } | Block::WallSkull { .. } | Block::MovingPiston { .. } | Block::Sculk { .. }
    ) || matches!(s, Block::WallBanner { .. } | Block::ChiseledBookshelf { .. } | Block::DaylightDetector { .. } | Block::Jukebox { .. }) // ---- entity-reacting blocks (clk-misc) ----
        || matches!(s, Block::ShulkerBox { .. } | Block::Chest { .. }) // ---- shulker study (clk-rt) ----
        || matches!(s, Block::Furnace { .. }) // ---- smelter study (track sorter) ----
        || matches!(s, Block::BrewingStand { .. }) // ---- brewer study (track sorter) ----
        || light_row(s).is_some_and(|p| p.block_entity) // ---- placed light sources: a campfire ----
}

/// A placed light source's row of the natural-state table (terrain lane: the table holds every
/// state of each of them, the game's own answers for shapes, solidity, conduction, support,
/// pistons and light).
pub fn light_row(s: &Block) -> Option<&'static crate::terrain::blocks::Props> {
    match s {
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => crate::terrain::blocks::id_of(&s.to_string()).map(crate::terrain::blocks::props),
        _ => None,
    }
}

/// `canBeReplaced` (by a moving block / a falling block): air, the fluids and fire
/// (`replaceable()`).
pub fn can_be_replaced(s: &Block) -> bool {
    s.is_air() || matches!(s, Block::Water { .. } | Block::BubbleColumn { .. } | Block::Fire { .. })
}

/// A box in 1/16 block units, laid out `[x0, x1, y0, y1, z0, z1]` (pistmodel's `B16`).
pub type Box16 = [i32; 6];

/// A plate `t` px thick against `face`, full on the other two axes.
fn plate(face: Dir, t: i32) -> Box16 {
    match face {
        Dir::Down => [0, 16, 0, t, 0, 16],
        Dir::Up => [0, 16, 16 - t, 16, 0, 16],
        Dir::North => [0, 16, 0, 16, 0, t],
        Dir::South => [0, 16, 0, 16, 16 - t, 16],
        Dir::West => [0, t, 0, 16, 0, 16],
        Dir::East => [16 - t, 16, 0, 16, 0, 16],
    }
}

/// `getCollisionShape(level, pos, CollisionContext.empty())` of a non-piston block, as boxes
/// (1/16 units; see [`Box16`]). What a moving block's shape is offset from
/// (`PistonMovingBlockEntity.getCollisionShape`), and what a falling block collides with
/// (scaffolding's entity-dependent shape: see `redstone::blocks::scaffolding::entity_boxes`).
/// Pistons, heads and moving pistons: `piston::shape` (empty here).
pub fn collision_boxes(s: &Block) -> Vec<Box16> {
    const FULL: Box16 = [0, 16, 0, 16, 0, 16];
    match *s {
        // terrain: the game's boxes to the nearest pixel (entities read them exactly:
        // `entity::collide::boxes_at`)
        Block::Terrain(id) => crate::terrain::blocks::props(id)
            .boxes
            .iter()
            .map(|b| {
                let p = |v: f64| (v * 16.0).round() as i32;
                [p(b[0]), p(b[3]), p(b[1]), p(b[4]), p(b[2]), p(b[5])]
            })
            .collect(),
        // placed light sources (terrain lane): the game's boxes from the table
        Block::PlainTorch { .. }
        | Block::PlainWallTorch { .. }
        | Block::SoulLantern { .. }
        | Block::JackOLantern { .. }
        | Block::Campfire { .. }
        | Block::EndRod { .. }
        | Block::Froglight { .. }
        | Block::SeaPickle { .. }
        | Block::Candle { .. } => light_row(s)
            .map(|p| {
                p.boxes
                    .iter()
                    .map(|b| {
                        let q = |v: f64| (v * 16.0).round() as i32;
                        [q(b[0]), q(b[3]), q(b[1]), q(b[4]), q(b[2]), q(b[5])]
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Block::Air
        | Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Rail { .. }
        | Block::Water { .. }
        | Block::BubbleColumn { .. } => vec![],
        // ---- crop farms (webrender): noCollision ----
        Block::SugarCane { .. } => vec![],
        // ---- crop farms (clk-misc): noCollision; a bamboo stalk's column(3, 0, 16) to the
        // nearest pixels (entities use its exact, offset box: entity::collide::boxes_at) ----
        Block::BambooSapling | Block::Kelp { .. } | Block::KelpPlant => vec![],
        Block::Bamboo { .. } => vec![[6, 10, 0, 16, 6, 10]],
        // ---- stem farms (clk-misc): stems have no collision; farmland is column(16, 0, 15) ----
        Block::Stem { .. } | Block::AttachedStem { .. } => vec![],
        Block::Farmland { .. } => vec![[0, 16, 0, 15, 0, 16]],
        // ---- cactus farms (clk-misc): SHAPE_COLLISION column(14, 0, 15); the flower: none ----
        Block::Cactus { .. } => vec![crate::cactus::COLLISION],
        Block::CactusFlower => vec![],
        // ---- mob farms (ironfarm): lava none; a bed half (`BedBlock.SHAPES`): column(16, 3, 9)
        // plus two 3 px legs at its outer end (the head's legs toward `facing`, the foot's away) ----
        Block::Lava { .. } | Block::Sign { .. } => vec![],
        Block::Bed { facing, head, .. } => crate::entity::mob::blocks::bed_boxes(facing, head),
        Block::Simple(Simple::HoneyBlock) => vec![[1, 15, 0, 15, 1, 15]],
        Block::Simple(Simple::SoulSand) | Block::Simple(Simple::Mud) => vec![[0, 16, 0, 14, 0, 16]],
        // DiodeBlock.SHAPE = column(16, 0, 2)
        Block::Repeater { .. } | Block::Comparator { .. } => vec![[0, 16, 0, 2, 0, 16]],
        // TrapDoorBlock.SHAPES = rotateAll(boxZ(16, 13, 16)), keyed by facing (open) or DOWN/UP
        Block::Trapdoor { facing, top, open, .. } => vec![plate(if open { facing.opposite() } else if top { Dir::Up } else { Dir::Down }, 3)],
        // DoorBlock.SHAPES = rotateHorizontal(boxZ(16, 13, 16))
        Block::Door { facing, hinge_right, open, .. } => {
            let d = if open {
                if hinge_right { facing.counter_clockwise() } else { facing.clockwise() }
            } else {
                facing
            };
            vec![plate(d.opposite(), 3)]
        }
        // FenceGateBlock.SHAPE_COLLISION = column(16, 4, 0, 24) (1.5 blocks tall), empty open
        Block::FenceGate { facing, open, .. } => {
            if open {
                vec![]
            } else if matches!(facing, Dir::North | Dir::South) {
                vec![[0, 16, 0, 24, 6, 10]]
            } else {
                vec![[6, 10, 0, 24, 0, 16]]
            }
        }
        // ---- fences (clk-misc): post + arms, 24 px tall ----
        Block::Fence { north, east, south, west, .. } => crate::fence::collision_boxes(north, east, south, west),
        Block::Slab { kind } => match kind {
            0 => vec![[0, 16, 0, 8, 0, 16]],
            1 => vec![[0, 16, 8, 16, 0, 16]],
            _ => vec![FULL],
        },
        // HopperBlock: rim and floor (column(16, 10, 16) minus column(12, 11, 16)), the middle
        // column(8, 4, 10), and the spout (rotateAll(boxZ(4, 4, 8, 0, 8)) about (8, 6, 8))
        Block::Hopper { facing, .. } => {
            let mut v = vec![[0, 16, 10, 11, 0, 16], [0, 2, 11, 16, 0, 16], [14, 16, 11, 16, 0, 16], [2, 14, 11, 16, 0, 2], [2, 14, 11, 16, 14, 16], [4, 12, 4, 10, 4, 12]];
            v.push(match facing {
                Dir::North => [6, 10, 4, 8, 0, 8],
                Dir::South => [6, 10, 4, 8, 8, 16],
                Dir::West => [0, 8, 4, 8, 6, 10],
                Dir::East => [8, 16, 4, 8, 6, 10],
                _ => [6, 10, 0, 6, 6, 10],
            });
            v
        }
        Block::Skull { .. } => vec![[4, 12, 0, 8, 4, 12]],
        // SculkSensorBlock.SHAPE = column(16, 0, 8)
        Block::Sculk { .. } => vec![[0, 16, 0, 8, 0, 16]],
        // WallSkullBlock.SHAPES = rotateHorizontal(boxZ(8, 8, 16)): against the wall behind it
        Block::WallSkull { facing, .. } => vec![match facing {
            Dir::North => [4, 12, 4, 12, 8, 16],
            Dir::South => [4, 12, 4, 12, 0, 8],
            Dir::West => [8, 16, 4, 12, 4, 12],
            _ => [0, 8, 4, 12, 4, 12],
        }],
        Block::Cake { bites } => vec![[1 + 2 * bites as i32, 15, 0, 8, 1, 15]],
        Block::EndPortalFrame { eye, .. } => {
            if eye {
                vec![[0, 16, 0, 13, 0, 16], [4, 12, 13, 16, 4, 12]]
            } else {
                vec![[0, 16, 0, 13, 0, 16]]
            }
        }
        // ComposterBlock.getCollisionShape = SHAPES[0]: the block minus column(12, 2, 16)
        Block::Composter { .. } => vec![[0, 16, 0, 2, 0, 16], [0, 2, 2, 16, 0, 16], [14, 16, 2, 16, 0, 16], [2, 14, 2, 16, 0, 2], [2, 14, 2, 16, 14, 16]],
        // ScaffoldingBlock with the empty context: SHAPE_STABLE (top plate + 4 legs)
        Block::Scaffolding { .. } => vec![[0, 16, 14, 16, 0, 16], [0, 2, 0, 16, 0, 2], [14, 16, 0, 16, 0, 2], [0, 2, 0, 16, 14, 16], [14, 16, 0, 16, 14, 16]],
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => vec![],
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::WallBanner { .. } => vec![],
        Block::PlainRail { .. } | Block::DetectorRail { .. } => vec![],
        Block::BigDripleaf { .. } => crate::react::dripleaf::collision_boxes(s),
        // DaylightDetectorBlock.SHAPE = column(16, 0, 6)
        Block::DaylightDetector { .. } => vec![[0, 16, 0, 6, 0, 16]],
        // PowderSnowBlock.getCollisionShape: empty without an entity (no support either)
        Block::PowderSnow => vec![],
        // LanternBlock: column(4, 7, 9) + column(6, 0, 7), 1 px higher when hanging
        Block::Lantern { hanging } => {
            let h = hanging as i32;
            vec![[6, 10, 7 + h, 9 + h, 6, 10], [5, 11, h, 7 + h, 5, 11]]
        }
        Block::StoneSlab { kind } | Block::AnySlab { kind, .. } => match kind {
            0 => vec![[0, 16, 0, 8, 0, 16]],
            1 => vec![[0, 16, 8, 16, 0, 16]],
            _ => vec![FULL],
        },
        // ---- TNT dupers (track tnt): dead coral fans have no collision ----
        Block::CoralFan { .. } | Block::CoralWallFan { .. } => vec![],
        // ---- shulker study (clk-rt): ChestBlock.SHAPE = column(14, 0, 14) ----
        Block::Chest { .. } => vec![[1, 15, 0, 14, 1, 15]],
        // ---- brewer study (track sorter): BrewingStandBlock.SHAPE = column(2, 2, 14) + column(14, 0, 2) ----
        Block::BrewingStand { .. } => vec![[7, 9, 2, 14, 7, 9], [1, 15, 0, 2, 1, 15]],
        // ---- weather (mobinfra): RodBlock's 4 x 4 px column ----
        Block::LightningRod { facing, .. } => crate::weather::lightning::rod_boxes(facing),
        // ---- fire (mobinfra): noCollision ----
        Block::Fire { .. } => vec![],
        _ => vec![FULL],
    }
}

// ---------------------------------------------------------------------- behavior

pub fn can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
    match *s {
        Block::Wire { .. } => wire::can_survive(w, p),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::can_survive(w, p),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::can_survive(w, s, p),
        Block::Lever { .. } | Block::Button { .. } => attached::can_survive(w, s, p),
        Block::Rail { .. } => rail::can_survive(w, p),
        Block::Door { .. } => indicator::door_can_survive(w, s, p),
        Block::Cake { .. } => is_solid(&w.get(p.below())),
        Block::Scaffolding { .. } => scaffolding::distance(w, p) < 7,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::can_survive(w, *s, p),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::can_survive(w, p),
        // ---- crop farms (webrender) ----
        Block::SugarCane { .. } => crate::crops::can_survive(w, p),
        // ---- crop farms (clk-misc) ----
        Block::Bamboo { .. } | Block::BambooSapling => crate::bamboo::can_survive(w, p),
        Block::Kelp { .. } | Block::KelpPlant => crate::kelp::can_survive(w, p),
        // ---- stem farms (clk-misc) ----
        Block::Stem { .. } | Block::AttachedStem { .. } => crate::stems::stem_can_survive(w, p),
        Block::Farmland { .. } => crate::stems::farmland_can_survive(w, p),
        // ---- cactus farms (clk-misc) ----
        Block::Cactus { .. } => crate::cactus::can_survive(w, p),
        Block::CactusFlower => crate::cactus::flower_can_survive(w, p),
        Block::TripwireHook { .. } => crate::react::hook::can_survive(w, s, p),
        Block::PlainRail { .. } | Block::DetectorRail { .. } => crate::rails::can_survive(w, p),
        Block::BigDripleaf { .. } => crate::react::dripleaf::can_survive(w, p),
        Block::WallBanner { facing } => is_solid(&w.get(p.rel(facing.opposite()))),
        // canSupportCenter of the block above (hanging) or below
        Block::Lantern { hanging } => {
            let d = if hanging { Dir::Up } else { Dir::Down };
            let q = p.rel(d);
            sturdy(w, &w.get(q), q, d.opposite(), Support::Center)
        }
        // ---- placed light sources (terrain lane) ----
        Block::SoulLantern { hanging } => {
            let d = if hanging { Dir::Up } else { Dir::Down };
            let q = p.rel(d);
            sturdy(w, &w.get(q), q, d.opposite(), Support::Center)
        }
        // TorchBlock / CandleBlock: canSupportCenter(below, UP)
        Block::PlainTorch { .. } | Block::Candle { .. } => {
            let q = p.below();
            sturdy(w, &w.get(q), q, Dir::Up, Support::Center)
        }
        // WallTorchBlock: the face of the block behind it, sturdy
        Block::PlainWallTorch { facing, .. } => {
            let q = p.rel(facing.opposite());
            sturdy(w, &w.get(q), q, facing, Support::Full)
        }
        // SeaPickleBlock.mayPlaceOn: the block below has a collision top or a sturdy top
        Block::SeaPickle { .. } => {
            let q = p.below();
            let b = w.get(q);
            collision_boxes(&b).iter().any(|bx| bx[3] == 16 && bx[0] < bx[1] && bx[4] < bx[5]) || sturdy(w, &b, q, Dir::Up, Support::Full)
        }
        Block::BubbleColumn { .. } => crate::fluid::bubble_can_survive(w, p),
        // ---- TNT dupers (track tnt) ----
        Block::CoralFan { .. } | Block::CoralWallFan { .. } => crate::tnt::fan_can_survive(w, s, p),
        // ---- fire (mobinfra) ----
        Block::Fire { .. } => crate::weather::fire::can_survive(w, s, p),
        _ => true,
    }
}

/// `neighborChanged`.
pub fn neighbor_changed(w: &mut World, s: Block, p: BlockPos, src: Kind, moved: bool) {
    match s {
        Block::Wire { .. } => wire::neighbor_changed(w, s, p, src, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::neighbor_changed(w, s, p, src, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::neighbor_changed(w, s, p),
        Block::Lamp { .. } => lamp::neighbor_changed(w, s, p),
        Block::Bulb { .. } => bulb::check_and_flip(w, s, p),
        Block::Note { .. } => note::neighbor_changed(w, s, p),
        Block::Trapdoor { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Skull { .. } | Block::WallSkull { .. } => indicator::neighbor_changed(w, s, p, src),
        Block::Rail { .. } => rail::neighbor_changed(w, s, p, moved),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::PlainRail { .. } | Block::DetectorRail { .. } => crate::rails::neighbor_changed(w, s, p, src, moved),
        Block::BigDripleaf { .. } => crate::react::dripleaf::neighbor_changed(w, s, p),
        Block::Dispenser { .. } | Block::Crafter { .. } => dispenser::neighbor_changed(w, s, p),
        Block::Hopper { .. } => hopper::check_powered_state(w, s, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::neighbor_changed(w, s, p, src, moved),
        Block::Water { .. } | Block::Lava { .. } => crate::fluid::liquid_on_place(w, s, p),
        // ---- TNT dupers (track tnt) ----
        Block::Tnt { .. } => crate::tnt::neighbor_changed(w, p),
        _ => {}
    }
}

/// `updateShape` (may schedule ticks: observers, leaves).
pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    match s {
        Block::Wire { .. } => wire::update_shape(w, s, p, d, np, ns),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::update_shape(w, s, p, d, np, ns),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::update_shape(w, s, p, d),
        Block::Lever { .. } | Block::Button { .. } => attached::update_shape(w, s, p, d),
        Block::Observer { .. } => observer::update_shape(w, s, p, d),
        Block::Note { .. } => note::update_shape(w, s, p, d),
        Block::Door { .. } => indicator::door_update_shape(w, s, p, d, ns),
        Block::FenceGate { .. } => indicator::gate_update_shape(s, d),
        // AbstractBedBlock.updateShape (the mob-AI lane): the half toward the other copies its
        // OCCUPIED (a sleeper's bed shows on both halves); without its other half, air
        Block::Bed { colour, facing, head, .. } => {
            let toward = if head { facing.opposite() } else { facing };
            if d == toward {
                match ns {
                    Block::Bed { colour: c, head: h, occupied, .. } if c == colour && h != head => Block::Bed { colour, facing, head, occupied },
                    _ => Block::Air,
                }
            } else {
                s
            }
        }
        Block::Leaves { .. } => leaves::update_shape(w, s, p, ns),
        Block::Scaffolding { .. } => {
            scaffolding::schedule(w, p);
            s
        }
        // CakeBlock: falls off a non-solid floor
        Block::Cake { .. } => {
            if d == Dir::Down && !is_solid(&w.get(p.below())) {
                Block::Air
            } else {
                s
            }
        }
        Block::Falling { .. } => crate::gravity::update_shape(w, s, p, d, np, ns), // pistmodel
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::update_shape(w, s, p, d, np, ns),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::update_shape(w, s, p, d),
        Block::Tripwire { .. } => crate::react::tripwire::update_shape(s, d, &ns),
        Block::TripwireHook { .. } => crate::react::hook::update_shape(w, s, p, d),
        Block::BigDripleaf { .. } => crate::react::dripleaf::update_shape(w, s, p, d, &ns),
        Block::WallBanner { facing } => {
            if d == facing.opposite() && !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        Block::Lantern { hanging } => {
            if d == (if hanging { Dir::Up } else { Dir::Down }) && !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        // ---- natural speleothems (terrain lane): SpeleothemBlock.updateShape ----
        Block::Terrain(id) if matches!(crate::terrain::blocks::props(id).block_id(), "minecraft:pointed_dripstone" | "minecraft:sulfur_spike") => crate::terrain::ticks::speleothem_update_shape(w, s, p, d),
        // ---- natural plants (terrain lane): VegetationBlock, DoublePlantBlock, CarpetBlock... ----
        Block::Terrain(id) if crate::terrain::plants::plant(id).is_some() => crate::terrain::plants::update_shape(w, s, id, p, d, ns),
        // ---- placed light sources (terrain lane) ----
        Block::SoulLantern { hanging } => {
            if d == (if hanging { Dir::Up } else { Dir::Down }) && !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        Block::PlainTorch { .. } | Block::Candle { .. } => {
            if d == Dir::Down && !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        Block::PlainWallTorch { facing, .. } => {
            if d == facing.opposite() && !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        Block::SeaPickle { .. } => {
            if !can_survive(w, &s, p) {
                Block::Air
            } else {
                s
            }
        }
        // CampfireBlock: a hay bale below makes it a signal fire (`isSmokeSource`)
        Block::Campfire { soul, lit, facing, .. } if d == Dir::Down => Block::Campfire { soul, lit, facing, signal_fire: ns.to_string().starts_with("minecraft:hay_block") },
        Block::Water { .. } | Block::Lava { .. } => crate::fluid::liquid_update_shape(w, s, p, d, ns),
        Block::BubbleColumn { .. } => crate::fluid::bubble_update_shape(w, s, p, d, ns),
        // ---- TNT dupers (track tnt) ----
        Block::CoralFan { .. } | Block::CoralWallFan { .. } => crate::tnt::fan_update_shape(w, s, p, d),
        // ---- fences (clk-misc) ----
        Block::Fence { .. } => crate::fence::update_shape(w, s, p, d, np, ns),
        // ---- crop farms (webrender): a cane that cannot stand breaks next tick; the grate's
        // water ticks ----
        Block::SugarCane { .. } => crate::crops::update_shape(w, s, p),
        Block::WaterloggedGrate => {
            crate::fluid::waterlogged_update_shape(w, &s, p);
            s
        }
        // ---- the slab family (terrain lane): SlabBlock.updateShape (a waterlogged slab's water
        // ticks) ----
        Block::AnySlab { .. } => {
            crate::fluid::waterlogged_update_shape(w, &s, p);
            s
        }
        // ---- crop farms (clk-misc) ----
        Block::Bamboo { .. } => crate::bamboo::update_shape(w, s, p, d, ns),
        Block::BambooSapling => crate::bamboo::sapling_update_shape(w, s, p, d, ns),
        Block::Kelp { .. } => crate::kelp::head_update_shape(w, s, p, d, ns),
        Block::KelpPlant => crate::kelp::body_update_shape(w, s, p, d, ns),
        // ---- stem farms (clk-misc) ----
        Block::Farmland { .. } => crate::stems::farmland_update_shape(w, s, p, d),
        Block::Stem { .. } => crate::stems::stem_update_shape(w, s, p),
        Block::AttachedStem { .. } => crate::stems::attached_update_shape(w, s, p, d, ns),
        // ---- cactus farms (clk-misc) ----
        Block::Cactus { .. } => crate::cactus::update_shape(w, s, p),
        Block::CactusFlower => crate::cactus::flower_update_shape(w, s, p),
        // ---- fire (mobinfra) ----
        Block::Fire { .. } => crate::weather::fire::update_shape(w, s, p),
        // ---- any other natural state: a named model error when it would react (terrain lane) ----
        Block::Terrain(id) => {
            crate::terrain::plants::unmodeled_update(w, id, p, d, &ns);
            s
        }
        _ => s,
    }
}

/// `Block.updateFromNeighbourShapes`.
pub fn update_from_neighbour_shapes(w: &mut World, s: Block, p: BlockPos) -> Block {
    let mut st = s;
    for d in Dir::SHAPE_ORDER {
        let np = p.rel(d);
        let ns = w.get(np);
        st = update_shape(w, st, p, d, np, ns);
    }
    st
}

/// `onPlace`.
pub fn on_place(w: &mut World, s: Block, p: BlockPos, old: Block, moved: bool) {
    match s {
        Block::Wire { .. } => wire::on_place(w, s, p, old, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::on_place(w, s, p, old, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::on_place(w, s, p),
        Block::Observer { .. } => observer::on_place(w, s, p, old),
        Block::Bulb { .. } => bulb::on_place(w, s, p, old),
        Block::Target { .. } => target::on_place(w, s, p, old),
        Block::Rail { .. } => rail::on_place(w, s, p, old, moved),
        Block::Hopper { .. } => hopper::on_place(w, s, p, old),
        Block::Composter { .. } => composter::on_place(w, s, p),
        Block::Scaffolding { .. } => scaffolding::schedule(w, p),
        Block::Falling { .. } => crate::gravity::on_place(w, s, p, old, moved), // pistmodel
        Block::Sculk { .. } => crate::sculk::on_place(w, s, p, old),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::on_place(w, s, p, old, moved),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Tripwire { .. } => crate::react::tripwire::on_place(w, s, p, old),
        Block::PlainRail { .. } | Block::DetectorRail { .. } => crate::rails::on_place(w, s, p, old, moved),
        Block::Water { .. } | Block::Lava { .. } => crate::fluid::liquid_on_place(w, s, p),
        // ---- TNT dupers (track tnt) ----
        Block::Tnt { .. } => crate::tnt::on_place(w, p, old),
        // ---- weather (mobinfra) ----
        Block::LightningRod { .. } => crate::weather::lightning::rod_on_place(w, s, p, old),
        Block::Fire { .. } => crate::weather::fire::on_place(w, s, p, old),
        _ => {}
    }
}

/// `affectNeighborsAfterRemoval`.
pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
    match s {
        Block::Wire { .. } => wire::affect_neighbors_after_removal(w, s, p, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::affect_neighbors_after_removal(w, s, p, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::affect_neighbors_after_removal(w, s, p, moved),
        Block::Lever { .. } | Block::Button { .. } => attached::affect_neighbors_after_removal(w, s, p, moved),
        Block::Observer { .. } => observer::affect_neighbors_after_removal(w, s, p, moved),
        Block::Rail { .. } => rail::affect_neighbors_after_removal(w, s, p, moved),
        Block::Dispenser { .. } | Block::Crafter { .. } | Block::Hopper { .. } => dispenser::affect_neighbors_after_removal(w, s, p),
        Block::Sculk { .. } => crate::sculk::affect_neighbors_after_removal(w, s, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::affect_neighbors_after_removal(w, s, p, moved),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::affect_neighbors_after_removal(w, s, p, moved),
        Block::Tripwire { .. } => crate::react::tripwire::affect_neighbors_after_removal(w, s, p, moved),
        Block::TripwireHook { .. } => crate::react::hook::affect_neighbors_after_removal(w, s, p, moved),
        // Containers.updateNeighboursAfterDestroy
        Block::ChiseledBookshelf { .. } => w.update_neighbour_for_output_signal(p, Kind::ChiseledBookshelf),
        Block::Jukebox { .. } => w.update_neighbour_for_output_signal(p, Kind::Jukebox),
        // ---- shulker study (clk-rt): Containers.updateNeighboursAfterDestroy ----
        Block::ShulkerBox { .. } | Block::Chest { .. } => w.update_neighbour_for_output_signal(p, s.kind()),
        // ---- smelter study (track sorter): AbstractFurnaceBlock -> Containers.updateNeighboursAfterDestroy ----
        Block::Furnace { .. } => w.update_neighbour_for_output_signal(p, Kind::Furnace),
        Block::BrewingStand { .. } => w.update_neighbour_for_output_signal(p, Kind::BrewingStand),
        Block::PlainRail { .. } | Block::DetectorRail { .. } => crate::rails::affect_neighbors_after_removal(w, s, p, moved),
        // ---- weather (mobinfra) ----
        Block::LightningRod { .. } => crate::weather::lightning::rod_after_removal(w, s, p),
        _ => {}
    }
}

/// `updateIndirectNeighbourShapes` (only dust has one).
pub fn update_indirect_neighbour_shapes(w: &mut World, s: Block, p: BlockPos, fl: u32, limit: i32) {
    if let Block::Wire { .. } = s {
        wire::update_indirect_neighbour_shapes(w, s, p, fl, limit);
    }
}

/// A scheduled tick (`tick`).
pub fn tick(w: &mut World, s: Block, p: BlockPos) {
    match s {
        Block::Repeater { .. } | Block::Comparator { .. } => diode::tick(w, s, p),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::tick(w, s, p),
        Block::Button { .. } => button::tick(w, s, p),
        Block::Observer { .. } => observer::tick(w, s, p),
        Block::Lamp { .. } => lamp::tick(w, s, p),
        Block::Target { .. } => target::tick(w, s, p),
        Block::Leaves { .. } => leaves::tick(w, s, p),
        Block::Scaffolding { .. } => scaffolding::tick(w, s, p),
        Block::Composter { .. } => composter::tick(w, s, p),
        Block::Falling { .. } => crate::gravity::tick(w, s, p), // pistmodel
        Block::Sculk { .. } => crate::sculk::tick(w, s, p),
        // Droppers push one item into a facing container; dispensers with items spawn entities
        // (noted); crafters craft their grid's recipe (container.rs).
        Block::Dispenser { dropper: true, .. } => crate::redstone::container::dropper_dispense(w, p),
        Block::Dispenser { dropper: false, .. } => crate::redstone::container::dispenser_dispense(w, p),
        Block::Crafter { .. } => crate::redstone::container::crafter_dispense(w, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::tick(w, s, p),
        // ---- entity-reacting blocks (clk-misc) ----
        Block::Plate { .. } => crate::react::plate::tick(w, s, p),
        Block::Tripwire { .. } => crate::react::tripwire::tick(w, p),
        Block::TripwireHook { .. } => crate::react::hook::tick(w, s, p),
        Block::DetectorRail { .. } => crate::rails::detector::tick(w, s, p),
        Block::BigDripleaf { .. } => crate::react::dripleaf::tick(w, s, p),
        Block::Water { .. } => crate::fluid::liquid_tick(w, s, p),
        Block::BubbleColumn { .. } => crate::fluid::bubble_tick(w, s, p),
        // ---- crop farms (webrender) ----
        Block::SugarCane { .. } => crate::crops::tick(w, p),
        // ---- crop farms (clk-misc) ----
        Block::Bamboo { .. } => crate::bamboo::tick(w, p),
        Block::Kelp { .. } | Block::KelpPlant => crate::kelp::tick(w, p),
        // ---- stem farms (clk-misc) ----
        Block::Farmland { .. } => crate::stems::farmland_tick(w, p),
        Block::Cactus { .. } => crate::cactus::tick(w, p),
        // ---- weather (mobinfra): a struck rod turns off; a fire's tick ----
        Block::LightningRod { .. } => crate::weather::lightning::rod_tick(w, s, p),
        Block::Fire { .. } => crate::weather::fire::tick(w, s, p),
        _ => {}
    }
}

/// A block event (`triggerEvent`).
pub fn trigger_event(w: &mut World, s: Block, p: BlockPos, a: i32, b: i32) -> bool {
    match s {
        // Note blocks: the sound only (`playSeededSound` takes `level.getRandom().nextLong()`; a
        // custom-head note block without a head sound returns before it: redsim's player heads
        // carry none)
        Block::Note { instrument, .. } => {
            if instrument == crate::block::Instrument::CustomHead {
                return false;
            }
            if let Some(r) = w.rng() {
                r.next_long();
            }
            true
        }
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::trigger_event(w, s, p, a, b),
        _ => false,
    }
}

/// Does a lever/button at this state hang on `support`?
pub fn attached_to(s: &Block, p: BlockPos, support: BlockPos) -> bool {
    match *s {
        Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => p.rel(Face::connected(face, facing).opposite()) == support,
        _ => false,
    }
}
