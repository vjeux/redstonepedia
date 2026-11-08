//! `PathType` and `WalkNodeEvaluator.getPathTypeFromState` (26.3, `net.minecraft.world.level.
//! pathfinder`): what a single block state means to a walking mob, before its neighbours and the
//! mob's own box are looked at (`walk.rs`).

use crate::block::{Block, Simple};
use crate::terrain::blocks::{self, FluidKind, Props};
use std::sync::OnceLock;

/// `PathType`, in its enum order (the order of an `EnumSet` of path types).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum PathType {
    Blocked,
    Open,
    Walkable,
    WalkableDoor,
    Trapdoor,
    PowderSnow,
    OnTopOfPowderSnow,
    Fence,
    Lava,
    Water,
    WaterBorder,
    Rail,
    UnpassableRail,
    FireInNeighbor,
    Fire,
    DamagingInNeighbor,
    Damaging,
    DoorOpen,
    DoorWoodClosed,
    DoorIronClosed,
    Breach,
    Leaves,
    StickyHoney,
    Cocoa,
    DamageCautious,
    OnTopOfTrapdoor,
    BigMobsCloseToDanger,
}

/// The number of path types.
pub const COUNT: usize = 27;

impl PathType {
    pub const ALL: [PathType; COUNT] = [
        PathType::Blocked,
        PathType::Open,
        PathType::Walkable,
        PathType::WalkableDoor,
        PathType::Trapdoor,
        PathType::PowderSnow,
        PathType::OnTopOfPowderSnow,
        PathType::Fence,
        PathType::Lava,
        PathType::Water,
        PathType::WaterBorder,
        PathType::Rail,
        PathType::UnpassableRail,
        PathType::FireInNeighbor,
        PathType::Fire,
        PathType::DamagingInNeighbor,
        PathType::Damaging,
        PathType::DoorOpen,
        PathType::DoorWoodClosed,
        PathType::DoorIronClosed,
        PathType::Breach,
        PathType::Leaves,
        PathType::StickyHoney,
        PathType::Cocoa,
        PathType::DamageCautious,
        PathType::OnTopOfTrapdoor,
        PathType::BigMobsCloseToDanger,
    ];

    /// `PathType.getMalus()` (the default cost a mob without an override pays).
    pub fn default_malus(self) -> f32 {
        match self {
            PathType::Blocked => -1.0,
            PathType::Open | PathType::Walkable | PathType::WalkableDoor | PathType::Trapdoor => 0.0,
            PathType::PowderSnow => -1.0,
            PathType::OnTopOfPowderSnow => 0.0,
            PathType::Fence | PathType::Lava => -1.0,
            PathType::Water | PathType::WaterBorder => 8.0,
            PathType::Rail => 0.0,
            PathType::UnpassableRail => -1.0,
            PathType::FireInNeighbor => 8.0,
            PathType::Fire => 16.0,
            PathType::DamagingInNeighbor => 8.0,
            PathType::Damaging => -1.0,
            PathType::DoorOpen => 0.0,
            PathType::DoorWoodClosed | PathType::DoorIronClosed => -1.0,
            PathType::Breach => 4.0,
            PathType::Leaves => -1.0,
            PathType::StickyHoney => 8.0,
            PathType::Cocoa | PathType::DamageCautious | PathType::OnTopOfTrapdoor => 0.0,
            PathType::BigMobsCloseToDanger => 4.0,
        }
    }

    /// The game's enum constant name (traces, the path probe).
    pub fn name(self) -> &'static str {
        match self {
            PathType::Blocked => "BLOCKED",
            PathType::Open => "OPEN",
            PathType::Walkable => "WALKABLE",
            PathType::WalkableDoor => "WALKABLE_DOOR",
            PathType::Trapdoor => "TRAPDOOR",
            PathType::PowderSnow => "POWDER_SNOW",
            PathType::OnTopOfPowderSnow => "ON_TOP_OF_POWDER_SNOW",
            PathType::Fence => "FENCE",
            PathType::Lava => "LAVA",
            PathType::Water => "WATER",
            PathType::WaterBorder => "WATER_BORDER",
            PathType::Rail => "RAIL",
            PathType::UnpassableRail => "UNPASSABLE_RAIL",
            PathType::FireInNeighbor => "FIRE_IN_NEIGHBOR",
            PathType::Fire => "FIRE",
            PathType::DamagingInNeighbor => "DAMAGING_IN_NEIGHBOR",
            PathType::Damaging => "DAMAGING",
            PathType::DoorOpen => "DOOR_OPEN",
            PathType::DoorWoodClosed => "DOOR_WOOD_CLOSED",
            PathType::DoorIronClosed => "DOOR_IRON_CLOSED",
            PathType::Breach => "BREACH",
            PathType::Leaves => "LEAVES",
            PathType::StickyHoney => "STICKY_HONEY",
            PathType::Cocoa => "COCOA",
            PathType::DamageCautious => "DAMAGE_CAUTIOUS",
            PathType::OnTopOfTrapdoor => "ON_TOP_OF_TRAPDOOR",
            PathType::BigMobsCloseToDanger => "BIG_MOBS_CLOSE_TO_DANGER",
        }
    }

    pub fn from_name(s: &str) -> Option<PathType> {
        PathType::ALL.iter().copied().find(|t| t.name() == s)
    }
}

