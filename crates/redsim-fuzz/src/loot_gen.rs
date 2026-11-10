//! `redsim-fuzz loot-gen`: the game's block loot tables as redsim needs them, written as
//! `crates/redsim/src/loot_gen.rs` (generated: never edit it by hand). Reads the decompiled data
//! (`data/minecraft/loot_table/blocks/*.json`) and keeps every table of a block redsim can build
//! (`Block::parse` accepts its name, or the name of its wall variant, which shares the table).
//!
//! The context is a block broken with no tool and no explosion radius parameter: a block a piston
//! or a neighbor update destroys (`Block.dropResources`) and a block a TNT explosion destroys with
//! the default `tnt_explosion_drop_decay = false` (`BlockInteraction.DESTROY`: the radius is only
//! added for `DESTROY_WITH_DECAY`). So `survives_explosion` passes and `explosion_decay` does
//! nothing, `tool/*` predicates and `match_tool` fail, `match_block` becomes a test of the block
//! state. A table whose outcome draws from the loot random (a random count, `table_bonus`,
//! `random_chance`, a pool choosing among entries...) is kept as an error: redsim reports it
//! instead of guessing (the loot random is the table's random sequence, not the level random).

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::fmt::Write as _;
use std::path::Path;

/// A loot condition in this context.
#[derive(Clone, Debug, PartialEq)]
enum Cond {
    T,
    F,
    /// The block state's property `k` is `v`.
    Prop(String, String),
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Box<Cond>),
    /// Draws from the loot random (or depends on something redsim does not know).
    Rand(String),
}

fn simplify(c: Cond) -> Cond {
    match c {
        Cond::And(v) => {
            let v: Vec<Cond> = v.into_iter().map(simplify).collect();
            if v.contains(&Cond::F) {
                return Cond::F;
            }
            let v: Vec<Cond> = v.into_iter().filter(|c| *c != Cond::T).collect();
            if let Some(r) = v.iter().find(|c| matches!(c, Cond::Rand(_))) {
                return r.clone();
            }
            match v.len() {
                0 => Cond::T,
                1 => v.into_iter().next().unwrap(),
                _ => Cond::And(v),
            }
        }
        Cond::Or(v) => {
            let v: Vec<Cond> = v.into_iter().map(simplify).collect();
            if v.contains(&Cond::T) {
                return Cond::T;
            }
            let v: Vec<Cond> = v.into_iter().filter(|c| *c != Cond::F).collect();
            if let Some(r) = v.iter().find(|c| matches!(c, Cond::Rand(_))) {
                return r.clone();
            }
            match v.len() {
                0 => Cond::F,
                1 => v.into_iter().next().unwrap(),
                _ => Cond::Or(v),
            }
        }
        Cond::Not(b) => match simplify(*b) {
            Cond::T => Cond::F,
            Cond::F => Cond::T,
            r @ Cond::Rand(_) => r,
            x => Cond::Not(Box::new(x)),
        },
        x => x,
    }
}

/// Rust code for a (simplified, non-constant, non-random) condition: `p(k)` reads the block
/// state's property `k` ("" when absent).
fn emit(c: &Cond) -> String {
    match c {
        Cond::T => "true".into(),
        Cond::F => "false".into(),
        Cond::Prop(k, v) => format!("p({k:?}) == {v:?}"),
        Cond::And(v) => format!("({})", v.iter().map(emit).collect::<Vec<_>>().join(" && ")),
        Cond::Or(v) => format!("({})", v.iter().map(emit).collect::<Vec<_>>().join(" || ")),
        Cond::Not(b) => format!("!{}", emit(b)),
        Cond::Rand(r) => format!("/* random: {r} */ false"),
    }
}

fn strip(id: &str) -> &str {
    id.trim_start_matches("minecraft:")
}

