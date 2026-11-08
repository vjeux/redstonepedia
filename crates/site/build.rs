//! The site build caches its simulations by content (src/cache.rs, the coordinator 2026-10-21
//! 06:07Z: deploys had gone from 1.5 to 4.5 minutes): an entry's key hashes what the result
//! depends on and the source of the code that computes it. This script hashes that code, one hash
//! per domain over the crates the domain runs, so a change to the simulator recomputes exactly
//! what it touches and nothing else (a viewer change does not regenerate the clock members).
use sha2::{Digest, Sha256};
use std::path::Path;

/// (domain, the crates whose code computes its entries, its own file of src/simulate/, where the
/// site's own simulation code lives: each domain also hashes src/simulate/mod.rs).
const DOMAINS: &[(&str, &[&str], &str)] = &[
    // the generated clock members: the clock judge's simulation, the design format, the
    // generators' dispatch (each generator's own crate is a domain below: a change to one
    // generator regenerates its families' members only)
    ("MEMBERS", &["mctas", "redsim", "clock", "clock-cli"], "members.rs"),
    ("GEN_ITEM", &["clock-item"], "members.rs"),
    ("GEN_MISC", &["clock-misc"], "members.rs"),
    ("GEN_OBS", &["clock-obs"], "members.rs"),
    ("GEN_RT", &["clock-rt"], "members.rs"),
    ("GEN_SCULK", &["clock-sculk"], "members.rs"),
    // (a generator of no known crate: all of them)
    ("GEN_ALL", &["clock-item", "clock-misc", "clock-obs", "clock-rt", "clock-sculk"], "members.rs"),
    // the circuit judge's run of a design (its timing diagram, metrics, verdict)
    ("CIRCUIT", &["mctas", "redsim", "clock", "circuit"], "judge.rs"),
    // a work-in-progress design's verdict for `mctas site preview` (the kind's redsim judge): the
    // judge's crates too, its own file (declared from lib.rs, so src/simulate/mod.rs, which every
    // domain hashes, stays as it is): it never recomputes the build's circuit runs
    ("PREVIEW", &["mctas", "redsim", "clock", "circuit"], "preview.rs"),
    // the 3D view's own session (whether it runs a design, a shulker machine's run, where a
    // circuit's demo stops, a clock's chart and lamp check, a thumbnail's still frame)
    ("VIEW", &["redsim", "mcview"], "view.rs"),
];

fn hash_file(h: &mut Sha256, root: &Path, path: &Path) {
    if let Ok(bytes) = std::fs::read(path) {
        let rel = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        h.update((rel.len() as u64).to_le_bytes());
        h.update(rel.as_bytes());
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(&bytes);
    }
}

/// Every file under `dir`, in path order.
fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let site = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).to_path_buf();
    let crates = site.parent().expect("crates/").to_path_buf();
    let own = site.join("src/simulate");
    println!("cargo:rerun-if-changed={}", own.display());
    for (name, list, file) in DOMAINS {
        let mut h = Sha256::new();
        h.update(name.as_bytes());
        for c in *list {
            let dir = crates.join(c);
            // (a directory: cargo looks at every file under it)
            println!("cargo:rerun-if-changed={}", dir.join("src").display());
            println!("cargo:rerun-if-changed={}", dir.join("Cargo.toml").display());
            hash_file(&mut h, &crates, &dir.join("Cargo.toml"));
            let mut all = Vec::new();
            files(&dir.join("src"), &mut all);
            for f in &all {
                hash_file(&mut h, &crates, f);
            }
        }
        hash_file(&mut h, &crates, &own.join("mod.rs"));
        hash_file(&mut h, &crates, &own.join(file));
        println!("cargo:rustc-env=SITE_CODE_{name}={:x}", h.finalize());
    }
}
