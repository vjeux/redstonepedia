//! (9fc3155f, ironspec Nov 12 00:11: the static push-range scan) Every cell where a cart rests or
//! creeps against every rail cell a cart passes on another track: a pair within push range is
//! flagged. A cart's box: 0.98 wide (x and z), 0.7 tall, its floor 0.0625 over the rail (a slope's
//! cart anywhere between its two ends, so its box spans the slope's rise too); the resting cart's
//! box inflated 0.2 in x and z (the push test) and 0.5 each way along its rail's axis (a braked
//! cart stops anywhere along its cell); the y ranges must overlap. Tracks: rail cells
//! joined where each one's shape exits toward the other (a level or a one-up neighbour); a rest
//! cell's own route (exempt) is what it reaches without entering a junction (a rail with 3+
//! potential connections, of any kind: a straight powered, detector or activator rail with three
//! rail neighbours counts too, conservatively, since a cart entering from its side is snapped onto
//! it; routes a junction joins are different tracks, ironspec Nov 12 05:52, the K contact). A
//! junction declared `own-route-junctions: x,y,z=REASON` (ironspec Nov 12 07:52, the S-bend's c3)
//! joins a rest cell's own route when its third arm is that rest cell and its shape never connects
//! to it and no source reaches it (both checked, refused otherwise), REASON the bench's evidence
//! that no other route's cart enters it, printed beside it. A curve's passing cart is also boxed
//! along the chord between its exit-edge midpoints (moveAlongTrack's projection of a side entry;
//! `--chord-speed` drops the chord's ends a side-entering cart never sits at after its first move).
//! A `single-occupant: x,y,z+...=REASON` group's pairs print as declared, not counted (ironspec Nov
//! 12 08:47). `--rest-pos` checks a resting cart at an exact point (inflated 0.2 only). Rest or
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

/// A passing cart's boxes on a track cell (G1's CHANGES 2 on ff3d181e6): a curve's cart is
/// projected by moveAlongTrack onto the chord between its two exit-edge midpoints (a cart entering
/// from a side lands at an end of it, a north_east curve entered from the west at its north edge's
/// midpoint), so a curve gives its cell's box and one box at each of 5 points along that chord.
/// The push test sees a cart's box after its move (AbstractMinecart.tick pushes after
/// moveAlongTrack), and a ridden cart moves 0.75 of its speed a tick along the chord: with
/// `chord_speed` V (the least speed of a cart entering a curve from its side) the chord's points
/// run from t0 = 0.75 V / 0.7071 to 1 - t0; V 0 keeps the whole chord, both midpoints included
/// (tick-exact's official crossing 1531189749067418: margin +0.107 at a 0.10 napper arrival with
/// 0.29 through-carts, -0.020 for a 0.10 through-cart). Any other rail gives the cell's box.
fn track_boxes(c: BlockPos, state: &str, chord_speed: f64) -> Vec<[f64; 6]> {
    let curve = ["north_east", "north_west", "south_east", "south_west"].iter().any(|s| state.contains(&format!("shape={s}")));
    if !curve {
        return vec![cart_box(c, state)];
    }
    let ex = exits(state);
    let (a, b) = (ex[0], ex[1]);
    let (ax, az) = (c.x as f64 + 0.5 + a.0 as f64 * 0.5, c.z as f64 + 0.5 + a.1 as f64 * 0.5);
    let (bx, bz) = (c.x as f64 + 0.5 + b.0 as f64 * 0.5, c.z as f64 + 0.5 + b.1 as f64 * 0.5);
    let y0 = c.y as f64 + 0.0625;
    let t0 = (0.75 * chord_speed / std::f64::consts::FRAC_1_SQRT_2).min(0.5);
    let mut v = vec![cart_box(c, state)];
    v.extend((0..=4).map(|k| {
        let t = t0 + (1.0 - 2.0 * t0) * k as f64 / 4.0;
        let (x, z) = (ax + (bx - ax) * t, az + (bz - az) * t);
        [x - 0.49, x + 0.49, y0, y0 + 0.7, z - 0.49, z + 0.49]
    }));
    v
}

fn overlap(a: &[f64; 6], b: &[f64; 6]) -> bool {
    a[0] < b[1] && b[0] < a[1] && a[2] < b[3] && b[2] < a[3] && a[4] < b[5] && b[4] < a[5]
}

