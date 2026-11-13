//! Block properties the mob rules read (ironfarm), from the 26.3 source: a bed's shape
//! (`BedBlock.SHAPES`), `BlockStateBase.entityCanStandOn` (the collision face UP is full),
//! `SpawnUtil.Strategy.LEGACY_IRON_GOLEM`'s excluded blocks, `NaturalSpawner.isValidEmptySpawnBlock`,
//! `EntityType.isBlockDangerous` (`NodeEvaluator.isBurningBlock`), `isSuffocating`, and where
//! lava could flow (`lava_can_flow`; lava flows since mobinfra's model, no refusal).

use crate::behavior::{self, Box16};
use crate::block::{Block, Simple};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `BedBlock.SHAPES.get(d)`: `rotateHorizontal(or(column(16, 3, 9), box(0, 0, 0, 3, 3, 3), its 90
/// degree turn))` with the legs on side `d` (north: z 0..3).
fn bed_shape(d: Dir) -> Vec<Box16> {
    let base: Box16 = [0, 16, 3, 9, 0, 16];
    // legs at the north edge (z 0..3): west (x 0..3) and east (x 13..16), 3 px tall
    let legs: [Box16; 2] = [[0, 3, 0, 3, 0, 3], [13, 16, 0, 3, 0, 3]];
    let turn = |b: Box16| -> Box16 {
        // [x0, x1, y0, y1, z0, z1] turned so that north goes to `d`
        match d {
            Dir::North => b,
            Dir::South => [16 - b[1], 16 - b[0], b[2], b[3], 16 - b[5], 16 - b[4]],
            Dir::East => [16 - b[5], 16 - b[4], b[2], b[3], b[0], b[1]],
            Dir::West => [b[4], b[5], b[2], b[3], 16 - b[1], 16 - b[0]],
            _ => b,
        }
    };
    let mut out = vec![base];
    out.extend(legs.iter().map(|b| turn(*b)));
    out
}

/// A bed half's collision boxes: `SHAPES.get(getConnectedDirection(state).getOpposite())` (the
/// head connects to the foot behind it: its legs face `facing`; the foot's face away).
pub fn bed_boxes(facing: Dir, head: bool) -> Vec<Box16> {
    bed_shape(if head { facing } else { facing.opposite() })
}

/// `Block.isFaceFull(shape, UP)` of a block's collision boxes: the boxes reaching the top of the
/// cell cover its whole 16 x 16 face.
pub fn face_up_full(boxes: &[Box16]) -> bool {
    let mut rows = [0u32; 16];
    for b in boxes.iter().filter(|b| b[3] >= 16 && b[1] > b[0] && b[5] > b[4]) {
        for row in rows.iter_mut().take(b[5].min(16) as usize).skip(b[4].max(0) as usize) {
            *row |= ((1u64 << b[1].min(16)) - 1) as u32 & !(((1u64 << b[0].max(0)) - 1) as u32);
        }
    }
    rows.iter().all(|r| *r == 0xffff)
}

/// `BlockStateBase.entityCanStandOn(level, pos, entity)` = `isFaceFull(collision shape, UP)`.
pub fn entity_can_stand_on(w: &World, p: BlockPos) -> bool {
    let s = w.get(p);
    if s.is_air() {
        return false;
    }
    let boxes: Vec<Box16> = crate::entity::collide::boxes_at(w, &s, p)
        .iter()
        .map(|b| {
            let f = |v: f64, o: i32| ((v - o as f64) * 16.0).round() as i32;
            [f(b.min_x, p.x), f(b.max_x, p.x), f(b.min_y, p.y), f(b.max_y, p.y), f(b.min_z, p.z), f(b.max_z, p.z)]
        })
        .collect();
    face_up_full(&boxes)
}

/// The blocks `SpawnUtil.Strategy.LEGACY_IRON_GOLEM` never spawns on: cobweb, cactus, glass panes,
/// stained glass (panes), leaves, conduit, ice, TNT, glowstone, beacon, sea lantern, frosted ice,
/// tinted glass, glass. (Packed and blue ice are not `Blocks.ICE`.)
pub fn golem_excluded(s: &Block) -> bool {
    if let Block::Terrain(id) = s {
        // terrain: the same list on the natural state's block and class (the natural-state table)
        let p = crate::terrain::blocks::props(*id);
        return p.leaves_block
            || p.classes.iter().any(|c| c == "StainedGlassPaneBlock" || c == "StainedGlassBlock")
            || matches!(
                p.block_id(),
                "minecraft:cobweb" | "minecraft:cactus" | "minecraft:glass_pane" | "minecraft:conduit" | "minecraft:ice" | "minecraft:tnt" | "minecraft:glowstone" | "minecraft:beacon" | "minecraft:sea_lantern" | "minecraft:frosted_ice" | "minecraft:tinted_glass" | "minecraft:glass"
            );
    }
    matches!(
        s,
        Block::Simple(Simple::Glass | Simple::TintedGlass | Simple::WhiteStainedGlass | Simple::BlackStainedGlass | Simple::Glowstone | Simple::SeaLantern)
            | Block::Leaves { .. }
            | Block::Tnt { .. }
            | Block::Cactus { .. }
    )
}

