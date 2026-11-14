//! `mctas design`: checks of the design text format itself.

use anyhow::{bail, Result};
use clap::Subcommand;
use mctas::design::{interior_blank_rows, BlankRows, Design};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum DesignCmd {
    /// (farmopt, Nov 12 2026, ironspec's ruling) List every whitespace-only line inside a layer
    /// that more rows of the same layer follow, in the design files (.txt) under the PATHs, and
    /// compare the two readings cell by cell: the old parser's (no row: the layer's later rows
    /// move north) and an empty row. The parser now refuses such a line, naming it. `--rewrite
    /// skip|air` writes each listed file in the explicit form of that reading (the line deleted,
    /// or `  .`) after checking that the rewritten text parses to exactly that reading.
    BlankRows {
        paths: Vec<PathBuf>,
        /// Rewrite each listed file in the explicit form of this reading: skip or air.
        #[arg(long)]
        rewrite: Option<String>,
    },
    /// (farmopt, Nov 12 2026, ironspec's ruling after onechunk found ironfarm's lane top slopes
    /// unsupported) List every rail in the design files (.txt) under the PATHs that would pop on
    /// its next neighbour update: 26.3's `BaseRailBlock.shouldBeRemoved` (redsim's port) — no
    /// rigid top below, or an ascending rail with no rigid block on its high side. The overworld
    /// blocks and the Nether part are each pasted raw into their own world. A rail beside a block
    /// redsim can't parse is reported as unjudged, not as a pop.
    WouldPop { paths: Vec<PathBuf> },
    /// (ironspec 09:05a / 09:09a: a design file in a train is READ by a selected piece) A V owner file (.delta: an
    /// "x,y,z state" line lays that block, a bare "x,y,z" line clears the cell to air, `#` lines skipped) applied to its
    /// base design (`--base`, else the delta's `# base: PATH` line): its cells laid with their updates into the base's
    /// world, then judged on the delta's cells and their six neighbours only: no rail there would pop, and every laid cell
    /// reads back as declared. Prints both files' md5s; exits non-zero on a failure or a missing base.
    DeltaCheck {
        delta: PathBuf,
        #[arg(long)]
        base: Option<PathBuf>,
    },
}

pub fn run(cmd: DesignCmd) -> Result<()> {
    match cmd {
        DesignCmd::BlankRows { paths, rewrite } => blank_rows(&paths, rewrite.as_deref()),
        DesignCmd::WouldPop { paths } => would_pop(&paths),
        DesignCmd::DeltaCheck { delta, base } => delta_check(&delta, base.as_deref()),
    }
}

