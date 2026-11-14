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
    /// design files FIELD BY FIELD (G1 06:54 (1), 08:16 (1)): the name, every header `key: value` line (keep-block,
    /// emulated, keep-clear, riderless, run-cells, positions …), the ports, every cell (air written or not counts as
    /// absent), the entities in file order (kind, position, NBT), the block entities (`nbt:` lines), the Nether part's
    /// cells, ports and block entities, and the DECLARATION comments mobfarm reads (`# key: …` with a key in
    /// mobfarm::declkeys: contacts, stops, shape, water …); prose comments never differ. `--box x0,y0,z0,x1,y1,z1` limits
    /// the overworld cells, entities and block entities compared (inclusive; the Nether part and the declarations are
    /// compared whole). Exits 1 on any difference, naming each one.
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

fn parse_box(within: Option<&str>) -> Result<Option<[i32; 6]>> {
    Ok(match within {
        Some(s) => {
            let v: Vec<i32> = s.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            if v.len() != 6 {
                bail!("--box x0,y0,z0,x1,y1,z1, got {s}");
            }
            Some([v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5]), v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])])
        }
        None => None,
    })
}

/// The declaration lines of a design file's text (G1 08:16 (1)): every `# key: …` comment whose key is one mobfarm reads
/// from a comment (`mobfarm::declkeys::COMMENT_KEYS`, and the either-form `EITHER_KEYS`), as (key, the line trimmed), in
/// file order; a prose comment is not a declaration
fn declarations(text: &str) -> Vec<(String, String)> {
    let known = |k: &str| mobfarm::declkeys::COMMENT_KEYS.contains(&k) || mobfarm::declkeys::EITHER_KEYS.contains(&k);
    text.lines()
        .filter_map(|l| {
            let body = l.trim().strip_prefix('#')?.trim();
            let (key, _) = body.split_once(':')?;
            let key = key.split_whitespace().collect::<Vec<_>>().join(" ");
            known(&key).then(|| (key, l.trim().to_string()))
        })
        .collect()
}

