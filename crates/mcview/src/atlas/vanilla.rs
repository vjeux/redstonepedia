//! The game's own textures, vendored in the repo (`web/textures/26.3`, the judge's version):
//! the block atlas and its index (`atlas.png`, `data.min.json`), the entity and item
//! atlases, the block definitions and models, and the explosion particle sprites.
//! Each of the viewer's tiles that has a game texture takes it; the hand-drawn tiles
//! stay for the rest and when the vendored files cannot be read.

use super::{Atlas, Rgba, TILE};
use crate::json::Json;

/// The game version the vendored textures are from.
pub const VERSION: &str = "26.3";
/// The vendored textures, repo-relative (`web/textures/<VERSION>`).
pub const DIR: &str = "web/textures/26.3";
/// The vendored textures, as served by the site (same origin as the pages).
pub const SITE_DIR: &str = "textures/26.3";

/// The vendored files' names.
pub const ATLAS: &str = "atlas.png";
pub const INDEX: &str = "data.min.json";
/// The entity and item atlases.
pub const ENTITY_ATLAS: &str = "entities.png";
pub const ENTITY_INDEX: &str = "entities.min.json";
pub const ITEM_ATLAS: &str = "items.png";
pub const ITEM_INDEX: &str = "items.min.json";
/// The block definitions and models.
pub const DEFS: &str = "block_definition.min.json";
pub const MODELS: &str = "model.min.json";
/// The particle sprites' subdirectory (`particle/<name>.png`).
pub const PARTICLE_DIR: &str = "particle";

/// The particle sprites the viewer draws (`crate::particles`): the explosion's 16 frames.
pub fn particle_names() -> Vec<String> {
    (0..crate::particles::FRAMES).map(|i| format!("explosion_{i}")).collect()
}

/// How a game texture becomes a tile.
#[derive(Clone, Copy, Debug)]
enum Op {
    /// As it is (an animated texture: its first frame).
    Plain,
    /// A grayscale texture the game colors (grass, leaves, water): times this color.
    Tint(u32),
    /// The texture's lower half moved to the top (a model whose UVs read the top half).
    LowerToTop,
}

