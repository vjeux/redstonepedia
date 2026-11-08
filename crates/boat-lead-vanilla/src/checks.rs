use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::path::Path;

/// Verify real intermediate phases and provenance from a completed run, never predicted motion.
pub fn checks(dir: &Path) -> Result<()> {
    let rows = crate::trace::read(&dir.join("trace.jsonl"))?;
    let mut phases = std::collections::BTreeSet::new();
    for r in &rows {
        phases.insert(r["phase"].as_str().unwrap());
        let e = &r["entity"];
        if e["no_gravity"] == true
            || e["no_physics"] == true
            || e["no_ai"] == true
            || e["touching_unloaded_chunk"] == true
        {
            bail!("nonordinary or unloaded sample seq {}", r["seq"]);
        }
    }
    for required in [
        "boat_pre",
        "boat_post",
        "leash_pre",
        "leash_post",
        "float_pre",
        "float_post",
        "ride_pre",
        "ride_after_tick",
        "ride_post",
    ] {
        if !phases.contains(required) {
            bail!("missing phase {required}");
        }
    }
    let props = std::fs::read_to_string(dir.join("server.properties"))?;
    if !props.lines().any(|s| s == "allow-flight=false")
        || !props.lines().any(|s| s == "gamemode=survival")
    {
        bail!("incorrect server profile");
    }
    let log = std::fs::read_to_string(dir.join("server.log"))?;
    if log.contains("BOAT_PROBE_ERROR") || log.contains("Ticking entity") {
        bail!("instrumentation failure");
    }
    let summary: Value = json!({"ordinary_flags_and_loaded_chunks":true,"phases":phases,"samples":rows.len(),"allow_flight":false,"gamemode":"survival"});
    std::fs::write(
        dir.join("checks.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    println!(
        "PASS ordinary flags, loaded chunks, all nine phases, survival/no-flight profile: {} records",
        rows.len()
    );
    Ok(())
}
