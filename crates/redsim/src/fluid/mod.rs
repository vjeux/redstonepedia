//! Fluids (redsim M1, clockinfra; plan: designs/clocks/tracks/clockinfra/REDSIM-FLUIDS-ENTITIES.md):
//! water read in the 26.3 source, method by method with the game's names and order.
//!
//! * `FluidState` of a block ([`fluid_state_of`]): `LiquidBlock.getFluidState` (the legacy
//!   `level` 0 = source, 1..7 = flowing 8 - level, 8+ = falling), bubble columns = sources.
//! * Fluid ticks: their own tick list (`World::fluid_ticks`, keyed by the fluid TYPE: a tick for
//!   flowing water does not run on a source), run right after the block ticks
//!   (`ServerLevel.tick`: blockTicks then fluidTicks), sharing the sub-tick counter.
//! * `FlowingFluid.tick / spread / spreadToSides / getNewLiquid / getSpread / getSlopeDistance /
//!   isWaterHole / canPassThroughWall` (`Shapes.mergedFaceOccludes` on the collision shapes) and
//!   `WaterFluid` (tick delay 5, drop-off 1, slope distance 4, source conversion on: the game
//!   rule's default). Water flows into air, water and the `#washed_away_by_fluids` blocks
//!   (dust, diodes, torches, levers, buttons, skulls, rails...), dropping what it washes away.
//! * `LiquidBlock`: onPlace / neighborChanged schedule a fluid tick, updateShape when either side
//!   is a source; a full source above soul sand / magma schedules a 20 gt block tick that turns
//!   it into a bubble column (`BubbleColumnBlock.updateColumn`, flags 2, up the water column).
//! * NOT modeled (a model error is recorded): waterlogging (redsim's waterloggable blocks never
//!   hold water).
//! * Lava (mobinfra 2026-11-08, the parent: "model the lava flowing out"): the same FlowingFluid
//!   methods with `LavaFluid`'s numbers and overrides, by the fluid that ticks (`kind`): tick
//!   delay 30 (the overworld), drop-off 2, slope distance 2, no source conversion
//!   (`lava_source_conversion` off), `getSpreadDelay` (x4 with 3 in 4 when a cell rises, the level
//!   random), `canBeReplacedWith` (by water, at height 4/9 and up), `spreadTo` (down into water:
//!   stone), `beforeDestroyingBlock` (no drops), and `LiquidBlock.shouldSpreadLiquid` (water beside
//!   lava: obsidian from a source, cobblestone from flowing lava).

use crate::behavior as bh;
use crate::block::{Block, Kind, Simple};
use crate::entity::Vec3;
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// The fluid types (`Fluids`): `Water` / `Lava` = the source types, `FlowingWater` /
/// `FlowingLava` = the flowing types.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Fluid {
    Empty,
    Water,
    FlowingWater,
    Lava,
    FlowingLava,
}

impl Fluid {
    /// `isSame(WATER)` / `is(FluidTags.WATER)`.
    pub fn is_water(self) -> bool {
        matches!(self, Fluid::Water | Fluid::FlowingWater)
    }

    /// `isSame(LAVA)` / `is(FluidTags.LAVA)`.
    pub fn is_lava(self) -> bool {
        matches!(self, Fluid::Lava | Fluid::FlowingLava)
    }

    /// `Fluid.isSame(other)`: both water or both lava.
    pub fn same(self, other: Fluid) -> bool {
        (self.is_water() && other.is_water()) || (self.is_lava() && other.is_lava())
    }

    /// `FlowingFluid.getSource()`.
    pub fn source(self) -> Fluid {
        if self.is_lava() { Fluid::Lava } else { Fluid::Water }
    }

    /// `FlowingFluid.getFlowing()`.
    pub fn flowing(self) -> Fluid {
        if self.is_lava() { Fluid::FlowingLava } else { Fluid::FlowingWater }
    }
}

/// `FluidState`: the fluid type, its amount (8 for a source; 1..8 flowing) and `FALLING`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FluidState {
    pub fluid: Fluid,
    pub amount: u8,
    pub falling: bool,
}

/// `WaterFluid.getTickDelay`.
pub const TICK_DELAY: i32 = 5;
/// `WaterFluid.getDropOff`.
const DROP_OFF: i32 = 1;
/// `WaterFluid.getSlopeFindDistance`.
const SLOPE_FIND_DISTANCE: i32 = 4;
/// `GameRules.WATER_SOURCE_CONVERSION` (default true).
const CAN_CONVERT_TO_SOURCE: bool = true;
/// `LavaFluid.getTickDelay` (a dimension that is not ultra-warm: the overworld).
pub const LAVA_TICK_DELAY: i32 = 30;

/// `FlowingFluid.getTickDelay` of a fluid (lava every 10 gt in an ultra-warm dimension: the Nether).
pub fn tick_delay(w: &World, f: Fluid) -> i32 {
    if f.is_lava() { if w.ultrawarm { 10 } else { LAVA_TICK_DELAY } } else { TICK_DELAY }
}

/// `getDropOff` (lava 2 outside ultra-warm dimensions, 1 in them).
fn drop_off(w: &World, f: Fluid) -> i32 {
    if f.is_lava() { if w.ultrawarm { 1 } else { 2 } } else { DROP_OFF }
}

/// `getSlopeFindDistance` (lava 2 outside ultra-warm dimensions, 4 in them).
fn slope_find_distance(w: &World, f: Fluid) -> i32 {
    if f.is_lava() { if w.ultrawarm { 4 } else { 2 } } else { SLOPE_FIND_DISTANCE }
}

