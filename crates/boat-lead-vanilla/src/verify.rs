use anyhow::{Context, Result, bail};
use std::{collections::BTreeMap, path::Path};

/// Verifies the read-only observer's sampled phases against the official operations.
pub fn verify(path: &Path) -> Result<()> {
    let rows = crate::trace::read(path)?;
    let mut floats = BTreeMap::new();
    let mut seen = 0;
    let mut maxerr = 0.0_f64;
    for r in &rows {
        let id = r["entity"]["id"].as_i64().context("id")?;
        if r["phase"] == "float_pre" {
            floats.insert(id, r);
        }
        if r["phase"] != "float_post" {
            continue;
        }
        let pre = floats.remove(&id).context("unpaired gravity phase")?;
        if pre["entity"]["on_ground"] == true
            || pre["entity"]["in_water"] == true
            || pre["entity"]["in_lava"] == true
        {
            continue;
        }
        let a = pre["entity"]["delta"][1].as_f64().unwrap();
        let b = r["entity"]["delta"][1].as_f64().unwrap();
        maxerr = maxerr.max(((a - 0.04) - b).abs());
        seen += 1;
    }
    if seen == 0 {
        bail!("no dry-air gravity positive control samples");
    }
    if maxerr > 1e-12 {
        bail!("gravity mismatch {maxerr}");
    }
    println!("PASS dry-air boat gravity: {seen} samples, max error {maxerr}");
    Ok(())
}
