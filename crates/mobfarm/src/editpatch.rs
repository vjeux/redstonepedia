//! (G1 Nov 14 08:52 / 09:47, parts 68 / 70) `mobfarm edit`'s in-place text edits: every mode (`--set`, `--swap-part`,
//! `--meta`, `--how`, `--chest`) changes the design's TEXT only where its edit requires, reading every line with the
//! parser's own classifier (`mctas::design::line_kind`, the one the parser and `interior_blank_rows` share) and the
//! parser's sections, and reads back what it wrote: the parsed result must equal the composed design, and every line
//! but the ones the edit names must equal the original's at its place. Nothing is re-serialized.

use mctas::design::{Design, LineKind, line_kind};
use std::collections::BTreeMap;

/// What one design line is, as the parser reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Role {
    Blank,
    Comment,
    /// A header line `key: value` (the parser's later one wins); `value` as the parser reads it (its comment cut).
    Header { key: String, value: String },
    /// `origin:` / `nether-origin:`
    Origin { nether: bool, at: [i32; 3] },
    /// Any other header-section line (`port`, `entity:`, `nbt:`, `nether nbt:`, `name:`).
    Other,
    PaletteHeader,
    /// A palette entry: its letter and state, as the parser reads them (comment cut, state trimmed).
    Entry { ch: char, state: String },
    /// `layer Y:` / `nether layer Y:`
    Layer { y: i32, nether: bool },
    /// A layer row: its layer, its row number in that layer (blank interior rows counted as the parser's Air reading).
    Row { y: i32, nether: bool, row: i32 },
}

/// Every line's [`Role`], the parser's sections and classifier: `palette:` starts the palette, `layer Y:` /
/// `nether layer Y:` a layer, any other unindented text a header line; an indented line is an entry or a row.
pub fn line_roles(text: &str) -> anyhow::Result<Vec<Role>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Sec {
        Header,
        Palette,
        Layer(i32, bool),
    }
    let mut sec = Sec::Header;
    let mut row = 0i32;
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let in_layer = matches!(sec, Sec::Layer(..));
        let line = match line_kind(raw, in_layer) {
            LineKind::Blank => {
                out.push(Role::Blank);
                continue;
            }
            LineKind::Comment => {
                out.push(Role::Comment);
                continue;
            }
            LineKind::Text(l) => l,
        };
        if !line.starts_with("  ") {
            let l = line.trim();
            if l == "palette:" {
                sec = Sec::Palette;
                out.push(Role::PaletteHeader);
            } else if let Some(y) = l.strip_prefix("nether layer ").and_then(|r| r.strip_suffix(':')) {
                let y: i32 = y.trim().parse().map_err(|e| anyhow::anyhow!("line {}: {raw:?}: {e}", n + 1))?;
                sec = Sec::Layer(y, true);
                row = 0;
                out.push(Role::Layer { y, nether: true });
            } else if let Some(y) = l.strip_prefix("layer ").and_then(|r| r.strip_suffix(':')) {
                let y: i32 = y.trim().parse().map_err(|e| anyhow::anyhow!("line {}: {raw:?}: {e}", n + 1))?;
                sec = Sec::Layer(y, false);
                row = 0;
                out.push(Role::Layer { y, nether: false });
            } else {
                sec = Sec::Header;
                if l.starts_with("port ") || l.starts_with("entity:") || l.starts_with("nbt:") || l.starts_with("nether nbt:") {
                    out.push(Role::Other);
                } else if let Some((k, v)) = l.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "origin" | "nether-origin" => {
                            let p: Vec<i32> = v.split_whitespace().map(|t| t.parse::<i32>()).collect::<Result<_, _>>().map_err(|e| anyhow::anyhow!("line {}: {raw:?}: {e}", n + 1))?;
                            anyhow::ensure!(p.len() == 3, "line {}: {raw:?}: x y z", n + 1);
                            out.push(Role::Origin { nether: k == "nether-origin", at: [p[0], p[1], p[2]] });
                        }
                        "name" => out.push(Role::Other),
                        _ => out.push(Role::Header { key: k.to_string(), value: v.to_string() }),
                    }
                } else {
                    anyhow::bail!("line {}: {raw:?}: unexpected", n + 1);
                }
            }
            continue;
        }
        match sec {
            Sec::Header => anyhow::bail!("line {}: {raw:?}: an indented line outside the palette and the layers", n + 1),
            Sec::Palette => {
                let body = line.trim();
                let mut it = body.chars();
                let ch = it.next().expect("non-empty");
                let state = it.as_str().trim().to_string();
                let state = mctas::blockstate::BlockState::parse(&state).map(|s| s.to_string()).unwrap_or(state);
                out.push(Role::Entry { ch, state });
            }
            Sec::Layer(y, nether) => {
                out.push(Role::Row { y, nether, row });
                row += 1;
            }
        }
    }
    Ok(out)
}

