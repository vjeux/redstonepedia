//! `mctas mobfarm column` (onechunk, farmopt's sub-lane, 2026-11-11; vjeux: "Can we have the
//! system in one chunk?"; ironspec's paper layout v0, artifact 1105169452432266): the one-chunk
//! column's skeleton. Each part is a committed design file, or the cells of it inside a kept box,
//! moved from its file's frame into the column's frame (the loop frame: the farm's alignment,
//! X & 15 = 6 and Z & 15 = 6, puts the chunk lines at x, z = -6 + 16k) and from its committed
//! place into its tier. Nothing is wired. The report gives each part's committed box in the frame
//! and the chunk lines it crosses there, its box once placed and where it leaves the column, the
//! cells two parts both hold, the tiers from the top down with the gaps between them, and the
//! whole column's box and blocks³. The skeleton file holds every part's blocks, block entities
//! and entities (its riders and zombies are the golem flood's summoners), the parts' `positions:`
//! moved, and the one Nether part; the first part keeps its ports (the judge's `port output`),
//! the others' are renamed `<part>.<port>`.

use mctas::design::Design;
use std::collections::BTreeMap;
use std::path::PathBuf;

type P = [i32; 3];

/// One part of the column: `NAME=FILE[%BASE][@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]`.
#[derive(Clone, Debug, PartialEq)]
pub struct PartSpec {
    pub name: String,
    pub file: PathBuf,
    /// Only what the file adds to this design (`%`: its cells empty there, its entities absent
    /// there).
    pub base: Option<PathBuf>,
    /// The move from the file's frame into the column's (`@`).
    pub frame: P,
    /// The move from the part's committed place in the column's frame to its tier (`^`).
    pub tier: P,
    /// The cells kept, in the file's frame (`#`, inclusive); none: the whole file.
    pub keep: Option<(P, P)>,
    /// A part that re-lays cleared cells (`NAME!=FILE`): the `--clear` boxes take nothing out of
    /// it (onechunk's runner, Nov 11: the escapement's detector replacing the hold's flat).
    pub relay: bool,
    /// A part made of its file's Nether part (`NAME~=FILE`: onechunk Nov 12, the per-group Nether
    /// layout composed from the loader's twin, the kill room and the storage).
    pub nether: bool,
}

fn ints(s: &str) -> Result<Vec<i32>, String> {
    s.split(',').map(|t| t.trim().parse::<i32>().map_err(|e| format!("{t:?}: {e}"))).collect()
}

/// (onechunk Nov 13; ironspec 09:43 / 09:48 / 09:55 / 09:57, G1 09:45 / 09:54: a composed record
/// carries every one of its parts' meta lines) Each part's declarations moved into the composed
/// record like its cells (`@` frame + `^` tier + the output shift), an entry kept only where the
/// part's `#` box keeps every cell it names, each naming the part it came from (the prose inside a
/// reason stays in the part's frame). The kinds: ENTRY lists (`x,y,z[+x,y,z]=REASON; ...`: the meta
/// lines in `ENTRY_META`, the `# key:` lines in `ENTRY_HASH`), a LEADING cell (`# stand-in: x,y,z
/// (why)`), cell LISTS (`keep-clear:` / `keep-out:`: cells, then the reason), NAMED cells
/// (`run-cells:` / `run-gates:`: `NAME x,y,z[/x,y,z]; ...`) and SCALARS (one value for every part,
/// else the compose fails). The composer's own meta (`name`, `origin`, `nether-origin`, `palette`,
/// `positions`, `declared`, `ground`, `entity`) and its generated `#` lines (`placement order`, `rail
/// T(s)`, `settled junction`) are left to it. Any other meta line, or a `# key:` line whose value
/// starts with a cell, is a declaration the composer can't carry: the compose FAILS naming it.
/// Returns (the `#` lines, the meta lines as (key, value)).
pub fn carried_declarations(specs: &[PartSpec], shift: P) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    carried_filtered(specs, shift, &CarryFilter::default()).map(|c| (c.heads, c.metas))
}

/// (vcompose, G1 Nov 13 21:53) What a carried declaration may still name once the parts are laid:
/// a non-relay part's entry naming a cell inside a clear box is dropped (the cleared cell is no
/// longer that part's), and the STALE part's entries naming a cell an owner part holds are dropped
/// (the owner's file declares its own cells); every dropped entry is listed, never silent.
#[derive(Clone, Debug, Default)]
pub struct CarryFilter {
    pub clear: Vec<(P, P)>,
    pub stale: Option<String>,
    /// The cells the owner (non-stale) parts hold, placed in the column's frame.
    pub held: std::collections::BTreeSet<P>,
    /// (part, cell) pairs a precedence line gave to another part: that part's declarations on the
    /// cell are dropped (its state isn't in the record).
    pub lost: Vec<(String, P)>,
    /// (vcompose) the `--delta` files, read for their entry declarations (a `# contacts:` line an
    /// owner's delta states at its cells) at a zero move; their other lines are prose, skipped.
    pub deltas: Vec<PartSpec>,
    /// (vcompose, G1 Nov 13 23:21, D1) the composed record's non-air cells (the column's frame):
    /// a stale part's declaration naming a cell the record leaves empty is dropped by name (None:
    /// not judged, for a compose with no stale part).
    pub record: Option<std::collections::BTreeSet<P>>,
    /// Cells whose stale declarations stay although the record leaves them empty, each with why
    /// (`--keep-stale x,y,z=REASON`).
    pub keep_stale: Vec<(P, String)>,
}

/// (vcompose) What [`carried_filtered`] carries: the header lines, the meta lines, every entry the
/// filter dropped (part, key, entry, why) and every entry carried from the stale part (G1 Nov 13
/// 23:21, D1: listed with a count, never carried unlisted).
#[derive(Debug)]
pub struct Carried {
    pub heads: Vec<String>,
    pub metas: Vec<(String, String)>,
    pub dropped: Vec<String>,
    pub stale_kept: Vec<String>,
}

/// Every `x,y,z` token in `text` (three integers joined by commas, an optional '-', bounded by a
/// non-digit), with its byte range.
fn cell_tokens(text: &str) -> Vec<(usize, usize, P)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start_ok = i == 0 || !(b[i - 1].is_ascii_digit() || b[i - 1] == b',' || b[i - 1] == b'-' || b[i - 1] == b'.');
        if start_ok && (b[i].is_ascii_digit() || (b[i] == b'-' && i + 1 < b.len() && b[i + 1].is_ascii_digit())) {
            let mut j = i;
            let mut nums: Vec<i32> = Vec::new();
            loop {
                let s = j;
                if j < b.len() && b[j] == b'-' {
                    j += 1;
                }
                let d0 = j;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if j == d0 {
                    break;
                }
                match text[s..j].parse::<i32>() {
                    Ok(n) => nums.push(n),
                    Err(_) => break,
                }
                if nums.len() == 3 || j >= b.len() || b[j] != b',' {
                    break;
                }
                j += 1;
            }
            let end_ok = j >= b.len() || !(b[j].is_ascii_digit() || b[j] == b',' || b[j] == b'.');
            if nums.len() == 3 && end_ok {
                out.push((i, j, [nums[0], nums[1], nums[2]]));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `text` with every `x,y,z` token moved by `d`.
fn move_cell_tokens(text: &str, d: P) -> String {
    let mut s = String::new();
    let mut last = 0;
    for (a, b, c) in cell_tokens(text) {
        s.push_str(&text[last..a]);
        s.push_str(&format!("{},{},{}", c[0] + d[0], c[1] + d[1], c[2] + d[2]));
        last = b;
    }
    s.push_str(&text[last..]);
    s
}

/// (vcompose, G1 Nov 14 00:38 (3)) `text` with `tag` appended and its chain of `(carried from part
/// …)` tags deduplicated: a tag equal to an earlier one in the line is dropped, so a line carried
/// through several composes names each part once, in order.
fn tagged(text: &str, tag: &str) -> String {
    let full = format!("{} {tag}", text.trim_end());
    let mut out = String::new();
    let mut seen: Vec<&str> = Vec::new();
    let mut rest = full.as_str();
    while let Some(i) = rest.find("(carried from part ") {
        let (before, from) = rest.split_at(i);
        let Some(j) = from.find(')') else { break };
        let group = &from[..=j];
        if seen.contains(&group) {
            out.push_str(before.trim_end());
        } else {
            out.push_str(before);
            out.push_str(group);
            seen.push(group);
        }
        rest = &from[j + 1..];
    }
    out.push_str(rest);
    out
}

/// [`carried_declarations`] under a [`CarryFilter`]: see [`Carried`].
pub fn carried_filtered(specs: &[PartSpec], shift: P, filt: &CarryFilter) -> Result<Carried, String> {
    const ENTRY_META: [&str; 7] = ["placement-order-ts", "update-only-ts", "emulated", "keep-block", "unreachable", "waker-places", "riderless"];
    const ENTRY_HASH: [&str; 16] = ["shape", "switch", "switches", "update-only", "contacts", "travel", "always-on", "out-of-scope", "water", "single-occupant", "own-route-junctions", "one-way", "a6-pickup", "stand-in-guard", "candidate", "contact-speeds"];
    // (vcompose: the station's `# held: x,y,z state (why)` is a leading-cell line too)
    const LEAD_HASH: [&str; 2] = ["stand-in", "held"];
    // (vcompose: an `# integrity:` line is machine-read, `x,y,z=connections a+b; holds c=shape`,
    // f1q2-run's --integrity-from: every cell in it moves with the part)
    const WHOLE_HASH: [&str; 1] = ["integrity"];
    const LIST_META: [&str; 2] = ["keep-clear", "keep-out"];
    const NAMED_META: [&str; 2] = ["run-cells", "run-gates"];
    const SCALAR_META: [&str; 4] = ["circuit", "dwell-cap", "k-lock", "k-sw"];
    const SCALAR_HASH: [&str; 1] = ["day-time"];
    // (the composer moves these itself: the part's name, frames, palette and layers, declared states,
    // ground, entities, and (onechunk 12:32, the loader's chests) its block entities' `nbt:` lines)
    const OWN_META: [&str; 9] = ["name", "origin", "nether-origin", "palette", "positions", "declared", "ground", "entity", "nbt"];
    // (vcompose: the traced compose's own lines are re-derived by every compose, never carried)
    const OWN_HASH: [&str; 11] = ["placement order", "rail T", "rail Ts", "settled junction", "+ ", "precedence", "outside-portals", "uncovered", "superseded", "cleared", "absent"];
    let cell3 = |t: &str| -> Option<[i32; 3]> {
        // (vcompose: a leading cell may end in ':', `# stand-in-guard: 9,25,9: golem=ok ...`)
        let v: Vec<i32> = t.trim().trim_end_matches(':').trim_start_matches('(').trim_end_matches(')').split(',').map(|n| n.trim().parse::<i32>()).collect::<Result<_, _>>().ok()?;
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    };
    let mut heads: Vec<String> = Vec::new();
    let mut metas: Vec<(String, Vec<String>)> = Vec::new();
    let mut scalars: Vec<(String, String, String)> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    let mut stale_kept: Vec<String> = Vec::new();
    let push_meta = |k: &str, v: String, metas: &mut Vec<(String, Vec<String>)>| match metas.iter_mut().find(|(m, _)| m == k) {
        Some((_, l)) => l.push(v),
        None => metas.push((k.to_string(), vec![v])),
    };
    for (s, is_delta) in specs.iter().filter(|s| !s.nether).map(|s| (s, false)).chain(filt.deltas.iter().map(|s| (s, true))) {
        let text = std::fs::read_to_string(&s.file).map_err(|e| format!("part {}: {}: {e}", s.name, s.file.display()))?;
        let pm = [s.frame[0] + s.tier[0], s.frame[1] + s.tier[1], s.frame[2] + s.tier[2]];
        let d = [pm[0] + shift[0], pm[1] + shift[1], pm[2] + shift[2]];
        let kept = |c: [i32; 3]| s.keep.is_none_or(|(a, b)| (0..3).all(|i| c[i] >= a[i].min(b[i]) && c[i] <= a[i].max(b[i])));
        let mv = |c: [i32; 3]| format!("{},{},{}", c[0] + d[0], c[1] + d[1], c[2] + d[2]);
        let tag = format!("(carried from part {}, moved by {},{},{})", s.name, d[0], d[1], d[2]);
        let is_stale = filt.stale.as_deref() == Some(s.name.as_str());
        // (G1 Nov 13 23:21, D1) what this part adds, so the stale part's carried entries are listed
        let (h0, m0): (usize, Vec<(String, usize)>) = (heads.len(), metas.iter().map(|(k, l)| (k.clone(), l.len())).collect());
        // (vcompose) why the filter drops a cell of this part, placed in the column's frame; `air_ok`
        // for the kinds that name air cells by design (keep-out / keep-clear, run cells and gates,
        // unreachable and waker places), which the empty-cell rule leaves alone
        let gone = |c: [i32; 3], air_ok: bool| -> Option<&'static str> {
            let p = add(c, pm);
            if !s.relay && filt.clear.iter().any(|b| inside(&p, b)) {
                Some("its cell is cleared")
            } else if is_stale && filt.held.contains(&p) {
                Some("an owner part holds its cell")
            } else if filt.lost.iter().any(|(n, q)| *n == s.name && *q == p) {
                Some("a precedence line gives its cell to another part")
            } else if is_stale && !air_ok && filt.record.as_ref().is_some_and(|r| !r.contains(&p)) && !filt.keep_stale.iter().any(|(q, _)| *q == p) {
                Some("its cell is empty in the record (G1 Nov 13 23:21 D1: a stale declaration naming no block of the record; --keep-stale x,y,z=REASON keeps one by name)")
            } else {
                None
            }
        };
        // an ENTRY (`cells=REASON`): Ok(None) when the box drops one of its cells, Ok(Some(Err(why)))
        // when the filter drops it, Err when its head isn't cells
        let entry = |e: &str, air_ok: bool| -> Result<Option<Result<String, &'static str>>, String> {
            let (head, why) = e.split_once('=').ok_or_else(|| format!("part {}: {e:?}: not CELLS=REASON", s.name))?;
            // (vcompose: a `# travel:` head joins its cells with '>', the others with '+'; both
            // kept as written)
            let seps: Vec<char> = head.chars().filter(|ch| *ch == '+' || *ch == '>').collect();
            let mut cells: Vec<String> = Vec::new();
            for c in head.split(['+', '>']) {
                let c = cell3(c).ok_or_else(|| format!("part {}: {e:?}: {c:?} is not a cell", s.name))?;
                if !kept(c) {
                    return Ok(None);
                }
                if let Some(w) = gone(c, air_ok) {
                    return Ok(Some(Err(w)));
                }
                cells.push(mv(c));
            }
            let mut joined = String::new();
            for (k, c) in cells.iter().enumerate() {
                if k > 0 {
                    joined.push(seps.get(k - 1).copied().unwrap_or('+'));
                }
                joined.push_str(c);
            }
            Ok(Some(Ok(tagged(&format!("{joined}={}", why.trim()), &tag))))
        };
        // (vcompose) a whole line whose every cell moves (an `# integrity:` line, or an owner's
        // line in an entry kind but not in the CELLS=REASON form, carried verbatim at a zero move):
        // None when the box drops one of its cells
        let whole = |v: &str| -> Option<Result<String, &'static str>> {
            let toks = cell_tokens(v);
            if toks.iter().any(|(_, _, c)| !kept(*c)) {
                return None;
            }
            if let Some(w) = toks.iter().find_map(|(_, _, c)| gone(*c, false)) {
                return Some(Err(w));
            }
            Some(Ok(move_cell_tokens(v.trim(), d)))
        };
        let short = |t: &str| t.chars().take(160).collect::<String>();
        for l in text.lines() {
            let t = l.trim_end();
            if t.is_empty() || t.starts_with("layer") || t.starts_with("nether") && !t.starts_with("nether-origin") {
                continue;
            }
            if let Some(body) = t.strip_prefix('#') {
                let body = body.trim_start();
                let Some((key, val)) = body.split_once(':') else { continue };
                let key = key.trim();
                if OWN_HASH.iter().any(|k| key.starts_with(k)) || key.contains(' ') && !OWN_HASH.contains(&key) && !val.trim_start().split([' ', '=', '+', ';']).next().is_some_and(|c| cell3(c).is_some()) {
                    continue;
                }
                let first_is_cell = val.trim_start().split([' ', '=', '+', ';']).next().is_some_and(|c| cell3(c).is_some());
                if ENTRY_HASH.contains(&key) {
                    let entries = crate::pushscan::header_entries(t, &format!("{key}:"));
                    let parsed: Vec<_> = entries.iter().map(|e| entry(e, false)).collect();
                    if parsed.iter().any(|r| r.is_err()) && first_is_cell {
                        // (vcompose) an owner's line in this kind but not in the CELLS=REASON form
                        // (the lift's `# shape: (x,y,z) north_west; held: ...`): carried verbatim
                        // only at a zero move, where no cell of it changes; a line the part's box
                        // drops is skipped; else it fails as before
                        let Some(r) = whole(val) else { continue };
                        if d != [0, 0, 0] && !is_stale {
                            return Err(format!("part {} ({}): its `# {key}:` line isn't in the CELLS=REASON form and the part moves by {},{},{}: the composer can't move it: {}", s.name, s.file.display(), d[0], d[1], d[2], short(t)));
                        }
                        match r {
                            Ok(m) if !is_stale => heads.push(format!("# {key}: {m} (carried verbatim from part {}: not in the CELLS=REASON form)", s.name)),
                            Ok(_) => dropped.push(format!("{} `# {key}:` {}: the stale part's line in a form the composer can't split", s.name, short(val.trim()))),
                            Err(w) => dropped.push(format!("{} `# {key}:` {}: {w}", s.name, short(val.trim()))),
                        }
                        continue;
                    }
                    for (e, r) in entries.iter().zip(parsed) {
                        match r? {
                            Some(Ok(m)) => heads.push(format!("# {key}: {m}")),
                            Some(Err(w)) => dropped.push(format!("{} `# {key}:` {}: {w}", s.name, short(e))),
                            None => {}
                        }
                    }
                } else if LEAD_HASH.contains(&key) {
                    let v = val.trim_start();
                    let (c, rest) = v.split_once(' ').unwrap_or((v, ""));
                    let c3 = cell3(c).ok_or_else(|| format!("part {}: `# {key}: {v}`: no leading cell", s.name))?;
                    if kept(c3) {
                        match gone(c3, false) {
                            None => heads.push(tagged(&format!("# {key}: {} {}", mv(c3), rest.trim()), &tag)),
                            Some(w) => dropped.push(format!("{} `# {key}:` {}: {w}", s.name, short(v))),
                        }
                    }
                } else if WHOLE_HASH.contains(&key) {
                    match whole(val) {
                        None => {}
                        Some(Ok(m)) => heads.push(tagged(&format!("# {key}: {m}"), &tag)),
                        Some(Err(w)) => dropped.push(format!("{} `# {key}:` {}: {w}", s.name, short(val.trim()))),
                    }
                } else if SCALAR_HASH.contains(&key) {
                    scalars.push((format!("# {key}"), val.trim().to_string(), s.name.clone()));
                } else if first_is_cell {
                    if is_delta {
                        // (vcompose) a delta's other `#` lines are its owner's prose (the
                        // designed falls it names, its ADD / REMOVE headings): not declarations
                        continue;
                    }
                    if is_stale {
                        // (vcompose) the stale part's unknown kinds are dropped by name, never
                        // carried (its owners' files declare their own cells)
                        dropped.push(format!("{} `# {key}:` {}: a kind the composer can't carry, from the stale part", s.name, short(val.trim())));
                        continue;
                    }
                    // (vcompose) a line naming only cells outside the part's kept box isn't this
                    // part's: skipped, as an entry outside the box is
                    if s.keep.is_some() && cell_tokens(val).iter().all(|(_, _, c)| !kept(*c)) {
                        dropped.push(format!("{} `# {key}:` {}: a kind the composer can't carry, naming no cell the part's box keeps", s.name, short(val.trim())));
                        continue;
                    }
                    return Err(format!("part {} ({}): the declaration `# {key}:` is a kind the composer can't carry (G1 09:54: an unknown meta line fails the compose): {}", s.name, s.file.display(), t.chars().take(120).collect::<String>()));
                }
                continue;
            }
            let Some((key, val)) = t.split_once(':') else { continue };
            let key = key.trim();
            if key.contains(' ') || !key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                continue;
            }
            if OWN_META.contains(&key) {
                continue;
            }
            if ENTRY_META.contains(&key) {
                for e in crate::pushscan::header_entries(&format!("# {t}"), &format!("{key}:")) {
                    match entry(&e, key == "unreachable" || key == "waker-places")? {
                        Some(Ok(m)) => push_meta(key, m, &mut metas),
                        Some(Err(w)) => dropped.push(format!("{} `{key}:` {}: {w}", s.name, short(&e))),
                        None => {}
                    }
                }
            } else if LIST_META.contains(&key) {
                let v = val.trim();
                let (cells, why) = match v.find('(') {
                    Some(i) => (&v[..i], v[i..].trim()),
                    None => (v, ""),
                };
                let mut out: Vec<String> = Vec::new();
                for c in cells.split_whitespace() {
                    let c3 = cell3(c).ok_or_else(|| format!("part {}: `{key}: {v}`: {c:?} is not a cell", s.name))?;
                    if kept(c3) {
                        match gone(c3, true) {
                            None => out.push(mv(c3)),
                            Some(w) => dropped.push(format!("{} `{key}:` {c}: {w}", s.name)),
                        }
                    }
                }
                if !out.is_empty() {
                    push_meta(key, tagged(&format!("{} {why}", out.join(" ")), &tag), &mut metas);
                }
            } else if NAMED_META.contains(&key) {
                for e in val.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                    let (name, cells) = e.split_once(' ').ok_or_else(|| format!("part {}: `{key}`: {e:?}: not NAME x,y,z", s.name))?;
                    let mut out: Vec<String> = Vec::new();
                    let mut keep = true;
                    let mut why: Option<&str> = None;
                    for c in cells.trim().split('/') {
                        let c3 = cell3(c).ok_or_else(|| format!("part {}: `{key}`: {e:?}: {c:?} is not a cell", s.name))?;
                        keep &= kept(c3);
                        why = why.or(gone(c3, true));
                        out.push(mv(c3));
                    }
                    // (vcompose) an item an earlier part already names with the same cells is
                    // carried once; the stale part's under another name is dropped by name
                    let cells_s = out.join("/");
                    let dup = metas.iter().find(|(m, _)| m == key).is_some_and(|(_, l)| l.iter().any(|it| it.split_once(' ').is_some_and(|(n, c)| c == cells_s && (n == name || is_stale))));
                    match (keep, why, dup) {
                        (true, None, false) => push_meta(key, format!("{name} {cells_s}"), &mut metas),
                        (true, None, true) => dropped.push(format!("{} `{key}:` {e}: an earlier part names the same cells", s.name)),
                        (true, Some(w), _) => dropped.push(format!("{} `{key}:` {e}: {w}", s.name)),
                        _ => {}
                    }
                }
            } else if SCALAR_META.contains(&key) {
                scalars.push((key.to_string(), val.trim().to_string(), s.name.clone()));
            } else if is_stale || (s.keep.is_some() && cell_tokens(val).iter().all(|(_, _, c)| !kept(*c))) {
                // (vcompose) the stale part's unknown meta lines, and a boxed part's naming no cell
                // its box keeps (the file's own description, `way-in: ...`), are dropped by name
                dropped.push(format!("{} `{key}:` {}: a meta kind the composer can't carry, {}", s.name, short(val.trim()), if is_stale { "from the stale part" } else { "naming no cell the part's box keeps" }));
            } else {
                return Err(format!("part {} ({}): the meta line `{key}:` is a kind the composer can't carry (G1 09:54: an unknown meta line fails the compose)", s.name, s.file.display()));
            }
        }
        if is_stale {
            stale_kept.extend(heads[h0..].iter().cloned());
            for (k, l) in &metas {
                let before = m0.iter().find(|(k0, _)| k0 == k).map(|(_, n)| *n).unwrap_or(0);
                stale_kept.extend(l[before..].iter().map(|v| format!("{k}: {v}")));
            }
        }
    }
    // (a scalar: one value across the parts, else the compose fails)
    let mut out_meta: Vec<(String, String)> = Vec::new();
    for (k, v, part) in &scalars {
        if let Some((_, v0, p0)) = scalars.iter().find(|(k2, _, _)| k2 == k)
            && v0 != v
        {
            return Err(format!("the scalar `{k}` differs across parts ({p0}: {v0:?}; {part}: {v:?}): the compose can't carry both"));
        }
        if k.starts_with('#') {
            let l = format!("{k}: {v}");
            if !heads.contains(&l) {
                heads.push(l);
            }
        } else if !out_meta.iter().any(|(k2, _)| k2 == k) {
            out_meta.push((k.clone(), v.clone()));
        }
    }
    let sep = |k: &str| if NAMED_META.contains(&k) { "; " } else if LIST_META.contains(&k) { " " } else { "; " };
    out_meta.extend(metas.into_iter().map(|(k, v)| {
        let j = v.join(sep(&k));
        (k, j)
    }));
    Ok(Carried { heads, metas: out_meta, dropped, stale_kept })
}

fn triple(s: &str) -> Result<P, String> {
    match ints(s)?.as_slice() {
        [x, y, z] => Ok([*x, *y, *z]),
        _ => Err(format!("{s:?}: give x,y,z")),
    }
}

impl PartSpec {
    pub fn parse(s: &str) -> Result<PartSpec, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("--part {s:?}: NAME=FILE[%BASE][@fx,fy,fz][^tx,ty,tz][#x0,y0,z0,x1,y1,z1]"))?;
        let marks = ['@', '^', '#'];
        let cut = rest.find(marks).unwrap_or(rest.len());
        let flags = name.trim();
        let bare = flags.trim_end_matches(['!', '~']);
        let marks_of = &flags[bare.len()..];
        let (file, base) = match rest[..cut].split_once('%') {
            Some((f, b)) => (PathBuf::from(f), Some(PathBuf::from(b))),
            None => (PathBuf::from(&rest[..cut]), None),
        };
        let mut p = PartSpec { name: bare.to_string(), file, base, frame: [0; 3], tier: [0; 3], keep: None, relay: marks_of.contains('!'), nether: marks_of.contains('~') };
        let mut tail = &rest[cut..];
        while let Some(m) = tail.chars().next() {
            let body = &tail[1..];
            let end = body.find(marks).unwrap_or(body.len());
            let v = &body[..end];
            match m {
                '@' => p.frame = triple(v)?,
                '^' => p.tier = triple(v)?,
                _ => {
                    let n = ints(v)?;
                    if n.len() != 6 {
                        return Err(format!("--part {s:?}: the kept box takes x0,y0,z0,x1,y1,z1"));
                    }
                    p.keep = Some(([n[0].min(n[3]), n[1].min(n[4]), n[2].min(n[5])], [n[0].max(n[3]), n[1].max(n[4]), n[2].max(n[5])]));
                }
            }
            tail = &body[end..];
        }
        Ok(p)
    }
}

fn inside(p: &P, b: &(P, P)) -> bool {
    (0..3).all(|i| b.0[i] <= p[i] && p[i] <= b.1[i])
}

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A design's `positions:` line (x,y,z words).
fn positions_of(d: &Design) -> Result<Vec<P>, String> {
    d.meta.get("positions").map(|l| l.split_whitespace().map(triple).collect()).unwrap_or(Ok(Vec::new()))
}

/// The part's design with only the kept cells: blocks, block entities, entities by their cell,
/// ports and `positions:`; the Nether part only when its anchor is kept.
fn kept(d: &Design, keep: Option<(P, P)>) -> Result<Design, String> {
    let Some(b) = keep else { return Ok(d.clone()) };
    let mut k = d.clone();
    k.blocks.retain(|p, _| inside(p, &b));
    k.block_entities.retain(|p, _| inside(p, &b));
    k.entities.retain(|e| inside(&e.cell(), &b));
    let anchor = k.ports.get("nether-anchor").is_some_and(|p| inside(p, &b));
    k.ports.retain(|n, p| inside(p, &b) || (anchor && n == "nether-axis"));
    if !anchor {
        k.ports.remove("nether-anchor");
        k.ports.remove("nether-axis");
        k.nether_blocks.clear();
        k.nether_ports.clear();
        k.nether_block_entities.clear();
    }
    let pos: Vec<String> = positions_of(d)?.into_iter().filter(|p| inside(p, &b)).map(|p| format!("{},{},{}", p[0], p[1], p[2])).collect();
    if pos.is_empty() {
        k.meta.remove("positions");
    } else {
        k.meta.insert("positions".into(), pos.join(" "));
    }
    Ok(k)
}

/// A part read and moved: as committed in the column's frame, and placed in its tier.
pub struct Part {
    pub spec: PartSpec,
    pub committed: Design,
    pub placed: Design,
    /// Its `positions:` cells, placed.
    pub positions: Vec<P>,
    /// Its `declared:` states, placed (cells and control blocks moved).
    pub declared: Vec<crate::declared::Decl>,
    /// (G1 on 39925b692) a `%BASE` part's cells BASE also holds with another state: left to BASE's
    /// parts, each wants a `--block` override.
    pub base_changed: Vec<String>,
}

