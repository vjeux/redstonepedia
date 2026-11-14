//! (farmopt / G1 Nov 14 13:19, part 88) The portal FRAME check: every nether portal plane of a design must stand on a
//! frame 26.3's `PortalShape` accepts (its `FRAME` test is obsidian alone): obsidian under every bottom cell, above every
//! top cell, and in both side columns over the plane's whole height (the corners aren't needed); the plane's interior
//! a full rectangle of portal blocks of one axis, 2..21 wide and 3..21 high (`PortalShape`'s MIN / MAX width and
//! height). A plane failing any of it would not survive a neighbour update (`NetherPortalBlock.updateShape` re-checks
//! the shape) or not light at all: each failure named.

use std::collections::{BTreeMap, BTreeSet};

/// One plane's verdict: its axis, its interior's box (min and max corners), and its failures (empty: it stands).
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub axis: char,
    pub lo: [i32; 3],
    pub hi: [i32; 3],
    pub failures: Vec<String>,
}

fn axis_of(state: &str) -> Option<char> {
    if !state.starts_with("minecraft:nether_portal") {
        return None;
    }
    Some(if state.contains("axis=z") { 'z' } else { 'x' })
}

/// The planes of a design's blocks (its portal cells grouped by axis and 6-connectivity within the plane), each checked.
pub fn portal_planes(blocks: &BTreeMap<[i32; 3], String>) -> Vec<Plane> {
    let portals: BTreeMap<[i32; 3], char> = blocks.iter().filter_map(|(p, s)| axis_of(s).map(|a| (*p, a))).collect();
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut out = Vec::new();
    for (start, axis) in &portals {
        if seen.contains(start) {
            continue;
        }
        // (the plane: the axis's horizontal direction and y; its normal fixed)
        let (along, normal) = if *axis == 'x' { (0usize, 2usize) } else { (2usize, 0usize) };
        let mut comp = vec![*start];
        let mut stack = vec![*start];
        seen.insert(*start);
        while let Some(c) = stack.pop() {
            for (d_along, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let mut n = c;
                n[along] += d_along;
                n[1] += dy;
                if portals.get(&n) == Some(axis) && seen.insert(n) {
                    comp.push(n);
                    stack.push(n);
                }
            }
        }
        let lo = [0, 1, 2].map(|i| comp.iter().map(|c| c[i]).min().unwrap_or(0));
        let hi = [0, 1, 2].map(|i| comp.iter().map(|c| c[i]).max().unwrap_or(0));
        let at = |a: i32, y: i32| -> [i32; 3] {
            let mut p = [0; 3];
            p[along] = a;
            p[1] = y;
            p[normal] = lo[normal];
            p
        };
        let obsidian = |p: [i32; 3]| blocks.get(&p).is_some_and(|s| s == "minecraft:obsidian");
        let cell = |p: [i32; 3]| format!("{},{},{}", p[0], p[1], p[2]);
        let mut failures = Vec::new();
        let (w, h) = (hi[along] - lo[along] + 1, hi[1] - lo[1] + 1);
        if !(2..=21).contains(&w) || !(3..=21).contains(&h) {
            failures.push(format!("an interior {w} wide and {h} high (26.3 PortalShape: 2..21 wide, 3..21 high)"));
        }
        for a in lo[along]..=hi[along] {
            for y in lo[1]..=hi[1] {
                if portals.get(&at(a, y)) != Some(axis) {
                    failures.push(format!("{} inside the plane's rectangle holds {} (the interior must be all portal)", cell(at(a, y)), blocks.get(&at(a, y)).map_or("air", |s| s.as_str())));
                }
            }
            for (y, side) in [(lo[1] - 1, "the bottom"), (hi[1] + 1, "the top row")] {
                if !obsidian(at(a, y)) {
                    failures.push(format!("{side} frame cell {} is {}, not obsidian", cell(at(a, y)), blocks.get(&at(a, y)).map_or("air", |s| s.as_str())));
                }
            }
        }
        for y in lo[1]..=hi[1] {
            for (a, side) in [(lo[along] - 1, "a side column"), (hi[along] + 1, "a side column")] {
                if !obsidian(at(a, y)) {
                    failures.push(format!("{side}'s frame cell {} is {}, not obsidian", cell(at(a, y)), blocks.get(&at(a, y)).map_or("air", |s| s.as_str())));
                }
            }
        }
        out.push(Plane { axis: *axis, lo, hi, failures });
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    fn plane(missing: Option<[i32; 3]>) -> BTreeMap<[i32; 3], String> {
        // (an axis-x portal 3 wide and 3 high at z 0: interior x 1..3, y 1..3; its frame x 0 / 4, y 0 / 4)
        let mut b = BTreeMap::new();
        for x in 0..=4 {
            for y in 0..=4 {
                let edge_x = x == 0 || x == 4;
                let edge_y = y == 0 || y == 4;
                if edge_x && edge_y {
                    continue;
                }
                let s = if edge_x || edge_y { "minecraft:obsidian" } else { "minecraft:nether_portal[axis=x]" };
                b.insert([x, y, 0], s.to_string());
            }
        }
        if let Some(m) = missing {
            b.remove(&m);
        }
        b
    }

    /// (part 88) a whole frame (corners left out) stands; a PLANTED plane missing one frame cell fails by name (a bottom
    /// cell, a side cell, a top cell each); an interior hole fails by name; an axis-z plane reads in its own direction
    #[test]
    fn a_plane_missing_one_frame_cell_fails_by_name() {
        let ok = super::portal_planes(&plane(None));
        assert!(ok.len() == 1 && ok[0].axis == 'x' && ok[0].failures.is_empty() && ok[0].lo == [1, 1, 0] && ok[0].hi == [3, 3, 0], "{ok:?}");
        for (m, why) in [([2, 0, 0], "the bottom frame cell 2,0,0 is air"), ([0, 2, 0], "a side column's frame cell 0,2,0 is air"), ([3, 4, 0], "the top row frame cell 3,4,0 is air")] {
            let r = super::portal_planes(&plane(Some(m)));
            assert!(r.len() == 1 && r[0].failures.len() == 1 && r[0].failures[0].contains(why), "{m:?}: {r:?}");
        }
        let mut hole = plane(None);
        hole.insert([2, 2, 0], "minecraft:air".to_string());
        let r = super::portal_planes(&hole);
        assert!(r.iter().any(|p| p.failures.iter().any(|f| f.contains("inside the plane's rectangle"))), "{r:?}");
        let z: BTreeMap<[i32; 3], String> = plane(None).into_iter().map(|([x, y, _], s)| ([0, y, x], s.replace("axis=x", "axis=z"))).collect();
        let r = super::portal_planes(&z);
        assert!(r.len() == 1 && r[0].axis == 'z' && r[0].failures.is_empty(), "{r:?}");
    }
}
