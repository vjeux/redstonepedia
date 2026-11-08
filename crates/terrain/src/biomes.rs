//! `mctas terrain biomes`: the biome table (`crates/redsim/src/terrain/biomes.tsv`) from the game's
//! own data (`data/minecraft/worldgen/biome/*.json` and the biome tags): per biome its climate,
//! colours, the tags the spawn rules read and its natural spawns
//! (`minecraft:gameplay/natural_mob_spawns`: per category the type, weight and count; spawn costs).

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The biome tags the rules read (`BiomeTags`).
pub const TAGS: &[&str] = &["more_frequent_drowned_spawns", "allows_surface_slime_spawns", "reduce_water_ambient_spawns", "without_zombie_sieges", "is_ocean", "is_river"];

fn read_json(p: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// A biome tag's members, nested tags resolved.
fn tag_members(data: &Path, tag: &str, out: &mut BTreeSet<String>) -> Result<()> {
    let p = data.join("tags/worldgen/biome").join(format!("{tag}.json"));
    if !p.exists() {
        return Ok(());
    }
    for v in read_json(&p)?["values"].as_array().cloned().unwrap_or_default() {
        let s = v.as_str().map(|s| s.to_string()).or_else(|| v["id"].as_str().map(|s| s.to_string())).unwrap_or_default();
        if let Some(inner) = s.strip_prefix("#minecraft:") {
            tag_members(data, inner, out)?;
        } else if !s.is_empty() {
            out.insert(s);
        }
    }
    Ok(())
}

fn count_of(v: &Value) -> (i64, i64) {
    match v {
        Value::Number(n) => {
            let c = n.as_i64().unwrap_or(1);
            (c, c)
        }
        Value::Object(_) => (v["min_inclusive"].as_i64().unwrap_or(1), v["max_inclusive"].as_i64().unwrap_or(1)),
        _ => (1, 1),
    }
}

fn color(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim_start_matches('#').to_string(),
        Value::Number(n) => format!("{:06x}", n.as_i64().unwrap_or(0)),
        _ => "-".into(),
    }
}

/// The structure table text (`structures.tsv`): per structure its `spawn_overrides` (per category:
/// the bounding box type and the spawns, `ChunkGenerator.getMobsAt`).
pub fn structures_table(version: &str) -> Result<String> {
    let data = mctas::paths::source_dir(version).join("data/minecraft");
    let mut files: Vec<_> = std::fs::read_dir(data.join("worldgen/structure"))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let mut out = format!("# The structure spawn overrides (mctas terrain biomes): the game's own structure data, {version}.\n");
    out.push_str("name\toverrides (category:bounding_box:type/weight/min/max,...;...)\n");
    for f in files {
        let id = format!("minecraft:{}", f.file_stem().unwrap().to_string_lossy());
        let s = read_json(&f)?;
        let mut cats = Vec::new();
        if let Some(o) = s["spawn_overrides"].as_object() {
            for (cat, v) in o {
                let mode = v["bounding_box"].as_str().unwrap_or("piece");
                let spawns: Vec<String> = v["spawns"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|e| {
                        let (lo, hi) = count_of(&e["count"]);
                        format!("{}/{}/{lo}/{hi}", e["type"].as_str().unwrap_or("?"), e["weight"].as_i64().unwrap_or(0))
                    })
                    .collect();
                cats.push(format!("{cat}:{mode}:{}", spawns.join(",")));
            }
        }
        out.push_str(&format!("{id}\t{}\n", if cats.is_empty() { "-".to_string() } else { cats.join(";") }));
    }
    Ok(out)
}

/// A colormap (`textures/colormap/<name>.png`, 256 x 256) as `0xRRGGBB` pixels, row by row.
fn colormap(version: &str, name: &str) -> Result<Vec<u32>> {
    let path = mctas::paths::source_dir(version).with_file_name("src-client").join("assets/minecraft/textures/colormap").join(format!("{name}.png"));
    let file = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("colormap size")?];
    let info = reader.next_frame(&mut buf)?;
    let ch = info.color_type.samples();
    anyhow::ensure!(info.width == 256 && info.height == 256, "{name}.png is {}x{}", info.width, info.height);
    Ok((0..256 * 256).map(|i| ((buf[i * ch] as u32) << 16) | ((buf[i * ch + 1] as u32) << 8) | buf[i * ch + 2] as u32).collect())
}

