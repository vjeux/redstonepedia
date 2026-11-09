//! The filtering UI: web/site (a separate cargo workspace, target wasm32-unknown-unknown),
//! compiled with cargo and bound with the wasm-bindgen CLI (`--target web`: an ES module the
//! page imports with one line; no other JavaScript).

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UI_DIR: &str = "web/site";
pub const UI_NAME: &str = "redstonepedia";

/// A tool from PATH, else from ~/.cargo/bin.
fn tool(name: &str) -> PathBuf {
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(name);
            if p.is_file() {
                return p;
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cargo/bin").join(name)
}

/// The wasm-bindgen version web/site's Cargo.lock pins (the CLI must match it exactly).
fn locked_bindgen_version(ui: &Path) -> Option<String> {
    let lock = std::fs::read_to_string(ui.join("Cargo.lock")).ok()?;
    let mut lines = lock.lines();
    while let Some(l) = lines.next() {
        if l.trim() == "name = \"wasm-bindgen\"" {
            return lines.next()?.trim().strip_prefix("version = \"")?.strip_suffix('"').map(String::from);
        }
    }
    None
}

/// The fingerprint of the UI's sources (web/site and crates/site-model: every file under their
/// `src/` and their Cargo files), written next to a built UI as `pkg/SOURCE` so a copied UI
/// (`--pkg`, `--pkg-ref`) can be checked against the checkout.
pub fn ui_source_hash(repo: &Path) -> Result<String> {
    let mut files: Vec<PathBuf> = Vec::new();
    for root in ["web/site", "crates/site-model"] {
        for f in ["Cargo.toml", "Cargo.lock"] {
            let p = repo.join(root).join(f);
            if p.is_file() {
                files.push(p);
            }
        }
        let mut stack = vec![repo.join(root).join("src")];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for f in &files {
        eat(f.strip_prefix(repo).unwrap_or(f).to_string_lossy().as_bytes());
        eat(&[0]);
        eat(&std::fs::read(f)?);
        eat(&[0]);
    }
    Ok(format!("{h:016x}"))
}

/// The fingerprint of the 3D viewer bundle's sources (web/viewer: crates/mcview-web and every
/// crate it reaches through `path =` dependencies, redsim among them: every file of each crate but
/// its `target/`), written next to a built bundle as `SOURCE` by `mctas web bundle`, so the site
/// build can refuse a bundle whose simulator is not the checkout's (the parent 2026-11-08:
/// iron-day1's live view ran an older redsim than its design needed: 0 ticks, no mobs).
pub fn viewer_source_hash(repo: &Path) -> Result<String> {
    let mut crates: Vec<PathBuf> = Vec::new();
    let mut todo = vec![repo.join("crates/mcview-web")];
    while let Some(c) = todo.pop() {
        let c = c.canonicalize().unwrap_or(c);
        if crates.contains(&c) {
            continue;
        }
        let toml = std::fs::read_to_string(c.join("Cargo.toml")).with_context(|| format!("reading {}", c.join("Cargo.toml").display()))?;
        for part in toml.split("path = \"").skip(1) {
            if let Some(p) = part.split('"').next().filter(|p| p.starts_with("..")) {
                todo.push(c.join(p));
            }
        }
        crates.push(c);
    }
    let root = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let mut files: Vec<PathBuf> = Vec::new();
    for c in &crates {
        let mut stack = vec![c.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).with_context(|| format!("reading {}", d.display()))?.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                if p.is_dir() {
                    stack.push(p);
                } else {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for f in &files {
        eat(f.strip_prefix(&root).unwrap_or(f).to_string_lossy().as_bytes());
        eat(&[0]);
        eat(&std::fs::read(f)?);
        eat(&[0]);
    }
    Ok(format!("{h:016x}"))
}

/// A viewer bundle in the site (`out/viewer`) must be built from the checkout's viewer sources
/// ([`viewer_source_hash`]): why not, if it is not.
pub fn stale_viewer(repo: &Path, viewer: &Path, from: &str) -> Result<Option<String>> {
    let want = viewer_source_hash(repo)?;
    Ok(match std::fs::read_to_string(viewer.join("SOURCE")) {
        Ok(s) if s.trim() == want => None,
        Ok(s) => Some(format!("the viewer bundle from {from} was built from other sources ({}) than this checkout's crates/mcview-web, mcview and redsim ({want}): its 3D views would not run this build's designs as the judge does; rebuild it on this commit (`mctas web bundle`)", s.trim())),
        Err(_) => Some(format!("the viewer bundle from {from} has no SOURCE fingerprint (built before the check): rebuild it on this commit (`mctas web bundle`, which writes it)")),
    })
}

/// Build web/site in release mode for wasm32-unknown-unknown and bind it into `out`
/// (redstonepedia.js + redstonepedia_bg.wasm + SOURCE).
pub fn build_ui(repo: &Path, out: &Path) -> Result<()> {
    let ui = repo.join(UI_DIR);
    let cargo = std::env::var_os("CARGO").map(PathBuf::from).filter(|p| p.is_file()).unwrap_or_else(|| tool("cargo"));
    let st = Command::new(&cargo)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path"])
        .arg(ui.join("Cargo.toml"))
        .status()
        .with_context(|| format!("running {}", cargo.display()))?;
    if !st.success() {
        bail!("building {UI_DIR} for wasm32-unknown-unknown failed (needs `rustup target add wasm32-unknown-unknown`)");
    }
    let wasm = ui.join("target/wasm32-unknown-unknown/release").join(format!("{}.wasm", UI_NAME.replace('-', "_")));
    let bindgen = tool("wasm-bindgen");
    let want = locked_bindgen_version(&ui).unwrap_or_default();
    let have = Command::new(&bindgen).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if !have.ends_with(&want) || want.is_empty() {
        bail!("wasm-bindgen CLI {have:?} does not match the crate ({want}): cargo install wasm-bindgen-cli --version {want} --locked");
    }
    std::fs::create_dir_all(out)?;
    let st = Command::new(&bindgen)
        .args(["--target", "web", "--no-typescript", "--out-name", UI_NAME, "--out-dir"])
        .arg(out)
        .arg(&wasm)
        .status()
        .with_context(|| format!("running {}", bindgen.display()))?;
    if !st.success() {
        bail!("wasm-bindgen failed on {}", wasm.display());
    }
    let size = std::fs::metadata(out.join(format!("{UI_NAME}_bg.wasm"))).map(|m| m.len()).unwrap_or(0);
    std::fs::write(out.join("SOURCE"), format!("{}\n", ui_source_hash(repo)?))?;
    eprintln!("UI: {} ({} KB of WebAssembly)", out.display(), size / 1024);
    Ok(())
}

/// A copied UI (`--pkg`, `--pkg-ref`) must come from the checkout's UI sources: fail when its
/// `SOURCE` names other sources (rebuild it), warn when it has none (built before the check).
pub fn check_copied_ui(repo: &Path, pkg: &Path, from: &str) -> Result<()> {
    let want = ui_source_hash(repo)?;
    match std::fs::read_to_string(pkg.join("SOURCE")) {
        Ok(s) if s.trim() == want => Ok(()),
        Ok(s) => bail!(
            "the UI from {from} was built from other sources ({}) than this checkout's web/site + crates/site-model ({want}): rebuild it (drop --pkg/--pkg-ref; needs `rustup target add wasm32-unknown-unknown` and wasm-bindgen-cli)",
            s.trim()
        ),
        Err(_) => bail!(
            "the UI from {from} has no SOURCE fingerprint (built before the check, so possibly from older sources): rebuild it (drop --pkg/--pkg-ref; needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli --version 0.2.129`)"
        ),
    }
}
