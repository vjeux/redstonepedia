//! (9fc3155f, ironspec Nov 12 00:11: the static push-range scan) Every cell where a cart rests or
//! creeps against every rail cell a cart passes on another track: a pair within push range is
//! flagged. A cart's box: 0.98 wide (x and z), 0.7 tall, its floor 0.0625 over the rail (a slope's
//! cart anywhere between its two ends, so its box spans the slope's rise too); the resting cart's
//! box inflated 0.2 in x and z (the push test); the y ranges must overlap. Tracks: rail cells
//! joined where each one's shape exits toward the other (a level or a one-up neighbour). Rest or
//! creep cells: the `--rest` cells (the queue's slopes, holds, D cells, the queue cell) and, unless
//! turned off, every unpowered powered rail (a brake).

use redsim::entity::mob::farm::FarmSpec;
use redsim::pos::BlockPos;
use std::collections::HashMap;

/// A rail state's exits (dx, dz, rises): a slope's high side rises one.
fn exits(state: &str) -> Vec<(i32, i32, bool)> {
    let shape = state.split("shape=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("")).unwrap_or("north_south");
    match shape {
        "north_south" => vec![(0, -1, false), (0, 1, false)],
        "east_west" => vec![(1, 0, false), (-1, 0, false)],
        "ascending_north" => vec![(0, -1, true), (0, 1, false)],
        "ascending_south" => vec![(0, 1, true), (0, -1, false)],
        "ascending_east" => vec![(1, 0, true), (-1, 0, false)],
        "ascending_west" => vec![(-1, 0, true), (1, 0, false)],
        "north_east" => vec![(0, -1, false), (1, 0, false)],
        "north_west" => vec![(0, -1, false), (-1, 0, false)],
        "south_east" => vec![(0, 1, false), (1, 0, false)],
        "south_west" => vec![(0, 1, false), (-1, 0, false)],
        _ => vec![(0, -1, false), (0, 1, false)],
    }
}

/// A cart's box on rail cell `c` (x0, x1, y0, y1, z0, z1).
fn cart_box(c: BlockPos, state: &str) -> [f64; 6] {
    let slope = state.contains("ascending");
    let y0 = c.y as f64 + 0.0625;
    let y1 = y0 + 0.7 + if slope { 1.0 } else { 0.0 };
    [c.x as f64 + 0.01, c.x as f64 + 0.99, y0, y1, c.z as f64 + 0.01, c.z as f64 + 0.99]
}

/// The scan: (the flagged pairs as lines, the rest cells checked, the tracks).
pub fn scan(spec: &FarmSpec, rests: &[BlockPos], auto: bool) -> (Vec<String>, usize, usize) {
    let rails: HashMap<BlockPos, String> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail")).map(|(p, b)| (*p, b.to_string())).collect();
    // (the tracks: union-find over the rail cells joined by their exits)
    let cells: Vec<BlockPos> = rails.keys().copied().collect();
    let idx: HashMap<BlockPos, usize> = cells.iter().enumerate().map(|(i, c)| (*c, i)).collect();
    let mut parent: Vec<usize> = (0..cells.len()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let n = p[j];
            p[j] = r;
            j = n;
        }
        r
    }
    for (i, c) in cells.iter().enumerate() {
        for (dx, dz, rises) in exits(&rails[c]) {
            for dy in if rises { vec![1] } else { vec![0, -1] } {
                let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                let Some(&j) = idx.get(&n) else { continue };
                let back = exits(&rails[&n]).iter().any(|(ex, ez, _)| *ex == -dx && *ez == -dz);
                if back {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                }
            }
        }
    }
    let mut rest_cells: Vec<BlockPos> = rests.to_vec();
    if auto {
        // (a brake: an unpowered powered rail, where a cart rests; the queue's slopes, holds and
        // the other creep cells come in as --rest; a detector is passed over, not rested on)
        for (c, s) in &rails {
            if s.contains("powered_rail") && s.contains("powered=false") {
                if !rest_cells.contains(c) {
                    rest_cells.push(*c);
                }
            }
        }
    }
    rest_cells.sort_by_key(|c| (c.y, c.z, c.x));
    let mut out = Vec::new();
    for r in &rest_cells {
        let Some(rs) = rails.get(r) else {
            out.push(format!("  {},{},{}: not a rail cell (skipped)", r.x, r.y, r.z));
            continue;
        };
        let ri = idx[r];
        let rt = find(&mut parent, ri);
        let mut rb = cart_box(*r, rs);
        rb[0] -= 0.2;
        rb[1] += 0.2;
        rb[4] -= 0.2;
        rb[5] += 0.2;
        for (p, ps) in &rails {
            if p == r {
                continue;
            }
            if (p.x - r.x).abs() > 2 || (p.z - r.z).abs() > 2 || (p.y - r.y).abs() > 2 {
                continue;
            }
            let pt = find(&mut parent, idx[p]);
            if pt == rt {
                continue;
            }
            let pb = cart_box(*p, ps);
            if rb[0] < pb[1] && pb[0] < rb[1] && rb[2] < pb[3] && pb[2] < rb[3] && rb[4] < pb[5] && pb[4] < rb[5] {
                out.push(format!("  PUSH {},{},{} (rests: {}) <- {},{},{} (another track: {})", r.x, r.y, r.z, short(rs), p.x, p.y, p.z, short(ps)));
            }
        }
    }
    let mut roots: Vec<usize> = (0..cells.len()).map(|i| find(&mut parent, i)).collect();
    roots.sort();
    roots.dedup();
    (out, rest_cells.len(), roots.len())
}