/// A condition (an object, a predicate reference, a list = all of them, or absent = true).
fn cond(v: &Value, block: &str) -> Result<Cond> {
    if v.is_null() {
        return Ok(Cond::T);
    }
    if let Some(s) = v.as_str() {
        // a predicate reference: the tool ones (silk touch, shears...) fail without a tool
        return Ok(if strip(s).starts_with("tool/") { Cond::F } else { Cond::Rand(format!("predicate {s}")) });
    }
    if let Some(a) = v.as_array() {
        return Ok(Cond::And(a.iter().map(|x| cond(x, block)).collect::<Result<_>>()?));
    }
    let ty = strip(v["type"].as_str().unwrap_or(""));
    Ok(match ty {
        "survives_explosion" => Cond::T,
        "match_tool" => Cond::F,
        "match_block" | "block_state_property" => {
            let blocks_ok = match &v["blocks"] {
                Value::Null => true,
                Value::String(s) if s.starts_with('#') => return Ok(Cond::Rand(format!("block tag {s}"))),
                Value::String(s) => strip(s) == block,
                Value::Array(a) => a.iter().any(|x| x.as_str().is_some_and(|s| strip(s) == block)),
                _ => false,
            };
            if !blocks_ok {
                Cond::F
            } else {
                let mut terms = Vec::new();
                if let Some(st) = v["state"].as_object() {
                    for (k, val) in st {
                        match val.as_str() {
                            Some(s) => terms.push(Cond::Prop(k.clone(), s.to_string())),
                            None => return Ok(Cond::Rand(format!("a state range on {k}"))),
                        }
                    }
                }
                Cond::And(terms)
            }
        }
        "any_of" => Cond::Or(v["terms"].as_array().context("any_of terms")?.iter().map(|x| cond(x, block)).collect::<Result<_>>()?),
        "all_of" => Cond::And(v["terms"].as_array().context("all_of terms")?.iter().map(|x| cond(x, block)).collect::<Result<_>>()?),
        "inverted" => Cond::Not(Box::new(cond(&v["term"], block)?)),
        other => Cond::Rand(other.to_string()),
    })
}

/// A condition that must not draw: simplified, or the table's error.
fn fixed(v: &Value, block: &str) -> Result<Cond> {
    match simplify(cond(v, block)?) {
        Cond::Rand(r) => bail!("random: {r}"),
        c => Ok(c),
    }
}

/// `body` guarded by `c`.
fn guard(c: &Cond, body: &str) -> String {
    match c {
        Cond::T => body.to_string(),
        Cond::F => String::new(),
        c if body.is_empty() => {
            let _ = c;
            String::new()
        }
        c => format!("if {} {{ {body} }}", emit(c)),
    }
}

/// The modifiers (loot functions) of an item: a Rust expression for its count.
fn count_expr(m: &Value, block: &str) -> Result<String> {
    let fs: Vec<&Value> = match m {
        Value::Null => Vec::new(),
        Value::Array(a) => a.iter().collect(),
        o => vec![o],
    };
    let mut code = "1u32".to_string();
    for f in fs {
        let ty = strip(f["type"].as_str().unwrap_or(""));
        let fc = fixed(&f["condition"], block)?;
        match ty {
            "set_count" => {
                let n = match &f["count"] {
                    Value::Number(n) => n.as_u64().context("set_count")?,
                    Value::Object(o) if o.get("type").and_then(|t| t.as_str()).map(strip) == Some("constant") => o.get("value").and_then(|x| x.as_u64()).context("constant")?,
                    _ => bail!("random: set_count {}", f["count"]),
                };
                let add = f["add"].as_bool().unwrap_or(false);
                let new = if add { format!("({code} + {n})") } else { format!("{n}u32") };
                code = match &fc {
                    Cond::T => new,
                    Cond::F => code,
                    c => format!("(if {} {{ {new} }} else {{ {code} }})", emit(c)),
                };
            }
            // no explosion radius in this context; components are cosmetic here
            "explosion_decay" | "copy_components" | "copy_state" | "copy_name" | "set_components" | "set_name" | "set_lore" | "copy_custom_data" => {}
            "limit_count" => {
                let l = &f["limit"];
                if let Some(min) = l["min"].as_u64() {
                    code = format!("({code}).max({min})");
                }
                if let Some(max) = l["max"].as_u64() {
                    code = format!("({code}).min({max})");
                }
            }
            other => bail!("random or unsupported: modifier {other}"),
        }
    }
    Ok(code)
}

