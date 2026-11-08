//! The slab family (`SlabBlock`: every material's slab, terrain lane for the optimizer's day-1
//! materials): the shapes of smooth stone's [`Block::Slab`] (bottom, top, double), waterloggable
//! (`SimpleWaterloggedBlock`: a bottom or top slab takes a water source, flowing water too, a
//! double one none). Each slab's other properties are its own block's (`ofLegacyCopy(base)`):
//! blast resistance, flammability (the wood slabs burn: 5/20), lava ignition, push reaction and
//! light, read from the natural-state table row of its state (dumped from the game); the note
//! block instrument is its base's (`instrument`: the wood family's bass, every other one's base
//! drum, Blocks.java 26.3). Smooth stone's and stone's dry slabs keep their own variants
//! ([`Block::Slab`], [`Block::StoneSlab`]); their waterlogged states are this one.

use crate::block::Block;

/// The materials (the block id without `_slab`), Blocks.java 26.3's order. The weathering copper
/// slabs (random ticks) are not here.
pub const SLABS: [&str; 61] = [
    "resin_brick",
    "prismarine",
    "prismarine_brick",
    "dark_prismarine",
    "oak",
    "spruce",
    "birch",
    "jungle",
    "acacia",
    "cherry",
    "dark_oak",
    "pale_oak",
    "mangrove",
    "poplar",
    "bamboo",
    "bamboo_mosaic",
    "stone",
    "sandstone",
    "cut_sandstone",
    "petrified_oak",
    "cobblestone",
    "brick",
    "stone_brick",
    "mud_brick",
    "nether_brick",
    "quartz",
    "red_sandstone",
    "cut_red_sandstone",
    "smooth_stone",
    "purpur",
    "polished_granite",
    "smooth_red_sandstone",
    "mossy_stone_brick",
    "polished_diorite",
    "mossy_cobblestone",
    "end_stone_brick",
    "smooth_sandstone",
    "smooth_quartz",
    "granite",
    "andesite",
    "red_nether_brick",
    "polished_andesite",
    "diorite",
    "crimson",
    "warped",
    "blackstone",
    "polished_blackstone_brick",
    "polished_blackstone",
    "tuff",
    "polished_tuff",
    "tuff_brick",
    "sulfur",
    "polished_sulfur",
    "sulfur_brick",
    "cinnabar",
    "polished_cinnabar",
    "cinnabar_brick",
    "cobbled_deepslate",
    "polished_deepslate",
    "deepslate_tile",
    "deepslate_brick",
];

/// Every material.
pub fn all() -> impl Iterator<Item = &'static str> {
    SLABS.iter().copied()
}

/// The material index of a slab block id (`cobblestone_slab` -> its index).
pub fn index(id: &str) -> Option<u8> {
    let m = id.strip_suffix("_slab")?;
    all().position(|s| s == m).map(|i| i as u8)
}

/// The material of index `mat`.
pub fn material(mat: u8) -> &'static str {
    all().nth(mat as usize).unwrap_or("stone")
}

/// The wood family's slabs (bass, flammable).
pub fn wood(mat: u8) -> bool {
    matches!(
        material(mat),
        "oak" | "spruce" | "birch" | "jungle" | "acacia" | "cherry" | "dark_oak" | "pale_oak" | "mangrove" | "poplar" | "bamboo" | "bamboo_mosaic" | "crimson" | "warped"
    )
}

/// The game string of a slab.
pub fn state_name(mat: u8, kind: u8, waterlogged: bool) -> String {
    format!("minecraft:{}_slab[type={},waterlogged={}]", material(mat), ["bottom", "top", "double"][kind.min(2) as usize], waterlogged)
}