/// The scan: (the flagged pairs as lines, the rest cells checked, the tracks, the declared
/// single-occupant pairs among the flagged). `own_decl`: the declared own-route junctions
/// (`own-route-junctions: x,y,z=REASON`, ironspec Nov 12 07:52, c3's ruling): a junction cell joins
/// a rest cell's own route when (a) every potential connection off its stored shape is that rest
/// cell, exactly one (its third arm is no other route), (b) its placed shape never connects to the
/// rest cell and no signal-source update reaches it (never switched), and (c) no other route's
/// cart ever enters it, a bench's evidence that the declaration's REASON carries and the scan
/// prints beside it. (a) and (b) are checked here and a declaration failing either is refused (an
/// error); an accepted junction is the rest's own route itself, not a way through it. `single`:
/// declared single-occupant groups (`single-occupant: x,y,z+x,y,z+...=REASON`, ironspec Nov 12
/// 08:47, 7b90c3f1's napper hold): a pair whose rest and track cells are both in one group is
/// printed as declared, with the group's REASON (the bench's numbers), and not counted. `rest_pos`:
/// resting carts at exact positions (the bench's rest points): each box inflated 0.2 in x and z
/// only, against every track cell off its cell's own route.
pub fn scan(spec: &FarmSpec, rests: &[BlockPos], auto: bool, own_decl: &[(BlockPos, String)], single: &[(Vec<BlockPos>, String)], rest_pos: &[[f64; 3]], chord_speed: f64, bridges: &[Bridge], speeds: &HashMap<String, (f64, f64)>) -> Result<(Vec<String>, usize, usize, usize, usize), String> {
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
    // (ironspec Nov 12 05:52, the K contact: routes joined by a junction (a rail with 3 or more
    // potential connections: rails beside it at its level, one up or one down) are different
    // tracks; only the rails reachable from the rest cell without entering a junction cell are its
    // own route and exempt)
    let is_rail = |p: &BlockPos| rails.contains_key(p);
    let junction = |c: &BlockPos| [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().filter(|(dx, dz)| (-1..=1).any(|dy| is_rail(&BlockPos::new(c.x + dx, c.y + dy, c.z + dz)))).count() >= 3;
    let joined = |c: &BlockPos| -> Vec<BlockPos> {
        let mut v = Vec::new();
        for (dx, dz, rises) in exits(&rails[c]) {
            for dy in if rises { vec![1] } else { vec![0, -1] } {
                let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if rails.get(&n).is_some_and(|s| exits(s).iter().any(|(ex, ez, _)| *ex == -dx && *ez == -dz)) {
                    v.push(n);
                }
            }
        }
        v
    };
    let mut out = Vec::new();
    // (the declared own-route junctions, checked: (a) a rest cell among its potential connections,
    // (b) its exits never toward that rest cell, no signal-source update reaching it)
    let reach = sources_reaching(spec);
    let mut own_j: Vec<(BlockPos, BlockPos)> = Vec::new();
    for (j, reason) in own_decl {
        let Some(js) = rails.get(j) else {
            return Err(format!("own-route junction {},{},{}: not a rail cell", j.x, j.y, j.z));
        };
        if !junction(j) {
            return Err(format!("own-route junction {},{},{}: not a junction (fewer than 3 potential connections)", j.x, j.y, j.z));
        }
        // ((a), G1's CHANGES 2: every potential connection off the stored shape's two exits must
        // be the rest cell, and exactly one)
        let off: Vec<(i32, i32, BlockPos)> = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .iter()
            .filter(|(dx, dz)| !exits(js).iter().any(|(ex, ez, _)| (*ex, *ez) == (*dx, *dz)))
            .filter_map(|(dx, dz)| (-1..=1).map(|dy| BlockPos::new(j.x + dx, j.y + dy, j.z + dz)).find(|p| is_rail(p)).map(|p| (*dx, *dz, p)))
            .collect();
        let off_rest: Vec<&(i32, i32, BlockPos)> = off.iter().filter(|(_, _, p)| rest_cells.contains(p)).collect();
        let off_other: Vec<String> = off.iter().filter(|(_, _, p)| !rest_cells.contains(p)).map(|(_, _, p)| format!("{},{},{} {}", p.x, p.y, p.z, short(&rails[p]))).collect();
        if !off_other.is_empty() {
            return Err(format!("own-route junction {},{},{}: (a) fails: a potential connection off its shape is not the rest cell: {}", j.x, j.y, j.z, off_other.join(", ")));
        }
        if off_rest.len() != 1 {
            return Err(format!("own-route junction {},{},{}: (a) fails: {} rest cells among its potential connections off its shape (exactly one required)", j.x, j.y, j.z, off_rest.len()));
        }
        let (dx, dz, r) = *off_rest[0];
        if exits(js).iter().any(|(ex, ez, _)| (*ex, *ez) == (dx, dz)) {
            return Err(format!("own-route junction {},{},{}: (b) fails: its shape {} connects to the rest cell {},{},{}", j.x, j.y, j.z, short(js), r.x, r.y, r.z));
        }
        if let Some(srcs) = reach.get(j).filter(|v| !v.is_empty()) {
            let s: Vec<String> = srcs.iter().map(|n| format!("{},{},{}", n.x, n.y, n.z)).collect();
            return Err(format!("own-route junction {},{},{}: (b) fails: signal-source updates reach it from {}", j.x, j.y, j.z, s.join(", ")));
        }
        out.push(format!(
            "  OWN-ROUTE JUNCTION {},{},{} of the rest {},{},{}: (a) its third arm is the rest cell; (b) stored {}, no exit toward the rest, no signal-source update reaches it; (c) {reason}",
            j.x,
            j.y,
            j.z,
            r.x,
            r.y,
            r.z,
            short(js)
        ));
        own_j.push((*j, r));
    }
    // (the declared single-occupant groups, checked: every cell a rail cell, one of them a rest)
    for (g, _) in single {
        for c in g {
            if !rails.contains_key(c) {
                return Err(format!("single-occupant {},{},{}: not a rail cell", c.x, c.y, c.z));
            }
        }
        if !g.iter().any(|c| rest_cells.contains(c)) {
            let s: Vec<String> = g.iter().map(|c| format!("{},{},{}", c.x, c.y, c.z)).collect();
            return Err(format!("single-occupant {}: no rest cell among its cells", s.join("+")));
        }
    }
    let group_of = |a: &BlockPos, b: &BlockPos| single.iter().find(|(g, _)| g.contains(a) && g.contains(b)).map(|(_, why)| why.clone());
    let mut declared = 0usize;
    let mut bridged = 0usize;
    // (a rail cell's own route: what it reaches without entering a junction, plus its declared
    // own-route junctions)
    let own_of = |r: &BlockPos| -> std::collections::HashSet<BlockPos> {
        let mut own: std::collections::HashSet<BlockPos> = std::collections::HashSet::new();
        own.insert(*r);
        let mut todo = vec![*r];
        while let Some(c) = todo.pop() {
            for n in joined(&c) {
                if !junction(&n) && own.insert(n) {
                    todo.push(n);
                }
            }
        }
        for (j, jr) in &own_j {
            if jr == r {
                own.insert(*j);
            }
        }
        own
    };
    for r in &rest_cells {
        let Some(rs) = rails.get(r) else {
            out.push(format!("  {},{},{}: not a rail cell (skipped)", r.x, r.y, r.z));
            continue;
        };
        let ri = idx[r];
        let rt = find(&mut parent, ri);
        let own = own_of(r);
        let mut rb = cart_box(*r, rs);
        rb[0] -= 0.2;
        rb[1] += 0.2;
        rb[4] -= 0.2;
        rb[5] += 0.2;
        // (a braked cart stops anywhere along its cell: its box grown 0.5 each way along the
        // rail's axis, G1 Nov 12 03:29)
        for (dx, dz, _) in exits(rs) {
            if dx < 0 {
                rb[0] -= 0.5;
            } else if dx > 0 {
                rb[1] += 0.5;
            }
            if dz < 0 {
                rb[4] -= 0.5;
            } else if dz > 0 {
                rb[5] += 0.5;
            }
        }
        for (p, ps) in &rails {
            if p == r {
                continue;
            }
            if (p.x - r.x).abs() > 2 || (p.z - r.z).abs() > 2 || (p.y - r.y).abs() > 2 {
                continue;
            }
            if own.contains(p) {
                continue;
            }
            let pt = find(&mut parent, idx[p]);
            let boxes = track_boxes(*p, ps, chord_speed);
            let hit: Vec<usize> = (0..boxes.len()).filter(|k| overlap(&rb, &boxes[*k])).collect();
            if !hit.is_empty() {
                let at = if boxes.len() > 1 { format!(", boxes {:?} of 0..5 (0 the cell's, 1..5 its chord's points from t0 {:.3})", hit, (0.75 * chord_speed / std::f64::consts::FRAC_1_SQRT_2).min(0.5)) } else { String::new() };
                let kind = if junction(p) { "the junction" } else if pt == rt { "the same track beyond a junction" } else { "another track" };
                if let Some(why) = group_of(r, p) {
                    declared += 1;
                    out.push(format!("  SINGLE-OCCUPANT (declared) {},{},{} (rests: {}) <- {},{},{} ({kind}: {}{at}): {why}", r.x, r.y, r.z, short(rs), p.x, p.y, p.z, short(ps)));
                } else if let Some(br) = bridges.iter().find(|br| (br.a == *r && br.b == *p) || (br.a == *p && br.b == *r)) {
                    // (a bridged pair: accepted only while the cells within 2 of both cells hash as in
                    // the bridged composition and the design's own speed prints lie in its ranges)
                    let h = neighborhood_hash(spec, br.a, br.b);
                    let mut fails: Vec<String> = Vec::new();
                    if h != br.hash {
                        fails.push(format!("VOID: the cells within 2 changed (hash {h}, bridged {})", br.hash));
                    }
                    for (name, lo, hi) in &br.ranges {
                        match speeds.get(name) {
                            None => fails.push(format!("no `speeds:` print for {name}")),
                            Some((dlo, dhi)) if dlo > dhi => fails.push(format!("{name} {dlo}..{dhi}: the design's print has lo above hi")),
                            Some((dlo, dhi)) if *dlo < *lo - 1e-9 || *dhi > *hi + 1e-9 => fails.push(format!("{name} {dlo}..{dhi} outside the bridged {lo}..{hi}")),
                            Some(_) => {}
                        }
                    }
                    if fails.is_empty() {
                        bridged += 1;
                        out.push(format!("  BRIDGED {},{},{} (rests: {}) <- {},{},{} ({kind}: {}{at}): bridged by {}, margin {} (hash {h}; {})", r.x, r.y, r.z, short(rs), p.x, p.y, p.z, short(ps), br.by, br.margin, br.ranges.iter().map(|(n, lo, hi)| format!("{n} {lo}..{hi}: the design's {}", speeds.get(n).map_or("?".to_string(), |(a, b)| format!("{a}..{b}")))).collect::<Vec<_>>().join(", ")));
                    } else {
                        out.push(format!("  PUSH {},{},{} (rests: {}) <- {},{},{} ({kind}: {}{at}): open, the bridge by {} fails: {}", r.x, r.y, r.z, short(rs), p.x, p.y, p.z, short(ps), br.by, fails.join("; ")));
                    }
                } else {
                    out.push(format!("  PUSH {},{},{} (rests: {}) <- {},{},{} ({kind}: {}{at})", r.x, r.y, r.z, short(rs), p.x, p.y, p.z, short(ps)));
                }
            }
        }
    }
    // (the rest points: a resting cart's exact position, its box inflated 0.2 in x and z only)
    for rp in rest_pos {
        let base = BlockPos::new(rp[0].floor() as i32, (rp[1] - 0.0625 + 1e-6).floor() as i32, rp[2].floor() as i32);
        let cell = if rails.contains_key(&base) { base } else { BlockPos::new(base.x, base.y - 1, base.z) };
        let Some(cs) = rails.get(&cell) else {
            out.push(format!("  rest point {:.3},{:.3},{:.3}: not on a rail cell (skipped)", rp[0], rp[1], rp[2]));
            continue;
        };
        let own = own_of(&cell);
        let rb = [rp[0] - 0.69, rp[0] + 0.69, rp[1], rp[1] + 0.7, rp[2] - 0.69, rp[2] + 0.69];
        let mut least = f64::MAX;
        let mut least_at = String::new();
        for (p, ps) in &rails {
            if own.contains(p) || (p.x - cell.x).abs() > 2 || (p.z - cell.z).abs() > 2 || (p.y - cell.y).abs() > 2 {
                continue;
            }
            for (k, b) in track_boxes(*p, ps, chord_speed).iter().enumerate() {
                // (the gap where the boxes share x and y: the least of the two z or x separations)
                if rb[2] < b[3] && b[2] < rb[3] {
                    let gx = (b[0] - rb[1]).max(rb[0] - b[1]);
                    let gz = (b[4] - rb[5]).max(rb[4] - b[5]);
                    let g = gx.max(gz);
                    if g < least {
                        least = g;
                        least_at = format!("{},{},{} ({}, box {k})", p.x, p.y, p.z, short(ps));
                    }
                }
                if overlap(&rb, b) {
                    out.push(format!("  PUSH rest point {:.3},{:.3},{:.3} (on {},{},{} {}) <- {},{},{} ({}, box {k})", rp[0], rp[1], rp[2], cell.x, cell.y, cell.z, short(cs), p.x, p.y, p.z, short(ps)));
                }
            }
        }
        out.push(format!(
            "  rest point {:.3},{:.3},{:.3} (on {},{},{}): the least gap between its push box and a passing cart's box off its own route: {}",
            rp[0],
            rp[1],
            rp[2],
            cell.x,
            cell.y,
            cell.z,
            if least == f64::MAX { "none within 2".to_string() } else { format!("{least:.3} to {least_at}") }
        ));
    }
    let mut roots: Vec<usize> = (0..cells.len()).map(|i| find(&mut parent, i)).collect();
    roots.sort();
    roots.dedup();
    Ok((out, rest_cells.len(), roots.len(), declared, bridged))
}

/// Every cell a signal-source update can reach, with its sources (the source set and each type's
/// update pattern from redsim's block model, `update_reach`).
fn sources_reaching(spec: &FarmSpec) -> HashMap<BlockPos, Vec<BlockPos>> {
    let mut reach: HashMap<BlockPos, Vec<BlockPos>> = HashMap::new();
    for (p, b) in &spec.blocks {
        if let Some((_, cells)) = update_reach(*p, b) {
            for q in cells {
                let v = reach.entry(q).or_default();
                if !v.contains(p) {
                    v.push(*p);
                }
            }
        }
    }
    reach
}

/// A bridged pair (ironspec Nov 12 08:58): a flagged pair carried by an official run's evidence.
/// `bridged-pairs: A=x,y,z B=x,y,z by=COMMIT/ARTIFACT hash=H name=lo..hi ... margin=M`: the pair's
/// two cells, the evidence, the hash of every cell within 2 of either cell in the bridged
/// composition (`--bridge-hash`), the ranges the run covers (each named as the design's `speeds:`
/// prints name them) and its least margin.
#[derive(Clone, Debug)]
pub struct Bridge {
    pub a: BlockPos,
    pub b: BlockPos,
    pub by: String,
    pub hash: String,
    pub ranges: Vec<(String, f64, f64)>,
    pub margin: String,
}

/// One `bridged-pairs:` entry.
pub fn parse_bridge(entry: &str) -> Result<Bridge, String> {
    let cell = |s: &str| -> Result<BlockPos, String> {
        let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>().map_err(|e| format!("bridged pair {entry}: {s}: {e}"))).collect::<Result<_, _>>()?;
        if v.len() == 3 { Ok(BlockPos::new(v[0], v[1], v[2])) } else { Err(format!("bridged pair {entry}: {s}: x,y,z")) }
    };
    let (mut a, mut b, mut by, mut hash, mut margin) = (None, None, None, None, None);
    let mut ranges = Vec::new();
    for tok in entry.split_whitespace() {
        let (k, v) = tok.split_once('=').ok_or_else(|| format!("bridged pair {entry}: {tok}: name=value"))?;
        match k {
            "A" => a = Some(cell(v)?),
            "B" => b = Some(cell(v)?),
            "by" => by = Some(v.to_string()),
            "hash" => hash = Some(v.to_string()),
            "margin" => margin = Some(v.to_string()),
            _ => {
                let (lo, hi) = v.split_once("..").ok_or_else(|| format!("bridged pair {entry}: {tok}: name=lo..hi"))?;
                let lo: f64 = lo.parse().map_err(|e| format!("bridged pair {entry}: {tok}: {e}"))?;
                let hi: f64 = hi.parse().map_err(|e| format!("bridged pair {entry}: {tok}: {e}"))?;
                if lo > hi {
                    return Err(format!("bridged pair {entry}: {tok}: lo above hi"));
                }
                ranges.push((k.to_string(), lo, hi));
            }
        }
    }
    let need = |o: Option<String>, n: &str| o.ok_or_else(|| format!("bridged pair {entry}: no {n}="));
    Ok(Bridge {
        a: a.ok_or_else(|| format!("bridged pair {entry}: no A="))?,
        b: b.ok_or_else(|| format!("bridged pair {entry}: no B="))?,
        by: need(by, "by")?,
        hash: need(hash, "hash")?,
        ranges,
        margin: need(margin, "margin")?,
    })
}