/// A text [`patch_cells`] wrote: its rows changed and its lines inserted (0-based in `text`: new palette entries, a grown
/// layer's rows, a new layer), and the original lines it removed (0-based in the original: entries the edit left unused).
pub struct Patched {
    pub text: String,
    pub changed: Vec<usize>,
    pub inserted: Vec<usize>,
    pub removed: Vec<usize>,
}

fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The design text with each overworld cell `(x,y,z) -> state` changed IN PLACE: its letter in its layer row replaced
/// (the row padded with `.`, its trailing air trimmed as the writer does; an all-air row stays `  .`); a state the
/// palette lacks given a letter the palette doesn't use, its entry after the palette's last entry; an entry no cell (the
/// Nether part's included) uses after the edit, and used before it, pruned; a row past a layer's last added at the
/// layer's end, a missing layer after the last overworld layer. Every other line is kept byte for byte, its line ending
/// included. A cell west or north of the origin (it would move the origin and every row) is refused by name.
pub fn patch_cells(original: &str, changes: &[([i32; 3], String)]) -> anyhow::Result<Patched> {
    let roles = line_roles(original)?;
    let nl = eol(original);
    let d0 = Design::parse(original)?;
    let origin = roles.iter().find_map(|r| match r {
        Role::Origin { nether: false, at } => Some(*at),
        _ => None,
    });
    let origin = origin.unwrap_or([0, 0, 0]);
    let mut by_state: BTreeMap<String, char> = BTreeMap::new();
    let mut used: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
    for r in &roles {
        if let Role::Entry { ch, state } = r {
            by_state.entry(state.clone()).or_insert(*ch);
            used.insert(*ch);
        }
    }
    let used_before: std::collections::BTreeSet<String> = d0.blocks.values().chain(d0.nether_blocks.values()).cloned().collect();
    let mut after = d0.blocks.clone();
    for (p, s) in changes {
        if s == "minecraft:air" {
            after.remove(p);
        } else {
            after.insert(*p, s.clone());
        }
    }
    let used_after: std::collections::BTreeSet<String> = after.values().chain(d0.nether_blocks.values()).cloned().collect();
    let pool: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$%&*+=?!<>^~/\\|;,'`\"(){}"
        .chars()
        .chain(('À'..='ÿ').filter(|c| c.is_alphabetic()))
        .chain(('Α'..='ω').filter(|c| c.is_alphabetic()))
        .chain(('Ѐ'..='ӿ').filter(|c| c.is_alphabetic()))
        .collect();
    struct Line {
        text: String,
        role: Role,
        orig: Option<usize>,
        changed: bool,
    }
    let mut ls: Vec<Line> = original
        .lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .zip(roles)
        .enumerate()
        .map(|(i, (text, role))| Line { text, role, orig: Some(i), changed: false })
        .collect();
    let mut added: Vec<(char, String)> = Vec::new();
    for (p, state) in changes {
        let ch = if state == "minecraft:air" {
            '.'
        } else if let Some(c) = by_state.get(state) {
            *c
        } else {
            let c = pool.iter().copied().find(|c| !used.contains(c)).ok_or_else(|| anyhow::anyhow!("edit: the palette has no letter left for {state}"))?;
            used.insert(c);
            by_state.insert(state.clone(), c);
            added.push((c, state.clone()));
            c
        };
        let (y, row, col) = (p[1] - origin[1], p[2] - origin[2], p[0] - origin[0]);
        anyhow::ensure!(row >= 0 && col >= 0, "edit: {},{},{} lies west or north of the design's origin: the edit would move its origin and every row; refused", p[0], p[1], p[2]);
        let start = match ls.iter().position(|l| l.role == Role::Layer { y, nether: false }) {
            Some(i) => i,
            None => {
                let last = ls.iter().rposition(|l| matches!(l.role, Role::Layer { nether: false, .. }) || matches!(l.role, Role::Row { nether: false, .. }));
                let at = last.map(|i| i + 1).unwrap_or(ls.len());
                ls.insert(at, Line { text: format!("layer {y}:"), role: Role::Layer { y, nether: false }, orig: None, changed: false });
                at
            }
        };
        let mut rows: Vec<usize> = Vec::new();
        let mut end = start + 1;
        for (i, l) in ls.iter().enumerate().skip(start + 1) {
            match l.role {
                Role::Row { y: ry, nether: false, .. } if ry == y => {
                    rows.push(i);
                    end = i + 1;
                }
                Role::Blank | Role::Comment => {}
                _ => break,
            }
        }
        while rows.len() <= row as usize {
            let r = rows.len() as i32;
            ls.insert(end, Line { text: "  .".to_string(), role: Role::Row { y, nether: false, row: r }, orig: None, changed: false });
            rows.push(end);
            end += 1;
        }
        let li = rows[row as usize];
        let mut body: Vec<char> = ls[li].text[2..].chars().collect();
        while body.len() <= col as usize {
            body.push('.');
        }
        body[col as usize] = ch;
        while body.len() > 1 && body.last() == Some(&'.') {
            body.pop();
        }
        ls[li].text = format!("  {}", body.iter().collect::<String>());
        ls[li].changed = ls[li].orig.is_some();
    }
    // the palette: entries the edit left unused pruned; new ones after the last entry
    let prune: Vec<usize> = ls.iter().enumerate().filter(|(_, l)| matches!(&l.role, Role::Entry { state, .. } if used_before.contains(state) && !used_after.contains(state))).map(|(i, _)| i).collect();
    let added: Vec<(char, String)> = added.into_iter().filter(|(_, s)| used_after.contains(s)).collect();
    if !added.is_empty() {
        let last = ls.iter().rposition(|l| matches!(l.role, Role::Entry { .. }) || l.role == Role::PaletteHeader).ok_or_else(|| anyhow::anyhow!("edit: the design has no palette to add {} to", added.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join(", ")))?;
        for (k, (c, s)) in added.into_iter().enumerate() {
            ls.insert(last + 1 + k, Line { text: format!("  {c} {s}"), role: Role::Entry { ch: c, state: s }, orig: None, changed: false });
        }
    }
    // (the pruned indices sit before any insertion point only when the palette precedes the layers; recompute by role)
    let removed: Vec<usize> = ls.iter().filter(|l| matches!(&l.role, Role::Entry { state, .. } if l.orig.is_some() && used_before.contains(state) && !used_after.contains(state))).filter_map(|l| l.orig).collect();
    let _ = prune;
    ls.retain(|l| !(l.orig.is_some_and(|o| removed.contains(&o))));
    let mut out = ls.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(nl);
    if original.ends_with('\n') {
        out.push_str(nl);
    }
    let changed: Vec<usize> = ls.iter().enumerate().filter(|(_, l)| l.changed).map(|(i, _)| i).collect();
    let inserted: Vec<usize> = ls.iter().enumerate().filter(|(_, l)| l.orig.is_none()).map(|(i, _)| i).collect();
    Ok(Patched { text: out, changed, inserted, removed })
}

