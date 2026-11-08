use anyhow::{Context, Result};
use std::path::Path;

/// Offline verification replays the diagnostics over kept evidence, not Minecraft ticks.
pub fn regress(dir: &Path) -> Result<()> {
    for name in [
        "seating",
        "authority",
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
        let source = dir.join(name).join("trace.jsonl.gz");
        let out = std::process::Command::new("gzip")
            .args(["-d", "-c"])
            .arg(&source)
            .output()
            .context("gzip")?;
        anyhow::ensure!(out.status.success(), "cannot read {}", source.display());
        let temp = std::env::temp_dir().join(format!(
            "mctas-boat-regress-{}-{name}.jsonl",
            std::process::id()
        ));
        std::fs::write(&temp, &out.stdout)?;
        let rows = crate::trace::read(&temp)?;
        anyhow::ensure!(!rows.is_empty());
        if name != "seating" && name != "authority" {
            crate::verify::verify(&temp)?;
        }
        let budget = temp.with_extension("budget.jsonl");
        crate::budget::budget(&temp, &budget)?;
        std::fs::remove_file(temp)?;
        std::fs::remove_file(budget)?;
        println!("PASS kept evidence {name}");
    }
    Ok(())
}
