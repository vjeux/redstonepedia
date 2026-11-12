//! `mctas mobfarm bill --components` (farmopt and ironspec, Nov 11: the parent's v4-vs-(b)
//! decision for vjeux): a design's redstone components and blocks, counted from the file, whole
//! and per named box, plus its controlled places (the gates a controller drives).
//!
//! The kinds counted: repeaters, comparators, observers, dust, torches (standing and wall),
//! detector rails, powered rails (split by their stored state: on at rest = a booster held on by
//! its power, off at rest = switched or unpowered), redstone blocks, other redstone (levers,
//! buttons, plates, targets, pistons, droppers, hoppers, lamps, ... each named), gates (fence
//! gates, doors, trapdoors: each one a controlled place), plain rails, activator rails, and every
//! block. Air is not a block.

use std::collections::BTreeMap;

/// One counted kind: its row label and whether it is a redstone component (the components total).
const ROWS: [(&str, bool); 14] = [
    ("repeaters", true),
    ("comparators", true),
    ("observers", true),
    ("dust", true),
    ("torches", true),
    ("detector rails", true),
    ("powered rails, on at rest", true),
    ("powered rails, off at rest", true),
    ("redstone blocks", true),
    ("other redstone", true),
    ("gates (controlled places)", false),
    ("plain rails", false),
    ("activator rails", false),
    ("other blocks", false),
];

/// The other-redstone kinds (named in their own line).
const OTHER_REDSTONE: [&str; 22] = [
    "lever", "target", "piston", "sticky_piston", "dropper", "dispenser", "hopper", "note_block", "daylight_detector",
    "redstone_lamp", "tripwire_hook", "tripwire", "lightning_rod", "sculk_sensor", "calibrated_sculk_sensor",
    "trapped_chest", "crafter", "copper_bulb", "jukebox", "tnt", "bell", "lectern",
];

/// The row of one block state string (`minecraft:` stripped or not, properties in brackets).
pub fn row_of(state: &str) -> usize {
    let s = state.strip_prefix("minecraft:").unwrap_or(state);
    let kind = s.split('[').next().unwrap_or("");
    let props = s.split_once('[').map(|(_, p)| p.trim_end_matches(']')).unwrap_or("");
    let powered = props.split(',').any(|kv| kv.trim() == "powered=true");
    match kind {
        "repeater" => 0,
        "comparator" => 1,
        "observer" => 2,
        "redstone_wire" => 3,
        "redstone_torch" | "redstone_wall_torch" => 4,
        "detector_rail" => 5,
        "powered_rail" if powered => 6,
        "powered_rail" => 7,
        "redstone_block" => 8,
        k if OTHER_REDSTONE.contains(&k) || k.ends_with("_button") || k.ends_with("_pressure_plate") => 9,
        k if k.ends_with("_fence_gate") || k.ends_with("_door") || k.ends_with("_trapdoor") => 10,
        "rail" => 11,
        "activator_rail" => 12,
        _ => 13,
    }
}

/// A named box, inclusive corners.
pub struct PartBox {
    pub name: String,
    pub lo: [i32; 3],
    pub hi: [i32; 3],
}

/// Parse `NAME=X0,Y0,Z0..X1,Y1,Z1`.
pub fn parse_part(t: &str) -> Result<PartBox, String> {
    let (name, b) = t.split_once('=').ok_or_else(|| format!("--part takes NAME=X0,Y0,Z0..X1,Y1,Z1: {t}"))?;
    let (a, c) = b.split_once("..").ok_or_else(|| format!("--part takes NAME=X0,Y0,Z0..X1,Y1,Z1: {t}"))?;
    let p = |s: &str| -> Result<[i32; 3], String> {
        let v: Vec<i32> = s.split(',').map(|n| n.trim().parse::<i32>().map_err(|e| format!("{t}: {e}"))).collect::<Result<_, _>>()?;
        match v.as_slice() {
            [x, y, z] => Ok([*x, *y, *z]),
            _ => Err(format!("--part corners take X,Y,Z: {t}")),
        }
    };
    let (a, c) = (p(a)?, p(c)?);
    let lo = [a[0].min(c[0]), a[1].min(c[1]), a[2].min(c[2])];
    let hi = [a[0].max(c[0]), a[1].max(c[1]), a[2].max(c[2])];
    Ok(PartBox { name: name.to_string(), lo, hi })
}

