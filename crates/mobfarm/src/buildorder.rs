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

/// (commit G, ironspec 02:43 (2) / 02:46 (5) / 02:54 (4), G1 02:44b) A design's declared build sequence,
/// its header line `# build-order: NAME=STEP;STEP;...`: each step `x,y,z=STATE` (a placement; `air`, a
/// removal) or `~x,y,z=STATE`, a TEMPORARY placement (the builder's helper block: removed at the
/// sequence's end unless a later step removes it). 58f770cc's re-laid stop: `park=2,0,3=slime;
/// ~2,1,3=rail;1,0,3=powered_rail[...ascending_east];2,1,3=air` (tick-exact's car 7, `mobai
/// park-string`: laid as the file reads it the park goes flat; with the temporary rail it ascends).
pub struct Declared {
    pub name: String,
    /// (the cell, the state, temporary)
    pub steps: Vec<(BlockPos, Block, bool)>,
}

/// The design text's `# build-order:` lines, parsed (a malformed one an error naming it).
pub fn declared(text: &str) -> Result<Vec<Declared>, String> {
    let mut out: Vec<Declared> = Vec::new();
    for line in text.lines() {
        let Some(body) = line.trim().strip_prefix("# build-order:") else { continue };
        let (name, rest) = body.trim().split_once('=').ok_or_else(|| format!("# build-order: NAME=x,y,z=STATE;...: {line:?}"))?;
        let name = name.trim();
        if name.is_empty() || name.contains(',') {
            return Err(format!("# build-order: a sequence needs a NAME before its first `=`: {line:?}"));
        }
        if out.iter().any(|d| d.name == name) {
            return Err(format!("# build-order: the sequence {name:?} declared twice"));
        }
        let mut steps = Vec::new();
        for s in rest.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (temp, s) = match s.strip_prefix('~') {
                Some(t) => (true, t),
                None => (false, s),
            };
            let (c, st) = s.split_once('=').ok_or_else(|| format!("# build-order {name}: a step x,y,z=STATE: {s:?}"))?;
            let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("# build-order {name}: {c:?}: {e}"))?;
            if v.len() != 3 {
                return Err(format!("# build-order {name}: a cell x,y,z: {c:?}"));
            }
            let st = st.trim();
            let st = if st.contains(':') { st.to_string() } else { format!("minecraft:{st}") };
            let b = Block::parse(&st).map_err(|e| format!("# build-order {name}: {st}: {e}"))?;
            if temp && b.is_air() {
                return Err(format!("# build-order {name}: a temporary step places a block, not air: {s:?}"));
            }
            steps.push((BlockPos::new(v[0], v[1], v[2]), b, temp));
        }
        if steps.is_empty() {
            return Err(format!("# build-order {name}: no steps"));
        }
        out.push(Declared { name: name.to_string(), steps });
    }
    Ok(out)
}

/// Whether two states are the same build result: a rail kind by its name and shape (its power is the
/// circuit's), anything else whole.
fn same_build(a: &Block, b: &Block) -> bool {
    if is_rail_state(a) && is_rail_state(b) {
        let name = |x: &Block| x.to_string().split('[').next().unwrap_or("").to_string();
        let sh = |x: &Block| x.to_string().split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("-").to_string();
        return name(a) == name(b) && sh(a) == sh(b);
    }
    a.to_string() == b.to_string()
}

/// (commit G) How many plain builds of `cells` (every order the game allows x each rail's two
/// placement facings, the T test's builds) end with `t` in `want`: (reaching, built).
fn plain_reach(blocks: &[(BlockPos, Block)], t: BlockPos, cells: &[(BlockPos, Block)], want: &str) -> Result<(usize, usize), String> {
    let rails: Vec<usize> = (0..cells.len()).filter(|&k| is_rail_state(&cells[k].1)).collect();
    let masks = 1usize << rails.len();
    let (mut reach, mut built) = (0usize, 0usize);
    for order in permutations(cells.len()) {
        if (0..cells.len()).any(|i| support_of(cells, i).is_some_and(|s| order.iter().position(|&k| k == s) > order.iter().position(|&k| k == i))) {
            continue;
        }
        for mask in 0..masks {
            built += 1;
            let ew = |k: usize| rails.iter().position(|&r| r == k).is_some_and(|j| mask & (1 << j) != 0);
            let mut w = base_world(blocks, t, cells);
            for &k in &order {
                let (p, b) = &cells[k];
                w.set_block(*p, placed_state(b, ew(k))?, flags::ALL);
            }
            if shape(&w, t) == want {
                reach += 1;
            }
        }
    }
    Ok((reach, built))
}

