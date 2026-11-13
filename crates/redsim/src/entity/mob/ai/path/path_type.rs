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
        Block::Fire { .. } | Block::Simple(Simple::MagmaBlock) | Block::Campfire { lit: true, .. } => PathType::Fire,
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
        | Block::Pane { .. }
        | Block::Slab { .. }
        | Block::StoneSlab { .. }
        | Block::AnySlab { .. }
        | Block::SoulLantern { .. }
        | Block::Campfire { .. }
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
        // (tickexact 2026-11-11, cart-pole) RodBlock.isPathfindable is false (an end rod blocks a
        // path), and so are PistonBaseBlock's (an extended base too), PistonHeadBlock's and
        // MovingPistonBlock's; a lightning rod is a RodBlock too (G1's N40)
        Block::EndRod { .. } | Block::LightningRod { .. } | Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => false,
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

    /// G1's N40: a lightning rod is a RodBlock, not land-pathfindable: a one-wide corridor blocked by one
    /// (standing on the floor, a stone block over it, a 3-high stone wall each side) reads unreached; with the
    /// rod gone, reached.
    #[test]
    fn a_lightning_rod_blocks_a_corridor() {
        use crate::pos::BlockPos;
        use crate::world::World;
        let rod = Block::parse("minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]").unwrap();
        assert!(!pathfindable_land(&rod), "a rod blocks a path");
        for blocked in [true, false] {
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            let stone = Block::parse("minecraft:stone").unwrap();
            for x in -6..=6 {
                for z in -6..=6 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), crate::world::flags::ALL);
                }
            }
            for x in -5..=5 {
                for y in 0..=2 {
                    w.set_block(BlockPos::new(x, y, -1), stone.clone(), crate::world::flags::ALL);
                    w.set_block(BlockPos::new(x, y, 1), stone.clone(), crate::world::flags::ALL);
                }
            }
            for y in 0..=2 {
                w.set_block(BlockPos::new(-6, y, 0), stone.clone(), crate::world::flags::ALL);
                w.set_block(BlockPos::new(6, y, 0), stone.clone(), crate::world::flags::ALL);
            }
            if blocked {
                w.set_block(BlockPos::new(0, 0, 0), rod.clone(), crate::world::flags::ALL);
                w.set_block(BlockPos::new(0, 1, 0), stone.clone(), crate::world::flags::ALL);
            }
            w.mob.seed(7, 6_000);
            w.mob.ai.on = true;
            let v = crate::entity::spawn(&mut w, "minecraft:villager", [-4.5, 0.0, 0.5], None).unwrap();
            // (ticks to land: a mob off the ground and no passenger can't update a path, canUpdatePath; a
            // fresh mob's first move is by its zero motion, the gravity reaching the move from its second tick)
            for _ in 0..3 {
                w.tick();
            }
            let i = crate::entity::index_of(&w, v).unwrap();
            assert!(w.entities.list[i].on_ground, "the villager landed");
            let path = crate::entity::mob::ai::with_nav(&mut w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, BlockPos::new(4, 0, 0), 0, arena));
            assert!(path.is_some(), "a path computed (blocked {blocked})");
            let reached = path.as_ref().is_some_and(|p| p.reached);
            assert_eq!(reached, !blocked, "the corridor {} the rod: reached {reached}", if blocked { "blocked by" } else { "without" });
        }
    }

    /// ironspec 07:47, tick-exact's citation (WalkNodeEvaluator.getPathTypeWithinMobBB 445-448: a RAIL cell
    /// of the body reads UNPASSABLE_RAIL unless the mob's own block or the one below it is a RAIL;
    /// getPathTypeOfMob 393-394: UNPASSABLE_RAIL outright when any body cell has it, malus -1): a villager on
    /// stone can't path across a rail line that spans the floor (unreached), while the same villager standing
    /// on a rail of the line's own kind paths across (reached); the rail cell's own type is RAIL either way.
    #[test]
    fn a_rail_is_unpassable_from_off_the_rails() {
        use crate::pos::BlockPos;
        use crate::world::World;
        assert_eq!(from_state(&Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap()), PathType::Rail);
        for on_rail in [false, true] {
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            let stone = Block::parse("minecraft:stone").unwrap();
            let rail = Block::parse("minecraft:rail[shape=north_south,waterlogged=false]").unwrap();
            for x in -6..=6 {
                for z in -6..=6 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), crate::world::flags::ALL);
                }
            }
            // (the line x 0 across the whole floor, z -6..6: no way round it; the void beyond)
            for z in -6..=6 {
                w.set_block(BlockPos::new(0, 0, z), rail.clone(), crate::world::flags::ALL);
            }
            if on_rail {
                w.set_block(BlockPos::new(-4, 0, 0), rail.clone(), crate::world::flags::ALL);
            }
            w.mob.seed(7, 6_000);
            w.mob.ai.on = true;
            let v = crate::entity::spawn(&mut w, "minecraft:villager", [-3.5, 0.0, 0.5], None).unwrap();
            for _ in 0..3 {
                w.tick();
            }
            let i = crate::entity::index_of(&w, v).unwrap();
            let e = &w.entities.list[i];
            assert!(e.on_ground, "the villager landed");
            let own = w.get(e.block_position());
            assert_eq!(matches!(own, Block::PlainRail { .. } | Block::Rail { .. }), on_rail, "its own block: {own}");
            let path = crate::entity::mob::ai::with_nav(&mut w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, BlockPos::new(4, 0, 0), 0, arena));
            assert!(path.is_some(), "a path computed (on a rail {on_rail})");
            let reached = path.as_ref().is_some_and(|p| p.reached);
            assert_eq!(reached, on_rail, "across the rail line from {}: reached {reached}", if on_rail { "a rail" } else { "stone" });
        }
    }
}
