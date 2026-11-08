//! `mctas terrain blockinfo`: the natural-state table (`crates/redsim/src/terrain/blocks.tsv`),
//! every block state of the given terrain files described by the game itself (the bridge's
//! `blockinfo` op calls the state's own methods: collision boxes, light, solidity, spawn rules per
//! entity type, tags, heightmap predicates...). `--check` re-dumps and compares with the table.

use anyhow::{Context, Result, bail};
use mctas::sim::{SandboxOptions, Worker, WorkerConfig};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The entity types whose spawn rules the table records per state (`isValidSpawn` and
/// `isBlockDangerous`): every type of the overworld biome lists, the village's and the custom
/// spawners' (the brief's list). Order = the bit order of the table's columns.
pub const TYPES: &[&str] = &[
    "zombie", "zombie_villager", "husk", "drowned", "skeleton", "stray", "bogged", "creeper", "spider", "cave_spider", "enderman", "witch", "slime", "phantom", "pillager", "vindicator", "evoker",
    "ravager", "vex", "illusioner", "bat", "sheep", "pig", "chicken", "cow", "horse", "donkey", "mule", "rabbit", "wolf", "fox", "cat", "ocelot", "parrot", "llama", "trader_llama", "polar_bear", "goat",
    "frog", "armadillo", "camel", "panda", "mooshroom", "squid", "glow_squid", "cod", "salmon", "tropical_fish", "pufferfish", "dolphin", "axolotl", "turtle", "iron_golem", "villager",
    "wandering_trader", "zombie_horse", "skeleton_horse", "sulfur_cube", "bee",
];

/// The block methods whose override makes a state react to its surroundings (the inert flag).
const REACTIVE: &[&str] = &["updateShape", "neighborChanged", "tick", "randomTick", "onPlace", "canSurvive", "entityInside", "stepOn", "fallOn", "onRemove", "affectNeighborsAfterRemoval"];

/// The block tags the rules read (spawn floors, holdable, climbing, doors, fire...); the table
/// keeps only these.
fn rule_tag(t: &str) -> bool {
    let t = t.trim_start_matches("minecraft:");
    t.contains("spawnable_on")
        || matches!(
            t,
            "prevent_mob_spawning_inside" | "enderman_holdable" | "leaves" | "logs" | "dirt" | "climbable" | "doors" | "wooden_doors" | "mob_interactable_doors" | "fence_gates" | "fences" | "walls" | "trapdoors" | "beds" | "infiniburn_overworld" | "fire" | "snow" | "ice" | "sand" | "replaceable" | "replaceable_by_trees" | "blocks_motion" | "blocks_motion_no_leaves" | "blocks_motion_in_heightmap" | "blocks_motion_in_heightmap_no_leaves" | "unstable_bottom_center" | "portals" | "rails" | "bee_growables" | "crops" | "saplings" | "flowers" | "small_flowers" | "baby_turtle_hatches_on" | "dripstone_replaceable_blocks" | "stone_ore_replaceables" | "deepslate_ore_replaceables" | "base_stone_overworld" | "wool" | "planks" | "wooden_slabs" | "slabs" | "stairs"
        )
}

/// Every state of the files, raw palette strings grouped later by their canonical name.
fn palettes(files: &[std::path::PathBuf]) -> Result<BTreeSet<String>> {
    let mut all = BTreeSet::new();
    for f in files {
        let t = crate::cli::read(f)?;
        all.extend(t.palette.iter().cloned());
    }
    Ok(all)
}

/// The game's description of each state (one bridge worker, one sandbox).
pub fn describe(version: &str, states: &[String]) -> Result<Vec<Value>> {
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 1536, ..WorkerConfig::default() })?;
    let sb = w.create(&SandboxOptions::default())?;
    let types: Vec<String> = TYPES.iter().map(|t| format!("minecraft:{t}")).collect();
    let mut out = Vec::new();
    for chunk in states.chunks(64) {
        let v = w.call(json!({"op": "blockinfo", "sandbox": sb, "states": chunk, "types": types}))?;
        let arr = v["states"].as_array().context("blockinfo: no states")?;
        if arr.len() != chunk.len() {
            bail!("blockinfo answered {} states for {}", arr.len(), chunk.len());
        }
        out.extend(arr.iter().cloned());
    }
    Ok(out)
}

fn b(v: &Value, k: &str) -> &'static str {
    if v[k].as_bool().unwrap_or(false) { "1" } else { "0" }
}