/// `canConvertToSource` (`GameRules.LAVA_SOURCE_CONVERSION` default false).
fn can_convert_to_source(f: Fluid) -> bool {
    if f.is_lava() { false } else { CAN_CONVERT_TO_SOURCE }
}

impl FluidState {
    pub const EMPTY: FluidState = FluidState { fluid: Fluid::Empty, amount: 0, falling: false };

    /// `getSource(falling)` (water).
    pub fn source(falling: bool) -> FluidState {
        FluidState { fluid: Fluid::Water, amount: 8, falling }
    }

    /// `getFlowing(amount, falling)` (water).
    pub fn flowing(amount: u8, falling: bool) -> FluidState {
        FluidState { fluid: Fluid::FlowingWater, amount, falling }
    }

    /// `getSource(falling)` of fluid `f`'s kind.
    pub fn source_of(f: Fluid, falling: bool) -> FluidState {
        FluidState { fluid: f.source(), amount: 8, falling }
    }

    /// `getFlowing(amount, falling)` of fluid `f`'s kind.
    pub fn flowing_of(f: Fluid, amount: u8, falling: bool) -> FluidState {
        FluidState { fluid: f.flowing(), amount, falling }
    }

    pub fn is_empty(self) -> bool {
        self.fluid == Fluid::Empty
    }

    pub fn is_source(self) -> bool {
        matches!(self.fluid, Fluid::Water | Fluid::Lava)
    }

    /// `isFull`: amount 8.
    pub fn is_full(self) -> bool {
        self.amount == 8
    }

    /// `getOwnHeight`: amount / 9.0F.
    pub fn own_height(self) -> f32 {
        if self.is_empty() { 0.0 } else { self.amount as f32 / 9.0 }
    }

    /// `getHeight(level, pos)`: 1 with the same fluid above, else its own height.
    pub fn height(self, w: &World, p: BlockPos) -> f32 {
        if self.is_empty() {
            0.0
        } else if fluid_state(w, p.above()).fluid.same(self.fluid) {
            1.0
        } else {
            self.own_height()
        }
    }

    /// `FlowingFluid.getLegacyLevel`.
    pub fn legacy_level(self) -> u8 {
        if self.is_source() { 0 } else { 8 - self.amount.min(8) + if self.falling { 8 } else { 0 } }
    }

    /// `createLegacyBlock`: the water or lava block of this state (air for no fluid).
    pub fn create_legacy_block(self) -> Block {
        if self.is_empty() {
            Block::Air
        } else if self.fluid.is_lava() {
            Block::Lava { level: self.legacy_level() }
        } else {
            Block::Water { level: self.legacy_level() }
        }
    }
}

/// `BlockState.getFluidState`.
pub fn fluid_state_of(b: &Block) -> FluidState {
    match *b {
        Block::Water { level: 0 } => FluidState::source(false),
        Block::Water { level } if level < 8 => FluidState::flowing(8 - level, false),
        Block::Water { .. } => FluidState::flowing(8, true),
        // ---- lava (mobinfra): LiquidBlock's legacy level, as water's ----
        Block::Lava { level: 0 } => FluidState::source_of(Fluid::Lava, false),
        Block::Lava { level } if level < 8 => FluidState::flowing_of(Fluid::Lava, 8 - level, false),
        Block::Lava { .. } => FluidState::flowing_of(Fluid::Lava, 8, true),
        Block::BubbleColumn { .. } => FluidState::source(false),
        // ---- crop farms (clk-misc): kelp holds a water source ----
        Block::Kelp { .. } | Block::KelpPlant => FluidState::source(false),
        // ---- terrain: a natural state holding water (seagrass, waterlogged leaves...) or lava:
        // the game's own fluid state (the natural-state table) ----
        Block::Terrain(id) => {
            let p = crate::terrain::blocks::props(id);
            match p.fluid {
                crate::terrain::blocks::FluidKind::Water if p.fluid_source => FluidState::source(false),
                crate::terrain::blocks::FluidKind::Water => FluidState::flowing(p.fluid_amount.clamp(1, 8), false),
                crate::terrain::blocks::FluidKind::Lava if p.fluid_source => FluidState::source_of(Fluid::Lava, false),
                crate::terrain::blocks::FluidKind::Lava => FluidState::flowing_of(Fluid::Lava, p.fluid_amount.clamp(1, 8), false),
                _ => FluidState::EMPTY,
            }
        }
        _ if waterlogged(b) == Some(true) => FluidState::source(false),
        _ => FluidState::EMPTY,
    }
}

/// The `WATERLOGGED` property of a block whose variant carries one (`SimpleWaterloggedBlock`s
/// modeled with their water: add each such variant here and in [`set_waterlogged`]). redsim's
/// older waterloggable blocks (slabs, trapdoors, rails, scaffolding, leaves, sculk sensors) carry
/// none: they are always dry, and water reaching them is a model note.
pub fn waterlogged(b: &Block) -> Option<bool> {
    match *b {
        // ---- entity-reacting blocks (clk-misc): their waterlogged variants ----
        Block::BigDripleaf { waterlogged, .. } => Some(waterlogged),
        // ---- TNT dupers (track tnt) ----
        Block::CoralFan { waterlogged, .. } | Block::CoralWallFan { waterlogged, .. } => Some(waterlogged),
        // ---- fences (clk-misc) ----
        Block::Fence { waterlogged, .. } => Some(waterlogged),
        // ---- crop farms (webrender): the waterlogged grate (a dry grate stays a plain block) ----
        Block::WaterloggedGrate => Some(true),
        // ---- placed light sources (terrain lane): sea pickles carry their water ----
        Block::SeaPickle { waterlogged, .. } => Some(waterlogged),
        // ---- the slab family (terrain lane); smooth stone's and stone's dry slabs take water too
        // (their waterlogged states are `AnySlab`s; a double slab never) ----
        Block::AnySlab { waterlogged, .. } => Some(waterlogged),
        Block::Slab { kind } | Block::StoneSlab { kind } if kind < 2 => Some(false),
        _ => None,
    }
}

