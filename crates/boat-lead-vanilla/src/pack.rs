use anyhow::{Result, bail};
use serde_json::Value;
use std::path::Path;

pub fn pack(root: &Path) -> Result<()> {
    let output = Path::new("designs/boat-lead-vanilla/evidence");
    for (source, name) in [
        ("seating-matrix-04", "seating"),
        ("authority-01", "authority"),
        ("pair-raft-ordinary-02", "pair-raft-ordinary"),
        ("pair-raft-mixed-01", "pair-raft-mixed"),
        ("normal-dock-mixed-02", "normal-dock-mixed"),
    ] {
        let dir = root.join(source);
        crate::checks::checks(&dir)?;
        if name != "seating" && name != "authority" {
            crate::report::report(&dir)?;
            crate::verify::verify(&dir.join("trace.jsonl"))?;
        }
        crate::bank::bank(&dir, &output.join(name))?;
    }
    for name in [
        "pair-raft-no-lead",
        "pair-oak-ordinary",
        "pair-oak-mixed",
        "pair-raft-mixed-no-lead",
    ] {
        let dir = root.join("controls-01").join(name);
        crate::checks::checks(&dir)?;
        crate::report::report(&dir)?;
        crate::verify::verify(&dir.join("trace.jsonl"))?;
        crate::bank::bank(&dir, &output.join(name))?;
    }
    for name in ["opposite-yaw", "opposite-yaw-no-lead"] {
        let dir = root.join("orientation-01").join(name);
        crate::checks::checks(&dir)?;
        crate::report::report(&dir)?;
        crate::verify::verify(&dir.join("trace.jsonl"))?;
        crate::bank::bank(&dir, &output.join(name))?;
    }
    let mut report = String::from(
        "# Fixed pair-control results\n\nCounts below include the terminal removal tick. Zero later-window samples is failed persistence, not successful 1200-tick flight. All are labeled mechanics fixtures, not survival assembly claims.\n\n",
    );
    for name in [
        "pair-raft-ordinary",
        "pair-raft-mixed",
        "normal-dock-mixed",
        "pair-raft-no-lead",
        "pair-oak-ordinary",
        "pair-oak-mixed",
        "pair-raft-mixed-no-lead",
        "opposite-yaw",
        "opposite-yaw-no-lead",
    ] {
        let dir = output.join(name);
        let v: Value = serde_json::from_slice(&std::fs::read(dir.join("report.json"))?)?;
        report += &format!(
            "## {name}\n\nServer continued {} ticks after release.\n\n",
            v["elapsed_ticks"]
        );
        for boat in v["boats"].as_array().unwrap() {
            report += &format!(
                "- Entity {}: {} post-release samples, last y {}; active spring {} ticks (undamped {}); first missing lead tick {}, removed tick {}; supporting-block samples {}; unloaded samples {}; tick gaps {}.\n",
                boat["id"],
                boat["flight_ticks_observed"],
                boat["last_y"],
                boat["active_spring_samples"],
                boat["undamped_samples"],
                boat["first_missing_lead_tick"],
                boat["first_removed_tick"],
                boat["supporting_block_samples"],
                boat["unloaded_samples"],
                boat["tick_gaps"]
            );
            if boat["flight_ticks_observed"].as_u64().unwrap() >= 1200 {
                bail!(
                    "unexpected surviving candidate: inspect full trace before publishing fixed-negative report"
                );
            }
        }
        report += &format!(
            "\nRaw phase trace, force budget, interventions, scenario, server/client logs and hashes: `evidence/{name}/`.\n\n"
        );
    }
    std::fs::write("designs/boat-lead-vanilla/CONTROL-RESULTS.md", report)?;
    Ok(())
}