/// A patched text read back: its overworld and Nether cells equal `want` / the original's Nether part, and, inserted
/// lines and pruned entries aside, every line equals the original's at its place unless the patch changed it; else the
/// edit fails by name.
pub fn readback_patch(original: &str, p: &Patched, want: &BTreeMap<[i32; 3], String>) -> anyhow::Result<()> {
    let back = Design::parse(&p.text)?;
    let d0 = Design::parse(original)?;
    anyhow::ensure!(&back.blocks == want, "edit read-back: the written text's cells differ from the composed ones ({} vs {} cells)", back.blocks.len(), want.len());
    anyhow::ensure!(back.nether_blocks == d0.nether_blocks, "edit read-back: the written text's Nether cells differ from the original's");
    anyhow::ensure!(back.meta == d0.meta && back.ports == d0.ports && back.entities.len() == d0.entities.len() && back.block_entities == d0.block_entities, "edit read-back: the written text's header (meta, ports, entities, nbt) differs from the original's");
    let orig: Vec<&str> = original.lines().enumerate().filter(|(i, _)| !p.removed.contains(i)).map(|(_, l)| l).collect();
    let kept: Vec<(usize, &str)> = p.text.lines().enumerate().filter(|(i, _)| !p.inserted.contains(i)).collect();
    anyhow::ensure!(kept.len() == orig.len(), "edit read-back: {} line(s) written for the original's {} (inserted and pruned lines aside)", kept.len(), orig.len());
    for ((i, l), o) in kept.iter().zip(orig.iter()) {
        anyhow::ensure!(p.changed.contains(i) || l == o, "edit read-back: line {} changed though the edit doesn't touch it: {o:?} -> {l:?}", i + 1);
    }
    Ok(())
}