fn short(s: &str) -> String {
    s.trim_start_matches("minecraft:").split(",waterlogged").next().unwrap_or(s).to_string()
}

/// The rail-T scan (ironspec Nov 12 02:17 / 02:46 / 03:05, G1: a detector beside a plain-rail T
/// flips it; the game re-shapes a 3-connection plain rail only on a neighbour update whose cause
/// is a signal source, 26.3 RailBlock.updateState lines 22-23): every plain rail with 3 or more
/// potential connections (rails beside it at its level, one up or one down; slopes included), its
/// stored shape, every signal source whose update can reach it, each of which must be one of
/// `switches` (its declared switch): the source's own face neighbours (a detector rail's
/// checkPressed: updateNeighborsAt(pos, this), DetectorRailBlock lines 93-94 / 102-103); a
/// detector rail's block below and that block's face neighbours (updateNeighborsAt(pos.below(),
/// this), the same lines; its updatePowerToConnected, lines 115-122, passes the connected rail's
/// own block as the cause, not a source, so a connected T keeps its shape); dust's 2-block update
/// reach; the block a torch, a repeater, a comparator, a lever or a button strongly powers and that
/// block's face neighbours. Also the cells a cart entering by a free side is snapped toward (the
/// stored shape's exits), and its placement order for the build guide (after its two intended
/// arms, the stored shape's exits, before the rest: a T keeps its placed shape through restarts
/// and loads while no signal source's update reaches it). Lines and the undeclared count.
pub fn tees(spec: &FarmSpec, switches: &[BlockPos]) -> (Vec<String>, usize) {
    let all: HashMap<BlockPos, String> = spec.blocks.iter().map(|(p, b)| (*p, b.to_string())).collect();
    let is_rail = |p: &BlockPos| all.get(p).is_some_and(|s| s.contains("rail"));
    let source = |s: &str| {
        ["redstone_block", "detector_rail", "redstone_torch", "redstone_wall_torch", "lever", "_button", "observer", "target", "daylight_detector", "repeater", "comparator", "redstone_wire"].iter().any(|k| s.contains(k))
    };
    const N6: [(i32, i32, i32); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
    // (each cell an update with a signal source as its cause can reach, with its sources)
    let mut reach: HashMap<BlockPos, Vec<BlockPos>> = HashMap::new();
    let mut mark = |q: BlockPos, s: BlockPos, reach: &mut HashMap<BlockPos, Vec<BlockPos>>| {
        let v = reach.entry(q).or_default();
        if !v.contains(&s) {
            v.push(s);
        }
    };
    for (p, s) in all.iter().filter(|(_, s)| source(s)) {
        for (dx, dy, dz) in N6 {
            mark(BlockPos::new(p.x + dx, p.y + dy, p.z + dz), *p, &mut reach);
        }
        if s.contains("redstone_wire") {
            for dx in -2i32..=2 {
                for dy in -2i32..=2 {
                    for dz in -2i32..=2 {
                        if dx.abs() + dy.abs() + dz.abs() <= 2 {
                            mark(BlockPos::new(p.x + dx, p.y + dy, p.z + dz), *p, &mut reach);
                        }
                    }
                }
            }
        }
        // (the blocks it strongly powers: a detector rail the block under it, a torch the block
        // over it, a repeater or comparator the blocks on its axis, a lever or button any side)
        let strong: Vec<BlockPos> = if s.contains("detector_rail") {
            vec![BlockPos::new(p.x, p.y - 1, p.z)]
        } else if s.contains("torch") {
            vec![BlockPos::new(p.x, p.y + 1, p.z)]
        } else if s.contains("repeater") || s.contains("comparator") {
            if s.contains("facing=east") || s.contains("facing=west") {
                vec![BlockPos::new(p.x + 1, p.y, p.z), BlockPos::new(p.x - 1, p.y, p.z)]
            } else {
                vec![BlockPos::new(p.x, p.y, p.z + 1), BlockPos::new(p.x, p.y, p.z - 1)]
            }
        } else if s.contains("lever") || s.contains("_button") {
            N6.iter().map(|(dx, dy, dz)| BlockPos::new(p.x + dx, p.y + dy, p.z + dz)).collect()
        } else {
            Vec::new()
        };
        for q in strong {
            for (dx, dy, dz) in N6 {
                mark(BlockPos::new(q.x + dx, q.y + dy, q.z + dz), *p, &mut reach);
            }
        }
    }
    let mut out = Vec::new();
    let mut undeclared = 0usize;
    let mut cells: Vec<&BlockPos> = all.iter().filter(|(_, s)| s.starts_with("minecraft:rail[")).map(|(p, _)| p).collect();
    cells.sort_by_key(|c| (c.y, c.z, c.x));
    for c in cells {
        let mut sides = Vec::new();
        let mut arms: Vec<(&str, BlockPos)> = Vec::new();
        for (dx, dz, name) in [(0, -1, "north"), (1, 0, "east"), (0, 1, "south"), (-1, 0, "west")] {
            if let Some(dy) = (-1..=1).find(|dy| is_rail(&BlockPos::new(c.x + dx, c.y + dy, c.z + dz))) {
                sides.push(name);
                arms.push((name, BlockPos::new(c.x + dx, c.y + dy, c.z + dz)));
            }
        }
        if sides.len() < 3 {
            continue;
        }
        let shape = all[c].split("shape=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("")).unwrap_or("").to_string();
        let mut srcs = Vec::new();
        if let Some(v) = reach.get(c) {
            for n in v {
                let ok = switches.contains(n);
                if !ok {
                    undeclared += 1;
                }
                srcs.push(format!("{},{},{} {}{}", n.x, n.y, n.z, short(&all[n]).split('[').next().unwrap_or(""), if ok { " (declared)" } else { " UNDECLARED" }));
            }
        }
        let exits = exits(&all[c]);
        let intended = |name: &str| {
            exits.iter().any(|(dx, dz, _)| match name {
                "north" => (*dx, *dz) == (0, -1),
                "east" => (*dx, *dz) == (1, 0),
                "south" => (*dx, *dz) == (0, 1),
                _ => (*dx, *dz) == (-1, 0),
            })
        };
        let free: Vec<&str> = sides.iter().copied().filter(|s| !intended(s)).collect();
        let snap: Vec<String> = exits.iter().map(|(dx, dz, _)| format!("{},{},{}", c.x + dx, c.y, c.z + dz)).collect();
        let before: Vec<String> = arms.iter().filter(|(n, _)| intended(n)).map(|(_, p)| format!("{},{},{}", p.x, p.y, p.z)).collect();
        let after: Vec<String> = arms.iter().filter(|(n, _)| !intended(n)).map(|(_, p)| format!("{},{},{}", p.x, p.y, p.z)).collect();
        out.push(format!(
            "  T {},{},{} (stored {shape}; rails on its {} sides: {}): signal-source updates reaching it: {}; a cart by its free side ({}) is snapped toward {}; placement order: after {} (its intended arms), before {}",
            c.x,
            c.y,
            c.z,
            sides.len(),
            sides.join(", "),
            if srcs.is_empty() { "none".to_string() } else { srcs.join(", ") },
            if free.is_empty() { "none".to_string() } else { free.join(", ") },
            snap.join(" / "),
            before.join(" and "),
            after.join(" and ")
        ));
    }
    (out, undeclared)
}
