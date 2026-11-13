//! `mctas review-map` (b8949c96; G1 10:38 on a re-cut chain landing as one commit): every function
//! of a file at the chain's last revision, with its line range, the chain commit that introduced it,
//! the chain commits that changed it, whether a REVIEWED commit introduced or changed it, and its text
//! compared with the last reviewed revision that holds it. Extra blocks (a CLI match arm) map the same
//! way. The chain's file history is read with `git show REV:PATH` from the repository the command runs
//! in.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub struct ReviewMapArgs {
    /// The file mapped (its repository path).
    #[arg(long)]
    file: String,
    /// The chain's commits that changed the file, oldest first (the map's history).
    #[arg(long, value_delimiter = ',', required = true)]
    chain: Vec<String>,
    /// The chain's commits a reviewer has reviewed (a subset of --chain).
    #[arg(long, value_delimiter = ',')]
    reviewed: Vec<String>,
    /// More blocks mapped like functions, NAME=FILE:PATTERN: the block starts at the first line
    /// holding PATTERN and ends where its braces close (a CLI match arm, its arguments' struct).
    #[arg(long)]
    block: Vec<String>,
    /// The map is also written here (markdown).
    #[arg(long)]
    out: Option<PathBuf>,
}

/// One mapped item: its qualified name, its 1-based inclusive line range and its text.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    name: String,
    start: usize,
    end: usize,
    text: String,
}

/// The lexer's state between lines: inside a block comment (its nesting depth), a string, or a raw
/// string (its hash count).
#[derive(Clone, Copy, Default)]
struct Lex {
    comment: u32,
    string: bool,
    raw: Option<usize>,
}

/// The brace depth change of `line` from the lexer state `st` (skipping strings, char literals, line
/// and block comments, raw strings and raw byte strings), and the state after it; `opened` is set when
/// the line opens a brace.
fn scan_line(line: &str, st: &mut Lex, depth: &mut i64, opened: &mut bool) {
    scan_line_mask(line, st, depth, opened, None);
}