/// `SpawnUtil.Strategy.LEGACY_IRON_GOLEM.canSpawnOn(level, pos, state, abovePos, aboveState)`: not
/// excluded, the block above is air or a liquid, and the block `isSolid()` (or powder snow).
pub fn legacy_golem_can_spawn_on(s: &Block, above: &Block) -> bool {
    // (the cheap test first: in a column of rock or of sky most cells fail on the cell above;
    // tickexact 2026-11-09, the spot search's column walks were 6% of the viewer's still)
    let terrain_air_or_liquid = matches!(above, Block::Terrain(id) if { let p = crate::terrain::blocks::props(*id); p.air || p.liquid });
    let above_ok = above.is_air() || terrain_air_or_liquid || matches!(above, Block::Water { .. } | Block::Lava { .. } | Block::BubbleColumn { .. });
    if !above_ok || golem_excluded(s) {
        return false;
    }
    behavior::is_solid(s) || matches!(s, Block::PowderSnow) || matches!(s, Block::Terrain(id) if crate::terrain::blocks::props(*id).block_id() == "minecraft:powder_snow")
}

/// `NodeEvaluator.isBurningBlock`: fire, lava, magma, a lit campfire, a lava cauldron (of the
/// modeled blocks: lava and magma; terrain states by their block and tags).
pub fn is_burning_block(s: &Block) -> bool {
    if let Block::Terrain(id) = s {
        let p = crate::terrain::blocks::props(*id);
        let lit_campfire = matches!(p.block_id(), "minecraft:campfire" | "minecraft:soul_campfire") && p.name.contains("lit=true");
        return p.has_tag("minecraft:fire") || lit_campfire || matches!(p.block_id(), "minecraft:lava" | "minecraft:magma_block" | "minecraft:lava_cauldron");
    }
    // a placed lit campfire (terrain lane), fire (mobinfra)
    matches!(s, Block::Lava { .. } | Block::Simple(Simple::MagmaBlock) | Block::Campfire { lit: true, .. } | Block::Fire { .. })
}

/// `EntityType.isBlockDangerous(state)` for a type that is not fire immune: burning blocks, wither
/// roses, sweet berry bushes, cactus, powder snow.
pub fn is_dangerous(s: &Block, fire_immune: bool) -> bool {
    let terrain = matches!(s, Block::Terrain(id) if matches!(crate::terrain::blocks::props(*id).block_id(), "minecraft:wither_rose" | "minecraft:sweet_berry_bush" | "minecraft:cactus" | "minecraft:powder_snow"));
    (!fire_immune && is_burning_block(s)) || terrain || matches!(s, Block::Cactus { .. } | Block::PowderSnow)
}

/// The rail blocks (`BlockTags.PREVENT_MOB_SPAWNING_INSIDE`; terrain states by the tag).
pub(crate) fn prevents_spawning_inside(s: &Block) -> bool {
    if let Block::Terrain(id) = s {
        return crate::terrain::blocks::props(*id).has_tag("minecraft:prevent_mob_spawning_inside");
    }
    matches!(s, Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. })
}

/// `EntityType.isBlockDangerous(state)` for entity type `type_id` (cc767b78, for the natural spawner's
/// states the block table has no row for; tick-exact's 26.3 reading, 2026-11-13: EntityType 338-346):
/// not when the type's `immuneTo` tag holds the block (empty by default, `#default_immune_to`; powder
/// snow for a stray, a snow golem and a polar bear, the wither rose for a wither skeleton and the
/// wither, the sweet berry bush for a fox: EntityTypes 491 / 825 / 947 / 999 / 1115 / 1125, the tags'
/// contents VanillaBlockTagsProvider 1233-1238), else [`is_dangerous`] with the type's fire immunity
/// (its row's `fire_immune`: `fireImmune()`).
pub fn dangerous_for(s: &Block, type_id: &str) -> bool {
    let name = s.to_string();
    let id = name.split('[').next().unwrap_or(&name);
    let immune: &[&str] = match type_id.trim_start_matches("minecraft:") {
        "stray" | "snow_golem" | "polar_bear" => &["minecraft:powder_snow"],
        "wither_skeleton" | "wither" => &["minecraft:wither_rose"],
        "fox" => &["minecraft:sweet_berry_bush"],
        _ => &[],
    };
    if immune.contains(&id) {
        return false;
    }
    let fire_immune = super::types::MobType::from_id(type_id).is_some_and(|t| t.def().fire_immune);
    is_dangerous(s, fire_immune)
}

