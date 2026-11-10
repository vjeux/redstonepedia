//! Natural plants on their supports (tickexact 2026-11-10; mob-AI's d1-bed-plate forest t2504: an
//! enderman took the block under a leaf litter, which stayed floating in redsim and dropped in the
//! game). The natural states redsim keeps as [`Block::Terrain`] rows whose class is a plant: each
//! one's `canSurvive`, and its `updateShape` (air when it can no longer stand: `Block.
//! updateOrDestroy` then destroys it with its drops, [`crate::terrain::loot`]). From the 26.3
//! sources, class by class:
//! - `VegetationBlock` (`BushBlock`, `FlowerBlock`, `TallGrassBlock`, `FireflyBushBlock`,
//!   `FlowerBedBlock`, `DoublePlantBlock`, `TallFlowerBlock`): the block below is
//!   `#supports_vegetation` (`mayPlaceOn`); `updateShape` from any side: air when it cannot
//!   survive;
//! - `DryVegetationBlock` (dead bush, short and tall dry grass): `#supports_dry_vegetation`;
//! - `LeafLitterBlock`: the block below is sturdy on its top face;
//! - `MushroomBlock`: `#overrides_mushroom_light_requirement` below, or raw brightness under 13
//!   over a solid-render block;
//! - `SeagrassBlock` / `TallSeagrassBlock`: sturdy on top and not `#cannot_support_seagrass`
//!   (a tall seagrass's own fluid is always a full water source);
//! - `CropBlock` (wheat, carrots, potatoes, beetroots): raw brightness 8 or more, and
//!   `#supports_crops` below;
//! - `CarpetBlock`: anything but air below;
//! - `DoublePlantBlock`: the upper half stands on its lower half; a half whose other half is gone
//!   along the vertical turns to air (`updateShape`'s first test), the lower half checks its
//!   support when the block below changes.
//!
//! A natural state with no model here whose neighbour turns to air on a side the game's table says
//! it reacts to (`support`: a torch, a vine, a bed...) is a named model error.

use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::terrain::blocks::{self, Props};
use crate::world::World;

/// What a plant needs under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Vegetation,
    DryVegetation,
    /// `LeafLitterBlock`: a sturdy top face below.
    SturdyUp,
    Mushroom,
    Seagrass,
    Crop,
    /// `CarpetBlock`: not air below.
    Carpet,
}

/// A natural plant's survival rule, and whether it is a two-block plant (`DoublePlantBlock`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plant {
    pub rule: Rule,
    pub double: bool,
}

/// The plant of a state's class chain (its own class first), for the classes checked in the 26.3
/// sources; `None` for any other.
fn plant_of(p: &Props) -> Option<Plant> {
    let own = p.classes.first().map(String::as_str).unwrap_or("");
    let rule = match own {
        "BushBlock" | "FlowerBlock" | "TallGrassBlock" | "FireflyBushBlock" | "FlowerBedBlock" | "DoublePlantBlock" | "TallFlowerBlock" => Rule::Vegetation,
        "DryVegetationBlock" | "ShortDryGrassBlock" | "TallDryGrassBlock" => Rule::DryVegetation,
        "LeafLitterBlock" => Rule::SturdyUp,
        "MushroomBlock" => Rule::Mushroom,
        "SeagrassBlock" | "TallSeagrassBlock" => Rule::Seagrass,
        "CropBlock" | "CarrotBlock" | "PotatoBlock" | "BeetrootBlock" => Rule::Crop,
        "CarpetBlock" => Rule::Carpet,
        _ => return None,
    };
    Some(Plant { rule, double: p.classes.iter().any(|c| c == "DoublePlantBlock") })
}

/// The plant of natural-state row `id` (one table lookup per row, computed once).
pub fn plant(id: u16) -> Option<Plant> {
    static ROWS: std::sync::OnceLock<Vec<Option<Plant>>> = std::sync::OnceLock::new();
    let rows = ROWS.get_or_init(|| blocks::table().props.iter().map(plant_of).collect());
    rows.get(id as usize).copied().flatten()
}

/// The natural-state row of any block (a modeled block by its game string), if the table has it:
/// its tags and render flags.
fn row(b: &Block) -> Option<&'static Props> {
    match *b {
        Block::Terrain(id) => Some(blocks::props(id)),
        Block::Air => None,
        _ => blocks::id_of(&b.to_string()).map(blocks::props),
    }
}