/// `state.setValue(WATERLOGGED, v)` (None: the variant has no such property).
pub fn set_waterlogged(b: Block, v: bool) -> Option<Block> {
    let _ = v;
    match b {
        // ---- entity-reacting blocks (clk-misc) ----
        Block::BigDripleaf { facing, tilt, .. } => Some(Block::BigDripleaf { facing, tilt, waterlogged: v }),
        // ---- TNT dupers (track tnt) ----
        Block::CoralFan { kind, .. } => Some(Block::CoralFan { kind, waterlogged: v }),
        Block::CoralWallFan { kind, facing, .. } => Some(Block::CoralWallFan { kind, facing, waterlogged: v }),
        // ---- fences (clk-misc) ----
        Block::Fence { kind, north, east, south, west, .. } => Some(Block::Fence { kind, north, east, south, west, waterlogged: v }),
        // ---- crop farms (webrender) ----
        Block::WaterloggedGrate => Some(if v { Block::WaterloggedGrate } else { Block::Simple(crate::block::Simple::WaxedCopperGrate) }),
        // ---- placed light sources (terrain lane) ----
        Block::SeaPickle { pickles, .. } => Some(Block::SeaPickle { pickles, waterlogged: v }),
        // ---- the slab family (terrain lane): smooth stone's and stone's dry slabs keep their own
        // variants ----
        Block::AnySlab { mat, kind, .. } => Some(match (crate::slab::material(mat), v) {
            ("smooth_stone", false) => Block::Slab { kind },
            ("stone", false) => Block::StoneSlab { kind },
            _ => Block::AnySlab { mat, kind, waterlogged: v },
        }),
        Block::Slab { kind } if v => Some(Block::AnySlab { mat: crate::slab::index("smooth_stone_slab").unwrap_or(0), kind, waterlogged: true }),
        Block::StoneSlab { kind } if v => Some(Block::AnySlab { mat: crate::slab::index("stone_slab").unwrap_or(0), kind, waterlogged: true }),
        Block::Slab { .. } | Block::StoneSlab { .. } => Some(b),
        _ => None,
    }
}

/// The water tick a waterlogged block schedules from its `updateShape`
/// (`if (state.getValue(WATERLOGGED)) ticks.scheduleTick(pos, Fluids.WATER, 5)`): call it first
/// in the updateShape of every variant [`waterlogged`] knows.
pub fn waterlogged_update_shape(w: &mut World, s: &Block, p: BlockPos) {
    if waterlogged(s) == Some(true) {
        w.schedule_fluid_tick(p, Fluid::Water, TICK_DELAY);
    }
}

/// The fluid state of the block at `p` (`Level.getFluidState`).
pub fn fluid_state(w: &World, p: BlockPos) -> FluidState {
    fluid_state_of(&w.get(p))
}

// ---------------------------------------------------------------------- what holds water

/// `instanceof LiquidBlockContainer`: the variants with a waterlogged property, and redsim's older
/// waterloggable blocks (always dry: see [`waterlogged`]).
fn is_container(b: &Block) -> bool {
    waterlogged(b).is_some()
        || matches!(b, Block::Slab { .. } | Block::Trapdoor { .. } | Block::Rail { .. } | Block::Scaffolding { .. } | Block::Leaves { .. } | Block::Sculk { .. })
        // ---- entity-reacting blocks (clk-misc): waterloggable, modeled dry ----
        || matches!(b, Block::PlainRail { .. } | Block::DetectorRail { .. } | Block::StoneSlab { .. } | Block::Lantern { .. })
        // ---- crop farms (clk-misc): kelp, a LiquidBlockContainer that takes no liquid ----
        || matches!(b, Block::Kelp { .. } | Block::KelpPlant)
        // ---- mob farms (ironfarm): a sign (SimpleWaterloggedBlock), modeled dry ----
        || matches!(b, Block::Sign { .. })
        // ---- placed light sources (terrain lane): waterloggable, modeled dry (a sea pickle
        // carries its water: `waterlogged`) ----
        || matches!(b, Block::SoulLantern { .. } | Block::Campfire { .. } | Block::Candle { .. })
}

/// `state.is(BlockTags.WASHED_AWAY_BY_FLUIDS)` among redsim's blocks.
fn washed_away(b: &Block) -> bool {
    matches!(
        b,
        Block::Air
            | Block::Water { .. }
            | Block::Wire { .. }
            | Block::Torch { .. }
            | Block::WallTorch { .. }
            | Block::Lever { .. }
            | Block::Button { .. }
            | Block::Repeater { .. }
            | Block::Comparator { .. }
            | Block::Skull { .. }
            | Block::WallSkull { .. }
            | Block::Rail { .. }
            | Block::Scaffolding { .. }
            // ---- entity-reacting blocks (clk-misc) ----
            | Block::PlainRail { .. }
            | Block::DetectorRail { .. }
            | Block::Tripwire { .. }
            | Block::TripwireHook { .. }
            | Block::BigDripleaf { .. }
            | Block::PowderSnow
            // ---- fire (mobinfra): `#fire` is washed away ----
            | Block::Fire { .. }
            // ---- placed light sources (terrain lane): torches, end rods, `#candles` ----
            | Block::PlainTorch { .. }
            | Block::PlainWallTorch { .. }
            | Block::EndRod { .. }
            | Block::Candle { .. }
            // ---- mobinfra: lava is in the tag too (a flow passes through flowing lava) ----
            | Block::Lava { .. }
    ) || matches!(b, Block::Terrain(id) if natural_washed_away(&crate::terrain::blocks::props(*id).name))
}

