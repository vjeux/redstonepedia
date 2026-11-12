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
}

pub fn run(cmd: DesignCmd) -> Result<()> {
    match cmd {
        DesignCmd::BlankRows { paths, rewrite } => blank_rows(&paths, rewrite.as_deref()),
    }
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