/// The code of one entry: what it pushes when it runs (its own condition included).
fn entry_code(e: &Value, block: &str) -> Result<String> {
    let ty = strip(e["type"].as_str().unwrap_or(""));
    let c = fixed(&e["condition"], block)?;
    match ty {
        "item" => {
            let name = strip(e["name"].as_str().context("item name")?);
            let count = count_expr(&e["modifier"], block)?;
            Ok(guard(&c, &format!("out.push(({name:?}, {count}));")))
        }
        "alternatives" => {
            // the first child whose condition passes runs
            let mut code = String::new();
            let mut open = false;
            for ch in e["children"].as_array().context("alternatives children")? {
                let cty = strip(ch["type"].as_str().unwrap_or(""));
                if cty != "item" && cty != "empty" {
                    bail!("unsupported: a {cty} inside alternatives");
                }
                let cc = fixed(&ch["condition"], block)?;
                let body = if cty == "item" {
                    let name = strip(ch["name"].as_str().context("item name")?);
                    format!("out.push(({name:?}, {}));", count_expr(&ch["modifier"], block)?)
                } else {
                    String::new()
                };
                match cc {
                    Cond::F => continue,
                    Cond::T => {
                        if open {
                            let _ = write!(code, " else {{ {body} }}");
                        } else {
                            code.push_str(&body);
                        }
                        break;
                    }
                    cc => {
                        let _ = write!(code, "{}if {} {{ {body} }}", if open { " else " } else { "" }, emit(&cc));
                        open = true;
                    }
                }
            }
            Ok(guard(&c, &code))
        }
        "empty" => Ok(String::new()),
        other => bail!("unsupported: entry {other}"),
    }
}

/// The body of a table's arm (Ok) or why it draws randomly / is not covered (Err).
fn table_code(v: &Value, block: &str) -> Result<String> {
    if !v["modifier"].is_null() || !v["functions"].is_null() {
        bail!("unsupported: table-level modifiers");
    }
    let mut code = String::new();
    for pool in v["pools"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let rolls = match &pool["rolls"] {
            Value::Number(n) => n.as_u64().context("rolls")?,
            Value::Object(o) if o.get("type").and_then(|t| t.as_str()).map(strip) == Some("constant") => o.get("value").and_then(|x| x.as_u64()).context("rolls")?,
            r => bail!("random: rolls {r}"),
        };
        if !pool["modifier"].is_null() || !pool["functions"].is_null() {
            bail!("unsupported: pool modifiers");
        }
        let pc = fixed(&pool["condition"], block)?;
        let entries = pool["entries"].as_array().context("entries")?;
        if entries.len() != 1 {
            bail!("random: a pool choosing among {} entries", entries.len());
        }
        let body = entry_code(&entries[0], block)?;
        let body = body.repeat(rolls as usize);
        let g = guard(&pc, &body);
        if !g.is_empty() {
            let _ = write!(code, " {g}");
        }
    }
    Ok(code)
}

/// The wall variant of a floor block (it shares the floor block's table: `wallVariant(base, true)`).
fn wall_variant(name: &str) -> Option<String> {
    for s in ["_banner", "_torch", "_skull", "_head", "_fan"] {
        if let Some(stem) = name.strip_suffix(s) {
            return Some(format!("{stem}_wall{s}"));
        }
    }
    None
}

pub fn run(data: &Path, out: &Path) -> Result<()> {
    let dir = data.join("loot_table/blocks");
    let mut files: Vec<_> = std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let mut arms = String::new();
    let (mut kept, mut random) = (0, 0);
    for f in files {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let buildable = redsim::Block::parse(&format!("minecraft:{name}")).is_ok() || wall_variant(&name).is_some_and(|w| redsim::Block::parse(&format!("minecraft:{w}")).is_ok());
        if !buildable {
            continue;
        }
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f)?).with_context(|| format!("{}", f.display()))?;
        kept += 1;
        match table_code(&v, &name) {
            Ok(code) => {
                let _ = writeln!(arms, "        {name:?} => {{{code} }}");
            }
            Err(e) => {
                random += 1;
                let _ = writeln!(arms, "        {name:?} => return Some(Err({:?})),", format!("{e:#}"));
            }
        }
    }
    let text = format!(
        "//! GENERATED by `redsim-fuzz loot-gen` from the game's block loot tables (do not edit): the\n\
         //! drops of a block broken with no tool and no explosion radius (`Block.dropResources`, and\n\
         //! TNT explosions with drop decay off), for every block redsim can build. {kept} tables, {random}\n\
         //! of them random or not covered (an error naming why).\n\n\
         /// The drops of the block named `name` (`p(k)`: its state's property `k`, \"\" when absent):\n\
         /// `Some(Ok(item, count))`, `Some(Err(why))` for a table that draws randomly, `None` when there\n\
         /// is no table of that name.\n\
         #[allow(clippy::all)]\n\
         pub fn table(name: &str, p: &dyn Fn(&str) -> String) -> Option<Result<Vec<(&'static str, u32)>, &'static str>> {{\n\
         \x20   let _ = p;\n\
         \x20   let mut out: Vec<(&'static str, u32)> = Vec::new();\n\
         \x20   match name {{\n{arms}\
         \x20       _ => return None,\n\
         \x20   }}\n\
         \x20   Some(Ok(out))\n\
         }}\n"
    );
    std::fs::write(out, text)?;
    println!("loot-gen: {kept} tables of buildable blocks ({random} random or not covered) -> {}", out.display());
    Ok(())
}