/// (viewer tile, game texture, how).
const MAP: &[(&str, &str, Op)] = &[
    ("stone", "stone", Op::Plain),
    ("smooth_stone", "smooth_stone", Op::Plain),
    ("smooth_stone_slab_side", "smooth_stone_slab_side", Op::Plain),
    ("cobblestone", "cobblestone", Op::Plain),
    ("mossy_cobblestone", "mossy_cobblestone", Op::Plain),
    ("obsidian", "obsidian", Op::Plain),
    ("crying_obsidian", "crying_obsidian", Op::Plain),
    ("granite", "granite", Op::Plain),
    ("polished_granite", "polished_granite", Op::Plain),
    ("diorite", "diorite", Op::Plain),
    ("andesite", "andesite", Op::Plain),
    ("stone_bricks", "stone_bricks", Op::Plain),
    ("tuff", "tuff", Op::Plain),
    ("calcite", "calcite", Op::Plain),
    ("bricks", "bricks", Op::Plain),
    ("sandstone_side", "sandstone", Op::Plain),
    ("sandstone_top", "sandstone_top", Op::Plain),
    ("nether_bricks", "nether_bricks", Op::Plain),
    ("end_stone", "end_stone", Op::Plain),
    ("blackstone", "blackstone", Op::Plain),
    ("quartz_block", "quartz_block_side", Op::Plain),
    ("prismarine", "prismarine", Op::Plain),
    ("purpur_block", "purpur_block", Op::Plain),
    ("terracotta", "terracotta", Op::Plain),
    ("coal_block", "coal_block", Op::Plain),
    ("raw_iron_block", "raw_iron_block", Op::Plain),
    ("mud_bricks", "mud_bricks", Op::Plain),
    ("mud", "mud", Op::Plain),
    ("white_concrete", "white_concrete", Op::Plain),
    ("black_concrete", "black_concrete", Op::Plain),
    ("white_concrete_powder", "white_concrete_powder", Op::Plain),
    ("soul_sand", "soul_sand", Op::Plain),
    ("soul_soil", "soul_soil", Op::Plain),
    ("clay", "clay", Op::Plain),
    ("packed_ice", "packed_ice", Op::Plain),
    ("sand", "sand", Op::Plain),
    ("red_sand", "red_sand", Op::Plain),
    ("gravel", "gravel", Op::Plain),
    ("dirt", "dirt", Op::Plain),
    // the plains grass of the real-game pictures (their measured color over the gray texture)
    ("grass_top", "grass_block_top", Op::Tint(0x8eb972)),
    ("grass_side", "grass_block_side", Op::Plain),
    ("gold_block", "gold_block", Op::Plain),
    ("iron_block", "iron_block", Op::Plain),
    ("emerald_block", "emerald_block", Op::Plain),
    ("diamond_block", "diamond_block", Op::Plain),
    ("lapis_block", "lapis_block", Op::Plain),
    ("netherite_block", "netherite_block", Op::Plain),
    ("redstone_block", "redstone_block", Op::Plain),
    ("amethyst_block", "amethyst_block", Op::Plain),
    ("glowstone", "glowstone", Op::Plain),
    ("sea_lantern", "sea_lantern", Op::Plain),
    ("shroomlight", "shroomlight", Op::Plain),
    ("nether_wart_block", "nether_wart_block", Op::Plain),
    ("warped_wart_block", "warped_wart_block", Op::Plain),
    ("dried_kelp_side", "dried_kelp_side", Op::Plain),
    ("dried_kelp_top", "dried_kelp_top", Op::Plain),
    ("waxed_copper_grate", "copper_grate", Op::Plain),
    ("oak_planks", "oak_planks", Op::Plain),
    ("spruce_planks", "spruce_planks", Op::Plain),
    ("oak_log", "oak_log", Op::Plain),
    ("oak_log_top", "oak_log_top", Op::Plain),
    ("oak_leaves", "oak_leaves", Op::Tint(0x77ab2f)),
    ("bookshelf", "bookshelf", Op::Plain),
    ("crafting_table_top", "crafting_table_top", Op::Plain),
    ("crafting_table_side", "crafting_table_front", Op::Plain),
    ("pumpkin_side", "pumpkin_side", Op::Plain),
    ("pumpkin_top", "pumpkin_top", Op::Plain),
    ("white_glazed_terracotta", "white_glazed_terracotta", Op::Plain),
    ("white_wool", "white_wool", Op::Plain),
    ("orange_wool", "orange_wool", Op::Plain),
    ("black_wool", "black_wool", Op::Plain),
    ("glass", "glass", Op::Plain),
    ("tinted_glass", "tinted_glass", Op::Plain),
    ("white_stained_glass", "white_stained_glass", Op::Plain),
    ("black_stained_glass", "black_stained_glass", Op::Plain),
    ("orange_stained_glass", "orange_stained_glass", Op::Plain),
    ("lime_stained_glass", "lime_stained_glass", Op::Plain),
    ("slime_block", "slime_block", Op::Plain),
    ("honey_block", "honey_block_side", Op::Plain),
    ("dust_dot", "redstone_dust_dot", Op::Plain),
    ("dust_line", "redstone_dust_line0", Op::Plain),
    ("torch_on", "redstone_torch", Op::Plain),
    ("torch_off", "redstone_torch_off", Op::Plain),
    ("lever_handle", "lever", Op::Plain),
    ("repeater_top", "repeater", Op::Plain),
    ("repeater_top_on", "repeater_on", Op::Plain),
    ("comparator_top", "comparator", Op::Plain),
    ("comparator_top_on", "comparator_on", Op::Plain),
    ("bedrock", "bedrock", Op::Plain),
    ("observer_front", "observer_front", Op::Plain),
    ("observer_back", "observer_back", Op::Plain),
    ("observer_back_on", "observer_back_on", Op::Plain),
    ("observer_side", "observer_side", Op::Plain),
    ("observer_top", "observer_top", Op::Plain),
    ("lamp_off", "redstone_lamp", Op::Plain),
    ("lamp_on", "redstone_lamp_on", Op::Plain),
    ("bulb_0", "copper_bulb", Op::Plain),
    ("bulb_0_lit", "copper_bulb_lit", Op::Plain),
    ("bulb_0_p", "copper_bulb_powered", Op::Plain),
    ("bulb_0_lit_p", "copper_bulb_lit_powered", Op::Plain),
    ("bulb_1", "exposed_copper_bulb", Op::Plain),
    ("bulb_1_lit", "exposed_copper_bulb_lit", Op::Plain),
    ("bulb_1_p", "exposed_copper_bulb_powered", Op::Plain),
    ("bulb_1_lit_p", "exposed_copper_bulb_lit_powered", Op::Plain),
    ("bulb_2", "weathered_copper_bulb", Op::Plain),
    ("bulb_2_lit", "weathered_copper_bulb_lit", Op::Plain),
    ("bulb_2_p", "weathered_copper_bulb_powered", Op::Plain),
    ("bulb_2_lit_p", "weathered_copper_bulb_lit_powered", Op::Plain),
    ("bulb_3", "oxidized_copper_bulb", Op::Plain),
    ("bulb_3_lit", "oxidized_copper_bulb_lit", Op::Plain),
    ("bulb_3_p", "oxidized_copper_bulb_powered", Op::Plain),
    ("bulb_3_lit_p", "oxidized_copper_bulb_lit_powered", Op::Plain),
    ("note_block", "note_block", Op::Plain),
    ("target_side", "target_side", Op::Plain),
    ("target_top", "target_top", Op::Plain),
    ("iron_trapdoor", "iron_trapdoor", Op::Plain),
    ("oak_trapdoor", "oak_trapdoor", Op::Plain),
    ("iron_door_top", "iron_door_top", Op::Plain),
    ("iron_door_bottom", "iron_door_bottom", Op::Plain),
    ("oak_door_top", "oak_door_top", Op::Plain),
    ("oak_door_bottom", "oak_door_bottom", Op::Plain),
    ("rail", "rail", Op::Plain),
    ("rail_corner", "rail_corner", Op::Plain),
    ("powered_rail", "powered_rail", Op::Plain),
    ("powered_rail_on", "powered_rail_on", Op::Plain),
    ("activator_rail", "activator_rail", Op::Plain),
    ("activator_rail_on", "activator_rail_on", Op::Plain),
    ("detector_rail", "detector_rail", Op::Plain),
    ("furnace_side", "furnace_side", Op::Plain),
    ("furnace_top", "furnace_top", Op::Plain),
    ("dispenser_front", "dispenser_front", Op::Plain),
    ("dropper_front", "dropper_front", Op::Plain),
    ("dispenser_front_vertical", "dispenser_front_vertical", Op::Plain),
    ("dropper_front_vertical", "dropper_front_vertical", Op::Plain),
    ("crafter_top", "crafter_top", Op::Plain),
    ("crafter_side", "crafter_east", Op::Plain),
    ("crafter_front", "crafter_north", Op::Plain),
    ("crafter_front_on", "crafter_north_crafting", Op::Plain),
    ("hopper_outside", "hopper_outside", Op::Plain),
    ("hopper_inside", "hopper_inside", Op::Plain),
    ("hopper_top", "hopper_top", Op::Plain),
    ("piston_side", "piston_side", Op::Plain),
    ("piston_top", "piston_top", Op::Plain),
    ("piston_top_sticky", "piston_top_sticky", Op::Plain),
    ("piston_bottom", "piston_bottom", Op::Plain),
    ("piston_inner", "piston_inner", Op::Plain),
    ("sculk_top", "sculk_sensor_top", Op::Plain),
    ("sculk_side", "sculk_sensor_side", Op::Plain),
    ("sculk_tendril", "sculk_sensor_tendril_inactive", Op::LowerToTop),
    ("sculk_tendril_active", "sculk_sensor_tendril_active", Op::LowerToTop),
    ("amethyst_cluster", "amethyst_cluster", Op::Plain),
    ("cake_top", "cake_top", Op::Plain),
    ("cake_side", "cake_side", Op::Plain),
    ("end_portal_frame_top", "end_portal_frame_top", Op::Plain),
    ("end_portal_frame_side", "end_portal_frame_side", Op::Plain),
    ("ender_eye", "end_portal_frame_eye", Op::Plain),
    ("composter_side", "composter_side", Op::Plain),
    ("compost", "composter_compost", Op::Plain),
    ("respawn_anchor_side", "respawn_anchor_side0", Op::Plain),
    ("respawn_anchor_top", "respawn_anchor_top_off", Op::Plain),
    ("scaffolding_top", "scaffolding_top", Op::Plain),
    ("scaffolding_side", "scaffolding_side", Op::Plain),
    ("jukebox_side", "jukebox_side", Op::Plain),
    ("jukebox_top", "jukebox_top", Op::Plain),
    ("water", "water_still", Op::Tint(0x3f76e4)),
    ("water_flow", "water_flow", Op::Tint(0x3f76e4)),
    ("lava", "lava_still", Op::Plain),
    ("magma", "magma", Op::Plain),
    ("dripleaf_top", "big_dripleaf_top", Op::Plain),
    ("tripwire", "tripwire", Op::Plain),
    ("snow", "snow", Op::Plain),
    ("podzol_top", "podzol_top", Op::Plain),
    ("daylight_detector_top", "daylight_detector_top", Op::Plain),
];

