use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader},
    path::Path,
};

pub fn analyze(path: &Path) -> Result<()> {
    let rows = read(path)?;
    let mut phases: BTreeMap<String, usize> = BTreeMap::new();
    let mut entities: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for row in &rows {
        *phases
            .entry(row["phase"].as_str().context("phase")?.into())
            .or_default() += 1;
        entities
            .entry(row["entity"]["id"].as_i64().context("id")?)
            .or_default()
            .push(row);
    }
    println!("{}", json!({"rows": rows.len(), "phases": phases}));
    for (id, entries) in entities {
        let ticks: BTreeSet<_> = entries.iter().filter_map(|r| r["tick"].as_i64()).collect();
        let post: Vec<_> = entries
            .iter()
            .filter(|r| r["phase"] == "boat_post" || r["phase"] == "ride_post")
            .collect();
        let known_y: BTreeSet<_> = post
            .iter()
            .filter_map(|r| r["entity"]["known"][1].as_f64())
            .map(f64::to_bits)
            .collect();
        println!(
            "{}",
            json!({"id":id,"type":entries[0]["entity"]["type"], "tags":entries[0]["entity"]["tags"],
            "ticks":ticks.len(), "first_tick":ticks.first(), "last_tick":ticks.last(),
            "first":entries.first(), "last":entries.last(),
            "post_known_y":known_y.iter().take(20).map(|x|format!("{:.17} (0x{x:016x})",f64::from_bits(*x))).collect::<Vec<_>>() })
        );
    }
    Ok(())
}

pub fn read(path: &Path) -> Result<Vec<Value>> {
    let mut rows = vec![];
    for (line, text) in BufReader::new(std::fs::File::open(path)?)
        .lines()
        .enumerate()
    {
        let row: Value =
            serde_json::from_str(&text?).with_context(|| format!("trace line {}", line + 1))?;
        if row.get("error").is_some() {
            bail!("observer failed: {row}");
        }
        rows.push(row);
    }
    if rows.is_empty() {
        bail!("empty evidence trace");
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_source_residual() {
        let y = -0.08_f64 * (0.98_f32 as f64);
        assert_eq!(y.to_bits(), (-0.0784000015258789_f64).to_bits());
    }
}