/// The meta path: each `--meta k=v` (and `--how`) edits the design's `k:` lines IN PLACE, as the parser reads them (the
/// parser takes a key's LAST line, wherever it is): the last one's value replaced (its `# note` kept), every one removed
/// for an empty value, or a new line added after the head's last header line; a value holding `#` (the parser would cut
/// it as a comment) is refused; `--chest` through [`patch_cells`]. Read back: the parsed result must hold each key as
/// set and everything else as the original did, and every line but the ones edited must be kept at its place.
pub fn edit_meta_text(text: &str, chest: bool, how: Option<&str>, meta: &[String], chest_state: &str) -> anyhow::Result<String> {
    let nl = eol(text);
    let roles = line_roles(text)?;
    let mut lines: Vec<(String, Option<usize>)> = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).enumerate().map(|(i, l)| (l, Some(i))).collect();
    let mut kvs: Vec<(String, String)> = Vec::new();
    for kv in meta {
        let (k, v) = kv.split_once('=').ok_or_else(|| anyhow::anyhow!("--meta {kv:?}: not key=value"))?;
        kvs.push((k.trim().to_string(), v.trim().to_string()));
    }
    if let Some(h) = how {
        kvs.push(("how".to_string(), h.to_string()));
    }
    let mut edited: Vec<usize> = Vec::new();
    let mut roles = roles;
    for (k, v) in &kvs {
        anyhow::ensure!(!k.is_empty() && !k.contains([':', '#', ' ']) && !["name", "origin", "nether-origin", "palette", "entity", "nbt"].contains(&k.as_str()) && !k.starts_with("port"), "--meta {k}: not a meta key this path edits");
        anyhow::ensure!(!v.contains('#'), "--meta {k}={v}: a value can't hold `#` (the parser cuts it as a comment)");
        let at: Vec<usize> = roles.iter().enumerate().filter(|(_, r)| matches!(r, Role::Header { key, .. } if key == k)).map(|(i, _)| i).collect();
        if v.is_empty() {
            anyhow::ensure!(!at.is_empty(), "--meta {k}=: the design has no `{k}:` line to remove");
            for &i in at.iter().rev() {
                lines.remove(i);
                roles.remove(i);
            }
        } else if let Some(&i) = at.last() {
            let note = lines[i].0.find('#').map(|j| format!(" {}", lines[i].0[j..].trim_end())).unwrap_or_default();
            lines[i].0 = format!("{k}: {v}{note}");
            roles[i] = Role::Header { key: k.clone(), value: v.clone() };
            if let Some(o) = lines[i].1 {
                edited.push(o);
            }
        } else {
            let head_end = roles.iter().position(|r| matches!(r, Role::PaletteHeader | Role::Layer { .. } | Role::Origin { .. })).unwrap_or(roles.len());
            let last_key = roles[..head_end].iter().rposition(|r| matches!(r, Role::Header { .. } | Role::Other));
            let at = last_key.map(|i| i + 1).unwrap_or(head_end);
            lines.insert(at, (format!("{k}: {v}"), None));
            roles.insert(at, Role::Header { key: k.clone(), value: v.clone() });
        }
    }
    let mut out = lines.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join(nl);
    if text.ends_with('\n') {
        out.push_str(nl);
    }
    // read back: each key as set, everything else as the original
    let d0 = Design::parse(text)?;
    let back = Design::parse(&out)?;
    let mut want = d0.meta.clone();
    for (k, v) in &kvs {
        if v.is_empty() {
            want.remove(k);
        } else {
            want.insert(k.clone(), v.clone());
        }
    }
    anyhow::ensure!(back.meta == want, "edit (meta) read-back: the written text's meta differs from the edit's ({:?} vs {:?})", back.meta, want);
    anyhow::ensure!(back.blocks == d0.blocks && back.nether_blocks == d0.nether_blocks && back.ports == d0.ports && back.entities.len() == d0.entities.len() && back.block_entities == d0.block_entities, "edit (meta) read-back: cells, ports, entities or nbt changed");
    // every original line kept at its place but the edited and removed ones
    let kept_orig: Vec<(usize, &str)> = lines.iter().filter_map(|(l, o)| o.map(|o| (o, l.as_str()))).collect();
    let orig_lines: Vec<&str> = text.lines().map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    for (o, l) in &kept_orig {
        anyhow::ensure!(edited.contains(o) || orig_lines[*o] == *l, "edit (meta) read-back: line {} changed though the edit doesn't touch it", o + 1);
    }
    if chest {
        let d = Design::parse(&out)?;
        let port = *d.ports.get("output").ok_or_else(|| anyhow::anyhow!("no `port output` for the chest"))?;
        match d.blocks.get(&port) {
            None => {
                let mut want = d.blocks.clone();
                want.insert(port, chest_state.to_string());
                let p = patch_cells(&out, &[(port, chest_state.to_string())])?;
                readback_patch(&out, &p, &want)?;
                out = p.text;
            }
            Some(s) if s.trim_start_matches("minecraft:").starts_with("chest[") => {}
            Some(s) => anyhow::bail!("port output {port:?} holds {s}"),
        }
    }
    Ok(out)
}

