//! The declaration keys every mobfarm tool reads from a design file's header, and the shared near-miss
//! check over them (ironspec 06:21 (c), 05:40 real): a key written in another form (case, spacing, a
//! comment where its reader takes a header line or the reverse) or one letter off a known key is an error
//! naming the exact form, never a silently skipped line, so a misspelled `#contacts:` can't drop a contact
//! declaration. `Design::parse` runs it for every mobfarm command ([`register`]); settle-check's and
//! record-match's readers call it too; a test fails if any reader in mobfarm takes a key these lists lack.

/// Keys read only from a comment line, `# key: …` (`strip_prefix("# key:")`, `pushscan::header_entries`).
pub const COMMENT_KEYS: &[&str] = &[
    "clock",
    "water",
    "held",
    "shape",
    "day-time",
    "stand-in",
    "stand-in-guard",
    "candidate",
    "bench-entity",
    "out-of-scope",
    "bench-removed",
    "world box",
    "contacts",
    "stops",
    "always-on",
    "one-way",
    "own-route-junctions",
    "single-occupant",
    "speeds",
    "bridged-pairs",
    "build-order",
];

/// Keys read from a comment line or a header line alike (the push / water-spawn readers strip an optional
/// `#`; flee-check's `PlaceClasses::from_design` reads `integrity:` both ways, `bars::Scope::from_text`
/// `scope-excludes:`, `wetspawn::Site::from_text` `site:`). `travel` is the one key two readers share in
/// different forms: flee-check reads `# travel: A>B>…=REASON` comment lines (`pushscan::header_entries`,
/// 9fc3155f's chain), while the piston-extender circuits' own `travel: x y z` header line belongs to the
/// door tools (designs/circuits/pext): both forms stand, a mis-form of either still errors.
pub const EITHER_KEYS: &[&str] = &["push-pair", "switch", "switches", "update-only", "inert", "water-spawn", "integrity", "scope-excludes", "site", "travel"];

/// Keys read only from a header line, `key: …` (`column::header_cells`, `header_cell_states`;
/// `late-exempt:` through `PlaceClasses::from_meta`).
pub const META_KEYS: &[&str] = &["riderless", "emulated", "waker-places", "placement-order-ts", "unreachable", "update-only-ts", "late-exempt"];

/// Levenshtein distance, capped at 2 (all the check needs).
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return 2;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()].min(2)
}

/// A line's key: the text before its first `:` (after a leading `#` for a comment), when it's at most three
/// words of letters, digits, hyphens and underscores (a prose comment's longer text is left alone).
fn key_of(body: &str) -> Option<String> {
    let (key, _) = body.split_once(':')?;
    let words: Vec<&str> = key.split_whitespace().collect();
    if words.is_empty() || words.len() > 3 || !key.chars().all(|c| c.is_alphanumeric() || c == '-' || c == ' ' || c == '_') {
        return None;
    }
    Some(words.join(" "))
}

/// The near-miss check over a design file's text.
/// - A comment line (`#…`) whose key is a comment or either-form key in another form errors; one whose
///   key is a header-only key errors (its reader never takes the comment); one whose key is one edit off a
///   known key of five letters or more errors.
/// - A header line (no `#`) whose key is a header or either-form key in another form errors; one whose key
///   is a comment-only key errors (its reader never takes the header line).
pub fn check(text: &str) -> Result<(), String> {
    let all = || COMMENT_KEYS.iter().chain(EITHER_KEYS).chain(META_KEYS);
    for line in text.lines() {
        let l = line.trim();
        if let Some(body) = l.strip_prefix('#') {
            let Some(norm) = key_of(body) else { continue };
            if let Some(k) = COMMENT_KEYS.iter().chain(EITHER_KEYS).find(|k| k.eq_ignore_ascii_case(&norm)) {
                if !l.starts_with(&format!("# {k}:")) {
                    return Err(format!("{l:?}: a declaration reads `# {k}:` exactly (its case, one space after the `#`, none before the colon): as written no reader takes it"));
                }
                continue;
            }
            if let Some(k) = META_KEYS.iter().find(|k| k.eq_ignore_ascii_case(&norm)) {
                return Err(format!("{l:?}: `{k}:` is read from a header line, never a comment: write `{k}: …` without the `#`"));
            }
            let lower = norm.to_lowercase();
            if let Some(k) = all().find(|k| k.len() >= 5 && distance(&k.to_lowercase(), &lower) <= 1) {
                return Err(format!("{l:?}: `{norm}` is no declaration key; did you mean `{k}`? (a misspelled key would be skipped by every reader; reword the line if it's prose)"));
            }
        } else if let Some(norm) = key_of(l) {
            if let Some(k) = META_KEYS.iter().chain(EITHER_KEYS).find(|k| k.eq_ignore_ascii_case(&norm)) {
                if !l.starts_with(&format!("{k}:")) {
                    return Err(format!("{l:?}: a header declaration reads `{k}:` exactly (its case, none before the colon): as written no reader takes it"));
                }
                continue;
            }
            if let Some(k) = COMMENT_KEYS.iter().find(|k| k.eq_ignore_ascii_case(&norm)) {
                return Err(format!("{l:?}: `# {k}:` is read from a comment line, never a header line: write `# {k}: …`"));
            }
        }
    }
    Ok(())
}