/// Every difference between two designs, field by field, one line each (empty: the designs are equal within `bx`);
/// `ta` / `tb` are the files' texts, for their declaration lines
fn design_differences(da: &Design, db: &Design, bx: Option<[i32; 6]>, ta: &str, tb: &str) -> Vec<String> {
    let inside = |p: &[i32; 3]| bx.is_none_or(|q| (0..3).all(|i| p[i] >= q[i] && p[i] <= q[i + 3]));
    let inside_f = |p: &[f64; 3]| bx.is_none_or(|q| (0..3).all(|i| p[i] >= q[i] as f64 && p[i] < (q[i + 3] + 1) as f64));
    let mut out: Vec<String> = Vec::new();
    if da.name != db.name {
        out.push(format!("name: {:?} -> {:?}", da.name, db.name));
    }
    fn map_diff<K: Ord + std::fmt::Debug, V: PartialEq + std::fmt::Debug>(what: &str, a: &BTreeMap<K, V>, b: &BTreeMap<K, V>, keep: impl Fn(&K) -> bool, out: &mut Vec<String>) {
        for (k, v) in a.iter().filter(|(k, _)| keep(k)) {
            match b.get(k) {
                None => out.push(format!("{what} only in A {k:?}: {v:?}")),
                Some(w) if w != v => out.push(format!("{what} changed {k:?}: {v:?} -> {w:?}")),
                Some(_) => {}
            }
        }
        for (k, w) in b.iter().filter(|(k, _)| keep(k)) {
            if !a.contains_key(k) {
                out.push(format!("{what} only in B {k:?}: {w:?}"));
            }
        }
    }
    map_diff("header line", &da.meta, &db.meta, |_| true, &mut out);
    map_diff("port", &da.ports, &db.ports, |_| true, &mut out);
    let cells = |d: &Design| -> BTreeMap<[i32; 3], String> { d.blocks.iter().filter(|(_, s)| s.as_str() != "minecraft:air").map(|(p, s)| (*p, s.clone())).collect() };
    map_diff("cell", &cells(da), &cells(db), |p| inside(p), &mut out);
    let ents = |d: &Design| -> Vec<String> { d.entities.iter().filter(|e| inside_f(&e.pos)).map(|e| format!("{} at ({}, {}, {}) {}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.nbt.clone().unwrap_or_default())).collect() };
    let (ea, eb) = (ents(da), ents(db));
    for i in 0..ea.len().max(eb.len()) {
        match (ea.get(i), eb.get(i)) {
            (Some(x), Some(y)) if x != y => out.push(format!("entity #{i}: {x} -> {y}")),
            (Some(x), None) => out.push(format!("entity #{i} only in A: {x}")),
            (None, Some(y)) => out.push(format!("entity #{i} only in B: {y}")),
            _ => {}
        }
    }
    map_diff("block entity", &da.block_entities, &db.block_entities, |p| inside(p), &mut out);
    map_diff("nether cell", &da.nether_blocks, &db.nether_blocks, |_| true, &mut out);
    map_diff("nether port", &da.nether_ports, &db.nether_ports, |_| true, &mut out);
    map_diff("nether block entity", &da.nether_block_entities, &db.nether_block_entities, |_| true, &mut out);
    // the declaration lines, per key in file order (a prose comment never differs)
    let (dla, dlb) = (declarations(ta), declarations(tb));
    let keys: std::collections::BTreeSet<&String> = dla.iter().chain(&dlb).map(|(k, _)| k).collect();
    for k in keys {
        let la: Vec<&String> = dla.iter().filter(|(q, _)| q == k).map(|(_, l)| l).collect();
        let lb: Vec<&String> = dlb.iter().filter(|(q, _)| q == k).map(|(_, l)| l).collect();
        if la != lb {
            for l in la.iter().filter(|l| !lb.contains(l)) {
                out.push(format!("declaration `# {k}:` only in A: {l}"));
            }
            for l in lb.iter().filter(|l| !la.contains(l)) {
                out.push(format!("declaration `# {k}:` only in B: {l}"));
            }
            if la.iter().all(|l| lb.contains(l)) && lb.iter().all(|l| la.contains(l)) {
                out.push(format!("declaration `# {k}:` lines in another order or count ({} in A, {} in B)", la.len(), lb.len()));
            }
        }
    }
    out
}

fn cell_diff(a: &Path, b: &Path, within: Option<&str>) -> Result<()> {
    let bx = parse_box(within)?;
    let (da, db) = (Design::load(a)?, Design::load(b)?);
    let (ta, tb) = (std::fs::read_to_string(a)?, std::fs::read_to_string(b)?);
    let diffs = design_differences(&da, &db, bx, &ta, &tb);
    // (G1 08:16 non-blocking) the cross-check: with no box, no field difference must mean the parsed designs are equal
    if bx.is_none() && diffs.iter().all(|d| d.starts_with("declaration")) && da != db {
        bail!("{} and {}: no field difference was found but the parsed designs differ (a field cell-diff does not compare)", a.display(), b.display());
    }
    for d in &diffs {
        println!("  {d}");
    }
    let n = |d: &Design| d.blocks.values().filter(|s| s.as_str() != "minecraft:air").count();
    println!(
        "{} vs {}{}: {} cells in A, {} in B, {} entities in A, {} in B; {}",
        a.display(),
        b.display(),
        bx.map(|q| format!(" within ({},{},{})..({},{},{})", q[0], q[1], q[2], q[3], q[4], q[5])).unwrap_or_default(),
        n(&da),
        n(&db),
        da.entities.len(),
        db.entities.len(),
        if diffs.is_empty() { "EQUAL field by field".to_string() } else { format!("{} difference(s)", diffs.len()) }
    );
    if !diffs.is_empty() {
        bail!("{} and {} differ: {} difference(s)", a.display(), b.display(), diffs.len());
    }
    Ok(())
}

#[cfg(test)]
mod cell_diff_tests {
    use super::*;

    const A: &str = "name: t\n# a comment\nkeep-block: 1 0 0\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  G minecraft:glass\nlayer 0:\n  SG\n  S.\nentity: minecraft:minecart 0.5 1 0.5\nnbt: 0 0 0 {Items:[]}\n";

    fn diffs(a: &str, b: &str) -> Vec<String> {
        design_differences(&Design::parse(a).expect("A parses"), &Design::parse(b).expect("B parses"), None, a, b)
    }

    #[test]
    fn identical_designs_are_equal() {
        assert!(diffs(A, A).is_empty());
    }

    #[test]
    fn a_comment_only_change_is_equal() {
        assert!(diffs(A, &A.replace("# a comment", "# another comment\n# and one more")).is_empty());
    }

    /// (G1 08:16 (1)) a `# contacts:` line is a declaration mobfarm reads: a change to it differs, by name
    #[test]
    fn a_contacts_declaration_change_differs() {
        let with = A.replace("# a comment", "# contacts: -3,0,4=the bed head's east face");
        let d = diffs(&with, &with.replace("-3,0,4=", "-1,0,4="));
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d.iter().all(|l| l.starts_with("declaration `# contacts:`")), "{d:?}");
        // a declaration added differs; one repeated differs by count
        assert!(!diffs(A, &with).is_empty());
        let twice = with.replace("# contacts: -3,0,4=the bed head's east face", "# contacts: -3,0,4=the bed head's east face\n# contacts: -3,0,4=the bed head's east face");
        assert!(diffs(&with, &twice).iter().any(|l| l.contains("another order or count")));
    }

    #[test]
    fn the_name_ports_and_nether_part_differ() {
        assert!(diffs(A, &A.replace("name: t", "name: u")).iter().any(|l| l.starts_with("name:")));
        let ported = A.replace("origin: 0 0 0", "origin: 0 0 0\nport in: 0 0 0");
        assert!(diffs(A, &ported).iter().any(|l| l.starts_with("port only in B")), "{:?}", diffs(A, &ported));
        let nether = format!("{A}nether-origin: 0 0 0\nnether layer 0:\n  S\n");
        let nether2 = format!("{A}nether-origin: 0 0 0\nnether layer 0:\n  G\n");
        assert!(diffs(&nether, &nether2).iter().any(|l| l.starts_with("nether cell changed")), "{:?}", diffs(&nether, &nether2));
    }

    #[test]
    fn one_cell_differs() {
        let d = diffs(A, &A.replace("  SG\n", "  SS\n"));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("cell changed"), "{d:?}");
    }

    #[test]
    fn one_entity_differs() {
        let d = diffs(A, &A.replace("minecart 0.5 1 0.5", "minecart 0.5 1 1.5"));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("entity #0"), "{d:?}");
    }

    #[test]
    fn a_keep_block_only_change_differs() {
        let d = diffs(A, &A.replace("keep-block: 1 0 0", "keep-block: 0 0 0"));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("header line changed"), "{d:?}");
    }

    #[test]
    fn a_block_entity_change_differs() {
        let d = diffs(A, &A.replace("{Items:[]}", "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}"));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("block entity changed"), "{d:?}");
    }
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