/// The swap path's guard: every `#` line of `original` is in `out` (a multiset) but exactly the lines the swap reports
/// it rewrote or dropped (`touched`); a lost line it didn't touch fails the edit by name.
pub fn check_carried(original: &str, out: &str, touched: &[String]) -> anyhow::Result<()> {
    let mut have: BTreeMap<&str, usize> = BTreeMap::new();
    for l in out.lines().filter(|l| l.trim_start().starts_with('#')) {
        *have.entry(l).or_insert(0) += 1;
    }
    let mut waived: Vec<&str> = touched.iter().map(|s| s.as_str()).collect();
    let mut lost = Vec::new();
    for l in original.lines().filter(|l| l.trim_start().starts_with('#')) {
        match have.get_mut(l) {
            Some(n) if *n > 0 => *n -= 1,
            _ => match waived.iter().position(|w| *w == l) {
                Some(i) => {
                    waived.remove(i);
                }
                None => lost.push(l.to_string()),
            },
        }
    }
    anyhow::ensure!(lost.is_empty(), "edit --swap-part: {} `#` line(s) not carried that the swap didn't rewrite (nothing silently dropped): {}", lost.len(), lost.iter().map(|l| format!("{l:?}")).collect::<Vec<_>>().join("; "));
    Ok(())
}

