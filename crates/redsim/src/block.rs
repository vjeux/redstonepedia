//! Block states: the [`Block`] enum (one variant per block family, the state's properties as
//! fields), its block type ([`Kind`], the game's `Block` identity used by `state.is(block)`,
//! scheduled ticks and block events), and the conversion from/to `minecraft:name[props]` strings
//! (properties sorted by name, like the game prints them).

use crate::pos::Dir;
use std::fmt;

/// `RedstoneSide`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Side {
    None,
    Side,
    Up,
}
impl Side {
    pub fn connected(self) -> bool {
        self != Side::None
    }
    fn name(self) -> &'static str {
        match self {
            Side::None => "none",
            Side::Side => "side",
            Side::Up => "up",
        }
    }
    fn parse(s: &str) -> Option<Side> {
        Some(match s {
            "none" => Side::None,
            "side" => Side::Side,
            "up" => Side::Up,
            _ => return None,
        })
    }
}

/// `AttachFace` (levers, buttons).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    Floor,
    Wall,
    Ceiling,
}
impl Face {
    fn name(self) -> &'static str {
        match self {
            Face::Floor => "floor",
            Face::Wall => "wall",
            Face::Ceiling => "ceiling",
        }
    }
    fn parse(s: &str) -> Option<Face> {
        Some(match s {
            "floor" => Face::Floor,
            "wall" => Face::Wall,
            "ceiling" => Face::Ceiling,
            _ => return None,
        })
    }
    /// `FaceAttachedHorizontalDirectionalBlock.getConnectedDirection`: from the block toward the
    /// side opposite its support... (the game's "connected direction": the support is at
    /// `pos.relative(connected.getOpposite())`).
    pub fn connected(self, facing: Dir) -> Dir {
        match self {
            Face::Ceiling => Dir::Down,
            Face::Floor => Dir::Up,
            Face::Wall => facing,
        }
    }
}

/// `NoteBlockInstrument`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Instrument {
    Harp,
    Basedrum,
    Snare,
    Hat,
    Bass,
    Flute,
    Bell,
    Guitar,
    Chime,
    Xylophone,
    IronXylophone,
    CowBell,
    Didgeridoo,
    Bit,
    Banjo,
    Pling,
    Trumpet,
    TrumpetExposed,
    TrumpetWeathered,
    TrumpetOxidized,
    Zombie,
    Skeleton,
    Creeper,
    Dragon,
    WitherSkeleton,
    Piglin,
    CustomHead,
}
const INSTRUMENTS: [(Instrument, &str); 27] = [
    (Instrument::Harp, "harp"),
    (Instrument::Basedrum, "basedrum"),
    (Instrument::Snare, "snare"),
    (Instrument::Hat, "hat"),
    (Instrument::Bass, "bass"),
    (Instrument::Flute, "flute"),
    (Instrument::Bell, "bell"),
    (Instrument::Guitar, "guitar"),
    (Instrument::Chime, "chime"),
    (Instrument::Xylophone, "xylophone"),
    (Instrument::IronXylophone, "iron_xylophone"),
    (Instrument::CowBell, "cow_bell"),
    (Instrument::Didgeridoo, "didgeridoo"),
    (Instrument::Bit, "bit"),
    (Instrument::Banjo, "banjo"),
    (Instrument::Pling, "pling"),
    (Instrument::Trumpet, "trumpet"),
    (Instrument::TrumpetExposed, "trumpet_exposed"),
    (Instrument::TrumpetWeathered, "trumpet_weathered"),
    (Instrument::TrumpetOxidized, "trumpet_oxidized"),
    (Instrument::Zombie, "zombie"),
    (Instrument::Skeleton, "skeleton"),
    (Instrument::Creeper, "creeper"),
    (Instrument::Dragon, "dragon"),
    (Instrument::WitherSkeleton, "wither_skeleton"),
    (Instrument::Piglin, "piglin"),
    (Instrument::CustomHead, "custom_head"),
];
impl Instrument {
    fn name(self) -> &'static str {
        INSTRUMENTS.iter().find(|(i, _)| *i == self).map(|(_, n)| *n).unwrap()
    }
    fn parse(s: &str) -> Option<Instrument> {
        INSTRUMENTS.iter().find(|(_, n)| *n == s).map(|(i, _)| *i)
    }
    /// `NoteBlockInstrument.worksAboveNoteBlock`: mob heads (and custom heads).
    pub fn works_above_note_block(self) -> bool {
        matches!(
            self,
            Instrument::Zombie | Instrument::Skeleton | Instrument::Creeper | Instrument::Dragon | Instrument::WitherSkeleton | Instrument::Piglin | Instrument::CustomHead
        )
    }
}

/// `PushReaction`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PushReaction {
    PushPull,
    Push,
    Popped,
    Immoveable,
    IgnoreEntity,
}

/// Plain blocks without redstone behavior (their only roles: conducting or not, holding things
/// up, note block instruments, piston push reactions).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Simple {
    Stone,
    SmoothStone,
    Cobblestone,
    Obsidian,
    Glass,
    TintedGlass,
    WhiteWool,
    OakPlanks,
    GoldBlock,
    IronBlock,
    Clay,
    EmeraldBlock,
    Glowstone,
    PackedIce,
    SoulSand,
    SlimeBlock,
    HoneyBlock,
    WaxedCopperGrate,
    SeaLantern,
    // plain blocks for the equivalence classes (`classes.rs`)
    Granite,
    PolishedGranite,
    Diorite,
    Andesite,
    MossyCobblestone,
    StoneBricks,
    Tuff,
    Calcite,
    Bricks,
    Sandstone,
    NetherBricks,
    EndStone,
    Blackstone,
    QuartzBlock,
    Prismarine,
    PurpurBlock,
    Terracotta,
    CoalBlock,
    RawIronBlock,
    MudBricks,
    WhiteConcrete,
    BlackConcrete,
    DiamondBlock,
    LapisBlock,
    NetheriteBlock,
    SoulSoil,
    DriedKelpBlock,
    Shroomlight,
    NetherWartBlock,
    WarpedWartBlock,
    AmethystBlock,
    Bookshelf,
    CraftingTable,
    SprucePlanks,
    OrangeWool,
    BlackWool,
    Pumpkin,
    WhiteStainedGlass,
    BlackStainedGlass,
    Mud,
    // ---- pistmodel ----
    CryingObsidian,
    // ---- entity-reacting blocks (clk-misc) ----
    Dirt,
    // ---- the TNT judge's ground (fly): a superflat floor, grass over dirt over bedrock ----
    GrassBlock,
    Bedrock,
    // ---- fluids (clockinfra): what turns water above into a bubble column ----
    MagmaBlock,
    // ---- the other wool colours (clk-misc, coverage: community-7x8-48b's cyan) ----
    MagentaWool,
    LightBlueWool,
    YellowWool,
    LimeWool,
    PinkWool,
    GrayWool,
    LightGrayWool,
    CyanWool,
    PurpleWool,
    BlueWool,
    BrownWool,
    GreenWool,
    RedWool,
    // ---- stem farms (clk-misc) ----
    Melon,
}