fn has_tag(b: &Block, tag: &str) -> bool {
    row(b).is_some_and(|p| p.has_tag(tag))
}

/// `isFaceSturdy(level, pos, UP)` of the block below.
fn sturdy_up(w: &World, below: &Block, bp: BlockPos) -> bool {
    crate::behavior::sturdy(w, below, bp, Dir::Up, crate::behavior::Support::Full)
}

/// Whether the half property of a two-block plant's state is `lower`.
fn is_lower(p: &Props) -> bool {
    p.name.contains("half=lower")
}

/// `mayPlaceOn` / the single-block `canSurvive` of a plant at `p` (the lower half of a two-block
/// plant).
fn base_can_survive(w: &mut World, rule: Rule, p: BlockPos) -> bool {
    let bp = p.below();
    let below = w.get(bp);
    match rule {
        Rule::Vegetation => has_tag(&below, "minecraft:supports_vegetation"),
        Rule::DryVegetation => has_tag(&below, "minecraft:supports_dry_vegetation"),
        Rule::SturdyUp => sturdy_up(w, &below, bp),
        Rule::Mushroom => {
            has_tag(&below, "minecraft:overrides_mushroom_light_requirement") || (crate::entity::mob::natural::brightness(w, p, 0) < 13 && row(&below).is_some_and(|r| r.solid_render))
        }
        Rule::Seagrass => sturdy_up(w, &below, bp) && !has_tag(&below, "minecraft:cannot_support_seagrass"),
        Rule::Crop => crate::entity::mob::natural::brightness(w, p, 0) >= 8 && has_tag(&below, "minecraft:supports_crops"),
        Rule::Carpet => !below.is_air(),
    }
}

/// `canSurvive` of the plant state `id` at `p`.
pub fn can_survive(w: &mut World, id: u16, p: BlockPos) -> bool {
    let Some(pl) = plant(id) else { return true };
    let props = blocks::props(id);
    if pl.double && !is_lower(props) {
        // DoublePlantBlock.canSurvive: the upper half stands on its own lower half
        return match w.get(p.below()) {
            Block::Terrain(b) => blocks::props(b).block_id() == props.block_id() && is_lower(blocks::props(b)),
            _ => false,
        };
    }
    base_can_survive(w, pl.rule, p)
}

/// `updateShape` of the natural plant `s` (row `id`) at `p`, its neighbour `d` now `ns`.
pub fn update_shape(w: &mut World, s: Block, id: u16, p: BlockPos, d: Dir, ns: Block) -> Block {
    let Some(pl) = plant(id) else { return s };
    if pl.double {
        // DoublePlantBlock.updateShape: along the vertical, toward the other half, that half gone
        let props = blocks::props(id);
        let lower = is_lower(props);
        let other_half = matches!(ns, Block::Terrain(n) if blocks::props(n).block_id() == props.block_id() && is_lower(blocks::props(n)) != lower);
        let vertical = matches!(d, Dir::Up | Dir::Down);
        if vertical && lower == (d == Dir::Up) && !other_half {
            return Block::Air;
        }
    }
    // VegetationBlock / CarpetBlock: air when it can no longer stand
    if can_survive(w, id, p) { s } else { Block::Air }
}

/// A natural state redsim does not model reacting to a neighbour that turned to air on a side its
/// row says it reacts to (`support`): a named model error (the game changes or breaks it).
pub fn unmodeled_update(w: &mut World, id: u16, p: BlockPos, d: Dir, ns: &Block) {
    if !ns.is_air() {
        return;
    }
    let props = blocks::props(id);
    // (only a state whose class declares updateShape reacts: an AirBlock (cave_air, void_air)
    // never does, whatever its support column says; mobviewer 2026-11-10: cross-4-underground
    // stopped at tick 440 on a cave_air)
    if props.air || !props.reactive.iter().any(|r| r == "updateShape") || !props.support[blocks::dir_index(d)] {
        return;
    }
    let e = format!("natural {} at {p}: its {d:?} neighbour turned to air (its updateShape reacts there), not modeled", props.name);
    if !w.errors.contains(&e) {
        w.errors.push(e);
    }
}

// ------------------------------------------------------------------ natural connections

/// Whether a natural row's class chain holds `class`.
fn is_class(p: &Props, class: &str) -> bool {
    p.classes.iter().any(|c| c == class)
}