/// [`scan_line`], also marking in `mask` (one entry per char) the chars that are CODE (false for the
/// chars of comments, strings, raw strings and char literals): the one lexer for both uses.
fn scan_line_mask(line: &str, st: &mut Lex, depth: &mut i64, opened: &mut bool, mut mask: Option<&mut Vec<bool>>) {
    let c: Vec<char> = line.chars().collect();
    if let Some(m) = mask.as_deref_mut() {
        *m = vec![true; c.len()];
    }
    let mut text = |m: &mut Option<&mut Vec<bool>>, a: usize, b: usize| {
        if let Some(m) = m.as_deref_mut() {
            for x in m.iter_mut().take(b.min(c.len())).skip(a) {
                *x = false;
            }
        }
    };
    let mut i = 0;
    while i < c.len() {
        let i0 = i;
        if st.comment > 0 {
            if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                st.comment -= 1;
                i += 2;
            } else if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                st.comment += 1;
                i += 2;
            } else {
                i += 1;
            }
            text(&mut mask, i0, i);
            continue;
        }
        if let Some(h) = st.raw {
            if c[i] == '"' && (1..=h).all(|k| c.get(i + k) == Some(&'#')) {
                st.raw = None;
                i += 1 + h;
            } else {
                i += 1;
            }
            text(&mut mask, i0, i);
            continue;
        }
        if st.string {
            if c[i] == '\\' {
                i += 2;
            } else {
                if c[i] == '"' {
                    st.string = false;
                }
                i += 1;
            }
            text(&mut mask, i0, i);
            continue;
        }
        // a token's own start: an identifier char before it means it's part of a longer name
        let starts = |k: usize| k == 0 || !(c[k - 1].is_alphanumeric() || c[k - 1] == '_');
        match c[i] {
            '/' if c.get(i + 1) == Some(&'/') => {
                text(&mut mask, i, c.len());
                break;
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                st.comment = 1;
                i += 2;
                text(&mut mask, i0, i);
            }
            '"' => {
                st.string = true;
                i += 1;
                text(&mut mask, i0, i);
            }
            // r"..", r#".."#, and (G1's note on 971bb6366) br"..", br#".."#
            'r' if (starts(i) || (i >= 1 && c[i - 1] == 'b' && starts(i - 1))) && matches!(c.get(i + 1), Some('"') | Some('#')) => {
                let mut h = 0;
                while c.get(i + 1 + h) == Some(&'#') {
                    h += 1;
                }
                if c.get(i + 1 + h) == Some(&'"') {
                    st.raw = Some(h);
                    i += 2 + h;
                    text(&mut mask, if i0 >= 1 && c[i0 - 1] == 'b' { i0 - 1 } else { i0 }, i);
                } else {
                    i += 1;
                }
            }
            '\'' => {
                // a char literal ('x', '\n', '\u{..}') or a lifetime ('a): a lifetime has no closing
                // quote right after its first character
                if c.get(i + 1) == Some(&'\\') {
                    let mut j = i + 2;
                    while j < c.len() && c[j] != '\'' {
                        j += 1;
                    }
                    i = j + 1;
                    text(&mut mask, i0, i);
                } else if c.get(i + 2) == Some(&'\'') {
                    i += 3;
                    text(&mut mask, i0, i);
                } else {
                    i += 1;
                }
            }
            '{' => {
                *depth += 1;
                *opened = true;
                i += 1;
            }
            '}' => {
                *depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
}

/// A function header's name (`[pub[(..)]] [const] [async] [unsafe] [extern ".."] fn NAME`), or None.
fn fn_name(line: &str) -> Option<String> {
    let mut s = line.trim_start();
    if let Some(r) = s.strip_prefix("pub") {
        s = r.trim_start();
        if s.starts_with('(') {
            s = s.split_once(')').map(|(_, r)| r.trim_start()).unwrap_or(s);
        }
    }
    for kw in ["const ", "async ", "unsafe "] {
        if let Some(r) = s.strip_prefix(kw) {
            s = r.trim_start();
        }
    }
    if let Some(r) = s.strip_prefix("extern ") {
        s = r.trim_start();
        if s.starts_with('"') {
            s = s[1..].split_once('"').map(|(_, r)| r.trim_start()).unwrap_or(s);
        }
    }
    let r = s.strip_prefix("fn ")?;
    let name: String = r.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    (!name.is_empty()).then_some(name)
}

/// A scope header's name: `mod NAME {`, `impl[<..>] [TRAIT for] TYPE[<..>] {`, `trait NAME`.
fn scope_name(line: &str) -> Option<String> {
    let mut s = line.trim_start();
    if let Some(r) = s.strip_prefix("pub") {
        s = r.trim_start();
        if s.starts_with('(') {
            s = s.split_once(')').map(|(_, r)| r.trim_start()).unwrap_or(s);
        }
    }
    let ident = |t: &str| -> String { t.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect() };
    if let Some(r) = s.strip_prefix("mod ") {
        return Some(ident(r)).filter(|n| !n.is_empty() && line.contains('{'));
    }
    if let Some(r) = s.strip_prefix("trait ") {
        return Some(ident(r)).filter(|n| !n.is_empty());
    }
    let r = s.strip_prefix("impl")?;
    if !(r.starts_with(' ') || r.starts_with('<')) {
        return None;
    }
    let head = r.split('{').next().unwrap_or(r);
    let ty = head.rsplit(" for ").next().unwrap_or(head).trim();
    // the type's last path segment, generics dropped (impl<T> Foo<T> -> Foo)
    let ty = if ty.starts_with('<') { ty.split_once('>').map(|(_, t)| t.trim()).unwrap_or(ty) } else { ty };
    let base = ty.split('<').next().unwrap_or(ty).trim();
    let last = base.rsplit("::").next().unwrap_or(base);
    Some(ident(last)).filter(|n| !n.is_empty())
}

/// From line `start` (0-based), the line (0-based) where the braces opened from it close again.
fn block_end(lines: &[&str], start: usize) -> Option<usize> {
    let (mut st, mut depth, mut opened) = (Lex::default(), 0i64, false);
    for (k, l) in lines.iter().enumerate().skip(start) {
        scan_line(l, &mut st, &mut depth, &mut opened);
        if opened && depth <= 0 {
            return Some(k);
        }
        // a declaration without a body (`fn f();` in a trait)
        if !opened && l.trim_end().ends_with(';') {
            return None;
        }
    }
    None
}

/// Every function of `text`, qualified by its enclosing modules, impls and traits; a name held twice
/// gets `#2`, `#3`.
fn items(text: &str) -> Vec<Item> {
    let lines: Vec<&str> = text.lines().collect();
    let mut st = Lex::default();
    let mut out: Vec<Item> = Vec::new();
    let mut scopes: Vec<(String, usize)> = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let (mut depth, mut opened) = (0i64, false);
    for (k, l) in lines.iter().enumerate() {
        let in_text = st.comment > 0 || st.string || st.raw.is_some();
        scopes.retain(|(_, end)| *end >= k);
        if !in_text {
            if let Some(n) = fn_name(l) {
                if let Some(e) = block_end(&lines, k) {
                    let mut q: Vec<String> = scopes.iter().map(|(s, _)| s.clone()).collect();
                    q.push(n);
                    let mut name = q.join("::");
                    let c = seen.entry(name.clone()).or_default();
                    *c += 1;
                    if *c > 1 {
                        name = format!("{name}#{c}");
                    }
                    out.push(Item { name, start: k + 1, end: e + 1, text: lines[k..=e].join("\n") });
                }
            } else if let Some(s) = scope_name(l) {
                if let Some(e) = block_end(&lines, k) {
                    scopes.push((s, e));
                }
            }
        }
        scan_line(l, &mut st, &mut depth, &mut opened);
    }
    out
}

/// `line` with its strings, chars and comments blanked (spaces), from the lexer state `st` (updated): a
/// pattern found in the result is in the code, not in a comment or a string.
fn code_only(line: &str, st: &mut Lex) -> String {
    let (mut d, mut o, mut mask) = (0i64, false, Vec::new());
    scan_line_mask(line, st, &mut d, &mut o, Some(&mut mask));
    line.chars().zip(mask).map(|(ch, code)| if code { ch } else { ' ' }).collect()
}

/// The block of `text` starting at the first line holding `pattern` IN THE CODE (not in a comment or a
/// string: G1's CHANGES 3 (2)), through its closing brace.
fn block(text: &str, name: &str, pattern: &str) -> Option<Item> {
    let lines: Vec<&str> = text.lines().collect();
    let mut st = Lex::default();
    let mut k = None;
    for (n, l) in lines.iter().enumerate() {
        if code_only(l, &mut st).contains(pattern) {
            k = Some(n);
            break;
        }
    }
    let k = k?;
    let e = block_end(&lines, k)?;
    Some(Item { name: name.to_string(), start: k + 1, end: e + 1, text: lines[k..=e].join("\n") })
}

/// (G1's CHANGES 3 (1)) the file's lines outside every mapped item (consts, statics, structs and their
/// fields, `use` lines, macros, comments between items), in order: the residual compared like an item.
fn residual(text: &str, its: &[Item]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut covered = vec![false; lines.len()];
    for it in its {
        for c in covered.iter_mut().take(it.end).skip(it.start - 1) {
            *c = true;
        }
    }
    lines.iter().zip(&covered).filter(|(_, c)| !**c).map(|(l, _)| *l).collect::<Vec<_>>().join("\n")
}

/// `git show REV:PATH`: None when the file isn't at REV; an error when REV isn't a commit.
fn show(rev: &str, path: &str) -> Result<Option<String>> {
    let ok = std::process::Command::new("git").args(["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")]).output().context("git rev-parse")?;
    if !ok.status.success() {
        bail!("{rev}: not a commit in this repository");
    }
    let o = std::process::Command::new("git").args(["show", &format!("{rev}:{path}")]).output().context("git show")?;
    if !o.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(o.stdout).with_context(|| format!("{rev}:{path} is not UTF-8"))?))
}

