use anyhow::{Context, Result, bail};
use mctas::{fabric::FabricInstall, video::ClientBuild};
use std::{path::{Path, PathBuf}, process::Command};

pub fn loader() -> Result<FabricInstall> {
    // Pinned cache from the established actor lane. Never query third-party designs or versions.
    let path = mctas::paths::version_dir("26.3").join("fabric/resolved-loader.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    let strs = |key: &str| -> Vec<String> {
        v[key].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)).collect()
    };
    let libs: Vec<PathBuf> = strs("libs").into_iter().map(PathBuf::from).collect();
    if libs.is_empty() || libs.iter().any(|p| !p.exists()) { bail!("incomplete pinned Fabric cache"); }
    Ok(FabricInstall {
        loader: v["loader"].as_str().context("loader")?.into(),
        main_class: v["main_class"].as_str().context("main_class")?.into(),
        jvm_args: strs("jvm_args"), libs, mods: vec![],
    })
}

pub fn actor(client: &ClientBuild, loader: &FabricInstall, dir: &Path) -> Result<PathBuf> {
    let sources = [
        ("Actor.java", include_str!("../../../bridge/actor/dev/vjeux/mctas/actor/Actor.java")),
        ("ActorMod.java", include_str!("../../../bridge/actor/dev/vjeux/mctas/actor/ActorMod.java")),
    ];
    let mut cp = vec![client.install.client_jar.clone()];
    cp.extend(client.install.libs.clone());
    cp.extend(loader.libs.clone());
    let classes = compile(&client.install.java, &cp, &sources, &dir.join("actor"))?;
    std::fs::write(classes.join("fabric.mod.json"), include_str!("../../../bridge/actor/fabric.mod.json"))?;
    jar(&client.install.java, &classes, &dir.join("actor.jar"), None)
}

pub fn observer(client: &ClientBuild, loader: &FabricInstall, dir: &Path) -> Result<PathBuf> {
    let sources = [
        ("PhaseAgent.java", include_str!("../instrumentation/PhaseAgent.java")),
        ("PhaseObserver.java", include_str!("../instrumentation/PhaseObserver.java")),
    ];
    let classes = compile(&client.install.java, &loader.libs, &sources, &dir.join("observer"))?;
    let asm = loader.libs.iter().find(|p| p.file_name().unwrap().to_string_lossy().starts_with("asm-9")).context("ASM jar")?;
    let helper = dir.join("observer-helper.jar");
    let helper_out = Command::new(mctas::setup::javac_for(&client.install.java).with_file_name("jar"))
        .args(["--create", "--file"]).arg(&helper).arg("-C").arg(&classes)
        .arg("dev/vjeux/mctas/boatprobe/PhaseObserver.class").output()?;
    if !helper_out.status.success() { bail!("observer helper jar: {}", String::from_utf8_lossy(&helper_out.stderr)); }
    let manifest = dir.join("observer.mf");
    std::fs::write(&manifest, format!("Manifest-Version: 1.0\nPremain-Class: dev.vjeux.mctas.boatprobe.PhaseAgent\nBoot-Class-Path: observer-helper.jar\nClass-Path: {}\n\n", asm.display()))?;
    jar(&client.install.java, &classes, &dir.join("observer.jar"), Some(&manifest))
}

fn compile(java: &Path, cp: &[PathBuf], sources: &[(&str, &str)], dir: &Path) -> Result<PathBuf> {
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes)?;
    let mut paths = vec![];
    for (name, text) in sources {
        let path = dir.join(name);
        std::fs::write(&path, text)?;
        paths.push(path);
    }
    let cp = cp.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(":");
    let out = Command::new(mctas::setup::javac_for(java))
        .args(["--release", "25", "-nowarn", "-cp", &cp, "-d"])
        .arg(&classes).args(paths).output()?;
    if !out.status.success() { bail!("javac: {}", String::from_utf8_lossy(&out.stderr)); }
    Ok(classes)
}

fn jar(java: &Path, classes: &Path, dest: &Path, manifest: Option<&Path>) -> Result<PathBuf> {
    let mut command = Command::new(mctas::setup::javac_for(java).with_file_name("jar"));
    command.args(["--create", "--file"]).arg(dest);
    if let Some(path) = manifest { command.arg("--manifest").arg(path); }
    let out = command.arg("-C").arg(classes).arg(".").output()?;
    if !out.status.success() { bail!("jar: {}", String::from_utf8_lossy(&out.stderr)); }
    Ok(dest.into())
}