/// A file's md5 as hex.
fn md5_hex(bytes: &[u8]) -> String {
    redsim::stems::md5(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The cells of a V owner file: laid (`Some(state)`) or cleared (`None`), in file order, and its `# base:` line.
fn delta_cells(text: &str) -> Result<(Vec<([i32; 3], Option<String>)>, Option<String>)> {
    let mut v = Vec::new();
    let mut base = None;
    for line in text.lines() {
        let l = line.trim();
        if let Some(b) = l.strip_prefix("# base:") {
            base.get_or_insert(b.trim().to_string());
            continue;
        }
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (p, s) = match l.split_once(char::is_whitespace) {
            Some((p, s)) => (p, Some(s.trim().to_string())),
            None => (l, None),
        };
        let c: Vec<i32> = p.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("{l:?}: {e}"))?;
        if c.len() != 3 {
            bail!("{l:?}: x,y,z [state]");
        }
        v.push(([c[0], c[1], c[2]], s));
    }
    Ok((v, base))
}

fn delta_check(delta: &Path, base: Option<&Path>) -> Result<()> {
    use redsim::pos::BlockPos;
    let dbytes = std::fs::read(delta)?;
    let dtext = String::from_utf8_lossy(&dbytes).to_string();
    let (cells, header_base) = delta_cells(&dtext)?;
    let base: PathBuf = match (base, header_base) {
        (Some(b), _) => b.to_path_buf(),
        (None, Some(b)) => PathBuf::from(b),
        (None, None) => bail!("{}: no --base and no `# base: PATH` line: the delta names no design to apply to", delta.display()),
    };
    let Ok(bbytes) = std::fs::read(&base) else {
        bail!("{}: its base {} is not in this tree: the delta can't be applied (FAIL)", delta.display(), base.display());
    };
    let btext = String::from_utf8_lossy(&bbytes).to_string();
    let d = Design::parse(&btext).map_err(|e| anyhow::anyhow!("{}: {e}", base.display()))?;
    // the base's world, pasted raw, one cell larger than the base and the delta together on every side
    let mut all: BTreeMap<[i32; 3], String> = d.blocks.clone();
    for (p, s) in &cells {
        all.entry(*p).or_insert_with(|| s.clone().unwrap_or_else(|| "minecraft:air".to_string()));
    }
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in all.keys() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if lo[0] > hi[0] {
        bail!("{}: neither the base nor the delta lays a block", delta.display());
    }
    let mut w = redsim::world::World::new(BlockPos::new(lo[0] - 1, lo[1] - 1, lo[2] - 1), [hi[0] - lo[0] + 3, hi[1] - lo[1] + 3, hi[2] - lo[2] + 3]);
    let mut raw = Vec::new();
    for (p, s) in &d.blocks {
        if let Ok(b) = redsim::block::Block::parse(s) {
            raw.push((BlockPos::new(p[0], p[1], p[2]), b));
        }
    }
    w.place_raw(&raw);
    // the delta's cells laid with their updates, in file order
    let mut unparsed = Vec::new();
    for (p, s) in &cells {
        let s = s.clone().unwrap_or_else(|| "minecraft:air".to_string());
        match redsim::block::Block::parse(&s) {
            Ok(b) => {
                w.set_block(BlockPos::new(p[0], p[1], p[2]), b, redsim::world::flags::ALL);
            }
            Err(e) => unparsed.push(format!("{},{},{} {s}: {e}", p[0], p[1], p[2])),
        }
    }
    // judged: the delta's cells and their six neighbours
    let mut scope: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (p, _) in &cells {
        scope.insert(*p);
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            scope.insert([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }
    let mut pops = Vec::new();
    for p in &scope {
        let bp = BlockPos::new(p[0], p[1], p[2]);
        let b = w.get(bp);
        if redsim::rails::is_rail(&b) && (redsim::rails::would_pop(&w, bp) || !redsim::behavior::can_survive(&w, &b, bp)) {
            pops.push(format!("{},{},{} {b}", p[0], p[1], p[2]));
        }
    }
    let mut mismatched = Vec::new();
    for (p, s) in &cells {
        let want = s.clone().unwrap_or_else(|| "minecraft:air".to_string());
        let Ok(wb) = redsim::block::Block::parse(&want) else { continue };
        let got = w.get(BlockPos::new(p[0], p[1], p[2]));
        if got != wb {
            mismatched.push(format!("{},{},{} declared {wb}, reads back {got}", p[0], p[1], p[2]));
        }
    }
    let laid = cells.iter().filter(|(_, s)| s.is_some()).count();
    println!(
        "delta-check {} (md5 {}) on {} (md5 {}): {} cell(s) laid, {} cleared, judged on {} cell(s) (the delta's and their neighbours)",
        delta.display(),
        md5_hex(&dbytes),
        base.display(),
        md5_hex(&bbytes),
        laid,
        cells.len() - laid,
        scope.len()
    );
    println!("  would pop: {}", if pops.is_empty() { "none".to_string() } else { pops.join("; ") });
    println!("  read back: {}", if mismatched.is_empty() { "every laid cell as declared".to_string() } else { mismatched.join("; ") });
    if !unparsed.is_empty() {
        println!("  states redsim can't parse: {}", unparsed.join("; "));
    }
    let pass = pops.is_empty() && mismatched.is_empty() && unparsed.is_empty();
    println!("delta-check: {}", if pass { "PASS" } else { "FAIL" });
    if !pass {
        std::process::exit(1);
    }
    Ok(())
}

/// The rails of one part (its blocks pasted raw into a world one cell larger on every side) that
/// would pop, and those left unjudged: a rail whose own state redsim can't parse, or one with a
/// neighbour (below or beside) that doesn't parse.
fn popping_rails(blocks: &BTreeMap<[i32; 3], String>) -> (Vec<[i32; 3]>, Vec<[i32; 3]>) {
    use redsim::pos::BlockPos;
    let Some(first) = blocks.keys().next() else { return (Vec::new(), Vec::new()) };
    let (mut lo, mut hi) = (*first, *first);
    for p in blocks.keys() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let mut w = redsim::world::World::new(BlockPos::new(lo[0] - 1, lo[1] - 1, lo[2] - 1), [hi[0] - lo[0] + 3, hi[1] - lo[1] + 3, hi[2] - lo[2] + 3]);
    let mut cells = Vec::new();
    let mut unparsed: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut unparsed_rails: Vec<[i32; 3]> = Vec::new();
    for (p, s) in blocks {
        match redsim::block::Block::parse(s) {
            Ok(b) => cells.push((BlockPos::new(p[0], p[1], p[2]), b)),
            Err(_) => {
                unparsed.insert(*p);
                // a rail redsim can't parse is itself unjudged (G1's note on 751a7e063)
                let name = s.strip_prefix("minecraft:").unwrap_or(s).split('[').next().unwrap_or("");
                if name.ends_with("rail") {
                    unparsed_rails.push(*p);
                }
            }
        }
    }
    w.place_raw(&cells);
    let (mut pops, mut unjudged) = (Vec::new(), unparsed_rails);
    for p in blocks.keys() {
        let bp = BlockPos::new(p[0], p[1], p[2]);
        if !redsim::rails::would_pop(&w, bp) {
            continue;
        }
        let near = [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d| unparsed.contains(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]));
        if near {
            unjudged.push(*p);
        } else {
            pops.push(*p);
        }
    }
    (pops, unjudged)
}

fn would_pop(paths: &[PathBuf]) -> Result<()> {
    let mut files = Vec::new();
    for p in paths {
        walk(p, &mut files)?;
    }
    let (mut designs, mut failing, mut cells, mut unjudged_files) = (0usize, 0usize, 0usize, 0usize);
    // .txt files that don't parse as designs (notes, tables, refused files): counted and named,
    // not silently skipped (G1's note on 751a7e063)
    let mut not_designs: Vec<&PathBuf> = Vec::new();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            not_designs.push(f);
            continue;
        };
        let Ok(d) = Design::parse(&text) else {
            not_designs.push(f);
            continue;
        };
        designs += 1;
        let (pops, unjudged) = popping_rails(&d.blocks);
        let (npops, nunjudged) = popping_rails(&d.nether_blocks);
        let show = |v: &[[i32; 3]]| v.iter().take(8).map(|p| format!("{},{},{}", p[0], p[1], p[2])).collect::<Vec<_>>().join(" ");
        if !pops.is_empty() || !npops.is_empty() {
            failing += 1;
            cells += pops.len() + npops.len();
            println!(
                "{}: {} rail(s) would pop: {}{}",
                f.display(),
                pops.len() + npops.len(),
                show(&pops),
                if npops.is_empty() { String::new() } else { format!(" (Nether: {})", show(&npops)) }
            );
        }
        if !unjudged.is_empty() || !nunjudged.is_empty() {
            unjudged_files += 1;
            println!("{}: {} rail(s) unjudged (the rail or a neighbour redsim can't parse): {}", f.display(), unjudged.len() + nunjudged.len(), show(&unjudged));
        }
    }
    println!("{designs} design file(s) read; {failing} with rails that would pop ({cells} cells); {unjudged_files} with unjudged rails");
    if !not_designs.is_empty() {
        let shown: Vec<String> = not_designs.iter().take(5).map(|p| p.display().to_string()).collect();
        println!("{} .txt file(s) not read as designs (they don't parse): {}{}", not_designs.len(), shown.join(", "), if not_designs.len() > 5 { ", ..." } else { "" });
    }
    Ok(())
}