/// Decode a PNG to RGBA8 (any color type, 8-bit).
pub fn decode_png(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    crate::staged::block_on(decode_png_staged(bytes, &crate::staged::Budget::none()))
}

/// [`decode_png`] as staged work (`crate::staged`): row by row, pausing every 16 rows once the
/// budget's time is up (the browser decodes the game's atlases over several tasks: the entity
/// atlas took 55 ms in one; the coordinator 2026-10-22). The same pixels (the decoder's own rows;
/// an interlaced image at once).
pub async fn decode_png_staged(bytes: &[u8], b: &crate::staged::Budget) -> Result<(Vec<u8>, u32, u32), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("png too big")?];
    let (w, h) = (r.info().width, r.info().height);
    let (color, _) = r.output_color_type();
    let size = if r.info().interlaced {
        r.next_frame(&mut buf).map_err(|e| e.to_string())?.buffer_size()
    } else {
        let line = r.output_line_size(w).ok_or("png too big")?;
        for y in 0..h as usize {
            r.read_row(&mut buf[y * line..(y + 1) * line]).map_err(|e| e.to_string())?;
            if y % 16 == 15 {
                b.pause().await;
            }
        }
        line * h as usize
    };
    let px = &buf[..size];
    let rgba: Vec<u8> = match color {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
        png::ColorType::Indexed => return Err("indexed png not expanded".into()),
    };
    Ok((rgba, w, h))
}