/// The natural-state table row of a slab's state (its resistance, odds, push reaction, light).
pub fn row(b: &Block) -> Option<&'static crate::terrain::blocks::Props> {
    match *b {
        Block::AnySlab { mat, kind, waterlogged } => crate::terrain::blocks::id_of(&state_name(mat, kind, waterlogged)).map(crate::terrain::blocks::props),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slab_state_parses_back_and_has_its_table_row() {
        for (i, m) in all().enumerate() {
            assert_eq!(index(&format!("{m}_slab")), Some(i as u8));
            for kind in 0..3u8 {
                for wet in [false, true] {
                    let s = state_name(i as u8, kind, wet);
                    let b = Block::parse(&s).unwrap_or_else(|e| panic!("{s}: {e}"));
                    assert_eq!(b.to_string(), s);
                    assert!(crate::terrain::blocks::id_of(&s).is_some(), "{s} is not in the natural-state table");
                }
            }
        }
    }

    /// A wood slab burns (5/20) unless waterlogged, a stone one does not; their resistances are
    /// their blocks' (planks 3, cobblestone 6); a double slab conducts and is solid, a half one not;
    /// a bottom slab is no spawn floor, a top one is; water fills a half slab, never a double one.
    #[test]
    fn slab_properties_are_their_blocks() {
        let b = |s: &str| Block::parse(s).unwrap();
        let oak = b("minecraft:oak_slab[type=bottom,waterlogged=false]");
        let wet_oak = b("minecraft:oak_slab[type=bottom,waterlogged=true]");
        let cobble = b("minecraft:cobblestone_slab[type=top,waterlogged=false]");
        let double = b("minecraft:cobblestone_slab[type=double,waterlogged=false]");
        assert_eq!(crate::weather::lightning::flammable(&oak), Some((5, 20)));
        assert_eq!(crate::weather::lightning::flammable(&wet_oak), None);
        assert_eq!(crate::weather::lightning::flammable(&cobble), None);
        assert!(crate::weather::fire::ignited_by_lava(&oak));
        assert_eq!(row(&oak).map(|p| p.resistance), Some(3.0));
        assert_eq!(row(&cobble).map(|p| p.resistance), Some(6.0));
        assert!(crate::behavior::conductor_of(&double) && !crate::behavior::conductor_of(&cobble));
        assert_eq!(crate::behavior::instrument(&oak), crate::block::Instrument::Bass);
        assert_eq!(crate::behavior::instrument(&cobble), crate::block::Instrument::Basedrum);
        assert_eq!(crate::fluid::waterlogged(&double), Some(false));
        assert_eq!(crate::fluid::set_waterlogged(b("minecraft:stone_slab[type=bottom,waterlogged=false]"), true).map(|x| x.to_string()), Some("minecraft:stone_slab[type=bottom,waterlogged=true]".to_string()));
        assert_eq!(crate::fluid::set_waterlogged(b("minecraft:stone_slab[type=bottom,waterlogged=true]"), false), Some(Block::StoneSlab { kind: 0 }));
        let zombie = crate::terrain::blocks::type_index("minecraft:zombie").unwrap();
        let floor = |x: &Block| crate::terrain::blocks::id_of(&x.to_string()).map(|id| crate::terrain::blocks::props(id).valid_spawn[zombie]);
        assert_eq!(floor(&oak), Some(false));
        assert_eq!(floor(&cobble), Some(true));
    }

    /// Water and slabs (`FlowingFluid`, `SimpleWaterloggedBlock`): spreading water never enters a
    /// half slab (`canPlaceLiquid` takes only a water source, spreading water is flowing water) nor
    /// a double one; a waterlogged slab's own water, once a neighbour's change ticks it
    /// (`SlabBlock.updateShape`), flows out through its open sides.
    #[test]
    fn slabs_hold_their_own_water_and_take_no_spreading_water() {
        use crate::pos::BlockPos;
        use crate::world::{World, flags};
        // the box reaches past where the water flows (8 out of each source over the floor)
        let mut w = World::new(BlockPos::new(-9, 0, -9), [25, 3, 21]);
        for x in -9..16 {
            for z in -9..12 {
                w.set_block(BlockPos::new(x, 0, z), Block::Simple(crate::block::Simple::Stone), flags::RAW);
            }
        }
        let b = |s: &str| Block::parse(s).unwrap();
        w.set_block(BlockPos::new(1, 1, 1), b("minecraft:oak_slab[type=bottom,waterlogged=false]"), flags::RAW);
        w.set_block(BlockPos::new(3, 1, 1), b("minecraft:cobblestone_slab[type=double,waterlogged=false]"), flags::RAW);
        w.set_block(BlockPos::new(2, 1, 1), Block::Water { level: 0 }, flags::ALL);
        // a waterlogged slab, then a block placed beside it (its updateShape ticks its water)
        w.set_block(BlockPos::new(5, 1, 1), b("minecraft:spruce_slab[type=top,waterlogged=true]"), flags::RAW);
        w.set_block(BlockPos::new(6, 1, 1), Block::Simple(crate::block::Simple::Stone), flags::ALL);
        for _ in 0..40 {
            w.tick();
        }
        assert_eq!(w.get(BlockPos::new(1, 1, 1)).to_string(), "minecraft:oak_slab[type=bottom,waterlogged=false]");
        assert_eq!(w.get(BlockPos::new(3, 1, 1)).to_string(), "minecraft:cobblestone_slab[type=double,waterlogged=false]");
        assert_eq!(w.get(BlockPos::new(5, 1, 1)).to_string(), "minecraft:spruce_slab[type=top,waterlogged=true]");
        assert!(crate::fluid::fluid_state(&w, BlockPos::new(5, 1, 0)).fluid.is_water(), "the waterlogged slab's water flowed out");
        assert!(w.errors.is_empty(), "{:?}", w.errors);
    }
}