/// A mob's path costs (`Mob.getPathfindingMalus`: its `pathfindingMalus` overrides, else the
/// type's default).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Malus(pub [f32; COUNT]);

impl Default for Malus {
    fn default() -> Malus {
        let mut m = [0.0f32; COUNT];
        for t in PathType::ALL {
            m[t as usize] = t.default_malus();
        }
        Malus(m)
    }
}

impl Malus {
    #[inline]
    pub fn get(&self, t: PathType) -> f32 {
        self.0[t as usize]
    }

    /// `Mob.setPathfindingMalus`.
    pub fn set(&mut self, t: PathType, v: f32) {
        self.0[t as usize] = v;
    }

    /// `AbstractVillager`'s constructor: FIRE_IN_NEIGHBOR 16, FIRE -1.
    pub fn villager() -> Malus {
        let mut m = Malus::default();
        m.set(PathType::FireInNeighbor, 16.0);
        m.set(PathType::Fire, -1.0);
        m
    }
}

/// `WalkNodeEvaluator.getPathTypeFromState(level, pos)` of a block state.
pub fn from_state(b: &Block) -> PathType {
    match *b {
        Block::Air => PathType::Open,
        Block::Terrain(id) => terrain_type(id),
        // BlockTags.TRAPDOORS (iron and wooden), Blocks.BIG_DRIPLEAF (lily pads: terrain only)
        Block::Trapdoor { .. } | Block::BigDripleaf { .. } => PathType::Trapdoor,
        Block::PowderSnow => PathType::PowderSnow,
        // cactus (sweet berry bushes: terrain only)
        Block::Cactus { .. } => PathType::Damaging,
        Block::Simple(Simple::HoneyBlock) => PathType::StickyHoney,
        Block::Lava { .. } => PathType::Lava,
        // isBurningBlock: BlockTags.FIRE, lava, magma, a lit campfire, a lava cauldron
        Block::Simple(Simple::MagmaBlock) => PathType::Fire,
        Block::Door { open: true, .. } => PathType::DoorOpen,
        // door.type().canOpenByHand(): every wooden door, not iron
        Block::Door { iron, .. } => {
            if iron {
                PathType::DoorIronClosed
            } else {
                PathType::DoorWoodClosed
            }
        }
        // BaseRailBlock
        Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } => PathType::Rail,
        // LeavesBlock
        Block::Leaves { .. } => PathType::Leaves,
        // BlockTags.FENCES, BlockTags.WALLS, a closed FenceGateBlock
        Block::Fence { .. } | Block::FenceGate { open: false, .. } => PathType::Fence,
        _ => {
            if !pathfindable_land(b) {
                PathType::Blocked
            } else if crate::fluid::fluid_state_of(b).fluid.is_water() {
                PathType::Water
            } else {
                PathType::Open
            }
        }
    }
}

/// `BlockBehaviour.isPathfindable(state, LAND)` of redsim's own blocks: the default (not a full
/// collision cube) unless the block's class overrides it to false (beds, cactus, fences, slabs,
/// stairs, soul sand, mud, farmland, hoppers, chests, lanterns, end portal frames, composters,
/// cakes, skulls, brewing stands, respawn anchors, sculk sensors, bamboo) or to its `open` state
/// (doors, trapdoors, fence gates); lava's `LiquidBlock` says no, water's yes.
pub fn pathfindable_land(b: &Block) -> bool {
    match *b {
        Block::Terrain(id) => blocks::props(id).path[0],
        Block::Bed { .. }
        | Block::Cactus { .. }
        | Block::Fence { .. }
        | Block::Slab { .. }
        | Block::StoneSlab { .. }
        | Block::Simple(Simple::SoulSand)
        | Block::Simple(Simple::Mud)
        | Block::Farmland { .. }
        | Block::Hopper { .. }
        | Block::Chest { .. }
        | Block::Lantern { .. }
        | Block::EndPortalFrame { .. }
        | Block::Composter { .. }
        | Block::Cake { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. }
        | Block::BrewingStand { .. }
        | Block::RespawnAnchor { .. }
        | Block::Sculk { .. }
        | Block::Bamboo { .. }
        | Block::Lava { .. } => false,
        Block::Door { open, .. } | Block::Trapdoor { open, .. } | Block::FenceGate { open, .. } => open,
        Block::PowderSnow => true,
        _ => !crate::behavior::full_cube(b),
    }
}