/// A reserved box (`NAME=x0,y0,z0,x1,y1,z1`, the column's frame): a part not yet laid out for the
/// column (a fold its owner re-generates, a run not yet benched), in the fit, clash and tier report
/// but holding no block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reserve {
    pub name: String,
    pub b: (P, P),
    /// (vcompose, G1 Nov 14 01:22) who the reserved cells are held for, and the ruled item whose
    /// landing retires the reservation (`NAME=BOX:OWNER:ITEM`).
    pub owner: Option<String>,
    pub item: Option<String>,
}

impl Reserve {
    pub fn parse(s: &str) -> Result<Reserve, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("--reserve {s:?}: NAME=x0,y0,z0,x1,y1,z1[:OWNER:ITEM]"))?;
        let mut it = rest.splitn(3, ':');
        let n = ints(it.next().unwrap_or(""))?;
        if n.len() != 6 {
            return Err(format!("--reserve {s:?}: NAME=x0,y0,z0,x1,y1,z1[:OWNER:ITEM]"));
        }
        let (owner, item) = (it.next().map(|o| o.trim().to_string()), it.next().map(|i| i.trim().to_string()));
        if owner.is_some() != item.is_some() || owner.as_deref() == Some("") || item.as_deref() == Some("") {
            return Err(format!("--reserve {s:?}: name both the OWNER and the ITEM (NAME=x0,y0,z0,x1,y1,z1:OWNER:ITEM)"));
        }
        Ok(Reserve { name: name.trim().to_string(), b: ([n[0].min(n[3]), n[1].min(n[4]), n[2].min(n[5])], [n[0].max(n[3]), n[1].max(n[4]), n[2].max(n[5])]), owner, item })
    }
}

/// The design moved by `d`, its `positions:` line too.
fn moved(d: &Design, by: P) -> Result<(Design, Vec<P>), String> {
    let m = d.translated(by);
    let pos: Vec<P> = positions_of(d)?.into_iter().map(|p| add(p, by)).collect();
    Ok((m, pos))
}

/// A whole skeleton moved by `by` for writing in another frame (farmopt Nov 11 16:45: the runner,
/// `borders` and `paste` take a file's frame as the world's, chunk lines every 16 from 0, so the
/// column x, z -6..9 is written at +6,0,+6): blocks, entities, ports, `positions:`, `declared:`
/// (cells and control blocks) and `ground:`.
pub fn shifted(d: &Design, by: P) -> Result<Design, String> {
    let (mut m, pos) = moved(d, by)?;
    if !pos.is_empty() {
        m.meta.insert("positions".into(), pos.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])).collect::<Vec<_>>().join(" "));
    }
    if let Some(l) = d.meta.get("declared") {
        let ds: Vec<crate::declared::Decl> = crate::declared::parse(l)?.into_iter().map(|x| crate::declared::Decl { cell: add(x.cell, by), ctl: x.ctl.map(|c| add(c, by)), ..x }).collect();
        m.meta.insert("declared".into(), crate::declared::format(&ds));
    }
    if let Some(g) = d.meta.get("ground") {
        let y: i32 = g.trim().parse().map_err(|e| format!("ground: {g}: {e}"))?;
        m.meta.insert("ground".into(), (y + by[1]).to_string());
    }
    // (the runner's named cells, `column-run`'s header: `NAME x,y,z[/x,y,z]; ...`)
    for key in ["run-gates", "run-cells"] {
        if let Some(l) = d.meta.get(key) {
            let mut items = Vec::new();
            for item in l.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let (name, rest) = item.split_once(' ').ok_or_else(|| format!("{key}: {item}"))?;
                let ps: Vec<String> = rest
                    .trim()
                    .split('/')
                    .map(|p| {
                        let n: Vec<i32> = p.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{key}: {item}: {e}"))).collect::<Result<_, _>>()?;
                        if n.len() != 3 {
                            return Err(format!("{key}: {item}: want x,y,z"));
                        }
                        Ok(format!("{},{},{}", n[0] + by[0], n[1] + by[1], n[2] + by[2]))
                    })
                    .collect::<Result<_, String>>()?;
                items.push(format!("{name} {}", ps.join("/")));
            }
            m.meta.insert(key.into(), items.join("; "));
        }
    }
    if let Some(c) = d.meta.get("run-controller") {
        m.meta.insert("run-controller".into(), c.clone());
    }
    Ok(m)
}

/// A `run-gates:` / `run-cells:` line's items (`NAME x,y,z[/x,y,z]`) with every cell moved by `by`.
fn named_cells_moved(l: &str, key: &str, by: P) -> Result<Vec<String>, String> {
    let mut items = Vec::new();
    for item in l.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, rest) = item.split_once(' ').ok_or_else(|| format!("{key}: {item}"))?;
        let mut ps = Vec::new();
        for p in rest.trim().split('/') {
            let n: Vec<i32> = p.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("{key}: {item}: {e}"))).collect::<Result<_, _>>()?;
            if n.len() != 3 {
                return Err(format!("{key}: {item}: want x,y,z"));
            }
            ps.push(format!("{},{},{}", n[0] + by[0], n[1] + by[1], n[2] + by[2]));
        }
        items.push(format!("{name} {}", ps.join("/")));
    }
    Ok(items)
}

pub fn load_part(spec: &PartSpec) -> Result<Part, String> {
    let text = std::fs::read_to_string(&spec.file).map_err(|e| format!("{}: {e}", spec.file.display()))?;
    let mut d = Design::parse(&text).map_err(|e| format!("{}: {e}", spec.file.display()))?;
    let mut base_changed: Vec<String> = Vec::new();
    if let Some(base) = &spec.base {
        // (`%BASE`: only what the file adds to BASE, b8949c96's way-in laid onto the module: its
        // cells BASE leaves empty, its entities BASE lacks; a cell both hold, changed or not, stays
        // BASE's part's, so a changed one is an override)
        let bt = std::fs::read_to_string(base).map_err(|e| format!("{}: {e}", base.display()))?;
        let b = Design::parse(&bt).map_err(|e| format!("{}: {e}", base.display()))?;
        base_changed = d.blocks.iter().filter(|(p, s)| b.blocks.get(*p).is_some_and(|bs| bs != *s)).map(|(p, s)| format!("({},{},{}) {} in {}, {} in BASE", p[0], p[1], p[2], s, spec.file.display(), b.blocks[p])).collect();
        d.blocks.retain(|p, _| !b.blocks.contains_key(p));
        d.block_entities.retain(|p, _| !b.block_entities.contains_key(p));
        d.entities.retain(|e| !b.entities.contains(e));
        d.ports.clear();
        d.nether_blocks.clear();
        d.nether_ports.clear();
        d.nether_block_entities.clear();
        d.meta.remove("positions");
        d.meta.remove("declared");
    }
    if spec.nether {
        // (the file's Nether part as the part's cells, in the Nether frame the file writes it in)
        d.blocks = std::mem::take(&mut d.nether_blocks);
        d.block_entities = std::mem::take(&mut d.nether_block_entities);
        d.ports = std::mem::take(&mut d.nether_ports);
        d.entities.clear();
        d.meta.remove("positions");
        d.meta.remove("declared");
    }
    let k = kept(&d, spec.keep)?;
    if k.blocks.is_empty() {
        return Err(format!("part {}: no block of {} in the kept box", spec.name, spec.file.display()));
    }
    let (committed, _) = moved(&k, spec.frame)?;
    let by = add(spec.frame, spec.tier);
    let (placed, positions) = moved(&k, by)?;
    let declared = crate::declared::parse(d.meta.get("declared").map(String::as_str).unwrap_or(""))?
        .into_iter()
        .filter(|x| spec.keep.is_none_or(|b| inside(&x.cell, &b)))
        .map(|x| crate::declared::Decl { cell: add(x.cell, by), ctl: x.ctl.map(|c| add(c, by)), ..x })
        .collect();
    Ok(Part { spec: spec.clone(), committed, placed, positions, declared, base_changed })
}

/// The column: its cells x0..=x1, z0..=z1 in the frame, and the chunk lines' phase (a line at
/// `phase + 16k` lies between cells line - 1 and line).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
    pub phase_x: i32,
    pub phase_z: i32,
}

/// The chunk lines a span of cells lo..=hi crosses.
fn lines_crossed(lo: i32, hi: i32, phase: i32) -> Vec<i32> {
    (lo + 1..=hi).filter(|c| (c - phase).rem_euclid(16) == 0).collect()
}

fn span(d: &Design) -> (P, P) {
    d.bounds().unwrap_or(([0; 3], [0; 3]))
}

fn size(b: &(P, P)) -> P {
    [b.1[0] - b.0[0] + 1, b.1[1] - b.0[1] + 1, b.1[2] - b.0[2] + 1]
}

fn fmt_box(b: &(P, P)) -> String {
    let s = size(b);
    format!("x {}..{} y {}..{} z {}..{} ({} x {} x {}, {} blocks³)", b.0[0], b.1[0], b.0[1], b.1[1], b.0[2], b.1[2], s[0], s[1], s[2], s[0] as i64 * s[1] as i64 * s[2] as i64)
}

/// The entities' cells' box (riders' carts, zombies' boats), if any.
fn entity_box(d: &Design) -> Option<(P, P)> {
    let mut it = d.entities.iter().map(|e| e.cell());
    let f = it.next()?;
    Some(it.fold((f, f), |(lo, hi), c| ([lo[0].min(c[0]), lo[1].min(c[1]), lo[2].min(c[2])], [hi[0].max(c[0]), hi[1].max(c[1]), hi[2].max(c[2])])))
}

/// Where a box leaves the column (cells past each side), or None inside it.
fn overhang(b: &(P, P), f: &Frame) -> Option<String> {
    let sides = [("west", f.x0 - b.0[0]), ("east", b.1[0] - f.x1), ("north", f.z0 - b.0[2]), ("south", b.1[2] - f.z1)];
    let out: Vec<String> = sides.iter().filter(|(_, n)| *n > 0).map(|(s, n)| format!("{n} {s}")).collect();
    (!out.is_empty()).then(|| out.join(", "))
}

fn lines_text(b: &(P, P), f: &Frame) -> String {
    let xs = lines_crossed(b.0[0], b.1[0], f.phase_x);
    let zs = lines_crossed(b.0[2], b.1[2], f.phase_z);
    let fmt = |axis: &str, v: &[i32]| v.iter().map(|c| format!("{axis} {}|{c}", c - 1)).collect::<Vec<_>>();
    let all: Vec<String> = fmt("x", &xs).into_iter().chain(fmt("z", &zs)).collect();
    if all.is_empty() { "no chunk line".into() } else { format!("chunk lines {}", all.join(", ")) }
}

fn counts(d: &Design) -> String {
    let riders = d.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|n| n.contains("villager"))).count();
    let carts = d.entities.iter().filter(|e| e.kind.ends_with("minecart")).count();
    let zombies = d.entities.iter().filter(|e| e.nbt.as_deref().is_some_and(|n| n.contains("zombie")) || e.kind.ends_with("zombie")).count();
    format!("{} blocks, {} carts ({} ridden), {} zombies{}", d.blocks.len(), carts, riders, zombies, if d.nether_blocks.is_empty() { String::new() } else { format!(", a Nether part of {} blocks", d.nether_blocks.len()) })
}

/// The report's part lines: committed (frame), placed (tier), the column's fit.
fn part_lines(p: &Part, f: &Frame) -> String {
    let c = span(&p.committed);
    let t = span(&p.placed);
    let mut s = format!("part {} ({}{}): {}\n", p.spec.name, p.spec.file.display(), p.spec.keep.map(|k| format!(", kept {}", fmt_box(&k))).unwrap_or_default(), counts(&p.committed));
    if !p.base_changed.is_empty() {
        // (G1 on 39925b692: what `%BASE` leaves to BASE's parts, each a `--block` override to lay)
        s.push_str(&format!("  {} cells BASE also holds with another state (left to BASE's parts: each wants a --block override): {}\n", p.base_changed.len(), p.base_changed.join("; ")));
    }
    s.push_str(&format!("  committed in the frame (file + {:?}): {}; {}\n", p.spec.frame, fmt_box(&c), lines_text(&c, f)));
    if let Some(e) = entity_box(&p.committed) {
        s.push_str(&format!("  its entities' cells: x {}..{} y {}..{} z {}..{}\n", e.0[0], e.1[0], e.0[1], e.1[1], e.0[2], e.1[2]));
    }
    s.push_str(&format!("  placed (+ {:?}): {}; {}; {}\n", p.spec.tier, fmt_box(&t), lines_text(&t, f), match overhang(&t, f) {
        Some(o) => format!("LEAVES the column by {o}"),
        None => "inside the column".into(),
    }));
    s
}

/// The cells two placed parts both hold, by pair; then each reserved box against the parts' blocks
/// and the other reserved boxes.
fn clashes(parts: &[Part], reserves: &[Reserve]) -> Vec<String> {
    // (vcompose: two parts writing one cell are judged before this, by `apply_precedence`: a
    // different state fails the compose by name, the same state is kept once and counted; what is
    // left here is a reserved box over a part or another reserved box)
    let mut out = Vec::new();
    for (i, r) in reserves.iter().enumerate() {
        for p in parts {
            let n = p.placed.blocks.keys().filter(|c| inside(c, &r.b)).count();
            if n > 0 {
                out.push(format!("{} (reserved) and {}: {n} of {}'s cells inside the reserved box", r.name, p.spec.name, p.spec.name));
            }
        }
        for s in &reserves[i + 1..] {
            if (0..3).all(|k| r.b.0[k] <= s.b.1[k] && s.b.0[k] <= r.b.1[k]) {
                out.push(format!("{} and {} (both reserved): the boxes overlap", r.name, s.name));
            }
        }
    }
    out
}

/// The tiers from the top down: each part's (or reserved box's) y range, its height and the gap
/// to the next.
fn tiers(parts: &[Part], reserves: &[Reserve]) -> String {
    let mut v: Vec<(String, (P, P))> = parts.iter().map(|p| (p.spec.name.clone(), span(&p.placed))).chain(reserves.iter().map(|r| (format!("{} (reserved)", r.name), r.b))).collect();
    v.sort_by(|a, b| b.1.1[1].cmp(&a.1.1[1]).then(b.1.0[1].cmp(&a.1.0[1])));
    let mut s = String::from("tiers (top down, placed y; footprint x..x z..z):\n");
    for (i, (n, b)) in v.iter().enumerate() {
        let gap = v.get(i + 1).map(|(m, c)| format!("; {} layers from its bottom to the top of {m}", b.0[1] - c.1[1] - 1)).unwrap_or_default();
        s.push_str(&format!("  {n:<24} y {}..{} ({} high), x {}..{} z {}..{}{gap}\n", b.0[1], b.1[1], b.1[1] - b.0[1] + 1, b.0[0], b.1[0], b.0[2], b.1[2]));
    }
    s
}

/// The skeleton: every placed part merged (the first part's ports as they are, the others'
/// renamed `<part>.<port>`; one Nether part; `positions:` and `declared:` joined), and the cells
/// two parts declare differently. A cell two parts hold keeps the first part's block (the clash is
/// in the report).
fn skeleton(parts: &[Part], name: &str) -> Result<(Design, Vec<String>, Vec<String>), String> {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    d.meta.insert("circuit".into(), "mob-farm iron_ingot".into());
    let mut pos: Vec<String> = Vec::new();
    let mut decl: Vec<(String, crate::declared::Decl)> = Vec::new();
    let mut two_ways = Vec::new();
    let mut dropped = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        for (c, s) in &p.placed.blocks {
            d.blocks.entry(*c).or_insert_with(|| s.clone());
        }
        for (c, s) in &p.placed.block_entities {
            d.block_entities.entry(*c).or_insert_with(|| s.clone());
        }
        d.entities.extend(p.placed.entities.iter().cloned());
        let nether = !p.placed.nether_blocks.is_empty();
        if nether && !d.nether_blocks.is_empty() {
            return Err(format!("part {}: a second Nether part", p.spec.name));
        }
        for (k, c) in &p.placed.ports {
            // (the circuit loader takes only its own port names: a later part's ports are dropped
            // and listed, but for its shared bed's `home` and a Nether part's anchor and axis)
            let keep = i == 0 || (nether && (k == "nether-anchor" || k == "nether-axis")) || (k == "home" && !d.ports.contains_key("home"));
            if keep {
                d.ports.insert(k.clone(), *c);
            } else {
                dropped.push(format!("{}'s port {k} at ({},{},{})", p.spec.name, c[0], c[1], c[2]));
            }
        }
        if nether {
            d.nether_blocks = p.placed.nether_blocks.clone();
            d.nether_ports = p.placed.nether_ports.clone();
            d.nether_block_entities = p.placed.nether_block_entities.clone();
        }
        pos.extend(p.positions.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])));
        for x in &p.declared {
            match decl.iter().find(|(_, e)| e.cell == x.cell) {
                Some((who, e)) if e != x => two_ways.push(format!("({},{},{}) declared two ways: {} by {who}, {} by {}", x.cell[0], x.cell[1], x.cell[2], crate::declared::format(std::slice::from_ref(e)), crate::declared::format(std::slice::from_ref(x)), p.spec.name)),
                Some(_) => {}
                None => decl.push((p.spec.name.clone(), x.clone())),
            }
        }
    }
    if !pos.is_empty() {
        // (vcompose: a position two parts list, the stale record and its owner's file, once)
        let mut seen = std::collections::BTreeSet::new();
        pos.retain(|p| seen.insert(p.clone()));
        d.meta.insert("positions".into(), pos.join(" "));
    }
    // (onechunk Nov 12: the runner's named cells merged from the parts, moved with them: the first
    // part's names as they are, a later part's as `<part>.<NAME>`)
    for key in ["run-gates", "run-cells"] {
        let mut items: Vec<String> = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            if let Some(l) = p.committed.meta.get(key) {
                let by = add(p.spec.frame, p.spec.tier);
                for item in named_cells_moved(l, key, by)? {
                    items.push(if i == 0 { item } else { format!("{}.{item}", p.spec.name) });
                }
            }
        }
        if !items.is_empty() {
            d.meta.insert(key.into(), items.join("; "));
        }
    }
    let ds: Vec<crate::declared::Decl> = decl.into_iter().map(|(_, x)| x).collect();
    if !ds.is_empty() {
        d.meta.insert("declared".into(), crate::declared::format(&ds));
    }
    Ok((d, two_ways, dropped))
}

/// A part's top view in the frame (as committed): each column's top block, rails as `-`/`/`
/// (`P`/`p` powered), riders `c`, zombies `Z`, the chunk lines `:` in the ruler.
fn plan(p: &Part, f: &Frame) -> String {
    let d = &p.committed;
    let b = span(d);
    let mut top: BTreeMap<(i32, i32), (i32, char)> = BTreeMap::new();
    for (c, s) in &d.blocks {
        let n = s.strip_prefix("minecraft:").unwrap_or(s);
        let base = n.split('[').next().unwrap_or(n);
        let g = match base {
            "rail" => if n.contains("ascending") { '/' } else { '-' },
            "powered_rail" => if n.contains("ascending") { 'p' } else { 'P' },
            "detector_rail" => 'D',
            "activator_rail" => 'A',
            "nether_portal" => 'N',
            "obsidian" => 'o',
            "redstone_wire" => '*',
            "repeater" | "comparator" => 'r',
            "redstone_torch" | "redstone_wall_torch" => 't',
            "redstone_block" => 'R',
            _ if base.ends_with("_bed") => 'B',
            _ if base.ends_with("glass") => 'g',
            _ => '#',
        };
        let e = top.entry((c[0], c[2])).or_insert((c[1], g));
        if c[1] >= e.0 {
            *e = (c[1], g);
        }
    }
    for e in &d.entities {
        let c = e.cell();
        let g = if e.kind.ends_with("minecart") { 'c' } else if e.kind.ends_with("boat") { 'Z' } else { 'e' };
        top.insert((c[0], c[2]), (c[1], g));
    }
    let mut s = format!("plan of {} (frame cells, x {}..{} across, z {}..{} down; ':' a chunk line before that column):\n      ", p.spec.name, b.0[0], b.1[0], b.0[2], b.1[2]);
    for x in b.0[0]..=b.1[0] {
        s.push(if (x - f.phase_x).rem_euclid(16) == 0 { ':' } else { char::from_digit(x.rem_euclid(10) as u32, 10).unwrap_or('?') });
    }
    s.push('\n');
    for z in b.0[2]..=b.1[2] {
        s.push_str(&format!("{z:>4}{} ", if (z - f.phase_z).rem_euclid(16) == 0 { ':' } else { ' ' }));
        for x in b.0[0]..=b.1[0] {
            s.push(top.get(&(x, z)).map(|t| t.1).unwrap_or('.'));
        }
        s.push('\n');
    }
    s
}

/// The whole report, and the skeleton design (`ports`: set or replaced on the skeleton after the
/// merge, the judge's `port output` when no part keeps one; `extra`: parts built here, the sketch
/// tracks, carts and posts, after the file parts; `clear`: boxes whose file-part blocks, block
/// entities and entities are taken out first, a committed cell a lane re-lays).
#[allow(clippy::too_many_arguments)]
pub fn run(specs: &[PartSpec], extra: Vec<Part>, clear: &[(P, P)], reserves: &[Reserve], ports: &[(String, P)], f: &Frame, name: &str, with_plan: bool) -> Result<(String, Design), String> {
    run_bed(specs, extra, clear, reserves, ports, f, name, with_plan, None).map(|(s, d, _)| (s, d))
}

/// [`run`], and with `bed` (the occupied shared bed's head, and the parts exempt as the napper's
/// own path) the bed-radius report after the tiers, its verdict third; with `sentinel` (a room's
/// box) the sentinel's rule (c) and its chunk section (the written frame `shift`).
#[allow(clippy::too_many_arguments)]
pub fn run_bed(specs: &[PartSpec], extra: Vec<Part>, clear: &[(P, P)], reserves: &[Reserve], ports: &[(String, P)], f: &Frame, name: &str, with_plan: bool, bed: Option<(P, &[String])>) -> Result<(String, Design, bool), String> {
    run_full(specs, extra, clear, reserves, ports, f, name, with_plan, bed, None, [0; 3], None)
}

#[allow(clippy::too_many_arguments)]
pub fn run_full(specs: &[PartSpec], extra: Vec<Part>, clear: &[(P, P)], reserves: &[Reserve], ports: &[(String, P)], f: &Frame, name: &str, with_plan: bool, bed: Option<(P, &[String])>, sentinel_box: Option<(P, P)>, shift: P, summon: Option<(&[P], (P, P))>) -> Result<(String, Design, bool), String> {
    let clears: Vec<Clear> = clear.iter().map(|b| Clear { b: *b, why: "--clear".into(), must: false, absent: None }).collect();
    run_traced(specs, extra, &clears, reserves, ports, f, name, with_plan, bed, sentinel_box, shift, summon, &Trace::default()).map(|(s, d, ok, _)| (s, d, ok))
}

/// (vcompose) A clear: its box (a cell, for a `--clear-part` or a delta's remove line), who asked
/// for it, and whether it must take something out (a `--clear` box or a delta's remove line: a
/// no-op refuses by name, ironspec Nov 13 21:55; a `--clear-part` cell may find nothing).
#[derive(Clone, Debug)]
pub struct Clear {
    pub b: (P, P),
    pub why: String,
    pub must: bool,
    /// (vcompose) A `must` clear its asker's list names but the composed parts don't hold:
    /// `--absent x,y,z=REASON` acknowledges it by name (a no-op then passes, printed; the same
    /// clear taking something out refuses: the cell isn't absent).
    pub absent: Option<String>,
}

/// (vcompose, G1 Nov 13 21:53) The traced compose's inputs: the STALE part's name and the
/// precedence lines (cell, part, reason).
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub stale: Option<String>,
    pub precedence: Vec<(P, String, String)>,
    /// (vcompose, G1 Nov 14 01:22) reserved cells a part holds, each acknowledged by name with the
    /// ruling that resolves it (cell, ruling, reason): the record stays not citable.
    pub reserve_clash: Vec<(P, String, String)>,
}

/// (vcompose) What the traced compose did, cell by cell.
#[derive(Clone, Debug, Default)]
pub struct Ledger {
    /// Every non-air block, block entity and entity a clear took out: `x,y,z STATE (from PART, WHY)`.
    pub cleared: Vec<String>,
    /// How many each clear took out, in the clears' order.
    pub cleared_by: Vec<usize>,
    /// The stale part's cells (and entities) an owner part holds with the same state.
    pub covered_same: usize,
    /// The stale part's cells (and entities) an owner part holds with another state, the owner's
    /// taken: `x,y,z STALE -> OWNER (PART)`.
    pub superseded: Vec<String>,
    /// The stale part's cells (and entities) no owner part holds, kept: `x,y,z STATE`.
    pub uncovered: Vec<String>,
    /// Cells two owner parts hold with the same state (kept once), per pair.
    pub same: Vec<String>,
    /// Clashes settled by a precedence line.
    pub resolved: Vec<String>,
    /// (vcompose, G1 Nov 14 01:22) the reserved cells a part holds, acknowledged by
    /// `--reserve-clash`: `x,y,z (reserved R (owner, item), held by PART: STATE; resolved by RULING: REASON)`.
    pub reserve_clash: Vec<String>,
    /// (part, cell) pairs a precedence line took from a part.
    pub lost: Vec<(String, P)>,
    /// The cells the owner parts hold, placed (the carried declarations' filter).
    pub held: std::collections::BTreeSet<P>,
}

fn c3s(c: &P) -> String {
    format!("{},{},{}", c[0], c[1], c[2])
}

/// (vcompose) The clears, every non-air block, block entity and entity each takes out printed
/// (G1 Nov 13 21:53 (b)); a `must` clear that takes out nothing refuses by name.
fn apply_clears(parts: &mut [Part], clears: &[Clear], ledger: &mut Ledger) -> Result<usize, String> {
    let mut noop: Vec<String> = Vec::new();
    let mut total = 0usize;
    for cl in clears {
        let b = &cl.b;
        let mut took = 0usize;
        for p in parts.iter_mut().filter(|p| !p.spec.relay) {
            let gone: Vec<(P, String)> = p.placed.blocks.iter().filter(|(c, _)| inside(c, b)).map(|(c, s)| (*c, s.clone())).collect();
            for (c, s) in &gone {
                p.placed.blocks.remove(c);
                if s != "minecraft:air" {
                    ledger.cleared.push(format!("{} {s} (from {}, {})", c3s(c), p.spec.name, cl.why));
                    took += 1;
                }
            }
            let gone_be: Vec<P> = p.placed.block_entities.keys().filter(|c| inside(c, b)).copied().collect();
            for c in gone_be {
                p.placed.block_entities.remove(&c);
                ledger.cleared.push(format!("{} its block entity (from {}, {})", c3s(&c), p.spec.name, cl.why));
            }
            let (gone_e, keep_e): (Vec<_>, Vec<_>) = std::mem::take(&mut p.placed.entities).into_iter().partition(|e| inside(&e.cell(), b));
            p.placed.entities = keep_e;
            for e in &gone_e {
                ledger.cleared.push(format!("{} entity {} {:?} (from {}, {})", c3s(&e.cell()), e.kind, e.pos, p.spec.name, cl.why));
                took += 1;
            }
            p.placed.ports.retain(|_, c| !inside(c, b));
            p.declared.retain(|x| !inside(&x.cell, b));
        }
        total += took;
        ledger.cleared_by.push(took);
        match (&cl.absent, took) {
            (Some(why), 0) => ledger.cleared.push(format!("{} absent, acknowledged: nothing to take out ({}; {why})", fmt_box(b), cl.why)),
            (Some(why), n) => noop.push(format!("{} ({}): declared absent ({why}) but it took out {n}", fmt_box(b), cl.why)),
            (None, 0) if cl.must => noop.push(format!("{} ({})", fmt_box(b), cl.why)),
            _ => {}
        }
    }
    if !noop.is_empty() {
        return Err(format!("a clear that takes out nothing refuses by name (ironspec Nov 13 21:55: a no-op refuses; --absent x,y,z=REASON acknowledges a listed cell the parts don't hold): {}", noop.join("; ")));
    }
    Ok(total)
}

