//! Top-down maps of a terrain file (for choosing spots by eye): each column's top block
//! (WORLD_SURFACE - 1) as a letter, entities over it.

use redsim::terrain::format::{Heightmap, Terrain};

/// One letter per top block.
fn letter(state: &str) -> char {
    let name = state.trim_start_matches("minecraft:");
    let name = name.split('[').next().unwrap_or(name);
    match name {
        "grass_block" => '.',
        "dirt" | "coarse_dirt" | "rooted_dirt" => ',',
        "dirt_path" => '=',
        "farmland" => 'f',
        "water" => '~',
        "lava" => 'L',
        "sand" | "gravel" => ':',
        "bell" => '@',
        n if n.ends_with("_bed") => 'B',
        n if n.ends_with("_leaves") => 'T',
        n if n.ends_with("_log") || n.ends_with("_wood") => 'O',
        "short_grass" | "tall_grass" | "fern" | "large_fern" => '"',
        n if n.ends_with("_planks") || n.ends_with("_stairs") || n.ends_with("_slab") || n.contains("cobblestone") || n.ends_with("_door") || n.ends_with("glass_pane") || n.ends_with("_fence") || n.ends_with("_wall") => '#',
        "hay_block" | "composter" | "barrel" | "smoker" | "blast_furnace" | "lectern" | "cartography_table" | "fletching_table" | "smithing_table" | "stonecutter" | "loom" | "grindstone" | "cauldron" | "brewing_stand" => 'J',
        "torch" | "wall_torch" | "lantern" => '*',
        "stone" | "andesite" | "granite" | "diorite" | "tuff" | "deepslate" => 's',
        n if n.contains("flower") || matches!(n, "dandelion" | "poppy" | "azure_bluet" | "oxeye_daisy" | "cornflower" | "allium" | "lily_of_the_valley") || n.ends_with("_tulip") => '*',
        _ => '?',
    }
}

/// The map, north up (z grows downward), x across; a ruler every 8 columns.
pub fn top_map(t: &Terrain, c: [i32; 2], r: i32) -> String {
    let mut out = String::new();
    out.push_str(&format!("map around x={} z={} (radius {}): x across from {} (every 8th marked), z down from {}\n", c[0], c[1], r, c[0] - r, c[1] - r));
    let mut ents: std::collections::HashMap<(i32, i32), char> = std::collections::HashMap::new();
    for e in &t.entities {
        let ch = match e.id.trim_start_matches("minecraft:") {
            "villager" => 'v',
            "iron_golem" => 'G',
            "cat" => 'c',
            _ => continue,
        };
        ents.insert((e.pos[0].floor() as i32, e.pos[2].floor() as i32), ch);
    }
    out.push_str("      ");
    for x in c[0] - r..c[0] + r {
        out.push(if (x - (c[0] - r)) % 8 == 0 { '|' } else { ' ' });
    }
    out.push('\n');
    for z in c[1] - r..c[1] + r {
        out.push_str(&format!("{z:>5} "));
        for x in c[0] - r..c[0] + r {
            if let Some(ch) = ents.get(&(x, z)) {
                out.push(*ch);
                continue;
            }
            let ch = match t.height(Heightmap::WorldSurface, x, z) {
                Some(h) => t.state(x, h - 1, z).map(letter).unwrap_or(' '),
                None => ' ',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}
