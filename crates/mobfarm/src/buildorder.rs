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
        // (G1 on G, non-blocking: a NAME is letters, digits, `-` and `_`)
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("# build-order: a sequence needs a NAME (letters, digits, - or _) before its first `=`: {line:?}"));
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
            // (G1 on G (6): a property the block doesn't have is refused, never dropped)
            if let Some((_, props)) = st.split_once('[') {
                let got = b.to_string();
                for kv in props.trim_end_matches(']').split(',').map(str::trim).filter(|kv| !kv.is_empty()) {
                    let k = kv.split('=').next().unwrap_or("").trim();
                    if !got.contains(&format!("[{k}=")) && !got.contains(&format!(",{k}=")) {
                        return Err(format!("# build-order {name}: {st}: the block has no property {k:?}"));
                    }
                }
            }
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
/// circuit's), anything else whole but for its circuit-driven properties (power, powered, triggered,
/// lit, locked: what a running circuit changes, not what a build lays).
fn same_build(a: &Block, b: &Block) -> bool {
    if is_rail_state(a) && is_rail_state(b) {
        let name = |x: &Block| x.to_string().split('[').next().unwrap_or("").to_string();
        let sh = |x: &Block| x.to_string().split("shape=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("-").to_string();
        return name(a) == name(b) && sh(a) == sh(b);
    }
    let strip = |x: &Block| {
        let s = x.to_string();
        match s.split_once('[') {
            None => s,
            Some((n, r)) => format!(
                "{n}[{}]",
                r.trim_end_matches(']').split(',').filter(|kv| !["power", "powered", "triggered", "lit", "locked"].contains(&kv.split('=').next().unwrap_or(""))).collect::<Vec<_>>().join(",")
            ),
        }
    };
    strip(a) == strip(b)
}

/// (commit G) How many plain builds of `cells` (every order the game allows x each rail's two
/// placement facings, the T test's builds) end with `t` in `want`: (reaching, built, one reaching
/// order with its rails' facings, G1 on G (5)).
fn plain_reach(blocks: &[(BlockPos, Block)], t: BlockPos, cells: &[(BlockPos, Block)], want: &str) -> Result<(usize, usize, Option<String>), String> {
    let rails: Vec<usize> = (0..cells.len()).filter(|&k| is_rail_state(&cells[k].1)).collect();
    let masks = 1usize << rails.len();
    let (mut reach, mut built, mut first) = (0usize, 0usize, None);
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
                if first.is_none() {
                    first = Some(order.iter().map(|&k| format!("{}{}", cells[k].0, if rails.contains(&k) { if ew(k) { " (facing east / west)" } else { " (facing north / south)" } } else { "" })).collect::<Vec<_>>().join(" "));
                }
            }
        }
    }
    Ok((reach, built, first))
}

/// (commit G; G′, G1's review of G) The declared-sequence mode. Each declared sequence EXECUTED on the
/// design: every cell it doesn't name laid first (raw, a paste), then its steps in order, each through
/// the game's placement check first (the block must survive where it is placed, `can_survive`; no rail
/// near it may pop) and placed with its updates (a rail in its default north_south shape, shaped by
/// `onPlace`), each step's resulting state checked against the state it declares, its temporaries
/// removed at its end (a placement over a pending temporary clears it), and then EVERY design cell and
/// every step cell read back against the design (circuit-driven properties aside): off any one fails
/// by name. Then the plain orders over every rail (each with up to 4 rails of its 3x3x3 box; every other
/// design block within 3 blocks laid RAW first, the window's assumption): a rail passes when at least
/// one plain build reaches its designed shape (below N of N, reaching / total and one reaching order
/// printed); a rail no plain build reaches fails unless a declared sequence lays it. Then ONE GLOBAL
/// ORDER on the whole design (the guide's claim): every block the sequences don't name placed with its
/// updates in a sweep (the blocks that stand alone first, then the rest, y up, then z, then x, each
/// rail facing its designed axis), then the declared sequences, then every design cell read back; four
/// sweeps tried, the first that reproduces the design printed, none a failure. `without` names declared
/// sequences left out (the positive control). Zero declared sequences or zero rails judge nothing: a
/// failure.
pub fn run_declared(blocks: &[(BlockPos, Block)], seqs: &[Declared], without: &[String]) -> Result<(Vec<String>, usize), String> {
    run_orders(blocks, seqs, without, false)
}