/// (vcompose, G1 Nov 13 21:53 (a)) The stale part gives only the cells no owner part holds: its
/// blocks, block entities, entities and Nether part an owner part holds are dropped (the same
/// state counted, another listed as superseded), the rest listed as uncovered; its declared states
/// and run cells on a held cell dropped too.
fn apply_stale(parts: &mut [Part], si: usize, ledger: &mut Ledger) {
    let mut held: BTreeMap<P, (String, String)> = BTreeMap::new();
    let mut owner_ents: Vec<(String, mctas::design::EntitySpec)> = Vec::new();
    let mut owner_nether: Option<String> = None;
    for (i, p) in parts.iter().enumerate().filter(|(i, _)| *i != si) {
        for (c, s) in &p.placed.blocks {
            held.entry(*c).or_insert_with(|| (p.spec.name.clone(), s.clone()));
        }
        owner_ents.extend(p.placed.entities.iter().map(|e| (p.spec.name.clone(), e.clone())));
        if !p.placed.nether_blocks.is_empty() && owner_nether.is_none() {
            owner_nether = Some(parts[i].spec.name.clone());
        }
    }
    let sp = &mut parts[si];
    let blocks: Vec<(P, String)> = sp.placed.blocks.iter().map(|(c, s)| (*c, s.clone())).collect();
    for (c, s) in blocks {
        match held.get(&c) {
            Some((_, os)) if *os == s => {
                ledger.covered_same += 1;
                sp.placed.blocks.remove(&c);
                sp.placed.block_entities.remove(&c);
            }
            Some((who, os)) => {
                ledger.superseded.push(format!("{} {s} -> {os} ({who})", c3s(&c)));
                sp.placed.blocks.remove(&c);
                sp.placed.block_entities.remove(&c);
            }
            None => ledger.uncovered.push(format!("{} {s}", c3s(&c))),
        }
    }
    let ents = std::mem::take(&mut sp.placed.entities);
    for e in ents {
        let same_cell: Vec<&(String, mctas::design::EntitySpec)> = owner_ents.iter().filter(|(_, o)| o.cell() == e.cell() && o.kind == e.kind).collect();
        if same_cell.iter().any(|(_, o)| *o == e) {
            ledger.covered_same += 1;
        } else if let Some((who, o)) = same_cell.first() {
            ledger.superseded.push(format!("{} entity {} {:?} {} -> {:?} {} ({who})", c3s(&e.cell()), e.kind, e.pos, e.nbt.as_deref().unwrap_or(""), o.pos, o.nbt.as_deref().unwrap_or("")));
        } else {
            ledger.uncovered.push(format!("{} entity {} {:?} {}", c3s(&e.cell()), e.kind, e.pos, e.nbt.as_deref().unwrap_or("")));
            sp.placed.entities.push(e);
        }
    }
    if !sp.placed.nether_blocks.is_empty() {
        match &owner_nether {
            Some(who) => {
                ledger.superseded.push(format!("the Nether part ({} blocks) -> {who}'s", sp.placed.nether_blocks.len()));
                sp.placed.nether_blocks.clear();
                sp.placed.nether_block_entities.clear();
                sp.placed.nether_ports.clear();
                sp.placed.ports.remove("nether-anchor");
                sp.placed.ports.remove("nether-axis");
            }
            None => {
                // (G1 Nov 13 23:21, D2) one line per Nether cell, so the uncovered count counts cells
                for (c, s) in &sp.placed.nether_blocks {
                    ledger.uncovered.push(format!("nether {} {s}", c3s(c)));
                }
                for c in sp.placed.nether_block_entities.keys() {
                    ledger.uncovered.push(format!("nether {} its block entity", c3s(c)));
                }
            }
        }
    }
    sp.declared.retain(|x| !held.contains_key(&x.cell));
    let by = add(sp.spec.frame, sp.spec.tier);
    for key in ["run-gates", "run-cells"] {
        if let Some(l) = sp.committed.meta.get(key).cloned() {
            let keep: Vec<&str> = l.split(';').map(str::trim).filter(|it| !it.is_empty()).filter(|it| !cell_tokens(it).iter().any(|(_, _, c)| held.contains_key(&add(*c, by)))).collect();
            if keep.is_empty() {
                sp.committed.meta.remove(key);
            } else {
                sp.committed.meta.insert(key.to_string(), keep.join("; "));
            }
        }
    }
    ledger.held = held.keys().copied().collect();
}

/// (vcompose, G1 Nov 13 21:53 (c)) Two owner parts writing different states (blocks, block
/// entities or entities) to one cell fail the compose by name unless a precedence line names the
/// part the cell takes; the same state twice is kept once and counted; a precedence line that
/// settles no clash refuses by name.
fn apply_precedence(parts: &mut [Part], stale: Option<usize>, tr: &Trace, ledger: &mut Ledger) -> Result<(), String> {
    // (a cell's block, with its block entity, and its entities are judged apart: tag 0 the block,
    // tag 1 the entities)
    let mut by_cell: BTreeMap<(P, u8), Vec<(usize, String)>> = BTreeMap::new();
    for (i, p) in parts.iter().enumerate().filter(|(i, _)| Some(*i) != stale) {
        for (c, s) in &p.placed.blocks {
            let be = p.placed.block_entities.get(c).map(|b| format!(" {b}")).unwrap_or_default();
            by_cell.entry((*c, 0)).or_default().push((i, format!("{s}{be}")));
        }
        for e in &p.placed.entities {
            by_cell.entry((e.cell(), 1)).or_default().push((i, format!("entity {} {:?} {}", e.kind, e.pos, e.nbt.as_deref().unwrap_or(""))));
        }
    }
    let mut errs: Vec<String> = Vec::new();
    let mut used = vec![false; tr.precedence.len()];
    let mut same: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut losers: Vec<(usize, P, u8)> = Vec::new();
    let mut dup_ents: Vec<(usize, P)> = Vec::new();
    for ((c, tag), v) in &by_cell {
        let owners: std::collections::BTreeSet<usize> = v.iter().map(|(i, _)| *i).collect();
        if owners.len() < 2 {
            continue;
        }
        let first = owners.iter().next().copied().unwrap_or(0);
        let state_of = |i: usize| -> Vec<&String> {
            let mut st: Vec<&String> = v.iter().filter(|(j, _)| *j == i).map(|(_, s)| s).collect();
            st.sort();
            st
        };
        if owners.iter().all(|i| state_of(*i) == state_of(first)) {
            for i in owners.iter().skip(1) {
                *same.entry((parts[first].spec.name.clone(), parts[*i].spec.name.clone())).or_default() += 1;
                if *tag == 1 {
                    dup_ents.push((*i, *c));
                }
            }
            continue;
        }
        match tr.precedence.iter().position(|(pc, part, _)| pc == c && owners.iter().any(|i| parts[*i].spec.name == *part)) {
            Some(k) => {
                used[k] = true;
                let (_, win, why) = &tr.precedence[k];
                for i in &owners {
                    if parts[*i].spec.name != *win {
                        losers.push((*i, *c, *tag));
                    }
                }
                ledger.resolved.push(format!("{}: {win}'s state taken over {} ({why})", c3s(c), owners.iter().filter(|i| parts[**i].spec.name != *win).map(|i| format!("{}'s {}", parts[*i].spec.name, state_of(*i).iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" + "))).collect::<Vec<_>>().join(", ")));
            }
            None => errs.push(format!("{}: {}", c3s(c), owners.iter().map(|i| format!("{} = {}", parts[*i].spec.name, state_of(*i).iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" + "))).collect::<Vec<_>>().join(" vs "))),
        }
    }
    for (k, u) in used.iter().enumerate() {
        if !u {
            let (c, part, _) = &tr.precedence[k];
            errs.push(format!("--precedence {}={part}: settles no clash (a no-op refuses by name)", c3s(c)));
        }
    }
    if !errs.is_empty() {
        return Err(format!("CLASH (G1 Nov 13 21:53 (c): two parts write different states to one cell; never first- or last-writer-wins; settle each with --precedence x,y,z=PART:REASON): {}", errs.join("; ")));
    }
    for (i, c, tag) in losers {
        ledger.lost.push((parts[i].spec.name.clone(), c));
        if tag == 0 {
            parts[i].placed.blocks.remove(&c);
            parts[i].placed.block_entities.remove(&c);
        } else {
            parts[i].placed.entities.retain(|e| e.cell() != c);
        }
    }
    // (the same entities in two owner parts: kept once, the first part's)
    for (i, c) in dup_ents {
        parts[i].placed.entities.retain(|e| e.cell() != c);
    }
    ledger.same = same.into_iter().map(|((a, b), n)| format!("{a} and {b}: {n} cells")).collect();
    Ok(())
}

/// (vcompose) [`run_full`] traced: the clears printed cell by cell, the stale part's uncovered
/// cells listed, a clash between owner parts failing by name unless a precedence line settles it;
/// the ledger returned fourth.
#[allow(clippy::too_many_arguments)]
pub fn run_traced(specs: &[PartSpec], extra: Vec<Part>, clears: &[Clear], reserves: &[Reserve], ports: &[(String, P)], f: &Frame, name: &str, with_plan: bool, bed: Option<(P, &[String])>, sentinel_box: Option<(P, P)>, shift: P, summon: Option<(&[P], (P, P))>, tr: &Trace) -> Result<(String, Design, bool, Ledger), String> {
    let mut parts: Vec<Part> = specs.iter().map(load_part).collect::<Result<_, _>>()?;
    let mut ledger = Ledger::default();
    let cleared = apply_clears(&mut parts, clears, &mut ledger)?;
    parts.extend(extra);
    // (G1 Nov 13 23:21, D3) the owners' clashes settled first, so the stale part is judged against
    // each cell's WINNING state (a precedence line's), never against the first owner's
    let stale_i = match &tr.stale {
        Some(sn) => Some(parts.iter().position(|p| p.spec.name == *sn).ok_or_else(|| format!("--stale {sn}: no part of that name"))?),
        None => None,
    };
    apply_precedence(&mut parts, stale_i, tr, &mut ledger)?;
    let stale = stale_i;
    if let Some(si) = stale {
        apply_stale(&mut parts, si, &mut ledger);
    } else {
        ledger.held = parts.iter().flat_map(|p| p.placed.blocks.keys().copied()).collect();
    }
    let mut s = format!("column x {}..{} z {}..{} (frame cells), chunk lines at x = {} + 16k, z = {} + 16k\n", f.x0, f.x1, f.z0, f.z1, f.phase_x, f.phase_z);
    for (k, cl) in clears.iter().enumerate().filter(|(_, c)| c.b.0 != c.b.1 || c.why.starts_with("--clear ")) {
        s.push_str(&format!("cleared {} ({}): {} taken out\n", fmt_box(&cl.b), cl.why, ledger.cleared_by.get(k).copied().unwrap_or(0)));
    }
    s.push_str(&format!("cleared: {cleared} non-air blocks and entities taken out in all ({} lines, each below)\n", ledger.cleared.len()));
    for l in &ledger.cleared {
        s.push_str(&format!("  cleared {l}\n"));
    }
    if let Some(si) = stale {
        // (G1 Nov 13 23:21, D2) the uncovered lines counted by kind, one line per cell
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for l in &ledger.uncovered {
            let nether = l.starts_with("nether ");
            let rest = l.strip_prefix("nether ").unwrap_or(l);
            let st = rest.split_once(' ').map(|(_, s)| s).unwrap_or("");
            let k = match st.strip_prefix("entity ") {
                Some(e) => format!("{} (entity)", e.split_whitespace().next().unwrap_or("").trim_start_matches("minecraft:")),
                None => st.trim_start_matches("minecraft:").split(['[', ' ']).next().unwrap_or("").to_string(),
            };
            *kinds.entry(if nether { format!("{k} (Nether)") } else { k }).or_default() += 1;
        }
        s.push_str(&format!("stale part {} ({}): {} cells an owner part holds with the same state; {} superseded (an owner's other state taken); {} uncovered (taken from it: each needs an owner or a confirmation before the record cites; by kind: {})\n", parts[si].spec.name, parts[si].spec.file.display(), ledger.covered_same, ledger.superseded.len(), ledger.uncovered.len(), kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")));
        for l in &ledger.superseded {
            s.push_str(&format!("  superseded {l}\n"));
        }
        for l in &ledger.uncovered {
            s.push_str(&format!("  uncovered {l}\n"));
        }
    }
    for l in &ledger.resolved {
        s.push_str(&format!("precedence {l}\n"));
    }
    for p in &parts {
        s.push_str(&part_lines(p, f));
        if with_plan {
            s.push_str(&plan(p, f));
        }
    }
    for r in reserves {
        s.push_str(&format!("reserved {}: {}; {}\n", r.name, fmt_box(&r.b), match overhang(&r.b, f) {
            Some(o) => format!("LEAVES the column by {o}"),
            None => "inside the column".into(),
        }));
    }
    let cl = clashes(&parts, reserves);
    for l in &ledger.same {
        s.push_str(&format!("held twice with the same state (kept once): {l}\n"));
    }
    // (vcompose, G1 Nov 14 01:22) a part's cell inside a reserved box fails by name unless a
    // --reserve-clash line acknowledges that cell with the ruling that resolves it (the record then
    // not citable); an acknowledgement that matches no such cell refuses; two reserved boxes
    // overlapping always fail
    let mut bad: Vec<String> = cl.iter().filter(|c| c.contains("(both reserved)")).cloned().collect();
    let mut used = vec![false; tr.reserve_clash.len()];
    for r in reserves {
        for p in &parts {
            for (c, st) in p.placed.blocks.iter().filter(|(c, _)| inside(c, &r.b)) {
                let who = format!("reserved {} ({}), held by {}: {st}", r.name, match (&r.owner, &r.item) {
                    (Some(o), Some(i)) => format!("for {o}, item {i}"),
                    _ => "owner and item not named".to_string(),
                }, p.spec.name);
                match tr.reserve_clash.iter().position(|(q, _, _)| q == c) {
                    Some(k) => {
                        used[k] = true;
                        let (_, ruling, why) = &tr.reserve_clash[k];
                        ledger.reserve_clash.push(format!("{} ({who}; resolved by {ruling}: {why})", c3s(c)));
                    }
                    None => bad.push(format!("{} ({who})", c3s(c))),
                }
            }
        }
    }
    for (k, u) in used.iter().enumerate() {
        if !u {
            let (c, ruling, _) = &tr.reserve_clash[k];
            bad.push(format!("--reserve-clash {}={ruling}: no part holds a reserved cell there (a no-op refuses by name)", c3s(c)));
        }
    }
    if !bad.is_empty() {
        return Err(format!("CLASH (a part's cell inside a reserved box, two reserved boxes overlapping, or an unused --reserve-clash; G1 Nov 13 23:21 / Nov 14 01:22): {}", bad.join("; ")));
    }
    for l in &ledger.reserve_clash {
        s.push_str(&format!("reserve-clash acknowledged (the record is NOT CITABLE until its ruling composes in): {l}\n"));
    }
    s.push_str(&format!("clashes: none (no cell two parts write differently; no reserved box over another's{})\n", if ledger.reserve_clash.is_empty() { String::new() } else { format!("; {} reserved cells held, each acknowledged above", ledger.reserve_clash.len()) }));
    s.push_str(&tiers(&parts, reserves));
    let mut bed_ok = true;
    if let Some((b, exempt)) = bed {
        let (t, ok) = bed_radius(&parts, reserves, b, exempt);
        s.push_str(&t);
        bed_ok = ok;
    }
    if let Some(sb) = sentinel_box {
        let (t, ok) = sentinel(&parts, sb, bed.map(|(b, _)| b), shift);
        s.push_str(&t);
        bed_ok &= ok;
    }
    if let Some((sc, g)) = summon {
        s.push_str(&summon_box(&parts, reserves, sc, g));
    }
    let (mut d, two_ways, dropped) = skeleton(&parts, name)?;
    if !dropped.is_empty() {
        s.push_str(&format!("ports dropped (the first part keeps its ports; a later part's only its shared bed's home and a Nether part's anchor): {}\n", dropped.join("; ")));
    }
    for (k, c) in ports {
        s.push_str(&format!("port {k}: {},{},{} (given){}\n", c[0], c[1], c[2], if d.ports.contains_key(k) { ", replacing the part's" } else { "" }));
        d.ports.insert(k.clone(), *c);
    }
    if two_ways.is_empty() {
        s.push_str(&format!("declared states: {} cells, none declared two ways\n", d.meta.get("declared").map(|l| l.split(';').count()).unwrap_or(0)));
    } else {
        for t in &two_ways {
            s.push_str(&format!("DECLARED TWO WAYS {t}\n"));
        }
    }
    // (a slope stays only on a sturdy block below and a sturdy block on its high side,
    // `BaseRailBlock.shouldBeRemoved`: a kept box or a clear that drops either pops the rail when
    // placed; onechunk's runner, Nov 11: v4's snap-turn slope (-3,2,0) had lost its (-4,2,0))
    let unsupported = slope_supports(&d);
    if unsupported.is_empty() {
        s.push_str("slopes: every ascending rail has a sturdy block below and on its high side\n");
    } else {
        s.push_str(&format!("SLOPES THAT POP (no sturdy block below or on the high side): {}\n", unsupported.join("; ")));
    }
    // the column's box: the parts' blocks and the reserved boxes
    let mut b = span(&d);
    for r in reserves {
        for k in 0..3 {
            b.0[k] = b.0[k].min(r.b.0[k]);
            b.1[k] = b.1[k].max(r.b.1[k]);
        }
    }
    let h = size(&b)[1] as i64;
    s.push_str(&format!("skeleton: {} (the parts' blocks and the reserved boxes); {} blocks placed; the column's own box {} x {} x {} = {} blocks³; {}\n", fmt_box(&b), d.blocks.len(), f.x1 - f.x0 + 1, h, f.z1 - f.z0 + 1, (f.x1 - f.x0 + 1) as i64 * h * (f.z1 - f.z0 + 1) as i64, match overhang(&b, f) {
        Some(o) => format!("the skeleton LEAVES the column by {o}"),
        None => "everything inside the column".into(),
    }));
    Ok((s, d, bed_ok, ledger))
}

/// A sketch track's cells through its waypoints: each leg along x or z. A rising leg climbs in its
/// last cells and a falling leg drops in its first, so the cell after a corner (rising) or before
/// one (falling) sits at the corner's height (a curve has no slope); a leg changing height more
/// than its length is refused.
pub fn track_cells(points: &[P]) -> Result<Vec<P>, String> {
    let first = *points.first().ok_or("a track needs a waypoint")?;
    let mut cells = vec![first];
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        if dx != 0 && dz != 0 {
            return Err(format!("track leg ({},{},{}) -> ({},{},{}): a leg runs along x or z", a[0], a[1], a[2], b[0], b[1], b[2]));
        }
        let n = dx.abs() + dz.abs();
        if dy.abs() > n || n == 0 {
            return Err(format!("track leg ({},{},{}) -> ({},{},{}): {} cells can't change height by {}", a[0], a[1], a[2], b[0], b[1], b[2], n, dy));
        }
        for k in 1..=n {
            let step = if dy > 0 { (k - (n - dy)).max(0) } else { k.min(-dy) };
            cells.push([a[0] + dx.signum() * k, a[1] + dy.signum() * step, a[2] + dz.signum() * k]);
        }
    }
    Ok(cells)
}

/// The horizontal direction from a cell to its neighbour.
fn dir_to(from: P, to: P) -> Result<&'static str, String> {
    match (to[0] - from[0], to[2] - from[2]) {
        (1, 0) => Ok("east"),
        (-1, 0) => Ok("west"),
        (0, 1) => Ok("south"),
        (0, -1) => Ok("north"),
        _ => Err(format!("({},{},{}) and ({},{},{}) are not neighbours", from[0], from[1], from[2], to[0], to[1], to[2])),
    }
}

/// A plain rail's shape at `c` between its track neighbours: a slope rising toward a neighbour one
/// higher, else straight or the curve joining the two directions.
fn rail_shape(prev: Option<P>, c: P, next: Option<P>) -> Result<String, String> {
    let ns: Vec<P> = [prev, next].into_iter().flatten().collect();
    let up: Vec<&P> = ns.iter().filter(|n| n[1] == c[1] + 1).collect();
    if up.len() > 1 {
        return Err(format!("({},{},{}): a rail can't rise toward both neighbours", c[0], c[1], c[2]));
    }
    if let Some(u) = up.first() {
        // (a slope runs straight: its other neighbour lies opposite)
        let d = dir_to(c, **u)?;
        if let Some(o) = ns.iter().find(|n| *n != *u) {
            let od = dir_to(c, *o)?;
            let opposite = matches!((d, od), ("east", "west") | ("west", "east") | ("north", "south") | ("south", "north"));
            if !opposite {
                return Err(format!("({},{},{}): a slope can't turn (a curve has no slope)", c[0], c[1], c[2]));
            }
        }
        return Ok(format!("ascending_{d}"));
    }
    let ds: Vec<&str> = ns.iter().map(|n| dir_to(c, *n)).collect::<Result<_, _>>()?;
    let axis = |d: &str| if d == "east" || d == "west" { "east_west" } else { "north_south" };
    Ok(match ds.as_slice() {
        [] => "north_south".to_string(),
        [d] => axis(d).to_string(),
        [a, b] if axis(a) == axis(b) => axis(a).to_string(),
        [a, b] => {
            let (ns_d, ew_d) = if axis(a) == "north_south" { (*a, *b) } else { (*b, *a) };
            format!("{ns_d}_{ew_d}")
        }
        _ => unreachable!(),
    })
}

/// The sketch part (`--track`, `--cart`, `--post`): plain rails through each track's waypoints with
/// a stone under each, carts (ridden by a villager unless `empty`) on rail cells, and the brief's
/// zombie posts (a persistent zombie in an oak boat at the cell, glass under it, leaves 3 above).
/// A track's `[first, last]` flags mark its end waypoints as context cells (another part's rail,
/// a joint): they shape the neighbouring rail and are not laid. A sketch: no powered rail, no
/// control, no spacing; the cells stand in for a lane's until it sends its own.
pub fn sketch_part(tracks: &[(String, Vec<P>, [bool; 2])], carts: &[(P, Option<f64>, bool)], posts: &[P]) -> Result<Vec<Part>, String> {
    let mut d = Design { name: "sketch".into(), ..Default::default() };
    let mut rails: BTreeMap<P, String> = BTreeMap::new();
    // (each laid cell's track, so each track is a part of its own in the report)
    let mut owner: BTreeMap<P, usize> = BTreeMap::new();
    for (ti, (name, pts, context)) in tracks.iter().enumerate() {
        let cells = track_cells(pts).map_err(|e| format!("track {name}: {e}"))?;
        for (i, c) in cells.iter().enumerate() {
            if (i == 0 && context[0]) || (i + 1 == cells.len() && context[1]) {
                continue;
            }
            let shape = rail_shape(i.checked_sub(1).map(|j| cells[j]), *c, cells.get(i + 1).copied()).map_err(|e| format!("track {name}: {e}"))?;
            if rails.insert(*c, shape).is_some() {
                return Err(format!("track {name}: the cell ({},{},{}) is laid twice", c[0], c[1], c[2]));
            }
            owner.insert(*c, ti);
        }
    }
    let mut track_designs: Vec<Design> = tracks.iter().map(|(name, _, _)| Design { name: format!("track:{}", name.trim_start_matches('+')), ..Default::default() }).collect();
    // (a track named `+NAME` is boosted: every straight cell a powered rail on its own redstone
    // block, the curves plain on glass; onechunk's runner, Nov 11: the climb back's boosters as a
    // labelled sketch until dfb89499's climb cells come)
    let boosted: Vec<bool> = tracks.iter().map(|(name, _, _)| name.starts_with('+')).collect();
    let straight = |s: &str| s.starts_with("ascending") || s == "north_south" || s == "east_west";
    for (c, shape) in &rails {
        let ti = owner[c];
        if boosted[ti] && straight(shape) {
            track_designs[ti].blocks.insert(*c, format!("minecraft:powered_rail[powered=true,shape={shape},waterlogged=false]"));
            track_designs[ti].blocks.insert([c[0], c[1] - 1, c[2]], "minecraft:redstone_block".into());
        } else {
            track_designs[ti].blocks.insert(*c, format!("minecraft:rail[shape={shape},waterlogged=false]"));
        }
    }
    for c in rails.keys() {
        let under = [c[0], c[1] - 1, c[2]];
        if rails.contains_key(&under) {
            return Err(format!("the rail at ({},{},{}) sits on another rail", c[0], c[1], c[2]));
        }
        // (glass: a rail's sturdy support that no golem stands on, `SpawnUtil`'s iron-golem
        // strategy refusing glass below, so a sketch track adds no golem floor to any band;
        // onechunk's v4 flood, Nov 11: a stone under the descent's first slope inside the hold's
        // band was a stray spot)
        track_designs[owner[c]].blocks.entry(under).or_insert_with(|| "minecraft:glass".into());
    }
    for (c, at_y, empty) in carts {
        // (on a sketch rail its height follows the shape; on another part's rail the cart's y is
        // given, else it sits as on a flat rail)
        let y = match (at_y, rails.get(c)) {
            (Some(y), _) => *y,
            (None, Some(shape)) if shape.starts_with("ascending") => c[1] as f64 + 0.5,
            (None, _) => c[1] as f64 + 0.0625,
        };
        let nbt = if *empty { None } else { Some("{Passengers:[{id:\"minecraft:villager\"}]}".to_string()) };
        d.entities.push(mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [c[0] as f64 + 0.5, y, c[2] as f64 + 0.5], nbt });
    }
    for p in posts {
        for (cell, block) in [([p[0], p[1] - 1, p[2]], "minecraft:glass"), ([p[0], p[1] + 3, p[2]], "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]")] {
            if d.blocks.insert(cell, block.into()).is_some() {
                return Err(format!("--post ({},{},{}): its {} cell ({},{},{}) is taken", p[0], p[1], p[2], block, cell[0], cell[1], cell[2]));
            }
        }
        d.entities.push(mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], nbt: Some("{Passengers:[{id:\"minecraft:zombie\",PersistenceRequired:1b}]}".into()) });
    }
    let part = |d: Design, file: &str| {
        let spec = PartSpec { name: d.name.clone(), file: PathBuf::from(file), base: None, frame: [0; 3], tier: [0; 3], keep: None, relay: true, nether: false };
        Part { spec, committed: d.clone(), placed: d, positions: Vec::new(), declared: Vec::new(), base_changed: Vec::new() }
    };
    let mut out: Vec<Part> = track_designs.into_iter().filter(|t| !t.blocks.is_empty()).map(|t| part(t, "(sketch track)")).collect();
    if !d.blocks.is_empty() || !d.entities.is_empty() {
        out.push(part(d, "(sketch carts, posts)"));
    }
    Ok(out)
}

/// A rail block's shape, from its state string (`minecraft:rail[shape=north_east,...]`, powered,
/// detector and activator rails alike); None for anything else.
pub fn rail_shape_of(state: &str) -> Option<&str> {
    let base = state.strip_prefix("minecraft:").unwrap_or(state);
    if !(base.starts_with("rail[") || base.starts_with("powered_rail[") || base.starts_with("detector_rail[") || base.starts_with("activator_rail[")) {
        return None;
    }
    let i = base.find("shape=")? + 6;
    let rest = &base[i..];
    Some(&rest[..rest.find([',', ']']).unwrap_or(rest.len())])
}

/// Every ascending rail missing its sturdy block below or on its high side (the neighbour it rises
/// toward): `BaseRailBlock.shouldBeRemoved` pops it when placed. Sturdy here: a full block by name
/// (anything but air, rails, gates, fences, walls, panes, slabs, stairs, carpets, plates, buttons,
/// torches, dust, repeaters, comparators, tripwire and hooks, ladders, signs, trapdoors, doors,
/// fluids).
pub fn slope_supports(d: &Design) -> Vec<String> {
    let sturdy = |c: P| -> bool {
        let Some(s) = d.blocks.get(&c) else { return false };
        let b = s.strip_prefix("minecraft:").unwrap_or(s);
        let name = &b[..b.find('[').unwrap_or(b.len())];
        let not = ["air", "rail", "gate", "fence", "wall", "pane", "slab", "stairs", "carpet", "plate", "button", "torch", "redstone_wire", "repeater", "comparator", "tripwire", "ladder", "sign", "trapdoor", "door", "water", "lava", "lever", "scaffolding", "snow", "chain", "bars", "candle", "head", "skull", "pot", "rod"];
        !not.iter().any(|n| name.contains(n))
    };
    let mut out = Vec::new();
    for (c, st) in &d.blocks {
        let Some(shape) = rail_shape_of(st) else { continue };
        let Some(dir) = shape.strip_prefix("ascending_") else { continue };
        let high = match dir {
            "east" => [c[0] + 1, c[1], c[2]],
            "west" => [c[0] - 1, c[1], c[2]],
            "south" => [c[0], c[1], c[2] + 1],
            "north" => [c[0], c[1], c[2] - 1],
            _ => continue,
        };
        let below = [c[0], c[1] - 1, c[2]];
        let mut why = Vec::new();
        if !sturdy(below) {
            why.push(format!("below ({},{},{}) {}", below[0], below[1], below[2], d.blocks.get(&below).map(String::as_str).unwrap_or("air")));
        }
        if !sturdy(high) {
            why.push(format!("high side ({},{},{}) {}", high[0], high[1], high[2], d.blocks.get(&high).map(String::as_str).unwrap_or("air")));
        }
        if !why.is_empty() {
            out.push(format!("({},{},{}) {shape}: {}", c[0], c[1], c[2], why.join(", ")));
        }
    }
    out
}

