//! (G1 Nov 14 11:25, part 74: CHANGES 1 on part 64; vcompose / G1 Nov 14 14:58: the `needs_support` car) The SURVIVAL
//! CENSUS's lists: the 26.3 kinds with NO survival rule (their class overrides no `canSurvive`, cited) and the kinds
//! WITH one (their class overrides `canSurvive`, cited), and the predicate a stand-in reader asks: may a stand-in of
//! this block omit its `support=` (no survival rule), must it name it (a rule), or is the block in neither list (refused
//! by name, never defaulted). The census test over every farm design (which also reads redsim's modelled and NOT
//! MODELED arms) rides tools16b's part 74.

/// A block id's 26.3 no-survival-rule citation: the kinds whose class (and every superclass) overrides no
/// `canSurvive`, so the game keeps them whatever their neighbours (a full or partial block that stands on its own, a
/// container, a fluid, a falling block (it falls by its own tick, it never pops)). None: not listed.
pub fn no_survival_rule(id: &str) -> Option<&'static str> {
    let id = id.trim_start_matches("minecraft:");
    let ends = |s: &[&str]| s.iter().any(|x| id.ends_with(x));
    Some(if id == "air" || id == "cave_air" || id == "void_air" {
        "AirBlock: no canSurvive"
    } else if ends(&["_wool", "_concrete", "_terracotta", "_planks", "_log", "_wood", "_bricks", "_ore", "_block", "_glass", "_stone"]) && !ends(&["_carpet"]) {
        "a full cube (Block / RotatedPillarBlock / TransparentBlock / StainedGlassBlock family): no canSurvive"
    } else if ends(&["_slab", "_stairs", "_wall", "_fence", "_fence_gate", "_trapdoor", "_pane", "_leaves", "_shulker_box", "_bed"]) {
        "SlabBlock / StairBlock / WallBlock / FenceBlock / FenceGateBlock / TrapDoorBlock / IronBarsBlock / LeavesBlock / ShulkerBoxBlock / BedBlock: no canSurvive (a bed's halves update each other by updateShape, not canSurvive)"
    } else if matches!(
        id,
        "stone" | "cobblestone" | "smooth_stone" | "glass" | "tinted_glass" | "obsidian" | "crying_obsidian" | "bedrock" | "dirt" | "grass_block" | "coarse_dirt" | "podzol" | "mycelium" | "netherrack"
            | "end_stone" | "sandstone" | "deepslate" | "cobbled_deepslate" | "tuff" | "calcite" | "basalt" | "blackstone" | "andesite" | "diorite" | "granite" | "glowstone" | "sea_lantern"
            | "shroomlight" | "ice" | "packed_ice" | "blue_ice" | "snow_block" | "clay" | "mud" | "soul_sand" | "soul_soil" | "magma_block" | "sponge" | "wet_sponge" | "prismarine"
            | "dark_prismarine" | "purpur_pillar" | "quartz_pillar" | "hay_block" | "bone_block" | "dried_kelp_block" | "honey_block" | "slime_block" | "target" | "lodestone" | "beacon"
            | "jack_o_lantern" | "pumpkin" | "carved_pumpkin" | "melon" | "moss_block" | "barrier" | "reinforced_deepslate" | "lightning_rod" | "chain" | "iron_bars" | "end_rod"
    ) {
        "a block that stands on its own (Block, a full or partial shape, its class overriding no canSurvive)"
    } else if matches!(
        id,
        "chest" | "trapped_chest" | "barrel" | "hopper" | "dropper" | "dispenser" | "furnace" | "blast_furnace" | "smoker" | "crafter" | "crafting_table" | "composter" | "cauldron"
            | "water_cauldron" | "lava_cauldron" | "powder_snow_cauldron" | "brewing_stand" | "lectern" | "jukebox" | "note_block" | "bookshelf" | "chiseled_bookshelf" | "beehive" | "bee_nest"
    ) {
        "a container or workstation (BaseEntityBlock / its class overriding no canSurvive)"
    } else if matches!(
        id,
        "redstone_block" | "redstone_lamp" | "observer" | "piston" | "sticky_piston" | "daylight_detector" | "target_block" | "sculk_sensor" | "calibrated_sculk_sensor" | "tnt" | "copper_bulb"
            | "exposed_copper_bulb" | "weathered_copper_bulb" | "oxidized_copper_bulb" | "waxed_copper_bulb" | "waxed_exposed_copper_bulb" | "waxed_weathered_copper_bulb" | "waxed_oxidized_copper_bulb"
    ) {
        "a redstone component that stands on its own (its class overriding no canSurvive; a piston's base keeps itself, its head is PistonHeadBlock's)"
    } else if id == "tripwire" {
        "TripWireBlock: no canSurvive (string keeps itself; its hooks' TripWireHookBlock.canSurvive is modelled)"
    } else if matches!(id, "water" | "lava") {
        "LiquidBlock: no canSurvive (a fluid spreads or dries by its own ticks)"
    } else if matches!(id, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg" | "pointed_dripstone_block") || id.ends_with("_concrete_powder") {
        "a FallingBlock: no canSurvive (unsupported it falls by its own tick, it never pops)"
    } else if matches!(id, "nether_portal" | "end_portal" | "end_gateway" | "spawner" | "trial_spawner" | "vault") {
        "a portal or spawner block: no canSurvive (a nether portal keeps itself by updateShape)"
    } else {
        return None;
    })
}