/// Every .txt under `p` (or `p` itself), in path order.
fn walk(p: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if p.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(p)?.map(|e| e.map(|e| e.path())).collect::<std::io::Result<_>>()?;
        entries.sort();
        for e in entries {
            walk(&e, out)?;
        }
    } else if p.extension().is_some_and(|e| e == "txt") {
        out.push(p.to_path_buf());
    }
    Ok(())
}

/// Cells (overworld and Nether) whose block differs between two readings of one file.
fn differing_cells(a: &Design, b: &Design) -> usize {
    fn diff(x: &BTreeMap<[i32; 3], String>, y: &BTreeMap<[i32; 3], String>) -> usize {
        x.iter().filter(|(p, s)| y.get(*p) != Some(*s)).count() + y.keys().filter(|p| !x.contains_key(*p)).count()
    }
    diff(&a.blocks, &b.blocks) + diff(&a.nether_blocks, &b.nether_blocks)
}

/// The text with each listed line deleted (skip) or written as an empty row `  .` (air), every
/// other byte kept.
fn rewritten(text: &str, lines: &[usize], mode: BlankRows) -> String {
    let set: BTreeSet<usize> = lines.iter().copied().collect();
    let mut out = String::with_capacity(text.len() + 4 * lines.len());
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if !set.contains(&i) {
            out.push_str(l);
        } else if mode == BlankRows::Air {
            let nl = if l.ends_with("\r\n") {
                "\r\n"
            } else if l.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str("  .");
            out.push_str(nl);
        }
    }
    out
}