/// The cart's path through a rail cell (its feet: the villager sits at the cart's own y): two
/// segments, from the cell's centre to each end the shape joins, a slope's high end one up.
fn rail_segments(c: P, shape: &str) -> Vec<([f64; 3], [f64; 3])> {
    let (x, y, z) = (c[0] as f64, c[1] as f64 + 0.0625, c[2] as f64);
    let end = |d: &str, rise: f64| -> [f64; 3] {
        match d {
            "north" => [x + 0.5, y + rise, z],
            "south" => [x + 0.5, y + rise, z + 1.0],
            "west" => [x, y + rise, z + 0.5],
            _ => [x + 1.0, y + rise, z + 0.5],
        }
    };
    let (a, b, slope) = match shape {
        "north_south" => (end("north", 0.0), end("south", 0.0), false),
        "east_west" => (end("west", 0.0), end("east", 0.0), false),
        s if s.starts_with("ascending_") => {
            let up = &s[10..];
            let down = match up {
                "north" => "south",
                "south" => "north",
                "east" => "west",
                _ => "east",
            };
            (end(up, 1.0), end(down, 0.0), true)
        }
        s => {
            let (p, q) = s.split_once('_').unwrap_or(("north", "south"));
            (end(p, 0.0), end(q, 0.0), false)
        }
    };
    let centre = [x + 0.5, y + if slope { 0.5 } else { 0.0 }, z + 0.5];
    vec![(centre, a), (centre, b)]
}

fn seg_dist(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let q = [a[0] + t * ab[0] - p[0], a[1] + t * ab[1] - p[1], a[2] + t * ab[2] - p[2]];
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt()
}

/// (onechunk Nov 12) A budget's reserved boxes as `--reserve` specs: every `# reserved (no
/// blocks): NAME x a..b y c..d z e..f` header line (`column`'s own output), but the names in `skip`.
pub fn reserves_from_header(text: &str, skip: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("# reserved (no blocks): ") else { continue };
        let f: Vec<&str> = rest.split_whitespace().collect();
        if f.len() < 7 || f[1] != "x" || f[3] != "y" || f[5] != "z" || skip.iter().any(|s| s == f[0]) {
            continue;
        }
        let span = |t: &str| t.split_once("..").map(|(a, b)| (a.to_string(), b.to_string()));
        let (Some((x0, x1)), Some((y0, y1)), Some((z0, z1))) = (span(f[2]), span(f[4]), span(f[6])) else { continue };
        out.push(format!("{}={x0},{y0},{z0},{x1},{y1},{z1}", f[0]));
    }
    out
}

/// (onechunk Nov 12) A part's every golem spawn spot, as `mobfarm exit`'s part flood finds them:
/// its villagers' 17 x 17 columns (`exit::spawn_spots_of`) and each extra summoner position's
/// (its cell and the one above, `trySpawnMob`'s scan from y + 6 down, `checkSpawnObstruction`),
/// each with the block under it, grouped by that block.
pub fn part_spots(spec: &redsim::entity::mob::farm::FarmSpec, extra: &[redsim::pos::BlockPos], seed: i64) -> Result<String, String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    use redsim::entity::mob::spawn::{check_spawn_obstruction, possible_spawn_position};
    use redsim::pos::BlockPos;
    let o = RunOpts { ai: false, ..RunOpts::steady(seed, 0, WalkMode::Random) };
    let r = Run::new_homeless(spec, &o)?;
    let w = &r.world;
    let mut out: Vec<BlockPos> = redsim::entity::mob::exit::spawn_spots_of(w, &r.own);
    for p in extra {
        for s in [*p, p.above()] {
            for dz in -8..=8 {
                for dx in -8..=8 {
                    let top = BlockPos::new(s.x + dx, s.y + 6, s.z + dz);
                    if let Some(cell) = possible_spawn_position(w, top, 6)
                        && !out.contains(&cell)
                        && check_spawn_obstruction(w, cell, redsim::entity::Vec3::new(cell.x as f64 + 0.5, cell.y as f64, cell.z as f64 + 0.5))
                    {
                        out.push(cell);
                    }
                }
            }
        }
    }
    let mut by: BTreeMap<String, Vec<BlockPos>> = BTreeMap::new();
    for c in &out {
        let under = format!("{:?}", w.get(c.below()));
        by.entry(under).or_default().push(*c);
    }
    let mut s = format!("spawn spots: {} (a golem's feet cell; grouped by the block under it)\n", out.len());
    for (b, cells) in &by {
        s.push_str(&format!("  {} on {b}:", cells.len()));
        for c in cells {
            s.push_str(&format!(" ({},{},{})", c.x, c.y, c.z));
        }
        s.push('\n');
    }
    Ok(s)
}

/// (G1 on ddebc9a65: the rail-T reach set) Every plain rail with 3 potential connections in the
/// finished build (`junction_at`: a T), its stored shape and the shapes its signal settles it to:
/// each must be a placement-order case or power-forced.
pub fn t_rails(spec: &redsim::entity::mob::farm::FarmSpec) -> Result<Vec<String>, String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    // (a rail-only run: the entities left out, so a scarer the loader doesn't model can't stop it)
    let mut sp = spec.clone();
    sp.entities.clear();
    let spec = &sp;
    let w = if spec.has_bed() { Run::new(spec, &o)?.world } else { Run::new_homeless(spec, &o)?.world };
    // (G1 on a9a418d03: the reach scan) a T re-settles only on a neighbour update from a signal
    // source block; a source's update reaches its own neighbours and those of the block it powers
    // or hangs on (a detector rail's block below, a lever's, a hook's or a button's support (below
    // a floor one, above a ceiling one, else behind its facing), a repeater's, comparator's or
    // observer's front, a wire's or a torch's neighbours): each source's reach set, scanned for Ts
    let source = |s: &str| is_source_name(s);
    let sources: Vec<(redsim::pos::BlockPos, String)> = spec.blocks.iter().filter(|(_, b)| source(&b.to_string())).map(|(p, b)| (*p, b.to_string())).collect();
    // (each source's own update reach, 26.3: the cells whose neighborChanged its state change
    // calls; a redstone block never changes, so it reaches nothing (an emulated switched cell is air
    // in the files and isn't scanned))
    let n6 = |c: [i32; 3]| -> Vec<[i32; 3]> { vec![[c[0] + 1, c[1], c[2]], [c[0] - 1, c[1], c[2]], [c[0], c[1] + 1, c[2]], [c[0], c[1] - 1, c[2]], [c[0], c[1], c[2] + 1], [c[0], c[1], c[2] - 1]] };
    let facing = |s: &str| -> Option<[i32; 3]> {
        let f = s.split("facing=").nth(1)?.split([',', ']']).next()?;
        Some(match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            "down" => [0, -1, 0],
            _ => return None,
        })
    };
    let support = |s: &str, c: [i32; 3]| -> Option<[i32; 3]> {
        if s.contains("face=floor") {
            Some([c[0], c[1] - 1, c[2]])
        } else if s.contains("face=ceiling") {
            Some([c[0], c[1] + 1, c[2]])
        } else {
            facing(s).map(|f| [c[0] - f[0], c[1] - f[1], c[2] - f[2]])
        }
    };
    let reach_of = |q: redsim::pos::BlockPos, s: &str| -> Vec<[i32; 3]> {
        let n = s.trim_start_matches("minecraft:");
        let n = n.split('[').next().unwrap_or(n);
        let c = [q.x, q.y, q.z];
        let with = |cells: Vec<[i32; 3]>| -> Vec<[i32; 3]> { cells.iter().flat_map(|x| n6(*x)).chain(cells.iter().copied()).collect() };
        match n {
            "redstone_block" => Vec::new(),
            // (its own neighbours and the block below's: `updatePowerToConnected`; an ascending one
            // also the cell above's (G1 on f1507f5dc: the ruled reach set, push-scan's above-slope
            // rule: split9's branch detectors ascend south beside J))
            "detector_rail" if s.contains("ascending") => with(vec![c, [c[0], c[1] - 1, c[2]], [c[0], c[1] + 1, c[2]]]),
            "detector_rail" => with(vec![c, [c[0], c[1] - 1, c[2]]]),
            "redstone_wire" | "redstone_torch" | "redstone_wall_torch" => with(n6(c).into_iter().chain([c]).collect()),
            // (the block in front and its neighbours; a repeater's or comparator's front is opposite
            // its facing, an observer's output its back)
            "repeater" | "comparator" => facing(s).map(|f| with(vec![[c[0] - f[0], c[1] - f[1], c[2] - f[2]]])).unwrap_or_else(|| with(n6(c))),
            "observer" => facing(s).map(|f| with(vec![[c[0] - f[0], c[1] - f[1], c[2] - f[2]]])).unwrap_or_else(|| with(n6(c))),
            // (its own neighbours and its support's (G1's note 2 on 9653a6971): below a floor lever
            // or button, above a ceiling one, else the block behind its facing; a hook hangs on a
            // wall)
            "lever" | "tripwire_hook" => support(s, c).map(|q| with(vec![c, q])).unwrap_or_else(|| with(n6(c))),
            _ if n.ends_with("_button") => support(s, c).map(|q| with(vec![c, q])).unwrap_or_else(|| with(n6(c))),
            _ if n.ends_with("_pressure_plate") => with(vec![c, [c[0], c[1] - 1, c[2]]]),
            _ => with(vec![c]),
        }
    };
    let mut out = Vec::new();
    for (p, b) in &spec.blocks {
        if !matches!(b, redsim::block::Block::PlainRail { .. }) {
            continue;
        }
        if let Some(j) = redsim::rails::junction_at(&w, *p) {
            let pc = [p.x, p.y, p.z];
            let hits: Vec<String> = sources.iter().filter(|(q, s)| reach_of(*q, s).contains(&pc)).map(|(q, s)| format!("the {} at ({},{},{})", s.trim_start_matches("minecraft:").split('[').next().unwrap_or(""), q.x, q.y, q.z)).collect();
            let near = sources.iter().filter(|(_, s)| !s.contains("redstone_block")).map(|(q, s)| ((q.x - p.x).abs() + (q.y - p.y).abs() + (q.z - p.z).abs(), *q, s)).min_by_key(|x| x.0);
            let reach = if !hits.is_empty() {
                format!("REACHED by {} (its update reaches this cell)", hits.join(", "))
            } else {
                match near {
                    Some((d, q, s)) => format!("no source's update reaches it (the nearest changing source, the {} at ({},{},{}), {d} away taxicab, updates its own neighbours and its support's; redstone blocks never change)", s.trim_start_matches("minecraft:").split('[').next().unwrap_or(""), q.x, q.y, q.z),
                    None => "no changing signal source in the file".to_string(),
                }
            };
            out.push(format!(
                "({},{},{}) stored {}, unpowered {}, powered {}, its signal now {}; {reach}",
                p.x,
                p.y,
                p.z,
                redsim::rails::shape_of(b).name(),
                j.unpowered.name(),
                j.powered.name(),
                if j.signal { "on" } else { "off" }
            ));
        }
    }
    Ok(out)
}

/// (ironspec 03:37, the placement-order rule) Every rail whose designed shape, or a neighbouring
/// rail's, differs from what vanilla placement gives when it is placed last into the finished
/// build (`onPlace`: the rail connects to its neighbours): the rails that need an order or a
/// temporary helper (the file is pasted with no settle).
pub fn placement_order(spec: &redsim::entity::mob::farm::FarmSpec) -> Result<Vec<String>, String> {
    use redsim::block::Block;
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    use redsim::pos::BlockPos;
    use redsim::world::flags;
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    // (a rail-only run: the entities left out, so a scarer the loader doesn't model can't stop it)
    let mut sp = spec.clone();
    sp.entities.clear();
    let spec = &sp;
    let w0 = if spec.has_bed() { Run::new(spec, &o)?.world } else { Run::new_homeless(spec, &o)?.world };
    let by: std::collections::HashMap<(i32, i32, i32), &Block> = spec.blocks.iter().map(|(p, b)| ((p.x, p.y, p.z), b)).collect();
    let mut out = Vec::new();
    for (p, b) in &spec.blocks {
        if !redsim::rails::is_rail(b) {
            continue;
        }
        let mut w = w0.clone();
        w.set_block(*p, Block::Air, flags::RAW);
        w.set_block(*p, b.clone(), flags::ALL);
        let mut diffs = Vec::new();
        for q in [*p, BlockPos::new(p.x + 1, p.y, p.z), BlockPos::new(p.x - 1, p.y, p.z), BlockPos::new(p.x, p.y, p.z + 1), BlockPos::new(p.x, p.y, p.z - 1), BlockPos::new(p.x, p.y + 1, p.z), BlockPos::new(p.x, p.y - 1, p.z)] {
            let Some(want) = by.get(&(q.x, q.y, q.z)).copied().filter(|x| redsim::rails::is_rail(x)) else { continue };
            let got = w.get(q);
            if !redsim::rails::is_rail(&got) {
                diffs.push(format!("({},{},{}) {} gone", q.x, q.y, q.z, redsim::rails::shape_of(want).name()));
            } else if redsim::rails::shape_of(&got) != redsim::rails::shape_of(want) {
                diffs.push(format!("({},{},{}) {} -> {}", q.x, q.y, q.z, redsim::rails::shape_of(want).name(), redsim::rails::shape_of(&got).name()));
            }
        }
        if !diffs.is_empty() {
            out.push(format!("({},{},{}) placed last: {}", p.x, p.y, p.z, diffs.join(", ")));
        }
    }
    Ok(out)
}

/// (ironspec 03:17 / 03:23 / 03:39) The creeper check: every position a creeper can take (a
/// hostile flood of the creeper's body from every cell the game can spawn one at, block light 0,
/// and every position walkable from there; `hostile::flood_with`, `SpawnRules`) within 5.5 of a
/// target block (every portal block; with `glass`, every glass block too: the loader's overworld
/// side): 0 required; those within 10.5 printed for charged creepers. The distance runs from the
/// creeper's feet point (the explosion's centre) to the block's nearest point. (report, passes)
/// (ironspec 08:10 and 08:21) The assembly check: `composed` (the design re-composed from its
/// declared parts and overrides) against `committed` (the file as committed): every cell and
/// entity must match. A cell or entity in the committed file that no part declares (a leftover:
/// a superseded part's cell that stayed, a stale mob) is printed, and so is a declared one the file
/// lacks or holds differently. Then the census of the file's entities (each passenger counted).
pub fn assembly_check(composed: &Design, committed: &Design, keep: &[(String, [i32; 3], [i32; 3])]) -> (String, bool) {
    let mut s = String::new();
    // (ironspec 09:31) the keep-clear boxes: no block of the committed file inside (a name ending
    // in "+glass": glass or air only, the daylight detector's roof skylight, ironspec 11:13)
    let mut kept_ok = true;
    for (name, lo, hi) in keep {
        let allow_glass = name.ends_with("+glass");
        let inside: Vec<String> = committed
            .blocks
            .iter()
            .filter(|(p, _)| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]))
            .filter(|(_, b)| !(allow_glass && b.trim_start_matches("minecraft:").split('[').next() == Some("glass")))
            .map(|(p, b)| format!("({},{},{}) {b}", p[0], p[1], p[2]))
            .collect();
        kept_ok &= inside.is_empty();
        s.push_str(&format!(
            "keep-clear {name} x {}..{} y {}..{} z {}..{}: {}\n",
            lo[0],
            hi[0],
            lo[1],
            hi[1],
            lo[2],
            hi[2],
            if inside.is_empty() { "clear (PASS)".to_string() } else { format!("{} blocks inside (FAIL): {}", inside.len(), inside.join("; ")) }
        ));
    }
    let mut leftover: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut differ: Vec<String> = Vec::new();
    for (p, b) in &committed.blocks {
        match composed.blocks.get(p) {
            None => leftover.push(format!("({},{},{}) {b}", p[0], p[1], p[2])),
            Some(c) if c != b => differ.push(format!("({},{},{}) {b} in the file, {c} declared", p[0], p[1], p[2])),
            Some(_) => {}
        }
    }
    for (p, c) in &composed.blocks {
        if !committed.blocks.contains_key(p) {
            missing.push(format!("({},{},{}) {c}", p[0], p[1], p[2]));
        }
    }
    let mut ent_left: Vec<String> = Vec::new();
    let mut want: Vec<&mctas::design::EntitySpec> = composed.entities.iter().collect();
    for e in &committed.entities {
        if let Some(i) = want.iter().position(|w| *w == e) {
            want.remove(i);
        } else {
            ent_left.push(format!("{} at ({},{},{}) {}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.nbt.as_deref().unwrap_or("")));
        }
    }
    let ent_missing: Vec<String> = want.iter().map(|e| format!("{} at ({},{},{}) {}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.nbt.as_deref().unwrap_or(""))).collect();
    let list = |v: &[String]| v.iter().take(40).cloned().collect::<Vec<_>>().join("; ");
    s.push_str(&format!(
        "{} cells in the file, {} declared by the parts and overrides: {} leftover (in the file, no part declares it){}, {} missing{}, {} different{}\n",
        committed.blocks.len(),
        composed.blocks.len(),
        leftover.len(),
        if leftover.is_empty() { String::new() } else { format!(": {}", list(&leftover)) },
        missing.len(),
        if missing.is_empty() { String::new() } else { format!(": {}", list(&missing)) },
        differ.len(),
        if differ.is_empty() { String::new() } else { format!(": {}", list(&differ)) }
    ));
    s.push_str(&format!(
        "{} entities in the file, {} declared: {} leftover{}, {} missing{}\n",
        committed.entities.len(),
        composed.entities.len(),
        ent_left.len(),
        if ent_left.is_empty() { String::new() } else { format!(": {}", list(&ent_left)) },
        ent_missing.len(),
        if ent_missing.is_empty() { String::new() } else { format!(": {}", list(&ent_missing)) }
    ));
    // the census: each entity by kind, its passengers too
    let mut census: BTreeMap<String, usize> = BTreeMap::new();
    for e in &committed.entities {
        *census.entry(e.kind.trim_start_matches("minecraft:").to_string()).or_insert(0) += 1;
        if let Some(n) = &e.nbt {
            for part in n.split("id:\"minecraft:").skip(1) {
                let k: String = part.chars().take_while(|c| *c != '"').collect();
                *census.entry(k).or_insert(0) += 1;
            }
        }
    }
    s.push_str(&format!("census (the file's entities, each passenger counted): {}\n", census.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")));
    // (G1 Nov 13 23:21, D4) the Nether part's mismatches reach the verdict too
    let nl = committed.nether_blocks.iter().filter(|(p, b)| composed.nether_blocks.get(*p) != Some(*b)).count();
    let nm = composed.nether_blocks.iter().filter(|(p, _)| !committed.nether_blocks.contains_key(*p)).count();
    if !committed.nether_blocks.is_empty() || !composed.nether_blocks.is_empty() {
        s.push_str(&format!("Nether part: {} cells in the file, {} declared: {nl} leftover or different, {nm} missing\n", committed.nether_blocks.len(), composed.nether_blocks.len()));
    }
    let ok = kept_ok && leftover.is_empty() && missing.is_empty() && differ.is_empty() && ent_left.is_empty() && ent_missing.is_empty() && nl == 0 && nm == 0;
    s.push_str(&format!("assembly: {}\n", if ok { "every cell and entity declared by a part (PASS)" } else { "FAIL" }));
    (s, ok)
}

/// (ironspec 10:20 / 10:24) The 6.05 flee bar, sight-gated with robust hiding, on the composed
/// design: every rider position every 0.1 along every rail of the composition (the cart's path:
/// along the rail's axis, a slope at its height, a curve along its chord) and along the two
/// bubble-column lifts (the shared lift x -5 z 7 and napper A's lift x 6 z 4), the rider's eye at
/// the cart's position + 1.62, against every zombie seated in a boat (its feet at the boat's y +
/// 0.1875 - 0.7, its eye 1.74 over them). A position within 6.05 of a zombie (feet to feet, the
/// flee's own test) is IN SIGHT (a failure) unless a full-collider block (`isCollisionShapeFullBlock`)
/// sits on the eye-to-eye ray (sampled every 0.025, the two end cells left out): then HIDDEN, listed
/// with the blocking cell. Beyond 6.05: clear.
pub fn flee_sight(d: &Design, declared: &[[i32; 3]], extra: &[(String, f64, f64, f64, f64)], riderless: &[[i32; 3]], unreachable: &[[i32; 3]]) -> (String, bool) {
    flee_sight_from(d, declared, extra, riderless, unreachable, &[], None)
}

/// A reach provider for [`flee_sight_from`]: given the stop's on-foot positions (cells with their
/// feet heights) and the stand-up cells, for each position the villager path that reaches it (its
/// nodes, the evidence), or None.
pub type ReachFn<'a> = &'a mut dyn FnMut(&[([i32; 3], f64)], &[[i32; 3]]) -> Result<Vec<Option<String>>, String>;

/// (vcompose, ironspec Nov 13 22:52: the flee static prints unreachable cells separately) Which of
/// the stop's on-foot positions (cells with their feet heights) a villager walks to from its
/// stand-up cells `starts` (the `# a6-pickup:` cells and the declared waker places): a step goes to
/// a horizontally adjacent position whose feet rise at most 1.25 (a villager's jump: 58f770cc 22:42)
/// or drop at most 3; None when no start cell is a position (not judged).
pub fn walk_reach(stop: &[([i32; 3], f64)], starts: &[[i32; 3]]) -> Option<Vec<bool>> {
    let mut seen: Vec<bool> = stop.iter().map(|(c, _)| starts.contains(c)).collect();
    if !seen.iter().any(|s| *s) {
        return None;
    }
    let mut todo: Vec<usize> = (0..stop.len()).filter(|i| seen[*i]).collect();
    while let Some(i) = todo.pop() {
        let (ci, fi) = stop[i];
        for (j, (cj, fj)) in stop.iter().enumerate() {
            let adjacent = (ci[0] - cj[0]).abs() + (ci[2] - cj[2]).abs() == 1;
            if !seen[j] && adjacent && fj - fi <= 1.25 + 1e-9 && fi - fj <= 3.0 + 1e-9 {
                seen[j] = true;
                todo.push(j);
            }
        }
    }
    Some(seen)
}

/// [`flee_sight`] with the stop's stand-up cells: the on-foot positions no walk from them reaches
/// ([`walk_reach`]) are printed as their own class, never failures (the stop owner's `unreachable:`
/// line still leaves its cells out, counted).
pub fn flee_sight_from(d: &Design, declared: &[[i32; 3]], extra: &[(String, f64, f64, f64, f64)], riderless: &[[i32; 3]], unreachable: &[[i32; 3]], starts: &[[i32; 3]], reach_fn: Option<ReachFn>) -> (String, bool) {
    use redsim::entity::Vec3;
    let zombies: Vec<Vec3> = d
        .entities
        .iter()
        .filter(|e| e.kind.ends_with("boat") && e.nbt.as_deref().is_some_and(|n| n.contains("minecraft:zombie\"")))
        .map(|e| Vec3::new(e.pos[0], e.pos[1] + (0.5625f32 / 3.0f32) as f64 - 0.7, e.pos[2]))
        .collect();
    let mut points: Vec<(String, Vec3)> = Vec::new();
    let mut riderless_cells = 0usize;
    for (p, s) in &d.blocks {
        if !s.contains("rail") || s.contains("trail") {
            continue;
        }
        // (a `riderless:` header line: rails only an empty cart rides, the stop's park to its
        // pickup; no rider position there)
        if riderless.contains(p) {
            riderless_cells += 1;
            continue;
        }
        let c = redsim::pos::BlockPos::new(p[0], p[1], p[2]);
        let curve = ["north_east", "north_west", "south_east", "south_west"].iter().any(|k| s.contains(&format!("shape={k}")));
        for k in 0..=10 {
            let t = k as f64 / 10.0;
            let q = if curve {
                match crate::fleecheck::rail_segment(c, s) {
                    Some((a, b)) => Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t),
                    None => continue,
                }
            } else {
                crate::fleecheck::along_rail(c, s, t)
            };
            points.push((format!("({},{},{})", p[0], p[1], p[2]), q));
        }
    }
    // (the lifts: the cart rising in the bubble column, its centre on the column's axis; and any
    // further vertical path the caller names: a fall shaft, `extra`: name, x, z, y0, y1)
    let mut paths: Vec<(String, f64, f64, f64, f64)> = lift_paths(d);
    paths.extend(extra.iter().cloned());
    // (G1's CHANGES 2 on 1b2f0442f and 39925b692) the stop's on-foot positions: every cell of the
    // stop's box (x -6..4, z -6..10, the bed head's y - 2 to + 5) a villager can stand in, its feet
    // on top of the cell's own collision shape or on the full cube under an empty cell, 1.95 of
    // head room free above them; labelled by cell like the rails, so the declared waker places
    // match by cell. (vcompose, ironspec Nov 13 23:08 (5)) The box is the stop's real footprint, to
    // the fold's local z 11 (its zombie's post (-5,0,10) and shutter (-5,1,9), 58f770cc): the stop's
    // own waker inside it is no "other zombie"; it was z -6..5, which left the waker outside.
    let stop_box = d.ports.get("home").map(|h| ([-6, h[1] - 2, -6], [4, h[1] + 5, 10]));
    let full = |c: [i32; 3]| d.blocks.get(&c).and_then(|s| redsim::block::Block::parse(s).ok()).is_some_and(|b| redsim::behavior::full_cube(&b));
    // (water, lava, a bubble column, or a waterlogged block: it holds water (G1's note 2 on
    // 52cbd2734))
    let liquid = |c: [i32; 3]| d.blocks.get(&c).is_some_and(|s| matches!(s.trim_start_matches("minecraft:").split('[').next(), Some("water" | "lava" | "bubble_column")) || s.contains("waterlogged=true"));
    // (a cell's collision boxes, 1/16 units [x0, x1, y0, y1, z0, z1]; none for air, rails, levers,
    // dust, signs)
    let boxes = |c: [i32; 3]| -> Vec<redsim::behavior::Box16> { d.blocks.get(&c).and_then(|s| redsim::block::Block::parse(s).ok()).map(|b| redsim::behavior::collision_boxes(&b)).unwrap_or_default() };
    // (the villager's box from `feet` up 1.95 over its 0.6 footprint, 0.2..0.8 of the cell: no
    // collider in it)
    let head_room = |c: [i32; 3], feet: f64| -> bool {
        (0..=2).all(|dy| {
            let cc = [c[0], c[1] + dy, c[2]];
            boxes(cc).iter().all(|b| {
                let (y0, y1) = (cc[1] as f64 + b[2] as f64 / 16.0, cc[1] as f64 + b[3] as f64 / 16.0);
                let (x0, x1, z0, z1) = (b[0] as f64 / 16.0, b[1] as f64 / 16.0, b[4] as f64 / 16.0, b[5] as f64 / 16.0);
                let overlaps = y1 > feet + 1e-9 && y0 < feet + 1.95 - 1e-9 && x1 > 0.2 && x0 < 0.8 && z1 > 0.2 && z0 < 0.8;
                !overlaps
            })
        })
    };
    let mut stop_points: Vec<(String, Vec3)> = Vec::new();
    let mut stop_cells: Vec<([i32; 3], f64)> = Vec::new();
    let mut chest_labels: Vec<String> = Vec::new();
    let mut unreachable_cells = 0usize;
    let mut partial_tops: Vec<String> = Vec::new();
    let mut liquid_cells = 0usize;
    // (a cell an entity sits in, the zombie's boat: no villager stands there)
    let seated: Vec<[i32; 3]> = d.entities.iter().map(|e| [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]).collect();
    if let Some((lo, hi)) = stop_box {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let (c, up, down) = ([x, y, z], [x, y + 1, z], [x, y - 1, z]);
                    if seated.contains(&c) {
                        continue;
                    }
                    // (an `unreachable:` header line: standable cells no walk from the napper's
                    // cells reaches, declared with the stop owner's evidence, 58f770cc 14:22)
                    if unreachable.contains(&c) {
                        unreachable_cells += 1;
                        continue;
                    }
                    let bed_here = d.blocks.get(&c).is_some_and(|s| s.contains("_bed"));
                    let _ = bed_here;
                    // (G1's CHANGES 2 on 39925b692) the feet on the top of the cell's own collision
                    // shape (a bed 0.5625, a chest 0.875, a slab 0.5, a closed trapdoor 0.1875), or,
                    // in an empty cell, on the top of the collider below when it reaches the cell
                    // (a full cube at y, a fence or a wall at y + 0.5: G1's note 1 on 52cbd2734,
                    // ironspec 17:01), with 1.95 of head room above the feet free of colliders over
                    // the villager's 0.6 footprint
                    let own = boxes(c);
                    let feet = if own.is_empty() {
                        let below = boxes(down).iter().map(|b| b[3]).max().unwrap_or(0);
                        if below < 16 {
                            // (no floor here: air below, or a collider whose top stays in its own
                            // cell, where its own cell is the standing cell)
                            continue;
                        }
                        y as f64 + (below - 16) as f64 / 16.0
                    } else {
                        let top = own.iter().map(|b| b[3]).max().unwrap_or(0);
                        if top >= 16 {
                            // (the cell's own shape reaches its top: a villager stands on it, in the
                            // cell above; a shape that isn't a full cube is counted and printed,
                            // never silently dropped)
                            if !full(c) {
                                partial_tops.push(format!("({x},{y},{z}) {}", d.blocks.get(&c).map(|s| s.trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string()).unwrap_or_default()));
                            }
                            continue;
                        }
                        y as f64 + top as f64 / 16.0
                    };
                    if liquid(c) || liquid(up) {
                        liquid_cells += 1;
                        continue;
                    }
                    if !head_room(c, feet) {
                        continue;
                    }
                    let label = format!("({x},{y},{z})");
                    if d.blocks.get(&c).is_some_and(|s| s.contains("chest")) {
                        chest_labels.push(label.clone());
                    }
                    stop_points.push((label, Vec3::new(x as f64 + 0.5, feet, z as f64 + 0.5)));
                    stop_cells.push((c, feet));
                }
            }
        }
    }
    // (vcompose, ironspec 22:52; G1 23:20 (2)) the positions no villager path from the stand-up
    // cells reaches: computed by redsim's pathfinder (the path probe's rule), never declared; the
    // step rule (rise <= 1.25, drop <= 3) printed beside it as a cross-check
    let step = walk_reach(&stop_cells, starts);
    let has_start = stop_cells.iter().any(|(c, _)| starts.contains(c));
    let mut reach_err: Option<String> = None;
    let reach_paths: Option<Vec<Option<String>>> = match reach_fn {
        Some(f) if has_start => match f(&stop_cells, starts) {
            Ok(r) if r.len() == stop_cells.len() => Some(r),
            Ok(r) => {
                reach_err = Some(format!("{} answers for {} positions", r.len(), stop_cells.len()));
                None
            }
            Err(e) => {
                reach_err = Some(e);
                None
            }
        },
        _ => None,
    };
    let reach: Option<Vec<bool>> = reach_paths.as_ref().map(|r| r.iter().map(|p| p.is_some()).collect());
    // (a reachable stop position's path, printed with its in-sight rows: the evidence)
    let path_of = |label: &str| -> Option<String> { reach_paths.as_ref().and_then(|r| stop_points.iter().zip(r).find(|((l, _), _)| l == label).and_then(|(_, p)| p.clone())) };
    let unreach_labels: Vec<String> = match &reach {
        Some(r) => stop_points.iter().zip(r).filter(|(_, ok)| !**ok).map(|((l, _), _)| l.clone()).collect(),
        None => Vec::new(),
    };
    let reach_line = match (&reach, &reach_err) {
        (None, Some(e)) => format!("walk reach NOT JUDGED (the pathfinder failed: {e}): every in-sight position counts"),
        (None, None) if starts.is_empty() => "walk reach not judged (no stand-up cell given): every in-sight position counts".to_string(),
        (None, None) if !has_start => "walk reach NOT JUDGED (no stand-up cell is an on-foot position): every in-sight position counts".to_string(),
        (None, None) => "walk reach not judged (no pathfinder given): every in-sight position counts".to_string(),
        (Some(r), _) => {
            let differ: Vec<String> = match &step {
                Some(sr) => stop_points.iter().zip(r.iter().zip(sr)).filter(|(_, (a, b))| a != b).map(|((l, _), (a, _))| format!("{l} (path {})", if *a { "reaches" } else { "doesn't" })).collect(),
                None => vec!["not judged".to_string()],
            };
            format!(
                "{} of {} on-foot positions no villager path from the stand-up cells reaches (redsim's pathfinder, the path probe's rule: G1 Nov 13 23:20){}; the step rule (rise <= 1.25, drop <= 3) {}",
                unreach_labels.len(),
                stop_points.len(),
                if unreach_labels.is_empty() { String::new() } else { format!(": {}", unreach_labels.join(" ")) },
                if differ.is_empty() { "agrees on every position".to_string() } else { format!("differs at {}: {}", differ.len(), differ.join(", ")) }
            )
        }
    };
    points.extend(stop_points.iter().cloned());
    for (name, x, z, y0, y1) in paths {
        let (lo, hi) = (y0.min(y1), y0.max(y1));
        let mut y = lo;
        while y <= hi + 1e-9 {
            points.push((name.clone(), Vec3::new(x, y, z)));
            y += 0.1;
        }
    }
    let solid = |c: [i32; 3]| -> Option<String> {
        let s = d.blocks.get(&c)?;
        let b = redsim::block::Block::parse(s).ok()?;
        redsim::behavior::full_cube(&b).then(|| s.clone())
    };
    let mut in_sight: Vec<String> = Vec::new();
    let mut in_sight_unreach: Vec<String> = Vec::new();
    let mut landing_rows: Vec<String> = Vec::new();
    let mut hidden: Vec<String> = Vec::new();
    let mut waker: Vec<String> = Vec::new();
    let mut clear = 0usize;
    for (place, p) in &points {
        let mut near = false;
        // (ironspec 11:01, ruling (a)) a declared waker place (the stop part's `waker-places:`):
        // its in-sight positions are the waker's, printed as their own class, never failures
        let declared_place = declared.iter().any(|c| *place == format!("({},{},{})", c[0], c[1], c[2]));
        for z in &zombies {
            let dist = ((p.x - z.x).powi(2) + (p.y - z.y).powi(2) + (p.z - z.z).powi(2)).sqrt();
            if dist >= 6.05 {
                continue;
            }
            near = true;
            let (a, b) = (Vec3::new(p.x, p.y + 1.62, p.z), Vec3::new(z.x, z.y + 1.74, z.z));
            let cell = |q: Vec3| [q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32];
            let (ca, cb) = (cell(a), cell(b));
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
            let n = ((len * 40.0).ceil() as usize).max(1);
            let mut block: Option<([i32; 3], String)> = None;
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let c = cell(Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t));
                if c == ca || c == cb {
                    continue;
                }
                if let Some(s) = solid(c) {
                    block = Some((c, s));
                    break;
                }
            }
            let row = format!("{place} at ({:.2},{:.2},{:.2}), the zombie at ({:.2},{:.2},{:.2}) {dist:.2} away", p.x, p.y, p.z, z.x, z.y, z.z);
            let class = match &block {
                Some((c, s)) => format!("hidden by ({},{},{}) {s}", c[0], c[1], c[2]),
                None if declared_place => "in sight, a declared waker place".to_string(),
                None => "IN SIGHT".to_string(),
            };
            if chest_labels.contains(place) && landing_rows.len() < 8 {
                landing_rows.push(format!("{row}: {class}"));
            }
            match block {
                Some((c, s)) => hidden.push(format!("{row}: hidden by ({},{},{}) {s}", c[0], c[1], c[2])),
                None if declared_place => waker.push(row),
                // (vcompose, ironspec 22:52) a position no villager path reaches: its own class
                None if unreach_labels.contains(place) => in_sight_unreach.push(row),
                // (G1 23:20 (2)) a stop position in sight: the path that reaches it printed
                None => in_sight.push(match path_of(place) {
                    Some(p) => format!("{row} (a villager path reaches it, {p})"),
                    None => row,
                }),
            }
        }
        if !near {
            clear += 1;
            if chest_labels.contains(place) && landing_rows.len() < 8 {
                landing_rows.push(format!("{place} at ({:.2},{:.2},{:.2}): no zombie within 6.05", p.x, p.y, p.z));
            }
        }
    }
    let list = |v: &[String]| v.iter().take(30).cloned().collect::<Vec<_>>().join("; ");
    // (ironspec 12:23 / G1: every zombie but the stop's own waker stays at least 6.05 from every
    // stop cell, sight or not: the queue's post among them; every rider position in the stop's
    // box, its rails and its on-foot cells)
    let in_box = |p: &Vec3| stop_box.is_some_and(|(lo, hi)| p.x >= lo[0] as f64 && p.x < hi[0] as f64 + 1.0 && p.y >= lo[1] as f64 - 1.0 && p.y < hi[1] as f64 + 1.0 && p.z >= lo[2] as f64 && p.z < hi[2] as f64 + 1.0);
    let mut stop_rows: Vec<String> = Vec::new();
    let mut stop_ok = true;
    for z in zombies.iter().filter(|z| !in_box(z)) {
        // (vcompose, ironspec 22:52) the nearest REACHABLE stop position judges; an unreachable
        // nearer one is printed beside it, never a failure
        let near = |reach_only: bool| points.iter().filter(|(place, p)| in_box(p) && (!reach_only || !unreach_labels.contains(place))).map(|(place, p)| (((p.x - z.x).powi(2) + (p.y - z.y).powi(2) + (p.z - z.z).powi(2)).sqrt(), place, p)).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, place, p)) = near(true) {
            let pass = d >= 6.05;
            stop_ok &= pass;
            let unreach_note = match near(false) {
                Some((du, pu, qu)) if du < d => format!("; nearer, no villager path reaches it: {pu} ({:.2},{:.2},{:.2}) {du:.2}{}", qu.x, qu.y, qu.z, if du < 6.05 { " (within 6.05: printed, not a failure)" } else { "" }),
                _ => String::new(),
            };
            stop_rows.push(format!("the zombie at ({:.2},{:.2},{:.2}): {d:.2} from the nearest stop position {place} ({:.2},{:.2},{:.2}) ({}){unreach_note}", z.x, z.y, z.z, p.x, p.y, p.z, if pass { "ok" } else { "FAIL" }));
        }
    }
    // (G1 Nov 13 23:20 (1)) what the real-footprint box moved: the zombies inside it are the
    // stop's own and leave this bar, each named with the bar that judges it instead; the on-foot
    // positions the wider box added counted
    let own_zombies: Vec<String> = zombies.iter().filter(|z| in_box(z)).map(|z| format!("the zombie at ({:.2},{:.2},{:.2})", z.x, z.y, z.z)).collect();
    let added = stop_cells.iter().filter(|(c, _)| c[2] > 5).count();
    let moved_line = format!(
        "the stop box x -6..4 z -6..10 (the stop's real footprint to the fold's local z 11, ironspec Nov 13 23:08 (5); it was z -6..5): {added} on-foot positions it added (z 6..10); zombies inside it, the stop's own, left this bar (9fc3155f's sight static judges each: the sleeper's eye through the open shutter): {}",
        if own_zombies.is_empty() { "none".to_string() } else { own_zombies.join(", ") }
    );
    let ok = in_sight.is_empty() && stop_ok;
    let s = format!(
        "the 6.05 flee bar, sight-gated ({} rider positions every 0.1 along every rail and both lifts ({} riderless rail cells declared and left out), and {} on-foot positions in the stop's box, the feet on the cell's own collision shape or on the top of the collider below an empty cell when it reaches the cell ({} standable cells declared unreachable and left out; {} cells holding liquid left out; the cells whose own shape reaches its top without a full cube, stood on from the cell above: {}; {reach_line}), {} zombies; the eye at the cart (or the feet) + 1.62, the zombie's at its feet + 1.74; hidden: a full-collider block on the eye-to-eye ray; {} declared waker places): IN SIGHT within 6.05 {}{}; in sight within 6.05 from positions no villager path reaches (printed, not failures) {}{}; the waker's declared places in sight within 6.05 {}{}; hidden within 6.05 {}{}; clear {}; {moved_line}; every other zombie at least 6.05 from every reachable stop position (sight or not): {}; the landings on a chest (G1): {}; the pre-wake state (the open hatch's plates, hiding the zombie until F shuts them) is not a full-collider test: it rests on tick-exact's official A6 scene (NEAREST_HOSTILE never set with the hatch open); {}\n",
        points.len() - stop_points.len(),
        riderless_cells,
        stop_points.len(),
        unreachable_cells,
        liquid_cells,
        if partial_tops.is_empty() { "none".to_string() } else { format!("{}: {}", partial_tops.len(), partial_tops.join(", ")) },
        zombies.len(),
        declared.len(),
        in_sight.len(),
        if in_sight.is_empty() { String::new() } else { format!(" (FAIL): {}", list(&in_sight)) },
        in_sight_unreach.len(),
        if in_sight_unreach.is_empty() { String::new() } else { format!(": {}", list(&in_sight_unreach)) },
        waker.len(),
        if waker.is_empty() { String::new() } else { format!(" (declared, not failures): {}", list(&waker)) },
        hidden.len(),
        if hidden.is_empty() { String::new() } else { format!(": {}", list(&hidden)) },
        clear,
        if stop_rows.is_empty() { "no zombie outside the stop".to_string() } else { stop_rows.join("; ") },
        if chest_labels.is_empty() { "no chest in the stop".to_string() } else if landing_rows.is_empty() { format!("{} (no position)", chest_labels.join(" ")) } else { landing_rows.join("; ") },
        if ok { "PASS" } else { "FAIL" }
    );
    (s, ok)
}