/// `#washed_away_by_fluids` (26.3's tag, its nested tags resolved) for a natural state of the
/// terrain lane's table (its game string): grass, flowers, lichen, vines, saplings, snow...
pub fn natural_washed_away(state: &str) -> bool {
    let name = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    const LISTED: &[&str] = &[
        "fire", "soul_fire", "air", "void_air", "cave_air", "flower_pot", "rail", "powered_rail", "detector_rail", "activator_rail", "player_head", "creeper_head", "zombie_head", "skeleton_skull",
        "wither_skeleton_skull", "dragon_head", "piglin_head", "azalea", "flowering_azalea", "mangrove_propagule", "candle", "water", "lava", "tall_seagrass", "wall_torch", "redstone_wall_torch",
        "soul_wall_torch", "copper_wall_torch", "attached_pumpkin_stem", "attached_melon_stem", "kelp_plant", "weeping_vines_plant", "twisting_vines_plant", "cave_vines_plant", "big_dripleaf_stem",
        "skeleton_wall_skull", "wither_skeleton_wall_skull", "zombie_wall_head", "player_wall_head", "creeper_wall_head", "dragon_wall_head", "piglin_wall_head", "bamboo_sapling", "cobweb", "snow",
        "end_rod", "chorus_plant", "chorus_flower", "big_dripleaf", "scaffolding", "powder_snow", "short_grass", "fern", "dead_bush", "bush", "short_dry_grass", "tall_dry_grass", "seagrass",
        "dandelion", "golden_dandelion", "torchflower", "poppy", "blue_orchid", "allium", "azure_bluet", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "oxeye_daisy", "cornflower",
        "wither_rose", "lily_of_the_valley", "brown_mushroom", "red_mushroom", "torch", "redstone_wire", "wheat", "lever", "redstone_torch", "cactus_flower", "soul_torch", "copper_torch",
        "pumpkin_stem", "melon_stem", "vine", "glow_lichen", "resin_clump", "nether_wart", "tripwire_hook", "tripwire", "carrots", "potatoes", "light", "sunflower", "lilac", "rose_bush", "peony",
        "tall_grass", "large_fern", "torchflower_crop", "pitcher_crop", "pitcher_plant", "beetroots", "kelp", "sweet_berry_bush", "warped_fungus", "warped_roots", "nether_sprouts", "crimson_fungus",
        "weeping_vines", "twisting_vines", "crimson_roots", "cave_vines", "spore_blossom", "pink_petals", "wildflowers", "leaf_litter", "small_dripleaf", "hanging_roots", "frogspawn",
        "pale_hanging_moss", "open_eyeblossom", "closed_eyeblossom", "firefly_bush", "repeater", "lily_pad", "cocoa", "comparator", "sea_pickle", "moss_carpet", "heavy_core", "pale_moss_carpet",
        "red_shrub", "shelf_mushroom", "straw_bed",
    ];
    LISTED.contains(&name)
        // #saplings, #wool_carpets, #candles, #buttons, #flower_pots' potted plants, the golem statues
        || name.ends_with("_sapling")
        || name.ends_with("_carpet")
        || name.ends_with("_candle")
        || name.ends_with("_button")
        || name.starts_with("potted_")
        || name.ends_with("copper_golem_statue")
        // #corals (the coral plants and fans) and #wall_corals, never the blocks
        || ((name.ends_with("_coral") || name.ends_with("_coral_fan") || name.ends_with("_coral_wall_fan")) && !name.starts_with("dead_"))
}

/// `FlowingFluid.canHoldAnyFluid`.
fn can_hold_any_fluid(b: &Block) -> bool {
    is_container(b) || washed_away(b)
}

/// `FlowingFluid.canHoldSpecificFluid`: a waterloggable block takes only a water SOURCE
/// (`SimpleWaterloggedBlock.canPlaceLiquid`; a double slab none).
fn can_hold_specific_fluid(b: &Block, f: Fluid) -> bool {
    if !is_container(b) {
        return true;
    }
    if matches!(b, Block::Slab { kind: 2 } | Block::StoneSlab { kind: 2 } | Block::AnySlab { kind: 2, .. }) {
        return false;
    }
    // kelp: canPlaceLiquid false
    if matches!(b, Block::Kelp { .. } | Block::KelpPlant) {
        return false;
    }
    f == Fluid::Water
}

fn can_hold_fluid(b: &Block, f: Fluid) -> bool {
    can_hold_any_fluid(b) && can_hold_specific_fluid(b, f)
}

/// `FluidState.canBeReplacedWith(level, pos, other, direction)` of the state `s` at `p`: no fluid
/// always; water only from above by another fluid (never by water); lava by water once it is
/// 4/9 high (`LavaFluid.canBeReplacedWith`).
fn can_be_replaced_with(w: &World, p: BlockPos, s: FluidState, other: Fluid, d: Dir) -> bool {
    match s.fluid {
        Fluid::Empty => true,
        f if f.is_lava() => s.height(w, p) >= 0.44444445 && other.is_water(),
        _ => d == Dir::Down && !other.is_water(),
    }
}