/// (vcompose / G1 Nov 14 14:58) A block id's 26.3 survival-rule citation: the kinds whose class overrides `canSurvive`
/// (the block pops when its support goes), so a stand-in of one names its `support=`. None: not listed.
pub fn survival_rule(id: &str) -> Option<&'static str> {
    let id = id.trim_start_matches("minecraft:");
    let ends = |s: &[&str]| s.iter().any(|x| id.ends_with(x));
    Some(if id == "lever" || ends(&["_button"]) {
        "FaceAttachedHorizontalDirectionalBlock.canSurvive: the face it hangs on is sturdy"
    } else if matches!(id, "torch" | "soul_torch" | "redstone_torch" | "copper_torch") {
        "BaseTorchBlock.canSurvive: canSupportCenter below"
    } else if matches!(id, "wall_torch" | "soul_wall_torch" | "redstone_wall_torch" | "copper_wall_torch") {
        "WallTorchBlock.canSurvive: the face behind is sturdy"
    } else if id == "redstone_wire" {
        "RedStoneWireBlock.canSurvive: canSurviveOn below (a sturdy top face, or a hopper)"
    } else if matches!(id, "repeater" | "comparator") {
        "DiodeBlock.canSurvive: canSupportRigidBlock below"
    } else if matches!(id, "rail" | "powered_rail" | "detector_rail" | "activator_rail") {
        "BaseRailBlock.canSurvive: canSupportRigidBlock below"
    } else if ends(&["_pressure_plate"]) {
        "BasePressurePlateBlock.canSurvive: canSupportRigidBlock or canSupportCenter below"
    } else if ends(&["_carpet"]) {
        "CarpetBlock.canSurvive: not air below"
    } else if ends(&["_wall_hanging_sign"]) {
        "WallHangingSignBlock.canSurvive: the face behind can attach"
    } else if ends(&["_hanging_sign"]) {
        "CeilingHangingSignBlock.canSurvive: the face above is sturdy"
    } else if ends(&["_wall_sign"]) {
        "WallSignBlock.canSurvive: the block behind is solid"
    } else if ends(&["_sign"]) {
        "StandingSignBlock.canSurvive: the block below is solid"
    } else if id == "tripwire_hook" {
        "TripWireHookBlock.canSurvive: the face behind is sturdy"
    } else if id == "ladder" {
        "LadderBlock.canSurvive: the face behind is sturdy"
    } else if id == "vine" {
        "VineBlock.canSurvive: a face it can attach to"
    } else if ends(&["_door"]) {
        "DoorBlock.canSurvive: the lower half on a sturdy top face"
    } else if matches!(id, "lantern" | "soul_lantern") || ends(&["_lantern"]) && !ends(&["jack_o_lantern", "sea_lantern"]) {
        "LanternBlock.canSurvive: canSupportCenter above (hanging) or below"
    } else if id == "bell" {
        "BellBlock.canSurvive: its attachment face"
    } else if ends(&["_wall_banner"]) {
        "WallBannerBlock.canSurvive: the block behind is solid"
    } else if ends(&["_banner"]) {
        "BannerBlock.canSurvive: the block below is solid"
    } else if id == "candle" || ends(&["_candle"]) {
        "CandleBlock.canSurvive: canSupportCenter below"
    } else if id == "snow" {
        "SnowLayerBlock.canSurvive: a full top face below (not ice, not a barrier)"
    } else if id == "scaffolding" {
        "ScaffoldingBlock.canSurvive: its distance under 7"
    } else if matches!(id, "pointed_dripstone" | "amethyst_cluster" | "small_amethyst_bud" | "medium_amethyst_bud" | "large_amethyst_bud" | "glow_lichen" | "sculk_vein" | "hanging_roots" | "spore_blossom") {
        "PointedDripstoneBlock / AmethystClusterBlock / MultifaceBlock / HangingRootsBlock / SporeBlossomBlock.canSurvive: its attachment face"
    } else if matches!(
        id,
        "dandelion" | "poppy" | "blue_orchid" | "allium" | "azure_bluet" | "oxeye_daisy" | "cornflower" | "lily_of_the_valley" | "wither_rose" | "torchflower" | "short_grass" | "fern"
            | "dead_bush" | "sweet_berry_bush" | "brown_mushroom" | "red_mushroom" | "crimson_fungus" | "warped_fungus" | "crimson_roots" | "warped_roots" | "nether_sprouts" | "wheat" | "carrots"
            | "potatoes" | "beetroots" | "nether_wart" | "sugar_cane" | "cactus" | "bamboo" | "bamboo_sapling" | "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass" | "lily_pad" | "cocoa"
            | "tall_grass" | "large_fern" | "sunflower" | "lilac" | "rose_bush" | "peony" | "sea_pickle" | "big_dripleaf" | "big_dripleaf_stem" | "small_dripleaf" | "azalea"
            | "flowering_azalea" | "mangrove_propagule" | "pink_petals" | "chorus_plant" | "chorus_flower" | "weeping_vines" | "weeping_vines_plant" | "twisting_vines" | "twisting_vines_plant"
            | "cave_vines" | "cave_vines_plant" | "frogspawn" | "pitcher_plant" | "pitcher_crop" | "torchflower_crop" | "melon_stem" | "pumpkin_stem" | "attached_melon_stem" | "attached_pumpkin_stem"
    ) || ends(&["_sapling", "_tulip"])
    {
        "a plant (BushBlock and its kin: canSurvive reads the block it grows on or hangs from)"
    } else {
        return None;
    })
}

