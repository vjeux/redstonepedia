//! Chunk-line crossings of a design's rails (ironspec's placement rule, Nov 11): where the farm's
//! track crosses a chunk line, a cart frozen on one side meets carts still moving on the other.
//! The rule wants plain, unpowered cells on both sides and any climb at least 2 cells away. For
//! each crossing (every pair of connected rail cells in different chunk columns) this prints both
//! cells (block, shape, powered as placed) and, walking away from the line along the track, the
//! distance to the first climb, drop, peak and powered rail on that side.

use std::collections::{BTreeSet, HashMap};

/// A rail as placed: its block (rail, powered_rail, detector_rail, activator_rail), shape and
/// powered state.
#[derive(Clone, Debug)]
pub struct Rail {
    pub block: String,
    pub shape: String,
    pub powered: bool,
}

/// The rail at a block string, if it is one.
pub fn rail_of(s: &str) -> Option<Rail> {
    let s = s.trim_start_matches("minecraft:");
    let (name, props) = match s.split_once('[') {
        Some((n, p)) => (n, p.trim_end_matches(']')),
        None => (s, ""),
    };
    if !matches!(name, "rail" | "powered_rail" | "detector_rail" | "activator_rail") {
        return None;
    }
    let get = |k: &str| props.split(',').find_map(|kv| kv.split_once('=').filter(|(a, _)| a.trim() == k).map(|(_, v)| v.trim().to_string()));
    Some(Rail { block: name.to_string(), shape: get("shape").unwrap_or_else(|| "north_south".into()), powered: get("powered").is_some_and(|v| v == "true") })
}

/// The two ends of a shape: (dx, dz, up) with up 1 for an ascending rail's high end.
fn ends(shape: &str) -> [(i32, i32, i32); 2] {
    match shape {
        "east_west" => [(1, 0, 0), (-1, 0, 0)],
        "ascending_north" => [(0, -1, 1), (0, 1, 0)],
        "ascending_south" => [(0, 1, 1), (0, -1, 0)],
        "ascending_east" => [(1, 0, 1), (-1, 0, 0)],
        "ascending_west" => [(-1, 0, 1), (1, 0, 0)],
        "north_east" => [(0, -1, 0), (1, 0, 0)],
        "north_west" => [(0, -1, 0), (-1, 0, 0)],
        "south_east" => [(0, 1, 0), (1, 0, 0)],
        "south_west" => [(0, 1, 0), (-1, 0, 0)],
        _ => [(0, -1, 0), (0, 1, 0)],
    }
}

fn slope(r: &Rail) -> bool {
    r.shape.starts_with("ascending_")
}

/// The rail cell the end `e` of the rail at `c` leads to: level with the end, or one lower (a
/// slope going down from it), and only if that rail has an end pointing back.
fn next(rails: &HashMap<[i32; 3], Rail>, c: [i32; 3], e: (i32, i32, i32)) -> Option<[i32; 3]> {
    let (dx, dz, up) = e;
    let level = [c[0] + dx, c[1] + up, c[2] + dz];
    for n in [level, [level[0], level[1] - 1, level[2]]] {
        // (connected: n's end toward c at the height of c's end toward n)
        if let Some(r) = rails.get(&n)
            && ends(&r.shape).iter().any(|&(ex, ez, eu)| n[0] + ex == c[0] && n[2] + ez == c[2] && n[1] + eu == c[1] + up)
        {
            return Some(n);
        }
    }
    None
}