#[cfg(test)]
mod staged_png_tests {
    /// The decoder's whole frame at once (`next_frame`): the oracle of the row by row decode.
    fn decode_whole(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut r = dec.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        let px = &buf[..info.buffer_size()];
        let rgba = match info.color_type {
            png::ColorType::Rgba => px.to_vec(),
            png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            png::ColorType::Grayscale => px.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
            png::ColorType::Indexed => panic!("indexed"),
        };
        (rgba, info.width, info.height)
    }

    /// Decoded row by row and paused every 16 rows, an image is the decoder's whole frame: RGBA,
    /// RGB, gray (8 and 16 bits), a palette, odd sizes.
    #[test]
    fn a_staged_png_decode_is_the_whole_frame() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed as u8
        };
        for (w, h, color, depth) in [(97u32, 53u32, png::ColorType::Rgba, png::BitDepth::Eight), (64, 200, png::ColorType::Rgb, png::BitDepth::Eight), (33, 17, png::ColorType::Grayscale, png::BitDepth::Sixteen), (40, 40, png::ColorType::GrayscaleAlpha, png::BitDepth::Eight), (31, 70, png::ColorType::Indexed, png::BitDepth::Eight)] {
            let samples = match color {
                png::ColorType::Rgba => 4,
                png::ColorType::Rgb => 3,
                png::ColorType::GrayscaleAlpha => 2,
                _ => 1,
            } * if depth == png::BitDepth::Sixteen { 2 } else { 1 };
            // smooth rows with noise: every filter type gets used
            let data: Vec<u8> = (0..w * h * samples).map(|i| ((i % 251) as u8).wrapping_add(rnd() % 7)).collect();
            let mut bytes = Vec::new();
            {
                let mut e = png::Encoder::new(&mut bytes, w, h);
                e.set_color(color);
                e.set_depth(depth);
                if color == png::ColorType::Indexed {
                    e.set_palette((0..768).map(|i| (i * 7 % 256) as u8).collect::<Vec<u8>>());
                }
                let mut wr = e.write_header().unwrap();
                wr.write_image_data(&data).unwrap();
            }
            let b = crate::staged::Budget::with_clock(std::rc::Rc::new(|| 1.0));
            let mut task = crate::staged::Task::new({
                let bytes = bytes.clone();
                let b = b.clone();
                async move { super::decode_png_staged(&bytes, &b).await }
            });
            let staged = loop {
                if let Some(r) = task.poll() {
                    break r.unwrap();
                }
            };
            assert!(h < 16 || b.pauses() >= h / 16 - 1, "{} pauses for {h} rows", b.pauses());
            assert!(staged == decode_whole(&bytes), "{color:?} {depth:?} {w}x{h}");
        }
    }
}