/// `NaturalSpawner.isValidEmptySpawnBlock(level, pos, state, fluid, type)`: not a full collision
/// cube, not a signal source, no fluid (`check_fluid` false: the caller passed `Fluids.EMPTY`), not
/// a rail, not dangerous for the type.
pub fn is_valid_empty_spawn_block(w: &World, p: BlockPos, check_fluid: bool, fire_immune: bool) -> bool {
    let s = w.get(p);
    if behavior::full_cube(&s) || behavior::is_signal_source(w, &s) {
        return false;
    }
    if check_fluid && (!crate::fluid::fluid_state(w, p).is_empty() || matches!(s, Block::Lava { .. }) || matches!(s, Block::Terrain(id) if crate::terrain::blocks::props(id).fluid != crate::terrain::blocks::FluidKind::Empty)) {
        return false;
    }
    !prevents_spawning_inside(&s) && !is_dangerous(&s, fire_immune)
}

/// `BlockStateBase.isSuffocating`: the default (`blocksMotion() && isCollisionShapeFullBlock`) for
/// every modeled block but glass, tinted and stained glass, leaves, copper grates and repeaters
/// (`isSuffocating(Blocks::never)`); terrain states: the game's own answer.
pub fn is_suffocating(s: &Block) -> bool {
    if let Block::Terrain(id) = s {
        return crate::terrain::blocks::props(*id).suffocating;
    }
    if matches!(s, Block::Simple(Simple::Glass | Simple::TintedGlass | Simple::WhiteStainedGlass | Simple::BlackStainedGlass | Simple::WaxedCopperGrate) | Block::Leaves { .. } | Block::WaterloggedGrate | Block::Repeater { .. }) {
        return false;
    }
    behavior::full_cube(s)
}

/// Whether cell `p` surely cannot see the sky (`LevelReader.canSeeSky`: sky light 15, which only
/// comes straight down through blocks that let it pass undimmed): some block above it in its
/// column is leaves (they dim sky light by 1), a full cube other than glass (opaque), a slab
/// (`SlabBlock.useShapeForLightOcclusion`: a bottom slab's down face and a top slab's up face are
/// full squares, so the light engine passes no sky light straight down through either), or a
/// waterlogged rail (b8949c96, G1 on 4b6ee86e5: `propagatesSkylightDown` needs an empty fluid
/// state, so it dims by 1 as water does). Conservative: every other roof counts as letting the sky
/// through (water itself included).
pub fn sky_hidden(w: &World, p: BlockPos) -> bool {
    // a terrain world knows its sky light (`canSeeSky`: sky light 15)
    if w.land.is_some() {
        return w.sees_or(w.sky_light(p).map(|l| l < 15), false);
    }
    let top = w.min.y + w.size[1];
    (p.y + 1..top).any(|y| {
        let s = w.get(BlockPos::new(p.x, y, p.z));
        matches!(s, Block::Leaves { .. } | Block::Slab { .. } | Block::AnySlab { .. })
            || matches!(s, Block::Rail { waterlogged: true, .. } | Block::PlainRail { waterlogged: true, .. } | Block::DetectorRail { waterlogged: true, .. })
            || (behavior::full_cube(&s) && !matches!(s, Block::Simple(Simple::Glass | Simple::WhiteStainedGlass | Simple::BlackStainedGlass | Simple::WaxedCopperGrate) | Block::WaterloggedGrate))
    })
}

/// Could the lava at `p` flow (`FlowingFluid.spread`)? Down into a cell lava can hold, or sideways
/// into one: air, or any block whose collision shape is empty and is not a waterloggable or a
/// fluid-proof block (`FlowingFluid.canHoldFluid`: doors, signs, ladders, sugar cane, bubble
/// columns, trapdoors and every `LiquidBlockContainer` refuse lava). Conservative: any air or
/// non-colliding neighbor below or beside it counts, except trapdoors and the fluid-proof blocks.
pub fn lava_can_flow(w: &World, p: BlockPos) -> Option<BlockPos> {
    let holds = |q: BlockPos| {
        let s = w.get(q);
        match s {
            Block::Air => true,
            Block::Lava { .. } => false,
            // 26.3 `FlowingFluid.canHoldAnyFluid`: a `LiquidBlockContainer` (which takes only
            // water) or a block in WASHED_AWAY_BY_FLUIDS; a fence gate is neither
            Block::Trapdoor { .. } | Block::Door { .. } | Block::SugarCane { .. } | Block::BubbleColumn { .. } | Block::Water { .. } | Block::Sign { .. } | Block::FenceGate { .. } => false,
            _ => !behavior::collides(&s),
        }
    };
    [Dir::Down, Dir::North, Dir::South, Dir::West, Dir::East].into_iter().map(|d| p.rel(d)).find(|q| holds(*q))
}