/// The hash of every cell within 2 (each axis) of either cell, in x, y, z order, air included
/// (FNV-1a 64 over "x,y,z=state;"): any geometry change near a bridged pair changes it.
pub fn neighborhood_hash(spec: &FarmSpec, a: BlockPos, b: BlockPos) -> String {
    let all: HashMap<BlockPos, String> = spec.blocks.iter().map(|(p, s)| (*p, s.to_string())).collect();
    let mut cells: Vec<BlockPos> = Vec::new();
    for c in [a, b] {
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    let p = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                    if !cells.contains(&p) {
                        cells.push(p);
                    }
                }
            }
        }
    }
    cells.sort_by_key(|p| (p.x, p.y, p.z));
    let mut h: u64 = 0xcbf29ce484222325;
    for p in cells {
        let s = format!("{},{},{}={};", p.x, p.y, p.z, all.get(&p).map_or("minecraft:air", |s| s.as_str()));
        for byte in s.bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}

/// A design's `speeds:` header prints (`speeds: name=lo..hi ...`, the benches' measured ranges).
pub fn header_speeds(text: &str) -> HashMap<String, (f64, f64)> {
    let mut m = HashMap::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("speeds:")) else { continue };
        for tok in body.split_whitespace() {
            if let Some((k, v)) = tok.split_once('=')
                && let Some((lo, hi)) = v.split_once("..")
                && let (Ok(lo), Ok(hi)) = (lo.parse::<f64>(), hi.parse::<f64>())
            {
                m.insert(k.to_string(), (lo, hi));
            }
        }
    }
    m
}

