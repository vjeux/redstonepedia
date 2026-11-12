//! (tickexact 2026-11-12, ironspec: cite a cell-diff of a design's revision) `mctas mobai
//! design-diff <old> <new>`: two design files' cells, overworld and Nether parts, compared cell by
//! cell (positions as the files give them, relative to their origins, which must match): every
//! cell added (and whether it was air before: absent from the old file), removed or changed.

use anyhow::{Result, bail};
use std::collections::BTreeMap;

fn diff(part: &str, old: &BTreeMap<[i32; 3], String>, new: &BTreeMap<[i32; 3], String>) -> (usize, usize, usize) {
    let (mut added, mut removed, mut changed) = (Vec::new(), Vec::new(), Vec::new());
    for (p, s) in new {
        match old.get(p) {
            None => added.push(format!("({},{},{}) {s}", p[0], p[1], p[2])),
            Some(o) if o != s => changed.push(format!("({},{},{}) {o} -> {s}", p[0], p[1], p[2])),
            _ => {}
        }
    }
    for (p, s) in old {
        if !new.contains_key(p) {
            removed.push(format!("({},{},{}) {s}", p[0], p[1], p[2]));
        }
    }
    println!("{part}: {} cells added (each absent, air, in the old file), {} removed, {} changed", added.len(), removed.len(), changed.len());
    for (what, v) in [("added", &added), ("removed", &removed), ("changed", &changed)] {
        if !v.is_empty() {
            println!("  {what}: {}", v.join(", "));
        }
    }
    (added.len(), removed.len(), changed.len())
}

pub fn design_diff(old: &std::path::Path, new: &std::path::Path) -> Result<()> {
    let a = mctas::design::Design::load(old)?;
    let b = mctas::design::Design::load(new)?;
    println!("== design-diff {} -> {}", old.display(), new.display());
    if a.meta.get("origin") != b.meta.get("origin") || a.meta.get("nether-origin") != b.meta.get("nether-origin") {
        bail!("the two files' origins differ: compare positions by hand");
    }
    let o = diff("overworld", &a.blocks, &b.blocks);
    let n = diff("nether", &a.nether_blocks, &b.nether_blocks);
    println!("totals: {} added, {} removed, {} changed", o.0 + n.0, o.1 + n.1, o.2 + n.2);
    Ok(())
}