impl Atlas {
    /// [`Atlas::apply_vanilla`] from the atlas file's PNG bytes.
    pub fn apply_vanilla_png(&mut self, png_bytes: &[u8], index: &str) -> Result<usize, String> {
        let (rgba, w, h) = decode_png(png_bytes)?;
        self.apply_vanilla(&rgba, w, h, index)
    }

    /// Take the game's textures from the vendored block atlas (`rgba`, `w` x `h`) and its index
    /// (`data.min.json`): every mapped tile found is replaced. Returns how many were.
    pub fn apply_vanilla(&mut self, rgba: &[u8], w: u32, h: u32, index: &str) -> Result<usize, String> {
        let idx = Json::parse(index).map_err(|e| format!("atlas index: {e}"))?;
        let mut n = 0;
        for (tile, tex, op) in MAP {
            let Some(&layer) = self.names.get(tile) else { continue };
            let Some(r) = idx.get(&format!("block/{tex}")).and_then(Json::as_arr) else { continue };
            let v: Vec<u32> = r.iter().filter_map(|x| x.as_f64()).map(|x| x as u32).collect();
            if v.len() != 4 || v[2] < TILE as u32 || v[3] < TILE as u32 || v[0] + TILE as u32 > w || v[1] + TILE as u32 > h {
                continue;
            }
            let (x0, y0) = (v[0], v[1]);
            let mut px: [Rgba; TILE * TILE] = [[0; 4]; TILE * TILE];
            for y in 0..TILE as u32 {
                for x in 0..TILE as u32 {
                    let sy = match op {
                        Op::LowerToTop if y < 8 => y + 8,
                        Op::LowerToTop => {
                            continue;
                        }
                        _ => y,
                    };
                    let o = (((y0 + sy) * w + x0 + x) * 4) as usize;
                    let mut c = [rgba[o], rgba[o + 1], rgba[o + 2], rgba[o + 3]];
                    if let Op::Tint(t) = op {
                        let tint = [(t >> 16) & 255, (t >> 8) & 255, t & 255];
                        for k in 0..3 {
                            c[k] = (c[k] as u32 * tint[k] / 255) as u8;
                        }
                    }
                    px[(y * TILE as u32 + x) as usize] = c;
                }
            }
            self.set_layer(layer, &px);
            n += 1;
        }
        Ok(n)
    }
}
