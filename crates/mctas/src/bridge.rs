//! Building the Java side of the simulator.
//!
//! The bridge (`bridge/` in the repository) is a few Java classes compiled against the
//! unobfuscated server jar with plain `javac`: no mod loader, no Gradle, no remapping. The
//! sources are embedded in this crate, so the `mctas` binary can rebuild them anywhere.

use crate::net;
use crate::paths;
use crate::setup::{self, ServerInstall};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::Command;

macro_rules! bridge_source {
    ($name:literal) => {
        (
            concat!("dev/vjeux/mctas/bridge/", $name),
            include_str!(concat!("../../../bridge/src/dev/vjeux/mctas/bridge/", $name)),
        )
    };
}

/// Java sources of the bridge, embedded at build time.
pub const SOURCES: &[(&str, &str)] = &[
    bridge_source!("Main.java"),
    bridge_source!("SimServer.java"),
    bridge_source!("Sandbox.java"),
    bridge_source!("Protocol.java"),
    bridge_source!("BlockInfo.java"),
    bridge_source!("Exact.java"),
    bridge_source!("Terrain.java"),
    bridge_source!("ExactVillageSiege.java"),
    bridge_source!("MobAiProbe.java"),
];

/// Logging configuration: Minecraft's logs go to stderr so stdout stays the protocol channel.
pub const LOG4J_CONFIG: &str = include_str!("../../../bridge/resources/mctas-log4j2.xml");

/// The main class of the bridge.
pub const MAIN_CLASS: &str = "dev.vjeux.mctas.bridge.Main";

/// The mob-farm gate's Java agent (`bridge/agent`): every iron golem a villager summons on the
/// official server is tagged `mctas.by.<the villager's UUID>` (the gate attributes golems by their
/// summoner).
pub const GOLEM_AGENT: (&str, &str) = ("dev/vjeux/mctas/agent/GolemAgent.java", include_str!("../../../bridge/agent/dev/vjeux/mctas/agent/GolemAgent.java"));

/// The golem agent's jar for `version`'s Java (built with the JDK's own `javac` and `jar` next to
/// the server's `java`, cached by a hash of the source): the value of `-javaagent:`.
pub fn golem_agent(version: &str) -> Result<PathBuf> {
    let install = setup::install_server(version)?;
    let (name, text) = GOLEM_AGENT;
    let hash = &net::sha256_hex(format!("{version}{name}{text}").as_bytes())[..12];
    let jar = paths::version_dir(version).join(format!("golem-agent-{hash}.jar"));
    if jar.exists() {
        return Ok(jar);
    }
    let tmp = paths::version_dir(version).join(format!("golem-agent-{hash}.tmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let src = tmp.join("src").join(name);
    std::fs::create_dir_all(src.parent().unwrap())?;
    std::fs::write(&src, text)?;
    let classes = tmp.join("classes");
    std::fs::create_dir_all(&classes)?;
    let javac = setup::javac_for(&install.java);
    let major = setup::required_java(version)?;
    let out = Command::new(&javac).args(["--release", &major.to_string(), "-nowarn", "-encoding", "UTF-8", "-d"]).arg(&classes).arg(&src).output().with_context(|| format!("running {}", javac.display()))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("javac (golem agent) failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    let manifest = tmp.join("MANIFEST.MF");
    std::fs::write(&manifest, "Premain-Class: dev.vjeux.mctas.agent.GolemAgent\n")?;
    let jar_tool = javac.with_file_name("jar");
    let part = tmp.join("agent.jar");
    let out = Command::new(&jar_tool).arg("--create").arg("--file").arg(&part).arg("--manifest").arg(&manifest).arg("-C").arg(&classes).arg(".").output().with_context(|| format!("running {}", jar_tool.display()))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("jar (golem agent) failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    // renamed into place (processes starting together build the same jar)
    if std::fs::rename(&part, &jar).is_err() && !jar.exists() {
        bail!("moving the golem agent into {}", jar.display());
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(jar)
}

/// A compiled bridge for one Minecraft version.
#[derive(Clone, Debug)]
pub struct BridgeBuild {
    pub install: ServerInstall,
    pub classes: PathBuf,
    pub log4j_config: PathBuf,
}

impl BridgeBuild {
    /// `-cp` value: bridge classes, then the game, then its libraries.
    pub fn classpath(&self) -> String {
        let sep = if cfg!(windows) { ";" } else { ":" };
        [
            self.classes.display().to_string(),
            self.install.server_jar.display().to_string(),
            self.install.libs_dir.join("*").display().to_string(),
        ]
        .join(sep)
    }
}

/// Compile the bridge for `version` (cached by a hash of the sources).
pub fn build(version: &str) -> Result<BridgeBuild> {
    let install = setup::install_server(version)?;
    let mut all = String::from(version);
    for (name, text) in SOURCES {
        all.push_str(name);
        all.push_str(text);
    }
    all.push_str(LOG4J_CONFIG);
    let hash = &net::sha256_hex(all.as_bytes())[..12];
    let dir = paths::version_dir(version).join(format!("bridge-{hash}"));
    let classes = dir.join("classes");
    let log4j_config = dir.join("mctas-log4j2.xml");
    let stamp = dir.join(".built");
    if stamp.exists() {
        return Ok(BridgeBuild { install, classes, log4j_config });
    }
    // built in a private folder, then renamed into place: processes starting together after a
    // bridge change (a selftest's JVM checks) used to clear each other's sources mid-build (clk-rt
    // 2026-10-21: "javac: Usage" in judge:verify-c79 right after a bridge change)
    let tmp = paths::version_dir(version).join(format!("bridge-{hash}.tmp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let src = tmp.join("src");
    let mut files = Vec::new();
    for (name, text) in SOURCES {
        let path = src.join(name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, text)?;
        files.push(path);
    }
    std::fs::write(tmp.join("mctas-log4j2.xml"), LOG4J_CONFIG)?;
    let tmp_classes = tmp.join("classes");
    std::fs::create_dir_all(&tmp_classes)?;
    let major = setup::required_java(version)?;
    let javac = setup::javac_for(&install.java);
    let build = BridgeBuild { install, classes: classes.clone(), log4j_config };
    let cp = {
        let sep = if cfg!(windows) { ";" } else { ":" };
        format!(
            "{}{sep}{}",
            build.install.server_jar.display(),
            build.install.libs_dir.join("*").display()
        )
    };
    let out = Command::new(&javac)
        .args(["--release", &major.to_string(), "-nowarn", "-encoding", "UTF-8", "-cp", &cp, "-d"])
        .arg(&tmp_classes)
        .args(&files)
        .output()
        .with_context(|| format!("running {}", javac.display()))?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("javac failed:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
    std::fs::write(tmp.join(".built"), b"ok")?;
    if std::fs::rename(&tmp, &dir).is_err() {
        if stamp.exists() {
            // another process finished first: its build is the same sources
            let _ = std::fs::remove_dir_all(&tmp);
        } else {
            // a stale, unfinished folder from an interrupted build
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::rename(&tmp, &dir).with_context(|| format!("moving the bridge build into {}", dir.display()))?;
        }
    }
    Ok(build)
}