/// `ColorMapColorUtil.get(temp, rain, pixels, default)` with the biome's clamped climate
/// (`Biome.get*ColorFromTexture`).
fn from_map(map: &[u32], temperature: f64, downfall: f64) -> u32 {
    let temp = temperature.clamp(0.0, 1.0);
    let rain = downfall.clamp(0.0, 1.0) * temp;
    let x = ((1.0 - temp) * 255.0) as usize;
    let y = ((1.0 - rain) * 255.0) as usize;
    let i = (y << 8) | x;
    map.get(i).copied().unwrap_or(0xff00ff)
}

/// The table text.
pub fn table(version: &str) -> Result<String> {
    let maps = [colormap(version, "grass")?, colormap(version, "foliage")?, colormap(version, "dry_foliage")?];
    let data = mctas::paths::source_dir(version).join("data/minecraft");
    let mut tags: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for t in TAGS {
        let mut s = BTreeSet::new();
        tag_members(&data, t, &mut s)?;
        tags.insert(t, s);
    }
    let mut rows = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(data.join("worldgen/biome"))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    for f in files {
        let id = format!("minecraft:{}", f.file_stem().unwrap().to_string_lossy());
        let b = read_json(&f)?;
        let attrs = &b["attributes"];
        let spawns = &attrs["minecraft:gameplay/natural_mob_spawns"]["argument"];
        let mut lists = Vec::new();
        if let Some(by) = spawns["spawns_by_category"].as_object() {
            for (cat, entries) in by {
                for e in entries.as_array().cloned().unwrap_or_default() {
                    let (lo, hi) = count_of(&e["count"]);
                    lists.push(format!("{cat}:{}:{}:{lo}:{hi}", e["type"].as_str().unwrap_or("?"), e["weight"].as_i64().unwrap_or(0)));
                }
            }
        }
        let mut costs = Vec::new();
        if let Some(c) = spawns["spawn_costs"].as_object() {
            for (ty, v) in c {
                costs.push(format!("{ty}:{}:{}", v["charge"].as_f64().unwrap_or(0.0), v["energy_budget"].as_f64().unwrap_or(0.0)));
            }
        }
        let fx = &b["effects"];
        let member: Vec<&str> = TAGS.iter().copied().filter(|t| tags[t].contains(&id)).collect();
        let slime = attrs["minecraft:gameplay/surface_slime_spawn_chance"].as_f64().map(|v| v.to_string()).unwrap_or_else(|| "0".into());
        let (temp, down) = (b["temperature"].as_f64().unwrap_or(0.5), b["downfall"].as_f64().unwrap_or(0.5));
        // the colours the client tints with: the override, else the colormap at the clamped climate
        let resolved = |key: &str, map: &[u32]| -> String {
            match fx[key].as_str() {
                Some(s) => s.trim_start_matches('#').to_string(),
                None => format!("{:06x}", from_map(map, temp, down)),
            }
        };
        let (rg, rf, rd) = (resolved("grass_color", &maps[0]), resolved("foliage_color", &maps[1]), resolved("dry_foliage_color", &maps[2]));
        let fields = [
            id.clone(),
            b["temperature"].as_f64().unwrap_or(0.5).to_string(),
            b["downfall"].as_f64().unwrap_or(0.5).to_string(),
            (b["has_precipitation"].as_bool().unwrap_or(true) as u8).to_string(),
            b["temperature_modifier"].as_str().unwrap_or("none").to_string(),
            color(&fx["water_color"]),
            color(&fx["grass_color"]),
            color(&fx["foliage_color"]),
            color(&fx["dry_foliage_color"]),
            fx["grass_color_modifier"].as_str().unwrap_or("none").to_string(),
            if member.is_empty() { "-".into() } else { member.join(",") },
            slime,
            if lists.is_empty() { "-".into() } else { lists.join(";") },
            if costs.is_empty() { "-".into() } else { costs.join(";") },
            rg,
            rf,
            rd,
        ];
        rows.push(fields.join("\t"));
    }
    let mut out = format!("# The biome table (mctas terrain biomes; designs/briefs/terrain-and-spawning.md): the game's own biome data, {version}.\n");
    out.push_str("name\ttemperature\tdownfall\tprecipitation\ttemperature_modifier\twater_color\tgrass_color\tfoliage_color\tdry_foliage_color\tgrass_modifier\ttags\tsurface_slime_chance\tspawns\tspawn_costs\tgrass_resolved\tfoliage_resolved\tdry_foliage_resolved\n");
    for r in rows {
        out.push_str(&r);
        out.push('\n');
    }
    Ok(out)
}