/// The two bubble-column lifts' rider paths: the shared lift (x -5, z 7, y 0..23) and napper A's
/// lift (x 6, z 4, from R's level to y -3; R at y -19 in rev 3, moving with the stop: the `home`
/// port's y + 18 below that).
fn lift_paths(d: &Design) -> Vec<(String, f64, f64, f64, f64)> {
    let dy = d.ports.get("home").map_or(0, |h| h[1] + 18) as f64;
    vec![("the shared lift".to_string(), -4.5, 7.5, 0.0, 23.0), ("napper A's lift".to_string(), 6.5, 4.5, -19.0 + dy, -3.0)]
}

/// (ironspec 12:14, G1 on tick-exact's dad9c1a38) Golem detection across the tiling, per axis: a
/// golem (box 1.4 x 2.7 x 1.4 at its feet) is detected by a villager (0.6 x 1.95 x 0.6) iff the
/// boxes' gap is under 16 on every axis (the villager's box inflated by 16 overlaps it). The
/// neighbours' golems: this module's pad spawn cells (feet over its cobblestone at y -7) moved to
/// each neighbour (east +32 and west -32, the southern copies mirrored in z: z' = 35 - z, at x 0 and
/// +-32); the riders: every rider position of [`flee_sight`] (every rail every 0.1, the lifts, the
/// extra vertical paths). Per path class (a rail cell's part of the module, a lift or a path), the
/// worst pair with its per-axis gaps and the result.
pub fn golem_tiling(d: &Design, extra: &[(String, f64, f64, f64, f64)], riderless: &[[i32; 3]]) -> String {
    use redsim::entity::Vec3;
    let pad: Vec<Vec3> = d.blocks.iter().filter(|(p, s)| p[1] == -7 && s.trim_start_matches("minecraft:").split('[').next() == Some("cobblestone") && !d.blocks.contains_key(&[p[0], -6, p[2]])).map(|(p, _)| Vec3::new(p[0] as f64 + 0.5, -6.0, p[2] as f64 + 0.5)).collect();
    // (the rider positions: rails every 0.1 but the declared riderless ones, the lifts, the extra
    // paths)
    let mut riders: Vec<(String, Vec3)> = Vec::new();
    for (p, s) in &d.blocks {
        if !s.contains("rail") || s.contains("trail") || riderless.contains(p) {
            continue;
        }
        let c = redsim::pos::BlockPos::new(p[0], p[1], p[2]);
        let curve = ["north_east", "north_west", "south_east", "south_west"].iter().any(|k| s.contains(&format!("shape={k}")));
        let class = if p[1] <= -14 {
            "the stop and the napper's way"
        } else if p[1] >= 16 && p[1] <= 22 && p[2] <= 6 {
            "the lanes"
        } else if p[1] >= 23 {
            "the split row and the sidings"
        } else {
            "the station and the merge row"
        };
        for k in 0..=10 {
            let t = k as f64 / 10.0;
            let q = if curve {
                match crate::fleecheck::rail_segment(c, s) {
                    Some((a, b)) => Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t),
                    None => continue,
                }
            } else {
                crate::fleecheck::along_rail(c, s, t)
            };
            riders.push((class.to_string(), q));
        }
    }
    let mut paths: Vec<(String, f64, f64, f64, f64)> = lift_paths(d);
    paths.extend(extra.iter().cloned());
    for (name, x, z, y0, y1) in paths {
        let (lo, hi) = (y0.min(y1), y0.max(y1));
        let mut y = lo;
        while y <= hi + 1e-9 {
            riders.push((name.clone(), Vec3::new(x, y, z)));
            y += 0.1;
        }
    }
    // (the gap between two intervals; the per-axis gaps between a golem and a villager)
    let gap = |a0: f64, a1: f64, b0: f64, b1: f64| (b0 - a1).max(a0 - b1).max(0.0);
    let gaps = |g: Vec3, v: Vec3| [gap(g.x - 0.7, g.x + 0.7, v.x - 0.3, v.x + 0.3), gap(g.y, g.y + 2.7, v.y, v.y + 1.95), gap(g.z - 0.7, g.z + 0.7, v.z - 0.3, v.z + 0.3)];
    let neighbours: [(&str, f64, bool); 6] = [("east (+32)", 32.0, false), ("west (-32)", -32.0, false), ("south, mirrored", 0.0, true), ("south-east, mirrored", 32.0, true), ("south-west, mirrored", -32.0, true), ("", 0.0, false)];
    let mut s = format!("golem detection across the tiling (per axis: detected iff every box gap < 16; {} pad spawn cells moved to each neighbour):\n", pad.len());
    for (name, dx, m) in neighbours.iter().filter(|n| !n.0.is_empty()) {
        let golems: Vec<Vec3> = pad.iter().map(|g| Vec3::new(g.x + dx, g.y, if *m { 35.0 - g.z + 1.0 } else { g.z })).collect();
        let mut classes: BTreeMap<String, (f64, [f64; 3], Vec3, Vec3, usize)> = BTreeMap::new();
        for (class, v) in &riders {
            for g in &golems {
                let gp = gaps(*g, *v);
                // (the margin: the largest per-axis gap - 16; negative on every axis = detected)
                let margin = gp.iter().cloned().fold(f64::MIN, f64::max) - 16.0;
                let e = classes.entry(class.clone()).or_insert((f64::MAX, [0.0; 3], *g, *v, 0));
                if margin < 0.0 {
                    e.4 += 1;
                }
                if margin < e.0 {
                    *e = (margin, gp, *g, *v, e.4);
                }
            }
        }
        s.push_str(&format!("  {name}:"));
        for (class, (m, gp, g, v, n)) in &classes {
            s.push_str(&format!(
                " {class}: {} (gaps x {:.2} y {:.2} z {:.2}, {:+.2}; golem ({:.1},{:.1},{:.1}), rider ({:.2},{:.2},{:.2}){});",
                if *m < 0.0 { "DETECTED" } else { "clear" },
                gp[0],
                gp[1],
                gp[2],
                m,
                g.x,
                g.y,
                g.z,
                v.x,
                v.y,
                v.z,
                if *n > 0 { format!(", {n} detecting pairs") } else { String::new() }
            ));
        }
        s.push('\n');
    }
    s
}

/// The cells of a design file's `waker-places: x,y,z+x,y,z+...=REASON[; x,y,z+...=REASON]...` line
/// (ironspec 11:01: the stop part's header, copied into the module's in the loop frame): every
/// group's cells.
pub fn waker_places(text: &str) -> Vec<[i32; 3]> {
    header_cells(text, "waker-places:")
}

/// The cells of a header line `KEY x,y,z+x,y,z+...=REASON[; ...]` (each group's cells before its
/// `=`; a group without `=` is all cells).
pub fn header_cells(text: &str, key: &str) -> Vec<[i32; 3]> {
    let mut v = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(key) else { continue };
        for group in rest.split(';') {
            let cells = group.split('=').next().unwrap_or("");
            for c in cells.split('+') {
                let n: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if n.len() == 3 {
                    v.push([n[0], n[1], n[2]]);
                }
            }
        }
    }
    v
}

/// (ironspec 20:19 PT (2a), G1) Whether a block state string names a signal source, from redsim's
/// own predicate (`behavior::is_signal_source` on the parsed state; dust counts, as a rail reads
/// its default state's answer: `shouldSignal` toggles only inside its own recalculation), so the
/// T-reach scan's list can't drift from the simulator's; a block redsim doesn't parse is refused
/// earlier, by the design's own parse.
pub fn is_source_name(s: &str) -> bool {
    thread_local! {
        static PROBE: redsim::world::World = redsim::world::World::new(redsim::pos::BlockPos::new(0, 0, 0), [1, 1, 1]);
    }
    let s = if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") };
    match redsim::block::Block::parse(&s) {
        Ok(b) => matches!(b, redsim::block::Block::Wire { .. }) || PROBE.with(|w| redsim::behavior::is_signal_source(w, &b)),
        Err(_) => false,
    }
}

/// (onechunk Nov 13, settle-check's strict reader) A header declaration's groups: the text after
/// its key split at every `;` outside brackets and parentheses, each group's leading cells (`x,y,z`
/// cells joined by `+`, or boxes of two corners, the token before the first `=`, space or `(`)
/// moved to integers, and the rest of the group as written. A group whose leading token holds no
/// cells is returned with no cells (a text group); a token that starts like cells but doesn't parse
/// as whole x,y,z triples is an error.
pub fn header_groups(rest: &str) -> Result<Vec<(Vec<[i32; 3]>, String)>, String> {
    let mut groups: Vec<String> = Vec::new();
    let (mut depth, mut cur) = (0i32, String::new());
    for ch in rest.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
        if ch == ';' && depth == 0 {
            groups.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    if depth != 0 {
        return Err(format!("unbalanced brackets in `{}`", rest.trim()));
    }
    groups.push(cur);
    let mut out = Vec::new();
    for g in groups {
        let g = g.trim();
        if g.is_empty() {
            continue;
        }
        let end = g.find(['=', ' ', '(']).unwrap_or(g.len());
        let (tok, tail) = g.split_at(end);
        let starts_like_cells = tok.chars().next().is_some_and(|c| c == '-' || c.is_ascii_digit()) && tok.contains(',');
        if !starts_like_cells {
            out.push((Vec::new(), g.to_string()));
            continue;
        }
        let mut cells = Vec::new();
        for part in tok.split('+') {
            let n: Vec<i32> = part.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("the cells `{tok}` in `{g}`: {e}"))).collect::<Result<_, _>>()?;
            if n.is_empty() || n.len() % 3 != 0 {
                return Err(format!("the cells `{tok}` in `{g}`: not whole x,y,z triples"));
            }
            for t in n.chunks(3) {
                cells.push([t[0], t[1], t[2]]);
            }
        }
        out.push((cells, tail.to_string()));
    }
    Ok(out)
}

/// (day-1 05:45 PT) The controller line ends a module declares as redstone-block stand-ins: the
/// cells of every `# stand-in:` group whose text names a redstone block.
pub fn block_standins(text: &str) -> Result<Vec<[i32; 3]>, String> {
    let mut v = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("# stand-in:") else { continue };
        for (cells, tail) in header_groups(rest)? {
            if tail.contains("redstone block") {
                v.extend(cells);
            }
        }
    }
    Ok(v)
}

/// The cells of a header line's groups with the state each names (`KEY CELLS=STATE (why); ...`:
/// the state is the first token after `=`, `minecraft:` added when it has no namespace; G1's
/// note (c) on 961090aef: the statics lay exactly what the run lays).
pub fn header_cell_states(text: &str, key: &str) -> Vec<([i32; 3], String)> {
    let mut v = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(key) else { continue };
        for group in rest.split(';') {
            let Some((cells, after)) = group.split_once('=') else { continue };
            let state = after.trim_start().split([' ', '(']).next().unwrap_or("").trim();
            if state.is_empty() {
                continue;
            }
            let state = if state.contains(':') { state.to_string() } else { format!("minecraft:{state}") };
            for c in cells.split('+') {
                let n: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if n.len() == 3 {
                    v.push(([n[0], n[1], n[2]], state.clone()));
                }
            }
        }
    }
    v
}

/// The cells of the file's `keep-clear: x,y,z x,y,z ... (reason)` lines (the stop part's header,
/// copied into the module's: cells that must hold no block; the reason's text after the first
/// token that is no cell is left alone).
pub fn header_keep_clear(text: &str) -> Vec<[i32; 3]> {
    let mut v = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("keep-clear:") else { continue };
        for tok in rest.split_whitespace() {
            let n: Vec<i32> = tok.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if n.len() != 3 || tok.split(',').count() != 3 {
                break;
            }
            v.push([n[0], n[1], n[2]]);
        }
    }
    v
}

/// (58f770cc 13:59) The file's `keep-block: x,y,z[+...]=BLOCK[props] (reason)[; ...]` lines: each
/// cell must hold that block (its name, and every property the line lists).
pub fn keep_block_check(d: &Design, text: &str) -> (String, bool) {
    let mut rows = Vec::new();
    let mut ok = true;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("keep-block:") else { continue };
        for group in rest.split(';') {
            let Some((cells, spec)) = group.split_once('=') else { continue };
            let spec = spec.split_whitespace().next().unwrap_or("");
            let (name, props) = spec.split_once('[').map_or((spec, ""), |(n, p)| (n, p.trim_end_matches(']')));
            let name = name.trim_start_matches("minecraft:");
            for c in cells.split('+') {
                let n: Vec<i32> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if n.len() != 3 {
                    continue;
                }
                let held = d.blocks.get(&[n[0], n[1], n[2]]);
                let good = held.is_some_and(|b| {
                    b.trim_start_matches("minecraft:").split('[').next() == Some(name) && {
                        // (key=value exactly: `shape=east` doesn't match `shape=east_west`)
                        let held: Vec<&str> = b.split_once('[').map_or("", |(_, r)| r.trim_end_matches(']')).split(',').collect();
                        props.split(',').filter(|p| !p.is_empty()).all(|p| held.contains(&p))
                    }
                });
                ok &= good;
                rows.push(format!("({},{},{}) {spec}: {} ({})", n[0], n[1], n[2], held.map_or("no block".to_string(), |b| b.clone()), if good { "PASS" } else { "FAIL" }));
            }
        }
    }
    if rows.is_empty() {
        return (String::new(), true);
    }
    (format!("keep-block: {}\n", rows.join("; ")), ok)
}