/// The fluid height of the lava at `p` (`FluidState.getHeight`): 1 under more lava, else its
/// amount / 9 (8/9 for a source; flowing lava since mobinfra's model).
pub fn lava_height(w: &World, p: BlockPos) -> Option<f64> {
    let f = crate::fluid::fluid_state(w, p);
    if f.fluid.is_lava() { Some(f.height(w, p) as f64) } else { None }
}

/// `LevelReader.containsAnyLiquid(box)`: a cell the box meets holds a fluid (water, lava).
pub fn box_has_liquid(w: &World, bb: crate::entity::Aabb) -> bool {
    // LevelReader.containsAnyLiquid: findBlocksIn(BlockPos.containing(min), containing(max)), both
    // corners inclusive (a box ending on a cell's face reaches into that cell)
    let (x0, y0, z0) = (bb.min_x.floor() as i32, bb.min_y.floor() as i32, bb.min_z.floor() as i32);
    let (x1, y1, z1) = (bb.max_x.floor() as i32, bb.max_y.floor() as i32, bb.max_z.floor() as i32);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let p = BlockPos::new(x, y, z);
                if matches!(w.get(p), Block::Lava { .. }) || !crate::fluid::fluid_state(w, p).is_empty() || matches!(w.get(p), Block::Terrain(id) if crate::terrain::blocks::props(id).fluid != crate::terrain::blocks::FluidKind::Empty) {
                    return true;
                }
            }
        }
    }
    false
}

/// `Block.getFriction` (`boat::block_friction`: ice 0.98, slime 0.8, else 0.6).
pub fn friction(s: &Block) -> f32 {
    crate::entity::boat::block_friction(s)
}

/// Is `s` a bed whose head is at `p` (`PoiTypes.HOME`: a bed's `part=head` state)?
pub fn is_bed_head(s: &Block) -> bool {
    matches!(s, Block::Bed { head: true, .. })
}

/// The bed head cells in the box `[lo, hi]` (inclusive).
pub fn bed_heads(w: &World, lo: BlockPos, hi: BlockPos) -> Vec<BlockPos> {
    let mut out = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = BlockPos::new(x, y, z);
                if is_bed_head(&w.get(p)) {
                    out.push(p);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bed_shapes_turn_with_the_head() {
        // a head facing north has its legs on its north edge; its foot (south of it) on its south edge
        let h = bed_boxes(Dir::North, true);
        assert!(h.contains(&[0, 3, 0, 3, 0, 3]) && h.contains(&[13, 16, 0, 3, 0, 3]));
        let f = bed_boxes(Dir::North, false);
        assert!(f.contains(&[13, 16, 0, 3, 13, 16]) && f.contains(&[0, 3, 0, 3, 13, 16]));
        let e = bed_boxes(Dir::East, true);
        assert!(e.contains(&[13, 16, 0, 3, 0, 3]) && e.contains(&[13, 16, 0, 3, 13, 16]));
        // the top at 9 px: not a full top face, but solid (mean size 0.85)
        assert!(!face_up_full(&h));
        assert!(behavior::is_solid(&Block::Bed { colour: 14, facing: Dir::North, head: true, occupied: false }));
    }

    #[test]
    fn standing_faces() {
        assert!(face_up_full(&[[0, 16, 0, 16, 0, 16]]));
        assert!(face_up_full(&[[0, 16, 8, 16, 0, 16]]));
        assert!(!face_up_full(&[[0, 16, 0, 8, 0, 16]]));
        // two halves side by side make a full face; a ring (a hopper's rim) does not
        assert!(face_up_full(&[[0, 8, 0, 16, 0, 16], [8, 16, 0, 16, 0, 16]]));
        assert!(!face_up_full(&[[0, 16, 10, 16, 0, 2], [0, 16, 10, 16, 14, 16], [0, 2, 10, 16, 0, 16], [14, 16, 10, 16, 0, 16]]));
    }

    #[test]
    fn legacy_strategy() {
        let stone = Block::Simple(Simple::Stone);
        assert!(legacy_golem_can_spawn_on(&stone, &Block::Air));
        assert!(legacy_golem_can_spawn_on(&stone, &Block::Water { level: 0 }));
        assert!(!legacy_golem_can_spawn_on(&Block::Simple(Simple::Glass), &Block::Air));
        assert!(!legacy_golem_can_spawn_on(&stone, &stone));
        // both slabs are solid (mean size 0.83): the strategy stops there; only the obstruction
        // check (a full top face) tells them apart
        assert!(legacy_golem_can_spawn_on(&Block::Slab { kind: 0 }, &Block::Air));
        assert!(legacy_golem_can_spawn_on(&Block::Slab { kind: 1 }, &Block::Air));
    }
}
