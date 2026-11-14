//! (G1 Nov 14 11:25, part 74: CHANGES 1 on part 64) The SURVIVAL CENSUS: the farm loader's survival check keeps a
//! block unless redsim models its 26.3 survival rule (`redsim::behavior::survival_modelled`, `can_survive`'s arms) and
//! the rule fails, or redsim names it NOT MODELED (`survival_not_modeled`: the load fails). A kind that is neither would
//! pass by default, so every kind in every farm design's palette must be one of the three: MODELLED, NOT MODELED, or
//! listed here with NO SURVIVAL RULE in 26.3, cited. The census test reads every `.txt` design under `designs/farms/`
//! (the designs the farm loader lays) and fails naming any kind in none of the three.

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

#[cfg(test)]
mod tests {
    /// (G1 Nov 14 11:25, part 74) THE CENSUS: every kind in every farm design's palette (`designs/farms/**.txt`) is
    /// MODELLED (`survival_modelled`), NOT MODELED (`survival_not_modeled`) or listed with no survival rule
    /// ([`super::no_survival_rule`], cited); the named kinds are modelled (lanterns, doors, pressure plates, torches,
    /// buttons, levers, rails, dust, repeaters, comparators, tripwire hooks) and string has no rule
    #[test]
    fn every_farm_design_kind_is_classified_for_survival() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/farms"));
        let mut stack = vec![root.to_path_buf()];
        let mut files = Vec::new();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "txt") {
                    files.push(p);
                }
            }
        }
        let mut kinds: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for f in &files {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            let Ok(d) = mctas::design::Design::parse(&text) else { continue };
            for s in d.blocks.values().chain(d.nether_blocks.values()) {
                let id = s.split('[').next().unwrap_or(s).to_string();
                kinds.entry(id).or_insert_with(|| s.clone());
            }
        }
        assert!(kinds.len() > 30, "the census read the farm designs ({} files, {} kinds)", files.len(), kinds.len());
        let (mut modelled, mut not_modelled, mut no_rule, mut unparsed) = (0, 0, 0, Vec::new());
        let mut unclassified = Vec::new();
        for (id, state) in &kinds {
            let Ok(b) = redsim::block::Block::parse(state) else {
                unparsed.push(id.clone());
                continue;
            };
            if redsim::behavior::survival_modelled(&b).is_some() {
                modelled += 1;
            } else if redsim::behavior::survival_not_modeled(&b).is_some() {
                not_modelled += 1;
            } else if super::no_survival_rule(id).is_some() {
                no_rule += 1;
            } else {
                unclassified.push(id.clone());
            }
        }
        println!("survival census: {} farm design files, {} kinds: {modelled} modelled, {not_modelled} NOT MODELED, {no_rule} with no survival rule (cited), {} unparsed by redsim (the loader refuses them): {unparsed:?}", files.len(), kinds.len(), unparsed.len());
        assert!(unclassified.is_empty(), "kinds in no class (each must be modelled, NOT MODELED, or listed with a cited no-rule reason): {unclassified:?}");
        let b = |s: &str| redsim::block::Block::parse(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        for s in [
            "minecraft:lantern[hanging=false,waterlogged=false]",
            "minecraft:oak_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
            "minecraft:stone_pressure_plate[powered=false]",
            "minecraft:redstone_torch[lit=true]",
            "minecraft:torch",
            "minecraft:stone_button[face=floor,facing=north,powered=false]",
            "minecraft:lever[face=floor,facing=north,powered=false]",
            "minecraft:rail[shape=north_south,waterlogged=false]",
            "minecraft:powered_rail[powered=false,shape=north_south,waterlogged=false]",
            "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]",
            "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]",
            "minecraft:repeater[delay=1,facing=north,locked=false,powered=false]",
            "minecraft:comparator[facing=north,mode=compare,powered=false]",
            "minecraft:tripwire_hook[attached=false,facing=north,powered=false]",
        ] {
            assert!(redsim::behavior::survival_modelled(&b(s)).is_some(), "{s}: modelled");
        }
        assert!(super::no_survival_rule("minecraft:tripwire").is_some_and(|c| c.contains("TripWireBlock")), "string: no survival rule, cited");
        // (default-deny in the census: a kind no list names is unclassified, never passed)
        assert!(super::no_survival_rule("minecraft:flower_pot").is_none() && super::no_survival_rule("minecraft:ladder").is_none(), "unlisted kinds stay unlisted");
    }
}