/// The swap's declarations step read back line by line: `after` must equal `before` with exactly the `touched` lines
/// replaced or removed and the `written` lines (rewritten or carried) put in: every other line kept in order.
pub fn check_declarations_step(before: &str, after: &str, touched: &[String], written: &[String]) -> anyhow::Result<()> {
    let mut t: Vec<&str> = touched.iter().map(|s| s.as_str()).collect();
    let a: Vec<&str> = before
        .lines()
        .filter(|l| match t.iter().position(|x| x == l) {
            Some(i) => {
                t.remove(i);
                false
            }
            None => true,
        })
        .collect();
    let mut w: Vec<&str> = written.iter().map(|s| s.as_str()).collect();
    let b: Vec<&str> = after
        .lines()
        .filter(|l| match w.iter().position(|x| x == l) {
            Some(i) => {
                w.remove(i);
                false
            }
            None => true,
        })
        .collect();
    anyhow::ensure!(t.is_empty() && w.is_empty(), "edit --swap-part read-back: {} touched line(s) not found before, {} written line(s) not found after", t.len(), w.len());
    anyhow::ensure!(a == b, "edit --swap-part read-back: the declarations step changed a line it doesn't report ({} vs {} kept lines)", a.len(), b.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    /// (G1 Nov 14 09:47 (2)/(3), part 70) the edit paths on VALID files the old second classifier broke: notes on the
    /// origin, palette, layer and entry lines, a port / entity / nbt line, `#` lines between rows and inside the palette, a
    /// Nether part sharing the palette; `--set` and its set-back byte for byte, the Nether-used entry never pruned;
    /// `--chest`; the meta path editing the parser's LAST `how:` line (after the origin), refusing a `#` in a value; CRLF
    /// kept; a multi-byte palette letter; the swap waiver pinned (an untouched EITHER / COMMENT line kept, the rewritten
    /// one reported) and `check_carried` failing by name on a lost untouched `#` line
    #[test]
    fn edit_paths_read_lines_as_the_parser_does() {
        let original = "name: t # its name\ncircuit: mob-farm iron_ingot\norigin: 0 0 0  # the origin\nport output: 0 -1 0\nentity: minecraft:minecart 0.5 1.0625 0.5\nnbt: 2 0 0 {Items:[]}\nhow: old\npalette:  # the palette\n  A minecraft:stone  # floor\n# inside the palette\n  B minecraft:glass\n  ë minecraft:dirt\nlayer 0:  # the floor layer\n  AAë\n# between rows\n  ABA\nnether-origin: 10 0 10\nnether layer 0:\n  B\nhow: tail\n";
        let d0 = mctas::design::Design::parse(original).unwrap_or_else(|e| panic!("the fixture parses: {e}"));
        assert_eq!(d0.meta.get("how").map(String::as_str), Some("tail"), "the parser takes the last how: line");
        // `--set`: one row changes, a new entry after the last one; set back: byte for byte
        let (out, _) = crate::layouts::edit_set_text(original, &["1,0,0=minecraft:redstone_block".to_string()]).unwrap_or_else(|e| panic!("{e}"));
        assert!(out.contains("\n  ë minecraft:dirt\n  C minecraft:redstone_block\nlayer 0:  # the floor layer\n  ACë\n"), "{out}");
        let (back, _) = crate::layouts::edit_set_text(&out, &["1,0,0=minecraft:stone".to_string()]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, original, "set and set back, byte for byte");
        // the glass entry is the Nether's too: replacing the overworld's only glass keeps its entry
        let (g, _) = crate::layouts::edit_set_text(original, &["1,0,1=minecraft:stone".to_string()]).unwrap_or_else(|e| panic!("{e}"));
        assert!(g.contains("\n  B minecraft:glass\n") && g.contains("nether layer 0:\n  B\n"), "the Nether-used entry kept: {g}");
        // `--chest` on the output port's air cell
        let c = crate::layouts::edit_meta(original, true, None, &[]).unwrap_or_else(|e| panic!("{e}"));
        assert!(mctas::design::Design::parse(&c).unwrap().blocks.get(&[0, -1, 0]).is_some_and(|s| s.contains("chest")), "{c}");
        // the meta path: `--how new` edits the parser's LAST how: line (the tail one), never a silent no-op
        let h = crate::layouts::edit_meta(original, false, Some("new"), &[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(h, original.replace("how: tail\n", "how: new\n"), "the last how: line edited");
        assert_eq!(mctas::design::Design::parse(&h).unwrap().meta.get("how").map(String::as_str), Some("new"));
        let r = crate::layouts::edit_meta(original, false, None, &["how=".to_string()]).unwrap_or_else(|e| panic!("{e}"));
        assert!(!r.contains("how:") && !mctas::design::Design::parse(&r).unwrap().meta.contains_key("how"), "every how: line removed: {r}");
        let e = crate::layouts::edit_meta(original, false, None, &["note=a # b".to_string()]).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(e.contains("a value can't hold `#`"), "{e}");
        // CRLF kept
        let crlf = original.replace('\n', "\r\n");
        let (oc, _) = crate::layouts::edit_set_text(&crlf, &["1,0,0=minecraft:redstone_block".to_string()]).unwrap_or_else(|e| panic!("{e}"));
        assert!(oc.contains("\r\n  C minecraft:redstone_block\r\n") && !oc.replace("\r\n", "").contains('\n'), "CRLF kept");
        // the swap waiver: an untouched `# switches:` line kept, the old part's `# update-consumers:` line reported
        let design = "name: t\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\n# switches: 5,0,0=another owner's lever\n# update-consumers: intended 1,0,0=2,0,0 redstone_lamp (the old part's lamp)\npalette:\n  r minecraft:repeater[delay=1,facing=west,locked=false,powered=false]\n  l minecraft:redstone_lamp[lit=false]\n  s minecraft:stone\nlayer 0:\n  srl\n";
        let old = "name: old\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\npalette:\n  r minecraft:repeater[delay=1,facing=west,locked=false,powered=false]\n  l minecraft:redstone_lamp[lit=false]\nlayer 0:\n  rl\n";
        let new = "name: new\ncircuit: mob-farm iron_ingot\norigin: 0 0 0\nport output: 0 -1 0\npalette:\n  g minecraft:glass\nlayer 0:\n  gg\n";
        let (sw, _) = crate::layouts::edit_swap(design, &[(old.to_string(), new.to_string(), [1, 0, 0])]).unwrap_or_else(|e| panic!("{e}"));
        assert!(sw.contains("\n# switches: 5,0,0=another owner's lever\n") && !sw.contains("the old part's lamp"), "{sw}");
        let lost = super::check_carried(design, &sw.replace("# switches: 5,0,0=another owner's lever\n", ""), &["# update-consumers: intended 1,0,0=2,0,0 redstone_lamp (the old part's lamp)".to_string()]);
        assert!(lost.is_err_and(|e| e.to_string().contains("not carried that the swap didn't rewrite") && e.to_string().contains("# switches: 5,0,0=another owner's lever")), "a lost untouched line fails by name");
        // the parser's classifier, line by line
        let roles = super::line_roles(original).unwrap();
        assert!(matches!(roles[2], super::Role::Origin { nether: false, at: [0, 0, 0] }) && roles[7] == super::Role::PaletteHeader && roles[12] == super::Role::Layer { y: 0, nether: false } && roles[14] == super::Role::Comment, "{roles:?}");
    }
}
