//! (ironspec 09:44 / 09:53 / 10:12, farmopt 10:07) The build-order test for a rail junction: the
//! junction's cells placed into the design around them (a box of 3 cells each way from the T, the
//! rest laid raw first), in EVERY order the game allows (a block never before its support: a rail,
//! a torch, a lever, dust or a diode after the block under it), each rail placed in its default
//! shape and shaped by `onPlace` (redsim's `rails::on_place`: `RailState.place` with the T's powered
//! preference), every other block set with its updates; two pastes (all the cells raw at once, then
//! each cell's neighbours updated in paste order; and a y-z-x placement loop with updates); and
//! named SEQUENCES of placements and removals (a T laid in a wrong order, a repair, a parity
//! positive), each step's T shape printed. Any build or sequence not ending in its wanted shape is a
//! failure; a sequence may want a shape the design must NOT reach (a positive control).

use redsim::block::Block;
use redsim::pos::BlockPos;
use redsim::world::{World, flags};

/// A named sequence of steps (a cell set to a state, `air` a removal) and the shape it must end in.
pub struct Sequence {
    pub name: String,
    pub steps: Vec<(BlockPos, Block)>,
    pub want: String,
}

/// The test's inputs: the junction T, the junction's cells (each with the state it ends in; a
/// rail's shape is the placement's to decide), the wanted shape at the T, extra sequences.
pub struct BuildOrderOpts {
    pub t: BlockPos,
    pub cells: Vec<(BlockPos, Block)>,
    pub want: String,
    pub sequences: Vec<Sequence>,
    /// The build rule's cells placed last (form (C): the torch last): an order with one of them
    /// before another cell is outside the rule, counted apart, not built.
    pub last: Vec<BlockPos>,
}

/// Whether a state is a rail kind (plain, powered, detector, activator): placed in its default
/// shape from the player's facing.
pub fn is_rail_state(b: &Block) -> bool {
    let s = b.to_string();
    s.starts_with("minecraft:rail[") || s.starts_with("minecraft:powered_rail[") || s.starts_with("minecraft:detector_rail[") || s.starts_with("minecraft:activator_rail[")
}

/// A block's placement state: a rail kind in its default shape from the player's facing
/// (`BaseRailBlock.getStateForPlacement`: east_west when the player faces east or west, else
/// north_south; unpowered), which `onPlace` then shapes; any other block as given.
fn placed_state(b: &Block, east_west: bool) -> Result<Block, String> {
    let s = b.to_string();
    let name = s.split('[').next().unwrap_or("");
    let sh = if east_west { "east_west" } else { "north_south" };
    let default = match name {
        "minecraft:rail" => format!("minecraft:rail[shape={sh},waterlogged=false]"),
        "minecraft:powered_rail" | "minecraft:activator_rail" | "minecraft:detector_rail" => format!("{name}[powered=false,shape={sh},waterlogged=false]"),
        _ => return Ok(b.clone()),
    };
    Block::parse(&default).map_err(|e| e.to_string())
}

/// The cell `i` is mounted on, if it is one of the cells (G1 on e2d9333bc (B10): by the block's
/// mounting: rails, standing torches, dust, diodes on the block below; a wall torch, a tripwire
/// hook, a wall lever or button on the block behind its facing; a ceiling lever or button on the
/// block above).
fn support_of(cells: &[(BlockPos, Block)], i: usize) -> Option<usize> {
    let (p, b) = &cells[i];
    let s = b.to_string();
    let prop = |k: &str| s.split(&format!("{k}=")).nth(1).and_then(|r| r.split([',', ']']).next()).map(|v| v.to_string());
    let behind = |f: Option<String>| -> Option<BlockPos> {
        match f.as_deref() {
            Some("north") => Some(p.rel(redsim::Dir::South)),
            Some("south") => Some(p.rel(redsim::Dir::North)),
            Some("east") => Some(p.rel(redsim::Dir::West)),
            Some("west") => Some(p.rel(redsim::Dir::East)),
            _ => None,
        }
    };
    let support = if s.contains("wall_torch") || s.contains("tripwire_hook") {
        behind(prop("facing"))
    } else if s.contains("lever") || s.contains("button") {
        match prop("face").as_deref() {
            Some("ceiling") => Some(p.rel(redsim::Dir::Up)),
            Some("wall") => behind(prop("facing")),
            _ => Some(p.rel(redsim::Dir::Down)),
        }
    } else if ["redstone_torch", "rail", "redstone_wire", "repeater", "comparator"].iter().any(|k| s.contains(k)) {
        Some(p.rel(redsim::Dir::Down))
    } else {
        None
    }?;
    cells.iter().position(|(q, _)| *q == support)
}