/// `isSourceBlockOfThisType` for the fluid `kind` that ticks.
fn is_source_block_of_this_type(kind: Fluid, s: FluidState) -> bool {
    s.fluid.same(kind) && s.is_source()
}

// ---------------------------------------------------------------------- shapes

/// The collision shape (empty context) as boxes in 1/16 units: water and bubble columns have
/// none; pistons come from `piston::shape`.
fn collision_shape(w: &World, b: &Block, p: BlockPos) -> Vec<[i32; 6]> {
    match b {
        Block::Water { .. } | Block::BubbleColumn { .. } => vec![],
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::shape::of(w, *b, p).into_iter().map(|x| x.0).collect(),
        _ => bh::collision_boxes(b),
    }
}

fn is_full_block(s: &[[i32; 6]]) -> bool {
    s.len() == 1 && s[0] == [0, 16, 0, 16, 0, 16]
}

/// (axis index into a box: 0 x, 2 y, 4 z; the two other axes) of a direction.
fn axes(d: Dir) -> (usize, usize, usize) {
    match d {
        Dir::Down | Dir::Up => (2, 0, 4),
        Dir::North | Dir::South => (4, 0, 2),
        Dir::West | Dir::East => (0, 2, 4),
    }
}

/// `Shapes.mergedFaceOccludes(shape, occluder, direction)`: the face of `shape` toward `d`
/// (its last voxel layer, if it reaches the cell's side) and the opposite face of `occluder`
/// together cover the whole side.
fn merged_face_occludes(shape: &[[i32; 6]], occluder: &[[i32; 6]], d: Dir) -> bool {
    if is_full_block(shape) && is_full_block(occluder) {
        return true;
    }
    if occluder.is_empty() && shape.is_empty() {
        return false;
    }
    let positive = matches!(d, Dir::Up | Dir::South | Dir::East);
    let (first, second) = if positive { (shape, occluder) } else { (occluder, shape) };
    let (a, u, v) = axes(d);
    let first_max = first.iter().map(|b| b[a + 1]).max();
    let second_min = second.iter().map(|b| b[a]).min();
    let mut covered = [[false; 16]; 16];
    let mut mark = |b: &[i32; 6]| {
        for x in b[u].max(0)..b[u + 1].min(16) {
            for y in b[v].max(0)..b[v + 1].min(16) {
                covered[x as usize][y as usize] = true;
            }
        }
    };
    if first_max == Some(16) {
        for b in first.iter().filter(|b| b[a + 1] == 16) {
            mark(b);
        }
    }
    if second_min == Some(0) {
        for b in second.iter().filter(|b| b[a] == 0) {
            mark(b);
        }
    }
    covered.iter().all(|r| r.iter().all(|c| *c))
}

/// `FlowingFluid.canPassThroughWall(direction, level, sourcePos, sourceState, targetPos,
/// targetState)`.
fn can_pass_through_wall(d: Dir, w: &World, sp: BlockPos, ss: &Block, tp: BlockPos, ts: &Block) -> bool {
    let target = collision_shape(w, ts, tp);
    if is_full_block(&target) {
        return false;
    }
    let source = collision_shape(w, ss, sp);
    if is_full_block(&source) {
        return false;
    }
    if source.is_empty() && target.is_empty() {
        return true;
    }
    !merged_face_occludes(&source, &target, d)
}

// ---------------------------------------------------------------------- FlowingFluid

fn can_maybe_pass_through(w: &World, kind: Fluid, sp: BlockPos, ss: &Block, d: Dir, tp: BlockPos, ts: &Block, tf: FluidState) -> bool {
    !is_source_block_of_this_type(kind, tf) && can_hold_any_fluid(ts) && can_pass_through_wall(d, w, sp, ss, tp, ts)
}

fn can_pass_through(w: &World, f: Fluid, sp: BlockPos, ss: &Block, d: Dir, tp: BlockPos, ts: &Block, tf: FluidState) -> bool {
    can_maybe_pass_through(w, f, sp, ss, d, tp, ts, tf) && can_hold_specific_fluid(ts, f)
}

/// `FlowingFluid.isWaterHole` (for the fluid `kind` that ticks).
fn is_water_hole(w: &World, kind: Fluid, top: BlockPos, top_state: &Block, bottom: BlockPos, bottom_state: &Block) -> bool {
    if !can_pass_through_wall(Dir::Down, w, top, top_state, bottom, bottom_state) {
        return false;
    }
    if fluid_state_of(bottom_state).fluid.same(kind) { true } else { can_hold_fluid(bottom_state, kind.flowing()) }
}

fn is_hole(w: &World, kind: Fluid, p: BlockPos) -> bool {
    let s = w.get(p);
    let below = p.below();
    is_water_hole(w, kind, p, &s, below, &w.get(below))
}

/// `FlowingFluid.sourceNeighborCount`.
fn source_neighbor_count(w: &World, kind: Fluid, p: BlockPos) -> i32 {
    Dir::HORIZONTAL.iter().filter(|d| is_source_block_of_this_type(kind, fluid_state(w, p.rel(**d)))).count() as i32
}

/// `FlowingFluid.getNewLiquid(level, pos, state)` of water ([`get_new_liquid_of`] for a kind).
pub fn get_new_liquid(w: &World, p: BlockPos, s: &Block) -> FluidState {
    get_new_liquid_of(w, Fluid::Water, p, s)
}