pub fn creeper_report(spec: &redsim::entity::mob::farm::FarmSpec, glass: bool, unlit_control: bool) -> Result<(String, bool), String> {
    use redsim::entity::mob::farm::{Run, RunOpts, WalkMode};
    use redsim::entity::mob::hostile;
    use redsim::pos::BlockPos;
    let o = RunOpts { ai: false, ..RunOpts::steady(1, 0, WalkMode::Random) };
    // (the world padded by 16 around the design: the flood's ring starts (a mob walking in from
    // beyond the bounds) sit in open air, not on the design's own outer cells)
    let mut sp = spec.clone();
    sp.lo = BlockPos::new(spec.lo.x - 16, spec.lo.y - 16, spec.lo.z - 16);
    sp.hi = BlockPos::new(spec.hi.x + 16, spec.hi.y + 16, spec.hi.z + 16);
    // (the farm's own mobs left out: a creeper's positions are the blocks' business)
    sp.entities.clear();
    // (a land world: redsim models block and sky light only there, so a portal's 11 and a torch
    // light their cells and a monster starts only at block light 0)
    sp.land = true;
    let targets: Vec<BlockPos> = sp
        .blocks
        .iter()
        .filter(|(_, b)| {
            let s = b.to_string();
            s.contains("nether_portal") || (glass && s.trim_start_matches("minecraft:").split('[').next() == Some("glass"))
        })
        .map(|(p, _)| *p)
        .collect();
    // (the positive control: every portal block glass, unlit, the targets kept: the spaces beside
    // them go dark and must be flagged)
    if unlit_control {
        for (_, b) in sp.blocks.iter_mut() {
            if b.to_string().contains("nether_portal") {
                *b = redsim::block::Block::parse("minecraft:glass").map_err(|e| format!("glass: {e}"))?;
            }
        }
    }
    let spec = &sp;
    let w = if spec.has_bed() { Run::new(spec, &o)?.world } else { Run::new_homeless(spec, &o)?.world };
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for (p, _) in &spec.blocks {
        for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v - 12);
            hi[k] = hi[k].max(v + 12);
        }
    }
    // (vanilla's #prevent_mob_spawning_inside: no natural spawn in a rail's cell; the flood's
    // `exclude` takes them out of the starts while its walks still cross them)
    let rail_cells: std::collections::HashSet<BlockPos> = spec.blocks.iter().filter(|(_, b)| redsim::rails::is_rail(b)).map(|(p, _)| *p).collect();
    let reach = hostile::flood_with(&w, hostile::CREEPER, &rail_cells, (BlockPos::new(lo[0], lo[1], lo[2]), BlockPos::new(hi[0], hi[1], hi[2])), hostile::Seeding::SpawnRules { seed: None });
    let dist = |c: BlockPos, feet: f64, t: BlockPos| {
        let pt = [c.x as f64 + 0.5, feet, c.z as f64 + 0.5];
        let tl = [t.x as f64, t.y as f64, t.z as f64];
        (0..3).map(|k| (tl[k] - pt[k]).max(pt[k] - (tl[k] + 1.0)).max(0.0).powi(2)).sum::<f64>().sqrt()
    };
    let (mut near, mut charged) = (Vec::new(), Vec::new());
    for (c, feet) in &reach.cells {
        let Some((d, t)) = targets.iter().map(|t| (dist(*c, *feet, *t), *t)).min_by(|a, b| a.0.total_cmp(&b.0)) else { continue };
        if d <= 5.5 {
            near.push((d, *c, *feet, t));
        } else if d <= 10.5 {
            charged.push((d, *c, *feet, t));
        }
    }
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    charged.sort_by(|a, b| a.0.total_cmp(&b.0));
    // (ironspec 05:05) the spawn origins: the reached cells a creeper can spawn at (block light 0
    // by the land world's propagation, its floor and its box: `hostile::spawn_parts`); an
    // unreadable light refuses the report (cc767b78, ironspec 12:54)
    let kind = redsim::entity::mob::natural::kind_of("minecraft:creeper");
    let mut starts: Vec<(BlockPos, f64)> = Vec::new();
    for &(c, f) in reach.cells.iter() {
        if rail_cells.contains(&c) || w.block_light(c)? != 0 {
            continue;
        }
        if kind.is_none_or(|k| {
            let (_, placement, free) = hostile::spawn_parts(&w, k, c);
            placement && free
        }) {
            starts.push((c, f));
        }
    }
    // (the light at every spawn space beside a target: an air cell face-adjacent to it with a
    // floor below)
    let mut spaces: Vec<(BlockPos, i32)> = Vec::new();
    for t in &targets {
        for q in [BlockPos::new(t.x + 1, t.y, t.z), BlockPos::new(t.x - 1, t.y, t.z), BlockPos::new(t.x, t.y, t.z + 1), BlockPos::new(t.x, t.y, t.z - 1), BlockPos::new(t.x, t.y + 1, t.z)] {
            let air = w.get(q).to_string().trim_start_matches("minecraft:").starts_with("air");
            let floor = !w.get(q.below()).to_string().trim_start_matches("minecraft:").starts_with("air");
            if air && floor && !spaces.iter().any(|(p, _)| *p == q) {
                spaces.push((q, w.block_light(q)?));
            }
        }
    }
    // (a light read inside the spawn rules that failed: the report is refused)
    let light_errors = w.take_light_errors();
    if !light_errors.is_empty() {
        return Err(format!("creeper report: {} light read(s) failed (untaken since the world's last take: pending, or moved into its errors by a tick: some may predate this report): {}", light_errors.len(), light_errors.join("; ")));
    }
    let lmin = spaces.iter().map(|x| x.1).min().unwrap_or(0);
    let lmax = spaces.iter().map(|x| x.1).max().unwrap_or(0);
    let dark: Vec<&(BlockPos, i32)> = spaces.iter().filter(|x| x.1 <= 0).collect();
    let show = |v: &Vec<(f64, BlockPos, f64, BlockPos)>| v.iter().take(12).map(|(d, c, f, t)| format!(" ({},{:.2},{}) {d:.2} from ({},{},{})", c.x, f, c.z, t.x, t.y, t.z)).collect::<String>();
    let mut s = format!(
        "creeper positions{} (a hostile flood of the creeper's body from every spawnable cell at block light 0 by the land world's propagation, and every position walkable from there; {} positions) against {} target blocks ({}):\n",
        if unlit_control { " — POSITIVE CONTROL: every portal block glass (unlit), the targets kept" } else { "" },
        reach.cells.len(),
        targets.len(),
        if glass { "every portal block and every glass block" } else { "every portal block" }
    );
    s.push_str(&format!(
        "  the spawn spaces beside the targets (an air cell face-adjacent with a floor below): {}, block light {lmin}..{lmax}; dark (0): {}{}\n",
        spaces.len(),
        dark.len(),
        dark.iter().take(12).map(|(p, _)| format!(" ({},{},{})", p.x, p.y, p.z)).collect::<String>()
    ));
    s.push_str(&format!(
        "  the spawn origins (reached cells a creeper can spawn at): {}{}\n",
        starts.len(),
        starts.iter().take(16).map(|(c, f)| format!(" ({},{:.2},{})", c.x, f, c.z)).collect::<String>()
    ));
    s.push_str(&format!("  within 5.5 (a creeper's blast): {}{}{}\n", near.len(), if near.is_empty() { " (PASS)" } else { " (FAIL)" }, show(&near)));
    s.push_str(&format!("  5.5 to 10.5 (a charged creeper's, information): {}{}\n", charged.len(), show(&charged)));
    Ok((s, near.is_empty()))
}

/// (G1 on 6704219e6 / 3326b8d96: an off-rail rider is invisible to the rail cells) The declared
/// off-rail paths (`--path NAME=x,y,z..x,y,z[..]`: the drop shafts, a bubble-column lift, a fall,
/// a hold with no cart placed): each consecutive pair of points a straight run of cells, both ends
/// included. Returns each path's cells.
pub fn path_cells(spec: &str) -> Result<(String, Vec<P>), String> {
    let (name, rest) = spec.split_once('=').ok_or(format!("--path {spec}: NAME=x,y,z..x,y,z"))?;
    let pts: Vec<P> = rest
        .split("..")
        .map(|t| {
            let n: Vec<i32> = t.split(',').map(|s| s.trim().parse::<i32>().map_err(|e| format!("--path {spec}: {e}"))).collect::<Result<_, _>>()?;
            if n.len() != 3 {
                return Err(format!("--path {spec}: a point is x,y,z"));
            }
            Ok([n[0], n[1], n[2]])
        })
        .collect::<Result<_, _>>()?;
    if pts.len() < 2 {
        return Err(format!("--path {spec}: at least two points"));
    }
    let mut cells: Vec<P> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = (0..3).map(|k| (b[k] - a[k]).abs()).max().unwrap_or(0);
        for i in 0..=n {
            let c = [0, 1, 2].map(|k| a[k] + if n == 0 { 0 } else { (b[k] - a[k]) * i / n });
            if !cells.contains(&c) {
                cells.push(c);
            }
        }
    }
    Ok((name.trim().to_string(), cells))
}

/// The bed radius over the declared off-rail paths: a rider anywhere in each path cell (its feet
/// over the cell's floor, its centre anywhere over the cell: the least distance from the head's
/// centre to the cell's box at feet height), judged like a part's rails.
pub fn path_bed_radius(paths: &[(String, Vec<P>)], bed: P, exempt: &[String]) -> (String, bool) {
    let centre = [bed[0] as f64 + 0.5, bed[1] as f64 + 0.5, bed[2] as f64 + 0.5];
    let limit = 17.0;
    let mut s = String::from("bed radius over the declared off-rail paths (a rider anywhere over each path cell, feet on its floor):\n");
    let mut ok = true;
    for (name, cells) in paths {
        let mut best: Option<(f64, P)> = None;
        for c in cells {
            let q = [centre[0].clamp(c[0] as f64, c[0] as f64 + 1.0), centre[1].clamp(c[1] as f64, c[1] as f64 + 0.999), centre[2].clamp(c[2] as f64, c[2] as f64 + 1.0)];
            let dd = ((q[0] - centre[0]).powi(2) + (q[1] - centre[1]).powi(2) + (q[2] - centre[2]).powi(2)).sqrt();
            if best.is_none_or(|(m, _)| dd < m) {
                best = Some((dd, *c));
            }
        }
        let Some((dd, c)) = best else { continue };
        let exempted = exempt.iter().any(|n| n == name);
        let verdict = if exempted {
            "EXEMPT (the napper's own path)"
        } else if dd > limit {
            "ok  "
        } else {
            ok = false;
            "FAIL"
        };
        s.push_str(&format!("  {verdict} path {name} ({} cells): least {dd:.2} at ({},{},{}){}\n", cells.len(), c[0], c[1], c[2], if dd > limit || exempted { String::new() } else { format!(" ({:.2} short)", limit - dd) }));
    }
    (s, ok)
}

/// The bed radius (ironspec Nov 11 18:02, from tick-exact's reload proof): after a load at night a
/// villager sits in REST until its first hostile scan, and `ValidateNearbyPoi` erases the HOME of a
/// villager whose position lies within 16 of the occupied shared bed's head centre
/// (`closerToCenterThan(position, 16)`). Every rider position must stay more than 16 + 1 from
/// it, the least over every track position: each part's rails (the cart's path through each cell)
/// and its ridden carts off rails; a reserved box is a bound (the least to any cell of it). Parts
/// named in `exempt` (the napper's own path) are printed, not judged.
pub fn bed_radius(parts: &[Part], reserves: &[Reserve], bed: P, exempt: &[String]) -> (String, bool) {
    let centre = [bed[0] as f64 + 0.5, bed[1] as f64 + 0.5, bed[2] as f64 + 0.5];
    let limit = 17.0;
    let mut s = format!("bed radius: every rider position more than {limit} (16 + 1) from the occupied shared bed's head centre ({:.1},{:.1},{:.1}), the least over the cart's path through every rail cell of each part and its ridden carts off rails:\n", centre[0], centre[1], centre[2]);
    let mut ok = true;
    for p in parts {
        let mut best: Option<(f64, String)> = None;
        for (c, st) in &p.placed.blocks {
            if let Some(shape) = rail_shape_of(st) {
                for (a, b) in rail_segments(*c, shape) {
                    let dd = seg_dist(centre, a, b);
                    if best.as_ref().is_none_or(|(m, _)| dd < *m) {
                        best = Some((dd, format!("rail ({},{},{}) {shape}", c[0], c[1], c[2])));
                    }
                }
            }
        }
        for e in p.placed.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))) {
            let dd = ((e.pos[0] - centre[0]).powi(2) + (e.pos[1] - centre[1]).powi(2) + (e.pos[2] - centre[2]).powi(2)).sqrt();
            if best.as_ref().is_none_or(|(m, _)| dd < *m) {
                best = Some((dd, format!("rider at ({:.2},{:.2},{:.2})", e.pos[0], e.pos[1], e.pos[2])));
            }
        }
        let Some((dd, at)) = best else { continue };
        let exempted = exempt.iter().any(|n| *n == p.spec.name);
        let verdict = if exempted {
            "EXEMPT (the napper's own path)"
        } else if dd > limit {
            "ok  "
        } else {
            ok = false;
            "FAIL"
        };
        s.push_str(&format!("  {verdict} {}: least {dd:.2} at {at}{}\n", p.spec.name, if dd > limit || exempted { String::new() } else { format!(" ({:.2} short)", limit - dd) }));
    }
    for r in reserves {
        let q = [0, 1, 2].map(|k| centre[k].clamp(r.b.0[k] as f64, r.b.1[k] as f64 + 1.0));
        let dd = ((q[0] - centre[0]).powi(2) + (q[1] - centre[1]).powi(2) + (q[2] - centre[2]).powi(2)).sqrt();
        s.push_str(&format!("  bound {} (reserved): its box comes within {dd:.2}{}\n", r.name, if dd > limit { "" } else { " (a rider position in it could fail)" }));
    }
    (s, ok)
}

/// Condition (a) extended to every villager position (ironspec Nov 11 18:54, for b8949c96's rule):
/// how many villager positions (every ridden cart's cell and every rail cell of `d`) lie more than
/// 1 section (Chebyshev, in section coordinates, y included) from every bed's section (`beds`: the
/// shared bed's head and the sentinel's bed, the column's frame). x and z sections come from the
/// written frame (`shift`); the y sections depend on where the site puts the design's y, so every
/// one of the 16 phases is printed (a phase p: design y + p is the world y modulo 16).
pub fn section_report(d: &Design, beds: &[P], shift: P) -> String {
    let mut cells: std::collections::BTreeSet<P> = std::collections::BTreeSet::new();
    for e in d.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))) {
        cells.insert(e.cell());
    }
    for (c, s) in &d.blocks {
        if rail_shape_of(s).is_some() {
            cells.insert(*c);
        }
    }
    let sec = |c: P, p: i32| [(c[0] + shift[0]).div_euclid(16), (c[1] + p).div_euclid(16), (c[2] + shift[2]).div_euclid(16)];
    let mut s = format!("sections (condition (a) over every villager position: {} cells, riders and rails; more than 1 section, Chebyshev, from every bed's section; beds {}), per y phase p (world y = design y + p, mod 16):\n", cells.len(), beds.iter().map(|b| format!("({},{},{})", b[0], b[1], b[2])).collect::<Vec<_>>().join(" "));
    for p in 0..16 {
        let mut far = 0usize;
        let mut worst = 0i32;
        let mut example: Option<P> = None;
        for c in &cells {
            let cs = sec(*c, p);
            let dmin = beds.iter().map(|b| {
                let bs = sec(*b, p);
                (cs[0] - bs[0]).abs().max((cs[1] - bs[1]).abs()).max((cs[2] - bs[2]).abs())
            }).min().unwrap_or(0);
            if dmin > 1 {
                far += 1;
                if dmin > worst || example.is_none() {
                    example = Some(*c);
                }
            }
            worst = worst.max(dmin);
        }
        s.push_str(&format!("  p {p:>2}: {far} of {} beyond 1 section (the farthest {worst} sections){}\n", cells.len(), example.map(|c| format!(", e.g. ({},{},{})", c[0], c[1], c[2])).unwrap_or_default()));
    }
    s
}

/// The summon box (ironspec Nov 11 19:16, day-1's summon model): `Villager.spawnGolemIfNeeded`
/// takes every villager inside the summoner's box grown by 10 on every axis, and every one of them
/// gets the summon mark (`GolemSensor.golemDetected`). For each summoner cell, every other villager
/// position whose box meets that box: every ridden cart's rider (its villager box, 0.6 x 1.95 at the
/// cart's y) and every rail cell of every part (the whole cell grown 0.3 sideways, 2.95 high: a
/// rider passing it, its centre anywhere over the cell, a curve's corner included (G1 on
/// 7eef80b12: the bare cell is up to 0.3 short there); conservative), each with its least axis gap
/// to the golem boxes at the pad's spots (feet cells in `golem`, a golem 1.4 x 2.7): 17.84 or less
/// on every axis means its own sensor box meets a golem (golem-locked while one exists); and the
/// reserved boxes as bounds. On-foot positions (the napper's walk) are not in the set.
pub fn summon_box(parts: &[Part], reserves: &[Reserve], summoners: &[P], golem: (P, P)) -> String {
    type B = ([f64; 3], [f64; 3]);
    let gap = |a: &B, b: &B| -> [f64; 3] { [0, 1, 2].map(|k| (a.0[k] - b.1[k]).max(b.0[k] - a.1[k]).max(0.0)) };
    let villager = |x: f64, y: f64, z: f64| -> B { ([x - 0.3, y, z - 0.3], [x + 0.3, y + 1.95, z + 0.3]) };
    let mut positions: Vec<(String, B, bool)> = Vec::new();
    for p in parts {
        for e in p.placed.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))) {
            positions.push((format!("rider ({:.2},{:.2},{:.2}) [{}]", e.pos[0], e.pos[1], e.pos[2], p.spec.name), villager(e.pos[0], e.pos[1], e.pos[2]), true));
        }
        for (c, st) in &p.placed.blocks {
            if rail_shape_of(st).is_some() {
                positions.push((format!("rail ({},{},{}) [{}]", c[0], c[1], c[2], p.spec.name), ([c[0] as f64 - 0.3, c[1] as f64, c[2] as f64 - 0.3], [c[0] as f64 + 1.3, c[1] as f64 + 2.95, c[2] as f64 + 1.3]), false));
            }
        }
    }
    let mut golems: Vec<B> = Vec::new();
    for x in golem.0[0]..=golem.1[0] {
        for y in golem.0[1]..=golem.1[1] {
            for z in golem.0[2]..=golem.1[2] {
                let (cx, cz) = (x as f64 + 0.5, z as f64 + 0.5);
                golems.push(([cx - 0.7, y as f64, cz - 0.7], [cx + 0.7, y as f64 + 2.7, cz + 0.7]));
            }
        }
    }
    let lock = |b: &B| -> (f64, bool) {
        let mut best = f64::MAX;
        let mut locked = false;
        for g in &golems {
            let gp = gap(b, g);
            let m = gp.iter().cloned().fold(0.0f64, f64::max);
            best = best.min(m);
            locked |= gp.iter().all(|v| *v <= 17.84);
        }
        (best, locked)
    };
    let mut s = format!("summon box (each summoner's villager box grown by 10 on every axis; every villager inside gets the summon mark): {} positions ({} riders, {} rail cells), golem spots x {}..{} y {}..{} z {}..{} (feet):\n", positions.len(), positions.iter().filter(|p| p.2).count(), positions.iter().filter(|p| !p.2).count(), golem.0[0], golem.1[0], golem.0[1], golem.1[1], golem.0[2], golem.1[2]);
    for sc in summoners {
        // the rider seated there, if the file has one in that cell, else a villager at the cell's
        // floor (a flat rail's cart y)
        let me = positions.iter().find(|p| p.2 && {
            let b = &p.1;
            (b.0[0] + 0.3).floor() as i32 == sc[0] && b.0[1].floor() as i32 == sc[1] && (b.0[2] + 0.3).floor() as i32 == sc[2]
        });
        let mb = me.map(|p| p.1).unwrap_or_else(|| villager(sc[0] as f64 + 0.5, sc[1] as f64 + 0.0625, sc[2] as f64 + 0.5));
        let inflated: B = ([mb.0[0] - 10.0, mb.0[1] - 10.0, mb.0[2] - 10.0], [mb.1[0] + 10.0, mb.1[1] + 10.0, mb.1[2] + 10.0]);
        let inside: Vec<&(String, B, bool)> = positions.iter().filter(|p| {
            let same = (p.1.0[1] - mb.0[1]).abs() < 1e-9 && (p.1.0[0] - mb.0[0]).abs() < 1e-9 && (p.1.0[2] - mb.0[2]).abs() < 1e-9;
            !same && gap(&p.1, &inflated).iter().all(|v| *v <= 0.0)
        }).collect();
        let riders = inside.iter().filter(|p| p.2).count();
        s.push_str(&format!("  summoner ({},{},{}): {} other positions in its box ({} riders, {} rail cells)\n", sc[0], sc[1], sc[2], inside.len(), riders, inside.len() - riders));
        for p in &inside {
            let (g, locked) = lock(&p.1);
            s.push_str(&format!("    {}: the least largest-axis gap to a golem box {g:.2}{}\n", p.0, if locked { " (golem-locked: its 17.84 box meets a golem)" } else { "" }));
        }
        for r in reserves {
            let rb: B = ([r.b.0[0] as f64, r.b.0[1] as f64, r.b.0[2] as f64], [r.b.1[0] as f64 + 1.0, r.b.1[1] as f64 + 1.0, r.b.1[2] as f64 + 1.0]);
            if gap(&rb, &inflated).iter().all(|v| *v <= 0.0) {
                s.push_str(&format!("    bound {} (reserved): its box meets the summon box\n", r.name));
            }
        }
    }
    s
}

/// A pocket chain (`--pockets NAME=x,y,z,DIR,N[,rstar]`; onechunk's lateral layout, Nov 11): N of
/// the stand's own pockets in a straight row heading DIR from the first pocket's P at (x,y,z), pitch
/// 5, net -1 a pocket: P a flat detector rail, R the rest slope (rising back toward P; a powered rail
/// with `rstar`, powered along the line from its G), G a powered slope (unpowered: the brake) under
/// an oak fence gate across the track, V a flat valley, U a slope up to the next pocket's P. Glass
/// under every rail (G's glass is its power source: the runner sets it to a redstone block while the
/// gate is open). Returns the part and its runner cells: NAME_Gk (gate/src), NAME_Pk and NAME_Rk.
pub fn pocket_part(name: &str, start: P, dir: &str, n: usize, rstar: bool) -> Result<(Part, Vec<(String, P, P)>, Vec<(String, P)>), String> {
    let (dx, dz, fwd, back, axis, facing) = match dir {
        "east" => (1, 0, "east", "west", "east_west", "east"),
        "west" => (-1, 0, "west", "east", "east_west", "west"),
        "south" => (0, 1, "south", "north", "north_south", "south"),
        "north" => (0, -1, "north", "south", "north_south", "north"),
        _ => return Err(format!("--pockets {name}: DIR is east, west, south or north")),
    };
    let mut d = Design { name: format!("pockets:{name}"), ..Default::default() };
    let mut gates = Vec::new();
    let mut cells = Vec::new();
    let at = |i: i32, y: i32| -> P { [start[0] + dx * i, y, start[2] + dz * i] };
    for k in 0..n as i32 {
        let y0 = start[1] - k;
        let b = 5 * k;
        let p = at(b, y0);
        let r = at(b + 1, y0 - 1);
        let g = at(b + 2, y0 - 2);
        let v = at(b + 3, y0 - 2);
        let u = at(b + 4, y0 - 2);
        // (the chain's last pocket ends at G: the caller's track continues from the next cell)
        let last = k + 1 == n as i32;
        let all: [(P, String); 5] = [
            (p, format!("minecraft:detector_rail[powered=false,shape={axis},waterlogged=false]")),
            (r, if rstar { format!("minecraft:powered_rail[powered=false,shape=ascending_{back},waterlogged=false]") } else { format!("minecraft:rail[shape=ascending_{back},waterlogged=false]") }),
            (g, format!("minecraft:powered_rail[powered=false,shape=ascending_{back},waterlogged=false]")),
            (v, format!("minecraft:rail[shape={axis},waterlogged=false]")),
            (u, format!("minecraft:rail[shape=ascending_{fwd},waterlogged=false]")),
        ];
        for (c, s) in all.into_iter().take(if last { 3 } else { 5 }) {
            d.blocks.insert(c, s);
            d.blocks.insert([c[0], c[1] - 1, c[2]], "minecraft:glass".into());
        }
        let gate = [g[0], g[1] + 1, g[2]];
        d.blocks.insert(gate, format!("minecraft:oak_fence_gate[facing={facing},in_wall=false,open=false,powered=false]"));
        let kk = k + 1;
        gates.push((format!("{name}_G{kk}"), gate, [g[0], g[1] - 1, g[2]]));
        cells.push((format!("{name}_P{kk}"), p));
        cells.push((format!("{name}_R{kk}"), r));
    }
    let spec = PartSpec { name: d.name.clone(), file: PathBuf::from("(pocket chain)"), base: None, frame: [0; 3], tier: [0; 3], keep: None, relay: true, nether: false };
    Ok((Part { spec, committed: d.clone(), placed: d, positions: Vec::new(), declared: Vec::new(), base_changed: Vec::new() }, gates, cells))
}

/// Single blocks (`--block x,y,z=STATE`): a part of its own (gates, detectors, powered rails, a
/// support) laid where no committed part has them.
pub fn block_part(blocks: &[(P, String)]) -> Part {
    block_part_named("blocks", "(--block)", blocks)
}

/// (vcompose) [`block_part`] under a name and a source (a `--delta`'s added cells, file named).
pub fn block_part_named(name: &str, source: &str, blocks: &[(P, String)]) -> Part {
    let mut d = Design { name: name.into(), ..Default::default() };
    for (c, s) in blocks {
        d.blocks.insert(*c, if s.contains(':') { s.clone() } else { format!("minecraft:{s}") });
    }
    let spec = PartSpec { name: name.into(), file: PathBuf::from(source), base: None, frame: [0; 3], tier: [0; 3], keep: None, relay: true, nether: false };
    Part { spec, committed: d.clone(), placed: d, positions: Vec::new(), declared: Vec::new(), base_changed: Vec::new() }
}

/// (vcompose) The --delta NAME=FILE files and --absent x,y,z=REASON lines as the command reads them
/// (G1 Nov 13 23:21 D5: the command's own path, so the tests run it): each delta's removed cells
/// one-cell clears that must take something out (an --absent line acknowledging one the parts don't
/// hold), its added cells a re-laying part, its `# +` header line; an --absent line naming no delta's
/// remove line refuses by name; a delta file must be UTF-8. Returns (clears, parts, header lines,
/// absents).
#[allow(clippy::type_complexity)]
pub fn delta_inputs(delta: &[String], absent: &[String]) -> Result<(Vec<Clear>, Vec<Part>, Vec<String>, Vec<(P, String)>), String> {
    let absents: Vec<(P, String)> = absent
        .iter()
        .map(|s| {
            let (c, why) = s.split_once('=').ok_or_else(|| format!("--absent {s}: x,y,z=REASON"))?;
            if why.trim().is_empty() {
                return Err(format!("--absent {s}: the reason is required"));
            }
            Ok((triple(c)?, why.trim().to_string()))
        })
        .collect::<Result<_, String>>()?;
    let mut used = vec![false; absents.len()];
    let (mut clears, mut parts, mut heads) = (Vec::new(), Vec::new(), Vec::new());
    for dl in delta {
        let (nm, file) = dl.split_once('=').ok_or_else(|| format!("--delta {dl}: NAME=FILE"))?;
        let bytes = std::fs::read(file).map_err(|e| format!("--delta {dl}: {e}"))?;
        // (G1 Nov 13 23:21: a delta file must be UTF-8; never read lossily)
        let text = String::from_utf8(bytes.clone()).map_err(|e| format!("--delta {dl}: not UTF-8: {e}"))?;
        let (adds, removes) = parse_delta(&text).map_err(|e| format!("--delta {dl}: {e}"))?;
        if adds.is_empty() && removes.is_empty() {
            return Err(format!("--delta {dl}: no cell in it (a no-op refuses by name)"));
        }
        for c in &removes {
            let ab = absents.iter().position(|(a, _)| a == c);
            if let Some(k) = ab {
                used[k] = true;
            }
            clears.push(Clear { b: (*c, *c), why: format!("--delta {nm}'s remove line"), must: true, absent: ab.map(|k| absents[k].1.clone()) });
        }
        if !adds.is_empty() {
            parts.push(block_part_named(nm, file, &adds));
        }
        heads.push(format!("# + {nm} (delta): {file} (md5 {}): {} cells added (a re-laying part), {} cells removed (one-cell clears)\n", redsim::stems::md5(&bytes).iter().map(|b| format!("{b:02x}")).collect::<String>(), adds.len(), removes.len()));
    }
    let unused: Vec<String> = absents.iter().zip(&used).filter(|(_, u)| !**u).map(|((c, _), _)| c3s(c)).collect();
    if !unused.is_empty() {
        return Err(format!("--absent names no delta remove line (a no-op refuses by name): {}", unused.join(" ")));
    }
    Ok((clears, parts, heads, absents))
}

/// (vcompose, ironspec Nov 14 00:34, G1 01:22) Every stand-in a composed record carries (its
/// `# stand-in:` lines and `emulated:` entries) names its support cells, `support=x,y,z[+x,y,z]`, or
/// `support=none` for a block that needs none (a free-standing redstone block): a named support the
/// record leaves empty (a clear or a drop took it), or a stand-in naming none, fails the compose by
/// name. `record`: the record's non-air cells in the written frame. Returns each stand-in checked.
pub fn stand_in_supports(heads: &[String], metas: &[(String, String)], record: &BTreeMap<P, String>) -> Result<Vec<String>, String> {
    let (mut ok, mut bad): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    // (G1 Nov 14 03:39a) the support token is the one right after the stand-in's cell (a
    // `# stand-in:` line) or its state (an `emulated:` entry, `x,y,z=STATE support=...`), never a
    // `support=` met later in the prose; `support=none` holds only for a block the record holds at the
    // cell and that needs no support
    let mut check = |what: String, cell: Option<P>, token: Option<&str>| {
        let Some(v) = token.and_then(|t| t.strip_prefix("support=")) else {
            bad.push(format!("{what}: names no support right after its cell (support=x,y,z or support=none)"));
            return;
        };
        let v = v.trim_end_matches([';', ')', ',']);
        if v == "none" {
            match cell.and_then(|c| record.get(&c)) {
                Some(st) if !needs_support(st) => ok.push(format!("{what}: support=none ({st} needs none)")),
                Some(st) => bad.push(format!("{what}: support=none, but {st} needs a support: name it")),
                None => bad.push(format!("{what}: support=none, but the record holds no block there to judge: name its support")),
            }
            return;
        }
        let mut missing = Vec::new();
        for c in v.split('+') {
            match triple(c) {
                Ok(p) if record.contains_key(&p) => {}
                Ok(p) => missing.push(c3s(&p)),
                Err(e) => {
                    bad.push(format!("{what}: support={v}: {e}"));
                    return;
                }
            }
        }
        if missing.is_empty() {
            ok.push(format!("{what}: support={v} held"));
        } else {
            bad.push(format!("{what}: its support {} is empty in the record", missing.join(" + ")));
        }
    };
    for h in heads {
        if let Some(rest) = h.strip_prefix("# stand-in:") {
            let mut it = rest.split_whitespace();
            let cell = it.next().unwrap_or("").to_string();
            check(format!("stand-in {cell}"), triple(&cell).ok(), it.next());
        }
    }
    for (k, v) in metas.iter().filter(|(k, _)| k == "emulated") {
        for e in crate::pushscan::header_entries(&format!("# {k}: {v}"), "emulated:") {
            let (cell, rest) = e.split_once('=').unwrap_or((e.as_str(), ""));
            let cell = cell.trim().to_string();
            check(format!("emulated {cell}"), triple(&cell).ok(), rest.split_whitespace().nth(1));
        }
    }
    if bad.is_empty() {
        Ok(ok)
    } else {
        Err(format!("STAND-IN SUPPORTS (ironspec Nov 14 00:34, G1 01:22 / 03:39a: a stand-in names its support cells right after its cell, support=x,y,z or support=none; a removal taking one fails by name): {}", bad.join("; ")))
    }
}