/// Registers [`check`] with `mctas::design::Design::parse` for this process (every mobfarm command): door
/// and other tools register none, so their designs are parsed as before.
pub fn register() {
    let _ = mctas::design::HEADER_CHECK.set(check);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A known key in another form, a header-only key as a comment, a comment-only key as a header line,
    /// and a key one letter off a known one are errors; the exact forms, short unknown keys and prose
    /// comments stand.
    #[test]
    fn near_misses() {
        for bad in ["#water: 1,2,3=x", "# Water: 1,2,3=x", "# water : 1,2,3=x", "#  clock: 1,2,3=x", "# contact: 1,2,3=x", "#contacts: 1,2,3=x", "# watr: 1,2,3=x", "# riderless: 1,2,3=x", "Riderless: 1,2,3=x", "water: 1,2,3=x", "# stand-ins: 1,2,3", "Switch: 1,2,3=x"] {
            assert!(check(bad).is_err(), "{bad:?} refused");
        }
        for ok in ["# water: 1,2,3=x", "# contacts: 1,2,3=x", "riderless: 1,2,3=x", "switch: 1,2,3=x", "# switch: 1,2,3=x", "# day-time: 18000 (r)", "# The return for F1 + Q2 at 24 (7b90c3f1; r3): loop frame", "# note: a remark", "# rail Ts (every plain rail; each a case): x", "name: x", "origin: 0 0 0"] {
            assert!(check(ok).is_ok(), "{ok:?} stands: {:?}", check(ok));
        }
    }

    /// The registration test (ironspec 05:40 real): every key a mobfarm reader takes, found in the crate's
    /// own source by its reading patterns (`strip_prefix("# K:")`, `header_entries(…, "K:")`,
    /// `header_cells(…, "K:")`, `header_cell_states(…, "K:")`, `let key = "K:";`, a `("K:", "…")` key table,
    /// `strip_prefix("K:")` after an optional `#`), is in these lists; the lists printed.
    #[test]
    fn every_reader_registered() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let lit_after = |s: &str, from: usize| -> Option<String> {
            let rest = &s[from..];
            let a = rest.find('"')?;
            let b = rest[a + 1..].find('"')?;
            Some(rest[a + 1..a + 1 + b].to_string())
        };
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") || p.file_name().and_then(|x| x.to_str()) == Some("declkeys.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&p).unwrap();
            for line in src.lines() {
                for pat in ["strip_prefix(\"# ", "header_entries(", "header_cells(", "header_cell_states(", "let key = \"", "strip_prefix(\""] {
                    let mut at = 0;
                    while let Some(i) = line[at..].find(pat) {
                        let start = at + i;
                        at = start + pat.len();
                        let key = match pat {
                            "strip_prefix(\"# " => line[at..].split('"').next().map(str::to_string),
                            "let key = \"" => line[at..].split('"').next().map(str::to_string),
                            "strip_prefix(\"" => {
                                // (only a key read after an optional `#`: the line strips `#` first)
                                if line[..start].contains("strip_prefix('#')") { line[at..].split('"').next().map(str::to_string) } else { None }
                            }
                            _ => {
                                // (the call's last string literal ending with `:` on the line)
                                let mut k = None;
                                let mut j = at;
                                while let Some(s) = lit_after(line, j) {
                                    let pos = line[j..].find(&format!("\"{s}\"")).map(|q| j + q + s.len() + 2).unwrap_or(line.len());
                                    if s.ends_with(':') {
                                        k = Some(s.clone());
                                    }
                                    j = pos;
                                    if j >= line.len() {
                                        break;
                                    }
                                }
                                k
                            }
                        };
                        if let Some(k) = key
                            && let Some(k) = k.strip_suffix(':')
                            && !k.is_empty()
                            && !k.contains('{')
                        {
                            found.insert(k.trim().to_string());
                        }
                    }
                }
                // (a key table: `[("K:", "…"), ("K:", "…")]`, each entry after `[` or `, `)
                let mut at = 0;
                while let Some(i) = line[at..].find("(\"") {
                    let open = at + i;
                    let s = open + 2;
                    at = s;
                    let before = &line[..open];
                    if !(before.ends_with('[') || before.ends_with(", ")) {
                        continue;
                    }
                    if let Some(end) = line[s..].find("\", \"") {
                        let k = &line[s..s + end];
                        if let Some(k) = k.strip_suffix(':')
                            && !k.is_empty()
                            && k.chars().all(|c| c.is_alphanumeric() || c == '-' || c == ' ')
                        {
                            found.insert(k.to_string());
                        }
                    }
                }
            }
        }
        let known: std::collections::BTreeSet<String> = COMMENT_KEYS.iter().chain(EITHER_KEYS).chain(META_KEYS).map(|k| k.to_string()).collect();
        println!("declaration keys registered: comment {COMMENT_KEYS:?}; either form {EITHER_KEYS:?}; header {META_KEYS:?}");
        let missing: Vec<&String> = found.iter().filter(|k| !known.contains(*k)).collect();
        assert!(missing.is_empty(), "readers take keys the shared check lacks: {missing:?} (found {found:?})");
        assert!(found.contains("contacts") && found.contains("riderless") && found.contains("water") && found.contains("push-pair"), "the scan finds the readers: {found:?}");
    }

    /// Every `.txt` file under `root`, with the header check's verdict on each failing one.
    fn failing_files(root: &std::path::Path) -> (usize, Vec<String>) {
        let (mut n, mut bad) = (0, Vec::new());
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().path()).collect();
            entries.sort();
            for p in entries {
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "txt") {
                    n += 1;
                    let text = std::fs::read_to_string(&p).unwrap();
                    if let Err(e) = check(&text) {
                        bad.push(format!("{}: {e}", p.display()));
                    }
                }
            }
        }
        (n, bad)
    }

    /// ironspec 06:48: no committed design file the farm tools read fails the header check (`Design::parse`
    /// runs it in every mobfarm command): every `.txt` under designs/farms, and every design the circuits
    /// catalog lists (`parts::place` loads them from designs/circuits), passes.
    #[test]
    fn committed_farm_designs_pass_the_header_check() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (n, mut bad) = failing_files(&repo.join("designs/farms"));
        let catalog = std::fs::read_to_string(repo.join(crate::parts::CATALOG)).unwrap();
        let mut rows = catalog.lines();
        let col = rows.next().unwrap().split('\t').position(|h| h == "path").expect("the catalog's path column");
        let mut parts = 0;
        for r in rows.filter(|r| !r.trim().is_empty()) {
            let rel = r.split('\t').nth(col).expect("a path");
            let p = repo.join("designs/circuits").join(rel);
            parts += 1;
            if let Err(e) = check(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))) {
                bad.push(format!("{}: {e}", p.display()));
            }
        }
        println!("the header check: {n} files under designs/farms and {parts} catalog parts, {} failing", bad.len());
        assert!(n > 100 && parts > 100 && bad.is_empty(), "{} files the farm tools read fail the header check:\n{}", bad.len(), bad.join("\n"));
    }
}
