use anyhow::{Result, bail};
use std::{io::Write, path::Path, process::Command};

/// Bank only explicitly named evidence files (no game jars, caches, worlds, or directory scans).
pub fn bank(run: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for required in [
        "scenario.json",
        "interventions.jsonl",
        "trace.jsonl",
        "server.log",
        "client.log",
    ] {
        if !run.join(required).exists() {
            bail!("missing required evidence {required}");
        }
    }
    let props = std::fs::read_to_string(run.join("server.properties"))?;
    let public_props = props
        .lines()
        .filter(|line| !line.starts_with("rcon.password="))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dest.join("server.properties"), public_props)?;
    for name in [
        "scenario.json",
        "run.json",
        "provenance.json",
        "interventions.jsonl",
        "report.json",
        "checks.json",
        "server.log",
        "client.log",
    ] {
        let path = run.join(name);
        if path.exists() {
            std::fs::copy(path, dest.join(name))?;
        }
    }
    for name in ["trace.jsonl", "budget.jsonl"] {
        let path = run.join(name);
        if !path.exists() {
            continue;
        }
        let output = Command::new("gzip")
            .args(["-n", "-9", "-c"])
            .arg(&path)
            .output()?;
        if !output.status.success() {
            bail!("gzip {name} failed");
        }
        let zipped = dest.join(format!("{name}.gz"));
        std::fs::write(&zipped, &output.stdout)?;
        let restored = Command::new("gzip")
            .args(["-d", "-c"])
            .arg(&zipped)
            .output()?;
        if !restored.status.success() || restored.stdout != std::fs::read(path)? {
            bail!("gzip roundtrip {name}");
        }
    }
    let mut manifest = std::fs::File::create(dest.join("SHA256SUMS"))?;
    for name in [
        "scenario.json",
        "run.json",
        "provenance.json",
        "interventions.jsonl",
        "report.json",
        "checks.json",
        "trace.jsonl.gz",
        "budget.jsonl.gz",
        "server.properties",
        "server.log",
        "client.log",
    ] {
        let path = dest.join(name);
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        writeln!(manifest, "{}  {}", mctas::net::sha256_hex(&bytes), name)?;
    }
    println!(
        "Banked evidence {} -> {} (gzip roundtrip verified)",
        run.display(),
        dest.display()
    );
    Ok(())
}