/// `FlowingFluid.getNewLiquid(level, pos, state)` for the fluid `kind` that ticks.
pub fn get_new_liquid_of(w: &World, kind: Fluid, p: BlockPos, s: &Block) -> FluidState {
    let mut highest = 0i32;
    let mut sources = 0;
    for d in Dir::HORIZONTAL {
        let rp = p.rel(d);
        let bs = w.get(rp);
        let f = fluid_state_of(&bs);
        if f.fluid.same(kind) && can_pass_through_wall(d, w, p, s, rp, &bs) {
            if f.is_source() {
                sources += 1;
            }
            highest = highest.max(f.amount as i32);
        }
    }
    if sources >= 2 && can_convert_to_source(kind) {
        let bs = w.get(p.below());
        let bf = fluid_state_of(&bs);
        if bh::is_solid(&bs) || is_source_block_of_this_type(kind, bf) {
            return FluidState::source_of(kind, false);
        }
    }
    let ap = p.above();
    let a = w.get(ap);
    let af = fluid_state_of(&a);
    if !af.is_empty() && af.fluid.same(kind) && can_pass_through_wall(Dir::Up, w, p, s, ap, &a) {
        return FluidState::flowing_of(kind, 8, true);
    }
    let amount = highest - drop_off(w, kind);
    if amount <= 0 { FluidState::EMPTY } else { FluidState::flowing_of(kind, amount as u8, false) }
}

/// `FlowingFluid.getSlopeDistance` (the `SpreadContext` caches change no result: the world does
/// not change during the search).
fn get_slope_distance(w: &World, kind: Fluid, p: BlockPos, pass: i32, from: Dir, s: &Block) -> i32 {
    let mut lowest = 1000;
    for d in Dir::HORIZONTAL {
        if d == from {
            continue;
        }
        let tp = p.rel(d);
        let ts = w.get(tp);
        let tf = fluid_state_of(&ts);
        if can_pass_through(w, kind.flowing(), p, s, d, tp, &ts, tf) {
            if is_hole(w, kind, tp) {
                return pass;
            }
            if pass < slope_find_distance(w, kind) {
                let v = get_slope_distance(w, kind, tp, pass + 1, d.opposite(), &ts);
                if v < lowest {
                    lowest = v;
                }
            }
        }
    }
    lowest
}

/// `FlowingFluid.getSpread`: the sides to spread to with their new states, in the `EnumMap`'s
/// order (Direction ordinals: north, south, west, east).
fn get_spread(w: &World, kind: Fluid, p: BlockPos, s: &Block) -> Vec<(Dir, FluidState)> {
    let mut lowest = 1000;
    let mut result: [Option<FluidState>; 6] = [None; 6];
    for d in Dir::HORIZONTAL {
        let tp = p.rel(d);
        let ts = w.get(tp);
        let tf = fluid_state_of(&ts);
        if can_maybe_pass_through(w, kind, p, s, d, tp, &ts, tf) {
            let nf = get_new_liquid_of(w, kind, tp, &ts);
            if can_hold_specific_fluid(&ts, nf.fluid) {
                let distance = if is_hole(w, kind, tp) { 0 } else { get_slope_distance(w, kind, tp, 1, d.opposite(), &ts) };
                if distance < lowest {
                    result = [None; 6];
                }
                if distance <= lowest {
                    if can_be_replaced_with(w, tp, tf, nf.fluid, d) {
                        result[d as usize] = Some(nf);
                    }
                    lowest = distance;
                }
            }
        }
    }
    [Dir::North, Dir::South, Dir::West, Dir::East].into_iter().filter_map(|d| result[d as usize].map(|f| (d, f))).collect()
}

/// `FlowingFluid.spreadTo` for the fluid `kind` that ticks: `LavaFluid.spreadTo` first (lava
/// going down into water makes stone of a liquid block, fizzing, and goes no further); a
/// waterloggable block takes a water source (`SimpleWaterloggedBlock.placeLiquid`; lava never);
/// `beforeDestroyingBlock`: water drops what it washes away, lava destroys it without drops.
fn spread_to(w: &mut World, kind: Fluid, p: BlockPos, s: Block, d: Dir, target: FluidState) {
    if kind.is_lava() && d == Dir::Down && fluid_state(w, p).fluid.is_water() {
        if matches!(s, Block::Water { .. }) {
            w.set_block_and_update(p, Block::Simple(Simple::Stone));
        }
        return;
    }
    if is_container(&s) {
        match waterlogged(&s) {
            Some(false) if target.fluid == Fluid::Water => {
                if let Some(wet) = set_waterlogged(s, true) {
                    w.set_block_and_update(p, wet);
                    w.schedule_fluid_tick(p, target.fluid, TICK_DELAY);
                }
            }
            Some(_) => {}
            None if target.fluid == Fluid::Water => w.errors.push(format!("water would waterlog {s} at {p}: waterlogging is not modeled for this block")),
            None => {}
        }
        return;
    }
    if !s.is_air() && kind.is_water() && bh::drops_item(&s) {
        // beforeDestroyingBlock -> Block.dropResources (its loot's stacks pop)
        w.drop_resources(p, s);
        w.drops.push((p, s));
    }
    w.set_block_and_update(p, target.create_legacy_block());
}

/// `FlowingFluid.spreadToSides`.
fn spread_to_sides(w: &mut World, p: BlockPos, f: FluidState, s: &Block) {
    let kind = f.fluid;
    let mut neighbor = f.amount as i32 - drop_off(w, kind);
    if f.falling {
        neighbor = 7;
    }
    if neighbor > 0 {
        for (d, nf) in get_spread(w, kind, p, s) {
            let np = p.rel(d);
            let ns = w.get(np);
            spread_to(w, kind, np, ns, d, nf);
        }
    }
}

