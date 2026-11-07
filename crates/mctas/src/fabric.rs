//! The Fabric client with Litematica (`mctas litematic verify-ingame`): Fabric Loader for the
//! version (its launcher profile from meta.fabricmc.net: `KnotClient`, ASM, Mixin, the loader),
//! MaLiLib and Litematica from Modrinth (the newest Fabric release for the game version), and the
//! repository's probe mod (`bridge/litematica-probe`: pastes .litematic files with Litematica's
//! own code), compiled against them; launched like [`crate::video::Client`] with the mods in the
//! game directory's `mods/`.

use crate::net::{self, Checksum};
use crate::paths;
use crate::video::{Client, ClientBuild, Display};
use anyhow::{Context, Result, anyhow, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

const PROBE_SRC: &str = include_str!("../../../bridge/litematica-probe/dev/vjeux/mctas/probe/LitematicaProbe.java");
const PROBE_MOD_JSON: &str = include_str!("../../../bridge/litematica-probe/fabric.mod.json");
const ACTOR_SRC: &str = include_str!("../../../bridge/actor/dev/vjeux/mctas/actor/Actor.java");
const ACTOR_MOD_SRC: &str = include_str!("../../../bridge/actor/dev/vjeux/mctas/actor/ActorMod.java");
const ACTOR_MOD_JSON: &str = include_str!("../../../bridge/actor/fabric.mod.json");

/// What the Fabric client needs besides the vanilla client.
#[derive(Clone, Debug)]
pub struct FabricInstall {
    /// Fabric Loader's version.
    pub loader: String,
    pub main_class: String,
    pub jvm_args: Vec<String>,
    /// The loader's libraries (put before the client's on the classpath).
    pub libs: Vec<PathBuf>,
    /// MaLiLib, Litematica and the probe mod.
    pub mods: Vec<PathBuf>,
}

/// `group:artifact:version` -> `group/path/artifact/version/artifact-version.jar`.
fn maven_path(coord: &str) -> Result<String> {
    let p: Vec<&str> = coord.split(':').collect();
    if p.len() < 3 {
        bail!("bad maven coordinate {coord:?}");
    }
    Ok(format!("{}/{}/{}/{}-{}.jar", p[0].replace('.', "/"), p[1], p[2], p[1], p[2]))
}

/// The newest Fabric release of a Modrinth project for this game version: (url, file name, sha1).
fn modrinth(project: &str, version: &str) -> Result<(String, String, String)> {
    let url = format!("https://api.modrinth.com/v2/project/{project}/version?game_versions=%5B%22{version}%22%5D&loaders=%5B%22fabric%22%5D");
    let v = net::get_json(&url)?;
    let first = v.as_array().and_then(|a| a.first()).ok_or_else(|| anyhow!("no {project} release for Minecraft {version} (Fabric) on Modrinth"))?;
    let files = first["files"].as_array().ok_or_else(|| anyhow!("{project}: no files"))?;
    let f = files.iter().find(|x| x["primary"].as_bool() == Some(true)).or(files.first()).ok_or_else(|| anyhow!("{project}: no files"))?;
    let s = |k: &serde_json::Value| k.as_str().map(str::to_string).ok_or_else(|| anyhow!("{project}: incomplete file entry"));
    Ok((s(&f["url"])?, s(&f["filename"])?, s(&f["hashes"]["sha1"])?))
}

/// Fabric Loader, MaLiLib and Litematica for `version` (downloaded once; the resolved choice is
/// kept in `fabric/resolved.json` and reused offline afterwards: vjeux 2026-10-20, nothing is
/// fetched again once it is there) and the probe mod.
pub fn install(version: &str, client: &ClientBuild) -> Result<FabricInstall> {
    let dir = paths::version_dir(version).join("fabric");
    let resolved = dir.join("resolved.json");
    if let Ok(text) = std::fs::read_to_string(&resolved)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let strs = |k: &str| -> Vec<String> { v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
        let libs: Vec<PathBuf> = strs("libs").into_iter().map(PathBuf::from).collect();
        let mut mods: Vec<PathBuf> = strs("mods").into_iter().map(PathBuf::from).collect();
        if libs.iter().chain(&mods).all(|p| p.exists()) && !mods.is_empty() {
            let probe = build_probe(version, &dir, client, &libs, &mods)?;
            mods.push(probe);
            return Ok(FabricInstall {
                loader: v["loader"].as_str().unwrap_or_default().to_string(),
                main_class: v["main_class"].as_str().unwrap_or_default().to_string(),
                jvm_args: strs("jvm_args"),
                libs,
                mods,
            });
        }
    }
    let loaders = net::get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{version}")).context("Fabric's version list")?;
    let loader = loaders
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l["loader"]["version"].as_str())
        .ok_or_else(|| anyhow!("no Fabric Loader for Minecraft {version}"))?
        .to_string();
    let profile = net::get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{version}/{loader}/profile/json")).context("Fabric's launcher profile")?;
    let main_class = profile["mainClass"].as_str().ok_or_else(|| anyhow!("Fabric profile: no mainClass"))?.to_string();
    let jvm_args: Vec<String> = profile["arguments"]["jvm"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect();
    let mut libs = Vec::new();
    for lib in profile["libraries"].as_array().into_iter().flatten() {
        let name = lib["name"].as_str().ok_or_else(|| anyhow!("Fabric profile: a library without a name"))?;
        let base = lib["url"].as_str().unwrap_or("https://maven.fabricmc.net/");
        let rel = maven_path(name)?;
        let dest = dir.join("libs").join(&rel);
        let sha1 = lib["sha1"].as_str().unwrap_or("");
        net::download(&format!("{base}{rel}"), &dest, if sha1.is_empty() { Checksum::None } else { Checksum::Sha1(sha1) })?;
        libs.push(dest);
    }
    let mut mods = Vec::new();
    for project in ["malilib", "litematica"] {
        let (url, name, sha1) = modrinth(project, version)?;
        let dest = dir.join("mods").join(&name);
        net::download(&url, &dest, Checksum::Sha1(&sha1))?;
        mods.push(dest);
    }
    let paths_json = |v: &[PathBuf]| serde_json::Value::Array(v.iter().map(|p| serde_json::Value::String(p.display().to_string())).collect());
    let record = serde_json::json!({
        "loader": loader,
        "main_class": main_class,
        "jvm_args": jvm_args,
        "libs": paths_json(&libs),
        "mods": paths_json(&mods),
    });
    std::fs::write(&resolved, serde_json::to_string_pretty(&record)?)?;
    let probe = build_probe(version, &dir, client, &libs, &mods)?;
    mods.push(probe);
    Ok(FabricInstall { loader, main_class, jvm_args, libs, mods })
}

/// Compile the probe mod against the client, the loader and the mods; jar it with its
/// `fabric.mod.json`.
fn build_probe(version: &str, dir: &Path, client: &ClientBuild, libs: &[PathBuf], mods: &[PathBuf]) -> Result<PathBuf> {
    let names: Vec<String> = mods.iter().map(|m| m.file_name().unwrap_or_default().to_string_lossy().to_string()).collect();
    let hash = &net::sha256_hex(format!("{version}{PROBE_SRC}{PROBE_MOD_JSON}{}", names.join(",")).as_bytes())[..12];
    let out = dir.join(format!("probe-{hash}"));
    let jar = out.join("mctas-litematica-probe.jar");
    if jar.exists() {
        return Ok(jar);
    }
    let src = out.join("src/dev/vjeux/mctas/probe/LitematicaProbe.java");
    std::fs::create_dir_all(src.parent().unwrap())?;
    std::fs::write(&src, PROBE_SRC)?;
    let classes = out.join("classes");
    std::fs::create_dir_all(&classes)?;
    std::fs::write(classes.join("fabric.mod.json"), PROBE_MOD_JSON)?;
    let mut cp = vec![client.install.client_jar.display().to_string()];
    cp.extend(client.install.libs.iter().map(|p| p.display().to_string()));
    cp.extend(libs.iter().map(|p| p.display().to_string()));
    cp.extend(mods.iter().map(|p| p.display().to_string()));
    let major = crate::setup::required_java(version)?;
    let javac = crate::setup::javac_for(&client.install.java);
    let o = Command::new(&javac).args(["--release", &major.to_string(), "-nowarn", "-cp", &cp.join(":"), "-d"]).arg(&classes).arg(&src).output()?;
    if !o.status.success() {
        bail!("javac (Litematica probe) failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    let jar_tool = javac.with_file_name("jar");
    let tmp = out.join("probe.jar.part");
    let o = Command::new(&jar_tool).arg("cf").arg(&tmp).arg("-C").arg(&classes).arg(".").output()?;
    if !o.status.success() {
        bail!("jar (Litematica probe) failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    std::fs::rename(&tmp, &jar)?;
    Ok(jar)
}

/// Fabric Loader alone (no MaLiLib/Litematica/probe) for `version`, downloaded once and kept in
/// `fabric/resolved-loader.json`; the loader's libraries are shared with [`install`]'s.
pub fn install_loader(version: &str) -> Result<FabricInstall> {
    let dir = paths::version_dir(version).join("fabric");
    let resolved = dir.join("resolved-loader.json");
    if let Ok(text) = std::fs::read_to_string(&resolved)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
    {
        let strs = |k: &str| -> Vec<String> { v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
        let libs: Vec<PathBuf> = strs("libs").into_iter().map(PathBuf::from).collect();
        if !libs.is_empty() && libs.iter().all(|p| p.exists()) {
            return Ok(FabricInstall {
                loader: v["loader"].as_str().unwrap_or_default().to_string(),
                main_class: v["main_class"].as_str().unwrap_or_default().to_string(),
                jvm_args: strs("jvm_args"),
                libs,
                mods: Vec::new(),
            });
        }
    }
    let loaders = net::get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{version}")).context("Fabric's version list")?;
    let loader = loaders
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l["loader"]["version"].as_str())
        .ok_or_else(|| anyhow!("no Fabric Loader for Minecraft {version}"))?
        .to_string();
    let profile = net::get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{version}/{loader}/profile/json")).context("Fabric's launcher profile")?;
    let main_class = profile["mainClass"].as_str().ok_or_else(|| anyhow!("Fabric profile: no mainClass"))?.to_string();
    let jvm_args: Vec<String> = profile["arguments"]["jvm"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect();
    let mut libs = Vec::new();
    for lib in profile["libraries"].as_array().into_iter().flatten() {
        let name = lib["name"].as_str().ok_or_else(|| anyhow!("Fabric profile: a library without a name"))?;
        let base = lib["url"].as_str().unwrap_or("https://maven.fabricmc.net/");
        let rel = maven_path(name)?;
        let dest = dir.join("libs").join(&rel);
        let sha1 = lib["sha1"].as_str().unwrap_or("");
        net::download(&format!("{base}{rel}"), &dest, if sha1.is_empty() { Checksum::None } else { Checksum::Sha1(sha1) })?;
        libs.push(dest);
    }
    let paths_json = |v: &[PathBuf]| serde_json::Value::Array(v.iter().map(|p| serde_json::Value::String(p.display().to_string())).collect());
    let record = serde_json::json!({ "loader": loader, "main_class": main_class, "jvm_args": jvm_args, "libs": paths_json(&libs) });
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&resolved, serde_json::to_string_pretty(&record)?)?;
    Ok(FabricInstall { loader, main_class, jvm_args, libs, mods: Vec::new() })
}

/// The actor mod (`bridge/actor`: the scripted survival player of `mctas bedrock video`) as a
/// jar, compiled against the client and the loader. Launch with [`launch`] on a
/// [`FabricInstall`] of [`install_loader`] whose `mods` is this jar alone.
pub fn build_actor(version: &str, client: &ClientBuild, libs: &[PathBuf]) -> Result<PathBuf> {
    let dir = paths::version_dir(version).join("fabric");
    let hash = &net::sha256_hex(format!("{version}{ACTOR_SRC}{ACTOR_MOD_SRC}{ACTOR_MOD_JSON}").as_bytes())[..12];
    let out = dir.join(format!("actor-{hash}"));
    let jar = out.join("mctas-actor.jar");
    if jar.exists() {
        return Ok(jar);
    }
    let mut sources = Vec::new();
    for (name, text) in [("Actor.java", ACTOR_SRC), ("ActorMod.java", ACTOR_MOD_SRC)] {
        let src = out.join("src/dev/vjeux/mctas/actor").join(name);
        std::fs::create_dir_all(src.parent().unwrap())?;
        std::fs::write(&src, text)?;
        sources.push(src);
    }
    let classes = out.join("classes");
    std::fs::create_dir_all(&classes)?;
    std::fs::write(classes.join("fabric.mod.json"), ACTOR_MOD_JSON)?;
    let mut cp = vec![client.install.client_jar.display().to_string()];
    cp.extend(client.install.libs.iter().map(|p| p.display().to_string()));
    cp.extend(libs.iter().map(|p| p.display().to_string()));
    let major = crate::setup::required_java(version)?;
    let javac = crate::setup::javac_for(&client.install.java);
    let mut cmd = Command::new(&javac);
    cmd.args(["--release", &major.to_string(), "-nowarn", "-cp", &cp.join(":"), "-d"]).arg(&classes);
    for s in &sources {
        cmd.arg(s);
    }
    let o = cmd.output()?;
    if !o.status.success() {
        bail!("javac (actor mod) failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    let jar_tool = javac.with_file_name("jar");
    let tmp = out.join("actor.jar.part");
    let o = Command::new(&jar_tool).arg("cf").arg(&tmp).arg("-C").arg(&classes).arg(".").output()?;
    if !o.status.success() {
        bail!("jar (actor mod) failed: {}", String::from_utf8_lossy(&o.stderr));
    }
    std::fs::rename(&tmp, &jar)?;
    Ok(jar)
}

/// Launch the Fabric client with the mods, joining `localhost:port` (no port: it stays on its
/// title screen; the game directory gets a fresh `mods/`).
pub fn launch(build: &ClientBuild, fabric: &FabricInstall, display: &Display, port: Option<u16>, width: u32, height: u32, game_dir: &Path) -> Result<Client> {
    let mods = game_dir.join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(&mods)?;
    for m in &fabric.mods {
        std::fs::copy(m, mods.join(m.file_name().unwrap_or_default()))?;
    }
    Client::launch_main(build, display, port, width, height, game_dir, &fabric.main_class, &fabric.jvm_args, &fabric.libs)
}
