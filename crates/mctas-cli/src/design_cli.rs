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
    /// (tickexact, Nov 14 2026, G1 04:10 (a): does a row run on one revision of a file stand for another?) Compare two
    /// design files cell by cell: every cell only in A, only in B, or with a different state (air written or not counts
    /// as absent), then the entities (kind, position, NBT). `--box x0,y0,z0,x1,y1,z1` limits the cells compared
    /// (inclusive). Exits 0 either way; the counts line says whether they differ.
    CellDiff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long = "box", allow_hyphen_values = true)]
        within: Option<String>,
    },
}

pub fn run(cmd: DesignCmd) -> Result<()> {
    match cmd {
        DesignCmd::BlankRows { paths, rewrite } => blank_rows(&paths, rewrite.as_deref()),
        DesignCmd::WouldPop { paths } => would_pop(&paths),
        DesignCmd::CellDiff { a, b, within } => cell_diff(&a, &b, within.as_deref()),
    }
}

fn cell_diff(a: &Path, b: &Path, within: Option<&str>) -> Result<()> {
    let bx: Option<[i32; 6]> = match within {
        Some(s) => {
            let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            if v.len() != 6 {
                bail!("--box x0,y0,z0,x1,y1,z1, got {s}");
            }
            Some([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5]), v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])])
        }
        None => None,
    };
    let inside = |p: &[i32; 3]| bx.is_none_or(|q| (0..3).all(|i| p[i] >= q[i] && p[i] <= q[i + 3]));
    let cells = |d: &Design| -> BTreeMap<[i32; 3], String> { d.block_list().into_iter().filter(|(p, s)| s != "minecraft:air" && inside(p)).collect() };
    let (da, db) = (Design::load(a)?, Design::load(b)?);
    let (ca, cb) = (cells(&da), cells(&db));
    let mut only_a = 0usize;
    let mut only_b = 0usize;
    let mut changed = 0usize;
    for (p, s) in &ca {
        match cb.get(p) {
            None => {
                only_a += 1;
                println!("  only in A ({},{},{}): {s}", p[0], p[1], p[2]);
            }
            Some(t) if t != s => {
                changed += 1;
                println!("  changed ({},{},{}): {s} -> {t}", p[0], p[1], p[2]);
            }
            Some(_) => {}
        }
    }
    for (p, t) in &cb {
        if !ca.contains_key(p) {
            only_b += 1;
            println!("  only in B ({},{},{}): {t}", p[0], p[1], p[2]);
        }
    }
    let ent = |d: &Design| -> Vec<String> { d.entities.iter().map(|e| format!("{} at ({}, {}, {}) {}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.nbt.clone().unwrap_or_default())).collect() };
    let (ea, eb) = (ent(&da), ent(&db));
    let ent_same = ea == eb;
    if !ent_same {
        for e in ea.iter().filter(|e| !eb.contains(e)) {
            println!("  entity only in A: {e}");
        }
        for e in eb.iter().filter(|e| !ea.contains(e)) {
            println!("  entity only in B: {e}");
        }
    }
    println!(
        "{} vs {}{}: {} cells in A, {} in B; {only_a} only in A, {only_b} only in B, {changed} changed; entities {}",
        a.display(),
        b.display(),
        bx.map(|q| format!(" within ({},{},{})..({},{},{})", q[0], q[1], q[2], q[3], q[4], q[5])).unwrap_or_default(),
        ca.len(),
        cb.len(),
        if ent_same { format!("the same ({})", ea.len()) } else { format!("DIFFER ({} vs {})", ea.len(), eb.len()) }
    );
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
