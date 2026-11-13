//! (cc767b78, ironspec 12:54 / 14:27 (d)) No light read in the workspace is silently defaulted: every
//! `block_light(..)`, `sky_light(..)`, `raw_brightness(..)`, `can_see_sky(..)` and `sky_source(..)` call
//! in the crates' sources is followed by something other than `.unwrap_or(..)`, `.unwrap_or_else(..)`,
//! `.unwrap_or_default()`, `.ok()` or `.is_ok_and(..)`, directly or after a chain of other calls (a
//! `.map(..)` and then a default counts): the error is returned, matched, or read through
//! `World::light_or` / `World::sees_or`, which surface it.

use std::path::Path;

const READS: [&str; 5] = ["block_light", "sky_light", "raw_brightness", "can_see_sky", "sky_source"];
const DEFAULTS: [&str; 5] = ["unwrap_or", "unwrap_or_else", "unwrap_or_default", "ok", "is_ok_and"];

/// The source with `//` comments removed (a `//` inside a string literal is rare in these files and
/// would only hide code after it on that line).
fn code_of(text: &str) -> String {
    text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// The index just past the parenthesised group that opens at `open` (`b[open] == b'('`).
fn past_group(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Each defaulted light read in `code`: (byte offset, the read, the default).
fn defaulted(code: &str) -> Vec<(usize, &'static str, String)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    for read in READS {
        let pat = format!("{read}(");
        let mut from = 0;
        while let Some(k) = code[from..].find(&pat) {
            let at = from + k;
            from = at + pat.len();
            // (a whole name: not the tail of a longer identifier, nor its definition)
            let before = code[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') || code[..at].trim_end().ends_with("fn") {
                continue;
            }
            let Some(mut k) = past_group(b, at + read.len()) else { continue };
            // follow the chain: `.name(..)` calls, whitespace between
            loop {
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k >= b.len() || b[k] != b'.' {
                    break;
                }
                let name_start = k + 1;
                let mut e = name_start;
                while e < b.len() && (b[e].is_ascii_alphanumeric() || b[e] == b'_') {
                    e += 1;
                }
                let name = &code[name_start..e];
                if DEFAULTS.contains(&name) {
                    out.push((at, read, name.to_string()));
                    break;
                }
                if e < b.len() && b[e] == b'(' {
                    match past_group(b, e) {
                        Some(n) => k = n,
                        None => break,
                    }
                } else {
                    break;
                }
            }
        }
    }
    out
}

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The scanner itself: it flags a direct default, one after a chain, one across lines, and passes a
/// surfaced read, a matched one and a lookalike name.
#[test]
fn the_scanner_finds_defaulted_reads() {
    let bad = "let a = w.block_light(p).unwrap_or(0);\nlet b = w.sky_light(q).map(|l| l < 15)\n    .unwrap_or(false);\nlet c = can_see_sky(w, x).ok();";
    let found: Vec<&str> = defaulted(&code_of(bad)).into_iter().map(|(_, r, _)| r).collect();
    assert_eq!(found.len(), 3, "{found:?}");
    let good = "let a = w.light_or(w.block_light(p), 0);\nlet b = match w.sky_light(q) { Ok(v) => v, Err(e) => return Err(e) };\nfn sky_light(&self) -> i32 { 0 }\nlet c = my_block_light(p).unwrap_or(0); // block_light(p).unwrap_or(0) in a comment";
    assert!(defaulted(&code_of(good)).is_empty(), "{:?}", defaulted(&code_of(good)));
}

/// Every crate's sources: no light read defaulted.
#[test]
fn no_light_read_in_the_workspace_is_silently_defaulted() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    sources(&crates, &mut files);
    assert!(files.len() > 100, "the scan reached the workspace's sources ({} files)", files.len());
    let this = Path::new(file!()).file_name().map(|n| n.to_owned());
    let mut bad = Vec::new();
    for f in &files {
        if f.file_name().map(|n| n.to_owned()) == this {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let code = code_of(&text);
        for (at, read, default) in defaulted(&code) {
            let line = code[..at].matches('\n').count() + 1;
            bad.push(format!("{}:{line}: {read}(..) then .{default}", f.display()));
        }
    }
    assert!(bad.is_empty(), "silently defaulted light reads:\n{}", bad.join("\n"));
}