fn num(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// One table row (tab-separated, in `COLUMNS` order).
fn row(v: &Value, aliases: &[String]) -> String {
    let classes: Vec<&str> = v["classes"].as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect()).unwrap_or_default();
    let declared: Vec<&str> = v["declared"].as_array().map(|a| a.iter().filter_map(|c| c.as_str()).filter(|m| REACTIVE.contains(m)).collect()).unwrap_or_default();
    let boxes: Vec<String> = v["collision"]
        .as_array()
        .map(|a| a.iter().map(|bx| bx.as_array().map(|c| c.iter().map(|x| x.as_f64().unwrap_or(0.0).to_string()).collect::<Vec<_>>().join(" ")).unwrap_or_default()).collect())
        .unwrap_or_default();
    let faces = match v["faces"].as_array() {
        Some(f) => f.iter().map(|rows| rows.as_array().map(|r| r.iter().map(|x| format!("{:04x}", x.as_u64().unwrap_or(0))).collect::<String>()).unwrap_or_default()).collect::<Vec<_>>().join(" "),
        None => "-".into(),
    };
    let tags: Vec<&str> = v["tags"].as_array().map(|a| a.iter().filter_map(|c| c.as_str()).filter(|t| rule_tag(t)).collect()).unwrap_or_default();
    let path = format!("{}{}{}", b(v, "path_land"), b(v, "path_water"), b(v, "path_air"));
    let fields: Vec<String> = vec![
        v["name"].as_str().unwrap_or("?").to_string(),
        if aliases.is_empty() { "-".into() } else { aliases.join(" ") },
        classes.join(","),
        if declared.is_empty() { "-".into() } else { declared.join(",") },
        if boxes.is_empty() { "-".into() } else { boxes.join(";") },
        b(v, "can_occlude").into(),
        b(v, "shape_occlusion").into(),
        faces,
        num(v, "dampening"),
        num(v, "emission"),
        b(v, "skylight_down").into(),
        b(v, "solid").into(),
        b(v, "solid_render").into(),
        b(v, "conductor").into(),
        b(v, "full_collision").into(),
        v["sturdy"].as_str().unwrap_or("000000").into(),
        b(v, "signal").into(),
        b(v, "air").into(),
        b(v, "liquid").into(),
        b(v, "replaceable").into(),
        b(v, "suffocating").into(),
        path,
        v["fluid"].as_str().unwrap_or("minecraft:empty").into(),
        num(v, "fluid_amount"),
        b(v, "fluid_source").into(),
        if tags.is_empty() { "-".into() } else { tags.join(",") },
        b(v, "leaves_block").into(),
        b(v, "random_ticking").into(),
        num(v, "friction"),
        num(v, "speed_factor"),
        num(v, "jump_factor"),
        v["push"].as_str().unwrap_or("NORMAL").into(),
        b(v, "block_entity").into(),
        v["heightmaps"].as_str().unwrap_or("0000").into(),
        v["valid_spawn"].as_str().unwrap_or("").into(),
        v["dangerous"].as_str().unwrap_or("").into(),
        num(v, "burn_odds"),
        num(v, "ignite_odds"),
        num(v, "resistance"),
        v["support"].as_str().unwrap_or("111111").into(),
        b(v, "ignited_by_lava").into(),
    ];
    fields.join("\t")
}

/// The table's columns (its header line).
pub const COLUMNS: &str = "name\taliases\tclasses\treactive\tcollision\tcan_occlude\tshape_occlusion\tfaces\tdampening\temission\tskylight_down\tsolid\tsolid_render\tconductor\tfull_collision\tsturdy\tsignal\tair\tliquid\treplaceable\tsuffocating\tpath\tfluid\tfluid_amount\tfluid_source\ttags\tleaves_block\trandom_ticking\tfriction\tspeed_factor\tjump_factor\tpush\tblock_entity\theightmaps\tvalid_spawn\tdangerous\tburn_odds\tignite_odds\tresistance\tsupport\tignited_by_lava";

/// Build the table text from the files' palettes (and `extra` states).
/// Every possible state of each block id (the bridge's `blockstates`).
pub fn expand(version: &str, ids: &[String]) -> Result<Vec<String>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), heap_mb: 1536, ..WorkerConfig::default() })?;
    let v = w.call(json!({"op": "blockstates", "blocks": ids}))?;
    Ok(v["states"].as_array().context("blockstates: no states")?.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
}

pub fn table(version: &str, files: &[std::path::PathBuf], extra: &[String]) -> Result<String> {
    let mut raw = palettes(files)?;
    raw.extend(extra.iter().cloned());
    // every block's bare id too: a 26.x chunk writes a default state as its bare id, so the game's
    // own default state of each block joins the table under that alias
    let bare: Vec<String> = raw.iter().map(|s| s.split('[').next().unwrap_or(s).to_string()).collect();
    raw.extend(bare);
    let raw: Vec<String> = raw.into_iter().collect();
    let described = describe(version, &raw)?;
    let mut rows: BTreeMap<String, (Value, Vec<String>)> = BTreeMap::new();
    for (r, v) in raw.iter().zip(described) {
        let name = v["name"].as_str().context("a state without a name")?.to_string();
        let e = rows.entry(name.clone()).or_insert_with(|| (v.clone(), Vec::new()));
        if *r != name {
            e.1.push(r.clone());
        }
    }
    let mut out = String::new();
    out.push_str(&format!("# The natural-state table (mctas terrain blockinfo; designs/briefs/terrain-and-spawning.md): the game's own answers, {version}.\n"));
    out.push_str(&format!("# types {}\n", TYPES.iter().map(|t| format!("minecraft:{t}")).collect::<Vec<_>>().join(",")));
    out.push_str(COLUMNS);
    out.push('\n');
    for (v, aliases) in rows.values() {
        out.push_str(&row(v, aliases));
        out.push('\n');
    }
    Ok(out)
}

/// `blockinfo`: write the table, or compare it (`check`).
pub fn run(version: &str, files: &[std::path::PathBuf], extra: &[String], out: &Path, check: bool) -> Result<()> {
    let text = table(version, files, extra)?;
    if check {
        let old = std::fs::read_to_string(out).with_context(|| format!("reading {}", out.display()))?;
        if old != text {
            let (a, b): (BTreeSet<&str>, BTreeSet<&str>) = (old.lines().collect(), text.lines().collect());
            for l in b.difference(&a).take(10) {
                eprintln!("new:  {}", l.chars().take(160).collect::<String>());
            }
            for l in a.difference(&b).take(10) {
                eprintln!("old:  {}", l.chars().take(160).collect::<String>());
            }
            bail!("{} differs from the game's dump", out.display());
        }
        println!("{}: equal to the game's dump ({} states)", out.display(), text.lines().count() - 3);
    } else {
        std::fs::write(out, &text)?;
        println!("wrote {} ({} states)", out.display(), text.lines().count() - 3);
    }
    Ok(())
}