/// (block, id, conductor, full collision cube (sturdy on every face), instrument, push reaction)
const SIMPLES: [(Simple, &str, bool, bool, Instrument, PushReaction); 78] = [
    (Simple::Stone, "stone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::SmoothStone, "smooth_stone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Cobblestone, "cobblestone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Obsidian, "obsidian", true, true, Instrument::Basedrum, PushReaction::Immoveable),
    (Simple::Glass, "glass", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::TintedGlass, "tinted_glass", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::WhiteWool, "white_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::OakPlanks, "oak_planks", true, true, Instrument::Bass, PushReaction::PushPull),
    (Simple::GoldBlock, "gold_block", true, true, Instrument::Bell, PushReaction::PushPull),
    (Simple::IronBlock, "iron_block", true, true, Instrument::IronXylophone, PushReaction::PushPull),
    (Simple::Clay, "clay", true, true, Instrument::Flute, PushReaction::PushPull),
    (Simple::EmeraldBlock, "emerald_block", true, true, Instrument::Bit, PushReaction::PushPull),
    (Simple::Glowstone, "glowstone", false, true, Instrument::Pling, PushReaction::PushPull),
    (Simple::PackedIce, "packed_ice", true, true, Instrument::Chime, PushReaction::PushPull),
    // Soul sand: `isRedstoneConductor(always)` though its collision box is 14 px tall; its faces
    // are still full for support (block support shape = full cube).
    (Simple::SoulSand, "soul_sand", true, true, Instrument::CowBell, PushReaction::PushPull),
    (Simple::SlimeBlock, "slime_block", true, true, Instrument::Harp, PushReaction::PushPull),
    // Honey: collision 15 px wide: not a conductor; support shape full.
    (Simple::HoneyBlock, "honey_block", false, false, Instrument::Harp, PushReaction::PushPull),
    (Simple::WaxedCopperGrate, "waxed_copper_grate", false, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::SeaLantern, "sea_lantern", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::Granite, "granite", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::PolishedGranite, "polished_granite", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Diorite, "diorite", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Andesite, "andesite", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::MossyCobblestone, "mossy_cobblestone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::StoneBricks, "stone_bricks", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Tuff, "tuff", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Calcite, "calcite", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Bricks, "bricks", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Sandstone, "sandstone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::NetherBricks, "nether_bricks", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::EndStone, "end_stone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Blackstone, "blackstone", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::QuartzBlock, "quartz_block", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Prismarine, "prismarine", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::PurpurBlock, "purpur_block", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::Terracotta, "terracotta", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::CoalBlock, "coal_block", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::RawIronBlock, "raw_iron_block", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::MudBricks, "mud_bricks", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::WhiteConcrete, "white_concrete", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::BlackConcrete, "black_concrete", true, true, Instrument::Basedrum, PushReaction::PushPull),
    (Simple::DiamondBlock, "diamond_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::LapisBlock, "lapis_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::NetheriteBlock, "netherite_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::SoulSoil, "soul_soil", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::DriedKelpBlock, "dried_kelp_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::Shroomlight, "shroomlight", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::NetherWartBlock, "nether_wart_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::WarpedWartBlock, "warped_wart_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::AmethystBlock, "amethyst_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::Bookshelf, "bookshelf", true, true, Instrument::Bass, PushReaction::PushPull),
    (Simple::CraftingTable, "crafting_table", true, true, Instrument::Bass, PushReaction::PushPull),
    (Simple::SprucePlanks, "spruce_planks", true, true, Instrument::Bass, PushReaction::PushPull),
    (Simple::OrangeWool, "orange_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::BlackWool, "black_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    // pumpkins break when a piston pushes them (POPPED)
    (Simple::Pumpkin, "pumpkin", true, true, Instrument::Didgeridoo, PushReaction::Popped),
    (Simple::WhiteStainedGlass, "white_stained_glass", false, true, Instrument::Hat, PushReaction::PushPull),
    (Simple::BlackStainedGlass, "black_stained_glass", false, true, Instrument::Hat, PushReaction::PushPull),
    // mud: like soul sand (always a conductor, 14 px collision, full support), dirt's harp
    (Simple::Mud, "mud", true, true, Instrument::Harp, PushReaction::PushPull),
    // ---- pistmodel ----
    (Simple::CryingObsidian, "crying_obsidian", true, true, Instrument::Basedrum, PushReaction::Immoveable),
    // ---- entity-reacting blocks (clk-misc) ----
    (Simple::Dirt, "dirt", true, true, Instrument::Harp, PushReaction::PushPull),
    // grass_block (its `snowy` property is not modeled: the bare name, snowy=false) and bedrock
    // (BLOCK: immovable): the TNT judge's ground
    (Simple::GrassBlock, "grass_block", true, true, Instrument::Harp, PushReaction::PushPull),
    (Simple::Bedrock, "bedrock", true, true, Instrument::Basedrum, PushReaction::Immoveable),
    // ---- fluids (clockinfra) ----
    (Simple::MagmaBlock, "magma_block", true, true, Instrument::Basedrum, PushReaction::PushPull),
    // ---- the other wool colours (clk-misc): every wool is a plain conductor, GUITAR ----
    (Simple::MagentaWool, "magenta_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::LightBlueWool, "light_blue_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::YellowWool, "yellow_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::LimeWool, "lime_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::PinkWool, "pink_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::GrayWool, "gray_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::LightGrayWool, "light_gray_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::CyanWool, "cyan_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::PurpleWool, "purple_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::BlueWool, "blue_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::BrownWool, "brown_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::GreenWool, "green_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    (Simple::RedWool, "red_wool", true, true, Instrument::Guitar, PushReaction::PushPull),
    // ---- stem farms (clk-misc): a melon is a plain block (no instrument: harp) that pistons pop ----
    (Simple::Melon, "melon", true, true, Instrument::Harp, PushReaction::Popped),
];

impl Simple {
    /// Every plain block's id (what `Block::parse` accepts).
    pub fn names() -> Vec<&'static str> {
        SIMPLES.iter().map(|r| r.1).collect()
    }
    fn row(self) -> &'static (Simple, &'static str, bool, bool, Instrument, PushReaction) {
        SIMPLES.iter().find(|r| r.0 == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        self.row().1
    }
    pub fn conductor(self) -> bool {
        self.row().2
    }
    pub fn full(self) -> bool {
        self.row().3
    }
    pub fn instrument(self) -> Instrument {
        self.row().4
    }
    pub fn push_reaction(self) -> PushReaction {
        self.row().5
    }
    fn parse(s: &str) -> Option<Simple> {
        SIMPLES.iter().find(|r| r.1 == s).map(|r| r.0)
    }
}

/// Copper oxidation stages (`WeatheringCopper.WeatherState`, in its order).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Oxid {
    Unaffected,
    Exposed,
    Weathered,
    Oxidized,
}
impl Oxid {
    /// The stage's block-id prefix ("", "exposed_", "weathered_", "oxidized_").
    pub fn prefix(self) -> &'static str {
        match self {
            Oxid::Unaffected => "",
            Oxid::Exposed => "exposed_",
            Oxid::Weathered => "weathered_",
            Oxid::Oxidized => "oxidized_",
        }
    }

    /// `WeatheringCopper.getNext`: the next stage (none after oxidized).
    pub fn next(self) -> Option<Oxid> {
        match self {
            Oxid::Unaffected => Some(Oxid::Exposed),
            Oxid::Exposed => Some(Oxid::Weathered),
            Oxid::Weathered => Some(Oxid::Oxidized),
            Oxid::Oxidized => None,
        }
    }

    /// `WeatheringCopper.getPrevious`: the stage before (none before unaffected).
    pub fn previous(self) -> Option<Oxid> {
        match self {
            Oxid::Unaffected => None,
            Oxid::Exposed => Some(Oxid::Unaffected),
            Oxid::Weathered => Some(Oxid::Exposed),
            Oxid::Oxidized => Some(Oxid::Weathered),
        }
    }

    /// A weathering copper block id (without `minecraft:`) split into (stage, waxed, the base id:
    /// `copper_bulb`, `lightning_rod`...).
    pub fn split(name: &str) -> (Oxid, bool, &str) {
        let (waxed, rest) = match name.strip_prefix("waxed_") {
            Some(r) => (true, r),
            None => (false, name),
        };
        for o in [Oxid::Exposed, Oxid::Weathered, Oxid::Oxidized] {
            if let Some(r) = rest.strip_prefix(o.prefix()) {
                return (o, waxed, r);
            }
        }
        (Oxid::Unaffected, waxed, rest)
    }

    /// The `Kind` byte of a weathering block: the stage, and 4 when waxed.
    fn kind_byte(self, waxed: bool) -> u8 {
        self as u8 | if waxed { 4 } else { 0 }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Half {
    Top,
    Bottom,
}

/// `RailShape`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum RailShape {
    NorthSouth,
    EastWest,
    AscendingEast,
    AscendingWest,
    AscendingNorth,
    AscendingSouth,
    // ---- entity-reacting blocks (clk-misc): the plain rail's curves ----
    SouthEast,
    SouthWest,
    NorthWest,
    NorthEast,
}
const RAIL_SHAPES: [(RailShape, &str); 10] = [
    (RailShape::NorthSouth, "north_south"),
    (RailShape::EastWest, "east_west"),
    (RailShape::AscendingEast, "ascending_east"),
    (RailShape::AscendingWest, "ascending_west"),
    (RailShape::AscendingNorth, "ascending_north"),
    (RailShape::AscendingSouth, "ascending_south"),
    (RailShape::SouthEast, "south_east"),
    (RailShape::SouthWest, "south_west"),
    (RailShape::NorthWest, "north_west"),
    (RailShape::NorthEast, "north_east"),
];
impl RailShape {
    pub fn name(self) -> &'static str {
        RAIL_SHAPES.iter().find(|(s, _)| *s == self).unwrap().1
    }
    fn parse(s: &str) -> Option<RailShape> {
        RAIL_SHAPES.iter().find(|(_, n)| *n == s).map(|(r, _)| *r)
    }
    pub fn is_ascending(self) -> bool {
        matches!(self, RailShape::AscendingEast | RailShape::AscendingWest | RailShape::AscendingNorth | RailShape::AscendingSouth)
    }
    /// A curve (plain rails only).
    pub fn is_curve(self) -> bool {
        matches!(self, RailShape::SouthEast | RailShape::SouthWest | RailShape::NorthWest | RailShape::NorthEast)
    }
}

/// `FrontAndTop` (crafter orientation).
pub const ORIENTATIONS: [&str; 12] = [
    "down_east", "down_north", "down_south", "down_west", "up_east", "up_north", "up_south", "up_west", "west_up", "east_up", "north_up", "south_up",
];

/// One block state.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Block {
    Air,
    Simple(Simple),
    /// Glazed terracotta (white): a conductor that pistons push but never pull or drag.
    Glazed { facing: Dir },
    Wire { north: Side, east: Side, south: Side, west: Side, power: u8 },
    RedstoneBlock,
    Torch { lit: bool },
    WallTorch { facing: Dir, lit: bool },
    Lever { face: Face, facing: Dir, powered: bool },
    /// Stone (20 gt) or oak (30 gt) button.
    Button { wood: bool, face: Face, facing: Dir, powered: bool },
    Repeater { facing: Dir, delay: u8, locked: bool, powered: bool },
    Comparator { facing: Dir, subtract: bool, powered: bool },
    Observer { facing: Dir, powered: bool },
    Lamp { lit: bool },
    /// A copper bulb (`CopperBulbBlock`), waxed or not (`WeatheringCopperBulbBlock`: an unwaxed one
    /// oxidizes by random ticks, `ChangeOverTimeBlock.changeOverTime`; mobinfra 2026-11-08).
    Bulb { oxid: Oxid, waxed: bool, lit: bool, powered: bool },
    Note { instrument: Instrument, note: u8, powered: bool },
    Target { power: u8 },
    /// Iron (redstone only) or oak trapdoor.
    Trapdoor { iron: bool, facing: Dir, top: bool, open: bool, powered: bool },
    /// Iron or oak door, one half.
    Door { iron: bool, facing: Dir, upper: bool, hinge_right: bool, open: bool, powered: bool },
    FenceGate { facing: Dir, in_wall: bool, open: bool, powered: bool },
    /// Powered rail or activator rail.
    Rail { activator: bool, shape: RailShape, powered: bool },
    Dispenser { dropper: bool, facing: Dir, triggered: bool },
    Crafter { orientation: u8, triggered: bool, crafting: bool },
    Hopper { facing: Dir, enabled: bool },
    /// Smooth stone slab: 0 bottom, 1 top, 2 double.
    Slab { kind: u8 },
    /// Persistent oak leaves (`persistent=false` ones decay at random: out of scope).
    Leaves { distance: u8, persistent: bool },
    /// Scaffolding (`ScaffoldingBlock`): `distance` 0-7 to a sturdy floor, `bottom` = distance > 0
    /// with no scaffold below.
    Scaffolding { distance: u8, bottom: bool },
    /// Oak log (a leaves distance source).
    Log { axis: u8 },
    /// Skeleton skull (`powered` follows its neighbor signal; pistons pop it).
    Skull { rotation: u8, powered: bool },
    WallSkull { facing: Dir, powered: bool },
    /// Constant comparator sources (analog output from the state alone).
    Cake { bites: u8 },
    EndPortalFrame { facing: Dir, eye: bool },
    Composter { level: u8 },
    RespawnAnchor { charges: u8 },
    // ---- pistons (agent pistmodel's side of the model) ----
    Piston { sticky: bool, facing: Dir, extended: bool },
    PistonHead { sticky: bool, facing: Dir, short: bool },
    MovingPiston { sticky: bool, facing: Dir },
    /// Gravity blocks (pistmodel, `gravity.rs`): index into `FALLING` (sand, red sand, gravel,
    /// white concrete powder).
    Falling { id: u8 },
    // ---- sculk (agent sculk, `sculk.rs`) ----
    /// Sculk sensor (`calibrated`: the calibrated sculk sensor, whose horizontal `facing` names
    /// its back, the frequency input, at `facing.opposite()`; a plain sensor keeps North).
    /// `phase`: 0 inactive, 1 active, 2 cooldown (`SculkSensorPhase`).
    Sculk { calibrated: bool, facing: Dir, phase: u8, power: u8 },
    // ---- entity-reacting blocks (clk-misc) ----
    /// Pressure plate (`react::plate`): `kind` 0 oak, 1 stone (`powered`: power 0 or 15), 2 light /
    /// 3 heavy weighted (`power` 0-15).
    Plate { kind: u8, power: u8 },
    /// Tripwire string (`react::tripwire`).
    Tripwire { powered: bool, attached: bool, disarmed: bool, north: bool, east: bool, south: bool, west: bool },
    /// Tripwire hook (`react::hook`), hung on the block at `facing.opposite()`.
    TripwireHook { facing: Dir, powered: bool, attached: bool },
    /// White wall banner, hung on the block at `facing.opposite()`: no collision, solid (water
    /// cannot enter it).
    WallBanner { facing: Dir },
    /// Lantern, hanging from the block above or standing.
    Lantern { hanging: bool },
    // ---- placed light sources (terrain lane: a day-1 player lights a farm) ----
    /// A torch on a floor (`TorchBlock`; `soul`: the soul torch): no collision, light 14 (soul
    /// 10), standing on the center of the block below (`canSupportCenter`), washed away by fluids.
    PlainTorch { soul: bool },
    /// A torch on a wall (`WallTorchBlock`), hung on the sturdy face of the block at
    /// `facing.opposite()`.
    PlainWallTorch { soul: bool, facing: Dir },
    /// A soul lantern (the lantern's shapes and rules, light 10).
    SoulLantern { hanging: bool },
    /// A jack o'lantern (`CarvedPumpkinBlock`, light 15): a full cube.
    JackOLantern { facing: Dir },
    /// A campfire (`CampfireBlock`: a 7 px slab; light 15 when lit, soul 10).
    Campfire { soul: bool, lit: bool, facing: Dir, signal_fire: bool },
    /// An end rod (`EndRodBlock`: a 4 px rod along `facing`, light 14).
    EndRod { facing: Dir },
    /// A froglight (`kind` 0 ochre, 1 verdant, 2 pearlescent; `axis` 0 x, 1 y, 2 z; light 15).
    Froglight { kind: u8, axis: u8 },
    /// Sea pickles (`pickles` 1-4; light 3 + 3 x pickles when waterlogged, else none).
    SeaPickle { pickles: u8, waterlogged: bool },
    /// Candles (the plain candle, `candles` 1-4; light 3 x candles when lit).
    Candle { candles: u8, lit: bool },
    /// Stone slab: 0 bottom, 1 top, 2 double (the shapes of `Slab`).
    StoneSlab { kind: u8 },
    /// Every other slab of the game's slab family, and smooth stone's and stone's waterlogged ones
    /// (`slab`: `mat` indexes `slab::SLABS`; `kind` 0 bottom, 1 top, 2 double).
    AnySlab { mat: u8, kind: u8, waterlogged: bool },
    /// Plain rail (`RailBlock`: curves too), `react::rails`.
    PlainRail { shape: RailShape },
    /// Detector rail (`DetectorRailBlock`: straight shapes), `react::rails`.
    DetectorRail { shape: RailShape, powered: bool },
    /// Big dripleaf (the leaf; `react::dripleaf`): `tilt` 0 none, 1 unstable, 2 partial, 3 full.
    BigDripleaf { facing: Dir, tilt: u8, waterlogged: bool },
    /// Chiseled bookshelf: `slots` = the six `slot_N_occupied` flags (bit N); its 6-slot container
    /// and `last_interacted_slot` live in the block entity.
    ChiseledBookshelf { facing: Dir, slots: u8 },
    /// Daylight detector (`daylight.rs`): its power from the sky light every 20 gt.
    DaylightDetector { inverted: bool, power: u8 },
    /// Powder snow (placed and picked up by dispensers' buckets): no collision without an
    /// entity, not solid, not a conductor.
    PowderSnow,
    /// Jukebox: `has_record`; its record and song live in the block entity (`container.rs`).
    Jukebox { has_record: bool },
    /// Furnace (`FurnaceBlock`, track sorter's smelter study): `lit` while it burns; its 3 slots
    /// (input, fuel, result) and burn and cooking timers live in the block entity
    /// (`container.rs`, `AbstractFurnaceBlockEntity`). `blast`: a blast furnace
    /// (`BlastFurnaceBlock`, tickexact 2026-11-10: blasting recipes, `#block/fast_cooking`: fuel
    /// burns half as long, cooking twice as fast).
    Furnace { facing: Dir, lit: bool, blast: bool },
    /// Brewing stand (`BrewingStandBlock`, track sorter's brewer study): `bottles` = the
    /// `has_bottle_0..2` bits; its 5 slots (3 potions, ingredient, fuel), brew time and fuel uses
    /// live in the block entity (`container.rs`, `BrewingStandBlockEntity`).
    BrewingStand { bottles: u8 },
    // ---- fluids (clockinfra, `fluid/`) ----
    /// Water (`LiquidBlock`): `level` 0 = source, 1..7 = flowing (amount 8 - level), 8..15 =
    /// falling (`getLegacyLevel`).
    Water { level: u8 },
    /// A bubble column (`BubbleColumnBlock`, a water source): `drag` = down (magma below), else up
    /// (soul sand below).
    BubbleColumn { drag: bool },
    // ---- TNT dupers (track tnt, `tnt.rs`) ----
    /// TNT (`TntBlock`): primes on placement or an update while powered.
    Tnt { unstable: bool },
    /// Dead coral fan on a floor (`BaseCoralFanBlock`): `kind` indexes `tnt::DEAD_CORALS`.
    CoralFan { kind: u8, waterlogged: bool },
    /// Dead coral wall fan (`BaseCoralWallFanBlock`), on the block at `facing.opposite()`.
    CoralWallFan { kind: u8, facing: Dir, waterlogged: bool },
    // ---- shulker study (clk-rt, `redstone/container.rs`) ----
    /// Shulker box (`ShulkerBoxBlock`, always closed here: a full cube, a conductor, solid):
    /// `colour` 0 = plain, else 1 + the `DyeColor` id ([`DYES`]); its 27 slots live in the block
    /// entity. Pistons destroy it: it drops as an item carrying its contents.
    ShulkerBox { colour: u8, facing: Dir },
    /// Chest (`ChestBlock`): `kind` 0 = `type=single`, 1 = `left`, 2 = `right`; 27 slots in each
    /// half's block entity, a double chest's two halves one container for hoppers and comparators
    /// (`CompoundContainer`: the right half's slots first; [`crate::redstone::container`]).
    /// Immovable (a block entity), not a conductor; shape 1..15 x 0..14 x 1..15 px (a double
    /// chest's halves keep the single shape here: nothing judged collides with them).
    Chest { facing: Dir, kind: u8, waterlogged: bool },
    // ---- fences (clk-misc, `fence.rs`) ----
    /// A fence (`FenceBlock`): `kind` indexes `fence::FENCES` (wooden, then nether brick); the four
    /// arms and the water.
    Fence { kind: u8, north: bool, east: bool, south: bool, west: bool, waterlogged: bool },
    // ---- crop farms (webrender, `crops.rs`) ----
    /// Sugar cane (`SugarCaneBlock`): its growth `age` (0..15: a random tick at 15 grows the block
    /// above, `crops::random_tick`).
    SugarCane { age: u8 },
    /// A waterlogged waxed copper grate (`WaterloggedTransparentBlock`): the dry grate
    /// (`Simple::WaxedCopperGrate`) holding a water source, which a full collision block never
    /// lets flow out (a farm's water beside its soil).
    WaterloggedGrate,
    // ---- crop farms (clk-misc, `bamboo.rs`, `kelp.rs`) ----
    /// A bamboo stalk (`BambooStalkBlock`): `age` 0 thin / 1 thick, `leaves` 0 none / 1 small /
    /// 2 large (`bamboo::LEAVES`), `stage` 1 = done growing.
    Bamboo { age: u8, leaves: u8, stage: u8 },
    /// A bamboo shoot (`BambooSaplingBlock`).
    BambooSapling,
    /// Kelp's top (`KelpBlock`, a water source inside): its `age` (0..25; 25 grows no more).
    Kelp { age: u8 },
    /// Kelp under its top (`KelpPlantBlock`, a water source inside).
    KelpPlant,
    // ---- stem farms (clk-misc, `stems.rs`) ----
    /// Farmland (`FarmlandBlock`): `moisture` 0..7 (7 = wet: near water).
    Farmland { moisture: u8 },
    /// A pumpkin or melon stem (`StemBlock`): `age` 0..7 (at 7 it grows its fruit).
    Stem { melon: bool, age: u8 },
    /// An attached stem (`AttachedStemBlock`), its fruit at `facing`.
    AttachedStem { melon: bool, facing: Dir },
    // ---- cactus farms (clk-misc, `cactus.rs`) ----
    /// Cactus (`CactusBlock`): `age` 0..15 (at 15 it grows a block above).
    Cactus { age: u8 },
    /// A cactus flower (`CactusFlowerBlock`).
    CactusFlower,
    // ---- mob farms (ironfarm, `entity/mob/`) ----
    /// Lava (`LiquidBlock` of `LavaFluid`): `level` 0 = a source, 1..7 flowing, 8+ falling (the
    /// legacy level, as water's). It flows (mobinfra: `fluid`'s FlowingFluid with LavaFluid's
    /// numbers and overrides); entities in it burn (`LavaFluid.entityInside`).
    Lava { level: u8 },
    /// A bed half (`BedBlock`): `colour` = the `DyeColor` id ([`DYES`]), `facing` toward the
    /// head, `head` = `part=head` (the villagers' POI), `occupied`.
    Bed { colour: u8, facing: Dir, head: bool, occupied: bool },
    /// An oak sign, standing (`rotation` 0-15) or on a wall (`facing`), never waterlogged here
    /// (`SignBlock`): no collision box, not solid, a `LiquidBlockContainer` that takes only a water
    /// source, so flowing water and lava never enter it: a mob farm's water stop and lava holder
    /// that golems and items pass through.
    Sign { wall: bool, rotation: u8, facing: Dir },
    // ---- terrain (the terrain lane, `terrain/`) ----
    /// A natural block state the official server generated (`terrain::blocks`: its row in the
    /// natural-state table, the game's own collision boxes, light, solidity and spawn rules). Inert:
    /// no redstone behaviour, no reaction to updates (the judge refuses a paste or a farm that would
    /// update one that reacts).
    Terrain(u16),
    // ---- weather (mobinfra, `weather/lightning.rs`) ----
    /// A lightning rod (`LightningRodBlock`, a `RodBlock`: a 4 x 4 px column along `facing`'s
    /// axis) at an oxidation stage, waxed or not (an unwaxed one, `WeatheringLightningRodBlock`,
    /// oxidizes by random ticks like every weathering copper block), never waterlogged: bolts
    /// within 128 go to the one that tops its column, and a struck rod powers for 8 gt (strongly
    /// into the block it hangs on).
    LightningRod { facing: Dir, powered: bool, waxed: bool, oxid: Oxid },
    // ---- fire (mobinfra, `weather/fire.rs`) ----
    /// Fire (`FireBlock`: `age` 0..15; `sides` its north, east, south, west, up properties, bits
    /// 0..4, the sides it burns toward: none on a floor it stands on) or soul fire (`SoulFireBlock`,
    /// on soul sand or soil: no age, no sides). No collision, replaceable, light 15 (soul 10).
    Fire { age: u8, sides: u8, soul: bool },
}

/// `DyeColor` names by id (a shulker box's `colour` - 1).
pub const DYES: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

/// `SculkSensorPhase` names, by `Block::Sculk::phase`.
pub const SCULK_PHASES: [&str; 3] = ["inactive", "active", "cooldown"];

/// Gravity blocks (`Block::Falling`), by id.
pub const FALLING: [&str; 4] = ["sand", "red_sand", "gravel", "white_concrete_powder"];

/// The block type (`state.getBlock()`): what `state.is(block)`, scheduled ticks and block events
/// compare.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Kind {
    Air,
    Simple(u8),
    Glazed,
    Wire,
    RedstoneBlock,
    Torch,
    WallTorch,
    Lever,
    Button(bool),
    Repeater,
    Comparator,
    Observer,
    Lamp,
    Bulb(u8),
    Note,
    Target,
    Trapdoor(bool),
    Door(bool),
    FenceGate,
    Rail(bool),
    Dispenser(bool),
    Crafter,
    Hopper,
    Slab,
    Leaves,
    Scaffolding,
    Log,
    Skull,
    WallSkull,
    Cake,
    EndPortalFrame,
    Composter,
    RespawnAnchor,
    Piston(bool),
    PistonHead,
    MovingPiston,
    Falling(u8),
    /// Sculk sensor (true: calibrated).
    Sculk(bool),
    // ---- entity-reacting blocks (clk-misc) ----
    /// Pressure plate, by `Block::Plate::kind`.
    Plate(u8),
    Tripwire,
    TripwireHook,
    WallBanner,
    Lantern,
    PlainTorch,
    PlainWallTorch,
    SoulLantern,
    JackOLantern,
    Campfire,
    EndRod,
    Froglight,
    SeaPickle,
    Candle,
    StoneSlab,
    AnySlab,
    PlainRail,
    DetectorRail,
    BigDripleaf,
    ChiseledBookshelf,
    DaylightDetector,
    PowderSnow,
    Jukebox,
    Furnace,
    BrewingStand,
    Water,
    BubbleColumn,
    // ---- TNT dupers (track tnt) ----
    Tnt,
    /// By `tnt::DEAD_CORALS` index.
    CoralFan(u8),
    CoralWallFan(u8),
    // ---- shulker study (clk-rt) ----
    /// By `Block::ShulkerBox::colour` (each colour is its own block).
    ShulkerBox(u8),
    Chest,
    // ---- fences (clk-misc) ----
    /// By `fence::FENCES` index.
    Fence(u8),
    // ---- crop farms (webrender) ----
    SugarCane,
    // ---- crop farms (clk-misc) ----
    Bamboo,
    BambooSapling,
    Kelp,
    KelpPlant,
    // ---- stem farms (clk-misc) ----
    Farmland,
    /// By `melon`.
    Stem(bool),
    AttachedStem(bool),
    // ---- cactus farms (clk-misc) ----
    Cactus,
    CactusFlower,
    // ---- mob farms (ironfarm) ----
    Lava,
    /// By `Block::Bed::colour` (each colour is its own block).
    Bed(u8),
    /// Standing (`false`) or wall (`true`) oak sign.
    Sign(bool),
    // ---- terrain ----
    /// By the natural-state table's row.
    Terrain(u16),
    // ---- weather (mobinfra): the oxidation stage, 4 when waxed ----
    LightningRod(u8),
    // ---- fire (mobinfra): fire or soul fire ----
    Fire(bool),
}

fn b(s: bool) -> &'static str {
    if s { "true" } else { "false" }
}