/// (G′, ironspec 09:32a: "would reshape 0" over every design file) The GLOBAL mode: the same plain
/// orders over every rail and the one global order (the declared sequences, if any, executed as in
/// the declared mode), for a design that needs no declared sequence: a rail the game reshapes or pops
/// in every order fails by name.
pub fn run_global(blocks: &[(BlockPos, Block)], seqs: &[Declared]) -> Result<(Vec<String>, usize), String> {
    run_orders(blocks, seqs, &[], true)
}

fn run_orders(blocks: &[(BlockPos, Block)], seqs: &[Declared], without: &[String], global: bool) -> Result<(Vec<String>, usize), String> {
    if let Some(w) = without.iter().find(|w| !seqs.iter().any(|d| &d.name == *w)) {
        return Err(format!("build-order --without {w}: no declared sequence of that name"));
    }
    if blocks.is_empty() {
        return Err("build-order --declared: an empty design".into());
    }
    let seqs: Vec<&Declared> = seqs.iter().filter(|d| !without.contains(&d.name)).collect();
    let (mut lines, mut failing) = (Vec::new(), 0usize);
    // (G1 on G (4): every step inside the design's own box, and each sequence naming a design cell)
    let dlo = BlockPos::new(blocks.iter().map(|(p, _)| p.x).min().unwrap_or(0), blocks.iter().map(|(p, _)| p.y).min().unwrap_or(0), blocks.iter().map(|(p, _)| p.z).min().unwrap_or(0));
    let dhi = BlockPos::new(blocks.iter().map(|(p, _)| p.x).max().unwrap_or(0), blocks.iter().map(|(p, _)| p.y).max().unwrap_or(0), blocks.iter().map(|(p, _)| p.z).max().unwrap_or(0));
    let inside = |p: BlockPos| p.x >= dlo.x && p.x <= dhi.x && p.y >= dlo.y && p.y <= dhi.y && p.z >= dlo.z && p.z <= dhi.z;
    for d in &seqs {
        if let Some((p, _, _)) = d.steps.iter().find(|(p, _, _)| !inside(*p)) {
            return Err(format!("build-order: the sequence \"{}\" steps outside the design's box {dlo}..{dhi}: {p}", d.name));
        }
        if !d.steps.iter().any(|(p, _, t)| !t && blocks.iter().any(|(q, _)| q == p)) {
            return Err(format!("build-order: the sequence \"{}\" names no cell of the design", d.name));
        }
    }
    // (G1 on G (3): zero samples judge nothing; the global mode judges the plain and global orders)
    if seqs.is_empty() && !global {
        failing += 1;
        lines.push("build-order --declared: NO DECLARED SEQUENCE (no `# build-order:` line, or every one left out): nothing judged, a failure (a plain junction is `build-order --t`'s)".to_string());
    } else if seqs.is_empty() {
        lines.push("build-order --global: no declared sequence: the whole design by its plain orders and one global order".to_string());
    }
    let lo = BlockPos::new(dlo.x - 2, dlo.y - 2, dlo.z - 2);
    let size = [dhi.x - dlo.x + 5, dhi.y - dlo.y + 5, dhi.z - dlo.z + 5];
    let named_all: Vec<BlockPos> = seqs.iter().flat_map(|d| d.steps.iter().map(|(p, _, _)| *p)).collect();
    for d in &seqs {
        let named: Vec<BlockPos> = d.steps.iter().map(|(p, _, _)| *p).collect();
        let mut w = World::new(lo, size);
        for (p, b) in blocks {
            if !named.contains(p) {
                w.set_block(*p, b.clone(), flags::RAW);
            }
        }
        let (mut trail, mut bad) = (Vec::new(), Vec::new());
        run_steps(&mut w, d, &mut trail, &mut bad)?;
        let mut off = read_back(&w, blocks, &named);
        off.extend(bad);
        if !off.is_empty() {
            failing += 1;
        }
        lines.push(format!(
            "build-order declared sequence \"{}\" (every cell it doesn't name laid first, raw; each step through the game's placement check, then placed with its updates, a rail in its default north_south shape shaped by onPlace, its result checked against the step's state): {}; read back, EVERY design cell and every step cell: {}",
            d.name,
            trail.join(", "),
            if off.is_empty() { format!("all as designed ({} design cells, {} step cells)", blocks.len(), named.len()) } else { format!("OFF: {} (FAILS)", off.join("; ")) }
        ));
    }
    // (the plain orders over every rail cell: each with the rails in its 3x3x3 box, at most 4 of them,
    // the nearest first)
    let rails: Vec<(BlockPos, Block)> = blocks.iter().filter(|(_, b)| is_rail_state(b)).cloned().collect();
    let (mut plain_ok, mut plain_part, mut plain_info, mut plain_fail) = (0usize, Vec::new(), Vec::new(), Vec::new());
    for (r, rb) in &rails {
        let mut near: Vec<(BlockPos, Block)> = rails.iter().filter(|(q, _)| q != r && (q.x - r.x).abs() <= 1 && (q.y - r.y).abs() <= 1 && (q.z - r.z).abs() <= 1).cloned().collect();
        near.sort_by_key(|(q, _)| (q.x - r.x).abs() + (q.y - r.y).abs() + (q.z - r.z).abs());
        // (at most 4 neighbours: 5 cells, 120 orders x 32 rail facings, so a dense rail field stays
        // bounded; the nearest first)
        near.truncate(4);
        let mut cells = vec![(*r, rb.clone())];
        cells.extend(near);
        let want = rb.to_string().split("shape=").nth(1).and_then(|s| s.split([',', ']']).next()).unwrap_or("-").to_string();
        let (reach, built, first) = plain_reach(blocks, *r, &cells, &want)?;
        if reach > 0 {
            plain_ok += 1;
            if reach < built {
                plain_part.push(format!("{r} ({want}) {reach} of {built}, e.g. {}", first.unwrap_or_default()));
            }
        } else if let Some(d) = seqs.iter().find(|d| d.steps.iter().any(|(p, _, _)| p == r)) {
            plain_info.push(format!("{r} ({want}): no plain order reaches it (0 of {built} builds); laid by the declared sequence \"{}\"", d.name));
        } else {
            plain_fail.push(format!("{r} ({want}): no plain order reaches it (0 of {built} builds), and no declared sequence lays it"));
        }
    }
    failing += plain_fail.len();
    if rails.is_empty() {
        failing += 1;
    }
    lines.push(format!(
        "build-order plain orders over every rail cell ({} rails: each with up to 4 rails of its 3x3x3 box, every other design block within 3 blocks laid RAW first (the window's assumption); every order the game allows x each rail's two placement facings; a rail passes when at least one plain build reaches its designed shape, ironspec 02:46 (5); a window's pass doesn't make the rails one buildable order: the global order below is that claim): {}{}{}{}",
        rails.len(),
        if rails.is_empty() { "NO RAIL: nothing judged, a failure".to_string() } else { format!("{plain_ok} reach it") },
        if plain_part.is_empty() { String::new() } else { format!("; below N of N (reaching / total, one reaching order): {}", plain_part.join("; ")) },
        if plain_info.is_empty() { String::new() } else { format!("; information (a declared sequence lays them): {}", plain_info.join("; ")) },
        if plain_fail.is_empty() { String::new() } else { format!("; FAILING: {}", plain_fail.join("; ")) }
    ));
    // (G1 on G (5): ONE GLOBAL ORDER on the whole design, the guide's claim)
    let rest: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| !named_all.contains(p)).cloned().collect();
    let alone = |b: &Block| redsim::behavior::can_survive(&World::new(BlockPos::new(0, 64, 0), [3, 3, 3]), b, BlockPos::new(1, 65, 1));
    type Key = fn(&BlockPos) -> (i32, i32, i32);
    let sweeps: [(&str, Key); 4] = [("y, z, x", |p| (p.y, p.z, p.x)), ("y, x, z", |p| (p.y, p.x, p.z)), ("y, -z, x", |p| (p.y, -p.z, p.x)), ("y, z, -x", |p| (p.y, p.z, -p.x))];
    let mut global: Result<&str, Vec<String>> = Err(Vec::new());
    for (name, key) in sweeps {
        let mut w = World::new(lo, size);
        let mut v = rest.clone();
        v.sort_by_key(|(p, b)| (!alone(b), key(p)));
        let mut refused = Vec::new();
        for (p, b) in &v {
            let ew = b.to_string().contains("shape=east_west") || b.to_string().contains("shape=ascending_east") || b.to_string().contains("shape=ascending_west");
            let st = placed_state(b, ew)?;
            if !redsim::behavior::can_survive(&w, &st, *p) {
                refused.push(format!("{p} {} refused (it can't survive there yet)", short(b)));
                continue;
            }
            w.set_block(*p, st, flags::ALL);
        }
        let (mut trail, mut bad) = (Vec::new(), Vec::new());
        for d in &seqs {
            run_steps(&mut w, d, &mut trail, &mut bad)?;
        }
        let mut off = read_back(&w, blocks, &named_all);
        off.extend(refused);
        off.extend(bad);
        if off.is_empty() {
            global = Ok(name);
            break;
        }
        if let Err(e) = &mut global {
            e.push(format!("[{name}] {}", off.join("; ")));
        }
    }
    match &global {
        Ok(name) => lines.push(format!("build-order one global order on the whole design (the guide's claim: the blocks that stand alone first, then the rest, a sweep {name}, each rail facing its designed axis, then the declared sequences in their order): every design cell as designed")),
        Err(e) => {
            failing += 1;
            lines.push(format!("build-order one global order on the whole design (four sweeps tried, the blocks that stand alone first, then the rest, each rail facing its designed axis, then the declared sequences): NONE reproduces the design (FAILS): {}", e.join(" | ")));
        }
    }
    Ok((lines, failing))
}