/// A table's predicate references (`"condition": "minecraft:tool/can_shear"`: a file under
/// `data/minecraft/predicate/`) replaced by the predicates themselves, recursively.
fn inline_predicates(v: &mut Value, data: &Path) -> Result<()> {
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                if (k == "condition" || k == "conditions" || k == "term" || k == "terms") && x.is_string() {
                    *x = predicate(x.as_str().unwrap_or(""), data)?;
                } else if (k == "condition" || k == "conditions" || k == "terms")
                    && let Value::Array(a) = x
                {
                    for t in a.iter_mut() {
                        if t.is_string() {
                            *t = predicate(t.as_str().unwrap_or(""), data)?;
                        }
                    }
                }
                inline_predicates(x, data)?;
            }
        }
        Value::Array(a) => {
            for x in a {
                inline_predicates(x, data)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// The predicate file a reference names (`minecraft:tool/can_shear`), its own references inlined.
fn predicate(id: &str, data: &Path) -> Result<Value> {
    let path = data.join("predicate").join(format!("{}.json", strip(id)));
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).with_context(|| format!("predicate {id}: {}", path.display()))?)?;
    inline_predicates(&mut v, data)?;
    Ok(v)
}

/// `redsim-fuzz natural-loot-gen`: the loot tables of the natural blocks that can break by
/// themselves (written `crates/redsim/src/terrain/loot_tables.rs`, generated: never edit it):
/// every block id of the natural-state table (`blocks.tsv`) with a state that pops off when a
/// neighbour goes (its `support` column) or that does not block motion (water washes it away). The
/// tables are kept as the game's JSON (predicate references inlined) for redsim's evaluator
/// (`redsim::terrain::loot`), which draws from each table's random sequence as the game does; a
/// block id without a table file is listed with an empty table (it drops nothing).
pub fn run_natural(data: &Path, table: &Path, out: &Path) -> Result<()> {
    let text = std::fs::read_to_string(table).with_context(|| format!("reading {}", table.display()))?;
    let mut cols: Vec<&str> = Vec::new();
    let mut ids: std::collections::BTreeSet<String> = Default::default();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.first() == Some(&"name") {
            cols = f;
            continue;
        }
        let col = |n: &str| cols.iter().position(|c| *c == n).and_then(|i| f.get(i).copied()).unwrap_or("");
        let support = col("support");
        let blocks_motion = col("tags").split(',').any(|t| t == "minecraft:blocks_motion");
        if (!support.is_empty() && support != "000000") || !blocks_motion {
            let id = f[0].split('[').next().unwrap_or(f[0]);
            ids.insert(strip(id).to_string());
        }
    }
    let mut rows = String::new();
    let mut n = 0;
    for id in &ids {
        if id == "air" || id == "cave_air" || id == "void_air" {
            continue;
        }
        let path = data.join("loot_table/blocks").join(format!("{id}.json"));
        // a wall variant shares its floor block's table (`overrideLootTable`: wall torches, wall
        // signs, banners, heads)
        let floor_id = id.strip_prefix("wall_").map(|s| s.to_string()).unwrap_or_else(|| id.replacen("_wall_", "_", 1));
        let floor = data.join("loot_table/blocks").join(format!("{floor_id}.json"));
        let json = match std::fs::read_to_string(&path).or_else(|_| std::fs::read_to_string(&floor)) {
            Ok(t) => {
                let mut v: Value = serde_json::from_str(&t).with_context(|| format!("{}", path.display()))?;
                inline_predicates(&mut v, data)?;
                serde_json::to_string(&v)?
            }
            Err(_) => String::new(),
        };
        let _ = writeln!(rows, "    ({id:?}, r##\"{json}\"##),");
        n += 1;
    }
    let text = format!(
        "//! GENERATED by `redsim-fuzz natural-loot-gen` from the game's block loot tables (do not edit):\n\
         //! the tables of the natural blocks that break by themselves (a plant whose support goes, a\n\
         //! block water washes away), as the game's JSON with predicate references inlined, for\n\
         //! `terrain::loot`. {n} block ids; an empty table drops nothing.\n\n\
         /// (block id without `minecraft:`, its loot table's JSON)\n\
         pub const TABLES: &[(&str, &str)] = &[\n{rows}];\n"
    );
    std::fs::write(out, text)?;
    println!("natural-loot-gen: {n} tables -> {}", out.display());
    Ok(())
}