/// (commit G) The declared-sequence mode: each declared sequence EXECUTED step by step on the design
/// with every cell it doesn't name already laid (raw, as a paste), each step with its updates (a rail
/// placed in its default shape, shaped by `onPlace`), its temporaries removed at its end, and every
/// cell it names read back against the design (a temporary against the design's own cell, air where
/// the design has none): off any one fails by name. Then the plain orders over EVERY rail cell of the
/// design (each rail with the rails around it, the T test's every-order build): a rail no plain order
/// brings to its designed shape fails unless a declared sequence lays it, where its plain-order result
/// prints as information. `without` names declared sequences left out (the positive control).
pub fn run_declared(blocks: &[(BlockPos, Block)], seqs: &[Declared], without: &[String]) -> Result<(Vec<String>, usize), String> {
    if let Some(w) = without.iter().find(|w| !seqs.iter().any(|d| &d.name == *w)) {
        return Err(format!("build-order --without {w}: no declared sequence of that name"));
    }
    let seqs: Vec<&Declared> = seqs.iter().filter(|d| !without.contains(&d.name)).collect();
    let at = |p: BlockPos| blocks.iter().find(|(q, _)| *q == p).map(|(_, b)| b.clone()).unwrap_or_else(|| Block::parse("minecraft:air").expect("air"));
    let (mut lines, mut failing) = (Vec::new(), 0usize);
    // (the box: every design block and every step's cell, 2 cells of margin)
    let all: Vec<BlockPos> = blocks.iter().map(|(p, _)| *p).chain(seqs.iter().flat_map(|d| d.steps.iter().map(|(p, _, _)| *p))).collect();
    if all.is_empty() {
        return Err("build-order --declared: an empty design".into());
    }
    let lo = BlockPos::new(all.iter().map(|p| p.x).min().unwrap_or(0) - 2, all.iter().map(|p| p.y).min().unwrap_or(0) - 2, all.iter().map(|p| p.z).min().unwrap_or(0) - 2);
    let hi = BlockPos::new(all.iter().map(|p| p.x).max().unwrap_or(0) + 2, all.iter().map(|p| p.y).max().unwrap_or(0) + 2, all.iter().map(|p| p.z).max().unwrap_or(0) + 2);
    let size = [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1];
    for d in &seqs {
        let named: Vec<BlockPos> = d.steps.iter().map(|(p, _, _)| *p).collect();
        let mut w = World::new(lo, size);
        for (p, b) in blocks {
            if !named.contains(p) {
                w.set_block(*p, b.clone(), flags::RAW);
            }
        }
        let mut pending: Vec<BlockPos> = Vec::new();
        let mut trail = Vec::new();
        for (p, b, temp) in &d.steps {
            if b.is_air() {
                w.set_block(*p, b.clone(), flags::ALL);
                pending.retain(|q| q != p);
                trail.push(format!("{p} removed"));
            } else {
                w.set_block(*p, placed_state(b, false)?, flags::ALL);
                if *temp {
                    pending.push(*p);
                }
                let s = w.get(*p).to_string();
                trail.push(format!("{}{p} {}", if *temp { "~" } else { "" }, s.trim_start_matches("minecraft:")));
            }
        }
        for p in &pending {
            w.set_block(*p, Block::parse("minecraft:air").expect("air"), flags::ALL);
            trail.push(format!("{p} (temporary) removed at the end"));
        }
        let mut off: Vec<String> = Vec::new();
        let mut checked: Vec<BlockPos> = Vec::new();
        for p in &named {
            if checked.contains(p) {
                continue;
            }
            checked.push(*p);
            let (want, got) = (at(*p), w.get(*p));
            if !same_build(&want, &got) {
                off.push(format!("{p}: {} (the design: {})", got.to_string().trim_start_matches("minecraft:"), want.to_string().trim_start_matches("minecraft:")));
            }
        }
        if !off.is_empty() {
            failing += 1;
        }
        lines.push(format!(
            "build-order declared sequence \"{}\" (each rail placed facing north / south, its default north_south, shaped by onPlace; every cell it doesn't name laid first): {}; read back: {}",
            d.name,
            trail.join(", "),
            if off.is_empty() { format!("every named cell as designed ({} cells)", checked.len()) } else { format!("OFF THE DESIGN: {} (FAILS)", off.join("; ")) }
        ));
    }
    // (the plain orders over every rail cell: each with the rails in its 3x3x3 box, at most 4 of them,
    // the nearest first)
    let rails: Vec<(BlockPos, Block)> = blocks.iter().filter(|(_, b)| is_rail_state(b)).cloned().collect();
    let (mut plain_ok, mut plain_info, mut plain_fail) = (0usize, Vec::new(), Vec::new());
    for (r, rb) in &rails {
        let mut near: Vec<(BlockPos, Block)> = rails.iter().filter(|(q, _)| q != r && (q.x - r.x).abs() <= 1 && (q.y - r.y).abs() <= 1 && (q.z - r.z).abs() <= 1).cloned().collect();
        near.sort_by_key(|(q, _)| (q.x - r.x).abs() + (q.y - r.y).abs() + (q.z - r.z).abs());
        // (at most 4 neighbours: 5 cells, 120 orders x 32 rail facings, so a dense rail field stays
        // bounded; the nearest first)
        near.truncate(4);
        let mut cells = vec![(*r, rb.clone())];
        cells.extend(near);
        let want = rb.to_string().split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).unwrap_or("-").to_string();
        let (reach, built) = plain_reach(blocks, *r, &cells, &want)?;
        if reach > 0 {
            plain_ok += 1;
        } else if let Some(d) = seqs.iter().find(|d| d.steps.iter().any(|(p, _, _)| p == r)) {
            plain_info.push(format!("{r} ({want}): no plain order reaches it (0 of {built} builds); laid by the declared sequence \"{}\"", d.name));
        } else {
            plain_fail.push(format!("{r} ({want}): no plain order reaches it (0 of {built} builds), and no declared sequence lays it"));
        }
    }
    failing += plain_fail.len();
    lines.push(format!(
        "build-order plain orders over every rail cell ({} rails: each with up to 4 rails of its 3x3x3 box, every order the game allows x each rail's two placement facings; a rail passes when at least one plain build reaches its designed shape, ironspec 02:46 (5)): {plain_ok} reach it{}{}",
        rails.len(),
        if plain_info.is_empty() { String::new() } else { format!("; information (a declared sequence lays them): {}", plain_info.join("; ")) },
        if plain_fail.is_empty() { String::new() } else { format!("; FAILING: {}", plain_fail.join("; ")) }
    ));
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

    /// (commit G) 58f770cc's re-laid stop's park, in small: the park rail (1,0,3) ascends east only
    /// with a TEMPORARY rail on the slime (2,0,3) while it is placed: the declared sequence reaches the
    /// design; left out (the positive control), the park fails by name.
    fn park() -> (Vec<(redsim::pos::BlockPos, redsim::block::Block)>, Vec<super::Declared>) {
        let b = |s: &str| redsim::block::Block::parse(s).unwrap();
        let p = redsim::pos::BlockPos::new;
        let blocks = vec![
            (p(0, -1, 3), b("minecraft:glass")),
            (p(1, -1, 3), b("minecraft:glass")),
            (p(0, 0, 3), b("minecraft:rail[shape=east_west,waterlogged=false]")),
            (p(1, 0, 3), b("minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]")),
            (p(2, 0, 3), b("minecraft:slime_block")),
        ];
        let seqs = super::declared("# build-order: park=2,0,3=minecraft:slime_block;~2,1,3=minecraft:rail[shape=north_south,waterlogged=false];1,0,3=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false];2,1,3=air\n# build-order-note: prose").unwrap();
        (blocks, seqs)
    }

    #[test]
    fn the_declared_park_reaches_the_design_and_its_control_fails_by_name() {
        let (blocks, seqs) = park();
        assert_eq!(seqs.len(), 1);
        assert_eq!(seqs[0].steps.len(), 4);
        assert!(seqs[0].steps[1].2, "the ~ step is temporary");
        let (lines, failing) = super::run_declared(&blocks, &seqs, &[]).unwrap();
        assert_eq!(failing, 0, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("every named cell as designed")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("1,0,3") && l.contains("laid by the declared sequence \"park\"")), "{lines:?}");
        let (lines, failing) = super::run_declared(&blocks, &seqs, &["park".to_string()]).unwrap();
        assert!(failing >= 1, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("FAILING") && l.contains("1,0,3") && l.contains("no declared sequence lays it")), "{lines:?}");
    }

    #[test]
    fn a_malformed_build_order_line_is_refused() {
        assert!(super::declared("# build-order: =1,0,0=stone").is_err());
        assert!(super::declared("# build-order: p=1,0=stone").is_err());
        assert!(super::declared("# build-order: p=~1,0,0=air").is_err());
        assert!(super::declared("# build-order: p=1,0,0=stone\n# build-order: p=2,0,0=stone").is_err());
        assert!(super::run_declared(&[], &super::declared("# build-order: p=1,0,0=stone").unwrap(), &["q".to_string()]).is_err());
    }
}