/// A state without its namespace, for the reports.
fn short(b: &Block) -> String {
    b.to_string().trim_start_matches("minecraft:").to_string()
}

/// Every design cell, and every given step cell the design leaves empty (air), read back: the cells off.
fn read_back(w: &World, blocks: &[(BlockPos, Block)], steps: &[BlockPos]) -> Vec<String> {
    let mut off = Vec::new();
    for (p, b) in blocks {
        let got = w.get(*p);
        if !same_build(b, &got) {
            off.push(format!("{p}: {} (the design: {})", short(&got), short(b)));
        }
    }
    let mut seen: Vec<BlockPos> = Vec::new();
    for p in steps {
        if seen.contains(p) || blocks.iter().any(|(q, _)| q == p) {
            continue;
        }
        seen.push(*p);
        let got = w.get(*p);
        if !got.is_air() {
            off.push(format!("{p}: {} (the design: air)", short(&got)));
        }
    }
    off
}

/// One declared sequence's steps on a world: each placement through the game's check first (it must
/// survive where it is placed; then no rail within a block of it may pop), placed with its updates, its
/// result checked against the step's state; a placement over a pending temporary clears it; the
/// temporaries left at the end removed. The trail and the failures are appended.
fn run_steps(w: &mut World, d: &Declared, trail: &mut Vec<String>, bad: &mut Vec<String>) -> Result<(), String> {
    let air = Block::parse("minecraft:air").map_err(|e| e.to_string())?;
    let mut pending: Vec<BlockPos> = Vec::new();
    for (i, (p, b, temp)) in d.steps.iter().enumerate() {
        if b.is_air() {
            w.set_block(*p, b.clone(), flags::ALL);
            pending.retain(|q| q != p);
            trail.push(format!("{p} removed"));
            continue;
        }
        let st = placed_state(b, false)?;
        if !redsim::behavior::can_survive(w, &st, *p) {
            bad.push(format!("\"{}\" step {} {p} {}: the game refuses the placement (it can't survive there)", d.name, i + 1, short(b)));
            trail.push(format!("{p} {} REFUSED", short(b)));
            continue;
        }
        w.set_block(*p, st, flags::ALL);
        pending.retain(|q| q != p);
        if *temp {
            pending.push(*p);
        }
        let got = w.get(*p);
        trail.push(format!("{}{p} {}", if *temp { "~" } else { "" }, short(&got)));
        if !same_build(b, &got) {
            bad.push(format!("\"{}\" step {} {p}: declared {}, placed as {}", d.name, i + 1, short(b), short(&got)));
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let q = BlockPos::new(p.x + dx, p.y + dy, p.z + dz);
                    if redsim::rails::would_pop(w, q) {
                        bad.push(format!("\"{}\" step {} {p}: the rail at {q} would pop", d.name, i + 1));
                    }
                }
            }
        }
    }
    for p in &pending {
        w.set_block(*p, air.clone(), flags::ALL);
        trail.push(format!("{p} (temporary) removed at the end"));
    }
    Ok(())
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
        assert!(lines.iter().any(|l| l.contains("read back, EVERY design cell") && l.contains("all as designed")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("1,0,3") && l.contains("laid by the declared sequence \"park\"")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("one global order") && l.contains("every design cell as designed")), "{lines:?}");
        let (lines, failing) = super::run_declared(&blocks, &seqs, &["park".to_string()]).unwrap();
        assert!(failing >= 1, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("FAILING") && l.contains("1,0,3") && l.contains("no declared sequence lays it")), "{lines:?}");
    }

    /// (G′, G1 on G (7)) The read-back's failing twins: the park WITHOUT its temporary step places the
    /// park rail flat (it fails by name at (1,0,3)); the helper placed BEFORE its support is refused by
    /// the game's placement check; zero declared sequences judge nothing (a failure).
    #[test]
    fn the_read_back_and_the_placement_check_fail_by_name() {
        let (blocks, _) = park();
        let no_helper = super::declared("# build-order: park=2,0,3=minecraft:slime_block;1,0,3=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]").unwrap();
        let (lines, failing) = super::run_declared(&blocks, &no_helper, &[]).unwrap();
        assert!(failing >= 1, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("\"park\"") && l.contains("1,0,3") && (l.contains("placed as") || l.contains("OFF"))), "{lines:?}");
        let early = super::declared("# build-order: park=~2,1,3=minecraft:rail[shape=north_south,waterlogged=false];2,0,3=minecraft:slime_block;1,0,3=minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false];2,1,3=air").unwrap();
        let (lines, failing) = super::run_declared(&blocks, &early, &[]).unwrap();
        assert!(failing >= 1, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("the game refuses the placement")), "{lines:?}");
        let (lines, failing) = super::run_declared(&blocks, &[], &[]).unwrap();
        assert!(failing >= 1 && lines.iter().any(|l| l.contains("NO DECLARED SEQUENCE")), "{lines:?}");
    }

    /// (G′, G1 08:32 (2c)) The station's popping pair as the global order's FAILING FIXTURE: lane 7's
    /// crest (-3,13,7) and the last climb slope (-2,12,7) without the lift's glass (-3,12,7) can't be
    /// placed (a rail needs a rigid block under it, an ascending one also on its high side: 26.3
    /// BaseRailBlock.shouldBeRemoved): the global order fails naming them; with the glass it reproduces
    /// the design.
    #[test]
    fn the_global_order_refuses_rails_without_their_support() {
        let b = |s: &str| redsim::block::Block::parse(s).unwrap();
        let p = redsim::pos::BlockPos::new;
        let pair = vec![
            (p(-2, 11, 7), b("minecraft:glass")),
            (p(-2, 12, 7), b("minecraft:rail[shape=ascending_west,waterlogged=false]")),
            (p(-3, 13, 7), b("minecraft:rail[shape=east_west,waterlogged=false]")),
            (p(-4, 12, 7), b("minecraft:glass")),
        ];
        let seqs = super::declared("# build-order: s=-4,12,7=minecraft:glass").unwrap();
        let (lines, failing) = super::run_declared(&pair, &seqs, &[]).unwrap();
        assert!(failing >= 1, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("one global order") && l.contains("NONE reproduces") && (l.contains("(-3,13,7)") || l.contains("(-2,12,7)"))), "{lines:?}");
        let mut held = pair.clone();
        held.push((p(-3, 12, 7), b("minecraft:glass")));
        let (lines, _) = super::run_declared(&held, &seqs, &[]).unwrap();
        assert!(lines.iter().any(|l| l.contains("one global order") && l.contains("every design cell as designed")), "{lines:?}");
    }

    #[test]
    fn a_malformed_build_order_line_is_refused() {
        assert!(super::declared("# build-order: =1,0,0=stone").is_err());
        assert!(super::declared("# build-order: p q=1,0,0=stone").is_err(), "a NAME with a space");
        assert!(super::declared("# build-order: p=1,0=stone").is_err());
        assert!(super::declared("# build-order: p=1,0,0").is_err(), "a step without =");
        assert!(super::declared("# build-order: p=").is_err(), "an empty sequence");
        assert!(super::declared("# build-order: p=~1,0,0=air").is_err());
        assert!(super::declared("# build-order: p=1,0,0=minecraft:notablock").is_err(), "an unknown block");
        assert!(super::declared("# build-order: p=1,0,0=minecraft:stone[shape=north_south]").is_err(), "a property the block doesn't have");
        assert!(super::declared("# build-order: p=1,0,0=stone\n# build-order: p=2,0,0=stone").is_err());
        let (blocks, _) = park();
        assert!(super::run_declared(&blocks, &super::declared("# build-order: p=1,0,3=minecraft:stone").unwrap(), &["q".to_string()]).is_err());
        assert!(super::run_declared(&blocks, &super::declared("# build-order: p=9,9,9=minecraft:stone;0,0,3=minecraft:rail[shape=east_west,waterlogged=false]").unwrap(), &[]).is_err(), "a step outside the design's box");
        assert!(super::run_declared(&blocks, &super::declared("# build-order: junk=2,-1,3=minecraft:stone").unwrap(), &[]).is_err(), "a sequence naming no design cell");
    }
}