fn blank_rows(paths: &[PathBuf], rewrite: Option<&str>) -> Result<()> {
    let mode = match rewrite {
        None => None,
        Some("skip") => Some(BlankRows::Skip),
        Some("air") => Some(BlankRows::Air),
        Some(o) => bail!("--rewrite takes skip or air, not {o}"),
    };
    let mut files = Vec::new();
    for p in paths {
        walk(p, &mut files)?;
    }
    let (mut listed, mut differ, mut not_design, mut written) = (0usize, 0usize, 0usize, 0usize);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let lines = interior_blank_rows(&text);
        if lines.is_empty() {
            continue;
        }
        let (skip, air) = match (Design::parse_with(&text, BlankRows::Skip), Design::parse_with(&text, BlankRows::Air)) {
            (Ok(s), Ok(a)) => (s, a),
            _ => {
                not_design += 1;
                println!("{}: {} blank row(s), not a design (does not parse either way)", f.display(), lines.len());
                continue;
            }
        };
        listed += 1;
        let n = differing_cells(&skip, &air);
        if n > 0 {
            differ += 1;
        }
        let shown: Vec<String> = lines.iter().take(8).map(|l| (l + 1).to_string()).collect();
        let more = if lines.len() > 8 { ",..." } else { "" };
        println!("{}: {} blank row(s) inside a layer at line {}{more}; skip vs air: {n} cell(s) differ", f.display(), lines.len(), shown.join(","));
        if let Some(m) = mode {
            let new_text = rewritten(&text, &lines, m);
            let want = if m == BlankRows::Skip { &skip } else { &air };
            let got = Design::parse(&new_text).map_err(|e| anyhow::anyhow!("{}: the rewrite does not parse: {e}", f.display()))?;
            if &got != want {
                bail!("{}: the rewrite does not parse to the chosen reading", f.display());
            }
            std::fs::write(f, new_text)?;
            written += 1;
        }
    }
    println!(
        "{listed} design file(s) with blank rows inside a layer, {differ} where the two readings differ; {not_design} other file(s) with such lines that parse neither way{}",
        if mode.is_some() { format!("; {written} rewritten") } else { String::new() }
    );
    Ok(())
}