impl Block {
    pub fn kind(&self) -> Kind {
        match *self {
            Block::Air => Kind::Air,
            Block::Simple(s) => Kind::Simple(s as u8),
            Block::Glazed { .. } => Kind::Glazed,
            Block::Wire { .. } => Kind::Wire,
            Block::RedstoneBlock => Kind::RedstoneBlock,
            Block::Torch { .. } => Kind::Torch,
            Block::WallTorch { .. } => Kind::WallTorch,
            Block::Lever { .. } => Kind::Lever,
            Block::Button { wood, .. } => Kind::Button(wood),
            Block::Repeater { .. } => Kind::Repeater,
            Block::Comparator { .. } => Kind::Comparator,
            Block::Observer { .. } => Kind::Observer,
            Block::Lamp { .. } => Kind::Lamp,
            Block::Bulb { oxid, waxed, .. } => Kind::Bulb(oxid.kind_byte(waxed)),
            Block::Note { .. } => Kind::Note,
            Block::Target { .. } => Kind::Target,
            Block::Trapdoor { iron, .. } => Kind::Trapdoor(iron),
            Block::Door { iron, .. } => Kind::Door(iron),
            Block::FenceGate { .. } => Kind::FenceGate,
            Block::Rail { activator, .. } => Kind::Rail(activator),
            Block::Dispenser { dropper, .. } => Kind::Dispenser(dropper),
            Block::Crafter { .. } => Kind::Crafter,
            Block::Hopper { .. } => Kind::Hopper,
            Block::Slab { .. } => Kind::Slab,
            Block::Leaves { .. } => Kind::Leaves,
            Block::Scaffolding { .. } => Kind::Scaffolding,
            Block::Log { .. } => Kind::Log,
            Block::Skull { .. } => Kind::Skull,
            Block::WallSkull { .. } => Kind::WallSkull,
            Block::Cake { .. } => Kind::Cake,
            Block::EndPortalFrame { .. } => Kind::EndPortalFrame,
            Block::Composter { .. } => Kind::Composter,
            Block::RespawnAnchor { .. } => Kind::RespawnAnchor,
            Block::Piston { sticky, .. } => Kind::Piston(sticky),
            Block::PistonHead { .. } => Kind::PistonHead,
            Block::MovingPiston { .. } => Kind::MovingPiston,
            Block::Falling { id } => Kind::Falling(id),
            Block::Sculk { calibrated, .. } => Kind::Sculk(calibrated),
            // ---- entity-reacting blocks (clk-misc) ----
            Block::Plate { kind, .. } => Kind::Plate(kind),
            Block::Tripwire { .. } => Kind::Tripwire,
            Block::TripwireHook { .. } => Kind::TripwireHook,
            Block::WallBanner { .. } => Kind::WallBanner,
            Block::Lantern { .. } => Kind::Lantern,
            Block::PlainTorch { .. } => Kind::PlainTorch,
            Block::PlainWallTorch { .. } => Kind::PlainWallTorch,
            Block::SoulLantern { .. } => Kind::SoulLantern,
            Block::JackOLantern { .. } => Kind::JackOLantern,
            Block::Campfire { .. } => Kind::Campfire,
            Block::EndRod { .. } => Kind::EndRod,
            Block::Froglight { .. } => Kind::Froglight,
            Block::SeaPickle { .. } => Kind::SeaPickle,
            Block::Candle { .. } => Kind::Candle,
            Block::StoneSlab { .. } => Kind::StoneSlab,
            Block::AnySlab { .. } => Kind::AnySlab,
            Block::PlainRail { .. } => Kind::PlainRail,
            Block::DetectorRail { .. } => Kind::DetectorRail,
            Block::BigDripleaf { .. } => Kind::BigDripleaf,
            Block::ChiseledBookshelf { .. } => Kind::ChiseledBookshelf,
            Block::DaylightDetector { .. } => Kind::DaylightDetector,
            Block::PowderSnow => Kind::PowderSnow,
            Block::Jukebox { .. } => Kind::Jukebox,
            Block::Furnace { .. } => Kind::Furnace,
            Block::BrewingStand { .. } => Kind::BrewingStand,
            Block::Water { .. } => Kind::Water,
            Block::BubbleColumn { .. } => Kind::BubbleColumn,
            Block::Tnt { .. } => Kind::Tnt,
            Block::CoralFan { kind, .. } => Kind::CoralFan(kind),
            Block::CoralWallFan { kind, .. } => Kind::CoralWallFan(kind),
            Block::ShulkerBox { colour, .. } => Kind::ShulkerBox(colour),
            Block::Chest { .. } => Kind::Chest,
            Block::Fence { kind, .. } => Kind::Fence(kind),
            Block::SugarCane { .. } => Kind::SugarCane,
            // the dry grate's block, another state
            Block::WaterloggedGrate => Kind::Simple(Simple::WaxedCopperGrate as u8),
            Block::Bamboo { .. } => Kind::Bamboo,
            Block::BambooSapling => Kind::BambooSapling,
            Block::Kelp { .. } => Kind::Kelp,
            Block::KelpPlant => Kind::KelpPlant,
            Block::Farmland { .. } => Kind::Farmland,
            Block::Stem { melon, .. } => Kind::Stem(melon),
            Block::AttachedStem { melon, .. } => Kind::AttachedStem(melon),
            Block::Cactus { .. } => Kind::Cactus,
            Block::CactusFlower => Kind::CactusFlower,
            Block::Lava { .. } => Kind::Lava,
            Block::Bed { colour, .. } => Kind::Bed(colour),
            Block::Sign { wall, .. } => Kind::Sign(wall),
            Block::Terrain(id) => Kind::Terrain(id),
            Block::LightningRod { waxed, oxid, .. } => Kind::LightningRod(oxid.kind_byte(waxed)),
            Block::Fire { soul, .. } => Kind::Fire(soul),
        }
    }

