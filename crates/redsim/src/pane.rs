//! Panes and bars (`IronBarsBlock`, a `CrossCollisionBlock`; tickexact 2026-11-12, for the lift's
//! pane walls and sight lines), read from the 26.3 source.
//!
//! * The glass pane, the 16 stained glass panes (`StainedGlassPaneBlock extends IronBarsBlock`) and
//!   iron bars. `IronBarsBlock` is `CrossCollisionBlock(2, 16, 2, 16, 16)`: the collision shape and
//!   the outline are both a 2 px post (7..9) 16 px tall plus a 2 px arm 16 px tall toward each
//!   attached side (`Block.boxZ(2, 0, 16, 0, 8)` rotated). Every pane state's shapes, solidity
//!   (solid: the post is 1 block tall), conduction (none), support (CENTER on the top and bottom
//!   faces: the post's 2 x 2 px covers `CENTER_SUPPORT_SHAPE`, so torches stand on panes and lanterns
//!   hang under them; no FULL or RIGID face) and light (dry: skylight passes, dampening 0;
//!   waterlogged: 1) are the natural-state table's rows (`mctas terrain blockinfo`); a stained pane
//!   reads the glass pane's row with the same arms (the same class, properties and shapes). The
//!   visual shape is empty (`getVisualShape`), so a `VISUAL` clip passes, while the sight test
//!   (`ClipContext.Block.COLLIDER`) sees the post and arms.
//! * `getStateForPlacement` and `updateShape` (a waterlogged pane first schedules its water tick):
//!   from a horizontal neighbour, that side's arm = `attachsTo(neighbour, neighbour.isFaceSturdy(
//!   face toward the pane, FULL))`: a sturdy face of a block that is not an exception for connection
//!   (`behavior::exception_for_connection`: leaves, barrier, carved pumpkin, jack o'lantern, melon,
//!   pumpkin, shulker boxes), another `IronBarsBlock` (glass, stained glass and bars all attach to
//!   each other) or a wall (`#walls`, not modeled). Vertical neighbours change nothing. redsim, like
//!   the judge, places a design's blocks raw: a pane keeps its written arms until a neighbour
//!   updates it, so `arm_mismatches` lists every pane and fence whose written arms its neighbours
//!   contradict (the farm judge and the scene lint refuse them).
//! * A `SimpleWaterloggedBlock`: its water is a source; `canPlaceLiquid` takes only a water source,
//!   so a dry pane refuses flowing water (`fluid::can_hold_specific_fluid`).
//! * `isPathfindable` false (`CrossCollisionBlock`): a mob's path sees `BLOCKED`. Instrument: the
//!   glass panes HAT (`Blocks.GLASS_PANE`, `STAINED_GLASS_PANE`), iron bars HARP. Explosion
//!   resistance 0.3 (glass), 6.0 (bars).

use crate::block::{Block, DYES};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `Block::Pane::kind` of iron bars (0 is the glass pane, 1..=16 the stained panes).
pub const IRON_BARS: u8 = 17;

/// The block id without `minecraft:` of a pane kind.
pub fn name(kind: u8) -> String {
    match kind {
        0 => "glass_pane".to_string(),
        IRON_BARS => "iron_bars".to_string(),
        k => format!("{}_stained_glass_pane", DYES[(k as usize - 1) % 16]),
    }
}

/// The pane kind of a block id without `minecraft:`.
pub fn kind_of(id: &str) -> Option<u8> {
    match id {
        "glass_pane" => Some(0),
        "iron_bars" => Some(IRON_BARS),
        n => {
            let dye = n.strip_suffix("_stained_glass_pane")?;
            DYES.iter().position(|d| *d == dye).map(|i| i as u8 + 1)
        }
    }
}

/// Whether the kind is a glass pane (stained or not), not bars.
pub fn glass(kind: u8) -> bool {
    kind != IRON_BARS
}

/// The pane's row of the natural-state table: the glass pane's row for a stained pane.
pub fn row(s: &Block) -> Option<&'static crate::terrain::blocks::Props> {
    let Block::Pane { kind, north, east, south, west, waterlogged } = *s else { return None };
    let table = if glass(kind) { 0 } else { IRON_BARS };
    let state = Block::Pane { kind: table, north, east, south, west, waterlogged };
    crate::terrain::blocks::id_of(&state.to_string()).map(crate::terrain::blocks::props)
}

/// `IronBarsBlock.attachsTo(state, faceSolid)`.
pub fn attaches_to(s: &Block, face_solid: bool) -> bool {
    (!crate::behavior::exception_for_connection(s) && face_solid) || matches!(s, Block::Pane { .. })
}

/// The panes and fences among `blocks` whose written arms their horizontal neighbours (in the
/// same list; a cell not in it is air) contradict: `getStateForPlacement` would give other arms,
/// and nothing updates a raw-placed block until a neighbour changes. Each: the cell, the written
/// state, the state with the arms its neighbours give.
pub fn arm_mismatches(blocks: &[(BlockPos, Block)]) -> Vec<(BlockPos, Block, Block)> {
    use crate::behavior::{Support, sturdy_of};
    let map: std::collections::HashMap<BlockPos, Block> = blocks.iter().map(|(p, s)| (*p, *s)).collect();
    let at = |p: BlockPos| map.get(&p).copied().unwrap_or(Block::Air);
    let mut out = Vec::new();
    for (p, s) in blocks {
        let want = |d: Dir| -> bool {
            let ns = at(p.rel(d));
            let solid = sturdy_of(&ns, d.opposite(), Support::Full);
            match *s {
                Block::Pane { .. } => attaches_to(&ns, solid),
                Block::Fence { kind, .. } => crate::fence::connects_to(kind, &ns, solid, d.opposite()),
                _ => false,
            }
        };
        let (north, east, south, west) = (want(Dir::North), want(Dir::East), want(Dir::South), want(Dir::West));
        let expected = match *s {
            Block::Pane { kind, waterlogged, .. } => Block::Pane { kind, north, east, south, west, waterlogged },
            Block::Fence { kind, waterlogged, .. } => Block::Fence { kind, north, east, south, west, waterlogged },
            _ => continue,
        };
        if expected != *s {
            out.push((*p, *s, expected));
        }
    }
    out
}

/// `IronBarsBlock.updateShape`.
pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    crate::fluid::waterlogged_update_shape(w, &s, p);
    let Block::Pane { kind, north, east, south, west, waterlogged } = s else { return s };
    if !d.is_horizontal() {
        return s;
    }
    let a = attaches_to(&ns, crate::behavior::sturdy(w, &ns, np, d.opposite(), crate::behavior::Support::Full));
    match d {
        Dir::North => Block::Pane { kind, north: a, east, south, west, waterlogged },
        Dir::East => Block::Pane { kind, north, east: a, south, west, waterlogged },
        Dir::South => Block::Pane { kind, north, east, south: a, west, waterlogged },
        _ => Block::Pane { kind, north, east, south, west: a, waterlogged },
    }
}