/// (vcompose / G1 Nov 14 14:58: "support-free" for stand-ins, from part 74's table) Whether a stand-in of the block in
/// the full `state` (`minecraft:lever[face=…]`) must name its `support=`: Ok(false) for a block with no 26.3 survival
/// rule ([`no_survival_rule`], cited), Ok(true) for one whose class overrides `canSurvive` ([`survival_rule`], cited),
/// Err naming the block when it is in neither list (never defaulted: cite it in the census first).
pub fn needs_support(state: &str) -> Result<bool, String> {
    let id = state.split('[').next().unwrap_or(state).trim();
    if id.is_empty() {
        return Err(format!("needs_support: {state:?}: no block id"));
    }
    if no_survival_rule(id).is_some() {
        return Ok(false);
    }
    if survival_rule(id).is_some() {
        return Ok(true);
    }
    Err(format!("needs_support: {id} is in neither survival list (no 26.3 rule cited, no canSurvive class cited): cite it in `survivalcensus` before a stand-in of it omits or names its support"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (vcompose / G1 Nov 14 14:58) `needs_support`: a block with no survival rule may omit `support=` (stone, glass, a
    /// redstone block, string); one whose class overrides canSurvive must name it (a lever, a crimson wall sign, a
    /// torch, a rail, dust, a repeater, a tripwire hook, a carpet); a block in neither list is refused by name; no id is
    /// refused; the two lists never overlap on the stand-in kinds
    #[test]
    fn needs_support_reads_the_two_lists_and_refuses_the_rest() {
        for s in ["minecraft:stone", "minecraft:glass", "minecraft:redstone_block", "minecraft:tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true]", "minecraft:obsidian", "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]"] {
            assert_eq!(needs_support(s), Ok(false), "{s}: support-free");
        }
        for s in [
            "minecraft:lever[face=ceiling,facing=north,powered=false]",
            "minecraft:crimson_wall_sign[facing=east,waterlogged=false]",
            "minecraft:redstone_torch[lit=true]",
            "minecraft:redstone_wall_torch[facing=east,lit=true]",
            "minecraft:rail[shape=north_south,waterlogged=false]",
            "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]",
            "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]",
            "minecraft:tripwire_hook[attached=true,facing=east,powered=false]",
            "minecraft:white_carpet",
            "minecraft:stone_button[face=floor,facing=north,powered=false]",
        ] {
            assert_eq!(needs_support(s), Ok(true), "{s}: names its support");
        }
        let e = needs_support("minecraft:flower_pot").unwrap_err();
        assert!(e.contains("flower_pot") && e.contains("neither"), "an unlisted block refused by name: {e}");
        assert!(needs_support("").is_err() && needs_support("[x=1]").is_err(), "no id: refused");
        for id in ["lever", "redstone_torch", "rail", "redstone_wire", "repeater", "tripwire_hook", "white_carpet", "crimson_wall_sign", "ladder", "oak_door"] {
            assert!(no_survival_rule(id).is_none(), "{id}: in the rule list only");
        }
    }
}