/// A natural state's `getPathTypeFromState`, from its row (classes, tags, fluid, `isPathfindable`).
fn terrain_type(id: u16) -> PathType {
    static TABLE: OnceLock<Vec<PathType>> = OnceLock::new();
    let t = TABLE.get_or_init(|| blocks::table().props.iter().map(terrain_row_type).collect());
    t[id as usize]
}

fn has_class(p: &Props, c: &str) -> bool {
    p.classes.iter().any(|x| x == c)
}

fn prop_true(p: &Props, key: &str) -> bool {
    p.name.contains(&format!("{key}=true"))
}

/// `getPathTypeFromState` read from a natural-state row.
pub fn terrain_row_type(p: &Props) -> PathType {
    if p.air {
        return PathType::Open;
    }
    let id = p.block_id();
    if p.has_tag("minecraft:trapdoors") || has_class(p, "TrapDoorBlock") || id == "minecraft:lily_pad" || id == "minecraft:big_dripleaf" {
        return PathType::Trapdoor;
    }
    if id == "minecraft:powder_snow" {
        return PathType::PowderSnow;
    }
    if id == "minecraft:cactus" || id == "minecraft:sweet_berry_bush" {
        return PathType::Damaging;
    }
    if id == "minecraft:honey_block" {
        return PathType::StickyHoney;
    }
    if id == "minecraft:cocoa" {
        return PathType::Cocoa;
    }
    if id == "minecraft:wither_rose" || p.has_tag("minecraft:speleothems") || has_class(p, "SpeleothemBlock") || has_class(p, "PointedDripstoneBlock") {
        return PathType::DamageCautious;
    }
    if p.fluid == FluidKind::Lava {
        return PathType::Lava;
    }
    let burning = p.has_tag("minecraft:fire")
        || has_class(p, "BaseFireBlock")
        || id == "minecraft:lava"
        || id == "minecraft:magma_block"
        || (has_class(p, "CampfireBlock") && prop_true(p, "lit"))
        || id == "minecraft:lava_cauldron";
    if burning {
        return PathType::Fire;
    }
    if has_class(p, "DoorBlock") {
        return if prop_true(p, "open") {
            PathType::DoorOpen
        } else if id == "minecraft:iron_door" {
            PathType::DoorIronClosed
        } else {
            PathType::DoorWoodClosed
        };
    }
    if has_class(p, "BaseRailBlock") {
        return PathType::Rail;
    }
    if has_class(p, "LeavesBlock") {
        return PathType::Leaves;
    }
    let fence = p.has_tag("minecraft:fences") || p.has_tag("minecraft:walls") || has_class(p, "WallBlock") || (has_class(p, "FenceGateBlock") && !prop_true(p, "open"));
    if fence {
        return PathType::Fence;
    }
    if !p.path[0] {
        return PathType::Blocked;
    }
    if p.fluid == FluidKind::Water { PathType::Water } else { PathType::Open }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_malus_and_order() {
        assert_eq!(PathType::ALL.len(), COUNT);
        for (k, t) in PathType::ALL.iter().enumerate() {
            assert_eq!(*t as usize, k);
        }
        assert_eq!(PathType::Water.default_malus(), 8.0);
        assert_eq!(Malus::villager().get(PathType::Fire), -1.0);
    }

    #[test]
    fn redsim_blocks() {
        use crate::pos::Dir;
        assert_eq!(from_state(&Block::Air), PathType::Open);
        assert_eq!(from_state(&Block::Simple(Simple::Stone)), PathType::Blocked);
        assert_eq!(from_state(&Block::Water { level: 0 }), PathType::Water);
        assert_eq!(from_state(&Block::Lava { level: 0 }), PathType::Lava);
        assert_eq!(from_state(&Block::Door { iron: false, facing: Dir::North, upper: false, hinge_right: false, open: false, powered: false }), PathType::DoorWoodClosed);
        assert_eq!(from_state(&Block::Door { iron: true, facing: Dir::North, upper: false, hinge_right: false, open: false, powered: false }), PathType::DoorIronClosed);
        assert_eq!(from_state(&Block::FenceGate { facing: Dir::North, in_wall: false, open: true, powered: false }), PathType::Open);
        assert_eq!(from_state(&Block::Slab { kind: 0 }), PathType::Blocked);
        assert_eq!(from_state(&Block::Wire { north: crate::block::Side::None, east: crate::block::Side::None, south: crate::block::Side::None, west: crate::block::Side::None, power: 0 }), PathType::Open);
    }
}