/// The table: one column per box (first box holding a cell wins), then "rest" (cells in no box)
/// when there are boxes, then the whole design; the other-redstone and gate kinds named below.
pub fn table(blocks: &[([i32; 3], String)], parts: &[PartBox]) -> String {
    let ncol = parts.len() + usize::from(!parts.is_empty()) + 1;
    let mut counts = vec![[0usize; ROWS.len()]; ncol];
    let mut named: BTreeMap<(usize, String), usize> = BTreeMap::new();
    let mut gates: Vec<String> = Vec::new();
    for (p, s) in blocks {
        let r = row_of(s);
        let col = parts.iter().position(|b| (0..3).all(|i| p[i] >= b.lo[i] && p[i] <= b.hi[i])).unwrap_or(parts.len());
        if !parts.is_empty() {
            counts[col][r] += 1;
        }
        counts[ncol - 1][r] += 1;
        let kind = s.strip_prefix("minecraft:").unwrap_or(s).split('[').next().unwrap_or("").to_string();
        if r == 9 || r == 10 {
            *named.entry((r, kind.clone())).or_default() += 1;
        }
        if r == 10 {
            let label = if parts.is_empty() { String::new() } else { format!(" ({})", parts.get(col).map(|b| b.name.as_str()).unwrap_or("rest")) };
            gates.push(format!("{},{},{} {kind}{label}", p[0], p[1], p[2]));
        }
    }
    let mut heads: Vec<String> = parts.iter().map(|b| b.name.clone()).collect();
    if !parts.is_empty() {
        heads.push("rest".into());
    }
    heads.push("whole".into());
    let mut out = format!("{:<28}", "kind");
    for h in &heads {
        out.push_str(&format!(" {h:>9}"));
    }
    out.push('\n');
    for (r, (label, _)) in ROWS.iter().enumerate() {
        out.push_str(&format!("{label:<28}"));
        for c in &counts {
            out.push_str(&format!(" {:>9}", c[r]));
        }
        out.push('\n');
    }
    for (label, pick) in [("components (redstone)", true), ("blocks (every non-air)", false)] {
        out.push_str(&format!("{label:<28}"));
        for c in &counts {
            let n: usize = ROWS.iter().enumerate().filter(|(_, (_, comp))| !pick || *comp).map(|(r, _)| c[r]).sum();
            out.push_str(&format!(" {n:>9}"));
        }
        out.push('\n');
    }
    let other: Vec<String> = named.iter().filter(|((r, _), _)| *r == 9).map(|((_, k), n)| format!("{k} {n}")).collect();
    if !other.is_empty() {
        out.push_str(&format!("other redstone: {}\n", other.join(", ")));
    }
    if let Some((lo, hi)) = blocks.iter().fold(None::<([i32; 3], [i32; 3])>, |acc, (p, _)| match acc {
        None => Some((*p, *p)),
        Some((l, h)) => Some(([l[0].min(p[0]), l[1].min(p[1]), l[2].min(p[2])], [h[0].max(p[0]), h[1].max(p[1]), h[2].max(p[2])])),
    }) {
        out.push_str(&format!("extent: x {}..{} y {}..{} z {}..{}\n", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]));
    }
    out.push_str(&format!("controlled places (gates, {}): {}\n", gates.len(), if gates.is_empty() { "none".to_string() } else { gates.join("; ") }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_land_in_their_rows_and_boxes_split_the_count() {
        let b = |x: i32, s: &str| ([x, 0, 0], s.to_string());
        let blocks = vec![
            b(0, "minecraft:repeater[delay=2,facing=north]"),
            b(1, "minecraft:powered_rail[powered=true,shape=north_south]"),
            b(2, "minecraft:powered_rail[powered=false,shape=north_south]"),
            b(3, "minecraft:oak_fence_gate[open=false]"),
            b(4, "minecraft:stone_button[face=floor]"),
            b(5, "minecraft:stone"),
            b(6, "minecraft:redstone_wall_torch[facing=east]"),
        ];
        assert_eq!(row_of("minecraft:powered_rail[powered=true]"), 6);
        assert_eq!(row_of("powered_rail[powered=false]"), 7);
        assert_eq!(row_of("minecraft:stone_button[face=floor]"), 9);
        assert_eq!(row_of("minecraft:spruce_fence_gate"), 10);
        let parts = vec![parse_part("a=0,0,0..2,0,0").unwrap()];
        let t = table(&blocks, &parts);
        let line = |label: &str| t.lines().find(|l| l.starts_with(label)).unwrap().split_whitespace().rev().take(3).map(|s| s.parse::<usize>().unwrap()).collect::<Vec<_>>();
        // (whole, rest, a) read right to left
        assert_eq!(line("repeaters"), vec![1, 0, 1]);
        assert_eq!(line("torches"), vec![1, 1, 0]);
        assert_eq!(line("components (redstone)"), vec![5, 2, 3]);
        assert_eq!(line("blocks (every non-air)"), vec![7, 4, 3]);
        assert!(t.contains("controlled places (gates, 1): 3,0,0 oak_fence_gate (rest)"));
        assert!(t.contains("other redstone: stone_button 1"));
    }
}
