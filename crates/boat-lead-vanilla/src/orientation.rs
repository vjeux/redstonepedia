use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

pub fn generate(dir: &Path) -> Result<()> {
    // One coordinator-approved orientation contrast only; no angle parameter.
    for (source, name) in [
        ("pair-raft-ordinary", "opposite-yaw"),
        ("pair-raft-no-lead", "opposite-yaw-no-lead"),
    ] {
        let mut scenario: Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{source}.json")))?)?;
        scenario["name"] = json!(name);
        for step in scenario["steps"].as_array_mut().context("steps")? {
            if let Some(command) = step.get("rcon").and_then(Value::as_str) {
                if command.starts_with("summon bamboo_raft 10.5 ") {
                    step["rcon"] = json!(
                        "summon bamboo_raft 10.5 201 0.5 {Rotation:[180.0f,0.0f],Tags:[\"mctas_probe\",\"boat_b\"]}"
                    );
                }
            }
            if step
                .get("note")
                .and_then(Value::as_str)
                .is_some_and(|s| s.starts_with("LAUNCH"))
            {
                step["note"] = json!(
                    "LAUNCH: fixed orientation contrast A yaw0, B yaw180. Command-arranged fixture, real attachments, no impulses; hypothesis opposite transverse components/same-sign torque. No other angles authorized."
                );
            }
        }
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string_pretty(&scenario)?,
        )?;
    }
    Ok(())
}