/// The box's blocks from the design, the junction's cells left out (raw).
fn base_world(blocks: &[(BlockPos, Block)], t: BlockPos, cells: &[(BlockPos, Block)]) -> World {
    let lo = BlockPos::new(t.x - 3, t.y - 3, t.z - 3);
    let mut w = World::new(lo, [7, 7, 7]);
    for (p, b) in blocks {
        if (p.x - t.x).abs() <= 3 && (p.y - t.y).abs() <= 3 && (p.z - t.z).abs() <= 3 && !cells.iter().any(|(q, _)| q == p) {
            w.set_block(*p, b.clone(), flags::RAW);
        }
    }
    w
}

fn shape(w: &World, p: BlockPos) -> String {
    let s = w.get(p).to_string();
    s.split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("-").to_string()
}

/// Every permutation of 0..n (Heap's algorithm).
fn permutations(n: usize) -> Vec<Vec<usize>> {
    let mut a: Vec<usize> = (0..n).collect();
    let mut out = vec![a.clone()];
    let mut c = vec![0usize; n];
    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                a.swap(0, i);
            } else {
                a.swap(c[i], i);
            }
            out.push(a.clone());
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    out
}

/// The test: (report lines, failures).
pub fn run(blocks: &[(BlockPos, Block)], o: &BuildOrderOpts) -> Result<(Vec<String>, usize), String> {
    if o.cells.len() > 7 {
        return Err("build-order: at most 7 cells (5,040 orders)".into());
    }
    if !o.cells.iter().any(|(p, _)| *p == o.t) {
        return Err(format!("build-order: the T {} is not among the cells", o.t));
    }
    let mut lines = Vec::new();
    if let Some(c) = o.last.iter().find(|c| !o.cells.iter().any(|(p, _)| p == *c)) {
        return Err(format!("build-order: --last {c} is not among the cells"));
    }
    // (G1 on e2d9333bc (B10): a cell or a --last named twice is refused)
    for (i, (p, _)) in o.cells.iter().enumerate() {
        if o.cells[..i].iter().any(|(q, _)| q == p) {
            return Err(format!("build-order: the cell {p} given twice"));
        }
    }
    for (i, c) in o.last.iter().enumerate() {
        if o.last[..i].contains(c) {
            return Err(format!("build-order: --last {c} given twice"));
        }
    }
    // (G1 on e2d9333bc (B10): each rail placed by a player facing north / south (its default
    // north_south) or east / west (east_west), every combination over the rails)
    let rails: Vec<usize> = (0..o.cells.len()).filter(|&k| crate::buildorder::is_rail_state(&o.cells[k].1)).collect();
    let masks = 1usize << rails.len();
    let (mut tried, mut skipped, mut failing) = (0usize, 0usize, 0usize);
    let mut skipped_rule = 0usize;
    let mut builds = 0usize;
    let mut ends: Vec<(String, usize)> = Vec::new();
    let mut first_fail: Vec<String> = Vec::new();
    for order in permutations(o.cells.len()) {
        // (an order the game can't build: a block before its support)
        if (0..o.cells.len()).any(|i| support_of(&o.cells, i).is_some_and(|s| order.iter().position(|&k| k == s) > order.iter().position(|&k| k == i))) {
            skipped += 1;
            continue;
        }
        // (an order against the build rule: a `--last` cell not among the final placements)
        let tail: Vec<BlockPos> = order[order.len() - o.last.len()..].iter().map(|&k| o.cells[k].0).collect();
        if !o.last.iter().all(|c| tail.contains(c)) {
            skipped_rule += 1;
            continue;
        }
        tried += 1;
        for mask in 0..masks {
            builds += 1;
            let ew = |k: usize| rails.iter().position(|&r| r == k).is_some_and(|j| mask & (1 << j) != 0);
            let mut w = base_world(blocks, o.t, &o.cells);
            for &k in &order {
                let (p, b) = &o.cells[k];
                w.set_block(*p, placed_state(b, ew(k))?, flags::ALL);
            }
            let s = shape(&w, o.t);
            match ends.iter_mut().find(|e| e.0 == s) {
                Some(e) => e.1 += 1,
                None => ends.push((s.clone(), 1)),
            }
            if s != o.want {
                failing += 1;
                if first_fail.len() < 4 {
                    first_fail.push(format!("{} -> {s}", order.iter().map(|&k| format!("{}{}", o.cells[k].0, if rails.contains(&k) { if ew(k) { " (placed facing east / west)" } else { " (placed facing north / south)" } } else { "" })).collect::<Vec<_>>().join(" ")));
                }
            }
        }
    }
    // (G1 on e2d9333bc (B1): a test that built nothing (every order skipped) fails)
    if tried == 0 {
        failing += 1;
    }
    lines.push(format!(
        "build-order at the T {}: {tried} orders built, {builds} builds over the rails' placement facings ({} each; {skipped} orders skipped: a block before its support{}){}, ending {}; wanted {}: {failing} failing{}",
        o.t,
        masks,
        if o.last.is_empty() { String::new() } else { format!("; {skipped_rule} skipped: against the build rule, {} placed last", o.last.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(" ")) },
        if tried == 0 { ": NO ORDER BUILT, a failure" } else { "" },
        ends.iter().map(|(s, n)| format!("{s} {n}")).collect::<Vec<_>>().join(", "),
        o.want,
        if first_fail.is_empty() { String::new() } else { format!(" (e.g. {})", first_fail.join("; ")) }
    ));
    // (the pastes: all the cells raw at once, then each cell's neighbours updated in paste order
    // (y, z, x); and the same order placed with updates)
    let mut sorted: Vec<usize> = (0..o.cells.len()).collect();
    sorted.sort_by_key(|&k| (o.cells[k].0.y, o.cells[k].0.z, o.cells[k].0.x));
    let mut w = base_world(blocks, o.t, &o.cells);
    for &k in &sorted {
        w.set_block(o.cells[k].0, o.cells[k].1.clone(), flags::RAW);
    }
    for &k in &sorted {
        let (p, b) = &o.cells[k];
        w.update_neighbors_at(*p, b.kind());
    }
    let raw = shape(&w, o.t);
    let mut w = base_world(blocks, o.t, &o.cells);
    for &k in &sorted {
        let (p, b) = &o.cells[k];
        w.set_block(*p, placed_state(b, false)?, flags::ALL);
    }
    let looped = shape(&w, o.t);
    for (what, s) in [("a raw paste, then each cell's neighbours updated", &raw), ("a y-z-x placement loop with updates (each rail placed facing north / south, its default north_south)", &looped)] {
        if *s != o.want {
            failing += 1;
        }
        lines.push(format!("build-order at the T {}: {what}: {s}{}", o.t, if *s == o.want { "" } else { " (FAILS)" }));
    }
    // (the named sequences: from the box without the junction's cells, each step a placement (a
    // rail in its default shape, shaped by onPlace) or a removal (air), the T's shape after each)
    for sq in &o.sequences {
        let mut w = base_world(blocks, o.t, &o.cells);
        let mut trail = Vec::new();
        for (p, b) in &sq.steps {
            let what = if b.is_air() { format!("{p} removed") } else { format!("{p} {}", b.to_string().trim_start_matches("minecraft:").split('[').next().unwrap_or("")) };
            w.set_block(*p, if b.is_air() { b.clone() } else { placed_state(b, false)? }, flags::ALL);
            trail.push(format!("{what} -> {}", shape(&w, o.t)));
        }
        let end = shape(&w, o.t);
        let ok = end == sq.want;
        if !ok {
            failing += 1;
        }
        lines.push(format!("build-order sequence \"{}\" (each rail placed facing north / south, its default north_south): {}; ends {end}, wanted {}{}", sq.name, trail.join(", "), sq.want, if ok { "" } else { " (FAILS)" }));
    }
    Ok((lines, failing))
}

#[cfg(test)]
mod tests {
    #[test]
    fn permutations_cover_every_order() {
        let p = super::permutations(4);
        assert_eq!(p.len(), 24);
        let mut s: Vec<Vec<usize>> = p.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), 24);
    }
}