/// The lines two texts differ by: (only in a, only in b), by a longest-common-subsequence walk.
fn line_diff(a: &str, b: &str) -> (usize, usize) {
    let (x, y): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut t = vec![vec![0u32; y.len() + 1]; x.len() + 1];
    for i in (0..x.len()).rev() {
        for j in (0..y.len()).rev() {
            t[i][j] = if x[i] == y[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let common = t[0][0] as usize;
    (x.len() - common, y.len() - common)
}

pub fn cmd_review_map(args: ReviewMapArgs) -> Result<()> {
    let ReviewMapArgs { file, chain, reviewed, block: blocks, out } = args;
    for r in &reviewed {
        if !chain.contains(r) {
            bail!("--reviewed {r} is not in --chain");
        }
    }
    let last = chain.len() - 1;
    // every revision's items: the file's functions, then the blocks; and the file's residual (the lines
    // outside every function: G1's CHANGES 3 (1))
    let mut per_rev: Vec<Vec<Item>> = Vec::new();
    let mut residuals: Vec<Option<String>> = Vec::new();
    for (k, rev) in chain.iter().enumerate() {
        let text = show(rev, &file)?;
        // (G1's CHANGES 3 (3)) the file absent at the last revision is an error, not an empty map
        if k == last && text.is_none() {
            bail!("--file {file} is not at the chain's last revision {rev}");
        }
        let mut v = text.as_deref().map(items).unwrap_or_default();
        residuals.push(text.as_deref().map(|t| residual(t, &v)));
        for b in &blocks {
            let (name, rest) = b.split_once('=').ok_or_else(|| anyhow::anyhow!("--block {b}: NAME=FILE:PATTERN"))?;
            let (path, pattern) = rest.split_once(':').ok_or_else(|| anyhow::anyhow!("--block {b}: NAME=FILE:PATTERN"))?;
            let found = show(rev, path)?.and_then(|t| block(&t, &format!("[{name}] {path}"), pattern));
            // (G1's CHANGES 3 (2)) a requested block missing at the last revision is an error
            match found {
                Some(it) => v.push(it),
                None if k == last => bail!("--block {b}: no line holds {pattern:?} in the code of {path} at the chain's last revision {rev}"),
                None => {}
            }
        }
        per_rev.push(v);
    }
    let last_reviewed = chain.iter().rposition(|r| reviewed.contains(r));
    let mut md = String::new();
    writeln!(md, "# Review map: {file} at {} (chain {}, reviewed {})", chain[last], chain.join(","), if reviewed.is_empty() { "none".to_string() } else { reviewed.join(",") })?;
    writeln!(md)?;
    writeln!(md, "Items are the file's functions (qualified by their modules, impls and traits) and the requested blocks; the RESIDUAL row is every line outside them (consts, statics, structs and their fields, `use` lines, macros, comments between items). A name held twice is numbered by order (`name#2`): an inserted same-named function re-points the numbers, and the text comparison then reads DIFFERS (fail safe).")?;
    writeln!(md)?;
    writeln!(md, "| # | item | lines | introduced | changed in | status | against the last reviewed revision ({}) |", last_reviewed.map_or("none", |k| chain[k].as_str()))?;
    writeln!(md, "|---|---|---|---|---|---|---|")?;
    let (mut n_rev, mut n_part, mut n_un) = (0, 0, 0);
    let rev_of = |k: &usize| reviewed.contains(&chain[*k]);
    // the status of an item from the revisions it changed in: the same rule for items and the residual
    let mut status_of = |first: usize, changed: &[usize]| -> String {
        if rev_of(&first) && changed.iter().all(rev_of) {
            n_rev += 1;
            format!("reviewed (introduced in {})", chain[first])
        } else if changed.iter().any(rev_of) {
            n_part += 1;
            format!("PARTLY: unreviewed base ({}), changed in reviewed {}", chain[first], changed.iter().filter(|k| rev_of(k)).map(|k| chain[*k].as_str()).collect::<Vec<_>>().join(", "))
        } else {
            n_un += 1;
            format!("UNREVIEWED (introduced in {})", chain[first])
        }
    };
    let compare = |was: Option<&str>, now: &str| -> String {
        match was {
            None => "not in it".to_string(),
            Some(x) if x == now => "identical".to_string(),
            Some(x) => {
                let (a, b) = line_diff(x, now);
                format!("DIFFERS: {a} line(s) out, {b} line(s) in")
            }
        }
    };
    for (idx, it) in per_rev[last].iter().enumerate() {
        let find = |k: usize| per_rev[k].iter().find(|x| x.name == it.name);
        let mut changed: Vec<usize> = Vec::new();
        for k in 0..=last {
            let now = find(k).map(|x| x.text.as_str());
            let before = if k == 0 { None } else { find(k - 1).map(|x| x.text.as_str()) };
            if now.is_some() && now != before {
                changed.push(k);
            }
        }
        let first = (0..=last).find(|&k| find(k).is_some()).unwrap_or(last);
        let status = status_of(first, &changed);
        let cmp = compare(last_reviewed.and_then(|k| find(k).map(|x| x.text.as_str())), &it.text);
        writeln!(md, "| {} | `{}` | {}..{} | {} | {} | {status} | {cmp} |", idx + 1, it.name, it.start, it.end, chain[first], changed.iter().map(|k| chain[*k].as_str()).collect::<Vec<_>>().join(", "))?;
    }
    // the residual row
    {
        let mut changed: Vec<usize> = Vec::new();
        for k in 0..=last {
            let now = residuals[k].as_deref();
            let before = if k == 0 { None } else { residuals[k - 1].as_deref() };
            if now.is_some() && now != before {
                changed.push(k);
            }
        }
        let first = (0..=last).find(|&k| residuals[k].is_some()).unwrap_or(last);
        let now = residuals[last].as_deref().unwrap_or_default();
        let status = status_of(first, &changed);
        let cmp = compare(last_reviewed.and_then(|k| residuals[k].as_deref()), now);
        writeln!(md, "| R | `[residual] {file}: the lines outside every function` | {} line(s) | {} | {} | {status} | {cmp} |", now.lines().count(), chain[first], changed.iter().map(|k| chain[*k].as_str()).collect::<Vec<_>>().join(", "))?;
    }
    writeln!(md)?;
    writeln!(md, "{} item(s) and the residual: {n_rev} reviewed, {n_part} partly (an unreviewed base with reviewed changes), {n_un} unreviewed", per_rev[last].len())?;
    print!("{md}");
    if let Some(p) = out {
        std::fs::write(&p, &md).with_context(|| format!("writing {}", p.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Functions, their scopes and their line ranges; braces in strings, chars and comments ignored;
    /// a multi-line signature; a one-line function; a trait declaration without a body skipped.
    #[test]
    fn items_and_their_ranges() {
        let src = "use x;\n\
                   /// a { doc brace\n\
                   pub fn a() -> u8 {\n    let s = \"}\";\n    let c = '{';\n    1 // }\n}\n\
                   pub(crate) fn b(\n    x: u8,\n) -> u8 { x }\n\
                   struct S;\n\
                   impl<T> Tr for S {\n    fn m(&self) {\n        /* { */\n    }\n}\n\
                   trait Q {\n    fn decl(&self);\n    fn with_body<'a>(&'a self) -> &'a str { r#\"}\"# }\n}\n\
                   mod tests {\n    fn t() {}\n}\n";
        let v = items(src);
        let names: Vec<(&str, usize, usize)> = v.iter().map(|i| (i.name.as_str(), i.start, i.end)).collect();
        assert_eq!(names, vec![("a", 3, 7), ("b", 8, 10), ("S::m", 13, 15), ("Q::with_body", 19, 19), ("tests::t", 22, 22)]);
    }

    /// A match arm whose pattern holds braces: the block ends where the arm's body closes.
    #[test]
    fn a_match_arm_block() {
        let src = "match c {\n    Cmd::A { x, y } => {\n        f(x);\n        g({ y });\n    }\n    Cmd::B => {}\n}\n";
        let b = block(src, "arm", "Cmd::A {").unwrap();
        assert_eq!((b.start, b.end), (2, 5));
    }

    /// (G1's CHANGES 3 (2)) a block's pattern is matched in the CODE: an earlier mention in a line
    /// comment, a block comment or a string is skipped; a pattern found nowhere in the code is None.
    #[test]
    fn a_block_pattern_matches_code_only() {
        let src = "// Cmd::A { in a comment\n/* Cmd::A {\n */\nlet s = \"Cmd::A {\";\nmatch c {\n    Cmd::A { x } => {\n        f(x);\n    }\n}\n";
        let b = block(src, "arm", "Cmd::A {").unwrap();
        assert_eq!((b.start, b.end), (6, 8), "the code's line, not the comments' or the string's");
        assert!(block("// Cmd::Z {\nlet s = \"Cmd::Z {\";\n", "arm", "Cmd::Z {").is_none());
    }

    /// (G1's CHANGES 3 (1), its control) the residual: a change only in a const (outside every function)
    /// changes the residual and no function; a change only inside a function changes that function and
    /// not the residual.
    #[test]
    fn the_residual_catches_a_const_change() {
        let a = "const BAR: f64 = 0.24;\nstruct S { x: u8 }\nfn f() -> f64 {\n    BAR\n}\n";
        let b = "const BAR: f64 = 0.30;\nstruct S { x: u8 }\nfn f() -> f64 {\n    BAR\n}\n";
        let c = "const BAR: f64 = 0.24;\nstruct S { x: u8 }\nfn f() -> f64 {\n    BAR * 2.0\n}\n";
        let (ia, ib, ic) = (items(a), items(b), items(c));
        assert_eq!(ia, ib, "the function unchanged by the const");
        assert_ne!(residual(a, &ia), residual(b, &ib), "the residual DIFFERS on a const change");
        assert_eq!(line_diff(&residual(a, &ia), &residual(b, &ib)), (1, 1));
        assert_ne!(ia[0].text, ic[0].text, "a body change shows on the function");
        assert_eq!(residual(a, &ia), residual(c, &ic), "and not on the residual");
    }

    /// (G1's note on 971bb6366) a raw byte string `br"…"` / `br#"…"#` is lexed as raw: its braces and a
    /// trailing backslash don't leak.
    #[test]
    fn raw_byte_strings() {
        let src = "fn a() -> &'static [u8] {\n    br\"}\\\"\n}\nfn b() -> &'static [u8] {\n    br#\"{\"#\n}\n";
        let v = items(src);
        let names: Vec<(&str, usize, usize)> = v.iter().map(|i| (i.name.as_str(), i.start, i.end)).collect();
        assert_eq!(names, vec![("a", 1, 3), ("b", 4, 6)]);
    }

    #[test]
    fn line_diff_counts() {
        assert_eq!(line_diff("a\nb\nc", "a\nb\nc"), (0, 0));
        assert_eq!(line_diff("a\nb\nc", "a\nx\nc\nd"), (1, 2));
    }
}
