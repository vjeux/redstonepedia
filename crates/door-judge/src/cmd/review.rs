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
/// and block comments, raw strings), and the state after it; `first_open` is set when the line opens
/// a brace at depth `depth0`.
fn scan_line(line: &str, st: &mut Lex, depth: &mut i64, opened: &mut bool) {
    let c: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < c.len() {
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
            continue;
        }
        if let Some(h) = st.raw {
            if c[i] == '"' && (1..=h).all(|k| c.get(i + k) == Some(&'#')) {
                st.raw = None;
                i += 1 + h;
            } else {
                i += 1;
            }
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
            continue;
        }
        match c[i] {
            '/' if c.get(i + 1) == Some(&'/') => break,
            '/' if c.get(i + 1) == Some(&'*') => {
                st.comment = 1;
                i += 2;
            }
            '"' => {
                st.string = true;
                i += 1;
            }
            'r' if (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) && matches!(c.get(i + 1), Some('"') | Some('#')) => {
                let mut h = 0;
                while c.get(i + 1 + h) == Some(&'#') {
                    h += 1;
                }
                if c.get(i + 1 + h) == Some(&'"') {
                    st.raw = Some(h);
                    i += 2 + h;
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
                } else if c.get(i + 2) == Some(&'\'') {
                    i += 3;
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

/// The block of `text` starting at the first line holding `pattern`, through its closing brace.
fn block(text: &str, name: &str, pattern: &str) -> Option<Item> {
    let lines: Vec<&str> = text.lines().collect();
    let k = lines.iter().position(|l| l.contains(pattern))?;
    let e = block_end(&lines, k)?;
    Some(Item { name: name.to_string(), start: k + 1, end: e + 1, text: lines[k..=e].join("\n") })
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
    // every revision's items: the file's functions, then the blocks
    let mut per_rev: Vec<Vec<Item>> = Vec::new();
    for rev in &chain {
        let mut v = show(rev, &file)?.map(|t| items(&t)).unwrap_or_default();
        for b in &blocks {
            let (name, rest) = b.split_once('=').ok_or_else(|| anyhow::anyhow!("--block {b}: NAME=FILE:PATTERN"))?;
            let (path, pattern) = rest.split_once(':').ok_or_else(|| anyhow::anyhow!("--block {b}: NAME=FILE:PATTERN"))?;
            if let Some(t) = show(rev, path)? {
                if let Some(it) = block(&t, &format!("[{name}] {path}"), pattern) {
                    v.push(it);
                }
            }
        }
        per_rev.push(v);
    }
    let last = chain.len() - 1;
    let last_reviewed = chain.iter().rposition(|r| reviewed.contains(r));
    let mut md = String::new();
    writeln!(md, "# Review map: {file} at {} (chain {}, reviewed {})", chain[last], chain.join(","), if reviewed.is_empty() { "none".to_string() } else { reviewed.join(",") })?;
    writeln!(md)?;
    writeln!(md, "| # | item | lines | introduced | changed in | status | against the last reviewed revision ({}) |", last_reviewed.map_or("none", |k| chain[k].as_str()))?;
    writeln!(md, "|---|---|---|---|---|---|---|")?;
    let (mut n_rev, mut n_part, mut n_un) = (0, 0, 0);
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
        let rev_of = |k: &usize| reviewed.contains(&chain[*k]);
        let status = if rev_of(&first) && changed.iter().all(rev_of) {
            n_rev += 1;
            format!("reviewed (introduced in {})", chain[first])
        } else if changed.iter().any(rev_of) {
            n_part += 1;
            format!("PARTLY: unreviewed base ({}), changed in reviewed {}", chain[first], changed.iter().filter(|k| rev_of(k)).map(|k| chain[*k].as_str()).collect::<Vec<_>>().join(", "))
        } else {
            n_un += 1;
            format!("UNREVIEWED (introduced in {})", chain[first])
        };
        let cmp = match last_reviewed.and_then(|k| find(k).map(|x| (k, x))) {
            None => "not in it".to_string(),
            Some((_, x)) if x.text == it.text => "identical".to_string(),
            Some((_, x)) => {
                let (a, b) = line_diff(&x.text, &it.text);
                format!("DIFFERS: {a} line(s) out, {b} line(s) in")
            }
        };
        writeln!(md, "| {} | `{}` | {}..{} | {} | {} | {status} | {cmp} |", idx + 1, it.name, it.start, it.end, chain[first], changed.iter().map(|k| chain[*k].as_str()).collect::<Vec<_>>().join(", "))?;
    }
    writeln!(md)?;
    writeln!(md, "{} item(s): {n_rev} reviewed, {n_part} partly (an unreviewed base with reviewed changes), {n_un} unreviewed", per_rev[last].len())?;
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

    #[test]
    fn line_diff_counts() {
        assert_eq!(line_diff("a\nb\nc", "a\nb\nc"), (0, 0));
        assert_eq!(line_diff("a\nb\nc", "a\nx\nc\nd"), (1, 2));
    }
}