/// A design's `bridged-pairs:` entries (a line's entries split where a ";" is followed by "A=").
pub fn header_bridges(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix("bridged-pairs:")) else { continue };
        let mut cur = String::new();
        for part in body.split(';') {
            if part.trim_start().starts_with("A=") && !cur.trim().is_empty() {
                v.push(cur.trim().to_string());
                cur = String::new();
            }
            if !cur.is_empty() {
                cur.push(';');
            }
            cur.push_str(part);
        }
        if !cur.trim().is_empty() {
            v.push(cur.trim().to_string());
        }
    }
    v
}

/// The entries of a design's header lines for `key` (`own-route-junctions:` or
/// `single-occupant:`), anchored at a comment line's start (G1's note on ff3d181e6: as junction-check
/// anchors `placement-order-ts:`): a line's entries split where a ";" is followed by the next
/// x,y,z=, so a REASON may hold "; ".
pub fn header_entries(text: &str, key: &str) -> Vec<String> {
    let mut v = Vec::new();
    for l in text.lines() {
        let Some(body) = l.trim_start().strip_prefix('#').map(str::trim_start).and_then(|t| t.strip_prefix(key)) else { continue };
        let mut cur = String::new();
        for part in body.split(';') {
            let head = part.trim_start();
            let starts = head.split_once('=').is_some_and(|(c, _)| c.split(['+', ',']).count() % 3 == 0 && c.split(['+', ',']).all(|t| t.trim().parse::<i32>().is_ok()));
            if starts && !cur.trim().is_empty() {
                v.push(cur.trim().to_string());
                cur = String::new();
            }
            if !cur.is_empty() {
                cur.push(';');
            }
            cur.push_str(part);
        }
        if !cur.trim().is_empty() {
            v.push(cur.trim().to_string());
        }
    }
    v
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
    // (each cell a signal-source update can reach, with its sources: the source set and each
    // type's update pattern from redsim's block model, `update_reach`)
    let reach = sources_reaching(spec);
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
        // (RailBlock.updateState re-shapes only at exactly 3 potential connections: a 4-way
        // crossing keeps its placed shape on every update, so its sources are listed, not counted)
        let four = sides.len() >= 4;
        let shape = all[c].split("shape=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("")).unwrap_or("").to_string();
        let mut srcs = Vec::new();
        if let Some(v) = reach.get(c) {
            for n in v {
                let ok = switches.contains(n);
                if !ok && !four {
                    undeclared += 1;
                }
                srcs.push(format!("{},{},{} {}{}", n.x, n.y, n.z, short(&all[n]).split('[').next().unwrap_or(""), if ok { " (declared)" } else if four { " (4-way: not counted)" } else { " UNDECLARED" }));
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
            "  T {},{},{} (stored {shape}; rails on its {} sides: {}{}): signal-source updates reaching it: {}; a cart by its free side ({}) is snapped toward {}; placement order: after {} (its intended arms), before {}",
            c.x,
            c.y,
            c.z,
            sides.len(),
            sides.join(", "),
            if four { "; a 4-way crossing, re-shaped only at placement" } else { "" },
            if srcs.is_empty() { "none".to_string() } else { srcs.join(", ") },
            if free.is_empty() { "none".to_string() } else { free.join(", ") },
            snap.join(" / "),
            before.join(" and "),
            after.join(" and ")
        ));
    }
    (out, undeclared)
}

/// The 6 face neighbours of a cell.
fn n6(c: BlockPos) -> Vec<BlockPos> {
    use redsim::pos::Dir;
    [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East].iter().map(|d| c.rel(*d)).collect()
}

/// Every cell within Manhattan distance 2 of a cell (itself included).
fn manhattan2(c: BlockPos) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for dx in -2i32..=2 {
        for dy in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx.abs() + dy.abs() + dz.abs() <= 2 {
                    v.push(BlockPos::new(c.x + dx, c.y + dy, c.z + dz));
                }
            }
        }
    }
    v
}

/// A signal source's type name (26.3 `isSignalSource`, redsim's block model: behavior.rs
/// is_signal_source and the rails' kind_is_signal_source together) and the cells its own
/// neighbour updates reach when it switches; None for a block that is no signal source. Each
/// type's 26.3 pattern (a neighbour update reaches a cell when the cell is updated; a plain rail
/// re-shapes at exactly 3 potential connections when the update's cause is a source):
/// redstone dust (updateNeighborsOfNeighboringWires and its own: Manhattan 2); a torch, standing or
/// on a wall (RedstoneTorchBlock.notifyNeighbors: updateNeighborsAt of each of its 6 face
/// neighbours, so Manhattan 2); a lever or a button (updateNeighbours: updateNeighborsAt(pos) and at
/// the block it hangs on); a repeater, a comparator or an observer (updateNeighborsInFront:
/// neighborChanged at its output block, updateNeighborsAtExceptFromFacing there, so that block and
/// its other 5 neighbours); a tripwire hook or a lightning rod (updateNeighborsAt(pos) and at the
/// block behind it, its support); a detector rail, a pressure plate, a sculk sensor or a trapped
/// chest (updateNeighborsAt(pos) and at pos.below()); a redstone block, a target, a daylight
/// detector or a jukebox (updateNeighborsAt(pos)); a sloped detector rail also the faces of the
/// cell above it (BaseRailBlock.affectNeighborsAfterRemoval on its POWERED flip).
pub fn update_reach(p: BlockPos, b: &redsim::block::Block) -> Option<(&'static str, Vec<BlockPos>)> {
    use redsim::block::{Block, Face};
    use redsim::pos::Dir;
    let below = |q: BlockPos| [n6(q), n6(q.rel(Dir::Down))].concat();
    Some(match *b {
        Block::Wire { .. } => ("redstone dust", manhattan2(p)),
        Block::Torch { .. } => ("redstone torch", manhattan2(p)),
        Block::WallTorch { .. } => ("redstone wall torch", manhattan2(p)),
        Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => {
            let conn = match face {
                Face::Floor => Dir::Up,
                Face::Ceiling => Dir::Down,
                Face::Wall => facing,
            };
            (if matches!(b, Block::Lever { .. }) { "lever" } else { "button" }, [n6(p), n6(p.rel(conn.opposite()))].concat())
        }
        Block::Repeater { facing, .. } | Block::Comparator { facing, .. } | Block::Observer { facing, .. } => {
            let front = p.rel(facing.opposite());
            let mut v = vec![front];
            v.extend(n6(front).into_iter().filter(|c| *c != front.rel(facing)));
            (
                match b {
                    Block::Repeater { .. } => "repeater",
                    Block::Comparator { .. } => "comparator",
                    _ => "observer",
                },
                v,
            )
        }
        Block::TripwireHook { facing, .. } => ("tripwire hook", [n6(p), n6(p.rel(facing.opposite()))].concat()),
        Block::LightningRod { facing, .. } => ("lightning rod", [n6(p), n6(p.rel(facing.opposite()))].concat()),
        // (a detector rail: checkPressed's updateNeighborsAt(pos) and (pos.below()); and every state
        // change (its POWERED flip) runs BaseRailBlock.affectNeighborsAfterRemoval (lines 117-128):
        // a straight rail updates pos and pos.below() again, a slope also pos.above())
        Block::DetectorRail { .. } => (
            "detector rail",
            if b.to_string().contains("ascending") { [below(p), n6(p.rel(Dir::Up))].concat() } else { below(p) },
        ),
        Block::Plate { .. } => ("pressure plate", below(p)),
        Block::Sculk { .. } => ("sculk sensor", below(p)),
        Block::RedstoneBlock => ("redstone block", n6(p)),
        Block::Target { .. } => ("target", n6(p)),
        Block::DaylightDetector { .. } => ("daylight detector", n6(p)),
        Block::Jukebox { .. } => ("jukebox", n6(p)),
        Block::Terrain(id) if redsim::terrain::blocks::props(id).signal => ("signal source (terrain)", below(p)),
        _ => return None,
    })
}