/// Walking away from the line from `s` (its neighbour across the line `other`): the distance to
/// the first climb (the track going up away from the line), drop (going down), peak (a flat cell
/// whose neighbours are both slopes going down from it) and powered rail; `s` itself is 0.
fn walk(rails: &HashMap<[i32; 3], Rail>, s: [i32; 3], other: [i32; 3], depth: usize) -> [Option<usize>; 4] {
    let mut out = [None; 4];
    let (mut prev, mut cur) = (other, s);
    for d in 0..depth {
        let Some(r) = rails.get(&cur) else { break };
        let nexts: Vec<[i32; 3]> = ends(&r.shape).iter().filter_map(|&e| next(rails, cur, e)).collect();
        if r.block == "powered_rail" && out[3].is_none() {
            out[3] = Some(d);
        }
        if !slope(r) && nexts.len() == 2 && nexts.iter().all(|n| n[1] == cur[1] - 1 && rails.get(n).is_some_and(slope)) && out[2].is_none() {
            out[2] = Some(d);
        }
        let Some(a) = nexts.iter().copied().find(|n| *n != prev) else { break };
        // (this cell's end toward the next one: a slope's high end there climbs away from the
        // line, its low end there drops; a flat cell drops when the next is a slope below)
        let up_ahead = ends(&r.shape).iter().find(|&&(ex, ez, _)| cur[0] + ex == a[0] && cur[2] + ez == a[2]).map_or(0, |e| e.2);
        let rises = slope(r) && up_ahead == 1;
        let falls = (slope(r) && up_ahead == 0) || (!slope(r) && a[1] < cur[1]);
        if rises && out[0].is_none() {
            out[0] = Some(d);
        }
        if falls && out[1].is_none() {
            out[1] = Some(d);
        }
        prev = cur;
        cur = a;
    }
    out
}

/// Every crossing of a chunk line by the design's rails (world = design + `offset`), with both
/// sides' cells and the distances along the track; the rule's verdict per crossing.
pub fn report(blocks: &std::collections::BTreeMap<[i32; 3], String>, offset: [i32; 2], depth: usize) -> String {
    let rails: HashMap<[i32; 3], Rail> = blocks.iter().filter_map(|(c, s)| rail_of(s).map(|r| (*c, r))).collect();
    let chunk = |c: [i32; 3]| ((c[0] + offset[0]).div_euclid(16), (c[2] + offset[1]).div_euclid(16));
    let mut seen: BTreeSet<([i32; 3], [i32; 3])> = BTreeSet::new();
    let mut lines = Vec::new();
    let mut bad = 0usize;
    let mut cells: Vec<[i32; 3]> = rails.keys().copied().collect();
    cells.sort_by_key(|c| (c[2], c[0], c[1]));
    for c in cells {
        let r = &rails[&c];
        for e in ends(&r.shape) {
            let Some(n) = next(&rails, c, e) else { continue };
            if chunk(c) == chunk(n) {
                continue;
            }
            let key = if c < n { (c, n) } else { (n, c) };
            if !seen.insert(key) {
                continue;
            }
            let (a, b) = key;
            let line = if chunk(a).0 != chunk(b).0 {
                let x = (a[0] + offset[0]).max(b[0] + offset[0]);
                format!("x {}|{} (world)", x - 1, x)
            } else {
                let z = (a[2] + offset[1]).max(b[2] + offset[1]);
                format!("z {}|{} (world)", z - 1, z)
            };
            let side = |s: [i32; 3], o: [i32; 3]| {
                let q = &rails[&s];
                let w = walk(&rails, s, o, depth);
                let d = |v: Option<usize>| v.map_or("-".to_string(), |v| v.to_string());
                (
                    format!("({},{},{}) {} {}{}; away: climb {}, drop {}, peak {}, powered {}", s[0], s[1], s[2], q.block, q.shape, if q.block == "powered_rail" || q.block == "detector_rail" { format!(" powered={}", q.powered) } else { String::new() }, d(w[0]), d(w[1]), d(w[2]), d(w[3])),
                    q.block == "rail" && !slope(q) && w[0].is_none_or(|v| v >= 2) && w[2].is_none_or(|v| v >= 1) && w[3].is_none_or(|v| v >= 1),
                )
            };
            let (sa, oka) = side(a, b);
            let (sb, okb) = side(b, a);
            let ok = oka && okb;
            if !ok {
                bad += 1;
            }
            lines.push(format!("border {line}: {sa} | {sb}: {}", if ok { "plain flat both sides, no climb within 2 (the rule holds)" } else { "the rule fails" }));
        }
    }
    format!("chunk-line crossings by the design's rails (world = design + {},{}; the rule: plain unpowered flat cells on both sides, no climb within 2 cells, no peak or powered cell at the line): {} crossings, {} failing\n{}", offset[0], offset[1], lines.len(), bad, lines.join("\n"))
}