/// The natural states whose connections follow their horizontal neighbours: `FenceBlock` and
/// `IronBarsBlock` (glass panes, iron bars).
pub fn connects(id: u16) -> bool {
    let p = blocks::props(id);
    is_class(p, "FenceBlock") || is_class(p, "IronBarsBlock")
}

/// The block id of any block (`minecraft:...`, no properties).
fn block_id(b: &Block) -> String {
    let s = b.to_string();
    s.split('[').next().unwrap_or(&s).to_string()
}

/// `Block.isExceptionForConnection`: leaves, barrier, pumpkins (carved, jack o'lantern), melon,
/// shulker boxes.
fn connection_exception(b: &Block) -> bool {
    if matches!(b, Block::Leaves { .. }) || has_tag(b, "minecraft:leaves") {
        return true;
    }
    let id = block_id(b);
    matches!(id.as_str(), "minecraft:barrier" | "minecraft:carved_pumpkin" | "minecraft:jack_o_lantern" | "minecraft:melon" | "minecraft:pumpkin") || id.ends_with("shulker_box")
}

/// A fence's family: wooden (`#wooden_fences`), or the nether brick fence's own.
fn fence_family(b: &Block) -> Option<bool> {
    match *b {
        Block::Fence { kind, .. } => Some(crate::fence::wooden(kind)),
        Block::Terrain(id) if is_class(blocks::props(id), "FenceBlock") => Some(blocks::props(id).block_id() != "minecraft:nether_brick_fence"),
        _ => None,
    }
}

/// A fence gate's facing axis is the direction's clockwise axis (`FenceGateBlock.connectsToDirection`).
fn gate_along(b: &Block, d: Dir) -> bool {
    match *b {
        Block::FenceGate { facing, .. } => facing.axis() == d.clockwise().axis(),
        Block::Terrain(id) if is_class(blocks::props(id), "FenceGateBlock") => {
            let name = &blocks::props(id).name;
            let ns = name.contains("facing=north") || name.contains("facing=south");
            let axis_x = d.clockwise().axis() == Dir::East.axis();
            // facing north/south: its axis is z
            if ns { !axis_x } else { axis_x }
        }
        _ => false,
    }
}

/// `FenceBlock.updateShape` / `IronBarsBlock.updateShape` of the natural state `s` (row `id`) at
/// `p`, its neighbour `d` now `ns` at `np`: a waterlogged one schedules its water tick; from a
/// horizontal neighbour, that side's connection (`connectsTo` / `attachsTo`) with the neighbour's
/// face toward it sturdy (FULL). The new state's row: a state the table lacks is a named model
/// error (the old one stays).
pub fn connection_update(w: &mut World, s: Block, id: u16, p: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    let props = blocks::props(id);
    if props.name.contains("waterlogged=true") {
        w.schedule_fluid_tick(p, crate::fluid::Fluid::Water, crate::fluid::TICK_DELAY);
    }
    if !d.is_horizontal() {
        return s;
    }
    let sturdy = crate::behavior::sturdy(w, &ns, np, d.opposite(), crate::behavior::Support::Full);
    let from = d.opposite();
    let c = if is_class(props, "FenceBlock") {
        // FenceBlock.connectsTo(neighbour, faceSolid, direction from the neighbour)
        let mine = props.block_id() != "minecraft:nether_brick_fence";
        (!connection_exception(&ns) && sturdy) || fence_family(&ns) == Some(mine) || gate_along(&ns, from)
    } else {
        // IronBarsBlock.attachsTo(neighbour, faceSolid)
        (!connection_exception(&ns) && sturdy) || row(&ns).is_some_and(|r| is_class(r, "IronBarsBlock")) || has_tag(&ns, "minecraft:walls")
    };
    let key = match d {
        Dir::North => "north",
        Dir::South => "south",
        Dir::West => "west",
        _ => "east",
    };
    let (old_v, new_v) = (format!("{key}={}", !c), format!("{key}={c}"));
    if !props.name.contains(&old_v) {
        return s;
    }
    let name = props.name.replacen(&old_v, &new_v, 1);
    match blocks::id_of(&name) {
        Some(n) => Block::Terrain(n),
        None => {
            let e = format!("natural {} at {p}: its connection turns to {name}, a state the natural table lacks (mctas terrain blockinfo --block)", props.name);
            if !w.errors.contains(&e) {
                w.errors.push(e);
            }
            s
        }
    }
}