/// `FlowingFluid.spread`.
fn spread(w: &mut World, p: BlockPos, s: Block, f: FluidState) {
    if f.is_empty() {
        return;
    }
    let kind = f.fluid;
    let below = p.below();
    let bs = w.get(below);
    let bf = fluid_state_of(&bs);
    if can_maybe_pass_through(w, kind, p, &s, Dir::Down, below, &bs, bf) {
        let nb = get_new_liquid_of(w, kind, below, &bs);
        if can_be_replaced_with(w, below, bf, nb.fluid, Dir::Down) && can_hold_specific_fluid(&bs, nb.fluid) {
            spread_to(w, kind, below, bs, Dir::Down, nb);
            if source_neighbor_count(w, kind, p) >= 3 {
                spread_to_sides(w, p, f, &s);
            }
            return;
        }
    }
    if f.is_source() || !is_water_hole(w, kind, p, &s, below, &bs) {
        spread_to_sides(w, p, f, &s);
    }
}

/// `FlowingFluid.getSpreadDelay`: the tick delay; lava's x4 when the cell rises (neither falling)
/// and the level random's `nextInt(4)` is not 0 (`LavaFluid.getSpreadDelay`).
fn spread_delay(w: &mut World, p: BlockPos, old: FluidState, new: FluidState) -> i32 {
    let mut d = tick_delay(w, old.fluid);
    if old.fluid.is_lava() && !old.is_empty() && !new.is_empty() && !old.falling && !new.falling && new.height(w, p) > old.height(w, p) && crate::weather::fire::rng(w).next_int(4) != 0 {
        d *= 4;
    }
    d
}

/// A fluid tick (`ServerLevel.tickFluid` -> `FlowingFluid.tick`), run when the cell's fluid is
/// still the scheduled type.
pub fn tick(w: &mut World, p: BlockPos, kind: Fluid) {
    let mut s = w.get(p);
    let mut f = fluid_state_of(&s);
    if f.fluid != kind {
        return;
    }
    if !f.is_source() {
        let nf = get_new_liquid_of(w, kind, p, &w.get(p));
        let delay = spread_delay(w, p, f, nf);
        if nf.is_empty() {
            f = nf;
            s = Block::Air;
            w.set_block_and_update(p, s);
        } else if nf != f {
            f = nf;
            s = f.create_legacy_block();
            w.set_block_and_update(p, s);
            w.schedule_fluid_tick(p, f.fluid, delay);
        }
    }
    spread(w, p, s, f);
}

/// `FlowingFluid.getFlow(level, pos, fluidState)`: the normalized current (floats as the game).
pub fn flow(w: &World, p: BlockPos, f: FluidState) -> Vec3 {
    let mut fx = 0.0f64;
    let mut fz = 0.0f64;
    let own = f.own_height();
    for d in Dir::HORIZONTAL {
        let np = p.rel(d);
        let nf = fluid_state(w, np);
        // affectsFlow: no fluid, or the same fluid
        if nf.is_empty() || nf.fluid.same(f.fluid) {
            let mut nh = nf.own_height();
            let mut dist = 0.0f32;
            if nh == 0.0 {
                if !blocks_fluid_flow(&w.get(np)) {
                    let bf = fluid_state(w, np.below());
                    if bf.is_empty() || bf.fluid.same(f.fluid) {
                        nh = bf.own_height();
                        if nh > 0.0 {
                            dist = own - (nh - 0.8888889);
                        }
                    }
                }
            } else if nh > 0.0 {
                dist = own - nh;
            }
            if dist != 0.0 {
                let st = d.step();
                fx += st[0] as f64 * dist as f64;
                fz += st[2] as f64 * dist as f64;
            }
        }
    }
    let mut v = Vec3::new(fx, 0.0, fz);
    if f.falling {
        for d in Dir::HORIZONTAL {
            let np = p.rel(d);
            if is_solid_face(w, f.fluid, np, d) || is_solid_face(w, f.fluid, np.above(), d) {
                v = v.normalize().add(0.0, -6.0, 0.0);
                break;
            }
        }
    }
    v.normalize()
}

/// `FlowingFluid.isSolidFace` (for the fluid `kind` whose flow it is).
fn is_solid_face(w: &World, kind: Fluid, p: BlockPos, d: Dir) -> bool {
    let s = w.get(p);
    if fluid_state_of(&s).fluid.same(kind) {
        false
    } else if d == Dir::Up {
        true
    } else {
        bh::sturdy(w, &s, p, d, bh::Support::Full)
    }
}

/// `state.is(BlockTags.BLOCKS_FLUID_FLOW)` (`#blocks_motion` + `#all_signs`): the solid blocks.
fn blocks_fluid_flow(b: &Block) -> bool {
    !matches!(b, Block::Water { .. } | Block::BubbleColumn { .. }) && bh::is_solid(b)
}

// ---------------------------------------------------------------------- LiquidBlock

/// `LiquidBlock.shouldBubbleColumnOccupy`: a full water source.
fn should_bubble_column_occupy(s: &Block) -> bool {
    let f = fluid_state_of(s);
    f.fluid == Fluid::Water && f.is_source() && f.is_full()
}

fn enables_bubble_column(b: &Block) -> bool {
    matches!(b, Block::Simple(Simple::SoulSand) | Block::Simple(Simple::MagmaBlock))
}

