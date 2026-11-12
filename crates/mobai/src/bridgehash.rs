//! (tickexact 2026-11-12, ironfarm's definition for push-scan's bridge, G1's CHANGES 3 on
//! 54fb5abe4) The neighbourhood hash of a composed world: the union of the 5x5x5 cubes centred on
//! the bridged pair's two cells A and B, each cell once, ascending x, then y, then z; each cell
//! contributes the bytes of `x,y,z=STATE;` (the full state string, properties alphabetical, as the
//! design files hold them; an empty cell `minecraft:air`; entities not included); FNV-1a 64 over
//! all of it, printed as 16 lowercase hex digits. The states come from the composition (the module,
//! then each part over it, no move) and every one is tested in the game (`execute if block` of the
//! full state), so a printed hash with 0 differing cells is the game's world.

use anyhow::Result;
use mctas::vanilla::VanillaServer;
use std::collections::{BTreeMap, BTreeSet};

const DY: i32 = 80;

/// The composed states: the module, then each part laid over it with no move.
pub(crate) fn composed(module: &std::path::Path, parts: &[std::path::PathBuf]) -> Result<BTreeMap<[i32; 3], String>> {
    let mut m = BTreeMap::new();
    for f in std::iter::once(module).chain(parts.iter().map(|p| p.as_path())) {
        for (p, s) in mctas::design::Design::load(f)?.blocks.iter() {
            m.insert([p[0], p[1], p[2]], s.to_string());
        }
    }
    Ok(m)
}

/// FNV-1a 64 of `x,y,z=STATE;` over the cells within 2 of `a` or `b`.
pub(crate) fn hash_of(comp: &BTreeMap<[i32; 3], String>, a: [i32; 3], b: [i32; 3]) -> (u64, Vec<[i32; 3]>) {
    let mut cells: BTreeSet<[i32; 3]> = BTreeSet::new();
    for c in [a, b] {
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    cells.insert([c[0] + dx, c[1] + dy, c[2] + dz]);
                }
            }
        }
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in &cells {
        let s = comp.get(p).map(|s| s.as_str()).unwrap_or("minecraft:air");
        for byte in format!("{},{},{}={s};", p[0], p[1], p[2]).bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    (h, cells.into_iter().collect())
}

/// The pair's hash from the composition, each of its cells tested in the game: (hex, the cells
/// whose game state differs from the composition's, each with the game's state where one of the
/// usual candidates matches it).
pub(crate) fn neighborhood_hash(srv: &mut VanillaServer, comp: &BTreeMap<[i32; 3], String>, a: [i32; 3], b: [i32; 3]) -> Result<(String, Vec<String>, Option<String>)> {
    let (h, cells) = hash_of(comp, a, b);
    let mut bad = Vec::new();
    let mut game = comp.clone();
    let mut known = true;
    for p in cells {
        let s = comp.get(&p).map(|s| s.as_str()).unwrap_or("minecraft:air");
        if !srv.cmd_long(&format!("execute if block {} {} {} {s}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            let found = identify(srv, p)?;
            bad.push(format!("({},{},{}) the composition {s}, the game {}", p[0], p[1], p[2], found.as_deref().unwrap_or("(not among the candidates)")));
            match found {
                Some(f) => {
                    game.insert(p, f);
                }
                None => known = false,
            }
        }
    }
    // the game's own hash, when every differing cell was identified
    let game_hash = (!bad.is_empty() && known).then(|| format!("{:016x}", hash_of(&game, a, b).0));
    Ok((format!("{h:016x}"), bad, game_hash))
}

/// A cell's state in the game among the usual candidates near a lane (fluids at every level, air,
/// the build's blocks), or None.
fn identify(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<String>> {
    let mut cands: Vec<String> = Vec::new();
    for fluid in ["water", "lava"] {
        for level in 0..16 {
            cands.push(format!("minecraft:{fluid}[level={level}]"));
        }
    }
    for b in ["minecraft:air", "minecraft:cave_air", "minecraft:glass", "minecraft:stone", "minecraft:slime_block", "minecraft:glowstone", "minecraft:redstone_block"] {
        cands.push(b.to_string());
    }
    for c in cands {
        if srv.cmd_long(&format!("execute if block {} {} {} {c}", p[0], p[1] + DY, p[2]))?.contains("passed") {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// Print the K-R pairs' hashes of the three lanes (R (x-2,24,7), K (x-2,25,9) for x = -1, 3, 7).
pub(crate) fn print_kr(srv: &mut VanillaServer, comp: &BTreeMap<[i32; 3], String>) -> Result<()> {
    for x in [-1, 3, 7] {
        let (a, b) = ([x - 2, 24, 7], [x - 2, 25, 9]);
        let (h, bad, game) = neighborhood_hash(srv, comp, a, b)?;
        println!(
            "  bridge hash R~K lane {x} ({},{},{})~({},{},{}): {h} from the composition; {} of its cells differ in the game{}{}",
            a[0],
            a[1],
            a[2],
            b[0],
            b[1],
            b[2],
            bad.len(),
            if bad.is_empty() { String::new() } else { format!(": {}", bad.join("; ")) },
            match game {
                Some(g) => format!("; the game's own hash {g}"),
                None if bad.is_empty() => " (the game's hash is the same)".to_string(),
                None => "; the game's own hash unknown (a cell not identified)".to_string(),
            }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn fnv_of_air_cell() {
        // one cell's bytes, FNV-1a 64 by hand: a known value guards the constants and the order
        let mut comp = std::collections::BTreeMap::new();
        comp.insert([0, 0, 0], "minecraft:glass".to_string());
        let (h1, cells) = super::hash_of(&comp, [0, 0, 0], [0, 0, 0]);
        assert_eq!(cells.len(), 125);
        assert_eq!(cells[0], [-2, -2, -2]);
        let (h2, _) = super::hash_of(&std::collections::BTreeMap::new(), [0, 0, 0], [0, 0, 0]);
        assert_ne!(h1, h2);
    }
}