/// A signal source's power out (26.3 getSignal / getDirectSignal): the cells it weakly powers
/// and the blocks it strongly powers (a conductor among them then powers its own face
/// neighbours); None for a block that is no signal source.
pub fn power_out(p: BlockPos, b: &redsim::block::Block) -> Option<(Vec<BlockPos>, Vec<BlockPos>)> {
    use redsim::block::{Block, Face};
    use redsim::pos::Dir;
    Some(match *b {
        // (dust: into the block under it and the sides it points to; direct the same)
        Block::Wire { .. } => {
            let v = vec![p.rel(Dir::Down), p.rel(Dir::North), p.rel(Dir::South), p.rel(Dir::West), p.rel(Dir::East)];
            (v.clone(), v)
        }
        // (a torch: every side but its support; strongly the block over it)
        Block::Torch { .. } => (n6(p).into_iter().filter(|c| *c != p.rel(Dir::Down)).collect(), vec![p.rel(Dir::Up)]),
        Block::WallTorch { facing, .. } => (n6(p).into_iter().filter(|c| *c != p.rel(facing.opposite())).collect(), vec![p.rel(Dir::Up)]),
        // (a lever or button: every side; strongly the block it hangs on)
        Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => {
            let conn = match face {
                Face::Floor => Dir::Up,
                Face::Ceiling => Dir::Down,
                Face::Wall => facing,
            };
            (n6(p), vec![p.rel(conn.opposite())])
        }
        // (a diode or an observer: its output block, weakly and strongly)
        Block::Repeater { facing, .. } | Block::Comparator { facing, .. } | Block::Observer { facing, .. } => (vec![p.rel(facing.opposite())], vec![p.rel(facing.opposite())]),
        // (a hook or a rod: every side; strongly the block behind it)
        Block::TripwireHook { facing, .. } | Block::LightningRod { facing, .. } => (n6(p), vec![p.rel(facing.opposite())]),
        // (a detector rail, a plate, a sculk sensor, a trapped chest: every side; strongly the block under it)
        Block::DetectorRail { .. } | Block::Plate { .. } | Block::Sculk { .. } => (n6(p), vec![p.rel(Dir::Down)]),
        Block::Terrain(id) if redsim::terrain::blocks::props(id).signal => (n6(p), vec![p.rel(Dir::Down)]),
        // (a redstone block, a target, a daylight detector: every side, no strong power)
        Block::RedstoneBlock | Block::Target { .. } | Block::DaylightDetector { .. } => (n6(p), Vec::new()),
        _ => return None,
    })
}

/// The rail-T scan's positive controls, one per source type (G1, Nov 12 03:29): a plain rail
/// with 3 potential connections at the origin, the source placed so the T is in its update reach
/// (flagged) and moved one step so it is not (not flagged).
pub fn tee_controls() -> Vec<String> {
    use redsim::block::Block;
    let parse = |s: &str| Block::parse(s).ok();
    let rails = [
        (BlockPos::new(0, 0, 0), "minecraft:rail[shape=north_south]"),
        (BlockPos::new(0, 0, -1), "minecraft:rail[shape=north_south]"),
        (BlockPos::new(0, 0, 1), "minecraft:rail[shape=north_south]"),
        (BlockPos::new(1, 0, 0), "minecraft:rail[shape=east_west]"),
    ];
    // (type, the source in reach, its cell; the source out of reach, its cell)
    let cases: [(&str, &str, [i32; 3], &str, [i32; 3]); 18] = [
        ("sloped detector rail (the cell above it)", "minecraft:detector_rail[powered=false,shape=ascending_east]", [0, -2, 0], "minecraft:detector_rail[powered=false,shape=north_south]", [0, -2, 0]),
        ("redstone dust", "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]", [-2, 0, 0], "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]", [-3, 0, 0]),
        ("redstone torch", "minecraft:redstone_torch[lit=true]", [-2, 0, 0], "minecraft:redstone_torch[lit=true]", [-3, 0, 0]),
        ("redstone wall torch", "minecraft:redstone_wall_torch[facing=west,lit=true]", [-2, 0, 0], "minecraft:redstone_wall_torch[facing=west,lit=true]", [-3, 0, 0]),
        ("lever", "minecraft:lever[face=wall,facing=west,powered=false]", [-2, 0, 0], "minecraft:lever[face=wall,facing=east,powered=false]", [-2, 0, 0]),
        ("button", "minecraft:stone_button[face=wall,facing=west,powered=false]", [-2, 0, 0], "minecraft:stone_button[face=wall,facing=east,powered=false]", [-2, 0, 0]),
        ("repeater", "minecraft:repeater[delay=1,facing=west,locked=false,powered=false]", [-2, 0, 0], "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]", [-2, 0, 0]),
        ("comparator", "minecraft:comparator[facing=west,mode=compare,powered=false]", [-2, 0, 0], "minecraft:comparator[facing=east,mode=compare,powered=false]", [-2, 0, 0]),
        ("observer", "minecraft:observer[facing=west,powered=false]", [-2, 0, 0], "minecraft:observer[facing=east,powered=false]", [-2, 0, 0]),
        ("tripwire hook", "minecraft:tripwire_hook[attached=false,facing=west,powered=false]", [-2, 0, 0], "minecraft:tripwire_hook[attached=false,facing=east,powered=false]", [-2, 0, 0]),
        ("lightning rod", "minecraft:lightning_rod[facing=west,powered=false,waterlogged=false]", [-2, 0, 0], "minecraft:lightning_rod[facing=east,powered=false,waterlogged=false]", [-2, 0, 0]),
        ("detector rail", "minecraft:detector_rail[powered=false,shape=north_south]", [0, 2, 0], "minecraft:detector_rail[powered=false,shape=north_south]", [0, 3, 0]),
        ("pressure plate", "minecraft:stone_pressure_plate[powered=false]", [0, 2, 0], "minecraft:stone_pressure_plate[powered=false]", [0, 3, 0]),
        ("sculk sensor", "minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]", [0, 2, 0], "minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]", [0, 3, 0]),
        ("redstone block", "minecraft:redstone_block", [-1, 0, 0], "minecraft:redstone_block", [-2, 0, 0]),
        ("target", "minecraft:target[power=0]", [-1, 0, 0], "minecraft:target[power=0]", [-2, 0, 0]),
        ("daylight detector", "minecraft:daylight_detector[inverted=false,power=0]", [-1, 0, 0], "minecraft:daylight_detector[inverted=false,power=0]", [-2, 0, 0]),
        ("trapped chest", "minecraft:trapped_chest[facing=north,type=single,waterlogged=false]", [0, 2, 0], "minecraft:trapped_chest[facing=north,type=single,waterlogged=false]", [0, 3, 0]),
    ];
    let spec_with = |src: Block, at: [i32; 3]| -> FarmSpec {
        let mut blocks: Vec<(BlockPos, Block)> = rails.iter().map(|(p, s)| (*p, Block::parse(s).expect("rail"))).collect();
        blocks.push((BlockPos::new(at[0], at[1], at[2]), src));
        FarmSpec {
            blocks,
            entities: Vec::new(),
            output: BlockPos::new(4, 4, 4),
            lo: BlockPos::new(-6, -3, -6),
            hi: BlockPos::new(6, 6, 6),
            block_entities: Vec::new(),
            rock: false,
            nether: None,
            land: false,
        }
    };
    let mut out = Vec::new();
    for (name, inside, ia, outside, oa) in cases {
        let (Some(ib), Some(ob)) = (parse(inside), parse(outside)) else {
            out.push(format!("  {name}: NOT MODELLED (redsim does not parse {inside})"));
            continue;
        };
        if update_reach(BlockPos::new(0, 0, 0), &ib).is_none() {
            out.push(format!("  {name}: NOT A SOURCE in redsim's block model"));
            continue;
        }
        let (_, u_in) = tees(&spec_with(ib, ia), &[]);
        let (_, u_out) = tees(&spec_with(ob, oa), &[]);
        out.push(format!(
            "  {name}: at {},{},{} {} the T at 0,0,0; at {},{},{} ({}) {}: {}",
            ia[0],
            ia[1],
            ia[2],
            if u_in > 0 { "flagged beside" } else { "NOT flagged beside" },
            oa[0],
            oa[1],
            oa[2],
            outside.split('[').nth(1).map(|s| s.trim_end_matches(']')).unwrap_or("moved"),
            if u_out == 0 { "not flagged" } else { "FLAGGED" },
            if u_in > 0 && u_out == 0 { "PASS" } else { "FAIL" }
        ));
    }
    out
}