/// `LiquidBlock.tryScheduleBubbleBlockColumn`.
fn try_schedule_bubble_block_column(w: &mut World, p: BlockPos, below: &Block) {
    if enables_bubble_column(below) {
        w.schedule_tick(p, Kind::Water, 20, Priority::Normal);
    }
}

/// `LiquidBlock.onPlace` and `neighborChanged` (the same for water).
pub fn liquid_on_place(w: &mut World, s: Block, p: BlockPos) {
    if should_spread_liquid(w, p, &s) {
        let f = fluid_state_of(&s).fluid;
        w.schedule_fluid_tick(p, f, tick_delay(w, f));
    }
    if should_bubble_column_occupy(&s) {
        let below = w.get(p.below());
        try_schedule_bubble_block_column(w, p, &below);
    }
}

/// `LiquidBlock.shouldSpreadLiquid`: lava beside water (above it or on a side:
/// `POSSIBLE_FLOW_DIRECTIONS` down, south, north, east, west, each one's opposite neighbour)
/// turns into obsidian (a source) or cobblestone and spreads no more (lava over soul soil beside
/// blue ice makes basalt: neither is modeled, so it cannot happen here).
fn should_spread_liquid(w: &mut World, p: BlockPos, s: &Block) -> bool {
    if !matches!(s, Block::Lava { .. }) {
        return true;
    }
    for d in [Dir::Down, Dir::South, Dir::North, Dir::East, Dir::West] {
        if fluid_state(w, p.rel(d.opposite())).fluid.is_water() {
            let to = if fluid_state(w, p).is_source() { Simple::Obsidian } else { Simple::Cobblestone };
            w.set_block_and_update(p, Block::Simple(to));
            return false;
        }
    }
    true
}

/// `LiquidBlock.updateShape`.
pub fn liquid_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if fluid_state_of(&s).is_source() || fluid_state_of(&ns).is_source() {
        let f = fluid_state_of(&s).fluid;
        w.schedule_fluid_tick(p, f, tick_delay(w, f));
    }
    if d == Dir::Down && should_bubble_column_occupy(&s) {
        try_schedule_bubble_block_column(w, p, &ns);
    }
    s
}

/// `LiquidBlock.tick` (the 20 gt block tick): a bubble column forms.
pub fn liquid_tick(w: &mut World, s: Block, p: BlockPos) {
    if should_bubble_column_occupy(&s) {
        let below = w.get(p.below());
        update_column(w, p, s, below);
    }
}

// ---------------------------------------------------------------------- BubbleColumnBlock

/// `BubbleColumnBlock.canOccupy`: a bubble column or a full water source block.
fn can_occupy(s: &Block) -> bool {
    matches!(s, Block::BubbleColumn { .. } | Block::Water { level: 0 })
}

/// `BubbleColumnBlock.getColumnState`.
fn get_column_state(below: &Block, occupy: &Block) -> Block {
    match below {
        Block::BubbleColumn { .. } => *below,
        Block::Simple(Simple::SoulSand) => Block::BubbleColumn { drag: false },
        Block::Simple(Simple::MagmaBlock) => Block::BubbleColumn { drag: true },
        _ => {
            if matches!(occupy, Block::BubbleColumn { .. }) {
                Block::Water { level: 0 }
            } else {
                *occupy
            }
        }
    }
}

/// `BubbleColumnBlock.updateColumn(bubbleColumn, level, occupyAt, occupyState, belowState)`.
pub fn update_column(w: &mut World, at: BlockPos, occupy: Block, below: Block) {
    if !can_occupy(&occupy) {
        return;
    }
    let col = get_column_state(&below, &occupy);
    w.set_block(at, col, flags::CLIENTS);
    let mut p = at.above();
    while can_occupy(&w.get(p)) {
        if !w.set_block(p, col, flags::CLIENTS) {
            return;
        }
        p = p.above();
    }
}

/// `BubbleColumnBlock.canSurvive`.
pub fn bubble_can_survive(w: &World, p: BlockPos) -> bool {
    let b = w.get(p.below());
    matches!(b, Block::BubbleColumn { .. }) || enables_bubble_column(&b)
}

/// `BubbleColumnBlock.tick`.
pub fn bubble_tick(w: &mut World, s: Block, p: BlockPos) {
    let below = w.get(p.below());
    update_column(w, p, s, below);
}

/// `BubbleColumnBlock.updateShape`.
pub fn bubble_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    w.schedule_fluid_tick(p, Fluid::Water, TICK_DELAY);
    if !bubble_can_survive(w, p) || d == Dir::Down || (d == Dir::Up && !matches!(ns, Block::BubbleColumn { .. }) && can_occupy(&ns)) {
        w.schedule_tick(p, Kind::BubbleColumn, 5, Priority::Normal);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces() {
        let full = vec![[0, 16, 0, 16, 0, 16]];
        let bottom = vec![[0, 16, 0, 8, 0, 16]];
        // a bottom slab next to air: its side face is half covered
        assert!(!merged_face_occludes(&bottom, &[], Dir::East));
        // a bottom slab above air: its bottom face is fully covered
        assert!(merged_face_occludes(&[], &bottom, Dir::Up));
        assert!(merged_face_occludes(&full, &full, Dir::North));
        assert_eq!(FluidState::flowing(7, false).legacy_level(), 1);
        assert_eq!(FluidState::flowing(8, true).legacy_level(), 8);
        assert_eq!(fluid_state_of(&Block::Water { level: 9 }), FluidState::flowing(8, true));
    }
}
