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
pub(crate) fn identify(srv: &mut VanillaServer, p: [i32; 3]) -> Result<Option<String>> {
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

/// `mctas mobai bridge-hash`: the pairs' hashes of a composition from its files alone (no server),
/// with `--list` each cube's non-air cells (to compare cell lists with another tool's).
pub fn bridge_hash_files(module: &std::path::Path, parts: &[std::path::PathBuf], pairs: &[([i32; 3], [i32; 3])], list: bool) -> Result<()> {
    let comp = composed(module, parts)?;
    for (a, b) in pairs {
        let (h, cells) = hash_of(&comp, *a, *b);
        println!("  {},{},{}~{},{},{}: {h:016x} ({} cells)", a[0], a[1], a[2], b[0], b[1], b[2], cells.len());
        if list {
            for p in cells {
                if let Some(s) = comp.get(&p) {
                    if s != "minecraft:air" {
                        println!("    {},{},{}={s}", p[0], p[1], p[2]);
                    }
                }
            }
        }
    }
    Ok(())
}

/// The hash's own checks, run by `cargo test` and by `mobai bridge-hash --selftest` (the release
/// binary, no test build): the published FNV-1a 64 vectors, the cube's order and bytes, and
/// ironfarm's lane 3 cross-check value (G1's pin).
pub(crate) mod checks {
    use anyhow::{Result, ensure};

    /// FNV-1a 64 of a byte string, as `hash_of` folds it.
    fn fnv(s: &str) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in s.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// The published FNV-1a 64 test vectors.
    pub(crate) fn fnv_vectors() -> Result<()> {
        ensure!(fnv("") == 0xcbf2_9ce4_8422_2325, "fnv(\"\")");
        ensure!(fnv("a") == 0xaf63_dc4c_8601_ec8c, "fnv(\"a\")");
        ensure!(fnv("foobar") == 0x8594_4171_f739_67e8, "fnv(\"foobar\")");
        Ok(())
    }

    /// ironfarm's cross-check value: lane 3's R~K cube of e9eeeee52's module with 71b76c221's slime
    /// lanes file laid over it, its non-air cells as `mobai bridge-hash --list` prints them (the
    /// other 148 of the 190 cells air).
    pub(crate) fn lane3_cross_check() -> Result<()> {
        let cells = [
            "-1,22,6=minecraft:rail[shape=ascending_south,waterlogged=false]",
            "-1,22,7=minecraft:glass",
            "-1,23,7=minecraft:glass",
            "-1,23,8=minecraft:glass",
            "-1,24,5=minecraft:glass",
            "-1,24,7=minecraft:rail[shape=north_west,waterlogged=false]",
            "-1,24,8=minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]",
            "-1,24,9=minecraft:glass",
            "-1,25,9=minecraft:rail[shape=north_east,waterlogged=false]",
            "-1,26,9=minecraft:lever[face=ceiling,facing=north,powered=false]",
            "-1,27,9=minecraft:glass",
            "0,24,9=minecraft:glass",
            "0,25,9=minecraft:rail[shape=east_west,waterlogged=false]",
            "1,22,8=minecraft:glass",
            "1,23,6=minecraft:glass",
            "1,23,7=minecraft:slime_block",
            "1,23,8=minecraft:repeater[delay=1,facing=south,locked=false,powered=false]",
            "1,24,6=minecraft:rail[shape=south_east,waterlogged=false]",
            "1,24,7=minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]",
            "1,24,8=minecraft:glass",
            "1,24,9=minecraft:glass",
            "1,25,8=minecraft:rail[shape=north_south,waterlogged=false]",
            "1,25,9=minecraft:rail[shape=north_east,waterlogged=false]",
            "1,26,9=minecraft:lever[face=ceiling,facing=north,powered=false]",
            "1,27,9=minecraft:glass",
            "2,23,6=minecraft:glass",
            "2,23,7=minecraft:glass",
            "2,24,6=minecraft:rail[shape=south_west,waterlogged=false]",
            "2,24,7=minecraft:rail[shape=north_east,waterlogged=false]",
            "2,24,9=minecraft:glass",
            "2,25,9=minecraft:rail[shape=east_west,waterlogged=false]",
            "3,22,6=minecraft:rail[shape=ascending_south,waterlogged=false]",
            "3,22,7=minecraft:glass",
            "3,23,7=minecraft:glass",
            "3,23,8=minecraft:glass",
            "3,24,5=minecraft:glass",
            "3,24,7=minecraft:rail[shape=north_west,waterlogged=false]",
            "3,24,8=minecraft:detector_rail[powered=false,shape=ascending_south,waterlogged=false]",
            "3,24,9=minecraft:glass",
            "3,25,9=minecraft:rail[shape=north_east,waterlogged=false]",
            "3,26,9=minecraft:lever[face=ceiling,facing=north,powered=false]",
            "3,27,9=minecraft:glass",
        ];
        let mut comp = std::collections::BTreeMap::new();
        for c in cells {
            let (p, s) = c.split_once('=').ok_or_else(|| anyhow::anyhow!("cell {c}"))?;
            let v: Vec<i32> = p.split(',').map(|n| n.parse()).collect::<Result<_, _>>()?;
            comp.insert([v[0], v[1], v[2]], s.to_string());
        }
        let (h, n) = super::hash_of(&comp, [1, 24, 7], [1, 25, 9]);
        ensure!(n.len() == 190, "lane 3's cubes: {} cells", n.len());
        ensure!(format!("{h:016x}") == "5d5bd15bea9d65f6", "lane 3's hash {h:016x}, ironfarm's 5d5bd15bea9d65f6");
        Ok(())
    }

    /// A one-cube pair: 125 cells from (-2,-2,-2) in ascending x, y, z; every cell's bytes.
    pub(crate) fn cube_order_and_bytes() -> Result<()> {
        let mut comp = std::collections::BTreeMap::new();
        comp.insert([0, 0, 0], "minecraft:glass".to_string());
        let (h, cells) = super::hash_of(&comp, [0, 0, 0], [0, 0, 0]);
        ensure!(cells.len() == 125 && cells[0] == [-2, -2, -2] && cells[1] == [-2, -2, -1], "the cube's order");
        let mut s = String::new();
        for p in &cells {
            s.push_str(&format!("{},{},{}={};", p[0], p[1], p[2], if *p == [0, 0, 0] { "minecraft:glass" } else { "minecraft:air" }));
        }
        ensure!(h == fnv(&s), "the cube's bytes");
        Ok(())
    }

    /// Every check, each named on its line.
    pub(crate) fn all() -> Result<()> {
        for (name, f) in [("fnv_vectors", fnv_vectors as fn() -> Result<()>), ("cube_order_and_bytes", cube_order_and_bytes), ("lane3_cross_check", lane3_cross_check)] {
            f()?;
            println!("  {name}: PASS");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fnv_vectors() {
        super::checks::fnv_vectors().unwrap();
    }

    #[test]
    fn lane3_cross_check() {
        super::checks::lane3_cross_check().unwrap();
    }

    #[test]
    fn cube_order_and_bytes() {
        super::checks::cube_order_and_bytes().unwrap();
    }
}