/// The placement-order scan (ironspec Nov 12 03:40, tick-exact's merge curve): every rail whose
/// designed shape differs from the shape vanilla placement gives it with every other designed
/// rail already laid (BaseRailBlock.onPlace -> RailState.place, from either default the
/// placer's facing gives: north_south or east_west), or whose placement re-shapes a laid
/// neighbour; each with its natural shape, its designed shape and the fix: an order (the rails it
/// must go before) or a helper rail on a designed exit with nothing there, placed first and
/// removed after (2 potential connections: no update re-shapes it).
pub fn placement(spec: &FarmSpec) -> Vec<String> {
    use redsim::block::Block;
    use redsim::world::{World, flags};
    let lo = BlockPos::new(spec.lo.x - 2, spec.lo.y - 2, spec.lo.z - 2);
    let size = [spec.hi.x - spec.lo.x + 5, spec.hi.y - spec.lo.y + 5, spec.hi.z - spec.lo.z + 5];
    let mut base = World::new(lo, size);
    for (p, b) in &spec.blocks {
        base.set_block(*p, *b, flags::RAW);
    }
    let air = Block::parse("minecraft:air").expect("air");
    let stone = Block::parse("minecraft:stone").expect("stone");
    let shape = |b: &Block| b.to_string().split("shape=").nth(1).map(|s| s.split([',', ']']).next().unwrap_or("").to_string());
    let with_shape = |b: &Block, sh: &str| -> Option<Block> {
        let s = b.to_string();
        let old = shape(b)?;
        Block::parse(&s.replace(&format!("shape={old}"), &format!("shape={sh}"))).ok()
    };
    let rails: Vec<(BlockPos, Block)> = spec.blocks.iter().filter(|(_, b)| b.to_string().contains("rail[")).cloned().collect();
    let designed: HashMap<BlockPos, Block> = rails.iter().cloned().collect();
    // (its neighbours a placement can connect: rails at its level, one up or one down)
    let near = |c: BlockPos| -> Vec<BlockPos> {
        let mut v = Vec::new();
        for (dx, dz) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            for dy in -1..=1 {
                let n = BlockPos::new(c.x + dx, c.y + dy, c.z + dz);
                if designed.contains_key(&n) {
                    v.push(n);
                }
            }
        }
        v
    };
    // (place `c` from each default into `w` and report whether it and `watch` end as designed)
    let try_place = |w: &World, c: BlockPos, after: &[BlockPos], watch: &[BlockPos]| -> Option<&'static str> {
        for def in ["north_south", "east_west"] {
            let mut w2 = w.clone();
            let b = with_shape(&designed[&c], def)?;
            w2.set_block(c, b, flags::ALL);
            for a in after {
                let mut ok = false;
                for d2 in ["north_south", "east_west"] {
                    let mut w3 = w2.clone();
                    let ab = with_shape(&designed[a], d2)?;
                    w3.set_block(*a, ab, flags::ALL);
                    if shape(&w3.get(*a)) == shape(&designed[a]) {
                        w2 = w3;
                        ok = true;
                        break;
                    }
                }
                if !ok {
                    break;
                }
            }
            if std::iter::once(&c).chain(watch.iter()).all(|q| shape(&w2.get(*q)) == shape(&designed[q])) {
                return Some(if def == "north_south" { "facing north or south" } else { "facing east or west" });
            }
        }
        None
    };
    let mut out = Vec::new();
    let mut pcells: Vec<BlockPos> = Vec::new();
    let mut results: Vec<(BlockPos, String, bool, Vec<BlockPos>, Option<String>)> = Vec::new();
    let mut cells: Vec<BlockPos> = rails.iter().map(|(p, _)| *p).collect();
    cells.sort_by_key(|c| (c.y, c.z, c.x));
    for c in cells {
        let nb = near(c);
        let mut w = base.clone();
        w.set_block(c, air, flags::RAW);
        if try_place(&w, c, &[], &nb).is_some() {
            continue;
        }
        // (its natural shape placed last, from the north_south default)
        let natural = {
            let mut w2 = w.clone();
            if let Some(b) = with_shape(&designed[&c], "north_south") {
                w2.set_block(c, b, flags::ALL);
            }
            let moved: Vec<String> = nb.iter().filter(|q| shape(&w2.get(**q)) != shape(&designed[q])).map(|q| format!("{},{},{} to {}", q.x, q.y, q.z, shape(&w2.get(*q)).unwrap_or_default())).collect();
            format!("{}{}", shape(&w2.get(c)).unwrap_or_default(), if moved.is_empty() { String::new() } else { format!(", re-shaping {}", moved.join(", ")) })
        };
        // (an order: before a subset of its neighbours, each then placed after it)
        let mut fix: Option<String> = None;
        for mask in 1u32..(1 << nb.len()) {
            let later: Vec<BlockPos> = nb.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, q)| *q).collect();
            let mut w2 = w.clone();
            for q in &later {
                w2.set_block(*q, air, flags::RAW);
            }
            if let Some(f) = try_place(&w2, c, &later, &nb) {
                fix = Some(format!("placement order: before {} ({f})", later.iter().map(|q| format!("{},{},{}", q.x, q.y, q.z)).collect::<Vec<_>>().join(" and ")));
                break;
            }
        }
        // (a helper rail on a designed exit with nothing there (a rising exit one level up; a
        // non-rail block there is laid after), on a temporary support where it needs one; also the
        // powered variant, a temporary redstone block beside it while it is placed (tick-exact's
        // official merge C); removed in game after it: the helper, its support, then the power,
        // then the cell's own block laid)
        let redstone = Block::parse("minecraft:redstone_block").expect("redstone block");
        if fix.is_none() {
            'h: for (dx, dz, rises) in exits(&designed[&c].to_string()) {
                let h = BlockPos::new(c.x + dx, c.y + rises as i32, c.z + dz);
                let support = BlockPos::new(h.x, h.y - 1, h.z);
                if designed.contains_key(&h) || designed.contains_key(&support) {
                    continue;
                }
                let orig_h = w.get(h);
                let need = matches!(w.get(support), Block::Air);
                let mut powers: Vec<Option<BlockPos>> = vec![None];
                for f in n6(c) {
                    if f != h && f != support && matches!(w.get(f), Block::Air) && !designed.contains_key(&f) {
                        powers.push(Some(f));
                    }
                }
                for pw in powers {
                    for def in ["north_south", "east_west"] {
                        let Some(b) = with_shape(&designed[&c], def) else { continue };
                        let mut w2 = w.clone();
                        w2.set_block(h, air, flags::RAW);
                        if need {
                            w2.set_block(support, stone, flags::RAW);
                        }
                        let helper = Block::parse(&format!("minecraft:rail[shape={}]", if dx != 0 { "east_west" } else { "north_south" })).expect("rail");
                        w2.set_block(h, helper, flags::RAW);
                        if let Some(f) = pw {
                            w2.set_block(f, redstone, flags::RAW);
                        }
                        w2.set_block(c, b, flags::ALL);
                        w2.set_block(h, air, flags::ALL);
                        if need {
                            w2.set_block(support, air, flags::ALL);
                        }
                        if let Some(f) = pw {
                            w2.set_block(f, air, flags::ALL);
                        }
                        if !matches!(orig_h, Block::Air) {
                            w2.set_block(h, orig_h, flags::ALL);
                        }
                        if shape(&w2.get(c)) == shape(&designed[&c]) && nb.iter().all(|q| shape(&w2.get(*q)) == shape(&designed[q])) {
                            fix = Some(format!(
                                "a helper rail at {},{},{}{}{}, then this rail ({}), then the helper{} removed{}{}",
                                h.x,
                                h.y,
                                h.z,
                                if need { format!(" on a temporary support at {},{},{}", support.x, support.y, support.z) } else { String::new() },
                                match pw {
                                    Some(f) => format!(" and a temporary redstone block at {},{},{}", f.x, f.y, f.z),
                                    None => String::new(),
                                },
                                if def == "north_south" { "facing north or south" } else { "facing east or west" },
                                if need { " and its support" } else { "" },
                                if pw.is_some() { ", then the redstone block" } else { "" },
                                if matches!(orig_h, Block::Air) { String::new() } else { format!(", then its cell's own {} laid", short(&orig_h.to_string()).split('[').next().unwrap_or("")) }
                            ));
                            break 'h;
                        }
                    }
                }
            }
        }
        let own_ok = {
            ["north_south", "east_west"].iter().any(|d| {
                let mut w3 = w.clone();
                with_shape(&designed[&c], d).is_some_and(|b| {
                    w3.set_block(c, b, flags::ALL);
                    shape(&w3.get(c)) == shape(&designed[&c])
                })
            })
        };
        let disturbed: Vec<BlockPos> = {
            let mut w2 = w.clone();
            if let Some(b) = with_shape(&designed[&c], "north_south") {
                w2.set_block(c, b, flags::ALL);
            }
            nb.iter().filter(|q| shape(&w2.get(**q)) != shape(&designed[q])).copied().collect()
        };
        results.push((c, natural, own_ok, disturbed, fix));
    }
    // (a rail natural in itself whose placement last re-shapes only neighbours that are placed
    // with a helper: laid before them)
    let helpered: Vec<BlockPos> = results.iter().filter(|r| r.4.as_ref().is_some_and(|f| f.starts_with("a helper"))).map(|r| r.0).collect();
    for r in results.iter_mut() {
        if r.4.is_none() && r.2 && !r.3.is_empty() && r.3.iter().all(|q| helpered.contains(q)) {
            r.4 = Some(format!("lay it before {} (their helper placement follows; laid last it re-shapes them)", r.3.iter().map(|q| format!("{},{},{}", q.x, q.y, q.z)).collect::<Vec<_>>().join(" and ")));
        }
    }
    for (c, natural, _, _, fix) in results {
        out.push(format!(
            "  {} {},{},{}: designed {}, placed last {}; fix: {}",
            short(&designed[&c].to_string()).split('[').next().unwrap_or(""),
            c.x,
            c.y,
            c.z,
            shape(&designed[&c]).unwrap_or_default(),
            natural,
            fix.unwrap_or_else(|| "NONE FOUND (re-route or re-level)".to_string())
        ));
        pcells.push(c);
    }
    // (ironspec Nov 12 03:55: every T and placement-order cell face-adjacent to the cell above a
    // sloped detector, powered or activator rail: its POWERED flip runs
    // BaseRailBlock.affectNeighborsAfterRemoval, lines 117-128, updateNeighborsAt(pos.above()))
    let mut above: HashMap<BlockPos, Vec<BlockPos>> = HashMap::new();
    for (p, b) in &rails {
        let s = b.to_string();
        if s.contains("ascending") && (s.contains("detector_rail") || s.contains("powered_rail") || s.contains("activator_rail")) {
            for q in n6(p.rel(redsim::pos::Dir::Up)) {
                above.entry(q).or_default().push(*p);
            }
        }
    }
    let conn3 = |c: &BlockPos| {
        [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().filter(|(dx, dz)| (-1..=1).any(|dy| designed.contains_key(&BlockPos::new(c.x + dx, c.y + dy, c.z + dz)))).count() == 3
    };
    let mut flagged: Vec<BlockPos> = rails.iter().filter(|(p, b)| matches!(b, Block::PlainRail { .. }) && (conn3(p) || pcells.contains(p))).map(|(p, _)| *p).filter(|p| above.contains_key(p)).collect();
    flagged.sort_by_key(|c| (c.y, c.z, c.x));
    for c in &flagged {
        out.push(format!(
            "  ABOVE-SLOPE {},{},{} ({}): a face of the cell above the sloped {}: declare it placement-order or power-forced, or move it",
            c.x,
            c.y,
            c.z,
            if conn3(c) { "a T" } else { "a placement-order cell" },
            above[c].iter().map(|p| format!("{} at {},{},{}", short(&designed[p].to_string()).split('[').next().unwrap_or(""), p.x, p.y, p.z)).collect::<Vec<_>>().join(", ")
        ));
    }
    out
}

