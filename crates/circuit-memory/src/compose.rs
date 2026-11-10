//! `mctas circuit memory compose`: one design from several, each placed at an offset (and turned),
//! for building a big circuit from tested modules (the carousel's nightly scheduler, day-1 lane
//! 2026-11-10). A cell two modules both fill must hold the same block (else it fails, naming the
//! cell); ports are kept, renamed with the module's prefix (`input` -> `input.<prefix>`, `input.n`
//! -> `input.<prefix>-n`, likewise `output`), or dropped (`=-`: a module whose inputs are fed by
//! other modules); block entities move with their blocks.

use anyhow::{Context as _, Result, anyhow, bail};
use mctas::design::Design;
use std::path::Path;

/// One `PATH@DX,DY,DZ[,TURNS][=PREFIX]`.
struct Part {
    path: String,
    off: [i32; 3],
    turns: u8,
    prefix: Option<String>,
}

fn parse(spec: &str) -> Result<Part> {
    let (path, rest) = spec.rsplit_once('@').ok_or_else(|| anyhow!("{spec:?}: PATH@DX,DY,DZ[,TURNS][=PREFIX]"))?;
    let (nums, prefix) = match rest.split_once('=') {
        Some((n, p)) => (n, Some(p.to_string())),
        None => (rest, None),
    };
    let v: Vec<i32> = nums.split(',').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("{spec:?}: offsets"))?;
    if v.len() != 3 && v.len() != 4 {
        bail!("{spec:?}: DX,DY,DZ[,TURNS]");
    }
    Ok(Part { path: path.to_string(), off: [v[0], v[1], v[2]], turns: v.get(3).copied().unwrap_or(0).rem_euclid(4) as u8, prefix })
}

fn rename(port: &str, prefix: &str) -> String {
    for kind in ["input", "output"] {
        if port == kind {
            return format!("{kind}.{prefix}");
        }
        if let Some(n) = port.strip_prefix(&format!("{kind}.")) {
            return format!("{kind}.{prefix}-{n}");
        }
    }
    format!("{prefix}-{port}")
}

/// Compose `parts` into one design named `name`, written to `out`.
pub fn compose(out: &Path, name: &str, parts: &[String]) -> Result<()> {
    let mut d = Design::parse(&format!("name: {name}\n"))?;
    for spec in parts {
        let p = parse(spec)?;
        let m = Design::load(Path::new(&p.path)).with_context(|| format!("loading {}", p.path))?;
        let place = |q: [i32; 3]| -> [i32; 3] {
            let r = clock::rotate_pos(q, p.turns);
            [r[0] + p.off[0], r[1] + p.off[1], r[2] + p.off[2]]
        };
        for (q, s) in &m.blocks {
            let at = place(*q);
            let s = clock::canon::rotate_state(s, p.turns);
            match d.blocks.get(&at) {
                Some(old) if *old != s => bail!("{}: cell {:?} holds {old} already, {} puts {s} there", p.path, at, p.path),
                _ => {
                    d.blocks.insert(at, s);
                }
            }
        }
        for (q, nbt) in &m.block_entities {
            d.block_entities.insert(place(*q), nbt.clone());
        }
        match p.prefix.as_deref() {
            Some("-") => {}
            pre => {
                for (k, q) in &m.ports {
                    let key = match pre {
                        Some(pre) => rename(k, pre),
                        None => k.clone(),
                    };
                    if d.ports.contains_key(&key) {
                        bail!("{}: port {key} twice (give the module a prefix: =NAME)", p.path);
                    }
                    d.ports.insert(key, place(*q));
                }
            }
        }
    }
    d.save(out)?;
    println!("{}: {} blocks from {} modules", out.display(), d.blocks.len(), parts.len());
    Ok(())
}