/// (vcompose, G1 Nov 14 03:39a) Whether a block state needs a block to stand on or hang from: the
/// attached and floor-standing kinds (levers, buttons, torches, rails, redstone dust, repeaters,
/// comparators, tripwire hooks and string, pressure plates, carpets, ladders, doors, signs, banners,
/// lanterns); a full block or a redstone block needs none.
pub fn needs_support(state: &str) -> bool {
    let n = state.trim_start_matches("minecraft:");
    let n = n.split('[').next().unwrap_or(n);
    matches!(n, "lever" | "redstone_wire" | "repeater" | "comparator" | "tripwire" | "tripwire_hook" | "ladder" | "lantern" | "soul_lantern" | "bell" | "rail")
        || n.contains("torch")
        || ["_button", "_rail", "_pressure_plate", "carpet", "_sign", "_banner", "candle"].iter().any(|s| n.ends_with(s))
        || n.ends_with("_door") && !n.ends_with("trapdoor")
}

/// (vcompose) The --precedence x,y,z=PART:REASON lines as the command reads them (the reason
/// required).
pub fn parse_precedence(lines: &[String]) -> Result<Vec<(P, String, String)>, String> {
    lines
        .iter()
        .map(|s| {
            let (c, rest) = s.split_once('=').ok_or_else(|| format!("--precedence {s}: x,y,z=PART:REASON"))?;
            let (part, why) = rest.split_once(':').ok_or_else(|| format!("--precedence {s}: x,y,z=PART:REASON (the reason is required)"))?;
            if why.trim().is_empty() {
                return Err(format!("--precedence {s}: the reason is required"));
            }
            Ok((triple(c)?, part.trim().to_string(), why.trim().to_string()))
        })
        .collect()
}

/// (vcompose) A cell delta file (b8949c96's form): each `x,y,z STATE` line an added cell, each bare
/// `x,y,z` line a removed cell, `#` lines and blank lines skipped; anything else refuses by line.
pub fn parse_delta(text: &str) -> Result<(Vec<(P, String)>, Vec<P>), String> {
    let mut add_cells: Vec<(P, String)> = Vec::new();
    let mut remove: Vec<P> = Vec::new();
    for (n, l) in text.lines().enumerate() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let (c, rest) = t.split_once(char::is_whitespace).map(|(a, b)| (a, b.trim())).unwrap_or((t, ""));
        let p = triple(c).map_err(|e| format!("delta line {}: {t:?}: {e}", n + 1))?;
        if rest.is_empty() {
            remove.push(p);
        } else if rest.contains(char::is_whitespace) {
            return Err(format!("delta line {}: {t:?}: one state after the cell", n + 1));
        } else {
            add_cells.push((p, rest.to_string()));
        }
    }
    let dup: Vec<String> = add_cells.iter().filter(|(c, _)| remove.contains(c)).map(|(c, _)| c3s(c)).collect();
    if !dup.is_empty() {
        return Err(format!("delta: cells both added and removed: {}", dup.join(" ")));
    }
    Ok((add_cells, remove))
}

/// The waiting positions' clearance from the pad (onechunk's lateral layout, ironspec's worst-case
/// constants Nov 11 21:00): every rail cell inside a `--wait-box` is a waiting rider's place (its
/// feet on the cart's path through the cell); against every golem spawn cell in `golem` (feet, the
/// golem centred on its cell): inside the sensor box when |dx| and |dz| are both < 18.84 and the
/// golem's feet are < 20.54 below or < 19.79 above the rider's. Lists every waiting place inside,
/// and per axis the least margin of the places outside (how close to the box each comes).
pub fn waiting_check(d: &Design, boxes: &[(P, P)], golem: (P, P), shift: P) -> (String, bool) {
    let mut inside: Vec<String> = Vec::new();
    let mut n = 0usize;
    let mut best: [f64; 3] = [f64::MAX; 3];
    for (c, st) in &d.blocks {
        let Some(shape) = rail_shape_of(st) else { continue };
        if !boxes.iter().any(|b| inside_box(c, b)) {
            continue;
        }
        n += 1;
        // (the rider's feet over the cell: a flat's cart y, a slope's low to high end)
        let (lo, hi) = if shape.starts_with("ascending") { (c[1] as f64 + 0.0625, c[1] as f64 + 1.0625) } else { (c[1] as f64 + 0.0625, c[1] as f64 + 0.0625) };
        let (px, pz) = (c[0] as f64 + 0.5, c[2] as f64 + 0.5);
        let gx = px.clamp(golem.0[0] as f64 + 0.5, golem.1[0] as f64 + 0.5);
        let gz = pz.clamp(golem.0[2] as f64 + 0.5, golem.1[2] as f64 + 0.5);
        let gy_lo = golem.0[1] as f64;
        let gy_hi = golem.1[1] as f64;
        let mx = (px - gx).abs() - 18.84;
        let mz = (pz - gz).abs() - 18.84;
        // (vertical: the rider's lowest feet against the highest golem feet, and the reverse)
        let my = (lo - gy_hi - 20.54).max(gy_lo - hi - 19.79);
        if mx < 0.0 && mz < 0.0 && my < 0.0 {
            inside.push(format!("({},{},{})", c[0] + shift[0], c[1] + shift[1], c[2] + shift[2]));
        } else {
            for (k, m) in [mx, my, mz].iter().enumerate() {
                if *m >= 0.0 {
                    best[k] = best[k].min(*m);
                }
            }
        }
    }
    let fmt = |v: f64| if v == f64::MAX { "-".to_string() } else { format!("{v:.2}") };
    let ok = inside.is_empty();
    (
        format!(
            "waiting places (rail cells in the wait boxes, written frame): {n}; inside a pad golem's sensor box (|dx|, |dz| < 18.84, feet -20.54..+19.79): {}{}; the least margin outside, by the axis that clears it: x {} y {} z {}\n",
            inside.len(),
            if ok { " (PASS)".to_string() } else { format!(" (FAIL): {}", inside.iter().take(12).cloned().collect::<Vec<_>>().join(" ")) },
            fmt(best[0]),
            fmt(best[1]),
            fmt(best[2])
        ),
        ok,
    )
}

fn inside_box(c: &P, b: &(P, P)) -> bool {
    (0..3).all(|k| c[k] >= b.0[k] && c[k] <= b.1[k])
}