    /// `BlockState.isAir()`: air, and the natural air states (cave air, void air: the
    /// natural-state table's `air` column; a terrain sample's caves hold cave air).
    pub fn is_air(&self) -> bool {
        match self {
            Block::Air => true,
            Block::Terrain(id) => crate::terrain::blocks::props(*id).air,
            _ => false,
        }
    }

    /// Parse `minecraft:name[k=v,...]` (the prefix is optional; missing properties take the
    /// game's defaults).
    pub fn parse(s: &str) -> Result<Block, String> {
        let s = s.trim();
        let s = s.strip_prefix("minecraft:").unwrap_or(s);
        let (name, props) = match s.find('[') {
            Some(i) => (&s[..i], s[i + 1..].trim_end_matches(']')),
            None => (s, ""),
        };
        let mut kv: Vec<(&str, &str)> = Vec::new();
        for p in props.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = p.split_once('=').ok_or_else(|| format!("bad property {p:?} in {s:?}"))?;
            kv.push((k.trim(), v.trim()));
        }
        let get = |k: &str| kv.iter().find(|(kk, _)| *kk == k).map(|(_, v)| *v);
        let bool_of = |k: &str, d: bool| -> Result<bool, String> {
            match get(k) {
                None => Ok(d),
                Some("true") => Ok(true),
                Some("false") => Ok(false),
                Some(v) => Err(format!("bad {k}={v} in {s:?}")),
            }
        };
        let int_of = |k: &str, d: u8, lo: u8, hi: u8| -> Result<u8, String> {
            match get(k) {
                None => Ok(d),
                Some(v) => v.parse::<u8>().ok().filter(|x| *x >= lo && *x <= hi).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let dir_of = |k: &str, d: Dir| -> Result<Dir, String> {
            match get(k) {
                None => Ok(d),
                Some(v) => Dir::from_name(v).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let hdir_of = |k: &str, d: Dir| -> Result<Dir, String> {
            let x = dir_of(k, d)?;
            if x.is_horizontal() { Ok(x) } else { Err(format!("bad {k} in {s:?}")) }
        };
        let side_of = |k: &str| -> Result<Side, String> {
            match get(k) {
                None => Ok(Side::None),
                Some(v) => Side::parse(v).ok_or_else(|| format!("bad {k}={v} in {s:?}")),
            }
        };
        let face_of = |d: Face| -> Result<Face, String> {
            match get("face") {
                None => Ok(d),
                Some(v) => Face::parse(v).ok_or_else(|| format!("bad face={v} in {s:?}")),
            }
        };
        let waterlogged_ok = || -> Result<(), String> {
            if bool_of("waterlogged", false)? { Err(format!("waterlogged blocks are out of scope: {s:?}")) } else { Ok(()) }
        };
        if let Some(simple) = Simple::parse(name) {
            if name == "waxed_copper_grate" && bool_of("waterlogged", false)? {
                return Ok(Block::WaterloggedGrate);
            }
            return Ok(Block::Simple(simple));
        }
        Ok(match name {
            "air" | "cave_air" | "void_air" => Block::Air,
            "white_glazed_terracotta" => Block::Glazed { facing: hdir_of("facing", Dir::North)? },
            "redstone_wire" => Block::Wire {
                north: side_of("north")?,
                east: side_of("east")?,
                south: side_of("south")?,
                west: side_of("west")?,
                power: int_of("power", 0, 0, 15)?,
            },
            "redstone_block" => Block::RedstoneBlock,
            "redstone_torch" => Block::Torch { lit: bool_of("lit", true)? },
            "redstone_wall_torch" => Block::WallTorch { facing: hdir_of("facing", Dir::North)?, lit: bool_of("lit", true)? },
            "lever" => Block::Lever { face: face_of(Face::Wall)?, facing: hdir_of("facing", Dir::North)?, powered: bool_of("powered", false)? },
            "stone_button" | "oak_button" => Block::Button {
                wood: name == "oak_button",
                face: face_of(Face::Wall)?,
                facing: hdir_of("facing", Dir::North)?,
                powered: bool_of("powered", false)?,
            },
            "repeater" => Block::Repeater {
                facing: hdir_of("facing", Dir::North)?,
                delay: int_of("delay", 1, 1, 4)?,
                locked: bool_of("locked", false)?,
                powered: bool_of("powered", false)?,
            },
            "comparator" => Block::Comparator {
                facing: hdir_of("facing", Dir::North)?,
                subtract: match get("mode") {
                    None | Some("compare") => false,
                    Some("subtract") => true,
                    Some(v) => return Err(format!("bad mode={v}")),
                },
                powered: bool_of("powered", false)?,
            },
            "observer" => Block::Observer { facing: dir_of("facing", Dir::South)?, powered: bool_of("powered", false)? },
            "redstone_lamp" => Block::Lamp { lit: bool_of("lit", false)? },
            n if Oxid::split(n).2 == "copper_bulb" => {
                let (oxid, waxed, _) = Oxid::split(n);
                Block::Bulb { oxid, waxed, lit: bool_of("lit", false)?, powered: bool_of("powered", false)? }
            }
            "note_block" => Block::Note {
                instrument: match get("instrument") {
                    None => Instrument::Harp,
                    Some(v) => Instrument::parse(v).ok_or_else(|| format!("bad instrument={v}"))?,
                },
                note: int_of("note", 0, 0, 24)?,
                powered: bool_of("powered", false)?,
            },
            "target" => Block::Target { power: int_of("power", 0, 0, 15)? },
            "iron_trapdoor" | "oak_trapdoor" => {
                waterlogged_ok()?;
                Block::Trapdoor {
                    iron: name == "iron_trapdoor",
                    facing: hdir_of("facing", Dir::North)?,
                    top: match get("half") {
                        None | Some("bottom") => false,
                        Some("top") => true,
                        Some(v) => return Err(format!("bad half={v}")),
                    },
                    open: bool_of("open", false)?,
                    powered: bool_of("powered", false)?,
                }
            }
            "iron_door" | "oak_door" => Block::Door {
                iron: name == "iron_door",
                facing: hdir_of("facing", Dir::North)?,
                upper: match get("half") {
                    None | Some("lower") => false,
                    Some("upper") => true,
                    Some(v) => return Err(format!("bad half={v}")),
                },
                hinge_right: match get("hinge") {
                    None | Some("left") => false,
                    Some("right") => true,
                    Some(v) => return Err(format!("bad hinge={v}")),
                },
                open: bool_of("open", false)?,
                powered: bool_of("powered", false)?,
            },
            "oak_fence_gate" => Block::FenceGate {
                facing: hdir_of("facing", Dir::North)?,
                in_wall: bool_of("in_wall", false)?,
                open: bool_of("open", false)?,
                powered: bool_of("powered", false)?,
            },
            "powered_rail" | "activator_rail" => {
                waterlogged_ok()?;
                Block::Rail {
                    activator: name == "activator_rail",
                    shape: match get("shape") {
                        None => RailShape::NorthSouth,
                        Some(v) => RailShape::parse(v).filter(|s| !s.is_curve()).ok_or_else(|| format!("bad shape={v}"))?,
                    },
                    powered: bool_of("powered", false)?,
                }
            }
            "big_dripleaf" => Block::BigDripleaf {
                facing: hdir_of("facing", Dir::North)?,
                tilt: match get("tilt") {
                    None | Some("none") => 0,
                    Some("unstable") => 1,
                    Some("partial") => 2,
                    Some("full") => 3,
                    Some(v) => return Err(format!("bad tilt={v}")),
                },
                waterlogged: bool_of("waterlogged", false)?,
            },
            "chiseled_bookshelf" => {
                let mut slots = 0u8;
                for n in 0..6 {
                    if bool_of(&format!("slot_{n}_occupied"), false)? {
                        slots |= 1 << n;
                    }
                }
                Block::ChiseledBookshelf { facing: hdir_of("facing", Dir::North)?, slots }
            }
            "daylight_detector" => Block::DaylightDetector { inverted: bool_of("inverted", false)?, power: int_of("power", 0, 0, 15)? },
            "powder_snow" => Block::PowderSnow,
            "jukebox" => Block::Jukebox { has_record: bool_of("has_record", false)? },
            "furnace" => Block::Furnace { facing: hdir_of("facing", Dir::North)?, lit: bool_of("lit", false)?, blast: false },
            "blast_furnace" => Block::Furnace { facing: hdir_of("facing", Dir::North)?, lit: bool_of("lit", false)?, blast: true },
            "brewing_stand" => Block::BrewingStand { bottles: (0..3).try_fold(0u8, |b, i| Ok::<u8, String>(b | (u8::from(bool_of(["has_bottle_0", "has_bottle_1", "has_bottle_2"][i], false)?) << i)))? },
            "dispenser" | "dropper" => Block::Dispenser { dropper: name == "dropper", facing: dir_of("facing", Dir::North)?, triggered: bool_of("triggered", false)? },
            "crafter" => Block::Crafter {
                orientation: match get("orientation") {
                    None => 10,
                    Some(v) => ORIENTATIONS.iter().position(|o| *o == v).ok_or_else(|| format!("bad orientation={v}"))? as u8,
                },
                triggered: bool_of("triggered", false)?,
                crafting: bool_of("crafting", false)?,
            },
            "hopper" => Block::Hopper {
                facing: {
                    let d = dir_of("facing", Dir::Down)?;
                    if d == Dir::Up {
                        return Err("hopper facing up".into());
                    }
                    d
                },
                enabled: bool_of("enabled", true)?,
            },
            "smooth_stone_slab" => {
                let kind = match get("type") {
                    None | Some("bottom") => 0,
                    Some("top") => 1,
                    Some("double") => 2,
                    Some(v) => return Err(format!("bad type={v}")),
                };
                if bool_of("waterlogged", false)? {
                    Block::AnySlab { mat: crate::slab::index("smooth_stone_slab").unwrap_or(0), kind, waterlogged: true }
                } else {
                    Block::Slab { kind }
                }
            }
            "oak_leaves" => {
                waterlogged_ok()?;
                Block::Leaves { distance: int_of("distance", 7, 1, 7)?, persistent: bool_of("persistent", false)? }
            }
            "scaffolding" => {
                waterlogged_ok()?;
                Block::Scaffolding { distance: int_of("distance", 7, 0, 7)?, bottom: bool_of("bottom", false)? }
            }
            "oak_log" => Block::Log {
                axis: match get("axis") {
                    None | Some("y") => 1,
                    Some("x") => 0,
                    Some("z") => 2,
                    Some(v) => return Err(format!("bad axis={v}")),
                },
            },
            "skeleton_skull" => Block::Skull { rotation: int_of("rotation", 0, 0, 15)?, powered: bool_of("powered", false)? },
            "skeleton_wall_skull" => Block::WallSkull { facing: hdir_of("facing", Dir::North)?, powered: bool_of("powered", false)? },
            "cake" => Block::Cake { bites: int_of("bites", 0, 0, 6)? },
            "end_portal_frame" => Block::EndPortalFrame { facing: hdir_of("facing", Dir::North)?, eye: bool_of("eye", false)? },
            "composter" => Block::Composter { level: int_of("level", 0, 0, 8)? },
            "respawn_anchor" => Block::RespawnAnchor { charges: int_of("charges", 0, 0, 4)? },
            "piston" | "sticky_piston" => Block::Piston { sticky: name == "sticky_piston", facing: dir_of("facing", Dir::North)?, extended: bool_of("extended", false)? },
            "piston_head" => Block::PistonHead {
                sticky: match get("type") {
                    None | Some("normal") => false,
                    Some("sticky") => true,
                    Some(v) => return Err(format!("bad type={v}")),
                },
                facing: dir_of("facing", Dir::North)?,
                short: bool_of("short", false)?,
            },
            "moving_piston" => Block::MovingPiston {
                sticky: match get("type") {
                    None | Some("normal") => false,
                    Some("sticky") => true,
                    Some(v) => return Err(format!("bad type={v}")),
                },
                facing: dir_of("facing", Dir::North)?,
            },
            n if FALLING.contains(&n) => Block::Falling { id: FALLING.iter().position(|x| *x == n).unwrap() as u8 },
            "sculk_sensor" | "calibrated_sculk_sensor" => {
                waterlogged_ok()?;
                let calibrated = name == "calibrated_sculk_sensor";
                Block::Sculk {
                    calibrated,
                    facing: if calibrated { hdir_of("facing", Dir::North)? } else { Dir::North },
                    phase: match get("sculk_sensor_phase") {
                        None => 0,
                        Some(v) => SCULK_PHASES.iter().position(|p| *p == v).ok_or_else(|| format!("bad sculk_sensor_phase={v}"))? as u8,
                    },
                    power: int_of("power", 0, 0, 15)?,
                }
            }
            // ---- entity-reacting blocks (clk-misc) ----
            "oak_pressure_plate" | "stone_pressure_plate" => {
                Block::Plate { kind: if name == "oak_pressure_plate" { 0 } else { 1 }, power: if bool_of("powered", false)? { 15 } else { 0 } }
            }
            "light_weighted_pressure_plate" | "heavy_weighted_pressure_plate" => {
                Block::Plate { kind: if name == "light_weighted_pressure_plate" { 2 } else { 3 }, power: int_of("power", 0, 0, 15)? }
            }
            "tripwire" => Block::Tripwire {
                powered: bool_of("powered", false)?,
                attached: bool_of("attached", false)?,
                disarmed: bool_of("disarmed", false)?,
                north: bool_of("north", false)?,
                east: bool_of("east", false)?,
                south: bool_of("south", false)?,
                west: bool_of("west", false)?,
            },
            "tripwire_hook" => Block::TripwireHook { facing: hdir_of("facing", Dir::North)?, powered: bool_of("powered", false)?, attached: bool_of("attached", false)? },
            "white_wall_banner" => Block::WallBanner { facing: hdir_of("facing", Dir::North)? },
            "lantern" => {
                waterlogged_ok()?;
                Block::Lantern { hanging: bool_of("hanging", false)? }
            }
            // ---- placed light sources (terrain lane) ----
            "torch" | "soul_torch" => Block::PlainTorch { soul: name == "soul_torch" },
            "wall_torch" | "soul_wall_torch" => Block::PlainWallTorch { soul: name == "soul_wall_torch", facing: hdir_of("facing", Dir::North)? },
            "soul_lantern" => {
                waterlogged_ok()?;
                Block::SoulLantern { hanging: bool_of("hanging", false)? }
            }
            "jack_o_lantern" => Block::JackOLantern { facing: hdir_of("facing", Dir::North)? },
            "campfire" | "soul_campfire" => {
                waterlogged_ok()?;
                Block::Campfire { soul: name == "soul_campfire", lit: bool_of("lit", true)?, facing: hdir_of("facing", Dir::North)?, signal_fire: bool_of("signal_fire", false)? }
            }
            "end_rod" => Block::EndRod { facing: dir_of("facing", Dir::Up)? },
            "ochre_froglight" | "verdant_froglight" | "pearlescent_froglight" => Block::Froglight {
                kind: match name {
                    "ochre_froglight" => 0,
                    "verdant_froglight" => 1,
                    _ => 2,
                },
                axis: match get("axis").unwrap_or("y") {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    v => return Err(format!("bad axis={v} in {s:?}")),
                },
            },
            "sea_pickle" => Block::SeaPickle { pickles: int_of("pickles", 1, 1, 4)?, waterlogged: bool_of("waterlogged", true)? },
            "candle" => {
                waterlogged_ok()?;
                Block::Candle { candles: int_of("candles", 1, 1, 4)?, lit: bool_of("lit", false)? }
            }
            "stone_slab" => {
                let kind = match get("type") {
                    None | Some("bottom") => 0,
                    Some("top") => 1,
                    Some("double") => 2,
                    Some(v) => return Err(format!("bad type={v}")),
                };
                if bool_of("waterlogged", false)? {
                    Block::AnySlab { mat: crate::slab::index("stone_slab").unwrap_or(0), kind, waterlogged: true }
                } else {
                    Block::StoneSlab { kind }
                }
            }
            // ---- the slab family (terrain lane, the optimizer's day-1 materials) ----
            _ if crate::slab::index(name).is_some() => Block::AnySlab {
                mat: crate::slab::index(name).unwrap_or(0),
                kind: match get("type") {
                    None | Some("bottom") => 0,
                    Some("top") => 1,
                    Some("double") => 2,
                    Some(v) => return Err(format!("bad type={v}")),
                },
                waterlogged: bool_of("waterlogged", false)?,
            },
            "water" => Block::Water { level: int_of("level", 0, 0, 15)? },
            "bubble_column" => Block::BubbleColumn { drag: bool_of("drag", true)? },
            // ---- TNT dupers (track tnt): the dead fans' default state is waterlogged ----
            "tnt" => Block::Tnt { unstable: bool_of("unstable", false)? },
            n if n.starts_with("dead_") && n.ends_with("_coral_fan") && crate::tnt::DEAD_CORALS.contains(&&n[5..n.len() - 10]) => Block::CoralFan {
                kind: crate::tnt::DEAD_CORALS.iter().position(|k| *k == &n[5..n.len() - 10]).unwrap() as u8,
                waterlogged: bool_of("waterlogged", true)?,
            },
            n if n.starts_with("dead_") && n.ends_with("_coral_wall_fan") && crate::tnt::DEAD_CORALS.contains(&&n[5..n.len() - 15]) => Block::CoralWallFan {
                kind: crate::tnt::DEAD_CORALS.iter().position(|k| *k == &n[5..n.len() - 15]).unwrap() as u8,
                facing: hdir_of("facing", Dir::North)?,
                waterlogged: bool_of("waterlogged", true)?,
            },
            // ---- crop farms (webrender) ----
            "sugar_cane" => Block::SugarCane { age: int_of("age", 0, 0, 15)? },
            // ---- crop farms (clk-misc) ----
            "bamboo" => Block::Bamboo {
                age: int_of("age", 0, 0, 1)?,
                leaves: match get("leaves") {
                    None => 0,
                    Some(v) => crate::bamboo::LEAVES.iter().position(|l| *l == v).ok_or_else(|| format!("bad leaves={v} in {s:?}"))? as u8,
                },
                stage: int_of("stage", 0, 0, 1)?,
            },
            "bamboo_sapling" => Block::BambooSapling,
            "kelp" => Block::Kelp { age: int_of("age", 0, 0, 25)? },
            "kelp_plant" => Block::KelpPlant,
            // ---- stem farms (clk-misc) ----
            "farmland" => Block::Farmland { moisture: int_of("moisture", 0, 0, 7)? },
            "pumpkin_stem" | "melon_stem" => Block::Stem { melon: name == "melon_stem", age: int_of("age", 0, 0, 7)? },
            "attached_pumpkin_stem" | "attached_melon_stem" => Block::AttachedStem { melon: name == "attached_melon_stem", facing: hdir_of("facing", Dir::North)? },
            // ---- cactus farms (clk-misc) ----
            "cactus" => Block::Cactus { age: int_of("age", 0, 0, 15)? },
            "cactus_flower" => Block::CactusFlower,
            // ---- mob farms (ironfarm) ----
            "lava" => Block::Lava { level: int_of("level", 0, 0, 15)? },
            // ---- weather (mobinfra) ----
            n if Oxid::split(n).2 == "lightning_rod" => {
                waterlogged_ok()?;
                let (oxid, waxed, _) = Oxid::split(n);
                Block::LightningRod { facing: dir_of("facing", Dir::Up)?, powered: bool_of("powered", false)?, waxed, oxid }
            }
            // ---- fire (mobinfra) ----
            "fire" => {
                let mut sides = 0u8;
                for (k, key) in ["north", "east", "south", "west", "up"].iter().enumerate() {
                    if bool_of(key, false)? {
                        sides |= 1 << k;
                    }
                }
                Block::Fire { age: int_of("age", 0, 0, 15)?, sides, soul: false }
            }
            "soul_fire" => Block::Fire { age: 0, sides: 0, soul: true },
            "oak_sign" | "oak_wall_sign" => {
                if bool_of("waterlogged", false)? {
                    return Err(format!("a waterlogged sign is not modeled: {s:?}"));
                }
                let wall = name == "oak_wall_sign";
                Block::Sign { wall, rotation: if wall { 0 } else { int_of("rotation", 0, 0, 15)? }, facing: if wall { hdir_of("facing", Dir::North)? } else { Dir::North } }
            }
            n if n.ends_with("_bed") && DYES.contains(&&n[..n.len() - 4]) => Block::Bed {
                colour: DYES.iter().position(|k| *k == &n[..n.len() - 4]).unwrap() as u8,
                facing: hdir_of("facing", Dir::North)?,
                head: match get("part") {
                    None | Some("foot") => false,
                    Some("head") => true,
                    Some(v) => return Err(format!("bad part={v} in {s:?}")),
                },
                occupied: bool_of("occupied", false)?,
            },
            // ---- fences (clk-misc) ----
            n if n.ends_with("_fence") && crate::fence::FENCES.contains(&&n[..n.len() - 6]) => Block::Fence {
                kind: crate::fence::FENCES.iter().position(|k| *k == &n[..n.len() - 6]).unwrap() as u8,
                north: bool_of("north", false)?,
                east: bool_of("east", false)?,
                south: bool_of("south", false)?,
                west: bool_of("west", false)?,
                waterlogged: bool_of("waterlogged", false)?,
            },
            // ---- entity-reacting blocks (clk-misc): rails ----
            "rail" => {
                waterlogged_ok()?;
                Block::PlainRail {
                    shape: match get("shape") {
                        None => RailShape::NorthSouth,
                        Some(v) => RailShape::parse(v).ok_or_else(|| format!("bad shape={v}"))?,
                    },
                }
            }
            "detector_rail" => {
                waterlogged_ok()?;
                Block::DetectorRail {
                    shape: match get("shape") {
                        None => RailShape::NorthSouth,
                        Some(v) => RailShape::parse(v).filter(|s| !s.is_curve()).ok_or_else(|| format!("bad shape={v}"))?,
                    },
                    powered: bool_of("powered", false)?,
                }
            }
            // ---- shulker study (clk-rt) ----
            "shulker_box" => Block::ShulkerBox { colour: 0, facing: dir_of("facing", Dir::Up)? },
            n if n.ends_with("_shulker_box") && DYES.contains(&&n[..n.len() - 12]) => Block::ShulkerBox {
                colour: 1 + DYES.iter().position(|d| *d == &n[..n.len() - 12]).unwrap() as u8,
                facing: dir_of("facing", Dir::Up)?,
            },
            "chest" => {
                let kind = match get("type").unwrap_or("single") {
                    "single" => 0,
                    "left" => 1,
                    "right" => 2,
                    t => return Err(format!("chest type {t:?}: {s:?}")),
                };
                Block::Chest { facing: hdir_of("facing", Dir::North)?, kind, waterlogged: bool_of("waterlogged", false)? }
            }
            _ => return Err(format!("block {name:?} is not modeled by redsim")),
        })
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Block::Air => write!(f, "minecraft:air"),
            Block::Simple(s) => write!(f, "minecraft:{}{}", s.name(), if s == Simple::WaxedCopperGrate { "[waterlogged=false]" } else { "" }),
            Block::Glazed { facing } => write!(f, "minecraft:white_glazed_terracotta[facing={}]", facing.name()),
            Block::Wire { north, east, south, west, power } => write!(
                f,
                "minecraft:redstone_wire[east={},north={},power={power},south={},west={}]",
                east.name(),
                north.name(),
                south.name(),
                west.name()
            ),
            Block::RedstoneBlock => write!(f, "minecraft:redstone_block"),
            Block::Torch { lit } => write!(f, "minecraft:redstone_torch[lit={}]", b(lit)),
            Block::WallTorch { facing, lit } => write!(f, "minecraft:redstone_wall_torch[facing={},lit={}]", facing.name(), b(lit)),
            Block::Lever { face, facing, powered } => write!(f, "minecraft:lever[face={},facing={},powered={}]", face.name(), facing.name(), b(powered)),
            Block::Button { wood, face, facing, powered } => write!(
                f,
                "minecraft:{}[face={},facing={},powered={}]",
                if wood { "oak_button" } else { "stone_button" },
                face.name(),
                facing.name(),
                b(powered)
            ),
            Block::Repeater { facing, delay, locked, powered } => {
                write!(f, "minecraft:repeater[delay={delay},facing={},locked={},powered={}]", facing.name(), b(locked), b(powered))
            }
            Block::Comparator { facing, subtract, powered } => write!(
                f,
                "minecraft:comparator[facing={},mode={},powered={}]",
                facing.name(),
                if subtract { "subtract" } else { "compare" },
                b(powered)
            ),
            Block::Observer { facing, powered } => write!(f, "minecraft:observer[facing={},powered={}]", facing.name(), b(powered)),
            Block::Lamp { lit } => write!(f, "minecraft:redstone_lamp[lit={}]", b(lit)),
            Block::Bulb { oxid, waxed, lit, powered } => write!(f, "minecraft:{}{}copper_bulb[lit={},powered={}]", if waxed { "waxed_" } else { "" }, oxid.prefix(), b(lit), b(powered)),
            Block::Note { instrument, note, powered } => write!(f, "minecraft:note_block[instrument={},note={note},powered={}]", instrument.name(), b(powered)),
            Block::Target { power } => write!(f, "minecraft:target[power={power}]"),
            Block::Trapdoor { iron, facing, top, open, powered } => write!(
                f,
                "minecraft:{}[facing={},half={},open={},powered={},waterlogged=false]",
                if iron { "iron_trapdoor" } else { "oak_trapdoor" },
                facing.name(),
                if top { "top" } else { "bottom" },
                b(open),
                b(powered)
            ),
            Block::Door { iron, facing, upper, hinge_right, open, powered } => write!(
                f,
                "minecraft:{}[facing={},half={},hinge={},open={},powered={}]",
                if iron { "iron_door" } else { "oak_door" },
                facing.name(),
                if upper { "upper" } else { "lower" },
                if hinge_right { "right" } else { "left" },
                b(open),
                b(powered)
            ),
            Block::FenceGate { facing, in_wall, open, powered } => {
                write!(f, "minecraft:oak_fence_gate[facing={},in_wall={},open={},powered={}]", facing.name(), b(in_wall), b(open), b(powered))
            }
            Block::Rail { activator, shape, powered } => write!(
                f,
                "minecraft:{}[powered={},shape={},waterlogged=false]",
                if activator { "activator_rail" } else { "powered_rail" },
                b(powered),
                shape.name()
            ),
            Block::Dispenser { dropper, facing, triggered } => {
                write!(f, "minecraft:{}[facing={},triggered={}]", if dropper { "dropper" } else { "dispenser" }, facing.name(), b(triggered))
            }
            Block::Crafter { orientation, triggered, crafting } => {
                write!(f, "minecraft:crafter[crafting={},orientation={},triggered={}]", b(crafting), ORIENTATIONS[orientation as usize], b(triggered))
            }
            Block::Hopper { facing, enabled } => write!(f, "minecraft:hopper[enabled={},facing={}]", b(enabled), facing.name()),
            Block::Slab { kind } => write!(f, "minecraft:smooth_stone_slab[type={},waterlogged=false]", ["bottom", "top", "double"][kind as usize]),
            Block::Leaves { distance, persistent } => write!(f, "minecraft:oak_leaves[distance={distance},persistent={},waterlogged=false]", b(persistent)),
            Block::Scaffolding { distance, bottom } => write!(f, "minecraft:scaffolding[bottom={},distance={distance},waterlogged=false]", b(bottom)),
            Block::Log { axis } => write!(f, "minecraft:oak_log[axis={}]", ["x", "y", "z"][axis as usize]),
            Block::Skull { rotation, powered } => write!(f, "minecraft:skeleton_skull[powered={},rotation={rotation}]", b(powered)),
            Block::WallSkull { facing, powered } => write!(f, "minecraft:skeleton_wall_skull[facing={},powered={}]", facing.name(), b(powered)),
            Block::Cake { bites } => write!(f, "minecraft:cake[bites={bites}]"),
            Block::EndPortalFrame { facing, eye } => write!(f, "minecraft:end_portal_frame[eye={},facing={}]", b(eye), facing.name()),
            Block::Composter { level } => write!(f, "minecraft:composter[level={level}]"),
            Block::RespawnAnchor { charges } => write!(f, "minecraft:respawn_anchor[charges={charges}]"),
            Block::Piston { sticky, facing, extended } => {
                write!(f, "minecraft:{}[extended={},facing={}]", if sticky { "sticky_piston" } else { "piston" }, b(extended), facing.name())
            }
            Block::PistonHead { sticky, facing, short } => {
                write!(f, "minecraft:piston_head[facing={},short={},type={}]", facing.name(), b(short), if sticky { "sticky" } else { "normal" })
            }
            Block::MovingPiston { sticky, facing } => write!(f, "minecraft:moving_piston[facing={},type={}]", facing.name(), if sticky { "sticky" } else { "normal" }),
            Block::Falling { id } => write!(f, "minecraft:{}", FALLING[id as usize]),
            Block::Sculk { calibrated: false, power, phase, .. } => {
                write!(f, "minecraft:sculk_sensor[power={power},sculk_sensor_phase={},waterlogged=false]", SCULK_PHASES[phase as usize])
            }
            Block::Sculk { calibrated: true, facing, power, phase } => write!(
                f,
                "minecraft:calibrated_sculk_sensor[facing={},power={power},sculk_sensor_phase={},waterlogged=false]",
                facing.name(),
                SCULK_PHASES[phase as usize]
            ),
            // ---- entity-reacting blocks (clk-misc) ----
            Block::Plate { kind, power } => {
                let name = crate::react::plate::NAMES[kind as usize];
                if kind < 2 { write!(f, "minecraft:{name}[powered={}]", b(power > 0)) } else { write!(f, "minecraft:{name}[power={power}]") }
            }
            Block::Tripwire { powered, attached, disarmed, north, east, south, west } => write!(
                f,
                "minecraft:tripwire[attached={},disarmed={},east={},north={},powered={},south={},west={}]",
                b(attached),
                b(disarmed),
                b(east),
                b(north),
                b(powered),
                b(south),
                b(west)
            ),
            Block::TripwireHook { facing, powered, attached } => {
                write!(f, "minecraft:tripwire_hook[attached={},facing={},powered={}]", b(attached), facing.name(), b(powered))
            }
            Block::WallBanner { facing } => write!(f, "minecraft:white_wall_banner[facing={}]", facing.name()),
            Block::Lantern { hanging } => write!(f, "minecraft:lantern[hanging={},waterlogged=false]", b(hanging)),
            Block::PlainTorch { soul } => write!(f, "minecraft:{}torch", if soul { "soul_" } else { "" }),
            Block::PlainWallTorch { soul, facing } => write!(f, "minecraft:{}wall_torch[facing={}]", if soul { "soul_" } else { "" }, facing.name()),
            Block::SoulLantern { hanging } => write!(f, "minecraft:soul_lantern[hanging={},waterlogged=false]", b(hanging)),
            Block::JackOLantern { facing } => write!(f, "minecraft:jack_o_lantern[facing={}]", facing.name()),
            Block::Campfire { soul, lit, facing, signal_fire } => write!(f, "minecraft:{}campfire[facing={},lit={},signal_fire={},waterlogged=false]", if soul { "soul_" } else { "" }, facing.name(), b(lit), b(signal_fire)),
            Block::EndRod { facing } => write!(f, "minecraft:end_rod[facing={}]", facing.name()),
            Block::Froglight { kind, axis } => write!(f, "minecraft:{}_froglight[axis={}]", ["ochre", "verdant", "pearlescent"][kind as usize % 3], ["x", "y", "z"][axis as usize % 3]),
            Block::SeaPickle { pickles, waterlogged } => write!(f, "minecraft:sea_pickle[pickles={pickles},waterlogged={}]", b(waterlogged)),
            Block::Candle { candles, lit } => write!(f, "minecraft:candle[candles={candles},lit={},waterlogged=false]", b(lit)),
            Block::StoneSlab { kind } => write!(f, "minecraft:stone_slab[type={},waterlogged=false]", ["bottom", "top", "double"][kind as usize]),
            Block::AnySlab { mat, kind, waterlogged } => write!(f, "{}", crate::slab::state_name(mat, kind, waterlogged)),
            Block::PlainRail { shape } => write!(f, "minecraft:rail[shape={},waterlogged=false]", shape.name()),
            Block::DetectorRail { shape, powered } => write!(f, "minecraft:detector_rail[powered={},shape={},waterlogged=false]", b(powered), shape.name()),
            Block::BigDripleaf { facing, tilt, waterlogged } => write!(
                f,
                "minecraft:big_dripleaf[facing={},tilt={},waterlogged={}]",
                facing.name(),
                ["none", "unstable", "partial", "full"][tilt as usize],
                b(waterlogged)
            ),
            Block::ChiseledBookshelf { facing, slots } => {
                write!(f, "minecraft:chiseled_bookshelf[facing={}", facing.name())?;
                for n in 0..6 {
                    write!(f, ",slot_{n}_occupied={}", b(slots & (1 << n) != 0))?;
                }
                write!(f, "]")
            }
            Block::DaylightDetector { inverted, power } => write!(f, "minecraft:daylight_detector[inverted={},power={power}]", b(inverted)),
            Block::PowderSnow => write!(f, "minecraft:powder_snow"),
            Block::Jukebox { has_record } => write!(f, "minecraft:jukebox[has_record={}]", b(has_record)),
            Block::Furnace { facing, lit, blast } => write!(f, "minecraft:{}[facing={},lit={}]", if blast { "blast_furnace" } else { "furnace" }, facing.name(), b(lit)),
            Block::BrewingStand { bottles } => write!(f, "minecraft:brewing_stand[has_bottle_0={},has_bottle_1={},has_bottle_2={}]", b(bottles & 1 != 0), b(bottles & 2 != 0), b(bottles & 4 != 0)),
            Block::Water { level } => write!(f, "minecraft:water[level={level}]"),
            Block::BubbleColumn { drag } => write!(f, "minecraft:bubble_column[drag={}]", b(drag)),
            Block::Tnt { unstable } => write!(f, "minecraft:tnt[unstable={}]", b(unstable)),
            Block::CoralFan { kind, waterlogged } => write!(f, "minecraft:dead_{}_coral_fan[waterlogged={}]", crate::tnt::DEAD_CORALS[kind as usize], b(waterlogged)),
            Block::CoralWallFan { kind, facing, waterlogged } => {
                write!(f, "minecraft:dead_{}_coral_wall_fan[facing={},waterlogged={}]", crate::tnt::DEAD_CORALS[kind as usize], facing.name(), b(waterlogged))
            }
            Block::ShulkerBox { colour: 0, facing } => write!(f, "minecraft:shulker_box[facing={}]", facing.name()),
            Block::ShulkerBox { colour, facing } => write!(f, "minecraft:{}_shulker_box[facing={}]", DYES[colour as usize - 1], facing.name()),
            Block::Chest { facing, kind, waterlogged } => write!(f, "minecraft:chest[facing={},type={},waterlogged={}]", facing.name(), ["single", "left", "right"][kind as usize % 3], b(waterlogged)),
            Block::Fence { kind, north, east, south, west, waterlogged } => write!(
                f,
                "minecraft:{}_fence[east={},north={},south={},waterlogged={},west={}]",
                crate::fence::FENCES[kind as usize],
                b(east),
                b(north),
                b(south),
                b(waterlogged),
                b(west)
            ),
            Block::SugarCane { age } => write!(f, "minecraft:sugar_cane[age={age}]"),
            Block::WaterloggedGrate => write!(f, "minecraft:waxed_copper_grate[waterlogged=true]"),
            Block::Bamboo { age, leaves, stage } => write!(f, "minecraft:bamboo[age={age},leaves={},stage={stage}]", crate::bamboo::LEAVES[leaves as usize]),
            Block::BambooSapling => write!(f, "minecraft:bamboo_sapling"),
            Block::Kelp { age } => write!(f, "minecraft:kelp[age={age}]"),
            Block::KelpPlant => write!(f, "minecraft:kelp_plant"),
            Block::Farmland { moisture } => write!(f, "minecraft:farmland[moisture={moisture}]"),
            Block::Stem { melon, age } => write!(f, "minecraft:{}_stem[age={age}]", if melon { "melon" } else { "pumpkin" }),
            Block::AttachedStem { melon, facing } => write!(f, "minecraft:attached_{}_stem[facing={}]", if melon { "melon" } else { "pumpkin" }, facing.name()),
            Block::Cactus { age } => write!(f, "minecraft:cactus[age={age}]"),
            Block::CactusFlower => write!(f, "minecraft:cactus_flower"),
            Block::Lava { level } => write!(f, "minecraft:lava[level={level}]"),
            Block::Bed { colour, facing, head, occupied } => {
                write!(f, "minecraft:{}_bed[facing={},occupied={},part={}]", DYES[colour as usize % 16], facing.name(), b(occupied), if head { "head" } else { "foot" })
            }
            Block::Sign { wall: false, rotation, .. } => write!(f, "minecraft:oak_sign[rotation={rotation},waterlogged=false]"),
            Block::Sign { wall: true, facing, .. } => write!(f, "minecraft:oak_wall_sign[facing={},waterlogged=false]", facing.name()),
            Block::Terrain(id) => write!(f, "{}", crate::terrain::blocks::props(id).name),
            Block::LightningRod { facing, powered, waxed, oxid } => write!(f, "minecraft:{}{}lightning_rod[facing={},powered={},waterlogged=false]", if waxed { "waxed_" } else { "" }, oxid.prefix(), facing.name(), b(powered)),
            Block::Fire { soul: true, .. } => write!(f, "minecraft:soul_fire"),
            Block::Fire { age, sides, soul: false } => write!(
                f,
                "minecraft:fire[age={age},east={},north={},south={},up={},west={}]",
                b(sides & 2 != 0),
                b(sides & 1 != 0),
                b(sides & 4 != 0),
                b(sides & 16 != 0),
                b(sides & 8 != 0)
            ),
        }
    }
}