/// The switched-cell scan (ironspec Nov 12 03:12, G1 03:10): for every switched cell (a plain
/// rail with exactly 3 potential connections, re-shaped on a signal-source update (26.3
/// RailBlock.updateState lines 22-23; a 4-way crossing re-shapes only at placement); a powered or
/// activator rail; a trapdoor), every source that can power it (26.3 hasNeighborSignal: a
/// component's weak signal into its face neighbours (a detector rail, a redstone block, a torch, a
/// lever, a button, a target, a daylight detector; dust into the block under it and its sides; a
/// repeater, comparator or observer into its output cell), and a conductor's strong power into its
/// face neighbours (a conductor beside the cell, strongly powered by a detector on top of it
/// (DetectorRailBlock.getDirectSignal: 15 for Direction.UP only), a torch under it, a repeater,
/// comparator or observer facing into it, a lever or button on it, dust on or into it); a powered
/// or activator rail's chain: the connected rails of its kind within 8 and their powers), and for
/// a T every signal-source update that reaches it (`tees`); each source must be one of `switches`.
/// Also each detector rail's face neighbours and its below-block's (checkPressed's
/// updateNeighborsAt(pos) / (pos.below()), DetectorRailBlock lines 93-94 / 102-103) with each plain
/// rail's potential connection count. Lines and the undeclared count.
pub fn switched(spec: &FarmSpec, switches: &[BlockPos]) -> (Vec<String>, usize) {
    let all: HashMap<BlockPos, String> = spec.blocks.iter().map(|(p, b)| (*p, b.to_string())).collect();
    const N6: [(i32, i32, i32); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
    let at = |p: BlockPos, d: (i32, i32, i32)| BlockPos::new(p.x + d.0, p.y + d.1, p.z + d.2);
    let is_rail = |p: &BlockPos| all.get(p).is_some_and(|s| s.contains("rail"));
    // (redsim's isRedstoneConductor; its world and position are unused)
    let dummy = redsim::world::World::new(BlockPos::new(0, 0, 0), [1, 1, 1]);
    let blk: HashMap<BlockPos, redsim::block::Block> = spec.blocks.iter().map(|(p, b)| (*p, *b)).collect();
    let conductor = |k: &BlockPos| blk.get(k).is_some_and(|b| redsim::behavior::is_conductor(&dummy, b, *k));
    // (each source: the cells it weakly powers, the conductors it strongly powers)
    let mut weak: HashMap<BlockPos, Vec<BlockPos>> = HashMap::new();
    let mut strong: HashMap<BlockPos, Vec<BlockPos>> = HashMap::new();
    let mut add = |m: &mut HashMap<BlockPos, Vec<BlockPos>>, q: BlockPos, s: BlockPos| {
        let v = m.entry(q).or_default();
        if !v.contains(&s) {
            v.push(s);
        }
    };
    for (p, b) in &spec.blocks {
        if let Some((w_cells, s_cells)) = power_out(*p, b) {
            for q in w_cells {
                add(&mut weak, q, *p);
            }
            for q in s_cells {
                add(&mut strong, q, *p);
            }
        }
    }
    // (the sources powering a cell: weak into it, or strong into a conductor beside it)
    let powers = |c: BlockPos| -> Vec<(BlockPos, String)> {
        let mut v: Vec<(BlockPos, String)> = Vec::new();
        for s in weak.get(&c).into_iter().flatten() {
            v.push((*s, "beside it".into()));
        }
        for d in N6 {
            let k = at(c, d);
            if conductor(&k) {
                for s in strong.get(&k).into_iter().flatten() {
                    v.push((*s, format!("through the conductor {},{},{}", k.x, k.y, k.z)));
                }
            }
        }
        v
    };
    let conn = |c: &BlockPos| (0..4).filter(|i| {
        let (dx, dz) = [(0, -1), (1, 0), (0, 1), (-1, 0)][*i];
        (-1..=1).any(|dy| is_rail(&BlockPos::new(c.x + dx, c.y + dy, c.z + dz)))
    }).count();
    let name = |p: &BlockPos| short(&all[p]).split('[').next().unwrap_or("").to_string();
    let mut out = Vec::new();
    let mut undeclared = 0usize;
    let mut cells: Vec<BlockPos> = all
        .iter()
        .filter(|(p, s)| (s.starts_with("minecraft:rail[") && conn(p) == 3) || s.contains("powered_rail") || s.contains("activator_rail") || s.contains("trapdoor"))
        .map(|(p, _)| *p)
        .collect();
    cells.sort_by_key(|c| (c.y, c.z, c.x));
    for c in cells {
        let s = &all[&c];
        let mut src: Vec<(BlockPos, String)> = powers(c);
        if s.contains("powered_rail") || s.contains("activator_rail") {
            // (the chain: the connected rails of its kind within 8 along its line)
            let kind = if s.contains("powered_rail") { "powered_rail" } else { "activator_rail" };
            for (dx, dz, _) in exits(s) {
                let mut q = c;
                for step in 1..=8 {
                    let n = (-1..=1).map(|dy| BlockPos::new(q.x + dx, q.y + dy, q.z + dz)).find(|n| all.get(n).is_some_and(|t| t.contains(kind)));
                    let Some(n) = n else { break };
                    for (p, how) in powers(n) {
                        src.push((p, format!("{how}, at {},{},{} ({step} along its chain)", n.x, n.y, n.z)));
                    }
                    q = n;
                }
            }
        }
        let mut parts = Vec::new();
        for (p, how) in &src {
            let ok = switches.contains(p);
            // (a redstone block in a design file never switches (the switched cells are left air
            // and emulated): its power is the design's own, listed and not counted)
            let fixed = matches!(blk.get(p), Some(redsim::block::Block::RedstoneBlock));
            if !ok && !fixed {
                undeclared += 1;
            }
            parts.push(format!("{} {},{},{} {how}{}", name(p), p.x, p.y, p.z, if ok { " (declared)" } else if fixed { " (fixed)" } else { " UNDECLARED" }));
        }
        out.push(format!("  {} {},{},{}: {}", name(&c), c.x, c.y, c.z, if parts.is_empty() { "no source reaches it".to_string() } else { parts.join("; ") }));
    }
    // (each detector rail's face neighbours and its below-block's: the plain rails' connection counts)
    let mut dets: Vec<&BlockPos> = all.iter().filter(|(_, s)| s.contains("detector_rail")).map(|(p, _)| p).collect();
    dets.sort_by_key(|c| (c.y, c.z, c.x));
    let mut raw3 = 0usize;
    for d in dets {
        let below = at(*d, (0, -1, 0));
        let mut rails = Vec::new();
        for base in [*d, below] {
            for dd in N6 {
                let n = at(base, dd);
                if n == *d {
                    continue;
                }
                if all.get(&n).is_some_and(|s| s.starts_with("minecraft:rail[")) {
                    let k = conn(&n);
                    rails.push(format!("{},{},{} ({k} potential connections{})", n.x, n.y, n.z, if k == 3 { ": RE-SHAPES on every switch" } else { "" }));
                }
            }
        }
        out.push(format!("  detector {},{},{}: plain rails among its and its below-block's face neighbours: {}", d.x, d.y, d.z, if rails.is_empty() { "none".to_string() } else { rails.join(", ") }));
        // (its raw connections: the cells its shape points to, an ascending end one level up,
        // and the one-down cell a flat end can connect to; ironspec Nov 12 03:54, from an official
        // probe: a cart pressing a detector re-settled the T at its raw connection, so no rail with
        // 3 potential connections may sit there unless an official probe shows it holds)
        let mut raws = Vec::new();
        for (dx, dz, rises) in exits(&all[d]) {
            let dys: &[i32] = if rises { &[1] } else { &[0, -1] };
            for dy in dys {
                let n = BlockPos::new(d.x + dx, d.y + dy, d.z + dz);
                if all.get(&n).is_some_and(|s| s.contains("rail")) {
                    let k = conn(&n);
                    if k == 3 {
                        raw3 += 1;
                    }
                    raws.push(format!("{},{},{} {} ({k} potential connections{})", n.x, n.y, n.z, name(&n), if k == 3 { ": FORBIDDEN unless an official probe shows it holds" } else { "" }));
                    break;
                }
            }
        }
        out.push(format!("  detector {},{},{}: raw connections: {}", d.x, d.y, d.z, if raws.is_empty() { "none".to_string() } else { raws.join(", ") }));
    }
    out.push(format!("  detectors' raw connections with 3 potential connections: {raw3}"));
    (out, undeclared + raw3)
}