/// The sentinel villager's room (b8949c96's rule (c), Nov 11): its box `s` (cells, inclusive) must
/// stay out of every villager position's box grown by 10 (`Villager.spawnGolemIfNeeded`'s
/// `inflate(10)`), so some axis gap of at least 10, ironspec's bar 11.7. Every rail cell of every
/// part and every ridden cart is a villager position, its box the whole cell, 2 high (a rider or a
/// villager on foot there, conservative). Prints the least of the largest axis gap over all
/// positions, and the room's and the bed's chunk sections in the written frame (`shift`, the
/// aligned frame: sections every 16 from 0; y sections depend on the site).
pub fn sentinel(parts: &[Part], s: (P, P), bed: Option<P>, shift: P) -> (String, bool) {
    let (slo, shi) = ([s.0[0] as f64, s.0[1] as f64, s.0[2] as f64], [s.1[0] as f64 + 1.0, s.1[1] as f64 + 1.0, s.1[2] as f64 + 1.0]);
    let mut best: Option<(f64, String)> = None;
    let mut consider = |lo: [f64; 3], hi: [f64; 3], what: String| {
        let gap = (0..3).map(|k| (slo[k] - hi[k]).max(lo[k] - shi[k]).max(0.0)).fold(0.0f64, f64::max);
        if best.as_ref().is_none_or(|(g, _)| gap < *g) {
            best = Some((gap, what));
        }
    };
    for p in parts {
        for (c, st) in &p.placed.blocks {
            if rail_shape_of(st).is_some() {
                consider([c[0] as f64, c[1] as f64, c[2] as f64], [c[0] as f64 + 1.0, c[1] as f64 + 2.0, c[2] as f64 + 1.0], format!("{}'s rail ({},{},{})", p.spec.name, c[0], c[1], c[2]));
            }
        }
        for e in p.placed.entities.iter().filter(|e| e.kind.ends_with("minecart") && e.nbt.as_deref().is_some_and(|t| t.contains("villager"))) {
            consider([e.pos[0] - 0.5, e.pos[1], e.pos[2] - 0.5], [e.pos[0] + 0.5, e.pos[1] + 2.0, e.pos[2] + 0.5], format!("{}'s rider at ({:.2},{:.2},{:.2})", p.spec.name, e.pos[0], e.pos[1], e.pos[2]));
        }
    }
    let bar = 11.7;
    let sec = |c: P| [(c[0] + shift[0]).div_euclid(16), (c[2] + shift[2]).div_euclid(16)];
    let mut out = format!("sentinel room x {}..{} y {}..{} z {}..{}: ", s.0[0], s.1[0], s.0[1], s.1[1], s.0[2], s.1[2]);
    let ok = match &best {
        Some((g, at)) => {
            out.push_str(&format!("(c) the least largest-axis gap to a villager position's box {g:.2} (at {at}; > 10 keeps it out of the summon box, the bar {bar}: {})", if *g >= bar { "ok" } else { "FAIL" }));
            *g >= bar
        }
        None => {
            out.push_str("(c) no villager position");
            true
        }
    };
    let ss = sec(s.0);
    out.push_str(&format!("; its chunk section (x, z) in the written frame ({}, {})", ss[0], ss[1]));
    if let Some(b) = bed {
        let bs = sec(b);
        out.push_str(&format!(", the shared bed's ({}, {}): {}", bs[0], bs[1], if (ss[0] - bs[0]).abs().max((ss[1] - bs[1]).abs()) <= 1 { "adjacent or the same (rule (a)'s fallback)" } else { "NOT adjacent" }));
    }
    out.push('\n');
    (out, ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ironspec 20:19 PT (2a), G1) the T-reach scan's sources are redsim's predicate: spot pins
    /// (the jukebox and the lightning rods are sources, a trapped chest and a lectern are refused
    /// at the parse, dust counts) and agreement with `behavior::is_signal_source` on named blocks
    #[test]
    fn the_source_names_follow_the_predicate() {
        assert!(is_source_name("minecraft:jukebox"));
        assert!(is_source_name("lightning_rod"));
        assert!(is_source_name("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"));
        assert!(!is_source_name("minecraft:trapped_chest[facing=north,type=single,waterlogged=false]"));
        assert!(!is_source_name("lectern"));
        assert!(!is_source_name("glass"));
        let w = redsim::world::World::new(redsim::pos::BlockPos::new(0, 0, 0), [1, 1, 1]);
        for n in ["lever", "stone_button", "repeater", "comparator", "observer", "target", "detector_rail", "daylight_detector", "redstone_block", "redstone_torch", "sculk_sensor", "stone_pressure_plate", "tripwire_hook", "rail", "powered_rail", "hopper", "piston", "slime_block", "iron_trapdoor", "note_block", "redstone_lamp"] {
            let Ok(b) = redsim::block::Block::parse(&format!("minecraft:{n}")) else { continue };
            assert_eq!(is_source_name(n), redsim::behavior::is_signal_source(&w, &b), "{n}");
        }
    }

    #[test]
    fn sections_per_phase() {
        // a rail at y 0 and a bed at y 40 in the same column: 2 or 3 sections apart by the phase
        let mut d = Design::default();
        d.blocks.insert([0, 0, 0], "minecraft:rail[shape=north_south,waterlogged=false]".into());
        let rep = section_report(&d, &[[0, 40, 0]], [0, 0, 0]);
        assert!(rep.contains("p  0: 1 of 1 beyond 1 section (the farthest 2 sections)"), "{rep}");
        assert!(rep.contains("p  8: 1 of 1 beyond 1 section (the farthest 3 sections)"), "{rep}");
        let near = section_report(&d, &[[0, 40, 0], [0, 10, 0]], [0, 0, 0]);
        assert!(near.contains("p  0: 0 of 1"), "{near}");
    }

    #[test]
    fn sentinel_gap() {
        let t = sketch_part(&[("t".into(), vec![[0, 0, 0], [0, 0, 3]], [false, false])], &[], &[]).unwrap();
        // a room 12 east of the rails' cells (x 0..0 -> the cell box ends at 1): gap 12 - 1 = 11
        let (rep, ok) = sentinel(&t, ([12, 0, 0], [13, 1, 2]), Some([0, 0, 1]), [6, 0, 6]);
        assert!(!ok && rep.contains("gap to a villager position's box 11.00") && rep.contains("FAIL"), "{rep}");
        let (rep2, ok2) = sentinel(&t, ([13, 0, 0], [14, 1, 2]), Some([0, 0, 1]), [6, 0, 6]);
        assert!(ok2 && rep2.contains("12.00") && rep2.contains("section (x, z) in the written frame (1, 0)") && rep2.contains("adjacent"), "{rep2}");
    }

    #[test]
    fn slope_support() {
        // a slope rising west on stone: popped without a block on its high side (the snap turn
        // v4's kept box cut), kept with one; a boosted sketch track lays its straight cells
        // powered on redstone blocks, its curves plain
        let mut d = Design { name: "s".into(), ..Default::default() };
        d.blocks.insert([0, 1, 0], "minecraft:rail[shape=ascending_west,waterlogged=false]".into());
        d.blocks.insert([0, 0, 0], "minecraft:stone".into());
        let pop = slope_supports(&d);
        assert!(pop.len() == 1 && pop[0].contains("high side (-1,1,0) air"), "{pop:?}");
        d.blocks.insert([-1, 1, 0], "minecraft:glass".into());
        assert!(slope_supports(&d).is_empty());
        d.blocks.insert([-1, 1, 0], "minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]".into());
        assert_eq!(slope_supports(&d).len(), 1);
        let t = sketch_part(&[("+b".into(), vec![[0, 0, 0], [0, 0, 1], [2, 1, 1]], [false, false])], &[], &[]).unwrap();
        let b = &t[0].placed.blocks;
        assert!(b[&[0, 0, 0]].starts_with("minecraft:powered_rail[powered=true,shape=north_south") && b[&[0, -1, 0]] == "minecraft:redstone_block", "{b:?}");
        assert!(b[&[0, 0, 1]].starts_with("minecraft:rail[shape=") && b[&[0, -1, 1]] == "minecraft:glass", "{b:?}");
        assert!(b[&[2, 1, 1]].starts_with("minecraft:powered_rail[powered=true,shape=ascending_east") || b[&[2, 1, 1]].starts_with("minecraft:powered_rail[powered=true,shape=east_west"), "{b:?}");
    }

    #[test]
    fn sketch_tracks() {
        // down 2 along -z (dropping first), then a corner east, flat
        let cells = track_cells(&[[0, 5, 4], [0, 3, 1], [2, 3, 1]]).unwrap();
        assert_eq!(cells, vec![[0, 5, 4], [0, 4, 3], [0, 3, 2], [0, 3, 1], [1, 3, 1], [2, 3, 1]]);
        // up 2 along +x after a corner: the first cell keeps the corner's height (rising last)
        assert_eq!(track_cells(&[[0, 0, 0], [0, 0, 1], [3, 2, 1]]).unwrap(), vec![[0, 0, 0], [0, 0, 1], [1, 0, 1], [2, 1, 1], [3, 2, 1]]);
        // a curve that would have to slope is refused
        assert!(sketch_part(&[("t".into(), vec![[0, 0, 0], [0, 0, 1], [1, 1, 1]], [false, false])], &[], &[]).is_err());
        // context ends: not laid, but they shape their neighbours (a slope continuing another
        // part's drop, a curve turning into another part's rail)
        let q = sketch_part(&[("c".into(), vec![[0, 9, 0], [0, 7, 2], [0, 7, 3], [2, 7, 3], [2, 7, 2]], [true, true])], &[], &[]).unwrap();
        assert_eq!(q.len(), 1, "one track, no carts or posts: one part");
        let q = &q[0];
        assert_eq!(q.spec.name, "track:c");
        assert!(!q.placed.blocks.contains_key(&[0, 9, 0]) && !q.placed.blocks.contains_key(&[2, 7, 2]));
        assert_eq!(q.placed.blocks[&[0, 8, 1]], "minecraft:rail[shape=ascending_north,waterlogged=false]");
        assert_eq!(q.placed.blocks[&[0, 7, 3]], "minecraft:rail[shape=north_east,waterlogged=false]");
        assert_eq!(q.placed.blocks[&[2, 7, 3]], "minecraft:rail[shape=north_west,waterlogged=false]");
        // the bed radius: the least over the cart's path (a flat rail's path at y + 0.0625), a
        // rider off rails, an exempt part printed but not judged, a reserved box as a bound
        let flat = sketch_part(&[("f".into(), vec![[0, 0, 0], [4, 0, 0]], [false, false])], &[], &[]).unwrap();
        let (rep, ok) = bed_radius(&flat, &[], [2, 10, 0], &[]);
        assert!(!ok && rep.contains("FAIL track:f: least 10.44 at rail (2,0,0) east_west (6.56 short)"), "{rep}");
        let (rep2, ok2) = bed_radius(&flat, &[Reserve::parse("r=0,30,0,1,31,1").unwrap()], [2, 10, 0], &["track:f".to_string()]);
        assert!(ok2 && rep2.contains("EXEMPT (the napper's own path) track:f") && rep2.contains("bound r (reserved): its box comes within 19.51"), "{rep2}");
        let far = sketch_part(&[], &[([0, 40, 0], Some(40.0), false)], &[]).unwrap();
        let (rep3, ok3) = bed_radius(&far, &[], [0, 10, 0], &[]);
        assert!(ok3 && rep3.contains("ok   sketch: least 29.50 at rider"), "{rep3}");
        let p = sketch_part(&[("t".into(), vec![[0, 5, 4], [0, 3, 1], [2, 3, 1]], [false, false])], &[([0, 3, 2], None, false), ([7, 7, 7], Some(7.18), true)], &[[5, 3, 5]]).unwrap();
        assert_eq!(p.len(), 2);
        let mut d = p[0].placed.clone();
        d.blocks.extend(p[1].placed.blocks.clone());
        d.entities.extend(p[1].placed.entities.clone());
        assert_eq!(d.blocks[&[0, 4, 3]], "minecraft:rail[shape=ascending_south,waterlogged=false]");
        assert_eq!(d.blocks[&[0, 3, 2]], "minecraft:rail[shape=ascending_south,waterlogged=false]");
        assert_eq!(d.blocks[&[0, 3, 1]], "minecraft:rail[shape=south_east,waterlogged=false]");
        assert_eq!(d.blocks[&[2, 3, 1]], "minecraft:rail[shape=east_west,waterlogged=false]");
        assert_eq!(d.blocks[&[0, 2, 1]], "minecraft:glass");
        assert_eq!(d.blocks[&[5, 2, 5]], "minecraft:glass");
        assert!(d.blocks[&[5, 6, 5]].contains("oak_leaves"));
        assert_eq!(d.entities.len(), 3);
        assert_eq!(d.entities[0].pos, [0.5, 3.5, 2.5]);
        assert_eq!(d.entities[1].pos, [7.5, 7.18, 7.5]);
        assert!(d.entities[1].nbt.is_none());
        assert!(track_cells(&[[0, 0, 0], [1, 0, 1]]).is_err());
        assert!(track_cells(&[[0, 0, 0], [0, 3, 2]]).is_err());
        assert!(sketch_part(&[("t".into(), vec![[0, 0, 0], [0, 0, 2]], [false, false]), ("u".into(), vec![[0, 1, 1], [1, 1, 1]], [false, false])], &[], &[]).is_err(), "a rail on another rail");
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;

    /// (onechunk Nov 13; ironspec 09:43 / 09:48 / 09:55 / 09:57, G1 09:45 / 09:54) a composed record
    /// carries every meta line of its parts: a part with one of each kind composed twice (A at its
    /// place, B moved 32 east, both raised by a tier step and the output shift), every declaration
    /// on both copies at its moved cell, a reason holding `;` kept whole, an entry outside the
    /// part's `#` box dropped, a scalar once, a prose note ignored; an unknown meta line, an unknown
    /// `# key:` declaration and a scalar that differs between parts each FAIL the compose
    #[test]
    fn a_composed_record_carries_its_parts_declarations() {
        let dir = std::env::temp_dir().join(format!("carry-decl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("part.txt");
        let lines = [
            "name: part",
            "circuit: mob-farm iron_ingot",
            "dwell-cap: 15 (provisional)",
            "k-lock: 42",
            "k-sw: 6",
            "emulated: 7,-2,8=glass (the hold input's support at (7,-1,8))",
            "keep-block: -5,-4,3=oak_leaves (cover)",
            "keep-clear: -3,-6,3 -3,-5,3 (the bed's column)",
            "keep-out: 10,-2,3 10,-2,4 (the swing)",
            "placement-order-ts: -1,0,7=the merge T (never switched; placed north_west after (-1,0,6)); 3,0,7=the other T",
            "update-only-ts: 1,5,9+5,5,9=the K junctions",
            "unreachable: -4,-6,4=a lever cell",
            "waker-places: -3,-7,3=asleep in the bed",
            "riderless: 1,-6,-5+0,-7,-5=the empty cart",
            "run-cells: P0 -1,0,0; D0 -1,0,2",
            "run-gates: G0 1,0,1/1,0,2",
            "# stand-in: -1,6,9 (J(-1)'s lever)",
            "# contacts: -1,4,5+3,4,5=the backstop",
            "# shape: 1,0,7=rail[shape=north_west] (build step: after the north arm)",
            "# switch: 9,0,9=its lever; 30,0,30=far away",
            "# switches: -4,-7,2=a hatch lever",
            "# update-only: -1,4,8=the branch detector",
            "# travel: -1,5,9+-1,4,8=the batch chain",
            "# always-on: -2,0,5=torch on glass",
            "# out-of-scope: 2,2,2=a far cell",
            "# water: -5,3,7=designed current",
            "# single-occupant: 9,0,8+9,0,7=one napper",
            "# own-route-junctions: -2,4,7=c3",
            "# one-way: -3,0,0=the step dust",
            "# a6-pickup: -2,-7,3=the pickup cell",
            "# stand-in-guard: -1,6,9=golem PASS",
            "# candidate: 4,4,4=a candidate",
            "# day-time: 18000",
            "# placement order: (-1,0,7) placed last",
            "# a note: see (1,0,7) for the T",
        ];
        std::fs::write(&f, lines.join("\n") + "\n").unwrap();
        let a = PartSpec::parse(&format!("A={}^0,1,0#-10,-10,-10,12,12,12", f.display())).unwrap();
        let b = PartSpec::parse(&format!("B={}@32,0,0^0,1,0#-10,-10,-10,12,12,12", f.display())).unwrap();
        let (heads, metas) = carried_declarations(&[a, b], [0, 0, 5]).unwrap();
        let all: String = heads.iter().map(|h| format!("{h}\n")).chain(metas.iter().map(|(k, v)| format!("{k}: {v}\n"))).collect();
        // every declaration on both copies, at its cell + (0,1,5) for A and + (32,1,5) for B
        for (part, dx) in [("A", 0), ("B", 32)] {
            let m = |x: i32, y: i32, z: i32| format!("{},{},{}", x + dx, y + 1, z + 5);
            let tag = format!("(carried from part {part}, moved by {dx},1,5)");
            for want in [
                format!("{}=glass (the hold input's support at (7,-1,8)) {tag}", m(7, -2, 8)),
                format!("{}=oak_leaves (cover) {tag}", m(-5, -4, 3)),
                format!("{} {} (the bed's column) {tag}", m(-3, -6, 3), m(-3, -5, 3)),
                format!("{} {} (the swing) {tag}", m(10, -2, 3), m(10, -2, 4)),
                format!("{}=the merge T (never switched; placed north_west after (-1,0,6)) {tag}", m(-1, 0, 7)),
                format!("{}=the other T {tag}", m(3, 0, 7)),
                format!("{}+{}=the K junctions {tag}", m(1, 5, 9), m(5, 5, 9)),
                format!("{}=a lever cell {tag}", m(-4, -6, 4)),
                format!("{}=asleep in the bed {tag}", m(-3, -7, 3)),
                format!("{}+{}=the empty cart {tag}", m(1, -6, -5), m(0, -7, -5)),
                format!("P0 {}", m(-1, 0, 0)),
                format!("D0 {}", m(-1, 0, 2)),
                format!("G0 {}/{}", m(1, 0, 1), m(1, 0, 2)),
                format!("# stand-in: {} (J(-1)'s lever) {tag}", m(-1, 6, 9)),
                format!("# contacts: {}+{}=the backstop {tag}", m(-1, 4, 5), m(3, 4, 5)),
                format!("# shape: {}=rail[shape=north_west] (build step: after the north arm) {tag}", m(1, 0, 7)),
                format!("# switch: {}=its lever {tag}", m(9, 0, 9)),
                format!("# switches: {}=a hatch lever {tag}", m(-4, -7, 2)),
                format!("# update-only: {}=the branch detector {tag}", m(-1, 4, 8)),
                format!("# travel: {}+{}=the batch chain {tag}", m(-1, 5, 9), m(-1, 4, 8)),
                format!("# always-on: {}=torch on glass {tag}", m(-2, 0, 5)),
                format!("# out-of-scope: {}=a far cell {tag}", m(2, 2, 2)),
                format!("# water: {}=designed current {tag}", m(-5, 3, 7)),
                format!("# single-occupant: {}+{}=one napper {tag}", m(9, 0, 8), m(9, 0, 7)),
                format!("# own-route-junctions: {}=c3 {tag}", m(-2, 4, 7)),
                format!("# one-way: {}=the step dust {tag}", m(-3, 0, 0)),
                format!("# a6-pickup: {}=the pickup cell {tag}", m(-2, -7, 3)),
                format!("# stand-in-guard: {}=golem PASS {tag}", m(-1, 6, 9)),
                format!("# candidate: {}=a candidate {tag}", m(4, 4, 4)),
            ] {
                assert!(all.contains(&want), "missing {want:?} in:\n{all}");
            }
            assert!(!all.contains(&m(30, 0, 30)), "the entry outside the box is dropped");
        }
        // a scalar once; the composer's own lines and a prose note not carried
        for (want, n) in [("circuit: mob-farm iron_ingot\n", 1), ("dwell-cap: 15 (provisional)\n", 1), ("k-lock: 42\n", 1), ("k-sw: 6\n", 1), ("# day-time: 18000\n", 1), ("name:", 0), ("placement order", 0), ("a note", 0)] {
            assert_eq!(all.matches(want).count(), n, "{want:?} in:\n{all}");
        }
        // an unknown meta line, an unknown declaration and a differing scalar fail the compose
        for (extra, why) in [("bar: 7", "meta line `bar:`"), ("# foo: 1,2,3=x", "declaration `# foo:`")] {
            let g = dir.join("bad.txt");
            std::fs::write(&g, format!("{}\n{extra}\n", lines.join("\n"))).unwrap();
            let e = carried_declarations(&[PartSpec::parse(&format!("A={}", g.display())).unwrap()], [0, 0, 0]).unwrap_err();
            assert!(e.contains(why), "{e}");
        }
        let h = dir.join("other.txt");
        std::fs::write(&h, "k-lock: 40\n").unwrap();
        let e = carried_declarations(&[PartSpec::parse(&format!("A={}", f.display())).unwrap(), PartSpec::parse(&format!("C={}", h.display())).unwrap()], [0, 0, 0]).unwrap_err();
        assert!(e.contains("the scalar `k-lock` differs"), "{e}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn part_spec_parses() {
        let p = PartSpec::parse("stand=/tmp/a b.txt@-6,0,-6^0,-3,1#0,-20,0,20,10,-4").unwrap();
        assert_eq!(p.name, "stand");
        assert_eq!(p.file, PathBuf::from("/tmp/a b.txt"));
        assert_eq!(p.frame, [-6, 0, -6]);
        assert_eq!(p.tier, [0, -3, 1]);
        assert_eq!(p.keep, Some(([0, -20, -4], [20, 10, 0])));
        let q = PartSpec::parse("r1=x.txt").unwrap();
        assert_eq!((q.frame, q.tier, q.keep, q.base.clone()), ([0; 3], [0; 3], None, None));
        let w = PartSpec::parse("wayin=w.txt%m.txt#9,-25,-5,9,25,9").unwrap();
        assert_eq!((w.file, w.base), (PathBuf::from("w.txt"), Some(PathBuf::from("m.txt"))));
        assert!(PartSpec::parse("x.txt").is_err());
        assert!(PartSpec::parse("a=x.txt#1,2,3").is_err());
    }

    #[test]
    fn chunk_lines_and_overhang() {
        // the loop frame: lines at -6 + 16k (cells -7|-6 and 9|10)
        assert_eq!(lines_crossed(-6, 9, -6), Vec::<i32>::new());
        assert_eq!(lines_crossed(-7, 10, -6), vec![-6, 10]);
        assert_eq!(lines_crossed(0, 25, -6), vec![10]);
        let f = Frame { x0: -6, z0: -6, x1: 9, z1: 9, phase_x: -6, phase_z: -6 };
        assert_eq!(overhang(&([-6, 0, -6], [9, 5, 9]), &f), None);
        assert_eq!(overhang(&([-8, 0, -6], [9, 5, 12]), &f).as_deref(), Some("2 west, 3 south"));
    }

    #[test]
    fn kept_box_and_skeleton() {
        let text = "name: t\ncircuit: mob-farm iron_ingot\npositions: 0,0,0 5,0,0\nport output: 0 0 0\nentity: minecraft:minecart 0.5 0 0.5 {Passengers:[{id:\"minecraft:villager\"}]}\nentity: minecraft:minecart 5.5 0 0.5\npalette:\n  S stone\nlayer -1:\n  SSSSSS\n";
        let d = Design::parse(text).unwrap();
        let k = kept(&d, Some(([0, -1, 0], [2, 0, 0]))).unwrap();
        assert_eq!(k.blocks.len(), 3);
        assert_eq!(k.entities.len(), 1);
        assert_eq!(k.meta["positions"], "0,0,0");
        assert!(k.ports.contains_key("output"));
        let dir = std::env::temp_dir().join(format!("column-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f1 = dir.join("a.txt");
        std::fs::write(&f1, text).unwrap();
        let specs = vec![
            PartSpec::parse(&format!("a={}@1,0,1", f1.display())).unwrap(),
            PartSpec::parse(&format!("b={}@1,0,1^0,-5,0#3,-1,0,5,0,0", f1.display())).unwrap(),
        ];
        let f = Frame { x0: -6, z0: -6, x1: 9, z1: 9, phase_x: -6, phase_z: -6 };
        let (rep, sk) = run(&specs, Vec::new(), &[], &[], &[("output".into(), [1, 0, 1])], &f, "sk", true).unwrap();
        assert!(rep.contains("clashes: none"), "{rep}");
        assert_eq!(sk.blocks.len(), 9);
        assert_eq!(sk.ports["output"], [1, 0, 1]);
        assert!(!sk.ports.contains_key("b.output"), "b's output lies outside its kept box");
        assert_eq!(sk.meta["positions"], "1,0,1 6,0,1 6,-5,1");
        // two parts holding the same cells with the same states: kept once and counted (vcompose:
        // a different state fails the compose, `traced_tests`); a reserved box over a part's blocks
        // fails the compose by name (G1 Nov 13 23:21)
        let specs2 = vec![PartSpec::parse(&format!("a={}", f1.display())).unwrap(), PartSpec::parse(&format!("b={}", f1.display())).unwrap()];
        let (rep2, sk2) = run(&specs2, Vec::new(), &[], &[], &[], &f, "sk", false).unwrap();
        assert!(rep2.contains("held twice with the same state (kept once): a and b: 8 cells"), "{rep2}");
        assert_eq!(sk2.entities.len(), 2, "the same entities in two parts are kept once");
        let e2 = run(&specs2, Vec::new(), &[], &[Reserve::parse("fold=0,-1,0,1,0,0").unwrap(), Reserve::parse("run=1,-1,0,1,-1,0").unwrap()], &[], &f, "sk", false).unwrap_err();
        assert!(e2.contains("CLASH") && e2.contains("reserved fold (owner and item not named), held by a"), "{e2}");
        // (G1 Nov 14 03:39a: the count the per-cell form kept) the fold box holds 2 of a's cells
        assert_eq!(e2.matches("(reserved fold (owner and item not named), held by a:").count(), 2, "{e2}");
        assert!(e2.contains("fold and run (both reserved)"), "{e2}");
        // declared states move with their part (the control block too); two ways is reported
        let decl = "name: d\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\ndeclared: 0,0,0 powered_rail[powered=false]<0,-1,0\npalette:\n  S stone\nlayer -1:\n  S\n";
        let decl2 = decl.replace("powered=false", "powered=true");
        let (fa, fb) = (dir.join("d1.txt"), dir.join("d2.txt"));
        std::fs::write(&fa, decl).unwrap();
        std::fs::write(&fb, decl2).unwrap();
        let (_, sk3) = run(&[PartSpec::parse(&format!("a={}@2,3,4", fa.display())).unwrap()], Vec::new(), &[], &[], &[("output".into(), [9, 9, 9])], &f, "sk", false).unwrap();
        assert_eq!(sk3.meta["declared"], "2,3,4 powered_rail[powered=false]<2,2,4");
        assert_eq!(sk3.ports["output"], [9, 9, 9], "a given port replaces the part's");
        let (rep4, _) = run(&[PartSpec::parse(&format!("a={}", fa.display())).unwrap(), PartSpec::parse(&format!("b={}^0,0,0", fb.display())).unwrap()], Vec::new(), &[], &[], &[], &f, "sk", false).unwrap();
        assert!(rep4.contains("DECLARED TWO WAYS (0,0,0) declared two ways"), "{rep4}");
        // a cleared box takes a file part's blocks (and its declared cells) out first
        let (rep5, sk5) = run(&[PartSpec::parse(&format!("a={}", f1.display())).unwrap()], Vec::new(), &[([0, -1, 0], [2, -1, 0])], &[], &[("output".into(), [9, 9, 9])], &f, "sk", false).unwrap();
        assert_eq!(sk5.blocks.len(), 3, "{rep5}");
        assert!(rep5.contains("cleared: 3 non-air blocks and entities taken out in all"), "{rep5}");
        assert!(rep5.contains("  cleared 0,-1,0 minecraft:stone (from a, --clear)"), "every cleared cell printed: {rep5}");
        // written in the aligned frame: everything moves, `ground:` by its y
        let mut g = sk3.clone();
        g.meta.insert("ground".into(), "12".into());
        g.meta.insert("positions".into(), "1,2,3".into());
        let w = shifted(&g, [6, 1, 6]).unwrap();
        assert_eq!(w.meta["declared"], "8,4,10 powered_rail[powered=false]<8,3,10");
        assert_eq!(w.meta["ground"], "13");
        assert_eq!(w.meta["positions"], "7,3,9");
        assert_eq!(w.ports["output"], [15, 10, 15]);
        assert_eq!(w.blocks.len(), g.blocks.len());
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// (vcompose, G1 Nov 13 21:53) The traced compose: clears printed and no-ops refused, the stale
/// part's uncovered cells listed, a clash failing by name unless a precedence line settles it.
#[cfg(test)]
mod traced_tests {
    use super::*;

    fn part(name: &str, cells: &[(P, &str)]) -> Part {
        let v: Vec<(P, String)> = cells.iter().map(|(c, s)| (*c, s.to_string())).collect();
        block_part_named(name, name, &v)
    }

    #[test]
    fn cell_tokens_move_every_cell_and_nothing_else() {
        let t = "-2,0,4=connections -1,0,4+-2,0,5; holds -2,0,3=north_east (0.5,1,2 and 1,2 and 1,2,3,4 stay)";
        let cells: Vec<P> = cell_tokens(t).into_iter().map(|(_, _, c)| c).collect();
        assert_eq!(cells, vec![[-2, 0, 4], [-1, 0, 4], [-2, 0, 5], [-2, 0, 3]]);
        assert_eq!(move_cell_tokens(t, [0, -20, -1]), "-2,-20,3=connections -1,-20,3+-2,-20,4; holds -2,-20,2=north_east (0.5,1,2 and 1,2 and 1,2,3,4 stay)");
    }

    #[test]
    fn a_delta_parses_adds_and_removes_and_refuses_the_rest() {
        let (a, r) = parse_delta("# ADD\n9,-9,-5 minecraft:glass\n\n## REMOVE\n9,-13,-6\n").unwrap();
        assert_eq!(a, vec![([9, -9, -5], "minecraft:glass".to_string())]);
        assert_eq!(r, vec![[9, -13, -6]]);
        assert!(parse_delta("9,-9 minecraft:glass\n").is_err(), "a cell needs three coordinates");
        assert!(parse_delta("1,2,3 minecraft:glass extra\n").is_err(), "one state after the cell");
        assert!(parse_delta("1,2,3 minecraft:glass\n1,2,3\n").unwrap_err().contains("both added and removed"));
    }

    #[test]
    fn a_clash_fails_by_name_unless_a_precedence_line_settles_it() {
        let mk = || vec![part("A", &[([0, 0, 0], "minecraft:glass"), ([1, 0, 0], "minecraft:stone")]), part("B", &[([0, 0, 0], "minecraft:glowstone"), ([1, 0, 0], "minecraft:stone")])];
        let mut parts = mk();
        let mut l = Ledger::default();
        let e = apply_precedence(&mut parts, None, &Trace::default(), &mut l).unwrap_err();
        assert!(e.contains("CLASH") && e.contains("0,0,0: A = minecraft:glass vs B = minecraft:glowstone"), "{e}");
        let mut parts = mk();
        let mut l = Ledger::default();
        let tr = Trace { stale: None, precedence: vec![([0, 0, 0], "B".into(), "the owner's newer file".into())], ..Trace::default() };
        apply_precedence(&mut parts, None, &tr, &mut l).unwrap();
        assert!(!parts[0].placed.blocks.contains_key(&[0, 0, 0]) && parts[1].placed.blocks[&[0, 0, 0]] == "minecraft:glowstone");
        assert_eq!(l.same, vec!["A and B: 1 cells".to_string()], "the same state twice is counted, not a clash");
        assert!(l.resolved[0].contains("B's state taken over A's minecraft:glass (the owner's newer file)"), "{:?}", l.resolved);
        // a precedence line settling no clash refuses by name
        let mut parts = mk();
        let tr = Trace { stale: None, precedence: vec![([0, 0, 0], "B".into(), "x".into()), ([1, 0, 0], "A".into(), "y".into())], ..Trace::default() };
        let e = apply_precedence(&mut parts, None, &tr, &mut Ledger::default()).unwrap_err();
        assert!(e.contains("--precedence 1,0,0=A: settles no clash"), "{e}");
    }

    #[test]
    fn the_stale_part_gives_only_uncovered_cells_each_listed() {
        let mut parts = vec![part("OWNER", &[([0, 0, 0], "minecraft:glass"), ([1, 0, 0], "minecraft:glass")]), part("R3", &[([0, 0, 0], "minecraft:glass"), ([1, 0, 0], "minecraft:glowstone"), ([2, 0, 0], "minecraft:stone")])];
        let mut l = Ledger::default();
        apply_stale(&mut parts, 1, &mut l);
        assert_eq!(l.covered_same, 1);
        assert_eq!(l.superseded, vec!["1,0,0 minecraft:glowstone -> minecraft:glass (OWNER)".to_string()]);
        assert_eq!(l.uncovered, vec!["2,0,0 minecraft:stone".to_string()]);
        assert_eq!(parts[1].placed.blocks.keys().copied().collect::<Vec<_>>(), vec![[2, 0, 0]]);
        // the stale part is never a clash party
        apply_precedence(&mut parts, Some(1), &Trace::default(), &mut l).unwrap();
    }

    #[test]
    fn clears_print_every_cell_and_a_noop_refuses() {
        let mut stale = part("R3", &[([0, 0, 0], "minecraft:glass"), ([0, 1, 0], "minecraft:stone")]);
        stale.spec.relay = false;
        let mut parts = vec![stale, part("NEW", &[([0, 0, 0], "minecraft:stone")])];
        let mut l = Ledger::default();
        let n = apply_clears(&mut parts, &[Clear { b: ([0, 0, 0], [0, 5, 0]), why: "--clear box".into(), must: true, absent: None }], &mut l).unwrap();
        assert_eq!(n, 2);
        assert_eq!(l.cleared, vec!["0,0,0 minecraft:glass (from R3, --clear box)".to_string(), "0,1,0 minecraft:stone (from R3, --clear box)".to_string()]);
        assert!(parts[1].placed.blocks.contains_key(&[0, 0, 0]), "a re-laying part is never cleared");
        let e = apply_clears(&mut parts, &[Clear { b: ([0, 0, 0], [0, 5, 0]), why: "--clear again".into(), must: true, absent: None }], &mut Ledger::default()).unwrap_err();
        assert!(e.contains("takes out nothing") && e.contains("--clear again"), "{e}");
        apply_clears(&mut parts, &[Clear { b: ([0, 0, 0], [0, 0, 0]), why: "--clear-part".into(), must: false, absent: None }], &mut Ledger::default()).unwrap();
        // an acknowledged absent cell passes as a no-op, printed; the same line on a held cell refuses
        let mut l = Ledger::default();
        apply_clears(&mut parts, &[Clear { b: ([0, 0, 0], [0, 0, 0]), why: "--delta W's remove line".into(), must: true, absent: Some("cut against another file".into()) }], &mut l).unwrap();
        assert!(l.cleared[0].contains("absent, acknowledged") && l.cleared[0].contains("cut against another file"), "{:?}", l.cleared);
        parts[0].placed.blocks.insert([4, 0, 0], "minecraft:glass".into());
        let e = apply_clears(&mut parts, &[Clear { b: ([4, 0, 0], [4, 0, 0]), why: "--delta W's remove line".into(), must: true, absent: Some("x".into()) }], &mut Ledger::default()).unwrap_err();
        assert!(e.contains("declared absent (x) but it took out 1"), "{e}");
    }

    #[test]
    fn carried_declarations_drop_cleared_and_owner_cells_by_name() {
        let dir = std::env::temp_dir().join(format!("vcompose-carry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("r3.txt");
        std::fs::write(&f, "# contacts: 1,0,0=a wall; 5,0,0=another wall\n# integrity: 2,0,0=connections 3,0,0+2,0,1; holds 2,0,-1=north_east\n# settled state: 6,0,0 (a kind the composer can't carry)\nkeep-out: 7,0,0 8,0,0 (the swing)\nname: r3\n").unwrap();
        let spec = |name: &str, frame: P| PartSpec { name: name.into(), file: f.clone(), base: None, frame, tier: [0; 3], keep: None, relay: false, nether: false };
        let filt = CarryFilter { clear: vec![([7, 0, 0], [7, 0, 0])], stale: Some("R3".into()), held: [[1, 0, 0]].into_iter().collect(), ..CarryFilter::default() };
        let c = carried_filtered(&[spec("R3", [0; 3])], [0; 3], &filt).unwrap();
        let (h, m, dropped) = (c.heads, c.metas, c.dropped);
        assert!(h.iter().any(|l| l.starts_with("# contacts: 5,0,0=another wall")) && !h.iter().any(|l| l.contains("1,0,0=a wall")), "{h:?}");
        assert!(dropped.iter().any(|d| d.contains("1,0,0=a wall") && d.contains("an owner part holds its cell")), "{dropped:?}");
        assert!(dropped.iter().any(|d| d.contains("settled state") && d.contains("can't carry")), "{dropped:?}");
        assert!(dropped.iter().any(|d| d.contains("keep-out") && d.contains("7,0,0") && d.contains("cleared")), "{dropped:?}");
        assert!(m.iter().any(|(k, v)| k == "keep-out" && v.starts_with("8,0,0 ")), "{m:?}");
        assert!(c.stale_kept.iter().any(|l| l.starts_with("# contacts: 5,0,0=another wall")) && c.stale_kept.iter().any(|l| l.starts_with("keep-out: 8,0,0")), "every carried stale entry listed: {:?}", c.stale_kept);
        // (G1 Nov 13 23:21, D1's control) a stale `# stand-in:` naming a cell the record leaves
        // empty is dropped by name; its twin naming a block the record holds is carried and listed;
        // --keep-stale keeps an empty one by name
        std::fs::write(&f, "# stand-in: 1,0,0 a lever ON (a bench input)\n# stand-in: 2,0,0 a lever ON (a bench input)\n").unwrap();
        let rec = |cells: &[P]| CarryFilter { stale: Some("R3".into()), record: Some(cells.iter().copied().collect()), ..CarryFilter::default() };
        let c = carried_filtered(&[spec("R3", [0; 3])], [0; 3], &rec(&[[2, 0, 0]])).unwrap();
        assert!(c.dropped.iter().any(|d| d.contains("1,0,0") && d.contains("empty in the record")), "{:?}", c.dropped);
        assert!(c.stale_kept.len() == 1 && c.stale_kept[0].starts_with("# stand-in: 2,0,0"), "{:?}", c.stale_kept);
        let kept = CarryFilter { keep_stale: vec![([1, 0, 0], "confirmed".into())], ..rec(&[[2, 0, 0]]) };
        assert_eq!(carried_filtered(&[spec("R3", [0; 3])], [0; 3], &kept).unwrap().stale_kept.len(), 2);
        // an owner part's integrity line moves whole with the part; an unknown kind fails it
        std::fs::write(&f, "# integrity: 2,0,0=connections 3,0,0+2,0,1; holds 2,0,-1=north_east\n").unwrap();
        let h = carried_filtered(&[spec("FOLD", [0, -20, -1])], [0; 3], &CarryFilter::default()).unwrap().heads;
        assert!(h.iter().any(|l| l.starts_with("# integrity: 2,-20,-1=connections 3,-20,-1+2,-20,0; holds 2,-20,-2=north_east (carried from part FOLD")), "{h:?}");
        std::fs::write(&f, "# settled state: 6,0,0 (x)\n").unwrap();
        assert!(carried_filtered(&[spec("OWNER", [0; 3])], [0; 3], &CarryFilter::default()).unwrap_err().contains("can't carry"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_delta_carries_its_entry_lines_and_skips_its_prose() {
        let dir = std::env::temp_dir().join(format!("vc-delta-carry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("w.delta");
        std::fs::write(&f, "# V's way-in (b8949c96, 20:40): prose\n# contacts: 9,-11,0=the drop stop\n#   the arrival slope's high side: (9,-9,-5) > (9,-8,-6) (air)\n9,-11,0 minecraft:glass\n9,-13,-6\n").unwrap();
        let filt = CarryFilter { deltas: vec![PartSpec::parse(&format!("WAYIN={}", f.display())).unwrap()], ..CarryFilter::default() };
        let c = carried_filtered(&[], [0; 3], &filt).unwrap();
        let (heads, dropped) = (c.heads, c.dropped);
        assert!(heads.iter().any(|h| h.starts_with("# contacts: 9,-11,0=the drop stop (carried from part WAYIN, moved by 0,0,0)")), "{heads:?}");
        assert_eq!(heads.len(), 1, "the prose lines are not declarations: {heads:?}");
        assert!(dropped.is_empty(), "{dropped:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_stale_part_is_judged_against_the_winning_state() {
        // (G1 Nov 13 23:21, D3's control) A east_west, B north_west at one cell, R3 (stale)
        // north_west: with --precedence c=B the stale cell reads same-state; its c=A twin superseded
        let dir = std::env::temp_dir().join(format!("vc-d3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = |n: &str, shape: &str| {
            let p = dir.join(format!("{n}.txt"));
            std::fs::write(&p, format!("name: {n}\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\npalette:\n  R minecraft:rail[shape={shape},waterlogged=false]\n  S stone\nlayer -1:\n  S\nlayer 0:\n  R\n")).unwrap();
            p
        };
        let (a, b, r3) = (file("a", "east_west"), file("b", "north_west"), file("r3", "north_west"));
        let specs: Vec<PartSpec> = [("A", &a), ("B", &b), ("R3", &r3)].iter().map(|(n, p)| PartSpec::parse(&format!("{n}={}", p.display())).unwrap()).collect();
        let f = Frame { x0: -6, z0: -6, x1: 9, z1: 9, phase_x: -6, phase_z: -6 };
        let go = |win: &str| run_traced(&specs, Vec::new(), &[], &[], &[("output".into(), [0, 0, 0])], &f, "t", false, None, None, [0; 3], None, &Trace { stale: Some("R3".into()), precedence: vec![([0, 0, 0], win.into(), "the test".into())], ..Trace::default() });
        let (_, d, _, l) = go("B").unwrap();
        assert!(l.superseded.is_empty() && d.blocks[&[0, 0, 0]].contains("north_west"), "{:?}", l.superseded);
        let (_, d, _, l) = go("A").unwrap();
        assert!(l.superseded.len() == 1 && l.superseded[0].contains("-> minecraft:rail[shape=east_west") && l.superseded[0].contains("(A)") && d.blocks[&[0, 0, 0]].contains("east_west"), "{:?}", l.superseded);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_commands_path_fails_a_clash_and_an_unused_absent() {
        // (G1 Nov 13 23:21, D5) twins through the command's own path: its parsers
        // (parse_precedence, delta_inputs) and run_traced, as `mobfarm column` calls them (the
        // binary maps an error to exit 1; a test calling the CLI's run() would make every
        // unit:mobfarm build codegen that 10,000-line function): a clash errors; the same compose
        // with a --precedence line passes; an unused --absent errors; a delta's remove line taking
        // nothing errors, and passes acknowledged by --absent
        let dir = std::env::temp_dir().join(format!("vc-d5-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = |n: &str, block: &str| {
            let p = dir.join(format!("{n}.txt"));
            std::fs::write(&p, format!("name: {n}\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\npalette:\n  X {block}\nlayer 0:\n  X\n")).unwrap();
            p
        };
        let (a, b) = (file("a", "stone"), file("b", "glass"));
        let dfile = dir.join("d.delta");
        std::fs::write(&dfile, "# a remove line naming an empty cell\n9,9,9\n").unwrap();
        let specs = vec![PartSpec::parse(&format!("A={}", a.display())).unwrap(), PartSpec::parse(&format!("B={}", b.display())).unwrap()];
        let f = Frame { x0: -6, z0: -6, x1: 9, z1: 9, phase_x: -6, phase_z: -6 };
        let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let go = |prec: &[&str], delta: &[String], absent: &[&str]| -> Result<String, String> {
            let prec = parse_precedence(&strs(prec))?;
            let (clears, parts, _, _) = delta_inputs(delta, &strs(absent))?;
            run_traced(&specs, parts, &clears, &[], &[("output".into(), [0, 0, 0])], &f, "t", false, None, None, [0; 3], None, &Trace { stale: None, precedence: prec, ..Trace::default() }).map(|(s, ..)| s)
        };
        let e = go(&[], &[], &[]).unwrap_err();
        assert!(e.contains("CLASH") && e.contains("0,0,0"), "{e}");
        let rep = go(&["0,0,0=B:the test's ruling"], &[], &[]).unwrap();
        assert!(rep.contains("precedence 0,0,0: B's state taken over A's minecraft:stone (the test's ruling)"), "{rep}");
        let e = go(&["0,0,0=B:the test's ruling"], &[], &["5,5,5=nothing there"]).unwrap_err();
        assert!(e.contains("--absent names no delta remove line"), "{e}");
        let d = vec![format!("D={}", dfile.display())];
        let e = go(&["0,0,0=B:the test's ruling"], &d, &[]).unwrap_err();
        assert!(e.contains("takes out nothing") && e.contains("--delta D's remove line"), "{e}");
        go(&["0,0,0=B:the test's ruling"], &d, &["9,9,9=the owner's cell, not composed"]).unwrap();
        assert!(parse_precedence(&strs(&["0,0,0=B"])).unwrap_err().contains("reason"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_carried_tag_chain_names_each_part_once() {
        // (G1 Nov 14 00:38 (3)) rev 3's one-way lines carried "part M" twice, then "part R3"
        let t = tagged("# one-way: 1,0,0=signal climbs (carried from part M, moved by 0,0,0) (carried from part M, moved by 0,0,0)", "(carried from part R3, moved by 0,0,0)");
        assert_eq!(t, "# one-way: 1,0,0=signal climbs (carried from part M, moved by 0,0,0) (carried from part R3, moved by 0,0,0)");
        assert_eq!(tagged("a=b", "(carried from part X, moved by 1,2,3)"), "a=b (carried from part X, moved by 1,2,3)");
        assert_eq!(tagged("a=b (carried from part X, moved by 1,2,3)", "(carried from part X, moved by 1,2,3)"), "a=b (carried from part X, moved by 1,2,3)");
    }

    #[test]
    fn a_reserved_cell_held_fails_unless_acknowledged_with_its_ruling() {
        // (G1 Nov 14 01:22) a part on a reserved cell fails by name; --reserve-clash names the cell
        // and the ruling that resolves it (listed; the record not citable); an unused one refuses
        let dir = std::env::temp_dir().join(format!("vc-rc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        std::fs::write(&a, "name: a\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\npalette:\n  L minecraft:lantern[hanging=false,waterlogged=false]\nlayer 0:\n  L\n").unwrap();
        let specs = vec![PartSpec::parse(&format!("F3={}", a.display())).unwrap()];
        let f = Frame { x0: -6, z0: -6, x1: 9, z1: 9, phase_x: -6, phase_z: -6 };
        let r = Reserve::parse("R-line-end=0,0,0,0,1,0:day-1:R7").unwrap();
        assert_eq!((r.owner.as_deref(), r.item.as_deref()), (Some("day-1"), Some("R7")));
        assert!(Reserve::parse("x=0,0,0,0,1,0:day-1").is_err(), "the owner and the item come together");
        let go = |rc: Vec<(P, String, String)>| run_traced(&specs, Vec::new(), &[], std::slice::from_ref(&r), &[("output".into(), [0, 0, 0])], &f, "t", false, None, None, [0; 3], None, &Trace { reserve_clash: rc, ..Trace::default() });
        let e = go(Vec::new()).unwrap_err();
        assert!(e.contains("0,0,0 (reserved R-line-end (for day-1, item R7), held by F3: minecraft:lantern"), "{e}");
        let (rep, _, _, l) = go(vec![([0, 0, 0], "F3 r4".into(), "the lanterns move".into())]).unwrap();
        assert!(l.reserve_clash.len() == 1 && l.reserve_clash[0].contains("resolved by F3 r4: the lanterns move") && rep.contains("NOT CITABLE"), "{rep}");
        let e = go(vec![([0, 0, 0], "F3 r4".into(), "x".into()), ([5, 5, 5], "F3 r4".into(), "y".into())]).unwrap_err();
        assert!(e.contains("--reserve-clash 5,5,5=F3 r4: no part holds a reserved cell there"), "{e}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_stand_in_names_its_support_and_the_record_holds_it() {
        // (ironspec Nov 14 00:34, G1 01:22 / 03:39a) support=x,y,z held, an empty support, a stand-in
        // naming none, the token read right after the cell only, and support=none judged against the
        // record's block: a redstone block needs none, a lever does
        let record: BTreeMap<P, String> = [([-1, 27, 9], "minecraft:glass".to_string()), ([4, -12, -2], "minecraft:redstone_block".to_string()), ([6, 0, 0], "minecraft:lever[face=floor,facing=north,powered=false]".to_string())].into_iter().collect();
        let ok = vec!["# stand-in: -1,26,9 support=-1,27,9 (J(-1)'s lever under the glass)".to_string(), "# stand-in: 4,-12,-2 support=none (a redstone block)".to_string()];
        assert_eq!(stand_in_supports(&ok, &[], &record).unwrap().len(), 2);
        let e = stand_in_supports(&["# stand-in: 3,26,9 support=3,27,9 (J(3)'s lever)".to_string()], &[], &record).unwrap_err();
        assert!(e.contains("stand-in 3,26,9: its support 3,27,9 is empty in the record"), "{e}");
        let e = stand_in_supports(&["# stand-in: 5,26,9 (K(7)'s lever, under the glass (5,27,9))".to_string()], &[], &record).unwrap_err();
        assert!(e.contains("stand-in 5,26,9: names no support"), "{e}");
        let e = stand_in_supports(&["# stand-in: -1,26,9 (the prose names support=-1,27,9 later)".to_string()], &[], &record).unwrap_err();
        assert!(e.contains("stand-in -1,26,9: names no support right after its cell"), "a `support=` in the prose is not the token: {e}");
        let e = stand_in_supports(&["# stand-in: 6,0,0 support=none (a lever)".to_string()], &[], &record).unwrap_err();
        assert!(e.contains("support=none, but minecraft:lever[face=floor,facing=north,powered=false] needs a support"), "{e}");
        let e = stand_in_supports(&["# stand-in: 7,0,0 support=none (laid by the bench)".to_string()], &[], &record).unwrap_err();
        assert!(e.contains("the record holds no block there to judge"), "{e}");
        let em = vec![("emulated".to_string(), "8,-14,-5=lever[face=wall] support=9,-14,-5 (NOT-ready)".to_string())];
        assert!(stand_in_supports(&[], &em, &record).unwrap_err().contains("emulated 8,-14,-5: its support 9,-14,-5 is empty"));
        assert!(needs_support("minecraft:powered_rail[shape=north_south]") && needs_support("minecraft:redstone_wall_torch[facing=east]") && !needs_support("minecraft:iron_trapdoor[half=top]") && !needs_support("minecraft:sea_lantern") && !needs_support("minecraft:glass"));
    }

    #[test]
    fn a_walk_steps_up_at_most_1_25() {
        // the bed head's top (feet 0.5625) beside a glass top (feet 2.0): a 1.44 rise is no walk
        // (58f770cc 22:42); a 0.44 rise is
        let stop = vec![([0, 0, 0], 0.5625), ([1, 1, 0], 2.0), ([0, 0, 1], 0.5625), ([0, 0, 2], 1.0)];
        assert_eq!(walk_reach(&stop, &[[0, 0, 0]]).unwrap(), vec![true, false, true, true]);
        assert!(walk_reach(&stop, &[[9, 9, 9]]).is_none(), "no start among the positions: not judged");
        // from the glass top every lower neighbour is reached (a drop of at most 3)
        assert_eq!(walk_reach(&stop, &[[1, 1, 0]]).unwrap(), vec![true, true, true, true]);
    }
}
